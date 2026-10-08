//! Bounded, background reads for cached mailbox summaries.
//!
//! `QueryService` owns one reader thread and opens the MegaMail cache there.
//! Callers submit owned requests without blocking their UI thread; callbacks
//! run on the reader thread and should hand results back through the caller's
//! own event channel.

use std::collections::HashMap;
use std::io;
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::thread;

use crate::cache::{Cache, MessageCursor, MESSAGE_PAGE_LIMIT_MAX};
use crate::models::Message;

const REQUEST_QUEUE_CAPACITY: usize = 32;

/// A page request. `generation` belongs to the caller and should increase when
/// the folder or search changes; it lets the UI ignore results that have
/// already gone stale.
#[derive(Debug, Clone)]
pub struct PageRequest {
    pub account_id: u32,
    pub folder_path: String,
    pub folder_id: u32,
    pub query: String,
    pub generation: u64,
    pub cursor: Option<MessageCursor>,
    /// Clamped by the cache layer to the inclusive range 1..=500.
    pub limit: usize,
}

/// State accompanying a page callback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PageStatus {
    /// The requested page was read successfully.
    Ready,
    /// A queued first-page request was replaced by a newer request for the
    /// same account and folder, or its query generation was already obsolete.
    Superseded,
    /// The cache could not be opened or the page query failed.
    Failed(String),
}

/// Owned result delivered for every request accepted by [`QueryService`].
/// A missing `next_cursor` means the page exhausted the matching rows.
#[derive(Debug, Clone)]
pub struct PageResult {
    pub account_id: u32,
    pub folder_path: String,
    pub folder_id: u32,
    pub generation: u64,
    pub rows: Vec<Message>,
    pub next_cursor: Option<MessageCursor>,
    pub status: PageStatus,
}

/// Why a nonblocking request could not be accepted by the bounded queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubmitError {
    /// The queue is full. Retry after the UI has consumed or superseded work.
    Full,
    /// The query thread has exited.
    Closed,
}

type Completion = Box<dyn FnOnce(PageResult) + Send + 'static>;

struct PendingPage {
    request: PageRequest,
    complete: Completion,
}

/// A handle to one dedicated background cache reader.
///
/// The bounded channel protects rapid search changes from building an
/// unbounded work queue. Requests which fit receive exactly one callback;
/// rejected submissions return [`SubmitError`] synchronously. Pending first
/// pages for the same account/folder are coalesced to the latest one, and each
/// discarded accepted request receives a `Superseded` callback.
pub struct QueryService {
    sender: SyncSender<PendingPage>,
}

impl QueryService {
    /// Start the reader lane. Cache opening and schema initialization happen on
    /// its named thread, not on the caller; any open failure is reported through
    /// the callback for each subsequently accepted request.
    pub fn start() -> io::Result<QueryService> {
        let (sender, receiver) = mpsc::sync_channel(REQUEST_QUEUE_CAPACITY);
        thread::Builder::new()
            .name("megamail-query".to_string())
            .spawn(move || query_loop(receiver))?;
        Ok(QueryService { sender })
    }

    /// Queue one page request without waiting for SQLite. The callback runs on
    /// the query thread; it should send the result to the UI's event channel
    /// rather than calling UI APIs directly.
    pub fn request_page<F>(&self, request: PageRequest, callback: F) -> Result<(), SubmitError>
    where
        F: FnOnce(PageResult) + Send + 'static,
    {
        let pending = PendingPage {
            request,
            complete: Box::new(callback),
        };
        self.sender.try_send(pending).map_err(|error| match error {
            TrySendError::Full(_) => SubmitError::Full,
            TrySendError::Disconnected(_) => SubmitError::Closed,
        })
    }
}

fn query_loop(receiver: Receiver<PendingPage>) {
    // `Cache::open` resolves config/data paths from this isolated mail-core
    // crate's MegaMail config module, and is deliberately called on this thread.
    // After its one-time schema setup, keep only a separate read-only WAL handle.
    let cache = Cache::open()
        .and_then(|cache| {
            let reader = cache.read_connection()?;
            drop(cache);
            Ok(reader)
        })
        .map_err(|error| error.to_string());
    let mut current_generation = HashMap::<(u32, String), u64>::new();

    while let Ok(first) = receiver.recv() {
        let mut batch = vec![first];
        while let Ok(next) = receiver.try_recv() {
            batch.push(next);
        }
        for pending in coalesce(batch, &mut current_generation) {
            let result = match &cache {
                Err(error) => failed_result(&pending, error.clone()),
                Ok(cache) => match cache.message_page(
                    pending.request.account_id,
                    &pending.request.folder_path,
                    pending.request.folder_id,
                    &pending.request.query,
                    pending.request.cursor,
                    pending.request.limit.min(MESSAGE_PAGE_LIMIT_MAX),
                ) {
                    Ok(page) => PageResult {
                        account_id: pending.request.account_id,
                        folder_path: pending.request.folder_path.clone(),
                        folder_id: pending.request.folder_id,
                        generation: pending.request.generation,
                        rows: page.rows,
                        next_cursor: page.next_cursor,
                        status: PageStatus::Ready,
                    },
                    Err(error) => failed_result(&pending, error.to_string()),
                },
            };
            (pending.complete)(result);
        }
    }
}

fn failed_result(pending: &PendingPage, message: String) -> PageResult {
    PageResult {
        account_id: pending.request.account_id,
        folder_path: pending.request.folder_path.clone(),
        folder_id: pending.request.folder_id,
        generation: pending.request.generation,
        rows: Vec::new(),
        next_cursor: None,
        status: PageStatus::Failed(message),
    }
}

fn superseded(pending: PendingPage) {
    let result = PageResult {
        account_id: pending.request.account_id,
        folder_path: pending.request.folder_path,
        folder_id: pending.request.folder_id,
        generation: pending.request.generation,
        rows: Vec::new(),
        next_cursor: None,
        status: PageStatus::Superseded,
    };
    (pending.complete)(result);
}

fn coalesce(
    batch: Vec<PendingPage>,
    current_generation: &mut HashMap<(u32, String), u64>,
) -> Vec<PendingPage> {
    let mut pending = Vec::<PendingPage>::with_capacity(batch.len());

    for item in batch {
        let scope = (item.request.account_id, item.request.folder_path.clone());
        if item.request.cursor.is_none() {
            if current_generation
                .get(&scope)
                .is_some_and(|generation| item.request.generation < *generation)
            {
                superseded(item);
                continue;
            }
            // A fresh first page invalidates any older queued page work for this
            // mailbox. The active request, if any, is already on the reader and
            // remains safe because the caller can discard its old generation.
            let mut retained = Vec::with_capacity(pending.len());
            for old in pending.drain(..) {
                if old.request.account_id == scope.0 && old.request.folder_path == scope.1 {
                    superseded(old);
                } else {
                    retained.push(old);
                }
            }
            pending = retained;
            current_generation.insert(scope.clone(), item.request.generation);
            pending.push(item);
        } else if current_generation
            .get(&scope)
            .is_some_and(|generation| *generation != item.request.generation)
        {
            superseded(item);
        } else {
            pending.push(item);
        }
    }

    pending
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    fn request(
        account_id: u32,
        folder_path: &str,
        generation: u64,
        cursor: Option<MessageCursor>,
    ) -> PageRequest {
        PageRequest {
            account_id,
            folder_path: folder_path.to_string(),
            folder_id: 1,
            query: String::new(),
            generation,
            cursor,
            limit: 100,
        }
    }

    #[test]
    fn coalescing_completes_stale_callbacks_and_keeps_other_scopes() {
        let (completed_tx, completed_rx) = mpsc::channel();
        let pending = |request, tag| {
            let completed_tx = completed_tx.clone();
            PendingPage {
                request,
                complete: Box::new(move |result| {
                    completed_tx.send((tag, result.status)).unwrap();
                }),
            }
        };

        let mut current_generation = HashMap::new();
        let kept = coalesce(
            vec![
                pending(request(1, "INBOX", 1, None), "first query"),
                pending(
                    request(1, "INBOX", 1, Some(MessageCursor { before_uid: 40 })),
                    "old page",
                ),
                pending(request(1, "Archive", 4, None), "other folder"),
                pending(request(1, "INBOX", 2, None), "new query"),
                pending(
                    request(1, "INBOX", 2, Some(MessageCursor { before_uid: 10 })),
                    "new page",
                ),
                pending(
                    request(2, "INBOX", 7, Some(MessageCursor { before_uid: 10 })),
                    "unknown scope page",
                ),
            ],
            &mut current_generation,
        );

        assert_eq!(kept.len(), 4);
        assert!(kept
            .iter()
            .any(|item| item.request.folder_path == "Archive"));
        assert!(kept.iter().any(|item| {
            item.request.folder_path == "INBOX"
                && item.request.generation == 2
                && item.request.cursor.is_none()
        }));
        assert!(kept.iter().any(|item| {
            item.request.folder_path == "INBOX"
                && item.request.generation == 2
                && item.request.cursor.is_some()
        }));
        assert!(kept
            .iter()
            .any(|item| { item.request.account_id == 2 && item.request.cursor.is_some() }));

        let late = coalesce(
            vec![pending(
                request(1, "INBOX", 1, Some(MessageCursor { before_uid: 20 })),
                "late old page",
            )],
            &mut current_generation,
        );
        assert!(
            late.is_empty(),
            "a cursor from the previous generation is stale"
        );

        let completed: Vec<_> = completed_rx.try_iter().collect();
        assert_eq!(completed.len(), 3);
        assert!(completed
            .iter()
            .all(|(_, status)| status == &PageStatus::Superseded));
        for expected in ["first query", "late old page", "old page"] {
            assert!(completed.iter().any(|(tag, _)| *tag == expected));
        }
    }

    fn synthetic_message(
        account_id: u32,
        folder_id: u32,
        uid: u32,
        from_name: String,
        subject: String,
    ) -> Message {
        Message {
            id: uid,
            account_id,
            folder_id,
            uid,
            from_name,
            from_addr: format!("sender{uid}@example.test"),
            reply_to: String::new(),
            to: String::new(),
            cc: String::new(),
            subject,
            preview: format!("Synthetic preview for message {uid}"),
            body: String::new(),
            date: String::new(),
            timestamp: uid as i64,
            unread: false,
            starred: false,
            keywords: Vec::new(),
            has_attachment: false,
            message_id: String::new(),
            references: String::new(),
            importance: crate::models::Importance::Normal,
            due: 0,
        }
    }

    fn count_cached_pages(cache: &Cache, account_id: u32, path: &str, folder_id: u32) -> usize {
        let mut cursor = None;
        let mut count = 0;
        let mut pages = 0;
        loop {
            let page = cache
                .message_page(account_id, path, folder_id, "", cursor, 500)
                .expect("bounded page count");
            count += page.rows.len();
            pages += 1;
            assert!(pages <= 300, "cursor must advance through a finite dataset");
            match page.next_cursor {
                Some(next) => cursor = Some(next),
                None => break,
            }
        }
        count
    }

    /// Synthetic release-mode probe for bounded mailbox reads. It uses only an
    /// isolated in-memory SQLite cache; it never opens the user's XDG database.
    /// Run with the test filter, `--ignored`, and `--nocapture` to see timings.
    #[test]
    #[ignore = "synthetic 100k-row cache paging benchmark"]
    fn synthetic_100k_cache_paging_benchmark() {
        const MAIN_ROWS: u32 = 90_000;
        const SCOPE_ROWS: u32 = 1_000;
        const BATCH_ROWS: u32 = 1_000;
        const SWITCH_ROUNDS: usize = 10;

        let cache = Cache::in_memory().expect("isolated synthetic SQLite cache");
        let setup_started = Instant::now();

        for start in (1..=MAIN_ROWS).step_by(BATCH_ROWS as usize) {
            let end = (start + BATCH_ROWS - 1).min(MAIN_ROWS);
            let rows: Vec<_> = (start..=end)
                .map(|uid| {
                    let name = if uid == 1 {
                        "Rare Needle sender".to_string()
                    } else {
                        format!("Sender {uid}")
                    };
                    let subject = if uid % 4 == 0 {
                        format!("Frequent campaign {uid}")
                    } else {
                        format!("Routine message {uid}")
                    };
                    synthetic_message(1, 1, uid, name, subject)
                })
                .collect();
            cache.upsert_messages(1, "INBOX", &rows);
        }

        let mut scopes = Vec::new();
        for account_id in 2..=11 {
            let path = if account_id % 2 == 0 {
                "Archive"
            } else {
                "INBOX"
            };
            let folder_id = account_id + 100;
            let rows: Vec<_> = (1..=SCOPE_ROWS)
                .map(|uid| {
                    synthetic_message(
                        account_id,
                        folder_id,
                        uid,
                        format!("Account {account_id} sender {uid}"),
                        format!("Account {account_id} note {uid}"),
                    )
                })
                .collect();
            cache.upsert_messages(account_id, path, &rows);
            scopes.push((account_id, path.to_string(), folder_id));
        }
        let setup_elapsed = setup_started.elapsed();

        let verify_started = Instant::now();
        let mut indexed_headers = count_cached_pages(&cache, 1, "INBOX", 1);
        for (account_id, path, folder_id) in &scopes {
            indexed_headers += count_cached_pages(&cache, *account_id, path, *folder_id);
        }
        let verify_elapsed = verify_started.elapsed();
        assert_eq!(indexed_headers, 100_000);

        let at = Instant::now();
        let page = cache
            .message_page(1, "INBOX", 1, "", None, 500)
            .expect("first page");
        let page_elapsed = at.elapsed();
        assert_eq!(page.rows.len(), 500);
        assert_eq!(page.rows.first().unwrap().uid, MAIN_ROWS);
        assert_eq!(page.rows.last().unwrap().uid, MAIN_ROWS - 499);
        assert_eq!(
            page.next_cursor,
            Some(MessageCursor {
                before_uid: MAIN_ROWS - 499
            })
        );

        let at = Instant::now();
        let rare = cache
            .message_page(1, "INBOX", 1, "rare needle", None, 500)
            .expect("rare search");
        let rare_elapsed = at.elapsed();
        assert_eq!(rare.rows.len(), 1);
        assert_eq!(rare.rows[0].uid, 1);
        assert_eq!(rare.next_cursor, None);

        let at = Instant::now();
        let frequent = cache
            .message_page(1, "INBOX", 1, "frequent", None, 500)
            .expect("frequent search");
        let frequent_elapsed = at.elapsed();
        assert_eq!(frequent.rows.len(), 500);
        assert_eq!(frequent.rows[0].uid, MAIN_ROWS);
        assert_eq!(frequent.rows[1].account_id, 1);
        assert!(frequent
            .rows
            .iter()
            .all(|row| row.subject.contains("Frequent campaign")));
        assert!(frequent.next_cursor.is_some());

        let at = Instant::now();
        let mut switched = 0;
        for _ in 0..SWITCH_ROUNDS {
            for (account_id, path, folder_id) in &scopes {
                let result = cache
                    .message_page(*account_id, path, *folder_id, "", None, 500)
                    .expect("switched mailbox page");
                assert_eq!(result.rows.len(), 500);
                assert_eq!(result.rows[0].account_id, *account_id);
                assert_eq!(result.rows[0].folder_id, *folder_id);
                assert_eq!(result.rows[0].uid, SCOPE_ROWS);
                assert!(result.next_cursor.is_some());
                switched += 1;
            }
        }
        let switch_elapsed = at.elapsed();

        println!(
            "synthetic cache: 100000 headers; setup={setup_elapsed:?}; \
             bounded-verification={verify_elapsed:?}; page500={page_elapsed:?}; \
             rare-search={rare_elapsed:?}; frequent-search={frequent_elapsed:?}; \
             {switched} account/folder switches={switch_elapsed:?}"
        );
    }
}

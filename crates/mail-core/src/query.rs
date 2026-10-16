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

use crate::cache::{Cache, MESSAGE_PAGE_LIMIT_MAX, MessageCursor};
use crate::models::Message;

const REQUEST_QUEUE_CAPACITY: usize = 32;

/// A page request. `generation` belongs to the caller and should increase when
/// the folder, search, or observed mailbox epoch changes; it lets the UI ignore
/// results that have already gone stale. It is not itself a UIDVALIDITY value.
#[derive(Debug, Clone)]
pub struct PageRequest {
    pub order: crate::cache::PageOrder,
    pub after_sorted: Option<crate::cache::SortCursor>,
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
    /// same account and folder, or its query/cache generation was obsolete.
    Superseded,
    /// The cache could not be opened or the page query failed.
    Failed(String),
}

/// Owned result delivered for every request accepted by [`QueryService`].
/// A missing `next_cursor` means the page exhausted the matching rows.
#[derive(Debug, Clone)]
pub struct PageResult {
    pub next_sorted: Option<crate::cache::SortCursor>,
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
    /// Optional remote namespace supplied by the caller, separate from its
    /// search generation. Legacy requests track the first observed namespace.
    expected_epoch: Option<u64>,
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
        self.submit_page(request, None, Box::new(callback))
    }

    /// Submit with the persisted mailbox generation observed by the worker.
    /// Mismatched epochs receive Superseded, never rows from a reused UID
    /// namespace. Existing PageRequest/PageResult fields stay compatible.
    pub fn request_page_at_epoch<F>(
        &self,
        request: PageRequest,
        expected_epoch: u64,
        callback: F,
    ) -> Result<(), SubmitError>
    where
        F: FnOnce(PageResult) + Send + 'static,
    {
        self.submit_page(request, Some(expected_epoch), Box::new(callback))
    }

    fn submit_page(
        &self,
        request: PageRequest,
        expected_epoch: Option<u64>,
        complete: Completion,
    ) -> Result<(), SubmitError> {
        let pending = PendingPage {
            request,
            expected_epoch,
            complete,
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
    // After its one-time schema setup, run_queries keeps a separately configured
    // read-only WAL handle. Registration and all reads stay on this thread.
    run_queries(receiver, Cache::open().map_err(|error| error.to_string()));
}

fn run_queries(receiver: Receiver<PendingPage>, cache: Result<Cache, String>) {
    // This common production/test seam registers on every reader connection.
    // A registration failure is a failed page, never empty successful results.
    let cache = cache.and_then(|cache| {
        cache
            .read_connection_with(crate::search::register_sql_functions)
            .map_err(|error| error.to_string())
    });
    let mut current_generation = HashMap::<(u32, String), u64>::new();
    // Caller search generation and persisted UID namespace form the effective
    // generation. A cursor cannot silently cross either half.
    let mut epochs = HashMap::<(u32, String), (u64, u64)>::new();

    while let Ok(first) = receiver.recv() {
        let mut batch = vec![first];
        while let Ok(next) = receiver.try_recv() {
            batch.push(next);
        }
        for pending in coalesce(batch, &mut current_generation) {
            let result = match &cache {
                Err(error) => failed_result(&pending, error.clone()),
                Ok(cache) => read_page(cache, &pending, &mut epochs),
            };
            (pending.complete)(result);
        }
    }
}

fn read_page(
    cache: &Cache,
    pending: &PendingPage,
    epochs: &mut HashMap<(u32, String), (u64, u64)>,
) -> PageResult {
    let request = &pending.request;
    let scope = (request.account_id, request.folder_path.clone());
    let predicate = match crate::search::SearchQuery::parse(&request.query)
        .and_then(|query| query.sql_predicate(true))
    {
        Ok(predicate) => predicate,
        Err(error) => return failed_result(pending, error.to_string()),
    };
    let run = || -> rusqlite::Result<Option<(u64, crate::cache::MessagePage)>> {
        let before = cache
            .mailbox_epoch(request.account_id, &request.folder_path)?
            .generation;
        if pending
            .expected_epoch
            .is_some_and(|expected| expected != before)
            || epochs
                .get(&scope)
                .is_some_and(|(caller, epoch)| *caller == request.generation && *epoch != before)
        {
            return Ok(None);
        }
        let page = cache.message_page_ordered(
            request.account_id,
            &request.folder_path,
            request.folder_id,
            &predicate.clause,
            &predicate.values,
            request.cursor,
            request.after_sorted.as_ref(),
            request.order,
            request.limit.min(MESSAGE_PAGE_LIMIT_MAX),
        )?;
        let after = cache
            .mailbox_epoch(request.account_id, &request.folder_path)?
            .generation;
        if before != after {
            return Ok(None);
        }
        Ok(Some((before, page)))
    };
    match run() {
        Ok(Some((epoch, page))) => {
            // A reset after the final check still requires caller-side event
            // invalidation; no database read can make UI delivery atomic.
            epochs.insert(scope, (request.generation, epoch));
            PageResult {
                next_sorted: page.next_sorted,
                account_id: request.account_id,
                folder_path: request.folder_path.clone(),
                folder_id: request.folder_id,
                generation: request.generation,
                rows: page.rows,
                next_cursor: page.next_cursor,
                status: PageStatus::Ready,
            }
        }
        Ok(None) => stale_result(pending),
        Err(error) => failed_result(pending, error.to_string()),
    }
}

fn stale_result(pending: &PendingPage) -> PageResult {
    let mut result = failed_result(pending, String::new());
    result.status = PageStatus::Superseded;
    result
}

fn failed_result(pending: &PendingPage, message: String) -> PageResult {
    PageResult {
        next_sorted: None,
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
        next_sorted: None,
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
            order: Default::default(),
            after_sorted: None,
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
                expected_epoch: None,
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
        assert!(
            kept.iter()
                .any(|item| item.request.folder_path == "Archive")
        );
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
        assert!(
            kept.iter()
                .any(|item| { item.request.account_id == 2 && item.request.cursor.is_some() })
        );

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
        assert!(
            completed
                .iter()
                .all(|(_, status)| status == &PageStatus::Superseded)
        );
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

    fn synthetic_query_path(tag: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "megamail-query-{tag}-{}-{}.db",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    fn remove_query_fixture(path: &std::path::Path) {
        for suffix in ["", "-wal", "-shm", "-journal"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
        }
    }

    #[test]
    fn cache_sort_pages_before_limiting_and_keeps_equal_senders_and_changed_flags() {
        use crate::cache::{PageOrder, SortKey};
        let c = Cache::in_memory().unwrap();
        let mut rows = Vec::new();
        for (uid, address) in [
            (1, "z@example.test"),
            (400, "a@example.test"),
            (2, "a@example.test"),
            (8, "b@example.test"),
        ] {
            let mut m = synthetic_message(7, 17, uid, "Fictional".into(), format!("Subject {uid}"));
            m.from_addr = address.into();
            m.unread = true;
            rows.push(m);
        }
        c.upsert_messages(7, "INBOX", &rows);
        let order = PageOrder {
            key: SortKey::Sender,
            reverse: false,
        };
        let first = c
            .message_page_ordered(7, "INBOX", 17, "1", &[], None, None, order, 1)
            .unwrap();
        assert_eq!(first.rows[0].uid, 2);
        let next = c
            .message_page_ordered(
                7,
                "INBOX",
                17,
                "1",
                &[],
                first.next_cursor,
                first.next_sorted.as_ref(),
                order,
                2,
            )
            .unwrap();
        assert_eq!(
            next.rows.iter().map(|m| m.uid).collect::<Vec<_>>(),
            [400, 8]
        );
        let last = c
            .message_page_ordered(
                7,
                "INBOX",
                17,
                "1",
                &[],
                next.next_cursor,
                next.next_sorted.as_ref(),
                order,
                2,
            )
            .unwrap();
        assert_eq!(last.rows[0].uid, 1);
        assert!(last.next_cursor.is_none());
        let reverse = c
            .message_page_ordered(
                7,
                "INBOX",
                17,
                "1",
                &[],
                None,
                None,
                PageOrder {
                    reverse: true,
                    ..order
                },
                4,
            )
            .unwrap();
        assert_eq!(
            reverse.rows.iter().map(|m| m.uid).collect::<Vec<_>>(),
            [1, 8, 400, 2]
        );
        let unread_order = PageOrder {
            key: SortKey::Unread,
            reverse: false,
        };
        let first = c
            .message_page_ordered(7, "INBOX", 17, "1", &[], None, None, unread_order, 1)
            .unwrap();
        assert_eq!(first.rows[0].uid, 400);
        rows[1].unread = false;
        c.upsert_messages(7, "INBOX", &[rows[1].clone()]);
        let next = c
            .message_page_ordered(
                7,
                "INBOX",
                17,
                "1",
                &[],
                first.next_cursor,
                first.next_sorted.as_ref(),
                unread_order,
                1,
            )
            .unwrap();
        assert_eq!(
            next.rows[0].uid, 8,
            "bookmark retains the original unread rank even after opening a message"
        );
    }

    #[test]
    fn native_parsed_search_uses_real_sql_paging_and_private_body_exclusion() {
        let path = synthetic_query_path("parsed-search");
        let writer = Cache::open_at(&path).unwrap();
        let mut first = synthetic_message(7, 17, 41, "Alice".into(), "Launch review".into());
        first.unread = true;
        let mut second = first.clone();
        second.id = 40;
        second.uid = 40;
        let mut other = first.clone();
        other.account_id = 8;
        writer.upsert_messages(7, "INBOX", &[first.clone(), second]);
        writer.upsert_messages(8, "INBOX", &[other]);
        writer.upsert_messages(7, "Archive", &[first]);
        writer.save_body(7, "INBOX", 41, "Fictional body needle");
        let reader = writer
            .read_connection_with(crate::search::register_sql_functions)
            .unwrap();
        let mut epochs = HashMap::new();
        let lookup = |query: &str, cursor, limit, epochs: &mut HashMap<_, _>| {
            let mut r = request(7, "INBOX", 1, cursor);
            r.query = query.into();
            r.limit = limit;
            read_page(
                &reader,
                &PendingPage {
                    request: r,
                    expected_epoch: None,
                    complete: Box::new(|_| {}),
                },
                epochs,
            )
        };
        let first = lookup("from:alice subject:review is:unread", None, 1, &mut epochs);
        assert_eq!(first.status, PageStatus::Ready);
        assert_eq!(first.rows.iter().map(|m| m.uid).collect::<Vec<_>>(), [41]);
        let second = lookup(
            "from:alice subject:review is:unread",
            first.next_cursor,
            1,
            &mut epochs,
        );
        assert_eq!(second.rows.iter().map(|m| m.uid).collect::<Vec<_>>(), [40]);
        assert!(second.next_cursor.is_none());
        assert_eq!(lookup("body:needle", None, 100, &mut epochs).rows.len(), 1);
        assert!(
            lookup("-body:needle", None, 100, &mut epochs)
                .rows
                .is_empty()
        );
        assert!(
            lookup(r#"subject:"' OR 1=1 --""#, None, 100, &mut epochs)
                .rows
                .is_empty()
        );
        assert!(matches!(
            lookup("unknown:value", None, 100, &mut epochs).status,
            PageStatus::Failed(_)
        ));
        let raw = rusqlite::Connection::open(&path).unwrap();
        raw.execute("INSERT INTO sender_checks(account_id,folder_path,uid,trust,summary,findings,pgp) VALUES(7,'INBOX',41,'','','','encrypted')", []).unwrap();
        assert!(
            lookup("body:needle", None, 100, &mut epochs)
                .rows
                .is_empty()
        );
        assert!(
            lookup("-body:needle", None, 100, &mut epochs)
                .rows
                .is_empty()
        );
        drop(raw);
        drop(reader);
        drop(writer);
        remove_query_fixture(&path);
    }

    #[test]
    fn configured_reader_registers_real_predicates_and_keeps_writes_forbidden() {
        let path = synthetic_query_path("registered-functions");
        let writer = Cache::open_at(&path).unwrap();
        writer.upsert_messages(
            7,
            "INBOX",
            &[synthetic_message(
                7,
                17,
                41,
                "Synthetic sender".into(),
                "Legacy".into(),
            )],
        );
        let raw = rusqlite::Connection::open(&path).unwrap();
        raw.execute(
            "UPDATE messages SET subject = ?1 WHERE account_id = 7 AND folder_path = 'INBOX' AND uid = 41",
            ["before\0secret"],
        )
        .unwrap();
        drop(raw);
        assert_eq!(
            writer.load_messages(7, "INBOX", 17)[0].subject,
            "before secret"
        );
        let reader = writer
            .read_connection_with(|connection| {
                crate::search::register_sql_functions(connection)?;
                assert!(connection.is_readonly(rusqlite::DatabaseName::Main)?);
                let result: (Option<bool>, Option<bool>, Option<bool>) = connection.query_row(
                    "SELECT megamail_search_header_contains(subject, ?1), megamail_search_header_contains(subject, ?2), megamail_search_body_contains(NULL, ?3) FROM messages WHERE account_id = 7 AND folder_path = 'INBOX' AND uid = 41",
                    rusqlite::params!["secret", "before secret", "body"],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )?;
                assert_eq!(result, (Some(true), Some(true), None));
                assert!(connection.execute("DELETE FROM messages", []).is_err());
                Ok(())
            })
            .unwrap();
        assert_eq!(reader.load_messages(7, "INBOX", 17).len(), 1);
        drop(reader);
        drop(writer);
        remove_query_fixture(&path);
    }

    #[test]
    fn reader_registration_failure_retains_rows_and_completes_as_failed_page() {
        let path = synthetic_query_path("registration-failure");
        let writer = Cache::open_at(&path).unwrap();
        writer.upsert_messages(
            7,
            "INBOX",
            &[synthetic_message(
                7,
                17,
                41,
                "Synthetic sender".into(),
                "Retain this row".into(),
            )],
        );
        let cache = writer
            .read_connection_with(|connection| {
                assert!(connection.is_readonly(rusqlite::DatabaseName::Main)?);
                Err(rusqlite::Error::InvalidParameterName(
                    "synthetic registration failed".into(),
                ))
            })
            .map_err(|error| error.to_string());
        assert!(cache.is_err());
        let (sender, receiver) = mpsc::sync_channel(REQUEST_QUEUE_CAPACITY);
        let join = thread::spawn(move || run_queries(receiver, cache));
        let service = QueryService { sender };
        let (tx, rx) = mpsc::channel();
        service
            .request_page(request(7, "INBOX", 1, None), move |result| {
                tx.send(result).unwrap()
            })
            .unwrap();
        let result = rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap();
        assert!(
            matches!(result.status, PageStatus::Failed(ref reason) if reason.contains("synthetic registration failed"))
        );
        assert!(result.rows.is_empty());
        assert_eq!(
            writer.load_messages(7, "INBOX", 17)[0].subject,
            "Retain this row"
        );
        drop(service);
        join.join().unwrap();
        assert!(rx.try_recv().is_err());
        drop(writer);
        remove_query_fixture(&path);
    }

    #[test]
    fn query_lane_supersedes_search_and_cursor_after_uid_namespace_reset() {
        let path = std::env::temp_dir().join(format!(
            "megamail-query-epoch-{}-{}.db",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        let writer = Cache::open_at(&path).unwrap();
        let old = writer.observe_uid_validity(1, "INBOX", 100).unwrap();
        writer.upsert_messages(
            1,
            "INBOX",
            &(1..=6)
                .map(|uid| synthetic_message(1, 1, uid, "Sender".into(), "Old namespace".into()))
                .collect::<Vec<_>>(),
        );
        writer.upsert_messages(
            2,
            "INBOX",
            &[synthetic_message(
                2,
                1,
                6,
                "Sibling".into(),
                "Keep sibling".into(),
            )],
        );
        let (sender, receiver) = mpsc::sync_channel(REQUEST_QUEUE_CAPACITY);
        let query_path = path.clone();
        let join = thread::spawn(move || {
            let cache = Cache::open_at(&query_path)
                .and_then(|cache| cache.read_connection())
                .map_err(|e| e.to_string());
            run_queries(receiver, cache);
        });
        let service = QueryService { sender };
        let (tx, rx) = mpsc::channel();
        let mut first = request(1, "INBOX", 1, None);
        first.limit = 2;
        service
            .request_page_at_epoch(first, old.generation, move |result| {
                tx.send(result).unwrap()
            })
            .unwrap();
        let page = rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap();
        assert_eq!(page.status, PageStatus::Ready);
        assert_eq!(page.rows.len(), 2);
        let cursor = page.next_cursor.unwrap();
        let new = writer.observe_uid_validity(1, "INBOX", 200).unwrap();
        writer.upsert_messages(
            1,
            "INBOX",
            &[synthetic_message(
                1,
                1,
                6,
                "Sender".into(),
                "New namespace".into(),
            )],
        );
        let (tx, rx) = mpsc::channel();
        service
            .request_page(request(1, "INBOX", 1, Some(cursor)), move |result| {
                tx.send(result).unwrap()
            })
            .unwrap();
        let stale_cursor = rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap();
        assert_eq!(stale_cursor.status, PageStatus::Superseded);
        assert!(stale_cursor.rows.is_empty());
        let (tx, rx) = mpsc::channel();
        service
            .request_page_at_epoch(
                request(1, "INBOX", 2, None),
                old.generation,
                move |result| tx.send(result).unwrap(),
            )
            .unwrap();
        assert_eq!(
            rx.recv_timeout(std::time::Duration::from_secs(5))
                .unwrap()
                .status,
            PageStatus::Superseded
        );
        let (tx, rx) = mpsc::channel();
        service
            .request_page_at_epoch(
                request(1, "INBOX", 3, None),
                new.generation,
                move |result| tx.send(result).unwrap(),
            )
            .unwrap();
        let fresh = rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap();
        assert_eq!(fresh.status, PageStatus::Ready);
        assert_eq!(fresh.rows[0].subject, "New namespace");
        let (tx, rx) = mpsc::channel();
        service
            .request_page(request(2, "INBOX", 1, None), move |result| {
                tx.send(result).unwrap()
            })
            .unwrap();
        let sibling = rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap();
        assert_eq!(sibling.status, PageStatus::Ready);
        assert_eq!(sibling.rows[0].subject, "Keep sibling");
        drop(service);
        join.join().unwrap();
        drop(writer);
        for suffix in ["", "-wal", "-shm", "-journal"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
        }
    }

    #[test]
    fn failed_cache_open_completes_each_accepted_query_without_fake_empty_success() {
        let (sender, receiver) = mpsc::sync_channel(REQUEST_QUEUE_CAPACITY);
        let join = thread::spawn(move || {
            run_queries(
                receiver,
                Err("synthetic migration failure; database retained".into()),
            )
        });
        let service = QueryService { sender };
        let (tx, rx) = mpsc::channel();
        service
            .request_page(request(1, "INBOX", 1, None), move |result| {
                tx.send(result).unwrap()
            })
            .unwrap();
        let failed = rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap();
        assert!(matches!(failed.status, PageStatus::Failed(_)));
        assert!(failed.rows.is_empty());
        drop(service);
        join.join().unwrap();
        assert!(rx.try_recv().is_err());
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
        assert!(
            frequent
                .rows
                .iter()
                .all(|row| row.subject.contains("Frequent campaign"))
        );
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

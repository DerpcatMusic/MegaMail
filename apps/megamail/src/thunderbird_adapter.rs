//! Session-scoped adapter for Thunderbird's authenticated MailExtension API.
//!
//! The extension owns its OAuth/session state. This module keeps opaque
//! Thunderbird IDs in the live actor only; it never writes message IDs or page
//! rows to MegaMail's native cache.

use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, SyncSender, TrySendError};
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::thread;
use std::time::{Duration, Instant};

use base64::Engine as _;
use futures::channel::mpsc as gpui_mpsc;
use megamail_core::mail_text::MailLink;
use megamail_core::models::{
    Account, Folder, FolderKind, Importance, Message, ThreadLatest, ThreadSummary,
};
use megamail_core::query::{PageRequest, PageResult, PageStatus, SubmitError};
use megamail_core::thunderbird::{ThunderbirdAccount, ThunderbirdProfile};
use megamail_core::thunderbird_bridge::{BridgeFailureKind, Runtime, classify_bridge_error};
use megamail_core::worker::{
    MailRequest, NativeMutationAction, NativeMutationOutcome, NativeMutationReceipt,
    NativeMutationRequest, NativeMutationScope, OutgoingMessage, SendOutcome,
};
use serde_json::{Value, json};

use crate::live::{
    AttachmentSummary, MailboxEvent, PageSender, RequestSender, SenderIdentity,
    ThunderbirdAccountOutcome, ThunderbirdAccountState, WorkerEnvelope, WorkerMailboxEvent,
};

const PAGE_LIMIT: usize = 100;
const SESSION_MESSAGE_LIMIT: usize = 25_000;
const FOLDER_LIMIT: usize = 4_096;
const CONVERSATION_ID_LIMIT: usize = 256;
const CONVERSATION_FOLDER_LIMIT: usize = 8;
const CONVERSATION_MEMBER_LIMIT: usize = 100;
const ATTACHMENT_LIMIT: usize = 32;
const ATTACHMENT_BYTES_LIMIT: usize = 50 * 1024 * 1024;
const RAW_BYTES_LIMIT: usize = 100 * 1024 * 1024;
const TRANSFER_CHUNK: usize = 512 * 1024;
const COMMAND_QUEUE: usize = 64;
const THUNDERBIRD_REFRESH_INTERVAL: Duration = Duration::from_secs(60);
const BODY_CACHE_ENTRIES: usize = 8;
const BODY_CACHE_BYTES: usize = 16 * 1024 * 1024;
const READER_BODY_BYTES_LIMIT: usize = 2 * 1024 * 1024;
const BRIDGE_REQUEST_FRAME_LIMIT: usize = 1024 * 1024;
const TRANSFER_ID_FRAME_RESERVE: usize = 64;

type StartResult = Result<
    (Vec<StartedAccount>, Vec<ThunderbirdAccountOutcome>),
    (ThunderbirdAccountState, String),
>;

static PROFILE_ACTORS: OnceLock<Mutex<HashMap<PathBuf, Weak<SyncSender<Command>>>>> =
    OnceLock::new();
static NEXT_SESSION_UID: AtomicU64 = AtomicU64::new(1);

type EventSender = gpui_mpsc::UnboundedSender<MailboxEvent>;
type BodyKey = (u32, String, u32);

#[cfg(test)]
use crate::account_session::validate_store;
use crate::account_session::{SavedIdentity, SelectedAccount, SourceStore, read_store, store_path};

pub(super) struct StartedAccount {
    pub source: SavedIdentity,
    pub account: Account,
    pub folders: Vec<Folder>,
    pub request_sender: RequestSender,
    pub page_sender: PageSender,
    pub identities: Vec<SenderIdentity>,
    pub error: Option<String>,
}

enum Command {
    #[cfg(test)]
    PageProbe(SyncSender<(bool, bool, usize)>),
    Request {
        account_id: u32,
        request: MailRequest,
    },
    Page(PageRequest),
    AddAccounts {
        selected: Vec<SelectedAccount>,
        reconnectable_ids: HashSet<u32>,
        occupied_ids: HashSet<u32>,
        occupied_emails: HashSet<String>,
        response: SyncSender<StartResult>,
    },
    AddAccountsPrepared {
        selected: Vec<SelectedAccount>,
        reconnectable_ids: HashSet<u32>,
        occupied_ids: HashSet<u32>,
        occupied_emails: HashSet<String>,
        prefetched: HashMap<u32, Result<(Value, Value), String>>,
        response: SyncSender<StartResult>,
    },
    BodyFinished {
        account_id: u32,
        path: String,
        message_id: u32,
        uid: u32,
        result: Result<Value, String>,
    },
    ConversationFinished {
        account_id: u32,
        task: ConversationTask,
        result: Result<Value, String>,
    },
    FolderRefreshFinished {
        account_id: u32,
        folder_id: u32,
        path: String,
        force: bool,
        result: Result<Value, String>,
    },
    FolderTreeFinished {
        account_id: u32,
        result: Result<Value, String>,
    },
}

enum ConversationTask {
    Related {
        message_id: u32,
        input_partial: bool,
        scope_partial: bool,
    },
    ThreadSummaries {
        groups: Vec<(String, Vec<String>)>,
        fallback: Vec<(String, ThreadSummary)>,
        input_partial: bool,
        scope_partial: bool,
    },
}

struct FolderBinding {
    folder: Folder,
    remote_id: Value,
}

struct AccountBinding {
    app_id: u32,
    remote_id: Value,
    email: String,
    identities: Vec<SenderIdentity>,
    primary_identity_id: String,
    folders: Vec<FolderBinding>,
}

#[derive(Clone)]
struct BodyPayload {
    original: Arc<crate::reader_session::OriginalBody>,
    body: String,
    html: String,
    html_without_quote: Option<String>,
    links: Vec<MailLink>,
    has_attachment: Option<bool>,
    reply_to: Option<String>,
    references: Option<String>,
}

#[derive(Default)]
struct BodyCache {
    entries: HashMap<BodyKey, BodyPayload>,
    order: VecDeque<BodyKey>,
    bytes: usize,
}

impl BodyCache {
    fn get(&mut self, key: &BodyKey) -> Option<BodyPayload> {
        let payload = self.entries.get(key)?.clone();
        if let Some(index) = self.order.iter().position(|candidate| candidate == key) {
            self.order.remove(index);
        }
        self.order.push_back(key.clone());
        Some(payload)
    }

    fn insert(&mut self, key: BodyKey, payload: BodyPayload) {
        let size = payload.byte_len();
        if size > BODY_CACHE_BYTES {
            return;
        }
        if let Some(previous) = self.entries.remove(&key) {
            self.bytes = self.bytes.saturating_sub(previous.byte_len());
            if let Some(index) = self.order.iter().position(|candidate| candidate == &key) {
                self.order.remove(index);
            }
        }
        while self.entries.len() >= BODY_CACHE_ENTRIES
            || self.bytes.saturating_add(size) > BODY_CACHE_BYTES
        {
            let Some(oldest) = self.order.pop_front() else {
                break;
            };
            if let Some(removed) = self.entries.remove(&oldest) {
                self.bytes = self.bytes.saturating_sub(removed.byte_len());
            }
        }
        self.bytes = self.bytes.saturating_add(size);
        self.order.push_back(key.clone());
        self.entries.insert(key, payload);
    }
}

impl BodyPayload {
    fn byte_len(&self) -> usize {
        self.original.retained_bytes()
            + self.body.len()
            + self.html.len()
            + self.html_without_quote.as_ref().map_or(0, String::len)
            + self
                .links
                .iter()
                .map(|link| link.label.len() + link.url.len())
                .sum::<usize>()
            + self.reply_to.as_ref().map_or(0, String::len)
            + self.references.as_ref().map_or(0, String::len)
    }

    fn event(&self, message_id: u32, path: &str) -> WorkerMailboxEvent {
        WorkerMailboxEvent::Body {
            message_id,
            path: path.to_owned(),
            version: None,
            sender_check: None,
            original: Some(self.original.clone()),
            body: self.body.clone(),
            html: self.html.clone(),
            html_without_quote: self.html_without_quote.clone(),
            links: self.links.clone(),
            has_attachment: self.has_attachment,
            reply_to: self.reply_to.clone(),
            references: self.references.clone(),
        }
    }
}

enum SendFailure {
    Definite(String, bool),
    Uncertain(String),
}

impl From<(String, bool)> for SendFailure {
    fn from((message, connectivity): (String, bool)) -> Self {
        Self::Definite(message, connectivity)
    }
}

// Private injection seam: production always delegates to the existing bounded
// Runtime; tests substitute only the external query boundary, not actor dispatch.
#[derive(Clone)]
enum QueryClient {
    Runtime(Runtime),
    #[cfg(test)]
    Fixture(Arc<dyn Fn(&str, Value) -> Result<Value, String> + Send + Sync>),
}

impl QueryClient {
    fn call(&self, method: &str, params: Value) -> Result<Value, String> {
        match self {
            Self::Runtime(runtime) => runtime.call(method, params),
            #[cfg(test)]
            Self::Fixture(call) => call(method, params),
        }
    }
}

const PAGE_POLL_INTERVAL: Duration = Duration::from_millis(10);
const PAGE_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
static NEXT_PAGE_SESSION: AtomicU64 = AtomicU64::new(1);

// All scope is captured before dispatch. These are TB session identities, not
// IMAP UIDVALIDITY. Only the actor allocates native UIDs/cursors or maps rows.
#[derive(Clone)]
struct PageJob {
    request: PageRequest,
    source: PathBuf,
    session: u64,
    ticket: u64,
    account_remote: Value,
    folder_remote: Value,
    params: Value,
}

impl PageJob {
    fn result(&self, status: PageStatus) -> PageResult {
        PageResult {
            next_sorted: None,
            account_id: self.request.account_id,
            folder_path: self.request.folder_path.clone(),
            folder_id: self.request.folder_id,
            generation: self.request.generation,
            rows: Vec::new(),
            next_cursor: None,
            status,
        }
    }
}

struct PageCompletion {
    job: PageJob,
    result: Result<Value, String>,
}

struct PageFlight {
    job: PageJob,
    retired: bool,
}

// Exactly one persistent reader per profile actor, one flight including its
// completion. No worker holds a command sender or mutable mailbox state.
struct PageLane {
    sender: Option<SyncSender<PageJob>>,
    replies: Option<mpsc::Receiver<PageCompletion>>,
    join: Option<thread::JoinHandle<()>>,
}

impl PageLane {
    fn start(runtime: QueryClient) -> io::Result<Self> {
        let (sender, jobs) = mpsc::sync_channel::<PageJob>(1);
        let (done, replies) = mpsc::sync_channel(1);
        let join = thread::Builder::new()
            .name("megamail-thunderbird-page".into())
            .spawn(move || {
                while let Ok(job) = jobs.recv() {
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        runtime
                            .call("list", job.params.clone())
                            .and_then(bounded_page_value)
                    }))
                    .unwrap_or_else(|_| {
                        Err("Thunderbird page reader stopped unexpectedly.".into())
                    });
                    if done.send(PageCompletion { job, result }).is_err() {
                        break;
                    }
                }
            })?;
        Ok(Self {
            sender: Some(sender),
            replies: Some(replies),
            join: Some(join),
        })
    }
}

impl Drop for PageLane {
    fn drop(&mut self) {
        // Disconnect completion delivery before joining: neither a full command
        // queue nor a dropped actor can trap the reader in send(). Runtime list
        // waits retain the core's 15s deadline; its final clone still owns cleanup.
        self.sender.take();
        self.replies.take();
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

fn bounded_page_value(value: Value) -> Result<Value, String> {
    struct Budget(usize);
    impl io::Write for Budget {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if bytes.len() > self.0 {
                return Err(io::Error::other("page byte budget exceeded"));
            }
            self.0 -= bytes.len();
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    serde_json::to_writer(Budget(PAGE_RESPONSE_BYTES), &value)
        .map_err(|_| "Thunderbird message page exceeds the 2 MiB page budget.".to_owned())?;
    Ok(value)
}

struct ActorState {
    runtime: QueryClient,
    page_lane: Option<PageLane>,
    page_flight: Option<PageFlight>,
    page_session: u64,
    next_page_ticket: u64,
    #[cfg(test)]
    serial_page_control: bool,
    profile: ThunderbirdProfile,
    remote_accounts: Vec<Value>,
    command_sender: Weak<SyncSender<Command>>,
    accounts: HashMap<u32, AccountBinding>,
    message_by_remote: HashMap<(u32, u32, String), u32>,
    remote_by_uid: HashMap<(u32, u32), Value>,
    message_id_by_uid: HashMap<(u32, u32), String>,
    reply_identity_by_uid: HashMap<(u32, u32), (String, i64)>,
    pending_bodies: HashSet<(u32, String, u32)>,
    body_cache: BodyCache,
    active_folder_by_account: HashMap<u32, (u32, String)>,
    refresh_inflight: HashSet<(u32, u32)>,
    last_refresh_started: HashMap<(u32, u32), Instant>,
    retire_requested: bool,
    cursors: HashMap<(u32, u32, u32), String>,
    initial_pages: HashMap<(u32, u32), Value>,
    next_cursor: u32,
    event_sender: EventSender,
}

impl Drop for ActorState {
    fn drop(&mut self) {
        for account_id in self.accounts.keys().copied().collect::<Vec<_>>() {
            self.emit(
                account_id,
                WorkerMailboxEvent::AdapterStopped {
                    session: self.page_session,
                },
            );
        }
        self.retire_page(PageStatus::Failed(
            "[megamail-tb:disconnected] Thunderbird actor dropped.".into(),
        ));
        self.page_lane.take();
    }
}

/// Import selected scanner rows. Runtime startup, authentication, folders and
/// all bridge operations run on named background threads.
pub(super) fn import(
    selections: Vec<(ThunderbirdProfile, Vec<String>)>,
    reserved_ids: HashSet<u32>,
    running_thunderbird_sources: HashMap<u32, SavedIdentity>,
    reconnectable_ids: HashSet<u32>,
    existing_emails: Vec<String>,
    event_sender: EventSender,
) {
    let result = import_selected(
        selections,
        reserved_ids,
        running_thunderbird_sources,
        reconnectable_ids,
        existing_emails,
        event_sender.clone(),
    );
    match result {
        Ok((accounts, results, notice, store)) => {
            let error = outcome_error(&results);
            let _ = event_sender.unbounded_send(MailboxEvent::ThunderbirdReady {
                accounts,
                results,
                error,
                notice,
                inventory: Some(store),
            });
        }
        Err(error) => {
            let _ = event_sender.unbounded_send(MailboxEvent::ThunderbirdReady {
                accounts: Vec::new(),
                results: Vec::new(),
                error: Some(error),
                notice: None,
                inventory: None,
            });
        }
    }
}

/// Restore the private source manifest against profiles currently on disk.
pub(super) fn restore(
    store: SourceStore,
    healthy: HashMap<u32, SavedIdentity>,
    event_sender: EventSender,
    reserved_ids: HashSet<u32>,
    reconnectable_ids: HashSet<u32>,
    existing_emails: Vec<String>,
) {
    let result = crate::account_session::restore_with(
        store,
        healthy,
        || {
            megamail_core::thunderbird::discover_profiles()
            .map_err(|error| format!("Could not find saved Thunderbird profiles: {error}. Retry after resolving the source problem."))
        },
        |profile, selected, reconnectable, occupied, emails| {
            start_profile(
                profile,
                selected,
                reconnectable,
                event_sender.clone(),
                occupied,
                emails,
            )
        },
        reserved_ids,
        reconnectable_ids,
        existing_emails,
    );
    match result {
        Ok((accounts, results, notice)) => {
            let error = outcome_error(&results);
            let _ = event_sender.unbounded_send(MailboxEvent::ThunderbirdReady {
                accounts,
                results,
                error,
                notice,
                inventory: None,
            });
        }
        Err(error) => {
            let _ = event_sender.unbounded_send(MailboxEvent::ThunderbirdReady {
                accounts: Vec::new(),
                results: Vec::new(),
                error: Some(error),
                notice: None,
                inventory: None,
            });
        }
    }
}

fn import_selected(
    selections: Vec<(ThunderbirdProfile, Vec<String>)>,
    reserved_ids: HashSet<u32>,
    running_thunderbird_sources: HashMap<u32, SavedIdentity>,
    reconnectable_ids: HashSet<u32>,
    existing_emails: Vec<String>,
    event_sender: EventSender,
) -> Result<
    (
        Vec<StartedAccount>,
        Vec<ThunderbirdAccountOutcome>,
        Option<String>,
        SourceStore,
    ),
    String,
> {
    let mut store =
        read_store().map_err(|error| format!("Could not read Thunderbird imports: {error}"))?;
    let mut reserved = crate::account_registry::direct_inventory(&store)?;
    reserved.extend(reserved_ids.iter().copied());
    reserved.extend(
        store
            .profiles
            .iter()
            .flat_map(|profile| profile.accounts.iter().map(|account| account.app_id)),
    );
    let mut connected_emails: HashSet<String> = existing_emails
        .into_iter()
        .map(|email| normalize_email(&email))
        .filter(|email| !email.is_empty())
        .collect();
    let mut emails = connected_emails.clone();
    let mut connected_ids = reserved_ids.clone();
    let mut to_start = Vec::new();
    let mut results = Vec::new();

    for (mut profile, ids) in selections {
        let canonical = fs::canonicalize(&profile.path).map_err(|error| {
            format!(
                "Could not locate Thunderbird profile {}: {error}",
                profile.name
            )
        })?;
        profile.path = canonical.clone();
        let mut selected_accounts = Vec::new();
        let mut stored = store
            .profiles
            .iter()
            .find(|candidate| candidate.path == canonical)
            .map(|candidate| {
                candidate
                    .accounts
                    .iter()
                    .map(|a| (a.source_id.clone(), a.app_id))
                    .collect::<HashMap<_, _>>()
            })
            .unwrap_or_default();
        let selected_ids: std::collections::BTreeSet<String> = ids.into_iter().collect();
        for source_id in selected_ids {
            let Some(account) = profile
                .accounts
                .iter()
                .find(|account| account.account_id == source_id)
                .cloned()
            else {
                results.push(outcome(
                    &profile,
                    &source_id,
                    "",
                    ThunderbirdAccountState::Unsupported,
                    Some("This Thunderbird account is no longer in the selected profile.".into()),
                ));
                continue;
            };
            let email = account.email.clone().unwrap_or_default();
            if let Some(app_id) = stored.get(&source_id).copied() {
                let identity = SavedIdentity {
                    path: profile.path.clone(),
                    source_id: source_id.clone(),
                };
                if running_thunderbird_sources.get(&app_id) == Some(&identity)
                    && !reconnectable_ids.contains(&app_id)
                {
                    results.push(outcome(
                        &profile,
                        &source_id,
                        &email,
                        ThunderbirdAccountState::Connected,
                        Some("This Thunderbird account is already connected.".into()),
                    ));
                    continue;
                }
                if reserved_ids.contains(&app_id) && !reconnectable_ids.contains(&app_id) {
                    results.push(outcome(
                        &profile,
                        &source_id,
                        &email,
                        ThunderbirdAccountState::Failed,
                        Some("This Thunderbird account ID conflicts with another configured account.".into()),
                    ));
                    continue;
                }
            }
            if !email.trim().is_empty() && !emails.insert(normalize_email(&email)) {
                results.push(outcome(
                    &profile,
                    &source_id,
                    &email,
                    ThunderbirdAccountState::Failed,
                    Some("An account with this email address is already connected.".into()),
                ));
                continue;
            }
            let app_id = if let Some(app_id) = stored.get(&source_id).copied() {
                app_id
            } else {
                let app_id = crate::account_registry::next_id(&reserved)?;
                reserved.insert(app_id);
                stored.insert(source_id.clone(), app_id);
                app_id
            };
            selected_accounts.push(SelectedAccount { account, app_id });
        }
        if selected_accounts.is_empty() {
            continue;
        }
        to_start.push((profile, selected_accounts));
    }

    let (started, results) = crate::account_session::commit_selection_with(
        &mut store,
        &to_start,
        |candidate| {
            write_store(candidate)
                .map_err(|error| format!("Could not save selected Thunderbird accounts: {error}"))
        },
        || {
            let mut started = Vec::new();
            for (profile, accounts) in to_start.iter().cloned() {
                match start_profile(
                    profile.clone(),
                    accounts.clone(),
                    reconnectable_ids.clone(),
                    event_sender.clone(),
                    connected_ids.clone(),
                    connected_emails.clone(),
                ) {
                    Ok((ready, mut outcomes)) => {
                        for ready_account in &ready {
                            connected_ids.insert(ready_account.account.id);
                            connected_emails.insert(normalize_email(&ready_account.account.email));
                        }
                        started.extend(ready);
                        results.append(&mut outcomes);
                    }
                    Err((state, message)) => {
                        for selected in accounts {
                            results.push(outcome(
                                &profile,
                                &selected.account.account_id,
                                selected.account.email.as_deref().unwrap_or_default(),
                                state,
                                Some(message.clone()),
                            ));
                        }
                    }
                }
            }

            (started, results)
        },
    )?;

    let connected = results
        .iter()
        .filter(|result| result.state == ThunderbirdAccountState::Connected)
        .count();
    let failed = results.len().saturating_sub(connected);
    let notice = (connected > 0).then(|| {
        if failed == 0 {
            format!(
                "Connected {connected} Thunderbird account{}.",
                if connected == 1 { "" } else { "s" }
            )
        } else {
            format!(
                "Connected {connected} Thunderbird account{}; {failed} need attention.",
                if connected == 1 { "" } else { "s" }
            )
        }
    });
    Ok((started, results, notice, store))
}

fn write_store(store: &SourceStore) -> io::Result<()> {
    let bytes = serde_json::to_vec_pretty(store)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let path = store_path()?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    megamail_core::config::write_private_file(&path, text)
}

fn start_profile(
    profile: ThunderbirdProfile,
    selected: Vec<SelectedAccount>,
    reconnectable_ids: HashSet<u32>,
    event_sender: EventSender,
    occupied_ids: HashSet<u32>,
    occupied_emails: HashSet<String>,
) -> StartResult {
    if let Some(sender) = profile_sender(&profile.path)? {
        return add_accounts_to_actor(
            sender,
            selected,
            reconnectable_ids,
            occupied_ids,
            occupied_emails,
        );
    }

    let (command_tx, command_rx) = mpsc::sync_channel(COMMAND_QUEUE);
    let command_sender = Arc::new(command_tx);
    let command_sender_weak = Arc::downgrade(&command_sender);
    let cleanup_sender = command_sender_weak.clone();
    let spawn_cleanup_sender = command_sender_weak.clone();
    {
        let mut actors = profile_actors().lock().map_err(|_| {
            (
                ThunderbirdAccountState::Failed,
                "Thunderbird account registry is unavailable.".into(),
            )
        })?;
        actors.retain(|_, sender| sender.strong_count() > 0);
        if let Some(existing) = actors.get(&profile.path).and_then(Weak::upgrade) {
            drop(actors);
            return add_accounts_to_actor(
                existing,
                selected,
                reconnectable_ids,
                occupied_ids,
                occupied_emails,
            );
        }
        actors.insert(profile.path.clone(), Arc::downgrade(&command_sender));
    }
    let (ready_tx, ready_rx) = mpsc::sync_channel(1);
    let actor_profile = profile.clone();
    let profile_path = profile.path.clone();
    let spawn_cleanup_path = profile_path.clone();
    let actor_name = format!(
        "megamail-thunderbird-{}",
        profile
            .name
            .chars()
            .filter(char::is_ascii_alphanumeric)
            .take(24)
            .collect::<String>()
    );
    thread::Builder::new()
        .name(actor_name)
        .spawn(move || {
            let runtime = match Runtime::start(&actor_profile) {
                Ok(runtime) => QueryClient::Runtime(runtime),
                Err(error) => {
                    let _ = ready_tx.send(Err((
                        classify_runtime_error(&error),
                        friendly_error(&error),
                    )));
                    return;
                }
            };
            let accounts = match runtime.call("accounts", json!({})).and_then(array_result) {
                Ok(accounts) => accounts,
                Err(error) => {
                    let _ = ready_tx.send(Err((
                        classify_runtime_error(&error),
                        friendly_error(&error),
                    )));
                    return;
                }
            };
            let mut state = ActorState {
                runtime,
                page_lane: None,
                page_flight: None,
                page_session: NEXT_PAGE_SESSION.fetch_add(1, Ordering::Relaxed),
                next_page_ticket: 1,
                #[cfg(test)]
                serial_page_control: false,
                profile: actor_profile,
                remote_accounts: accounts,
                command_sender: command_sender_weak,
                accounts: HashMap::new(),
                message_by_remote: HashMap::new(),
                remote_by_uid: HashMap::new(),
                message_id_by_uid: HashMap::new(),
                reply_identity_by_uid: HashMap::new(),
                pending_bodies: HashSet::new(),
                body_cache: BodyCache::default(),
                active_folder_by_account: HashMap::new(),
                refresh_inflight: HashSet::new(),
                last_refresh_started: HashMap::new(),
                retire_requested: false,
                cursors: HashMap::new(),
                initial_pages: HashMap::new(),
                next_cursor: 1,
                event_sender: event_sender.clone(),
            };
            let prefetched = prefetch_accounts(&state.runtime, &state.remote_accounts, &selected);
            if ready_tx
                .send(state.add_accounts_prepared(
                    selected,
                    reconnectable_ids,
                    occupied_ids,
                    occupied_emails,
                    prefetched,
                ))
                .is_err()
            {
                return;
            }
            state.run(command_rx);
            let retired = state.retire_requested;
            drop(state);
            if retired {
                remove_profile_sender(&profile_path, &cleanup_sender);
            }
        })
        .map_err(|error| {
            remove_profile_sender(&spawn_cleanup_path, &spawn_cleanup_sender);
            (
                ThunderbirdAccountState::Failed,
                format!("Could not start Thunderbird worker: {error}"),
            )
        })?;

    ready_rx.recv().map_err(|_| {
        (
            ThunderbirdAccountState::Failed,
            "Thunderbird stopped before connecting.".into(),
        )
    })?
}

fn profile_actors() -> &'static Mutex<HashMap<PathBuf, Weak<SyncSender<Command>>>> {
    PROFILE_ACTORS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn profile_sender(
    path: &Path,
) -> Result<Option<Arc<SyncSender<Command>>>, (ThunderbirdAccountState, String)> {
    let mut actors = profile_actors().lock().map_err(|_| {
        (
            ThunderbirdAccountState::Failed,
            "Thunderbird account registry is unavailable.".into(),
        )
    })?;
    actors.retain(|_, sender| sender.strong_count() > 0);
    Ok(actors.get(path).and_then(Weak::upgrade))
}

fn remove_profile_sender(path: &Path, sender: &Weak<SyncSender<Command>>) {
    if let Ok(mut actors) = profile_actors().lock() {
        if actors
            .get(path)
            .is_some_and(|current| Weak::ptr_eq(current, sender))
        {
            actors.remove(path);
        }
    }
}

fn add_accounts_to_actor(
    sender: Arc<SyncSender<Command>>,
    selected: Vec<SelectedAccount>,
    reconnectable_ids: HashSet<u32>,
    occupied_ids: HashSet<u32>,
    occupied_emails: HashSet<String>,
) -> StartResult {
    let (response_tx, response_rx) = mpsc::sync_channel(1);
    sender
        .try_send(Command::AddAccounts {
            selected,
            reconnectable_ids,
            occupied_ids,
            occupied_emails,
            response: response_tx,
        })
        .map_err(|error| match error {
            TrySendError::Full(_) => (
                ThunderbirdAccountState::Failed,
                "Thunderbird is busy. Try importing the account again shortly.".into(),
            ),
            TrySendError::Disconnected(_) => (
                ThunderbirdAccountState::Failed,
                "The existing Thunderbird session stopped. Try importing again.".into(),
            ),
        })?;
    response_rx.recv().map_err(|_| {
        (
            ThunderbirdAccountState::Failed,
            "The Thunderbird session stopped before adding the selected account.".into(),
        )
    })?
}

fn prefetch_accounts(
    runtime: &QueryClient,
    remote_accounts: &[Value],
    selected: &[SelectedAccount],
) -> HashMap<u32, Result<(Value, Value), String>> {
    thread::scope(|scope| {
        let handles = selected
            .iter()
            .map(|selected_account| {
                let selected_account = selected_account.clone();
                let remote_account = find_remote_account(remote_accounts, &selected_account);
                let runtime = runtime.clone();
                let app_id = selected_account.app_id;
                let handle = scope.spawn(move || {
                    let remote_account = remote_account?;
                    let remote_id = remote_account.get("id").cloned().ok_or_else(|| {
                        "Thunderbird returned an account without an ID.".to_owned()
                    })?;
                    let folders = runtime.call("folders", json!({ "accountId": remote_id }))?;
                    let parsed = parse_folders(app_id, &folders)?;
                    let initial_folder = parsed
                        .iter()
                        .find(|folder| folder.kind == FolderKind::Inbox)
                        .or_else(|| parsed.first())
                        .ok_or_else(|| {
                            "Thunderbird returned no mail folders for this account.".to_owned()
                        })?;
                    let initial_page = runtime.call(
                        "list",
                        json!({
                            "folderId": initial_folder.path.clone(),
                            "limit": PAGE_LIMIT,
                        }),
                    )?;
                    Ok((folders, initial_page))
                });
                (app_id, handle)
            })
            .collect::<Vec<_>>();
        handles
            .into_iter()
            .map(|(app_id, handle)| {
                let result = handle.join().unwrap_or_else(|_| {
                    Err("Thunderbird account discovery worker stopped unexpectedly.".into())
                });
                (app_id, result)
            })
            .collect()
    })
}

fn find_remote_account(
    remote_accounts: &[Value],
    selected: &SelectedAccount,
) -> Result<Value, String> {
    if let Some(account) = remote_accounts
        .iter()
        .find(|account| string_id(account.get("id")) == selected.account.account_id)
    {
        return Ok(account.clone());
    }
    let email = selected
        .account
        .email
        .as_deref()
        .map(normalize_email)
        .unwrap_or_default();
    let matching = remote_accounts
        .iter()
        .filter(|account| {
            identity_values(account)
                .iter()
                .any(|identity| normalize_email(&string(identity.get("email"))) == email)
        })
        .collect::<Vec<_>>();
    if email.is_empty() || matching.len() != 1 {
        return Err(
            "This account could not be matched to Thunderbird's authenticated account list.".into(),
        );
    }
    Ok(matching[0].clone())
}

impl ActorState {
    fn run(&mut self, command_rx: mpsc::Receiver<Command>) {
        let mut next_periodic_refresh = Instant::now() + THUNDERBIRD_REFRESH_INTERVAL;
        let mut commands_since_page_poll = 0;
        loop {
            if self.retire_requested {
                break;
            }
            let mut timeout = next_periodic_refresh.saturating_duration_since(Instant::now());
            if self.page_flight.is_some() {
                timeout = timeout.min(PAGE_POLL_INTERVAL);
            }
            let command = match command_rx.recv_timeout(timeout) {
                Ok(command) => Some(command),
                Err(mpsc::RecvTimeoutError::Timeout) => None,
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            };
            if let Some(command) = command {
                commands_since_page_poll += 1;
                match command {
                    #[cfg(test)]
                    Command::PageProbe(reply) => {
                        let _ = reply.send((
                            self.page_flight.is_some(),
                            self.remote_by_uid.values().any(|id| *id == json!(43)),
                            self.cursors.len(),
                        ));
                    }
                    Command::Request {
                        account_id,
                        request,
                    } => self.handle_request(account_id, request),
                    Command::Page(request) => self.handle_page(request),
                    Command::AddAccounts {
                        selected,
                        reconnectable_ids,
                        occupied_ids,
                        occupied_emails,
                        response,
                    } => {
                        if let Err((_, error)) = self.schedule_add_accounts(
                            selected,
                            reconnectable_ids,
                            occupied_ids,
                            occupied_emails,
                            response.clone(),
                        ) {
                            let _ = response.send(Err((ThunderbirdAccountState::Failed, error)));
                        }
                    }
                    Command::AddAccountsPrepared {
                        selected,
                        reconnectable_ids,
                        occupied_ids,
                        occupied_emails,
                        prefetched,
                        response,
                    } => {
                        let _ = response.send(self.add_accounts_prepared(
                            selected,
                            reconnectable_ids,
                            occupied_ids,
                            occupied_emails,
                            prefetched,
                        ));
                    }
                    Command::BodyFinished {
                        account_id,
                        path,
                        message_id,
                        uid,
                        result,
                    } => {
                        self.handle_body_finished(account_id, path, message_id, uid, result);
                    }
                    Command::ConversationFinished {
                        account_id,
                        task,
                        result,
                    } => self.handle_conversation_finished(account_id, task, result),
                    Command::FolderRefreshFinished {
                        account_id,
                        folder_id,
                        path,
                        force,
                        result,
                    } => self
                        .handle_folder_refresh_finished(account_id, folder_id, path, force, result),
                    Command::FolderTreeFinished { account_id, result } => {
                        self.handle_folder_tree_finished(account_id, result)
                    }
                }
            } else {
                // Body admission/completion and ordered mutations win over the
                // single read completion whenever commands remain queued.
                self.poll_page();
                commands_since_page_poll = 0;
            }
            // A continuously replenished command queue cannot retain a finished
            // read/gate forever. At most one bounded page projection per full
            // command-queue burst; no command or mutation is reordered.
            if commands_since_page_poll >= COMMAND_QUEUE {
                self.poll_page();
                commands_since_page_poll = 0;
            }
            if self.retire_requested {
                break;
            }
            if Instant::now() >= next_periodic_refresh {
                self.refresh_connected_inboxes();
                next_periodic_refresh = Instant::now() + THUNDERBIRD_REFRESH_INTERVAL;
            }
        }
        self.retire_page(PageStatus::Failed(
            "[megamail-tb:disconnected] Thunderbird session stopped.".into(),
        ));
        self.retire_requested = true;
        // Retire already-admitted page commands, without executing any queued
        // mutations during shutdown. The receiver closes before joining the
        // read lane, so new submissions fail instead of extending its lifetime.
        for _ in 0..COMMAND_QUEUE {
            let Ok(command) = command_rx.try_recv() else {
                break;
            };
            if let Command::Page(request) = command {
                let _ = self
                    .event_sender
                    .unbounded_send(MailboxEvent::Page(PageResult {
                        next_sorted: None,
                        account_id: request.account_id,
                        folder_path: request.folder_path,
                        folder_id: request.folder_id,
                        generation: request.generation,
                        rows: Vec::new(),
                        next_cursor: None,
                        status: PageStatus::Failed(
                            "[megamail-tb:disconnected] Thunderbird session stopped.".into(),
                        ),
                    }));
            }
        }
        drop(command_rx);
        self.page_lane.take();
        self.page_flight.take();
    }

    fn schedule_add_accounts(
        &self,
        selected: Vec<SelectedAccount>,
        reconnectable_ids: HashSet<u32>,
        occupied_ids: HashSet<u32>,
        occupied_emails: HashSet<String>,
        response: SyncSender<StartResult>,
    ) -> Result<(), (ThunderbirdAccountState, String)> {
        let sender = self.command_sender.upgrade().ok_or_else(|| {
            (
                ThunderbirdAccountState::Failed,
                "The Thunderbird session is shutting down.".into(),
            )
        })?;
        let runtime = self.runtime.clone();
        let remote_accounts = self.remote_accounts.clone();
        thread::Builder::new()
            .name("megamail-thunderbird-account-discovery".into())
            .spawn(move || {
                let prefetched = prefetch_accounts(&runtime, &remote_accounts, &selected);
                let _ = sender.send(Command::AddAccountsPrepared {
                    selected,
                    reconnectable_ids,
                    occupied_ids,
                    occupied_emails,
                    prefetched,
                    response,
                });
            })
            .map_err(|error| {
                (
                    ThunderbirdAccountState::Failed,
                    format!("Could not schedule Thunderbird account discovery: {error}"),
                )
            })?;
        Ok(())
    }

    fn add_accounts_prepared(
        &mut self,
        selected: Vec<SelectedAccount>,
        reconnectable_ids: HashSet<u32>,
        occupied_ids: HashSet<u32>,
        occupied_emails: HashSet<String>,
        mut prefetched: HashMap<u32, Result<(Value, Value), String>>,
    ) -> StartResult {
        let broken_prefetch = prefetched.iter().find_map(|(account_id, result)| {
            result
                .as_ref()
                .err()
                .filter(|error| is_broken_bridge_error(error))
                .map(|error| (*account_id, error.clone()))
        });
        let profile = self.profile.clone();
        let remote_accounts = self.remote_accounts.clone();
        let command_sender = self.command_sender.upgrade().ok_or_else(|| {
            (
                ThunderbirdAccountState::Failed,
                "The Thunderbird session is shutting down.".into(),
            )
        })?;
        let mut started = Vec::new();
        let mut outcomes = Vec::new();
        for selected_account in selected {
            let reconnecting = reconnectable_ids.contains(&selected_account.app_id);
            if (occupied_ids.contains(&selected_account.app_id)
                || self.accounts.contains_key(&selected_account.app_id))
                && !reconnecting
            {
                outcomes.push(outcome(
                    &profile,
                    &selected_account.account.account_id,
                    selected_account
                        .account
                        .email
                        .as_deref()
                        .unwrap_or_default(),
                    ThunderbirdAccountState::Failed,
                    Some("This Thunderbird account is already active in this session.".into()),
                ));
                continue;
            }
            if selected_account
                .account
                .email
                .as_deref()
                .map(normalize_email)
                .is_some_and(|email| !email.is_empty() && occupied_emails.contains(&email))
            {
                outcomes.push(outcome(
                    &profile,
                    &selected_account.account.account_id,
                    selected_account
                        .account
                        .email
                        .as_deref()
                        .unwrap_or_default(),
                    ThunderbirdAccountState::Failed,
                    Some("An account with this email address is already connected.".into()),
                ));
                continue;
            }
            let account_prefetch =
                prefetched
                    .remove(&selected_account.app_id)
                    .unwrap_or_else(|| {
                        Err("Thunderbird account discovery did not return a result.".into())
                    });
            match self.add_account(
                &remote_accounts,
                &selected_account,
                &profile.path,
                command_sender.clone(),
                account_prefetch,
            ) {
                Ok((ready, binding, (folder_id, initial_page))) => {
                    if occupied_emails.contains(&normalize_email(&binding.email))
                        || self.accounts.iter().any(|(existing_id, existing)| {
                            *existing_id != binding.app_id
                                && normalize_email(&existing.email)
                                    == normalize_email(&binding.email)
                        })
                    {
                        outcomes.push(outcome(
                            &profile,
                            &selected_account.account.account_id,
                            &binding.email,
                            ThunderbirdAccountState::Failed,
                            Some("An account with this email address is already connected.".into()),
                        ));
                        continue;
                    }
                    outcomes.push(outcome(
                        &profile,
                        &selected_account.account.account_id,
                        &binding.email,
                        ThunderbirdAccountState::Connected,
                        None,
                    ));
                    self.initial_pages
                        .insert((binding.app_id, folder_id), initial_page);
                    self.retire_account_page(binding.app_id);
                    let account_id = binding.app_id;
                    self.accounts.insert(account_id, binding);
                    self.emit(
                        account_id,
                        WorkerMailboxEvent::AdapterSession {
                            session: self.page_session,
                        },
                    );
                    started.push(ready);
                }
                Err((kind, message)) => outcomes.push(outcome(
                    &profile,
                    &selected_account.account.account_id,
                    selected_account
                        .account
                        .email
                        .as_deref()
                        .unwrap_or_default(),
                    kind,
                    Some(message),
                )),
            }
        }
        let has_broken_prefetch = broken_prefetch.is_some();
        if let Some((failed_account_id, error)) = broken_prefetch {
            let reason = friendly_error(&error);
            suppress_started_accounts_on_broken_prefetch(&mut started, &mut outcomes, &reason);
            self.emit(
                failed_account_id,
                WorkerMailboxEvent::Error {
                    text: reason,
                    connectivity: true,
                },
            );
            self.retire_disconnected_runtime(failed_account_id);
        }
        if should_retire_after_add(started.len(), self.accounts.len(), has_broken_prefetch) {
            self.retire_requested = true;
        }
        Ok((started, outcomes))
    }

    fn add_account(
        &mut self,
        remote_accounts: &[Value],
        selected: &SelectedAccount,
        source_path: &Path,
        command_tx: Arc<SyncSender<Command>>,
        prefetched: Result<(Value, Value), String>,
    ) -> Result<(StartedAccount, AccountBinding, (u32, Value)), (ThunderbirdAccountState, String)>
    {
        let source = &selected.account;
        let exact = remote_accounts
            .iter()
            .find(|account| string_id(account.get("id")) == source.account_id);
        let remote = if let Some(account) = exact {
            account
        } else {
            let email = source
                .email
                .as_deref()
                .map(normalize_email)
                .unwrap_or_default();
            let matching: Vec<_> = remote_accounts
                .iter()
                .filter(|account| {
                    identity_values(account)
                        .iter()
                        .any(|identity| normalize_email(&string(identity.get("email"))) == email)
                })
                .collect();
            if email.is_empty() || matching.len() != 1 {
                return Err((ThunderbirdAccountState::Unsupported, "This account could not be matched to Thunderbird's authenticated account list.".into()));
            }
            matching[0]
        };
        let remote_id = remote.get("id").cloned().ok_or_else(|| {
            (
                ThunderbirdAccountState::Unsupported,
                "Thunderbird returned an account without an ID.".into(),
            )
        })?;
        let remote_type = string(remote.get("type")).to_ascii_lowercase();
        if remote_type == "none" || remote_type == "addressbook" || remote_type == "news" {
            return Err((
                ThunderbirdAccountState::Unsupported,
                "This Thunderbird account type does not provide email folders.".into(),
            ));
        }
        let identities = parse_identities(remote);
        let email = source
            .email
            .clone()
            .filter(|email| !email.trim().is_empty())
            .or_else(|| identities.first().map(|identity| identity.email.clone()))
            .unwrap_or_default();
        if email.trim().is_empty() || identities.is_empty() {
            return Err((
                ThunderbirdAccountState::Unsupported,
                "Thunderbird did not provide a usable sending identity for this account.".into(),
            ));
        }
        let primary_identity_id = identities
            .iter()
            .find(|identity| normalize_email(&identity.email) == normalize_email(&email))
            .or_else(|| identities.first())
            .map(|identity| identity.id.clone())
            .unwrap_or_default();
        let (folders_value, initial_page) =
            prefetched.map_err(|error| (classify_runtime_error(&error), friendly_error(&error)))?;
        let folders = parse_folders(selected.app_id, &folders_value)
            .map_err(|error| (ThunderbirdAccountState::Failed, error))?;
        if folders.is_empty() {
            return Err((
                ThunderbirdAccountState::Unsupported,
                "Thunderbird returned no mail folders for this account.".into(),
            ));
        }
        let initial_folder = folders
            .iter()
            .find(|folder| folder.kind == FolderKind::Inbox)
            .or_else(|| folders.first())
            .expect("non-empty folder list checked");
        let initial_folder_id = initial_folder.id;
        if initial_page
            .get("messages")
            .and_then(Value::as_array)
            .is_none_or(|messages| messages.len() > PAGE_LIMIT)
        {
            return Err((
                ThunderbirdAccountState::Failed,
                "Thunderbird did not return a valid first page from this account.".into(),
            ));
        }

        let account = Account {
            id: selected.app_id,
            name: if source.name.trim().is_empty() {
                string(remote.get("name"))
            } else {
                source.name.clone()
            },
            email: email.clone(),
            label: email.clone(),
            accent: default_accent(selected.app_id).to_owned(),
        };
        let binding = AccountBinding {
            app_id: selected.app_id,
            remote_id,
            email,
            identities: identities.clone(),
            primary_identity_id,
            folders: folders
                .iter()
                .cloned()
                .map(|folder| FolderBinding {
                    remote_id: Value::String(folder.path.clone()),
                    folder,
                })
                .collect(),
        };
        let app_id = selected.app_id;
        let command_sender = command_tx.clone();
        let event_sender = self.event_sender.clone();
        let request_sender: RequestSender = Arc::new(move |request| {
            match command_sender.try_send(Command::Request {
                account_id: app_id,
                request,
            }) {
                Ok(()) => true,
                Err(TrySendError::Full(_)) => false,
                Err(TrySendError::Disconnected(_)) => {
                    let _ = event_sender.unbounded_send(MailboxEvent::Worker(WorkerEnvelope {
                        account_id: app_id,
                        event: WorkerMailboxEvent::Error {
                            text: "The Thunderbird worker session stopped.".into(),
                            connectivity: true,
                        },
                    }));
                    false
                }
            }
        });
        let page_sender = page_sender(command_tx, app_id);
        let ready = StartedAccount {
            source: SavedIdentity {
                path: source_path.to_owned(),
                source_id: source.account_id.clone(),
            },
            account,
            folders,
            request_sender,
            page_sender,
            identities,
            error: None,
        };
        Ok((ready, binding, (initial_folder_id, initial_page)))
    }

    fn publish_page(&mut self, mut result: PageResult) {
        if let PageStatus::Failed(error) = &result.status {
            // Classify the original bridge tag before adapting UI prose.
            if is_broken_bridge_error(error) {
                self.emit(
                    result.account_id,
                    WorkerMailboxEvent::Error {
                        text: friendly_error(error),
                        connectivity: true,
                    },
                );
                self.retire_disconnected_runtime(result.account_id);
            }
            result.status = PageStatus::Failed(friendly_error(error));
        }
        let _ = self.event_sender.unbounded_send(MailboxEvent::Page(result));
    }

    fn retire_page(&mut self, status: PageStatus) {
        let result = self.page_flight.as_mut().and_then(|flight| {
            if flight.retired {
                return None;
            }
            flight.retired = true;
            Some(flight.job.result(status))
        });
        if let Some(result) = result {
            let _ = self.event_sender.unbounded_send(MailboxEvent::Page(result));
        }
        // Do not clear the flight gate until the worker returns. Logical
        // cancellation must never admit another concurrent remote list call.
    }

    fn retire_account_page(&mut self, account_id: u32) {
        if self
            .page_flight
            .as_ref()
            .is_some_and(|flight| flight.job.request.account_id == account_id)
        {
            self.retire_page(PageStatus::Superseded);
        }
    }

    fn handle_page(&mut self, request: PageRequest) {
        let (job, preloaded) = match self.prepare_page(request) {
            Ok(prepared) => prepared,
            Err(result) => {
                self.publish_page(result);
                return;
            }
        };
        if let Some(value) = preloaded {
            let result = bounded_page_value(value)
                .map(|value| self.project_page(&job, value))
                .unwrap_or_else(|error| job.result(PageStatus::Failed(error)));
            self.publish_page(result);
            return;
        }
        #[cfg(test)]
        if self.serial_page_control {
            // Controlled negative control: the original actor await, using the
            // same real dispatcher and consumer as the passing regression.
            let result = self
                .runtime
                .call("list", job.params.clone())
                .map(|value| self.project_page(&job, value))
                .unwrap_or_else(|error| job.result(PageStatus::Failed(error)));
            self.publish_page(result);
            return;
        }
        if self.page_lane.is_none() {
            match PageLane::start(self.runtime.clone()) {
                Ok(lane) => self.page_lane = Some(lane),
                Err(error) => {
                    self.publish_page(job.result(PageStatus::Failed(format!(
                        "Could not start Thunderbird page reader: {error}"
                    ))));
                    return;
                }
            }
        }
        let submitted = self
            .page_lane
            .as_ref()
            .and_then(|lane| lane.sender.as_ref())
            .map(|sender| sender.try_send(job.clone()));
        match submitted {
            Some(Ok(())) => {
                self.page_flight = Some(PageFlight {
                    job,
                    retired: false,
                });
            }
            _ => self.publish_page(job.result(PageStatus::Failed(
                "Thunderbird page reader is unavailable. Retry the folder.".into(),
            ))),
        }
    }

    fn prepare_page(
        &mut self,
        request: PageRequest,
    ) -> Result<(PageJob, Option<Value>), PageResult> {
        let account_id = request.account_id;
        let path = request.folder_path.clone();
        let folder_id = request.folder_id;
        let failed = |status| PageResult {
            next_sorted: None,
            account_id,
            folder_path: path.clone(),
            folder_id,
            generation: request.generation,
            rows: Vec::new(),
            next_cursor: None,
            status,
        };
        if self.retire_requested {
            return Err(failed(PageStatus::Failed(
                "[megamail-tb:disconnected] Thunderbird session stopped.".into(),
            )));
        }
        if request.limit > PAGE_LIMIT || request.limit == 0 {
            return Err(failed(PageStatus::Failed(format!(
                "Thunderbird pages are limited to {PAGE_LIMIT} messages."
            ))));
        }
        let Some(account) = self.accounts.get(&account_id) else {
            return Err(failed(PageStatus::Failed(
                "This Thunderbird account is no longer connected.".into(),
            )));
        };
        let Some(folder) = account
            .folders
            .iter()
            .find(|folder| folder.folder.id == folder_id && folder.folder.path == path)
        else {
            return Err(failed(PageStatus::Failed(
                "This Thunderbird folder is no longer available.".into(),
            )));
        };
        let account_remote = account.remote_id.clone();
        let folder_remote = folder.remote_id.clone();
        let cursor_key = request
            .cursor
            .map(|cursor| (account_id, folder_id, cursor.before_uid));
        let remote_cursor = match cursor_key.and_then(|key| self.cursors.get(&key).cloned()) {
            Some(cursor) => Some(cursor),
            None if cursor_key.is_some() => return Err(failed(PageStatus::Superseded)),
            None => None,
        };
        // A newer request retires the old scope immediately, but retains its
        // physical flight gate. No "empty ready" on cancellation or overload.
        self.retire_account_page(account_id);
        self.active_folder_by_account
            .insert(account_id, (folder_id, path.clone()));
        let use_preloaded = request.cursor.is_none()
            && request.limit == PAGE_LIMIT
            && self.initial_pages.contains_key(&(account_id, folder_id));
        if self.page_flight.is_some() && !use_preloaded {
            return Err(failed(PageStatus::Failed(
                "[megamail-tb:busy] One Thunderbird page read is still running. Retry shortly."
                    .into(),
            )));
        }
        let ticket = self.next_page_ticket;
        let Some(next_ticket) = ticket.checked_add(1) else {
            return Err(failed(PageStatus::Failed(
                "Thunderbird page request IDs are exhausted. Reconnect this profile.".into(),
            )));
        };
        let mut params = json!({ "folderId": folder_remote, "limit": request.limit });
        if let Some(cursor) = remote_cursor {
            params["cursor"] = Value::String(cursor);
        }
        // Parsed header predicates run on the loaded page in LiveMailbox. Do
        // not send operator text to Thunderbird as a literal full-text phrase.
        preflight_request("list", &params)
            .map_err(|(error, _)| failed(PageStatus::Failed(error)))?;
        let preloaded = if use_preloaded {
            self.initial_pages.remove(&(account_id, folder_id))
        } else {
            None
        };
        self.next_page_ticket = next_ticket;
        if request.cursor.is_none() {
            self.cursors
                .retain(|(id, f_id, _), _| *id != account_id || *f_id != folder_id);
        }
        Ok((
            PageJob {
                request,
                source: self.profile.path.clone(),
                session: self.page_session,
                ticket,
                account_remote,
                folder_remote,
                params,
            },
            preloaded,
        ))
    }

    fn page_scope_current(&self, job: &PageJob) -> bool {
        !self.retire_requested
            && job.session == self.page_session
            && job.source == self.profile.path
            && self.active_folder_by_account.get(&job.request.account_id)
                == Some(&(job.request.folder_id, job.request.folder_path.clone()))
            && self
                .accounts
                .get(&job.request.account_id)
                .is_some_and(|account| {
                    account.remote_id == job.account_remote
                        && account.folders.iter().any(|folder| {
                            folder.folder.id == job.request.folder_id
                                && folder.folder.path == job.request.folder_path
                                && folder.remote_id == job.folder_remote
                        })
                })
    }

    fn poll_page(&mut self) {
        if self.page_flight.is_none() {
            return;
        }
        let reply = self
            .page_lane
            .as_ref()
            .and_then(|lane| lane.replies.as_ref())
            .map(|replies| replies.try_recv());
        let completion = match reply {
            Some(Ok(completion)) => completion,
            Some(Err(mpsc::TryRecvError::Empty)) => return,
            _ => {
                if let Some(flight) = self.page_flight.take() {
                    if !flight.retired {
                        self.publish_page(flight.job.result(PageStatus::Failed(
                            "Thunderbird page reader stopped unexpectedly.".into(),
                        )));
                    }
                }
                self.page_lane.take();
                return;
            }
        };
        let Some(flight) = self.page_flight.take() else {
            return;
        };
        // A retired failure can still prove the shared transport is broken.
        if flight.retired {
            if let Err(error) = &completion.result {
                self.report_broken_bridge_error(completion.job.request.account_id, error);
            }
            return;
        }
        if completion.job.ticket != flight.job.ticket || !self.page_scope_current(&completion.job) {
            self.publish_page(flight.job.result(PageStatus::Superseded));
            return;
        }
        let result = match completion.result {
            Ok(value) => self.project_page(&completion.job, value),
            Err(error) => completion.job.result(PageStatus::Failed(error)),
        };
        self.publish_page(result);
    }

    fn project_page(&mut self, job: &PageJob, value: Value) -> PageResult {
        if !self.page_scope_current(job) {
            return job.result(PageStatus::Superseded);
        }
        let request = &job.request;
        let account_id = request.account_id;
        let path = &request.folder_path;
        let folder_id = request.folder_id;
        let failed = |error| job.result(PageStatus::Failed(error));
        let Some(headers) = value.get("messages").and_then(Value::as_array) else {
            return failed("Thunderbird returned an invalid message page.".into());
        };
        if headers.len() > request.limit {
            return failed(
                "Thunderbird returned a message page larger than the requested limit.".into(),
            );
        }
        if let Some(cursor) = request.cursor {
            self.cursors
                .remove(&(account_id, folder_id, cursor.before_uid));
        }
        let mut rows = Vec::with_capacity(headers.len());
        for header in headers {
            match self.map_header(account_id, folder_id, header) {
                Ok(message) => rows.push(message),
                Err(error) => return failed(error),
            }
        }
        if let Some(unread) = folder_unread_count(&value) {
            if let Some(folder) = self.accounts.get_mut(&account_id).and_then(|account| {
                account
                    .folders
                    .iter_mut()
                    .find(|folder| folder.folder.path == *path)
            }) {
                folder.folder.unread = unread;
            }
            self.emit(
                account_id,
                WorkerMailboxEvent::FolderUnreadByPath {
                    path: path.clone(),
                    unread,
                },
            );
        }
        let next_cursor = value
            .get("cursor")
            .and_then(Value::as_str)
            .filter(|cursor| !cursor.is_empty())
            .map(|cursor| {
                let token = self.next_cursor;
                self.next_cursor = self.next_cursor.wrapping_add(1).max(1);
                self.cursors
                    .insert((account_id, folder_id, token), cursor.to_owned());
                megamail_core::cache::MessageCursor { before_uid: token }
            });
        PageResult {
            next_sorted: None,
            rows,
            next_cursor,
            ..job.result(PageStatus::Ready)
        }
    }

    fn map_header(
        &mut self,
        account_id: u32,
        folder_id: u32,
        header: &Value,
    ) -> Result<Message, String> {
        let remote_id = header
            .get("id")
            .ok_or_else(|| "Thunderbird returned a message without an ID.".to_owned())?;
        let external_key = string_id(Some(remote_id));
        let uid = if let Some(uid) = self
            .message_by_remote
            .get(&(account_id, folder_id, external_key.clone()))
            .copied()
        {
            uid
        } else {
            if self.remote_by_uid.len() >= SESSION_MESSAGE_LIMIT {
                return Err("This Thunderbird session reached its 25,000-message safety limit. Restart the account to load more.".into());
            }
            let uid =
                u32::try_from(NEXT_SESSION_UID.fetch_add(1, Ordering::Relaxed)).map_err(|_| {
                    "This Thunderbird session exhausted its message ID space.".to_owned()
                })?;
            self.message_by_remote
                .insert((account_id, folder_id, external_key.clone()), uid);
            self.remote_by_uid
                .insert((account_id, uid), remote_id.clone());
            uid
        };
        self.message_id_by_uid
            .insert((account_id, uid), string(header.get("headerMessageId")));
        let (from_name, from_addr) = parse_author(&string(header.get("author")));
        let timestamp = header
            .get("date")
            .and_then(Value::as_i64)
            .map(|date| {
                if date > 10_000_000_000 {
                    date / 1000
                } else {
                    date
                }
            })
            .unwrap_or(0);
        self.reply_identity_by_uid.insert(
            (account_id, uid),
            reply_identity(&string(header.get("author")), timestamp),
        );
        let date = if timestamp > 0 {
            megamail_core::datefmt::date_time(timestamp)
        } else {
            String::new()
        };
        let recipients = address_list(header.get("recipients"));
        let cc = address_list(header.get("cc"));
        let references = string(header.get("references"));
        Ok(Message {
            id: uid,
            account_id,
            folder_id,
            uid,
            from_name,
            from_addr,
            reply_to: string(header.get("replyTo")),
            to: recipients,
            cc,
            subject: string(header.get("subject")),
            preview: string(header.get("preview")),
            body: String::new(),
            date,
            timestamp,
            unread: !header.get("read").and_then(Value::as_bool).unwrap_or(false),
            starred: header
                .get("flagged")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            keywords: string_array(header.get("tags")),
            has_attachment: header
                .get("hasAttachment")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            message_id: string(header.get("headerMessageId")),
            references,
            importance: Importance::Normal,
            due: 0,
        })
    }

    fn conversation_scope(
        &self,
        account_id: u32,
        seed_ids: &[String],
    ) -> Result<(Value, Vec<Value>, bool), (String, bool)> {
        let account = self.accounts.get(&account_id).ok_or_else(|| {
            (
                "This Thunderbird account is no longer connected.".into(),
                true,
            )
        })?;
        let mut selected = Vec::new();
        let mut ordered_folder_ids = Vec::new();
        let mut partial = false;

        // Put known seed folders first so the bounded account-folder window covers them.
        let mut seed_folders = Vec::new();
        for id in seed_ids {
            if let Some(folder_id) = self.folder_for_header_id(account_id, id) {
                if !seed_folders.contains(&folder_id) {
                    seed_folders.push(folder_id);
                }
            }
        }
        if let Some(folder_id) = seed_folders.first() {
            ordered_folder_ids.push(*folder_id);
        }
        for kind in [FolderKind::Inbox, FolderKind::Sent] {
            for folder in account
                .folders
                .iter()
                .filter(|folder| folder.folder.kind == kind)
            {
                ordered_folder_ids.push(folder.folder.id);
            }
        }
        for folder_id in seed_folders.iter().skip(1) {
            ordered_folder_ids.push(*folder_id);
        }
        let mut extra_folders = account
            .folders
            .iter()
            .filter(|folder| {
                !matches!(
                    folder.folder.kind,
                    FolderKind::Inbox
                        | FolderKind::Sent
                        | FolderKind::Drafts
                        | FolderKind::Templates
                        | FolderKind::Trash
                        | FolderKind::Junk
                ) && !ordered_folder_ids.contains(&folder.folder.id)
            })
            .collect::<Vec<_>>();
        extra_folders.sort_by_key(|folder| folder.folder.kind != FolderKind::Archive);
        ordered_folder_ids.extend(extra_folders.into_iter().map(|folder| folder.folder.id));
        let mut seen = HashSet::new();
        for folder_id in ordered_folder_ids {
            let Some(folder) = account
                .folders
                .iter()
                .find(|folder| folder.folder.id == folder_id)
            else {
                continue;
            };
            let remote = string(Some(&folder.remote_id));
            if remote.is_empty() || !seen.insert(remote.clone()) {
                continue;
            }
            if selected.len() == CONVERSATION_FOLDER_LIMIT {
                partial = true;
                continue;
            }
            selected.push(Value::String(remote));
        }
        Ok((account.remote_id.clone(), selected, partial))
    }

    fn folder_for_header_id(&self, account_id: u32, header_id: &str) -> Option<u32> {
        let wanted = normalize_message_id(header_id);
        if wanted.is_empty() {
            return None;
        }
        let uid = self
            .message_id_by_uid
            .iter()
            .find_map(|((id, uid), message_id)| {
                (*id == account_id && normalize_message_id(message_id) == wanted).then_some(*uid)
            })?;
        self.message_by_remote
            .iter()
            .find_map(|((id, folder_id, _), local_uid)| {
                (*id == account_id && *local_uid == uid).then_some(*folder_id)
            })
    }

    fn map_conversation_headers(
        &mut self,
        account_id: u32,
        value: &Value,
    ) -> Result<Vec<Message>, String> {
        let headers = value
            .get("messages")
            .and_then(Value::as_array)
            .ok_or_else(|| "Thunderbird returned an invalid conversation response.".to_owned())?;
        if headers.len() > CONVERSATION_MEMBER_LIMIT {
            return Err("Thunderbird returned more than 100 conversation messages.".into());
        }
        let mut messages = Vec::with_capacity(headers.len());
        for header in headers {
            let remote_folder = string(header.get("folderId"));
            let folder_id = self
                .accounts
                .get(&account_id)
                .and_then(|account| {
                    account
                        .folders
                        .iter()
                        .find(|folder| {
                            folder.folder.path == remote_folder
                                || string(Some(&folder.remote_id)) == remote_folder
                        })
                        .map(|folder| folder.folder.id)
                })
                .ok_or_else(|| {
                    "Thunderbird returned a conversation message outside the selected account's folders.".to_owned()
                })?;
            let mut normalized_header = header.clone();
            let mut references = string(header.get("references"));
            let in_reply_to = string(header.get("inReplyTo"));
            let in_reply_to_id = normalize_message_id(&in_reply_to);
            if !in_reply_to_id.is_empty()
                && !references
                    .split_whitespace()
                    .any(|id| normalize_message_id(id) == in_reply_to_id)
            {
                if !references.is_empty() {
                    references.push(' ');
                }
                references.push_str(&in_reply_to);
            }
            normalized_header["references"] = Value::String(references);
            let message = self.map_header(account_id, folder_id, &normalized_header)?;
            let kind = self.accounts[&account_id]
                .folders
                .iter()
                .find(|folder| folder.folder.id == folder_id)
                .map(|folder| folder.folder.kind);
            if matches!(
                kind,
                Some(
                    FolderKind::Drafts
                        | FolderKind::Templates
                        | FolderKind::Trash
                        | FolderKind::Junk
                )
            ) {
                continue;
            }
            if kind == Some(FolderKind::Sent) && !self.message_is_from_self(account_id, &message) {
                continue;
            }
            messages.push(message);
        }
        Ok(self.dedupe_conversation_messages(account_id, messages))
    }

    fn message_is_from_self(&self, account_id: u32, message: &Message) -> bool {
        self.accounts.get(&account_id).is_some_and(|account| {
            account.identities.iter().any(|identity| {
                !identity.email.is_empty()
                    && normalize_email(&identity.email) == normalize_email(&message.from_addr)
            })
        })
    }

    fn conversation_copy_priority(&self, account_id: u32, message: &Message) -> u8 {
        let kind = self.accounts.get(&account_id).and_then(|account| {
            account
                .folders
                .iter()
                .find(|folder| folder.folder.id == message.folder_id)
                .map(|folder| folder.folder.kind)
        });
        if kind == Some(FolderKind::Sent) && self.message_is_from_self(account_id, message) {
            2
        } else if kind == Some(FolderKind::Inbox) {
            1
        } else {
            0
        }
    }

    fn dedupe_conversation_messages(
        &self,
        account_id: u32,
        messages: Vec<Message>,
    ) -> Vec<Message> {
        let mut out = Vec::<Message>::new();
        let mut priorities = Vec::<u8>::new();
        let mut by_identity = HashMap::<(String, String, i64), usize>::new();
        for message in messages {
            let header_id = normalize_message_id(&message.message_id);
            if header_id.is_empty() {
                out.push(message);
                priorities.push(0);
                continue;
            }
            let priority = self.conversation_copy_priority(account_id, &message);
            let identity = (
                header_id,
                normalize_email(&message.from_addr),
                message.timestamp,
            );
            if let Some(index) = by_identity.get(&identity).copied() {
                let existing = &out[index];
                let unread = existing.unread || message.unread;
                let starred = existing.starred || message.starred;
                let has_attachment = existing.has_attachment || message.has_attachment;
                let prefer = priority > priorities[index]
                    || (priority == priorities[index]
                        && (message.folder_id, message.uid) < (existing.folder_id, existing.uid));
                if prefer {
                    out[index] = message;
                    priorities[index] = priority;
                }
                out[index].unread = unread;
                out[index].starred = starred;
                out[index].has_attachment = has_attachment;
            } else {
                by_identity.insert(identity, out.len());
                out.push(message);
                priorities.push(priority);
            }
        }
        out
    }

    fn thread_summaries(
        &self,
        account_id: u32,
        groups: &[(String, Vec<String>)],
        messages: Vec<Message>,
    ) -> Vec<(String, ThreadSummary)> {
        let sent_folders = self
            .accounts
            .get(&account_id)
            .map(|account| {
                account
                    .folders
                    .iter()
                    .filter(|folder| folder.folder.kind == FolderKind::Sent)
                    .map(|folder| (account_id, folder.folder.id))
                    .collect::<HashSet<_>>()
            })
            .unwrap_or_default();
        let conversations =
            megamail_core::conversation::group_conversations(messages, &sent_folders);
        groups
            .iter()
            .map(|(tag, ids)| {
                let ids = ids
                    .iter()
                    .take(24)
                    .map(|id| normalize_message_id(id))
                    .filter(|id| !id.is_empty())
                    .collect::<HashSet<_>>();
                if ids.is_empty() {
                    return (tag.clone(), ThreadSummary::default());
                }
                let mut members = conversations
                    .iter()
                    .filter(|conversation| {
                        conversation.members.iter().any(|message| {
                            ids.contains(&normalize_message_id(&message.message_id))
                                || message
                                    .references
                                    .split_whitespace()
                                    .any(|id| ids.contains(&normalize_message_id(id)))
                        })
                    })
                    .flat_map(|conversation| conversation.members.iter().cloned())
                    .collect::<Vec<_>>();
                members = self.dedupe_conversation_messages(account_id, members);
                if members.is_empty() {
                    return (tag.clone(), ThreadSummary::default());
                }
                members.sort_by(|a, b| {
                    a.timestamp
                        .cmp(&b.timestamp)
                        .then_with(|| a.uid.cmp(&b.uid))
                });
                let latest = members.last().map(|message| ThreadLatest {
                    from_name: message.from_name.clone(),
                    from_addr: message.from_addr.clone(),
                    preview: message.preview.clone(),
                    timestamp: message.timestamp,
                    date: message.date.clone(),
                });
                (
                    tag.clone(),
                    ThreadSummary {
                        count: members.len(),
                        latest,
                        members,
                    },
                )
            })
            .collect()
    }

    fn cached_body(&mut self, key: &(u32, String, u32)) -> Option<BodyPayload> {
        self.body_cache.get(key)
    }

    fn cache_body(&mut self, key: (u32, String, u32), payload: BodyPayload) {
        self.body_cache.insert(key, payload);
    }

    fn schedule_body(
        &mut self,
        account_id: u32,
        message_id: u32,
        path: &str,
        uid: u32,
    ) -> Result<(), (String, bool)> {
        let key = (account_id, path.to_owned(), uid);
        if let Some(payload) = self.cached_body(&key) {
            self.emit(account_id, payload.event(message_id, path));
            return Ok(());
        }
        if self.pending_bodies.contains(&key) {
            return Ok(());
        }
        let remote_id = match self.remote_message(account_id, path, message_id, uid) {
            Ok(remote_id) => remote_id,
            Err((text, _)) => {
                self.emit(
                    account_id,
                    WorkerMailboxEvent::BodyFailed {
                        message_id,
                        path: path.to_owned(),
                        text,
                    },
                );
                return Ok(());
            }
        };
        let Some(sender) = self.command_sender.upgrade() else {
            self.emit(
                account_id,
                WorkerMailboxEvent::BodyFailed {
                    message_id,
                    path: path.to_owned(),
                    text: "The Thunderbird session is shutting down.".into(),
                },
            );
            return Ok(());
        };
        let runtime = self.runtime.clone();
        let path = path.to_owned();
        let command_path = path.clone();
        let spawn = thread::Builder::new()
            .name(format!("megamail-thunderbird-body-{account_id}-{uid}"))
            .spawn(move || {
                let result = runtime.call("body", json!({ "messageId": remote_id }));
                let _ = sender.send(Command::BodyFinished {
                    account_id,
                    path: command_path,
                    message_id,
                    uid,
                    result,
                });
            });
        if let Err(error) = spawn {
            self.emit(
                account_id,
                WorkerMailboxEvent::BodyFailed {
                    message_id,
                    path: path.clone(),
                    text: format!("Could not schedule Thunderbird message body: {error}"),
                },
            );
            return Ok(());
        }
        self.pending_bodies.insert((account_id, path, uid));
        Ok(())
    }

    fn handle_body_finished(
        &mut self,
        account_id: u32,
        path: String,
        message_id: u32,
        uid: u32,
        result: Result<Value, String>,
    ) {
        let key = (account_id, path.clone(), uid);
        self.pending_bodies.remove(&key);
        let payload = match result {
            Ok(value) => match Self::parse_body_payload(value) {
                Ok(payload) => payload,
                Err(text) => {
                    self.emit(
                        account_id,
                        WorkerMailboxEvent::BodyFailed {
                            message_id,
                            path,
                            text,
                        },
                    );
                    return;
                }
            },
            Err(error) => {
                let text = friendly_error(&error);
                self.emit(
                    account_id,
                    WorkerMailboxEvent::BodyFailed {
                        message_id,
                        path,
                        text,
                    },
                );
                if is_broken_bridge_error(&error) {
                    self.emit(
                        account_id,
                        WorkerMailboxEvent::Error {
                            text: "The Thunderbird bridge disconnected while loading this message."
                                .into(),
                            connectivity: true,
                        },
                    );
                    self.retire_disconnected_runtime(account_id);
                }
                return;
            }
        };
        self.cache_body(key, payload.clone());
        self.emit(account_id, payload.event(message_id, &path));
    }

    fn parse_body_payload(value: Value) -> Result<BodyPayload, String> {
        let source_html = string(value.get("html"));
        let plain = string(value.get("plainText"));
        if source_html.len() > READER_BODY_BYTES_LIMIT || plain.len() > READER_BODY_BYTES_LIMIT {
            return Err("This message body exceeds the 2 MiB reader limit.".into());
        }
        let original = Arc::new(crate::reader_session::OriginalBody::capture(
            &source_html,
            value
                .get("truncated")
                .and_then(Value::as_bool)
                .map(|truncated| !truncated),
            None,
        ));
        let html = if source_html.trim().is_empty() {
            String::new()
        } else {
            megamail_core::mail_text::reader_html(&source_html)
        };
        let html_without_quote = megamail_core::mail_text::reader_html_without_quote(&html);
        if html.len() > READER_BODY_BYTES_LIMIT
            || html_without_quote
                .as_ref()
                .is_some_and(|html| html.len() > READER_BODY_BYTES_LIMIT)
        {
            return Err("This message body exceeds the 2 MiB reader limit.".into());
        }
        let body = if !plain.is_empty() {
            plain
        } else {
            megamail_core::markdown::plain_text(&source_html)
        };
        if body.len() > READER_BODY_BYTES_LIMIT {
            return Err("This message body exceeds the 2 MiB reader limit.".into());
        }
        let links = if source_html.is_empty() {
            Vec::new()
        } else {
            megamail_core::mail_text::extract_links(&source_html)
        };
        Ok(BodyPayload {
            original,
            body,
            html,
            html_without_quote,
            links,
            has_attachment: value.get("hasAttachment").and_then(Value::as_bool),
            reply_to: value.get("replyTo").map(|_| string(value.get("replyTo"))),
            references: value
                .get("references")
                .map(|_| string(value.get("references"))),
        })
    }

    fn handle_request(&mut self, account_id: u32, request: MailRequest) {
        let result = self.request(account_id, request);
        if let Err((text, connectivity)) = result {
            self.emit(
                account_id,
                WorkerMailboxEvent::Error {
                    text: text.clone(),
                    connectivity,
                },
            );
            if connectivity && is_broken_bridge_error(&text) {
                self.retire_disconnected_runtime(account_id);
            }
        }
    }

    fn schedule_conversation(
        &self,
        account_id: u32,
        params: Value,
        task: ConversationTask,
    ) -> Result<(), (String, bool)> {
        let sender = self
            .command_sender
            .upgrade()
            .ok_or_else(|| ("The Thunderbird session is shutting down.".into(), true))?;
        let runtime = self.runtime.clone();
        thread::Builder::new()
            .name(format!("megamail-thunderbird-conversation-{account_id}"))
            .spawn(move || {
                let result = runtime.call("conversation", params);
                let _ = sender.send(Command::ConversationFinished {
                    account_id,
                    task,
                    result,
                });
            })
            .map_err(|error| {
                (
                    format!("Could not schedule Thunderbird conversation lookup: {error}"),
                    false,
                )
            })?;
        Ok(())
    }

    fn handle_conversation_finished(
        &mut self,
        account_id: u32,
        task: ConversationTask,
        result: Result<Value, String>,
    ) {
        let result = match result {
            Ok(result) => result,
            Err(error) => {
                let connectivity = is_connectivity_error(&error);
                let text = friendly_error(&error);
                self.emit(account_id, WorkerMailboxEvent::Error { text, connectivity });
                if connectivity && is_broken_bridge_error(&error) {
                    self.retire_disconnected_runtime(account_id);
                }
                return;
            }
        };
        let messages = match self.map_conversation_headers(account_id, &result) {
            Ok(messages) => messages,
            Err(error) => {
                self.emit(
                    account_id,
                    WorkerMailboxEvent::Error {
                        text: error,
                        connectivity: false,
                    },
                );
                return;
            }
        };
        match task {
            ConversationTask::Related {
                message_id,
                input_partial,
                scope_partial,
            } => {
                let partial = input_partial
                    || scope_partial
                    || result
                        .get("partial")
                        .and_then(Value::as_bool)
                        .unwrap_or(false);
                let warning = if partial {
                    Some("More messages may exist outside the indexed/query window".into())
                } else {
                    result
                        .get("warning")
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                };
                self.emit(
                    account_id,
                    WorkerMailboxEvent::Related {
                        message_id,
                        messages,
                        partial,
                        warning,
                    },
                );
            }
            ConversationTask::ThreadSummaries {
                groups,
                fallback,
                input_partial,
                scope_partial,
            } => {
                let partial = input_partial
                    || scope_partial
                    || result
                        .get("partial")
                        .and_then(Value::as_bool)
                        .unwrap_or(false);
                let mut summaries = self.thread_summaries(account_id, &groups, messages);
                let returned = summaries
                    .iter()
                    .map(|(tag, _)| tag.clone())
                    .collect::<HashSet<_>>();
                summaries.extend(
                    fallback
                        .into_iter()
                        .filter(|(tag, _)| !returned.contains(tag)),
                );
                let warning = if partial {
                    Some("More messages may exist outside the indexed/query window".into())
                } else {
                    result
                        .get("warning")
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                };
                self.emit(
                    account_id,
                    WorkerMailboxEvent::ThreadSummaries { summaries, warning },
                );
            }
        }
    }

    fn schedule_folder_refresh(
        &mut self,
        account_id: u32,
        folder_id: u32,
        path: &str,
        force: bool,
    ) -> Result<(), (String, bool)> {
        let key = (account_id, folder_id);
        if self.page_flight.as_ref().is_some_and(|flight| {
            flight.job.request.account_id == account_id
                && (flight.job.request.folder_id != folder_id
                    || flight.job.request.folder_path != path)
        }) {
            self.retire_account_page(account_id);
        }
        if self.refresh_inflight.contains(&key) {
            return Ok(());
        }
        if !force
            && self
                .last_refresh_started
                .get(&key)
                .is_some_and(|last| last.elapsed() < THUNDERBIRD_REFRESH_INTERVAL)
        {
            return Ok(());
        }
        let remote_id = self.folder_remote_id(account_id, path)?;
        let sender = self
            .command_sender
            .upgrade()
            .ok_or_else(|| ("The Thunderbird session is shutting down.".into(), true))?;
        let runtime = self.runtime.clone();
        let path = path.to_owned();
        thread::Builder::new()
            .name(format!(
                "megamail-thunderbird-refresh-{account_id}-{folder_id}"
            ))
            .spawn(move || {
                let result = runtime.call("refresh", json!({"folderId": remote_id}));
                let _ = sender.send(Command::FolderRefreshFinished {
                    account_id,
                    folder_id,
                    path,
                    force,
                    result,
                });
            })
            .map_err(|error| {
                (
                    format!("Could not schedule Thunderbird folder refresh: {error}"),
                    false,
                )
            })?;
        self.refresh_inflight.insert(key);
        self.last_refresh_started.insert(key, Instant::now());
        Ok(())
    }

    fn refresh_active_folder(
        &mut self,
        account_id: u32,
        force: bool,
    ) -> Result<(), (String, bool)> {
        let active = self
            .active_folder_by_account
            .get(&account_id)
            .cloned()
            .or_else(|| {
                self.accounts.get(&account_id).and_then(|account| {
                    account
                        .folders
                        .iter()
                        .find(|folder| folder.folder.kind == FolderKind::Inbox)
                        .or_else(|| account.folders.first())
                        .map(|folder| (folder.folder.id, folder.folder.path.clone()))
                })
            });
        if let Some((folder_id, path)) = active {
            self.schedule_folder_refresh(account_id, folder_id, &path, force)?;
        }
        Ok(())
    }

    fn refresh_connected_inboxes(&mut self) {
        let inboxes = self
            .accounts
            .iter()
            .filter_map(|(account_id, account)| {
                account
                    .folders
                    .iter()
                    .find(|folder| folder.folder.kind == FolderKind::Inbox)
                    .map(|folder| (*account_id, folder.folder.id, folder.folder.path.clone()))
            })
            .collect::<Vec<_>>();
        for (account_id, folder_id, path) in inboxes {
            if let Err((text, connectivity)) =
                self.schedule_folder_refresh(account_id, folder_id, &path, false)
            {
                self.emit(account_id, WorkerMailboxEvent::Error { text, connectivity });
            }
        }
    }

    fn handle_folder_refresh_finished(
        &mut self,
        account_id: u32,
        folder_id: u32,
        _path: String,
        force: bool,
        result: Result<Value, String>,
    ) {
        self.refresh_inflight.remove(&(account_id, folder_id));
        match result {
            Ok(result) if result.get("synced").and_then(Value::as_bool) == Some(true) => {
                self.retire_account_page(account_id);
                if let Err(error) = self.schedule_folder_tree_refresh(account_id) {
                    self.emit(
                        account_id,
                        WorkerMailboxEvent::Error {
                            text: error,
                            connectivity: false,
                        },
                    );
                }
                self.emit(account_id, WorkerMailboxEvent::FolderChanged { folder_id });
            }
            Ok(result) if force && result.get("local").and_then(Value::as_bool) == Some(true) => {
                self.emit(
                    account_id,
                    WorkerMailboxEvent::Notice(
                        "Thunderbird showed its local folder cache; remote mail was not refreshed."
                            .into(),
                    ),
                );
            }
            Ok(_) => {}
            Err(error) => {
                let connectivity = is_connectivity_error(&error);
                self.emit(
                    account_id,
                    WorkerMailboxEvent::Error {
                        text: friendly_error(&error),
                        connectivity,
                    },
                );
                if connectivity && is_broken_bridge_error(&error) {
                    self.retire_disconnected_runtime(account_id);
                }
            }
        }
    }

    fn schedule_folder_tree_refresh(&self, account_id: u32) -> Result<(), String> {
        let remote_id = self
            .accounts
            .get(&account_id)
            .map(|account| account.remote_id.clone())
            .ok_or_else(|| "This Thunderbird account is no longer connected.".to_owned())?;
        let sender = self
            .command_sender
            .upgrade()
            .ok_or_else(|| "The Thunderbird session is shutting down.".to_owned())?;
        let runtime = self.runtime.clone();
        thread::Builder::new()
            .name(format!("megamail-thunderbird-folders-{account_id}"))
            .spawn(move || {
                let result = runtime.call("folders", json!({ "accountId": remote_id }));
                let _ = sender.send(Command::FolderTreeFinished { account_id, result });
            })
            .map_err(|error| format!("Could not schedule Thunderbird folder refresh: {error}"))?;
        Ok(())
    }

    fn handle_folder_tree_finished(&mut self, account_id: u32, result: Result<Value, String>) {
        match result {
            Ok(value) => {
                if let Err((error, connectivity)) = self.apply_folder_tree(account_id, value) {
                    self.emit(
                        account_id,
                        WorkerMailboxEvent::Error {
                            text: friendly_error(&error),
                            connectivity,
                        },
                    );
                    if connectivity && is_broken_bridge_error(&error) {
                        self.retire_disconnected_runtime(account_id);
                    }
                }
            }
            Err(error) => {
                let connectivity = is_connectivity_error(&error);
                self.emit(
                    account_id,
                    WorkerMailboxEvent::Error {
                        text: friendly_error(&error),
                        connectivity,
                    },
                );
                if connectivity && is_broken_bridge_error(&error) {
                    self.retire_disconnected_runtime(account_id);
                }
            }
        }
    }

    fn retire_disconnected_runtime(&mut self, failed_account_id: u32) {
        self.retire_page(PageStatus::Failed(
            "[megamail-tb:disconnected] The shared Thunderbird session stopped.".into(),
        ));
        self.retire_requested = true;
        let other_accounts = self
            .accounts
            .keys()
            .copied()
            .filter(|account_id| *account_id != failed_account_id)
            .collect::<Vec<_>>();
        for account_id in other_accounts {
            self.emit(account_id, WorkerMailboxEvent::Error {
                text: "The shared Thunderbird bridge stopped. Reconnect this profile to resume mail.".into(),
                connectivity: true,
            });
        }
    }

    fn report_broken_bridge_error(&mut self, account_id: u32, error: &str) {
        if !is_broken_bridge_error(error) {
            return;
        }
        self.emit(
            account_id,
            WorkerMailboxEvent::Error {
                text: friendly_error(error),
                connectivity: true,
            },
        );
        self.retire_disconnected_runtime(account_id);
    }

    fn request(&mut self, account_id: u32, request: MailRequest) -> Result<(), (String, bool)> {
        // Stateful operations remain on this serial dispatcher. A page that
        // overlaps one cannot later repopulate pre-mutation headers/cursors,
        // even if the mutation's outcome is uncertain.
        if !matches!(
            &request,
            MailRequest::LoadBody { .. }
                | MailRequest::LoadBodies { .. }
                | MailRequest::LoadRelated { .. }
                | MailRequest::LoadThreadSummaries { .. }
                | MailRequest::LoadMessages { .. }
                | MailRequest::SyncFolder { .. }
                | MailRequest::LoadAttachments { .. }
                | MailRequest::Settle { .. }
        ) {
            self.retire_account_page(account_id);
        }
        match request {
            MailRequest::CancelQueued(request) => { self.emit(account_id, WorkerMailboxEvent::QueuedCancelFinished { request, canceled: false }); Ok(()) },
            MailRequest::NativeMutation(request) => { self.native_mutation(account_id, request); Ok(()) },
            MailRequest::LoadMessages { folder_id, path } => {
                self.active_folder_by_account.insert(account_id, (folder_id, path.clone()));
                self.schedule_folder_refresh(account_id, folder_id, &path, false)
            }
            MailRequest::SyncFolder { folder_id, path } => {
                self.active_folder_by_account.insert(account_id, (folder_id, path.clone()));
                self.schedule_folder_refresh(account_id, folder_id, &path, false)
            }
            MailRequest::Settle { .. } => Ok(()),
            MailRequest::RefreshUnread | MailRequest::Reconnect => {
                self.refresh_active_folder(account_id, true)
            }
            MailRequest::LoadBody { message_id, path, uid } => {
                self.schedule_body(account_id, message_id, &path, uid)
            }
            MailRequest::LoadBodies { items, path } => {
                if items.len() > CONVERSATION_MEMBER_LIMIT {
                    return Err(("This conversation has more than 100 messages to load.".into(), false));
                }
                let mut seen = HashSet::new();
                for (message_id, uid) in items {
                    if seen.insert(uid) {
                        self.schedule_body(account_id, message_id, &path, uid)?;
                    }
                }
                Ok(())
            }
            MailRequest::LoadRelated { message_id, ids } => {
                let input_partial = ids.len() > 24;
                let ids = ids.into_iter().take(24).collect::<Vec<_>>();
                let seed_ids = ids.first().cloned().into_iter().collect::<Vec<_>>();
                let (remote_account, folder_ids, scope_partial) = self.conversation_scope(account_id, &seed_ids)?;
                if ids.is_empty() || folder_ids.is_empty() {
                    let partial = input_partial || scope_partial;
                    self.emit(account_id, WorkerMailboxEvent::Related {
                        message_id,
                        messages: Vec::new(),
                        partial,
                        warning: partial.then(|| "More messages may exist outside the indexed/query window".into()),
                    });
                    return Ok(());
                }
                self.schedule_conversation(account_id, json!({
                    "accountId": remote_account,
                    "folderIds": folder_ids,
                    "ids": ids,
                    "includeReplies": true,
                }), ConversationTask::Related { message_id, input_partial, scope_partial })
            }
            MailRequest::LoadThreadSummaries { groups } => {
                let fallback = groups
                    .iter()
                    .map(|(tag, _)| (tag.clone(), ThreadSummary::default()))
                    .collect::<Vec<_>>();
                let mut ids = Vec::new();
                let mut seen_ids = HashSet::new();
                let mut query_groups = Vec::new();
                let mut input_partial = groups.len() > 100;
                for (tag, group_ids) in groups.iter().take(100) {
                    if group_ids.len() > 24 {
                        input_partial = true;
                        continue;
                    }
                    let mut current = Vec::with_capacity(group_ids.len());
                    let mut current_seen = HashSet::new();
                    for id in group_ids {
                        let normalized = normalize_message_id(&id);
                        if normalized.is_empty() || !current_seen.insert(normalized) {
                            continue;
                        }
                        current.push(id.clone());
                    }
                    let additions = current
                        .iter()
                        .filter(|id| !seen_ids.contains(&normalize_message_id(id)))
                        .count();
                    if ids.len() + additions > CONVERSATION_ID_LIMIT {
                        input_partial = true;
                        continue;
                    }
                    for id in &current {
                        let normalized = normalize_message_id(id);
                        if seen_ids.insert(normalized) {
                            ids.push(id.clone());
                        }
                    }
                    if !current.is_empty() {
                        query_groups.push((tag.clone(), current));
                    }
                }
                if ids.is_empty() || query_groups.is_empty() {
                    self.emit(account_id, WorkerMailboxEvent::ThreadSummaries {
                        summaries: fallback,
                        warning: input_partial.then(|| "More messages may exist outside the indexed/query window".into()),
                    });
                    return Ok(());
                }
                let seed_ids = query_groups
                    .iter()
                    .filter_map(|(_, group)| group.first().cloned())
                    .collect::<Vec<_>>();
                let (remote_account, folder_ids, scope_partial) = self.conversation_scope(account_id, &seed_ids)?;
                if folder_ids.is_empty() {
                    self.emit(account_id, WorkerMailboxEvent::ThreadSummaries {
                        summaries: fallback,
                        warning: Some("More messages may exist outside the indexed/query window".into()),
                    });
                    return Ok(());
                }
                self.schedule_conversation(account_id, json!({
                    "accountId": remote_account,
                    "folderIds": folder_ids,
                    "ids": ids,
                    "seedIds": seed_ids,
                    "includeBatchRelated": true,
                }), ConversationTask::ThreadSummaries {
                    groups: query_groups,
                    fallback,
                    input_partial,
                    scope_partial,
                })
            }
            MailRequest::LoadAttachments { message_id, path, uid, download } => {
                if !download {
                    self.emit(account_id, WorkerMailboxEvent::AttachmentsPending { message_id, path });
                    return Ok(());
                }
                let remote_id = self.remote_message(account_id, &path, message_id, uid)?;
                let value = self.call("attachments", json!({ "messageId": remote_id }))?;
                let list = value.as_array().ok_or_else(|| ("Thunderbird returned an invalid attachment list.".into(), false))?;
                if list.len() > ATTACHMENT_LIMIT {
                    return Err((format!("This message has more than {ATTACHMENT_LIMIT} attachments; save them from Thunderbird."), false));
                }
                let mut attachments = Vec::with_capacity(list.len());
                let mut total = 0usize;
                for attachment in list {
                    let part_name = string(attachment.get("partName"));
                    if part_name.is_empty() { return Err(("Thunderbird returned an attachment without a part ID.".into(), false)); }
                    let meta = self.call("attachment_start", json!({ "messageId": remote_id, "partName": part_name }))?;
                    let size = meta
                        .get("size")
                        .and_then(Value::as_u64)
                        .and_then(|size| usize::try_from(size).ok())
                        .unwrap_or(usize::MAX);
                    if size > ATTACHMENT_BYTES_LIMIT.saturating_sub(total) {
                        let transfer_id = string(meta.get("transferId"));
                        let size_limit = "Attachments exceed MegaMail’s 50 MiB reader limit; save them from Thunderbird.";
                        if !transfer_id.is_empty() {
                            if let Err((error, _)) = self.call("download_end", json!({"transferId":transfer_id})) {
                                if is_broken_bridge_error(&error) {
                                    return Err((format!("{size_limit} {}", friendly_error(&error)), true));
                                }
                            }
                        }
                        return Err((size_limit.into(), false));
                    }
                    let transfer_id = string(meta.get("transferId"));
                    let bytes = self.download(&transfer_id, size)?;
                    total += bytes.len();
                    attachments.push(AttachmentSummary {
                        name: safe_name(&string(meta.get("name"))),
                        content: Arc::new(bytes),
                    });
                }
                self.emit(account_id, WorkerMailboxEvent::Attachments { message_id, path, items: attachments, warning: None });
                Ok(())
            }
            MailRequest::ExportRaw { token, path, uid, max_bytes, .. } => {
                let message_id = self.remote_for_path_uid(account_id, &path, uid)?;
                let mut cleanup_error = None;
                let downloaded: Result<Vec<u8>, String> = (|| {
                    let meta = self
                        .call("raw_start", json!({ "messageId": message_id }))
                        .map_err(|(text, _)| text)?;
                    let transfer_id = string(meta.get("transferId"));
                    let size = meta.get("size").and_then(Value::as_u64).unwrap_or(u64::MAX);
                    let cap = max_bytes.unwrap_or(RAW_BYTES_LIMIT as u64).min(RAW_BYTES_LIMIT as u64);
                    if size > cap {
                        if let Err((error, _)) = self.call("download_end", json!({ "transferId": transfer_id })) {
                            if is_broken_bridge_error(&error) {
                                cleanup_error = Some(error);
                            }
                        }
                        return Err(format!("This message is over the {} MiB draft editing limit and was left unchanged.", cap / 1024 / 1024));
                    }
                    self.download(&transfer_id, size as usize)
                        .map_err(|(error, _)| friendly_error(&error))
                })();
                let broken_bridge_error = downloaded
                    .as_ref()
                    .err()
                    .filter(|error| is_broken_bridge_error(error))
                    .cloned()
                    .or(cleanup_error);
                let raw = downloaded.map(Arc::new);
                self.emit(account_id, WorkerMailboxEvent::RawExported { token, raw });
                if let Some(error) = broken_bridge_error {
                    self.report_broken_bridge_error(account_id, &error);
                }
                Ok(())
            }
            MailRequest::SetSeen { path, uid, seen } => self.update_message(account_id, &path, uid, json!({"read":seen})),
            MailRequest::SetFlagged { path, uid, flagged } => self.update_message(account_id, &path, uid, json!({"flagged":flagged})),
            MailRequest::MoveMessage { path, uid, dest } => self.move_message(account_id, &path, uid, &dest),
            MailRequest::MarkSpam { .. } | MailRequest::MarkHam { .. } => Err(("Spam classification is not supported for Thunderbird accounts yet.".into(), false)),
            MailRequest::PurgeMessages { .. } => Err(("Permanent deletion is not supported for Thunderbird accounts yet.".into(), false)),
            MailRequest::MoveMessages { .. } => Err(("Bulk moves are not supported for Thunderbird accounts yet.".into(), false)),
            MailRequest::Send { message, request_id, .. } => {
                let Some(request_id) = request_id else { return Err(("This Thunderbird send did not include a request ID.".into(), false)); };
                let outcome = self.send_message(account_id, &message);
                match outcome {
                    Ok(outcome) => {
                        self.emit(account_id, WorkerMailboxEvent::SendFinished { request_id, outcome });
                        Ok(())
                    }
                    Err(SendFailure::Uncertain(text)) => {
                            self.emit(
                                account_id,
                                WorkerMailboxEvent::SendUnknown {
                                    request_id,
                                    reason: text.clone(),
                                },
                            );
                            self.emit(account_id, WorkerMailboxEvent::Notice(text.clone()));
                            self.report_broken_bridge_error(account_id, &text);
                            Ok(())
                    }
                    Err(SendFailure::Definite(text, _connectivity)) => {
                            self.emit(
                                account_id,
                                WorkerMailboxEvent::SendFinished {
                                    request_id,
                                    outcome: SendOutcome::Failed,
                                },
                            );
                            self.emit(account_id, WorkerMailboxEvent::Notice(text.clone()));
                            self.report_broken_bridge_error(account_id, &text);
                            Ok(())
                    }
                }
            }
            MailRequest::SaveDraft { message, autosave, template, .. } => {
                if template { return Err(("Saving Thunderbird templates is not supported yet.".into(), false)); }
                match self.save_draft(account_id, &message, autosave) {
                    Ok(message_id) => {
                        self.emit(account_id, WorkerMailboxEvent::DraftSaved { autosave, message_id: Some(message_id) });
                        self.refresh_current_folder(account_id, FolderKind::Drafts);
                        Ok(())
                    }
                    Err(error) => Err(error),
                }
            }
            MailRequest::LoadOutbox => Ok(()),
            MailRequest::FlushOutbox { .. } | MailRequest::DeleteOutbox { .. } => Err(("Thunderbird manages its queued messages. Open Thunderbird’s Outbox to retry or discard them.".into(), false)),
            MailRequest::LoadSource { .. } => Err(("Raw source viewing is not supported for Thunderbird accounts.".into(), false)),
            MailRequest::SetKeyword { .. } => Err(("Thunderbird tag editing is not available yet.".into(), false)),
            MailRequest::MarkHamMany { .. } | MailRequest::MarkAllRead { .. } => Err(("This bulk action is not supported for Thunderbird accounts.".into(), false)),
            _ => Err(("This action is not supported for Thunderbird accounts.".into(), false)),
        }
    }

    fn send_message(
        &mut self,
        account_id: u32,
        message: &OutgoingMessage,
    ) -> Result<SendOutcome, SendFailure> {
        if message.from_account_id != account_id {
            return Err(SendFailure::Definite(
                "The message sender does not match the selected Thunderbird account.".into(),
                false,
            ));
        }
        if message.send_at.is_some() {
            return Err(SendFailure::Definite(
                "Scheduled sending is managed by Thunderbird and is not available in MegaMail yet."
                    .into(),
                false,
            ));
        }
        if message.sign
            || message.encrypt
            || message.calendar.is_some()
            || message.outbox_origin.is_some()
        {
            return Err(SendFailure::Definite(
                "This message uses a send feature that Thunderbird adapter does not support."
                    .into(),
                false,
            ));
        }
        let reply_message_id = self.reply_remote_id(account_id, &message.in_reply_to)?;
        let identity_id = self.identity_for_message(account_id, message)?;
        let placeholders = attachment_request_hints(&message.attachments)?;
        let preview = compose_params(
            message,
            &identity_id,
            reply_message_id.as_ref(),
            placeholders,
        );
        preflight_request("send", &preview)?;
        let attachments = self.upload_attachments(&message.attachments)?;
        let params = compose_params(
            message,
            &identity_id,
            reply_message_id.as_ref(),
            attachments,
        );
        let result = self.call("send", params).map_err(|(error, connectivity)| {
            if error.contains("request exceeded the 1 MiB protocol frame limit") {
                return SendFailure::Definite(
                    "This message exceeds Thunderbird's 1 MiB request limit. Shorten the message or reduce its recipients and attachments.".into(),
                    false,
                );
            }
            let _ = connectivity;
            SendFailure::Uncertain(format!(
                "Thunderbird did not confirm this send. Check Sent before retrying. {error}"
            ))
        })?;
        match result.get("mode").and_then(Value::as_str) {
            Some("sendNow") => {
                if let Some(origin) = &message.draft_origin {
                    if !self.remove_draft_origin(origin.account_id, &origin.path, origin.uid) {
                        self.emit(
                            account_id,
                            WorkerMailboxEvent::Notice(
                                "Message sent, but the original draft is still in Drafts.".into(),
                            ),
                        );
                    }
                }
                Ok(SendOutcome::Sent)
            }
            Some("sendLater") => Ok(SendOutcome::Queued),
            _ => Err(SendFailure::Uncertain(
                "Thunderbird’s send result was unclear. Check Sent in Thunderbird before trying again.".into(),
            )),
        }
    }

    fn save_draft(
        &mut self,
        account_id: u32,
        message: &OutgoingMessage,
        autosave: Option<u32>,
    ) -> Result<String, (String, bool)> {
        if message.from_account_id != account_id
            || message.send_at.is_some()
            || message.sign
            || message.encrypt
            || message.calendar.is_some()
        {
            return Err((
                "This draft uses a feature that Thunderbird adapter does not support.".into(),
                false,
            ));
        }
        let reply_message_id = self.reply_remote_id(account_id, &message.in_reply_to)?;
        let identity_id = self.identity_for_message(account_id, message)?;
        let placeholders = attachment_request_hints(&message.attachments)?;
        let preview = compose_params(
            message,
            &identity_id,
            reply_message_id.as_ref(),
            placeholders,
        );
        preflight_request("save", &preview)?;
        let attachments = self.upload_attachments(&message.attachments)?;
        let params = compose_params(
            message,
            &identity_id,
            reply_message_id.as_ref(),
            attachments,
        );
        let result = self.call("save", params).map_err(|(error, connectivity)| {
            (
                format!("Thunderbird did not confirm this draft save. Check Drafts before retrying. {error}"),
                connectivity,
            )
        })?;
        if !result
            .get("saved")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            return Err((
                "Thunderbird did not confirm that the draft was saved.".into(),
                false,
            ));
        }
        if let Some(origin) = &message.draft_origin {
            if !self.remove_draft_origin(origin.account_id, &origin.path, origin.uid) {
                self.emit(
                    account_id,
                    WorkerMailboxEvent::Notice(
                        "Draft saved, but the previous draft copy is still in Drafts.".into(),
                    ),
                );
            }
        }
        let remote = result
            .get("messages")
            .and_then(Value::as_array)
            .and_then(|messages| messages.first());
        let id = remote
            .and_then(|message| message.get("id"))
            .map(|id| string_id(Some(id)))
            .unwrap_or_else(|| format!("draft-{autosave:?}"));
        Ok(id)
    }

    fn identity_for_message(
        &self,
        account_id: u32,
        message: &OutgoingMessage,
    ) -> Result<String, (String, bool)> {
        let account = self.accounts.get(&account_id).ok_or_else(|| {
            (
                "This Thunderbird account is no longer connected.".into(),
                true,
            )
        })?;
        if let Some(alias) = message.from_alias.as_deref() {
            let email = address_from_identity(alias);
            if let Some(identity) = account
                .identities
                .iter()
                .find(|identity| normalize_email(&identity.email) == normalize_email(&email))
            {
                return Ok(identity.id.clone());
            }
            return Err((
                "The selected From address is not a Thunderbird sending identity for this account."
                    .into(),
                false,
            ));
        }
        Ok(account.primary_identity_id.clone())
    }

    fn reply_remote_id(
        &self,
        account_id: u32,
        in_reply_to: &str,
    ) -> Result<Option<Value>, (String, bool)> {
        resolve_reply_remote_id(
            account_id,
            in_reply_to,
            &self.message_id_by_uid,
            &self.reply_identity_by_uid,
            &self.remote_by_uid,
        )
    }

    fn upload_attachments(&mut self, paths: &[String]) -> Result<Vec<Value>, (String, bool)> {
        if paths.len() > ATTACHMENT_LIMIT {
            return Err((
                format!("Choose no more than {ATTACHMENT_LIMIT} attachments."),
                false,
            ));
        }
        let mut total = 0usize;
        let mut attachments = Vec::with_capacity(paths.len());
        for path in paths {
            let meta = fs::symlink_metadata(path)
                .map_err(|error| (format!("Could not read attachment: {error}"), false))?;
            if !meta.file_type().is_file() {
                return Err(("An attachment is no longer a regular file.".into(), false));
            }
            let size = usize::try_from(meta.len()).unwrap_or(usize::MAX);
            total = total.saturating_add(size);
            if total > ATTACHMENT_BYTES_LIMIT {
                return Err((
                    "Attachments exceed MegaMail’s 50 MiB send limit.".into(),
                    false,
                ));
            }
            let bytes = fs::read(path).map_err(|error| {
                (
                    format!(
                        "Could not read {}: {error}",
                        Path::new(path)
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                    ),
                    false,
                )
            })?;
            if bytes.len() != size {
                return Err((
                    "An attachment changed while it was being read.".into(),
                    false,
                ));
            }
            let name = Path::new(path)
                .file_name()
                .and_then(|name| name.to_str())
                .map(safe_name)
                .unwrap_or_else(|| "attachment.bin".into());
            let mime_type = mime_type(&name).to_owned();
            let begun = self.call("upload_begin", json!({ "kind": "attachment", "size": bytes.len(), "name": name, "mimeType": mime_type }))?;
            let transfer_id = string(begun.get("transferId"));
            if transfer_id.is_empty() {
                return Err((
                    "Thunderbird did not create an attachment transfer.".into(),
                    false,
                ));
            }
            let upload = (|| {
                for (index, chunk) in bytes.chunks(TRANSFER_CHUNK).enumerate() {
                    let offset = index * TRANSFER_CHUNK;
                    let encoded = base64::engine::general_purpose::STANDARD.encode(chunk);
                    self.call(
                        "upload_chunk",
                        json!({ "transferId": transfer_id, "offset": offset, "data": encoded }),
                    )?;
                }
                self.call("upload_finish", json!({ "transferId": transfer_id }))?;
                Ok::<_, (String, bool)>(())
            })();
            if let Err(error) = upload {
                let _ = self.call("upload_cancel", json!({ "transferId": transfer_id }));
                return Err(error);
            }
            attachments
                .push(json!({ "transferId": transfer_id, "name": name, "mimeType": mime_type }));
        }
        Ok(attachments)
    }

    fn download(&mut self, transfer_id: &str, size: usize) -> Result<Vec<u8>, (String, bool)> {
        if transfer_id.is_empty() || size > RAW_BYTES_LIMIT {
            return Err(("Thunderbird returned an invalid transfer.".into(), false));
        }
        let transfer_id = transfer_id.to_owned();
        let result = (|| {
            let mut bytes = Vec::with_capacity(size);
            let mut offset = 0usize;
            while offset < size {
                let length = (size - offset).min(TRANSFER_CHUNK);
                let chunk = self.call(
                    "download_chunk",
                    json!({ "transferId": transfer_id, "offset": offset, "length": length }),
                )?;
                if chunk.get("offset").and_then(Value::as_u64) != Some(offset as u64) {
                    return Err((
                        "Thunderbird returned an out-of-order transfer chunk.".into(),
                        false,
                    ));
                }
                let encoded = string(chunk.get("data"));
                let decoded = base64::engine::general_purpose::STANDARD
                    .decode(encoded)
                    .map_err(|_| {
                        (
                            "Thunderbird returned an invalid transfer chunk.".to_owned(),
                            false,
                        )
                    })?;
                if decoded.is_empty() || decoded.len() > length || decoded.len() > size - offset {
                    return Err((
                        "Thunderbird returned an invalid transfer chunk size.".into(),
                        false,
                    ));
                }
                let next_offset = offset + decoded.len();
                let done = chunk.get("done").and_then(Value::as_bool).unwrap_or(false);
                if done != (next_offset == size) {
                    return Err((
                        "Thunderbird returned an inconsistent transfer completion flag.".into(),
                        false,
                    ));
                }
                offset = next_offset;
                bytes.extend_from_slice(&decoded);
            }
            if bytes.len() != size {
                return Err(("Thunderbird returned an incomplete transfer.".into(), false));
            }
            Ok(bytes)
        })();
        if let Err((error, _)) = self.call("download_end", json!({ "transferId": transfer_id })) {
            if is_broken_bridge_error(&error) {
                return Err((error, true));
            }
        }
        result
    }

    fn native_mutation(&mut self, account_id: u32, request: NativeMutationRequest) {
        let prepared = (|| {
            if !request.valid()
                || self.retire_requested
                || request.scope
                    != (NativeMutationScope::Thunderbird {
                        session: self.page_session,
                    })
            {
                return Err(
                    "This mutation belongs to an invalid or retired Thunderbird session."
                        .to_string(),
                );
            }
            let message = self
                .remote_for_path_uid(account_id, &request.path, request.uid)
                .map_err(|e| e.0)?;
            let source = self
                .folder_remote_id(account_id, &request.path)
                .map_err(|e| e.0)?;
            let account = self
                .accounts
                .get(&account_id)
                .ok_or("The owning account is unavailable.")?
                .remote_id
                .clone();
            let mut params =
                json!({"messageId": message, "sourceFolderId": source, "accountId": account});
            match &request.action {
                NativeMutationAction::SetSeen(seen) => {
                    params["read"] = json!(seen);
                }
                NativeMutationAction::SetFlagged(flagged) => {
                    params["flagged"] = json!(flagged);
                }
                NativeMutationAction::Move { destination } => {
                    params["destinationFolderId"] = self
                        .folder_remote_id(account_id, destination)
                        .map_err(|e| e.0)?;
                }
            }
            if let Some(expected) = request.required_current_flag {
                params["requiredCurrentFlag"] = json!(expected);
            }
            Ok::<_, String>(params)
        })();
        let mut broken = None;
        let outcome = match prepared {
            Err(reason) => NativeMutationOutcome::Rejected(reason),
            Ok(params) => match self.call("native_mutation", params) {
                Ok(value) => match value.get("status").and_then(Value::as_str) {
                    Some("succeeded") => NativeMutationOutcome::Succeeded,
                    Some("rejected") => NativeMutationOutcome::Rejected("Thunderbird rejected the message identity or action before changing it. Refresh and review the selection.".into()),
                    _ => NativeMutationOutcome::Uncertain("Thunderbird could not confirm this action. Refresh and review it before retrying.".into()),
                },
                Err((error, _)) => { broken = Some(error); NativeMutationOutcome::Uncertain("The Thunderbird acknowledgment was lost. No automatic retry was made.".into()) },
            },
        };
        if outcome == NativeMutationOutcome::Succeeded {
            if let NativeMutationAction::Move { destination } = &request.action {
                self.forget_message(account_id, &request.path, request.uid);
                self.changed_folder(account_id, destination);
            }
            self.changed_folder(account_id, &request.path);
        }
        self.emit(
            account_id,
            WorkerMailboxEvent::NativeMutationFinished(NativeMutationReceipt { request, outcome }),
        );
        if let Some(error) = broken {
            self.report_broken_bridge_error(account_id, &error);
        }
    }

    fn update_message(
        &mut self,
        account_id: u32,
        path: &str,
        uid: u32,
        fields: Value,
    ) -> Result<(), (String, bool)> {
        let remote_id = self.remote_for_path_uid(account_id, path, uid)?;
        self.call("update", json!({ "messageId": remote_id, "read": fields.get("read"), "flagged": fields.get("flagged") }))?;
        self.changed_folder(account_id, path);
        Ok(())
    }

    fn move_message(
        &mut self,
        account_id: u32,
        path: &str,
        uid: u32,
        destination: &str,
    ) -> Result<(), (String, bool)> {
        let remote_id = self.remote_for_path_uid(account_id, path, uid)?;
        let target = self.folder_remote_id(account_id, destination)?;
        self.call(
            "move",
            json!({ "messageId": remote_id, "folderId": target }),
        )?;
        self.forget_message(account_id, path, uid);
        self.changed_folder(account_id, path);
        self.changed_folder(account_id, destination);
        Ok(())
    }

    fn remove_draft_origin(&mut self, account_id: u32, path: &str, uid: u32) -> bool {
        let Ok(remote_id) = self.remote_for_path_uid(account_id, path, uid) else {
            return false;
        };
        if let Err((error, _)) = self.call("delete", json!({ "messageId": remote_id })) {
            self.report_broken_bridge_error(account_id, &error);
            return false;
        }
        self.forget_message(account_id, path, uid);
        self.changed_folder(account_id, path);
        true
    }

    fn forget_message(&mut self, account_id: u32, path: &str, uid: u32) {
        let folder_id = self.accounts.get(&account_id).and_then(|account| {
            account
                .folders
                .iter()
                .find(|folder| folder.folder.path == path)
                .map(|folder| folder.folder.id)
        });
        if let Some(folder_id) = folder_id {
            self.message_by_remote.retain(|(id, folder, _), local| {
                *id != account_id || *folder != folder_id || *local != uid
            });
        }
        self.remote_by_uid.remove(&(account_id, uid));
        self.message_id_by_uid.remove(&(account_id, uid));
        self.reply_identity_by_uid.remove(&(account_id, uid));
    }

    fn refresh_folders(&mut self, account_id: u32) -> Result<(), (String, bool)> {
        let Some(binding) = self.accounts.get(&account_id) else {
            return Err((
                "This Thunderbird account is no longer connected.".into(),
                true,
            ));
        };
        let remote_id = binding.remote_id.clone();
        let value = self.call("folders", json!({ "accountId": remote_id }))?;
        self.apply_folder_tree(account_id, value)
    }

    fn apply_folder_tree(&mut self, account_id: u32, value: Value) -> Result<(), (String, bool)> {
        let mut folders = parse_folders(account_id, &value).map_err(|error| (error, false))?;
        if let Some(previous) = self.accounts.get(&account_id) {
            for folder in &mut folders {
                if let Some(previous_folder) = previous
                    .folders
                    .iter()
                    .find(|previous| previous.folder.path == folder.path)
                {
                    // Folder trees do not include counts. Keep the last known
                    // count until a fresh selected-folder list supplies one.
                    folder.unread = previous_folder.folder.unread;
                }
            }
        }
        self.retire_account_page(account_id);
        self.accounts
            .get_mut(&account_id)
            .expect("account checked")
            .folders = folders
            .iter()
            .cloned()
            .map(|folder| FolderBinding {
                remote_id: Value::String(folder.path.clone()),
                folder,
            })
            .collect();
        self.emit(
            account_id,
            WorkerMailboxEvent::AdapterSession {
                session: self.page_session,
            },
        );
        self.emit(account_id, WorkerMailboxEvent::Folders(folders));
        Ok(())
    }

    fn refresh_current_folder(&mut self, account_id: u32, kind: FolderKind) {
        if let Some(folder) = self.accounts.get(&account_id).and_then(|account| {
            account
                .folders
                .iter()
                .find(|folder| folder.folder.kind == kind)
        }) {
            self.emit(
                account_id,
                WorkerMailboxEvent::FolderChanged {
                    folder_id: folder.folder.id,
                },
            );
        }
    }

    fn changed_folder(&mut self, account_id: u32, path: &str) {
        if self.page_flight.as_ref().is_some_and(|flight| {
            flight.job.request.account_id == account_id && flight.job.request.folder_path == path
        }) {
            self.retire_account_page(account_id);
        }
        if let Some(folder) = self.accounts.get(&account_id).and_then(|account| {
            account
                .folders
                .iter()
                .find(|folder| folder.folder.path == path)
        }) {
            self.emit(
                account_id,
                WorkerMailboxEvent::FolderChanged {
                    folder_id: folder.folder.id,
                },
            );
        }
    }

    fn remote_for_path_uid(
        &self,
        account_id: u32,
        path: &str,
        uid: u32,
    ) -> Result<Value, (String, bool)> {
        let folder_id = self
            .accounts
            .get(&account_id)
            .and_then(|account| {
                account
                    .folders
                    .iter()
                    .find(|folder| folder.folder.path == path)
                    .map(|folder| folder.folder.id)
            })
            .ok_or_else(|| {
                (
                    "This Thunderbird folder is no longer available.".into(),
                    false,
                )
            })?;
        self.remote_by_uid
            .get(&(account_id, uid))
            .cloned()
            .filter(|_remote| {
                self.message_by_remote.iter().any(|((id, f_id, _), local)| {
                    *id == account_id && *f_id == folder_id && *local == uid
                })
            })
            .ok_or_else(|| {
                (
                    "This Thunderbird message is no longer available; refresh the folder.".into(),
                    false,
                )
            })
    }

    fn remote_message(
        &self,
        account_id: u32,
        path: &str,
        message_id: u32,
        uid: u32,
    ) -> Result<Value, (String, bool)> {
        if message_id != uid {
            return Err(("The Thunderbird message selection is stale.".into(), false));
        }
        self.remote_for_path_uid(account_id, path, uid)
    }

    fn folder_remote_id(&self, account_id: u32, path: &str) -> Result<Value, (String, bool)> {
        self.accounts
            .get(&account_id)
            .and_then(|account| {
                account
                    .folders
                    .iter()
                    .find(|folder| folder.folder.path == path)
            })
            .map(|folder| folder.remote_id.clone())
            .ok_or_else(|| {
                (
                    "This Thunderbird destination folder is not available.".into(),
                    false,
                )
            })
    }

    fn call(&self, method: &str, params: Value) -> Result<Value, (String, bool)> {
        self.runtime
            .call(method, params)
            .map_err(|error| (friendly_error(&error), is_connectivity_error(&error)))
    }

    fn emit(&self, account_id: u32, event: WorkerMailboxEvent) {
        let _ = self
            .event_sender
            .unbounded_send(MailboxEvent::Worker(WorkerEnvelope { account_id, event }));
    }
}

fn page_sender(command_tx: Arc<SyncSender<Command>>, account_id: u32) -> PageSender {
    Arc::new(move |mut request| {
        request.account_id = account_id;
        command_tx
            .try_send(Command::Page(request))
            .map_err(|error| match error {
                TrySendError::Full(_) => SubmitError::Full,
                TrySendError::Disconnected(_) => SubmitError::Closed,
            })
    })
}

fn parse_folders(account_id: u32, value: &Value) -> Result<Vec<Folder>, String> {
    let mut folders = Vec::new();
    let mut remote_ids = HashSet::new();
    flatten_folders(account_id, value, 0, &mut folders, &mut remote_ids)?;
    Ok(folders)
}

fn folder_unread_count(value: &Value) -> Option<u32> {
    value
        .get("unreadMessageCount")
        .and_then(Value::as_u64)
        .map(|count| count.min(u32::MAX as u64) as u32)
}

fn flatten_folders(
    account_id: u32,
    parent: &Value,
    depth: usize,
    output: &mut Vec<Folder>,
    remote_ids: &mut HashSet<String>,
) -> Result<(), String> {
    if depth > 32 {
        return Err("Thunderbird folder hierarchy exceeds the 32-level safety limit.".into());
    }
    let children = parent
        .get("subFolders")
        .or_else(|| parent.get("subfolders"))
        .and_then(Value::as_array);
    if let Some(children) = children {
        for child in children {
            if output.len() >= FOLDER_LIMIT {
                return Err("Thunderbird returned more than 4,096 folders.".into());
            }
            let kind_text = string(child.get("type")).to_ascii_lowercase();
            let name = string(child.get("name"));
            let remote_id = child
                .get("id")
                .map(|value| string(Some(value)))
                .unwrap_or_default();
            if remote_id.is_empty() {
                return Err("Thunderbird returned a folder without an ID.".into());
            }
            if !remote_ids.insert(remote_id.clone()) {
                return Err("Thunderbird returned a duplicate folder ID.".into());
            }
            if kind_text != "account" {
                let id = u32::try_from(output.len() + 1)
                    .map_err(|_| "Too many Thunderbird folders.".to_owned())?;
                output.push(Folder {
                    id,
                    account_id,
                    name: if name.trim().is_empty() {
                        "Mail folder".into()
                    } else {
                        name
                    },
                    path: remote_id,
                    kind: folder_kind(&kind_text, child),
                    unread: child
                        .get("unreadMessageCount")
                        .or_else(|| child.get("unread"))
                        .and_then(Value::as_u64)
                        .unwrap_or(0)
                        .min(u32::MAX as u64) as u32,
                });
            }
            flatten_folders(account_id, child, depth + 1, output, remote_ids)?;
        }
    }
    Ok(())
}

fn folder_kind(kind: &str, folder: &Value) -> FolderKind {
    let name = string(folder.get("name")).to_ascii_lowercase();
    match kind {
        "inbox" => FolderKind::Inbox,
        "sent" => FolderKind::Sent,
        "drafts" | "draft" => FolderKind::Drafts,
        "trash" => FolderKind::Trash,
        "archive" | "archives" => FolderKind::Archive,
        "junk" | "spam" => FolderKind::Junk,
        "templates" => FolderKind::Templates,
        "virtual" if name.contains("starred") || name.contains("flagged") => FolderKind::Starred,
        _ => match name.as_str() {
            "inbox" => FolderKind::Inbox,
            "sent" | "sent mail" | "sent items" => FolderKind::Sent,
            "draft" | "drafts" => FolderKind::Drafts,
            "trash" | "deleted items" | "bin" => FolderKind::Trash,
            "archive" | "archives" => FolderKind::Archive,
            "junk" | "spam" => FolderKind::Junk,
            _ => FolderKind::Custom,
        },
    }
}

fn parse_identities(account: &Value) -> Vec<SenderIdentity> {
    let primary_email = account
        .get("identities")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .map(|identity| normalize_email(&string(identity.get("email"))))
        .unwrap_or_default();
    identity_values(account)
        .into_iter()
        .filter_map(|identity| {
            let id = string_id(identity.get("id"));
            let email = string(identity.get("email"));
            if id.is_empty() || email.trim().is_empty() {
                return None;
            }
            Some(SenderIdentity {
                id,
                email: email.trim().to_owned(),
                name: string(identity.get("name")),
                primary: normalize_email(&email) == primary_email,
            })
        })
        .collect()
}

fn identity_values(account: &Value) -> Vec<&Value> {
    account
        .get("identities")
        .and_then(Value::as_array)
        .map(|identities| identities.iter().collect())
        .unwrap_or_default()
}

fn array_result(value: Value) -> Result<Vec<Value>, String> {
    value
        .as_array()
        .cloned()
        .or_else(|| value.get("accounts").and_then(Value::as_array).cloned())
        .ok_or_else(|| "Thunderbird returned an invalid account list.".into())
}

fn outcome(
    profile: &ThunderbirdProfile,
    source_id: &str,
    email: &str,
    state: ThunderbirdAccountState,
    message: Option<String>,
) -> ThunderbirdAccountOutcome {
    let name = profile
        .accounts
        .iter()
        .find(|account| account.account_id == source_id)
        .map(|account| account.name.clone())
        .unwrap_or_else(|| "Thunderbird account".into());
    ThunderbirdAccountOutcome {
        profile_path: profile.path.clone(),
        source_account_id: source_id.to_owned(),
        name,
        email: email.to_owned(),
        state,
        message,
    }
}

fn outcome_error(results: &[ThunderbirdAccountOutcome]) -> Option<String> {
    let failed: Vec<_> = results
        .iter()
        .filter(|result| result.state != ThunderbirdAccountState::Connected)
        .collect();
    if failed.is_empty() {
        return None;
    }
    let (mut summary, reason_count) = compact_error_summary(
        failed
            .iter()
            .map(|result| result.message.as_deref().unwrap_or("Could not connect.")),
    );
    if reason_count > 3 {
        summary.push_str("; other errors");
    }
    let noun = if failed.len() == 1 {
        "account"
    } else {
        "accounts"
    };
    let verb = if failed.len() == 1 { "needs" } else { "need" };
    Some(format!(
        "{} Thunderbird {noun} {verb} attention: {summary}",
        failed.len()
    ))
}

fn should_retire_after_add(
    started_accounts: usize,
    active_accounts: usize,
    broken_prefetch: bool,
) -> bool {
    broken_prefetch || (started_accounts == 0 && active_accounts == 0)
}

fn suppress_started_accounts_on_broken_prefetch<T>(
    started: &mut Vec<T>,
    outcomes: &mut [ThunderbirdAccountOutcome],
    reason: &str,
) {
    started.clear();
    for result in outcomes
        .iter_mut()
        .filter(|result| result.state == ThunderbirdAccountState::Connected)
    {
        result.state = ThunderbirdAccountState::Failed;
        result.message = Some(reason.to_owned());
    }
}

fn compact_error_summary<'a>(messages: impl IntoIterator<Item = &'a str>) -> (String, usize) {
    let mut reasons: Vec<(String, usize)> = Vec::new();
    for message in messages {
        let reason = message
            .chars()
            .filter(|character| !character.is_control())
            .take(160)
            .collect::<String>();
        if let Some((_, count)) = reasons.iter_mut().find(|(known, _)| known == &reason) {
            *count += 1;
        } else {
            reasons.push((reason, 1));
        }
    }
    let summary = reasons
        .iter()
        .take(3)
        .map(|(reason, count)| {
            if *count > 1 {
                format!("{count}× {reason}")
            } else {
                reason.clone()
            }
        })
        .collect::<Vec<_>>()
        .join("; ");
    (summary, reasons.len())
}

fn classify_runtime_error(error: &str) -> ThunderbirdAccountState {
    if let Some(kind) = classify_bridge_error(error) {
        return match kind {
            BridgeFailureKind::NeedsLogin => ThunderbirdAccountState::NeedsLogin,
            BridgeFailureKind::Unsupported => ThunderbirdAccountState::Unsupported,
            _ => ThunderbirdAccountState::Failed,
        };
    }
    let error = error.to_ascii_lowercase();
    if [
        "login",
        "sign in",
        "authentication",
        "unauthorized",
        "not authenticated",
        "credentials",
        "oauth",
    ]
    .iter()
    .any(|part| error.contains(part))
    {
        ThunderbirdAccountState::NeedsLogin
    } else if [
        "unsupported",
        "not supported",
        "account type",
        "no supported",
    ]
    .iter()
    .any(|part| error.contains(part))
    {
        ThunderbirdAccountState::Unsupported
    } else {
        ThunderbirdAccountState::Failed
    }
}

fn friendly_error(error: &str) -> String {
    if classify_bridge_error(error) == Some(BridgeFailureKind::Unsupported) {
        return "Thunderbird bridge compatibility needs repair. Check the supported Thunderbird version and bridge installation, then Retry.".into();
    }
    match classify_runtime_error(error) {
        ThunderbirdAccountState::NeedsLogin => {
            "Sign in to this Thunderbird profile, then try again.".into()
        }
        ThunderbirdAccountState::Unsupported => {
            "This Thunderbird account type is not available through the mail API.".into()
        }
        _ => {
            let clean: String = error
                .chars()
                .filter(|character| !character.is_control())
                .take(256)
                .collect();
            if clean.is_empty() {
                "Thunderbird could not complete the operation.".into()
            } else {
                clean
            }
        }
    }
}

fn is_connectivity_error(error: &str) -> bool {
    if let Some(kind) = classify_bridge_error(error) {
        return matches!(
            kind,
            BridgeFailureKind::Disconnected
                | BridgeFailureKind::Protocol
                | BridgeFailureKind::RequestTimeout
                | BridgeFailureKind::NeedsLogin
        );
    }
    if is_broken_bridge_error(error) {
        return true;
    }
    let error = error.to_ascii_lowercase();
    [
        "timeout",
        "offline",
        "network",
        "connection",
        "not authenticated",
        "sign in",
        "authentication",
    ]
    .iter()
    .any(|part| error.contains(part))
}

pub(super) fn is_broken_bridge_error(error: &str) -> bool {
    if let Some(kind) = classify_bridge_error(error) {
        return matches!(
            kind,
            BridgeFailureKind::Disconnected | BridgeFailureKind::Protocol
        );
    }
    let error = error.to_ascii_lowercase();
    [
        "broken pipe",
        "pipe closed",
        "socket closed",
        "socket is not connected",
        "bridge is disconnected",
        "bridge disconnected",
        "bridge closed the connection",
        "shared thunderbird bridge stopped",
        "bridge disconnected while",
        "connection reset",
        "connection closed",
        "worker session stopped",
        "native messaging host has exited",
    ]
    .iter()
    .any(|part| error.contains(part))
}

fn string(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(value)) => value.clone(),
        Some(Value::Number(value)) => value.to_string(),
        Some(Value::Bool(value)) => value.to_string(),
        _ => String::new(),
    }
}

fn string_id(value: Option<&Value>) -> String {
    string(value)
}

fn normalize_message_id(value: &str) -> String {
    value
        .trim()
        .trim_start_matches('<')
        .trim_end_matches('>')
        .trim()
        .to_ascii_lowercase()
}

fn reply_identity(author: &str, timestamp: i64) -> (String, i64) {
    let (_, address) = parse_author(author);
    (normalize_email(&address), timestamp)
}

fn resolve_reply_remote_id(
    account_id: u32,
    in_reply_to: &str,
    message_ids: &HashMap<(u32, u32), String>,
    identities: &HashMap<(u32, u32), (String, i64)>,
    remote_ids: &HashMap<(u32, u32), Value>,
) -> Result<Option<Value>, (String, bool)> {
    if in_reply_to.trim().is_empty() {
        return Ok(None);
    }
    let wanted = normalize_message_id(in_reply_to);
    let mut selected_identity: Option<(String, i64)> = None;
    let mut selected_uid: Option<u32> = None;
    for ((candidate_account, uid), message_id) in message_ids {
        if *candidate_account != account_id
            || normalize_message_id(message_id) != wanted
            || !remote_ids.contains_key(&(*candidate_account, *uid))
        {
            continue;
        }
        let Some(identity) = identities.get(&(*candidate_account, *uid)) else {
            return Err((
                "Thunderbird cannot safely choose a reply target because its loaded message identity is incomplete.".into(),
                false,
            ));
        };
        if selected_identity
            .as_ref()
            .is_some_and(|selected| selected != identity)
            || (selected_uid.is_some() && (identity.0.is_empty() || identity.1 <= 0))
        {
            return Err((
                "Thunderbird cannot safely choose a reply target because multiple loaded messages share this Message-ID.".into(),
                false,
            ));
        }
        selected_identity = Some(identity.clone());
        selected_uid = Some(selected_uid.map_or(*uid, |selected| selected.min(*uid)));
    }
    selected_uid
        .and_then(|uid| remote_ids.get(&(account_id, uid)))
        .cloned()
        .map(Some)
        .ok_or_else(|| {
            (
                "The original message is no longer loaded in Thunderbird. Refresh the folder before replying so its thread headers are preserved.".into(),
                false,
            )
        })
}

fn string_array(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|item| string(Some(item)))
                .filter(|item| !item.is_empty())
                .take(128)
                .collect()
        })
        .unwrap_or_default()
}

fn address_list(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(value)) => value.clone(),
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| {
                if let Some(text) = item.as_str() {
                    Some(text.to_owned())
                } else {
                    let email = string(item.get("email"));
                    let name = string(item.get("name"));
                    if email.is_empty() {
                        None
                    } else if name.is_empty() {
                        Some(email)
                    } else {
                        Some(format!("{name} <{email}>"))
                    }
                }
            })
            .take(256)
            .collect::<Vec<_>>()
            .join(", "),
        _ => String::new(),
    }
}

fn compose_recipients(value: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut quote = false;
    let mut angle = false;
    let mut start = 0;
    for (index, character) in value.char_indices() {
        match character {
            '"' => quote = !quote,
            '<' if !quote => angle = true,
            '>' if !quote => angle = false,
            ',' if !quote && !angle => {
                let item = value[start..index].trim();
                if !item.is_empty() {
                    result.push(item.to_owned());
                }
                start = index + 1;
            }
            _ => {}
        }
    }
    let last = value[start..].trim();
    if !last.is_empty() {
        result.push(last.to_owned());
    }
    result
}

fn attachment_request_hints(paths: &[String]) -> Result<Vec<Value>, (String, bool)> {
    if paths.len() > ATTACHMENT_LIMIT {
        return Err((
            format!("Choose no more than {ATTACHMENT_LIMIT} attachments."),
            false,
        ));
    }
    Ok(paths
        .iter()
        .map(|path| {
            let name = Path::new(path)
                .file_name()
                .and_then(|name| name.to_str())
                .map(safe_name)
                .unwrap_or_else(|| "attachment.bin".into());
            json!({
                "transferId": "x".repeat(TRANSFER_ID_FRAME_RESERVE),
                "name": name,
                "mimeType": mime_type(&name),
            })
        })
        .collect())
}

fn compose_params(
    message: &OutgoingMessage,
    identity_id: &str,
    reply_message_id: Option<&Value>,
    attachments: Vec<Value>,
) -> Value {
    let html = !message.html.trim().is_empty();
    let mut params = json!({
        "identityId": identity_id,
        "to": compose_recipients(&message.to),
        "cc": compose_recipients(&message.cc),
        "bcc": compose_recipients(&message.bcc),
        "replyTo": compose_recipients(&message.reply_to).join(", "),
        "subject": message.subject,
        "body": if html { message.html.clone() } else { message.body.clone() },
        "plainTextBody": message.body,
        "isPlainText": !html,
        "attachments": attachments,
    });
    if let Some(remote_id) = reply_message_id {
        params["replyMessageId"] = remote_id.clone();
        params["replyType"] = Value::String(
            if message.cc.trim().is_empty() {
                "replyToSender"
            } else {
                "replyToAll"
            }
            .into(),
        );
    }
    params
}

fn preflight_request(method: &str, params: &Value) -> Result<(), (String, bool)> {
    let request = json!({ "id": u64::MAX, "method": method, "params": params });
    let size = serde_json::to_vec(&request)
        .map_err(|_| {
            (
                "Could not size the Thunderbird message request.".into(),
                false,
            )
        })?
        .len();
    if size > BRIDGE_REQUEST_FRAME_LIMIT {
        return Err((
            "This message exceeds Thunderbird's 1 MiB request limit. Shorten the message or reduce its recipients and attachments.".into(),
            false,
        ));
    }
    Ok(())
}

fn parse_author(author: &str) -> (String, String) {
    let author = author.trim();
    if let Some(open) = author.rfind('<') {
        if let Some(close) = author[open..].find('>') {
            let name = author[..open].trim().trim_matches('"').to_owned();
            let address = author[open + 1..open + close].trim().to_owned();
            return (
                if name.is_empty() {
                    address.clone()
                } else {
                    name
                },
                address,
            );
        }
    }
    (author.to_owned(), author.to_owned())
}

fn address_from_identity(identity: &str) -> String {
    parse_author(identity).1
}

fn normalize_email(email: &str) -> String {
    email.trim().to_ascii_lowercase()
}

fn safe_name(name: &str) -> String {
    let basename = name.rsplit(['/', '\\']).next().unwrap_or_default();
    let value: String = basename
        .chars()
        .filter(|character| !character.is_control())
        .take(255)
        .collect();
    if value.trim().is_empty() {
        "attachment.bin".into()
    } else {
        value
    }
}

fn mime_type(name: &str) -> &'static str {
    match Path::new(name)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "txt" => "text/plain",
        "html" | "htm" => "text/html",
        "pdf" => "application/pdf",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "json" => "application/json",
        "csv" => "text/csv",
        "eml" => "message/rfc822",
        _ => "application/octet-stream",
    }
}

fn default_accent(account_id: u32) -> &'static str {
    const ACCENTS: [&str; 6] = [
        "#7c86ff", "#58b6a9", "#df9a48", "#d9728b", "#8d79d6", "#54a3cf",
    ];
    ACCENTS[account_id.saturating_sub(1) as usize % ACCENTS.len()]
}

#[cfg(test)]
mod bridge_classification_tests {
    use super::*;
    #[test]
    fn tagged_timeout_busy_and_operation_never_retire_or_infer_login_from_prose() {
        for tag in ["request-timeout", "busy", "operation"] {
            let error = format!(
                "[megamail-tb:{tag}] credentials authentication socket closed session stopped"
            );
            assert!(!is_broken_bridge_error(&error), "{tag}");
            assert_eq!(
                classify_runtime_error(&error),
                ThunderbirdAccountState::Failed,
                "{tag}"
            );
        }
        assert!(!is_connectivity_error("[megamail-tb:busy] socket closed"));
        assert!(!is_connectivity_error(
            "[megamail-tb:operation] authentication"
        ));
        assert!(is_connectivity_error(
            "[megamail-tb:request-timeout] request exceeded deadline"
        ));
    }
    #[test]
    fn tagged_disconnect_protocol_and_explicit_login_have_distinct_recovery() {
        for tag in ["disconnected", "protocol"] {
            assert!(is_broken_bridge_error(&format!(
                "[megamail-tb:{tag}] unrelated description"
            )));
        }
        assert_eq!(
            classify_runtime_error("[megamail-tb:needs-login] explicit evidence"),
            ThunderbirdAccountState::NeedsLogin
        );
        assert_eq!(
            classify_runtime_error("[megamail-tb:unsupported] compatibility"),
            ThunderbirdAccountState::Unsupported
        );
        assert!(friendly_error("[megamail-tb:unsupported] old version").contains("compatibility"));
    }
}

#[cfg(test)]
mod page_lane_tests {
    use super::*;

    // No filesystem, profile, socket, process, provider, or cache database.
    // Only the QueryClient boundary is fake; run/handle_request/BodyFinished
    // and the public mailbox event consumer below are production paths.
    struct Fixture {
        sender: Option<Arc<SyncSender<Command>>>,
        events: gpui_mpsc::UnboundedReceiver<MailboxEvent>,
        entered: mpsc::Receiver<()>,
        release: Arc<(Mutex<bool>, std::sync::Condvar)>,
        join: Option<thread::JoinHandle<ActorState>>,
        calls: Arc<Mutex<Vec<(String, Value)>>>,
        list_reply: Arc<Mutex<Result<Value, String>>>,
        panic_next: Arc<std::sync::atomic::AtomicBool>,
        selected_uid: u32,
    }

    impl Fixture {
        fn start() -> Self {
            Self::start_with(true, false)
        }

        fn start_with(warm: bool, serial_page_control: bool) -> Self {
            let (command_tx, command_rx) = mpsc::sync_channel(COMMAND_QUEUE);
            let sender = Arc::new(command_tx);
            let (event_sender, events) = gpui_mpsc::unbounded();
            let (entered_tx, entered) = mpsc::sync_channel(1);
            let release = Arc::new((Mutex::new(false), std::sync::Condvar::new()));
            let gate = release.clone();
            let calls = Arc::new(Mutex::new(Vec::new()));
            let query_calls = calls.clone();
            let list_reply = Arc::new(Mutex::new(Ok(json!({
                "messages":[{"id":43,"subject":"Late fictional page"}],
                "cursor":"fictional-next", "unreadMessageCount":7
            }))));
            let reply = list_reply.clone();
            let panic_next = Arc::new(std::sync::atomic::AtomicBool::new(false));
            let query_panic = panic_next.clone();
            let runtime = QueryClient::Fixture(Arc::new(move |method, params| {
                query_calls
                    .lock()
                    .unwrap()
                    .push((method.to_owned(), params));
                match method {
                    "list" => {
                        entered_tx.send(()).unwrap();
                        let (lock, wake) = &*gate;
                        let mut released = lock.lock().unwrap();
                        while !*released {
                            released = wake.wait(released).unwrap();
                        }
                        drop(released);
                        if query_panic.swap(false, Ordering::AcqRel) {
                            panic!("fictional query panic");
                        }
                        reply.lock().unwrap().clone()
                    }
                    "body" => Ok(json!({"plainText":"Exact selected body",
                                        "html":"<p>Exact selected body</p>"})),
                    _ => Ok(json!({})),
                }
            }));
            let folders = parse_folders(
                1,
                &json!({"subFolders":[
                    {"id":"fiction://INBOX","name":"Inbox","type":"inbox"},
                    {"id":"fiction://Archive","name":"Archive","type":"folder"}
                ]}),
            )
            .unwrap();
            let mut state = ActorState {
                runtime,
                page_lane: None,
                page_flight: None,
                page_session: NEXT_PAGE_SESSION.fetch_add(1, Ordering::Relaxed),
                next_page_ticket: 1,
                serial_page_control,
                profile: ThunderbirdProfile {
                    name: "fictional".into(),
                    path: PathBuf::from("/fictional/not-opened"),
                    accounts: Vec::new(),
                },
                remote_accounts: Vec::new(),
                command_sender: Arc::downgrade(&sender),
                accounts: HashMap::new(),
                message_by_remote: HashMap::new(),
                remote_by_uid: HashMap::new(),
                message_id_by_uid: HashMap::new(),
                reply_identity_by_uid: HashMap::new(),
                pending_bodies: HashSet::new(),
                body_cache: BodyCache::default(),
                active_folder_by_account: HashMap::new(),
                refresh_inflight: HashSet::new(),
                last_refresh_started: HashMap::new(),
                retire_requested: false,
                cursors: HashMap::new(),
                initial_pages: HashMap::new(),
                next_cursor: 1,
                event_sender,
            };
            let inbox = folders[0].id;
            state.accounts.insert(
                1,
                AccountBinding {
                    app_id: 1,
                    remote_id: json!("fiction-account"),
                    email: "reader@example.invalid".into(),
                    identities: Vec::new(),
                    primary_identity_id: String::new(),
                    folders: folders
                        .into_iter()
                        .map(|folder| FolderBinding {
                            remote_id: json!(folder.path),
                            folder,
                        })
                        .collect(),
                },
            );
            let message = state
                .map_header(1, inbox, &json!({"id":42,"subject":"Selected"}))
                .unwrap();
            if warm {
                state.body_cache.insert(
                    (1, "fiction://INBOX".into(), message.uid),
                    ActorState::parse_body_payload(json!({"plainText":"Exact selected body",
                        "html":"<p>Exact selected body</p>"}))
                    .unwrap(),
                );
            }
            let join = thread::spawn(move || {
                state.run(command_rx);
                state
            });
            let fixture = Self {
                sender: Some(sender),
                events,
                entered,
                release,
                join: Some(join),
                calls,
                list_reply,
                panic_next,
                selected_uid: message.uid,
            };
            fixture.send(Command::Page(PageRequest {
                order: Default::default(),
                after_sorted: None,
                account_id: 1,
                folder_path: "fiction://INBOX".into(),
                folder_id: inbox,
                query: String::new(),
                generation: 11,
                cursor: None,
                limit: PAGE_LIMIT,
            }));
            fixture
                .entered
                .recv_timeout(Duration::from_secs(2))
                .unwrap();
            fixture.send(Command::Request {
                account_id: 1,
                request: MailRequest::LoadBody {
                    message_id: message.id,
                    path: "fiction://INBOX".into(),
                    uid: message.uid,
                },
            });
            fixture
        }

        fn send(&self, command: Command) {
            self.sender.as_ref().unwrap().send(command).unwrap();
        }

        fn next_event(&mut self, timeout: Duration) -> Option<MailboxEvent> {
            let deadline = Instant::now() + timeout;
            loop {
                match self.events.try_next() {
                    Ok(Some(event)) => return Some(event),
                    Ok(None) => return None,
                    Err(_) if Instant::now() < deadline => thread::sleep(Duration::from_millis(1)),
                    Err(_) => return None,
                }
            }
        }

        fn probe(&self) -> (bool, bool, usize) {
            let (tx, rx) = mpsc::sync_channel(1);
            self.send(Command::PageProbe(tx));
            rx.recv_timeout(Duration::from_secs(2)).unwrap()
        }

        fn wait_idle(&self) {
            let deadline = Instant::now() + Duration::from_secs(2);
            while self.probe().0 {
                assert!(Instant::now() < deadline, "page gate did not reset");
                thread::sleep(Duration::from_millis(20));
            }
        }

        fn finish(mut self) -> ActorState {
            self.release_page();
            self.sender.take();
            self.join.take().unwrap().join().unwrap()
        }

        fn release_page(&self) {
            let (lock, wake) = &*self.release;
            *lock.lock().unwrap() = true;
            wake.notify_all();
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            self.release_page();
            self.sender.take();
            if let Some(join) = self.join.take() {
                let _ = join.join();
            }
        }
    }

    #[test]
    fn slow_actual_page_does_not_hold_queued_selected_body_event() {
        let mut fixture = Fixture::start();
        let event = fixture.next_event(Duration::from_millis(250));
        // Assertion occurs before release; Drop releases even on baseline failure.
        assert!(
            matches!(event, Some(MailboxEvent::Worker(WorkerEnvelope {
            account_id:1, event:WorkerMailboxEvent::Body { ref body, .. }
        })) if body == "Exact selected body"),
            "selected body must arrive before page release"
        );
    }

    fn page(folder: &str, folder_id: u32, generation: u64) -> Command {
        Command::Page(PageRequest {
            order: Default::default(),
            after_sorted: None,
            account_id: 1,
            folder_path: folder.into(),
            folder_id,
            query: String::new(),
            generation,
            cursor: None,
            limit: PAGE_LIMIT,
        })
    }

    fn selected_body(fixture: &mut Fixture) {
        assert!(matches!(fixture.next_event(Duration::from_millis(250)),
            Some(MailboxEvent::Worker(WorkerEnvelope { account_id:1,
                event: WorkerMailboxEvent::Body { ref body, original:Some(_), .. }
            })) if body == "Exact selected body"));
    }

    fn next_page(fixture: &mut Fixture) -> PageResult {
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            if let Some(MailboxEvent::Page(page)) = fixture.next_event(Duration::from_millis(50)) {
                return page;
            }
        }
        panic!("missing page terminal event");
    }

    #[test]
    fn cold_body_finished_is_consumed_before_slow_page_release() {
        let mut fixture = Fixture::start_with(false, false);
        selected_body(&mut fixture);
        assert_eq!(
            fixture
                .calls
                .lock()
                .unwrap()
                .iter()
                .filter(|(method, _)| method == "body")
                .count(),
            1
        );
    }

    #[test]
    fn original_serial_actor_await_is_a_negative_control_for_body_event() {
        let mut fixture = Fixture::start_with(true, true);
        assert!(
            fixture.next_event(Duration::from_millis(250)).is_none(),
            "original synchronous actor must hold the queued body"
        );
        fixture.release_page();
        assert_eq!(next_page(&mut fixture).status, PageStatus::Ready);
        selected_body(&mut fixture);
    }

    #[test]
    fn late_page_after_new_folder_is_not_mapped_or_relabelled() {
        let mut fixture = Fixture::start();
        selected_body(&mut fixture);
        fixture.send(page("fiction://Archive", 2, 12));
        let retired = next_page(&mut fixture);
        assert_eq!(
            (
                retired.folder_path.as_str(),
                retired.generation,
                retired.status
            ),
            ("fiction://INBOX", 11, PageStatus::Superseded)
        );
        let rejected = next_page(&mut fixture);
        assert_eq!(rejected.generation, 12);
        assert!(matches!(rejected.status,PageStatus::Failed(ref e)
            if classify_bridge_error(e) == Some(BridgeFailureKind::Busy)));
        fixture.release_page();
        fixture.wait_idle();
        assert_eq!(
            fixture.probe(),
            (false, false, 0),
            "late page must allocate neither native IDs nor cursor tokens"
        );
        assert!(
            fixture.next_event(Duration::from_millis(30)).is_none(),
            "retired page must not produce Ready or a duplicate terminal event"
        );
    }

    #[test]
    fn new_generation_same_folder_retires_original_request_scope() {
        let mut fixture = Fixture::start();
        selected_body(&mut fixture);
        fixture.send(page("fiction://INBOX", 1, 12));
        assert_eq!(next_page(&mut fixture).status, PageStatus::Superseded);
        assert_eq!(next_page(&mut fixture).generation, 12);
        fixture.release_page();
        fixture.wait_idle();
        assert!(!fixture.probe().1);
    }

    #[test]
    fn one_page_flight_does_not_reorder_serial_mutations() {
        let mut fixture = Fixture::start();
        selected_body(&mut fixture);
        for generation in 12..=20 {
            fixture.send(page("fiction://INBOX", 1, generation));
        }
        for seen in [true, false] {
            fixture.send(Command::Request {
                account_id: 1,
                request: MailRequest::SetSeen {
                    path: "fiction://INBOX".into(),
                    uid: fixture.selected_uid,
                    seen,
                },
            });
        }
        assert!(fixture.probe().0);
        let calls = fixture.calls.lock().unwrap();
        assert_eq!(
            calls.iter().filter(|(method, _)| method == "list").count(),
            1
        );
        assert_eq!(
            calls
                .iter()
                .filter(|(method, _)| method == "update")
                .map(|(_, params)| params["read"].clone())
                .collect::<Vec<_>>(),
            vec![json!(true), json!(false)]
        );
        drop(calls);
        fixture.release_page();
        fixture.wait_idle();
        assert_eq!(fixture.probe(), (false, false, 0));
    }

    #[test]
    fn request_timeout_is_not_login_and_releases_page_flight() {
        let mut fixture = Fixture::start();
        selected_body(&mut fixture);
        *fixture.list_reply.lock().unwrap() =
            Err("[megamail-tb:request-timeout] fictional list deadline".into());
        fixture.release_page();
        let failed = next_page(&mut fixture);
        assert!(matches!(failed.status,PageStatus::Failed(ref e)
            if classify_bridge_error(e) == Some(BridgeFailureKind::RequestTimeout)
                && classify_runtime_error(e) == ThunderbirdAccountState::Failed));
        fixture.wait_idle();
        *fixture.list_reply.lock().unwrap() = Ok(json!({"messages":[]}));
        fixture.send(page("fiction://INBOX", 1, 12));
        assert_eq!(next_page(&mut fixture).status, PageStatus::Ready);
        fixture.wait_idle();
    }

    #[test]
    fn caught_query_panic_releases_gate_and_reader_remains_usable() {
        let mut fixture = Fixture::start();
        selected_body(&mut fixture);
        fixture.panic_next.store(true, Ordering::Release);
        fixture.release_page();
        assert!(matches!(
            next_page(&mut fixture).status,
            PageStatus::Failed(_)
        ));
        fixture.wait_idle();
        fixture.send(page("fiction://INBOX", 1, 12));
        assert_eq!(next_page(&mut fixture).status, PageStatus::Ready);
        fixture.wait_idle();
    }

    #[test]
    fn disconnect_failure_retires_session_without_empty_success() {
        let mut fixture = Fixture::start();
        selected_body(&mut fixture);
        *fixture.list_reply.lock().unwrap() =
            Err("[megamail-tb:disconnected] fictional close".into());
        fixture.release_page();
        let failed = next_page(&mut fixture);
        assert_eq!(failed.generation, 11);
        assert!(matches!(failed.status,PageStatus::Failed(ref e)
            if classify_bridge_error(e) == Some(BridgeFailureKind::Disconnected)));
        let state = fixture.finish();
        assert!(state.retire_requested && state.page_lane.is_none() && state.page_flight.is_none());
        assert!(!state.remote_by_uid.values().any(|id| *id == json!(43)));
    }

    #[test]
    fn actor_sender_close_retires_pending_status_then_joins_page_worker() {
        let mut fixture = Fixture::start();
        selected_body(&mut fixture);
        fixture.sender.take();
        let failed = next_page(&mut fixture);
        assert_eq!(failed.generation, 11);
        assert!(matches!(failed.status, PageStatus::Failed(_)));
        // Status is delivered before releasing the fake list; shutdown joins
        // the owned lane after release instead of detaching its runtime clone.
        let state = fixture.finish();
        assert!(state.page_lane.is_none() && state.page_flight.is_none());
        assert!(!state.remote_by_uid.values().any(|id| *id == json!(43)));
    }

    #[test]
    fn closed_or_replaced_session_envelope_cannot_allocate_ids_or_cursors() {
        let mut fixture = Fixture::start();
        selected_body(&mut fixture);
        let mut state = fixture.finish();
        let job = PageJob {
            request: PageRequest {
                order: Default::default(),
                after_sorted: None,
                account_id: 1,
                folder_path: "fiction://INBOX".into(),
                folder_id: 1,
                query: String::new(),
                generation: 11,
                cursor: None,
                limit: PAGE_LIMIT,
            },
            source: state.profile.path.clone(),
            session: state.page_session,
            ticket: 1,
            account_remote: json!("fiction-account"),
            folder_remote: json!("fiction://INBOX"),
            params: json!({}),
        };
        let size = state.remote_by_uid.len();
        let value = json!({"messages":[{"id":999}],"cursor":"stale"});
        assert_eq!(
            state.project_page(&job, value.clone()).status,
            PageStatus::Superseded
        );
        state.retire_requested = false;
        state.page_session += 1;
        assert_eq!(
            state.project_page(&job, value.clone()).status,
            PageStatus::Superseded
        );
        state.page_session = job.session;
        state.profile.path = PathBuf::from("/fictional/replaced-source");
        assert_eq!(
            state.project_page(&job, value).status,
            PageStatus::Superseded
        );
        assert_eq!(state.remote_by_uid.len(), size);
        assert!(state.cursors.is_empty());
    }

    #[test]
    fn oversized_page_is_failed_not_projected() {
        let mut fixture = Fixture::start();
        selected_body(&mut fixture);
        *fixture.list_reply.lock().unwrap() = Ok(json!({
            "messages":[{"id":43,"subject":"x".repeat(PAGE_RESPONSE_BYTES)}]
        }));
        fixture.release_page();
        assert!(matches!(
            next_page(&mut fixture).status,
            PageStatus::Failed(_)
        ));
        fixture.wait_idle();
        assert_eq!(fixture.probe(), (false, false, 0));
    }

    #[test]
    fn accepted_page_keeps_captured_generation_and_actor_allocates_cursor() {
        let mut fixture = Fixture::start();
        selected_body(&mut fixture);
        fixture.release_page();
        let result = next_page(&mut fixture);
        assert_eq!(result.generation, 11);
        assert_eq!(result.folder_path, "fiction://INBOX");
        assert_eq!(result.status, PageStatus::Ready);
        assert_eq!(result.rows.len(), 1);
        assert_eq!(result.rows[0].subject, "Late fictional page");
        assert!(result.next_cursor.is_some());
        fixture.wait_idle();
        assert_eq!(fixture.probe(), (false, true, 1));
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ActorState, BODY_CACHE_BYTES, BODY_CACHE_ENTRIES, BRIDGE_REQUEST_FRAME_LIMIT, BodyCache,
        BodyPayload, READER_BODY_BYTES_LIMIT, SourceStore, WorkerMailboxEvent,
        address_from_identity, compact_error_summary, compose_recipients, is_broken_bridge_error,
        parse_folders, preflight_request, profile_actors, profile_sender, remove_profile_sender,
        reply_identity, resolve_reply_remote_id, should_retire_after_add,
        suppress_started_accounts_on_broken_prefetch, validate_store,
    };
    use crate::account_session::{StoredAccount, StoredProfile};
    use megamail_core::models::FolderKind;
    use serde_json::{Value, json};
    use std::collections::HashMap;
    use std::sync::{Arc, mpsc};

    #[test]
    fn folder_parser_rejects_duplicate_missing_and_excessively_nested_ids() {
        let duplicate = json!({
            "id": "account1://",
            "subFolders": [
                {"id": "account1://INBOX", "name": "Inbox", "type": "inbox"},
                {"id": "account1://INBOX", "name": "Copy", "type": "folder"}
            ]
        });
        assert!(
            parse_folders(1, &duplicate)
                .unwrap_err()
                .contains("duplicate folder ID")
        );

        let missing = json!({
            "id": "account1://",
            "subFolders": [{"name": "Inbox", "type": "inbox"}]
        });
        assert!(
            parse_folders(1, &missing)
                .unwrap_err()
                .contains("without an ID")
        );

        let mut nested = json!({"id": "folder-0", "name": "0", "type": "folder"});
        for index in 1..=34 {
            nested = json!({
                "id": format!("folder-{index}"),
                "name": format!("{index}"),
                "type": "folder",
                "subFolders": [nested]
            });
        }
        let too_deep = json!({"id": "account1://", "subFolders": [nested]});
        assert!(
            parse_folders(1, &too_deep)
                .unwrap_err()
                .contains("32-level safety limit")
        );
    }

    #[test]
    fn folder_parser_maps_thunderbird_archives_special_use_type() {
        let tree = json!({
            "id": "account1://",
            "subFolders": [{"id": "account1://Archive", "name": "All Mail", "type": "archives"}]
        });
        let folders = parse_folders(7, &tree).unwrap();
        assert_eq!(folders.len(), 1);
        assert_eq!(folders[0].kind, FolderKind::Archive);
        assert_eq!(folders[0].path, "account1://Archive");
    }

    #[test]
    fn source_manifest_rejects_duplicate_app_and_source_account_ids() {
        let profile_path = std::env::temp_dir().join("thunderbird-profile");
        let account = |source_id: &str, app_id| StoredAccount {
            source_id: source_id.into(),
            app_id,
        };
        let profile = |accounts| StoredProfile {
            path: profile_path.clone(),
            accounts,
        };

        let duplicate_source = SourceStore {
            version: 1,
            profiles: vec![profile(vec![
                account("account1", 10),
                account("account1", 11),
            ])],
        };
        assert!(validate_store(&duplicate_source).is_err());

        let duplicate_app_id = SourceStore {
            version: 1,
            profiles: vec![
                profile(vec![account("account1", 10)]),
                StoredProfile {
                    path: profile_path.with_extension("two"),
                    accounts: vec![account("account2", 10)],
                },
            ],
        };
        assert!(validate_store(&duplicate_app_id).is_err());
    }

    #[test]
    fn recipient_parser_preserves_commas_inside_quoted_from_names() {
        let recipients = compose_recipients(
            r#""Surname, Name" <a@example.test>, Other Person <b@example.test>"#,
        );
        assert_eq!(
            recipients,
            [
                r#""Surname, Name" <a@example.test>"#,
                "Other Person <b@example.test>"
            ]
        );
        assert_eq!(
            address_from_identity(r#""Surname, Name" <a@example.test>"#),
            "a@example.test"
        );
    }

    #[test]
    fn reply_target_accepts_label_copies_but_rejects_distinct_loaded_identities() {
        assert_eq!(
            reply_identity("Alice Person <alice@example.test>", 123),
            reply_identity("A. Person <ALICE@example.test>", 123),
            "Display-name changes do not make a duplicate folder copy a different identity"
        );
        assert_ne!(
            reply_identity("Alice Person <alice@example.test>", 123),
            reply_identity("Alice Person <alice@example.test>", 124)
        );

        let message_ids = HashMap::from([
            ((7, 10), "<same@example.test>".to_owned()),
            ((7, 11), "same@example.test".to_owned()),
            ((8, 12), "<same@example.test>".to_owned()),
        ]);
        let identities = HashMap::from([
            (
                (7, 10),
                reply_identity("Alice Person <alice@example.test>", 123),
            ),
            (
                (7, 11),
                reply_identity("A. Person <ALICE@example.test>", 123),
            ),
            ((8, 12), reply_identity("Bob <bob@example.test>", 456)),
        ]);
        let remote_ids = HashMap::from([
            ((7, 10), json!("remote-10")),
            ((7, 11), json!("remote-11")),
            ((8, 12), json!("other-account-remote")),
        ]);
        assert_eq!(
            resolve_reply_remote_id(
                7,
                "<SAME@example.test>",
                &message_ids,
                &identities,
                &remote_ids
            )
            .unwrap(),
            Some(json!("remote-10")),
            "Same-sender/date folder copies resolve deterministically within the account"
        );

        let mut distinct_ids = message_ids.clone();
        distinct_ids.insert((7, 13), "<same@example.test>".to_owned());
        let mut distinct_identities = identities;
        distinct_identities.insert((7, 13), reply_identity("Bob <bob@example.test>", 456));
        let mut distinct_remote_ids = remote_ids;
        distinct_remote_ids.insert((7, 13), json!("remote-13"));
        let error = resolve_reply_remote_id(
            7,
            "<same@example.test>",
            &distinct_ids,
            &distinct_identities,
            &distinct_remote_ids,
        )
        .unwrap_err();
        assert!(
            error
                .0
                .contains("multiple loaded messages share this Message-ID")
        );
        assert!(!error.1);

        let incomplete_ids = HashMap::from([
            ((7, 20), "<unknown@example.test>".to_owned()),
            ((7, 21), "<unknown@example.test>".to_owned()),
        ]);
        let incomplete_identities = HashMap::from([
            ((7, 20), reply_identity("", 0)),
            ((7, 21), reply_identity("", 0)),
        ]);
        let incomplete_remote_ids = HashMap::from([
            ((7, 20), json!("unknown-20")),
            ((7, 21), json!("unknown-21")),
        ]);
        assert!(
            resolve_reply_remote_id(
                7,
                "<unknown@example.test>",
                &incomplete_ids,
                &incomplete_identities,
                &incomplete_remote_ids
            )
            .is_err()
        );
    }

    #[test]
    fn oversized_send_payload_fails_preflight_before_bridge_call() {
        let params = json!({"body": "x".repeat(BRIDGE_REQUEST_FRAME_LIMIT)});
        let error = preflight_request("send", &params).unwrap_err();
        assert!(!error.1);
        assert!(error.0.contains("1 MiB request limit"));
    }

    #[test]
    fn small_bridge_payload_passes_preflight() {
        let params: Value = json!({"subject": "Hello", "body": "World"});
        assert!(preflight_request("send", &params).is_ok());
    }

    #[test]
    fn repeated_account_failures_are_collapsed_in_summary() {
        let errors = vec!["Thunderbird bridge is disconnected."; 6];
        let (summary, distinct) = compact_error_summary(errors);
        assert_eq!(distinct, 1);
        assert_eq!(summary, "6× Thunderbird bridge is disconnected.");
        assert!(!summary.contains('@'));
    }

    #[test]
    fn request_timeout_does_not_retire_a_healthy_bridge() {
        assert!(!is_broken_bridge_error(
            "Thunderbird timed out loading a body."
        ));
        assert!(is_broken_bridge_error(
            "Thunderbird bridge disconnected while the operation was running."
        ));
        assert!(is_broken_bridge_error("Broken pipe"));
        assert!(is_broken_bridge_error(
            "Thunderbird did not confirm this send. Check Sent before retrying. Thunderbird bridge disconnected while the operation was running."
        ));
        assert!(is_broken_bridge_error(
            "Thunderbird could not export raw source: socket closed"
        ));
        assert!(!is_broken_bridge_error(
            "Thunderbird did not confirm draft deletion before the timeout."
        ));
    }

    #[test]
    fn body_cache_is_lru_bounded_by_entries_and_memory() {
        let mut cache = BodyCache::default();
        let payload = |body: String, html: String| BodyPayload {
            original: Arc::new(crate::reader_session::OriginalBody::capture(
                &html,
                Some(true),
                None,
            )),
            body,
            html,
            html_without_quote: None,
            links: Vec::new(),
            has_attachment: None,
            reply_to: None,
            references: None,
        };
        let key = |uid| (7, "Inbox".to_owned(), uid);
        for uid in 1..=BODY_CACHE_ENTRIES as u32 {
            cache.insert(key(uid), payload(format!("body-{uid}"), String::new()));
        }
        assert_eq!(cache.entries.len(), BODY_CACHE_ENTRIES);
        assert!(cache.get(&key(1)).is_some());
        cache.insert(key(9), payload("body-9".into(), String::new()));
        assert!(
            cache.get(&key(1)).is_some(),
            "a hit should refresh LRU order"
        );
        assert!(
            cache.get(&key(2)).is_none(),
            "the least-recently-used entry should be evicted"
        );

        let large = "x".repeat(BODY_CACHE_BYTES / 2 + 1);
        cache.insert(key(10), payload(large.clone(), String::new()));
        cache.insert(key(11), payload(large, String::new()));
        assert!(cache.bytes <= BODY_CACHE_BYTES);
        assert_eq!(cache.entries.len(), 1);

        let mut html_cache = BodyCache::default();
        let mut formatted = payload("plain".into(), "<p>format</p>".into());
        formatted.html_without_quote = Some("<p>format</p>".into());
        let expected_bytes = formatted.original.retained_bytes()
            + formatted.body.len()
            + formatted.html.len()
            + formatted.html_without_quote.as_ref().unwrap().len();
        assert_eq!(formatted.original.retained_bytes(), "<p>format</p>".len());
        html_cache.insert(key(12), formatted);
        assert_eq!(html_cache.bytes, expected_bytes);
        html_cache.insert(
            key(13),
            payload("plain".into(), "x".repeat(BODY_CACHE_BYTES)),
        );
        assert_eq!(html_cache.entries.len(), 1);
        assert!(html_cache.get(&key(12)).is_some());
    }

    #[test]
    fn thunderbird_body_keeps_allowlisted_html_and_plain_text() {
        let payload = ActorState::parse_body_payload(json!({
            "html": "<p>Hello <strong>world</strong><script>alert(1)</script><img src=\"https://tracker.test/pixel\"></p><blockquote><p>Earlier note</p></blockquote>",
            "plainText": "Hello world",
            "hasAttachment": false
        }))
        .unwrap();
        assert_eq!(payload.body, "Hello world");
        assert!(payload.html.contains("<strong>world</strong>"));
        assert!(!payload.html.contains("<script"));
        assert!(!payload.html.contains("tracker.test"));
        assert!(payload.html_without_quote.is_some());
        assert!(
            !payload
                .html_without_quote
                .as_ref()
                .unwrap()
                .contains("Earlier note")
        );
    }

    #[test]
    fn actual_thunderbird_payload_preserves_original_truncation_and_unknown_cache_policy() {
        let source = "<style>table{width:100%}</style><table><tr><td>Fixture</td></tr></table>";
        let payload = ActorState::parse_body_payload(
            json!({ "html": source, "plainText": "Fixture", "truncated": false }),
        )
        .unwrap();
        assert_eq!(payload.original.source.as_deref(), Some(source));
        assert_eq!(
            payload.original.completeness,
            crate::reader_document::Completeness::Complete
        );
        assert_eq!(
            payload.original.eligibility,
            crate::reader_document::CacheEligibility::Unknown
        );
        assert!(!payload.html.contains("<style>"));
        assert!(payload.byte_len() >= source.len() + payload.body.len() + payload.html.len());
        let event = payload.event(9, "INBOX");
        assert!(matches!(
            event,
            WorkerMailboxEvent::Body {
                original: Some(_),
                message_id: 9,
                ..
            }
        ));
        let truncated =
            ActorState::parse_body_payload(json!({ "html": source, "truncated": true })).unwrap();
        assert_eq!(
            truncated.original.completeness,
            crate::reader_document::Completeness::Truncated
        );
    }
    #[test]
    fn thunderbird_body_rejects_oversized_plain_or_html_text() {
        let oversized = "x".repeat(READER_BODY_BYTES_LIMIT + 1);
        assert!(ActorState::parse_body_payload(json!({ "html": oversized })).is_err());
        let oversized = "x".repeat(READER_BODY_BYTES_LIMIT + 1);
        assert!(ActorState::parse_body_payload(json!({ "plainText": oversized })).is_err());
    }

    #[test]
    fn failed_empty_startup_retires_actor_so_retry_gets_a_new_channel() {
        assert!(should_retire_after_add(0, 0, false));
        assert!(should_retire_after_add(0, 2, true));
        assert!(!should_retire_after_add(0, 1, false));

        let path = std::env::temp_dir().join(format!(
            "megamail-thunderbird-retry-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let (first_tx, _first_rx) = mpsc::sync_channel::<super::Command>(1);
        let first = Arc::new(first_tx);
        let first_weak = Arc::downgrade(&first);
        profile_actors()
            .lock()
            .unwrap()
            .insert(path.clone(), first_weak.clone());
        remove_profile_sender(&path, &first_weak);
        assert!(profile_sender(&path).unwrap().is_none());

        let (retry_tx, _retry_rx) = mpsc::sync_channel::<super::Command>(1);
        let retry = Arc::new(retry_tx);
        let retry_weak = Arc::downgrade(&retry);
        profile_actors()
            .lock()
            .unwrap()
            .insert(path.clone(), retry_weak.clone());
        let registered_retry = profile_sender(&path).unwrap().unwrap();
        assert!(Arc::ptr_eq(&retry, &registered_retry));
        remove_profile_sender(&path, &retry_weak);
    }

    #[test]
    fn broken_startup_prefetch_never_returns_ready_accounts() {
        let mut ready_accounts = vec![()];
        let mut outcomes = vec![super::ThunderbirdAccountOutcome {
            profile_path: std::path::PathBuf::from("synthetic-profile"),
            source_account_id: "account-1".into(),
            name: "Synthetic account".into(),
            email: "synthetic@example.test".into(),
            state: super::ThunderbirdAccountState::Connected,
            message: None,
        }];

        suppress_started_accounts_on_broken_prefetch(
            &mut ready_accounts,
            &mut outcomes,
            "Thunderbird bridge disconnected during startup.",
        );

        assert!(ready_accounts.is_empty());
        assert_eq!(outcomes[0].state, super::ThunderbirdAccountState::Failed);
        assert_eq!(
            outcomes[0].message.as_deref(),
            Some("Thunderbird bridge disconnected during startup.")
        );
    }
}

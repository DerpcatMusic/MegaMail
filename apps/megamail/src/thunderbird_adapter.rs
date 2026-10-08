//! Session-scoped adapter for Thunderbird's authenticated MailExtension API.
//!
//! The extension owns its OAuth/session state. This module keeps opaque
//! Thunderbird IDs in the live actor only; it never writes message IDs or page
//! rows to MegaMail's native cache.

use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, SyncSender, TrySendError};
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::thread;

use base64::Engine as _;
use futures::channel::mpsc as gpui_mpsc;
use megamail_core::models::{Account, Folder, FolderKind, Importance, Message};
use megamail_core::query::{PageRequest, PageResult, PageStatus, SubmitError};
use megamail_core::thunderbird::{ThunderbirdAccount, ThunderbirdProfile};
use megamail_core::thunderbird_bridge::Runtime;
use megamail_core::worker::{MailRequest, OutgoingMessage, SendOutcome};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::live::{
    AttachmentSummary, MailboxEvent, PageSender, RequestSender, SenderIdentity,
    ThunderbirdAccountOutcome, ThunderbirdAccountState, WorkerEnvelope, WorkerMailboxEvent,
};

const STORE_NAME: &str = "thunderbird-sources.json";
const STORE_VERSION: u32 = 1;
const PAGE_LIMIT: usize = 100;
const SESSION_MESSAGE_LIMIT: usize = 25_000;
const FOLDER_LIMIT: usize = 4_096;
const ATTACHMENT_LIMIT: usize = 32;
const ATTACHMENT_BYTES_LIMIT: usize = 50 * 1024 * 1024;
const RAW_BYTES_LIMIT: usize = 100 * 1024 * 1024;
const TRANSFER_CHUNK: usize = 512 * 1024;
const COMMAND_QUEUE: usize = 64;
const BRIDGE_REQUEST_FRAME_LIMIT: usize = 1024 * 1024;
const TRANSFER_ID_FRAME_RESERVE: usize = 64;

type StartResult = Result<
    (Vec<StartedAccount>, Vec<ThunderbirdAccountOutcome>),
    (ThunderbirdAccountState, String),
>;

static PROFILE_ACTORS: OnceLock<Mutex<HashMap<PathBuf, Weak<SyncSender<Command>>>>> =
    OnceLock::new();

type EventSender = gpui_mpsc::UnboundedSender<MailboxEvent>;

#[derive(Serialize, Deserialize, Default)]
struct SourceStore {
    version: u32,
    #[serde(default)]
    profiles: Vec<StoredProfile>,
}

#[derive(Serialize, Deserialize)]
struct StoredProfile {
    path: PathBuf,
    accounts: Vec<StoredAccount>,
}

#[derive(Serialize, Deserialize)]
struct StoredAccount {
    source_id: String,
    app_id: u32,
}

#[derive(Clone)]
struct SelectedAccount {
    account: ThunderbirdAccount,
    app_id: u32,
}

pub(super) struct StartedAccount {
    pub account: Account,
    pub folders: Vec<Folder>,
    pub request_sender: RequestSender,
    pub page_sender: PageSender,
    pub identities: Vec<SenderIdentity>,
    pub error: Option<String>,
}

enum Command {
    Request {
        account_id: u32,
        request: MailRequest,
    },
    Page(PageRequest),
    AddAccounts {
        selected: Vec<SelectedAccount>,
        occupied_ids: HashSet<u32>,
        occupied_emails: HashSet<String>,
        response: SyncSender<StartResult>,
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

enum SendFailure {
    Definite(String, bool),
    Uncertain(String),
}

impl From<(String, bool)> for SendFailure {
    fn from((message, connectivity): (String, bool)) -> Self {
        Self::Definite(message, connectivity)
    }
}

struct ActorState {
    runtime: Runtime,
    profile: ThunderbirdProfile,
    remote_accounts: Vec<Value>,
    command_sender: Weak<SyncSender<Command>>,
    accounts: HashMap<u32, AccountBinding>,
    message_by_remote: HashMap<(u32, u32, String), u32>,
    remote_by_uid: HashMap<(u32, u32), Value>,
    message_id_by_uid: HashMap<(u32, u32), String>,
    cursors: HashMap<(u32, u32, u32), String>,
    initial_pages: HashMap<(u32, u32), Value>,
    next_uid: u32,
    next_cursor: u32,
    event_sender: EventSender,
}

/// Import selected scanner rows. Runtime startup, authentication, folders and
/// all bridge operations run on named background threads.
pub(super) fn import(
    selections: Vec<(ThunderbirdProfile, Vec<String>)>,
    reserved_ids: HashSet<u32>,
    running_thunderbird_ids: HashSet<u32>,
    existing_emails: Vec<String>,
    event_sender: EventSender,
) {
    let result = import_selected(
        selections,
        reserved_ids,
        running_thunderbird_ids,
        existing_emails,
        event_sender.clone(),
    );
    match result {
        Ok((accounts, results, notice)) => {
            let error = outcome_error(&results);
            let _ = event_sender.unbounded_send(MailboxEvent::ThunderbirdReady {
                accounts,
                results,
                error,
                notice,
            });
        }
        Err(error) => {
            let _ = event_sender.unbounded_send(MailboxEvent::ThunderbirdReady {
                accounts: Vec::new(),
                results: Vec::new(),
                error: Some(error),
                notice: None,
            });
        }
    }
}

/// Restore the private source manifest against profiles currently on disk.
pub(super) fn restore(
    event_sender: EventSender,
    reserved_ids: HashSet<u32>,
    existing_emails: Vec<String>,
) {
    let result = restore_saved(event_sender.clone(), reserved_ids, existing_emails);
    match result {
        Ok((accounts, results, notice)) => {
            let error = outcome_error(&results);
            let _ = event_sender.unbounded_send(MailboxEvent::ThunderbirdReady {
                accounts,
                results,
                error,
                notice,
            });
        }
        Err(error) => {
            let _ = event_sender.unbounded_send(MailboxEvent::ThunderbirdReady {
                accounts: Vec::new(),
                results: Vec::new(),
                error: Some(error),
                notice: None,
            });
        }
    }
}

fn import_selected(
    selections: Vec<(ThunderbirdProfile, Vec<String>)>,
    reserved_ids: HashSet<u32>,
    running_thunderbird_ids: HashSet<u32>,
    existing_emails: Vec<String>,
    event_sender: EventSender,
) -> Result<
    (
        Vec<StartedAccount>,
        Vec<ThunderbirdAccountOutcome>,
        Option<String>,
    ),
    String,
> {
    let mut store =
        read_store().map_err(|error| format!("Could not read Thunderbird imports: {error}"))?;
    let mut reserved = reserved_ids.clone();
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
    let mut used_new_id = reserved
        .iter()
        .copied()
        .max()
        .unwrap_or(0)
        .saturating_add(1)
        .max(1);

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
        let selected_ids: HashSet<String> = ids.into_iter().collect();
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
                if running_thunderbird_ids.contains(&app_id) {
                    results.push(outcome(
                        &profile,
                        &source_id,
                        &email,
                        ThunderbirdAccountState::Connected,
                        Some("This Thunderbird account is already connected.".into()),
                    ));
                    continue;
                }
                if reserved_ids.contains(&app_id) {
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
                let app_id = next_free_id(&mut used_new_id, &reserved);
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

    let mut started = Vec::new();
    let mut new_profiles = Vec::new();
    for (profile, accounts) in to_start {
        match start_profile(
            profile.clone(),
            accounts.clone(),
            event_sender.clone(),
            connected_ids.clone(),
            connected_emails.clone(),
        ) {
            Ok((ready, mut outcomes)) => {
                for ready_account in &ready {
                    new_profiles.push((
                        profile.path.clone(),
                        ready_account.account.id,
                        source_id_for(&accounts, ready_account.account.id),
                    ));
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

    if !started.is_empty() {
        for (path, app_id, source_id) in new_profiles {
            if let Some(profile) = store.profiles.iter_mut().find(|item| item.path == path) {
                if !profile
                    .accounts
                    .iter()
                    .any(|account| account.source_id == source_id)
                {
                    profile.accounts.push(StoredAccount { source_id, app_id });
                }
            } else {
                store.profiles.push(StoredProfile {
                    path,
                    accounts: vec![StoredAccount { source_id, app_id }],
                });
            }
        }
        write_store(&store).map_err(|error| {
            format!("Connected Thunderbird accounts could not be saved: {error}")
        })?;
    }

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
    Ok((started, results, notice))
}

fn restore_saved(
    event_sender: EventSender,
    reserved_ids: HashSet<u32>,
    existing_emails: Vec<String>,
) -> Result<
    (
        Vec<StartedAccount>,
        Vec<ThunderbirdAccountOutcome>,
        Option<String>,
    ),
    String,
> {
    let store =
        read_store().map_err(|error| format!("Could not read Thunderbird imports: {error}"))?;
    if store.profiles.is_empty() {
        return Ok((Vec::new(), Vec::new(), None));
    }
    let available = megamail_core::thunderbird::discover_profiles()
        .map_err(|error| format!("Could not find imported Thunderbird profiles: {error}"))?;
    let mut by_path = HashMap::new();
    for profile in available {
        if let Ok(path) = fs::canonicalize(&profile.path) {
            by_path.insert(path, profile);
        }
    }
    let mut started = Vec::new();
    let mut results = Vec::new();
    let mut connected_ids = reserved_ids;
    let mut connected_emails: HashSet<String> = existing_emails
        .into_iter()
        .map(|email| normalize_email(&email))
        .filter(|email| !email.is_empty())
        .collect();
    let mut claimed_ids = connected_ids.clone();
    let mut claimed_emails = connected_emails.clone();
    for stored in store.profiles {
        let canonical = fs::canonicalize(&stored.path).unwrap_or(stored.path.clone());
        let Some(mut profile) = by_path.remove(&canonical) else {
            for account in stored.accounts {
                results.push(ThunderbirdAccountOutcome {
                    profile_path: stored.path.clone(),
                    source_account_id: account.source_id,
                    name: "Thunderbird account".into(),
                    email: String::new(),
                    state: ThunderbirdAccountState::Failed,
                    message: Some("The imported Thunderbird profile is unavailable.".into()),
                });
            }
            continue;
        };
        profile.path = canonical;
        let mut selected = Vec::new();
        let stored_accounts = stored.accounts;
        for account in &stored_accounts {
            if let Some(source) = profile
                .accounts
                .iter()
                .find(|item| item.account_id == account.source_id)
                .cloned()
            {
                if !claimed_ids.insert(account.app_id) {
                    results.push(outcome(
                        &profile,
                        &account.source_id,
                        source.email.as_deref().unwrap_or_default(),
                        ThunderbirdAccountState::Failed,
                        Some("This Thunderbird account ID conflicts with another configured account.".into()),
                    ));
                    continue;
                }
                let email = source.email.as_deref().unwrap_or_default();
                if !email.trim().is_empty() && !claimed_emails.insert(normalize_email(email)) {
                    results.push(outcome(
                        &profile,
                        &account.source_id,
                        email,
                        ThunderbirdAccountState::Failed,
                        Some("An account with this email address is already connected.".into()),
                    ));
                    continue;
                }
                selected.push(SelectedAccount {
                    account: source,
                    app_id: account.app_id,
                });
            } else {
                results.push(outcome(
                    &profile,
                    &account.source_id,
                    "",
                    ThunderbirdAccountState::Unsupported,
                    Some("This Thunderbird account is no longer available in its profile.".into()),
                ));
            }
        }
        if selected.is_empty() {
            continue;
        }
        match start_profile(
            profile.clone(),
            selected,
            event_sender.clone(),
            connected_ids.clone(),
            connected_emails.clone(),
        ) {
            Ok((mut ready, mut outcomes)) => {
                for account in &ready {
                    connected_ids.insert(account.account.id);
                    connected_emails.insert(normalize_email(&account.account.email));
                }
                started.append(&mut ready);
                results.append(&mut outcomes);
            }
            Err((state, message)) => {
                for stored_account in &stored_accounts {
                    let email = profile
                        .accounts
                        .iter()
                        .find(|account| account.account_id == stored_account.source_id)
                        .and_then(|account| account.email.as_deref())
                        .unwrap_or_default();
                    results.push(outcome(
                        &profile,
                        &stored_account.source_id,
                        email,
                        state,
                        Some(message.clone()),
                    ));
                }
            }
        }
    }
    let connected = started.len();
    let failed = results
        .iter()
        .filter(|result| result.state != ThunderbirdAccountState::Connected)
        .count();
    let notice = (connected > 0).then(|| {
        if failed == 0 {
            format!(
                "Restored {connected} Thunderbird account{}.",
                if connected == 1 { "" } else { "s" }
            )
        } else {
            format!(
                "Restored {connected} Thunderbird account{}; {failed} need attention.",
                if connected == 1 { "" } else { "s" }
            )
        }
    });
    Ok((started, results, notice))
}

fn source_id_for(accounts: &[SelectedAccount], app_id: u32) -> String {
    accounts
        .iter()
        .find(|selected| selected.app_id == app_id)
        .map(|selected| selected.account.account_id.clone())
        .unwrap_or_default()
}

fn next_free_id(next: &mut u32, used: &HashSet<u32>) -> u32 {
    loop {
        let candidate = (*next).max(1);
        *next = candidate.wrapping_add(1).max(1);
        if !used.contains(&candidate) {
            return candidate;
        }
    }
}

fn read_store() -> io::Result<SourceStore> {
    let path = store_path()?;
    let mut file = match File::open(&path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(SourceStore {
                version: STORE_VERSION,
                profiles: Vec::new(),
            });
        }
        Err(error) => return Err(error),
    };
    const MAX_STORE_BYTES: usize = 256 * 1024;
    let mut bytes = Vec::with_capacity(MAX_STORE_BYTES.min(16 * 1024));
    file.by_ref()
        .take((MAX_STORE_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_STORE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Thunderbird source manifest is too large",
        ));
    }
    let store: SourceStore = serde_json::from_slice(&bytes)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    if store.version != STORE_VERSION {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "unsupported Thunderbird source manifest version",
        ));
    }
    validate_store(&store)?;
    Ok(store)
}

fn validate_store(store: &SourceStore) -> io::Result<()> {
    if store.profiles.len() > 32 || store.profiles.iter().any(|p| p.accounts.len() > 32) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "too many saved Thunderbird sources",
        ));
    }
    let mut profile_paths = HashSet::new();
    let mut source_accounts = HashSet::new();
    let mut app_ids = HashSet::new();
    for profile in &store.profiles {
        if !profile.path.is_absolute() || !profile_paths.insert(profile.path.clone()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Thunderbird source manifest has a duplicate or invalid profile path",
            ));
        }
        for account in &profile.accounts {
            if account.source_id.trim().is_empty()
                || account.source_id.len() > 128
                || account.source_id.chars().any(char::is_control)
                || account.app_id == 0
                || !source_accounts.insert((profile.path.clone(), account.source_id.clone()))
                || !app_ids.insert(account.app_id)
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Thunderbird source manifest has a duplicate or invalid account ID",
                ));
            }
        }
    }
    Ok(())
}

fn write_store(store: &SourceStore) -> io::Result<()> {
    let bytes = serde_json::to_vec_pretty(store)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let path = store_path()?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    megamail_core::config::write_private_file(&path, text)
}

fn store_path() -> io::Result<PathBuf> {
    megamail_core::config::config_base()
        .map(|path| path.join(STORE_NAME))
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "could not locate MegaMail's private config directory",
            )
        })
}

fn start_profile(
    profile: ThunderbirdProfile,
    selected: Vec<SelectedAccount>,
    event_sender: EventSender,
    occupied_ids: HashSet<u32>,
    occupied_emails: HashSet<String>,
) -> StartResult {
    if let Some(sender) = profile_sender(&profile.path)? {
        return add_accounts_to_actor(sender, selected, occupied_ids, occupied_emails);
    }

    let (command_tx, command_rx) = mpsc::sync_channel(COMMAND_QUEUE);
    let command_sender = Arc::new(command_tx);
    let command_sender_weak = Arc::downgrade(&command_sender);
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
            return add_accounts_to_actor(existing, selected, occupied_ids, occupied_emails);
        }
        actors.insert(profile.path.clone(), Arc::downgrade(&command_sender));
    }
    let (ready_tx, ready_rx) = mpsc::sync_channel(1);
    let actor_profile = profile.clone();
    let profile_path = profile.path.clone();
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
                Ok(runtime) => runtime,
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
                profile: actor_profile,
                remote_accounts: accounts,
                command_sender: command_sender_weak,
                accounts: HashMap::new(),
                message_by_remote: HashMap::new(),
                remote_by_uid: HashMap::new(),
                message_id_by_uid: HashMap::new(),
                cursors: HashMap::new(),
                initial_pages: HashMap::new(),
                next_uid: 1,
                next_cursor: 1,
                event_sender: event_sender.clone(),
            };
            if ready_tx
                .send(state.add_accounts(selected, occupied_ids, occupied_emails))
                .is_err()
            {
                return;
            }
            while let Ok(command) = command_rx.recv() {
                match command {
                    Command::Request {
                        account_id,
                        request,
                    } => state.handle_request(account_id, request),
                    Command::Page(request) => state.handle_page(request),
                    Command::AddAccounts {
                        selected,
                        occupied_ids,
                        occupied_emails,
                        response,
                    } => {
                        let _ = response.send(state.add_accounts(
                            selected,
                            occupied_ids,
                            occupied_emails,
                        ));
                    }
                }
            }
        })
        .map_err(|error| {
            remove_profile_sender(&profile_path, &command_sender);
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

fn remove_profile_sender(path: &Path, sender: &Arc<SyncSender<Command>>) {
    if let Ok(mut actors) = profile_actors().lock() {
        if actors
            .get(path)
            .and_then(Weak::upgrade)
            .is_some_and(|current| Arc::ptr_eq(&current, sender))
        {
            actors.remove(path);
        }
    }
}

fn add_accounts_to_actor(
    sender: Arc<SyncSender<Command>>,
    selected: Vec<SelectedAccount>,
    occupied_ids: HashSet<u32>,
    occupied_emails: HashSet<String>,
) -> StartResult {
    let (response_tx, response_rx) = mpsc::sync_channel(1);
    sender
        .try_send(Command::AddAccounts {
            selected,
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

impl ActorState {
    fn add_accounts(
        &mut self,
        selected: Vec<SelectedAccount>,
        occupied_ids: HashSet<u32>,
        occupied_emails: HashSet<String>,
    ) -> StartResult {
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
            if occupied_ids.contains(&selected_account.app_id)
                || self.accounts.contains_key(&selected_account.app_id)
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
            match self.add_account(&remote_accounts, &selected_account, command_sender.clone()) {
                Ok((ready, binding, (folder_id, initial_page))) => {
                    if occupied_emails.contains(&normalize_email(&binding.email))
                        || self.accounts.values().any(|existing| {
                            normalize_email(&existing.email) == normalize_email(&binding.email)
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
                    self.accounts.insert(binding.app_id, binding);
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
        Ok((started, outcomes))
    }

    fn add_account(
        &mut self,
        remote_accounts: &[Value],
        selected: &SelectedAccount,
        command_tx: Arc<SyncSender<Command>>,
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
        let folders_value = self
            .runtime
            .call("folders", json!({ "accountId": remote_id }))
            .map_err(|error| (classify_runtime_error(&error), friendly_error(&error)))?;
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
        let initial_folder_path = initial_folder.path.clone();
        let initial_page = self
            .runtime
            .call(
                "list",
                json!({
                    "folderId": initial_folder_path,
                    "limit": PAGE_LIMIT,
                }),
            )
            .map_err(|error| (classify_runtime_error(&error), friendly_error(&error)))?;
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
        let request_sender: RequestSender = Arc::new(move |request| {
            command_sender
                .try_send(Command::Request {
                    account_id: app_id,
                    request,
                })
                .is_ok()
        });
        let page_sender = page_sender(command_tx, app_id);
        let ready = StartedAccount {
            account,
            folders,
            request_sender,
            page_sender,
            identities,
            error: None,
        };
        Ok((ready, binding, (initial_folder_id, initial_page)))
    }

    fn handle_page(&mut self, request: PageRequest) {
        let result = self.page(request);
        let _ = self.event_sender.unbounded_send(MailboxEvent::Page(result));
    }

    fn page(&mut self, request: PageRequest) -> PageResult {
        let account_id = request.account_id;
        let path = request.folder_path.clone();
        let folder_id = request.folder_id;
        let generation = request.generation;
        let failed = |error: String| PageResult {
            account_id,
            folder_path: path.clone(),
            folder_id,
            generation,
            rows: Vec::new(),
            next_cursor: None,
            status: PageStatus::Failed(error),
        };
        if request.limit > PAGE_LIMIT || request.limit == 0 {
            return failed(format!(
                "Thunderbird pages are limited to {PAGE_LIMIT} messages."
            ));
        }
        let Some(account) = self.accounts.get(&account_id) else {
            return failed("This Thunderbird account is no longer connected.".into());
        };
        let Some(folder) = account
            .folders
            .iter()
            .find(|folder| folder.folder.id == folder_id && folder.folder.path == path)
        else {
            return failed("This Thunderbird folder is no longer available.".into());
        };
        let remote_folder_id = folder.remote_id.clone();
        if request.cursor.is_none() {
            self.cursors
                .retain(|(id, f_id, _), _| *id != account_id || *f_id != folder_id);
        }
        let preloaded_page = request
            .cursor
            .is_none()
            .then(|| self.initial_pages.remove(&(account_id, folder_id)))
            .flatten();
        let remote_cursor = match request.cursor {
            Some(cursor) => {
                match self
                    .cursors
                    .remove(&(account_id, folder_id, cursor.before_uid))
                {
                    Some(cursor) => Some(cursor),
                    None => {
                        return PageResult {
                            status: PageStatus::Superseded,
                            ..failed(String::new())
                        };
                    }
                }
            }
            None => None,
        };
        let mut params = json!({ "folderId": remote_folder_id, "limit": request.limit });
        if let Some(cursor) = remote_cursor {
            params["cursor"] = Value::String(cursor);
        }
        if !request.query.trim().is_empty() {
            params["search"] = Value::String(request.query.clone());
        }
        let value = match preloaded_page.filter(|_| {
            request.cursor.is_none()
                && request.query.trim().is_empty()
                && request.limit == PAGE_LIMIT
        }) {
            Some(value) => value,
            None => match self.runtime.call("list", params) {
                Ok(value) => value,
                Err(error) => return failed(friendly_error(&error)),
            },
        };
        if let Some(unread) = folder_unread_count(&value) {
            if let Some(folder) = self.accounts.get_mut(&account_id).and_then(|account| {
                account
                    .folders
                    .iter_mut()
                    .find(|folder| folder.folder.path == path)
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
        let headers = value.get("messages").and_then(Value::as_array);
        let Some(headers) = headers else {
            return failed("Thunderbird returned an invalid message page.".into());
        };
        if headers.len() > request.limit {
            return failed(
                "Thunderbird returned a message page larger than the requested limit.".into(),
            );
        }
        let mut rows = Vec::with_capacity(headers.len());
        for header in headers {
            match self.map_header(account_id, folder_id, header) {
                Ok(message) => rows.push(message),
                Err(error) => return failed(error),
            }
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
            account_id,
            folder_path: path,
            folder_id,
            generation,
            rows,
            next_cursor,
            status: PageStatus::Ready,
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
            let uid = self.next_uid;
            self.next_uid = self.next_uid.wrapping_add(1).max(1);
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

    fn handle_request(&mut self, account_id: u32, request: MailRequest) {
        let result = self.request(account_id, request);
        if let Err((text, connectivity)) = result {
            let _ = self
                .event_sender
                .unbounded_send(MailboxEvent::Worker(WorkerEnvelope {
                    account_id,
                    event: WorkerMailboxEvent::Error { text, connectivity },
                }));
        }
    }

    fn request(&mut self, account_id: u32, request: MailRequest) -> Result<(), (String, bool)> {
        match request {
            MailRequest::LoadMessages { .. } | MailRequest::SyncFolder { .. } | MailRequest::Settle { .. } => Ok(()),
            MailRequest::RefreshUnread => self.refresh_folders(account_id),
            MailRequest::Reconnect => self.refresh_folders(account_id),
            MailRequest::LoadBody { message_id, path, uid } => {
                let remote_id = self.remote_message(account_id, &path, message_id, uid)?;
                let value = self.call("body", json!({ "messageId": remote_id }))?;
                let html = string(value.get("html"));
                if html.len() > 2 * 1024 * 1024 {
                    return Err(("This message body exceeds the 2 MiB reader limit.".into(), false));
                }
                let plain = string(value.get("plainText"));
                let body = if !plain.is_empty() { plain } else { megamail_core::markdown::plain_text(&html) };
                let links = if html.is_empty() { Vec::new() } else { megamail_core::mail_text::extract_links(&html) };
                let has_attachment = value.get("hasAttachment").and_then(Value::as_bool);
                let reply_to = value.get("replyTo").map(|_| string(value.get("replyTo")));
                let references = value.get("references").map(|_| string(value.get("references")));
                self.emit(account_id, WorkerMailboxEvent::Body { message_id, path, body, links, has_attachment, reply_to, references });
                Ok(())
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
                        if !transfer_id.is_empty() { let _ = self.call("download_end", json!({"transferId":transfer_id})); }
                        return Err(("Attachments exceed MegaMail’s 50 MiB reader limit; save them from Thunderbird.".into(), false));
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
                let downloaded: Result<Vec<u8>, String> = (|| {
                    let meta = self
                        .call("raw_start", json!({ "messageId": message_id }))
                        .map_err(|(text, _)| text)?;
                    let transfer_id = string(meta.get("transferId"));
                    let size = meta.get("size").and_then(Value::as_u64).unwrap_or(u64::MAX);
                    let cap = max_bytes.unwrap_or(RAW_BYTES_LIMIT as u64).min(RAW_BYTES_LIMIT as u64);
                    if size > cap {
                        let _ = self.call("download_end", json!({ "transferId": transfer_id }));
                        return Err(format!("This message is over the {} MiB draft editing limit and was left unchanged.", cap / 1024 / 1024));
                    }
                    self.download(&transfer_id, size as usize)
                        .map_err(|(error, _)| friendly_error(&error))
                })();
                let raw = downloaded.map(Arc::new);
                self.emit(account_id, WorkerMailboxEvent::RawExported { token, raw });
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
                            self.emit(account_id, WorkerMailboxEvent::Notice(text));
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
                            self.emit(account_id, WorkerMailboxEvent::Notice(text));
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
        if in_reply_to.trim().is_empty() {
            return Ok(None);
        }
        let wanted = normalize_message_id(in_reply_to);
        self.message_id_by_uid
            .iter()
            .find(|((id, _), message_id)| {
                *id == account_id && normalize_message_id(message_id) == wanted
            })
            .and_then(|((_, uid), _)| self.remote_by_uid.get(&(account_id, *uid)))
            .cloned()
            .map(Some)
            .ok_or_else(|| {
                (
                    "The original message is no longer loaded in Thunderbird. Refresh the folder before replying so its thread headers are preserved.".into(),
                    false,
                )
            })
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
        let _ = self.call("download_end", json!({ "transferId": transfer_id }));
        result
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
        if self
            .call("delete", json!({ "messageId": remote_id }))
            .is_err()
        {
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
    Some(
        failed
            .iter()
            .map(|result| {
                let address = if result.email.is_empty() {
                    result.name.as_str()
                } else {
                    result.email.as_str()
                };
                format!(
                    "{address}: {}",
                    result.message.as_deref().unwrap_or("Could not connect.")
                )
            })
            .collect::<Vec<_>>()
            .join(" "),
    )
}

fn classify_runtime_error(error: &str) -> ThunderbirdAccountState {
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
mod tests {
    use super::{
        BRIDGE_REQUEST_FRAME_LIMIT, SourceStore, StoredAccount, StoredProfile,
        address_from_identity, compose_recipients, folder_kind, parse_folders, preflight_request,
        validate_store,
    };
    use megamail_core::models::FolderKind;
    use serde_json::{Value, json};

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
}

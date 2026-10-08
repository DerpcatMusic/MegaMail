//! Live mailbox state backed by the extracted Hylki mail workers.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use futures::StreamExt;
use gpui_kit::{Context, Task};
use megamail_core::{
    cache::MessageCursor,
    config::AccountConfig,
    mail_text::MailLink,
    models::{Account, Attachment, DraftOrigin, Folder, FolderKind, Message, OutboxItem},
    query::{PageRequest, PageResult, PageStatus, QueryService, SubmitError},
    thunderbird::ThunderbirdProfile,
    worker::{self, MailRequest, OutgoingMessage, SendOutcome, WorkerEvent},
};

const PAGE_SIZE: usize = 100;
const MESSAGE_CAP: usize = 5_000;
const BODY_HTML_CAP: usize = 2 * 1024 * 1024;
const ATTACHMENT_COUNT_CAP: usize = 32;
const ATTACHMENT_BYTES_CAP: usize = 50 * 1024 * 1024;
const DRAFT_RAW_BYTES_CAP: usize = 100 * 1024 * 1024;

pub(super) type RequestSender = Arc<dyn Fn(MailRequest) -> bool + Send + Sync>;
pub(super) type PageSender = Arc<dyn Fn(PageRequest) -> Result<(), SubmitError> + Send + Sync>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SenderIdentity {
    pub id: String,
    pub email: String,
    pub name: String,
    pub primary: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThunderbirdAccountState {
    Connected,
    NeedsLogin,
    Unsupported,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThunderbirdAccountOutcome {
    pub profile_path: std::path::PathBuf,
    pub source_account_id: String,
    pub name: String,
    pub email: String,
    pub state: ThunderbirdAccountState,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MessageKey {
    pub account_id: u32,
    pub folder_path: String,
    pub uid: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MessageRow {
    pub key: MessageKey,
    pub message: Message,
}

#[derive(Debug, Clone)]
pub struct OutboxSummary {
    pub id: u32,
    pub recipients: String,
    pub subject: String,
    pub preview: String,
    pub queued_at: i64,
    pub attempts: u32,
    pub last_error: String,
    pub send_at: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SendResult {
    pub account_id: u32,
    pub request_id: u64,
    pub outcome: SendOutcome,
    pub uncertain: bool,
}

#[derive(Debug, Clone)]
pub struct AttachmentSummary {
    pub name: String,
    pub content: Arc<Vec<u8>>,
}

#[derive(Debug, Clone)]
pub enum AttachmentState {
    NotLoaded,
    Loading,
    DownloadRequired,
    Ready {
        items: Arc<Vec<AttachmentSummary>>,
        warning: Option<String>,
    },
    Failed(String),
}

#[derive(Debug, Clone)]
pub enum DraftSourceState {
    Loading,
    Raw(Result<Arc<Vec<u8>>, String>),
    Prepared(Arc<Result<OutgoingMessage, String>>),
}

#[derive(Debug, Clone)]
pub struct DraftSourceSnapshot {
    pub key: MessageKey,
    pub origin: DraftOrigin,
    pub token: u64,
    pub state: DraftSourceState,
}

#[derive(Debug, Clone)]
pub struct MailboxSnapshot {
    pub accounts: Vec<Account>,
    pub current_account_id: Option<u32>,
    pub folders: Vec<Folder>,
    pub current_folder_path: Option<String>,
    pub page: Arc<Vec<Arc<MessageRow>>>,
    pub selected_key: Option<MessageKey>,
    pub selected_message: Option<Arc<MessageRow>>,
    pub attachment_key: Option<MessageKey>,
    pub links: Vec<MailLink>,
    pub attachment_state: AttachmentState,
    pub draft_source: Option<DraftSourceSnapshot>,
    pub search_query: String,
    pub status: String,
    pub loading: bool,
    pub loading_more: bool,
    pub body_loading: bool,
    pub unread_count: u32,
    pub has_more: bool,
    pub display_cap_reached: bool,
    pub pending_send: bool,
    pub send_result: Option<SendResult>,
    pub error: Option<String>,
    pub action_error: Option<String>,
    pub outbox: Vec<OutboxSummary>,
    pub is_demo: bool,
    pub is_thunderbird_account: bool,
    pub sender_identities: Vec<SenderIdentity>,
    pub thunderbird_loading: bool,
    pub thunderbird_error: Option<String>,
    pub thunderbird_notice: Option<String>,
    pub thunderbird_results: Vec<ThunderbirdAccountOutcome>,
}

pub(super) struct WorkerEnvelope {
    pub(super) account_id: u32,
    pub(super) event: WorkerMailboxEvent,
}

pub(super) enum MailboxEvent {
    Worker(WorkerEnvelope),
    Page(PageResult),
    ThunderbirdReady {
        accounts: Vec<crate::thunderbird_adapter::StartedAccount>,
        results: Vec<ThunderbirdAccountOutcome>,
        error: Option<String>,
        notice: Option<String>,
    },
}

pub(super) enum WorkerMailboxEvent {
    Account(Account),
    Folders(Vec<Folder>),
    FolderChanged {
        folder_id: u32,
    },
    Body {
        message_id: u32,
        path: String,
        body: String,
        links: Vec<MailLink>,
        has_attachment: Option<bool>,
        reply_to: Option<String>,
        references: Option<String>,
    },
    DemoMessages {
        folder_id: u32,
        messages: Vec<Message>,
        links_by_uid: HashMap<u32, Vec<MailLink>>,
    },
    Attachments {
        message_id: u32,
        path: String,
        items: Vec<AttachmentSummary>,
        warning: Option<String>,
    },
    AttachmentsPending {
        message_id: u32,
        path: String,
    },
    RawExported {
        token: u64,
        raw: Result<Arc<Vec<u8>>, String>,
    },
    Gone {
        message_id: u32,
        path: String,
        uid: u32,
    },
    FolderUnread {
        folder_id: u32,
        unread: u32,
    },
    FolderUnreadByPath {
        path: String,
        unread: u32,
    },
    FolderSynced {
        folder_id: u32,
    },
    Outbox {
        items: Vec<OutboxSummary>,
    },
    Sent,
    SendFinished {
        request_id: u64,
        outcome: SendOutcome,
    },
    SendUnknown {
        request_id: u64,
        reason: String,
    },
    DraftSaved {
        autosave: Option<u32>,
        message_id: Option<String>,
    },
    Status(String),
    Notice(String),
    Error {
        text: String,
        connectivity: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct BodyRequestSlot {
    account_id: u32,
    folder_path: String,
    message_id: u32,
}

#[derive(Debug, Clone)]
struct PendingBody {
    key: MessageKey,
    message_id: u32,
}

#[derive(Debug, Clone)]
struct PendingAttachment {
    key: MessageKey,
    message_id: u32,
}

enum BodyApplication {
    Ignored,
    Applied,
    Stale,
}

/// Retained, observable UI state for the real mail workers.
pub struct LiveMailbox {
    request_senders: HashMap<u32, RequestSender>,
    page_senders: HashMap<u32, PageSender>,
    thunderbird_accounts: HashSet<u32>,
    sender_identities: HashMap<u32, Vec<SenderIdentity>>,
    event_sender: futures::channel::mpsc::UnboundedSender<MailboxEvent>,
    _event_task: Task<()>,
    query_service: Option<QueryService>,
    query_start_error: Option<String>,
    query_generation: u64,
    pending_page_generation: Option<u64>,
    pending_page_append: bool,
    next_cursor: Option<MessageCursor>,
    page_rows: Arc<Vec<Arc<MessageRow>>>,
    accounts: Vec<Account>,
    folders_by_account: HashMap<u32, Vec<Folder>>,
    selected_folder_by_account: HashMap<u32, String>,
    current_account_id: Option<u32>,
    current_folder_path: Option<String>,
    messages: Vec<Message>,
    visible_indices: Vec<usize>,
    truncated: bool,
    selected_key: Option<MessageKey>,
    selected_row: Option<Arc<MessageRow>>,
    links: Vec<MailLink>,
    demo_links_by_key: HashMap<MessageKey, Vec<MailLink>>,
    attachment_state: AttachmentState,
    pending_attachment: Option<PendingAttachment>,
    draft_source: Option<DraftSourceSnapshot>,
    next_draft_token: u64,
    pending_bodies: HashMap<BodyRequestSlot, PendingBody>,
    pending_sends: HashMap<u32, HashSet<u64>>,
    send_results: HashMap<u32, SendResult>,
    next_send_id: u64,
    outbox_by_account: HashMap<u32, Vec<OutboxSummary>>,
    search_query: String,
    status: String,
    loading: bool,
    loading_more: bool,
    body_loading: bool,
    connectivity_errors: HashMap<u32, String>,
    query_errors: HashMap<u32, String>,
    action_errors: HashMap<u32, String>,
    thunderbird_loading: bool,
    thunderbird_restore_started: bool,
    thunderbird_error: Option<String>,
    thunderbird_notice: Option<String>,
    thunderbird_results: Vec<ThunderbirdAccountOutcome>,
    demo_mode: bool,
}

impl LiveMailbox {
    /// Start one worker for each saved `(stable account id, config)` pair.
    pub fn new(
        profiles: Vec<(u32, AccountConfig)>,
        initial_account_id: Option<u32>,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::new_with_demo_mode(profiles, initial_account_id, false, cx)
    }

    fn new_with_demo_mode(
        profiles: Vec<(u32, AccountConfig)>,
        initial_account_id: Option<u32>,
        demo_mode: bool,
        cx: &mut Context<Self>,
    ) -> Self {
        let (event_tx, mut event_rx) = futures::channel::mpsc::unbounded::<MailboxEvent>();
        let (query_service, query_start_error) = if demo_mode {
            (None, None)
        } else {
            match QueryService::start() {
                Ok(query_service) => (Some(query_service), None),
                Err(error) => (None, Some(error.to_string())),
            }
        };
        let mut request_senders = HashMap::new();
        let mut accounts = Vec::with_capacity(profiles.len());
        let mut account_ids = Vec::with_capacity(profiles.len());

        for (account_id, config) in profiles {
            if account_ids.contains(&account_id) {
                continue;
            }
            account_ids.push(account_id);
            accounts.push(Account {
                id: account_id,
                name: config.name.clone(),
                email: config.email.clone(),
                label: config.email.clone(),
                accent: default_accent(account_id).to_owned(),
            });

            let sender = spawn_account_worker(account_id, config, event_tx.clone());
            let _ = sender(MailRequest::LoadOutbox);
            request_senders.insert(account_id, sender);
        }

        let current_account_id = initial_account_id
            .filter(|id| account_ids.contains(id))
            .or_else(|| account_ids.first().copied());
        let event_task = cx.spawn(async move |this, cx| {
            while let Some(event) = event_rx.next().await {
                if this
                    .update(cx, |mailbox, cx| mailbox.handle_event(event, cx))
                    .is_err()
                {
                    break;
                }
            }
        });

        Self {
            request_senders,
            page_senders: HashMap::new(),
            thunderbird_accounts: HashSet::new(),
            sender_identities: HashMap::new(),
            event_sender: event_tx,
            _event_task: event_task,
            query_service,
            query_start_error,
            query_generation: 0,
            pending_page_generation: None,
            pending_page_append: false,
            next_cursor: None,
            page_rows: Arc::new(Vec::new()),
            accounts,
            folders_by_account: HashMap::new(),
            selected_folder_by_account: HashMap::new(),
            current_account_id,
            current_folder_path: None,
            messages: Vec::new(),
            visible_indices: Vec::new(),
            truncated: false,
            selected_key: None,
            selected_row: None,
            links: Vec::new(),
            demo_links_by_key: HashMap::new(),
            attachment_state: AttachmentState::NotLoaded,
            pending_attachment: None,
            draft_source: None,
            next_draft_token: 1,
            pending_bodies: HashMap::new(),
            pending_sends: HashMap::new(),
            send_results: HashMap::new(),
            next_send_id: 1,
            outbox_by_account: HashMap::new(),
            search_query: String::new(),
            status: String::new(),
            loading: !account_ids.is_empty(),
            loading_more: false,
            body_loading: false,
            connectivity_errors: HashMap::new(),
            query_errors: HashMap::new(),
            action_errors: HashMap::new(),
            thunderbird_loading: false,
            thunderbird_restore_started: false,
            thunderbird_error: None,
            thunderbird_notice: None,
            thunderbird_results: Vec::new(),
            demo_mode,
        }
    }

    /// Start a read-only local fixture mailbox for visual QA without saved
    /// profiles or network credentials.
    pub fn new_demo(cx: &mut Context<Self>) -> Self {
        let mut mailbox = Self::new_with_demo_mode(Vec::new(), Some(1), true, cx);
        mailbox.current_account_id = Some(1);
        mailbox.loading = true;
        let sender = spawn_demo_worker(1, mailbox.event_sender.clone());
        let _ = sender(MailRequest::LoadOutbox);
        mailbox.request_senders.insert(1, sender);
        mailbox
    }

    pub fn snapshot(&self) -> MailboxSnapshot {
        let folders = self
            .current_account_id
            .and_then(|id| self.folders_by_account.get(&id))
            .cloned()
            .unwrap_or_default();
        let current_folder_path = self.current_folder_path.clone();
        let selected_message = self.selected_row.clone();
        let current_account_id = self.current_account_id;
        let error = current_account_id
            .and_then(|id| self.query_errors.get(&id).cloned())
            .or_else(|| {
                current_account_id.and_then(|id| self.connectivity_errors.get(&id).cloned())
            })
            .or_else(|| self.query_start_error.clone());
        let action_error = current_account_id.and_then(|id| self.action_errors.get(&id).cloned());
        let outbox = current_account_id
            .and_then(|id| self.outbox_by_account.get(&id))
            .into_iter()
            .flatten()
            .cloned()
            .collect();
        let has_more = self.next_cursor.is_some();
        let unread_count = current_folder_path
            .as_ref()
            .and_then(|path| folders.iter().find(|folder| &folder.path == path))
            .map(|folder| folder.unread)
            .unwrap_or(0);

        MailboxSnapshot {
            accounts: self.accounts.clone(),
            current_account_id,
            folders,
            current_folder_path,
            page: self.page_rows.clone(),
            selected_key: self.selected_key.clone(),
            selected_message,
            attachment_key: self.selected_key.clone(),
            links: self.links.clone(),
            attachment_state: self.attachment_state.clone(),
            draft_source: self.draft_source.clone(),
            search_query: self.search_query.clone(),
            status: self.status.clone(),
            loading: self.loading,
            loading_more: self.loading_more,
            body_loading: self.body_loading,
            unread_count,
            has_more,
            display_cap_reached: self.truncated,
            pending_send: current_account_id
                .and_then(|id| self.pending_sends.get(&id))
                .is_some_and(|pending| !pending.is_empty()),
            send_result: current_account_id.and_then(|id| self.send_results.get(&id).copied()),
            error,
            action_error,
            outbox,
            is_demo: self.demo_mode,
            is_thunderbird_account: current_account_id
                .is_some_and(|id| self.thunderbird_accounts.contains(&id)),
            sender_identities: current_account_id
                .and_then(|id| self.sender_identities.get(&id).cloned())
                .unwrap_or_default(),
            thunderbird_loading: self.thunderbird_loading,
            thunderbird_error: self.thunderbird_error.clone(),
            thunderbird_notice: self.thunderbird_notice.clone(),
            thunderbird_results: self.thunderbird_results.clone(),
        }
    }

    /// Begin restoring previously imported Thunderbird accounts once the
    /// native account list is settled. This is deliberately separate from
    /// `new`: startup replaces the initial empty mailbox after reading config.
    pub fn restore_thunderbird(&mut self, cx: &mut Context<Self>) -> bool {
        if self.demo_mode || self.thunderbird_restore_started || self.thunderbird_loading {
            return false;
        }
        self.thunderbird_restore_started = true;
        self.thunderbird_loading = true;
        self.thunderbird_error = None;
        self.thunderbird_notice = None;
        let event_sender = self.event_sender.clone();
        let reserved_account_ids = self.accounts.iter().map(|account| account.id).collect();
        let existing_emails = self
            .accounts
            .iter()
            .map(|account| account.email.clone())
            .collect();
        std::thread::Builder::new()
            .name("megamail-thunderbird-restore".into())
            .spawn(move || {
                crate::thunderbird_adapter::restore(
                    event_sender,
                    reserved_account_ids,
                    existing_emails,
                )
            })
            .map_err(|error| {
                self.thunderbird_loading = false;
                self.thunderbird_error = Some(format!("Could not restore Thunderbird: {error}"));
            })
            .ok();
        cx.notify();
        true
    }

    /// Connect the selected mail accounts from Thunderbird's authenticated
    /// profile runtime and persist their source paths and stable app IDs.
    pub fn import_thunderbird(
        &mut self,
        selections: Vec<(ThunderbirdProfile, Vec<String>)>,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        if self.demo_mode {
            return Err("Thunderbird import is unavailable in the local demo.".into());
        }
        if self.thunderbird_loading {
            return Err("Thunderbird is already connecting.".into());
        }
        if selections.is_empty() || selections.iter().all(|(_, ids)| ids.is_empty()) {
            return Err("Select at least one Thunderbird account.".into());
        }

        let reserved_account_ids = self.accounts.iter().map(|account| account.id).collect();
        let running_thunderbird_ids = self.thunderbird_accounts.clone();
        let existing_emails = self
            .accounts
            .iter()
            .map(|account| account.email.clone())
            .collect();
        self.thunderbird_loading = true;
        self.thunderbird_error = None;
        self.thunderbird_notice = None;
        self.thunderbird_results.clear();
        let event_sender = self.event_sender.clone();
        if let Err(error) = std::thread::Builder::new()
            .name("megamail-thunderbird-import".into())
            .spawn(move || {
                crate::thunderbird_adapter::import(
                    selections,
                    reserved_account_ids,
                    running_thunderbird_ids,
                    existing_emails,
                    event_sender,
                )
            })
        {
            self.thunderbird_loading = false;
            self.thunderbird_error = Some(format!("Could not start Thunderbird: {error}"));
            cx.notify();
            return Err(error.to_string());
        }
        cx.notify();
        Ok(())
    }

    /// Thunderbird accounts can have different From identities and SMTP
    /// routes; the authenticated extension runtime is the source of truth.
    pub fn sender_identities(&self, account_id: u32) -> Vec<SenderIdentity> {
        self.sender_identities
            .get(&account_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Add one newly persisted profile without replacing the live mailbox.
    /// Existing workers, pending sends, and the retained event loop stay intact.
    pub fn add_profile(
        &mut self,
        account_id: u32,
        config: AccountConfig,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        if self.request_senders.contains_key(&account_id) {
            return Err("This account is already connected.".into());
        }
        if self.accounts.iter().any(|account| {
            account
                .email
                .trim()
                .eq_ignore_ascii_case(config.email.trim())
        }) {
            return Err("An account with this email address is already connected.".into());
        }

        let sender = spawn_account_worker(account_id, config.clone(), self.event_sender.clone());
        if !sender(MailRequest::LoadOutbox) {
            return Err("The new account worker could not be started.".into());
        }
        self.accounts.push(Account {
            id: account_id,
            name: config.name.clone(),
            email: config.email.clone(),
            label: config.email.clone(),
            accent: default_accent(account_id).to_owned(),
        });
        self.request_senders.insert(account_id, sender);
        if self.current_account_id.is_none() {
            self.current_account_id = Some(account_id);
            self.loading = true;
        }
        cx.notify();
        Ok(())
    }

    pub fn switch_account(&mut self, account_id: u32, cx: &mut Context<Self>) -> bool {
        if !self.request_senders.contains_key(&account_id)
            || self.current_account_id == Some(account_id)
        {
            return false;
        }
        self.current_account_id = Some(account_id);
        self.current_folder_path = self
            .folders_by_account
            .get(&account_id)
            .and_then(|folders| {
                self.selected_folder_by_account
                    .get(&account_id)
                    .and_then(|path| folders.iter().find(|folder| &folder.path == path))
                    .or_else(|| {
                        folders
                            .iter()
                            .find(|folder| folder.kind == FolderKind::Inbox)
                    })
                    .or_else(|| folders.first())
                    .map(|folder| folder.path.clone())
            });
        self.clear_current_page();
        self.loading = self.current_folder_path.is_some()
            || !self.folders_by_account.contains_key(&account_id);
        if let Some(path) = self.current_folder_path.clone() {
            self.request_page(None, false, cx);
            self.request_current_folder(&path, cx);
        }
        cx.notify();
        true
    }

    pub fn select_folder(&mut self, path: impl Into<String>, cx: &mut Context<Self>) -> bool {
        let path = path.into();
        let Some(account_id) = self.current_account_id else {
            return false;
        };
        let Some(folder) = self
            .folders_by_account
            .get(&account_id)
            .and_then(|folders| folders.iter().find(|folder| folder.path == path))
        else {
            return false;
        };
        if self.current_folder_path.as_deref() == Some(path.as_str()) {
            return false;
        }
        let folder_id = folder.id;
        self.current_folder_path = Some(path.clone());
        self.selected_folder_by_account
            .insert(account_id, path.clone());
        self.clear_current_page();
        self.loading = true;
        self.request_page(None, false, cx);
        self.send_request(
            account_id,
            MailRequest::LoadMessages {
                folder_id,
                path: path.clone(),
            },
            cx,
        );
        cx.notify();
        true
    }

    pub fn select_message(&mut self, key: MessageKey, cx: &mut Context<Self>) -> bool {
        if !self
            .messages
            .iter()
            .any(|message| self.key_for(message).as_ref() == Some(&key))
        {
            return false;
        }
        if self.selected_key.as_ref() == Some(&key) {
            let needs_body = self
                .selected_row
                .as_ref()
                .filter(|row| row.key == key)
                .is_none_or(|row| row.message.body.is_empty());
            if needs_body && !self.body_loading {
                self.request_selected_body(cx);
                cx.notify();
            }
            return needs_body;
        }
        self.selected_key = Some(key.clone());
        self.body_loading = false;
        self.links = self
            .demo_links_by_key
            .get(&key)
            .cloned()
            .unwrap_or_default();
        self.attachment_state = AttachmentState::NotLoaded;
        self.pending_attachment = None;
        self.draft_source = None;
        self.refresh_selected_row();
        self.request_selected_body(cx);
        cx.notify();
        true
    }

    pub fn set_search(&mut self, query: impl Into<String>, cx: &mut Context<Self>) {
        let query = query.into();
        self.search_query = if self.demo_mode {
            query.chars().take(256).collect()
        } else {
            query
        };
        if !self.demo_mode {
            self.clear_current_page();
            self.loading = self.current_folder_path.is_some();
        }
        self.request_page(None, false, cx);
        cx.notify();
    }

    pub fn load_more(&mut self, cx: &mut Context<Self>) {
        if self.loading_more || self.loading || self.pending_page_generation.is_some() {
            return;
        }
        if self.messages.len() >= MESSAGE_CAP && self.next_cursor.is_some() {
            self.set_action_error(
                "Showing the newest 5,000 messages. Older messages are not loaded yet.",
            );
            cx.notify();
        } else if let Some(cursor) = self.next_cursor {
            self.request_page(Some(cursor), true, cx);
        }
    }

    pub fn refresh(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(path) = self.current_folder_path.clone() else {
            return false;
        };
        let Some(account_id) = self.current_account_id else {
            return false;
        };
        let Some(folder_id) = self.folder_id(account_id, &path) else {
            return false;
        };
        self.loading = true;
        self.request_page(None, false, cx);
        let sent = self.send_request(
            account_id,
            MailRequest::LoadMessages {
                folder_id,
                path: path.clone(),
            },
            cx,
        );
        self.send_request(account_id, MailRequest::RefreshUnread, cx);
        cx.notify();
        sent
    }

    /// Queue an archive move. The row remains visible until a worker refresh confirms it.
    pub fn archive_selected(&mut self, cx: &mut Context<Self>) -> bool {
        self.move_selected_to(FolderKind::Archive, cx)
    }

    /// Queue a restore to Inbox. The row remains visible until a worker refresh confirms it.
    pub fn restore_selected(&mut self, cx: &mut Context<Self>) -> bool {
        self.move_selected_to(FolderKind::Inbox, cx)
    }

    pub fn send_message(
        &mut self,
        message: OutgoingMessage,
        cx: &mut Context<Self>,
    ) -> Option<u64> {
        if self.demo_mode {
            self.set_action_error("Sending is disabled in the local demo.");
            cx.notify();
            return None;
        }
        let account_id = message.from_account_id;
        let request_id = self.next_send_id;
        self.next_send_id = self.next_send_id.wrapping_add(1).max(1);
        let sent_path = self
            .folders_by_account
            .get(&account_id)
            .and_then(|folders| {
                folders
                    .iter()
                    .find(|folder| folder.kind == FolderKind::Sent)
            })
            .map(|folder| folder.path.clone());
        if !self.send_request(
            account_id,
            MailRequest::Send {
                message: Box::new(message),
                sent_path,
                request_id: Some(request_id),
            },
            cx,
        ) {
            return None;
        }
        self.pending_sends
            .entry(account_id)
            .or_default()
            .insert(request_id);
        self.send_results.remove(&account_id);
        self.action_errors.remove(&account_id);
        if self.current_account_id == Some(account_id) {
            self.status = "Sending…".into();
        }
        cx.notify();
        Some(request_id)
    }

    pub fn save_draft(
        &mut self,
        message: OutgoingMessage,
        autosave: Option<u32>,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.demo_mode {
            self.set_action_error("Drafts are disabled in the local demo.");
            cx.notify();
            return false;
        }
        let account_id = message.from_account_id;
        let Some(folder) = self
            .folders_by_account
            .get(&account_id)
            .and_then(|folders| {
                folders
                    .iter()
                    .find(|folder| folder.kind == FolderKind::Drafts)
            })
        else {
            self.set_action_error("This account has no Drafts folder.");
            cx.notify();
            return false;
        };
        let request = MailRequest::SaveDraft {
            message: Box::new(message),
            folder_id: folder.id,
            path: folder.path.clone(),
            autosave,
            template: false,
        };
        let sent = self.send_request(account_id, request, cx);
        if autosave.is_none() && self.current_account_id == Some(account_id) {
            if sent {
                self.status = "Saving draft…".into();
                self.action_errors.remove(&account_id);
            } else {
                self.status.clear();
            }
        }
        cx.notify();
        sent
    }

    pub fn flush_outbox(&mut self, id: Option<u32>, cx: &mut Context<Self>) -> bool {
        if self.demo_mode {
            self.set_action_error("Outbox actions are disabled in the local demo.");
            cx.notify();
            return false;
        }
        let Some(account_id) = self.current_account_id else {
            return false;
        };
        let sent = self.send_request(account_id, MailRequest::FlushOutbox { id }, cx);
        if sent {
            self.status = "Retrying queued mail…".into();
        }
        cx.notify();
        sent
    }

    pub fn delete_outbox(&mut self, id: u32, cx: &mut Context<Self>) -> bool {
        if self.demo_mode {
            self.set_action_error("Outbox actions are disabled in the local demo.");
            cx.notify();
            return false;
        }
        let Some(account_id) = self.current_account_id else {
            return false;
        };
        if !self
            .outbox_by_account
            .get(&account_id)
            .is_some_and(|items| items.iter().any(|item| item.id == id))
        {
            return false;
        }
        self.send_request(account_id, MailRequest::DeleteOutbox { id }, cx)
    }

    /// Show cached files first; `download = true` is the explicit network fetch.
    /// A response is applied only while the same account, folder, UID and
    /// selected message remain active.
    pub fn request_attachments(
        &mut self,
        key: MessageKey,
        download: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.selected_key.as_ref() != Some(&key)
            || self.current_account_id != Some(key.account_id)
            || self.current_folder_path.as_deref() != Some(key.folder_path.as_str())
        {
            return false;
        }
        let Some(message) = self
            .messages
            .iter()
            .find(|message| message.account_id == key.account_id && message.uid == key.uid)
        else {
            return false;
        };
        let message_id = message.id;
        if self
            .pending_attachment
            .as_ref()
            .is_some_and(|pending| pending.key == key)
        {
            return false;
        }
        self.pending_attachment = Some(PendingAttachment {
            key: key.clone(),
            message_id,
        });
        self.attachment_state = AttachmentState::Loading;
        self.action_errors.remove(&key.account_id);
        let sent = self.send_request(
            key.account_id,
            MailRequest::LoadAttachments {
                message_id,
                path: key.folder_path,
                uid: key.uid,
                download,
            },
            cx,
        );
        if !sent {
            self.pending_attachment = None;
            self.attachment_state =
                AttachmentState::Failed("The mail worker is unavailable.".into());
        }
        cx.notify();
        sent
    }

    /// Export the selected draft's original MIME bytes before opening it for
    /// editing; the visible row body omits Bcc and attachment data.
    pub fn request_edit_draft(&mut self, key: MessageKey, cx: &mut Context<Self>) -> Option<u64> {
        if self.selected_key.as_ref() != Some(&key)
            || self.current_account_id != Some(key.account_id)
            || self.current_folder_path.as_deref() != Some(key.folder_path.as_str())
            || self.draft_source.is_some()
        {
            return None;
        }
        let Some(folder) = self
            .folders_by_account
            .get(&key.account_id)
            .and_then(|folders| folders.iter().find(|folder| folder.path == key.folder_path))
            .filter(|folder| folder.kind == FolderKind::Drafts)
            .cloned()
        else {
            return None;
        };
        let Some(message) = self
            .messages
            .iter()
            .find(|message| message.account_id == key.account_id && message.uid == key.uid)
        else {
            return None;
        };
        if message.folder_id != folder.id {
            return None;
        }
        let uid = message.uid;
        let origin = DraftOrigin {
            account_id: key.account_id,
            folder_id: folder.id,
            path: key.folder_path.clone(),
            uid: key.uid,
        };
        let token = self.next_draft_token;
        self.next_draft_token = self.next_draft_token.wrapping_add(1).max(1);
        self.draft_source = Some(DraftSourceSnapshot {
            key: key.clone(),
            origin,
            token,
            state: DraftSourceState::Loading,
        });
        self.action_errors.remove(&key.account_id);
        let sent = self.send_request(
            key.account_id,
            MailRequest::ExportRaw {
                token,
                path: key.folder_path.clone(),
                uid,
                for_reader: true,
                max_bytes: Some(DRAFT_RAW_BYTES_CAP as u64),
            },
            cx,
        );
        if !sent {
            self.draft_source = None;
            cx.notify();
            return None;
        }
        cx.notify();
        Some(token)
    }

    /// Publish a MIME-parsed, attachment-staged message produced on a
    /// background executor. A stale conversion is rejected so the caller can
    /// remove any temporary attachment files it created.
    pub fn publish_prepared_draft(
        &mut self,
        key: MessageKey,
        token: u64,
        prepared: Arc<Result<OutgoingMessage, String>>,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(source) = self.draft_source.as_mut() else {
            return false;
        };
        let prepared_matches_origin = match prepared.as_ref() {
            Ok(message) => {
                message.from_account_id == key.account_id
                    && message.draft_origin.as_ref().is_some_and(|origin| {
                        origin.account_id == key.account_id
                            && origin.folder_id == source.origin.folder_id
                            && origin.path == key.folder_path
                            && origin.uid == key.uid
                    })
            }
            Err(_) => true,
        };
        if source.key != key
            || source.token != token
            || !matches!(&source.state, DraftSourceState::Raw(Ok(_)))
            || !prepared_matches_origin
            || self.selected_key.as_ref() != Some(&key)
            || self.current_account_id != Some(key.account_id)
            || self.current_folder_path.as_deref() != Some(key.folder_path.as_str())
        {
            return false;
        }
        source.state = DraftSourceState::Prepared(prepared);
        cx.notify();
        true
    }

    /// Clear the draft result after the native compose shell consumes it.
    pub fn dismiss_draft_source(
        &mut self,
        key: &MessageKey,
        token: u64,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self
            .draft_source
            .as_ref()
            .is_some_and(|source| source.key == *key && source.token == token)
        {
            return false;
        }
        self.draft_source = None;
        cx.notify();
        true
    }

    pub fn dismiss_error(&mut self, cx: &mut Context<Self>) {
        if let Some(account_id) = self.current_account_id {
            self.connectivity_errors.remove(&account_id);
            self.query_errors.remove(&account_id);
            self.action_errors.remove(&account_id);
            cx.notify();
        }
    }

    fn move_selected_to(&mut self, kind: FolderKind, cx: &mut Context<Self>) -> bool {
        if self.demo_mode {
            self.set_action_error("Message actions are disabled in the local demo.");
            cx.notify();
            return false;
        }
        let Some(key) = self.selected_key.clone() else {
            return false;
        };
        let Some(account_id) = self.current_account_id else {
            return false;
        };
        if key.account_id != account_id
            || self.current_folder_path.as_deref() != Some(&key.folder_path)
        {
            return false;
        }
        let Some(destination) = self
            .folders_by_account
            .get(&account_id)
            .and_then(|folders| folders.iter().find(|folder| folder.kind == kind))
            .map(|folder| folder.path.clone())
        else {
            let label = if kind == FolderKind::Archive {
                "Archive"
            } else {
                "Inbox"
            };
            self.set_action_error(format!("This account has no {label} folder."));
            cx.notify();
            return false;
        };
        let Some(folder_id) = self.folder_id(account_id, &key.folder_path) else {
            return false;
        };
        let moved = self.send_request(
            account_id,
            MailRequest::MoveMessage {
                path: key.folder_path.clone(),
                uid: key.uid,
                dest: destination,
            },
            cx,
        );
        if moved {
            self.loading = true;
            self.status = "Moving message…".into();
            // Requests from this account are FIFO: this list reflects the completed move or its failure.
            self.send_request(
                account_id,
                MailRequest::LoadMessages {
                    folder_id,
                    path: key.folder_path,
                },
                cx,
            );
        }
        cx.notify();
        moved
    }

    fn request_current_folder(&mut self, path: &str, cx: &mut Context<Self>) {
        let Some(account_id) = self.current_account_id else {
            return;
        };
        self.request_current_folder_for(account_id, path, cx);
    }

    fn request_current_folder_for(&mut self, account_id: u32, path: &str, cx: &mut Context<Self>) {
        let Some(folder_id) = self.folder_id(account_id, path) else {
            return;
        };
        self.send_request(
            account_id,
            MailRequest::LoadMessages {
                folder_id,
                path: path.to_owned(),
            },
            cx,
        );
    }

    fn send_request(
        &mut self,
        account_id: u32,
        request: MailRequest,
        cx: &mut Context<Self>,
    ) -> bool {
        let sent = self
            .request_senders
            .get(&account_id)
            .is_some_and(|sender| sender(request));
        if !sent {
            self.action_errors
                .insert(account_id, "The mail worker is unavailable.".into());
            if self.current_account_id == Some(account_id) {
                cx.notify();
            }
        }
        sent
    }

    fn handle_event(&mut self, event: MailboxEvent, cx: &mut Context<Self>) {
        match event {
            MailboxEvent::Worker(envelope) => self.handle_worker_event(envelope, cx),
            MailboxEvent::Page(result) => self.handle_page_result(result, cx),
            MailboxEvent::ThunderbirdReady {
                accounts,
                results,
                error,
                notice,
            } => {
                self.thunderbird_loading = false;
                self.thunderbird_results = results;
                self.thunderbird_error = error;
                self.thunderbird_notice = notice;
                for started in accounts {
                    if self.request_senders.contains_key(&started.account.id)
                        || self.accounts.iter().any(|account| {
                            account.email.eq_ignore_ascii_case(&started.account.email)
                        })
                    {
                        self.thunderbird_error.get_or_insert_with(|| {
                            format!("{} is already connected.", started.account.email)
                        });
                        continue;
                    }
                    let account_id = started.account.id;
                    self.accounts.push(started.account.clone());
                    self.request_senders
                        .insert(account_id, started.request_sender);
                    self.page_senders.insert(account_id, started.page_sender);
                    self.thunderbird_accounts.insert(account_id);
                    self.sender_identities
                        .insert(account_id, started.identities);
                    self.folders_by_account
                        .insert(account_id, started.folders.clone());
                    self.selected_folder_by_account
                        .entry(account_id)
                        .or_insert_with(|| {
                            started
                                .folders
                                .iter()
                                .find(|folder| folder.kind == FolderKind::Inbox)
                                .or_else(|| started.folders.first())
                                .map(|folder| folder.path.clone())
                                .unwrap_or_default()
                        });
                    if let Some(error) = started.error {
                        self.connectivity_errors.insert(account_id, error);
                    }
                    if self.current_account_id.is_none() {
                        self.current_account_id = Some(account_id);
                        self.current_folder_path = self
                            .selected_folder_by_account
                            .get(&account_id)
                            .cloned()
                            .filter(|path| !path.is_empty());
                        self.loading = self.current_folder_path.is_some();
                        self.clear_current_page();
                    }
                    if let Some(path) = self
                        .selected_folder_by_account
                        .get(&account_id)
                        .filter(|path| !path.is_empty())
                        .cloned()
                    {
                        self.request_current_folder_for(account_id, &path, cx);
                    }
                }
                if let Some(account_id) = self.current_account_id {
                    if self.current_folder_path.is_some() && self.messages.is_empty() {
                        self.request_page(None, false, cx);
                        self.loading = true;
                    }
                }
                cx.notify();
            }
        }
    }

    fn handle_worker_event(&mut self, envelope: WorkerEnvelope, cx: &mut Context<Self>) {
        let account_id = envelope.account_id;
        let mut changed = false;
        match envelope.event {
            WorkerMailboxEvent::Account(account) if account.id == account_id => {
                if let Some(existing) = self
                    .accounts
                    .iter_mut()
                    .find(|existing| existing.id == account_id)
                {
                    *existing = account;
                } else {
                    self.accounts.push(account);
                }
                changed = true;
            }
            WorkerMailboxEvent::Folders(folders) => {
                let folders: Vec<_> = folders
                    .into_iter()
                    .filter(|folder| folder.account_id == account_id)
                    .collect();
                let old_folder_id = self.current_folder_path.as_ref().and_then(|path| {
                    self.folders_by_account
                        .get(&account_id)
                        .and_then(|old| old.iter().find(|folder| &folder.path == path))
                        .map(|folder| folder.id)
                });
                self.folders_by_account.insert(account_id, folders.clone());
                if self.current_account_id == Some(account_id) {
                    let preferred = self
                        .current_folder_path
                        .as_ref()
                        .filter(|path| folders.iter().any(|folder| &folder.path == *path))
                        .cloned()
                        .or_else(|| {
                            folders
                                .iter()
                                .find(|folder| folder.kind == FolderKind::Inbox)
                                .map(|folder| folder.path.clone())
                        })
                        .or_else(|| folders.first().map(|folder| folder.path.clone()));
                    let path_changed = preferred != self.current_folder_path;
                    self.current_folder_path = preferred.clone();
                    if let Some(path) = preferred {
                        self.selected_folder_by_account
                            .insert(account_id, path.clone());
                        let new_folder_id = folders
                            .iter()
                            .find(|folder| folder.path == path)
                            .map(|folder| folder.id);
                        let folder_id_changed = old_folder_id != new_folder_id;
                        if path_changed || folder_id_changed || self.messages.is_empty() {
                            self.clear_current_page();
                            self.loading = true;
                            self.request_page(None, false, cx);
                            self.request_current_folder(&path, cx);
                        }
                    } else {
                        self.clear_current_page();
                        self.loading = false;
                    }
                    self.connectivity_errors.remove(&account_id);
                }
                changed = true;
            }
            WorkerMailboxEvent::FolderChanged { folder_id } => {
                if let Some(path) = self.path_for_folder(account_id, folder_id) {
                    if self.is_current_folder(account_id, &path) {
                        self.request_page(None, false, cx);
                        self.status.clear();
                        self.connectivity_errors.remove(&account_id);
                        changed = true;
                    }
                }
            }
            WorkerMailboxEvent::Body {
                message_id,
                path,
                body,
                links,
                has_attachment,
                reply_to,
                references,
            } => {
                match self.apply_body(
                    account_id,
                    &path,
                    message_id,
                    body,
                    links,
                    has_attachment,
                    reply_to,
                    references,
                ) {
                    BodyApplication::Ignored => {}
                    BodyApplication::Applied => {
                        self.body_loading = false;
                        changed = true;
                    }
                    BodyApplication::Stale => {
                        self.body_loading = false;
                        self.request_selected_body(cx);
                        changed = true;
                    }
                }
            }
            WorkerMailboxEvent::DemoMessages {
                folder_id,
                messages,
                links_by_uid,
            } => {
                if self.demo_mode
                    && self.current_account_id == Some(account_id)
                    && self
                        .current_folder_path
                        .as_deref()
                        .and_then(|path| self.folder_id(account_id, path))
                        == Some(folder_id)
                {
                    self.demo_links_by_key.retain(|key, _| {
                        key.account_id != account_id
                            || Some(key.folder_path.as_str()) != self.current_folder_path.as_deref()
                    });
                    let path = self.current_folder_path.as_deref().unwrap_or_default();
                    self.demo_links_by_key.extend(
                        links_by_uid
                            .into_iter()
                            .map(|(uid, links)| (message_key(account_id, path, uid), links)),
                    );
                    let (messages, _) =
                        merge_message_batch(&[], messages, account_id, folder_id, false);
                    self.messages = messages;
                    self.loading = false;
                    self.rebuild_visible();
                    self.ensure_visible_selection(cx);
                    self.links = self
                        .selected_key
                        .as_ref()
                        .and_then(|key| self.demo_links_by_key.get(key))
                        .cloned()
                        .unwrap_or_default();
                    changed = true;
                }
            }
            WorkerMailboxEvent::Attachments {
                message_id,
                path,
                items,
                warning,
            } => {
                if self.attachment_event_matches(account_id, &path, message_id) {
                    self.pending_attachment = None;
                    self.attachment_state = AttachmentState::Ready {
                        items: Arc::new(items),
                        warning,
                    };
                    changed = true;
                }
            }
            WorkerMailboxEvent::AttachmentsPending { message_id, path } => {
                if self.attachment_event_matches(account_id, &path, message_id) {
                    self.pending_attachment = None;
                    self.attachment_state = AttachmentState::DownloadRequired;
                    changed = true;
                }
            }
            WorkerMailboxEvent::RawExported { token, raw } => {
                let matches = self.draft_source.as_ref().is_some_and(|source| {
                    draft_export_matches(
                        account_id,
                        token,
                        self.current_account_id,
                        self.current_folder_path.as_deref(),
                        self.selected_key.as_ref(),
                        source,
                        self.messages.iter().any(|message| {
                            message.account_id == source.key.account_id
                                && message.uid == source.key.uid
                        }),
                    )
                });
                if matches {
                    if let Some(source) = self.draft_source.as_mut() {
                        source.state = DraftSourceState::Raw(raw);
                        changed = true;
                    }
                }
            }
            WorkerMailboxEvent::Gone {
                message_id,
                path,
                uid,
            } => {
                if self.is_current_folder(account_id, &path) {
                    self.messages.retain(|message| {
                        !(message.account_id == account_id
                            && message.id == message_id
                            && message.uid == uid)
                    });
                    self.rebuild_visible();
                    self.ensure_visible_selection(cx);
                    self.request_page(None, false, cx);
                    changed = true;
                }
            }
            WorkerMailboxEvent::FolderUnread { folder_id, unread } => {
                if let Some(folder) = self
                    .folders_by_account
                    .get_mut(&account_id)
                    .and_then(|folders| folders.iter_mut().find(|folder| folder.id == folder_id))
                {
                    folder.unread = unread;
                    changed = true;
                }
            }
            WorkerMailboxEvent::FolderUnreadByPath { path, unread } => {
                changed |=
                    set_folder_unread(&mut self.folders_by_account, account_id, &path, unread);
            }
            WorkerMailboxEvent::FolderSynced { folder_id } => {
                if self
                    .path_for_folder(account_id, folder_id)
                    .is_some_and(|path| self.is_current_folder(account_id, &path))
                {
                    self.loading = false;
                    self.status.clear();
                    self.connectivity_errors.remove(&account_id);
                    self.request_page(None, false, cx);
                    changed = true;
                }
            }
            WorkerMailboxEvent::Outbox { items } => {
                self.outbox_by_account.insert(account_id, items);
                changed = true;
            }
            WorkerMailboxEvent::SendFinished {
                request_id,
                outcome,
            } => {
                if self
                    .pending_sends
                    .get_mut(&account_id)
                    .is_some_and(|sends| sends.remove(&request_id))
                {
                    self.send_results.insert(
                        account_id,
                        SendResult {
                            account_id,
                            request_id,
                            outcome,
                            uncertain: false,
                        },
                    );
                    if self.current_account_id == Some(account_id) {
                        self.status = match outcome {
                            SendOutcome::Sent => "Sent".into(),
                            SendOutcome::Queued
                                if self.thunderbird_accounts.contains(&account_id) =>
                            {
                                "Thunderbird queued this message. Manage it in Thunderbird’s Outbox."
                                    .into()
                            }
                            SendOutcome::Queued => {
                                "Saved to Outbox. Retry or discard it from MegaMail.".into()
                            }
                            SendOutcome::Failed => "Send failed.".into(),
                        };
                    }
                    changed = true;
                }
            }
            WorkerMailboxEvent::SendUnknown { request_id, reason } => {
                if self
                    .pending_sends
                    .get_mut(&account_id)
                    .is_some_and(|sends| sends.remove(&request_id))
                {
                    self.send_results.insert(
                        account_id,
                        SendResult {
                            account_id,
                            request_id,
                            outcome: SendOutcome::Failed,
                            uncertain: true,
                        },
                    );
                    if self.current_account_id == Some(account_id) {
                        self.status = reason;
                        changed = true;
                    }
                }
            }
            WorkerMailboxEvent::Sent => {
                if self.current_account_id == Some(account_id) {
                    self.status.clear();
                    changed = true;
                }
            }
            WorkerMailboxEvent::DraftSaved {
                autosave,
                message_id,
            } => {
                if self.current_account_id == Some(account_id)
                    && (autosave.is_none() || message_id.is_some())
                {
                    self.status = "Draft saved".into();
                    changed = true;
                }
            }
            WorkerMailboxEvent::Status(status) => {
                if self.current_account_id == Some(account_id) {
                    self.status = status;
                    changed = true;
                }
            }
            WorkerMailboxEvent::Notice(notice) => {
                if self.current_account_id == Some(account_id) {
                    self.status = notice;
                    changed = true;
                }
            }
            WorkerMailboxEvent::Error { text, connectivity } => {
                if connectivity {
                    self.connectivity_errors.insert(account_id, text.clone());
                } else {
                    self.action_errors.insert(account_id, text.clone());
                }
                if self
                    .pending_attachment
                    .as_ref()
                    .is_some_and(|pending| pending.key.account_id == account_id)
                {
                    self.pending_attachment = None;
                    self.attachment_state = AttachmentState::Failed(text);
                }
                if self.current_account_id == Some(account_id) {
                    self.loading = false;
                    self.body_loading = false;
                    self.pending_bodies
                        .retain(|slot, _| slot.account_id != account_id);
                    self.status.clear();
                    changed = true;
                }
            }
            _ => {}
        }
        if changed {
            cx.notify();
        }
    }

    fn request_page(
        &mut self,
        cursor: Option<MessageCursor>,
        append: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.demo_mode {
            self.loading = false;
            self.loading_more = false;
            self.pending_page_generation = None;
            self.rebuild_visible();
            self.ensure_visible_selection(cx);
            cx.notify();
            return true;
        }
        let Some(account_id) = self.current_account_id else {
            return false;
        };
        let Some(path) = self.current_folder_path.clone() else {
            return false;
        };
        let Some(folder_id) = self.folder_id(account_id, &path) else {
            return false;
        };

        self.query_generation = self.query_generation.wrapping_add(1).max(1);
        let generation = self.query_generation;
        self.pending_page_generation = Some(generation);
        self.pending_page_append = append;
        self.loading = !append;
        self.loading_more = append;
        let request = make_page_request(
            account_id,
            path,
            folder_id,
            self.search_query.clone(),
            generation,
            cursor,
        );
        if let Some(sender) = self.page_senders.get(&account_id) {
            if let Err(error) = sender(request) {
                self.pending_page_generation = None;
                self.loading = false;
                self.loading_more = false;
                self.query_errors
                    .insert(account_id, submit_error_text(error).into());
                cx.notify();
                return false;
            }
            return true;
        }

        let Some(query_service) = self.query_service.as_ref() else {
            self.loading = false;
            self.loading_more = false;
            self.pending_page_generation = None;
            self.query_errors.insert(
                account_id,
                self.query_start_error
                    .clone()
                    .unwrap_or_else(|| "The message index is unavailable.".into()),
            );
            cx.notify();
            return false;
        };
        let sender = self.event_sender.clone();
        let submitted = query_service.request_page(request, move |result| {
            let _ = sender.unbounded_send(MailboxEvent::Page(result));
        });
        if let Err(error) = submitted {
            self.pending_page_generation = None;
            self.loading = false;
            self.loading_more = false;
            self.query_errors
                .insert(account_id, submit_error_text(error).into());
            cx.notify();
            return false;
        }
        true
    }

    fn handle_page_result(&mut self, result: PageResult, cx: &mut Context<Self>) {
        if self.pending_page_generation != Some(result.generation)
            || self.current_account_id != Some(result.account_id)
            || self.current_folder_path.as_deref() != Some(result.folder_path.as_str())
            || self.folder_id(result.account_id, &result.folder_path) != Some(result.folder_id)
        {
            return;
        }
        self.pending_page_generation = None;
        self.loading = false;
        self.loading_more = false;
        match result.status {
            PageStatus::Ready => {
                let append = self.pending_page_append;
                let chronological = self.thunderbird_accounts.contains(&result.account_id);
                let (messages, _) = merge_message_batch_ordered(
                    if append { &self.messages } else { &[] },
                    result.rows,
                    result.account_id,
                    result.folder_id,
                    append,
                    chronological,
                );
                self.messages = messages;
                self.next_cursor = result.next_cursor;
                self.truncated = self.messages.len() >= MESSAGE_CAP && self.next_cursor.is_some();
                self.query_errors.remove(&result.account_id);
                self.connectivity_errors.remove(&result.account_id);
                self.rebuild_visible();
                self.ensure_visible_selection(cx);
            }
            PageStatus::Failed(error) => {
                self.query_errors.insert(result.account_id, error);
            }
            PageStatus::Superseded => return,
        }
        cx.notify();
    }

    fn apply_body(
        &mut self,
        account_id: u32,
        path: &str,
        message_id: u32,
        body: String,
        links: Vec<MailLink>,
        has_attachment: Option<bool>,
        reply_to: Option<String>,
        references: Option<String>,
    ) -> BodyApplication {
        let slot = BodyRequestSlot {
            account_id,
            folder_path: path.to_owned(),
            message_id,
        };
        let Some(pending) = self.pending_bodies.remove(&slot) else {
            return BodyApplication::Ignored;
        };
        let matches = body_event_matches(
            account_id,
            path,
            message_id,
            self.current_account_id,
            self.current_folder_path.as_deref(),
            self.selected_key.as_ref(),
            &pending,
        );
        if !matches {
            return BodyApplication::Stale;
        }
        let Some(message_index) = self.messages.iter().position(|message| {
            message.id == message_id
                && message.uid == pending.key.uid
                && message.account_id == account_id
        }) else {
            return BodyApplication::Stale;
        };
        let metadata_changed =
            has_attachment.is_some() || reply_to.is_some() || references.is_some();
        if let Some(has_attachment) = has_attachment {
            self.messages[message_index].has_attachment = has_attachment;
        }
        if let Some(reply_to) = reply_to {
            self.messages[message_index].reply_to = reply_to;
        }
        if let Some(references) = references {
            self.messages[message_index].references = references;
        }
        if metadata_changed {
            self.rebuild_visible();
        }
        let mut selected_message = self.messages[message_index].clone();
        selected_message.body = body;
        self.selected_row = Some(Arc::new(MessageRow {
            key: pending.key,
            message: selected_message,
        }));
        self.links = links;
        BodyApplication::Applied
    }

    fn attachment_event_matches(&self, account_id: u32, path: &str, message_id: u32) -> bool {
        let Some(pending) = self.pending_attachment.as_ref() else {
            return false;
        };
        let message_present = self.messages.iter().any(|message| {
            message.account_id == account_id
                && message.id == message_id
                && message.uid == pending.key.uid
        });
        attachment_response_matches(
            account_id,
            path,
            message_id,
            self.current_account_id,
            self.current_folder_path.as_deref(),
            self.selected_key.as_ref(),
            pending,
            message_present,
        )
    }

    fn request_selected_body(&mut self, cx: &mut Context<Self>) {
        let Some(key) = self.selected_key.clone() else {
            self.body_loading = false;
            return;
        };
        let Some(message) = self
            .messages
            .iter()
            .find(|message| self.key_for(message).as_ref() == Some(&key))
        else {
            self.body_loading = false;
            return;
        };
        let message_id = message.id;
        let body_loaded = self
            .selected_row
            .as_ref()
            .is_some_and(|row| row.key == key && !row.message.body.is_empty());
        if body_loaded {
            self.body_loading = false;
            return;
        }
        let slot = BodyRequestSlot {
            account_id: key.account_id,
            folder_path: key.folder_path.clone(),
            message_id,
        };
        if self.pending_bodies.contains_key(&slot) {
            self.body_loading = self
                .pending_bodies
                .get(&slot)
                .is_some_and(|pending| pending.key == key);
            return;
        }
        self.pending_bodies.insert(
            slot,
            PendingBody {
                key: key.clone(),
                message_id,
            },
        );
        self.body_loading = true;
        let sent = self.send_request(
            key.account_id,
            MailRequest::LoadBody {
                message_id,
                path: key.folder_path.clone(),
                uid: key.uid,
            },
            cx,
        );
        if !sent {
            self.pending_bodies.remove(&BodyRequestSlot {
                account_id: key.account_id,
                folder_path: key.folder_path,
                message_id,
            });
            self.body_loading = false;
        }
    }

    fn ensure_visible_selection(&mut self, cx: &mut Context<Self>) {
        let selected_visible = self.selected_key.as_ref().is_some_and(|key| {
            self.visible_indices.iter().any(|index| {
                self.messages
                    .get(*index)
                    .is_some_and(|message| self.key_for(message).as_ref() == Some(key))
            })
        });
        if !selected_visible {
            self.selected_key = self
                .visible_indices
                .first()
                .and_then(|index| self.messages.get(*index))
                .and_then(|message| {
                    self.current_folder_path
                        .as_ref()
                        .map(|path| message_key(message.account_id, path, message.uid))
                });
            self.body_loading = false;
            self.links = self
                .selected_key
                .as_ref()
                .and_then(|key| self.demo_links_by_key.get(key))
                .cloned()
                .unwrap_or_default();
            self.attachment_state = AttachmentState::NotLoaded;
            self.pending_attachment = None;
            self.draft_source = None;
        }
        if !selected_visible {
            self.refresh_selected_row();
        }
        self.request_selected_body(cx);
    }

    fn rebuild_visible(&mut self) {
        let query = if self.demo_mode {
            self.search_query.trim().to_lowercase()
        } else {
            String::new()
        };
        self.visible_indices = (0..self.messages.len())
            .filter(|index| {
                let Some(message) = self.messages.get(*index) else {
                    return false;
                };
                query.is_empty()
                    || message.from_name.to_lowercase().contains(&query)
                    || message.from_addr.to_lowercase().contains(&query)
                    || message.subject.to_lowercase().contains(&query)
                    || message.preview.to_lowercase().contains(&query)
            })
            .collect();
        self.page_rows = Arc::new(
            self.visible_indices
                .iter()
                .filter_map(|index| self.messages.get(*index))
                .filter_map(|message| {
                    let path = self.current_folder_path.as_ref()?;
                    let mut message = message.clone();
                    message.body.clear();
                    Some(Arc::new(MessageRow {
                        key: message_key(message.account_id, path, message.uid),
                        message,
                    }))
                })
                .collect(),
        );
        self.refresh_selected_row();
    }

    fn refresh_selected_row(&mut self) {
        let Some(key) = self.selected_key.clone() else {
            self.selected_row = None;
            return;
        };
        let selected_body = self
            .selected_row
            .as_ref()
            .filter(|row| row.key == key)
            .map(|row| row.message.body.clone());
        let selected_row = self
            .messages
            .iter()
            .find(|message| message.account_id == key.account_id && message.uid == key.uid)
            .map(|message| {
                let mut message = message.clone();
                if message.body.is_empty() {
                    if let Some(body) = selected_body {
                        message.body = body;
                    }
                }
                Arc::new(MessageRow { key, message })
            });
        self.selected_row = selected_row;
    }

    fn clear_current_page(&mut self) {
        self.query_generation = self.query_generation.wrapping_add(1).max(1);
        self.pending_page_generation = None;
        self.pending_page_append = false;
        self.next_cursor = None;
        self.messages.clear();
        self.visible_indices.clear();
        self.page_rows = Arc::new(Vec::new());
        self.truncated = false;
        self.selected_key = None;
        self.selected_row = None;
        self.loading_more = false;
        self.body_loading = false;
        self.links.clear();
        self.demo_links_by_key.clear();
        self.attachment_state = AttachmentState::NotLoaded;
        self.pending_attachment = None;
        self.draft_source = None;
    }

    fn key_for(&self, message: &Message) -> Option<MessageKey> {
        let path = self.current_folder_path.as_ref()?;
        Some(message_key(message.account_id, path, message.uid))
    }

    fn folder_id(&self, account_id: u32, path: &str) -> Option<u32> {
        self.folders_by_account
            .get(&account_id)?
            .iter()
            .find(|folder| folder.path == path)
            .map(|folder| folder.id)
    }

    fn path_for_folder(&self, account_id: u32, folder_id: u32) -> Option<String> {
        self.folders_by_account
            .get(&account_id)?
            .iter()
            .find(|folder| folder.id == folder_id)
            .map(|folder| folder.path.clone())
    }

    fn is_current_folder(&self, account_id: u32, path: &str) -> bool {
        self.current_account_id == Some(account_id)
            && self.current_folder_path.as_deref() == Some(path)
    }

    fn set_action_error(&mut self, text: impl Into<String>) {
        if let Some(account_id) = self.current_account_id {
            self.action_errors.insert(account_id, text.into());
        }
    }
}

fn message_key(account_id: u32, folder_path: &str, uid: u32) -> MessageKey {
    MessageKey {
        account_id,
        folder_path: folder_path.to_owned(),
        uid,
    }
}

fn spawn_account_worker(
    account_id: u32,
    config: AccountConfig,
    sender: futures::channel::mpsc::UnboundedSender<MailboxEvent>,
) -> RequestSender {
    let worker_sender = worker::spawn(account_id, Some(config), move |event| {
        if let Some(event) = compact_worker_event(event, false) {
            let _ =
                sender.unbounded_send(MailboxEvent::Worker(WorkerEnvelope { account_id, event }));
        }
    });
    Arc::new(move |request| worker_sender.send(request).is_ok())
}

fn spawn_demo_worker(
    account_id: u32,
    sender: futures::channel::mpsc::UnboundedSender<MailboxEvent>,
) -> RequestSender {
    let worker_sender = worker::spawn(account_id, None, move |event| {
        if let Some(event) = compact_worker_event(event, true) {
            let _ =
                sender.unbounded_send(MailboxEvent::Worker(WorkerEnvelope { account_id, event }));
        }
    });
    Arc::new(move |request| worker_sender.send(request).is_ok())
}

fn default_accent(account_id: u32) -> &'static str {
    const ACCENTS: [&str; 6] = [
        "#7c86ff", "#58b6a9", "#df9a48", "#d9728b", "#8d79d6", "#54a3cf",
    ];
    ACCENTS[account_id.saturating_sub(1) as usize % ACCENTS.len()]
}

fn outbox_summary(item: &OutboxItem) -> OutboxSummary {
    OutboxSummary {
        id: item.id,
        recipients: item.recipients.clone(),
        subject: item.subject.clone(),
        preview: item.preview.clone(),
        queued_at: item.queued_at,
        attempts: item.attempts,
        last_error: item.last_error.clone(),
        send_at: item.send_at,
    }
}

fn compact_worker_event(event: WorkerEvent, demo_mode: bool) -> Option<WorkerMailboxEvent> {
    Some(match event {
        WorkerEvent::Account(account) => WorkerMailboxEvent::Account(account),
        WorkerEvent::Folders(folders) => WorkerMailboxEvent::Folders(folders),
        WorkerEvent::Messages {
            folder_id,
            messages,
        } if demo_mode => {
            let mut links_by_uid = HashMap::new();
            let messages = messages
                .into_iter()
                .map(|mut message| {
                    let (body, links) = plain_body_and_links(&message.body);
                    message.body = body;
                    if !links.is_empty() {
                        links_by_uid.insert(message.uid, links);
                    }
                    message
                })
                .collect();
            WorkerMailboxEvent::DemoMessages {
                folder_id,
                messages,
                links_by_uid,
            }
        }
        WorkerEvent::Messages { folder_id, .. } => WorkerMailboxEvent::FolderChanged { folder_id },
        // Page reads come from the cache actor. Avoid retaining the worker's
        // full batches in the UI event queue alongside those cursor pages.
        WorkerEvent::MessagesAppend { .. } => return None,
        WorkerEvent::FolderSynced { folder_id } => WorkerMailboxEvent::FolderSynced { folder_id },
        WorkerEvent::FolderUnread { folder_id, unread } => {
            WorkerMailboxEvent::FolderUnread { folder_id, unread }
        }
        WorkerEvent::FolderUnreadByPath { path, unread } => {
            WorkerMailboxEvent::FolderUnreadByPath { path, unread }
        }
        WorkerEvent::Body {
            message_id,
            path,
            body,
        } => {
            let (body, links) = plain_body_and_links(&body);
            WorkerMailboxEvent::Body {
                message_id,
                path,
                body,
                links,
                has_attachment: None,
                reply_to: None,
                references: None,
            }
        }
        WorkerEvent::Attachments {
            path,
            message_id,
            items,
        } => {
            let (items, warning) = bounded_attachments(items);
            WorkerMailboxEvent::Attachments {
                message_id,
                path,
                items,
                warning,
            }
        }
        WorkerEvent::AttachmentsPending { path, message_id } => {
            WorkerMailboxEvent::AttachmentsPending { message_id, path }
        }
        WorkerEvent::RawExported { token, raw } => WorkerMailboxEvent::RawExported {
            token,
            raw: bound_draft_source(raw),
        },
        WorkerEvent::Gone {
            message_id,
            path,
            uid,
        } => WorkerMailboxEvent::Gone {
            message_id,
            path,
            uid,
        },
        WorkerEvent::Outbox { items } => WorkerMailboxEvent::Outbox {
            items: items.iter().map(outbox_summary).collect(),
        },
        WorkerEvent::Unsent(_) => return None,
        WorkerEvent::Sent => WorkerMailboxEvent::Sent,
        WorkerEvent::SendFinished {
            request_id,
            outcome,
        } => WorkerMailboxEvent::SendFinished {
            request_id,
            outcome,
        },
        WorkerEvent::DraftSaved {
            autosave,
            message_id,
        } => WorkerMailboxEvent::DraftSaved {
            autosave,
            message_id,
        },
        WorkerEvent::Status(status) => WorkerMailboxEvent::Status(status),
        WorkerEvent::Notice(notice) => WorkerMailboxEvent::Notice(notice),
        WorkerEvent::Error { text, connectivity } => {
            WorkerMailboxEvent::Error { text, connectivity }
        }
        // Other events have no native-shell contract. Raw draft exports take
        // the bounded, token-checked route above; all attachment payloads are
        // compacted through the capped reader DTO.
        _ => return None,
    })
}

/// Convert the worker's HTML reader representation before it crosses the GPUI
/// event boundary. The reader receives text plus explicit HTTP(S) links only.
fn plain_body_and_links(body: &str) -> (String, Vec<MailLink>) {
    if body.len() > BODY_HTML_CAP {
        return (
            "This message body is over the 2 MiB reader limit.".into(),
            Vec::new(),
        );
    }
    let html = body.trim_start().starts_with('<');
    if html {
        (
            megamail_core::markdown::plain_text(body),
            megamail_core::mail_text::extract_links(body),
        )
    } else {
        (body.to_owned(), Vec::new())
    }
}

/// Bound bytes retained by a reader response. Oversized batches stay usable,
/// and omissions are surfaced to the user instead of silently truncating.
fn bounded_attachments(attachments: Vec<Attachment>) -> (Vec<AttachmentSummary>, Option<String>) {
    bounded_attachments_with_limits(attachments, ATTACHMENT_COUNT_CAP, ATTACHMENT_BYTES_CAP)
}

fn bounded_attachments_with_limits(
    attachments: Vec<Attachment>,
    count_cap: usize,
    byte_cap: usize,
) -> (Vec<AttachmentSummary>, Option<String>) {
    let mut accepted = Vec::with_capacity(attachments.len().min(count_cap));
    let mut used_bytes = 0usize;
    let mut omitted = false;
    for attachment in attachments {
        let size = attachment.data.len();
        if accepted.len() >= count_cap || size > byte_cap.saturating_sub(used_bytes) {
            omitted = true;
            continue;
        }
        used_bytes += size;
        accepted.push(AttachmentSummary {
            name: safe_attachment_name(&attachment.name),
            content: Arc::new(attachment.data),
        });
    }
    let warning = omitted.then(|| {
        format!("Some attachments were omitted to keep this message under the {count_cap}-file and {} MiB limit.", byte_cap / (1024 * 1024))
    });
    (accepted, warning)
}

fn bound_draft_source(raw: Result<Vec<u8>, String>) -> Result<Arc<Vec<u8>>, String> {
    match raw {
        Ok(bytes) if bytes.len() > DRAFT_RAW_BYTES_CAP => {
            Err("This draft is over the 100 MiB editing limit. It was left unchanged.".into())
        }
        Ok(bytes) => Ok(Arc::new(bytes)),
        Err(error) => Err(error.chars().take(1024).collect()),
    }
}

fn safe_attachment_name(name: &str) -> String {
    let basename = name
        .rsplit(|character| character == '/' || character == '\\')
        .next()
        .unwrap_or_default();
    let cleaned: String = basename
        .chars()
        .filter(|character| !character.is_control())
        .take(255)
        .collect();
    if cleaned.trim().is_empty() {
        "attachment".into()
    } else {
        cleaned
    }
}

fn submit_error_text(error: SubmitError) -> &'static str {
    match error {
        SubmitError::Full => "The message index is busy. Try again shortly.",
        SubmitError::Closed => "The message index stopped unexpectedly.",
    }
}

fn make_page_request(
    account_id: u32,
    folder_path: String,
    folder_id: u32,
    query: String,
    generation: u64,
    cursor: Option<MessageCursor>,
) -> PageRequest {
    PageRequest {
        account_id,
        folder_path,
        folder_id,
        query,
        generation,
        cursor,
        limit: PAGE_SIZE,
    }
}

fn body_event_matches(
    event_account_id: u32,
    event_path: &str,
    event_message_id: u32,
    current_account_id: Option<u32>,
    current_path: Option<&str>,
    selected_key: Option<&MessageKey>,
    pending: &PendingBody,
) -> bool {
    event_account_id == pending.key.account_id
        && Some(event_account_id) == current_account_id
        && event_path == pending.key.folder_path
        && current_path == Some(pending.key.folder_path.as_str())
        && event_message_id == pending.message_id
        && selected_key == Some(&pending.key)
}

fn attachment_response_matches(
    event_account_id: u32,
    event_path: &str,
    event_message_id: u32,
    current_account_id: Option<u32>,
    current_path: Option<&str>,
    selected_key: Option<&MessageKey>,
    pending: &PendingAttachment,
    message_present: bool,
) -> bool {
    message_present
        && event_account_id == pending.key.account_id
        && Some(event_account_id) == current_account_id
        && event_path == pending.key.folder_path
        && current_path == Some(pending.key.folder_path.as_str())
        && event_message_id == pending.message_id
        && selected_key == Some(&pending.key)
}

fn draft_export_matches(
    event_account_id: u32,
    event_token: u64,
    current_account_id: Option<u32>,
    current_path: Option<&str>,
    selected_key: Option<&MessageKey>,
    source: &DraftSourceSnapshot,
    message_present: bool,
) -> bool {
    message_present
        && matches!(&source.state, DraftSourceState::Loading)
        && source.token == event_token
        && source.key.account_id == event_account_id
        && Some(event_account_id) == current_account_id
        && source.key.folder_path == source.origin.path
        && source.key.uid == source.origin.uid
        && source.key.account_id == source.origin.account_id
        && current_path == Some(source.key.folder_path.as_str())
        && selected_key == Some(&source.key)
}

fn merge_message_batch(
    existing: &[Message],
    incoming: Vec<Message>,
    account_id: u32,
    folder_id: u32,
    append: bool,
) -> (Vec<Message>, bool) {
    merge_message_batch_ordered(existing, incoming, account_id, folder_id, append, false)
}

fn merge_message_batch_ordered(
    existing: &[Message],
    incoming: Vec<Message>,
    account_id: u32,
    folder_id: u32,
    append: bool,
    chronological: bool,
) -> (Vec<Message>, bool) {
    let mut by_uid = HashMap::new();
    if append {
        // `existing` is already the current account+path page; folder IDs may shift on relist.
        for message in existing
            .iter()
            .filter(|message| message.account_id == account_id)
        {
            by_uid.insert(message.uid, message.clone());
        }
    }
    for mut message in incoming
        .into_iter()
        .filter(|message| message.account_id == account_id && message.folder_id == folder_id)
    {
        if let Some(old) = existing
            .iter()
            .find(|old| old.account_id == account_id && old.uid == message.uid)
        {
            if message.body.is_empty() && !old.body.is_empty() {
                message.body.clone_from(&old.body);
            }
        }
        by_uid.insert(message.uid, message);
    }
    let mut messages: Vec<_> = by_uid.into_values().collect();
    if chronological {
        messages.sort_by(|left, right| {
            right
                .timestamp
                .cmp(&left.timestamp)
                .then_with(|| right.uid.cmp(&left.uid))
        });
    } else {
        messages.sort_by(|left, right| right.uid.cmp(&left.uid));
    }
    let truncated = messages.len() > MESSAGE_CAP;
    messages.truncate(MESSAGE_CAP);
    (messages, truncated)
}

fn set_folder_unread(
    folders_by_account: &mut HashMap<u32, Vec<Folder>>,
    account_id: u32,
    path: &str,
    unread: u32,
) -> bool {
    let Some(folder) = folders_by_account
        .get_mut(&account_id)
        .and_then(|folders| folders.iter_mut().find(|folder| folder.path == path))
    else {
        return false;
    };
    if folder.unread == unread {
        return false;
    }
    folder.unread = unread;
    true
}

#[cfg(test)]
mod tests {
    use super::{
        ATTACHMENT_BYTES_CAP, ATTACHMENT_COUNT_CAP, DraftSourceSnapshot, DraftSourceState,
        MESSAGE_CAP, PAGE_SIZE, PendingAttachment, PendingBody, attachment_response_matches,
        body_event_matches, bounded_attachments, bounded_attachments_with_limits,
        draft_export_matches, make_page_request, merge_message_batch, merge_message_batch_ordered,
        message_key, plain_body_and_links, set_folder_unread,
    };
    use megamail_core::cache::MessageCursor;
    use megamail_core::models::{Attachment, DraftOrigin, Folder, FolderKind, Importance, Message};
    use std::collections::HashMap;

    fn message(
        id: u32,
        account_id: u32,
        folder_id: u32,
        uid: u32,
        subject: &str,
        timestamp: i64,
    ) -> Message {
        Message {
            id,
            account_id,
            folder_id,
            uid,
            from_name: "Ari Chen".into(),
            from_addr: "ari@example.test".into(),
            reply_to: String::new(),
            to: String::new(),
            cc: String::new(),
            subject: subject.into(),
            preview: String::new(),
            body: String::new(),
            date: String::new(),
            timestamp,
            unread: true,
            starred: false,
            keywords: Vec::new(),
            has_attachment: false,
            message_id: String::new(),
            references: String::new(),
            importance: Importance::Normal,
            due: 0,
        }
    }

    #[test]
    fn body_response_must_match_account_folder_uid_and_current_selection() {
        let key = message_key(7, "INBOX", 42);
        let pending = PendingBody {
            key: key.clone(),
            message_id: 12,
        };
        let matches = |account, path, uid| {
            let selected = message_key(7, "INBOX", uid);
            body_event_matches(
                account,
                path,
                12,
                Some(7),
                Some("INBOX"),
                Some(&selected),
                &pending,
            )
        };

        assert!(matches(7, "INBOX", 42));
        assert!(!matches(8, "INBOX", 42));
        assert!(!matches(7, "Archive", 42));
        assert!(!matches(7, "INBOX", 43));
        assert!(!body_event_matches(
            7,
            "INBOX",
            13,
            Some(7),
            Some("INBOX"),
            Some(&key),
            &pending
        ));
    }

    #[test]
    fn attachment_response_must_match_selected_account_folder_uid_and_message() {
        let key = message_key(7, "INBOX", 42);
        let pending = PendingAttachment {
            key: key.clone(),
            message_id: 12,
        };
        let matches = |account, path, selected: &super::MessageKey, message_id, present| {
            attachment_response_matches(
                account,
                path,
                message_id,
                Some(7),
                Some("INBOX"),
                Some(selected),
                &pending,
                present,
            )
        };

        assert!(matches(7, "INBOX", &key, 12, true));
        assert!(!matches(8, "INBOX", &key, 12, true));
        assert!(!matches(7, "Archive", &key, 12, true));
        assert!(!matches(7, "INBOX", &message_key(7, "INBOX", 43), 12, true));
        assert!(!matches(7, "INBOX", &key, 13, true));
        assert!(!matches(7, "INBOX", &key, 12, false));
    }

    #[test]
    fn draft_export_response_must_match_token_account_folder_and_uid() {
        let key = message_key(7, "Drafts", 42);
        let source = DraftSourceSnapshot {
            key: key.clone(),
            origin: DraftOrigin {
                account_id: 7,
                folder_id: 3,
                path: "Drafts".into(),
                uid: 42,
            },
            token: 19,
            state: DraftSourceState::Loading,
        };
        let matches = |account, token, path, selected: &super::MessageKey, present| {
            draft_export_matches(
                account,
                token,
                Some(7),
                Some(path),
                Some(selected),
                &source,
                present,
            )
        };

        assert!(matches(7, 19, "Drafts", &key, true));
        assert!(!matches(7, 20, "Drafts", &key, true));
        assert!(!matches(8, 19, "Drafts", &key, true));
        assert!(!matches(7, 19, "Archive", &key, true));
        assert!(!matches(
            7,
            19,
            "Drafts",
            &message_key(7, "Drafts", 43),
            true
        ));
        assert!(!matches(7, 19, "Drafts", &key, false));
    }

    #[test]
    fn reader_strips_html_and_keeps_only_explicit_safe_links() {
        let (body, links) = plain_body_and_links(
            r#"<html><body><p>Hi <b>Ari</b></p><a href="https://example.test/path">Open</a><a href="javascript:alert(1)">bad</a><img src="https://tracker.test/pixel"></body></html>"#,
        );
        assert!(body.contains("Hi Ari"));
        assert!(!body.contains("<b>"));
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].label, "Open");
        assert_eq!(links[0].url, "https://example.test/path");
    }

    #[test]
    fn attachment_batch_has_bounded_count_and_safe_names() {
        let input = (0..ATTACHMENT_COUNT_CAP + 1)
            .map(|index| Attachment {
                name: if index == 0 {
                    "../../private/report.txt".into()
                } else {
                    format!("file-{index}.txt")
                },
                data: vec![index as u8],
            })
            .collect();
        let (items, warning) = bounded_attachments(input);
        assert_eq!(items.len(), ATTACHMENT_COUNT_CAP);
        assert_eq!(items[0].name, "report.txt");
        assert!(warning.is_some());
        assert!(items.iter().map(|item| item.content.len()).sum::<usize>() <= ATTACHMENT_BYTES_CAP);
    }

    #[test]
    fn attachment_batch_omits_files_over_the_total_byte_limit_with_a_warning() {
        let input = vec![
            Attachment {
                name: "fits.bin".into(),
                data: vec![0; 4],
            },
            Attachment {
                name: "too-large.bin".into(),
                data: vec![1; 2],
            },
        ];
        let (items, warning) = bounded_attachments_with_limits(input, 8, 5);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].content.len(), 4);
        assert!(warning.is_some());
    }

    #[test]
    fn page_requests_keep_search_literal_and_cursor_scope() {
        let cursor = MessageCursor { before_uid: 700 };
        let request = make_page_request(
            7,
            "Archive/2024".into(),
            4,
            "50%_ off".into(),
            22,
            Some(cursor),
        );

        assert_eq!(request.account_id, 7);
        assert_eq!(request.folder_path, "Archive/2024");
        assert_eq!(request.folder_id, 4);
        assert_eq!(request.query, "50%_ off");
        assert_eq!(request.generation, 22);
        assert_eq!(request.cursor, Some(cursor));
        assert_eq!(request.limit, PAGE_SIZE);
    }

    #[test]
    fn cursor_pages_merge_by_uid_and_preserve_loaded_body() {
        let mut first = message(1, 7, 1, 702, "Newest", 1);
        first.body = "already loaded".into();
        let second = message(2, 7, 1, 701, "Second", 3);
        let older = message(3, 7, 1, 700, "Older", 9);
        let refreshed_second = message(2, 7, 1, 701, "Updated", 2);

        let first_page = vec![first, second];
        let (next_page, truncated) =
            merge_message_batch(&first_page, vec![older, refreshed_second], 7, 1, true);

        assert!(!truncated);
        assert_eq!(next_page.len(), 3);
        assert_eq!(
            next_page
                .iter()
                .map(|message| message.uid)
                .collect::<Vec<_>>(),
            [702, 701, 700]
        );
        assert_eq!(next_page[1].subject, "Updated");
        assert_eq!(next_page[0].body, "already loaded");
        assert_eq!(next_page.iter().take(PAGE_SIZE).count(), 3);
    }

    #[test]
    fn cached_message_page_stays_bounded_at_the_ui_cap() {
        let messages = (1..=MESSAGE_CAP as u32 + 2)
            .map(|uid| message(uid, 7, 1, uid, "Invoice", uid as i64))
            .collect();
        let (page, truncated) = merge_message_batch(&[], messages, 7, 1, false);

        assert!(truncated);
        assert_eq!(page.len(), MESSAGE_CAP);
        assert_eq!(
            page.first().map(|message| message.uid),
            Some(MESSAGE_CAP as u32 + 2)
        );
        assert!(page.windows(2).all(|pair| pair[0].uid > pair[1].uid));
    }

    #[test]
    fn full_worker_refresh_is_authoritative_after_move_success_or_failure() {
        let old = message(1, 7, 1, 42, "Still here", 1);
        // Queuing a move does not alter the visible page; a failed move refresh keeps the row.
        let (failed_refresh, _) =
            merge_message_batch(std::slice::from_ref(&old), vec![old.clone()], 7, 1, false);
        assert_eq!(failed_refresh.len(), 1);
        // A successful move is reflected only when the worker's refreshed source list omits it.
        let (successful_refresh, _) = merge_message_batch(&failed_refresh, Vec::new(), 7, 1, false);
        assert!(successful_refresh.is_empty());
    }

    #[test]
    fn full_refresh_removes_gone_rows_but_keeps_loaded_body_for_matching_uid() {
        let mut cached = message(1, 7, 1, 42, "Old header", 1);
        cached.body = "loaded plain text".into();
        let updated = message(1, 7, 1, 42, "Updated header", 2);
        let other = message(2, 7, 1, 43, "Gone after refresh", 3);
        let (refreshed, _) = merge_message_batch(&[cached, other], vec![updated], 7, 1, false);

        assert_eq!(refreshed.len(), 1);
        assert_eq!(refreshed[0].body, "loaded plain text");
        assert_eq!(refreshed[0].subject, "Updated header");
    }

    #[test]
    fn thunderbird_page_cap_keeps_newest_messages_before_truncating() {
        let existing = (1..=MESSAGE_CAP as u32)
            .map(|uid| message(uid, 7, 1, uid, "Loaded", 10_001 - uid as i64))
            .collect::<Vec<_>>();
        let older_page = vec![
            message(5_001, 7, 1, 5_001, "Older 1", 5_000),
            message(5_002, 7, 1, 5_002, "Older 2", 4_999),
        ];

        let (page, truncated) =
            merge_message_batch_ordered(&existing, older_page, 7, 1, true, true);

        assert!(truncated);
        assert_eq!(page.len(), MESSAGE_CAP);
        assert_eq!(page.first().map(|message| message.uid), Some(1));
        assert!(page.iter().any(|message| message.uid == MESSAGE_CAP as u32));
        assert!(!page.iter().any(|message| message.uid == 5_002));
    }

    #[test]
    fn folder_unread_update_is_scoped_to_the_account_and_folder_path() {
        let make_folder = |account_id, path: &str, unread| Folder {
            id: 1,
            account_id,
            name: path.into(),
            path: path.into(),
            kind: FolderKind::Inbox,
            unread,
        };
        let mut folders_by_account = HashMap::from([
            (
                7,
                vec![make_folder(7, "INBOX", 5), make_folder(7, "Archive", 9)],
            ),
            (8, vec![make_folder(8, "INBOX", 11)]),
        ]);

        assert!(set_folder_unread(&mut folders_by_account, 7, "INBOX", 3));
        assert!(!set_folder_unread(&mut folders_by_account, 7, "Sent", 1));
        assert_eq!(folders_by_account[&7][0].unread, 3);
        assert_eq!(folders_by_account[&7][1].unread, 9);
        assert_eq!(folders_by_account[&8][0].unread, 11);
    }
}

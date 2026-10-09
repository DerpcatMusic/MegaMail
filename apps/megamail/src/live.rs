//! Live mailbox state backed by the extracted Hylki mail workers.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use futures::StreamExt;
use gpui_kit::{Context, Task};
use megamail_core::{
    cache::MessageCursor,
    config::AccountConfig,
    conversation::group_conversations,
    mail_text::MailLink,
    models::{
        Account, Attachment, DraftOrigin, Folder, FolderKind, Message, OutboxItem, ThreadSummary,
        thread_ids,
    },
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum MailboxScope {
    #[default]
    Account,
    Unified,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MessageRow {
    pub key: MessageKey,
    pub message: Message,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ConversationKey {
    pub account_id: u32,
    pub anchor: String,
}

#[derive(Debug, Clone)]
pub struct ConversationRow {
    pub key: ConversationKey,
    pub representative: Arc<MessageRow>,
    pub members: Arc<Vec<Arc<MessageRow>>>,
    pub count: usize,
    pub unread: usize,
    pub replied: bool,
    pub forwarded: bool,
    pub latest_sent: Option<Arc<MessageRow>>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum MessageFilter {
    #[default]
    All,
    Unread,
    Starred,
    Attachments,
}

impl MessageFilter {
    pub const ALL: [Self; 4] = [Self::All, Self::Unread, Self::Starred, Self::Attachments];

    pub const fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Unread => "Unread",
            Self::Starred => "Starred",
            Self::Attachments => "Attachments",
        }
    }
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
    pub scope: MailboxScope,
    pub title: String,
    pub account_labels: HashMap<u32, String>,
    pub account_loading: HashMap<u32, bool>,
    pub account_errors: HashMap<u32, String>,
    pub current_account_id: Option<u32>,
    pub folders: Vec<Folder>,
    pub current_folder_path: Option<String>,
    pub page: Arc<Vec<Arc<MessageRow>>>,
    pub conversations: Arc<Vec<ConversationRow>>,
    pub conversation_loading: bool,
    pub conversation_error: Option<String>,
    pub conversation_warning: Option<String>,
    pub selected_key: Option<MessageKey>,
    pub selected_message: Option<Arc<MessageRow>>,
    pub attachment_key: Option<MessageKey>,
    pub links: Vec<MailLink>,
    pub attachment_state: AttachmentState,
    pub draft_source: Option<DraftSourceSnapshot>,
    pub search_query: String,
    pub filter: MessageFilter,
    /// Number of rows in the active search result page/cache, before this local filter.
    pub loaded_count: usize,
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
    BodyFailed {
        message_id: u32,
        path: String,
        text: String,
    },
    DemoMessages {
        folder_id: u32,
        messages: Vec<Message>,
        links_by_uid: HashMap<u32, Vec<MailLink>>,
    },
    Related {
        message_id: u32,
        messages: Vec<Message>,
        partial: bool,
        warning: Option<String>,
    },
    ThreadSummaries {
        summaries: Vec<(String, ThreadSummary)>,
        warning: Option<String>,
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct PageKey {
    account_id: u32,
    folder_path: String,
}

#[derive(Debug, Clone, Default)]
struct FolderPageState {
    folder_id: u32,
    generation: u64,
    pending_generation: Option<u64>,
    pending_append: bool,
    next_cursor: Option<MessageCursor>,
    truncated: bool,
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
    reconnectable_thunderbird_ids: HashSet<u32>,
    sender_identities: HashMap<u32, Vec<SenderIdentity>>,
    event_sender: futures::channel::mpsc::UnboundedSender<MailboxEvent>,
    _event_task: Task<()>,
    query_service: Option<QueryService>,
    query_start_error: Option<String>,
    page_states: HashMap<PageKey, FolderPageState>,
    page_keys: HashSet<MessageKey>,
    page_rows: Arc<Vec<Arc<MessageRow>>>,
    conversation_rows: Arc<Vec<ConversationRow>>,
    related_members: HashMap<ConversationKey, Vec<Message>>,
    related_counts: HashMap<ConversationKey, usize>,
    related_requests: HashSet<ConversationKey>,
    thread_summary_requests: HashMap<u32, HashMap<String, Vec<String>>>,
    pending_thread_summaries: HashMap<(u32, String), ConversationKey>,
    next_thread_request_id: u64,
    pending_related: HashMap<(u32, u32), ConversationKey>,
    conversation_errors: HashMap<ConversationKey, String>,
    conversation_warnings: HashMap<ConversationKey, String>,
    sent_header_requests: HashSet<(u32, u32)>,
    pending_sent_refresh: HashSet<PageKey>,
    pending_message_refresh: HashSet<PageKey>,
    accounts: Vec<Account>,
    folders_by_account: HashMap<u32, Vec<Folder>>,
    selected_folder_by_account: HashMap<u32, String>,
    current_account_id: Option<u32>,
    current_folder_path: Option<String>,
    scope: MailboxScope,
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
    filter: MessageFilter,
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
            reconnectable_thunderbird_ids: HashSet::new(),
            sender_identities: HashMap::new(),
            event_sender: event_tx,
            _event_task: event_task,
            query_service,
            query_start_error,
            page_states: HashMap::new(),
            page_keys: HashSet::new(),
            page_rows: Arc::new(Vec::new()),
            conversation_rows: Arc::new(Vec::new()),
            related_members: HashMap::new(),
            related_counts: HashMap::new(),
            related_requests: HashSet::new(),
            thread_summary_requests: HashMap::new(),
            pending_thread_summaries: HashMap::new(),
            next_thread_request_id: 1,
            pending_related: HashMap::new(),
            conversation_errors: HashMap::new(),
            conversation_warnings: HashMap::new(),
            sent_header_requests: HashSet::new(),
            pending_sent_refresh: HashSet::new(),
            pending_message_refresh: HashSet::new(),
            accounts,
            folders_by_account: HashMap::new(),
            selected_folder_by_account: HashMap::new(),
            current_account_id,
            current_folder_path: None,
            scope: MailboxScope::Account,
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
            filter: MessageFilter::All,
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
        for account_id in [1, 2] {
            let sender = spawn_demo_worker(account_id, mailbox.event_sender.clone());
            let _ = sender(MailRequest::LoadOutbox);
            mailbox.request_senders.insert(account_id, sender);
        }
        mailbox
    }

    pub fn snapshot(&self) -> MailboxSnapshot {
        let folders = self
            .current_account_id
            .and_then(|id| self.folders_by_account.get(&id))
            .cloned()
            .unwrap_or_default();
        let current_folder_path = (self.scope == MailboxScope::Account)
            .then(|| self.current_folder_path.clone())
            .flatten();
        let selected_message = self.selected_row.clone();
        let current_account_id = self.current_account_id;
        let account_loading = self.account_loading_snapshot();
        let account_errors = self.account_errors_snapshot();
        let error = current_account_id
            .and_then(|id| account_errors.get(&id).cloned())
            .or_else(|| account_errors.values().next().cloned())
            .or_else(|| self.query_start_error.clone());
        let action_account_id = self
            .selected_key
            .as_ref()
            .map(|key| key.account_id)
            .or(current_account_id);
        let action_error = action_account_id.and_then(|id| self.action_errors.get(&id).cloned());
        let selected_conversation = self.selected_key.as_ref().and_then(|key| {
            self.conversation_rows
                .iter()
                .find(|conversation| conversation.members.iter().any(|row| row.key == *key))
        });
        let conversation_loading = selected_conversation.is_some_and(|conversation| {
            self.pending_related
                .values()
                .any(|key| key == &conversation.key)
                || self
                    .pending_thread_summaries
                    .values()
                    .any(|key| key == &conversation.key)
                || self
                    .pending_sent_refresh
                    .iter()
                    .any(|page| page.account_id == conversation.key.account_id)
        });
        let conversation_error = selected_conversation
            .and_then(|conversation| self.conversation_errors.get(&conversation.key).cloned());
        let conversation_warning = selected_conversation
            .and_then(|conversation| self.conversation_warnings.get(&conversation.key).cloned());
        let outbox = current_account_id
            .and_then(|id| self.outbox_by_account.get(&id))
            .into_iter()
            .flatten()
            .cloned()
            .collect();
        let page_states = self.active_page_keys();
        let has_more = page_states.iter().any(|key| {
            self.page_states
                .get(key)
                .is_some_and(|state| state.next_cursor.is_some())
        });
        let display_cap_reached = page_states.iter().any(|key| {
            self.page_states
                .get(key)
                .is_some_and(|state| state.truncated)
        });
        let unread_count = match self.scope {
            MailboxScope::Account => current_folder_path
                .as_ref()
                .and_then(|path| folders.iter().find(|folder| &folder.path == path))
                .map(|folder| folder.unread)
                .unwrap_or(0),
            MailboxScope::Unified => self
                .folders_by_account
                .values()
                .flatten()
                .filter(|folder| folder.kind == FolderKind::Inbox)
                .map(|folder| folder.unread)
                .sum(),
        };
        let account_labels = self
            .accounts
            .iter()
            .map(|account| (account.id, account.label.clone()))
            .collect();
        let title = self.scope_title();
        let loaded_count = self.active_page_message_count();
        let loading = account_loading.values().any(|loading| *loading)
            || (self.loading && self.accounts.is_empty());
        let loading_more = page_states.iter().any(|key| {
            self.page_states
                .get(key)
                .is_some_and(|state| state.pending_generation.is_some() && state.pending_append)
        });

        MailboxSnapshot {
            accounts: self.accounts.clone(),
            scope: self.scope,
            title,
            account_labels,
            account_loading,
            account_errors,
            current_account_id,
            folders,
            current_folder_path,
            page: self.page_rows.clone(),
            conversations: self.conversation_rows.clone(),
            conversation_loading,
            conversation_error,
            conversation_warning,
            selected_key: self.selected_key.clone(),
            selected_message,
            attachment_key: self.selected_key.clone(),
            links: self.links.clone(),
            attachment_state: self.attachment_state.clone(),
            draft_source: self.draft_source.clone(),
            search_query: self.search_query.clone(),
            filter: self.filter,
            loaded_count,
            status: self.status.clone(),
            loading,
            loading_more,
            body_loading: self.body_loading,
            unread_count,
            has_more,
            display_cap_reached,
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
        let reconnectable_ids = reconnectable_thunderbird_ids(
            &self.thunderbird_accounts,
            &self.reconnectable_thunderbird_ids,
        );
        let reserved_account_ids = self
            .accounts
            .iter()
            .filter(|account| !reconnectable_ids.contains(&account.id))
            .map(|account| account.id)
            .collect();
        let existing_emails = self
            .accounts
            .iter()
            .filter(|account| !reconnectable_ids.contains(&account.id))
            .map(|account| account.email.clone())
            .collect();
        std::thread::Builder::new()
            .name("megamail-thunderbird-restore".into())
            .spawn(move || {
                crate::thunderbird_adapter::restore(
                    event_sender,
                    reserved_account_ids,
                    reconnectable_ids,
                    existing_emails,
                )
            })
            .map_err(|error| {
                self.thunderbird_loading = false;
                self.thunderbird_restore_started = false;
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

        let reconnectable_ids = reconnectable_thunderbird_ids(
            &self.thunderbird_accounts,
            &self.reconnectable_thunderbird_ids,
        );
        let reserved_account_ids = self
            .accounts
            .iter()
            .filter(|account| !reconnectable_ids.contains(&account.id))
            .map(|account| account.id)
            .collect();
        let running_thunderbird_ids = self
            .thunderbird_accounts
            .difference(&reconnectable_ids)
            .copied()
            .collect();
        let existing_emails = self
            .accounts
            .iter()
            .filter(|account| !reconnectable_ids.contains(&account.id))
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
                    reconnectable_ids,
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

    pub fn select_unified(&mut self, cx: &mut Context<Self>) -> bool {
        if self.scope == MailboxScope::Unified {
            return false;
        }
        self.scope = MailboxScope::Unified;
        self.current_folder_path = None;
        self.clear_selection();
        self.loading = (self.accounts.is_empty() && !self.request_senders.is_empty())
            || self.accounts.iter().any(|account| {
                self.folders_by_account
                    .get(&account.id)
                    .is_none_or(|folders| {
                        !folders
                            .iter()
                            .any(|folder| folder.kind == FolderKind::Inbox)
                    })
            });
        for key in self.active_page_keys() {
            if let Some(state) = self.page_states.get(&key) {
                if state.pending_generation.is_none() {
                    self.request_page_for(&key, None, false, cx);
                }
            } else {
                self.request_page_for(&key, None, false, cx);
            }
            self.request_current_folder_for(key.account_id, &key.folder_path, cx);
        }
        self.rebuild_visible();
        self.ensure_visible_selection(cx);
        cx.notify();
        true
    }

    pub fn switch_account(&mut self, account_id: u32, cx: &mut Context<Self>) -> bool {
        if !self.request_senders.contains_key(&account_id)
            || (self.current_account_id == Some(account_id) && self.scope == MailboxScope::Account)
        {
            return false;
        }
        self.scope = MailboxScope::Account;
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
        self.clear_selection();
        self.loading = self.current_folder_path.is_some()
            || !self.folders_by_account.contains_key(&account_id);
        if let Some(path) = self.current_folder_path.clone() {
            let key = PageKey {
                account_id,
                folder_path: path.clone(),
            };
            self.request_page_for(&key, None, false, cx);
            self.request_current_folder(&path, cx);
        }
        self.rebuild_visible();
        self.ensure_visible_selection(cx);
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
        self.scope = MailboxScope::Account;
        let folder_id = folder.id;
        self.current_folder_path = Some(path.clone());
        self.selected_folder_by_account
            .insert(account_id, path.clone());
        self.clear_selection();
        self.loading = true;
        let key = PageKey {
            account_id,
            folder_path: path.clone(),
        };
        self.request_page_for(&key, None, false, cx);
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
        if !self.selectable_key(&key) {
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
            self.request_selected_conversation_members(false, cx);
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
        self.request_selected_conversation_members(false, cx);
        cx.notify();
        true
    }

    pub fn select_conversation(&mut self, key: ConversationKey, cx: &mut Context<Self>) -> bool {
        let Some(row) = self
            .conversation_rows
            .iter()
            .find(|row| row.key == key)
            .cloned()
        else {
            return false;
        };
        self.select_message(row.representative.key.clone(), cx);
        self.request_conversation_members(&row, true, cx);
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
            self.clear_all_page_data();
            self.loading =
                self.scope == MailboxScope::Unified || self.current_folder_path.is_some();
        }
        self.request_page(None, false, cx);
        cx.notify();
    }

    /// Filter the currently loaded current-folder result set without changing its search or cursor.
    pub fn set_filter(&mut self, filter: MessageFilter, cx: &mut Context<Self>) -> bool {
        if self.filter == filter {
            return false;
        }
        self.filter = filter;
        self.rebuild_visible();
        self.ensure_visible_selection(cx);
        cx.notify();
        true
    }

    pub fn load_more(&mut self, cx: &mut Context<Self>) {
        if self.loading_more || self.loading {
            return;
        }
        let keys = self.active_page_keys();
        let mut changed = false;
        for key in keys {
            let Some(state) = self.page_states.get(&key) else {
                continue;
            };
            if state.pending_generation.is_some() {
                continue;
            }
            if self.page_message_count(&key) >= MESSAGE_CAP && state.next_cursor.is_some() {
                self.action_errors.insert(
                    key.account_id,
                    "Showing the newest 5,000 messages for this inbox. Older messages are not loaded yet.".into(),
                );
                if let Some(state) = self.page_states.get_mut(&key) {
                    state.truncated = true;
                }
            } else if let Some(cursor) = state.next_cursor {
                changed |= self.request_page_for(&key, Some(cursor), true, cx);
            }
        }
        if changed {
            cx.notify();
        }
    }

    pub fn refresh(&mut self, cx: &mut Context<Self>) -> bool {
        let reconnectable_ids = reconnectable_thunderbird_ids(
            &self.thunderbird_accounts,
            &self.reconnectable_thunderbird_ids,
        );
        let mut sent = !reconnectable_ids.is_empty() && self.restore_thunderbird(cx);
        let keys = self.active_page_keys();
        for key in keys {
            // A failed Thunderbird session keeps its old sender around until
            // reconnect succeeds. Retrying it here only produces another
            // closed-channel error and can leave the refresh state misleading.
            if reconnectable_ids.contains(&key.account_id) {
                continue;
            }
            sent |= self.request_page_for(&key, None, false, cx);
            if let Some(folder_id) = self.folder_id(key.account_id, &key.folder_path) {
                sent |= self.send_request(
                    key.account_id,
                    MailRequest::LoadMessages {
                        folder_id,
                        path: key.folder_path.clone(),
                    },
                    cx,
                );
                self.send_request(key.account_id, MailRequest::RefreshUnread, cx);
            }
        }
        self.update_loading_state();
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

    pub fn trash_selected(&mut self, cx: &mut Context<Self>) -> bool {
        self.move_selected_to(FolderKind::Trash, cx)
    }

    pub fn set_selected_unread(&mut self, unread: bool, cx: &mut Context<Self>) -> bool {
        if self.demo_mode {
            self.set_action_error("Message actions are disabled in the local demo.");
            cx.notify();
            return false;
        }
        let Some(key) = self.selected_key.clone() else {
            return false;
        };
        let Some(message) = self.message_for_key(&key) else {
            return false;
        };
        if !self.selectable_key(&key) || message.unread == unread {
            return false;
        }
        let sent = self.send_request(
            key.account_id,
            MailRequest::SetSeen {
                path: key.folder_path.clone(),
                uid: key.uid,
                seen: !unread,
            },
            cx,
        );
        if sent {
            self.pending_message_refresh.insert(PageKey {
                account_id: key.account_id,
                folder_path: key.folder_path,
            });
        }
        cx.notify();
        sent
    }

    pub fn set_selected_starred(&mut self, starred: bool, cx: &mut Context<Self>) -> bool {
        if self.demo_mode {
            self.set_action_error("Message actions are disabled in the local demo.");
            cx.notify();
            return false;
        }
        let Some(key) = self.selected_key.clone() else {
            return false;
        };
        let Some(message) = self.message_for_key(&key) else {
            return false;
        };
        if !self.selectable_key(&key) || message.starred == starred {
            return false;
        }
        let sent = self.send_request(
            key.account_id,
            MailRequest::SetFlagged {
                path: key.folder_path.clone(),
                uid: key.uid,
                flagged: starred,
            },
            cx,
        );
        if sent {
            self.pending_message_refresh.insert(PageKey {
                account_id: key.account_id,
                folder_path: key.folder_path,
            });
        }
        cx.notify();
        sent
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
        if self.selected_key.as_ref() != Some(&key) || !self.selectable_key(&key) {
            return false;
        }
        let Some(message) = self
            .messages
            .iter()
            .find(|message| self.key_for(message).as_ref() == Some(&key))
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
        if let Some(account_id) = self
            .selected_key
            .as_ref()
            .map(|key| key.account_id)
            .or(self.current_account_id)
        {
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
        if !self.selectable_key(&key) {
            return false;
        }
        let account_id = key.account_id;
        let Some(destination) = self
            .folders_by_account
            .get(&account_id)
            .and_then(|folders| folders.iter().find(|folder| folder.kind == kind))
            .map(|folder| folder.path.clone())
        else {
            let label = match kind {
                FolderKind::Archive => "Archive",
                FolderKind::Trash => "Trash",
                _ => "Inbox",
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
            self.status = "Moving message…".into();
            self.pending_message_refresh.insert(PageKey {
                account_id,
                folder_path: key.folder_path.clone(),
            });
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
                self.thunderbird_restore_started = false;
                self.thunderbird_results = results;
                self.thunderbird_error = error;
                self.thunderbird_notice = notice;
                for started in accounts {
                    let account_id = started.account.id;
                    let reconnecting = is_reconnectable_thunderbird_id(
                        account_id,
                        &self.thunderbird_accounts,
                        &self.reconnectable_thunderbird_ids,
                    );
                    let duplicate_id = (self.request_senders.contains_key(&account_id)
                        || self.accounts.iter().any(|account| account.id == account_id))
                        && !reconnecting;
                    let duplicate_email = self.accounts.iter().any(|account| {
                        account.id != account_id
                            && account.email.eq_ignore_ascii_case(&started.account.email)
                    });
                    if duplicate_id || duplicate_email {
                        self.thunderbird_error.get_or_insert_with(|| {
                            format!("{} is already connected.", started.account.email)
                        });
                        continue;
                    }
                    if reconnecting {
                        self.reset_thunderbird_runtime_requests(account_id);
                    }
                    if let Some(account) = self
                        .accounts
                        .iter_mut()
                        .find(|account| account.id == account_id)
                    {
                        *account = started.account.clone();
                    } else {
                        self.accounts.push(started.account.clone());
                    }
                    self.request_senders
                        .insert(account_id, started.request_sender);
                    self.page_senders.insert(account_id, started.page_sender);
                    self.thunderbird_accounts.insert(account_id);
                    self.sender_identities
                        .insert(account_id, started.identities);
                    if !reconnecting || !started.folders.is_empty() {
                        self.folders_by_account
                            .insert(account_id, started.folders.clone());
                    }
                    let folders = self
                        .folders_by_account
                        .get(&account_id)
                        .cloned()
                        .unwrap_or_default();
                    let selected_path = self
                        .selected_folder_by_account
                        .get(&account_id)
                        .filter(|path| folders.iter().any(|folder| &folder.path == *path))
                        .cloned()
                        .or_else(|| {
                            (self.current_account_id == Some(account_id))
                                .then(|| self.current_folder_path.as_ref())
                                .flatten()
                                .filter(|path| folders.iter().any(|folder| &folder.path == *path))
                                .cloned()
                        })
                        .or_else(|| {
                            folders
                                .iter()
                                .find(|folder| folder.kind == FolderKind::Inbox)
                                .or_else(|| folders.first())
                                .map(|folder| folder.path.clone())
                        });
                    if let Some(path) = &selected_path {
                        self.selected_folder_by_account
                            .insert(account_id, path.clone());
                    }
                    if let Some(error) = started.error.as_ref() {
                        if is_reconnectable_thunderbird_failure(error) {
                            self.reconnectable_thunderbird_ids.insert(account_id);
                            clear_pending_pages_if_transport_closed(
                                &mut self.page_states,
                                account_id,
                                true,
                                error,
                            );
                        }
                        self.connectivity_errors.insert(account_id, error.clone());
                    } else {
                        self.reconnectable_thunderbird_ids.remove(&account_id);
                        self.connectivity_errors.remove(&account_id);
                        self.query_errors.remove(&account_id);
                        if self
                            .action_errors
                            .get(&account_id)
                            .is_some_and(|error| is_reconnectable_thunderbird_failure(error))
                        {
                            self.action_errors.remove(&account_id);
                        }
                    }
                    if self.current_account_id.is_none() {
                        self.current_account_id = Some(account_id);
                        self.current_folder_path = (self.scope == MailboxScope::Account)
                            .then(|| selected_path.clone())
                            .flatten();
                        self.loading = self.current_folder_path.is_some();
                        self.clear_current_page();
                    } else if self.current_account_id == Some(account_id)
                        && self.scope == MailboxScope::Account
                        && self.current_folder_path != selected_path
                    {
                        self.current_folder_path = selected_path.clone();
                        self.clear_current_page();
                    }
                    let path = if self.scope == MailboxScope::Unified {
                        folders
                            .iter()
                            .find(|folder| folder.kind == FolderKind::Inbox)
                            .map(|folder| folder.path.clone())
                    } else {
                        selected_path
                    };
                    if let Some(path) = path {
                        if self.scope == MailboxScope::Unified {
                            let key = PageKey {
                                account_id,
                                folder_path: path.clone(),
                            };
                            self.request_page_for(&key, None, false, cx);
                        }
                        self.request_current_folder_for(account_id, &path, cx);
                    }
                }
                if self.scope == MailboxScope::Account && self.current_folder_path.is_some() {
                    let _ = self.request_page(None, false, cx);
                }
                self.update_loading_state();
                cx.notify();
            }
        }
    }

    fn reset_thunderbird_runtime_requests(&mut self, account_id: u32) {
        for (key, state) in &mut self.page_states {
            if key.account_id == account_id {
                state.generation = state.generation.wrapping_add(1).max(1);
                state.pending_generation = None;
                state.pending_append = false;
                state.next_cursor = None;
                state.truncated = false;
            }
        }
        self.pending_bodies
            .retain(|slot, _| slot.account_id != account_id);
        self.pending_related
            .retain(|(pending_account, _), _| *pending_account != account_id);
        self.related_requests
            .retain(|key| key.account_id != account_id);
        self.thread_summary_requests.remove(&account_id);
        self.pending_thread_summaries
            .retain(|(pending_account, _), _| *pending_account != account_id);
        self.sent_header_requests
            .retain(|(pending_account, _)| *pending_account != account_id);
        self.pending_sent_refresh
            .retain(|page| page.account_id != account_id);
        self.pending_message_refresh
            .retain(|page| page.account_id != account_id);
        self.conversation_errors
            .retain(|key, _| key.account_id != account_id);
        if self
            .pending_attachment
            .as_ref()
            .is_some_and(|pending| pending.key.account_id == account_id)
        {
            self.pending_attachment = None;
            self.attachment_state = AttachmentState::NotLoaded;
        }
        if self
            .draft_source
            .as_ref()
            .is_some_and(|source| source.key.account_id == account_id)
        {
            self.draft_source = None;
        }
        if self
            .selected_key
            .as_ref()
            .is_some_and(|key| key.account_id == account_id)
        {
            self.body_loading = false;
            self.links.clear();
            if let Some(row) = self
                .selected_row
                .clone()
                .filter(|row| row.key.account_id == account_id)
            {
                let mut message = row.message.clone();
                message.body.clear();
                self.selected_row = Some(Arc::new(MessageRow {
                    key: row.key.clone(),
                    message,
                }));
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
                self.reconnectable_thunderbird_ids.remove(&account_id);
                self.connectivity_errors.remove(&account_id);
                let folders: Vec<_> = folders
                    .into_iter()
                    .filter(|folder| folder.account_id == account_id)
                    .collect();
                let old_selected = self
                    .selected_folder_by_account
                    .get(&account_id)
                    .cloned()
                    .or_else(|| {
                        (self.current_account_id == Some(account_id))
                            .then(|| self.current_folder_path.clone())
                            .flatten()
                    });
                let old_folder_id = old_selected.as_ref().and_then(|path| {
                    self.folders_by_account
                        .get(&account_id)
                        .and_then(|old| old.iter().find(|folder| &folder.path == path))
                        .map(|folder| folder.id)
                });
                let old_inbox_id = self
                    .folders_by_account
                    .get(&account_id)
                    .and_then(|old| old.iter().find(|folder| folder.kind == FolderKind::Inbox))
                    .map(|folder| folder.id);
                self.folders_by_account.insert(account_id, folders.clone());
                if self.scope == MailboxScope::Unified {
                    if let Some(inbox) = folders
                        .iter()
                        .find(|folder| folder.kind == FolderKind::Inbox)
                    {
                        if old_selected
                            .as_ref()
                            .is_none_or(|path| !folders.iter().any(|folder| &folder.path == path))
                        {
                            self.selected_folder_by_account
                                .insert(account_id, inbox.path.clone());
                        }
                        let key = PageKey {
                            account_id,
                            folder_path: inbox.path.clone(),
                        };
                        let changed_id = old_inbox_id != Some(inbox.id);
                        if changed_id || !self.page_states.contains_key(&key) {
                            self.request_page_for(&key, None, false, cx);
                        }
                        self.request_current_folder_for(account_id, &inbox.path, cx);
                    }
                    if self.current_account_id.is_none() {
                        self.current_account_id = Some(account_id);
                    }
                } else if self.current_account_id == Some(account_id) {
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
                        let key = PageKey {
                            account_id,
                            folder_path: path.clone(),
                        };
                        if path_changed
                            || folder_id_changed
                            || !self.page_states.contains_key(&key)
                            || self.page_message_count(&key) == 0
                        {
                            self.clear_current_page();
                            self.loading = true;
                            self.request_page_for(&key, None, false, cx);
                            self.request_current_folder_for(account_id, &path, cx);
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
                    let key = PageKey {
                        account_id,
                        folder_path: path.clone(),
                    };
                    let active = self.is_current_folder(account_id, &path);
                    if active {
                        if !self.demo_mode {
                            self.request_page_for(&key, None, false, cx);
                        }
                        self.status.clear();
                        self.connectivity_errors.remove(&account_id);
                        self.reconnectable_thunderbird_ids.remove(&account_id);
                        changed = true;
                    }
                    let sent_refresh = self.pending_sent_refresh.remove(&key);
                    let message_refresh = self.pending_message_refresh.remove(&key);
                    if sent_refresh || message_refresh {
                        self.invalidate_conversation_cache(account_id);
                        self.request_thread_summaries(cx);
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
            WorkerMailboxEvent::BodyFailed {
                message_id,
                path,
                text,
            } => {
                if remove_failed_body_request(
                    &mut self.pending_bodies,
                    account_id,
                    &path,
                    message_id,
                    self.selected_key.as_ref(),
                ) {
                    self.body_loading = false;
                    self.action_errors.insert(account_id, text);
                    changed = true;
                }
            }
            WorkerMailboxEvent::DemoMessages {
                folder_id,
                messages,
                links_by_uid,
            } => {
                if self.demo_mode {
                    let Some(path) = self.path_for_folder(account_id, folder_id) else {
                        return;
                    };
                    let page_key = PageKey {
                        account_id,
                        folder_path: path.clone(),
                    };
                    let active_page = self.active_page_keys().contains(&page_key);
                    let sent_headers = self.pending_sent_refresh.remove(&page_key);
                    let message_refresh = self.pending_message_refresh.remove(&page_key);
                    if !active_page && !sent_headers && !message_refresh {
                        return;
                    }
                    self.demo_links_by_key
                        .retain(|key, _| key.account_id != account_id || key.folder_path != path);
                    self.demo_links_by_key.extend(
                        links_by_uid
                            .into_iter()
                            .map(|(uid, links)| (message_key(account_id, &path, uid), links)),
                    );

                    let mut messages = messages;
                    if account_id == 1 && folder_id == 1 {
                        messages.push(demo_long_thread_reply());
                    }
                    if account_id == 2 && folder_id == 13 {
                        messages.push(demo_team_sent_reply());
                    }
                    let existing = self
                        .messages
                        .iter()
                        .filter(|message| {
                            message.account_id == account_id
                                && message.folder_id == folder_id
                                && self.page_keys.contains(&message_key(
                                    account_id,
                                    &path,
                                    message.uid,
                                ))
                        })
                        .cloned()
                        .collect::<Vec<_>>();
                    let (messages, _) =
                        merge_message_batch(&existing, messages, account_id, folder_id, false);
                    self.messages.retain(|message| {
                        message.account_id != account_id || message.folder_id != folder_id
                    });
                    if active_page {
                        self.page_keys
                            .retain(|key| key.account_id != account_id || key.folder_path != path);
                        self.page_keys.extend(
                            messages
                                .iter()
                                .map(|message| message_key(account_id, &path, message.uid)),
                        );
                        if let Some(state) = self.page_states.get_mut(&page_key) {
                            state.pending_generation = None;
                            state.next_cursor = None;
                        }
                    }
                    self.messages.extend(messages);
                    if sent_headers || message_refresh {
                        let sent_rows = self
                            .messages
                            .iter()
                            .filter(|message| {
                                message.account_id == account_id
                                    && message.folder_id == folder_id
                                    && !self.page_keys.contains(&message_key(
                                        account_id,
                                        &path,
                                        message.uid,
                                    ))
                            })
                            .cloned()
                            .collect::<Vec<_>>();
                        for related in self
                            .related_members
                            .iter_mut()
                            .filter(|(key, _)| key.account_id == account_id)
                            .map(|(_, related)| related)
                        {
                            let thread_ids = thread_ids(related.iter());
                            for message in &sent_rows {
                                if message_matches_thread_ids(message, &thread_ids)
                                    && !related.iter().any(|existing| {
                                        existing.account_id == message.account_id
                                            && existing.folder_id == message.folder_id
                                            && existing.uid == message.uid
                                    })
                                {
                                    related.push(message.clone());
                                }
                            }
                        }
                    }
                    self.rebuild_visible();
                    self.update_loading_state();
                    if active_page {
                        self.ensure_visible_selection(cx);
                    }
                    if self
                        .selected_key
                        .as_ref()
                        .is_some_and(|key| key.account_id == account_id)
                    {
                        self.links = self
                            .selected_key
                            .as_ref()
                            .and_then(|key| self.demo_links_by_key.get(key))
                            .cloned()
                            .unwrap_or_default();
                    }
                    changed = true;
                }
            }
            WorkerMailboxEvent::Related {
                message_id,
                messages,
                partial,
                warning,
            } => {
                if let Some(key) = self.pending_related.remove(&(account_id, message_id)) {
                    let accepted = messages
                        .into_iter()
                        .filter(|message| message.account_id == account_id)
                        .filter(|message| {
                            !matches!(
                                self.folder_kind(message.account_id, message.folder_id),
                                Some(
                                    FolderKind::Drafts
                                        | FolderKind::Templates
                                        | FolderKind::Trash
                                        | FolderKind::Junk
                                )
                            )
                        })
                        .filter_map(|mut message| {
                            self.path_for_folder(account_id, message.folder_id)?;
                            message.body.clear();
                            Some(message)
                        })
                        .collect::<Vec<_>>();
                    for message in accepted {
                        {
                            let related = self.related_members.entry(key.clone()).or_default();
                            if let Some(existing) = related.iter_mut().find(|existing| {
                                existing.account_id == message.account_id
                                    && existing.folder_id == message.folder_id
                                    && existing.uid == message.uid
                            }) {
                                replace_message_header(existing, message.clone());
                            } else {
                                related.push(message.clone());
                            }
                        }
                        if let Some(existing) = self.messages.iter_mut().find(|existing| {
                            existing.account_id == message.account_id
                                && existing.folder_id == message.folder_id
                                && existing.uid == message.uid
                        }) {
                            replace_message_header(existing, message);
                        } else {
                            self.messages.push(message);
                        }
                    }
                    if let Some(warning) = conversation_response_warning(partial, warning) {
                        self.conversation_warnings.insert(key.clone(), warning);
                    } else {
                        self.conversation_warnings.remove(&key);
                    }
                    self.conversation_errors.remove(&key);
                    self.rebuild_visible();
                    changed = true;
                }
            }
            WorkerMailboxEvent::ThreadSummaries { summaries, warning } => {
                for (tag, summary) in summaries {
                    let Some(key) = self.pending_thread_summaries.remove(&(account_id, tag)) else {
                        continue;
                    };
                    if let Some(warning) = warning.as_ref() {
                        self.conversation_warnings
                            .insert(key.clone(), warning.clone());
                    }
                    let accepted = summary
                        .members
                        .into_iter()
                        .filter_map(|mut message| {
                            if message.account_id != account_id
                                || matches!(
                                    self.folder_kind(message.account_id, message.folder_id),
                                    Some(
                                        FolderKind::Drafts
                                            | FolderKind::Templates
                                            | FolderKind::Trash
                                            | FolderKind::Junk
                                    )
                                )
                            {
                                return None;
                            }
                            self.path_for_folder(account_id, message.folder_id)?;
                            message.body.clear();
                            Some(message)
                        })
                        .collect::<Vec<_>>();
                    for message in accepted {
                        {
                            let related = self.related_members.entry(key.clone()).or_default();
                            if let Some(existing) = related.iter_mut().find(|existing| {
                                existing.account_id == message.account_id
                                    && existing.folder_id == message.folder_id
                                    && existing.uid == message.uid
                            }) {
                                replace_message_header(existing, message.clone());
                            } else {
                                related.push(message.clone());
                            }
                        }
                        if let Some(existing) = self.messages.iter_mut().find(|existing| {
                            existing.account_id == message.account_id
                                && existing.folder_id == message.folder_id
                                && existing.uid == message.uid
                        }) {
                            replace_message_header(existing, message);
                        } else {
                            self.messages.push(message);
                        }
                    }
                    self.related_counts.insert(key, summary.count);
                }
                self.rebuild_visible();
                changed = true;
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
                if let Some(path) = self.path_for_folder(account_id, folder_id) {
                    let key = PageKey {
                        account_id,
                        folder_path: path.clone(),
                    };
                    if self.is_current_folder(account_id, &path) {
                        self.status.clear();
                        self.connectivity_errors.remove(&account_id);
                        if !self.demo_mode {
                            self.invalidate_conversation_cache(account_id);
                            self.request_page_for(&key, None, false, cx);
                        }
                        changed = true;
                    }
                    let sent_refresh = self.pending_sent_refresh.remove(&key);
                    let message_refresh = self.pending_message_refresh.remove(&key);
                    if sent_refresh || message_refresh {
                        self.invalidate_conversation_cache(account_id);
                        self.request_thread_summaries(cx);
                        changed = true;
                    }
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
                    if outcome == SendOutcome::Sent {
                        self.invalidate_conversation_cache(account_id);
                        if self.thunderbird_accounts.contains(&account_id) {
                            self.request_thread_summaries(cx);
                        } else {
                            self.request_sent_headers(account_id, true, cx);
                        }
                    }
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
                let transport_closed = self.thunderbird_accounts.contains(&account_id)
                    && is_reconnectable_thunderbird_failure(&text);
                let abandoned_pages = clear_pending_pages_if_transport_closed(
                    &mut self.page_states,
                    account_id,
                    self.thunderbird_accounts.contains(&account_id),
                    &text,
                );
                if transport_closed {
                    self.reconnectable_thunderbird_ids.insert(account_id);
                    retain_selected_body_after_error(&mut self.pending_bodies, account_id, None);
                }
                if connectivity {
                    self.connectivity_errors.insert(account_id, text.clone());
                } else {
                    self.action_errors.insert(account_id, text.clone());
                }
                let mut account_changed =
                    self.active_account_ids().contains(&account_id) || abandoned_pages;
                if self.pending_attachment.as_ref().is_some_and(|pending| {
                    pending.key.account_id == account_id
                        && self.selected_key.as_ref() == Some(&pending.key)
                }) {
                    self.pending_attachment = None;
                    self.attachment_state = AttachmentState::Failed(text.clone());
                    account_changed = true;
                }
                let related_keys: Vec<_> = self
                    .pending_related
                    .iter()
                    .filter(|((pending_account, _), _)| *pending_account == account_id)
                    .map(|(slot, key)| (*slot, key.clone()))
                    .collect();
                for (slot, key) in related_keys {
                    self.pending_related.remove(&slot);
                    self.conversation_errors.insert(key, text.clone());
                    account_changed = true;
                }
                for key in self
                    .pending_thread_summaries
                    .iter()
                    .filter(|((pending_account, _), _)| *pending_account == account_id)
                    .map(|(_, key)| key.clone())
                    .collect::<Vec<_>>()
                {
                    self.conversation_errors.insert(key, text.clone());
                    account_changed = true;
                }
                self.pending_thread_summaries
                    .retain(|(pending_account, _), _| *pending_account != account_id);
                self.thread_summary_requests.remove(&account_id);
                let failed_sent_pages = self
                    .pending_sent_refresh
                    .iter()
                    .filter(|page| page.account_id == account_id)
                    .cloned()
                    .collect::<Vec<_>>();
                for page in failed_sent_pages {
                    self.pending_sent_refresh.remove(&page);
                    if let Some(folder_id) = self.folder_id(account_id, &page.folder_path) {
                        self.sent_header_requests.remove(&(account_id, folder_id));
                    }
                    account_changed = true;
                }
                self.pending_message_refresh
                    .retain(|page| page.account_id != account_id);
                if self
                    .selected_key
                    .as_ref()
                    .is_some_and(|key| key.account_id == account_id)
                {
                    self.body_loading = false;
                    retain_selected_body_after_error(
                        &mut self.pending_bodies,
                        account_id,
                        self.selected_key.as_ref(),
                    );
                    account_changed = true;
                }
                if account_changed {
                    self.update_loading_state();
                }
                if self.current_account_id == Some(account_id) {
                    self.status.clear();
                }
                changed |= account_changed;
            }
            _ => {}
        }
        if changed {
            self.update_loading_state();
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
            self.rebuild_visible();
            self.ensure_visible_selection(cx);
            cx.notify();
            return true;
        }
        let keys = self.active_page_keys();
        let mut submitted = false;
        for key in keys {
            let page_cursor = if append {
                self.page_states
                    .get(&key)
                    .and_then(|state| state.next_cursor)
            } else {
                cursor
            };
            submitted |= self.request_page_for(&key, page_cursor, append, cx);
        }
        submitted
    }

    fn request_page_for(
        &mut self,
        key: &PageKey,
        cursor: Option<MessageCursor>,
        append: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(folder_id) = self.folder_id(key.account_id, &key.folder_path) else {
            return false;
        };
        if self.demo_mode {
            let folder_changed = self
                .page_states
                .get(key)
                .is_some_and(|state| state.folder_id != folder_id);
            if folder_changed {
                self.remove_page_partition(key);
            }
            let state = self.page_states.entry(key.clone()).or_default();
            state.folder_id = folder_id;
            state.generation = state.generation.wrapping_add(1).max(1);
            state.pending_generation = Some(state.generation);
            state.pending_append = append;
            state.next_cursor = None;
            state.truncated = false;
            return true;
        }
        let folder_changed = self
            .page_states
            .get(key)
            .map_or(true, |state| state.folder_id != folder_id);
        if folder_changed {
            self.remove_page_partition(key);
        }
        let state = self.page_states.entry(key.clone()).or_default();
        if folder_changed {
            state.folder_id = folder_id;
            state.next_cursor = None;
            state.truncated = false;
            state.pending_generation = None;
        }
        state.generation = state.generation.wrapping_add(1).max(1);
        let generation = state.generation;
        state.pending_generation = Some(generation);
        state.pending_append = append;
        let request = make_page_request(
            key.account_id,
            key.folder_path.clone(),
            folder_id,
            self.search_query.clone(),
            generation,
            cursor,
        );
        if let Some(sender) = self.page_senders.get(&key.account_id) {
            if let Err(error) = sender(request) {
                if let Some(state) = self.page_states.get_mut(key) {
                    state.pending_generation = None;
                }
                let error_text = submit_error_text(error);
                if self.thunderbird_accounts.contains(&key.account_id)
                    && is_reconnectable_thunderbird_failure(error_text)
                {
                    self.reconnectable_thunderbird_ids.insert(key.account_id);
                }
                self.query_errors.insert(key.account_id, error_text.into());
                self.update_loading_state();
                cx.notify();
                return false;
            }
            self.update_loading_state();
            return true;
        }

        let Some(query_service) = self.query_service.as_ref() else {
            if let Some(state) = self.page_states.get_mut(key) {
                state.pending_generation = None;
            }
            self.query_errors.insert(
                key.account_id,
                self.query_start_error
                    .clone()
                    .unwrap_or_else(|| "The message index is unavailable.".into()),
            );
            self.update_loading_state();
            cx.notify();
            return false;
        };
        let sender = self.event_sender.clone();
        let submitted = query_service.request_page(request, move |result| {
            let _ = sender.unbounded_send(MailboxEvent::Page(result));
        });
        if let Err(error) = submitted {
            if let Some(state) = self.page_states.get_mut(key) {
                state.pending_generation = None;
            }
            self.query_errors
                .insert(key.account_id, submit_error_text(error).into());
            self.update_loading_state();
            cx.notify();
            return false;
        }
        self.update_loading_state();
        true
    }

    fn handle_page_result(&mut self, result: PageResult, cx: &mut Context<Self>) {
        let page_ready = matches!(&result.status, PageStatus::Ready);
        let key = PageKey {
            account_id: result.account_id,
            folder_path: result.folder_path.clone(),
        };
        let Some(state) = self.page_states.get(&key) else {
            return;
        };
        if !page_result_is_current(
            &key,
            state,
            &result,
            self.folder_id(result.account_id, &result.folder_path),
        ) {
            return;
        }
        let append = state.pending_append;
        if let Some(state) = self.page_states.get_mut(&key) {
            state.pending_generation = None;
        }
        match result.status {
            PageStatus::Ready => {
                let chronological = self.thunderbird_accounts.contains(&result.account_id);
                let existing: Vec<_> = self
                    .messages
                    .iter()
                    .filter(|message| {
                        message.account_id == result.account_id
                            && message.folder_id == result.folder_id
                            && self.page_keys.contains(&message_key(
                                result.account_id,
                                &result.folder_path,
                                message.uid,
                            ))
                    })
                    .cloned()
                    .collect();
                let (messages, _) = merge_message_batch_ordered(
                    &existing,
                    result.rows,
                    result.account_id,
                    result.folder_id,
                    append,
                    chronological,
                );
                self.messages.retain(|message| {
                    message.account_id != result.account_id
                        || message.folder_id != result.folder_id
                        || !self.page_keys.contains(&message_key(
                            result.account_id,
                            &result.folder_path,
                            message.uid,
                        ))
                });
                self.messages.extend(messages.iter().cloned());
                if !append {
                    self.page_keys.retain(|message_key| {
                        message_key.account_id != result.account_id
                            || message_key.folder_path != result.folder_path
                    });
                }
                self.page_keys.extend(messages.iter().map(|message| {
                    message_key(result.account_id, &result.folder_path, message.uid)
                }));
                if let Some(state) = self.page_states.get_mut(&key) {
                    state.next_cursor = result.next_cursor;
                    state.truncated = messages.len() >= MESSAGE_CAP && state.next_cursor.is_some();
                }
                self.query_errors.remove(&result.account_id);
                self.connectivity_errors.remove(&result.account_id);
                self.reconnectable_thunderbird_ids
                    .remove(&result.account_id);
                self.rebuild_visible();
                if self.active_page_keys().contains(&key) {
                    self.ensure_visible_selection(cx);
                }
            }
            PageStatus::Failed(error) => {
                if self.thunderbird_accounts.contains(&result.account_id)
                    && is_reconnectable_thunderbird_failure(&error)
                {
                    self.reconnectable_thunderbird_ids.insert(result.account_id);
                }
                self.query_errors.insert(result.account_id, error);
            }
            PageStatus::Superseded => {
                if let Some(state) = self.page_states.get_mut(&key) {
                    finish_superseded_page(state, append);
                }
                self.query_errors.insert(
                    result.account_id,
                    "The mailbox request was superseded. Refresh to try again.".into(),
                );
                self.update_loading_state();
                cx.notify();
                return;
            }
        }
        if page_ready {
            self.request_thread_summaries(cx);
        }
        self.update_loading_state();
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
                && self.key_for(message).as_ref() == Some(&pending.key)
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
        let mut summary_message = self.messages[message_index].clone();
        summary_message.body.clear();
        for related in self.related_members.values_mut() {
            for existing in related.iter_mut().filter(|existing| {
                existing.account_id == summary_message.account_id
                    && existing.folder_id == summary_message.folder_id
                    && existing.uid == summary_message.uid
            }) {
                *existing = summary_message.clone();
            }
        }
        let mut selected_message = self.messages[message_index].clone();
        selected_message.body = body;
        self.selected_row = Some(Arc::new(MessageRow {
            key: pending.key,
            message: selected_message,
        }));
        if !self.demo_mode {
            self.messages[message_index].body.clear();
        }
        self.links = links;
        if metadata_changed {
            self.rebuild_visible();
        }
        BodyApplication::Applied
    }

    fn attachment_event_matches(&self, account_id: u32, path: &str, message_id: u32) -> bool {
        let Some(pending) = self.pending_attachment.as_ref() else {
            return false;
        };
        let message_present = self.messages.iter().any(|message| {
            message.account_id == account_id
                && message.id == message_id
                && self.key_for(message).as_ref() == Some(&pending.key)
        });
        attachment_response_matches(
            account_id,
            path,
            message_id,
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
        if let Some(pending) = self.pending_bodies.get(&slot) {
            if pending.key == key && self.body_loading {
                return;
            }
            if pending.key != key {
                return;
            }
            // A generic account-level error cannot identify which body request failed.
            // Keep the selected slot so a late valid response still matches, but allow
            // a subsequent selection attempt to issue a retry.
            self.pending_bodies.remove(&slot);
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
            self.page_rows.iter().any(|row| row.key == *key)
                || self
                    .conversation_rows
                    .iter()
                    .any(|conversation| conversation.members.iter().any(|row| row.key == *key))
        });
        if !selected_visible {
            self.selected_key = self.page_rows.first().map(|row| row.key.clone());
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
        self.request_selected_conversation_members(false, cx);
    }

    fn rebuild_visible(&mut self) {
        let query = if self.demo_mode {
            self.search_query.trim().to_lowercase()
        } else {
            String::new()
        };
        let active_keys = self.active_page_keys();
        let active_set: HashSet<_> = active_keys.iter().cloned().collect();
        let query = query.to_lowercase();
        let mut visible: Vec<_> = self
            .messages
            .iter()
            .enumerate()
            .filter(|(_, message)| {
                self.key_for(message).is_some_and(|key| {
                    self.page_keys.contains(&key)
                        && active_set.contains(&PageKey {
                            account_id: key.account_id,
                            folder_path: key.folder_path.clone(),
                        })
                }) && (query.is_empty() || message_matches_query(message, &query))
                    && message_matches_filter(message, self.filter)
            })
            .filter_map(|(index, message)| {
                self.key_for(message).map(|key| {
                    (
                        index,
                        message.timestamp,
                        key.account_id,
                        key.folder_path,
                        message.uid,
                    )
                })
            })
            .collect();
        visible.sort_by(|left, right| {
            right
                .1
                .cmp(&left.1)
                .then_with(|| left.2.cmp(&right.2))
                .then_with(|| left.3.cmp(&right.3))
                .then_with(|| left.4.cmp(&right.4))
        });
        self.visible_indices = visible.into_iter().map(|(index, ..)| index).collect();
        self.page_rows = Arc::new(
            self.visible_indices
                .iter()
                .filter_map(|index| self.messages.get(*index))
                .filter_map(|message| {
                    let key = self.key_for(message)?;
                    let mut message = message.clone();
                    message.body.clear();
                    Some(Arc::new(MessageRow { key, message }))
                })
                .collect(),
        );
        self.rebuild_conversations();
        self.refresh_selected_row();
    }

    fn rebuild_conversations(&mut self) {
        let sent_folders: HashSet<_> = self
            .folders_by_account
            .iter()
            .flat_map(|(account_id, folders)| {
                folders
                    .iter()
                    .filter(|folder| folder.kind == FolderKind::Sent)
                    .map(|folder| (*account_id, folder.id))
            })
            .collect();
        let active_page_keys: HashSet<_> = self.active_page_keys().into_iter().collect();
        let active_page_messages = self
            .messages
            .iter()
            .filter(|message| {
                let Some(key) = self.key_for(message) else {
                    return false;
                };
                self.page_keys.contains(&key)
                    && active_page_keys.contains(&PageKey {
                        account_id: key.account_id,
                        folder_path: key.folder_path,
                    })
            })
            .map(|message| {
                let mut message = message.clone();
                message.body.clear();
                message
            })
            .collect::<Vec<_>>();
        let visible_page_messages = self
            .visible_indices
            .iter()
            .filter_map(|index| self.messages.get(*index))
            .collect::<Vec<_>>();
        if visible_page_messages.is_empty() {
            self.conversation_rows = Arc::new(Vec::new());
            return;
        }
        let visible_members: HashSet<_> = visible_page_messages
            .iter()
            .map(|message| physical_message_key(message))
            .collect();
        let active_accounts: HashSet<_> =
            active_page_keys.iter().map(|key| key.account_id).collect();
        let mut all = active_page_messages.clone();
        all.extend(
            self.related_members
                .iter()
                .filter(|(key, _)| active_accounts.contains(&key.account_id))
                .flat_map(|(_, messages)| messages.iter().cloned())
                .filter(|message| {
                    !matches!(
                        self.folder_kind(message.account_id, message.folder_id),
                        Some(
                            FolderKind::Drafts
                                | FolderKind::Templates
                                | FolderKind::Trash
                                | FolderKind::Junk
                        )
                    )
                }),
        );
        let previous_rows = self.conversation_rows.as_ref();
        let mut reused_keys = HashSet::new();
        let mut rows: Vec<ConversationRow> = group_conversations(all, &sent_folders)
            .into_iter()
            .filter(|summary| {
                summary.members.iter().any(|message| {
                    visible_members.contains(&physical_message_key(message))
                        || visible_page_messages
                            .iter()
                            .any(|visible| same_mail_copy(visible, message))
                })
            })
            .filter_map(|summary| {
                let group_members = summary
                    .members
                    .iter()
                    .map(|message| {
                        active_page_messages
                            .iter()
                            .find(|active| same_mail_copy(active, message))
                            .unwrap_or(message)
                            .clone()
                    })
                    .collect::<Vec<_>>();
                let derived_key = ConversationKey {
                    account_id: summary.key.0,
                    anchor: summary.key.1.clone(),
                };
                let key = stable_conversation_key(
                    &group_members,
                    derived_key,
                    previous_rows,
                    self.selected_key.as_ref(),
                    &mut reused_keys,
                );
                let members = group_members
                    .iter()
                    .filter_map(|message| self.message_row(message))
                    .collect::<Vec<_>>();
                let representative = active_representative(
                    &summary.members,
                    &active_page_messages,
                    summary.members.get(summary.latest_index)?,
                );
                let representative = self.message_row(representative)?;
                let latest_sent = summary
                    .latest_sent_index
                    .and_then(|index| summary.members.get(index))
                    .and_then(|message| self.message_row(message));
                Some(ConversationRow {
                    count: self
                        .related_counts
                        .get(&key)
                        .copied()
                        .unwrap_or(summary.count)
                        .max(summary.count),
                    unread: summary.unread,
                    replied: summary.replied,
                    forwarded: summary.forwarded,
                    key,
                    representative,
                    members: Arc::new(members),
                    latest_sent,
                })
            })
            .collect();
        rows.sort_by(|left, right| {
            right
                .representative
                .message
                .timestamp
                .cmp(&left.representative.message.timestamp)
                .then_with(|| left.key.account_id.cmp(&right.key.account_id))
                .then_with(|| {
                    left.representative
                        .key
                        .folder_path
                        .cmp(&right.representative.key.folder_path)
                })
                .then_with(|| {
                    left.representative
                        .key
                        .uid
                        .cmp(&right.representative.key.uid)
                })
        });
        self.conversation_rows = Arc::new(rows);
    }

    fn request_thread_summaries(&mut self, cx: &mut Context<Self>) {
        if self.demo_mode {
            return;
        }
        let active = self.active_page_keys();
        let active_set: HashSet<_> = active.iter().cloned().collect();
        let primary = self
            .messages
            .iter()
            .filter(|message| {
                let Some(key) = self.key_for(message) else {
                    return false;
                };
                self.page_keys.contains(&key)
                    && active_set.contains(&PageKey {
                        account_id: key.account_id,
                        folder_path: key.folder_path,
                    })
            })
            .cloned()
            .collect::<Vec<_>>();
        if primary.is_empty() {
            return;
        }
        let sent_folders: HashSet<_> = self
            .folders_by_account
            .iter()
            .flat_map(|(account_id, folders)| {
                folders
                    .iter()
                    .filter(|folder| folder.kind == FolderKind::Sent)
                    .map(|folder| (*account_id, folder.id))
            })
            .collect();
        let mut by_account: HashMap<u32, Vec<(ConversationKey, Vec<String>)>> = HashMap::new();
        for summary in group_conversations(primary, &sent_folders) {
            let ids = thread_ids(summary.members.iter());
            if ids.is_empty() {
                continue;
            }
            by_account.entry(summary.key.0).or_default().push((
                ConversationKey {
                    account_id: summary.key.0,
                    anchor: summary.key.1,
                },
                ids,
            ));
        }
        for (account_id, groups) in by_account {
            let previous = self.thread_summary_requests.get(&account_id);
            let changed = groups
                .into_iter()
                .filter(|(key, ids)| previous.and_then(|items| items.get(&key.anchor)) != Some(ids))
                .collect::<Vec<_>>();
            if changed.is_empty() {
                continue;
            }
            let mut requests = Vec::with_capacity(changed.len());
            let mut pending = Vec::with_capacity(changed.len());
            for (key, ids) in changed {
                let request_id = self.next_thread_request_id;
                self.next_thread_request_id = self.next_thread_request_id.wrapping_add(1).max(1);
                let tag = format!("live-thread-{request_id}");
                pending.push((tag.clone(), key.clone(), ids.clone()));
                requests.push((tag, ids));
            }
            if self.send_request(
                account_id,
                MailRequest::LoadThreadSummaries { groups: requests },
                cx,
            ) {
                let signatures = self.thread_summary_requests.entry(account_id).or_default();
                for (tag, key, ids) in pending {
                    signatures.insert(key.anchor.clone(), ids);
                    self.pending_thread_summaries.insert((account_id, tag), key);
                }
            }
        }
    }

    fn request_conversation_members(
        &mut self,
        row: &ConversationRow,
        explicit_retry: bool,
        cx: &mut Context<Self>,
    ) {
        let ids = thread_ids(row.members.iter().map(|member| &member.message));
        if !ids.is_empty() {
            let mut seed = row
                .members
                .iter()
                .map(|member| member.message.clone())
                .collect::<Vec<_>>();
            seed.extend(
                self.messages
                    .iter()
                    .filter(|message| {
                        message.account_id == row.key.account_id
                            && self.folder_kind(message.account_id, message.folder_id)
                                == Some(FolderKind::Sent)
                            && message_matches_thread_ids(message, &ids)
                    })
                    .cloned(),
            );
            let related = self.related_members.entry(row.key.clone()).or_default();
            for mut message in seed {
                message.body.clear();
                if let Some(existing) = related.iter_mut().find(|existing| {
                    existing.account_id == message.account_id
                        && existing.folder_id == message.folder_id
                        && existing.uid == message.uid
                }) {
                    replace_message_header(existing, message);
                } else {
                    related.push(message);
                }
            }
        }
        let pending = self
            .pending_related
            .values()
            .any(|pending_key| pending_key == &row.key);
        if explicit_retry && !pending {
            self.related_requests.remove(&row.key);
        }
        if !ids.is_empty() && !pending && self.related_requests.insert(row.key.clone()) {
            let message_id = row.representative.message.id;
            self.pending_related
                .insert((row.key.account_id, message_id), row.key.clone());
            self.conversation_errors.remove(&row.key);
            if !self.send_request(
                row.key.account_id,
                MailRequest::LoadRelated { message_id, ids },
                cx,
            ) {
                self.pending_related
                    .remove(&(row.key.account_id, message_id));
                self.conversation_errors.insert(
                    row.key.clone(),
                    "Could not load the other messages in this conversation.".into(),
                );
            }
        }
        if self.thunderbird_accounts.contains(&row.key.account_id) {
            return;
        }
        self.request_sent_headers(row.key.account_id, false, cx);
    }

    fn request_selected_conversation_members(
        &mut self,
        explicit_retry: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(selected_key) = self.selected_key.as_ref() else {
            return;
        };
        let Some(row) = self
            .conversation_rows
            .iter()
            .find(|conversation| {
                conversation
                    .members
                    .iter()
                    .any(|member| &member.key == selected_key)
            })
            .cloned()
        else {
            return;
        };
        self.request_conversation_members(&row, explicit_retry, cx);
    }

    fn request_sent_headers(
        &mut self,
        account_id: u32,
        force: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        let sent_folder = self
            .folders_by_account
            .get(&account_id)
            .and_then(|folders| {
                folders
                    .iter()
                    .find(|folder| folder.kind == FolderKind::Sent)
            })
            .cloned();
        let Some(folder) = sent_folder else {
            return false;
        };
        if force {
            self.sent_header_requests.remove(&(account_id, folder.id));
        }
        if !self.sent_header_requests.insert((account_id, folder.id)) {
            return false;
        }
        let sent_key = PageKey {
            account_id,
            folder_path: folder.path.clone(),
        };
        if self.send_request(
            account_id,
            MailRequest::LoadMessages {
                folder_id: folder.id,
                path: folder.path,
            },
            cx,
        ) {
            self.pending_sent_refresh.insert(sent_key);
            true
        } else {
            self.sent_header_requests.remove(&(account_id, folder.id));
            false
        }
    }

    fn message_row(&self, message: &Message) -> Option<Arc<MessageRow>> {
        let key = self.key_for(message)?;
        let mut message = message.clone();
        message.body.clear();
        Some(Arc::new(MessageRow { key, message }))
    }

    fn message_for_key(&self, key: &MessageKey) -> Option<&Message> {
        self.messages
            .iter()
            .find(|message| self.key_for(message).as_ref() == Some(key))
    }

    fn invalidate_conversation_cache(&mut self, account_id: u32) {
        self.related_members
            .retain(|key, _| key.account_id != account_id);
        self.related_counts
            .retain(|key, _| key.account_id != account_id);
        self.related_requests
            .retain(|key| key.account_id != account_id);
        self.conversation_errors
            .retain(|key, _| key.account_id != account_id);
        self.conversation_warnings
            .retain(|key, _| key.account_id != account_id);
        self.thread_summary_requests.remove(&account_id);
        self.pending_thread_summaries
            .retain(|(pending_account, _), _| *pending_account != account_id);
        let folders_by_account = &self.folders_by_account;
        let page_keys = &self.page_keys;
        let selected_key = self.selected_key.as_ref();
        self.messages.retain(|message| {
            message.account_id != account_id
                || message_key_for_folders(folders_by_account, message)
                    .is_some_and(|key| page_keys.contains(&key) || selected_key == Some(&key))
        });
        self.rebuild_visible();
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
        let previous = self
            .selected_row
            .as_ref()
            .filter(|row| row.key == key)
            .cloned();
        let selected_row = self
            .messages
            .iter()
            .find(|message| self.key_for(message).as_ref() == Some(&key))
            .map(|message| {
                let mut message = message.clone();
                if message.body.is_empty() {
                    if let Some(body) = selected_body {
                        message.body = body;
                    }
                }
                Arc::new(MessageRow { key, message })
            });
        self.selected_row = selected_row.or(previous);
    }

    fn clear_current_page(&mut self) {
        self.clear_selection();
    }

    fn clear_selection(&mut self) {
        self.selected_key = None;
        self.selected_row = None;
        self.body_loading = false;
        self.links.clear();
        self.attachment_state = AttachmentState::NotLoaded;
        self.pending_attachment = None;
        self.draft_source = None;
    }

    fn clear_all_page_data(&mut self) {
        for state in self.page_states.values_mut() {
            state.generation = state.generation.wrapping_add(1).max(1);
            state.pending_generation = None;
            state.pending_append = false;
            state.next_cursor = None;
            state.truncated = false;
        }
        self.messages.clear();
        self.page_keys.clear();
        self.visible_indices.clear();
        self.page_rows = Arc::new(Vec::new());
        self.conversation_rows = Arc::new(Vec::new());
        self.related_members.clear();
        self.related_counts.clear();
        self.related_requests.clear();
        self.thread_summary_requests.clear();
        self.pending_thread_summaries.clear();
        self.pending_related.clear();
        self.conversation_errors.clear();
        self.conversation_warnings.clear();
        self.sent_header_requests.clear();
        self.pending_sent_refresh.clear();
        self.pending_message_refresh.clear();
        self.query_errors.clear();
        self.truncated = false;
        self.loading_more = false;
        self.demo_links_by_key.clear();
        self.clear_selection();
    }

    fn key_for(&self, message: &Message) -> Option<MessageKey> {
        message_key_for_folders(&self.folders_by_account, message)
    }

    fn selectable_key(&self, key: &MessageKey) -> bool {
        self.page_rows.iter().any(|row| row.key == *key)
            || self
                .conversation_rows
                .iter()
                .any(|conversation| conversation.members.iter().any(|row| row.key == *key))
    }

    fn active_page_keys(&self) -> Vec<PageKey> {
        let mut keys = match self.scope {
            MailboxScope::Account => self
                .current_account_id
                .zip(self.current_folder_path.as_ref())
                .map(|(account_id, path)| {
                    vec![PageKey {
                        account_id,
                        folder_path: path.clone(),
                    }]
                })
                .unwrap_or_default(),
            MailboxScope::Unified => self
                .accounts
                .iter()
                .flat_map(|account| {
                    self.folders_by_account
                        .get(&account.id)
                        .into_iter()
                        .flatten()
                        .filter(|folder| folder.kind == FolderKind::Inbox)
                        .map(|folder| PageKey {
                            account_id: account.id,
                            folder_path: folder.path.clone(),
                        })
                })
                .collect(),
        };
        keys.sort_by(|a, b| {
            a.account_id
                .cmp(&b.account_id)
                .then_with(|| a.folder_path.cmp(&b.folder_path))
        });
        keys.dedup();
        keys
    }

    fn active_account_ids(&self) -> Vec<u32> {
        match self.scope {
            MailboxScope::Account => self.current_account_id.into_iter().collect(),
            MailboxScope::Unified => self.accounts.iter().map(|account| account.id).collect(),
        }
    }

    fn account_loading_snapshot(&self) -> HashMap<u32, bool> {
        self.active_account_ids()
            .into_iter()
            .map(|account_id| {
                let keys: Vec<_> = self
                    .active_page_keys()
                    .into_iter()
                    .filter(|key| key.account_id == account_id)
                    .collect();
                let loading = if keys.is_empty() {
                    account_waiting_for_folders(
                        self.folders_by_account.contains_key(&account_id),
                        self.account_has_loading_error(account_id),
                    )
                } else {
                    keys.iter().any(|key| {
                        self.page_states.get(key).map_or(self.loading, |state| {
                            state.pending_generation.is_some() && !state.pending_append
                        })
                    })
                };
                (account_id, loading)
            })
            .collect()
    }

    fn account_has_loading_error(&self, account_id: u32) -> bool {
        self.query_errors.contains_key(&account_id)
            || self.connectivity_errors.contains_key(&account_id)
            || self.action_errors.contains_key(&account_id)
    }

    fn account_errors_snapshot(&self) -> HashMap<u32, String> {
        self.active_account_ids()
            .into_iter()
            .filter_map(|account_id| {
                self.query_errors
                    .get(&account_id)
                    .or_else(|| self.connectivity_errors.get(&account_id))
                    .cloned()
                    .map(|error| (account_id, error))
            })
            .collect()
    }

    fn scope_title(&self) -> String {
        if self.scope == MailboxScope::Unified {
            return "Unified Inbox".into();
        }
        self.current_folder_path
            .as_ref()
            .and_then(|path| {
                self.current_account_id.and_then(|account_id| {
                    self.folders_by_account
                        .get(&account_id)?
                        .iter()
                        .find(|folder| &folder.path == path)
                        .map(|folder| folder.name.clone())
                })
            })
            .or_else(|| {
                self.current_account_id.and_then(|account_id| {
                    self.accounts
                        .iter()
                        .find(|account| account.id == account_id)
                        .map(|account| account.label.clone())
                })
            })
            .unwrap_or_else(|| "Inbox".into())
    }

    fn active_page_message_count(&self) -> usize {
        let active: HashSet<_> = self.active_page_keys().into_iter().collect();
        self.page_keys
            .iter()
            .filter(|key| {
                active.contains(&PageKey {
                    account_id: key.account_id,
                    folder_path: key.folder_path.clone(),
                })
            })
            .count()
    }

    fn page_message_count(&self, page: &PageKey) -> usize {
        self.page_keys
            .iter()
            .filter(|key| key.account_id == page.account_id && key.folder_path == page.folder_path)
            .count()
    }

    fn update_loading_state(&mut self) {
        let keys = self.active_page_keys();
        let pages_loading = page_requests_loading(&keys, &self.page_states);
        let active_accounts = self.active_account_ids();
        let folders_loading = if active_accounts.is_empty() && self.scope == MailboxScope::Unified {
            self.request_senders.keys().any(|account_id| {
                account_waiting_for_folders(
                    self.folders_by_account.contains_key(account_id),
                    self.account_has_loading_error(*account_id),
                )
            })
        } else {
            active_accounts.iter().any(|account_id| {
                account_waiting_for_folders(
                    self.folders_by_account.contains_key(account_id),
                    self.account_has_loading_error(*account_id),
                )
            })
        };
        self.loading = pages_loading || folders_loading;
        self.loading_more = keys.iter().any(|key| {
            self.page_states
                .get(key)
                .is_some_and(|state| state.pending_generation.is_some() && state.pending_append)
        });
        self.truncated = keys.iter().any(|key| {
            self.page_states
                .get(key)
                .is_some_and(|state| state.truncated)
        });
    }

    fn folder_kind(&self, account_id: u32, folder_id: u32) -> Option<FolderKind> {
        self.folders_by_account
            .get(&account_id)?
            .iter()
            .find(|folder| folder.id == folder_id)
            .map(|folder| folder.kind)
    }

    fn remove_page_partition(&mut self, page: &PageKey) {
        let keys: HashSet<_> = self
            .page_keys
            .iter()
            .filter(|key| key.account_id == page.account_id && key.folder_path == page.folder_path)
            .cloned()
            .collect();
        self.page_keys.retain(|key| !keys.contains(key));
        let folders_by_account = &self.folders_by_account;
        self.messages.retain(|message| {
            message_key_for_folders(folders_by_account, message)
                .is_none_or(|key| !keys.contains(&key))
        });
        self.related_members
            .retain(|key, _| key.account_id != page.account_id);
        self.related_counts
            .retain(|key, _| key.account_id != page.account_id);
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
        self.active_page_keys()
            .iter()
            .any(|key| key.account_id == account_id && key.folder_path == path)
    }

    fn set_action_error(&mut self, text: impl Into<String>) {
        if let Some(account_id) = self
            .selected_key
            .as_ref()
            .map(|key| key.account_id)
            .or(self.current_account_id)
        {
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

fn message_key_for_folders(
    folders_by_account: &HashMap<u32, Vec<Folder>>,
    message: &Message,
) -> Option<MessageKey> {
    let path = folders_by_account
        .get(&message.account_id)?
        .iter()
        .find(|folder| folder.id == message.folder_id)?
        .path
        .as_str();
    Some(message_key(message.account_id, path, message.uid))
}

fn replace_message_header(existing: &mut Message, mut replacement: Message) {
    if replacement.body.is_empty() {
        replacement.body = std::mem::take(&mut existing.body);
    }
    *existing = replacement;
}

fn physical_message_key(message: &Message) -> (u32, u32, u32) {
    (message.account_id, message.folder_id, message.uid)
}

fn stable_conversation_key(
    members: &[Message],
    derived_key: ConversationKey,
    previous_rows: &[ConversationRow],
    selected_key: Option<&MessageKey>,
    reused_keys: &mut HashSet<ConversationKey>,
) -> ConversationKey {
    let mut candidates = previous_rows
        .iter()
        .filter(|previous| previous.key.account_id == derived_key.account_id)
        .filter(|previous| {
            let representative_key = physical_message_key(&previous.representative.message);
            members
                .iter()
                .any(|member| physical_message_key(member) == representative_key)
        })
        .map(|previous| previous.key.clone())
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| left.anchor.cmp(&right.anchor));
    candidates.dedup();

    let selected_candidate = selected_key.and_then(|selected| {
        previous_rows
            .iter()
            .filter(|previous| previous.key.account_id == derived_key.account_id)
            .filter(|previous| {
                previous
                    .members
                    .iter()
                    .any(|member| &member.key == selected)
            })
            .map(|previous| previous.key.clone())
            .filter(|candidate| candidates.contains(candidate))
            .filter(|candidate| !reused_keys.contains(candidate))
            .min_by(|left, right| left.anchor.cmp(&right.anchor))
    });
    if let Some(previous_key) = selected_candidate.or_else(|| {
        candidates
            .into_iter()
            .find(|candidate| !reused_keys.contains(candidate))
    }) {
        reused_keys.insert(previous_key.clone());
        previous_key
    } else {
        derived_key
    }
}

fn remap_demo_event(account_id: u32, mut event: WorkerMailboxEvent) -> WorkerMailboxEvent {
    if account_id == 2 {
        if let WorkerMailboxEvent::DemoMessages {
            messages,
            links_by_uid,
            ..
        } = &mut event
        {
            for message in messages.iter_mut() {
                message.uid = message.uid.saturating_sub(29);
            }
            let links = std::mem::take(links_by_uid);
            *links_by_uid = links
                .into_iter()
                .map(|(uid, links)| (uid.saturating_sub(29), links))
                .collect();
        }
    }
    event
}

fn demo_team_sent_reply() -> Message {
    Message {
        id: 1_031,
        account_id: 2,
        folder_id: 13,
        uid: 2,
        from_name: "Hyprlab".into(),
        from_addr: "hello@hyprlab.dev".into(),
        reply_to: String::new(),
        to: "no-reply@buymeacoffee.com".into(),
        cc: String::new(),
        subject: "Re: You have a new supporter ☕".into(),
        preview: "Thank you for supporting the project — it means a lot to the team.".into(),
        body: "Thanks for the thoughtful note, Alex. Your support helps us keep building the mail client we've wanted for years.\n\n— Hyprlab\n\nOn Yesterday, Alex wrote:\n> Thanks for building this.".into(),
        date: "Yesterday, 10:42 AM".into(),
        timestamp: 1_759_892_000,
        unread: false,
        starred: false,
        keywords: Vec::new(),
        has_attachment: false,
        message_id: "<demo-team-supporter-reply@hylki.local>".into(),
        references: "<demo-31@hylki.local>".into(),
        importance: megamail_core::models::Importance::Normal,
        due: 0,
    }
}

fn demo_long_thread_reply() -> Message {
    Message {
        id: 1_090,
        account_id: 1,
        folder_id: 1,
        uid: 1_090,
        from_name: "Priya Sharma".into(),
        from_addr: "priya@studio.dev".into(),
        reply_to: String::new(),
        to: "jason@hylki.hyprlab.co".into(),
        cc: String::new(),
        subject: "Re: Reader redesign: final review".into(),
        preview: "One last pass: the keyboard focus now stays with the conversation as you move between messages.".into(),
        body: "One last pass: keyboard focus now stays with the conversation as you move between messages. I checked the reader at laptop height, too, and the longer thread scrolls cleanly.\n\nPriya".into(),
        date: "Today, 11:06 AM".into(),
        timestamp: 1_760_003_960,
        unread: false,
        starred: false,
        keywords: Vec::new(),
        has_attachment: false,
        message_id: "<demo-1090@hylki.local>".into(),
        references: "<demo-1@hylki.local>".into(),
        importance: megamail_core::models::Importance::Normal,
        due: 0,
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
        if let Some(event) =
            compact_worker_event(event, true).map(|event| remap_demo_event(account_id, event))
        {
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
        WorkerEvent::Related {
            message_id,
            messages,
        } => WorkerMailboxEvent::Related {
            message_id,
            messages,
            partial: false,
            warning: None,
        },
        WorkerEvent::ThreadSummaries { summaries } => WorkerMailboxEvent::ThreadSummaries {
            summaries,
            warning: None,
        },
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

fn reconnectable_thunderbird_ids(
    thunderbird_accounts: &HashSet<u32>,
    reconnectable_ids: &HashSet<u32>,
) -> HashSet<u32> {
    thunderbird_accounts
        .intersection(reconnectable_ids)
        .copied()
        .collect()
}

fn page_requests_loading(
    keys: &[PageKey],
    page_states: &HashMap<PageKey, FolderPageState>,
) -> bool {
    keys.iter().any(|key| {
        page_states
            .get(key)
            .is_some_and(|state| state.pending_generation.is_some() && !state.pending_append)
    })
}

fn is_reconnectable_thunderbird_id(
    account_id: u32,
    thunderbird_accounts: &HashSet<u32>,
    reconnectable_ids: &HashSet<u32>,
) -> bool {
    thunderbird_accounts.contains(&account_id) && reconnectable_ids.contains(&account_id)
}

fn is_reconnectable_thunderbird_failure(error: &str) -> bool {
    if crate::thunderbird_adapter::is_broken_bridge_error(error) {
        return true;
    }
    let error = error.to_ascii_lowercase();
    [
        "worker disconnected",
        "session disconnected",
        "worker is unavailable",
        "worker stopped",
        "session stopped",
        "session is shutting down",
        "runtime stopped",
        "stopped unexpectedly",
    ]
    .iter()
    .any(|pattern| error.contains(pattern))
}

fn account_waiting_for_folders(folders_loaded: bool, has_error: bool) -> bool {
    !folders_loaded && !has_error
}

fn finish_superseded_page(state: &mut FolderPageState, append: bool) {
    state.pending_generation = None;
    state.pending_append = false;
    if append {
        state.next_cursor = None;
        state.truncated = false;
    }
}

fn clear_pending_pages_if_transport_closed(
    page_states: &mut HashMap<PageKey, FolderPageState>,
    account_id: u32,
    is_thunderbird: bool,
    error: &str,
) -> bool {
    if !is_thunderbird || !is_reconnectable_thunderbird_failure(error) {
        return false;
    }
    let mut changed = false;
    for (key, state) in page_states {
        if key.account_id == account_id {
            changed |= state.pending_generation.take().is_some();
            changed |= state.pending_append;
            state.pending_append = false;
        }
    }
    changed
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

fn page_result_is_current(
    key: &PageKey,
    state: &FolderPageState,
    result: &PageResult,
    current_folder_id: Option<u32>,
) -> bool {
    key.account_id == result.account_id
        && key.folder_path == result.folder_path
        && state.pending_generation == Some(result.generation)
        && state.folder_id == result.folder_id
        && current_folder_id == Some(result.folder_id)
}

fn active_representative<'a>(
    members: &[Message],
    active_page: &'a [Message],
    fallback: &'a Message,
) -> &'a Message {
    active_page
        .iter()
        .filter(|candidate| {
            members
                .iter()
                .any(|member| same_mail_copy(candidate, member))
        })
        .max_by(|left, right| {
            left.timestamp
                .cmp(&right.timestamp)
                .then_with(|| left.folder_id.cmp(&right.folder_id))
                .then_with(|| left.uid.cmp(&right.uid))
        })
        .unwrap_or(fallback)
}

fn same_mail_copy(left: &Message, right: &Message) -> bool {
    physical_message_key(left) == physical_message_key(right)
        || (left.account_id == right.account_id
            && normalized_message_id(&left.message_id)
                .zip(normalized_message_id(&right.message_id))
                .is_some_and(|(left_id, right_id)| left_id == right_id)
            && left.from_addr == right.from_addr
            && left.timestamp == right.timestamp)
}

fn normalized_message_id(message_id: &str) -> Option<String> {
    let message_id = message_id
        .trim()
        .trim_start_matches('<')
        .trim_end_matches('>');
    (!message_id.is_empty()).then(|| message_id.to_ascii_lowercase())
}

fn conversation_response_warning(partial: bool, warning: Option<String>) -> Option<String> {
    if partial {
        Some(
            warning.unwrap_or_else(|| {
                "More messages may exist outside the indexed/query window".into()
            }),
        )
    } else {
        warning
    }
}

fn message_matches_filter(message: &Message, filter: MessageFilter) -> bool {
    match filter {
        MessageFilter::All => true,
        MessageFilter::Unread => message.unread,
        MessageFilter::Starred => message.starred,
        MessageFilter::Attachments => message.has_attachment,
    }
}

fn message_matches_query(message: &Message, query: &str) -> bool {
    query.is_empty()
        || message.from_name.to_lowercase().contains(query)
        || message.from_addr.to_lowercase().contains(query)
        || message.subject.to_lowercase().contains(query)
        || message.preview.to_lowercase().contains(query)
}

fn message_matches_thread_ids(message: &Message, ids: &[String]) -> bool {
    ids.iter().any(|id| {
        id == &message.message_id
            || message
                .references
                .split_whitespace()
                .any(|reference| reference == id)
    })
}

fn visible_message_indices(messages: &[Message], query: &str, filter: MessageFilter) -> Vec<usize> {
    messages
        .iter()
        .enumerate()
        .filter(|(_, message)| {
            message_matches_query(message, query) && message_matches_filter(message, filter)
        })
        .map(|(index, _)| index)
        .collect()
}

fn body_event_matches(
    event_account_id: u32,
    event_path: &str,
    event_message_id: u32,
    selected_key: Option<&MessageKey>,
    pending: &PendingBody,
) -> bool {
    event_account_id == pending.key.account_id
        && event_path == pending.key.folder_path
        && event_message_id == pending.message_id
        && selected_key == Some(&pending.key)
}

fn remove_failed_body_request(
    pending_bodies: &mut HashMap<BodyRequestSlot, PendingBody>,
    account_id: u32,
    path: &str,
    message_id: u32,
    selected_key: Option<&MessageKey>,
) -> bool {
    let slot = BodyRequestSlot {
        account_id,
        folder_path: path.to_owned(),
        message_id,
    };
    pending_bodies.remove(&slot).is_some_and(|pending| {
        body_event_matches(account_id, path, message_id, selected_key, &pending)
    })
}

fn retain_selected_body_after_error(
    pending_bodies: &mut HashMap<BodyRequestSlot, PendingBody>,
    failed_account_id: u32,
    selected_key: Option<&MessageKey>,
) {
    pending_bodies.retain(|slot, pending| {
        slot.account_id != failed_account_id || selected_key == Some(&pending.key)
    });
}

fn attachment_response_matches(
    event_account_id: u32,
    event_path: &str,
    event_message_id: u32,
    selected_key: Option<&MessageKey>,
    pending: &PendingAttachment,
    message_present: bool,
) -> bool {
    message_present
        && event_account_id == pending.key.account_id
        && event_path == pending.key.folder_path
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
        ATTACHMENT_BYTES_CAP, ATTACHMENT_COUNT_CAP, BodyRequestSlot, ConversationKey,
        ConversationRow, DraftSourceSnapshot, DraftSourceState, FolderPageState, MESSAGE_CAP,
        MessageFilter, MessageRow, PAGE_SIZE, PageKey, PendingAttachment, PendingBody,
        account_waiting_for_folders, active_representative, attachment_response_matches,
        body_event_matches, bounded_attachments, bounded_attachments_with_limits,
        clear_pending_pages_if_transport_closed, conversation_response_warning,
        draft_export_matches, finish_superseded_page, is_reconnectable_thunderbird_failure,
        is_reconnectable_thunderbird_id, make_page_request, merge_message_batch,
        merge_message_batch_ordered, message_key, page_requests_loading, page_result_is_current,
        plain_body_and_links, reconnectable_thunderbird_ids, remove_failed_body_request,
        retain_selected_body_after_error, set_folder_unread, stable_conversation_key,
        visible_message_indices,
    };
    use megamail_core::cache::MessageCursor;
    use megamail_core::models::{Attachment, DraftOrigin, Folder, FolderKind, Importance, Message};
    use megamail_core::query::{PageResult, PageStatus};
    use std::collections::{HashMap, HashSet};
    use std::sync::Arc;

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
            body_event_matches(account, path, 12, Some(&selected), &pending)
        };

        assert!(matches(7, "INBOX", 42));
        assert!(!matches(8, "INBOX", 42));
        assert!(!matches(7, "Archive", 42));
        assert!(!matches(7, "INBOX", 43));
        assert!(!body_event_matches(7, "INBOX", 13, Some(&key), &pending));
    }

    #[test]
    fn unrelated_same_account_body_error_preserves_selected_pending_response() {
        let failed = message_key(7, "INBOX", 42);
        let selected = message_key(7, "Archive", 43);
        let other_account = message_key(8, "INBOX", 44);
        let mut pending = HashMap::from([
            (
                BodyRequestSlot {
                    account_id: 7,
                    folder_path: "INBOX".into(),
                    message_id: 12,
                },
                PendingBody {
                    key: failed.clone(),
                    message_id: 12,
                },
            ),
            (
                BodyRequestSlot {
                    account_id: 7,
                    folder_path: "Archive".into(),
                    message_id: 13,
                },
                PendingBody {
                    key: selected.clone(),
                    message_id: 13,
                },
            ),
            (
                BodyRequestSlot {
                    account_id: 8,
                    folder_path: "INBOX".into(),
                    message_id: 14,
                },
                PendingBody {
                    key: other_account.clone(),
                    message_id: 14,
                },
            ),
        ]);

        retain_selected_body_after_error(&mut pending, 7, Some(&selected));

        assert!(!pending.values().any(|slot| slot.key == failed));
        assert!(pending.values().any(|slot| slot.key == selected));
        assert!(pending.values().any(|slot| slot.key == other_account));
        retain_selected_body_after_error(&mut pending, 7, None);
        assert!(!pending.values().any(|slot| slot.key == selected));
        assert!(pending.values().any(|slot| slot.key == other_account));
    }

    #[test]
    fn stale_body_failure_removes_only_its_request_and_preserves_new_selection() {
        let failed_key = message_key(7, "INBOX", 42);
        let selected_key = message_key(7, "Archive", 43);
        let failed_slot = BodyRequestSlot {
            account_id: 7,
            folder_path: "INBOX".into(),
            message_id: 12,
        };
        let selected_slot = BodyRequestSlot {
            account_id: 7,
            folder_path: "Archive".into(),
            message_id: 13,
        };
        let mut pending = HashMap::from([
            (
                failed_slot.clone(),
                PendingBody {
                    key: failed_key,
                    message_id: 12,
                },
            ),
            (
                selected_slot.clone(),
                PendingBody {
                    key: selected_key.clone(),
                    message_id: 13,
                },
            ),
        ]);

        assert!(!remove_failed_body_request(
            &mut pending,
            7,
            "INBOX",
            12,
            Some(&selected_key),
        ));
        assert!(!pending.contains_key(&failed_slot));
        assert!(pending.contains_key(&selected_slot));
        assert!(remove_failed_body_request(
            &mut pending,
            7,
            "Archive",
            13,
            Some(&selected_key),
        ));
        assert!(pending.is_empty());
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
    fn account_and_folder_are_part_of_message_and_page_keys() {
        let first = message_key(7, "INBOX", 42);
        let second = message_key(8, "INBOX", 42);
        let third = message_key(7, "Archive", 42);
        assert_ne!(first, second);
        assert_ne!(first, third);

        let first_page = PageKey {
            account_id: 7,
            folder_path: "INBOX".into(),
        };
        let second_page = PageKey {
            account_id: 8,
            folder_path: "INBOX".into(),
        };
        let third_page = PageKey {
            account_id: 7,
            folder_path: "Archive".into(),
        };
        let pages = HashMap::from([
            (
                first_page.clone(),
                FolderPageState {
                    next_cursor: Some(MessageCursor { before_uid: 101 }),
                    ..FolderPageState::default()
                },
            ),
            (
                second_page.clone(),
                FolderPageState {
                    next_cursor: Some(MessageCursor { before_uid: 202 }),
                    ..FolderPageState::default()
                },
            ),
            (
                third_page.clone(),
                FolderPageState {
                    next_cursor: Some(MessageCursor { before_uid: 303 }),
                    ..FolderPageState::default()
                },
            ),
        ]);

        assert_ne!(first_page, second_page);
        assert_ne!(first_page, third_page);
        assert_eq!(pages[&first_page].next_cursor.unwrap().before_uid, 101);
        assert_eq!(pages[&second_page].next_cursor.unwrap().before_uid, 202);
        assert_eq!(pages[&third_page].next_cursor.unwrap().before_uid, 303);
    }

    #[test]
    fn page_result_requires_the_current_folder_generation() {
        let key = PageKey {
            account_id: 7,
            folder_path: "INBOX".into(),
        };
        let state = FolderPageState {
            folder_id: 3,
            pending_generation: Some(9),
            ..FolderPageState::default()
        };
        let result = |account_id, generation| PageResult {
            account_id,
            folder_path: "INBOX".into(),
            folder_id: 3,
            generation,
            rows: Vec::new(),
            next_cursor: None,
            status: PageStatus::Ready,
        };

        assert!(page_result_is_current(&key, &state, &result(7, 9), Some(3)));
        assert!(!page_result_is_current(
            &key,
            &state,
            &result(7, 8),
            Some(3)
        ));
        assert!(!page_result_is_current(
            &key,
            &state,
            &result(8, 9),
            Some(3)
        ));
        assert!(!page_result_is_current(
            &key,
            &state,
            &result(7, 9),
            Some(4)
        ));
    }

    #[test]
    fn only_failed_thunderbird_accounts_are_reconnectable_by_stable_id() {
        let thunderbird_accounts = HashSet::from([7]);
        let reconnectable = HashSet::from([7, 8]);

        assert_eq!(
            reconnectable_thunderbird_ids(&thunderbird_accounts, &reconnectable),
            HashSet::from([7])
        );
        assert!(is_reconnectable_thunderbird_id(
            7,
            &thunderbird_accounts,
            &reconnectable
        ));
        assert!(!is_reconnectable_thunderbird_id(
            8,
            &thunderbird_accounts,
            &reconnectable
        ));
        assert!(is_reconnectable_thunderbird_failure("Broken pipe"));
        assert!(is_reconnectable_thunderbird_failure("socket closed"));
        assert!(is_reconnectable_thunderbird_failure(
            "Thunderbird bridge is disconnected. Restart the account runtime before trying again."
        ));
        assert!(is_reconnectable_thunderbird_failure(
            "Thunderbird bridge disconnected during startup."
        ));
        assert!(is_reconnectable_thunderbird_failure(
            "Thunderbird bridge closed the connection."
        ));
        assert!(is_reconnectable_thunderbird_failure(
            "The shared Thunderbird bridge stopped. Reconnect this profile to resume mail."
        ));
        assert!(is_reconnectable_thunderbird_failure(
            "The Thunderbird worker session stopped."
        ));
        assert!(is_reconnectable_thunderbird_failure(
            "The native messaging host has exited."
        ));
        assert!(!is_reconnectable_thunderbird_failure("offline"));
        assert!(!is_reconnectable_thunderbird_failure("bad search query"));
    }

    #[test]
    fn failed_refresh_submission_does_not_leave_page_loading_without_pending_work() {
        let key = PageKey {
            account_id: 7,
            folder_path: "INBOX".into(),
        };
        let state = FolderPageState {
            generation: 4,
            pending_generation: None,
            pending_append: false,
            ..FolderPageState::default()
        };

        assert!(!page_requests_loading(
            &[key.clone()],
            &HashMap::from([(key, state)])
        ));
    }

    #[test]
    fn initial_folder_loading_stops_after_error_or_folder_response() {
        assert!(account_waiting_for_folders(false, false));
        assert!(!account_waiting_for_folders(false, true));
        assert!(!account_waiting_for_folders(true, false));
    }

    #[test]
    fn superseded_append_clears_stale_cursor_without_restarting_the_page() {
        let mut state = FolderPageState {
            generation: 9,
            pending_generation: Some(9),
            pending_append: true,
            next_cursor: Some(MessageCursor { before_uid: 101 }),
            truncated: true,
            ..FolderPageState::default()
        };

        finish_superseded_page(&mut state, true);

        assert_eq!(state.pending_generation, None);
        assert!(!state.pending_append);
        assert_eq!(state.next_cursor, None);
        assert!(!state.truncated);
    }

    #[test]
    fn transport_closure_releases_only_that_accounts_pending_pages() {
        let inbox = PageKey {
            account_id: 7,
            folder_path: "INBOX".into(),
        };
        let archive = PageKey {
            account_id: 7,
            folder_path: "Archive".into(),
        };
        let other_account = PageKey {
            account_id: 8,
            folder_path: "INBOX".into(),
        };
        let mut states = HashMap::from([
            (
                inbox.clone(),
                FolderPageState {
                    pending_generation: Some(3),
                    pending_append: true,
                    ..FolderPageState::default()
                },
            ),
            (
                archive.clone(),
                FolderPageState {
                    pending_generation: Some(4),
                    ..FolderPageState::default()
                },
            ),
            (
                other_account.clone(),
                FolderPageState {
                    pending_generation: Some(5),
                    pending_append: true,
                    ..FolderPageState::default()
                },
            ),
        ]);

        assert!(!clear_pending_pages_if_transport_closed(
            &mut states,
            7,
            true,
            "The body request timed out.",
        ));
        assert_eq!(states[&inbox].pending_generation, Some(3));
        assert!(states[&inbox].pending_append);

        assert!(clear_pending_pages_if_transport_closed(
            &mut states,
            7,
            true,
            "Thunderbird bridge is disconnected. Restart Thunderbird, then click Retry.",
        ));
        assert_eq!(states[&inbox].pending_generation, None);
        assert_eq!(states[&archive].pending_generation, None);
        assert!(!states[&inbox].pending_append);
        assert!(!states[&archive].pending_append);
        assert_eq!(states[&other_account].pending_generation, Some(5));
        assert!(states[&other_account].pending_append);
    }

    #[test]
    fn sent_reply_does_not_replace_the_active_inbox_representative() {
        let inbox = message(1, 7, 1, 1, "Conversation", 100);
        let mut sent = message(2, 7, 3, 2, "Re: Conversation", 200);
        sent.references = "<parent@example.test>".into();
        let members = [inbox.clone(), sent.clone()];
        let active_page = [inbox.clone()];

        assert_eq!(
            active_representative(&members, &active_page, &sent).uid,
            inbox.uid
        );
    }

    #[test]
    fn enriched_references_preserve_the_selected_conversation_ui_key() {
        let mut reply = message(1, 7, 1, 42, "Re: Conversation", 200);
        reply.message_id = "<reply@example.test>".into();
        let old_key = ConversationKey {
            account_id: 7,
            anchor: "reply@example.test".into(),
        };
        let previous = ConversationRow {
            key: old_key.clone(),
            representative: Arc::new(MessageRow {
                key: message_key(7, "INBOX", 42),
                message: reply.clone(),
            }),
            members: Arc::new(Vec::new()),
            count: 1,
            unread: 0,
            replied: false,
            forwarded: false,
            latest_sent: None,
        };
        let mut parent = message(2, 7, 1, 41, "Conversation", 100);
        parent.message_id = "<parent@example.test>".into();
        reply.references = "<parent@example.test>".into();
        let members = [parent, reply];
        let derived_key = ConversationKey {
            account_id: 7,
            anchor: "parent@example.test".into(),
        };
        let mut reused_keys = HashSet::new();

        let stable = stable_conversation_key(
            &members,
            derived_key.clone(),
            std::slice::from_ref(&previous),
            None,
            &mut reused_keys,
        );
        let warnings = HashMap::from([(old_key.clone(), "More messages may exist".to_string())]);
        let counts = HashMap::from([(old_key.clone(), 4usize)]);

        assert_eq!(stable, old_key);
        assert_eq!(
            warnings.get(&stable).map(String::as_str),
            Some("More messages may exist")
        );
        assert_eq!(counts.get(&stable), Some(&4));

        // A previous key is assigned to one resulting group at most.
        assert_eq!(
            stable_conversation_key(
                &members,
                derived_key.clone(),
                std::slice::from_ref(&previous),
                None,
                &mut reused_keys,
            ),
            derived_key
        );
    }

    #[test]
    fn merged_references_keep_the_selected_prior_thread_key() {
        let first_message = message(1, 7, 1, 42, "Re: Conversation", 200);
        let selected_message = message(2, 7, 1, 43, "Re: Conversation", 180);
        let selected_key = message_key(7, "INBOX", 43);
        let first_key = ConversationKey {
            account_id: 7,
            anchor: "a-prior-thread".into(),
        };
        let selected_conversation_key = ConversationKey {
            account_id: 7,
            anchor: "z-selected-thread".into(),
        };
        let previous_rows = vec![
            ConversationRow {
                key: first_key,
                representative: Arc::new(MessageRow {
                    key: message_key(7, "INBOX", 42),
                    message: first_message.clone(),
                }),
                members: Arc::new(vec![Arc::new(MessageRow {
                    key: message_key(7, "INBOX", 42),
                    message: first_message.clone(),
                })]),
                count: 1,
                unread: 0,
                replied: false,
                forwarded: false,
                latest_sent: None,
            },
            ConversationRow {
                key: selected_conversation_key.clone(),
                representative: Arc::new(MessageRow {
                    key: selected_key.clone(),
                    message: selected_message.clone(),
                }),
                members: Arc::new(vec![Arc::new(MessageRow {
                    key: selected_key.clone(),
                    message: selected_message.clone(),
                })]),
                count: 1,
                unread: 0,
                replied: false,
                forwarded: false,
                latest_sent: None,
            },
        ];
        let mut parent = message(3, 7, 1, 41, "Conversation", 100);
        parent.message_id = "<new-root@example.test>".into();
        let mut first_message = first_message;
        first_message.references = "<new-root@example.test>".into();
        let mut selected_message = selected_message;
        selected_message.references = "<new-root@example.test>".into();
        let enriched_members = [parent, first_message, selected_message];
        let derived_key = ConversationKey {
            account_id: 7,
            anchor: "new-root@example.test".into(),
        };

        let stable = stable_conversation_key(
            &enriched_members,
            derived_key,
            &previous_rows,
            Some(&selected_key),
            &mut HashSet::new(),
        );

        assert_eq!(stable, selected_conversation_key);
    }

    #[test]
    fn inbox_label_copy_remains_the_conversation_representative() {
        let mut inbox = message(1, 7, 1, 1, "Conversation", 100);
        let mut sent_copy = message(2, 7, 3, 2, "Conversation", 100);
        inbox.message_id = "<same@example.test>".into();
        sent_copy.message_id = "same@example.test".into();
        sent_copy.from_addr = inbox.from_addr.clone();

        assert_eq!(
            active_representative(&[sent_copy.clone()], &[inbox.clone()], &sent_copy).uid,
            inbox.uid
        );
    }

    #[test]
    fn partial_conversation_response_keeps_a_visible_warning() {
        assert_eq!(
            conversation_response_warning(true, None).as_deref(),
            Some("More messages may exist outside the indexed/query window")
        );
        assert_eq!(
            conversation_response_warning(true, Some("Limited result window".into())).as_deref(),
            Some("Limited result window")
        );
        assert_eq!(conversation_response_warning(false, None), None);
    }

    #[test]
    fn current_folder_filters_compose_with_search_and_preserve_page_order() {
        let mut messages = vec![
            message(1, 7, 1, 40, "Needle one", 4),
            message(2, 7, 1, 30, "Needle two", 3),
            message(3, 7, 1, 20, "Needle three", 2),
            message(4, 7, 1, 10, "Other", 1),
        ];
        messages[1].unread = false;
        messages[1].starred = true;
        messages[2].has_attachment = true;
        messages[3].starred = true;
        messages[3].has_attachment = true;

        assert_eq!(
            visible_message_indices(&messages, "", MessageFilter::All),
            [0, 1, 2, 3]
        );
        assert_eq!(
            visible_message_indices(&messages, "", MessageFilter::Unread),
            [0, 2, 3]
        );
        assert_eq!(
            visible_message_indices(&messages, "", MessageFilter::Starred),
            [1, 3]
        );
        assert_eq!(
            visible_message_indices(&messages, "", MessageFilter::Attachments),
            [2, 3]
        );
        assert_eq!(
            visible_message_indices(&messages, "needle", MessageFilter::Unread),
            [0, 2]
        );
        assert_eq!(
            messages
                .iter()
                .map(|message| message.uid)
                .collect::<Vec<_>>(),
            [40, 30, 20, 10]
        );
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

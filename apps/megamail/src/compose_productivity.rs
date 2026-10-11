//! Offline plain-text compose operations over the existing OutgoingMessage seam.
//! No dispatch, rich editor, spellcheck, MIME conversion, or incoming-mail persistence.
//! UI owners must call these operations on the actual prepared composer message.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use megamail_core::{config::AccountConfig, worker::OutgoingMessage};
use serde::{Deserialize, Serialize};

const VERSION: u32 = 1;
const MAX_ITEMS: usize = 128;
const MAX_TEXT: usize = 64 * 1024;
const MAX_FILE: usize = 1024 * 1024;
const MAX_BODY: usize = 2 * 1024 * 1024;
static TEMP_ID: AtomicU64 = AtomicU64::new(0);

/// Plain-text assets only. Templates may additionally supply a subject.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextKind {
    Signature,
    Template,
    Snippet,
}

/// User-authored text, scoped by stable app account ID, never by sender headers.
/// Do not populate this store automatically from incoming/encrypted messages.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextItem {
    pub id: String,
    pub account_id: u32,
    pub kind: TextKind,
    pub label: String,
    pub subject: Option<String>,
    pub body: String,
}

/// Errors deliberately contain no mail text, credentials, or filesystem paths.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProductivityError {
    InvalidText,
    LimitExceeded,
    WrongAccount,
    AccountDisabled,
    InvalidSender,
    UnsupportedHtml,
    MissingVariables,
    InvalidCursor,
    InvalidStore,
    Conflict,
    UnknownVersion(u32),
    UnsafePath,
    UnsupportedPlatform,
    Io(std::io::ErrorKind),
}

impl From<std::io::Error> for ProductivityError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error.kind())
    }
}

fn plain(value: &str, max: usize) -> Result<(), ProductivityError> {
    if value.len() > max {
        return Err(ProductivityError::LimitExceeded);
    }
    if value
        .chars()
        .any(|c| c.is_control() && c != '\n' && c != '\t')
    {
        return Err(ProductivityError::InvalidText);
    }
    Ok(())
}

fn header(value: &str) -> Result<(), ProductivityError> {
    plain(value, 998)?;
    if value.contains(['\n', '\t']) {
        return Err(ProductivityError::InvalidText);
    }
    Ok(())
}

fn validate_item(item: &TextItem) -> Result<(), ProductivityError> {
    if item.account_id == 0
        || item.id.is_empty()
        || item.id.len() > 64
        || !item
            .id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_'))
    {
        return Err(ProductivityError::InvalidText);
    }
    header(&item.label)?;
    if item.label.trim().is_empty() {
        return Err(ProductivityError::InvalidText);
    }
    plain(&item.body, MAX_TEXT)?;
    if let Some(subject) = &item.subject {
        if item.kind != TextKind::Template {
            return Err(ProductivityError::InvalidText);
        }
        header(subject)?;
    }
    Ok(())
}

/// A render result is reviewable; missing variables remain literal and block insertion.
/// Values are substituted once (not recursively), with no shell/HTML/date evaluation.
pub struct RenderedText {
    pub subject: Option<String>,
    pub body: String,
    pub missing: Vec<String>,
}

fn expand(
    text: &str,
    values: &BTreeMap<String, String>,
    missing: &mut BTreeSet<String>,
) -> Result<String, ProductivityError> {
    let mut output = String::new();
    let mut rest = text;
    while let Some(start) = rest.find("{{") {
        output.push_str(&rest[..start]);
        let tail = &rest[start + 2..];
        let end = tail.find("}}").ok_or(ProductivityError::InvalidText)?;
        let key = &tail[..end];
        if key.is_empty()
            || key.len() > 64
            || !key.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
        {
            return Err(ProductivityError::InvalidText);
        }
        if let Some(value) = values.get(key) {
            plain(value, MAX_TEXT)?;
            output.push_str(value);
        } else {
            missing.insert(key.to_owned());
            output.push_str(&rest[start..start + 2 + end + 2]);
        }
        if output.len() > MAX_TEXT {
            return Err(ProductivityError::LimitExceeded);
        }
        rest = &tail[end + 2..];
    }
    output.push_str(rest);
    plain(&output, MAX_TEXT)?;
    Ok(output)
}

/// Expand an explicitly chosen asset using caller-reviewed values. The numeric account
/// is supplied from trusted composer state, never inferred from incoming text.
pub fn render_item(
    item: &TextItem,
    account_id: u32,
    values: &BTreeMap<String, String>,
) -> Result<RenderedText, ProductivityError> {
    validate_item(item)?;
    if item.account_id != account_id {
        return Err(ProductivityError::WrongAccount);
    }
    if values.len() > 64 {
        return Err(ProductivityError::LimitExceeded);
    }
    let mut missing = BTreeSet::new();
    let body = expand(&item.body, values, &mut missing)?;
    let subject = item
        .subject
        .as_deref()
        .map(|subject| expand(subject, values, &mut missing))
        .transpose()?;
    if let Some(subject) = &subject {
        header(subject)?;
    }
    Ok(RenderedText {
        subject,
        body,
        missing: missing.into_iter().collect(),
    })
}

/// Insert without deleting existing text. `cursor` is a UTF-8 byte position; signature
/// insertion adds the standard delimiter. Subject replacement requires explicit consent.
/// Validation finishes before any mutation. All other OutgoingMessage fields survive.
/// HTML alternatives are rejected rather than silently becoming stale or being discarded.
pub fn insert_item(
    message: &mut OutgoingMessage,
    item: &TextItem,
    values: &BTreeMap<String, String>,
    cursor: usize,
    replace_subject: bool,
) -> Result<(), ProductivityError> {
    if !message.html.is_empty() {
        return Err(ProductivityError::UnsupportedHtml);
    }
    let rendered = render_item(item, message.from_account_id, values)?;
    if !rendered.missing.is_empty() {
        return Err(ProductivityError::MissingVariables);
    }
    if !message.body.is_char_boundary(cursor) {
        return Err(ProductivityError::InvalidCursor);
    }
    let insertion = if item.kind == TextKind::Signature {
        format!("\n\n-- \n{}", rendered.body)
    } else {
        rendered.body
    };
    if message.body.len().saturating_add(insertion.len()) > MAX_BODY {
        return Err(ProductivityError::LimitExceeded);
    }
    message.body.insert_str(cursor, &insertion);
    if replace_subject && let Some(subject) = rendered.subject {
        message.subject = subject;
    }
    Ok(())
}

/// Extract only the account's plain signature; do not reinterpret HTML as plain text.
pub fn configured_signature(
    account: &AccountConfig,
    account_id: u32,
) -> Result<Option<TextItem>, ProductivityError> {
    if account.signature_html && account.signature.as_ref().is_some_and(|s| !s.is_empty()) {
        return Err(ProductivityError::UnsupportedHtml);
    }
    let Some(body) = account.signature.as_ref().filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    let item = TextItem {
        id: "account-signature".into(),
        account_id,
        kind: TextKind::Signature,
        label: "Account signature".into(),
        subject: None,
        body: body.clone(),
    };
    validate_item(&item)?;
    Ok(Some(item))
}

/// Exact configured identity, not an arbitrary address copied from an email.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SenderChoice {
    Primary,
    Alias(String),
}

fn validate_identity(identity: &str) -> Result<(), ProductivityError> {
    header(identity).map_err(|_| ProductivityError::InvalidSender)?;
    let address = if let Some((name, rest)) = identity.split_once('<') {
        if name.contains('>') || !rest.ends_with('>') || rest[..rest.len() - 1].contains(['<', '>'])
        {
            return Err(ProductivityError::InvalidSender);
        }
        &rest[..rest.len() - 1]
    } else {
        identity
    };
    let address = address.trim();
    let Some((local, domain)) = address.split_once('@') else {
        return Err(ProductivityError::InvalidSender);
    };
    if local.is_empty()
        || domain.is_empty()
        || domain.contains('@')
        || address
            .chars()
            .any(|c| c.is_whitespace() || matches!(c, '<' | '>' | ',' | ';' | '"' | '\\'))
    {
        return Err(ProductivityError::InvalidSender);
    }
    Ok(())
}

/// Validate against the CURRENT account configuration at action time. This selects an
/// identity; it does not prove provider send-as authorization or alias SMTP readiness.
/// `account_id` binds the config passed by the owning account registry to the message.
pub fn choose_sender(
    message: &mut OutgoingMessage,
    account_id: u32,
    account: &AccountConfig,
    choice: &SenderChoice,
) -> Result<(), ProductivityError> {
    if account_id == 0 || message.from_account_id != account_id {
        return Err(ProductivityError::WrongAccount);
    }
    if !account.enabled || account.goa_mail_disabled {
        return Err(ProductivityError::AccountDisabled);
    }
    let alias = match choice {
        SenderChoice::Primary => {
            validate_identity(&account.email)?;
            None
        }
        SenderChoice::Alias(identity) => {
            if !account
                .aliases
                .iter()
                .any(|alias| alias.identity == *identity)
            {
                return Err(ProductivityError::InvalidSender);
            }
            validate_identity(identity)?;
            Some(identity.clone())
        }
    };
    message.from_alias = alias;
    Ok(())
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LibraryFile {
    version: u32,
    items: Vec<TextItem>,
}

/// Bounded versioned user-authored library. No AccountConfig, credentials, MIME, or
/// recovered mail is serialized. One store writer must serialize all edits.
#[derive(Clone, Default)]
pub struct TextLibrary {
    items: Vec<TextItem>,
}

impl TextLibrary {
    pub fn revision(&self) -> Result<Vec<u8>, ProductivityError> {
        self.encode()
    }
    /// Validate a replacement library; duplicate IDs within an account are rejected.
    pub fn new(items: Vec<TextItem>) -> Result<Self, ProductivityError> {
        if items.len() > MAX_ITEMS {
            return Err(ProductivityError::LimitExceeded);
        }
        let mut ids = BTreeSet::new();
        for item in &items {
            validate_item(item)?;
            if !ids.insert((item.account_id, &item.id)) {
                return Err(ProductivityError::InvalidStore);
            }
        }
        Ok(Self { items })
    }

    /// Edit one account without replacing another account's library. Scope comes from
    /// trusted settings state; rejection leaves all existing entries unchanged.
    pub fn upsert(&mut self, account_id: u32, item: TextItem) -> Result<(), ProductivityError> {
        validate_item(&item)?;
        if item.account_id != account_id {
            return Err(ProductivityError::WrongAccount);
        }
        if let Some(existing) = self
            .items
            .iter_mut()
            .find(|existing| existing.account_id == account_id && existing.id == item.id)
        {
            *existing = item;
        } else {
            if self.items.len() >= MAX_ITEMS {
                return Err(ProductivityError::LimitExceeded);
            }
            self.items.push(item);
        }
        Ok(())
    }

    /// Remove only an explicitly selected account's asset. No incoming email may call
    /// this as an automatic action; saving the resulting library is a separate step.
    pub fn remove(&mut self, account_id: u32, id: &str) -> bool {
        let before = self.items.len();
        self.items
            .retain(|item| item.account_id != account_id || item.id != id);
        before != self.items.len()
    }

    /// Only the selected account's assets are exposed to its picker.
    pub fn for_account(&self, account_id: u32) -> impl Iterator<Item = &TextItem> {
        self.items
            .iter()
            .filter(move |item| item.account_id == account_id)
    }

    /// Unknown versions are errors, not empty libraries. Callers retain the file for repair.
    pub fn decode(bytes: &[u8]) -> Result<Self, ProductivityError> {
        if bytes.len() > MAX_FILE {
            return Err(ProductivityError::LimitExceeded);
        }
        #[derive(Deserialize)]
        struct Version {
            version: u32,
        }
        let version: Version =
            serde_json::from_slice(bytes).map_err(|_| ProductivityError::InvalidStore)?;
        if version.version != VERSION {
            return Err(ProductivityError::UnknownVersion(version.version));
        }
        let file: LibraryFile =
            serde_json::from_slice(bytes).map_err(|_| ProductivityError::InvalidStore)?;
        Self::new(file.items)
    }

    /// Encoding and decoding use the same validation as the production file seam.
    pub fn encode(&self) -> Result<Vec<u8>, ProductivityError> {
        // Items cannot be mutated through the public interface after validation.
        let bytes = serde_json::to_vec(&LibraryFile {
            version: VERSION,
            items: self.items.clone(),
        })
        .map_err(|_| ProductivityError::InvalidStore)?;
        if bytes.len() > MAX_FILE {
            return Err(ProductivityError::LimitExceeded);
        }
        Ok(bytes)
    }
}

/// Inject an EXISTING private directory owned by MegaMail (or a fictional test directory).
/// No implicit environment/profile lookup or directory creation. Linux/Unix only.
/// The caller must guarantee ownership, trusted ancestors, and single-writer access;
/// this is not a defense against another process running as the same Unix user.
pub struct PrivateLibraryStore {
    directory: PathBuf,
    thunderbird: bool,
}

impl PrivateLibraryStore {
    pub fn new(directory: &Path) -> Result<Self, ProductivityError> {
        let store = Self {
            directory: directory.to_path_buf(),
            thunderbird: false,
        };
        store.validate_directory()?;
        Ok(store)
    }

    fn validate_directory(&self) -> Result<(), ProductivityError> {
        #[cfg(not(unix))]
        {
            return Err(ProductivityError::UnsupportedPlatform);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let meta = fs::symlink_metadata(&self.directory)?;
            if !self.directory.is_absolute()
                || !meta.is_dir()
                || meta.permissions().mode() & 0o077 != 0
            {
                return Err(ProductivityError::UnsafePath);
            }
            Ok(())
        }
    }

    fn path(&self) -> PathBuf {
        self.directory.join(if self.thunderbird {
            "compose-productivity.thunderbird.v1.json"
        } else {
            "compose-productivity.v1.json"
        })
    }

    /// Keep legacy direct preferences in place; Thunderbird uses a separate file.
    pub fn for_backend(directory: &Path, thunderbird: bool) -> Result<Self, ProductivityError> {
        let mut store = Self::new(directory)?;
        store.thunderbird = thunderbird;
        Ok(store)
    }

    fn lock(&self) -> Result<File, ProductivityError> {
        self.validate_directory()?;
        #[cfg(unix)]
        {
            use std::os::unix::{
                fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
                io::AsRawFd,
            };
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .mode(0o600)
                .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
                .open(self.path().with_extension("lock"))?;
            let meta = file.metadata()?;
            if !meta.is_file()
                || meta.permissions().mode() & 0o077 != 0
                || meta.uid() != unsafe { libc::geteuid() }
            {
                return Err(ProductivityError::UnsafePath);
            }
            if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
                return Err(std::io::Error::last_os_error().into());
            }
            Ok(file)
        }
        #[cfg(not(unix))]
        {
            Err(ProductivityError::UnsupportedPlatform)
        }
    }

    /// Compare the displayed snapshot while holding the same lock as all writers.
    pub fn apply(
        &self,
        expected: &[u8],
        account_id: u32,
        change: Option<TextItem>,
        remove: Option<&str>,
    ) -> Result<TextLibrary, ProductivityError> {
        let _lock = self.lock()?;
        let mut library = self.load()?.unwrap_or_default();
        if library.revision()? != expected {
            return Err(ProductivityError::Conflict);
        }
        match (change, remove) {
            (Some(item), None) => library.upsert(account_id, item)?,
            (None, Some(id)) if library.remove(account_id, id) => {}
            _ => return Err(ProductivityError::InvalidStore),
        }
        self.save_unlocked(&library)?;
        Ok(library)
    }

    /// Missing file is distinct from unreadable/corrupt/unknown version. Bounded read.
    pub fn load(&self) -> Result<Option<TextLibrary>, ProductivityError> {
        self.validate_directory()?;
        let path = self.path();
        let meta = match fs::symlink_metadata(&path) {
            Ok(meta) => meta,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        if !meta.is_file() {
            return Err(ProductivityError::UnsafePath);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if meta.permissions().mode() & 0o077 != 0 {
                return Err(ProductivityError::UnsafePath);
            }
        }
        if meta.len() > MAX_FILE as u64 {
            return Err(ProductivityError::LimitExceeded);
        }
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC);
        }
        let file = options.open(path)?;
        let opened = file.metadata()?;
        if !opened.is_file() {
            return Err(ProductivityError::UnsafePath);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::{MetadataExt, PermissionsExt};
            if opened.permissions().mode() & 0o077 != 0
                || opened.uid() != unsafe { libc::geteuid() }
            {
                return Err(ProductivityError::UnsafePath);
            }
        }
        let mut bytes = Vec::new();
        file.take((MAX_FILE + 1) as u64).read_to_end(&mut bytes)?;
        TextLibrary::decode(&bytes).map(Some)
    }

    /// Atomic private replacement, synced file AND directory. Refuses to overwrite a
    /// corrupt/unknown-version library. A directory-sync error may occur after rename:
    /// report it and reload, never claim durable success on that error.
    pub fn save(&self, library: &TextLibrary) -> Result<(), ProductivityError> {
        let _lock = self.lock()?;
        self.save_unlocked(library)
    }

    fn save_unlocked(&self, library: &TextLibrary) -> Result<(), ProductivityError> {
        self.load()?;
        let bytes = library.encode()?;
        let serial = TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let temp = self.directory.join(format!(
            ".compose-productivity.{}.{}.tmp",
            std::process::id(),
            serial
        ));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temp)?;
        let result = (|| {
            file.write_all(&bytes)?;
            file.sync_all()?;
            fs::rename(&temp, self.path())?;
            File::open(&self.directory)?.sync_all()?;
            Ok::<_, std::io::Error>(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temp);
        }
        result.map_err(Into::into)
    }
}

/// Contracts only: no timers, writes, or provider-side scheduling are implemented here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeferredIntent {
    SendLater { at: i64 },
    UndoSend { seconds: u16 },
    Snooze { until: i64 },
}

/// Evidence must come from actual owning queue/policy, not UI optimism or incoming text.
/// Full-payload recovery means Bcc, MIME/calendar alternatives, attachments and security
/// flags are durable and owned before eligibility can be advertised.
#[derive(Clone, Copy, Debug, Default)]
pub struct DeferredEvidence {
    pub account_id: u32,
    pub durable_payload: bool,
    pub full_payload_recoverable: bool,
    pub dispatch_while_app_closed: bool,
    pub durable_wakeup: bool,
    pub cancel_before_submission: bool,
    pub submission_started_or_uncertain: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeferredBlocker {
    WrongAccount,
    InvalidDeadline,
    NotDurable,
    RecoveryIncomplete,
    NoClosedAppPolicy,
    NoWakeupPolicy,
    CannotCancel,
    AlreadySubmitted,
}

/// Validate UTC Unix deadlines against an injected clock. Offline deadlines are not
/// promises of delivery: clock shifts, reconnect/late policy, idempotency and cancellation
/// races remain queue-owner obligations. Never maps elapsed time to fake success.
pub fn validate_deferred(
    account_id: u32,
    now: i64,
    intent: DeferredIntent,
    evidence: DeferredEvidence,
) -> Result<i64, DeferredBlocker> {
    if account_id == 0 || evidence.account_id != account_id {
        return Err(DeferredBlocker::WrongAccount);
    }
    let deadline = match intent {
        DeferredIntent::SendLater { at } => at,
        DeferredIntent::Snooze { until } => until,
        DeferredIntent::UndoSend { seconds } if (1..=120).contains(&seconds) => now
            .checked_add(i64::from(seconds))
            .ok_or(DeferredBlocker::InvalidDeadline)?,
        DeferredIntent::UndoSend { .. } => return Err(DeferredBlocker::InvalidDeadline),
    };
    if now < 0 || deadline <= now || deadline.saturating_sub(now) > 366 * 24 * 60 * 60 {
        return Err(DeferredBlocker::InvalidDeadline);
    }
    if evidence.submission_started_or_uncertain {
        return Err(DeferredBlocker::AlreadySubmitted);
    }
    if !evidence.durable_payload {
        return Err(DeferredBlocker::NotDurable);
    }
    if !evidence.full_payload_recoverable {
        return Err(DeferredBlocker::RecoveryIncomplete);
    }
    if !evidence.dispatch_while_app_closed {
        return Err(DeferredBlocker::NoClosedAppPolicy);
    }
    if !evidence.durable_wakeup {
        return Err(DeferredBlocker::NoWakeupPolicy);
    }
    if !matches!(intent, DeferredIntent::Snooze { .. }) && !evidence.cancel_before_submission {
        return Err(DeferredBlocker::CannotCancel);
    }
    Ok(deadline)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item() -> TextItem {
        TextItem {
            id: "reply".into(),
            account_id: 7,
            kind: TextKind::Template,
            label: "Reply".into(),
            subject: Some("For {{name}}".into()),
            body: "Hello {{name}}".into(),
        }
    }

    fn message() -> OutgoingMessage {
        OutgoingMessage {
            from_account_id: 7,
            from_alias: None,
            to: "to@example.invalid".into(),
            cc: String::new(),
            bcc: "hidden@example.invalid".into(),
            reply_to: "reply@example.invalid".into(),
            subject: "Original".into(),
            body: "Old 📨 text".into(),
            html: String::new(),
            attachments: vec!["fictional.bin".into()],
            in_reply_to: "parent".into(),
            references: "ancestor parent".into(),
            draft_origin: Some(megamail_core::models::DraftOrigin {
                account_id: 7,
                folder_id: 2,
                path: "Drafts".into(),
                uid: 11,
            }),
            outbox_origin: Some(4),
            sign: true,
            encrypt: true,
            send_at: Some(100),
            calendar: Some(megamail_core::worker::CalendarPart {
                ics: "BEGIN:VCALENDAR\nEND:VCALENDAR".into(),
                method: "REPLY".into(),
            }),
        }
    }

    #[test]
    fn insertion_preserves_recovery_fields_and_original_text() {
        let mut m = message();
        insert_item(
            &mut m,
            &item(),
            &BTreeMap::from([("name".into(), "Lee".into())]),
            0,
            false,
        )
        .unwrap();
        assert_eq!(m.body, "Hello LeeOld 📨 text");
        assert_eq!(
            (
                m.bcc.as_str(),
                m.reply_to.as_str(),
                m.references.as_str(),
                m.in_reply_to.as_str()
            ),
            (
                "hidden@example.invalid",
                "reply@example.invalid",
                "ancestor parent",
                "parent"
            )
        );
        assert_eq!(
            (
                m.subject.as_str(),
                m.outbox_origin,
                m.sign,
                m.encrypt,
                m.send_at
            ),
            ("Original", Some(4), true, true, Some(100))
        );
        assert_eq!(m.attachments, ["fictional.bin"]);
        let origin = m.draft_origin.as_ref().unwrap();
        assert_eq!(
            (
                origin.account_id,
                origin.folder_id,
                origin.path.as_str(),
                origin.uid
            ),
            (7, 2, "Drafts", 11)
        );
        let calendar = m.calendar.as_ref().unwrap();
        assert_eq!(
            (calendar.ics.as_str(), calendar.method.as_str()),
            ("BEGIN:VCALENDAR\nEND:VCALENDAR", "REPLY")
        );
    }

    #[test]
    fn signature_snippet_and_explicit_subject_use_same_insertion_seam() {
        let mut m = message();
        let mut account = AccountConfig::new("self@example.invalid");
        account.signature = Some("Lee".into());
        let signature = configured_signature(&account, 7).unwrap().unwrap();
        let end = m.body.len();
        insert_item(&mut m, &signature, &BTreeMap::new(), end, false).unwrap();
        assert!(m.body.ends_with("\n\n-- \nLee"));
        let snippet = TextItem {
            id: "thanks".into(),
            account_id: 7,
            kind: TextKind::Snippet,
            label: "Thanks".into(),
            subject: None,
            body: "Thanks. ".into(),
        };
        insert_item(&mut m, &snippet, &BTreeMap::new(), 0, false).unwrap();
        assert!(m.body.starts_with("Thanks. Old"));
        insert_item(
            &mut m,
            &item(),
            &BTreeMap::from([("name".into(), "Lee".into())]),
            0,
            true,
        )
        .unwrap();
        assert_eq!(m.subject, "For Lee");
    }

    #[test]
    fn rejected_operations_do_not_mutate_message() {
        for (mut asset, cursor, values) in [
            (item(), 0, BTreeMap::new()),
            (item(), 5, BTreeMap::from([("name".into(), "Lee".into())])),
            (
                item(),
                0,
                BTreeMap::from([("name".into(), "X\nBcc: x".into())]),
            ),
        ] {
            let mut m = message();
            let before = format!("{m:?}");
            assert!(insert_item(&mut m, &asset, &values, cursor, true).is_err());
            assert_eq!(before, format!("{m:?}"));
            asset.account_id = 9;
            assert!(render_item(&asset, 7, &values).is_err());
        }
    }

    #[test]
    fn variables_are_literal_nonrecursive_and_missing_are_sorted() {
        let rendered = render_item(
            &item(),
            7,
            &BTreeMap::from([("name".into(), "{{other}}".into())]),
        )
        .unwrap();
        assert_eq!(rendered.body, "Hello {{other}}");
        assert_eq!(
            render_item(&item(), 7, &BTreeMap::new()).unwrap().missing,
            ["name"]
        );
    }

    #[test]
    fn html_is_not_silently_lost_or_desynchronized() {
        let mut m = message();
        m.html = "<b>Original</b>".into();
        assert_eq!(
            insert_item(&mut m, &item(), &BTreeMap::new(), 0, true),
            Err(ProductivityError::UnsupportedHtml)
        );
        let mut account = AccountConfig::new("self@example.invalid");
        account.signature = Some("<img src='https://example.invalid/x'>".into());
        account.signature_html = true;
        assert!(matches!(
            configured_signature(&account, 7),
            Err(ProductivityError::UnsupportedHtml)
        ));
    }

    #[test]
    fn alias_revalidated_and_account_bound() {
        use megamail_core::config::AliasConfig;
        let mut m = message();
        let mut account = AccountConfig::new("self@example.invalid");
        let identity = "Lee <alias@example.invalid>";
        account.aliases.push(AliasConfig {
            identity: identity.into(),
            ..Default::default()
        });
        let choice = SenderChoice::Alias(identity.into());
        choose_sender(&mut m, 7, &account, &choice).unwrap();
        assert_eq!(m.from_alias.as_deref(), Some(identity));
        account.aliases.clear();
        assert_eq!(
            choose_sender(&mut m, 7, &account, &choice),
            Err(ProductivityError::InvalidSender)
        );
        assert_eq!(
            choose_sender(&mut m, 8, &account, &SenderChoice::Primary),
            Err(ProductivityError::WrongAccount)
        );
    }

    #[test]
    fn configured_alias_header_injection_is_rejected_without_fallback() {
        use megamail_core::config::AliasConfig;
        let mut m = message();
        let mut account = AccountConfig::new("self@example.invalid");
        let identity = "alias@example.invalid\r\nBcc: injected@example.invalid";
        account.aliases.push(AliasConfig {
            identity: identity.into(),
            ..Default::default()
        });
        let before = format!("{m:?}");
        assert_eq!(
            choose_sender(&mut m, 7, &account, &SenderChoice::Alias(identity.into())),
            Err(ProductivityError::InvalidSender)
        );
        assert_eq!(before, format!("{m:?}"));
    }

    #[test]
    fn bounded_library_codec_and_unknown_version() {
        let library = TextLibrary::new(vec![item()]).unwrap();
        let decoded = TextLibrary::decode(&library.encode().unwrap()).unwrap();
        assert_eq!(decoded.for_account(7).count(), 1);
        assert_eq!(decoded.for_account(9).count(), 0);
        assert!(matches!(
            TextLibrary::decode(br#"{"version":99,"items":[]}"#),
            Err(ProductivityError::UnknownVersion(99))
        ));
        assert!(TextLibrary::decode(&vec![b' '; MAX_FILE + 1]).is_err());
        assert!(TextLibrary::new(vec![item(), item()]).is_err());
    }

    #[test]
    fn account_scoped_library_edits_retain_other_accounts() {
        let mut library = TextLibrary::new(vec![item()]).unwrap();
        let mut other = item();
        other.account_id = 9;
        library.upsert(9, other.clone()).unwrap();
        assert_eq!(
            library.upsert(7, other),
            Err(ProductivityError::WrongAccount)
        );
        assert!(library.remove(7, "reply"));
        assert_eq!(library.for_account(9).count(), 1);
        assert_eq!(library.for_account(7).count(), 0);
    }

    #[cfg(unix)]
    #[test]
    fn private_store_uses_public_seam_and_preserves_unknown_version() {
        use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
        let directory = std::env::temp_dir().join(format!(
            "megamail-productivity-test-{}-{}",
            std::process::id(),
            TEMP_ID.fetch_add(1, Ordering::Relaxed)
        ));
        struct Cleanup(PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&directory)
            .unwrap();
        let _cleanup = Cleanup(directory.clone());
        let store = PrivateLibraryStore::new(&directory).unwrap();
        assert!(store.load().unwrap().is_none());
        store
            .save(&TextLibrary::new(vec![item()]).unwrap())
            .unwrap();
        assert_eq!(store.load().unwrap().unwrap().for_account(7).count(), 1);
        let original = store.load().unwrap().unwrap();
        let mut changed = item();
        changed.body = "Updated fictional text".into();
        let updated = store
            .apply(
                &original.revision().unwrap(),
                7,
                Some(changed.clone()),
                None,
            )
            .unwrap();
        assert_eq!(
            store
                .apply(&original.revision().unwrap(), 7, Some(item()), None)
                .err(),
            Some(ProductivityError::Conflict)
        );
        let tb = PrivateLibraryStore::for_backend(&directory, true).unwrap();
        assert!(tb.load().unwrap().is_none());
        tb.apply(
            &TextLibrary::default().revision().unwrap(),
            7,
            Some(item()),
            None,
        )
        .unwrap();
        assert_eq!(
            store
                .load()
                .unwrap()
                .unwrap()
                .for_account(7)
                .next()
                .unwrap()
                .body,
            changed.body
        );
        assert_eq!(
            tb.load()
                .unwrap()
                .unwrap()
                .for_account(7)
                .next()
                .unwrap()
                .body,
            item().body
        );
        {
            let _held = store.lock().unwrap();
            assert!(
                store
                    .apply(&updated.revision().unwrap(), 7, None, Some(&changed.id))
                    .is_err()
            );
        }
        assert_eq!(
            store
                .apply(&updated.revision().unwrap(), 7, None, Some(&changed.id))
                .unwrap()
                .for_account(7)
                .count(),
            0
        );
        let path = directory.join("compose-productivity.v1.json");
        assert_eq!(fs::metadata(&path).unwrap().permissions().mode() & 0o077, 0);
        fs::write(&path, br#"{"version":99}"#).unwrap();
        assert_eq!(
            store.save(&TextLibrary::default()),
            Err(ProductivityError::UnknownVersion(99))
        );
        assert_eq!(fs::read(&path).unwrap().as_slice(), br#"{"version":99}"#);
    }

    #[test]
    fn deferred_policy_fails_closed_without_durable_background_evidence() {
        let mut evidence = DeferredEvidence {
            account_id: 7,
            ..Default::default()
        };
        let intent = DeferredIntent::UndoSend { seconds: 10 };
        assert_eq!(
            validate_deferred(7, 100, intent, evidence),
            Err(DeferredBlocker::NotDurable)
        );
        evidence.durable_payload = true;
        evidence.full_payload_recoverable = true;
        assert_eq!(
            validate_deferred(7, 100, intent, evidence),
            Err(DeferredBlocker::NoClosedAppPolicy)
        );
        evidence.dispatch_while_app_closed = true;
        evidence.durable_wakeup = true;
        evidence.cancel_before_submission = true;
        assert_eq!(validate_deferred(7, 100, intent, evidence), Ok(110));
        evidence.submission_started_or_uncertain = true;
        assert_eq!(
            validate_deferred(7, 100, intent, evidence),
            Err(DeferredBlocker::AlreadySubmitted)
        );
    }
}

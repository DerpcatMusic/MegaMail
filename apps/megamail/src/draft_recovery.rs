//! Private local recovery, independent of provider drafts and durable Outbox.
//!
//! All filesystem operations are synchronous **background-lane** operations.
//! The caller supplies an isolated directory and holds the app's single-instance
//! lock. One writer owns the store. Never infer retention consent from `encrypt`:
//! that flag describes outgoing transport, not the origin of decrypted content.
//! See docs/research/batch2-draft-recovery.md for the UI/worker handoff.

use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Component, Path, PathBuf},
    sync::{Arc, mpsc},
    thread,
    time::{Duration, Instant},
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use megamail_core::{
    models::DraftOrigin,
    worker::{CalendarPart, OutgoingMessage, SendOutcome},
};
use serde::{Deserialize, Serialize};

const MAGIC: &[u8] = b"MEGAMAIL-DRAFT 1\n";
const RECORD_LIMIT: usize = 80 * 1024 * 1024;
const TEXT_LIMIT: usize = 4 * 1024 * 1024;
const FILE_LIMIT: usize = 25 * 1024 * 1024;
const TOTAL_LIMIT: usize = 50 * 1024 * 1024;
const COUNT_LIMIT: usize = 20;

/// Failures contain no message body, recipient, attachment name or profile path.
#[derive(Debug)]
pub enum RecoveryError {
    Io(io::Error),
    UnsafePath,
    InvalidRecord,
    UnsupportedVersion,
    TooLarge,
    StaleGeneration,
    SensitiveConsentRequired,
    UnsupportedPlatform,
}
impl From<io::Error> for RecoveryError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}
impl std::fmt::Display for RecoveryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Io(_) => "Local draft storage failed; keep this composition open.",
            Self::UnsafePath => {
                "Local draft storage or an attachment has an unsafe path or permissions."
            }
            Self::InvalidRecord => "Local draft recovery data is invalid; it was not replaced.",
            Self::UnsupportedVersion => {
                "Local draft recovery uses an unsupported version; it was not replaced."
            }
            Self::TooLarge => "This draft exceeds the local recovery size limit.",
            Self::StaleGeneration => "An older draft update was rejected.",
            Self::SensitiveConsentRequired => {
                "Recovery of decrypted mail requires explicit plaintext-retention consent."
            }
            Self::UnsupportedPlatform => "Private local recovery is supported only on Linux.",
        })
    }
}
impl std::error::Error for RecoveryError {}

/// Stable opaque composer identity. A new composition gets a new key even in
/// the same account. Never derive the token from a subject/address/provider UID.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DraftKey {
    account_id: u32,
    token: String,
}
impl DraftKey {
    pub fn new(account_id: u32) -> Result<Self, RecoveryError> {
        let token = megamail_core::rng::token(24).map_err(|_| RecoveryError::InvalidRecord)?;
        Self::from_parts(account_id, token)
    }
    pub fn from_parts(account_id: u32, token: String) -> Result<Self, RecoveryError> {
        if !valid_token(&token) {
            return Err(RecoveryError::UnsafePath);
        }
        Ok(Self { account_id, token })
    }
    pub fn account_id(&self) -> u32 {
        self.account_id
    }
    fn filename(&self) -> String {
        format!("{}-{}.draft", self.account_id, self.token)
    }
}
fn valid_token(token: &str) -> bool {
    token.len() == 24
        && token
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-._~".contains(&b))
}

/// Set by the source-message policy, not by body inspection or send flags.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Retention {
    Ordinary,
    /// No disk writes and no attachment reads. Not crash recoverable.
    DecryptedSessionOnly,
    /// User explicitly agreed to private *plaintext* retention for this draft.
    DecryptedPlaintextOptIn,
}

/// Immutable UI handoff. Move/clone the outgoing fields on the UI lane, then
/// submit. Keep source attachments alive until the background result arrives.
pub struct DraftUpdate {
    pub key: DraftKey,
    pub generation: u64,
    pub message: OutgoingMessage,
    /// Address selected at capture; catches changed account/alias configuration.
    pub source_address: String,
    pub retention: Retention,
    /// Retain restored owned paths through background capture; ordinary picker
    /// paths still use the no-symlink source reader.
    pub attachment_lease: Option<AttachmentLease>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredOrigin {
    account_id: u32,
    folder_id: u32,
    path: String,
    uid: u32,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredAttachment {
    name: String,
    base64: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredCalendar {
    ics: String,
    method: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    key: DraftKey,
    generation: u64,
    sensitive: bool,
    source_address: String,
    from_alias: Option<String>,
    to: String,
    cc: String,
    bcc: String,
    reply_to: String,
    subject: String,
    body: String,
    html: String,
    in_reply_to: String,
    references: String,
    draft_origin: Option<StoredOrigin>,
    outbox_origin: Option<u32>,
    sign: bool,
    encrypt: bool,
    send_at: Option<i64>,
    calendar: Option<StoredCalendar>,
    attachments: Vec<StoredAttachment>,
}
impl Record {
    fn capture(update: &DraftUpdate) -> Result<Self, RecoveryError> {
        if update.key.account_id != update.message.from_account_id || update.generation == 0 {
            return Err(RecoveryError::InvalidRecord);
        }
        let m = &update.message;
        validate_text(m)?;
        if m.attachments.len() > COUNT_LIMIT {
            return Err(RecoveryError::TooLarge);
        }
        let mut attachments = Vec::new();
        let mut total = 0usize;
        for path in &m.attachments {
            // Open every component without following symlinks. Regular user-picked
            // files need not be 0600; only copies inside our store must be private.
            let path_ref = Path::new(path);
            let file = match &update.attachment_lease {
                Some(lease) if path_ref.starts_with(fd_path(&lease.0.directory)) => {
                    lease.open(path_ref)?
                }
                _ => open_source(path_ref)?,
            };
            let bytes = bounded_read(file, FILE_LIMIT)?;
            total = total
                .checked_add(bytes.len())
                .ok_or(RecoveryError::TooLarge)?;
            if total > TOTAL_LIMIT {
                return Err(RecoveryError::TooLarge);
            }
            let name = Path::new(path)
                .file_name()
                .and_then(|n| n.to_str())
                .ok_or(RecoveryError::UnsafePath)?
                .to_owned();
            validate_name(&name)?;
            attachments.push(StoredAttachment {
                name,
                base64: STANDARD.encode(bytes),
            });
        }
        let record = Self {
            key: update.key.clone(),
            generation: update.generation,
            sensitive: update.retention == Retention::DecryptedPlaintextOptIn,
            source_address: update.source_address.clone(),
            from_alias: m.from_alias.clone(),
            to: m.to.clone(),
            cc: m.cc.clone(),
            bcc: m.bcc.clone(),
            reply_to: m.reply_to.clone(),
            subject: m.subject.clone(),
            body: m.body.clone(),
            html: m.html.clone(),
            in_reply_to: m.in_reply_to.clone(),
            references: m.references.clone(),
            draft_origin: m.draft_origin.as_ref().map(|o| StoredOrigin {
                account_id: o.account_id,
                folder_id: o.folder_id,
                path: o.path.clone(),
                uid: o.uid,
            }),
            outbox_origin: m.outbox_origin,
            sign: m.sign,
            encrypt: m.encrypt,
            send_at: m.send_at,
            calendar: m.calendar.as_ref().map(|c| StoredCalendar {
                ics: c.ics.clone(),
                method: c.method.clone(),
            }),
            attachments,
        };
        record.validate(&update.key)?;
        Ok(record)
    }
    fn message(&self) -> OutgoingMessage {
        OutgoingMessage {
            from_account_id: self.key.account_id,
            from_alias: self.from_alias.clone(),
            to: self.to.clone(),
            cc: self.cc.clone(),
            bcc: self.bcc.clone(),
            reply_to: self.reply_to.clone(),
            subject: self.subject.clone(),
            body: self.body.clone(),
            html: self.html.clone(),
            attachments: Vec::new(),
            in_reply_to: self.in_reply_to.clone(),
            references: self.references.clone(),
            draft_origin: self.draft_origin.as_ref().map(|o| DraftOrigin {
                account_id: o.account_id,
                folder_id: o.folder_id,
                path: o.path.clone(),
                uid: o.uid,
            }),
            outbox_origin: self.outbox_origin,
            sign: self.sign,
            encrypt: self.encrypt,
            send_at: self.send_at,
            calendar: self.calendar.as_ref().map(|c| CalendarPart {
                ics: c.ics.clone(),
                method: c.method.clone(),
            }),
        }
    }
    fn validate(&self, key: &DraftKey) -> Result<(), RecoveryError> {
        if &self.key != key
            || !valid_token(&self.key.token)
            || self.generation == 0
            || self
                .draft_origin
                .as_ref()
                .is_some_and(|o| o.account_id != key.account_id)
        {
            return Err(RecoveryError::InvalidRecord);
        }
        if self.source_address.len() > 1024
            || !self.source_address.contains('@')
            || self.source_address.chars().any(char::is_control)
        {
            return Err(RecoveryError::InvalidRecord);
        }
        validate_text(&self.message())?;
        if self.attachments.len() > COUNT_LIMIT {
            return Err(RecoveryError::TooLarge);
        }
        let mut total = 0;
        for a in &self.attachments {
            validate_name(&a.name)?;
            if a.base64.len() > FILE_LIMIT.div_ceil(3) * 4 {
                return Err(RecoveryError::TooLarge);
            }
            let bytes = STANDARD
                .decode(&a.base64)
                .map_err(|_| RecoveryError::InvalidRecord)?;
            if bytes.len() > FILE_LIMIT {
                return Err(RecoveryError::TooLarge);
            }
            total += bytes.len();
            if total > TOTAL_LIMIT {
                return Err(RecoveryError::TooLarge);
            }
        }
        Ok(())
    }
}
fn validate_text(m: &OutgoingMessage) -> Result<(), RecoveryError> {
    let fields = [
        &m.to,
        &m.cc,
        &m.bcc,
        &m.reply_to,
        &m.subject,
        &m.body,
        &m.html,
        &m.in_reply_to,
        &m.references,
    ];
    let mut total: usize = fields.iter().map(|s| s.len()).sum();
    total += m.from_alias.as_ref().map_or(0, String::len);
    total += m.draft_origin.as_ref().map_or(0, |o| o.path.len());
    total += m
        .calendar
        .as_ref()
        .map_or(0, |c| c.ics.len() + c.method.len());
    if total > TEXT_LIMIT {
        return Err(RecoveryError::TooLarge);
    }
    // Incomplete recipients are valid drafts; wire validation still belongs to
    // send preflight. Never silently normalize Bcc or threading during restore.
    Ok(())
}
fn validate_name(name: &str) -> Result<(), RecoveryError> {
    if name.is_empty()
        || name.len() > 720
        || name == "."
        || name == ".."
        || name
            .chars()
            .any(|c| c.is_control() || matches!(c, '/' | '\\'))
    {
        return Err(RecoveryError::UnsafePath);
    }
    Ok(())
}
fn bounded_read(file: File, limit: usize) -> Result<Vec<u8>, RecoveryError> {
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        return Err(RecoveryError::UnsafePath);
    }
    if metadata.len() > limit as u64 {
        return Err(RecoveryError::TooLarge);
    }
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err(RecoveryError::TooLarge);
    }
    Ok(bytes)
}

/// Owned private copies used by the restored composer/worker. Clones share the
/// lifetime. Drop after the send worker has finished reading, not on enqueue.
#[derive(Clone)]
pub struct AttachmentLease(Arc<StagedAttachments>);
impl AttachmentLease {
    fn open(&self, path: &Path) -> Result<File, RecoveryError> {
        let relative = path
            .strip_prefix(fd_path(&self.0.directory))
            .map_err(|_| RecoveryError::UnsafePath)?;
        let mut parts = relative.components();
        let (Some(Component::Normal(slot)), Some(Component::Normal(name)), None) =
            (parts.next(), parts.next(), parts.next())
        else {
            return Err(RecoveryError::UnsafePath);
        };
        if !self
            .0
            .names
            .iter()
            .any(|(s, n)| slot == std::ffi::OsStr::new(s) && name == std::ffi::OsStr::new(n))
        {
            return Err(RecoveryError::UnsafePath);
        }
        let directory = open_directory(&fd_path(&self.0.directory).join(slot))?;
        open_private_file(&fd_path(&directory).join(name), &self.0.directory)
    }
}
struct StagedAttachments {
    directory: File,
    parent: File,
    name: String,
    names: Vec<(String, String)>,
}
impl Drop for StagedAttachments {
    fn drop(&mut self) {
        let directory = fd_path(&self.directory);
        for (slot, name) in &self.names {
            // Slots are created by this lease only. Do not recursively remove
            // directories or follow a substituted link during cleanup.
            if let Ok(child) = open_directory(&directory.join(slot)) {
                let _ = fs::remove_file(fd_path(&child).join(name));
                let _ = fs::remove_dir(directory.join(slot));
            }
        }
        let _ = fs::remove_dir(fd_path(&self.parent).join(&self.name));
    }
}
/// Generation and provenance remain separate from the mutable outgoing model.
pub struct RecoveredCompose {
    pub key: DraftKey,
    pub generation: u64,
    pub sensitive: bool,
    pub source_address: String,
    pub message: OutgoingMessage,
    pub attachment_lease: AttachmentLease,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveOutcome {
    Durable,
    SessionOnly,
}

/// Small production store seam. Supply an absolute private MegaMail directory;
/// no environment, keyring or legacy profile lookup is performed here.
pub struct RecoveryStore {
    root: File,
    commit_guard: Box<dyn Fn() -> io::Result<()> + Send>,
    uncertain: std::collections::HashMap<DraftKey, UncertainCommit>,
    #[cfg(test)]
    directory_sync_guard: Box<dyn Fn() -> io::Result<()> + Send>,
}

/// Pin the installed inode, not composition/body bytes. A reopened store or a
/// replacement by another writer cannot inherit this store's resync permission.
struct UncertainCommit {
    generation: u64,
    file: File,
    identity: InstalledIdentity,
}
#[derive(PartialEq, Eq)]
struct InstalledIdentity {
    device: u64,
    inode: u64,
    length: u64,
    modified: (i64, i64),
    changed: (i64, i64),
}
fn installed_identity(file: &File) -> Result<InstalledIdentity, RecoveryError> {
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::MetadataExt;
        let metadata = file.metadata()?;
        Ok(InstalledIdentity {
            device: metadata.dev(),
            inode: metadata.ino(),
            length: metadata.len(),
            modified: (metadata.mtime(), metadata.mtime_nsec()),
            changed: (metadata.ctime(), metadata.ctime_nsec()),
        })
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = file;
        Err(RecoveryError::UnsupportedPlatform)
    }
}
impl RecoveryStore {
    pub fn open(root: &Path) -> Result<Self, RecoveryError> {
        Self::open_with_commit_guard(root, || Ok(()))
    }
    /// Fault injection after file sync and before replacement. Tests still use
    /// the production save/validation/write path, not a fake store.
    pub fn open_with_commit_guard(
        root: &Path,
        guard: impl Fn() -> io::Result<()> + Send + 'static,
    ) -> Result<Self, RecoveryError> {
        Ok(Self {
            root: open_private_root(root)?,
            commit_guard: Box::new(guard),
            uncertain: std::collections::HashMap::new(),
            #[cfg(test)]
            directory_sync_guard: Box::new(|| Ok(())),
        })
    }
    /// Per-store deterministic fault at the real post-rename directory-sync
    /// seam. Successful calls still execute the production directory sync.
    #[cfg(test)]
    fn open_with_directory_sync_guard(
        root: &Path,
        guard: impl Fn() -> io::Result<()> + Send + 'static,
    ) -> Result<Self, RecoveryError> {
        let mut store = Self::open(root)?;
        store.directory_sync_guard = Box::new(guard);
        Ok(store)
    }
    fn sync_directory(&self) -> Result<(), RecoveryError> {
        #[cfg(test)]
        (self.directory_sync_guard)()?;
        self.root.sync_all()?;
        Ok(())
    }
    /// Lists only this account's opaque keys, bounded to 256 directory entries.
    /// `complete=false` requires an explicit continuation/repair UI, not "no drafts".
    pub fn list(&self, account_id: u32) -> Result<(Vec<DraftKey>, bool), RecoveryError> {
        let mut entries = fs::read_dir(fd_path(&self.root))?;
        let mut keys = Vec::new();
        for _ in 0..256 {
            let Some(entry) = entries.next() else {
                return Ok((keys, true));
            };
            let entry = entry?;
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            let Some((account, token)) =
                name.strip_suffix(".draft").and_then(|s| s.split_once('-'))
            else {
                continue;
            };
            if account.parse::<u32>().ok() != Some(account_id) {
                continue;
            }
            keys.push(DraftKey::from_parts(account_id, token.to_owned())?);
        }
        Ok((keys, entries.next().is_none()))
    }
    fn read_record(&self, key: &DraftKey) -> Result<Option<Record>, RecoveryError> {
        Ok(self.read_installed(key)?.map(|(record, _, _)| record))
    }
    fn read_installed(
        &self,
        key: &DraftKey,
    ) -> Result<Option<(Record, File, Vec<u8>)>, RecoveryError> {
        if !valid_token(&key.token) {
            return Err(RecoveryError::UnsafePath);
        }
        let path = fd_path(&self.root).join(key.filename());
        let file = match open_private_file(&path, &self.root) {
            Ok(file) => file,
            Err(RecoveryError::Io(e)) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        let bytes = bounded_read(file.try_clone()?, RECORD_LIMIT)?;
        if !bytes.starts_with(MAGIC) {
            return Err(RecoveryError::UnsupportedVersion);
        }
        let record: Record = serde_json::from_slice(&bytes[MAGIC.len()..])
            .map_err(|_| RecoveryError::InvalidRecord)?;
        record.validate(key)?;
        Ok(Some((record, file, bytes)))
    }
    /// No overwrite if old data is unsupported/corrupt/unsafe, the generation is
    /// stale, attachment capture fails, or the write/sync before rename fails.
    /// An equal generation can only resync this store's uncertain installation,
    /// after exact snapshot/attachment-byte readback; it never replaces a file.
    pub fn save(&mut self, update: &DraftUpdate) -> Result<SaveOutcome, RecoveryError> {
        if update.retention == Retention::DecryptedSessionOnly {
            return Ok(SaveOutcome::SessionOnly);
        }
        let old = self.read_installed(&update.key)?;
        let resync = if let Some((old, file, _)) = &old {
            if update.generation < old.generation {
                return Err(RecoveryError::StaleGeneration);
            }
            if update.generation == old.generation {
                if !self.owns_uncertain_installation(&update.key, update.generation, file)? {
                    return Err(RecoveryError::StaleGeneration);
                }
                true
            } else {
                false
            }
        } else {
            false
        };
        let record = Record::capture(update)?;
        let data = serde_json::to_vec(&record).map_err(|_| RecoveryError::InvalidRecord)?;
        if data.len() + MAGIC.len() > RECORD_LIMIT {
            return Err(RecoveryError::TooLarge);
        }
        if resync {
            let Some((_, _, bytes)) = old else {
                return Err(RecoveryError::StaleGeneration);
            };
            if &bytes[MAGIC.len()..] != data.as_slice() {
                return Err(RecoveryError::StaleGeneration);
            }
            self.sync_directory()?;
            // Reopen the name after sync: a replaced inode (even with identical
            // bytes and generation) must not receive this writer's durable ack.
            let Some((_, file, bytes)) = self.read_installed(&update.key)? else {
                return Err(RecoveryError::StaleGeneration);
            };
            if !self.owns_uncertain_installation(&update.key, update.generation, &file)?
                || &bytes[MAGIC.len()..] != data.as_slice()
            {
                return Err(RecoveryError::StaleGeneration);
            }
            self.uncertain.remove(&update.key);
        } else {
            self.atomic_write(&update.key, update.generation, &data)?;
        }
        Ok(SaveOutcome::Durable)
    }
    fn owns_uncertain_installation(
        &self,
        key: &DraftKey,
        generation: u64,
        file: &File,
    ) -> Result<bool, RecoveryError> {
        let Some(commit) = self.uncertain.get(key) else {
            return Ok(false);
        };
        Ok(commit.generation == generation
            && commit.identity == installed_identity(file)?
            && commit.identity == installed_identity(&commit.file)?)
    }
    fn atomic_write(
        &mut self,
        key: &DraftKey,
        generation: u64,
        data: &[u8],
    ) -> Result<(), RecoveryError> {
        // Match the writer's key bound and cap pinned descriptors even when the
        // store is used directly and multiple drafts hit a directory-sync fault.
        if !self.uncertain.contains_key(key) && self.uncertain.len() >= 256 {
            return Err(RecoveryError::TooLarge);
        }
        let token = megamail_core::rng::token(24).map_err(|_| RecoveryError::InvalidRecord)?;
        let tmp = fd_path(&self.root).join(format!("{token}.pending"));
        // Never unlink an existing path if create_new failed (including a
        // substituted symlink or a random-token collision).
        let mut file = create_private_file(&tmp)?;
        let result = (|| {
            file.write_all(MAGIC)?;
            file.write_all(data)?;
            file.sync_all()?;
            (self.commit_guard)()?;
            fs::rename(&tmp, fd_path(&self.root).join(key.filename()))?;
            Ok::<(), RecoveryError>(())
        })();
        if let Err(error) = result {
            let _ = fs::remove_file(tmp);
            return Err(error);
        }
        // Rename has actually installed this file. Keep narrow resync authority
        // before the fallible directory sync, and release it only on durability.
        let identity = installed_identity(&file)?;
        self.uncertain.insert(
            key.clone(),
            UncertainCommit {
                generation,
                file,
                identity,
            },
        );
        self.sync_directory()?;
        self.uncertain.remove(key);
        Ok(())
    }
    /// Restores only an explicitly selected key; creates independently owned
    /// private attachment copies. Refuses sensitive records without renewed consent.
    pub fn restore(
        &self,
        key: &DraftKey,
        allow_sensitive: bool,
    ) -> Result<Option<RecoveredCompose>, RecoveryError> {
        let Some(record) = self.read_record(key)? else {
            return Ok(None);
        };
        if record.sensitive && !allow_sensitive {
            return Err(RecoveryError::SensitiveConsentRequired);
        }
        let parent = fd_path(&self.root);
        let token = megamail_core::rng::token(24).map_err(|_| RecoveryError::InvalidRecord)?;
        let name = format!("{token}.stage");
        create_private_dir(&parent.join(&name))?;
        let directory = match open_directory(&parent.join(&name)) {
            Ok(file) => file,
            Err(error) => {
                let _ = fs::remove_dir(parent.join(&name));
                return Err(error);
            }
        };
        let mut staged = StagedAttachments {
            directory,
            parent: self.root.try_clone()?,
            name,
            names: Vec::new(),
        };
        let mut message = record.message();
        for (index, attachment) in record.attachments.iter().enumerate() {
            // An indexed subdirectory preserves the exact basename even when
            // two attachments have the same name, hence MIME filename semantics.
            let slot = index.to_string();
            let slot_path = fd_path(&staged.directory).join(&slot);
            create_private_dir(&slot_path)?;
            staged.names.push((slot, attachment.name.clone()));
            let path = slot_path.join(&attachment.name);
            let bytes = STANDARD
                .decode(&attachment.base64)
                .map_err(|_| RecoveryError::InvalidRecord)?;
            let mut file = create_private_file(&path)?;
            file.write_all(&bytes)?;
            message
                .attachments
                .push(path.to_string_lossy().into_owned());
        }
        Ok(Some(RecoveredCompose {
            key: record.key,
            generation: record.generation,
            sensitive: record.sensitive,
            source_address: record.source_address,
            message,
            attachment_lease: AttachmentLease(Arc::new(staged)),
        }))
    }
    /// Remove only the exact generation the user discarded or definitively sent.
    /// Do not call this for queued/uncertain/send-failed/server-draft-save outcomes.
    pub fn discard(&mut self, key: &DraftKey, generation: u64) -> Result<bool, RecoveryError> {
        let Some(record) = self.read_record(key)? else {
            return Ok(false);
        };
        if record.generation != generation {
            return Err(RecoveryError::StaleGeneration);
        }
        fs::remove_file(fd_path(&self.root).join(key.filename()))?;
        self.uncertain.remove(key);
        self.root.sync_all()?;
        Ok(true)
    }
    pub fn clear_after_send(
        &mut self,
        key: &DraftKey,
        generation: u64,
        expected_request: u64,
        account_id: u32,
        request_id: u64,
        outcome: SendOutcome,
        uncertain: bool,
    ) -> Result<bool, RecoveryError> {
        if account_id != key.account_id
            || request_id != expected_request
            || uncertain
            || outcome != SendOutcome::Sent
        {
            return Ok(false);
        }
        self.discard(key, generation)
    }
    /// Bounded cleanup under the app single-instance lock. Never removes final
    /// records or follows links. Use `prune_stale_work` for crashed owned staging.
    pub fn prune_pending(&self, limit: usize) -> Result<usize, RecoveryError> {
        let mut removed = 0;
        for entry in fs::read_dir(fd_path(&self.root))?.take(limit.min(256)) {
            let entry = entry?;
            let name = entry.file_name();
            let Some(token) = name.to_str().and_then(|n| n.strip_suffix(".pending")) else {
                continue;
            };
            if !valid_token(token) {
                continue;
            }
            if open_private_file(&entry.path(), &self.root).is_err() {
                continue;
            }
            fs::remove_file(entry.path())?;
            removed += 1;
        }
        if removed > 0 {
            self.root.sync_all()?;
        }
        Ok(removed)
    }
    /// Startup-only cleanup of generated attachment leases from a dead process.
    /// At most 256 root entries, 20 numbered slots and one private file per slot.
    /// Unexpected content is left for explicit repair, never recursively removed.
    pub fn prune_stale_work(&self, limit: usize) -> Result<usize, RecoveryError> {
        let mut removed = self.prune_pending(limit)?;
        for entry in fs::read_dir(fd_path(&self.root))?.take(limit.min(256)) {
            let entry = entry?;
            let name = entry.file_name();
            let Some(token) = name.to_str().and_then(|n| n.strip_suffix(".stage")) else {
                continue;
            };
            if !valid_token(token) {
                continue;
            }
            let Ok(directory) = open_directory(&entry.path()) else {
                continue;
            };
            if !is_private_directory(&directory, &self.root)? {
                continue;
            }
            for slot in fs::read_dir(fd_path(&directory))?.take(COUNT_LIMIT) {
                let slot = slot?;
                let name = slot.file_name();
                let Some(number) = name.to_str().and_then(|s| s.parse::<usize>().ok()) else {
                    continue;
                };
                if number >= COUNT_LIMIT || name != std::ffi::OsStr::new(&number.to_string()) {
                    continue;
                }
                let Ok(child) = open_directory(&slot.path()) else {
                    continue;
                };
                if !is_private_directory(&child, &self.root)? {
                    continue;
                }
                let mut files = fs::read_dir(fd_path(&child))?;
                if let Some(file) = files.next() {
                    let file = file?;
                    if files.next().is_some()
                        || open_private_file(&file.path(), &self.root).is_err()
                    {
                        continue;
                    }
                    fs::remove_file(file.path())?;
                }
                let _ = fs::remove_dir(slot.path());
            }
            if fs::remove_dir(entry.path()).is_ok() {
                removed += 1;
            }
        }
        if removed > 0 {
            self.root.sync_all()?;
        }
        Ok(removed)
    }
}

fn is_private_directory(directory: &File, root: &File) -> Result<bool, RecoveryError> {
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::MetadataExt;
        let metadata = directory.metadata()?;
        return Ok(metadata.is_dir()
            && metadata.mode() & 0o077 == 0
            && metadata.uid() == root.metadata()?.uid());
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (directory, root);
        Err(RecoveryError::UnsupportedPlatform)
    }
}

// Linux UAPI flags: use std's safe fd ownership, no native/helper compilation
// and no unsafe code/dependency addition. Non-Linux fails closed below.
#[cfg(target_os = "linux")]
const NOFOLLOW: i32 = 0o400000;
#[cfg(target_os = "linux")]
const DIRECTORY: i32 = 0o200000;
#[cfg(target_os = "linux")]
const NONBLOCK: i32 = 0o4000;
#[cfg(target_os = "linux")]
fn fd_path(file: &File) -> PathBuf {
    use std::os::fd::AsRawFd;
    PathBuf::from(format!("/proc/self/fd/{}", file.as_raw_fd()))
}
#[cfg(not(target_os = "linux"))]
fn fd_path(_: &File) -> PathBuf {
    PathBuf::new()
}
#[cfg(target_os = "linux")]
fn open_directory(path: &Path) -> Result<File, RecoveryError> {
    use std::os::unix::fs::OpenOptionsExt;
    Ok(OpenOptions::new()
        .read(true)
        .custom_flags(NOFOLLOW | DIRECTORY)
        .open(path)?)
}
#[cfg(not(target_os = "linux"))]
fn open_directory(_: &Path) -> Result<File, RecoveryError> {
    Err(RecoveryError::UnsupportedPlatform)
}
fn open_parent(path: &Path) -> Result<(File, std::ffi::OsString), RecoveryError> {
    if !path.is_absolute() {
        return Err(RecoveryError::UnsafePath);
    }
    let mut parts = path.components();
    if parts.next() != Some(Component::RootDir) {
        return Err(RecoveryError::UnsafePath);
    }
    let mut names = Vec::new();
    for part in parts {
        let Component::Normal(name) = part else {
            return Err(RecoveryError::UnsafePath);
        };
        names.push(name.to_owned());
    }
    let name = names.pop().ok_or(RecoveryError::UnsafePath)?;
    let mut directory = open_directory(Path::new("/"))?;
    for part in names {
        directory = open_directory(&fd_path(&directory).join(part))?;
    }
    Ok((directory, name))
}
fn open_private_root(root: &Path) -> Result<File, RecoveryError> {
    let (parent, name) = open_parent(root)?;
    let path = fd_path(&parent).join(name);
    let directory = match open_directory(&path) {
        Ok(file) => file,
        Err(RecoveryError::Io(e)) if e.kind() == io::ErrorKind::NotFound => {
            create_private_dir(&path)?;
            parent.sync_all()?;
            open_directory(&path)?
        }
        Err(error) => return Err(error),
    };
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::MetadataExt;
        let metadata = directory.metadata()?;
        if metadata.mode() & 0o077 != 0 || metadata.uid() != fs::metadata("/proc/self")?.uid() {
            return Err(RecoveryError::UnsafePath);
        }
    }
    Ok(directory)
}
#[cfg(target_os = "linux")]
fn create_private_dir(path: &Path) -> Result<(), RecoveryError> {
    use std::os::unix::fs::DirBuilderExt;
    Ok(fs::DirBuilder::new().mode(0o700).create(path)?)
}
#[cfg(not(target_os = "linux"))]
fn create_private_dir(_: &Path) -> Result<(), RecoveryError> {
    Err(RecoveryError::UnsupportedPlatform)
}
#[cfg(target_os = "linux")]
fn create_private_file(path: &Path) -> Result<File, RecoveryError> {
    use std::os::unix::fs::OpenOptionsExt;
    Ok(OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(NOFOLLOW)
        .open(path)?)
}
#[cfg(not(target_os = "linux"))]
fn create_private_file(_: &Path) -> Result<File, RecoveryError> {
    Err(RecoveryError::UnsupportedPlatform)
}
#[cfg(target_os = "linux")]
fn open_private_file(path: &Path, root: &File) -> Result<File, RecoveryError> {
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(NOFOLLOW | NONBLOCK)
        .open(path)?;
    let m = file.metadata()?;
    if !m.is_file() || m.mode() & 0o077 != 0 || m.nlink() != 1 || m.uid() != root.metadata()?.uid()
    {
        return Err(RecoveryError::UnsafePath);
    }
    Ok(file)
}
#[cfg(not(target_os = "linux"))]
fn open_private_file(_: &Path, _: &File) -> Result<File, RecoveryError> {
    Err(RecoveryError::UnsupportedPlatform)
}
#[cfg(target_os = "linux")]
fn open_source(path: &Path) -> Result<File, RecoveryError> {
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
    let (parent, name) = open_parent(path)?;
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(NOFOLLOW | NONBLOCK)
        .open(fd_path(&parent).join(name))?;
    if !file.metadata()?.is_file() || file.metadata()?.nlink() != 1 {
        return Err(RecoveryError::UnsafePath);
    }
    Ok(file)
}
#[cfg(not(target_os = "linux"))]
fn open_source(_: &Path) -> Result<File, RecoveryError> {
    Err(RecoveryError::UnsupportedPlatform)
}

/// Debounced off-UI writer for one active composer. Queue admission never means
/// durable save. UI keeps its latest update on Busy/Closed and retries explicitly.
pub struct RecoveryWriter {
    sender: mpsc::SyncSender<WriterCommand>,
}
pub enum WriterCommand {
    Update(DraftUpdate),
    Flush,
    Discard {
        key: DraftKey,
        generation: u64,
    },
    SendFinished {
        key: DraftKey,
        generation: u64,
        expected_request: u64,
        account_id: u32,
        request_id: u64,
        outcome: SendOutcome,
        uncertain: bool,
    },
}
pub enum WriterEvent {
    Saved {
        key: DraftKey,
        generation: u64,
        outcome: SaveOutcome,
    },
    Failed {
        key: DraftKey,
        generation: u64,
        error: RecoveryError,
    },
    Discarded {
        key: DraftKey,
        generation: u64,
        removed: bool,
    },
    /// Coalesced without a disk save. Never present this as saved.
    Superseded { key: DraftKey, generation: u64 },
}
impl RecoveryWriter {
    /// Create/open the store off UI before starting the writer. Drain results
    /// through GPUI's existing background callback lane, never block the UI.
    pub fn start(
        mut store: RecoveryStore,
        debounce: Duration,
    ) -> Result<(Self, mpsc::Receiver<WriterEvent>), RecoveryError> {
        let (sender, receiver) = mpsc::sync_channel::<WriterCommand>(4);
        let (events, results) = mpsc::sync_channel(16);
        thread::Builder::new()
            .name("draft-recovery".into())
            .spawn(move || {
                let mut pending: Option<DraftUpdate> = None;
                let mut deadline = Instant::now();
                let mut max_deadline = deadline;
                let mut retired = std::collections::HashMap::<DraftKey, u64>::new();
                let mut seen = std::collections::HashMap::<DraftKey, u64>::new();
                loop {
                    let next = if pending.is_some() {
                        receiver.recv_timeout(deadline.saturating_duration_since(Instant::now()))
                    } else {
                        receiver
                            .recv()
                            .map_err(|_| mpsc::RecvTimeoutError::Disconnected)
                    };
                    match next {
                        Ok(WriterCommand::Update(update)) => {
                            if retired
                                .get(&update.key)
                                .is_some_and(|g| *g >= update.generation)
                                || seen
                                    .get(&update.key)
                                    .is_some_and(|g| *g > update.generation)
                                || pending.as_ref().is_some_and(|p| {
                                    p.key == update.key && p.generation >= update.generation
                                })
                            {
                                let _ = events.send(WriterEvent::Failed {
                                    key: update.key,
                                    generation: update.generation,
                                    error: RecoveryError::StaleGeneration,
                                });
                                continue;
                            }
                            if !seen.contains_key(&update.key) && seen.len() >= 256 {
                                let _ = events.send(WriterEvent::Failed {
                                    key: update.key,
                                    generation: update.generation,
                                    error: RecoveryError::TooLarge,
                                });
                                continue;
                            }
                            seen.insert(update.key.clone(), update.generation);
                            if pending.as_ref().is_some_and(|p| p.key != update.key) {
                                save_pending(&mut store, &mut pending, &events);
                            }
                            if pending.is_none() {
                                max_deadline = Instant::now() + Duration::from_secs(5);
                            }
                            if let Some(old) = pending.take() {
                                let _ = events.send(WriterEvent::Superseded {
                                    key: old.key,
                                    generation: old.generation,
                                });
                            }
                            pending = Some(update);
                            deadline = (Instant::now() + debounce.min(Duration::from_secs(5)))
                                .min(max_deadline);
                        }
                        Ok(WriterCommand::Flush) | Err(mpsc::RecvTimeoutError::Timeout) => {
                            save_pending(&mut store, &mut pending, &events);
                        }
                        Ok(WriterCommand::Discard { key, generation }) => {
                            if pending
                                .as_ref()
                                .is_some_and(|p| p.key == key && p.generation <= generation)
                            {
                                pending = None;
                            }
                            retired.insert(key.clone(), generation);
                            let result = store.discard(&key, generation);
                            let event = match result {
                                Ok(removed) => WriterEvent::Discarded {
                                    key,
                                    generation,
                                    removed,
                                },
                                Err(error) => WriterEvent::Failed {
                                    key,
                                    generation,
                                    error,
                                },
                            };
                            let _ = events.send(event);
                        }
                        Ok(WriterCommand::SendFinished {
                            key,
                            generation,
                            expected_request,
                            account_id,
                            request_id,
                            outcome,
                            uncertain,
                        }) => {
                            if account_id != key.account_id
                                || request_id != expected_request
                                || uncertain
                                || outcome != SendOutcome::Sent
                                || seen.get(&key).is_some_and(|latest| *latest > generation)
                            {
                                continue;
                            }
                            if pending
                                .as_ref()
                                .is_some_and(|p| p.key == key && p.generation <= generation)
                            {
                                pending = None;
                            }
                            retired.insert(key.clone(), generation);
                            let event = match store.clear_after_send(
                                &key,
                                generation,
                                expected_request,
                                account_id,
                                request_id,
                                outcome,
                                uncertain,
                            ) {
                                Ok(removed) => WriterEvent::Discarded {
                                    key,
                                    generation,
                                    removed,
                                },
                                Err(error) => WriterEvent::Failed {
                                    key,
                                    generation,
                                    error,
                                },
                            };
                            let _ = events.send(event);
                        }
                        Err(mpsc::RecvTimeoutError::Disconnected) => {
                            save_pending(&mut store, &mut pending, &events);
                            break;
                        }
                    }
                }
            })?;
        Ok((Self { sender }, results))
    }
    pub fn submit(&self, command: WriterCommand) -> Result<(), mpsc::TrySendError<WriterCommand>> {
        self.sender.try_send(command)
    }
}
fn save_pending(
    store: &mut RecoveryStore,
    pending: &mut Option<DraftUpdate>,
    events: &mpsc::SyncSender<WriterEvent>,
) {
    let Some(update) = pending.take() else {
        return;
    };
    let event = match store.save(&update) {
        Ok(outcome) => WriterEvent::Saved {
            key: update.key,
            generation: update.generation,
            outcome,
        },
        Err(error) => WriterEvent::Failed {
            key: update.key,
            generation: update.generation,
            error,
        },
    };
    let _ = events.send(event);
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use crate::compose;
    use megamail_core::config::{AccountConfig, AliasConfig};
    use std::os::unix::fs::{PermissionsExt, symlink};

    struct Fixture {
        path: PathBuf,
    }
    impl Fixture {
        fn new() -> Self {
            let path = PathBuf::from("/tmp").join(format!(
                "megamail-f7-{}",
                megamail_core::rng::token(24).unwrap()
            ));
            create_private_dir(&path).unwrap();
            Self { path }
        }
        fn root(&self) -> PathBuf {
            self.path.join("recovery")
        }
        fn file(&self, name: &str, data: &[u8]) -> PathBuf {
            let path = self.path.join(name);
            let mut f = create_private_file(&path).unwrap();
            f.write_all(data).unwrap();
            path
        }
        fn update(&self, generation: u64) -> DraftUpdate {
            let mut message = compose::new_message(7);
            message.to = "Ada <ada@example.test>".into();
            message.cc = "bob@example.test".into();
            message.bcc = "secret@example.test".into();
            message.reply_to = "replies@example.test".into();
            message.subject = "Private fictional draft".into();
            message.body = "plain body\nsecond line".into();
            message.html = "<p>plain body</p>".into();
            message.in_reply_to = "parent@example.test".into();
            message.references = "older@example.test parent@example.test".into();
            message.from_alias = Some("Work <work@example.test>".into());
            message.draft_origin = Some(DraftOrigin {
                account_id: 7,
                folder_id: 3,
                path: "Drafts".into(),
                uid: 17,
            });
            message.outbox_origin = Some(4);
            message.sign = true;
            message.encrypt = true;
            message.send_at = Some(9000);
            message.calendar = Some(CalendarPart {
                ics: "BEGIN:VCALENDAR\r\nEND:VCALENDAR".into(),
                method: "REPLY".into(),
            });
            DraftUpdate {
                key: DraftKey::from_parts(7, "abcdefghijklmnopqrstuvwx".into()).unwrap(),
                generation,
                message,
                source_address: "work@example.test".into(),
                retention: Retention::Ordinary,
                attachment_lease: None,
            }
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
    fn account() -> AccountConfig {
        let mut account = AccountConfig::new("me@example.test");
        account.aliases.push(AliasConfig {
            identity: "Work <work@example.test>".into(),
            ..Default::default()
        });
        account
    }

    #[test]
    fn restart_restores_all_outgoing_fields_and_binary_attachment_after_source_is_deleted() {
        let f = Fixture::new();
        let source = f.file("bytes.bin", b"binary\0\xff\x80\r\n");
        let mut update = f.update(9);
        update
            .message
            .attachments
            .push(source.to_string_lossy().into_owned());
        {
            let mut store = RecoveryStore::open(&f.root()).unwrap();
            assert_eq!(store.save(&update).unwrap(), SaveOutcome::Durable);
        }
        fs::remove_file(source).unwrap();
        let mut store = RecoveryStore::open(&f.root()).unwrap();
        let (keys, complete) = store.list(7).unwrap();
        assert!(complete);
        assert_eq!(keys, vec![update.key.clone()]);
        assert!(store.list(8).unwrap().0.is_empty());
        let restored = compose::prepare_local_recovery(&store, &update.key, &account(), false)
            .unwrap()
            .unwrap();
        let m = &restored.message;
        assert_eq!(m.to, update.message.to);
        assert_eq!(m.cc, update.message.cc);
        assert_eq!(m.bcc, update.message.bcc);
        assert_eq!(m.reply_to, update.message.reply_to);
        assert_eq!(m.from_alias, update.message.from_alias);
        assert_eq!(m.body, update.message.body);
        assert_eq!(m.html, update.message.html);
        assert_eq!(m.in_reply_to, update.message.in_reply_to);
        assert_eq!(m.references, update.message.references);
        assert_eq!(m.draft_origin.as_ref().unwrap().uid, 17);
        assert_eq!(m.outbox_origin, Some(4));
        assert!(m.sign && m.encrypt);
        assert_eq!(m.send_at, Some(9000));
        assert_eq!(m.calendar.as_ref().unwrap().method, "REPLY");
        assert_eq!(
            fs::read(&m.attachments[0]).unwrap(),
            b"binary\0\xff\x80\r\n"
        );
        assert_eq!(
            fs::metadata(&m.attachments[0])
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        // Real compose preparation and real store capture cross the same seam
        // again; restored /proc-fd paths must not be mistaken for picker paths.
        let recapture = compose::prepare_recovery_update(
            m,
            &restored.key,
            10,
            &account(),
            Retention::Ordinary,
            Some(restored.attachment_lease.clone()),
        )
        .unwrap();
        store.save(&recapture).unwrap();
        let paths = m.attachments.clone();
        drop(recapture);
        drop(restored);
        assert!(!Path::new(&paths[0]).exists());
        let again = store.restore(&update.key, false).unwrap().unwrap();
        assert_eq!(
            fs::read(&again.message.attachments[0]).unwrap(),
            b"binary\0\xff\x80\r\n"
        );
    }

    #[test]
    fn mime_preparation_snapshot_and_restart_use_the_real_compose_and_store_seams() {
        let f = Fixture::new();
        let staging = f.path.join("mime");
        let prepared = compose::prepare_draft_from_raw_in(
            include_bytes!("fixtures/batch2_draft_recovery/private-draft.eml"),
            &account(),
            DraftOrigin {
                account_id: 7,
                folder_id: 3,
                path: "Drafts".into(),
                uid: 17,
            },
            &staging,
        )
        .unwrap();
        let key = DraftKey::new(7).unwrap();
        let update = compose::prepare_recovery_update(
            &prepared.message,
            &key,
            1,
            &account(),
            Retention::Ordinary,
            None,
        )
        .unwrap();
        RecoveryStore::open(&f.root())
            .unwrap()
            .save(&update)
            .unwrap();
        compose::remove_staged_attachments(prepared.temporary_attachment_paths);
        let store = RecoveryStore::open(&f.root()).unwrap();
        let recovered = compose::prepare_local_recovery(&store, &key, &account(), false)
            .unwrap()
            .unwrap();
        assert_eq!(recovered.message.bcc, "Hidden <hidden@example.test>");
        assert_eq!(recovered.message.reply_to, "replies@example.test");
        assert_eq!(recovered.message.in_reply_to, "parent@example.test");
        assert_eq!(
            recovered.message.references,
            "older@example.test parent@example.test"
        );
        assert_eq!(
            recovered.message.from_alias.as_deref(),
            Some("Work <work@example.test>")
        );
        assert_eq!(
            fs::read(&recovered.message.attachments[0]).unwrap(),
            b"binary\0\xff\x80"
        );
    }

    #[test]
    fn duplicate_basenames_keep_separate_bytes_and_exact_filenames() {
        let f = Fixture::new();
        let mut update = f.update(1);
        for (slot, data) in [("a", b"first".as_slice()), ("b", b"second".as_slice())] {
            let directory = f.path.join(slot);
            create_private_dir(&directory).unwrap();
            let path = directory.join("same.bin");
            fs::write(&path, data).unwrap();
            update
                .message
                .attachments
                .push(path.to_string_lossy().into_owned());
        }
        let mut store = RecoveryStore::open(&f.root()).unwrap();
        store.save(&update).unwrap();
        let restored = store.restore(&update.key, false).unwrap().unwrap();
        assert_eq!(
            fs::read(&restored.message.attachments[0]).unwrap(),
            b"first"
        );
        assert_eq!(
            fs::read(&restored.message.attachments[1]).unwrap(),
            b"second"
        );
        for path in &restored.message.attachments {
            assert_eq!(Path::new(path).file_name().unwrap(), "same.bin");
        }
    }

    fn directory_sync_fault_store(
        root: &Path,
    ) -> (
        RecoveryStore,
        Arc<std::sync::atomic::AtomicBool>,
        Arc<std::sync::atomic::AtomicUsize>,
    ) {
        use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
        let fault = Arc::new(AtomicBool::new(true));
        let calls = Arc::new(AtomicUsize::new(0));
        let guard_fault = fault.clone();
        let guard_calls = calls.clone();
        let store = RecoveryStore::open_with_directory_sync_guard(root, move || {
            guard_calls.fetch_add(1, Ordering::SeqCst);
            if guard_fault.load(Ordering::SeqCst) {
                Err(io::Error::from_raw_os_error(5))
            } else {
                Ok(())
            }
        })
        .unwrap();
        (store, fault, calls)
    }

    #[test]
    fn writer_resyncs_exact_unchanged_generation_after_actual_rename_sync_failure() {
        use std::sync::atomic::Ordering;
        let f = Fixture::new();
        let attachment_bytes = b"binary\0\xff\x80\r\n";
        let source = f.file("bytes.bin", attachment_bytes);
        let mut update = f.update(9);
        update
            .message
            .attachments
            .push(source.to_string_lossy().into_owned());
        let key = update.key.clone();
        let expected = serde_json::to_vec(&Record::capture(&update).unwrap()).unwrap();
        let make_update = || {
            let mut update = f.update(9);
            update
                .message
                .attachments
                .push(source.to_string_lossy().into_owned());
            update
        };
        let (store, fault, sync_calls) = directory_sync_fault_store(&f.root());
        let (writer, events) = RecoveryWriter::start(store, Duration::from_secs(5)).unwrap();
        assert!(writer.submit(WriterCommand::Update(update)).is_ok());
        assert!(writer.submit(WriterCommand::Flush).is_ok());
        assert!(matches!(
            events.recv_timeout(Duration::from_secs(2)).unwrap(),
            WriterEvent::Failed { key: failed_key, generation: 9, error: RecoveryError::Io(_) }
                if failed_key == key
        ));
        // This is the installed final name, not the precommit fault seam. The
        // fixture observes every serialized field and attachment after rename.
        let path = f.root().join(key.filename());
        let installed = fs::read(&path).unwrap();
        assert_eq!(&installed[MAGIC.len()..], expected.as_slice());
        let identity = installed_identity(&File::open(&path).unwrap()).unwrap();
        assert_eq!(sync_calls.load(Ordering::SeqCst), 1);

        // A repeated failure remains retryable, without overwriting generation 9.
        assert!(writer.submit(WriterCommand::Update(make_update())).is_ok());
        assert!(writer.submit(WriterCommand::Flush).is_ok());
        assert!(matches!(
            events.recv_timeout(Duration::from_secs(2)).unwrap(),
            WriterEvent::Failed {
                generation: 9,
                error: RecoveryError::Io(_),
                ..
            }
        ));
        assert_eq!(sync_calls.load(Ordering::SeqCst), 2);
        fault.store(false, Ordering::SeqCst);

        let mut different = make_update();
        different.message.bcc = "different@example.test".into();
        assert!(writer.submit(WriterCommand::Update(different)).is_ok());
        assert!(writer.submit(WriterCommand::Flush).is_ok());
        assert!(matches!(
            events.recv_timeout(Duration::from_secs(2)).unwrap(),
            WriterEvent::Failed {
                generation: 9,
                error: RecoveryError::StaleGeneration,
                ..
            }
        ));
        // Same path and length do not establish attachment equality.
        fs::write(&source, b"change\0\xfe\x81\r\n").unwrap();
        assert!(writer.submit(WriterCommand::Update(make_update())).is_ok());
        assert!(writer.submit(WriterCommand::Flush).is_ok());
        assert!(matches!(
            events.recv_timeout(Duration::from_secs(2)).unwrap(),
            WriterEvent::Failed {
                generation: 9,
                error: RecoveryError::StaleGeneration,
                ..
            }
        ));
        assert_eq!(sync_calls.load(Ordering::SeqCst), 2);
        fs::write(&source, attachment_bytes).unwrap();

        assert!(writer.submit(WriterCommand::Update(make_update())).is_ok());
        assert!(writer.submit(WriterCommand::Flush).is_ok());
        assert!(matches!(
            events.recv_timeout(Duration::from_secs(2)).unwrap(),
            WriterEvent::Saved { key: saved_key, generation: 9, outcome: SaveOutcome::Durable }
                if saved_key == key
        ));
        assert_eq!(sync_calls.load(Ordering::SeqCst), 3);
        // Resync did not replace the installed inode or its contents.
        assert!(identity == installed_identity(&File::open(&path).unwrap()).unwrap());
        assert_eq!(fs::read(&path).unwrap(), installed);
        fs::remove_file(&source).unwrap();
        let reader = RecoveryStore::open(&f.root()).unwrap();
        let recovered = reader.restore(&key, false).unwrap().unwrap();
        assert_eq!(
            fs::read(&recovered.message.attachments[0]).unwrap(),
            attachment_bytes
        );
        let recaptured = DraftUpdate {
            key: recovered.key,
            generation: recovered.generation,
            source_address: recovered.source_address,
            retention: if recovered.sensitive {
                Retention::DecryptedPlaintextOptIn
            } else {
                Retention::Ordinary
            },
            message: recovered.message,
            attachment_lease: Some(recovered.attachment_lease),
        };
        // Compare ALL Record fields, including full origin/calendar metadata and
        // the restored attachment name/bytes, not only visible composer inputs.
        assert_eq!(
            serde_json::to_vec(&Record::capture(&recaptured).unwrap()).unwrap(),
            expected
        );
        for generation in [8, 9] {
            assert!(
                writer
                    .submit(WriterCommand::Update(f.update(generation)))
                    .is_ok()
            );
            assert!(writer.submit(WriterCommand::Flush).is_ok());
            assert!(matches!(
                events.recv_timeout(Duration::from_secs(2)).unwrap(),
                WriterEvent::Failed {
                    error: RecoveryError::StaleGeneration,
                    ..
                }
            ));
        }
        assert_eq!(sync_calls.load(Ordering::SeqCst), 3);
        drop(writer);
        assert!(matches!(
            events.recv_timeout(Duration::from_secs(2)),
            Err(mpsc::RecvTimeoutError::Disconnected)
        ));
    }

    #[test]
    fn uncertain_resync_rejects_every_changed_record_field_without_consuming_permission() {
        use std::sync::atomic::Ordering;
        let f = Fixture::new();
        let (mut store, fault, sync_calls) = directory_sync_fault_store(&f.root());
        let source = f.file("original.bin", b"attachment");
        let renamed = f.file("renamed.bin", b"attachment");
        let make_update = || {
            let mut update = f.update(9);
            update
                .message
                .attachments
                .push(source.to_string_lossy().into_owned());
            update
        };
        let original = make_update();
        assert!(matches!(store.save(&original), Err(RecoveryError::Io(_))));
        fault.store(false, Ordering::SeqCst);
        let mutations: &[fn(&mut DraftUpdate)] = &[
            |u| u.source_address = "other@example.test".into(),
            |u| u.retention = Retention::DecryptedPlaintextOptIn,
            |u| u.message.from_alias = None,
            |u| u.message.to.push('x'),
            |u| u.message.cc.push('x'),
            |u| u.message.bcc.push('x'),
            |u| u.message.reply_to.push('x'),
            |u| u.message.subject.push('x'),
            |u| u.message.body.push('x'),
            |u| u.message.html.push('x'),
            |u| u.message.in_reply_to.push('x'),
            |u| u.message.references.push('x'),
            |u| u.message.draft_origin.as_mut().unwrap().folder_id += 1,
            |u| u.message.draft_origin.as_mut().unwrap().path.push('x'),
            |u| u.message.draft_origin.as_mut().unwrap().uid += 1,
            |u| u.message.draft_origin = None,
            |u| u.message.outbox_origin = None,
            |u| u.message.sign = false,
            |u| u.message.encrypt = false,
            |u| u.message.send_at = None,
            |u| u.message.calendar.as_mut().unwrap().ics.push('x'),
            |u| u.message.calendar.as_mut().unwrap().method.push('x'),
            |u| u.message.calendar = None,
            |u| u.message.attachments.clear(),
        ];
        for mutate in mutations {
            let mut changed = make_update();
            mutate(&mut changed);
            assert!(matches!(
                store.save(&changed),
                Err(RecoveryError::StaleGeneration)
            ));
        }
        let mut different_name = make_update();
        different_name.message.attachments = vec![renamed.to_string_lossy().into_owned()];
        assert!(matches!(
            store.save(&different_name),
            Err(RecoveryError::StaleGeneration)
        ));
        assert_eq!(sync_calls.load(Ordering::SeqCst), 1);
        assert_eq!(store.save(&original).unwrap(), SaveOutcome::Durable);
        assert!(matches!(
            store.save(&original),
            Err(RecoveryError::StaleGeneration)
        ));
    }

    #[test]
    fn uncertain_permission_is_store_owned_and_rejects_another_writers_identical_byte_replacement()
    {
        use std::sync::atomic::Ordering;
        let f = Fixture::new();
        let (mut store, fault, sync_calls) = directory_sync_fault_store(&f.root());
        let update = f.update(9);
        assert!(matches!(store.save(&update), Err(RecoveryError::Io(_))));
        fault.store(false, Ordering::SeqCst);
        // A fresh store cannot acknowledge even exactly equal fields/bytes.
        let mut other = RecoveryStore::open(&f.root()).unwrap();
        assert!(matches!(
            other.save(&update),
            Err(RecoveryError::StaleGeneration)
        ));
        let path = f.root().join(update.key.filename());
        let bytes = fs::read(&path).unwrap();
        let replacement = f.root().join("other-writer.pending");
        let mut file = create_private_file(&replacement).unwrap();
        file.write_all(&bytes).unwrap();
        file.sync_all().unwrap();
        fs::rename(&replacement, &path).unwrap();
        File::open(f.root()).unwrap().sync_all().unwrap();
        assert_eq!(fs::read(&path).unwrap(), bytes);
        assert!(matches!(
            store.save(&update),
            Err(RecoveryError::StaleGeneration)
        ));
        assert_eq!(sync_calls.load(Ordering::SeqCst), 1);
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }

    #[test]
    fn newer_durable_generation_retires_uncertain_resync_permission() {
        use std::sync::atomic::Ordering;
        let f = Fixture::new();
        let (mut store, fault, _) = directory_sync_fault_store(&f.root());
        assert!(matches!(
            store.save(&f.update(9)),
            Err(RecoveryError::Io(_))
        ));
        fault.store(false, Ordering::SeqCst);
        assert_eq!(store.save(&f.update(10)).unwrap(), SaveOutcome::Durable);
        assert!(store.uncertain.is_empty());
        for generation in [9, 10] {
            assert!(matches!(
                store.save(&f.update(generation)),
                Err(RecoveryError::StaleGeneration)
            ));
        }
    }

    #[test]
    fn disk_full_precommit_fault_preserves_good_copy_and_removes_pending_file() {
        let f = Fixture::new();
        let original = f.update(1);
        RecoveryStore::open(&f.root())
            .unwrap()
            .save(&original)
            .unwrap();
        let mut store = RecoveryStore::open_with_commit_guard(&f.root(), || {
            Err(io::Error::from_raw_os_error(28))
        })
        .unwrap();
        let mut changed = f.update(2);
        changed.message.body = "not committed".into();
        assert!(matches!(store.save(&changed), Err(RecoveryError::Io(_))));
        assert_eq!(
            store
                .restore(&original.key, false)
                .unwrap()
                .unwrap()
                .message
                .body,
            original.message.body
        );
        assert_eq!(fs::read_dir(f.root()).unwrap().count(), 1);
    }

    #[test]
    fn unsupported_corrupt_and_oversize_records_are_not_overwritten() {
        let f = Fixture::new();
        let mut store = RecoveryStore::open(&f.root()).unwrap();
        let update = f.update(1);
        let path = f.root().join(update.key.filename());
        for bytes in [
            b"MEGAMAIL-DRAFT 99\n{}".as_slice(),
            b"MEGAMAIL-DRAFT 1\n{broken".as_slice(),
        ] {
            fs::write(&path, bytes).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
            assert!(store.save(&update).is_err());
            assert_eq!(fs::read(&path).unwrap(), bytes);
        }
        let file = OpenOptions::new().write(true).open(&path).unwrap();
        file.set_len(RECORD_LIMIT as u64 + 1).unwrap();
        assert!(matches!(store.save(&update), Err(RecoveryError::TooLarge)));
        assert_eq!(file.metadata().unwrap().len(), RECORD_LIMIT as u64 + 1);
    }

    #[test]
    fn stale_generation_and_newer_edit_block_old_send_cleanup() {
        let f = Fixture::new();
        let mut store = RecoveryStore::open(&f.root()).unwrap();
        let update = f.update(5);
        store.save(&update).unwrap();
        assert!(matches!(
            store.save(&f.update(4)),
            Err(RecoveryError::StaleGeneration)
        ));
        assert!(matches!(
            store.clear_after_send(&update.key, 4, 100, 7, 100, SendOutcome::Sent, false),
            Err(RecoveryError::StaleGeneration)
        ));
        assert!(store.restore(&update.key, false).unwrap().is_some());
    }

    #[test]
    fn queued_uncertain_failed_wrong_request_and_wrong_account_never_clear_recovery() {
        let f = Fixture::new();
        let mut store = RecoveryStore::open(&f.root()).unwrap();
        let update = f.update(1);
        store.save(&update).unwrap();
        for (account, request, outcome, uncertain) in [
            (7, 100, SendOutcome::Queued, false),
            (7, 100, SendOutcome::Sent, true),
            (7, 100, SendOutcome::Failed, false),
            (8, 100, SendOutcome::Sent, false),
            (7, 101, SendOutcome::Sent, false),
        ] {
            assert!(
                !store
                    .clear_after_send(&update.key, 1, 100, account, request, outcome, uncertain)
                    .unwrap()
            );
            assert!(store.restore(&update.key, false).unwrap().is_some());
        }
        assert!(
            store
                .clear_after_send(&update.key, 1, 100, 7, 100, SendOutcome::Sent, false)
                .unwrap()
        );
        assert!(store.restore(&update.key, false).unwrap().is_none());
    }

    #[test]
    fn decrypted_session_policy_does_not_read_or_persist_attachments_and_optin_is_explicit() {
        let f = Fixture::new();
        let mut store = RecoveryStore::open(&f.root()).unwrap();
        let mut update = f.update(1);
        update.retention = Retention::DecryptedSessionOnly;
        update
            .message
            .attachments
            .push("/not/a/real/attachment".into());
        assert_eq!(store.save(&update).unwrap(), SaveOutcome::SessionOnly);
        assert_eq!(fs::read_dir(f.root()).unwrap().count(), 0);
        update.message.attachments.clear();
        update.retention = Retention::DecryptedPlaintextOptIn;
        store.save(&update).unwrap();
        assert!(matches!(
            store.restore(&update.key, false),
            Err(RecoveryError::SensitiveConsentRequired)
        ));
        assert!(store.restore(&update.key, true).unwrap().unwrap().sensitive);
    }

    #[test]
    fn unsafe_paths_links_hardlinks_and_permissions_fail_closed() {
        let f = Fixture::new();
        assert!(DraftKey::from_parts(7, "../../../../../../../../..".into()).is_err());
        let source = f.file("source.bin", b"private");
        let source_link = f.path.join("link.bin");
        symlink(&source, &source_link).unwrap();
        let mut update = f.update(1);
        update
            .message
            .attachments
            .push(source_link.to_string_lossy().into_owned());
        let mut store = RecoveryStore::open(&f.root()).unwrap();
        assert!(store.save(&update).is_err());
        let root_link = f.path.join("linked-root");
        symlink(f.root(), &root_link).unwrap();
        assert!(RecoveryStore::open(&root_link).is_err());
        let record_path = f.root().join(update.key.filename());
        symlink(&source, &record_path).unwrap();
        assert!(store.save(&f.update(1)).is_err());
        assert_eq!(fs::read(&source).unwrap(), b"private");
        fs::remove_file(&record_path).unwrap();
        fs::hard_link(&source, &record_path).unwrap();
        assert!(store.restore(&update.key, false).is_err());
        fs::remove_file(&record_path).unwrap();
        store.save(&f.update(1)).unwrap();
        fs::set_permissions(&record_path, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(store.restore(&update.key, false).is_err());
        fs::set_permissions(f.root(), fs::Permissions::from_mode(0o755)).unwrap();
        assert!(RecoveryStore::open(&f.root()).is_err());
    }

    #[test]
    fn missing_oversize_and_too_many_attachments_preserve_previous_generation() {
        let f = Fixture::new();
        let mut store = RecoveryStore::open(&f.root()).unwrap();
        let original = f.update(1);
        store.save(&original).unwrap();
        let mut changed = f.update(2);
        changed
            .message
            .attachments
            .push(f.path.join("missing").to_string_lossy().into_owned());
        assert!(store.save(&changed).is_err());
        let huge = f.file("huge.bin", b"");
        OpenOptions::new()
            .write(true)
            .open(&huge)
            .unwrap()
            .set_len(FILE_LIMIT as u64 + 1)
            .unwrap();
        changed.message.attachments = vec![huge.to_string_lossy().into_owned()];
        assert!(matches!(store.save(&changed), Err(RecoveryError::TooLarge)));
        changed.message.attachments = vec![huge.to_string_lossy().into_owned(); COUNT_LIMIT + 1];
        assert!(matches!(store.save(&changed), Err(RecoveryError::TooLarge)));
        assert_eq!(
            store
                .restore(&original.key, false)
                .unwrap()
                .unwrap()
                .generation,
            1
        );
    }

    #[test]
    fn crash_pending_and_owned_staging_cleanup_never_remove_final_records_or_follow_links() {
        let f = Fixture::new();
        let mut store = RecoveryStore::open(&f.root()).unwrap();
        let update = f.update(1);
        store.save(&update).unwrap();
        let pending = f.root().join("abcdefghijklmnopqrstuvwx.pending");
        create_private_file(&pending)
            .unwrap()
            .write_all(b"unfinished")
            .unwrap();
        let stage = f.root().join("abcdefghijklmnopqrstuvwx.stage");
        create_private_dir(&stage).unwrap();
        create_private_dir(&stage.join("0")).unwrap();
        create_private_file(&stage.join("0/bytes.bin"))
            .unwrap()
            .write_all(b"owned")
            .unwrap();
        let linked = f.root().join("zyxwvutsrqponmlkjihgfedc.stage");
        symlink(&f.path, &linked).unwrap();
        assert_eq!(store.prune_stale_work(64).unwrap(), 2);
        assert!(!pending.exists());
        assert!(!stage.exists());
        assert!(
            fs::symlink_metadata(linked)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert!(store.restore(&update.key, false).unwrap().is_some());
    }

    #[test]
    fn writer_flushes_the_latest_generation_and_rejects_late_updates_after_discard() {
        let f = Fixture::new();
        let store = RecoveryStore::open(&f.root()).unwrap();
        let (writer, events) = RecoveryWriter::start(store, Duration::from_secs(5)).unwrap();
        let first = f.update(1);
        let second = f.update(2);
        let key = second.key.clone();
        assert!(writer.submit(WriterCommand::Update(first)).is_ok());
        assert!(writer.submit(WriterCommand::Update(second)).is_ok());
        assert!(writer.submit(WriterCommand::Flush).is_ok());
        assert!(matches!(
            events.recv_timeout(Duration::from_secs(2)).unwrap(),
            WriterEvent::Superseded { generation: 1, .. }
        ));
        assert!(matches!(
            events.recv_timeout(Duration::from_secs(2)).unwrap(),
            WriterEvent::Saved {
                generation: 2,
                outcome: SaveOutcome::Durable,
                ..
            }
        ));
        assert!(
            writer
                .submit(WriterCommand::Discard { key, generation: 2 })
                .is_ok()
        );
        assert!(matches!(
            events.recv_timeout(Duration::from_secs(2)).unwrap(),
            WriterEvent::Discarded { removed: true, .. }
        ));
        assert!(writer.submit(WriterCommand::Update(f.update(2))).is_ok());
        assert!(matches!(
            events.recv_timeout(Duration::from_secs(2)).unwrap(),
            WriterEvent::Failed {
                error: RecoveryError::StaleGeneration,
                ..
            }
        ));
    }

    #[test]
    fn restore_reports_changed_from_identity_instead_of_silently_substituting() {
        let f = Fixture::new();
        let mut store = RecoveryStore::open(&f.root()).unwrap();
        let mut update = f.update(1);
        update.message.from_alias = None;
        update.source_address = "old@example.test".into();
        store.save(&update).unwrap();
        assert!(compose::prepare_local_recovery(&store, &update.key, &account(), false).is_err());
        assert!(store.restore(&update.key, false).unwrap().is_some());
        assert!(
            compose::prepare_recovery_update(
                &update.message,
                &DraftKey::new(8).unwrap(),
                2,
                &account(),
                Retention::Ordinary,
                None
            )
            .is_err()
        );
    }
}

//! Private local address book. No EDS/OS contacts, provider access or mail-history mining.
//!
//! Call `LocalContacts::open` and all disk operations on a background lane. The caller
//! supplies MegaMail's private data root and a backend-qualified stable account ID.
//! A snapshot is immutable completion input; account switches must replace it, not merge it.
//! See docs/research/batch2-contacts-calendar.md for integration and recovery obligations.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

const STORE_VERSION: u32 = 1;
const MAX_STORE_BYTES: usize = 4 * 1024 * 1024;
const MAX_IMPORT_BYTES: usize = 1024 * 1024;
const MAX_CONTACTS: usize = 5000;
const MAX_EMAILS: usize = 16;
const MAX_LINE_BYTES: usize = 8192;

/// Qualifies IDs that can overlap between backend namespaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContactBackend {
    Direct,
    Thunderbird,
}

/// A local book belongs to exactly one account; it is not a provider address book.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContactScope {
    pub backend: ContactBackend,
    pub account_id: u32,
}

/// User-supplied fields. Names are never inferred from addresses or structured `N` fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContactDraft {
    pub display_name: String,
    pub emails: Vec<String>,
    pub notes: String,
}

/// IDs are monotonic within a scope, never recycled after delete.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocalContact {
    pub id: u64,
    pub fields: ContactDraft,
}

/// Read-only results for list/detail and compose completion.
#[derive(Debug, Clone)]
pub struct ContactSnapshot {
    pub scope: ContactScope,
    pub revision: u64,
    pub contacts: Vec<LocalContact>,
}

/// Raw fields, not a preformatted recipient header. Compose must escape the name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddressSuggestion {
    pub scope: ContactScope,
    pub contact_id: u64,
    pub display_name: String,
    pub email: String,
}

impl ContactSnapshot {
    /// Export only this immutable local book; no linked photos or remote fields.
    pub fn vcard_bytes(&self) -> Result<Vec<u8>, ContactError> {
        fn escaped(value: &str) -> String {
            value
                .replace('\\', "\\\\")
                .replace('\n', "\\n")
                .replace(';', "\\;")
                .replace(',', "\\,")
        }
        fn line(output: &mut String, value: &str) -> Result<(), ContactError> {
            if value.len() > MAX_LINE_BYTES {
                return Err(ContactError::Invalid(
                    "vCard field exceeds the import line limit",
                ));
            }
            let mut width = 0;
            for ch in value.chars() {
                if width + ch.len_utf8() > 75 {
                    output.push_str("\r\n ");
                    width = 1;
                }
                output.push(ch);
                width += ch.len_utf8();
            }
            output.push_str("\r\n");
            Ok(())
        }
        if self.contacts.len() > MAX_CONTACTS {
            return Err(ContactError::Invalid("too many contacts to export"));
        }
        if self.contacts.is_empty() {
            return Err(ContactError::Invalid("no contacts to export"));
        }
        let mut output = String::new();
        for contact in &self.contacts {
            let fields = normalize_draft(contact.fields.clone())?;
            output.push_str("BEGIN:VCARD\r\nVERSION:4.0\r\n");
            line(
                &mut output,
                &format!("FN:{}", escaped(&fields.display_name)),
            )?;
            for email in fields.emails {
                line(&mut output, &format!("EMAIL:{email}"))?;
            }
            if !fields.notes.is_empty() {
                line(&mut output, &format!("NOTE:{}", escaped(&fields.notes)))?;
            }
            output.push_str("END:VCARD\r\n");
            if output.len() > MAX_IMPORT_BYTES {
                return Err(ContactError::Invalid("vCard export exceeds 1 MiB"));
            }
        }
        Ok(output.into_bytes())
    }

    /// Stable, local-only prefix completion, capped at 20 results. Empty/oversize queries
    /// return none. No cross-account fallback, network lookup or insertion side effect.
    pub fn complete(&self, query: &str, limit: usize) -> Vec<AddressSuggestion> {
        let query = query.trim();
        if limit == 0
            || query.is_empty()
            || query.len() > 256
            || query.chars().any(char::is_control)
        {
            return Vec::new();
        }
        let query = query.to_lowercase();
        let mut suggestions = Vec::new();
        // Iterate the bounded store, retain only the requested result window.
        for contact in &self.contacts {
            let name_match = contact
                .fields
                .display_name
                .to_lowercase()
                .starts_with(&query);
            for email in &contact.fields.emails {
                if name_match || email.to_lowercase().starts_with(&query) {
                    suggestions.push(AddressSuggestion {
                        scope: self.scope,
                        contact_id: contact.id,
                        display_name: contact.fields.display_name.clone(),
                        email: email.clone(),
                    });
                    if suggestions.len() >= limit.min(20) {
                        return suggestions;
                    }
                }
            }
        }
        suggestions
    }
}

/// A single atomic mutation. Update replaces fields; import conservatively merges emails.
pub enum ContactChange {
    Create(ContactDraft),
    Update { id: u64, fields: ContactDraft },
    Delete { id: u64 },
}

/// Safe error labels contain no personal fields or paths.
#[derive(Debug)]
pub enum ContactError {
    Io(io::Error),
    Invalid(&'static str),
    Busy,
    Conflict,
    NotFound,
    /// Rename succeeded; directory fsync failed. Reload, do not blindly repeat creation.
    DurabilityUncertain {
        revision: u64,
    },
    UnsupportedPlatform,
}

impl std::fmt::Display for ContactError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(_) => f.write_str("Local contacts storage I/O failed"),
            Self::Invalid(reason) => write!(f, "Local contacts rejected: {reason}"),
            Self::Busy => f.write_str("Local contacts are busy; retry or recover the stale lock"),
            Self::Conflict => f.write_str("Contacts changed; reload before saving"),
            Self::NotFound => f.write_str("Contact no longer exists"),
            Self::DurabilityUncertain { .. } => {
                f.write_str("Contacts saved, but crash durability is uncertain; reload")
            }
            Self::UnsupportedPlatform => {
                f.write_str("Private contacts storage requires Unix permission support")
            }
        }
    }
}
impl std::error::Error for ContactError {}
impl From<io::Error> for ContactError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Book {
    version: u32,
    scope: ContactScope,
    revision: u64,
    next_id: u64,
    contacts: Vec<LocalContact>,
}

impl Book {
    fn snapshot(self) -> ContactSnapshot {
        ContactSnapshot {
            scope: self.scope,
            revision: self.revision,
            contacts: self.contacts,
        }
    }

    fn validate(&self, scope: ContactScope) -> Result<(), ContactError> {
        if self.version != STORE_VERSION {
            return Err(ContactError::Invalid("unsupported store version"));
        }
        if self.scope != scope {
            return Err(ContactError::Invalid("account scope mismatch"));
        }
        if self.contacts.len() > MAX_CONTACTS {
            return Err(ContactError::Invalid("too many contacts"));
        }
        let mut ids = BTreeSet::new();
        let mut emails = BTreeSet::new();
        for contact in &self.contacts {
            if contact.id == 0 || contact.id >= self.next_id || !ids.insert(contact.id) {
                return Err(ContactError::Invalid("invalid contact IDs"));
            }
            if normalize_draft(contact.fields.clone())? != contact.fields {
                return Err(ContactError::Invalid("noncanonical contact fields"));
            }
            for email in &contact.fields.emails {
                if !emails.insert(email) {
                    return Err(ContactError::Invalid("duplicate scoped address"));
                }
            }
        }
        if self.next_id == 0 {
            return Err(ContactError::Invalid("invalid ID counter"));
        }
        Ok(())
    }

    fn ensure_unique(
        &self,
        fields: &ContactDraft,
        except: Option<u64>,
    ) -> Result<(), ContactError> {
        if self.contacts.iter().any(|c| {
            Some(c.id) != except && c.fields.emails.iter().any(|e| fields.emails.contains(e))
        }) {
            return Err(ContactError::Invalid(
                "address already belongs to another contact",
            ));
        }
        Ok(())
    }

    fn create(&mut self, fields: ContactDraft) -> Result<(), ContactError> {
        if self.contacts.len() >= MAX_CONTACTS {
            return Err(ContactError::Invalid("too many contacts"));
        }
        self.ensure_unique(&fields, None)?;
        let id = self.next_id;
        self.next_id = id
            .checked_add(1)
            .ok_or(ContactError::Invalid("ID counter exhausted"))?;
        self.contacts.push(LocalContact { id, fields });
        Ok(())
    }
}

/// Single-file versioned store; atomic replacement and revision checks protect edits.
/// Clone handles are safe: the exclusive directory lock serializes all mutations.
#[derive(Clone)]
pub struct LocalContacts {
    directory: PathBuf,
    file: PathBuf,
    scope: ContactScope,
}

impl LocalContacts {
    /// Open only beneath an existing private, non-symlink MegaMail data root. Does not
    /// discover profiles or repair permissions. Non-Unix platforms fail closed.
    pub fn open(private_data_root: &Path, scope: ContactScope) -> Result<Self, ContactError> {
        if scope.account_id == 0 {
            return Err(ContactError::Invalid("stable account ID required"));
        }
        check_private(private_data_root, true)?;
        let directory = private_data_root.join("contacts-v1");
        let created = create_private_directory(&directory)?;
        check_private(&directory, true)?;
        if created {
            File::open(private_data_root)?.sync_all()?;
        }
        let backend = match scope.backend {
            ContactBackend::Direct => "direct",
            ContactBackend::Thunderbird => "thunderbird",
        };
        let file = directory.join(format!("{backend}-{}.json", scope.account_id));
        let store = Self {
            directory,
            file,
            scope,
        };
        // Missing is a new book; corruption/version/scope errors never reset a book.
        store.read_book()?;
        Ok(store)
    }

    /// Bounded disk read, without UI-thread I/O. Missing storage is revision zero.
    pub fn load(&self) -> Result<ContactSnapshot, ContactError> {
        Ok(self.read_book()?.snapshot())
    }

    /// Persist a CRUD command against the displayed revision. No write on validation
    /// failure or conflict. See `DurabilityUncertain` before retrying failed saves.
    pub fn apply(
        &self,
        expected_revision: u64,
        change: ContactChange,
    ) -> Result<ContactSnapshot, ContactError> {
        self.transaction(expected_revision, |book| {
            match change {
                ContactChange::Create(fields) => book.create(normalize_draft(fields)?)?,
                ContactChange::Update { id, fields } => {
                    let fields = normalize_draft(fields)?;
                    book.ensure_unique(&fields, Some(id))?;
                    let contact = book
                        .contacts
                        .iter_mut()
                        .find(|c| c.id == id)
                        .ok_or(ContactError::NotFound)?;
                    contact.fields = fields;
                }
                ContactChange::Delete { id } => {
                    let index = book
                        .contacts
                        .iter()
                        .position(|c| c.id == id)
                        .ok_or(ContactError::NotFound)?;
                    book.contacts.remove(index);
                }
            }
            Ok(())
        })
    }

    /// Import UTF-8 vCard 3/4 bytes selected by the user, all-or-nothing. No source
    /// path, URI, PHOTO, OS address book or credential is opened. Duplicate identities
    /// preserve existing names/notes; overlapping emails add to one existing record.
    pub fn import_vcards(
        &self,
        expected_revision: u64,
        bytes: &[u8],
    ) -> Result<(ContactSnapshot, ImportReport), ContactError> {
        let parsed = parse_vcards(bytes)?;
        let mut report = ImportReport {
            ignored_properties: parsed.ignored_properties,
            ..ImportReport::default()
        };
        let snapshot = self.transaction(expected_revision, |book| {
            for fields in parsed.contacts {
                let matches: Vec<usize> = book
                    .contacts
                    .iter()
                    .enumerate()
                    .filter(|(_, c)| c.fields.emails.iter().any(|e| fields.emails.contains(e)))
                    .map(|(i, _)| i)
                    .collect();
                match matches.as_slice() {
                    [] => {
                        book.create(fields)?;
                        report.created += 1;
                    }
                    [index] => {
                        let contact = &mut book.contacts[*index];
                        let before = contact.fields.emails.len();
                        for email in fields.emails {
                            if !contact.fields.emails.contains(&email) {
                                contact.fields.emails.push(email);
                            }
                        }
                        contact.fields = normalize_draft(contact.fields.clone())?;
                        if contact.fields.emails.len() == before {
                            report.duplicates += 1;
                        } else {
                            report.merged += 1;
                        }
                    }
                    _ => {
                        return Err(ContactError::Invalid(
                            "import bridges separate contacts; edit explicitly",
                        ));
                    }
                }
            }
            Ok(())
        })?;
        Ok((snapshot, report))
    }

    fn transaction(
        &self,
        expected_revision: u64,
        mutate: impl FnOnce(&mut Book) -> Result<(), ContactError>,
    ) -> Result<ContactSnapshot, ContactError> {
        check_private(&self.directory, true)?;
        let lock_path = self.directory.join("write.lock");
        let lock = Lock::acquire(lock_path)?;
        let mut book = self.read_book()?;
        if book.revision != expected_revision {
            return Err(ContactError::Conflict);
        }
        mutate(&mut book)?;
        book.revision = book
            .revision
            .checked_add(1)
            .ok_or(ContactError::Invalid("revision exhausted"))?;
        book.validate(self.scope)?;
        let bytes =
            serde_json::to_vec(&book).map_err(|_| ContactError::Invalid("cannot encode store"))?;
        if bytes.len() > MAX_STORE_BYTES {
            return Err(ContactError::Invalid("store byte limit"));
        }
        // The temp file lives INSIDE the exclusively created private lock directory;
        // a stale lock is never overwritten or automatically broken.
        let temporary = lock.path.join("next.json");
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut output = options.open(&temporary)?;
        output.write_all(&bytes)?;
        output.sync_all()?;
        drop(output);
        fs::rename(&temporary, &self.file)?;
        let synced = File::open(&self.directory)
            .and_then(|dir| dir.sync_all())
            .is_ok();
        drop(lock);
        if !synced {
            return Err(ContactError::DurabilityUncertain {
                revision: book.revision,
            });
        }
        Ok(book.snapshot())
    }

    fn read_book(&self) -> Result<Book, ContactError> {
        check_private(&self.directory, true)?;
        match fs::symlink_metadata(&self.file) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                return Ok(Book {
                    version: STORE_VERSION,
                    scope: self.scope,
                    revision: 0,
                    next_id: 1,
                    contacts: Vec::new(),
                });
            }
            Err(e) => return Err(e.into()),
            Ok(_) => check_private(&self.file, false)?,
        }
        let file = File::open(&self.file)?;
        let metadata = file.metadata()?;
        if !metadata.is_file() || metadata.len() > MAX_STORE_BYTES as u64 {
            return Err(ContactError::Invalid("store byte limit"));
        }
        let mut bytes = Vec::new();
        file.take((MAX_STORE_BYTES + 1) as u64)
            .read_to_end(&mut bytes)?;
        if bytes.len() > MAX_STORE_BYTES {
            return Err(ContactError::Invalid("store byte limit"));
        }
        let book: Book =
            serde_json::from_slice(&bytes).map_err(|_| ContactError::Invalid("malformed store"))?;
        book.validate(self.scope)?;
        Ok(book)
    }
}

struct Lock {
    path: PathBuf,
}
impl Lock {
    fn acquire(path: PathBuf) -> Result<Self, ContactError> {
        match create_private_directory_exclusive(&path) {
            Ok(()) => Ok(Self { path }),
            Err(ContactError::Io(e)) if e.kind() == io::ErrorKind::AlreadyExists => {
                Err(ContactError::Busy)
            }
            Err(e) => Err(e),
        }
    }
}
impl Drop for Lock {
    fn drop(&mut self) {
        // Only our known temp path; never recursively remove an arbitrary directory.
        let _ = fs::remove_file(self.path.join("next.json"));
        let _ = fs::remove_dir(&self.path);
    }
}

fn create_private_directory(path: &Path) -> Result<bool, ContactError> {
    match create_private_directory_exclusive(path) {
        Err(ContactError::Io(e)) if e.kind() == io::ErrorKind::AlreadyExists => Ok(false),
        Ok(()) => Ok(true),
        Err(error) => Err(error),
    }
}
fn create_private_directory_exclusive(path: &Path) -> Result<(), ContactError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        fs::DirBuilder::new().mode(0o700).create(path)?;
        Ok(())
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Err(ContactError::UnsupportedPlatform)
    }
}
fn check_private(path: &Path, directory: bool) -> Result<(), ContactError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        let metadata = fs::symlink_metadata(path)?;
        if metadata.file_type().is_symlink()
            || (directory && !metadata.is_dir())
            || (!directory && !metadata.is_file())
            || metadata.permissions().mode() & 0o077 != 0
            || (!directory && metadata.nlink() != 1)
        {
            return Err(ContactError::Invalid("storage is not private or is a link"));
        }
        Ok(())
    }
    #[cfg(not(unix))]
    {
        let _ = (path, directory);
        Err(ContactError::UnsupportedPlatform)
    }
}

/// Conservative ASCII addr-spec subset. Preserve local-part case and plus/dot spelling;
/// lowercase only the DNS domain. Reject headers, lists, EAI and quoted local parts.
pub fn normalize_address(value: &str) -> Result<String, ContactError> {
    let value = value.trim();
    if value.len() > 254 || !value.is_ascii() {
        return Err(ContactError::Invalid("unsupported email address"));
    }
    let (local, domain) = value
        .split_once('@')
        .ok_or(ContactError::Invalid("invalid email address"))?;
    if local.is_empty()
        || local.len() > 64
        || local.starts_with('.')
        || local.ends_with('.')
        || local.contains("..")
        || !local
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"!#$%&'*+-/=?^_`{|}~.".contains(&c))
        || domain.is_empty()
        || domain.len() > 253
        || domain.split('.').any(|label| {
            label.is_empty()
                || label.len() > 63
                || label.starts_with('-')
                || label.ends_with('-')
                || !label
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || c == b'-')
        })
    {
        return Err(ContactError::Invalid("invalid email address"));
    }
    Ok(format!("{local}@{}", domain.to_ascii_lowercase()))
}
fn normalize_draft(mut draft: ContactDraft) -> Result<ContactDraft, ContactError> {
    draft.display_name = draft.display_name.trim().to_string();
    if draft.display_name.len() > 256
        || draft.display_name.chars().any(char::is_control)
        || draft.notes.len() > 4096
        || draft
            .notes
            .chars()
            .any(|c| c.is_control() && !matches!(c, '\n' | '\t'))
        || draft.emails.is_empty()
        || draft.emails.len() > MAX_EMAILS
    {
        return Err(ContactError::Invalid(
            "contact field limit or control character",
        ));
    }
    let emails: Result<BTreeSet<String>, _> =
        draft.emails.iter().map(|e| normalize_address(e)).collect();
    draft.emails = emails?.into_iter().collect();
    Ok(draft)
}

/// Import losses are explicit; ignored fields are never persisted or fetched.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ImportReport {
    pub created: usize,
    pub merged: usize,
    pub duplicates: usize,
    pub ignored_properties: usize,
}
struct ParsedCards {
    contacts: Vec<ContactDraft>,
    ignored_properties: usize,
}

fn parse_vcards(bytes: &[u8]) -> Result<ParsedCards, ContactError> {
    if bytes.is_empty() || bytes.len() > MAX_IMPORT_BYTES {
        return Err(ContactError::Invalid("vCard byte limit"));
    }
    let text =
        std::str::from_utf8(bytes).map_err(|_| ContactError::Invalid("vCard must be UTF-8"))?;
    let mut lines: Vec<String> = Vec::new();
    for raw in text.split('\n') {
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        if line.contains('\r') || line.contains('\0') || line.len() > MAX_LINE_BYTES {
            return Err(ContactError::Invalid("invalid vCard line"));
        }
        if let Some(rest) = line.strip_prefix([' ', '\t']) {
            let previous = lines
                .last_mut()
                .ok_or(ContactError::Invalid("orphan vCard continuation"))?;
            if previous.len() + rest.len() > MAX_LINE_BYTES {
                return Err(ContactError::Invalid("vCard unfolded line limit"));
            }
            previous.push_str(rest);
        } else if !line.is_empty() {
            lines.push(line.to_string());
        }
    }
    let mut result = ParsedCards {
        contacts: Vec::new(),
        ignored_properties: 0,
    };
    let mut current: Option<ContactDraft> = None;
    let mut version = false;
    let mut named = false;
    let mut noted = false;
    for line in lines {
        // Quoted parameters may contain ':'. Split only outside quotes.
        let mut quoted = false;
        let colon = line
            .char_indices()
            .find_map(|(i, c)| {
                if c == '"' {
                    quoted = !quoted;
                }
                (c == ':' && !quoted).then_some(i)
            })
            .ok_or(ContactError::Invalid("malformed vCard property"))?;
        let (head, value) = (&line[..colon], &line[colon + 1..]);
        let (property, params) = head.split_once(';').unwrap_or((head, ""));
        let property = property
            .rsplit('.')
            .next()
            .unwrap_or(property)
            .to_ascii_uppercase();
        match property.as_str() {
            "BEGIN" if value.eq_ignore_ascii_case("VCARD") && current.is_none() => {
                current = Some(ContactDraft {
                    display_name: String::new(),
                    emails: Vec::new(),
                    notes: String::new(),
                });
                version = false;
                named = false;
                noted = false;
            }
            "END" if value.eq_ignore_ascii_case("VCARD") && current.is_some() => {
                if !version {
                    return Err(ContactError::Invalid("vCard version missing"));
                }
                result.contacts.push(normalize_draft(
                    current
                        .take()
                        .ok_or(ContactError::Invalid("missing vCard"))?,
                )?);
                if result.contacts.len() > MAX_CONTACTS {
                    return Err(ContactError::Invalid("too many imported contacts"));
                }
            }
            "BEGIN" | "END" => return Err(ContactError::Invalid("invalid vCard nesting")),
            _ => {
                let fields = current
                    .as_mut()
                    .ok_or(ContactError::Invalid("property outside vCard"))?;
                if matches!(property.as_str(), "FN" | "EMAIL" | "NOTE") {
                    let upper = params.to_ascii_uppercase();
                    if upper.contains("ENCODING=")
                        || upper.contains("CHARSET=")
                        || upper.contains("VALUE=BINARY")
                    {
                        return Err(ContactError::Invalid("encoded contact field unsupported"));
                    }
                }
                match property.as_str() {
                    "VERSION"
                        if !version && params.is_empty() && matches!(value, "3.0" | "4.0") =>
                    {
                        version = true
                    }
                    "VERSION" => {
                        return Err(ContactError::Invalid(
                            "unsupported or duplicate vCard version",
                        ));
                    }
                    "FN" if !named => {
                        fields.display_name = unescape_text(value)?;
                        named = true;
                    }
                    "FN" => return Err(ContactError::Invalid("duplicate vCard name")),
                    "NOTE" if !noted => {
                        fields.notes = unescape_text(value)?;
                        noted = true;
                    }
                    "NOTE" => return Err(ContactError::Invalid("duplicate vCard note")),
                    "EMAIL" => {
                        if fields.emails.len() >= MAX_EMAILS {
                            return Err(ContactError::Invalid("too many email addresses"));
                        }
                        fields
                            .emails
                            .push(value.strip_prefix("mailto:").unwrap_or(value).to_string());
                    }
                    _ => result.ignored_properties += 1,
                }
            }
        }
    }
    if current.is_some() || result.contacts.is_empty() {
        return Err(ContactError::Invalid("incomplete or empty vCard document"));
    }
    Ok(result)
}
fn unescape_text(value: &str) -> Result<String, ContactError> {
    let mut output = String::new();
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            output.push(c);
            continue;
        }
        output.push(match chars.next() {
            Some('n' | 'N') => '\n',
            Some('\\') => '\\',
            Some(',') => ',',
            Some(';') => ';',
            _ => return Err(ContactError::Invalid("invalid vCard text escape")),
        });
    }
    Ok(output)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(1);
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "megamail-local-contacts-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            create_private_directory_exclusive(&path).unwrap();
            Self(path)
        }
        fn store(&self, backend: ContactBackend, account_id: u32) -> LocalContacts {
            LocalContacts::open(
                &self.0,
                ContactScope {
                    backend,
                    account_id,
                },
            )
            .unwrap()
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn draft(name: &str, email: &str) -> ContactDraft {
        ContactDraft {
            display_name: name.into(),
            emails: vec![email.into()],
            notes: String::new(),
        }
    }
    fn card(name: &str, email: &str) -> String {
        format!("BEGIN:VCARD\r\nVERSION:4.0\r\nFN:{name}\r\nEMAIL:{email}\r\nEND:VCARD\r\n")
    }

    #[test]
    fn export_roundtrips_unicode_escapes_and_folds_on_utf8_boundaries() {
        let fixture = Fixture::new();
        let store = fixture.store(ContactBackend::Direct, 7);
        let fields = ContactDraft {
            display_name: "Zoë, 山田; Fictional".into(),
            emails: vec!["person@example.test".into()],
            notes: format!("line one\nslash \\ {}", "界".repeat(100)),
        };
        let book = store
            .apply(0, ContactChange::Create(fields.clone()))
            .unwrap();
        let encoded = book.vcard_bytes().unwrap();
        for line in std::str::from_utf8(&encoded).unwrap().split("\r\n") {
            assert!(line.len() <= 75);
        }
        let mut oversized_line = book.clone();
        oversized_line.contacts[0].fields.notes = ";".repeat(4096);
        assert!(oversized_line.vcard_bytes().is_err());
        let parsed = parse_vcards(&encoded).unwrap();
        assert_eq!(parsed.contacts, vec![fields]);
        assert!(
            ContactSnapshot {
                scope: ContactScope {
                    backend: ContactBackend::Direct,
                    account_id: 7
                },
                revision: 0,
                contacts: vec![]
            }
            .vcard_bytes()
            .is_err()
        );
    }

    #[test]
    fn crud_reopens_real_store_and_does_not_reuse_deleted_ids() {
        let f = Fixture::new();
        let store = f.store(ContactBackend::Direct, 7);
        let created = store
            .apply(0, ContactChange::Create(draft("Ada", "Ada@EXAMPLE.test")))
            .unwrap();
        let id = created.contacts[0].id;
        let updated = store
            .apply(
                created.revision,
                ContactChange::Update {
                    id,
                    fields: draft("Ada Lovelace", "Ada@example.test"),
                },
            )
            .unwrap();
        assert_eq!(
            f.store(ContactBackend::Direct, 7).load().unwrap().contacts,
            updated.contacts
        );
        let deleted = store
            .apply(updated.revision, ContactChange::Delete { id })
            .unwrap();
        assert!(deleted.contacts.is_empty());
        let next = store
            .apply(
                deleted.revision,
                ContactChange::Create(draft("", "next@example.test")),
            )
            .unwrap();
        assert!(next.contacts[0].id > id);
    }
    #[test]
    fn completion_is_scoped_and_blank_names_stay_blank() {
        let f = Fixture::new();
        let store = f.store(ContactBackend::Direct, 7);
        let a = store
            .apply(0, ContactChange::Create(draft("", "Ada@example.test")))
            .unwrap();
        let other = f.store(ContactBackend::Thunderbird, 7).load().unwrap();
        assert!(other.complete("Ada", 20).is_empty());
        assert!(
            f.store(ContactBackend::Direct, 8)
                .load()
                .unwrap()
                .contacts
                .is_empty()
        );
        assert_eq!(a.complete("ada", 1)[0].display_name, "");
        assert!(a.complete("", 20).is_empty());
        assert!(a.complete("a", 0).is_empty());
    }
    #[test]
    fn domain_dedup_does_not_guess_provider_local_part_rules() {
        assert_eq!(
            normalize_address(" Ada+tag@EXAMPLE.test ").unwrap(),
            "Ada+tag@example.test"
        );
        let f = Fixture::new();
        let store = f.store(ContactBackend::Direct, 1);
        let a = store
            .apply(0, ContactChange::Create(draft("Ada", "Ada@example.test")))
            .unwrap();
        assert!(
            store
                .apply(
                    a.revision,
                    ContactChange::Create(draft("Other", "Ada@EXAMPLE.test"))
                )
                .is_err()
        );
        assert!(
            store
                .apply(
                    a.revision,
                    ContactChange::Create(draft("Other", "ada@example.test"))
                )
                .is_ok()
        );
    }
    #[test]
    fn stale_revision_and_busy_lock_never_replace_store() {
        let f = Fixture::new();
        let store = f.store(ContactBackend::Direct, 1);
        let a = store
            .apply(0, ContactChange::Create(draft("A", "a@example.test")))
            .unwrap();
        assert!(matches!(
            store.apply(
                0,
                ContactChange::Delete {
                    id: a.contacts[0].id
                }
            ),
            Err(ContactError::Conflict)
        ));
        let lock = Lock::acquire(store.directory.join("write.lock")).unwrap();
        assert!(matches!(
            store.apply(
                a.revision,
                ContactChange::Delete {
                    id: a.contacts[0].id
                }
            ),
            Err(ContactError::Busy)
        ));
        drop(lock);
        assert_eq!(store.load().unwrap().contacts, a.contacts);
    }
    #[test]
    fn vcard_import_folds_and_unescapes_without_fetching_photos() {
        let f = Fixture::new();
        let store = f.store(ContactBackend::Direct, 1);
        let raw = b"BEGIN:VCARD\r\nVERSION:3.0\r\nFN:Smith\\, Jo\r\n an\r\nitem1.EMAIL;TYPE=HOME:jo@example.test\r\nNOTE:one\\ntwo\r\nPHOTO;VALUE=URI:https://example.test/tracker\r\nEND:VCARD\r\n";
        let (a, report) = store.import_vcards(0, raw).unwrap();
        assert_eq!(a.contacts[0].fields.display_name, "Smith, Joan");
        assert_eq!(a.contacts[0].fields.notes, "one\ntwo");
        assert_eq!(report.ignored_properties, 1);
        let (_, repeat) = store
            .import_vcards(
                a.revision,
                card("Do not rename", "jo@EXAMPLE.test").as_bytes(),
            )
            .unwrap();
        assert_eq!(repeat.duplicates, 1);
        assert_eq!(
            store.load().unwrap().contacts[0].fields.display_name,
            "Smith, Joan"
        );
    }
    #[test]
    fn malformed_oversize_or_colliding_import_is_atomic() {
        let f = Fixture::new();
        let store = f.store(ContactBackend::Direct, 1);
        let a = store
            .apply(0, ContactChange::Create(draft("A", "a@example.test")))
            .unwrap();
        let broken = format!(
            "{}BEGIN:VCARD\r\nVERSION:4.0\r\nEMAIL:broken\r\nEND:VCARD\r\n",
            card("B", "b@example.test")
        );
        assert!(store.import_vcards(a.revision, broken.as_bytes()).is_err());
        assert!(
            store
                .import_vcards(a.revision, &vec![b'x'; MAX_IMPORT_BYTES + 1])
                .is_err()
        );
        assert_eq!(store.load().unwrap().contacts, a.contacts);
        for address in [
            "a@example.test\r\nBcc:evil@example.test",
            "Name <a@example.test>",
            "a..b@example.test",
            "a@-bad.test",
            "a@b@c",
        ] {
            assert!(normalize_address(address).is_err());
        }
    }
    #[test]
    fn corrupt_unknown_version_and_wrong_scope_are_not_reset() {
        let f = Fixture::new();
        let store = f.store(ContactBackend::Direct, 1);
        store
            .apply(0, ContactChange::Create(draft("A", "a@example.test")))
            .unwrap();
        let original = fs::read(&store.file).unwrap();
        let mut data: serde_json::Value = serde_json::from_slice(&original).unwrap();
        data["version"] = 99.into();
        fs::write(&store.file, serde_json::to_vec(&data).unwrap()).unwrap();
        assert!(store.load().is_err());
        data["version"] = 1.into();
        data["scope"]["account_id"] = 2.into();
        fs::write(&store.file, serde_json::to_vec(&data).unwrap()).unwrap();
        assert!(store.load().is_err());
        fs::write(&store.file, b"{").unwrap();
        assert!(store.load().is_err());
        assert!(
            store
                .apply(0, ContactChange::Create(draft("B", "b@example.test")))
                .is_err()
        );
    }
    #[test]
    fn import_cannot_bridge_two_existing_records_and_completion_stays_bounded() {
        let f = Fixture::new();
        let store = f.store(ContactBackend::Direct, 1);
        let cards: String = (0..25)
            .map(|i| card("Person", &format!("person{i}@example.test")))
            .collect();
        let (before, _) = store.import_vcards(0, cards.as_bytes()).unwrap();
        assert_eq!(before.complete("person", usize::MAX).len(), 20);
        let bridge = "BEGIN:VCARD\r\nVERSION:4.0\r\nFN:Combined\r\nEMAIL:person0@example.test\r\nEMAIL:person1@example.test\r\nEND:VCARD\r\n";
        assert!(
            store
                .import_vcards(before.revision, bridge.as_bytes())
                .is_err()
        );
        assert_eq!(store.load().unwrap().contacts, before.contacts);
    }

    #[test]
    fn utf8_import_preserves_explicit_names_and_rejects_encoded_fields() {
        let f = Fixture::new();
        let store = f.store(ContactBackend::Direct, 1);
        let (snapshot, _) = store
            .import_vcards(0, card("Zoë 山田", "zoe@example.test").as_bytes())
            .unwrap();
        assert_eq!(snapshot.contacts[0].fields.display_name, "Zoë 山田");
        let encoded =
            card("Other", "other@example.test").replace("FN:", "FN;ENCODING=QUOTED-PRINTABLE:");
        assert!(
            store
                .import_vcards(snapshot.revision, encoded.as_bytes())
                .is_err()
        );
        assert_eq!(store.load().unwrap().contacts, snapshot.contacts);
    }

    #[test]
    fn oversized_store_is_not_loaded_or_overwritten() {
        let f = Fixture::new();
        let store = f.store(ContactBackend::Direct, 1);
        store
            .apply(0, ContactChange::Create(draft("A", "a@example.test")))
            .unwrap();
        fs::write(&store.file, vec![b'x'; MAX_STORE_BYTES + 1]).unwrap();
        assert!(store.load().is_err());
        assert!(store.apply(0, ContactChange::Delete { id: 1 }).is_err());
        assert_eq!(
            fs::metadata(&store.file).unwrap().len(),
            (MAX_STORE_BYTES + 1) as u64
        );
    }

    #[test]
    fn private_modes_and_symlink_rejection_use_real_files() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let f = Fixture::new();
        let store = f.store(ContactBackend::Direct, 1);
        store
            .apply(0, ContactChange::Create(draft("A", "a@example.test")))
            .unwrap();
        assert_eq!(
            fs::metadata(&store.file).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let other = store.directory.join("other.json");
        fs::rename(&store.file, &other).unwrap();
        symlink(&other, &store.file).unwrap();
        assert!(store.load().is_err());
        fs::set_permissions(&f.0, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(LocalContacts::open(&f.0, store.scope).is_err());
    }
}

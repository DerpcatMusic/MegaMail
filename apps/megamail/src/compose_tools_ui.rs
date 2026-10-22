//! Reachable composer tools over the existing local stores and full outgoing seam.
//! One app-wide in-flight job bounds blocking-executor reads, CAS writes and rendering.
//! No clipboard, provider operation, automatic body edit, or incoming-text harvesting.
use crate::{compose_productivity as text, compose_session, contacts, *};
use gpui_kit::base::{Disableable as _, Selectable as _};
use gpui_kit::component::{Sizable as _, button::ButtonVariants as _};
use gpui_kit::{
    AppContext as _, IntoElement as _, ParentElement as _, Styled as _, prelude::FluentBuilder as _,
};
use std::collections::BTreeMap;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Recipient {
    To,
    Cc,
    Bcc,
}
impl Recipient {
    fn label(self) -> &'static str {
        match self {
            Self::To => "To",
            Self::Cc => "Cc",
            Self::Bcc => "Bcc",
        }
    }
}

/// Retained inputs and immutable worker-produced data; never serialized with mail.
pub(crate) struct Tools {
    open: bool,
    /// Contacts & snippets panel is expanded; `open` only means a read was requested.
    pub(crate) show: bool,
    pub(crate) autoloaded: bool,
    revision: u64,
    query: Entity<InputState>,
    name: Entity<InputState>,
    email: Entity<InputState>,
    notes: Entity<TextareaState>,
    editing_contact: Option<u64>,
    confirm_delete: bool,
    contact_page: usize,
    text_id: Entity<InputState>,
    text_label: Entity<InputState>,
    text_subject: Entity<InputState>,
    text_body: Entity<TextareaState>,
    variables: Entity<TextareaState>,
    text_kind: text::TextKind,
    confirm_text_delete: Option<String>,
    recipient: Recipient,
    snapshot: Option<Arc<Snapshot>>,
    suggestions: Vec<contacts::AddressSuggestion>,
    status: String,
}
impl Tools {
    pub(crate) fn book(&self) -> Option<Arc<contacts::ContactSnapshot>> {
        self.snapshot.as_ref()?.contacts.clone()
    }
    pub fn new(window: &mut Window, cx: &mut Context<MailApp>) -> Self {
        Self {
            open: false,
            show: false,
            autoloaded: false,
            revision: 0,
            query: new_input(window, cx, "Name or address prefix"),
            name: new_input(window, cx, "Contact display name"),
            email: new_input(window, cx, "Addresses, separated by commas"),
            notes: cx.new(|cx| TextareaState::new(window, cx)),
            editing_contact: None,
            confirm_delete: false,
            contact_page: 0,
            text_id: new_input(window, cx, "Short name"),
            text_label: new_input(window, cx, "Display name"),
            text_subject: new_input(window, cx, "Optional template subject"),
            text_body: cx.new(|cx| TextareaState::new(window, cx)),
            variables: cx.new(|cx| TextareaState::new(window, cx)),
            text_kind: text::TextKind::Snippet,
            confirm_text_delete: None,
            recipient: Recipient::To,
            snapshot: None,
            suggestions: Vec::new(),
            status: "Read this account's local contacts and literal text before choosing an item."
                .into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ScopeStamp {
    scope: contacts::ContactScope,
    address: String,
    // Actual worker-qualified epochs, not IMAP assumptions or a current-scope restamp.
    epochs: Vec<(String, live::NativeMailboxEpoch)>,
}
#[derive(Clone)]
struct Envelope {
    owner: Entity<TextareaState>, // The actual retained composer/editor session identity.
    stamp: ScopeStamp,
    revision: u64,
}
struct Snapshot {
    envelope: Envelope,
    contacts: Option<Arc<contacts::ContactSnapshot>>,
    contact_error: Option<String>,
    items: Vec<text::TextItem>,
    text_error: Option<String>,
    library: Option<Arc<text::TextLibrary>>,
}

fn read_contact_import(path: &std::path::Path) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC);
    }
    let file = options
        .open(path)
        .map_err(|_| "The selected contact file could not be opened.")?;
    if !file
        .metadata()
        .map_err(|_| "The contact file could not be inspected.")?
        .is_file()
    {
        return Err("Choose a regular vCard file.".into());
    }
    let mut bytes = Vec::new();
    file.take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "The contact file could not be read.")?;
    if bytes.len() > 1024 * 1024 {
        return Err("Contact imports are limited to 1 MiB.".into());
    }
    Ok(bytes)
}

fn storage_root() -> Result<PathBuf, String> {
    // This function is called ONLY inside background_spawn.
    config::config_base().ok_or_else(|| "MegaMail's private local storage is unavailable.".into())
}
fn read_snapshot(
    scope: contacts::ContactScope,
    signature: Option<Result<Option<text::TextItem>, text::ProductivityError>>,
) -> (
    Option<Arc<contacts::ContactSnapshot>>,
    Option<String>,
    Vec<text::TextItem>,
    Option<String>,
    Option<Arc<text::TextLibrary>>,
) {
    let root = match storage_root() {
        Ok(root) => root,
        Err(error) => return (None, Some(error.clone()), Vec::new(), Some(error), None),
    };
    let book = contacts::LocalContacts::open(&root, scope).and_then(|book| book.load());
    let (contacts, contact_error) = match book {
        Ok(snapshot) => (Some(Arc::new(snapshot)), None),
        Err(error) => (None, Some(error.to_string())),
    };
    let (library, mut text_error) = match text::PrivateLibraryStore::for_backend(
        &root,
        scope.backend == contacts::ContactBackend::Thunderbird,
    )
    .and_then(|store| store.load())
    {
        Ok(library) => (Some(Arc::new(library.unwrap_or_default())), None),
        Err(error) => (None, Some(format!("Text library rejected: {error:?}"))),
    };
    let mut items: Vec<_> = library
        .as_ref()
        .map(|library| library.for_account(scope.account_id).cloned().collect())
        .unwrap_or_default();
    match signature {
        Some(Ok(Some(signature))) => items.insert(0, signature),
        Some(Err(error)) => {
            let signature_error = "This account's signature cannot be used in plain text. Edit it in account settings.".to_owned();
            text_error = Some(text_error.map_or(signature_error.clone(), |previous| {
                format!("{previous}; {signature_error}")
            }));
        }
        _ => {}
    }
    (contacts, contact_error, items, text_error, library)
}

fn variable_values(input: &str) -> Result<BTreeMap<String, String>, String> {
    if input.len() > 64 * 1024 {
        return Err("Template values are limited to 64 KiB.".into());
    }
    let mut values = BTreeMap::new();
    for line in input.lines().filter(|line| !line.trim().is_empty()) {
        let (key, value) = line.split_once('=').ok_or("Use one name=value per line.")?;
        let key = key.trim();
        if key.is_empty()
            || key.len() > 64
            || !key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
            || values.len() >= 64
            || values.insert(key.to_owned(), value.to_owned()).is_some()
        {
            return Err(
                "Template names must be unique letters, digits or underscores; at most 64 values."
                    .into(),
            );
        }
    }
    Ok(values)
}

/// Returns only a checked delta. Subject replacement is deliberately not offered.
fn literal_edit(
    message: &OutgoingMessage,
    item: &text::TextItem,
    cursor: usize,
    values: &BTreeMap<String, String>,
) -> Result<(OutgoingMessage, String), String> {
    let rendered = text::render_item(item, message.from_account_id, values)
        .map_err(|error| format!("Literal text rejected: {error:?}"))?;
    if !rendered.missing.is_empty() {
        return Err(format!(
            "Fill template values for: {}. Nothing was inserted.",
            rendered.missing.join(", ")
        ));
    }
    let mut edited = message.clone();
    text::insert_item(&mut edited, item, values, cursor, false)
        .map_err(|error| format!("Literal insertion rejected: {error:?}"))?;
    let bytes = edited.body.len() - message.body.len();
    let delta = edited.body[cursor..cursor + bytes].to_owned();
    Ok((edited, delta))
}

fn editor_unchanged(
    before: &OutgoingMessage,
    current: &OutgoingMessage,
    cursor: usize,
    current_cursor: usize,
    selection: &std::ops::Range<usize>,
    current_selection: &std::ops::Range<usize>,
) -> bool {
    compose_session::same(before, current)
        && cursor == current_cursor
        && selection == current_selection
}

/// Append a validated suggestion without rewriting existing recipient spelling.
/// Use the real worker's formatter/parser and deduplicate across To/Cc/Bcc.
fn recipient_edit(
    message: &OutgoingMessage,
    scope: contacts::ContactScope,
    suggestion: &contacts::AddressSuggestion,
    target: Recipient,
) -> Result<(OutgoingMessage, String), String> {
    if scope != suggestion.scope || scope.account_id != message.from_account_id {
        return Err("The contact belongs to another account.".into());
    }
    if suggestion.display_name.len() > 256 || suggestion.display_name.chars().any(char::is_control)
    {
        return Err("The contact name contains an unsafe header character.".into());
    }
    let address = contacts::normalize_address(&suggestion.email).map_err(|e| e.to_string())?;
    for field in [&message.to, &message.cc, &message.bcc] {
        if field.len() > 16 * 1024 || field.chars().any(char::is_control) {
            return Err("A recipient field is too long or contains a control character. Edit it before adding a contact.".into());
        }
        if worker::parse_recipients(field)
            .iter()
            .any(|(_, existing)| existing.eq_ignore_ascii_case(&address))
        {
            return Err("This address is already in To, Cc or Bcc; nothing was added.".into());
        }
    }
    let mut edited = message.clone();
    let field = match target {
        Recipient::To => &mut edited.to,
        Recipient::Cc => &mut edited.cc,
        Recipient::Bcc => &mut edited.bcc,
    };
    let formatted = worker::format_recipient(&suggestion.display_name, &address);
    let parsed = worker::parse_recipients(&formatted);
    if parsed.len() != 1
        || parsed[0].1 != address
        || worker::invalid_recipient(&formatted).is_some()
    {
        return Err("This contact cannot be formatted as one safe recipient.".into());
    }
    let delta = format!(
        "{}{formatted}",
        if field.trim().is_empty() { "" } else { ", " }
    );
    if field.len().saturating_add(delta.len()) > 16 * 1024 {
        return Err("The recipient field limit would be exceeded.".into());
    }
    field.push_str(&delta);
    Ok((edited, delta))
}

impl MailApp {
    fn tools_stamp(&self, cx: &App) -> Option<ScopeStamp> {
        let composer = self.composer.as_ref()?;
        let id = composer.message.from_account_id;
        let live = self.live.read(cx);
        if self.demo || !live.can_use_account(id) {
            return None;
        }
        let snapshot = live.snapshot();
        let account = snapshot.accounts.iter().find(|a| a.id == id)?;
        let mut epochs: Vec<_> = snapshot
            .mailbox_epochs
            .iter()
            .filter(|((account, _), _)| *account == id)
            .take(129)
            .map(|((_, path), epoch)| (path.clone(), *epoch))
            .collect();
        if epochs.len() > 128 {
            return None;
        }
        epochs.sort_by(|a, b| a.0.cmp(&b.0));
        Some(ScopeStamp {
            scope: contacts::ContactScope {
                backend: if live.is_direct_account(id) {
                    contacts::ContactBackend::Direct
                } else {
                    contacts::ContactBackend::Thunderbird
                },
                account_id: id,
            },
            address: account.email.clone(),
            epochs,
        })
    }
    fn tools_admit(&self, envelope: &Envelope, cx: &App) -> bool {
        self.view == View::Compose
            && self.composer.as_ref().is_some_and(|composer| {
                composer.can_edit()
                    && composer.body.entity_id() == envelope.owner.entity_id()
                    && composer.tools.revision == envelope.revision
            })
            && self.tools_stamp(cx).as_ref() == Some(&envelope.stamp)
    }
    fn tools_request(&mut self, cx: &App) -> Option<Envelope> {
        if self.compose_tools_busy || self.view != View::Compose {
            return None;
        }
        let stamp = self.tools_stamp(cx)?;
        let composer = self.composer.as_mut()?;
        if !composer.can_edit() {
            return None;
        }
        let Some(revision) = composer.tools.revision.checked_add(1) else {
            composer.tools.status =
                "Local tools revision exhausted; close and reopen this composer.".into();
            return None;
        };
        composer.tools.revision = revision;
        composer.tools.suggestions.clear();
        composer.tools.status = "Reading local tools…".into();
        composer.tools.open = true;
        self.compose_tools_busy = true;
        Some(Envelope {
            owner: composer.body.clone(),
            stamp,
            revision,
        })
    }
    pub(crate) fn read_compose_tools(&mut self, cx: &mut Context<Self>) {
        let Some(envelope) = self.tools_request(cx) else {
            return;
        };
        let scope = envelope.stamp.scope;
        let account = self
            .profiles
            .iter()
            .find(|(id, _)| *id == scope.account_id)
            .filter(|_| scope.backend == contacts::ContactBackend::Direct)
            .map(|(_, account)| account.clone());
        let task = cx.background_spawn(async move {
            let signature = account
                .as_ref()
                .map(|account| text::configured_signature(account, scope.account_id));
            read_snapshot(scope, signature)
        });
        cx.spawn(async move |this, cx| {
            let (contacts, contact_error, items, text_error, library) = task.await;
            let _ = this.update(cx, |this, cx| {
                this.compose_tools_busy = false;
                if !this.tools_admit(&envelope, cx) { cx.notify(); return; }
                if let Some(composer) = &mut this.composer {
                    composer.tools.status = "Local tools read. Choose explicitly; loading never edits recipients or body.".into();
                    composer.tools.snapshot = Some(Arc::new(Snapshot { envelope, contacts, contact_error, items, text_error, library }));
                }
                cx.notify();
            });
        }).detach();
        cx.notify();
    }
    fn add_local_contact(&mut self, cx: &mut Context<Self>) {
        let Some(snapshot) = self
            .composer
            .as_ref()
            .and_then(|c| c.tools.snapshot.clone())
        else {
            return;
        };
        if !self.tools_admit(&snapshot.envelope, cx) {
            return;
        }
        if snapshot.contacts.is_none() {
            return;
        }
        let Some(composer) = self.composer.as_ref() else {
            return;
        };
        let fields = contacts::ContactDraft {
            display_name: composer.tools.name.read(cx).value().to_string(),
            emails: composer
                .tools
                .email
                .read(cx)
                .value()
                .split(',')
                .map(|s| s.trim().to_owned())
                .collect(),
            notes: composer.tools.notes.read(cx).value().to_string(),
        };
        let change = match composer.tools.editing_contact {
            Some(id) => contacts::ContactChange::Update { id, fields },
            None => contacts::ContactChange::Create(fields),
        };
        self.apply_local_contact(change, cx);
    }
    fn edit_local_contact(&mut self, id: Option<u64>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(snapshot) = self
            .composer
            .as_ref()
            .and_then(|c| c.tools.snapshot.clone())
        else {
            return;
        };
        if self.compose_tools_busy || !self.tools_admit(&snapshot.envelope, cx) {
            return;
        }
        let fields = match id {
            Some(id) => match snapshot
                .contacts
                .as_ref()
                .and_then(|b| b.contacts.iter().find(|c| c.id == id))
            {
                Some(c) => c.fields.clone(),
                None => return,
            },
            None => contacts::ContactDraft {
                display_name: String::new(),
                emails: vec![],
                notes: String::new(),
            },
        };
        let composer = self.composer.as_mut().unwrap();
        composer.tools.editing_contact = id;
        composer.tools.confirm_delete = false;
        composer.tools.name.update(cx, |input, cx| {
            input.set_value(fields.display_name, window, cx)
        });
        composer.tools.email.update(cx, |input, cx| {
            input.set_value(fields.emails.join(", "), window, cx)
        });
        composer
            .tools
            .notes
            .update(cx, |input, cx| input.set_value(fields.notes, window, cx));
        cx.notify();
    }
    fn delete_local_contact(&mut self, cx: &mut Context<Self>) {
        let Some(composer) = &mut self.composer else {
            return;
        };
        let Some(id) = composer.tools.editing_contact else {
            return;
        };
        if !composer.tools.confirm_delete {
            composer.tools.confirm_delete = true;
            cx.notify();
            return;
        }
        self.apply_local_contact(contacts::ContactChange::Delete { id }, cx);
    }
    fn apply_local_contact(&mut self, change: contacts::ContactChange, cx: &mut Context<Self>) {
        self.write_local_contacts(Some(change), None, cx);
    }
    fn export_local_contacts(&mut self, cx: &mut Context<Self>) {
        let Some(snapshot) = self
            .composer
            .as_ref()
            .and_then(|c| c.tools.snapshot.clone())
        else {
            return;
        };
        if self.compose_tools_busy || !self.tools_admit(&snapshot.envelope, cx) {
            return;
        }
        let Some(book) = snapshot.contacts.clone() else {
            return;
        };
        let directory = std::env::var_os("XDG_DOWNLOAD_DIR")
            .or_else(|| std::env::var_os("HOME"))
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        let picker = cx.prompt_for_new_path(&directory, Some("megamail-contacts.vcf"));
        self.compose_tools_busy = true;
        cx.spawn(async move |this, cx| {
            let chosen = picker.await;
            let task = this.update(cx, |this, cx| {
                this.compose_tools_busy = false;
                if !this.tools_admit(&snapshot.envelope, cx) {
                    cx.notify();
                    return None;
                }
                let path = match chosen {
                    Ok(Ok(Some(path))) => path,
                    Ok(Err(_)) => {
                        if let Some(c) = &mut this.composer {
                            c.tools.status = "Could not open the contact export dialog.".into();
                        }
                        cx.notify();
                        return None;
                    }
                    _ => {
                        cx.notify();
                        return None;
                    }
                };
                this.compose_tools_busy = true;
                cx.notify();
                Some(cx.background_spawn(async move {
                    let bytes = book.vcard_bytes().map_err(|e| e.to_string())?;
                    desktop_actions::write_new_private_file(&path, &bytes)
                }))
            });
            let Ok(Some(task)) = task else {
                return;
            };
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.compose_tools_busy = false;
                if this.tools_admit(&snapshot.envelope, cx) {
                    if let Some(c) = &mut this.composer {
                        c.tools.status = match result {
                            Ok(()) => {
                                "Exported this account's local contacts to the selected vCard file."
                                    .into()
                            }
                            Err(error) => error,
                        };
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn import_local_contacts(&mut self, cx: &mut Context<Self>) {
        let Some(snapshot) = self
            .composer
            .as_ref()
            .and_then(|c| c.tools.snapshot.clone())
        else {
            return;
        };
        if self.compose_tools_busy || !self.tools_admit(&snapshot.envelope, cx) {
            return;
        }
        let picker = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Import local contacts from a vCard file".into()),
        });
        self.compose_tools_busy = true;
        cx.spawn(async move |this, cx| {
            let chosen = picker.await;
            let _ = this.update(cx, |this, cx| {
                this.compose_tools_busy = false;
                if !this.tools_admit(&snapshot.envelope, cx) {
                    cx.notify();
                    return;
                }
                match chosen {
                    Ok(Ok(Some(paths))) => {
                        if let Some(path) = paths.into_iter().next() {
                            this.write_local_contacts(None, Some(path), cx);
                        }
                    }
                    Ok(Err(_)) => {
                        if let Some(c) = &mut this.composer {
                            c.tools.status = "Could not open the contact file picker.".into();
                        }
                    }
                    _ => {}
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn write_local_contacts(
        &mut self,
        change: Option<contacts::ContactChange>,
        import: Option<PathBuf>,
        cx: &mut Context<Self>,
    ) {
        let Some(snapshot) = self
            .composer
            .as_ref()
            .and_then(|c| c.tools.snapshot.clone())
        else {
            return;
        };
        if !self.tools_admit(&snapshot.envelope, cx) {
            return;
        }
        let Some(book) = &snapshot.contacts else {
            return;
        };
        let expected = book.revision;
        let Some(envelope) = self.tools_request(cx) else {
            return;
        };
        let scope = envelope.stamp.scope;
        let task = cx.background_spawn(async move {
            let root = storage_root()?;
            let store = contacts::LocalContacts::open(&root, scope).map_err(|e| e.to_string())?;
            if let Some(path) = import {
                let bytes = read_contact_import(&path)?;
                let (book, report) = store
                    .import_vcards(expected, &bytes)
                    .map_err(|e| e.to_string())?;
                Ok((
                    Arc::new(book),
                    format!(
                        "Imported {} contacts; merged {}, unchanged {}. Recipients were kept.",
                        report.created, report.merged, report.duplicates
                    ),
                ))
            } else {
                let change = change.ok_or("No contact change was selected.")?;
                let book = store.apply(expected, change).map_err(|e| e.to_string())?;
                Ok::<_, String>((
                    Arc::new(book),
                    "Local contacts updated. Recipients were kept.".into(),
                ))
            }
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.compose_tools_busy = false;
                if !this.tools_admit(&envelope, cx) {
                    cx.notify();
                    return;
                }
                if let Some(composer) = &mut this.composer {
                    match result {
                        Ok((book, status)) => {
                            composer.tools.editing_contact = composer
                                .tools
                                .editing_contact
                                .filter(|id| book.contacts.iter().any(|c| c.id == *id));
                            composer.tools.snapshot = Some(Arc::new(Snapshot {
                                envelope,
                                contacts: Some(book),
                                contact_error: None,
                                items: snapshot.items.clone(),
                                text_error: snapshot.text_error.clone(),
                                library: snapshot.library.clone(),
                            }));
                            composer.tools.confirm_delete = false;
                            composer.tools.contact_page = 0;
                            composer.tools.suggestions.clear();
                            composer.tools.status = status;
                        }
                        Err(error) => {
                            // Unknown durability/conflicts require reload, not blind create retry.
                            composer.tools.snapshot = None;
                            composer.tools.status =
                                format!("{error}. Read local tools again before saving.");
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn edit_literal_item(
        &mut self,
        item: Option<text::TextItem>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(snapshot) = self
            .composer
            .as_ref()
            .and_then(|c| c.tools.snapshot.clone())
        else {
            return;
        };
        if self.compose_tools_busy || !self.tools_admit(&snapshot.envelope, cx) {
            return;
        }
        if let Some(item) = &item {
            if !snapshot.library.as_ref().is_some_and(|l| {
                l.for_account(snapshot.envelope.stamp.scope.account_id)
                    .any(|i| i == item)
            }) {
                return;
            }
        }
        let c = self.composer.as_mut().unwrap();
        c.tools.text_kind = item.as_ref().map_or(text::TextKind::Snippet, |i| i.kind);
        c.tools.confirm_text_delete = None;
        c.tools.text_id.update(cx, |input, cx| {
            input.set_value(item.as_ref().map_or("", |i| i.id.as_str()), window, cx)
        });
        c.tools.text_label.update(cx, |input, cx| {
            input.set_value(item.as_ref().map_or("", |i| i.label.as_str()), window, cx)
        });
        c.tools.text_subject.update(cx, |input, cx| {
            input.set_value(
                item.as_ref()
                    .and_then(|i| i.subject.as_deref())
                    .unwrap_or(""),
                window,
                cx,
            )
        });
        c.tools.text_body.update(cx, |input, cx| {
            input.set_value(item.map_or(String::new(), |i| i.body), window, cx)
        });
        cx.notify();
    }
    fn save_literal_item(&mut self, remove: bool, cx: &mut Context<Self>) {
        let Some(snapshot) = self
            .composer
            .as_ref()
            .and_then(|c| c.tools.snapshot.clone())
        else {
            return;
        };
        if !self.tools_admit(&snapshot.envelope, cx) {
            return;
        }
        let Some(library) = &snapshot.library else {
            return;
        };
        let expected = library.clone();
        let Some(c) = &mut self.composer else {
            return;
        };
        let id = c.tools.text_id.read(cx).value().to_string();
        if id == "account-signature" {
            c.tools.status =
                "This ID belongs to the account signature. Choose another ID for library text."
                    .into();
            cx.notify();
            return;
        }
        if remove && c.tools.confirm_text_delete.as_deref() != Some(id.as_str()) {
            c.tools.confirm_text_delete = Some(id.clone());
            cx.notify();
            return;
        }
        let scope = snapshot.envelope.stamp.scope;
        let item = (!remove).then(|| text::TextItem {
            id: id.clone(),
            account_id: scope.account_id,
            kind: c.tools.text_kind,
            label: c.tools.text_label.read(cx).value().to_string(),
            subject: (c.tools.text_kind == text::TextKind::Template
                && !c.tools.text_subject.read(cx).value().is_empty())
            .then(|| c.tools.text_subject.read(cx).value().to_string()),
            body: c.tools.text_body.read(cx).value().to_string(),
        });
        let Some(envelope) = self.tools_request(cx) else {
            return;
        };
        let task = cx.background_spawn(async move {
            let root = storage_root()?;
            text::PrivateLibraryStore::for_backend(
                &root,
                scope.backend == contacts::ContactBackend::Thunderbird,
            )
            .and_then(|store| {
                store.apply(
                    &expected.revision()?,
                    scope.account_id,
                    item,
                    remove.then_some(id.as_str()),
                )
            })
            .map(Arc::new)
            .map_err(|e| format!("Text library update failed: {e:?}. Reload before retrying."))
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.compose_tools_busy = false;
                if !this.tools_admit(&envelope, cx) {
                    cx.notify();
                    return;
                }
                if let Some(c) = &mut this.composer {
                    match result {
                        Ok(library) => {
                            let mut items: Vec<_> = snapshot
                                .items
                                .iter()
                                .filter(|i| i.id == "account-signature")
                                .cloned()
                                .collect();
                            items.extend(library.for_account(scope.account_id).cloned());
                            c.tools.snapshot = Some(Arc::new(Snapshot {
                                envelope,
                                contacts: snapshot.contacts.clone(),
                                contact_error: snapshot.contact_error.clone(),
                                items,
                                text_error: None,
                                library: Some(library),
                            }));
                            c.tools.confirm_text_delete = None;
                            c.tools.status = "Text library updated. Composition was kept.".into();
                        }
                        Err(error) => {
                            c.tools.snapshot = None;
                            c.tools.status = error;
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn find_local_contacts(&mut self, cx: &mut Context<Self>) {
        let Some(snapshot) = self
            .composer
            .as_ref()
            .and_then(|c| c.tools.snapshot.clone())
        else {
            return;
        };
        if self.compose_tools_busy || !self.tools_admit(&snapshot.envelope, cx) {
            return;
        }
        if let (Some(composer), Some(book)) = (&mut self.composer, &snapshot.contacts) {
            composer.tools.suggestions = book.complete(&composer.tools.query.read(cx).value(), 20);
            composer.tools.status = if composer.tools.suggestions.is_empty() {
                "No local prefix matches. Enter up to 256 characters, or save a local contact below.".into()
            } else {
                "Choose a result to append it without replacing typed recipient text (up to 20 results).".into()
            };
        }
        cx.notify();
    }
    fn choose_local_contact(
        &mut self,
        suggestion: contacts::AddressSuggestion,
        revision: u64,
        cx: &mut Context<Self>,
        window: &mut Window,
    ) {
        let Some(snapshot) = self
            .composer
            .as_ref()
            .and_then(|c| c.tools.snapshot.clone())
        else {
            return;
        };
        if self.compose_tools_busy
            || snapshot.contacts.as_ref().map(|b| b.revision) != Some(revision)
            || !self.tools_admit(&snapshot.envelope, cx)
        {
            return;
        }
        let Some(composer) = &self.composer else {
            return;
        };
        // Accept only a suggestion still present in this immutable scoped book.
        if !snapshot.contacts.as_ref().is_some_and(|book| {
            book.contacts.iter().any(|c| {
                c.id == suggestion.contact_id
                    && c.fields.display_name == suggestion.display_name
                    && c.fields.emails.contains(&suggestion.email)
            })
        }) {
            return;
        }
        let target = composer.tools.recipient;
        let input = match target {
            Recipient::To => composer.to.clone(),
            Recipient::Cc => composer.cc.clone(),
            Recipient::Bcc => composer.bcc.clone(),
        };
        let selection = input.read(cx).selected_range();
        let cursor = input.read(cx).cursor();
        // The pinned setter cannot restore a reversed selection. Refuse rather
        // than silently moving its active caret to the other end.
        if !selection.is_empty() && cursor != selection.end {
            if let Some(composer) = &mut self.composer {
                composer.tools.status = "Collapse the recipient selection to a caret before appending a contact; its direction cannot be restored by this editor API.".into();
            }
            cx.notify();
            return;
        }
        let before = composer.outgoing(cx);
        let Some(envelope) = self.tools_request(cx) else {
            return;
        };
        let task_before = before.clone();
        let scope = envelope.stamp.scope;
        let task =
            cx.background_spawn(
                async move { recipient_edit(&task_before, scope, &suggestion, target) },
            );
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.compose_tools_busy = false;
                if !this.tools_admit(&envelope, cx) {
                    cx.notify();
                    return;
                }
                let Some(composer) = &mut this.composer else {
                    return;
                };
                let unchanged = editor_unchanged(
                    &before,
                    &composer.outgoing(cx),
                    cursor,
                    input.read(cx).cursor(),
                    &selection,
                    &input.read(cx).selected_range(),
                );
                match result {
                    Ok((_edited, delta)) if unchanged => {
                        input.update(cx, |input, cx| {
                            let end = input.value().len();
                            input.set_selected_range(end..end, cx);
                            input.insert(delta, window, cx);
                            input.set_selected_range(selection, cx);
                        });
                        composer.recovery.dirty_input = true;
                        composer.draft_saved = None;
                        composer.tools.status = format!(
                            "Contact appended to {}. Review recipients before sending.",
                            target.label()
                        );
                    }
                    Ok(_) => composer.tools.status =
                        "Composition or recipient selection changed; nothing added. Choose again."
                            .into(),
                    Err(error) => composer.tools.status = error,
                }
                composer.tools.snapshot = Some(Arc::new(Snapshot {
                    envelope,
                    contacts: snapshot.contacts.clone(),
                    contact_error: snapshot.contact_error.clone(),
                    items: snapshot.items.clone(),
                    text_error: snapshot.text_error.clone(),
                    library: snapshot.library.clone(),
                }));
                this.capture_recovery(cx);
                cx.notify();
            });
        })
        .detach();
        let _ = window; // The asynchronous update_in uses the same owning window.
        cx.notify();
    }
    fn insert_literal_item(&mut self, item: text::TextItem, cx: &mut Context<Self>) {
        let Some(snapshot) = self
            .composer
            .as_ref()
            .and_then(|c| c.tools.snapshot.clone())
        else {
            return;
        };
        if !self.tools_admit(&snapshot.envelope, cx) || !snapshot.items.contains(&item) {
            return;
        }
        let Some(composer) = &self.composer else {
            return;
        };
        if composer.body.read(cx).value().len() > 2 * 1024 * 1024 {
            if let Some(composer) = &mut self.composer {
                composer.tools.status =
                    "Literal insertion supports bodies up to 2 MiB; nothing was changed.".into();
            }
            cx.notify();
            return;
        }
        let before = composer.outgoing(cx);
        let cursor = composer.body.read(cx).cursor();
        let selection = composer.body.read(cx).selected_range();
        let Some(envelope) = self.tools_request(cx) else {
            return;
        };
        let task_before = before.clone();
        let variables = self
            .composer
            .as_ref()
            .unwrap()
            .tools
            .variables
            .read(cx)
            .value()
            .to_string();
        let task = cx.background_spawn(async move {
            literal_edit(&task_before, &item, cursor, &variable_values(&variables)?)
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.compose_tools_busy = false;
                if !this.tools_admit(&envelope, cx) { cx.notify(); return; }
                let Some(composer) = &mut this.composer else { return; };
                // Do not lose a single typed byte, hidden metadata update, caret move,
                // attachment or security change while background formatting was pending.
                let unchanged = editor_unchanged(&before, &composer.outgoing(cx), cursor, composer.body.read(cx).cursor(), &selection, &composer.body.read(cx).selected_range());
                match result {
                    Ok((_edited, delta)) if unchanged => {
                        composer.body.update(cx, |body, cx| body.insert(delta, window, cx));
                        composer.recovery.dirty_input = true;
                        composer.draft_saved = None;
                        composer.tools.status = "Literal text inserted at the body caret; selected text and subject were kept.".into();
                    }
                    Ok(_) => composer.tools.status = "Composition or caret changed during formatting; nothing inserted. Read tools and choose again.".into(),
                    Err(error) => composer.tools.status = error,
                }
                // Keep the same immutable snapshot, but explicitly bind this UI request
                // revision only after admission of its original envelope. No scope restamp.
                composer.tools.snapshot = Some(Arc::new(Snapshot {
                    envelope, contacts: snapshot.contacts.clone(), contact_error: snapshot.contact_error.clone(),
                    items: snapshot.items.clone(), text_error: snapshot.text_error.clone(), library: snapshot.library.clone(),
                }));
                this.capture_recovery(cx);
                cx.notify();
            });
        }).detach();
        cx.notify();
    }
    fn apply_template_subject(&mut self, item: text::TextItem, cx: &mut Context<Self>) {
        let Some(snapshot) = self
            .composer
            .as_ref()
            .and_then(|c| c.tools.snapshot.clone())
        else {
            return;
        };
        if !self.tools_admit(&snapshot.envelope, cx)
            || !snapshot.items.iter().any(|i| i == &item)
            || item.kind != text::TextKind::Template
        {
            return;
        }
        let c = self.composer.as_ref().unwrap();
        let before = c.outgoing(cx);
        let variables = c.tools.variables.read(cx).value().to_string();
        let Some(envelope) = self.tools_request(cx) else {
            return;
        };
        let account = before.from_account_id;
        let task = cx.background_spawn(async move {
            let values = variable_values(&variables)?;
            let rendered = text::render_item(&item, account, &values)
                .map_err(|e| format!("Template rejected: {e:?}"))?;
            if !rendered.missing.is_empty() {
                return Err(format!(
                    "Fill template values for: {}",
                    rendered.missing.join(", ")
                ));
            }
            rendered
                .subject
                .ok_or_else(|| "This template has no subject.".into())
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.compose_tools_busy = false;
                if !this.tools_admit(&envelope, cx) {
                    cx.notify();
                    return;
                }
                if let Some(c) = &mut this.composer {
                    match result {
                        Ok(subject) if compose_session::same(&before, &c.outgoing(cx)) => {
                            c.subject
                                .update(cx, |input, cx| input.set_value(subject, window, cx));
                            c.recovery.dirty_input = true;
                            c.draft_saved = None;
                            c.tools.status =
                                "Template subject applied. Body and recipients were kept.".into();
                        }
                        Ok(_) => {
                            c.tools.status =
                                "Composition changed; subject was kept. Choose the template again."
                                    .into()
                        }
                        Err(error) => c.tools.status = error,
                    }
                    c.tools.snapshot = Some(Arc::new(Snapshot {
                        envelope,
                        contacts: snapshot.contacts.clone(),
                        contact_error: snapshot.contact_error.clone(),
                        items: snapshot.items.clone(),
                        text_error: snapshot.text_error.clone(),
                        library: snapshot.library.clone(),
                    }));
                }
                this.capture_recovery(cx);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub(crate) fn compose_tools_view(
        &self,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(composer) = &self.composer else {
            return v_flex().into_any_element();
        };
        let tools = &composer.tools;
        let usable = self.tools_stamp(cx).is_some();
        let enabled = composer.can_edit() && usable && !self.compose_tools_busy;
        let ready = tools
            .snapshot
            .as_ref()
            .is_some_and(|s| self.tools_admit(&s.envelope, cx));
        let contacts_ready = ready
            && tools
                .snapshot
                .as_ref()
                .is_some_and(|s| s.contacts.is_some());
        v_flex().w_full().gap_2()
            .child(Button::new("read-compose-tools").ghost().small()
                .icon(Icon::new(if tools.show { IconName::ChevronDown } else { IconName::ChevronRight }))
                .label(if self.compose_tools_busy { "Loading contacts…" } else if tools.show { "Hide contacts & snippets" } else { "Contacts & snippets" })
                .disabled(!enabled && !tools.show)
                .tooltip("Saved contacts, signatures and reusable text. Stored privately on this device; nothing is looked up online.")
                .on_click(cx.listener(|this, _, _, cx| {
                    let Some(composer) = &mut this.composer else { return };
                    composer.tools.show = !composer.tools.show;
                    if composer.tools.show && composer.tools.snapshot.is_none() {
                        this.read_compose_tools(cx);
                    }
                    cx.notify();
                })))
            .when(!usable, |el| el.child(div().text_size(px(11.)).text_color(palette.muted).child("Contacts and snippets need a connected account, so they are unavailable here.")))
            .when(tools.show, |el| {
                let mut panel = v_flex().w_full().gap_2()
                    .child(div().text_size(px(11.)).text_color(palette.muted).child(format!("Private local plaintext • account {} • {}", composer.message.from_account_id, self.tools_stamp(cx).map_or("unavailable".into(), |s| s.address))))
                    .child(div().text_size(px(11.)).text_color(palette.muted).child(tools.status.clone()))
                    .child(h_flex().gap_2().items_center()
                        .child(Input::new(&tools.query).aria_label("Local contact prefix").disabled(!enabled || !contacts_ready).flex_1().min_w_0())
                        .child(Button::new("find-local-contacts").ghost().small().label("Find contacts").disabled(!enabled || !contacts_ready)
                            .on_click(cx.listener(|this, _, _, cx| this.find_local_contacts(cx)))))
                    .child(h_flex().gap_2().children([Recipient::To, Recipient::Cc, Recipient::Bcc].into_iter().map(|target|
                        Button::new(SharedString::from(format!("contact-target-{}", target.label()))).ghost().small()
                            .label(format!("Append to {}", target.label())).selected(tools.recipient == target).disabled(!enabled)
                            .on_click(cx.listener(move |this, _, _, cx| { if let Some(c) = &mut this.composer { c.tools.recipient = target; } cx.notify(); })))))
                    .children(tools.suggestions.iter().enumerate().map(|(index, suggestion)| {
                        let revision = tools.snapshot.as_ref().and_then(|s| s.contacts.as_ref()).map_or(0, |b| b.revision);
                        let suggestion = suggestion.clone();
                        let contact_id = suggestion.contact_id;
                        // Button labels/children are native text, never HTML or Markdown.
                        h_flex().gap_2().child(Button::new(SharedString::from(format!("local-contact-{index}"))).ghost().small()
                            .label(format!("{} <{}>", suggestion.display_name, suggestion.email)).disabled(!enabled || !contacts_ready)
                            .on_click(cx.listener(move |this, _, window, cx| this.choose_local_contact(suggestion.clone(), revision, cx, window))))
                            .child(Button::new(SharedString::from(format!("edit-found-contact-{index}"))).ghost().small().label("Edit")
                                .disabled(!enabled || !contacts_ready)
                                .on_click(cx.listener(move |this,_,window,cx|this.edit_local_contact(Some(contact_id),window,cx))))
                    }))
                    .child(h_flex().gap_2()
                        .child(Input::new(&tools.name).aria_label("Local contact name").disabled(!enabled || !contacts_ready).flex_1().min_w_0())
                        .child(Input::new(&tools.email).aria_label("Local contact email addresses").disabled(!enabled || !contacts_ready).flex_1().min_w_0()))
                    .child(Button::new("create-local-contact").ghost().small().label(if tools.editing_contact.is_some() { "Save contact changes" } else { "Save new local contact" }).disabled(!enabled || !contacts_ready)
                        .on_click(cx.listener(|this, _, _, cx| this.add_local_contact(cx))))
                    .child(div().text_size(px(11.)).text_color(palette.muted).child("Literal items insert at the body caret, keeping selected text and subject. HTML alternatives and missing variables block insertion. Save snippets, templates and signatures below."));
                panel = panel.child(Textarea::new(&tools.notes).h(px(80.)).aria_label("Local contact notes").disabled(!enabled || !contacts_ready))
                    .child(h_flex().gap_2()
                        .child(Button::new("export-local-contacts").ghost().small().label("Export contacts")
                            .disabled(!enabled || !contacts_ready)
                            .tooltip("Export this account's local contacts as a private vCard file. Choose a new filename.")
                            .on_click(cx.listener(|this, _, _, cx| this.export_local_contacts(cx))))
                        .child(Button::new("import-local-contacts").ghost().small().label("Import contacts")
                            .disabled(!enabled || !contacts_ready)
                            .on_click(cx.listener(|this, _, _, cx| this.import_local_contacts(cx))))
                        .child(Button::new("new-local-contact").ghost().small().label("New contact").disabled(!enabled || !contacts_ready)
                            .on_click(cx.listener(|this, _, window, cx| this.edit_local_contact(None, window, cx))))
                        .child(Button::new("delete-local-contact").ghost().small()
                            .label(if tools.confirm_delete { "Confirm delete contact" } else { "Delete contact" })
                            .disabled(!enabled || !contacts_ready || tools.editing_contact.is_none())
                            .on_click(cx.listener(|this, _, _, cx| this.delete_local_contact(cx)))));
                if let Some(snapshot) = &tools.snapshot {
                    if let Some(book) = &snapshot.contacts {
                        let pages = book.contacts.len().max(1).div_ceil(20);
                        let page = tools.contact_page.min(pages - 1);
                        panel = panel.child(h_flex().gap_2().items_center()
                            .child(Button::new("previous-contact-page").ghost().small().label("Previous contacts").disabled(!enabled || page == 0)
                                .on_click(cx.listener(|this, _, _, cx| { if let Some(c) = &mut this.composer { c.tools.contact_page = c.tools.contact_page.saturating_sub(1); } cx.notify(); })))
                            .child(div().text_size(px(11.)).child(format!("{} contacts · page {} of {}", book.contacts.len(), page + 1, pages)))
                            .child(Button::new("next-contact-page").ghost().small().label("Next contacts").disabled(!enabled || page + 1 >= pages)
                                .on_click(cx.listener(move |this, _, _, cx| { if let Some(c) = &mut this.composer { c.tools.contact_page = (c.tools.contact_page + 1).min(pages - 1); } cx.notify(); }))));
                        for contact in book.contacts.iter().skip(page * 20).take(20) {
                            let id = contact.id;
                            panel = panel.child(Button::new(SharedString::from(format!("edit-contact-{id}"))).ghost().small()
                                .label(format!("Edit {} · {}", contact.fields.display_name, contact.fields.emails.join(", ")))
                                .disabled(!enabled || !contacts_ready)
                                .on_click(cx.listener(move |this, _, window, cx| this.edit_local_contact(Some(id), window, cx))));
                        }
                    }
                    if let Some(error) = &snapshot.contact_error { panel = panel.child(div().text_size(px(11.)).text_color(palette.muted).child(error.clone())); }
                    if let Some(error) = &snapshot.text_error { panel = panel.child(div().text_size(px(11.)).text_color(palette.muted).child(error.clone())); }
                    let library_ready = ready && snapshot.library.is_some();
                    panel = panel.child(h_flex().gap_2()
                        .child(Input::new(&tools.text_id).aria_label("Text item short name").disabled(!enabled || !library_ready).flex_1())
                        .child(Input::new(&tools.text_label).aria_label("Text library item name").disabled(!enabled || !library_ready).flex_1()))
                        .child(h_flex().gap_2().children([text::TextKind::Signature,text::TextKind::Snippet,text::TextKind::Template].into_iter().map(|kind|
                            Button::new(SharedString::from(format!("text-kind-{kind:?}"))).ghost().small().label(format!("{kind:?}"))
                                .selected(tools.text_kind == kind).disabled(!enabled || !library_ready)
                                .on_click(cx.listener(move |this,_,_,cx| { if let Some(c) = &mut this.composer { c.tools.text_kind = kind; } cx.notify(); })))))
                        .child(Input::new(&tools.text_subject).aria_label("Template subject").disabled(!enabled || !library_ready || tools.text_kind != text::TextKind::Template))
                        .child(Textarea::new(&tools.text_body).h(px(120.)).aria_label("Text library content").disabled(!enabled || !library_ready))
                        .child(h_flex().gap_2()
                            .child(Button::new("new-text-item").ghost().small().label("New text item").disabled(!enabled || !library_ready)
                                .on_click(cx.listener(|this,_,window,cx| this.edit_literal_item(None,window,cx))))
                            .child(Button::new("save-text-item").ghost().small().label("Save text item").disabled(!enabled || !library_ready)
                                .on_click(cx.listener(|this,_,_,cx| this.save_literal_item(false,cx))))
                            .child(Button::new("delete-text-item").ghost().small().label(if tools.confirm_text_delete.as_deref() == Some(tools.text_id.read(cx).value().as_ref()) { "Confirm delete text item" } else { "Delete text item" })
                                .disabled(!enabled || !library_ready)
                                .on_click(cx.listener(|this,_,_,cx| this.save_literal_item(true,cx)))));
                    if let Some(library) = &snapshot.library {
                        for item in library.for_account(composer.message.from_account_id) {
                            let item = item.clone();
                            panel = panel.child(Button::new(SharedString::from(format!("edit-text-{}",item.id))).ghost().small().label(format!("Edit text: {}",item.label))
                                .disabled(!enabled || !library_ready).on_click(cx.listener(move |this,_,window,cx| this.edit_literal_item(Some(item.clone()),window,cx))));
                        }
                    }
                    panel = panel.child(div().text_size(px(11.)).text_color(palette.muted).child("Template values: one name=value per line for {{name}} placeholders."))
                        .child(Textarea::new(&tools.variables).h(px(80.)).aria_label("Template values, one name equals value per line").disabled(!enabled || !ready));
                    if snapshot.items.is_empty() { panel = panel.child(div().text_size(px(11.)).text_color(palette.muted).child("No usable literal text items or configured plain signature were loaded.")); }
                    for (index, item) in snapshot.items.iter().enumerate() {
                        let item = item.clone();
                        let kind = match item.kind { text::TextKind::Signature => "signature", text::TextKind::Snippet => "snippet", text::TextKind::Template => "template" };
                        if item.kind == text::TextKind::Template && item.subject.is_some() {
                            let template = item.clone();
                            panel = panel.child(Button::new(SharedString::from(format!("template-subject-{index}"))).ghost().small()
                                .label(format!("Use subject from {}",item.label)).disabled(!enabled || !ready)
                                .on_click(cx.listener(move |this,_,_,cx| this.apply_template_subject(template.clone(),cx))));
                        }
                        panel = panel.child(Button::new(SharedString::from(format!("literal-item-{index}"))).ghost().small()
                            .label(format!("Insert {kind}: {}", item.label)).disabled(!enabled || !ready)
                            .on_click(cx.listener(move |this, _, _, cx| this.insert_literal_item(item.clone(), cx))));
                    }

                }
                el.child(panel)
            }).into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn scope() -> contacts::ContactScope {
        contacts::ContactScope {
            backend: contacts::ContactBackend::Direct,
            account_id: 7,
        }
    }
    fn outgoing() -> OutgoingMessage {
        let mut m = compose::new_message(7);
        m.from_alias = Some("Alias <alias@example.test>".into());
        m.to = "Existing <existing@example.test>".into();
        m.cc = "copy@example.test".into();
        m.bcc = "hidden@example.test".into();
        m.reply_to = "replies@example.test".into();
        m.subject = "Keep subject".into();
        m.body = "αمرحبا終".into();
        m.attachments = vec!["/fictional/staged.bin".into()];
        m.in_reply_to = "parent@example.test".into();
        m.references = "older@example.test parent@example.test".into();
        m.sign = true;
        m.encrypt = true;
        m.send_at = Some(123);
        m.calendar = Some(worker::CalendarPart {
            ics: "BEGIN:VCALENDAR\r\nEND:VCALENDAR\r\n".into(),
            method: "REPLY".into(),
        });
        m.draft_origin = Some(megamail_core::models::DraftOrigin {
            account_id: 7,
            folder_id: 8,
            path: "Drafts".into(),
            uid: 19,
        });
        m.outbox_origin = Some(55);
        m
    }
    fn item(kind: text::TextKind) -> text::TextItem {
        text::TextItem {
            id: "literal".into(),
            account_id: 7,
            kind,
            label: "Fictional".into(),
            subject: None,
            body: "literal {{missing}}".into(),
        }
    }
    fn suggestion() -> contacts::AddressSuggestion {
        contacts::AddressSuggestion {
            scope: scope(),
            contact_id: 1,
            display_name: "Doe, \"A\" <literal>".into(),
            email: "a@example.test".into(),
        }
    }
    #[test]
    fn contact_export_keeps_existing_files_and_writes_private_bytes() {
        let path = std::env::temp_dir().join(format!(
            "megamail-contact-export-{}-{}",
            std::process::id(),
            megamail_core::rng::token(12).unwrap()
        ));
        desktop_actions::write_new_private_file(&path, b"fictional vcard").unwrap();
        assert!(desktop_actions::write_new_private_file(&path, b"replacement").is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"fictional vcard");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o077,
                0
            );
        }
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn selected_contact_file_is_bounded_and_rejects_nonregular_sources() {
        let root = std::env::temp_dir().join(format!(
            "megamail-vcard-picker-{}-{}",
            std::process::id(),
            megamail_core::rng::token(12).unwrap()
        ));
        std::fs::create_dir(&root).unwrap();
        let path = root.join("contacts.vcf");
        std::fs::write(
            &path,
            b"BEGIN:VCARD\nVERSION:4.0\nFN:Fictional\nEMAIL:person@example.test\nEND:VCARD\n",
        )
        .unwrap();
        assert!(
            read_contact_import(&path)
                .unwrap()
                .starts_with(b"BEGIN:VCARD")
        );
        assert!(read_contact_import(&root).is_err());
        #[cfg(unix)]
        {
            let link = root.join("link.vcf");
            std::os::unix::fs::symlink(&path, &link).unwrap();
            assert!(read_contact_import(&link).is_err());
        }
        std::fs::write(&path, vec![b'x'; 1024 * 1024 + 1]).unwrap();
        assert!(read_contact_import(&path).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn template_values_are_bounded_unique_and_preserve_complete_composition() {
        assert!(variable_values("name=a\nname=b").is_err());
        assert!(variable_values("bad name=value").is_err());
        assert!(variable_values(&"x".repeat(65537)).is_err());
        let mut item = item(text::TextKind::Template);
        item.body = "Hello {{name}}".into();
        item.subject = None;
        let before = outgoing();
        let values = variable_values("name=世界").unwrap();
        let (edited, delta) = literal_edit(&before, &item, 0, &values).unwrap();
        assert_eq!(delta, "Hello 世界");
        let mut expected = before.clone();
        expected.body.insert_str(0, &delta);
        assert!(compose_session::same(&edited, &expected));
    }

    #[test]
    fn real_full_outgoing_survives_signature_snippet_template_and_dirty_capture() {
        for kind in [
            text::TextKind::Signature,
            text::TextKind::Snippet,
            text::TextKind::Template,
        ] {
            let mut item = item(kind);
            item.body = "Fictional literal".into();
            if kind == text::TextKind::Template {
                item.subject = Some("Not replacing the subject".into());
            }
            let before = outgoing();
            let cursor = "α".len();
            let (edited, delta) = literal_edit(&before, &item, cursor, &BTreeMap::new()).unwrap();
            let mut expected = before.clone();
            expected.body.insert_str(cursor, &delta);
            assert!(compose_session::same(&expected, &edited));
            let mut capture = compose_session::Capture::default();
            capture.observe(before).unwrap();
            assert!(capture.observe(edited).unwrap());
            assert_eq!(capture.generation, 2);
            assert!(compose_session::same(
                capture.last.as_ref().unwrap(),
                &expected
            ));
        }
    }
    #[test]
    fn real_recipient_formatter_preserves_hidden_payload_and_rejects_injection_duplicates_scope() {
        let before = outgoing();
        let s = suggestion();
        let (edited, delta) = recipient_edit(&before, scope(), &s, Recipient::To).unwrap();
        let mut expected = before.clone();
        expected.to.push_str(&delta);
        assert!(compose_session::same(&expected, &edited));
        assert_eq!(
            worker::parse_recipients(&edited.to).last().unwrap().1,
            s.email
        );
        assert!(recipient_edit(&edited, scope(), &s, Recipient::Bcc).is_err());
        let mut unsafe_s = s.clone();
        unsafe_s.display_name = "Name\r\nBcc: bad@example.test".into();
        assert!(recipient_edit(&before, scope(), &unsafe_s, Recipient::To).is_err());
        unsafe_s = s.clone();
        unsafe_s.scope.backend = contacts::ContactBackend::Thunderbird;
        assert!(recipient_edit(&before, scope(), &unsafe_s, Recipient::To).is_err());
    }
    #[test]
    fn real_literal_errors_never_modify_outgoing() {
        let mut m = outgoing();
        let before = m.clone();
        assert!(literal_edit(&m, &item(text::TextKind::Snippet), 0, &BTreeMap::new()).is_err());
        let mut i = item(text::TextKind::Snippet);
        i.body = "literal".into();
        assert!(literal_edit(&m, &i, 1, &BTreeMap::new()).is_err()); // Inside UTF-8 alpha.
        i.account_id = 8;
        assert!(literal_edit(&m, &i, 0, &BTreeMap::new()).is_err());
        assert!(compose_session::same(&m, &before));
        i.account_id = 7;
        m.html = "<p>Preserve</p>".into();
        let before = m.clone();
        assert!(literal_edit(&m, &i, 0, &BTreeMap::new()).is_err());
        assert!(compose_session::same(&m, &before));
    }
    #[test]
    fn production_edit_delivery_guard_rejects_typing_hidden_changes_and_cursor_races() {
        let before = outgoing();
        assert!(editor_unchanged(&before, &before, 2, 2, &(0..2), &(0..2)));
        let mut typed = before.clone();
        typed.body.push_str("Typed during background work");
        assert!(!editor_unchanged(&before, &typed, 2, 2, &(0..2), &(0..2)));
        let mut changed = before.clone();
        changed.bcc.push_str(", another@example.test");
        assert!(!editor_unchanged(&before, &changed, 2, 2, &(0..2), &(0..2)));
        changed = before.clone();
        changed.calendar.as_mut().unwrap().ics.push_str("CHANGED");
        assert!(!editor_unchanged(&before, &changed, 2, 2, &(0..2), &(0..2)));
        assert!(!editor_unchanged(&before, &before, 2, 0, &(0..2), &(0..2)));
        assert!(!editor_unchanged(&before, &before, 2, 2, &(0..2), &(2..2)));
    }
    #[test]
    fn worker_scope_stamp_distinguishes_same_address_backend_and_source_epoch() {
        let first = ScopeStamp {
            scope: scope(),
            address: "same@example.test".into(),
            epochs: vec![(
                "Inbox".into(),
                live::NativeMailboxEpoch {
                    uid_validity: 4,
                    generation: 8,
                },
            )],
        };
        let mut second = first.clone();
        second.scope.account_id = 8;
        assert_ne!(first, second);
        second = first.clone();
        second.scope.backend = contacts::ContactBackend::Thunderbird;
        assert_ne!(first, second);
        second = first.clone();
        second.epochs[0].1.generation += 1;
        assert_ne!(first, second);
    }
    #[test]
    fn actual_restored_attachment_lease_and_origins_survive_literal_dirty_capture() {
        use std::os::unix::fs::PermissionsExt;
        let root = std::env::temp_dir().join(format!(
            "megamail-compose-tools-lease-fictional-{}-{}",
            std::process::id(),
            megamail_core::rng::token(12).unwrap()
        ));
        std::fs::create_dir(&root).unwrap();
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        let source = root.join("fictional.bin");
        std::fs::write(&source, b"fictional\0attachment").unwrap();
        std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o600)).unwrap();
        let mut store = draft_recovery::RecoveryStore::open(&root).unwrap();
        let key =
            draft_recovery::DraftKey::from_parts(7, "abcdefghijklmnopqrstuvwx".into()).unwrap();
        let mut message = outgoing();
        message.attachments = vec![source.to_str().unwrap().into()];
        store
            .save(&draft_recovery::DraftUpdate {
                key: key.clone(),
                generation: 1,
                message,
                source_address: "from@example.test".into(),
                retention: draft_recovery::Retention::Ordinary,
                attachment_lease: None,
            })
            .unwrap();
        let restored = store.restore(&key, false).unwrap().unwrap();
        let path = restored.message.attachments[0].clone();
        let mut capture = compose_session::Capture {
            key: Some(key),
            lease: Some(restored.attachment_lease),
            restored_hold: true,
            retention: draft_recovery::Retention::Ordinary,
            ..Default::default()
        };
        let before = restored.message;
        capture.observe(before.clone()).unwrap();
        let mut i = item(text::TextKind::Snippet);
        i.body = "Fictional insert".into();
        let (edited, delta) = literal_edit(&before, &i, 2, &BTreeMap::new()).unwrap();
        let mut expected = before.clone();
        expected.body.insert_str(2, &delta);
        assert!(compose_session::same(&edited, &expected));
        capture.observe(edited).unwrap();
        assert!(capture.lease.is_some());
        assert!(capture.restored_hold);
        assert_eq!(capture.retention, draft_recovery::Retention::Ordinary);
        assert_eq!(capture.retired, None);
        assert!(capture.blocks_provider_origin(capture.last.as_ref().unwrap()));
        assert_eq!(std::fs::read(&path).unwrap(), b"fictional\0attachment");
        assert!(compose_session::same(
            capture.last.as_ref().unwrap(),
            &expected
        ));
        drop(capture);
        drop(store);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn fictional_local_contacts_cas_does_not_overwrite_a_newer_revision() {
        use std::os::unix::fs::PermissionsExt;
        let root = std::env::temp_dir().join(format!(
            "megamail-compose-tools-fictional-{}-{}",
            std::process::id(),
            megamail_core::rng::token(12).unwrap()
        ));
        std::fs::create_dir(&root).unwrap();
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        let book = contacts::LocalContacts::open(&root, scope()).unwrap();
        let first = book.load().unwrap();
        let draft = contacts::ContactDraft {
            display_name: "Fictional".into(),
            emails: vec!["fiction@example.test".into()],
            notes: String::new(),
        };
        let saved = book
            .apply(
                first.revision,
                contacts::ContactChange::Create(draft.clone()),
            )
            .unwrap();
        assert!(matches!(
            book.apply(first.revision, contacts::ContactChange::Create(draft)),
            Err(contacts::ContactError::Conflict)
        ));
        assert_eq!(book.load().unwrap().revision, saved.revision);
        assert_eq!(book.load().unwrap().contacts.len(), 1);
        drop(book);
        std::fs::remove_dir_all(root).unwrap();
    }
}

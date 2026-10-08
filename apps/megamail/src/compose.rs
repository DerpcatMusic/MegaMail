//! Compose helpers shared by the native message reader and composer.

use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use megamail_core::{
    config::AccountConfig,
    models::{Attachment, DraftOrigin, Message},
    worker::{self, OutgoingMessage},
};

const DRAFT_ATTACHMENT_COUNT_LIMIT: usize = 20;
const DRAFT_ATTACHMENT_FILE_LIMIT: usize = 25 * 1024 * 1024;
const DRAFT_ATTACHMENT_TOTAL_LIMIT: usize = 50 * 1024 * 1024;
const DRAFT_RAW_LIMIT: usize = 100 * 1024 * 1024;

pub struct PreparedDraft {
    pub message: OutgoingMessage,
    pub temporary_attachment_paths: Vec<PathBuf>,
    pub notice: Option<String>,
}

/// Rebuild a draft from its original MIME bytes. The visible list row omits
/// Bcc and file contents, so this path preserves the full recipient set and
/// stages attachment bytes in a private cache directory before opening the
/// composer.
pub fn prepare_draft_from_raw(
    raw: &[u8],
    account: &AccountConfig,
    origin: DraftOrigin,
) -> Result<PreparedDraft, String> {
    prepare_draft_from_raw_impl(raw, account, origin, None)
}

fn prepare_draft_from_raw_impl(
    raw: &[u8],
    account: &AccountConfig,
    origin: DraftOrigin,
    supplied_staging_dir: Option<&Path>,
) -> Result<PreparedDraft, String> {
    if raw.is_empty() {
        return Err("The draft has no message data.".into());
    }
    if raw.len() > DRAFT_RAW_LIMIT {
        return Err("This draft is larger than MegaMail can safely open (100 MiB).".into());
    }
    let mut editable = worker::editable_from_raw(raw, &[]);
    let inline_image_count = editable.inline_images.len();
    editable.attachments.append(&mut editable.inline_images);
    validate_draft_attachment_sizes(editable.attachments.iter().map(|item| item.data.len()))?;
    let (attachment_paths, temporary_attachment_paths) = if editable.attachments.is_empty() {
        (Vec::new(), Vec::new())
    } else {
        let owned_staging_dir = if supplied_staging_dir.is_none() {
            Some(new_draft_attachment_dir()?)
        } else {
            None
        };
        let staging_dir = supplied_staging_dir
            .or(owned_staging_dir.as_deref())
            .expect("a staging directory was supplied or created");
        stage_draft_attachments(editable.attachments, staging_dir)?
    };

    let from_address = address_from_identity(&editable.from);
    let from_is_account =
        from_address.is_some_and(|address| address.eq_ignore_ascii_case(&account.email));
    let from_alias = from_address.and_then(|address| {
        account
            .aliases
            .iter()
            .find(|alias| alias.address().eq_ignore_ascii_case(address))
            .map(|alias| alias.identity.clone())
    });
    let unsupported_from =
        !editable.from.trim().is_empty() && !from_is_account && from_alias.is_none();

    let mut message = new_message(origin.account_id);
    message.from_alias = from_alias;
    message.to = editable.to;
    message.cc = editable.cc;
    message.bcc = editable.bcc;
    message.reply_to = editable.reply_to;
    message.subject = editable.subject;
    message.body = megamail_core::markdown::plain_text(&editable.body_html);
    message.attachments = attachment_paths;
    message.in_reply_to = editable.in_reply_to;
    message.references = editable.references;
    message.draft_origin = Some(origin);

    let mut notices = Vec::new();
    if unsupported_from {
        notices.push(format!(
            "The original From address is not configured as a send-as alias. This draft will use {}.",
            account.email
        ));
    }
    notices.push(if inline_image_count > 0 {
        "HTML formatting is simplified; inline images were preserved as attachments.".to_owned()
    } else {
        "This composer keeps the message as plain text; HTML formatting is simplified.".to_owned()
    });
    let notice = Some(notices.join(" "));

    Ok(PreparedDraft {
        message,
        temporary_attachment_paths,
        notice,
    })
}

#[cfg(test)]
fn prepare_draft_from_raw_in(
    raw: &[u8],
    account: &AccountConfig,
    origin: DraftOrigin,
    staging_dir: &Path,
) -> Result<PreparedDraft, String> {
    prepare_draft_from_raw_impl(raw, account, origin, Some(staging_dir))
}

pub fn remove_staged_attachments(paths: Vec<PathBuf>) {
    let mut directories = std::collections::HashSet::new();
    for path in paths {
        if let Some(parent) = path.parent() {
            directories.insert(parent.to_path_buf());
        }
        let _ = fs::remove_file(path);
    }
    for directory in directories {
        let _ = fs::remove_dir(directory);
    }
}

fn validate_draft_attachment_sizes(sizes: impl IntoIterator<Item = usize>) -> Result<(), String> {
    let mut count = 0usize;
    let mut total = 0usize;
    for size in sizes {
        count += 1;
        if count > DRAFT_ATTACHMENT_COUNT_LIMIT {
            return Err("This draft has more than 20 attachments, so MegaMail cannot open it without dropping files.".into());
        }
        if size > DRAFT_ATTACHMENT_FILE_LIMIT {
            return Err("This draft has an attachment larger than 25 MiB, so MegaMail cannot open it without dropping files.".into());
        }
        total = total
            .checked_add(size)
            .ok_or_else(|| "This draft's attachments are too large to open safely.".to_owned())?;
        if total > DRAFT_ATTACHMENT_TOTAL_LIMIT {
            return Err("This draft has more than 50 MiB of attachments, so MegaMail cannot open it without dropping files.".into());
        }
    }
    Ok(())
}

fn new_draft_attachment_dir() -> Result<PathBuf, String> {
    let cache = megamail_core::config::cache_base()
        .ok_or_else(|| "The private cache directory is unavailable.".to_owned())?;
    let root = cache.join("draft-attachments");
    ensure_private_directory(&root)?;
    let token = megamail_core::rng::token(24)
        .map_err(|error| format!("Could not create private draft storage: {error}"))?;
    let staging_dir = root.join(token);
    fs::create_dir(&staging_dir)
        .map_err(|error| format!("Could not create private draft storage: {error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Err(error) = fs::set_permissions(&staging_dir, fs::Permissions::from_mode(0o700)) {
            let _ = fs::remove_dir(&staging_dir);
            return Err(format!("Could not secure draft storage: {error}"));
        }
    }
    Ok(staging_dir)
}

/// Remove a small batch of leftover attachment directories from an earlier
/// MegaMail process. Startup holds the single-instance lock before calling
/// this, so no live composer can own these files.
pub fn prune_stale_draft_attachment_dirs() -> usize {
    let Some(cache) = megamail_core::config::cache_base() else {
        return 0;
    };
    prune_generated_draft_dirs(&cache.join("draft-attachments"), 64)
}

fn prune_generated_draft_dirs(root: &Path, limit: usize) -> usize {
    if limit == 0 {
        return 0;
    }
    let Ok(root_metadata) = fs::symlink_metadata(root) else {
        return 0;
    };
    if root_metadata.file_type().is_symlink() || !root_metadata.is_dir() {
        return 0;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if root_metadata.mode() & 0o077 != 0 {
            return 0;
        }
    }
    let Ok(entries) = fs::read_dir(root) else {
        return 0;
    };

    let mut removed = 0;
    for entry in entries.take(limit).flatten() {
        let name = entry.file_name();
        if !is_generated_draft_dir_name(&name.to_string_lossy()) {
            continue;
        }
        let path = entry.path();
        let Ok(metadata) = fs::symlink_metadata(&path) else {
            continue;
        };
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            continue;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if metadata.uid() != root_metadata.uid() || metadata.mode() & 0o077 != 0 {
                continue;
            }
        }
        if fs::remove_dir_all(path).is_ok() {
            removed += 1;
        }
    }
    removed
}

fn is_generated_draft_dir_name(name: &str) -> bool {
    name.len() == 24
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-._~".contains(&byte))
}

fn ensure_private_directory(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err("The private draft attachment directory is not a safe directory.".into());
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir(path).map_err(|error| {
                format!("Could not create a private draft attachment directory: {error}")
            })?;
        }
        Err(error) => {
            return Err(format!(
                "Could not inspect the draft attachment directory: {error}"
            ));
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .map_err(|error| format!("Could not secure the draft attachment directory: {error}"))?;
    }
    Ok(())
}

fn stage_draft_attachments(
    attachments: Vec<Attachment>,
    staging_dir: &Path,
) -> Result<(Vec<String>, Vec<PathBuf>), String> {
    let mut staged_paths = Vec::with_capacity(attachments.len());
    let mut temporary_paths = Vec::with_capacity(attachments.len());
    for attachment in attachments {
        let original_name = safe_attachment_filename(&attachment.name);
        let (file_name, path) = unique_attachment_path(staging_dir, &original_name);
        let write_result = (|| {
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options.open(&path)?;
            file.write_all(&attachment.data)?;
            Ok::<(), std::io::Error>(())
        })();
        if let Err(error) = write_result {
            let _ = fs::remove_file(&path);
            remove_staged_attachments(temporary_paths);
            let _ = fs::remove_dir(staging_dir);
            return Err(format!(
                "Could not stage {} from this draft: {error}",
                file_name
            ));
        }
        staged_paths.push(path.to_string_lossy().into_owned());
        temporary_paths.push(path);
    }
    Ok((staged_paths, temporary_paths))
}

fn unique_attachment_path(directory: &Path, original_name: &str) -> (String, PathBuf) {
    let path = directory.join(original_name);
    if !path.exists() {
        return (original_name.to_owned(), path);
    }
    let (stem, extension) = Path::new(original_name)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(|stem| {
            (
                stem.to_owned(),
                Path::new(original_name)
                    .extension()
                    .and_then(|extension| extension.to_str())
                    .map(|extension| format!(".{extension}"))
                    .unwrap_or_default(),
            )
        })
        .unwrap_or_else(|| (original_name.to_owned(), String::new()));
    for suffix in 2..=1000 {
        let candidate = format!("{stem} ({suffix}){extension}");
        let path = directory.join(&candidate);
        if !path.exists() {
            return (candidate, path);
        }
    }
    let fallback = format!(
        "{}-{}",
        megamail_core::rng::token(8).unwrap_or_default(),
        original_name
    );
    (fallback.clone(), directory.join(fallback))
}

fn safe_attachment_filename(name: &str) -> String {
    let cleaned = name
        .chars()
        .filter(|character| !character.is_control())
        .map(|character| {
            if matches!(character, '/' | '\\') {
                '_'
            } else {
                character
            }
        })
        .take(180)
        .collect::<String>();
    let cleaned = cleaned.trim();
    if cleaned.is_empty() || cleaned == "." || cleaned == ".." {
        "attachment".into()
    } else {
        cleaned.to_owned()
    }
}

fn address_from_identity(identity: &str) -> Option<&str> {
    let address = identity
        .rsplit_once('<')
        .map(|(_, address)| address.trim_end_matches('>').trim())
        .unwrap_or(identity.trim());
    (!address.is_empty() && address.contains('@')).then_some(address)
}

/// The first composer keeps email bodies as plain text. The worker can still
/// generate the MIME alternative and add reply threading headers.
pub fn new_message(account_id: u32) -> OutgoingMessage {
    OutgoingMessage {
        from_account_id: account_id,
        from_alias: None,
        to: String::new(),
        cc: String::new(),
        bcc: String::new(),
        reply_to: String::new(),
        subject: String::new(),
        body: String::new(),
        html: String::new(),
        attachments: Vec::new(),
        in_reply_to: String::new(),
        references: String::new(),
        draft_origin: None,
        outbox_origin: None,
        sign: false,
        encrypt: false,
        send_at: None,
        calendar: None,
    }
}

pub fn reply(account_id: u32, message: &Message) -> OutgoingMessage {
    let mut outgoing = new_message(account_id);
    outgoing.to = if message.reply_to.trim().is_empty() {
        message.from_addr.clone()
    } else {
        message.reply_to.clone()
    };
    outgoing.subject = prefixed_subject(&message.subject, "Re");
    outgoing.body = quoted_body(message);
    outgoing.in_reply_to = message.message_id.clone();
    outgoing.references = append_reference(&message.references, &message.message_id);
    outgoing
}

pub fn forward(account_id: u32, message: &Message) -> OutgoingMessage {
    let mut outgoing = new_message(account_id);
    outgoing.subject = prefixed_subject(&message.subject, "Fwd");
    outgoing.body = forwarded_body(message);
    outgoing
}

fn prefixed_subject(subject: &str, prefix: &str) -> String {
    let normalized = subject.trim_start();
    let already_prefixed = normalized
        .split_once(':')
        .is_some_and(|(existing, _)| existing.eq_ignore_ascii_case(prefix));
    if already_prefixed {
        subject.to_owned()
    } else if normalized.is_empty() {
        format!("{prefix}: ")
    } else {
        format!("{prefix}: {normalized}")
    }
}

fn append_reference(existing: &str, message_id: &str) -> String {
    let existing = existing.trim();
    let message_id = message_id.trim();
    match (existing.is_empty(), message_id.is_empty()) {
        (true, true) => String::new(),
        (false, true) => existing.to_owned(),
        (true, false) => message_id.to_owned(),
        (false, false) if existing.split_whitespace().last() == Some(message_id) => {
            existing.to_owned()
        }
        (false, false) => format!("{existing} {message_id}"),
    }
}

fn quoted_body(message: &Message) -> String {
    let attribution = if message.date.is_empty() {
        format!("{} wrote:", message.from_name)
    } else {
        format!("On {}, {} wrote:", message.date, message.from_name)
    };
    let quote = message
        .body
        .lines()
        .map(|line| format!("> {line}"))
        .collect::<Vec<_>>()
        .join("\n");
    format!("\n\n{attribution}\n{quote}")
}

fn forwarded_body(message: &Message) -> String {
    format!(
        "\n\n---------- Forwarded message ----------\nFrom: {} <{}>\nDate: {}\nSubject: {}\n\n{}",
        message.from_name, message.from_addr, message.date, message.subject, message.body
    )
}

#[cfg(test)]
mod tests {
    use super::{
        forward, new_message, prepare_draft_from_raw_in, reply, safe_attachment_filename,
        validate_draft_attachment_sizes,
    };
    use megamail_core::{
        config::{AccountConfig, AliasConfig},
        models::{DraftOrigin, Importance, Message},
    };
    use std::{fs, path::PathBuf};

    fn sample() -> Message {
        Message {
            id: 1,
            account_id: 7,
            folder_id: 1,
            uid: 42,
            from_name: "Ari Chen".into(),
            from_addr: "ari@example.test".into(),
            reply_to: "team@example.test".into(),
            to: "me@example.test".into(),
            cc: String::new(),
            subject: "Quarterly notes".into(),
            preview: String::new(),
            body: "First line\nSecond line".into(),
            date: "Oct 8, 2026".into(),
            timestamp: 0,
            unread: false,
            starred: false,
            keywords: Vec::new(),
            has_attachment: false,
            message_id: "message-42@example.test".into(),
            references: "parent@example.test".into(),
            importance: Importance::Normal,
            due: 0,
        }
    }

    #[test]
    fn new_message_is_empty_and_bound_to_the_selected_account() {
        let message = new_message(12);
        assert_eq!(message.from_account_id, 12);
        assert!(message.to.is_empty());
        assert!(message.subject.is_empty());
        assert!(message.body.is_empty());
        assert!(message.in_reply_to.is_empty());
    }

    #[test]
    fn reply_uses_reply_to_and_preserves_thread_headers() {
        let message = sample();
        let outgoing = reply(7, &message);
        assert_eq!(outgoing.to, "team@example.test");
        assert_eq!(outgoing.subject, "Re: Quarterly notes");
        assert_eq!(outgoing.in_reply_to, "message-42@example.test");
        assert_eq!(
            outgoing.references,
            "parent@example.test message-42@example.test"
        );
        assert!(outgoing.body.contains("> First line\n> Second line"));
    }

    #[test]
    fn forward_does_not_claim_to_be_in_the_original_thread() {
        let outgoing = forward(7, &sample());
        assert_eq!(outgoing.subject, "Fwd: Quarterly notes");
        assert!(outgoing.in_reply_to.is_empty());
        assert!(outgoing.references.is_empty());
        assert!(outgoing.body.contains("From: Ari Chen <ari@example.test>"));
    }

    #[test]
    fn draft_restore_keeps_bcc_thread_headers_and_binary_attachments() {
        let staging_dir = test_directory();
        let mut account = AccountConfig::new("me@example.test");
        account.aliases.push(AliasConfig {
            identity: "Work <work@example.test>".into(),
            ..Default::default()
        });
        let raw = b"From: \"Work Finance\" <WORK@example.test>\r\nTo: Ada <ada@example.test>\r\nCc: Bob <bob@example.test>\r\nBcc: Secret <secret@example.test>\r\nReply-To: replies@example.test\r\nSubject: Saved note\r\nMessage-ID: <draft-1@example.test>\r\nIn-Reply-To: <parent@example.test>\r\nReferences: <older@example.test> <parent@example.test>\r\nMIME-Version: 1.0\r\nContent-Type: multipart/mixed; boundary=\"mega-boundary\"\r\n\r\n--mega-boundary\r\nContent-Type: multipart/related; boundary=\"related-boundary\"\r\n\r\n--related-boundary\r\nContent-Type: text/html; charset=utf-8\r\n\r\n<p>Hello <b>Ada</b></p><img src=\"cid:logo\">\r\n--related-boundary\r\nContent-Type: image/png; name=\"logo.png\"\r\nContent-ID: <logo>\r\nContent-Disposition: inline; filename=\"logo.png\"\r\nContent-Transfer-Encoding: base64\r\n\r\niVBORw0KGgo=\r\n--related-boundary--\r\n--mega-boundary\r\nContent-Type: application/octet-stream\r\nContent-Disposition: attachment; filename=\"secret.bin\"\r\nContent-Transfer-Encoding: base64\r\n\r\nYmluYXJ5AA==\r\n--mega-boundary--\r\n";
        let origin = DraftOrigin {
            account_id: 9,
            folder_id: 4,
            path: "[Gmail]/Drafts".into(),
            uid: 77,
        };

        let prepared = prepare_draft_from_raw_in(raw, &account, origin.clone(), &staging_dir)
            .expect("MIME draft restores");
        assert_eq!(prepared.message.from_account_id, 9);
        assert_eq!(
            prepared.message.from_alias.as_deref(),
            Some("Work <work@example.test>")
        );
        assert_eq!(prepared.message.to, "Ada <ada@example.test>");
        assert_eq!(prepared.message.cc, "Bob <bob@example.test>");
        assert_eq!(prepared.message.bcc, "Secret <secret@example.test>");
        assert_eq!(prepared.message.reply_to, "replies@example.test");
        assert_eq!(prepared.message.subject, "Saved note");
        assert_eq!(prepared.message.in_reply_to, "parent@example.test");
        assert_eq!(
            prepared.message.references,
            "older@example.test parent@example.test"
        );
        assert!(prepared.message.body.contains("Hello Ada"));
        assert_eq!(prepared.message.draft_origin.as_ref().unwrap().uid, 77);
        assert_eq!(prepared.message.attachments.len(), 2);
        assert!(
            prepared
                .notice
                .as_deref()
                .unwrap()
                .contains("inline images were preserved as attachments")
        );
        assert_eq!(
            PathBuf::from(prepared.message.attachments[0].as_str())
                .file_name()
                .unwrap()
                .to_string_lossy(),
            "secret.bin"
        );
        assert_eq!(
            fs::read(&prepared.message.attachments[0]).unwrap(),
            b"binary\0"
        );
        assert_eq!(
            PathBuf::from(prepared.message.attachments[1].as_str())
                .file_name()
                .unwrap()
                .to_string_lossy(),
            "logo.png"
        );
        assert_eq!(
            fs::read(&prepared.message.attachments[1]).unwrap(),
            [137, 80, 78, 71, 13, 10, 26, 10]
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&prepared.temporary_attachment_paths[0])
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
        super::remove_staged_attachments(prepared.temporary_attachment_paths);
        let _ = fs::remove_dir_all(staging_dir);
    }

    #[test]
    fn draft_attachment_limits_fail_before_files_are_staged() {
        assert!(validate_draft_attachment_sizes(std::iter::repeat_n(1, 20)).is_ok());
        assert!(validate_draft_attachment_sizes(std::iter::repeat_n(1, 21)).is_err());
        assert!(validate_draft_attachment_sizes([25 * 1024 * 1024]).is_ok());
        assert!(validate_draft_attachment_sizes([25 * 1024 * 1024 + 1]).is_err());
        assert!(validate_draft_attachment_sizes([25 * 1024 * 1024, 25 * 1024 * 1024]).is_ok());
        assert!(validate_draft_attachment_sizes([25 * 1024 * 1024, 25 * 1024 * 1024 + 1]).is_err());
    }

    #[test]
    fn attachment_names_cannot_escape_the_private_staging_directory() {
        for name in ["../../etc/passwd", "..", "./", "secret\nname.bin"] {
            let safe = safe_attachment_filename(name);
            assert!(!safe.contains('/'));
            assert!(!safe.contains('\\'));
            assert!(!safe.chars().any(char::is_control));
            assert_ne!(safe, "..");
        }
    }

    #[test]
    fn startup_pruning_removes_only_generated_private_draft_directories() {
        let root = test_directory();
        set_private(&root);
        let stale = root.join(megamail_core::rng::token(24).unwrap());
        fs::create_dir(&stale).unwrap();
        set_private(&stale);
        fs::write(stale.join("secret.bin"), b"private").unwrap();

        let unrelated = root.join("keep-this-directory");
        fs::create_dir(&unrelated).unwrap();
        set_private(&unrelated);

        assert_eq!(super::prune_generated_draft_dirs(&root, 64), 1);
        assert!(!stale.exists());
        assert!(unrelated.is_dir());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn startup_pruning_is_bounded_to_the_requested_batch() {
        let root = test_directory();
        set_private(&root);
        for _ in 0..65 {
            let dir = root.join(megamail_core::rng::token(24).unwrap());
            fs::create_dir(&dir).unwrap();
            set_private(&dir);
        }

        assert_eq!(super::prune_generated_draft_dirs(&root, 64), 64);
        assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    fn set_private(path: &std::path::Path) {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }

    #[cfg(not(unix))]
    fn set_private(_: &std::path::Path) {}

    fn test_directory() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "megamail-draft-test-{}",
            megamail_core::rng::token(12).unwrap()
        ));
        fs::create_dir(&path).unwrap();
        path
    }
}

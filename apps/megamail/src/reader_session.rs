//! Immutable original source and one scoped temporary reader host.
use crate::live::{DirectBodyVersion, MessageKey};
use crate::reader_document::{CacheEligibility, Completeness, ReaderDocument, ReaderScope};
use crate::reader_host::{ReaderHost, ReaderHostEvent};
use megamail_core::models::SenderCheck;
use std::{path::PathBuf, sync::Arc};

#[derive(Clone)]
pub(crate) struct OriginalBody {
    pub source: Option<Arc<str>>,
    pub completeness: Completeness,
    pub eligibility: CacheEligibility,
}
impl std::fmt::Debug for OriginalBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Snapshots need Debug; that is not permission to log raw mail HTML.
        f.debug_struct("OriginalBody")
            .field("source_bytes", &self.retained_bytes())
            .field("completeness", &self.completeness)
            .field("eligibility", &self.eligibility)
            .finish_non_exhaustive()
    }
}
impl OriginalBody {
    /// Producer's decoded HTML BEFORE the native sanitizer. Direct workers do
    /// not attest selected MIME completeness (may concatenate/expand CID).
    pub fn capture(source: &str, complete: Option<bool>, check: Option<&SenderCheck>) -> Self {
        let html = source.contains('<') && source.contains('>');
        let completeness = if source.len() > 2 * 1024 * 1024 {
            Completeness::TooLarge
        } else if !html || source.trim().is_empty() {
            Completeness::Missing
        } else if complete == Some(true) {
            Completeness::Complete
        } else {
            Completeness::Truncated
        };
        let eligibility = match check {
            Some(check)
                if check
                    .pgp
                    .as_ref()
                    .is_some_and(|p| p.encrypted || p.decrypted) =>
            {
                CacheEligibility::Decrypted
            }
            Some(check) if check.pgp.is_some() => CacheEligibility::Signed,
            Some(_) => CacheEligibility::Allowed,
            None => CacheEligibility::Unknown,
        };
        Self {
            source: (html && source.len() <= 2 * 1024 * 1024).then(|| Arc::from(source)),
            completeness,
            eligibility,
        }
    }
    pub fn retained_bytes(&self) -> usize {
        self.source.as_ref().map_or(0, |s| s.len())
    }
    pub fn document(
        &self,
        scope: ReaderScope,
        semantic: Arc<str>,
        plain: Arc<str>,
        title: Arc<str>,
    ) -> Result<Arc<ReaderDocument>, String> {
        ReaderDocument::new(scope, self.source.clone(), semantic, plain, title, self.completeness, self.eligibility, Vec::new())
            .map(Arc::new).map_err(|_| "Complete original formatting is unavailable or exceeds the document budget. Native selectable text remains available.".to_owned())
    }
}

pub(crate) fn scope(
    key: &MessageKey,
    version: Option<DirectBodyVersion>,
    generation: u64,
) -> ReaderScope {
    ReaderScope {
        account_id: Arc::from(key.account_id.to_string()),
        folder_id: Arc::from(key.folder_path.as_str()),
        message_id: Arc::from(format!(
            "{}:{}",
            key.uid,
            version.map_or(0, |v| v.source_generation)
        )),
        generation,
    }
}

/// Correlates the selectable native consumer, not the external HTML helper.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SemanticTextIdentity {
    pub key: MessageKey,
    pub version: Option<DirectBodyVersion>,
    pub generation: u64,
    pub plain: bool,
    pub quote_visible: bool,
}

fn semantic_text_unchanged(
    previous: &SemanticTextIdentity,
    previous_html: &str,
    current: &SemanticTextIdentity,
    current_html: &str,
) -> bool {
    previous == current && previous_html == current_html
}

pub(crate) struct SemanticTextTarget {
    pub identity: SemanticTextIdentity,
    pub html: gpui_kit::SharedString,
    pub state: gpui_kit::Entity<gpui_kit::component::text::TextViewState>,
}
impl SemanticTextTarget {
    /// Called on body/mode transitions, never from render. The vendor state
    /// bounds synchronous parsing to 4 KiB and parses larger input off-thread.
    pub fn new(
        identity: SemanticTextIdentity,
        html: gpui_kit::SharedString,
        cx: &mut gpui_kit::App,
    ) -> Self {
        use gpui_kit::AppContext as _;
        let state =
            cx.new(|cx| gpui_kit::component::text::TextViewState::html(&html, cx).selectable(true));
        Self {
            identity,
            html,
            state,
        }
    }
    pub fn matches(&self, identity: &SemanticTextIdentity) -> bool {
        &self.identity == identity
    }
    pub fn unchanged(&self, identity: &SemanticTextIdentity, html: &str) -> bool {
        semantic_text_unchanged(&self.identity, &self.html, identity, html)
    }
}

/// No automatic release enablement. A developer can explicitly preview only
/// fictional fixtures with an absolute, privately trusted helper. Presence of
/// a binary/policy handshake is NOT security acceptance for real mail.
pub(crate) fn developer_helper() -> Result<PathBuf, String> {
    if std::env::var_os("MEGAMAIL_HTML_READER_DEVELOPER_PREVIEW").as_deref()
        != Some(std::ffi::OsStr::new("1"))
    {
        return Err("Original formatting is unavailable: helper security acceptance is pending. Native selectable formatting remains available.".into());
    }
    let path = std::env::var_os("MEGAMAIL_HTML_READER_HELPER")
        .map(PathBuf::from)
        .ok_or_else(|| {
            "Developer preview needs an explicit absolute trusted helper path.".to_owned()
        })?;
    if !path.is_absolute() || path.canonicalize().ok().as_ref() != Some(&path) {
        return Err("The helper path must be absolute and contain no symlink or traversal.".into());
    }
    let metadata =
        std::fs::symlink_metadata(&path).map_err(|_| "The helper is unavailable.".to_owned())?;
    #[cfg(unix)]
    for parent in path.ancestors().skip(1) {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        let parent = std::fs::symlink_metadata(parent)
            .map_err(|_| "The helper parent is unavailable.".to_owned())?;
        // SAFETY: geteuid has no arguments or memory preconditions.
        let uid = unsafe { libc::geteuid() };
        if !parent.is_dir()
            || parent.permissions().mode() & 0o022 != 0
            || (parent.uid() != 0 && parent.uid() != uid)
        {
            return Err("The helper and every parent must be privately trusted and non-writable by other users.".into());
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        if !metadata.is_file()
            || metadata.permissions().mode() & 0o022 != 0
            || (metadata.uid() != 0
                && metadata.uid() != {
                    // SAFETY: geteuid has no arguments or memory preconditions.
                    unsafe { libc::geteuid() }
                })
        {
            return Err("The helper must be a trusted, non-writable executable.".into());
        }
    }
    Ok(path)
}

pub(crate) fn selection_matches(
    expected: &MessageKey,
    version: Option<DirectBodyVersion>,
    generation: u64,
    current: Option<&MessageKey>,
    current_version: Option<DirectBodyVersion>,
    current_generation: u64,
) -> bool {
    current == Some(expected) && version == current_version && generation == current_generation
}

#[derive(Default)]
pub(crate) struct Session {
    pub generation: u64,
    pub exhausted: bool,
    pub active: Option<ReaderScope>,
    pub host: Option<ReaderHost>,
    pub notice: Option<String>,
}
impl Session {
    pub fn retire(&mut self) {
        if let Some(host) = self.host.take() {
            host.cancel();
        }
        self.active = None;
        match self.generation.checked_add(1) {
            Some(generation) => self.generation = generation,
            None => self.exhausted = true,
        }
    }
    pub fn accepts(&self, scope: &ReaderScope) -> bool {
        !self.exhausted && self.active.as_ref() == Some(scope)
    }
    pub fn poll(&mut self) -> Vec<String> {
        let mut links = Vec::new();
        for _ in 0..16 {
            let event = match self.host.as_ref().map(ReaderHost::try_recv) {
                Some(Ok(event)) => event,
                Some(Err(std::sync::mpsc::TryRecvError::Disconnected)) => {
                    self.notice =
                        Some("Original reader closed; native text remains available.".into());
                    self.retire();
                    break;
                }
                _ => break,
            };
            match event {
                ReaderHostEvent::Ready { scope } if self.accepts(&scope) => self.notice = Some("Helper reports ready — temporary developer window. Remote-resource/CID blocking is a policy claim; helper security acceptance remains pending.".into()),
                ReaderHostEvent::UserClickedLink { scope, url } if self.accepts(&scope) => {
                    if megamail_core::mail_text::safe_browser_url(&url) { links.push(url); }
                },
                ReaderHostEvent::Fallback { scope, .. } | ReaderHostEvent::Closed { scope } if self.accepts(&scope) => {
                    self.notice = Some("Original renderer unavailable or rejected its policy. Native selectable text remains available.".into()); self.retire(); break;
                },
                _ => {},
            }
        }
        links
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        if let Some(host) = &self.host {
            host.cancel();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn semantic_identity() -> SemanticTextIdentity {
        SemanticTextIdentity {
            key: MessageKey {
                account_id: 7,
                folder_path: "INBOX".into(),
                uid: 42,
            },
            version: Some(DirectBodyVersion {
                uid: 42,
                source_generation: 2,
            }),
            generation: 3,
            plain: false,
            quote_visible: false,
        }
    }
    #[test]
    fn semantic_target_retention_reuses_only_unchanged_body_and_mode() {
        let identity = semantic_identity();
        assert!(semantic_text_unchanged(
            &identity,
            "<p>same</p>",
            &identity,
            "<p>same</p>"
        ));
        assert!(!semantic_text_unchanged(
            &identity,
            "<p>old</p>",
            &identity,
            "<p>new</p>"
        ));
        for current in [
            SemanticTextIdentity {
                plain: true,
                ..identity.clone()
            },
            SemanticTextIdentity {
                quote_visible: true,
                ..identity.clone()
            },
        ] {
            assert!(!semantic_text_unchanged(
                &identity,
                "<p>same</p>",
                &current,
                "<p>same</p>"
            ));
        }
    }
    #[test]
    fn semantic_target_retention_rejects_uid_reuse_and_source_generation_changes() {
        let identity = semantic_identity();
        for current in [
            SemanticTextIdentity {
                generation: 4,
                ..identity.clone()
            },
            SemanticTextIdentity {
                version: Some(DirectBodyVersion {
                    uid: 42,
                    source_generation: 4,
                }),
                ..identity.clone()
            },
            SemanticTextIdentity {
                version: None,
                ..identity.clone()
            },
        ] {
            assert!(!semantic_text_unchanged(
                &identity,
                "<p>same</p>",
                &current,
                "<p>same</p>"
            ));
        }
    }
    #[test]
    fn semantic_target_retention_rejects_equal_uid_in_other_account_folder_or_message() {
        let identity = semantic_identity();
        for key in [
            MessageKey {
                account_id: 8,
                ..identity.key.clone()
            },
            MessageKey {
                folder_path: "Sent".into(),
                ..identity.key.clone()
            },
            MessageKey {
                uid: 43,
                ..identity.key.clone()
            },
        ] {
            let current = SemanticTextIdentity {
                key,
                ..identity.clone()
            };
            assert!(!semantic_text_unchanged(
                &identity,
                "<p>same</p>",
                &current,
                "<p>same</p>"
            ));
        }
    }
    #[test]
    fn original_is_not_reconstructed_from_semantic_html() {
        let original =
            OriginalBody::capture("<style>p{color:red}</style><p>Source</p>", Some(true), None);
        let doc = original
            .document(
                ReaderScope {
                    account_id: Arc::from("1"),
                    folder_id: Arc::from("INBOX"),
                    message_id: Arc::from("7"),
                    generation: 1,
                },
                Arc::from("<p>Source</p>"),
                Arc::from("Source"),
                Arc::from("fixture"),
            )
            .unwrap();
        assert!(doc.original_source().unwrap().contains("<style>"));
        assert!(!doc.persistent_cache_allowed());
    }
    #[test]
    fn unknown_completeness_and_oversize_fail_closed() {
        assert_eq!(
            OriginalBody::capture("<p>x</p>", None, None).completeness,
            Completeness::Truncated
        );
        assert!(
            OriginalBody::capture(&"<p>".repeat(800_000), Some(true), None)
                .source
                .is_none()
        );
    }
    #[test]
    fn actual_invitation_delivery_guard_rejects_account_body_and_epoch_switches() {
        let key = MessageKey {
            account_id: 7,
            folder_path: "INBOX".into(),
            uid: 42,
        };
        let version = Some(DirectBodyVersion {
            uid: 42,
            source_generation: 2,
        });
        assert!(selection_matches(&key, version, 3, Some(&key), version, 3));
        assert!(!selection_matches(&key, version, 3, Some(&key), version, 4));
        assert!(!selection_matches(
            &key,
            version,
            3,
            Some(&key),
            Some(DirectBodyVersion {
                uid: 42,
                source_generation: 4
            }),
            3
        ));
        let other = MessageKey {
            account_id: 8,
            ..key.clone()
        };
        assert!(!selection_matches(
            &key,
            version,
            3,
            Some(&other),
            version,
            3
        ));
    }
    #[test]
    fn original_body_debug_does_not_expose_source_html() {
        let body = OriginalBody::capture("<p>fictional-private-html-marker</p>", Some(true), None);
        let debug = format!("{body:?}");
        assert!(debug.contains("source_bytes"));
        assert!(!debug.contains("fictional-private-html-marker"));
        assert!(!debug.contains("<p>"));
    }

    #[test]
    fn retired_or_foreign_scope_never_authorizes_events() {
        let scope = ReaderScope {
            account_id: Arc::from("1"),
            folder_id: Arc::from("INBOX"),
            message_id: Arc::from("7"),
            generation: 1,
        };
        let mut session = Session::default();
        session.active = Some(scope.clone());
        assert!(session.accepts(&scope));
        session.retire();
        assert!(!session.accepts(&scope));
    }
}

// SPDX-License-Identifier: AGPL-3.0-or-later
//! Immutable reader input. Original untrusted source never substitutes for the
//! existing mail_text semantic fallback. Construct and encode off the UI thread.
use std::{collections::HashSet, sync::Arc};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::Serialize;

pub const SOURCE_MAX: usize = 2 * 1024 * 1024;
pub const WIRE_MAX: usize = 20 * 1024 * 1024;
pub const RESOURCE_MAX: usize = 2 * 1024 * 1024;
pub const RESOURCE_COUNT_MAX: usize = 32;

/// Never sent to WebKit. The owner must compare the entire scope on every event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReaderScope {
    pub account_id: Arc<str>,
    pub folder_id: Arc<str>,
    /// UID including UIDVALIDITY, or a session-scoped Thunderbird message key.
    pub message_id: Arc<str>,
    pub generation: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Completeness {
    Complete,
    Truncated,
    TooLarge,
    Missing,
}

/// Unknown is deliberately ineligible. Preserve the direct worker's signed and
/// encrypted exclusions; do not infer TB eligibility from the presence of HTML.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CacheEligibility {
    Allowed,
    Signed,
    Decrypted,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RasterType {
    Png,
    Jpeg,
    WebP,
}
impl RasterType {
    fn mime(self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::WebP => "image/webp",
        }
    }
}

/// Bytes must come from the selected MIME related-part scope, not an attachment
/// filename lookup or URL fetch. The helper independently decodes and normalizes.
#[derive(Clone)]
pub struct ReaderResource {
    cid: Arc<str>,
    media_type: RasterType,
    bytes: Arc<[u8]>,
}
impl ReaderResource {
    pub fn new(cid: &str, media_type: RasterType, bytes: Arc<[u8]>) -> Result<Self, DocumentError> {
        let cid = canonical_cid(cid).ok_or(DocumentError::InvalidResource)?;
        if bytes.is_empty() || bytes.len() > RESOURCE_MAX {
            return Err(DocumentError::InvalidResource);
        }
        Ok(Self {
            cid: cid.into(),
            media_type,
            bytes,
        })
    }
}
fn canonical_cid(cid: &str) -> Option<String> {
    let cid = cid.trim();
    let cid = cid
        .strip_prefix('<')
        .and_then(|s| s.strip_suffix('>'))
        .unwrap_or(cid);
    if cid.is_empty()
        || cid.len() > 256
        || !cid
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"@._+-".contains(&b))
    {
        return None;
    }
    Some(cid.to_ascii_lowercase())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DocumentError {
    InvalidScope,
    InvalidCompleteness,
    SourceLimit,
    InvalidResource,
    DuplicateResource,
    OriginalUnavailable,
    WireLimit,
    Encoding,
}

/// No Debug/Serialize on this sensitive bundle: avoid body/resource logging.
#[derive(Clone)]
pub struct ReaderDocument {
    scope: ReaderScope,
    original_html: Option<Arc<str>>,
    semantic_html: Arc<str>,
    plain_text: Arc<str>,
    title: Arc<str>,
    completeness: Completeness,
    cache_eligibility: CacheEligibility,
    resources: Arc<[ReaderResource]>,
}
impl ReaderDocument {
    /// semantic_html must be the existing mail_text::reader_html result, never
    /// the original. The helper's parser constructs a distinct inert engine tree.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        scope: ReaderScope,
        original_html: Option<Arc<str>>,
        semantic_html: Arc<str>,
        plain_text: Arc<str>,
        title: Arc<str>,
        completeness: Completeness,
        cache_eligibility: CacheEligibility,
        resources: Vec<ReaderResource>,
    ) -> Result<Self, DocumentError> {
        if [&scope.account_id, &scope.folder_id, &scope.message_id]
            .iter()
            .any(|s| s.is_empty() || s.len() > 1024 || s.contains('\0'))
        {
            return Err(DocumentError::InvalidScope);
        }
        if (completeness == Completeness::Complete
            && original_html.as_deref().is_none_or(str::is_empty))
            || (completeness == Completeness::Missing && original_html.is_some())
        {
            return Err(DocumentError::InvalidCompleteness);
        }
        if original_html
            .as_deref()
            .is_some_and(|s| s.len() > SOURCE_MAX || s.contains('\0'))
            || semantic_html.len() > SOURCE_MAX
            || plain_text.len() > SOURCE_MAX
            || semantic_html.contains('\0')
            || plain_text.contains('\0')
            || title.len() > 512
            || title.chars().any(char::is_control)
        {
            return Err(DocumentError::SourceLimit);
        }
        let mut ids = HashSet::new();
        let mut total = 0usize;
        if resources.len() > RESOURCE_COUNT_MAX {
            return Err(DocumentError::InvalidResource);
        }
        for r in &resources {
            if !ids.insert(r.cid.as_ref()) {
                return Err(DocumentError::DuplicateResource);
            }
            total = total
                .checked_add(r.bytes.len())
                .ok_or(DocumentError::InvalidResource)?;
            if total > 8 * 1024 * 1024 {
                return Err(DocumentError::InvalidResource);
            }
        }
        Ok(Self {
            scope,
            original_html,
            semantic_html,
            plain_text,
            title,
            completeness,
            cache_eligibility,
            resources: resources.into(),
        })
    }
    pub fn scope(&self) -> &ReaderScope {
        &self.scope
    }
    /// Inspection/encoding only. Never give this string to native TextView.
    pub fn original_source(&self) -> Option<&str> {
        self.original_html.as_deref()
    }
    pub fn semantic_html(&self) -> &str {
        &self.semantic_html
    }
    pub fn plain_text(&self) -> &str {
        &self.plain_text
    }
    pub fn completeness(&self) -> Completeness {
        self.completeness
    }
    pub fn cache_eligibility(&self) -> CacheEligibility {
        self.cache_eligibility
    }
    pub fn persistent_cache_allowed(&self) -> bool {
        self.cache_eligibility == CacheEligibility::Allowed
            && self.completeness == Completeness::Complete
    }
    /// Retained-byte accounting excludes Arc overhead and transient parser/IPC copies.
    pub fn retained_bytes(&self) -> usize {
        self.original_html.as_ref().map_or(0, |s| s.len())
            + self.semantic_html.len()
            + self.plain_text.len()
            + self.title.len()
            + self
                .resources
                .iter()
                .map(|r| r.bytes.len() + r.cid.len())
                .sum::<usize>()
    }
    pub fn helper_request(&self) -> Result<Vec<u8>, DocumentError> {
        if self.completeness != Completeness::Complete {
            return Err(DocumentError::OriginalUnavailable);
        }
        let html = self
            .original_html
            .as_deref()
            .ok_or(DocumentError::OriginalUnavailable)?;
        let resources = self
            .resources
            .iter()
            .map(|r| WireResource {
                cid: &r.cid,
                mime: r.media_type.mime(),
                base64: STANDARD.encode(&r.bytes),
            })
            .collect();
        let wire = WireDocument {
            version: 1,
            html,
            plain_text: &self.plain_text,
            title: &self.title,
            resources,
        };
        // The bounded inputs bound worst-case escaping allocation; final wire has
        // a separate cap. This is intentionally performed on the host worker.
        let bytes = serde_json::to_vec(&wire).map_err(|_| DocumentError::Encoding)?;
        if bytes.len() > WIRE_MAX {
            return Err(DocumentError::WireLimit);
        }
        Ok(bytes)
    }
}
#[derive(Serialize)]
struct WireDocument<'a> {
    version: u8,
    html: &'a str,
    #[serde(rename = "plainText")]
    plain_text: &'a str,
    title: &'a str,
    resources: Vec<WireResource<'a>>,
}
#[derive(Serialize)]
struct WireResource<'a> {
    cid: &'a str,
    mime: &'a str,
    base64: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    fn document(completeness: Completeness, cache: CacheEligibility) -> ReaderDocument {
        ReaderDocument::new(
            ReaderScope { account_id: "a".into(), folder_id: "Inbox".into(), message_id: "1:9".into(), generation: 7 },
            Some("<style>@media(max-width:400px){td{display:block}}</style><table><tr><td>original</td></tr></table>".into()),
            "<p>safe fallback</p>".into(), "plain fallback".into(), "Fixture".into(),
            completeness, cache, vec![],
        ).unwrap()
    }
    #[test]
    fn wire_keeps_original_separate_from_native_fallback() {
        let doc = document(Completeness::Complete, CacheEligibility::Unknown);
        let wire: serde_json::Value =
            serde_json::from_slice(&doc.helper_request().unwrap()).unwrap();
        assert!(wire["html"].as_str().unwrap().contains("@media"));
        assert_eq!(doc.semantic_html(), "<p>safe fallback</p>");
        assert!(wire.get("account_id").is_none());
    }
    #[test]
    fn truncated_source_never_enters_engine() {
        assert_eq!(
            document(Completeness::Truncated, CacheEligibility::Allowed).helper_request(),
            Err(DocumentError::OriginalUnavailable)
        );
    }
    #[test]
    fn signed_decrypted_and_unknown_are_not_persistent_cache_candidates() {
        for policy in [
            CacheEligibility::Signed,
            CacheEligibility::Decrypted,
            CacheEligibility::Unknown,
        ] {
            assert!(!document(Completeness::Complete, policy).persistent_cache_allowed());
        }
    }
    #[test]
    fn cid_normalization_is_unambiguous_and_svg_has_no_type() {
        assert_eq!(
            canonical_cid(" <Logo@Example.INVALID> "),
            Some("logo@example.invalid".into())
        );
        assert_eq!(canonical_cid("../profile"), None);
    }
    #[test]
    fn duplicate_cids_are_rejected_before_ipc() {
        let r = ReaderResource::new("<logo@fixture.invalid>", RasterType::Png, vec![1u8].into())
            .unwrap();
        let doc = document(Completeness::Complete, CacheEligibility::Allowed);
        assert!(matches!(
            ReaderDocument::new(
                doc.scope.clone(),
                doc.original_html.clone(),
                doc.semantic_html.clone(),
                doc.plain_text.clone(),
                doc.title.clone(),
                doc.completeness,
                doc.cache_eligibility,
                vec![r.clone(), r]
            ),
            Err(DocumentError::DuplicateResource)
        ));
    }
}

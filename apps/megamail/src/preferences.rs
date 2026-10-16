//! Private mail-reading preferences for MegaMail.

use std::fs::File;
use std::io::{Read, Take};
use std::path::Path;

use megamail_core::config::{config_base, write_private_file};
use serde::{Deserialize, Serialize};

const PREFERENCES_FILE: &str = "preferences.v1";
const MAX_PREFERENCES_BYTES: u64 = 4096;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Density {
    #[default]
    Comfortable,
    Compact,
}

impl Density {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Comfortable => "Regular",
            Self::Compact => "Compact",
        }
    }

    pub(crate) const fn row_height(self) -> f32 {
        match self {
            Self::Comfortable => 78.0,
            Self::Compact => 64.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub(crate) struct MailPreferences {
    pub(crate) density: Density,
    pub(crate) group_conversations: bool,
    pub(crate) hide_quoted_text: bool,
    pub(crate) reduced_motion: bool,
    pub(crate) default_unified: bool,
}

impl Default for MailPreferences {
    fn default() -> Self {
        Self {
            density: Density::Comfortable,
            group_conversations: true,
            hide_quoted_text: true,
            reduced_motion: false,
            default_unified: true,
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct VersionedMailPreferences {
    version: u8,
    preferences: MailPreferences,
}

impl MailPreferences {
    /// Read preferences on a background task. Missing files use the defaults.
    pub(crate) fn load() -> Result<Self, String> {
        let config_dir = config_base().ok_or_else(|| {
            "could not create MegaMail's private configuration directory".to_string()
        })?;
        Self::load_from_path(&config_dir.join(PREFERENCES_FILE))
    }

    pub(crate) fn persist(&self) -> Result<(), String> {
        let config_dir = config_base().ok_or_else(|| {
            "could not create MegaMail's private configuration directory".to_string()
        })?;
        self.persist_to_path(&config_dir.join(PREFERENCES_FILE))
    }

    fn load_from_path(path: &Path) -> Result<Self, String> {
        let file = match File::open(path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default());
            }
            Err(error) => return Err(format!("could not open mail preferences: {error}")),
        };
        let mut bytes = Vec::new();
        take_bounded(file, MAX_PREFERENCES_BYTES)
            .read_to_end(&mut bytes)
            .map_err(|error| format!("could not read mail preferences: {error}"))?;
        if bytes.len() as u64 > MAX_PREFERENCES_BYTES {
            return Err("mail preferences exceed the 4096-byte limit".into());
        }
        let stored: VersionedMailPreferences = serde_json::from_slice(&bytes)
            .map_err(|error| format!("could not parse mail preferences: {error}"))?;
        if stored.version != 1 {
            return Err(format!(
                "unsupported mail preferences version: {}",
                stored.version
            ));
        }
        Ok(stored.preferences)
    }

    fn persist_to_path(&self, path: &Path) -> Result<(), String> {
        let encoded = serde_json::to_string_pretty(&VersionedMailPreferences {
            version: 1,
            preferences: *self,
        })
        .map_err(|error| format!("could not encode mail preferences: {error}"))?;
        write_private_file(path, &encoded)
            .map_err(|error| format!("could not save mail preferences: {error}"))
    }
}

fn take_bounded(file: File, limit: u64) -> Take<File> {
    file.take(limit + 1)
}

/// Extract trailing outer blockquotes from the canonical, already sanitized
/// native fragment. A linear scan avoids building another DOM on the UI thread.
/// Authored text after a quote prevents hiding that quote.
pub(crate) fn quoted_html(html: &str) -> Option<String> {
    if html.len() > 2 * 1024 * 1024 {
        return None;
    }
    let mut ranges = Vec::new();
    let mut depth = 0usize;
    let mut start = 0;
    let mut offset = 0;
    while let Some(next) = html[offset..].find('<') {
        let at = offset + next;
        let end = at + html[at..].find('>')? + 1;
        let tag = &html[at..end];
        if tag == "<blockquote>" || tag.starts_with("<blockquote ") {
            if depth == 0 {
                start = at;
            }
            depth += 1;
        } else if tag == "</blockquote>" {
            depth = depth.checked_sub(1)?;
            if depth == 0 {
                ranges.push(start..end);
            }
        }
        offset = end;
    }
    if depth != 0 {
        return None;
    }
    fn only_wrapper_ends(mut text: &str) -> bool {
        loop {
            text = text.trim();
            if text.is_empty() {
                return true;
            }
            if !text.starts_with("</") {
                return false;
            }
            let Some(end) = text.find('>') else {
                return false;
            };
            text = &text[end + 1..];
        }
    }
    let mut end = html.len();
    let mut quotes = Vec::new();
    for range in ranges.into_iter().rev() {
        if !only_wrapper_ends(&html[range.end..end]) {
            break;
        }
        quotes.push(&html[range.clone()]);
        end = range.start;
    }
    if quotes.is_empty() {
        None
    } else {
        quotes.reverse();
        Some(quotes.concat())
    }
}

/// Split only a trailing, contiguous quoted block, retaining the original text.
pub(crate) fn split_quoted_text(body: &str) -> (&str, Option<&str>) {
    if let Some(split) = outlook_quote_boundary(body) {
        return (&body[..split], Some(&body[split..]));
    }
    let lines = body.split_inclusive('\n').collect::<Vec<_>>();
    let mut offset = 0;
    for line in &lines {
        let marker = line_content(line).trim();
        if offset > 0
            && !body[..offset].trim().is_empty()
            && is_reply_marker(marker)
            && marker.contains('@')
            && marker.contains('<')
            && marker.contains('>')
        {
            return (&body[..offset], Some(&body[offset..]));
        }
        offset += line.len();
    }
    let mut suffix_start = lines.len();
    let mut has_quote = false;
    while suffix_start > 0 {
        let line = line_content(lines[suffix_start - 1]);
        if line.starts_with('>') {
            has_quote = true;
            suffix_start -= 1;
        } else if line.trim().is_empty() {
            suffix_start -= 1;
        } else {
            break;
        }
    }
    if !has_quote {
        return (body, None);
    }

    if suffix_start > 0 && is_reply_marker(line_content(lines[suffix_start - 1]).trim()) {
        suffix_start -= 1;
    }

    let split = lines[..suffix_start].iter().map(|line| line.len()).sum();
    (&body[..split], Some(&body[split..]))
}

/// Outlook and localized reply history often has no > or blockquote marker.
/// Require a From header plus two distinct following header roles and a real
/// current reply before it, so an isolated "From:" in authored prose is kept.
fn outlook_quote_boundary(body: &str) -> Option<usize> {
    let lines: Vec<_> = body.split_inclusive('\n').collect();
    let mut offset = 0;
    for (i, line) in lines.iter().enumerate() {
        let value = line
            .trim()
            .trim_start_matches(['\u{200e}', '\u{200f}'])
            .trim();
        let from = ["from:", "מאת:", "von:", "de:"]
            .iter()
            .any(|p| value.to_lowercase().starts_with(p));
        if from && offset > 0 && !body[..offset].trim().is_empty() {
            let tail: Vec<_> = lines[i + 1..]
                .iter()
                .filter(|l| !l.trim().is_empty())
                .take(6)
                .map(|l| {
                    l.trim()
                        .trim_start_matches(['\u{200e}', '\u{200f}'])
                        .to_lowercase()
                })
                .collect();
            let roles = [
                ["sent:", "נשלח:", "gesendet:", "envoyé:"],
                ["to:", "אל:", "an:", "à:"],
                ["subject:", "נושא:", "betreff:", "objet:"],
            ];
            if roles
                .iter()
                .filter(|role| {
                    tail.iter()
                        .any(|line| role.iter().any(|prefix| line.starts_with(prefix)))
                })
                .count()
                >= 2
            {
                return Some(offset);
            }
        }
        offset += line.len();
    }
    None
}

fn line_content(line: &str) -> &str {
    let line = line.strip_suffix('\n').unwrap_or(line);
    line.strip_suffix('\r').unwrap_or(line)
}

/// The native selectable text view receives generated markup, never email HTML.
pub(crate) fn plain_text_html(body: &str) -> String {
    body.split('\n')
        .map(|line| {
            let escaped = line
                .replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;");
            format!(
                "<p>{}</p>",
                if escaped.is_empty() {
                    "&#160;"
                } else {
                    &escaped
                }
            )
        })
        .collect()
}

fn is_reply_marker(line: &str) -> bool {
    line.strip_prefix("On ")
        .is_some_and(|remainder| remainder.ends_with(" wrote:"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn quoted_html_preserves_nested_history_without_repeating_authored_text() {
        let html = "<p>Current reply</p><blockquote dir=\"rtl\"><p>Previous</p><blockquote>Older</blockquote></blockquote>";
        let quote = super::quoted_html(html).unwrap();
        assert!(!quote.contains("Current reply"));
        assert!(quote.contains("dir=\"rtl\""));
        assert!(quote.contains("<blockquote>Older</blockquote>"));
        assert_eq!(
            super::quoted_html("<p>Before</p><blockquote>Inline citation</blockquote><p>After</p>"),
            None
        );
        assert_eq!(
            super::quoted_html("<blockquote>One</blockquote><blockquote>Two</blockquote>"),
            Some("<blockquote>One</blockquote><blockquote>Two</blockquote>".into())
        );
        assert_eq!(super::quoted_html("<blockquote>Unclosed"), None);
    }
    #[test]
    fn address_attribution_splits_unprefixed_history_and_restores_exactly() {
        let original = "Current reply.\n\nOn 16 Oct 2026, Ada <ada@example.test> wrote:\n\nEarlier text\nOlder text";
        let (current, history) = super::split_quoted_text(original);
        assert_eq!(current, "Current reply.\n\n");
        assert!(history.unwrap().starts_with("On 16 Oct"));
        assert_eq!(format!("{current}{}", history.unwrap()), original);
        assert!(
            super::split_quoted_text(
                "Today I read a note.\nOn empathy, Ada wrote:\nThis is authored prose"
            )
            .1
            .is_none()
        );
    }
    #[test]
    fn localized_outlook_history_is_collapsed_and_exactly_restorable() {
        let body = "שלום,\nהתשובה כאן.\n\nמאת: Fictional <a@example.test>\nנשלח: yesterday\nאל: b@example.test\nנושא: שאלה\n\nOriginal text";
        let (reply, quote) = split_quoted_text(body);
        assert!(reply.contains("התשובה כאן"));
        assert!(!reply.contains("Original text"));
        assert_eq!(format!("{}{}", reply, quote.unwrap()), body);
        assert!(
            split_quoted_text("Authored prose\nFrom: the documentation\nTo: illustrate a point")
                .1
                .is_none()
        );
        assert!(
            split_quoted_text(
                "From: a@example.test\nSent: today\nTo: b@example.test\nEntire original message"
            )
            .1
            .is_none()
        );
    }
    #[test]
    fn selectable_body_markup_cannot_create_remote_content_or_links() {
        let html = super::plain_text_html(
            "<img src='https://remote.test/pixel'><script>bad()</script>\n<a href='file:///private'>x & y</a>",
        );
        assert!(!html.contains("<img"));
        assert!(!html.contains("<script"));
        assert!(!html.contains("<a "));
        assert!(html.contains("&lt;img"));
        assert_eq!(html.matches("<p>").count(), 2);
        assert_eq!(
            super::plain_text_html("one\n\ntwo"),
            "<p>one</p><p>&#160;</p><p>two</p>"
        );
        assert!(html.contains("x &amp; y"));
    }
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

    struct TempFile(PathBuf);

    impl Drop for TempFile {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.0);
        }
    }

    fn temp_file() -> TempFile {
        let id = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        TempFile(std::env::temp_dir().join(format!(
            "megamail-preferences-{}-{id}.json",
            std::process::id()
        )))
    }

    #[test]
    fn missing_preferences_use_defaults_and_persist_round_trips() {
        let path = temp_file();
        assert_eq!(
            MailPreferences::load_from_path(&path.0).unwrap(),
            MailPreferences::default()
        );

        let preferences = MailPreferences {
            density: Density::Compact,
            group_conversations: false,
            ..MailPreferences::default()
        };
        preferences.persist_to_path(&path.0).unwrap();
        assert_eq!(
            MailPreferences::load_from_path(&path.0).unwrap(),
            preferences
        );
    }

    #[test]
    fn load_rejects_oversized_files_and_unknown_versions_or_fields() {
        let path = temp_file();
        fs::write(&path.0, vec![b' '; MAX_PREFERENCES_BYTES as usize + 1]).unwrap();
        assert!(
            MailPreferences::load_from_path(&path.0)
                .unwrap_err()
                .contains("4096-byte")
        );

        fs::write(&path.0, br#"{"version":2,"preferences":{}}"#).unwrap();
        assert!(
            MailPreferences::load_from_path(&path.0)
                .unwrap_err()
                .contains("version: 2")
        );

        fs::write(&path.0, br#"{"version":1,"preferences":{"density":17}}"#).unwrap();
        assert!(MailPreferences::load_from_path(&path.0).is_err());
    }

    #[test]
    fn quoted_text_splits_only_from_the_end_and_keeps_reply_marker() {
        assert_eq!(
            split_quoted_text("Hello\nOn Tue, Ada wrote:\n> Old message\n"),
            ("Hello\n", Some("On Tue, Ada wrote:\n> Old message\n")),
        );
        assert_eq!(
            split_quoted_text("Hello\n> quoted\nfooter"),
            ("Hello\n> quoted\nfooter", None),
        );
        assert_eq!(
            split_quoted_text("Hello\n> quoted\n\n"),
            ("Hello\n", Some("> quoted\n\n")),
        );
        assert_eq!(split_quoted_text("Plain body\n"), ("Plain body\n", None));
    }
}

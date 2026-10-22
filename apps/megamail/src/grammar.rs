//! Opt-in grammar check for the composer, fully offline through Harper
//! (English only). Spelling stays with `spell.rs`; quoted reply lines, URLs
//! and the signature are not linted.
use harper_core::linting::{LintGroup, LintKind, Linter, Suggestion};
use harper_core::spell::FstDictionary;
use harper_core::{Dialect, Document};
use std::ops::Range;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Issue {
    pub range: Range<usize>,
    pub message: String,
    pub suggestions: Vec<String>,
}

/// Blank out text Harper should not judge, keeping every byte offset intact.
fn mask(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_signature = false;
    for line in text.split_inclusive('\n') {
        if line.trim_end() == "--" || line.trim_end_matches('\n') == "-- " {
            in_signature = true;
        }
        let skip = in_signature || line.trim_start().starts_with('>');
        if skip {
            out.extend(line.chars().map(|c| if c == '\n' { '\n' } else { ' ' }));
        } else {
            let mut rest = line;
            while let Some(at) = rest.find("http") {
                out.push_str(&rest[..at]);
                let end = rest[at..].find(char::is_whitespace).map_or(rest.len(), |e| at + e);
                out.extend(rest[at..end].chars().map(|_| ' '));
                rest = &rest[end..];
            }
            out.push_str(rest);
        }
    }
    out
}

pub(crate) fn check(text: &str) -> Vec<Issue> {
    let masked = mask(text);
    let mut linter = LintGroup::new_curated(FstDictionary::curated(), Dialect::American);
    let document = Document::new_plain_english_curated(&masked);
    let byte_at: Vec<usize> = masked.char_indices().map(|(i, _)| i).chain([masked.len()]).collect();
    linter
        .lint(&document)
        .into_iter()
        .filter(|lint| lint.lint_kind != LintKind::Spelling && lint.span.end < byte_at.len())
        .filter(|lint| !masked[byte_at[lint.span.start]..byte_at[lint.span.end]].trim().is_empty())
        .map(|lint| Issue {
            range: byte_at[lint.span.start]..byte_at[lint.span.end],
            message: lint.message,
            suggestions: lint
                .suggestions
                .iter()
                .filter_map(|s| match s {
                    Suggestion::ReplaceWith(chars) => Some(chars.iter().collect::<String>()),
                    _ => None,
                })
                .take(4)
                .collect(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_grammar_but_not_quotes_or_signature() {
        let issues = check("This are a test of the the system.\n> This are quoted.\n-- \nThis are signed\n");
        assert!(issues.iter().all(|i| i.range.end <= 40), "{issues:?}");
        assert!(!issues.is_empty());
        for issue in &issues {
            assert!(issue.range.start < issue.range.end);
        }
    }

    #[test]
    fn mask_keeps_length() {
        let text = "a http://x.y/z b\n> q\n";
        assert_eq!(mask(text).len(), text.len());
    }
}

//! Parsed, bounded mail-search predicates shared by native loaded-header filtering
//! and future cache queries. This module does not fetch mail or open a database.
//! SQL uses `m` for messages and (only for body terms) `b` for eligible bodies.
//! Every search connection must call `register_sql_functions` before SQL use.
//! See docs/research/batch2-search-normalization.md for the repaired text seam
//! and docs/research/batch2-search-triage.md for the remaining cache contract.

use chrono::NaiveDate;
use rusqlite::functions::FunctionFlags;
use rusqlite::types::Value;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::models::Message;

/// Maximum UTF-8 query size. Oversized input is rejected, never truncated.
pub const QUERY_BYTES_MAX: usize = 4096;
/// Maximum conjunctive terms in one query.
pub const TERMS_MAX: usize = 32;
/// Maximum eligible body text examined by either matcher, before normalization.
pub const BODY_BYTES_MAX: usize = 2 * 1024 * 1024;
/// Maximum bytes in one header/preview field, before normalization.
/// Oversize fields are Unknown, never silently truncated or treated as absent.
pub const HEADER_BYTES_MAX: usize = 2 * 1024 * 1024;

/// Install the exact text predicates used by [`SearchQuery::sql_predicate`].
/// Requires rusqlite's `functions` feature. Register on every owning background
/// search connection, including replacement/read-only connections. No schema or
/// cache contents are changed. Unregistered SQL fails rather than using LIKE.
///
/// Both predicates replace each NUL byte with one ASCII space for comparison,
/// exactly like `Message::scrub_nuls`; they do not remove NULs or decode escapes.
/// Missing, non-text, non-UTF-8 and oversize fields return SQL NULL (Unknown).
/// Input lengths are checked before UTF-8 validation/scanning; no normalized
/// text allocation is needed. Needles are bounded by the parser's byte limit.
/// Functions are deterministic and direct-only, not callable by schema code.
pub fn register_sql_functions(connection: &Connection) -> rusqlite::Result<()> {
    for (name, limit) in [
        ("megamail_search_header_contains", HEADER_BYTES_MAX),
        ("megamail_search_body_contains", BODY_BYTES_MAX),
    ] {
        connection.create_scalar_function(
            name,
            2,
            FunctionFlags::SQLITE_UTF8
                | FunctionFlags::SQLITE_DETERMINISTIC
                | FunctionFlags::SQLITE_DIRECTONLY,
            move |context| {
                let needle_value = context.get_raw(1);
                // Text access must retain SQLite's explicit byte length; neither
                // C-string decoding nor SQLite length(text) is NUL-safe.
                let needle_bytes = match needle_value {
                    rusqlite::types::ValueRef::Text(bytes) => Some(bytes),
                    _ => None,
                };
                let Some(needle_bytes) = needle_bytes
                    .filter(|bytes| !bytes.is_empty() && bytes.len() <= QUERY_BYTES_MAX)
                else {
                    return Err(rusqlite::Error::UserFunctionError(Box::new(
                        std::io::Error::new(
                            std::io::ErrorKind::InvalidInput,
                            "Search predicate needle has an invalid byte length.",
                        ),
                    )));
                };
                let needle = std::str::from_utf8(needle_bytes)
                    .map_err(|error| rusqlite::Error::UserFunctionError(Box::new(error)))?;
                let field_value = context.get_raw(0);
                let rusqlite::types::ValueRef::Text(bytes) = field_value else {
                    return Ok(None);
                };
                if bytes.len() > limit {
                    return Ok(None);
                }
                // A corrupt UTF-8 projection cannot be compared to a Rust String.
                // Retain Unknown under NOT rather than classifying it as empty.
                Ok(std::str::from_utf8(bytes)
                    .ok()
                    .and_then(|field| contains(field, needle, limit)))
            },
        )?;
    }
    Ok(())
}

/// Search coverage, not a claim about provider synchronization completeness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SearchScope {
    LoadedHeaders,
    AllLocalIndexed,
    ProviderHistory,
}

impl SearchScope {
    /// Accurate UI copy even when all currently indexed rows have been searched.
    pub const fn label(self) -> &'static str {
        match self {
            Self::LoadedHeaders => "Loaded headers only",
            Self::AllLocalIndexed => "All locally indexed mail (not all provider history)",
            Self::ProviderHistory => "Provider search (coverage reported by provider)",
        }
    }
}

/// Explicit availability supplied by the owning backend. Default is fail-closed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SearchCapabilities {
    pub all_local_indexed: bool,
    pub provider_history: bool,
    pub body: bool,
}

/// Errors contain no source query text (queries can contain personal data).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchError {
    TooLong,
    TooManyTerms,
    InvalidSyntax,
    UnknownOperator,
    InvalidDate,
    InvalidBoolean,
    UnsupportedScope,
    UnsupportedBody,
    InvalidSavedQuery,
}

impl std::fmt::Display for SearchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::TooLong => "Search is limited to 4,096 bytes.",
            Self::TooManyTerms => "Search is limited to 32 terms.",
            Self::InvalidSyntax => {
                "Use words, quoted phrases and - exclusions. Boolean groups are not supported."
            }
            Self::UnknownOperator => "This search operator or value is not supported.",
            Self::InvalidDate => "Use a valid date in YYYY-MM-DD format.",
            Self::InvalidBoolean => "Use true or false for this search filter.",
            Self::UnsupportedScope => "This account does not support the requested search scope.",
            Self::UnsupportedBody => "Body search is not available in this search scope.",
            Self::InvalidSavedQuery => {
                "This saved query has an invalid version, name or account/folder scope."
            }
        })
    }
}
impl std::error::Error for SearchError {}

/// Fixed field set, also used to generate parameterized SQL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextField {
    AnyHeader,
    From,
    To,
    Cc,
    Subject,
    Body,
}

/// A single AND term. Date bounds are UTC Unix seconds; end is exclusive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Condition {
    Contains(TextField, String),
    Date {
        start: Option<i64>,
        end: Option<i64>,
    },
    Unread(bool),
    Starred(bool),
    Attachments(bool),
}

/// Negation is supported, but OR/grouping are rejected rather than misread.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Term {
    pub negated: bool,
    pub condition: Condition,
}

/// Validated query. Empty input matches every header in the explicit scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchQuery {
    terms: Vec<Term>,
}

/// Body absence is not an empty body. Decrypted/sensitive bodies must be Excluded.
#[derive(Debug, Clone, Copy)]
pub enum BodyText<'a> {
    Available(&'a str),
    NotLoaded,
    Excluded,
}

/// Unknown results must not be displayed as known matches or known nonmatches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchMatch {
    Match,
    NoMatch,
    Unknown,
}

impl SearchQuery {
    /// Parse AND terms, quoted phrases, `-` exclusions and documented operators.
    /// Rejects unsupported boolean expressions and malformed values.
    pub fn parse(input: &str) -> Result<Self, SearchError> {
        let tokens = tokenize(input)?;
        let mut terms = Vec::with_capacity(tokens.len());
        for token in tokens {
            let literal = token.literal;
            let negated = token.excluded;
            let token = if negated {
                token
                    .text
                    .strip_prefix('-')
                    .ok_or(SearchError::InvalidSyntax)?
            } else {
                token.text.as_str()
            };
            if token.is_empty()
                || (!literal
                    && ["AND", "OR", "NOT"]
                        .iter()
                        .any(|op| token.eq_ignore_ascii_case(op)))
            {
                return Err(SearchError::InvalidSyntax);
            }
            let condition = if literal {
                text(TextField::AnyHeader, token)
            } else if let Some((operator, value)) = token.split_once(':') {
                if value.is_empty() {
                    return Err(SearchError::InvalidSyntax);
                }
                match operator.to_ascii_lowercase().as_str() {
                    "from" => text(TextField::From, value),
                    "to" => text(TextField::To, value),
                    "cc" => text(TextField::Cc, value),
                    "subject" => text(TextField::Subject, value),
                    "body" => text(TextField::Body, value),
                    "date" => {
                        let start = utc_day(value)?;
                        Condition::Date {
                            start: Some(start),
                            end: Some(start + 86400),
                        }
                    }
                    "before" => Condition::Date {
                        start: None,
                        end: Some(utc_day(value)?),
                    },
                    "after" => Condition::Date {
                        start: Some(utc_day(value)? + 86400),
                        end: None,
                    },
                    "unread" => Condition::Unread(boolean(value)?),
                    "starred" => Condition::Starred(boolean(value)?),
                    "attachments" => Condition::Attachments(boolean(value)?),
                    "has" if value.eq_ignore_ascii_case("attachment") => {
                        Condition::Attachments(true)
                    }
                    "is" if value.eq_ignore_ascii_case("unread") => Condition::Unread(true),
                    "is" if value.eq_ignore_ascii_case("read") => Condition::Unread(false),
                    "is" if value.eq_ignore_ascii_case("starred") => Condition::Starred(true),
                    _ => return Err(SearchError::UnknownOperator),
                }
            } else {
                text(TextField::AnyHeader, token)
            };
            terms.push(Term { negated, condition });
        }
        Ok(Self { terms })
    }

    /// Read-only parsed representation for backend adapters; never splice values into SQL.
    pub fn terms(&self) -> &[Term] {
        &self.terms
    }

    /// True only for explicit body terms; bare words search headers/preview only.
    pub fn requires_body(&self) -> bool {
        self.terms
            .iter()
            .any(|t| matches!(t.condition, Condition::Contains(TextField::Body, _)))
    }

    /// Gate scope/body support before submission. LoadedHeaders cannot search bodies.
    pub fn validate_scope(
        &self,
        scope: SearchScope,
        caps: SearchCapabilities,
    ) -> Result<(), SearchError> {
        match scope {
            SearchScope::LoadedHeaders => {}
            SearchScope::AllLocalIndexed if caps.all_local_indexed => {}
            SearchScope::ProviderHistory if caps.provider_history => {}
            _ => return Err(SearchError::UnsupportedScope),
        }
        if self.requires_body() && (scope == SearchScope::LoadedHeaders || !caps.body) {
            return Err(SearchError::UnsupportedBody);
        }
        Ok(())
    }

    /// Evaluate the exact parser's semantics on the existing production Message model.
    /// ASCII case folding is shared with the registered SQLite predicates;
    /// non-ASCII letters remain case-sensitive. NULs compare as ASCII spaces,
    /// matching the production model scrubber even for raw unsanitized models.
    /// Oversize fields, absent/excluded bodies and unknown dates stay Unknown
    /// under NOT. A known header hit dominates an unknown alternative header.
    pub fn matches(&self, message: &Message, body: BodyText<'_>) -> SearchMatch {
        let mut unknown = false;
        for term in &self.terms {
            let hit = match &term.condition {
                Condition::Contains(field, needle) => match field {
                    TextField::AnyHeader => header_contains(
                        &[
                            &message.from_name,
                            &message.from_addr,
                            &message.subject,
                            &message.preview,
                        ],
                        needle,
                    ),
                    TextField::From => {
                        header_contains(&[&message.from_name, &message.from_addr], needle)
                    }
                    TextField::To => contains(&message.to, needle, HEADER_BYTES_MAX),
                    TextField::Cc => contains(&message.cc, needle, HEADER_BYTES_MAX),
                    TextField::Subject => contains(&message.subject, needle, HEADER_BYTES_MAX),
                    TextField::Body => match body {
                        BodyText::Available(s) => contains(s, needle, BODY_BYTES_MAX),
                        _ => None,
                    },
                },
                Condition::Date { start, end } => (message.timestamp != 0).then(|| {
                    start.is_none_or(|s| message.timestamp >= s)
                        && end.is_none_or(|e| message.timestamp < e)
                }),
                Condition::Unread(want) => Some(message.unread == *want),
                Condition::Starred(want) => Some(message.starred == *want),
                Condition::Attachments(want) => Some(message.has_attachment == *want),
            };
            match hit {
                Some(hit) if hit == term.negated => return SearchMatch::NoMatch,
                None => unknown = true,
                _ => {}
            }
        }
        if unknown {
            SearchMatch::Unknown
        } else {
            SearchMatch::Match
        }
    }

    /// Produce a WHERE fragment with anonymous placeholders and owned bound values.
    /// No account/folder constraint is included: the cache owner MUST add those
    /// separately and page on the existing stable cursor. Body joins must exclude
    /// encrypted bodies and supply normalized plain text, not HTML markup.
    /// Call [`register_sql_functions`] on the connection first. Text functions
    /// share NUL-to-space/ASCII/literal semantics and byte bounds with `matches`.
    /// Missing/invalid/oversize text remains SQL NULL, including under NOT.
    /// This is a predicate, not a provider-search implementation or an index.
    pub fn sql_predicate(&self, allow_body: bool) -> Result<SqlPredicate, SearchError> {
        if self.requires_body() && !allow_body {
            return Err(SearchError::UnsupportedBody);
        }
        let mut values = Vec::new();
        let mut clauses = Vec::new();
        for term in &self.terms {
            let clause = match &term.condition {
                Condition::Contains(field, needle) => {
                    let columns: &[&str] = match field {
                        TextField::AnyHeader => {
                            &["m.from_name", "m.from_addr", "m.subject", "m.preview"]
                        }
                        TextField::From => &["m.from_name", "m.from_addr"],
                        TextField::To => &["m.recipients"],
                        TextField::Cc => &["m.cc"],
                        TextField::Subject => &["m.subject"],
                        TextField::Body => &["b.body"],
                    };
                    let mut parts = Vec::new();
                    for column in columns {
                        let function = if *field == TextField::Body {
                            "megamail_search_body_contains"
                        } else {
                            "megamail_search_header_contains"
                        };
                        parts.push(format!("{function}({column}, ?)"));
                        values.push(Value::Text(needle.clone()));
                    }
                    format!("({})", parts.join(" OR "))
                }
                Condition::Date { start, end } => {
                    let mut bounds = Vec::new();
                    if let Some(start) = start {
                        bounds.push("m.ts >= ?");
                        values.push(Value::Integer(*start));
                    }
                    if let Some(end) = end {
                        bounds.push("m.ts < ?");
                        values.push(Value::Integer(*end));
                    }
                    // NULL lets NOT preserve unknown dates instead of matching them.
                    format!(
                        "(CASE WHEN m.ts = 0 THEN NULL ELSE ({}) END)",
                        bounds.join(" AND ")
                    )
                }
                Condition::Unread(want) => {
                    values.push(Value::Integer(i64::from(*want)));
                    "(m.unread != 0) = ?".into()
                }
                Condition::Starred(want) => {
                    values.push(Value::Integer(i64::from(*want)));
                    "(m.starred != 0) = ?".into()
                }
                Condition::Attachments(want) => {
                    values.push(Value::Integer(i64::from(*want)));
                    "(m.has_attachment != 0) = ?".into()
                }
            };
            clauses.push(if term.negated {
                format!("NOT ({clause})")
            } else {
                format!("({clause})")
            });
        }
        Ok(SqlPredicate {
            clause: if clauses.is_empty() {
                "1".into()
            } else {
                clauses.join(" AND ")
            },
            values,
            requires_body: self.requires_body(),
        })
    }
}

/// Bound SQL output. `clause` contains only fixed schema identifiers/placeholders.
#[derive(Debug, Clone)]
pub struct SqlPredicate {
    pub clause: String,
    pub values: Vec<Value>,
    pub requires_body: bool,
}

/// Untrusted persistence record. Deserialize this, then validate before use.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SavedQueryRecord {
    pub version: u32,
    pub name: String,
    pub query: String,
    pub scope: SearchScope,
    pub account_id: Option<u32>,
    pub folder_path: Option<String>,
}

/// Validated saved query; persistence and selection of accounts belong to caller.
#[derive(Debug, Clone)]
pub struct SavedQuery {
    record: SavedQueryRecord,
    parsed: SearchQuery,
}

impl SavedQuery {
    /// Validate schema, bounds, account/folder identity, grammar and actual capabilities.
    /// Caller must also check the account remains configured before executing.
    pub fn validate(
        record: SavedQueryRecord,
        caps: SearchCapabilities,
    ) -> Result<Self, SearchError> {
        if record.version != 1
            || record.name.trim().is_empty()
            || record.name.len() > 128
            || record.name.chars().any(char::is_control)
            || record.account_id == Some(0)
            || record.folder_path.as_ref().is_some_and(|p| {
                record.account_id.is_none()
                    || p.is_empty()
                    || p.len() > 1024
                    || p.chars().any(char::is_control)
            })
        {
            return Err(SearchError::InvalidSavedQuery);
        }
        let parsed = SearchQuery::parse(&record.query)?;
        parsed.validate_scope(record.scope, caps)?;
        Ok(Self { record, parsed })
    }

    pub fn record(&self) -> &SavedQueryRecord {
        &self.record
    }
    pub fn parsed(&self) -> &SearchQuery {
        &self.parsed
    }
}

fn text(field: TextField, value: &str) -> Condition {
    Condition::Contains(field, value.to_owned())
}
fn boolean(value: &str) -> Result<bool, SearchError> {
    if value.eq_ignore_ascii_case("true") {
        Ok(true)
    } else if value.eq_ignore_ascii_case("false") {
        Ok(false)
    } else {
        Err(SearchError::InvalidBoolean)
    }
}
fn utc_day(value: &str) -> Result<i64, SearchError> {
    if value.len() != 10
        || !value.bytes().enumerate().all(|(i, b)| {
            if i == 4 || i == 7 {
                b == b'-'
            } else {
                b.is_ascii_digit()
            }
        })
    {
        return Err(SearchError::InvalidDate);
    }
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .ok()
        .and_then(|d| d.and_hms_opt(0, 0, 0))
        .map(|d| d.and_utc().timestamp())
        .ok_or(SearchError::InvalidDate)
}
fn contains(haystack: &str, needle: &str, limit: usize) -> Option<bool> {
    if haystack.len() > limit {
        return None;
    }
    // Parsed needles are nonempty and bounded. Keep this helper total as well.
    if needle.is_empty() {
        return Some(true);
    }
    if !haystack.as_bytes().contains(&0) {
        return Some(
            haystack
                .as_bytes()
                .windows(needle.len())
                .any(|window| window.eq_ignore_ascii_case(needle.as_bytes())),
        );
    }
    Some(haystack.as_bytes().windows(needle.len()).any(|window| {
        window.iter().zip(needle.bytes()).all(|(&byte, want)| {
            // scrub_nuls replaces NUL with SPACE, not with the empty string.
            let normalized = if byte == 0 { b' ' } else { byte };
            normalized.eq_ignore_ascii_case(&want)
        })
    }))
}

fn header_contains(fields: &[&str], needle: &str) -> Option<bool> {
    let mut unknown = false;
    for field in fields {
        match contains(field, needle, HEADER_BYTES_MAX) {
            Some(true) => return Some(true),
            None => unknown = true,
            Some(false) => {}
        }
    }
    if unknown {
        None
    } else {
        Some(false)
    }
}
struct Token {
    text: String,
    literal: bool,
    excluded: bool,
}

fn tokenize(input: &str) -> Result<Vec<Token>, SearchError> {
    if input.len() > QUERY_BYTES_MAX {
        return Err(SearchError::TooLong);
    }
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut escaped = false;
    let mut literal = false;
    let mut started = false;
    let mut excluded = false;
    for ch in input.chars() {
        if escaped {
            if !matches!(ch, '"' | '\\') {
                return Err(SearchError::InvalidSyntax);
            }
            current.push(ch);
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == '"' {
            if !started && (current.is_empty() || current == "-") {
                literal = true;
            }
            started = true;
            quoted = !quoted;
        } else if ch.is_control() && (quoted || !ch.is_whitespace()) {
            return Err(SearchError::InvalidSyntax);
        } else if !quoted && matches!(ch, '(' | ')') {
            return Err(SearchError::InvalidSyntax);
        } else if !quoted && ch.is_whitespace() {
            if started && current.is_empty() {
                return Err(SearchError::InvalidSyntax);
            }
            if !current.is_empty() {
                tokens.push(Token {
                    text: std::mem::take(&mut current),
                    literal,
                    excluded,
                });
            }
            literal = false;
            started = false;
            excluded = false;
        } else {
            if !quoted && current.is_empty() && ch == '-' {
                excluded = true;
            }
            current.push(ch);
            // A leading exclusion does not make a following quoted term an operator.
            if current != "-" {
                started = true;
            }
        }
        if tokens.len() > TERMS_MAX {
            return Err(SearchError::TooManyTerms);
        }
    }
    if quoted || escaped {
        return Err(SearchError::InvalidSyntax);
    }
    if started && current.is_empty() {
        return Err(SearchError::InvalidSyntax);
    }
    if !current.is_empty() {
        tokens.push(Token {
            text: current,
            literal,
            excluded,
        });
    }
    if tokens.len() > TERMS_MAX {
        return Err(SearchError::TooManyTerms);
    }
    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Importance;

    fn message() -> Message {
        Message {
            id: 7,
            account_id: 1,
            folder_id: 2,
            uid: 7,
            from_name: "A Sender".into(),
            from_addr: "alice@example.invalid".into(),
            reply_to: String::new(),
            to: "bob@example.invalid".into(),
            cc: "carol@example.invalid".into(),
            subject: "100%_done!".into(),
            preview: "header preview".into(),
            body: String::new(),
            date: String::new(),
            timestamp: utc_day("2026-10-10").unwrap(),
            unread: true,
            starred: false,
            keywords: vec![],
            has_attachment: true,
            message_id: "fictional-7".into(),
            references: String::new(),
            importance: Importance::Normal,
            due: 0,
        }
    }

    fn sql_match(db: &Connection, query: &SearchQuery, from: &str) -> SearchMatch {
        let predicate = query.sql_predicate(true).unwrap();
        // Read the tri-state expression, not just a WHERE count which could
        // conceal NULL-vs-false disagreement under a positive or negative term.
        let result: Option<bool> = db
            .query_row(
                &format!("SELECT {} FROM {from}", predicate.clause),
                rusqlite::params_from_iter(predicate.values.iter()),
                |row| row.get(0),
            )
            .unwrap();
        match result {
            Some(true) => SearchMatch::Match,
            Some(false) => SearchMatch::NoMatch,
            None => SearchMatch::Unknown,
        }
    }

    fn field_query(operator: &str, needle: &str, negated: bool) -> SearchQuery {
        let quoted = needle.replace('\\', "\\\\").replace('"', "\\\"");
        SearchQuery::parse(&format!(
            "{}{operator}\"{quoted}\"",
            if negated { "-" } else { "" }
        ))
        .unwrap()
    }

    struct TemporaryCache(std::path::PathBuf);

    impl TemporaryCache {
        fn new() -> Self {
            let mut nonce = [0_u8; 8];
            getrandom::getrandom(&mut nonce).unwrap();
            let directory = std::env::temp_dir().join(format!(
                "megamail-search-fixture-{}-{:016x}",
                std::process::id(),
                u64::from_ne_bytes(nonce)
            ));
            let mut builder = std::fs::DirBuilder::new();
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                builder.mode(0o700);
            }
            // create, not create_dir_all: never adopt an existing directory.
            builder.create(&directory).unwrap();
            Self(directory)
        }

        fn database_path(&self) -> std::path::PathBuf {
            self.0.join("synthetic.db")
        }
    }

    impl Drop for TemporaryCache {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn legacy_cache_loaded_headers_and_registered_sql_have_identical_nul_semantics() {
        let temporary = TemporaryCache::new();
        // Production initialization + production model loading, with no profile
        // path resolution. The separate connection edits only this fixture.
        let cache = crate::cache::Cache::open_at(&temporary.database_path()).unwrap();
        let db = Connection::open(temporary.database_path()).unwrap();
        register_sql_functions(&db).unwrap();
        db.execute_batch("INSERT INTO messages (account_id,folder_path,uid,from_name,from_addr,subject,date,ts,unread,starred,has_attachment) VALUES (1,'INBOX',7,'','','','',1791590400,1,0,1), (2,'INBOX',7,'','','other account','',1791590400,1,0,1), (1,'Other',7,'','','other folder','',1791590400,1,0,1);").unwrap();
        for (raw, needle, expected) in [
            ("before\0secret", "secret", true),
            ("sec\0ret", "secret", false),
            ("sec\0ret", "sec ret", true),
            ("sec\0\0ret", "sec ret", false),
            ("sec\0\0ret", "sec  ret", true),
            ("\0SeCrEt\0", "SECRET", true),
            ("before\\u0000secret", "\\u0000", true),
            ("before\0secret", "\\u0000", false),
            ("é\0秘密", "é 秘密", true),
            ("é\0秘密", "É 秘密", false),
            ("a\0%_!z", "%_!", true),
            ("a\0%_!z", "%X!", false),
            ("a\0x' OR 1=1 --", "x' OR 1=1 --", true),
        ] {
            db.execute("UPDATE messages SET from_name=?1,from_addr=?1,recipients=?1,cc=?1,subject=?1,preview=?1 WHERE account_id=1 AND folder_path='INBOX' AND uid=7", [raw]).unwrap();
            let loaded = cache.load_messages(1, "INBOX", 2);
            assert_eq!(loaded.len(), 1);
            assert_eq!(loaded[0].subject, raw.replace('\0', " "));
            let mut raw_model = loaded[0].clone();
            raw_model.from_name = raw.into();
            raw_model.from_addr = raw.into();
            raw_model.to = raw.into();
            raw_model.cc = raw.into();
            raw_model.subject = raw.into();
            raw_model.preview = raw.into();
            for operator in ["", "from:", "to:", "cc:", "subject:"] {
                for negated in [false, true] {
                    let query = field_query(operator, needle, negated);
                    let expected = if expected != negated {
                        SearchMatch::Match
                    } else {
                        SearchMatch::NoMatch
                    };
                    let actual = query.matches(&loaded[0], BodyText::NotLoaded);
                    assert_eq!(actual, expected, "{operator} negative={negated}");
                    assert_eq!(query.matches(&raw_model, BodyText::NotLoaded), actual);
                    assert_eq!(
                        sql_match(
                            &db,
                            &query,
                            "messages m WHERE m.account_id=1 AND m.folder_path='INBOX' AND m.uid=7"
                        ),
                        actual
                    );
                }
            }
        }
    }

    #[test]
    fn body_nuls_use_the_same_space_normalization_and_unknown_rules() {
        let db = Connection::open_in_memory().unwrap();
        register_sql_functions(&db).unwrap();
        db.execute_batch("CREATE TABLE bodies(body TEXT)").unwrap();
        for (raw, needle, expected) in [
            ("before\0secret", "secret", true),
            ("sec\0ret", "secret", false),
            ("sec\0ret", "sec ret", true),
            ("é\0秘密", "é 秘密", true),
            ("é\0秘密", "É 秘密", false),
            ("a\0%_!z", "%_!", true),
            ("before\\u0000secret", "\\u0000", true),
        ] {
            db.execute("INSERT INTO bodies VALUES (?1)", [raw]).unwrap();
            for negated in [false, true] {
                let query = field_query("body:", needle, negated);
                let actual = query.matches(&message(), BodyText::Available(raw));
                assert_eq!(
                    actual,
                    if expected != negated {
                        SearchMatch::Match
                    } else {
                        SearchMatch::NoMatch
                    }
                );
                assert_eq!(sql_match(&db, &query, "bodies b"), actual);
                assert_eq!(
                    query.matches(&message(), BodyText::Available(&raw.replace('\0', " "))),
                    actual
                );
                assert_eq!(
                    query.matches(&message(), BodyText::Excluded),
                    SearchMatch::Unknown
                );
                assert_eq!(
                    query.matches(&message(), BodyText::NotLoaded),
                    SearchMatch::Unknown
                );
            }
            db.execute("DELETE FROM bodies", []).unwrap();
        }
    }

    #[test]
    fn text_bounds_are_byte_exact_before_nul_normalization() {
        let db = Connection::open_in_memory().unwrap();
        register_sql_functions(&db).unwrap();
        db.execute_batch("CREATE TABLE messages(subject TEXT); CREATE TABLE bodies(body TEXT);")
            .unwrap();
        for limit in [HEADER_BYTES_MAX, BODY_BYTES_MAX] {
            // Includes NUL and multi-byte UTF-8: exactly `limit` bytes, then
            // `limit + 1`. Never truncate at NUL or at a UTF-8 boundary.
            let mut value = "x".repeat(limit - "\0秘密".len());
            value.push_str("\0秘密");
            for oversize in [false, true] {
                if oversize {
                    value.push('x');
                }
                db.execute("INSERT INTO messages VALUES (?1)", [&value])
                    .unwrap();
                db.execute("INSERT INTO bodies VALUES (?1)", [&value])
                    .unwrap();
                let mut m = message();
                m.subject = value.clone();
                m.scrub_nuls();
                for negated in [false, true] {
                    for (operator, from, body) in [
                        ("subject:", "messages m", BodyText::NotLoaded),
                        ("body:", "bodies b", BodyText::Available(value.as_str())),
                    ] {
                        let query = field_query(operator, " 秘密", negated);
                        let actual = query.matches(&m, body);
                        assert_eq!(
                            actual,
                            if oversize {
                                SearchMatch::Unknown
                            } else if negated {
                                SearchMatch::NoMatch
                            } else {
                                SearchMatch::Match
                            }
                        );
                        assert_eq!(sql_match(&db, &query, from), actual);
                    }
                }
                db.execute_batch("DELETE FROM messages; DELETE FROM bodies;")
                    .unwrap();
            }
        }
    }

    #[test]
    fn invalid_or_absent_sql_text_is_unknown_including_under_negation() {
        let db = Connection::open_in_memory().unwrap();
        register_sql_functions(&db).unwrap();
        db.execute_batch("CREATE TABLE messages(subject); CREATE TABLE bodies(body);")
            .unwrap();
        for expression in ["NULL", "CAST(X'80FF' AS TEXT)", "X'736563726574'", "42"] {
            // Only fixed fixture expressions, never interpolated search values.
            db.execute_batch(&format!("INSERT INTO messages VALUES ({expression}); INSERT INTO bodies VALUES ({expression});")).unwrap();
            for operator in ["subject:", "body:"] {
                for negated in [false, true] {
                    let query = field_query(operator, "secret", negated);
                    assert_eq!(
                        sql_match(
                            &db,
                            &query,
                            if operator == "body:" {
                                "bodies b"
                            } else {
                                "messages m"
                            }
                        ),
                        SearchMatch::Unknown
                    );
                }
            }
            db.execute_batch("DELETE FROM messages; DELETE FROM bodies;")
                .unwrap();
        }
        // A Rust String cannot represent invalid UTF-8; the body eligibility
        // adapter must supply NotLoaded/Excluded, not lossy-decoded replacement.
        assert_eq!(
            SearchQuery::parse("-body:secret")
                .unwrap()
                .matches(&message(), BodyText::Excluded),
            SearchMatch::Unknown
        );
    }

    #[test]
    fn header_alternatives_preserve_sql_three_valued_logic() {
        let db = Connection::open_in_memory().unwrap();
        register_sql_functions(&db).unwrap();
        db.execute_batch("CREATE TABLE messages(from_name TEXT,from_addr TEXT,subject TEXT,preview TEXT,starred INTEGER);").unwrap();
        for known_hit in [false, true] {
            let mut m = message();
            m.from_name = "x".repeat(HEADER_BYTES_MAX + 1);
            m.from_addr = if known_hit { "secret" } else { "other" }.into();
            m.subject = "other".into();
            m.preview = "other".into();
            db.execute(
                "INSERT INTO messages VALUES (?1,?2,?3,?4,0)",
                rusqlite::params![m.from_name, m.from_addr, m.subject, m.preview],
            )
            .unwrap();
            for input in [
                "secret",
                "-secret",
                "from:secret",
                "-from:secret",
                "from:secret starred:true",
            ] {
                let query = SearchQuery::parse(input).unwrap();
                let expected = if input.ends_with("starred:true") {
                    SearchMatch::NoMatch
                } else if !known_hit {
                    SearchMatch::Unknown
                } else if input.starts_with('-') {
                    SearchMatch::NoMatch
                } else {
                    SearchMatch::Match
                };
                assert_eq!(query.matches(&m, BodyText::NotLoaded), expected);
                assert_eq!(sql_match(&db, &query, "messages m"), expected);
            }
            db.execute("DELETE FROM messages", []).unwrap();
        }
    }

    #[test]
    fn sql_text_functions_are_explicit_connection_local_and_not_like_dependent() {
        let query = SearchQuery::parse("subject:secret").unwrap();
        let predicate = query.sql_predicate(false).unwrap();
        assert_eq!(predicate.values, vec![Value::Text("secret".into())]);
        assert!(!predicate.clause.contains("LIKE"));
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch(
            "CREATE TABLE messages(subject TEXT); INSERT INTO messages VALUES ('SECRET');",
        )
        .unwrap();
        assert!(db
            .prepare(&format!("SELECT {} FROM messages m", predicate.clause))
            .is_err());
        register_sql_functions(&db).unwrap();
        register_sql_functions(&db).unwrap(); // Repeat registration is supported.
        for case_sensitive in [false, true] {
            db.pragma_update(None, "case_sensitive_like", case_sensitive)
                .unwrap();
            assert_eq!(sql_match(&db, &query, "messages m"), SearchMatch::Match);
        }
        let other = Connection::open_in_memory().unwrap();
        assert!(other
            .prepare("SELECT megamail_search_header_contains('a','a')")
            .is_err());
        assert!(db
            .query_row(
                "SELECT megamail_search_header_contains('a', ?1)",
                ["x".repeat(QUERY_BYTES_MAX + 1)],
                |r| r.get::<_, Option<bool>>(0)
            )
            .is_err());
        assert!(db
            .query_row("SELECT megamail_search_header_contains('a', '')", [], |r| r
                .get::<_, Option<bool>>(0))
            .is_err());
    }

    #[test]
    fn operators_evaluate_existing_message_fields() {
        let query = SearchQuery::parse("from:ALICE to:bob cc:carol subject:100%_done! date:2026-10-10 unread:true starred:false has:attachment").unwrap();
        assert_eq!(
            query.matches(&message(), BodyText::NotLoaded),
            SearchMatch::Match
        );
    }

    #[test]
    fn absent_body_is_unknown_even_for_exclusion() {
        let query = SearchQuery::parse("-body:secret").unwrap();
        assert_eq!(
            query.matches(&message(), BodyText::Excluded),
            SearchMatch::Unknown
        );
    }

    #[test]
    fn body_never_falls_back_to_preview() {
        assert_eq!(
            SearchQuery::parse("body:preview")
                .unwrap()
                .matches(&message(), BodyText::Available("actual text")),
            SearchMatch::NoMatch
        );
    }

    #[test]
    fn quoted_phrase_and_negative_term_are_conjunctive() {
        assert_eq!(
            SearchQuery::parse("from:\"a sender\" -subject:other")
                .unwrap()
                .matches(&message(), BodyText::NotLoaded),
            SearchMatch::Match
        );
    }

    #[test]
    fn unsupported_or_bad_syntax_is_rejected() {
        for input in [
            "x OR y",
            "(x)",
            "from:",
            "subject:\"open",
            "body:foo\\q",
            "is:banana",
            "date:2026-02-30",
            "unread:yes",
            "xyz:value",
        ] {
            assert!(SearchQuery::parse(input).is_err(), "{input}");
        }
    }

    #[test]
    fn limits_are_rejections_not_silent_truncation() {
        assert_eq!(
            SearchQuery::parse(&"x".repeat(QUERY_BYTES_MAX + 1)),
            Err(SearchError::TooLong)
        );
        assert_eq!(
            SearchQuery::parse(&vec!["x"; TERMS_MAX + 1].join(" ")),
            Err(SearchError::TooManyTerms)
        );
    }

    #[test]
    fn local_scope_does_not_claim_provider_history() {
        let q = SearchQuery::parse("x").unwrap();
        assert_eq!(
            q.validate_scope(
                SearchScope::ProviderHistory,
                SearchCapabilities {
                    all_local_indexed: true,
                    ..Default::default()
                }
            ),
            Err(SearchError::UnsupportedScope)
        );
    }

    #[test]
    fn loaded_headers_reject_body_queries() {
        assert_eq!(
            SearchQuery::parse("body:x").unwrap().validate_scope(
                SearchScope::LoadedHeaders,
                SearchCapabilities {
                    body: true,
                    ..Default::default()
                }
            ),
            Err(SearchError::UnsupportedBody)
        );
    }

    #[test]
    fn saved_query_rejects_unscoped_folder_and_unavailable_backend() {
        let record = SavedQueryRecord {
            version: 1,
            name: "Work".into(),
            query: "from:alice".into(),
            scope: SearchScope::LoadedHeaders,
            account_id: None,
            folder_path: Some("INBOX".into()),
        };
        assert!(SavedQuery::validate(record, SearchCapabilities::default()).is_err());
    }

    #[test]
    fn literal_quotes_do_not_turn_words_into_operators_or_exclusions() {
        let mut m = message();
        m.subject = "OR -private subject:x".into();
        for input in ["\"OR\"", "\"-private\"", "\"subject:x\""] {
            assert_eq!(
                SearchQuery::parse(input)
                    .unwrap()
                    .matches(&m, BodyText::NotLoaded),
                SearchMatch::Match,
                "{input}"
            );
        }
        for input in ["\"\"", "x or y", "-\"\"", "\"line\nfeed\""] {
            assert!(SearchQuery::parse(input).is_err(), "{input:?}");
        }
    }

    #[test]
    fn unknown_date_remains_unknown_when_negated() {
        let mut m = message();
        m.timestamp = 0;
        assert_eq!(
            SearchQuery::parse("-date:2026-10-10")
                .unwrap()
                .matches(&m, BodyText::NotLoaded),
            SearchMatch::Unknown
        );
    }

    #[test]
    fn known_nonmatch_dominates_unknown_body() {
        assert_eq!(
            SearchQuery::parse("starred:true body:missing")
                .unwrap()
                .matches(&message(), BodyText::NotLoaded),
            SearchMatch::NoMatch
        );
    }

    #[test]
    fn utc_before_after_bounds_are_not_inclusive_date_aliases() {
        assert_eq!(
            SearchQuery::parse("after:2026-10-09 before:2026-10-11")
                .unwrap()
                .matches(&message(), BodyText::NotLoaded),
            SearchMatch::Match
        );
        assert_eq!(
            SearchQuery::parse("after:2026-10-10")
                .unwrap()
                .matches(&message(), BodyText::NotLoaded),
            SearchMatch::NoMatch
        );
    }

    #[test]
    fn oversize_body_stays_unknown_in_matcher_and_sql() {
        let body = "x".repeat(BODY_BYTES_MAX + 1);
        let q = SearchQuery::parse("-body:secret").unwrap();
        assert_eq!(
            q.matches(&message(), BodyText::Available(&body)),
            SearchMatch::Unknown
        );
        let db = rusqlite::Connection::open_in_memory().unwrap();
        register_sql_functions(&db).unwrap();
        db.execute_batch("CREATE TABLE bodies(body TEXT);").unwrap();
        db.execute("INSERT INTO bodies VALUES (?1)", [&body])
            .unwrap();
        let predicate = q.sql_predicate(true).unwrap();
        let count: i64 = db
            .query_row(
                &format!("SELECT count(*) FROM bodies b WHERE {}", predicate.clause),
                rusqlite::params_from_iter(predicate.values.iter()),
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn saved_query_is_validated_on_the_execution_capabilities() {
        let record = SavedQueryRecord {
            version: 1,
            name: "Work".into(),
            query: "body:project".into(),
            scope: SearchScope::AllLocalIndexed,
            account_id: Some(1),
            folder_path: Some("INBOX".into()),
        };
        assert!(SavedQuery::validate(
            record.clone(),
            SearchCapabilities {
                all_local_indexed: true,
                body: true,
                ..Default::default()
            }
        )
        .is_ok());
        assert!(SavedQuery::validate(record, SearchCapabilities::default()).is_err());
    }

    #[test]
    fn parameterized_sql_and_matcher_agree_for_headers_and_missing_bodies() {
        // Fictional in-memory schema only. The same public predicate will feed
        // production cache queries, rather than a separately implemented parser.
        let db = rusqlite::Connection::open_in_memory().unwrap();
        register_sql_functions(&db).unwrap();
        db.execute_batch("CREATE TABLE messages (from_name TEXT, from_addr TEXT, recipients TEXT, cc TEXT, subject TEXT, preview TEXT, ts INTEGER, unread INTEGER, starred INTEGER, has_attachment INTEGER); CREATE TABLE bodies (body TEXT); INSERT INTO messages VALUES ('A Sender','alice@example.invalid','bob@example.invalid','carol@example.invalid','100%_done!','header preview',1791590400,1,0,1); INSERT INTO bodies VALUES (NULL);").unwrap();
        for input in [
            "from:ALICE to:bob cc:carol",
            "subject:100%_done!",
            "subject:100Zdone",
            "date:2026-10-10",
            "-date:2026-10-09",
            "unread:true -starred:true attachments:true",
            "-body:secret",
        ] {
            let query = SearchQuery::parse(input).unwrap();
            let predicate = query.sql_predicate(true).unwrap();
            let count: i64 = db
                .query_row(
                    &format!(
                        "SELECT count(*) FROM messages m CROSS JOIN bodies b WHERE {}",
                        predicate.clause
                    ),
                    rusqlite::params_from_iter(predicate.values.iter()),
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(
                count != 0,
                query.matches(&message(), BodyText::NotLoaded) == SearchMatch::Match,
                "{input}"
            );
        }
    }
}

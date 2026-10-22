//! Inline recipient completion for To/Cc/Bcc. Pure over in-memory data: saved
//! local contacts first, then people already visible in the loaded mail list.
//! Nothing is persisted or fetched; accepting a suggestion is an explicit click.
use crate::contacts::ContactSnapshot;
use megamail_core::worker;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Suggestion {
    pub name: String,
    pub email: String,
}

/// Split a recipient field into the finished part and the token being typed.
pub(crate) fn split_token(field: &str) -> (&str, &str) {
    match field.rfind(',') {
        Some(i) => (&field[..=i], field[i + 1..].trim_start()),
        None => ("", field.trim_start()),
    }
}

fn matches(query: &str, name: &str, email: &str) -> bool {
    email.to_lowercase().starts_with(query)
        || name
            .to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .any(|word| !word.is_empty() && word.starts_with(query))
        || name.to_lowercase().starts_with(query)
}

/// Up to `limit` matches for the token at the end of `field`. Addresses already
/// present in any of `taken` are skipped.
pub(crate) fn suggest(
    field: &str,
    taken: &[&str],
    book: Option<&ContactSnapshot>,
    seen: &[(String, String)],
    limit: usize,
) -> Vec<Suggestion> {
    let (_, token) = split_token(field);
    let query = token.trim().to_lowercase();
    if query.len() < 2 || query.contains(['<', '>', '"']) {
        return Vec::new();
    }
    let used: Vec<String> = taken
        .iter()
        .flat_map(|f| worker::parse_recipients(f))
        .map(|(_, e)| e.to_lowercase())
        .collect();
    let mut out: Vec<Suggestion> = Vec::new();
    let candidates = book
        .into_iter()
        .flat_map(|b| &b.contacts)
        .flat_map(|c| {
            c.fields
                .emails
                .iter()
                .map(|e| (c.fields.display_name.clone(), e.clone()))
        })
        .chain(seen.iter().cloned());
    for (name, email) in candidates {
        let lower = email.to_lowercase();
        if matches(&query, &name, &email)
            && !used.contains(&lower)
            && !out.iter().any(|s| s.email.eq_ignore_ascii_case(&email))
        {
            out.push(Suggestion { name, email });
            if out.len() >= limit {
                break;
            }
        }
    }
    out
}

/// Replace the token being typed with the chosen recipient and leave a
/// separator so the next address can follow immediately.
pub(crate) fn accept(field: &str, choice: &Suggestion) -> String {
    let (done, _) = split_token(field);
    let lead = if done.is_empty() { "" } else { " " };
    format!(
        "{done}{lead}{}, ",
        worker::format_recipient(&choice.name, &choice.email)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completes_by_word_and_skips_used() {
        let seen = vec![
            ("Daniel Elmaliah".to_owned(), "dan@x.com".to_owned()),
            ("Ada".to_owned(), "ada@x.com".to_owned()),
        ];
        let got = suggest("ada@x.com, elm", &["ada@x.com"], None, &seen, 5);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].email, "dan@x.com");
        assert!(suggest("a", &[], None, &seen, 5).is_empty());
        assert_eq!(
            accept("ada@x.com, elm", &got[0]),
            "ada@x.com, Daniel Elmaliah <dan@x.com>, "
        );
    }
}

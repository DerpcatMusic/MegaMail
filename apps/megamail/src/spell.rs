//! Opt-in spell check for the composer: pipes the draft through the system
//! `hunspell -a` on a background thread. No dictionary or hunspell means an
//! explanatory message, never a silent pass. Nothing leaves the machine.
use std::io::Write;
use std::process::{Command, Stdio};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Misspelling {
    pub word: String,
    pub suggestions: Vec<String>,
}

/// Parse ispell-pipe output: `& word n offset: a, b` and `# word offset` lines.
pub(crate) fn parse(output: &str) -> Vec<Misspelling> {
    let mut found: Vec<Misspelling> = Vec::new();
    for line in output.lines() {
        let (word, suggestions) = if let Some(rest) = line.strip_prefix("& ") {
            let (head, tail) = rest.split_once(": ").unwrap_or((rest, ""));
            let word = head.split_whitespace().next().unwrap_or("");
            (word, tail.split(", ").map(str::to_owned).collect::<Vec<_>>())
        } else if let Some(rest) = line.strip_prefix("# ") {
            (rest.split_whitespace().next().unwrap_or(""), Vec::new())
        } else {
            continue;
        };
        if !word.is_empty() && !found.iter().any(|m| m.word == word) {
            found.push(Misspelling {
                word: word.to_owned(),
                suggestions: suggestions.into_iter().filter(|s| !s.is_empty()).take(4).collect(),
            });
        }
    }
    found
}

pub(crate) fn check(text: &str) -> Result<Vec<Misspelling>, String> {
    const MISSING: &str =
        "No spelling dictionary was found. Install hunspell and a dictionary (for example hunspell-en_us).";
    let mut child = Command::new("hunspell")
        .arg("-a")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| MISSING.to_owned())?;
    // A leading ^ keeps a line from being read as an ispell command.
    let input: String = text.lines().map(|l| format!("^{l}\n")).collect();
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(input.as_bytes());
    }
    let output = child.wait_with_output().map_err(|_| MISSING.to_owned())?;
    if String::from_utf8_lossy(&output.stderr).contains("Can't open affix") {
        return Err(MISSING.to_owned());
    }
    Ok(parse(&String::from_utf8_lossy(&output.stdout)))
}

/// Replace whole-word occurrences of `word`.
pub(crate) fn replace_word(text: &str, word: &str, with: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find(word) {
        let before = rest[..at].chars().next_back();
        let after = rest[at + word.len()..].chars().next();
        let boundary = |c: Option<char>| c.is_none_or(|c| !c.is_alphanumeric() && c != '\'');
        out.push_str(&rest[..at]);
        out.push_str(if boundary(before) && boundary(after) { with } else { word });
        rest = &rest[at + word.len()..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_pipe_output_and_replaces_whole_words() {
        let out = "@(#) International Ispell\n*\n& helo 3 1: hello, help, helot\n# wrold 6\n*\n& helo 3 9: hello\n";
        let found = parse(out);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].suggestions[0], "hello");
        assert!(found[1].suggestions.is_empty());
        assert_eq!(replace_word("helo, helos and helo", "helo", "hello"), "hello, helos and hello");
    }
}

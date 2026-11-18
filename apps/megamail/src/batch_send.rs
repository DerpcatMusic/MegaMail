//! "Send individually": one message per recipient with `{name}`,
//! `{first_name}` and `{email}` merged into subject and body.

use megamail_core::worker::{self, OutgoingMessage};

/// Recipients from a To field, accepting a pasted list separated by
/// newlines, tabs or semicolons as well as commas.
pub(crate) fn recipients(to: &str) -> Vec<(String, String)> {
    let normalized: String = to
        .chars()
        .map(|c| if matches!(c, '\n' | '\r' | '\t' | ';') { ',' } else { c })
        .collect();
    let mut seen = std::collections::HashSet::new();
    worker::parse_recipients(&normalized)
        .into_iter()
        .filter(|(_, email)| email.contains('@') && seen.insert(email.to_ascii_lowercase()))
        .collect()
}

fn merge(template: &str, name: &str, email: &str) -> String {
    let first = name.split_whitespace().next().unwrap_or("there");
    let name = if name.trim().is_empty() { "there" } else { name.trim() };
    template
        .replace("{first_name}", first)
        .replace("{name}", name)
        .replace("{email}", email)
}

/// One personalized copy per recipient. Cc/Bcc are dropped: each person
/// gets a private message. Message-IDs stay unique by suffixing the index.
pub(crate) fn split(message: &OutgoingMessage) -> Vec<OutgoingMessage> {
    recipients(&message.to)
        .into_iter()
        .enumerate()
        .map(|(index, (name, email))| {
            let mut copy = message.clone();
            copy.to = if name.is_empty() { email.clone() } else { format!("\"{}\" <{email}>", name.replace('"', "")) };
            copy.cc.clear();
            copy.bcc.clear();
            copy.subject = merge(&message.subject, &name, &email);
            copy.body = merge(&message.body, &name, &email);
            copy.html = merge(&message.html, &name, &email);
            copy.client_message_id = message.client_message_id.as_ref().map(|id| match id.split_once('@') {
                Some((local, domain)) => format!("{local}-{index}@{domain}"),
                None => format!("{id}-{index}"),
            });
            copy
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pasted_list_splits_and_merges_names() {
        let message = OutgoingMessage {
            to: "Ada Lovelace <ada@x.org>\nbob@y.org; ada@x.org".into(),
            cc: "boss@z.org".into(),
            subject: "Hi {first_name}".into(),
            body: "Dear {name}, sent to {email}".into(),
            client_message_id: Some("megamail-t@x.org".into()),
            from_account_id: 1,
            from_alias: None,
            bcc: String::new(),
            reply_to: String::new(),
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
        };
        let out = split(&message);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].subject, "Hi Ada");
        assert_eq!(out[0].body, "Dear Ada Lovelace, sent to ada@x.org");
        assert_eq!(out[1].subject, "Hi there");
        assert!(out[1].cc.is_empty());
        assert_eq!(out[1].client_message_id.as_deref(), Some("megamail-t-1@x.org"));
    }
}

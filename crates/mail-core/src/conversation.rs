//! Pure grouping of the messages already loaded for a mailbox snapshot.

use std::collections::{HashMap, HashSet};

use crate::models::Message;

/// One conversation assembled from the currently loaded messages.
///
/// `latest_index` and `latest_sent_index` refer into `members`, avoiding a
/// second copy of each full message (including its body).
#[derive(Debug, Clone, PartialEq)]
pub struct ConversationSummary {
    /// Account-scoped anchor. Wire IDs in this key are normalized for matching.
    pub key: (u32, String),
    /// Members in chronological order, with repeated physical and Gmail-label
    /// copies removed.
    pub members: Vec<Message>,
    pub count: usize,
    /// Number of unread members, including a cross-folder unread copy once.
    pub unread: usize,
    pub has_attachment: bool,
    pub latest_index: usize,
    /// The newest member filed in a known Sent folder, if any.
    pub latest_sent_index: Option<usize>,
    /// True when an answered keyword is present or a Sent member references
    /// another loaded member.
    pub replied: bool,
    /// True when a member's keyword list contains `$Forwarded`.
    pub forwarded: bool,
}

/// Group a bounded set of loaded messages by Message-ID and References.
///
/// `sent_folders` contains `(account_id, folder_id)` pairs. The function never
/// groups by subject, and it keeps each message's original header spelling.
pub fn group_conversations(
    messages: impl IntoIterator<Item = Message>,
    sent_folders: &HashSet<(u32, u32)>,
) -> Vec<ConversationSummary> {
    let messages = dedupe_messages(messages, sent_folders);
    if messages.is_empty() {
        return Vec::new();
    }

    let mut unique_ids = HashMap::<(u32, String), usize>::new();
    let mut ambiguous_ids = HashSet::<(u32, String)>::new();
    for (index, message) in messages.iter().enumerate() {
        let Some(id) = normalized_id(&message.message_id) else {
            continue;
        };
        let key = (message.account_id, id);
        if unique_ids.remove(&key).is_some() {
            ambiguous_ids.insert(key);
        } else if !ambiguous_ids.contains(&key) {
            unique_ids.insert(key, index);
        }
    }

    let mut sets = DisjointSet::new(messages.len());
    let mut first_missing_reference = HashMap::<(u32, String), usize>::new();
    for (index, message) in messages.iter().enumerate() {
        for reference in reference_ids(&message.references) {
            let key = (message.account_id, reference);
            if let Some(parent) = unique_ids.get(&key) {
                sets.union(index, *parent);
            } else if !ambiguous_ids.contains(&key) {
                if let Some(first) = first_missing_reference.get(&key) {
                    sets.union(index, *first);
                } else {
                    first_missing_reference.insert(key, index);
                }
            }
        }
    }

    let mut components = HashMap::<usize, Vec<Message>>::new();
    for (index, message) in messages.into_iter().enumerate() {
        let root = sets.find(index);
        components.entry(root).or_default().push(message);
    }

    let mut summaries = components
        .into_values()
        .map(|mut members| {
            members.sort_by(message_chronology);
            let account_id = members[0].account_id;
            let latest_index = members.len() - 1;
            let latest_sent_index = members.iter().rposition(|message| {
                sent_folders.contains(&(message.account_id, message.folder_id))
            });
            let member_ids = unique_member_ids(&members);
            let replied = members.iter().enumerate().any(|(index, message)| {
                message.has_keyword("$Answered")
                    || message.has_keyword(r"\Answered")
                    || (sent_folders.contains(&(message.account_id, message.folder_id))
                        && reference_ids(&message.references).iter().any(|reference| {
                            member_ids
                                .get(reference)
                                .is_some_and(|member_index| *member_index != index)
                        }))
            });
            let key = (account_id, conversation_anchor(&members, &ambiguous_ids));
            let count = members.len();
            let unread = members.iter().filter(|message| message.unread).count();
            let has_attachment = members.iter().any(|message| message.has_attachment);
            let forwarded = members
                .iter()
                .any(|message| message.has_keyword("$Forwarded"));
            ConversationSummary {
                key,
                members,
                count,
                unread,
                has_attachment,
                latest_index,
                latest_sent_index,
                replied,
                forwarded,
            }
        })
        .collect::<Vec<_>>();

    summaries.sort_by(|a, b| {
        let a_latest = &a.members[a.latest_index];
        let b_latest = &b.members[b.latest_index];
        b_latest
            .timestamp
            .cmp(&a_latest.timestamp)
            .then_with(|| a.key.cmp(&b.key))
    });
    summaries
}

fn dedupe_messages(
    messages: impl IntoIterator<Item = Message>,
    sent_folders: &HashSet<(u32, u32)>,
) -> Vec<Message> {
    let mut messages = messages.into_iter().collect::<Vec<_>>();
    messages.sort_by(message_stable_order);
    messages.dedup_by(|a, b| location_key(a) == location_key(b));

    let mut out = Vec::<Message>::with_capacity(messages.len());
    let mut copies = HashMap::<MailIdentity, usize>::new();
    for message in messages {
        let Some(id) = normalized_id(&message.message_id) else {
            out.push(message);
            continue;
        };
        let identity = MailIdentity {
            account_id: message.account_id,
            message_id: id,
            from_addr: message.from_addr.clone(),
            timestamp: message.timestamp,
        };
        if let Some(index) = copies.get(&identity).copied() {
            let existing = &mut out[index];
            let unread = existing.unread || message.unread;
            let starred = existing.starred || message.starred;
            let has_attachment = existing.has_attachment || message.has_attachment;
            if prefer_copy(&message, existing, sent_folders) {
                out[index] = message;
            }
            out[index].unread = unread;
            out[index].starred = starred;
            out[index].has_attachment = has_attachment;
        } else {
            copies.insert(identity, out.len());
            out.push(message);
        }
    }
    out
}

fn prefer_copy(a: &Message, b: &Message, sent_folders: &HashSet<(u32, u32)>) -> bool {
    let a_sent = sent_folders.contains(&(a.account_id, a.folder_id));
    let b_sent = sent_folders.contains(&(b.account_id, b.folder_id));
    (a_sent && !b_sent) || (a_sent == b_sent && location_key(a) < location_key(b))
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct MailIdentity {
    account_id: u32,
    message_id: String,
    from_addr: String,
    timestamp: i64,
}

fn location_key(message: &Message) -> (u32, u32, u32) {
    (message.account_id, message.folder_id, message.uid)
}

fn message_stable_order(a: &Message, b: &Message) -> std::cmp::Ordering {
    location_key(a)
        .cmp(&location_key(b))
        .then_with(|| a.message_id.cmp(&b.message_id))
        .then_with(|| a.from_addr.cmp(&b.from_addr))
        .then_with(|| a.timestamp.cmp(&b.timestamp))
        .then_with(|| a.subject.cmp(&b.subject))
        .then_with(|| a.preview.cmp(&b.preview))
        .then_with(|| a.body.cmp(&b.body))
        .then_with(|| a.unread.cmp(&b.unread))
        .then_with(|| a.starred.cmp(&b.starred))
        .then_with(|| a.has_attachment.cmp(&b.has_attachment))
}

fn message_chronology(a: &Message, b: &Message) -> std::cmp::Ordering {
    a.timestamp
        .cmp(&b.timestamp)
        .then_with(|| location_key(a).cmp(&location_key(b)))
        .then_with(|| normalized_id(&a.message_id).cmp(&normalized_id(&b.message_id)))
}

fn normalized_id(id: &str) -> Option<String> {
    let id = id.trim().trim_start_matches('<').trim_end_matches('>');
    (!id.is_empty()).then(|| id.to_ascii_lowercase())
}

// ponytail: This mirrors the cache's 24-reference lookup cap; raise both if the supported thread depth changes.
const REFERENCE_ID_LIMIT: usize = 24;

fn reference_ids(references: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    references
        .split_whitespace()
        .take(REFERENCE_ID_LIMIT)
        .filter_map(normalized_id)
        .filter(|id| seen.insert(id.clone()))
        .collect()
}

fn unique_member_ids(members: &[Message]) -> HashMap<String, usize> {
    let mut ids = HashMap::<String, usize>::new();
    let mut ambiguous = HashSet::<String>::new();
    for (index, member) in members.iter().enumerate() {
        let Some(id) = normalized_id(&member.message_id) else {
            continue;
        };
        if ids.remove(&id).is_some() {
            ambiguous.insert(id);
        } else if !ambiguous.contains(&id) {
            ids.insert(id, index);
        }
    }
    ids
}

fn conversation_anchor(members: &[Message], ambiguous_ids: &HashSet<(u32, String)>) -> String {
    let first_reference = members.iter().find_map(|member| {
        reference_ids(&member.references)
            .into_iter()
            .next()
            .map(|id| (member.account_id, id))
    });
    if let Some((account_id, id)) = first_reference {
        if ambiguous_ids.contains(&(account_id, id.clone())) {
            return format!("id:{id}:{}:{}", members[0].folder_id, members[0].uid);
        }
        return format!("id:{id}");
    }
    if let Some(id) = normalized_id(&members[0].message_id) {
        if ambiguous_ids.contains(&(members[0].account_id, id.clone())) {
            return format!("id:{id}:{}:{}", members[0].folder_id, members[0].uid);
        }
        return format!("id:{id}");
    }
    format!("folder:{}:uid:{}", members[0].folder_id, members[0].uid)
}

struct DisjointSet {
    parent: Vec<usize>,
    size: Vec<usize>,
}

impl DisjointSet {
    fn new(count: usize) -> Self {
        Self {
            parent: (0..count).collect(),
            size: vec![1; count],
        }
    }

    fn find(&mut self, mut item: usize) -> usize {
        let mut root = item;
        while self.parent[root] != root {
            root = self.parent[root];
        }
        while self.parent[item] != item {
            let next = self.parent[item];
            self.parent[item] = root;
            item = next;
        }
        root
    }

    fn union(&mut self, a: usize, b: usize) {
        let mut a = self.find(a);
        let mut b = self.find(b);
        if a == b {
            return;
        }
        if self.size[a] < self.size[b] {
            std::mem::swap(&mut a, &mut b);
        }
        self.parent[b] = a;
        self.size[a] += self.size[b];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(
        account_id: u32,
        folder_id: u32,
        uid: u32,
        message_id: &str,
        references: &str,
        timestamp: i64,
    ) -> Message {
        Message {
            id: uid,
            account_id,
            folder_id,
            uid,
            from_name: String::new(),
            from_addr: "sender@example.test".to_string(),
            reply_to: String::new(),
            to: String::new(),
            cc: String::new(),
            subject: "same subject".to_string(),
            preview: String::new(),
            body: String::new(),
            date: String::new(),
            timestamp,
            unread: false,
            starred: false,
            keywords: Vec::new(),
            has_attachment: false,
            message_id: message_id.to_string(),
            references: references.to_string(),
            importance: Default::default(),
            due: 0,
        }
    }

    fn sent_folder() -> HashSet<(u32, u32)> {
        HashSet::from([(1, 9)])
    }

    #[test]
    fn groups_reference_chains_in_chronological_order() {
        let root = message(1, 1, 1, "root@x", "", 10);
        let mut second = message(1, 1, 2, "RePlY@X", "<ROOT@X>", 20);
        second.has_attachment = true;
        let mut third = message(1, 1, 3, "third@x", "<ROOT@X> <reply@x>", 30);
        third.unread = true;
        let rows = vec![third, root.clone(), second];

        let initial = group_conversations(vec![root], &HashSet::new());
        let conversations = group_conversations(rows, &HashSet::new());
        assert_eq!(conversations.len(), 1);
        let conversation = &conversations[0];
        assert_eq!(conversation.count, 3);
        assert_eq!(conversation.key, (1, "id:root@x".to_string()));
        assert_eq!(
            conversation.key, initial[0].key,
            "adding members keeps the row key"
        );
        assert_eq!(conversation.unread, 1);
        assert!(conversation.has_attachment);
        assert_eq!(
            conversation
                .members
                .iter()
                .map(|member| member.message_id.as_str())
                .collect::<Vec<_>>(),
            vec!["root@x", "RePlY@X", "third@x"]
        );
        assert_eq!(conversation.latest_index, 2);
    }

    #[test]
    fn leaves_anonymous_mail_alone_and_scopes_ids_to_the_account() {
        let anonymous = message(1, 1, 1, "", "", 10);
        let rows = vec![
            anonymous.clone(),
            anonymous,
            message(1, 1, 2, "", "", 20),
            message(1, 1, 3, "same@x", "", 30),
            message(2, 1, 3, "same@x", "", 30),
        ];

        let conversations = group_conversations(rows, &HashSet::new());
        assert_eq!(conversations.len(), 4);
        assert!(conversations
            .iter()
            .all(|conversation| conversation.count == 1));
        assert_eq!(
            conversations
                .iter()
                .filter(|conversation| conversation.key.1.starts_with("id:same@x"))
                .count(),
            2
        );
    }

    #[test]
    fn dedupes_gmail_copies_but_keeps_reused_ids_from_other_senders_separate() {
        let mut inbox_copy = message(1, 2, 12, "<same@x>", "", 10);
        inbox_copy.unread = true;
        let archive_copy = message(1, 4, 99, "same@x", "", 10);
        let mut reused_id = message(1, 3, 7, "same@x", "", 10);
        reused_id.from_addr = "other@example.test".to_string();

        let conversations =
            group_conversations(vec![archive_copy, reused_id, inbox_copy], &HashSet::new());
        assert_eq!(conversations.len(), 2);
        assert_eq!(conversations.iter().map(|c| c.count).sum::<usize>(), 2);
        let copy = conversations
            .iter()
            .find(|conversation| conversation.unread == 1)
            .unwrap();
        assert_eq!(copy.members[0].folder_id, 2);
    }

    #[test]
    fn references_to_a_reused_ambiguous_id_do_not_merge_unrelated_messages() {
        let mut first = message(1, 1, 1, "collision@x", "", 10);
        first.from_addr = "first@example.test".to_string();
        let mut second = message(1, 1, 2, "collision@x", "", 11);
        second.from_addr = "second@example.test".to_string();
        let reply_a = message(1, 1, 3, "reply-a@x", "collision@x", 20);
        let reply_b = message(1, 1, 4, "reply-b@x", "collision@x", 21);

        let conversations =
            group_conversations(vec![first, second, reply_a, reply_b], &HashSet::new());
        assert_eq!(conversations.len(), 4);
        assert!(conversations
            .iter()
            .all(|conversation| conversation.count == 1));
    }

    #[test]
    fn sent_reply_is_marked_only_when_it_references_a_loaded_member() {
        let mut reply = message(1, 9, 44, "reply@x", "<root@x>", 20);
        reply.from_addr = "me@example.test".to_string();
        let mut archive_copy = reply.clone();
        archive_copy.folder_id = 8;
        archive_copy.uid = 99;
        let conversations = group_conversations(
            vec![message(1, 1, 1, "root@x", "", 10), archive_copy, reply],
            &sent_folder(),
        );
        let conversation = &conversations[0];
        assert_eq!(conversation.count, 2, "the Sent label copy counts once");
        assert!(conversation.replied);
        assert_eq!(conversation.latest_sent_index, Some(1));
        assert_eq!(conversation.members[1].folder_id, 9);

        let mut orphan_reply = message(1, 9, 46, "orphan-reply@x", "missing-parent@x", 40);
        orphan_reply.from_addr = "me@example.test".to_string();
        let mut sibling = message(1, 1, 2, "sibling@x", "missing-parent@x", 35);
        sibling.subject = "unrelated subject text".to_string();
        let orphan_thread = group_conversations(vec![orphan_reply, sibling], &sent_folder());
        assert_eq!(orphan_thread.len(), 1);
        assert_eq!(orphan_thread[0].latest_sent_index, Some(1));
        assert!(!orphan_thread[0].replied);
    }

    #[test]
    fn forwarded_keyword_never_joins_a_message_by_subject() {
        let mut forward = message(1, 9, 45, "forward@x", "", 20);
        forward.from_addr = "me@example.test".to_string();
        forward.keywords.push("$fOrWaRdEd".to_string());
        let conversations = group_conversations(
            vec![message(1, 1, 1, "root@x", "", 10), forward],
            &sent_folder(),
        );

        assert_eq!(conversations.len(), 2);
        let forwarded = conversations
            .iter()
            .find(|conversation| conversation.forwarded)
            .unwrap();
        assert_eq!(forwarded.count, 1);
        assert!(!forwarded.replied);
        assert!(conversations
            .iter()
            .filter(|conversation| !conversation.forwarded)
            .all(|conversation| !conversation.replied));
    }

    #[test]
    fn answered_keywords_are_case_insensitive_and_account_scoped() {
        let mut dollar_answered = message(1, 1, 1, "same@x", "", 10);
        dollar_answered.keywords.push("$aNsWeReD".to_string());
        let mut system_answered = message(2, 1, 1, "same@x", "", 10);
        system_answered.keywords.push(r"\Answered".to_string());
        let unmarked = message(3, 1, 1, "same@x", "", 10);

        let conversations = group_conversations(
            vec![dollar_answered, system_answered, unmarked],
            &HashSet::new(),
        );
        assert_eq!(conversations.len(), 3);
        assert_eq!(
            conversations
                .iter()
                .map(|conversation| (conversation.key.0, conversation.replied))
                .collect::<Vec<_>>(),
            vec![(1, true), (2, true), (3, false)]
        );
    }

    #[test]
    fn cycles_and_missing_shared_ancestors_terminate_without_subject_matching() {
        let mut a = message(1, 1, 1, "a@x", "b@x", 10);
        a.subject = "one".to_string();
        let mut b = message(1, 1, 2, "b@x", "a@x", 20);
        b.subject = "different".to_string();
        let c = message(1, 1, 3, "c@x", "missing@x", 30);
        let d = message(1, 1, 4, "d@x", "missing@x", 40);

        let conversations = group_conversations(vec![a, b, c, d], &HashSet::new());
        assert_eq!(conversations.len(), 2);
        assert_eq!(conversations.iter().map(|c| c.count).sum::<usize>(), 4);
    }

    #[test]
    #[ignore = "synthetic grouping scalability benchmark"]
    fn groups_50k_headers_into_5k_threads_with_sent_and_inbox_members() {
        use std::time::Instant;

        const THREADS: usize = 5_000;
        const MESSAGES_PER_THREAD: usize = 10;
        let mut rows = Vec::with_capacity(THREADS * MESSAGES_PER_THREAD);
        let sent_folders = sent_folder();
        for thread in 0..THREADS {
            let root_id = format!("root-{thread}@x");
            for step in 0..MESSAGES_PER_THREAD {
                let message_id = if step == 0 {
                    root_id.clone()
                } else {
                    format!("reply-{thread}-{step}@x")
                };
                let references = if step == 0 {
                    String::new()
                } else if step == 1 {
                    root_id.clone()
                } else {
                    format!("{root_id} reply-{thread}-{}@x", step - 1)
                };
                let sent = step + 1 == MESSAGES_PER_THREAD;
                let folder_id = if sent { 9 } else { 1 };
                let uid = if sent {
                    thread as u32 + 1
                } else {
                    (thread * (MESSAGES_PER_THREAD - 1) + step + 1) as u32
                };
                let mut row = message(
                    1,
                    folder_id,
                    uid,
                    &message_id,
                    &references,
                    (thread * MESSAGES_PER_THREAD + step) as i64,
                );
                if step == 5 {
                    row.unread = true;
                }
                if step == 3 {
                    row.has_attachment = true;
                }
                if sent {
                    row.from_addr = "me@example.test".to_string();
                }
                rows.push(row);
            }
        }

        let started = Instant::now();
        let conversations = group_conversations(rows, &sent_folders);
        let elapsed = started.elapsed();

        assert_eq!(conversations.len(), THREADS);
        assert!(conversations
            .iter()
            .all(|conversation| conversation.count == MESSAGES_PER_THREAD));
        assert!(conversations
            .iter()
            .all(|conversation| conversation.replied));
        assert_eq!(
            conversations.iter().map(|c| c.unread).sum::<usize>(),
            THREADS
        );
        assert_eq!(
            conversations
                .iter()
                .filter(|conversation| conversation.has_attachment)
                .count(),
            THREADS
        );
        eprintln!(
            "grouped {} headers into {} conversations ({} Sent members) in {:.2?}",
            THREADS * MESSAGES_PER_THREAD,
            conversations.len(),
            THREADS,
            elapsed
        );
    }
}

//! Ctrl+K command palette: every mailbox command, searchable, with its key.
//! Commands are plain actions, so the palette, the keyboard and the toolbar
//! all run the same guarded handlers.

use gpui_kit::Action;

use crate::desktop_actions::action::*;

pub(crate) struct Command {
    pub label: &'static str,
    pub keys: &'static str,
    make: fn() -> Box<dyn Action>,
}

impl Command {
    pub(crate) fn action(&self) -> Box<dyn Action> {
        (self.make)()
    }
}

macro_rules! command {
    ($label:expr, $keys:expr, $action:ident) => {
        Command { label: $label, keys: $keys, make: || Box::new($action) }
    };
}

pub(crate) fn commands() -> Vec<Command> {
    vec![
        command!("Archive (Done)", "E", ArchiveMessage),
        command!("Move to Trash", "#", TrashMessage),
        command!("Star / unstar", "S", ToggleStar),
        command!("Mark read / unread", "U", ToggleUnread),
        command!("Reply", "R", ReplyMessage),
        command!("Reply all", "A", ReplyAll),
        command!("Forward", "F", ForwardMessage),
        command!("Compose new message", "C", ComposeMessage),
        command!("Search", "/", FocusSearch),
        command!("Next conversation", "J", NextMessage),
        command!("Previous conversation", "K", PreviousMessage),
        command!("Next split", "Tab", NextSplit),
        command!("Previous split", "Shift Tab", PreviousSplit),
        command!("Screener: allow sender", "Y", ScreenerAllow),
        command!("Screener: block sender", "N", ScreenerBlock),
        command!("Unsubscribe", "Ctrl U", Unsubscribe),
        command!("Undo send", "Z", UndoSend),
        command!("Sender's colors / theme colors", "Shift V", ToggleSenderColors),
        command!("Go to Inbox", "G I", GoInbox),
        command!("Go to Starred", "G S", GoStarred),
        command!("Go to Sent", "G T", GoSent),
        command!("Go to Drafts", "G D", GoDrafts),
        command!("Go to Archive", "G E", GoArchive),
        command!("Unified inbox", "Ctrl Shift L", UnifiedInbox),
        command!("Find in message", "Ctrl Shift F", FindInMessage),
        command!("Read message", "Enter", FocusReader),
        command!("Clean / plain text view", "V", ToggleReaderMode),
        command!("Refresh mail", "F5", RefreshMail),
        command!("Sort by sender", "Ctrl Alt S", SortSender),
        command!("Toggle folders sidebar", "Ctrl B", ToggleSidebar),
        command!("Toggle light / dark", "", ToggleTheme),
        command!("Settings", "Ctrl ,", OpenSettings),
    ]
}

/// Commands whose label contains every typed word, in list order.
pub(crate) fn filter(query: &str) -> Vec<Command> {
    let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    commands()
        .into_iter()
        .filter(|c| {
            let label = c.label.to_lowercase();
            words.iter().all(|w| label.contains(w.as_str()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn filter_matches_every_word() {
        let hits = super::filter("go dr");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].label, "Go to Drafts");
    }
}

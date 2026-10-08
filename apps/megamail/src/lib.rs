#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Folder {
    Inbox,
    Starred,
    Drafts,
    Sent,
    Archive,
}

#[derive(Debug, Clone, Copy)]
pub struct Message {
    pub sender: &'static str,
    pub address: &'static str,
    pub initials: &'static str,
    pub subject: &'static str,
    pub preview: &'static str,
    pub body: &'static [&'static str],
    pub time: &'static str,
    pub folder: Folder,
    pub unread: bool,
    pub starred: bool,
}

// ponytail: fixed in-memory sample data; replace with paged storage before loading a real mailbox.
pub const MESSAGES: [Message; 8] = [
    Message {
        sender: "Maya Chen",
        address: "maya@northstar.example",
        initials: "MC",
        subject: "The new identity system is ready",
        preview: "I pulled the final typography and color studies into one clear direction.",
        body: &[
            "Hi team,",
            "I pulled the final typography and color studies into one clear direction. The new system feels calmer, but it still has enough character to stand on its own.",
            "The attached notes cover the type scale, the revised mark, and a few practical examples for the product UI. I would love one last look before we share it more widely.",
            "Thanks,\nMaya",
        ],
        time: "10:42 AM",
        folder: Folder::Inbox,
        unread: true,
        starred: true,
    },
    Message {
        sender: "Daniel Kim",
        address: "daniel@fieldwork.example",
        initials: "DK",
        subject: "Friday's launch checklist",
        preview: "A short list of the details to settle before we call this ready.",
        body: &[
            "Hey everyone,",
            "Here is the short list of details to settle before Friday: confirm the help copy, check the small-screen layout, and make sure the release notes are easy to scan.",
            "I have time tomorrow afternoon if a second pair of eyes would help.",
            "Daniel",
        ],
        time: "9:18 AM",
        folder: Folder::Inbox,
        unread: true,
        starred: false,
    },
    Message {
        sender: "Lina Park",
        address: "lina@atelier.example",
        initials: "LP",
        subject: "A small idea for onboarding",
        preview: "What if the first screen showed one useful action instead of a tour?",
        body: &[
            "Hi Maya,",
            "What if the first screen showed one useful action instead of a tour? It could make the product feel more immediate and leave the deeper details for the moment someone needs them.",
            "I mocked up two versions this morning. No rush, but I would love your instinct on which one feels more natural.",
            "Lina",
        ],
        time: "Yesterday",
        folder: Folder::Inbox,
        unread: true,
        starred: false,
    },
    Message {
        sender: "Northstar Studio",
        address: "notes@northstar.example",
        initials: "NS",
        subject: "Your weekly product snapshot",
        preview: "The team shipped three small improvements and closed the open review notes.",
        body: &[
            "Good morning,",
            "The team shipped three small improvements this week and closed the open review notes. The activity view is easier to scan, the empty states have clearer next steps, and keyboard focus is more visible.",
            "Thanks for keeping the feedback thoughtful and specific. We will bring a short progress update to the next check-in.",
            "Northstar Studio",
        ],
        time: "Yesterday",
        folder: Folder::Inbox,
        unread: false,
        starred: false,
    },
    Message {
        sender: "Rohan Mehta",
        address: "rohan@commonroom.example",
        initials: "RM",
        subject: "Re: Homepage review notes",
        preview: "The quieter header reads better. I left two small comments in the draft.",
        body: &[
            "Hi Maya,",
            "The quieter header reads better. I left two small comments in the draft around the spacing and the caption below the first image.",
            "Everything else feels settled from my side.",
            "Rohan",
        ],
        time: "Tue",
        folder: Folder::Inbox,
        unread: false,
        starred: true,
    },
    Message {
        sender: "Studio Fieldnotes",
        address: "hello@fieldnotes.example",
        initials: "SF",
        subject: "The quiet details matter",
        preview: "A few references for making dense tools feel a little more human.",
        body: &[
            "Hello,",
            "This week's notes collect a few references for making dense tools feel a little more human: clear type, deliberate spacing, quiet dividers, and interactions that explain themselves.",
            "We hope something here is useful for your next round of work.",
            "Studio Fieldnotes",
        ],
        time: "Mon",
        folder: Folder::Inbox,
        unread: false,
        starred: false,
    },
    Message {
        sender: "You",
        address: "maya@northstar.example",
        initials: "MV",
        subject: "Notes from our call",
        preview: "I will send over the two directions and the updated schedule tomorrow.",
        body: &[
            "Hi Daniel,",
            "Thanks for the thoughtful conversation. I will send over the two directions and the updated schedule tomorrow morning.",
            "Best,\nMaya",
        ],
        time: "Mon",
        folder: Folder::Sent,
        unread: false,
        starred: false,
    },
    Message {
        sender: "You",
        address: "maya@northstar.example",
        initials: "MV",
        subject: "Product naming options",
        preview: "A few names to revisit after the next round of research.",
        body: &[
            "Working notes",
            "A few names to revisit after the next round of research. Keep the language short, easy to say, and grounded in the product's most useful action.",
        ],
        time: "Sun",
        folder: Folder::Drafts,
        unread: false,
        starred: false,
    },
];

pub struct Mailbox {
    folder: Folder,
    query: String,
    selected: Option<usize>,
    archived: [bool; MESSAGES.len()],
}

impl Default for Mailbox {
    fn default() -> Self {
        Self {
            folder: Folder::Inbox,
            query: String::new(),
            selected: Some(0),
            archived: [false; MESSAGES.len()],
        }
    }
}

impl Mailbox {
    pub fn folder(&self) -> Folder {
        self.folder
    }

    pub fn selected(&self) -> Option<usize> {
        self.selected
    }

    pub fn visible_ids(&self) -> Vec<usize> {
        let query = self.query.trim().to_lowercase();
        MESSAGES
            .iter()
            .enumerate()
            .filter(|(id, message)| {
                let in_folder = match self.folder {
                    Folder::Inbox => message.folder == Folder::Inbox && !self.archived[*id],
                    Folder::Starred => message.starred && !self.archived[*id],
                    Folder::Drafts => message.folder == Folder::Drafts && !self.archived[*id],
                    Folder::Sent => message.folder == Folder::Sent && !self.archived[*id],
                    Folder::Archive => self.archived[*id],
                };
                in_folder
                    && (query.is_empty()
                        || message.sender.to_lowercase().contains(&query)
                        || message.subject.to_lowercase().contains(&query)
                        || message.preview.to_lowercase().contains(&query))
            })
            .map(|(id, _)| id)
            .collect()
    }

    pub fn count(&self, folder: Folder) -> usize {
        let mailbox = Self {
            folder,
            query: String::new(),
            selected: None,
            archived: self.archived,
        };
        mailbox.visible_ids().len()
    }

    pub fn unread_count(&self) -> usize {
        MESSAGES
            .iter()
            .enumerate()
            .filter(|(id, message)| {
                message.folder == Folder::Inbox && message.unread && !self.archived[*id]
            })
            .count()
    }

    pub fn set_folder(&mut self, folder: Folder) {
        self.folder = folder;
        self.select_first_visible();
    }

    pub fn set_query(&mut self, query: String) {
        self.query = query;
        self.select_first_visible();
    }

    pub fn select(&mut self, id: usize) {
        if self.visible_ids().contains(&id) {
            self.selected = Some(id);
        }
    }

    pub fn move_selection(&mut self, delta: isize) {
        let visible = self.visible_ids();
        if visible.is_empty() {
            self.selected = None;
            return;
        }
        let current = self
            .selected
            .and_then(|selected| visible.iter().position(|id| *id == selected))
            .unwrap_or(0);
        let next = (current as isize + delta).clamp(0, visible.len() as isize - 1) as usize;
        self.selected = Some(visible[next]);
    }

    pub fn archive_selected(&mut self) -> bool {
        self.set_selected_archived(true)
    }

    pub fn restore_selected(&mut self) -> bool {
        self.set_selected_archived(false)
    }

    fn set_selected_archived(&mut self, archived: bool) -> bool {
        let Some(id) = self.selected else {
            return false;
        };
        if self.archived[id] == archived {
            return false;
        }
        self.archived[id] = archived;
        self.select_first_visible();
        true
    }

    fn select_first_visible(&mut self) {
        self.selected = self.visible_ids().first().copied();
    }
}

#[cfg(test)]
mod tests {
    use super::{Folder, MESSAGES, Mailbox};

    #[test]
    fn search_is_case_insensitive_and_selection_stays_visible() {
        let mut mailbox = Mailbox::default();
        mailbox.set_query("FRIDAY".into());
        assert_eq!(mailbox.visible_ids(), [1]);
        assert_eq!(mailbox.selected(), Some(1));

        mailbox.set_query("not found".into());
        assert!(mailbox.visible_ids().is_empty());
        assert_eq!(mailbox.selected(), None);
    }

    #[test]
    fn archive_and_restore_move_the_selected_message_between_folders() {
        let mut mailbox = Mailbox::default();
        let original_subject = MESSAGES[0].subject;

        assert!(mailbox.archive_selected());
        assert!(!mailbox.visible_ids().contains(&0));
        mailbox.set_folder(Folder::Archive);
        assert_eq!(mailbox.selected(), Some(0));
        assert_eq!(
            MESSAGES[mailbox.selected().unwrap()].subject,
            original_subject
        );

        assert!(mailbox.restore_selected());
        assert!(mailbox.visible_ids().is_empty());
        mailbox.set_folder(Folder::Inbox);
        assert!(mailbox.visible_ids().contains(&0));
    }

    #[test]
    fn keyboard_selection_stops_at_list_edges() {
        let mut mailbox = Mailbox::default();
        mailbox.move_selection(-1);
        assert_eq!(mailbox.selected(), Some(0));
        mailbox.move_selection(1);
        assert_eq!(mailbox.selected(), Some(1));
        mailbox.move_selection(20);
        assert_eq!(mailbox.selected(), Some(5));
    }
}

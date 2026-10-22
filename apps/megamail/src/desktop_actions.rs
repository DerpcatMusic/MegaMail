//! Native desktop compose and keyboard seams. Parent must declare this module,
//! replace the inline `action` module/bindings, and wire guarded listeners.
//! No URI launching, provider calls, settings mutation, printing, or AI runtime.

use gpui_kit::{App, FocusHandle, KeyBinding, Window};
use megamail_core::{desktop::MailtoIntent, worker::OutgoingMessage};

pub(crate) fn write_new_private_file(path: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    let mut file = options
        .open(path)
        .map_err(|_| "Choose a new filename; existing files are kept.")?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| {
            "The file could not be saved. The selected file may contain a partial copy.".into()
        })
}

pub(crate) mod action {
    gpui_kit::actions!(
        megamail,
        [
            NextMessage,
            PreviousMessage,
            ComposeMessage,
            ReplyMessage,
            ReplyAll,
            FocusSearch,
            ArchiveMessage,
            TrashMessage,
            JunkMessage,
            FindInMessage,
            RefreshMail,
            UnifiedInbox,
            FocusMessages,
            FocusReader,
            CyclePane,
            UndoMove,
            StarMessage,
            ToggleRead,
            CommandPalette,
            AllSent,
            AllDrafts,
            SortSender
        ]
    );
}

/// Install `MAILBOX_CONTEXT` only on the Mailbox surface, never on composer,
/// settings, setup, or modal ancestors. Keep arrows at the list focus node.
pub(crate) const MAILBOX_CONTEXT: &str = "MegaMailMailbox";
pub(crate) const MESSAGES_CONTEXT: &str = "MegaMailMessages";

pub(crate) fn bind_keys(cx: &mut App) {
    use action::*;
    cx.bind_keys([
        KeyBinding::new("down", NextMessage, Some(MESSAGES_CONTEXT)),
        KeyBinding::new("up", PreviousMessage, Some(MESSAGES_CONTEXT)),
        // Single keys only while the message list itself has focus, so typing
        // in search or the composer never triggers them.
        KeyBinding::new("j", NextMessage, Some(MESSAGES_CONTEXT)),
        KeyBinding::new("k", PreviousMessage, Some(MESSAGES_CONTEXT)),
        KeyBinding::new("r", ReplyMessage, Some(MESSAGES_CONTEXT)),
        KeyBinding::new("a", ReplyAll, Some(MESSAGES_CONTEXT)),
        KeyBinding::new("c", ComposeMessage, Some(MESSAGES_CONTEXT)),
        KeyBinding::new("/", FocusSearch, Some(MESSAGES_CONTEXT)),
        KeyBinding::new("e", ArchiveMessage, Some(MESSAGES_CONTEXT)),
        KeyBinding::new("#", TrashMessage, Some(MESSAGES_CONTEXT)),
        KeyBinding::new("shift-3", TrashMessage, Some(MESSAGES_CONTEXT)),
        KeyBinding::new("!", JunkMessage, Some(MESSAGES_CONTEXT)),
        KeyBinding::new("shift-1", JunkMessage, Some(MESSAGES_CONTEXT)),
        KeyBinding::new("ctrl-n", ComposeMessage, Some(MAILBOX_CONTEXT)),
        KeyBinding::new("ctrl-r", ReplyMessage, Some(MAILBOX_CONTEXT)),
        KeyBinding::new("ctrl-shift-r", ReplyAll, Some(MAILBOX_CONTEXT)),
        KeyBinding::new("ctrl-f", FocusSearch, Some(MAILBOX_CONTEXT)),
        KeyBinding::new("ctrl-shift-f", FindInMessage, Some(MAILBOX_CONTEXT)),
        KeyBinding::new("ctrl-shift-l", UnifiedInbox, Some(MAILBOX_CONTEXT)),
        KeyBinding::new("f5", RefreshMail, Some(MAILBOX_CONTEXT)),
        KeyBinding::new("f6", CyclePane, Some(MAILBOX_CONTEXT)),
        KeyBinding::new("ctrl-z", UndoMove, Some(MESSAGES_CONTEXT)),
        KeyBinding::new("s", StarMessage, Some(MESSAGES_CONTEXT)),
        KeyBinding::new("u", ToggleRead, Some(MESSAGES_CONTEXT)),
        KeyBinding::new("?", CommandPalette, Some(MESSAGES_CONTEXT)),
        KeyBinding::new("shift-/", CommandPalette, Some(MESSAGES_CONTEXT)),
        KeyBinding::new("ctrl-k", CommandPalette, Some(MAILBOX_CONTEXT)),
        KeyBinding::new("ctrl-alt-l", FocusMessages, Some(MAILBOX_CONTEXT)),
        KeyBinding::new("ctrl-alt-r", FocusReader, Some(MAILBOX_CONTEXT)),
        KeyBinding::new("ctrl-alt-2", AllSent, Some(MAILBOX_CONTEXT)),
        KeyBinding::new("ctrl-alt-3", AllDrafts, Some(MAILBOX_CONTEXT)),
        KeyBinding::new("ctrl-alt-s", SortSender, Some(MAILBOX_CONTEXT)),
    ]);
}

/// Snapshot at dispatch time, not a cached decision from when a key was bound.
/// `body_ready` must mean a usable body for the exact selected account/message,
/// not merely `!body_loading` after an error or stale response.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct ActionState {
    pub mailbox_visible: bool,
    pub composer_present: bool,
    pub modal_active: bool,
    pub has_accounts: bool,
    pub usable_sender: bool,
    pub has_rows: bool,
    pub body_ready: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MailboxAction {
    NextMessage,
    PreviousMessage,
    ComposeMessage,
    ReplyMessage,
    ReplyAll,
    FocusSearch,
    RefreshMail,
    UnifiedInbox,
    FocusMessages,
    FocusReader,
}

impl ActionState {
    /// Use this same guard from toolbar/menu and keyboard listeners. Even when
    /// the message pane is still rendered beside a composer, arrows must not
    /// change selection behind it. No destructive/send/cancel keys are added.
    pub(crate) fn allows(self, action: MailboxAction) -> bool {
        if !self.mailbox_visible || self.composer_present || self.modal_active {
            return false;
        }
        match action {
            MailboxAction::ComposeMessage => self.usable_sender,
            MailboxAction::ReplyMessage | MailboxAction::ReplyAll => {
                self.usable_sender && self.body_ready
            }
            MailboxAction::NextMessage | MailboxAction::PreviousMessage => self.has_rows,
            MailboxAction::RefreshMail | MailboxAction::UnifiedInbox => self.has_accounts,
            MailboxAction::FocusSearch | MailboxAction::FocusMessages => true,
            MailboxAction::FocusReader => self.body_ready,
        }
    }
}

/// Focus an existing, rendered tab-stop handle only after checking its action.
/// Caller owns `track_focus`, visible focus indication, and restoring focus
/// when views close. This does not create an invisible focus target.
pub(crate) fn focus_pane(
    action: MailboxAction,
    state: ActionState,
    target: &FocusHandle,
    window: &mut Window,
    cx: &mut App,
) -> bool {
    if !matches!(
        action,
        MailboxAction::FocusSearch | MailboxAction::FocusMessages | MailboxAction::FocusReader
    ) || !state.allows(action)
    {
        return false;
    }
    target.focus(window, cx);
    true
}

/// Same parser for command-line URI ingress and an explicitly clicked renderer
/// mailto link. No caller may pre-decode the URI or populate headers separately.
pub(crate) fn external_compose(
    uri: &str,
) -> Result<MailtoIntent, megamail_core::desktop::MailtoError> {
    megamail_core::desktop::parse_mailto(uri)
}

/// Turn validated external intent into the existing compose/worker payload.
/// Account is chosen explicitly by the UI after setup/availability checks;
/// nothing in the URI chooses a sender or bypasses send/draft preflight.
/// Opening this payload is not authorization to send it.
pub(crate) fn compose_message(intent: MailtoIntent, account_id: u32) -> OutgoingMessage {
    let mut message = crate::compose::new_message(account_id);
    message.to = intent.to.join(", ");
    message.cc = intent.cc.join(", ");
    message.bcc = intent.bcc.join(", ");
    message.subject = intent.subject;
    message.body = intent.body;
    message
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ready() -> ActionState {
        ActionState {
            mailbox_visible: true,
            has_accounts: true,
            usable_sender: true,
            has_rows: true,
            body_ready: true,
            ..ActionState::default()
        }
    }

    const ACTIONS: [MailboxAction; 10] = [
        MailboxAction::NextMessage,
        MailboxAction::PreviousMessage,
        MailboxAction::ComposeMessage,
        MailboxAction::ReplyMessage,
        MailboxAction::ReplyAll,
        MailboxAction::FocusSearch,
        MailboxAction::RefreshMail,
        MailboxAction::UnifiedInbox,
        MailboxAction::FocusMessages,
        MailboxAction::FocusReader,
    ];

    #[test]
    fn blocks_every_mailbox_action_behind_composer_modal_or_other_view() {
        for blocked in [
            ActionState {
                composer_present: true,
                ..ready()
            },
            ActionState {
                modal_active: true,
                ..ready()
            },
            ActionState {
                mailbox_visible: false,
                ..ready()
            },
        ] {
            for action in ACTIONS {
                assert!(!blocked.allows(action), "{action:?}");
            }
        }
    }

    #[test]
    fn missing_body_does_not_reply_or_focus_but_preserves_navigation() {
        let state = ActionState {
            body_ready: false,
            ..ready()
        };
        assert!(!state.allows(MailboxAction::ReplyMessage));
        assert!(!state.allows(MailboxAction::ReplyAll));
        assert!(!state.allows(MailboxAction::FocusReader));
        assert!(state.allows(MailboxAction::NextMessage));
        assert!(state.allows(MailboxAction::FocusSearch));
    }

    #[test]
    fn unavailable_sender_is_not_an_available_compose_or_reply() {
        let state = ActionState {
            usable_sender: false,
            ..ready()
        };
        assert!(!state.allows(MailboxAction::ComposeMessage));
        assert!(!state.allows(MailboxAction::ReplyAll));
        assert!(state.allows(MailboxAction::FocusReader));
    }

    #[test]
    fn empty_mailbox_keeps_focus_reachable_without_mutation_actions() {
        let state = ActionState {
            mailbox_visible: true,
            ..ActionState::default()
        };
        assert!(state.allows(MailboxAction::FocusMessages));
        assert!(state.allows(MailboxAction::FocusSearch));
        assert!(!state.allows(MailboxAction::NextMessage));
        assert!(!state.allows(MailboxAction::UnifiedInbox));
        assert!(!state.allows(MailboxAction::RefreshMail));
    }

    #[test]
    fn production_compose_seam_preserves_bcc_and_uses_only_explicit_account() {
        let intent = external_compose("mailto:a@example.test?cc=c@example.test&bcc=b@example.test&subject=Review&body=%3Cb%3Enot+HTML%3C%2Fb%3E").unwrap();
        let message = compose_message(intent, 42);
        assert_eq!(message.from_account_id, 42);
        assert_eq!(message.to, "a@example.test");
        assert_eq!(message.cc, "c@example.test");
        assert_eq!(message.bcc, "b@example.test");
        assert_eq!(message.subject, "Review");
        assert_eq!(message.body, "<b>not+HTML</b>");
        assert!(message.html.is_empty());
        assert!(message.attachments.is_empty());
        assert!(message.from_alias.is_none());
        assert!(message.reply_to.is_empty());
        assert!(message.in_reply_to.is_empty());
        assert!(message.references.is_empty());
        assert!(message.draft_origin.is_none());
        assert!(message.outbox_origin.is_none());
        assert!(message.send_at.is_none());
        assert!(!message.sign && !message.encrypt);
    }

    #[test]
    fn production_ingress_rejects_header_injection_before_compose_creation() {
        assert_eq!(
            external_compose("mailto:?subject=x%0ABcc:y@example.test"),
            Err(megamail_core::desktop::MailtoError::UnsafeHeader)
        );
    }
}

//! Ctrl+K command palette: every mailbox command, searchable, with its shortcut shown.
//! It doubles as the keyboard cheat sheet (`?`).
use crate::action::*;
use crate::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Cmd {
    Compose,
    Reply,
    ReplyAll,
    Archive,
    Trash,
    Junk,
    Undo,
    Star,
    ToggleRead,
    Search,
    FindInMessage,
    Refresh,
    AllInboxes,
    AllSent,
    AllDrafts,
    SortSender,
    SwitchPane,
    Sidebar,
    Settings,
}

const COMMANDS: &[(Cmd, &str, &str)] = &[
    (Cmd::Compose, "New message", "C"),
    (Cmd::Reply, "Reply", "R"),
    (Cmd::ReplyAll, "Reply all", "A"),
    (Cmd::Archive, "Archive", "E"),
    (Cmd::Trash, "Move to Trash", "#"),
    (Cmd::Junk, "Report junk", "!"),
    (Cmd::Undo, "Undo last move", "Ctrl+Z"),
    (Cmd::Star, "Star or unstar", "S"),
    (Cmd::ToggleRead, "Mark read or unread", "U"),
    (Cmd::Search, "Search mail", "/"),
    (Cmd::FindInMessage, "Find in message", "Ctrl+Shift+F"),
    (Cmd::Refresh, "Check for new mail", "F5"),
    (Cmd::AllInboxes, "Go to all inboxes", "Ctrl+Shift+L"),
    (Cmd::AllSent, "Go to all sent mail", "Ctrl+Alt+2"),
    (Cmd::AllDrafts, "Go to all drafts", "Ctrl+Alt+3"),
    (Cmd::SortSender, "Sort by sender", "Ctrl+Alt+S"),
    (Cmd::SwitchPane, "Switch between list and message", "F6"),
    (Cmd::Sidebar, "Show or hide the sidebar", ""),
    (Cmd::Settings, "Settings and appearance", ""),
];

fn matching(query: &str) -> Vec<(Cmd, &'static str, &'static str)> {
    let query = query.to_lowercase();
    COMMANDS
        .iter()
        .filter(|(_, label, _)| query.split_whitespace().all(|word| label.to_lowercase().contains(word)))
        .copied()
        .collect()
}

impl MailApp {
    pub(crate) fn toggle_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.palette_open = !self.palette_open && self.view == View::Mailbox && self.composer.is_none();
        self.palette_sel = 0;
        self.palette_input.update(cx, |input, cx| input.set_value("", window, cx));
        if self.palette_open {
            self.palette_input.read(cx).focus_handle(cx).focus(window, cx);
        } else {
            self.message_focus.focus(window, cx);
        }
        cx.notify();
    }

    fn run_command(&mut self, cmd: Cmd, window: &mut Window, cx: &mut Context<Self>) {
        self.palette_open = false;
        self.mailbox_focus.focus(window, cx);
        let snapshot = self.live.read(cx).snapshot();
        let mut dispatch = |action: Box<dyn gpui_kit::Action>| window.dispatch_action(action, cx);
        match cmd {
            Cmd::Compose => dispatch(Box::new(ComposeMessage)),
            Cmd::Reply => dispatch(Box::new(ReplyMessage)),
            Cmd::ReplyAll => dispatch(Box::new(ReplyAll)),
            Cmd::Archive => dispatch(Box::new(ArchiveMessage)),
            Cmd::Trash => dispatch(Box::new(TrashMessage)),
            Cmd::Junk => dispatch(Box::new(JunkMessage)),
            Cmd::Undo => dispatch(Box::new(UndoMove)),
            Cmd::Star => dispatch(Box::new(StarMessage)),
            Cmd::ToggleRead => dispatch(Box::new(ToggleRead)),
            Cmd::Search => dispatch(Box::new(FocusSearch)),
            Cmd::FindInMessage => dispatch(Box::new(FindInMessage)),
            Cmd::Refresh => dispatch(Box::new(RefreshMail)),
            Cmd::AllInboxes => dispatch(Box::new(UnifiedInbox)),
            Cmd::AllSent => dispatch(Box::new(AllSent)),
            Cmd::AllDrafts => dispatch(Box::new(AllDrafts)),
            Cmd::SortSender => dispatch(Box::new(SortSender)),
            Cmd::SwitchPane => dispatch(Box::new(CyclePane)),
            Cmd::Sidebar => self.sidebar_collapsed = !self.sidebar_collapsed,
            Cmd::Settings => self.open_appearance(cx),
        }
        let _ = snapshot;
        cx.notify();
    }

    pub(crate) fn star_selected(&mut self, cx: &mut Context<Self>) {
        let starred = self.live.read(cx).snapshot().selected_message.is_some_and(|row| row.message.starred);
        self.live.update(cx, |live, cx| live.set_selected_starred(!starred, cx));
    }

    pub(crate) fn toggle_selected_read(&mut self, cx: &mut Context<Self>) {
        let unread = self.live.read(cx).snapshot().selected_message.is_some_and(|row| row.message.unread);
        self.live.update(cx, |live, cx| live.set_selected_unread(!unread, cx));
    }

    pub(crate) fn palette_overlay(&self, palette: Palette, cx: &mut Context<Self>) -> AnyElement {
        let query = self.palette_input.read(cx).value().to_string();
        let found = matching(&query);
        let sel = self.palette_sel.min(found.len().saturating_sub(1));
        let enter = found.get(sel).map(|c| c.0);
        let count = found.len();
        div()
            .absolute()
            .size_full()
            .flex()
            .justify_center()
            .pt(px(96.))
            .bg(palette.background.opacity(0.6))
            .on_mouse_down(gpui_kit::MouseButton::Left, cx.listener(|this, _, window, cx| this.toggle_palette(window, cx)))
            .child(
                v_flex()
                    .w(px(560.))
                    .h_auto()
                    .max_h(px(520.))
                    .self_start()
                    .gap_1()
                    .p_2()
                    .rounded(px(10.))
                    .bg(palette.surface)
                    .border_1()
                    .border_color(palette.border)
                    .shadow_lg()
                    .on_mouse_down(gpui_kit::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_key_down(cx.listener(move |this, event: &gpui_kit::KeyDownEvent, window, cx| {
                        match event.keystroke.key.as_str() {
                            "escape" => this.toggle_palette(window, cx),
                            "down" => { this.palette_sel = (this.palette_sel + 1).min(count.saturating_sub(1)); cx.notify(); }
                            "up" => { this.palette_sel = this.palette_sel.saturating_sub(1); cx.notify(); }
                            "enter" => if let Some(cmd) = enter { this.run_command(cmd, window, cx) },
                            _ => {}
                        }
                    }))
                    .child(Input::new(&self.palette_input).appearance(false))
                    .children(found.into_iter().enumerate().map(|(row, (cmd, label, keys))| {
                        h_flex()
                            .id(SharedString::from(format!("cmd-{row}")))
                            .w_full()
                            .h(px(32.))
                            .px_3()
                            .items_center()
                            .justify_between()
                            .rounded(px(6.))
                            .cursor_pointer()
                            .text_size(px(13.))
                            .when(row == sel, |el| el.bg(palette.selected))
                            .hover(|el| el.bg(palette.hover))
                            .on_click(cx.listener(move |this, _, window, cx| this.run_command(cmd, window, cx)))
                            .child(label)
                            .child(div().text_size(px(12.)).text_color(palette.faint).child(keys))
                    }))
                    .when(count == 0, |el| el.child(div().p_3().text_size(px(12.)).text_color(palette.muted).child("No matching command"))),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn filters_by_every_word() {
        assert_eq!(matching("").len(), COMMANDS.len());
        assert_eq!(matching("mark unread").first().map(|c| c.0), Some(Cmd::ToggleRead));
        assert!(matching("zzz").is_empty());
    }
}

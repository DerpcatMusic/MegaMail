//! Local rule editing; saved rules are previewed explicitly before any application.
use crate::*;
use gpui_kit::base::Disableable as _;
use gpui_kit::component::{Sizable as _, button::ButtonVariants as _};
use gpui_kit::{
    AppContext as _, IntoElement as _, ParentElement as _, Styled as _, prelude::FluentBuilder as _,
};
use megamail_core::config::{FilterField, FilterMatch, FilterRule};

pub(crate) struct State {
    open: bool,
    busy: bool,
    rules: Option<Vec<FilterRule>>,
    editing: Option<usize>,
    delete: Option<usize>,
    page: usize,
    name: Entity<InputState>,
    value: Entity<InputState>,
    destination: Entity<InputState>,
    field: FilterField,
    account: Option<(u32, String)>,
    status: String,
}
impl State {
    pub fn new(window: &mut Window, cx: &mut Context<MailApp>) -> Self {
        Self {
            open: false,
            busy: false,
            rules: None,
            editing: None,
            delete: None,
            page: 0,
            name: new_input(window, cx, "Rule name"),
            value: new_input(window, cx, "Text to match"),
            destination: new_input(window, cx, "Exact destination folder path"),
            field: FilterField::Subject,
            account: None,
            status: String::new(),
        }
    }
}
fn editable(rule: &FilterRule) -> bool {
    rule.matcher == FilterMatch::Contains
        && matches!(rule.field, FilterField::Subject | FilterField::FromAddress)
        && rule.more.is_empty()
        && !rule.any
        && rule.tag.is_empty()
}
fn updated_rule(
    existing: Option<&FilterRule>,
    name: String,
    value: String,
    destination: String,
    field: FilterField,
    email: &str,
) -> Result<FilterRule, String> {
    if existing.is_some_and(|r| !editable(r) || r.account_email != email) {
        return Err("This rule uses advanced conditions or another account. It can be previewed but cannot be simplified by this editor.".into());
    }
    if name.trim().is_empty()
        || name.len() > 256
        || value.trim().is_empty()
        || value.len() > 8192
        || destination.is_empty()
        || destination.len() > 1024
        || destination.chars().any(char::is_control)
    {
        return Err("Enter a name, matching text and a valid destination folder.".into());
    }
    Ok(FilterRule {
        name,
        account_email: email.into(),
        field,
        matcher: FilterMatch::Contains,
        value,
        more: vec![],
        any: false,
        dest_path: destination,
        tag: String::new(),
        count_unread: existing.map_or(true, |r| r.count_unread),
    })
}
impl MailApp {
    fn load_rule_editor(&mut self, cx: &mut Context<Self>) {
        if self.demo || self.rule_editor.busy {
            return;
        }
        self.rule_editor.open = true;
        self.rule_editor.busy = true;
        self.rule_editor.status = "Loading saved rules…".into();
        let task = cx.background_spawn(async { config::load_filters_checked() });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                let state = &mut this.rule_editor;
                state.busy = false;
                state.editing = None; state.delete = None; state.account = None; state.page = 0;
                match result {
                    Ok(rules) => { state.rules = Some(rules); state.status = "Saved rules loaded. Preview a selected message before applying a rule.".into(); },
                    Err(error) => { state.rules = None; state.status = error; }
                }
                cx.notify();
            });
        }).detach();
        cx.notify();
    }
    fn edit_local_rule(
        &mut self,
        index: Option<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.rule_editor.busy || self.demo || self.composer.is_some() {
            return;
        }
        let snapshot = self.live.read(cx).snapshot();
        let Some(account) = snapshot
            .current_account_id
            .and_then(|id| snapshot.accounts.iter().find(|a| a.id == id))
        else {
            self.rule_editor.status = "Choose one account before editing a rule.".into();
            cx.notify();
            return;
        };
        let rule = index
            .and_then(|i| self.rule_editor.rules.as_ref()?.get(i))
            .cloned();
        if index.is_some()
            && rule
                .as_ref()
                .is_none_or(|r| !editable(r) || r.account_email != account.email)
        {
            self.rule_editor.status = "Choose the rule's account. Advanced rules are preserved and available for preview.".into();
            cx.notify();
            return;
        }
        self.rule_editor.account = Some((account.id, account.email.clone()));
        self.rule_editor.editing = index;
        self.rule_editor.delete = None;
        self.rule_editor.field = rule.as_ref().map_or(FilterField::Subject, |r| r.field);
        for (input, value) in [
            (
                &self.rule_editor.name,
                rule.as_ref().map_or(String::new(), |r| r.name.clone()),
            ),
            (
                &self.rule_editor.value,
                rule.as_ref().map_or(String::new(), |r| r.value.clone()),
            ),
            (
                &self.rule_editor.destination,
                rule.as_ref().map_or(String::new(), |r| r.dest_path.clone()),
            ),
        ] {
            input.update(cx, |input, cx| input.set_value(value, window, cx));
        }
        self.rule_editor.status = format!(
            "Editing a local rule for {}. Saving does not change existing messages.",
            account.label
        );
        cx.notify();
    }
    fn save_local_rule(&mut self, delete: Option<usize>, cx: &mut Context<Self>) {
        if self.rule_editor.busy || self.demo || self.composer.is_some() {
            return;
        }
        let Some(expected) = self.rule_editor.rules.clone() else {
            return;
        };
        let mut next = expected.clone();
        if let Some(index) = delete {
            if self.rule_editor.delete != Some(index) || index >= next.len() {
                return;
            }
            next.remove(index);
        } else {
            let Some((id, email)) = self.rule_editor.account.clone() else {
                return;
            };
            let snapshot = self.live.read(cx).snapshot();
            let destination = self.rule_editor.destination.read(cx).value().to_string();
            if snapshot.current_account_id != Some(id)
                || !snapshot
                    .accounts
                    .iter()
                    .any(|a| a.id == id && a.email == email)
                || !snapshot
                    .folders
                    .iter()
                    .any(|f| f.account_id == id && f.path == destination)
            {
                self.rule_editor.status =
                    "The account or destination changed. Choose the account and edit again.".into();
                cx.notify();
                return;
            }
            let result = updated_rule(
                self.rule_editor.editing.and_then(|i| next.get(i)),
                self.rule_editor.name.read(cx).value().to_string(),
                self.rule_editor.value.read(cx).value().to_string(),
                destination,
                self.rule_editor.field,
                &email,
            );
            let rule = match result {
                Ok(rule) => rule,
                Err(error) => {
                    self.rule_editor.status = error;
                    cx.notify();
                    return;
                }
            };
            if let Some(index) = self.rule_editor.editing {
                next[index] = rule;
            } else {
                next.push(rule);
            }
        }
        self.rule_editor.busy = true;
        self.rule_editor.status = "Saving rules…".into();
        let task = cx.background_spawn(async move {
            config::save_filters_checked(&expected, &next).map(|_| next)
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                let state = &mut this.rule_editor; state.busy = false;
                match result {
                    Ok(rules) => { state.rules = Some(rules); state.editing = None; state.account = None; state.delete = None; state.status = "Saved. Existing messages were unchanged; use Preview saved rules on a selected message.".into(); },
                    Err(error) => state.status = error,
                }
                cx.notify();
            });
        }).detach();
        cx.notify();
    }
    pub(crate) fn rule_editor_panel(&self, palette: Palette, cx: &mut Context<Self>) -> AnyElement {
        let state = &self.rule_editor;
        let enabled = !self.demo && !state.busy && self.composer.is_none();
        let count = state.rules.as_ref().map_or(0, Vec::len);
        v_flex().w_full().gap_2()
            .child(Button::new("manage-local-rules").ghost().small().label(if state.open { "Hide rules" } else { "Manage local rules" }).disabled(self.demo || state.busy)
                .on_click(cx.listener(|this, _, _, cx| { if this.rule_editor.open { this.rule_editor.open = false; cx.notify(); } else { this.load_rule_editor(cx); } })))
            .when(state.open, |el| el.child(v_flex().id("local-rule-editor").w_full().gap_2().max_h(px(360.)).overflow_y_scroll()
                .child(div().text_size(px(11.)).text_color(palette.muted).child("Rules are stored on this computer. This editor creates subject or sender conditions that move mail; existing messages need explicit preview and application."))
                .child(h_flex().flex_wrap().gap_2()
                    .child(Button::new("reload-local-rules").ghost().small().label("Reload").disabled(!enabled).on_click(cx.listener(|this, _, _, cx| this.load_rule_editor(cx))))
                    .child(Button::new("new-local-rule").ghost().small().label("New rule for this account").disabled(!enabled || state.rules.is_none()).on_click(cx.listener(|this, _, window, cx| this.edit_local_rule(None, window, cx)))))
                .children(state.rules.iter().flatten().enumerate().skip(state.page * 20).take(20).map(|(index, rule)| {
                    h_flex().flex_wrap().gap_2().child(div().flex_1().min_w_0().text_size(px(11.)).child(format!("{} · {} · {}", rule.name, rule.account_email, rule.dest_path)))
                        .child(Button::new(SharedString::from(format!("edit-rule-{index}"))).ghost().small().label("Edit").disabled(!enabled || !editable(rule)).on_click(cx.listener(move |this, _, window, cx| this.edit_local_rule(Some(index), window, cx))))
                        .child(Button::new(SharedString::from(format!("delete-rule-{index}"))).ghost().small().label(if state.delete == Some(index) { "Confirm delete" } else { "Delete" }).disabled(!enabled)
                            .on_click(cx.listener(move |this, _, _, cx| { if this.rule_editor.delete == Some(index) { this.save_local_rule(Some(index), cx); } else { this.rule_editor.delete = Some(index); cx.notify(); } })))
                }))
                .when(count > 20, |el| el.child(h_flex().gap_2()
                    .child(Button::new("previous-rules").ghost().small().label("Previous").disabled(!enabled || state.page == 0).on_click(cx.listener(|this, _, _, cx| { this.rule_editor.page = this.rule_editor.page.saturating_sub(1); cx.notify(); })))
                    .child(Button::new("next-rules").ghost().small().label("Next").disabled(!enabled || (state.page + 1) * 20 >= count).on_click(cx.listener(|this, _, _, cx| { this.rule_editor.page += 1; cx.notify(); })))))
                .when(state.account.is_some(), |el| el
                    .child(Input::new(&state.name).aria_label("Rule name").disabled(!enabled))
                    .child(Button::new("rule-condition-field").ghost().small().label(if state.field == FilterField::Subject { "Match subject containing" } else { "Match sender address containing" }).disabled(!enabled)
                        .on_click(cx.listener(|this, _, _, cx| { this.rule_editor.field = if this.rule_editor.field == FilterField::Subject { FilterField::FromAddress } else { FilterField::Subject }; cx.notify(); })))
                    .child(Input::new(&state.value).aria_label("Rule matching text").disabled(!enabled))
                    .child(Input::new(&state.destination).aria_label("Rule destination folder").disabled(!enabled))
                    .child(Button::new("save-local-rule").small().label("Save rule").disabled(!enabled).on_click(cx.listener(|this, _, _, cx| this.save_local_rule(None, cx)))))
                .child(div().text_size(px(11.)).text_color(palette.muted).child(state.status.clone()))))
            .into_any_element()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn simple_edits_preserve_unread_policy_and_refuse_advanced_rules() {
        let mut rule = updated_rule(
            None,
            "Fixture".into(),
            "needle".into(),
            "Archive".into(),
            FilterField::Subject,
            "owner@example.test",
        )
        .unwrap();
        rule.count_unread = false;
        let edited = updated_rule(
            Some(&rule),
            "Edited".into(),
            "sender".into(),
            "Archive".into(),
            FilterField::FromAddress,
            "owner@example.test",
        )
        .unwrap();
        assert!(!edited.count_unread);
        assert!(
            updated_rule(
                Some(&rule),
                "Edited".into(),
                "sender".into(),
                "Archive".into(),
                FilterField::Subject,
                "other@example.test"
            )
            .is_err()
        );
        rule.tag = "keep".into();
        assert!(
            updated_rule(
                Some(&rule),
                "Edited".into(),
                "sender".into(),
                "Archive".into(),
                FilterField::Subject,
                "owner@example.test"
            )
            .is_err()
        );
        assert!(
            updated_rule(
                None,
                "Fixture".into(),
                "".into(),
                "Archive".into(),
                FilterField::Subject,
                "owner@example.test"
            )
            .is_err()
        );
    }
}

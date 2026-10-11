//! Loaded-message selection and serial, qualified native mutations.
use crate::triage::{
    self, AdapterKind, Batch, Capabilities, Completion, ItemOutcome, Mutation, OperationToken,
    ScopedMessageKey, SelectedItem, Selection, SelectionEdit, UndoReceipt,
};
use crate::*;
use gpui_kit::base::{Disableable as _, Selectable as _};
use gpui_kit::component::{Sizable as _, button::ButtonVariants as _};
use gpui_kit::{
    AppContext as _, IntoElement as _, ParentElement as _, Styled as _, prelude::FluentBuilder as _,
};
use megamail_core::worker::{
    MailRequest, NativeMutationAction, NativeMutationOutcome, NativeMutationRequest,
    NativeMutationScope, NativeMutationToken,
};
use std::collections::{HashMap, HashSet};

#[derive(Clone, PartialEq, Eq)]
struct ViewStamp {
    scope: MailboxScope,
    account: Option<u32>,
    folder: Option<String>,
    query: String,
    filter: MessageFilter,
    grouped: bool,
    sessions: Vec<(u32, u64)>,
    epochs: Vec<((u32, String), live::NativeMailboxEpoch)>,
}

pub(crate) struct State {
    pub open: bool,
    pub selection: Selection,
    pub available: Arc<HashSet<MessageKey>>,
    generation: u64,
    stamp: Option<ViewStamp>,
    pub destination: Entity<InputState>,
    batch: Option<Batch>,
    next_batch: u64,
    pending: Option<(ScopedMessageKey, NativeMutationRequest)>,
    undo: Vec<UndoReceipt>,
    undoing: bool,
    plans: Vec<String>,
    results: Vec<(ScopedMessageKey, ItemOutcome)>,
    labels: HashMap<ScopedMessageKey, String>,
    notice: String,
    show_results: bool,
}
impl State {
    pub fn new(window: &mut Window, cx: &mut Context<MailApp>) -> Self {
        Self {
            open: false,
            selection: Selection::default(),
            available: Arc::new(HashSet::new()),
            generation: 0,
            stamp: None,
            destination: new_input(window, cx, "Discovered destination folder path"),
            batch: None,
            next_batch: 0,
            pending: None,
            undo: vec![],
            undoing: false,
            plans: vec![],
            results: vec![],
            labels: HashMap::new(),
            notice: String::new(),
            show_results: false,
        }
    }
    fn next_id(&mut self) -> Option<u64> {
        self.next_batch = self.next_batch.checked_add(1)?;
        Some(self.next_batch)
    }
    pub(crate) fn busy(&self) -> bool {
        self.batch.is_some() || self.pending.is_some()
    }
    pub fn selected(&self, key: &MessageKey) -> bool {
        self.selection.keys().any(|k| &k.message == key)
    }
}

#[derive(Clone)]
pub(crate) enum Choice {
    Unread(bool),
    Starred(bool),
    Folder(String),
    Role(FolderKind),
}

fn scoped(key: &MessageKey, scope: NativeMutationScope) -> ScopedMessageKey {
    let (session, mailbox) = match scope {
        NativeMutationScope::Imap {
            generation,
            session,
            ..
        } => (session, generation),
        NativeMutationScope::Thunderbird { session } => (session, session),
    };
    ScopedMessageKey {
        message: key.clone(),
        session_generation: session,
        mailbox_generation: mailbox,
    }
}

fn qualified(
    dispatch: triage::Dispatch,
    scope: NativeMutationScope,
) -> Option<NativeMutationRequest> {
    if scoped(&dispatch.key.message, scope) != dispatch.key {
        return None;
    }
    let action = match dispatch.request {
        MailRequest::SetSeen { path, uid, seen }
            if path == dispatch.key.message.folder_path && uid == dispatch.key.message.uid =>
        {
            NativeMutationAction::SetSeen(seen)
        }
        MailRequest::SetFlagged { path, uid, flagged }
            if path == dispatch.key.message.folder_path && uid == dispatch.key.message.uid =>
        {
            NativeMutationAction::SetFlagged(flagged)
        }
        MailRequest::MoveMessage { path, uid, dest }
            if path == dispatch.key.message.folder_path && uid == dispatch.key.message.uid =>
        {
            NativeMutationAction::Move { destination: dest }
        }
        _ => return None,
    };
    Some(NativeMutationRequest {
        token: NativeMutationToken {
            batch_id: dispatch.token.batch_id,
            item_index: dispatch.token.item_index,
        },
        path: dispatch.key.message.folder_path,
        uid: dispatch.key.message.uid,
        scope,
        action,
        required_current_flag: None,
    })
}

fn result_label(outcome: ItemOutcome) -> &'static str {
    match outcome {
        ItemOutcome::Succeeded => "Confirmed",
        ItemOutcome::Unchanged => "Already in the requested state",
        ItemOutcome::Failed => "Failed; refresh before trying again",
        ItemOutcome::Uncertain => "Unconfirmed; review manually before retrying",
        ItemOutcome::Rejected(_) => "Unavailable; no action submitted",
        ItemOutcome::Canceled => "Canceled before submission",
        ItemOutcome::Pending => "Waiting",
        ItemOutcome::InFlight => "Awaiting confirmation",
    }
}

fn action_error(error: triage::TriageError) -> &'static str {
    use triage::TriageError::*;
    match error {
        TooManyVisible => {
            "Too many messages are loaded for selection. Narrow the view to at most 5000 messages."
        }
        TooManySelected => {
            "Select at most 500 messages per action. Narrow or clear the selection before trying again."
        }
        NotVisible | MissingSnapshot | StaleView => {
            "The message list changed. Refresh and select the current messages again."
        }
        InvalidKey | InvalidGeneration | DuplicateKey => {
            "The selection identity is unavailable. Refresh before selecting messages."
        }
        InvalidCompletion => {
            "A result did not match its original action. Review the affected message manually."
        }
        InvalidBatchId | InvalidAction => {
            "This action could not be prepared. Review the selection and destination."
        }
        ConfirmationRequired => "Review the destination list, then confirm the move.",
        UndoConflict => {
            "A message changed after the original action. Undo was not submitted; review it manually."
        }
    }
}

impl MailApp {
    fn triage_items(&self, cx: &App) -> Vec<(SelectedItem, NativeMutationScope)> {
        let live = self.live.read(cx);
        let mut seen = HashSet::new();
        self.list_rows
            .iter()
            .filter_map(|row| {
                if !seen.insert(row.row.key.clone()) {
                    return None;
                }
                let scope = live.native_mutation_scope(&row.row.key)?;
                Some((
                    SelectedItem {
                        key: scoped(&row.row.key, scope),
                        unread: row.row.message.unread,
                        starred: row.row.message.starred,
                    },
                    scope,
                ))
            })
            .collect()
    }
    fn triage_caps(
        &self,
        item: &SelectedItem,
        scope: NativeMutationScope,
        snapshot: &MailboxSnapshot,
        cx: &App,
    ) -> Capabilities {
        let account = item.key.message.account_id;
        Capabilities::new(
            item.key.clone(),
            match scope {
                NativeMutationScope::Imap { .. } => AdapterKind::DirectImap,
                _ => AdapterKind::Thunderbird,
            },
            !self.demo
                && self.live.read(cx).can_use_account(account)
                && !snapshot.account_errors.contains_key(&account),
            true,
            true,
            snapshot
                .folders
                .iter()
                .filter(|f| f.account_id == account)
                .map(|f| f.path.clone())
                .collect(),
        )
    }
    pub(crate) fn refresh_triage(&mut self, snapshot: &MailboxSnapshot, cx: &mut Context<Self>) {
        let mut sessions: Vec<_> = snapshot
            .adapter_sessions
            .iter()
            .map(|(id, n)| (*id, *n))
            .collect();
        sessions.sort_unstable();
        let mut epochs: Vec<_> = snapshot
            .mailbox_epochs
            .iter()
            .map(|(key, epoch)| (key.clone(), *epoch))
            .collect();
        epochs.sort_by(|a, b| a.0.cmp(&b.0));
        let stamp = ViewStamp {
            scope: snapshot.scope,
            account: snapshot.current_account_id,
            folder: snapshot.current_folder_path.clone(),
            query: snapshot.search_query.clone(),
            filter: snapshot.filter,
            grouped: self.preferences.group_conversations,
            sessions,
            epochs,
        };
        if self.triage.stamp.as_ref() != Some(&stamp) {
            let Some(generation) = self.triage.generation.checked_add(1) else {
                self.triage.notice =
                    "Selection lifetime exhausted. Restart before another batch.".into();
                return;
            };
            self.triage.generation = generation;
            self.triage.stamp = Some(stamp);
            if let Some(batch) = &mut self.triage.batch {
                batch.cancel();
            }
        }
        let visible: Vec<_> = self
            .triage_items(cx)
            .into_iter()
            .map(|(item, _)| item.key)
            .collect();
        self.triage.available = Arc::new(visible.iter().map(|key| key.message.clone()).collect());
        if let Err(error) = self
            .triage
            .selection
            .update_view(self.triage.generation, visible)
        {
            self.triage.notice = action_error(error).into();
        }
    }
    pub(crate) fn edit_triage_selection(
        &mut self,
        key: Option<MessageKey>,
        all: bool,
        range: bool,
        cx: &mut Context<Self>,
    ) {
        if self.triage.busy() || self.view != View::Mailbox || self.composer.is_some() {
            return;
        }
        let edit = if all {
            SelectionEdit::SelectAllLoaded
        } else if let Some(key) = key {
            let Some(scope) = self.live.read(cx).native_mutation_scope(&key) else {
                self.triage.notice = "This message's mailbox/session identity is unavailable. Refresh or open the message first.".into();
                cx.notify();
                return;
            };
            let key = scoped(&key, scope);
            if range {
                SelectionEdit::ExtendRange(key)
            } else {
                SelectionEdit::Toggle(key)
            }
        } else {
            SelectionEdit::Clear
        };
        self.triage.notice = match self.triage.selection.edit(edit) {
            Ok(()) => format!(
                "{} loaded messages selected. A batch is limited to 500 messages.",
                self.triage.selection.len()
            ),
            Err(error) => action_error(error).into(),
        };
        cx.notify();
    }
    pub(crate) fn prepare_single_rule_move(
        &mut self,
        key: MessageKey,
        scope: NativeMutationScope,
        destination: String,
        cx: &mut Context<Self>,
    ) {
        let snapshot = self.live.read(cx).snapshot();
        self.refresh_triage(&snapshot, cx);
        if self.triage.busy() || self.live.read(cx).native_mutation_scope(&key) != Some(scope) {
            return;
        }
        if self
            .triage
            .selection
            .edit(SelectionEdit::Replace(scoped(&key, scope)))
            .is_err()
        {
            self.triage.notice =
                "This message is no longer in the loaded selection. Refresh and preview again."
                    .into();
            cx.notify();
            return;
        }
        self.triage.open = true;
        self.begin_triage(Choice::Folder(destination), cx);
    }

    pub(crate) fn begin_triage(&mut self, choice: Choice, cx: &mut Context<Self>) {
        if self.demo
            || self.triage.busy()
            || self.triage.selection.is_empty()
            || self.view != View::Mailbox
            || self.composer.is_some()
        {
            return;
        }
        let snapshot = self.live.read(cx).snapshot();
        let selected: HashSet<_> = self.triage.selection.keys().cloned().collect();
        let items: Vec<_> = self
            .triage_items(cx)
            .into_iter()
            .filter(|(item, _)| selected.contains(&item.key))
            .collect();
        let caps: HashMap<_, _> = items
            .iter()
            .map(|(item, scope)| {
                (
                    item.key.clone(),
                    self.triage_caps(item, *scope, &snapshot, cx),
                )
            })
            .collect();
        let snapshots: Vec<_> = items.iter().map(|(item, _)| item.clone()).collect();
        let labels: HashMap<_, _> = self
            .triage
            .selection
            .keys()
            .map(|key| {
                let account = snapshot
                    .accounts
                    .iter()
                    .find(|a| a.id == key.message.account_id)
                    .map(|a| a.label.as_str())
                    .unwrap_or("Unavailable account");
                let subject = self
                    .list_rows
                    .iter()
                    .find(|r| r.row.key == key.message)
                    .map(|r| r.row.message.subject.as_str())
                    .unwrap_or("Message no longer loaded");
                (
                    key.clone(),
                    format!("{account} · {} · {subject}", key.message.folder_path),
                )
            })
            .collect();
        let mut plans = Vec::new();
        let actions: HashMap<_, _> = self
            .triage
            .selection
            .keys()
            .filter_map(|key| {
                let action = match &choice {
                    Choice::Unread(value) => Mutation::SetUnread(*value),
                    Choice::Starred(value) => Mutation::SetStarred(*value),
                    Choice::Folder(destination) => Mutation::MoveFolder {
                        destination: destination.clone(),
                    },
                    Choice::Role(role) => Mutation::MoveFolder {
                        destination: snapshot
                            .folders
                            .iter()
                            .find(|f| f.account_id == key.message.account_id && f.kind == *role)?
                            .path
                            .clone(),
                    },
                };
                if let Mutation::MoveFolder { destination } = &action {
                    plans.push(format!("{} → {destination}", labels[key]));
                }
                Some((key.clone(), action))
            })
            .collect();
        if actions.len() != self.triage.selection.len() {
            self.triage.notice = "An owning account has no discovered destination for that folder role. No messages were changed.".into();
            cx.notify();
            return;
        }
        let Some(id) = self.triage.next_id() else {
            self.triage.notice = "Operation IDs exhausted. Restart before another batch.".into();
            cx.notify();
            return;
        };
        let result = Batch::prepare_each(
            id,
            &self.triage.selection,
            &snapshots,
            |key| actions.get(key).cloned(),
            |key| caps[key].clone(),
        );
        match result {
            Ok(batch) => {
                self.triage.notice = if batch.requires_confirmation() {
                    "Review the exact destinations below, then confirm the move. Only selected loaded messages are included.".into()
                } else {
                    "Applying one message at a time; results are confirmed individually.".into()
                };
                self.triage.plans = plans;
                self.triage.labels = labels;
                self.triage.results.clear();
                self.triage.undoing = false;
                self.triage.batch = Some(batch);
                self.pump_triage(cx);
            }
            Err(error) => self.triage.notice = action_error(error).into(),
        }
        cx.notify();
    }
    pub(crate) fn undo_triage(&mut self, cx: &mut Context<Self>) {
        if self.triage.busy()
            || self.triage.undo.is_empty()
            || self.view != View::Mailbox
            || self.composer.is_some()
        {
            return;
        }
        let snapshot = self.live.read(cx).snapshot();
        let selected: HashSet<_> = self.triage.undo.iter().map(|r| r.key().clone()).collect();
        let items: Vec<_> = self
            .triage_items(cx)
            .into_iter()
            .filter(|(item, _)| selected.contains(&item.key))
            .collect();
        let mut selection = Selection::default();
        let keys = self.triage.undo.iter().map(|r| r.key().clone()).collect();
        if selection
            .update_view(self.triage.generation, keys)
            .and_then(|_| selection.edit(SelectionEdit::SelectAllLoaded))
            .is_err()
        {
            return;
        }
        let mut inverse = HashMap::new();
        for receipt in &self.triage.undo {
            let Some((item, _)) = items.iter().find(|(item, _)| &item.key == receipt.key()) else {
                self.triage.notice = "Undo needs the same loaded messages and verified mailbox/session. Return to that view and refresh.".into();
                cx.notify();
                return;
            };
            let Ok(action) = receipt.inverse_for(item) else {
                self.triage.notice = "A message changed after this batch. Undo was not submitted; review it manually.".into();
                cx.notify();
                return;
            };
            inverse.insert(item.key.clone(), action);
        }
        let caps: HashMap<_, _> = items
            .iter()
            .map(|(item, scope)| {
                (
                    item.key.clone(),
                    self.triage_caps(item, *scope, &snapshot, cx),
                )
            })
            .collect();
        let snapshots: Vec<_> = items.iter().map(|(item, _)| item.clone()).collect();
        let Some(id) = self.triage.next_id() else {
            return;
        };
        match Batch::prepare_each(
            id,
            &selection,
            &snapshots,
            |key| inverse.get(key).cloned(),
            |key| caps[key].clone(),
        ) {
            Ok(batch) => {
                self.triage.undoing = true;
                self.triage.batch = Some(batch);
                self.triage.plans.clear();
                self.pump_triage(cx);
            }
            Err(error) => self.triage.notice = action_error(error).into(),
        }
        cx.notify();
    }
    pub(crate) fn pump_triage(&mut self, cx: &mut Context<Self>) {
        if self.triage.batch.is_none() && self.triage.pending.is_none() {
            return;
        }
        if self.view != View::Mailbox || self.composer.is_some() {
            if let Some(batch) = &mut self.triage.batch {
                batch.cancel();
            }
        }
        let receipts = self
            .live
            .update(cx, |live, _| live.take_mutation_receipts());
        for (account, receipt) in receipts {
            let Some((key, expected)) = self.triage.pending.as_ref() else {
                continue;
            };
            if account != key.message.account_id || &receipt.request != expected {
                continue;
            }
            let completion = match receipt.outcome {
                NativeMutationOutcome::Succeeded => Completion::Succeeded,
                NativeMutationOutcome::Rejected(_) => Completion::Rejected,
                NativeMutationOutcome::Uncertain(_) => Completion::Uncertain,
            };
            let token = OperationToken {
                batch_id: receipt.request.token.batch_id,
                item_index: receipt.request.token.item_index,
            };
            if self
                .triage
                .batch
                .as_mut()
                .is_none_or(|batch| batch.complete(token, key, completion).is_err())
            {
                self.triage.notice = "The completion did not match the active operation. Pending work remains blocked for review.".into();
                continue;
            }
            self.triage.pending = None;
            self.live.update(cx, |live, cx| {
                live.refresh_mutated_folder(account, &receipt.request.path, cx)
            });
        }
        if self.triage.pending.is_some() {
            return;
        }
        let snapshot = self.live.read(cx).snapshot();
        let needed: HashSet<_> = self
            .triage
            .batch
            .as_ref()
            .map(|b| b.outcomes().map(|(key, _)| key.clone()).collect())
            .unwrap_or_default();
        let items: Vec<_> = self
            .triage_items(cx)
            .into_iter()
            .filter(|(item, _)| needed.contains(&item.key))
            .collect();
        let caps: HashMap<_, _> = items
            .iter()
            .map(|(item, scope)| {
                (
                    item.key.clone(),
                    (self.triage_caps(item, *scope, &snapshot, cx), *scope),
                )
            })
            .collect();
        let live = self.live.clone();
        let state = &mut self.triage;
        if let Some(batch) = &mut state.batch {
            for _ in 0..triage::ACTION_MAX {
                if state.pending.is_some() || batch.requires_confirmation() || batch.is_finished() {
                    break;
                }
                let mut accepted = None;
                let result = batch.submit_next(
                    state.generation,
                    |key| caps.get(key).map(|(cap, _)| cap.clone()),
                    |dispatch| {
                        let key = dispatch.key.clone();
                        let Some((_, scope)) = caps.get(&key) else {
                            return false;
                        };
                        let Some(mut request) = qualified(dispatch, *scope) else {
                            return false;
                        };
                        if state.undoing {
                            request.required_current_flag = match request.action {
                                NativeMutationAction::SetSeen(want)
                                | NativeMutationAction::SetFlagged(want) => Some(!want),
                                NativeMutationAction::Move { .. } => None,
                            };
                        }
                        if !live.update(cx, |live, cx| {
                            live.dispatch_native_mutation(
                                key.message.account_id,
                                request.clone(),
                                cx,
                            )
                        }) {
                            return false;
                        }
                        accepted = Some((key, request));
                        true
                    },
                );
                state.pending = accepted;
                if result.is_err() {
                    break;
                }
            }
            state.results = batch
                .outcomes()
                .map(|(key, outcome)| (key.clone(), outcome))
                .collect();
            if batch.is_finished() {
                state.undo = if state.undoing {
                    vec![]
                } else {
                    batch.undo_receipts()
                };
                let count = |want| state.results.iter().filter(|(_, got)| *got == want).count();
                let rejected = state
                    .results
                    .iter()
                    .filter(|(_, outcome)| matches!(outcome, ItemOutcome::Rejected(_)))
                    .count();
                state.notice = format!(
                    "Batch finished: {} confirmed, {} unchanged, {} failed, {} uncertain, {} rejected, {} canceled. Uncertain items require manual review; they were not retried.",
                    count(ItemOutcome::Succeeded),
                    count(ItemOutcome::Unchanged),
                    count(ItemOutcome::Failed),
                    count(ItemOutcome::Uncertain),
                    rejected,
                    count(ItemOutcome::Canceled)
                );
                state.batch = None;
                state.plans.clear();
                state.undoing = false;
            }
        }
    }
    pub(crate) fn triage_panel(&self, palette: Palette, cx: &mut Context<Self>) -> AnyElement {
        let busy = self.triage.busy();
        let disabled =
            self.demo || busy || self.triage.selection.is_empty() || self.composer.is_some();
        let confirmed = self
            .triage
            .batch
            .as_ref()
            .is_some_and(Batch::requires_confirmation);
        v_flex().w_full().gap_2()
            .child(Button::new("open-bulk-selection").ghost().small().label(if self.triage.open { "Hide selection controls" } else { "Select messages" })
                .disabled(self.demo || self.composer.is_some())
                .on_click(cx.listener(|this, _, _, cx| { this.triage.open = !this.triage.open; cx.notify(); })))
            .when(self.triage.open, |el| el
                .child(div().text_size(px(11.)).text_color(palette.muted).child("Select individual loaded messages. Collapsed conversations select only their displayed representative; expand a conversation to select its replies. Maximum 500 per action."))
                .child(h_flex().flex_wrap().gap_2()
                    .child(Button::new("select-all-loaded").ghost().small().label("Select available loaded messages").disabled(busy).on_click(cx.listener(|this, _, _, cx| this.edit_triage_selection(None, true, false, cx))))
                    .child(Button::new("select-range-loaded").ghost().small().label("Extend to focused message").disabled(busy).on_click(cx.listener(|this, _, _, cx| { let key = this.live.read(cx).snapshot().selected_key; this.edit_triage_selection(key, false, true, cx); })))
                    .child(Button::new("clear-bulk-selection").ghost().small().label("Clear selection").disabled(busy).on_click(cx.listener(|this, _, _, cx| this.edit_triage_selection(None, false, false, cx)))))
                .child(h_flex().flex_wrap().gap_2().children([
                    ("bulk-read", "Mark read", Choice::Unread(false)), ("bulk-unread", "Mark unread", Choice::Unread(true)),
                    ("bulk-star", "Star", Choice::Starred(true)), ("bulk-unstar", "Remove stars", Choice::Starred(false)),
                    ("bulk-archive", "Archive", Choice::Role(FolderKind::Archive)), ("bulk-trash", "Move to Trash", Choice::Role(FolderKind::Trash))
                ].into_iter().map(|(id, label, choice)| Button::new(id).ghost().small().label(label).disabled(disabled)
                    .on_click(cx.listener(move |this, _, _, cx| this.begin_triage(choice.clone(), cx))))))
                .child(h_flex().gap_2().child(Input::new(&self.triage.destination).aria_label("Destination folder path for selected messages").disabled(disabled).flex_1().min_w_0())
                    .child(Button::new("bulk-move-folder").ghost().small().label("Move to folder").disabled(disabled)
                        .on_click(cx.listener(|this, _, _, cx| { let path = this.triage.destination.read(cx).value().to_string(); this.begin_triage(Choice::Folder(path), cx); }))))
                .when(confirmed, |el| el.child(Button::new("confirm-bulk-move").secondary().small().label("Confirm these moves")
                    .on_click(cx.listener(|this, _, _, cx| { if let Some(batch) = &mut this.triage.batch { batch.confirm_destructive(); } this.pump_triage(cx); cx.notify(); }))))
                .child(Button::new("cancel-bulk-pending").ghost().small().label("Cancel pending work").disabled(!busy)
                    .tooltip("Already submitted work waits for its own receipt; cancel stops only items not yet submitted.")
                    .on_click(cx.listener(|this, _, _, cx| { if let Some(batch) = &mut this.triage.batch { batch.cancel(); } this.pump_triage(cx); cx.notify(); })))
                .child(Button::new("undo-last-flag-batch").ghost().small().label("Undo last confirmed flag changes").disabled(busy || self.triage.undo.is_empty())
                    .tooltip("Only confirmed flag changes can be undone. Moves have no destination identity receipt and require manual review.")
                    .on_click(cx.listener(|this, _, _, cx| this.undo_triage(cx))))
            )
            .when(!self.triage.notice.is_empty(), |el| el.child(div().text_size(px(11.)).text_color(palette.muted).child(self.triage.notice.clone())))
            .when(!self.triage.plans.is_empty(), |el| el.child(div().id("bulk-move-plan").max_h(px(160.)).overflow_y_scroll()
                .children(self.triage.plans.iter().cloned().map(|line| div().text_size(px(11.)).child(line)))))
            .when(!self.triage.results.is_empty(), |el| el
                .child(Button::new("bulk-result-details").ghost().small().label(if self.triage.show_results { "Hide item results" } else { "Show each item result" })
                    .on_click(cx.listener(|this, _, _, cx| { this.triage.show_results = !this.triage.show_results; cx.notify(); })))
                .when(self.triage.show_results, |el| el.child(div().id("bulk-item-results").max_h(px(160.)).overflow_y_scroll()
                    .children(self.triage.results.iter().map(|(key, outcome)| div().text_size(px(11.)).child(format!("{}: {}", self.triage.labels.get(key).map(String::as_str).unwrap_or("Message"), result_label(*outcome))))))))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn qualification_keeps_the_original_token_action_and_real_session() {
        let key = MessageKey {
            account_id: 7,
            folder_path: "INBOX".into(),
            uid: 42,
        };
        let scope = NativeMutationScope::Imap {
            uid_validity: 12,
            generation: 3,
            session: 9,
        };
        let dispatch = || triage::Dispatch {
            token: OperationToken {
                batch_id: 17,
                item_index: 2,
            },
            key: scoped(&key, scope),
            request: MailRequest::SetSeen {
                path: "INBOX".into(),
                uid: 42,
                seen: false,
            },
        };
        let request = qualified(dispatch(), scope).unwrap();
        assert_eq!(
            request.token,
            NativeMutationToken {
                batch_id: 17,
                item_index: 2
            }
        );
        assert_eq!(request.scope, scope);
        assert_eq!(request.action, NativeMutationAction::SetSeen(false));
        let mut wrong = dispatch();
        wrong.request = MailRequest::SetSeen {
            path: "Archive".into(),
            uid: 42,
            seen: false,
        };
        assert!(qualified(wrong, scope).is_none());
        assert!(
            qualified(
                dispatch(),
                NativeMutationScope::Imap {
                    uid_validity: 12,
                    generation: 4,
                    session: 9
                }
            )
            .is_none()
        );
        assert!(qualified(dispatch(), NativeMutationScope::Thunderbird { session: 9 }).is_none());
    }
}

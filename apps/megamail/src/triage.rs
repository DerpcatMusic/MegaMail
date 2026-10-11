//! Bounded native selection and mutation admission over the existing MailRequest
//! seam. No worker, transport, provider calls or optimistic mailbox mutation.
//! Parent must wire native request correlation before enabling dispatch; legacy
//! SeenSettled/MovesSettled/BulkComplete are NOT success receipts.

use std::collections::HashSet;

use megamail_core::worker::MailRequest;

use crate::live::MessageKey;

/// Loaded selection domain; never an implicit all-provider-history selection.
pub const VISIBLE_MAX: usize = 5_000;
/// One user action cannot enqueue more than this many item operations.
pub const ACTION_MAX: usize = 500;

/// Existing production message identity plus validated backend/mailbox lifetime.
/// Direct accounts supply the persisted mailbox generation; Thunderbird supplies
/// its message-map/session generation. Neither is an RFC Message-ID or folder ID.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ScopedMessageKey {
    pub message: MessageKey,
    pub session_generation: u64,
    pub mailbox_generation: u64,
}

impl ScopedMessageKey {
    fn valid(&self) -> bool {
        self.message.account_id != 0
            && self.message.uid != 0
            && valid_path(&self.message.folder_path)
            && self.session_generation != 0
            && self.mailbox_generation != 0
    }
}

/// Range extends selection; SelectAll means exactly the current loaded/filtered
/// visible rows, including individually represented conversation children only.
#[derive(Debug, Clone)]
pub enum SelectionEdit {
    Replace(ScopedMessageKey),
    Toggle(ScopedMessageKey),
    ExtendRange(ScopedMessageKey),
    SelectAllLoaded,
    Clear,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriageError {
    InvalidKey,
    InvalidGeneration,
    TooManyVisible,
    TooManySelected,
    DuplicateKey,
    NotVisible,
    MissingSnapshot,
    ConfirmationRequired,
    StaleView,
    InvalidCompletion,
    InvalidBatchId,
    InvalidAction,
    UndoConflict,
}

/// Selection lives on a view generation, not on row indices or a current account.
/// Updating scope/query/filter must increase the generation and clear selection.
#[derive(Debug, Default)]
pub struct Selection {
    generation: u64,
    visible: Vec<ScopedMessageKey>,
    selected: HashSet<ScopedMessageKey>,
    anchor: Option<ScopedMessageKey>,
}

impl Selection {
    /// Replace the visible domain atomically; oversized/duplicate/invalid inputs
    /// leave state unchanged. Same-generation refresh prunes missing selections.
    /// Selection never grows merely because pagination appended another page.
    pub fn update_view(
        &mut self,
        generation: u64,
        visible: Vec<ScopedMessageKey>,
    ) -> Result<(), TriageError> {
        if generation == 0 || generation < self.generation {
            return Err(TriageError::InvalidGeneration);
        }
        if visible.len() > VISIBLE_MAX {
            return Err(TriageError::TooManyVisible);
        }
        let mut unique = HashSet::with_capacity(visible.len());
        for key in &visible {
            if !key.valid() {
                return Err(TriageError::InvalidKey);
            }
            if !unique.insert(key.clone()) {
                return Err(TriageError::DuplicateKey);
            }
        }
        if generation != self.generation {
            self.selected.clear();
            self.anchor = None;
        } else {
            self.selected.retain(|k| unique.contains(k));
            if self.anchor.as_ref().is_some_and(|k| !unique.contains(k)) {
                self.anchor = None;
            }
        }
        self.visible = visible;
        self.generation = generation;
        Ok(())
    }

    /// Apply an atomic selection change. An over-limit range/select-all is an
    /// error, not a truncated selection presented as all matching messages.
    pub fn edit(&mut self, edit: SelectionEdit) -> Result<(), TriageError> {
        if self.generation == 0 {
            return Err(TriageError::InvalidGeneration);
        }
        let mut selected = self.selected.clone();
        let mut anchor = self.anchor.clone();
        match edit {
            SelectionEdit::Clear => {
                selected.clear();
                anchor = None;
            }
            SelectionEdit::SelectAllLoaded => {
                if self.visible.len() > ACTION_MAX {
                    return Err(TriageError::TooManySelected);
                }
                selected = self.visible.iter().cloned().collect();
                anchor = self.visible.first().cloned();
            }
            SelectionEdit::Replace(key) => {
                self.position(&key)?;
                selected.clear();
                selected.insert(key.clone());
                anchor = Some(key);
            }
            SelectionEdit::Toggle(key) => {
                self.position(&key)?;
                if !selected.remove(&key) {
                    selected.insert(key.clone());
                }
                anchor = Some(key);
            }
            SelectionEdit::ExtendRange(key) => {
                let end = self.position(&key)?;
                let start = match &anchor {
                    Some(key) => self.position(key)?,
                    None => end,
                };
                selected.extend(
                    self.visible[start.min(end)..=start.max(end)]
                        .iter()
                        .cloned(),
                );
                if anchor.is_none() {
                    anchor = Some(key);
                }
            }
        }
        if selected.len() > ACTION_MAX {
            return Err(TriageError::TooManySelected);
        }
        self.selected = selected;
        self.anchor = anchor;
        Ok(())
    }

    fn position(&self, key: &ScopedMessageKey) -> Result<usize, TriageError> {
        self.visible
            .iter()
            .position(|k| k == key)
            .ok_or(TriageError::NotVisible)
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn len(&self) -> usize {
        self.selected.len()
    }
    pub fn is_empty(&self) -> bool {
        self.selected.is_empty()
    }
    /// Deterministic visible order, retaining account/folder/epoch in every item.
    pub fn keys(&self) -> impl Iterator<Item = &ScopedMessageKey> {
        self.visible
            .iter()
            .filter(|key| self.selected.contains(*key))
    }
}

/// Semantic actions: labels are deliberately not folder moves or IMAP keywords.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mutation {
    SetUnread(bool),
    SetStarred(bool),
    /// Destination must be an already discovered folder in this SAME account.
    /// Every move requires confirmation, so a trash role cannot bypass consent.
    MoveFolder {
        destination: String,
    },
    /// No current native adapter implements Gmail label semantics; fail closed.
    Label {
        name: String,
        add: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdapterKind {
    DirectImap,
    Thunderbird,
    Unsupported,
}

/// A fresh folder-level capability snapshot. Flags/move reflect actual current
/// request handlers, not speculative Graph/JMAP/provider support. Results must be
/// correlated by the native adapter; default availability is not assumed.
#[derive(Debug, Clone)]
pub struct Capabilities {
    scope: ScopedMessageKey,
    adapter: AdapterKind,
    online: bool,
    epoch_validated: bool,
    correlated_results: bool,
    destinations: HashSet<String>,
}

impl Capabilities {
    /// Called off render from actual account/session state. Unknown UIDVALIDITY,
    /// read-only ACLs, unavailable account or uncorrelated results must fail closed.
    /// `online` means a usable owning adapter. The qualified backend must check
    /// actual message presence and persistent-write permission before success.
    pub fn new(
        scope: ScopedMessageKey,
        adapter: AdapterKind,
        online: bool,
        epoch_validated: bool,
        correlated_results: bool,
        destinations: Vec<String>,
    ) -> Self {
        let bounded = destinations.len() <= VISIBLE_MAX;
        Self {
            scope,
            adapter,
            online: online && bounded,
            epoch_validated,
            correlated_results,
            destinations: if bounded {
                destinations.into_iter().filter(|p| valid_path(p)).collect()
            } else {
                HashSet::new()
            },
        }
    }

    fn gate(&self, key: &ScopedMessageKey, action: &Mutation) -> Result<(), Rejection> {
        if !key.valid() {
            return Err(Rejection::InvalidIdentity);
        }
        if &self.scope != key {
            return Err(Rejection::StaleIdentity);
        }
        if self.adapter == AdapterKind::Unsupported {
            return Err(Rejection::Unsupported);
        }
        if !self.online {
            return Err(Rejection::Unavailable);
        }
        if !self.epoch_validated {
            return Err(Rejection::UnvalidatedEpoch);
        }
        if !self.correlated_results {
            return Err(Rejection::UncorrelatedBackend);
        }
        match action {
            Mutation::SetUnread(_) | Mutation::SetStarred(_) => Ok(()),
            Mutation::MoveFolder { destination, .. }
                if valid_path(destination)
                    && self.destinations.contains(destination)
                    && destination != &key.message.folder_path =>
            {
                Ok(())
            }
            Mutation::MoveFolder { .. } => Err(Rejection::InvalidDestination),
            Mutation::Label { .. } => Err(Rejection::UnsupportedLabel),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rejection {
    Unsupported,
    UnsupportedLabel,
    InvalidIdentity,
    Unavailable,
    UnvalidatedEpoch,
    UncorrelatedBackend,
    InvalidDestination,
    StaleIdentity,
    AdmissionFailed,
    BackendRejected,
}

/// Exact header snapshot used for flag no-op and supported flag undo only.
#[derive(Debug, Clone)]
pub struct SelectedItem {
    pub key: ScopedMessageKey,
    pub unread: bool,
    pub starred: bool,
}

/// No successful outcome is inferred from channel admission or a generic event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemOutcome {
    Pending,
    InFlight,
    Unchanged,
    Succeeded,
    Failed,
    /// Connection/timeout after submission: never retry automatically.
    Uncertain,
    Rejected(Rejection),
    /// Only work not yet admitted to a worker can be canceled here.
    Canceled,
}

/// Correlation envelope owned by the native integration, not the legacy GTK API.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OperationToken {
    pub batch_id: u64,
    pub item_index: usize,
}

/// Native adapter must carry this token AND the scoped key through execution.
/// The legacy MailRequest payload alone is insufficient to attribute success.
pub struct Dispatch {
    pub token: OperationToken,
    pub key: ScopedMessageKey,
    pub request: MailRequest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Completion {
    Succeeded,
    Rejected,
    Failed,
    Uncertain,
}

#[derive(Debug)]
struct Item {
    snapshot: SelectedItem,
    action: Mutation,
    outcome: ItemOutcome,
}

/// One batch with at most one in-flight item. Parent retains only one active
/// batch, bounded undo history, and a monotonically increasing batch ID.
/// This favors honest per-item outcomes over uncorrelated legacy MoveMessages.
#[derive(Debug)]
pub struct Batch {
    id: u64,
    view_generation: u64,
    items: Vec<Item>,
    confirmed: bool,
    cancel_requested: bool,
}

impl Batch {
    /// Prepare from the actual Selection interface and exact snapshots. Every
    /// selected key must appear exactly once; capability rejections are per-item.
    /// Missing/duplicate snapshots abort preparation without submitting anything.
    pub fn prepare(
        id: u64,
        selection: &Selection,
        snapshots: &[SelectedItem],
        action: Mutation,
        caps: impl FnMut(&ScopedMessageKey) -> Capabilities,
    ) -> Result<Self, TriageError> {
        validate_action(&action)?;
        Self::prepare_each(id, selection, snapshots, |_| Some(action.clone()), caps)
    }

    /// Resolve folder roles independently per owning account; the entire plan is
    /// validated before the first item can be admitted.
    pub fn prepare_each(
        id: u64,
        selection: &Selection,
        snapshots: &[SelectedItem],
        mut action: impl FnMut(&ScopedMessageKey) -> Option<Mutation>,
        mut caps: impl FnMut(&ScopedMessageKey) -> Capabilities,
    ) -> Result<Self, TriageError> {
        if id == 0 {
            return Err(TriageError::InvalidBatchId);
        }
        if selection.generation == 0 {
            return Err(TriageError::InvalidGeneration);
        }
        if snapshots.len() > ACTION_MAX {
            return Err(TriageError::TooManySelected);
        }
        let mut unique = HashSet::new();
        for item in snapshots {
            if !unique.insert(&item.key) {
                return Err(TriageError::DuplicateKey);
            }
        }
        let mut items = Vec::with_capacity(selection.len());
        for key in selection.keys() {
            let action = action(key).ok_or(TriageError::InvalidAction)?;
            validate_action(&action)?;
            let snapshot = snapshots
                .iter()
                .find(|s| &s.key == key)
                .ok_or(TriageError::MissingSnapshot)?
                .clone();
            let outcome = match caps(key).gate(key, &action) {
                Err(error) => ItemOutcome::Rejected(error),
                Ok(()) if unchanged(&snapshot, &action) => ItemOutcome::Unchanged,
                Ok(()) => ItemOutcome::Pending,
            };
            items.push(Item {
                snapshot,
                action: action.clone(),
                outcome,
            });
        }
        Ok(Self {
            id,
            view_generation: selection.generation,
            items,
            confirmed: false,
            cancel_requested: false,
        })
    }

    /// UI must show action, destination, selected count and loaded-only scope
    /// before confirmation. Calling this method is NOT an automatic UI consent.
    pub fn confirm_destructive(&mut self) {
        self.confirmed = true;
    }

    pub fn requires_confirmation(&self) -> bool {
        !self.confirmed
            && self.items.iter().any(|item| {
                item.outcome == ItemOutcome::Pending
                    && matches!(item.action, Mutation::MoveFolder { .. })
            })
    }

    /// Submit one item through the SAME Dispatch seam used by the native adapter.
    /// Some(token) means admission only, never success. None means no admission;
    /// inspect outcomes/is_finished to distinguish rejection from blocked/finished.
    /// At most one item is in flight. Admission failure is terminal, not success.
    /// `current` must supply fresh exact-key epoch/availability state; None means
    /// account/session/mailbox was removed/replaced. View generation mismatch
    /// cancels pending work but does not pretend to cancel an in-flight mutation.
    pub fn submit_next(
        &mut self,
        view_generation: u64,
        mut current: impl FnMut(&ScopedMessageKey) -> Option<Capabilities>,
        submit: impl FnOnce(Dispatch) -> bool,
    ) -> Result<Option<OperationToken>, TriageError> {
        if view_generation != self.view_generation {
            self.cancel();
            return Err(TriageError::StaleView);
        }
        if self.requires_confirmation() {
            return Err(TriageError::ConfirmationRequired);
        }
        if self.cancel_requested
            || self
                .items
                .iter()
                .any(|item| item.outcome == ItemOutcome::InFlight)
        {
            return Ok(None);
        }
        let Some(index) = self
            .items
            .iter()
            .position(|item| item.outcome == ItemOutcome::Pending)
        else {
            return Ok(None);
        };
        let item = &mut self.items[index];
        let allowed = current(&item.snapshot.key)
            .ok_or(Rejection::StaleIdentity)
            .and_then(|caps| caps.gate(&item.snapshot.key, &item.action));
        if let Err(error) = allowed {
            item.outcome = ItemOutcome::Rejected(error);
            return Ok(None);
        }
        let Some(request) = request_for(&item.snapshot.key, &item.action) else {
            item.outcome = ItemOutcome::Rejected(Rejection::Unsupported);
            return Ok(None);
        };
        let token = OperationToken {
            batch_id: self.id,
            item_index: index,
        };
        let admitted = submit(Dispatch {
            token,
            key: item.snapshot.key.clone(),
            request,
        });
        item.outcome = if admitted {
            ItemOutcome::InFlight
        } else {
            ItemOutcome::Rejected(Rejection::AdmissionFailed)
        };
        Ok(admitted.then_some(token))
    }

    /// Apply only an exact correlated completion. Duplicate, foreign or stale
    /// completion cannot create undo rights. Worker terminal status must come
    /// from the operation result, not from refreshed/absent list rows.
    pub fn complete(
        &mut self,
        token: OperationToken,
        key: &ScopedMessageKey,
        completion: Completion,
    ) -> Result<(), TriageError> {
        if token.batch_id != self.id {
            return Err(TriageError::InvalidCompletion);
        }
        let item = self
            .items
            .get_mut(token.item_index)
            .ok_or(TriageError::InvalidCompletion)?;
        if item.outcome != ItemOutcome::InFlight || &item.snapshot.key != key {
            return Err(TriageError::InvalidCompletion);
        }
        item.outcome = match completion {
            Completion::Succeeded => ItemOutcome::Succeeded,
            Completion::Rejected => ItemOutcome::Rejected(Rejection::BackendRejected),
            Completion::Failed => ItemOutcome::Failed,
            Completion::Uncertain => ItemOutcome::Uncertain,
        };
        if completion == Completion::Uncertain {
            self.cancel();
        }
        Ok(())
    }

    /// Stop only not-yet-submitted work. InFlight stays pending until a terminal
    /// result (or explicit Uncertain on session loss/timeout) arrives.
    pub fn cancel(&mut self) {
        self.cancel_requested = true;
        for item in &mut self.items {
            if item.outcome == ItemOutcome::Pending {
                item.outcome = ItemOutcome::Canceled;
            }
        }
    }

    pub fn outcomes(&self) -> impl Iterator<Item = (&ScopedMessageKey, ItemOutcome)> {
        self.items
            .iter()
            .map(|item| (&item.snapshot.key, item.outcome))
    }
    pub fn is_finished(&self) -> bool {
        self.items
            .iter()
            .all(|item| !matches!(item.outcome, ItemOutcome::Pending | ItemOutcome::InFlight))
    }

    /// Only confirmed successful flag changes have undo receipts. Moves change
    /// UIDs and current adapters do not return an unambiguous destination key;
    /// labels/purge are unsupported. No Message-ID-based speculative move undo.
    pub fn undo_receipts(&self) -> Vec<UndoReceipt> {
        self.items
            .iter()
            .filter_map(|item| {
                if item.outcome != ItemOutcome::Succeeded {
                    return None;
                }
                let inverse = match item.action {
                    Mutation::SetUnread(_) => Mutation::SetUnread(item.snapshot.unread),
                    Mutation::SetStarred(_) => Mutation::SetStarred(item.snapshot.starred),
                    _ => return None,
                };
                Some(UndoReceipt {
                    key: item.snapshot.key.clone(),
                    applied: item.action.clone(),
                    inverse,
                })
            })
            .collect()
    }
}

/// A successful flag change only. Fresh snapshot must still show the applied
/// state and the exact backend/mailbox generation before an inverse is prepared.
#[derive(Debug, Clone)]
pub struct UndoReceipt {
    key: ScopedMessageKey,
    applied: Mutation,
    inverse: Mutation,
}

impl UndoReceipt {
    pub fn key(&self) -> &ScopedMessageKey {
        &self.key
    }
    pub fn inverse_for(&self, current: &SelectedItem) -> Result<Mutation, TriageError> {
        if current.key != self.key || !unchanged(current, &self.applied) {
            return Err(TriageError::UndoConflict);
        }
        Ok(self.inverse.clone())
    }
    /// Build a single-item inverse through Batch::prepare, not a parallel fake path.
    /// No automatic retry; server races after the fresh snapshot remain possible.
    pub fn prepare(
        &self,
        id: u64,
        view_generation: u64,
        current: SelectedItem,
        caps: Capabilities,
    ) -> Result<Batch, TriageError> {
        self.inverse_for(&current)?;
        let mut selection = Selection::default();
        selection.update_view(view_generation, vec![self.key.clone()])?;
        selection.edit(SelectionEdit::Replace(self.key.clone()))?;
        Batch::prepare(id, &selection, &[current], self.inverse.clone(), |_| {
            caps.clone()
        })
    }
}

fn validate_action(action: &Mutation) -> Result<(), TriageError> {
    let invalid = match action {
        Mutation::MoveFolder { destination } => !valid_path(destination),
        Mutation::Label { name, .. } => {
            name.is_empty() || name.len() > 128 || name.chars().any(char::is_control)
        }
        _ => false,
    };
    if invalid {
        Err(TriageError::InvalidAction)
    } else {
        Ok(())
    }
}

fn valid_path(path: &str) -> bool {
    !path.is_empty() && path.len() <= 1024 && !path.chars().any(char::is_control)
}
fn unchanged(item: &SelectedItem, action: &Mutation) -> bool {
    match action {
        Mutation::SetUnread(unread) => item.unread == *unread,
        Mutation::SetStarred(starred) => item.starred == *starred,
        _ => false,
    }
}
fn request_for(key: &ScopedMessageKey, action: &Mutation) -> Option<MailRequest> {
    let path = key.message.folder_path.clone();
    let uid = key.message.uid;
    Some(match action {
        Mutation::SetUnread(unread) => MailRequest::SetSeen {
            path,
            uid,
            seen: !unread,
        },
        Mutation::SetStarred(starred) => MailRequest::SetFlagged {
            path,
            uid,
            flagged: *starred,
        },
        Mutation::MoveFolder { destination, .. } => MailRequest::MoveMessage {
            path,
            uid,
            dest: destination.clone(),
        },
        Mutation::Label { .. } => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(account: u32, path: &str, uid: u32) -> ScopedMessageKey {
        ScopedMessageKey {
            message: MessageKey {
                account_id: account,
                folder_path: path.into(),
                uid,
            },
            session_generation: 1,
            mailbox_generation: 1,
        }
    }
    fn caps(key: &ScopedMessageKey) -> Capabilities {
        Capabilities::new(
            key.clone(),
            AdapterKind::DirectImap,
            true,
            true,
            true,
            vec!["Archive".into(), "Trash".into()],
        )
    }
    fn selected(keys: Vec<ScopedMessageKey>) -> Selection {
        let mut selection = Selection::default();
        selection.update_view(1, keys).unwrap();
        selection.edit(SelectionEdit::SelectAllLoaded).unwrap();
        selection
    }
    fn snapshots(selection: &Selection) -> Vec<SelectedItem> {
        selection
            .keys()
            .map(|key| SelectedItem {
                key: key.clone(),
                unread: true,
                starred: false,
            })
            .collect()
    }
    fn batch(action: Mutation) -> Batch {
        let selection = selected(vec![key(1, "INBOX", 7), key(2, "INBOX", 7)]);
        Batch::prepare(1, &selection, &snapshots(&selection), action, caps).unwrap()
    }

    #[test]
    fn equal_uids_in_different_accounts_and_folders_stay_distinct() {
        let selection = selected(vec![
            key(1, "INBOX", 7),
            key(2, "INBOX", 7),
            key(1, "Sent", 7),
        ]);
        assert_eq!(selection.len(), 3);
    }

    #[test]
    fn range_is_inclusive_and_keeps_anchor() {
        let mut selection = Selection::default();
        let keys: Vec<_> = (1..=5).map(|uid| key(1, "INBOX", uid)).collect();
        selection.update_view(1, keys.clone()).unwrap();
        selection
            .edit(SelectionEdit::Replace(keys[3].clone()))
            .unwrap();
        selection
            .edit(SelectionEdit::ExtendRange(keys[1].clone()))
            .unwrap();
        assert_eq!(selection.keys().cloned().collect::<Vec<_>>(), keys[1..=3]);
    }

    #[test]
    fn select_all_rejects_oversize_without_truncating_or_changing_selection() {
        let mut selection = Selection::default();
        let keys: Vec<_> = (1..=501).map(|uid| key(1, "INBOX", uid)).collect();
        selection.update_view(1, keys.clone()).unwrap();
        selection
            .edit(SelectionEdit::Replace(keys[0].clone()))
            .unwrap();
        assert_eq!(
            selection.edit(SelectionEdit::SelectAllLoaded),
            Err(TriageError::TooManySelected)
        );
        assert_eq!(selection.len(), 1);
    }

    #[test]
    fn pagination_does_not_select_new_rows_and_scope_change_clears_selection() {
        let mut selection = selected(vec![key(1, "INBOX", 1)]);
        selection
            .update_view(1, vec![key(1, "INBOX", 1), key(1, "INBOX", 2)])
            .unwrap();
        assert_eq!(selection.len(), 1);
        selection.update_view(2, vec![key(2, "INBOX", 1)]).unwrap();
        assert!(selection.is_empty());
    }

    #[test]
    fn account_specific_destinations_confirm_before_serial_dispatch() {
        let selection = selected(vec![key(1, "INBOX", 7), key(2, "INBOX", 7)]);
        let mut batch = Batch::prepare_each(
            3,
            &selection,
            &snapshots(&selection),
            |key| {
                Some(Mutation::MoveFolder {
                    destination: if key.message.account_id == 1 {
                        "Archive"
                    } else {
                        "Trash"
                    }
                    .into(),
                })
            },
            caps,
        )
        .unwrap();
        assert_eq!(
            batch.submit_next(1, |key| Some(caps(key)), |_| panic!("must confirm first")),
            Err(TriageError::ConfirmationRequired)
        );
        batch.confirm_destructive();
        let mut destinations = Vec::new();
        for _ in 0..2 {
            let mut dispatched = None;
            let token = batch
                .submit_next(
                    1,
                    |key| Some(caps(key)),
                    |dispatch| {
                        match &dispatch.request {
                            MailRequest::MoveMessage { uid, dest, .. } => {
                                assert_eq!(*uid, 7);
                                destinations.push((dispatch.key.message.account_id, dest.clone()));
                            }
                            _ => panic!("wrong mutation"),
                        }
                        dispatched = Some(dispatch.key);
                        true
                    },
                )
                .unwrap()
                .unwrap();
            assert_eq!(
                batch.submit_next(
                    1,
                    |key| Some(caps(key)),
                    |_| panic!("only one physical flight")
                ),
                Ok(None)
            );
            batch
                .complete(token, &dispatched.unwrap(), Completion::Succeeded)
                .unwrap();
        }
        destinations.sort();
        assert_eq!(
            destinations,
            vec![(1, "Archive".into()), (2, "Trash".into())]
        );
        assert!(batch.is_finished());
    }

    #[test]
    fn dispatch_maps_actual_worker_request_and_owning_account() {
        let mut batch = batch(Mutation::SetUnread(false));
        let token = batch.submit_next(1, |key| Some(caps(key)), |dispatch| {
            assert_eq!(dispatch.key.message.account_id, 1);
            assert!(matches!(dispatch.request, MailRequest::SetSeen { path, uid: 7, seen: true } if path == "INBOX")); true
        }).unwrap().unwrap();
        assert_eq!(token.item_index, 0);
        assert!(!batch.is_finished());
    }

    #[test]
    fn legacy_uncorrelated_backend_cannot_dispatch() {
        let selection = selected(vec![key(1, "INBOX", 7)]);
        let batch = Batch::prepare(
            1,
            &selection,
            &snapshots(&selection),
            Mutation::SetStarred(true),
            |key| {
                Capabilities::new(
                    key.clone(),
                    AdapterKind::Thunderbird,
                    true,
                    true,
                    false,
                    vec![],
                )
            },
        )
        .unwrap();
        assert_eq!(
            batch.outcomes().next().unwrap().1,
            ItemOutcome::Rejected(Rejection::UncorrelatedBackend)
        );
    }

    #[test]
    fn labels_never_map_to_move_or_keyword_requests() {
        let batch = batch(Mutation::Label {
            name: "Work".into(),
            add: true,
        });
        assert!(
            batch
                .outcomes()
                .all(|(_, outcome)| outcome == ItemOutcome::Rejected(Rejection::UnsupportedLabel))
        );
    }

    #[test]
    fn trash_requires_confirmation_before_any_admission() {
        let mut batch = batch(Mutation::MoveFolder {
            destination: "Trash".into(),
        });
        assert_eq!(
            batch.submit_next(1, |key| Some(caps(key)), |_| panic!("must not dispatch")),
            Err(TriageError::ConfirmationRequired)
        );
        batch.confirm_destructive();
        assert!(
            batch
                .submit_next(1, |key| Some(caps(key)), |_| true)
                .unwrap()
                .is_some()
        );
    }

    #[test]
    fn stale_epoch_and_cross_account_completion_cannot_grant_undo() {
        let mut batch = batch(Mutation::SetStarred(true));
        let token = batch
            .submit_next(1, |key| Some(caps(key)), |_| true)
            .unwrap()
            .unwrap();
        assert_eq!(
            batch.complete(token, &key(2, "INBOX", 7), Completion::Succeeded),
            Err(TriageError::InvalidCompletion)
        );
        let mut stale = key(1, "INBOX", 7);
        stale.mailbox_generation = 2;
        assert_eq!(
            batch.complete(token, &stale, Completion::Succeeded),
            Err(TriageError::InvalidCompletion)
        );
        assert!(batch.undo_receipts().is_empty());
    }

    #[test]
    fn uncertain_operation_stops_the_remaining_batch_without_another_write() {
        let mut batch = batch(Mutation::SetStarred(true));
        let token = batch
            .submit_next(1, |key| Some(caps(key)), |_| true)
            .unwrap()
            .unwrap();
        batch
            .complete(token, &key(1, "INBOX", 7), Completion::Uncertain)
            .unwrap();
        assert_eq!(
            batch
                .outcomes()
                .map(|(_, outcome)| outcome)
                .collect::<Vec<_>>(),
            vec![ItemOutcome::Uncertain, ItemOutcome::Canceled]
        );
        assert!(batch.is_finished());
        assert_eq!(
            batch.submit_next(
                1,
                |key| Some(caps(key)),
                |_| panic!("no continuation after uncertainty")
            ),
            Ok(None)
        );
    }

    #[test]
    fn cancel_only_stops_unsent_items() {
        let mut batch = batch(Mutation::SetStarred(true));
        let token = batch
            .submit_next(1, |key| Some(caps(key)), |_| true)
            .unwrap()
            .unwrap();
        batch.cancel();
        assert_eq!(
            batch
                .outcomes()
                .map(|(_, outcome)| outcome)
                .collect::<Vec<_>>(),
            [ItemOutcome::InFlight, ItemOutcome::Canceled]
        );
        batch
            .complete(token, &key(1, "INBOX", 7), Completion::Uncertain)
            .unwrap();
        assert!(batch.is_finished());
        assert!(batch.undo_receipts().is_empty());
    }

    #[test]
    fn admission_is_not_success_and_only_one_item_can_be_in_flight() {
        let mut batch = batch(Mutation::SetUnread(false));
        batch
            .submit_next(1, |key| Some(caps(key)), |_| true)
            .unwrap();
        assert_eq!(
            batch.submit_next(
                1,
                |key| Some(caps(key)),
                |_| panic!("second in-flight operation")
            ),
            Ok(None)
        );
        assert!(batch.undo_receipts().is_empty());
    }

    #[test]
    fn confirmed_flag_undo_crosses_same_dispatch_interface() {
        let mut batch = batch(Mutation::SetUnread(false));
        let token = batch
            .submit_next(1, |key| Some(caps(key)), |_| true)
            .unwrap()
            .unwrap();
        batch
            .complete(token, &key(1, "INBOX", 7), Completion::Succeeded)
            .unwrap();
        let receipt = batch.undo_receipts().remove(0);
        let current = SelectedItem {
            key: key(1, "INBOX", 7),
            unread: false,
            starred: false,
        };
        let mut undo = receipt
            .prepare(2, 1, current, caps(&key(1, "INBOX", 7)))
            .unwrap();
        undo.submit_next(
            1,
            |key| Some(caps(key)),
            |dispatch| {
                assert!(matches!(
                    dispatch.request,
                    MailRequest::SetSeen { seen: false, .. }
                ));
                true
            },
        )
        .unwrap();
    }

    #[test]
    fn flag_undo_rejects_changed_snapshot() {
        let mut batch = batch(Mutation::SetStarred(true));
        let token = batch
            .submit_next(1, |key| Some(caps(key)), |_| true)
            .unwrap()
            .unwrap();
        batch
            .complete(token, &key(1, "INBOX", 7), Completion::Succeeded)
            .unwrap();
        assert!(matches!(
            batch.undo_receipts()[0].prepare(
                2,
                1,
                SelectedItem {
                    key: key(1, "INBOX", 7),
                    unread: true,
                    starred: false
                },
                caps(&key(1, "INBOX", 7))
            ),
            Err(TriageError::UndoConflict)
        ));
    }

    #[test]
    fn foreign_account_capabilities_reject_before_sender() {
        let mut batch = batch(Mutation::SetStarred(true));
        batch
            .submit_next(
                1,
                |_| Some(caps(&key(2, "INBOX", 7))),
                |_| panic!("wrong-account dispatch"),
            )
            .unwrap();
        assert_eq!(
            batch.outcomes().next().unwrap().1,
            ItemOutcome::Rejected(Rejection::StaleIdentity)
        );
    }

    #[test]
    fn unvalidated_epoch_blocks_mutation_even_with_online_channel() {
        let selection = selected(vec![key(1, "INBOX", 7)]);
        let batch = Batch::prepare(
            1,
            &selection,
            &snapshots(&selection),
            Mutation::SetUnread(false),
            |key| {
                Capabilities::new(
                    key.clone(),
                    AdapterKind::DirectImap,
                    true,
                    false,
                    true,
                    vec![],
                )
            },
        )
        .unwrap();
        assert_eq!(
            batch.outcomes().next().unwrap().1,
            ItemOutcome::Rejected(Rejection::UnvalidatedEpoch)
        );
    }

    #[test]
    fn stale_view_cancels_pending_without_canceling_admitted_work() {
        let mut batch = batch(Mutation::SetUnread(false));
        batch
            .submit_next(1, |key| Some(caps(key)), |_| true)
            .unwrap();
        assert_eq!(
            batch.submit_next(2, |key| Some(caps(key)), |_| panic!("stale view")),
            Err(TriageError::StaleView)
        );
        assert_eq!(
            batch.outcomes().map(|(_, o)| o).collect::<Vec<_>>(),
            [ItemOutcome::InFlight, ItemOutcome::Canceled]
        );
    }

    #[test]
    fn successful_move_does_not_offer_old_uid_undo() {
        let mut batch = batch(Mutation::MoveFolder {
            destination: "Archive".into(),
        });
        batch.confirm_destructive();
        let token = batch.submit_next(1, |key| Some(caps(key)), |dispatch| {
            assert!(matches!(dispatch.request, MailRequest::MoveMessage { path, uid: 7, dest } if path == "INBOX" && dest == "Archive")); true
        }).unwrap().unwrap();
        batch
            .complete(token, &key(1, "INBOX", 7), Completion::Succeeded)
            .unwrap();
        assert!(batch.undo_receipts().is_empty());
    }

    #[test]
    fn admission_failure_is_terminal_and_has_no_undo() {
        let mut batch = batch(Mutation::SetStarred(true));
        batch
            .submit_next(1, |key| Some(caps(key)), |_| false)
            .unwrap();
        assert_eq!(
            batch.outcomes().next().unwrap().1,
            ItemOutcome::Rejected(Rejection::AdmissionFailed)
        );
        assert!(batch.undo_receipts().is_empty());
    }

    #[test]
    fn malformed_visible_domain_is_atomic() {
        let mut selection = selected(vec![key(1, "INBOX", 7)]);
        assert_eq!(
            selection.update_view(2, vec![key(2, "INBOX", 8), key(2, "INBOX", 8)]),
            Err(TriageError::DuplicateKey)
        );
        assert_eq!(selection.keys().next(), Some(&key(1, "INBOX", 7)));
        assert_eq!(selection.generation(), 1);
    }

    #[test]
    fn completion_is_exactly_once_not_a_generic_settled_signal() {
        let mut batch = batch(Mutation::SetUnread(false));
        let token = batch
            .submit_next(1, |key| Some(caps(key)), |_| true)
            .unwrap()
            .unwrap();
        batch
            .complete(token, &key(1, "INBOX", 7), Completion::Failed)
            .unwrap();
        assert_eq!(
            batch.complete(token, &key(1, "INBOX", 7), Completion::Succeeded),
            Err(TriageError::InvalidCompletion)
        );
        assert!(batch.undo_receipts().is_empty());
    }

    #[test]
    fn session_removed_before_dispatch_is_per_item_rejection() {
        let mut batch = batch(Mutation::SetStarred(true));
        batch
            .submit_next(1, |_| None, |_| panic!("stale admission"))
            .unwrap();
        assert_eq!(
            batch.outcomes().next().unwrap().1,
            ItemOutcome::Rejected(Rejection::StaleIdentity)
        );
    }
}

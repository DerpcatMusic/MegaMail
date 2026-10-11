# Batch 3 native focus and compose integration — source handoff

## Status and ownership

Source implementation only. **No Cargo/rustc/build/compiler/test execution, lock resolution, private-profile inspection, credentials/keyring, real Thunderbird/provider, mail send, install, or default-handler operation was performed.** Rustfmt parsed/formatted only the two owned Rust files with `skip_children=true`; `git diff --check` checked Main whitespace. These are not Rust acceptance results.

Only these files were edited/created by this owner:

- `apps/megamail/src/main.rs` (existing parent changes preserved; only new declaration is `mod compose_tools_ui`).
- `apps/megamail/src/compose_tools_ui.rs` (new).
- `docs/research/batch3-native-compose-integration.md` (new).

No separate fixture file was needed; all seven authored tests embed fictional data. Frozen contacts/productivity/session/live/provider/core/manifests/locks/vendor files were not edited. **All owned source files are released; there is no further editing owner hold.** Parent owns source review, cross-track corrections, refreshed whole-batch source inventory/receipt, and the single grouped compilation after every owner releases.

Read repository AGENTS, PRODUCT, DESIGN, architecture, feature matrix/current implementation batches, all three reader-clipboard reports, Rust/codebase-design/impeccable guidance and pinned toolkit source. The initial `graft ask --source` returned no nodes; no graph rebuild/install occurred. Impeccable launcher was not run because its fallback can download/install tooling; project context and applicable craft/operate references were read directly. Zeron materials, palette, native reader and the closed original-formatting gate are unchanged.

## P0 production focus path

`MailApp::render` now tracks a real mailbox-root `FocusHandle` with the existing scoped mailbox key context. One admission latch schedules the pinned `Context::on_next_frame` callback after a frame mounts. It admits once on initial mailbox display and again after Setup/Compose/Settings or existing modal guards return to mailbox. The callback rechecks the real `ActionState`; it retains any focused handle contained in the mounted mailbox subtree. Thus Search, native TextView and mailbox buttons keep usable focus. Missing or retired Setup focus gets the rendered list wrapper, or the mounted root for an empty list. FocusMessages uses the same empty-list root fallback.

This is UI focus only: no I/O, global/unscoped shortcut, wrapper substituted for TextView, or body/header refresh focus loop. The existing identity-checked `FocusReader` still focuses `SemanticTextTarget.state` itself. The latch is not reset on ordinary mailbox data notifications.

**No startup runtime PASS is claimed.** The normal correction5 binary's actual startup CLIPBOARD owner=0 remains the last startup result. The parent's phase4 real consumer must be rerun against the newly reviewed/compiled binary with its new source receipt; inspect startup/Priya/returned-RTL results individually, not wrapper exit zero. GPUI test-support is not enabled in this package, and no dependency/features were added to fabricate a mounted-focus test. No mock-focus result is offered.

## P1 reachable production callgraph

Main composer → `compose_tools_view` → explicit **Read local contacts & literal text** → app-wide one-in-flight guard → `background_spawn` → `LocalContacts::open/load` and `PrivateLibraryStore::new/load`, plus `configured_signature` from the actual owning direct AccountConfig → immutable `Arc<Snapshot>` delivery. Reads never edit To/Cc/Bcc/Subject/Body or clear user tool inputs. Loading, unavailable, empty, corrupt/unsupported-store and busy states are visible.

Local-contact prefix input → **Find contacts** → actual `ContactSnapshot::complete(..., 20)` → native text result labels → explicitly chosen **Append to To/Cc/Bcc** → off-UI `recipient_edit` with actual worker `format_recipient`/`parse_recipients` validation, control-character rejection and cross-To/Cc/Bcc duplicate check → admitted editor delta. Existing typed recipient spelling is kept, not reconstructed. Forward recipient selection/caret is restored; a reversed selection is explicitly refused before mutation because the pinned public setter cannot restore its direction.

Editable name/address inputs → **Save new local contact** → background actual `LocalContacts::apply(displayed_revision, Create)` → immutable new scoped snapshot. CAS conflicts/errors invalidate the snapshot and require reload; uncertain durability does not blindly repeat creation. This is genuine local creation, not seeded suggestions. Update/delete/vCard UI remains unimplemented despite existing store primitives.

Chosen signature/snippet/template → background actual `render_item` and `insert_item` on the complete `OutgoingMessage` at the editor's actual UTF-8 caret → missing-variable/HTML/size/cursor rejection or checked delta → foreground `TextareaState::insert`. Selected body text is kept; insertion advances the caret normally. Template subject replacement is not offered. No whole-body `set_value`, automatic load insertion, HTML flattening or security flag change occurs. The real configured plain signature is placed first in the picker; no demo library entries are generated. Picker display caps at 20 items and reports truncation.

Each asynchronous delivery checks actual retained editor entity identity, stable account ID/backend/address, tools request revision, usable owning account, current view/editability and the original worker-qualified mailbox epochs (at most 128; larger scopes refuse tools). Mutation deliveries additionally use production `editor_unchanged`: full `compose_session::same` plus original caret/selection. Concurrent typing, hidden metadata changes and caret movement cause rejection, not overwrite. No result adopts the current account scope after arrival.

Successful recipient/body edits set the existing `Capture.dirty_input`, invalidate `draft_saved`, and call Main `capture_recovery` immediately. Existing Service observation/generation/lease/receipt paths remain authoritative; staged attachment leases, sensitivity, unsafe restored origins, send locks, Saved-vs-Sent cleanup and provider dispatch are not changed. Existing direct sender cycling now calls actual `choose_sender` against the owning enabled AccountConfig; adapters lacking that validator have switching disabled, while their existing chosen identity remains preserved. This validates configured selection, not provider send-as authorization.

## Limits and minimal parent contracts

- Literal library insertion is direct-account-only. Its frozen format scopes solely by numeric account ID, not backend. No same-ID Thunderbird composer can read it. Contacts use the real Direct/Thunderbird adapter qualifier; Direct is not hardcoded to IMAP and does not exclude the core's other direct protocols.
- Library editing is deliberately absent. `PrivateLibraryStore::save` has neither a revision/CAS parameter nor backend-qualified entries. Minimal producer correction for parent: a versioned backend-qualified library snapshot and `apply(scope, expected_revision, TextChange) -> snapshot` with bounded locked atomic writes, Conflict and DurabilityUncertain outcomes, plus explicit migration policy. Do not add a blind reload-modify-save UI writer.
- Live exposes real mailbox source epochs but no distinct per-account adapter incarnation. The implementation identifies the actual composer/editor session and rejects changed/disappearing epoch maps; it cannot distinguish a same-account reconnect that changes neither usable roster/address nor published epochs. If adapter-incarnation rejection is required in addition to local compose-session identity, parent must export a read-only owning-account stamp containing actual adapter kind, stable ID and producer session generation. No invented counter is used as that generation.
- For adapter sender switching, parent must supply its validated owning identities through the real `choose_sender` contract (or a separately reviewed equivalent); no synthetic AccountConfig/owned From identity is constructed.
- Operations are bounded in outstanding count and file/body/picker size by existing stores and this UI. Store filesystem latency itself has no exported cancellation/deadline contract. An outstanding job can finish against its original local scope after the composer closes; its UI result is discarded, never applied to a replacement composer.
- No DAV/system contacts/TB calendar/AI, encrypted-at-rest, variable editor, rich HTML, rule execution, snooze/schedule/undo-send, parsed-search or bulk-triage completion is claimed. The last two remain untouched/unwired.

## Authored, NOT EXECUTED regression tests

Seven ordinary Rust tests in the new module exercise the production transformations and existing real models/stores:

1. Signature/snippet/template UTF-8 insertion and actual Capture dirty generations; full Outgoing equality across all hidden/thread/security/calendar/origin/attachment fields.
2. Actual worker recipient formatting, quoted names, full-payload preservation, cross-field duplicates, header injection and wrong backend.
3. Missing variables, split UTF-8 cursor, wrong account and nonempty HTML rejection without mutation.
4. Production edit-delivery guard rejects concurrent body/Bcc/calendar/caret/selection changes.
5. Scope stamp distinguishes same-address account IDs, backends and actual mailbox source epochs; this scalar test is not a GPUI focus or adapter-reconnect proof.
6. Actual fictional RecoveryStore save/restore, owned attachment lease survival through literal transformation and Capture observation, retained origins/hold/retention and no forged Sent retirement.
7. Actual fictional LocalContacts CAS conflict preserves the newer book/revision.

These tests are authored source only. Actual mounted editor/recipient/focus dispatch and the clipboard oracle remain parent runtime gates.

## SHA-256 provenance

Owned Main input before this task: `187593f75ca835927379049d87cee67e9ac4ba2d28d9cf63bc77afeec0ff7495`.

Released source:

```text
e34e1914c2474b5f681628ca31354a8f990e392f2a96615bf6f66265eabd1143  apps/megamail/src/main.rs
2f7b93569cda27ae4a48357e70e7a282f258a104f2595efc1c1d81f78a4401a7  apps/megamail/src/compose_tools_ui.rs
```

Frozen producer hashes, verified unchanged before/after this owner:

```text
d605a3997bd919f14bb116e21363e9a52835a281ecea07193260a0ef8dd51c1b  apps/megamail/src/contacts.rs
d6d052edf2a219c8b1773a48ab07064aef014d9782d6e305a95d731be3e6ffd1  apps/megamail/src/compose_productivity.rs
7357d307e5e8d13b6b0615c63d14d42906f77be800e00f9e0f30df2a0be0f8cd  apps/megamail/src/compose_session.rs
```

Pinned API provenance (read, not edited):

```text
2fe230dc311a9dc9b0475e2b5bbe3fbd6b4f54251e38c2b1a977d806fa3ae3e0  vendor/gpui-pre/src/window.rs
af9c74ba63623cf4f107861e041cddb72cca5b5c65695ab6531bb44fbf7b3573  vendor/gpui-pre/src/app/context.rs
072e76d2bad177019da4ffecb5cfbe1a036ae1aef63c5bdb8933e1c14709e418  vendor/gpui-base/src/input/base/state.rs
```

No new dependencies or manifest/lock edits are requested for this handoff. Compile and runtime acceptance remain pending, not green.

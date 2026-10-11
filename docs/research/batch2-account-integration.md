# Batch 2: native account admission and integration handoff

## Freeze / evidence

Implementation owner releases the six source files listed below after formatting and source checks. Parent may now serialize integrations into them before the single batch build window. No Cargo/build/check/clippy/rustc or native/helper compilation was run by this owner. No app, Thunderbird process, real account, credential store, profile scan outside synthetic tests, provider login/send, or mailbox mutation was run. No commit/push. Existing working-tree foundation changes were present at entry; this report describes additions to that work, not a claim that the complete Git diff is new.

Fixed-point research is `d4dbd2b`. Previously reported foundation counts (core 471 pass/19 ignored, native 103 pass/1 private ignored, OAuth 4 public +22 unit pass) are historical parent evidence, **not new validation for this batch**. There is still no actual UI/live-provider proof here.

Owned sources changed:

- `apps/megamail/src/main.rs`
- `apps/megamail/src/live.rs`
- `apps/megamail/src/thunderbird_adapter.rs`
- `apps/megamail/src/account_session.rs` (foundation file extended)
- `apps/megamail/src/account_registry.rs` (new)
- `crates/mail-core/src/config.rs`

This report is the only new documentation written by this owner. Root GTK `src/config.rs`, core manifests/lib, worker/cache/query, compose/recovery, bridge runtime/extension, reader helper/resources, and other owners' modules were not edited.

## Implemented source changes

### P1: admission and durable ID allocation

`MailApp::connect_account` and `MailApp::import_selected_thunderbird` use `account_registry::mutation_blocked` before admission. Direct Add rejects an in-flight first Thunderbird import even before a saved TB roster exists. The direct button also uses `LiveMailbox::direct_add_block_reason`; the TB picker already includes direct `setup.connecting` in its disabled/loading input. Existing healthy mixed rosters are no longer blanket-blocked.

`account_registry::save_direct` re-reads the saved TB store and the complete direct stable-ID inventory, reserves new direct IDs **before** account/credential save, then reloads with TB IDs reserved. `config::profile_id_inventory` is metadata-only and includes removed direct-account tombstones. `config::reserve_profile_ids` and `load_profiles_with_reserved` use the existing config mutex and checked allocation. Conflicting stored IDs return an error; no silent renumbering. An unsuccessful subsequent credential/config save can leave a reserved direct tombstone; that is intentional protection against identity reuse, not an added account.

`config::load_profiles` retains its existing public shape and empty-external-reservation behavior for shared callers. Root GTK config is untouched. Native startup reads the TB inventory **before** any missing direct ID can be allocated. When ownership is unreadable or conflicts, `load_profiles_existing` restores only known direct IDs without ID allocation; startup blocks TB connection where its ID could own a direct cache/Outbox. Ordinary direct account-metadata errors do not suppress a valid independent TB inventory. Healthy existing workers/roster are not replaced with an error-derived empty list.

The serialized native admission rule plus existing single-instance guard is the application concurrency boundary. This is not a transaction spanning two manifests and a keyring, and it does not make arbitrary external edits safe while the app is running.

### F1: authorized intent survives initial connection failure

`thunderbird_adapter::import_selected` allocates against both durable inventories, including inactive direct IDs, with deterministic selected-source iteration and checked exhaustion. `account_session::commit_selection_with` validates the whole candidate manifest and persists every admitted, explicitly selected source **before** calling runtime startup. A failed persistence does not start transport or replace the in-memory roster. Failed initial login/folders/page attempts remain configured for next startup/Retry.

`MailboxEvent::ThunderbirdReady` now has:

```rust
inventory: Option<account_session::SourceStore>
```

Explicit import publishes `Some(store)` even when every runtime connection fails. Restore publishes `None` because its inventory was installed already. `LiveMailbox::install_saved_thunderbird` installs dormant rows; `install_thunderbird_ready` applies the same production Ready path to dormant/success/failure cases without replacing the sole mailbox/event owner. Ready must match the durable `(profile path, source ID, app ID)` identity before replacing a runtime. Import's healthy-account skip also compares qualified `SavedIdentity`, not numeric ID alone.

The manifest remains schema v1. Dormant rows still have honest generic names and no fabricated sender email. Unsupported/stale selections and inability to establish a canonical source identity are admission failures, not silently authorized new sources. No persisted per-provider login proof was added. Missing source, Unsupported, NeedsLogin and operation errors remain distinct visible outcomes with existing Retry/storage-repair controls.

### P2: one composer owner resolution

`LiveMailbox::composer_account_id` is called by both `open_new_message` and the Compose button. Unified Inbox prefers a usable selected-message owner, then usable current account, then usable roster member. Account-folder compose does not silently route to another account. Dormant/disconnected TB owners cannot send; direct accounts still retain offline durable-Outbox behavior.

`MailApp::on_live_change` now looks up `LiveMailbox::send_result_for(from_account_id, request_id)` instead of the current rail account's snapshot result. This matters when a healthy selected Unified Inbox owner differs from a dormant current account. Correlation is still account + request ID.

### Typed uncertain direct sends

The approved shared `WorkerEvent::SendUnknown { request_id, reason }` maps directly in `compact_worker_event` to the existing native `WorkerMailboxEvent::SendUnknown`. `record_send_completion` is the actual correlated completion seam used by the event handler and tests; wrong-account, wrong-request and duplicate completions cannot consume another pending send. `SendOutcome` public variants are unchanged. Unknown sets `SendResult.uncertain = true`, which the existing composer applies to its Send/Save/Close/edit/destructive-action lock. Normal Failed remains recoverable. No error-text classification is used for send uncertainty.

Outbox retry permission cannot be recovered from `OutboxItem.last_error` prose: that public shape does not carry a typed held/ready phase. The UI's former generic Retry is therefore disabled and labeled **Review required**, with an honest unavailable explanation. It is not a disguised informed-review command. Eligible automatic retries remain worker-owned, and ordinary `FlushOutbox` cannot release holds under the delivery owner's contract. A typed informed-review UI is **not implemented**. Existing explicit discard remains separate from retransmission permission.

### Typed bridge recovery

Integrated the transport owner's published `classify_bridge_error` / `BridgeFailureKind` contract into adapter classification first:

- Disconnected / Protocol: broken runtime; existing retirement path applies.
- RequestTimeout: request failure, never runtime retirement.
- Busy: retryable overload, never runtime retirement.
- Unsupported: compatibility repair, not invented authentication failure.
- NeedsLogin: explicit tagged runtime evidence only for this branch.
- Operation: ordinary operation failure, never inferred login/retirement from its prose.

`live::is_reconnectable_thunderbird_failure` also honors tags before legacy string fallback. Untagged compatibility fallback remains. The present extension's string-only errors are **not proof of primary-password/provider login recovery**.

### U1: immediate native reader repairs

The duplicated extracted URL list is collapsed by default behind a per-message **Link details** toggle; validated semantic anchors remain clickable without opening that panel. Opening a link remains explicit. No tracking URL is fetched/resolved, no URL rewritten, no remote-resource permission broadened.

Long native sender/address headers now wrap. Existing wrapping selectable From/To/Cc/Reply-to metadata remains; a selectable wrapping Date row exposes `Message::datetime_full()` without the short header's former fixed 90px ellipsis. Semantic/plain fallback and Zeron pane/theme structure are unchanged. No runtime narrow-window screenshot or accessibility validation was performed.

## Tests authored, not executed

Twenty-six new Rust tests cross the modified production seams (17 initial account/send/classification regressions plus 9 native epoch/metadata/reader-retirement regressions):

- `account_registry::tests`: inventory collision rejection without mutation; dormant/removed reservations/exhaustion; bidirectional import/direct admission.
- `config::tests`: native reserved allocator preserving existing/tombstone IDs; conflicting inventory without renumbering; exhausted ID space.
- `account_session::tests`: selected intent is on disk before a synthetic connection failure and survives restart; failed intent write never reaches connection or changes roster. These use explicit temporary synthetic profiles, not personal paths/keyrings.
- `live::account_integration_tests`: actual `LiveMailbox::install_thunderbird_ready` + `composer_account_id` with dormant current/healthy selected owner; failed import roster with unchanged healthy sender; wrong qualified Ready rejection; first import admission before saved TB exists; Unified-only fallback and disconnected-owner exclusion. Test construction uses the same `from_parts` constructor seam without starting workers/query/cache.
- `thunderbird_adapter::bridge_classification_tests`: typed timeout/busy/operation precedence; distinct disconnect/protocol/login/unsupported recovery.
- `main::tests`: actual direct WorkerEvent compact mapping + correlated live completion + owner/request result resolution + production composer action policy for Unknown and normal Failed.

Existing foundation restoration tests remain. These are not all UI-driven tests and do not establish network/provider behavior. Parent must compile and run them in the batch window after integrations. `rustfmt --edition 2024 --config skip_children=true` and `git diff --check` passed. Impeccable's mechanical detector returned `[]` for `main.rs`; native visual correctness was not proven by that source scan.

## Deferred H2: actual synchronous page head-of-line remains

Inspected `ActorState::handle_page` -> `page` -> `Runtime::call("list", params)`. Page list RPC is still synchronous on the actor. Body/discovery already use completion commands. **No page offload or latency improvement is claimed.**

A safe next slice needs a capped per-profile page-RPC admission lane and a `Command::PageFinished` completion, not an unbounded spawn. Capture owning account, folder ID/path, session epoch, query generation and cursor capability **at dispatch**. Validate again before mapping headers, consuming the old cursor, publishing unread counts or minting a new cursor. New first-page/search/folder work must supersede old completions; an append cursor cannot be consumed twice. Transport timeout/Busy must release that operation slot without retiring a healthy shared runtime. Add deterministic same-production dispatch/completion tests that hold one list RPC and deliver another account/body command, then reverse completions and cross folder/query/session boundaries. The concrete Runtime-owning actor did not have a runtime-neutral complete page seam for this owner to validate cheaply; a disconnected fake page benchmark would not satisfy the task. This was kept out of the bounded lifecycle batch.

## Exact parent integration points (after release)

### Provider auth

Keep `MailApp::connect_account` admission and `account_registry::save_direct` reservation-before-save for **all** future authorization additions, not just password setup. Provider owner can replace its connection-test/auth step, but must not call plain `config::save` then allocate IDs later. Keep the same `LiveMailbox` entity and `add_profile`; do not reconstruct workers on success. The parent owns module declarations/presentation and registration/config choices. No speculative provider UI was wired here.

### Draft recovery / composer routing

`mod draft_recovery` is now declared for the published compose dependency. At this owner's final source inspection, `compose::PreparedDraft` still has only `message`, `temporary_attachment_paths`, and `notice`; `RecoveredCompose` carries an actual `AttachmentLease`. Parent must move/retain that lease in `Composer` and clone it into any in-flight send/save/recovery capture until the correlated worker consumption/acknowledgement. Exact current insertion points are `Composer::new_prepared_draft`, the prepared-result branch in `MailApp::open_draft` that clones message/paths/notice, `send_current_message`, `save_current_draft`, `on_live_change`, and `cleanup_composer_attachments`. Do not drop a restored attachment lease merely on enqueue or strip it while converting a recovered model. No speculative PreparedDraft field or recovery UI was guessed here.

Use the composer message's `from_account_id`, not `snapshot.current_account_id`, for recovery/save completions, errors and sender choices. Send completion is now handled by `send_result_for`; draft-save/status acknowledgements are still largely current-account-shaped in the pre-existing UI and need the recovery owner's scoped contract. Preserve `composer_operation_pending`, `composer_can_edit`, `composer_can_close` and the uncertain lock when integrating recovery/desktop actions. Native close/shutdown recoverability is not declared complete by this account patch.

### Original HTML / completeness / encryption retention

Do not call the existing `ReaderBody.source_html` original HTML: it is sanitized semantic HTML. Exact insertion points:

1. Direct `live::compact_worker_event`, `WorkerEvent::Body` projection: retain an immutable bounded original decoded document **before** `body_content_and_links` / native sanitizer. Feed the reader-document owner on its background projection boundary, not from render.
2. TB `thunderbird_adapter::ActorState::parse_body_payload`: capture source HTML, real plain alternative, bridge completeness/truncation and verified resource descriptors **before** `mail_text::reader_html`. `BodyPayload` / its byte accounting / `.event` are the native bridge retention seam; update all of them together, not just one field.
3. `WorkerMailboxEvent::Body` / `MailboxSnapshot` / selected body retention / `ReaderBody::new` and `MailApp::render_reader` are the delivery/presentation points. Keep semantic/plain fallback and opaque authored canvas distinct from Zeron chrome.
4. TB needs actual chosen MIME/related-part/CID payloads from extension/runtime; never infer these from link extraction, filenames or flattened HTML. A truncated document must show an explicit incomplete/too-large state, not claim original fidelity.
5. Carry cache eligibility explicitly: preserve direct signed/decrypted/OpenPGP persistent-cache exclusions; TB decrypted/signed eligibility cannot be invented from sanitized HTML. Unknown eligibility must default to non-persistent. Do not persist HTML/resources/helper frames or screenshots for decrypted mail. Apply body/resource/frame/pixel budgets and clear scoped in-memory state on epoch/session retirement.
6. No remote fetch by default; helper resource/link access must use the document owner's nonce/message-scoped capability and explicit remote consent. Never load arbitrary sender URLs or filesystem paths on the GPUI thread.

No `reader_document`/`reader_host` module declaration, helper compilation, original renderer action or source-retention interface was guessed or integrated by this owner.

### Contacts / calendar: correlation blocker, not fake UI

`compact_worker_event` still drops legacy `WorkerEvent::SenderChecked` via its wildcard. That event has only `{ path, message_id, check }`; worker background prefetch also emits it. Stamping current selected metadata onto that late callback is unsafe. The final **direct versioned body** now retains its own producer-captured optional SenderCheck and attachment fact, and applies them only with accepted body scope/UID/source generation. Legacy checks and TB calendar MIME remain unresolved; no invitation projection/UI/RSVP was wired here.

Smallest required structured contract (additive event/request or equivalent captured producer envelope):

```text
BodyRequestIdentity {
  backend, account_id, folder_path, folder_id, uid, message_id,
  session_epoch, body_generation, request_token
}
SenderCheckedFor { identity: BodyRequestIdentity, check: SenderCheck }
```

The producer must echo the identity captured **when its body/check work was submitted**, not consult today's UI/cache epoch. Background unsolicited checks need their own producer-captured source epoch and must not masquerade as a selected request. Native projector should cap metadata off-UI, match the pending identity and current source/session epoch, then expose only a matching selected result to `invitation::from_sender_check`. TB must provide real calendar MIME evidence. Preserve typed `CommandTransportUnavailable` for RSVP; do not enable Accept/Send. Parent may add local contact completion from `LocalContacts`/`snapshot.complete(query, limit)` separately using the composer's actual backend/account scope and cap20, but this owner added no module declarations or presentation.

### F6/cache epoch retirement — implemented narrow body contract, supersedes initial handoff

Integrated the delivery owner's exact additive contract in `batch2-delivery.md`:

```text
LoadBodyAtEpoch { message_id, path, uid, expected_generation }
BodyAtEpoch { message_id, path, uid, generation, body,
              check: Option<SenderCheck>, has_attachments: Option<bool> }
BodyEpochStale { message_id, path, uid, expected_generation,
                 actual_generation: Option<u64>, reason }
MailboxEpochChanged { path, uid_validity, generation }
```

Account identity remains bound by the existing worker callback. `LiveMailbox` tracks direct IMAP accounts separately; TB session/synthetic IDs, Graph/JMAP/POP3 and demo keep their legacy request/body behavior. `prepare_body_request` is the actual selected-reader request seam and issues the versioned variant only for direct IMAP. Source generation and UID are captured before off-UI HTML compaction in `compact_worker_event`; legacy unversioned direct bodies/failures cannot consume versioned pending slots.

`apply_body_event` / `apply_body` require the owning account/path/message slot, pending UID, selected key, and exact pending/current source generation before applying body or metadata. A late old body cannot consume a newer request with reused numeric UID/message ID. Accepted bodies expose `selected_body_version` and optional `selected_sender_check` in the snapshot. The complete nested sender check (trust, unsubscribe, invite and PGP/sensitivity evidence) is retained only with an accepted body, with a 64 KiB off-UI Debug-format byte-count budget that allocates/logs no private formatting output; oversize checks become unknown, not truncated verification. `has_attachments: None` leaves the header fact unchanged. A missing check is unknown, not verified. No invitation/RSVP UI is declared integrated.

`retire_mailbox_epoch` removes that account/folder's header rows/page keys, pending bodies/versions, cursor/pending page generation, scoped attachment/draft source, and matching selection. Derived threads can span folders, so this account's derived thread cache is conservatively invalidated while other-folder page/header rows and other accounts remain. `MailboxSnapshot.mailbox_epochs` drives `MailApp::on_live_change` through `changed_mailbox_scopes` / `retire_reader_body_cache`, clearing scoped reader bodies, format/quote/link overrides and message expansion state. Parent must hook new original-document/helper resource/frame capabilities into this same retirement boundary; they were not yet wired by this owner.

**First-start/warm offline bootstrap:** an exact pending `expected_generation: 0` `BodyEpochStale` may initialize an absent namespace once from a nonzero `actual_generation`. The worker contract publishes that field only for a known persisted UIDVALIDITY namespace. It authorizes a **new** request after scoped header/reader retirement and reload, never relabels the rejected body and never overwrites an established namespace. `NativeMailboxEpoch.uid_validity == 0` represents an undisclosed UIDVALIDITY value from this structured cache bootstrap; nonzero source generation is the namespace fingerprint, not online server proof. A later explicit UIDVALIDITY disclosure at the same generation does not invalidate that generation. A missing namespace/read failure is terminal: the spinner ends, a scope retry guard prevents automatic header/body loops, and explicit Refresh can clear the guard. `reload_epoch_scope` schedules the dedicated cache page-query lane independently **before** an optional worker/server header reload, so a known-cache bootstrap does not wait for SELECT to rebuild local headers. Known direct namespaces use the published `QueryService::request_page_at_epoch` API; other callers retain `request_page`. No UI DB I/O or current-cache stamping of old events was added. These are source-wired paths; the actual GPUI/query/worker timing integration has not been executed.

Nine additional native Rust regressions are authored, unexecuted: queued old body → epoch reset → late old body → new reused UID with stale check/attachment rejection; unknown first-start terminal/no-loop; actual native versioned request/stale projection/bootstrap/cache-body apply without SELECT/transport; rapid selection; scoped retirement preserving other folder/account headers; direct-only legacy rejection; bounded sender-check retention; old bootstrap evidence cannot overwrite a known newer namespace; actual reader cache retirement.

**Residuals:** unversioned header queues, related/attachment/raw-export completions and legacy mutation requests do not gain full end-to-end source-epoch safety from this body patch. A late old header may still populate header metadata before a versioned body is requested; account/path/UID alone is not a provenance guarantee. Native body checks are narrower than full source/session/request-token correlation needed by an original-resource host or calendar action. Do not stamp a current cache generation onto those events or claim all mail actions are epoch-safe.

### Desktop activation/actions

Parent may replace existing inlined GPUI actions/bindings with the desktop owner's action namespace once; no duplicate action names were added here. Startup activation queue hooks belong after `main`'s instance acquisition and after `load_saved_accounts` establishes a usable owning account. Respect `composer_account_id`, pending/uncertain composer guards and explicit selected owner for reply actions; no activation implementation was guessed.

## Honest assessment

**Good:** direct/TB add cannot knowingly allocate over the other durable inventory; intent commits precede runtime connection; Ready and send-result ownership use real source/account/request scope; healthy mailbox workers survive sibling failure; immediate duplicate-link/date repairs are small and preserve existing fallback/privacy.

**Bad / unfinished:** H2 page head-of-line remains. Safe account removal/forget is not added because durable Outbox/drafts/cache/source-generation ownership needs a reviewed explicit deletion contract. Login state is not persisted as proof. Typed informed Outbox review, original-format reader, contacts/calendar presentation, OAuth UI, draft recovery/lease wiring and desktop activation are not integrated here. The final native body/metadata epoch contract and scoped native reader retirement are implemented in source, but headers, mutations and other legacy payloads remain incompletely correlated.

**Risk / blocker:** all changed Rust/tests still require parent compilation/execution. Cross-module contract compatibility, scoped draft results, source epochs and source/resource cache eligibility must be checked in serialized integration. Native wrapping/toggle accessibility and narrow-width behavior need actual UI inspection. No benchmark or performance gain, live-provider success, renderer integration or overall launch readiness is claimed.

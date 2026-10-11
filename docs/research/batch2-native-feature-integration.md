# Batch 2 — native feature integration and source release

Status: **owned native source frozen and released to the parent for grouped verification, including the requested conservative IMAP draft-origin guard. Not compiled, runtime-accepted, or shipped.**

Baseline HEAD remains `d4dbd2bba14f474a33870269cc15bd98376ff360`. The checkout already contained other owners' changes. HEAD diff totals are not an attribution ledger for this serial owner.

## Scope and constraints

This owner changed only:

- `apps/megamail/src/main.rs`
- `apps/megamail/src/live.rs`
- `apps/megamail/src/thunderbird_adapter.rs`
- New `apps/megamail/src/provider_setup.rs`
- New `apps/megamail/src/compose_session.rs`
- New `apps/megamail/src/reader_session.rs`
- New `apps/megamail/src/feature_integration.rs`
- This new report

No manifest, shared/GTK source, core producer, bridge JS, C/helper, prior handoff, credential store, installer or desktop setting was edited. Parent separately supplied the frozen `PreparedDraft.attachment_lease` field, query registration integration, and provider/recovery P2 repairs. Their public APIs were not changed here. Native-only body projections acquired additive original-source and draft-ticket state.

Graft was queried first and returned no matching nodes. Repository guidance, Rust/interface/UI guidance, relevant foundation/feature-matrix documents and producer handoffs were consulted. Large combined document outputs were sometimes truncated; this is not an exhaustive documentation-review claim.

**No Cargo, rustc, build/check/clippy, native/helper compilation, Rust-test execution, app/private-profile launch, credential/keyring access, real browser authorization, provider/TB login/send/mail mutation, commit or push occurred.**

## Implemented source paths and reachable controls

### 1. Browser authorization and native persistence

Setup now has separate Google browser sign-in (IMAP/SMTP) and Microsoft browser sign-in (Graph) controls. Microsoft audience cycles through personal, work/school and both. Registration discovery is off UI. Missing configuration disables authorization and explains the blocker; no client IDs, fake token exchanges or successful provider responses were invented.

`provider_setup::authorize` runs the published onboarding Transaction, ProviderHttp, SystemBrowser and ProviderProbe on the blocking executor. ReadyAccount and pending secret-bearing grants remain worker-local. The UI receives the cancellation handle and final checked profiles, not ReadyAccount. A pre-start atomic cancellation flag handles cancellation before the worker installs that handle. Existing producer checks govern later cancellation/staleness; blocking DNS/HTTP and credential finalization are not made interruptible by this caller.

The concrete `RegistryStore: AccountStore` loads the checked current direct roster and Thunderbird inventory under native direct-admission serialization. It checks duplicates and calls `account_registry::save_direct`, which reserves native IDs before the credential/config save. It does not use ConfigAccountStore followed by post-hoc ID allocation. Password/manual account admission now also reloads the checked roster under the same mutex rather than replacing it with an old UI snapshot. TB inventory with an unverified address blocks admission instead of silently omitting it from duplicate checks.

The UI then uses the existing `LiveMailbox::add_profile` connection path. Persisted is not connected: connection failure retains the saved roster and prior inventory. The documented post-save reload-failure prefix is explicitly surfaced as saved-but-not-reloaded and blocks further admission until repair/restart. Native mutex serialization does not establish process-wide token-rotation coordination or real provider acceptance.

### 2. Private local recovery, full capture and correlated results

One off-UI `compose_session::Service` opens the private `config_base()/draft-recovery` store and starts one RecoveryWriter. Retry uses that same live writer/store. A separately opened store is used only for read/list/restore, never to obtain unchanged-generation write permission. Draft keys combine a random 16-character session namespace with a checked 8-hex-digit counter; generations are checked rather than wrapping.

Actual InputEvent subscriptions mark To/Cc/Bcc/subject/body changes. Alias and attachment mutations and explicit HTML conversion also mark dirty. The bounded native event pump and send/save dispatch capture through the actual outgoing model. Full equality includes From/alias, recipients/Bcc, text and HTML, reply/thread fields, draft/outbox origins, security flags, calendar content/method, scheduling and attachments. Saved-state comparison uses the same equality seam.

Ordinary new user-authored messages opt into ordinary local retention. Reply/forward/prepared sources without a trusted sensitivity classification default to session-only; no decrypted plaintext retention or consent is inferred from absence of flags. TB without a validated native From AccountConfig is explicitly session-only. No sensitive-plaintext opt-in UI was invented.

The reachable recovery chooser lists bounded records for an explicitly current direct account and reports missing/partial/error state. Restore is off UI, validates that active account's source address/aliases through `prepare_local_recovery`, and retains the restored attachment lease. It rechecks the current account/view/composer before applying the result; stale delivery releases through the background reaper. It restores full outgoing metadata, not just visible fields. HTML remains present and editing is disabled until the user explicitly chooses plain-text conversion/discard. Conversion is dirty and explicit; no silent HTML stripping occurs.

**Recovered content is not resend authority.** Send remains disabled for recovered drafts because durable held/queued Outbox reconciliation is unavailable. Recovered provider Draft/Outbox origins also block provider draft replacement: those stored origins are not versioned mutation authority. A recovered fresh-origin plain composition can use the separate provider Save Draft path. Contacts/config identities are not synthesized to make TB restore look validated.

### Native IMAP original-draft retention guard

The confirmed legacy worker residual is not safe retirement: `run_imap` Send/SaveDraft can automatically call `delete_draft` using only account/path/UID; that helper re-SELECTs without a captured origin generation and uses generic EXPUNGE. A restored old UID can alias a new message, and EXPUNGE can remove other-client deleted messages. Body epochs and SendUnknown do not authorize this cleanup.

The exact production barrier is `LiveMailbox::send_request` → `prepare_native_dispatch`, immediately before the native request sender. Classification comes from the actual registered `direct_imap_accounts` set. For direct IMAP Send and SaveDraft only, it clones the outgoing message inside the request and clears **only that dispatch clone's `draft_origin`**. All other outgoing fields, request IDs/folder/path/tickets and non-Send/SaveDraft requests remain unchanged. Composer, F7 capture/record/restore and attachment lease ownership retain the full old origin. No current generation is stamped onto it. Other backends are not given a new retirement protocol.

The composer shows a visible notice that IMAP uses a separate copy and retains the original server draft for manual review; no automatic original deletion/EXPUNGE is authorized by that composer dispatch. Provider Save Draft is therefore copy creation, not safe replacement lifecycle. Recovered origin-bearing provider mutation remains conservatively UI-blocked as described above; the request boundary is defense-in-depth if such an origin-bearing request reaches it. Previously persisted Outbox intents/background worker paths, GTK and other header/mutation residuals are not repaired by this native-only guard.

The added native regression uses the actual recovery preparation/store/restore, full outgoing comparator, actual mailbox epoch-retirement helper and this exact dispatch boundary for Send and SaveDraft. It restores a full origin, changes UIDVALIDITY/native epoch and reuses the UID, then verifies that dispatched messages cannot enter the worker's origin-based delete branch while hidden metadata, encryption/signing/schedule/calendar fields, attachment bytes, full composer capture and durable origin remain intact. It also exercises the actual HeldAttachments consumer lease after view capture/restore owners drop. This is authored source, not an executed worker/provider/EXPUNGE test.

A separate owner handles the encrypted durable-Outbox plaintext-preview correction. This owner did not edit worker code or change that API. The native guard preserves encrypt/sign, content and envelope metadata; it does not establish encrypted MIME/claims/privacy acceptance for that separate correction.

Provider draft dispatch carries a checked native ticket via the existing autosave field. Live state accepts only a ticket already pending for the exact account. Foreign, absent and duplicate DraftSaved events cannot unlock the composer; receipt consumption is one-time. Legacy error/status prose lacks that identity and therefore does not release a pending save or attachment lease. A second replacement save after successful provider save is blocked until a genuinely versioned remote origin is available. This is correlated source wiring, not real server-draft acceptance evidence.

Actual send IDs are checked/nonwrapping and existing account/request SendFinished projection remains authoritative. Only a matching definitive Sent result supplies the recovery SendFinished command. Queued/unknown/failed results do not clear recovery. **Saved and retirement acknowledgments are separate**: a pre-send Saved receipt cannot satisfy Sent cleanup. The native composer awaits the exact generation's Discarded receipt, and keeps the actual Sent account/request/generation for cleanup-only retry through the same writer. Retry does not dispatch Send. `removed=false` is explicitly reported as no exact-generation removal/possible older copy, not successful deletion. Exact older-copy repair and post-unlink directory-sync uncertainty remain acceptance/residual gates; no crash-safe delete claim follows.

Ordinary close attempts capture first and refuse to replace/drop a composition while its required current acknowledgment is missing. Close is not application-shutdown flush acknowledgment. Generic Close copy warns that session-only text is lost; it does not promise a durable copy when recovery initialization failed. Initialization errors are also visible in the composer. Native window/process shutdown has no acknowledged flush/worker-join seam and is **not crash-safe close**.

Restored leases are held independently for exact accepted send/save tickets and released off UI only on the corresponding terminal receipt. The composer retains its own lease while still visible. Uncorrelated save failure, unacknowledged request shutdown or a saturated/unavailable reaper deliberately retains staging rather than deleting worker-consumed paths. This is an explicit cleanup leak until verified startup pruning, not graceful retirement. Original non-lease prepared paths retain their existing lifecycle; no universal lease/teardown proof is claimed.

### 3. Immutable source and gated temporary original reader

Direct and TB body producers capture bounded original decoded HTML **before** native sanitization. The selectable native reader continues to use the sanitized semantic body. Original source is never reconstructed from semantic HTML. Original bytes, sender metadata and selected body version are accepted atomically with the same account/UID/version body envelope; foreign/stale delivery and epoch/selection retirement clear the associated state. TB payload byte accounting includes the separately retained source.

Direct selected MIME completeness is not attested and remains fail-closed (`Truncated`, not Complete). TB derives completeness only from an explicit truncated boolean; its cache eligibility stays unknown without actual sender/security metadata. Missing/oversized sources and unknown/signed/decrypted cache policy are not relabeled as eligible cached HTML. No original-source disk cache was added and no CID MIME transport was invented.

The reachable Original formatting action builds ReaderDocument and starts the actual ReaderHost off UI. Scope includes stable native account/folder-path/message identity, native source generation and checked reader generation. Host events are bounded and scoped; current selection/body/epoch/view/window changes retire/cancel the host. Generation exhaustion disables further admission. Only verified helper click events with a current scope and safe HTTP(S) URL open a browser off UI.

Release remains disabled by default. Explicit developer opt-in requires both `MEGAMAIL_HTML_READER_DEVELOPER_PREVIEW=1` and an absolute canonical `MEGAMAIL_HTML_READER_HELPER`. Unix file/ancestor ownership and non-group/world-writable checks reject untrusted paths. These checks are not hostile same-user executable-race resistance, helper security acceptance or renderer fidelity. The helper runs in a **temporary separate window**, not embedded GPUI. Ready text describes a reported policy claim, not proved resource blocking. Developer fixtures must be fictional; this opt-in is not a release gate for real mail.

### 4. Desktop activation and shared action policy

Normal startup uses args_os and accepts only no argument, exact `--demo`, or one bounded UTF-8 mailto URI. It does not lossy-convert or re-decode. The actual TB-host branch remains first. Normal Linux startup calls `instance::start_activation` before GPUI/workers. Forwarded exits successfully; malformed/error paths exit before UI/worker creation, without guessed takeover or URI logging. Demo is deliberately excluded from normal activation.

The activation inbox is retained through setup and the app. Queued validated intent is drained only when the real ActionState allows composition, no composer/modal blocks it, and the user explicitly confirms the current usable From account through Open queued email link. Default/Unified selection alone is not authorization. The validated payload is passed to `desktop_actions::compose_message`; it opens an unsent composer and never Send. Compose status does not bypass account readiness.

The primary owner is deliberately retained until OS process teardown because current workers have no joined retirement acknowledgment. This prevents early unlock while an admitted worker may still operate. The next startup's existing verified stale-socket handling owns recovery. It is not graceful socket/worker shutdown or a flush receipt. System association/package acceptance and cross-process Linux runtime tests remain separate.

Native bindings use `desktop_actions::action`, `bind_keys` and mailbox-only context. Compose/reply/reply-all, movement/search/refresh/unified and pane/retained-reader focus use the shared ActionState; reply/refresh toolbar admission is guarded too. Composer/setup/preferences are not mailbox keyboard context. This is source-level parity, not an executed focus/IME/editor usability test.

### 5. Small P1 integration and deliberately unwired modules

An accepted direct BodyAtEpoch sender-check projection now produces a read-only invitation preview off UI through the actual invitation producer. Delivery checks current account/message key, native body version and reader generation; stale account/body/epoch results are rejected. Date values retain explicit timezone/floating semantics in the preview. Typed RSVP state distinguishes no response requested, cancellation and unavailable command transport. No RSVP, calendar mutation or response email is dispatched. TB/missing metadata remains unknown/unavailable.

Contacts, invitation, productivity, rules and triage modules are declared so their frozen source/tests participate in native compilation. **Declarations are not feature completion.** Contacts CRUD/completion/group expansion, signatures/snippets/templates, rules preview/automation, parsed-search routing/coverage UX, and correlated bulk triage remain unwired. Existing literal search/direct/TB/demo behavior is retained. Parent QueryService predicate registration is not native parsed/indexed/body search, and no indexing, latency or coverage improvement is claimed.

## Authored tests — all uncompiled and unexecuted

Nineteen new native Rust test declarations:

- Provider setup (4): duplicate policy; actual checked adapter roster/ID preservation; no persistence call for duplicate; honest saved/reload-failure state. Saver closures are synthetic seam fixtures, not real config/keyring acceptance.
- Compose session (5): hidden/full dirty generation; checked overflow; actual Service/RecoveryWriter/prepare/restore path with fictional full outgoing metadata and background lease cleanup; distinct scoped retirement versus save acknowledgment; recovered Outbox origin is not provider replacement authority.
- Reader session (4): original versus sanitized semantic source; direct completeness/eligibility; scoped host acceptance; actual invitation selection guard across account/version/epoch changes.
- Startup (2): strict actual args_os-facing ingress policy and no re-decoding/lossy conversion.
- Live projection/dispatch (3): actual compact body envelope/atomic original+sender acceptance and epoch retirement; actual compact DraftSaved account/ticket admission and one-time consumption; full actual recovery/epoch-change/Send-and-SaveDraft dispatch/lease regression for original server-draft retention.
- TB adapter (1): actual payload's original preservation, explicit completeness/unknown eligibility, byte accounting and body-event propagation.

The Service fixture writes only a private synthetic temp directory with example.test identities, flushes the actual writer, restores through the actual native prepare seam, and awaits its lease-reaper fixture cleanup. Reopening the read store for restore does not retry a write. No substitute production implementation was added to make these tests pass. Writer/transport/helper acceptance still requires compilation and execution by the parent.

## Executed checks and honest limitations

Executed successfully on owned files:

- Direct rustfmt, edition 2024, skip_children=true, followed by direct --check. It parses/formats source; it is not type checking.
- Scoped git diff --check for the three tracked native files; explicit Python whitespace/NUL scans cover all seven owned source files, including untracked additions.
- Sixteen Python source-order/call-site assertions, one additional recovered-origin guard assertion across the same production button/dispatch seam, and final native IMAP dispatch-source assertions: actual pre-sender invocation, direct-only Send/SaveDraft clone/clear, visible retention notice, full epoch/recovery/lease regression declaration and no other outgoing-field assignments. These are **text checks only**, not Rust, security, provider, parser or runtime tests. Early audit drafts had incorrect symbol/count markers; they were corrected to actual source, not used to change production behavior.

No app UI screenshot, focus journey, performance measurement, crash/power-loss test, OAuth/TLS/probe test, TB launch, helper security/fidelity test, IPC runtime test or Rust test pass is established. Formatting and source review do not establish type/borrow/API correctness. Parent integrated review must examine this native caller wiring, not infer acceptance from producer-only review.

## Parent grouped verification and remaining gates

After this release, parent owns the single serial build/test window and any necessary type/API repairs. A prepared native command, **not executed here**, is:

```sh
cargo test --manifest-path apps/megamail/Cargo.toml --bin megamail -- --test-threads=1
```

Combine it with the parent-planned core/cache/query/provider/recovery suites in that same window. Do not compile concurrently from child owners. Run the provider ignored SYN-blackhole test only in its documented fresh rootless loopback-only network namespace; namespace/calibration failure stays blocked, never a transport pass. Preserve the same live recovery writer/store when exercising post-rename unchanged-generation retry.

Required acceptance/review journeys:

1. Actual checked persistence/cancellation and duplicate direct/TB roster preservation; connection failure after save; real owned registrations, TLS probes and token rotation separately, with explicit authorization.
2. Native mutation capture, full restored From/hidden metadata, session-only/decrypted exclusions, current-generation retry, send/save ticket ownership, exact Sent cleanup receipt and cleanup-only retry; queued/unknown/older record handling and stale restore delivery. Confirm no asynchronous provider draft replaces an already opened composer. Execute the actual native origin-dispatch regression and inspect both worker requests: no IMAP draft_origin, no restamping, unchanged hidden fields/lease, visible original-retained/manual-review notice. Original server cleanup stays denied absent a generation-qualified selective retirement contract. Separately verify the worker owner's encrypted Outbox preview correction.
3. Lease survival during consumer/save/send/close and reaper failure; uncertain request/process shutdown; original non-lease staging cleanup; acknowledged close/shutdown remains blocked absent a real seam.
4. Default reader rejection, incomplete/direct source fallback, immutable-original fidelity, helper path/parent rejection and current scope cancellation. Helper subprocess, link-token, sandbox/resource blocking and faithful fictional rendering need the helper owner's security/runtime suite. Do not test real mail as the developer opt-in acceptance path.
5. Actual Linux duplicate-instance forwarding/error/unknown outcome, explicit From confirmation, bounded backlog and profile/worker startup ordering; retained lock through process termination and stale socket handling; packaging association separately.
6. Shared focus/toolbar/keyboard policy across compose/setup/preferences/search/reader and rapid selection/account/epoch switches, plus stale invitation/no-RSVP cases. Preserve inherited F6 source/body/cache lanes; unversioned legacy header/mutation/session/draft-origin residuals are not silently resolved here.

**Good:** actual bounded native producer/registry/recovery/host/activation call paths and real UI controls replace infrastructure-only handoffs; immutable sources and correlated receipts stay separate from sanitized display/status prose; missing capabilities fail closed.

**Bad/incomplete:** no compiled/native journey proof; developer helper only; restored resend and recovered provider-origin replacement blocked; save failures can stay locked; window shutdown is unacknowledged; conservative lease/primary retention needs startup repair; P1 managers/search/rules/triage remain unwired.

**Risks/blockers:** source type/borrow/API verification; owned registrations and real protocols; DNS/in-flight cancellation/token coordination; exact old-copy/deletion durability; versioned remote origins and durable Outbox reconciliation; helper security/fidelity/embedded GPUI; actual activation/focus behavior and responsive layout. No shipped completeness, production readiness, universal cleanup, security certification or performance gain is claimed.

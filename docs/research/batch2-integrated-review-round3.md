## Standards — frozen integrated batch

**Baseline:** `d4dbd2bba14f474a33870269cc15bd98376ff360`; HEAD matches, with no intervening commits. Reviewed working-tree changes and untracked native modules, tests, helper sources and verification scripts—not the empty committed three-dot diff. Current `src/worker.rs` SHA-256 matches the supplied calendar-guard receipt.

Graft returned no matching nodes. Inspection was read-only: no edits, builds, tests, application launches, credentials, provider operations or commits.

### Confirmed hard documented-standard breaches

**None established.** This is source-review disposition, not compilation or runtime acceptance.

Relevant positive findings:

- **Off-UI work:** `main.rs::{begin_provider_authorization,restore_local_recovery,open_original_formatting,connect_account}` dispatch persistence, protocol work, restore and helper preparation to background execution. `compose_session::LeaseDelivery` routes stale attachment cleanup through the bounded reaper; `ReaderHost::drop` cancels without joining on UI. These follow **AGENTS.md**, “Mail, database queries, parsing and thumbnail decoding belong off the UI thread,” and the no-render-I/O rule.
- **Shared engine and compatibility:** query registration uses `Cache::read_connection_with`; shared Cache/GTK do not import native search. Worker additions preserve legacy request shapes, with additive GTK event handling. This respects **AGENTS.md**’s shared-caller requirement and **MEGAMAIL_ARCHITECTURE.md — Decision**’s retained Hylki engine.
- **Privacy/integrity:** `queue_built_outbox` branches on encryption before reading plaintext body for preview. `build_message` refuses calendar crypto/files rather than downgrading or dropping them. `LiveMailbox::prepare_native_dispatch` clears only the direct-IMAP dispatch copy’s unversioned draft origin. Full composer/recovery metadata remains intact.
- **Isolation and honesty:** separate namespaces and upstream notices remain; no new competing mail engine was introduced. Original-source completeness, unknown eligibility, correlated body metadata and unavailable capabilities remain explicit rather than being presented as verified fidelity.
- **Verification safety:** `verify-megamail-batch2.py` is plan-only by default, requires a separate matching freeze receipt and runs serial bounded steps. Native QA requires isolated HOME/XDG, network/PID namespaces and an owned Xvfb; observations are not feature passes. Neither introduces a sandbox-disabled fallback.

### Material Fowler heuristics — judgment calls, not hard bugs

1. **P3 — possible Primitive Obsession / Repeated Switches: persistence outcome encoded in prose.**
   `account_registry.rs:save_direct`, `provider_setup.rs:RegistryStore::save_checked`, and `main.rs` authorization/password completion branches recognize `"The account was saved, but its profile could not be reloaded:"`. Current matching is consistent, but copy changes could alter admission-blocking semantics.

   **Smallest improvement:** a private typed `SavedButReloadFailed` outcome, mapped to text at presentation; retain existing public compatibility signatures. **Regression:** induce post-save reload failure through the actual registry/consumer seam and verify saved-state reporting and continued admission blocking independently of message wording.

2. **P3 — possible Primitive Obsession: lease-ticket namespace uses a Boolean.**
   `compose_session.rs:HeldAttachments::{hold,finish}` keys leases by `(bool, u32, u64)`; callers use `true` for send and `false` for draft save. Current callers agree, but transposed Boolean meaning is easy to miss around resource retirement.

   **Smallest improvement:** private `ConsumerKind::{Send,DraftSave}` in that key. **Regression:** hold both kinds with equal account/numeric ticket, finish one, and verify the other lease remains usable until its own receipt.

The prior private Outbox-phase-string concern remains nonblocking; this review does not escalate it or recommend a wider refactor.

### Separate gates

Unversioned headers/mutations, synchronous Thunderbird page head-of-line blocking, recovery/shutdown uncertainty, provider registration/rotation, helper engine/CID/inline/A11y acceptance and native performance remain declared limitations—not newly discovered Standards bugs. Rust/C execution remains for the parent’s grouped window.

**Summary: 0 confirmed hard Standards breaches; 2 new nonblocking P3 heuristics.**

---

## Spec review — frozen batch 2

**Baseline:** `d4dbd2bba14f474a33870269cc15bd98376ff360`; HEAD unchanged. Reviewed working-tree source, untracked modules/tests/helper/scripts, producer and consumer handoffs—not the empty committed diff. Graft returned no matching nodes. Worker SHA matches the supplied `edece443…c42f1`.

**Read-only:** no edits, builds, test execution, app/helper launches, private profiles, credentials, provider operations, mail mutations, installation or commits.

### Confirmed incorrect implemented paths

**P2 — Password/manual admission can persist a duplicate Thunderbird mailbox before rejecting it.**

**Source:** `apps/megamail/src/main.rs:MailApp::connect_account` (3159–3219); `account_registry.rs:save_direct`; `live.rs:LiveMailbox::add_profile` (1109–1118).

**Input/path:** A connected Thunderbird account already owns `a@example.test`. Add the same address through password/manual setup. `direct_add_block_reason` permits a healthy mixed roster. Both duplicate checks in `connect_account` inspect only direct profiles; the background check reloads only the direct roster. After successful connection tests, `save_direct` reserves another ID and persists credentials/configuration. Only afterward does `add_profile` inspect the complete live roster and reject the duplicate email.

Thus a rejected native admission has already become a durable direct account. The OAuth path does not have this omission: `RegistryStore::save_checked` checks direct profiles **and** `known_emails`.

**Requirement:** PRODUCT requires “data integrity and honest operational states”; F0 requires configured/connecting/ready states to remain distinct. The integrated handoff additionally requires “duplicate direct/TB roster preservation.”

**Smallest fix:** Apply one normalized duplicate policy to password and OAuth admission before connection/persistence. Carry the verified Thunderbird addresses into the serialized password admission check; preserve the existing fail-closed rule for unknown Thunderbird addresses.

**Production-seam regression:** With a synthetic connected TB roster and no direct profile for that address, invoke the actual password-admission decision using mixed case/whitespace. Assert zero connection, reservation and save calls, unchanged roster, and unchanged IDs. Also verify that a different address can join the mixed roster.

---

**P2 — FocusReader focuses a wrapper, not the selectable reader text.**

**Source:** `apps/megamail/src/main.rs:MailApp::new`, reader construction at 5066/5212, and `FocusReader` listener at 7239–7246; `vendor/gpui-base/src/text/text_view.rs` at 698–728.

`reader_focus` is independently created and attached to `reader-scroll`. Ctrl+Alt+R focuses that handle. The body remains a `TextView::html`, whose internal `TextViewState` has a **different** focus handle and owns the `TextView` key context, Copy and Select All handlers. Focusing the ancestor does not target those descendant handlers.

**Requirement:** U1 requires “Keyboard/focus/accessibility tree.” The desktop consumer contract specifically requires FocusReader to target “the real selectable TextView state's focus handle, not a decorative wrapper.” This is incorrect wiring, separate from unexecuted accessibility acceptance.

**Smallest fix:** Retain the selected body’s actual `TextViewState`, render it through `TextView::new`, and focus its handle. Update that target on selection and plain/quote-mode changes.

**Production-seam regression:** In an isolated fictional native window, invoke FocusReader and assert the selected body’s actual handle is focused. Exercise Select All/Copy through that target; repeat after selection/mode changes and verify composer/modal guards.

### Source repairs accepted; execution pending

- Calendar MIME now refuses signing/encryption intent and extra attachments before its early return. No flag downgrade or silent file omission is apparent in that builder.
- Encrypted Outbox preview suppression branches before reading plaintext body. Synthetic prebuilt MIME tests do **not** prove encryption.
- Native IMAP dispatch clears only the outgoing clone’s generationless draft origin, preserving composer/recovery metadata.
- Scoped body/source/metadata acceptance, draft tickets, Sent-only cleanup receipts and cleanup-only retry are present.
- Prior search-NUL, provider-budget and recovery-resync findings remain source-resolved.

### Declared gaps and blocked acceptance—not new defects

The original P0 requirement remains incomplete: direct original completeness fails closed; the helper is developer-only and separate-window; MIME-CID, consented remote images and inline GPUI are absent. H2 actor page head-of-line remains. Contacts/productivity/rules/parsed search/bulk are accurately declared unwired.

Epoch-wide mutation/header safety, token rotation, Outbox reconciliation, deletion/shutdown durability, helper tracing/interaction/A11y and measured native performance remain gates. Verification/QA scripts distinguish observations from feature passes and retain isolation requirements.

**No confirmed scope creep. No batch-2 runtime acceptance or passed-test count is established by this review.**

## Parent disposition

Standards: zero confirmed hard breaches; two nonblocking P3 heuristics are retained for later targeted work, not expanded into speculative refactoring. Spec: two P2 source defects require correction before the batch freeze/build: mixed direct/TB password admission persists before late rejection; FocusReader targets a wrapper rather than the selectable TextView state. New exclusive corrective owner megamail-bigbatch2-native-spec-r3-fixes owns only main/provider_setup/reader_session and a new report. No compilation starts before release and follow-up review.

Parent verification driver also narrows embedded asset hashing to explicit reviewed namespaces, covering native fonts, native/core fictional fixtures, bridge manifest/schema, and vendor dock test fixtures without indiscriminately reading arbitrary runtime JSON/EML. Launch-free driver contracts are in tools/test-megamail-batch2-verifier.py. They mock command execution and signals, not compilation, kernel cleanup or provider operation. New source digest must be recorded after these changes and the corrective owner release.

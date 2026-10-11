# Codex takeover and integration

The user authorized completing the released work without delegation, integrating
and pushing it to main, replacing the local binary, updating the icon, and
removing other MegaMail worktrees after preserving their changes.

## Source review

Reviewed the three released batch 3 changes against their producer contracts:

- Native mailbox focus is admitted on the next mounted frame and on returns
  from guarded views. It preserves focused controls inside the mailbox subtree;
  it does not run on each body/header notification.
- Composer contact and literal-text deliveries check retained editor identity,
  account/backend, source epochs, request revision and editability. Mutations
  additionally reject changed complete payload/caret/selection and mark recovery
  dirty. Local contact creation uses the store revision; unsupported library
  writes and contact update/delete are not exposed.
- Thunderbird pages use one joined reader with bounded command/completion queues.
  Retired flights keep their physical admission slot until completion. Only
  current immutable envelopes project rows/cursors. Ordered mutations stay on
  the actor; the existing 15-second transport deadline remains.
- Helper chrome wraps subject/notice and separates action rows without changing
  the document, sandbox, original-HTML release gate, or network policy.

No new concrete source defect was identified in this review. This is a parent
source review, not independent review or runtime acceptance.

The icon is an original SVG with a violet tile, folded paper and restrained
shading, inspected after rendering at launcher size. Existing desktop icon and
installer paths remain valid. Generated helper binaries are ignored.

## Repository disposition

At takeover, HEAD was d4dbd2b on main. There was one registered MegaMail
worktree. Both origin/mail-experience and origin/zeron-workspace had zero commits
outside main. No unrelated repository worktrees will be removed.

## Verification

Results and installation receipt are recorded below after execution. Earlier
research reports remain historical evidence and do not imply that every
Thunderbird/Spark/Superhuman feature is shipped. Parsed search, bulk triage,
rules, scheduling, rich inline HTML, complete contact editing and other feature
matrix gaps remain incomplete unless a production consumer is verified.

### Completed core/source checks

The source/packaging/bridge/driver/kernel checks in the serial verifier passed.
Core tests completed: 574 library + 4 OAuth public + 21 provider-auth public
checks passed (599 total), zero failures, 20 ignored. The connect-blackhole
ignored gate was not included in that total. Logs:
`/tmp/megamail-codex-verification/`.

Shared-machine coordination interrupted the verifier after core completion.
The verifier was stopped before admitting native tests, then terminated only
after its core Cargo drained. No other owner's process was signaled. Remaining
Cargo commands must use `flock --close /tmp/mui-cargo.lock` and recheck the Hive
marker before admission. The never-admitted marker waiter was subsequently
cancelled and drained (exit 143); MegaMail awaits KURV's explicit 0.5.9 tail
return before requesting that slot.

## Continued source implementation — 2026-10-11

The latest user instruction is to finish implementation personally and leave
building to the user. No new Cargo/rustc/native launch, installation or push was
performed in this phase. The earlier 599 core tests do not cover these changes.

Native consumers now include parsed cache search, complete local contact editing,
paged browsing and bounded vCard import/export; private library editing with
writer locking and snapshot comparison; explicit template values and subject
application; and selected-message rule preview. Newly authored regression tests
cover real parsed SQL paging/body exclusion, attachment flag consistency, contact
file bounds/export non-overwrite/Unicode round-trip, checked rule loading, template
payload preservation and text-library conflicts/backend separation. They are
unexecuted. Rustfmt source parsing and scoped whitespace checks pass.

Outstanding production integrations remain tracked in the feature matrix.
This is an implementation checkpoint, not a finished-parity or release receipt.

The reader also has a native displayed-text export using the selectable state's
rendered text, selection lifetime checks and a background private-file writer.
The same writer is reused for vCard export. Unicode bounds and destination
non-overwrite checks are authored. The Thunderbird bridge Node checks passed;
the static desktop audit passed its bounded source-marker checks. Neither check
compiles Rust or establishes native UI behavior.

## Continued native workflows — 2026-10-11

New source wiring reaches the native controls, live state, direct worker,
Thunderbird adapter/extension and legacy exhaustive event/request consumers:

- Loaded multi-selection (individual/range/all loaded; 500/action), qualified
  per-item read/star/move results, exact destination confirmation, cancellation
  of undispatched batch items and confirmed-flag undo. The direct lane verifies
  worker session and mailbox epoch before submission, UID presence/persistent
  flags and observed post-write flags. Thunderbird verifies actual owning
  account/folder and API acknowledgments. Unknown outcomes are not retried.
- Private local rule add/edit/delete/pagination, checked snapshot saves under a
  writer lock, selected-message preview and explicit preparation/confirmation of
  a supported previewed move. Advanced rules are preserved. Body uncertainty,
  tag operations and unsupported filing policy block executable plans.
- Bounded exact, case-sensitive search of current displayed text, 500-match
  result navigation and Unicode excerpts. Results are ephemeral and scoped to
  both reader identity and actual rendered text; they do not implement scrolling
  or highlight overlays inside the body.
- Direct IMAP Send Later, explicit numeric timezone offset, reviewed future-time
  dispatch guard and existing durable Outbox scheduling. Typed delivery phases
  reach the native Outbox. Two-step cancellation/discard is session-qualified
  and uses an atomic ready-only exact-schedule delete. Due/claimed/held/accepted
  intents retain their bytes. Scheduling requires the app running/connected;
  closed-app server delivery and Thunderbird scheduling are not implemented.

The new Rust regression checks are authored, not run. They cover exact
admission/receipt identity, retired mailbox UID reuse, old worker retirement,
account-specific move plans, Unicode find bounds, preserved advanced rule
conditions, rule snapshot conflicts/corruption, explicit schedule offsets,
ready/claimed Outbox phases and atomic cancellation preserving held bytes.

Executed checks: the Thunderbird bridge protocol suite passed; five Python
checks executed the actual production cancellation SQL against extracted
production Outbox schema/triggers and passed. Correct-edition rustfmt parsing,
new-module formatting and scoped whitespace checks passed. The Impeccable
source detector emitted no findings for new native controls; it does not prove
visual behavior. No Cargo/rustc/native/provider launch or installation was done.

Remaining matrix gaps (including rich HTML/CID, full source/print, incoming
snooze/reminders, provider contact synchronization, automatic rules, account-type
coverage, multi-window and collaborative/AI features) are not complete. This
receipt does not assert full competitor parity or release readiness. The user
has deferred compilation until implementation is complete; do not resume a
build waiter or treat absent shared-machine markers as admission.

Native newsletter metadata now exposes reviewed unsubscribe email/page actions.
Email requests use the existing bounded mailto parser, refuse CC/BCC and invalid
headers, retain current metadata equality and open an unsent composer on the
owning account. Browser requests recheck the current URL and use the existing
HTTP(S)/userinfo/control validation. No unsubscribe POST or send is automatic.
Authored parser tests cover legitimate email/reply routes and malicious hidden
recipients/header injection. Ctrl+Shift+F focuses the native message find control.
Queued cancellation also safely discards unclaimed, unscheduled ready copies;
held/claimed copies have no discard permission. The five actual SQL checks cover
both schedule and non-schedule cases. Tool: `tools/test-megamail-queued-cancel.py`.

Explicit local image attachment preview is also wired through the current reader
and immutable attachment data. PNG/JPEG/WebP input is bounded to 8 MiB and strict
4096-pixel dimensions, decoded off the UI executor in one physical flight and
scaled to a static native BGRA preview. Current reader identity and retained
attachment pointer are revalidated before showing it. Closing/changing messages
retires preview state without admitting another decoder before the current job
finishes. Original bytes and Save As remain unchanged. Authored Rust tests cover
real PNG decode/channel order and overlarge dimensions/unsupported input.
PDF preview, EXIF orientation and galleries remain unimplemented. The decoder's
allocation limit is best effort; strict dimensions provide the hard image-size
bound ([image 0.25.10 Limits](https://docs.rs/image/0.25.10/image/struct.Limits.html)).

Printable-message export now reuses the native private-file dialog/writer and
retained reader checks. It generates escaped, self-contained HTML from displayed
text and bounded Subject/From/To fields, preserves whitespace, sets a restrictive
CSP and print margins, and provides browser Print/Save-as-PDF instructions. It is
not a native printer dialog or direct PDF engine. Authored tests cover markup
injection, Unicode, bounds and external-request policy. Native worker lifecycle
and receipt events map to an explicit legacy GTK no-op, preserving existing
status/errors rather than clearing them; a regression check is authored.

An uncertain mutation also cancels the not-yet-submitted remainder of its batch;
it cannot automatically reconnect and continue provider writes while an earlier
outcome needs manual review. A domain regression check enforces this behavior.

Outbox delivery details now replaces the disabled review button. The native view
shows the current typed Ready/InFlight/Accepted/Refused/Uncertain phase, account,
recipients, subject, queue time, attempts, server response and the existing
cache's encryption-safe preview. Its recovery instructions distinguish accepted
mail from unresolved delivery and never grant retry permission. The panel is
scrollable. Held resend, raw-MIME export and acceptance reconciliation remain
unimplemented; this is a working read-only review, not a release of held mail.

Final source-only rechecks passed the Thunderbird bridge protocol suite and
29 Python checks: five production queue SQL checks, six reader focus source
contracts, six activation packaging contracts, ten verifier driver contracts and
two native QA import contracts. The source contracts are not native runtime or
Rust type checks. No new Cargo process or build waiter was started.

The final staged-source pass parsed all 53 changed Rust files using their owning
edition and passed staged whitespace checks. New module formatting and the
Impeccable source detector passed. Parsing is not type checking or test execution.

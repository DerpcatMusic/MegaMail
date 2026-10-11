# Batch 2: offline compose productivity and rule preview

## Status and ownership

**Implemented module interfaces; NOT shipped native features.** The parent owns
module registration, native UI wiring, and the single batch compilation window.
This owner changed only these new files:

- `apps/megamail/src/compose_productivity.rs`
- `apps/megamail/src/rules_preview.rs`
- `docs/research/batch2-compose-rules.md`

No edits to `compose.rs`, `main.rs`, `live.rs`, core/config/worker/manifests,
Thunderbird, or another owner's files. No Cargo/build/check/clippy/rustc/helper
compilation, commits, provider/TB operations, profile/credential access, or mail
mutations. Other working-tree changes belong to other owners.

Read AGENTS, PRODUCT, DESIGN, the architecture, the feature matrix, and relevant
foundation findings for account lifecycle, mail integrity, provider auth, body
latency, and HTML fidelity. Graft was queried first; ask returned no hits and grep
reported no graph. Scoped source inspection followed; no graph build was run.
Loaded Rust guidance and documentation-lookup guidance. Verified serde_json's
public `from_slice` documentation; serde's container-attribute page was blocked
by HTTP error. No dependency additions or UI toolkit API assumptions are needed.

## C1: what the module actually does

All compose edits operate on **the existing `worker::OutgoingMessage`**, not a
parallel compose model or fake action.

- `TextItem` represents a plain signature, template, or canned snippet, bound to
  a stable numeric MegaMail account ID. Only templates may supply a subject.
- `render_item` expands `{{name}}` placeholders from explicit caller-reviewed
  values. Names contain ASCII alphanumerics/underscore, up to 64 bytes. Expansion
  is literal and nonrecursive. Missing names are sorted/deduplicated and block
  insertion; malformed opening placeholders fail. Lone closing braces are text.
  No inferred recipient/date/AI variables, commands, network, or HTML evaluation.
- `insert_item` validates everything before mutation, inserts at a UTF-8 byte
  cursor without removing original body text, and changes subject only when the
  caller explicitly requests replacement. Signature insertion adds `-- ` with
  separating newlines. Repeated insertion is intentionally an explicit edit,
  not deduplicated by fragile text matching.
- Bcc, Reply-To, attachments, thread references, draft/outbox origins, calendar
  alternative, signing/encryption, and scheduling fields are untouched. Existing
  HTML alternatives cause `UnsupportedHtml`, not silent flattening/removal or a
  stale HTML/plain pair. HTML signatures are rejected, not fetched or converted.
- `configured_signature` uses the existing `AccountConfig` plain signature.
  `choose_sender` accepts only primary or an exact current configured alias.
  It checks the trusted registry account ID, enabled state, identity syntax and
  header controls before changing `from_alias`. It never changes transport or
  credentials, creates an alias, switches account, or silently falls back.
  Provider send-as permission remains unverified; the existing backend validates
  its own identity/transport when dispatch actually occurs.

This correlates to matrix C10/C13/C21/C29 and F7 recovery invariants. Rich HTML
compose, spellcheck, CID editing, contact completion, original-MIME forwarding,
provider draft autosave and recovery storage are not implemented by this module.
It does not duplicate or supersede the draft owner's recovery document.

### User-authored library persistence

`TextLibrary` has validated construction, scoped `for_account`, `upsert`,
`remove`, and the versioned `encode`/`decode` seam. Scoped edits retain other
accounts' items, including equal asset IDs under different accounts.

`PrivateLibraryStore::new(&directory)` requires an injected **existing**, absolute
private Unix directory. Parent may provision a new private MegaMail subdirectory,
then pass it in off the UI thread. No implicit home/XDG/account path lookup. The
only final file is `compose-productivity.v1.json` below that directory; no existing
account configuration is modified. Tests inject fictional temporary directories.

Bounds: 128 assets, 64 KiB per asset body/expanded result, 998-byte subject/label,
64 variables per render, 1 MiB serialized library, 2 MiB final composer plain
body. IDs are bounded ASCII labels, never used as paths. File reads are bounded;
regular-file and private-permission checks reject symlinks and public files.
Writes use a private create-new temporary file, file sync, atomic rename and
mandatory directory sync. If directory sync fails after rename, the error does
not establish durable success; reload and report uncertainty. Failed pre-rename
writes retain the previous file. Unknown/corrupt versions are recoverable errors,
never an empty-library success, and normal save refuses to overwrite them.

Caller obligations: trusted directory ownership/ancestors, stable non-recycled
account IDs, one serialized writer, disk I/O off the UI thread, explicit account
removal retention policy. This is not a cross-process compare-and-swap store or
protection from hostile same-UID writers/ancestor replacement. Abrupt termination
can leave a small temporary file; no cleanup crawler was added. The aggregate
1 MiB serialization limit is checked at encode/save, so a large in-memory library
may be rejected on save. Do not report that failed save as persisted.

The store holds **only user-authored preferences**, not original/decrypted MIME,
attachments, credentials, OAuth grants, automatic incoming-mail-derived snippets,
or recovery bodies. Unix private permissions are not encryption at rest. A user
can deliberately write sensitive text; do not log item contents or render results.
Existing encrypted-body cache exclusions and remote-resource privacy are untouched.

## R1: real predicate and action preview, never execution

`preview_rules(&[FilterRule], &PreviewContext, &PreviewSample)` consumes mail-core's
actual `FilterRule` and `FilterInput`, with borrowed untrusted input. It calls
`FilterRule::matches`; partial-body logic delegates each known condition to the
existing `FilterCondition::matches`, not a new matcher implementation.

The context supplies trusted account ID/email, source folder, known destinations,
known/existing keywords, and current adapter move/tag capability. Incoming sender
or body text cannot choose the account or supply capabilities. Ownership mismatch
fails; a different account's rule is `OtherAccount` with no proposed actions.

`BodyCoverage::HeadersOnly` includes snippets/partial text. Body-dependent matches
are `NeedsBody`, unless existing AND/OR logic already determines the answer from
headers. No body fetch occurs. Complete inputs preserve existing comma alternative,
case-folding, recipient parsing and Reply-To fallback semantics. Core currently
uses substring/body-hit semantics for Body regardless of its matcher enum;
`body_uses_contains_semantics` exposes this quirk rather than silently fixing it.

Outcomes borrow rule actions and expose:

- tags from matching rules, already present/unknown/duplicate tags;
- first known non-source move, vanished destinations, same-folder and later moves;
- unavailable adapter capability versus `WouldApply`;
- the existing move `count_unread` policy, not an invented mark-read operation.

Precedence follows the existing GTK `src/app.rs::filter_pass` action-selection
shape; **it is not a replacement for that execution path**. Provider mutation
failure/permission errors, mailbox epochs, pending move deduplication, sync's
Sent-copy exclusions and account session state are not simulated. If ANY row is
`NeedsBody`, the entire list's move/tag precedence is provisional: UI must label
it incomplete. Even complete preview results cannot authorize execution. Render
all labels/destinations as native text, never active HTML or resource URLs.

Bounds: 256 rules, 16 conditions per rule, 64 KiB individual header/rule strings,
2 MiB aggregate rule strings/body/context-list bytes, 1024 folders/tags per context
list, 128 body hits with 2 MiB aggregate hit bytes. Rejection yields an error, not a
truncated successful preview. All work is deterministic but bounded processing is
not a latency measurement; run preview off the render path.

## Deferred send/undo/snooze: eligibility contract only

`validate_deferred(account_id, now, DeferredIntent, DeferredEvidence)` validates
UTC Unix deadlines against an injected clock. Send Later/snooze must be future
and at most 366 days away. Undo delay is 1–120 seconds with overflow checks.
Evidence is account-bound and defaults fail closed. It requires durable payload,
full recovery, actual closed-app dispatch/wakeup policy, and pre-submission
cancellation for sends. Submitted/uncertain outcomes are blocked, not retried.

Evidence must come from the actual queue and policy owner. For send/undo, full
recovery includes envelope/Bcc, MIME/calendar parts, attachment lifetime, origin
replacement and security flags. For snooze, it means durable scoped message key,
mailbox epoch, prior state and wake policy. A deadline is **not** an enqueue, send,
undo success, server schedule, or wakeup result. No `send_at` changes, elapsed UI
timer, pretend button, notification, or mail mutation is implemented here.

Parent must leave these controls unavailable until a real durable background or
server scheduler supports each adapter and desired app/device/offline lifecycle.
App closed is not device powered off; due-time late policy, clock changes,
restart, cancellation/submission races, account removal, reconnect, and uncertain
SMTP acceptance need separate F5/F7 owner implementation and tests. The existing
worker queue/timestamp alone does not establish always-on delivery.

## Integration obligations before any shipped claim

1. Parent registers `mod compose_productivity; mod rules_preview;` in the native
   crate. This owner did not edit `main.rs` or manifests. Both modules use only
   dependencies already declared in the native manifest.
2. Wire signature/template/snippet actions to the **real** prepared composer
   message. Check `composer.can_edit()` and trusted current account before edits;
   set dirty/recovery state through F7, clear obsolete saved status, update native
   input/cursor through supported GPUI input APIs, and surface errors/missing
   variables. Use the same public operation as the synthetic tests. Never insert
   into a separate sample message or silently lose a calendar/attachment/Bcc.
3. Alias picker/cycling must resolve CURRENT config from the registry and use
   `choose_sender`; no address inferred from incoming email. For Thunderbird,
   supply its actual imported identity projection in `AccountConfig` or preserve
   its existing authoritative identity path until that projection exists. A
   fabricated empty config must not claim Thunderbird alias support.
4. Provision/inject the new private store off-thread and serialize edits; distinguish
   absent store from error/repair and never save defaults after a load failure.
5. Rules UI passes an immutable rule snapshot, account-scoped message key, actual
   folder/tag metadata and adapter capabilities. Use Complete coverage only with
   complete body/hit evidence. Show dry-run/incomplete scope explicitly; no Run
   button backed only by this preview. No new execution backend is necessary.
6. Parent executes the synthetic Rust tests through these registered modules in
   its single batch build window, then checks actual UI-to-operation-to-recovery
   round trips. Until that wiring and verification, **all new UI outcomes here
   remain NOT shipped**.

## Verification and honest assessment

Executed: direct `rustfmt --edition 2024` and formatting verification on the owned
Rust files; owned-file whitespace audit; a Python **static source-contract audit**
checking the real matcher calls, restricted composer assignments, and absence of
worker/network/profile-dispatch calls. The Python audit is not a reimplementation
or runtime test of the Rust logic. No compiler, Rust unit tests, native UI, disk
fault injection, protocol test, performance benchmark, or real-provider test ran.

Authored synthetic Rust tests use the public production interfaces and fictional
addresses/data. They cover insertion/signature/snippet/template subject consent,
F7 metadata/calendar preservation, rejection without partial mutation, UTF-8
cursor, missing/nonrecursive variables, HTML refusal, current alias membership,
account/header-injection checks, bounded/versioned codec, cross-account library
edits, injected private-store roundtrip/unknown-version preservation, deferred
policy rejection, actual rule matching/body quirks, partial-body uncertainty,
account scope, tag/move precedence/capability and bounded rule count. These are
**unexecuted**, not passing tests. Further parent gates: disk-full/file-sync/rename/
directory-sync faults; file/ancestor symlinks; concurrent writers; actual composer
input synchronization and F7 crash/restart; rule previews across direct/TB and
mailbox epochs; queue cancellation/lateness/uncertain submission invariants.

**Good:** small existing seams, no new engine, fail-closed sender/HTML/deadline
handling, scoped private preference store, actual matcher reuse, mail integrity
fields preserved, no incoming-mail authority or remote fetches.

**Bad/incomplete:** no UI registration/wiring, no compilation/test result, no rich
editor or spellcheck, no scheduling execution, no provider alias certification,
no rule execution or provider failure simulation. These are module implementations,
not a shipped productivity suite or a performance improvement claim.

**Risk:** trusted numeric ID/config binding and input coverage must be correct at
integration; unknown body can change list precedence; library persistence assumes
single-writer/trusted private ancestors and can report a post-rename sync error.

**Blocker:** parent-owned native integration and batch build/test window; F7 dirty/
recovery integration; real durable closed-app policy for deferred actions. No
external provider credentials are required to validate this offline slice.

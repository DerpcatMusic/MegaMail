# Batch 2: desktop compose, keyboard contracts, and optional AI/team policy

## Status and boundaries

**Helpers implemented; native integration pending. This is not shipped Linux
mailto/default-app support, full keyboard parity, accessibility certification,
or an AI/team feature.** Parent owns native integration and the one large-batch
build window. No compilation, app/helper launch, provider/TB login, mail send or
mutation, profile/credential access, desktop setting change, notification, or
commit was performed by this owner.

Read AGENTS, PRODUCT, DESIGN, architecture, the [feature matrix](email-client-feature-matrix.md)
(U13–U21, S14, A01–A21), and foundation [engine](foundation-mail-engine.md),
[account lifecycle](foundation-account-lifecycle.md), [provider auth](foundation-provider-auth.md),
[fetch latency](foundation-fetch-latency.md), and [HTML reader](foundation-html-reader.md)
findings. Graft was queried first and returned an empty graph. Source review
uses current symbols, not inherited feature claims. Other owners are changing
main/live/auth/worker concurrently; recheck these call paths at freeze.

Applicable guidance: Rust best-practices/error/testing references, rust-skills,
Impeccable context and technical audit/craft-floor, and find-docs. Context7
resolved `/longbridge/gpui-kit` and returned action/key-context/focus guidance;
the locked 0.7.1 facade and vendored/locked GPUI sources govern actual APIs.
Linux is the product target; mobile accessibility scoring is not applicable.
No numerical UX/performance score is justified by this source-only review.

### Actual owned changes

- [`crates/mail-core/src/desktop.rs`](../../crates/mail-core/src/desktop.rs):
  additive pure `parse_mailto`, intent/error types, bounded decoding/address
  validation and ten production-helper regression tests. Existing `setting`,
  `flag`, portal and host-setting function bodies remain unchanged. Corrected
  the module's dangling `present_window` doc link; the GTK implementation is
  still in [`src/desktop.rs`](../../src/desktop.rs), which was not edited.
- New [`apps/megamail/src/desktop_actions.rs`](../../apps/megamail/src/desktop_actions.rs):
  actual GPUI action definitions/bindings, dispatch policy, guarded focus helper,
  parser delegation and adapter to `compose::new_message`/`OutgoingMessage`.
  Six regression tests exercise that same adapter and policy, not a fake send.
  Parent has not declared/wired this module at this owner's freeze.
- This report and new [`tools/megamail-batch2-static-audit.py`](../../tools/megamail-batch2-static-audit.py):
  bounded read-only source checks and audit-scanner self-tests. Its optional
  integration marker check intentionally exposes missing native wiring.

## Mailto contract

`desktop::parse_mailto(&str) -> Result<MailtoIntent, MailtoError>` is the single
untrusted-URI seam. `desktop_actions::external_compose` delegates directly to it.
`compose_message(intent, account_id)` uses the existing production initializer
and fills only To/Cc/Bcc/Subject/plain body. There is **no send dispatch**.

- Case-insensitive `mailto` scheme; no authority (`mailto://`), raw fragment,
  raw control characters, lossy UTF-8, malformed percent escape, or repeated
  decoding. A literal `+` stays `+`, including plus-addressing and body text.
- At most 64 KiB encoded URI, 32 query fields, 100 combined recipient
  occurrences including duplicates, 512 decoded subject bytes and 48 KiB
  decoded body bytes. Check URI size before allocation. No truncation.
- Path recipients and repeated To/Cc/Bcc fields merge in order. Empty fields
  and no recipients are valid *draft intent*, not valid send authorization.
  Empty list entries, display names, groups, quoted local parts, address
  literals, Unicode mailbox addresses and malformed domains are rejected.
  Supported addresses are ASCII dot-atoms with DNS/ACE labels: local <=64
  bytes, label <=63, domain <=253, address <=254. This is a conservative
  subset, **not full RFC 5322/SMTPUTF8 support or domain-deliverability proof**.
- Header values/names reject CR/LF, NUL and other controls, Unicode line/paragraph
  separators and bidi controls that can hide header/recipient content. Unicode
  subject/body text itself remains supported. Body CRLF normalizes to LF;
  LF/tab are allowed, bare CR and other body controls rejected.
- Only To/Cc/Bcc/Subject/Body are allowed, including percent-encoded/case-varied
  names. Unknown fields fail explicitly; duplicate Subject/Body fail. From,
  Reply-To, thread headers, HTML, attachment paths, schedule/send directives
  cannot cross this seam. Attachments require the normal explicit picker.
- Escaped `<script>` in Body remains literal plain text, never `html`.
  Percent escapes decoded to a literal `%` in an address/subject remain literal
  thereafter. Parent must not decode again in the composer/MIME pipeline.
- Errors contain no URI, body, subject or address. Do not log raw arguments or
  intent debug output. Parent must retain normal send/MIME preflight, account
  authorization, durable outbox semantics and Bcc envelope handling.

Reference scope: [RFC 6068](https://www.rfc-editor.org/rfc/rfc6068) names `+`
as literal, warns about unsafe headers and automatic sending. The implementation
intentionally rejects additional legal URI/address forms rather than widening
untrusted external compose into file access or arbitrary header creation.

## Exact parent integration obligations

### Native action wiring (`apps/megamail/src/main.rs`, parent only)

1. Add `mod desktop_actions;`. Remove the inline `mod action` definitions and
   import `desktop_actions::action::{...}` instead; do not register duplicate
   GPUI action names in the `megamail` namespace. Replace the old `init` binding
   body with `desktop_actions::bind_keys(cx)`.
2. Apply `desktop_actions::MAILBOX_CONTEXT` to a mailbox-only ancestor. Do not
   leave mailbox shortcuts on a root that also contains Setup/Compose/Settings.
   Keep `MESSAGES_CONTEXT` at the list's rendered focus node; arrows must never
   steal search/body/composer cursor movement. Apply `ActionState::allows` to
   both existing toolbar/menu handlers and keyboard listeners at dispatch time.
3. Construct policy from live state: visible Mailbox, **any retained composer**,
   modal/popup/dialog presence, account inventory, usable explicit sender, rows,
   and a successful usable body for the exact selected account/message/epoch.
   `!body_loading` alone is insufficient after error or stale selection. Reply
   sender comes from the selected message owner, not whichever account was last
   viewed; compose sender follows the explicit existing account chooser policy.
4. Wire FocusMessages (`Ctrl+Alt+L`) to existing `message_focus`. Wire FocusReader
   (`Ctrl+Alt+R`) to the real selectable TextView state's focus handle, not a
   decorative wrapper. The current `TextView::html(id, ...)` internally manages
   state; use retained `TextViewState::html`/`TextView::new` if needed to expose
   the existing `focus_handle()` (vendored `text/state.rs`). `focus_pane` guards
   dispatch but does not create/fake that handle. Check visible focus styling
   and restore search/list/reader focus on view return. Keep Tab/Shift+Tab and
   Kit editing/copy/IME behavior. No Escape-discard, Delete, automatic-send,
   print or speculative keyboard mutation shortcuts were added.
5. Update shortcut help with the actual installed bindings only after listeners
   exist. Current module guards are not effective until caller wiring lands.

### Compose ingress (startup, renderer, packaging)

1. Preserve `--thunderbird-host` exactly. Parse OS arguments without lossy URI
   conversion/panic. For a recognized mailto argument, call `external_compose`
   once and retain the validated intent, not raw URI/header strings. Do not
   auto-open arbitrary paths, schemes or `attach=`. Enforce a small bounded
   pending-intent queue (suggest <=8), with explicit notice on overflow rather
   than silent replacement; multi-URI activation needs its own parent tests.
2. After account inventory/setup, let the user explicitly choose a usable
   account. Revalidate account availability at compose opening. Preserve intent
   through setup/login failure, but do not persist it in a decrypted-body cache.
   Keep demo sending disabled. Never replace an existing dirty/pending composer:
   use existing `can_replace_composer` confirmation/recovery, or show a pending
   compose notice. Build `Composer::new(compose_message(intent, account_id), ...)`
   and focus To/Subject/body appropriately; sending remains a later human action.
3. Renderer mailto activation requires a verified user click and current
   account/message/generation. Route it through the same `external_compose`
   parser; do not launch `xdg-open mailto:` recursively. The existing native
   link handler only admits validated HTTP(S); do not weaken browser URL checks
   or default-denied remote resources when adding a separate mailto branch.
4. Current [`data/megamail.desktop`](../../data/megamail.desktop) uses
   `Exec=megamail`, lacks `MimeType=x-scheme-handler/mailto;` and URI field code.
   [`tools/install-megamail.sh`](../../tools/install-megamail.sh) rewrites Exec
   to the quoted binary path **without `%u`**. Parent must update both template
   and installer, preserving executable-path escaping, and refresh the desktop
   database through normal packaging. Do not set the user's default handler
   without explicit consent. No packaging files were changed/run here.
5. Cold start is not enough: [`instance::acquire`](../../apps/megamail/src/instance.rs)
   deliberately rejects a second process to protect account workers/Outbox.
   Retain that protection. Implement authenticated bounded running-instance
   handoff separately (user-session D-Bus or private Unix socket with ownership/
   permissions and length limits), routing to the same parser. Do not bypass
   the lock, start duplicate workers, or advertise working default-app support
   before the already-running-app activation test passes.
6. Locked `gpui-pre-linux 0.3.8/src/linux/platform.rs::on_open_urls` stores a
   callback; the inspected Linux source has no delivery call site for it.
   `register_url_scheme` returns `register_url_scheme unimplemented`. Thus a
   bare GPUI callback/registration call is not evidence of Linux URI ingress.

## Honest desktop review: current call paths

| Area | Good, backed by source | Bad / remaining risk | Release obligation |
| --- | --- | --- | --- |
| Accessibility | Native main uses explicit folder/account/action accessibility labels and Input/Textarea aria labels; reader uses selectable TextView. Locked Linux X11/Wayland window sources contain AccessKit Unix adapters/tree updates. | Toolkit support and labels do not establish role/state/focus ordering, visible tree or Orca correctness. Icon/menu/button and error announcement coverage remains unverified. | Inspect the live AT-SPI tree and keyboard/screen-reader traversal with fictional mail; test empty/loading/error and modal focus restore. |
| Focus/keyboard | `message_pane` tracks `message_focus`, tab-stop enabled; Up/Down are list-context actions. Search shortcut explicitly focuses InputState. | Current root `MegaMail` context covers composer/setup/settings too; refresh is unconditional, list stays rendered beside composer, no native reader-focus shortcut. Existing handlers are not yet using the new guard. | Wire this policy on buttons and keyboard; confirm input arrows and reply shortcuts do not mutate behind compose/modal state. |
| Scale/adaptivity | Native render calculates pane widths from logical window width; reader wraps metadata/actions and locally scrolls data tables. | Main uses fixed `px` text sizes (body 14, headings 22/19/16) and no text-scale preference in MailPreferences. Startup minimum 1060x640 conflicts with DESIGN's described 1000px layout range. Compositor scaling is not independent reading zoom. | Fictional-mail checks at 100/150/200% display scaling, two monitor scales, narrow reader, long dates/addresses; add body text scaling deliberately. No runtime clipping finding claimed here. |
| RTL/localization | `mail_text::reader_html` preserves sanitized dir; vendored text shaping/cache/wrapping include paragraph direction and logical UTF-8 indices. | Native English shell strings and left/right pane structure are not a localized RTL UI. Arabic/Hebrew shaping, mixed addresses, caret/copy/IME need runtime proof. Conservative mailto addresses currently exclude SMTPUTF8. | RTL/mixed-script fixture; verify logical copy vs visual selection, bidi-isolated addresses, mail dates and focus traversal. |
| Notifications/background | Legacy GTK has `src/notify.rs`, `src/tray.rs` and app actions with scoped IDs. | These are not native module/call paths. GPUI `cx.notify()` is UI invalidation, not a desktop mail notification. No native notification/tray/close-to-background guarantee found. | New consented account-scoped adapter; redact body/subject by default, respect quiet hours/locked screen, stale-message and account removal checks; no settings/notifications exercised here. |
| Default app/package | Native app ID `com.megamail.MegaMail`, separate installer/template and single-instance guard exist. | No native scheme registration/argument routing or running-instance handoff; root Hylki desktop mailto support is a different application. | Integrate parser, URI-preserving Exec, user-consented default handler and secure activation; test cold/running state separately. |
| Printing | GTK app has real `PrintMessage`/`PrintPreview` → MessageView renderer paths. | Native semantic reader has no print path. Concurrent `tools/megamail-html-reader` policy/sandbox sources inspected have no print operation; presence of a renderer policy is not native print support. | **Not implemented.** Only add after actual renderer supports selected-document print/preview, safe resources and account/generation ownership. No fake “print supported” capability. |
| Privacy | New parser/compose seam performs no I/O/uploads/settings/send; existing HTTP(S) validation, isolated profiles, TLS/OAuth and cache/outbox code are untouched. | Text scans do not prove all upstream paths secure. Future URI handoff, renderer callbacks and optional uploads could bypass intended account/privacy boundaries. | Exercise the production ingress and approval seams; preserve encrypted-body exclusions for every new retained/persisted payload. |

`main.rs` symbols: `init`, `open_new_message`, `message_pane`, `render_reader`,
`Render::render`, `main`; [`preferences.rs`](../../apps/megamail/src/preferences.rs)
contains the real density/reduced-motion model. [`vendor/gpui-base/src/text/state.rs`](../../vendor/gpui-base/src/text/state.rs)
and [`text_view.rs`](../../vendor/gpui-base/src/text/text_view.rs) contain retained
text/focus APIs. Foundation reader findings still apply: a helper pixel stream
is not a focus/accessibility bridge or renderer-print integration.

## A1: optional AI/team requirements, not implementation

**Optional AI and team capabilities are not implemented and are blocked on
reliable account lifecycle, exact message scope, draft recovery, durable send
outcomes, safe reader resources and offline/search correctness.** No new runtime,
AI request, provider upload, telemetry payload, autonomous mail agent, team
storage, misleading enabled toggle or “supported” capability was added.

The feature matrix's competitors document optional AI and hosted collaboration;
that is neither proof of native parity nor a license to upload mailbox history.
Default capability must remain unavailable, with no implied configuration-only
route to enable it. A requirements inventory can distinguish unimplemented,
policy-reviewed, configured, and usable; only real implementations may advance.

Minimum gates before any implementation owner enables a capability:

1. **Explicit consent per feature and account.** Default off; opt-in cannot be
   bundled with ordinary mail sync. State who receives data, why, model/vendor,
   price, scope and retention before sending. Show preview/redaction and allow
   cancel. Consent to one draft is not permission to crawl an Inbox or team.
2. **Data minimization and account boundary.** Default selected text/current
   draft only, not whole thread/history/attachments/address book. Separate
   permission for thread, attachments and history; include exact owning account/
   folder/message/generation and reject stale responses. Never send passwords,
   tokens, raw private profile paths, or unrelated Bcc/hidden recipients. Signed/
   encrypted/decrypted mail is excluded by default; any future explicit consent
   needs a reviewed transient-data policy and no new plaintext disk cache.
3. **No-training and vendor policy evidence.** Require documented contractual
   no-training/no secondary use, model/subprocessor list, regions, encryption,
   support access and incident notification. Marketing text is not assurance.
   If a vendor cannot meet it, capability stays unavailable. Local-only inference
   is a distinct architecture choice, not a promised feature in this batch.
4. **Retention/deletion.** Define maximum request/result/log/backup retention,
   transient processing, deletion controls and completion deadlines, revocation,
   export and auditability. “Not trained on” does not mean “not stored.” Exclude
   raw mail from application/provider diagnostics. Deleting local state cannot
   be labeled deletion of vendor/team copies until confirmed by that service.
5. **Incoming prompt injection.** Treat email, HTML, attachments and retrieved
   text as untrusted data, never instructions. Separate trusted user request
   from quoted inputs; no model-controlled tools/browser/network/credentials.
   Reject requests to exfiltrate content/change recipient/upload elsewhere.
   Add adversarial fictional messages; sanitizing HTML is not an LLM defense.
6. **Human approval and traceability.** Results are suggestions/drafts, with
   source links and uncertainty; inspect final body, recipient set, attachment
   set, sender and changes before committing. No autonomous send/archive/trash,
   scheduling, rule installation, credential change or team-share operation.
   Approval is scoped to the exact content/account/action and becomes invalid
   after edits or selection/identity change. Normal preflight/outbox still owns
   actual delivery; AI success must not be reported as mail delivery.
7. **Team isolation.** Do not share provider credentials or convert personal
   account authorization into organization-wide access. Specify roles, membership,
   guest limits, private comment-vs-email separation, draft conflict handling,
   revocation, expiring shares, Bcc redaction, audit events and tenant isolation.
   Every attachment/descendant/thread reply must respect share scope. No public
   live-thread link until authorization and deletion/revocation tests pass.

## Validation and freeze handoff

Rust tests call the actual production `parse_mailto`, `external_compose`,
`compose_message` and `ActionState::allows` seams. They cover percent/UTF-8,
header injection/control/bidi cases, encoded field names, unsupported/duplicate
fields, malformed addresses, body newlines/controls, recipient/field/size
boundaries, plain-text intent, Bcc and sender isolation, and keyboard blocking
behind compose/modal/non-mailbox state. **They are authored, not executed.**
Parent must declare the native module before its six tests can be discovered.

Executed, within the no-build boundary:

- `rustfmt --edition 2024` on the two owned Rust files; subsequent standalone
  `rustfmt --check` passed. No Cargo wrapper or compiler was invoked.
- `python3 tools/megamail-batch2-static-audit.py --self-test`: passed audit-scanner
  self-tests and bounded owned-source checks. Existing portable settings helper
  bodies matched HEAD. All four owned paths were present/changed; unrelated
  checkout changes were reported without attribution.
- `git diff --check -- crates/mail-core/src/desktop.rs`: passed. Separate Python
  whitespace/final-newline checks passed for all three new files; Python
  `compile(..., 'exec')` syntax check passed without writing bytecode.
- `python3 tools/megamail-batch2-static-audit.py --require-integration`: **exit 1,
  expected**. Native module declaration, binding/parser/payload call sites,
  action guard and mailbox-only context markers are absent at this freeze.

The Python audit does not reimplement/execute the parser and is not a green
parser regression suite. Even a green integration marker check would not prove
Linux activation or correct focus/sender/dirty-composer behavior.

Unexecuted acceptance: Rust compile/tests; native cold and running-instance URI
activation; account setup/deep-link queue retention; dirty/pending composer
protection; toolbar and keyboard use of the same policy; real text focus/copy/IME;
Orca/AT-SPI; monitor scaling and RTL; package/install behavior; default-app choice;
notifications; print; AI/team. No provider, delivery, performance or full security
claim is supported by this work.

# Batch 2: local contacts and invitation preview

Date: 2026-10-10. Implementation-owner handoff, not a native feature-parity claim.

## Status and exact ownership

Changed only these files:

- **New** `apps/megamail/src/contacts.rs`: private, versioned, bounded, account-scoped local contacts CRUD, explicit vCard import, immutable completion snapshots, synthetic real-store tests.
- **New** `apps/megamail/src/invitation.rs`: read-only presentation of actual MIME-derived `Invite` metadata through the existing core helper; synthetic MIME/helper tests.
- **Changed, with additional parent approval** `src/invite.rs`: bounded parsing, checked numeric arithmetic and UTF-8-safe timezone weekday parsing, with production-core regressions. Existing public `detect_raw`, `detect`, `parse`, and `reply_ics` signatures remain unchanged for GTK and native callers.
- **New** this report.

No manifest, worker, core library, compose, main, live, provider configuration, Thunderbird extension, or other owner's file was edited. No commits, builds, native/helper compilation, Rust tests, private-profile reads, Thunderbird/provider logins, sends, mailbox mutations, or real OS-contact reads were performed.

The new native modules are **integration-ready source, not completed product features**. At handoff inspection, main/live/compose had no calls or module declarations for them. The parent owns module declarations, transport correlation and native presentation after release/freeze. In contrast, the core helper fix is on the existing `verify` → `invite::detect` → `parse` production path; its tests do not depend on someone remembering to call a disconnected preflight utility.

## Evidence consulted

Read `AGENTS.md`, `PRODUCT.md`, `DESIGN.md`, `docs/MEGAMAIL_ARCHITECTURE.md`, `email-client-feature-matrix.md`, and foundation account-lifecycle, mail-engine, provider-auth, fetch-latency and HTML-reader findings. Read the Rust skills and applicable error/test guidance. Context7 verified the existing serde_json byte-slice/to-vector APIs; no dependency was added.

Queried Graft before source inspection: `graft ask 'contacts vCard ICS invite MIME calendar' .` returned no nodes. Used source inspection rather than rebuilding Graft.

Relevant existing seams:

- `crates/mail-core/src/lib.rs` exports the real shared `src/invite.rs`, not the GTK/EDS contacts module.
- `src/verify.rs` extracts `Invite` from the parsed MIME message and places it on `SenderCheck`.
- `src/models.rs::Invite` preserves the selected calendar part in `ics`, the MIME method fallback, event identity, attendees, timezone-resolved timestamps and cancellation status.
- `src/worker.rs::WorkerEvent::SenderChecked` currently contains `path`, `message_id`, and `check`. It does **not** itself carry a request ID, body generation or session epoch.
- The inspected native worker-event projection does not expose `SenderChecked` as invitation presentation state. New preview code does not fix that omission by guessing from HTML.
- `mail_text::safe_browser_url` provides the existing HTTP(S), host, length and no-userinfo policy. Preview links use that exact policy and never open a URL.

## Local contacts: implemented contract

### Storage

Call on a background lane:

```text
LocalContacts::open(private_data_root, ContactScope { backend, account_id })
  -> LocalContacts
load() -> ContactSnapshot
apply(expected_revision, ContactChange::{Create, Update, Delete}) -> ContactSnapshot
import_vcards(expected_revision, selected_bytes) -> (ContactSnapshot, ImportReport)
ContactSnapshot::complete(query, limit) -> Vec<AddressSuggestion>
```

Use the existing MegaMail `config::data_base()` root, not a Hylki, Thunderbird, EDS, arbitrary OS address-book or credential path. Supply a nonzero stable account ID and qualify it with `Direct`/`Thunderbird`. Scope is written into the JSON and revalidated on read; equal IDs from different namespaces use different files.

Files are `contacts-v1/{direct|thunderbird}-{account_id}.json`. Format version is 1; a corrupt, unknown-version or wrong-scope file fails without being replaced by an empty book. A genuinely missing file is revision zero. Contact IDs are monotonic within a scope and are not recycled after delete. Each mutation reloads the current file, checks the caller's expected revision, validates all resulting records and increments revision.

Limits:

| Boundary | Limit |
| --- | --- |
| Store bytes | 4 MiB, checked before decode and before write |
| Contacts per scope | 5,000 |
| Emails per contact | 16 |
| Explicit display name | 256 UTF-8 bytes, no control characters |
| Notes | 4,096 bytes; newline/tab allowed, other controls rejected |
| vCard import | 1 MiB UTF-8, all-or-nothing |
| Unfolded vCard line | 8,192 bytes |
| Completion | Nonempty query ≤256 bytes; at most 20 suggestions |

Unix storage requires private non-symlink directories and private, single-link regular files. New directories use 0700 and new files 0600. A create-exclusive private directory lock serializes mutations across handles/processes. A temp file inside that lock directory is fsynced, renamed over the book, and the containing directory is fsynced. Creation of the contacts directory also syncs its parent directory. Non-Unix storage fails closed; no unverified Windows ACL promise is made.

A conflicting revision returns `Conflict`; an existing lock returns `Busy`. A directory-fsync failure after rename returns `DurabilityUncertain { revision }`: **reload instead of blindly repeating Create/Import**. A crash can leave `write.lock`; do not automatically break it. A future recovery UI must prove no writer is active before removing its known temporary file and empty lock directory. Contact scope deletion/retention on account removal also needs an explicit user policy; account removal must not silently erase the book.

This is private-at-rest file access, **not encryption** or protection against a hostile same-UID process. The caller must supply a trusted MegaMail root whose ancestors are trusted. Standard-library path checks are not a race-proof `openat` sandbox against another process with the same directory privileges. No incoming/decrypted mail is mined or copied to contacts automatically.

### Identity, import and completion

Addresses use a deliberately conservative ASCII dot-atom/DNS-domain subset. Strip surrounding whitespace; lowercase the domain only. Preserve local-part case, plus tags and dots. Never invent a name from the local part, rearrange structured `N`, apply Gmail-specific equality, merge across accounts, or read OS contacts. Quoted/EAI addresses and domain literals are unsupported, with an explicit rejection instead of lossy conversion. Completion matching may be case-insensitive without changing stored identity.

vCard 3.0/4.0 import supports UTF-8, folded lines, explicit `FN`, grouped `EMAIL`, `NOTE` text escapes, and multiple emails. Unknown fields, photos, external URLs, organization/structured names and UID are not fetched or persisted; `ignored_properties` counts losses. Encoded relevant fields and malformed cards reject the whole import. vCard 2.1, LDIF, export, photos and general-purpose vCard round-tripping are not implemented.

Dedupe uses normalized email identities within the selected scope. Re-import preserves existing names/notes. A card that overlaps one existing contact can add its other emails; a card that would join two separate existing records is rejected for explicit user resolution. Earlier records in a rejected import are not persisted.

Completion returns raw `{scope, contact_id, display_name, email}`, **not** a concatenated email header. Compose owns correct RFC mailbox/name escaping and insertion into the actual To/Cc/Bcc field. Do not copy a suggestion to `OutgoingMessage` by constructing a string that can misinterpret commas or quotes in a name.

Native/C1 obligations:

1. Load the selected sender's book off the UI thread. Replace its snapshot when the sender/backend changes; do not aggregate all account snapshots implicitly.
2. Require `suggestion.scope == active_composer_scope` at selection, even if the popup was opened under an older account.
3. Insert only on explicit selection; preserve ordinary recipient entry when there are no results or the store fails.
4. CRUD/import must use the displayed revision and report Conflict/Busy/validation errors. Keep edit input recoverable on error.
5. Use real contacts and completion APIs in the actual composer/Contacts surface. Disabled/invisible unrelated demo actions are not integration.

## Invitation preview: implemented contract

```text
from_sender_check(InvitationMessageKey, &SenderCheck)
  -> Result<Option<InvitationPreview>, InvitationError>
from_metadata(InvitationMessageKey, &Invite)
  -> Result<InvitationPreview, InvitationError>
preview_calendar(InvitationMessageKey, decoded_ics, mime_method)
  -> Result<InvitationPreview, InvitationError>
```

`from_sender_check` is the direct-account production integration seam. It consumes `check.invite`, revalidates the preserved calendar, and uses the real core parser and MIME-method fallback. No metadata means no preview. `preview_calendar` is for an explicitly provided local decoded part, **not** MIME detection, a filesystem/URL loader, or permission to infer a meeting from an email body. `validate_calendar` is optional preview-subset validation; the shared helper is independently hardened and does not rely on callers using it first.

Preview retains account/backend, folder, message, session epoch and body generation. It presents summary/location, organizer/attendees, known/unknown participation status, cancellation, recurrence rule and recurrence-instance clock metadata. It is native plain presentation data, not sender HTML. Only an explicit `URL` property passing `safe_browser_url` is offered as an action destination; nothing is fetched/opened, and a URL-looking LOCATION is not auto-opened. Display strings remove control/bidi override characters and are bounded. The first event is shown; multiple events warn. Attendee presentation is capped at 256 and warns about truncation.

Dates are validated and kept **as written** with explicit `DateOnly`, `Utc`, `Floating`, or `Named(TZID)` semantics. All-day end dates are exclusive. Floating times never acquire an invented timezone. Unknown/unresolved named zones warn; the existing core's known-zone and VTIMEZONE fallback remains in use. An obviously reversed range in the same zone or a mixed date/date-time range is rejected. No cross-zone arithmetic, recurrence expansion, DST scheduling guarantee or invented all-day end date is provided. Duration is parsed safely by core but not materialized into a new displayed event interval; a preview warning makes that limitation explicit.

`METHOD:CANCEL` or `STATUS:CANCELLED` is cancellation. Unknown methods/statuses remain read-only metadata with warnings. RSVP capability is `Cancelled`, `NotRequested`, or **`CommandTransportUnavailable`**. There is no send callback, acceptance side effect, calendar write or date-grid calendar feature in this module.

### Required production correlation — do not label stale checks as current

`InvitationMessageKey` is a consumer guard contract, not proof that a caller has an authentic correlation token. Existing `SenderChecked` fields do not satisfy its epoch/generation requirements on their own.

The native owner must correlate metadata to the **causing fetch**, including its account/backend, folder/UID or TB message identity, worker/session epoch and fetch generation/request ID. Retain the token when the request starts and preserve it through worker/adapter completion. Do **not** copy today's selected body's generation or today's session epoch onto a late `SenderChecked`; an old result for the same folder/UID would then pass a fabricated freshness check. Do not assume that a UID-only pending map distinguishes retries or unsolicited prefetch results.

A serial shared-owner change may supply a new correlated native request/event variant or an equivalent causally correlated adapter envelope while preserving existing GTK request/event callers. If causal metadata is unavailable, omit the preview rather than show stale invitation state. Native tests must deliver an old check after account/epoch/body switches, including the same folder/UID reused in a newer session.

Keep the source reader document's encryption/cache eligibility: invitation data from decrypted mail must not enter a new persistent preview/body/ICS cache. This module writes no invitation cache. Clear retained presentation on selection/account/session changes.

Thunderbird integration remains blocked until its actual MIME/getFull/attachment path supplies calendar metadata with the same causal identity and privacy policy. Do not reconstruct an invitation from flattened/sanitized HTML or reuse a local-looking file path without account/part ownership. No Thunderbird login or runtime test was run here.

## Confirmed core-helper issues fixed in production source

These are source/arithmetic findings, not reproduced live-provider vulnerabilities. Rust regressions were authored but **not executed**.

| Existing function and concrete input | Prior failure established from source | Change and production regression |
| --- | --- | --- |
| `Observance::take`: a VTIMEZONE STANDARD RRULE with `BYDAY=💣` | `split_at(v.len().saturating_sub(2))` computes byte index 2 inside a four-byte UTF-8 code point; Rust slicing there panics. Reachable through `parse` and actual MIME `detect_raw`. | `checked_sub` + `str::get` and weekday/ordinal validation; invalid grammar returns None. `malformed_multibyte_timezone_byday_is_rejected_without_a_byte_slice_panic` exercises both production paths. |
| `duration_seconds`: `DURATION:P9223372036854775807W` | The parsed i64 fits, but unchecked multiplication by 604800 exceeds i64. Overflow-checking builds panic; otherwise unchecked overflow can produce a wrapped duration. No build-mode exploit claim. | Checked multiplication/addition/sign; invalid duration propagates through `EventProps::take` to reject the calendar. `oversized_duration_multiplication_and_accumulation_fail_closed` covers direct parse and MIME detection. |
| `parse`: valid positive DTSTART plus `DURATION:PT9223372036854775807S` | The duration alone fits i64, but unchecked `start + duration` does not. | Checked end addition and GLib representability gate. `resolved_start_plus_duration_overflow_is_rejected` uses the actual parser. |

Additional bounds are enforced **inside parse**, not only in the new native wrapper: 256 KiB decoded calendar, 4,096 content lines, 8,192 unfolded bytes per line, component depth 16. Dates, signed offsets, sequence and duration grammar reject malformed input instead of extracting digits/defaulting silently. Components must balance. Binary `.ics` parts that are not UTF-8 are not lossily reinterpreted. Ordinary Unicode names, locations, descriptions, CN and custom timezone IDs remain accepted; `unicode_calendar_text_and_timezone_names_remain_supported` covers them with the real helper.

Compatibility risk: stricter malformed/unsupported timezone/date/duration admission can suppress a previously guessed invitation while leaving the ordinary message reader intact. This is not a full RFC 5545 recurrence/timezone engine. Existing legitimate Zimbra/Windows zone, UTC/all-day/duration, folding, cancellation and reply tests remain and must be run by the parent. Negative timezone BYMONTHDAY and multi-valued BYDAY/BYMONTHDAY rules outside the helper's supported calculation are rejected rather than treated as a guessed offset. No blanket ASCII restriction is applied to the calendar document or opaque event recurrence text.

## RSVP is a separate command capability, currently blocked

Do not expose an enabled Accept/Tentative/Decline control until the actual send seam exists. The future contract must include:

- User-selected status and a fresh, explicit confirmation; verified responding account alias; backend-qualified account and message identity; request ID and source fetch/session token.
- Full event UID, organizer, sequence and recurrence instance **including its VALUE/TZID parameters**, not a display-truncated UID or an unqualified `Invite::key()` shared across accounts.
- Policy rejecting cancellations, stale event revisions, malformed organizer/attendee addresses and an identity that is not eligible to respond. Calendar metadata is not proof of organizer authentication.
- Real iTIP `METHOD:REPLY` MIME construction and actual adapter send admission through the existing durable send/outbox pipeline. Distinguish rejected, durably queued, sent and ambiguous/unknown outcomes; do not label queued as delivered or promise exactly-once SMTP.
- Synthetic transport tests against the same production command seam, including recurrence TZID, cancellation, stale epochs, alternate verified aliases and SMTP uncertainty. No send during preview or contacts import.

`Invite::recurrence_id` currently drops the content-line TZID/VALUE parameters and `reply_ics` writes only its value. That is an additional RSVP correctness obligation for the command owner; this preview instead retains the original clock parameters from `ics`. Changing the response model/send path was not authorized in this slice. Existing GTK reply construction is not proof of a native RSVP feature.

## DAV/provider contacts and calendars: integration contract, not an implementation

No authenticated CardDAV/CalDAV discovery, contact sync, event CRUD, subscribed-calendar fetch, tasks, alarms or unified calendar exists in this slice. Local CRUD is not provider CRUD. IMAP/TB mail access does not establish DAV permission.

Required adapter contract:

```text
Grant: provider_id + backend-qualified account + credential_reference
       + validated endpoint/origin + granted permission set + auth/session epoch
Capabilities: list_address_books, read_contacts, write_contacts,
              list_calendars, read_events, write_events, scheduling
Request: request_id + Grant reference + auth/session epoch
         + collection/resource identity + explicit operation + concurrency precondition
Result: same request/account/epoch/resource identity
        + complete/partial + typed auth/scope/offline/conflict/unsupported/failure outcome
```

Requirements per operation:

1. **Auth/discovery:** provider-owned client registration and actual granted contact/calendar read/write scopes, or a provider-supported DAV app-password flow, separately validated by the auth owner. Consent must identify contact/calendar access. Use Secret Service references, never tokens/passwords in the local book. Do not copy a Thunderbird mail OAuth registration/grant or assume an IMAP password/scope authorizes DAV. Provider scope strings and endpoint policies must come from that provider's documented registration, not a generic invented scope.
2. **Collections/read:** bounded authenticated discovery, stable collection IDs and resource hrefs confined to the validated origin; bounded pages, bodies and XML depth. Carry sync-token/ETag and explicit partial results. No profile scraping or automatic subscribed-ICS network request merely because an email contains a URL.
3. **Contact write:** create with `If-None-Match: *`; update/delete with the last resource ETag in `If-Match`. A 409/412 conflict preserves local edits and requests explicit resolution. Model provider UID/href/ETag separately from this local book's IDs; local emails alone are not remote object keys.
4. **Event write:** same ETag/conflict rules, with UID, sequence, recurrence ID plus parameters, timezone source, recurrence exceptions, attendees and scheduling semantics preserved. Split event storage CRUD from scheduling/RSVP mail. Do not silently add alarms, send attendee notifications or materialize guessed recurrence instances.
5. **Transport/privacy:** validated TLS, bounded redirects restricted to authorized origins, no credential forwarding across origins, no plaintext fallback. Apply the existing timeout/cancellation/backpressure principles; return causally correlated terminal outcomes and redact diagnostics. Preserve account isolation and encrypted-content exclusions.

**Exact blocker:** there is no selected/registered native DAV/provider auth grant, endpoint/scopes contract or authenticated DAV command adapter in the owned files. The current password-mail and isolated TB mail paths cannot supply that implicitly. Auth/provider owner must supply the grant and capabilities before network CRUD can be implemented honestly. Calendar views, task/reminder lifecycle and recurrence scheduling are later product slices, not a static date grid bolted onto this preview.

## Validation and honest assessment

Executed, without native compilation:

- `rustfmt --edition 2024` and `rustfmt --edition 2024 --check` for the three owned Rust files: passed.
- `git diff --check -- src/invite.rs`: passed. New files were formatting-checked and checked for trailing whitespace by the final static audit.
- Inline Python read-only source/arithmetic audit: **13 assertions passed**. It compared the original helper from Git with current source, established the invalid UTF-8 byte boundary and out-of-i64 arithmetic condition, and checked retained public signatures, in-parser bounds, checked math, metadata/core usage, no RSVP send, scoped store paths, revision/atomic replacement and completion cap. This is not Rust execution, a security penetration test or product behavior validation.

Authored but unexecuted: **11 local-contact tests**, **11 native-preview tests**, and **7 new core regressions** (21 core tests total including existing tests). Real-store tests use newly created private temporary directories, not user profiles. MIME tests call the real core helper and then the real preview seam. Covered cases include persisted CRUD/reopen/nonrecycled IDs, scoped completion, duplicate identities, import rollback/bridged-record rejection, UTF-8/folding/escaping/ignored photos, malformed/oversize store/import, revisions/locks/private modes/symlinks, calendar/octet-stream attachment MIME, recurrence/TZ/cancellation/status/URL handling, participant bounds, missing metadata, helper arithmetic/Unicode failures and parser bounds.

Parent must include these modules in its one coordinated build window and arrange test execution under its own serialized validation policy. No Cargo/check/clippy/rustc or helper compilation was invoked here. Not tested: native UI integration, request/epoch correlation, corruption/crash power-loss recovery, filesystem adversarial races, Linux runtime/accessibility, provider DAV/auth, real TB calendar metadata, native RSVP, or performance.

**Good:** bounded local data implementation with transactional revisions and synthetic real-store tests; production core no longer relies on an optional native preflight to avoid the identified slice/arithmetic failures; actual MIME helper reused; scope/privacy/no-send boundaries explicit; no engine rewrite or dependency addition.

**Bad / incomplete:** new local contacts/preview modules remain unwired until the parent integrates them; all Rust code and tests remain uncompiled/unexecuted in this task; import/address subsets are intentionally limited; no full calendar, tasks, provider sync or RSVP transport.

**Risk:** stricter parser admission needs the existing compatibility corpus; stale-check correlation is not provided by current event fields; private JSON is not encrypted and a stale lock needs deliberate recovery; arbitrary VTIMEZONE/DST/recurrence correctness is not established. No speed, security-completeness, provider coverage, delivery or feature-parity claim is warranted.

**Release:** ownership of the four listed files is frozen after the final formatting/static checks. Parent/native/C1 owners may integrate the documented APIs after release. Any build failure or failed runtime regression remains for the parent's build window; it is not hidden by these static checks.

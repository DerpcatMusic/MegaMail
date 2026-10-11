# Native MegaMail capability and foundation audit

The latest source integration also includes typed, read-only Outbox delivery
review and cancellation of the unsent remainder after an uncertain bulk result.
Held mail never gains retry permission from this view. These paths still need
Rust compilation and native acceptance checks before they can be called shipped.

**Audit date:** 2026-10-10 UTC. **Source snapshot:** `d4dbd2bba14f474a33870269cc15bd98376ff360`.
**Target:** Linux GPUI app in `apps/megamail/`, not the root GTK Hylki application.

## Implementation evidence update — 2026-10-10, correction5

The 189-row status counts below remain the **original audit baseline**, not a new parity or shipment count. Parent completed the large source batch and separately recorded/reviewed freeze-correction5.json (575 inputs) before its grouped serial verification. **599 ordinary core/public, 267 native and 25 theme tests passed**; normal native build, production helper/extension +test-only no-CSP build, 35 compiled policy assertions/fixture policy checks and license refresh passed. A calibrated isolated production connect-budget test passed separately. Normal private Thunderbird probes stayed ignored.

Native production wiring now exists for serialized provider/password admission, persistent mixed-account inventory/retry, mutation-driven draft recovery/receipts/leases, stamped direct IMAP bodies, guarded activation/actions and causal read-only invitation. This does not establish real provider/TB operation, crash-safe shutdown, full global UID safety or an installed feature.

A real, network/private-profile-isolated fictional demo opened and yielded owned-window captures; the 1060x640 view wraps long sender/recipient metadata and subject. Input delivery did not yet show selection/composer changes. Actual selectable-state Copy acceptance is a separate ongoing probe—not a screenshot claim.

The compiled isolated original-HTML helper now reaches document-load READY+heartbeat for an authored fictional fixture. It is still developer-only/temporary, **not inline GPUI fidelity**; actual hostile request/descendant trace gates are ongoing. CSS token safety, real MIME CID/image transport, remote consent, accessibility and physical fetch-to-paint remain incomplete/unverified. Its normal release gate remains closed.

Contacts, compose productivity, rules preview, parsed-search routing and bulk triage units now compile/run, but their native user call paths remain **unwired**. Optional AI/team work is policy only. Do not upgrade matrix rows on the basis of test/module existence. See implementation-batches.md and batch2-verification-results.md for dated good/red/blocked evidence.

## Native integration update — 2026-10-11, source only

The user now owns build/installation qualification. The following production
call paths have been added in source; none of these newest changes has been
compiled or exercised in the app:

- Parsed search reaches the real SQLite query lane, including bound operators,
  pagination and corrected attachment flags. Body predicates cover cached direct
  mail only; encrypted bodies and missing bodies remain unknown, including
  negated predicates. Thunderbird uses parsed filtering over loaded headers;
  this is not all-history Thunderbird search.
- The composer opens account/backend-scoped local contacts, prefix completion,
  explicit To/Cc/Bcc append, contact create/update/delete, paged contact browsing
  and vCard import/export. Writes use store revisions. Export refuses existing
  destinations; import/export are bounded to 1 MiB. Photos, groups, CardDAV,
  LDAP and LDIF remain absent.
- The composer reads and edits signatures/snippets/templates in a private local
  library. Direct and Thunderbird libraries have separate filenames. Library
  updates use canonical snapshot comparison under the writer lock. Variable
  values are explicit; template subject replacement is a separate deliberate
  action. Full outgoing payload/caret guards preserve hidden fields and recovery.
  Rich HTML templates and provider-synchronized libraries remain absent. Local direct-account scheduling is described below.
- The reader can save its currently rendered text through a native file dialog.
  It uses the same selectable semantic text that Copy reads, checks selection
  lifetime before dispatch, writes off the UI executor and preserves existing
  destination files. It exports displayed text only, including any displayed
  decrypted content; hidden quotes and attachments are omitted. Full RFC822 export and a native print dialog/direct PDF still need separate implementation. Printable HTML export now escapes the current text/headers and disables external requests; users print or save PDF from a browser. A bounded exact-text find control now searches the displayed body with previous/next excerpts. It does not scroll/highlight inside the message body.
- Native multi-selection now supports individual rows, visible ranges and all
  loaded rows (500 per action). Read/unread, star/unstar, archive, trash and a
  discovered custom destination have a serial qualified worker lane. Moves
  require destination review and confirmation; all outcomes are per item.
  Direct IMAP validates worker session, UIDVALIDITY and persisted generation,
  checks UID presence and verifies flag changes. Thunderbird resolves the actual
  account/folder/message mapping and acknowledges its API result, which is not
  an independent remote-server commit. Unsupported MOVE has no unsafe fallback.
  Confirmed flags support undo with current-flag preflight; preflight is not an
  atomic CONDSTORE transaction. Moves remain manually reversible; arbitrary
  provider-wide selections, labels and permanent purge are still absent.
- The reader previews actual saved rules against its selected message. Native
  local rule management now creates/edits subject or sender Contains rules,
  lists all saved rules in pages and confirms deletion. CAS saves under a writer
  lock reject newer/corrupt snapshots. Advanced rules are preserved. A previewed
  supported move can prepare the selected message for explicit triage
  confirmation, with retained reader identity and worker scope. Missing bodies,
  tag actions and unsupported unread policies cannot become an executable plan.
  Automatic incoming-rule execution and bulk rule application remain absent.
- Direct IMAP composers can set Send Later using an explicit date/time/numeric
  UTC offset. The full outgoing payload and local recovery retain send_at;
  unreviewed/invalid/past times cannot silently become immediate sends. Delivery
  uses the existing durable Outbox and requires MegaMail running/connected; when
  closed it resumes after launch. The native Outbox now carries typed persisted
  delivery phases. Confirmed cancellation/discard uses an exact owning worker,
  immutable AUTOINCREMENT id and schedule, plus an atomic ready-only database
  deletion. It cannot delete claimed, accepted, refused, uncertain or due
  scheduled messages. Held retry/reconciliation, Thunderbird scheduling and
  server-side delivery while the app is closed remain absent.
- Downloaded/cached PNG/JPEG/WebP attachments have explicit native static-image
  preview, one background decoder at a time, a maximum 8 MiB input and strict
  4096-pixel dimension limits. It validates the current reader and exact retained
  attachment before applying a result, scales the displayed preview, converts
  RGBA to GPUI BGRA and retains original bytes. Decoder allocation limits are
  best effort, not a process sandbox. PDF preview, EXIF orientation correction
  and attachment galleries remain absent.
- Newsletter metadata now has explicit review-email and review-webpage controls.
  Email routes open an unsent draft on the owning account, preserving current
  metadata identity and refusing copied/hidden recipients or malformed headers.
  Browser actions validate the actual current route and show its URL. Automatic
  RFC8058 POST still requires a verified provider policy and is not implemented.
  Find-in-message can be focused with Ctrl+Shift+F; Ctrl+F still searches mail.


The baseline rows below remain historical. These source paths do not establish
feature acceptance. Bulk triage still needs typed, correlated backend outcomes;
legacy settlement events cannot be used as success receipts. Inline faithful
HTML/CID/remote consent, timed delivery, complete account lifecycle, calendar
CRUD/synchronization, and optional team/AI functionality remain open work.

## Decision summary: fix the foundations before feature fanout (baseline)

1. **Keep the Thunderbird adapter available.** It is a real second adapter, not a fake import or a cache migration. Its profile/session lifecycle, interactive authentication limitation, version dependency, and offline behavior need hardening. Do not replace it merely because a direct Rust adapter might be faster.
2. **Stop conflating saved configuration with successful connection.** Startup already restores saved Thunderbird accounts. A return to setup is not proof that persistence is missing. Preserve configured accounts during keyring errors, offline starts, missing source profiles, and bridge failures; use explicit reconnect/repair states.
3. **Implement real provider authorization.** Native setup offers app-password/manual IMAP+SMTP, not Google/Microsoft browser sign-in. Core OAuth helpers and Graph/JMAP workers exist, but existence is not native onboarding. Use MegaMail-owned registrations and reviewed scopes; never reuse Thunderbird's identity.
4. **Close mail-loss/duplicate-send risks before triage/AI.** Direct send failure retries, mailbox UID epochs, shutdown/draft recovery, migration failures, and cross-adapter request completion need adversarial fixtures. This audit did not execute a reproduction and does not call those runtime failures proven.
5. **Treat faithful HTML/CSS reading and selected-body latency as P0 foundations.** The parent's latest screenshot-driven brief explicitly prioritizes styled layout, CID images, consented remote images, responsive reader behavior, and fast selected-body presentation. Current semantic HTML is **partial**, not full styled HTML: `mail_text::reader_html` removes sender CSS, dimensions/images and presentation-table layout [N23]. Preserve privacy while restoring layout; never enable remote fetching by default.
6. **Make capability differences explicit.** Backend methods, GTK features, Thunderbird desktop features, and native controls are four different inventories. Search, templates, bulk operations, contacts/calendar, encryption controls, and notifications are not native merely because shared source supports them.

The **189-row matrix** below inventories distinct user outcomes, not a promise of exact visual parity. Competing clients also have gaps: Superhuman Mail has no real Unified Inbox; Spark is not a local archival/backup client; several paid, mobile, provider, and server-dependent features are limited. Marketing speed claims are not benchmark evidence.

## Scope, method, and confidence

Read `AGENTS.md`, `PRODUCT.md`, `docs/MEGAMAIL_ARCHITECTURE.md`, native main/live/compose/onboarding/settings/preferences/adapter code, core configuration/query/OAuth/bridge code, and relevant shared worker/cache/reader and legacy feature modules. The code findings describe the recorded audit snapshot, not completion claims for concurrent implementation work. Parent has since started four disjoint foundation owners (startup/restore, OAuth callback, discovery authentication alternatives, offline-send/outbox); do not duplicate or overwrite them. Dedicated HTML-renderer and selected-body-latency researchers own deeper analysis in separate documents. This inventory records their requirements and integration contracts without repeating their research or waiting for it. Queried `graft ask ... --source` first; its index returned no matching nodes, so used scoped `rg` and source reads. Loaded Rust, codebase-design, find-docs, diagnosing-bugs, and research skills. Batched independent reads and public-document retrieval with codemode.

**No Cargo builds/tests/checks, implementation, commits, profile/credential reads, or mail-server mutations were performed.** Public first-party HTTP documentation was read. A background browser opened only Mozilla's public KB and was closed afterward. Parent owns the serial Cargo slot. Existing test source is evidence of intended coverage, not a test result from this audit.

Native status:
- **implemented**: reachable native source path exists. Operational/provider validation is still unverified unless explicitly noted.
- **partial**: some native behavior exists, but scope, fidelity, adapter parity, or controls are incomplete.
- **missing**: no native user path found in the inspected module inventory. Legacy support is identified separately; this is new porting/product work, not automatically a bug.
- **unverified**: behavior cannot be established from this static audit or requires runtime, provider, security, or accessibility validation.

Findings:
- **C**: confirmed source behavior or omission; no live reproduction implied.
- **S**: suspected runtime bug/risk; not a diagnosis. The diagnosing-bugs reproduction gate cannot be met under this research-only restriction.
- **W**: already implemented but operationally unverified; preserve and test instead of rebuilding.
- **N**: new native work.

Competitor notation:
- **Yes** = fetched first-party documentation explicitly describes it.
- **API** = Thunderbird's current desktop engine/API explicitly documents the capability, **not verification of a matching desktop UI workflow**.
- **No** = explicit first-party limitation.
- **Unknown** = not established; absence from marketing is not evidence of absence.
- **Planned** = advertised as coming soon, not counted as available.
- TB defaults to **free desktop Linux/Windows/macOS**. Standards pages separately describe Android; desktop parity must not be assumed there.
- Spark defaults to **macOS/Windows/iOS/iPadOS/Android**, no documented Linux release. “Plan ?” means the fetched feature/tutorial page did not establish entitlement. Current plans are Free/Plus/Pro/Enterprise, with legacy exceptions [SP].
- Superhuman defaults to **paid Gmail/Microsoft 365 accounts**. Starter/Business/Enterprise entitlements are identified when verified [HP]. Native Mac/Windows, browser/Chrome, iOS/iPadOS, Android are documented [HD]; not every feature is on every platform.
- Unless a cell provides a stronger statement, version, tenant/provider policy, exact plan entitlement, and platform-specific UI parity are **unknown**.

**Retrieval limitations:** Mozilla SUMO KB returned an HTTP-200 “Client Challenge”, including in a normal browser. Its pages were not treated as fetched content. Thunderbird's live desktop page, 157.0 release notes, standards page, and generated **157.0 Manifest V2** API docs were available and used instead. Superhuman's old help root/old `/article/` URLs failed, but its current `/hc/` sitemap and public Zendesk article JSON returned first-party content. Spark articles and pricing were fetched live. Source conflicts are called out rather than silently resolved. This is comprehensive discovery, **not exhaustive runtime certification**.

## Foundation and recurring-import findings

| Finding | Class / priority | Evidence and consequence | Narrow next verification / fix |
|---|---|---|---|
| Startup restoration already exists | W / P0 | [N1] `MailApp::new` calls `has_saved_accounts` and `LiveMailbox::restore_thunderbird`; [N2] restores asynchronously; [N3] `restore_saved` reuses app IDs. Empty list copy says “No import is needed” [N1]. Repeated prompts must not be “fixed” by adding a second import mechanism. | Restart with a fictional saved manifest; offline/bridge-start failure should show configured accounts with reconnect, never first-run instructions. |
| Manifest-read error is treated like “no saved accounts” for routing | C / P0 | [N1] startup `saved_thunderbird.unwrap_or(false)` selects Setup on the empty direct-account branch. Restore separately surfaces an error, but routing loses the difference between absent and unreadable manifest [N3]. | Missing, malformed, unsupported-version, permission-denied manifests must have distinct states. Preserve error and repair path; do not overwrite with an empty store. |
| Direct-account load failure skips Thunderbird startup | C / P0 | [N1] `Err(error)` sets `View::Setup` without the restore calls in the two successful branches. A direct-account configuration/keyring/profile-ID error therefore blocks independent startup of saved Thunderbird accounts. | Inject direct-load failure plus valid saved Thunderbird references; assert independent restore and non-destructive recovery. |
| Initial failed imports are not necessarily saved | C / P0 | [N3] `import_selected` records `new_profiles` from returned ready accounts and writes only if `started` is nonempty; `add_account` rejects failed authentication/prefetch. Failed selections have outcomes but no durable configured intent. Saved successful selections do remain saved. | A mixed success/needs-login import should remember **all authorized selections** as configured-but-unavailable with stable IDs, not require re-selection. Decide policy for explicitly unsupported entries. |
| Restoration still depends on discoverable original source profile | C / P0 design constraint | [N3] `restore_saved` re-discovers standard profile roots and matches canonical source paths. An unavailable/moved source yields failure even though a private clone may exist. [N4] clone separates mail storage. | State whether source is an ongoing dependency or just migration provenance. Add safe source relocation/repair, or self-contained clone restore after a deliberate design decision. Never search arbitrary private profiles without consent. |
| Headless Thunderbird cannot complete interactive login/unlock | C / P0 gap | [A] and [N3] classify needs-login; [N4] isolates runtime. No native provider or Primary Password interaction completes that state. Import is not a general sign-in flow. | Provide explicit repair/unlock/re-authorize path with permission and clear lifecycle. Preserve unsupported primary-password state until a safe interactive design exists. |
| Google/Microsoft core OAuth is not a native sign-in button | C / P0 gap | [N5] OAuth presets, empty app-owned defaults, PKCE flow helpers; [N6] setup validates credentials but native [N7] offers password discovery/manual forms. `oauth_settings` is not called by that UI. | Registration/release policy first, then browser consent/cancel/timeout/retry/refresh/revocation fixtures. Distinguish account authorization from server discovery. |
| Direct SMTP can queue non-refused errors without an uncertain outcome | S / P0 | [N8] direct send branch calls `send_failed`; non-refused errors are queued for retry. A lost SMTP final acknowledgment may mean accepted mail; no direct equivalent of Thunderbird `SendFailure::Uncertain` was found. **Duplicate delivery not reproduced.** | Loopback SMTP fixture accepts DATA then drops final acknowledgment. Assert explicit uncertain delivery and no blind automatic retry. Parent owns execution. |
| Direct cached identity has no inspected UIDVALIDITY epoch | S / P0 | [N9] schema keys `(account_id,folder_path,uid)`; scoped search found UIDNEXT handling but no UIDVALIDITY invalidation in inspected cache/worker/models/core/native. [N10] protects account/folder identity, which does not solve reused UIDs after mailbox recreation. **Wrong-message behavior not reproduced.** | Fixture changes UIDVALIDITY and reuses a UID; assert old body/attachments/flags are not applied or mutated. Check protocol library and all callers before fixing. |
| Draft save completion is coupled to current-account status | S / P1 | [N10] `DraftSaved` sets “Draft saved” only for `current_account_id`; [N1] composer observes text status, unlike send's request/account correlation. Cross-account Unified Inbox draft editing or overlapping statuses needs verification. | Save a draft owned by another account while Unified/current selection differs; require request-ID and account-scoped completion. Do not classify a hang proven yet. |
| Empty-To sends are rejected even when Cc/Bcc has recipients | C / P1 | [N11] `send_current_message` requires nonempty To; the shared builder processes To/Cc/Bcc independently [N8]. This is a confirmed native validation restriction, not a live SMTP result. | Fixture composes Cc-only/Bcc-only mail; accept at least one valid envelope recipient, preserve Bcc secrecy, and require no provider send to test validation. |
| Manual drafts exist; durable autosave/crash recovery does not follow | N / P0 | [N11] explicit save uses `save_draft(message,None,...)`; [N12] plain-text composer; no native timer/local recovery store/window-close recovery found. Explicit Discard is correctly labeled; do not call it accidental loss. | Local private recovery snapshot, attachment ownership, atomic save, crash/restart, close prompt, and provider-draft reconciliation. |
| Existing bounds and stale-result checks are meaningful foundations | W / P0 preserve | [N13] query queue 32, generation/coalescing, 100-row pages, cache cap 500; [N10] mailbox cap 5,000, scoped results; [N3] bounded adapter queue/body/attachments. Event/direct request channels also include unbounded paths. | Measure total memory and backlog under slow providers; do not equate one bounded query queue with globally bounded execution. |
| Thunderbird queued-send controls absent in native app | C / P0/P1 | [N14] `LoadOutbox` is a no-op; retry/delete reject with advice to open Thunderbird's Outbox. That is the **isolated runtime**, not necessarily the user's normal profile. [A] already documents gap. | Native outbox status/recovery or exact authorized private-runtime handoff. Never send/migrate source Outbox automatically. |
| Styled email fidelity is absent despite a semantic HTML view | C / P0 gap | [N23] `reader_html` discards sender CSS, dimensions, images/resources, and presentation-table layout; [N22] displays the sanitized fragment. This is **partial HTML support**, not faithful HTML/CSS reading. Screenshot priority comes from the parent's updated brief; this audit did not independently inspect that screenshot. | Dedicated renderer research owns engine/options analysis. Acceptance must retain safe CSS/layout, render CID, default-block remote images with explicit consent, and preserve selection/accessibility/plain fallback. |
| Selected-body latency and responsive styled layout are not certified | S / P0 performance gap | [N2], [N3] fetch selected bodies; [N1] `live_changed` retains reader content; [N22], [N23] convert/display semantic HTML. Bounds/cache existence do not establish first-visible-body speed or faithful reflow. No latency sample or slow-stage diagnosis was made here. | Dedicated latency research owns tracing/baselines. Measure selection-to-request/body-ready/first-visible-render cold/warm per adapter, cancellation and cache; validate narrow/wide/resized readers without obsolete content. |
| Existing loopback/probe evidence is narrower than full reliability | W / P0 | [A]/[P] report six authorized imported IMAP accounts and Thunderbird 157 loopback compose/draft checks; direct-provider ongoing sync/send is explicitly unverified. This audit did not repeat or inspect private artifacts. | Separate fictional protocol fixtures, Linux UI validation, then user-authorized provider read/send/delivery certification with mutations separately approved. |

### Reproduction gate for the parent

No red-capable runtime loop was run here. For S findings, first construct a deterministic fixture that exercises the actual interface and exact symptom; only then rank causal hypotheses, instrument one prediction at a time, and fix. Suggested fixture matrix: saved/absent/unreadable config; direct-only/TB-only/mixed accounts; locked keyring; missing/old/crashed Thunderbird; needs-login; startup offline then recovery; mailbox epoch reset; stale page/body/attachment reply; send accepted but acknowledgment lost; disk-full outbox/draft writes; crash between queue insert and old-row deletion; rapid account/folder switching. Use temporary fictional profiles. Existing tests must not be presented as passing unless parent runs them.

## Capability matrix

Priorities: **P0** correctness/authentication/data protection **plus faithful HTML/CSS reading and selected-body latency**; **P1** everyday native parity; **P2** advanced productivity; **P3** optional differentiation/platform scope. Dependency packages are defined after the matrix.

### 1. Account lifecycle, authentication, and protocols

| ID | Meaningful capability / user outcome | Native MegaMail status and evidence | Thunderbird | Spark | Superhuman Mail | Priority / dependencies |
|---|---|---|---|---|---|---|
| F01 | Saved accounts reconnect without another import | partial; restore exists, failure routing remains [N1], [N3] | API account enumeration [TA]; restart UX unknown | Yes cross-device account sync [SA]; plan ? | Yes saved account management [HA]; mobile added separately | P0 F0/F1 |
| F02 | Unreadable account configuration shows repair rather than first-run setup | partial; error surfaced but routing conflates states [N1] | Unknown | Unknown | Unknown | P0 F0/F1 |
| F03 | Failed initial imports retain configured intent for later retry | missing for rejected selections [N3] | Unknown | Unknown | Unknown | P0 F1 |
| F04 | Import several Thunderbird IMAP accounts in one selected batch | implemented; scanner/picker/adapter [N1], [N3], [A] | API multiple accounts [TA]; import UI details unknown | Unknown for Thunderbird import | Unknown for Thunderbird import | P0 preserve F1/F2 |
| F05 | Use Thunderbird as an ongoing isolated mail backend | implemented; private runtime; 153+ required [N4], [A] | Not applicable; standalone client [TD] | Unknown | Unknown | P0 preserve F2 |
| F06 | Recover after Thunderbird binary/process/bridge failure without re-import | partial; refresh/reconnect classification [N2], [N3] | Unknown | Unknown | Unknown | P0 F0/F2 |
| F07 | Repair an unavailable or relocated imported source profile | missing native repair control [N3] | Unknown | Unknown | Unknown | P0 F1/F2 |
| F08 | Interactive unlock for imported Primary Password protected store | missing; headless needs-login [A], [N4] | Unknown detailed UI; blocked KB [TBLOCK] | Unknown | Unknown | P0 F2 |
| F09 | Google browser OAuth with no app password | missing native flow; reusable helpers [N5], [N6], [N7] | Yes OAuth/Gmail fixes [TS], [TR]; provider policy | Yes Google consent [SA]; provider policy | Yes Sign in with Google [HA]; Gmail hosted only | P0 F3 |
| F10 | Microsoft browser OAuth for personal and tenant accounts | missing native flow; Graph/helpers latent [N5], [N8] | Yes OAuth standard [TS]; account-policy detail unknown | Yes Microsoft provider sign-in [SA]; tenant limits ? | Yes Microsoft 365 [HA]; tenant SSO [HS] | P0 F4 |
| F11 | App-password Gmail connection where account policy permits | implemented; test/add IMAP+SMTP [N7], [P]; live unverified | Yes SMTP/IMAP [TS]; app-password policy unknown | Yes Gmail [SA]; app-password requirement ? | No generic password setup documented; OAuth path [HA] | P0 preserve F3 |
| F12 | Custom IMAP account via discovery and editable server settings | implemented; password-based [N7], [A] | Yes IMAP/manual config [TS], [TR] | Yes custom IMAP [SA] | No; Gmail/Microsoft 365 only [HA] | P1 F4 |
| F13 | Separate SMTP login credentials or no-login SMTP | implemented; TLS required [N7], [A] | Yes SMTP [TS]; exact auth controls unknown | Yes custom account [SA]; separate login control unknown | No generic SMTP account connection [HA] | P1 preserve F4 |
| F14 | Explicit TLS/STARTTLS and reject plaintext transport | implemented direct setup checks [N7], [A]; security unverified | Unknown exact policy in fetched docs | Yes Spark TLS policy [SV]; server TLS controls ? | Unknown exact transport policy | P0 F4/D1 |
| F15 | Token refresh, expired consent, and revoked-access recovery | partial; core refresh exists, native OAuth repair missing [N5] | Yes OAuth [TS]; recovery UX unknown | Yes revoke app token via provider [SV]; repair UX ? | Yes provider login [HA]; repair/error detail ? | P0 F3/F4 |
| F16 | Add, rename/reorder, edit, remove, and reconnect accounts without file editing | partial; add/open inbox; edit/remove absent [N21], [N7] | API account read [TA]; UI management details unknown | Yes add/remove/name/color tutorials [ST]; plan ? | Yes add/remove/reorder [HA] | P1 F0/F1 |
| F17 | POP3 accounts and local download policies | missing native setup/import [N7], [A] | Yes desktop and Android POP3 [TS] | No supported list is IMAP/EWS/Graph [SA], [SO] | No [HA] | P2 F4/F6 |
| F18 | JMAP account discovery/authentication/sync | missing native setup; direct worker dispatch exists [N8], [N7] | Unknown | Unknown | No supported account types [HA] | P2 F4 |
| F19 | Exchange/Graph account mail when IMAP is disabled | missing native setup; Graph worker latent [N8], [N5] | API EWS since 142 [TA]; not a Graph promise | Yes Exchange/EWS/Graph configurations [SA], [SO] | Yes Microsoft 365 [HA]; protocol implementation ? | P1 F4 |
| F20 | Proton Bridge/local-only provider compatibility | unverified; generic TLS path not a compatibility claim [N7] | API IMAP/SMTP [TS]; Bridge compatibility not verified here | Yes desktop Mac/Windows; many cloud features unavailable [SB] | No [HA] | P2 F4/D1 |
| F21 | NNTP newsgroups and RSS/Atom subscriptions | missing native UI [N15], [P] | Yes desktop NNTP/RSS/Atom [TS]; RSS Android N/A | Unknown | No new account types beyond Gmail/M365 [HA] | P3 F4 |

### 2. Delivery, data integrity, and offline foundations

| ID | Meaningful capability / user outcome | Native MegaMail status and evidence | Thunderbird | Spark | Superhuman Mail | Priority / dependencies |
|---|---|---|---|---|---|---|
| D01 | Account/folder-scoped message identity across equal UIDs | implemented; MessageKey [N10] | API message IDs session-local, not header IDs [TM] | Unknown internal identity model | Unknown internal identity model | P0 preserve F0/F6 |
| D02 | Invalidate old message bodies/flags when IMAP UIDVALIDITY changes | unverified; inspected schema lacks epoch; suspected risk [N9], [N8] | Unknown; IMAP standard support is not proof of epoch handling [TS] | Unknown | Unknown | P0 F6/D1 |
| D03 | Reject stale folder/page/body/attachment responses after switching | implemented checks; runtime unverified [N10], [N13] | API paginated query/abort [TM], [TI]; UI races unknown | Unknown | Unknown | P0 preserve F0/D1 |
| D04 | Bound mailbox headers, reader memory, query backlog, MIME/attachments | partial; explicit bounds, some unbounded channels [N13], [N3], [N10] | API paginated lists [TI]; total memory unknown | Unknown | Offline cache cap documented, total memory unknown [HO] | P0 F0/F6/D1 |
| D05 | Persist failed direct sends in durable Outbox before claiming queued | implemented direct worker/cache [N8], [N9]; live unverified | API queue to Outbox [TC]; crash guarantee unknown | Yes Outbox undo delay [SU]; durable crash guarantee unknown | Yes offline work synchronizes [HO]; crash guarantee unknown | P0 preserve F5 |
| D06 | Retry/delete queued messages through native Outbox controls | partial; direct controls; TB rejects/no list [N2], [N14] | API sendLater queue [TC]; queue-management UI unknown | Yes cancel during undo delay [SU]; full retry/discard detail ? | Yes offline pending sends [HO]; full queue UI ? | P0/P1 F5/F2 |
| D07 | Show uncertain delivery after lost acknowledgment and suppress blind retry | partial; TB uncertain lock; direct path at risk [N11], [N8], [N14] | Unknown | Unknown | Unknown | P0 F5/D1 |
| D08 | Report accepted send separately from Sent-folder copy failure | implemented direct worker path [N8]; live unverified | Yes Sent-save reliability fix [TR]; API sent copies [TC] | Unknown | Unknown | P0 preserve F5 |
| D09 | Private local drafts survive crash, disk-full, and restart | missing native recovery store; manual remote save only [N11], [N12] | API save draft [TC]; crash semantics unknown | Unknown | Yes offline reply/work [HO]; crash semantics unknown | P0 F7 |
| D10 | Request-correlated draft-save acknowledgment | partial; status text/current account coupling [N10], [N11] | API save Promise result [TC] | Unknown | Unknown | P0/P1 F0/F7 |
| D11 | Read cached bodies offline with an explicit retention policy | partial; direct cache, TB own cache; availability unverified [A], [N9], [N3] | Unknown detailed offline KB blocked [TBLOCK] | Yes past-month Wi-Fi cache, read-only offline [SO] | Yes recent 30 days plus 1250 per Split; attachments [HO] | P1 F6/S1 |
| D12 | Queue offline triage operations and reconcile after reconnect | unverified; do not infer from worker or cache [N8], [N9] | Unknown | No offline move/copy; cached reading only [SO] | Yes offline inbox triage [HO] | P1 F0/F6/T1 |
| D13 | Offline attachment availability and bounded prefetch controls | partial; explicit download/cache path; TB cache policy unknown [N2], [A] | Unknown | Yes cached past-month attachments [SO] | Yes auto-download cached attachments [HO] | P1 F6/S1 |
| D14 | Configured accounts stay visible during partial unified-sync failure | partial; successful results/warnings retained, failed startup accounts absent [N10], [N3] | Yes unified/multi-account [TD]; partial-failure semantics ? | Yes unified [SF]; failure semantics ? | No true Unified Inbox [HU] | P0 F0/F1 |
| D15 | Background new-mail sync plus explicit refresh/reconnect | implemented direct worker; TB refresh interval 60s [N8], [N3], [N2] | Yes IMAP IDLE [TS] | Yes notifications [SF]; polling mechanics ? | Yes notifications [HN]; mechanics ? | P1 preserve F2/D1 |
| D16 | Cache schema upgrades are recoverable without silent mail-state loss | unverified; schema version/migrations exist [N9] | Unknown | Unknown | Unknown | P0 F6/D1 |
| D17 | Profile/cache/keyring namespace isolated from Hylki and source Thunderbird | implemented private namespaces/clone [N4], [N9], [N5], [A] | Standalone Thunderbird [TD]; third-party clone not applicable | Unknown | Unknown | P0 preserve F1/F2 |
| D18 | Single-instance/profile locking prevents concurrent profile corruption | implemented instance lock; runtime unverified [N26] | Unknown | Unknown | Unknown | P0 preserve F2/D1 |

### 3. Reading, conversations, and attachments

| ID | Meaningful capability / user outcome | Native MegaMail status and evidence | Thunderbird | Spark | Superhuman Mail | Priority / dependencies |
|---|---|---|---|---|---|---|
| R01 | Unified Inbox independently pages multiple account Inboxes | implemented; per-account page state/warnings [N10] | Yes unified Inbox [TD] | Yes unified Inbox [SF] | No; tabs workaround Mac/Windows [HU] | P1 preserve F0 |
| R02 | Separate account Inbox and folder navigation | implemented [N2], [N1] | API account/folder read [TA], [TF] | Yes separate Inboxes tutorial [ST] | Yes folder/account navigation [HA], [HF] | P1 preserve T1 |
| R03 | Reference-header conversations across folders and Sent | partial; bounded candidates/ancestors, no subject-only grouping [N10], [A] | API header/full-text query [TM]; exact UI grouping scope ? | Yes threads [SF], [ST]; algorithm ? | Yes conversations [HC]; algorithm ? | P1 F0/S1 |
| R04 | Expand individual replies chronologically within conversations | implemented native list/reader [N1], [A] | Unknown exact UI | Yes grouped threads [ST]; order specifics ? | Yes conversation view [HC]; order specifics ? | P1 preserve F0 |
| R05 | Read selectable semantic HTML headings/lists/quotes/tables without claiming full fidelity | partial; semantic subset wired, not full styled HTML [N22], [N23] | Yes MIME [TS]; rendered-HTML UI fidelity not established | Yes email display; rich text tutorials [ST]; fidelity ? | Yes message view/formatting [HFORMAT]; reader fidelity ? | P0 H1 preserve selectable fallback |
| R06 | Faithfully render sender HTML/CSS, dimensions, layout tables, and images | partial; reader discards sender CSS/dimensions/images and flattens layout tables [N23] | Unknown exact current fidelity in available docs | Unknown exact fidelity | Unknown exact fidelity | P0 H1/U1 |
| R07 | Per-message plain-text reading mode | implemented toggle [N22], [A] | Unknown UI; API MIME/raw [TM] | Unknown | Unknown | P1 preserve U1 |
| R08 | Collapse trailing quoted content and restore it safely | implemented conservative trailing blockquote [N23], [N24] | Unknown | Unknown | Yes Quick Quote offered; quote hiding unknown [HFORMAT] | P1 preserve U1 |
| R09 | Open sanitized external HTTP(S) links without executable schemes | implemented URL filter/native links [N23], [N22]; runtime unverified | Unknown exact scheme controls | Yes browser selection tutorial [ST]; sanitization ? | Unknown exact scheme controls | P0 preserve U1/D1 |
| R10 | Load remote images only after explicit per-message/sender permission | partial; images removed, no consented-load control [N23], [A] | Yes remote-content option for protected OpenPGP [TR]; general policy ? | Yes remote images off control [SREAD]; per-message/sender consent detail ? | Unknown | P0 H1/U1 |
| R11 | View message headers and raw RFC822 source | missing native control; direct LoadSource exists, TB rejects [N8], [N14], [N22] | API getRaw/listHeaders [TM] | Unknown | Unknown | P1 U1 |
| R12 | Find text within the currently opened message | missing reader-find control; Ctrl+F targets mail search [N1], [N22] | Unknown detailed keyboard KB blocked [TBLOCK] | Yes Ctrl/Cmd+F searches current body [SS] | Unknown | P1 U1 |
| R13 | Print message or export a printable PDF | missing native; legacy print preview [N17] | Unknown detailed UI | Yes print tutorial [ST] | Yes desktop/iOS; thread-print single-message workaround [HPRINT] | P1 U1 |
| R14 | List/download/save message attachments explicitly | implemented bounded requests/file path UI [N2], [N22] | API list/get attachments [TM] | Yes attach tutorials; download detail ? [ST] | Yes open/download; previews vary by type [HATT] | P1 preserve F0 |
| R15 | Preview images/PDFs inline without external application | missing native preview/gallery; legacy gallery/lightbox [N17], [N22] | Unknown | Unknown exact preview | Yes PDF and PNG previews desktop/web [HATT] | P2 U1 |
| R16 | Attachment gallery across folders with filters | missing native; legacy attachments_gallery [N17] | Unknown | Unknown | Unknown | P2 S1/U1 |
| R17 | Safe filename, aggregate limits, binary-preserving attachment transfer | implemented bounds/staging/bridge; loopback evidence narrow [N12], [N14], [A] | API attachment File [TC], [TM]; guarantees ? | Unknown exact safety limits | Yes file attach [HATT]; exact integrity limits ? | P0 preserve F7/D1 |
| R18 | Detached message/composer windows and concurrent drafts | missing native single composer/view [N1] | API windows and compose windows [TX], [TC] | Yes desktop multi-window tutorial [ST] | Yes native account tabs [HU]; concurrent draft UI ? | P2 C1/U1 |
| R19 | Resolve MIME CID images inline without remote/provider image requests | missing native image rendering; reader strips images, draft images become attachments [N23], [N12] | Yes inline-image draft handling [TR]; exact reader resolver unknown | Yes signature images [SSIG]; incoming CID resolver fidelity unknown | Yes inline/attached image handling [HATT]; exact CID fidelity unknown | P0 H1/F7 |
| R20 | Responsive styled reader at narrow/wide widths and live pane resize | partial; semantic flow/data-table local scroll, sender styled layout not retained [N22], [N23], [P] | Unknown current responsive-reader limits | Unknown current responsive-reader limits | Unknown current responsive-reader limits | P0 H1/U1 |
| R21 | Fast selected-body display with measured cold/warm latency and stale-selection cancellation | unverified; fetch/cache/generation paths exist, no latency measurements [N1], [N2], [N3], [N10] | API asynchronous/paginated reads [TI], [TM]; selected-body latency unknown | Unknown measured selected-body latency | Unknown measured selected-body latency; marketing speed is not measurement [HMAIN] | P0 H2/F0/F2/F6 |

### 4. Triage, folder organization, and deterministic automation

| ID | Meaningful capability / user outcome | Native MegaMail status and evidence | Thunderbird | Spark | Superhuman Mail | Priority / dependencies |
|---|---|---|---|---|---|---|
| T01 | Mark a selected message read/unread on its owning account | implemented [N2], [N10] | API update read state [TM] | Yes email actions [SK] | Yes unread filtering [HSPLIT]; mutation UI ? | P1 preserve T1 |
| T02 | Star/unstar a selected message | implemented [N2] | API flagged property [TM] | Yes Pin, not necessarily provider star [SF] | Unknown exact provider-star parity | P1 preserve T1 |
| T03 | Archive selected mail and restore it to Inbox | implemented direct/TB request paths [N2], [N14] | API archive/move [TM] | Yes Done archives [SF] | Yes Done/folder moves [HF] | P1 preserve T1 |
| T04 | Trash selected message, recover from Trash, permanently purge | partial; trash/restore, no native purge; TB rejects purge [N2], [N14] | API delete/move [TM] | Yes Delete actions [SK]; purge UI ? | Yes unwanted-mail actions [HUNWANTED]; purge UI ? | P1 T1 |
| T05 | Select multiple messages, ranges, and all matching search results | missing native multi-selection [N1], [P] | API bulk message IDs [TM]; selection UI ? | Yes multi-email actions [ST] | Yes mass archive [HMASS]; selection scope ? | P1 T1 |
| T06 | Bulk archive/trash/move with item-scoped failures | missing native; direct core exists, TB bulk move rejected [N8], [N14] | API array move/archive [TM] | Yes multiple-email actions [ST] | Yes Mass Archive [HMASS] | P1 T1/F0 |
| T07 | Mark all messages in a folder read and bulk unstar | missing native; MarkAllRead direct core, TB rejects [N8], [N14] | API update messages [TM]; bulk UI ? | Yes mark all read [SK] | Unknown exact folder bulk semantics | P1 T1 |
| T08 | Move/copy selected mail to arbitrary destination folder | missing native picker; archive/trash only [N2], [N1] | API move/copy [TM] | Yes Move action [SK] | Yes Move to folder [HF] | P1 T1 |
| T09 | Create, rename, and delete provider folders/subfolders | missing native controls; direct core exists [N8], [N1] | API create/rename/delete [TF] | Yes Create Folder tutorial [ST]; rename/delete ? | Partial create/subfolder; deletion external provider [HF] | P1 T1 |
| T10 | Choose favorite folders and customize sidebar order/visibility | missing native persisted folder customization [N1], [N24] | Unknown | Yes Add folders sidebar [ST] | Partial folder navigation; displayed folder cap [HL] | P2 T1/U1 |
| T11 | Apply/remove multiple Gmail labels without moving mail | missing native label UI; core label methods not native [N1], [N8] | API tags; Gmail-label parity unknown [TM] | Yes Gmail Labels [ST]; cloud Bridge caveats [SB] | Yes Gmail labels [HL] | P1 T1/F4 |
| T12 | Apply/remove Outlook categories distinct from folders | missing native Graph onboarding/control [N8], [N5] | Unknown native EWS categories | Unknown category parity | Yes desktop/web; mobile planned [HL] | P1 T1/F4 |
| T13 | Color tags and filter by user-defined tags | missing native tags UI [N1], [N16] | API message tags [TM] | Yes Labels [SF]; colors/provider limits ? | Yes labels; new neutral gray [HL] | P2 T1/S1 |
| T14 | Mark spam/not-spam and learn junk classification | missing native; direct MarkSpam/Ham; TB rejects [N8], [N14] | API junk state [TM]; learning UI ? | Yes Spam actions/tutorial [SK], [ST] | Yes unwanted-email guidance [HUNWANTED]; learning ? | P1 T1 |
| T15 | Undo archive/trash/move/read actions safely after success | missing native mutation undo [N2], [N1] | Unknown | Yes Undo Last Action tutorial [ST] | Yes Undo; 10-second desktop window [HUNDO] | P1 T1/F5 |
| T16 | Snooze an incoming message until a chosen time | missing native timer/state [N1], [N15] | Unknown built-in; add-on ecosystem [TX] | Yes Snooze [SF]; plan ? | Yes Remind Me hides/returns message [HREM] | P2 R1/F6 |
| T17 | Mute a noisy thread while retaining searchable mail | missing native [N1], [N15] | Unknown | Yes Mute Threads [SF]; Bridge unavailable [SB] | Yes Gmail only; Outlook not supported [HMUTE] | P2 R1 |
| T18 | Pin or set aside a message independently from provider star | missing native separate workflow [N1], [N2] | Unknown | Yes Pin/Set Aside [SF]; plan ? | Yes Remind Me someday; exact pin equivalence unknown [HREM] | P2 R1 |
| T19 | VIP/priority senders and important-vs-newsletter Inbox splits | missing native [N1], [N15] | Unknown | Yes Smart Inbox/Priority [SF]; Bridge limitations [SB] | Yes default/custom Split Inbox and Auto Labels [HSPLIT], [HAL] | P2 R1 |
| T20 | Gatekeeper approval/blocking for first-time senders | missing native [N1], [N15] | Unknown | Yes Gatekeeper [SF]; Bridge unavailable [SB] | Unknown equivalent Gatekeeper behavior | P2 R1 |
| T21 | Group Inbox mail by sender or domain rather than thread | missing native; conversations only [N10] | Unknown | Yes group sender/domain [ST]; Bridge unavailable [SB] | Yes custom split sender/domain [HSPLIT], not identical grouping | P2 R1 |
| T22 | Rule editor with conditions AND/OR, preview, and ordered actions | missing native editor; FilterRule core/legacy [N16], [N17] | Unknown detailed filter KB blocked [TBLOCK] | Planned Autopilot [SP]; Smart Folders are not rules | Partial deterministic Auto Labels AND/OR; not full rule parity [HAL] | P2 R1 |
| T23 | Saved searches/smart folders automatically show matching mail | missing native saved-query UX [N1], [N13] | API query/recursive folders [TM]; saved folder UI ? | Yes desktop paid Smart Folders; Gmail vs others scope [SM] | Yes Custom Split Inbox; body-keyword criteria need Gmail workaround [HSPLIT] | P2 S1/R1 |
| T24 | One-click unsubscribe with safe List-Unsubscribe/mailto/web handling | missing native banner; portable detector/action exists [N20] | Yes list-management headers [TS]; one-click UI unknown | Unknown exact unsubscribe method | Yes unsubscribe advertised [HMAIN]; auth/safety detail ? | P1 T1/U1 |
| T25 | Set provider vacation/Out-of-Office reply with date range | missing native [N1], [N15] | Unknown | No native setup; provider workaround [SOUT] | Yes Gmail/Outlook desktop responder [HOOO] | P2 R1/F4 |

### 5. Compose, drafts, scheduling, and response productivity

| ID | Meaningful capability / user outcome | Native MegaMail status and evidence | Thunderbird | Spark | Superhuman Mail | Priority / dependencies |
|---|---|---|---|---|---|---|
| C01 | Compose a new plain-text message with subject and recipients | implemented [N12], [N11] | API beginNew/plainTextBody [TC] | Yes compose tutorial [ST] | Yes compose/format [HFORMAT] | P1 preserve F7 |
| C02 | Send To/Cc/Bcc, preserve Bcc in drafts but not recipient message headers | partial; fields/core bytes, no live-provider validation [N11], [N12], [N8] | API To/Cc/Bcc [TC] | Yes compose and auto-Cc/Bcc [ST] | Yes To/Cc/Bcc attachments instructions [HATT] | P0 preserve F5/F7 |
| C03 | Send to Bcc-only or Cc-only recipients with empty To | missing native; To-only gate rejects [N11] | API recipient fields [TC]; empty-To acceptance unknown | Unknown | Unknown | P1 F7/F0 |
| C04 | Reply honors Reply-To and retains threading References/In-Reply-To | implemented [N12], [N8] | API beginReply/headers [TC] | Yes reply tutorial [ST]; exact header behavior ? | Yes reply [HMAIN]; exact headers ? | P1 preserve F7 |
| C05 | Reply All deduplicates recipients and excludes own aliases | implemented [N12]; live unverified | API replyToAll [TC]; dedupe algorithm ? | Yes reply-all action [SK]; self-exclusion ? | Yes reply-all [HATT]; self-exclusion ? | P1 preserve F7 |
| C06 | Forward selected message text | implemented [N12] | API inline/attachment forwarding [TC] | Yes Forward action [SK] | Yes Forward [HATT] | P1 preserve F7 |
| C07 | Forward original attachments automatically or include them on reply | missing native original-file carryover [N12]; manual attach available | API forward modes [TC] | Unknown exact auto-forward attachment behavior | Yes automatic forward files; optional reply originals [HATT] | P1 F7/C1 |
| C08 | Forward original email as an RFC822 attachment | missing native [N12] | API forwardAsAttachment [TC] | Unknown | Unknown | P2 C1 |
| C09 | Inline reply/quick quote within current conversation | missing native separate Compose view [N1], [P] | API compose windows; inline UI unknown [TC] | Yes Quick Replies iOS [ST], not same as inline rich reply | Yes Quick Quote/Instant Reply [HFORMAT], [HIR] | P1 C1 |
| C10 | Select imported/configured sender alias or identity | implemented identity cycling; no native alias creation [N1], [N12] | API identity selection [TC], [TA] | Yes aliases; Gmail auto-import, provider limits [SALI] | Partial Gmail alias selection; Outlook primary alias only [HAlIAS] | P1 preserve C1 |
| C11 | Manage per-account aliases and separate alias SMTP credentials | partial; config supports aliases; native management missing [N5], [N1] | API identities [TA]; separate SMTP UI ? | Yes provider-configured aliases [SALI] | Partial Gmail provider-side setup; Outlook cannot switch aliases [HAlIAS] | P1 C1/F4 |
| C12 | Automatic To/Cc/Bcc defaults and recipient-group completion | missing native controls; legacy contacts suggestions [N17], [N11] | API compose fields/mailing lists [TC], [TL] | Yes auto Cc/Bcc and group email Mac/iOS tutorials [ST] | Yes Auto Bcc [HABCC]; group completion ? | P2 C1/B1 |
| C13 | Per-account text/HTML signatures and multiple signature selection | missing native signature editor/insertion [N12], [N21] | Unknown signature UI; compose HTML API [TC] | Yes multiple defaults and HTML [SSIG] | Partial Gmail signature inherited, Outlook pasted [HSIG] | P1 C1 |
| C14 | Rich HTML compose with bold/lists/link formatting and plain alternative | missing native; GTK rich editor not wired [N12], [N17] | API HTML/plain/both formats [TC] | Yes rich text tutorial [ST] | Yes formatting desktop/mobile [HFORMAT] | P1 C1/F7 |
| C15 | Inline CID images and preserving their position during draft editing | partial; incoming draft images become attachments [N12] | Yes inline draft image fix [TR]; API compose files [TC] | Yes signature images [SSIG]; CID-draft preservation ? | Yes inline attachment handling [HATT]; CID preservation ? | P1 C1/F7 |
| C16 | Attach/remove local files with picker and size checks | implemented native picker/staging [N11], [N12] | API attachments [TC] | Yes Attach File tutorial [ST] | Yes picker/drop; ~25MB provider limits [HATT] | P1 preserve F7 |
| C17 | Drag/drop and paste files/images into compose | missing native compose transfer gestures [N11] | Unknown UI; API attachments [TC] | Unknown exact gestures in retrieved docs | Yes drag/drop desktop/web [HATT] | P2 C1/U1 |
| C18 | Large attachment cloud upload/link with access controls | missing native; legacy cloud modules [N17] | Yes FileLink feature policy [TR]; provider add-ons [TX] | Yes large-attachment tutorial [ST]; plan/storage ? | No cloud-based attachments supported [HATT] | P2 C1 |
| C19 | Save an incomplete draft and reopen it with Bcc/files/thread headers | implemented manual draft save/export/staging; HTML simplified [N11], [N12] | API draft save/edit [TC] | Yes compose/drafts [ST]; exact fidelity ? | Yes offline drafts and shared drafts [HO], [HSD]; fidelity ? | P1 preserve F7 |
| C20 | Autosave draft locally and to provider without duplicate draft copies | missing native autosave; worker autosave field latent [N11], [N8] | API save draft [TC]; autosave policy unknown | Unknown exact autosave policy | Yes drafts [HSD]; autosave protocol details ? | P0 F7 |
| C21 | Reusable templates/snippets with variables and missing-field warnings | missing native; direct template core/TB rejects [N8], [N14] | API save template [TC]; variables unknown | Yes custom/shared Templates [SP], [SF]; Plus+ | Yes Snippets variables/placeholders; create desktop [HSNIP] | P2 C1 |
| C22 | Send Later at a scheduled date/time | missing native scheduling control; direct send_at latent [N12], [N8] | API queue sendLater, not scheduled timestamp [TC] | Yes Scheduled Sends [SP]; hosted store; Bridge unavailable [SB] | Yes Send Later [HMAIN], [HO]; online before offline needed | P2 C1/F5 |
| C23 | Send Later still dispatches while app/device is closed or offline | unverified; no native scheduler contract [N8], [N12] | Unknown; queue API not an always-on timer [TC] | Yes server-supported Send Later [SO]; Bridge unavailable [SB] | Yes if scheduled before going offline [HO] | P2 C1/F5 |
| C24 | Undo Send delay with cancellation before SMTP submission | missing native delay/undo control [N11] | Unknown | Yes customizable; default 5 seconds [SU] | Yes fixed 10-second desktop undo [HUNDO] | P1 C1/F5 |
| C25 | Follow-up reminders only when no reply arrives | missing native reminder state [N1], [N12] | Unknown | Yes Reminders [SF]; plan ? | Yes Remind Me if-no-reply [HREM] | P2 R1/C1 |
| C26 | Read/open statuses for sent email and recipients | missing native tracking; should be opt-in [N12] | API MDN requests, not tracking pixels [TC] | Yes Spark Pro Read Statuses [SREAD] | Yes Read Statuses; Recent Opens Business [HREAD], [HP] | P3 C1/A1 |
| C27 | Request standards-based delivery and read receipts | missing native DSN/MDN toggles [N12], [N11] | API DSN/returnReceipt; server/recipient may ignore [TC] | Unknown standards-based receipt support | Unknown standards-based receipt support | P2 C1/F4 |
| C28 | Spellcheck/autocorrect dictionaries and correction controls | missing native; GTK spell implementation exists [N17] | Unknown current UI | Unknown local spell dictionary behavior | Yes desktop autocorrect; listed languages only [HAUTO] | P1 C1/U1 |
| C29 | Validation prevents invalid recipients/header injection/unsafe file mutation | partial; parser/preflight/core MIME validation; audit not security proof [N8], [N14], [N11] | API compose validation; exact threat guarantees ? [TC] | Unknown | Unknown | P0 F5/F7/D1 |

### 6. Search, retention, archive, and navigation

| ID | Meaningful capability / user outcome | Native MegaMail status and evidence | Thunderbird | Spark | Superhuman Mail | Priority / dependencies |
|---|---|---|---|---|---|---|
| S01 | Search sender/subject/preview in loaded current-folder headers | implemented; loaded-only, not server search [N10], [A] | API query subject/author [TM] | Yes Smart Search [SS] | Yes Search [HSEARCH] | P1 preserve S1 |
| S02 | Search all provider history beyond loaded/paged headers | missing native; 5000 loaded cap [N10], [N13] | API query/pagination [TM], [TI]; index completeness ? | Yes Smart Search with provider folder limits [SS] | Yes inbox/folders search; history limits ? [HSEARCH] | P1 S1/F6 |
| S03 | Search message body/full text across folders | missing native [N10], [P] | API fullText/body/recursive folders [TM] | Yes content search; Gmail all except Spam/Trash, others Inbox/Archive default [SS] | Yes query words in inbox/folders [HSEARCH] | P1 S1 |
| S04 | Search across accounts with optional account scope | partial; loaded Unified Inbox only [N10] | API query accountId array [TM] | Yes all accounts default/choose account [SS] | Unknown cross-account search; no unified Inbox [HU] | P1 S1 |
| S05 | Advanced query operators dates/from/to/subject/exclusion/AND/OR | missing native substring query [N10] | API structured query conditions [TM], textual operator UI unknown | Yes natural language/from/to/date/attachment [SS] | Yes AND/OR/exclusion/exact phrase [HSEARCH] | P1 S1 |
| S06 | Filter unread/starred/with-attachment messages | implemented loaded-header filter [N10] | API read/flagged/attachment query [TM] | Yes Smart Search [SS] | Yes unread filtering [HSPLIT]; other operators [HSEARCH] | P1 preserve S1 |
| S07 | Search attachment filename/type/content rather than just has-attachment | missing native full search [N10] | API attachment-related query; attachment content unknown [TM] | Yes attachment search [SS]; contents ? | Partial files search [HSEARCH]; AI attachment reads Business [HASK] | P2 S1 |
| S08 | Save/reuse recent queries and smart views across restart | missing native persisted search preferences [N24], [N13] | Unknown saved-query UI | Yes saved search mobile [SS], desktop Smart Folders paid [SM] | Yes saved Split from search [HSPLIT]; history UI ? | P2 S1/R1 |
| S09 | Clearly label incomplete/offline search scope and loading errors | partial; loaded-mail text and warnings [N1], [N10] | Unknown | Yes documented provider scopes [SS]; runtime warning UX ? | Yes offline cache scope documented [HO]; runtime warnings ? | P1 S1/F0 |
| S10 | Import/export RFC822/mbox archives and local-only folders | missing native; source TB mail/local folders deliberately not cloned [A], [N4] | API local account/import messages [TA], [TM]; archive format UI ? | No local email backup/export as separate files [SO] | Unknown | P2 F6/S1 |
| S11 | Choose per-account offline body/attachment retention and cache cleanup | missing native retention controls [N21], [N24] | Unknown detailed offline KB blocked [TBLOCK] | Partial fixed past-month cache and cache-clean tutorial [SO], [ST] | Partial documented recent cache limits [HO]; custom retention ? | P1 F6/S1 |
| S12 | Sort lists by sender/subject/date/read/size and reverse order | partial; native date ordering, no sort chooser [N10], [N1] | API list sort properties [TM] | Unknown list sort customization | Unknown list sort customization | P2 S1/T1 |
| S13 | Jump to folder/message/action through command palette | missing native palette; fixed shortcuts only [N1], [N21] | Unknown UI palette; commands API [TX] | Yes Command Center [SF] | Yes Command and shortcuts [HK] | P1 U1 |
| S14 | Full keyboard triage and customizable shortcut presets | partial; new/reply/reply-all/search/unified/refresh/arrows [N1] | Unknown detailed shortcut KB blocked [TBLOCK] | Yes presets/custom shortcuts desktop [SK] | Partial extensive shortcuts, no customization [HK] | P1 U1/T1 |

### 7. Contacts, calendars, invitations, and tasks

| ID | Meaningful capability / user outcome | Native MegaMail status and evidence | Thunderbird | Spark | Superhuman Mail | Priority / dependencies |
|---|---|---|---|---|---|---|
| B01 | Recipient autocomplete from contacts and recent correspondents | missing native; legacy contacts suggestions not wired [N17], [N11] | API contacts/address books [TAB]; autocomplete UX unknown | Yes event/contact suggestions [SCE]; email-completion scope ? | Yes Contact Pane [HCONTACT]; recipient-completion scope ? | P1 B1/C1 |
| B02 | Local contacts CRUD with names/emails/photos/notes | missing native contacts screen [N17], [N15] | API contacts/address-books CRUD [TAB], [TCONTACT] | Unknown full contacts manager | No editing others' profiles in Contact Pane [HCONTACT]; full contacts CRUD unknown | P2 B1 |
| B03 | Contact groups/mailing lists with recipient expansion | missing native [N17] | API mailing lists/members [TL] | Yes group-email Mac/iOS tutorial [ST]; other platforms ? | Unknown | P2 B1/C1 |
| B04 | CardDAV synchronized address books | missing native; legacy directory/contact integration [N17] | Yes desktop CardDAV; Android N/A [TS], [TR] | Unknown general CardDAV | Unknown | P2 B1/F4 |
| B05 | Import/export vCard and LDIF contacts | missing native [N17], [N15] | Yes desktop vCard/LDIF import-export [TS] | Unknown | Unknown | P2 B1 |
| B06 | LDAP directory lookup for enterprise recipients | missing native [N17], [N15] | Yes desktop LDAP [TS] | Unknown | Unknown | P3 B1/F4 |
| B07 | Contact context with recent correspondence and social profile details | missing native contextual contact pane [N22], [N17] | Unknown social enrichment | Yes Avatars tutorial [ST]; broader enrichment ? | Yes Contact Pane from external providers; others not editable [HCONTACT] | P3 B1/S1 |
| B08 | ICS invitation preview, meeting details, and timezone-safe RSVP | missing native; portable detect/reply_ics exists [N18] | Yes desktop iCalendar/iTIP; Android not supported [TS], [TR] | Yes calendar event invitations [SCE]; inbound RSVP specifics ? | Yes invite/calendar view [HCAL]; RSVP specifics ? | P1 B1/F7 |
| B09 | Unified calendar day/week/month views across selected accounts | missing native [N15], [N18] | Yes desktop calendar [TD]; view specifics unknown | Yes calendar selection/views [SCV]; supported providers only [SC] | Partial desktop day/week; iOS month/multi-day; Android reduced [HCAL] | P2 B1 |
| B10 | Create/edit/delete events with attendees and locations | missing native [N18], [N15] | Yes calendar/iCalendar [TD], [TS]; full UI CRUD unknown | Yes create/edit details and edit/delete tutorial [SCE], [ST] | Yes create/manual/AI events [HEVENT]; edit/delete specifics ? | P2 B1 |
| B11 | CalDAV and subscribed ICS calendars independent of mail provider | missing native [N18], [N15] | Yes CalDAV/iCalendar desktop [TS] | Partial Google/Exchange/M365/iCloud/Outlook; custom IMAP/Yahoo calendar unavailable [SC] | Partial Google/Microsoft calendars; generic CalDAV unknown [HCAL] | P2 B1/F4 |
| B12 | Recurring events with exceptions and timezone/DST behavior | missing native [N18], [N15] | Yes recurrence fixes; full exception semantics unverified [TR] | Yes recurring-event setup [SCE] | Unknown recurrence semantics | P2 B1/D1 |
| B13 | Event alarms/notifications and configurable multiple reminders | missing native [N18], [N17] | Yes calendar/tasks [TD]; alarms specifics unknown | Yes up to five alerts/event [SCE] | Yes calendar event notifications [HCAL] | P2 B1/U1 |
| B14 | Share availability and find team free/busy across timezones | missing native [N18], [N15] | Unknown CalDAV Scheduling status blank [TS] | Yes calendar-scheduling tutorial [ST]; exact free/busy scope ? | Yes Share Availability/Find Time; team/timezone desktop [HAVAIL], [HTIME] | P2 B1 |
| B15 | Conference links (Meet/Zoom/Teams) in created events | missing native [N18], [N15] | Unknown add-on integrations [TX] | Yes Zoom/Meet; Meet Google Calendar only; Teams pricing [SCE], [SP] | Yes conference links in manual create [HEVENT] | P2 B1 |
| B16 | Calendar tasks/to-dos with completion and CalDAV sync | missing native [N18], [N15] | Yes desktop tasks; CalDAV task fixes [TD], [TR] | Unknown native tasks; productivity integrations exist [SP] | Unknown native tasks | P2 B1 |

### 8. AI-assisted mail and team collaboration

| ID | Meaningful capability / user outcome | Native MegaMail status and evidence | Thunderbird | Spark | Superhuman Mail | Priority / dependencies |
|---|---|---|---|---|---|---|
| A01 | Optional AI draft from prompt with human review before send | missing native AI integration [N15], [P] | Unknown built-in; extension ecosystem [TX] | Yes Spark +AI; paid quotas, free preview [SP], [SAI] | Yes Write with AI; most AI Starter [HWRITE], [HP] | P3 A1/C1 |
| A02 | Rewrite tone/length/grammar and translate draft text | missing native; legacy translation not native AI [N17] | Unknown | Yes AI writing/translation [SP], [SAI] | Yes Write with AI editing and translation [HWRITE] | P3 A1 |
| A03 | Summarize a message or whole conversation with source traceability | missing native [N15], [N22] | Unknown | Yes AI summaries [SP], [SAI]; source-citation detail ? | Yes one-line/expanded Auto Summarize; length/type limits [HSUM] | P3 A1/S1 |
| A04 | Suggested quick replies in user's voice using thread context | missing native [N12], [N15] | Unknown | Yes AI quick replies [SP]; paid quotas | Yes Instant Reply [HIR]; enabled AI, Auto Draft responses can replace it [HAD] | P3 A1/C1 |
| A05 | Semantic question-answer/search across mailbox history | missing native search/AI [N10], [N15] | Unknown | Yes AI Assistant; Free 1 month/Plus 1 year/Pro full-history pricing [SP] | Yes Ask AI Business+; up to five years excluding Trash/Spam [HASK] | P3 A1/S1 |
| A06 | AI attachment analysis with permission and visible evidence | missing native [N15] | Unknown | Yes Assistant across attachments [SP]; privacy detail needs confirmation | Yes Ask AI reads named thread attachments; retention text conflicts [HASK] | P3 A1/S1 |
| A07 | Automatic AI labels/classification and auto-archive controls | missing native [N15], [N16] | Unknown | Planned Autopilot rules [SP]; Smart Inbox is shipped [SF] | Yes Auto Labels/Auto Archive; custom AI prompts Business+ [HAL], [HAARCH] | P3 A1/R1 |
| A08 | Automatically draft responses/follow-ups without sending | missing native [N15] | Unknown | Planned in pricing; KB Auto-Drafts listed; rollout unresolved [SP], [ST] | Yes Auto Drafts Business/Enterprise [HAD] | P3 A1/C1 |
| A09 | AI follow-up detection and reminder generation | missing native [N15] | Unknown | Unknown AI reminder inference; manual reminders [SF] | Yes Auto Reminders detects needed follow-ups [HAD] | P3 A1/R1 |
| A10 | Recommend recipient-specific send times | missing native [N12] | Unknown | Unknown | Yes Smart Send Business/Enterprise desktop; data availability dependent [HSMART] | P3 A1/C1 |
| A11 | AI meeting transcription/notes associated with calendar/email | missing native [N18], [N15] | Unknown | Yes quota-limited notes; Free lifetime 5, Plus 40/month, Pro unlimited count [SP] | Unknown Mail-native meeting transcription; suite products are not Mail parity | P3 A1/B1 |
| A12 | Agent/CLI/MCP access with explicit read vs mail-mutation permissions | missing native external agent interface [N15] | Extension/native messaging API, not an MCP promise [TX] | Yes Spark CLI read vs Pro triage access [SP], [SCLI] | Yes Mail MCP Business+; Ask AI/auth/admin requirements [HMCP] | P3 A1/F0 |
| A13 | Share live email threads and revoke access without forwarding | missing native collaboration store [N15] | Unknown | Yes Shared Threads/links [SF], [SP]; plan/limits | Yes live Shared Conversations incl. guests [HC], [HP] | P3 A1 |
| A14 | Private team comments/@mentions separate from outgoing mail | missing native [N15] | Unknown | Yes Private Comments [SP], [SF]; plan limits | Yes Team Comments with guest collaboration [HC] | P3 A1 |
| A15 | Multiple people edit a shared draft in real time | missing native [N12], [N15] | Unknown | Yes co-edit team members; tutorial vs pricing limits differ [SSD], [SP] | No; Shared Draft author alone edits, others watch/comment [HSD] | P3 A1/C1 |
| A16 | Delegate email ownership/deadlines and track completion | missing native [N15] | Unknown | Yes Assignments; Free 10/team, paid unlimited [SP], [SF] | Unknown equivalent assignment/deadline workflow | P3 A1/R1 |
| A17 | Shared/team Inbox without distributing provider credentials | missing native [N15] | Unknown | Yes Shared Inboxes Pro+ [SSI], [SP] | Unknown; team-shared conversations not shared-mailbox ownership [HC] | P3 A1/F4 |
| A18 | Reply-collision indicators and team read-status sharing | missing native [N15] | Unknown | Unknown collision indicators; Pro Read Statuses [SREAD] | Yes Team Reply Indicators/Team Read Statuses [HTEAMREAD] | P3 A1 |
| A19 | Shared template libraries and consistent responses | missing native; legacy templates not team sync [N8], [N15] | Unknown | Yes Shared Templates [SF], [SP]; paid team | Yes shared Snippets; desktop creation, mobile use [HSNIP] | P3 A1/C1 |
| A20 | CRM integration/email logging with configurable exclusions | missing native [N15] | Unknown | Yes HubSpot Pro; Salesforce/Pipedrive pricing says coming soon [SP] | Yes HubSpot/Salesforce Business+; Auto Bcc desktop/iOS [HP], [HABCC] | P3 A1 |
| A21 | Team admin roles, access revocation, SSO/SAML, SCIM provisioning | missing native; local accounts not team identity [N15] | Unknown; enterprise policy != team cloud admin [TR] | Partial Enterprise controls; SAML/SCIM exact support unknown [SP] | Yes Enterprise SSO/SAML/SCIM; SCIM no group/role management [HS], [HSCIM] | P3 A1 |

### 9. Security, privacy, accessibility, and platform integration

| ID | Meaningful capability / user outcome | Native MegaMail status and evidence | Thunderbird | Spark | Superhuman Mail | Priority / dependencies |
|---|---|---|---|---|---|---|
| U01 | Store direct credentials outside account TOML in private namespace | implemented config/keyring transactional paths [N5], [A]; runtime unverified | Unknown exact store in retrieved docs | Yes provider tokens/passwords encrypted in server storage [SV] | Yes provider authorization [HA]; secret-store details ? | P0 preserve F3/F4 |
| U02 | Sanitize scripts/styles/forms/remote resources from message HTML | implemented native bounded sanitizer [N23] | Unknown exact sanitizer | Unknown exact sanitizer | Unknown exact sanitizer | P0 preserve U1/D1 |
| U03 | Block tracking pixels and expose remote-image privacy controls | partial; native strips images, no allow controls [N23] | Unknown general controls; encrypted remote-content change [TR] | Yes remote image off; tracking protection incomplete [SREAD] | Unknown inbound blocking; outgoing tracking opt-in [HREAD] | P1 U1 |
| U04 | Display trusted sender authentication/DKIM/SPF/DMARC evidence | missing native evidence panel; shared check_sender not UI [N20], [N10] | Unknown | Unknown | Unknown | P1 U1/F4 |
| U05 | OpenPGP decrypt/verify incoming messages and avoid plaintext disk cache | partial direct worker path/cache exclusion, native status/key UX missing [N19] | Yes desktop/Android OpenPGP [TS]; key fixes [TR] | Unknown | Unknown | P0/P1 F6/U1 |
| U06 | Manage PGP keys, sign/encrypt outgoing mail and warn missing keys | missing native controls; shared GPG module/GTK UI [N19], [N12] | API OpenPGP compose; desktop key management implicated by fixes [TC], [TR] | Unknown | Unknown | P2 C1/U1 |
| U07 | S/MIME sign/encrypt with certificates | missing native shared/direct controls found [N12], [N15] | Yes desktop; Android No; compose API [TS], [TC] | Unknown | Unknown | P2 C1/F4 |
| U08 | Local cache encryption at rest with lock/recovery policy | unverified; namespace/privacy != encrypted cache [N9], [A] | Unknown; mail encryption is not cache encryption | Yes cloud databases encrypted [SV]; local cache encryption unknown | Yes AI email store encrypted [HAI]; local cache encryption unknown | P0 policy F6 |
| U09 | Private file permissions and bounded untrusted profile/MIME parsing | implemented checks and isolated clone; audit not penetration test [N4], [N5], [N12], [N23] | Unknown exact bounds | Unknown | Unknown | P0 preserve F1/F2/D1 |
| U10 | AI consent/data minimization/no-training/retention/deletion controls | missing native AI policy; no AI runtime found [N15] | Yes privacy policy; AI feature-specific controls unknown [TP] | Yes +AI default off; no training; vendor <=30d policy [SP], [SAI] | Partial activate/deactivate; no per-feature opt-out on Business [HASK], [HAI] | P0 before A1 |
| U11 | Enterprise customer-managed keys for hosted AI memory | missing; not applicable to local-only baseline [N15] | Unknown | Unknown | Yes Enterprise BYOK only AI memory; Google KMS; not all stores [HBYOK] | P3 A1 |
| U12 | App lock/biometric protection on unattended devices | missing native app lock control [N21] | Unknown | Yes mobile passkey/biometric; not documented desktop [SLOCK] | Unknown | P2 U1 |
| U13 | Accessible names/roles/states, screen-reader reading, keyboard focus order | unverified; some explicit labels and native selectable reader [N1], [N22] | Unknown current runtime compliance | Unknown current runtime compliance | Unknown current runtime compliance | P1 U1/D1 |
| U14 | Reduced motion, density, contrast, and configurable reading text scale | partial; motion/density/themes; no native text-scale preference [N21], [N24] | Unknown detailed accessibility UI | Yes reading font/dark tutorials [ST]; motion/contrast compliance ? | Partial desktop/web zoom [HFORMAT]; reduced motion ? | P1 U1 |
| U15 | RTL/bidirectional and international recipient/message text correctness | unverified runtime; sanitizer dir preservation/examples [N23], [N10] | Unknown runtime coverage | Unknown runtime coverage | Unknown runtime coverage; international shortcut help [HK] | P1 U1/D1 |
| U16 | Translate UI language and provide localized mail dates/shortcuts | partial shared i18n/date modules; native strings mostly literals, no language UI [N15], [N24] | Unknown detailed locale controls | Yes language-change tutorials incl. Android [ST] | Partial international shortcut guidance [HK]; UI locale coverage ? | P2 U1 |
| U17 | Light/dark/system themes, accents, imported themes and removable library | implemented native themes/import/reset [N21], [P] | Yes themes API/add-ons [TX]; system/theme UI specifics ? | Yes dark mode [ST] | Yes Theme article [HTHEME]; import/custom library ? | P2 preserve U1 |
| U18 | Desktop notifications with account/VIP/sound/privacy controls | missing native notification adapter; GTK notify not wired [N17], [N15] | API notifications [TX]; end-user controls unknown | Yes smart/custom notifications [SF], [ST]; Bridge unavailable [SB] | Yes priority notification behavior desktop/web/mobile [HN] | P1 U1 |
| U19 | System tray/background mail and badge policy after window close | missing native tray; lifecycle behavior unverified [N17], [N1] | Unknown | Yes badge tutorials [ST]; close/background semantics ? | Yes OS notification badge controls [HN]; close semantics ? | P1 U1/F2 |
| U20 | Default-mail application, mailto links, external compose/attachment entry | missing native command parsing/handler found; desktop helper not mailto [N26], [N1] | Unknown current default-app UI | Yes default-client/deep-link tutorials [ST] | Unknown default-handler behavior | P1 U1 |
| U21 | Native Linux desktop availability and packaging/compositor validation | partial Linux native package; real compositor validation incomplete [P], [A] | Yes Linux desktop [TD] | No documented Linux platform [SF] | No native Linux documented; browser platform separate [HD] | P0/P1 U1/D1 |
| U22 | Native Windows/macOS desktop support | missing verified ports; future product scope [P] | Yes Windows/macOS [TD] | Yes Windows/macOS [SF] | Yes native Mac/Windows [HU], [HD] | P3 platform product |
| U23 | Mobile iOS/Android and tablet email apps | missing product targets [P] | Yes Android [TD], [TS]; iOS released availability unknown here | Yes iPhone/iPad/Android; watch listed [SF] | Yes iOS/iPadOS/Android; feature gaps per article [HD] | P3 platform product |
| U24 | Cross-device account/preferences/productivity-state sync | missing native cross-device layer [N24], [P] | Unknown account-settings sync promise | Yes sync settings/accounts [SA]; Bridge local-only [SB] | Partial app state; accounts must be added separately on mobile [HA] | P3 platform/A1 |


## Native versus legacy: prevent false parity claims

| Existing source | Native reality | Reuse decision |
|---|---|---|
| `crates/mail-core/src/lib.rs:13-58` shares worker/cache/models/reader/PGP/invite/unsubscribe and includes GOA/MS broker | Exported module is not a native screen. Native imports and call sites matter. [N15] | Preserve portable behavior; add adapter capabilities rather than expose GTK internals. |
| `src/worker.rs:1160-1180`, `src/worker/{graph,jmap}.rs` | Protocol dispatch exists; native setup creates direct IMAP configurations. Graph/JMAP are latent, not certified native onboarding. [N8], [N7] | Reuse after provider-specific auth and feature contract tests. |
| `src/worker.rs:379-509,2345-2790`; `config::FilterRule:1586` | Bulk moves, folder management, rules exist in direct core, but no native bulk/folder/rule UI found. Thunderbird adapter explicitly rejects many requests. [N14], [N16] | Port native controls only after cross-adapter capability reporting. |
| `src/ui/rich_editor.rs`, `src/ui/compose.rs`, `src/spell.rs:27` | GTK rich editor/spell underlines are not GPUI components. Native uses plain-text Inputs. [N12], [N17] | Extract portable document/validation logic, not GTK widget trees. |
| `src/contacts.rs:54,141`, `src/ui/contacts_{browser,page}.rs` | Contacts/suggestions are legacy modules, not exported native contacts UX. [N17], [N15] | Separate contacts data module from GTK/EDS presentation. |
| `src/invite.rs:47,95,756`; worker calendar MIME `5030-5103` | Invite parsing/reply construction exists; no native RSVP/calendar workflow found. [N18] | Preserve RFC/timezone behavior, then add native invitation actions. |
| `src/pgp.rs:198,607,724,865`; `src/worker.rs:4038-4083,5132` | Direct shared worker can decrypt/check PGP and exclude encrypted bodies from cache; native key management/sign/encrypt controls absent. Thunderbird parity not established. [N19] | Keep cache exclusions; explicitly expose security status and never infer secure-send from module linkage. |
| `src/unsubscribe.rs:48,817`; `src/verify.rs:98` | Detection/authentication modules exported; native reader does not expose equivalent unsubscribe/security panels. [N20] | User-consented action, sender evidence provenance, safe URL validation. |
| `src/notify.rs:83`, `src/tray.rs`, `src/ui/print_preview.rs`, `src/translate.rs` | Not in native root module wiring; GTK notifications/tray/print/translation do not establish native availability. [N17] | Narrow desktop/print/language adapters with Linux permission handling. |
| Native `preferences.rs`, `appearance_view.rs` | Native mail preferences/themes/wallpapers really are wired. Accounts section only opens inbox/add-import; it is not edit/remove/re-auth management. [N21] | Preserve visual work; prioritize foundational account control over more wallpaper features. |

## Backend direction and the real seam

Two adapters already vary at a real seam: direct Hylki worker and Thunderbird actor. Keep both. The current seam includes `MailRequest`, request/page senders, `WorkerMailboxEvent`, and the UI's account/folder/message keys [N8], [N10], [N3]. Its implicit interface is large: adapter-specific IDs, supported requests, event ordering, queue failures, body limits, authentication, and save/send outcomes leak into callers.

Prefer a **deep mail-session module**: one small interface for configured-account lifecycle, scoped page/body/attachment reads, and request-correlated mutations; capability and typed error information cover differences. Each adapter keeps its implementation. Do not invent a giant pass-through trait for every speculative competitor feature. Add an internal seam for clock/filesystem/transport fixtures and test across the same external interface callers use.

Required invariants: persistent account identity ≠ session-local Thunderbird message ID; direct message identity includes mailbox epoch; operation outcome distinguishes rejected/queued/accepted/uncertain; stale responses are discarded; cancellation does not discard durable work; no credential or mail I/O on render/input paths; decrypted content exclusions apply to **every** cache; import intent survives authentication failure.

**Faster direct backend is a hypothesis.** First measure startup time, provider login time, first useful Inbox, p50/p95 body open, large-folder paging, idle CPU, resident memory per account, reconnect delay, throughput, and energy under identical fictional workloads. Separate Thunderbird process startup and bridge serialization from provider latency. Its API explicitly warns IDs are session-local [TM]; its paginated-list guidance is useful inspiration [TI]. Do not transfer Thunderbird OAuth credentials into the direct adapter.

## Prioritized backlog and narrow work packages

The ordering is by risk, the screenshot-driven HTML/latency priority, and dependencies, not competitor checkbox count. Parent owns integration and serial Cargo. These are **backlog contracts, not instructions to restart current work**: startup/restore, OAuth callback, discovery authentication alternatives, and offline-send/outbox already have parent-designated owners. Map F1/F3/F4/F5 recommendations onto those owners rather than spawning duplicates. Dedicated renderer/latency research is independent; this inventory does not await it. Do **not** simultaneously assign `main.rs`, `live.rs`, `config.rs`, or `worker.rs` to multiple implementing agents. “Disjoint” means file ownership, not merely different feature names in the same monolith.

| Package | Priority / owns | Deliverable / acceptance contract | Depends / exclusions |
|---|---|---|---|
| **F0 Lifecycle contract + integration owner** | P0; parent/integrator exclusively owns native `main.rs`, `live.rs`, core `lib.rs`, manifests, shared event/request declarations | Model configured/connecting/ready/offline/needs-login/unavailable/repairable separately; request/account/epoch correlation; capability reporting; apply other packages' integration deltas serially. Existing adapter preserved. | First. No broad monolith rewrite or replacement of Thunderbird. |
| **F1 Import persistence + restore** | P0; `thunderbird_adapter.rs` persistence/restore portions, preferably extracted **new** `account_registry.rs` with owner-approved integration | Persist configured intent before login; source-error vs absence; successful stable IDs retained; mixed failures survive restart; source relocation/clone policy; explicit forget/remove; unreadable data not overwritten. | F0 contracts. Adapter file changes serialized with F2; do not run F1/F2 editing it together. |
| **F2 Thunderbird runtime lifecycle** | P0; `crates/mail-core/src/thunderbird_bridge.rs`, `tools/thunderbird-bridge/` only; adapter integration by F0 | Binary compatibility gate, bounded startup/deadline/shutdown, restart/late response handling, safe clone refresh, private permissions, needs-login repair contract. Test 153 minimum and current 157 deliberately. | F0; F1 interfaces. Does not touch source profiles or import source mail/Outbox; no unsolicited unlock/auth. |
| **F3 Google browser authorization** | P0; new provider/auth transaction module and fictional auth fixtures; registration plan; existing OAuth edits by a single auth owner | App-owned registration, system browser, PKCE/state/random loopback, consent cancel/timeout, token storage/refresh/revocation, Gmail policy errors. Result produces validated account, never fake success. | F0 and release-owner registration approval. No copied Thunderbird client identity; live credential operations only with explicit authorization. |
| **F4 Other-provider authorization** | P0/P1; distinct new Microsoft/provider modules and fixtures, no shared OAuth edits in parallel with F3 | Microsoft personal/tenant consent + Graph; IMAP OAuth where appropriate; app-password path for iCloud; custom-provider policy errors; later JMAP discovery. Declare tenant/provider restrictions. | Shared auth contract from F3; integration F0. EWS/JMAP not interchangeable with IMAP. |
| **F5 Direct delivery + outbox integrity** | P0; single shared-worker/outbox owner; `src/worker.rs` send sections and cache outbox only after F6 cache serialization | Durable pre-send intent; accepted/refused/uncertain distinction; no automatic retries of ambiguous acceptance; Sent-copy failure separate; disk-full/crash fixtures; TB queue control contract delivered for F2/F0 integration. | F0; cache ownership lock with F6. Preserve GTK callers. |
| **F6 Cache/identity correctness** | P0; `src/cache.rs`, cache metadata model, query module; worker epoch integration exclusively by F0/F5 owner | UIDVALIDITY invalidation, namespace isolation, schema migration failure behavior, generation/epoch stale response rejection, bounded backlog; offline retention policy. | F0; serial shared-worker/cache edits. No destructive cache reset as default recovery. |
| **F7 Draft recovery + MIME** | P0/P1; `compose.rs` and new recovery/document module; F0 owns native UI integration | Private local autosave/crash recovery; request-correlated server save; attachment lifetime; HTML conversion notices; Bcc/Reply-To/thread headers preserved; no silent attachment loss on forward. | F0/F5/F6. Native rich editor presentation split into later package; do not edit GTK composer. |
| **H1 Faithful HTML/CSS reader module** | P0; new renderer/resource-policy module; existing main/live integration by parent-designated owners | Consume dedicated renderer findings when available. Preserve safe sender CSS/layout/dimensions, resolve CID from MIME, default-deny remote resources with explicit message/sender consent, constrain scripts/navigation/network, responsive reflow and local wide-content overflow, selection/copy/accessibility and plain fallback. Use fictional newsletter/receipt/RTL/table/CID visual fixtures. | F0/F7 resource interfaces and privacy invariants. Detailed engine comparison belongs to dedicated research. No duplicate research or parallel main/live edits. |
| **H2 Selected-body latency module/measurement** | P0; dedicated research owns traces; new scheduling/instrumentation files where possible, adapter/live edits coordinated by parent | Measure cold/warm selection-to-request, body-ready, sanitize/layout, first-visible-body and stable content per adapter. Prioritize current selection over background refresh, discard stale results, honest immediate loading, bounded cache. Set targets from baseline, not invented numbers. | F0/F2/F6 plus H1 rendering stages; do not assume direct-engine replacement fixes UI/bridge latency. Dedicated researcher owns profiling; parent serial Cargo. |
| **D1 Validation/measurement fixtures** | P0 throughout; new fixture/test files only; parent runs Cargo | Real interface fixtures for startup, transport, malformed MIME, send uncertainty, epoch changes, queue saturation and reconnect. Representative synthetic performance budgets and Linux a11y checklist. | F0 contract. No claims of speed, provider send, or test success without measurement. |
| **T1 Folder/bulk mutation module** | P1; new native triage module; backend integration by F0/F5/F2 owners | Multi-select/range/select-all semantics; per-item outcomes; archive/trash/move/read/star/spam; undo ownership; folder CRUD/capabilities; apply labels without pretending folder moves are labels. | F0/F5/F6, TB capability support. New UI module only; no parallel main/live edits. |
| **S1 Search/offline module** | P1; new search AST/index/controller files; query integration by F6 | All-history/all-folder/body search, filters/date/recipient/attachment terms, provider fallback, explicit offline scope, cancellation/paging; saved searches. | F0/F6, direct+TB access contracts. No “loaded headers” relabelled full search. |
| **C1 Compose productivity** | P1/P2; new editor/signature/template/scheduling modules owned separately; shared compose integration serial | Rich-text document safety, signatures/alias management, address completion, original attachments, spellcheck, templates, deferred send/undo/follow-up. | F7/F5; contacts contract for completion. Send Later while app closed requires scheduler/server policy, not just a button. |
| **B1 Contacts/invitations/calendar** | P1/P2; separate new contacts and calendar modules; native UI integration F0 | Contacts CRUD/import/CardDAV; ICS invitation preview/RSVP; CalDAV/provider event CRUD/recurrence/timezones/tasks; consent/permission scopes explicit. | F3/F4/F7; contacts/calendar may fan out only after stable contracts. Not calendar-by-rendering-a-date-grid. |
| **U1 Linux integration/accessibility** | P1; new desktop/action/a11y modules, packaging owner separate | Keyboard/focus/accessibility tree, scaling/RTL/contrast, mailto, notifications, tray policy, default app, printing, screen-reader and compositor verification. | F0; preserve Zeron design. Windows/macOS/mobile are separate P3 products. |
| **R1 Rules and advanced triage** | P2; new rules engine/config UI files; existing worker filter edits by shared owner | Deterministic rules, preview/dry-run, actions and failures, snooze/mute/VIP/categorization; recoverable timers. | T1/S1/F5/F6. Deterministic features first; no AI required for basic rules. |
| **A1 Optional AI/team exploration** | P3 initially; new consent/provider and collaboration design modules only | Explicit per-feature data scope/retention/no-training policy, prompt injection controls, human approval for mail mutations; summaries/rewrite before autonomous actions. Team auth/roles/share revocation before shared inbox. | Reliable foundations + S1/C1/B1. Never let untrusted incoming email authorize an action. Not launch-blocking email basics. |

### Safe fanout waves

- **Current wave:** keep the four already-running foundation owners; parent tracks disjoint files/integration. Dedicated H1/H2 researchers run separately. This inventory launches no agents and proposes no backend replacement.
- **Next P0 wave:** F0/D1 contracts, F2 runtime, F7 recovery, and H1/H2 implementation planning in new modules as their research arrives. Faithful reader/latency is not deferred until optional triage/AI. Startup/main/live/adapter/OAuth overlap stays with current owners and parent serial integration.
- **Following foundation wave:** remaining F5/F6 worker/cache work is **serial** where files overlap; S1 may prepare new AST/controller files. Parent runs Cargo between integrated slices, not subagents.
- **Wave 3:** T1, C1, B1, U1 independent new modules; serial UI/event/protocol integration by F0. Rules afterward; AI/team/platform scope last.
- Parent can select Pi GPT-6.1 Sol high-thinking implementation agents after this research handoff. This research agent launches no implementation fanout.

## Open questions and documentation debt

1. Do saved Thunderbird accounts need the original source profile forever, or should the app-owned clone become independently restorable? Credential refresh provenance and stale store policy must remain explicit.
2. What is the supported interactive Thunderbird login/Primary Password policy? Headless-only does not satisfy general Google/tenant re-auth.
3. Gmail scope choice: IMAP OAuth presently uses `https://mail.google.com/`; a Gmail REST adapter can choose other scopes, but this is not a drop-in protocol change [GO], [GS].
4. What offline retention and encrypted-content policy is promised per adapter? Local cache existence is not full offline availability.
5. Is native account removal expected to revoke credentials, delete cache, retain local archive, or only disconnect? Separate those actions.
6. Which send-later/reminder tasks run while MegaMail is closed? Local scheduling cannot match a hosted always-on promise without a background lifecycle.
7. Spark pricing lists Autopilot/Auto-Drafts as “SOON”, while its KB index already has Auto-Drafts documentation. Treat rollout/plan availability as unresolved, not universally shipped.
8. Superhuman's older AI overview says attachment metadata rather than contents is stored; current Ask AI can read attachments. Per-feature retention must be confirmed; do not extrapolate either statement globally.
9. Mozilla SUMO access remains a research limitation for detailed user-facing filters, backup/import, offline, printing, accessibility, and settings. API/standards evidence establishes only the scope stated.
10. No private profiles, authorized probe artifacts, mail credentials, or current live account failure logs were consulted. Exact recurrence and performance symptoms remain for authorized fixture/runtime work.

## Sources and source-code evidence

Public documents below were retrieved on **2026-10-10**, except entries explicitly identified as **blocked** or **linked-only**. Linked-only entries are navigation pointers, not evidence for uninspected detailed behavior. Dynamic pricing/platform pages can change; linked APIs are current generated docs, not a pin of the user's installed Thunderbird binary. Repository links resolve relative to this research file. Symbols and inspected line ranges provide navigation even if parent edits shift the snapshot.

- **A** — [Architecture: runtime, isolation, setup, bounded queries, validation limitations](../../docs/MEGAMAIL_ARCHITECTURE.md).
- **P** — [Product target, native capabilities, six-account probe and loopback evidence; not repeated here](../../PRODUCT.md).
- **N1** — [MailApp::new/startup 754-969; live_changed 1020-1155; sender_choices 1160-1244; actions 1774-2020; import picker 2121-2287; lists 3100-3469; render 5978-6220](../../apps/megamail/src/main.rs#L754).
- **N2** — [LiveMailbox restore/import 733-845; account/folder/message selection 895-1062; refresh/actions 1134-1377; attachment/draft source requests 1378-1551](../../apps/megamail/src/live.rs#L733).
- **N3** — [has_saved_accounts 101; import/restore 343-775; read/validate/write store 759-847; profile actor/start 849-1392; add_account 1393-1544; bounded caches/constants 34-255](../../apps/megamail/src/thunderbird_adapter.rs#L343).
- **N4** — [Runtime::start/start_with_executables; call 275-340; private clone, transport deadlines, permissions and platform fallback 1897-1912](../../crates/mail-core/src/thunderbird_bridge.rs#L102).
- **N5** — [OAuth presets 219-252, provider_credentials 322-412, run_flow 458, Authorization 528, refresh 650; config.rs credential helpers 1325-1519](../../crates/mail-core/src/oauth.rs#L219).
- **N6** — [oauth_settings; explicit registration gate; password/SMTP validation helpers](../../crates/mail-core/src/onboarding.rs#L68).
- **N7** — [AccountForm::build_manual; native main collect_account 2292, connect_account 2358, setup UI 4740-5260; discovery in core discovery.rs](../../apps/megamail/src/onboarding.rs#L147).
- **N8** — [Shared worker protocol dispatch 1160-1180; MailRequest 298-509; folder/bulk handling 2174-2790; direct sends/drafts 2792-3000; outbox 4265-4580; MIME 5030-5282; encryption cache exclusions 4038-4083](../../src/worker.rs#L1160).
- **N9** — [Schema identity keys 1-141; SCHEMA_VERSION 187; migration/init 489-658; message_page 878; upsert_messages 1028; persisted Outbox schema 103ff](../../src/cache.rs#L1).
- **N10** — [MessageKey/scope 59-79; retained mailbox state 372; event application 1668ff, DraftSaved 2521-2535; page/body/attachment key checks 4525-4703; filters/query 4589-4627](../../apps/megamail/src/live.rs#L59).
- **N11** — [composer_view 5273-5708; explicit Discard 5356-5387; send_current_message 5709; save_current_draft 5740; choose_attachments 5768; save_reader_attachment 5832](../../apps/megamail/src/main.rs#L5273).
- **N12** — [prepare_draft_from_raw 30-114; staging and bounds 138-361; new_message 372; reply/forward 395-416; reply_for_account 417-455; plain-text identity defaults](../../apps/megamail/src/compose.rs#L30).
- **N13** — [REQUEST_QUEUE_CAPACITY=32; QueryService 81-114; query_loop/coalesce 115-229; generation completion; src/cache.rs::message_page 878](../../crates/mail-core/src/query.rs#L16).
- **N14** — [ActorState::request support/rejections 2642-2902; send_message 2905; save_draft 2984; compose preflight 3942-4017; extension dispatch background.js 825-906](../../apps/megamail/src/thunderbird_adapter.rs#L2642).
- **N15** — [Portable exports versus native main.rs module list 58-68; root src/main.rs is GTK package, not native app](../../crates/mail-core/src/lib.rs#L1).
- **N16** — [FilterRule/conditions/matching/loading 1586-1840; legacy execution src/app.rs::apply_filters 19243; native caller absent from inspected main/live inventory](../../crates/mail-core/src/config.rs#L1586).
- **N17** — [Legacy Hylki feature inventory, NOT native parity; source src/contacts.rs 54/141, spell.rs 27/384, notify.rs 83, ui/rich_editor.rs, ui/print_preview.rs, ui/attachments_gallery.rs, translate.rs, tray.rs](../../docs/FEATURES.md).
- **N18** — [detect_raw/detect/parse/reply_ics 47/53/95/756; shared worker.rs calendar MIME 5030-5103; no inspected native invitation/calendar actions](../../src/invite.rs#L47).
- **N19** — [unwrap_message, key list/import/sign/encrypt 198/607/724/829/865; worker.rs::body_cacheable 4073 and pgp_wrap 5217; native compose flags false at compose.rs 389-390](../../src/pgp.rs#L198).
- **N20** — [detect/from_headers/one_click_post 48/163/817; sender verification src/verify.rs::check_sender 98; native UI path absent from inspected reader](../../src/unsubscribe.rs#L48).
- **N21** — [Mail settings 25-149; theme/background controls 150-323; accounts 325-354 only open/add; fixed shortcut list 356-385](../../apps/megamail/src/appearance_view.rs#L25).
- **N22** — [attachments_panel 3730; reader 3958-4739, safe selected HTML, links, plain mode, quote toggle; reader attachment save 5832](../../apps/megamail/src/main.rs#L3730).
- **N23** — [Reader size/node/depth limits; reader_html 53; quoted toggle 61; sanitation 65-249; extract_links 443; safe_browser_url 483; RTL fixtures 571ff](../../crates/mail-core/src/mail_text.rs#L7).
- **N24** — [MailPreferences density/grouping/quote/motion/default Unified, load/persist 68-120; no comprehensive account/search/offline settings model](../../apps/megamail/src/preferences.rs#L37).
- **N26** — [instance::acquire profile lock; native entry main.rs 7115-7174; core desktop.rs exports settings()/flag() rather than mailto parser](../../apps/megamail/src/instance.rs#L9).
- **TD** — [Thunderbird Desktop: free/open, multi-account/Unified Inbox, Linux/Windows/macOS, calendar/contacts/tasks](https://www.thunderbird.net/en-US/desktop/).
- **TS** — [Thunderbird supported standards: desktop vs Android; some lines explicitly partial/blank](https://developer.thunderbird.net/planning/standards-status.md).
- **TR** — [Thunderbird 157.0, released 2026-09-30: OAuth, Sent copy, IMAP setup/moves, filters, OpenPGP, CardDAV, recurring/CalDAV fixes; fixes do not prove all policies](https://www.thunderbird.net/en-US/thunderbird/157.0/releasenotes/).
- **TM** — [Generated Thunderbird 157.0 MV2 messages API; scope/IDs, query/fullText, bulk actions, raw/attachments, tags](https://webextension-api.thunderbird.net/en/mv2/messages.html).
- **TC** — [Thunderbird 157.0 MV2 compose API; HTML/plain, reply/forward, draft/template/sendLater queue, attachments, MDN/DSN, encryption; no timed scheduler promise](https://webextension-api.thunderbird.net/en/mv2/compose.html).
- **TF** — [Thunderbird 157.0 MV2 folder API: create/rename/delete/access; permission requirements](https://webextension-api.thunderbird.net/en/mv2/folders.html).
- **TA** — [Thunderbird 157.0 MV2 account types: IMAP/POP3/EWS/local/NNTP/RSS, EWS since 142](https://webextension-api.thunderbird.net/en/mv2/accounts.html).
- **TAB** — [Thunderbird address-books API CRUD/UI; generated 157.0](https://webextension-api.thunderbird.net/en/mv2/addressBooks.html).
- **TCONTACT** — [LINKED-ONLY: Thunderbird contacts API linked from fetched supported APIs guide; individual page not independently retrieved; contact editing/vCard also described in TI](https://webextension-api.thunderbird.net/en/mv2/contacts.html).
- **TL** — [Thunderbird mailing-list/member CRUD API; generated 157.0](https://webextension-api.thunderbird.net/en/mv2/mailingLists.html).
- **TX** — [Thunderbird extensions guide: compose/windows/contacts/commands/notifications/native messaging/themes; engine API not native MegaMail UX](https://developer.thunderbird.net/add-ons/mailextensions/supported-webextension-api).
- **TI** — [Thunderbird 157 MV3 current documentation index and paginated-list/vCard guides; API version choice matters](https://webextension-api.thunderbird.net/en/latest/).
- **TP** — [Thunderbird privacy notice; inspect per-service collection, not a claim of zero networking/telemetry](https://www.thunderbird.net/en-US/privacy/).
- **TBLOCK** — [BLOCKED: Mozilla SUMO index and KB automatic-account-configuration, imap-synchronization, global-search, keyboard-shortcuts-thunderbird, organize-your-messages-using-filters, openpgp-thunderbird-howto-and-faq, thunderbird-and-gmail, Primary Password; returned challenge, not article content](https://support.mozilla.org/en-US/products/thunderbird).
- **SF** — [Spark features: smart/unified Inbox, priority/triage, send/remind, team workflows; current listed device families](https://sparkmailapp.com/features).
- **SP** — [Current Spark Free/Plus/Pro/Enterprise comparison, AI quotas/history, team/storage limits, CLI and planned Auto-Drafts/Autopilot; old plans-comparison redirects here](https://sparkmailapp.com/pricing).
- **ST** — [Current first-party tutorial inventory, including compose, bulk/folders, printing, language, keyboard, integrations, AI/team; index confirms named workflow, not all limits](https://sparkmailapp.com/help/spark-tutorials).
- **SA** — [Spark providers, Google browser consent, iCloud app-specific password, account/settings cross-device sync](https://sparkmailapp.com/help/add-manage-accounts/connect-to-your-email-account-in-spark).
- **SO** — [No local mail archival/export/backup; past-month Wi-Fi cache; cached reading only offline; hosted Send Later/Teams](https://sparkmailapp.com/help/general/email-storage-and-backups).
- **SS** — [Smart Search operators/body/account scope, Gmail/non-Gmail folder limits, saved searches on mobile](https://sparkmailapp.com/help/manage-your-inbox/use-smart-search).
- **SSIG** — [Multiple account-default text/HTML/image signatures and per-message selection](https://sparkmailapp.com/help/sending-emails/manage-email-signatures).
- **SALI** — [Aliases for named providers, Google import, provider prerequisite/default/reply caveats](https://sparkmailapp.com/help/sending-emails/set-up-an-email-alias).
- **SU** — [Default five-second Undo Send and configurable Outbox delay on documented platforms](https://sparkmailapp.com/help/sending-emails/customize-undo-send-timer).
- **SOUT** — [Explicitly no in-app Vacation/OOO setup; provider workarounds](https://sparkmailapp.com/help/tips-tricks/out-of-office-auto-reply).
- **SV** — [Spark data/tokens/passwords/push/hosted features, TLS and encryption, GDPR/CCPA assertions](https://sparkmailapp.com/help/privacy-data/spark-email-privacy-everything-you-need-to-know).
- **SAI** — [Azure OpenAI, prompt/body processing, no model training, <=30-day vendor retention claim, human review](https://sparkmailapp.com/help/spark-ai/spark-ai-security-and-data).
- **SB** — [Proton Bridge Mac/Windows only; local-only account; excluded server-dependent features](https://sparkmailapp.com/help/add-manage-accounts/proton-mail-in-spark-desktop).
- **SC** — [Supported calendar providers; Yahoo/custom IMAP calendar control inactive](https://sparkmailapp.com/help/calendars/what-calendars-can-i-use-in-spark).
- **SCE** — [Events, attendees, alerts, conference links, recurrence; per-platform instructions](https://sparkmailapp.com/help/calendars/create-an-event).
- **SCV** — [Calendar selection/default/calendar views and supported accounts](https://sparkmailapp.com/help/calendars/enable-and-view-calendars-in-spark).
- **SSD** — [Team co-edit drafts; membership/send permissions; tutorial two-plus-collaborator upgrade language conflicts with new pricing tiers](https://sparkmailapp.com/help/spark-for-teams/shared-drafts-spark).
- **SSI** — [Shared account/team ownership without password sharing; Pro advanced collaboration](https://sparkmailapp.com/help/spark-for-teams/shared-inboxes).
- **SREAD** — [Pro Read Statuses, tracking pixels, blocking limitations and remote-image off controls](https://sparkmailapp.com/help/spark-for-teams/read-statuses).
- **SM** — [Desktop Smart Folders on paid plans; provider scope limits; local Spark view not provider folder](https://sparkmailapp.com/help/manage-your-inbox/create-smart-folder).
- **SK** — [Desktop presets/custom shortcuts and action inventory](https://sparkmailapp.com/help/tips-tricks/use-keyboard-shortcuts).
- **SLOCK** — [Mobile lock/biometric policy; forgotten passkey requires reinstall](https://sparkmailapp.com/help/tips-tricks/passkey-lock-on-mobile-devices).
- **SCLI** — [LINKED-ONLY: CLI knowledge-base section linked from current help index; individual section not fetched; entitlement evidence SP](https://sparkmailapp.com/help/spark-cli).
- **GO** — [Google installed-app OAuth: client registration, system browser policy, PKCE, loopback, verification](https://developers.google.com/identity/protocols/oauth2/native-app).
- **GS** — [Google Gmail scopes; mail.google.com and other restricted scopes, verification and least-privilege implications](https://developers.google.com/workspace/gmail/api/auth/scopes).
- **MS** — [Microsoft authorization code/PKCE, consent/admin restrictions and refresh](https://learn.microsoft.com/en-us/entra/identity-platform/v2-oauth2-auth-code-flow).
- **MR** — [Microsoft app registration/mobile-desktop redirect platform setup](https://learn.microsoft.com/en-us/graph/auth-register-app-v2).
- **HP** — [Pricing Plans; fetched public /api/v2/help_center/en-us/articles/46005733349517.json because old help routes failed; updated 2026-08-18T21:20:31Z](https://help.superhuman.com/hc/en-us/articles/46005733349517-Pricing-Plans).
- **HD** — [Download Superhuman Mail; fetched public /api/v2/help_center/en-us/articles/46005778798605.json because old help routes failed; updated 2026-10-09T16:56:10Z](https://help.superhuman.com/hc/en-us/articles/46005778798605-Download-Superhuman-Mail).
- **HA** — [Managing Accounts; fetched public /api/v2/help_center/en-us/articles/46005777934733.json because old help routes failed; updated 2026-10-02T09:20:36Z](https://help.superhuman.com/hc/en-us/articles/46005777934733-Managing-Accounts).
- **HO** — [Offline Access; fetched public /api/v2/help_center/en-us/articles/46005499629325.json because old help routes failed; updated 2026-08-19T21:34:45Z](https://help.superhuman.com/hc/en-us/articles/46005499629325-Offline-Access).
- **HU** — [Unified Inbox Workaround; fetched public /api/v2/help_center/en-us/articles/46005722297229.json because old help routes failed; updated 2026-10-03T19:52:41Z](https://help.superhuman.com/hc/en-us/articles/46005722297229-Unified-Inbox-Workaround).
- **HSEARCH** — [Search; fetched public /api/v2/help_center/en-us/articles/46005672652301.json because old help routes failed; updated 2026-09-01T14:23:32Z](https://help.superhuman.com/hc/en-us/articles/46005672652301-Search).
- **HF** — [Folders; fetched public /api/v2/help_center/en-us/articles/46005732666253.json because old help routes failed; updated 2026-08-17T21:55:34Z](https://help.superhuman.com/hc/en-us/articles/46005732666253-Folders).
- **HL** — [Labels; fetched public /api/v2/help_center/en-us/articles/46005736546061.json because old help routes failed; updated 2026-09-10T17:12:19Z](https://help.superhuman.com/hc/en-us/articles/46005736546061-Labels).
- **HAI** — [Superhuman AI Overview; fetched public /api/v2/help_center/en-us/articles/46005588676237.json because old help routes failed; updated 2026-07-16T10:15:25Z](https://help.superhuman.com/hc/en-us/articles/46005588676237-Superhuman-AI-Overview).
- **HAL** — [Auto Labels; fetched public /api/v2/help_center/en-us/articles/46005657758861.json because old help routes failed; updated 2026-09-01T14:25:26Z](https://help.superhuman.com/hc/en-us/articles/46005657758861-Auto-Labels).
- **HAD** — [Auto Reminders & Auto Drafts; fetched public /api/v2/help_center/en-us/articles/46005658551053.json because old help routes failed; updated 2026-09-01T14:24:26Z](https://help.superhuman.com/hc/en-us/articles/46005658551053-Auto-Reminders-Auto-Drafts).
- **HAARCH** — [Auto Archive; fetched public /api/v2/help_center/en-us/articles/46005662460813.json because old help routes failed; updated 2026-09-01T14:25:03Z](https://help.superhuman.com/hc/en-us/articles/46005662460813-Auto-Archive).
- **HSMART** — [Smart Send; fetched public /api/v2/help_center/en-us/articles/46005572688525.json because old help routes failed; updated 2026-10-01T21:15:20Z](https://help.superhuman.com/hc/en-us/articles/46005572688525-Smart-Send).
- **HCAL** — [Calendar Overview; fetched public /api/v2/help_center/en-us/articles/46005615985293.json because old help routes failed; updated 2026-09-01T14:24:32Z](https://help.superhuman.com/hc/en-us/articles/46005615985293-Calendar-Overview).
- **HEVENT** — [Create Event; fetched public /api/v2/help_center/en-us/articles/46005621734669.json because old help routes failed; updated 2026-09-10T21:45:33Z](https://help.superhuman.com/hc/en-us/articles/46005621734669-Create-Event).
- **HTIME** — [Team Scheduling; fetched public /api/v2/help_center/en-us/articles/46005682760973.json because old help routes failed; updated 2026-09-01T14:24:25Z](https://help.superhuman.com/hc/en-us/articles/46005682760973-Team-Scheduling).
- **HSD** — [Shared Drafts; fetched public /api/v2/help_center/en-us/articles/46005578703885.json because old help routes failed; updated 2026-09-01T14:25:07Z](https://help.superhuman.com/hc/en-us/articles/46005578703885-Shared-Drafts).
- **HC** — [Shared Conversations and Team Comments; fetched public /api/v2/help_center/en-us/articles/46005593675917.json because old help routes failed; updated 2026-09-09T12:13:57Z](https://help.superhuman.com/hc/en-us/articles/46005593675917-Shared-Conversations-and-Team-Comments).
- **HTEAMREAD** — [Team Read Statuses and Team Reply Indicators; fetched public /api/v2/help_center/en-us/articles/46005718826125.json because old help routes failed; updated 2026-09-01T14:24:26Z](https://help.superhuman.com/hc/en-us/articles/46005718826125-Team-Read-Statuses-and-Team-Reply-Indicators).
- **HBYOK** — [Bring Your Own Key (BYOK); fetched public /api/v2/help_center/en-us/articles/46005802988813.json because old help routes failed; updated 2026-05-29T13:48:24Z](https://help.superhuman.com/hc/en-us/articles/46005802988813-Bring-Your-Own-Key-BYOK).
- **HSCIM** — [User Provisioning (SCIM); fetched public /api/v2/help_center/en-us/articles/46005519038093.json because old help routes failed; updated 2026-07-07T18:35:05Z](https://help.superhuman.com/hc/en-us/articles/46005519038093-User-Provisioning-SCIM).
- **HS** — [SSO for Google and Microsoft; fetched public /api/v2/help_center/en-us/articles/46005513632653.json because old help routes failed; updated 2026-07-17T00:29:11Z](https://help.superhuman.com/hc/en-us/articles/46005513632653-SSO-for-Google-and-Microsoft).
- **HUNDO** — [Undo; fetched public /api/v2/help_center/en-us/articles/46005666743309.json because old help routes failed](https://help.superhuman.com/hc/en-us/articles/46005666743309-Undo).
- **HREM** — [Remind Me; fetched public /api/v2/help_center/en-us/articles/46005666142733.json because old help routes failed](https://help.superhuman.com/hc/en-us/articles/46005666142733-Remind-Me).
- **HSPLIT** — [Custom Split Inbox; fetched public /api/v2/help_center/en-us/articles/46005636204941.json because old help routes failed](https://help.superhuman.com/hc/en-us/articles/46005636204941-Custom-Split-Inbox).
- **HSNIP** — [Snippets; fetched public /api/v2/help_center/en-us/articles/46005686571149.json because old help routes failed](https://help.superhuman.com/hc/en-us/articles/46005686571149-Snippets).
- **HFORMAT** — [Formatting Text; fetched public /api/v2/help_center/en-us/articles/46005721681165.json because old help routes failed](https://help.superhuman.com/hc/en-us/articles/46005721681165-Formatting-Text).
- **HATT** — [Attachments; fetched public /api/v2/help_center/en-us/articles/46005568142989.json because old help routes failed](https://help.superhuman.com/hc/en-us/articles/46005568142989-Attachments).
- **HSIG** — [Signatures; fetched public /api/v2/help_center/en-us/articles/46005771841933.json because old help routes failed](https://help.superhuman.com/hc/en-us/articles/46005771841933-Signatures).
- **HOOO** — [Out of Office; fetched public /api/v2/help_center/en-us/articles/46005626883725.json because old help routes failed](https://help.superhuman.com/hc/en-us/articles/46005626883725-Out-of-Office).
- **HCONTACT** — [Contact Pane; fetched public /api/v2/help_center/en-us/articles/46005778939789.json because old help routes failed](https://help.superhuman.com/hc/en-us/articles/46005778939789-Contact-Pane).
- **HMCP** — [Superhuman Mail MCP Server; fetched public /api/v2/help_center/en-us/articles/46005696690317.json because old help routes failed](https://help.superhuman.com/hc/en-us/articles/46005696690317-Superhuman-Mail-MCP-Server).
- **HSUM** — [Auto Summarize; fetched public /api/v2/help_center/en-us/articles/46005642123917.json because old help routes failed](https://help.superhuman.com/hc/en-us/articles/46005642123917-Auto-Summarize).
- **HASK** — [Ask AI; fetched public /api/v2/help_center/en-us/articles/46005676610829.json because old help routes failed](https://help.superhuman.com/hc/en-us/articles/46005676610829-Ask-AI).
- **HWRITE** — [Write with AI; fetched public /api/v2/help_center/en-us/articles/46005557122957.json because old help routes failed](https://help.superhuman.com/hc/en-us/articles/46005557122957-Write-with-AI).
- **HAUTO** — [Autocorrect; fetched public /api/v2/help_center/en-us/articles/46005640149389.json because old help routes failed](https://help.superhuman.com/hc/en-us/articles/46005640149389-Autocorrect).
- **HN** — [Email Notifications; fetched public /api/v2/help_center/en-us/articles/46005802618765.json because old help routes failed](https://help.superhuman.com/hc/en-us/articles/46005802618765-Email-Notifications).
- **HK** — [Keyboard Shortcuts in Superhuman Mail; fetched public /api/v2/help_center/en-us/articles/46005701270541.json because old help routes failed](https://help.superhuman.com/hc/en-us/articles/46005701270541-Keyboard-Shortcuts-in-Superhuman-Mail).
- **HUNWANTED** — [Dealing with Unwanted Emails; fetched public /api/v2/help_center/en-us/articles/46005635358349.json because old help routes failed](https://help.superhuman.com/hc/en-us/articles/46005635358349-Dealing-with-Unwanted-Emails).
- **HMASS** — [Mass Archive; fetched public /api/v2/help_center/en-us/articles/46005611576589.json because old help routes failed](https://help.superhuman.com/hc/en-us/articles/46005611576589-Mass-Archive).
- **HMUTE** — [Muting Conversations; fetched public /api/v2/help_center/en-us/articles/46005597483661.json because old help routes failed](https://help.superhuman.com/hc/en-us/articles/46005597483661-Muting-Conversations).
- **HAlIAS** — [Alias; fetched public /api/v2/help_center/en-us/articles/46005743269901.json because old help routes failed](https://help.superhuman.com/hc/en-us/articles/46005743269901-Alias).
- **HABCC** — [Auto Bcc; fetched public /api/v2/help_center/en-us/articles/46005654497549.json because old help routes failed](https://help.superhuman.com/hc/en-us/articles/46005654497549-Auto-Bcc).
- **HREAD** — [Read Statuses and Recent Opens Feed; fetched public /api/v2/help_center/en-us/articles/46005603745293.json because old help routes failed](https://help.superhuman.com/hc/en-us/articles/46005603745293-Read-Statuses-and-Recent-Opens-Feed).
- **HPRINT** — [Print; fetched public /api/v2/help_center/en-us/articles/46005716192653.json because old help routes failed](https://help.superhuman.com/hc/en-us/articles/46005716192653-Print).
- **HIR** — [Instant Reply; fetched public /api/v2/help_center/en-us/articles/46005583725709.json because old help routes failed](https://help.superhuman.com/hc/en-us/articles/46005583725709-Instant-Reply).
- **HAVAIL** — [Share Availability; fetched public /api/v2/help_center/en-us/articles/46005575115021.json because old help routes failed](https://help.superhuman.com/hc/en-us/articles/46005575115021-Share-Availability).
- **HTHEME** — [Theme; fetched public /api/v2/help_center/en-us/articles/46005763646093.json because old help routes failed](https://help.superhuman.com/hc/en-us/articles/46005763646093-Theme).
- **HMAIN** — [Current Superhuman Mail feature page; Gmail/Outlook, splits, reminders, Snippets, send later, read statuses, calendar; marketing performance not measured](https://superhuman.com/mail).

[A]: ../../docs/MEGAMAIL_ARCHITECTURE.md
[P]: ../../PRODUCT.md
[N1]: ../../apps/megamail/src/main.rs#L754
[N2]: ../../apps/megamail/src/live.rs#L733
[N3]: ../../apps/megamail/src/thunderbird_adapter.rs#L343
[N4]: ../../crates/mail-core/src/thunderbird_bridge.rs#L102
[N5]: ../../crates/mail-core/src/oauth.rs#L219
[N6]: ../../crates/mail-core/src/onboarding.rs#L68
[N7]: ../../apps/megamail/src/onboarding.rs#L147
[N8]: ../../src/worker.rs#L1160
[N9]: ../../src/cache.rs#L1
[N10]: ../../apps/megamail/src/live.rs#L59
[N11]: ../../apps/megamail/src/main.rs#L5273
[N12]: ../../apps/megamail/src/compose.rs#L30
[N13]: ../../crates/mail-core/src/query.rs#L16
[N14]: ../../apps/megamail/src/thunderbird_adapter.rs#L2642
[N15]: ../../crates/mail-core/src/lib.rs#L1
[N16]: ../../crates/mail-core/src/config.rs#L1586
[N17]: ../../docs/FEATURES.md
[N18]: ../../src/invite.rs#L47
[N19]: ../../src/pgp.rs#L198
[N20]: ../../src/unsubscribe.rs#L48
[N21]: ../../apps/megamail/src/appearance_view.rs#L25
[N22]: ../../apps/megamail/src/main.rs#L3730
[N23]: ../../crates/mail-core/src/mail_text.rs#L7
[N24]: ../../apps/megamail/src/preferences.rs#L37
[N26]: ../../apps/megamail/src/instance.rs#L9
[TD]: https://www.thunderbird.net/en-US/desktop/
[TS]: https://developer.thunderbird.net/planning/standards-status.md
[TR]: https://www.thunderbird.net/en-US/thunderbird/157.0/releasenotes/
[TM]: https://webextension-api.thunderbird.net/en/mv2/messages.html
[TC]: https://webextension-api.thunderbird.net/en/mv2/compose.html
[TF]: https://webextension-api.thunderbird.net/en/mv2/folders.html
[TA]: https://webextension-api.thunderbird.net/en/mv2/accounts.html
[TAB]: https://webextension-api.thunderbird.net/en/mv2/addressBooks.html
[TCONTACT]: https://webextension-api.thunderbird.net/en/mv2/contacts.html
[TL]: https://webextension-api.thunderbird.net/en/mv2/mailingLists.html
[TX]: https://developer.thunderbird.net/add-ons/mailextensions/supported-webextension-api
[TI]: https://webextension-api.thunderbird.net/en/latest/
[TP]: https://www.thunderbird.net/en-US/privacy/
[TBLOCK]: https://support.mozilla.org/en-US/products/thunderbird
[SF]: https://sparkmailapp.com/features
[SP]: https://sparkmailapp.com/pricing
[ST]: https://sparkmailapp.com/help/spark-tutorials
[SA]: https://sparkmailapp.com/help/add-manage-accounts/connect-to-your-email-account-in-spark
[SO]: https://sparkmailapp.com/help/general/email-storage-and-backups
[SS]: https://sparkmailapp.com/help/manage-your-inbox/use-smart-search
[SSIG]: https://sparkmailapp.com/help/sending-emails/manage-email-signatures
[SALI]: https://sparkmailapp.com/help/sending-emails/set-up-an-email-alias
[SU]: https://sparkmailapp.com/help/sending-emails/customize-undo-send-timer
[SOUT]: https://sparkmailapp.com/help/tips-tricks/out-of-office-auto-reply
[SV]: https://sparkmailapp.com/help/privacy-data/spark-email-privacy-everything-you-need-to-know
[SAI]: https://sparkmailapp.com/help/spark-ai/spark-ai-security-and-data
[SB]: https://sparkmailapp.com/help/add-manage-accounts/proton-mail-in-spark-desktop
[SC]: https://sparkmailapp.com/help/calendars/what-calendars-can-i-use-in-spark
[SCE]: https://sparkmailapp.com/help/calendars/create-an-event
[SCV]: https://sparkmailapp.com/help/calendars/enable-and-view-calendars-in-spark
[SSD]: https://sparkmailapp.com/help/spark-for-teams/shared-drafts-spark
[SSI]: https://sparkmailapp.com/help/spark-for-teams/shared-inboxes
[SREAD]: https://sparkmailapp.com/help/spark-for-teams/read-statuses
[SM]: https://sparkmailapp.com/help/manage-your-inbox/create-smart-folder
[SK]: https://sparkmailapp.com/help/tips-tricks/use-keyboard-shortcuts
[SLOCK]: https://sparkmailapp.com/help/tips-tricks/passkey-lock-on-mobile-devices
[SCLI]: https://sparkmailapp.com/help/spark-cli
[GO]: https://developers.google.com/identity/protocols/oauth2/native-app
[GS]: https://developers.google.com/workspace/gmail/api/auth/scopes
[MS]: https://learn.microsoft.com/en-us/entra/identity-platform/v2-oauth2-auth-code-flow
[MR]: https://learn.microsoft.com/en-us/graph/auth-register-app-v2
[HP]: https://help.superhuman.com/hc/en-us/articles/46005733349517-Pricing-Plans
[HD]: https://help.superhuman.com/hc/en-us/articles/46005778798605-Download-Superhuman-Mail
[HA]: https://help.superhuman.com/hc/en-us/articles/46005777934733-Managing-Accounts
[HO]: https://help.superhuman.com/hc/en-us/articles/46005499629325-Offline-Access
[HU]: https://help.superhuman.com/hc/en-us/articles/46005722297229-Unified-Inbox-Workaround
[HSEARCH]: https://help.superhuman.com/hc/en-us/articles/46005672652301-Search
[HF]: https://help.superhuman.com/hc/en-us/articles/46005732666253-Folders
[HL]: https://help.superhuman.com/hc/en-us/articles/46005736546061-Labels
[HAI]: https://help.superhuman.com/hc/en-us/articles/46005588676237-Superhuman-AI-Overview
[HAL]: https://help.superhuman.com/hc/en-us/articles/46005657758861-Auto-Labels
[HAD]: https://help.superhuman.com/hc/en-us/articles/46005658551053-Auto-Reminders-Auto-Drafts
[HAARCH]: https://help.superhuman.com/hc/en-us/articles/46005662460813-Auto-Archive
[HSMART]: https://help.superhuman.com/hc/en-us/articles/46005572688525-Smart-Send
[HCAL]: https://help.superhuman.com/hc/en-us/articles/46005615985293-Calendar-Overview
[HEVENT]: https://help.superhuman.com/hc/en-us/articles/46005621734669-Create-Event
[HTIME]: https://help.superhuman.com/hc/en-us/articles/46005682760973-Team-Scheduling
[HSD]: https://help.superhuman.com/hc/en-us/articles/46005578703885-Shared-Drafts
[HC]: https://help.superhuman.com/hc/en-us/articles/46005593675917-Shared-Conversations-and-Team-Comments
[HTEAMREAD]: https://help.superhuman.com/hc/en-us/articles/46005718826125-Team-Read-Statuses-and-Team-Reply-Indicators
[HBYOK]: https://help.superhuman.com/hc/en-us/articles/46005802988813-Bring-Your-Own-Key-BYOK
[HSCIM]: https://help.superhuman.com/hc/en-us/articles/46005519038093-User-Provisioning-SCIM
[HS]: https://help.superhuman.com/hc/en-us/articles/46005513632653-SSO-for-Google-and-Microsoft
[HUNDO]: https://help.superhuman.com/hc/en-us/articles/46005666743309-Undo
[HREM]: https://help.superhuman.com/hc/en-us/articles/46005666142733-Remind-Me
[HSPLIT]: https://help.superhuman.com/hc/en-us/articles/46005636204941-Custom-Split-Inbox
[HSNIP]: https://help.superhuman.com/hc/en-us/articles/46005686571149-Snippets
[HFORMAT]: https://help.superhuman.com/hc/en-us/articles/46005721681165-Formatting-Text
[HATT]: https://help.superhuman.com/hc/en-us/articles/46005568142989-Attachments
[HSIG]: https://help.superhuman.com/hc/en-us/articles/46005771841933-Signatures
[HOOO]: https://help.superhuman.com/hc/en-us/articles/46005626883725-Out-of-Office
[HCONTACT]: https://help.superhuman.com/hc/en-us/articles/46005778939789-Contact-Pane
[HMCP]: https://help.superhuman.com/hc/en-us/articles/46005696690317-Superhuman-Mail-MCP-Server
[HSUM]: https://help.superhuman.com/hc/en-us/articles/46005642123917-Auto-Summarize
[HASK]: https://help.superhuman.com/hc/en-us/articles/46005676610829-Ask-AI
[HWRITE]: https://help.superhuman.com/hc/en-us/articles/46005557122957-Write-with-AI
[HAUTO]: https://help.superhuman.com/hc/en-us/articles/46005640149389-Autocorrect
[HN]: https://help.superhuman.com/hc/en-us/articles/46005802618765-Email-Notifications
[HK]: https://help.superhuman.com/hc/en-us/articles/46005701270541-Keyboard-Shortcuts-in-Superhuman-Mail
[HUNWANTED]: https://help.superhuman.com/hc/en-us/articles/46005635358349-Dealing-with-Unwanted-Emails
[HMASS]: https://help.superhuman.com/hc/en-us/articles/46005611576589-Mass-Archive
[HMUTE]: https://help.superhuman.com/hc/en-us/articles/46005597483661-Muting-Conversations
[HAlIAS]: https://help.superhuman.com/hc/en-us/articles/46005743269901-Alias
[HABCC]: https://help.superhuman.com/hc/en-us/articles/46005654497549-Auto-Bcc
[HREAD]: https://help.superhuman.com/hc/en-us/articles/46005603745293-Read-Statuses-and-Recent-Opens-Feed
[HPRINT]: https://help.superhuman.com/hc/en-us/articles/46005716192653-Print
[HIR]: https://help.superhuman.com/hc/en-us/articles/46005583725709-Instant-Reply
[HAVAIL]: https://help.superhuman.com/hc/en-us/articles/46005575115021-Share-Availability
[HTHEME]: https://help.superhuman.com/hc/en-us/articles/46005763646093-Theme
[HMAIN]: https://superhuman.com/mail

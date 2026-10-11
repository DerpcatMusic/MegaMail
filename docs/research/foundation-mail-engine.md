# Foundation mail-engine audit and research

Audit date: 2026-10-10. Local source snapshot: `d4dbd2bba14f474a33870269cc15bd98376ff360` (working tree initially clean).

## Decision in brief

**Keep both production implementations. Repair correctness and lifecycle contracts before replacing protocols.** Thunderbird is a supported, explicit backend choice, not an import wizard that users must repeat. The direct implementation already exists: Hylki workers implement IMAP/SMTP, POP3, Graph and JMAP. Native onboarding currently exposes only password-based IMAP and the separate Thunderbird path. Adding a third engine from scratch would duplicate existing work before establishing what is broken or expensive.

Introduce a native account-engine seam around the existing direct workers and Thunderbird actor, not around the demo-only `src/backend.rs::MailBackend`. Start with typed admission errors and correlated completion, then account/session epochs and honest capabilities. Preserve the original GTK interface during extraction. Keep caches backend-owned initially; do not put Thunderbird's ephemeral message IDs into Hylki SQLite tables.

**First narrow implementation slice:** fix direct IMAP requests silently consumed while IMAP reconnection fails. In particular, an accepted send must finish as durably queued or failed with its composition recoverable; it must not remain pending forever. Read-only Outbox listing and local discard should not require a live IMAP session. This is more urgent than another abstraction layer or a claimed faster engine. File ownership and acceptance cases are in §9.

No faster-backend claim is supported by this audit. Compare equivalent workloads, including Thunderbird's child process, before deciding whether to change the direct implementation or library.

## 1. Scope, evidence and restrictions

Read `AGENTS.md`, `PRODUCT.md`, `docs/MEGAMAIL_ARCHITECTURE.md`, existing onboarding/performance research, the relevant research, codebase-design, diagnosing-bugs, Rust and find-docs skills, and targeted production source. `graft ask ... --source` returned no matching nodes; source inspection then used `rg` and file reads. Rust skill rule-file links are missing from this installation; the installed skill and relevant Apollo performance/error/dispatch chapters were available.

This is **research and static inspection**, not a completed diagnosis of a reported runtime incident. The diagnosing-bugs feedback-loop gate cannot be satisfied under the task's no-Cargo/no-profile/no-mail-execution restrictions. No red-capable runtime command was run, and no claim below means a provider reproduction occurred. Static control-flow contradictions can nevertheless be established from source. Runtime hypotheses have explicit proposed falsification cases.

No implementation, commits, Cargo builds/checks/tests, private profile or credential reads, Thunderbird launches, or mail-server requests were performed. Public first-party documentation/source was fetched with HTTP GET. Context7 resolved `/chatmail/async-imap` and returned authentication/IDLE documentation; primary upstream source remains authoritative for version-specific details. Current upstream `main`/`master` sources are moving references, not proof about every installed Thunderbird version. Official architecture landing pages returned HTTP 403; primary Thunderbird source was used instead. A guessed JMAP landing URL returned 404; authoritative RFCs were used. No failed URL is treated as evidence.

Labels used throughout:

- **Confirmed static defect:** the inspected code violates a stated contract under an identifiable input/state. Not a live-provider reproduction.
- **Suspected bug:** a plausible runtime failure whose occurrence or impact is unmeasured/unreproduced.
- **Already implemented, unverified here:** production logic exists; not missing work and not a new verification claim.
- **New work:** capability or product behavior absent from the inspected native path.

The prior interrupted audits supplied no recoverable evidence. This report does not inherit findings from them. Historical files in `docs/research/` are useful context, but their line numbers and some recommendations precede the current Thunderbird integration.

## 2. Actual implementations and ownership

| Module / seam | What exists | Ownership and contract | Source |
|---|---|---|---|
| `src/backend.rs::MailBackend` | Three synchronous getters returning owned vectors; only `MockBackend` implements it | Demo data, not production network or lifecycle abstraction. Its introductory comment overstates production use. Do not expand or delete it as the first engine refactor. | [trait](../../src/backend.rs#L12), [only implementation](../../src/backend.rs#L212), [mock worker](../../src/worker.rs#L9892) |
| `crates/mail-core` | Portable Hylki modules through `#[path]`, MegaMail-specific config/OAuth/discovery/query/TB modules | Same worker/cache source used by GTK and native app; different configuration implementation yields isolated app paths/credential namespace | [module wiring](../../crates/mail-core/src/lib.rs#L4) |
| Direct account worker | `worker::spawn` → cache lane + mail thread/current-thread Tokio + LocalSet; protocol dispatch to IMAP, POP3, Graph or JMAP | Account-scoped protocol session, sync, mutations, Outbox and cache access. GTK consumers retain `MailRequest`/`WorkerEvent`. Cache lane answers cached reads without waiting behind network work. | [spawn](../../src/worker.rs#L896), [cache lane](../../src/worker.rs#L960), [dispatch](../../src/worker.rs#L1160) |
| Native query lane | `QueryService`, bounded submissions, coalesced first pages and generation-tagged callbacks | Separate SQLite WAL reader. Shared native service, not a provider protocol implementation. Every accepted page gets Ready/Failed/Superseded. | [contract and start](../../crates/mail-core/src/query.rs#L63), [query loop](../../crates/mail-core/src/query.rs#L117) |
| Native UI seam today | `RequestSender = Arc<dyn Fn(MailRequest) -> bool>` and `PageSender = ... Result<(), SubmitError>` | `LiveMailbox` owns separate sender maps, backend membership, selection, generations, pending sends/bodies, reconnect decisions and event projection | [closures](../../apps/megamail/src/live.rs#L29), [retained state](../../apps/megamail/src/live.rs#L372), [direct worker wrapper](../../apps/megamail/src/live.rs#L4115) |
| Thunderbird native adapter | One `ActorState`/bounded 64-command queue per source profile, account bindings and session IDs, threaded read/refresh jobs | Source manifest stores profile references + stable app IDs. Actor owns ephemeral TB→native mappings, cursor tokens, 16 MiB/8-entry body cache; it emits native mailbox events directly. Several accounts share a runtime. | [types/limits](../../apps/megamail/src/thunderbird_adapter.rs#L36), [actor state](../../apps/megamail/src/thunderbird_adapter.rs#L281), [actor loop](../../apps/megamail/src/thunderbird_adapter.rs#L947) |
| Thunderbird bridge | Cloneable synchronous `Runtime::call`, request ID registry, framed Unix socket, native host, child process, private clone lock | Owns process/profile/transport lifetime, operation deadlines, response routing and cleanup. It does not own GPUI selection. | [runtime](../../crates/mail-core/src/thunderbird_bridge.rs#L93), [call](../../crates/mail-core/src/thunderbird_bridge.rs#L273), [cleanup](../../crates/mail-core/src/thunderbird_bridge.rs#L340) |
| Bundled extension | `messages`, `folders`, `accounts`, `compose` operations plus privileged `megamailSync` experiment | Thunderbird owns authentication, folders, protocol sessions, mail store and send machinery. Custom experiment invokes XPCOM IMAP folder discovery/sync because ordinary list APIs are not a complete sync engine. | [dispatcher](../../tools/thunderbird-bridge/extension/background.js#L825), [experiment](../../tools/thunderbird-bridge/extension/api/implementation.js#L8) |

### Consequences for a real seam

There are two real adapters, but neither implements the current `MailBackend` trait. The valid seam is **native account operations/events**, above direct-worker and Thunderbird scheduling, below GPUI presentation. A deep module here should own admission outcomes, request completion, lifetime/epoch validity, account metadata and capability semantics. A pass-through trait over the existing closures alone is shallow: it does not remove complexity from callers.

Do not force both implementations to share a protocol session, credential store or cache. Thunderbird runs inside Gecko/XPCOM with its own profile; a native Rust IMAP session is not interchangeable with it. Inject these implementations at the native account-engine seam, and keep transport/testing seams internal.

### Existing direct HTTP engines are not hypothetical

`src/worker/graph.rs` is 1,836 lines and `src/worker/jmap.rs` 2,901 lines at this snapshot. Both participate in `worker::run`; both use the shared message/cache/send model. Graph lists a capped 300-header window and rewrites that folder's summary cache ([`graph_list_messages`](../../src/worker/graph.rs#L349), [`graph_load_folder`](../../src/worker/graph.rs#L1378)). JMAP batches `Email/query` plus `Email/get`, capped at 300 ([`jmap_list_messages`](../../src/worker/jmap.rs#L671)), and has a push path ([`spawn_jmap_push`](../../src/worker/jmap.rs#L1547)). Native password validation rejects non-IMAP profiles ([core validation](../../crates/mail-core/src/onboarding.rs#L261)). **Engine support is not the same as native onboarding support or full history support.**

## 3. Priority findings

### Confirmed static defects

**C1 — P0: accepted direct IMAP send can disappear without completion when incoming reconnection fails.**

The cache lane forwards `Send` and local Outbox operations (`other => Some(other)`, [worker:1059](../../src/worker.rs#L1059)). The IMAP loop's offline cache-first match handles lists/bodies/conversations/attachments but not send, schedule, local Outbox listing/discard ([worker:1758–1835](../../src/worker.rs#L1758)). Then a failed `connect_and_list` reaches unconditional `continue` ([worker:1837–1858](../../src/worker.rs#L1837)). Only list requests receive a terminal backfill signal there. `Send` never reaches its SMTP/send-failure branch ([worker:2839](../../src/worker.rs#L2839)), so it is not queued and does not produce `SendFinished`. The native UI already recorded a pending send by request ID ([live:1253–1291](../../apps/megamail/src/live.rs#L1253)). This violates the pending-send/Outbox contract and also couples outgoing SMTP availability to incoming IMAP availability. Local Outbox operations are unnecessarily gated on IMAP.

**Repro to add:** synthetic account whose IMAP connect deterministically fails; submit a correlated send through the actual cache-lane/worker seam with a temporary SQLite cache and fictional attachment. Require exactly one terminal send completion and durable recoverable content. Exercise local Outbox list/discard while offline. No real provider or private profile is needed. The immediate policy can queue rather than attempt SMTP; independent SMTP send while IMAP is down is a later explicit decision.

**C2 — P0: direct IMAP cache identity lacks UIDVALIDITY.**

Message/body/attachment tables key by `(account_id, folder_path, uid)` ([cache schema:39–96](../../src/cache.rs#L39)); `sel` returns mailbox metadata without validating an epoch ([worker:5893](../../src/worker.rs#L5893)). `load_messages` observes UIDNEXT but merges cached rows by UID ([worker:7407–7446](../../src/worker.rs#L7407)). There is no `uid_validity`/`uidvalidity`/`UIDVALIDITY` handling in inspected `src/worker.rs`, `src/worker/imap_io.rs` or `src/cache.rs`. Cached-body requests can be answered without network validation ([worker:1004](../../src/worker.rs#L1004)). After a server resets a folder's UIDs, a reused UID can address a different message while an old cached body/attachment still answers it. This is an identity/invalidation defect, not proof that a particular user's server reset UIDs.

RFC 9051 defines the UID + UIDVALIDITY relationship ([§2.3.1.1](https://www.rfc-editor.org/rfc/rfc9051.html#section-2.3.1.1)). The existing library already exposes `Mailbox::uid_validity` and `highest_modseq` ([async-imap 0.10.4 metadata](https://docs.rs/async-imap/0.10.4/async_imap/types/struct.Mailbox.html)); this gap does not require a protocol-parser rewrite. Thunderbird detects changed validity and invalidates folder state ([`nsImapMailFolder.cpp::UpdateImapMailboxInfo`, current source around 2548](https://github.com/mozilla/releases-comm-central/blob/master/mailnews/imap/src/nsImapMailFolder.cpp#L2548)). Borrow that invariant, not its entire implementation.

**Required follow-on:** persist mailbox epoch; atomically invalidate all per-copy tables and native pending/reader state when epoch changes; prevent in-flight old-epoch cache writes after invalidation. A migration must treat old caches as epoch-unknown, allow labeled offline reading, and require online epoch validation before mutation or presenting cached content as current. Do not claim a folder resync alone fixes bodies.

**C3 — P1: Thunderbird request admission discards queue-full vs queue-closed distinction.**

TB `try_send` maps Full to false and Disconnected to false plus an error event ([adapter:1514–1533](../../apps/megamail/src/thunderbird_adapter.rs#L1514)); `LiveMailbox::send_request` maps false to “The mail worker is unavailable” ([live:1645](../../apps/megamail/src/live.rs#L1645)). Full is transient overload, not worker death. Page submissions already preserve Full/Closed ([adapter:3453](../../apps/megamail/src/thunderbird_adapter.rs#L3453), [query:56](../../crates/mail-core/src/query.rs#L56)). A busy shared profile queue therefore gives a misleading operational state and no distinct overload policy for ordinary actions.

**Minimal repair:** use typed admission errors for native action senders, preserve the request on rejection where retry/recovery needs ownership, and distinguish Busy from Closed in UI. Never auto-retry send/move on ambiguous completion. Do not change GTK's worker return type in this slice.

**C4 — P1: Thunderbird body truncation is not represented in the native reader contract.**

The extension truncates a combined plain/HTML budget and returns `truncated: true` ([background.js:644–666](../../tools/thunderbird-bridge/extension/background.js#L644)). `parse_body_payload` ignores this flag ([adapter:2224–2267](../../apps/megamail/src/thunderbird_adapter.rs#L2224)); `BodyPayload`/Body events carry no completeness warning ([adapter:187](../../apps/megamail/src/thunderbird_adapter.rs#L187), [live body events](../../apps/megamail/src/live.rs#L237)). A successfully parsed prefix can be displayed as if complete. Carry a typed completeness warning to the reader; keep the original explicit raw/attachment download option. Testing should include multibyte truncation and a large plain+HTML multipart body.

**C5 — P1: provider string IDs are reduced to a collision-prone 31-bit message key with no collision detection.**

`hash_uid` masks SipHash to 31 bits ([worker:9282–9292](../../src/worker.rs#L9282)). Graph/JMAP map headers through it and use `HashMap<u32, remote_id>` ([graph:270,364](../../src/worker/graph.rs#L270), [jmap:556,74](../../src/worker/jmap.rs#L556)). Two different IDs with equal hashes can overwrite mappings or cache rows, potentially routing a later body/mutation to the wrong remote item. A particular collision was not observed. This is a provably lossy identity design; uniform-hash probability rises with mailbox size but is not a measured incidence rate. Use persisted collision-checked surrogate allocation or preserve opaque remote IDs behind a newtype. Pinning a hash algorithm does not make it unique. Existing POP3 UIDL hashing deserves the same follow-up. Preserve or explicitly migrate existing cache keys; do not silently change the hash.

**C6 — P1 before native OAuth launch: callback deadline and success-page behavior are unsafe operationally.**

`wait_for_code` switches an accepted stream to blocking then reads without a per-stream timeout; the five-minute deadline is checked only when `accept` would block ([oauth:735–804](../../crates/mail-core/src/oauth.rs#L735)). A local connection that sends no request can hold the flow beyond the documented deadline. The browser success response is written before `state` validation and before code exchange/keyring persistence ([oauth:772–791](../../crates/mail-core/src/oauth.rs#L772)). An invalid callback may see a false success page even though the function rejects it. These do not demonstrate authorization bypass: state checking still exists. Add bounded callback reads, validated request method/path, cancellation, and an honest “authorization received; finishing connection” page until account persistence succeeds.

**C7 — P1 for JMAP exposure: per-item mutation rejections can be treated as success.**

`jmap_set_emails` logs `notUpdated`/`notDestroyed` then returns `Ok` ([jmap:710–735](../../src/worker/jmap.rs#L710)); `jmap_patch_all` returns successful unit result without converting those failures ([jmap:738](../../src/worker/jmap.rs#L738)). RFC 8621 inherits per-item `/set` results ([§4.6](https://www.rfc-editor.org/rfc/rfc8621.html#section-4.6)). Surface partial mutation results and preserve failed items. This is relevant to existing code, although native onboarding cannot currently add a JMAP account. A method-level HTTP/JSON success is not every-item success.

### Suspected bugs / robustness risks requiring a feedback loop

**S1 — P0: uncertain direct sends may duplicate on retry.** `SendOutcome` has Sent/Queued/Failed, not Unknown ([worker:653](../../src/worker.rs#L653)); `send_failed` queues non-refusal errors for later automatic sending ([worker:4304–4346](../../src/worker.rs#L4304)); reconnect triggers Outbox flushing ([worker:1463](../../src/worker.rs#L1463)). An SMTP disconnect after acceptance but before acknowledgement can therefore queue a possibly delivered message. TB already has conservative `SendFailure::Uncertain` and native `SendUnknown` handling ([adapter:2905–2981](../../apps/megamail/src/thunderbird_adapter.rs#L2905), [live:2494](../../apps/megamail/src/live.rs#L2494)). No SMTP fault-injection reproduction was run. Add a local SMTP scenario accepting DATA then closing before the final reply; persist an Unknown outcome and require user review rather than retrying automatically. Request IDs alone do not establish exactly-once delivery. Inspect Graph/JMAP ambiguity independently; an HTTP timeout after server acceptance has the same conceptual risk.

**S2 — repeated import may actually be a restoration/error-state failure.** Restoration and stable manifest IDs already exist ([adapter import persistence:543](../../apps/megamail/src/thunderbird_adapter.rs#L543), [restore:600](../../apps/megamail/src/thunderbird_adapter.rs#L600), [live:733](../../apps/megamail/src/live.rs#L733), [startup:905–954](../../apps/megamail/src/main.rs#L905)). Startup uses `saved_thunderbird.unwrap_or(false)`, which hides a manifest-inspection error when choosing Mailbox versus Setup. Failed/locked/moved source profiles and failed private-clone auth can produce no started account rows; verify which UI the user then sees. Source manifest write failure is returned, not silently treated as persistent success ([adapter:560](../../apps/megamail/src/thunderbird_adapter.rs#L560)). Do not implement another automatic import or re-copy credentials on every launch. Test missing/malformed/versioned manifest, source moved, login required, child crash, disconnected clone, and direct-config load failure. Keep saved account identity visible with Retry/Sign in/Repair source; successful restore should not show import choices as the required next step.

**S3 — shared TB actor head-of-line blocking.** Metadata pages call `Runtime::call("list")` synchronously inside the profile actor ([adapter:1545–1646](../../apps/megamail/src/thunderbird_adapter.rs#L1545)); send/draft/attachments/mutations also execute from that actor's request handler. Bodies/conversations/refresh jobs have separate threads, but their completions still return to the actor ([adapter:2101](../../apps/megamail/src/thunderbird_adapter.rs#L2101)). A slow list or mutation can delay unrelated accounts' results in the same profile. Measure queue age and time-to-selected-body while another account operation stalls. Introduce bounded jobs/priority lanes only if needed; do not violate mutation order to chase latency.

**S4 — no explicit common lifetime/epoch contract.** Query results have generations; TB cursors and synthetic message IDs are session-local; direct events largely have account/message/folder IDs but no backend-session epoch ([query:20](../../crates/mail-core/src/query.rs#L20), [live:59,203](../../apps/megamail/src/live.rs#L59), [adapter:281](../../apps/megamail/src/thunderbird_adapter.rs#L281)). A late event after account replacement/reconnect needs a deterministic rejection rule beyond matching account IDs. The TB global UID allocator helps avoid immediate collisions but is not a common request-lifecycle contract. Test replacement while body/page/send jobs are pending; draining/canceling must not lose or repeat mutations.

**S5 — unbounded direct/event queues, spawned TB jobs, and resource cleanup.** Direct request/cache/summary lanes and the UI event queue are unbounded ([worker:900,978](../../src/worker.rs#L900), [live:458](../../apps/megamail/src/live.rs#L458)); TB has a bounded command queue but starts separate threads for body jobs. Queue boundedness alone does not bound all in-flight work. Direct workers have no common explicit close/join handle. Channel drop is an implicit lifetime mechanism; test account removal and app replacement while LocalSet watchers/background jobs remain. TB has bounded child cleanup and profile ownership but shares runtime across account handles ([bridge:340](../../crates/mail-core/src/thunderbird_bridge.rs#L340)). Closing one account must not kill another account's process. Collect thread/process/queue counts before consolidating runtimes.

**S6 — cached/decrypted-content policy differs across adapters.** Direct code excludes decrypted bodies from persistent cache via `body_cacheable` ([worker:4068–4078](../../src/worker.rs#L4068)). TB body extraction uses `getFull` and native `BodyPayload` has no encrypted/decrypted/cache-policy marker ([background.js:626](../../tools/thunderbird-bridge/extension/background.js#L626), [adapter:187](../../apps/megamail/src/thunderbird_adapter.rs#L187)). Its RAM cache and UI reader cache should respect explicit sensitivity/persistence policy. This audit does not establish that TB returns decrypted plaintext for an actual imported encrypted message, nor that it persists that plaintext. Use synthetic encrypted fixtures; document cached RAM, persistent stores and process lifetime separately.

**S7 — page/order/limit differences are hidden in reused types.** Direct cache pages use a `before_uid` keyset; TB stores an opaque remote cursor behind that field and sends date-sorted headers ([cache:279,878](../../src/cache.rs#L279), [adapter:1667](../../apps/megamail/src/thunderbird_adapter.rs#L1667), [background.js:544](../../tools/thunderbird-bridge/extension/background.js#L544)). Non-search TB lists do not pass the requested limit into `messages.list`, relying on TB paging behavior, while the adapter rejects responses above request.limit. Test varied TB page-size defaults and custom small limits; do not claim this already fails on the current private clone. Make cursor/session/order explicit rather than interpreting opaque tokens as IMAP UIDs.

## 4. Already implemented: preserve, do not duplicate

- MegaMail profile/credential isolation and stable direct profile IDs are present in core config; shared root module edits must preserve GTK. TB source discovery is read-only; bridge builds an allowlisted clone with private mail paths, source mail files excluded and encrypted credential-store copies handled by TB ([architecture](../MEGAMAIL_ARCHITECTURE.md), [bridge prepare/copy](../../crates/mail-core/src/thunderbird_bridge.rs#L617)). No credentials were inspected here.
- Atomic private manifest/config write uses temp-file, file sync, rename and Unix directory sync ([config:68–125](../../crates/mail-core/src/config.rs#L68)). Do not replace it with a direct JSON overwrite. Audit multiprocess manifest read-modify-write separately if that use case is enabled.
- Native bounded cache pages, query coalescing/generations, per-account Unified Inbox and successful-account results during partial failure already exist. Full worker batches are compacted into FolderChanged before entering the UI queue ([live:4164–4228](../../apps/megamail/src/live.rs#L4164)); paging does **not** prove worker sync is bounded.
- Direct cache-first body/attachment lane, WAL readers, reconnect-on-read logic, IO timeouts and reader-over-refresh cancellation are present ([worker:896–1070](../../src/worker.rs#L896), [read retry](../../src/worker.rs#L3073), [interrupt helper](../../src/worker.rs#L1251), [IO module](../../src/worker/imap_io.rs#L15)). Mutation ordering must survive any refactor.
- TB bridge multiplexes responses by ID, ignores expired replies, detects transport breakage, times out operations and cleans up child/artifacts ([bridge:273–483](../../crates/mail-core/src/thunderbird_bridge.rs#L273)). This is not a naive blocking stdin/stdout round-trip implementation. Its String error semantics still leak into substring classifiers ([adapter:3775](../../apps/megamail/src/thunderbird_adapter.rs#L3775)).
- Direct send completions correlate request IDs; Queued is tied to successful SQLite insert; envelope/raw bytes preserve Bcc/attachments on retry ([worker:4295](../../src/worker.rs#L4295), [queue construction](../../src/worker.rs#L4384), [cache:1982](../../src/cache.rs#L1982)). Preserve these while repairing C1/S1.
- Direct provider OAuth code already has browser/PKCE/state/token exchange, refresh rotation and access-token reuse, but the default registrations are empty ([oauth:83](../../crates/mail-core/src/oauth.rs#L83), [worker:5618–5691](../../src/worker.rs#L5618)). That is reusable implementation, not shipped Google login.
- Sanitized native reader HTML, explicit attachment downloads, selected-body retention and bounded reader cache are present; TB has a separate 16 MiB/8-entry BodyCache ([adapter:205](../../apps/megamail/src/thunderbird_adapter.rs#L205)). `src/ram_cache.rs` is a reusable legacy FIFO byte cache, **not** the TB LRU or the native reader cache. It intentionally permits one oversized newest entry ([ram_cache:74–88](../../src/ram_cache.rs#L74)); that is not a strict hard memory cap.

Verification evidence remains scoped: `PRODUCT.md`/architecture report a six-account authorized TB read probe and TB 157 loopback send/draft checks, not direct live-provider verification or provider-wide compatibility. The real-account runtime test is explicitly ignored and authorization-gated ([test](../../apps/megamail/tests/thunderbird_runtime.rs#L37)). Nothing was rerun here.

## 5. Duplication and costs: what to consolidate, what not to

### Duplicated lifecycle/request/state logic

1. **Admission:** bool direct action sender, bool TB action sender, typed page senders, special direct QueryService fallback. Consolidate typed native admission and request ownership; do not invent another worker channel layer unless it adds boundedness/completion guarantees.
2. **Result state:** `WorkerEvent`, projected `WorkerMailboxEvent`, `MailboxEvent`, TB bridge JSON errors, TB account outcomes and native action/connectivity/query error maps. Retain the legacy event type; make one native projection/engine-event contract and a typed TB transport error internally. Avoid mapping human text back into operational states.
3. **Identity:** IMAP UID/per-folder keys, provider ID hashes, TB session surrogate IDs, folder IDs and native physical/message/conversation keys. These are different concepts. Define account ID, mailbox identity/epoch, backend item reference, Message-ID header and session reference as separate types. No subject-only grouping and no Message-ID-only mutation routing.
4. **Refresh/reconnect:** direct IDLE/watchers/read retries/backfill; TB 60-second refresh jobs/runtime retirement; `LiveMailbox` reconnect membership and restore logic. Protocol liveness stays with adapters. Durable account presence and UI status should be common; a network fault must not change the chosen backend or delete account metadata.
5. **Body processing/caching:** direct renders raw MIME and excludes decrypted disk cache; TB extracts body via extension then sanitizes; native reader prepares presentation formats again. Share privacy/completeness outcomes, not raw opaque protocol stores. Keep a single canonical sanitized representation where feasible after measurements.
6. **Send/draft states:** separate pending sends, TB Unknown lock, direct Outbox outcomes, draft-save events with optional autosave IDs. Completion contract should cover all admitted operations even if backend-specific submission or draft behavior differs.

### Known cost centers, not performance findings

| Path | Work visible in code | Measurement needed |
|---|---|---|
| Direct IMAP refresh | Full cached folder vector + recent fetch + full UID SEARCH set + merge/deletion reconciliation, despite paged GPUI ([worker:7407](../../src/worker.rs#L7407)); summary cache writes are transactional ([cache:1028](../../src/cache.rs#L1028)) | CPU/allocation peak at 1k/10k/100k rows, UID search bytes, transaction/busy duration and selected-body latency during refresh |
| Direct cached body | Cache lane SQLite lookup, repeated miss in worker, render/sanitization projection on hits, event delivery | Cache-hit latency during sync, repeated sanitize cost, encrypted exclusion correctness |
| TB list/metadata | TB database/protocol work → extension object → native-messaging JSON → host/socket frames → Rust Value → domain types; `route_response` clones result subtree ([bridge:424](../../crates/mail-core/src/thunderbird_bridge.rs#L424)) | Per-stage time/bytes/allocations; do not assign all latency to serialization |
| TB body | `getFull`, `getHeaders`, `listAttachments` concurrently; source cap 20 MiB, output 2 MiB combined; parse/sanitize; clone for cache + event; another payload clone on cache get and event ([adapter:205,255,2220](../../apps/megamail/src/thunderbird_adapter.rs#L205)) | Actual retained bytes, HTML expansion, cache hit time, allocation profile; evaluate Arc payloads only if material |
| TB binary transfer | 512 KiB chunks, base64 encode/decode and JSON framing; response/upload limits; two process transports ([adapter:3098,3175](../../apps/megamail/src/thunderbird_adapter.rs#L3098), [background.js:693–797](../../tools/thunderbird-bridge/extension/background.js#L693)) | Throughput/peak memory at 1/10/25 MiB, 20 attachments, incomplete transfer cleanup; base64 expansion is known, overall speed is not |
| Native reader | `ReaderBody::new` materializes plain/quote/HTML variants ([main:610–664](../../apps/megamail/src/main.rs#L610)); separate cache from TB BodyCache | Combined retained bytes across both caches plus selected body, work on UI event application, long-message interaction latency |
| Direct concurrency | Per-account cache/mail/summary threads + Tokio tasks/IMAP watches + shared query lane | Threads/FDs/RSS and startup at 1/6/20 accounts; distinguish readers from SQLite writers |

Do not normalize away these differences to make a benchmark look favorable. A direct cache hit versus a cold Thunderbird clone is not an engine comparison. UI memory bounds do not prove bounded worker memory. WAL permits concurrent readers but does not eliminate writer contention. Legacy FIFO, TB LRU and native reader cache have different policies and representations; identical-looking caches are not automatically redundant.

## 6. Thunderbird architecture: useful inspiration and actual constraints

First-party code, retrieved during this audit:

- [IMAP protocol implementation](https://github.com/mozilla/releases-comm-central/blob/master/mailnews/imap/src/nsImapProtocol.cpp): C++ protocol/session/IO handling, capabilities, chunked fetches and threading integration. Thunderbird is not merely a Rust library that MegaMail can relink behind a tiny trait.
- [IMAP folder implementation](https://github.com/mozilla/releases-comm-central/blob/master/mailnews/imap/src/nsImapMailFolder.cpp#L2500): reconciles folder metadata/UIDVALIDITY/highest MODSEQ and invalidates obsolete UID state. The invariant belongs in the direct implementation too.
- [Folder interface](https://github.com/mozilla/releases-comm-central/blob/master/mailnews/base/public/nsIMsgFolder.idl): account/folder/storage/permissions contracts; the backend should not infer every operation from folder names.
- [Message summary database](https://github.com/mozilla/releases-comm-central/blob/master/mailnews/db/msgdb/src/nsMsgDatabase.cpp): MDB/Mork summary structures and caches. [Gloda](https://github.com/mozilla/releases-comm-central/blob/master/mailnews/db/gloda/modules/Gloda.sys.mjs#L130) separately initializes a SQLite global datastore/index. “Thunderbird cache” is not one interchangeable database.
- [MailExtension message implementation](https://github.com/mozilla/releases-comm-central/blob/master/mail/components/extensions/parent/ext-messages.js): privileged JS wraps Thunderbird message/folder machinery. [Published messages contract](https://webextension-api.thunderbird.net/en/latest/messages.html) explicitly says internal `messageId` does not persist across restart or follow a moved message; do not persist it as server identity. Lists require continue/abort handling.
- [OAuth provider registry](https://github.com/mozilla/releases-comm-central/blob/master/mailnews/base/src/OAuth2Providers.sys.mjs#L218): its comment explicitly instructs downstream applications to register their own OAuth clients, not copy Thunderbird's values.

**Borrow:** durable account identity independent of live connectivity; mailbox epoch invalidation; explicit cache/index/store roles; cache-first reads; incremental change state where server-supported; asynchronous event projection; capabilities/permissions and actionable auth states.

**Do not borrow wholesale:** Gecko/XPCOM internals, credential contents, Mork database layout, provider client registrations, or the assumption that a folder list operation logs in/syncs automatically. The current bridge has an experimental privileged sync extension and version floor; it needs API/version compatibility checks and tests, not a claim that all TB functionality is automatically inherited. Preserve installed-binary 153+ requirements and the tested 157 compose fallback while checking newer runtime APIs ([extension send fallback](../../tools/thunderbird-bridge/extension/background.js#L868)).

Keep the existing isolated profile path. Headless primary-password/provider-prompt recovery is **new work**, not a case a background retry can solve. A user-visible isolated-runtime authentication flow needs a threat model and explicit UX; it must not launch or modify the source profile behind the user’s back.

## 7. Realistic direct-engine/provider choices and login

| Choice | Official evidence | Recommendation / limits |
|---|---|---|
| Existing `async-imap` + lettre + mail-parser | [async-imap upstream](https://github.com/chatmail/async-imap), [lettre](https://lettre.rs/), local [dependencies](../../crates/mail-core/Cargo.toml#L17) | Keep first. MegaMail already has IO timeouts, IDLE, reconnect, TLS/XOAUTH2 and MIME/send logic. Verify pinned 0.10 behavior rather than copying a latest-doc example. A protocol crate does not supply offline indexing, account lifecycle or durable mutations. |
| `imap-codec` / `imap-types` + `imap-next` | [codec upstream](https://github.com/duesee/imap-codec), [sans-IO flow upstream](https://github.com/duesee/imap-next) | Types/parser/serializer and AUTH/literal/IDLE flow building blocks, not a complete mail-account engine. Consider only for a measured parser/flow interoperability problem, behind an internal seam with corpus/replay coverage. No from-scratch rewrite recommendation. |
| Existing JMAP path; optionally maintained Rust client | [RFC 8620](https://www.rfc-editor.org/rfc/rfc8620.html), [RFC 8621](https://www.rfc-editor.org/rfc/rfc8621.html), [Stalwart jmap-client](https://github.com/stalwartlabs/jmap-client), [Fastmail protocols](https://www.fastmail.com/dev/) | Existing worker already batches requests and supports push. First fix identity/per-item errors and assess 300-row/full-history limitations. RFC changes/queryChanges/state/capability limits support efficient sync semantics, not automatic performance. A maintained client may reduce wire-shape code, but still needs cache/state/resync policy. Use only for providers that actually advertise mail/submission capabilities. JMAP is not a general Gmail/Microsoft replacement. |
| Gmail API | [sync guide](https://developers.google.com/workspace/gmail/api/guides/sync), [scopes](https://developers.google.com/workspace/gmail/api/auth/scopes) | New provider adapter, not currently implemented. `historyId` supports partial sync; history expiry returns 404 and requires full resync. Provider message/thread/label semantics can reduce mapping work, but quotas, pagination, retries and data handling remain. Evaluate after login and cache correctness, not as an assumed faster choice. |
| Existing Microsoft Graph path | [message delta](https://learn.microsoft.com/en-us/graph/delta-query-messages), [immutable IDs](https://learn.microsoft.com/en-us/graph/outlook-immutable-id) | Reuse existing worker before introducing another Graph implementation. Persist opaque per-folder next/delta links and respect scopes/policies; optional ImmutableId header changes ID semantics and requires migration. Current source handles move-ID changes but uses capped polling/list windows, not durable delta history. |
| Thunderbird backend | [messages contract](https://webextension-api.thunderbird.net/en/latest/messages.html), local bridge/adapter above | Keep for accounts already handled by TB and as an explicit compatibility option. Authentication stays inside TB runtime. Do not claim broad OAuth onboarding or full source mail import. Benchmark runtime overhead and compatibility separately. |

### Real Google/other-provider sign-in: launch dependency, not a widget

1. Register **MegaMail-owned** desktop clients. No borrowed TB/other app IDs. Google supports installed desktop loopback callbacks and PKCE using a system browser ([official native flow](https://developers.google.com/identity/protocols/oauth2/native-app)). Distributed desktop app client metadata does not provide a confidential secret boundary.
2. Gmail IMAP/SMTP uses XOAUTH2 with `https://mail.google.com/`, a broad scope requiring public-app verification/policy compliance ([mechanism/scopes](https://developers.google.com/workspace/gmail/imap/xoauth2-protocol)). Gmail API permits narrower scopes, but several relevant scopes remain restricted; [Google scope documentation](https://developers.google.com/workspace/gmail/api/auth/scopes) states additional assessment requirements when restricted data is stored/transmitted on servers. Do not claim every local-only desktop app invariably needs a paid assessment; classify the actual architecture and verification requirements.
3. Reuse core OAuth/worker refresh code after repairing C6. Native setup currently calls password-only `AccountForm` conversion and rejects OAuth candidates for that path ([form](../../apps/megamail/src/onboarding.rs#L101), [core validator](../../crates/mail-core/src/onboarding.rs#L261)); `oauth_settings` exists but does not by itself provide a native sign-in UI. Microsoft presets use Graph scopes ([oauth:264](../../crates/mail-core/src/oauth.rs#L264)); do not pair them with an IMAP-only account inadvertently.
4. Implement native auth as a cancellable attempt with attempt ID, account identity verification, verified TLS/provider endpoints, loopback/state/PKCE checks, token-exchange deadline, keyring persistence/rotation, revoked-token/consent-denied states and account save **only after success**. Test locked/unavailable keyring without plaintext fallback.
5. Provider discovery is server metadata and authentication hints, not authorization. Arbitrary IMAP providers may require app passwords, manual TLS settings, enabled IMAP or a local bridge. Keep truthful password/manual setup and explicit unsupported OAuth guidance.
6. A saved account remains visible through auth expiration. “Needs sign-in” must offer Repair/Sign in, not “import again.” An auth retry is not a backend migration.

### Broad native gaps that depend on foundations

`PRODUCT.md` and architecture explicitly separate current native behavior from legacy Hylki capability. New work includes real native Google/Microsoft sign-in; TB interactive recovery and native TB Outbox controls; full server-history/all-folder/body search; richer composition/inline reply/bulk triage; broader provider-policy coverage and runtime verification. Existing Unified Inbox, scoped conversation grouping, single-message actions, native sanitized reader, attachments, settings and direct Outbox should not be recounted as missing.

The direct Graph/JMAP 300-header caps mean server-history completion should be explicit if those protocols become addable. Reference-based related-message bounds should remain labeled partial, not marketed as exhaustive threading. Search/triage/send-later/AI/team features associated with Thunderbird/Spark/Superhuman are product tracks after correctness/auth; this focused engine audit does **not** certify exhaustive competitor parity or competitor-specific current feature claims.

## 8. Interface design: small method count versus capability-driven flexibility

These are **proposed sketches**, not compiling code or implemented symbols.

### A. Deep, three-method native account engine (recommended target)

```rust
// Constructed off-UI with immutable account binding, epoch and event sink.
// Handle is cheap to clone; implementation lifetime is owned explicitly.
trait AccountEngine: Send + Sync {
    fn descriptor(&self) -> EngineDescriptor;
    fn submit(&self, request: EngineRequest)
        -> Result<RequestId, RejectedRequest>;
    fn close(&self) -> CloseTicket;
}

enum EngineRequest {
    Page(PageQuery),
    Mail(MailOperation), // initially adapts a reviewed subset of MailRequest
}
// EngineDescriptor: BackendKind, account ID, epoch, capability/limit snapshot.
// RejectedRequest: Full | Closed | Unsupported | Invalid, plus owned request.
// CloseTicket is observed asynchronously; close does not block GPUI.
// EngineEvent: account + epoch + request ID + progress/terminal/delta payload.
```

Illustrative usage:

```rust
let engine = open_account(binding, event_sink)?; // Direct or TB, never UI I/O
let request = EngineRequest::Page(PageQuery::first(folder, 100, generation));
let id = engine.submit(request)?;
// Later, on UI thread:
if event.epoch == active_epoch && page_generation_matches(&event) {
    apply_event(event); // admitted id eventually has a terminal outcome
}
// Never await process shutdown on the UI thread:
let closing = engine.close();
```

**Adapters:** DirectAccountEngine wraps `worker::spawn`, native projection, and a shared QueryService handle; ThunderbirdAccountEngine wraps the profile actor/account binding and its TB page path. TB account handles share a runtime owner; releasing/closing one account must detach it, not kill the runtime for other handles. Engine construction/account repository manages setup; `submit` never launches processes or opens a SQLite database on GPUI.

**Depth comes from contracts, not three methods:** nonblocking admission; one terminal outcome for each accepted finite request; immutable account/session binding; per-mailbox freshness and opaque cursors; bounded queue/job policy; cancellation of obsolete reads; FIFO mutation barriers; explicit privacy/completeness; transport-independent Busy/NeedsLogin/Offline/Unsupported/Unknown states; durable send state separated from recipient delivery. Background deltas are not mistaken for request completion. Request cancellation after a send is accepted does not mean remote sending stopped.

**Hidden complexity / tradeoffs:** a broad command enum can disguise a huge interface. Callers still learn operation payloads, result variants and capability semantics. Default behavior must reject unsupported operations, not silently succeed. A bool-returning wrapper does not provide exactly-one completion. Retrofitting request IDs to legacy events must distinguish unsolicited refreshes from completion; ambiguous matches cannot be guessed. Construction and close are lifecycle work, not a forced unified Tokio runtime. Keep the constructor separate from the 1–3 instance methods, and document it as part of the full interface.

**Compatibility:** add new native types under mail-core without altering existing `src/backend.rs::MailBackend`, `worker::spawn` or GTK event consumers initially. Use an internal legacy adapter/projection. Move TB domain scheduling into core only after the new native event types no longer depend on `crate::live`; do not move GPUI entities into core. Keep Hylki/MegaMail namespaces and source-manifest schema unchanged in the first slices. No silent Direct↔TB fallback on auth/network failure: a migration can change ID/cache/send semantics and risks duplicate mutations.

### B. Flexible capability-driven shape

```rust
trait MailAccount {
    fn capabilities(&self) -> Capabilities;
    fn execute(&self, op: CapabilityOperation)
        -> OperationTicket;
    fn close(&self) -> CloseTicket;
}
// Capabilities are typed: SearchScope, OfflineReadPolicy, ChangeFeedKind,
// DraftSupport, SubmissionSupport, MaxPage/Body/Attachment limits, etc.
// OperationTicket reports rejected/accepted plus typed eventual outcomes.
```

Usage:

```rust
let caps = account.capabilities();
match caps.search_scope {
    SearchScope::LoadedHeaders => show_loaded_only_search(),
    SearchScope::RemoteMailbox => account.execute(remote_search(query)),
    SearchScope::AllCachedAndRemote => account.execute(full_search(query)),
}
if caps.submission.schedule == ScheduleSupport::DurableLocal {
    offer_send_later();
} // TB currently rejects MegaMail send_at rather than implementing this
```

A larger alternative exposes separate `MailboxRead`, `Mutation`, `Submission`, `Search` and `ChangeFeed` typed facets. That is more extensible for JMAP/Graph/EWS-like semantics but increases caller-visible concepts and object lifetime contracts. Avoid `supports("whatever")` strings and a generic JSON RPC interface leaking TB method names into UI.

**Contrast:** A centralizes common behavior and locality; B accurately models variable operations/rights and prevents offering unsupported features. Capabilities must describe semantic differences (queued durability/uncertain send/draft location/search completeness), not just booleans. B risks scattering policy branches through every caller and turning the module shallow. Prefer A with a small typed capability snapshot as descriptor data now; grow facets only when a real adapter/caller needs them. Capability changes after authentication or permission refresh should arrive as events; folder-level ACLs cannot be represented solely by one static account boolean.

### Minimal first extraction, not the whole target in one PR

Use three native handle methods `request(MailRequest) -> Result<(), NativeSubmitError>`, `page(PageRequest) -> Result<(), SubmitError>`, `descriptor() -> Descriptor` as a compatibility landing point. Direct wraps existing worker + shared QueryService; TB wraps existing sender closures; both preserve events. This removes duplicate dispatch and corrects C3 without claiming full lifecycle depth. Require queue Full/Closed tests and native-call-site simplification; then add IDs/epochs/close in separate slices. A method-count slogan must not obscure the remaining contracts.

## 9. Small migration slices and implementation ownership

### First narrow slice: complete accepted offline direct operations

**One owner:** `src/worker.rs` (cache lane + `run_imap` offline path), regression tests colocated there; tiny test seam/helper only if required to inject deterministic connect failure. Prefer temporary SQLite/fictional loopback fixtures; **do not run existing private-provider ignored tests**. `src/cache.rs` is a consulted dependency, not expected production-edit ownership for this slice. Parent holds the serial Cargo slot and validates GTK plus native callers.

Acceptance:

1. With IMAP unreachable, correlated `Send` receives exactly one `SendFinished` as Queued if SQLite persistence succeeds, otherwise Failed plus recoverable outgoing content. Persist raw/envelope/Bcc/attachments using the existing queue routine; do not report Queued before insertion succeeds.
2. Reject/defer or classify sends independently of IMAP state; no new provider registration or TB change. The minimal safe policy is offline queueing, not speculative automatic SMTP retry.
3. `LoadOutbox` works from local cache while IMAP is down; `DeleteOutbox` updates local state without reconnecting. No unauthenticated server mutation occurs.
4. IMAP-down scheduled sends preserve their existing durable scheduling semantics; no draft deletion when scheduling fails.
5. An offline cache-miss body request reports an appropriate terminal failure rather than leaving a native pending reader indefinitely. If this needs event-contract changes, keep it as the next slice rather than inflating the send patch.
6. GTK continues receiving legacy events; native pending-send logic clears its exact request. Test failed cache open/insert and a superseded edited Outbox message so repairs do not cause silent loss or duplication.

### Follow-on slices, independently reviewable

| Order | Slice | Exclusive file owner(s) | Gate / compatibility |
|---|---|---|---|
| 2 | Typed native admission, including Busy versus Closed; then narrow handle extraction | `apps/megamail/src/live.rs`, `apps/megamail/src/thunderbird_adapter.rs`; a new native engine/types module after agreeing placement | Reject queue-full actions without marking account disconnected; preserve owned request/composer; no GTK API change. Both existing adapters remain selectable. Avoid concurrent editors of live.rs. |
| 3 | Saved account restoration/repair state | `main.rs` startup/setup integration, adapter manifest/restore code | Restart synthetic saved manifests; no required re-import; persisted account identity visible on failures; malformed manifest error visible; old manifests read unchanged. Coordinate with slice 2 owner because adapter overlaps. |
| 4 | Direct UIDVALIDITY + collision-safe provider references | `src/cache.rs`, `src/worker.rs`, later `src/worker/graph.rs`/`jmap.rs` | Separate mailbox-epoch migration from provider-ID migration. Old caches have explicit unknown epoch. Atomic invalidation of every per-copy table and stale write rejection; sibling-copy fallback cannot bypass validation. GTK compatibility and isolated migration fixtures. |
| 5 | Unknown-send journal/result | `src/worker.rs`, `src/cache.rs`, native send projection/UI, optionally compose state | Local SMTP acceptance/ack-loss tests; persisted Unknown survives restart; no auto-flush of Unknown. Recipient delivery never equated with server acceptance. Do not batch with UID migration. |
| 6 | TB body completeness/privacy | extension `background.js`, `thunderbird_adapter.rs`, native reader event/UI | Truncation warning; invalid frame/body limits fail closed; no remote resources; encrypted fixture policy. Extension version/capability compatibility check. |
| 7 | OAuth native provider flow and callback hardening | `crates/mail-core/src/oauth.rs`, core/native onboarding, provider settings UI | Parent obtains registrations outside code review; synthetic OAuth callback/token/keyring suite; no credentials committed; cancel/revoke/retry. Provider verification tracked separately from code. |
| 8 | Common IDs/epochs/lifecycle and bounded jobs; move TB scheduling into core | new engine module, `lib.rs`, native adapter/live projection, bridge only if necessary | Every admitted finite request terminates; stale epoch drops; no UI joins/I/O; close one TB account preserves others; app close releases processes/locks; no automatic backend conversion. |
| 9 | Incremental history/index and measured optimizations | direct protocol-specific modules, cache/query, TB scheduling only where trace identifies cost | Delta expiry/full resync, page ordering/completeness, quotas/rate limits and benchmark comparison. Preserve Thunderbird fallback as explicit account choice. |

Suggested fanout after parent planning: independent research/test-design or implementation owners for **OAuth callback/core flow**, **TB body completeness/extension**, **direct worker correctness**, and **bench instrumentation**. These are not independent if all touch `live.rs`, `main.rs` or `cache.rs`; serialize those merge/edit points. Do not launch Cargo from child agents. No implementation fanout was started by this research sub-agent.

## 10. Benchmarks and robustness gates needed

Benchmarks must use optimized builds prepared by the parent under its serial Cargo slot; this audit ran none. Record binary commit/build profile, dependency/runtime versions, dataset, filesystem/compositor/hardware, account count, cache state, network RTT/bandwidth, server configuration and public synthetic-fixture seed. Compare existing direct versus TB on the same loopback IMAP/SMTP server and same messages, then separately run explicitly authorized read-only provider trials. Do not use demo data as evidence of live sync performance.

**Matrix:** 1/6/20 accounts; 1k/10k/100k headers; cold start/first-ever sync/warm restart/offline reopen; 100-row first page and 50-page scrolling; body 10 KiB/1 MiB/2 MiB plus oversize MIME; 1/10/25 MiB attachments; nested/malformed MIME and encrypted mail; Unified Inbox with one delayed/failing account; auth expiry and reconnect bursts. Keep page completeness, body content and privacy behavior equivalent.

**Metrics:** startup→account-visible→first header page→usable body; p50/p95/p99 click-to-reader and input delay during sync; per-stage queue age/depth, in-flight jobs, canceled/coalesced/stale results, protocol requests/bytes, socket/native-message JSON bytes, base64 bytes, MIME/sanitize/SQL timings; CPU and allocations/peak memory; RSS/PSS including TB subprocesses, threads/FDs, WAL size/busy time, retained body/cache bytes; error/partial/Unknown state correctness. Report variance, not just a single favorable run. A backend may be better at cold sync and worse at warm reads; report both.

**Deterministic fault gates:** UIDVALIDITY reset with reused UID and different body; provider ID collision injected through allocation seam; cache migration/read-only/full disk/locked DB; source-manifest corruption/save failure; TB primary-password/login-required/unsupported binary/child exit/late reply; queue Full versus Closed; stale body/page after switch/reconnect/removal; expired TB list cursor; response/body UTF-8 and framing limits; JMAP notUpdated/notDestroyed; Graph/JMAP expired delta state; OAuth state mismatch/callback stall/denied consent/cancel/token rotation/keyring failure; SMTP disconnect before DATA/after acceptance/before Sent append; crash after durable queue insert and before completion. All fixtures should be isolated and fictional, with no provider-bound mail or flag changes.

Release criteria are semantic first: no wrong-message routing/cache reuse, no silently lost accepted request, no implicit credential/profile mutation, no hidden truncation/partial result, and no automatic retry of uncertain sends. Only then set performance targets from a measured baseline. Existing documentation’s targets are proposals, not achieved measurements.

## Source index / follow-up cautions

Local evidence is linked at the symbol's inspected source range above. Core reference documents: [AGENTS](../../AGENTS.md), [PRODUCT](../../PRODUCT.md), [architecture](../MEGAMAIL_ARCHITECTURE.md), [older onboarding research](account-onboarding.md), [older performance audit](performance-security.md). Current architecture/PRODUCT should win over historical research where the latter says not to embed TB or describes only a settings import.

External sources used: Mozilla `releases-comm-central` protocol/folder/database/extension/OAuth source; published Thunderbird messages API; Google native OAuth, XOAUTH2, scopes and Gmail sync guides; Microsoft Graph delta/immutable IDs; JMAP RFCs 8620/8621; async-imap, imap-codec/imap-next and Stalwart jmap-client upstream; Fastmail developer documentation. Docs establish supported mechanisms, not verified compatibility, maintenance guarantees, performance superiority or permission to reuse third-party OAuth registrations.

This report is a foundation audit, not an exhaustive competitor catalog or a completed runtime diagnosis. Its implementation sketches intentionally avoid a protocol rewrite and preserve both existing production paths.

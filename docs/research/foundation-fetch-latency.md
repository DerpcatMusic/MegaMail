# Foundation: selected-message latency and faithful HTML

Research date: 2026-10-10. Checkout HEAD: `d4dbd2bba14f474a33870269cc15bd98376ff360`.
**Working tree is actively changing under four implementation owners.**
References below identify symbols as well as approximate line anchors; they describe inspected source, not a built revision. Recheck them after the owners finish.

## Decision

Keep both existing mail implementations. Make **the exact selected message body an independently completable read**, before related headers, Sent discovery, folder counts, attachment listing or sibling bodies. Render designed mail in an isolated HTML/CSS document, not in the shell's semantic text renderer. Preserve Zeron's shell, navigation, theme and compact hierarchy. These are scheduling and reader fixes, not a new mail-engine rewrite.

There is an executed, red-capable injected-stall loop in [`tools/megamail-latency-fixture.py`](../../tools/megamail-latency-fixture.py). It executes the **production Thunderbird extension listener and body handler** with fictional Messenger methods in a Node VM. It demonstrates that the selected body's reply waits for its extra header/attachment operations. It also demonstrates that this JS listener does **not** serialize independent body requests. This is not a provider reproduction, a real Thunderbird measurement, a Rust actor benchmark, or a complete click-to-paint diagnosis.

No Cargo commands, app/TB launches, private profiles, credentials, real mail, provider mutations, tracking URLs or commits were used. Only this note and the new isolated tool were written. Parent retains the serial Cargo/build slot. Source inspections and public documentation reads used codemode. Graft was queried first and returned no hits.

Read: AGENTS, PRODUCT, DESIGN, architecture, [foundation-mail-engine](foundation-mail-engine.md), existing probe, diagnosis/codebase-design/Rust/find-docs/impeccable skills and relevant references. Impeccable's context loader misclassifies the freeform native Platform field as web; do not apply that classification or repair product docs in this package. User's screenshot symptoms are supplied observations; the actual screenshot was not attached to this sub-agent. No claim here is a visual verification of that image.

## 1. Executed evidence, before runtime theories

### Commands already run

Run from repository root, with Python 3 and Node available. No dependencies are installed by the fixture.

```sh
python3 tools/megamail-latency-fixture.py --scenario unrelated --samples 3
python3 tools/megamail-latency-fixture.py --scenario headers --samples 3
python3 tools/megamail-latency-fixture.py --scenario attachments --samples 3
python3 tools/megamail-latency-fixture.py --dispatch serial --samples 3
```

Each trial injects a 300 ms stall; exact selected body 42 must finish within 100 ms. Body 1 is unrelated. Selection starts **after** unrelated work enters the stall, not merely after it is queued. The assertions check the exact body marker and HTML marker, not just eventual success. A serial-dispatch control deliberately delays selected dispatch; it is test injection, not the production actor.

Executed extension SHA-256:
`24156e8077649bb7156464c12ce9c2c0f01f33653b73871a8ca67f9683b3c4eb`.

| Injected dependency / dispatch | Samples | p50 ms | p95 ms | Exit / verdict |
| --- | ---: | ---: | ---: | --- |
| Unrelated getFull(1), real listener parallel | 3 | 5.364 | 5.452 | 0 / green |
| Selected getHeaders(42), real handler | 3 | 300.194 | 300.415 | 1 / red |
| Selected listAttachments(42), real handler | 3 | 300.324 | 300.579 | 1 / red |
| Unrelated getFull(1), injected serial host | 3 | 305.025 | 305.277 | 1 / red |

Repeat confirmation at 12:19 UTC: unrelated, 5 trials, p50 5.196 / p95 5.563 ms, green; headers, 5 trials, p50 300.293 / p95 300.497 ms, red. Nearest-rank p95 at these tiny sample counts is the maximum, **not a representative product tail percentile**.

Each selected request invokes getFull once and returns 486 UTF-8 bytes of JSON in this tiny fixture. **Network bytes and network round trips are both zero**: Messenger methods are injected in memory. Calls to get/getFull/getHeaders/listAttachments are API calls, not proof of four IMAP downloads. No allocations, app memory, Gecko memory or reader paint timings were measured.

The concrete defect reproduced is **body completion coupled to ancillary selected-message operations**. Removing getHeaders from the critical path is a testable narrow fix; removing attachment enumeration requires an explicit metadata/completeness policy. Independent sibling-request serialization at the extension listener is falsified in this fixture. The serial control proves the assertion can detect unrelated head-of-line blocking, but does not establish that the Rust actor currently suffers it in a running app.

### Feedback-loop limits

This fixture is a real production-JS seam, with a local substitute for a true external dependency. It cannot exercise Rust request admission, actor completions, protocol cancellation, the private runtime, MIME/sanitize work in Rust or GPUI painting. There is **no executed direct-versus-TB adapter or full native click reproduction** in this audit. Do not describe the whole user's latency bug as diagnosed or fixed. The missing full-adapter test seam is a deliverable for the parent, not a reason to invent a fixed cause.

## 2. Click → body: exact control flow

### Native selection and conversation work

1. Message/list/conversation click calls `LiveMailbox::select_message` from [main.rs](../../apps/megamail/src/main.rs) (list handlers and expanded-message handlers).
2. [live.rs::select_message](../../apps/megamail/src/live.rs#L995) refreshes selected header state, clears previous HTML/links and calls `request_selected_body` **before** `request_selected_conversation_members`. Already-selected retry can request body again if not loaded. There is no intentional “wait for complete thread, then select body” here.
3. `request_selected_body` registers a `BodyRequestSlot(account, path, message_id)` with the scoped `MessageKey(account, path, uid)`, then dispatches `MailRequest::LoadBody`. Duplicate suppression uses pending slot and body-loading state.
4. `request_conversation_members` sends `LoadRelated`, seeds existing member/Sent headers, and for direct accounts requests Sent headers once per folder through `request_sent_headers`. TB returns before the direct Sent-header request: TB's own conversation resolver includes its bounded candidate/Sent search.
5. `request_thread_summaries` deduplicates signatures and emits batches after page completion. Related/Sent work updates header grouping; it need not fetch or render every body before the exact selected body.
6. `apply_body` rejects responses without a matching pending slot, then checks account/path/current selection/UID against the pending key. Real mailbox header rows have their body cleared; selected body is held separately. This is useful header/body separation, not full-mailbox body retention.
7. `MailApp::on_live_change` snapshots retained live state, rebuilds list rows, and constructs `ReaderBody` only if selected text/HTML differ from the cached entry. `ReaderBody::new` prepares plain/quoted/HTML variants; [reader](../../apps/megamail/src/main.rs#L4302) renders `gpui_kit::component::text::TextView::html`.

**Stale suppression is not cancellation.** Ignoring A's late body when B is selected protects correctness but does not recover A's network/CPU time. The inspected stale arm resets loading and retries selection; probe A→B→A, stale failure and account replacement to ensure it does not cause duplicate B loads. Account/session lifetime is already owned by a concurrent agent; this note does not propose conflicting epoch edits.

### Direct Hylki worker

[worker.rs::spawn/cache_lane](../../src/worker.rs#L896) inserts an account cache lane before the network worker. SQLite bodies and attachments can be served while network sync is stalled; conversation summaries normally use a separate WAL reader. Body cache lookup also checks carefully matched sibling copies across labels ([cache.rs::load_body/sibling_copies](../../src/cache.rs#L1489)). Do not replace these with unconditional downloads or subject-only deduplication.

Cache misses reach the single account IMAP session loop. Queued `LoadBody` and `LoadBodies` overtake list/refresh requests through `reorder_reader_loads`, but not mutation/reconnect barriers. **Current working source already has `interrupt_read_for_reader`** around folder read and background work: it drops owned partial-response sessions, restores work and reconnects for foreground reading. “There is no reader priority/preemption” would be false. Preserve ordered mutations and retry/cancellation session safety.

Selected `load_body_retry → load_body → fetch_raw_message` reads the selected mailbox with BODY.PEEK[], parses/render_raw, caches eligible rendered body and sender checks, then emits Body/attachment facts. [`load_body`](../../src/worker.rs#L8719) explicitly avoids a universal BODYSTRUCTURE text-part shortcut because of existing server parser failures. Whole MIME includes attachment bytes even when the UI has not requested an attachment save. Selected body can be expensive for a huge MIME envelope; **do not claim header-only listing means zero attachment bytes during a selected whole-message fetch**.

`LoadBodies` batches use `try_collect` over a UID-set fetch before emitting members ([`load_bodies`](../../src/worker.rs#L8750)). An exact selected UID inside such a batch can wait for a slow sibling until the collection ends. Native selection currently uses LoadBody, not an automatic whole-thread batch; retain this separation. Direct cold LoadBody is not wrapped in the folder-read preemption helper, so selected A→B requests can still serialize on one session even though background lists can yield. Runtime incidence remains unmeasured.

Worker body events are transformed **on the emitting worker/cache thread**, not in GPUI render: [live.rs::spawn_account_worker/compact_worker_event/body_content_and_links](../../apps/megamail/src/live.rs#L4183). HTML → plain-text conversion, semantic sanitization, quote extraction and link extraction happen there. Cache hits skip IMAP/MIME download but do not necessarily skip this native transformation.

### Thunderbird adapter, transport and extension

[`ActorState`](../../apps/megamail/src/thunderbird_adapter.rs) is profile-scoped, so several accounts can share one actor. `schedule_body` maps scoped native UID to TB message ID, consults its BodyCache, deduplicates pending bodies, and starts a thread calling `Runtime::call("body")`. Completion returns through `Command::BodyFinished`; parsing, cache insertion and event emission occur when the actor handles that completion.

Body/conversation/folder-refresh jobs are already threaded. **Page reads are different:** `handle_page → page` calls `runtime.call("list")` synchronously in the actor (unless initial preloaded data answers it). A slow page can prevent the actor from admitting a new selected body or processing its already-ready BodyFinished. The actor is the source-supported candidate, not the JS Promise listener. This potential delay has not been executed against the real actor here.

[`Runtime::call`](../../crates/mail-core/src/thunderbird_bridge.rs#L275) allocates a request ID, registers a reply channel, locks only to write a framed request, then waits on its own reply receiver. A routing reader maps response IDs and expires timed-out requests. The stream mutex is **not held across response wait**. Existing transport supports overlapping calls; a new IPC engine is unnecessary. A thread in recv_timeout still cannot be “canceled” merely by dropping a UI ticket.

[`background.js::connectHost`](../../tools/thunderbird-bridge/extension/background.js#L909) invokes asynchronous dispatch separately for each incoming request. Executed fixture confirms overlap at this seam. [`messageBody`](../../tools/thunderbird-bridge/extension/background.js#L625) first validates advertised message size (20 MiB source cap), then Promise.all waits for getFull/getHeaders/listAttachments. Output has a shared 2 MiB plain+HTML budget and a truncated flag. That cap does not make DOM parsing, source download or JSON escaping free.

`conversationMessages/expandParentHeaders` use header-only queries, up to 24 parents, 8 folders, 128 candidates and limited mapping parallelism; successive ancestor rounds await previous discovery. Their getHeaders calls and Sent candidate work can be costly, but **thread header traversal is not a selected-body prerequisite in native selection**. It should remain background and independently report partial results.

`parse_body_payload` prepares native semantic HTML, plain fallback, quote-free variant and extracted links. It currently ignores `truncated`. getFull defaults to decrypt=true according to official docs; sensitivity/decryptionStatus needs to survive into cache policy before any expanded prefetch or persistent TB reader cache. Direct encrypted cache exclusions already exist; do not silently weaken them.

## 3. Cache / repeated work / whole-mailbox costs

| Layer | Present behavior | Exact opportunity and limit |
| --- | --- | --- |
| Direct SQLite body | Rendered MIME HTML cached; sibling-copy fallback; encrypted exclusion | Warm provider download avoided; native plain/sanitize/link projections repeat. Cache derived representation by content digest + renderer/privacy version, not numeric UID alone. |
| Direct list/cache | UI query lane is paged; worker load_messages still loads cached folder vector, recent window, ALL UID set, merge/prune | Paged UI does not prove bounded worker CPU/memory. Keep correctness reconciliation, then measure changed-row transactions and chunk scans rather than whole vectors. |
| TB native BodyCache | 8 entries / 16 MiB, scoped account/path/UID; pending dedup | Cache hits avoid JS body calls and Rust re-sanitization; payload clones remain. Measure before converting to Arc. No unconditional TB repeated-download claim. |
| Thunderbird itself | Backend-owned store in isolated runtime | Do not count a getFull API call as a network miss; record actual loopback IMAP commands/bytes. No TB persistent offline-reader contract was verified. |
| Live mailbox | Selected body separate; list/group rows body-free | Header changes may rebuild_visible → sort/clone/group the loaded set, capped to a header window, not every server row. |
| MailApp | ReaderBody LRU 8 entries / 32 MiB, multiple text/HTML variants | on_live_change rebuild_list_rows runs for notifications even if only status/body changes. ReaderBody equality guards construction; not re-sanitizing every event. UI cache is not hydrated back into LiveMailbox on selection, so reopening may still dispatch a backend cache hit. |

Source: [live.rs::rebuild_visible/rebuild_conversations](../../apps/megamail/src/live.rs#L3085), [main.rs::ReaderBody/on_live_change/rebuild_list_rows](../../apps/megamail/src/main.rs#L610), [worker.rs::load_messages](../../src/worker.rs#L7451), [query.rs](../../crates/mail-core/src/query.rs), [cache.rs](../../src/cache.rs).

**Avoid the misleading “whole mailbox rebuild on every event” diagnosis.** Native rebuild_visible is called on several header/related/flag events, not every event; on_live_change separately rebuilds ListRow allocations whenever notified. Direct worker does have full cached-folder merging. These are three different costs. Measure each. One new body can also update references/attachment metadata and legitimately trigger grouping; a plain body paint alone should not.

## 4. Faithful HTML is a renderer contract, not only a sanitizer tweak

The reported flattened marketing layout, absent images/styles and concatenated footer are consistent with an **intentional capability restriction**, not proof the provider sent no HTML. [mail_text.rs::reader_html](../../crates/mail-core/src/mail_text.rs#L49) removes CSS, dimensions/alignment/resources, replaces images with alt text and flattens presentation tables. `ReaderBody.source_html` is already sanitized, not the original sender document. Letting style tags through this sanitizer alone cannot turn a native semantic TextView into a browser layout engine.

Direct `extract_body` already preserves a single HTML document and resolves inline CID bytes into data URIs with a 16 MiB decoded-image budget ([worker.rs::extract_body](../../src/worker.rs#L10474)); native projection subsequently removes its images/styles. TB returns text/html parts but no inline CID byte bundle in its body response. Its getFull's nontext parts require an explicit getAttachmentFile operation; preserving cid: URLs without resolving them does not display those images.

Existing GTK reader uses WebKit, CSP and sandboxed per-message frames ([message_view.rs::build_webview](../../src/ui/message_view.rs#L5011), [inject_csp](../../src/ui/message_view.rs#L6655)). Borrow those privacy/design lessons, **not the entire GTK widget tree**. Native Cargo uses GPUI Kit 0.7.1 and does not currently declare a webview host; the root GTK package has webkit6. A GPUI-native Linux surface/texture/input integration must be proved, not asserted to be a drop-in GTK WebView. This is a reader implementation slice, not an engine rewrite.

### Required isolated document policy

- Sender CSS is allowed **inside the message document only**: typography, inline styles, style blocks, table layout, backgrounds/colors, width/max-width and local responsive media queries. It must not select shell elements or inherit opaque privileged origin access.
- Keep shell toolbar/metadata outside that document. Use an opaque message canvas (sender background when set; neutral readable document background otherwise), with no wallpaper showing through the content. Keep 24/10/5% shell bleed caps elsewhere; do not redesign the Zeron panes.
- Fail closed on JavaScript, event attributes, forms, plugins, frames, base URL overrides, meta refresh, auto navigation, popups/downloads and file access. No extension/native bridge exposed to sender content.
- Install CSP before parsing/loading any sender markup, block network at the resource-request layer as well, and verify zero external requests. Baseline: default-src 'none'; script-src 'none'; connect-src 'none'; object-src 'none'; frame-src 'none'; form-action 'none'; base-uri 'none'; style-src 'unsafe-inline'; font-src 'none'; img-src only the bounded app-controlled inline-image mechanism. CSS url(), imports, font loads, srcset, backgrounds, SVG links, preload and DNS-prefetch must not bypass denial.
- An HTML iframe sandbox does not itself block image/style fetches. Avoid allow-scripts and unnecessary allow-same-origin; if trusted wrapper sizing needs DOM access, review its origin/privilege model explicitly. Native host isolation must implement equivalent restrictions. CSP does not replace navigation interception.
- Resolve only message-owned CID images, safe raster MIME types with bounded bytes/dimensions, no file:// mapping, no arbitrary data/SVG payload privilege. TB may need lazy explicit inline-part reads; measure their network cost separately and display layout/placeholders before they arrive.
- Remote images remain **blocked by default**. Keep dimensions and alt text/placeholders so design remains legible. Explicit per-message “Load remote images” may fetch images under a separate user-approved policy; never auto-fetch on selection, prefetch, sender trust or screenshot parity. Do not load remote styles/fonts as a side effect.
- Links retain labels in place; open only validated destinations on explicit activation, with the full destination inspectable before opening. Do not “clean” tracking redirects by fetching them or guessing rewritten URLs.
- Preserve plain view, copying, selection, RTL, quote restoration and attachments. Do not silently truncate a document as complete. Keep an immutable bounded source/policy result off UI thread; theme changes must not rerun MIME parsing/network reads.

### Immediate UI repairs, separate from faithful HTML

[main.rs::links_panel](../../apps/megamail/src/main.rs#L3652) displays every extracted URL on a separate line (up to 32 from extract_links). Collapse this duplicated panel by default; use inline labels and destination tooltip/context action, with an optional Link details disclosure. Long opaque tracking URLs should not dominate the reading surface.

The inspected conversation date is capped at 90 px and truncated ([main.rs](../../apps/megamail/src/main.rs#L4450)). Render a localized compact date in the header and expose full date/time through tooltip/accessible text; at narrow width move metadata to a wrapping second line. A wider fixed cap alone does not solve narrow panes.

Preserve 232/332 reference shell widths and smooth compact rail/list rules. At insufficient reader width collapse navigation structurally; keep sender/recipients/date/actions wrapping without losing labels. Test 1280×800, 1000×700, 800×600, 200% text/UI scaling, dark/light, RTL, multi-column marketing mail and wide data tables. Use a local scroll viewport only for genuinely wide message content; never widen the whole app or force marketing typography onto shell metadata.

## 5. Ranked remaining hypotheses and falsification cases

These are test plans, not declarations of a root cause. The ancillary JS wait is already reproduced; the unrelated JS serialization hypothesis failed its test.

1. **Shared TB actor page wait:** stall another account's list response; timestamp selected admission, JS reply and actor BodyFinished emission. If reply is quick but native body completion waits for list, actor scheduling is responsible. Offloading page transport from actor should remove that wait while retaining cursor/session ownership.
2. **Direct selected A holds B / batched siblings:** stall UID A, select B; selected B must arrive before A is released, and A must never paint over B. Contrast foreground LoadBody with LoadBodies batching. Current list/background preemption must remain green; do not generalize B's delay to every read.
3. **Body projections/notification rebuild dominate warm read:** repeat same scoped selection with provider replies disabled; count sanitizer parses and ListRow/group rebuilds. If dispatch/network is zero/short but paint follows CPU/allocation spike, derived cache or retained row identities are the win.
4. **Actual MIME size / inline expansion dominates cold body:** compare 10 KiB HTML vs same HTML with 20 MiB attachment at equal RTT; measure raw bytes and body parse time. If delay follows bytes rather than queue age, priority alone cannot fix it.
5. **Thread/Sent lookup competes but is not awaited:** inject 1 s ancestor/candidate stall, compare grouping on/off while exact body is quick. If selected completion changes only with background activity, inspect admission/resource contention; if it stays fast, do not rewrite conversation discovery for reader latency.

## 6. Comparable synthetic benchmark and red gates for parent

### Existing probe: use precisely what it measures

[`tools/reader-latency-probe.py`](../../tools/reader-latency-probe.py) provides a real synthetic TLS IMAP peer, fresh XDG dirs, private D-Bus, seed cache/schema, 121 folders × 70 ms STATUS, UID42, stalls for list/first-body/repeated SELECT, and bwrap launch/cleanup. HYLKI_SHOWCASE selection at 2 s and Inbox refresh at 1 s target **GTK Hylki**, not native MegaMail selection. Success is “body cached”, not “body painted”; automatic prefetch can satisfy it. It has no deadline assertion for the exact selected request and no common TB runner. No existing compiled binary was launched here, since even an isolated direct password path can involve desktop credential storage; any full runner must substitute credentials/Secret Service too.

Use its IMAP vocabulary/schema as the basis of a loopback fixture, but do not report it as a native/TB apples-to-apples benchmark or run private-provider ignored tests. Parent builds artifacts in its exclusive Cargo slot and approves an isolated runtime path.

### Same workload for both actual adapters

Implement through the **existing native request/event interface**, not a new MailBackend stub. Direct worker receives synthetic account/cache; TB actor receives an injected framed Runtime transport first, then a credential-free synthetic isolated TB profile pointing only at loopback IMAP. Factory/transport injection is an internal seam; user-facing account operations stay small. All identities/domains fictional .test, fresh dirs, no real homes/config/keyring/manifest copies.

| Dimension | Required cases |
| --- | --- |
| Data | Same raw MIME corpus, UID/message-ID graph, 100-row page, 1k/10k/100k headers; 1/6 accounts; 121 folders |
| Body | 10 KiB, 1 MiB, 2 MiB, oversized; marketing tables/CSS, inline CID raster, remote images blocked, plain+HTML, malformed MIME, multibyte truncation, encrypted/cache-excluded |
| Network | Loopback deterministic 0/50/150 ms delay and bounded throughput; explicit delayed UID/list/STATUS/ancestor response; TLS same policy |
| Cache | Fresh-empty cold; warm header-only; warm backend body; warm native derived reader; warm restart; offline body present/missing |
| Priority | Slow unrelated list/account, slow sibling body, Sent/ancestor stall, A→B→C rapid selection, stale response after reconnect/removal |
| Safety | No send/store/expunge/flag changes; BODY.PEEK only; no remote document requests; mutations represented as fake ordering barriers only |

**Red condition:** hold unrelated work until the test explicitly releases it; selected exact body must complete first within a 100 ms in-memory deadline. If fixture release is required to obtain selected completion, fail even if final content eventually matches. Add final paint callback to a native integration run; cache-write time alone is not enough. A synchronous fake actor is the negative control; parallel JS and background-read preemption are positive controls. Return precise missing-current-body errors, never an eternal spinner.

For cancellation, compare selected admission time to body request time and paint, record superseded reads separately from dropped late events, and preserve mutation order. A backend API with no physical cancel must reject obsolete results, stop queued work, bound active requests and reclaim them at deadlines. Dropping a task future while retaining a partially consumed IMAP session is forbidden; current worker ownership/cancel helper is the safer precedent.

**Observability:** correlated synthetic request ID + scoped key + selection generation + account/session epoch; timestamps click/admit/cache lookup/queue/dequeue/transport submit/first byte/last byte/MIME/sanitize/actor completion/UI apply/first usable paint. Count queue age/depth, jobs/threads, canceled/coalesced/stale, list and body RPCs, cache hits/misses, sanitizer/DOM parse count, rebuild counts. Logs use fictional identity only.

Server records IMAP command count, TLS plaintext payload bytes and UID-specific raw bytes; distinguish connection/TLS/auth/SELECT/SEARCH/FETCH overhead from message bytes. Record TB API calls, socket/native JSON bytes separately, not as network round trips. System measurement includes direct worker process **and all TB child processes** (RSS/PSS, threads/FDs, CPU); allocation profiler counts source/derived copies and peak live bytes; Python tracemalloc is not a Rust/Gecko memory metric. Retained string budgets do not cap DOM, images, cache DB or child processes.

Run ≥100 trials per cohort for product p50/p95 (report cold independent profiles vs warm repetitions separately), fixed versions/build profile/hardware/filesystem/compositor, seed and trace. Produce stage distributions plus total; do not average per-stage p95s into a supposed end-to-end p95.

### Explicit goals, not measured product performance

For a 10 KiB body, 50 ms synthetic RTT, ≥10 MiB/s throughput, ≤100 visible rows and no auth prompt:

| Goal | p50 | p95 | Other budget |
| --- | ---: | ---: | --- |
| Header selection feedback | ≤16 ms | ≤50 ms | No body dependency |
| Warm native/derived body to usable paint | ≤30 ms | ≤75 ms | 0 network bytes/round trips; 0 repeat MIME/sanitize parse |
| Warm backend body / offline reopen | ≤50 ms | ≤100 ms | 0 provider bytes; local lookup/IPC permitted |
| Cold body with connected backend | ≤250 ms | ≤500 ms | Direct ≤1 selected raw FETCH, plus at most required mailbox select; TB body contract ≤1 getFull, actual IMAP bytes measured |
| Cold startup first selected body | ≤1 s | ≤2 s | Includes connection setup; not all accounts/folders finishing |
| Latest selection during unrelated 1 s stall | ≤100 ms above unstalled baseline | ≤150 ms above baseline | Exact body available before unrelated release |
| UI input during background sync | ≤16 ms | ≤50 ms | No list-wide work in render |

Additional **goals**: no ancestor/Sent/STATUS network work required before selected-body completion; zero automatic remote image/font/style bytes; zero duplicate concurrent selected raw downloads for the same valid scoped copy. Memory stays inside current TB 16 MiB + native 32 MiB retained-body budgets plus explicitly measured parse/image overhead; do not call these a 48 MiB process cap. Set allocation goal relative to measured baseline: ≥50% fewer warm-path allocated bytes and no O(loaded-header-count) allocations for a body-only paint. Do not invent absolute Rust allocation counts before profiling.

For large body/attachment cases report service-time goals separately and derive wire-time floor from byte size / throughput. No 500 ms promise for a 20 MiB MIME over a constrained link. Fast preview must be labeled preview/partial until full content is available.

## 7. Bounded opportunistic prefetch

Current direct worker queues up to **50 recent bodies and 25 attachment-bearing messages per eligible folder** ([worker.rs::queue_body_prefetch/queue_attachment_prefetch](../../src/worker.rs#L3414)); these are per-folder counts, not a total byte budget. Native admission still gives selected work first; TB schedule_body can create many simultaneous threads through LoadBodies. Header-page limits alone do not bound body work.

Proposed native account policy (goals/new work):
- After foreground body is usable and user selection settles, consider next/previous visible message only, at most two queued neighbors and one background body in flight per account; two background bodies globally.
- Use advertised source-size/byte limits (e.g. 2 MiB neighbor body and 4 MiB per selection burst), bounded time and backend-appropriate cancellation. Skip unknown/oversized, encrypted/cache-excluded, Junk/Trash and metered/offline accounts by default.
- Cancel/coalesce queued neighbors on selection/folder/account changes. Latest selected bypasses prefetch; don't put selected in a sibling batch or spawn a thread for every loaded row.
- Cache canonical bounded body/derived view, keyed by scoped copy + freshness/session + renderer/privacy version. Retain direct sibling-copy identity checks.
- No prefetch of attachment files, remote images, fonts, CSS or link targets. Already-owned inline MIME resources are separately budgeted; an explicit image action remains separate from body prefetch.
- Policy belongs to each account; visible preview, offline availability and cold download are distinct states. Existing direct disk cache can provide offline bodies; TB RAM cache is session-only and TB store availability is not a verified native offline promise. Do not add persistent decrypted-body caches for “instant” reopening.

Do not globally change shared GTK defaults as part of this native research slice. Native-only policy control needs a serial worker/adapter follow-on with existing owners.

## 8. Narrow packages and ownership

### Next disjoint package: extension-only body/header de-duplication

**Proposed owner:** a new implementation agent only after parent confirms extension ownership is free. Scope `tools/thunderbird-bridge/extension/background.js` and this isolated latency fixture. No native main/live/TB adapter, worker, core OAuth/discovery/onboarding, vendor, Cargo or profile changes.

1. Replace the selected body's extra getHeaders call with root decoded headers already returned by getFull (explicit decodeHeaders=true); preserve replyTo/references/inReplyTo schema and behavior. Keep conversation-only getHeaders: official docs say it is faster when only headers are needed.
2. Extend fixture full.headers/virtual-root cases, header equivalence assertions and missing-header behavior; headers-stall case must turn green, with one getFull and zero selected getHeaders. Keep unrelated-stall green and serial negative control red.
3. Keep listAttachments behavior unchanged in this first package so attachment semantics aren't guessed. Its red case remains an explicit known limitation, not hidden.
4. Verify getFull root/virtual message/rfc822 header placement using the minimum supported TB version's tests/source, not only latest docs. Treat decrypt/cache policy as unchanged but unresolved; no new persistence.
5. Parent integrates/validates under its serial build slot. This package removes one real body prerequisite/API call without changing the app event contract; actual network/download savings require full-adapter counters.

### Serial follow-ons, not concurrent edits to current owners

A. After TB owner releases adapter: move page transport waits out of actor, with bounded jobs, retained cursor/session validation and completion priority; test body ready while another page stalls. Separate body completion from optional attachment metadata using Unknown→Ready metadata, not false “no attachments.” Bound in-flight body jobs and cancel queued obsolete reads.

B. After worker owner releases: test selected A→B cancellation/latest-priority and foreground batch behavior; expose native prefetch policy. Keep reconnect-on-canceled-stream safety, encrypted exclusions and FIFO mutation barriers. Do not propose an unsupported text-part fast path as a universal fix.

C. After UI owner releases: retain list row identities by header revision, avoid body-only row reconstruction, hydrate warm ReaderBody into selection only under content/session validity; collapse duplicate links, wrap dates, opaque message content background. Instrument paint and allocation before claims.

D. Reader-only proof slice: new isolated HTML/CSS renderer module/host with raster CID + zero-network tests, then serial main/live/body-contract integration. Validate native surface/input/focus/resize/clipboard/a11y and message isolation before replacing semantic fallback. This addresses the screenshot's fidelity demand directly without changing mail engines. Keeping Zeron shell and sender-owned document separate is the design seam.

## 9. Primary sources checked

- Repository sources linked above; [AGENTS](../../AGENTS.md), [PRODUCT](../../PRODUCT.md), [DESIGN](../../DESIGN.md), [architecture](../MEGAMAIL_ARCHITECTURE.md), [foundation engine research](foundation-mail-engine.md).
- [Thunderbird messages API](https://webextension-api.thunderbird.net/en/latest/messages.html#getfull-messageid-options): getFull includes headers and MIME parts; decodeContent/decodeHeaders default true; decrypt defaults true; MessagePart has headers/decryptionStatus and nontext bytes require getAttachmentFile.
- [getHeaders](https://webextension-api.thunderbird.net/en/latest/messages.html#getheaders-messageid-options): header-only and faster than getFull for headers; added TB147. [listAttachments](https://webextension-api.thunderbird.net/en/latest/messages.html#listattachments-messageid) can list decrypted-message attachments.
- [MDN iframe sandbox](https://developer.mozilla.org/en-US/docs/Web/HTML/Reference/Elements/iframe#sandbox): origin/script/form/navigation restrictions. [MDN CSP](https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Content-Security-Policy): separate fetch/style/font/image/navigation directives. These describe policy requirements, not a completed native integration.
- Context7 resolved /tokio-rs/tokio and returned [task cancellation documentation](https://docs.rs/tokio/latest/tokio/task/): started blocking tasks cannot be stopped with JoinHandle::abort. The synchronous bridge thread needs bounded work/late-result invalidation, not a false abort promise.
- Initial guessed Thunderbird /en/stable/messages.html URL returned 404; latest API URL succeeded. Moving latest docs are not compatibility evidence for every deployed runtime version.

**Outcome:** concrete production-JS stall evidence, source-supported scheduling/renderer map, and a small extension-only improvement available without an engine rewrite. Full adapter comparison and faithful native HTML integration remain explicitly unmeasured/unfinished; the packages above define the next real work rather than marketing claims.

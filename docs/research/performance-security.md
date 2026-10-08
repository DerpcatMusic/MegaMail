# MegaMail performance and mail-security audit

Audit snapshot: MegaMail checkout 43f2d46, Zeron checkout 037f4c1. The source under MegaMail is still Hylki 1.42.0 with GTK4/Relm4/WebKitGTK; Zeron is the GPUI reference. No source was changed, and no build, test, or benchmark was run for this audit. Every number under “targets” is proposed acceptance criteria, not an achieved result.

## Decisions for the GPUI Kit port

Keep Hylki’s mail protocols, account actors, SQLite cache, and durable outbox as the domain boundary. Replace the presentation layer with official GPUI Kit and bridge asynchronous results into GPUI entities. Hylki already gives cached-body reads a separate lane so a body can open during network sync, while each account’s protocol session stays inside a current-thread Tokio runtime and LocalSet (Hylki src/worker.rs:742-799). Keep that actor boundary until the session types and IDLE lifecycle are deliberately changed.

Zeron shows one GPUI integration seam: its UI initializes gpui_tokio from a runtime handle at startup ([Zeron `ui/src/lib.rs`](https://github.com/zeronsh/zeron/blob/037f4c1/crates/ui/src/lib.rs#L158-L160)), routes subscription results into GPUI entities ([Zeron `ui/src/state.rs`](https://github.com/zeronsh/zeron/blob/037f4c1/crates/ui/src/state.rs#L11-L15)), and renders rows through GPUI’s ListState-backed list ([Zeron `ui/src/transcript.rs`](https://github.com/zeronsh/zeron/blob/037f4c1/crates/ui/src/transcript.rs#L3113-L3117)). Its shared two-thread Tokio runtime is a useful service pattern ([Zeron `client/src/runtime.rs`](https://github.com/zeronsh/zeron/blob/037f4c1/crates/client/src/runtime.rs#L20-L32)), but it cannot be copied directly over Hylki’s current non-Send account sessions. Zeron uses a custom ZUI/GPUI fork; its fork-specific patches do not transfer directly to the selected official GPUI Kit APIs.

Keep SQLite and rusqlite for local mail. Hylki already uses WAL, a five-second busy timeout, per-account connections, and private cache paths (Hylki src/cache.rs:465-494, 626-639). Zeron’s store also moves potentially blocking SQLite and mutex waits off Tokio’s runtime core and bounds WAL maintenance work ([Zeron `sync/src/store.rs`](https://github.com/zeronsh/zeron/blob/037f4c1/crates/sync/src/store.rs#L21-L31)). The GPUI render path must never wait on SQLite, a keyring prompt, MIME parsing, or network I/O. After profiling, inspect GPUI Kit’s own list virtualization, image lifecycle, and async integration APIs for equivalent capabilities.

## Priority findings

### P0 — Preserve the incoming-mail security boundary

Incoming HTML is untrusted. Hylki’s reader permits only its own nonce-bearing wrapper script; message frames block scripts, objects, and base URLs, and the default resource policy allows no network fetches (Hylki src/ui/message_view.rs:4412-4434, 6655-6663). Remote images and media load only after the user allows remote content; the frame also sends no referrer (Hylki src/ui/message_view.rs:5868-5874, 6690-6696). Its HTML reader filters image and link schemes (Hylki src/reader.rs:522-575).

Keep that policy if the reader moves to GPUI. Render email in a sandboxed browser surface or a constrained tree built from parsed, allow-listed content. Keep remote images off by default, expose the allow action as explicit user consent, and retain a restrictive CSP as a second boundary. Never place message HTML in a privileged GPUI web surface with general script or navigation access.

### P0 — Model uncertain send outcomes so retries do not silently duplicate mail

Hylki durably stores the exact composed raw message in SQLite before retrying (Hylki src/worker.rs:4078-4123; Hylki src/cache.rs:102-115, 1841-1882). After SMTP reports success, the retry loop deletes the local row before filing the Sent copy (Hylki src/worker.rs:4219-4238). JMAP follows the same delete-after-submission order (Hylki src/worker/jmap.rs:1350-1404). A process crash or lost final response around server acceptance can therefore leave the client unable to know whether delivery happened. Retrying can duplicate a delivered message; deleting too early can leave no local Sent copy.

Give the outbox explicit queued, sending, accepted, and delivery-unknown states. Persist a stable Message-ID with the raw bytes. On restart, reconcile an unknown item against the Sent folder by Message-ID before offering retry; never claim exactly-once delivery across SQLite and SMTP/JMAP. Keep a clear manual retry path when the server offers no reliable reconciliation.

### P1 — Page message summaries and search results from SQLite

The cache already indexes folders incrementally: an initial recent window is followed by background chunks of 1,000 summaries, and backfill yields to reader requests (Hylki src/worker.rs:61-64, 1440-1496, 7125-7198, 8201-8298). Keep this scheduling.

The UI path then materializes the whole cached folder: load_messages orders all matching rows by UID without a LIMIT (Hylki src/cache.rs:776-831). A cached IMAP refresh merges that complete vector with the recent server window and obtains the complete server UID set (Hylki src/worker.rs:7071-7096). All Folders search additionally clones every cached message into a new vector (Hylki src/app.rs:22718-22723); the filter linearly checks only sender, subject, and preview (Hylki src/ui/message_list.rs:3456-3468). The cache schema has no FTS virtual table (Hylki src/cache.rs:20-121), so full-text search over local bodies is not available.

Add keyset pages for message rows and All Folders search. Use a stable cursor such as the current sort key plus UID; avoid OFFSET for long mailboxes. Keep only the visible page and a small page buffer in GPUI. Add SQLite FTS5 for cached summary fields first, then cached plain text if body search is in scope. Use provider search for uncached server history and label its results as remote. Hylki’s attachment gallery is the existing page-first SQL example, including LIMIT/OFFSET and a cap on returned attachment bytes (Hylki src/cache.rs:1555-1604); replace OFFSET with keyset paging for deep mail history.

Thread work already moves to a background thread once a source reaches the configured threshold, and caps per-thread membership at 100 messages (Hylki src/ui/message_list.rs:273, 3381-3418; Hylki src/cache.rs:224-233). Keep the cap and keep large thread computation off the GPUI executor.

### P1 — Avoid downloading and parsing attachments to show a message body

Hylki fetches BODY.PEEK[] for an opened message, which returns headers, body, and attachments in one raw buffer; the comment documents malformed BODYSTRUCTURE responses as the reason it avoids fetching a selected MIME part (Hylki src/worker.rs:8303-8325). render_raw then performs MIME parsing, sender checks, OpenPGP handling, attachment extraction, and body HTML construction (Hylki src/worker.rs:3753-3791, 9990-10022). Inline CID images have a 16 MiB decoded budget, but the raw message and cached rendered HTML have no corresponding size limit (Hylki src/worker.rs:9993-10022; Hylki src/cache.rs:1350-1426). The account runtime is current-thread, so synchronous rendering occupies that account’s async loop.

Fetch only selected text/html and text/plain parts, then fetch referenced CID images within an explicit byte budget. Retain the full-message fallback for servers whose MIME structure cannot be parsed; do not silently truncate body text. Run MIME parsing, verification, and image decoding on a bounded blocking pool, and emit progressive results for conversation batches. Preserve the existing policy that OpenPGP plaintext and attachments are not cached (Hylki src/worker.rs:3784-3805).

### P1 — Make offline state visible and preserve local data during the rebrand

The cache stores summaries, rendered bodies, attachment bytes, and raw Outbox messages (Hylki src/cache.rs:20-121). On Unix, Hylki attempts owner-only directory/file modes (0700/0600), but this is best effort; the SQLite contents remain plaintext at rest (Hylki src/cache.rs:296-309, 476-495, 626-639). Hylki prefetches only the 50 newest bodies and 25 newest attachment sets; older content opens on demand and may be unavailable offline (Hylki src/worker.rs:65-93). POP3 is capped at 200 entries (Hylki src/worker.rs:8766, 9374-9382), and JMAP’s initial index is capped at 300 (Hylki src/worker/jmap.rs:25, 688).

Keep “not cached” distinct from “empty”; the IMAP body path already reports an error and does not cache an empty body when the server returns no content (Hylki src/worker.rs:8310-8324, 3690-3723). Show offline availability for bodies and attachments that were not saved.

Give MegaMail its own profile, config/data roots, database, and credential namespace. Never inspect, migrate, or mutate the Hylki profile automatically, and never point both apps at the same database. Offer an explicit, optional user-initiated import from Hylki’s known config and data roots (Hylki src/config.rs:81-83, 661-663; Hylki src/cache.rs:476-495). Make import read-only against Hylki, copy into MegaMail’s profile, preserve queued Outbox rows, and make interrupted copies restartable or discardable. This keeps a fresh MegaMail install isolated while providing a deliberate recovery path for users who choose it.

### P1 — Keep credentials out of files and support each shipped platform

Hylki writes account config with mode 0600 and stores account passwords and refresh tokens in Secret Service (Hylki src/config.rs:13-38, 697-767, 773-807). OAuth uses PKCE S256 and a random state checked on callback (Hylki src/oauth.rs:469-489, 639-694); rotated refresh tokens are written back to the keyring (Hylki src/worker.rs:5280-5309). IMAP TLS validation is the default, with invalid-certificate acceptance controlled by an explicit account setting (Hylki src/worker.rs:6294-6309). The current Cargo features select synchronous Secret Service only (Hylki Cargo.toml:42); a multi-platform GPUI build needs the native credential backend for each supported OS. Do not fall back to plaintext if a backend is missing or locked.

OAuth client secrets may exist in the private accounts TOML through OAuthSettings (Hylki src/config.rs:590-605). Owner-only file mode protects the local file, but a distributed desktop OAuth client cannot keep an embedded client secret confidential. Treat native app clients as public clients and rely on PKCE; keep provider client configuration out of logs and never put access or refresh tokens in UI state.

### P2 — Measure account-worker and event-queue costs before consolidating

Each enabled non-POP3 account starts a cache lane and mail worker; the cache lane also starts a thread-summary worker (Hylki src/worker.rs:745-784, 809-840). The request/event channels are unbounded (same locations), while current code already gives cached bodies a fast lane. Count worker threads, queue depth, and cache latency at one, two, and four active accounts. Keep separate protocol actors until profiling shows a meaningful cost and the protocol-session Send/LocalSet constraints are solved. Coalesce replaceable status and list-refresh events; never drop user commands or outbox updates.

Zeron’s performance value comes from profiling and bounded resource paths as well as GPUI: its pinned custom GPUI fork includes bounded blur scratch, idle GPU resource release, image eviction, and GPU stats hooks ([Zeron `Cargo.toml`](https://github.com/zeronsh/zeron/blob/037f4c1/Cargo.toml#L65-L98)). These fork-specific changes do not transfer directly to official GPUI Kit. Profile the mail list and reader first, then assess the equivalent Kit APIs before adding blur or decoded attachment images to long-lived views.

## Measurement plan and proposed acceptance targets

Use a reproducible release build on a documented 4-core/8-thread, 16 GiB Linux desktop with integrated graphics and 60 Hz display. Record commit, OS/compositor, GPU, compiler, and cold/warm cache state. Generate four accounts with 100,000 summaries each, 20 folders per account, repeated Gmail-style label copies, long threads, 10,000 cached bodies, and attachment metadata with only recent blobs downloaded. Record UI and renderer process-tree RSS/PSS, worker thread count, SQLite query duration, rows returned, request/event queue depth, fetched bytes, MIME parse/render duration, and 50th/95th/99th frame time. Trace metadata only; do not log message bodies or tokens.

| Measure | Proposed target, not yet achieved |
| --- | --- |
| Warm open of a 100,000-message folder from SQLite | First visible 60 rows at p95 ≤ 250 ms |
| Local summary search over 400,000 rows | p95 ≤ 100 ms; results start after the first indexed page without cloning all rows |
| Scroll the message list at 60 Hz | 99% of frames ≤ 16.7 ms after warm-up |
| Open a cached 1 MiB HTML message | First readable paint at p95 ≤ 200 ms, with no MIME/SQLite work on the GPUI thread |
| UI-resident summaries for a 100,000-row folder | At most 2,000 summaries for the current page plus nearby page buffers, independent of total folder size |
| Four-account 30-minute sync and folder-switch run | Process-tree memory grows ≤ 5% after warm-up; no unbounded event or request queue growth |
| Cached offline content | Every cached body opens offline; missing body/attachment yields an explicit unavailable state, never an empty-message success |
| Mail delivery ambiguity | Fault injection around SMTP final acceptance and JMAP submission never silently retries an unknown delivery; user sees the unknown state and a reconciliation/retry choice |
| Incoming HTML policy | Automated fixtures prove script/object/navigation denial and zero remote requests until the user opts in |
| Credentials and optional import | Locked/missing credential store never writes secrets to TOML; MegaMail never touches Hylki’s profile automatically; an explicitly requested import is read-only against Hylki, restartable, and preserves Outbox rows |

Benchmark sequence: capture the unmodified Hylki baseline first; implement paging and body-fetch work separately; capture the same datasets and traces after each slice. Add synthetic HTML cases with large attachments, nested MIME, malformed BODYSTRUCTURE, CID images, tracking pixels, and OpenPGP. Add send fault-injection points before and after DATA/submission acknowledgment and before Sent filing. Run the OAuth callback suite with wrong state, stale callback, rotated refresh token, and unavailable keyring. Gate the GPUI port on UI-thread tracing showing no synchronous database, secret-store, network, MIME, or image-decode work.

## Scope intentionally skipped

No mail-server replacement, CRDT mail sync, cloud body proxy, or new protocol was evaluated. Zeron’s agent engine, workspace sync, and realtime chat storage do not solve MegaMail’s IMAP/JMAP/SMTP correctness problems. This audit changes documentation only.

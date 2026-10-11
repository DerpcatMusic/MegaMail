# Batch 2: Thunderbird runtime reliability

Date: 2026-10-10. Owner: runtime subagent, Pi GPT-6.1 Sol. Status: implementation and synthetic kernel/static checks; **Rust compilation and runtime-contract execution deferred to the parent's frozen batch window**.

## Scope and evidence

Read AGENTS, PRODUCT, DESIGN, architecture, feature matrix, and foundation mail-engine, account-lifecycle, provider-auth and fetch-latency reports. Queried Graft first; the query returned no matching nodes, so source inspection followed. Applied Rust best-practices/concurrency guidance. The foundation reports explicitly do not establish an executed native latency or provider incident. This patch does not turn their hypotheses into reproduced incidents.

Owned changes only:

- `crates/mail-core/src/thunderbird_bridge.rs`
- `crates/mail-core/tests/runtime_batch2/contract.rs`
- `crates/mail-core/tests/runtime_batch2/process_contract.rs`
- `crates/mail-core/tests/runtime_batch2_kernel.py`
- This report.

No manifests, core library declarations, GTK shared files, `thunderbird.rs`, native adapter/UI, or active MailExtension-owner files changed. No commits, builds, checks, Clippy, rustc, helper compilation, actual Thunderbird launch, private-profile/credential inspection, live provider login, send, or mail mutation occurred.

## Implemented production behavior

### Transport admission and deadlines

`Runtime::call` retains its existing public `Result<Value, String>` signature. It delegates to the same internal `call_until` route that the synthetic Rust tests use with shortened deadlines; the tests do not implement a replacement actor.

- At most **64 pending requests** per shared runtime. Admission returns a tagged Busy error instead of adding unbounded pending channels.
- Request encoding stops at **1 MiB**, rather than allocating an arbitrarily large serialized JSON buffer before checking its size.
- Reply frames retain the existing **16 MiB** maximum. There is one response reader, no user-space response-read queue, and one bounded reply slot per admitted call. This is a transport bound, not an assertion that the native actor or extension's independent work is globally bounded.
- IDs are monotonically issued within JavaScript's exact-integer range. A separate exact shutdown ID replaces `u64::MAX`, which did not round-trip through JSON/JavaScript safely. Exhaustion refuses further requests rather than wrapping.
- Future, zero, missing, or unsafe response IDs retire the transport; expired/already-completed IDs are ignored. A response envelope must have exactly one result or string error. A guessed *different currently pending valid ID* cannot be distinguished from that ID's legitimate response by correlation alone; this is not an authentication protocol.
- Pending replies carry their original deadline. Late completions are discarded, including the race where the reader removed an entry just before a caller's deadline.
- Each operation's existing budget now includes writer-lock admission, socket frame transfer, and reply waiting. The socket mutex is **not held while waiting for a reply**. Existing out-of-order body-versus-metadata multiplexing remains intact.
- Socket writes use per-call nonblocking sends and monotonic polling, not a timeout that resets with each partial write. A timeout before sending any bytes remains local; a partially written frame closes the shared transport because its framing cannot safely be reused.
- An idle healthy response connection does not time out. Once a frame begins, a **15-second whole-frame deadline** prevents an indefinitely trickling partial frame. A frame timeout, malformed JSON, oversized frame, or truncated frame is a transport/protocol failure, unlike an individual operation timeout.
- Admission checks the disconnected flag while holding the pending lock, paired with disconnect's drain. A call cannot install an orphaned reply channel after retirement.

### Startup and compatibility

- The process-start accept and handshake phases share one **60-second deadline**, rather than an accept budget followed by another 90-second handshake budget. This does **not** time-bound preceding filesystem clone preparation or an OS process-spawn stall.
- The handshake must report ID 0, ready true, and the exact content-derived version of the extension just bundled for that clone. Cached/stale extension code is not silently accepted.
- Version probing retains a **five-second budget** and uses nonblocking bounded stdout/stderr reads, with **16 KiB combined output**. It avoids both full-pipe deadlock and `wait_with_output` waiting indefinitely on inherited pipe descriptors.
- The minimum remains **Thunderbird 153**, intentionally. **157** remains the current documented loopback reference, not a raised floor or an upper bound. Newer majors pass the floor check; their individual APIs/provider behavior are not thereby verified. Exact current bridge packaging is checked separately from the executable major.
- RAII guards remain armed through process creation, handshake, socket cloning, and reader-thread creation. A startup error cleans transient socket/host artifacts and the owned process, and never publishes a working Runtime/channel.

### Shutdown and process ownership

- Both the version probe and the runtime get a dedicated process group with `process_group(0)`.
- Linux startup/version liveness uses `waitid(... WNOWAIT)` rather than reaping the leader before descendant cleanup. Keeping the leader unreaped anchors the group's numeric identity until cleanup, avoiding PID/PGID reuse targeting an unrelated process.
- Cleanup signals **only the dedicated group created for this spawn** and reaps its direct child. There is no Thunderbird-name scan, global `pkill`, source-profile launch, or unrelated-process kill.
- Last-owner drop reserves up to **two seconds** for the shutdown request/acknowledgment, closes both socket directions even if the peer acknowledges but stays open, joins the response reader, kills the owned group if needed, and removes transient files. A separate socket control handle can wake I/O without waiting for a writer mutex.
- Persistent clone generations, drafts, and Outbox data are retained. This patch does not prune generations or retry a timed-out mutation automatically.
- **Limit:** direct-child `wait()` after SIGKILL still relies on kernel process exit/reaping; an uninterruptible kernel task can exceed the user-space shutdown budget. This is not a hard realtime shutdown guarantee. Descendants that deliberately leave the dedicated process group are not tracked/killed. Non-child zombie reaping belongs to the OS/subreaper, not this library. Snap/daemonized launch modes and non-Linux behavior need separate verification; Linux remains the product target.

### Clone safety

The existing allowlisted encrypted-store copy remains; MegaMail neither decrypts it nor copies source mail storage. New checks reject overlapping source/target paths, parent traversal, and symlinked destination components. Private directories are created with mode 0700, opened without following the final symlink, checked for current-user ownership, and secured through the open descriptor. Destination files and private profile locks must be owned, singly linked regular files; unchanged packaged files have mode 0600 restored rather than bypassing permission repair. Snapshot-stamp reads are bounded to 4 KiB. Socket permission failures now abort startup.

These checks are not a filesystem sandbox against an adversary with the same UID racing every intermediate component. Source preference parsing and its allowlist still belong to `thunderbird.rs`; no ownership transfer was needed. Credential-generation selection is still a metadata stamp, not proof of provider-token validity. Clone independent restoration, source relinking, old-generation retention policy, interactive unlock, and account inventory durability remain native/session work.

## Error classification and exact native-owner obligations

New additive API:

```rust
thunderbird_bridge::classify_bridge_error(&str) -> Option<BridgeFailureKind>
```

Recognized errors start with `[megamail-tb:<kind>] `. Only this stable prefix is classified; arbitrary provider prose is not searched. Existing string-returning public APIs remain compatible.

The native owner was sent this handoff through T3 while working:

1. In `is_broken_bridge_error`, `classify_runtime_error`, and `is_connectivity_error`, classify tagged bridge errors **before** legacy substring handling.
2. `Disconnected | Protocol`: retire the shared profile runtime and offer retry/restart, preserving configured identities and session-epoch rejection.
3. `RequestTimeout | Busy`: do **not** retire a healthy runtime. Busy is overload/retry-after-completion, not missing worker or NeedsLogin. A request timeout rejects only that request's late result; it does not physically cancel extension work.
4. `Unsupported`: distinguish installed-version/bridge compatibility repair from unsupported account type where possible.
5. `NeedsLogin`: explicit extension `errorCode: "needs-login"` only; no automatic headless recovery is promised.
6. `Operation`: ordinary extension/provider failure. Do not label it NeedsLogin merely because prose contains “credentials”, “OAuth”, or “authentication”. Untagged legacy operation-specific fallback can remain.
7. Send/draft timeout or disconnect after admission remains potentially **unknown outcome**. Preserve the composer and check Sent/Outbox/Drafts; never infer safe automatic retry or exactly-once delivery.
8. Native actor admission, selected-body scheduling, reconnect/account epochs, and durable inventory must retain their own production-path tests. This core transport patch does not substitute for those tests.

**Needs-login limit:** the current extension emits string errors and no `errorCode`, so the new explicit-code route is exercised only by synthetic framed fixtures. Enumeration/handshake/cached folders are not authentication proof. Wiring reliable provider/primary-password evidence requires the extension/native owners; their files were not changed here.

## Tests and validation ledger

Executed:

- `rustfmt --edition 2021` on the owned Rust source/test files: passed parsing/formatting; **not type-checking**.
- `git diff --check -- crates/mail-core/src/thunderbird_bridge.rs`: passed.
- `python3 crates/mail-core/tests/runtime_batch2_kernel.py`: **3 tests passed**. They verify duplicate-socket shutdown waking an idle reader, Linux WNOWAIT/group cleanup with an unrelated synthetic process spared, and static inclusion/use of the production Runtime test seam. These are kernel/static checks, **not Rust Runtime execution or a native benchmark**.

Authored, **not executed**:

- `runtime_batch2/contract.rs`, included under the bridge's `cfg(test)` module: timeout/late rejection with healthy subsequent public call; writer-lock deadline with no expired request emitted; 64-call saturation and disconnect drain; wrong/missing ID; oversized/truncated frame; malformed envelope; oversized outbound request; explicit login code versus ambiguous prose; JS-safe ID exhaustion; ack-with-open-socket shutdown; shared startup/handshake deadline; stale/current bridge handshake; 153/157 compatibility; synthetic clone snapshot isolation/privacy; symlink/hardlink rejection; permission repair; bounded serialization.
- `runtime_batch2/process_contract.rs`: synthetic shell version output; failed-startup direct-child reap; owned-group descendant termination. These invoke no Thunderbird executable.
- Existing production-route metadata/body multiplexing regression was adjusted to the shared connection constructor and no longer needs a `true` child process.

Parent batch-window test target, if included in its single coordinated build/test plan:

```text
cargo test --manifest-path crates/mail-core/Cargo.toml --lib thunderbird_bridge::unix
```

This command was **not run**. Core dependency unit tests are not automatically executed by testing only the native app. No second build window is requested; if the parent's plan cannot execute core tests, record them as unexecuted rather than claiming pass.

## Honest assessment

**Good:** concrete production transport/lifetime changes with tests crossing the same constructor/request/reply seam; preserved multiplexing, source isolation, mutation uncertainty, and persistent clone/Outbox retention; stable recovery classification without changing public call signatures.

**Bad / incomplete:** Rust types and tests are unvalidated until the parent runs its batch. No actual Thunderbird 153/157, live provider, native click-to-paint, clone-side auth, or shutdown-descendant behavior was executed in this slice. No throughput/latency improvement is claimed. Tagged classification needs the native owner's integration; reliable NeedsLogin evidence needs extension support.

**Risks / blockers:** strict extension hash handshakes can turn a stale cached extension into an explicit restart-required startup failure. The new frame completion limit must be tested on supported hardware/runtime versions. A 64-call transport bound does not cap extension work already running after callers time out. OS reap stalls and group-escaping/daemonized descendants remain limitations. Existing private files intentionally rejected by the stricter symlink/hardlink policy need a safe user-visible repair path, never automatic deletion of drafts or Outbox.

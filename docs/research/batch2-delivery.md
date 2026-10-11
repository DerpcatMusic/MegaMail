# Batch 2 delivery and mailbox epochs

Status: source freeze for the worker owner. Rust compilation and runtime tests remain unexecuted. The parent owns the single batch build window.

Owned changes: `src/worker.rs` and this report. No fixture tool was added. This owner did not change cache, models, configuration, onboarding, core library exports, manifests, GTK adapters or native adapters. Existing worker edits from the prior offline-send batch remain intact.

## Final integration contract

The parent approved these additive variants. Existing GTK, Graph, JMAP, Thunderbird and demo variants remain unchanged. Only native direct IMAP submits `LoadBodyAtEpoch`.

```rust
MailRequest::LoadBodyAtEpoch {
    message_id: u32,
    path: String,
    uid: u32,
    expected_generation: u64,
}
WorkerEvent::MailboxEpochChanged {
    path: String,
    uid_validity: u32,
    generation: u64,
}
WorkerEvent::BodyAtEpoch {
    message_id: u32,
    path: String,
    uid: u32,
    generation: u64,
    body: String,
    check: Option<crate::models::SenderCheck>,
    has_attachments: Option<bool>,
}
WorkerEvent::BodyEpochStale {
    message_id: u32,
    path: String,
    uid: u32,
    expected_generation: u64,
    actual_generation: Option<u64>,
    reason: String,
}
WorkerEvent::SendUnknown {
    request_id: u64,
    reason: String,
}
```

Account identity remains bound by the existing worker callback. Native must reject body and metadata together when their generation differs from the selected account/folder generation. HTML transformation must retain this envelope. Network results carry `Some(check)` and `Some(has_attachments)`. Cached bodies carry the cached sender check and `None` for attachment state. `None` means unknown. Native must retain a known header attachment fact, not clear its icon.

`MailboxEpochChanged` follows committed scoped invalidation and precedes new scope results. `BodyAtEpoch.generation` is captured before the read. An old event queued before a reset retains its old stamp after the reset. `BodyEpochStale` is terminal. Native must stop the loading indicator. A known `actual_generation` can initialize the startup namespace once. Missing generations must not cause an automatic reload loop.

Unknown SMTP acceptance emits exactly one correlated `SendUnknown`, instead of `SendFinished(Failed)`. Untagged GTK callers receive an explicit uncertainty notice. The worker does not emit `Unsent` in that case. Definite failures retain `SendFinished(Failed)` or `SendFinished(Queued)`. `Queued` means persisted, not delivered. `Sent` means SMTP accepted, not recipient delivery. The parent confirmed GTK adapters and native mappings. Their compiled integration is still unverified.

## Cache APIs consumed

The implementation consumes the final signatures in [batch2-cache-integrity.md](batch2-cache-integrity.md):

- `queue_outbox_replacing` inserts exact MIME bytes and replaces a ready scoped origin atomically. The cache owner's final contract requires informed review before releasing a refused origin.
- `outbox_ready_items` excludes in-flight, uncertain, refused and accepted rows from automatic and ordinary explicit retry.
- `mark_outbox_before_transmission` grants the durable ready-to-in-flight claim. False or error means no SMTP call.
- `mark_outbox_accepted`, `mark_outbox_retryable`, `mark_outbox_refused` and `mark_outbox_uncertain` record the outcome.
- `outbox_delivery_state` and `delete_outbox_scoped` preserve account ownership for discard and retirement.
- `observe_uid_validity` and `mailbox_epoch` provide the persisted mailbox namespace and generation.

The worker does not call `retry_outbox_after_review`. That API requires an explicit informed review action. Ordinary Retry cannot release uncertainty. Editing an in-flight, uncertain or accepted source intent does not submit a replacement. It returns typed `SendUnknown` with a no-new-attempt reason.

The worker refuses in-flight discard. The cache owner confirmed the final atomic SQL exclusion for in-flight rows in `delete_outbox_scoped`. Another writer cannot delete active recovery bytes between the worker's state lookup and deletion. Ready, refused and uncertain rows still permit deliberate discard. Accepted rows permit Sent-copy retirement. The cache report is the authority for the method behavior.

## Delivery implementation

The immediate direct IMAP arm calls `send_imap_intent`. It builds MIME once, persists its bytes and envelope, and claims the durable intent before SMTP. Atomic replacement removes the prior two-transaction duplicate-ready crash gap. A failed insertion keeps the composition recoverable and never calls SMTP.

`transmit_outbox_intent` is the shared production seam for immediate sends and flushes. Tests replace only its external SMTP sender. Transport setup failures occur before submission. Explicit negative SMTP responses are definite failures. Lettre does not expose the DATA acknowledgement phase for generic I/O errors. The worker conservatively holds those failures as unknown, even when a real connection failure occurred earlier.

Acceptance is persisted before Sent append or retirement. A failed acceptance write leaves the in-flight hold intact. A failed or unavailable Sent append retains the accepted raw intent. These failures do not convert acceptance into a queued retry. Accepted raw copies have no new Sent-only repair action in this batch. They remain visible and cannot be resent through ordinary flush.

`flush_outbox_with_sender` reads only ready rows, preserves scheduling and account scope, and claims each row before transmission. Explicit Send Now overrides the schedule, not a delivery hold. `claim_connection_flush` is the actual `run_imap` connection gate. Reconnect resets that gate. Background flushes do not complete an unrelated composer request.

## Mailbox epoch implementation

Every direct `SELECT` and `EXAMINE` goes through `sel` or `exam`. Those wrappers call `observe_selected_epoch` before trusting the selected scope. The worker uses a separate account-owned cache connection on its current-thread runtime. Missing UIDVALIDITY or unavailable epoch persistence fails closed. Child IDLE watchers use the same account context.

The observer serializes cache invalidation against cache-lane reads. It emits the scoped change event after the cache commit. It clears old prefetch work and queues a header reload. A scope needs fresh headers after invalidation. Active legacy UID requests also carry a worker-local guard, but incoming legacy requests still lack caller generations.

The versioned cache lane reads a known persisted namespace without waiting for SELECT or unrelated network work. It captures the generation before reading body and sender metadata, and compares it after reading. This is a cached namespace snapshot, not proof of the current online server epoch. Unknown legacy namespaces do not produce a versioned body. Offline known-namespace body reads remain available.

The versioned network arm checks the caller's expected generation before polling the read future. It compares the captured generation after the read, before cache writes or publication. It preserves encrypted-body cache exclusions through `body_cacheable`. Body, trust, invitation, unsubscribe and attachment facts share one stamped completion. It emits no unversioned metadata event for that read.

Additional guards reject changed-epoch results for legacy single/batched bodies, explicit attachment reads, body/attachment prefetch, folder header reads and backfill. Existing reader priority, mutation barriers and owned-session background cancellation remain in place. The new request participates in reader priority. No physical selected A-to-B cancellation or new mail engine was added.

## Unexecuted Rust regressions

Fourteen new tests were authored in `src/worker.rs`. Eight asynchronous tests exercise the actual durable send/flush seams. One synchronous test exercises the observer and cached-body gate. Five asynchronous tests exercise versioned read completion and cache seams.

The parent can select `worker::tests::actual_`, `worker::tests::versioned_` and the inherited `worker::tests::offline_imap_` tests in its one build window. The new cases cover:

- Accepted request completion exactly once and separate failed Sent-copy recovery.
- Typed unknown acknowledgement completion, with no automatic or ordinary explicit retry.
- Actual automatic connection and Send Later gates, plus explicit Send Now.
- Definite transient failure versus held permanent refusal.
- Failure to queue, persistence failure before transmission and recoverable composition.
- Scoped discard races, false claims and another account's retained row.
- Restart with in-flight and uncertain SQLite intents still held.
- SELECT epoch change, old cached body exclusion and active request rejection.
- Warm known-namespace body and metadata while an unrelated future remains stalled, both online and offline.
- Missing epoch with one terminal stale response and no network-future poll.
- A queued old body after an epoch-change event and a reused UID, with immutable old body/metadata stamp.
- Network body/metadata with one captured stamp, and reset during a body read with no stale cache write.

These tests have not run. The helper tests do not launch the complete `run_imap` loop, keyring, profile startup, TLS, a real IMAP server or lettre SMTP protocol transport. The connection and scheduling gate is the same helper used by `run_imap`, but a full-loop synthetic protocol fixture remains absent. The queued-event test proves immutable worker stamps. Native's atomic rejection of body and metadata still needs its own test. The unknown attachment flag preserving an existing header icon is also a native test obligation.

Executed checks: targeted rustfmt formatting, full worker syntax parsing via `rustfmt --emit stdout` with child modules skipped, `git diff --check` for owned files, and eight Python source-contract assertions. The source assertions confirm shared production call sites and ordering only. They are not Rust tests or protocol evidence.

## Good, bad, risks and blockers

Good: direct IMAP now persists transmission intent, uses the same claim seam for immediate and flushed messages, and distinguishes unknown acceptance with a typed terminal event. Atomic replacement removes a known crash gap. The versioned reader protects reused UIDs without forcing warm cached bodies behind network work. Existing MIME, privacy and GTK variants remain.

Bad: no compilation, no executed Rust regression, no full worker-loop fake-server run and no provider verification occurred. Accepted raw intents need a separate Sent-copy repair workflow. This package does not prove exactly-once SMTP delivery or faster reader performance. Conservative generic I/O classification also holds some failures that were actually safe to retry.

Remaining unsafe legacy edges are explicit:

- `WorkerEvent::Messages` and `MessagesAppend` lack generations. Cached/older header events can arrive after a reset. Header/page requests and results need a captured scope generation, or native must retire their pending requests on the scoped change.
- Incoming `SetSeen`, `SetFlagged`, keyword, move, purge, export and attachment-mutation requests lack `expected_generation`. A request queued before a reset can arrive after fresh headers clear the local hold and name a reused UID. The local SELECT guard cannot reconstruct the caller's old namespace.
- `OutgoingMessage.draft_origin` has account/path/UID but no mailbox generation. Post-send and save-draft retirement calls to `delete_draft` can target an obsolete draft UID. A generation-bound draft origin and guarded retirement are required before claiming that edge is safe.
- References repair, attachment scanning and some legacy mutation cache writes are not all transactional generation-bound writes. A watcher or overlapping worker can invalidate a scope during their awaits. The new body guards do not establish safety for every old writer.
- Legacy `Body` and conversation `LoadBodies` events remain unversioned for compatibility. Native's approved selected direct reader must use `LoadBodyAtEpoch`. Existing versioned reader integration does not version every historical conversation event.

Parent integration blockers before the build window: compile the final shared variants and adapters, make native reject stale body and metadata atomically, make startup epoch initialization stop correctly, retain known attachment facts on unknown metadata, and exercise the final atomic active-discard SQL guard. Broader legacy header/mutation/draft generation work remains a follow-up, not completed F6 parity.

This owner used no Cargo, build, check, clippy, rustc or native/helper compilation. It accessed no private profile, credentials, provider login or live mail operation. It made no commit or push and started no process that needs cleanup. Graft was queried first and returned no matches. AGENTS, PRODUCT, DESIGN, architecture, the feature matrix and foundation mail/latency reports informed this work.

# Batch 2 cache integrity — cooperative API handoff

Status: owned implementation complete and re-frozen for parent integration/build gate after the final atomic in-flight discard guard. API names were published before implementation and delivery owner confirmed consumption through parent. No Rust compilation/tests were executed in this owner window.
Owned: `src/cache.rs`, `crates/mail-core/src/query.rs`, this report only.

## Worker integration contract (implemented signatures)

All methods below are additive; `OutboxItem` and existing GTK signatures stay intact.

```rust
Cache::queue_outbox_replacing(&self, account_id: u32, from_addr: &str, rcpts: &[String], recipients: &str, subject: &str, preview: &str, raw: &[u8], sent_path: Option<&str>, error: &str, send_at: Option<i64>, replaces: Option<u32>) -> Option<u32>
Cache::outbox_ready_items(&self, account_id: u32) -> rusqlite::Result<Vec<OutboxItem>>
Cache::outbox_delivery_state(&self, account_id: u32, id: u32) -> rusqlite::Result<Option<String>>
Cache::mark_outbox_before_transmission(&self, account_id: u32, id: u32) -> rusqlite::Result<bool>
Cache::mark_outbox_accepted(&self, account_id: u32, id: u32) -> rusqlite::Result<bool>
Cache::mark_outbox_refused(&self, account_id: u32, id: u32, error: &str) -> rusqlite::Result<bool>
Cache::mark_outbox_retryable(&self, account_id: u32, id: u32, error: &str) -> rusqlite::Result<bool>
Cache::mark_outbox_uncertain(&self, account_id: u32, id: u32, error: &str) -> rusqlite::Result<bool>
Cache::retry_outbox_after_review(&self, account_id: u32, id: u32) -> rusqlite::Result<bool>
Cache::delete_outbox_scoped(&self, account_id: u32, id: u32) -> rusqlite::Result<bool>
Cache::observe_uid_validity(&self, account_id: u32, folder_path: &str, uid_validity: u32) -> rusqlite::Result<MailboxEpoch>
Cache::mailbox_epoch(&self, account_id: u32, folder_path: &str) -> rusqlite::Result<MailboxEpoch>
// MailboxEpoch { uid_validity: Option<u32>, generation: u64, invalidated: bool }
Cache::open_at(db: &std::path::Path) -> rusqlite::Result<Cache>
```

`queue_outbox_replacing` validates the old scoped ID is **ready only**, then inserts new ready intent and deletes old intent in one transaction. Refused/uncertain/in-flight/accepted/wrong-account/missing replacement IDs reject the whole insertion. Refused replacement requires the separate informed review to release it first. This tightens the initial ready/refused proposal to the final coordinated contract. Worker must remove second deletion calls on this path. Existing `queue_outbox` wraps it with `None`.

`mark_outbox_before_transmission` atomically claims only a ready item and persists `in_flight` before any SMTP transmission. A false result means DO NOT send. Any persistence error means DO NOT send. Persist accepted/refused/uncertain explicitly; accepted means SMTP accepted, not delivered. `mark_outbox_retryable` is strictly in-flight → ready and is ONLY for a definite pre-acceptance failure, never an IO/timeout with unknown phase. Raw persisted in-flight is conservatively uncertain on restart (held in-flight is never eligible; UI explains uncertainty), without auto-reset. Refused is NOT automatically ready. In-flight and uncertain survive restart and NEVER appear in ready list. Only explicit informed user review can re-enable retry (not accepted rows). Existing `outbox_items` remains an all-state UI view; the worker MUST switch flushing to `outbox_ready_items`. Existing unscoped deletion remains for GTK signature compatibility; new worker code must use scoped deletion. `delete_outbox_scoped` atomically excludes in-flight state in its single DELETE, including other writers. It permits deliberate ready/refused/uncertain discard and accepted retirement. A false return on an active intent must produce a refusal/notice, not a claim that the item was discarded. Do not use legacy failure recording to reset delivery state. Persist acceptance before deleting/file-copy work; failure to persist acceptance leaves in-flight safely held.

First observed UIDVALIDITY on an epoch-unknown legacy scope invalidates that scope's UID-keyed derived rows conservatively. Same value is a no-op. Changed value increments scope generation and atomically deletes that account/folder's messages, bodies, sender checks, attachments, checked/scan/meta and refs-repair rows. Other accounts/folders, local tags, addresses, Outbox and local drafts outside the remote UID scope remain. Worker owner must compare every SELECT epoch before using UID cache or issuing UID mutations, reject stale in-flight writes, retire cached selections/reader results, and refetch headers after invalidation. `mailbox_epoch` is readable via the WAL reader. Missing server UIDVALIDITY must not be presented as validated.

QueryService::request_page_at_epoch<F>(&self, request: PageRequest, expected_epoch: u64, callback: F) -> Result<(), SubmitError>, F: FnOnce(PageResult) + Send + 'static, is implemented without new mandatory request/result fields. The cache generation is checked before/after SQL paging. Legacy requests additionally bind their caller search generation to first-observed cache generation; a stale cursor gets Superseded until caller increases generation. A reset after the final DB check still requires caller-side invalidation: UI event delivery is not an atomic DB transaction.

## Actual owned changes

- `src/cache.rs`: v21 additive `mailbox_epochs` and `outbox_delivery` tables, deletion trigger, transactional delivery state machine and replacement, transactional scoped UID invalidation, explicit-path opening, and 14 new production-seam Rust tests. Existing models and GTK method signatures remain. Existing failure recording does not reset side-state. Listing includes all rows and adds held-state explanations through the existing `last_error` field; ready-list returns errors instead of dropping malformed rows. Attempts retains the legacy counter rather than claiming an exact SMTP-transmission count.
- `crates/mail-core/src/query.rs`: epoch-aware additive submission; before/after namespace checks; caller-search/cache generation pairing for legacy cursors; the production loop is extracted into `run_queries` so synthetic thread tests use the same loop. Two new lane tests cover epoch reset and terminal cache failure. Queue bound/coalescing and existing request/result shapes remain.
- This report: API contract and integration/validation limitations. No worker, models, config, lib/module exports or manifests were edited. Standalone rustfmt also reformatted some pre-existing code within owned files.

## Recoverable migration and privacy behavior

Initialization now uses one IMMEDIATE transaction, checks user_version rather than substituting zero on failure, checks required columns, adds only genuinely absent optional columns, propagates migration/helper failures, and commits user_version only after all work succeeds. Previously ignored ALTER/cleanup/subject/name/attachment-seeding errors are no longer interpreted as a successful upgrade.

- Fresh empty version-0 database: initialize normally.
- Supported v6–v20 layout: upgrade in place. Headers, Outbox bytes, local tags, addresses and folder inventory survive. Render-derived invalidation remains for old render versions; remote UID-positive render products are reconstructible, not a blanket reset. UID-zero local sentinels are retained except for an explicit encrypted-content exclusion.
- Unsupported old layouts, unversioned nonempty files, future versions, malformed required layout: fail closed, preserve the file for explicit recovery. There is no automatic wipe or downgrade. A separate recovery/export UX remains parent/product work.
- Old outbox rows lack reliable submission-phase evidence, so migration holds them uncertain, including historically scheduled rows. This may require unnecessary review, but does not risk a blind duplicate send. Missing side-state in a current DB is also held, not ready.
- In-flight stays in-flight on reopening (another live worker may still own it). It is never ready; UI explanation conservatively says delivery uncertain. No startup resets it. Accepted raw bytes survive until explicit retirement/discard.
- UIDVALIDITY zero/empty folder is rejected. First online observation conservatively clears UID-positive copies in that scope. UID zero is not a valid remote IMAP UID and remains a local sentinel. Folder identity remains; its unread count is reset pending refetch. No Outbox, local-draft store, local tags, global address history or sibling scope is deleted.
- Encrypted/OpenPGP-derived bodies and attachment blobs are purged using existing PGP markers **before** old sender-verdict removal, fixing the old ordering hazard. Render version remains 19: this batch does not introduce a richer HTML or screenshot/body cache. Worker encryption exclusions remain essential, including unknown Thunderbird decryption eligibility; Thunderbird does not use these direct UID tables.
- File-backed cache uses FULL synchronous WAL; initialization/readiness/claim failure prevents a permission-to-send result. This is SQLite durability, not a guarantee against faulty storage hardware.

No credentials, private profiles/databases, provider/TB login, SMTP send, mail mutation, OAuth/TLS changes, remote resources or image URLs were accessed. Graft was queried first; it had no graph/hits. Source inspection used repository files. Relevant product/architecture/feature-matrix and foundation findings informed the scope. Rusqlite transaction rollback/API details were checked against installed 0.32.1 source; no dependency install/build was run.

## Executed validation (not native acceptance)

1. `rustfmt --edition 2021 --config skip_children=true src/cache.rs crates/mail-core/src/query.rs`: passed. This establishes formatter parsing, not Rust type checking.
2. `git diff --check -- src/cache.rs crates/mail-core/src/query.rs docs/research/batch2-cache-integrity.md`: passed.
3. Python stdlib SQLite 3.53.4 smoke test extracted the actual SCHEMA and relevant SQL literals from owned production functions. Six groups passed: account isolation/claim/held readiness; delivery-state transaction rollback; SQLITE_FULL queue rollback; replacement rollback; scoped invalidation rollback/success preserving siblings/local scope; actual schema SQLITE_FULL rollback and retry with migrated sends held. A follow-up extracted cleanup SQL test passed encrypted-blob purge ordering and local UID-zero preservation. These tests execute production SQL, but orchestrate it in Python; they do **not** execute Rust APIs or replace the deferred Rust production-seam tests. SQLITE_FULL is induced with max_page_count, not by filling the machine's disk. Final follow-up smoke checks passed the tightened ready-only account-scoped replacement guard (including refused/in-flight/uncertain/accepted rejection) and SQLITE_FULL replacement rollback retaining the original raw bytes and sole ready intent. A final safety smoke test extracted the scoped DELETE SQL and used two separate synthetic file-backed WAL connections: active in-flight deletion and wrong-account deletion were blocked; raw/state remained; ready/refused/uncertain/accepted retirement and transactional side-state cleanup succeeded.

## Unexecuted production-seam Rust tests / parent gate

`cache::integrity_tests` (14 tests): scoped invalidation and local/sibling preservation; failed purge rollback/recovery; file-backed in-flight/accepted restart retention; second-connection claim/reset/active-discard exclusion and explicit uncertain discard; definite transient versus permanent refusal; failed claim rollback; atomic replacement and held/wrong-account guards; SQLITE_FULL queue/replacement/outcome/upgrade behavior; legacy hold migration; failed ALTER/seed migration rollback/recovery; unsupported/future version preservation; encrypted cleanup ordering.

`query::tests::query_lane_supersedes_search_and_cursor_after_uid_namespace_reset`: uses explicit synthetic file-backed Cache/WAL reader, actual bounded QueryService submission and production run_queries; validates old cursor/epoch rejection, fresh generation and unaffected account. `query::tests::failed_cache_open_completes_each_accepted_query_without_fake_empty_success` exercises the production loop's error branch. Existing cache/query tests and GTK compilation remain required.

Parent owns the one authorized batch build/test window after every owner freezes. No Cargo, build/check/clippy/rustc, helper/native compilation, commits or pushes were run here. Rust type/behavior tests, real file-device disk-failure, abrupt process-kill/WAL recovery, worker/GTK event wiring, and provider acceptance behavior remain unverified.

## Integration obligations and honest assessment

**Good:** a narrow persisted state seam replaces blind eligibility; scoped invalidation and migration operations are transactional; failure injection is aimed at the same SQL and Rust APIs used in production; public UI shapes remain compatible. No engine rewrite or destructive default recovery was added.

**Bad/limits:** conservative migration holds all ambiguous legacy sends and can inconvenience users. Very old layouts now require explicit recovery instead of being silently deleted. Legacy Vec-returning UI APIs still cannot express a storage error directly; new ready-list is fallible. Ordinary legacy callers alone do not establish a correct mail flow. No performance measurement was made.

**Risks / release gates:**

1. Delivery owner must use queue-before-transmission, atomic replacement, ready-list plus durable claim, explicit outcome transitions, acceptance-before-copy/deletion, and scoped retirement. Every scheduled/offline/direct path must use the same seam. Do not infer retryability from text or mere UI visibility. IO/timeout without SMTP-phase evidence is uncertain; explicit pre-acceptance refusal can be retryable or permanently held.
2. Parent reports the coordinated `WorkerEvent::SendUnknown` GTK/native mappings are source-wired; they still require the batch compile/test gate. Keep unknown attempts out of queued/delivered claims. The existing all-state list explains uncertainty, including crash-held in-flight. Ordinary Retry cannot release it. No review UI was added here.
3. **Review requires a quiescent account send lane.** Compatibility bool APIs carry no attempt nonce; releasing an actively transmitting row could let an old completion affect a later attempt. The separate informed-review action must be blocked while an attempt is active; it may release crash-held state only when there is no live transport. This is not solved by the cache alone.
4. Worker SELECT/EXAMINE observation and cache-lane validation must compare epoch before using UID-derived content or UID mutations; retire stale selections/results and reject old-generation writes. Existing unguarded cache-write signatures are preserved for GTK; owner/parent must gate them by session epoch. DB invalidation without these gates is not end-to-end F6 completion.
5. Parent/native integration must increase caller search generation on epoch events and discard already-delivered old-generation callbacks. Query's last DB check cannot be atomic with UI delivery. Opt-in `request_page_at_epoch` offers explicit checking; no new mandatory PageRequest/PageResult fields were introduced.
6. The single batch compile/test gate is still a blocker to claiming these source changes work in Rust. Recovery/review UX, broader offline retention/cache-at-rest policy, actual worker transport crash tests, and TLS/OAuth/provider assurance are not implemented or verified in this owner slice.

No shipped-feature, exactly-once SMTP, fixed real-user mail problem, provider compatibility, or speed claim is made by this report.

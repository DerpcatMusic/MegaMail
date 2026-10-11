# Batch 2 frozen-slice review round 1

Source baseline: d4dbd2bba14f474a33870269cc15bd98376ff360. Review includes uncommitted tracked changes and full new files. Still-changing native/worker/auth/draft/renderer/contacts/activation source was excluded; this is not a full-batch certification. Both reviewers used Pi GPT-6.1 Sol/high-thinking. No builds or private/live-provider operations.

## Standards — frozen batch 2

**Baseline:** `d4dbd2bba14f474a33870269cc15bd98376ff360`; HEAD unchanged, no later commits. Reviewed scoped working-tree changes and manually read frozen new modules/tests. Mutable integration files were not assessed as settled. No edits, builds, profiles, credentials, provider operations, or commits.

### Hard documented-standard violations
**None established in the frozen scope.** This is a source-review result, not runtime approval.

The changes respect AGENTS’ “Shared-source edits must preserve GTK callers”: existing cache/request/result shapes remain intact, and epoch-aware submission is additive. No new engine/framework replacement or render-path I/O was introduced in these modules. No frozen presentation changes conflict with the preserved Zeron shell.

### Judgment calls
**P3 — possible Primitive Obsession, nonblocking:** `src/cache.rs:transition_outbox` expresses delivery phases through `from: &str, to: &str`; callers include `"ready", "in_flight"` and `"in_flight", "accepted"`. A future misspelled source phase would silently produce `false`, while destination mistakes become database errors. Current literals are consistent; this is maintenance risk, **not a verified delivery bug**. Optional minimal improvement: a private delivery-phase enum with one SQL-string mapping, preserving public compatibility APIs. No broader abstraction/refactor is warranted.

### What looks good
- Transactional migrations/replacement/claims; ambiguous legacy sends remain held, not automatically replayed.
- Scoped UID invalidation and before/after query epoch checks.
- Thunderbird bounds, whole-frame deadlines, independent reply routing, dedicated process-group cleanup, and reader wake/join.
- Actual `OutgoingMessage`/`FilterRule` reuse; compose metadata preservation and explicit HTML refusal.
- Exact-scoped triage correlation, admission distinct from success, and uncertainty distinct from cancellation.
- Parameterized search, explicit coverage, and conservative missing-body semantics.

### Risks / blocked acceptance
These are documented integration obligations, not additional frozen-scope violations: informed retry requires a **quiescent send lane** because cache APIs lack attempt nonces; worker/UI epoch gates must prevent stale writes and callbacks; triage requires genuine typed terminal results; search needs eligible plain-body projection; productivity persistence and rule preview must execute off-thread.

Batch-2 Rust tests remain authored but unexecuted pending the parent’s grouped build window. Reported Python/Node SQL/kernel/static checks do not prove Rust behavior, GTK compatibility, native activation, provider operation, faithful inline HTML, or product latency. Prepared modules are not shipped features.

**Summary:** 0 hard Standards violations; 1 nonblocking heuristic. Spec findings remain separate.

## Spec review — frozen batch 2

Compared against `d4dbd2bba14f474a33870269cc15bd98376ff360`, including full reads of the specified new files. HEAD remains baseline; no intervening commits. No edits, compilation, profiles, credentials, or provider operations.

### Wrong implemented behavior

**P2 — prepared search predicate/model disagreement for legacy NUL-bearing headers.**
`crates/mail-core/src/search.rs:SearchQuery::{matches,sql_predicate}` uses byte-substring matching versus SQLite `LIKE`. A retained subject `before\0secret` matches `subject:secret` after `Cache::load_messages` applies `Message::scrub_nuls`, but SQL `LIKE` stops at NUL and rejects it. Negation reverses the discrepancy. Existing cache comments explicitly recognize legacy NUL-bearing rows.

Requirement: feature-matrix **S1** (`email-client-feature-matrix.md:356`) requests “filters/date/recipient/attachment terms”; the prepared interface additionally promises the same semantics for model and SQL evaluation.

**Smallest fix:** require a NUL-normalized header projection matching `scrub_nuls` before SQL filtering, or implement an equivalent bounded predicate. **Proof requested:** a production-seam Rust equivalence test using a legacy cached subject, positive and negative queries, and the actual loaded model. An in-memory Python SQLite probe confirmed the `LIKE` behavior; Rust execution remains pending. This is a prepared-interface defect, **not evidence of a shipped full-search regression**.

### Missing/partial requirements — integration gates, not regressions

- **F2/F5/F6:** the matrix requires “restart/late response handling,” “no automatic retries of ambiguous acceptance,” and “generation/epoch stale response rejection.” Prove native tagged-error mapping, worker claim/outcome consumption, source-captured epoch checks, and UI invalidation. `Cache::retry_outbox_after_review` requires a **quiescent send lane** because its compatibility API has no attempt nonce.
- **S1/T1:** SQL projection/paging and actual coverage remain unwired; triage requires typed, exact per-item terminal outcomes. Generic settled events cannot establish success.
- **C1/R1/U1:** composer dirty/recovery/input synchronization, serialized private-library writes, dry-run coverage labels, guarded action/focus dispatch, Linux activation and accessibility checks remain acceptance gates. Deferred scheduling needs the matrix’s “scheduler/server policy, not just a button.”

### Good / scope / blocked

Source shows transactional migrations and scoped delivery claims, held legacy Outbox rows, JS-safe bounded Thunderbird requests, admission distinct from mutation success, and bounded mailto prefilling without send authority. **No scope creep identified.**

Batch-2 compilation/runtime acceptance remains blocked on the parent’s single grouped window. Earlier results and Python/static checks do not prove provider login, inline HTML fidelity, or fetch-to-view latency; mutable integration files were not reviewed as settled.

## Responses and unresolved gates

- Standards: private typed delivery phases are a nonblocking maintenance improvement, not a verified send bug. Avoid a broad refactor during this batch.
- Spec: the confirmed search NUL model/SQL disagreement is assigned to a new scoped implementation owner, limited to core search and a new correction report. Regression execution waits for the integrated batch window. No production full-search completion claim exists yet.
- All integration/runtime/registration/performance gates listed by reviewers remain tracked in implementation-batches.md. Frozen modules need real callers, not just declarations.
- A later review round must include original brief, these findings and responses, the normalization repair, all integrated source, and unresolved objections. It must use new delegated task IDs, not restart these completed child threads.

## Frozen-scope review

**Baseline:** `d4dbd2bba14f474a33870269cc15bd98376ff360`; HEAD remains unchanged. Reviewed scoped working-tree changes, untracked modules/tests, required handoff reports and repository contracts. Mutable main/live files were context only. No edits, builds, private-profile/credential access, provider operations, installer execution or commits.

### Confirmed correctness defects

**P2 — HTTP connect can exceed the remaining authorization budget.**
`crates/mail-core/src/provider_auth.rs:321–325 — ProviderHttp::agent`

The adapter sets `.timeout(timeout.min(HTTP_TIMEOUT))` but leaves ureq’s default connect timeout intact. The locked **ureq 2.12.1** initializes `timeout_connect` to **30 seconds**; its production `stream.rs` chooses that connect deadline instead of the overall request deadline.

**Concrete case:** two seconds remain in an attempt; DNS returns a provider address whose TCP SYNs are silently dropped. Connection establishment can occupy approximately 30 seconds, despite the two-second remaining budget and advertised 20-second operation cap. Later checks prevent persistence, but they do not enforce the blocking-operation bound. This is separate from the already-documented DNS limitation.

**Smallest fix:** explicitly set the connect timeout to the same capped remaining budget. Keep post-operation cancellation/staleness checks.

**Regression requested:** exercise the actual `ProviderHttp`/ureq adapter against a controlled synthetic blackholed connection, with short remaining budgets. Verify bounded completion and no subsequent identity/probe/store operation. `FakeHttp` returning promptly cannot establish this.

---

**P2 — post-rename recovery failure cannot be retried with the unchanged snapshot.**
`apps/megamail/src/draft_recovery.rs:503–539 — RecoveryStore::{save,atomic_write}`

**Concrete interleaving:**

1. Save generation **G**; temporary-file write and file sync succeed.
2. Rename succeeds, installing generation G.
3. Directory `sync_all()` fails; save returns `RecoveryError::Io`, and the writer publishes `Failed`.
4. Storage recovers; the user retries the unchanged generation G.
5. `save` reads the installed record and rejects `G <= old.generation` as `StaleGeneration`.

Thus the documented retry after uncertain directory durability cannot acknowledge the same composition as durable. The validated replacement exists; this is **not proof of lost bytes**, but the dirty/error state cannot resolve by the promised unchanged-snapshot retry.

**Smallest fix:** distinguish post-rename durability uncertainty from precommit failure and provide a checked resync/readback retry for that exact installed generation. Do not generally permit arbitrary equal-generation replacement.

**Regression requested:** inject failure at the actual post-rename directory-sync seam, then retry through the production store/writer path after removing the fault. Require successful durability acknowledgement, exact restored fields/attachment bytes, and continued rejection of genuinely stale updates. The existing pre-rename ENOSPC test does not cover this state.

### Standards, caveats and gates

No additional hard documented-standard violation or confirmed exploitable renderer/activation defect was established.

Positive source facts: provider protocol/resource separation and authenticated-primary identity checks; serialized commit finalization with explicit cancel-race semantics; owned recovery attachment leases; scoped contact revisions; checked ICS arithmetic/UTF-8 boundaries; real SO_PEERCRED activation; renderer request-hook acknowledgement before sender-document load, exact capabilities, CSP and network namespace containment.

Keep these as **integration/deferred obligations**, not newly discovered bugs: roster serialization and committed-token rotation, queued/unknown-send reconciliation, exact-generation deletion/older-copy repair, causal invitation epochs, MIME-scoped CID transport, helper packaging, and inline GPUI/input/accessibility work.

All new Rust/C tests remain unexecuted here. No engine security, provider login, faithful rendering, teardown or latency acceptance is implied by source inspection or static scanners.

**Summary:** two P2 source defects for parent-assigned correction; no P1 established. Preserve the single grouped verification window.

## Parent response and ownership

Both confirmed P2 defects received new, narrowly scoped correction tasks. Their source repairs are now frozen in batch2-provider-budget-repair.md and batch2-recovery-resync-repair.md. A new delegated round-2 review includes this original brief/findings, both repair responses and unresolved gates; no runtime acceptance is claimed. The HTTP owner may change provider_auth.rs and its contract tests only; the recovery owner may change draft_recovery.rs only. Each adds a repair report. The serial native integrator owns main/live/Thunderbird adapter and new integration modules, not these producer modules. No compilation is authorized before whole-batch freeze.

Provider connection-budget repair must retain TLS, fixed authenticated endpoints and post-operation cancellation checks. Its regression must use the actual adapter with controlled local transport, not only FakeHttp.

Recovery repair must permit checked durability acknowledgement for the exact uncertain installed snapshot, not general equal-generation overwrite. It must inject failure after real rename, retry through the real store/writer, verify complete fields and attachment bytes, and keep stale or different equal-generation updates rejected.

Native integration keeps recovery failures dirty. It uses worker-local pending-token ProviderProbe rather than old stored-keyring probes; its registry AccountStore saves through stable-ID reservation and checked roster serialization. HTML helper availability remains release-gated by default; trusted developer opt-in is not production security acceptance. These statements describe requested/current source wiring, not compiled or runtime acceptance.

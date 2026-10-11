## Round 2 — Good / Bad / Blocked

**Baseline:** `d4dbd2bba14f474a33870269cc15bd98376ff360`; HEAD unchanged. Reviewed the scoped tracked/untracked producer changes, regressions, required reports and original provider/draft/cache handoffs. Read repository architecture/development/validation/design contracts and applicable Rust/module-design guidance. Graft was queried first and returned no matching nodes. Mutable native integration files were context only.

No edits, builds, test execution, private-profile/credential access, provider operations, mail mutations, installers or commits.

### Good — original P2 dispositions

**HTTP connect budget: SOURCE-RESOLVED; runtime acceptance pending.**
`crates/mail-core/src/provider_auth.rs — ProviderHttp::agent`

One `min(remaining, 20s)` budget now configures connect/read/write/overall timeouts. Inspection of locked ureq **2.12.1** confirms this removes the overriding default 30-second connect timeout; response-body reads retain the overall deadline wrapper. Zero duration remains an expired deadline, not an unlimited-timeout setting. Fixed catalogue HTTPS endpoints, default TLS validation, disabled redirects and post-operation attempt checks remain.

The four authored regressions delegate to actual `ProviderHttp`, not fabricated HTTP results. Header/body fixtures use numeric loopback addresses, finite fixture deadlines, acceptance confirmation, minimum-stall assertions and Drop-based stop/join cleanup. The ignored connect regression checks loopback-only isolation and calibrates actual SYN timeout before exercising short-budget authorization attempts. **Accepted-connection stalls do not establish connect coverage.**

**Recovery unchanged-generation retry: SOURCE-RESOLVED; runtime acceptance pending.**
`apps/megamail/src/draft_recovery.rs — RecoveryStore::{save, atomic_write, owns_uncertain_installation}`

After actual rename, the store pins its installed inode and retains bounded uncertainty authority. Equal-generation retry requires that same store-owned installation plus exact full serialized record/attachment-byte equality. Retry performs real directory resync and final identity/readback checks without replacement. Different same-generation content, older generations, reopened stores and another writer’s identical-byte replacement cannot borrow permission. Successful newer writes retire uncertainty. The marker adds no retained plaintext composition buffer.

The four regressions cross actual store seams; the principal case also crosses the production writer queue/event path. They cover repeated post-rename failure, unchanged retry, hidden fields, changed attachment bytes, restore fidelity, inode preservation and stale rejection. Writer admission permits unchanged-generation retry after a completed failed save.

### Good — adjacent scoped changes

- `src/cache.rs — Cache::read_connection_with` configures a separate read-only WAL reader before publication. Failure drops it without migration/reset. Existing `read_connection` remains compatible; neither shared Cache nor GTK imports core search.
- `crates/mail-core/src/query.rs — run_queries` registers predicates on the background initialization path. Initialization errors feed failed-page results rather than successful empty pages.
- `apps/megamail/src/compose.rs — PreparedDraft` now carries `Option<AttachmentLease>`; both existing producer constructors initialize it to `None`. Recovery preparation retains an explicitly supplied lease.

### Bad / incomplete

**No new confirmed correctness defect or hard documented-standard violation established within this frozen scope. Neither original P2 remains source-open.**

One nonblocking coverage limitation: the query registration-failure test exercises the real factory’s failure and then supplies that resulting `Err` to `run_queries`. It checks callback/error behavior, but does not induce failure inside `run_queries`’ own registrar invocation. Source ordering is correct; this is not evidence of a production bug.

Recovery retry still requires the **same live store/writer**, intact attachment sources or lease, and the documented single-writer policy. Descriptor/metadata checks do not establish hostile same-user race resistance or multiwriter linearizability.

### Blocked / acceptance gates

All new Rust regressions remain uncompiled/unexecuted. Parent retains the single grouped verification window, including the separately gated isolated SYN-blackhole run; ordinary test execution leaves it ignored.

No runtime proof is supplied for native retry/dirty-state acknowledgement, consumer lease lifetime, TLS/probes/provider registrations, roster serialization/token rotation, queued/unknown-send reconciliation, exact-generation deletion/older-copy repair, causal epochs, helper security/fidelity/inline GPUI, teardown or latency.

**Recommendation:** accept both bounded P2 source repairs for grouped verification—not as shipped features or runtime acceptance.

## Parent disposition

Both original P2 findings are accepted as source-resolved for the upcoming grouped verification window. No new confirmed defect was reported in this frozen scope. Neither source review nor formatting closes runtime acceptance.

The parent will run the core provider contracts, including normal transport stalls, and separately attempt the ignored SYN-blackhole case only inside its fresh rootless loopback-only network namespace. Namespace/calibration failure remains blocked, not a pass. The parent will also run new recovery tests and the existing store/writer suite, then verify native dirty retry and attachment lifetime after the serial native owner releases.

The query registration-failure coverage limitation is retained as nonblocking: the factory error is real, but the test passes the resulting Err into run_queries rather than faulting its registrar internally. No speculative API refactor is authorized to remove that limitation.

A full integrated Standards/Spec review must include native caller wiring once frozen. The earlier unversioned header/mutation/session/draft-origin residuals, source/renderer eligibility, helper security/fidelity and real-provider gates remain unresolved, not silently accepted here.

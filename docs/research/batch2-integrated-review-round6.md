## Standards

**0 new confirmed hard breaches in bounded R6.**

HEAD remains `d4dbd2bba14f474a33870269cc15bd98376ff360`. Review used the working-tree diff and untracked inventory, not the empty committed diff. Graft returned no matches. Current inventory contains **98 untracked paths**.

Comparing all **574 source/embedded inputs** with `freeze-correction3.json` found changes only in:

- `apps/megamail/src/live.rs`
- `apps/megamail/src/thunderbird_adapter.rs`

The new metadata branch modifies retained state only; it introduces no filesystem, network, keyring or parsing work on the GUI thread. The LRU correction changes tests, not production accounting or limits.

`compose_session.rs::Capture.finished_send` preserves the full `u64` request identity through `main.rs` cleanup submission/retry. `reader_session.rs::OriginalBody::fmt` exposes size/classification, not source HTML. These corrections preserve correlation and diagnostic privacy.

Verifier/QA scripts retain their explicit execution, receipt and isolation gates. They do not turn source checks or observations into runtime acceptance. Existing P3 heuristics remain deferred; no optional refactor requested.

## Spec

**No new concrete P2 found in the assigned corrections.**

### Failed-import identity metadata — SOURCE-RESOLVED; execution pending

Producer evidence: `thunderbird.rs::import_identity` obtains a trimmed, bounded, control-free configured address from allowlisted preferences. `account_session.rs::restore_with` and `thunderbird_adapter.rs::import_selected` carry that address in outcomes for the selected source identity, including connection failures.

Consumer: `live.rs::install_thunderbird_ready` resolves the outcome’s exact **profile path + source account ID** through the durable store, requires Thunderbird ownership, and fills **only an existing blank address** from a nonblank hint. It does not install a sender, change readiness, overwrite a known healthy address, or attach failure metadata to a direct owner.

`can_use_account` still requires a sender and excludes reconnectable accounts. `provider_setup.rs::load_admission` reloads durable ownership and uses the configured addresses for duplicate checking—not authentication. Blank-address outcomes remain fail-closed. Saved references and IDs remain intact, so Retry does not require another import.

The strengthened actual-install regression and new blank-address/healthy-sibling regression cover the corrected consumer but **have not executed after this patch**.

### Protocol, cancellation and privacy corrections — accepted at source

`src/worker.rs::reject_unsupported_epoch_body`, called by Graph/JMAP/POP3/demo dispatch, emits one exactly correlated `BodyEpochStale`, with `actual_generation=None`, without I/O, fabricated authority or legacy-body downgrade.

`provider_auth.rs::ProviderProbe::validate` adds only the explicit SMTP executor type; TLS, deadlines and cancellation logic remain unchanged.

### LRU failure — TEST EXPECTATION RESOLVED

`thunderbird_adapter.rs::body_cache_is_lru_bounded_by_entries_and_memory` now counts the **13 retained original bytes** alongside the former 31 bytes. Production `byte_len`, entry/memory limits and oversize refusal remain unchanged.

Existing logs establish **599 ordinary core passes**, one isolated synthetic connect pass, and **264 native passes / 2 failures** before these latest repairs—not a green current batch. Native-build/license gates and real UI/provider/engine acceptance remain pending.

**Read-only review released. No edits, test/build execution, launches, credentials, provider or mail operations performed.**

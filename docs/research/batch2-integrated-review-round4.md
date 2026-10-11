## Standards

**0 new confirmed hard breaches.** Read-only review used the working-tree diff against `d4dbd2bba14f474a33870269cc15bd98376ff360` and untracked inventory, not the empty committed diff. HEAD and all four supplied source hashes match. Current inventory contains **90 untracked paths**.

The admission repair keeps persistence/probes off the UI thread, preserves stable-ID reservation-before-save, and leaves Core CLI/public registry contracts unchanged. Managed reader state is prepared outside rendering; existing Zeron presentation and action guards remain.

Verifier inspection found no new unsafe command classification or false runtime-proof claim: separate matching receipt, explicit asset namespaces, serial prerequisites, scoped cleanup, and `release_accepted: false` remain. Its ten contracts mock command execution/signals; they do **not** establish compiler or kernel-cleanup acceptance.

**Nonblocking smells only:** the existing prose-coded saved/reload-failure outcome and Boolean send/save lease namespace remain P3 heuristics. No optional refactor is requested.

## Spec

### Prior P2 dispositions

- **Mixed Thunderbird/password duplicate admission — SOURCE-RESOLVED.**
  `main.rs::connect_account` and `begin_provider_authorization` call shared normalized `provider_setup::check_new_account`. Actual password dispatch reaches `add_password → add_password_with`: under `DIRECT_WRITE`, `load_admission` reloads TB IDs, checks cross-backend ownership, requires verified nonblank live addresses, and reloads direct profiles through allocation-free `load_profiles_existing`. Admission precedes probe and `save_direct` reservation/persistence. OAuth rechecks through `RegistryStore::save_new`. Six new production-admission closure tests cover duplicate refusal, distinct admission, unknown addresses, fresh direct duplicates and failed probes; **unexecuted**.

- **FocusReader wrapper wiring — SOURCE-RESOLVED for the steady-state selected body.**
  `SemanticTextTarget` retains the actual `Entity<TextViewState>`; selected rendering uses `TextView::new(&target.state)`, and the action reads that state’s `focus_handle`. However, the lifecycle regression below prevents accepting the repair across all reachable transitions.

- **Legacy-NUL search disagreement — SOURCE-RESOLVED.** Shared bounded predicates preserve exact NUL→SPACE and Unknown semantics; `QueryService` installs them through `Cache::read_connection_with`.
- **Provider remaining-budget override — SOURCE-RESOLVED.** `ProviderHttp::agent` applies the same remaining/20-second cap to connect/read/write/overall timeouts.
- **Unchanged-generation postrename recovery retry — SOURCE-RESOLVED.** `RecoveryStore::save` permits only store-owned, exact-installation/exact-byte directory resync with final readback—not equal-generation overwrite.

### R4 P2 — UNRESOLVED: helper retirement invalidates native focus without refreshing its target

**Source:** `main.rs::open_original_formatting` (1498–1550), `reader_text_identity` (1635–1656), selected-body rendering (5248–5264), and FocusReader (7280–7301); `reader_session.rs::Session::{retire,poll}`.

**Input/interleaving:** Select a loaded HTML message, then click **Original formatting — temporary window** in ordinary mode. The button is enabled when `selected_original` exists—even without developer helper permission. `open_original_formatting` calls `retire`, incrementing the generation. Default helper refusal then calls `retire` again. Neither path refreshes `reader_text`.

The retained target has generation G; current identity has G+1/G+2. Rendering therefore switches to unmanaged `TextView::html`, while FocusReader rejects the retained target and does nothing. This persists until another live/mode transition refreshes it. Helper close/fallback polling has the same missing refresh.

This is a concrete native fallback regression, **not** a demand to enable the blocked engine.

**Smallest fix:** synchronize the retained semantic target after every helper-generation transition, including immediate retirement, refusal, polling closure/fallback and view retirement. Preserve mailbox/body correlation and no-render-I/O.

**Production-seam regression:** With fictional loaded HTML and developer preview disabled, invoke the actual original-formatting refusal path; assert that the rendered managed state and FocusReader target still agree. Repeat helper closure/fallback. Actual focus/SelectAll/Copy remains a separate isolated GPUI gate.

Provider, engine/CID/inline, UI, clipboard/A11y, shutdown and performance execution acceptance remain excluded. No tests, builds, launches, credentials or mail operations were performed. **One bounded correction remains before the final freeze receipt.**

## Parent corrective disposition

Parent added retire_reader_session to refresh the actual current native TextView consumer immediately after helper retirement. It is used on original opening, refusal and hidden-view retirement. Timer polling now compares helper generation and refreshes after closure/fallback/disconnection changes. Existing live/body retirement retains its existing post-projection refresh; no render-path parsing/I/O or body-version restamping is introduced. Helper generation exhaustion no longer disables native text identity: only the external helper is denied.

Six source-only wiring checks are in tools/test-megamail-reader-focus-wiring.py. An initial test expected the trait-style focus_handle(cx) spelling, but the pinned vendor exposes inherent focus_handle(); that test was corrected to the actual call, not production changed to satisfy the check. All six source checks pass. These checks do not execute Rust, create a GPUI window or prove focus/SelectAll/Copy. The recommended actual default-refusal/closure GPUI regression remains an execution gate; native test-support is not enabled and no fake window proof was substituted.

Parent main.rs source syntax and whitespace checks pass. A new read-only R5 source review must disposition this bounded lifecycle correction before the final freeze receipt. All implementations and other prior source corrections remain released. No grouped compilation yet.

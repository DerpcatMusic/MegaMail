# Batch 2 — native Spec R3 corrective source release

Status: **source prepared, frozen and released to parent; not compiled, Rust-tested, runtime-accepted or shipped.** Baseline remains `d4dbd2bba14f474a33870269cc15bd98376ff360`; the pre-existing working batch is not this owner's attribution ledger.

## Ownership and constraints

Changed only `apps/megamail/src/main.rs`, `provider_setup.rs`, `reader_session.rs`, and this new report. Direct rustfmt used `skip_children=true`. No live/adapter/core/worker/vendor/manifests/lockfiles/driver/prior-report edits. No Cargo/compiler/helper builds, app/private-profile launches, credential/key reads, provider requests, browser authorization, mail mutation, installation, handler changes, commits, or additional owners. Graft was queried first; no usable graph was available. Relevant source/contracts and pinned vendor APIs were inspected. This is a bounded corrective implementation, not a fresh comprehensive review of every batch file.

## Standards

The integrated review supplied **zero hard breaches**. Outcome-string and Boolean-lease P3 suggestions were not expanded into optional refactoring. Existing Zeron presentation, semantic fallback, action guards, attribution and package boundaries remain unchanged. Formatting/source checks are not type or standards acceptance of the entire batch.

## Spec — two confirmed P2 source repairs

### Mixed Thunderbird/password duplicate admission

Previous input: a connected Thunderbird mailbox and a newly entered direct password account with the same normalized address, without a direct-profile duplicate. `MailApp::connect_account` could probe and durably save before `LiveMailbox::add_profile` rejected it.

Now `connect_account` and `begin_provider_authorization` use `provider_setup::check_new_account` against direct profiles plus live verified addresses. The authoritative password consumer is `add_password` → `add_password_with`: acquire existing `DIRECT_WRITE`, reload the saved Thunderbird inventory, validate cross-backend IDs, match every saved TB ID to a nonblank verified live address, reload current direct profiles, check admission, then probe, then call unchanged `account_registry::save_direct`. Missing/unverified TB rows fail closed. `load_profiles_existing` avoids pre-admission ID allocation. `save_direct` retains reserve-before-credential-save and stable-ID APIs. Core password/CLI APIs were not changed.

OAuth uses the same policy before transaction/browser/probe work and again in `RegistryStore::save_new` under serialization immediately before commit persistence. Ready/pending grants remain worker-local; cancellation, TLS/probe errors and saved-versus-connected/reload-failure states are preserved.

TB email evidence is the live metadata snapshot captured while UI import/admission exclusion holds, matched to the freshly loaded saved IDs. No new private preference scan, credential import or provider query was introduced. This is not cross-process roster/token-rotation coordination or a new source-identity protocol.

### Focus the actual selected native text consumer

Previous interleaving: Ctrl+Alt+R focused an independent reader-scroll wrapper; the body `TextView::html` owned another handle and its own Copy/SelectAll context.

Now `MailApp::reader_text` retains `SemanticTextTarget.state`; selected-body rendering uses `TextView::new(&target.state)` and FocusReader uses that state's actual `focus_handle()`, through unchanged `desktop_actions::focus_pane` guards. Correlation includes account/folder/UID, direct body version, reader generation, plain mode and quote mode. Body/selection/epoch, collapse, preference and mode transitions refresh or clear the target outside render. Unchanged content reuses state; replacement content/modes get new state so old selection cannot survive. Missing/collapsed/mismatched targets cannot receive FocusReader. Other conversation members retain existing semantic rendering.

`ReaderBody::markup` selects the exact existing semantic/plain/quote strings. No MIME/sanitizer/render I/O was added. Pinned vendor `TextViewState` bounds synchronous parse input to 4 KiB and parses larger replacements off-thread; this is source evidence, not a UI-thread performance measurement. Native HTML/helper release remains closed.

## Authored tests — 10 new, zero executed

Provider (6):
- `actual_password_admission_rejects_mixed_tb_duplicate_before_probe_reserve_or_save`
- `actual_password_admission_accepts_distinct_mailbox_and_preserves_existing_ids`
- `actual_password_admission_blocks_unknown_tb_address_or_new_unverified_saved_id`
- `actual_password_admission_rejects_reloaded_direct_duplicate_before_probe`
- `actual_password_admission_does_not_persist_a_failed_probe`
- `shared_oauth_admission_policy_fails_closed_on_unknown_address`

Reader (3):
- `semantic_target_retention_reuses_only_unchanged_body_and_mode`
- `semantic_target_retention_rejects_uid_reuse_and_source_generation_changes`
- `semantic_target_retention_rejects_equal_uid_in_other_account_folder_or_message`

Main (1): `managed_reader_markup_preserves_exact_semantic_plain_and_quote_variants`.

Admission tests exercise the production decision consumer with fictional injected loader/probe/reserve-save closures. Rejection asserts zero probe/reservation/save, retained TB roster/ID; admission checks existing direct IDs and ordering. They do not access actual config/keyring/network. Reader tests exercise retention/correlation and exact markup, **not actual focus, SelectAll or clipboard**. Prior four provider/four reader tests are retained; no prior integration tests were removed.

## Checks, gates and hashes

Executed: direct rustfmt and direct rustfmt `--check` on the three owned sources; `git diff --check`; 14 inline Python source-order/callsite assertions and three-file NUL/trailing-whitespace scan. All passed. No Rust execution or compiler acceptance follows.

GPUI test-support is not enabled in the native manifest; no manifest change or pretend wrapper-focus test was made. Parent must verify actual selected TextView focus/SelectAll/Copy across selection, body replacement, plain/quote modes and composer/modal guards in its serial grouped window. Existing IMAP dispatch-clone origin guard, receipts, leases, metadata and calendar refusal remain untouched. Full epochs/origins, Outbox/recovery/shutdown, real OAuth/TLS/token rotation, inline HTML/CID/A11y/helper security, H2 head-of-line and native performance gates remain separate. Declared-but-unwired P1 work is not reported as a new defect or feature completion.

SHA-256:
- main.rs: `0c8e8278c503a03b2dec4e4653147542ad146c55b44d925a954f3e5141386aa9`
- provider_setup.rs: `4ed8e5f423b7b3f62d56bfd3af822b0a157136b72ac8f0e535462e5e744ba072`
- reader_session.rs: `45791fe273e1871a497937b96ac1e3c69fb9f67204ad0f12ef22fab7dbf89614`
- unchanged worker.rs: `edece443444d2120d6096140156fa60fa581568ac26f34ec385e3448a53c42f1`

No further source changes or runs by this owner after release; parent owns source receipt and grouped verification.

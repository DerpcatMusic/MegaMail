# Batch 2: exact installed-generation recovery resync repair

Status: bounded P2 source correction complete; Rust acceptance remains pending in the parent's single grouped verification window.

Baseline: `d4dbd2bba14f474a33870269cc15bd98376ff360`. HEAD is unchanged. This correction edits only `apps/megamail/src/draft_recovery.rs` and adds this report. The recovery module remains untracked with the rest of the parent batch; no changes were committed. No main/live/compose/adapter, shared GTK source, manifest, renderer, activation or provider-auth changes belong to this correction.

## Finding and correction

The frozen review correctly identified a post-rename failure: generation G was installed, directory sync failed, the writer emitted `Failed`, and an unchanged G retry was rejected by the old `G <= stored generation` guard.

`RecoveryStore` now distinguishes its own uncertain installation without granting general equal-generation writes:

1. The existing private temporary-file creation, complete record write, file `sync_all`, precommit fault guard and actual `fs::rename` remain in that order. Pre-rename failures still leave the previous record intact and remove only the temporary file created by this attempt.
2. **After actual rename**, the store records an `UncertainCommit` for that key and generation. It pins the installed file descriptor and records Linux device/inode, length, modification and change timestamps. It retains no additional composition/body/attachment-byte buffer. Pinning prevents inode reuse from making another installation look like this one.
3. The store executes real root-directory `sync_all`. Failure returns the existing `RecoveryError::Io`; the unchanged writer publishes the existing `WriterEvent::Failed`. The marker survives failure. Success removes the marker and returns `Durable`.
4. An equal-generation retry is eligible only while this same store owns a marker for that exact key/generation and the reopened installed file and pinned descriptor still match its installation identity. A fresh store has no such permission. Older generations remain rejected before attachment capture.
5. The retry captures and validates the complete current outgoing snapshot, including source address, sensitivity policy, all hidden composition fields, attachment basenames/order and freshly read bytes. Its complete serialized record must exactly equal the installed JSON bytes. A differing snapshot returns `StaleGeneration` without writing or resyncing. Invalid/missing/oversized attachment input remains an error, never a durable acknowledgement.
6. An eligible retry performs **directory resync only**, not a temporary write or rename. It then reopens and validates the final name, rechecks the store-owned installation identity, and compares the exact serialized snapshot again. Only then does it remove the marker and publish the existing `Saved { outcome: Durable }` acknowledgement.

This is not “equal generations may overwrite.” Equal generations with no marker, a different composition or attachment payload, or another writer's replacement inode remain rejected. Even a byte-identical replacement installed by another writer cannot borrow this store's marker. A newer successful generation retires prior uncertainty; a subsequent equal retry is again stale. Explicit successful unlink drops its marker before the existing deletion directory sync.

Uncertainty is tracked per key, with at most 256 pinned descriptors, matching the existing writer key bound. An attempt to add another uncertain key beyond that bound is refused before replacement. This bounds failure-state resources when the store is used directly.

The record format and public save/restore/discard/writer APIs are unchanged. Existing `open_with_commit_guard` remains compatible. No new native event or parent adapter API is needed. The test-only directory-sync guard is private, per store, and absent from non-test builds. With its fault removed, the same function always executes the real production directory sync.

## Authored regressions — not executed

Four new Linux Rust tests use the production store. The principal test also submits through the production `RecoveryWriter`, its real bounded queue, debounce/flush and event path:

- `writer_resyncs_exact_unchanged_generation_after_actual_rename_sync_failure`: injects EIO at the real post-rename directory-sync seam. It observes `Failed` and reads the actual final record to prove G was installed. Another unchanged retry while the fault remains produces another failure. After recovery, different Bcc and same-path/same-length changed attachment bytes are rejected without another sync. Unchanged G then receives `Saved/Durable`. The installed inode and bytes remain unchanged. Restore after deleting the original source file recaptures and compares **every serialized record field** and attachment name/bytes, including complete draft origin/calendar metadata. Older and already-durable equal generations remain rejected. The writer is drained to disconnection before fixture teardown.
- `uncertain_resync_rejects_every_changed_record_field_without_consuming_permission`: changes each persisted composition/source/policy field individually, including draft-origin folder/path/UID, calendar content/method, security flags, schedule, attachment removal and same-bytes/different-basename replacement. Each is refused. The original snapshot still resyncs once; a second equal-generation save is stale.
- `uncertain_permission_is_store_owned_and_rejects_another_writers_identical_byte_replacement`: a reopened store rejects unchanged G. A synthetic competing atomic replacement installs exactly identical bytes under a different inode; the original uncertain store still rejects G and does not sync or acknowledge it.
- `newer_durable_generation_retires_uncertain_resync_permission`: a newer durable snapshot clears the marker and preserves stale/equal-generation rejection.

The prior production precommit-ENOSPC rollback test and retention/attachment-limit/unsafe-path/cleanup/send-correlation tests remain intact. They are also unexecuted by this correction. The fault injection is deterministic seam injection, not evidence from an actual failing disk or power-loss experiment.

## Executed static checks

- Direct `rustfmt --edition 2024 --config skip_children=true` on the owned Rust file, followed by its direct formatting check.
- Scoped `git diff --check`. Because the Rust file is untracked, an additional Python whitespace assertion inspected its full contents directly.
- Ten Python source-contract assertions: actual rename precedes marker and directory sync; file sync/old precommit guard precede rename; real sync remains after the test-only guard; session-only returns before storage/attachment reads; equal generation requires store-owned identity; exact serialized readback is checked twice; the final name is reopened after sync; the resync branch does not write; the authored tests cross the real store/writer/restore seams; and owned-source whitespace is clean.

These checks establish formatting and source ordering only. They do not typecheck, execute Rust tests, establish crash survival, or certify native integration. No Cargo, rustc, build/check/clippy, helper compilation, private profile/keyring access, provider login/mail operation, installer execution, commit or push ran. No background service was started.

## Parent handoff and unresolved gates

The native owner can keep the current event mapping and attachment-lease integration. Retry the unchanged current generation through the **same live writer/store** after `Failed`; do not reopen the writer/store to acquire equal-generation permission. Keep the snapshot's source paths or lease alive through capture. Busy/Closed is still not a save. Apply `Saved` only to the matching account/key/generation, and keep the dirty/open state on failure. There is no new UI-thread I/O: reads, capture, resync and validation stay inside the existing background store/writer lane. Session-only decrypted content still returns before disk or attachment access; sensitive plaintext recovery still requires explicit opt-in and renewed restore consent. No reader/decrypted-body cache was added.

The parent must compile and run the four new `draft_recovery::tests` cases and inherited recovery tests in its grouped window, then exercise native retry/dirty-state acknowledgement and lease lifetime. This correction does not accept the original native parity, account durability, renderer security/fidelity, latency or teardown objectives.

The existing single-instance-lock/one-writer-per-root requirement remains mandatory. The inode/time checks detect replacement and ordinary in-place changes; they are not a lock or protection against a hostile same-user process/root modifying files or racing validation. Competing writers are unsupported; the regression establishes refusal of a completed competing atomic replacement, not multiwriter linearizability. Exact-generation deletion/older-copy repair, queued/unknown-send reconciliation, shutdown/error UI and broader scope obligations from `batch2-draft-recovery.md` remain separate parent/deferred work. After process/store restart, the in-memory uncertainty marker is deliberately absent; restore and advance generation through the existing explicit flow instead of silently acknowledging an equal generation.

Review response: the recovery P2 has a bounded source repair and authored production-seam regressions. The separate HTTP-connect budget P2 is untouched and remains with its assigned owner. Prior Standards/Spec findings and other frozen security/integration obligations are not resolved or broadened by this patch.

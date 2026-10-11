# Batch 2: F7 local draft recovery handoff

Date: 2026-10-10. This implementation owns `compose.rs`, the new `draft_recovery.rs`, this report, and `apps/megamail/src/fixtures/batch2_draft_recovery/` only.

The store and compose operations exist in source. Native autosave and restore UI are not ready. The parent must add the module declaration and connect the operations below. No Cargo command, Rust compiler, native helper compilation, commit, provider operation, private profile read, or credential access ran for this task.

## Evidence and scope

The work follows F7/D09/D10/C19/C20 in `email-client-feature-matrix.md`. The foundation mail-engine report requires retained composition after failed or uncertain sends. The account-lifecycle report requires stable account identity without reimport. The provider-auth report separates app identity from Thunderbird identity. The reader and fetch-latency reports require background work and continued remote-resource blocking.

`graft ask` ran first and returned no matching nodes. Source inspection followed. Rust and documentation skills supplied the implementation rules. Context7 returned current Rust source for `OpenOptions`, Unix flags, descriptor permissions, and `create_new`. Existing dependencies supply serde, serde_json, and base64. No manifest changes are required.

`OutgoingMessage` has no composer/request identifier. The send request and its completion carry identifiers outside that model. The model remains unchanged. Current native `SendResult` exposes account, request, outcome, and uncertainty. Current provider draft completion exposes `autosave: Option<u32>` and an optional server message ID. That contract still needs account/request/generation correlation in main/live.

## Actual changes

The new store writes one versioned file per account and opaque composer key. Each record preserves the outgoing fields, source address, generation, and sensitivity policy. It includes To, Cc, Bcc, Reply-To, From alias, plain/HTML alternatives, thread headers, draft origin, Outbox origin, sign/encrypt flags, scheduled time, calendar part, and owned attachment bytes. Provider credentials never enter this record.

Attachments use base64 inside the record. A restored composition receives private copies with the same basenames, including duplicate basenames in separate numbered directories. `AttachmentLease` retains their lifetime through shared ownership. Drop removes only the lease's staging files. The record retains its independent bytes. The parent must retain a lease until the send/save worker finishes reading those paths. The outgoing model stores attachment paths, not original MIME Content-Type or CID placement. Recovery preserves those files and bytes, not full-fidelity MIME reconstruction.

`RecoveryWriter` handles capture, encoding, serialization, writing, and debounce on one background thread. Admission uses a four-command queue. Results use a sixteen-event queue. A pending update coalesces newer generations and emits `Superseded`, not a false save result. The idle debounce has a five-second maximum dirty interval. The writer bounds remembered composer keys to 256 admitted update keys. `Flush` forces the latest pending capture. A rejected submission returns the owned command through `TrySendError`.

`compose::prepare_recovery_update` prepares the real outgoing model without filesystem work. It rejects account/key mismatches and ambiguous or unconfigured From aliases. `compose::prepare_local_recovery` restores the same outgoing model that the real send path consumes. It refuses a changed source address instead of silently sending from the current account address. `prepare_draft_from_raw_in` exposes explicit staging-directory injection for production preparation and synthetic tests. Existing MIME restoration still simplifies HTML and converts inline images to attachments, with a notice.

`compose::prepare_forward` returns a warning when the text-only forward omits original attachments. It does not download those attachments or claim attachment-preserving forwarding. The original `forward` API remains compatible. The parent must switch its forward preparation call to the new helper to show this warning.

## Storage and privacy rules

A write first reads and validates the existing record. Unsupported, corrupt, oversized, unsafe, or newer records block replacement. The store then captures all attachments. It creates a new 0600 temporary file in the private root, writes the version header and JSON, syncs the file, renames it, and syncs the root directory. Errors before replacement leave the previous final file intact. Temporary-file cleanup never unlinks a preexisting path when exclusive creation fails.

A directory-sync failure after rename is a save failure, not durable success. The replacement contains validated bytes, but crash durability remains uncertain. The parent must keep the composition open and offer retry. There is no second backup or automatic repair of a corrupt final record.

Reads have an 80 MiB encoded-file limit and a version header. Strict serde decoding rejects unknown fields and malformed records. Validation enforces the account/key relationship, positive generation, source address, and attachment names. Text has a 4 MiB aggregate limit. Attachments have limits of twenty files, 25 MiB per file, and 50 MiB total. An attachment failure blocks the whole update instead of silently dropping a file.

Linux directory traversal opens each path component with `O_NOFOLLOW`. Store operations use retained directory descriptors through `/proc/self/fd`. Private roots require current-user ownership and no group/other permissions. Private record reads also reject hard links. Attachment-source reads reject symlinks, traversal, nonregular files, and hard links. `O_NONBLOCK` prevents a substituted FIFO from blocking the reader. These restrictions reject otherwise legitimate symlinked picker paths. The parent must explain that failure, not bypass it.

Linux is the only supported recovery platform. Other targets fail closed. Linux also requires a usable `/proc/self/fd`. The module adds no unsafe code or native dependencies. The caller must hold the app's single-instance lock and provide one writer per root. This module does not implement an interprocess lock. It does not defend against a hostile process with the same user permissions or root access.

The caller supplies an absolute path in MegaMail's persistent private profile. Do not use Hylki paths, provider profile directories, or a cache directory that routine reader-cache cleanup deletes. The module never discovers profile paths or reads environment/keyring configuration.

Retention describes the source content, not outgoing encryption. `DecryptedSessionOnly` performs no attachment reads or disk writes. It returns `SessionOnly`, not `Durable`. `DecryptedPlaintextOptIn` requires explicit consent to local plaintext retention for that composition. Such a record requires renewed consent before restore. Ordinary recovery files are also private plaintext files, not encrypted storage. Filesystem deletion is not secure erasure.

If source sensitivity is unknown, use session-only policy. Keep that classification through reply, forward, restore, and edits. Do not infer safety from `message.encrypt`, a sanitized body, or an encrypted-mail cache exclusion. Never reuse an ordinary draft key for unrelated decrypted content. A policy change does not silently delete an older good record. Offer explicit discard of that record when the user revokes retention consent.

`prune_stale_work` removes bounded generated temporary files and dead staging directories at startup under the single-instance lock. It never deletes final records or follows links. Unexpected directory content remains for explicit repair. Do not run it while a composer or send worker owns a staging lease. Existing `compose::prune_stale_draft_attachment_dirs` does not know this store and must not target its root.

## Required main integration

Add the declaration in `apps/megamail/src/main.rs` during the parent's integration window:

```rust
mod draft_recovery;
```

Initialize storage on an existing background lane after the app acquires its single-instance lock. Keep the root separate from existing MIME staging and reader-cache directories. The snippets describe the required calls, not a finished GPUI patch:

```rust
// Background lane. root is an explicitly supplied MegaMail profile path.
let store = draft_recovery::RecoveryStore::open(&root)?;
store.prune_stale_work(64)?;
let (writer, results) = draft_recovery::RecoveryWriter::start(
    store,
    std::time::Duration::from_millis(500),
)?;
```

Each new composition gets `DraftKey::new(account_id)`. Each mutation increases its generation with checked arithmetic. After restore, begin from the returned generation and increase it before the next write. Never reuse keys for a different composition or silently wrap the counter.

Connect all mutable composer fields and attachment changes to preparation. Account changes require an explicit identity decision and a new account-scoped key. Do not autosave only the visible To/Subject/Body fields. Preserve Cc/Bcc/Reply-To, threading, alias, and origins that currently live outside those inputs.

```rust
// UI lane: no file reads here. Clone only the real outgoing model.
let update = compose::prepare_recovery_update(
    &outgoing,
    &key,
    generation,
    &account_configuration,
    retention,
    restored_attachment_lease.clone(),
)?;
let admission = writer.submit(draft_recovery::WriterCommand::Update(update));
// Busy/Closed: retain the returned command and dirty UI state. Do not say saved.
```

Drain `results` through the existing background callback mechanism. Match account/key/generation before changing composer state. `Saved { outcome: Durable }` acknowledges private storage only. `SessionOnly` must display that this composition is not crash recoverable. `Failed` keeps the dirty composition and previous record. Retry requires the current outgoing model and live source attachment paths. `Superseded` is not a durable save.

Keep original MIME staging and picker paths alive through background capture and while the active outgoing model still references them. A durable snapshot does not authorize unlinking paths that send/save still uses. Either retain those originals through composer/request completion, or replace the outgoing paths with restored owned copies and retain their lease. Do not remove paths when the UI merely accepts a queue command. A restored update also carries an `AttachmentLease` clone so the background thread can safely recapture its descriptor-backed paths. Existing direct workers can read those paths in the same process. The Thunderbird adapter must copy/read attachments in the native process while the lease remains live, not interpret `/proc/self/fd` paths inside a different process.

For restore, list keys for the owning account on a background lane. `list` scans at most 256 root entries and returns a completeness flag. An incomplete scan is not proof that no other drafts exist. Enumeration pagination beyond that bound remains a UI/repair obligation. Do not list drafts across unrelated accounts. Restore only an explicitly selected key:

```rust
// Background lane. Keep the returned object/lease alive in composer state.
let recovered = compose::prepare_local_recovery(
    &restore_store,
    &selected_key,
    &account_configuration,
    user_consented_to_sensitive_restore,
)?;
```

The parent must arbitrate store ownership for restore versus writing. Restore at startup before starting the writer, or open a separate store handle for background reads and staging only. Atomic replacement supports those reads. Do not call save, discard, or pruning through a competing handle. Do not open competing writers. Populate all composer inputs and hidden outgoing metadata from `recovered.message`. For nonempty HTML, preserve coherent plain/HTML editing or report unsupported editing. Do not show raw recovered HTML in a network-capable renderer. The store never fetches remote resources.

If the From address is no longer configured, the helper retains recovery and returns an error. The parent needs a repair choice that reads the retained composition, shows the old address, and lets the user explicitly select another identity. No such repair UI is implemented here.

For close/shutdown, submit `Flush` and wait for its matching result on a background lane. Keep the application alive until that result arrives or the user chooses an explicit loss policy. Dropping the writer requests a best-effort flush, but process exit is not an acknowledgment. There is no implemented close prompt or shutdown wait in main yet.

For forward preparation, call `compose::prepare_forward`. Show its notice before sending. Fetch and stage original files only after the existing explicit attachment action. This batch does not implement automatic original-file forwarding.

## Send and provider draft contracts

Retain the request ID, account ID, key, generation, and attachment lease when main dispatches send. Map an actual terminal send result to this command:

```rust
writer.submit(draft_recovery::WriterCommand::SendFinished {
    key: sent_key,
    generation: sent_generation,
    expected_request: pending_request_id,
    account_id: result.account_id,
    request_id: result.request_id,
    outcome: result.outcome,
    uncertain: result.uncertain,
});
```

The writer clears only a definitive `Sent` result with the expected account and request. Newer admitted edits prevent an old result from clearing recovery. Queued, uncertain, failed, and unrelated results do nothing. A mismatched stored generation also prevents deletion, including a send after a failed newer save. Provider draft-save success never clears local recovery. The direct `clear_after_send` method requires the caller to enforce current-composer generation in addition to its supplied correlation fields.

Recovery is not permission to resend. A retained draft can also exist in durable Outbox after a queued send. The parent/F5 owner must reconcile send intent and queued/uncertain status before enabling send on a restored composition. This store does not persist a new-send delivery ticket or identify a newly created Outbox row. Keep the recovery chooser explicit and never automatically send recovered content. Exactly-once resend protection is not established by this batch.

Explicit discard uses `WriterCommand::Discard { key, generation }`. It cancels pending updates at or below that generation and rejects late updates that attempt to resurrect them. Deletion still requires the exact stored generation. If the current generation never saved, report the retained older copy and offer explicit discard of its generation. Never hide a deletion failure or treat it as a successful discard. After terminal discard/send, a different composition gets a new key.

The parent/live owner needs a server-draft ticket with these fields: account ID, native request/autosave ID, local draft key, and local generation. Existing `MailRequest::SaveDraft.autosave` can carry `Some(id)` without changing `OutgoingMessage`. Use checked allocation and avoid ID reuse while requests remain pending. Both direct and Thunderbird adapters must return that ID with the bound account. Failures need the same correlation, not free-form status text.

A server Message-ID is not the native request ID. A successful HTTP/bridge operation is not automatically a saved draft. Preserve remote origin/replacement rules and prevent overlapping saves from duplicating server drafts. These main/live/adapter changes are integration obligations, not changes in this batch. Do not close the composer or delete its staged attachment files because current-account status text says "Draft saved".

## Validation and honest assessment

Executed: `rustfmt --edition 2024` on the two owned Rust files, the synthetic Python probe, and scoped `git diff --check`. The probe passes MIME fixture header/byte assertions and Linux atomic-rename, fsync, retained-directory-descriptor, and no-follow primitive assertions. It does not execute Rust. Formatting parses Rust syntax but does not typecheck the module.

Authored, not executed: thirteen Rust tests in `draft_recovery.rs` plus two additional compose tests. They use temporary fictional data through `RecoveryStore` and the real compose preparation interface. Coverage includes restart, deleted source files, Bcc/threading/alias/calendar preservation, binary bytes, duplicate filenames, MIME preparation, precommit ENOSPC injection, pending/staging cleanup, unsupported/corrupt/oversized records, stale generations, account/request send correlation, sensitivity consent, unsafe paths, writer coalescing/discard, and changed From identity. Existing compose tests remain in place. The parent must run them after module wiring in its one batch window.

Good: this is a narrow store and compose handoff, not another mail engine. It owns attachment bytes independently of ephemeral MIME staging. Save success follows file and directory sync. Errors preserve the prior record instead of silently dropping attachments. Neither server draft status nor queued/uncertain send status deletes recovery.

Bad: the feature is not reachable from native composer mutation or recovery controls yet. There is no completed shutdown prompt, From repair UI, provider reconciliation, or rich compose editor. Base64 and whole-record JSON increase disk use and transient memory. Updates recapture all attachment bytes. No latency or memory improvement is measured. The Linux-only no-symlink policy restricts legitimate file choices.

Risk: the parent must correctly preserve sensitivity and attachment lifetime across send/save handoffs. Descriptor-backed attachment paths require same-process copying before Thunderbird IPC. Directory sync and storage failures need visible errors. Same-user attackers, secure erasure, multiwriter locking, and comprehensive recovery-list pagination remain outside this slice. The test fault represents ENOSPC before replacement, not an actual full-filesystem crash experiment.

Blocker: module wiring and all main/live integration remain with the parent. Rust compilation and tests remain unexecuted by instruction. Do not claim shipped autosave, provider draft correlation, native crash survival, or performance gains until those integration gates run.

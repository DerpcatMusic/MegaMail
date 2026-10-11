# Batch 2 encrypted Outbox preview correction

Status: source correction frozen and released for parent review. Three new Rust regressions remain uncompiled and unexecuted. The parent owns the grouped verification window after integration freezes.

Ownership: this correction changes only `src/worker.rs` and this new report. The original delivery report remains unchanged. Cache, models, configuration, manifests, native integration, helper and bridge files remain outside this ownership.

## Defect and correction

`queue_built_outbox` previously copied `OutgoingMessage.body` into every durable Outbox preview. OpenPGP wraps the formatted MIME message, but the composition still holds its plaintext body. Immediate direct IMAP sends now persist their intent before SMTP, so even accepted encrypted sends persisted this plaintext preview. Deleting the row does not erase SQLite pages or its write-ahead log (WAL).

The correction branches on the borrowed composition's `encrypt` flag before reading its body. When `encrypt` is true, the persisted preview is an empty string. When `encrypt` is false, the existing whitespace collapse and 160-character Unicode limit remain unchanged. The existing Outbox UI already displays waiting, schedule or error status when the body preview is empty.

Envelope and subject metadata remain intentionally separate. The correction does not hide those fields or claim encryption at rest for the database. It does not scrub previously stored previews or promise removal from old database files, WAL files or backups.

## Released contract and production paths

All public types, request/event variants, cache methods and GTK caller signatures remain unchanged. This correction adds no public API. The existing private production signature remains:

```rust
fn queue_built_outbox(
    cache: &Cache,
    account_id: u32,
    msg: &OutgoingMessage,
    sent_path: Option<&str>,
    error: &str,
    send_at: Option<i64>,
    email: &LettreMessage,
) -> Option<u32>
```

The new privacy contract is `msg.encrypt == true => persisted preview == ""`. The helper still persists `email.formatted()` and its exact envelope. It still passes the same account, subject, recipients, Sent path, error, schedule and scoped replacement ID to `Cache::queue_outbox_replacing`.

Source inspection covered these production paths:

- Immediate direct IMAP: `send_imap_intent` builds once and calls `queue_built_outbox` before the durable transmission claim.
- Offline IMAP, scheduled sends and failed-send queueing: `queue_outbox_message` builds once and calls the same helper. POP3 and Graph/JMAP scheduling or failure paths also reach this shared queue helper.
- Flush and retry: `flush_outbox_with_sender` consumes persisted raw MIME, envelope and delivery state. It does not recreate a body preview from a composition.
- Received-message previews: `preview_of` and retry extraction consume received MIME parts without OpenPGP decryption. `finish_preview`/`pgp_preview` recognize the version stub or armored ciphertext. Graph/JMAP use provider preview fields, not locally decrypted composition bodies. POP3 raw summaries initialize an empty preview. These paths remain unchanged.

`build_email`, `build_message`, `pgp_wrap` and both queue callers borrow the same `OutgoingMessage`. No caller changes `encrypt` between MIME construction and preview persistence. `restore_msgid_case` changes only reply/reference IDs. Existing SaveDraft branches explicitly disable signing/encryption for draft storage, but they do not call this queue helper. The flag controls this preview policy, not a heuristic based on body text, subject, UI state or successful decryption.

Atomic replacement, Bcc envelope-only handling, durable claims, held-state exclusions and typed unknown-send outcomes remain unchanged. A source comparison against the released worker snapshot confirms that only the queue helper and the new regression block changed. Every previous delivery/body test remains byte-for-byte intact outside those regions.

## Authored regressions and evidence limits

The parent can select `worker::tests::encrypted_outbox_preview_`. Three new tests call the actual production queue helper:

1. `encrypted_outbox_preview_persists_no_body_and_retains_exact_accepted_mime`: uses an already-built synthetic PGP/MIME message and an encrypted composition with distinctive fictional plaintext/HTML. It asserts exact raw MIME, envelope, alias sender, To/Cc/Bcc, subject and Sent-path roundtrips. It inspects every text/blob field in the actual Outbox and delivery tables through a second connection. It also searches only the fixture's new SQLite files, including WAL, for the fictional secret marker. An injected acceptance calls the production durable transmission seam. The accepted row remains safe after cache reopen and remains excluded from ready rows.
2. `encrypted_outbox_preview_plaintext_behavior_is_unchanged`: asserts whitespace collapse and the exact 160-character Unicode preview through persisted rows. It covers the flag policy for unsigned and sign-only compositions. The prebuilt message for the sign-only policy case is deliberately unsigned to avoid GPG.
3. `encrypted_outbox_preview_replacement_and_rollback_keep_only_safe_rows`: injects a DELETE-trigger failure after insertion inside the real atomic replacement transaction. It asserts preservation of the old safe row and ready state. It then asserts wrong-account rejection, successful encrypted replacement, exact MIME/envelope preservation, schedule retention, sibling preservation and in-flight replacement refusal.

These fixtures prove queue privacy only if the Rust tests pass. Their ciphertext and MIME shape are synthetic. They do not prove real encryption, signing, SMTP, provider behavior or recipient delivery. No test invokes actual GPG keys or a real transport. The file-backed tests use explicit new temporary database paths, never profile discovery or the normal cache path.

Executed evidence consists of full-worker syntax parsing through standalone `rustfmt --emit stdout` with child modules skipped, targeted formatting, `git diff --check` and 12 Python source-contract assertions. These assertions confirm branch placement, shared production callers, unchanged surrounding source, fixture coverage and preserved test names. They are not Rust execution or a substitute for compilation. Context7/network documentation lookup was not run because this task prohibits network operations. Local installed dependency source confirmed the envelope comparison API.

## Residuals and parent gate

The parent must review this correction before the single grouped compilation/test window. Existing GTK/native compatibility and the inherited delivery/body tests remain part of that window. No compilation or runtime test passed in this owner window.

A separate pre-existing source observation remains outside this correction: `build_message` returns calendar MIME before its OpenPGP branch. Therefore `encrypt=true` alone does not establish encrypted calendar MIME. This preview guard still suppresses that composition's preview, but it cannot protect plaintext raw MIME produced by that earlier branch. Do not claim encrypted calendar sending from this work. No crypto/MIME builder redesign belongs to this narrow correction.

The native integrator separately strips generationless direct-IMAP `draft_origin` in the dispatch copy. This owner does not change draft retirement. Unversioned header events, UID mutations, legacy writers and generationless draft origins remain documented residuals from the original delivery report. This correction introduces no new epoch protocol and does not claim those edges are safe.

No Cargo, compiler, build, check, clippy, helper/native compilation, private profile/keyring/key access, provider/network/mail/installer operation, commit or push ran here. Graft was queried first and returned no matching nodes. AGENTS, Rust guidance, SECURITY, DESIGN, architecture, performance-security, delivery/cache reports and implementation-batches informed this correction. No background process needs cleanup.

Released `src/worker.rs` SHA-256:

```text
7a05b8364d29cc47299193a3562c4c23a9dd565c17a1f5be375d64eaab7f540a
```

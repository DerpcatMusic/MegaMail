# Batch 2: private Linux compose activation and packaging

## Status: source implementation, not shipped default-mail support

**Activation transport and URI-preserving packaging are implemented in the owned
files. Native startup/composer integration and compiled/runtime acceptance are
still parent obligations. Do not install or advertise the new mailto association
as working until those gates pass.** The former pure parser alone did not provide
cold startup or running-instance activation. This module provides the real
profile-lock/socket seam for those paths; it is not yet a caller in `main.rs` at
this owner's freeze.

No Cargo/build/check/clippy/rustc, native/helper compilation, installer execution,
application launch, desktop-database/default-handler update, account/profile or
credential access, provider/Thunderbird login, mail send/mutation, or commit/push
was performed. Rust tests below are authored, **not compiled or executed**.
Python packaging tests run only extracted production generation code in synthetic
temporary directories. There is no claimed activation latency or product speedup.

Read AGENTS, PRODUCT, DESIGN, architecture, the [feature matrix](email-client-feature-matrix.md)
(U09, U20, U21 and the F0/U1 ownership contracts), [desktop/privacy handoff](batch2-desktop-privacy.md),
and the foundation [engine](foundation-mail-engine.md), [account lifecycle](foundation-account-lifecycle.md),
[provider/auth](foundation-provider-auth.md), [fetch latency](foundation-fetch-latency.md),
and [HTML reader](foundation-html-reader.md) reports. Graft was queried first and
returned no matching nodes; source inspection followed. Applied Rust best practices
(ownership, errors, tests, documentation/FFI safety), rust-skills, and deep-module
seam guidance. No UI design or new mail-engine rewrite is included.

## Actual owned changes

- [`apps/megamail/src/instance.rs`](../../apps/megamail/src/instance.rs): additive
  Linux activation module and real Unix socket protocol tests. The existing
  `acquire(&Path) -> io::Result<File>` signature/body remains byte-for-byte unchanged;
  the original lock regression test remains. Both old and new callers use the same
  lock inode. No second worker/session path exists in this module.
- [`data/megamail.desktop`](../../data/megamail.desktop): `Exec=megamail %u` and
  `MimeType=x-scheme-handler/mailto;`. No D-Bus activation or GPUI URL-scheme stub.
- [`tools/install-megamail.sh`](../../tools/install-megamail.sh): the existing
  desktop-generation stage appends exactly one separate `%u`, verifies the template's
  sole MIME declaration, and correctly applies both Desktop Entry string escaping
  and Exec argument quoting. Literal executable-path `%` becomes `%%`. Existing
  install/build/license behavior otherwise stays in place and was not executed.
- New [`tools/test-megamail-activation-packaging.py`](../../tools/test-megamail-activation-packaging.py):
  six passing test cases exercising the extracted actual production quoting function
  and generation heredoc, never a fake full install.
- This new report. No changes to main/live/desktop/actions/core/manifests by this owner.

Parent acknowledged adding direct native `libc = "0.2"` and the dependency edge to
already-locked libc 0.2.190. Those are **parent changes**, not this owner's work.
Parent reports TOML/lock static validation only. The actual `--locked` gate and
license-inventory lock hash refresh remain parent-owned before installation.

## Stable public interface for parent integration

These exports exist under `#[cfg(target_os = "linux")]`:

```rust
instance::start_activation(
    profile_dir: &std::path::Path,
    initial_uri: Option<&str>,
) -> Result<instance::ActivationStartup, instance::ActivationError>

// One variant permits mail workers. The other must exit before workers.
instance::ActivationStartup::Primary(instance::ActivationInbox)
instance::ActivationStartup::Forwarded

// No filesystem/socket/URI parsing. A false readiness predicate does not pop.
inbox.take_if_ready(ready: bool) -> Option<megamail_core::desktop::MailtoIntent>
```

The interface was sent to parent early, before source freeze. Required integration:

1. Preserve `--thunderbird-host` dispatch **before** normal-app argument handling.
   Do not pass native-host socket paths/flags as mailto input. Keep demo mode
   separate from profile/activation/mail workers. This module deliberately does
   not parse CLI flags or implement native-host dispatch.
2. Obtain the app-selected private `config::config_base()` directory. Preserve
   the original profile namespace; never accept an arbitrary activation socket
   path, profile override, or account hint from a URI.
3. On Linux normal startup, replace the existing single `acquire` call with
   `start_activation(config_dir, optional_exact_utf8_uri)`. Do **not** acquire first
   and then call this function: that would correctly look like a second instance.
   Reject non-UTF-8 CLI input without lossy conversion. Admit one URI argument;
   reject unsupported extra/unknown flags instead of silently reinterpreting them.
4. Call startup before entering GPUI Application, or on a background executor.
   It performs private filesystem/lock/connect I/O and can wait up to the configured
   handoff deadline. `Forwarded` exits immediately, before creating any workers,
   TB runtimes, or a second UI. Every error also exits without creating workers.
   A no-URI second launch keeps the old already-running rejection behavior.
5. Retain `Primary(inbox)` for at least as long as all workers and outbox retries.
   Retain it through first-run setup and account restoration. Do not reacquire,
   clone the lock, or release it just after reading the initial link. Shutdown
   joins/cancels the I/O thread and removes only its verified socket inode before
   the lock is released. Prefer dropping after the app/event loop and worker
   shutdown, not from a GPUI input/render callback.
6. Poll `take_if_ready` from a native event/update task, not rendering. `ready`
   must be false during setup/no usable sender, account-choice dialogs, any
   existing composer/modal, pending send/save/attachment work, uncertain send
   state, or an otherwise nonreplaceable draft. Choose/confirm a **usable explicit
   account** first; never derive sender identity from URI To/Cc/Bcc fields or a
   stale active/unified selection. Re-evaluate this predicate on the UI thread.
7. Once eligible, consume one validated `MailtoIntent` and pass it directly to
   `desktop_actions::compose_message(intent, explicit_account_id)`. This is the
   existing production `OutgoingMessage` initializer. Do **not** call
   `external_compose`/`parse_mailto` again on already-decoded fields; do not encode
   and reconstruct a new URI. Open the ordinary composer with those fields. No
   automatic Send, Save, attachment loading, resource fetching, or account setup.
8. Expose safe rejection/busy/uncertain notices. URI/body/recipient data must not
   enter logs. `HandoffUncertain` means the request may already be queued; do not
   automatically retry it. User inspection is needed before another attempt.

The queue is not a draft store. Acceptance means admission to an **in-memory
compose queue**, not a visible composer, durable draft, mail submission or delivery.
Crash/restart can lose not-yet-opened intents; existing draft recovery takes over
only once integrated into normal composition. No auto-retry/exactly-once claim.

## Implemented transport/security contract

- Linux-only, filesystem AF_UNIX stream socket. No external TCP, session service,
  executable spawn, daemon, native URL-scheme registration or credential access.
- Fixed `activation/compose.sock` beneath the configured profile. Directory
  traversal uses `openat(O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC)` one component at a
  time. Reject relative paths, symlink components and foreign/writable ancestor
  directories; permit root-owned sticky temporary parents for synthetic fixtures.
  Profile and activation directory must be current-UID owned and have no
  group/world access. Already-bad permissions are rejected, not silently repaired.
- Directory FDs pin the endpoint and lock location through `/proc/self/fd`; no
  process-wide umask change, unsafe global cwd switch or input-provided pathname.
  Opening the lock inode uses `O_NOFOLLOW`, regular-file/private-owner/single-link
  checks, and then calls the existing `acquire` through its pinned FD alias. The
  successful lock File is held in `ActivationInbox`. All secondary paths retain
  the same exclusion and cannot instantiate workers through this module.
- Lock-owner writes only an 8-byte version/PID record. Secondary reads this bounded
  private record, and checks server **kernel SO_PEERCRED UID and PID** before sending
  any URI. Receiver checks kernel credentials/effective UID before reading input.
  Kernel credential retrieval failure, foreign UID or server PID mismatch rejects.
  This is real libc enforcement, not a fake peer-auth callback or a transitive-import
  assumption. Same-UID malicious code can modify the user's profile/lock record;
  this is **not** a sandbox against a compromised same-UID process or root.
- Socket is privately owned, non-symlink, single-link and mode 0600 inside a private
  directory. Receiver revalidates pinned profile/directory linkage, socket inode
  and private permissions before processing and before queue admission. Changed
  permissions, a replaced socket or moved endpoint fail closed.
- Only a successful profile-lock owner reaches stale cleanup. Cleanup requires a
  verified private owned socket and a nonblocking connect returning ECONNREFUSED,
  then rechecks inode/type/ownership before unlink. Any connectable, busy, foreign,
  corrupt, timed-out or uninspectable endpoint is left untouched and startup fails.
  Drop only removes this receiver's socket inode in its still-valid private location;
  replacement files/sockets are not deleted. Failed cleanup can leave a harmless
  stale endpoint that the next successful lock owner must validate.
- Versioned frame: `MMA1`, big-endian u32 length, exact raw UTF-8 URI bytes, then
  sender write-half-close. Empty/over-64-KiB, truncated, wrong-version, non-UTF-8,
  trailing-data and multiple-frame input reject before admission. Parse exactly
  once using **core `desktop::parse_mailto`**, after authenticated/bounded framing.
  Its existing ASCII recipient subset, unsupported-header/attachment rejection,
  CRLF injection checks, percent-decoding limits and plain-text body rules apply.
  Literal `+` and double-encoded percent data are not decoded a second time.
- One named receiver thread; nonblocking sockets and poll slices up to 25 ms.
  One shared monotonic 750 ms read/framing/response deadline per accepted connection,
  not reset per byte; 2 s total secondary readiness/transmission/ACK budget. Cancellation
  checked per poll slice. These are configured bounds, not measured latency results
  or hard scheduling/filesystem guarantees. No unbounded per-client thread fanout.
- Maximum **8 pending validated intents**, including cold initial input. One bounded
  in-flight frame is additional. FIFO retains pending input behind setup/busy UI;
  overflow explicitly returns Busy without replacing existing drafts/intents.
  ACK contains only accepted/busy/invalid status. No mail body or URI logging.
- Endpoint-readiness retries happen only before transmitting a request. Transport
  failure after transmission starts or an unknown ACK returns HandoffUncertain;
  no automatic resend. An old primary that only uses `acquire` has no working
  activation endpoint; secondary fails rather than creating a second mail session.

TLS/OAuth checks, account isolation, durable outbox and encrypted-body/remote-resource
policies are not weakened or reimplemented here. The module has no mail-worker,
provider, cache, credential, reader or send operation.

## Executed checks and unexecuted tests

Executed without a build or real-user profile:

- `rustfmt --edition 2024 --check apps/megamail/src/instance.rs` — passed.
- `bash -n tools/install-megamail.sh` — passed; installer was not run.
- `python3 tools/test-megamail-activation-packaging.py` — **6 passed**. Exercises
  actual extracted production generator; spaces/Unicode, quotes, backslashes,
  dollar/backtick/shell punctuation, literal `%`, placeholder-looking paths,
  exact unchanged URI argument, unsupported/control/relative paths, malformed
  MIME/Exec templates, and sensitivity to the former single-escape-layer bug.
  The reference decoder follows the desktop specification; it is not a test of
  an actual compositor's launcher or system default settings.
- Scoped `git diff --check` — passed. Read-only original/current source comparison
  confirms the old `acquire` signature/body is unchanged.

`instance.rs` contains **14 new Linux `#[test]` functions**, including a subprocess
fixture entry, plus the untouched original lock test. They are authored but
**uncompiled/unexecuted**. Tests use the actual production `start_activation` seam,
receiver, frame transport, private temporary profiles and Unix sockets:

- cold/running exact-once decoding, Bcc, FIFO and retained old profile lock;
- 8-pending saturation/recovery and false readiness retention;
- wrong version, oversize, truncated/extra frames, invalid UTF-8, injection,
  unsupported scheme/attachment fields and invalid percent encoding;
- separate-profile isolation and ordinary second-launch rejection;
- bad permissions, lock/socket symlinks, corrupt non-sockets, active endpoint
  preservation, lock-owner-only stale recovery, replaced endpoint cleanup;
- real kernel peer UID/PID, wrong server PID rejection before URI transmission;
- silent/no-half-close clients, shared deadlines and shutdown cancellation;
- distinct-process primary/client through the same exported startup seam, with
  a test-only lifecycle handshake to hold the synthetic primary until client ACK.

Foreign-UID rejection is implemented but no privilege-changing/cross-UID runtime
fixture was executed. Corrupt endpoint behavior and subprocess timing still need
compiled testing. Rustfmt is a syntax/format check, not type/link/test evidence.

Parent's one batch build/test window must compile/run these production tests and
then add actual native app acceptance for cold link, already-running link,
setup→account-ready queueing, busy/dirty/uncertain composer retention, explicit
account choice, overflow rejection, unsupported/non-UTF-8 CLI input,
`--thunderbird-host` separation, duplicate-worker prevention, shutdown/crash/stale
endpoint recovery, and a real supported desktop launcher's URI argument handling.
Use fictional/synthetic profiles, no provider login/send. Refresh license inventory
before any later real installation; do not update user defaults in this task.

## Honest assessment

**Good:** real shared-lock transport, real peer credentials, production core parser
at admission, bounded queue/framing/deadlines and cleanup, no native-host shortcut,
no fake send action, and tests crossing the same module seam. Packaging tests prove
correct two-layer quoting of actual generation code rather than simulating a live
installation. Existing GTK-compatible `acquire` is untouched.

**Bad / incomplete:** main/composer do not acquire/drain this interface until parent
integration. MIME/Exec metadata alone must not be published as usable default-mail
support. Rust tests were not run. The queue is volatile and default-app/launcher
behavior remains unverified. No attachment entry, full RFC recipient support,
notifications, print, accessibility certification or AI feature was added.

**Risk:** unsafe libc FFI and Linux `/proc` reliance require the parent batch's
real type/runtime checks; mode-enforcement filesystems and no-symlink config paths
are required. Config homes on permissive/non-POSIX mounts or symlinked paths will
fail closed. Same-UID compromise is not a security boundary. Slow/silent clients
can occupy the single receiver until their fixed deadline; legitimate activation
may be Busy/Unavailable/Uncertain under load. Drop must happen after workers and
outside UI input/render paths. Queue admission is not a durable-send guarantee.

**Blocker:** native integration, parent-owned locked build/test gate and cold/running
native/compositor acceptance. Direct libc dependency was supplied by parent; no
remaining request for this owner to edit manifests or compile separately.

## Documentation evidence

- [Freedesktop Desktop Entry Exec variables/quoting](https://specifications.freedesktop.org/desktop-entry-spec/latest/exec-variables.html)
  retrieved successfully: quote Exec arguments, apply general string escaping first
  when reading, double literal `%`, keep URI replacement a separate field code,
  and do not re-expand replacement content. Executable paths containing `=` are
  unsupported by that specification and rejected explicitly.
- [Linux unix(7)](https://man7.org/linux/man-pages/man7/unix.7.html): public web GET
  timed out; installed `/usr/share/man/man7/unix.7.gz` supplied the SO_PEERCRED
  contract (credentials at connect/listen/socketpair; connected AF_UNIX streams).
- Direct libc 0.2.190 cached source supplied actual getsockopt/openat/constants/type
  signatures. Context7 resolved `/rust-lang/libc`, but returned no matching
  SO_PEERCRED documentation; that lookup was not treated as successful API evidence.

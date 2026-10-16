# Batch 2 native QA: frozen-source synthetic harness and acceptance plan

## Native-reader repair: current acceptance evidence

This section supersedes the historical source-only handoff below for the native
reader repair at source `42cc125`. It is not a build or installation receipt.
The initial PR #10 qualification remains historical; a new release receipt must
record the final optimized build, portable-core results and installation hashes.

The corrected light and dark evidence lives in
`.impeccable/review/native-reader/{dark,light}`: 28 fictional screenshots and
`observations.json` files. A fresh full independent finish review inspected all
28 captures and returned **SHIP**, with no material fixes. Native tests pass (319). CI also passes 616 portable-core tests and 25 theme tests at source `42cc125`. Optimized release installation is recorded separately in [the repair receipt](native-reader-qualification.md).

The extended isolated harness now performs guarded coordinate actions on its
owned demo window and reads its private X11 clipboard. Actual copied body text,
sender address and newsletter text matched fictional expected content. It also
exercised independent quote disclosure, message disclosure/details, a local-photo
fixture, divider dragging, expanded/compact layout retention and window resizing.
An explicit original-formatting action opened a separate helper window; closing
it retained native text. These results are narrower than complete accessibility,
provider behavior or physical-device latency qualification. Screenshot existence
and delivered input alone remain observations rather than feature assertions.

Current source behavior and acceptance boundaries:

- Stable selectable GPUI semantic text is the default. Native states are retained;
  no helper is started automatically, and no late PNG replaces selectable text.
- Authored reply content and recognized quoted history use separate text states;
  nested attribution remains restorable. Explicitly hidden inline preheaders are
  omitted from the semantic view. Quote recognition is not universal HTML parity.
- Sender/address text sits outside the disclosure button. Details show actual
  authentication summary/findings or an explicit unavailable state.
- Explicit local PNG/JPEG/WebP photos and configured own-account avatars use
  bounded background thumbnail decoding with at most two concurrent loads and
  initials fallback. No automatic Gravatar, remote image fetch or contacts sync.
- Pane dividers enforce minimum widths and retain separate expanded/compact
  layouts in session and through window resizing, not across restart.
- Original formatting is an explicit separate isolated window, with remote
  resources blocked. It requires complete eligible source; unattested direct
  MIME fails honestly rather than claiming full original rendering. Synthetic
  helper-window evidence does not establish every provider's MIME/CID coverage.
- Clipboard observations under software Xvfb demonstrate those actual interactions;
  they do not establish hardware FPS, input latency, AT-SPI or screen-reader support.

## Historical batch-2 source-only handoff

The following sections retain the earlier harness preparation, source review and
limitations as historical evidence. Their statements about unexecuted runtime,
missing clipboard/coordinate automation and a closed original-mode release gate
are descriptions of that earlier handoff, not the repaired source above. Do not
reuse those historical limitations as current implementation claims or treat the
current narrow acceptance as proof that every older backlog item is complete.

## Status / ownership

Prepared, not executed native QA. This owner created only:

- `tools/megamail-native-batch2-qa.py`
- `docs/research/batch2-native-qa-plan.md`

No source/manifest/helper/installer edits, Cargo/compiler/build, native/helper
launch, provider/TB/keyring/profile operation, send, installation, default-handler
change, commit, or parallel integration owner. Parent runs actual QA **after all
owners freeze and one grouped build**. Existing documentation's earlier test counts
are historical, not batch 2 acceptance. New registered modules alone are not features.

Graft queried before native source inspection: `graft ask 'native demo startup isolation LiveMail inventory
Thunderbird provider helper mailto activation QA' --source` returned no indexed
nodes. No graph rebuild. Read AGENTS, PRODUCT/DESIGN, architecture/validation,
implementation-batches, synthetic latency/reader/link probes, and reader,
activation, provider, recovery, desktop, compose, contacts/calendar handoffs.
Diagnosis skill applied to explicit test seams, not speculative bug fixes: runtime
reproduction is forbidden now, so no red/green native result. Impeccable context
loaded; Linux GPUI is not mobile/web. Its unrecognized PRODUCT platform warning is
reported only, not repaired. No UI score or accessibility certification from source.

## Early integration requirements / blockers sent to parent

1. **Mailto cannot be exercised through current demo.** `feature_integration::startup`
   accepts one `--demo` or one mailto, not both. `main` sets activation to `None`
   for demo. An explicitly usable fictional composer sender exists through
   `LiveMailbox::composer_account_id`, but there is no demo IPC receiver. Do not
   use normal mode merely to make this test appear green. If native activation
   acceptance is required, the integration owner must supply a separately reviewed
   synthetic-only flag/fixture that skips real inventory and enables explicit
   fictional sender choice. This QA owner does not add it. Module-level activation
   tests are a different surface.
2. **Causal invalidation requires event fixtures, not pixels.** Existing production
   Rust tests such as `queued_old_body_then_reset_cannot_consume_reused_uid_or_its_metadata`
   in `live.rs` are appropriate. Current demo has no reviewed external causal event
   injection/receipt interface. Same-UID reuse, session/body generation changes,
   and invitation correlation remain source-only until parent executes those tests.
3. **Original mode is gated and demo is not an original MIME/CID fixture.** Native
   demo bodies supply selectable semantic formatting; current selected-original
   transport must be checked at freeze. Do not reconstruct original source from
   sanitized `reader_html` or advertise complete styled images from demo. Parent
   must supply complete selected MIME HTML + bounded CID map + causal scope for
   native original-mode acceptance. Missing/unknown/truncated original must fail
   back to native selectable content.
4. **Reachability differs from authored modules.** At review time contacts/snippets
   had modules, but no reviewed reachable native demo CRUD/completion/snippet
   controls. Invitation UI needs actual correlated SenderCheck/MIME data. If controls
   remain absent, mark unavailable; do not fabricate account data or activate imports.

The serial integration owner is active. These are observations of the inspected
source, not final claims about the frozen batch. Re-review after its release.

## Source evidence that demo skips real inventory

Review these symbols in final source, not stale line numbers:

- `feature_integration::startup`: exact `--demo` sets `demo=true`, `uri=None`.
- `main`: demo bypasses `instance::start_activation`; `--thunderbird-host` is a
  separate first-argument branch, never passed by this harness.
- `MailApp::new`: demo constructs `LiveMailbox::new_demo`; no SetupState; only
  `if !demo` calls `watch_setup_email`, `load_saved_accounts`, `start_native_features`.
  Appearance/theme/MailPreferences still load, but only from the owned empty XDG
  directories in this harness. Event pump starts; no activation/recovery service
  exists in demo. Review any new unconditional startup call at freeze.
- `LiveMailbox::new_demo`: calls `new_with_demo_mode(Vec::new(), ..., true)`.
  QueryService is skipped, real-profile loop is empty, and only fictional workers
  are spawned for IDs 1/2.
- `spawn_demo_worker`: `worker::spawn(account_id, None, ...)`.
- `worker::spawn` / `cache_lane`: protocol is None; `protocol.and_then` does not
  open a Cache. `run` routes None to `run_mock`, not IMAP/SMTP/Graph/JMAP. Read
  `run_mock` to confirm it uses MockBackend, not credentials/provider inventory.
- Native `send_composer`, `save_composer_draft`, live send/mutation/import/Outbox
  paths contain demo guards. Never press those controls as a way to test guards.

This establishes the inspected first-party startup routing, not arbitrary-library
security or binary/source provenance. Execute requires **manual final-source review**
and a matching source receipt; the digest is not an automatic Rust control-flow
proof. If any final startup path cannot be proven synthetic, report blocked and do
not supply the review receipt. The binary must be the parent's build of that source.

## Harness contract and isolation

Python standard library only. No command runs on default/`--dry-run` paths, even
when `--execute` is also supplied. Dependency lookup uses filesystem PATH search;
source digest reads first-party Rust/manifests and helper C/header/Makefile files.
It does not import the existing native helper tests or execute a substitute parser.

Execution needs an explicit absolute canonical parent-built `--binary`, trusted
ownership/permissions on the executable **and every ancestor**, and matching
`--reviewed-source-sha256`. Group/other-writable or symlinked executable paths are
rejected, including ordinary `/tmp` staging. Stage binaries in a private trusted
build directory if the mounted checkout's permissions cannot meet this gate.
No installation or default handler is involved. A receipt hashes source and records
binary SHA-256; neither proves the parent compiled that binary from those sources.

Mandatory tools on Linux: Python 3.11+ (`hashlib.file_digest`), `/usr/bin/bwrap`
with unprivileged user/PID/net/IPC namespaces, `Xvfb`, `xdotool`, ImageMagick 7
`magick import` with X11 support, software Mesa/Vulkan support required by the
parent-built GPUI executable, and matching runtime libraries. `/usr/bin/Xvfb`,
`xdotool`, `magick`, `bwrap`, `python3` were found by filesystem lookup here;
**no version or runtime probe was run**. `xwd` was not found and is not needed.
Runtime namespace/X11/image support remains untested. Never add a host-display or
sandbox-disabled fallback to compensate for missing dependencies.

The parent launcher:

- Creates a private temporary root with owned HOME/config/data/cache/state/run
  and artifacts subdirectories. Sets umask/private directories; exports only
  fictional artifacts, logs and receipt, never the temporary profile.
- Uses `bwrap --unshare-all --clearenv --die-with-parent --new-session` with new
  network, PID, IPC and user namespaces. Does not mount host root, home, profiles,
  credentials, /mnt, host /tmp, runtime user bus, Wayland socket, GPU devices,
  Xauthority, resolv.conf or session/system DBus sockets. Read-only system runtime
  under /usr and needed lib/font paths is visible. System-installed executable
  code remains trusted; this is not defense against a malicious root runtime.
- Binds only the selected executable, this harness and its owned temporary root.
  Environment is reconstructed, not inherited; no OAuth/client IDs, tokens,
  Thunderbird paths, HYLKI fixture knobs, LD_PRELOAD or desktop session variables.
- Checks that the network namespace differs from the parent's before launching
  native. Starts its own `Xvfb -displayfd 1 -ac -nolisten tcp` **inside** the
  namespace, using private /tmp sockets. `-ac` grants only this isolated display;
  no personal auth/display or host firewall changes. X11 remains usable by Unix
  socket while external network is isolated. No namespace availability probe runs
  now; bwrap failure is blocked, never a weakened retry.
- Launches only `/app/megamail --demo`, discovers exactly one visible window with
  the app PID and demo title, and rechecks owning PID before each capture/action.
  Uses `magick import -window WINDOW`, never root/desktop capture. Records window
  ID, owning PID, viewport geometry, display, UTC and monotonic elapsed time.
- Sends only guarded focus/list Up/Down and a final Ctrl+N request; resizes to
  1060×640 and 1520×900. No pixel-coordinate clicks, URLs, sign-in/connect/import,
  Refresh, attachment picker, save/send/discard, or external browser action.
  No xdg-open shim: native `cx.open_url` may use portals and a shim would not prove
  interception. The harness simply never invokes external link paths.
- Bounds session time to 20–120 seconds (default 60), command wait to at most
  5 seconds, Xvfb allocation to 8 seconds, observation pauses to 0.2–3 seconds.
  Parent waits session budget + 15 seconds; TERM/reap grace 2 seconds, then KILL
  and reap 2 seconds. Starts owned process groups only. Normal exits, exceptions,
  timeout, SIGINT/HUP/TERM all trigger cleanup. Bubblewrap PID namespace teardown
  is the descendant backstop. No broad pkill, stale-PID signaling, host-display
  socket deletion or profile crawler. SIGKILL/power loss cannot run Python cleanup;
  die-with-parent/PID namespace teardown protects descendants, but an owned temp
  directory can remain after that abnormal exit. Remove only its exact known path.

Optional `--output-dir` must be a NEW absolute canonical directory with an existing
parent. It intentionally persists exported fictional evidence; all working HOME/XDG
and temporary directories are removed. Without it evidence disappears except JSON
stdout. Parent should choose a private evidence location and inspect cleanup; no
script-controlled old output deletion. The namespace may reject nested helper
sandboxes or software rendering; report blocked/fail and retain logs, not assumptions.

### Optional developer-only helper probe

`--helper` alone merely validates/records an absolute trusted executable. Native
`MEGAMAIL_HTML_READER_DEVELOPER_PREVIEW`/`MEGAMAIL_HTML_READER_HELPER` are never set;
the normal **Original formatting — temporary window** release gate stays closed.

Only explicit `--developer-helper-preview --helper /absolute/trusted/helper`
also mounts that compiled binary and its sibling
`extensions/libmegamail-reader-policy.so`, each read-only, under an isolated
package path. After native observations it launches the standalone helper with
one small in-script fictional v1 HTML/table/media-query request, empty resources,
no source paths and no external links. Its own nested containment stays enabled.
It waits at most 15 seconds within the whole-session deadline and records a ready
event as **not_run: observation only**, never feature/security pass. No ready event
is blocked. No helper screenshot is taken because nested PID namespace window
attribution is not established by this harness. This is not native inline integration,
complete CSS/CID/image parity, accessibility, selectable-copy proof or zero-network
engine acceptance. It intentionally does not override the release gate.

Parent's separate `tools/megamail-html-reader/{tests.py,engine_probe.py}` and
README own compiled policy, adversarial request/CSP mutation, syscall/packet trace,
missing-extension, active-content, raster budget and descendant cleanup acceptance.
Network isolation prevents egress; it does not prove requests were never attempted.
An engine-ready token or screenshot cannot replace that security work.

## Exact commands

All commands below run from `/mnt/Windows11/DEV_PROJECTS/Repos/MegaMail`.

Safe **now**, no binaries/native processes:

```sh
python3 tools/megamail-native-batch2-qa.py
python3 tools/megamail-native-batch2-qa.py --dry-run
python3 tools/megamail-native-batch2-qa.py --dry-run --binary /absolute/trusted/parent-built/megamail
```

After whole-batch source freeze and parent's grouped build only:

1. Re-read final startup symbols above and every changed unconditional startup
   path. If isolation is not proven, stop. Confirm binary/build provenance.
2. Run dry-run again and copy `source_sha256` from that exact frozen checkout.
   The script fails closed on any intervening first-party source change.
3. Parent-only bounded native observation:

```sh
python3 tools/megamail-native-batch2-qa.py \
  --execute --binary /absolute/trusted/parent-built/megamail \
  --reviewed-source-sha256 FINAL_MANUALLY_REVIEWED_SOURCE_SHA256 \
  --timeout 60 --settle 1 \
  --output-dir /absolute/existing/private-parent/NEW-native-batch2-evidence
```

Optional synthetic helper observation (same freeze/build gate; trusted compiled
extension must be next to the helper; use a different NEW evidence directory):

```sh
python3 tools/megamail-native-batch2-qa.py \
  --execute --binary /absolute/trusted/parent-built/megamail \
  --reviewed-source-sha256 FINAL_MANUALLY_REVIEWED_SOURCE_SHA256 \
  --helper /absolute/trusted/helper-package/megamail-html-reader \
  --developer-helper-preview --timeout 90 \
  --output-dir /absolute/existing/private-parent/NEW-native-batch2-helper-evidence
```

No Cargo/build/install command is supplied for this owner. Parent controls its
single grouped compile/test window and the helper security suite order.

## Automated assertions versus observations

JSON result statuses are `blocked`, `not_run`, `pass`, `fail`.

- **Dry-run/default:** all results are not_run/blocked. Zero process launch, no pass.
- **Only native automated pass:** unique visible window owning PID and explicit
  Local demo title. Narrow fact only; not first readable body or feature readiness.
- **Captures, focus/navigation, resize and compose request:** not_run, with evidence
  for manual evaluation. Delivered keys/screenshot existence do not verify UI effects.
- **Unavailable fixtures/controls:** blocked. Code/module/text markers do not pass.
- **Isolation/tool failures:** blocked before native launch; native crash or capture/
  ownership/command failure during requested session: fail. Inspect logs to classify
  software renderer/dependency limitations versus a product defect. Exit 0 means the
  observation session completed, **not that feature QA passed**; exit 1 is a failed
  session/probe; exit 2 is a precondition block. There is no aggregate feature pass.

There is no native semantic tree/assertion API in this tool. Parent must fill in
actual manual outcomes and refer to compiled production-seam regression output.
Do not derive textual correctness, hidden metadata, focus behavior or performance
from screenshot byte counts/OCR guesses.

## Bounded manual acceptance checklist after freeze/build

Inspect one batched set of desktop captures; repair by another owner if needed,
then at most one confirmation round. No open-ended visual polish loop. Interactive
steps below require an owned synthetic session; this script does not expose a
personal display or pause indefinitely. Parent may drive a separate equivalently
isolated bounded session, without loosening the restrictions above.

| Surface | Real action / red-capable expected evidence | Available evidence / honest limitation |
| --- | --- | --- |
| Selected body | Select known fictional original/reply, switch rapidly to next/previous; native selectable fallback stays tied to selected sender/subject/body. No old account body flashes as current. Test loading/failure states with an actual fixture. | Harness captures delivered list navigation; manually compare expected fictional content. No paint/body-ready telemetry. Original completeness/failure fixtures are still required. |
| Reader wrapping | At actual 1060×640/1280×800/1520×900 geometry, check subject, long recipients/dates, RTL/mixed Unicode, actions and local data-table scroll; no metadata clipped behind actions/footer. | Existing `demo_reader_thread_messages` exercises LTR/RTL wrapping and threading. Source min width 1060 disagrees with DESIGN's 1000 range; do not claim 1000/mobile support. Two monitor scales/IME are separate physical/compositor tests. |
| Links/quotes | Expand/collapse a **panel only**, not a destination; hidden links do not create excess empty height; quote restore retains exact selected text/formatting. Toggle selectable plain view; keyboard copy into a fixture-only isolated clipboard. | No label/click assertion in harness. Recheck actual panel/toggle affordance at freeze. Do not click hyperlink, attachment/save, location URL or helper link. Clipboard/A11y not proven by focus key. |
| Generation invalidation | Deliver old direct BodyAtEpoch after new epoch/UID reuse, then body metadata under old source generation; accept/drop body and SenderCheck together. Cancel helper on selection/account/epoch change, reject late Ready/click. | Parent executes actual `live.rs`/`reader_session.rs` regressions. External demo causal fixture unavailable; no UI automation claim from screenshots. |
| Composer | Open with explicit fictional sender; enter To/Cc/Bcc/subject/body via actual reachable inputs; leave send/save untouched. Focus body and press arrows/Ctrl+R/Ctrl+N: caret changes or guarded no-op, no hidden mailbox/composer replacement. | Harness requests Ctrl+N only. Fields may be visible rather than collapsible: check actual UI, not an imagined Bcc dropdown. Manual visible Bcc is not hidden model preservation. |
| Hidden outgoing fields | Start the real prepared-compose fixture with Bcc, Reply-To, alias, references, HTML/calendar, binary attachments, schedule/security flags; make supported edit/snippet; assert exact untouched fields through real compose/recovery seams. | `compose.rs`, `compose_productivity.rs`, `draft_recovery.rs` tests supply model evidence after execution. Plain demo cannot inject all hidden fields; no fake MIME/send. Rich HTML edit must explicitly preserve or refuse, not flatten silently. |
| Contacts/snippets | Only if reachable: fictional scoped CRUD/completion ≤20 results; sender switch clears old account snapshot; names with comma/quotes insert safely into correct To/Cc/Bcc. Snippet missing variables/HTML refusal leaves model unchanged and keeps edit recoverable. | At initial review controls unavailable. Prepared store/render APIs are not UI features. No OS/TB/DAV import/connect. Existing real-store/model tests are separate acceptance. |
| Invitation | Select actual synthetic MIME/calendar with causal source token. Verify timezone/all-day/recurrence/cancellation/warnings; old metadata after generation switch is absent. RSVP stays unavailable. | Current demo SenderChecked data is not production causal direct metadata. Use actual invitation/MIME regression fixtures. Never infer a meeting from HTML dates or click Accept/Decline/URL. |
| Mailto IPC | If owner supplies reviewed synthetic activation fixture: explicit sender, cold/secondary prefill To/Cc/Bcc/subject/body, literal +/%252B, no double decode; busy/dirty composer holds FIFO, no replacement/send; unsupported inputs reject. | Blocked in current demo. Execute `instance.rs`/desktop parser production tests separately. No normal native launch, installed desktop launcher or default-handler change in this harness. |
| Recovery | Actual private-store fixtures: restart/flush acknowledgment, Bcc/attachments/hidden fields, ENOSPC/precommit vs post-rename fsync uncertainty, stale send request/newer edit retention, decrypted session-only excludes persistence. | Demo `start_native_features` skipped; UI durability cannot pass here. Parent's existing fixture `apps/megamail/src/fixtures/batch2_draft_recovery/private-draft.eml` and Rust tests are fictional. Never discard/send a real draft. |
| Account/provider routes | Actual checked add/reload and separate direct/TB identities; Google IMAP/SMTP pending probe vs Microsoft Graph pending Inbox GET vs custom TLS config; saved is not connected. | Synthetic production transaction/registry tests only in this task. No real login/keyring/probe/TB inventory. Owned registrations/approval and separately authorized account restart/refresh tests remain release gates. |
| Security/accessibility/performance | Helper zero-request/active-content tracing and current-scope teardown; native real AT-SPI tree/keyboard order, text scaling; selected-body seam timings from real correlated telemetry, same cold/warm workload. | Separate parent acceptance. Xvfb software elapsed time is not physical FPS, responsive rendering or accessibility. No accelerated/latency improvement claim from toolkit or readiness. |

## Good / bad / blocked at this source-only handoff

**Good:** isolated no-launch default, explicit freeze receipt/binary, no inherited
profile/credential/display/bus, owned network+Xvfb, window-only fictional evidence,
bounded process/timeout cleanup, honest observation statuses, no brittle UI labels
except mandatory demo title, temporary original-formatting gate preserved.

**Bad/incomplete:** no native runtime result, no first-body/first-paint assertion,
no coordinate/label automation, no semantic tree or clipboard validation, no hidden
field test via UI, no external stale-generation fixture, no helper screenshot with
verified nested owning PID, no binary provenance proof, no physical GPU/FPS or A11y.

**Blocked:** demo mailto IPC; production original-MIME/CID and causal invitation
fixtures; contacts/snippet reachability at inspected source; provider registrations
and explicitly authorized real-provider/TB/keyring acceptance; independent engine
security acceptance and inline GPUI frame/input/accessibility integration. The
standalone formatting helper is temporary, never an inline completion claim.

## Source freeze / static-only evidence

This owner's two new files are released to parent after AST/dry-run checks. Native
integration remains exclusively with its active owner. Exact current source receipt
is intentionally not pinned in this report while that owner changes source: obtain
it after final freeze. No fake native success is implied by static preparation.

Executed checks (all static/Python-only):

- Python `ast.parse` and in-memory `compile` of the new harness: clean; no bytecode.
- Eleven mocked no-launch cases: default, dry-run, dry-run plus execute, explicit
  harmless `/usr/bin/true` path validation only, dry-run with a proposed output,
  missing binary/review receipt, wrong receipt, relative executable refusal,
  helper-preview without helper refusal, internal-entry refusal and internal-entry
  dry-run refusal. `Popen`, `subprocess.run`, `killpg` and temporary-root creation
  were mocked to raise if called. No pass status was emitted.
- Static sandbox argv assertions: network/PID/etc isolation, empty environment,
  capability drop, helper/extension read-only mapping, no host display/bus path or
  native helper release-gate environment variable. Not namespace execution.
- Mocked reaped-process cleanup check: no stale-PID signaling.
- Two owned-file trailing-whitespace/final-newline checks: clean.
- `python3 tools/megamail-native-batch2-qa.py --dry-run`: exit 0, all feature
  results not_run/blocked; filesystem dependency paths found. No native/helper run.
- Scoped `git diff --check -- tools/megamail-native-batch2-qa.py
  docs/research/batch2-native-qa-plan.md`: clean (untracked-file whitespace covered
  by the separate Python audit).

All native/helper actions above remain **unexecuted** in this task.

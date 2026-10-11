# Batch 2 HTML reader — bounded implementation candidate

## SERIAL native integration contract — supplied early

Only new owned files are being changed. Native main/live/TB owner is exclusive;
parent performs original-source injection **after releases, before its one batch
build window**. Do not interpret these unregistered modules as a shipped feature.
No Cargo, compiler, helper build or engine implementation test has run.

`apps/megamail/src/reader_document.rs` provides:

- `ReaderScope { account_id: Arc<str>, folder_id: Arc<str>, message_id: Arc<str>, generation: u64 }`.
  Message identity includes UIDVALIDITY for IMAP or TB session epoch/key.
- `Completeness::{Complete,Truncated,TooLarge,Missing}`.
- `CacheEligibility::{Allowed,Signed,Decrypted,Unknown}`. Only complete + explicitly
  Allowed qualifies for any later persistent cache. Unknown is excluded.
- `RasterType::{Png,Jpeg,WebP}` and
  `ReaderResource::new(cid: &str, media_type: RasterType, bytes: Arc<[u8]>)`.
  No SVG, arbitrary paths, attachment-name lookup or remote byte provider.
- `ReaderDocument::new(scope, original_html: Option<Arc<str>>, semantic_html: Arc<str>,
  plain_text: Arc<str>, title: Arc<str>, completeness, cache_eligibility,
  resources: Vec<ReaderResource>) -> Result<ReaderDocument, DocumentError>`.
  Construct off-thread. Original must be complete selected decoded HTML, not
  `reader_html` output; semantic must remain existing `mail_text::reader_html` output.
  Original is immutable/untrusted, kept separate; never pass it to TextView.
  Original/semantic/plain each cap at 2 MiB, title 512 bytes, resources 32,
  compressed raster 2 MiB each / 8 MiB total. Oversize is rejected, not truncated.
- Accessors `scope`, `original_source`, `semantic_html`, `plain_text`, `completeness`,
  `cache_eligibility`, `persistent_cache_allowed`, `retained_bytes` and
  `helper_request -> Result<Vec<u8>, DocumentError>`.
  Incomplete original cannot enter the helper. Retained accounting is not RSS.

IPC v1: one EOF-delimited UTF-8 JSON request on stdin, <=20 MiB, <=10 seconds:

```json
{"version":1,"html":"<html>complete selected original</html>","plainText":"selectable fallback","title":"Subject","resources":[{"cid":"logo@fixture.invalid","mime":"image/png","base64":"canonical RFC4648 bytes"}]}
```

No account IDs, profile paths, secrets or generation appear in the helper payload.
One child process handles one immutable scoped generation. Never reuse it across
messages/accounts. Parent must cancel on scope change/account removal. Responses
are bounded newline JSON on stdout (`version:1`, `event`, `token`, optional field).
The native host authorizes a token only from successful `ready` after policy
handshake/document load; error/close tokens cannot authorize link handoff.
Link events must match that ready token **and** the locally retained scope.
Helper does not open a browser or provide eval/shell/network commands.
Native host and exact build/security commands are documented below and in
[`tools/megamail-html-reader/README.md`](../../tools/megamail-html-reader/README.md).

## Current policy direction

Parser reconstruction preserves inert tables/styles/media queries; the engine
receives a distinct rebuilt document, not unchecked original. JS/forms/fonts/
frames/popups/downloads/navigation are denied. A web-process `send-request`
extension is installed before bootstrap; the helper must receive its policy
acknowledgement before any untrusted document load. Missing extension fails closed.
All remote/file/data/unexpected requests remain denied, including redirects.
Raster CID img sources become exact per-process capabilities after independent
bounded decode/normalization. Remote images require an app-owned consent broker
in a later slice; there is no permissive remote toggle here.

CSS is retained as inert layout input under CSP + exact request denial + external
network namespace. This slice does not claim a complete CSS token sanitizer or
universal original fidelity. CSS CID URLs/data images remain blocked rather than
being rewritten by regex. Standalone native selection is real toolkit behavior;
inline GPUI frame/input/clipboard/A11y remains a separate timeboxed proof.

## Actual owned source delivered

- NEW `apps/megamail/src/reader_document.rs`: immutable source/scope/completeness/
  cache-policy/resource seam, separate native fallback, bounded v1 encoder, five
  dormant Rust unit tests through its actual constructors/encoder.
- NEW `apps/megamail/src/reader_host.rs`: `ReaderHost::start(absolute_executable:
  PathBuf, document: Arc<ReaderDocument>) -> io::Result<ReaderHost>`, `try_recv`,
  `cancel`, nonblocking Drop; four dormant Rust tests through its real protocol
  decoder/scope verification/bounded line reader. Encoding, process spawn and
  pipe I/O run on named workers. No shell, browser launch or arbitrary RPC.
- NEW `tools/megamail-html-reader/{main.c,policy.c,policy.h,request_policy.c,
  extension.c,sandbox.c,sandbox.h,policy_tests.c,Makefile,README.md,
  fixture_request.py,tests.py,engine_probe.py}`: independently packaged C helper,
  pre-load extension, parser/resource policy, containment and synthetic harnesses.
- NEW `docs/fixtures/html-reader/batch2/{authored.html,hostile.html,
  parser-confusion.html}`: fictional CSS/table/media/CID and hostile parser/resource
  fixtures. No mail/profile/credential data or real destination was inspected.
- NEW this report. No existing main/live/TB/bridge/worker/cache/manifests/corelib/
  shared mail_text/vendor file was edited. Other owners' concurrent changes remain.

### Native host state/protocol details

`ReaderHostEvent::{Ready {scope}, UserClickedLink {scope,url},
Fallback {scope,reason}, Closed {scope}}` carries local scope, not helper-supplied
account identity. `HostFailure` differentiates unavailable/document/pipe/protocol/
policy/timeout/crash/backpressure. Native host requires exact v1 ready policy,
32-hex ready token and matching token on every click/heartbeat; safe link is checked
again with existing core `mail_text::safe_browser_url`. Helper click requires actual
WebKit `LINK_CLICKED` plus `is_user_gesture`, ignores browser navigation, and never
opens a destination. Native owner **still** verifies the active account/folder/
message/generation and the native external-link opening policy at delivery.

Stdout lines cap at 8192 bytes; wire queue 8, native event queue 16, click candidates
256/session. Ready deadline 12 seconds; subsequent healthy heartbeat deadline 6
seconds; one viewer lifetime 30 minutes. Cancellation is checked at <=25ms worker
waits after encoding. Encode allocation is input-bounded but not mid-encode
interruptible. Kill + reap grace is bounded to 2 seconds; no `Child::wait` on GPUI.
A disconnected queue without a terminal event is always fallback, not permission
to render unchecked source. Process-tree teardown relies on bubblewrap's PID
namespace/reaper + `--die-with-parent`; parent must prove no orphan descendants.
Kernel-stuck processes need OS recovery; the code does not promise impossible
instant cleanup. Heartbeat uses GLib loop plus WebKit responsiveness flag, not a
first-paint or input-latency metric.

Helper shows actual WebKit widgets, selection/copy behavior, native plain-text
widgets and zoom controls, not a screenshot impersonating a reader. However,
interactive selection/copy has **not** been executed for this implementation.
Backing is opaque white, authored body backgrounds/colors remain; native app
chrome is outside the document. Settings force software acceleration (NEVER),
including Wayland; no JS selection bridge exists.

## Parent SERIAL integration obligations — before the one batch build

1. Add `mod reader_document; mod reader_host;` in native main after its owner
   releases. No manifest change is needed: serde/base64/serde_json already exist.
   This registers the actual module tests; unregistered source is not completion.
2. On both ingestion paths, before `reader_html`, retain `Arc<str>` complete chosen
   original HTML separately from the existing sanitized semantic fragment/plain
   text. Do this on the existing worker path, not rendering/input callbacks.
   Constructor `Complete` requires nonempty bounded original; for too-large source
   retain **no partial document as Complete** and expose TooLarge/Truncated state.
   TB truncated flag and source budget must not be ignored. Do not recreate
   original from current `ReaderBody.source_html`: it already lost CSS/layout.
3. Bind scope to account + folder + epoch-aware message ID + generation. Cancel
   the host on message change, account removal, session epoch change, and window
   close. Check the entire scope again before applying every response or URL.
   Equal UIDs in different accounts cannot share a host/resource capability.
4. Preserve direct signed/decrypted-body exclusions. TB decryption/cache status
   Unknown remains ineligible; do not persist HTML/CID/remote bytes/screenshots.
   The helper has only ephemeral stores, no-store responses, tmpfs writable
   paths, empty home, no profile/bus mounts, and core dumps disabled. Parent's
   existing caches still need their own exclusion tests.
5. CID inputs must be selected MIME-related bytes, not guessed from attachment
   names. Direct worker currently expands some CID to data URI: that is **not**
   compatible with this strict helper's data-resource denial. Parent needs raw
   selected HTML + related CID map, or an audited data-to-scoped-raster adapter.
   TB does not yet supply CID bytes. An empty resource list permits text/styles
   but unresolved images remain absent/alt, not advertised as complete CID support.
6. Package helper and sibling extension together, use absolute trusted installed
   path, expose only an explicitly temporary **Original formatting** action after
   security gates pass. Keep existing native semantic/plain reader visible during
   startup/failure. Never use raw original as a fallback and never replace GTK
   shared callers or the mail worker. This slice does not touch TLS/OAuth/outbox.
7. Apply the owner's event path to host events; do not do process/file/serialization
   work in GPUI rendering. Honor backpressure/disconnection as fallback. External
   link activation does not mean remote image consent. No URL is resolved/fetched
   by this helper or module.

## Build commands and execution evidence

**Nothing was compiled here.** Parent-only exact `make -j1 ... all mutation`, cc
commands, package paths and engine probes are in the helper README. The no-CSP
mutation package emits `test-only:no-csp` and is rejected by ReaderHost; it must
never be installed or used with real mail. Parent's batch window also runs the
registered Rust tests under its existing native build/test plan.

Executed in patch phase:

- Graft queried first; index returned no matching nodes. Switched to source/docs,
  no graft rebuild or ownership-wide rewrite.
- Full `foundation-html-reader.md` read; consulted the feature matrix and other
  foundation reports. Retrieved current WebKit send-request/sandbox/ephemeral/
  custom scheme/settings/header-CSP docs and inspected installed 4.1 headers.
  No repeated engine comparison. Context7 was checked against primary docs.
- Python source-contract audit: **14 checks pass**, **63 installed WebKit API
  names found**. This is a source/header audit, not C typechecking, sanitizer
  execution, runtime extension enforcement or security acceptance.
- Fictional authored v1 request producer/schema assertion passes.
- Synthetic Python/bubblewrap namespace probe with the same isolation flags:
  exit 0, inherited fd 3 present, inner network namespace identity differs from
  original. No engine, display, mailbox or provider operation occurred. This
  proves the namespace/FD primitive on this host, not the full compiled launcher
  or nested WebKit sandbox. An initially drafted nonexistent `--preserve-fds`
  option was caught against installed `bwrap --help`, removed, and the actual
  inherited-FD primitive tested. No fallback to weakened isolation was used.
- `rustfmt --edition 2024` and formatting-only `rustfmt --check` on only the two
  owned modules; `git diff --check` passes. Explicit new-file whitespace checks
  cover all 19 owned files (including untracked files), and Python AST parsing
  passes for the three synthetic/audit scripts. No compilation occurred.
- Initial source-audit literal failed after rustfmt split `.env_clear()` across
  lines; corrected the audit's literal. No runtime failure is concealed by that.

Unexecuted: all C compilation/self-tests/`--policy-test` fixture assertions,
all nine Rust tests, engine readiness probes, zero-network/redirect/DNS mutation
acceptance, nested sandbox/missing-extension behavior, actual pixels/selection/
copy/zoom/resize/crash/cancel/orphan teardown, X11/native Wayland interactive
acceptance, clipboard isolation, screen-reader access, real CID transport,
real-provider/body latency and aggregate descendant RSS/PSS. Existing foundation
render experiments remain **research only**, not implementation acceptance.

## Required post-build acceptance and stop gates

Run the exact real C policy seams with `tests.py --helper ...`: fixtures must
retain media CSS and tables, rewrite verified PNG CID, remove active nodes/attrs,
reject NUL/version/unknown path fields/oversize/duplicate CID/SVG/corrupt base64/
pixel bombs. These tests call the same C parser/resource/request functions used
by the helper; Python is not substituting its own sanitizer. The embedded
`--self-test` exercises exact capability membership, traversal, foreign tokens,
unsafe links and parser/schema guards.

Then run real engine probes + interactive fixture on both backends. For hostile
and parser-confusion fixtures, observe **all descendants** using a synthetic
endpoint plus syscall/packet tracing. Require zero HTTP/DNS/redirect egress;
mutate CSP off in the deliberately separate test build so CSS attempts must still
hit the extension deny gate. An unreachable host-side loopback server inside a
private network namespace is not proof of request callback coverage. Inspect
attempted requests and clean-start baseline. Reject any security-disable fix.
Missing extension/ack, failed nested sandbox or malformed input must show native
fallback with no untrusted engine load. Confirm display grant isolation and no
home/credentials/bus access. Confirm actual user click emits a scoped candidate
without navigation; script/form/refresh/popup/download never do. Verify JS off
independently of CSP using a controlled test-only engine probe, not the production
HTML sanitizer deleting the script first. That probe remains to be implemented.

**Stop:** no real mail / release action until compilation and these security gates
pass. Do not mistake `get_sandbox_enabled`, a ready policy string or source markers
for adversarial acceptance.

## Timeboxed inline GPUI frame/input/accessibility proof — next slice

Allocate **one engineering day** after Gate B security acceptance, not an open-ended
new engine rewrite. Current v1 only opens the labeled temporary native window;
no frame/input/A11y protocol or GPUI renderer adapter is implemented.

1. Separate versioned adapter proof sends viewport logical width/height/scale and
   receives bounded opaque frames keyed to scope/generation. Use one-shot WebKit
   async snapshot into audited bytes/memfd; verify stride, RGBA length, checked
   arithmetic, DPR and stale frames. Do not pretend snapshot is a documented
   continuous windowless rendering/input API.
2. In the actual GPUI rectangle, 680/360 logical widths must produce authored
   side-by-side/stacked panels. Test scale 1/1.5/2, resize, overlay/menu clipping,
   scrolling and white canvas over wallpaper. Do not enlarge the logical viewport
   to desktop scrollWidth, which defeats sender media queries.
3. Forward pointer/drag/wheel/key/focus/IME through real toolkit routing; prove
   selectable text, Ctrl+C on a fixture-only isolated clipboard and verified link
   hit-testing. No unsafe WebKit JS/eval/DOM selection bridge. Test loss of focus,
   resize mid-selection and late input/frame cancellation.
4. Attach accessibility to the **visible GPUI rectangle/focus order**. Current
   sandbox disables the general AT-SPI bus to avoid broad DBus/Secret Service
   exposure. A narrowly filtered accessibility transport is still a blocker even
   for accessibility-ready standalone delivery. GTK widgets alone are not proof.
5. Measure idle scheduling, first readable/paint/cold/warm costs and aggregate
   helper+engine descendants. Event-driven snapshots only, not idle 60Hz copying.
   No speed/RSS improvement claim until representative data exists.

**Stop at day end** if frame/input/A11y linkage fails: retain safe native fallback
and explicitly temporary selectable original-formatting window (still subject to
its standalone A11y acceptance). Escalate an isolated CEF OSR adapter using the
same document/resource semantics per foundation plan; do not use wry drop-in on
GPUI XCB, assume cross-connection Wayland embedding, or ship an unselectable bitmap
as the final reader. That contingency is not implemented in this slice.

## Honest assessment

**Good:** coherent bounded source/document/host/helper chain; exact shared policy
seam; parser+CSP+pre-dispatch request hook+verified network namespace; ephemeral
per-generation process; opaque authored canvas; native text fallback; no JS bridge,
profile or mail-engine mutation. Tests and build commands target real source.

**Bad/incomplete:** not yet compiled or integrated, so no shipped original-reading
feature. This deliberately sacrifices unknown/remote/data/CSS-CID images and
active/nonallowlisted content; source after MIME concatenation or existing CID
expansion is not universal original fidelity. CSS is not fully token-rewritten,
libxml2 is not HTML5, external/system font differences remain, and adversarial
parser/engine behavior still needs runtime evidence. One context/process per
selection favors isolation over warm reuse; no latency/RSS claim is justified.

**Risk:** nested WebKit/bubblewrap packaging, engine custom-scheme CSP/response
behavior, decoder support/budgets, malformed CSS load cost, FD/process teardown,
clipboard/display privileges and original-source ownership/caching must be proven.
The tested namespace primitive alone does not prove the compiled security stack.

**Blockers:** parent SERIAL registration and original-source retention; parent batch
build + real security/pixels/interaction acceptance; selected MIME-related CID
transport; reviewed authenticated-X11 display grant (current local :N/Xvfb path
never mounts .Xauthority); accessibility transport; actual inline GPUI frame/input/
A11y implementation. Explicit consented remote images require a separately owned
app broker with per-account/message/generation consent, SSRF/redirect/byte/decode
limits and no auth/cookies/referrer — **next slice, no renderer remote bypass**.

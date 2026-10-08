# MegaMail architecture and migration

## Decision

Build a Rust desktop email client with the official `gpui-kit` umbrella crate.
Keep Hylki's mail implementation and full history; translate Zeron's visual
system into Kit components. Do not replace Hylki's protocol implementation
with a new mail engine or bring Zeron's coding-agent runtime into this app.

This checkout starts from Hylki commit
`43f2d4665f01a1f0ef4474d719179b8e5b61471c`. The root package is still Hylki.
`apps/megamail` is an independent GPUI preview, with its own manifest and lock
file, so it can build without GTK, libadwaita or WebKitGTK. It currently uses
sample data. The preview is a foundation, not a completed email client.

## Source evidence

The seven reports in [research](research/) cover the actual extraction points,
upstream commit pins, current APIs and risks. They are snapshots, not ongoing
guarantees about upstream releases.

Hylki is a single binary crate. Its shared types include presentation and
GLib calls; its worker and cache have cross-module dependencies. A path
dependency on the root package cannot expose a reusable mail engine today.
The extraction must move code and update both callers rather than shadowing
its behavior with stubs. See [Hylki](research/hylki.md).

Zeron uses its own GPUI fork and theme system. Reuse its proportions, colors,
hierarchy and small control vocabulary. Custom blur/edge-fade APIs cannot be
assumed to exist in the released Kit. See [Zeron](research/zeron.md) and
[GPUI Kit](research/gpui-kit.md).

## Runtime boundary

```text
GPUI Kit application
  retained folder/search/selection/reader state
          │ commands                         ▲ events
          ▼                                  │
Hylki mail worker on dedicated Tokio runtime
  per-account protocol sessions, reconnect/IDLE, OAuth and SMTP
          │                                  │
          └──── SQLite cache / durable outbox ┘
                 separate background readers
```

The native UI owns view state and publishes requests. The mail worker owns
protocol state. Database work and MIME/body parsing do not run inside
`Render`. Cache results arrive as immutable pages; new pages must carry the
query generation so a late result cannot replace a newer folder or search.
Message identity must include account, folder and protocol identity; an IMAP
UID alone is insufficient, and UIDVALIDITY changes invalidate its namespace.

Preserve the current request/event behavior first. Introduce bounded queues
with explicit cancellation/backpressure only after accounting for every
command: archive, send and flags cannot be silently dropped. Coalesce
replaceable progress/refresh events, not mail mutations. A failed enqueue or
worker disconnect must be visible to the caller.

## Migration order

1. **Native preview:** real Kit window, Zeron-derived shell, virtual message
   list, search/selection/archiving over visibly synthetic data. Compile and
   verify this independently. This is the initial deliverable.
2. **Extract the existing core:** move models, config, parsing, cache and
   worker code into an importable library. Separate presentation formatting,
   GTK notification/launch hooks and GLib timers behind the existing runtime
   boundary. Update the GTK caller and run its regression tests before using
   the library from GPUI. Preserve schema, query and protocol behavior.
3. **Read-only real account slice:** create MegaMail-specific paths and secret
   namespace. Add explicit account setup, cached folders/header pagination,
   body loading, cancelled/stale-result handling and honest offline/error
   states. Verify against a local test server before personal accounts.
4. **Safe mail mutations:** connect read/flag/archive/move with operation
   results and error recovery, then composer/drafts and durable outbox. Keep
   SMTP ambiguity visible; never promise exactly-once delivery after a
   connection failure during submission.
5. **Rich messages and parity:** implement the audited HTML renderer decision,
   attachments, PGP, contacts, account/provider variants and packaging. Audit
   Hylki's feature list as an explicit parity checklist, not an implication
   that these features ship in the preview.

## HTML mail decision

The first preview reads plain text. GPUI rich text/Markdown is not a browser
engine and is not a full-fidelity email HTML renderer. Do not pass arbitrary
email HTML through a convenience Markdown renderer and call it equivalent.

Before rich mail ships, choose and test an isolated WebView for each supported
OS or a deliberately limited safe renderer. Reuse Hylki's sanitizer,
remote-resource rewriting and policy where possible, but verify the new
engine's behavior separately. Block scripts, forms, popups, local-file access
and unsolicited remote requests. Remote images require explicit consent.
External navigation crosses a validated application boundary. The composer
is a separate trusted surface with different privileges from incoming mail.

## Account setup decision

Use Thunderbird's public ISPDB/autoconfig XML protocol for domain-to-server
discovery while keeping Hylki's Rust mail engine. Hylki currently offers
provider presets and manual/custom OAuth setup; discovery is new work.
Thunderbird's Gecko/JavaScript/XPCOM account/OAuth modules are not a drop-in
Rust login library. XML identifies server settings and authentication hints;
it does not grant permission to use Thunderbird's OAuth client registrations.

MegaMail needs its own provider registrations and system-browser OAuth with
PKCE, state validation, loopback handling and app-owned keyring entries.
Keep HTTPS discovery, validate untrusted configuration, and separate setup
from token handling. One-address-first onboarding should make common providers
easy without promising universal one-click authorization. See the
[onboarding research](research/account-onboarding.md) for official sources
and the Google launch/verification requirements. No live authentication is
connected by the initial preview.

## Performance contract

Virtualization bounds instantiated UI rows; it does not bound the backing
mailbox. Use cache pages and a bounded header window for real accounts. Fetch
and decode bodies and attachments on demand, with byte-bounded caches and
eviction. Search runs off-thread against an index; never clone or lowercase
the entire live mailbox each frame. Any new search index migration is
transactional and tested before it replaces existing query behavior.

Start with opaque Zeron-derived surfaces. If translucent chrome is added,
make it an OS capability with an opaque fallback; measure GPU allocations,
frame times and idle residency. Avoid animated full-window blur and copying
Zeron's fork solely for effects.

Initial **targets, not measured results**: cached first screen below 250 ms,
warm selection below 50 ms, scroll frame time p95 below 16.7 ms at 60 Hz,
idle CPU below 1%, and no RSS growth after repeated folder switches once
caches reach their limits. Record hardware, OS, build mode, cache warmth,
mailbox distribution and p50/p95/p99. Set a memory budget after measuring the
Kit baseline; do not invent a number before the renderer is profiled.

Exercise 100k headers, large bodies, multiple accounts, burst sync, reconnect,
repeated search/cancellation and long idle sessions. Verify queue bounds and
that UI responsiveness survives slow disk and network. The detailed audit
and acceptance matrix live in [performance/security](research/performance-security.md).

## Release gates

- Separate MegaMail identity, data paths and credentials; explicit import only.
- Account/folder/UIDVALIDITY-safe cache identity and migration tests.
- No credential/body leakage in logs; no decrypted PGP bodies persisted.
- Remote content blocked with the new renderer; hostile-message tests.
- Durable drafts/outbox, recoverable mutation errors and honest send ambiguity.
- Keyboard navigation, visible focus, sufficient contrast, Unicode/bidi text.
- Actual platform builds and screenshot inspection; a Linux check proves
  nothing about Windows/macOS runtime behavior.
- Measured performance results before claiming the app is ultra performant.

## Licensing

Hylki's AGPL-3.0-or-later license remains the fork's license. Preserve its
copyright and [LICENSE](../LICENSE). Zeron is MIT; any reused material retains
its MIT attribution in [THIRD_PARTY_NOTICES](../THIRD_PARTY_NOTICES.md). No
Zeron logo, wallpaper or screenshot is bundled as a MegaMail product asset.

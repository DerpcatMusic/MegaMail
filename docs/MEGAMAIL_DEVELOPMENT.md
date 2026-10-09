# Developing MegaMail

The root manifest builds the original Hylki GTK application. The native
GPUI Kit client has an independent manifest and lockfile at
`apps/megamail/Cargo.toml`; its shared `megamail-core` dependency is in
`crates/mail-core`. Use explicit manifests so a command cannot accidentally
target the root app:

```sh
# Run the native client
cargo run --manifest-path apps/megamail/Cargo.toml --locked

# Run with fictional demonstration data and account mutations disabled
cargo run --manifest-path apps/megamail/Cargo.toml --locked -- --demo

# Test the native package and its core dependency
cargo test --manifest-path apps/megamail/Cargo.toml --locked

# Test the core library on its own
cargo test --manifest-path crates/mail-core/Cargo.toml --locked

# Format the two native workspaces
cargo fmt --manifest-path apps/megamail/Cargo.toml --check
cargo fmt --manifest-path crates/mail-core/Cargo.toml --check
```

The app uses shared Hylki worker, cache, model, and protocol source modules
through `#[path]` declarations. Changes to those modules can affect both the
root GTK app and the native client; keep the root package's tests and behavior
in view when changing shared code.

## Linux

Use Rust and Cargo 1.92 or newer. A graphical X11 or Wayland session with a
working Vulkan driver is required. Runtime use also needs GLib/GIO,
fontconfig, OpenGL/EGL or GLX, Vulkan, and a Secret Service implementation on
the session D-Bus for credentials. Account setup uses Gmail app passwords or
provider-discovered and manual IMAP/SMTP settings.

The official GPUI Kit Ubuntu 24.04 setup lists these development packages:

```sh
sudo apt install gcc g++ clang pkg-config libfontconfig-dev libwayland-dev \
  libwebkit2gtk-4.1-dev libxkbcommon-x11-dev libx11-xcb-dev libssl-dev \
  libzstd-dev vulkan-validationlayers libvulkan1
```

These are the upstream documented prerequisites; install equivalent
development packages on other distributions. A Vulkan loader package alone
does not supply a GPU driver. See the source-pinned [Kit report](research/gpui-kit.md)
for platform details and [installation](MEGAMAIL_INSTALL.md) for the per-user
installer.

On this workstation, Cargo uses `~/.cargo/config.toml`, the shared target
directory, and sccache. Never export `CARGO_TARGET_DIR` or `RUSTC_WRAPPER`.
Check for an active build before starting another; only one Cargo build, test,
or check should run at a time. Use a small job count for renderer builds.

## Current behavior and manual review

The app is a real account-backed client, not the earlier synthetic-only
preview. Add an account using a Gmail app password or reviewed provider
settings; or use `--demo` for fictional rows backed by `HylkiMockBackend`.
Demo mode disables sending, drafts, archive, and Outbox operations. With a
configured account, inspect per-account Unified Inbox paging and partial
failure warnings, account/folder browsing, loaded-header search and filters,
conversation expansion and chronology, and single-message read/unread, star,
and trash actions. Check that equal UIDs from different folders remain
distinct and that matching subjects alone do not create conversations. Also
review drafts, plain-text compose, attachment actions, links, and Outbox retry
and discard. The Outbox's queued state is a durable local save for later
sending, not a delivery receipt. Message bodies use native semantic formatting from a bounded HTML allowlist,
with a per-message plain-text view and conservative quote hiding. The demo
includes a linked LTR/RTL review thread with long URLs, headers, lists, code,
and a data table. Inspect it at 1060 and 1280 logical pixels in both themes;
check selection/copy, quoted replies, local table scrolling, and wrapping.
There is no browser CSS renderer or automatic remote-content loading. Settings should expose account management, Regular/Compact density,
conversation grouping, the default Unified Inbox, quote hiding, reduced motion,
and the active keyboard shortcuts.

The Thunderbird import path requires an installed Thunderbird 153+ binary.
Use **Find mail accounts** in account setup to discover local profiles, then
check that eligible email mailboxes are grouped by source profile, selected by
default, and imported together. Review per-account results and confirm saved
source references restore asynchronously after restart. The app should create
a private Thunderbird profile, keep the original profile unchanged, and return
a typed needs-login result when the headless clone cannot unlock a
Primary-Password-protected store. Close Thunderbird before retrying an import
after its encrypted credential store changes.

A private, authorized probe using six existing Thunderbird IMAP accounts
authenticated all six, discovered folders, and returned an initial Inbox
page (up to 100 messages) for each with parsed headers, bodies, and unread
metadata. Source
preference and credential-store stamps stayed unchanged; the probe sent no
provider-bound messages and requested no flag changes. This is scoped evidence for those
account configurations, not provider-wide qualification.

Thunderbird bridge requests are routed by ID, so a body request can complete
while a conversation request is still pending. Request timeouts remove only
that request; actual socket closure fails all pending requests and retires
the shared profile actor. Reconnection retains saved source references and
replaces stale session message identities. Tests cover late replies, shared
transport failure, startup failure ordering, and loading-state cleanup.

The first network refresh awaits folder discovery before selecting a folder.
Thunderbird can notify an update listener when discovery finishes before
the SELECT operation; see [Mozilla’s update and listener implementation](https://raw.githubusercontent.com/mozilla/releases-comm-central/master/mailnews/imap/src/nsImapMailFolder.cpp).
This ordering does not delay cached header pages. Metadata/body timeouts are
15/30 seconds; folders allow 90 seconds, while first refresh allows 165 seconds
for the two separately bounded Thunderbird URL operations. Network refresh
and conversation/body work run outside the profile actor.

The 2026-10-09 runtime fix passed 430 portable-core, 25 theme, and 87 native
tests, plus the Node bridge fixtures. A read-only release probe using a fresh
private clone verified authenticated headers, nonempty bounded bodies, and
unread metadata for all six existing accounts without modifying the source
preference or credential files. All six cached pages initially had zero rows;
all six returned rows after explicit network refresh. Aggregate timings were
1.125 s runtime startup, 0.363 s account metadata, 0.017 s cached headers,
50.060 s sequential network refresh, and 20.315 s for six sequential body
downloads (76.499 s total). These measure provider work, not UI frame rate;
the native app runs account refreshes in the background. An earlier warm-cache
probe returned six accounts in 38.305 s total with 0.494 s cached-header work.
One fresh-run attempt failed before clone preparation at the five-second
startup boundary; a retry against the still-empty clone passed. The probe
does not report a private startup error, so its exact cause is unconfirmed.

A separate Thunderbird 157 headless loopback test exercised production
new-message and reply send branches, capturing the plain-text body, Reply-To,
Bcc only in the SMTP envelope, and exact binary attachment bytes. Draft save
created a draft in the isolated runtime's Local Folders without triggering
SMTP; this does not establish remote Drafts synchronization. No real-provider
send or delivery test was run. For direct SMTP,
review the separate username/password and no-login choices independently from
required IMAP authentication.

Review Appearance in System, Light, and Dark modes, with independently
selected Light/Dark variants, accent and surface settings, and imported theme
removal/reset. Try Aurora, Midnight, Paper, and a custom image with Original,
Dither, ASCII, Halftone, and Scanlines. Check the 0–100% treatment, opacity,
and bottom-fade controls, blur choices 0/10/16, 240 ms crossfade, and restart
persistence. The 24/10/5% pane bleed caps use a sampled theme-contrast estimate;
that is not a per-pixel guarantee. MegaMail does not implement Zeron's
renderer-specific backdrop blur or per-primitive edge fades.

The current release captures in `docs/preview/` use fictional demo mail and
show the current interface, not live provider results. They cover dark, light,
search, conversation, compact rows, the reader at a smaller size, and
appearance. Inspect the native window on Linux at representative desktop
sizes, and check keyboard navigation, focus visibility, search, account setup,
and slow/offline states before treating a UI change as reviewed. The native
desktop has no web/mobile breakpoint contract. Windows and macOS need their
own implementation and runtime checks when porting begins.

## Verification scope

The core has automated tests for local behavior. The scoped Thunderbird probe
above exercised six existing account configurations, and loopback tests covered
Thunderbird compose/send behavior; neither the probe nor unit tests establish
provider-wide support, long-run sync, or real-provider send delivery.
Native input, screen readers, compositor behavior, and performance require
separate review. A successful direct-account **Test and add** checks the
incoming and outgoing servers entered for that account; it does not establish
support for every provider or account policy.

Measure a release build before making performance claims. Record the machine,
GPU, display refresh rate, build mode, dataset, and cache warmth. Keep the
research performance figures labeled as targets until measurements exist.

The root package, preserved Hylki packaging, and upstream workflows still
describe Hylki artifacts. Use the native app manifest and MegaMail installer
when working with the new client; do not treat an upstream Hylki release job
as a MegaMail release.

The native app patches GPUI Base 0.7.1 locally for HTML text direction and
formatting behavior; see [patch notes](../vendor/gpui-base/MEGAMAIL_PATCH.md).
The matching GPUI and Linux text backend patches delegate wrapping to the native
shaper and retain per-row logical source ranges for selection and hit testing;
see [GPUI notes](../vendor/gpui-pre/MEGAMAIL_PATCH.md) and
[backend notes](../vendor/gpui-pre-wgpu/MEGAMAIL_PATCH.md).
[Cosmic notes](../vendor/cosmic-text/MEGAMAIL_PATCH.md) explain explicit paragraph base levels.
The native bidi backend is currently verified on Linux.
Keep these patches limited to text layout and retain the upstream Apache licenses.

```sh
cargo test --manifest-path vendor/gpui-base/Cargo.toml --lib --locked -j 2
```

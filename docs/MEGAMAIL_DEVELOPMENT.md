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
configured account, inspect folder sync, search, message loading,
archive/restore, drafts, plain-text compose, attachment actions, links, and
Outbox retry and discard. The Outbox's queued state is a durable local save
for later sending, not a delivery receipt. Message bodies remain on the safe
plain-text path; there is no full-fidelity HTML renderer or automatic
remote-content loading.

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

A separate Thunderbird 157 headless loopback test exercised production
new-message and reply send branches, capturing the plain-text body, Reply-To,
Bcc only in the SMTP envelope, and exact binary attachment bytes. Draft save
created a draft in the isolated runtime's Local Folders without triggering
SMTP; this does not establish remote Drafts synchronization. No real-provider
send or delivery test was run. For direct SMTP,
review the separate username/password and no-login choices independently from
required IMAP authentication.

Review the Appearance view in both themes with Aurora, Midnight, Paper, and a
custom image. Check all five image treatments, strength and blur selections,
restart persistence, and the static cached result. Text contrast is guarded
using a sampled 5th/95th-percentile estimate; review high-contrast image areas
and do not describe this as a per-pixel guarantee or compositor blur.

The current captures in `docs/preview/` show sample mail and are retained as
visual fixtures. They are not captures of the current account-connected
client. Inspect the current native window on Linux at representative desktop
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

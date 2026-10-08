# Developing MegaMail

The root manifest builds the original Hylki GTK application. The new native
preview uses its own independent manifest and lockfile:

```sh
cargo run --manifest-path apps/megamail/Cargo.toml --locked
cargo test --manifest-path apps/megamail/Cargo.toml --locked
cargo fmt --manifest-path apps/megamail/Cargo.toml --check
```

The preview uses synthetic mail; selecting folders, searching, reading and
archiving affect only that process's sample model. Restarting resets it.
No account credentials are requested and no email is sent. The mail-engine
extraction is still a separate milestone; see
[architecture](MEGAMAIL_ARCHITECTURE.md).

## Linux

Use a modern Rust toolchain (the current GPUI Kit graph documents Rust 1.92+
as its minimum; verify the committed lockfile with your installed version).
A graphical X11 or Wayland session with a working Vulkan driver is required.
The official Kit Ubuntu 24.04 setup lists:

```sh
sudo apt install gcc g++ clang pkg-config libfontconfig-dev libwayland-dev \
  libwebkit2gtk-4.1-dev libxkbcommon-x11-dev libx11-xcb-dev libssl-dev \
  libzstd-dev vulkan-validationlayers libvulkan1
```

These are the upstream documented prerequisites; the plain-text preview does
not use WebView or speech. Install equivalent development packages on other
distributions. A Vulkan loader package alone does not supply a GPU driver.
See the source-pinned [Kit report](research/gpui-kit.md) for platform details.

On this workstation, Cargo must use `~/.cargo/config.toml` and the shared
target directory/sccache. Never export `CARGO_TARGET_DIR` or `RUSTC_WRAPPER`.
Check for an active build before starting another; one Cargo build/test/check
at a time. Use a small job count on the first renderer build.

## Verification scope

Library tests verify sample-state behavior. They do not verify real mail,
native input, screen readers, window-manager interactions or performance.
Inspect the actual native window at representative desktop sizes and test
keyboard/focus/search/archive before calling a UI change complete. Windows
and macOS need their own builds and runtime checks when porting begins.

Measure a release build for performance claims. Record machine, GPU, display
refresh rate, build mode, dataset and cache warmth. Keep all benchmark targets
labeled as targets until measurements exist.

The inherited `.github` workflows and Hylki packaging scripts still describe
Hylki artifacts. Do not use them as MegaMail release jobs. The added native
workflow checks the new preview only; no release or deployment is configured.

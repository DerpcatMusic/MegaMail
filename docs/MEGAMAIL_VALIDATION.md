# Initial native preview validation

Checked on 2026-10-08. This verifies the GPUI sample-data foundation, not the
Hylki mail-engine extraction, account setup, real mail or release performance.

## Automated checks

- `cargo build --manifest-path apps/megamail/Cargo.toml --locked -j 2` passed.
- `cargo test --manifest-path apps/megamail/Cargo.toml --locked -j 2` passed:
  three mailbox-state tests (filter/selection, archive/restore, keyboard
  boundary behavior) and one regression test for bundled mail-action icons.
- `cargo fmt --manifest-path apps/megamail/Cargo.toml --check` passed.
- Project/research Markdown local links resolve; the original Hylki license
  is preserved verbatim. The original root Cargo package and source remain
  unchanged.

The native package pins GPUI Kit 0.7.1 and commits its own lockfile. It builds
with Rust 1.98.1 on this Linux workstation. Cargo used the machine's shared
target directory and sccache; no target/wrapper override was set.

## Rendered interaction checks

The compiled app was opened on a dedicated Xvfb X11 display. Captures used
GPUI's X11 scale factor 1 at 1280×800 and the supported minimum 1060×640.
The host exposes AMD Radeon RX 6600 and integrated RADV Vulkan devices.
Xvfb reported DRI3 presentation warnings; successful captures therefore do
not establish physical-display GPU performance.

The following were exercised against the actual native window:

- Dark/light theme toggle, pane geometry, readable wrapping at both sizes.
- Case-insensitive `FRIDAY` search with one result and singular count.
- No-match search with clear empty list and reader states.
- Folder and message selection.
- Arrow-key selection to the last row at minimum height, including scrolling
  the selected row into view.
- Archive of a sample message, selection in Archive, restore, and the empty
  Archive view afterward. Counts reflected the local changes.
- Presence of mail/send/archive/restore glyphs from the registered Kit assets.

The screenshot review artifacts use only fictional `.example` sample mail.
They are app captures, unlike the linked Zeron screenshots used for research.

## Independent finish review

A separate agent reviewed the actual screenshots and source, identified the
Linux duplicate-caption risk, and re-reviewed the corrected chrome and four
replacement theme/size views. Its final disposition was to ship the sample
preview with real compositor verification retained as a release gate; no
material source findings remained in that scope.

## Not established

No real account was connected. No IMAP, SMTP, JMAP, provider OAuth, rich HTML,
network privacy, credential-store or durable-Outbox behavior is implemented
by this preview. Hylki's corresponding implementation remains at the root.

Client-side decorations are explicitly requested and the root uses Kit's
`window_border()` resize wrapper. A server-decoration fallback hides the
duplicate app title and uses a normal demo/theme toolbar. The final four
theme/size captures exercise that fallback because bare Xvfb has no window
manager. The final source batch passed the build and all four tests again.

A nested Weston 15.0.1 Wayland check was attempted, but the compositor
exited with a DMA-BUF mapping permission failure. It does not establish
client-decoration, Wayland or caption-control correctness.

No release benchmark, large-mailbox test, successful Wayland/compositor test,
screen-reader assessment, full window-manager caption-control test, Windows
build or macOS build was run. The native GitHub workflow is configured; local
checks do not imply a GitHub Actions run has passed.

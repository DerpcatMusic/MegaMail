# Native client validation

Checked on 2026-10-09 on Linux with Rust 1.98.1. MegaMail now runs Hylki's
mail worker through a portable core and a native GPUI Kit interface. The
checks below distinguish executable behavior from live provider testing.

## Mail core

`cargo test --manifest-path crates/mail-core/Cargo.toml --locked -j 2`
passed with **414 tests passed and 18 ignored**, including Thunderbird settings parsing and isolated bridge safety
and the manual SMTP username regression. Ignored tests include integrations requiring a real account or
external service. The cache permission test was also run separately with
`--ignored --test-threads=1` and passed.

The native app suite passed **61 tests**, with one private test ignored. It
covers chronological page truncation, Thunderbird folder/identity parsing,
bounded source manifests, request-size preflight, wallpaper limits, and the
redesign's appearance and theme behavior. The copied `zeron-theme` crate passed
**25 tests**. The existing mail-core result remains **414 passed and 18
ignored**. The JavaScript extension contract check passed for bounded binary
transfers and message-size preflight.

Coverage includes tagged send outcomes, durable Outbox MIME, credential
staging, stable account IDs, duplicate-account rejection, TLS setup validation,
discovery parsing, bounded and account-scoped cache pages, MIME draft
restoration, Bcc/thread/sender metadata, binary and inline-image attachments,
bounded reader text, and validated HTTP(S) links. Native IMAP draft restoration
checks `RFC822.SIZE` before fetching its body.

The original GTK package's locked Cargo metadata resolves. Its full build
was not run because this workstation lacks WebKitGTK 6 development files.
Shared worker changes therefore have portable-core coverage, rather than
a verified legacy GTK executable.

## Synthetic mailbox performance

The ignored `synthetic_100k_cache_paging_benchmark` passed in release mode.
It creates an isolated in-memory SQLite database with 100,000 headers:
90,000 in one Inbox and ten account/folder scopes with 1,000 each. It checks
ordering, cursor boundaries, scope isolation, and a full bounded traversal.

One run on this workstation's Ryzen 7 7800X3D measured:

| Operation | Elapsed |
| --- | ---: |
| Seed the database | 1.510 s |
| Verify all pages | 368.4 ms |
| Fetch 500 headers | 0.438 ms |
| Rare metadata search | 24.55 ms |
| Frequent metadata search | 0.987 ms |
| 100 account/folder switches | 43.63 ms |

Command:

```sh
cargo test --manifest-path crates/mail-core/Cargo.toml --release --locked -j 2 synthetic_100k_cache_paging_benchmark -- --ignored --nocapture
```

These are one-run cache timings, not end-to-end latency, network sync,
disk-cache benchmarks, message-body search, frame-rate measurements, or
physical GPU performance. The native list requests bounded pages and retains
at most 5,000 headers; the inherited worker can still perform larger sync work
on its background thread.

## Installation and native window

`tools/install-megamail.sh` built the locked release executable and installed
the per-user binary, SVG icon, and application-menu launcher. The installed
release binary SHA-256 is
`b38f591a81900e042ccd9659321cbdfdbe7c41ca1de986bd8fbaaf6d77c80588`. The installer
uses Cargo's configured shared target directory and two build jobs. No
`CARGO_TARGET_DIR` or `RUSTC_WRAPPER` override was used.

The executable opened successfully on Hyprland 0.56.2 using native Wayland
(`xwayland: false`). This verifies a real compositor startup, not caption
button correctness or a complete Wayland interaction assessment.

Isolated Xvfb captures use temporary config/data/cache directories
and fictional runtime mail fixtures. The desktop's personal accounts and
mail are excluded. Setup was checked in dark and light themes, centered in
the window, with server fields revealed through discovery or manual setup.
Gmail discovery displayed its IMAP/SMTP endpoints and app-password guidance.
An earlier bounded debug UI pass checked all five wallpaper treatments
with a custom image, private image import, persisted appearance, and the
1060×640 Appearance/composer footers. The current release captures in
`docs/preview/` use fictional demo messages and show the built-in Aurora
background with Original and Dither, along with the appearance controls and
theme menus. The search capture shows Unread narrowing 19 loaded rows to 2.
Search and All/Unread/Starred/Attachments filters apply to loaded messages in
the current folder. The final captures used Xvfb without a window manager;
earlier captures used Openbox.

The native and vendored theme suites cover theme import parsing, preference
persistence/reset, and wallpaper transition behavior. Imported-theme removal
has not been manually reviewed in the UI. This redesign turn added no new
real-provider send, physical-GPU, or performance measurements; the earlier
Thunderbird and cache checks below retain their separate scope.

Xvfb reported DRI3/Vulkan presentation warnings. Successful screenshots do
not establish physical-display rendering speed. The host has AMD Radeon
RX 6600 and integrated RADV devices.

## Remaining verification limits

An explicit read-only integration probe authenticated all six supported
accounts discovered in this workstation's Thunderbird profile, using an
isolated copy of its encrypted authentication store. Each account returned
remote folders and a bounded Inbox page; all six pages contained parsed sender
headers. The source prefs/key4/logins/cert9 file sizes and modification times
were unchanged. The initial probe took 30.70 seconds and sent no mail. A repeat with the
installed release and persisted private clone verified all six accounts again
and read one bounded message body from each in 17.14 seconds, without sending
mail or requesting flag changes. This validates
these saved accounts, not every provider or authentication configuration.

The exact extension also passed a headless loopback IMAP discovery/sync check;
its synthetic header fixture produced blank fields, so it is not evidence for
header parsing. A headless reply compose tab resolved the synthetic sender and
was closed without sending.

An exact-source synthetic loopback SMTP check passed new-message sending,
reply sending, and saving a draft. It verified plaintext, Reply-To, Bcc as
envelope recipients only, and exact binary attachment bytes; the draft added
no SMTP delivery. Sending uses Thunderbird's composer API because the
advertised `messages.sendMessage` method is unavailable in the installed
Thunderbird 157 runtime. A Node regression covers both send paths and compose
tab cleanup on success and failure. The installed-folder metadata path was
also rechecked against all six real accounts in 18.91 seconds.

No real-provider message delivery or desktop keyring unlock flow was tested. Imported
authentication stays inside Thunderbird; a Primary Password or fresh provider
login prompt cannot currently be shown by the headless runtime. Direct Gmail
setup requires an app password because MegaMail's own OAuth registration has
not been provisioned.

The reader uses safe plain text and explicit links/files rather than a full
HTML renderer. Rich drafts are flattened to text with their images preserved
as attachments. The inherited SMTP retry behavior can duplicate mail if a
server accepted a message but the success response was lost.

Screen-reader access, physical-display frame timings, complete client-side
caption/resize behavior, Windows, and macOS remain unverified. A configured
GitHub Actions workflow is not evidence that a remote run has passed.

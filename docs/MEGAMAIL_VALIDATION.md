# Native client validation

Checked on 2026-10-09 on Linux with Rust 1.98.1. MegaMail now runs Hylki's
mail worker through a portable core and a native GPUI Kit interface. The
checks below distinguish executable behavior from live provider testing.

## Mail core

The standard suites passed **524 tests**: portable mail core **423 passed,
19 ignored**, native app **76 passed** with one separate private runtime test
ignored, and the copied `zeron-theme` crate **25 passed**. The 20 ignored
cases require a private runtime, external integration, or explicit benchmark
invocation; they are not standard-suite passes.

The conversation tests cover account-scoped reference grouping, duplicate
copies, ambiguous identifiers, chronological Sent members, stable UI keys,
partial unified results, stale events, and owning-account actions. Preference
and composer tests cover bounded private persistence, escaped selectable text,
restorable quotes, and Reply All excluding the account's own aliases.
The JavaScript extension fixtures passed, including 65-seed batches, parent
reference traversal, ambiguity rejection, partial warnings, and continuation
retry. Formatting, installer syntax, and whitespace checks passed.

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

## Conversation grouping benchmark

The explicit ignored `groups_50k_headers_into_5k_threads_with_sent_and_inbox_members`
benchmark passed: **50,000 headers grouped into 5,000 conversations with 5,000
Sent members in 658.11 ms**, in one debug-mode run on this workstation. This
measures the pure grouping function, not the full mailbox rebuild, network,
rendering, frame rate, or physical GPU performance.

## Installation and native window

`tools/install-megamail.sh` built the locked release and installed the per-user
binary, icon, application-menu launcher, and license bundle. The installed
binary and configured shared-target release artifact both have SHA-256
`152abdf2b6684dec6ab6736785a5c1622567219a839771d8b478fa1181310ea2`.
The installer validated the dependency/license inventory. No `CARGO_TARGET_DIR`
or `RUSTC_WRAPPER` override was used.

An earlier release opened on Hyprland 0.56.2 using native Wayland
(`xwayland: false`). The current release was exercised under Xvfb at
1280×800 and 1060×640, with temporary private config/data/cache directories
and fictional mail from two account-scoped demo workers. Personal accounts,
mail, and credentials are excluded from published captures.

The current native QA checked:

- Unified Inbox and automatic Sent-header discovery, including identical UIDs
  in different accounts and a seven-member conversation.
- Reply-count expansion, chronological reader headers, owning-account Sent
  selection, and a single active child-row fill.
- Selectable body text copied and pasted into Search; body paragraphs and
  blank lines remained intact.
- Conservative quote hiding and restoration, and Reply All from an own Sent
  message addressing its original recipient while excluding the sender.
- Search narrowing to one thread among 24 loaded headers; Attachments narrowing
  to four conversations; the All/Unread/Starred/Attachments menu.
- Dark and Light themes, persisted Compact 64px rows, and scrollable Mail,
  theme, wallpaper, account, and keyboard-shortcut settings.
- Small-window reader, settings, and composer, with fixed action footers.

`docs/preview/` contains fictional current-release dark, light, search,
settings, expanded-conversation, compact, and small-reader captures. A fresh
finish review returned **ship** after recapturing centered avatar initials and
single-row conversation selection. Its persistence, fidelity, ceiling,
material-fix, and keep checks followed the pinned Zeron direction; this is
visual evidence, not a broader provider or performance certification.

The native and theme suites cover theme import, persistence/reset, and wallpaper
transitions. An earlier isolated UI pass exercised all five wallpaper treatments
with a custom image. Imported-theme removal has not been manually reviewed.
Owned demo processes and the virtual display were stopped after capture.

Xvfb reported presentation/portal warnings. Screenshots do not establish
physical-display rendering speed. No new real-provider delivery, flag-mutation,
physical-GPU, or complete Wayland interaction test is claimed for this release.

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

Search is limited to loaded headers; full-history/body search, bulk triage,
inline reply, and rich HTML composition are not ported. Related Thunderbird
lookups inspect at most 128 candidate headers across 8 folders and walk up to
24 references; incomplete coverage is reported, and every historical or future
reply is not guaranteed. Pane translucency and cached wallpaper blur do not
provide compositor backdrop blur.

The reader uses safe plain text and explicit links/files rather than a full
HTML renderer. Rich drafts are flattened to text with their images preserved
as attachments. The inherited SMTP retry behavior can duplicate mail if a
server accepted a message but the success response was lost.

Screen-reader access, physical-display frame timings, complete client-side
caption/resize behavior, Windows, and macOS remain unverified. The current
source commit `cd517ba` passed the [Linux native GitHub Actions run](https://github.com/DerpcatMusic/MegaMail/actions/runs/37865816007),
including core/theme/native tests, formatting, bridge fixtures, and executable
checking. Later documentation-only changes do not add provider or performance
coverage.

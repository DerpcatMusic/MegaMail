# MegaMail architecture

## Decision

MegaMail is a Linux-first native email client built with the official
`gpui-kit` crate and the Hylki Rust mail implementation. Hylki's full history
and original GTK application remain at the repository root. The native app
uses its own package, account profile, persistent cache namespace, and keyring
service; it never imports or edits an existing Hylki profile automatically.
Zeron remains the pinned visual North Star. See the [design contract](../DESIGN.md)
and the source-linked [research](research/README.md).

The fork starts from Hylki commit
`43f2d4665f01a1f0ef4474d719179b8e5b61471c`. `apps/megamail` is an independent
GPUI Kit application with its own manifest and lockfile. It depends on
`crates/mail-core`, which exposes the existing portable Hylki modules through
Rust `#[path]` declarations into the root `src/` files. That shares real
worker, cache, model, and protocol code; it does not create a second stub mail
engine. MegaMail-specific configuration, discovery, onboarding, query,
OAuth, desktop integration, and message-text handling live in the core crate.

## Runtime shape

```text
GPUI Kit window and retained mailbox state
       ├── direct-account requests ──► Hylki worker ──► IMAP / SMTP
       │                              events/results ──► UI event channel
       ├── Thunderbird requests ────► Thunderbird adapter
       │                              command queue ──► private Unix socket
       │                                                 MailExtension
       │                                                 headless Thunderbird clone
       └── cached direct-account list/search ──► bounded QueryService
                                                SQLite cache + read-only WAL reader
```

The UI owns selection, current account and folder, search text, pagination,
composer state, and presentation. Account workers own network sessions and
mail operations. Network events arrive through a channel and are applied to
retained UI state. SQLite page queries run on the named `megamail-query`
thread, outside GPUI rendering and input handling. The query queue is bounded;
the app requests 100-row pages, and the cache layer caps a page at 500 rows.
Rapid first-page requests for the same account and folder can supersede queued
work. Each query carries a generation; results for an older search or folder
are discarded instead of replacing current rows. The visible mailbox also
caps the loaded header window at 5,000 rows and reports when that limit is
reached.

## Profiles, cache, and credentials

MegaMail stores non-secret account metadata in
`$XDG_CONFIG_HOME/megamail/accounts.toml` (normally
`~/.config/megamail/accounts.toml`). Its app directory is private on Unix, and
the account file is written with private permissions. Direct-account passwords, app
passwords, and tokens go through the desktop Secret Service under the
`com.megamail.MegaMail` service name; they are not stored in account TOML.
Thunderbird source references live in a separate private
`thunderbird-sources.json` manifest. Thunderbird's encrypted credential-store
files are copied into its private clone under
`$XDG_DATA_HOME/megamail/thunderbird/profiles/`; only Thunderbird handles those
files. This path is separate from both MegaMail's Secret Service entries and
its Hylki SQLite cache.

The shared SQLite cache is under MegaMail's own data namespace at
`$XDG_DATA_HOME/megamail/hylki/cache.db` (normally
`~/.local/share/megamail/hylki/cache.db`). The existing Hylki cache code and
schema are reused, while the MegaMail base path keeps this database apart from
the legacy Hylki cache. The query thread opens and initializes the cache, then
uses a separate read-only WAL connection for pages. No automatic cache,
profile, or credential migration is performed.

## Account setup and current mail flow

The first-run setup supports Gmail with an app password. Provider discovery
can use Thunderbird's public ISPDB and
provider-hosted autoconfiguration; a manual form accepts separate incoming
IMAP and outgoing SMTP hosts, ports, and TLS modes. Password-based setup
allows implicit TLS or STARTTLS, rejects plaintext, and tests both servers
before saving the direct account. For SMTP, the form supports password
authentication with either the IMAP password or a separate SMTP
username/password, and a no-login mode. Direct IMAP still requires a password
or app password. MegaMail does
not provide Google OAuth sign-in; Google accounts must allow app-password
access. A future direct Google OAuth integration would require a MegaMail-owned
client registration. Thunderbird's OAuth client registration is not copied
into MegaMail's direct-auth path; imported Thunderbird authentication stays
inside its isolated Thunderbird runtime.

Thunderbird import is a separate account path from MegaMail's password-based
Hylki worker. MegaMail finds profiles in `~/.thunderbird` and the official
Flatpak profile root, reads literal allowlisted `profiles.ini` and `prefs.js`
settings, and groups the real IMAP email accounts by profile. The picker
preselects eligible email mailboxes and submits all selected accounts in one
pass. Each account receives a persistent MegaMail ID and an outcome; the
selected source profile and email are recorded in the private
`thunderbird-sources.json` manifest so saved references can be restored on
startup.

For this path, MegaMail requires a locally installed Thunderbird 153+ binary.
It launches Thunderbird headlessly against a private per-profile clone and
bundles its MailExtension with the app. A generated host wrapper connects the
extension to a private Unix-domain socket. The clone receives only allowlisted
account preferences and a bounded copy of Thunderbird's encrypted credential
files (`key4.db`, `logins.json`, and `cert9.db`). MegaMail does not decrypt,
display, or log credential contents; Thunderbird reads them. A changed source
credential snapshot creates a new private clone; close Thunderbird before
refreshing an updated encrypted store. Source mail files and Local Folders are
not copied, and the original profile is not modified. The runtime rewrites
account mail paths to directories under MegaMail's private data area.

The isolated process is headless and cannot show a Thunderbird Primary
Password or provider login prompt. If the cloned store requires interactive
unlock, that account returns a typed needs-login result and remains unavailable
in MegaMail. Standalone Gmail setup still uses an app password; there is no
GNOME Online Accounts startup or sign-in flow.

The source profile's mail store, Local Folders, and queued Outbox items are not
cloned or automatically sent. Mailboxes stay on Thunderbird's IMAP path; new
queued sends are managed by Thunderbird, and MegaMail does not offer native
Outbox retry/discard controls for those accounts.

The bulk import UI is in [`main.rs`](../apps/megamail/src/main.rs#L1462),
the profile parser is in [`thunderbird.rs`](../crates/mail-core/src/thunderbird.rs#L88),
the private runtime is in [`thunderbird_bridge.rs`](../crates/mail-core/src/thunderbird_bridge.rs#L104),
and the session adapter is in [`thunderbird_adapter.rs`](../apps/megamail/src/thunderbird_adapter.rs#L675).
The bundled [MailExtension](../tools/thunderbird-bridge/extension/background.js#L83)
handles account and folder discovery.
A private, authorized probe using six existing Thunderbird IMAP accounts
authenticated all six, discovered their folders, and returned each account's
initial Inbox page (up to 100 messages) with parsed headers, bodies, and unread
metadata.
Source preference and credential-store stamps stayed unchanged; the probe sent
no provider-bound messages and requested no message-flag changes. This is evidence for
those account configurations, not a provider-wide compatibility guarantee.
A separate Thunderbird 157 headless loopback test exercised production
new-message and reply send branches, capturing the plain-text body, Reply-To,
Bcc only in the SMTP envelope, and exact binary attachment bytes. Draft save
created a draft in the isolated runtime's Local Folders without triggering
SMTP; this does not establish remote Drafts synchronization. No real-provider
send or delivery test was run.

Direct IMAP account workers use the shared Hylki cache and load cached headers
in pages. Search is applied through cache page queries; body and attachment
requests remain explicit operations. Archive and restore use the existing
worker path. Direct-account drafts and sends also go through the worker rather
than blocking the UI thread. Thunderbird accounts use their isolated profile
and runtime API path; their account references and state are not written to
the Hylki SQLite cache.

For direct accounts, sending uses a request identifier generated by the
native UI and returned in the completion event, so a late completion is
matched to the correct composer request. A direct-account message reported as
queued has been persisted in the SQLite Outbox; that state means it can be
retried, not that the recipient received it. The direct-account Outbox view
exposes retry and discard. SMTP can fail after the remote server has accepted
a message but before the client receives confirmation, so the interface must
not promise exactly-once delivery in that ambiguous case. Thunderbird sends
and drafts use the Thunderbird runtime API; account retrieval was exercised in the scoped probe, while new-message/reply
send and draft-save branches were exercised only against loopback servers.
Real-provider sending and delivery remain unverified.

## Reader, links, and attachments

Incoming message bodies use the safe plain-text path. The native reader does
not render arbitrary email HTML in a browser engine or automatically fetch
remote images and other resources. The core can extract a separate, bounded
list of links from HTML anchors; only validated `http` and `https` destinations
without embedded user information are offered, and the user explicitly opens
one in the browser.

Attachments are listed separately from the body. The app first shows cached
attachments and requires an explicit **Download attachments** action when a
network fetch is needed. Each available file has an explicit **Save** action
using the native save dialog. The composer supports file-picker attachments,
with a limit of 20 files, 25 MiB per file, and 50 MiB total. These paths do
not imply HTML rendering, inline attachment preview, or automatic remote
content loading.

## Appearance and Zeron material adaptation

The native app retains its dark/light `Palette::new` roles as the base tint
colors. `zeron_background` paints a full-window gradient or cached custom
wallpaper beneath the panes; the rail, message list, and reader then use
translucent palette tints. Their nominal maximum wallpaper bleed is 20% for
the navigation rail and 5% each for the list and reader. For a custom image,
the allowed artwork opacity is reduced using Zeron's sampled 5th/95th
percentile contrast estimate, targeting at least 4.5:1 for the sampled worst
color. This is an estimate, not a per-pixel contrast guarantee.

The Appearance view offers Aurora, Midnight, and Paper backgrounds, plus an
optional custom image; dark/light mode is independent. Custom PNG, JPEG, or
WebP input is limited to 24 MiB and 16 million pixels, with a 96 MiB decode
allocation limit. MegaMail normalizes the saved private PNG to at most 2500px
on its longest edge and 32 MiB stored size, and does not retain the source
path. Original, Dither, ASCII, Halftone, and Scanlines effects plus blur are
processed off the UI/render path into a cached static `RenderImage`; wallpaper
strength controls the painted image's opacity. There is no live compositor
blur. The choices persist in `$XDG_CONFIG_HOME/megamail/appearance.v1`, with
the normalized image under `$XDG_CONFIG_HOME/megamail/wallpapers/`.

The material pipeline and Zeron wallpaper effects are in
[`zeron_background.rs`](../apps/megamail/src/zeron_background.rs#L40),
[`appearance.rs`](../apps/megamail/src/appearance.rs#L25), and
[`zeron_wallpaper.rs`](../apps/megamail/src/zeron_wallpaper.rs#L33). Compact
sidebar interactions use Zeron's soft neutral wash and its primary/muted/faint
text roles through [`zeron_style.rs`](../apps/megamail/src/zeron_style.rs#L35);
the native palette values remain authoritative.

## Current boundaries

MegaMail's direct-account path has not yet been verified against a live Gmail
account or another real provider. The setup screen's **Test and add** checks
the entered incoming and outgoing server connections, but does not prove
ongoing sync or send behavior against every account policy. Thunderbird's
separate six-account probe is summarized above and does not establish
provider-wide compatibility. No performance target in the research
is a measured result. Linux is the current target; native Windows/macOS
behavior needs separate implementation and runtime checks. This is a native
desktop layout, with no web or mobile breakpoint contract.

Further work should widen provider and account-policy coverage, validate the
client on real Linux compositors, and expand regression coverage for protocol
reconnects, cache migrations, stale results, attachment limits, and ambiguous
SMTP outcomes. Retain the real shared Hylki engine and the Zeron visual
contract as those areas evolve.

## Licensing

Hylki's AGPL-3.0-or-later license remains the fork's license. Preserve its
copyright and [LICENSE](../LICENSE). Zeron is MIT; reused material retains its
attribution in [THIRD_PARTY_NOTICES](../THIRD_PARTY_NOTICES.md). GPUI Kit and
its bundled assets retain their respective licenses.

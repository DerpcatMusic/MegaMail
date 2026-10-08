# MegaMail

MegaMail is a Linux-first native email client: a history-preserving
[Hylki](https://github.com/hyprlab/hylki) fork using
[GPUI Kit](https://github.com/longbridge/gpui-kit) and the visual language of
[Zeron](https://github.com/zeronsh/zeron).

The original Hylki GTK application and full Git history remain at the
repository root. The native client lives in `apps/megamail`; its `megamail-core`
library exposes shared Hylki Rust modules while keeping MegaMail account,
cache, and keyring paths separate. MegaMail does not automatically read or
import an existing Hylki profile.

The client connects Gmail with an app password and other IMAP mailboxes through
provider discovery or manually entered IMAP and SMTP settings. It syncs mail
and searches the loaded messages in the current folder, reads messages,
archives and restores them, saves drafts, and sends plain-text messages through
SMTP. Search and the All, Unread, Starred, and Attachments filters narrow that
loaded folder result; they are not server-wide search. SMTP can use password
authentication, a separate SMTP username and password, or no SMTP login.
For direct accounts, failed or offline sends can be retained in the durable
Outbox for retry or discard. Imported Thunderbird accounts keep queued messages
under Thunderbird's management; MegaMail has no native retry/discard controls
for that path.

The reader displays a safe plain-text body, with extracted links and
attachments shown in separate panels. Only validated HTTP(S) links are offered
and they open after an explicit action. Attachments require explicit download
and save actions; the inherited worker may also prefetch recent attachments
into the private cache. There is no full HTML-mail renderer or automatic
remote-content loading. Automated core tests cover local behavior; direct
account behavior still needs verification against live provider policies.

For existing Thunderbird setups, **Find mail accounts** discovers real IMAP
mailboxes in standard Linux Thunderbird profiles. The import picker groups
them by profile, preselects valid email accounts, and offers **Import all
selected**; connection results are reported per mailbox. The implementation
starts an installed Thunderbird 153+ binary in a private
headless profile and uses a bundled MailExtension over a local Unix socket.
The source profile is not modified. MegaMail copies allowlisted preferences
and Thunderbird authentication-store files to a private profile; Thunderbird
handles those files, including any encrypted login data. MegaMail does not
decrypt, display, or log credentials. The original mail store, Local Folders,
and queued source Outbox items are not copied or automatically sent. A
primary-password prompt is not available in the headless runtime, so accounts
that need that unlock cannot be imported yet.

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
Standalone Gmail setup remains available with an app password; no GNOME
Online Accounts or MegaMail-owned Google OAuth flow is provided.

Appearance follows Zeron's theme model: the standalone MIT theme crate is
copied from commit `0c4835d2b73aa632b7b4d626ee0826a25ec1c9b9` and supplies 19
built-in families with 30 variants. System mode follows the desktop appearance;
Light and Dark variants are selected independently. Accent and surface
overrides, imported-theme removal, and preference reset are available. Theme
imports accept VS Code JSON/JSONC theme files or extension `package.json`
files; they are normalized into MegaMail's model, not loaded as native Zeron
theme JSON.

The Appearance view also offers Aurora, Midnight, and Paper backgrounds plus
private custom wallpaper. Original, Dither, ASCII, Halftone, and Scanlines
work with built-in and custom backgrounds. Effect strength, wallpaper opacity,
and bottom fade range from 0–100%; blur is 0/10/16. Images are processed into
a static cache and crossfade over 240 ms. The window does not use Zeron's
renderer-specific backdrop blur or per-primitive edge fades.

```sh
cargo run --manifest-path apps/megamail/Cargo.toml --locked
```

See [Linux installation](docs/MEGAMAIL_INSTALL.md) and
[development setup](docs/MEGAMAIL_DEVELOPMENT.md). Running `cargo run` without
the manifest path still builds the original Hylki GTK application.

To inspect the desktop shell without configuring an account, run
`cargo run --manifest-path apps/megamail/Cargo.toml --locked -- --demo`.
Demo messages are fictional and demo mode disables sending, drafts, archive,
restore, and Outbox operations.

The images in `docs/preview/` are captures of the current release in demo
mode. They use fictional messages and contain no personal mailbox data; they
show the current interface rather than a live provider session:
[dark](docs/preview/dark.png), [light](docs/preview/light.png),
[search](docs/preview/search.png), and
[appearance](docs/preview/appearance.png). Current implementation and design decisions
are documented in the [visual contract](DESIGN.md). See
the [validation report](docs/MEGAMAIL_VALIDATION.md) for the tested scope and
limits; it does not establish provider-wide compatibility.

## Research and direction

[The research index](docs/research/README.md) brings together seven
source-linked reports and a portable visual atlas. The Zeron work covers its
surface, type, spacing, state, hierarchy, materials, platform behavior, and
rendering techniques—not only its palette.

- [Zeron design system](docs/research/zeron.md), [hierarchy](docs/research/zeron-hierarchy.md),
  [materials/rendering](docs/research/zeron-materials.md), and [visual atlas](docs/research/design-atlas.html).
- [Hylki extraction seams](docs/research/hylki.md), [GPUI Kit APIs](docs/research/gpui-kit.md),
  [performance/privacy audit](docs/research/performance-security.md), and
  [account onboarding](docs/research/account-onboarding.md).
- [Implementation architecture](docs/MEGAMAIL_ARCHITECTURE.md), [visual contract](DESIGN.md),
  and [product scope](PRODUCT.md).

MegaMail translates Zeron's compact hierarchy, text roles, interaction washes,
and layered materials into the official GPUI Kit 0.7.1 shell. Its standalone
theme model is preserved from the pinned Zeron source, while wallpaper is
processed into a cached still image beneath theme-contrast-bounded pane tints.
MegaMail is a native desktop app; this repository makes no web or mobile
breakpoint promises.

Current work is focused on broadening provider coverage and verifying
failure recovery against real providers. Performance figures in the research
remain proposed targets until measured against realistic mailboxes.

## Upstream and license

Forked from Hylki commit `43f2d4665f01a1f0ef4474d719179b8e5b61471c`.
The preserved [upstream README](docs/HYLKI_UPSTREAM_README.md) and Hylki docs
explain the original application; its packaging and workflows still refer to
Hylki.

MegaMail retains Hylki's **AGPL-3.0-or-later** license. See [LICENSE](LICENSE),
[upstream asset licenses](docs/LICENSE.md), and [third-party notices](THIRD_PARTY_NOTICES.md)
for Zeron MIT and Geist SIL OFL attribution. GPUI Kit and its bundled assets
retain their respective licenses.

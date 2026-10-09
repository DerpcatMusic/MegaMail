# Install and set up MegaMail on Linux

MegaMail is a native Linux mail client built on GPUI Kit and Hylki's Rust mail core. The per-user installer builds that app and adds a desktop launcher; it does not install the root Hylki GTK application, start a background service, or import an Hylki profile.

## Requirements

- Linux in a logged-in graphical X11 or Wayland session, with a working Vulkan loader and GPU driver.
- Rust and Cargo 1.92 or newer, Python 3, Bash, and standard install utilities.
- The distribution's GLib/GIO, fontconfig, OpenGL/EGL or GLX, and Vulkan runtime packages.
- A working Secret Service implementation on the session D-Bus for storing direct IMAP mail credentials. The app may ask the desktop keyring to unlock before it can add an account.
- Thunderbird 153 or newer installed as a native Linux executable if you plan to import accounts from a Thunderbird profile. Direct IMAP setup does not need Thunderbird.
- GPUI Kit's native build development packages. Its current Linux guide lists the Ubuntu 24.04 setup and equivalent requirements for other distributions in the [installation guide](https://github.com/longbridge/gpui-kit/blob/288767cc730ca4977852a7860f52a8465a61876f/website/docs/installation.md#L11-L49); this repository also summarizes them in [Linux development notes](MEGAMAIL_DEVELOPMENT.md#linux).

Gmail currently uses an app password. Other IMAP accounts use discovered or manually entered server settings and a provider-issued password or app password.

## Install

Run the installer from any directory in the repository checkout:

```sh
./tools/install-megamail.sh
```

It reads the target directory and binary name from Cargo metadata, builds the locked application in release mode using two jobs, then installs these per-user files:

- `~/.local/bin/megamail`
- `${XDG_DATA_HOME:-~/.local/share}/applications/megamail.desktop`
- `${XDG_DATA_HOME:-~/.local/share}/icons/hicolor/scalable/apps/megamail.svg`
- `${XDG_DATA_HOME:-~/.local/share}/megamail/licenses/` (third-party license bundle)

Open **MegaMail** from your desktop application menu. You can also start it in your graphical session with `~/.local/bin/megamail`. The app runs as a foreground desktop process; it does not daemonize or install a service.

## Add Gmail

When MegaMail has no saved accounts, it opens the **Add an account** form. Enter the Gmail address and press **Find settings**. MegaMail discovers Gmail's IMAP and SMTP settings. MegaMail's own Google OAuth registration is not configured, so this form asks for a Google app password rather than opening a Google sign-in flow. Generate an app password in your Google Account and enter it in the masked password field; do not enter your regular Google password. Google requires 2-Step Verification, and the app-password option can be unavailable for work or school accounts, Advanced Protection, or accounts configured to use only security keys. For those accounts, MegaMail does not currently offer Google OAuth. A future direct Google OAuth integration must use a MegaMail-owned client registration; imported Thunderbird authentication stays inside its isolated Thunderbird runtime. See [Google's current app-password guidance](https://support.google.com/accounts/answer/185833?hl=en).

Before adding the account, review the discovered server settings and any warnings. Press **Test and add**. MegaMail verifies both incoming IMAP and outgoing SMTP before it stores credentials in the desktop keyring and saves the non-secret profile metadata.

## Add another IMAP account

Enter the account's email address and press **Find settings**. If provider configuration is found, MegaMail fills in candidate IMAP and SMTP hosts, ports, and security settings. Review the provider and server details. If discovery has no usable result, or the provider gives you different server details, enter the incoming IMAP and outgoing SMTP hosts and ports manually. The form supports implicit TLS and STARTTLS; it does not accept plaintext connections. For manual or unrecognized settings, review the server details in the form before **Test and add** becomes available.

Enter the mail username and the provider-issued password or app password in the sign-in section, then choose **Test and add**. MegaMail checks both servers first. If either check fails, it displays the error and does not save the new account. Providers that require OAuth cannot be added through password-based IMAP unless MegaMail has its own compatible provider integration.

For SMTP, the form can use password authentication or **No login**. When SMTP uses a password, you can use the incoming password or provide a separate SMTP username and password. IMAP setup still requires a password or app password. Review any discovered or imported endpoints and authentication modes before testing.

## Import Thunderbird mailboxes

Choose **Find mail accounts** in account setup to inspect the standard Linux
Thunderbird profile locations (`~/.thunderbird` and
`~/.var/app/org.mozilla.Thunderbird/.thunderbird`). MegaMail reads allowlisted
`profiles.ini` and `prefs.js` settings, groups eligible IMAP mailboxes by
profile, preselects valid email accounts, and offers **Import all selected**.
Each account reports its connection result separately, so one failed mailbox
does not hide the others.

MegaMail starts Thunderbird 153 or newer in a private headless profile and
connects its bundled MailExtension over a local Unix socket. The private clone
copies allowlisted account preferences and Thunderbird's authentication-store
files (`key4.db`, `logins.json`, and `cert9.db`); Thunderbird handles their
contents, including any encrypted login data. MegaMail does not decrypt,
display, or log credentials. The clone rewrites server mail paths into
MegaMail's private data area. If credentials change, close Thunderbird before
importing the updated snapshot; MegaMail creates a new private clone for that
snapshot. The source profile is not modified.

The headless runtime cannot display Thunderbird's Primary Password or provider
sign-in prompt. Accounts that require an interactive unlock are currently
unavailable through this import path. This does not transfer Thunderbird's
OAuth client registration to MegaMail. Standalone Gmail setup continues to
use an app password, and MegaMail has no GNOME Online Accounts sign-in flow.

The source profile's mail store, Local Folders, and queued Outbox items are not
copied or automatically sent. New queued messages remain Thunderbird-managed;
MegaMail does not provide native Outbox retry/discard controls for imported
Thunderbird accounts.

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

For direct IMAP accounts, use the regular Gmail/manual setup steps above;
the six-account Thunderbird probe does not establish support for every
provider or account policy.

## Use MegaMail

The native mailbox opens a Unified Inbox by default, combining bounded Inbox header pages with independent paging per account and a warning when an account page fails. Account and folder browsing remain available. Message identity includes account and folder scope. Optional conversation grouping uses message references across folders, including Sent copies; matching subjects alone do not group messages. Related lookups inspect up to 128 candidate headers across at most 8 folders and walk up to 24 reference ancestors. Partial results produce warnings, and a thread is not guaranteed to include every historical or future reply. Reply rows can expand in the list and conversations open in chronological order. Read/unread, star, and trash actions apply to one message through its account worker. Search and the All, Unread, Starred, and Attachments filters operate on loaded headers in the active account/folder or Unified Inbox; full server-history and body search are not available. Settings can change Regular/Compact density, conversation grouping, the default Unified Inbox, quote hiding, and reduced motion.

The composer sends plain-text messages through SMTP, supports replies and forwards, saves drafts, and lets you attach files with the native file picker. It accepts up to 20 attachments per message, 25 MiB per file, and 50 MiB total. Reply All uses the active account and excludes your own addresses and aliases. Imported Thunderbird accounts use the separate runtime. The six-account probe validated authentication, folder discovery, and initial Inbox pages, while a separate Thunderbird 157 loopback test exercised new-message/reply sends and draft save. Broader provider coverage, long-run sync, and real-provider send/delivery remain untested; queued messages stay Thunderbird-managed.

The reader shows selectable, escaped plain text. It does not render email HTML or automatically load remote images and other resources. With quote hiding enabled, only a conservative trailing quote block collapses, and its text can be restored. Links extracted from HTML anchors appear in a separate **Links in this message** section; only validated HTTP(S) destinations are offered, and each opens only after you choose its browser button. Attachments appear in a separate panel: cached items can be shown, but **Download attachments** explicitly starts a network fetch when needed, and **Save** opens the native save dialog. The UI does not offer inline attachment previews, rich HTML composition, or inline reply.

Open **Appearance** from the sidebar to use System, Light, or Dark mode, choose the Light and Dark theme variants independently, tune accent and surface treatment, or import a VS Code JSON/JSONC theme or extension `package.json`. The theme registry contains 19 built-in families and 30 variants. Imported themes can be removed; Reset restores preferences and keeps the imported theme library. Imports normalize into MegaMail's Zeron theme model; native Zeron theme JSON is not accepted.

Choose Aurora, Midnight, Paper, or a custom image. Original, Dither, ASCII, Halftone, and Scanlines are available for built-in and custom backgrounds. Treatment strength, wallpaper opacity, and bottom fade range from 0–100%; blur choices are 0, 10, and 16. MegaMail accepts PNG, JPEG, or WebP input up to 24 MiB and 16 million pixels, decodes within a 96 MiB allocation limit, and normalizes images to a maximum 2500-pixel edge before saving a private PNG copy. The original file path is not retained. Image processing is cached, and background changes crossfade over 240 ms. Theme contrast bounds pane wallpaper bleed at 24/10/5% for rail/list/reader. MegaMail does not use Zeron's renderer-specific backdrop blur or per-primitive edge fades.

This checkout has automated core coverage, but it has not been verified against a live Gmail account or another real provider. A successful **Test and add** checks the entered account's IMAP and SMTP connection; it is not a claim that every provider or account policy is supported.

## Account data and credentials

MegaMail keeps direct-account metadata in its private `megamail` directory under `$XDG_CONFIG_HOME` (normally `~/.config/megamail/accounts.toml`). The account file uses private permissions; direct IMAP passwords and tokens go through the desktop Secret Service under MegaMail's own service name. The direct-account SQLite cache lives under `$XDG_DATA_HOME/megamail/hylki/cache.db`, separate from the legacy Hylki cache. Thunderbird source references are stored in a separate private `thunderbird-sources.json` file. Its isolated profile clones, including the Thunderbird-managed encrypted authentication files and private mail paths, are stored under `$XDG_DATA_HOME/megamail/thunderbird/profiles/`; they are separate from the Hylki SQLite cache. Account setup tests direct IMAP and SMTP servers before saving direct-account credentials. MegaMail does not read, migrate, modify, or delete Hylki's existing configuration or mail cache; Hylki profile import is not automatic.

Never put mail passwords, app passwords, OAuth tokens, or client secrets in chat, source files, shell history, or installer environment variables. Enter them only into the corresponding account screen. Community MegaMail builds do not use Hylki's OAuth registrations. If a provider later offers a documented MegaMail-owned OAuth configuration, use that provider's own registration instructions; do not copy another app's client credentials.

## Remove the per-user launcher

This removes the installed executable, launcher, icon, and generated license bundle. It leaves account settings, cached mail, and custom appearance data in place:

```sh
data_home="${XDG_DATA_HOME:-$HOME/.local/share}"
rm -f -- "$HOME/.local/bin/megamail" \
  "$data_home/applications/megamail.desktop" \
  "$data_home/icons/hicolor/scalable/apps/megamail.svg"
rm -rf -- "$data_home/megamail/licenses"
```

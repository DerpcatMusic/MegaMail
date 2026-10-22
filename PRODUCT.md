# MegaMail

<!-- impeccable:product-schema 1 -->

## Platform

Native Linux desktop. The visual language is Zeron's quiet, continuous canvas
and compact hierarchy adapted to email; Windows and macOS are future ports,
not verified targets.

## Stack

Rust with official GPUI Kit 0.7.1. Hylki supplies the mail implementation and
history. MegaMail includes a standalone MIT copy of Zeron's theme crate from
commit `0c4835d2b73aa632b7b4d626ee0826a25ec1c9b9`.

## Product Purpose

Build MegaMail as a dependable native Linux email client from Hylki's mail
engine, with Zeron's visual hierarchy and theme model expressed in official
GPUI Kit.

## Capabilities and Constraints

Hylki already implements multi-account mail, background sync, a local cache,
OAuth, composing, durable Outbox and privacy controls. Preserve that existing
behavior when extracting the mail core. A GPUI shell does not establish
feature parity or production readiness.

The native client uses Hylki's mail engine through `crates/mail-core` and
stores its profile, cache, and keyring entries separately. Gmail with an app
password and other IMAP accounts through discovered or manual settings use
MegaMail's Hylki-backed mail worker. A second path provides bulk selection
of IMAP mailboxes from a local Thunderbird profile, grouped by profile and
submitted in one pass; its implementation runs an installed Thunderbird
153+ binary with a private cloned profile and MailExtension/native Unix-socket
bridge. The source profile is
not edited, the encrypted credential store is copied into MegaMail's private
clone for Thunderbird to handle, and source mail storage is not cloned.
Accounts that need an interactive primary-password unlock are not supported
by the headless runtime. A private, authorized probe using six existing
Thunderbird IMAP accounts authenticated all six, discovered folders, and returned an initial Inbox page
for each with parsed headers, bodies, and unread metadata.
The source preference and credential-store stamps stayed unchanged; the probe
sent no provider-bound messages and requested no flag changes. This is evidence for
those account configurations, not provider-wide compatibility. A separate
Thunderbird 157 loopback test exercised new-message/reply sends and draft save.
No real-provider send or delivery test was run.

The native mailbox includes a Unified Inbox backed by independently paged
per-account Inbox results and visible partial-failure warnings. Message keys
retain account and folder scope. Optional conversation grouping follows
message-reference headers across folders and Sent; it never groups solely by
subject. Related lookups inspect up to 128 candidate headers across at most 8
folders and walk up to 24 reference ancestors; warnings report partial results,
and the client does not promise every historical or future reply will appear.
Expanded list replies open in chronological order, with the selected, newest and
latest sent messages available as distinct authored blocks. Own-message labels use
the folder role: “You · draft” in Drafts and “You · sent” in Sent, with the full
sender address visible. Single-message read/unread, star, and trash actions use
the owning account worker.

The shared rail exposes All inboxes, All Sent, All Drafts, Archived, All local
mail and Outbox. Direct-account search queries the local SQLite cache across the
selected folder or local-mail scope; `body:` covers downloaded, unencrypted
bodies only. All local mail excludes Trash, Junk and Templates. Legacy adapters
search loaded headers. No view establishes provider-global history coverage.
Date, sender email, subject and unread sorting support both directions; direct
accounts sort the local folder cache before paging, while legacy adapters sort
loaded headers. Unified lists merge bounded per-account pages, and work filters
apply to the loaded results.

The default reader is stable, selectable native GPUI semantic text with sanitized
HTML formatting. It retains native text states and never automatically starts a
WebKit helper or replaces text with a rendered image. Current authored content
and recognized quoted history have separate selectable text states; history can
be disclosed independently, preserving nested attribution. Explicitly hidden
inline preheaders are omitted from semantic mail. The semantic view removes
message CSS and offers a plain-text mode; it simplifies authored table layout.
**Original formatting…** deliberately opens an isolated, ephemeral WebKit window
for eligible complete source, preserving authored colors, typography and table
layout while blocking remote resources. Closing it leaves native text intact.
Unknown or incomplete source, including unattested direct-worker MIME, reports
unavailability rather than reconstructing an original from sanitized text.
Reply All excludes the user's own addresses and aliases. Attachments retain
explicit open, download and save actions.

Sender names and addresses remain selectable outside each message's disclosure
button. Message details display the actual available authentication summary and
findings; missing evidence says unavailable. Photos come from explicit local
PNG/JPEG/WebP choices or a configured own-account avatar, with initials as the
fallback. Thumbnail loading and decoding run in bounded background work with at
most two concurrent loads. No automatic Gravatar lookup, remote photo fetch or
contact synchronization is performed. Native draggable pane dividers enforce
minimum widths; expanded and compact layouts retain their separate sizes during
the session and window resizing, but do not persist across restart.

Navigation retains an active composition, and Resume draft returns to it. Plain
text is the default; opt-in Markdown composition includes a formatted preview
and preserves recovered HTML. It is not a WYSIWYG editor. Settings expose account
management, Regular/Compact density, conversation grouping, default Unified
Inbox, quote hiding, reduced motion, and the current keyboard shortcuts.

Conversation work states—Needs reply, Waiting, Done and one-day Snoozed—are saved
on this device independently of read/unread and provider flags. Newer inbound
mail reopens Waiting/Done, and expired snoozes reappear. Suggested states are
explicitly distinguished from saved work; demo changes last only for the session.
Per-account connection errors and observed folder-sync times remain visible,
without claiming complete provider synchronization.

An outgoing message appears immediately as a provisional reply card with its
actual pending, queued, accepted, uncertain or refused state. Reconciliation
requires the exact account and Message-ID. These cards are session-local;
durable Outbox tickets and recovered drafts carry restart and retry authority.
Server acceptance is not delivery, and an unknown outcome never triggers an
automatic retry.

SMTP supports a separate username/password or a no-login choice for MegaMail's
direct IMAP path. Gmail app-password setup remains available; there is no
GNOME Online Accounts flow or MegaMail-owned Google OAuth registration.
Imported Thunderbird credential-store files remain with Thunderbird in its
isolated profile and do not provide a MegaMail OAuth client registration.
Performance claims require representative measurements.

Appearance includes 19 built-in theme families with 30 variants, independent
Light and Dark selections, System mode, accent and surface controls, and local
VS Code JSON/JSONC or extension-package theme import. Imported themes can be
removed; Reset restores preferences while retaining the theme library. Aurora,
Midnight, Paper, and custom wallpaper support Original, Dither, ASCII,
Halftone, and Scanlines, 0–100% treatment strength and wallpaper opacity, blur
values 0/10/16, and bottom fade. Cached wallpaper crossfades over 240 ms behind frosted navigation, message-list, reader and composer panes. Each pane lets the wallpaper show through by a fixed share (rail 52%, list 36%, reader and composer 24%), capped by the image's measured contrast-safe opacity so text stays legible. This does not include
Zeron's renderer-level backdrop blur or per-primitive edge fade.

## Brand Commitments

The name is MegaMail. The user explicitly wants much of Zeron's look and a
deep understanding of its hierarchy, design system and visual language.
Translate the existing Zeron language to email rather than inventing a new
visual identity. Preserve upstream attribution and applicable licenses: the
Hylki-derived application/mail core remains AGPL-3.0-or-later, and the standalone
Zeron theme copy remains MIT.

## Evidence on Hand

The full Hylki history is retained. Source-pinned research lives in
`docs/research/`. Zeron screenshots remain reference material; current
MegaMail screenshots are release captures using fictional demo data. The native
application is separate from Hylki's existing GTK package.

## Open Decisions

The public GitHub fork is `DerpcatMusic/MegaMail`, in Hylki's fork network.
WYSIWYG rich HTML editing, provider-global search, complete historical thread
coverage, reminders beyond one-day snooze, cloud work-state synchronization,
broader provider qualification, and MegaMail-owned public OAuth registrations
remain separate launch work. See `docs/MAIL_COMFORT_CHECKLIST.md` for the daily-use
guide and qualification boundaries.

## Product Principles

- Preserve working mail behavior before changing its presentation.
- Derive hierarchy, density, materials and interactions from Zeron evidence.
- Keep disk, network and heavy parsing away from the UI thread.
- Preserve privacy, data integrity and honest operational states.
- Measure performance instead of treating the toolkit as proof.

## Added in the composer and reader pass

Recipient completion (saved contacts, then senders visible in the loaded list; nothing is stored),
8-second undo send for immediate sends, plain-text account signatures on new, reply and forward
mail, a Markdown formatting strip, file drop onto the composer, opt-in spell check through the
system `hunspell`, an opt-in offline English grammar check (Harper; quoted lines, links and
signatures are skipped), Junk/Trash/Archive keys (J K R A C / E # ! with the list
focused), Save as .eml and Forward as attachment through the worker's raw export, a warning before
opening deceptive links, a Mark all read action, Ctrl/Shift-click multi-select, a folder picker for
bulk moves, desktop notifications for new Inbox mail (`notify-send`; a Settings toggle), and the
unread count in the window title. Settings has a per-account signature editor (explicit Save, never
in demo). The reader's details offer Save to contacts, Write to, Mail from sender and Copy address.
Snooze has presets and a typed date. Archive, Trash and Junk wait 5 seconds behind an Undo banner
(Ctrl+Z); the move is deferred, not reversed, so quitting inside the window drops it. Focus rings
are always visible and F6 cycles panes.

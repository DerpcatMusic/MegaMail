# MegaMail

<!-- impeccable:product-schema 1 -->

## Platform

Linux desktop first. Windows and macOS remain future ports, not verified
targets of the initial foundation. This is a native desktop product; the
mobile/web platform categories do not describe it.

## Stack

Rust and the official GPUI Kit, as explicitly requested. Hylki is the mail
implementation source and history; Zeron is the binding visual reference.

## Product Purpose

Create a beautiful, highly performant native email client called MegaMail
as a fork of Hylki. The user explicitly asks for deep source research into
Hylki, Zeron and the required GPUI implementation, with multiple agents.

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

The reader renders safe plain text and presents validated links and
attachments separately for explicit actions. SMTP supports a separate
username/password or a no-login choice for MegaMail's direct IMAP path. Gmail
app-password setup remains available; there is no GNOME Online Accounts flow
or MegaMail-owned Google OAuth registration. Imported Thunderbird
credential-store files remain with Thunderbird in its isolated profile and do
not provide a MegaMail OAuth client registration. Performance claims require
representative measurements.

The appearance system retains dark/light semantic palette roles and Zeron's
compact hierarchy. It includes three built-in backgrounds and optional private
custom wallpaper with static image processing and contrast-guarded pane tints;
it does not rely on live desktop compositor blur.

## Brand Commitments

The name is MegaMail. The user explicitly wants much of Zeron's look and a
deep understanding of its hierarchy, design system and visual language.
Translate the existing Zeron language to email rather than inventing a new
visual identity. Preserve upstream attribution and applicable licenses.

## Evidence on Hand

The full Hylki history is retained. Source-pinned research lives in
`docs/research/`. Zeron screenshots and implementation are inspected as
references; its screenshots are not MegaMail screenshots. The native application is separate from Hylki's existing GTK package.

## Open Decisions

The public GitHub fork is `DerpcatMusic/MegaMail`, in Hylki's fork network,
following the requested public-source fork. The initial mail-core extraction is implemented. Full-fidelity rich HTML
and MegaMail-owned public OAuth registrations remain separate launch work.

## Product Principles

- Preserve working mail behavior before changing its presentation.
- Derive hierarchy, density, materials and interactions from Zeron evidence.
- Keep disk, network and heavy parsing away from the UI thread.
- Preserve privacy, data integrity and honest operational states.
- Measure performance instead of treating the toolkit as proof.

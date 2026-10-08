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

The first native foundation uses clearly labeled sample mail. It does not
connect accounts or send messages. Performance claims require measurements
on representative mailboxes and hardware.

## Brand Commitments

The name is MegaMail. The user explicitly wants much of Zeron's look and a
deep understanding of its hierarchy, design system and visual language.
Translate the existing Zeron language to email rather than inventing a new
visual identity. Preserve upstream attribution and applicable licenses.

## Evidence on Hand

The full Hylki history is retained. Source-pinned research lives in
`docs/research/`. Zeron screenshots and implementation are inspected as
references; its screenshots are not MegaMail screenshots. The native preview
is separate from Hylki's existing GTK package.

## Open Decisions

The public GitHub fork is `DerpcatMusic/MegaMail`, in Hylki's fork network,
following the requested public-source fork. Initial email-core extraction,
rich-mail renderer and provider registrations are implementation work still
to do.

## Product Principles

- Preserve working mail behavior before changing its presentation.
- Derive hierarchy, density, materials and interactions from Zeron evidence.
- Keep disk, network and heavy parsing away from the UI thread.
- Preserve privacy, data integrity and honest operational states.
- Measure performance instead of treating the toolkit as proof.

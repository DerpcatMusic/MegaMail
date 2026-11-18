---
title: "Mail images: CSS backgrounds need a second message pass, and DMARC-failing senders are trusted per sender"
author: DerpcatMusic
date: 2026-11-18
branch: superhuman-mcp
cost: { minutes: 47, tokens: 155696 }
id: l-4298d2b7
scope: repo
status: active
src: agent
touches:
  - apps/megamail/src/compose_markdown.rs
  - apps/megamail/src/composer_ui.rs
  - apps/megamail/src/list_ui.rs
  - apps/megamail/src/main.rs
  - apps/megamail/src/original_ui.rs
  - apps/megamail/src/reader_ui.rs
  - apps/megamail/src/setup_ui.rs
  - apps/megamail/src/image_trust.rs
  - apps/megamail/src/original_view.rs
---

## Decided
- Blitz requests CSS background images while styles resolve, after the first handle_messages(); a second handle_messages()+resolve() applies the cached bytes. Without it Instagram post thumbnails were blank even with images allowed.
- Images stay blocked for DMARC-failing mail by default; the reader offers "Always from this sender" (lowercase address list) instead of weakening the forgery rule. Junk never loads.

## Tried and ruled out
- The URL encoding (&amp; in style attributes) and declaration order were not the cause; fetches returned 200.

## Watch out
- Template bugs like width="undefined" height="NaN" make Blitz draw an img at zero size; such attributes are stripped.
- Blitz ignores the size part of the background shorthand (url(...) center / cover); longhands are added.
- HEIC images can't be decoded.

In the code: render_now and tame_inline_tags in apps/megamail/src/original_view.rs; image_trust.rs (image-senders.v1.json); MM_REMOTE=1 MM_HTML=file MM_OUT=dir cargo test render_file -- --ignored

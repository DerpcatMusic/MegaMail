---
title: Blitz mail layout bugs fixed by inline-tag rewrites
author: DerpcatMusic
date: 2026-11-17
branch: superhuman-mcp
cost: { minutes: 25, tokens: 910 }
id: l-141bf52d
scope: repo
status: active
src: agent
touches:
  - "apps/megamail/src/original_view.rs#tame_inline_tags"
---

Instagram/Meta mail rendered with missing photos, overlapping avatars and vanished buttons. Isolated with tiny pages through render_file (MM_HTML + MM_OUT): Blitz 0.3.0-alpha.4 (1) drops a <td style=display:block> entirely, text included; (2) gives a CSS-sized <img> zero width when its sibling cell is width:100%; (3) ignores img width/height attributes when the img is display:block. A div of the same size works, so it is replaced-element min-content, not tables in general. tame_inline_tags strips display from td/th, adds min-width to imgs <=160px (not larger: hero images must stay responsive), and copies size attrs into the style for display:block imgs. Remove these when Blitz fixes them; re-check with the same tiny pages.

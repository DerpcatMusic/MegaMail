---
title: "Icons render blank unless bundled, and a gradient background hides the custom wallpaper"
author: DerpcatMusic
date: 2026-11-18
branch: superhuman-mcp
cost: { minutes: 19, tokens: 184213 }
id: l-95421562
scope: repo
status: active
src: agent
touches:
  - apps/megamail/src/appearance_view.rs
  - apps/megamail/src/batch_send.rs
  - apps/megamail/src/comfort.rs
  - apps/megamail/src/main.rs
  - docs/HANDOFF.md
---

## Decided
- Every IconName the app uses must be listed in the app's bundled icon set, otherwise the button renders an empty gap with no error.
- When a custom wallpaper exists but a gradient preset is selected, Background settings show "Use my image" rather than silently switching presets.

## Watch out
- "No blur, no fade" reports usually mean preset is a gradient (e.g. midnight) while wallpaper_file is set: blur/fade only process the custom image.
- Only ~106 Lucide icons ship in gpui-kit-assets default-icons.txt; reply-all is not among them.

In the code: icon_assets!(MailIcons, [...]) in apps/megamail/src/main.rs; background_row in apps/megamail/src/appearance_view.rs; appearance.v1 keys preset / wallpaper_file.

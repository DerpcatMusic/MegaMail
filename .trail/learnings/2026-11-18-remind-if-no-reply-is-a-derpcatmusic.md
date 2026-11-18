---
title: Remind if no reply is a snooze that any reply cancels
author: DerpcatMusic
date: 2026-11-18
branch: superhuman-mcp
cost: { minutes: 11, tokens: 285726 }
id: l-3a9423b6
scope: repo
status: active
src: agent
touches:
  - apps/megamail/Cargo.lock
  - apps/megamail/Cargo.toml
  - apps/megamail/src/comfort.rs
  - apps/megamail/src/composer_ui.rs
  - apps/megamail/src/main.rs
  - apps/megamail/src/reader_ui.rs
  - apps/megamail/src/scheduling.rs
  - apps/megamail/src/workflow.rs
  - docs/HANDOFF.md
---

## Decided
- No separate reminder state: "Remind" snoozes the replied-to conversation for 3 days, and any new inbound message now brings snoozed (as well as waiting and done) conversations back as Needs reply. Plain snoozes therefore also return early on a reply, matching Superhuman.
- gpui-kit 0.10 / gpui-fast upgrade deferred by the owner (2026-11-18): it requires re-porting the vendored RTL/bidi patches.

## Watch out
- Remind applies to the conversation selected when the reply is sent, which is the replied-to one when replying from the reader.

In the code: Record::effective in apps/megamail/src/workflow.rs; mark_work_until in comfort.rs; later_presets in scheduling.rs; remind flag in composer_ui.rs send path.

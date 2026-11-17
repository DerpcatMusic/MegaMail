---
title: Bulk unsubscribe sends email requests directly; single unsubscribe still opens a draft
author: DerpcatMusic
date: 2026-11-17
branch: superhuman-mcp
cost: { minutes: 15, tokens: 2033 }
id: l-4781700a
scope: repo
status: active
src: agent
touches:
  - "apps/megamail/src/subscriptions.rs#unsubscribe_senders"
---

## Decided
- In the bulk screen, pressing Unsubscribe (one row or Unsubscribe from N) is the user's go-ahead, so email routes send straight away from the account the newsletter arrived in. Ctrl+U on one open message keeps the reviewed draft.
- One-click POST only when the headers declared an https one-click route; a route guessed from the body falls back to email or web.
- Web-only senders open their page only when a batch holds at most 3, so a bulk action never sprays browser tabs; the rest are reported back.
- Rows: unsubscribable senders first, then emails per month over a 90-day window. Keep, done and Later (30 days) persist beside the screener file.

## Watch out
- An empty unsubscribe subject becomes "unsubscribe"; some lists reject blank subjects.
- Demo mode never sends or POSTs.

In the code: subscriptions.rs (Decisions, Sender::method, MailApp::unsubscribe_senders), Cache::subscription_senders in src/cache.rs, file subscriptions.v1.json.

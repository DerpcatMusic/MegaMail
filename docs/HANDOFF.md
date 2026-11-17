# MegaMail handoff (for the next agent)

Self-contained summary of the work done so far. Read this before touching the code.
Repo: `DerpcatMusic/MegaMail` (fork of Hylki). Working branch: **`superhuman-mcp`** (PR #13).
Native app: `apps/megamail/` (Rust + GPUI via gpui-kit 0.7.1). Also read `AGENTS.md` and `DESIGN.md`.

## Owner's direction (keep to it)
- Wants **Superhuman's UI and feel**: keyboard-first, fast, clean, minimal "slop", no clutter stacked vertically.
- **Implement more, test less.** Skip long verification loops. Don't stall on small things. Ship builds often.
- Deliver each build by replacing **`~/.local/bin/megamail`** (old one kept as `megamail.bak`).
- Auto-load everything (images, message bodies). Never show "Load message"/"Load images" by default.
  Exception: senders that fail DMARC (Fail/Suspicious) and Junk/Spam.

## Build / run
```bash
cargo build  --manifest-path apps/megamail/Cargo.toml            # dev
cargo build  --release --manifest-path apps/megamail/Cargo.toml  # ~10 min incremental
cargo test   --manifest-path apps/megamail/Cargo.toml
./apps/megamail/target/debug/megamail --demo                     # sample mail, sending off
megamail mcp                                                     # MCP server on stdio for agents
```
Linux deps (Debian names): libdbus-1-dev libglib2.0-dev libxkbcommon-dev libxkbcommon-x11-dev
libwayland-dev libfontconfig-dev libfreetype-dev libssl-dev libx11-xcb-dev libxcb1-dev libvulkan-dev.
Render one email to PNG for debugging: `MM_HTML=mail.html cargo test --manifest-path apps/megamail/Cargo.toml render_file -- --ignored --nocapture`
(writes `/tmp/render-dark.png` and `/tmp/render-light.png`).
Commits end with the session attribution lines; push with `git -c push.negotiate=true push`.
Do not commit `vendor/cosmic-text` LFS noise or `crates/mail-core/target`.

## What exists now (all on `superhuman-mcp`)
**Agents / MCP** – `src/mcp.rs`: stdio JSON-RPC (protocol 2025-06-18). Tools: list_accounts, list_folders,
list_messages, search_messages, read_message, send_message, reply, forward, save_draft (draft_uid edits),
update_messages (read/unread/star/archive/trash/spam/move). Uses saved profiles; no extra login.

**Layout (Superhuman)** – `main.rs`
- Permanent app bar (`app_bar`): folders toggle (Ctrl+B), search, layout (F8), list options, settings, Compose.
- Inbox: full-width single-line rows (`message_lines`), split tabs All/People/Screener/Updates/Newsletters (`splits.rs`).
- `preferences.layout`: Full (conversation replaces list, Esc back), Right (side by side, two-line rows
  `message_lines_two`), Below. F8 cycles; also Settings → Mail → Layout.
- Conversation view: back + "N of M", reader mode segmented control (Original/Clean/Text, V), actions,
  collapsed messages as one-line rows, right sender panel (`contact_pane`: photo, trust, domain, recent).
- Rows: hover actions (Done/Star/Read/Trash), right-click menu, trackpad swipe (left Done, right read).
- Sent rows show "To <names>" (`recipient_names`).
- Settings (`appearance_view.rs`): section nav, switches, segmented controls, keymap from `command_palette.rs`.
- Ctrl+K command palette; `?` shortcuts; Undo Send (10 s, Z); Ctrl+U one-click unsubscribe (RFC 8058);
  Screener Y/N; sender trust badges + one-line spoof warning; avatars (BIMI/brand logo/Gravatar opt-in).
- Animations: `MailApp::reveal()` (fade+rise) on conversation open, palette, settings, undo toast.
  Respect `preferences.reduced_motion`.
- Wallpaper: `appearance.rs` + `zeron_wallpaper.rs` (fade, blur, strength). Panes are translucent over it
  via `pane_surface()`.

**Email rendering** – `original_view.rs` (Blitz 0.3.0-alpha.4 on a render thread, CPU paint)
- Dark-mode recolor in CIELAB; brand colours keep chroma.
- Fixes for real mail: leading newline before `<!DOCTYPE`/`<?xml` (else Blitz uses XML parser and shows
  `<style>` as text); `* {border-collapse: separate !important}` + cellspacing/cellpadding mapping;
  `tame_div_tables` (div display:table/table-cell → inline-block columns, MJML/Loops);
  Windows font names aliased to Liberation/DejaVu (`alias_windows_fonts`), fixes spaced Hebrew;
  remote stylesheets deferred until cached (`defer_stylesheets`); remote fetches never block first paint,
  go to a shared cache and trigger a redraw; shared warmed FontContext.
- Search: SQLite FTS5 over bodies (`src/cache.rs`, `crates/mail-core/src/search.rs`).

## Open work, in the owner's priority order
1. ~~Mass unsubscriber~~ **done** (`src/subscriptions.rs`, apps/megamail). Screen opened from the app bar
   (MailOpen icon), palette "Subscriptions: unsubscribe in bulk", or "Unsubscribe in bulk" on the Newsletters
   tab; Esc closes. Rows: unsubscribable senders first, then emails/month over 90 days, method badge,
   Unsubscribe / Keep / Later (30 days), click to multi-select, "Unsubscribe from N". One-click POSTs only
   for header-declared https routes; email routes send directly via `live.send_message` (subject defaults to
   "unsubscribe"); web routes open at most 3 pages per batch. Decisions persist in `subscriptions.v1.json`.
   Polish left: keyboard nav (j/k/x/u) on the screen, a sender count badge on the Newsletters tab.
2. **Batch composer.** "Send individually" (one message per recipient, `{first_name}`/`{name}` merge),
   recipient groups, paste a list. Send through `live.send_message` per recipient.
3. Keep removing vertical clutter in the reader; everything secondary goes into the top bar or the side panel.
4. Squeeze the codebase (main.rs is ~11k lines): move reader, list, app bar into their own modules.
5. Not built yet: remind-if-no-reply, snooze UI polish, send later UI, read receipts (privacy-safe no).

## Owner's machine facts
- CachyOS (Arch-based) Linux, Hyprland/Noctalia, many fonts installed. Installed binary: `~/.local/bin/megamail`.
- Gmail account `djderpcat@gmail.com` (app password, no OAuth by choice) plus others.

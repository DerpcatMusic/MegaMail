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
  `tame_inline_tags`: `<td style=display:block>` → plain cell (Blitz drops it), small `<img>` gets
  `min-width` (else squeezed to 0 beside a width:100% cell), display:block imgs get their size attrs as CSS.
  Debug renders: `MM_OUT=<dir>` overrides `/tmp`. Learnings live in `.trail/learnings/` (trail replaced graft).
- Search: SQLite FTS5 over bodies (`src/cache.rs`, `crates/mail-core/src/search.rs`).

## Open work, in the owner's priority order
1. ~~Mass unsubscriber~~ **done** (`src/subscriptions.rs`, apps/megamail). Screen opened from the app bar
   (MailOpen icon), palette "Subscriptions: unsubscribe in bulk", or "Unsubscribe in bulk" on the Newsletters
   tab; Esc closes. Rows: unsubscribable senders first, then emails/month over 90 days, method badge,
   Unsubscribe / Keep / Later (30 days), click to multi-select, "Unsubscribe from N". One-click POSTs only
   for header-declared https routes; email routes send directly via `live.send_message` (subject defaults to
   "unsubscribe"); web routes open at most 3 pages per batch. Decisions persist in `subscriptions.v1.json`.
   Polish left: keyboard nav (j/k/x/u) on the screen, a sender count badge on the Newsletters tab.
2. **Batch composer (done, basic).** Composer "Individually" toggle: one private copy per To recipient,
   `{first_name}`/`{name}`/`{email}` merged into subject/body (`batch_send.rs`); a pasted list split by
   newline/semicolon/tab works. Left: recipient groups, Undo for batches, per-copy progress (the composer
   only tracks the last request).
   Also done: list filter rail (Sort/State/Search-in chips + sort direction), reader toolbar hierarchy
   (labeled Reply, icon+All, icon expand/collapse), "Use my image" when a gradient hides the custom wallpaper.
   Icons must be listed in `icon_assets!(MailIcons, …)` in main.rs or they render blank.
3. Keep removing vertical clutter in the reader; everything secondary goes into the top bar or the side panel.
4. Codebase split (done): main.rs 6.7k lines; list_ui, reader_ui, original_ui, setup_ui, composer_ui hold
   the screens as `impl MailApp` blocks. Next candidates: render(), on_live_change, new().
   Composer: Formatted (Markdown) by default with a toolbar and Ctrl+B/I/K; inline borderless To/Subject rows.
   Mail rendering: second handle_messages/resolve pass applies CSS background images; bogus img sizes dropped;
   "Always from this sender" image trust in image-senders.v1.json (DMARC-failing senders like AgroCafe).
   HEIC images (some Instagram thumbnails) still can't decode.
6. gpui-kit 0.10 / gpui-fast: needs re-porting the RTL/bidi patches in vendor/gpui-pre, gpui-base,
   gpui-pre-wgpu and cosmic-text onto gpui-pre 0.3.12 (or gpui-fast). Not started.
5. Done: composer "Later" presets (this evening / tomorrow 8:00 / next week / 3 days) and "Remind" (snoozes
   the replied-to conversation 3 days; any reply brings snoozed, waiting and done threads back). Snooze presets in
   the work-state menu, which now sits in the reader toolbar. Not built: read receipts (privacy-safe no).
   Known: on narrow panes the reader toolbar's collapse icon can be covered by Reply.

## Owner's machine facts
- CachyOS (Arch-based) Linux, Hyprland/Noctalia, many fonts installed. Installed binary: `~/.local/bin/megamail`.
- Gmail account `djderpcat@gmail.com` (app password, no OAuth by choice) plus others.

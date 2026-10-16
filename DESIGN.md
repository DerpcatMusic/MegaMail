---
name: MegaMail
description: Native Linux email workspace carrying Zeron's quiet canvas, compact hierarchy, and theme model into GPUI Kit.
colors:
  canvas-dark: "#060606"
  shell-dark: "#0d0d0d"
  list-dark: "#090909"
  surface-dark: "#0e0e0e"
  border-dark: "#202020"
  text-dark: "#e8e8ea"
  muted-dark: "#a9a9ae"
  faint-dark: "#85858a"
  accent-dark: "#7c86ff"
  accent-wash-dark: "rgb(124 134 255 / 14%)"
  selected-dark: "#2e2e30"
  hover-dark: "#252527"
  on-accent-dark: "#101014"
  canvas-light: "#ffffff"
  shell-light: "#f7f7f9"
  list-light: "#fcfcfd"
  surface-light: "#ffffff"
  border-light: "#e5e5e9"
  text-light: "#303035"
  muted-light: "#62626a"
  faint-light: "#73737a"
  accent-light: "#5b43e8"
  accent-wash-light: "rgb(91 67 232 / 10%)"
  selected-light: "#e8e8eb"
  hover-light: "#efeff2"
  on-accent-light: "#ffffff"
typography:
  headline:
    fontFamily: "Geist, sans-serif"
    fontSize: "24px"
    fontWeight: 600
    lineHeight: 1.2
  title:
    fontFamily: "Geist, sans-serif"
    fontSize: "14px"
    fontWeight: 600
  body:
    fontFamily: "Geist, sans-serif"
    fontSize: "14px"
    fontWeight: 400
    lineHeight: 1.6
  folder:
    fontFamily: "Geist, sans-serif"
    fontSize: "13px"
  row-sender:
    fontFamily: "Geist, sans-serif"
    fontSize: "13px"
  row-subject:
    fontFamily: "Geist, sans-serif"
    fontSize: "12px"
  label:
    fontFamily: "Geist, sans-serif"
    fontSize: "10px"
    fontWeight: 600
  metadata:
    fontFamily: "Geist, sans-serif"
    fontSize: "11px"
rounded:
  sm: "4px"
  md: "6px"
  reader-block: "8px"
  lg: "10px"
  xl: "10px"
  full: "9999px"
spacing:
  xs: "4px"
  sm: "8px"
  md: "12px"
  lg: "16px"
  xl: "20px"
  xxl: "24px"
components:
  folder-selected-dark:
    backgroundColor: "rgb(235 235 235 / 11%)"
    textColor: "{colors.text-dark}"
    rounded: "4px"
    padding: "8px"
    height: "32px"
    typography: "{typography.folder}"
  folder-hover-dark:
    backgroundColor: "rgb(235 235 235 / 11%)"
    textColor: "{colors.text-dark}"
    rounded: "4px"
    padding: "8px"
    height: "32px"
    typography: "{typography.folder}"
  folder-selected-light:
    backgroundColor: "rgb(26 26 26 / 6%)"
    textColor: "{colors.text-light}"
    rounded: "4px"
    padding: "8px"
    height: "32px"
    typography: "{typography.folder}"
  folder-hover-light:
    backgroundColor: "rgb(26 26 26 / 6%)"
    textColor: "{colors.text-light}"
    rounded: "4px"
    padding: "8px"
    height: "32px"
    typography: "{typography.folder}"
  message-row-selected-dark:
    backgroundColor: "{colors.selected-dark}"
    textColor: "{colors.text-dark}"
    rounded: "4px"
    padding: "12px"
    height: "78px"
    typography: "{typography.row-sender}"
  message-row-selected-light:
    backgroundColor: "{colors.selected-light}"
    textColor: "{colors.text-light}"
    rounded: "4px"
    padding: "12px"
    height: "78px"
    typography: "{typography.row-sender}"
  input-search-dark:
    backgroundColor: "{colors.surface-dark}"
    textColor: "{colors.text-dark}"
    rounded: "{rounded.md}"
    padding: "8px 10px"
    height: "32px"
    typography: "{typography.body}"
  input-search-light:
    backgroundColor: "{colors.surface-light}"
    textColor: "{colors.text-light}"
    rounded: "{rounded.md}"
    padding: "8px 10px"
    height: "32px"
    typography: "{typography.body}"
  button-secondary-dark:
    backgroundColor: "{colors.surface-dark}"
    textColor: "{colors.text-dark}"
    rounded: "{rounded.md}"
    size: "small"
  button-secondary-light:
    backgroundColor: "{colors.surface-light}"
    textColor: "{colors.text-light}"
    rounded: "{rounded.md}"
    size: "small"
  message-reader-dark:
    backgroundColor: "{colors.canvas-dark}"
    textColor: "{colors.text-dark}"
    padding: "24px"
    typography: "{typography.body}"
  message-reader-light:
    backgroundColor: "{colors.canvas-light}"
    textColor: "{colors.text-light}"
    padding: "24px"
    typography: "{typography.body}"
---

# Design System: MegaMail

## Overview

**Creative North Star: “Zeron’s quiet, continuous work canvas.”**

MegaMail carries Zeron’s calm surface hierarchy, compact controls, and careful text ladder into a native email workspace. The desktop layout is a 232px account/folder rail, a 332px message list, and an opaque reading pane. Rows are 78px high with 4px corners. Hairlines separate the work panes; individual messages use restrained authored blocks within the reader.

The app uses official GPUI Kit 0.7.1 and the complete standalone Zeron theme model copied from MIT-licensed source commit `0c4835d2b73aa632b7b4d626ee0826a25ec1c9b9`: 19 families and 30 built-in variants. System mode follows the desktop appearance; Light and Dark variant selections are independent. Accent and surface treatments can be overridden. The [preview captures](docs/preview/) show earlier release surfaces in fictional demo mode; comfort-work captures live under `.impeccable/review/`. Source is authoritative where a capture predates a correction.

**Key Characteristics:**
- Opaque work panes with a quiet navigation shell, one message-list plane and distinct authored message blocks.
- Dense but legible email rows, with sender, subject, and snippet in clear order.
- Indigo reserved for focus and identity cues; neutral fills show hover and selection.
- Opaque mail and task surfaces that keep wallpaper out of reading and interaction areas.

## Colors

The YAML palette records MegaMail’s default dark and light roles. Selecting another Zeron family or imported VS Code theme replaces those values; the accent selector and surface setting can override the selected variant. Zeron’s source roles remain documented in the [hierarchy research](docs/research/zeron-hierarchy.md) and the copied [`zeron-theme` crate](crates/zeron-theme/README.md).

| Role | Dark native token | Light native token | Assignment |
| --- | --- | --- | --- |
| Reading canvas | `#060606` | `#ffffff` | Main reading surface |
| Navigation shell | `#0d0d0d` | `#f7f7f9` | Account and folder rail |
| Message-list plane | `#090909` | `#fcfcfd` | List background |
| Raised component surface | `#0e0e0e` | `#ffffff` | Search and secondary controls |
| Border | `#202020` | `#e5e5e9` | Pane separators and field outlines |
| Primary text | `#e8e8ea` | `#303035` | Sender, subject, and message body |
| Muted text | `#a9a9ae` | `#62626a` | Snippets and secondary labels |
| Faint metadata | `#85858a` | `#73737a` | Timestamps and quiet metadata |
| Accent | `#7c86ff` | `#5b43e8` | Focus, unread dot, star, and small identity mark |
| Accent wash | `rgb(124 134 255 / 14%)` | `rgb(91 67 232 / 10%)` | Small tinted notices and avatar fill |
| Selected fill | `#2e2e30` | `#e8e8eb` | Selected message row and selected controls |
| Hover fill | `#252527` | `#efeff2` | Hovered controls and message rows; folder rows use Zeron's shared neutral wash |
| Text on accent | `#101014` | `#ffffff` | Inverse text when a filled accent action is used |

Zeron uses neutral lightness roles, translucent hairlines, and restrained hover/selection washes. MegaMail resolves these roles from the selected theme and forces the navigation, list, reader and task pane backgrounds opaque. Correspondent colors are derived from full addresses and constrained against the active theme’s reader, control, selected, hover and avatar surfaces for small-text legibility; the full address and own-message label carry authorship independently of color.

**The Role Remap Rule.** Keep semantic roles stable across modes and use each mode’s own value. Do not mechanically invert the dark palette.

**The Quiet Accent Rule.** Keep the selected accent to focus, unread, star and own-message identity cues. A quiet accent wash distinguishes authored outgoing blocks; neutral fills carry large selected and hovered areas.

## Typography

**Display Font:** Geist, with the platform sans-serif fallback.

**Body Font:** Geist, with the platform sans-serif fallback.

**Label/Mono Font:** Geist for labels; reserve a monospace face for actual code or fixed-width data.

The native app registers the bundled Geist regular, medium, and semibold faces before opening its window ([font loading in `main.rs`](apps/megamail/src/main.rs#L5786)). This follows Zeron’s utilitarian Geist pairing while keeping email copy in the same readable sans face.

### Hierarchy

- **Headline** (600, 24px, 1.2): message subject in the reader.
- **Title** (600, 14px): pane heading.
- **Body** (400, 14px, 1.6): message paragraphs.
- **Folder and sender** (13px): navigation labels and row sender; unread senders use semibold weight.
- **Row subject** (12px): medium when unread and regular when read.
- **Label** (600, 10px): compact uppercase “RECENT” and message-count labels.
- **Metadata** (10–11px): timestamps, addresses, and secondary profile information; preserve the role color and avoid further reducing size.

Zeron’s source hierarchy and its contrast targets are captured in the [design-system research](docs/research/zeron.md). Apply the actual MegaMail token values above and check small text against its rendered surface when changing a theme. Email content does not switch to Geist Mono.

## Layout

At the 1280×800 reference size, the open sidebar is 232px and the message list is 332px; the reader takes the remaining width. Unified Inbox is the default landing and combines independently paged Inbox headers from configured accounts. Each account keeps its own page state and can show a partial-failure warning without hiding successful results. Account and folder browsing remains available in the same rail. Between 1280 and 1000 logical pixels, the rail and list compact smoothly from 232px/332px to 192px/280px to leave the reader usable without a width jump. Reader actions wrap as needed; subject, metadata, and message body share one leading edge and scroll together. These are native desktop dimensions, not responsive web breakpoints.

Use the observed 4/8/12/16/20/24px spacing steps for the app’s grouping. Tighten icon-and-label pairs, then give separate controls and content groups more room. Keep the window controls, search field, folder actions, focus, and keyboard message navigation native to GPUI Kit.

## Elevation & Depth

The app conveys depth with opaque theme-derived planes and 1px separators. The full-window wallpaper remains behind the workspace without bleeding into navigation, message-list, reader or task panes. Authored message blocks use a neutral control surface for correspondents and a quiet accent wash for own mail. Wallpaper blur and treatments are preprocessed into cached images; image changes crossfade over 240ms. The [materials research](docs/research/zeron-materials.md) records the earlier material exploration and the distinction from Zeron’s renderer implementation.

**The Opaque Work Pane Rule.** Keep navigation, message-list, reader and task pane backgrounds fully opaque in every theme and surface preference. Wallpaper must not compete with mail text or controls.

**The Static Material Rule.** Apply wallpaper blur and treatments before caching. MegaMail does not implement Zeron’s renderer-specific backdrop blur or per-primitive edge fades.

## Shapes

Message and navigation rows use a restrained 4px corner. GPUI Kit’s shared component radii remain 6px and 10px where its controls and larger surfaces use them. Avatars and unread dots are circular. Pane edges stay square and use hairlines. Authored reader blocks use an 8px corner and 16px internal padding to separate messages without changing the pane silhouette.

## Components

### Folder navigation

A quiet, compact row with a clear active state. Use a 4px radius and a soft theme-derived neutral wash for selected or hovered state. Keep the folder icon, name, and nonzero count aligned, and preserve the accessible folder name.

### Message row

A compact text hierarchy rather than a card stack. Regular rows are 78px high and Compact rows are 64px, both with 4px corners. Put sender and time first, subject and optional star second, and a one-line snippet last. Use a small accent dot plus semibold sender for unread mail; selected and hover fills remain distinct. With conversation grouping enabled, show reply counts and expandable child messages. Group only by message-reference headers across folders and Sent, never by subject alone. Related-message lookups inspect at most 128 candidate headers in 8 folders and walk up to 24 reference ancestors, so not every historical or future reply is guaranteed to appear. Partial results show a warning. Keep timestamp and status at the trailing edge.

### Search field

Direct accounts search the local SQLite cache within the selected folder or All local mail; `body:` covers downloaded, unencrypted bodies only. All local mail excludes Trash, Junk and Templates. Legacy adapters search loaded headers. Filters intersect with the bounded loaded results; no search claims provider-global coverage. Date, sender email, subject and unread sorting run before paging for direct-account folder caches, and over loaded headers for legacy adapters. Keep the scope hint, partial-result warnings and focus indicator visible.

### Archive and restore action

Use compact secondary actions with the Kit icon, short label, tooltip, and accessible action name. Archive/restore, read/unread, star, and trash target one message through its account and folder worker. Keep the selected message identity scoped so equal numeric UIDs in different folders or accounts cannot collide.

### Reading pane

Separate each message into a quiet authored block with an 8px corner and 16px padding. Use the neutral surface for correspondents and the accent wash for own messages. Keep the full sender address visible; “You · draft” and “You · sent” derive from the folder role. Correspondent avatars and names share a stable address-derived color, constrained against the active theme surfaces. Color supplements the explicit identity.

Conversations remain chronological, with controls to expand all or collapse history. The selected, newest and latest sent messages can open independently. Show warnings when the bounded related-message lookup is incomplete. The native semantic view remains selectable and sanitized; the isolated ephemeral WebKit helper preserves authored HTML typography, colors and table layout while blocking remote resources. Plain-text and selectable-text views remain available. Keep recognized quote history restorable and metadata details on demand. Reply All excludes own addresses and aliases; attachments keep explicit open, download and save actions.

### Composition and work state

Retain the composer while browsing accounts and folders, with an explicit Resume draft action. Plain text is the default; opt-in Markdown has a formatted preview, and recovered HTML is preserved. Keep the editor’s format and sender identity explicit. Work-state controls distinguish suggestions from device-saved Needs reply, Waiting, Done and one-day Snoozed states. These states do not change provider flags; newer inbound mail reopens Waiting/Done. Demo work-state changes are session-only.

Provisional outgoing blocks show the actual pending, queued, accepted, uncertain or refused state. Exact account and Message-ID reconciliation retires the session-local block. Durable Outbox and draft recovery carry restart authority; accepted never claims delivery, and uncertainty requires review before retry.

### Appearance

The Appearance view offers System, Light, and Dark modes; Light and Dark variants are chosen independently from the 19 built-in families or imported themes. Accent and surface choices can override the variant defaults. Users can import VS Code theme JSON/JSONC or an extension package manifest, remove an imported family, and reset preferences without deleting the theme library. This import path normalizes those formats into Zeron’s theme model; it does not import native Zeron theme JSON.

Aurora, Midnight, Paper, and custom wallpapers all support Original, Dither, ASCII, Halftone, and Scanlines. Treatment strength, wallpaper opacity, and bottom fade each range from 0–100%; blur choices are 0, 10, or 16. Image decode, blur, and treatment run away from the UI/render path and produce a cached static image. Background changes crossfade over 240ms. Custom images accept PNG, JPEG, and WebP up to 24 MiB and 16 million decoded pixels; decoding is capped at 96 MiB, the stored normalized PNG at 32 MiB, and the longest edge at 2500px. The original file path is not retained. Theme preferences use `theme.v1`; appearance and wallpaper preferences use `appearance.v1`, with private image data under MegaMail’s configuration directory. These are in-window materials, not Zeron’s renderer-level backdrop blur or per-primitive edge fade.

Settings manages accounts and mail preferences, and displays MegaMail’s active keyboard shortcuts. Regular/Compact density, conversation grouping, default Unified Inbox, quote hiding, and reduced motion are user preferences. The screenshot fixtures are fictional demo data; they are not live provider sessions.

## Do's and Don'ts

### Do:
- **Do** use the native palette values in the YAML as the source for text and tint roles; label Zeron source values separately.
- **Do** keep the outer reading pane quiet and use authored blocks to distinguish individual messages.
- **Do** distinguish hover from selection, and pair unread weight with its dot cue.
- **Do** keep mail and task pane backgrounds fully opaque across appearance settings.
- **Do** check small sender text against the actual theme and its selected, hover and avatar surfaces.
- **Do** keep keyboard navigation, Kit focus treatment, accessible action labels, and Linux caption behavior intact.
- **Do** distinguish live account state and operation results from sample-data visual fixtures.
- **Do** keep motion tied to changing state and honor reduced-motion settings if transitions are added.
- **Do** preserve Zeron source attribution and the license for every copied implementation or asset.

### Don't:
- **Don't** wrap every pane or message row in a card.
- **Don't** use saturated accent as a large surface or rely on color alone for unread state or authorship.
- **Don't** expose wallpaper through mail or task panes or imply a per-pixel contrast guarantee.
- **Don't** imply web/mobile responsiveness or measured performance from this native desktop app.
- **Don't** add idle decorative animation or copy Zeron icons without their license and attribution.

Incoming HTML mail uses an isolated ephemeral WebKit helper inside the native reader; authored styling is confined to that rendering path. Native semantic formatting remains the fallback and selectable text view. Message headers sit within authored blocks, with recipients on wrapping selectable lines. Find, view switching, exports and rule previews live in the message menu. Selection and mail rules live in the mailbox menu; show their controls only while the corresponding task is open.

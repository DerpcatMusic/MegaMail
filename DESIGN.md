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
    backgroundColor: "rgb(6 6 6 / 95%)"
    textColor: "{colors.text-dark}"
    padding: "24px"
    typography: "{typography.body}"
  message-reader-light:
    backgroundColor: "rgb(255 255 255 / 95%)"
    textColor: "{colors.text-light}"
    padding: "24px"
    typography: "{typography.body}"
---

# Design System: MegaMail

## Overview

**Creative North Star: “Zeron’s quiet, continuous work canvas.”**

MegaMail carries Zeron’s calm surface hierarchy, compact controls, and careful text ladder into a native email workspace. The desktop layout is a 232px account/folder rail, a 332px message list, and a continuous reading pane. Rows are 78px high with 4px corners. The panes read as one work surface separated by hairlines, not as a stack of cards.

The app uses official GPUI Kit 0.7.1 and the complete standalone Zeron theme model copied from MIT-licensed source commit `0c4835d2b73aa632b7b4d626ee0826a25ec1c9b9`: 19 families and 30 built-in variants. System mode follows the desktop appearance; Light and Dark variant selections are independent. Accent and surface treatments can be overridden. The [dark](docs/preview/dark.png), [light](docs/preview/light.png), [search](docs/preview/search.png), and [appearance](docs/preview/appearance.png) captures show the current release in fictional demo mode.

**Key Characteristics:**
- A continuous reading canvas with a quiet navigation shell and one message-list plane.
- Dense but legible email rows, with sender, subject, and snippet in clear order.
- Indigo reserved for focus and identity cues; neutral fills show hover and selection.
- Wallpaper-backed tinted panes with a contrast-aware cap on background bleed.

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

Zeron uses neutral lightness roles, translucent hairlines, and restrained hover/selection washes. MegaMail resolves roles from the selected theme and caps wallpaper bleed at 24% in the rail, 10% in the list, and 5% in the reader. The sampled theme-contrast guard can reduce it further.

**The Role Remap Rule.** Keep semantic roles stable across modes and use each mode’s own value. Do not mechanically invert the dark palette.

**The Quiet Accent Rule.** Keep indigo to focus, unread, star, and identity cues. Neutral fills carry large selected and hovered areas.

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

At the 1280×800 reference size, the open sidebar is 232px and the message list is 332px; the reader takes the remaining width. Unified Inbox is the default landing and combines independently paged Inbox headers from configured accounts. Each account keeps its own page state and can show a partial-failure warning without hiding successful results. Account and folder browsing remains available in the same rail. These are native desktop dimensions, not responsive web breakpoints.

Use the observed 4/8/12/16/20/24px spacing steps for the app’s grouping. Tighten icon-and-label pairs, then give separate controls and content groups more room. Keep the window controls, search field, folder actions, focus, and keyboard message navigation native to GPUI Kit.

## Elevation & Depth

The app conveys depth with a full-window background, theme-derived pane tints, and 1px separators. Wallpaper bleed is capped at 24% in the rail, 10% in the list, and 5% in the reader, then bounded by a sampled contrast estimate for the selected theme. This is a sampled safeguard, not a per-pixel contrast guarantee. Wallpaper blur and treatments are preprocessed into cached images; image changes crossfade over 240ms. The [materials research](docs/research/zeron-materials.md) explains the distinction from Zeron’s renderer implementation.

**The Tinted Pane Rule.** Keep pane tint roles and the 24/10/5% bleed caps explicit; allow the theme-based contrast estimate to reduce wallpaper visibility further.

**The Static Material Rule.** Apply wallpaper blur and treatments before caching. MegaMail does not implement Zeron’s renderer-specific backdrop blur or per-primitive edge fades.

## Shapes

Message and navigation rows use a restrained 4px corner. GPUI Kit’s shared component radii remain 6px and 10px where its controls and larger surfaces use them. Avatars and unread dots are circular. Pane edges stay square and use hairlines rather than rounded cards.

## Components

### Folder navigation

A quiet, compact row with a clear active state. Use a 4px radius and a soft theme-derived neutral wash for selected or hovered state. Keep the folder icon, name, and nonzero count aligned, and preserve the accessible folder name.

### Message row

A compact text hierarchy rather than a card stack. Regular rows are 78px high and Compact rows are 64px, both with 4px corners. Put sender and time first, subject and optional star second, and a one-line snippet last. Use a small accent dot plus semibold sender for unread mail; selected and hover fills remain distinct. With conversation grouping enabled, show reply counts and expandable child messages. Group only by message-reference headers across folders and Sent, never by subject alone. Related-message lookups inspect at most 128 candidate headers in 8 folders and walk up to 24 reference ancestors, so not every historical or future reply is guaranteed to appear. Partial results show a warning. Keep timestamp and status at the trailing edge.

### Search field

Search is scoped to loaded headers in the active account/folder or Unified Inbox. The All, Unread, Starred, and Attachments filters intersect with the search text over that loaded set. Search does not cover all server history or message bodies. Keep the focus indicator visible in every theme.

### Archive and restore action

Use compact secondary actions with the Kit icon, short label, tooltip, and accessible action name. Archive/restore, read/unread, star, and trash target one message through its account and folder worker. Keep the selected message identity scoped so equal numeric UIDs in different folders or accounts cannot collide.

### Reading pane

Keep the body on the continuous reading plane. A selected conversation opens in chronological order, and the list lets users expand or collapse reply rows. Display selectable escaped plain text; do not render raw mail HTML or load remote content. When quote hiding is on, collapse only a conservative trailing quote block and keep the original text available to restore. Reply All excludes the user’s own addresses and aliases. Extracted validated links and attachments stay in separate panels with explicit open, download, or save actions.

### Appearance

The Appearance view offers System, Light, and Dark modes; Light and Dark variants are chosen independently from the 19 built-in families or imported themes. Accent and surface choices can override the variant defaults. Users can import VS Code theme JSON/JSONC or an extension package manifest, remove an imported family, and reset preferences without deleting the theme library. This import path normalizes those formats into Zeron’s theme model; it does not import native Zeron theme JSON.

Aurora, Midnight, Paper, and custom wallpapers all support Original, Dither, ASCII, Halftone, and Scanlines. Treatment strength, wallpaper opacity, and bottom fade each range from 0–100%; blur choices are 0, 10, or 16. Image decode, blur, and treatment run away from the UI/render path and produce a cached static image. Background changes crossfade over 240ms. Custom images accept PNG, JPEG, and WebP up to 24 MiB and 16 million decoded pixels; decoding is capped at 96 MiB, the stored normalized PNG at 32 MiB, and the longest edge at 2500px. The original file path is not retained. Theme preferences use `theme.v1`; appearance and wallpaper preferences use `appearance.v1`, with private image data under MegaMail’s configuration directory. These are in-window materials, not Zeron’s renderer-level backdrop blur or per-primitive edge fade.

Settings manages accounts and mail preferences, and displays MegaMail’s active keyboard shortcuts. Regular/Compact density, conversation grouping, default Unified Inbox, quote hiding, and reduced motion are user preferences. The screenshot fixtures are fictional demo data; they are not live provider sessions.

## Do's and Don'ts

### Do:
- **Do** use the native palette values in the YAML as the source for text and tint roles; label Zeron source values separately.
- **Do** keep the reading pane continuous and reserve raised surfaces for controls and overlays.
- **Do** distinguish hover from selection, and pair unread weight with its dot cue.
- **Do** preserve the rail/list/reader wallpaper bleed caps of 24/10/5%.
- **Do** let the sampled custom-image contrast estimate reduce wallpaper opacity beneath text.
- **Do** keep keyboard navigation, Kit focus treatment, accessible action labels, and Linux caption behavior intact.
- **Do** distinguish live account state and operation results from sample-data visual fixtures.
- **Do** keep motion tied to changing state and honor reduced-motion settings if transitions are added.
- **Do** preserve Zeron source attribution and the license for every copied implementation or asset.

### Don't:
- **Don't** wrap every pane or message row in a card.
- **Don't** use accent as a large surface or rely on color alone for unread state.
- **Don't** exceed the pane bleed caps or imply a per-pixel contrast guarantee.
- **Don't** imply web/mobile responsiveness or measured performance from this native desktop app.
- **Don't** add idle decorative animation or copy Zeron icons without their license and attribution.

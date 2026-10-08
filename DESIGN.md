---
name: MegaMail
description: Linux-first native email workspace adapted from Zeron's quiet canvas and compact hierarchy.
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
    fontSize: "22px"
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
    lineHeight: 1.5
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
  sm: "6px"
  md: "8px"
  lg: "10px"
  xl: "16px"
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
    backgroundColor: "{colors.selected-dark}"
    textColor: "{colors.text-dark}"
    rounded: "{rounded.md}"
    padding: "8px"
    height: "36px"
    typography: "{typography.folder}"
  folder-hover-dark:
    backgroundColor: "{colors.hover-dark}"
    textColor: "{colors.text-dark}"
    rounded: "{rounded.md}"
    padding: "8px"
    height: "36px"
    typography: "{typography.folder}"
  folder-selected-light:
    backgroundColor: "{colors.selected-light}"
    textColor: "{colors.text-light}"
    rounded: "{rounded.md}"
    padding: "8px"
    height: "36px"
    typography: "{typography.folder}"
  folder-hover-light:
    backgroundColor: "{colors.hover-light}"
    textColor: "{colors.text-light}"
    rounded: "{rounded.md}"
    padding: "8px"
    height: "36px"
    typography: "{typography.folder}"
  message-row-selected-dark:
    backgroundColor: "{colors.selected-dark}"
    textColor: "{colors.text-dark}"
    rounded: "{rounded.lg}"
    padding: "12px"
    height: "86px"
    typography: "{typography.row-sender}"
  message-row-selected-light:
    backgroundColor: "{colors.selected-light}"
    textColor: "{colors.text-light}"
    rounded: "{rounded.lg}"
    padding: "12px"
    height: "86px"
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
    width: "680px"
    typography: "{typography.body}"
  message-reader-light:
    backgroundColor: "{colors.canvas-light}"
    textColor: "{colors.text-light}"
    padding: "24px"
    width: "680px"
    typography: "{typography.body}"
---

# Design System: MegaMail

## Overview

**Creative North Star: “Zeron’s quiet, continuous work canvas.”**

MegaMail carries Zeron’s calm surface hierarchy, compact controls, and careful text ladder into a native email workspace. Its desktop layout maps Zeron’s navigation-plus-work-area logic to an account and folder rail, a message list, and a continuous reading pane. The native Rust palette is the rendered source of truth; Zeron’s upstream values are cited as references wherever the implementation adapts them.

The product is Linux-first and built with official GPUI Kit. Dark and light modes remap the same semantic roles. The visible Linux surfaces stay opaque so text remains independent of wallpaper and compositor effects. This system describes a native desktop window and the preview states shown in the [dark](docs/preview/dark.png), [light](docs/preview/light.png), and [focused search](docs/preview/search.png) captures.

**Key Characteristics:**
- A continuous reading canvas with a quiet navigation shell and one message-list plane.
- Dense but legible email rows, with sender, subject, and snippet in clear order.
- Indigo reserved for focus and identity cues; neutral fills show hover and selection.
- Opaque Linux surfaces and role-based dark/light palettes.

## Colors

The YAML palette transcribes `Palette::new` in [`main.rs`](apps/megamail/src/main.rs#L55): those values are the actual native preview tokens. The dark canvas (`#060606`), shell (`#0d0d0d`), and indigo (`#7c86ff`) match Zeron’s corresponding source roles. The dark list plane, solid border, neutral text values, selection and hover fills, and the complete light palette are MegaMail adaptations. Zeron’s source neutral lightness and alpha-wash roles remain documented in the [hierarchy research](docs/research/zeron-hierarchy.md); they do not override this table.

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
| Selected fill | `#2e2e30` | `#e8e8eb` | Selected folder and message row |
| Hover fill | `#252527` | `#efeff2` | Hovered controls and rows |
| Text on accent | `#101014` | `#ffffff` | Inverse text when a filled accent action is used |

The source Zeron hierarchy uses neutral lightness roles, translucent white/black hairlines, and translucent hover/selection washes. MegaMail’s Rust palette adapts those into explicit opaque role colors for reliable native rendering. Its light accent is `#5b43e8`; the source’s dark accent remains `#7c86ff`.

**The Role Remap Rule.** Keep semantic roles stable across modes and use each mode’s own value. Do not mechanically invert the dark palette.

**The Quiet Accent Rule.** Keep indigo to focus, unread, star, and identity cues. Neutral fills carry large selected and hovered areas.

## Typography

**Display Font:** Geist, with the platform sans-serif fallback.

**Body Font:** Geist, with the platform sans-serif fallback.

**Label/Mono Font:** Geist for labels; reserve a monospace face for actual code or fixed-width data.

The preview registers the bundled Geist regular, medium, and semibold faces before opening its window ([font loading in `main.rs`](apps/megamail/src/main.rs#L928)). This follows Zeron’s utilitarian Geist pairing while keeping email copy in the same readable sans face.

### Hierarchy

- **Headline** (600, 22px, 1.2): message subject in the reader.
- **Title** (600, 14px): pane heading.
- **Body** (400, 14px, 1.5): message paragraphs.
- **Folder and sender** (13px): navigation labels and row sender; unread senders use semibold weight.
- **Row subject** (12px): medium when unread and regular when read.
- **Label** (600, 10px): compact uppercase “RECENT” and message-count labels.
- **Metadata** (10–11px): timestamps, addresses, and secondary profile information; preserve the role color and avoid further reducing size.

Zeron’s source hierarchy and its contrast targets are captured in the [design-system research](docs/research/zeron.md). Apply the actual MegaMail token values above and check small text against its rendered surface when changing a theme. Email content does not switch to Geist Mono.

## Layout

The preview opens at 1280×800 and declares a 1060×640 minimum. Its desktop composition is a 38px titlebar followed by a 232px account/folder rail, a 360px message list, and a reading pane that takes the remaining width. The 232px and 360px widths are MegaMail preview choices; Zeron’s source sidebar is 224–400px with a 256px default. No web or mobile breakpoints are defined for this native window.

Both the message-list header and reader toolbar are 44px high. Message rows are 86px high, inset from the list edge, and virtualized through GPUI Kit’s uniform list. The reader body is capped at 680px and padded by 24px so long lines do not fill a wide monitor. Keep the three panes readable at the declared minimum window size; any future pane collapse or narrower-window behavior needs a native desktop design decision.

Use the observed 4/8/12/16/20/24px spacing steps for the preview’s grouping. Tighten icon-and-label pairs, then give separate controls and content groups more room. Keep the window controls, search field, folder actions, focus, and keyboard message navigation native to GPUI Kit.

## Elevation & Depth

The current app conveys depth with tonal planes and 1px separators. It has no app-authored shadows or backdrop blur. In dark mode, the canvas is `#060606`, the sidebar `#0d0d0d`, the list `#090909`, and controls `#0e0e0e`. Zeron’s source also defines dialog/menu planes at `#101010` and `#161616`, plus translucent border roles; those are upstream references for future surfaces, not current MegaMail tokens. The [materials research](docs/research/zeron-materials.md) explains Zeron’s platform-dependent window tint and custom GPUI blur. On Linux, keep all visible content opaque; reserve native window transparency for client-decoration corner cutouts where the window system requires it.

**The Opaque Linux Rule.** Keep text-bearing window surfaces independent of wallpaper and compositor blur. Add a raised surface only when an interaction needs separation.

## Shapes

MegaMail uses an 8px radius for folder controls, search, and secondary buttons; 10px for selected message rows; 6px for small notice chips; and 16px for the large theme radius. Avatars, unread dots, and the titlebar status mark are circular. Pane edges stay square and use hairlines instead of rounded cards. Zeron’s reference uses 6px small controls, 10px panels, and 16px larger action surfaces; MegaMail retains the 10/16px steps and adapts the small control radius to the 8px Kit theme radius.

## Components

### Folder navigation

A quiet row with a clear active state. Use a 36px-high GPUI Kit ghost button, 8px radius and 8px horizontal inset. The selected row uses the mode’s selected fill; hover uses a distinct hover fill. Keep the folder icon, name, and nonzero count aligned, and preserve the accessible folder name.

### Message row

A compact text hierarchy rather than a card stack. Rows are 86px high with 10px corners and 12px inner padding. Put sender and time first, subject and optional star second, and a one-line snippet last. Use a small accent dot plus semibold sender for unread mail; selected and hover fills remain distinct. Keep timestamp and status at the trailing edge.

### Search field

Use the GPUI Kit input with its search prefix and clear action. The preview field is about 32px high, rounded 8px, and uses a one-pixel role border. Keep the Kit focus ring visible in both themes; the active search capture shows the light-mode indigo focus state.

### Archive and restore action

Use a compact secondary button with the Kit icon, short label, tooltip, and accessible action name. Its fill comes from the raised surface role, with a separate hover state. In the sample preview, these actions affect local sample messages only.

### Reading pane

Keep the body on the continuous canvas. Use a 22px semibold subject, a 13px sender, 11px address/time metadata, a single hairline, and 14px body text at 1.5 line height. Center the content column and cap it at 680px. The current renderer displays plain-text sample paragraphs; the design does not imply incoming HTML-mail behavior.

The preview uses GPUI Kit’s existing icon assets. Zeron’s icon catalog has separate attribution and license terms; copy individual assets only with their applicable notices. The [native preview captures](docs/preview/) show the implemented components and both palette modes.

## Do's and Don'ts

### Do:
- **Do** use the native palette in the YAML as the implementation source of truth; label Zeron source values separately when documenting them.
- **Do** keep the reading pane continuous and reserve raised surfaces for controls and overlays.
- **Do** distinguish hover from selection, and pair unread weight with its dot cue.
- **Do** keep keyboard navigation, Kit focus treatment, accessible action labels, and Linux caption behavior intact.
- **Do** keep sample mailbox state explicit; the preview has no connected account, send, or network action.
- **Do** keep motion tied to changing state and honor reduced-motion settings if transitions are added.
- **Do** preserve Zeron source attribution and the license for every copied implementation or asset.

### Don't:
- **Don't** wrap every pane or message row in a card.
- **Don't** use accent as a large surface or rely on color alone for unread state.
- **Don't** make Linux text contrast depend on wallpaper, blur, or desktop transparency.
- **Don't** imply web/mobile responsiveness or measured performance from this native preview.
- **Don't** add idle decorative animation or copy Zeron icons without their license and attribution.

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
    backgroundColor: "rgb(235 235 235 / 11%)"
    textColor: "{colors.text-dark}"
    rounded: "{rounded.md}"
    padding: "8px"
    height: "32px"
    typography: "{typography.folder}"
  folder-hover-dark:
    backgroundColor: "rgb(235 235 235 / 11%)"
    textColor: "{colors.text-dark}"
    rounded: "{rounded.md}"
    padding: "8px"
    height: "32px"
    typography: "{typography.folder}"
  folder-selected-light:
    backgroundColor: "rgb(26 26 26 / 6%)"
    textColor: "{colors.text-light}"
    rounded: "{rounded.md}"
    padding: "8px"
    height: "32px"
    typography: "{typography.folder}"
  folder-hover-light:
    backgroundColor: "rgb(26 26 26 / 6%)"
    textColor: "{colors.text-light}"
    rounded: "{rounded.md}"
    padding: "8px"
    height: "32px"
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

MegaMail carries Zeron’s calm surface hierarchy, compact controls, and careful text ladder into a native email workspace. Its desktop layout maps Zeron’s navigation-plus-work-area logic to an account and folder rail, a message list, and a continuous reading pane. The native Rust palette is the source for each pane's base tint; Zeron’s upstream values are cited separately wherever the implementation adapts them.

The product is Linux-first and built with official GPUI Kit. Dark and light modes remap the same semantic roles. Optional backgrounds sit below translucent pane tints: the navigation rail allows up to 20% wallpaper bleed, while the message list and reading pane allow up to 5% each. For custom images, a sampled contrast estimate can reduce artwork opacity further. Wallpaper blur and treatments are baked into a cached still image, not supplied by the Linux compositor; the processing path is in [`appearance.rs`](apps/megamail/src/appearance.rs#L27) and [`zeron_wallpaper.rs`](apps/megamail/src/zeron_wallpaper.rs#L35). This system describes a native desktop window. The [dark](docs/preview/dark.png), [light](docs/preview/light.png), and [focused search](docs/preview/search.png) images remain sample-data visual fixtures until replaced with current native captures.

**Key Characteristics:**
- A continuous reading canvas with a quiet navigation shell and one message-list plane.
- Dense but legible email rows, with sender, subject, and snippet in clear order.
- Indigo reserved for focus and identity cues; neutral fills show hover and selection.
- Wallpaper-backed tinted panes with a contrast-aware cap on background bleed.

## Colors

The YAML palette transcribes `Palette::new` in [`main.rs`](apps/megamail/src/main.rs#L77): those values are the actual native Rust palette tokens used as surface tints and text/control roles. The dark canvas (`#060606`), shell (`#0d0d0d`), and indigo (`#7c86ff`) match Zeron’s corresponding source roles. The dark list tint, border, neutral text values, selection and hover fills, and complete light palette are MegaMail adaptations. The final rail/list/reader colors are these values composited with the background layer at the documented pane opacity. Zeron’s source neutral lightness and alpha-wash roles remain documented in the [hierarchy research](docs/research/zeron-hierarchy.md); they do not override this table.

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

The source Zeron hierarchy uses neutral lightness roles, translucent white/black hairlines, and translucent hover/selection washes. MegaMail keeps its native palette values for base colors, then adapts Zeron’s neutral interaction washes in [`zeron_style.rs`](apps/megamail/src/zeron_style.rs#L82); background-backed pane surfaces use separate tint alpha in [`zeron_background.rs`](apps/megamail/src/zeron_background.rs#L81). Its light accent is `#5b43e8`; the source’s dark accent remains `#7c86ff`.

**The Role Remap Rule.** Keep semantic roles stable across modes and use each mode’s own value. Do not mechanically invert the dark palette.

**The Quiet Accent Rule.** Keep indigo to focus, unread, star, and identity cues. Neutral fills carry large selected and hovered areas.

## Typography

**Display Font:** Geist, with the platform sans-serif fallback.

**Body Font:** Geist, with the platform sans-serif fallback.

**Label/Mono Font:** Geist for labels; reserve a monospace face for actual code or fixed-width data.

The native app registers the bundled Geist regular, medium, and semibold faces before opening its window ([font loading in `main.rs`](apps/megamail/src/main.rs#L5786)). This follows Zeron’s utilitarian Geist pairing while keeping email copy in the same readable sans face.

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

The native app opens at 1280×800 and declares a 1060×640 minimum ([window setup](apps/megamail/src/main.rs#L5794)). Its desktop composition uses a 38px titlebar or fallback toolbar ([chrome](apps/megamail/src/main.rs#L4801)), a 256px account/folder rail that can collapse to 60px ([rail](apps/megamail/src/main.rs#L2096), [collapsed rail](apps/megamail/src/main.rs#L2286)), a 360px message list ([list](apps/megamail/src/main.rs#L2465)), and a reading pane that takes the remaining width. Message rows are 86px high ([row](apps/megamail/src/main.rs#L2425)); the reader body is capped at 680px ([reader](apps/megamail/src/main.rs#L3105)). The open rail uses Zeron-derived navigation dimensions; no web or mobile breakpoints are defined for this native window.

Both the message-list header and reader toolbar are 44px high. Message rows are 86px high, inset from the list edge, and virtualized through GPUI Kit’s uniform list. The reader body is capped at 680px and padded by 24px so long lines do not fill a wide monitor. The expandable sidebar groups accounts, pinned folders, and additional folders, with mailbox status and Appearance controls in its footer. Keep the three panes readable at the declared minimum window size; any future pane collapse or narrower-window behavior needs a native desktop design decision.

Use the observed 4/8/12/16/20/24px spacing steps for the app’s grouping. Tighten icon-and-label pairs, then give separate controls and content groups more room. Keep the window controls, search field, folder actions, focus, and keyboard message navigation native to GPUI Kit.

## Elevation & Depth

The app conveys depth with its full-window background layer, translucent pane tints, and 1px separators. Dark-mode base values are `#060606` for the reading plane, `#0d0d0d` for the sidebar, `#090909` for the message list, and `#0e0e0e` for raised controls. The rail permits up to 20% background bleed; the list and reader permit up to 5%. Custom wallpaper opacity is further limited using the sampled 5th/95th-percentile estimate from Zeron’s contrast guard, which targets 4.5:1 for the sampled worst color and does not promise per-pixel contrast. The [materials research](docs/research/zeron-materials.md) explains Zeron’s source window materials. MegaMail composites the wallpaper beneath tinted panes in [`zeron_background.rs`](apps/megamail/src/zeron_background.rs#L40); optional image blur is preprocessed into a static cached image, with no live compositor blur.

**The Tinted Pane Rule.** Keep pane tint roles and their bleed limits explicit; allow the custom-image guard to reduce wallpaper visibility whenever its sampled contrast estimate requires it.

**The Static Material Rule.** Apply wallpaper blur and treatments before caching the image. Do not make Linux compositor blur a runtime dependency.

## Shapes

MegaMail uses an 8px radius for folder controls, search, and secondary buttons; 10px for selected message rows; 6px for small notice chips; and 16px for the large theme radius. Avatars, unread dots, and the titlebar status mark are circular. Pane edges stay square and use hairlines instead of rounded cards. Zeron’s reference uses 6px small controls, 10px panels, and 16px larger action surfaces; MegaMail retains the 10/16px steps and adapts the small control radius to the 8px Kit theme radius.

## Components

### Folder navigation

A quiet row with a clear active state. Use a 32px-high GPUI Kit ghost button, 8px radius and 8px horizontal inset. Selected and hovered rows share Zeron’s soft neutral wash; selection remains distinct in model and accessibility state, while selected folder text and icons use the primary text role. Keep the folder icon, name, and nonzero count aligned, and preserve the accessible folder name.

### Message row

A compact text hierarchy rather than a card stack. Rows are 86px high with 10px corners and 12px inner padding. Put sender and time first, subject and optional star second, and a one-line snippet last. Use a small accent dot plus semibold sender for unread mail; selected and hover fills remain distinct. Keep timestamp and status at the trailing edge.

### Search field

Use the GPUI Kit input with its search prefix and clear action. The field is about 32px high, rounded 8px, and uses a one-pixel role border. Keep the Kit focus ring visible in both themes; the sample-data search fixture shows the light-mode indigo focus state.

### Archive and restore action

Use a compact secondary button with the Kit icon, short label, tooltip, and accessible action name. Its fill comes from the raised surface role, with a separate hover state. In the native client, archive and restore act on the connected mailbox; the older screenshots remain sample-data fixtures.

### Reading pane

Keep the body on the continuous reading plane. Use a 22px semibold subject, a 13px sender, 11px address/time metadata, a single hairline, and 14px body text at 1.5 line height. Center the content column and cap it at 680px. The 95%-opaque reader tint sits above the selected background. The reader displays a safe plain-text body, with extracted links and attachments in separate panels. Opening a validated link, downloading an attachment, and saving a file each require an explicit user action. This does not imply full-fidelity incoming HTML rendering or automatic remote-content loading.

### Appearance

The Appearance view combines the native dark/light palette with Aurora, Midnight, and Paper backgrounds or a custom wallpaper. Custom images support PNG, JPEG, and WebP source files up to 24 MiB and 16 million decoded pixels; decoding is capped at 96 MiB of allocation. MegaMail keeps one normalized PNG copy in its private configuration directory, limits the stored file to 32 MiB, and resizes the longest edge to at most 2500px. It does not persist the original path.

Original, Dither, ASCII, Halftone, and Scanlines are the custom-image treatments. Wallpaper strength is 72%, 86%, or 96%; blur choices are Sharp, Blur 10, and Blur 16. Image decode, blur, and treatment run away from the UI/render path and produce a cached static image; strength controls how that image is painted. Appearance preferences persist in `appearance.v1`; the custom image is stored in the adjacent private `wallpapers/` directory. These are in-window materials, not platform compositor effects. The app stores appearance preferences and image-processing limits in [`appearance.rs`](apps/megamail/src/appearance.rs#L27), pane fills in [`zeron_background.rs`](apps/megamail/src/zeron_background.rs#L81), and the five pixel effects in [`zeron_wallpaper.rs`](apps/megamail/src/zeron_wallpaper.rs#L35).

The native app uses GPUI Kit’s existing icon assets. Zeron’s icon catalog has separate attribution and license terms; copy individual assets only with their applicable notices. The [existing captures](docs/preview/) are sample-data fixtures from the earlier preview; they do not document the wallpaper materials or serve as current native captures.

## Do's and Don'ts

### Do:
- **Do** use the native palette values in the YAML as the source for text and tint roles; label Zeron source values separately.
- **Do** keep the reading pane continuous and reserve raised surfaces for controls and overlays.
- **Do** distinguish hover from selection, and pair unread weight with its dot cue.
- **Do** preserve the rail/list/reader wallpaper bleed caps.
- **Do** let the sampled custom-image contrast estimate reduce wallpaper opacity beneath text.
- **Do** keep keyboard navigation, Kit focus treatment, accessible action labels, and Linux caption behavior intact.
- **Do** distinguish live account state and operation results from sample-data visual fixtures.
- **Do** keep motion tied to changing state and honor reduced-motion settings if transitions are added.
- **Do** preserve Zeron source attribution and the license for every copied implementation or asset.

### Don't:
- **Don't** wrap every pane or message row in a card.
- **Don't** use accent as a large surface or rely on color alone for unread state.
- **Don't** exceed the pane bleed caps or rely on a compositor effect for text contrast.
- **Don't** imply web/mobile responsiveness or measured performance from this native desktop app.
- **Don't** add idle decorative animation or copy Zeron icons without their license and attribution.

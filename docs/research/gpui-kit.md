# GPUI Kit research for MegaMail

## Recommendation

Build against the official umbrella crate, `gpui-kit = "0.7.1"`, with its
default `component` and `assets` features. The official repository at the time
of this review is commit
[`288767cc730ca4977852a7860f52a8465a61876f`](https://github.com/longbridge/gpui-kit/commit/288767cc730ca4977852a7860f52a8465a61876f);
the crate's current published release is 0.7.1 ([docs.rs](https://docs.rs/crate/gpui-kit/latest)).
The package manifest identifies the crate as 0.7.1 and enables styled
components and icon assets by default ([`crates/kit/Cargo.toml:1-25`](https://github.com/longbridge/gpui-kit/blob/288767cc730ca4977852a7860f52a8465a61876f/crates/kit/Cargo.toml#L1-L25)).

Keep the whole UI on the `gpui_kit` re-exports. Do not add another direct GPUI
dependency. Kit pins the matching `gpui-pre`, platform, web, macro, HTTP-client,
and sum-tree snapshots to `=0.3.8`; its optional GPUI Fast family is 0.1.3
([`Cargo.toml:63-81`](https://github.com/longbridge/gpui-kit/blob/288767cc730ca4977852a7860f52a8465a61876f/Cargo.toml#L63-L81)).
Pin `gpui-kit` to 0.7.1 and commit MegaMail's `Cargo.lock` so component and
other compatible package updates do not drift between app builds.
The umbrella re-exports GPUI at its root, `base`, `component`, `assets`, and the
selected `platform` ([`crates/kit/src/lib.rs:100-139`](https://github.com/longbridge/gpui-kit/blob/288767cc730ca4977852a7860f52a8465a61876f/crates/kit/src/lib.rs#L100-L139),
[`crates/kit/src/lib.rs:164-168`](https://github.com/longbridge/gpui-kit/blob/288767cc730ca4977852a7860f52a8465a61876f/crates/kit/src/lib.rs#L164-L168)).

### Minimal consumer recipe

`Cargo.toml`:

```toml
[package]
name = "megamail"
version = "0.1.0"
edition = "2024"

[dependencies]
gpui-kit = "=0.7.1"
```

The exact application startup pattern from the current Kit example is:

```rust
use gpui_kit::*;

struct AppView;

impl Render for AppView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child("MegaMail")
    }
}

fn main() {
    application().with_assets(assets::Assets).run(|cx| {
        init(cx);
        open_window(WindowOptions::default(), cx, |_, cx| cx.new(|_| AppView))
            .expect("Failed to open window");
    });
}
```

Kit's getting-started sample shows the same dependency, initialization, and
window flow ([`website/docs/getting-started.md:18-25`](https://github.com/longbridge/gpui-kit/blob/288767cc730ca4977852a7860f52a8465a61876f/website/docs/getting-started.md#L18-L25),
[`website/docs/getting-started.md:83-134`](https://github.com/longbridge/gpui-kit/blob/288767cc730ca4977852a7860f52a8465a61876f/website/docs/getting-started.md#L83-L134)).
The component initializer must run before windows that use Kit components.
Use `gpui_kit::open_window` so the Base `Root` and its overlay hosts are
installed; return app content from the closure ([`crates/kit/src/lib.rs:170-202`](https://github.com/longbridge/gpui-kit/blob/288767cc730ca4977852a7860f52a8465a61876f/crates/kit/src/lib.rs#L170-L202)).

## Platform support and setup

The framework targets macOS, Windows, and Linux. The current installation guide
specifies macOS 15+ with Xcode Command Line Tools; Windows 10+ with Visual
Studio 2022 C++ workload/MSVC, a Windows SDK, and CMake; and Ubuntu 24.04 with
`gcc`, `g++`, `clang`, `libfontconfig-dev`, `libwayland-dev`,
`libwebkit2gtk-4.1-dev`, `libxkbcommon-x11-dev`, `libx11-xcb-dev`, `libssl-dev`,
`libzstd-dev`, `vulkan-validationlayers`, and `libvulkan1`.
`libasound2-dev` is only needed for the `speech` feature. Other Linux
distributions need equivalents; Linux also needs a graphical X11/Wayland session
and working Vulkan driver. The documented current dependency graph needs Rust
1.92+ ([`website/docs/installation.md:11-49`](https://github.com/longbridge/gpui-kit/blob/288767cc730ca4977852a7860f52a8465a61876f/website/docs/installation.md#L11-L49)).
The checked-in platform dependency turns on `font-kit`, X11, Wayland, and runtime
shaders, so an app using Kit does not need to configure GPUI platform crates
separately ([`Cargo.toml:70-81`](https://github.com/longbridge/gpui-kit/blob/288767cc730ca4977852a7860f52a8465a61876f/Cargo.toml#L70-L81)).

## Mail UI primitives

- **Search, subject, and address fields:** use
  `gpui_kit::component::input::{Input, InputState}`. The component binds a view
  to retained state with `Input::new(&state)` and supports an explicit
  accessibility label; the public exports and recipe are in
  [`crates/component/src/input/mod.rs:25-53`](https://github.com/longbridge/gpui-kit/blob/288767cc730ca4977852a7860f52a8465a61876f/crates/component/src/input/mod.rs#L25-L53),
  [`crates/component/src/input/input.rs:206-215`](https://github.com/longbridge/gpui-kit/blob/288767cc730ca4977852a7860f52a8465a61876f/crates/component/src/input/input.rs#L206-L215),
  and [`website/docs/accessibility.md:94-139`](https://github.com/longbridge/gpui-kit/blob/288767cc730ca4977852a7860f52a8465a61876f/website/docs/accessibility.md#L94-L139).
  `.cleanable(true)` adds a clear button for a non-empty editable single-line
  field ([`input/input.rs:316-320`](https://github.com/longbridge/gpui-kit/blob/288767cc730ca4977852a7860f52a8465a61876f/crates/component/src/input/input.rs#L316-L320)).
- **Plain-text compose:** use `Textarea` and `TextareaState`; they are explicitly
  for ordinary multiline text. `Editor` is the source-code editor and `Input`
  is single-line ([`website/component/textarea.md:6-28`](https://github.com/longbridge/gpui-kit/blob/288767cc730ca4977852a7860f52a8465a61876f/website/component/textarea.md#L6-L28),
  [`website/component/editor.md:6-15`](https://github.com/longbridge/gpui-kit/blob/288767cc730ca4977852a7860f52a8465a61876f/website/component/editor.md#L6-L15)).
  The editor supports syntax highlighting, folding, line numbers, and a virtual
  text buffer when MegaMail later needs an advanced plain-text composer
  ([`website/component/editor.md:86-106`](https://github.com/longbridge/gpui-kit/blob/288767cc730ca4977852a7860f52a8465a61876f/website/component/editor.md#L86-L106)).
- **Inbox and message lists:** for fixed-height rows, GPUI's `uniform_list`
  already renders a requested range; the Kit repo uses it for commit history
  ([`examples/tig/src/app.rs:551-570`](https://github.com/longbridge/gpui-kit/blob/288767cc730ca4977852a7860f52a8465a61876f/examples/tig/src/app.rs#L551-L570)).
  Kit's styled `component::List` is also virtualized and adds delegate-driven
  selection/search, but requires uniform item, header, and footer heights
  ([`crates/component/src/list/delegate.rs:8-67`](https://github.com/longbridge/gpui-kit/blob/288767cc730ca4977852a7860f52a8465a61876f/crates/component/src/list/delegate.rs#L8-L67),
  [`crates/component/src/list/list.rs:67-70`](https://github.com/longbridge/gpui-kit/blob/288767cc730ca4977852a7860f52a8465a61876f/crates/component/src/list/list.rs#L67-L70),
  [`crates/component/src/list/list.rs:552-564`](https://github.com/longbridge/gpui-kit/blob/288767cc730ca4977852a7860f52a8465a61876f/crates/component/src/list/list.rs#L552-L564)).
  For genuinely variable-height content, Base's `v_virtual_list` takes the
  item-size table and renders only the visible range
  ([`crates/base/src/virtual_list.rs:129-149`](https://github.com/longbridge/gpui-kit/blob/288767cc730ca4977852a7860f52a8465a61876f/crates/base/src/virtual_list.rs#L129-L149),
  [`crates/base/src/virtual_list.rs:687-729`](https://github.com/longbridge/gpui-kit/blob/288767cc730ca4977852a7860f52a8465a61876f/crates/base/src/virtual_list.rs#L687-L729)).
  The Kit guide calls the render closure a hot path and recommends retaining the
  size vector rather than rebuilding it during render; use `uniform_list` when
  one size can represent the rows ([`website/base/virtual-list.md:119-145`](https://github.com/longbridge/gpui-kit/blob/288767cc730ca4977852a7860f52a8465a61876f/website/base/virtual-list.md#L119-L145)).
- **Icons and theme:** Kit's default `assets::Assets` includes the icon pack;
  the complete Lucide catalog is `gpui_kit::assets::IconName`, while the
  component-level compatibility enum is intentionally smaller
  ([`crates/component/src/icon.rs:11-38`](https://github.com/longbridge/gpui-kit/blob/288767cc730ca4977852a7860f52a8465a61876f/crates/component/src/icon.rs#L11-L38),
  [`crates/assets/src/lib.rs:1-13`](https://github.com/longbridge/gpui-kit/blob/288767cc730ca4977852a7860f52a8465a61876f/crates/assets/src/lib.rs#L1-L13)).
  For palette changes, use `component::Theme::update(cx, ...)`; it reconciles
  semantic tokens and colors, updates Base, and refreshes all windows. Direct
  `Theme::global_mut` can leave those copies out of sync
  ([`crates/component/src/theme/mod.rs:236-290`](https://github.com/longbridge/gpui-kit/blob/288767cc730ca4977852a7860f52a8465a61876f/crates/component/src/theme/mod.rs#L236-L290)).
- **Custom title bar:** `component::TitleBar::window_options()` configures a
  transparent native title bar and delegates dragging/double-click behavior to
  the component ([`crates/component/src/title_bar.rs:59-91`](https://github.com/longbridge/gpui-kit/blob/288767cc730ca4977852a7860f52a8465a61876f/crates/component/src/title_bar.rs#L59-L91),
  [`crates/component/src/title_bar.rs:322-375`](https://github.com/longbridge/gpui-kit/blob/288767cc730ca4977852a7860f52a8465a61876f/crates/component/src/title_bar.rs#L322-L375)).
  On Linux, window-manager server decorations can override client decorations,
  so Kit omits duplicate window controls in that case
  ([`crates/component/src/title_bar.rs:254-296`](https://github.com/longbridge/gpui-kit/blob/288767cc730ca4977852a7860f52a8465a61876f/crates/component/src/title_bar.rs#L254-L296)).

## HTML mail

Use Kit's native `TextView::html` for the first reader: it handles common
document structure, links, images, tables, lists, and basic inline formatting,
but its documented goal is simple reader-mode HTML and it explicitly does not
support CSS as a general HTML viewer
([`crates/base/src/text/text_view.rs:126-141`](https://github.com/longbridge/gpui-kit/blob/288767cc730ca4977852a7860f52a8465a61876f/crates/base/src/text/text_view.rs#L126-L141),
[`crates/base/src/text/text_view.rs:236-271`](https://github.com/longbridge/gpui-kit/blob/288767cc730ca4977852a7860f52a8465a61876f/crates/base/src/text/text_view.rs#L236-L271)).
Treat it as a candidate for intentionally limited reader-mode rendering;
security and latency need verification against sanitized mail samples. Keep a
plain-text path for layout it cannot reproduce. Sanitize message markup before
rendering and own link/image policy. `TextView`'s default image path fetches
HTTP(S) images and decodes data URLs; `.image_source(...)` lets MegaMail route
those through an app-owned policy ([`website/component/text-view.md:319-347`](https://github.com/longbridge/gpui-kit/blob/288767cc730ca4977852a7860f52a8465a61876f/website/component/text-view.md#L319-L347)).

If browser fidelity is required later, `gpui-webview` is experimental and has
real native-browser behavior. Current docs list WKWebView on macOS, WebView2 on
Windows, WebKitGTK on Linux/X11, and no Linux Wayland support. More critically,
adding the webview forces the entire Kit app onto GPUI Fast; Linux must start
with X11/XWayland ([`website/docs/webview.md:10-19`](https://github.com/longbridge/gpui-kit/blob/288767cc730ca4977852a7860f52a8465a61876f/website/docs/webview.md#L10-L19),
[`website/docs/webview.md:47-79`](https://github.com/longbridge/gpui-kit/blob/288767cc730ca4977852a7860f52a8465a61876f/website/docs/webview.md#L47-L79),
[`crates/webview/Cargo.toml:1-28`](https://github.com/longbridge/gpui-kit/blob/288767cc730ca4977852a7860f52a8465a61876f/crates/webview/Cargo.toml#L1-L28)).
Do not make that a default mail-reading dependency unless MegaMail accepts this
backend/display-server switch. Kit warns that web content is untrusted and
requires explicit download, external-link, and IPC policy
([`website/docs/webview.md:120-130`](https://github.com/longbridge/gpui-kit/blob/288767cc730ca4977852a7860f52a8465a61876f/website/docs/webview.md#L120-L130)).

## Accessibility, testing, and performance

Kit and Base bridge semantics through AccessKit. Controls need stable IDs,
meaningful roles/names/states/actions, keyboard operation, and visible focus;
repeated list item IDs should derive from stable mail IDs, not row positions
([`website/docs/accessibility.md:7-24`](https://github.com/longbridge/gpui-kit/blob/288767cc730ca4977852a7860f52a8465a61876f/website/docs/accessibility.md#L7-L24),
[`website/docs/accessibility.md:189-200`](https://github.com/longbridge/gpui-kit/blob/288767cc730ca4977852a7860f52a8465a61876f/website/docs/accessibility.md#L189-L200)).
Enable Kit's `test-support` feature for headless integration tests that drive
real controls and inspect state, focus, layout, and accessibility snapshots.
These tests do not establish actual screen-reader announcements or native
window/platform behavior ([`website/docs/test.md:10-17`](https://github.com/longbridge/gpui-kit/blob/288767cc730ca4977852a7860f52a8465a61876f/website/docs/test.md#L10-L17),
[`website/docs/accessibility.md:327-329`](https://github.com/longbridge/gpui-kit/blob/288767cc730ca4977852a7860f52a8465a61876f/website/docs/accessibility.md#L327-L329)).

Treat the README's 120 FPS as an aspiration, not a guarantee. Kit's own FPS
guide defines 120 Hz as an 8.33 ms frame budget and says draw time excludes
platform submission, GPU completion, and compositor display; benchmark a
realistic inbox and long message on each target OS
([`README.md:34-50`](https://github.com/longbridge/gpui-kit/blob/288767cc730ca4977852a7860f52a8465a61876f/README.md#L34-L50),
[`website/docs/fps.md:28-35`](https://github.com/longbridge/gpui-kit/blob/288767cc730ca4977852a7860f52a8465a61876f/website/docs/fps.md#L28-L35),
[`website/docs/fps.md:101-134`](https://github.com/longbridge/gpui-kit/blob/288767cc730ca4977852a7860f52a8465a61876f/website/docs/fps.md#L101-L134)).
First performance wins are bounded UI work: virtualize the inbox, keep row
rendering cheap, and avoid invalidating the full window while idle. GPUI Fast is
an opt-in backend, not an automatic performance guarantee. Optimizing Kit
dependencies in dev builds improves runtime responsiveness but can increase
first-build time and is not a shipping benchmark
([`website/docs/installation.md:108-133`](https://github.com/longbridge/gpui-kit/blob/288767cc730ca4977852a7860f52a8465a61876f/website/docs/installation.md#L108-L133)).

## Zeron compatibility

Zeron is also a Rust/GPUI app, but its checked-out workspace does not use the
same GPUI distribution: it pins `gpui`, `gpui_platform`, and `gpui_tokio` to a
custom `zeronsh/zui` Git revision and `gpui-base` to a `zeronsh/gpui-component`
Git revision ([Zeron at
`037f4c10d67a38175b2386e776aba54355b4e941`, `Cargo.toml:65-98`](https://github.com/zeronsh/zeron/blob/037f4c10d67a38175b2386e776aba54355b4e941/Cargo.toml#L65-L98)).
Kit uses the distinct crates.io `gpui-pre-*` snapshot family. Their UI entities,
windows, and elements are not interchangeable Cargo types. Port the visual
language and interaction patterns into Kit views, then reuse only code that is
independent of those GPUI types; do not add Zeron's GPUI fork beside Kit.

## Checks and limits

This is source/API research only. I did not run Cargo builds, tests, native
screen-reader checks, or app-level performance measurements. The specific
message-rendering quality and frame budget still need evaluation in MegaMail.

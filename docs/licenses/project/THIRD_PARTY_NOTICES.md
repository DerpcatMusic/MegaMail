# Third-party notices

MegaMail is a fork of [Hylki](https://github.com/hyprlab/hylki) and retains
Hylki’s AGPL-3.0-or-later license in `LICENSE`. Upstream media notices remain
in `docs/LICENSE.md`; they are not all covered by the application license.

## Zeron

The native application's visual tokens and layout direction are derived from
[Zeron](https://github.com/zeronsh/zeron), inspected at commit
`037f4c10d67a38175b2386e776aba54355b4e941`. The following modules also adapt
actual MIT-licensed Zeron implementation code, with source anchors and the
copyright notice retained in their headers:

- `apps/megamail/src/zeron_style.rs`: navigation and account control geometry,
  settings selectors, neutral interaction washes, inset rings, and hairlines
  from `theme.rs`, `popover.rs`, `settings/widgets.rs`, and `shell.rs`.
- `apps/megamail/src/zeron_background.rs`: gradient material construction from
  `crates/ui/src/glass.rs`, used with MegaMail's built-in backgrounds.
- `apps/megamail/src/zeron_wallpaper.rs`: Dither, ASCII, Halftone, Scanlines,
  and contrast-based wallpaper opacity processing from
  `crates/mobile/src/wallpaper.rs`, adapted to bounded `image::RgbaImage` data.

No Zeron logo or upstream wallpaper artwork is used as a MegaMail product
asset. Custom artwork is selected by the user. Retain this notice with copied
material.

```text
MIT License

Copyright (c) 2026 Wing

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## Geist

The application bundles Geist Regular, Medium and Semibold from the pinned
Zeron source. Copyright The Geist Project Authors; source provenance is
Vercel Geist Font v1.7.2. The full SIL Open Font License 1.1 and provenance
are retained in `apps/megamail/assets/fonts/`.

## GPUI Kit

The application depends on [GPUI Kit](https://github.com/longbridge/gpui-kit)
0.7.1, published under Apache-2.0. Its transitive dependencies and bundled
icon assets retain their respective licenses. The generated
[dependency and asset notice bundle](docs/licenses/README.md) records the
locked Cargo graph, available license texts, and source archives with missing
license files. The per-user installer places this bundle alongside the app's
shared data. The repository does not publish a downloadable binary release.

The application registers Kit's complete icon source so catalog mail icons are
present. The Lucide ISC and Feather-derived MIT notices are retained in
`apps/megamail/assets/icons/LICENSE-LUCIDE`.

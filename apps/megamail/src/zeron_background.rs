//! Wallpaper-backed shell surfaces adapted from Zeron's GPUI material system.
//!
//! The gradient construction follows `zeronsh/zeron`, pinned commit
//! `037f4c10d67a38175b2386e776aba54355b4e941`, `crates/ui/src/glass.rs`
//! (`vertical`), MIT. The in-window wallpaper is MegaMail's own static cached
//! `RenderImage`; translucent panels reveal that image without relying on
//! platform compositor blur. The image importer/effects live in `appearance`
//! and `zeron_wallpaper`.
//!
//! Copyright (c) 2026 Wing
//!
//! Permission is hereby granted, free of charge, to any person obtaining a copy
//! of this software and associated documentation files (the "Software"), to deal
//! in the Software without restriction, including without limitation the rights
//! to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
//! copies of the Software, and to permit persons to whom the Software is
//! furnished to do so, subject to the following conditions:
//!
//! The above copyright notice and this permission notice shall be included in all
//! copies or substantial portions of the Software.
//!
//! THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
//! IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
//! FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
//! AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
//! LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
//! OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
//! SOFTWARE.

use gpui_kit::{
    Background, Div, Hsla, ImageSource, ObjectFit, ParentElement as _, Styled as _, div, hsla, img,
    linear_color_stop, linear_gradient, rgb,
};

use crate::appearance::{AppearancePreset, AppearanceState};
use gpui_kit::StyledImage as _;

/// Paint a full-window wallpaper layer. Use as the first child of a `relative`
/// shell; all foreground text stays in separate contrast-checked surfaces.
pub(crate) fn background_element(state: &AppearanceState, dark: bool, accent: Hsla) -> Div {
    if state.preset() == AppearancePreset::Custom {
        if let Some(image) = state.cached_image() {
            let base: Hsla = if dark {
                rgb(0x060606).into()
            } else {
                rgb(0xf7f7f9).into()
            };
            let clear = Hsla { a: 0.0, ..base };
            return div()
                .absolute()
                .inset_0()
                .overflow_hidden()
                .bg(base)
                .child(
                    img(ImageSource::Render(image))
                        .size_full()
                        .object_fit(ObjectFit::Cover)
                        .opacity(state.opacity()),
                )
                .child(div().absolute().inset_0().bg(vertical(
                    hsla(
                        0.0,
                        0.0,
                        if dark { 0.0 } else { 1.0 },
                        if dark { 0.10 } else { 0.045 },
                    ),
                    clear,
                )));
        }
    }

    match state.preset() {
        AppearancePreset::Aurora | AppearancePreset::Custom => aurora(dark, accent),
        AppearancePreset::Midnight => midnight(dark),
        AppearancePreset::Paper => paper(dark),
    }
}

/// A dark frosted navigation plane. The image's measured safe opacity bounds
/// wallpaper bleed, while the 20% tint keeps folder labels independent of it.
pub(crate) fn rail_surface(state: &AppearanceState, dark: bool) -> Hsla {
    surface(state, if dark { 0x0d0d0d } else { 0xf7f7f9 }, 0.20)
}

/// Message-list material keeps row labels legible while allowing a restrained
/// view of the selected background at pane edges.
pub(crate) fn list_surface(state: &AppearanceState, dark: bool) -> Hsla {
    surface(state, if dark { 0x090909 } else { 0xfcfcfd }, 0.05)
}

/// The reading plane is nearly opaque so long text remains comfortable.
pub(crate) fn reader_surface(state: &AppearanceState, dark: bool) -> Hsla {
    surface(state, if dark { 0x060606 } else { 0xffffff }, 0.05)
}

fn surface(state: &AppearanceState, color: u32, nominal_bleed: f32) -> Hsla {
    let wallpaper_opacity = state.opacity().clamp(0.0, 1.0);
    let maximum_bleed = if state.preset() == AppearancePreset::Custom && wallpaper_opacity > 0.001 {
        (state.safe_opacity() / wallpaper_opacity).clamp(0.0, 1.0)
    } else {
        1.0
    };
    let alpha = 1.0 - nominal_bleed.min(maximum_bleed);
    let color: Hsla = rgb(color).into();
    Hsla { a: alpha, ..color }
}

fn aurora(dark: bool, accent: Hsla) -> Div {
    let (base, violet, aqua, neutral_top) = if dark {
        (0x07090d, 0x9c84ff, 0x52d6c8, hsla(0.0, 0.0, 1.0, 0.055))
    } else {
        (0xf4f2f8, 0x795ce6, 0x41b5aa, hsla(0.0, 0.0, 0.0, 0.025))
    };
    let base: Hsla = rgb(base).into();
    let clear = Hsla { a: 0.0, ..base };
    let violet: Hsla = rgb(violet).into();
    let aqua: Hsla = rgb(aqua).into();
    let accent = Hsla {
        a: if dark { 0.12 } else { 0.07 },
        ..accent
    };
    div()
        .absolute()
        .inset_0()
        .overflow_hidden()
        .bg(base)
        .child(div().absolute().inset_0().bg(vertical(neutral_top, clear)))
        .child(div().absolute().inset_0().bg(wash(
            140.0,
            with_alpha(violet, if dark { 0.52 } else { 0.24 }),
            clear,
        )))
        .child(div().absolute().inset_0().bg(wash(
            310.0,
            with_alpha(aqua, if dark { 0.30 } else { 0.16 }),
            clear,
        )))
        .child(div().absolute().inset_0().bg(wash(42.0, accent, clear)))
}

fn midnight(dark: bool) -> Div {
    let (base, blue, cyan, violet) = if dark {
        (0x050812, 0x1b2f64, 0x174964, 0x27204d)
    } else {
        (0xeef1f8, 0x6a8cd8, 0x71b9c3, 0x8b80ca)
    };
    let base: Hsla = rgb(base).into();
    let clear = Hsla { a: 0.0, ..base };
    div()
        .absolute()
        .inset_0()
        .overflow_hidden()
        .bg(base)
        .child(div().absolute().inset_0().bg(vertical(
            hsla(
                0.0,
                0.0,
                if dark { 1.0 } else { 0.0 },
                if dark { 0.06 } else { 0.018 },
            ),
            clear,
        )))
        .child(div().absolute().inset_0().bg(wash(
            126.0,
            tint(blue, if dark { 0.68 } else { 0.24 }),
            clear,
        )))
        .child(div().absolute().inset_0().bg(wash(
            305.0,
            tint(cyan, if dark { 0.28 } else { 0.13 }),
            clear,
        )))
        .child(div().absolute().inset_0().bg(wash(
            54.0,
            tint(violet, if dark { 0.26 } else { 0.12 }),
            clear,
        )))
}

fn paper(dark: bool) -> Div {
    let (base, amber, rose, light) = if dark {
        (
            0x17120f,
            0x8d5a32,
            0x5b3028,
            hsla(35.0 / 360.0, 0.30, 0.91, 0.05),
        )
    } else {
        (
            0xf2ecdf,
            0xc9955c,
            0xd3a18d,
            hsla(35.0 / 360.0, 0.56, 0.98, 0.09),
        )
    };
    let base: Hsla = rgb(base).into();
    let clear = Hsla { a: 0.0, ..base };
    div()
        .absolute()
        .inset_0()
        .overflow_hidden()
        .bg(base)
        .child(div().absolute().inset_0().bg(vertical(light, clear)))
        .child(div().absolute().inset_0().bg(wash(
            128.0,
            tint(amber, if dark { 0.45 } else { 0.30 }),
            clear,
        )))
        .child(div().absolute().inset_0().bg(wash(
            320.0,
            tint(rose, if dark { 0.28 } else { 0.19 }),
            clear,
        )))
}

fn vertical(top: Hsla, bottom: Hsla) -> Background {
    linear_gradient(
        180.0,
        linear_color_stop(top, 0.0),
        linear_color_stop(bottom, 1.0),
    )
}

fn wash(angle: f32, near: Hsla, clear: Hsla) -> Background {
    linear_gradient(
        angle,
        linear_color_stop(near, 0.0),
        linear_color_stop(clear, 1.0),
    )
}

fn with_alpha(color: Hsla, alpha: f32) -> Hsla {
    Hsla { a: alpha, ..color }
}

fn tint(color: u32, alpha: f32) -> Hsla {
    let color: Hsla = rgb(color).into();
    with_alpha(color, alpha)
}

// Copyright (c) 2026 Wing
// SPDX-License-Identifier: MIT
//
// Adapted from Zeron commit 037f4c10d67a38175b2386e776aba54355b4e941:
// `crates/ui/src/theme.rs` (wash_for/card_selected_bg/card_selected_shadows,
// 1728-1733, 1804-1808, 1842-1853),
// `crates/ui/src/settings/widgets.rs` (section_tab, 1034-1073), and
// `crates/ui/src/shell.rs` (account trigger geometry, 8964-9069).
// MegaMail's Palette remains authoritative for base colors; these helpers add
// only neutral interaction washes and preserve the UI's primary/muted/faint
// text hierarchy while background materials remain a separate layer.
//
// MIT License
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

use gpui_kit::{BoxShadow, Hsla, hsla, point, px};

// Zeron's compact settings tab height and navigation text size.
pub(crate) const NAV_ROW_MIN_HEIGHT: f32 = 32.0;
pub(crate) const NAV_ROW_TEXT_SIZE: f32 = 13.0;

pub(crate) const ACCOUNT_TRIGGER_HEIGHT: f32 = 28.0;
pub(crate) const ACCOUNT_TRIGGER_GAP: f32 = 8.0;
pub(crate) const ACCOUNT_AVATAR_SIZE: f32 = 16.0;
pub(crate) const ACCOUNT_AVATAR_GLYPH_SIZE: f32 = 10.0;
pub(crate) const ACCOUNT_PRIMARY_TEXT_SIZE: f32 = 13.0;
pub(crate) const ACCOUNT_DETAIL_TEXT_SIZE: f32 = 11.0;
pub(crate) const ACCOUNT_DETAIL_LINE_HEIGHT: f32 = 17.0;

/// The app's three neutral text roles, kept tied to its one normative palette.
/// Zeron uses the same primary/muted/faint hierarchy instead of inventing a
/// separate text color for every component.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TextRole {
    Primary,
    Secondary,
    Tertiary,
}

pub(crate) fn text_role_color(palette: &super::Palette, role: TextRole) -> Hsla {
    match role {
        TextRole::Primary => palette.text,
        TextRole::Secondary => palette.muted,
        TextRole::Tertiary => palette.faint,
    }
}

pub(crate) const fn text_role_size(role: TextRole) -> f32 {
    match role {
        TextRole::Primary => NAV_ROW_TEXT_SIZE,
        TextRole::Secondary => ACCOUNT_DETAIL_TEXT_SIZE,
        TextRole::Tertiary => ACCOUNT_AVATAR_GLYPH_SIZE,
    }
}

/// Zeron-style neutral interaction wash, rendered over the UI's selected
/// background material rather than introducing another surface color.
pub(crate) fn floating_interaction_fill(dark: bool) -> Hsla {
    wash(dark, if dark { 0.11 } else { 0.06 })
}

/// Zeron's compact navigation uses one soft neutral wash for both the active
/// row and hover; the caller keeps selection semantics in its model/accessibility
/// state, not in a second colored plate.
pub(crate) fn sidebar_row_fill(dark: bool, selected: bool, hovered: bool) -> Hsla {
    if selected || hovered {
        floating_interaction_fill(dark)
    } else {
        wash(dark, 0.0)
    }
}

/// Zeron's selected floating-control outline: an inset one-pixel ring that
/// defines selection without shifting neighboring rows or changing their size.
pub(crate) fn floating_selection_ring(dark: bool) -> BoxShadow {
    let color = if dark {
        hsla(0.0, 0.0, 1.0, 0.09)
    } else {
        hsla(0.0, 0.0, 0.0, 0.07)
    };

    BoxShadow {
        color,
        offset: point(px(0.0), px(0.0)),
        blur_radius: px(0.0),
        spread_radius: px(1.0),
        inset: true,
    }
}

/// Zeron's soft-white/soft-black interactive wash, ported from `wash_for`.
fn wash(dark: bool, alpha: f32) -> Hsla {
    if dark {
        hsla(0.0, 0.0, 0.92, alpha)
    } else {
        hsla(0.0, 0.0, 0.10, alpha)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floating_treatment_keeps_zeron_light_and_dark_values() {
        let dark_fill = floating_interaction_fill(true);
        assert_eq!((dark_fill.l, dark_fill.a), (0.92, 0.11));

        let light_fill = floating_interaction_fill(false);
        assert_eq!((light_fill.l, light_fill.a), (0.10, 0.06));

        let dark_ring = floating_selection_ring(true);
        assert_eq!((dark_ring.color.l, dark_ring.color.a), (1.0, 0.09));
        assert!(dark_ring.inset);
        assert_eq!(dark_ring.spread_radius, px(1.0));

        let light_ring = floating_selection_ring(false);
        assert_eq!((light_ring.color.l, light_ring.color.a), (0.0, 0.07));
        assert!(light_ring.inset);
        assert_eq!(light_ring.spread_radius, px(1.0));
    }
}

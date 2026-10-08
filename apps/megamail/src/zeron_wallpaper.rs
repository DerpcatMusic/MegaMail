//! Zeron's cached wallpaper treatments and contrast guard, ported to
//! `image::RgbaImage` so they run on GPUI Kit's background executor.
//!
//! Adapted from `zeronsh/zeron`, pinned commit
//! `037f4c10d67a38175b2386e776aba54355b4e941`,
//! `crates/mobile/src/wallpaper.rs` (`wallpaper_render`, the four pixel
//! treatments and `wallpaper_safe_opacity`), MIT. The source-space algorithms
//! are retained; input dimensions are now checked by the importer before any
//! processing to bound allocation and arithmetic.
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

use image::{Rgba, RgbaImage};

/// A source-space treatment from Zeron's new-thread wallpaper controls.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum WallpaperEffect {
    #[default]
    None,
    Dither,
    Ascii,
    Halftone,
    Scanlines,
}

impl WallpaperEffect {
    pub(crate) const ALL: [Self; 5] = [
        Self::None,
        Self::Dither,
        Self::Ascii,
        Self::Halftone,
        Self::Scanlines,
    ];

    pub(crate) fn as_key(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Dither => "dither",
            Self::Ascii => "ascii",
            Self::Halftone => "halftone",
            Self::Scanlines => "scanlines",
        }
    }

    pub(crate) fn from_key(value: &str) -> Option<Self> {
        match value {
            "none" => Some(Self::None),
            "dither" => Some(Self::Dither),
            "ascii" => Some(Self::Ascii),
            "halftone" => Some(Self::Halftone),
            "scanlines" => Some(Self::Scanlines),
            _ => None,
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::None => "Original",
            Self::Dither => "Dither",
            Self::Ascii => "ASCII",
            Self::Halftone => "Halftone",
            Self::Scanlines => "Scanlines",
        }
    }
}

fn luma(r: u8, g: u8, b: u8) -> u8 {
    ((2126 * u32::from(r) + 7152 * u32::from(g) + 722 * u32::from(b)) / 10000) as u8
}

/// Apply a wallpaper treatment. The input has already been bounded to at most
/// 2500px per edge by the importer; every effect preserves its dimensions.
pub(crate) fn render(source: &RgbaImage, effect: WallpaperEffect, light: bool) -> RgbaImage {
    match effect {
        WallpaperEffect::None => source.clone(),
        WallpaperEffect::Scanlines => scanlines(source, light),
        WallpaperEffect::Ascii => ascii(source, light),
        WallpaperEffect::Halftone => halftone(source, light),
        WallpaperEffect::Dither => dither(source),
    }
}

/// Blend the cached Zeron treatment over its source without involving the UI
/// renderer. Strength is clamped here because preference files are untrusted.
pub(crate) fn render_with_strength(
    source: &RgbaImage,
    effect: WallpaperEffect,
    light: bool,
    strength: f32,
) -> RgbaImage {
    let strength = if strength.is_finite() {
        strength.clamp(0.0, 1.0)
    } else {
        1.0
    };
    if strength >= 1.0 || effect == WallpaperEffect::None {
        return render(source, effect, light);
    }
    if strength <= 0.0 {
        return source.clone();
    }
    let treated = render(source, effect, light);
    RgbaImage::from_fn(source.width(), source.height(), |x, y| {
        let original = source.get_pixel(x, y).0;
        let treated = treated.get_pixel(x, y).0;
        let blend = |from: u8, to: u8| {
            (f32::from(from) + (f32::from(to) - f32::from(from)) * strength)
                .round()
                .clamp(0.0, 255.0) as u8
        };
        Rgba([
            blend(original[0], treated[0]),
            blend(original[1], treated[1]),
            blend(original[2], treated[2]),
            original[3],
        ])
    })
}

fn scanlines(source: &RgbaImage, light: bool) -> RgbaImage {
    let (width, height) = source.dimensions();
    let mut output = RgbaImage::new(width, height);
    for y in 0..height {
        let gain = if y % 3 == 0 { 0.52 } else { 1.0 };
        for x in 0..width {
            let [r, g, b, a] = source.get_pixel(x, y).0;
            let channel = |value: u8| {
                if light {
                    (value as f32 + (255.0 - value as f32) * (1.0 - gain)) as u8
                } else {
                    (value as f32 * gain) as u8
                }
            };
            output.put_pixel(x, y, Rgba([channel(r), channel(g), channel(b), a]));
        }
    }
    output
}

fn ascii(source: &RgbaImage, light: bool) -> RgbaImage {
    // Five-column bitmap glyphs, one column/row of spacing (the Zeron table).
    const GLYPHS: [[u8; 7]; 10] = [
        [0, 0, 0, 0, 0, 0, 0],
        [0, 0, 0, 0, 0, 4, 0],
        [0, 4, 0, 0, 4, 0, 0],
        [0, 0, 0, 14, 0, 0, 0],
        [0, 0, 14, 0, 14, 0, 0],
        [0, 4, 4, 31, 4, 4, 0],
        [0, 21, 14, 31, 14, 21, 0],
        [10, 10, 31, 10, 31, 10, 10],
        [17, 2, 4, 4, 8, 16, 17],
        [14, 17, 23, 21, 23, 16, 14],
    ];
    let (width, height) = source.dimensions();
    RgbaImage::from_fn(width, height, |x, y| {
        let sx = (x / 6 * 6 + 3).min(width - 1);
        let sy = (y / 8 * 8 + 4).min(height - 1);
        let sample = source.get_pixel(sx, sy);
        let density = if light {
            255 - luma(sample[0], sample[1], sample[2])
        } else {
            luma(sample[0], sample[1], sample[2])
        };
        let index = ((density as f32 / 255.0).sqrt() * 9.0) as usize;
        let ink = x % 6 < 5
            && y % 8 < 7
            && GLYPHS[index.min(9)][(y % 8) as usize] & (1 << (4 - x % 6)) != 0;
        let [r, g, b, a] = source.get_pixel(x, y).0;
        let [cr, cg, cb, _] = sample.0;
        let paper = if light { 255.0 } else { 0.0 };
        let mix = |base: u8, glyph: u8| {
            (base as f32 * 0.60
                + if ink {
                    glyph as f32 * 0.40
                } else {
                    paper * 0.40
                }) as u8
        };
        Rgba([mix(r, cr), mix(g, cg), mix(b, cb), a])
    })
}

fn halftone(source: &RgbaImage, light: bool) -> RgbaImage {
    let (width, height) = source.dimensions();
    let paper = if light { 255u8 } else { 0 };
    let mut output = RgbaImage::from_pixel(width, height, Rgba([paper, paper, paper, 255]));
    for y in (0..height).step_by(4) {
        for x in (0..width).step_by(4) {
            let source_pixel = source.get_pixel(x, y);
            let mut luminance = luma(source_pixel[0], source_pixel[1], source_pixel[2]);
            if light {
                luminance = 255 - luminance;
            }
            let radius = 2.0 * (0.3 + 0.7 * (luminance as f32 / 255.0).sqrt());
            let sample = source.get_pixel((x + 2).min(width - 1), (y + 2).min(height - 1));
            let [r, g, b, a] = sample.0;
            for dy in 0..4.min(height - y) {
                for dx in 0..4.min(width - x) {
                    let distance = ((dx as f32 - 1.5).powi(2) + (dy as f32 - 1.5).powi(2)).sqrt();
                    let coverage = (radius + 0.5 - distance).clamp(0.0, 1.0) * a as f32 / 255.0;
                    let [sr, sg, sb, sa] = source.get_pixel(x + dx, y + dy).0;
                    let blend = |source: u8, dot: u8| {
                        (source as f32 * 0.60
                            + (dot as f32 * coverage + paper as f32 * (1.0 - coverage)) * 0.40)
                            as u8
                    };
                    output.put_pixel(
                        x + dx,
                        y + dy,
                        Rgba([blend(sr, r), blend(sg, g), blend(sb, b), sa]),
                    );
                }
            }
        }
    }
    output
}

fn dither(source: &RgbaImage) -> RgbaImage {
    const BAYER: [[u8; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];
    let (width, height) = source.dimensions();
    let mut output = RgbaImage::new(width, height);
    for y in (0..height).step_by(2) {
        for x in (0..width).step_by(2) {
            let [r, g, b, a] = source
                .get_pixel((x + 1).min(width - 1), (y + 1).min(height - 1))
                .0;
            let threshold = BAYER[(y as usize / 2) % 4][(x as usize / 2) % 4];
            let peak = r.max(g).max(b) as f32;
            let bright = peak / 255.0 > (threshold as f32 + 0.5) / 16.0;
            let gain = if bright { 255.0 / peak.max(1.0) } else { 0.08 };
            let color = [
                (r as f32 * gain).round() as u8,
                (g as f32 * gain).round() as u8,
                (b as f32 * gain).round() as u8,
                a,
            ];
            for dy in 0..2.min(height - y) {
                for dx in 0..2.min(width - x) {
                    output.put_pixel(x + dx, y + dy, Rgba(color));
                }
            }
        }
    }
    output
}

/// Estimate a contrast-preserving artwork alpha over the theme's base color.
/// This ports Zeron's 5th/95th-percentile sample guard; it intentionally ignores
/// rare outlier pixels instead of claiming a per-pixel contrast guarantee.
pub(crate) fn safe_opacity(
    image: &RgbaImage,
    text_rgb: u32,
    background_rgb: u32,
    region: f32,
    min_contrast: f32,
    max_opacity: f32,
) -> f32 {
    let (width, height) = image.dimensions();
    if width == 0 || height == 0 {
        return 0.0;
    }
    let rows = ((height as f32 * region.clamp(0.05, 1.0)).ceil() as u32).min(height);
    let sample_capacity = (width as usize * rows as usize) / 4 + 1;
    let mut samples: Vec<(f32, u32)> = Vec::with_capacity(sample_capacity);
    for y in (0..rows).step_by(2) {
        for x in (0..width).step_by(2) {
            let pixel = image.get_pixel(x, y);
            let rgb =
                (u32::from(pixel[0]) << 16) | (u32::from(pixel[1]) << 8) | u32::from(pixel[2]);
            samples.push((rel_luminance(rgb), rgb));
        }
    }
    if samples.is_empty() {
        return 0.0;
    }
    samples.sort_by(|left, right| {
        left.0
            .partial_cmp(&right.0)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let text = rel_luminance(text_rgb);
    let light_text = text > rel_luminance(background_rgb);
    let percentile =
        |fraction: f32| samples[((samples.len() - 1) as f32 * fraction).round() as usize].1;
    let worst = if light_text {
        percentile(0.95)
    } else {
        percentile(0.05)
    };
    let over = |alpha: f32| {
        let channel = |shift: u32| {
            let artwork = (worst >> shift & 0xFF) as f32;
            let base = (background_rgb >> shift & 0xFF) as f32;
            (alpha * artwork + (1.0 - alpha) * base)
                .round()
                .clamp(0.0, 255.0) as u32
        };
        channel(16) << 16 | channel(8) << 8 | channel(0)
    };
    let is_safe = |alpha: f32| contrast(text, rel_luminance(over(alpha))) >= min_contrast;
    if is_safe(max_opacity) {
        return max_opacity;
    }
    let (mut low, mut high) = (0.0f32, max_opacity);
    for _ in 0..24 {
        let middle = (low + high) / 2.0;
        if is_safe(middle) {
            low = middle;
        } else {
            high = middle;
        }
    }
    low
}

fn linear(channel: u8) -> f32 {
    let value = channel as f32 / 255.0;
    if value <= 0.040_45 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

fn rel_luminance(rgb: u32) -> f32 {
    let red = (rgb >> 16) as u8;
    let green = (rgb >> 8) as u8;
    let blue = rgb as u8;
    0.2126 * linear(red) + 0.7152 * linear(green) + 0.0722 * linear(blue)
}

fn contrast(first: f32, second: f32) -> f32 {
    let (high, low) = if first > second {
        (first, second)
    } else {
        (second, first)
    };
    (high + 0.05) / (low + 0.05)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_wallpaper_effects_keep_dimensions() {
        let image = RgbaImage::from_fn(32, 24, |x, y| {
            Rgba([(x * 5) as u8, (y * 7) as u8, 170, 255])
        });
        for effect in WallpaperEffect::ALL {
            for light in [false, true] {
                let output = render(&image, effect, light);
                assert_eq!(output.dimensions(), image.dimensions());
            }
        }
    }

    #[test]
    fn effect_strength_blends_from_original_to_full_zeron_treatment() {
        let source = RgbaImage::from_pixel(8, 8, Rgba([120, 90, 45, 200]));
        assert_eq!(
            render_with_strength(&source, WallpaperEffect::Scanlines, false, 0.0),
            source
        );
        assert_eq!(
            render_with_strength(&source, WallpaperEffect::Scanlines, false, 1.0),
            render(&source, WallpaperEffect::Scanlines, false)
        );
        assert_eq!(
            render_with_strength(&source, WallpaperEffect::None, false, 0.3),
            source
        );
    }

    #[test]
    fn safe_opacity_allows_black_wallpaper_behind_light_text() {
        let image = RgbaImage::from_pixel(8, 8, Rgba([0, 0, 0, 255]));
        let limit = safe_opacity(&image, 0xE8E8EA, 0x060606, 1.0, 4.5, 1.0);
        assert!((limit - 1.0).abs() < 0.0001);
    }

    #[test]
    fn safe_opacity_reduces_a_bright_wallpaper_under_light_text() {
        let image = RgbaImage::from_pixel(8, 8, Rgba([255, 255, 255, 255]));
        let text = 0xA9A9AE;
        let background = 0x060606;
        let limit = safe_opacity(&image, text, background, 1.0, 4.5, 1.0);
        assert!(limit < 0.5);

        let channel = (limit * 255.0 + (1.0 - limit) * 6.0).round() as u8;
        let composite = (u32::from(channel) << 16) | (u32::from(channel) << 8) | u32::from(channel);
        assert!(contrast(rel_luminance(text), rel_luminance(composite)) >= 4.45);
    }
}

// Copyright (C) 2026 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Counting measurements over fractional regions of an [`RgbaImage`].
//!
//! Ported unchanged from the dogfood suite (GUP-394, GUP-397). Each function
//! takes an image and a [`Region`] (fractions of the image size, origin
//! top-left) and returns a count, fraction or centroid list; the caller turns
//! it into a pass/fail check. Nothing here knows about gup.
//!
//! "Ink" differs from [`checks`](crate::checks): [`count_ink`] counts pixels
//! darker than the background by a luma contrast (dark text and axis lines
//! only), whereas the `checks` module's ink is any pixel more than a
//! perceptual (CIEDE2000) distance from the background, whatever its colour.
//!
//! ```
//! use gup_visual_regression::{Rgba8, RgbaImage};
//! use gup_visual_regression::measure::{Region, count_near};
//!
//! let img = RgbaImage::filled(10, 10, Rgba8::rgb(31, 119, 180));
//! assert_eq!(count_near(&img, Region::ALL, [31, 119, 180], 4), 100);
//! ```

use crate::{Rgba8, RgbaImage};
use std::collections::HashMap;

/// A rectangle expressed as fractions (0..1) of the image width and height,
/// origin top-left.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Region {
    /// Left edge fraction.
    pub x0: f32,
    /// Top edge fraction.
    pub y0: f32,
    /// Right edge fraction (exclusive).
    pub x1: f32,
    /// Bottom edge fraction (exclusive).
    pub y1: f32,
}

impl Region {
    /// The whole image.
    pub const ALL: Region = Region {
        x0: 0.0,
        y0: 0.0,
        x1: 1.0,
        y1: 1.0,
    };

    /// A region from its edge fractions.
    pub const fn new(x0: f32, y0: f32, x1: f32, y1: f32) -> Self {
        Self { x0, y0, x1, y1 }
    }

    /// Pixel bounds `(x0, y0, x1, y1)` (exclusive end) inside `img`.
    fn bounds(&self, img: &RgbaImage) -> (u32, u32, u32, u32) {
        let (w, h) = (img.width() as f32, img.height() as f32);
        let clamp = |v: f32, max: f32| v.clamp(0.0, max).round() as u32;
        (
            clamp(self.x0 * w, w),
            clamp(self.y0 * h, h),
            clamp(self.x1 * w, w),
            clamp(self.y1 * h, h),
        )
    }

    fn pixels<'a>(&self, img: &'a RgbaImage) -> impl Iterator<Item = [u8; 3]> + 'a {
        let (x0, y0, x1, y1) = self.bounds(img);
        (y0..y1).flat_map(move |y| (x0..x1).map(move |x| rgb(img.pixel(x, y))))
    }
}

fn rgb(p: Rgba8) -> [u8; 3] {
    [p.r, p.g, p.b]
}

fn max_channel_diff(a: [u8; 3], b: [u8; 3]) -> u8 {
    a.iter()
        .zip(b)
        .map(|(x, y)| x.abs_diff(y))
        .max()
        .unwrap_or(0)
}

/// The most common colour in the image (alpha ignored); for charts this is
/// the background.
pub fn background(img: &RgbaImage) -> [u8; 3] {
    let mut hist: HashMap<[u8; 3], usize> = HashMap::new();
    for (_, _, p) in img.pixels() {
        *hist.entry(rgb(p)).or_default() += 1;
    }
    hist.into_iter()
        .max_by_key(|(_, n)| *n)
        .map(|(c, _)| c)
        .unwrap_or([0; 3])
}

/// Fraction of pixels in `region` that differ from the image background by
/// more than `tol` in any channel.
pub fn non_background_fraction(img: &RgbaImage, region: Region, tol: u8) -> f32 {
    let bg = background(img);
    let (mut n, mut total) = (0usize, 0usize);
    for p in region.pixels(img) {
        total += 1;
        if max_channel_diff(p, bg) > tol {
            n += 1;
        }
    }
    if total == 0 {
        0.0
    } else {
        n as f32 / total as f32
    }
}

/// Number of pixels in `region` within `tol` (per channel) of `target`.
pub fn count_near(img: &RgbaImage, region: Region, target: [u8; 3], tol: u8) -> usize {
    region
        .pixels(img)
        .filter(|p| max_channel_diff(*p, target) <= tol)
        .count()
}

/// Centroids (in pixels, pixel centres at +0.5) of the 4-connected blobs of
/// pixels within `tol` of `target` inside `region`.
pub fn blob_centroids(
    img: &RgbaImage,
    region: Region,
    target: [u8; 3],
    tol: u8,
) -> Vec<(f32, f32)> {
    let (x0, y0, x1, y1) = region.bounds(img);
    let (w, h) = ((x1 - x0) as usize, (y1 - y0) as usize);
    let hit = |x: usize, y: usize| {
        max_channel_diff(rgb(img.pixel(x0 + x as u32, y0 + y as u32)), target) <= tol
    };
    let mut seen = vec![false; w * h];
    let mut out = Vec::new();
    for sy in 0..h {
        for sx in 0..w {
            if seen[sy * w + sx] || !hit(sx, sy) {
                continue;
            }
            let (mut n, mut cx, mut cy) = (0f64, 0f64, 0f64);
            let mut stack = vec![(sx, sy)];
            seen[sy * w + sx] = true;
            while let Some((x, y)) = stack.pop() {
                n += 1.0;
                cx += x as f64 + 0.5 + x0 as f64;
                cy += y as f64 + 0.5 + y0 as f64;
                let nb = [
                    (x.wrapping_sub(1), y),
                    (x + 1, y),
                    (x, y.wrapping_sub(1)),
                    (x, y + 1),
                ];
                for (nx, ny) in nb {
                    if nx < w && ny < h && !seen[ny * w + nx] && hit(nx, ny) {
                        seen[ny * w + nx] = true;
                        stack.push((nx, ny));
                    }
                }
            }
            out.push(((cx / n) as f32, (cy / n) as f32));
        }
    }
    out
}

/// Number of "ink" pixels in `region`: pixels darker than the background by
/// at least `min_contrast` in luma. Text and axis lines are ink; empty
/// margins are not.
pub fn count_ink(img: &RgbaImage, region: Region, min_contrast: f32) -> usize {
    let bg = luma(background(img));
    region
        .pixels(img)
        .filter(|p| bg - luma(*p) >= min_contrast)
        .count()
}

/// What [`tooltip_panel`] found in a region.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Panel {
    /// The panel's fill: the most common exact colour in the interior.
    pub fill: [u8; 3],
    /// The text colour: the most common exact colour among contrasting pixels.
    pub ink: [u8; 3],
    /// Fraction of interior pixels on the fill-ink segment (fill, text or an
    /// antialiased blend of the two).
    pub panel_fraction: f32,
    /// Number of separate lines of text.
    pub lines: usize,
}

fn mode(it: impl Iterator<Item = [u8; 3]>) -> Option<[u8; 3]> {
    let mut hist: HashMap<[u8; 3], usize> = HashMap::new();
    for c in it {
        *hist.entry(c).or_default() += 1;
    }
    // Ties break on the colour so the result is deterministic.
    hist.into_iter()
        .max_by_key(|(c, n)| (*n, *c))
        .map(|(c, _)| c)
}

/// Largest per-channel distance from `p` to the segment `a..b`.
fn segment_distance(p: [u8; 3], a: [u8; 3], b: [u8; 3]) -> f32 {
    let d: [f32; 3] = std::array::from_fn(|i| b[i] as f32 - a[i] as f32);
    let len2: f32 = d.iter().map(|v| v * v).sum();
    let t = if len2 == 0.0 {
        0.0
    } else {
        ((0..3)
            .map(|i| (p[i] as f32 - a[i] as f32) * d[i])
            .sum::<f32>()
            / len2)
            .clamp(0.0, 1.0)
    };
    (0..3)
        .map(|i| (p[i] as f32 - (a[i] as f32 + t * d[i])).abs())
        .fold(0.0, f32::max)
}

/// Recognise a tooltip box of any colours in `region`: a panel of one fill
/// colour holding lines of text in a contrasting colour. The region is
/// inset by 2 px per side first, to skip a border and sub-pixel slop.
pub fn tooltip_panel(img: &RgbaImage, region: Region) -> Panel {
    let (x0, y0, x1, y1) = region.bounds(img);
    let (x0, y0, x1, y1) = (x0 + 2, y0 + 2, x1.saturating_sub(2), y1.saturating_sub(2));
    let none = Panel {
        fill: [0; 3],
        ink: [0; 3],
        panel_fraction: 0.0,
        lines: 0,
    };
    if x1 <= x0 || y1 <= y0 {
        return none;
    }
    let px = |x: u32, y: u32| rgb(img.pixel(x, y));
    let all = || (y0..y1).flat_map(|y| (x0..x1).map(move |x| (x, y)));
    let Some(fill) = mode(all().map(|(x, y)| px(x, y))) else {
        return none;
    };
    let fl = luma(fill);
    let is_text = |c: [u8; 3]| (luma(c) - fl).abs() >= 90.0;
    let Some(ink) = mode(all().map(|(x, y)| px(x, y)).filter(|c| is_text(*c))) else {
        return Panel { fill, ..none };
    };
    let total = ((x1 - x0) * (y1 - y0)) as f32;
    let on_panel = all()
        .filter(|&(x, y)| segment_distance(px(x, y), fill, ink) <= 8.0)
        .count();
    let (mut lines, mut run) = (0, 0);
    for y in y0..y1 {
        let inked = (x0..x1).filter(|&x| is_text(px(x, y))).count() >= 2;
        if inked {
            run += 1;
        } else {
            if run >= 3 {
                lines += 1;
            }
            run = 0;
        }
    }
    if run >= 3 {
        lines += 1;
    }
    Panel {
        fill,
        ink,
        panel_fraction: on_panel as f32 / total,
        lines,
    }
}

/// Number of pixels in `region` whose hue is within `tol_deg` of `hue_deg`
/// and whose HSV saturation is at least `min_sat` (0..1). Hue survives
/// alpha blending over white and gamma errors far better than exact RGB, so
/// this answers "is this colour family visibly present".
pub fn count_hue(
    img: &RgbaImage,
    region: Region,
    hue_deg: f32,
    tol_deg: f32,
    min_sat: f32,
) -> usize {
    region
        .pixels(img)
        .filter(|p| {
            let (h, s) = hue_sat(*p);
            s >= min_sat && hue_distance(h, hue_deg) <= tol_deg
        })
        .count()
}

/// Number of light-to-mid grey pixels in `region`: near-neutral (HSV
/// saturation at most `max_sat`), at least 3 luma levels darker than the
/// background, and no darker than `min_luma` (so black text and axis
/// strokes are excluded). Faint grid lines and de-emphasised marks count.
pub fn count_grey(img: &RgbaImage, region: Region, max_sat: f32, min_luma: f32) -> usize {
    let bg = luma(background(img));
    region
        .pixels(img)
        .filter(|p| {
            let (_, s) = hue_sat(*p);
            let l = luma(*p);
            s <= max_sat && l >= min_luma && l <= bg - 3.0
        })
        .count()
}

/// Number of horizontal runs (at least `min_len` pixels long) of
/// non-background pixels along the row at fraction `y`, between fractions
/// `x0` and `x1` — e.g. the number of separate bars crossing that row.
pub fn count_runs(img: &RgbaImage, y: f32, x0: f32, x1: f32, min_len: u32) -> usize {
    let bg = background(img);
    let (w, h) = (img.width() as f32, img.height() as f32);
    let row = ((y * h) as u32).min(img.height().saturating_sub(1));
    let (start, end) = ((x0 * w) as u32, ((x1 * w) as u32).min(img.width()));
    let (mut runs, mut len) = (0, 0);
    for x in start..end {
        if max_channel_diff(rgb(img.pixel(x, row)), bg) > 8 {
            len += 1;
        } else {
            if len >= min_len {
                runs += 1;
            }
            len = 0;
        }
    }
    if len >= min_len {
        runs += 1;
    }
    runs
}

/// Number of pixels in `region` with HSV saturation of at least `min_sat`
/// (any hue) — i.e. visibly coloured rather than white/grey/black.
pub fn count_saturated(img: &RgbaImage, region: Region, min_sat: f32) -> usize {
    region
        .pixels(img)
        .filter(|p| hue_sat(*p).1 >= min_sat)
        .count()
}

/// Hue in degrees of an sRGB colour given as floats in 0..1.
pub fn hue_of(c: [f32; 3]) -> f32 {
    hue_sat([
        (c[0] * 255.0) as u8,
        (c[1] * 255.0) as u8,
        (c[2] * 255.0) as u8,
    ])
    .0
}

fn luma(p: [u8; 3]) -> f32 {
    0.2126 * p[0] as f32 + 0.7152 * p[1] as f32 + 0.0722 * p[2] as f32
}

fn hue_sat(p: [u8; 3]) -> (f32, f32) {
    let [r, g, b] = p.map(|v| v as f32 / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    let s = if max <= 0.0 { 0.0 } else { d / max };
    if d <= f32::EPSILON {
        return (0.0, s);
    }
    let h = if max == r {
        60.0 * (((g - b) / d).rem_euclid(6.0))
    } else if max == g {
        60.0 * ((b - r) / d + 2.0)
    } else {
        60.0 * ((r - g) / d + 4.0)
    };
    (h, s)
}

fn hue_distance(a: f32, b: f32) -> f32 {
    let d = (a - b).abs() % 360.0;
    d.min(360.0 - d)
}

#[cfg(test)]
mod tests {
    use super::*;

    const WHITE: Rgba8 = Rgba8::WHITE;

    fn rgba(r: u8, g: u8, b: u8, a: u8) -> Rgba8 {
        Rgba8::new(r, g, b, a)
    }

    /// White 100x50 image with a blue block on the left half and a black
    /// 10x2 "text" bar near the top-right.
    fn synthetic() -> RgbaImage {
        let mut img = RgbaImage::filled(100, 50, WHITE);
        for y in 20..40 {
            for x in 10..40 {
                img.set_pixel(x, y, rgba(31, 119, 180, 255));
            }
        }
        for y in 2..4 {
            for x in 80..90 {
                img.set_pixel(x, y, rgba(0, 0, 0, 255));
            }
        }
        img
    }

    #[test]
    fn background_is_dominant_colour() {
        assert_eq!(background(&synthetic()), [255, 255, 255]);
    }

    #[test]
    fn blank_image_has_no_foreground() {
        let img = RgbaImage::filled(10, 10, WHITE);
        assert_eq!(non_background_fraction(&img, Region::ALL, 8), 0.0);
        assert!(non_background_fraction(&synthetic(), Region::ALL, 8) > 0.1);
    }

    #[test]
    fn exact_and_hue_colour_counts() {
        let img = synthetic();
        assert_eq!(count_near(&img, Region::ALL, [31, 119, 180], 4), 600);
        assert_eq!(
            count_near(&img, Region::new(0.5, 0.0, 1.0, 1.0), [31, 119, 180], 4),
            0
        );
        let blue = hue_of([0.122, 0.467, 0.706]);
        assert_eq!(count_hue(&img, Region::ALL, blue, 10.0, 0.3), 600);
        // Orange is absent.
        assert_eq!(
            count_hue(&img, Region::ALL, hue_of([1.0, 0.498, 0.055]), 10.0, 0.3),
            0
        );
    }

    #[test]
    fn ink_detects_dark_marks_in_region_only() {
        let img = synthetic();
        assert_eq!(count_ink(&img, Region::new(0.75, 0.0, 1.0, 0.2), 100.0), 20);
        assert_eq!(count_ink(&img, Region::new(0.0, 0.0, 0.5, 0.2), 100.0), 0);
    }

    #[test]
    fn runs_count_separate_blocks_on_a_row() {
        let mut img = synthetic();
        for y in 20..40 {
            for x in 50..60 {
                img.set_pixel(x, y, rgba(200, 60, 60, 255));
            }
        }
        assert_eq!(count_runs(&img, 0.5, 0.0, 1.0, 3), 2);
        assert_eq!(count_runs(&img, 0.1, 0.0, 1.0, 3), 0);
    }

    #[test]
    fn grey_excludes_background_and_black() {
        let mut img = synthetic();
        for x in 0..100 {
            img.set_pixel(x, 45, rgba(220, 220, 220, 255));
        }
        assert_eq!(count_grey(&img, Region::ALL, 0.08, 120.0), 100);
        assert_eq!(count_saturated(&img, Region::ALL, 0.25), 600);
    }

    #[test]
    fn hue_wraps_around_red() {
        assert!(hue_distance(355.0, 5.0) <= 10.0);
    }

    fn tooltip_img(fill: [u8; 3], ink: [u8; 3], bars: usize) -> RgbaImage {
        let mut img = RgbaImage::filled(200, 160, WHITE);
        let dot = |img: &mut RgbaImage, x, y, c: [u8; 3]| {
            img.set_pixel(x, y, Rgba8::rgb(c[0], c[1], c[2]))
        };
        for y in 0..160 {
            for x in 0..200 {
                if (x + y) % 5 == 0 {
                    let hues = [[200, 30, 30], [30, 160, 30], [30, 30, 200]];
                    dot(&mut img, x, y, hues[(x % 3) as usize]);
                }
            }
        }
        for y in 20..140 {
            for x in 30..150 {
                let edge = x == 30 || x == 149 || y == 20 || y == 139;
                dot(&mut img, x, y, if edge { [0x99; 3] } else { fill });
            }
        }
        for i in 0..bars as u32 {
            let top = 30 + i * 17;
            for y in top..top + 12 {
                for x in 40..110 {
                    dot(&mut img, x, y, ink);
                }
                // Antialiasing column.
                let mid = [0, 1, 2].map(|c| ((fill[c] as u16 + ink[c] as u16) / 2) as u8);
                dot(&mut img, 110, y, mid);
            }
        }
        img
    }

    const TIP: Region = Region::new(0.15, 0.125, 0.75, 0.875);

    #[test]
    fn tooltip_panel_recognises_light_and_dark_panels() {
        let light = tooltip_panel(&tooltip_img([255; 3], [0x33; 3], 5), TIP);
        assert_eq!(
            (light.fill, light.ink, light.lines),
            ([255; 3], [0x33; 3], 5)
        );
        assert!(light.panel_fraction >= 0.95, "{light:?}");
        let dark = tooltip_panel(&tooltip_img([0x1b; 3], [255; 3], 5), TIP);
        assert_eq!((dark.fill, dark.ink, dark.lines), ([0x1b; 3], [255; 3], 5));
        assert!(dark.panel_fraction >= 0.95, "{dark:?}");
    }

    #[test]
    fn tooltip_panel_counts_lines() {
        let p = tooltip_panel(&tooltip_img([255; 3], [0x33; 3], 2), TIP);
        assert_eq!(p.lines, 2);
    }

    #[test]
    fn tooltip_panel_rejects_dots_without_a_panel() {
        let img = tooltip_img([255; 3], [0x33; 3], 5);
        // Outside the box: coloured dots on white.
        let p = tooltip_panel(&img, Region::new(0.78, 0.02, 0.99, 0.98));
        assert!(p.panel_fraction < 0.95 && p.lines < 5, "{p:?}");
    }

    #[test]
    fn tooltip_panel_rejects_marks_over_the_panel() {
        let mut img = tooltip_img([255; 3], [0x33; 3], 5);
        // Cover about 10% of the interior with coloured dots, off the
        // fill-ink segment.
        for y in 24..136 {
            for x in 34..146 {
                if (x * 7 + y * 13) % 10 == 0 {
                    img.set_pixel(x, y, rgba(200, 30, 30, 255));
                }
            }
        }
        let p = tooltip_panel(&img, TIP);
        assert!(p.panel_fraction < 0.95, "{p:?}");
    }
}

// Copyright (C) 2026 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Target-agnostic pixel measurements on decoded RGBA images.
//!
//! These functions know nothing about gup: they take an [`image::RgbaImage`]
//! and a [`Region`] (fractions of the image size) and return counts. The
//! suite manifest (`suite.rs`) turns them into pass/fail checks. They are
//! deliberately simple and should migrate to the shared visual-assertion
//! module once it exists (GUP-388).

use image::RgbaImage;
use std::collections::HashMap;

/// A rectangle expressed as fractions (0..1) of the image width and height,
/// origin top-left.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Region {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
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
        (y0..y1).flat_map(move |y| (x0..x1).map(move |x| rgb(img.get_pixel(x, y).0)))
    }
}

fn rgb(p: [u8; 4]) -> [u8; 3] {
    [p[0], p[1], p[2]]
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
    for p in img.pixels() {
        *hist.entry(rgb(p.0)).or_default() += 1;
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
        if max_channel_diff(rgb(img.get_pixel(x, row).0), bg) > 8 {
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
    use image::Rgba;

    /// White 100x50 image with a blue block on the left half and a black
    /// 10x2 "text" bar near the top-right.
    fn synthetic() -> RgbaImage {
        let mut img = RgbaImage::from_pixel(100, 50, Rgba([255, 255, 255, 255]));
        for y in 20..40 {
            for x in 10..40 {
                img.put_pixel(x, y, Rgba([31, 119, 180, 255]));
            }
        }
        for y in 2..4 {
            for x in 80..90 {
                img.put_pixel(x, y, Rgba([0, 0, 0, 255]));
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
        let img = RgbaImage::from_pixel(10, 10, Rgba([255, 255, 255, 255]));
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
                img.put_pixel(x, y, Rgba([200, 60, 60, 255]));
            }
        }
        assert_eq!(count_runs(&img, 0.5, 0.0, 1.0, 3), 2);
        assert_eq!(count_runs(&img, 0.1, 0.0, 1.0, 3), 0);
    }

    #[test]
    fn grey_excludes_background_and_black() {
        let mut img = synthetic();
        for x in 0..100 {
            img.put_pixel(x, 45, Rgba([220, 220, 220, 255]));
        }
        assert_eq!(count_grey(&img, Region::ALL, 0.08, 120.0), 100);
        assert_eq!(count_saturated(&img, Region::ALL, 0.25), 600);
    }

    #[test]
    fn hue_wraps_around_red() {
        assert!(hue_distance(355.0, 5.0) <= 10.0);
    }
}

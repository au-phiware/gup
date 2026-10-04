// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Perceptual image comparison based on per-pixel CIEDE2000.

use crate::color::Rgba8;
use crate::image::RgbaImage;
use std::fmt;

/// How different two images may be and still "match".
///
/// A pixel differs when its CIEDE2000 distance exceeds
/// [`pixel_delta_e`](Self::pixel_delta_e); the images match when no more
/// than [`max_differing_fraction`](Self::max_differing_fraction) of pixels
/// differ. Byte-exact comparison is too brittle across GPU backends; the
/// structural checks carry the correctness burden, this catches change.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct DiffTolerance {
    /// Per-pixel CIEDE2000 threshold.
    pub pixel_delta_e: f32,
    /// Maximum fraction (0–1) of pixels allowed to exceed the threshold.
    pub max_differing_fraction: f64,
}

impl Default for DiffTolerance {
    fn default() -> Self {
        Self {
            pixel_delta_e: 3.0,
            max_differing_fraction: 0.0025,
        }
    }
}

/// Summary statistics from comparing two equally sized images.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct DiffStats {
    /// Pixels whose ΔE exceeds the tolerance threshold.
    pub differing_pixels: usize,
    /// Total pixels compared.
    pub total_pixels: usize,
    /// Largest per-pixel ΔE.
    pub max_delta_e: f32,
    /// Mean per-pixel ΔE.
    pub mean_delta_e: f32,
}

impl DiffStats {
    /// Fraction (0–1) of pixels that differ.
    pub fn differing_fraction(&self) -> f64 {
        if self.total_pixels == 0 {
            0.0
        } else {
            self.differing_pixels as f64 / self.total_pixels as f64
        }
    }

    /// Whether these statistics are within `tol`.
    pub fn within(&self, tol: &DiffTolerance) -> bool {
        self.differing_fraction() <= tol.max_differing_fraction
    }
}

impl fmt::Display for DiffStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} of {} pixels differ ({:.3}%), max ΔE {:.1}, mean ΔE {:.3}",
            self.differing_pixels,
            self.total_pixels,
            self.differing_fraction() * 100.0,
            self.max_delta_e,
            self.mean_delta_e
        )
    }
}

/// The two images have different dimensions.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct SizeMismatch {
    /// Expected `(width, height)`.
    pub expected: (u32, u32),
    /// Actual `(width, height)`.
    pub actual: (u32, u32),
}

impl fmt::Display for SizeMismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "image size {}x{} does not match expected {}x{}",
            self.actual.0, self.actual.1, self.expected.0, self.expected.1
        )
    }
}

impl std::error::Error for SizeMismatch {}

fn same_size(expected: &RgbaImage, actual: &RgbaImage) -> Result<(), SizeMismatch> {
    if expected.width() != actual.width() || expected.height() != actual.height() {
        return Err(SizeMismatch {
            expected: (expected.width(), expected.height()),
            actual: (actual.width(), actual.height()),
        });
    }
    Ok(())
}

/// Compare two images pixel by pixel using CIEDE2000.
pub fn perceptual_diff(
    expected: &RgbaImage,
    actual: &RgbaImage,
    tol: &DiffTolerance,
) -> Result<DiffStats, SizeMismatch> {
    same_size(expected, actual)?;
    let mut differing = 0usize;
    let mut max = 0f32;
    let mut sum = 0f64;
    for ((_, _, e), (_, _, a)) in expected.pixels().zip(actual.pixels()) {
        let d = e.delta_e(a);
        if d > tol.pixel_delta_e {
            differing += 1;
        }
        max = max.max(d);
        sum += d as f64;
    }
    let total = expected.pixel_count();
    Ok(DiffStats {
        differing_pixels: differing,
        total_pixels: total,
        max_delta_e: max,
        mean_delta_e: if total == 0 {
            0.0
        } else {
            (sum / total as f64) as f32
        },
    })
}

/// Build a visual diff: the actual image faded to light grey, with pixels
/// exceeding the threshold painted magenta.
pub fn diff_image(
    expected: &RgbaImage,
    actual: &RgbaImage,
    tol: &DiffTolerance,
) -> Result<RgbaImage, SizeMismatch> {
    same_size(expected, actual)?;
    let mut out = RgbaImage::filled(actual.width(), actual.height(), Rgba8::WHITE);
    for ((x, y, e), (_, _, a)) in expected.pixels().zip(actual.pixels()) {
        let c = if e.delta_e(a) > tol.pixel_delta_e {
            Rgba8::rgb(255, 0, 255)
        } else {
            let o = a.over(Rgba8::WHITE);
            let luma = (0.299 * o.r as f32 + 0.587 * o.g as f32 + 0.114 * o.b as f32) as u8;
            let faded = 255 - (255 - luma) / 4;
            Rgba8::rgb(faded, faded, faded)
        };
        out.set_pixel(x, y, c);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::PxRect;

    #[test]
    fn identical_images_have_no_difference() {
        let a = RgbaImage::filled(20, 10, Rgba8::from_hex(0x336699));
        let stats = perceptual_diff(&a, &a, &DiffTolerance::default()).unwrap();
        assert_eq!(stats.differing_pixels, 0);
        assert_eq!(stats.max_delta_e, 0.0);
        assert!(stats.within(&DiffTolerance::default()));
    }

    #[test]
    fn imperceptible_noise_is_tolerated() {
        let a = RgbaImage::filled(20, 10, Rgba8::from_hex(0x336699));
        let mut b = a.clone();
        for x in 0..20 {
            b.set_pixel(x, 5, Rgba8::from_hex(0x346699));
        }
        let stats = perceptual_diff(&a, &b, &DiffTolerance::default()).unwrap();
        assert_eq!(stats.differing_pixels, 0, "{stats}");
    }

    #[test]
    fn a_moved_mark_is_a_mismatch() {
        let mut a = RgbaImage::filled(100, 100, Rgba8::WHITE);
        let mut b = a.clone();
        a.fill_rect(PxRect::new(10.0, 10.0, 10.0, 10.0), Rgba8::BLACK);
        b.fill_rect(PxRect::new(30.0, 10.0, 10.0, 10.0), Rgba8::BLACK);
        let tol = DiffTolerance::default();
        let stats = perceptual_diff(&a, &b, &tol).unwrap();
        assert_eq!(stats.differing_pixels, 200);
        assert!(!stats.within(&tol), "{stats}");
        let diff = diff_image(&a, &b, &tol).unwrap();
        assert_eq!(diff.pixel(15, 15), Rgba8::rgb(255, 0, 255));
        assert_ne!(diff.pixel(50, 50), Rgba8::rgb(255, 0, 255));
    }

    #[test]
    fn size_mismatch_is_an_error() {
        let a = RgbaImage::filled(2, 2, Rgba8::WHITE);
        let b = RgbaImage::filled(3, 2, Rgba8::WHITE);
        assert!(perceptual_diff(&a, &b, &DiffTolerance::default()).is_err());
    }
}

// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Perceptual image comparison based on per-pixel CIEDE2000.

use crate::color::Rgba8;
use crate::image::RgbaImage;
use std::fmt;

/// The per-pixel ΔE above which a pixel is a *strong* difference
/// ([`DiffTolerance::strong_delta_e`]).
///
/// Measured (GUP-687, 2026-11-09): in nine Platforms runs (2026-11-08 to
/// 2026-11-09, 49 to 55 tour, tutorial and gallery goldens each) no pixel
/// differed from its lavapipe golden by more than ΔE 2.7 on Windows WARP or
/// 1.2 on macOS Metal; on Linux lavapipe all 117 goldens are byte-equal to
/// their renders. A pixel moved by one antialiasing coverage level (±8 per
/// channel) is at most ΔE 3.0. The GUP-676 hover ring's 52 ink pixels
/// differed from the old goldens by more than ΔE 40 each (max 93.2).
pub const STRONG_DELTA_E: f32 = 12.0;

/// How many strong pixels a match may have
/// ([`DiffTolerance::max_strong_pixels`]).
///
/// From GUP-687's rule `max(2 * M, 8)` rounded up to a multiple of 4, where
/// `M` is the largest strong-pixel count seen on any platform against the
/// checked-in goldens: `M` = 0 on Linux lavapipe, Windows WARP and macOS
/// Metal (no pixel above ΔE 2.7), so 8. The GUP-676 hover ring alone has 52.
pub const MAX_STRONG_PIXELS: usize = 8;

/// How different two images may be and still "match".
///
/// Two rules, both of which must hold:
///
/// - **Fraction**: a pixel differs when its CIEDE2000 distance exceeds
///   [`pixel_delta_e`](Self::pixel_delta_e); no more than
///   [`max_differing_fraction`](Self::max_differing_fraction) of pixels may
///   differ. This absorbs antialiasing and glyph noise across backends.
/// - **Strong count**: no more than
///   [`max_strong_pixels`](Self::max_strong_pixels) pixels may exceed
///   [`strong_delta_e`](Self::strong_delta_e). This catches a thin,
///   high-contrast feature (a 1 px rule or ring) that is too small in count
///   for the fraction rule.
///
/// Byte-exact comparison is too brittle across GPU backends; the
/// structural checks carry the correctness burden, this catches change.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct DiffTolerance {
    /// Per-pixel CIEDE2000 threshold.
    pub pixel_delta_e: f32,
    /// Maximum fraction (0–1) of pixels allowed to exceed the threshold.
    pub max_differing_fraction: f64,
    /// CIEDE2000 distance above which a pixel is a strong difference.
    pub strong_delta_e: f32,
    /// Maximum number of pixels allowed to exceed `strong_delta_e`.
    pub max_strong_pixels: usize,
}

impl Default for DiffTolerance {
    fn default() -> Self {
        Self {
            pixel_delta_e: 3.0,
            max_differing_fraction: 0.0025,
            strong_delta_e: STRONG_DELTA_E,
            max_strong_pixels: MAX_STRONG_PIXELS,
        }
    }
}

/// Summary statistics from comparing two equally sized images.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct DiffStats {
    /// Pixels whose ΔE exceeds the tolerance threshold.
    pub differing_pixels: usize,
    /// Pixels whose ΔE exceeds the tolerance's strong threshold.
    pub strong_pixels: usize,
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

    /// Whether these statistics are within `tol`: both the fraction rule
    /// and the strong-count rule hold.
    pub fn within(&self, tol: &DiffTolerance) -> bool {
        self.within_fraction(tol) && self.within_strong(tol)
    }

    /// Whether the fraction rule alone holds.
    pub fn within_fraction(&self, tol: &DiffTolerance) -> bool {
        self.differing_fraction() <= tol.max_differing_fraction
    }

    /// Whether the strong-count rule alone holds.
    pub fn within_strong(&self, tol: &DiffTolerance) -> bool {
        self.strong_pixels <= tol.max_strong_pixels
    }

    /// Which rules these statistics break, in words, or `None` when they
    /// are [`within`](Self::within) `tol`.
    pub fn rejection(&self, tol: &DiffTolerance) -> Option<String> {
        let fraction = format!(
            "{} pixels over ΔE {} ({:.3}%, max {:.2}%)",
            self.differing_pixels,
            tol.pixel_delta_e,
            self.differing_fraction() * 100.0,
            tol.max_differing_fraction * 100.0
        );
        let strong = format!(
            "{} pixels over ΔE {} (max {})",
            self.strong_pixels, tol.strong_delta_e, tol.max_strong_pixels
        );
        match (self.within_fraction(tol), self.within_strong(tol)) {
            (true, true) => None,
            (false, true) => Some(format!("fraction rule broken: {fraction}")),
            (false, false) => Some(format!(
                "fraction rule broken: {fraction}; strong rule broken: {strong}"
            )),
            (true, false) => Some(format!(
                "strong rule broken: {strong}; the fraction rule alone would have passed ({fraction})"
            )),
        }
    }
}

impl fmt::Display for DiffStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} of {} pixels differ ({:.3}%), {} strongly, max ΔE {:.1}, mean ΔE {:.3}",
            self.differing_pixels,
            self.total_pixels,
            self.differing_fraction() * 100.0,
            self.strong_pixels,
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
    let mut strong = 0usize;
    let mut max = 0f32;
    let mut sum = 0f64;
    for ((_, _, e), (_, _, a)) in expected.pixels().zip(actual.pixels()) {
        let d = e.delta_e(a);
        if d > tol.pixel_delta_e {
            differing += 1;
        }
        if d > tol.strong_delta_e {
            strong += 1;
        }
        max = max.max(d);
        sum += d as f64;
    }
    let total = expected.pixel_count();
    Ok(DiffStats {
        differing_pixels: differing,
        strong_pixels: strong,
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

/// Synthetic scenes for the strong-rule tests here and in `golden.rs`.
#[cfg(test)]
pub(crate) mod scenes {
    use crate::color::Rgba8;
    use crate::image::RgbaImage;
    use crate::layout::PxRect;

    pub const W: u32 = 640;
    pub const H: u32 = 400;
    /// Where [`add_ring`] centres the ring.
    pub const RING_CENTRE: (u32, u32) = (320, 200);
    /// The ring's edge colour: light grey.
    pub const EDGE: Rgba8 = Rgba8::rgb(0xdd, 0xdd, 0xdd);

    /// Discs (centre x, centre y, radius, colour) with antialiased edges.
    const DISCS: [(f32, f32, f32, u32); 6] = [
        (100.0, 100.0, 9.5, 0x0072b2),
        (220.0, 300.0, 7.0, 0xd55e00),
        (400.0, 120.0, 11.3, 0x0072b2),
        (520.0, 260.0, 6.4, 0xd55e00),
        (250.0, 180.0, 4.0, 0x0072b2),
        (580.0, 80.0, 8.8, 0x009e73),
    ];

    /// Coverage of pixel `(x, y)` by disc `d`, quantised to 1/32.
    fn coverage(d: (f32, f32, f32, u32), x: u32, y: u32) -> f32 {
        let (cx, cy, r, _) = d;
        let dist = ((x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2)).sqrt();
        ((r + 0.5 - dist).clamp(0.0, 1.0) * 32.0).round() / 32.0
    }

    fn blend(c: Rgba8, cov: f32) -> Rgba8 {
        let mix = |v: u8| (v as f32 * cov + 255.0 * (1.0 - cov)).round() as u8;
        Rgba8::rgb(mix(c.r), mix(c.g), mix(c.b))
    }

    /// A white 640 by 400 render with a few antialiased discs; returns the
    /// image and its partially covered (edge) pixels.
    pub fn plain() -> (RgbaImage, Vec<(u32, u32)>) {
        let mut img = RgbaImage::filled(W, H, Rgba8::WHITE);
        let mut edges = Vec::new();
        for y in 0..H {
            for x in 0..W {
                for d in DISCS {
                    let cov = coverage(d, x, y);
                    if cov > 0.0 {
                        img.set_pixel(x, y, blend(Rgba8::from_hex(d.3), cov));
                        if cov < 1.0 {
                            edges.push((x, y));
                        }
                    }
                }
            }
        }
        (img, edges)
    }

    /// Draw the GUP-676 hover ring (`ring_item` with outer 16, ink 1) at
    /// [`RING_CENTRE`]: a 16 px square of 1 px light-grey sides around a
    /// 14 px square of 1 px black sides. Returns the ring's pixel count.
    pub fn add_ring(img: &mut RgbaImage) -> usize {
        let (outer, ink_width) = (16.0f32, 1.0f32);
        let l = RING_CENTRE.0 as f32 - outer / 2.0;
        let t = RING_CENTRE.1 as f32 - outer / 2.0;
        let inner = outer - 2.0;
        let side = inner - 2.0 * ink_width;
        let far = outer - 1.0;
        let (edge, ink) = (EDGE, Rgba8::BLACK);
        let rects = [
            (l, t, outer, 1.0, edge),
            (l, t + far, outer, 1.0, edge),
            (l, t + 1.0, 1.0, inner, edge),
            (l + far, t + 1.0, 1.0, inner, edge),
            (l + 1.0, t + 1.0, inner, ink_width, ink),
            (l + 1.0, t + far - ink_width, inner, ink_width, ink),
            (l + 1.0, t + 1.0 + ink_width, ink_width, side, ink),
            (
                l + far - ink_width,
                t + 1.0 + ink_width,
                ink_width,
                side,
                ink,
            ),
        ];
        for (x, y, w, h, c) in rects {
            img.fill_rect(PxRect::new(x, y, w, h), c);
        }
        ring_pixels().len()
    }

    /// The ring's pixels: the borders of the 16 px box (edge, 60 pixels)
    /// and of the 14 px box inside it (ink, 52 pixels).
    pub fn ring_pixels() -> Vec<(u32, u32)> {
        let (cx, cy) = RING_CENTRE;
        let mut px = Vec::new();
        for inset in [0u32, 1] {
            let (l, t) = (cx - 8 + inset, cy - 8 + inset);
            let n = 16 - 2 * inset;
            for y in t..t + n {
                for x in l..l + n {
                    if x == l || y == t || x == l + n - 1 || y == t + n - 1 {
                        px.push((x, y));
                    }
                }
            }
        }
        px
    }

    /// Move every listed pixel by one level of coverage: alternately +8 and
    /// -8 on every channel (saturating), the kind of difference two
    /// rasterisers make on an antialiased edge.
    pub fn jitter(img: &RgbaImage, pixels: &[(u32, u32)]) -> RgbaImage {
        let mut out = img.clone();
        for (i, &(x, y)) in pixels.iter().enumerate() {
            let p = img.pixel(x, y);
            let m = |v: u8| {
                if i % 2 == 0 {
                    v.saturating_add(8)
                } else {
                    v.saturating_sub(8)
                }
            };
            out.set_pixel(x, y, Rgba8::rgb(m(p.r), m(p.g), m(p.b)));
        }
        out
    }
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

    #[test]
    fn a_thin_ring_is_a_mismatch_though_the_fraction_rule_passes() {
        let tol = DiffTolerance::default();
        let (plain, _) = scenes::plain();
        let same = perceptual_diff(&plain, &plain, &tol).unwrap();
        assert!(same.within(&tol), "{same}");

        let mut ringed = plain.clone();
        let ring = scenes::add_ring(&mut ringed);
        assert_eq!(ring, 60 + 52);
        let stats = perceptual_diff(&plain, &ringed, &tol).unwrap();
        eprintln!("ring added: {stats}");
        assert!(
            stats.differing_pixels <= ring,
            "only the ring changed: {stats}"
        );
        // The fraction rule alone misses it (112 of 256,000 pixels)...
        assert!(stats.within_fraction(&tol), "{stats}");
        // ...but its 52 black ink pixels are strong differences.
        assert!(stats.strong_pixels >= 52, "{stats}");
        assert!(!stats.within(&tol), "{stats}");
        let why = stats.rejection(&tol).unwrap();
        assert!(why.contains("strong rule broken"), "{why}");
        assert!(
            why.contains("fraction rule alone would have passed"),
            "{why}"
        );
    }

    #[test]
    fn antialiasing_noise_on_every_edge_pixel_still_matches() {
        let tol = DiffTolerance::default();
        let (plain, edges) = scenes::plain();
        assert!(edges.len() > 200, "the scene has edges: {}", edges.len());
        let noisy = scenes::jitter(&plain, &edges);
        let stats = perceptual_diff(&plain, &noisy, &tol).unwrap();
        eprintln!("{} edge pixels moved: {stats}", edges.len());
        assert_eq!(stats.strong_pixels, 0, "{stats}");
        assert!(stats.within(&tol), "{stats}");

        // A ring in both renders, with its pixels and every disc edge moved
        // by one coverage level, also matches.
        let mut ringed = plain.clone();
        scenes::add_ring(&mut ringed);
        let mut moved = edges;
        moved.extend(scenes::ring_pixels());
        let noisy = scenes::jitter(&ringed, &moved);
        let stats = perceptual_diff(&ringed, &noisy, &tol).unwrap();
        eprintln!("{} edge and ring pixels moved: {stats}", moved.len());
        assert_eq!(stats.strong_pixels, 0, "{stats}");
        assert!(stats.within(&tol), "{stats}");
    }

    #[test]
    fn rejection_names_the_broken_rules() {
        let tol = DiffTolerance::default();
        let stats = |differing, strong| DiffStats {
            differing_pixels: differing,
            strong_pixels: strong,
            total_pixels: 10_000,
            max_delta_e: 50.0,
            mean_delta_e: 1.0,
        };
        assert_eq!(stats(25, 8).rejection(&tol), None);
        let fraction = stats(26, 0).rejection(&tol).unwrap();
        assert!(
            fraction.starts_with("fraction rule broken: 26 pixels"),
            "{fraction}"
        );
        assert!(!fraction.contains("strong rule"), "{fraction}");
        let both = stats(26, 9).rejection(&tol).unwrap();
        assert!(
            both.contains("strong rule broken: 9 pixels over ΔE 12 (max 8)"),
            "{both}"
        );
        let strong = stats(25, 9).rejection(&tol).unwrap();
        assert!(strong.contains("would have passed"), "{strong}");
    }
}

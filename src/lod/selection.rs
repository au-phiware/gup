// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! LOD level selection heuristic.
//!
//! Provides a pure function that chooses the coarsest pyramid level whose
//! on-screen point density does not exceed a configurable threshold.

use crate::mark::batch_renderer::Viewport2D;

/// Default maximum on-screen point density (points per pixel).
///
/// When the estimated density for a given level exceeds this threshold the
/// selector falls back to the next coarser level.
pub const DEFAULT_MAX_DENSITY: f32 = 4.0;

/// Select the appropriate LOD level for the current viewport.
///
/// # Algorithm
///
/// The heuristic estimates on-screen point density for each level and returns
/// the coarsest level whose density does not exceed `max_density` (default
/// `DEFAULT_MAX_DENSITY`).
///
/// Density for a level is computed as:
///
/// ```text
/// density = estimated_points_at_level / viewport_pixel_area
/// ```
///
/// where `estimated_points_at_level` is derived by assuming each level reduces
/// the point count by a factor of 4 from the previous level.
///
/// The function walks from the finest level (0) towards the coarsest. Level 0
/// is tried first. If its density is below the threshold it is returned
/// immediately (no need to coarsen). Otherwise the walk continues to coarser
/// levels until one satisfies the threshold. If no level does, the coarsest
/// level is returned.
///
/// # Parameters
///
/// - `viewport`: The current viewport (pixel dimensions are used).
/// - `point_count`: Total number of source points at level 0.
/// - `levels`: Total number of levels in the pyramid.
///
/// # Returns
///
/// The zero-based level index to use for rendering.
///
/// # Examples
///
/// ```
/// use gup::mark::batch_renderer::Viewport2D;
/// use gup::lod::select_lod_level;
///
/// let vp = Viewport2D { pixel_width: 1920.0, pixel_height: 1080.0, ..Default::default() };
/// let level = select_lod_level(&vp, 1_000_000, 5);
/// assert!(level < 5);
/// ```
pub fn select_lod_level(viewport: &Viewport2D, point_count: u64, levels: usize) -> usize {
    select_lod_level_with_density(viewport, point_count, levels, DEFAULT_MAX_DENSITY)
}

/// Like [`select_lod_level`] but with a configurable maximum density.
pub fn select_lod_level_with_density(
    viewport: &Viewport2D,
    point_count: u64,
    levels: usize,
    max_density: f32,
) -> usize {
    if levels <= 1 {
        return 0;
    }

    let pixel_area = (viewport.pixel_width * viewport.pixel_height).max(1.0);

    // Each level reduces by approximately 4× (the default reduction factor).
    // We use a geometric reduction model: points_at_level(i) ≈ point_count / 4^i.
    let reduction = 4.0_f64;

    // Walk from finest (level 0) to coarsest. Return the first level whose
    // density is at or below the threshold. This ensures we never discard
    // data when the finer level is already within budget.
    for level in 0..levels {
        let estimated_points = (point_count as f64) / reduction.powi(level as i32);
        let density = estimated_points as f32 / pixel_area;

        if density <= max_density {
            return level;
        }
    }

    // All levels exceed the threshold — use the coarsest available.
    levels - 1
}

#[cfg(test)]
mod tests {
    use super::*;

    fn viewport(w: f32, h: f32) -> Viewport2D {
        Viewport2D {
            pixel_width: w,
            pixel_height: h,
            ..Default::default()
        }
    }

    #[test]
    fn single_level_pyramid_returns_zero() {
        let vp = viewport(1920.0, 1080.0);
        assert_eq!(select_lod_level(&vp, 1_000_000, 1), 0);
    }

    #[test]
    fn zero_levels_returns_zero() {
        let vp = viewport(1920.0, 1080.0);
        assert_eq!(select_lod_level(&vp, 1_000_000, 0), 0);
    }

    #[test]
    fn small_dataset_returns_level_zero() {
        // 100 points on a 1920×1080 screen — density is well below threshold.
        let vp = viewport(1920.0, 1080.0);
        assert_eq!(select_lod_level(&vp, 100, 5), 0);
    }

    #[test]
    fn large_dataset_selects_coarser_level() {
        // 100M points — should select a coarser level than 0.
        let vp = viewport(1920.0, 1080.0);
        let level = select_lod_level(&vp, 100_000_000, 5);
        assert!(
            level > 0,
            "Expected coarser level for 100M points, got {}",
            level
        );
    }

    #[test]
    fn fully_zoomed_in_prefers_fine_level() {
        // Very large viewport (simulating extreme zoom-in) — density is low.
        let vp = viewport(100_000.0, 100_000.0);
        let level = select_lod_level(&vp, 1_000_000, 5);
        assert_eq!(level, 0, "Zoomed-in viewport should use finest level");
    }

    #[test]
    fn fully_zoomed_out_prefers_coarse_level() {
        // Tiny viewport (simulating extreme zoom-out).
        let vp = viewport(100.0, 100.0);
        let level = select_lod_level(&vp, 100_000_000, 5);
        assert!(
            level >= 2,
            "Zoomed-out viewport should use coarse level, got {}",
            level
        );
    }

    #[test]
    fn monotonically_increasing_with_point_count() {
        let vp = viewport(1920.0, 1080.0);
        let mut prev_level = 0;
        for &count in &[1_000u64, 100_000, 1_000_000, 100_000_000, 1_000_000_000] {
            let level = select_lod_level(&vp, count, 5);
            assert!(
                level >= prev_level,
                "Level should not decrease as point count grows: {} < {} at count={}",
                level,
                prev_level,
                count,
            );
            prev_level = level;
        }
    }

    #[test]
    fn custom_density_threshold() {
        let vp = viewport(1920.0, 1080.0);
        // Very strict threshold — should push to coarser levels.
        let strict = select_lod_level_with_density(&vp, 10_000_000, 5, 0.5);
        // Lenient threshold — should stay at finer levels.
        let lenient = select_lod_level_with_density(&vp, 10_000_000, 5, 100.0);
        assert!(
            strict >= lenient,
            "Strict density should select coarser level: strict={}, lenient={}",
            strict,
            lenient
        );
    }
}

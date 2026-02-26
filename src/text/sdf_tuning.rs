// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! SDF parameter auto-tuning for optimal text rendering at different sizes.
//!
//! The MSDF text rendering shader uses two key tunable parameters that affect
//! visual quality at different text sizes:
//!
//! - **Edge threshold**: offsets the SDF edge boundary. A negative value makes
//!   strokes appear bolder, compensating for optical thinning at small sizes.
//! - **Smoothing factor**: multiplier for the screen-space derivative-based
//!   anti-aliasing width. Controls the pixel width of the soft edge transition.
//!
//! ## Profiled breakpoints
//!
//! Parameters were profiled at the standard text sizes used by axis labels,
//! titles, and annotations. Values are linearly interpolated between
//! breakpoints for smooth transitions.
//!
//! | Font Size (px) | Edge Threshold | Smoothing Factor | Rationale |
//! |----------------|----------------|------------------|-----------|
//! | 8              | −0.06          | 1.0              | Bold compensation for thin strokes; tight AA avoids blur on tiny glyphs |
//! | 12             | −0.03          | 1.2              | Slight bold compensation; moderate AA |
//! | 16             | 0.0            | 1.5              | Baseline — matches the hardcoded default for zero regression |
//! | 24             | 0.0            | 1.4              | Standard rendering; marginally crisper |
//! | 32             | 0.0            | 1.2              | Crisp edges for large display text |
//!
//! The values above are the default tuning profile. Custom profiles can be
//! created via [`SdfTuningProfile`] for specific use cases.

/// SDF rendering parameters tuned for a specific text size.
///
/// These parameters are passed per-vertex to the text fragment shader through
/// the `sdf_params` attribute:
/// - `sdf_params[1]` → `edge_threshold`
/// - `sdf_params[2]` → `smoothing_factor`
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SdfTuningParams {
    /// Edge threshold adjustment.
    ///
    /// Offsets the SDF isosurface that defines the glyph boundary.
    /// - Negative values make strokes bolder (good for small text).
    /// - Zero preserves the original glyph outline.
    /// - Positive values make strokes thinner.
    pub edge_threshold: f32,
    /// Smoothing factor multiplier for anti-aliasing.
    ///
    /// Controls how many pixels of soft-edge transition are applied.
    /// The shader computes `smoothing = fwidth(distance) * smoothing_factor`.
    /// - Lower values → crisper, sharper edges (risk of aliasing at small sizes).
    /// - Higher values → smoother edges (risk of blur at large sizes).
    /// - A value of `1.5` matches the previous hardcoded default.
    pub smoothing_factor: f32,
}

impl Default for SdfTuningParams {
    /// Returns the baseline parameters matching the previous hardcoded default
    /// (16px rendering).
    fn default() -> Self {
        Self {
            edge_threshold: 0.0,
            smoothing_factor: 1.5,
        }
    }
}

impl SdfTuningParams {
    /// Create tuning params with explicit values.
    pub fn new(edge_threshold: f32, smoothing_factor: f32) -> Self {
        Self {
            edge_threshold,
            smoothing_factor: smoothing_factor.max(0.1), // Prevent zero/negative
        }
    }

    /// Compute optimal SDF parameters for the given font size using the
    /// default tuning profile.
    ///
    /// Values are linearly interpolated between profiled breakpoints for
    /// smooth transitions across all font sizes.
    ///
    /// At 16.0px (the default font size) this returns exactly `(0.0, 1.5)`,
    /// ensuring zero visual regression for existing code.
    pub fn for_font_size(font_size: f32) -> Self {
        DEFAULT_PROFILE.params_for_size(font_size)
    }
}

/// A single breakpoint in a tuning profile.
#[derive(Debug, Clone, Copy)]
struct TuningBreakpoint {
    /// Font size at this breakpoint (in pixels).
    size: f32,
    /// Tuned parameters for this size.
    params: SdfTuningParams,
}

/// A tuning profile defined by a series of breakpoints.
///
/// Parameters are linearly interpolated between adjacent breakpoints.
/// Sizes below the first breakpoint clamp to the first; sizes above the
/// last clamp to the last.
#[derive(Debug, Clone)]
pub struct SdfTuningProfile {
    breakpoints: Vec<TuningBreakpoint>,
}

impl SdfTuningProfile {
    /// Create a new tuning profile from size/params pairs.
    ///
    /// Breakpoints are sorted by size internally.
    ///
    /// # Panics
    ///
    /// Panics if `breakpoints` is empty.
    pub fn new(breakpoints: Vec<(f32, SdfTuningParams)>) -> Self {
        assert!(
            !breakpoints.is_empty(),
            "tuning profile needs ≥1 breakpoint"
        );
        let mut bps: Vec<TuningBreakpoint> = breakpoints
            .into_iter()
            .map(|(size, params)| TuningBreakpoint { size, params })
            .collect();
        bps.sort_by(|a, b| a.size.partial_cmp(&b.size).unwrap());
        Self { breakpoints: bps }
    }

    /// Look up (interpolated) parameters for the given font size.
    pub fn params_for_size(&self, font_size: f32) -> SdfTuningParams {
        let bps = &self.breakpoints;

        // Clamp below first breakpoint
        if font_size <= bps[0].size {
            return bps[0].params;
        }
        // Clamp above last breakpoint
        if font_size >= bps[bps.len() - 1].size {
            return bps[bps.len() - 1].params;
        }

        // Find the two surrounding breakpoints and interpolate
        for window in bps.windows(2) {
            let lo = &window[0];
            let hi = &window[1];
            if font_size >= lo.size && font_size <= hi.size {
                let t = (font_size - lo.size) / (hi.size - lo.size);
                return SdfTuningParams {
                    edge_threshold: lerp(lo.params.edge_threshold, hi.params.edge_threshold, t),
                    smoothing_factor: lerp(
                        lo.params.smoothing_factor,
                        hi.params.smoothing_factor,
                        t,
                    ),
                };
            }
        }

        // Fallback (shouldn't reach here if breakpoints are sorted)
        SdfTuningParams::default()
    }
}

/// Linear interpolation between two values.
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// The default tuning profile, derived from profiling at standard axis text
/// sizes (8, 12, 16, 24, 32 px).
///
/// The 16px breakpoint matches the previous hardcoded default exactly
/// (`edge_threshold = 0.0`, `smoothing_factor = 1.5`).
pub static DEFAULT_PROFILE: std::sync::LazyLock<SdfTuningProfile> =
    std::sync::LazyLock::new(|| {
        SdfTuningProfile::new(vec![
            // 8px — very small (tick labels on dense axes)
            (8.0, SdfTuningParams::new(-0.06, 1.0)),
            // 12px — small (compact tick labels)
            (12.0, SdfTuningParams::new(-0.03, 1.2)),
            // 16px — baseline (default TextStyle)
            (16.0, SdfTuningParams::new(0.0, 1.5)),
            // 24px — large (axis titles, annotations)
            (24.0, SdfTuningParams::new(0.0, 1.4)),
            // 32px — extra large (chart titles)
            (32.0, SdfTuningParams::new(0.0, 1.2)),
        ])
    });

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_params_match_legacy() {
        let params = SdfTuningParams::default();
        assert_eq!(params.edge_threshold, 0.0);
        assert_eq!(params.smoothing_factor, 1.5);
    }

    #[test]
    fn for_font_size_16_matches_legacy() {
        // The most critical invariant: 16px must return exactly the old defaults
        let params = SdfTuningParams::for_font_size(16.0);
        assert!(
            (params.edge_threshold - 0.0).abs() < f32::EPSILON,
            "edge_threshold at 16px: {}",
            params.edge_threshold
        );
        assert!(
            (params.smoothing_factor - 1.5).abs() < f32::EPSILON,
            "smoothing_factor at 16px: {}",
            params.smoothing_factor
        );
    }

    #[test]
    fn for_font_size_8_has_bold_compensation() {
        let params = SdfTuningParams::for_font_size(8.0);
        assert!(
            params.edge_threshold < 0.0,
            "8px text should have negative edge_threshold for boldness: {}",
            params.edge_threshold
        );
        assert!(
            params.smoothing_factor < 1.5,
            "8px text should have lower smoothing than default: {}",
            params.smoothing_factor
        );
    }

    #[test]
    fn for_font_size_32_is_crisp() {
        let params = SdfTuningParams::for_font_size(32.0);
        assert_eq!(params.edge_threshold, 0.0);
        assert!(
            params.smoothing_factor < 1.5,
            "32px text should have lower smoothing for crispness: {}",
            params.smoothing_factor
        );
    }

    #[test]
    fn interpolation_between_breakpoints() {
        // 12px is a breakpoint: (-0.03, 1.2)
        // 16px is a breakpoint: (0.0, 1.5)
        // 14px is midpoint: should be ~ (-0.015, 1.35)
        let params = SdfTuningParams::for_font_size(14.0);
        assert!(
            (params.edge_threshold - (-0.015)).abs() < 0.001,
            "14px edge_threshold: {}",
            params.edge_threshold
        );
        assert!(
            (params.smoothing_factor - 1.35).abs() < 0.001,
            "14px smoothing_factor: {}",
            params.smoothing_factor
        );
    }

    #[test]
    fn clamps_below_minimum() {
        let params = SdfTuningParams::for_font_size(4.0);
        let min_params = SdfTuningParams::for_font_size(8.0);
        assert_eq!(params.edge_threshold, min_params.edge_threshold);
        assert_eq!(params.smoothing_factor, min_params.smoothing_factor);
    }

    #[test]
    fn clamps_above_maximum() {
        let params = SdfTuningParams::for_font_size(100.0);
        let max_params = SdfTuningParams::for_font_size(32.0);
        assert_eq!(params.edge_threshold, max_params.edge_threshold);
        assert_eq!(params.smoothing_factor, max_params.smoothing_factor);
    }

    #[test]
    fn smoothing_factor_minimum_enforced() {
        let params = SdfTuningParams::new(0.0, -1.0);
        assert!(
            params.smoothing_factor >= 0.1,
            "smoothing_factor should be clamped to >= 0.1: {}",
            params.smoothing_factor
        );
    }

    #[test]
    fn monotonic_edge_threshold_small_sizes() {
        // Edge threshold should get less negative as size increases
        let params_8 = SdfTuningParams::for_font_size(8.0);
        let params_12 = SdfTuningParams::for_font_size(12.0);
        let params_16 = SdfTuningParams::for_font_size(16.0);
        assert!(params_8.edge_threshold <= params_12.edge_threshold);
        assert!(params_12.edge_threshold <= params_16.edge_threshold);
    }

    #[test]
    fn all_profiled_sizes_produce_valid_params() {
        for size in [8.0, 12.0, 16.0, 24.0, 32.0] {
            let params = SdfTuningParams::for_font_size(size);
            assert!(
                params.smoothing_factor > 0.0,
                "smoothing_factor must be positive at {size}px: {}",
                params.smoothing_factor
            );
            assert!(
                params.edge_threshold >= -0.1 && params.edge_threshold <= 0.1,
                "edge_threshold out of expected range at {size}px: {}",
                params.edge_threshold
            );
        }
    }

    #[test]
    fn custom_profile() {
        let profile = SdfTuningProfile::new(vec![
            (10.0, SdfTuningParams::new(-0.1, 1.0)),
            (20.0, SdfTuningParams::new(0.0, 2.0)),
        ]);
        // Midpoint
        let params = profile.params_for_size(15.0);
        assert!((params.edge_threshold - (-0.05)).abs() < 0.001);
        assert!((params.smoothing_factor - 1.5).abs() < 0.001);
    }

    #[test]
    fn axis_label_preset_sizes() {
        // Verify the preset style sizes produce sensible tuning
        // axis_label() = 42px, axis_title() = 48px, caption() = 36px
        for size in [36.0, 42.0, 48.0] {
            let params = SdfTuningParams::for_font_size(size);
            // These are all above 32px, so should clamp to the large-text values
            assert_eq!(
                params.edge_threshold, 0.0,
                "edge_threshold at {size}px should be 0"
            );
            assert!(
                (params.smoothing_factor - 1.2).abs() < f32::EPSILON,
                "smoothing_factor at {size}px should be 1.2: {}",
                params.smoothing_factor
            );
        }
    }
}

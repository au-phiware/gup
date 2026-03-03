// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Adaptive renderer for LOD-driven viewport rendering.
//!
//! Selects the coarsest LOD tier that provides sufficient screen density,
//! culls off-screen points, and manages smooth cross-fade transitions between
//! tiers.

use crate::lod::LodPyramid;
use crate::mark::batch_renderer::Viewport2D;

use super::blend::LodBlendState;
use super::debug_overlay::{DebugOverlay, DebugOverlayInfo};
use super::viewport::AdaptiveViewport;

/// Configuration for [`AdaptiveRenderer`].
#[derive(Debug, Clone)]
pub struct AdaptiveRendererConfig {
    /// Number of frames over which to blend LOD transitions.
    ///
    /// Set to 0 for instant switching (no blending).
    /// Default: 8.
    pub blend_frames: u32,

    /// Configurable density threshold multiplier for LOD selection.
    ///
    /// Higher values prefer finer detail (more points rendered); lower values
    /// prefer coarser tiers (faster rendering). Default: 1.0.
    pub heuristic_scale: f32,
}

impl Default for AdaptiveRendererConfig {
    fn default() -> Self {
        Self {
            blend_frames: 8,
            heuristic_scale: 1.0,
        }
    }
}

/// Adaptive viewport renderer that selects LOD tiers and manages transitions.
///
/// Per-frame, the renderer:
/// 1. Selects the coarsest tier whose density yields ≥ 1 pixel per point.
/// 2. Starts a cross-fade blend if the tier changed.
/// 3. Reports culling/debug info via the optional debug overlay.
///
/// The actual GPU dispatch (compute culling + indirect draw) is handled by the
/// caller using the tier index and viewport information exposed by this struct.
/// This keeps `AdaptiveRenderer` testable without requiring a GPU context.
pub struct AdaptiveRenderer {
    /// Current viewport state.
    viewport: AdaptiveViewport,
    /// Blend state for smooth tier transitions.
    blend_state: LodBlendState,
    /// Debug overlay controller.
    debug_overlay: DebugOverlay,
    /// Configuration.
    config: AdaptiveRendererConfig,
    /// Number of levels in the pyramid.
    level_count: usize,
    /// Point counts per level (cached from the pyramid).
    level_point_counts: Vec<usize>,
    /// Data-space bounds from level-0 metadata: [min_x, min_y, max_x, max_y].
    data_bounds: [f32; 4],
}

impl AdaptiveRenderer {
    /// Create a new adaptive renderer for the given pyramid.
    ///
    /// The pyramid metadata is cached; the GPU buffers are not referenced
    /// directly — the caller is responsible for binding the correct tier's
    /// buffer when rendering.
    pub fn new(pyramid: &LodPyramid, config: AdaptiveRendererConfig) -> Self {
        let level_count = pyramid.level_count();
        let level_point_counts: Vec<usize> = (0..level_count)
            .map(|i| pyramid.level_point_count(i))
            .collect();
        let data_bounds = if level_count > 0 {
            pyramid.metadata(0).bounds
        } else {
            [0.0, 0.0, 1.0, 1.0]
        };

        Self {
            viewport: AdaptiveViewport::default(),
            blend_state: LodBlendState::new(level_count.saturating_sub(1), config.blend_frames),
            debug_overlay: DebugOverlay::new(),
            config,
            level_count,
            level_point_counts,
            data_bounds,
        }
    }

    /// Select the LOD tier for the given viewport.
    ///
    /// Uses a pixels-per-data-point heuristic: walks from the finest tier
    /// (level 0, most points) towards the coarsest. The first tier whose
    /// estimated on-screen density is ≤ 1 point per pixel (adjusted by
    /// `heuristic_scale`) is returned. This gives the finest detail that
    /// fits comfortably on screen. If no tier is sparse enough, the coarsest
    /// tier is returned for performance.
    ///
    /// This function is pure — no GPU side effects — for testability.
    pub fn select_tier(&self, viewport: &AdaptiveViewport) -> usize {
        if self.level_count <= 1 {
            return 0;
        }

        let heuristic_scale = viewport.heuristic_scale * self.config.heuristic_scale;

        // Compute the fraction of data space visible in the viewport.
        let vp_bounds = viewport.world_bounds();
        let data_width = (self.data_bounds[2] - self.data_bounds[0]).max(f32::EPSILON);
        let data_height = (self.data_bounds[3] - self.data_bounds[1]).max(f32::EPSILON);

        // Clamp viewport bounds to data extents and compute visible fraction.
        let vis_min_x = vp_bounds[0].max(self.data_bounds[0]);
        let vis_min_y = vp_bounds[1].max(self.data_bounds[1]);
        let vis_max_x = vp_bounds[2].min(self.data_bounds[2]);
        let vis_max_y = vp_bounds[3].min(self.data_bounds[3]);

        let vis_width = (vis_max_x - vis_min_x).max(0.0);
        let vis_height = (vis_max_y - vis_min_y).max(0.0);

        let visible_fraction = (vis_width / data_width) * (vis_height / data_height);
        if visible_fraction <= 0.0 {
            // Nothing visible — return coarsest tier.
            return self.level_count - 1;
        }

        let pixel_area = viewport.pixel_area();
        // Maximum acceptable on-screen density (points per pixel).
        let max_density = 1.0 / heuristic_scale.max(f32::EPSILON);

        // Walk from finest (0) towards coarsest. The first tier whose
        // estimated visible density is within budget is returned — this is
        // the finest tier that avoids sub-pixel overdraw.
        for tier in 0..self.level_count {
            let tier_points = self.level_point_counts[tier] as f32;
            let estimated_visible = tier_points * visible_fraction;
            let density = estimated_visible / pixel_area;

            if density <= max_density {
                return tier;
            }
        }

        // All tiers exceed density — use coarsest for performance.
        self.level_count - 1
    }

    /// Advance one frame: select tier, update blend state, and collect debug info.
    ///
    /// Returns the tier index to render this frame, the blend alpha, and
    /// optionally the previous tier to cross-fade from.
    pub fn update(&mut self, viewport: &AdaptiveViewport) -> FrameState {
        self.viewport = *viewport;

        let target_tier = self.select_tier(viewport);
        self.blend_state.transition_to(target_tier);
        let alpha = self.blend_state.tick();

        let from_tier = if self.blend_state.is_transitioning() || alpha < 1.0 {
            Some(self.blend_state.from_tier())
        } else {
            None
        };

        // Collect debug info.
        let selected_tier = self.blend_state.active_tier();
        let total_in_tier = self
            .level_point_counts
            .get(selected_tier)
            .copied()
            .unwrap_or(0) as u32;

        self.debug_overlay.update(DebugOverlayInfo {
            tier_index: selected_tier,
            tier_count: self.level_count,
            visible_points: 0, // Updated after culling.
            total_points_in_tier: total_in_tier,
            blending: self.blend_state.is_transitioning(),
            blend_alpha: alpha,
        });

        FrameState {
            tier: selected_tier,
            alpha,
            blend_from_tier: from_tier,
        }
    }

    /// Update the visible-point count in the debug overlay after culling.
    pub fn set_visible_count(&mut self, count: u32) {
        if let Some(info) = self.debug_overlay.info().copied() {
            self.debug_overlay.update(DebugOverlayInfo {
                visible_points: count,
                ..info
            });
        }
    }

    /// Get the selected tier index (after the most recent [`update`](Self::update)).
    pub fn selected_tier(&self) -> usize {
        self.blend_state.active_tier()
    }

    /// Get a reference to the blend state.
    pub fn blend_state(&self) -> &LodBlendState {
        &self.blend_state
    }

    /// Enable or disable the debug overlay.
    pub fn set_debug_overlay(&mut self, enabled: bool) {
        self.debug_overlay.set_enabled(enabled);
    }

    /// Whether the debug overlay is enabled.
    pub fn debug_overlay_enabled(&self) -> bool {
        self.debug_overlay.is_enabled()
    }

    /// Get the debug overlay (for reading current frame info).
    pub fn debug_overlay(&self) -> &DebugOverlay {
        &self.debug_overlay
    }

    /// Get the current viewport.
    pub fn viewport(&self) -> &AdaptiveViewport {
        &self.viewport
    }

    /// Convert the current adaptive viewport to a `Viewport2D` for use with
    /// the compute instance filter.
    pub fn viewport_2d(&self) -> Viewport2D {
        self.viewport.to_viewport_2d()
    }

    /// Get the number of levels.
    pub fn level_count(&self) -> usize {
        self.level_count
    }

    /// Point count at the given level.
    pub fn level_point_count(&self, level: usize) -> usize {
        self.level_point_counts.get(level).copied().unwrap_or(0)
    }

    /// Get the configuration.
    pub fn config(&self) -> &AdaptiveRendererConfig {
        &self.config
    }

    /// The data-space bounds from level 0.
    pub fn data_bounds(&self) -> [f32; 4] {
        self.data_bounds
    }
}

/// Per-frame rendering state returned by [`AdaptiveRenderer::update`].
#[derive(Debug, Clone, Copy)]
pub struct FrameState {
    /// The LOD tier to render.
    pub tier: usize,
    /// Blend alpha for the incoming tier (0.0 .. 1.0).
    ///
    /// The outgoing tier (if blending) should be drawn at `1.0 - alpha`.
    pub alpha: f32,
    /// If a blend transition is active, the previous tier to cross-fade from.
    ///
    /// `None` when no transition is in progress.
    pub blend_from_tier: Option<usize>,
}

impl std::fmt::Debug for AdaptiveRenderer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AdaptiveRenderer")
            .field("level_count", &self.level_count)
            .field("selected_tier", &self.blend_state.active_tier())
            .field("blending", &self.blend_state.is_transitioning())
            .field("config", &self.config)
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Helper: convert pyramid-level point counts to the density model used by
// select_tier.
// ---------------------------------------------------------------------------

impl AdaptiveRenderer {
    /// Construct from raw metadata (for testing without a real LodPyramid).
    #[cfg(test)]
    pub(crate) fn from_metadata(
        level_point_counts: Vec<usize>,
        data_bounds: [f32; 4],
        config: AdaptiveRendererConfig,
    ) -> Self {
        Self::from_metadata_for_bench(level_point_counts, data_bounds, config)
    }

    /// Construct from raw metadata for benchmarking.
    ///
    /// Creates an `AdaptiveRenderer` without requiring a real `LodPyramid`.
    /// The caller supplies the point counts and data bounds directly.
    pub fn from_metadata_for_bench(
        level_point_counts: Vec<usize>,
        data_bounds: [f32; 4],
        config: AdaptiveRendererConfig,
    ) -> Self {
        let level_count = level_point_counts.len();
        Self {
            viewport: AdaptiveViewport::default(),
            blend_state: LodBlendState::new(level_count.saturating_sub(1), config.blend_frames),
            debug_overlay: DebugOverlay::new(),
            config,
            level_count,
            level_point_counts,
            data_bounds,
        }
    }
}

/// Create `AdaptiveRenderer` from a `LodPyramid` with default config.
impl From<&LodPyramid> for AdaptiveRenderer {
    fn from(pyramid: &LodPyramid) -> Self {
        Self::new(pyramid, AdaptiveRendererConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Simulated pyramid: 5 levels with 4× reduction per level.
    /// Level 0: 10M, Level 1: 2.5M, Level 2: 625K, Level 3: 156K, Level 4: 39K
    fn test_renderer() -> AdaptiveRenderer {
        let counts = vec![10_000_000, 2_500_000, 625_000, 156_250, 39_062];
        AdaptiveRenderer::from_metadata(
            counts,
            [0.0, 0.0, 100.0, 100.0],
            AdaptiveRendererConfig {
                blend_frames: 8,
                heuristic_scale: 1.0,
            },
        )
    }

    // --- AC1: LOD Tier Selection Algorithm tests ---

    #[test]
    fn ac1_maximum_zoom_in_selects_finest_tier() {
        let renderer = test_renderer();
        // Extreme zoom-in: each world unit covers 10000 pixels.
        // Viewport sees 0.192 × 0.108 world units (tiny fraction of 100×100).
        // visible_fraction ≈ 0.000002 → very few visible points at any tier.
        // Level 0: 10M × 2e-6 ≈ 20.7 pts → density = 20.7/2M ≈ 0.00001
        // The finest tier easily fits → selected for best detail.
        let vp = AdaptiveViewport::new(10000.0, [50.0, 50.0], [1920, 1080]);
        let tier = renderer.select_tier(&vp);
        assert_eq!(
            tier, 0,
            "Extreme zoom-in should select finest tier (all fit), got {tier}"
        );
    }

    #[test]
    fn ac1_maximum_zoom_out_selects_coarsest_tier() {
        let renderer = test_renderer();
        // Maximum zoom-out: entire data visible on small screen.
        // zoom=1, screen=200×200, data 100×100 → fraction = 1.0
        // pixel_area = 40000
        // Level 0: 10M / 40K = 250 → FAILS (way too dense)
        // Level 1: 2.5M / 40K = 62.5 → FAILS
        // Level 2: 625K / 40K = 15.6 → FAILS
        // Level 3: 156K / 40K = 3.9 → FAILS
        // Level 4: 39K / 40K = 0.98 → OK
        // Fallback to coarsest.
        let vp = AdaptiveViewport::new(1.0, [50.0, 50.0], [200, 200]);
        let tier = renderer.select_tier(&vp);
        assert_eq!(
            tier, 4,
            "Maximum zoom-out on small screen should select coarsest tier, got {tier}"
        );
    }

    #[test]
    fn ac1_mid_range_zoom() {
        let renderer = test_renderer();
        // Mid-range: entire data visible on 1920×1080 screen.
        // pixel_area = 2073600, fraction = 1.0
        // Level 0: 10M/2M = 4.8 → FAILS (> 1)
        // Level 1: 2.5M/2M = 1.2 → FAILS (> 1)
        // Level 2: 625K/2M = 0.30 → OK → return 2
        let vp = AdaptiveViewport::new(5.0, [50.0, 50.0], [1920, 1080]);
        let tier = renderer.select_tier(&vp);
        assert!(
            tier > 0 && tier < 4,
            "Mid-range zoom should select an intermediate tier, got {tier}"
        );
    }

    #[test]
    fn ac1_sub_region_viewport() {
        let renderer = test_renderer();
        // Viewport covers only a small sub-region of the data extents.
        // zoom=200, center=5,5, screen=1920×1080
        // world: 9.6 × 5.4, visible in data: 0..9.6, 0..5.4
        // fraction ≈ (9.6 × 5.4) / (100 × 100) = 0.005184
        // Level 0: 10M × 0.005184 ≈ 51840 / 2073600 = 0.025 → OK → return 0
        // Zoomed into a small region → finest tier for best detail.
        let vp = AdaptiveViewport::new(200.0, [5.0, 5.0], [1920, 1080]);
        let tier = renderer.select_tier(&vp);
        assert_eq!(
            tier, 0,
            "Sub-region viewport should use finest tier for best detail, got {tier}"
        );
    }

    #[test]
    fn ac1_selected_tier_exposed() {
        let mut renderer = test_renderer();
        let vp = AdaptiveViewport::new(20.0, [50.0, 50.0], [1920, 1080]);
        let frame = renderer.update(&vp);
        assert_eq!(frame.tier, renderer.selected_tier());
    }

    // --- AC3: LodBlendState integration tests ---

    #[test]
    fn ac3_blend_alpha_progression() {
        let mut renderer = AdaptiveRenderer::from_metadata(
            vec![10_000_000, 2_500_000, 625_000, 156_250, 39_062],
            [0.0, 0.0, 100.0, 100.0],
            AdaptiveRendererConfig {
                blend_frames: 4,
                heuristic_scale: 1.0,
            },
        );

        // Force a tier change by switching between very different viewports.
        let zoomed_out = AdaptiveViewport::new(1.0, [50.0, 50.0], [200, 200]);
        let frame1 = renderer.update(&zoomed_out);
        let initial_tier = frame1.tier;

        // Now zoom in significantly to trigger a tier change.
        let zoomed_in = AdaptiveViewport::new(100.0, [50.0, 50.0], [1920, 1080]);
        let frame2 = renderer.update(&zoomed_in);

        if frame2.tier != initial_tier {
            // A transition should have started.
            assert!(
                frame2.alpha < 1.0,
                "Blend alpha should be < 1.0 during transition"
            );
        }
    }

    #[test]
    fn ac3_instant_blend_with_zero_frames() {
        let mut renderer = AdaptiveRenderer::from_metadata(
            vec![10_000_000, 2_500_000, 625_000, 156_250, 39_062],
            [0.0, 0.0, 100.0, 100.0],
            AdaptiveRendererConfig {
                blend_frames: 0,
                heuristic_scale: 1.0,
            },
        );

        let vp1 = AdaptiveViewport::new(1.0, [50.0, 50.0], [200, 200]);
        renderer.update(&vp1);

        let vp2 = AdaptiveViewport::new(10000.0, [50.0, 50.0], [1920, 1080]);
        let frame = renderer.update(&vp2);

        // With blend_frames=0, alpha should be 1.0 immediately.
        assert!(
            (frame.alpha - 1.0).abs() < f32::EPSILON,
            "Instant blend should have alpha=1.0, got {}",
            frame.alpha
        );
        assert!(
            frame.blend_from_tier.is_none(),
            "No blend_from_tier when instant"
        );
    }

    // --- Debug overlay tests ---

    #[test]
    fn debug_overlay_disabled_by_default() {
        let renderer = test_renderer();
        assert!(!renderer.debug_overlay_enabled());
        assert!(renderer.debug_overlay().info().is_none());
    }

    #[test]
    fn debug_overlay_enabled() {
        let mut renderer = test_renderer();
        renderer.set_debug_overlay(true);
        assert!(renderer.debug_overlay_enabled());

        let vp = AdaptiveViewport::new(20.0, [50.0, 50.0], [1920, 1080]);
        renderer.update(&vp);

        let info = renderer.debug_overlay().info().unwrap();
        assert_eq!(info.tier_count, 5);
        assert!(info.total_points_in_tier > 0);
    }

    #[test]
    fn set_visible_count_updates_overlay() {
        let mut renderer = test_renderer();
        renderer.set_debug_overlay(true);

        let vp = AdaptiveViewport::new(20.0, [50.0, 50.0], [1920, 1080]);
        renderer.update(&vp);
        renderer.set_visible_count(42);

        let info = renderer.debug_overlay().info().unwrap();
        assert_eq!(info.visible_points, 42);
    }

    // --- General tests ---

    #[test]
    fn single_level_pyramid() {
        let renderer = AdaptiveRenderer::from_metadata(
            vec![1_000_000],
            [0.0, 0.0, 100.0, 100.0],
            AdaptiveRendererConfig::default(),
        );
        let vp = AdaptiveViewport::default();
        assert_eq!(renderer.select_tier(&vp), 0);
    }

    #[test]
    fn viewport_2d_conversion() {
        let mut renderer = test_renderer();
        let vp = AdaptiveViewport::new(100.0, [50.0, 50.0], [800, 600]);
        renderer.update(&vp);
        let v2d = renderer.viewport_2d();
        assert!((v2d.pixel_width - 800.0).abs() < f32::EPSILON);
        assert!((v2d.pixel_height - 600.0).abs() < f32::EPSILON);
    }
}

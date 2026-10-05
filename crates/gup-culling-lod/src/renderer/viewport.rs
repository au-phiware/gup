// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Viewport type for adaptive rendering with zoom, pan, and screen size.

/// A viewport describing the visible region for adaptive LOD rendering.
///
/// Combines screen resolution (in physical pixels) with a zoom/pan transform
/// that maps from data (world) space into screen space.
///
/// # Coordinate Model
///
/// - **World space**: the data coordinate system (e.g. 0.0 .. 1.0).
/// - **Zoom**: a multiplier; `zoom = 2.0` means each world-space unit covers
///   twice as many pixels.
/// - **Pan**: offset in world space; the viewport centre in world space is
///   `pan`.
/// - **Screen size**: physical pixel dimensions of the render target.
///
/// The viewport's world-space extents are derived from `screen_size`, `zoom`,
/// and `pan`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AdaptiveViewport {
    /// Zoom factor — pixels per world-space unit.
    ///
    /// Must be > 0.0.
    pub zoom: f32,

    /// Pan offset — the world-space coordinate at the viewport centre.
    pub pan: [f32; 2],

    /// Screen resolution in physical pixels: `[width, height]`.
    pub screen_size: [u32; 2],

    /// Configurable heuristic scale factor.
    ///
    /// Multiplier on the density threshold for LOD selection. Higher values
    /// prefer finer detail (more points); lower values prefer coarser tiers.
    /// Default is 1.0.
    pub heuristic_scale: f32,
}

impl Default for AdaptiveViewport {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            pan: [0.0, 0.0],
            screen_size: [1920, 1080],
            heuristic_scale: 1.0,
        }
    }
}

impl AdaptiveViewport {
    /// Create a new viewport.
    pub fn new(zoom: f32, pan: [f32; 2], screen_size: [u32; 2]) -> Self {
        Self {
            zoom: zoom.max(f32::EPSILON),
            pan,
            screen_size,
            heuristic_scale: 1.0,
        }
    }

    /// Pixels per world-space unit along the larger screen axis.
    ///
    /// This is simply `zoom` — by definition `zoom` expresses how many pixels
    /// one world-space unit occupies.
    #[inline]
    pub fn pixels_per_world_unit(&self) -> f32 {
        self.zoom.max(f32::EPSILON)
    }

    /// Width of the visible world-space region (horizontal).
    #[inline]
    pub fn world_width(&self) -> f32 {
        self.screen_size[0] as f32 / self.pixels_per_world_unit()
    }

    /// Height of the visible world-space region (vertical).
    #[inline]
    pub fn world_height(&self) -> f32 {
        self.screen_size[1] as f32 / self.pixels_per_world_unit()
    }

    /// Bounding box of the visible world-space region: `[min_x, min_y, max_x, max_y]`.
    #[inline]
    pub fn world_bounds(&self) -> [f32; 4] {
        let hw = self.world_width() * 0.5;
        let hh = self.world_height() * 0.5;
        [
            self.pan[0] - hw,
            self.pan[1] - hh,
            self.pan[0] + hw,
            self.pan[1] + hh,
        ]
    }

    /// Total screen area in pixels.
    #[inline]
    pub fn pixel_area(&self) -> f32 {
        (self.screen_size[0] as f32 * self.screen_size[1] as f32).max(1.0)
    }

    /// Visible world-space area in square world-space units.
    #[inline]
    pub fn world_area(&self) -> f32 {
        self.world_width() * self.world_height()
    }

    /// Convert to a `Viewport2D` for use with `ComputeInstanceFilter`.
    ///
    /// The min/max fields are set to the world-space bounds and pixel
    /// dimensions are carried through.
    pub fn to_viewport_2d(&self) -> crate::mark::batch_renderer::Viewport2D {
        let bounds = self.world_bounds();
        crate::mark::batch_renderer::Viewport2D {
            min_x: bounds[0],
            max_x: bounds[2],
            min_y: bounds[1],
            max_y: bounds[3],
            pixel_width: self.screen_size[0] as f32,
            pixel_height: self.screen_size[1] as f32,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_viewport() {
        let vp = AdaptiveViewport::default();
        assert_eq!(vp.zoom, 1.0);
        assert_eq!(vp.pan, [0.0, 0.0]);
        assert_eq!(vp.screen_size, [1920, 1080]);
        assert!((vp.pixels_per_world_unit() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn pixels_per_world_unit_is_zoom() {
        let vp = AdaptiveViewport::new(500.0, [0.0, 0.0], [1920, 1080]);
        assert!((vp.pixels_per_world_unit() - 500.0).abs() < f32::EPSILON);
    }

    #[test]
    fn world_bounds_centered() {
        let vp = AdaptiveViewport::new(100.0, [5.0, 5.0], [200, 100]);
        // world_width = 200 / 100 = 2.0, world_height = 100 / 100 = 1.0
        let b = vp.world_bounds();
        assert!((b[0] - 4.0).abs() < 1e-5); // 5.0 - 1.0
        assert!((b[1] - 4.5).abs() < 1e-5); // 5.0 - 0.5
        assert!((b[2] - 6.0).abs() < 1e-5); // 5.0 + 1.0
        assert!((b[3] - 5.5).abs() < 1e-5); // 5.0 + 0.5
    }

    #[test]
    fn pixel_area() {
        let vp = AdaptiveViewport::new(1.0, [0.0, 0.0], [1920, 1080]);
        assert!((vp.pixel_area() - 2_073_600.0).abs() < 1.0);
    }

    #[test]
    fn to_viewport_2d_roundtrip() {
        let vp = AdaptiveViewport::new(100.0, [5.0, 5.0], [800, 600]);
        let v2d = vp.to_viewport_2d();
        assert!((v2d.pixel_width - 800.0).abs() < f32::EPSILON);
        assert!((v2d.pixel_height - 600.0).abs() < f32::EPSILON);
        let bounds = vp.world_bounds();
        assert!((v2d.min_x - bounds[0]).abs() < 1e-5);
        assert!((v2d.max_x - bounds[2]).abs() < 1e-5);
    }

    #[test]
    fn zoom_clamped_positive() {
        let vp = AdaptiveViewport::new(-5.0, [0.0, 0.0], [800, 600]);
        assert!(vp.pixels_per_world_unit() > 0.0);
    }
}

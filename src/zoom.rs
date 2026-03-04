// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Zoom and pan interaction behaviour for GPU-accelerated visualizations.
//!
//! This module provides [`ZoomBehavior`] — a configurable behaviour that
//! translates mouse-wheel and drag events into a [`GpuViewportTransform`]
//! uniform uploaded to the GPU each frame.  The vertex shader applies this
//! transform directly in clip space, so geometry buffers are never touched
//! during navigation.  This enables smooth 60 FPS panning and zooming even
//! for very large datasets (1M+ points).
//!
//! # Architecture
//!
//! ```text
//! ┌──────────────┐     ┌──────────────┐     ┌──────────────────────┐
//! │ winit events  │────▶│ ZoomBehavior │────▶│ GpuViewportTransform │
//! │ (wheel, drag) │     │  (state)     │     │  (GPU uniform)       │
//! └──────────────┘     └──────────────┘     └──────────────────────┘
//! ```
//!
//! # Quick Start
//!
//! ```rust,no_run
//! use gup::zoom::{ZoomBehavior, GpuViewportTransform};
//!
//! let mut zoom = ZoomBehavior::new()
//!     .scale_extent(0.1, 100.0)
//!     .inertia_decay(0.85);
//!
//! // On mouse wheel: zoom centered on cursor
//! let (delta_y, cursor_clip_x, cursor_clip_y) = (0.0, 0.0, 0.0);
//! zoom.on_wheel(delta_y, cursor_clip_x, cursor_clip_y);
//!
//! // On drag: pan the viewport
//! zoom.on_drag_start(cursor_clip_x, cursor_clip_y);
//! zoom.on_drag_move(cursor_clip_x, cursor_clip_y);
//! zoom.on_drag_end();
//!
//! // Each frame: tick inertia and get the GPU uniform
//! zoom.tick();
//! let transform: GpuViewportTransform = zoom.gpu_transform();
//! ```

/// GPU-ready viewport transform uniform.
///
/// Uploaded to a wgpu uniform buffer and read by mark vertex shaders to
/// apply zoom and pan in clip space.  When no [`ZoomBehavior`] is attached
/// the default identity transform (`scale = 1, translate = 0`) leaves the
/// scene unchanged.
///
/// # WGSL Layout
///
/// ```wgsl
/// struct ViewportTransform {
///     scale_x: f32,
///     scale_y: f32,
///     translate_x: f32,
///     translate_y: f32,
/// };
/// ```
///
/// The struct is 16 bytes and satisfies WGSL uniform alignment requirements
/// (vec4-sized).
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuViewportTransform {
    /// Horizontal scale factor (1.0 = no zoom).
    pub scale_x: f32,
    /// Vertical scale factor (1.0 = no zoom).
    pub scale_y: f32,
    /// Horizontal translation in clip space.
    pub translate_x: f32,
    /// Vertical translation in clip space.
    pub translate_y: f32,
}

impl Default for GpuViewportTransform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl GpuViewportTransform {
    /// The identity transform: no zoom, no pan.
    pub const IDENTITY: Self = Self {
        scale_x: 1.0,
        scale_y: 1.0,
        translate_x: 0.0,
        translate_y: 0.0,
    };
}

// ---------------------------------------------------------------------------
// ZoomState — internal f64-precision state
// ---------------------------------------------------------------------------

/// Internal zoom/pan state maintained in f64 for numerical stability.
///
/// The f64 state is down-cast to f32 only when producing the
/// [`GpuViewportTransform`] for upload to the GPU.
#[derive(Debug, Clone)]
struct ZoomState {
    /// Current scale (uniform: `scale_x == scale_y`).
    scale: f64,
    /// Current translation `[x, y]` in clip space.
    translate: [f64; 2],
    /// Current velocity `[vx, vy]` in clip-space units per frame (for inertia).
    velocity: [f64; 2],
}

impl Default for ZoomState {
    fn default() -> Self {
        Self {
            scale: 1.0,
            translate: [0.0, 0.0],
            velocity: [0.0, 0.0],
        }
    }
}

// ---------------------------------------------------------------------------
// ZoomBehavior — public API
// ---------------------------------------------------------------------------

/// Configurable zoom and pan behaviour for chart navigation.
///
/// `ZoomBehavior` processes mouse-wheel and drag events and maintains an
/// internal `ZoomState` that is converted to a [`GpuViewportTransform`]
/// each frame.  The transform is applied by the vertex shader in clip space,
/// so geometry buffers are never modified during navigation.
///
/// # Builder API
///
/// ```rust
/// use gup::zoom::ZoomBehavior;
///
/// let zoom = ZoomBehavior::new()
///     .scale_extent(0.1, 100.0)
///     .translate_extent(-2.0, -2.0, 2.0, 2.0)
///     .inertia_decay(0.9);
/// ```
///
/// # Defaults
///
/// | Parameter        | Default             |
/// |------------------|---------------------|
/// | `scale_extent`   | `(0.01, 10_000.0)`  |
/// | `translate_extent` | None (unconstrained) |
/// | `inertia_decay`  | `0.85`              |
/// | `velocity_threshold` | `0.5` px/frame  |
/// | `wheel_sensitivity` | `0.001`          |
#[derive(Debug, Clone)]
pub struct ZoomBehavior {
    /// Internal zoom/pan state.
    state: ZoomState,

    // -- Configuration --
    /// Minimum allowed scale.
    scale_min: f64,
    /// Maximum allowed scale.
    scale_max: f64,
    /// Optional translate extent `[x0, y0, x1, y1]` in world space.
    translate_extent: Option<[f64; 4]>,
    /// Inertia decay coefficient per frame (`0.0` = no inertia, close to
    /// `1.0` = very slow decay).
    inertia_decay: f64,
    /// Velocity magnitude below which inertia stops (clip-space units/frame).
    velocity_threshold: f64,
    /// Sensitivity multiplier for wheel delta → scale change.
    wheel_sensitivity: f64,

    // -- Drag tracking --
    /// Whether a drag is currently active.
    dragging: bool,
    /// Last drag position in clip space.
    drag_last: [f64; 2],
    /// Whether inertia is currently active (post-drag glide).
    inertia_active: bool,
}

impl Default for ZoomBehavior {
    fn default() -> Self {
        Self::new()
    }
}

impl ZoomBehavior {
    /// Create a new `ZoomBehavior` with default settings.
    ///
    /// The default scale extent is `(0.01, 10_000.0)` and no translate
    /// extent is set. Inertia decay is `0.85`.
    pub fn new() -> Self {
        Self {
            state: ZoomState::default(),
            scale_min: 0.01,
            scale_max: 10_000.0,
            translate_extent: None,
            inertia_decay: 0.85,
            velocity_threshold: 0.5 / 800.0, // ~0.5 px at 800px viewport
            wheel_sensitivity: 0.001,
            dragging: false,
            drag_last: [0.0; 2],
            inertia_active: false,
        }
    }

    // -- Builder methods --

    /// Constrain the zoom range to `[min, max]`.
    ///
    /// # Panics
    ///
    /// Panics if `min <= 0.0` or `min >= max`.
    pub fn scale_extent(mut self, min: f64, max: f64) -> Self {
        assert!(min > 0.0, "scale_extent min must be > 0.0, got {min}");
        assert!(min < max, "scale_extent min ({min}) must be < max ({max})");
        self.scale_min = min;
        self.scale_max = max;
        self
    }

    /// Optionally constrain the pan range to a world-space rectangle.
    ///
    /// The rectangle is defined by `(x0, y0)` (bottom-left in clip space)
    /// and `(x1, y1)` (top-right in clip space).
    pub fn translate_extent(mut self, x0: f64, y0: f64, x1: f64, y1: f64) -> Self {
        self.translate_extent = Some([x0, y0, x1, y1]);
        self
    }

    /// Set the inertia decay coefficient.
    ///
    /// - `0.0` disables inertia entirely (drag stops instantly).
    /// - Close to `1.0` produces very slow decay (long glide).
    ///
    /// The value is clamped to `[0.0, 1.0)`.
    pub fn inertia_decay(mut self, alpha: f64) -> Self {
        self.inertia_decay = alpha.clamp(0.0, 1.0 - f64::EPSILON);
        self
    }

    /// Set the wheel sensitivity (scale change per unit of wheel delta).
    ///
    /// Default is `0.001`. Higher values make zooming faster.
    pub fn wheel_sensitivity(mut self, sensitivity: f64) -> Self {
        self.wheel_sensitivity = sensitivity;
        self
    }

    // -- Event handlers --

    /// Handle a mouse-wheel event.
    ///
    /// `delta_y` is the scroll amount (positive = zoom in, negative = zoom
    /// out). `cursor_clip_x` and `cursor_clip_y` are the cursor position in
    /// clip space `[-1, 1]`.
    ///
    /// The zoom is anchored to the cursor position so the point under the
    /// cursor remains fixed after the scale change.
    pub fn on_wheel(&mut self, delta_y: f64, cursor_clip_x: f64, cursor_clip_y: f64) {
        let old_scale = self.state.scale;

        // Compute new scale with exponential zoom.
        let factor = (-delta_y * self.wheel_sensitivity).exp();
        let new_scale = (old_scale * factor).clamp(self.scale_min, self.scale_max);

        if (new_scale - old_scale).abs() < f64::EPSILON {
            return;
        }

        // Zoom-to-cursor: keep the point under the cursor fixed.
        //
        // Before zoom, the world point under the cursor is:
        //   world = (cursor_clip - translate) / old_scale
        //
        // After zoom, we want:
        //   cursor_clip = world * new_scale + new_translate
        //   new_translate = cursor_clip - world * new_scale
        //   new_translate = cursor_clip - (cursor_clip - old_translate) / old_scale * new_scale
        //   new_translate = cursor_clip - (cursor_clip - old_translate) * (new_scale / old_scale)
        let ratio = new_scale / old_scale;
        self.state.translate[0] = cursor_clip_x - (cursor_clip_x - self.state.translate[0]) * ratio;
        self.state.translate[1] = cursor_clip_y - (cursor_clip_y - self.state.translate[1]) * ratio;

        self.state.scale = new_scale;
        self.clamp_translate();
    }

    /// Signal the start of a drag (mouse-down or touch-start).
    ///
    /// Cancels any active inertia animation.
    pub fn on_drag_start(&mut self, clip_x: f64, clip_y: f64) {
        self.dragging = true;
        self.drag_last = [clip_x, clip_y];
        // Cancel inertia on new drag.
        self.inertia_active = false;
        self.state.velocity = [0.0, 0.0];
    }

    /// Handle a drag-move event.
    ///
    /// `clip_x` and `clip_y` are the current cursor position in clip space.
    pub fn on_drag_move(&mut self, clip_x: f64, clip_y: f64) {
        if !self.dragging {
            return;
        }
        let dx = clip_x - self.drag_last[0];
        let dy = clip_y - self.drag_last[1];
        self.state.translate[0] += dx;
        self.state.translate[1] += dy;
        // Record velocity for inertia (simple: last frame delta).
        self.state.velocity = [dx, dy];
        self.drag_last = [clip_x, clip_y];
        self.clamp_translate();
    }

    /// Signal the end of a drag (mouse-up or touch-end).
    ///
    /// If inertia is enabled (decay > 0), the viewport will continue to
    /// glide with the release velocity.
    pub fn on_drag_end(&mut self) {
        self.dragging = false;
        if self.inertia_decay > 0.0 {
            let speed = (self.state.velocity[0].powi(2) + self.state.velocity[1].powi(2)).sqrt();
            if speed > self.velocity_threshold {
                self.inertia_active = true;
            }
        }
    }

    /// Advance the inertia simulation by one frame.
    ///
    /// Call this once per frame (e.g., in `RedrawRequested`). If inertia is
    /// not active this is a no-op.
    ///
    /// Returns `true` if the viewport changed (i.e., a redraw is needed).
    pub fn tick(&mut self) -> bool {
        if !self.inertia_active {
            return false;
        }

        self.state.translate[0] += self.state.velocity[0];
        self.state.translate[1] += self.state.velocity[1];
        self.clamp_translate();

        // Decay velocity.
        self.state.velocity[0] *= self.inertia_decay;
        self.state.velocity[1] *= self.inertia_decay;

        let speed = (self.state.velocity[0].powi(2) + self.state.velocity[1].powi(2)).sqrt();
        if speed < self.velocity_threshold {
            self.state.velocity = [0.0, 0.0];
            self.inertia_active = false;
        }

        true
    }

    // -- Accessors --

    /// Produce the GPU-ready viewport transform from the current state.
    ///
    /// The f64 state is down-cast to f32 for the GPU uniform.
    pub fn gpu_transform(&self) -> GpuViewportTransform {
        GpuViewportTransform {
            scale_x: self.state.scale as f32,
            scale_y: self.state.scale as f32,
            translate_x: self.state.translate[0] as f32,
            translate_y: self.state.translate[1] as f32,
        }
    }

    /// Returns `true` if inertia is currently animating.
    pub fn is_animating(&self) -> bool {
        self.inertia_active
    }

    /// Returns `true` if a drag is in progress.
    pub fn is_dragging(&self) -> bool {
        self.dragging
    }

    /// Returns the current scale factor.
    pub fn scale(&self) -> f64 {
        self.state.scale
    }

    /// Returns the current translation `[x, y]`.
    pub fn translate(&self) -> [f64; 2] {
        self.state.translate
    }

    /// Reset the zoom/pan state to identity (scale = 1, translate = 0).
    pub fn reset(&mut self) {
        self.state = ZoomState::default();
        self.inertia_active = false;
        self.dragging = false;
    }

    // -- Internal helpers --

    /// Clamp translation to the configured extent (if any).
    fn clamp_translate(&mut self) {
        if let Some([x0, y0, x1, y1]) = self.translate_extent {
            // The visible region in world space has half-size 1/scale in each axis.
            // Clamp so that the visible region stays within the extent.
            let half_w = 1.0 / self.state.scale;
            let half_h = 1.0 / self.state.scale;
            let min_tx = -(x1 - half_w) * self.state.scale;
            let max_tx = -(x0 + half_w) * self.state.scale;
            let min_ty = -(y1 - half_h) * self.state.scale;
            let max_ty = -(y0 + half_h) * self.state.scale;
            if min_tx <= max_tx {
                self.state.translate[0] = self.state.translate[0].clamp(min_tx, max_tx);
            }
            if min_ty <= max_ty {
                self.state.translate[1] = self.state.translate[1].clamp(min_ty, max_ty);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_produces_identity_transform() {
        let zoom = ZoomBehavior::new();
        let t = zoom.gpu_transform();
        assert_eq!(t.scale_x, 1.0);
        assert_eq!(t.scale_y, 1.0);
        assert_eq!(t.translate_x, 0.0);
        assert_eq!(t.translate_y, 0.0);
    }

    #[test]
    fn gpu_viewport_transform_default_is_identity() {
        let t = GpuViewportTransform::default();
        assert_eq!(t.scale_x, 1.0);
        assert_eq!(t.scale_y, 1.0);
        assert_eq!(t.translate_x, 0.0);
        assert_eq!(t.translate_y, 0.0);
    }

    #[test]
    fn gpu_viewport_transform_is_16_bytes() {
        assert_eq!(std::mem::size_of::<GpuViewportTransform>(), 16);
    }

    #[test]
    fn gpu_viewport_transform_pod() {
        // Verify bytemuck traits work.
        let t = GpuViewportTransform {
            scale_x: 2.0,
            scale_y: 3.0,
            translate_x: 0.5,
            translate_y: -0.5,
        };
        let bytes: &[u8] = bytemuck::bytes_of(&t);
        assert_eq!(bytes.len(), 16);
        let round: &GpuViewportTransform = bytemuck::from_bytes(bytes);
        assert_eq!(round.scale_x, 2.0);
    }

    // -- Scale extent --

    #[test]
    #[should_panic(expected = "scale_extent min must be > 0.0")]
    fn scale_extent_panics_on_zero_min() {
        ZoomBehavior::new().scale_extent(0.0, 10.0);
    }

    #[test]
    #[should_panic(expected = "scale_extent min must be > 0.0")]
    fn scale_extent_panics_on_negative_min() {
        ZoomBehavior::new().scale_extent(-1.0, 10.0);
    }

    #[test]
    #[should_panic(expected = "scale_extent min")]
    fn scale_extent_panics_when_min_ge_max() {
        ZoomBehavior::new().scale_extent(10.0, 5.0);
    }

    // -- Zoom to cursor --

    #[test]
    fn zoom_to_cursor_keeps_point_fixed() {
        let mut zoom = ZoomBehavior::new().scale_extent(0.1, 100.0);

        // Cursor at the center of the viewport (clip 0,0).
        let cx = 0.0;
        let cy = 0.0;

        // Zoom in (negative delta_y = zoom in with our exp formula).
        zoom.on_wheel(-500.0, cx, cy);

        let t = zoom.gpu_transform();
        // At center, translation should remain near 0.
        assert!(
            t.translate_x.abs() < 1e-5,
            "translate_x should be ~0, got {}",
            t.translate_x
        );
        assert!(
            t.translate_y.abs() < 1e-5,
            "translate_y should be ~0, got {}",
            t.translate_y
        );
        // Scale should have increased.
        assert!(t.scale_x > 1.0, "scale should increase, got {}", t.scale_x);
    }

    #[test]
    fn zoom_to_cursor_off_center_keeps_point_fixed() {
        let mut zoom = ZoomBehavior::new().scale_extent(0.1, 100.0);

        let cx = 0.5;
        let cy = -0.3;

        // Before zoom, the world point under cursor at (0.5, -0.3)
        // with identity transform is (0.5, -0.3).
        let old_scale = zoom.scale();
        let world_x = (cx - zoom.translate()[0]) / old_scale;
        let world_y = (cy - zoom.translate()[1]) / old_scale;

        // Zoom in.
        zoom.on_wheel(-300.0, cx, cy);

        // After zoom, the world point under cursor should be the same.
        let new_scale = zoom.scale();
        let new_world_x = (cx - zoom.translate()[0]) / new_scale;
        let new_world_y = (cy - zoom.translate()[1]) / new_scale;

        assert!(
            (world_x - new_world_x).abs() < 1e-10,
            "world_x should be preserved: {world_x} vs {new_world_x}"
        );
        assert!(
            (world_y - new_world_y).abs() < 1e-10,
            "world_y should be preserved: {world_y} vs {new_world_y}"
        );
    }

    // -- Scale clamping --

    #[test]
    fn scale_is_clamped_to_extent() {
        let mut zoom = ZoomBehavior::new().scale_extent(0.5, 4.0);

        // Zoom way out (positive delta = zoom out).
        zoom.on_wheel(10000.0, 0.0, 0.0);
        assert!(
            zoom.scale() >= 0.5 - f64::EPSILON,
            "scale should be >= 0.5, got {}",
            zoom.scale()
        );

        // Zoom way in.
        zoom.on_wheel(-10000.0, 0.0, 0.0);
        assert!(
            zoom.scale() <= 4.0 + f64::EPSILON,
            "scale should be <= 4.0, got {}",
            zoom.scale()
        );
    }

    // -- Translate clamping --

    #[test]
    fn translate_is_clamped_to_extent() {
        let mut zoom = ZoomBehavior::new()
            .scale_extent(0.5, 4.0)
            .translate_extent(-1.0, -1.0, 1.0, 1.0);

        // Pan far to the right.
        zoom.on_drag_start(0.0, 0.0);
        zoom.on_drag_move(100.0, 0.0);
        zoom.on_drag_end();

        // Translate should be clamped.
        let t = zoom.gpu_transform();
        // The exact bound depends on scale, but it shouldn't be at 100.
        assert!(
            t.translate_x < 10.0,
            "translate_x should be clamped, got {}",
            t.translate_x
        );
    }

    // -- Inertia --

    #[test]
    fn inertia_decays_to_zero() {
        let mut zoom = ZoomBehavior::new().inertia_decay(0.8);

        // Simulate a fast drag.
        zoom.on_drag_start(0.0, 0.0);
        zoom.on_drag_move(0.1, 0.0);
        zoom.on_drag_end();

        assert!(zoom.is_animating(), "inertia should be active after drag");

        // Tick until inertia stops.
        let mut frames = 0;
        while zoom.tick() {
            frames += 1;
            if frames > 1000 {
                panic!("inertia did not converge within 1000 frames");
            }
        }

        assert!(
            !zoom.is_animating(),
            "inertia should be inactive after convergence"
        );
        assert!(frames > 1, "inertia should have run for multiple frames");
    }

    #[test]
    fn inertia_disabled_with_zero_decay() {
        let mut zoom = ZoomBehavior::new().inertia_decay(0.0);

        zoom.on_drag_start(0.0, 0.0);
        zoom.on_drag_move(0.1, 0.0);
        zoom.on_drag_end();

        // With zero decay, inertia should not activate.
        assert!(
            !zoom.is_animating(),
            "inertia should not be active with decay=0"
        );
        assert!(!zoom.tick(), "tick should return false when no inertia");
    }

    #[test]
    fn new_drag_cancels_inertia() {
        let mut zoom = ZoomBehavior::new().inertia_decay(0.9);

        // Start inertia.
        zoom.on_drag_start(0.0, 0.0);
        zoom.on_drag_move(0.1, 0.0);
        zoom.on_drag_end();
        assert!(zoom.is_animating());

        // Tick a few frames.
        zoom.tick();
        zoom.tick();

        // Start a new drag — should cancel inertia.
        zoom.on_drag_start(0.0, 0.0);
        assert!(!zoom.is_animating(), "new drag should cancel inertia");
    }

    // -- Drag/pan --

    #[test]
    fn drag_translates_viewport() {
        let mut zoom = ZoomBehavior::new().inertia_decay(0.0);

        zoom.on_drag_start(0.0, 0.0);
        zoom.on_drag_move(0.5, 0.3);
        zoom.on_drag_end();

        let t = zoom.gpu_transform();
        assert!((t.translate_x - 0.5).abs() < 1e-5);
        assert!((t.translate_y - 0.3).abs() < 1e-5);
    }

    // -- Reset --

    #[test]
    fn reset_returns_to_identity() {
        let mut zoom = ZoomBehavior::new();
        zoom.on_wheel(-500.0, 0.5, 0.5);
        zoom.on_drag_start(0.0, 0.0);
        zoom.on_drag_move(1.0, 1.0);
        zoom.on_drag_end();

        zoom.reset();

        let t = zoom.gpu_transform();
        assert_eq!(t.scale_x, 1.0);
        assert_eq!(t.translate_x, 0.0);
        assert_eq!(t.translate_y, 0.0);
        assert!(!zoom.is_animating());
        assert!(!zoom.is_dragging());
    }

    // -- Builder chaining --

    #[test]
    fn builder_chaining_works() {
        let zoom = ZoomBehavior::new()
            .scale_extent(0.1, 50.0)
            .translate_extent(-5.0, -5.0, 5.0, 5.0)
            .inertia_decay(0.7)
            .wheel_sensitivity(0.002);

        // Just verify it compiles and doesn't panic.
        let _ = zoom.gpu_transform();
    }

    // -- Replacing behavior --

    #[test]
    fn replacing_zoom_behavior_does_not_panic() {
        let mut zoom = ZoomBehavior::new().scale_extent(0.1, 10.0);
        zoom.on_wheel(-200.0, 0.0, 0.0);

        // Replace with a new behavior.
        zoom = ZoomBehavior::new().scale_extent(0.5, 5.0);
        let t = zoom.gpu_transform();
        assert_eq!(t.scale_x, 1.0); // Fresh state.
    }
}

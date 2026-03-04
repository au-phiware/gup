// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Temporal animation and easing shader functions.
//!
//! Provides GPU-accelerated temporal interpolation, easing functions,
//! keyframe animation, animation timelines, and cubic bezier timing curves.

use super::core::*;

/// Temporal animation function: interpolates between two values over time.
///
/// Enables smooth transitions and animations in visualizations.
#[derive(Clone, Debug)]
pub struct TemporalInterpolation {
    /// Starting value of the interpolation.
    pub start_value: f32,
    /// Ending value of the interpolation.
    pub end_value: f32,
    /// Duration of the interpolation in seconds.
    pub duration: f32,
}

impl TemporalInterpolation {
    /// Creates a new temporal interpolation with the given parameters.
    pub fn new(start_value: f32, end_value: f32, duration: f32) -> Self {
        Self {
            start_value,
            end_value,
            duration,
        }
    }
}

/// GPU uniform data for the temporal interpolation shader function.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TemporalInterpolationUniforms {
    /// Starting value of the interpolation.
    pub start_value: f32,
    /// Ending value of the interpolation.
    pub end_value: f32,
    /// Duration of the interpolation in seconds.
    pub duration: f32,
    /// Padding for GPU alignment.
    pub _padding: f32,
}

impl ShaderUniform for TemporalInterpolationUniforms {
    fn wgsl_struct_definition() -> String {
        "struct TemporalInterpolationUniforms {\n    start_value: f32,\n    end_value: f32,\n    duration: f32,\n}".to_string()
    }

    fn wgsl_type_name() -> &'static str {
        "TemporalInterpolationUniforms"
    }
}

impl ComposableShaderFunction for TemporalInterpolation {
    type Input = f32; // time input
    type Output = f32;
    type Uniforms = TemporalInterpolationUniforms;

    fn wgsl_function() -> &'static str {
        r#"
        fn temporal_interpolation(time: f32, params: TemporalInterpolationUniforms) -> f32 {
            let t = clamp(time / params.duration, 0.0, 1.0);
            return mix(params.start_value, params.end_value, t);
        }
        "#
    }

    fn create_uniforms(&self) -> Option<Self::Uniforms> {
        Some(TemporalInterpolationUniforms {
            start_value: self.start_value,
            end_value: self.end_value,
            duration: self.duration,
            _padding: 0.0,
        })
    }

    fn function_name() -> &'static str {
        "temporal_interpolation"
    }
}

/// Easing function for smooth animations.
///
/// Applies common easing curves to temporal values.
#[derive(Clone, Debug)]
pub enum EasingFunction {
    /// Linear easing (no acceleration).
    Linear,
    /// Quadratic ease-in (accelerating).
    EaseInQuad,
    /// Quadratic ease-out (decelerating).
    EaseOutQuad,
    /// Quadratic ease-in-out (accelerate then decelerate).
    EaseInOutQuad,
    /// Cubic ease-in (accelerating).
    EaseInCubic,
    /// Cubic ease-out (decelerating).
    EaseOutCubic,
    /// Cubic ease-in-out (accelerate then decelerate).
    EaseInOutCubic,
}

/// Easing shader function that applies an easing curve to a value.
#[derive(Clone, Debug)]
pub struct Easing {
    /// The easing function to apply.
    pub function: EasingFunction,
}

impl Easing {
    /// Creates a new easing function with the given curve.
    pub fn new(function: EasingFunction) -> Self {
        Self { function }
    }

    /// Creates a linear easing function (no acceleration).
    pub fn linear() -> Self {
        Self {
            function: EasingFunction::Linear,
        }
    }

    /// Creates a cubic ease-in-out easing function.
    pub fn ease_in_out() -> Self {
        Self {
            function: EasingFunction::EaseInOutCubic,
        }
    }
}

/// GPU uniform data for the easing shader function.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct EasingUniforms {
    /// Easing type index (0=linear, 1=ease_in_quad, etc.).
    pub easing_type: u32, // 0=linear, 1=ease_in_quad, etc.
    /// Padding for GPU alignment.
    pub _padding: [f32; 3],
}

impl ShaderUniform for EasingUniforms {
    fn wgsl_struct_definition() -> String {
        "struct EasingUniforms {\n    easing_type: u32,\n}".to_string()
    }

    fn wgsl_type_name() -> &'static str {
        "EasingUniforms"
    }
}

impl ComposableShaderFunction for Easing {
    type Input = f32; // normalized time (0-1)
    type Output = f32;
    type Uniforms = EasingUniforms;

    fn wgsl_function() -> &'static str {
        r#"
        fn easing(t: f32, params: EasingUniforms) -> f32 {
            let normalized = clamp(t, 0.0, 1.0);

            if (params.easing_type == 0u) {
                return normalized; // Linear
            } else if (params.easing_type == 1u) {
                return normalized * normalized; // EaseInQuad
            } else if (params.easing_type == 2u) {
                return 1.0 - (1.0 - normalized) * (1.0 - normalized); // EaseOutQuad
            } else if (params.easing_type == 3u) {
                if (normalized < 0.5) {
                    return 2.0 * normalized * normalized; // EaseInOutQuad first half
                } else {
                    let n = 1.0 - normalized;
                    return 1.0 - 2.0 * n * n; // EaseInOutQuad second half
                }
            } else if (params.easing_type == 4u) {
                return normalized * normalized * normalized; // EaseInCubic
            } else if (params.easing_type == 5u) {
                let n = 1.0 - normalized;
                return 1.0 - n * n * n; // EaseOutCubic
            } else if (params.easing_type == 6u) {
                if (normalized < 0.5) {
                    return 4.0 * normalized * normalized * normalized; // EaseInOutCubic first half
                } else {
                    let n = 2.0 * normalized - 2.0;
                    return 0.5 * n * n * n + 1.0; // EaseInOutCubic second half
                }
            }

            return normalized; // Fallback to linear
        }
        "#
    }

    fn create_uniforms(&self) -> Option<Self::Uniforms> {
        let easing_type = match self.function {
            EasingFunction::Linear => 0,
            EasingFunction::EaseInQuad => 1,
            EasingFunction::EaseOutQuad => 2,
            EasingFunction::EaseInOutQuad => 3,
            EasingFunction::EaseInCubic => 4,
            EasingFunction::EaseOutCubic => 5,
            EasingFunction::EaseInOutCubic => 6,
        };

        Some(EasingUniforms {
            easing_type,
            _padding: [0.0; 3],
        })
    }

    fn function_name() -> &'static str {
        "easing"
    }
}

// ============================================================================
// Advanced Temporal Animation System (GUP-138)
// ============================================================================

/// Keyframe for animations - represents a single point in time with a value.
///
/// Keyframes are used to define animation trajectories with multiple control points.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Keyframe {
    /// Time position of this keyframe.
    pub time: f32,
    /// Value at this keyframe.
    pub value: f32,
    /// Padding for uniform buffer alignment.
    pub _padding: [f32; 2], // Align to 16 bytes
}

impl Keyframe {
    /// Creates a new keyframe at the given time with the given value.
    pub fn new(time: f32, value: f32) -> Self {
        Self {
            time,
            value,
            _padding: [0.0; 2],
        }
    }
}

/// Maximum number of keyframes supported in uniform buffer-based animations.
/// For more keyframes, use storage buffer-based animations.
pub const MAX_KEYFRAMES: usize = 16;

/// Interpolation mode for keyframe animation.
///
/// Determines how values are interpolated between keyframes.
#[derive(Clone, Debug, Copy, PartialEq, Default)]
pub enum InterpolationMode {
    /// Linear interpolation between keyframes (default).
    #[default]
    Linear,
    /// Catmull-Rom spline interpolation with configurable tension.
    /// Tension of 0.0 gives a standard Catmull-Rom spline (C1 continuous).
    /// Tension of 1.0 gives straight lines. Range: \[0.0, 1.0\]
    CatmullRom {
        /// Catmull-Rom tension parameter.
        tension: f32,
    },
    /// Cubic B-spline interpolation (C2 continuous, very smooth).
    BSpline,
}

impl InterpolationMode {
    /// Returns the mode identifier for WGSL code generation.
    fn mode_id(&self) -> u32 {
        match self {
            InterpolationMode::Linear => 0,
            InterpolationMode::CatmullRom { .. } => 1,
            InterpolationMode::BSpline => 2,
        }
    }

    /// Returns the tension parameter (only used for Catmull-Rom).
    fn tension(&self) -> f32 {
        match self {
            InterpolationMode::CatmullRom { tension } => *tension,
            _ => 0.0,
        }
    }
}

/// Keyframe animation with up to 16 keyframes in a uniform buffer.
///
/// Supports multiple interpolation modes including linear, Catmull-Rom, and B-spline.
/// For animations requiring more keyframes, use KeyframeAnimationStorageBuffer.
#[derive(Clone, Debug)]
pub struct KeyframeAnimation {
    /// The list of keyframes.
    pub keyframes: Vec<Keyframe>,
    /// Whether the animation loops.
    pub loop_animation: bool,
    /// Whether playback reverses on each loop.
    pub reverse_on_loop: bool,
    /// Interpolation mode between keyframes.
    pub interpolation_mode: InterpolationMode,
}

impl KeyframeAnimation {
    /// Creates a new empty keyframe animation.
    pub fn new() -> Self {
        Self {
            keyframes: Vec::new(),
            loop_animation: false,
            reverse_on_loop: false,
            interpolation_mode: InterpolationMode::default(),
        }
    }

    /// Adds a keyframe at the given time with the given value.
    pub fn add_keyframe(mut self, time: f32, value: f32) -> Self {
        if self.keyframes.len() < MAX_KEYFRAMES {
            self.keyframes.push(Keyframe::new(time, value));
            // Keep keyframes sorted by time
            self.keyframes
                .sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());
        }
        self
    }

    /// Enables or disables animation looping.
    pub fn with_loop(mut self, enable: bool) -> Self {
        self.loop_animation = enable;
        self
    }

    /// Enables or disables reverse playback on loop.
    pub fn with_reverse(mut self, enable: bool) -> Self {
        self.reverse_on_loop = enable;
        self
    }

    /// Set the interpolation mode for this animation.
    ///
    /// # Examples
    ///
    /// ```
    /// use gup::shader_function::{KeyframeAnimation, InterpolationMode};
    ///
    /// let anim = KeyframeAnimation::new()
    ///     .add_keyframe(0.0, 0.0)
    ///     .add_keyframe(1.0, 10.0)
    ///     .with_interpolation(InterpolationMode::CatmullRom { tension: 0.0 });
    /// ```
    pub fn with_interpolation(mut self, mode: InterpolationMode) -> Self {
        self.interpolation_mode = mode;
        self
    }

    /// Convenience method to set Catmull-Rom interpolation with specified tension.
    ///
    /// # Examples
    ///
    /// ```
    /// use gup::shader_function::KeyframeAnimation;
    ///
    /// let anim = KeyframeAnimation::new()
    ///     .add_keyframe(0.0, 0.0)
    ///     .add_keyframe(1.0, 10.0)
    ///     .with_catmull_rom(0.0); // Standard Catmull-Rom spline
    /// ```
    pub fn with_catmull_rom(mut self, tension: f32) -> Self {
        self.interpolation_mode = InterpolationMode::CatmullRom {
            tension: tension.clamp(0.0, 1.0),
        };
        self
    }

    /// Convenience method to set B-spline interpolation.
    ///
    /// # Examples
    ///
    /// ```
    /// use gup::shader_function::KeyframeAnimation;
    ///
    /// let anim = KeyframeAnimation::new()
    ///     .add_keyframe(0.0, 0.0)
    ///     .add_keyframe(1.0, 10.0)
    ///     .with_bspline();
    /// ```
    pub fn with_bspline(mut self) -> Self {
        self.interpolation_mode = InterpolationMode::BSpline;
        self
    }

    /// Evaluate the animation on the CPU at the given normalised time.
    ///
    /// `time` is expected in `[0.0, 1.0]` for a simple two-keyframe
    /// animation. If there are no keyframes, returns 0.0. For a single
    /// keyframe, returns its value. For multiple keyframes, linearly
    /// interpolates between the two surrounding keyframes.
    ///
    /// This mirrors the GPU-side `keyframe_animation` WGSL function so
    /// that CPU-side transition interpolation matches GPU behaviour.
    pub fn evaluate(&self, time: f32) -> f32 {
        if self.keyframes.is_empty() {
            return 0.0;
        }
        if self.keyframes.len() == 1 {
            return self.keyframes[0].value;
        }
        // Clamp to keyframe time range.
        let first = self.keyframes.first().unwrap();
        let last = self.keyframes.last().unwrap();
        if time <= first.time {
            return first.value;
        }
        if time >= last.time {
            return last.value;
        }
        // Find surrounding keyframes.
        for window in self.keyframes.windows(2) {
            let kf0 = &window[0];
            let kf1 = &window[1];
            if time >= kf0.time && time <= kf1.time {
                let span = kf1.time - kf0.time;
                if span <= f32::EPSILON {
                    return kf0.value;
                }
                let local_t = (time - kf0.time) / span;
                return kf0.value + (kf1.value - kf0.value) * local_t;
            }
        }
        last.value
    }
}

impl Default for KeyframeAnimation {
    fn default() -> Self {
        Self::new()
    }
}

/// GPU uniform data for the keyframe animation shader function.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct KeyframeAnimationUniforms {
    /// Array of keyframes (up to MAX_KEYFRAMES).
    pub keyframes: [Keyframe; MAX_KEYFRAMES],
    /// Number of active keyframes.
    pub keyframe_count: u32,
    /// Whether the animation loops (0 or 1).
    pub loop_animation: u32,
    /// Whether playback reverses on loop (0 or 1).
    pub reverse_on_loop: u32,
    /// Interpolation mode (0=Linear, 1=CatmullRom, 2=BSpline).
    pub interpolation_mode: u32, // 0=Linear, 1=CatmullRom, 2=BSpline
    /// Tension parameter for Catmull-Rom interpolation.
    pub tension: f32, // For Catmull-Rom interpolation
    /// Padding for 16-byte alignment.
    pub _padding: [f32; 3], // Ensure 16-byte alignment
    /// Extra padding to match WGSL struct size.
    pub _padding2: [f32; 4], // Extra padding to match WGSL struct size (304 bytes)
}

impl ShaderUniform for KeyframeAnimationUniforms {
    fn wgsl_struct_definition() -> String {
        format!(
            "struct Keyframe {{\n    time: f32,\n    value: f32,\n    _padding0: f32,\n    _padding1: f32,\n}}\n\n\
             struct KeyframeAnimationUniforms {{\n    keyframes: array<Keyframe, {}>,\n    \
             keyframe_count: u32,\n    loop_animation: u32,\n    reverse_on_loop: u32,\n    \
             interpolation_mode: u32,\n    tension: f32,\n    _padding: vec3<f32>,\n}}",
            MAX_KEYFRAMES
        )
    }

    fn wgsl_type_name() -> &'static str {
        "KeyframeAnimationUniforms"
    }
}

impl ComposableShaderFunction for KeyframeAnimation {
    type Input = f32; // time input
    type Output = f32;
    type Uniforms = KeyframeAnimationUniforms;

    fn wgsl_function() -> &'static str {
        r#"
        // Helper function: Catmull-Rom spline interpolation
        // Interpolates between p1 and p2 using p0 and p3 as control points
        // tension: 0.0 = standard Catmull-Rom, 1.0 = linear
        fn catmull_rom_interpolate(p0: f32, p1: f32, p2: f32, p3: f32, t: f32, tension: f32) -> f32 {
            let t2 = t * t;
            let t3 = t2 * t;

            // Catmull-Rom basis matrix with tension parameter
            // Standard Catmull-Rom uses tension = 0.0
            let s = (1.0 - tension) * 0.5;

            let c0 = -s * t3 + 2.0 * s * t2 - s * t;
            let c1 = (2.0 - s) * t3 + (s - 3.0) * t2 + 1.0;
            let c2 = (s - 2.0) * t3 + (3.0 - 2.0 * s) * t2 + s * t;
            let c3 = s * t3 - s * t2;

            return c0 * p0 + c1 * p1 + c2 * p2 + c3 * p3;
        }

        // Helper function: Cubic B-spline interpolation
        // Interpolates within the segment using four control points
        fn bspline_interpolate(p0: f32, p1: f32, p2: f32, p3: f32, t: f32) -> f32 {
            let t2 = t * t;
            let t3 = t2 * t;

            // Cubic B-spline basis functions
            let b0 = (1.0 - t) * (1.0 - t) * (1.0 - t) / 6.0;
            let b1 = (3.0 * t3 - 6.0 * t2 + 4.0) / 6.0;
            let b2 = (-3.0 * t3 + 3.0 * t2 + 3.0 * t + 1.0) / 6.0;
            let b3 = t3 / 6.0;

            return b0 * p0 + b1 * p1 + b2 * p2 + b3 * p3;
        }

        fn keyframe_animation(time: f32, params: KeyframeAnimationUniforms) -> f32 {
            if (params.keyframe_count == 0u) {
                return 0.0;
            }

            if (params.keyframe_count == 1u) {
                return params.keyframes[0].value;
            }

            // Get time range from first and last keyframes
            let start_time = params.keyframes[0].time;
            let end_time = params.keyframes[params.keyframe_count - 1u].time;
            let duration = end_time - start_time;

            var t = time;

            // Handle looping
            if (params.loop_animation != 0u && duration > 0.0) {
                t = start_time + ((time - start_time) % duration);
                if (t < start_time) {
                    t = t + duration;
                }

                // Handle reverse on loop
                if (params.reverse_on_loop != 0u) {
                    let cycle = floor((time - start_time) / duration);
                    if (u32(cycle) % 2u == 1u) {
                        t = end_time - (t - start_time);
                    }
                }
            }

            // Clamp to time range
            if (t <= params.keyframes[0].time) {
                return params.keyframes[0].value;
            }
            if (t >= params.keyframes[params.keyframe_count - 1u].time) {
                return params.keyframes[params.keyframe_count - 1u].value;
            }

            // Find the segment containing time t
            var segment_index = 0u;
            for (var i = 0u; i < params.keyframe_count - 1u; i = i + 1u) {
                if (t >= params.keyframes[i].time && t <= params.keyframes[i + 1u].time) {
                    segment_index = i;
                    break;
                }
            }

            let k1 = params.keyframes[segment_index];
            let k2 = params.keyframes[segment_index + 1u];
            let segment_duration = k2.time - k1.time;

            if (segment_duration <= 0.0) {
                return k1.value;
            }

            let local_t = (t - k1.time) / segment_duration;

            // Interpolation mode selection
            if (params.interpolation_mode == 0u) {
                // Linear interpolation
                return mix(k1.value, k2.value, local_t);
            } else if (params.interpolation_mode == 1u) {
                // Catmull-Rom spline
                // Need 4 control points: p0, p1 (k1), p2 (k2), p3
                var p0: f32;
                var p3: f32;

                // Get p0 (point before k1)
                if (segment_index > 0u) {
                    p0 = params.keyframes[segment_index - 1u].value;
                } else {
                    // Duplicate first point for boundary
                    p0 = k1.value;
                }

                // Get p3 (point after k2)
                if (segment_index + 2u < params.keyframe_count) {
                    p3 = params.keyframes[segment_index + 2u].value;
                } else {
                    // Duplicate last point for boundary
                    p3 = k2.value;
                }

                return catmull_rom_interpolate(p0, k1.value, k2.value, p3, local_t, params.tension);
            } else if (params.interpolation_mode == 2u) {
                // B-spline interpolation
                // Need 4 control points
                var p0: f32;
                var p3: f32;

                // Get p0
                if (segment_index > 0u) {
                    p0 = params.keyframes[segment_index - 1u].value;
                } else {
                    p0 = k1.value;
                }

                // Get p3
                if (segment_index + 2u < params.keyframe_count) {
                    p3 = params.keyframes[segment_index + 2u].value;
                } else {
                    p3 = k2.value;
                }

                return bspline_interpolate(p0, k1.value, k2.value, p3, local_t);
            }

            // Fallback to linear
            return mix(k1.value, k2.value, local_t);
        }
        "#
    }

    fn create_uniforms(&self) -> Option<Self::Uniforms> {
        let mut keyframes = [Keyframe {
            time: 0.0,
            value: 0.0,
            _padding: [0.0; 2],
        }; MAX_KEYFRAMES];

        for (i, kf) in self.keyframes.iter().enumerate().take(MAX_KEYFRAMES) {
            keyframes[i] = *kf;
        }

        Some(KeyframeAnimationUniforms {
            keyframes,
            keyframe_count: self.keyframes.len().min(MAX_KEYFRAMES) as u32,
            loop_animation: if self.loop_animation { 1 } else { 0 },
            reverse_on_loop: if self.reverse_on_loop { 1 } else { 0 },
            interpolation_mode: self.interpolation_mode.mode_id(),
            tension: self.interpolation_mode.tension(),
            _padding: [0.0; 3],
            _padding2: [0.0; 4],
        })
    }

    fn function_name() -> &'static str {
        "keyframe_animation"
    }
}

// ============================================================================
// Storage Buffer Keyframe Animation (GUP-140)
// ============================================================================

/// Storage buffer-based keyframe animation supporting unlimited keyframes.
///
/// Similar to ColorGradientStorage, this uses storage buffers instead of uniform
/// buffers to support arbitrarily large keyframe arrays. Uses efficient binary
/// search in WGSL for O(log n) keyframe lookup.
///
/// For animations with <= 16 keyframes, prefer KeyframeAnimation (uniform-based)
/// for simplicity and performance.
#[derive(Clone, Debug)]
pub struct KeyframeAnimationStorage {
    /// The list of keyframes.
    pub keyframes: Vec<Keyframe>,
    /// Whether the animation loops.
    pub loop_animation: bool,
    /// Whether playback reverses on each loop.
    pub reverse_on_loop: bool,
}

impl KeyframeAnimationStorage {
    /// Creates a new storage-based keyframe animation.
    pub fn new(keyframes: Vec<Keyframe>) -> Self {
        assert!(!keyframes.is_empty(), "Must have at least one keyframe");
        let mut kfs = keyframes;
        kfs.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());
        Self {
            keyframes: kfs,
            loop_animation: false,
            reverse_on_loop: false,
        }
    }

    /// Creates a new animation and enables looping.
    pub fn with_loop(mut self, enable: bool) -> Self {
        self.loop_animation = enable;
        self
    }

    /// Creates a new animation with reverse-on-loop enabled.
    pub fn with_reverse(mut self, enable: bool) -> Self {
        self.reverse_on_loop = enable;
        self
    }

    /// Returns a builder for fluent keyframe construction.
    pub fn builder() -> KeyframeAnimationStorageBuilder {
        KeyframeAnimationStorageBuilder::new()
    }

    /// Creates buffer data for keyframes (for storage buffer upload).
    pub fn create_keyframes_buffer_data(&self) -> Vec<u8> {
        let mut data = Vec::with_capacity(self.keyframes.len() * 16); // 16 bytes per keyframe
        for kf in &self.keyframes {
            data.extend_from_slice(&kf.time.to_le_bytes());
            data.extend_from_slice(&kf.value.to_le_bytes());
            data.extend_from_slice(&kf._padding[0].to_le_bytes());
            data.extend_from_slice(&kf._padding[1].to_le_bytes());
        }
        data
    }

    /// Returns the number of keyframes.
    pub fn count(&self) -> u32 {
        self.keyframes.len() as u32
    }

    /// Returns the WGSL struct definition for the storage buffer.
    pub fn wgsl_struct_definition() -> &'static str {
        r#"
struct Keyframe {
    time: f32,
    value: f32,
    _padding0: f32,
    _padding1: f32,
}

struct KeyframeAnimationStorageInfo {
    keyframe_count: u32,
    loop_animation: u32,
    reverse_on_loop: u32,
    _padding: u32,
}

@group(0) @binding(1) var<storage, read> keyframe_data: array<Keyframe>;
@group(0) @binding(2) var<uniform> animation_info: KeyframeAnimationStorageInfo;
"#
    }

    /// Returns the WGSL function implementation with efficient binary search.
    pub fn wgsl_function() -> &'static str {
        r#"
fn keyframe_animation_storage(time: f32) -> f32 {
    let count = animation_info.keyframe_count;

    // Handle edge cases
    if (count == 0u) {
        return 0.0;
    }

    if (count == 1u) {
        return keyframe_data[0].value;
    }

    // Get time range from first and last keyframes
    let start_time = keyframe_data[0].time;
    let end_time = keyframe_data[count - 1u].time;
    let duration = end_time - start_time;

    var t = time;

    // Handle looping
    if (animation_info.loop_animation != 0u && duration > 0.0) {
        t = start_time + ((time - start_time) % duration);
        if (t < start_time) {
            t = t + duration;
        }

        // Handle reverse on loop
        if (animation_info.reverse_on_loop != 0u) {
            let cycle = floor((time - start_time) / duration);
            if (u32(cycle) % 2u == 1u) {
                t = end_time - (t - start_time);
            }
        }
    }

    // Clamp to time range
    if (t <= keyframe_data[0].time) {
        return keyframe_data[0].value;
    }
    if (t >= keyframe_data[count - 1u].time) {
        return keyframe_data[count - 1u].value;
    }

    // Binary search to find the interval containing t
    var low = 0u;
    var high = count - 1u;

    while (low + 1u < high) {
        let mid = (low + high) / 2u;
        if (keyframe_data[mid].time <= t) {
            low = mid;
        } else {
            high = mid;
        }
    }

    // Interpolate between the two keyframes
    let k1 = keyframe_data[low];
    let k2 = keyframe_data[high];
    let segment_duration = k2.time - k1.time;

    if (segment_duration <= 0.0) {
        return k1.value;
    }

    let local_t = (t - k1.time) / segment_duration;
    return mix(k1.value, k2.value, local_t);
}
"#
    }
}

/// Builder for creating storage-based keyframe animations with a fluent API.
pub struct KeyframeAnimationStorageBuilder {
    keyframes: Vec<Keyframe>,
    loop_animation: bool,
    reverse_on_loop: bool,
}

impl KeyframeAnimationStorageBuilder {
    /// Creates a new builder.
    pub fn new() -> Self {
        Self {
            keyframes: Vec::new(),
            loop_animation: false,
            reverse_on_loop: false,
        }
    }

    /// Adds a keyframe at the specified time and value.
    pub fn add_keyframe(mut self, time: f32, value: f32) -> Self {
        self.keyframes.push(Keyframe::new(time, value));
        self
    }

    /// Enables looping.
    pub fn with_loop(mut self, enable: bool) -> Self {
        self.loop_animation = enable;
        self
    }

    /// Enables reverse-on-loop.
    pub fn with_reverse(mut self, enable: bool) -> Self {
        self.reverse_on_loop = enable;
        self
    }

    /// Builds the animation, sorting keyframes by time.
    pub fn build(mut self) -> KeyframeAnimationStorage {
        assert!(
            !self.keyframes.is_empty(),
            "Animation must have at least one keyframe"
        );

        // Sort by time
        self.keyframes
            .sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());

        KeyframeAnimationStorage {
            keyframes: self.keyframes,
            loop_animation: self.loop_animation,
            reverse_on_loop: self.reverse_on_loop,
        }
    }
}

impl Default for KeyframeAnimationStorageBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Cubic bezier timing function for advanced easing curves.
///
/// Defines a cubic bezier curve with two control points for custom timing.
/// Common presets:
/// - ease: (0.25, 0.1, 0.25, 1.0)
/// - ease-in: (0.42, 0.0, 1.0, 1.0)
/// - ease-out: (0.0, 0.0, 0.58, 1.0)
/// - ease-in-out: (0.42, 0.0, 0.58, 1.0)
#[derive(Clone, Debug)]
pub struct CubicBezierTiming {
    /// X coordinate of the first control point.
    pub x1: f32,
    /// Y coordinate of the first control point.
    pub y1: f32,
    /// X coordinate of the second control point.
    pub x2: f32,
    /// Y coordinate of the second control point.
    pub y2: f32,
}

impl CubicBezierTiming {
    /// Creates a new cubic bezier timing function with the given control points.
    pub fn new(x1: f32, y1: f32, x2: f32, y2: f32) -> Self {
        Self { x1, y1, x2, y2 }
    }

    /// Creates the CSS `ease` timing function.
    pub fn ease() -> Self {
        Self::new(0.25, 0.1, 0.25, 1.0)
    }

    /// Creates the CSS `ease-in` timing function.
    pub fn ease_in() -> Self {
        Self::new(0.42, 0.0, 1.0, 1.0)
    }

    /// Creates the CSS `ease-out` timing function.
    pub fn ease_out() -> Self {
        Self::new(0.0, 0.0, 0.58, 1.0)
    }

    /// Creates the CSS `ease-in-out` timing function.
    pub fn ease_in_out() -> Self {
        Self::new(0.42, 0.0, 0.58, 1.0)
    }
}

/// GPU uniform data for the cubic bezier timing shader function.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CubicBezierTimingUniforms {
    /// X coordinate of the first control point.
    pub x1: f32,
    /// Y coordinate of the first control point.
    pub y1: f32,
    /// X coordinate of the second control point.
    pub x2: f32,
    /// Y coordinate of the second control point.
    pub y2: f32,
}

impl ShaderUniform for CubicBezierTimingUniforms {
    fn wgsl_struct_definition() -> String {
        "struct CubicBezierTimingUniforms {\n    x1: f32,\n    y1: f32,\n    x2: f32,\n    y2: f32,\n}".to_string()
    }

    fn wgsl_type_name() -> &'static str {
        "CubicBezierTimingUniforms"
    }
}

impl ComposableShaderFunction for CubicBezierTiming {
    type Input = f32; // normalized time (0-1)
    type Output = f32;
    type Uniforms = CubicBezierTimingUniforms;

    fn wgsl_function() -> &'static str {
        r#"
        fn cubic_bezier_timing(t: f32, params: CubicBezierTimingUniforms) -> f32 {
            let normalized = clamp(t, 0.0, 1.0);

            // Newton-Raphson method to solve for bezier X coordinate
            // We want to find t_bezier such that bezier_x(t_bezier) = normalized
            var t_bezier = normalized; // Initial guess

            for (var i = 0; i < 8; i = i + 1) {
                // Cubic bezier X formula: 3*(1-t)^2*t*x1 + 3*(1-t)*t^2*x2 + t^3
                let one_minus_t = 1.0 - t_bezier;
                let bezier_x = 3.0 * one_minus_t * one_minus_t * t_bezier * params.x1 +
                               3.0 * one_minus_t * t_bezier * t_bezier * params.x2 +
                               t_bezier * t_bezier * t_bezier;

                // Derivative of bezier X
                let bezier_x_derivative = 3.0 * one_minus_t * one_minus_t * params.x1 +
                                          6.0 * one_minus_t * t_bezier * (params.x2 - params.x1) +
                                          3.0 * t_bezier * t_bezier * (1.0 - params.x2);

                if (abs(bezier_x_derivative) < 0.000001) {
                    break;
                }

                // Newton-Raphson iteration
                let delta = (bezier_x - normalized) / bezier_x_derivative;
                t_bezier = t_bezier - delta;

                if (abs(delta) < 0.000001) {
                    break;
                }
            }

            // Calculate Y value at the found t_bezier
            let one_minus_t = 1.0 - t_bezier;
            let bezier_y = 3.0 * one_minus_t * one_minus_t * t_bezier * params.y1 +
                           3.0 * one_minus_t * t_bezier * t_bezier * params.y2 +
                           t_bezier * t_bezier * t_bezier;

            return clamp(bezier_y, 0.0, 1.0);
        }
        "#
    }

    fn create_uniforms(&self) -> Option<Self::Uniforms> {
        Some(CubicBezierTimingUniforms {
            x1: self.x1,
            y1: self.y1,
            x2: self.x2,
            y2: self.y2,
        })
    }

    fn function_name() -> &'static str {
        "cubic_bezier_timing"
    }
}

/// Animation playback state for timeline coordination.
///
/// Manages play, pause, seek, and time direction for animations.
#[derive(Clone, Debug)]
pub enum AnimationPlaybackState {
    /// Animation is currently playing.
    Playing,
    /// Animation is paused.
    Paused,
    /// Animation is stopped.
    Stopped,
}

/// Animation timeline controller for complex animation sequences.
///
/// Provides playback control and time management for animations.
#[derive(Clone, Debug)]
pub struct AnimationTimeline {
    /// Current playback time in seconds.
    pub current_time: f32,
    /// Playback speed multiplier.
    pub playback_rate: f32,
    /// Current playback state.
    pub state: AnimationPlaybackState,
    /// Whether the timeline loops.
    pub loop_timeline: bool,
    /// Total duration in seconds.
    pub duration: f32,
}

impl AnimationTimeline {
    /// Creates a new animation timeline with the given duration.
    pub fn new(duration: f32) -> Self {
        Self {
            current_time: 0.0,
            playback_rate: 1.0,
            state: AnimationPlaybackState::Stopped,
            loop_timeline: false,
            duration,
        }
    }

    /// Starts or resumes playback.
    pub fn play(&mut self) {
        self.state = AnimationPlaybackState::Playing;
    }

    /// Pauses playback at the current time.
    pub fn pause(&mut self) {
        self.state = AnimationPlaybackState::Paused;
    }

    /// Stops playback and resets to the beginning.
    pub fn stop(&mut self) {
        self.state = AnimationPlaybackState::Stopped;
        self.current_time = 0.0;
    }

    /// Seeks to a specific time position.
    pub fn seek(&mut self, time: f32) {
        self.current_time = time.clamp(0.0, self.duration);
    }

    /// Sets the playback speed multiplier.
    pub fn set_playback_rate(&mut self, rate: f32) {
        self.playback_rate = rate;
    }

    /// Enables or disables timeline looping.
    pub fn enable_loop(&mut self, enable: bool) {
        self.loop_timeline = enable;
    }

    /// Update timeline with elapsed time (in seconds)
    pub fn update(&mut self, delta_time: f32) -> f32 {
        if let AnimationPlaybackState::Playing = self.state {
            self.current_time += delta_time * self.playback_rate;

            if self.current_time > self.duration {
                if self.loop_timeline {
                    self.current_time %= self.duration;
                } else {
                    self.current_time = self.duration;
                    self.state = AnimationPlaybackState::Stopped;
                }
            } else if self.current_time < 0.0 {
                if self.loop_timeline {
                    self.current_time = self.duration + (self.current_time % self.duration);
                } else {
                    self.current_time = 0.0;
                    self.state = AnimationPlaybackState::Stopped;
                }
            }
        }

        self.current_time
    }

    /// Returns the current time as a normalized value between 0.0 and 1.0.
    pub fn normalized_time(&self) -> f32 {
        if self.duration <= 0.0 {
            0.0
        } else {
            (self.current_time / self.duration).clamp(0.0, 1.0)
        }
    }
}

// ============================================================================
// Animation Event System (GUP-142)
// ============================================================================

/// Callback type for animation events.
///
/// Events receive the timeline reference and event time for context.
#[cfg(not(target_arch = "wasm32"))]
pub type AnimationEventCallback = Box<dyn FnMut(&AnimationTimeline, f32) + Send + Sync>;
/// Callback type for animation events (WASM: relaxed Send+Sync bounds).
#[cfg(target_arch = "wasm32")]
pub type AnimationEventCallback = Box<dyn FnMut(&AnimationTimeline, f32)>;

/// Types of animation events that can be triggered.
#[derive(Clone, Debug, PartialEq)]
pub enum AnimationEventType {
    /// Event fires once at a specific time
    Once(f32),
    /// Event fires every time the timeline crosses a specific time
    Repeating(f32),
    /// Event fires when animation completes (reaches duration)
    Complete,
    /// Event fires at progress milestones (0.0 to 1.0)
    Progress(f32),
    /// Event fires when entering a specific keyframe (0-indexed)
    Keyframe(usize),
    /// Event fires at a custom named marker
    Marker(String),
}

/// A registered animation event with its trigger condition and callback.
struct AnimationEvent {
    event_type: AnimationEventType,
    callback: AnimationEventCallback,
    fired_this_frame: bool,
    last_fire_time: Option<f32>,
}

/// Extended AnimationTimeline with event system support.
///
/// Provides event registration, synchronization, and timeline coordination.
pub struct AnimationTimelineWithEvents {
    /// The underlying timeline
    pub timeline: AnimationTimeline,
    /// Registered events
    events: Vec<AnimationEvent>,
    /// Named markers for custom event triggers
    markers: std::collections::HashMap<String, f32>,
    /// Previous time for detecting time crossings
    previous_time: f32,
    /// Child timelines for hierarchical animation
    children: Vec<AnimationTimelineWithEvents>,
}

impl AnimationTimelineWithEvents {
    /// Create a new timeline with event support
    pub fn new(duration: f32) -> Self {
        Self {
            timeline: AnimationTimeline::new(duration),
            events: Vec::new(),
            markers: std::collections::HashMap::new(),
            previous_time: 0.0,
            children: Vec::new(),
        }
    }

    /// Register an event callback at a specific time
    pub fn on_time(&mut self, time: f32, callback: AnimationEventCallback) -> &mut Self {
        self.events.push(AnimationEvent {
            event_type: AnimationEventType::Once(time),
            callback,
            fired_this_frame: false,
            last_fire_time: None,
        });
        self
    }

    /// Register a repeating event callback at a specific time
    pub fn on_time_repeating(&mut self, time: f32, callback: AnimationEventCallback) -> &mut Self {
        self.events.push(AnimationEvent {
            event_type: AnimationEventType::Repeating(time),
            callback,
            fired_this_frame: false,
            last_fire_time: None,
        });
        self
    }

    /// Register a callback when animation completes
    pub fn on_complete(&mut self, callback: AnimationEventCallback) -> &mut Self {
        self.events.push(AnimationEvent {
            event_type: AnimationEventType::Complete,
            callback,
            fired_this_frame: false,
            last_fire_time: None,
        });
        self
    }

    /// Register a callback at a progress milestone (0.0 to 1.0)
    pub fn on_progress(&mut self, progress: f32, callback: AnimationEventCallback) -> &mut Self {
        self.events.push(AnimationEvent {
            event_type: AnimationEventType::Progress(progress.clamp(0.0, 1.0)),
            callback,
            fired_this_frame: false,
            last_fire_time: None,
        });
        self
    }

    /// Register a callback for a specific keyframe
    pub fn on_keyframe(
        &mut self,
        keyframe_index: usize,
        callback: AnimationEventCallback,
    ) -> &mut Self {
        self.events.push(AnimationEvent {
            event_type: AnimationEventType::Keyframe(keyframe_index),
            callback,
            fired_this_frame: false,
            last_fire_time: None,
        });
        self
    }

    /// Add a named marker at a specific time
    pub fn add_marker(&mut self, name: String, time: f32) -> &mut Self {
        self.markers.insert(name, time);
        self
    }

    /// Register a callback for a named marker
    pub fn on_marker(
        &mut self,
        marker_name: String,
        callback: AnimationEventCallback,
    ) -> &mut Self {
        self.events.push(AnimationEvent {
            event_type: AnimationEventType::Marker(marker_name),
            callback,
            fired_this_frame: false,
            last_fire_time: None,
        });
        self
    }

    /// Remove all events matching a predicate
    pub fn remove_events<F>(&mut self, predicate: F) -> &mut Self
    where
        F: Fn(&AnimationEventType) -> bool,
    {
        self.events.retain(|event| !predicate(&event.event_type));
        self
    }

    /// Clear all registered events
    pub fn clear_events(&mut self) -> &mut Self {
        self.events.clear();
        self
    }

    /// Add a child timeline for hierarchical coordination
    pub fn add_child(&mut self, child: AnimationTimelineWithEvents) -> &mut Self {
        self.children.push(child);
        self
    }

    /// Play this timeline and all children
    pub fn play(&mut self) {
        self.timeline.play();
        for child in &mut self.children {
            child.play();
        }
    }

    /// Pause this timeline and all children
    pub fn pause(&mut self) {
        self.timeline.pause();
        for child in &mut self.children {
            child.pause();
        }
    }

    /// Stop this timeline and all children
    pub fn stop(&mut self) {
        self.timeline.stop();
        for child in &mut self.children {
            child.stop();
        }
    }

    /// Seek this timeline and all children to a specific time
    pub fn seek(&mut self, time: f32) {
        self.previous_time = self.timeline.current_time;
        self.timeline.seek(time);
        for child in &mut self.children {
            child.seek(time);
        }
    }

    /// Update timeline and fire events
    pub fn update(&mut self, delta_time: f32) -> f32 {
        let old_time = self.timeline.current_time;
        self.previous_time = old_time;

        // Calculate what the new time would be before wrapping
        let unwrapped_new_time = if matches!(self.timeline.state, AnimationPlaybackState::Playing) {
            old_time + delta_time * self.timeline.playback_rate
        } else {
            old_time
        };

        // Update timeline (may wrap due to loop)
        let new_time = self.timeline.update(delta_time);

        // Detect if we looped
        let looped = self.timeline.loop_timeline && unwrapped_new_time > self.timeline.duration;

        // Check for time crossing (handles forward, backward, and loops)
        let crossed_events = self.find_crossed_events(old_time, new_time, looped);

        // Fire events in order
        for event_index in crossed_events {
            if let Some(event) = self.events.get_mut(event_index)
                && !event.fired_this_frame
            {
                event.fired_this_frame = true;
                event.last_fire_time = Some(new_time);
                // Call the callback
                (event.callback)(&self.timeline, new_time);
            }
        }

        // Reset fired flags after processing all events
        for event in &mut self.events {
            event.fired_this_frame = false;
        }

        // Update children
        for child in &mut self.children {
            child.update(delta_time);
        }

        new_time
    }

    /// Find events that should fire based on time crossing
    fn find_crossed_events(&self, old_time: f32, new_time: f32, looped: bool) -> Vec<usize> {
        let mut crossed = Vec::new();

        for (index, event) in self.events.iter().enumerate() {
            let should_fire = match &event.event_type {
                AnimationEventType::Once(time) => {
                    // Fire only if we haven't fired before and we crossed the time
                    event.last_fire_time.is_none()
                        && self.time_crossed(old_time, new_time, *time, looped)
                }
                AnimationEventType::Repeating(time) => {
                    // Fire every time we cross the time
                    self.time_crossed(old_time, new_time, *time, looped)
                }
                AnimationEventType::Complete => {
                    // Fire when we reach the end and stop
                    matches!(self.timeline.state, AnimationPlaybackState::Stopped)
                        && new_time >= self.timeline.duration
                        && old_time < self.timeline.duration
                }
                AnimationEventType::Progress(progress) => {
                    let target_time = progress * self.timeline.duration;
                    self.time_crossed(old_time, new_time, target_time, looped)
                }
                AnimationEventType::Keyframe(_keyframe_index) => {
                    // For keyframe events, we need keyframe time information
                    // This is a placeholder - actual implementation would need keyframe data
                    false
                }
                AnimationEventType::Marker(marker_name) => {
                    if let Some(&marker_time) = self.markers.get(marker_name) {
                        self.time_crossed(old_time, new_time, marker_time, looped)
                    } else {
                        false
                    }
                }
            };

            if should_fire {
                crossed.push(index);
            }
        }

        // Sort by event time for proper ordering
        crossed.sort_by(|a, b| {
            let time_a = self.event_time(&self.events[*a].event_type);
            let time_b = self.event_time(&self.events[*b].event_type);
            time_a
                .partial_cmp(&time_b)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        crossed
    }

    /// Check if timeline crossed a specific time
    fn time_crossed(&self, old_time: f32, new_time: f32, target_time: f32, looped: bool) -> bool {
        if looped {
            // When looping forward, we crossed the time if:
            // 1. The target is between old_time and duration, OR
            // 2. The target is between 0 and new_time
            (old_time < target_time && target_time <= self.timeline.duration)
                || (0.0 <= target_time && target_time <= new_time)
        } else if old_time < new_time {
            // Forward playback
            old_time < target_time && new_time >= target_time
        } else if old_time > new_time {
            // Backward playback (negative playback rate)
            old_time > target_time && new_time <= target_time
        } else {
            // No time change
            false
        }
    }

    /// Get the time for an event type (for sorting)
    fn event_time(&self, event_type: &AnimationEventType) -> f32 {
        match event_type {
            AnimationEventType::Once(time) => *time,
            AnimationEventType::Repeating(time) => *time,
            AnimationEventType::Complete => self.timeline.duration,
            AnimationEventType::Progress(progress) => progress * self.timeline.duration,
            AnimationEventType::Keyframe(_) => 0.0, // Placeholder
            AnimationEventType::Marker(name) => self.markers.get(name).copied().unwrap_or(0.0),
        }
    }

    /// Get normalized progress (0.0 to 1.0)
    pub fn normalized_time(&self) -> f32 {
        self.timeline.normalized_time()
    }

    /// Get current playback state
    pub fn state(&self) -> &AnimationPlaybackState {
        &self.timeline.state
    }

    /// Get current time
    pub fn current_time(&self) -> f32 {
        self.timeline.current_time
    }

    /// Set playback rate (can be negative for reverse)
    pub fn set_playback_rate(&mut self, rate: f32) {
        self.timeline.set_playback_rate(rate);
    }

    /// Enable or disable looping
    pub fn enable_loop(&mut self, enable: bool) {
        self.timeline.enable_loop(enable);
    }
}

// End of Advanced Temporal Animation System

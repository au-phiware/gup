// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Mathematical scale and transformation shader functions.
//!
//! Provides GPU-accelerated scale functions (linear, logarithmic, power,
//! exponential, ordinal/band/point), filtering (clamp, threshold), and
//! interpolation (smooth step).

use super::core::*;

// ============================================================================

/// Uniform buffer layout for [`LinearScale`] and [`LinearScaleInvert`].
///
/// The struct is `#[repr(C)]` with `bytemuck::Pod` + `bytemuck::Zeroable` so it
/// can be uploaded directly to a GPU uniform buffer.  The `clamp` field uses
/// `u32` (rather than `bool`) for WGSL alignment compatibility — `0` means
/// unclamped and `1` means clamped.
///
/// Three padding fields (`_pad0`, `_pad1`, `_pad2`) round the struct up to 32
/// bytes, ensuring correct WGSL layout when the struct is embedded inside
/// `ChainUniforms` alongside types that contain `vec4<f32>` (which require
/// 16-byte alignment).
///
/// # Layout
///
/// | Offset | Field        | Type  |
/// |--------|-------------|-------|
/// | 0      | `domain_min` | `f32` |
/// | 4      | `domain_max` | `f32` |
/// | 8      | `range_min`  | `f32` |
/// | 12     | `range_max`  | `f32` |
/// | 16     | `clamp`      | `u32` |
/// | 20     | `_pad0`      | `u32` |
/// | 24     | `_pad1`      | `u32` |
/// | 28     | `_pad2`      | `u32` |
///
/// Total size: 32 bytes.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LinearScaleUniforms {
    /// Minimum domain value.
    pub domain_min: f32,
    /// Maximum domain value.
    pub domain_max: f32,
    /// Minimum range value.
    pub range_min: f32,
    /// Maximum range value.
    pub range_max: f32,
    /// 0 = unclamped (extrapolates beyond domain), 1 = clamped to range.
    pub clamp: u32,
    /// Padding for GPU alignment (must be 0).
    pub _pad0: u32,
    /// Padding for GPU alignment (must be 0).
    pub _pad1: u32,
    /// Padding for GPU alignment (must be 0).
    pub _pad2: u32,
}

impl ShaderUniform for LinearScaleUniforms {
    fn wgsl_struct_definition() -> String {
        "struct LinearScaleUniforms {\n    domain_min: f32,\n    domain_max: f32,\n    range_min: f32,\n    range_max: f32,\n    clamp_flag: u32,\n    _pad0: u32,\n    _pad1: u32,\n    _pad2: u32,\n}".to_string()
    }

    fn wgsl_type_name() -> &'static str {
        "LinearScaleUniforms"
    }
}

/// Linear scaling transformation for numeric data on the GPU.
///
/// Maps values from a data domain `[domain_min, domain_max]` to an output range
/// `[range_min, range_max]` using linear interpolation.  When clamping is
/// enabled, out-of-domain values are clamped to the output range boundaries
/// instead of extrapolating.
///
/// The generated WGSL includes both a forward function (`linear_scale`) and an
/// inverse function (`linear_scale_invert`) that maps output range values back
/// to domain values.
///
/// # Examples
///
/// ```
/// use gup::shader_function::{LinearScale, ComposableShaderFunction};
///
/// // Unclamped scale: domain [0, 100] → range [0, 1]
/// let scale = LinearScale::new(0.0, 100.0, 0.0, 1.0);
/// assert!(LinearScale::wgsl_function().contains("linear_scale"));
///
/// // Clamped scale: values outside domain are clamped to range
/// let clamped = LinearScale::with_clamp(0.0, 100.0, 0.0, 1.0);
/// let u = clamped.create_uniforms().unwrap();
/// assert_eq!(u.clamp, 1);
/// ```
#[derive(Debug, Clone)]
pub struct LinearScale {
    /// Minimum domain value.
    pub domain_min: f32,
    /// Maximum domain value.
    pub domain_max: f32,
    /// Minimum range value.
    pub range_min: f32,
    /// Maximum range value.
    pub range_max: f32,
    /// Whether to clamp the output to `[range_min, range_max]`.
    pub clamp: bool,
}

impl LinearScale {
    /// Create an unclamped linear scale.
    ///
    /// Values outside the domain are extrapolated linearly.
    pub fn new(domain_min: f32, domain_max: f32, range_min: f32, range_max: f32) -> Self {
        Self {
            domain_min,
            domain_max,
            range_min,
            range_max,
            clamp: false,
        }
    }

    /// Create a clamped linear scale.
    ///
    /// Values outside the domain are clamped to `range_min` / `range_max`.
    pub fn with_clamp(domain_min: f32, domain_max: f32, range_min: f32, range_max: f32) -> Self {
        Self {
            domain_min,
            domain_max,
            range_min,
            range_max,
            clamp: true,
        }
    }

    /// Return a [`LinearScaleInvert`] that performs the mathematical inverse of
    /// this scale, mapping output range values back to domain values.
    pub fn invert(&self) -> LinearScaleInvert {
        LinearScaleInvert {
            domain_min: self.domain_min,
            domain_max: self.domain_max,
            range_min: self.range_min,
            range_max: self.range_max,
            clamp: self.clamp,
        }
    }
}

impl ComposableShaderFunction for LinearScale {
    type Input = f32;
    type Output = f32;
    type Uniforms = LinearScaleUniforms;

    fn wgsl_function() -> &'static str {
        r#"
fn linear_scale(value: f32, scale: LinearScaleUniforms) -> f32 {
    var normalized = (value - scale.domain_min) / (scale.domain_max - scale.domain_min);
    if (scale.clamp_flag == 1u) {
        normalized = clamp(normalized, 0.0, 1.0);
    }
    return scale.range_min + normalized * (scale.range_max - scale.range_min);
}

fn linear_scale_invert(value: f32, scale: LinearScaleUniforms) -> f32 {
    var normalized = (value - scale.range_min) / (scale.range_max - scale.range_min);
    if (scale.clamp_flag == 1u) {
        normalized = clamp(normalized, 0.0, 1.0);
    }
    return scale.domain_min + normalized * (scale.domain_max - scale.domain_min);
}
        "#
    }

    fn create_uniforms(&self) -> Option<Self::Uniforms> {
        Some(LinearScaleUniforms {
            domain_min: self.domain_min,
            domain_max: self.domain_max,
            range_min: self.range_min,
            range_max: self.range_max,
            clamp: if self.clamp { 1 } else { 0 },
            _pad0: 0,
            _pad1: 0,
            _pad2: 0,
        })
    }

    fn function_name() -> &'static str {
        "linear_scale"
    }
}

/// Inverse of [`LinearScale`] — maps output range values back to domain values.
///
/// Created via [`LinearScale::invert()`].  Implements
/// [`ComposableShaderFunction`] with `Input = f32`, `Output = f32` so it can be
/// composed with other shader functions through the pipeline builder.
///
/// The underlying WGSL delegates to `linear_scale_invert` which is emitted
/// alongside `linear_scale` from the same [`LinearScale`] WGSL block.
#[derive(Debug, Clone)]
pub struct LinearScaleInvert {
    /// Minimum domain value.
    pub domain_min: f32,
    /// Maximum domain value.
    pub domain_max: f32,
    /// Minimum range value.
    pub range_min: f32,
    /// Maximum range value.
    pub range_max: f32,
    /// Whether to clamp the output.
    pub clamp: bool,
}

impl ComposableShaderFunction for LinearScaleInvert {
    type Input = f32;
    type Output = f32;
    type Uniforms = LinearScaleUniforms;

    fn wgsl_function() -> &'static str {
        // The invert function is defined together with the forward function.
        LinearScale::wgsl_function()
    }

    fn create_uniforms(&self) -> Option<Self::Uniforms> {
        Some(LinearScaleUniforms {
            domain_min: self.domain_min,
            domain_max: self.domain_max,
            range_min: self.range_min,
            range_max: self.range_max,
            clamp: if self.clamp { 1 } else { 0 },
            _pad0: 0,
            _pad1: 0,
            _pad2: 0,
        })
    }

    fn function_name() -> &'static str {
        "linear_scale_invert"
    }
}
// ============================================================================
// Additional Scale Functions (AC2: Common Transformation Functions)
// ============================================================================

/// Logarithmic scale transformation for numeric data on the GPU.
///
/// Maps values from a data domain `[domain_min, domain_max]` to an output range
/// `[range_min, range_max]` using logarithmic interpolation.  Values that span
/// multiple orders of magnitude (e.g. 1 to 1 000 000) are spread out evenly on
/// a log axis, making patterns across magnitudes visible.
///
/// # Symmetric-log mode
///
/// When `symmetric` is `true`, the scale handles negative values and zero by
/// applying `sign(x) * log_base(|x| + 1)`, preserving sign symmetry around
/// zero.  This is useful for profit-and-loss or other data that straddles zero.
///
/// # Builder API
///
/// ```
/// use gup::shader_function::LogScale;
///
/// let scale = LogScale::new(10.0)
///     .domain(1.0, 1000.0)
///     .range(0.0, 800.0);
///
/// let symlog = LogScale::new(10.0)
///     .domain(-1000.0, 1000.0)
///     .range(0.0, 800.0)
///     .symmetric(true);
/// ```
#[derive(Clone, Debug)]
pub struct LogScale {
    /// Minimum domain value.
    pub domain_min: f32,
    /// Maximum domain value.
    pub domain_max: f32,
    /// Minimum range value.
    pub range_min: f32,
    /// Maximum range value.
    pub range_max: f32,
    /// Logarithm base.
    pub base: f32,
    /// Whether to use symmetric log for negative values.
    pub is_symmetric: bool,
}

impl LogScale {
    /// Create a logarithmic scale with the given base and sensible defaults.
    ///
    /// Defaults: `domain = [1, 10]`, `range = [0, 1]`, `symmetric = false`.
    pub fn new(base: f32) -> Self {
        Self {
            domain_min: 1.0,
            domain_max: 10.0,
            range_min: 0.0,
            range_max: 1.0,
            base,
            is_symmetric: false,
        }
    }

    /// Set the input domain `[min, max]`.
    pub fn domain(mut self, min: f32, max: f32) -> Self {
        self.domain_min = min;
        self.domain_max = max;
        self
    }

    /// Set the output range `[min, max]`.
    pub fn range(mut self, min: f32, max: f32) -> Self {
        self.range_min = min;
        self.range_max = max;
        self
    }

    /// Enable or disable symmetric-log mode.
    ///
    /// When enabled, negative values are mapped as `-log_base(|x| + 1)` and
    /// zero maps to `0.0`, preserving sign symmetry around zero.
    pub fn symmetric(mut self, enabled: bool) -> Self {
        self.is_symmetric = enabled;
        self
    }

    // -- Legacy constructors (preserved for backward compatibility) -----------

    /// Creates a new logarithmic scale with base 10 (legacy four-argument form).
    pub fn base10(domain_min: f32, domain_max: f32, range_min: f32, range_max: f32) -> Self {
        Self {
            domain_min,
            domain_max,
            range_min,
            range_max,
            base: 10.0,
            is_symmetric: false,
        }
    }

    /// Creates a new logarithmic scale with natural log (base e).
    pub fn natural(domain_min: f32, domain_max: f32, range_min: f32, range_max: f32) -> Self {
        Self {
            domain_min,
            domain_max,
            range_min,
            range_max,
            base: std::f32::consts::E,
            is_symmetric: false,
        }
    }

    /// Creates a new logarithmic scale with custom base (legacy form).
    pub fn with_base(
        domain_min: f32,
        domain_max: f32,
        range_min: f32,
        range_max: f32,
        base: f32,
    ) -> Self {
        Self {
            domain_min,
            domain_max,
            range_min,
            range_max,
            base,
            is_symmetric: false,
        }
    }

    /// Evaluate the log scale on the CPU (for testing).
    ///
    /// Mirrors the WGSL `log_scale` function exactly.
    pub fn apply(&self, value: f32) -> f32 {
        let uniforms = self.create_uniforms().unwrap();
        Self::apply_uniforms(value, &uniforms)
    }

    /// CPU-side evaluation using a `LogScaleUniforms` struct.
    fn apply_uniforms(value: f32, u: &LogScaleUniforms) -> f32 {
        let inv_log2_base = 1.0 / u.base.log2();

        if u.symmetric != 0 {
            // Symmetric-log: sign(x) * log_base(|x| + 1)
            let sym_val = if value >= 0.0 {
                ((value.abs() + 1.0).log2()) * inv_log2_base
            } else {
                -((-value + 1.0).log2()) * inv_log2_base
            };
            let sym_min = if u.domain_min >= 0.0 {
                ((u.domain_min.abs() + 1.0).log2()) * inv_log2_base
            } else {
                -(((-u.domain_min) + 1.0).log2()) * inv_log2_base
            };
            let sym_max = if u.domain_max >= 0.0 {
                ((u.domain_max.abs() + 1.0).log2()) * inv_log2_base
            } else {
                -(((-u.domain_max) + 1.0).log2()) * inv_log2_base
            };
            let normalized = (sym_val - sym_min) / (sym_max - sym_min);
            u.range_min + normalized * (u.range_max - u.range_min)
        } else {
            // Standard log scale: clamp to domain_min (which must be > 0)
            // to guard against log(0) = -inf.
            let safe_min = u.domain_min.max(1e-10);
            let safe_max = u.domain_max.max(1e-10);
            let safe_value = value.max(safe_min);
            let log_min = safe_min.log2() * inv_log2_base;
            let log_max = safe_max.log2() * inv_log2_base;
            let log_value = safe_value.log2() * inv_log2_base;
            let normalized = (log_value - log_min) / (log_max - log_min);
            u.range_min + normalized * (u.range_max - u.range_min)
        }
    }
}

/// Uniform buffer layout for [`LogScale`].
///
/// The struct is `#[repr(C)]` with `bytemuck::Pod` + `bytemuck::Zeroable` so it
/// can be uploaded directly to a GPU uniform buffer.  The `symmetric` field uses
/// `u32` (rather than `bool`) for WGSL alignment compatibility — `0` means
/// standard log scale and `1` means symmetric-log.
///
/// Two padding fields (`_pad0`, `_pad1`) round the struct up to 32 bytes,
/// ensuring correct WGSL layout when embedded inside `ChainUniforms` alongside
/// types that contain `vec4<f32>` (which require 16-byte alignment).
///
/// # Layout
///
/// | Offset | Field        | Type  |
/// |--------|-------------|-------|
/// | 0      | `domain_min` | `f32` |
/// | 4      | `domain_max` | `f32` |
/// | 8      | `range_min`  | `f32` |
/// | 12     | `range_max`  | `f32` |
/// | 16     | `base`       | `f32` |
/// | 20     | `symmetric`  | `u32` |
/// | 24     | `_pad0`      | `u32` |
/// | 28     | `_pad1`      | `u32` |
///
/// Total size: 32 bytes.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LogScaleUniforms {
    /// Minimum domain value.
    pub domain_min: f32,
    /// Maximum domain value.
    pub domain_max: f32,
    /// Minimum range value.
    pub range_min: f32,
    /// Maximum range value.
    pub range_max: f32,
    /// Logarithm base.
    pub base: f32,
    /// 0 = standard log scale, 1 = symmetric-log (sign-preserving).
    pub symmetric: u32,
    /// Padding for GPU alignment (must be 0).
    pub _pad0: u32,
    /// Padding for GPU alignment (must be 0).
    pub _pad1: u32,
}

impl ShaderUniform for LogScaleUniforms {
    fn wgsl_struct_definition() -> String {
        "struct LogScaleUniforms {\n    domain_min: f32,\n    domain_max: f32,\n    range_min: f32,\n    range_max: f32,\n    base: f32,\n    symmetric: u32,\n    _pad0: u32,\n    _pad1: u32,\n}".to_string()
    }

    fn wgsl_type_name() -> &'static str {
        "LogScaleUniforms"
    }
}

impl ComposableShaderFunction for LogScale {
    type Input = f32;
    type Output = f32;
    type Uniforms = LogScaleUniforms;

    fn wgsl_function() -> &'static str {
        r#"
fn log_scale(value: f32, scale: LogScaleUniforms) -> f32 {
    let inv_log2_base = 1.0 / log2(scale.base);

    if (scale.symmetric != 0u) {
        // Symmetric-log: sign(x) * log_base(|x| + 1, base), normalised to domain.
        // Use select() to avoid sub-group divergence on non-uniform values.
        let abs_val = abs(value);
        let sym_val = select(
            -(log2(abs_val + 1.0) * inv_log2_base),
            log2(abs_val + 1.0) * inv_log2_base,
            value >= 0.0
        );
        let abs_dmin = abs(scale.domain_min);
        let sym_min = select(
            -(log2(abs_dmin + 1.0) * inv_log2_base),
            log2(abs_dmin + 1.0) * inv_log2_base,
            scale.domain_min >= 0.0
        );
        let abs_dmax = abs(scale.domain_max);
        let sym_max = select(
            -(log2(abs_dmax + 1.0) * inv_log2_base),
            log2(abs_dmax + 1.0) * inv_log2_base,
            scale.domain_max >= 0.0
        );
        let normalized = (sym_val - sym_min) / (sym_max - sym_min);
        return scale.range_min + normalized * (scale.range_max - scale.range_min);
    } else {
        // Standard log scale: clamp value to domain_min (which must be > 0)
        // to guard against log(0) = -inf and log(negative).
        let safe_min = max(scale.domain_min, 1e-10);
        let safe_max = max(scale.domain_max, 1e-10);
        let safe_value = max(value, safe_min);
        let log_min = log2(safe_min) * inv_log2_base;
        let log_max = log2(safe_max) * inv_log2_base;
        let log_value = log2(safe_value) * inv_log2_base;
        let normalized = (log_value - log_min) / (log_max - log_min);
        return scale.range_min + normalized * (scale.range_max - scale.range_min);
    }
}
        "#
    }

    fn create_uniforms(&self) -> Option<Self::Uniforms> {
        Some(LogScaleUniforms {
            domain_min: self.domain_min,
            domain_max: self.domain_max,
            range_min: self.range_min,
            range_max: self.range_max,
            base: self.base,
            symmetric: if self.is_symmetric { 1 } else { 0 },
            _pad0: 0,
            _pad1: 0,
        })
    }

    fn function_name() -> &'static str {
        "log_scale"
    }
}

// ---------------------------------------------------------------------------
// Ordinal (categorical) scales — BandScale & PointScale
// ---------------------------------------------------------------------------

/// GPU uniform layout for ordinal (categorical) scales.
///
/// Both [`BandScale`] and [`PointScale`] share this struct.  The fields are
/// pre-computed on the CPU so that the WGSL function needs only a single
/// multiply-add per invocation.
///
/// # Layout
///
/// | Offset | Field            | Type  |
/// |--------|-----------------|-------|
/// | 0      | `range_start`    | `f32` |
/// | 4      | `step_size`      | `f32` |
/// | 8      | `padding`        | `f32` |
/// | 12     | `category_count` | `u32` |
///
/// Total size: 16 bytes (naturally aligned, no explicit padding needed).
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct OrdinalScaleUniforms {
    /// Start of the output pixel range.
    pub range_start: f32,
    /// Distance from the start of one band to the start of the next.
    pub step_size: f32,
    /// Fraction of `step_size` reserved as inner padding (0.0–1.0).
    pub padding: f32,
    /// Number of categories.
    pub category_count: u32,
}

impl ShaderUniform for OrdinalScaleUniforms {
    fn wgsl_struct_definition() -> String {
        "struct OrdinalScaleUniforms {\n    range_start: f32,\n    step_size: f32,\n    padding: f32,\n    category_count: u32,\n}".to_string()
    }

    fn wgsl_type_name() -> &'static str {
        "OrdinalScaleUniforms"
    }
}

/// Band scale — maps integer category indices to the **centre** of equal-width
/// bands within a pixel range.
///
/// This is the categorical analogue of a linear scale: given *n* categories and
/// a pixel range, the range is divided into *n* equal steps.  Each band's
/// usable width is `step_size * (1.0 - padding)` and the centre of band *i* is:
///
/// ```text
/// range_start + (f32(i) + 0.5) * step_size * (1.0 - padding)
/// ```
///
/// Use [`BandScale::bandwidth()`] to obtain the band width for downstream
/// sizing (e.g. bar widths).
///
/// # Examples
///
/// ```
/// use gup::shader_function::{BandScale, OrdinalScaleUniforms, ComposableShaderFunction};
///
/// let scale = BandScale::new(0.0, 300.0, 3, 0.1);
/// assert!(BandScale::wgsl_function().contains("band_scale"));
///
/// let bw = scale.bandwidth();
/// assert!((bw - 90.0).abs() < 1e-4); // 100 * 0.9
/// ```
#[derive(Clone, Debug)]
pub struct BandScale {
    /// Start of the output range.
    pub range_start: f32,
    /// End of the output range.
    pub range_end: f32,
    /// Number of categories.
    pub category_count: u32,
    /// Padding between bands (0.0 to 1.0).
    pub padding: f32,
}

impl BandScale {
    /// Create a new band scale.
    ///
    /// * `range_start` / `range_end` — output pixel range.
    /// * `category_count` — number of categories.
    /// * `padding` — fraction of each step reserved as inner padding (0.0–1.0).
    pub fn new(range_start: f32, range_end: f32, category_count: u32, padding: f32) -> Self {
        Self {
            range_start,
            range_end,
            category_count,
            padding,
        }
    }

    /// The step size: distance from the start of one band to the next.
    pub fn step_size(&self) -> f32 {
        if self.category_count == 0 {
            return 0.0;
        }
        (self.range_end - self.range_start) / self.category_count as f32
    }

    /// The usable band width (excluding inner padding).
    ///
    /// Matches the GPU formula: `step_size * (1.0 - padding)`.
    pub fn bandwidth(&self) -> f32 {
        self.step_size() * (1.0 - self.padding)
    }

    /// Evaluate the band scale on the CPU (for testing / cross-checking).
    ///
    /// Returns the **centre** of band `index`.
    pub fn apply(&self, index: u32) -> f32 {
        let step = self.step_size();
        let bw = step * (1.0 - self.padding);
        self.range_start + index as f32 * step + bw * 0.5
    }

    /// Build the [`OrdinalScaleUniforms`] for this band scale.
    pub fn uniforms(&self) -> OrdinalScaleUniforms {
        OrdinalScaleUniforms {
            range_start: self.range_start,
            step_size: self.step_size(),
            padding: self.padding,
            category_count: self.category_count,
        }
    }
}

impl ComposableShaderFunction for BandScale {
    type Input = u32;
    type Output = f32;
    type Uniforms = OrdinalScaleUniforms;

    fn wgsl_function() -> &'static str {
        r#"
fn band_scale(index: u32, scale: OrdinalScaleUniforms) -> f32 {
    let bw = scale.step_size * (1.0 - scale.padding);
    return scale.range_start + f32(index) * scale.step_size + bw * 0.5;
}

fn band_scale_bandwidth(scale: OrdinalScaleUniforms) -> f32 {
    return scale.step_size * (1.0 - scale.padding);
}
        "#
    }

    fn create_uniforms(&self) -> Option<Self::Uniforms> {
        Some(self.uniforms())
    }

    fn function_name() -> &'static str {
        "band_scale"
    }
}

/// Point scale — distributes points evenly across a pixel range with outer
/// padding.
///
/// Unlike [`BandScale`], a point scale places each category at a single
/// coordinate (no band width).  Outer padding shifts the first and last points
/// inward by `padding * step` on each side, where *step* is calculated as:
///
/// ```text
/// step = (range_end - range_start) / (n - 1 + padding)
/// ```
///
/// For a single category the point is placed at the midpoint of the range.
///
/// # Examples
///
/// ```
/// use gup::shader_function::{PointScale, OrdinalScaleUniforms, ComposableShaderFunction};
///
/// let scale = PointScale::new(0.0, 400.0, 4, 0.5);
/// assert!(PointScale::wgsl_function().contains("point_scale"));
/// ```
#[derive(Clone, Debug)]
pub struct PointScale {
    /// Start of the output range.
    pub range_start: f32,
    /// End of the output range.
    pub range_end: f32,
    /// Number of categories.
    pub category_count: u32,
    /// Padding between points (0.0 to 1.0).
    pub padding: f32,
}

impl PointScale {
    /// Create a new point scale.
    ///
    /// * `range_start` / `range_end` — output pixel range.
    /// * `category_count` — number of categories.
    /// * `padding` — outer padding expressed as a multiple of the step size.
    pub fn new(range_start: f32, range_end: f32, category_count: u32, padding: f32) -> Self {
        Self {
            range_start,
            range_end,
            category_count,
            padding,
        }
    }

    /// The step size between adjacent points.
    pub fn step_size(&self) -> f32 {
        if self.category_count <= 1 {
            return 0.0;
        }
        let n = self.category_count as f32;
        (self.range_end - self.range_start) / (n - 1.0 + self.padding)
    }

    /// The effective start position (accounting for outer padding).
    fn effective_start(&self) -> f32 {
        if self.category_count <= 1 {
            return (self.range_start + self.range_end) / 2.0;
        }
        self.range_start + self.step_size() * self.padding / 2.0
    }

    /// Evaluate the point scale on the CPU (for testing / cross-checking).
    pub fn apply(&self, index: u32) -> f32 {
        self.effective_start() + index as f32 * self.step_size()
    }

    /// Build the [`OrdinalScaleUniforms`] for this point scale.
    ///
    /// `range_start` and `step_size` are pre-adjusted for outer padding so the
    /// WGSL function only needs `range_start + f32(i) * step_size`.
    pub fn uniforms(&self) -> OrdinalScaleUniforms {
        OrdinalScaleUniforms {
            range_start: self.effective_start(),
            step_size: self.step_size(),
            padding: self.padding,
            category_count: self.category_count,
        }
    }
}

impl ComposableShaderFunction for PointScale {
    type Input = u32;
    type Output = f32;
    type Uniforms = OrdinalScaleUniforms;

    fn wgsl_function() -> &'static str {
        r#"
fn point_scale(index: u32, scale: OrdinalScaleUniforms) -> f32 {
    return scale.range_start + f32(index) * scale.step_size;
}
        "#
    }

    fn create_uniforms(&self) -> Option<Self::Uniforms> {
        Some(self.uniforms())
    }

    fn function_name() -> &'static str {
        "point_scale"
    }
}

/// CPU-side ordinal scale that maps string category labels to integer indices.
///
/// This struct maintains a string-to-index hash map for O(1) lookups.  It can
/// produce [`BandScale`] or [`PointScale`] shader functions that use the
/// integer indices on the GPU.
///
/// # Examples
///
/// ```
/// use gup::shader_function::OrdinalScale;
///
/// let scale = OrdinalScale::from_categories(&["Apple", "Banana", "Cherry"]);
/// assert_eq!(scale.category_index("Apple"), Some(0));
/// assert_eq!(scale.category_index("Banana"), Some(1));
/// assert_eq!(scale.category_index("Cherry"), Some(2));
/// assert_eq!(scale.category_index("Durian"), None);
///
/// let band = scale.band_scale((0.0, 300.0), 0.1);
/// assert!((band.bandwidth() - 90.0).abs() < 1e-4);
/// ```
#[derive(Clone, Debug)]
pub struct OrdinalScale {
    labels: Vec<String>,
    index_map: std::collections::HashMap<String, u32>,
}

impl OrdinalScale {
    /// Build an ordinal scale from a slice of category labels.
    ///
    /// Indices are assigned in the order the labels appear.  Duplicate labels
    /// keep the first occurrence's index.
    pub fn from_categories(labels: &[&str]) -> Self {
        let mut index_map = std::collections::HashMap::with_capacity(labels.len());
        let mut unique_labels = Vec::with_capacity(labels.len());
        for &label in labels {
            let next_idx = unique_labels.len() as u32;
            if let std::collections::hash_map::Entry::Vacant(e) = index_map.entry(label.to_string())
            {
                e.insert(next_idx);
                unique_labels.push(label.to_string());
            }
        }
        Self {
            labels: unique_labels,
            index_map,
        }
    }

    /// O(1) lookup of a category's integer index.
    ///
    /// Returns `None` for labels not present in the original set.
    pub fn category_index(&self, label: &str) -> Option<u32> {
        self.index_map.get(label).copied()
    }

    /// Number of categories.
    pub fn category_count(&self) -> u32 {
        self.labels.len() as u32
    }

    /// The category labels in index order.
    pub fn labels(&self) -> &[String] {
        &self.labels
    }

    /// Produce a [`BandScale`] for the given pixel range and inner padding.
    pub fn band_scale(&self, range: (f32, f32), padding: f32) -> BandScale {
        BandScale::new(range.0, range.1, self.category_count(), padding)
    }

    /// Produce a [`PointScale`] for the given pixel range and outer padding.
    pub fn point_scale(&self, range: (f32, f32), padding: f32) -> PointScale {
        PointScale::new(range.0, range.1, self.category_count(), padding)
    }

    /// Produce the GPU uniform struct for a band scale configuration.
    pub fn uniforms(&self, range: (f32, f32), padding: f32) -> OrdinalScaleUniforms {
        self.band_scale(range, padding).uniforms()
    }
}

/// Power scale transformation (exponential scaling).
///
/// Maps values using a power function: output = (normalized_input)^exponent.
/// Exponent < 1 compresses high values, > 1 expands them.
#[derive(Clone, Debug)]
pub struct PowerScale {
    /// Minimum domain value.
    pub domain_min: f32,
    /// Maximum domain value.
    pub domain_max: f32,
    /// Minimum range value.
    pub range_min: f32,
    /// Maximum range value.
    pub range_max: f32,
    /// Exponent for the power curve.
    pub exponent: f32,
}

impl PowerScale {
    /// Creates a new power scale with the given parameters.
    pub fn new(
        domain_min: f32,
        domain_max: f32,
        range_min: f32,
        range_max: f32,
        exponent: f32,
    ) -> Self {
        Self {
            domain_min,
            domain_max,
            range_min,
            range_max,
            exponent,
        }
    }

    /// Creates a square root scale (exponent = 0.5).
    pub fn sqrt(domain_min: f32, domain_max: f32, range_min: f32, range_max: f32) -> Self {
        Self::new(domain_min, domain_max, range_min, range_max, 0.5)
    }

    /// Creates a square scale (exponent = 2.0).
    pub fn square(domain_min: f32, domain_max: f32, range_min: f32, range_max: f32) -> Self {
        Self::new(domain_min, domain_max, range_min, range_max, 2.0)
    }
}

/// GPU uniform data for the power scale shader function.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PowerScaleUniforms {
    /// Minimum domain value.
    pub domain_min: f32,
    /// Maximum domain value.
    pub domain_max: f32,
    /// Minimum range value.
    pub range_min: f32,
    /// Maximum range value.
    pub range_max: f32,
    /// Exponent for the power curve.
    pub exponent: f32,
    /// Padding for GPU alignment.
    pub _padding: [f32; 3],
}

impl ShaderUniform for PowerScaleUniforms {
    fn wgsl_struct_definition() -> String {
        "struct PowerScaleUniforms {\n    domain_min: f32,\n    domain_max: f32,\n    range_min: f32,\n    range_max: f32,\n    exponent: f32,\n}".to_string()
    }

    fn wgsl_type_name() -> &'static str {
        "PowerScaleUniforms"
    }
}

impl ComposableShaderFunction for PowerScale {
    type Input = f32;
    type Output = f32;
    type Uniforms = PowerScaleUniforms;

    fn wgsl_function() -> &'static str {
        r#"
        fn power_scale(value: f32, scale: PowerScaleUniforms) -> f32 {
            let normalized = (value - scale.domain_min) / (scale.domain_max - scale.domain_min);
            let powered = pow(max(normalized, 0.0), scale.exponent);
            return scale.range_min + powered * (scale.range_max - scale.range_min);
        }
        "#
    }

    fn create_uniforms(&self) -> Option<Self::Uniforms> {
        Some(PowerScaleUniforms {
            domain_min: self.domain_min,
            domain_max: self.domain_max,
            range_min: self.range_min,
            range_max: self.range_max,
            exponent: self.exponent,
            _padding: [0.0; 3],
        })
    }

    fn function_name() -> &'static str {
        "power_scale"
    }
}

/// Exponential scale transformation.
///
/// Maps values from a domain to a range using exponential scaling.
/// The inverse of logarithmic scale — useful for emphasizing differences
/// in large values while compressing small value differences.
#[derive(Clone, Debug)]
pub struct ExponentialScale {
    /// Minimum domain value.
    pub domain_min: f32,
    /// Maximum domain value.
    pub domain_max: f32,
    /// Minimum range value.
    pub range_min: f32,
    /// Maximum range value.
    pub range_max: f32,
    /// Base for the exponential curve.
    pub base: f32,
}

impl ExponentialScale {
    /// Creates a new exponential scale with the given parameters.
    pub fn new(
        domain_min: f32,
        domain_max: f32,
        range_min: f32,
        range_max: f32,
        base: f32,
    ) -> Self {
        Self {
            domain_min,
            domain_max,
            range_min,
            range_max,
            base,
        }
    }

    /// Creates an exponential scale with base 10.
    pub fn base10(domain_min: f32, domain_max: f32, range_min: f32, range_max: f32) -> Self {
        Self::new(domain_min, domain_max, range_min, range_max, 10.0)
    }

    /// Creates an exponential scale with base e.
    pub fn natural(domain_min: f32, domain_max: f32, range_min: f32, range_max: f32) -> Self {
        Self::new(
            domain_min,
            domain_max,
            range_min,
            range_max,
            std::f32::consts::E,
        )
    }
}

/// GPU uniform data for the exponential scale shader function.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ExponentialScaleUniforms {
    /// Minimum domain value.
    pub domain_min: f32,
    /// Maximum domain value.
    pub domain_max: f32,
    /// Minimum range value.
    pub range_min: f32,
    /// Maximum range value.
    pub range_max: f32,
    /// Base for the exponential curve.
    pub base: f32,
    /// Padding for GPU alignment.
    pub _padding: [f32; 3],
}

impl ShaderUniform for ExponentialScaleUniforms {
    fn wgsl_struct_definition() -> String {
        "struct ExponentialScaleUniforms {\n    domain_min: f32,\n    domain_max: f32,\n    range_min: f32,\n    range_max: f32,\n    base: f32,\n}".to_string()
    }

    fn wgsl_type_name() -> &'static str {
        "ExponentialScaleUniforms"
    }
}

impl ComposableShaderFunction for ExponentialScale {
    type Input = f32;
    type Output = f32;
    type Uniforms = ExponentialScaleUniforms;

    fn wgsl_function() -> &'static str {
        r#"
        fn exponential_scale(value: f32, scale: ExponentialScaleUniforms) -> f32 {
            let normalized = (value - scale.domain_min) / (scale.domain_max - scale.domain_min);
            let t = clamp(normalized, 0.0, 1.0);
            let exp_value = (pow(scale.base, t) - 1.0) / (scale.base - 1.0);
            return scale.range_min + exp_value * (scale.range_max - scale.range_min);
        }
        "#
    }

    fn create_uniforms(&self) -> Option<Self::Uniforms> {
        Some(ExponentialScaleUniforms {
            domain_min: self.domain_min,
            domain_max: self.domain_max,
            range_min: self.range_min,
            range_max: self.range_max,
            base: self.base,
            _padding: [0.0; 3],
        })
    }

    fn function_name() -> &'static str {
        "exponential_scale"
    }
}

// ============================================================================
// Filtering and Clamping Functions (AC2)
// ============================================================================

/// Clamps values to a specified range.
#[derive(Clone, Debug)]
pub struct Clamp {
    /// Minimum clamp value.
    pub min: f32,
    /// Maximum clamp value.
    pub max: f32,
}

impl Clamp {
    /// Creates a new clamp function with the given bounds.
    pub fn new(min: f32, max: f32) -> Self {
        Self { min, max }
    }
}

/// GPU uniform data for the clamp shader function.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ClampUniforms {
    /// Minimum clamp value.
    pub min: f32,
    /// Maximum clamp value.
    pub max: f32,
    /// Padding for GPU alignment.
    pub _padding: [f32; 2],
}

impl ShaderUniform for ClampUniforms {
    fn wgsl_struct_definition() -> String {
        "struct ClampUniforms {\n    min: f32,\n    max: f32,\n}".to_string()
    }

    fn wgsl_type_name() -> &'static str {
        "ClampUniforms"
    }
}

impl ComposableShaderFunction for Clamp {
    type Input = f32;
    type Output = f32;
    type Uniforms = ClampUniforms;

    fn wgsl_function() -> &'static str {
        r#"
        fn clamp_fn(value: f32, params: ClampUniforms) -> f32 {
            return clamp(value, params.min, params.max);
        }
        "#
    }

    fn create_uniforms(&self) -> Option<Self::Uniforms> {
        Some(ClampUniforms {
            min: self.min,
            max: self.max,
            _padding: [0.0; 2],
        })
    }

    fn function_name() -> &'static str {
        "clamp_fn"
    }
}

/// Threshold function - outputs 0 or 1 based on threshold.
#[derive(Clone, Debug)]
pub struct Threshold {
    /// Threshold value.
    pub threshold: f32,
}

impl Threshold {
    /// Creates a new threshold function with the given value.
    pub fn new(threshold: f32) -> Self {
        Self { threshold }
    }
}

/// GPU uniform data for the threshold shader function.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ThresholdUniforms {
    /// Threshold value.
    pub threshold: f32,
    /// Padding for GPU alignment.
    pub _padding: [f32; 3],
}

impl ShaderUniform for ThresholdUniforms {
    fn wgsl_struct_definition() -> String {
        "struct ThresholdUniforms {\n    threshold: f32,\n}".to_string()
    }

    fn wgsl_type_name() -> &'static str {
        "ThresholdUniforms"
    }
}

impl ComposableShaderFunction for Threshold {
    type Input = f32;
    type Output = f32;
    type Uniforms = ThresholdUniforms;

    fn wgsl_function() -> &'static str {
        r#"
        fn threshold_fn(value: f32, params: ThresholdUniforms) -> f32 {
            return select(0.0, 1.0, value >= params.threshold);
        }
        "#
    }

    fn create_uniforms(&self) -> Option<Self::Uniforms> {
        Some(ThresholdUniforms {
            threshold: self.threshold,
            _padding: [0.0; 3],
        })
    }

    fn function_name() -> &'static str {
        "threshold_fn"
    }
}

// ============================================================================
// Interpolation Functions (AC2)
// ============================================================================

/// Smooth step interpolation (ease-in-ease-out).
#[derive(Clone, Debug)]
pub struct SmoothStep {
    /// Lower edge of the transition.
    pub edge0: f32,
    /// Upper edge of the transition.
    pub edge1: f32,
}

impl SmoothStep {
    /// Creates a new smooth step function with the given edges.
    pub fn new(edge0: f32, edge1: f32) -> Self {
        Self { edge0, edge1 }
    }
}

/// GPU uniform data for the smooth step shader function.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SmoothStepUniforms {
    /// Lower edge of the transition.
    pub edge0: f32,
    /// Upper edge of the transition.
    pub edge1: f32,
    /// Padding for GPU alignment.
    pub _padding: [f32; 2],
}

impl ShaderUniform for SmoothStepUniforms {
    fn wgsl_struct_definition() -> String {
        "struct SmoothStepUniforms {\n    edge0: f32,\n    edge1: f32,\n}".to_string()
    }

    fn wgsl_type_name() -> &'static str {
        "SmoothStepUniforms"
    }
}

impl ComposableShaderFunction for SmoothStep {
    type Input = f32;
    type Output = f32;
    type Uniforms = SmoothStepUniforms;

    fn wgsl_function() -> &'static str {
        r#"
        fn smooth_step_fn(value: f32, params: SmoothStepUniforms) -> f32 {
            return smoothstep(params.edge0, params.edge1, value);
        }
        "#
    }

    fn create_uniforms(&self) -> Option<Self::Uniforms> {
        Some(SmoothStepUniforms {
            edge0: self.edge0,
            edge1: self.edge1,
            _padding: [0.0; 2],
        })
    }

    fn function_name() -> &'static str {
        "smooth_step_fn"
    }
}

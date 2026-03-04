// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Color mapping, gradient, and color space shader functions.
//!
//! Provides GPU-accelerated color transformations including simple color maps,
//! multi-stop gradients, domain-to-colour scales, HSV mapping, alpha blending,
//! RGB/HSL color space conversion, and perceptual (LAB/OKLab/LCH) color spaces.

use super::core::*;

/// GPU uniform data for the colour map shader function.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ColorMapUniforms {
    /// Minimum colour (RGBA).
    pub min_color: [f32; 4],
    /// Maximum colour (RGBA).
    pub max_color: [f32; 4],
}

impl ShaderUniform for ColorMapUniforms {
    fn wgsl_struct_definition() -> String {
        "struct ColorMapUniforms {\n    min_color: vec4<f32>,\n    max_color: vec4<f32>,\n}"
            .to_string()
    }

    fn wgsl_type_name() -> &'static str {
        "ColorMapUniforms"
    }
}

/// Simple two-color linear interpolation for data visualization.
///
/// This is a basic example shader function. Advanced color mapping features
/// (HSV color space, multi-stop gradients, color space conversions) will be added in future updates.
pub struct ColorMap {
    /// Minimum colour value.
    pub min_color: Vec4,
    /// Maximum colour value.
    pub max_color: Vec4,
}

impl ColorMap {
    /// Creates a new colour map with the given colour range.
    pub fn new(min_color: Vec4, max_color: Vec4) -> Self {
        Self {
            min_color,
            max_color,
        }
    }
}

impl ComposableShaderFunction for ColorMap {
    type Input = f32;
    type Output = Vec4;
    type Uniforms = ColorMapUniforms;

    fn wgsl_function() -> &'static str {
        r#"
        fn color_map(value: f32, colors: ColorMapUniforms) -> vec4<f32> {
            let t = clamp(value, 0.0, 1.0);
            return mix(vec4<f32>(colors.min_color), vec4<f32>(colors.max_color), t);
        }
        "#
    }

    fn create_uniforms(&self) -> Option<Self::Uniforms> {
        Some(ColorMapUniforms {
            min_color: [
                self.min_color.x,
                self.min_color.y,
                self.min_color.z,
                self.min_color.w,
            ],
            max_color: [
                self.max_color.x,
                self.max_color.y,
                self.max_color.z,
                self.max_color.w,
            ],
        })
    }

    fn function_name() -> &'static str {
        "color_map"
    }
}

/// Multi-point color interpolation (gradient with multiple stops).
#[derive(Clone, Debug)]
pub struct ColorGradient {
    /// Colour stops as RGBA values.
    pub colors: Vec<Vec4>,
    /// Positions of each colour stop (0.0 to 1.0).
    pub stops: Vec<f32>,
}

impl ColorGradient {
    /// Creates a new colour gradient with the given colours and stops.
    pub fn new(colors: Vec<Vec4>, stops: Vec<f32>) -> Self {
        assert_eq!(
            colors.len(),
            stops.len(),
            "Colors and stops must have same length"
        );
        assert!(!colors.is_empty(), "Must have at least one color");
        Self { colors, stops }
    }

    /// Creates a gradient with evenly spaced stops.
    pub fn with_colors(colors: Vec<Vec4>) -> Self {
        let count = colors.len();
        let stops = (0..count)
            .map(|i| i as f32 / (count - 1).max(1) as f32)
            .collect();
        Self { colors, stops }
    }
}

// For now, we'll use a simplified uniform that supports up to 8 color stops
// A more advanced implementation would use storage buffers for arbitrary length
/// GPU uniform data for the colour gradient shader function (up to 8 stops).
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ColorGradientUniforms {
    /// Colour stop values (RGBA, up to 8).
    pub colors: [[f32; 4]; 8],
    /// Position of each colour stop.
    pub stops: [f32; 8],
    /// Number of active colour stops.
    pub count: u32,
    /// Padding for GPU alignment.
    pub _padding: [f32; 3],
}

impl ShaderUniform for ColorGradientUniforms {
    fn wgsl_struct_definition() -> String {
        "struct ColorGradientUniforms {\n    colors: array<vec4<f32>, 8>,\n    stops: array<f32, 8>,\n    count: u32,\n}".to_string()
    }

    fn wgsl_type_name() -> &'static str {
        "ColorGradientUniforms"
    }
}

impl ComposableShaderFunction for ColorGradient {
    type Input = f32;
    type Output = Vec4;
    type Uniforms = ColorGradientUniforms;

    fn wgsl_function() -> &'static str {
        r#"
        fn color_gradient(value: f32, gradient: ColorGradientUniforms) -> vec4<f32> {
            let t = clamp(value, 0.0, 1.0);

            // Handle single color
            if (gradient.count == 1u) {
                return gradient.colors[0];
            }

            // Find the two stops to interpolate between
            var i = 0u;
            for (i = 0u; i < gradient.count - 1u; i = i + 1u) {
                if (t <= gradient.stops[i + 1u]) {
                    break;
                }
            }

            // Interpolate between the two colors
            let t0 = gradient.stops[i];
            let t1 = gradient.stops[i + 1u];
            let local_t = (t - t0) / (t1 - t0);

            return mix(gradient.colors[i], gradient.colors[i + 1u], local_t);
        }
        "#
    }

    fn create_uniforms(&self) -> Option<Self::Uniforms> {
        let count = self.colors.len().min(8);
        let mut colors = [[0.0f32; 4]; 8];
        let mut stops = [0.0f32; 8];

        for i in 0..count {
            colors[i] = [
                self.colors[i].x,
                self.colors[i].y,
                self.colors[i].z,
                self.colors[i].w,
            ];
            stops[i] = self.stops[i];
        }

        Some(ColorGradientUniforms {
            colors,
            stops,
            count: count as u32,
            _padding: [0.0; 3],
        })
    }

    fn function_name() -> &'static str {
        "color_gradient"
    }
}

/// Storage buffer-based color gradient supporting unlimited color stops.
///
/// Unlike the uniform-based `ColorGradient` which is limited to 8 stops,
/// this implementation uses storage buffers to support arbitrary numbers of color stops.
/// Uses efficient binary search in WGSL for stop lookup.
#[derive(Clone, Debug)]
pub struct ColorGradientStorage {
    /// Colour stops as RGBA values.
    pub colors: Vec<Vec4>,
    /// Positions of each colour stop (0.0 to 1.0).
    pub stops: Vec<f32>,
}

impl ColorGradientStorage {
    /// Creates a new gradient with explicit color stops.
    pub fn new(colors: Vec<Vec4>, stops: Vec<f32>) -> Self {
        assert_eq!(
            colors.len(),
            stops.len(),
            "Colors and stops must have same length"
        );
        assert!(!colors.is_empty(), "Must have at least one color");
        Self { colors, stops }
    }

    /// Creates a gradient with evenly spaced stops.
    pub fn with_colors(colors: Vec<Vec4>) -> Self {
        let count = colors.len();
        let stops = (0..count)
            .map(|i| i as f32 / (count - 1).max(1) as f32)
            .collect();
        Self { colors, stops }
    }

    /// Returns a builder for creating gradients.
    pub fn builder() -> ColorGradientBuilder {
        ColorGradientBuilder::new()
    }

    /// Creates the Viridis color gradient (perceptually uniform, colorblind-friendly).
    pub fn viridis() -> Self {
        Self::with_colors(vec![
            vec4![0.267004, 0.004874, 0.329415, 1.0],
            vec4![0.282623, 0.140926, 0.457517, 1.0],
            vec4![0.253935, 0.265254, 0.529983, 1.0],
            vec4![0.206756, 0.371758, 0.553117, 1.0],
            vec4![0.163625, 0.471133, 0.558148, 1.0],
            vec4![0.127568, 0.566949, 0.550556, 1.0],
            vec4![0.134692, 0.658636, 0.517649, 1.0],
            vec4![0.266941, 0.748751, 0.440573, 1.0],
            vec4![0.477504, 0.821444, 0.318195, 1.0],
            vec4![0.741388, 0.873449, 0.149561, 1.0],
            vec4![0.993248, 0.906157, 0.143936, 1.0],
        ])
    }

    /// Creates the Plasma color gradient (bright, vibrant, perceptually uniform).
    pub fn plasma() -> Self {
        Self::with_colors(vec![
            vec4![0.050383, 0.029803, 0.527975, 1.0],
            vec4![0.230556, 0.012923, 0.627545, 1.0],
            vec4![0.401315, 0.000564, 0.658149, 1.0],
            vec4![0.562738, 0.051545, 0.641509, 1.0],
            vec4![0.706680, 0.165141, 0.564522, 1.0],
            vec4![0.828139, 0.283102, 0.461594, 1.0],
            vec4![0.920354, 0.417642, 0.338648, 1.0],
            vec4![0.980260, 0.573940, 0.215906, 1.0],
            vec4![0.991043, 0.746138, 0.137562, 1.0],
            vec4![0.949368, 0.922887, 0.144767, 1.0],
            vec4![0.940015, 0.975158, 0.131326, 1.0],
        ])
    }

    /// Creates the Inferno color gradient (dark to bright, warm colors).
    pub fn inferno() -> Self {
        Self::with_colors(vec![
            vec4![0.001462, 0.000466, 0.013866, 1.0],
            vec4![0.087411, 0.044556, 0.224813, 1.0],
            vec4![0.258234, 0.038571, 0.406485, 1.0],
            vec4![0.461407, 0.075611, 0.437064, 1.0],
            vec4![0.652443, 0.136307, 0.405923, 1.0],
            vec4![0.816442, 0.223710, 0.331061, 1.0],
            vec4![0.930395, 0.358711, 0.229521, 1.0],
            vec4![0.986163, 0.543537, 0.142718, 1.0],
            vec4![0.977201, 0.747849, 0.164568, 1.0],
            vec4![0.929898, 0.937506, 0.349556, 1.0],
            vec4![0.988362, 0.998364, 0.644924, 1.0],
        ])
    }

    /// Creates a simple rainbow gradient.
    pub fn rainbow() -> Self {
        Self::with_colors(vec![
            vec4![1.0, 0.0, 0.0, 1.0],     // Red
            vec4![1.0, 0.5, 0.0, 1.0],     // Orange
            vec4![1.0, 1.0, 0.0, 1.0],     // Yellow
            vec4![0.0, 1.0, 0.0, 1.0],     // Green
            vec4![0.0, 0.0, 1.0, 1.0],     // Blue
            vec4![0.294, 0.0, 0.510, 1.0], // Indigo
            vec4![0.561, 0.0, 1.0, 1.0],   // Violet
        ])
    }

    /// Creates a cool to warm gradient (blue to red).
    pub fn cool_warm() -> Self {
        Self::with_colors(vec![
            vec4![0.0, 0.0, 1.0, 1.0], // Blue
            vec4![0.0, 0.5, 1.0, 1.0], // Light blue
            vec4![1.0, 1.0, 1.0, 1.0], // White
            vec4![1.0, 0.5, 0.0, 1.0], // Orange
            vec4![1.0, 0.0, 0.0, 1.0], // Red
        ])
    }

    /// Creates a grayscale gradient.
    pub fn grayscale() -> Self {
        Self::with_colors(vec![
            vec4![0.0, 0.0, 0.0, 1.0], // Black
            vec4![1.0, 1.0, 1.0, 1.0], // White
        ])
    }

    /// Creates buffer data for colors.
    pub fn create_colors_buffer_data(&self) -> Vec<u8> {
        let mut data = Vec::with_capacity(self.colors.len() * 16);
        for color in &self.colors {
            data.extend_from_slice(&color.x.to_le_bytes());
            data.extend_from_slice(&color.y.to_le_bytes());
            data.extend_from_slice(&color.z.to_le_bytes());
            data.extend_from_slice(&color.w.to_le_bytes());
        }
        data
    }

    /// Creates buffer data for stops.
    pub fn create_stops_buffer_data(&self) -> Vec<u8> {
        let mut data = Vec::with_capacity(self.stops.len() * 4);
        for stop in &self.stops {
            data.extend_from_slice(&stop.to_le_bytes());
        }
        data
    }

    /// Returns the number of color stops.
    pub fn count(&self) -> u32 {
        self.colors.len() as u32
    }

    /// Returns the WGSL struct definition for the storage buffer.
    pub fn wgsl_struct_definition() -> &'static str {
        r#"
struct ColorGradientStorage {
    count: u32,
}

@group(0) @binding(1) var<storage, read> gradient_colors: array<vec4<f32>>;
@group(0) @binding(2) var<storage, read> gradient_stops: array<f32>;
@group(0) @binding(3) var<uniform> gradient_info: ColorGradientStorage;
"#
    }

    /// Returns the WGSL function implementation with efficient binary search.
    pub fn wgsl_function() -> &'static str {
        r#"
fn color_gradient_storage(value: f32) -> vec4<f32> {
    let t = clamp(value, 0.0, 1.0);
    let count = gradient_info.count;

    // Handle single color
    if (count == 1u) {
        return gradient_colors[0];
    }

    // Handle edge cases
    if (t <= gradient_stops[0]) {
        return gradient_colors[0];
    }
    if (t >= gradient_stops[count - 1u]) {
        return gradient_colors[count - 1u];
    }

    // Binary search for the correct stop range
    var low = 0u;
    var high = count - 1u;

    // Find the interval containing t
    while (low + 1u < high) {
        let mid = (low + high) / 2u;
        if (gradient_stops[mid] <= t) {
            low = mid;
        } else {
            high = mid;
        }
    }

    // Interpolate between the two colors
    let t0 = gradient_stops[low];
    let t1 = gradient_stops[high];
    let local_t = (t - t0) / (t1 - t0);

    return mix(gradient_colors[low], gradient_colors[high], local_t);
}
"#
    }
}

/// Builder for creating color gradients with a fluent API.
pub struct ColorGradientBuilder {
    stops: Vec<(f32, Vec4)>,
}

impl ColorGradientBuilder {
    /// Creates a new gradient builder.
    pub fn new() -> Self {
        Self { stops: Vec::new() }
    }

    /// Adds a color stop at the specified position (0.0 to 1.0).
    pub fn add_stop(mut self, position: f32, color: Vec4) -> Self {
        assert!(
            (0.0..=1.0).contains(&position),
            "Stop position must be between 0.0 and 1.0"
        );
        self.stops.push((position, color));
        self
    }

    /// Adds a color stop with RGB values (alpha = 1.0).
    pub fn add_rgb(self, position: f32, r: f32, g: f32, b: f32) -> Self {
        self.add_stop(position, vec4![r, g, b, 1.0])
    }

    /// Adds a color stop with RGBA values.
    pub fn add_rgba(self, position: f32, r: f32, g: f32, b: f32, a: f32) -> Self {
        self.add_stop(position, vec4![r, g, b, a])
    }

    /// Builds the gradient, sorting stops by position.
    pub fn build(mut self) -> ColorGradientStorage {
        assert!(
            !self.stops.is_empty(),
            "Gradient must have at least one stop"
        );

        // Sort by position
        self.stops.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

        let (positions, colors): (Vec<f32>, Vec<Vec4>) = self.stops.into_iter().unzip();
        ColorGradientStorage::new(colors, positions)
    }
}

impl Default for ColorGradientBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// ColorScale — domain → colour ShaderFunction (GUP-255)
// ============================================================================

/// The kind of mapping a [`ColorScale`] applies.
///
/// - `Continuous` — normalises linearly over the full domain.
/// - `Diverging` — normalises in two halves around a midpoint.
/// - `Quantize` — snaps to one of `n_bins` equal-width buckets.
#[derive(Clone, Debug, PartialEq)]
pub enum ColorScaleKind {
    /// Linearly normalise over the full domain.
    Continuous,
    /// Piecewise-linear normalisation around `midpoint`.
    Diverging {
        /// The domain value that maps to the centre of the gradient (0.5).
        midpoint: f32,
    },
    /// Snap the normalised value into one of `n_bins` equal-width buckets.
    Quantize {
        /// Number of discrete colour bins.
        n_bins: u32,
    },
}

/// GPU-side uniform block for [`ColorScale`].
///
/// Uploaded to the GPU as a uniform buffer alongside the storage-buffer colour
/// and stop arrays from [`ColorGradientStorage`].
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ColorScaleUniforms {
    /// Minimum domain value.
    pub domain_min: f32,
    /// Maximum domain value.
    pub domain_max: f32,
    /// Midpoint domain value (only meaningful when `scale_kind == 1`).
    pub midpoint: f32,
    /// 0 = continuous, 1 = diverging, 2 = quantize.
    pub scale_kind: u32,
    /// Number of discrete bins (only meaningful when `scale_kind == 2`).
    pub n_bins: u32,
    /// Number of gradient colour stops.
    pub stop_count: u32,
    /// Padding for 16-byte alignment.
    pub _pad0: u32,
    /// Padding for 16-byte alignment.
    pub _pad1: u32,
}

impl ShaderUniform for ColorScaleUniforms {
    fn wgsl_struct_definition() -> String {
        "struct ColorScaleUniforms {\n    domain_min: f32,\n    domain_max: f32,\n    midpoint: f32,\n    scale_kind: u32,\n    n_bins: u32,\n    stop_count: u32,\n    _pad0: u32,\n    _pad1: u32,\n}"
            .to_string()
    }

    fn wgsl_type_name() -> &'static str {
        "ColorScaleUniforms"
    }
}

/// A composable shader function that maps a numeric domain value to an RGBA
/// colour on the GPU.
///
/// `ColorScale` wraps a [`ColorGradientStorage`] palette and adds domain
/// normalisation so that the mapping can be expressed as a single
/// [`ComposableShaderFunction`] with `Input = f32` and `Output = Vec4`.
///
/// Three mapping modes are supported:
///
/// - **Continuous** — linearly interpolates over the whole domain.
/// - **Diverging** — splits the domain at a midpoint so that values below the
///   midpoint use the first half of the gradient and values above use the
///   second half.
/// - **Quantize** — divides the domain into `n_bins` equal-width buckets and
///   snaps each input to the bucket's colour.
///
/// # Built-in palettes
///
/// ```
/// use gup::shader_function::ColorScale;
///
/// let viridis = ColorScale::viridis(0.0, 100.0);
/// let plasma  = ColorScale::plasma(0.0, 1.0);
/// let inferno = ColorScale::inferno(-10.0, 40.0);
/// let magma   = ColorScale::magma(0.0, 255.0);
/// let rd_bu   = ColorScale::rd_bu(-1.0, 1.0);
/// ```
///
/// # Diverging scale
///
/// ```
/// use gup::shader_function::ColorScale;
///
/// let div = ColorScale::diverging(
///     ColorScale::rd_bu_gradient(),
///     -5.0,  // domain_min
///     0.0,   // midpoint
///     10.0,  // domain_max
/// );
/// ```
///
/// # Quantize (discrete) scale
///
/// ```
/// use gup::shader_function::ColorScale;
///
/// let quant = ColorScale::quantize(
///     ColorScale::viridis_gradient(),
///     (0.0, 100.0), // domain
///     5,            // number of bins
/// );
/// ```
#[derive(Clone, Debug)]
pub struct ColorScale {
    /// The underlying gradient palette.
    pub gradient: ColorGradientStorage,
    /// Minimum domain value.
    pub domain_min: f32,
    /// Maximum domain value.
    pub domain_max: f32,
    /// What kind of mapping to apply.
    pub kind: ColorScaleKind,
}

impl ColorScale {
    // ------------------------------------------------------------------
    // Core constructors
    // ------------------------------------------------------------------

    /// Create a continuous colour scale from an arbitrary gradient and domain.
    pub fn new(gradient: ColorGradientStorage, domain: (f32, f32)) -> Self {
        Self {
            gradient,
            domain_min: domain.0,
            domain_max: domain.1,
            kind: ColorScaleKind::Continuous,
        }
    }

    /// Create a diverging colour scale that maps the midpoint to the exact
    /// centre (0.5) of the gradient.
    pub fn diverging(
        gradient: ColorGradientStorage,
        domain_min: f32,
        midpoint: f32,
        domain_max: f32,
    ) -> Self {
        Self {
            gradient,
            domain_min,
            domain_max,
            kind: ColorScaleKind::Diverging { midpoint },
        }
    }

    /// Create a discrete (quantize) colour scale with `n_bins` equal-width
    /// buckets.
    pub fn quantize(gradient: ColorGradientStorage, domain: (f32, f32), n_bins: u32) -> Self {
        assert!(n_bins > 0, "n_bins must be > 0");
        Self {
            gradient,
            domain_min: domain.0,
            domain_max: domain.1,
            kind: ColorScaleKind::Quantize { n_bins },
        }
    }

    // ------------------------------------------------------------------
    // Palette gradient helpers (no domain — for diverging/quantize ctors)
    // ------------------------------------------------------------------

    /// Viridis gradient (perceptually uniform, colorblind-friendly).
    pub fn viridis_gradient() -> ColorGradientStorage {
        ColorGradientStorage::viridis()
    }

    /// Plasma gradient (bright, vibrant, perceptually uniform).
    pub fn plasma_gradient() -> ColorGradientStorage {
        ColorGradientStorage::plasma()
    }

    /// Inferno gradient (dark-to-bright warm ramp).
    pub fn inferno_gradient() -> ColorGradientStorage {
        ColorGradientStorage::inferno()
    }

    /// Magma gradient (dark-to-bright muted ramp).
    pub fn magma_gradient() -> ColorGradientStorage {
        Self::magma_gradient_data()
    }

    /// Red–Blue diverging gradient.
    pub fn rd_bu_gradient() -> ColorGradientStorage {
        Self::rd_bu_gradient_data()
    }

    // ------------------------------------------------------------------
    // Built-in palette constructors (continuous, with domain)
    // ------------------------------------------------------------------

    /// Viridis continuous colour scale — perceptually uniform, colorblind-friendly.
    pub fn viridis(domain_min: f32, domain_max: f32) -> Self {
        Self::new(ColorGradientStorage::viridis(), (domain_min, domain_max))
    }

    /// Plasma continuous colour scale — bright, vibrant, perceptually uniform.
    pub fn plasma(domain_min: f32, domain_max: f32) -> Self {
        Self::new(ColorGradientStorage::plasma(), (domain_min, domain_max))
    }

    /// Inferno continuous colour scale — dark-to-bright warm ramp.
    pub fn inferno(domain_min: f32, domain_max: f32) -> Self {
        Self::new(ColorGradientStorage::inferno(), (domain_min, domain_max))
    }

    /// Magma continuous colour scale — dark-to-bright muted ramp.
    pub fn magma(domain_min: f32, domain_max: f32) -> Self {
        Self::new(Self::magma_gradient_data(), (domain_min, domain_max))
    }

    /// Red–Blue diverging colour scale.
    pub fn rd_bu(domain_min: f32, domain_max: f32) -> Self {
        Self::new(Self::rd_bu_gradient_data(), (domain_min, domain_max))
    }

    // ------------------------------------------------------------------
    // Storage buffer data helpers (delegates to inner gradient)
    // ------------------------------------------------------------------

    /// Returns the raw colour data formatted for a GPU storage buffer.
    pub fn create_colors_buffer_data(&self) -> Vec<u8> {
        self.gradient.create_colors_buffer_data()
    }

    /// Returns the raw stop-position data formatted for a GPU storage buffer.
    pub fn create_stops_buffer_data(&self) -> Vec<u8> {
        self.gradient.create_stops_buffer_data()
    }

    // ------------------------------------------------------------------
    // Internal palette data
    // ------------------------------------------------------------------

    /// Magma palette stop data (11 samples).
    fn magma_gradient_data() -> ColorGradientStorage {
        ColorGradientStorage::with_colors(vec![
            vec4![0.001462, 0.000466, 0.013866, 1.0],
            vec4![0.078815, 0.054184, 0.211667, 1.0],
            vec4![0.232077, 0.059889, 0.437695, 1.0],
            vec4![0.396353, 0.083446, 0.530720, 1.0],
            vec4![0.564546, 0.120000, 0.533488, 1.0],
            vec4![0.735683, 0.169706, 0.467480, 1.0],
            vec4![0.886029, 0.257398, 0.359630, 1.0],
            vec4![0.967671, 0.412740, 0.261876, 1.0],
            vec4![0.994738, 0.602842, 0.226289, 1.0],
            vec4![0.997341, 0.803547, 0.329797, 1.0],
            vec4![0.987053, 0.991438, 0.749504, 1.0],
        ])
    }

    /// Red–Blue diverging palette stop data (11 samples, red → white → blue).
    fn rd_bu_gradient_data() -> ColorGradientStorage {
        ColorGradientStorage::with_colors(vec![
            vec4![0.403922, 0.000000, 0.121569, 1.0], // dark red
            vec4![0.698039, 0.094118, 0.168627, 1.0],
            vec4![0.839216, 0.376471, 0.301961, 1.0],
            vec4![0.956863, 0.647059, 0.509804, 1.0],
            vec4![0.992157, 0.858824, 0.780392, 1.0],
            vec4![0.968627, 0.968627, 0.968627, 1.0], // near-white centre
            vec4![0.819608, 0.898039, 0.941176, 1.0],
            vec4![0.572549, 0.772549, 0.870588, 1.0],
            vec4![0.262745, 0.576471, 0.764706, 1.0],
            vec4![0.129412, 0.400000, 0.674510, 1.0],
            vec4![0.019608, 0.188235, 0.380392, 1.0], // dark blue
        ])
    }
}

impl ComposableShaderFunction for ColorScale {
    type Input = f32;
    type Output = Vec4;
    type Uniforms = ColorScaleUniforms;

    fn wgsl_function() -> &'static str {
        r#"
fn color_scale(value: f32, params: ColorScaleUniforms) -> vec4<f32> {
    // Clamp and normalise to [0, 1] based on scale_kind.
    var t: f32;

    if (params.scale_kind == 1u) {
        // Diverging: piecewise normalisation around midpoint.
        if (value <= params.midpoint) {
            let range = params.midpoint - params.domain_min;
            if (range == 0.0) {
                t = 0.5;
            } else {
                t = 0.5 * clamp((value - params.domain_min) / range, 0.0, 1.0);
            }
        } else {
            let range = params.domain_max - params.midpoint;
            if (range == 0.0) {
                t = 0.5;
            } else {
                t = 0.5 + 0.5 * clamp((value - params.midpoint) / range, 0.0, 1.0);
            }
        }
    } else if (params.scale_kind == 2u) {
        // Quantize: integer bin selection.
        let normalized = clamp(
            (value - params.domain_min) / (params.domain_max - params.domain_min),
            0.0, 1.0
        );
        let bin = min(u32(normalized * f32(params.n_bins)), params.n_bins - 1u);
        // Map bin centre to [0, 1].
        t = (f32(bin) + 0.5) / f32(params.n_bins);
    } else {
        // Continuous: simple linear normalisation.
        t = clamp(
            (value - params.domain_min) / (params.domain_max - params.domain_min),
            0.0, 1.0
        );
    }

    // --- gradient lookup (binary search over storage buffers) ---
    let count = params.stop_count;
    if (count == 1u) {
        return gradient_colors[0];
    }
    if (t <= gradient_stops[0]) {
        return gradient_colors[0];
    }
    if (t >= gradient_stops[count - 1u]) {
        return gradient_colors[count - 1u];
    }

    var low = 0u;
    var high = count - 1u;
    while (low + 1u < high) {
        let mid = (low + high) / 2u;
        if (gradient_stops[mid] <= t) {
            low = mid;
        } else {
            high = mid;
        }
    }
    let t0 = gradient_stops[low];
    let t1 = gradient_stops[high];
    let local_t = (t - t0) / (t1 - t0);
    return mix(gradient_colors[low], gradient_colors[high], local_t);
}
"#
    }

    fn create_uniforms(&self) -> Option<Self::Uniforms> {
        let (kind_u32, midpoint, n_bins) = match &self.kind {
            ColorScaleKind::Continuous => (0u32, 0.0f32, 0u32),
            ColorScaleKind::Diverging { midpoint } => (1, *midpoint, 0),
            ColorScaleKind::Quantize { n_bins } => (2, 0.0, *n_bins),
        };
        Some(ColorScaleUniforms {
            domain_min: self.domain_min,
            domain_max: self.domain_max,
            midpoint,
            scale_kind: kind_u32,
            n_bins,
            stop_count: self.gradient.count(),
            _pad0: 0,
            _pad1: 0,
        })
    }

    fn function_name() -> &'static str {
        "color_scale"
    }
}

// ============================================================================
// HSV Color Mapping and Color Space Conversion (GUP-053 AC2)
// ============================================================================

/// HSV-based color mapping from a scalar value to RGBA color.
///
/// Maps a normalized [0..1] input to a color specified by hue range,
/// saturation, and value (brightness). The hue is interpolated linearly
/// across the given range in degrees.
#[derive(Clone, Debug)]
pub struct HSVColorMap {
    /// Start hue in degrees (0..360)
    pub hue_start: f32,
    /// End hue in degrees (0..360)
    pub hue_end: f32,
    /// Saturation (0..1)
    pub saturation: f32,
    /// Value / brightness (0..1)
    pub value: f32,
}

impl HSVColorMap {
    /// Creates a new HSV colour map with the given parameters.
    pub fn new(hue_start: f32, hue_end: f32, saturation: f32, value: f32) -> Self {
        Self {
            hue_start,
            hue_end,
            saturation,
            value,
        }
    }

    /// Creates a full-spectrum rainbow mapping (0°–360°).
    pub fn rainbow() -> Self {
        Self::new(0.0, 360.0, 1.0, 1.0)
    }

    /// Creates a cool-to-warm mapping (240° blue → 0° red).
    pub fn cool_warm() -> Self {
        Self::new(240.0, 0.0, 0.9, 0.9)
    }
}

/// GPU uniform data for the HSV colour map shader function.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct HSVColorMapUniforms {
    /// Starting hue angle in degrees.
    pub hue_start: f32,
    /// Ending hue angle in degrees.
    pub hue_end: f32,
    /// Saturation (0.0 to 1.0).
    pub saturation: f32,
    /// Value/brightness (0.0 to 1.0).
    pub value: f32,
}

impl ShaderUniform for HSVColorMapUniforms {
    fn wgsl_struct_definition() -> String {
        "struct HSVColorMapUniforms {\n    hue_start: f32,\n    hue_end: f32,\n    saturation: f32,\n    value: f32,\n}".to_string()
    }

    fn wgsl_type_name() -> &'static str {
        "HSVColorMapUniforms"
    }
}

impl ComposableShaderFunction for HSVColorMap {
    type Input = f32;
    type Output = Vec4;
    type Uniforms = HSVColorMapUniforms;

    fn wgsl_function() -> &'static str {
        r#"
        fn hsv_to_rgb(h: f32, s: f32, v: f32) -> vec3<f32> {
            let c = v * s;
            let hp = h / 60.0;
            let x = c * (1.0 - abs(hp % 2.0 - 1.0));
            let m = v - c;
            var rgb: vec3<f32>;
            if (hp < 1.0) { rgb = vec3<f32>(c, x, 0.0); }
            else if (hp < 2.0) { rgb = vec3<f32>(x, c, 0.0); }
            else if (hp < 3.0) { rgb = vec3<f32>(0.0, c, x); }
            else if (hp < 4.0) { rgb = vec3<f32>(0.0, x, c); }
            else if (hp < 5.0) { rgb = vec3<f32>(x, 0.0, c); }
            else { rgb = vec3<f32>(c, 0.0, x); }
            return rgb + vec3<f32>(m, m, m);
        }

        fn hsv_color_map(value: f32, params: HSVColorMapUniforms) -> vec4<f32> {
            let t = clamp(value, 0.0, 1.0);
            let hue = params.hue_start + t * (params.hue_end - params.hue_start);
            let h = ((hue % 360.0) + 360.0) % 360.0;
            let rgb = hsv_to_rgb(h, params.saturation, params.value);
            return vec4<f32>(rgb.x, rgb.y, rgb.z, 1.0);
        }
        "#
    }

    fn create_uniforms(&self) -> Option<Self::Uniforms> {
        Some(HSVColorMapUniforms {
            hue_start: self.hue_start,
            hue_end: self.hue_end,
            saturation: self.saturation,
            value: self.value,
        })
    }

    fn function_name() -> &'static str {
        "hsv_color_map"
    }
}

/// Alpha blending shader function for transparency control.
///
/// Applies an alpha multiplier to an RGBA color, useful for controlling
/// opacity in visualization layers.
#[derive(Clone, Debug)]
pub struct AlphaBlending {
    /// Alpha multiplier (0.0 = transparent, 1.0 = opaque)
    pub alpha: f32,
}

impl AlphaBlending {
    /// Creates a new alpha blending function with the given alpha value.
    pub fn new(alpha: f32) -> Self {
        Self { alpha }
    }

    /// Creates a semi-transparent blending (alpha = 0.5).
    pub fn semi_transparent() -> Self {
        Self::new(0.5)
    }
}

/// GPU uniform data for the alpha blending shader function.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct AlphaBlendingUniforms {
    /// Alpha multiplier value.
    pub alpha: f32,
    /// Padding for GPU alignment.
    pub _padding: [f32; 3],
}

impl ShaderUniform for AlphaBlendingUniforms {
    fn wgsl_struct_definition() -> String {
        "struct AlphaBlendingUniforms {\n    alpha: f32,\n}".to_string()
    }

    fn wgsl_type_name() -> &'static str {
        "AlphaBlendingUniforms"
    }
}

impl ComposableShaderFunction for AlphaBlending {
    type Input = Vec4;
    type Output = Vec4;
    type Uniforms = AlphaBlendingUniforms;

    fn wgsl_function() -> &'static str {
        r#"
        fn alpha_blending(color: vec4<f32>, params: AlphaBlendingUniforms) -> vec4<f32> {
            return vec4<f32>(color.xyz, color.w * params.alpha);
        }
        "#
    }

    fn create_uniforms(&self) -> Option<Self::Uniforms> {
        Some(AlphaBlendingUniforms {
            alpha: self.alpha,
            _padding: [0.0; 3],
        })
    }

    fn function_name() -> &'static str {
        "alpha_blending"
    }
}

/// Direction of color space conversion.
#[derive(Clone, Debug, Copy)]
pub enum ColorSpaceDirection {
    /// Convert from RGB to HSV
    RGBToHSV,
    /// Convert from HSV to RGB
    HSVToRGB,
}

/// Color space converter between RGB and HSV.
///
/// Converts colors between RGB and HSV color spaces on the GPU.
/// Input and output are Vec4 where xyz = color components, w = alpha (preserved).
#[derive(Clone, Debug)]
pub struct ColorSpaceConverter {
    /// The conversion direction (0 = RGB→HSV, 1 = HSV→RGB)
    pub direction: ColorSpaceDirection,
}

impl ColorSpaceConverter {
    /// Creates a new colour space converter with the given direction.
    pub fn new(direction: ColorSpaceDirection) -> Self {
        Self { direction }
    }

    /// Creates an RGB-to-HSV converter.
    pub fn rgb_to_hsv() -> Self {
        Self::new(ColorSpaceDirection::RGBToHSV)
    }

    /// Creates an HSV-to-RGB converter.
    pub fn hsv_to_rgb() -> Self {
        Self::new(ColorSpaceDirection::HSVToRGB)
    }
}

/// GPU uniform data for the colour space conversion shader function.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ColorSpaceConverterUniforms {
    /// 0 = RGB→HSV, 1 = HSV→RGB
    pub direction: u32,
    /// Padding for GPU alignment.
    pub _padding: [u32; 3],
}

impl ShaderUniform for ColorSpaceConverterUniforms {
    fn wgsl_struct_definition() -> String {
        "struct ColorSpaceConverterUniforms {\n    direction: u32,\n}".to_string()
    }

    fn wgsl_type_name() -> &'static str {
        "ColorSpaceConverterUniforms"
    }
}

impl ComposableShaderFunction for ColorSpaceConverter {
    type Input = Vec4;
    type Output = Vec4;
    type Uniforms = ColorSpaceConverterUniforms;

    fn wgsl_function() -> &'static str {
        r#"
        fn rgb_to_hsv_convert(rgb: vec3<f32>) -> vec3<f32> {
            let cmax = max(rgb.x, max(rgb.y, rgb.z));
            let cmin = min(rgb.x, min(rgb.y, rgb.z));
            let delta = cmax - cmin;
            var h: f32 = 0.0;
            if (delta > 0.0001) {
                if (cmax == rgb.x) {
                    h = 60.0 * (((rgb.y - rgb.z) / delta) % 6.0);
                } else if (cmax == rgb.y) {
                    h = 60.0 * (((rgb.z - rgb.x) / delta) + 2.0);
                } else {
                    h = 60.0 * (((rgb.x - rgb.y) / delta) + 4.0);
                }
            }
            if (h < 0.0) { h = h + 360.0; }
            let s = select(0.0, delta / cmax, cmax > 0.0);
            return vec3<f32>(h, s, cmax);
        }

        fn hsv_to_rgb_convert(hsv: vec3<f32>) -> vec3<f32> {
            let c = hsv.z * hsv.y;
            let hp = hsv.x / 60.0;
            let x = c * (1.0 - abs(hp % 2.0 - 1.0));
            let m = hsv.z - c;
            var rgb: vec3<f32>;
            if (hp < 1.0) { rgb = vec3<f32>(c, x, 0.0); }
            else if (hp < 2.0) { rgb = vec3<f32>(x, c, 0.0); }
            else if (hp < 3.0) { rgb = vec3<f32>(0.0, c, x); }
            else if (hp < 4.0) { rgb = vec3<f32>(0.0, x, c); }
            else if (hp < 5.0) { rgb = vec3<f32>(x, 0.0, c); }
            else { rgb = vec3<f32>(c, 0.0, x); }
            return rgb + vec3<f32>(m, m, m);
        }

        fn color_space_converter(color: vec4<f32>, params: ColorSpaceConverterUniforms) -> vec4<f32> {
            var result: vec3<f32>;
            if (params.direction == 0u) {
                result = rgb_to_hsv_convert(color.xyz);
            } else {
                result = hsv_to_rgb_convert(color.xyz);
            }
            return vec4<f32>(result.x, result.y, result.z, color.w);
        }
        "#
    }

    fn create_uniforms(&self) -> Option<Self::Uniforms> {
        Some(ColorSpaceConverterUniforms {
            direction: match self.direction {
                ColorSpaceDirection::RGBToHSV => 0,
                ColorSpaceDirection::HSVToRGB => 1,
            },
            _padding: [0; 3],
        })
    }

    fn function_name() -> &'static str {
        "color_space_converter"
    }
}

// ============================================================================
// Perceptual Color Space Conversions (GUP-293)
// ============================================================================

/// Direction of perceptual color space conversion.
#[derive(Clone, Debug, Copy, PartialEq, Eq)]
pub enum PerceptualColorSpaceDirection {
    /// Convert from linear RGB to CIE XYZ (D65 illuminant)
    RGBToXYZ,
    /// Convert from CIE XYZ (D65 illuminant) to linear RGB
    XYZToRGB,
    /// Convert from linear RGB to CIE LAB (via XYZ, D65 illuminant)
    RGBToLAB,
    /// Convert from CIE LAB to linear RGB (via XYZ, D65 illuminant)
    LABToRGB,
    /// Convert from linear RGB to OKLab
    RGBToOKLab,
    /// Convert from OKLab to linear RGB
    OKLabToRGB,
    /// Convert from linear RGB to LCH (cylindrical LAB, D65 illuminant)
    RGBToLCH,
    /// Convert from LCH (cylindrical LAB) to linear RGB (D65 illuminant)
    LCHToRGB,
}

/// Perceptual color space converter.
///
/// Converts colors between RGB and perceptual color spaces (CIE LAB, OKLab,
/// LCH) on the GPU. These spaces are designed so that equal numerical
/// differences correspond to equal perceived colour differences, making them
/// ideal for data visualisation colour scales.
///
/// Input and output are `Vec4` where xyz = colour components, w = alpha (preserved).
///
/// ## Colour Spaces
///
/// - **CIE XYZ**: Intermediate space derived from sRGB via the D65 illuminant.
/// - **CIE LAB**: Perceptually uniform space with L* (lightness 0–100),
///   a* (green–red), b* (blue–yellow).
/// - **OKLab**: A modern perceptual space by Björn Ottosson with improved
///   uniformity. L (0–1), a, b.
/// - **LCH**: Cylindrical form of CIE LAB with L* (lightness), C* (chroma),
///   h° (hue in degrees).
///
/// ## D65 Illuminant
///
/// The default illuminant is CIE Standard Illuminant D65, which represents
/// average daylight. Custom illuminant values may be supplied via
/// [`PerceptualColorSpaceConverter::with_illuminant`].
#[derive(Clone, Debug)]
pub struct PerceptualColorSpaceConverter {
    /// The conversion direction.
    pub direction: PerceptualColorSpaceDirection,
    /// D65 illuminant X reference (default: 0.95047).
    pub illuminant_x: f32,
    /// D65 illuminant Y reference (default: 1.0).
    pub illuminant_y: f32,
    /// D65 illuminant Z reference (default: 1.08883).
    pub illuminant_z: f32,
}

impl PerceptualColorSpaceConverter {
    /// Creates a new converter with the given direction and default D65 illuminant.
    pub fn new(direction: PerceptualColorSpaceDirection) -> Self {
        Self {
            direction,
            illuminant_x: 0.950_47,
            illuminant_y: 1.0,
            illuminant_z: 1.088_83,
        }
    }

    /// Overrides the illuminant reference white point.
    pub fn with_illuminant(mut self, x: f32, y: f32, z: f32) -> Self {
        self.illuminant_x = x;
        self.illuminant_y = y;
        self.illuminant_z = z;
        self
    }

    /// Creates an RGB → CIE XYZ converter.
    pub fn rgb_to_xyz() -> Self {
        Self::new(PerceptualColorSpaceDirection::RGBToXYZ)
    }

    /// Creates a CIE XYZ → RGB converter.
    pub fn xyz_to_rgb() -> Self {
        Self::new(PerceptualColorSpaceDirection::XYZToRGB)
    }

    /// Creates an RGB → CIE LAB converter.
    pub fn rgb_to_lab() -> Self {
        Self::new(PerceptualColorSpaceDirection::RGBToLAB)
    }

    /// Creates a CIE LAB → RGB converter.
    pub fn lab_to_rgb() -> Self {
        Self::new(PerceptualColorSpaceDirection::LABToRGB)
    }

    /// Creates an RGB → OKLab converter.
    pub fn rgb_to_oklab() -> Self {
        Self::new(PerceptualColorSpaceDirection::RGBToOKLab)
    }

    /// Creates an OKLab → RGB converter.
    pub fn oklab_to_rgb() -> Self {
        Self::new(PerceptualColorSpaceDirection::OKLabToRGB)
    }

    /// Creates an RGB → LCH converter.
    pub fn rgb_to_lch() -> Self {
        Self::new(PerceptualColorSpaceDirection::RGBToLCH)
    }

    /// Creates an LCH → RGB converter.
    pub fn lch_to_rgb() -> Self {
        Self::new(PerceptualColorSpaceDirection::LCHToRGB)
    }
}

/// GPU uniform data for the perceptual colour space conversion shader function.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PerceptualColorSpaceConverterUniforms {
    /// Conversion direction (see `PerceptualColorSpaceDirection` ordinals).
    pub direction: u32,
    /// D65 illuminant X reference.
    pub illuminant_x: f32,
    /// D65 illuminant Y reference.
    pub illuminant_y: f32,
    /// D65 illuminant Z reference.
    pub illuminant_z: f32,
}

impl ShaderUniform for PerceptualColorSpaceConverterUniforms {
    fn wgsl_struct_definition() -> String {
        concat!(
            "struct PerceptualColorSpaceConverterUniforms {\n",
            "    direction: u32,\n",
            "    illuminant_x: f32,\n",
            "    illuminant_y: f32,\n",
            "    illuminant_z: f32,\n",
            "}",
        )
        .to_string()
    }

    fn wgsl_type_name() -> &'static str {
        "PerceptualColorSpaceConverterUniforms"
    }
}

impl ComposableShaderFunction for PerceptualColorSpaceConverter {
    type Input = Vec4;
    type Output = Vec4;
    type Uniforms = PerceptualColorSpaceConverterUniforms;

    fn wgsl_function() -> &'static str {
        r#"
        fn srgb_to_linear(c: f32) -> f32 {
            if (c <= 0.04045) {
                return c / 12.92;
            }
            return pow((c + 0.055) / 1.055, 2.4);
        }

        fn linear_to_srgb(c: f32) -> f32 {
            if (c <= 0.0031308) {
                return c * 12.92;
            }
            return 1.055 * pow(c, 1.0 / 2.4) - 0.055;
        }

        fn rgb_to_xyz_convert(rgb: vec3<f32>) -> vec3<f32> {
            let r = srgb_to_linear(rgb.x);
            let g = srgb_to_linear(rgb.y);
            let b = srgb_to_linear(rgb.z);
            let x = r * 0.4124564 + g * 0.3575761 + b * 0.1804375;
            let y = r * 0.2126729 + g * 0.7151522 + b * 0.0721750;
            let z = r * 0.0193339 + g * 0.1191920 + b * 0.9503041;
            return vec3<f32>(x, y, z);
        }

        fn xyz_to_rgb_convert(xyz: vec3<f32>) -> vec3<f32> {
            let r = xyz.x *  3.2404542 + xyz.y * -1.5371385 + xyz.z * -0.4985314;
            let g = xyz.x * -0.9692660 + xyz.y *  1.8760108 + xyz.z *  0.0415560;
            let b = xyz.x *  0.0556434 + xyz.y * -0.2040259 + xyz.z *  1.0572252;
            return vec3<f32>(
                linear_to_srgb(clamp(r, 0.0, 1.0)),
                linear_to_srgb(clamp(g, 0.0, 1.0)),
                linear_to_srgb(clamp(b, 0.0, 1.0))
            );
        }

        fn lab_f(t: f32) -> f32 {
            let delta: f32 = 6.0 / 29.0;
            if (t > delta * delta * delta) {
                return pow(t, 1.0 / 3.0);
            }
            return t / (3.0 * delta * delta) + 4.0 / 29.0;
        }

        fn lab_f_inv(t: f32) -> f32 {
            let delta: f32 = 6.0 / 29.0;
            if (t > delta) {
                return t * t * t;
            }
            return 3.0 * delta * delta * (t - 4.0 / 29.0);
        }

        fn xyz_to_lab_convert(xyz: vec3<f32>, ref_x: f32, ref_y: f32, ref_z: f32) -> vec3<f32> {
            let fx = lab_f(xyz.x / ref_x);
            let fy = lab_f(xyz.y / ref_y);
            let fz = lab_f(xyz.z / ref_z);
            let l = 116.0 * fy - 16.0;
            let a = 500.0 * (fx - fy);
            let b = 200.0 * (fy - fz);
            return vec3<f32>(l, a, b);
        }

        fn lab_to_xyz_convert(lab: vec3<f32>, ref_x: f32, ref_y: f32, ref_z: f32) -> vec3<f32> {
            let fy = (lab.x + 16.0) / 116.0;
            let fx = lab.y / 500.0 + fy;
            let fz = fy - lab.z / 200.0;
            let x = ref_x * lab_f_inv(fx);
            let y = ref_y * lab_f_inv(fy);
            let z = ref_z * lab_f_inv(fz);
            return vec3<f32>(x, y, z);
        }

        fn rgb_to_lab_convert(rgb: vec3<f32>, ref_x: f32, ref_y: f32, ref_z: f32) -> vec3<f32> {
            let xyz = rgb_to_xyz_convert(rgb);
            return xyz_to_lab_convert(xyz, ref_x, ref_y, ref_z);
        }

        fn lab_to_rgb_convert(lab: vec3<f32>, ref_x: f32, ref_y: f32, ref_z: f32) -> vec3<f32> {
            let xyz = lab_to_xyz_convert(lab, ref_x, ref_y, ref_z);
            return xyz_to_rgb_convert(xyz);
        }

        fn rgb_to_oklab_convert(rgb: vec3<f32>) -> vec3<f32> {
            let r = srgb_to_linear(rgb.x);
            let g = srgb_to_linear(rgb.y);
            let b = srgb_to_linear(rgb.z);
            let l_ = 0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b;
            let m_ = 0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b;
            let s_ = 0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b;
            let l_c = pow(max(l_, 0.0), 1.0 / 3.0);
            let m_c = pow(max(m_, 0.0), 1.0 / 3.0);
            let s_c = pow(max(s_, 0.0), 1.0 / 3.0);
            let ol = 0.2104542553 * l_c + 0.7936177850 * m_c - 0.0040720468 * s_c;
            let oa = 1.9779984951 * l_c - 2.4285922050 * m_c + 0.4505937099 * s_c;
            let ob = 0.0259040371 * l_c + 0.7827717662 * m_c - 0.8086757660 * s_c;
            return vec3<f32>(ol, oa, ob);
        }

        fn oklab_to_rgb_convert(lab: vec3<f32>) -> vec3<f32> {
            let l_ = lab.x + 0.3963377774 * lab.y + 0.2158037573 * lab.z;
            let m_ = lab.x - 0.1055613458 * lab.y - 0.0638541728 * lab.z;
            let s_ = lab.x - 0.0894841775 * lab.y - 1.2914855480 * lab.z;
            let l_c = l_ * l_ * l_;
            let m_c = m_ * m_ * m_;
            let s_c = s_ * s_ * s_;
            let r =  4.0767416621 * l_c - 3.3077115913 * m_c + 0.2309699292 * s_c;
            let g = -1.2684380046 * l_c + 2.6097574011 * m_c - 0.3413193965 * s_c;
            let b = -0.0041960863 * l_c - 0.7034186147 * m_c + 1.7076147010 * s_c;
            return vec3<f32>(
                linear_to_srgb(clamp(r, 0.0, 1.0)),
                linear_to_srgb(clamp(g, 0.0, 1.0)),
                linear_to_srgb(clamp(b, 0.0, 1.0))
            );
        }

        fn lab_to_lch_convert(lab: vec3<f32>) -> vec3<f32> {
            let c = sqrt(lab.y * lab.y + lab.z * lab.z);
            var h = atan2(lab.z, lab.y) * 180.0 / 3.14159265358979;
            if (h < 0.0) {
                h = h + 360.0;
            }
            return vec3<f32>(lab.x, c, h);
        }

        fn lch_to_lab_convert(lch: vec3<f32>) -> vec3<f32> {
            let h_rad = lch.z * 3.14159265358979 / 180.0;
            let a = lch.y * cos(h_rad);
            let b = lch.y * sin(h_rad);
            return vec3<f32>(lch.x, a, b);
        }

        fn rgb_to_lch_convert(rgb: vec3<f32>, ref_x: f32, ref_y: f32, ref_z: f32) -> vec3<f32> {
            let lab = rgb_to_lab_convert(rgb, ref_x, ref_y, ref_z);
            return lab_to_lch_convert(lab);
        }

        fn lch_to_rgb_convert(lch: vec3<f32>, ref_x: f32, ref_y: f32, ref_z: f32) -> vec3<f32> {
            let lab = lch_to_lab_convert(lch);
            return lab_to_rgb_convert(lab, ref_x, ref_y, ref_z);
        }

        fn perceptual_color_space_converter(color: vec4<f32>, params: PerceptualColorSpaceConverterUniforms) -> vec4<f32> {
            var result: vec3<f32>;
            if (params.direction == 0u) {
                result = rgb_to_xyz_convert(color.xyz);
            } else if (params.direction == 1u) {
                result = xyz_to_rgb_convert(color.xyz);
            } else if (params.direction == 2u) {
                result = rgb_to_lab_convert(color.xyz, params.illuminant_x, params.illuminant_y, params.illuminant_z);
            } else if (params.direction == 3u) {
                result = lab_to_rgb_convert(color.xyz, params.illuminant_x, params.illuminant_y, params.illuminant_z);
            } else if (params.direction == 4u) {
                result = rgb_to_oklab_convert(color.xyz);
            } else if (params.direction == 5u) {
                result = oklab_to_rgb_convert(color.xyz);
            } else if (params.direction == 6u) {
                result = rgb_to_lch_convert(color.xyz, params.illuminant_x, params.illuminant_y, params.illuminant_z);
            } else {
                result = lch_to_rgb_convert(color.xyz, params.illuminant_x, params.illuminant_y, params.illuminant_z);
            }
            return vec4<f32>(result, color.w);
        }
        "#
    }

    fn create_uniforms(&self) -> Option<Self::Uniforms> {
        Some(PerceptualColorSpaceConverterUniforms {
            direction: match self.direction {
                PerceptualColorSpaceDirection::RGBToXYZ => 0,
                PerceptualColorSpaceDirection::XYZToRGB => 1,
                PerceptualColorSpaceDirection::RGBToLAB => 2,
                PerceptualColorSpaceDirection::LABToRGB => 3,
                PerceptualColorSpaceDirection::RGBToOKLab => 4,
                PerceptualColorSpaceDirection::OKLabToRGB => 5,
                PerceptualColorSpaceDirection::RGBToLCH => 6,
                PerceptualColorSpaceDirection::LCHToRGB => 7,
            },
            illuminant_x: self.illuminant_x,
            illuminant_y: self.illuminant_y,
            illuminant_z: self.illuminant_z,
        })
    }

    fn function_name() -> &'static str {
        "perceptual_color_space_converter"
    }
}

/// Perceptual color interpolation space.
#[derive(Clone, Debug, Copy, PartialEq, Eq)]
pub enum PerceptualInterpolationSpace {
    /// Interpolate in CIE LAB space.
    LAB,
    /// Interpolate in OKLab space.
    OKLab,
    /// Interpolate in LCH space (with hue interpolation).
    LCH,
}

/// Perceptual colour interpolation between two RGB colours.
///
/// Converts the two endpoint colours into a perceptual space (LAB, OKLab, or
/// LCH), performs linear interpolation at the given *t* value, and converts
/// back to sRGB. This produces perceptually uniform gradients that avoid the
/// muddy mid-tones common with naïve RGB interpolation.
///
/// Input is a scalar `f32` in \[0, 1\] representing the interpolation parameter.
/// Output is a `Vec4` RGBA colour.
#[derive(Clone, Debug)]
pub struct PerceptualInterpolation {
    /// First endpoint colour (sRGB, 0–1 per channel).
    pub color_a: Vec4,
    /// Second endpoint colour (sRGB, 0–1 per channel).
    pub color_b: Vec4,
    /// The perceptual space in which interpolation is performed.
    pub space: PerceptualInterpolationSpace,
    /// D65 illuminant X reference (used for LAB/LCH only).
    pub illuminant_x: f32,
    /// D65 illuminant Y reference.
    pub illuminant_y: f32,
    /// D65 illuminant Z reference.
    pub illuminant_z: f32,
}

impl PerceptualInterpolation {
    /// Creates a LAB-space interpolator between two colours.
    pub fn lab(color_a: Vec4, color_b: Vec4) -> Self {
        Self {
            color_a,
            color_b,
            space: PerceptualInterpolationSpace::LAB,
            illuminant_x: 0.950_47,
            illuminant_y: 1.0,
            illuminant_z: 1.088_83,
        }
    }

    /// Creates an OKLab-space interpolator between two colours.
    pub fn oklab(color_a: Vec4, color_b: Vec4) -> Self {
        Self {
            color_a,
            color_b,
            space: PerceptualInterpolationSpace::OKLab,
            illuminant_x: 0.950_47,
            illuminant_y: 1.0,
            illuminant_z: 1.088_83,
        }
    }

    /// Creates an LCH-space interpolator between two colours.
    pub fn lch(color_a: Vec4, color_b: Vec4) -> Self {
        Self {
            color_a,
            color_b,
            space: PerceptualInterpolationSpace::LCH,
            illuminant_x: 0.950_47,
            illuminant_y: 1.0,
            illuminant_z: 1.088_83,
        }
    }

    /// Overrides the illuminant reference white point.
    pub fn with_illuminant(mut self, x: f32, y: f32, z: f32) -> Self {
        self.illuminant_x = x;
        self.illuminant_y = y;
        self.illuminant_z = z;
        self
    }
}

/// GPU uniform data for the perceptual interpolation shader function.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PerceptualInterpolationUniforms {
    /// First endpoint colour red.
    pub color_a_r: f32,
    /// First endpoint colour green.
    pub color_a_g: f32,
    /// First endpoint colour blue.
    pub color_a_b: f32,
    /// First endpoint colour alpha.
    pub color_a_a: f32,
    /// Second endpoint colour red.
    pub color_b_r: f32,
    /// Second endpoint colour green.
    pub color_b_g: f32,
    /// Second endpoint colour blue.
    pub color_b_b: f32,
    /// Second endpoint colour alpha.
    pub color_b_a: f32,
    /// Interpolation space (0 = LAB, 1 = OKLab, 2 = LCH).
    pub space: u32,
    /// D65 illuminant X reference.
    pub illuminant_x: f32,
    /// D65 illuminant Y reference.
    pub illuminant_y: f32,
    /// D65 illuminant Z reference.
    pub illuminant_z: f32,
}

impl ShaderUniform for PerceptualInterpolationUniforms {
    fn wgsl_struct_definition() -> String {
        concat!(
            "struct PerceptualInterpolationUniforms {\n",
            "    color_a_r: f32,\n",
            "    color_a_g: f32,\n",
            "    color_a_b: f32,\n",
            "    color_a_a: f32,\n",
            "    color_b_r: f32,\n",
            "    color_b_g: f32,\n",
            "    color_b_b: f32,\n",
            "    color_b_a: f32,\n",
            "    space: u32,\n",
            "    illuminant_x: f32,\n",
            "    illuminant_y: f32,\n",
            "    illuminant_z: f32,\n",
            "}",
        )
        .to_string()
    }

    fn wgsl_type_name() -> &'static str {
        "PerceptualInterpolationUniforms"
    }
}

impl ComposableShaderFunction for PerceptualInterpolation {
    type Input = f32;
    type Output = Vec4;
    type Uniforms = PerceptualInterpolationUniforms;

    fn wgsl_function() -> &'static str {
        // The helper functions have a `_pi` suffix to avoid WGSL name
        // collisions when both PerceptualColorSpaceConverter and
        // PerceptualInterpolation are composed in the same pipeline.
        r#"
        fn srgb_to_linear_pi(c: f32) -> f32 {
            if (c <= 0.04045) {
                return c / 12.92;
            }
            return pow((c + 0.055) / 1.055, 2.4);
        }

        fn linear_to_srgb_pi(c: f32) -> f32 {
            if (c <= 0.0031308) {
                return c * 12.92;
            }
            return 1.055 * pow(c, 1.0 / 2.4) - 0.055;
        }

        fn rgb_to_xyz_pi(rgb: vec3<f32>) -> vec3<f32> {
            let r = srgb_to_linear_pi(rgb.x);
            let g = srgb_to_linear_pi(rgb.y);
            let b = srgb_to_linear_pi(rgb.z);
            let x = r * 0.4124564 + g * 0.3575761 + b * 0.1804375;
            let y = r * 0.2126729 + g * 0.7151522 + b * 0.0721750;
            let z = r * 0.0193339 + g * 0.1191920 + b * 0.9503041;
            return vec3<f32>(x, y, z);
        }

        fn xyz_to_rgb_pi(xyz: vec3<f32>) -> vec3<f32> {
            let r = xyz.x *  3.2404542 + xyz.y * -1.5371385 + xyz.z * -0.4985314;
            let g = xyz.x * -0.9692660 + xyz.y *  1.8760108 + xyz.z *  0.0415560;
            let b = xyz.x *  0.0556434 + xyz.y * -0.2040259 + xyz.z *  1.0572252;
            return vec3<f32>(
                linear_to_srgb_pi(clamp(r, 0.0, 1.0)),
                linear_to_srgb_pi(clamp(g, 0.0, 1.0)),
                linear_to_srgb_pi(clamp(b, 0.0, 1.0))
            );
        }

        fn lab_f_pi(t: f32) -> f32 {
            let delta: f32 = 6.0 / 29.0;
            if (t > delta * delta * delta) {
                return pow(t, 1.0 / 3.0);
            }
            return t / (3.0 * delta * delta) + 4.0 / 29.0;
        }

        fn lab_f_inv_pi(t: f32) -> f32 {
            let delta: f32 = 6.0 / 29.0;
            if (t > delta) {
                return t * t * t;
            }
            return 3.0 * delta * delta * (t - 4.0 / 29.0);
        }

        fn rgb_to_lab_pi(rgb: vec3<f32>, ref_x: f32, ref_y: f32, ref_z: f32) -> vec3<f32> {
            let xyz = rgb_to_xyz_pi(rgb);
            let fx = lab_f_pi(xyz.x / ref_x);
            let fy = lab_f_pi(xyz.y / ref_y);
            let fz = lab_f_pi(xyz.z / ref_z);
            let l = 116.0 * fy - 16.0;
            let a = 500.0 * (fx - fy);
            let b = 200.0 * (fy - fz);
            return vec3<f32>(l, a, b);
        }

        fn lab_to_rgb_pi(lab: vec3<f32>, ref_x: f32, ref_y: f32, ref_z: f32) -> vec3<f32> {
            let fy = (lab.x + 16.0) / 116.0;
            let fx = lab.y / 500.0 + fy;
            let fz = fy - lab.z / 200.0;
            let x = ref_x * lab_f_inv_pi(fx);
            let y = ref_y * lab_f_inv_pi(fy);
            let z = ref_z * lab_f_inv_pi(fz);
            return xyz_to_rgb_pi(vec3<f32>(x, y, z));
        }

        fn rgb_to_oklab_pi(rgb: vec3<f32>) -> vec3<f32> {
            let r = srgb_to_linear_pi(rgb.x);
            let g = srgb_to_linear_pi(rgb.y);
            let b = srgb_to_linear_pi(rgb.z);
            let l_ = 0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b;
            let m_ = 0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b;
            let s_ = 0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b;
            let l_c = pow(max(l_, 0.0), 1.0 / 3.0);
            let m_c = pow(max(m_, 0.0), 1.0 / 3.0);
            let s_c = pow(max(s_, 0.0), 1.0 / 3.0);
            let ol = 0.2104542553 * l_c + 0.7936177850 * m_c - 0.0040720468 * s_c;
            let oa = 1.9779984951 * l_c - 2.4285922050 * m_c + 0.4505937099 * s_c;
            let ob = 0.0259040371 * l_c + 0.7827717662 * m_c - 0.8086757660 * s_c;
            return vec3<f32>(ol, oa, ob);
        }

        fn oklab_to_rgb_pi(lab: vec3<f32>) -> vec3<f32> {
            let l_ = lab.x + 0.3963377774 * lab.y + 0.2158037573 * lab.z;
            let m_ = lab.x - 0.1055613458 * lab.y - 0.0638541728 * lab.z;
            let s_ = lab.x - 0.0894841775 * lab.y - 1.2914855480 * lab.z;
            let l_c = l_ * l_ * l_;
            let m_c = m_ * m_ * m_;
            let s_c = s_ * s_ * s_;
            let r =  4.0767416621 * l_c - 3.3077115913 * m_c + 0.2309699292 * s_c;
            let g = -1.2684380046 * l_c + 2.6097574011 * m_c - 0.3413193965 * s_c;
            let b = -0.0041960863 * l_c - 0.7034186147 * m_c + 1.7076147010 * s_c;
            return vec3<f32>(
                linear_to_srgb_pi(clamp(r, 0.0, 1.0)),
                linear_to_srgb_pi(clamp(g, 0.0, 1.0)),
                linear_to_srgb_pi(clamp(b, 0.0, 1.0))
            );
        }

        fn lab_to_lch_pi(lab: vec3<f32>) -> vec3<f32> {
            let c = sqrt(lab.y * lab.y + lab.z * lab.z);
            var h = atan2(lab.z, lab.y) * 180.0 / 3.14159265358979;
            if (h < 0.0) {
                h = h + 360.0;
            }
            return vec3<f32>(lab.x, c, h);
        }

        fn lch_to_lab_pi(lch: vec3<f32>) -> vec3<f32> {
            let h_rad = lch.z * 3.14159265358979 / 180.0;
            let a = lch.y * cos(h_rad);
            let b = lch.y * sin(h_rad);
            return vec3<f32>(lch.x, a, b);
        }

        fn lch_shortest_hue_lerp(h0: f32, h1: f32, t: f32) -> f32 {
            var diff = h1 - h0;
            if (diff > 180.0) {
                diff = diff - 360.0;
            } else if (diff < -180.0) {
                diff = diff + 360.0;
            }
            var h = h0 + t * diff;
            if (h < 0.0) { h = h + 360.0; }
            if (h >= 360.0) { h = h - 360.0; }
            return h;
        }

        fn perceptual_interpolation(t: f32, params: PerceptualInterpolationUniforms) -> vec4<f32> {
            let t_clamped = clamp(t, 0.0, 1.0);
            let ca = vec3<f32>(params.color_a_r, params.color_a_g, params.color_a_b);
            let cb = vec3<f32>(params.color_b_r, params.color_b_g, params.color_b_b);
            var result: vec3<f32>;

            if (params.space == 0u) {
                let lab_a = rgb_to_lab_pi(ca, params.illuminant_x, params.illuminant_y, params.illuminant_z);
                let lab_b = rgb_to_lab_pi(cb, params.illuminant_x, params.illuminant_y, params.illuminant_z);
                let lab_mix = mix(lab_a, lab_b, vec3<f32>(t_clamped));
                result = lab_to_rgb_pi(lab_mix, params.illuminant_x, params.illuminant_y, params.illuminant_z);
            } else if (params.space == 1u) {
                let oklab_a = rgb_to_oklab_pi(ca);
                let oklab_b = rgb_to_oklab_pi(cb);
                let oklab_mix = mix(oklab_a, oklab_b, vec3<f32>(t_clamped));
                result = oklab_to_rgb_pi(oklab_mix);
            } else {
                let lch_a = lab_to_lch_pi(rgb_to_lab_pi(ca, params.illuminant_x, params.illuminant_y, params.illuminant_z));
                let lch_b = lab_to_lch_pi(rgb_to_lab_pi(cb, params.illuminant_x, params.illuminant_y, params.illuminant_z));
                let l = mix(lch_a.x, lch_b.x, t_clamped);
                let c = mix(lch_a.y, lch_b.y, t_clamped);
                let h = lch_shortest_hue_lerp(lch_a.z, lch_b.z, t_clamped);
                let lab_result = lch_to_lab_pi(vec3<f32>(l, c, h));
                result = lab_to_rgb_pi(lab_result, params.illuminant_x, params.illuminant_y, params.illuminant_z);
            }

            let alpha = mix(params.color_a_a, params.color_b_a, t_clamped);
            return vec4<f32>(result, alpha);
        }
        "#
    }

    fn create_uniforms(&self) -> Option<Self::Uniforms> {
        Some(PerceptualInterpolationUniforms {
            color_a_r: self.color_a.x,
            color_a_g: self.color_a.y,
            color_a_b: self.color_a.z,
            color_a_a: self.color_a.w,
            color_b_r: self.color_b.x,
            color_b_g: self.color_b.y,
            color_b_b: self.color_b.z,
            color_b_a: self.color_b.w,
            space: match self.space {
                PerceptualInterpolationSpace::LAB => 0,
                PerceptualInterpolationSpace::OKLab => 1,
                PerceptualInterpolationSpace::LCH => 2,
            },
            illuminant_x: self.illuminant_x,
            illuminant_y: self.illuminant_y,
            illuminant_z: self.illuminant_z,
        })
    }

    fn function_name() -> &'static str {
        "perceptual_interpolation"
    }
}

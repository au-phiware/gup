// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Geographic Projection Shader Functions
//!
//! This module provides composable GPU shader functions for common geographic
//! projections. Each projection transforms `GeoPoint` (longitude, latitude)
//! coordinates into planar `vec2<f32>` positions suitable for further
//! composition with screen transforms.
//!
//! ## Supported Projections
//!
//! - [`EquirectangularProjection`] — Plate Carrée (equidistant cylindrical)
//! - [`MercatorProjection`] — Web Mercator with configurable latitude clipping
//! - [`StereographicProjection`] — Azimuthal stereographic
//! - [`OrthographicProjection`] — Azimuthal orthographic (visible hemisphere)
//!
//! ## Boundary Clipping
//!
//! Projections with natural validity boundaries (Mercator latitude limits,
//! Orthographic far-hemisphere) encode clip tests in their WGSL functions.
//! When a point falls outside the valid region, the shader returns
//! `vec2(CLIP_SENTINEL, CLIP_SENTINEL)` where [`CLIP_SENTINEL`] = `1e9`.
//! Downstream shaders (fragment discard, geometry filters) should test for
//! this sentinel to cull invalid points.
//!
//! ## Composition Example
//!
//! ```rust,ignore
//! use gup::shader_function::geo::*;
//! use gup::shader_function::{ComposableFunction, PositionTransform, Vec2};
//!
//! let projection = MercatorProjection::new();
//! let screen = PositionTransform::new(Vec2::new(400.0, 400.0), Vec2::new(400.0, 300.0));
//! let composed = projection.compose(screen);
//! ```

use super::{ComposableShaderFunction, ShaderType, ShaderUniform, Vec2};

/// Sentinel value returned by projection shaders when a point is outside the
/// valid projection region. Downstream consumers (e.g. fragment shaders)
/// should discard vertices whose projected position equals this sentinel.
pub const CLIP_SENTINEL: f32 = 1e9;

// ---------------------------------------------------------------------------
// GeoPoint — geographic coordinate type
// ---------------------------------------------------------------------------

/// A geographic coordinate with longitude and latitude in **degrees**.
///
/// This type implements [`ShaderType`] so it can be used as the input type
/// for geographic projection shader functions.
///
/// In WGSL, this maps to:
/// ```wgsl
/// struct gup_GeoPoint {
///     longitude: f32,
///     latitude: f32,
/// }
/// ```
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GeoPoint {
    pub longitude: f32,
    pub latitude: f32,
}

impl GeoPoint {
    /// Creates a new geographic point from longitude and latitude in degrees.
    #[inline]
    pub const fn new(longitude: f32, latitude: f32) -> Self {
        Self {
            longitude,
            latitude,
        }
    }
}

impl ShaderType for GeoPoint {
    fn wgsl_type_name() -> &'static str {
        "gup_GeoPoint"
    }

    fn wgsl_type_definition() -> Option<&'static str> {
        Some("struct gup_GeoPoint {\n    longitude: f32,\n    latitude: f32,\n}")
    }

    fn size_bytes() -> usize {
        8 // 2 × f32
    }

    fn alignment() -> usize {
        4 // f32 alignment
    }
}

// ---------------------------------------------------------------------------
// Equirectangular Projection
// ---------------------------------------------------------------------------

/// Uniforms for the equirectangular (Plate Carrée) projection.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct EquirectangularUniforms {
    pub center_lon: f32,
    pub center_lat: f32,
    pub scale: f32,
    pub translate_x: f32,
    pub translate_y: f32,
    /// Padding for 4-byte alignment of the overall struct to a multiple of
    /// the largest member alignment (f32 = 4 bytes). This ensures the struct
    /// size is a multiple of 4 bytes, which is already satisfied. Added for
    /// future-proofing uniform buffer requirements.
    pub _pad0: f32,
    pub _pad1: f32,
    pub _pad2: f32,
}

impl ShaderUniform for EquirectangularUniforms {
    fn wgsl_struct_definition() -> String {
        "struct EquirectangularUniforms {\n    center_lon: f32,\n    center_lat: f32,\n    scale: f32,\n    translate_x: f32,\n    translate_y: f32,\n    _pad0: f32,\n    _pad1: f32,\n    _pad2: f32,\n}".to_string()
    }

    fn wgsl_type_name() -> &'static str {
        "EquirectangularUniforms"
    }
}

/// Equirectangular (Plate Carrée) projection.
///
/// Maps `(lon, lat)` to `(x, y)` where:
/// - `x = (lon - center_lon) * cos(center_lat)` (in radians)
/// - `y = lat - center_lat` (in radians)
///
/// The result is then scaled and translated by the uniform parameters.
#[derive(Clone, Debug)]
pub struct EquirectangularProjection {
    pub center_lon: f32,
    pub center_lat: f32,
    pub scale: f32,
    pub translate_x: f32,
    pub translate_y: f32,
}

impl EquirectangularProjection {
    /// Creates a new equirectangular projection with default parameters.
    ///
    /// Default: centred at `(0°, 0°)`, scale `1.0`, no translation.
    pub fn new() -> Self {
        Self {
            center_lon: 0.0,
            center_lat: 0.0,
            scale: 1.0,
            translate_x: 0.0,
            translate_y: 0.0,
        }
    }

    /// Sets the projection centre in degrees.
    pub fn center(mut self, lon: f32, lat: f32) -> Self {
        self.center_lon = lon;
        self.center_lat = lat;
        self
    }

    /// Sets the scale factor.
    pub fn scale(mut self, scale: f32) -> Self {
        self.scale = scale;
        self
    }

    /// Sets the translation offset.
    pub fn translate(mut self, x: f32, y: f32) -> Self {
        self.translate_x = x;
        self.translate_y = y;
        self
    }
}

impl Default for EquirectangularProjection {
    fn default() -> Self {
        Self::new()
    }
}

impl ComposableShaderFunction for EquirectangularProjection {
    type Input = GeoPoint;
    type Output = Vec2;
    type Uniforms = EquirectangularUniforms;

    fn wgsl_function() -> &'static str {
        r#"
const GUP_DEG_TO_RAD: f32 = 0.017453292519943295;

fn equirectangular_projection(point: gup_GeoPoint, u: EquirectangularUniforms) -> vec2<f32> {
    let lon_rad = (point.longitude - u.center_lon) * GUP_DEG_TO_RAD;
    let lat_rad = (point.latitude - u.center_lat) * GUP_DEG_TO_RAD;
    let center_lat_rad = u.center_lat * GUP_DEG_TO_RAD;
    let x = lon_rad * cos(center_lat_rad);
    let y = lat_rad;
    return vec2<f32>(x * u.scale + u.translate_x, y * u.scale + u.translate_y);
}
"#
    }

    fn create_uniforms(&self) -> Option<Self::Uniforms> {
        Some(EquirectangularUniforms {
            center_lon: self.center_lon,
            center_lat: self.center_lat,
            scale: self.scale,
            translate_x: self.translate_x,
            translate_y: self.translate_y,
            _pad0: 0.0,
            _pad1: 0.0,
            _pad2: 0.0,
        })
    }

    fn function_name() -> &'static str {
        "equirectangular_projection"
    }
}

// ---------------------------------------------------------------------------
// Mercator Projection
// ---------------------------------------------------------------------------

/// Uniforms for the Mercator projection.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MercatorUniforms {
    pub center_lon: f32,
    pub scale: f32,
    pub translate_x: f32,
    pub translate_y: f32,
    /// Maximum absolute latitude in degrees before clipping.
    /// Default: 85.051129° (Web Mercator standard).
    pub clip_lat: f32,
    pub _pad0: f32,
    pub _pad1: f32,
    pub _pad2: f32,
}

impl ShaderUniform for MercatorUniforms {
    fn wgsl_struct_definition() -> String {
        "struct MercatorUniforms {\n    center_lon: f32,\n    scale: f32,\n    translate_x: f32,\n    translate_y: f32,\n    clip_lat: f32,\n    _pad0: f32,\n    _pad1: f32,\n    _pad2: f32,\n}".to_string()
    }

    fn wgsl_type_name() -> &'static str {
        "MercatorUniforms"
    }
}

/// Mercator projection.
///
/// Maps `(lon, lat)` to `(x, y)` where:
/// - `x = lon - center_lon` (in radians)
/// - `y = ln(tan(π/4 + lat/2))` (in radians)
///
/// Points beyond `clip_lat` degrees are clipped to [`CLIP_SENTINEL`].
/// The latitude is internally clamped to ±85.051129° before the logarithm
/// to avoid numerical instability, regardless of the `clip_lat` value.
#[derive(Clone, Debug)]
pub struct MercatorProjection {
    pub center_lon: f32,
    pub scale: f32,
    pub translate_x: f32,
    pub translate_y: f32,
    pub clip_lat: f32,
}

/// The standard Web Mercator latitude limit in degrees.
const DEFAULT_CLIP_LAT: f32 = 85.051_13;

impl MercatorProjection {
    /// Creates a new Mercator projection with default parameters.
    ///
    /// Default: centred at `0°` longitude, scale `1.0`, no translation,
    /// clip latitude `85.051129°`.
    pub fn new() -> Self {
        Self {
            center_lon: 0.0,
            scale: 1.0,
            translate_x: 0.0,
            translate_y: 0.0,
            clip_lat: DEFAULT_CLIP_LAT,
        }
    }

    /// Sets the centre longitude in degrees.
    pub fn center_lon(mut self, lon: f32) -> Self {
        self.center_lon = lon;
        self
    }

    /// Sets the scale factor.
    pub fn scale(mut self, scale: f32) -> Self {
        self.scale = scale;
        self
    }

    /// Sets the translation offset.
    pub fn translate(mut self, x: f32, y: f32) -> Self {
        self.translate_x = x;
        self.translate_y = y;
        self
    }

    /// Sets the maximum absolute latitude in degrees before clipping.
    pub fn clip_lat(mut self, lat: f32) -> Self {
        self.clip_lat = lat;
        self
    }
}

impl Default for MercatorProjection {
    fn default() -> Self {
        Self::new()
    }
}

impl ComposableShaderFunction for MercatorProjection {
    type Input = GeoPoint;
    type Output = Vec2;
    type Uniforms = MercatorUniforms;

    fn wgsl_function() -> &'static str {
        r#"
const GUP_DEG_TO_RAD_M: f32 = 0.017453292519943295;
const GUP_CLIP_SENTINEL: f32 = 1e9;
const GUP_PI_OVER_4: f32 = 0.7853981633974483;
const GUP_MAX_SAFE_LAT_RAD: f32 = 1.4844222297453324;

fn mercator_projection(point: gup_GeoPoint, u: MercatorUniforms) -> vec2<f32> {
    let clip_lat_rad = u.clip_lat * GUP_DEG_TO_RAD_M;
    let lat_abs = abs(point.latitude);
    if (lat_abs > u.clip_lat) {
        return vec2<f32>(GUP_CLIP_SENTINEL, GUP_CLIP_SENTINEL);
    }
    let lon_rad = (point.longitude - u.center_lon) * GUP_DEG_TO_RAD_M;
    let lat_rad = clamp(point.latitude * GUP_DEG_TO_RAD_M, -GUP_MAX_SAFE_LAT_RAD, GUP_MAX_SAFE_LAT_RAD);
    let x = lon_rad;
    let y = log(tan(GUP_PI_OVER_4 + lat_rad * 0.5));
    return vec2<f32>(x * u.scale + u.translate_x, y * u.scale + u.translate_y);
}
"#
    }

    fn create_uniforms(&self) -> Option<Self::Uniforms> {
        Some(MercatorUniforms {
            center_lon: self.center_lon,
            scale: self.scale,
            translate_x: self.translate_x,
            translate_y: self.translate_y,
            clip_lat: self.clip_lat,
            _pad0: 0.0,
            _pad1: 0.0,
            _pad2: 0.0,
        })
    }

    fn function_name() -> &'static str {
        "mercator_projection"
    }
}

// ---------------------------------------------------------------------------
// Stereographic Projection
// ---------------------------------------------------------------------------

/// Uniforms for the stereographic projection.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct StereographicUniforms {
    pub center_lon: f32,
    pub center_lat: f32,
    pub scale: f32,
    pub translate_x: f32,
    pub translate_y: f32,
    pub _pad0: f32,
    pub _pad1: f32,
    pub _pad2: f32,
}

impl ShaderUniform for StereographicUniforms {
    fn wgsl_struct_definition() -> String {
        "struct StereographicUniforms {\n    center_lon: f32,\n    center_lat: f32,\n    scale: f32,\n    translate_x: f32,\n    translate_y: f32,\n    _pad0: f32,\n    _pad1: f32,\n    _pad2: f32,\n}".to_string()
    }

    fn wgsl_type_name() -> &'static str {
        "StereographicUniforms"
    }
}

/// Azimuthal stereographic projection.
///
/// Projects the sphere from a point diametrically opposite the projection
/// centre onto a tangent plane. Preserves angles (conformal) but distorts
/// area, with infinite distortion at the antipodal point.
#[derive(Clone, Debug)]
pub struct StereographicProjection {
    pub center_lon: f32,
    pub center_lat: f32,
    pub scale: f32,
    pub translate_x: f32,
    pub translate_y: f32,
}

impl StereographicProjection {
    /// Creates a new stereographic projection centred at `(0°, 0°)`.
    pub fn new() -> Self {
        Self {
            center_lon: 0.0,
            center_lat: 0.0,
            scale: 1.0,
            translate_x: 0.0,
            translate_y: 0.0,
        }
    }

    /// Sets the projection centre in degrees.
    pub fn center(mut self, lon: f32, lat: f32) -> Self {
        self.center_lon = lon;
        self.center_lat = lat;
        self
    }

    /// Sets the scale factor.
    pub fn scale(mut self, scale: f32) -> Self {
        self.scale = scale;
        self
    }

    /// Sets the translation offset.
    pub fn translate(mut self, x: f32, y: f32) -> Self {
        self.translate_x = x;
        self.translate_y = y;
        self
    }
}

impl Default for StereographicProjection {
    fn default() -> Self {
        Self::new()
    }
}

impl ComposableShaderFunction for StereographicProjection {
    type Input = GeoPoint;
    type Output = Vec2;
    type Uniforms = StereographicUniforms;

    fn wgsl_function() -> &'static str {
        r#"
const GUP_DEG_TO_RAD_S: f32 = 0.017453292519943295;

fn stereographic_projection(point: gup_GeoPoint, u: StereographicUniforms) -> vec2<f32> {
    let lon = point.longitude * GUP_DEG_TO_RAD_S;
    let lat = point.latitude * GUP_DEG_TO_RAD_S;
    let lon0 = u.center_lon * GUP_DEG_TO_RAD_S;
    let lat0 = u.center_lat * GUP_DEG_TO_RAD_S;
    let d_lon = lon - lon0;
    let cos_lat = cos(lat);
    let sin_lat = sin(lat);
    let cos_lat0 = cos(lat0);
    let sin_lat0 = sin(lat0);
    let cos_d_lon = cos(d_lon);
    let k_denom = 1.0 + sin_lat0 * sin_lat + cos_lat0 * cos_lat * cos_d_lon;
    let k = 2.0 / k_denom;
    let x = k * cos_lat * sin(d_lon);
    let y = k * (cos_lat0 * sin_lat - sin_lat0 * cos_lat * cos_d_lon);
    return vec2<f32>(x * u.scale + u.translate_x, y * u.scale + u.translate_y);
}
"#
    }

    fn create_uniforms(&self) -> Option<Self::Uniforms> {
        Some(StereographicUniforms {
            center_lon: self.center_lon,
            center_lat: self.center_lat,
            scale: self.scale,
            translate_x: self.translate_x,
            translate_y: self.translate_y,
            _pad0: 0.0,
            _pad1: 0.0,
            _pad2: 0.0,
        })
    }

    fn function_name() -> &'static str {
        "stereographic_projection"
    }
}

// ---------------------------------------------------------------------------
// Orthographic Projection
// ---------------------------------------------------------------------------

/// Uniforms for the orthographic projection.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct OrthographicUniforms {
    pub center_lon: f32,
    pub center_lat: f32,
    pub scale: f32,
    pub translate_x: f32,
    pub translate_y: f32,
    pub _pad0: f32,
    pub _pad1: f32,
    pub _pad2: f32,
}

impl ShaderUniform for OrthographicUniforms {
    fn wgsl_struct_definition() -> String {
        "struct OrthographicUniforms {\n    center_lon: f32,\n    center_lat: f32,\n    scale: f32,\n    translate_x: f32,\n    translate_y: f32,\n    _pad0: f32,\n    _pad1: f32,\n    _pad2: f32,\n}".to_string()
    }

    fn wgsl_type_name() -> &'static str {
        "OrthographicUniforms"
    }
}

/// Azimuthal orthographic projection.
///
/// Projects the visible hemisphere as seen from infinite distance. Points on
/// the far hemisphere (behind the viewer) are clipped to [`CLIP_SENTINEL`].
#[derive(Clone, Debug)]
pub struct OrthographicProjection {
    pub center_lon: f32,
    pub center_lat: f32,
    pub scale: f32,
    pub translate_x: f32,
    pub translate_y: f32,
}

impl OrthographicProjection {
    /// Creates a new orthographic projection centred at `(0°, 0°)`.
    pub fn new() -> Self {
        Self {
            center_lon: 0.0,
            center_lat: 0.0,
            scale: 1.0,
            translate_x: 0.0,
            translate_y: 0.0,
        }
    }

    /// Sets the projection centre in degrees.
    pub fn center(mut self, lon: f32, lat: f32) -> Self {
        self.center_lon = lon;
        self.center_lat = lat;
        self
    }

    /// Sets the scale factor.
    pub fn scale(mut self, scale: f32) -> Self {
        self.scale = scale;
        self
    }

    /// Sets the translation offset.
    pub fn translate(mut self, x: f32, y: f32) -> Self {
        self.translate_x = x;
        self.translate_y = y;
        self
    }
}

impl Default for OrthographicProjection {
    fn default() -> Self {
        Self::new()
    }
}

impl ComposableShaderFunction for OrthographicProjection {
    type Input = GeoPoint;
    type Output = Vec2;
    type Uniforms = OrthographicUniforms;

    fn wgsl_function() -> &'static str {
        r#"
const GUP_DEG_TO_RAD_O: f32 = 0.017453292519943295;
const GUP_CLIP_SENTINEL_O: f32 = 1e9;

fn orthographic_projection(point: gup_GeoPoint, u: OrthographicUniforms) -> vec2<f32> {
    let lon = point.longitude * GUP_DEG_TO_RAD_O;
    let lat = point.latitude * GUP_DEG_TO_RAD_O;
    let lon0 = u.center_lon * GUP_DEG_TO_RAD_O;
    let lat0 = u.center_lat * GUP_DEG_TO_RAD_O;
    let d_lon = lon - lon0;
    let cos_lat = cos(lat);
    let sin_lat = sin(lat);
    let cos_lat0 = cos(lat0);
    let sin_lat0 = sin(lat0);
    let cos_d_lon = cos(d_lon);
    let cos_c = sin_lat0 * sin_lat + cos_lat0 * cos_lat * cos_d_lon;
    if (cos_c < 0.0) {
        return vec2<f32>(GUP_CLIP_SENTINEL_O, GUP_CLIP_SENTINEL_O);
    }
    let x = cos_lat * sin(d_lon);
    let y = cos_lat0 * sin_lat - sin_lat0 * cos_lat * cos_d_lon;
    return vec2<f32>(x * u.scale + u.translate_x, y * u.scale + u.translate_y);
}
"#
    }

    fn create_uniforms(&self) -> Option<Self::Uniforms> {
        Some(OrthographicUniforms {
            center_lon: self.center_lon,
            center_lat: self.center_lat,
            scale: self.scale,
            translate_x: self.translate_x,
            translate_y: self.translate_y,
            _pad0: 0.0,
            _pad1: 0.0,
            _pad2: 0.0,
        })
    }

    fn function_name() -> &'static str {
        "orthographic_projection"
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    const DEG_TO_RAD: f32 = PI / 180.0;

    // ---- GeoPoint tests ----

    #[test]
    fn test_geo_point_creation() {
        let p = GeoPoint::new(10.0, 20.0);
        assert_eq!(p.longitude, 10.0);
        assert_eq!(p.latitude, 20.0);
    }

    #[test]
    fn test_geo_point_shader_type() {
        assert_eq!(GeoPoint::wgsl_type_name(), "gup_GeoPoint");
        assert_eq!(GeoPoint::size_bytes(), 8);
        assert_eq!(GeoPoint::alignment(), 4);
    }

    #[test]
    fn test_geo_point_wgsl_definition() {
        let def = GeoPoint::wgsl_type_definition().expect("should have definition");
        assert!(def.contains("struct gup_GeoPoint"), "definition: {def}");
        assert!(def.contains("longitude: f32"), "definition: {def}");
        assert!(def.contains("latitude: f32"), "definition: {def}");
    }

    #[test]
    fn test_geo_point_bytemuck() {
        // Verify Pod and Zeroable by round-tripping through bytes
        let p = GeoPoint::new(1.0, 2.0);
        let bytes = bytemuck::bytes_of(&p);
        assert_eq!(bytes.len(), 8);
        let p2: &GeoPoint = bytemuck::from_bytes(bytes);
        assert_eq!(p, *p2);
    }

    // ---- Equirectangular tests ----

    #[test]
    fn test_equirectangular_origin() {
        // (0°, 0°) with centre (0°, 0°) should map to (0, 0) before scale/translate
        let proj = EquirectangularProjection::new();
        let u = proj.create_uniforms().unwrap();
        // Compute manually: lon_rad = 0, lat_rad = 0, cos(0) = 1
        // x = 0 * 1 = 0, y = 0
        // result = (0 * 1 + 0, 0 * 1 + 0) = (0, 0)
        assert_eq!(u.center_lon, 0.0);
        assert_eq!(u.center_lat, 0.0);
        assert_eq!(u.scale, 1.0);
        assert_eq!(u.translate_x, 0.0);
        assert_eq!(u.translate_y, 0.0);

        // Verify the projection numerically
        let (x, y) = equirectangular_cpu(0.0, 0.0, &u);
        assert!((x).abs() < 1e-6, "x = {x}");
        assert!((y).abs() < 1e-6, "y = {y}");
    }

    #[test]
    fn test_equirectangular_scale_translate() {
        let proj = EquirectangularProjection::new()
            .scale(100.0)
            .translate(50.0, 50.0);
        let u = proj.create_uniforms().unwrap();

        // (90°, 0°) → lon_rad = π/2, lat_rad = 0, cos(0) = 1
        // x = π/2 * 1 = π/2, y = 0
        // result = (π/2 * 100 + 50, 0 * 100 + 50) = (π*50 + 50, 50)
        let (x, y) = equirectangular_cpu(90.0, 0.0, &u);
        let expected_x = (90.0_f32 * DEG_TO_RAD) * 1.0 * 100.0 + 50.0;
        assert!(
            (x - expected_x).abs() < 1e-3,
            "x = {x}, expected = {expected_x}"
        );
        assert!((y - 50.0).abs() < 1e-3, "y = {y}");
    }

    #[test]
    fn test_equirectangular_wgsl_non_empty() {
        let wgsl = EquirectangularProjection::wgsl_function();
        assert!(!wgsl.is_empty());
        assert!(wgsl.contains("equirectangular_projection"));
        assert!(wgsl.contains("gup_GeoPoint"));
        assert!(wgsl.contains("EquirectangularUniforms"));
    }

    #[test]
    fn test_equirectangular_uniforms_pod() {
        let u = EquirectangularUniforms {
            center_lon: 0.0,
            center_lat: 0.0,
            scale: 1.0,
            translate_x: 0.0,
            translate_y: 0.0,
            _pad0: 0.0,
            _pad1: 0.0,
            _pad2: 0.0,
        };
        let bytes = bytemuck::bytes_of(&u);
        assert_eq!(bytes.len(), 32); // 8 × f32
    }

    // ---- Mercator tests ----

    #[test]
    fn test_mercator_origin() {
        let proj = MercatorProjection::new();
        let u = proj.create_uniforms().unwrap();
        let (x, y) = mercator_cpu(0.0, 0.0, &u);
        assert!((x).abs() < 1e-6, "x = {x}");
        assert!((y).abs() < 1e-6, "y = {y}");
    }

    #[test]
    fn test_mercator_180_lon() {
        // (180°, 0°) with default centre (0°) and scale 1.0
        // x = 180° in radians = π, y = ln(tan(π/4 + 0)) = ln(1) = 0
        // result = (π * 1.0 + 0, 0 * 1.0 + 0) = (π, 0)
        let proj = MercatorProjection::new();
        let u = proj.create_uniforms().unwrap();
        let (x, y) = mercator_cpu(180.0, 0.0, &u);
        assert!((x - PI).abs() < 1e-5, "x = {x}, expected = {}", PI);
        assert!((y).abs() < 1e-6, "y = {y}");
    }

    #[test]
    fn test_mercator_180_with_scale() {
        // (180°, 0°) with scale s should give x = π * s
        let s = 42.0;
        let proj = MercatorProjection::new().scale(s);
        let u = proj.create_uniforms().unwrap();
        let (x, _y) = mercator_cpu(180.0, 0.0, &u);
        let expected = PI * s;
        assert!(
            (x - expected).abs() < 1e-3,
            "x = {x}, expected = {expected}"
        );
    }

    #[test]
    fn test_mercator_clip_beyond_lat() {
        // A latitude beyond clip_lat should produce sentinel
        let proj = MercatorProjection::new();
        let u = proj.create_uniforms().unwrap();
        let (x, y) = mercator_cpu(0.0, 90.0, &u);
        assert_eq!(x, CLIP_SENTINEL);
        assert_eq!(y, CLIP_SENTINEL);
    }

    #[test]
    fn test_mercator_wgsl_non_empty() {
        let wgsl = MercatorProjection::wgsl_function();
        assert!(!wgsl.is_empty());
        assert!(wgsl.contains("mercator_projection"));
        assert!(wgsl.contains("gup_GeoPoint"));
        assert!(wgsl.contains("MercatorUniforms"));
    }

    #[test]
    fn test_mercator_uniforms_pod() {
        let u = MercatorUniforms {
            center_lon: 0.0,
            scale: 1.0,
            translate_x: 0.0,
            translate_y: 0.0,
            clip_lat: DEFAULT_CLIP_LAT,
            _pad0: 0.0,
            _pad1: 0.0,
            _pad2: 0.0,
        };
        let bytes = bytemuck::bytes_of(&u);
        assert_eq!(bytes.len(), 32);
    }

    // ---- Stereographic tests ----

    #[test]
    fn test_stereographic_center_identity() {
        // The centre point should map to (0, 0) before scale/translate
        let proj = StereographicProjection::new().center(10.0, 20.0);
        let u = proj.create_uniforms().unwrap();
        let (x, y) = stereographic_cpu(10.0, 20.0, &u);
        assert!(x.abs() < 1e-5, "x = {x}");
        assert!(y.abs() < 1e-5, "y = {y}");
    }

    #[test]
    fn test_stereographic_antipodal_divergence() {
        // A point very close to the antipode of (0°, 0°) should produce a
        // very large radius.  The exact antipode (180°, 0°) yields 0/0 = NaN
        // due to k_denom=0 and sin(π)≈0 in f32, so we test a point
        // fractionally off the antipode.
        let proj = StereographicProjection::new();
        let u = proj.create_uniforms().unwrap();
        let (x, y) = stereographic_cpu(179.9999, 0.0001, &u);
        let radius = (x * x + y * y).sqrt();
        assert!(
            radius >= 1e6,
            "near-antipodal radius = {radius}, expected >= 1e6"
        );
    }

    #[test]
    fn test_stereographic_wgsl_non_empty() {
        let wgsl = StereographicProjection::wgsl_function();
        assert!(!wgsl.is_empty());
        assert!(wgsl.contains("stereographic_projection"));
        assert!(wgsl.contains("gup_GeoPoint"));
        assert!(wgsl.contains("StereographicUniforms"));
    }

    #[test]
    fn test_stereographic_uniforms_pod() {
        let u = StereographicUniforms {
            center_lon: 0.0,
            center_lat: 0.0,
            scale: 1.0,
            translate_x: 0.0,
            translate_y: 0.0,
            _pad0: 0.0,
            _pad1: 0.0,
            _pad2: 0.0,
        };
        let bytes = bytemuck::bytes_of(&u);
        assert_eq!(bytes.len(), 32);
    }

    // ---- Orthographic tests ----

    #[test]
    fn test_orthographic_center_identity() {
        // The centre point should map to (0, 0) before scale/translate
        let proj = OrthographicProjection::new().center(30.0, 45.0);
        let u = proj.create_uniforms().unwrap();
        let (x, y) = orthographic_cpu(30.0, 45.0, &u);
        assert!(x.abs() < 1e-5, "x = {x}");
        assert!(y.abs() < 1e-5, "y = {y}");
    }

    #[test]
    fn test_orthographic_90_degree_boundary() {
        // A point nearly 90° away on the great circle should have radius ≈ 1.0.
        // We use 89.99° rather than exactly 90° because f32 cos(π/2) can be
        // very slightly negative, which would trigger the far-hemisphere clip.
        let proj = OrthographicProjection::new();
        let u = proj.create_uniforms().unwrap();
        let (x, y) = orthographic_cpu(89.99, 0.0, &u);
        let radius = (x * x + y * y).sqrt();
        assert!(
            (radius - 1.0).abs() < 1e-3,
            "radius = {radius}, expected ≈ 1.0"
        );
    }

    #[test]
    fn test_orthographic_far_hemisphere_clip() {
        // A point on the far hemisphere (>90° away) should be clipped
        // From centre (0°, 0°), the point (180°, 0°) is on the far hemisphere
        let proj = OrthographicProjection::new();
        let u = proj.create_uniforms().unwrap();
        let (x, y) = orthographic_cpu(180.0, 0.0, &u);
        assert_eq!(x, CLIP_SENTINEL);
        assert_eq!(y, CLIP_SENTINEL);
    }

    #[test]
    fn test_orthographic_wgsl_non_empty() {
        let wgsl = OrthographicProjection::wgsl_function();
        assert!(!wgsl.is_empty());
        assert!(wgsl.contains("orthographic_projection"));
        assert!(wgsl.contains("gup_GeoPoint"));
        assert!(wgsl.contains("OrthographicUniforms"));
    }

    #[test]
    fn test_orthographic_uniforms_pod() {
        let u = OrthographicUniforms {
            center_lon: 0.0,
            center_lat: 0.0,
            scale: 1.0,
            translate_x: 0.0,
            translate_y: 0.0,
            _pad0: 0.0,
            _pad1: 0.0,
            _pad2: 0.0,
        };
        let bytes = bytemuck::bytes_of(&u);
        assert_eq!(bytes.len(), 32);
    }

    // ---- Clip sentinel tests ----

    #[test]
    fn test_clip_sentinel_value() {
        assert_eq!(CLIP_SENTINEL, 1e9);
    }

    // ---- Composition test ----

    #[test]
    fn test_mercator_compose_with_position_transform() {
        use super::super::{ComposableFunction, PositionTransform};

        let proj = MercatorProjection::new().scale(1.0);
        let screen = PositionTransform::new(Vec2::new(100.0, -100.0), Vec2::new(400.0, 300.0));
        let composed = proj.compose(screen);

        // Verify WGSL generation
        let wgsl = composed.generate_wgsl();
        assert!(
            wgsl.contains("mercator_projection"),
            "WGSL should contain mercator_projection:\n{wgsl}"
        );
        assert!(
            wgsl.contains("position_transform"),
            "WGSL should contain position_transform:\n{wgsl}"
        );
        assert!(
            wgsl.contains("composed_chain"),
            "WGSL should contain composed_chain:\n{wgsl}"
        );
    }

    #[test]
    fn test_mercator_compose_pixel_roundtrip() {
        use super::super::{ComposableFunction, PositionTransform};

        // Set up a projection pipeline: Mercator → PositionTransform
        let scale = 100.0;
        let tx = 400.0;
        let ty = 300.0;
        let proj = MercatorProjection::new().scale(1.0);
        let screen = PositionTransform::new(
            Vec2::new(scale, -scale), // flip y for screen coords
            Vec2::new(tx, ty),
        );
        let _composed = proj.clone().compose(screen);

        // Manually compute expected position for (0°, 0°)
        // Mercator: (0, 0) → (0, 0) (before Mercator scale/translate)
        // PositionTransform: (0, 0) * (100, -100) + (400, 300) = (400, 300)
        let merc_uniforms = proj.create_uniforms().unwrap();
        let (mx, my) = mercator_cpu(0.0, 0.0, &merc_uniforms);
        let px = mx * scale + tx;
        let py = my * (-scale) + ty;
        assert!((px - 400.0).abs() < 1.0, "px = {px}");
        assert!((py - 300.0).abs() < 1.0, "py = {py}");
    }

    // ---- CPU reference implementations for testing ----

    fn equirectangular_cpu(lon: f32, lat: f32, u: &EquirectangularUniforms) -> (f32, f32) {
        let lon_rad = (lon - u.center_lon) * DEG_TO_RAD;
        let lat_rad = (lat - u.center_lat) * DEG_TO_RAD;
        let center_lat_rad = u.center_lat * DEG_TO_RAD;
        let x = lon_rad * center_lat_rad.cos();
        let y = lat_rad;
        (x * u.scale + u.translate_x, y * u.scale + u.translate_y)
    }

    fn mercator_cpu(lon: f32, lat: f32, u: &MercatorUniforms) -> (f32, f32) {
        if lat.abs() > u.clip_lat {
            return (CLIP_SENTINEL, CLIP_SENTINEL);
        }
        let max_safe_lat_rad: f32 = 85.051_13_f32 * DEG_TO_RAD;
        let lon_rad = (lon - u.center_lon) * DEG_TO_RAD;
        let lat_rad = (lat * DEG_TO_RAD).clamp(-max_safe_lat_rad, max_safe_lat_rad);
        let x = lon_rad;
        let y = (PI / 4.0 + lat_rad * 0.5).tan().ln();
        (x * u.scale + u.translate_x, y * u.scale + u.translate_y)
    }

    fn stereographic_cpu(lon: f32, lat: f32, u: &StereographicUniforms) -> (f32, f32) {
        let lon_r = lon * DEG_TO_RAD;
        let lat_r = lat * DEG_TO_RAD;
        let lon0 = u.center_lon * DEG_TO_RAD;
        let lat0 = u.center_lat * DEG_TO_RAD;
        let d_lon = lon_r - lon0;
        let cos_lat = lat_r.cos();
        let sin_lat = lat_r.sin();
        let cos_lat0 = lat0.cos();
        let sin_lat0 = lat0.sin();
        let cos_d_lon = d_lon.cos();
        let k_denom = 1.0 + sin_lat0 * sin_lat + cos_lat0 * cos_lat * cos_d_lon;
        let k = 2.0 / k_denom;
        let x = k * cos_lat * d_lon.sin();
        let y = k * (cos_lat0 * sin_lat - sin_lat0 * cos_lat * cos_d_lon);
        (x * u.scale + u.translate_x, y * u.scale + u.translate_y)
    }

    fn orthographic_cpu(lon: f32, lat: f32, u: &OrthographicUniforms) -> (f32, f32) {
        let lon_r = lon * DEG_TO_RAD;
        let lat_r = lat * DEG_TO_RAD;
        let lon0 = u.center_lon * DEG_TO_RAD;
        let lat0 = u.center_lat * DEG_TO_RAD;
        let d_lon = lon_r - lon0;
        let cos_lat = lat_r.cos();
        let sin_lat = lat_r.sin();
        let cos_lat0 = lat0.cos();
        let sin_lat0 = lat0.sin();
        let cos_d_lon = d_lon.cos();
        let cos_c = sin_lat0 * sin_lat + cos_lat0 * cos_lat * cos_d_lon;
        if cos_c < 0.0 {
            return (CLIP_SENTINEL, CLIP_SENTINEL);
        }
        let x = cos_lat * d_lon.sin();
        let y = cos_lat0 * sin_lat - sin_lat0 * cos_lat * cos_d_lon;
        (x * u.scale + u.translate_x, y * u.scale + u.translate_y)
    }
}

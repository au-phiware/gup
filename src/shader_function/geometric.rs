// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Geometric and spatial transformation shader functions.
//!
//! Provides GPU-accelerated geometric transformations including position
//! transform, polar coordinates, matrix transforms, projection transforms,
//! and distance functions.

use super::core::*;

/// Basic 2D position transformation with scaling and translation.
///
/// This is a basic example shader function. Advanced geometric transformations
/// (polar coordinates, matrix transforms, projections) will be added in future updates.
#[derive(Clone, Debug)]
pub struct PositionTransform {
    /// Scale factor for position.
    pub scale: Vec2,
    /// Offset for position.
    pub offset: Vec2,
}

impl PositionTransform {
    /// Creates a new position transform with the given scale and offset.
    pub fn new(scale: Vec2, offset: Vec2) -> Self {
        Self { scale, offset }
    }
}

/// GPU uniform data for the position transform shader function.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PositionTransformUniforms {
    /// Scale factor (x, y).
    pub scale: [f32; 2],
    /// Offset (x, y).
    pub offset: [f32; 2],
}

impl ShaderUniform for PositionTransformUniforms {
    fn wgsl_struct_definition() -> String {
        "struct PositionTransformUniforms {\n    scale: vec2<f32>,\n    offset: vec2<f32>,\n}"
            .to_string()
    }

    fn wgsl_type_name() -> &'static str {
        "PositionTransformUniforms"
    }
}

impl ComposableShaderFunction for PositionTransform {
    type Input = Vec2;
    type Output = Vec2;
    type Uniforms = PositionTransformUniforms;

    fn wgsl_function() -> &'static str {
        r#"
        fn position_transform(pos: vec2<f32>, transform: PositionTransformUniforms) -> vec2<f32> {
            return pos * vec2<f32>(transform.scale) + vec2<f32>(transform.offset);
        }
        "#
    }

    fn create_uniforms(&self) -> Option<Self::Uniforms> {
        Some(PositionTransformUniforms {
            scale: [self.scale.x, self.scale.y],
            offset: [self.offset.x, self.offset.y],
        })
    }

    fn function_name() -> &'static str {
        "position_transform"
    }
}

// ============================================================================
// Geometric and Spatial Functions (GUP-053 AC3)
// ============================================================================

/// Polar coordinate transform.
///
/// Converts 2D Cartesian coordinates to polar form (angle, radius)
/// or from polar to Cartesian, relative to a configurable center point.
#[derive(Clone, Debug)]
pub struct PolarTransform {
    /// Center point for the polar coordinate system
    pub center: Vec2,
    /// Angle offset in radians
    pub angle_offset: f32,
    /// If true, converts Cartesian → Polar; if false, Polar → Cartesian
    pub to_polar: bool,
}

impl PolarTransform {
    /// Creates a new polar transform with the given centre and angle offset.
    pub fn new(center: Vec2, angle_offset: f32) -> Self {
        Self {
            center,
            angle_offset,
            to_polar: true,
        }
    }

    /// Creates a transform that converts Cartesian → Polar.
    pub fn to_polar(center: Vec2) -> Self {
        Self {
            center,
            angle_offset: 0.0,
            to_polar: true,
        }
    }

    /// Creates a transform that converts Polar → Cartesian.
    pub fn to_cartesian(center: Vec2) -> Self {
        Self {
            center,
            angle_offset: 0.0,
            to_polar: false,
        }
    }
}

/// GPU uniform data for the polar coordinate transform shader function.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PolarTransformUniforms {
    /// X component of the centre point.
    pub center_x: f32,
    /// Y component of the centre point.
    pub center_y: f32,
    /// Angle offset in radians.
    pub angle_offset: f32,
    /// 0 = Cartesian→Polar, 1 = Polar→Cartesian
    pub direction: u32,
}

impl ShaderUniform for PolarTransformUniforms {
    fn wgsl_struct_definition() -> String {
        "struct PolarTransformUniforms {\n    center_x: f32,\n    center_y: f32,\n    angle_offset: f32,\n    direction: u32,\n}".to_string()
    }

    fn wgsl_type_name() -> &'static str {
        "PolarTransformUniforms"
    }
}

impl ComposableShaderFunction for PolarTransform {
    type Input = Vec2;
    type Output = Vec2;
    type Uniforms = PolarTransformUniforms;

    fn wgsl_function() -> &'static str {
        r#"
        fn polar_transform(pos: vec2<f32>, params: PolarTransformUniforms) -> vec2<f32> {
            if (params.direction == 0u) {
                // Cartesian to Polar
                let dx = pos.x - params.center_x;
                let dy = pos.y - params.center_y;
                let radius = sqrt(dx * dx + dy * dy);
                let angle = atan2(dy, dx) + params.angle_offset;
                return vec2<f32>(angle, radius);
            } else {
                // Polar to Cartesian (x=angle, y=radius)
                let angle = pos.x + params.angle_offset;
                let radius = pos.y;
                return vec2<f32>(
                    params.center_x + radius * cos(angle),
                    params.center_y + radius * sin(angle)
                );
            }
        }
        "#
    }

    fn create_uniforms(&self) -> Option<Self::Uniforms> {
        Some(PolarTransformUniforms {
            center_x: self.center.x,
            center_y: self.center.y,
            angle_offset: self.angle_offset,
            direction: if self.to_polar { 0 } else { 1 },
        })
    }

    fn function_name() -> &'static str {
        "polar_transform"
    }
}

/// General 2D affine matrix transformation.
///
/// Applies a 3×3 affine transformation matrix (stored as 2×3) to 2D points.
/// Supports rotation, scaling, shearing, and translation.
#[derive(Clone, Debug)]
pub struct MatrixTransform {
    /// 2×3 affine matrix stored as [a, b, c, d, tx, ty]
    /// where the transform is: x' = a*x + c*y + tx, y' = b*x + d*y + ty
    pub matrix: [f32; 6],
}

impl MatrixTransform {
    /// Creates a new matrix transform from a 6-element affine matrix.
    pub fn new(matrix: [f32; 6]) -> Self {
        Self { matrix }
    }

    /// Creates an identity transform (no change).
    pub fn identity() -> Self {
        Self::new([1.0, 0.0, 0.0, 1.0, 0.0, 0.0])
    }

    /// Creates a rotation transform (angle in radians).
    pub fn rotation(angle: f32) -> Self {
        let c = angle.cos();
        let s = angle.sin();
        Self::new([c, s, -s, c, 0.0, 0.0])
    }

    /// Creates a scaling transform.
    pub fn scaling(sx: f32, sy: f32) -> Self {
        Self::new([sx, 0.0, 0.0, sy, 0.0, 0.0])
    }

    /// Creates a translation transform.
    pub fn translation(tx: f32, ty: f32) -> Self {
        Self::new([1.0, 0.0, 0.0, 1.0, tx, ty])
    }
}

/// GPU uniform data for the affine matrix transform shader function.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MatrixTransformUniforms {
    /// Row-major: a, b, c, d
    pub matrix: [f32; 4],
    /// Translation: tx, ty
    pub translation: [f32; 2],
    /// Padding for GPU alignment.
    pub _padding: [f32; 2],
}

impl ShaderUniform for MatrixTransformUniforms {
    fn wgsl_struct_definition() -> String {
        "struct MatrixTransformUniforms {\n    matrix: vec4<f32>,\n    translation: vec2<f32>,\n}"
            .to_string()
    }

    fn wgsl_type_name() -> &'static str {
        "MatrixTransformUniforms"
    }
}

impl ComposableShaderFunction for MatrixTransform {
    type Input = Vec2;
    type Output = Vec2;
    type Uniforms = MatrixTransformUniforms;

    fn wgsl_function() -> &'static str {
        r#"
        fn matrix_transform(pos: vec2<f32>, params: MatrixTransformUniforms) -> vec2<f32> {
            let x = params.matrix.x * pos.x + params.matrix.z * pos.y + params.translation.x;
            let y = params.matrix.y * pos.x + params.matrix.w * pos.y + params.translation.y;
            return vec2<f32>(x, y);
        }
        "#
    }

    fn create_uniforms(&self) -> Option<Self::Uniforms> {
        Some(MatrixTransformUniforms {
            matrix: [
                self.matrix[0],
                self.matrix[1],
                self.matrix[2],
                self.matrix[3],
            ],
            translation: [self.matrix[4], self.matrix[5]],
            _padding: [0.0; 2],
        })
    }

    fn function_name() -> &'static str {
        "matrix_transform"
    }
}

/// Viewport projection transform.
///
/// Maps data coordinates (domain) to screen coordinates (viewport).
/// Supports independent X and Y axis mapping with configurable inversion.
#[derive(Clone, Debug)]
pub struct ProjectionTransform {
    /// Minimum data coordinate
    pub data_min: Vec2,
    /// Maximum data coordinate
    pub data_max: Vec2,
    /// Minimum screen coordinate (top-left)
    pub viewport_min: Vec2,
    /// Maximum screen coordinate (bottom-right)
    pub viewport_max: Vec2,
}

impl ProjectionTransform {
    /// Creates a new projection transform with the given coordinate ranges.
    pub fn new(data_min: Vec2, data_max: Vec2, viewport_min: Vec2, viewport_max: Vec2) -> Self {
        Self {
            data_min,
            data_max,
            viewport_min,
            viewport_max,
        }
    }
}

/// GPU uniform data for the viewport projection transform shader function.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ProjectionTransformUniforms {
    /// Minimum data range coordinates.
    pub data_min: [f32; 2],
    /// Maximum data range coordinates.
    pub data_max: [f32; 2],
    /// Minimum viewport coordinates.
    pub viewport_min: [f32; 2],
    /// Maximum viewport coordinates.
    pub viewport_max: [f32; 2],
}

impl ShaderUniform for ProjectionTransformUniforms {
    fn wgsl_struct_definition() -> String {
        "struct ProjectionTransformUniforms {\n    data_min: vec2<f32>,\n    data_max: vec2<f32>,\n    viewport_min: vec2<f32>,\n    viewport_max: vec2<f32>,\n}".to_string()
    }

    fn wgsl_type_name() -> &'static str {
        "ProjectionTransformUniforms"
    }
}

impl ComposableShaderFunction for ProjectionTransform {
    type Input = Vec2;
    type Output = Vec2;
    type Uniforms = ProjectionTransformUniforms;

    fn wgsl_function() -> &'static str {
        r#"
        fn projection_transform(pos: vec2<f32>, params: ProjectionTransformUniforms) -> vec2<f32> {
            let data_min = vec2<f32>(params.data_min);
            let data_max = vec2<f32>(params.data_max);
            let vp_min = vec2<f32>(params.viewport_min);
            let vp_max = vec2<f32>(params.viewport_max);
            let normalized = (pos - data_min) / (data_max - data_min);
            return vp_min + normalized * (vp_max - vp_min);
        }
        "#
    }

    fn create_uniforms(&self) -> Option<Self::Uniforms> {
        Some(ProjectionTransformUniforms {
            data_min: [self.data_min.x, self.data_min.y],
            data_max: [self.data_max.x, self.data_max.y],
            viewport_min: [self.viewport_min.x, self.viewport_min.y],
            viewport_max: [self.viewport_max.x, self.viewport_max.y],
        })
    }

    fn function_name() -> &'static str {
        "projection_transform"
    }
}

/// Distance function for computing distance between a point and a reference.
///
/// Computes Euclidean distance from each input point to a configurable
/// reference point. Useful for radial visualizations and proximity-based
/// visual encoding.
#[derive(Clone, Debug)]
pub struct DistanceFunction {
    /// Reference point to measure distance from
    pub reference_point: Vec2,
}

impl DistanceFunction {
    /// Creates a new distance function from the given reference point.
    pub fn new(reference_point: Vec2) -> Self {
        Self { reference_point }
    }

    /// Creates a distance function from the origin.
    pub fn from_origin() -> Self {
        Self::new(Vec2::zero())
    }
}

/// GPU uniform data for the distance function shader.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct DistanceFunctionUniforms {
    /// X coordinate of the reference point.
    pub ref_x: f32,
    /// Y coordinate of the reference point.
    pub ref_y: f32,
    /// Padding for GPU alignment.
    pub _padding: [f32; 2],
}

impl ShaderUniform for DistanceFunctionUniforms {
    fn wgsl_struct_definition() -> String {
        "struct DistanceFunctionUniforms {\n    ref_x: f32,\n    ref_y: f32,\n}".to_string()
    }

    fn wgsl_type_name() -> &'static str {
        "DistanceFunctionUniforms"
    }
}

impl ComposableShaderFunction for DistanceFunction {
    type Input = Vec2;
    type Output = f32;
    type Uniforms = DistanceFunctionUniforms;

    fn wgsl_function() -> &'static str {
        r#"
        fn distance_fn(pos: vec2<f32>, params: DistanceFunctionUniforms) -> f32 {
            let dx = pos.x - params.ref_x;
            let dy = pos.y - params.ref_y;
            return sqrt(dx * dx + dy * dy);
        }
        "#
    }

    fn create_uniforms(&self) -> Option<Self::Uniforms> {
        Some(DistanceFunctionUniforms {
            ref_x: self.reference_point.x,
            ref_y: self.reference_point.y,
            _padding: [0.0; 2],
        })
    }

    fn function_name() -> &'static str {
        "distance_fn"
    }
}

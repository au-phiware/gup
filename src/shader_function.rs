// Gup - GPU-Accelerated Data Visualization Library
// Copyright (C) 2025 Corin Lawson <corin@phiware.com.au>
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

//! Composable Shader Function System
//!
//! This module implements Gup's core innovation: treating all data transformations as
//! composable WGSL functions that run on the GPU. This enables type-safe composition
//! of data processing pipelines with guaranteed performance.
//!
//! ## Current Implementation Status
//! - ✅ Core trait system with type safety
//! - ✅ Function composition with compile-time validation
//! - ✅ Uniform buffer management
//! - ✅ Basic example functions
//! - ✅ Dynamic WGSL code generation templates
//! - ✅ Compile-time WGSL template macro system
//! - ✅ Runtime dynamic composition code generation
//! - ✅ GPU shader compilation validation
//!
//! ## Template System Features
//! - `wgsl_function!` macro for defining shader functions with WGSL templates
//! - Automatic generation of uniform structures with proper GPU alignment
//! - Dynamic WGSL composition for function chains
//! - Compile-time type safety and validation
//! - GPU compilation testing for generated WGSL
//!
//! ## Usage Examples
//!
//! ### Using the Template Macro
//! ```rust,ignore
//! wgsl_function! {
//!     struct MyTransform {
//!         scale: f32,
//!         offset: f32,
//!     }
//!
//!
//!     uniforms MyTransformUniforms {
//!         scale: f32,
//!         offset: f32,
//!     }
//!
//!
//!     fn my_transform(f32) -> f32,
//!
//!
//!     wgsl {
//!         "fn my_transform(value: f32, uniforms: MyTransformUniforms) -> f32 {\n    return value * uniforms.scale + uniforms.offset;\n}"
//!     }
//! }
//! ```
//!
//! ### Function Composition
//! ```rust,ignore
//! let scale = LinearScale::new(0.0, 100.0, 0.0, 1.0);
//! let min_color = Vec4::new(0.0, 0.0, 0.0, 1.0);
//! let max_color = Vec4::new(1.0, 1.0, 1.0, 1.0);
//! let color_map = ColorMap::new(min_color, max_color);
//! let composed = scale.compose(color_map);
//!
//! // Generated WGSL is available via:
//! let wgsl_code = composed.generate_wgsl();
//! ```
//!
//! ## Future Development
//! - GUP-052: GPU pipeline builder integration
//! - GUP-053: Expanded shader function library
//! - GUP-054: Performance optimization

pub mod macros;

use crate::buffer::{BufferType, GpuBuffer};
use crate::error::GupResult;
use std::marker::PhantomData;
use wgpu::{Device, Queue};

// Re-export macros for easier access
pub use macros::*;
// Bring macro into scope for this module
use crate::wgsl_function;

pub trait ShaderType: Clone + Send + Sync + 'static {
    fn wgsl_type_name() -> &'static str;
    fn wgsl_type_definition() -> Option<&'static str> {
        None
    }
    fn size_bytes() -> usize;
    fn alignment() -> usize;
}

impl ShaderType for f32 {
    fn wgsl_type_name() -> &'static str {
        "f32"
    }
    fn size_bytes() -> usize {
        4
    }
    fn alignment() -> usize {
        4
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

impl ShaderType for Vec2 {
    fn wgsl_type_name() -> &'static str {
        "vec2<f32>"
    }
    fn size_bytes() -> usize {
        8
    }
    fn alignment() -> usize {
        8
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub _padding: f32, // Ensure 16-byte alignment
}

impl Vec3 {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self {
            x,
            y,
            z,
            _padding: 0.0,
        }
    }
}

impl ShaderType for Vec3 {
    fn wgsl_type_name() -> &'static str {
        "vec3<f32>"
    }
    fn size_bytes() -> usize {
        12
    }
    fn alignment() -> usize {
        16
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vec4 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl Vec4 {
    pub fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self { x, y, z, w }
    }
}

impl ShaderType for Vec4 {
    fn wgsl_type_name() -> &'static str {
        "vec4<f32>"
    }
    fn size_bytes() -> usize {
        16
    }
    fn alignment() -> usize {
        16
    }
}

pub trait ComposableShaderFunction {
    type Input: ShaderType;
    type Output: ShaderType;
    type Uniforms: bytemuck::Pod + bytemuck::Zeroable;

    /// Returns the WGSL code for this shader function.
    ///
    /// Dynamic WGSL code generation is now supported via template system.
    fn wgsl_function() -> &'static str;

    /// Generates dynamic WGSL code with proper type substitution.
    /// This method enables runtime WGSL generation for composed functions.
    fn generate_wgsl(&self) -> String {
        Self::wgsl_function().to_string()
    }

    fn create_uniforms(&self) -> Option<Self::Uniforms>;
    fn function_name() -> &'static str;
}

/// Trait for checking type compatibility between shader function inputs and outputs.
///
/// Note: Currently provides permissive default behavior. Advanced type compatibility
/// checking with automatic conversions will be implemented in GUP-053.
pub trait TypeCompatible<T> {
    fn is_compatible() -> bool {
        true
    }
}

impl<T> TypeCompatible<T> for T {}

pub struct FunctionChain<A: ComposableShaderFunction, B: ComposableShaderFunction>
where
    A::Output: TypeCompatible<B::Input>,
{
    first: A,
    second: B,
    _phantom: PhantomData<(A::Output, B::Input)>,
}

impl<A: ComposableShaderFunction, B: ComposableShaderFunction> FunctionChain<A, B>
where
    A::Output: TypeCompatible<B::Input>,
{
    pub fn new(first: A, second: B) -> Self {
        Self {
            first,
            second,
            _phantom: PhantomData,
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct ChainUniforms<A, B>
where
    A: Copy,
    B: Copy,
{
    pub first: A,
    pub second: B,
}

unsafe impl<A: bytemuck::Pod, B: bytemuck::Pod> bytemuck::Pod for ChainUniforms<A, B>
where
    A: bytemuck::Zeroable + Copy,
    B: bytemuck::Zeroable + Copy,
{
}

unsafe impl<A: bytemuck::Zeroable, B: bytemuck::Zeroable> bytemuck::Zeroable for ChainUniforms<A, B>
where
    A: Copy,
    B: Copy,
{
}

impl<A: ComposableShaderFunction, B: ComposableShaderFunction> ComposableShaderFunction
    for FunctionChain<A, B>
where
    A::Output: TypeCompatible<B::Input>,
    A::Uniforms: bytemuck::Pod + bytemuck::Zeroable + Copy,
    B::Uniforms: bytemuck::Pod + bytemuck::Zeroable + Copy,
{
    type Input = A::Input;
    type Output = B::Output;
    type Uniforms = ChainUniforms<A::Uniforms, B::Uniforms>;

    fn wgsl_function() -> &'static str {
        // Dynamic WGSL generation placeholder for function composition
        // Full template substitution would happen at pipeline creation time
        "fn composed_chain(input: INPUT_TYPE, uniforms: ChainUniforms) -> OUTPUT_TYPE {\n    let intermediate = FIRST_FUNCTION(input, uniforms.first);\n    return SECOND_FUNCTION(intermediate, uniforms.second);\n}"
    }

    fn generate_wgsl(&self) -> String {
        // Generate proper WGSL with type substitution for composition
        format!(
            "fn composed_chain(input: {}, uniforms: ChainUniforms) -> {} {{\n    let intermediate = {}(input, uniforms.first);\n    return {}(intermediate, uniforms.second);\n}}",
            <A::Input as ShaderType>::wgsl_type_name(),
            <B::Output as ShaderType>::wgsl_type_name(),
            A::function_name(),
            B::function_name()
        )
    }

    fn create_uniforms(&self) -> Option<Self::Uniforms> {
        match (self.first.create_uniforms(), self.second.create_uniforms()) {
            (Some(first), Some(second)) => Some(ChainUniforms { first, second }),
            _ => None,
        }
    }

    fn function_name() -> &'static str {
        "composed_chain"
    }
}

/// Enables composition of shader functions through a fluent API.
///
/// Note: GPU pipeline integration for composed functions will be implemented in GUP-052.
pub trait ComposableFunction<T: ComposableShaderFunction>: ComposableShaderFunction
where
    Self::Output: TypeCompatible<T::Input>,
{
    fn compose(self, other: T) -> FunctionChain<Self, T>
    where
        Self: Sized,
    {
        FunctionChain::new(self, other)
    }
}

impl<S: ComposableShaderFunction, T: ComposableShaderFunction> ComposableFunction<T> for S where
    S::Output: TypeCompatible<T::Input>
{
}

/// Manages uniform buffers for shader functions.
///
/// Note: Advanced uniform buffer batching and pipeline binding will be implemented in GUP-052.
pub struct UniformBuffer<T: bytemuck::Pod + bytemuck::Zeroable> {
    buffer: Option<GpuBuffer<T>>,
}

impl<T: bytemuck::Pod + bytemuck::Zeroable> Default for UniformBuffer<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: bytemuck::Pod + bytemuck::Zeroable> UniformBuffer<T> {
    pub fn new() -> Self {
        Self { buffer: None }
    }

    pub fn upload(&mut self, device: &Device, queue: &Queue, data: &T) -> GupResult<()> {
        if self.buffer.is_none() {
            self.buffer = Some(GpuBuffer::new(device, BufferType::Uniform, 1));
        }

        if let Some(ref mut buffer) = self.buffer {
            buffer.upload(device, queue, &[*data])?;
        }

        Ok(())
    }

    pub fn buffer(&self) -> Option<&GpuBuffer<T>> {
        self.buffer.as_ref()
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LinearScaleUniforms {
    pub domain_min: f32,
    pub domain_max: f32,
    pub range_min: f32,
    pub range_max: f32,
}

/// Linear scaling transformation for numeric data.
///
/// This is a basic example shader function. Advanced mathematical transformations
/// (logarithmic, exponential, power law) will be added in GUP-053.
pub struct LinearScale {
    pub domain_min: f32,
    pub domain_max: f32,
    pub range_min: f32,
    pub range_max: f32,
}

impl LinearScale {
    pub fn new(domain_min: f32, domain_max: f32, range_min: f32, range_max: f32) -> Self {
        Self {
            domain_min,
            domain_max,
            range_min,
            range_max,
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
            let normalized = (value - scale.domain_min) / (scale.domain_max - scale.domain_min);
            return scale.range_min + normalized * (scale.range_max - scale.range_min);
        }
        "#
    }

    fn create_uniforms(&self) -> Option<Self::Uniforms> {
        Some(LinearScaleUniforms {
            domain_min: self.domain_min,
            domain_max: self.domain_max,
            range_min: self.range_min,
            range_max: self.range_max,
        })
    }

    fn function_name() -> &'static str {
        "linear_scale"
    }
}

// Example of new shader function using the template macro system
wgsl_function! {
    struct LinearScaleTemplate {
        domain_min: f32,
        domain_max: f32,
        range_min: f32,
        range_max: f32,
    }

    uniforms LinearScaleTemplateUniforms {
        domain_min: f32,
        domain_max: f32,
        range_min: f32,
        range_max: f32,
    }

    fn linear_scale_template(f32) -> f32,

    wgsl {
        "fn linear_scale_template(value: f32, scale: LinearScaleTemplateUniforms) -> f32 {\n    let normalized = (value - scale.domain_min) / (scale.domain_max - scale.domain_min);\n    return scale.range_min + normalized * (scale.range_max - scale.range_min);\n}"
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ColorMapUniforms {
    pub min_color: [f32; 4],
    pub max_color: [f32; 4],
}

/// Simple two-color linear interpolation for data visualization.
///
/// This is a basic example shader function. Advanced color mapping features
/// (HSV color space, multi-stop gradients, color space conversions) will be added in future updates.
pub struct ColorMap {
    pub min_color: Vec4,
    pub max_color: Vec4,
}

impl ColorMap {
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

/// Basic 2D position transformation with scaling and translation.
///
/// This is a basic example shader function. Advanced geometric transformations
/// (polar coordinates, matrix transforms, projections) will be added in future updates.
pub struct PositionTransform {
    pub scale: Vec2,
    pub offset: Vec2,
}

impl PositionTransform {
    pub fn new(scale: Vec2, offset: Vec2) -> Self {
        Self { scale, offset }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PositionTransformUniforms {
    pub scale: [f32; 2],
    pub offset: [f32; 2],
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shader_type_properties() {
        assert_eq!(f32::wgsl_type_name(), "f32");
        assert_eq!(f32::size_bytes(), 4);
        assert_eq!(f32::alignment(), 4);

        assert_eq!(Vec2::wgsl_type_name(), "vec2<f32>");
        assert_eq!(Vec2::size_bytes(), 8);
        assert_eq!(Vec2::alignment(), 8);

        assert_eq!(Vec3::wgsl_type_name(), "vec3<f32>");
        assert_eq!(Vec3::size_bytes(), 12);
        assert_eq!(Vec3::alignment(), 16);

        assert_eq!(Vec4::wgsl_type_name(), "vec4<f32>");
        assert_eq!(Vec4::size_bytes(), 16);
        assert_eq!(Vec4::alignment(), 16);
    }

    #[test]
    fn test_compatible_trait() {
        assert!(f32::is_compatible());
        assert!(Vec2::is_compatible());
    }

    #[test]
    fn test_vec_types() {
        let v2 = Vec2::new(1.0, 2.0);
        assert_eq!(v2.x, 1.0);
        assert_eq!(v2.y, 2.0);

        let v3 = Vec3::new(1.0, 2.0, 3.0);
        assert_eq!(v3.x, 1.0);
        assert_eq!(v3.y, 2.0);
        assert_eq!(v3.z, 3.0);

        let v4 = Vec4::new(1.0, 2.0, 3.0, 4.0);
        assert_eq!(v4.x, 1.0);
        assert_eq!(v4.y, 2.0);
        assert_eq!(v4.z, 3.0);
        assert_eq!(v4.w, 4.0);
    }

    #[test]
    fn test_linear_scale_shader_function() {
        let scale = LinearScale::new(0.0, 100.0, 0.0, 1.0);
        let uniforms = scale.create_uniforms().unwrap();

        assert_eq!(LinearScale::function_name(), "linear_scale");
        assert!(LinearScale::wgsl_function().contains("linear_scale"));
        assert_eq!(uniforms.domain_min, 0.0);
        assert_eq!(uniforms.domain_max, 100.0);
        assert_eq!(uniforms.range_min, 0.0);
        assert_eq!(uniforms.range_max, 1.0);
    }

    #[test]
    fn test_color_map_shader_function() {
        let min_color = Vec4::new(0.0, 0.0, 0.0, 1.0);
        let max_color = Vec4::new(1.0, 1.0, 1.0, 1.0);
        let color_map = ColorMap::new(min_color, max_color);
        let uniforms = color_map.create_uniforms().unwrap();

        assert_eq!(ColorMap::function_name(), "color_map");
        assert!(ColorMap::wgsl_function().contains("color_map"));
        assert_eq!(uniforms.min_color, [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(uniforms.max_color, [1.0, 1.0, 1.0, 1.0]);
    }

    #[test]
    fn test_position_transform_shader_function() {
        let scale = Vec2::new(2.0, 3.0);
        let offset = Vec2::new(1.0, 1.5);
        let transform = PositionTransform::new(scale, offset);
        let uniforms = transform.create_uniforms().unwrap();

        assert_eq!(PositionTransform::function_name(), "position_transform");
        assert!(PositionTransform::wgsl_function().contains("position_transform"));
        assert_eq!(uniforms.scale, [2.0, 3.0]);
        assert_eq!(uniforms.offset, [1.0, 1.5]);
    }

    #[test]
    fn test_function_composition() {
        let scale = LinearScale::new(0.0, 100.0, 0.0, 1.0);
        let color_map = ColorMap::new(Vec4::new(0.0, 0.0, 0.0, 1.0), Vec4::new(1.0, 1.0, 1.0, 1.0));

        let composed = scale.compose(color_map);
        assert_eq!(
            FunctionChain::<LinearScale, ColorMap>::function_name(),
            "composed_chain"
        );

        let chain_uniforms = composed.create_uniforms();
        assert!(chain_uniforms.is_some());
    }

    #[test]
    fn test_type_compatibility() {
        let scale = LinearScale::new(0.0, 100.0, 0.0, 1.0);
        let color_map = ColorMap::new(Vec4::new(0.0, 0.0, 0.0, 1.0), Vec4::new(1.0, 1.0, 1.0, 1.0));

        let _valid_composition = scale.compose(color_map);
    }

    #[test]
    fn test_chain_uniforms() {
        let scale_uniforms = LinearScaleUniforms {
            domain_min: 0.0,
            domain_max: 10.0,
            range_min: 0.0,
            range_max: 1.0,
        };

        let color_uniforms = ColorMapUniforms {
            min_color: [0.0, 0.0, 0.0, 1.0],
            max_color: [1.0, 1.0, 1.0, 1.0],
        };

        let chain_uniforms = ChainUniforms {
            first: scale_uniforms,
            second: color_uniforms,
        };

        assert_eq!(chain_uniforms.first.domain_min, 0.0);
        assert_eq!(chain_uniforms.second.min_color[0], 0.0);
    }

    #[test]
    fn test_uniform_buffer() {
        let buffer: UniformBuffer<LinearScaleUniforms> = UniformBuffer::new();
        assert!(buffer.buffer().is_none());
    }
}

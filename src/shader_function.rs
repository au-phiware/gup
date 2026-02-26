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
//! let min_color = vec4![0.0, 0.0, 0.0, 1.0];
//! let max_color = vec4![1.0, 1.0, 1.0, 1.0];
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

pub mod conversions;
pub mod macros;

use crate::buffer::{BufferType, GpuBuffer};
use crate::error::{GupError, GupResult};
use std::marker::PhantomData;
use std::sync::Arc;
use wgpu::{Device, Queue};

/// Macro for creating 2D vectors.
///
/// Creates a `Vec2` with proper GPU alignment (8 bytes).
///
/// # Example
///
/// ```rust
/// use gup::*;
///
/// let position = vec2![10.0, 20.0];
/// assert_eq!(position.x, 10.0);
/// assert_eq!(position.y, 20.0);
/// ```
///
/// # Performance
///
/// This is a zero-cost abstraction that expands at compile time to direct
/// struct initialization.
#[macro_export]
macro_rules! vec2 {
    ($x:expr, $y:expr) => {
        Vec2 { x: $x, y: $y }
    };
}

/// Macro for creating 3D vectors with proper GPU alignment.
///
/// Creates a `Vec3` with automatic padding for 16-byte GPU alignment.
/// The padding is handled automatically and you never need to specify it.
///
/// # Example
///
/// ```rust
/// use gup::*;
///
/// let position = vec3![1.0, 2.0, 3.0];
/// assert_eq!(position.x, 1.0);
/// assert_eq!(position.y, 2.0);
/// assert_eq!(position.z, 3.0);
/// // _padding field is automatically set to 0.0
/// ```
///
/// # GPU Memory Layout
///
/// `Vec3` requires 16-byte alignment on GPU (12 bytes data + 4 bytes padding).
/// This macro ensures correct layout for GPU buffer uploads.
///
/// # Performance
///
/// Zero-cost abstraction with compile-time expansion. Identical performance
/// to manual struct initialization.
#[macro_export]
macro_rules! vec3 {
    ($x:expr, $y:expr, $z:expr) => {
        Vec3 {
            x: $x,
            y: $y,
            z: $z,
            _padding: 0.0,
        }
    };
}

/// Macro for creating 4D vectors.
///
/// Creates a `Vec4` commonly used for RGBA colors or homogeneous coordinates.
///
/// # Example
///
/// ```rust
/// use gup::*;
///
/// // RGBA color: orange with full opacity
/// let color = vec4![1.0, 0.5, 0.0, 1.0];
///
/// // Homogeneous coordinates
/// let position = vec4![10.0, 20.0, 0.0, 1.0];
/// ```
///
/// # GPU Memory Layout
///
/// `Vec4` is 16 bytes (4 components × 4 bytes each) with natural GPU alignment.
///
/// # Performance
///
/// Zero-cost abstraction that expands at compile time.
#[macro_export]
macro_rules! vec4 {
    ($x:expr, $y:expr, $z:expr, $w:expr) => {
        Vec4 {
            x: $x,
            y: $y,
            z: $z,
            w: $w,
        }
    };
}

/// Macro for creating 2x2 matrices.
///
/// Creates a `Mat2` with row-major order for natural reading.
///
/// # Example
///
/// ```rust
/// use gup::*;
///
/// // Identity matrix
/// let identity = mat2![
///     1.0, 0.0,
///     0.0, 1.0
/// ];
///
/// // 90-degree rotation
/// let rotation = mat2![
///     0.0, -1.0,
///     1.0,  0.0
/// ];
/// ```
///
/// # GPU Memory Layout
///
/// `Mat2` is 16 bytes with GPU-standard padding between rows.
///
/// # Performance
///
/// Compile-time expansion with zero runtime overhead.
#[macro_export]
macro_rules! mat2 {
    ($m00:expr, $m01:expr,
     $m10:expr, $m11:expr) => {
        Mat2 {
            m00: $m00,
            m01: $m01,
            m10: $m10,
            m11: $m11,
        }
    };
}

/// Macro for creating 3x3 matrices with row-major ordering.
///
/// Creates a `Mat3` with automatic padding for GPU alignment. Takes 9 arguments
/// representing the matrix elements in row-major (natural reading) order.
///
/// # Example
///
/// ```rust
/// use gup::*;
///
/// // Identity matrix
/// let identity = mat3![
///     1.0, 0.0, 0.0,
///     0.0, 1.0, 0.0,
///     0.0, 0.0, 1.0
/// ];
///
/// // 2D affine transformation (scale + translate)
/// let transform = mat3![
///     2.0, 0.0, 10.0,  // Scale X=2, Translate X=10
///     0.0, 2.0, 20.0,  // Scale Y=2, Translate Y=20
///     0.0, 0.0,  1.0   // Homogeneous coordinate
/// ];
/// ```
///
/// # GPU Memory Layout
///
/// `Mat3` requires 48 bytes with padding between rows for GPU alignment.
/// The padding is handled automatically.
///
/// # Performance
///
/// Zero-cost abstraction with compile-time expansion.
#[macro_export]
macro_rules! mat3 {
    ($m00:expr, $m01:expr, $m02:expr,
     $m10:expr, $m11:expr, $m12:expr,
     $m20:expr, $m21:expr, $m22:expr) => {
        Mat3 {
            m00: $m00,
            m01: $m01,
            m02: $m02,
            _padding0: 0.0,
            m10: $m10,
            m11: $m11,
            m12: $m12,
            _padding1: 0.0,
            m20: $m20,
            m21: $m21,
            m22: $m22,
            _padding2: 0.0,
        }
    };
}

/// Macro for creating 4x4 matrices with row-major ordering.
///
/// Creates a `Mat4` with proper alignment for GPU usage. Takes 16 arguments
/// representing the matrix elements in row-major (natural reading) order.
/// Commonly used for 3D transformations and projection matrices.
///
/// # Example
///
/// ```rust
/// use gup::*;
///
/// // Identity matrix
/// let identity = mat4![
///     1.0, 0.0, 0.0, 0.0,
///     0.0, 1.0, 0.0, 0.0,
///     0.0, 0.0, 1.0, 0.0,
///     0.0, 0.0, 0.0, 1.0
/// ];
///
/// // Translation matrix
/// let translate = mat4![
///     1.0, 0.0, 0.0, 10.0,
///     0.0, 1.0, 0.0, 20.0,
///     0.0, 0.0, 1.0, 30.0,
///     0.0, 0.0, 0.0,  1.0
/// ];
/// ```
///
/// # GPU Memory Layout
///
/// `Mat4` is 64 bytes with natural GPU alignment (16 bytes per row).
///
/// # Performance
///
/// Compile-time expansion with zero runtime overhead. Identical to direct
/// struct initialization.
#[macro_export]
macro_rules! mat4 {
    ($m00:expr, $m01:expr, $m02:expr, $m03:expr,
     $m10:expr, $m11:expr, $m12:expr, $m13:expr,
     $m20:expr, $m21:expr, $m22:expr, $m23:expr,
     $m30:expr, $m31:expr, $m32:expr, $m33:expr) => {
        Mat4 {
            m00: $m00,
            m01: $m01,
            m02: $m02,
            m03: $m03,
            m10: $m10,
            m11: $m11,
            m12: $m12,
            m13: $m13,
            m20: $m20,
            m21: $m21,
            m22: $m22,
            m23: $m23,
            m30: $m30,
            m31: $m31,
            m32: $m32,
            m33: $m33,
        }
    };
}

// Re-export macros for easier access
pub use macros::*;
// Bring macro into scope for this module
use crate::wgsl_function;
pub use conversions::AutoConvert;

pub trait ShaderType: Clone + Send + Sync + 'static {
    /// Returns the WGSL type name for this type
    fn wgsl_type_name() -> &'static str;

    /// Returns optional WGSL type definition (for custom structs)
    fn wgsl_type_definition() -> Option<&'static str> {
        None
    }

    /// Returns the size in bytes for GPU memory layout
    fn size_bytes() -> usize;

    /// Returns the alignment requirement for GPU memory layout
    fn alignment() -> usize;

    /// Checks if this type is compatible with another shader type.
    ///
    /// Two types are compatible if they have the same WGSL type name.
    /// For automatic conversion compatibility, use `is_compatible_through`.
    fn is_compatible_with<T: ShaderType>() -> bool {
        Self::wgsl_type_name() == T::wgsl_type_name()
    }
}

/// Trait for flexible compatibility checking including automatic conversions.
///
/// This trait extends basic type compatibility to include automatic conversions
/// via the `AutoConvert` trait. Types are considered compatible if:
/// 1. They are the same type (direct compatibility)
/// 2. There is an automatic conversion available (via `AutoConvert`)
///
/// # Example
///
/// ```rust,ignore
/// use gup::shader_function::*;
///
/// // Direct compatibility
/// assert!(f32::is_compatible::<f32>());
///
/// // Automatic conversion compatibility
/// assert!(<f32 as FlexibleCompatibility>::is_compatible_through::<Vec3>());
/// ```
pub trait FlexibleCompatibility: ShaderType {
    /// Checks if this type can be used where another type is expected,
    /// either directly or through automatic conversion.
    fn is_compatible_through<T: ShaderType>() -> bool
    where
        Self: AutoConvert<T>,
    {
        // Direct compatibility or automatic conversion available
        Self::is_compatible_with::<T>() || Self::can_convert()
    }

    /// Returns the WGSL code needed to convert this type to another type.
    ///
    /// If no conversion is needed (types are the same), returns None.
    /// If conversion is needed and available, returns the WGSL conversion expression.
    fn conversion_code_for<T: ShaderType>(input_expr: &str) -> Option<String>
    where
        Self: AutoConvert<T>,
    {
        if Self::is_compatible_with::<T>() {
            None // No conversion needed
        } else {
            Some(Self::conversion_wgsl(input_expr))
        }
    }
}

/// Trait for types that have WGSL struct definitions.
///
/// This trait is automatically implemented by the `#[derive(WgslStruct)]` macro
/// and enables automatic generation of WGSL struct definitions from Rust types.
///
/// # Example
///
/// ```rust,ignore
/// use gup_macros::WgslStruct;
///
/// #[derive(WgslStruct, Clone, Copy)]
/// #[repr(C)]
/// struct Material {
///     albedo: Vec3,
///     metallic: f32,
///     roughness: f32,
/// }
///
/// // The macro automatically implements WgslStructType:
/// let wgsl = Material::wgsl_struct_definition();
/// assert!(wgsl.contains("struct Material"));
/// ```
pub trait WgslStructType: ShaderType {
    /// Returns the complete WGSL struct definition as a string.
    ///
    /// The definition includes the struct name and all fields with their types.
    fn wgsl_struct_definition() -> &'static str;

    /// Returns the struct name as it appears in WGSL.
    ///
    /// This is typically the same as the Rust struct name.
    fn struct_name() -> &'static str;
}

// Blanket implementation for all ShaderType implementors
impl<T: ShaderType> FlexibleCompatibility for T {}

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

impl ShaderType for i32 {
    fn wgsl_type_name() -> &'static str {
        "i32"
    }
    fn size_bytes() -> usize {
        4
    }
    fn alignment() -> usize {
        4
    }
}

impl ShaderType for u32 {
    fn wgsl_type_name() -> &'static str {
        "u32"
    }
    fn size_bytes() -> usize {
        4
    }
    fn alignment() -> usize {
        4
    }
}

impl ShaderType for bool {
    fn wgsl_type_name() -> &'static str {
        "bool"
    }
    fn size_bytes() -> usize {
        4 // WGSL bool is 32-bit
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
    /// Creates a new 2D vector.
    ///
    /// # Example
    /// ```
    /// use gup::shader_function::Vec2;
    /// let v = Vec2::new(1.0, 2.0);
    /// assert_eq!(v.x, 1.0);
    /// assert_eq!(v.y, 2.0);
    /// ```
    #[inline]
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// Creates a vector with both components set to zero.
    ///
    /// # Example
    /// ```
    /// use gup::shader_function::Vec2;
    /// let v = Vec2::zero();
    /// assert_eq!(v.x, 0.0);
    /// assert_eq!(v.y, 0.0);
    /// ```
    #[inline]
    pub const fn zero() -> Self {
        Self { x: 0.0, y: 0.0 }
    }

    /// Creates a vector with both components set to one.
    ///
    /// # Example
    /// ```
    /// use gup::shader_function::Vec2;
    /// let v = Vec2::one();
    /// assert_eq!(v.x, 1.0);
    /// assert_eq!(v.y, 1.0);
    /// ```
    #[inline]
    pub const fn one() -> Self {
        Self { x: 1.0, y: 1.0 }
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
    /// Creates a new 3D vector.
    ///
    /// # Example
    /// ```
    /// use gup::shader_function::Vec3;
    /// let v = Vec3::new(1.0, 2.0, 3.0);
    /// assert_eq!(v.x, 1.0);
    /// assert_eq!(v.y, 2.0);
    /// assert_eq!(v.z, 3.0);
    /// ```
    #[inline]
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self {
            x,
            y,
            z,
            _padding: 0.0,
        }
    }

    /// Creates a vector with all components set to zero.
    ///
    /// # Example
    /// ```
    /// use gup::shader_function::Vec3;
    /// let v = Vec3::zero();
    /// assert_eq!(v.x, 0.0);
    /// assert_eq!(v.y, 0.0);
    /// assert_eq!(v.z, 0.0);
    /// ```
    #[inline]
    pub const fn zero() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            _padding: 0.0,
        }
    }

    /// Creates a vector with all components set to one.
    ///
    /// # Example
    /// ```
    /// use gup::shader_function::Vec3;
    /// let v = Vec3::one();
    /// assert_eq!(v.x, 1.0);
    /// assert_eq!(v.y, 1.0);
    /// assert_eq!(v.z, 1.0);
    /// ```
    #[inline]
    pub const fn one() -> Self {
        Self {
            x: 1.0,
            y: 1.0,
            z: 1.0,
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
    /// Creates a new 4D vector.
    ///
    /// # Example
    /// ```
    /// use gup::shader_function::Vec4;
    /// let v = Vec4::new(1.0, 2.0, 3.0, 4.0);
    /// assert_eq!(v.x, 1.0);
    /// assert_eq!(v.y, 2.0);
    /// assert_eq!(v.z, 3.0);
    /// assert_eq!(v.w, 4.0);
    /// ```
    #[inline]
    pub const fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self { x, y, z, w }
    }

    /// Creates a vector with all components set to zero.
    ///
    /// # Example
    /// ```
    /// use gup::shader_function::Vec4;
    /// let v = Vec4::zero();
    /// assert_eq!(v.x, 0.0);
    /// assert_eq!(v.y, 0.0);
    /// assert_eq!(v.z, 0.0);
    /// assert_eq!(v.w, 0.0);
    /// ```
    #[inline]
    pub const fn zero() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            w: 0.0,
        }
    }

    /// Creates a vector with all components set to one.
    ///
    /// # Example
    /// ```
    /// use gup::shader_function::Vec4;
    /// let v = Vec4::one();
    /// assert_eq!(v.x, 1.0);
    /// assert_eq!(v.y, 1.0);
    /// assert_eq!(v.z, 1.0);
    /// assert_eq!(v.w, 1.0);
    /// ```
    #[inline]
    pub const fn one() -> Self {
        Self {
            x: 1.0,
            y: 1.0,
            z: 1.0,
            w: 1.0,
        }
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

/// 2x2 matrix type for 2D transformations
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Mat2 {
    pub m00: f32,
    pub m01: f32,
    pub m10: f32,
    pub m11: f32,
}

impl Mat2 {
    /// Creates a new 2x2 matrix.
    ///
    /// # Arguments
    /// * `m00`, `m01` - First row
    /// * `m10`, `m11` - Second row
    ///
    /// # Example
    /// ```
    /// use gup::shader_function::Mat2;
    /// let m = Mat2::new(
    ///     1.0, 0.0,
    ///     0.0, 1.0
    /// );
    /// assert_eq!(m.m00, 1.0);
    /// assert_eq!(m.m11, 1.0);
    /// ```
    #[inline]
    pub const fn new(m00: f32, m01: f32, m10: f32, m11: f32) -> Self {
        Self { m00, m01, m10, m11 }
    }

    /// Creates an identity matrix.
    ///
    /// # Example
    /// ```
    /// use gup::shader_function::Mat2;
    /// let m = Mat2::identity();
    /// assert_eq!(m.m00, 1.0);
    /// assert_eq!(m.m11, 1.0);
    /// assert_eq!(m.m01, 0.0);
    /// assert_eq!(m.m10, 0.0);
    /// ```
    #[inline]
    pub fn identity() -> Self {
        mat2![1.0, 0.0, 0.0, 1.0]
    }
}

impl ShaderType for Mat2 {
    fn wgsl_type_name() -> &'static str {
        "mat2x2<f32>"
    }
    fn size_bytes() -> usize {
        16 // 2x2 matrix = 4 f32s
    }
    fn alignment() -> usize {
        8 // Column alignment
    }
}

/// 3x3 matrix type for 2D/3D transformations
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Mat3 {
    pub m00: f32,
    pub m01: f32,
    pub m02: f32,
    pub _padding0: f32,
    pub m10: f32,
    pub m11: f32,
    pub m12: f32,
    pub _padding1: f32,
    pub m20: f32,
    pub m21: f32,
    pub m22: f32,
    pub _padding2: f32,
}

impl Mat3 {
    /// Creates a new 3x3 matrix.
    ///
    /// # Arguments
    /// * `m00`, `m01`, `m02` - First row
    /// * `m10`, `m11`, `m12` - Second row
    /// * `m20`, `m21`, `m22` - Third row
    ///
    /// # Example
    /// ```
    /// use gup::shader_function::Mat3;
    /// let m = Mat3::new(
    ///     1.0, 0.0, 0.0,
    ///     0.0, 1.0, 0.0,
    ///     0.0, 0.0, 1.0
    /// );
    /// assert_eq!(m.m00, 1.0);
    /// assert_eq!(m.m11, 1.0);
    /// assert_eq!(m.m22, 1.0);
    /// ```
    #[inline]
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        m00: f32,
        m01: f32,
        m02: f32,
        m10: f32,
        m11: f32,
        m12: f32,
        m20: f32,
        m21: f32,
        m22: f32,
    ) -> Self {
        Self {
            m00,
            m01,
            m02,
            _padding0: 0.0,
            m10,
            m11,
            m12,
            _padding1: 0.0,
            m20,
            m21,
            m22,
            _padding2: 0.0,
        }
    }

    /// Creates an identity matrix.
    ///
    /// # Example
    /// ```
    /// use gup::shader_function::Mat3;
    /// let m = Mat3::identity();
    /// assert_eq!(m.m00, 1.0);
    /// assert_eq!(m.m11, 1.0);
    /// assert_eq!(m.m22, 1.0);
    /// ```
    #[inline]
    pub fn identity() -> Self {
        mat3![1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]
    }
}

impl ShaderType for Mat3 {
    fn wgsl_type_name() -> &'static str {
        "mat3x3<f32>"
    }
    fn size_bytes() -> usize {
        48 // 3 columns * 16 bytes (vec4 alignment)
    }
    fn alignment() -> usize {
        16 // vec4 alignment for columns
    }
}

/// 4x4 matrix type for 3D transformations
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Mat4 {
    pub m00: f32,
    pub m01: f32,
    pub m02: f32,
    pub m03: f32,
    pub m10: f32,
    pub m11: f32,
    pub m12: f32,
    pub m13: f32,
    pub m20: f32,
    pub m21: f32,
    pub m22: f32,
    pub m23: f32,
    pub m30: f32,
    pub m31: f32,
    pub m32: f32,
    pub m33: f32,
}

impl Mat4 {
    /// Creates a new 4x4 matrix.
    ///
    /// # Arguments
    /// * `m00`, `m01`, `m02`, `m03` - First row
    /// * `m10`, `m11`, `m12`, `m13` - Second row
    /// * `m20`, `m21`, `m22`, `m23` - Third row
    /// * `m30`, `m31`, `m32`, `m33` - Fourth row
    ///
    /// # Example
    /// ```
    /// use gup::shader_function::Mat4;
    /// let m = Mat4::new(
    ///     1.0, 0.0, 0.0, 0.0,
    ///     0.0, 1.0, 0.0, 0.0,
    ///     0.0, 0.0, 1.0, 0.0,
    ///     0.0, 0.0, 0.0, 1.0
    /// );
    /// assert_eq!(m.m00, 1.0);
    /// assert_eq!(m.m11, 1.0);
    /// assert_eq!(m.m22, 1.0);
    /// assert_eq!(m.m33, 1.0);
    /// ```
    #[inline]
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        m00: f32,
        m01: f32,
        m02: f32,
        m03: f32,
        m10: f32,
        m11: f32,
        m12: f32,
        m13: f32,
        m20: f32,
        m21: f32,
        m22: f32,
        m23: f32,
        m30: f32,
        m31: f32,
        m32: f32,
        m33: f32,
    ) -> Self {
        Self {
            m00,
            m01,
            m02,
            m03,
            m10,
            m11,
            m12,
            m13,
            m20,
            m21,
            m22,
            m23,
            m30,
            m31,
            m32,
            m33,
        }
    }

    /// Creates an identity matrix.
    ///
    /// # Example
    /// ```
    /// use gup::shader_function::Mat4;
    /// let m = Mat4::identity();
    /// assert_eq!(m.m00, 1.0);
    /// assert_eq!(m.m11, 1.0);
    /// assert_eq!(m.m22, 1.0);
    /// assert_eq!(m.m33, 1.0);
    /// ```
    #[inline]
    pub fn identity() -> Self {
        mat4![
            1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0
        ]
    }
}

impl ShaderType for Mat4 {
    fn wgsl_type_name() -> &'static str {
        "mat4x4<f32>"
    }
    fn size_bytes() -> usize {
        64 // 4x4 matrix = 16 f32s
    }
    fn alignment() -> usize {
        16 // vec4 alignment for columns
    }
}

/// Trait for uniform structures that can be automatically converted to WGSL struct definitions.
///
/// This trait enables automatic generation of WGSL struct definitions from Rust uniform types,
/// eliminating manual type mapping and preventing WGSL/Rust struct mismatches.
///
/// # Examples
///
/// ```rust,ignore
/// #[derive(ShaderUniform)]
/// #[repr(C)]
/// struct MyUniforms {
///     scale: f32,
///     offset: f32,
/// }
///
/// // Generates WGSL:
/// // struct MyUniforms {
/// //     scale: f32,
/// //     offset: f32,
/// // }
/// ```
pub trait ShaderUniform: bytemuck::Pod + bytemuck::Zeroable {
    /// Returns the WGSL struct definition for this uniform type.
    ///
    /// The generated WGSL should match the Rust struct layout exactly,
    /// ensuring GPU memory alignment compatibility.
    fn wgsl_struct_definition() -> String;

    /// Returns the WGSL type name for this uniform struct.
    ///
    /// This is used for generating uniform buffer bindings and function signatures.
    fn wgsl_type_name() -> &'static str;
}

// Implement ShaderUniform for basic types (they don't need struct definitions)
impl ShaderUniform for f32 {
    fn wgsl_struct_definition() -> String {
        String::new() // Primitive types don't have struct definitions
    }

    fn wgsl_type_name() -> &'static str {
        "f32"
    }
}

impl ShaderUniform for i32 {
    fn wgsl_struct_definition() -> String {
        String::new()
    }

    fn wgsl_type_name() -> &'static str {
        "i32"
    }
}

impl ShaderUniform for u32 {
    fn wgsl_struct_definition() -> String {
        String::new()
    }

    fn wgsl_type_name() -> &'static str {
        "u32"
    }
}

impl ShaderUniform for [f32; 2] {
    fn wgsl_struct_definition() -> String {
        String::new()
    }

    fn wgsl_type_name() -> &'static str {
        "vec2<f32>"
    }
}

impl ShaderUniform for [f32; 3] {
    fn wgsl_struct_definition() -> String {
        String::new()
    }

    fn wgsl_type_name() -> &'static str {
        "vec3<f32>"
    }
}

impl ShaderUniform for [f32; 4] {
    fn wgsl_struct_definition() -> String {
        String::new()
    }

    fn wgsl_type_name() -> &'static str {
        "vec4<f32>"
    }
}

pub trait ComposableShaderFunction {
    type Input: ShaderType;
    type Output: ShaderType;
    type Uniforms: bytemuck::Pod + bytemuck::Zeroable + ShaderUniform;

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
/// This trait enables compile-time validation of shader function composition by ensuring
/// that the output type of one function is compatible with the input type of another.
///
/// Automatic compatibility rules:
/// - Same types are always compatible
/// - f32 can be expanded to Vec2, Vec3, Vec4 (broadcast expansion)
/// - Smaller vector types can be expanded to larger ones with appropriate padding
pub trait ShaderCompatible<T: ShaderType>: ShaderType {
    /// Checks if types are compatible at compile time
    fn is_compatible() -> bool {
        true // Default implementation - compatibility is enforced at trait level
    }
}

// Same types are always compatible
impl<T: ShaderType> ShaderCompatible<T> for T {}

// f32 can be expanded to vector types (broadcast expansion)
impl ShaderCompatible<Vec2> for f32 {
    fn is_compatible() -> bool {
        true
    }
}
impl ShaderCompatible<Vec3> for f32 {
    fn is_compatible() -> bool {
        true
    }
}
impl ShaderCompatible<Vec4> for f32 {
    fn is_compatible() -> bool {
        true
    }
}

// Vector expansion compatibility (smaller to larger)
impl ShaderCompatible<Vec3> for Vec2 {
    fn is_compatible() -> bool {
        true
    }
}
impl ShaderCompatible<Vec4> for Vec2 {
    fn is_compatible() -> bool {
        true
    }
}
impl ShaderCompatible<Vec4> for Vec3 {
    fn is_compatible() -> bool {
        true
    }
}

// For backward compatibility with existing TypeCompatible usage
pub trait TypeCompatible<T> {
    fn is_compatible() -> bool {
        true
    }
}

impl<T> TypeCompatible<T> for T {}

/// A chain of two composed shader functions with compile-time type validation.
///
/// This struct enforces that the output type of the first function is compatible
/// with the input type of the second function, providing type safety at compilation time.
///
/// # Type Safety
///
/// The `FunctionChain` only compiles when `A::Output: ShaderCompatible<B::Input>`.
/// This means type mismatches are caught at compile time rather than runtime.
///
/// # Examples
///
/// Valid composition:
/// ```
/// # use gup::*;
/// let scale = LinearScale::new(0.0, 100.0, 0.0, 1.0); // f32 -> f32
/// let color_map = ColorMap::new(vec4![0.0, 0.0, 0.0, 1.0], vec4![1.0, 1.0, 1.0, 1.0]); // f32 -> Vec4
/// let composed = scale.compose(color_map); // ✓ Compiles: f32 -> f32 -> Vec4
/// ```
///
/// Invalid composition (compile error):
/// ```compile_fail
/// # use gup::*;
/// let scale = LinearScale::new(0.0, 100.0, 0.0, 1.0); // f32 -> f32
/// let position = PositionTransform::new(vec2![1.0, 1.0], vec2![0.0, 0.0]); // Vec2 -> Vec2
/// let bad = position.compose(scale); // ✗ Compile error: Vec2 not compatible with f32
/// ```
pub struct FunctionChain<A: ComposableShaderFunction, B: ComposableShaderFunction>
where
    A::Output: ShaderCompatible<B::Input>,
{
    first: A,
    second: B,
    _phantom: PhantomData<(A::Output, B::Input)>,
}

impl<A: ComposableShaderFunction, B: ComposableShaderFunction> FunctionChain<A, B>
where
    A::Output: ShaderCompatible<B::Input>,
{
    pub fn new(first: A, second: B) -> Self {
        Self {
            first,
            second,
            _phantom: PhantomData,
        }
    }
}

#[repr(C)]
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

impl<A, B> ShaderUniform for ChainUniforms<A, B>
where
    A: ShaderUniform + Copy,
    B: ShaderUniform + Copy,
{
    fn wgsl_struct_definition() -> String {
        let mut def = String::new();
        // Include nested struct definitions so the generated WGSL is
        // self-contained.  Skip empty definitions (primitive types).
        let first_def = A::wgsl_struct_definition();
        if !first_def.is_empty() {
            def.push_str(&first_def);
            def.push('\n');
        }
        let second_def = B::wgsl_struct_definition();
        if !second_def.is_empty() {
            def.push_str(&second_def);
            def.push('\n');
        }
        def.push_str("struct ChainUniforms {\n");
        def.push_str(&format!("    first: {},\n", A::wgsl_type_name()));
        def.push_str(&format!("    second: {},\n", B::wgsl_type_name()));
        def.push('}');
        def
    }

    fn wgsl_type_name() -> &'static str {
        "ChainUniforms"
    }
}

impl<A: ComposableShaderFunction, B: ComposableShaderFunction> ComposableShaderFunction
    for FunctionChain<A, B>
where
    A::Output: ShaderCompatible<B::Input>,
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
        // Include WGSL for both component functions so the composed code is
        // self-contained when injected into a vertex shader.
        let mut wgsl = String::new();
        wgsl.push_str(self.first.generate_wgsl().trim());
        wgsl.push_str("\n\n");
        wgsl.push_str(self.second.generate_wgsl().trim());
        wgsl.push_str("\n\n");
        // Append the composed entry-point that chains them together.
        wgsl.push_str(&format!(
            "fn composed_chain(input: {}, uniforms: ChainUniforms) -> {} {{\n    let intermediate = {}(input, uniforms.first);\n    return {}(intermediate, uniforms.second);\n}}",
            <A::Input as ShaderType>::wgsl_type_name(),
            <B::Output as ShaderType>::wgsl_type_name(),
            A::function_name(),
            B::function_name()
        ));
        wgsl
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

/// Enables composition of shader functions through a fluent API with compile-time type validation.
///
/// This trait provides a fluent interface for composing shader functions while ensuring
/// type compatibility at compile time. Functions can only be composed if their types
/// are compatible according to the `ShaderCompatible` trait.
pub trait ComposableFunction<T: ComposableShaderFunction>: ComposableShaderFunction
where
    Self::Output: ShaderCompatible<T::Input>,
{
    /// Composes this shader function with another, creating a function chain.
    ///
    /// # Type Safety
    /// This method will only compile if `Self::Output` is compatible with `T::Input`.
    /// The compatibility rules are defined by the `ShaderCompatible` trait implementation.
    ///
    /// # Examples
    /// ```rust,ignore
    /// let scale = LinearScale::new(0.0, 100.0, 0.0, 1.0);  // f32 -> f32
    /// let color_map = ColorMap::new(min_color, max_color);  // f32 -> Vec4
    /// let composed = scale.compose(color_map);              // f32 -> Vec4
    /// ```
    fn compose(self, other: T) -> FunctionChain<Self, T>
    where
        Self: Sized,
    {
        FunctionChain::new(self, other)
    }
}

impl<S: ComposableShaderFunction, T: ComposableShaderFunction> ComposableFunction<T> for S where
    S::Output: ShaderCompatible<T::Input>
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

// ============================================================================
// Advanced Composition Patterns (AC3)
// ============================================================================

/// Parallel composition: applies two functions to the same input and produces both outputs.
///
/// This enables patterns like computing both position and color from a single data value.
/// Output is a tuple-like struct with both results.
#[allow(dead_code)]
pub struct ParallelComposition<A: ComposableShaderFunction, B: ComposableShaderFunction>
where
    A::Input: ShaderCompatible<B::Input>,
{
    first: A,
    second: B,
    _phantom: PhantomData<(A::Input, B::Input)>,
}

impl<A: ComposableShaderFunction, B: ComposableShaderFunction> ParallelComposition<A, B>
where
    A::Input: ShaderCompatible<B::Input>,
{
    pub fn new(first: A, second: B) -> Self {
        Self {
            first,
            second,
            _phantom: PhantomData,
        }
    }
}

/// Output type for parallel composition - holds both results.
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct ParallelOutput<
    A: bytemuck::Pod + bytemuck::Zeroable,
    B: bytemuck::Pod + bytemuck::Zeroable,
> {
    pub first: A,
    pub second: B,
}

unsafe impl<A: bytemuck::Pod + bytemuck::Zeroable, B: bytemuck::Pod + bytemuck::Zeroable>
    bytemuck::Pod for ParallelOutput<A, B>
{
}

unsafe impl<A: bytemuck::Pod + bytemuck::Zeroable, B: bytemuck::Pod + bytemuck::Zeroable>
    bytemuck::Zeroable for ParallelOutput<A, B>
{
}

/// Uniforms for parallel composition - combines uniforms from both functions.
#[derive(Copy, Clone, Debug)]
pub struct ParallelUniforms<A, B>
where
    A: Copy,
    B: Copy,
{
    pub first: A,
    pub second: B,
}

unsafe impl<A: bytemuck::Pod, B: bytemuck::Pod> bytemuck::Pod for ParallelUniforms<A, B>
where
    A: bytemuck::Zeroable + Copy,
    B: bytemuck::Zeroable + Copy,
{
}

unsafe impl<A: bytemuck::Zeroable, B: bytemuck::Zeroable> bytemuck::Zeroable
    for ParallelUniforms<A, B>
where
    A: Copy,
    B: Copy,
{
}

impl<A, B> ShaderUniform for ParallelUniforms<A, B>
where
    A: ShaderUniform + Copy,
    B: ShaderUniform + Copy,
{
    fn wgsl_struct_definition() -> String {
        let mut def = String::from("struct ParallelUniforms {\n");
        def.push_str(&format!("    first: {},\n", A::wgsl_type_name()));
        def.push_str(&format!("    second: {},\n", B::wgsl_type_name()));
        def.push('}');
        def
    }

    fn wgsl_type_name() -> &'static str {
        "ParallelUniforms"
    }
}

impl<A: ShaderType, B: ShaderType> ShaderType for ParallelOutput<A, B>
where
    A: bytemuck::Pod + bytemuck::Zeroable,
    B: bytemuck::Pod + bytemuck::Zeroable,
{
    fn wgsl_type_name() -> &'static str {
        // This will be dynamically generated in the actual WGSL output
        "ParallelOutput"
    }

    fn size_bytes() -> usize {
        A::size_bytes() + B::size_bytes()
    }

    fn alignment() -> usize {
        // Use the maximum alignment of the two types
        A::alignment().max(B::alignment())
    }
}

impl<A: ComposableShaderFunction, B: ComposableShaderFunction> ComposableShaderFunction
    for ParallelComposition<A, B>
where
    A::Input: ShaderCompatible<B::Input>,
    A::Output: bytemuck::Pod + bytemuck::Zeroable + ShaderType,
    B::Output: bytemuck::Pod + bytemuck::Zeroable + ShaderType,
    A::Uniforms: bytemuck::Pod + bytemuck::Zeroable + Copy,
    B::Uniforms: bytemuck::Pod + bytemuck::Zeroable + Copy,
{
    type Input = A::Input;
    type Output = ParallelOutput<A::Output, B::Output>;
    type Uniforms = ParallelUniforms<A::Uniforms, B::Uniforms>;

    fn wgsl_function() -> &'static str {
        // Template for parallel composition
        "fn parallel_composed(input: INPUT_TYPE, uniforms: ParallelUniforms) -> ParallelOutput {\n    let first_result = FIRST_FUNCTION(input, uniforms.first);\n    let second_result = SECOND_FUNCTION(input, uniforms.second);\n    var output: ParallelOutput;\n    output.first = first_result;\n    output.second = second_result;\n    return output;\n}"
    }

    fn generate_wgsl(&self) -> String {
        // Generate the ParallelOutput struct definition
        let mut wgsl = format!(
            "struct ParallelOutput {{\n    first: {},\n    second: {},\n}}\n\n",
            <A::Output as ShaderType>::wgsl_type_name(),
            <B::Output as ShaderType>::wgsl_type_name()
        );

        // Generate the parallel composition function
        wgsl.push_str(&format!(
            "fn parallel_composed(input: {}, uniforms: ParallelUniforms) -> ParallelOutput {{\n",
            <A::Input as ShaderType>::wgsl_type_name()
        ));
        wgsl.push_str(&format!(
            "    let first_result = {}(input, uniforms.first);\n",
            A::function_name()
        ));
        wgsl.push_str(&format!(
            "    let second_result = {}(input, uniforms.second);\n",
            B::function_name()
        ));
        wgsl.push_str("    var output: ParallelOutput;\n");
        wgsl.push_str("    output.first = first_result;\n");
        wgsl.push_str("    output.second = second_result;\n");
        wgsl.push_str("    return output;\n");
        wgsl.push_str("}\n");

        wgsl
    }

    fn create_uniforms(&self) -> Option<Self::Uniforms> {
        match (self.first.create_uniforms(), self.second.create_uniforms()) {
            (Some(first), Some(second)) => Some(ParallelUniforms { first, second }),
            _ => None,
        }
    }

    fn function_name() -> &'static str {
        "parallel_composed"
    }
}

/// Extension trait to enable parallel composition with fluent API.
pub trait ParallelComposable<T: ComposableShaderFunction>: ComposableShaderFunction
where
    Self::Input: ShaderCompatible<T::Input>,
{
    /// Composes two functions in parallel - both receive the same input and produce separate outputs.
    ///
    /// # Type Safety
    /// This method will only compile if both functions accept compatible input types.
    ///
    /// # Examples
    /// ```rust,ignore
    /// let scale = LinearScale::new(0.0, 100.0, 0.0, 1.0);      // f32 -> f32
    /// let color_map = ColorMap::new(min_color, max_color);      // f32 -> Vec4
    /// let parallel = scale.parallel(color_map);                  // f32 -> ParallelOutput<f32, Vec4>
    /// ```
    fn parallel(self, other: T) -> ParallelComposition<Self, T>
    where
        Self: Sized,
    {
        ParallelComposition::new(self, other)
    }
}

impl<S: ComposableShaderFunction, T: ComposableShaderFunction> ParallelComposable<T> for S where
    S::Input: ShaderCompatible<T::Input>
{
}

/// Buffer extraction utilities for ParallelOutput types.
///
/// These utilities enable the Selection API to split `ParallelOutput<A, B>` into
/// separate GPU buffers for individual attribute binding.
pub mod parallel_output_extraction {
    use super::*;

    /// Extract the first component from a buffer of ParallelOutput values.
    ///
    /// # Type Safety
    /// Both A and B must be `Pod` and `Zeroable` for safe GPU memory operations.
    ///
    /// # Memory Layout
    /// This function correctly handles memory alignment and padding in the source buffer.
    pub fn extract_first<A, B>(parallel_buffer: &[ParallelOutput<A, B>]) -> Vec<A>
    where
        A: bytemuck::Pod + bytemuck::Zeroable + Copy,
        B: bytemuck::Pod + bytemuck::Zeroable,
    {
        parallel_buffer.iter().map(|p| p.first).collect()
    }

    /// Extract the second component from a buffer of ParallelOutput values.
    ///
    /// # Type Safety
    /// Both A and B must be `Pod` and `Zeroable` for safe GPU memory operations.
    ///
    /// # Memory Layout
    /// This function correctly handles memory alignment and padding in the source buffer.
    pub fn extract_second<A, B>(parallel_buffer: &[ParallelOutput<A, B>]) -> Vec<B>
    where
        A: bytemuck::Pod + bytemuck::Zeroable,
        B: bytemuck::Pod + bytemuck::Zeroable + Copy,
    {
        parallel_buffer.iter().map(|p| p.second).collect()
    }

    /// Split a ParallelOutput buffer into two separate buffers.
    ///
    /// This is the most efficient way to extract both components when you need
    /// both for separate attribute bindings.
    ///
    /// # Returns
    /// A tuple of (first_buffer, second_buffer).
    ///
    /// # Example
    /// ```rust,ignore
    /// let parallel_buffer = vec![
    ///     ParallelOutput { first: [0.0, 1.0], second: [1.0, 0.0, 0.0, 1.0] },
    ///     ParallelOutput { first: [1.0, 0.0], second: [0.0, 1.0, 0.0, 1.0] },
    /// ];
    /// let (positions, colors) = split_parallel_buffer(&parallel_buffer);
    /// ```
    pub fn split_parallel_buffer<A, B>(parallel_buffer: &[ParallelOutput<A, B>]) -> (Vec<A>, Vec<B>)
    where
        A: bytemuck::Pod + bytemuck::Zeroable + Copy,
        B: bytemuck::Pod + bytemuck::Zeroable + Copy,
    {
        let mut first_buffer = Vec::with_capacity(parallel_buffer.len());
        let mut second_buffer = Vec::with_capacity(parallel_buffer.len());

        for output in parallel_buffer {
            first_buffer.push(output.first);
            second_buffer.push(output.second);
        }

        (first_buffer, second_buffer)
    }
}

/// Conditional composition: applies different functions based on a condition.
///
/// This enables if-then-else logic in shader function pipelines.
#[derive(Clone, Debug)]
pub struct ConditionalFunction<T: ComposableShaderFunction, F: ComposableShaderFunction>
where
    T::Input: ShaderCompatible<F::Input>,
    T::Output: ShaderCompatible<F::Output>,
{
    condition_threshold: f32,
    true_branch: T,
    false_branch: F,
    _phantom: PhantomData<(T::Input, F::Input)>,
}

impl<T: ComposableShaderFunction, F: ComposableShaderFunction> ConditionalFunction<T, F>
where
    T::Input: ShaderCompatible<F::Input>,
    T::Output: ShaderCompatible<F::Output>,
{
    pub fn new(condition_threshold: f32, true_branch: T, false_branch: F) -> Self {
        Self {
            condition_threshold,
            true_branch,
            false_branch,
            _phantom: PhantomData,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct ConditionalUniforms<T, F>
where
    T: Copy,
    F: Copy,
{
    pub condition_threshold: f32,
    pub _padding: [f32; 3],
    pub true_uniforms: T,
    pub false_uniforms: F,
}

unsafe impl<T: bytemuck::Pod, F: bytemuck::Pod> bytemuck::Pod for ConditionalUniforms<T, F>
where
    T: bytemuck::Zeroable + Copy,
    F: bytemuck::Zeroable + Copy,
{
}

unsafe impl<T: bytemuck::Zeroable, F: bytemuck::Zeroable> bytemuck::Zeroable
    for ConditionalUniforms<T, F>
where
    T: Copy,
    F: Copy,
{
}

impl<T: ShaderUniform + Copy, F: ShaderUniform + Copy> ShaderUniform for ConditionalUniforms<T, F> {
    fn wgsl_struct_definition() -> String {
        format!(
            "struct ConditionalUniforms {{\n    condition_threshold: f32,\n    true_uniforms: {},\n    false_uniforms: {},\n}}",
            T::wgsl_type_name(),
            F::wgsl_type_name()
        )
    }

    fn wgsl_type_name() -> &'static str {
        "ConditionalUniforms"
    }
}

impl<T: ComposableShaderFunction, F: ComposableShaderFunction> ComposableShaderFunction
    for ConditionalFunction<T, F>
where
    T::Input: ShaderCompatible<F::Input>,
    T::Output: ShaderCompatible<F::Output>,
    T::Uniforms: bytemuck::Pod + bytemuck::Zeroable + Copy,
    F::Uniforms: bytemuck::Pod + bytemuck::Zeroable + Copy,
{
    type Input = T::Input;
    type Output = T::Output;
    type Uniforms = ConditionalUniforms<T::Uniforms, F::Uniforms>;

    fn wgsl_function() -> &'static str {
        "fn conditional(input: INPUT_TYPE, uniforms: ConditionalUniforms) -> OUTPUT_TYPE {\n    if (input >= uniforms.condition_threshold) {\n        return TRUE_FUNCTION(input, uniforms.true_uniforms);\n    } else {\n        return FALSE_FUNCTION(input, uniforms.false_uniforms);\n    }\n}"
    }

    fn generate_wgsl(&self) -> String {
        format!(
            "fn conditional(input: {}, uniforms: ConditionalUniforms) -> {} {{\n    if (input >= uniforms.condition_threshold) {{\n        return {}(input, uniforms.true_uniforms);\n    }} else {{\n        return {}(input, uniforms.false_uniforms);\n    }}\n}}",
            <T::Input as ShaderType>::wgsl_type_name(),
            <T::Output as ShaderType>::wgsl_type_name(),
            T::function_name(),
            F::function_name()
        )
    }

    fn create_uniforms(&self) -> Option<Self::Uniforms> {
        match (
            self.true_branch.create_uniforms(),
            self.false_branch.create_uniforms(),
        ) {
            (Some(true_uniforms), Some(false_uniforms)) => Some(ConditionalUniforms {
                condition_threshold: self.condition_threshold,
                _padding: [0.0; 3],
                true_uniforms,
                false_uniforms,
            }),
            _ => None,
        }
    }

    fn function_name() -> &'static str {
        "conditional"
    }
}

/// Temporal animation function: interpolates between two values over time.
///
/// Enables smooth transitions and animations in visualizations.
#[derive(Clone, Debug)]
pub struct TemporalInterpolation {
    pub start_value: f32,
    pub end_value: f32,
    pub duration: f32,
}

impl TemporalInterpolation {
    pub fn new(start_value: f32, end_value: f32, duration: f32) -> Self {
        Self {
            start_value,
            end_value,
            duration,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TemporalInterpolationUniforms {
    pub start_value: f32,
    pub end_value: f32,
    pub duration: f32,
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
    Linear,
    EaseInQuad,
    EaseOutQuad,
    EaseInOutQuad,
    EaseInCubic,
    EaseOutCubic,
    EaseInOutCubic,
}

#[derive(Clone, Debug)]
pub struct Easing {
    pub function: EasingFunction,
}

impl Easing {
    pub fn new(function: EasingFunction) -> Self {
        Self { function }
    }

    pub fn linear() -> Self {
        Self {
            function: EasingFunction::Linear,
        }
    }

    pub fn ease_in_out() -> Self {
        Self {
            function: EasingFunction::EaseInOutCubic,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct EasingUniforms {
    pub easing_type: u32, // 0=linear, 1=ease_in_quad, etc.
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
    pub time: f32,
    pub value: f32,
    pub _padding: [f32; 2], // Align to 16 bytes
}

impl Keyframe {
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
    /// Tension of 1.0 gives straight lines. Range: [0.0, 1.0]
    CatmullRom { tension: f32 },
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
    pub keyframes: Vec<Keyframe>,
    pub loop_animation: bool,
    pub reverse_on_loop: bool,
    pub interpolation_mode: InterpolationMode,
}

impl KeyframeAnimation {
    pub fn new() -> Self {
        Self {
            keyframes: Vec::new(),
            loop_animation: false,
            reverse_on_loop: false,
            interpolation_mode: InterpolationMode::default(),
        }
    }

    pub fn add_keyframe(mut self, time: f32, value: f32) -> Self {
        if self.keyframes.len() < MAX_KEYFRAMES {
            self.keyframes.push(Keyframe::new(time, value));
            // Keep keyframes sorted by time
            self.keyframes
                .sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());
        }
        self
    }

    pub fn with_loop(mut self, enable: bool) -> Self {
        self.loop_animation = enable;
        self
    }

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
}

impl Default for KeyframeAnimation {
    fn default() -> Self {
        Self::new()
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct KeyframeAnimationUniforms {
    pub keyframes: [Keyframe; MAX_KEYFRAMES],
    pub keyframe_count: u32,
    pub loop_animation: u32,
    pub reverse_on_loop: u32,
    pub interpolation_mode: u32, // 0=Linear, 1=CatmullRom, 2=BSpline
    pub tension: f32,            // For Catmull-Rom interpolation
    pub _padding: [f32; 3],      // Ensure 16-byte alignment
    pub _padding2: [f32; 4],     // Extra padding to match WGSL struct size (304 bytes)
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
    pub keyframes: Vec<Keyframe>,
    pub loop_animation: bool,
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
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
}

impl CubicBezierTiming {
    pub fn new(x1: f32, y1: f32, x2: f32, y2: f32) -> Self {
        Self { x1, y1, x2, y2 }
    }

    pub fn ease() -> Self {
        Self::new(0.25, 0.1, 0.25, 1.0)
    }

    pub fn ease_in() -> Self {
        Self::new(0.42, 0.0, 1.0, 1.0)
    }

    pub fn ease_out() -> Self {
        Self::new(0.0, 0.0, 0.58, 1.0)
    }

    pub fn ease_in_out() -> Self {
        Self::new(0.42, 0.0, 0.58, 1.0)
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CubicBezierTimingUniforms {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
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
    Playing,
    Paused,
    Stopped,
}

/// Animation timeline controller for complex animation sequences.
///
/// Provides playback control and time management for animations.
#[derive(Clone, Debug)]
pub struct AnimationTimeline {
    pub current_time: f32,
    pub playback_rate: f32,
    pub state: AnimationPlaybackState,
    pub loop_timeline: bool,
    pub duration: f32,
}

impl AnimationTimeline {
    pub fn new(duration: f32) -> Self {
        Self {
            current_time: 0.0,
            playback_rate: 1.0,
            state: AnimationPlaybackState::Stopped,
            loop_timeline: false,
            duration,
        }
    }

    pub fn play(&mut self) {
        self.state = AnimationPlaybackState::Playing;
    }

    pub fn pause(&mut self) {
        self.state = AnimationPlaybackState::Paused;
    }

    pub fn stop(&mut self) {
        self.state = AnimationPlaybackState::Stopped;
        self.current_time = 0.0;
    }

    pub fn seek(&mut self, time: f32) {
        self.current_time = time.clamp(0.0, self.duration);
    }

    pub fn set_playback_rate(&mut self, rate: f32) {
        self.playback_rate = rate;
    }

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
pub type AnimationEventCallback = Box<dyn FnMut(&AnimationTimeline, f32) + Send + Sync>;

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
// ============================================================================

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LinearScaleUniforms {
    pub domain_min: f32,
    pub domain_max: f32,
    pub range_min: f32,
    pub range_max: f32,
}

impl ShaderUniform for LinearScaleUniforms {
    fn wgsl_struct_definition() -> String {
        "struct LinearScaleUniforms {\n    domain_min: f32,\n    domain_max: f32,\n    range_min: f32,\n    range_max: f32,\n}".to_string()
    }

    fn wgsl_type_name() -> &'static str {
        "LinearScaleUniforms"
    }
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
#[derive(Clone, Debug)]
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
// Additional Scale Functions (AC2: Common Transformation Functions)
// ============================================================================

/// Logarithmic scale transformation.
///
/// Maps values from a domain to a range using logarithmic scaling.
/// Useful for data that spans multiple orders of magnitude.
#[derive(Clone, Debug)]
pub struct LogScale {
    pub domain_min: f32,
    pub domain_max: f32,
    pub range_min: f32,
    pub range_max: f32,
    pub base: f32,
}

impl LogScale {
    /// Creates a new logarithmic scale with base 10.
    pub fn new(domain_min: f32, domain_max: f32, range_min: f32, range_max: f32) -> Self {
        Self {
            domain_min,
            domain_max,
            range_min,
            range_max,
            base: 10.0,
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
        }
    }

    /// Creates a new logarithmic scale with custom base.
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
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LogScaleUniforms {
    pub domain_min: f32,
    pub domain_max: f32,
    pub range_min: f32,
    pub range_max: f32,
    pub base: f32,
    pub _padding: [f32; 3],
}

impl ShaderUniform for LogScaleUniforms {
    fn wgsl_struct_definition() -> String {
        "struct LogScaleUniforms {\n    domain_min: f32,\n    domain_max: f32,\n    range_min: f32,\n    range_max: f32,\n    base: f32,\n}".to_string()
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
            let log_min = log(scale.domain_min) / log(scale.base);
            let log_max = log(scale.domain_max) / log(scale.base);
            let log_value = log(max(value, 0.0001)) / log(scale.base);
            let normalized = (log_value - log_min) / (log_max - log_min);
            return scale.range_min + normalized * (scale.range_max - scale.range_min);
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
            _padding: [0.0; 3],
        })
    }

    fn function_name() -> &'static str {
        "log_scale"
    }
}

/// Power scale transformation (exponential scaling).
///
/// Maps values using a power function: output = (normalized_input)^exponent.
/// Exponent < 1 compresses high values, > 1 expands them.
#[derive(Clone, Debug)]
pub struct PowerScale {
    pub domain_min: f32,
    pub domain_max: f32,
    pub range_min: f32,
    pub range_max: f32,
    pub exponent: f32,
}

impl PowerScale {
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

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PowerScaleUniforms {
    pub domain_min: f32,
    pub domain_max: f32,
    pub range_min: f32,
    pub range_max: f32,
    pub exponent: f32,
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

// ============================================================================
// Filtering and Clamping Functions (AC2)
// ============================================================================

/// Clamps values to a specified range.
#[derive(Clone, Debug)]
pub struct Clamp {
    pub min: f32,
    pub max: f32,
}

impl Clamp {
    pub fn new(min: f32, max: f32) -> Self {
        Self { min, max }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ClampUniforms {
    pub min: f32,
    pub max: f32,
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
    pub threshold: f32,
}

impl Threshold {
    pub fn new(threshold: f32) -> Self {
        Self { threshold }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ThresholdUniforms {
    pub threshold: f32,
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
    pub edge0: f32,
    pub edge1: f32,
}

impl SmoothStep {
    pub fn new(edge0: f32, edge1: f32) -> Self {
        Self { edge0, edge1 }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SmoothStepUniforms {
    pub edge0: f32,
    pub edge1: f32,
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

/// Multi-point color interpolation (gradient with multiple stops).
#[derive(Clone, Debug)]
pub struct ColorGradient {
    pub colors: Vec<Vec4>,
    pub stops: Vec<f32>,
}

impl ColorGradient {
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
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ColorGradientUniforms {
    pub colors: [[f32; 4]; 8],
    pub stops: [f32; 8],
    pub count: u32,
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
    pub colors: Vec<Vec4>,
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
// Statistical Aggregation Functions (GUP-139)
// ============================================================================

/// GPU-accelerated statistical aggregation system for computing mean, median,
/// standard deviation, percentiles, and other statistical measures on large datasets.
///
/// This module provides compute shader-based parallel reduction algorithms for
/// efficient statistical computation on the GPU. These functions are designed for
/// data-driven statistical visualizations like box plots, violin plots, and
/// distribution analyses.
///
/// # Architecture
///
/// Statistical aggregations use a two-stage reduction approach:
/// 1. **Local Reduction**: Each workgroup computes partial results using shared memory
/// 2. **Global Reduction**: Partial results are combined to produce final statistics
///
/// This approach enables efficient processing of millions of data points with minimal
/// CPU-GPU round trips.
/// Result of statistical aggregation computation
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct StatisticsResult {
    /// Number of valid data points processed
    pub count: u32,
    /// Sum of all values
    pub sum: f32,
    /// Minimum value
    pub min: f32,
    /// Maximum value
    pub max: f32,
    /// Mean (average) value
    pub mean: f32,
    /// Variance
    pub variance: f32,
    /// Standard deviation
    pub std_dev: f32,
    /// Padding for 16-byte alignment
    pub _padding: u32,
}

/// GPU compute pipeline for statistical aggregations
pub struct StatisticsCompute {
    /// Compute pipeline for basic statistics (mean, min, max, std dev)
    basic_stats_pipeline: Option<wgpu::ComputePipeline>,
    /// Compute pipeline for variance (second pass)
    variance_pipeline: Option<wgpu::ComputePipeline>,
    /// Compute pipeline for median and percentiles
    #[allow(dead_code)]
    percentile_pipeline: Option<wgpu::ComputePipeline>,
    /// Input data buffer
    data_buffer: Option<wgpu::Buffer>,
    /// Output statistics buffer
    result_buffer: Option<wgpu::Buffer>,
    /// Maximum number of elements
    #[allow(dead_code)]
    max_elements: usize,
    /// Device and queue references
    device: Option<Arc<wgpu::Device>>,
    queue: Option<Arc<wgpu::Queue>>,
}

impl StatisticsCompute {
    /// Create a new statistics compute system
    pub async fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        max_elements: usize,
    ) -> GupResult<Self> {
        let basic_stats_pipeline = Self::create_basic_stats_pipeline(device).await?;
        let variance_pipeline = Self::create_variance_pipeline(device).await?;
        let percentile_pipeline = Self::create_percentile_pipeline(device).await?;

        let data_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("statistics_data"),
            size: (max_elements * std::mem::size_of::<f32>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let result_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("statistics_result"),
            size: std::mem::size_of::<StatisticsResult>() as u64,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Ok(Self {
            basic_stats_pipeline: Some(basic_stats_pipeline),
            variance_pipeline: Some(variance_pipeline),
            percentile_pipeline: Some(percentile_pipeline),
            data_buffer: Some(data_buffer),
            result_buffer: Some(result_buffer),
            max_elements,
            device: Some(Arc::new(device.clone())),
            queue: Some(Arc::new(queue.clone())),
        })
    }

    /// Create compute pipeline for basic statistics
    async fn create_basic_stats_pipeline(
        device: &wgpu::Device,
    ) -> GupResult<wgpu::ComputePipeline> {
        let shader_source = include_str!("shaders/statistics.compute.wgsl");

        let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("statistics_compute"),
            source: wgpu::ShaderSource::Wgsl(shader_source.into()),
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("basic_stats_pipeline"),
            layout: None,
            module: &shader_module,
            entry_point: Some("compute_basic_stats"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        Ok(pipeline)
    }

    /// Create compute pipeline for variance calculation
    async fn create_variance_pipeline(device: &wgpu::Device) -> GupResult<wgpu::ComputePipeline> {
        let shader_source = include_str!("shaders/statistics.compute.wgsl");

        let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("statistics_compute"),
            source: wgpu::ShaderSource::Wgsl(shader_source.into()),
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("variance_pipeline"),
            layout: None,
            module: &shader_module,
            entry_point: Some("compute_variance"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        Ok(pipeline)
    }

    /// Create compute pipeline for percentile calculation
    async fn create_percentile_pipeline(device: &wgpu::Device) -> GupResult<wgpu::ComputePipeline> {
        let shader_source = include_str!("shaders/percentile.compute.wgsl");

        let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("percentile_compute"),
            source: wgpu::ShaderSource::Wgsl(shader_source.into()),
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("percentile_pipeline"),
            layout: None,
            module: &shader_module,
            entry_point: Some("compute_percentile"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        Ok(pipeline)
    }

    /// Compute basic statistics (mean, min, max, std dev) for a dataset
    pub async fn compute_basic_stats(&self, data: &[f32]) -> GupResult<StatisticsResult> {
        if data.is_empty() {
            return Ok(StatisticsResult {
                count: 0,
                sum: 0.0,
                min: 0.0,
                max: 0.0,
                mean: 0.0,
                variance: 0.0,
                std_dev: 0.0,
                _padding: 0,
            });
        }

        let device = self.device.as_ref().ok_or_else(|| {
            GupError::gpu_initialization_failed(
                "StatisticsCompute device not initialized".to_string(),
            )
        })?;
        let queue = self.queue.as_ref().ok_or_else(|| {
            GupError::gpu_initialization_failed(
                "StatisticsCompute queue not initialized".to_string(),
            )
        })?;
        let data_buffer = self.data_buffer.as_ref().ok_or_else(|| {
            GupError::gpu_initialization_failed(
                "StatisticsCompute data_buffer not initialized".to_string(),
            )
        })?;
        let result_buffer = self.result_buffer.as_ref().ok_or_else(|| {
            GupError::gpu_initialization_failed(
                "StatisticsCompute result_buffer not initialized".to_string(),
            )
        })?;

        // Upload data to GPU
        queue.write_buffer(data_buffer, 0, bytemuck::cast_slice(data));

        // Initialize result buffer with actual data count
        // IMPORTANT: Set count to actual data size so shader can use it via result.count
        let init_result = StatisticsResult {
            count: data.len() as u32,
            sum: 0.0,
            min: 0.0,
            max: 0.0,
            mean: 0.0,
            variance: 0.0,
            std_dev: 0.0,
            _padding: 0,
        };
        queue.write_buffer(result_buffer, 0, bytemuck::bytes_of(&init_result));

        // Create bind group
        let pipeline = self.basic_stats_pipeline.as_ref().ok_or_else(|| {
            GupError::gpu_initialization_failed(
                "StatisticsCompute pipeline not initialized".to_string(),
            )
        })?;
        let bind_group_layout = pipeline.get_bind_group_layout(0);
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("statistics_bind_group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: data_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: result_buffer.as_entire_binding(),
                },
            ],
        });

        // Execute compute pass for basic stats
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("statistics_encoder"),
        });

        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("statistics_pass"),
                timestamp_writes: None,
            });
            compute_pass.set_pipeline(pipeline);
            compute_pass.set_bind_group(0, &bind_group, &[]);
            // Dispatch with workgroups covering all data
            let workgroup_size = 256;
            let num_workgroups = data.len().div_ceil(workgroup_size) as u32;
            compute_pass.dispatch_workgroups(num_workgroups, 1, 1);
        }

        queue.submit(Some(encoder.finish()));
        let _ = device.poll(wgpu::PollType::Wait);

        // Second pass: compute variance (requires mean from first pass)
        let variance_pipeline = self.variance_pipeline.as_ref().ok_or_else(|| {
            GupError::gpu_initialization_failed(
                "StatisticsCompute variance pipeline not initialized".to_string(),
            )
        })?;

        // Create bind group for variance pipeline
        let variance_bind_group_layout = variance_pipeline.get_bind_group_layout(0);
        let variance_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("variance_bind_group"),
            layout: &variance_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: data_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: result_buffer.as_entire_binding(),
                },
            ],
        });

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("variance_encoder"),
        });

        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("variance_pass"),
                timestamp_writes: None,
            });
            compute_pass.set_pipeline(variance_pipeline);
            compute_pass.set_bind_group(0, &variance_bind_group, &[]);
            let workgroup_size = 256;
            let num_workgroups = data.len().div_ceil(workgroup_size) as u32;
            compute_pass.dispatch_workgroups(num_workgroups, 1, 1);
        }

        queue.submit(Some(encoder.finish()));

        // Read results back
        let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("statistics_staging"),
            size: std::mem::size_of::<StatisticsResult>() as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("statistics_copy_encoder"),
        });
        encoder.copy_buffer_to_buffer(
            result_buffer,
            0,
            &staging_buffer,
            0,
            std::mem::size_of::<StatisticsResult>() as u64,
        );
        queue.submit(Some(encoder.finish()));

        // Wait for GPU to complete
        let _ = device.poll(wgpu::PollType::Wait);

        // Map and read results
        let buffer_slice = staging_buffer.slice(..);
        let (tx, rx) = futures_channel::oneshot::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
        let _ = device.poll(wgpu::PollType::Wait);
        rx.await
            .map_err(|_| {
                GupError::gpu_initialization_failed(
                    "Failed to receive buffer map result".to_string(),
                )
            })?
            .map_err(|e| {
                GupError::gpu_initialization_failed(format!("Buffer mapping failed: {:?}", e))
            })?;

        let data = buffer_slice.get_mapped_range();

        let result: StatisticsResult =
            *bytemuck::from_bytes(&data[..std::mem::size_of::<StatisticsResult>()]);
        drop(data);
        staging_buffer.unmap();

        Ok(result)
    }
}

/// Configuration for histogram computation on GPU
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct HistogramConfig {
    /// Number of bins
    pub bin_count: u32,
    /// Minimum value for binning range
    pub min_value: f32,
    /// Maximum value for binning range
    pub max_value: f32,
    /// 0 = counts, 1 = probabilities
    pub normalize: u32,
    /// Actual number of data elements (not buffer size)
    pub data_length: u32,
    /// Padding for 16-byte alignment (uniform buffer requirement)
    _padding: u32,
    _padding2: u32,
    _padding3: u32,
}

/// GPU compute pipeline for histogram generation
pub struct HistogramCompute {
    /// Compute pipeline for histogram binning
    histogram_pipeline: Option<wgpu::ComputePipeline>,
    /// Input data buffer
    data_buffer: Option<wgpu::Buffer>,
    /// Output bins buffer (atomic u32 array)
    bins_buffer: Option<wgpu::Buffer>,
    /// Configuration uniform buffer
    config_buffer: Option<wgpu::Buffer>,
    /// Maximum number of elements
    #[allow(dead_code)]
    max_elements: usize,
    /// Maximum number of bins
    max_bins: usize,
    /// Device and queue references
    device: Option<Arc<wgpu::Device>>,
    queue: Option<Arc<wgpu::Queue>>,
}

impl HistogramCompute {
    /// Create a new histogram compute system
    pub async fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        max_elements: usize,
        max_bins: usize,
    ) -> GupResult<Self> {
        let histogram_pipeline = Self::create_histogram_pipeline(device).await?;

        let data_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("histogram_data"),
            size: (max_elements * std::mem::size_of::<f32>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bins_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("histogram_bins"),
            size: (max_bins * std::mem::size_of::<u32>()) as u64,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let config_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("histogram_config"),
            size: std::mem::size_of::<HistogramConfig>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Ok(Self {
            histogram_pipeline: Some(histogram_pipeline),
            data_buffer: Some(data_buffer),
            bins_buffer: Some(bins_buffer),
            config_buffer: Some(config_buffer),
            max_elements,
            max_bins,
            device: Some(Arc::new(device.clone())),
            queue: Some(Arc::new(queue.clone())),
        })
    }

    /// Create compute pipeline for histogram generation
    async fn create_histogram_pipeline(device: &wgpu::Device) -> GupResult<wgpu::ComputePipeline> {
        let shader_source = include_str!("shaders/histogram.compute.wgsl");

        let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("histogram_compute"),
            source: wgpu::ShaderSource::Wgsl(shader_source.into()),
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("histogram_pipeline"),
            layout: None,
            module: &shader_module,
            entry_point: Some("compute_histogram"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        Ok(pipeline)
    }

    /// Compute histogram for a dataset
    pub async fn compute_histogram(
        &self,
        data: &[f32],
        bin_count: usize,
        min_value: f32,
        max_value: f32,
        normalize: bool,
    ) -> GupResult<HistogramResult> {
        if data.is_empty() {
            return Ok(HistogramResult {
                bins: vec![0; bin_count],
                edges: vec![0.0; bin_count + 1],
                min: min_value,
                max: max_value,
                count: 0,
            });
        }

        if bin_count > self.max_bins {
            return Err(GupError::buffer_error(format!(
                "Requested {} bins exceeds maximum of {}",
                bin_count, self.max_bins
            )));
        }

        let device = self.device.as_ref().ok_or_else(|| {
            GupError::gpu_initialization_failed(
                "HistogramCompute device not initialized".to_string(),
            )
        })?;
        let queue = self.queue.as_ref().ok_or_else(|| {
            GupError::gpu_initialization_failed(
                "HistogramCompute queue not initialized".to_string(),
            )
        })?;
        let data_buffer = self.data_buffer.as_ref().ok_or_else(|| {
            GupError::gpu_initialization_failed(
                "HistogramCompute data_buffer not initialized".to_string(),
            )
        })?;
        let bins_buffer = self.bins_buffer.as_ref().ok_or_else(|| {
            GupError::gpu_initialization_failed(
                "HistogramCompute bins_buffer not initialized".to_string(),
            )
        })?;
        let config_buffer = self.config_buffer.as_ref().ok_or_else(|| {
            GupError::gpu_initialization_failed(
                "HistogramCompute config_buffer not initialized".to_string(),
            )
        })?;

        // Upload data to GPU
        queue.write_buffer(data_buffer, 0, bytemuck::cast_slice(data));

        // Clear bins buffer
        let zero_bins = vec![0u32; bin_count];
        queue.write_buffer(bins_buffer, 0, bytemuck::cast_slice(&zero_bins));

        // Upload configuration
        let config = HistogramConfig {
            bin_count: bin_count as u32,
            min_value,
            max_value,
            normalize: if normalize { 1 } else { 0 },
            data_length: data.len() as u32,
            _padding: 0,
            _padding2: 0,
            _padding3: 0,
        };
        queue.write_buffer(config_buffer, 0, bytemuck::bytes_of(&config));

        // Create bind group
        let pipeline = self.histogram_pipeline.as_ref().ok_or_else(|| {
            GupError::gpu_initialization_failed(
                "HistogramCompute pipeline not initialized".to_string(),
            )
        })?;
        let bind_group_layout = pipeline.get_bind_group_layout(0);
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("histogram_bind_group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: data_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: bins_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: config_buffer.as_entire_binding(),
                },
            ],
        });

        // Execute compute shader
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("histogram_encoder"),
        });

        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("histogram_pass"),
                timestamp_writes: None,
            });
            compute_pass.set_pipeline(pipeline);
            compute_pass.set_bind_group(0, &bind_group, &[]);

            let workgroup_count = (data.len() as u32).div_ceil(256);
            compute_pass.dispatch_workgroups(workgroup_count, 1, 1);
        }

        // Read back results
        let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("histogram_staging"),
            size: (bin_count * std::mem::size_of::<u32>()) as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        encoder.copy_buffer_to_buffer(
            bins_buffer,
            0,
            &staging_buffer,
            0,
            (bin_count * std::mem::size_of::<u32>()) as u64,
        );

        queue.submit(Some(encoder.finish()));

        let buffer_slice = staging_buffer.slice(..);
        let (sender, receiver) = futures::channel::oneshot::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            sender.send(result).ok();
        });

        let _ = device.poll(wgpu::PollType::Wait);
        receiver
            .await
            .map_err(|_| GupError::webgpu_error("Failed to receive buffer mapping".to_string()))?
            .map_err(|e| GupError::webgpu_error(format!("Failed to map buffer: {:?}", e)))?;

        let buffer_data = buffer_slice.get_mapped_range();
        let bins: Vec<u32> = bytemuck::cast_slice(&buffer_data).to_vec();
        drop(buffer_data);
        staging_buffer.unmap();

        // Compute bin edges
        let range = max_value - min_value;
        let step = range / bin_count as f32;
        let edges: Vec<f32> = (0..=bin_count)
            .map(|i| min_value + i as f32 * step)
            .collect();

        Ok(HistogramResult {
            bins,
            edges,
            min: min_value,
            max: max_value,
            count: data.len(),
        })
    }
}

/// Mean calculation shader function - computes average of dataset
#[derive(Clone, Debug)]
pub struct Mean {
    /// Data values to compute mean over
    pub values: Vec<f32>,
}

impl Mean {
    pub fn new(values: Vec<f32>) -> Self {
        Self { values }
    }

    /// Compute mean on CPU (for small datasets or fallback)
    pub fn compute_cpu(&self) -> f32 {
        if self.values.is_empty() {
            return 0.0;
        }
        let sum: f32 = self.values.iter().sum();
        sum / self.values.len() as f32
    }
}

/// Standard deviation shader function
#[derive(Clone, Debug)]
pub struct StandardDeviation {
    /// Data values to compute std dev over
    pub values: Vec<f32>,
}

impl StandardDeviation {
    pub fn new(values: Vec<f32>) -> Self {
        Self { values }
    }

    /// Compute standard deviation on CPU (for small datasets or fallback)
    pub fn compute_cpu(&self) -> f32 {
        if self.values.len() < 2 {
            return 0.0;
        }
        let mean = Mean::new(self.values.clone()).compute_cpu();
        let variance: f32 = self
            .values
            .iter()
            .map(|v| {
                let diff = v - mean;
                diff * diff
            })
            .sum::<f32>()
            / self.values.len() as f32;
        variance.sqrt()
    }
}

/// Min/Max aggregation shader function
#[derive(Clone, Debug)]
pub struct MinMax {
    /// Data values to find min/max over
    pub values: Vec<f32>,
}

impl MinMax {
    pub fn new(values: Vec<f32>) -> Self {
        Self { values }
    }

    /// Compute min and max on CPU (for small datasets or fallback)
    pub fn compute_cpu(&self) -> (f32, f32) {
        if self.values.is_empty() {
            return (0.0, 0.0);
        }
        let min = self.values.iter().fold(f32::INFINITY, |a, &b| a.min(b));
        let max = self.values.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));
        (min, max)
    }
}

/// Percentile calculation shader function
#[derive(Clone, Debug)]
pub struct Percentile {
    /// Data values to compute percentile over
    pub values: Vec<f32>,
    /// Percentile to compute (0.0 to 1.0)
    pub percentile: f32,
}

impl Percentile {
    pub fn new(values: Vec<f32>, percentile: f32) -> Self {
        Self { values, percentile }
    }

    /// Compute percentile on CPU (for small datasets or fallback)
    pub fn compute_cpu(&self) -> f32 {
        if self.values.is_empty() {
            return 0.0;
        }
        let mut sorted = self.values.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let index = (self.percentile * (sorted.len() - 1) as f32) as usize;
        sorted[index]
    }
}

/// Binning strategy for histogram generation
#[derive(Clone, Debug, PartialEq)]
pub enum BinningStrategy {
    /// Equal-width bins across the data range
    EqualWidth,
    /// Equal-frequency bins (each bin has approximately same count)
    EqualFrequency,
}

/// Histogram generation shader function
#[derive(Clone, Debug)]
pub struct Histogram {
    /// Data values to compute histogram over
    pub values: Vec<f32>,
    /// Number of bins
    pub bin_count: usize,
    /// Custom bin edges (if None, will auto-detect from min/max)
    pub custom_edges: Option<Vec<f32>>,
    /// Binning strategy
    pub strategy: BinningStrategy,
    /// Whether to normalize to probabilities
    pub normalize: bool,
}

impl Histogram {
    /// Create a new histogram with equal-width bins
    pub fn new(values: Vec<f32>, bin_count: usize) -> Self {
        Self {
            values,
            bin_count,
            custom_edges: None,
            strategy: BinningStrategy::EqualWidth,
            normalize: false,
        }
    }

    /// Set custom bin edges
    pub fn with_edges(mut self, edges: Vec<f32>) -> Self {
        self.custom_edges = Some(edges);
        self
    }

    /// Set binning strategy
    pub fn with_strategy(mut self, strategy: BinningStrategy) -> Self {
        self.strategy = strategy;
        self
    }

    /// Enable probability normalization
    pub fn with_normalization(mut self, normalize: bool) -> Self {
        self.normalize = normalize;
        self
    }

    /// Compute histogram on CPU (for small datasets or fallback)
    pub fn compute_cpu(&self) -> HistogramResult {
        if self.values.is_empty() {
            return HistogramResult {
                bins: vec![0; self.bin_count],
                edges: vec![0.0; self.bin_count + 1],
                min: 0.0,
                max: 0.0,
                count: 0,
            };
        }

        // Determine bin edges
        let (min, max) = if let Some(ref edges) = self.custom_edges {
            (*edges.first().unwrap(), *edges.last().unwrap())
        } else {
            let min = self.values.iter().fold(f32::INFINITY, |a, &b| a.min(b));
            let max = self.values.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));
            (min, max)
        };

        let edges = self.compute_bin_edges(min, max);
        let mut bins = vec![0u32; self.bin_count];

        // Bin the data
        for &value in &self.values {
            if value < min || value > max {
                continue;
            }
            let range = max - min;
            if range == 0.0 {
                bins[0] += 1;
            } else {
                let normalized = (value - min) / range;
                let bin_index = (normalized * self.bin_count as f32) as usize;
                let bin_index = bin_index.min(self.bin_count - 1);
                bins[bin_index] += 1;
            }
        }

        // Normalize if requested
        if self.normalize {
            let total: u32 = bins.iter().sum();
            if total > 0 {
                // Convert to f32 for normalization, then back to u32 (storing as bits)
                bins = bins
                    .iter()
                    .map(|&count| {
                        let prob = count as f32 / total as f32;
                        prob.to_bits()
                    })
                    .collect();
            }
        }

        HistogramResult {
            bins,
            edges,
            min,
            max,
            count: self.values.len(),
        }
    }

    /// Compute bin edges based on strategy
    fn compute_bin_edges(&self, min: f32, max: f32) -> Vec<f32> {
        if let Some(ref edges) = self.custom_edges {
            return edges.clone();
        }

        match self.strategy {
            BinningStrategy::EqualWidth => {
                let range = max - min;
                let step = range / self.bin_count as f32;
                (0..=self.bin_count)
                    .map(|i| min + i as f32 * step)
                    .collect()
            }
            BinningStrategy::EqualFrequency => {
                // For equal frequency, we need to sort and find quantiles
                let mut sorted = self.values.clone();
                sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

                let mut edges = Vec::with_capacity(self.bin_count + 1);
                edges.push(min);

                for i in 1..self.bin_count {
                    let quantile = i as f32 / self.bin_count as f32;
                    let index = (quantile * (sorted.len() - 1) as f32) as usize;
                    edges.push(sorted[index]);
                }

                edges.push(max);
                edges
            }
        }
    }
}

/// Result of histogram computation
#[derive(Clone, Debug)]
pub struct HistogramResult {
    /// Bin counts (or normalized probabilities if normalize=true)
    pub bins: Vec<u32>,
    /// Bin edges (length = bin_count + 1)
    pub edges: Vec<f32>,
    /// Minimum value in dataset
    pub min: f32,
    /// Maximum value in dataset
    pub max: f32,
    /// Total count of values
    pub count: usize,
}

impl HistogramResult {
    /// Get bin counts as f32 (handles normalized histograms)
    pub fn bin_values(&self) -> Vec<f32> {
        self.bins.iter().map(|&bits| f32::from_bits(bits)).collect()
    }

    /// Check if this is a normalized histogram
    pub fn is_normalized(&self) -> bool {
        // If any bin value is less than 1.0, it's likely normalized
        self.bins
            .iter()
            .any(|&bits| f32::from_bits(bits) < 1.0 && bits != 0)
    }
}

/// Streaming statistical aggregation for datasets larger than GPU memory
///
/// Uses Welford's online algorithm for numerically stable variance computation
/// and processes data in configurable chunks to handle arbitrarily large datasets.
///
/// # Examples
///
/// ```rust,ignore
/// use gup::StreamingStatistics;
///
/// // Process 1 billion points in chunks
/// let mut stats = StreamingStatistics::with_chunk_size(1_000_000);
///
/// for chunk in data_source.chunks() {
///     stats.push_chunk(&chunk);
/// }
///
/// let result = stats.finalize();
/// println!("Mean: {}, Std Dev: {}", result.mean, result.std_dev);
/// ```
#[derive(Clone, Debug)]
pub struct StreamingStatistics {
    /// Running count of elements processed
    count: u64,
    /// Running mean (Welford's algorithm)
    mean: f64,
    /// Running M2 value for variance computation (Welford's algorithm)
    m2: f64,
    /// Running minimum value
    min: f32,
    /// Running maximum value
    max: f32,
    /// Running sum (for verification)
    sum: f64,
    /// Chunk size for processing (default: 1M elements)
    chunk_size: usize,
    /// Total chunks processed
    chunks_processed: usize,
}

impl Default for StreamingStatistics {
    fn default() -> Self {
        Self::new()
    }
}

/// Callback type for progress reporting in streaming statistics: (processed, total).
type ProgressCallback = Box<dyn Fn(usize, Option<usize>)>;

impl StreamingStatistics {
    /// Create a new streaming statistics aggregator with default chunk size (1M elements)
    pub fn new() -> Self {
        Self::with_chunk_size(1_000_000)
    }

    /// Create a new streaming statistics aggregator with custom chunk size
    ///
    /// # Arguments
    /// * `chunk_size` - Number of elements to process per chunk (affects GPU buffer size)
    pub fn with_chunk_size(chunk_size: usize) -> Self {
        Self {
            count: 0,
            mean: 0.0,
            m2: 0.0,
            min: f32::INFINITY,
            max: f32::NEG_INFINITY,
            sum: 0.0,
            chunk_size,
            chunks_processed: 0,
        }
    }

    /// Push a single value into the stream (uses Welford's algorithm)
    ///
    /// # Arguments
    /// * `value` - Single f32 value to aggregate
    pub fn push(&mut self, value: f32) {
        self.count += 1;
        let value_f64 = value as f64;

        // Update sum
        self.sum += value_f64;

        // Update min/max
        self.min = self.min.min(value);
        self.max = self.max.max(value);

        // Welford's online algorithm for mean and variance
        let delta = value_f64 - self.mean;
        self.mean += delta / self.count as f64;
        let delta2 = value_f64 - self.mean;
        self.m2 += delta * delta2;
    }

    /// Push a chunk of values into the stream
    ///
    /// # Arguments
    /// * `chunk` - Slice of f32 values to aggregate
    pub fn push_chunk(&mut self, chunk: &[f32]) {
        for &value in chunk {
            self.push(value);
        }
        self.chunks_processed += 1;
    }

    /// Process data from an iterator in chunks
    ///
    /// This is the recommended way to process large datasets as it handles
    /// chunking automatically and provides progress reporting.
    ///
    /// # Arguments
    /// * `data` - Iterator providing f32 values
    /// * `progress_callback` - Optional callback for progress reporting (processed, total)
    pub fn process_iter<I>(&mut self, data: I, progress_callback: Option<ProgressCallback>)
    where
        I: Iterator<Item = f32>,
    {
        let mut chunk = Vec::with_capacity(self.chunk_size);
        let mut total_processed = 0;

        for value in data {
            chunk.push(value);
            if chunk.len() >= self.chunk_size {
                self.push_chunk(&chunk);
                total_processed += chunk.len();
                chunk.clear();

                if let Some(ref callback) = progress_callback {
                    callback(total_processed, None);
                }
            }
        }

        // Process remaining values
        if !chunk.is_empty() {
            self.push_chunk(&chunk);
            total_processed += chunk.len();

            if let Some(ref callback) = progress_callback {
                callback(total_processed, None);
            }
        }
    }

    /// Process data from a slice in chunks with progress reporting
    ///
    /// # Arguments
    /// * `data` - Slice of f32 values to process
    /// * `progress_callback` - Optional callback for progress reporting (processed, total)
    pub fn process_slice(
        &mut self,
        data: &[f32],
        progress_callback: Option<Box<dyn Fn(usize, usize)>>,
    ) {
        let total = data.len();
        let mut processed = 0;

        for chunk in data.chunks(self.chunk_size) {
            self.push_chunk(chunk);
            processed += chunk.len();

            if let Some(ref callback) = progress_callback {
                callback(processed, total);
            }
        }
    }

    /// Merge statistics from another streaming aggregator
    ///
    /// This enables parallel processing where multiple StreamingStatistics
    /// instances process different parts of the dataset and then merge.
    ///
    /// # Arguments
    /// * `other` - Another StreamingStatistics instance to merge
    pub fn merge(&mut self, other: &StreamingStatistics) {
        if other.count == 0 {
            return;
        }

        if self.count == 0 {
            *self = other.clone();
            return;
        }

        // Merge using parallel algorithm
        let total_count = self.count + other.count;
        let delta = other.mean - self.mean;

        // Update mean
        let new_mean =
            (self.count as f64 * self.mean + other.count as f64 * other.mean) / total_count as f64;

        // Update M2 (variance component)
        let new_m2 = self.m2
            + other.m2
            + delta * delta * (self.count as f64 * other.count as f64) / total_count as f64;

        self.mean = new_mean;
        self.m2 = new_m2;
        self.count = total_count;
        self.sum += other.sum;
        self.min = self.min.min(other.min);
        self.max = self.max.max(other.max);
        self.chunks_processed += other.chunks_processed;
    }

    /// Get current count of processed elements
    pub fn count(&self) -> u64 {
        self.count
    }

    /// Get current mean (available before finalization)
    pub fn mean(&self) -> f32 {
        self.mean as f32
    }

    /// Get current variance (available before finalization)
    pub fn variance(&self) -> f32 {
        if self.count < 2 {
            0.0
        } else {
            (self.m2 / self.count as f64) as f32
        }
    }

    /// Get current standard deviation (available before finalization)
    pub fn std_dev(&self) -> f32 {
        self.variance().sqrt()
    }

    /// Get current min/max (available before finalization)
    pub fn min_max(&self) -> (f32, f32) {
        (self.min, self.max)
    }

    /// Finalize and get complete statistics result
    ///
    /// Returns a `StatisticsResult` compatible with GPU compute results.
    pub fn finalize(&self) -> StatisticsResult {
        let variance = self.variance();
        let std_dev = variance.sqrt();

        StatisticsResult {
            count: self.count as u32,
            sum: self.sum as f32,
            min: self.min,
            max: self.max,
            mean: self.mean as f32,
            variance,
            std_dev,
            _padding: 0,
        }
    }

    /// Reset the aggregator to initial state
    pub fn reset(&mut self) {
        self.count = 0;
        self.mean = 0.0;
        self.m2 = 0.0;
        self.min = f32::INFINITY;
        self.max = f32::NEG_INFINITY;
        self.sum = 0.0;
        self.chunks_processed = 0;
    }

    /// Get number of chunks processed
    pub fn chunks_processed(&self) -> usize {
        self.chunks_processed
    }

    /// Get configured chunk size
    pub fn chunk_size(&self) -> usize {
        self.chunk_size
    }
}

/// Kernel function for density estimation
#[derive(Clone, Debug, Copy, PartialEq)]
pub enum KernelFunction {
    /// Gaussian kernel (most common) - K(u) = (1/√(2π)) * exp(-u²/2)
    Gaussian,
    /// Epanechnikov kernel (optimal for MSE) - K(u) = (3/4) * (1 - u²) for |u| ≤ 1
    Epanechnikov,
    /// Uniform kernel (rectangular) - K(u) = 1/2 for |u| ≤ 1
    Uniform,
    /// Triangular kernel - K(u) = (1 - |u|) for |u| ≤ 1
    Triangular,
}

impl KernelFunction {
    /// Evaluate the kernel function at point u
    pub fn evaluate(&self, u: f32) -> f32 {
        match self {
            KernelFunction::Gaussian => {
                let factor = 1.0 / (2.0 * std::f32::consts::PI).sqrt();
                factor * (-0.5 * u * u).exp()
            }
            KernelFunction::Epanechnikov => {
                if u.abs() <= 1.0 {
                    0.75 * (1.0 - u * u)
                } else {
                    0.0
                }
            }
            KernelFunction::Uniform => {
                if u.abs() <= 1.0 {
                    0.5
                } else {
                    0.0
                }
            }
            KernelFunction::Triangular => {
                let abs_u = u.abs();
                if abs_u <= 1.0 { 1.0 - abs_u } else { 0.0 }
            }
        }
    }

    /// Get the WGSL function code for this kernel
    #[allow(dead_code)]
    fn wgsl_code(&self) -> &'static str {
        match self {
            KernelFunction::Gaussian => {
                r#"
fn gaussian_kernel(u: f32) -> f32 {
    let factor = 1.0 / sqrt(2.0 * 3.14159265359);
    return factor * exp(-0.5 * u * u);
}
"#
            }
            KernelFunction::Epanechnikov => {
                r#"
fn epanechnikov_kernel(u: f32) -> f32 {
    let abs_u = abs(u);
    if (abs_u <= 1.0) {
        return 0.75 * (1.0 - u * u);
    } else {
        return 0.0;
    }
}
"#
            }
            KernelFunction::Uniform => {
                r#"
fn uniform_kernel(u: f32) -> f32 {
    if (abs(u) <= 1.0) {
        return 0.5;
    } else {
        return 0.0;
    }
}
"#
            }
            KernelFunction::Triangular => {
                r#"
fn triangular_kernel(u: f32) -> f32 {
    let abs_u = abs(u);
    if (abs_u <= 1.0) {
        return 1.0 - abs_u;
    } else {
        return 0.0;
    }
}
"#
            }
        }
    }

    /// Get the WGSL function name for this kernel
    #[allow(dead_code)]
    fn wgsl_function_name(&self) -> &'static str {
        match self {
            KernelFunction::Gaussian => "gaussian_kernel",
            KernelFunction::Epanechnikov => "epanechnikov_kernel",
            KernelFunction::Uniform => "uniform_kernel",
            KernelFunction::Triangular => "triangular_kernel",
        }
    }
}

/// Bandwidth estimation method for kernel density estimation
#[derive(Clone, Debug, Copy, PartialEq)]
pub enum BandwidthMethod {
    /// Manual bandwidth specification
    Manual(f32),
    /// Silverman's rule of thumb - bandwidth = 0.9 * min(std, IQR/1.34) * n^(-1/5)
    Silverman,
    /// Scott's rule - bandwidth = std * n^(-1/5)
    Scott,
}

/// 1D Kernel Density Estimation
#[derive(Clone, Debug)]
pub struct KernelDensity1D {
    /// Sample data points
    pub samples: Vec<f32>,
    /// Kernel function to use
    pub kernel: KernelFunction,
    /// Bandwidth (smoothing parameter)
    pub bandwidth: BandwidthMethod,
    /// Evaluation points (if None, will auto-generate)
    pub eval_points: Option<Vec<f32>>,
    /// Number of evaluation points for auto-generation
    pub n_eval_points: usize,
}

impl KernelDensity1D {
    /// Create a new 1D KDE with default settings (Gaussian kernel, Silverman bandwidth, 1000 eval points)
    pub fn new(samples: Vec<f32>) -> Self {
        Self {
            samples,
            kernel: KernelFunction::Gaussian,
            bandwidth: BandwidthMethod::Silverman,
            eval_points: None,
            n_eval_points: 1000,
        }
    }

    /// Set the kernel function
    pub fn with_kernel(mut self, kernel: KernelFunction) -> Self {
        self.kernel = kernel;
        self
    }

    /// Set manual bandwidth
    pub fn with_bandwidth(mut self, bandwidth: f32) -> Self {
        self.bandwidth = BandwidthMethod::Manual(bandwidth);
        self
    }

    /// Set bandwidth method
    pub fn with_bandwidth_method(mut self, method: BandwidthMethod) -> Self {
        self.bandwidth = method;
        self
    }

    /// Set custom evaluation points
    pub fn with_eval_points(mut self, points: Vec<f32>) -> Self {
        self.eval_points = Some(points);
        self
    }

    /// Set number of evaluation points for auto-generation
    pub fn with_n_eval_points(mut self, n: usize) -> Self {
        self.n_eval_points = n;
        self
    }

    /// Estimate optimal bandwidth using the specified method
    fn estimate_bandwidth(&self) -> f32 {
        match self.bandwidth {
            BandwidthMethod::Manual(bw) => bw,
            BandwidthMethod::Silverman => {
                // Silverman's rule: 0.9 * min(std, IQR/1.34) * n^(-1/5)
                let n = self.samples.len() as f32;
                let std_dev = StandardDeviation::new(self.samples.clone()).compute_cpu();

                // Compute IQR (interquartile range)
                let q1 = Percentile::new(self.samples.clone(), 0.25).compute_cpu();
                let q3 = Percentile::new(self.samples.clone(), 0.75).compute_cpu();
                let iqr = q3 - q1;

                let scale = std_dev.min(iqr / 1.34);
                0.9 * scale * n.powf(-0.2)
            }
            BandwidthMethod::Scott => {
                // Scott's rule: std * n^(-1/5)
                let n = self.samples.len() as f32;
                let std_dev = StandardDeviation::new(self.samples.clone()).compute_cpu();
                std_dev * n.powf(-0.2)
            }
        }
    }

    /// Generate evaluation points across the data range
    fn generate_eval_points(&self) -> Vec<f32> {
        if let Some(ref points) = self.eval_points {
            return points.clone();
        }

        let (min, max) = MinMax::new(self.samples.clone()).compute_cpu();
        let bandwidth = self.estimate_bandwidth();

        // Extend range slightly beyond data bounds
        let padding = bandwidth * 3.0;
        let start = min - padding;
        let end = max + padding;
        let step = (end - start) / (self.n_eval_points - 1) as f32;

        (0..self.n_eval_points)
            .map(|i| start + i as f32 * step)
            .collect()
    }

    /// Compute KDE on CPU (for small datasets or fallback)
    pub fn compute_cpu(&self) -> KDEResult {
        if self.samples.is_empty() {
            return KDEResult {
                densities: vec![],
                eval_points: vec![],
                bandwidth: 0.0,
                kernel: self.kernel,
            };
        }

        let bandwidth = self.estimate_bandwidth();
        let eval_points = self.generate_eval_points();
        let n = self.samples.len() as f32;

        // Compute density at each evaluation point
        let densities: Vec<f32> = eval_points
            .iter()
            .map(|&x| {
                // Sum kernel contributions from all samples
                let sum: f32 = self
                    .samples
                    .iter()
                    .map(|&xi| {
                        let u = (x - xi) / bandwidth;
                        self.kernel.evaluate(u)
                    })
                    .sum();

                // Normalize by sample count and bandwidth
                sum / (n * bandwidth)
            })
            .collect();

        KDEResult {
            densities,
            eval_points,
            bandwidth,
            kernel: self.kernel,
        }
    }
}

/// 2D Kernel Density Estimation
#[derive(Clone, Debug)]
pub struct KernelDensity2D {
    /// Sample data points (x, y)
    pub samples: Vec<(f32, f32)>,
    /// Kernel function to use
    pub kernel: KernelFunction,
    /// Bandwidth for x dimension
    pub bandwidth_x: BandwidthMethod,
    /// Bandwidth for y dimension
    pub bandwidth_y: BandwidthMethod,
    /// Evaluation grid points (if None, will auto-generate)
    pub eval_grid: Option<(Vec<f32>, Vec<f32>)>,
    /// Number of evaluation points per dimension for auto-generation
    pub n_eval_points: usize,
}

impl KernelDensity2D {
    /// Create a new 2D KDE with default settings
    pub fn new(samples: Vec<(f32, f32)>) -> Self {
        Self {
            samples,
            kernel: KernelFunction::Gaussian,
            bandwidth_x: BandwidthMethod::Silverman,
            bandwidth_y: BandwidthMethod::Silverman,
            eval_grid: None,
            n_eval_points: 100, // 100x100 = 10,000 points
        }
    }

    /// Set the kernel function
    pub fn with_kernel(mut self, kernel: KernelFunction) -> Self {
        self.kernel = kernel;
        self
    }

    /// Set manual bandwidth for both dimensions
    pub fn with_bandwidth(mut self, bandwidth: f32) -> Self {
        self.bandwidth_x = BandwidthMethod::Manual(bandwidth);
        self.bandwidth_y = BandwidthMethod::Manual(bandwidth);
        self
    }

    /// Set bandwidths separately for x and y
    pub fn with_bandwidths(mut self, bandwidth_x: f32, bandwidth_y: f32) -> Self {
        self.bandwidth_x = BandwidthMethod::Manual(bandwidth_x);
        self.bandwidth_y = BandwidthMethod::Manual(bandwidth_y);
        self
    }

    /// Set custom evaluation grid
    pub fn with_eval_grid(mut self, x_points: Vec<f32>, y_points: Vec<f32>) -> Self {
        self.eval_grid = Some((x_points, y_points));
        self
    }

    /// Set number of evaluation points per dimension
    pub fn with_n_eval_points(mut self, n: usize) -> Self {
        self.n_eval_points = n;
        self
    }

    /// Estimate bandwidth for a single dimension
    fn estimate_bandwidth_dim(&self, values: &[f32], method: &BandwidthMethod) -> f32 {
        match method {
            BandwidthMethod::Manual(bw) => *bw,
            BandwidthMethod::Silverman => {
                let n = values.len() as f32;
                let std_dev = StandardDeviation::new(values.to_vec()).compute_cpu();
                let q1 = Percentile::new(values.to_vec(), 0.25).compute_cpu();
                let q3 = Percentile::new(values.to_vec(), 0.75).compute_cpu();
                let iqr = q3 - q1;
                let scale = std_dev.min(iqr / 1.34);
                0.9 * scale * n.powf(-0.2)
            }
            BandwidthMethod::Scott => {
                let n = values.len() as f32;
                let std_dev = StandardDeviation::new(values.to_vec()).compute_cpu();
                std_dev * n.powf(-0.2)
            }
        }
    }

    /// Generate evaluation grid across the data range
    fn generate_eval_grid(&self) -> (Vec<f32>, Vec<f32>, f32, f32) {
        if let Some((ref x_points, ref y_points)) = self.eval_grid {
            let x_values: Vec<f32> = self.samples.iter().map(|(x, _)| *x).collect();
            let y_values: Vec<f32> = self.samples.iter().map(|(_, y)| *y).collect();
            let bw_x = self.estimate_bandwidth_dim(&x_values, &self.bandwidth_x);
            let bw_y = self.estimate_bandwidth_dim(&y_values, &self.bandwidth_y);
            return (x_points.clone(), y_points.clone(), bw_x, bw_y);
        }

        let x_values: Vec<f32> = self.samples.iter().map(|(x, _)| *x).collect();
        let y_values: Vec<f32> = self.samples.iter().map(|(_, y)| *y).collect();

        let (x_min, x_max) = MinMax::new(x_values.clone()).compute_cpu();
        let (y_min, y_max) = MinMax::new(y_values.clone()).compute_cpu();

        let bw_x = self.estimate_bandwidth_dim(&x_values, &self.bandwidth_x);
        let bw_y = self.estimate_bandwidth_dim(&y_values, &self.bandwidth_y);

        // Extend range slightly beyond data bounds
        let x_padding = bw_x * 3.0;
        let y_padding = bw_y * 3.0;

        let x_start = x_min - x_padding;
        let x_end = x_max + x_padding;
        let x_step = (x_end - x_start) / (self.n_eval_points - 1) as f32;

        let y_start = y_min - y_padding;
        let y_end = y_max + y_padding;
        let y_step = (y_end - y_start) / (self.n_eval_points - 1) as f32;

        let x_points: Vec<f32> = (0..self.n_eval_points)
            .map(|i| x_start + i as f32 * x_step)
            .collect();

        let y_points: Vec<f32> = (0..self.n_eval_points)
            .map(|i| y_start + i as f32 * y_step)
            .collect();

        (x_points, y_points, bw_x, bw_y)
    }

    /// Compute 2D KDE on CPU (for small datasets or fallback)
    pub fn compute_cpu(&self) -> KDEResult2D {
        if self.samples.is_empty() {
            return KDEResult2D {
                densities: vec![],
                x_points: vec![],
                y_points: vec![],
                bandwidth_x: 0.0,
                bandwidth_y: 0.0,
                kernel: self.kernel,
            };
        }

        let (x_points, y_points, bw_x, bw_y) = self.generate_eval_grid();
        let n = self.samples.len() as f32;

        // Compute density at each grid point
        let mut densities = Vec::with_capacity(x_points.len() * y_points.len());

        for &y in &y_points {
            for &x in &x_points {
                // Sum kernel contributions from all samples
                let sum: f32 = self
                    .samples
                    .iter()
                    .map(|&(xi, yi)| {
                        let ux = (x - xi) / bw_x;
                        let uy = (y - yi) / bw_y;
                        // Product kernel: K(ux, uy) = K(ux) * K(uy)
                        self.kernel.evaluate(ux) * self.kernel.evaluate(uy)
                    })
                    .sum();

                // Normalize by sample count and bandwidth product
                densities.push(sum / (n * bw_x * bw_y));
            }
        }

        KDEResult2D {
            densities,
            x_points,
            y_points,
            bandwidth_x: bw_x,
            bandwidth_y: bw_y,
            kernel: self.kernel,
        }
    }
}

/// Result of 1D kernel density estimation
#[derive(Clone, Debug)]
pub struct KDEResult {
    /// Density values at evaluation points
    pub densities: Vec<f32>,
    /// Evaluation points
    pub eval_points: Vec<f32>,
    /// Bandwidth used
    pub bandwidth: f32,
    /// Kernel function used
    pub kernel: KernelFunction,
}

impl KDEResult {
    /// Check if density is properly normalized (integral ≈ 1.0)
    pub fn is_normalized(&self) -> bool {
        if self.densities.is_empty() {
            return false;
        }

        // Numerical integration using trapezoidal rule
        let integral: f32 = self
            .densities
            .windows(2)
            .zip(self.eval_points.windows(2))
            .map(|(d, x)| {
                let dx = x[1] - x[0];
                0.5 * (d[0] + d[1]) * dx
            })
            .sum();

        // Allow 10% tolerance for numerical integration error
        (integral - 1.0).abs() < 0.1
    }

    /// Find the peak density value
    pub fn peak_density(&self) -> f32 {
        self.densities
            .iter()
            .fold(f32::NEG_INFINITY, |a, &b| a.max(b))
    }

    /// Find the mode (point with maximum density)
    pub fn mode(&self) -> Option<f32> {
        if self.densities.is_empty() {
            return None;
        }

        let max_idx = self
            .densities
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))?
            .0;

        Some(self.eval_points[max_idx])
    }
}

/// Result of 2D kernel density estimation
#[derive(Clone, Debug)]
pub struct KDEResult2D {
    /// Density values at grid points (row-major order: y varies faster)
    pub densities: Vec<f32>,
    /// X-axis evaluation points
    pub x_points: Vec<f32>,
    /// Y-axis evaluation points
    pub y_points: Vec<f32>,
    /// Bandwidth used for x dimension
    pub bandwidth_x: f32,
    /// Bandwidth used for y dimension
    pub bandwidth_y: f32,
    /// Kernel function used
    pub kernel: KernelFunction,
}

impl KDEResult2D {
    /// Get density at grid position (i, j)
    pub fn density_at(&self, x_idx: usize, y_idx: usize) -> Option<f32> {
        if x_idx >= self.x_points.len() || y_idx >= self.y_points.len() {
            return None;
        }
        let idx = y_idx * self.x_points.len() + x_idx;
        self.densities.get(idx).copied()
    }

    /// Find the peak density value
    pub fn peak_density(&self) -> f32 {
        self.densities
            .iter()
            .fold(f32::NEG_INFINITY, |a, &b| a.max(b))
    }

    /// Find the mode (point with maximum density)
    pub fn mode(&self) -> Option<(f32, f32)> {
        if self.densities.is_empty() {
            return None;
        }

        let max_idx = self
            .densities
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))?
            .0;

        let y_idx = max_idx / self.x_points.len();
        let x_idx = max_idx % self.x_points.len();

        Some((self.x_points[x_idx], self.y_points[y_idx]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_primitive_shader_types() {
        assert_eq!(<f32 as ShaderType>::wgsl_type_name(), "f32");
        assert_eq!(f32::size_bytes(), 4);
        assert_eq!(f32::alignment(), 4);

        assert_eq!(<i32 as ShaderType>::wgsl_type_name(), "i32");
        assert_eq!(i32::size_bytes(), 4);
        assert_eq!(i32::alignment(), 4);

        assert_eq!(<u32 as ShaderType>::wgsl_type_name(), "u32");
        assert_eq!(u32::size_bytes(), 4);
        assert_eq!(u32::alignment(), 4);

        assert_eq!(bool::wgsl_type_name(), "bool");
        assert_eq!(bool::size_bytes(), 4);
        assert_eq!(bool::alignment(), 4);
    }

    #[test]
    fn test_vector_shader_types() {
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
    fn test_matrix_shader_types() {
        assert_eq!(Mat2::wgsl_type_name(), "mat2x2<f32>");
        assert_eq!(Mat2::size_bytes(), 16);
        assert_eq!(Mat2::alignment(), 8);

        assert_eq!(Mat3::wgsl_type_name(), "mat3x3<f32>");
        assert_eq!(Mat3::size_bytes(), 48);
        assert_eq!(Mat3::alignment(), 16);

        assert_eq!(Mat4::wgsl_type_name(), "mat4x4<f32>");
        assert_eq!(Mat4::size_bytes(), 64);
        assert_eq!(Mat4::alignment(), 16);
    }

    #[test]
    fn test_type_compatibility() {
        // Same types are always compatible
        assert!(<f32 as ShaderCompatible<f32>>::is_compatible());
        assert!(<Vec2 as ShaderCompatible<Vec2>>::is_compatible());
        assert!(<Vec3 as ShaderCompatible<Vec3>>::is_compatible());
        assert!(<Vec4 as ShaderCompatible<Vec4>>::is_compatible());

        // f32 can be expanded to vector types
        assert!(<f32 as ShaderCompatible<Vec2>>::is_compatible());
        assert!(<f32 as ShaderCompatible<Vec3>>::is_compatible());
        assert!(<f32 as ShaderCompatible<Vec4>>::is_compatible());

        // Vector expansion compatibility
        assert!(<Vec2 as ShaderCompatible<Vec3>>::is_compatible());
        assert!(<Vec2 as ShaderCompatible<Vec4>>::is_compatible());
        assert!(<Vec3 as ShaderCompatible<Vec4>>::is_compatible());
    }

    #[test]
    fn test_matrix_types() {
        let mat2 = Mat2::identity();
        assert_eq!(mat2.m00, 1.0);
        assert_eq!(mat2.m01, 0.0);
        assert_eq!(mat2.m10, 0.0);
        assert_eq!(mat2.m11, 1.0);

        let mat3 = Mat3::identity();
        assert_eq!(mat3.m00, 1.0);
        assert_eq!(mat3.m11, 1.0);
        assert_eq!(mat3.m22, 1.0);
        assert_eq!(mat3.m01, 0.0);

        let mat4 = Mat4::identity();
        assert_eq!(mat4.m00, 1.0);
        assert_eq!(mat4.m11, 1.0);
        assert_eq!(mat4.m22, 1.0);
        assert_eq!(mat4.m33, 1.0);
        assert_eq!(mat4.m01, 0.0);
    }

    #[test]
    fn test_type_construction_macros() {
        // Test vec2! macro
        let v2 = vec2![1.0, 2.0];
        assert_eq!(v2.x, 1.0);
        assert_eq!(v2.y, 2.0);

        // Test vec3! macro
        let v3 = vec3![1.0, 2.0, 3.0];
        assert_eq!(v3.x, 1.0);
        assert_eq!(v3.y, 2.0);
        assert_eq!(v3.z, 3.0);
        assert_eq!(v3._padding, 0.0); // Check padding

        // Test vec4! macro
        let v4 = vec4![1.0, 2.0, 3.0, 4.0];
        assert_eq!(v4.x, 1.0);
        assert_eq!(v4.y, 2.0);
        assert_eq!(v4.z, 3.0);
        assert_eq!(v4.w, 4.0);

        // Test mat2! macro
        let m2 = mat2![1.0, 2.0, 3.0, 4.0];
        assert_eq!(m2.m00, 1.0);
        assert_eq!(m2.m01, 2.0);
        assert_eq!(m2.m10, 3.0);
        assert_eq!(m2.m11, 4.0);

        // Test mat3! macro
        let m3 = mat3![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0];
        assert_eq!(m3.m00, 1.0);
        assert_eq!(m3.m01, 2.0);
        assert_eq!(m3.m02, 3.0);
        assert_eq!(m3.m10, 4.0);
        assert_eq!(m3.m11, 5.0);
        assert_eq!(m3.m12, 6.0);
        assert_eq!(m3.m20, 7.0);
        assert_eq!(m3.m21, 8.0);
        assert_eq!(m3.m22, 9.0);
        // Check padding is zero
        assert_eq!(m3._padding0, 0.0);
        assert_eq!(m3._padding1, 0.0);
        assert_eq!(m3._padding2, 0.0);

        // Test mat4! macro
        let m4 = mat4![
            1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0
        ];
        assert_eq!(m4.m00, 1.0);
        assert_eq!(m4.m01, 2.0);
        assert_eq!(m4.m03, 4.0);
        assert_eq!(m4.m10, 5.0);
        assert_eq!(m4.m33, 16.0);
    }

    #[test]
    fn test_constructor_methods() {
        // Test Vec2::new()
        let v2 = Vec2::new(1.0, 2.0);
        assert_eq!(v2.x, 1.0);
        assert_eq!(v2.y, 2.0);

        // Test Vec2::zero() and Vec2::one()
        let v2_zero = Vec2::zero();
        assert_eq!(v2_zero.x, 0.0);
        assert_eq!(v2_zero.y, 0.0);

        let v2_one = Vec2::one();
        assert_eq!(v2_one.x, 1.0);
        assert_eq!(v2_one.y, 1.0);

        // Test Vec3::new()
        let v3 = Vec3::new(1.0, 2.0, 3.0);
        assert_eq!(v3.x, 1.0);
        assert_eq!(v3.y, 2.0);
        assert_eq!(v3.z, 3.0);
        assert_eq!(v3._padding, 0.0);

        // Test Vec3::zero() and Vec3::one()
        let v3_zero = Vec3::zero();
        assert_eq!(v3_zero.x, 0.0);
        assert_eq!(v3_zero.y, 0.0);
        assert_eq!(v3_zero.z, 0.0);

        let v3_one = Vec3::one();
        assert_eq!(v3_one.x, 1.0);
        assert_eq!(v3_one.y, 1.0);
        assert_eq!(v3_one.z, 1.0);

        // Test Vec4::new()
        let v4 = Vec4::new(1.0, 2.0, 3.0, 4.0);
        assert_eq!(v4.x, 1.0);
        assert_eq!(v4.y, 2.0);
        assert_eq!(v4.z, 3.0);
        assert_eq!(v4.w, 4.0);

        // Test Vec4::zero() and Vec4::one()
        let v4_zero = Vec4::zero();
        assert_eq!(v4_zero.x, 0.0);
        assert_eq!(v4_zero.y, 0.0);
        assert_eq!(v4_zero.z, 0.0);
        assert_eq!(v4_zero.w, 0.0);

        let v4_one = Vec4::one();
        assert_eq!(v4_one.x, 1.0);
        assert_eq!(v4_one.y, 1.0);
        assert_eq!(v4_one.z, 1.0);
        assert_eq!(v4_one.w, 1.0);

        // Test Mat2::new()
        let m2 = Mat2::new(1.0, 2.0, 3.0, 4.0);
        assert_eq!(m2.m00, 1.0);
        assert_eq!(m2.m01, 2.0);
        assert_eq!(m2.m10, 3.0);
        assert_eq!(m2.m11, 4.0);

        // Test Mat2::identity()
        let m2_identity = Mat2::identity();
        assert_eq!(m2_identity.m00, 1.0);
        assert_eq!(m2_identity.m01, 0.0);
        assert_eq!(m2_identity.m10, 0.0);
        assert_eq!(m2_identity.m11, 1.0);

        // Test Mat3::new()
        let m3 = Mat3::new(1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0);
        assert_eq!(m3.m00, 1.0);
        assert_eq!(m3.m01, 2.0);
        assert_eq!(m3.m02, 3.0);
        assert_eq!(m3.m10, 4.0);
        assert_eq!(m3.m11, 5.0);
        assert_eq!(m3.m12, 6.0);
        assert_eq!(m3.m20, 7.0);
        assert_eq!(m3.m21, 8.0);
        assert_eq!(m3.m22, 9.0);
        assert_eq!(m3._padding0, 0.0);
        assert_eq!(m3._padding1, 0.0);
        assert_eq!(m3._padding2, 0.0);

        // Test Mat3::identity()
        let m3_identity = Mat3::identity();
        assert_eq!(m3_identity.m00, 1.0);
        assert_eq!(m3_identity.m11, 1.0);
        assert_eq!(m3_identity.m22, 1.0);
        assert_eq!(m3_identity.m01, 0.0);
        assert_eq!(m3_identity.m02, 0.0);

        // Test Mat4::new()
        let m4 = Mat4::new(
            1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0,
        );
        assert_eq!(m4.m00, 1.0);
        assert_eq!(m4.m01, 2.0);
        assert_eq!(m4.m02, 3.0);
        assert_eq!(m4.m03, 4.0);
        assert_eq!(m4.m10, 5.0);
        assert_eq!(m4.m11, 6.0);
        assert_eq!(m4.m33, 16.0);

        // Test Mat4::identity()
        let m4_identity = Mat4::identity();
        assert_eq!(m4_identity.m00, 1.0);
        assert_eq!(m4_identity.m11, 1.0);
        assert_eq!(m4_identity.m22, 1.0);
        assert_eq!(m4_identity.m33, 1.0);
        assert_eq!(m4_identity.m01, 0.0);
        assert_eq!(m4_identity.m12, 0.0);
    }

    #[test]
    fn test_vec_types() {
        let v2 = vec2![1.0, 2.0];
        assert_eq!(v2.x, 1.0);
        assert_eq!(v2.y, 2.0);

        let v3 = vec3![1.0, 2.0, 3.0];
        assert_eq!(v3.x, 1.0);
        assert_eq!(v3.y, 2.0);
        assert_eq!(v3.z, 3.0);

        let v4 = vec4![1.0, 2.0, 3.0, 4.0];
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
        let min_color = vec4![0.0, 0.0, 0.0, 1.0];
        let max_color = vec4![1.0, 1.0, 1.0, 1.0];
        let color_map = ColorMap::new(min_color, max_color);
        let uniforms = color_map.create_uniforms().unwrap();

        assert_eq!(ColorMap::function_name(), "color_map");
        assert!(ColorMap::wgsl_function().contains("color_map"));
        assert_eq!(uniforms.min_color, [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(uniforms.max_color, [1.0, 1.0, 1.0, 1.0]);
    }

    #[test]
    fn test_position_transform_shader_function() {
        let scale = vec2![2.0, 3.0];
        let offset = vec2![1.0, 1.5];
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
        let color_map = ColorMap::new(vec4![0.0, 0.0, 0.0, 1.0], vec4![1.0, 1.0, 1.0, 1.0]);

        let composed = scale.compose(color_map);
        assert_eq!(
            FunctionChain::<LinearScale, ColorMap>::function_name(),
            "composed_chain"
        );

        let chain_uniforms = composed.create_uniforms();
        assert!(chain_uniforms.is_some());
    }

    #[test]
    fn test_function_composition_type_safety() {
        let scale = LinearScale::new(0.0, 100.0, 0.0, 1.0);
        let color_map = ColorMap::new(vec4![0.0, 0.0, 0.0, 1.0], vec4![1.0, 1.0, 1.0, 1.0]);

        // Valid composition: f32 -> f32 -> Vec4
        let _valid_composition = scale.compose(color_map);
    }

    #[test]
    fn test_compile_time_type_validation() {
        // This test verifies that the type system catches errors at compile time
        // The following code should compile because f32 is compatible with f32
        let scale1 = LinearScale::new(0.0, 100.0, 0.0, 1.0); // f32 -> f32
        let scale2 = LinearScale::new(0.0, 1.0, 0.0, 100.0); // f32 -> f32
        let _valid = scale1.compose(scale2); // f32 -> f32 -> f32

        // This should also compile because f32 can be expanded to Vec4
        let scale = LinearScale::new(0.0, 100.0, 0.0, 1.0);
        let color_map = ColorMap::new(vec4![0.0, 0.0, 0.0, 1.0], vec4![1.0, 1.0, 1.0, 1.0]);
        let _valid = scale.compose(color_map); // f32 -> f32 -> Vec4
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

    // Test for the derive macro - would require importing gup_macros
    // This is a compile-time test to ensure the derive macro works correctly
    #[test]
    fn test_custom_shader_type_definition() {
        // This test would use the derive macro once it's imported
        // For now, we demonstrate manual implementation of a custom type

        #[repr(C)]
        #[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
        struct TestData {
            position: [f32; 2], // Vec2 equivalent
            color: [f32; 4],    // Vec4 equivalent
            intensity: f32,
        }

        impl ShaderType for TestData {
            fn wgsl_type_name() -> &'static str {
                "TestData"
            }

            fn wgsl_type_definition() -> Option<&'static str> {
                Some(
                    "struct TestData {\n    position: vec2<f32>,\n    color: vec4<f32>,\n    intensity: f32,\n}",
                )
            }

            fn size_bytes() -> usize {
                8 + 16 + 4 // Vec2 + Vec4 + f32
            }

            fn alignment() -> usize {
                16 // Max alignment (Vec4)
            }
        }

        assert_eq!(TestData::wgsl_type_name(), "TestData");
        assert!(TestData::wgsl_type_definition().is_some());
        assert_eq!(TestData::size_bytes(), 28);
        assert_eq!(TestData::alignment(), 16);

        let definition = TestData::wgsl_type_definition().unwrap();
        assert!(definition.contains("struct TestData"));
        assert!(definition.contains("position: vec2<f32>"));
        assert!(definition.contains("color: vec4<f32>"));
        assert!(definition.contains("intensity: f32"));
    }

    #[test]
    fn test_shader_uniform_trait() {
        // Test that LinearScaleUniforms implements ShaderUniform
        let wgsl_def = LinearScaleUniforms::wgsl_struct_definition();
        assert!(wgsl_def.contains("struct LinearScaleUniforms"));
        assert!(wgsl_def.contains("domain_min: f32"));
        assert!(wgsl_def.contains("domain_max: f32"));
        assert_eq!(LinearScaleUniforms::wgsl_type_name(), "LinearScaleUniforms");

        // Test ColorMapUniforms
        let color_def = ColorMapUniforms::wgsl_struct_definition();
        assert!(color_def.contains("struct ColorMapUniforms"));
        assert!(color_def.contains("min_color: vec4<f32>"));
        assert!(color_def.contains("max_color: vec4<f32>"));
        assert_eq!(ColorMapUniforms::wgsl_type_name(), "ColorMapUniforms");

        // Test PositionTransformUniforms
        let pos_def = PositionTransformUniforms::wgsl_struct_definition();
        assert!(pos_def.contains("struct PositionTransformUniforms"));
        assert!(pos_def.contains("scale: vec2<f32>"));
        assert!(pos_def.contains("offset: vec2<f32>"));
        assert_eq!(
            PositionTransformUniforms::wgsl_type_name(),
            "PositionTransformUniforms"
        );

        // Test ChainUniforms
        type TestChain = ChainUniforms<LinearScaleUniforms, ColorMapUniforms>;
        let chain_def = TestChain::wgsl_struct_definition();
        assert!(chain_def.contains("struct ChainUniforms"));
        assert!(chain_def.contains("first: LinearScaleUniforms"));
        assert!(chain_def.contains("second: ColorMapUniforms"));
        assert_eq!(TestChain::wgsl_type_name(), "ChainUniforms");
    }

    #[test]
    fn test_log_scale_creation() {
        let log_scale = LogScale::new(1.0, 1000.0, 0.0, 1.0);
        assert_eq!(log_scale.domain_min, 1.0);
        assert_eq!(log_scale.domain_max, 1000.0);
        assert_eq!(log_scale.range_min, 0.0);
        assert_eq!(log_scale.range_max, 1.0);
        assert_eq!(log_scale.base, 10.0);

        let natural_log = LogScale::natural(1.0, 100.0, 0.0, 1.0);
        assert_eq!(natural_log.base, std::f32::consts::E);

        let custom_base = LogScale::with_base(1.0, 16.0, 0.0, 1.0, 2.0);
        assert_eq!(custom_base.base, 2.0);
    }

    #[test]
    fn test_log_scale_uniforms() {
        let log_scale = LogScale::new(1.0, 1000.0, 0.0, 1.0);
        let uniforms = log_scale.create_uniforms().unwrap();
        assert_eq!(uniforms.domain_min, 1.0);
        assert_eq!(uniforms.domain_max, 1000.0);
        assert_eq!(uniforms.base, 10.0);
        assert_eq!(LogScale::function_name(), "log_scale");
    }

    #[test]
    fn test_power_scale_creation() {
        let power_scale = PowerScale::new(0.0, 100.0, 0.0, 1.0, 2.0);
        assert_eq!(power_scale.exponent, 2.0);

        let sqrt_scale = PowerScale::sqrt(0.0, 100.0, 0.0, 1.0);
        assert_eq!(sqrt_scale.exponent, 0.5);

        let square_scale = PowerScale::square(0.0, 100.0, 0.0, 1.0);
        assert_eq!(square_scale.exponent, 2.0);
    }

    #[test]
    fn test_power_scale_uniforms() {
        let power_scale = PowerScale::new(0.0, 100.0, 0.0, 1.0, 0.5);
        let uniforms = power_scale.create_uniforms().unwrap();
        assert_eq!(uniforms.exponent, 0.5);
        assert_eq!(PowerScale::function_name(), "power_scale");
    }

    #[test]
    fn test_clamp_function() {
        let clamp = Clamp::new(0.0, 1.0);
        assert_eq!(clamp.min, 0.0);
        assert_eq!(clamp.max, 1.0);

        let uniforms = clamp.create_uniforms().unwrap();
        assert_eq!(uniforms.min, 0.0);
        assert_eq!(uniforms.max, 1.0);
        assert_eq!(Clamp::function_name(), "clamp_fn");
    }

    #[test]
    fn test_threshold_function() {
        let threshold = Threshold::new(0.5);
        assert_eq!(threshold.threshold, 0.5);

        let uniforms = threshold.create_uniforms().unwrap();
        assert_eq!(uniforms.threshold, 0.5);
        assert_eq!(Threshold::function_name(), "threshold_fn");
    }

    #[test]
    fn test_smooth_step_function() {
        let smooth_step = SmoothStep::new(0.0, 1.0);
        assert_eq!(smooth_step.edge0, 0.0);
        assert_eq!(smooth_step.edge1, 1.0);

        let uniforms = smooth_step.create_uniforms().unwrap();
        assert_eq!(uniforms.edge0, 0.0);
        assert_eq!(uniforms.edge1, 1.0);
        assert_eq!(SmoothStep::function_name(), "smooth_step_fn");
    }

    #[test]
    fn test_color_gradient_creation() {
        let colors = vec![
            vec4![0.0, 0.0, 0.0, 1.0],
            vec4![1.0, 0.0, 0.0, 1.0],
            vec4![1.0, 1.0, 0.0, 1.0],
        ];
        let stops = vec![0.0, 0.5, 1.0];
        let gradient = ColorGradient::new(colors.clone(), stops);
        assert_eq!(gradient.colors.len(), 3);
        assert_eq!(gradient.stops.len(), 3);

        let even_gradient = ColorGradient::with_colors(colors);
        assert_eq!(even_gradient.stops[0], 0.0);
        assert_eq!(even_gradient.stops[2], 1.0);
    }

    #[test]
    fn test_color_gradient_uniforms() {
        let colors = vec![vec4![0.0, 0.0, 0.0, 1.0], vec4![1.0, 1.0, 1.0, 1.0]];
        let stops = vec![0.0, 1.0];
        let gradient = ColorGradient::new(colors, stops);
        let uniforms = gradient.create_uniforms().unwrap();
        assert_eq!(uniforms.count, 2);
        assert_eq!(uniforms.colors[0], [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(uniforms.colors[1], [1.0, 1.0, 1.0, 1.0]);
        assert_eq!(ColorGradient::function_name(), "color_gradient");
    }

    #[test]
    fn test_scale_composition() {
        // Test composing log scale with color map
        let log_scale = LogScale::new(1.0, 1000.0, 0.0, 1.0);
        let color_map = ColorMap::new(vec4![0.0, 0.0, 0.0, 1.0], vec4![1.0, 1.0, 1.0, 1.0]);
        let composed = log_scale.compose(color_map);

        // Verify the composition maintains correct types
        let uniforms = composed.create_uniforms();
        assert!(uniforms.is_some());
    }

    #[test]
    fn test_filter_composition() {
        // Test composing clamp with threshold
        let clamp = Clamp::new(0.0, 1.0);
        let threshold = Threshold::new(0.5);
        let composed = clamp.compose(threshold);

        let uniforms = composed.create_uniforms();
        assert!(uniforms.is_some());
    }

    #[test]
    fn test_multi_stage_composition() {
        // Test a 3-stage pipeline: power -> clamp -> smooth step
        let power = PowerScale::sqrt(0.0, 100.0, 0.0, 1.5);
        let clamp = Clamp::new(0.0, 1.0);
        let smooth = SmoothStep::new(0.0, 1.0);

        let stage1 = power.compose(clamp);
        let stage2 = stage1.compose(smooth);

        let uniforms = stage2.create_uniforms();
        assert!(uniforms.is_some());
    }

    #[test]
    fn test_conditional_function() {
        // Test conditional composition: different scales based on threshold
        let high_scale = LinearScale::new(0.0, 100.0, 0.5, 1.0);
        let low_scale = LinearScale::new(0.0, 100.0, 0.0, 0.5);
        let conditional = ConditionalFunction::new(50.0, high_scale, low_scale);

        let uniforms = conditional.create_uniforms();
        assert!(uniforms.is_some());
        let u = uniforms.unwrap();
        assert_eq!(u.condition_threshold, 50.0);
        assert_eq!(
            ConditionalFunction::<LinearScale, LinearScale>::function_name(),
            "conditional"
        );
    }

    #[test]
    fn test_temporal_interpolation() {
        let temporal = TemporalInterpolation::new(0.0, 100.0, 2.0);
        assert_eq!(temporal.start_value, 0.0);
        assert_eq!(temporal.end_value, 100.0);
        assert_eq!(temporal.duration, 2.0);

        let uniforms = temporal.create_uniforms().unwrap();
        assert_eq!(uniforms.start_value, 0.0);
        assert_eq!(uniforms.end_value, 100.0);
        assert_eq!(uniforms.duration, 2.0);
        assert_eq!(
            TemporalInterpolation::function_name(),
            "temporal_interpolation"
        );
    }

    #[test]
    fn test_easing_functions() {
        let linear = Easing::linear();
        let uniforms = linear.create_uniforms().unwrap();
        assert_eq!(uniforms.easing_type, 0);

        let ease_in_out = Easing::ease_in_out();
        let uniforms2 = ease_in_out.create_uniforms().unwrap();
        assert_eq!(uniforms2.easing_type, 6); // EaseInOutCubic

        let ease_in_quad = Easing::new(EasingFunction::EaseInQuad);
        let uniforms3 = ease_in_quad.create_uniforms().unwrap();
        assert_eq!(uniforms3.easing_type, 1);

        assert_eq!(Easing::function_name(), "easing");
    }

    #[test]
    fn test_temporal_composition() {
        // Test composing temporal interpolation with easing
        let temporal = TemporalInterpolation::new(0.0, 1.0, 2.0);
        let easing = Easing::ease_in_out();
        let composed = temporal.compose(easing);

        let uniforms = composed.create_uniforms();
        assert!(uniforms.is_some());
    }

    #[test]
    fn test_conditional_with_colors() {
        // Test conditional that outputs different colors
        let hot_color = ColorMap::new(vec4![1.0, 0.0, 0.0, 1.0], vec4![1.0, 1.0, 0.0, 1.0]);
        let cold_color = ColorMap::new(vec4![0.0, 0.0, 1.0, 1.0], vec4![0.0, 1.0, 1.0, 1.0]);
        let conditional = ConditionalFunction::new(0.5, hot_color, cold_color);

        let uniforms = conditional.create_uniforms();
        assert!(uniforms.is_some());
    }

    #[test]
    fn test_complex_pipeline_with_conditional() {
        // Test a complex pipeline: scale -> threshold decision -> different color paths
        let scale = LinearScale::new(0.0, 100.0, 0.0, 1.0);
        let hot_gradient =
            ColorGradient::with_colors(vec![vec4![1.0, 1.0, 0.0, 1.0], vec4![1.0, 0.0, 0.0, 1.0]]);
        let cold_gradient =
            ColorGradient::with_colors(vec![vec4![0.0, 1.0, 1.0, 1.0], vec4![0.0, 0.0, 1.0, 1.0]]);

        let conditional = ConditionalFunction::new(0.5, hot_gradient, cold_gradient);
        let pipeline = scale.compose(conditional);

        let uniforms = pipeline.create_uniforms();
        assert!(uniforms.is_some());
    }

    // GUP-139: Statistical function tests
    #[test]
    fn test_mean_cpu() {
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let mean = Mean::new(values);
        assert_eq!(mean.compute_cpu(), 3.0);

        let empty_mean = Mean::new(vec![]);
        assert_eq!(empty_mean.compute_cpu(), 0.0);
    }

    #[test]
    fn test_std_dev_cpu() {
        let values = vec![2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0];
        let std_dev = StandardDeviation::new(values);
        let result = std_dev.compute_cpu();
        // Expected std dev is 2.0
        assert!((result - 2.0).abs() < 0.01, "Expected ~2.0, got {}", result);

        let single_value = StandardDeviation::new(vec![5.0]);
        assert_eq!(single_value.compute_cpu(), 0.0);
    }

    #[test]
    fn test_min_max_cpu() {
        let values = vec![3.0, 1.0, 4.0, 1.0, 5.0, 9.0, 2.0, 6.0];
        let min_max = MinMax::new(values);
        let (min, max) = min_max.compute_cpu();
        assert_eq!(min, 1.0);
        assert_eq!(max, 9.0);

        let empty_min_max = MinMax::new(vec![]);
        let (min, max) = empty_min_max.compute_cpu();
        assert_eq!(min, 0.0);
        assert_eq!(max, 0.0);
    }

    #[test]
    fn test_percentile_cpu() {
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];

        let p50 = Percentile::new(values.clone(), 0.5);
        assert_eq!(p50.compute_cpu(), 5.0);

        let p25 = Percentile::new(values.clone(), 0.25);
        assert_eq!(p25.compute_cpu(), 3.0);

        let p75 = Percentile::new(values.clone(), 0.75);
        assert_eq!(p75.compute_cpu(), 7.0);

        let empty_percentile = Percentile::new(vec![], 0.5);
        assert_eq!(empty_percentile.compute_cpu(), 0.0);
    }

    #[test]
    fn test_statistics_result_alignment() {
        // Verify that StatisticsResult has correct alignment for GPU
        use std::mem;
        assert_eq!(
            mem::size_of::<StatisticsResult>(),
            32,
            "StatisticsResult should be 32 bytes for GPU alignment"
        );
        assert_eq!(mem::align_of::<StatisticsResult>(), 4);
    }
}

#[cfg(test)]
mod compatibility_tests {
    use super::*;

    #[test]
    fn test_direct_compatibility() {
        assert!(f32::is_compatible_with::<f32>());
        assert!(Vec2::is_compatible_with::<Vec2>());
        assert!(Vec3::is_compatible_with::<Vec3>());
        assert!(Vec4::is_compatible_with::<Vec4>());
    }

    #[test]
    fn test_incompatible_types() {
        assert!(!f32::is_compatible_with::<Vec2>());
        assert!(!Vec2::is_compatible_with::<Vec3>());
        assert!(!Vec3::is_compatible_with::<Vec4>());
    }

    #[test]
    fn test_flexible_compatibility_with_conversions() {
        // f32 can convert to vectors
        assert!(<f32 as FlexibleCompatibility>::is_compatible_through::<Vec2>());
        assert!(<f32 as FlexibleCompatibility>::is_compatible_through::<Vec3>());
        assert!(<f32 as FlexibleCompatibility>::is_compatible_through::<Vec4>());

        // Vec2 can convert to larger vectors
        assert!(<Vec2 as FlexibleCompatibility>::is_compatible_through::<Vec3>());
        assert!(<Vec2 as FlexibleCompatibility>::is_compatible_through::<Vec4>());

        // Vec3 can convert to Vec4
        assert!(<Vec3 as FlexibleCompatibility>::is_compatible_through::<Vec4>());
    }

    #[test]
    fn test_conversion_code_generation() {
        // f32 to Vec3 - needs conversion
        let code = <f32 as FlexibleCompatibility>::conversion_code_for::<Vec3>("temp");
        assert_eq!(code, Some("vec3<f32>(temp, temp, temp)".to_string()));

        // Vec2 to Vec4 - needs conversion
        let code = <Vec2 as FlexibleCompatibility>::conversion_code_for::<Vec4>("pos");
        assert_eq!(code, Some("vec4<f32>(pos.x, pos.y, 0.0, 1.0)".to_string()));

        // Vec3 to Vec4 - needs conversion
        let code = <Vec3 as FlexibleCompatibility>::conversion_code_for::<Vec4>("position");
        assert_eq!(
            code,
            Some("vec4<f32>(position.x, position.y, position.z, 1.0)".to_string())
        );
    }
}

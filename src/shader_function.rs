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
use crate::error::GupResult;
use std::marker::PhantomData;
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
        let mut def = String::from("struct ChainUniforms {\n");
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
            vec4![1.0, 0.0, 0.0, 1.0],      // Red
            vec4![1.0, 0.5, 0.0, 1.0],      // Orange
            vec4![1.0, 1.0, 0.0, 1.0],      // Yellow
            vec4![0.0, 1.0, 0.0, 1.0],      // Green
            vec4![0.0, 0.0, 1.0, 1.0],      // Blue
            vec4![0.294, 0.0, 0.510, 1.0],  // Indigo
            vec4![0.561, 0.0, 1.0, 1.0],    // Violet
        ])
    }

    /// Creates a cool to warm gradient (blue to red).
    pub fn cool_warm() -> Self {
        Self::with_colors(vec![
            vec4![0.0, 0.0, 1.0, 1.0],    // Blue
            vec4![0.0, 0.5, 1.0, 1.0],    // Light blue
            vec4![1.0, 1.0, 1.0, 1.0],    // White
            vec4![1.0, 0.5, 0.0, 1.0],    // Orange
            vec4![1.0, 0.0, 0.0, 1.0],    // Red
        ])
    }

    /// Creates a grayscale gradient.
    pub fn grayscale() -> Self {
        Self::with_colors(vec![
            vec4![0.0, 0.0, 0.0, 1.0],    // Black
            vec4![1.0, 1.0, 1.0, 1.0],    // White
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
        assert!(!self.stops.is_empty(), "Gradient must have at least one stop");
        
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

// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Core shader function types, traits, and composition infrastructure.
//!
//! This module provides the foundational building blocks for Gup's composable
//! shader function system: type definitions (Vec3, Vec4, Mat2, Mat3, Mat4),
//! core traits (ShaderType, ShaderUniform, ComposableShaderFunction), and
//! composition patterns (FunctionChain, ParallelComposition, ConditionalFunction).

use super::conversions::AutoConvert;
use crate::buffer::{BufferType, GpuBuffer};
use crate::error::GupResult;
use crate::{MaybeSend, MaybeSync};
use std::marker::PhantomData;
use std::ops::{Add, Div, Mul, Sub};
use wgpu::{Device, Queue};

// Re-export macros for easier access
/// Trait for types that can be used in GPU shader functions.
pub trait ShaderType: Clone + MaybeSend + MaybeSync + 'static {
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

pub use crate::math::Vec2;

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

/// A 3-component vector with GPU-compatible 16-byte alignment.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vec3 {
    /// X component.
    pub x: f32,
    /// Y component.
    pub y: f32,
    /// Z component.
    pub z: f32,
    /// Padding for 16-byte GPU alignment.
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

// ---------------------------------------------------------------------------
// Conversions: [f32; 3] <-> Vec3
// ---------------------------------------------------------------------------

impl From<[f32; 3]> for Vec3 {
    #[inline]
    fn from(array: [f32; 3]) -> Self {
        Self {
            x: array[0],
            y: array[1],
            z: array[2],
            _padding: 0.0,
        }
    }
}

impl From<Vec3> for [f32; 3] {
    #[inline]
    fn from(vec: Vec3) -> Self {
        [vec.x, vec.y, vec.z]
    }
}

// ---------------------------------------------------------------------------
// Component-wise arithmetic: Vec3 op Vec3
// ---------------------------------------------------------------------------

impl Add for Vec3 {
    type Output = Self;
    #[inline]
    fn add(self, rhs: Self) -> Self {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
            z: self.z + rhs.z,
            _padding: 0.0,
        }
    }
}

impl Sub for Vec3 {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: Self) -> Self {
        Self {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
            z: self.z - rhs.z,
            _padding: 0.0,
        }
    }
}

impl Mul for Vec3 {
    type Output = Self;
    #[inline]
    fn mul(self, rhs: Self) -> Self {
        Self {
            x: self.x * rhs.x,
            y: self.y * rhs.y,
            z: self.z * rhs.z,
            _padding: 0.0,
        }
    }
}

impl Div for Vec3 {
    type Output = Self;
    #[inline]
    fn div(self, rhs: Self) -> Self {
        Self {
            x: self.x / rhs.x,
            y: self.y / rhs.y,
            z: self.z / rhs.z,
            _padding: 0.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Scalar arithmetic: Vec3 op f32  and  f32 op Vec3
// ---------------------------------------------------------------------------

impl Mul<f32> for Vec3 {
    type Output = Self;
    #[inline]
    fn mul(self, rhs: f32) -> Self {
        Self {
            x: self.x * rhs,
            y: self.y * rhs,
            z: self.z * rhs,
            _padding: 0.0,
        }
    }
}

impl Mul<Vec3> for f32 {
    type Output = Vec3;
    #[inline]
    fn mul(self, rhs: Vec3) -> Vec3 {
        Vec3 {
            x: self * rhs.x,
            y: self * rhs.y,
            z: self * rhs.z,
            _padding: 0.0,
        }
    }
}

impl Div<f32> for Vec3 {
    type Output = Self;
    #[inline]
    fn div(self, rhs: f32) -> Self {
        Self {
            x: self.x / rhs,
            y: self.y / rhs,
            z: self.z / rhs,
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

/// A 4-component vector for RGBA colours or homogeneous coordinates.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vec4 {
    /// X component.
    pub x: f32,
    /// Y component.
    pub y: f32,
    /// Z component.
    pub z: f32,
    /// W component.
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

// ---------------------------------------------------------------------------
// Conversions: [f32; 4] <-> Vec4
// ---------------------------------------------------------------------------

impl From<[f32; 4]> for Vec4 {
    #[inline]
    fn from(array: [f32; 4]) -> Self {
        Self {
            x: array[0],
            y: array[1],
            z: array[2],
            w: array[3],
        }
    }
}

impl From<Vec4> for [f32; 4] {
    #[inline]
    fn from(vec: Vec4) -> Self {
        [vec.x, vec.y, vec.z, vec.w]
    }
}

// ---------------------------------------------------------------------------
// Component-wise arithmetic: Vec4 op Vec4
// ---------------------------------------------------------------------------

impl Add for Vec4 {
    type Output = Self;
    #[inline]
    fn add(self, rhs: Self) -> Self {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
            z: self.z + rhs.z,
            w: self.w + rhs.w,
        }
    }
}

impl Sub for Vec4 {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: Self) -> Self {
        Self {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
            z: self.z - rhs.z,
            w: self.w - rhs.w,
        }
    }
}

impl Mul for Vec4 {
    type Output = Self;
    #[inline]
    fn mul(self, rhs: Self) -> Self {
        Self {
            x: self.x * rhs.x,
            y: self.y * rhs.y,
            z: self.z * rhs.z,
            w: self.w * rhs.w,
        }
    }
}

impl Div for Vec4 {
    type Output = Self;
    #[inline]
    fn div(self, rhs: Self) -> Self {
        Self {
            x: self.x / rhs.x,
            y: self.y / rhs.y,
            z: self.z / rhs.z,
            w: self.w / rhs.w,
        }
    }
}

// ---------------------------------------------------------------------------
// Scalar arithmetic: Vec4 op f32  and  f32 op Vec4
// ---------------------------------------------------------------------------

impl Mul<f32> for Vec4 {
    type Output = Self;
    #[inline]
    fn mul(self, rhs: f32) -> Self {
        Self {
            x: self.x * rhs,
            y: self.y * rhs,
            z: self.z * rhs,
            w: self.w * rhs,
        }
    }
}

impl Mul<Vec4> for f32 {
    type Output = Vec4;
    #[inline]
    fn mul(self, rhs: Vec4) -> Vec4 {
        Vec4 {
            x: self * rhs.x,
            y: self * rhs.y,
            z: self * rhs.z,
            w: self * rhs.w,
        }
    }
}

impl Div<f32> for Vec4 {
    type Output = Self;
    #[inline]
    fn div(self, rhs: f32) -> Self {
        Self {
            x: self.x / rhs,
            y: self.y / rhs,
            z: self.z / rhs,
            w: self.w / rhs,
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
    /// Element at row 0, column 0.
    pub m00: f32,
    /// Element at row 0, column 1.
    pub m01: f32,
    /// Element at row 1, column 0.
    pub m10: f32,
    /// Element at row 1, column 1.
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
    /// Element at row 0, column 0.
    pub m00: f32,
    /// Element at row 0, column 1.
    pub m01: f32,
    /// Element at row 0, column 2.
    pub m02: f32,
    /// Padding for row 0 GPU alignment.
    pub _padding0: f32,
    /// Element at row 1, column 0.
    pub m10: f32,
    /// Element at row 1, column 1.
    pub m11: f32,
    /// Element at row 1, column 2.
    pub m12: f32,
    /// Padding for row 1 GPU alignment.
    pub _padding1: f32,
    /// Element at row 2, column 0.
    pub m20: f32,
    /// Element at row 2, column 1.
    pub m21: f32,
    /// Element at row 2, column 2.
    pub m22: f32,
    /// Padding for row 2 GPU alignment.
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
    /// Element at row 0, column 0.
    pub m00: f32,
    /// Element at row 0, column 1.
    pub m01: f32,
    /// Element at row 0, column 2.
    pub m02: f32,
    /// Element at row 0, column 3.
    pub m03: f32,
    /// Element at row 1, column 0.
    pub m10: f32,
    /// Element at row 1, column 1.
    pub m11: f32,
    /// Element at row 1, column 2.
    pub m12: f32,
    /// Element at row 1, column 3.
    pub m13: f32,
    /// Element at row 2, column 0.
    pub m20: f32,
    /// Element at row 2, column 1.
    pub m21: f32,
    /// Element at row 2, column 2.
    pub m22: f32,
    /// Element at row 2, column 3.
    pub m23: f32,
    /// Element at row 3, column 0.
    pub m30: f32,
    /// Element at row 3, column 1.
    pub m31: f32,
    /// Element at row 3, column 2.
    pub m32: f32,
    /// Element at row 3, column 3.
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

    /// Returns the nesting depth for chain uniform types.
    ///
    /// For primitive and non-chain uniform types this is 0. For
    /// [`ChainUniforms<A, B>`] it returns `max(A::chain_depth(),
    /// B::chain_depth()) + 1`.  The depth is used to generate unique WGSL
    /// struct names when chains are nested (e.g. `ChainUniforms_1`,
    /// `ChainUniforms_2`).
    fn chain_depth() -> usize {
        0
    }
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

/// Trait for composable GPU shader functions with typed inputs and outputs.
pub trait ComposableShaderFunction {
    /// The input type of this shader function.
    type Input: ShaderType;
    /// The output type of this shader function.
    type Output: ShaderType;
    /// The uniform buffer type for this shader function.
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

    /// Creates the uniform buffer data for this shader function instance.
    fn create_uniforms(&self) -> Option<Self::Uniforms>;
    /// Returns the WGSL function name used in shader code.
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
/// Backward-compatible trait for checking type compatibility.
pub trait TypeCompatible<T> {
    /// Returns whether the types are compatible.
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
    /// Creates a new function chain from two composable shader functions.
    pub fn new(first: A, second: B) -> Self {
        Self {
            first,
            second,
            _phantom: PhantomData,
        }
    }
}

/// Replaces whole-word occurrences of `word` with `replacement` in WGSL code.
///
/// A "whole word" match requires that the character immediately before and after
/// the match is **not** an ASCII alphanumeric character or underscore.  This
/// prevents renaming `ChainUniforms_1` when the target word is `ChainUniforms`.
pub(crate) fn replace_wgsl_identifier(text: &str, word: &str, replacement: &str) -> String {
    let bytes = text.as_bytes();
    let mut result = String::with_capacity(text.len() + 64);
    let mut pos = 0;
    while let Some(rel) = text[pos..].find(word) {
        let start = pos + rel;
        let end = start + word.len();
        let before_ok = start == 0 || !is_ident_char(bytes[start - 1]);
        let after_ok = end >= bytes.len() || !is_ident_char(bytes[end]);
        if before_ok && after_ok {
            result.push_str(&text[pos..start]);
            result.push_str(replacement);
        } else {
            result.push_str(&text[pos..end]);
        }
        pos = end;
    }
    result.push_str(&text[pos..]);
    result
}

/// Returns `true` if `b` is an ASCII identifier character (`[A-Za-z0-9_]`).
fn is_ident_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Deduplicates WGSL function definitions in a code string.
///
/// When composing shader functions that share component functions (e.g.
/// `LinearScale.compose(LinearScale)`), the generated WGSL may contain the
/// same function definition multiple times.  This helper scans for `fn `
/// boundaries, extracts function names, and emits each function only once.
///
/// Non-function content (empty lines, comments) is preserved.
pub(crate) fn deduplicate_wgsl_functions(wgsl: &str) -> String {
    let mut result = String::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut pos = 0;

    while pos < wgsl.len() {
        // Find the next `fn ` token that starts a function definition.
        // It must either be at the start of the string or preceded by a
        // newline (possibly with whitespace).
        let fn_start = {
            let mut found = None;
            let mut search = pos;
            while search < wgsl.len() {
                match wgsl[search..].find("fn ") {
                    Some(rel) => {
                        let abs = search + rel;
                        // Accept if at start or preceded by a newline
                        // (ignoring any leading whitespace on the line).
                        let before = &wgsl[pos..abs];
                        let trimmed = before.trim_end();
                        if abs == 0
                            || trimmed.is_empty()
                            || trimmed.ends_with('\n')
                            || trimmed.ends_with('}')
                        {
                            found = Some(abs);
                            break;
                        }
                        // Not a top-level function definition, skip.
                        search = abs + 3;
                    }
                    None => break,
                }
            }
            found
        };

        let fn_start = match fn_start {
            Some(s) => s,
            None => break,
        };

        // Emit any content before this function (whitespace, comments).
        let prefix = &wgsl[pos..fn_start];
        if !prefix.trim().is_empty() {
            result.push_str(prefix);
        }

        // Extract the function name: after `fn ` until `(`.
        let name_region = &wgsl[fn_start + 3..];
        let name_end = name_region.find('(').unwrap_or(name_region.len());
        let fn_name = name_region[..name_end].trim().to_string();

        // Find end of function by matching braces.
        let mut depth = 0i32;
        let mut fn_end = fn_start;
        for (i, c) in wgsl[fn_start..].char_indices() {
            match c {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        fn_end = fn_start + i + 1;
                        break;
                    }
                }
                _ => {}
            }
        }

        if seen.insert(fn_name) {
            result.push_str(&wgsl[fn_start..fn_end]);
            result.push_str("\n\n");
        }

        pos = fn_end;
    }

    // Append remaining content.
    let remaining = &wgsl[pos..];
    if !remaining.trim().is_empty() {
        result.push_str(remaining);
    }

    result.trim().to_string()
}

/// Combined uniform data for a chain of two shader functions.
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct ChainUniforms<A, B>
where
    A: Copy,
    B: Copy,
{
    /// Uniforms for the first shader function.
    pub first: A,
    /// Uniforms for the second shader function.
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
        //
        // When a component is itself a ChainUniforms (deep chain), rename its
        // top-level struct from `ChainUniforms` to `ChainUniforms_<depth>` to
        // avoid WGSL name collisions.  The outermost chain always keeps the
        // plain `ChainUniforms` name, matching `wgsl_type_name()`.
        let first_def = A::wgsl_struct_definition();
        let first_type_name: String;
        if !first_def.is_empty() {
            if A::wgsl_type_name() == "ChainUniforms" {
                let suffix = format!("_{}", A::chain_depth());
                def.push_str(&replace_wgsl_identifier(
                    &first_def,
                    "ChainUniforms",
                    &format!("ChainUniforms{suffix}"),
                ));
                first_type_name = format!("ChainUniforms{suffix}");
            } else {
                def.push_str(&first_def);
                first_type_name = A::wgsl_type_name().to_string();
            }
            def.push('\n');
        } else {
            first_type_name = A::wgsl_type_name().to_string();
        }

        let second_def = B::wgsl_struct_definition();
        let second_type_name: String;
        if !second_def.is_empty() {
            if B::wgsl_type_name() == "ChainUniforms" {
                let suffix = format!("_{}", B::chain_depth());
                def.push_str(&replace_wgsl_identifier(
                    &second_def,
                    "ChainUniforms",
                    &format!("ChainUniforms{suffix}"),
                ));
                second_type_name = format!("ChainUniforms{suffix}");
            } else {
                def.push_str(&second_def);
                second_type_name = B::wgsl_type_name().to_string();
            }
            def.push('\n');
        } else {
            second_type_name = B::wgsl_type_name().to_string();
        }

        def.push_str("struct ChainUniforms {\n");
        def.push_str(&format!("    first: {first_type_name},\n"));
        def.push_str(&format!("    second: {second_type_name},\n"));
        def.push('}');
        def
    }

    fn wgsl_type_name() -> &'static str {
        "ChainUniforms"
    }

    fn chain_depth() -> usize {
        1 + std::cmp::max(A::chain_depth(), B::chain_depth())
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
        //
        // When a component is itself a FunctionChain (deep chain), its
        // `composed_chain` and `ChainUniforms` identifiers are renamed with a
        // depth suffix to avoid WGSL name collisions.  The outermost chain
        // always keeps the plain names, matching `function_name()` and
        // `wgsl_type_name()`.
        let mut wgsl = String::new();

        let first_wgsl = self.first.generate_wgsl();
        let first_fn_name: String;
        if A::function_name() == "composed_chain" {
            let depth = <A::Uniforms as ShaderUniform>::chain_depth();
            let suffix = format!("_{depth}");
            let renamed = replace_wgsl_identifier(
                first_wgsl.trim(),
                "composed_chain",
                &format!("composed_chain{suffix}"),
            );
            let renamed = replace_wgsl_identifier(
                &renamed,
                "ChainUniforms",
                &format!("ChainUniforms{suffix}"),
            );
            wgsl.push_str(&renamed);
            first_fn_name = format!("composed_chain{suffix}");
        } else {
            wgsl.push_str(first_wgsl.trim());
            first_fn_name = A::function_name().to_string();
        }
        wgsl.push_str("\n\n");

        let second_wgsl = self.second.generate_wgsl();
        let second_fn_name: String;
        if B::function_name() == "composed_chain" {
            let depth = <B::Uniforms as ShaderUniform>::chain_depth();
            let suffix = format!("_{depth}");
            let renamed = replace_wgsl_identifier(
                second_wgsl.trim(),
                "composed_chain",
                &format!("composed_chain{suffix}"),
            );
            let renamed = replace_wgsl_identifier(
                &renamed,
                "ChainUniforms",
                &format!("ChainUniforms{suffix}"),
            );
            wgsl.push_str(&renamed);
            second_fn_name = format!("composed_chain{suffix}");
        } else {
            wgsl.push_str(second_wgsl.trim());
            second_fn_name = B::function_name().to_string();
        }
        wgsl.push_str("\n\n");

        // Append the composed entry-point that chains them together.
        wgsl.push_str(&format!(
            "fn composed_chain(input: {}, uniforms: ChainUniforms) -> {} {{\n    let intermediate = {}(input, uniforms.first);\n    return {}(intermediate, uniforms.second);\n}}",
            <A::Input as ShaderType>::wgsl_type_name(),
            <B::Output as ShaderType>::wgsl_type_name(),
            first_fn_name,
            second_fn_name
        ));

        // Deduplicate function definitions that appear when both components
        // share the same underlying shader function (e.g. two LinearScale
        // instances produce two identical `fn linear_scale(...)` blocks).
        deduplicate_wgsl_functions(&wgsl)
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
    /// Creates a new empty uniform buffer.
    pub fn new() -> Self {
        Self { buffer: None }
    }

    /// Uploads data to the GPU uniform buffer, creating it if needed.
    pub fn upload(&mut self, device: &Device, queue: &Queue, data: &T) -> GupResult<()> {
        if self.buffer.is_none() {
            self.buffer = Some(GpuBuffer::new(device, BufferType::Uniform, 1));
        }

        if let Some(ref mut buffer) = self.buffer {
            buffer.upload(device, queue, &[*data])?;
        }

        Ok(())
    }

    /// Returns a reference to the underlying GPU buffer, if allocated.
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
    /// Creates a new parallel composition from two shader functions.
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
    /// Output from the first shader function.
    pub first: A,
    /// Output from the second shader function.
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
    /// Uniforms for the first shader function.
    pub first: A,
    /// Uniforms for the second shader function.
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
    /// Creates a new conditional function with the given threshold and branches.
    pub fn new(condition_threshold: f32, true_branch: T, false_branch: F) -> Self {
        Self {
            condition_threshold,
            true_branch,
            false_branch,
            _phantom: PhantomData,
        }
    }
}

/// Combined uniform data for a conditional shader function.
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct ConditionalUniforms<T, F>
where
    T: Copy,
    F: Copy,
{
    /// Threshold value for the condition.
    pub condition_threshold: f32,
    /// Padding for GPU alignment.
    pub _padding: [f32; 3],
    /// Uniforms for the true branch.
    pub true_uniforms: T,
    /// Uniforms for the false branch.
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

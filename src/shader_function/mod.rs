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
pub mod geo;
pub mod macros;

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

// Category-based submodules (GUP-294)
pub mod color;
pub mod core;
pub mod geometric;
pub mod math;
pub mod statistical;
pub mod temporal;

pub use self::core::*;
pub use color::*;
pub use geometric::*;
pub use math::*;
pub use statistical::*;
pub use temporal::*;

pub use conversions::AutoConvert;
pub use macros::*;

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

    // -----------------------------------------------------------------------
    // Vec3 arithmetic and conversion tests
    // -----------------------------------------------------------------------

    #[test]
    fn vec3_from_array() {
        let v: Vec3 = [3.0, 4.0, 5.0].into();
        assert_eq!(v, Vec3::new(3.0, 4.0, 5.0));
        assert_eq!(v._padding, 0.0);
    }

    #[test]
    fn vec3_into_array() {
        let arr: [f32; 3] = Vec3::new(5.0, 6.0, 7.0).into();
        assert_eq!(arr, [5.0, 6.0, 7.0]);
    }

    #[test]
    fn vec3_array_roundtrip() {
        let original = [1.5, 2.5, 3.5];
        let v: Vec3 = original.into();
        let back: [f32; 3] = v.into();
        assert_eq!(back, original);
    }

    #[test]
    fn vec3_add() {
        let a = Vec3::new(1.0, 2.0, 3.0);
        let b = Vec3::new(4.0, 5.0, 6.0);
        assert_eq!(a + b, Vec3::new(5.0, 7.0, 9.0));
    }

    #[test]
    fn vec3_sub() {
        let a = Vec3::new(5.0, 8.0, 10.0);
        let b = Vec3::new(2.0, 3.0, 4.0);
        assert_eq!(a - b, Vec3::new(3.0, 5.0, 6.0));
    }

    #[test]
    fn vec3_mul() {
        let a = Vec3::new(2.0, 3.0, 4.0);
        let b = Vec3::new(5.0, 6.0, 7.0);
        assert_eq!(a * b, Vec3::new(10.0, 18.0, 28.0));
    }

    #[test]
    fn vec3_div() {
        let a = Vec3::new(10.0, 20.0, 30.0);
        let b = Vec3::new(2.0, 5.0, 6.0);
        assert_eq!(a / b, Vec3::new(5.0, 4.0, 5.0));
    }

    #[test]
    fn vec3_mul_scalar() {
        let v = Vec3::new(3.0, 4.0, 5.0);
        assert_eq!(v * 2.0, Vec3::new(6.0, 8.0, 10.0));
        assert_eq!(2.0 * v, Vec3::new(6.0, 8.0, 10.0));
    }

    #[test]
    fn vec3_div_scalar() {
        let v = Vec3::new(10.0, 20.0, 30.0);
        assert_eq!(v / 2.0, Vec3::new(5.0, 10.0, 15.0));
    }

    #[test]
    fn vec3_zero_arithmetic() {
        let v = Vec3::new(5.0, 10.0, 15.0);
        let z = Vec3::zero();
        assert_eq!(v + z, v);
        assert_eq!(v - z, v);
        assert_eq!(v * z, z);
    }

    #[test]
    fn vec3_negative_values() {
        let a = Vec3::new(-1.0, -2.0, -3.0);
        let b = Vec3::new(4.0, 5.0, 6.0);
        assert_eq!(a + b, Vec3::new(3.0, 3.0, 3.0));
        assert_eq!(a * b, Vec3::new(-4.0, -10.0, -18.0));
    }

    #[test]
    fn vec3_padding_preserved() {
        // Ensure _padding is always zero after all operations
        let a = Vec3 {
            x: 1.0,
            y: 2.0,
            z: 3.0,
            _padding: 999.0, // Deliberately set non-zero padding
        };
        let b = Vec3::new(4.0, 5.0, 6.0);

        assert_eq!((a + b)._padding, 0.0);
        assert_eq!((a - b)._padding, 0.0);
        assert_eq!((a * b)._padding, 0.0);
        assert_eq!((a / b)._padding, 0.0);
        assert_eq!((a * 2.0)._padding, 0.0);
        assert_eq!((2.0 * a)._padding, 0.0);
        assert_eq!((a / 2.0)._padding, 0.0);

        // From array also preserves zero padding
        let from_arr: Vec3 = [1.0, 2.0, 3.0].into();
        assert_eq!(from_arr._padding, 0.0);
    }

    #[test]
    fn vec3_large_values() {
        let a = Vec3::new(1e10, 1e10, 1e10);
        let b = Vec3::new(2e10, 2e10, 2e10);
        assert_eq!(a + b, Vec3::new(3e10, 3e10, 3e10));
    }

    #[test]
    fn vec3_bytemuck_pod() {
        let v = Vec3::new(1.0, 2.0, 3.0);
        let bytes: &[u8] = bytemuck::bytes_of(&v);
        assert_eq!(bytes.len(), 16); // 4 fields × 4 bytes
        let roundtrip: &Vec3 = bytemuck::from_bytes(bytes);
        assert_eq!(*roundtrip, v);
    }

    #[test]
    fn vec3_repr_c_layout() {
        assert_eq!(std::mem::size_of::<Vec3>(), 16);
        assert_eq!(std::mem::align_of::<Vec3>(), 4);
        assert_eq!(std::mem::offset_of!(Vec3, x), 0);
        assert_eq!(std::mem::offset_of!(Vec3, y), 4);
        assert_eq!(std::mem::offset_of!(Vec3, z), 8);
        assert_eq!(std::mem::offset_of!(Vec3, _padding), 12);
    }

    // -----------------------------------------------------------------------
    // Vec4 arithmetic and conversion tests
    // -----------------------------------------------------------------------

    #[test]
    fn vec4_from_array() {
        let v: Vec4 = [3.0, 4.0, 5.0, 6.0].into();
        assert_eq!(v, Vec4::new(3.0, 4.0, 5.0, 6.0));
    }

    #[test]
    fn vec4_into_array() {
        let arr: [f32; 4] = Vec4::new(5.0, 6.0, 7.0, 8.0).into();
        assert_eq!(arr, [5.0, 6.0, 7.0, 8.0]);
    }

    #[test]
    fn vec4_array_roundtrip() {
        let original = [1.5, 2.5, 3.5, 4.5];
        let v: Vec4 = original.into();
        let back: [f32; 4] = v.into();
        assert_eq!(back, original);
    }

    #[test]
    fn vec4_add() {
        let a = Vec4::new(1.0, 2.0, 3.0, 4.0);
        let b = Vec4::new(5.0, 6.0, 7.0, 8.0);
        assert_eq!(a + b, Vec4::new(6.0, 8.0, 10.0, 12.0));
    }

    #[test]
    fn vec4_sub() {
        let a = Vec4::new(5.0, 8.0, 10.0, 12.0);
        let b = Vec4::new(2.0, 3.0, 4.0, 5.0);
        assert_eq!(a - b, Vec4::new(3.0, 5.0, 6.0, 7.0));
    }

    #[test]
    fn vec4_mul() {
        let a = Vec4::new(2.0, 3.0, 4.0, 5.0);
        let b = Vec4::new(6.0, 7.0, 8.0, 9.0);
        assert_eq!(a * b, Vec4::new(12.0, 21.0, 32.0, 45.0));
    }

    #[test]
    fn vec4_div() {
        let a = Vec4::new(10.0, 20.0, 30.0, 40.0);
        let b = Vec4::new(2.0, 5.0, 6.0, 8.0);
        assert_eq!(a / b, Vec4::new(5.0, 4.0, 5.0, 5.0));
    }

    #[test]
    fn vec4_mul_scalar() {
        let v = Vec4::new(3.0, 4.0, 5.0, 6.0);
        assert_eq!(v * 2.0, Vec4::new(6.0, 8.0, 10.0, 12.0));
        assert_eq!(2.0 * v, Vec4::new(6.0, 8.0, 10.0, 12.0));
    }

    #[test]
    fn vec4_div_scalar() {
        let v = Vec4::new(10.0, 20.0, 30.0, 40.0);
        assert_eq!(v / 2.0, Vec4::new(5.0, 10.0, 15.0, 20.0));
    }

    #[test]
    fn vec4_zero_arithmetic() {
        let v = Vec4::new(5.0, 10.0, 15.0, 20.0);
        let z = Vec4::zero();
        assert_eq!(v + z, v);
        assert_eq!(v - z, v);
        assert_eq!(v * z, z);
    }

    #[test]
    fn vec4_negative_values() {
        let a = Vec4::new(-1.0, -2.0, -3.0, -4.0);
        let b = Vec4::new(5.0, 6.0, 7.0, 8.0);
        assert_eq!(a + b, Vec4::new(4.0, 4.0, 4.0, 4.0));
        assert_eq!(a * b, Vec4::new(-5.0, -12.0, -21.0, -32.0));
    }

    #[test]
    fn vec4_large_values() {
        let a = Vec4::new(1e10, 1e10, 1e10, 1e10);
        let b = Vec4::new(2e10, 2e10, 2e10, 2e10);
        assert_eq!(a + b, Vec4::new(3e10, 3e10, 3e10, 3e10));
    }

    #[test]
    fn vec4_bytemuck_pod() {
        let v = Vec4::new(1.0, 2.0, 3.0, 4.0);
        let bytes: &[u8] = bytemuck::bytes_of(&v);
        assert_eq!(bytes.len(), 16);
        let roundtrip: &Vec4 = bytemuck::from_bytes(bytes);
        assert_eq!(*roundtrip, v);
    }

    #[test]
    fn vec4_repr_c_layout() {
        assert_eq!(std::mem::size_of::<Vec4>(), 16);
        assert_eq!(std::mem::align_of::<Vec4>(), 4);
        assert_eq!(std::mem::offset_of!(Vec4, x), 0);
        assert_eq!(std::mem::offset_of!(Vec4, y), 4);
        assert_eq!(std::mem::offset_of!(Vec4, z), 8);
        assert_eq!(std::mem::offset_of!(Vec4, w), 12);
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
            clamp: 0,
            _pad0: 0,
            _pad1: 0,
            _pad2: 0,
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

    // --- Deep chain tests (GUP-219) ---

    #[test]
    fn test_deep_chain_struct_definition_unique_names() {
        // A three-function chain: LinearScale → LinearScale → ColorMap
        // produces ChainUniforms<ChainUniforms<LSU, LSU>, CMU>.
        // The inner ChainUniforms must be renamed to avoid collision.
        type DeepChain = ChainUniforms<
            ChainUniforms<LinearScaleUniforms, LinearScaleUniforms>,
            ColorMapUniforms,
        >;

        let def = DeepChain::wgsl_struct_definition();

        // Must contain the inner struct with a depth suffix.
        assert!(
            def.contains("struct ChainUniforms_1"),
            "Missing renamed inner ChainUniforms_1:\n{def}"
        );
        // Must contain the outer struct without suffix.
        assert!(
            def.contains("struct ChainUniforms {"),
            "Missing outer ChainUniforms:\n{def}"
        );
        // The outer struct's first field should reference the renamed inner.
        assert!(
            def.contains("first: ChainUniforms_1"),
            "Outer struct should reference ChainUniforms_1:\n{def}"
        );
        // Component structs must be present.
        assert!(
            def.contains("struct LinearScaleUniforms"),
            "Missing LinearScaleUniforms:\n{def}"
        );
        assert!(
            def.contains("struct ColorMapUniforms"),
            "Missing ColorMapUniforms:\n{def}"
        );
    }

    #[test]
    fn test_deep_chain_depth_values() {
        assert_eq!(f32::chain_depth(), 0);
        assert_eq!(LinearScaleUniforms::chain_depth(), 0);
        assert_eq!(
            <ChainUniforms<LinearScaleUniforms, ColorMapUniforms>>::chain_depth(),
            1
        );
        assert_eq!(
            <ChainUniforms<
                ChainUniforms<LinearScaleUniforms, LinearScaleUniforms>,
                ColorMapUniforms,
            >>::chain_depth(),
            2
        );
    }

    #[test]
    fn test_deep_chain_generate_wgsl_unique_function_names() {
        // Build scale1.compose(scale2).compose(color_map).
        let scale1 = LinearScale::new(0.0, 100.0, 0.0, 1.0);
        let scale2 = LinearScale::new(0.0, 1.0, -1.0, 1.0);
        let color_map = ColorMap::new(vec4![0.0, 0.0, 1.0, 1.0], vec4![1.0, 0.0, 0.0, 1.0]);

        let deep_chain = scale1.compose(scale2).compose(color_map);
        let wgsl = deep_chain.generate_wgsl();

        // Inner chain function should be renamed.
        assert!(
            wgsl.contains("fn composed_chain_1("),
            "Missing renamed inner composed_chain_1:\n{wgsl}"
        );
        // Outer entry point keeps the plain name.
        assert!(
            wgsl.contains("fn composed_chain("),
            "Missing outer composed_chain:\n{wgsl}"
        );
        // The outer function should call the renamed inner.
        assert!(
            wgsl.contains("composed_chain_1(input"),
            "Outer should call composed_chain_1:\n{wgsl}"
        );
        // Both component functions must be present.
        assert!(
            wgsl.contains("fn linear_scale("),
            "Missing linear_scale:\n{wgsl}"
        );
        assert!(wgsl.contains("fn color_map("), "Missing color_map:\n{wgsl}");
    }

    #[test]
    fn test_deep_chain_bytemuck_layout() {
        // Verify that ChainUniforms<ChainUniforms<LSU, LSU>, CMU> can be
        // serialised and deserialised correctly via bytemuck.
        let inner = ChainUniforms {
            first: LinearScaleUniforms {
                domain_min: 0.0,
                domain_max: 100.0,
                range_min: 0.0,
                range_max: 1.0,
                clamp: 0,
                _pad0: 0,
                _pad1: 0,
                _pad2: 0,
            },
            second: LinearScaleUniforms {
                domain_min: 0.0,
                domain_max: 1.0,
                range_min: -1.0,
                range_max: 1.0,
                clamp: 0,
                _pad0: 0,
                _pad1: 0,
                _pad2: 0,
            },
        };
        let outer = ChainUniforms {
            first: inner,
            second: ColorMapUniforms {
                min_color: [0.0, 0.0, 1.0, 1.0],
                max_color: [1.0, 0.0, 0.0, 1.0],
            },
        };

        let bytes = bytemuck::bytes_of(&outer);
        assert!(
            !bytes.is_empty(),
            "Nested ChainUniforms should serialise to non-empty bytes"
        );

        // Round-trip: deserialise back and verify.
        let restored: &ChainUniforms<
            ChainUniforms<LinearScaleUniforms, LinearScaleUniforms>,
            ColorMapUniforms,
        > = bytemuck::from_bytes(bytes);
        assert_eq!(restored.first.first.domain_max, 100.0);
        assert_eq!(restored.first.second.range_min, -1.0);
        assert_eq!(restored.second.min_color, [0.0, 0.0, 1.0, 1.0]);
    }

    #[test]
    fn test_replace_wgsl_identifier() {
        // Basic rename.
        assert_eq!(
            replace_wgsl_identifier("struct ChainUniforms {", "ChainUniforms", "ChainUniforms_1"),
            "struct ChainUniforms_1 {"
        );
        // Should NOT rename an already-suffixed identifier.
        assert_eq!(
            replace_wgsl_identifier(
                "first: ChainUniforms_1,",
                "ChainUniforms",
                "ChainUniforms_2"
            ),
            "first: ChainUniforms_1,"
        );
        // Multiple occurrences.
        assert_eq!(
            replace_wgsl_identifier(
                "uniforms: ChainUniforms) -> ChainUniforms",
                "ChainUniforms",
                "ChainUniforms_1"
            ),
            "uniforms: ChainUniforms_1) -> ChainUniforms_1"
        );
        // No match.
        assert_eq!(
            replace_wgsl_identifier("struct Foo {", "ChainUniforms", "ChainUniforms_1"),
            "struct Foo {"
        );
    }

    #[test]
    fn test_deep_chain_create_uniforms() {
        let scale1 = LinearScale::new(0.0, 100.0, 0.0, 1.0);
        let scale2 = LinearScale::new(0.0, 1.0, -1.0, 1.0);
        let color_map = ColorMap::new(vec4![0.0, 0.0, 1.0, 1.0], vec4![1.0, 0.0, 0.0, 1.0]);

        let deep_chain = scale1.compose(scale2).compose(color_map);
        let uniforms = deep_chain.create_uniforms();
        assert!(uniforms.is_some(), "Deep chain should produce uniforms");

        let u = uniforms.unwrap();
        assert_eq!(u.first.first.domain_max, 100.0);
        assert_eq!(u.first.second.range_min, -1.0);
        assert_eq!(u.second.max_color, [1.0, 0.0, 0.0, 1.0]);
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
        let log_scale = LogScale::new(10.0).domain(1.0, 1000.0).range(0.0, 1.0);
        assert_eq!(log_scale.domain_min, 1.0);
        assert_eq!(log_scale.domain_max, 1000.0);
        assert_eq!(log_scale.range_min, 0.0);
        assert_eq!(log_scale.range_max, 1.0);
        assert_eq!(log_scale.base, 10.0);
        assert!(!log_scale.is_symmetric);

        let natural_log = LogScale::natural(1.0, 100.0, 0.0, 1.0);
        assert_eq!(natural_log.base, std::f32::consts::E);

        let custom_base = LogScale::with_base(1.0, 16.0, 0.0, 1.0, 2.0);
        assert_eq!(custom_base.base, 2.0);
    }

    #[test]
    fn test_log_scale_uniforms() {
        let log_scale = LogScale::new(10.0).domain(1.0, 1000.0).range(0.0, 1.0);
        let uniforms = log_scale.create_uniforms().unwrap();
        assert_eq!(uniforms.domain_min, 1.0);
        assert_eq!(uniforms.domain_max, 1000.0);
        assert_eq!(uniforms.base, 10.0);
        assert_eq!(uniforms.symmetric, 0);
        assert_eq!(LogScale::function_name(), "log_scale");
    }

    // ---- GUP-253: LogScale GPU Shader Function tests ----

    #[test]
    fn test_log_scale_uniforms_layout() {
        // AC1: LogScaleUniforms must be 32 bytes (padded to 16-byte boundary
        // for ChainUniforms compatibility) and 4-byte aligned.
        assert_eq!(std::mem::size_of::<LogScaleUniforms>(), 32);
        assert_eq!(std::mem::align_of::<LogScaleUniforms>(), 4);

        // Verify field offsets match the documented layout.
        assert_eq!(std::mem::offset_of!(LogScaleUniforms, domain_min), 0);
        assert_eq!(std::mem::offset_of!(LogScaleUniforms, domain_max), 4);
        assert_eq!(std::mem::offset_of!(LogScaleUniforms, range_min), 8);
        assert_eq!(std::mem::offset_of!(LogScaleUniforms, range_max), 12);
        assert_eq!(std::mem::offset_of!(LogScaleUniforms, base), 16);
        assert_eq!(std::mem::offset_of!(LogScaleUniforms, symmetric), 20);
        assert_eq!(std::mem::offset_of!(LogScaleUniforms, _pad0), 24);
        assert_eq!(std::mem::offset_of!(LogScaleUniforms, _pad1), 28);

        // Verify bytemuck round-trip.
        let uniforms = LogScaleUniforms {
            domain_min: 1.0,
            domain_max: 1000.0,
            range_min: 0.0,
            range_max: 1.0,
            base: 10.0,
            symmetric: 0,
            _pad0: 0,
            _pad1: 0,
        };
        let bytes = bytemuck::bytes_of(&uniforms);
        assert_eq!(bytes.len(), 32);
        let round_trip: &LogScaleUniforms = bytemuck::from_bytes(bytes);
        assert_eq!(round_trip.domain_min, 1.0);
        assert_eq!(round_trip.domain_max, 1000.0);
        assert_eq!(round_trip.base, 10.0);
    }

    #[test]
    fn test_log_scale_boundary_values() {
        // AC2: log_scale(domain_min) == range_min, log_scale(domain_max) == range_max
        let scale = LogScale::new(10.0).domain(1.0, 1000.0).range(0.0, 1.0);
        let at_min = scale.apply(1.0);
        let at_max = scale.apply(1000.0);
        assert!(
            (at_min - 0.0).abs() < 1e-5,
            "log_scale(domain_min) should be range_min, got {at_min}"
        );
        assert!(
            (at_max - 1.0).abs() < 1e-5,
            "log_scale(domain_max) should be range_max, got {at_max}"
        );
    }

    #[test]
    fn test_log_scale_midpoint_value() {
        // AC2: log_scale(100) with domain=[1,1000], range=[0,1], base=10
        // should be ≈ log10(100)/log10(1000) = 2/3 ≈ 0.667
        let scale = LogScale::new(10.0).domain(1.0, 1000.0).range(0.0, 1.0);
        let result = scale.apply(100.0);
        assert!(
            (result - 2.0 / 3.0).abs() < 1e-5,
            "log_scale(100) should be ~0.667, got {result}"
        );
    }

    #[test]
    fn test_log_scale_base2() {
        // Verify base conversion with base=2: log2(4)/log2(16) = 2/4 = 0.5
        let scale = LogScale::new(2.0).domain(1.0, 16.0).range(0.0, 1.0);
        let result = scale.apply(4.0);
        assert!(
            (result - 0.5).abs() < 1e-5,
            "base-2 log_scale(4) should be 0.5, got {result}"
        );
    }

    #[test]
    fn test_log_scale_base_e() {
        // Verify with natural logarithm base.
        let scale = LogScale::new(std::f32::consts::E)
            .domain(1.0, std::f32::consts::E * std::f32::consts::E)
            .range(0.0, 1.0);
        let result = scale.apply(std::f32::consts::E);
        assert!(
            (result - 0.5).abs() < 1e-4,
            "base-e log_scale(e) should be ~0.5, got {result}"
        );
    }

    #[test]
    fn test_log_scale_zero_guard() {
        // AC3: log_scale(0) and log_scale(-1) should return range_min,
        // not NaN or ±infinity.  Values ≤ 0 are clamped to domain_min.
        let scale = LogScale::new(10.0).domain(1.0, 1000.0).range(0.0, 1.0);

        let at_zero = scale.apply(0.0);
        assert!(
            at_zero.is_finite(),
            "log_scale(0.0) must be finite, got {at_zero}"
        );
        assert!(
            (at_zero - 0.0).abs() < 1e-5,
            "log_scale(0.0) should be range_min (clamped to domain_min), got {at_zero}"
        );

        let at_neg = scale.apply(-1.0);
        assert!(
            at_neg.is_finite(),
            "log_scale(-1.0) must be finite, got {at_neg}"
        );
        assert!(
            (at_neg - 0.0).abs() < 1e-5,
            "log_scale(-1.0) should be range_min (clamped to domain_min), got {at_neg}"
        );
    }

    #[test]
    fn test_log_scale_symmetric_sign_symmetry() {
        // AC4: log_scale(-v) == -log_scale(v) when symmetric=true and domain
        // is centred on zero (i.e. the raw symmetric-log values are symmetric).
        let scale = LogScale::new(10.0)
            .domain(-1000.0, 1000.0)
            .range(-1.0, 1.0)
            .symmetric(true);

        let pos = scale.apply(100.0);
        let neg = scale.apply(-100.0);
        assert!(
            (pos + neg).abs() < 1e-5,
            "Symmetric log_scale should satisfy f(-v) == -f(v): pos={pos}, neg={neg}"
        );
    }

    #[test]
    fn test_log_scale_symmetric_zero() {
        // AC4: log_scale(0.0) returns the midpoint (0.0 mapped through
        // the normalisation) when symmetric=true and domain is centred on zero.
        let scale = LogScale::new(10.0)
            .domain(-1000.0, 1000.0)
            .range(-1.0, 1.0)
            .symmetric(true);

        let at_zero = scale.apply(0.0);
        assert!(
            at_zero.abs() < 1e-5,
            "Symmetric log_scale(0.0) should map to 0.0, got {at_zero}"
        );
    }

    #[test]
    fn test_log_scale_builder_api() {
        // AC5: Builder chaining with new(base), domain(), range(), symmetric().
        let scale = LogScale::new(10.0)
            .domain(1.0, 10.0)
            .range(0.0, 1.0)
            .symmetric(false);

        assert_eq!(scale.base, 10.0);
        assert_eq!(scale.domain_min, 1.0);
        assert_eq!(scale.domain_max, 10.0);
        assert_eq!(scale.range_min, 0.0);
        assert_eq!(scale.range_max, 1.0);
        assert!(!scale.is_symmetric);

        // Defaults: domain=[1,10], range=[0,1], symmetric=false
        let defaults = LogScale::new(2.0);
        assert_eq!(defaults.domain_min, 1.0);
        assert_eq!(defaults.domain_max, 10.0);
        assert_eq!(defaults.range_min, 0.0);
        assert_eq!(defaults.range_max, 1.0);
        assert!(!defaults.is_symmetric);
    }

    #[test]
    fn test_log_scale_wgsl_struct_definition() {
        let def = LogScaleUniforms::wgsl_struct_definition();
        assert!(
            def.contains("symmetric: u32"),
            "WGSL struct should contain symmetric field"
        );
        assert!(
            def.contains("base: f32"),
            "WGSL struct should contain base field"
        );
        assert!(
            def.contains("LogScaleUniforms"),
            "WGSL struct name should be LogScaleUniforms"
        );
    }

    #[test]
    fn test_log_scale_wgsl_function_contents() {
        let wgsl = LogScale::wgsl_function();
        assert!(
            wgsl.contains("fn log_scale("),
            "WGSL should contain log_scale function"
        );
        assert!(wgsl.contains("log2("), "WGSL should use log2 built-in");
        assert!(wgsl.contains("1e-10"), "WGSL should contain epsilon guard");
        assert!(
            wgsl.contains("symmetric"),
            "WGSL should reference symmetric flag"
        );
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

    // ---------------------------------------------------------------
    // Ordinal scale tests
    // ---------------------------------------------------------------

    #[test]
    fn test_ordinal_scale_uniforms_bytemuck_round_trip() {
        let uniforms = OrdinalScaleUniforms {
            range_start: 10.0,
            step_size: 50.0,
            padding: 0.1,
            category_count: 5,
        };

        // Cast to bytes and back — verifies Pod/Zeroable/repr(C) correctness.
        let bytes: &[u8] = bytemuck::bytes_of(&uniforms);
        assert_eq!(bytes.len(), 16);
        let round_tripped: &OrdinalScaleUniforms = bytemuck::from_bytes(bytes);
        assert_eq!(round_tripped.range_start, 10.0);
        assert_eq!(round_tripped.step_size, 50.0);
        assert_eq!(round_tripped.padding, 0.1);
        assert_eq!(round_tripped.category_count, 5);
    }

    #[test]
    fn test_ordinal_scale_uniforms_size_and_alignment() {
        assert_eq!(std::mem::size_of::<OrdinalScaleUniforms>(), 16);
        assert_eq!(std::mem::align_of::<OrdinalScaleUniforms>(), 4);
    }

    #[test]
    fn test_band_scale_three_categories() {
        // 3 categories over [0, 300] with 10% padding.
        let scale = BandScale::new(0.0, 300.0, 3, 0.1);
        let step = 300.0 / 3.0; // 100.0
        let _bw = step * 0.9; // 90.0

        assert!((scale.step_size() - 100.0).abs() < 1e-5);
        assert!((scale.bandwidth() - 90.0).abs() < 1e-5);

        // Centre positions: bw/2, step + bw/2, 2*step + bw/2
        let expected = [45.0, 145.0, 245.0];
        for (i, &exp) in expected.iter().enumerate() {
            let pos = scale.apply(i as u32);
            assert!(
                (pos - exp).abs() < 1e-4,
                "band_scale({i}) = {pos}, expected {exp}"
            );
        }
    }

    #[test]
    fn test_band_scale_two_categories_no_padding() {
        let scale = BandScale::new(0.0, 200.0, 2, 0.0);
        assert!((scale.step_size() - 100.0).abs() < 1e-5);
        assert!((scale.bandwidth() - 100.0).abs() < 1e-5);
        assert!((scale.apply(0) - 50.0).abs() < 1e-5);
        assert!((scale.apply(1) - 150.0).abs() < 1e-5);
    }

    #[test]
    fn test_band_scale_zero_categories() {
        let scale = BandScale::new(0.0, 300.0, 0, 0.1);
        assert_eq!(scale.step_size(), 0.0);
        assert_eq!(scale.bandwidth(), 0.0);
    }

    #[test]
    fn test_band_scale_uniforms() {
        let scale = BandScale::new(0.0, 300.0, 3, 0.1);
        let u = scale.uniforms();
        assert_eq!(u.range_start, 0.0);
        assert!((u.step_size - 100.0).abs() < 1e-5);
        assert_eq!(u.padding, 0.1);
        assert_eq!(u.category_count, 3);
    }

    #[test]
    fn test_band_scale_shader_function() {
        let wgsl = BandScale::wgsl_function();
        assert!(wgsl.contains("band_scale"));
        assert!(wgsl.contains("band_scale_bandwidth"));
        assert!(wgsl.contains("OrdinalScaleUniforms"));
        assert_eq!(BandScale::function_name(), "band_scale");
    }

    #[test]
    fn test_point_scale_four_categories() {
        // 4 categories over [0, 400] with padding = 0.5.
        // step = 400 / (4 - 1 + 0.5) = 400 / 3.5 ≈ 114.2857
        // effective_start = 0 + step * 0.25 ≈ 28.5714
        // Positions: 28.5714, 142.857, 257.143, 371.429
        let scale = PointScale::new(0.0, 400.0, 4, 0.5);
        let step = 400.0 / 3.5;

        assert!((scale.step_size() - step).abs() < 1e-4);

        let start = step * 0.25;
        let expected = [start, start + step, start + 2.0 * step, start + 3.0 * step];
        for (i, &exp) in expected.iter().enumerate() {
            let pos = scale.apply(i as u32);
            assert!(
                (pos - exp).abs() < 1e-3,
                "point_scale({i}) = {pos}, expected {exp}"
            );
        }
    }

    #[test]
    fn test_point_scale_two_categories_no_padding() {
        // 2 categories over [0, 100] with no padding → endpoints.
        let scale = PointScale::new(0.0, 100.0, 2, 0.0);
        assert!((scale.apply(0) - 0.0).abs() < 1e-5);
        assert!((scale.apply(1) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn test_point_scale_single_category() {
        // 1 category → midpoint of range.
        let scale = PointScale::new(0.0, 400.0, 1, 0.5);
        assert!((scale.apply(0) - 200.0).abs() < 1e-5);
    }

    #[test]
    fn test_point_scale_uniforms() {
        let scale = PointScale::new(0.0, 400.0, 4, 0.5);
        let u = scale.uniforms();
        // range_start should be the effective start (with outer padding)
        let expected_start = scale.step_size() * 0.25;
        assert!((u.range_start - expected_start).abs() < 1e-4);
        assert!((u.step_size - scale.step_size()).abs() < 1e-4);
        assert_eq!(u.category_count, 4);
    }

    #[test]
    fn test_point_scale_shader_function() {
        let wgsl = PointScale::wgsl_function();
        assert!(wgsl.contains("point_scale"));
        assert!(wgsl.contains("OrdinalScaleUniforms"));
        assert_eq!(PointScale::function_name(), "point_scale");
    }

    #[test]
    fn test_ordinal_scale_category_index() {
        let scale = OrdinalScale::from_categories(&["Apple", "Banana", "Cherry"]);
        assert_eq!(scale.category_index("Apple"), Some(0));
        assert_eq!(scale.category_index("Banana"), Some(1));
        assert_eq!(scale.category_index("Cherry"), Some(2));
        assert_eq!(scale.category_index("Durian"), None);
        assert_eq!(scale.category_count(), 3);
    }

    #[test]
    fn test_ordinal_scale_duplicate_labels() {
        let scale = OrdinalScale::from_categories(&["A", "B", "A", "C"]);
        assert_eq!(scale.category_count(), 3); // "A" deduped
        assert_eq!(scale.category_index("A"), Some(0));
        assert_eq!(scale.category_index("C"), Some(2)); // originally index 3, but stored as 2
    }

    #[test]
    fn test_ordinal_scale_empty() {
        let scale = OrdinalScale::from_categories(&[]);
        assert_eq!(scale.category_count(), 0);
        assert_eq!(scale.category_index("anything"), None);
    }

    #[test]
    fn test_ordinal_scale_labels() {
        let scale = OrdinalScale::from_categories(&["X", "Y", "Z"]);
        assert_eq!(scale.labels(), &["X", "Y", "Z"]);
    }

    #[test]
    fn test_ordinal_scale_band_scale_integration() {
        let scale = OrdinalScale::from_categories(&["A", "B", "C"]);
        let band = scale.band_scale((0.0, 300.0), 0.1);
        assert_eq!(band.category_count, 3);
        assert!((band.bandwidth() - 90.0).abs() < 1e-4);
    }

    #[test]
    fn test_ordinal_scale_point_scale_integration() {
        let scale = OrdinalScale::from_categories(&["A", "B", "C", "D"]);
        let point = scale.point_scale((0.0, 400.0), 0.5);
        assert_eq!(point.category_count, 4);
    }

    #[test]
    fn test_ordinal_scale_round_trip_five_categories() {
        // AC4 round-trip: from_categories → category_index → uniform → apply
        let labels = ["Mon", "Tue", "Wed", "Thu", "Fri"];
        let scale = OrdinalScale::from_categories(&labels);
        let band = scale.band_scale((0.0, 500.0), 0.2);
        let step = 500.0 / 5.0; // 100.0
        let bw = step * 0.8; // 80.0

        for (i, &label) in labels.iter().enumerate() {
            let idx = scale.category_index(label).unwrap();
            assert_eq!(idx, i as u32);
            let pos = band.apply(idx);
            let expected = i as f32 * step + bw * 0.5;
            assert!(
                (pos - expected).abs() < 1e-4,
                "round-trip for {label}: got {pos}, expected {expected}"
            );
        }
    }

    #[test]
    fn test_ordinal_scale_uniforms() {
        let scale = OrdinalScale::from_categories(&["A", "B", "C"]);
        let u = scale.uniforms((0.0, 300.0), 0.1);
        assert_eq!(u.range_start, 0.0);
        assert!((u.step_size - 100.0).abs() < 1e-5);
        assert_eq!(u.padding, 0.1);
        assert_eq!(u.category_count, 3);
    }

    #[test]
    fn test_band_scale_composable_shader_function() {
        let scale = BandScale::new(0.0, 300.0, 3, 0.1);
        let uniforms = scale.create_uniforms().unwrap();
        assert_eq!(uniforms.category_count, 3);
        assert!((uniforms.step_size - 100.0).abs() < 1e-5);
    }

    #[test]
    fn test_point_scale_composable_shader_function() {
        let scale = PointScale::new(0.0, 400.0, 4, 0.5);
        let uniforms = scale.create_uniforms().unwrap();
        assert_eq!(uniforms.category_count, 4);
    }

    #[test]
    fn test_ordinal_scale_wgsl_struct_definition() {
        let def = OrdinalScaleUniforms::wgsl_struct_definition();
        assert!(def.contains("OrdinalScaleUniforms"));
        assert!(def.contains("range_start"));
        assert!(def.contains("step_size"));
        assert!(def.contains("padding"));
        assert!(def.contains("category_count"));
    }

    #[test]
    fn test_band_scale_compose_with_linear_scale() {
        // BandScale outputs f32, LinearScale takes f32 → composition is valid.
        let band = BandScale::new(0.0, 300.0, 3, 0.1);
        let linear = LinearScale::new(0.0, 300.0, 0.0, 1.0);
        let composed = band.compose(linear);
        let uniforms = composed.create_uniforms();
        assert!(uniforms.is_some());
        let wgsl = composed.generate_wgsl();
        assert!(wgsl.contains("band_scale"));
        assert!(wgsl.contains("linear_scale"));
    }

    #[test]
    fn test_point_scale_compose_with_linear_scale() {
        // PointScale outputs f32, LinearScale takes f32 → composition is valid.
        let point = PointScale::new(0.0, 400.0, 4, 0.5);
        let linear = LinearScale::new(0.0, 400.0, 0.0, 1.0);
        let composed = point.compose(linear);
        let uniforms = composed.create_uniforms();
        assert!(uniforms.is_some());
        let wgsl = composed.generate_wgsl();
        assert!(wgsl.contains("point_scale"));
        assert!(wgsl.contains("linear_scale"));
    }

    #[test]
    fn test_band_scale_compose_with_color_map() {
        // BandScale → ColorMap: f32 → Vec4 composition.
        let band = BandScale::new(0.0, 300.0, 3, 0.1);
        let color_map = ColorMap::new(vec4![0.0, 0.0, 0.0, 1.0], vec4![1.0, 1.0, 1.0, 1.0]);
        let composed = band.compose(color_map);
        let uniforms = composed.create_uniforms();
        assert!(uniforms.is_some());
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
        let log_scale = LogScale::new(10.0).domain(1.0, 1000.0).range(0.0, 1.0);
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

    // ======================================================================
    // GUP-053: Advanced Shader Function Library Tests
    // ======================================================================

    // AC1: Mathematical Transform Functions

    #[test]
    fn test_exponential_scale_creation() {
        let scale = ExponentialScale::new(0.0, 1.0, 0.0, 100.0, 10.0);
        assert_eq!(scale.domain_min, 0.0);
        assert_eq!(scale.domain_max, 1.0);
        assert_eq!(scale.range_min, 0.0);
        assert_eq!(scale.range_max, 100.0);
        assert_eq!(scale.base, 10.0);
    }

    #[test]
    fn test_exponential_scale_base10() {
        let scale = ExponentialScale::base10(0.0, 100.0, 0.0, 1.0);
        assert_eq!(scale.base, 10.0);
    }

    #[test]
    fn test_exponential_scale_natural() {
        let scale = ExponentialScale::natural(0.0, 100.0, 0.0, 1.0);
        assert!((scale.base - std::f32::consts::E).abs() < 0.001);
    }

    #[test]
    fn test_exponential_scale_uniforms() {
        let scale = ExponentialScale::new(0.0, 1.0, 0.0, 100.0, 2.0);
        let uniforms = scale.create_uniforms().unwrap();
        assert_eq!(uniforms.domain_min, 0.0);
        assert_eq!(uniforms.domain_max, 1.0);
        assert_eq!(uniforms.range_min, 0.0);
        assert_eq!(uniforms.range_max, 100.0);
        assert_eq!(uniforms.base, 2.0);
    }

    #[test]
    fn test_exponential_scale_wgsl() {
        let wgsl = ExponentialScale::wgsl_function();
        assert!(wgsl.contains("fn exponential_scale"));
        assert!(wgsl.contains("ExponentialScaleUniforms"));
        assert!(wgsl.contains("pow(scale.base, t)"));
    }

    #[test]
    fn test_exponential_scale_function_name() {
        assert_eq!(ExponentialScale::function_name(), "exponential_scale");
    }

    // AC2: Color and Visual Functions

    #[test]
    fn test_hsv_color_map_creation() {
        let map = HSVColorMap::new(0.0, 360.0, 1.0, 1.0);
        assert_eq!(map.hue_start, 0.0);
        assert_eq!(map.hue_end, 360.0);
        assert_eq!(map.saturation, 1.0);
        assert_eq!(map.value, 1.0);
    }

    #[test]
    fn test_hsv_color_map_rainbow() {
        let map = HSVColorMap::rainbow();
        assert_eq!(map.hue_start, 0.0);
        assert_eq!(map.hue_end, 360.0);
    }

    #[test]
    fn test_hsv_color_map_cool_warm() {
        let map = HSVColorMap::cool_warm();
        assert_eq!(map.hue_start, 240.0);
        assert_eq!(map.hue_end, 0.0);
    }

    #[test]
    fn test_hsv_color_map_uniforms() {
        let map = HSVColorMap::new(120.0, 240.0, 0.8, 0.9);
        let uniforms = map.create_uniforms().unwrap();
        assert_eq!(uniforms.hue_start, 120.0);
        assert_eq!(uniforms.hue_end, 240.0);
        assert_eq!(uniforms.saturation, 0.8);
        assert_eq!(uniforms.value, 0.9);
    }

    #[test]
    fn test_hsv_color_map_wgsl() {
        let wgsl = HSVColorMap::wgsl_function();
        assert!(wgsl.contains("fn hsv_color_map"));
        assert!(wgsl.contains("fn hsv_to_rgb"));
        assert!(wgsl.contains("HSVColorMapUniforms"));
    }

    #[test]
    fn test_hsv_color_map_type_signature() {
        // Input: f32, Output: Vec4
        assert_eq!(<f32 as ShaderType>::wgsl_type_name(), "f32");
        assert_eq!(<Vec4 as ShaderType>::wgsl_type_name(), "vec4<f32>");
    }

    #[test]
    fn test_alpha_blending_creation() {
        let blend = AlphaBlending::new(0.75);
        assert_eq!(blend.alpha, 0.75);
    }

    #[test]
    fn test_alpha_blending_semi_transparent() {
        let blend = AlphaBlending::semi_transparent();
        assert_eq!(blend.alpha, 0.5);
    }

    #[test]
    fn test_alpha_blending_uniforms() {
        let blend = AlphaBlending::new(0.3);
        let uniforms = blend.create_uniforms().unwrap();
        assert_eq!(uniforms.alpha, 0.3);
    }

    #[test]
    fn test_alpha_blending_wgsl() {
        let wgsl = AlphaBlending::wgsl_function();
        assert!(wgsl.contains("fn alpha_blending"));
        assert!(wgsl.contains("color.w * params.alpha"));
    }

    #[test]
    fn test_color_space_converter_rgb_to_hsv() {
        let conv = ColorSpaceConverter::rgb_to_hsv();
        let uniforms = conv.create_uniforms().unwrap();
        assert_eq!(uniforms.direction, 0);
    }

    #[test]
    fn test_color_space_converter_hsv_to_rgb() {
        let conv = ColorSpaceConverter::hsv_to_rgb();
        let uniforms = conv.create_uniforms().unwrap();
        assert_eq!(uniforms.direction, 1);
    }

    #[test]
    fn test_color_space_converter_wgsl() {
        let wgsl = ColorSpaceConverter::wgsl_function();
        assert!(wgsl.contains("fn color_space_converter"));
        assert!(wgsl.contains("fn rgb_to_hsv_convert"));
        assert!(wgsl.contains("fn hsv_to_rgb_convert"));
    }

    // AC3: Geometric and Spatial Functions

    #[test]
    fn test_polar_transform_to_polar() {
        let t = PolarTransform::to_polar(Vec2::new(100.0, 100.0));
        assert!(t.to_polar);
        assert_eq!(t.center.x, 100.0);
        assert_eq!(t.center.y, 100.0);
        assert_eq!(t.angle_offset, 0.0);
    }

    #[test]
    fn test_polar_transform_to_cartesian() {
        let t = PolarTransform::to_cartesian(Vec2::new(50.0, 50.0));
        assert!(!t.to_polar);
    }

    #[test]
    fn test_polar_transform_uniforms() {
        let t = PolarTransform::new(Vec2::new(10.0, 20.0), 0.5);
        let uniforms = t.create_uniforms().unwrap();
        assert_eq!(uniforms.center_x, 10.0);
        assert_eq!(uniforms.center_y, 20.0);
        assert_eq!(uniforms.angle_offset, 0.5);
        assert_eq!(uniforms.direction, 0); // to_polar = true → 0
    }

    #[test]
    fn test_polar_transform_wgsl() {
        let wgsl = PolarTransform::wgsl_function();
        assert!(wgsl.contains("fn polar_transform"));
        assert!(wgsl.contains("atan2"));
        assert!(wgsl.contains("cos(angle)"));
    }

    #[test]
    fn test_matrix_transform_identity() {
        let t = MatrixTransform::identity();
        assert_eq!(t.matrix, [1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
    }

    #[test]
    fn test_matrix_transform_rotation() {
        let t = MatrixTransform::rotation(std::f32::consts::FRAC_PI_2);
        // 90-degree rotation: cos(π/2) ≈ 0, sin(π/2) ≈ 1
        assert!((t.matrix[0] - 0.0).abs() < 0.001); // cos
        assert!((t.matrix[1] - 1.0).abs() < 0.001); // sin
        assert!((t.matrix[2] - -1.0).abs() < 0.001); // -sin
        assert!((t.matrix[3] - 0.0).abs() < 0.001); // cos
    }

    #[test]
    fn test_matrix_transform_scaling() {
        let t = MatrixTransform::scaling(2.0, 3.0);
        assert_eq!(t.matrix, [2.0, 0.0, 0.0, 3.0, 0.0, 0.0]);
    }

    #[test]
    fn test_matrix_transform_translation() {
        let t = MatrixTransform::translation(10.0, 20.0);
        assert_eq!(t.matrix, [1.0, 0.0, 0.0, 1.0, 10.0, 20.0]);
    }

    #[test]
    fn test_matrix_transform_uniforms() {
        let t = MatrixTransform::new([1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        let uniforms = t.create_uniforms().unwrap();
        assert_eq!(uniforms.matrix, [1.0, 2.0, 3.0, 4.0]);
        assert_eq!(uniforms.translation, [5.0, 6.0]);
    }

    #[test]
    fn test_matrix_transform_wgsl() {
        let wgsl = MatrixTransform::wgsl_function();
        assert!(wgsl.contains("fn matrix_transform"));
        assert!(wgsl.contains("params.matrix"));
        assert!(wgsl.contains("params.translation"));
    }

    #[test]
    fn test_projection_transform_creation() {
        let t = ProjectionTransform::new(
            Vec2::new(0.0, 0.0),
            Vec2::new(100.0, 100.0),
            Vec2::new(0.0, 0.0),
            Vec2::new(800.0, 600.0),
        );
        assert_eq!(t.data_min.x, 0.0);
        assert_eq!(t.data_max.x, 100.0);
        assert_eq!(t.viewport_max.x, 800.0);
        assert_eq!(t.viewport_max.y, 600.0);
    }

    #[test]
    fn test_projection_transform_uniforms() {
        let t = ProjectionTransform::new(
            Vec2::new(-1.0, -1.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(0.0, 0.0),
            Vec2::new(800.0, 600.0),
        );
        let uniforms = t.create_uniforms().unwrap();
        assert_eq!(uniforms.data_min, [-1.0, -1.0]);
        assert_eq!(uniforms.data_max, [1.0, 1.0]);
        assert_eq!(uniforms.viewport_min, [0.0, 0.0]);
        assert_eq!(uniforms.viewport_max, [800.0, 600.0]);
    }

    #[test]
    fn test_projection_transform_wgsl() {
        let wgsl = ProjectionTransform::wgsl_function();
        assert!(wgsl.contains("fn projection_transform"));
        assert!(wgsl.contains("ProjectionTransformUniforms"));
    }

    #[test]
    fn test_distance_function_creation() {
        let d = DistanceFunction::new(Vec2::new(5.0, 10.0));
        assert_eq!(d.reference_point.x, 5.0);
        assert_eq!(d.reference_point.y, 10.0);
    }

    #[test]
    fn test_distance_function_from_origin() {
        let d = DistanceFunction::from_origin();
        assert_eq!(d.reference_point.x, 0.0);
        assert_eq!(d.reference_point.y, 0.0);
    }

    #[test]
    fn test_distance_function_uniforms() {
        let d = DistanceFunction::new(Vec2::new(3.0, 4.0));
        let uniforms = d.create_uniforms().unwrap();
        assert_eq!(uniforms.ref_x, 3.0);
        assert_eq!(uniforms.ref_y, 4.0);
    }

    #[test]
    fn test_distance_function_wgsl() {
        let wgsl = DistanceFunction::wgsl_function();
        assert!(wgsl.contains("fn distance_fn"));
        assert!(wgsl.contains("sqrt"));
    }

    // AC4: Statistical and Data Functions

    #[test]
    fn test_normalize_function_creation() {
        let n = NormalizeFunction::new(10.0, 50.0);
        assert_eq!(n.min, 10.0);
        assert_eq!(n.max, 50.0);
    }

    #[test]
    fn test_normalize_function_uniforms() {
        let n = NormalizeFunction::new(0.0, 100.0);
        let uniforms = n.create_uniforms().unwrap();
        assert_eq!(uniforms.min, 0.0);
        assert_eq!(uniforms.max, 100.0);
    }

    #[test]
    fn test_normalize_function_wgsl() {
        let wgsl = NormalizeFunction::wgsl_function();
        assert!(wgsl.contains("fn normalize_fn"));
        assert!(wgsl.contains("NormalizeFunctionUniforms"));
        // Handles zero range
        assert!(wgsl.contains("range == 0.0"));
    }

    #[test]
    fn test_standardize_function_creation() {
        let s = StandardizeFunction::new(50.0, 10.0);
        assert_eq!(s.mean, 50.0);
        assert_eq!(s.std_dev, 10.0);
    }

    #[test]
    fn test_standardize_function_uniforms() {
        let s = StandardizeFunction::new(0.0, 1.0);
        let uniforms = s.create_uniforms().unwrap();
        assert_eq!(uniforms.mean, 0.0);
        assert_eq!(uniforms.std_dev, 1.0);
    }

    #[test]
    fn test_standardize_function_wgsl() {
        let wgsl = StandardizeFunction::wgsl_function();
        assert!(wgsl.contains("fn standardize_fn"));
        assert!(wgsl.contains("params.mean"));
        assert!(wgsl.contains("params.std_dev"));
    }

    #[test]
    fn test_quantile_function_creation() {
        let q = QuantileFunction::new(vec![25.0, 50.0, 75.0]);
        assert_eq!(q.boundaries.len(), 3);
    }

    #[test]
    fn test_quantile_function_from_quartiles() {
        let q = QuantileFunction::from_quartiles(25.0, 50.0, 75.0);
        assert_eq!(q.boundaries, vec![25.0, 50.0, 75.0]);
    }

    #[test]
    fn test_quantile_function_uniforms() {
        let q = QuantileFunction::new(vec![10.0, 20.0, 30.0, 40.0]);
        let uniforms = q.create_uniforms().unwrap();
        assert_eq!(uniforms.count, 4);
        assert_eq!(uniforms.boundaries[0], 10.0);
        assert_eq!(uniforms.boundaries[1], 20.0);
        assert_eq!(uniforms.boundaries[2], 30.0);
        assert_eq!(uniforms.boundaries[3], 40.0);
    }

    #[test]
    fn test_quantile_function_max_boundaries() {
        // Test that we cap at 16 boundaries
        let boundaries: Vec<f32> = (0..20).map(|i| i as f32).collect();
        let q = QuantileFunction::new(boundaries);
        let uniforms = q.create_uniforms().unwrap();
        assert_eq!(uniforms.count, 16); // Capped at 16
    }

    #[test]
    fn test_quantile_function_wgsl() {
        let wgsl = QuantileFunction::wgsl_function();
        assert!(wgsl.contains("fn quantile_fn"));
        assert!(wgsl.contains("params.boundaries"));
    }

    #[test]
    fn test_binning_function_creation() {
        let b = BinningFunction::new(0.0, 100.0, 10);
        assert_eq!(b.min, 0.0);
        assert_eq!(b.max, 100.0);
        assert_eq!(b.bin_count, 10);
    }

    #[test]
    fn test_binning_function_uniforms() {
        let b = BinningFunction::new(0.0, 50.0, 5);
        let uniforms = b.create_uniforms().unwrap();
        assert_eq!(uniforms.min, 0.0);
        assert_eq!(uniforms.max, 50.0);
        assert_eq!(uniforms.bin_count, 5);
    }

    #[test]
    fn test_binning_function_wgsl() {
        let wgsl = BinningFunction::wgsl_function();
        assert!(wgsl.contains("fn binning_fn"));
        assert!(wgsl.contains("BinningFunctionUniforms"));
        assert!(wgsl.contains("params.bin_count"));
    }

    // GUP-053: Composition tests

    #[test]
    fn test_exponential_scale_compose_with_color_map() {
        let scale = ExponentialScale::new(0.0, 100.0, 0.0, 1.0, 10.0);
        let color = ColorMap::new(vec4![0.0, 0.0, 1.0, 1.0], vec4![1.0, 0.0, 0.0, 1.0]);
        let composed = scale.compose(color);
        let wgsl = composed.generate_wgsl();
        assert!(wgsl.contains("exponential_scale"));
        assert!(wgsl.contains("color_map"));
    }

    #[test]
    fn test_normalize_compose_with_hsv_color() {
        let normalize = NormalizeFunction::new(0.0, 100.0);
        let color = HSVColorMap::rainbow();
        let composed = normalize.compose(color);
        let wgsl = composed.generate_wgsl();
        assert!(wgsl.contains("normalize_fn"));
        assert!(wgsl.contains("hsv_color_map"));
    }

    #[test]
    fn test_distance_compose_with_normalize() {
        // Vec2 -> f32 (distance), then f32 -> f32 (normalize)
        let distance = DistanceFunction::from_origin();
        let normalize = NormalizeFunction::new(0.0, 500.0);
        let composed = distance.compose(normalize);
        let wgsl = composed.generate_wgsl();
        assert!(wgsl.contains("distance_fn"));
        assert!(wgsl.contains("normalize_fn"));
    }

    #[test]
    fn test_standardize_compose_with_clamp() {
        let standardize = StandardizeFunction::new(50.0, 10.0);
        let clamp = Clamp::new(-3.0, 3.0);
        let composed = standardize.compose(clamp);
        let wgsl = composed.generate_wgsl();
        assert!(wgsl.contains("standardize_fn"));
        assert!(wgsl.contains("clamp_fn"));
    }

    #[test]
    fn test_polar_transform_compose_with_projection() {
        // Both Vec2 -> Vec2
        let polar = PolarTransform::to_cartesian(Vec2::new(400.0, 300.0));
        let proj = ProjectionTransform::new(
            Vec2::new(-500.0, -500.0),
            Vec2::new(500.0, 500.0),
            Vec2::new(0.0, 0.0),
            Vec2::new(800.0, 600.0),
        );
        let composed = polar.compose(proj);
        let wgsl = composed.generate_wgsl();
        assert!(wgsl.contains("polar_transform"));
        assert!(wgsl.contains("projection_transform"));
    }

    #[test]
    fn test_matrix_transform_compose_with_matrix_transform() {
        // Chain two matrix transforms (rotate then scale)
        let rotate = MatrixTransform::rotation(std::f32::consts::FRAC_PI_4);
        let scale = MatrixTransform::scaling(2.0, 2.0);
        let composed = rotate.compose(scale);
        let wgsl = composed.generate_wgsl();
        assert!(wgsl.contains("matrix_transform"));
    }

    #[test]
    fn test_binning_compose_with_color_gradient() {
        let binning = BinningFunction::new(0.0, 100.0, 10);
        let gradient =
            ColorGradient::with_colors(vec![vec4![0.0, 0.0, 1.0, 1.0], vec4![1.0, 0.0, 0.0, 1.0]]);
        let composed = binning.compose(gradient);
        let wgsl = composed.generate_wgsl();
        assert!(wgsl.contains("binning_fn"));
        assert!(wgsl.contains("color_gradient"));
    }

    #[test]
    fn test_three_stage_pipeline() {
        // standardize → normalize → color map
        let standardize = StandardizeFunction::new(50.0, 15.0);
        let normalize = NormalizeFunction::new(-3.0, 3.0);
        let color = HSVColorMap::cool_warm();
        let pipeline = standardize.compose(normalize).compose(color);
        let wgsl = pipeline.generate_wgsl();
        assert!(wgsl.contains("standardize_fn"));
        assert!(wgsl.contains("normalize_fn"));
        assert!(wgsl.contains("hsv_color_map"));
    }

    #[test]
    fn test_hsv_compose_alpha_blending() {
        // HSVColorMap (f32 → Vec4) then AlphaBlending (Vec4 → Vec4)
        let hsv = HSVColorMap::rainbow();
        let alpha = AlphaBlending::new(0.5);
        let composed = hsv.compose(alpha);
        let wgsl = composed.generate_wgsl();
        assert!(wgsl.contains("hsv_color_map"));
        assert!(wgsl.contains("alpha_blending"));
    }

    #[test]
    fn test_color_space_roundtrip_composition() {
        // RGB → HSV then HSV → RGB
        let to_hsv = ColorSpaceConverter::rgb_to_hsv();
        let to_rgb = ColorSpaceConverter::hsv_to_rgb();
        let roundtrip = to_hsv.compose(to_rgb);
        let wgsl = roundtrip.generate_wgsl();
        assert!(wgsl.contains("color_space_converter"));
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

    // ========================================================================
    // LinearScale / LinearScaleInvert tests (GUP-252)
    // ========================================================================

    #[test]
    fn test_linear_scale_uniforms_layout() {
        use std::mem;

        // Total size should be exactly 32 bytes (8 × 4-byte fields, including padding).
        assert_eq!(
            mem::size_of::<LinearScaleUniforms>(),
            32,
            "LinearScaleUniforms should be 32 bytes"
        );

        // Field offsets (verified via bytemuck round-trip).
        let u = LinearScaleUniforms {
            domain_min: 1.0,
            domain_max: 2.0,
            range_min: 3.0,
            range_max: 4.0,
            clamp: 1,
            _pad0: 0,
            _pad1: 0,
            _pad2: 0,
        };
        let bytes = bytemuck::bytes_of(&u);
        assert_eq!(bytes.len(), 32);

        // Verify field order by reading back as f32/u32 slices.
        let f32_vals: &[f32] = bytemuck::cast_slice(&bytes[0..16]);
        assert_eq!(f32_vals[0], 1.0); // domain_min at offset 0
        assert_eq!(f32_vals[1], 2.0); // domain_max at offset 4
        assert_eq!(f32_vals[2], 3.0); // range_min at offset 8
        assert_eq!(f32_vals[3], 4.0); // range_max at offset 12

        let clamp_val: u32 = u32::from_ne_bytes(bytes[16..20].try_into().unwrap());
        assert_eq!(clamp_val, 1); // clamp at offset 16

        // Padding fields should be zero.
        let pad_vals: &[u32] = bytemuck::cast_slice(&bytes[20..32]);
        assert_eq!(pad_vals, &[0, 0, 0]);
    }

    #[test]
    fn test_linear_scale_wgsl_contains_both_functions() {
        let wgsl = LinearScale::wgsl_function();
        assert!(
            wgsl.contains("fn linear_scale("),
            "Should contain forward function: {wgsl}"
        );
        assert!(
            wgsl.contains("fn linear_scale_invert("),
            "Should contain inverse function: {wgsl}"
        );
        assert!(
            wgsl.contains("clamp_flag"),
            "Should reference clamp_flag: {wgsl}"
        );
        assert!(
            wgsl.contains("LinearScaleUniforms"),
            "Should reference uniform struct: {wgsl}"
        );
    }

    #[test]
    fn test_linear_scale_unclamped_in_range() {
        // domain [0, 100] → range [0, 1], input 50 → 0.5
        let scale = LinearScale::new(0.0, 100.0, 0.0, 1.0);
        let u = scale.create_uniforms().unwrap();
        assert_eq!(u.domain_min, 0.0);
        assert_eq!(u.domain_max, 100.0);
        assert_eq!(u.range_min, 0.0);
        assert_eq!(u.range_max, 1.0);
        assert_eq!(u.clamp, 0);
    }

    #[test]
    fn test_linear_scale_clamped_constructor() {
        let scale = LinearScale::with_clamp(0.0, 100.0, 0.0, 1.0);
        let u = scale.create_uniforms().unwrap();
        assert_eq!(u.clamp, 1);
    }

    #[test]
    fn test_linear_scale_identity_mapping() {
        // domain == range should be identity
        let scale = LinearScale::new(0.0, 100.0, 0.0, 100.0);
        let u = scale.create_uniforms().unwrap();
        assert_eq!(u.domain_min, u.range_min);
        assert_eq!(u.domain_max, u.range_max);
    }

    #[test]
    fn test_linear_scale_invert_creates_companion() {
        let scale = LinearScale::with_clamp(0.0, 100.0, 0.0, 1.0);
        let inv = scale.invert();
        assert_eq!(inv.domain_min, 0.0);
        assert_eq!(inv.domain_max, 100.0);
        assert_eq!(inv.range_min, 0.0);
        assert_eq!(inv.range_max, 1.0);
        assert!(inv.clamp);

        // LinearScaleInvert should use the invert function name.
        assert_eq!(LinearScaleInvert::function_name(), "linear_scale_invert");
    }

    #[test]
    fn test_linear_scale_invert_wgsl_shares_code() {
        // LinearScaleInvert::wgsl_function() should contain both forward and
        // inverse functions (same code block as LinearScale).
        let wgsl = LinearScaleInvert::wgsl_function();
        assert!(wgsl.contains("fn linear_scale("));
        assert!(wgsl.contains("fn linear_scale_invert("));
    }

    #[test]
    fn test_linear_scale_round_trip_composition() {
        // Composing LinearScale → LinearScaleInvert should type-check.
        let scale = LinearScale::new(0.0, 100.0, 0.0, 1.0);
        let inv = scale.invert();
        let composed = scale.compose(inv);
        assert!(composed.create_uniforms().is_some());
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

// ============================================================================
// ColorScale unit tests (GUP-255)
// ============================================================================

#[cfg(test)]
mod color_scale_tests {
    use super::*;
    use std::mem;

    // ------------------------------------------------------------------
    // AC1: ColorScale as a ShaderFunction
    // ------------------------------------------------------------------

    #[test]
    fn color_scale_implements_composable_shader_function() {
        let cs = ColorScale::viridis(0.0, 100.0);
        // Must produce uniforms.
        let u = cs.create_uniforms().expect("uniforms must be Some");
        assert_eq!(u.domain_min, 0.0);
        assert_eq!(u.domain_max, 100.0);
        assert_eq!(u.scale_kind, 0); // Continuous
        assert_eq!(u.stop_count, 11); // viridis has 11 stops
    }

    #[test]
    fn color_scale_wgsl_contains_function() {
        let wgsl = ColorScale::wgsl_function();
        assert!(
            wgsl.contains("fn color_scale("),
            "WGSL must contain color_scale function"
        );
    }

    #[test]
    fn color_scale_function_name() {
        assert_eq!(ColorScale::function_name(), "color_scale");
    }

    #[test]
    fn color_scale_uniforms_size_and_alignment() {
        // 8 × f32/u32 = 32 bytes, 16-byte aligned.
        assert_eq!(
            mem::size_of::<ColorScaleUniforms>(),
            32,
            "ColorScaleUniforms should be 32 bytes"
        );
    }

    #[test]
    fn color_scale_uniforms_wgsl_struct() {
        let def = ColorScaleUniforms::wgsl_struct_definition();
        assert!(def.contains("struct ColorScaleUniforms"));
        assert!(def.contains("domain_min: f32"));
        assert!(def.contains("scale_kind: u32"));
        assert!(def.contains("n_bins: u32"));
        assert!(def.contains("stop_count: u32"));
    }

    #[test]
    fn color_scale_storage_buffer_helpers() {
        let cs = ColorScale::viridis(0.0, 1.0);
        let colors_data = cs.create_colors_buffer_data();
        let stops_data = cs.create_stops_buffer_data();
        // 11 stops × 16 bytes/colour = 176 bytes
        assert_eq!(colors_data.len(), 11 * 16);
        // 11 stops × 4 bytes/f32 = 44 bytes
        assert_eq!(stops_data.len(), 11 * 4);
    }

    // ------------------------------------------------------------------
    // AC2: Built-in Palettes
    // ------------------------------------------------------------------

    #[test]
    fn palette_viridis() {
        let cs = ColorScale::viridis(0.0, 100.0);
        assert_eq!(cs.gradient.count(), 11);
        assert_eq!(cs.domain_min, 0.0);
        assert_eq!(cs.domain_max, 100.0);
        assert_eq!(cs.kind, ColorScaleKind::Continuous);
    }

    #[test]
    fn palette_plasma() {
        let cs = ColorScale::plasma(0.0, 1.0);
        assert_eq!(cs.gradient.count(), 11);
    }

    #[test]
    fn palette_inferno() {
        let cs = ColorScale::inferno(-10.0, 40.0);
        assert_eq!(cs.gradient.count(), 11);
    }

    #[test]
    fn palette_magma() {
        let cs = ColorScale::magma(0.0, 255.0);
        assert_eq!(cs.gradient.count(), 11);
        assert_eq!(cs.domain_min, 0.0);
        assert_eq!(cs.domain_max, 255.0);
    }

    #[test]
    fn palette_rd_bu() {
        let cs = ColorScale::rd_bu(-1.0, 1.0);
        assert_eq!(cs.gradient.count(), 11);
        assert_eq!(cs.domain_min, -1.0);
        assert_eq!(cs.domain_max, 1.0);
    }

    #[test]
    fn palette_constructors_are_pure_rust() {
        // Constructors must not panic and must only build CPU-side data.
        let _ = ColorScale::viridis(0.0, 1.0);
        let _ = ColorScale::plasma(0.0, 1.0);
        let _ = ColorScale::inferno(0.0, 1.0);
        let _ = ColorScale::magma(0.0, 1.0);
        let _ = ColorScale::rd_bu(0.0, 1.0);
    }

    // ------------------------------------------------------------------
    // AC3: Diverging Scale Support
    // ------------------------------------------------------------------

    #[test]
    fn diverging_scale_uniforms() {
        let cs = ColorScale::diverging(ColorScale::rd_bu_gradient(), -5.0, 0.0, 10.0);
        let u = cs.create_uniforms().unwrap();
        assert_eq!(u.scale_kind, 1); // Diverging
        assert_eq!(u.midpoint, 0.0);
        assert_eq!(u.domain_min, -5.0);
        assert_eq!(u.domain_max, 10.0);
    }

    #[test]
    fn diverging_midpoint_maps_to_centre() {
        // The WGSL normalisation logic for diverging scale:
        // when value == midpoint, t should be exactly 0.5.
        // We can verify the CPU-side equivalent of the WGSL logic.
        let domain_min = -5.0_f32;
        let midpoint = 0.0_f32;
        let _domain_max = 10.0_f32;

        // Simulate the WGSL diverging branch for value == midpoint
        let value = midpoint;
        let t = if value <= midpoint {
            let range = midpoint - domain_min;
            if range == 0.0 {
                0.5
            } else {
                0.5 * ((value - domain_min) / range).clamp(0.0, 1.0)
            }
        } else {
            unreachable!()
        };
        assert!(
            (t - 0.5).abs() < 1e-6,
            "Midpoint should map to 0.5, got {t}"
        );
    }

    #[test]
    fn diverging_asymmetric_domain() {
        // Asymmetric domain: midpoint != arithmetic mean.
        let domain_min = -2.0_f32;
        let midpoint = 1.0_f32;
        let domain_max = 100.0_f32;

        // value at midpoint → t = 0.5
        let value = midpoint;
        let t = 0.5 * ((value - domain_min) / (midpoint - domain_min)).clamp(0.0, 1.0);
        assert!((t - 0.5).abs() < 1e-6);

        // value at domain_max → t = 1.0
        let value = domain_max;
        let t = 0.5 + 0.5 * ((value - midpoint) / (domain_max - midpoint)).clamp(0.0, 1.0);
        assert!((t - 1.0).abs() < 1e-6);
    }

    // ------------------------------------------------------------------
    // AC4: Quantize (discrete) variant
    // ------------------------------------------------------------------

    #[test]
    fn quantize_scale_uniforms() {
        let cs = ColorScale::quantize(ColorScale::viridis_gradient(), (0.0, 100.0), 5);
        let u = cs.create_uniforms().unwrap();
        assert_eq!(u.scale_kind, 2); // Quantize
        assert_eq!(u.n_bins, 5);
    }

    #[test]
    fn quantize_bin_assignment_boundaries() {
        // Simulate the WGSL quantize logic.
        let domain_min = 0.0_f32;
        let domain_max = 100.0_f32;
        let n_bins = 5u32;

        let check_bin = |value: f32| -> u32 {
            let normalized = ((value - domain_min) / (domain_max - domain_min)).clamp(0.0, 1.0);
            (normalized * n_bins as f32).min((n_bins - 1) as f32) as u32
        };

        assert_eq!(check_bin(0.0), 0); // bottom edge → bin 0
        assert_eq!(check_bin(10.0), 0); // within first bin
        assert_eq!(check_bin(20.0), 1); // boundary → bin 1
        assert_eq!(check_bin(50.0), 2); // mid → bin 2
        assert_eq!(check_bin(99.9), 4); // near top → last bin
        assert_eq!(check_bin(100.0), 4); // top edge → last bin (clamped)
    }

    #[test]
    #[should_panic(expected = "n_bins must be > 0")]
    fn quantize_zero_bins_panics() {
        ColorScale::quantize(ColorScale::viridis_gradient(), (0.0, 1.0), 0);
    }

    // ------------------------------------------------------------------
    // AC5: Composition with LinearScale
    // ------------------------------------------------------------------

    #[test]
    fn linear_scale_compose_color_scale_type_checks() {
        let scale = LinearScale::new(0.0, 100.0, 0.0, 1.0);
        let cs = ColorScale::viridis(0.0, 1.0);
        let composed = scale.compose(cs);
        // The composed chain should produce uniforms.
        let u = composed.create_uniforms();
        assert!(u.is_some(), "Composed uniforms must be Some");
    }

    #[test]
    fn linear_scale_compose_color_scale_wgsl() {
        let scale = LinearScale::new(0.0, 100.0, 0.0, 1.0);
        let cs = ColorScale::viridis(0.0, 1.0);
        let composed = scale.compose(cs);
        let wgsl = composed.generate_wgsl();
        assert!(
            wgsl.contains("fn linear_scale("),
            "Composed WGSL missing linear_scale:\n{wgsl}"
        );
        assert!(
            wgsl.contains("fn color_scale("),
            "Composed WGSL missing color_scale:\n{wgsl}"
        );
        assert!(
            wgsl.contains("fn composed_chain("),
            "Composed WGSL missing composed_chain:\n{wgsl}"
        );
    }

    // ------------------------------------------------------------------
    // Gradient data helpers
    // ------------------------------------------------------------------

    #[test]
    fn gradient_helpers_produce_correct_types() {
        let g = ColorScale::viridis_gradient();
        assert_eq!(g.count(), 11);
        let g = ColorScale::magma_gradient();
        assert_eq!(g.count(), 11);
        let g = ColorScale::rd_bu_gradient();
        assert_eq!(g.count(), 11);
    }

    // -----------------------------------------------------------------------
    // KeyframeAnimation::evaluate tests
    // -----------------------------------------------------------------------

    #[test]
    fn keyframe_animation_evaluate_empty() {
        let anim = KeyframeAnimation::new();
        assert!((anim.evaluate(0.5) - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn keyframe_animation_evaluate_single() {
        let anim = KeyframeAnimation::new().add_keyframe(0.0, 42.0);
        assert!((anim.evaluate(0.0) - 42.0).abs() < f32::EPSILON);
        assert!((anim.evaluate(0.5) - 42.0).abs() < f32::EPSILON);
    }

    #[test]
    fn keyframe_animation_evaluate_two_keyframes() {
        let anim = KeyframeAnimation::new()
            .add_keyframe(0.0, 0.0)
            .add_keyframe(1.0, 100.0);
        assert!((anim.evaluate(0.0) - 0.0).abs() < f32::EPSILON);
        assert!((anim.evaluate(0.5) - 50.0).abs() < 1e-4);
        assert!((anim.evaluate(1.0) - 100.0).abs() < f32::EPSILON);
    }

    #[test]
    fn keyframe_animation_evaluate_clamps_at_boundaries() {
        let anim = KeyframeAnimation::new()
            .add_keyframe(0.0, 10.0)
            .add_keyframe(1.0, 20.0);
        // Before first keyframe — clamp to first value.
        assert!((anim.evaluate(-1.0) - 10.0).abs() < f32::EPSILON);
        // After last keyframe — clamp to last value.
        assert!((anim.evaluate(2.0) - 20.0).abs() < f32::EPSILON);
    }

    #[test]
    fn keyframe_animation_evaluate_three_keyframes() {
        let anim = KeyframeAnimation::new()
            .add_keyframe(0.0, 0.0)
            .add_keyframe(0.5, 100.0)
            .add_keyframe(1.0, 50.0);
        // Midpoint of first segment: 0.25 → 50.0.
        assert!((anim.evaluate(0.25) - 50.0).abs() < 1e-4);
        // Midpoint of second segment: 0.75 → 75.0.
        assert!((anim.evaluate(0.75) - 75.0).abs() < 1e-4);
    }

    // ------------------------------------------------------------------
    // Perceptual Color Space Conversions (GUP-293)
    // ------------------------------------------------------------------

    #[test]
    fn perceptual_converter_rgb_to_xyz_direction() {
        let conv = PerceptualColorSpaceConverter::rgb_to_xyz();
        let uniforms = conv.create_uniforms().unwrap();
        assert_eq!(uniforms.direction, 0);
    }

    #[test]
    fn perceptual_converter_xyz_to_rgb_direction() {
        let conv = PerceptualColorSpaceConverter::xyz_to_rgb();
        let uniforms = conv.create_uniforms().unwrap();
        assert_eq!(uniforms.direction, 1);
    }

    #[test]
    fn perceptual_converter_rgb_to_lab_direction() {
        let conv = PerceptualColorSpaceConverter::rgb_to_lab();
        let uniforms = conv.create_uniforms().unwrap();
        assert_eq!(uniforms.direction, 2);
    }

    #[test]
    fn perceptual_converter_lab_to_rgb_direction() {
        let conv = PerceptualColorSpaceConverter::lab_to_rgb();
        let uniforms = conv.create_uniforms().unwrap();
        assert_eq!(uniforms.direction, 3);
    }

    #[test]
    fn perceptual_converter_rgb_to_oklab_direction() {
        let conv = PerceptualColorSpaceConverter::rgb_to_oklab();
        let uniforms = conv.create_uniforms().unwrap();
        assert_eq!(uniforms.direction, 4);
    }

    #[test]
    fn perceptual_converter_oklab_to_rgb_direction() {
        let conv = PerceptualColorSpaceConverter::oklab_to_rgb();
        let uniforms = conv.create_uniforms().unwrap();
        assert_eq!(uniforms.direction, 5);
    }

    #[test]
    fn perceptual_converter_rgb_to_lch_direction() {
        let conv = PerceptualColorSpaceConverter::rgb_to_lch();
        let uniforms = conv.create_uniforms().unwrap();
        assert_eq!(uniforms.direction, 6);
    }

    #[test]
    fn perceptual_converter_lch_to_rgb_direction() {
        let conv = PerceptualColorSpaceConverter::lch_to_rgb();
        let uniforms = conv.create_uniforms().unwrap();
        assert_eq!(uniforms.direction, 7);
    }

    #[test]
    fn perceptual_converter_default_d65_illuminant() {
        let conv = PerceptualColorSpaceConverter::rgb_to_lab();
        let uniforms = conv.create_uniforms().unwrap();
        assert!((uniforms.illuminant_x - 0.95047).abs() < 1e-5);
        assert!((uniforms.illuminant_y - 1.0).abs() < 1e-5);
        assert!((uniforms.illuminant_z - 1.08883).abs() < 1e-5);
    }

    #[test]
    fn perceptual_converter_custom_illuminant() {
        let conv =
            PerceptualColorSpaceConverter::rgb_to_lab().with_illuminant(0.96422, 1.0, 0.82521);
        let uniforms = conv.create_uniforms().unwrap();
        assert!((uniforms.illuminant_x - 0.96422).abs() < 1e-5);
        assert!((uniforms.illuminant_z - 0.82521).abs() < 1e-5);
    }

    #[test]
    fn perceptual_converter_wgsl_contains_all_helpers() {
        let wgsl = PerceptualColorSpaceConverter::wgsl_function();
        assert!(wgsl.contains("fn srgb_to_linear"));
        assert!(wgsl.contains("fn linear_to_srgb"));
        assert!(wgsl.contains("fn rgb_to_xyz_convert"));
        assert!(wgsl.contains("fn xyz_to_rgb_convert"));
        assert!(wgsl.contains("fn lab_f("));
        assert!(wgsl.contains("fn lab_f_inv("));
        assert!(wgsl.contains("fn xyz_to_lab_convert"));
        assert!(wgsl.contains("fn lab_to_xyz_convert"));
        assert!(wgsl.contains("fn rgb_to_lab_convert"));
        assert!(wgsl.contains("fn lab_to_rgb_convert"));
        assert!(wgsl.contains("fn rgb_to_oklab_convert"));
        assert!(wgsl.contains("fn oklab_to_rgb_convert"));
        assert!(wgsl.contains("fn lab_to_lch_convert"));
        assert!(wgsl.contains("fn lch_to_lab_convert"));
        assert!(wgsl.contains("fn rgb_to_lch_convert"));
        assert!(wgsl.contains("fn lch_to_rgb_convert"));
        assert!(wgsl.contains("fn perceptual_color_space_converter"));
    }

    #[test]
    fn perceptual_converter_wgsl_srgb_linearisation() {
        let wgsl = PerceptualColorSpaceConverter::wgsl_function();
        assert!(wgsl.contains("0.04045"));
        assert!(wgsl.contains("12.92"));
        assert!(wgsl.contains("0.0031308"));
    }

    #[test]
    fn perceptual_converter_wgsl_xyz_matrix_values() {
        let wgsl = PerceptualColorSpaceConverter::wgsl_function();
        assert!(wgsl.contains("0.4124564"));
        assert!(wgsl.contains("3.2404542"));
    }

    #[test]
    fn perceptual_converter_uniform_size_aligned() {
        assert_eq!(
            std::mem::size_of::<PerceptualColorSpaceConverterUniforms>(),
            16
        );
    }

    // CPU-side reference implementations for validation --------------------

    fn cpu_srgb_to_linear(c: f32) -> f32 {
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    }

    fn cpu_linear_to_srgb(c: f32) -> f32 {
        if c <= 0.0031308 {
            c * 12.92
        } else {
            1.055 * c.powf(1.0 / 2.4) - 0.055
        }
    }

    fn cpu_rgb_to_xyz(rgb: [f32; 3]) -> [f32; 3] {
        let r = cpu_srgb_to_linear(rgb[0]);
        let g = cpu_srgb_to_linear(rgb[1]);
        let b = cpu_srgb_to_linear(rgb[2]);
        [
            r * 0.4124564 + g * 0.3575761 + b * 0.1804375,
            r * 0.2126729 + g * 0.7151522 + b * 0.0721750,
            r * 0.0193339 + g * 0.119_192 + b * 0.9503041,
        ]
    }

    fn cpu_xyz_to_rgb(xyz: [f32; 3]) -> [f32; 3] {
        let r = xyz[0] * 3.2404542 + xyz[1] * -1.5371385 + xyz[2] * -0.4985314;
        let g = xyz[0] * -0.969_266 + xyz[1] * 1.8760108 + xyz[2] * 0.0415560;
        let b = xyz[0] * 0.0556434 + xyz[1] * -0.2040259 + xyz[2] * 1.0572252;
        [
            cpu_linear_to_srgb(r.clamp(0.0, 1.0)),
            cpu_linear_to_srgb(g.clamp(0.0, 1.0)),
            cpu_linear_to_srgb(b.clamp(0.0, 1.0)),
        ]
    }

    fn cpu_lab_f(t: f32) -> f32 {
        let delta: f32 = 6.0 / 29.0;
        if t > delta * delta * delta {
            t.powf(1.0 / 3.0)
        } else {
            t / (3.0 * delta * delta) + 4.0 / 29.0
        }
    }

    fn cpu_lab_f_inv(t: f32) -> f32 {
        let delta: f32 = 6.0 / 29.0;
        if t > delta {
            t * t * t
        } else {
            3.0 * delta * delta * (t - 4.0 / 29.0)
        }
    }

    fn cpu_rgb_to_lab(rgb: [f32; 3]) -> [f32; 3] {
        let xyz = cpu_rgb_to_xyz(rgb);
        let ref_x = 0.95047_f32;
        let ref_y = 1.0_f32;
        let ref_z = 1.08883_f32;
        let fx = cpu_lab_f(xyz[0] / ref_x);
        let fy = cpu_lab_f(xyz[1] / ref_y);
        let fz = cpu_lab_f(xyz[2] / ref_z);
        [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
    }

    fn cpu_lab_to_rgb(lab: [f32; 3]) -> [f32; 3] {
        let ref_x = 0.95047_f32;
        let ref_y = 1.0_f32;
        let ref_z = 1.08883_f32;
        let fy = (lab[0] + 16.0) / 116.0;
        let fx = lab[1] / 500.0 + fy;
        let fz = fy - lab[2] / 200.0;
        let xyz = [
            ref_x * cpu_lab_f_inv(fx),
            ref_y * cpu_lab_f_inv(fy),
            ref_z * cpu_lab_f_inv(fz),
        ];
        cpu_xyz_to_rgb(xyz)
    }

    fn cpu_rgb_to_oklab(rgb: [f32; 3]) -> [f32; 3] {
        let r = cpu_srgb_to_linear(rgb[0]);
        let g = cpu_srgb_to_linear(rgb[1]);
        let b = cpu_srgb_to_linear(rgb[2]);
        let l_ = (0.412_221_46 * r + 0.536_332_55 * g + 0.051_445_995 * b).max(0.0);
        let m_ = (0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b).max(0.0);
        let s_ = (0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b).max(0.0);
        let l_c = l_.cbrt();
        let m_c = m_.cbrt();
        let s_c = s_.cbrt();
        [
            0.210_454_26 * l_c + 0.793_617_8 * m_c - 0.004_072_047 * s_c,
            1.977_998_5 * l_c - 2.428_592_2 * m_c + 0.450_593_7 * s_c,
            0.025_904_037 * l_c + 0.782_771_77 * m_c - 0.808_675_77 * s_c,
        ]
    }

    fn cpu_oklab_to_rgb(lab: [f32; 3]) -> [f32; 3] {
        let l_ = lab[0] + 0.396_337_78 * lab[1] + 0.215_803_76 * lab[2];
        let m_ = lab[0] - 0.105_561_346 * lab[1] - 0.063_854_17 * lab[2];
        let s_ = lab[0] - 0.089_484_18 * lab[1] - 1.291_485_5 * lab[2];
        let l_c = l_ * l_ * l_;
        let m_c = m_ * m_ * m_;
        let s_c = s_ * s_ * s_;
        let r = 4.076_741_7 * l_c - 3.307_711_6 * m_c + 0.230_969_94 * s_c;
        let g = -1.268_438 * l_c + 2.609_757_4 * m_c - 0.341_319_38 * s_c;
        let b = -0.0041960863 * l_c - 0.703_418_6 * m_c + 1.707_614_7 * s_c;
        [
            cpu_linear_to_srgb(r.clamp(0.0, 1.0)),
            cpu_linear_to_srgb(g.clamp(0.0, 1.0)),
            cpu_linear_to_srgb(b.clamp(0.0, 1.0)),
        ]
    }

    fn cpu_lab_to_lch(lab: [f32; 3]) -> [f32; 3] {
        let c = (lab[1] * lab[1] + lab[2] * lab[2]).sqrt();
        let mut h = lab[2].atan2(lab[1]).to_degrees();
        if h < 0.0 {
            h += 360.0;
        }
        [lab[0], c, h]
    }

    fn cpu_lch_to_lab(lch: [f32; 3]) -> [f32; 3] {
        let h_rad = lch[2].to_radians();
        [lch[0], lch[1] * h_rad.cos(), lch[1] * h_rad.sin()]
    }

    // Known-value tests (CIE reference colours) ---------------------------

    #[test]
    fn known_value_white_rgb_to_lab() {
        let lab = cpu_rgb_to_lab([1.0, 1.0, 1.0]);
        assert!((lab[0] - 100.0).abs() < 0.5, "L* for white: {}", lab[0]);
        assert!(lab[1].abs() < 0.5, "a* for white: {}", lab[1]);
        assert!(lab[2].abs() < 0.5, "b* for white: {}", lab[2]);
    }

    #[test]
    fn known_value_black_rgb_to_lab() {
        let lab = cpu_rgb_to_lab([0.0, 0.0, 0.0]);
        assert!(lab[0].abs() < 0.01, "L* for black: {}", lab[0]);
        assert!(lab[1].abs() < 0.01, "a* for black: {}", lab[1]);
        assert!(lab[2].abs() < 0.01, "b* for black: {}", lab[2]);
    }

    #[test]
    fn known_value_red_rgb_to_lab() {
        let lab = cpu_rgb_to_lab([1.0, 0.0, 0.0]);
        assert!((lab[0] - 53.23).abs() < 1.0, "L* for red: {}", lab[0]);
        assert!((lab[1] - 80.11).abs() < 1.0, "a* for red: {}", lab[1]);
        assert!((lab[2] - 67.22).abs() < 1.0, "b* for red: {}", lab[2]);
    }

    #[test]
    fn known_value_white_rgb_to_oklab() {
        let oklab = cpu_rgb_to_oklab([1.0, 1.0, 1.0]);
        assert!((oklab[0] - 1.0).abs() < 0.01, "L for white: {}", oklab[0]);
        assert!(oklab[1].abs() < 0.01, "a for white: {}", oklab[1]);
        assert!(oklab[2].abs() < 0.01, "b for white: {}", oklab[2]);
    }

    #[test]
    fn known_value_black_rgb_to_oklab() {
        let oklab = cpu_rgb_to_oklab([0.0, 0.0, 0.0]);
        assert!(oklab[0].abs() < 0.01, "L for black: {}", oklab[0]);
        assert!(oklab[1].abs() < 0.01, "a for black: {}", oklab[1]);
        assert!(oklab[2].abs() < 0.01, "b for black: {}", oklab[2]);
    }

    #[test]
    fn known_value_red_lch_hue() {
        let lab = cpu_rgb_to_lab([1.0, 0.0, 0.0]);
        let lch = cpu_lab_to_lch(lab);
        assert!(lch[2] > 30.0 && lch[2] < 50.0, "hue for red: {}", lch[2]);
        assert!(lch[1] > 50.0, "chroma for red: {}", lch[1]);
    }

    // Round-trip tests ----------------------------------------------------

    #[test]
    fn round_trip_rgb_xyz_rgb() {
        for rgb in &[
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            [0.5, 0.5, 0.5],
            [1.0, 1.0, 1.0],
            [0.0, 0.0, 0.0],
            [0.2, 0.6, 0.9],
        ] {
            let xyz = cpu_rgb_to_xyz(*rgb);
            let back = cpu_xyz_to_rgb(xyz);
            for i in 0..3 {
                assert!(
                    (back[i] - rgb[i]).abs() < 0.01,
                    "RGB→XYZ→RGB round-trip failed for {rgb:?}: got {back:?}",
                );
            }
        }
    }

    #[test]
    fn round_trip_rgb_lab_rgb() {
        for rgb in &[
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            [0.5, 0.5, 0.5],
            [1.0, 1.0, 1.0],
            [0.2, 0.6, 0.9],
        ] {
            let lab = cpu_rgb_to_lab(*rgb);
            let back = cpu_lab_to_rgb(lab);
            for i in 0..3 {
                assert!(
                    (back[i] - rgb[i]).abs() < 0.01,
                    "RGB→LAB→RGB round-trip failed for {rgb:?}: got {back:?} (via LAB {lab:?})",
                );
            }
        }
    }

    #[test]
    fn round_trip_rgb_oklab_rgb() {
        for rgb in &[
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            [0.5, 0.5, 0.5],
            [1.0, 1.0, 1.0],
            [0.2, 0.6, 0.9],
        ] {
            let oklab = cpu_rgb_to_oklab(*rgb);
            let back = cpu_oklab_to_rgb(oklab);
            for i in 0..3 {
                assert!(
                    (back[i] - rgb[i]).abs() < 0.02,
                    "RGB→OKLab→RGB round-trip failed for {rgb:?}: got {back:?} (via OKLab {oklab:?})",
                );
            }
        }
    }

    #[test]
    fn round_trip_lab_lch_lab() {
        for lab in &[
            [50.0, 30.0, -20.0],
            [100.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            [75.0, -50.0, 40.0],
        ] {
            let lch = cpu_lab_to_lch(*lab);
            let back = cpu_lch_to_lab(lch);
            for i in 0..3 {
                assert!(
                    (back[i] - lab[i]).abs() < 0.01,
                    "LAB→LCH→LAB round-trip failed for {lab:?}: got {back:?}",
                );
            }
        }
    }

    #[test]
    fn round_trip_rgb_lch_rgb() {
        for rgb in &[
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.5, 0.5, 0.5],
            [0.2, 0.6, 0.9],
        ] {
            let lab = cpu_rgb_to_lab(*rgb);
            let lch = cpu_lab_to_lch(lab);
            let lab_back = cpu_lch_to_lab(lch);
            let rgb_back = cpu_lab_to_rgb(lab_back);
            for i in 0..3 {
                assert!(
                    (rgb_back[i] - rgb[i]).abs() < 0.01,
                    "RGB→LCH→RGB round-trip failed for {rgb:?}: got {rgb_back:?}",
                );
            }
        }
    }

    // Perceptual interpolation tests --------------------------------------

    #[test]
    fn perceptual_interpolation_lab_constructor() {
        let a = vec4![1.0, 0.0, 0.0, 1.0];
        let b = vec4![0.0, 0.0, 1.0, 1.0];
        let interp = PerceptualInterpolation::lab(a, b);
        let uniforms = interp.create_uniforms().unwrap();
        assert_eq!(uniforms.space, 0);
        assert!((uniforms.color_a_r - 1.0).abs() < 1e-6);
        assert!((uniforms.color_b_b - 1.0).abs() < 1e-6);
    }

    #[test]
    fn perceptual_interpolation_oklab_constructor() {
        let a = vec4![1.0, 0.0, 0.0, 1.0];
        let b = vec4![0.0, 0.0, 1.0, 1.0];
        let interp = PerceptualInterpolation::oklab(a, b);
        let uniforms = interp.create_uniforms().unwrap();
        assert_eq!(uniforms.space, 1);
    }

    #[test]
    fn perceptual_interpolation_lch_constructor() {
        let a = vec4![1.0, 0.0, 0.0, 1.0];
        let b = vec4![0.0, 0.0, 1.0, 1.0];
        let interp = PerceptualInterpolation::lch(a, b);
        let uniforms = interp.create_uniforms().unwrap();
        assert_eq!(uniforms.space, 2);
    }

    #[test]
    fn perceptual_interpolation_custom_illuminant() {
        let a = vec4![1.0, 0.0, 0.0, 1.0];
        let b = vec4![0.0, 0.0, 1.0, 1.0];
        let interp = PerceptualInterpolation::lab(a, b).with_illuminant(0.96422, 1.0, 0.82521);
        let uniforms = interp.create_uniforms().unwrap();
        assert!((uniforms.illuminant_x - 0.96422).abs() < 1e-5);
    }

    #[test]
    fn perceptual_interpolation_wgsl_contains_entry_point() {
        let wgsl = PerceptualInterpolation::wgsl_function();
        assert!(wgsl.contains("fn perceptual_interpolation"));
        assert!(wgsl.contains("rgb_to_lab_pi"));
        assert!(wgsl.contains("rgb_to_oklab_pi"));
        assert!(wgsl.contains("lch_shortest_hue_lerp"));
    }

    #[test]
    fn perceptual_interpolation_uniform_size() {
        assert_eq!(std::mem::size_of::<PerceptualInterpolationUniforms>(), 48);
    }

    // Perceptual uniformity spot-check ------------------------------------

    #[test]
    fn lab_interpolation_midpoint_is_perceptually_between() {
        let lab_red = cpu_rgb_to_lab([1.0, 0.0, 0.0]);
        let lab_blue = cpu_rgb_to_lab([0.0, 0.0, 1.0]);
        let lab_mid = [
            (lab_red[0] + lab_blue[0]) / 2.0,
            (lab_red[1] + lab_blue[1]) / 2.0,
            (lab_red[2] + lab_blue[2]) / 2.0,
        ];
        let min_l = lab_red[0].min(lab_blue[0]);
        let max_l = lab_red[0].max(lab_blue[0]);
        assert!(
            lab_mid[0] >= min_l && lab_mid[0] <= max_l,
            "Midpoint L*={} not between {}..{}",
            lab_mid[0],
            min_l,
            max_l
        );
    }

    #[test]
    fn oklab_interpolation_avoids_muddy_midpoint() {
        let oklab_red = cpu_rgb_to_oklab([1.0, 0.0, 0.0]);
        let oklab_cyan = cpu_rgb_to_oklab([0.0, 1.0, 1.0]);
        let oklab_mid = [
            (oklab_red[0] + oklab_cyan[0]) / 2.0,
            (oklab_red[1] + oklab_cyan[1]) / 2.0,
            (oklab_red[2] + oklab_cyan[2]) / 2.0,
        ];
        let mid_rgb = cpu_oklab_to_rgb(oklab_mid);
        let min_ch = mid_rgb.iter().cloned().fold(f32::INFINITY, f32::min);
        let max_ch = mid_rgb.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let range = max_ch - min_ch;
        assert!(
            range > 0.05,
            "OKLab midpoint is too grey: {mid_rgb:?} (range={range})",
        );
    }

    // Composability test --------------------------------------------------

    #[test]
    fn perceptual_converter_composes_with_linear_scale() {
        let scale = LinearScale::new(0.0, 100.0, 0.0, 1.0);
        let interp =
            PerceptualInterpolation::oklab(vec4![1.0, 0.0, 0.0, 1.0], vec4![0.0, 0.0, 1.0, 1.0]);
        let composed = scale.compose(interp);
        let uniforms = composed.create_uniforms();
        assert!(uniforms.is_some());
    }

    #[test]
    fn perceptual_converter_function_name() {
        assert_eq!(
            PerceptualColorSpaceConverter::function_name(),
            "perceptual_color_space_converter"
        );
    }

    #[test]
    fn perceptual_interpolation_function_name() {
        assert_eq!(
            PerceptualInterpolation::function_name(),
            "perceptual_interpolation"
        );
    }
}

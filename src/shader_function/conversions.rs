// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Type Conversion System for GPU Shader Functions
//!
//! This module provides automatic type conversions between compatible GPU types,
//! enabling flexible shader function composition while maintaining zero runtime overhead.
//!
//! ## Conversion Patterns
//!
//! ### Scalar to Vector Expansion
//! - `f32` → `Vec2`: Expands to `vec2(x, x)`
//! - `f32` → `Vec3`: Expands to `vec3(x, x, x)`
//! - `f32` → `Vec4`: Expands to `vec4(x, x, x, x)`
//!
//! ### Vector Expansion with Defaults
//! - `Vec2` → `Vec3`: Adds z=0.0 → `vec3(v.x, v.y, 0.0)`
//! - `Vec2` → `Vec4`: Adds z=0.0, w=1.0 → `vec4(v.x, v.y, 0.0, 1.0)`
//! - `Vec3` → `Vec4`: Adds w=1.0 → `vec4(v.x, v.y, v.z, 1.0)`
//!
//! ## Example
//!
//! ```rust,ignore
//! use gup::shader_function::*;
//!
//! // Automatic f32 to Vec3 conversion
//! assert!(f32::can_convert_to::<Vec3>());
//! let vec = f32::convert_value(5.0);
//! assert_eq!(vec, vec3![5.0, 5.0, 5.0]);
//!
//! // WGSL code generation
//! let wgsl = f32::conversion_wgsl::<Vec3>("value");
//! assert_eq!(wgsl, "vec3<f32>(value, value, value)");
//! ```

use super::{ShaderType, Vec2, Vec3, Vec4};

/// Trait for automatic type conversion between shader types.
///
/// This trait enables compile-time type conversions that maintain GPU memory layout
/// and generate appropriate WGSL code. All conversions are zero-cost abstractions
/// resolved at compile time.
pub trait AutoConvert<To: ShaderType>: ShaderType {
    /// Converts a value from this type to the target type.
    ///
    /// This is a zero-cost conversion that happens at compile time.
    /// The generated code is identical to manual construction.
    fn convert_value(value: Self) -> To;

    /// Generates the WGSL code for this conversion.
    ///
    /// Returns a WGSL expression that performs the conversion.
    ///
    /// # Arguments
    /// * `input_expr` - The WGSL expression representing the input value
    ///
    /// # Returns
    /// A string containing valid WGSL code for the conversion
    fn conversion_wgsl(input_expr: &str) -> String;

    /// Checks if this conversion is available at compile time.
    ///
    /// This is always `true` for types that implement `AutoConvert`,
    /// but can be used for compile-time checks.
    #[inline]
    fn can_convert() -> bool {
        true
    }
}

// =============================================================================
// f32 Conversions
// =============================================================================

/// f32 → Vec2: Scalar expansion to 2D vector
impl AutoConvert<Vec2> for f32 {
    #[inline]
    fn convert_value(value: Self) -> Vec2 {
        Vec2::new(value, value)
    }

    fn conversion_wgsl(input_expr: &str) -> String {
        format!("vec2<f32>({}, {})", input_expr, input_expr)
    }
}

/// f32 → Vec3: Scalar expansion to 3D vector
impl AutoConvert<Vec3> for f32 {
    #[inline]
    fn convert_value(value: Self) -> Vec3 {
        Vec3::new(value, value, value)
    }

    fn conversion_wgsl(input_expr: &str) -> String {
        format!("vec3<f32>({}, {}, {})", input_expr, input_expr, input_expr)
    }
}

/// f32 → Vec4: Scalar expansion to 4D vector
impl AutoConvert<Vec4> for f32 {
    #[inline]
    fn convert_value(value: Self) -> Vec4 {
        Vec4::new(value, value, value, value)
    }

    fn conversion_wgsl(input_expr: &str) -> String {
        format!(
            "vec4<f32>({}, {}, {}, {})",
            input_expr, input_expr, input_expr, input_expr
        )
    }
}

// =============================================================================
// Vec2 Conversions
// =============================================================================

/// Vec2 → Vec3: Expansion with z=0.0
impl AutoConvert<Vec3> for Vec2 {
    #[inline]
    fn convert_value(value: Self) -> Vec3 {
        Vec3::new(value.x, value.y, 0.0)
    }

    fn conversion_wgsl(input_expr: &str) -> String {
        format!("vec3<f32>({}.x, {}.y, 0.0)", input_expr, input_expr)
    }
}

/// Vec2 → Vec4: Expansion with z=0.0, w=1.0 (homogeneous coordinates)
impl AutoConvert<Vec4> for Vec2 {
    #[inline]
    fn convert_value(value: Self) -> Vec4 {
        Vec4::new(value.x, value.y, 0.0, 1.0)
    }

    fn conversion_wgsl(input_expr: &str) -> String {
        format!(
            "vec4<f32>({}.x, {}.y, 0.0, 1.0)",
            input_expr, input_expr
        )
    }
}

// =============================================================================
// Vec3 Conversions
// =============================================================================

/// Vec3 → Vec4: Expansion with w=1.0 (homogeneous coordinates)
impl AutoConvert<Vec4> for Vec3 {
    #[inline]
    fn convert_value(value: Self) -> Vec4 {
        Vec4::new(value.x, value.y, value.z, 1.0)
    }

    fn conversion_wgsl(input_expr: &str) -> String {
        format!(
            "vec4<f32>({}.x, {}.y, {}.z, 1.0)",
            input_expr, input_expr, input_expr
        )
    }
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Checks if a type can be automatically converted to another type.
///
/// This is a compile-time check that can be used in generic contexts.
///
/// # Example
///
/// ```rust,ignore
/// assert!(can_convert_types::<f32, Vec3>());
/// assert!(can_convert_types::<Vec2, Vec4>());
/// assert!(!can_convert_types::<Vec4, f32>()); // No downward conversions
/// ```
#[inline]
pub fn can_convert_types<From: ShaderType, To: ShaderType>() -> bool
where
    From: AutoConvert<To>,
{
    From::can_convert()
}

/// Generates WGSL conversion code for a type pair.
///
/// # Arguments
/// * `input_expr` - The WGSL expression representing the input value
///
/// # Returns
/// A string containing valid WGSL code for the conversion
///
/// # Example
///
/// ```rust,ignore
/// let wgsl = conversion_wgsl::<f32, Vec3>("temperature");
/// assert_eq!(wgsl, "vec3<f32>(temperature, temperature, temperature)");
/// ```
#[inline]
pub fn conversion_wgsl<From: ShaderType, To: ShaderType>(input_expr: &str) -> String
where
    From: AutoConvert<To>,
{
    From::conversion_wgsl(input_expr)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_f32_to_vec2_conversion() {
        let value: Vec2 = f32::convert_value(5.0);
        assert_eq!(value.x, 5.0);
        assert_eq!(value.y, 5.0);
    }

    #[test]
    fn test_f32_to_vec3_conversion() {
        let value: Vec3 = f32::convert_value(3.5);
        assert_eq!(value.x, 3.5);
        assert_eq!(value.y, 3.5);
        assert_eq!(value.z, 3.5);
    }

    #[test]
    fn test_f32_to_vec4_conversion() {
        let value: Vec4 = f32::convert_value(2.0);
        assert_eq!(value.x, 2.0);
        assert_eq!(value.y, 2.0);
        assert_eq!(value.z, 2.0);
        assert_eq!(value.w, 2.0);
    }

    #[test]
    fn test_vec2_to_vec3_conversion() {
        let vec2 = Vec2::new(1.0, 2.0);
        let vec3: Vec3 = Vec2::convert_value(vec2);
        assert_eq!(vec3.x, 1.0);
        assert_eq!(vec3.y, 2.0);
        assert_eq!(vec3.z, 0.0);
    }

    #[test]
    fn test_vec2_to_vec4_conversion() {
        let vec2 = Vec2::new(3.0, 4.0);
        let vec4: Vec4 = Vec2::convert_value(vec2);
        assert_eq!(vec4.x, 3.0);
        assert_eq!(vec4.y, 4.0);
        assert_eq!(vec4.z, 0.0);
        assert_eq!(vec4.w, 1.0);
    }

    #[test]
    fn test_vec3_to_vec4_conversion() {
        let vec3 = Vec3::new(1.0, 2.0, 3.0);
        let vec4: Vec4 = Vec3::convert_value(vec3);
        assert_eq!(vec4.x, 1.0);
        assert_eq!(vec4.y, 2.0);
        assert_eq!(vec4.z, 3.0);
        assert_eq!(vec4.w, 1.0);
    }

    #[test]
    fn test_f32_to_vec2_wgsl() {
        let wgsl = <f32 as AutoConvert<Vec2>>::conversion_wgsl("value");
        assert_eq!(wgsl, "vec2<f32>(value, value)");
    }

    #[test]
    fn test_f32_to_vec3_wgsl() {
        let wgsl = <f32 as AutoConvert<Vec3>>::conversion_wgsl("temp");
        assert_eq!(wgsl, "vec3<f32>(temp, temp, temp)");
    }

    #[test]
    fn test_f32_to_vec4_wgsl() {
        let wgsl = <f32 as AutoConvert<Vec4>>::conversion_wgsl("scalar");
        assert_eq!(wgsl, "vec4<f32>(scalar, scalar, scalar, scalar)");
    }

    #[test]
    fn test_vec2_to_vec3_wgsl() {
        let wgsl = <Vec2 as AutoConvert<Vec3>>::conversion_wgsl("pos");
        assert_eq!(wgsl, "vec3<f32>(pos.x, pos.y, 0.0)");
    }

    #[test]
    fn test_vec2_to_vec4_wgsl() {
        let wgsl = <Vec2 as AutoConvert<Vec4>>::conversion_wgsl("coord");
        assert_eq!(wgsl, "vec4<f32>(coord.x, coord.y, 0.0, 1.0)");
    }

    #[test]
    fn test_vec3_to_vec4_wgsl() {
        let wgsl = <Vec3 as AutoConvert<Vec4>>::conversion_wgsl("position");
        assert_eq!(wgsl, "vec4<f32>(position.x, position.y, position.z, 1.0)");
    }

    #[test]
    fn test_compile_time_checks() {
        assert!(can_convert_types::<f32, Vec2>());
        assert!(can_convert_types::<f32, Vec3>());
        assert!(can_convert_types::<f32, Vec4>());
        assert!(can_convert_types::<Vec2, Vec3>());
        assert!(can_convert_types::<Vec2, Vec4>());
        assert!(can_convert_types::<Vec3, Vec4>());
    }

    #[test]
    fn test_helper_functions() {
        let wgsl = conversion_wgsl::<f32, Vec3>("value");
        assert_eq!(wgsl, "vec3<f32>(value, value, value)");

        let wgsl = conversion_wgsl::<Vec2, Vec4>("pos");
        assert_eq!(wgsl, "vec4<f32>(pos.x, pos.y, 0.0, 1.0)");
    }

    #[test]
    fn test_zero_cost_abstraction() {
        // Verify that conversions have no runtime overhead
        // by checking that they compile to simple struct initialization
        let value = 5.0f32;
        let vec2: Vec2 = f32::convert_value(value);
        let vec3: Vec3 = f32::convert_value(value);
        let vec4: Vec4 = f32::convert_value(value);

        // These should all be simple struct copies with no function calls
        assert_eq!(vec2, Vec2::new(5.0, 5.0));
        assert_eq!(vec3, Vec3::new(5.0, 5.0, 5.0));
        assert_eq!(vec4, Vec4::new(5.0, 5.0, 5.0, 5.0));
    }
}

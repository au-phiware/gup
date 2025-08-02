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

//! WGSL Code Generation Macros
//!
//! This module provides macros for generating WGSL shader functions at compile time.
//! The macro system ensures type safety and generates optimized WGSL code.

/// Generate a shader function with automatic WGSL code generation.
///
/// This macro creates a struct implementing `ComposableShaderFunction` and generates
/// the corresponding WGSL code at compile time.
///
/// # Syntax
///
/// ```rust
/// wgsl_function! {
///     struct FunctionName {
///         // Fields for the shader function struct
///         field1: Type1,
///         field2: Type2,
///     }
///
///     uniforms UniformsName {
///         // Uniform fields with proper WGSL alignment
///         uniform_field1: f32,
///         uniform_field2: [f32; 4],
///     }
///
///     fn function_name(input: InputType, uniforms: UniformsName) -> OutputType {
///         // WGSL function body
///         // This will be used as-is in the generated WGSL
///         return result;
///     }
/// }
/// ```
///
/// # Example
///
/// ```rust
/// wgsl_function! {
///     struct LinearScale {
///         domain_min: f32,
///         domain_max: f32,
///         range_min: f32,
///         range_max: f32,
///     }
///
///     uniforms LinearScaleUniforms {
///         domain_min: f32,
///         domain_max: f32,
///         range_min: f32,
///         range_max: f32,
///     }
///
///     fn linear_scale(f32) -> f32,
///
///     wgsl {
///         "fn linear_scale(value: f32, scale: LinearScaleUniforms) -> f32 {\n    let normalized = (value - scale.domain_min) / (scale.domain_max - scale.domain_min);\n    return scale.range_min + normalized * (scale.range_max - scale.range_min);\n}"
///     }
/// }
/// ```
///
/// This generates:
/// - A `LinearScale` struct with the specified fields
/// - A `LinearScaleUniforms` struct implementing `bytemuck::Pod + bytemuck::Zeroable`
/// - An implementation of `ComposableShaderFunction` for `LinearScale`
/// - WGSL code that can be compiled and run on the GPU
/// - Automatic type checking and composition support
#[macro_export]
macro_rules! wgsl_function {
    (
        struct $struct_name:ident {
            $($field_name:ident: $field_type:ty),* $(,)?
        }

        uniforms $uniforms_name:ident {
            $($uniform_field:ident: $uniform_type:ty),* $(,)?
        }

        fn $fn_name:ident($input_type:ty) -> $output_type:ty,

        wgsl {
            $wgsl_code:literal
        }
    ) => {
        // Generate the uniform structure
        #[repr(C)]
        #[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
        pub struct $uniforms_name {
            $(pub $uniform_field: $uniform_type),*
        }

        // Generate the main struct
        #[derive(Debug, Clone)]
        pub struct $struct_name {
            $(pub $field_name: $field_type),*
        }

        impl $struct_name {
            pub fn new($($field_name: $field_type),*) -> Self {
                Self {
                    $($field_name),*
                }
            }
        }

        impl $crate::shader_function::ComposableShaderFunction for $struct_name {
            type Input = $input_type;
            type Output = $output_type;
            type Uniforms = $uniforms_name;

            fn wgsl_function() -> &'static str {
                $wgsl_code
            }

            fn create_uniforms(&self) -> Option<Self::Uniforms> {
                Some($uniforms_name {
                    $($uniform_field: self.$field_name),*
                })
            }

            fn function_name() -> &'static str {
                stringify!($fn_name)
            }
        }
    };
}

/// Generate a function composition with optimized WGSL code.
///
/// This macro generates WGSL code for chaining two shader functions together,
/// creating an optimized single function that performs both operations.
#[macro_export]
macro_rules! wgsl_compose_functions {
    ($first_fn:ident, $second_fn:ident, $chain_name:ident) => {
        paste::paste! {
            concat!(
                "fn ", stringify!($chain_name), "(input: ",
                <A::Input as $crate::shader_function::ShaderType>::wgsl_type_name(),
                ", uniforms: ChainUniforms) -> ",
                <B::Output as $crate::shader_function::ShaderType>::wgsl_type_name(), " {\n",
                "    let intermediate = ", stringify!($first_fn), "(input, uniforms.first);\n",
                "    return ", stringify!($second_fn), "(intermediate, uniforms.second);\n",
                "}"
            )
        }
    };
}

/// Helper macro for generating uniform struct definitions in WGSL
#[macro_export]
macro_rules! wgsl_uniform_struct {
    ($name:ident { $($field:ident: $ty:ty),* $(,)? }) => {
        concat!(
            "struct ", stringify!($name), " {\n",
            $(
                "    ", stringify!($field), ": ",
                <$ty as $crate::shader_function::ShaderType>::wgsl_type_name(), ",\n"
            ),*
            "}"
        )
    };
}

// Helper trait for automatic code generation context
pub trait WgslCodeGenerator {
    fn generate_function_definition(&self) -> String;
    fn generate_uniform_struct(&self) -> String;
    fn generate_composed_function(&self, other_name: &str) -> String;
}

#[cfg(test)]
mod tests {
    use crate::shader_function::*;

    wgsl_function! {
        struct TestScale {
            min: f32,
            max: f32,
        }

        uniforms TestScaleUniforms {
            min: f32,
            max: f32,
        }

        fn test_scale(f32) -> f32,

        wgsl {
            "fn test_scale(value: f32, scale: TestScaleUniforms) -> f32 {\n    return value * scale.max + scale.min;\n}"
        }
    }

    #[test]
    fn test_macro_generates_struct() {
        let scale = TestScale::new(0.0, 10.0);
        assert_eq!(scale.min, 0.0);
        assert_eq!(scale.max, 10.0);
    }

    #[test]
    fn test_macro_generates_uniforms() {
        let scale = TestScale::new(1.0, 5.0);
        let uniforms = scale.create_uniforms().unwrap();
        assert_eq!(uniforms.min, 1.0);
        assert_eq!(uniforms.max, 5.0);
    }

    #[test]
    fn test_macro_generates_wgsl() {
        let wgsl = TestScale::wgsl_function();
        assert!(wgsl.contains("fn test_scale"));
        assert!(wgsl.contains("value: f32"));
        assert!(wgsl.contains("scale: TestScaleUniforms"));
        assert!(wgsl.contains("-> f32"));
    }

    #[test]
    fn test_function_name() {
        assert_eq!(TestScale::function_name(), "test_scale");
    }
}

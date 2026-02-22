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

//! Integration tests for the `#[wgsl_function]` procedural macro
//!
//! These tests verify that the macro generates working code that can be compiled
//! and used with the shader function system.

use gup::*;
use gup_macros::wgsl_function;

#[wgsl_function]
fn test_linear_scale(value: f32, scale: f32, offset: f32) -> f32 {
    return value * scale + offset;
}

#[wgsl_function]
fn test_vector_transform(pos: Vec2, scale: Vec2) -> Vec2 {
    return pos * scale;
}

#[wgsl_function]
fn test_color_blend(color: Vec4, tint: Vec4, intensity: f32) -> Vec4 {
    return color * tint * intensity;
}

#[wgsl_function]
fn test_identity(value: f32) -> f32 {
    return value;
}

#[tokio::test]
async fn test_macro_generated_linear_scale() {
    let scale_func = TestLinearScale::new(2.0, 1.0);

    // Test struct creation
    assert_eq!(scale_func.scale, 2.0);
    assert_eq!(scale_func.offset, 1.0);

    // Test trait implementation
    assert_eq!(TestLinearScale::function_name(), "test_linear_scale");
    assert!(TestLinearScale::wgsl_function().contains("test_linear_scale"));
    assert!(TestLinearScale::wgsl_function().contains("f32"));

    // Test uniform creation
    let uniforms = scale_func.create_uniforms().unwrap();
    assert_eq!(uniforms.scale, 2.0);
    assert_eq!(uniforms.offset, 1.0);
}

#[tokio::test]
async fn test_macro_generated_vector_transform() {
    let scale = vec2![2.0, 3.0];
    let transform = TestVectorTransform::new(scale);

    // Test struct creation
    assert_eq!(transform.scale.x, 2.0);
    assert_eq!(transform.scale.y, 3.0);

    // Test trait implementation
    assert_eq!(
        TestVectorTransform::function_name(),
        "test_vector_transform"
    );
    assert!(TestVectorTransform::wgsl_function().contains("test_vector_transform"));
    assert!(TestVectorTransform::wgsl_function().contains("vec2<f32>"));

    // Test uniform creation
    let uniforms = transform.create_uniforms().unwrap();
    assert_eq!(uniforms.scale, [2.0, 3.0]);
}

#[tokio::test]
async fn test_macro_generated_color_blend() {
    let tint = vec4![1.0, 0.5, 0.8, 1.0];
    let blend = TestColorBlend::new(tint, 0.75);

    // Test struct creation
    assert_eq!(blend.tint.x, 1.0);
    assert_eq!(blend.tint.y, 0.5);
    assert_eq!(blend.tint.z, 0.8);
    assert_eq!(blend.tint.w, 1.0);
    assert_eq!(blend.intensity, 0.75);

    // Test trait implementation
    assert_eq!(TestColorBlend::function_name(), "test_color_blend");
    assert!(TestColorBlend::wgsl_function().contains("test_color_blend"));
    assert!(TestColorBlend::wgsl_function().contains("vec4<f32>"));

    // Test uniform creation
    let uniforms = blend.create_uniforms().unwrap();
    assert_eq!(uniforms.tint, [1.0, 0.5, 0.8, 1.0]);
    assert_eq!(uniforms.intensity, 0.75);
}

#[tokio::test]
async fn test_macro_generated_identity() {
    let identity = TestIdentity::new();

    // Test trait implementation for function with no uniforms
    assert_eq!(TestIdentity::function_name(), "test_identity");
    assert!(TestIdentity::wgsl_function().contains("test_identity"));
    assert!(TestIdentity::wgsl_function().contains("f32"));

    // Test uniform creation (should return unit struct)
    let _uniforms = identity.create_uniforms().unwrap();
    // Uniforms is a unit struct, so we can't test its contents
    // but we can verify it was created successfully
}

#[tokio::test]
async fn test_function_composition() {
    let scale = TestLinearScale::new(2.0, 0.0);
    let transform = TestVectorTransform::new(vec2![1.0, 1.0]);

    // Test that generated functions can be composed
    // Note: This requires that f32 is compatible with Vec2's input
    // which should work through the TypeCompatible trait

    // Verify the functions can be created and have the right types
    let _scale_uniforms = scale.create_uniforms().unwrap();
    let _transform_uniforms = transform.create_uniforms().unwrap();

    // Test WGSL code generation
    let scale_wgsl = TestLinearScale::wgsl_function();
    let transform_wgsl = TestVectorTransform::wgsl_function();

    assert!(!scale_wgsl.is_empty());
    assert!(!transform_wgsl.is_empty());
    assert!(scale_wgsl.contains("test_linear_scale"));
    assert!(transform_wgsl.contains("test_vector_transform"));
}

#[test]
fn test_generated_uniform_structs_are_pod() {
    // Test that generated uniform structs implement the required traits
    let scale_uniforms = TestLinearScaleUniforms {
        scale: 2.0,
        offset: 1.0,
    };

    let transform_uniforms = TestVectorTransformUniforms { scale: [2.0, 3.0] };

    let blend_uniforms = TestColorBlendUniforms {
        tint: [1.0, 0.5, 0.8, 1.0],
        intensity: 0.75,
    };

    // These operations verify that the types implement Pod and Zeroable
    let _scale_bytes = bytemuck::bytes_of(&scale_uniforms);
    let _transform_bytes = bytemuck::bytes_of(&transform_uniforms);
    let _blend_bytes = bytemuck::bytes_of(&blend_uniforms);

    // Test that we can create zeroed instances
    let _zero_scale: TestLinearScaleUniforms = bytemuck::Zeroable::zeroed();
    let _zero_transform: TestVectorTransformUniforms = bytemuck::Zeroable::zeroed();
    let _zero_blend: TestColorBlendUniforms = bytemuck::Zeroable::zeroed();
}

#[test]
fn test_generated_structs_implement_debug_clone() {
    let scale = TestLinearScale::new(2.0, 1.0);
    let cloned_scale = scale.clone();

    assert_eq!(scale.scale, cloned_scale.scale);
    assert_eq!(scale.offset, cloned_scale.offset);

    // Test Debug implementation
    let debug_string = format!("{scale:?}");
    assert!(debug_string.contains("TestLinearScale"));
    assert!(debug_string.contains("2.0"));
    assert!(debug_string.contains("1.0"));
}

// Test with GPU context to ensure WGSL compiles correctly
#[tokio::test]
async fn test_wgsl_compilation() {
    // Create a GPU context for testing
    let context = match GupContext::headless().await {
        Ok(ctx) => ctx,
        Err(_) => {
            // Skip test if no GPU is available
            println!("Skipping GPU test - no device available");
            return;
        }
    };

    // Test that the generated WGSL can be compiled
    let scale_wgsl = TestLinearScale::wgsl_function();
    let transform_wgsl = TestVectorTransform::wgsl_function();

    // Create a simple shader module to test compilation
    let full_shader = format!(
        r#"
        {scale_wgsl}

        {transform_wgsl}

        @vertex
        fn vs_main() -> @builtin(position) vec4<f32> {{
            return vec4<f32>(0.0, 0.0, 0.0, 1.0);
        }}

        @fragment
        fn fs_main() -> @location(0) vec4<f32> {{
            return vec4<f32>(1.0, 0.0, 0.0, 1.0);
        }}
        "#
    );

    // Try to create a shader module - this will fail if WGSL syntax is invalid
    let shader_result = context
        .device
        .create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("test_generated_wgsl"),
            source: wgpu::ShaderSource::Wgsl(full_shader.into()),
        });

    // If we reach this point, the WGSL compiled successfully
    drop(shader_result);
}

#[wgsl_function]
fn test_vec_constructor(x: f32, scale: f32) -> Vec2 {
    let scaled_x = x * scale;
    let scaled_y = x * scale;
    return Vec2(scaled_x, scaled_y);
}

#[wgsl_function]
fn test_clamp_function(value: f32, min_val: f32, max_val: f32) -> f32 {
    return clamp(value, min_val, max_val);
}

#[tokio::test]
async fn test_vector_constructor_translation() {
    let func = TestVecConstructor::new(2.0);

    // Test that generated WGSL includes Vec2 constructor as vec2<f32>
    let wgsl = TestVecConstructor::wgsl_function();
    println!("Generated WGSL:\n{}", wgsl);
    assert!(
        wgsl.contains("vec2<f32>"),
        "WGSL should contain vec2<f32> constructor"
    );
    assert!(
        wgsl.contains("uniforms.scale"),
        "WGSL should reference uniforms.scale"
    );

    // Test uniform creation
    let uniforms = func.create_uniforms().unwrap();
    assert_eq!(uniforms.scale, 2.0);
}

#[tokio::test]
async fn test_wgsl_builtin_functions() {
    let _func = TestClampFunction::new(0.0, 1.0);

    // Test that clamp function is preserved in WGSL
    let wgsl = TestClampFunction::wgsl_function();
    assert!(wgsl.contains("clamp"), "WGSL should contain clamp function");
    assert!(
        wgsl.contains("uniforms.min_val"),
        "WGSL should reference uniforms.min_val"
    );
    assert!(
        wgsl.contains("uniforms.max_val"),
        "WGSL should reference uniforms.max_val"
    );

    // Verify GPU compilation
    let context = match GupContext::headless().await {
        Ok(ctx) => ctx,
        Err(_) => {
            println!("Skipping GPU test - no device available");
            return;
        }
    };

    let full_shader = format!(
        r#"
        {}

        @vertex
        fn vs_main() -> @builtin(position) vec4<f32> {{
            return vec4<f32>(0.0, 0.0, 0.0, 1.0);
        }}

        @fragment
        fn fs_main() -> @location(0) vec4<f32> {{
            return vec4<f32>(1.0, 0.0, 0.0, 1.0);
        }}
        "#,
        wgsl
    );

    let shader_result = context
        .device
        .create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("test_clamp_wgsl"),
            source: wgpu::ShaderSource::Wgsl(full_shader.into()),
        });

    drop(shader_result);
}

// Test matrix types
#[wgsl_function]
fn test_matrix_transform(pos: Vec2, transform: Mat3) -> Vec2 {
    let homogeneous = Vec3(pos.x, pos.y, 1.0);
    let transformed = transform * homogeneous;
    return Vec2(transformed.x, transformed.y);
}

#[tokio::test]
async fn test_matrix_type_support() {
    // Create identity matrix for testing
    let identity_mat3 = Mat3::identity();
    let transform = TestMatrixTransform::new(identity_mat3);

    // Test that generated WGSL includes matrix type
    let wgsl = TestMatrixTransform::wgsl_function();
    println!("Generated matrix WGSL:\n{}", wgsl);
    assert!(
        wgsl.contains("mat3x3<f32>"),
        "WGSL should contain mat3x3<f32> type"
    );

    // Test uniform creation
    let uniforms = transform.create_uniforms().unwrap();
    // Verify the matrix was stored correctly
    assert_eq!(uniforms.transform.m00, 1.0);
    assert_eq!(uniforms.transform.m11, 1.0);
    assert_eq!(uniforms.transform.m22, 1.0);
}

#[tokio::test]
async fn test_matrix_gpu_compilation() {
    // Test that matrix functions compile on GPU
    let context = match GupContext::headless().await {
        Ok(ctx) => ctx,
        Err(_) => {
            println!("Skipping GPU test - no device available");
            return;
        }
    };

    let matrix_wgsl = TestMatrixTransform::wgsl_function();

    let full_shader = format!(
        r#"
        {matrix_wgsl}

        @vertex
        fn vs_main() -> @builtin(position) vec4<f32> {{
            return vec4<f32>(0.0, 0.0, 0.0, 1.0);
        }}

        @fragment
        fn fs_main() -> @location(0) vec4<f32> {{
            return vec4<f32>(1.0, 0.0, 0.0, 1.0);
        }}
        "#
    );

    // Try to create a shader module - this will fail if WGSL syntax is invalid
    let shader_result = context
        .device
        .create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("test_matrix_wgsl"),
            source: wgpu::ShaderSource::Wgsl(full_shader.into()),
        });

    drop(shader_result);
}




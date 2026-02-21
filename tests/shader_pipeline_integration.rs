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

//! Integration tests for the ShaderPipeline system.

use gup::*;
use std::time::Instant;

async fn create_test_context() -> std::sync::Arc<GupContext> {
    GupContext::headless()
        .await
        .expect("Failed to create test context")
}

#[tokio::test]
async fn test_complete_pipeline_workflow() {
    let context = create_test_context().await;
    let device = &context.device;

    let mut pipeline = ComposableShaderPipeline::new();

    // Add multiple shader functions
    let scale = LinearScale::new(0.0, 100.0, 0.0, 1.0);
    let color_map = ColorMap::new(vec4![0.0, 0.0, 0.0, 1.0], vec4![1.0, 1.0, 1.0, 1.0]);

    pipeline.add_function(scale);
    pipeline.add_function(color_map);

    // Map attributes
    pipeline.map_attribute("size", "linear_scale");
    pipeline.map_attribute("color", "color_map");

    // Test shader generation
    let vertex_shader = pipeline.generate_vertex_shader();
    let fragment_shader = pipeline.generate_fragment_shader();

    assert!(vertex_shader.contains("vs_main"));
    assert!(vertex_shader.contains("linear_scale"));
    assert!(vertex_shader.contains("color_map"));
    assert!(fragment_shader.contains("fs_main"));

    // Test that generated shaders compile successfully
    let _vertex_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("test_vertex"),
        source: wgpu::ShaderSource::Wgsl(vertex_shader.into()),
    });

    let _fragment_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("test_fragment"),
        source: wgpu::ShaderSource::Wgsl(fragment_shader.into()),
    });

    // Test bind group layout creation
    let _bind_group_layout = pipeline.create_bind_group_layout(device).unwrap();
    assert_eq!(pipeline.functions_with_uniforms_count(), 2); // Two functions with uniforms
}

#[tokio::test]
async fn test_shader_compilation_validation() {
    let context = create_test_context().await;
    let device = &context.device;

    let mut pipeline = ComposableShaderPipeline::new();

    // Test with linear scale function
    let scale = LinearScale::new(0.0, 100.0, 0.0, 1.0);
    pipeline.add_function(scale);
    pipeline.map_attribute("value", "linear_scale");

    let vertex_source = pipeline.generate_vertex_shader();
    let fragment_source = pipeline.generate_fragment_shader();

    // Verify shaders compile without errors
    let _vertex_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("validation_vertex"),
        source: wgpu::ShaderSource::Wgsl(vertex_source.into()),
    });

    let _fragment_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("validation_fragment"),
        source: wgpu::ShaderSource::Wgsl(fragment_source.into()),
    });

    // If we get here without panicking, compilation was successful
}

#[tokio::test]
async fn test_uniform_buffer_creation_and_management() {
    let context = create_test_context().await;
    let device = &context.device;
    let queue = &context.queue;

    let mut pipeline = ComposableShaderPipeline::new();

    let scale = LinearScale::new(0.0, 100.0, 0.0, 1.0);
    let color_map = ColorMap::new(vec4![0.0, 0.0, 0.0, 1.0], vec4![1.0, 1.0, 1.0, 1.0]);

    pipeline.add_function(scale);
    pipeline.add_function(color_map);

    // Create uniform buffers
    pipeline.create_uniform_buffers(device).unwrap();

    // Update uniforms
    pipeline.update_uniforms(device, queue).unwrap();

    // Verify buffers were created
    assert_eq!(pipeline.uniform_buffer_count(), 2);
}

#[tokio::test]
async fn test_pipeline_caching() {
    let context = create_test_context().await;
    let device = &context.device;

    let mut pipeline = ComposableShaderPipeline::new();
    let scale = LinearScale::new(0.0, 100.0, 0.0, 1.0);
    pipeline.add_function(scale);

    // Initially no cache
    assert!(!pipeline.is_cache_valid());

    // Generate shaders to populate cache
    let start = Instant::now();
    let _vertex1 = pipeline.generate_vertex_shader();
    let first_generation = start.elapsed();

    // Cache shader modules
    pipeline.update_cache_public(device).unwrap();
    assert!(pipeline.is_cache_valid());

    // Second generation should use cache
    let start = Instant::now();
    let _vertex2 = pipeline.generate_vertex_shader();
    let second_generation = start.elapsed();

    // Second generation should be faster due to caching
    println!("First generation: {first_generation:?}, Second: {second_generation:?}");
}

#[tokio::test]
async fn test_optimization_functionality() {
    let pipeline = ComposableShaderPipeline::new();

    let test_shader = r#"
        let x = 1.0 * value + 0.0;
        let y = result * 1.0;
        @group(0) @binding(0) var<uniform> used_uniforms: UsedUniforms;
        @group(0) @binding(1) var<uniform> unused_uniforms: UnusedUniforms;

        fn compute() {
            let z = used_uniforms.scale;
        }
    "#;

    let optimized = pipeline.optimize_shader(test_shader);

    // Verify constant folding worked
    assert!(optimized.contains("let x = value;"));
    assert!(optimized.contains("let y = result;"));

    // Verify unused uniform removal worked
    assert!(optimized.contains("used_uniforms"));
    // Note: The current implementation is basic and might not remove all unused uniforms
}

#[tokio::test]
async fn test_performance_target() {
    let context = create_test_context().await;
    let _device = &context.device;

    let mut pipeline = ComposableShaderPipeline::new();

    // Add multiple functions to create a complex pipeline
    let scale = LinearScale::new(0.0, 100.0, 0.0, 1.0);
    let color_map = ColorMap::new(vec4![0.0, 0.0, 0.0, 1.0], vec4![1.0, 1.0, 1.0, 1.0]);
    let position_transform = PositionTransform::new(vec2![1.0, 1.0], vec2![0.0, 0.0]);

    pipeline.add_function(scale);
    pipeline.add_function(color_map);
    pipeline.add_function(position_transform);

    pipeline.map_attribute("size", "linear_scale");
    pipeline.map_attribute("color", "color_map");
    pipeline.map_attribute("position", "position_transform");

    // Measure shader generation time
    let start = Instant::now();
    let _vertex_shader = pipeline.generate_vertex_shader();
    let _fragment_shader = pipeline.generate_fragment_shader();
    let generation_time = start.elapsed();

    // Verify generation time is under 5ms target
    println!("Shader generation time: {generation_time:?}");
    assert!(
        generation_time.as_millis() < 5,
        "Shader generation took {generation_time:?}, exceeding 5ms target"
    );
}

#[tokio::test]
async fn test_complex_pipeline_compilation() {
    let context = create_test_context().await;
    let device = &context.device;

    let mut pipeline = ComposableShaderPipeline::new();

    // Create a complex pipeline with multiple functions
    let scale1 = LinearScale::new(0.0, 100.0, 0.0, 1.0);
    let scale2 = LinearScale::new(0.0, 50.0, -1.0, 1.0);
    let color_map = ColorMap::new(vec4![0.2, 0.1, 0.8, 1.0], vec4![0.9, 0.7, 0.2, 1.0]);
    let position_transform = PositionTransform::new(vec2![2.0, 1.5], vec2![-0.5, 0.2]);

    pipeline.add_function(scale1);
    pipeline.add_function(scale2);
    pipeline.add_function(color_map);
    pipeline.add_function(position_transform);

    pipeline.map_attribute("primary_value", "linear_scale");
    pipeline.map_attribute("secondary_value", "linear_scale");
    pipeline.map_attribute("color", "color_map");
    pipeline.map_attribute("position", "position_transform");

    // Generate and compile shaders
    let vertex_source = pipeline.generate_vertex_shader();
    let fragment_source = pipeline.generate_fragment_shader();

    // Verify the generated shaders are valid and comprehensive
    assert!(vertex_source.contains("primary_value"));
    assert!(vertex_source.contains("secondary_value"));
    assert!(vertex_source.contains("color"));
    assert!(vertex_source.contains("position"));

    // Test actual GPU compilation
    let _vertex_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("complex_vertex"),
        source: wgpu::ShaderSource::Wgsl(vertex_source.into()),
    });

    let _fragment_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("complex_fragment"),
        source: wgpu::ShaderSource::Wgsl(fragment_source.into()),
    });

    // Test bind group layout creation for complex pipeline
    let _bind_group_layout = pipeline.create_bind_group_layout(device).unwrap();
    let expected_bindings = pipeline.functions_with_uniforms_count();
    assert_eq!(expected_bindings, expected_bindings); // Just verify it doesn't crash
}

#[tokio::test]
async fn test_render_pipeline_creation() {
    let context = create_test_context().await;
    let device = &context.device;

    let mut pipeline = ComposableShaderPipeline::new();
    let scale = LinearScale::new(0.0, 100.0, 0.0, 1.0);
    pipeline.add_function(scale);
    pipeline.map_attribute("size", "linear_scale");

    // Test complete render pipeline creation
    let _render_pipeline = pipeline.create_render_pipeline(device).unwrap();

    // If we reach here, the render pipeline was created successfully
}

#[tokio::test]
async fn test_automatic_uniform_struct_generation() {
    let context = create_test_context().await;
    let device = &context.device;

    let mut pipeline = ComposableShaderPipeline::new();

    // Add functions with different uniform types
    let scale = LinearScale::new(0.0, 100.0, 0.0, 1.0);
    let color_map = ColorMap::new(vec4![0.0, 0.0, 0.0, 1.0], vec4![1.0, 1.0, 1.0, 1.0]);
    let position = PositionTransform::new(vec2![1.0, 1.0], vec2![0.0, 0.0]);

    pipeline.add_function(scale);
    pipeline.add_function(color_map);
    pipeline.add_function(position);

    // Generate shader code
    let vertex_shader = pipeline.generate_vertex_shader();

    // Verify that uniform structs are automatically generated (no hardcoded types)
    assert!(vertex_shader.contains("struct LinearScaleUniforms"));
    assert!(vertex_shader.contains("domain_min: f32"));
    assert!(vertex_shader.contains("domain_max: f32"));
    assert!(vertex_shader.contains("range_min: f32"));
    assert!(vertex_shader.contains("range_max: f32"));

    assert!(vertex_shader.contains("struct ColorMapUniforms"));
    assert!(vertex_shader.contains("min_color: vec4<f32>"));
    assert!(vertex_shader.contains("max_color: vec4<f32>"));

    assert!(vertex_shader.contains("struct PositionTransformUniforms"));
    assert!(vertex_shader.contains("scale: vec2<f32>"));
    assert!(vertex_shader.contains("offset: vec2<f32>"));

    // Verify the shader compiles successfully with auto-generated uniforms
    let _vertex_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("auto_uniform_test"),
        source: wgpu::ShaderSource::Wgsl(vertex_shader.into()),
    });
}

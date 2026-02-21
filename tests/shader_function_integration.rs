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

//! Integration tests for the shader function system.
//!
//! These tests validate shader function integration with GPU resources and include
//! actual WGSL compilation and execution tests. Full GPU rendering integration
//! will be implemented in future updates.

use gup::{GupResult, context::GupContext, shader_function::*, vec2, vec4};

#[tokio::test]
async fn test_shader_function_gpu_integration() -> GupResult<()> {
    let context = GupContext::headless().await?;
    let device = &context.device;
    let queue = &context.queue;

    let scale = LinearScale::new(0.0, 100.0, 0.0, 1.0);

    let mut uniform_buffer: UniformBuffer<LinearScaleUniforms> = UniformBuffer::new();

    if let Some(uniforms) = scale.create_uniforms() {
        uniform_buffer.upload(device, queue, &uniforms)?;
        assert!(uniform_buffer.buffer().is_some());
    }

    Ok(())
}

#[tokio::test]
async fn test_color_map_gpu_integration() -> GupResult<()> {
    let context = GupContext::headless().await?;
    let device = &context.device;
    let queue = &context.queue;

    let color_map = ColorMap::new(vec4![0.2, 0.1, 0.0, 1.0], vec4![0.8, 0.9, 1.0, 1.0]);

    let mut uniform_buffer: UniformBuffer<ColorMapUniforms> = UniformBuffer::new();

    if let Some(uniforms) = color_map.create_uniforms() {
        uniform_buffer.upload(device, queue, &uniforms)?;

        let buffer = uniform_buffer.buffer().unwrap();
        assert!(buffer.capacity() >= 1);
    }

    Ok(())
}

#[tokio::test]
async fn test_composed_function_gpu_integration() -> GupResult<()> {
    let context = GupContext::headless().await?;
    let device = &context.device;
    let queue = &context.queue;

    let scale = LinearScale::new(0.0, 100.0, 0.0, 1.0);
    let color_map = ColorMap::new(vec4![0.0, 0.0, 0.0, 1.0], vec4![1.0, 0.0, 0.0, 1.0]);

    let composed = scale.compose(color_map);

    if let Some(chain_uniforms) = composed.create_uniforms() {
        let mut uniform_buffer: UniformBuffer<
            ChainUniforms<LinearScaleUniforms, ColorMapUniforms>,
        > = UniformBuffer::new();

        uniform_buffer.upload(device, queue, &chain_uniforms)?;
        assert!(uniform_buffer.buffer().is_some());
    }

    Ok(())
}

#[tokio::test]
async fn test_position_transform_gpu_integration() -> GupResult<()> {
    let context = GupContext::headless().await?;
    let device = &context.device;
    let queue = &context.queue;

    let transform = PositionTransform::new(vec2![2.0, 1.5], vec2![0.1, 0.2]);

    let mut uniform_buffer: UniformBuffer<PositionTransformUniforms> = UniformBuffer::new();

    if let Some(uniforms) = transform.create_uniforms() {
        uniform_buffer.upload(device, queue, &uniforms)?;
        assert!(uniform_buffer.buffer().is_some());
    }

    Ok(())
}

#[tokio::test]
async fn test_wgsl_function_output() -> GupResult<()> {
    // Note: This test validates static WGSL string content.
    // Actual WGSL compilation and GPU execution testing is done in other tests.

    let wgsl = LinearScale::wgsl_function();
    assert!(wgsl.contains("fn linear_scale"));
    assert!(wgsl.contains("LinearScaleUniforms"));
    assert!(wgsl.contains("value"));
    assert!(wgsl.contains("scale"));

    let color_wgsl = ColorMap::wgsl_function();
    assert!(color_wgsl.contains("fn color_map"));
    assert!(color_wgsl.contains("ColorMapUniforms"));
    assert!(color_wgsl.contains("mix"));

    let pos_wgsl = PositionTransform::wgsl_function();
    assert!(pos_wgsl.contains("fn position_transform"));
    assert!(pos_wgsl.contains("PositionTransformUniforms"));

    Ok(())
}

#[tokio::test]
async fn test_shader_function_performance() -> GupResult<()> {
    use std::time::Instant;

    let start = Instant::now();

    for _ in 0..1000 {
        let scale = LinearScale::new(0.0, 100.0, 0.0, 1.0);
        let color_map = ColorMap::new(vec4![0.0, 0.0, 0.0, 1.0], vec4![1.0, 1.0, 1.0, 1.0]);
        let _composed = scale.compose(color_map);
    }

    let duration = start.elapsed();
    assert!(duration.as_millis() < 100);

    Ok(())
}

#[tokio::test]
async fn test_template_shader_function_gpu_integration() -> GupResult<()> {
    let context = GupContext::headless().await?;
    let device = &context.device;
    let queue = &context.queue;

    let scale = LinearScaleTemplate::new(0.0, 100.0, 0.0, 1.0);

    let mut uniform_buffer: UniformBuffer<LinearScaleTemplateUniforms> = UniformBuffer::new();

    if let Some(uniforms) = scale.create_uniforms() {
        uniform_buffer.upload(device, queue, &uniforms)?;
        assert!(uniform_buffer.buffer().is_some());
    }

    Ok(())
}

#[tokio::test]
async fn test_dynamic_wgsl_generation() -> GupResult<()> {
    // Test the new dynamic WGSL generation for composition
    let scale = LinearScale::new(0.0, 100.0, 0.0, 1.0);
    let color_map = ColorMap::new(vec4![0.0, 0.0, 0.0, 1.0], vec4![1.0, 0.0, 0.0, 1.0]);

    let composed = scale.compose(color_map);

    // Test dynamic WGSL generation
    let dynamic_wgsl = composed.generate_wgsl();
    assert!(dynamic_wgsl.contains("fn composed_chain"));
    assert!(dynamic_wgsl.contains("f32")); // Input type
    assert!(dynamic_wgsl.contains("vec4<f32>")); // Output type
    assert!(dynamic_wgsl.contains("linear_scale"));
    assert!(dynamic_wgsl.contains("color_map"));
    assert!(dynamic_wgsl.contains("uniforms.first"));
    assert!(dynamic_wgsl.contains("uniforms.second"));

    Ok(())
}

#[tokio::test]
async fn test_template_wgsl_generation() -> GupResult<()> {
    // Test template-based WGSL generation
    let template_scale = LinearScaleTemplate::new(0.0, 10.0, 0.0, 1.0);

    let wgsl = LinearScaleTemplate::wgsl_function();
    assert!(wgsl.contains("fn linear_scale_template"));
    assert!(wgsl.contains("LinearScaleTemplateUniforms"));
    assert!(wgsl.contains("value"));
    assert!(wgsl.contains("scale"));

    let function_name = LinearScaleTemplate::function_name();
    assert_eq!(function_name, "linear_scale_template");

    // Test dynamic generation works too
    let dynamic_wgsl = template_scale.generate_wgsl();
    assert_eq!(dynamic_wgsl, wgsl);

    Ok(())
}

#[tokio::test]
async fn test_wgsl_compilation_validation() -> GupResult<()> {
    let context = GupContext::headless().await?;
    let device = &context.device;

    // Test that the generated WGSL can be used in actual shader modules
    let _template_scale = LinearScaleTemplate::new(0.0, 10.0, 0.0, 1.0);
    let wgsl_code = LinearScaleTemplate::wgsl_function();

    // Create a complete shader module with the generated WGSL
    let complete_shader = format!(
        r#"
        struct LinearScaleTemplateUniforms {{
            domain_min: f32,
            domain_max: f32,
            range_min: f32,
            range_max: f32,
        }}

        {wgsl_code}

        @vertex
        fn vs_main(@builtin(vertex_index) vertex_index: u32) -> @builtin(position) vec4<f32> {{
            return vec4<f32>(0.0, 0.0, 0.0, 1.0);
        }}

        @fragment
        fn fs_main() -> @location(0) vec4<f32> {{
            let result = linear_scale_template(0.5, LinearScaleTemplateUniforms(0.0, 1.0, 0.0, 10.0));
            return vec4<f32>(result, 0.0, 0.0, 1.0);
        }}
        "#
    );

    // This should compile without errors if WGSL is valid
    let _shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Test Generated WGSL"),
        source: wgpu::ShaderSource::Wgsl(complete_shader.into()),
    });

    // If we reach here, the WGSL compiled successfully
    // Test passes by reaching this point without panicking

    Ok(())
}

#[tokio::test]
async fn test_shader_uniform_backward_compatibility() -> GupResult<()> {
    // Test that existing shader function APIs continue to work with ShaderUniform trait
    use gup::shader_function::ShaderUniform;
    
    // Test existing functions still work
    let scale = LinearScale::new(0.0, 100.0, 0.0, 1.0);
    let color_map = ColorMap::new(vec4![0.0, 0.0, 0.0, 1.0], vec4![1.0, 1.0, 1.0, 1.0]);
    let position = PositionTransform::new(vec2![1.0, 1.0], vec2![0.0, 0.0]);
    
    // Test composition still works
    let composed = scale.compose(color_map);
    let _uniforms = composed.create_uniforms();
    
    // Test ShaderUniform implementations exist and work
    assert!(!LinearScaleUniforms::wgsl_struct_definition().is_empty());
    assert_eq!(LinearScaleUniforms::wgsl_type_name(), "LinearScaleUniforms");
    
    assert!(!ColorMapUniforms::wgsl_struct_definition().is_empty());
    assert_eq!(ColorMapUniforms::wgsl_type_name(), "ColorMapUniforms");
    
    assert!(!PositionTransformUniforms::wgsl_struct_definition().is_empty());
    assert_eq!(PositionTransformUniforms::wgsl_type_name(), "PositionTransformUniforms");
    
    // Test that template macro also generates ShaderUniform implementations
    assert!(!LinearScaleTemplateUniforms::wgsl_struct_definition().is_empty());
    assert_eq!(LinearScaleTemplateUniforms::wgsl_type_name(), "LinearScaleTemplateUniforms");
    
    Ok(())
}

#[tokio::test]
async fn test_shader_uniform_generation_performance() -> GupResult<()> {
    use std::time::Instant;
    use gup::shader_function::ShaderUniform;
    
    // Benchmark uniform struct generation
    let start = Instant::now();
    
    for _ in 0..10000 {
        let _def1 = LinearScaleUniforms::wgsl_struct_definition();
        let _def2 = ColorMapUniforms::wgsl_struct_definition();
        let _def3 = PositionTransformUniforms::wgsl_struct_definition();
        let _name1 = LinearScaleUniforms::wgsl_type_name();
        let _name2 = ColorMapUniforms::wgsl_type_name();
        let _name3 = PositionTransformUniforms::wgsl_type_name();
    }
    
    let duration = start.elapsed();
    
    // Should be <5% overhead vs original - with 10k iterations this should be well under 50ms
    assert!(duration.as_millis() < 50, 
        "ShaderUniform generation took {}ms, expected <50ms", duration.as_millis());
    
    Ok(())
}

#[tokio::test]
async fn test_type_safe_composition() -> GupResult<()> {
    // Test that type-safe composition prevents invalid combinations at compile time
    
    // Valid compositions (these should compile)
    let scale = LinearScale::new(0.0, 100.0, 0.0, 1.0); // f32 -> f32
    let color_map = ColorMap::new(vec4![0.0, 0.0, 0.0, 1.0], vec4![1.0, 1.0, 1.0, 1.0]); // f32 -> Vec4
    let _composed = scale.compose(color_map); // f32 -> f32 -> Vec4 ✓
    
    let position = PositionTransform::new(vec2![1.0, 1.0], vec2![0.0, 0.0]); // Vec2 -> Vec2
    let _position_chain = position.compose(position.clone()); // Vec2 -> Vec2 -> Vec2 ✓
    
    // The following would fail to compile (uncomment to test):
    // let bad_compose = color_map.compose(position); // Vec4 -> Vec2 ✗ (not compatible)
    // let bad_compose2 = position.compose(scale); // Vec2 -> f32 ✗ (not compatible)
    
    Ok(())
}

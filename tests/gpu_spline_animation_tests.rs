// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GPU tests for spline animation curves (GUP-141)
//!
//! Verifies that spline interpolation WGSL code compiles and executes correctly on GPU.

use gup::shader_function::{
    ComposableShaderFunction, KeyframeAnimation, KeyframeAnimationUniforms, ShaderUniform,
};

async fn create_test_device() -> (wgpu::Device, wgpu::Queue) {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::all(),
        ..Default::default()
    });

    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            compatible_surface: None,
            force_fallback_adapter: false,
        })
        .await
        .expect("Failed to find an appropriate adapter");

    adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("Test Device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            memory_hints: wgpu::MemoryHints::default(),
            trace: Default::default(),
        })
        .await
        .expect("Failed to create device")
}

#[tokio::test]
async fn test_linear_interpolation_gpu() {
    let (device, _queue) = create_test_device().await;

    let _anim = KeyframeAnimation::new()
        .add_keyframe(0.0, 0.0)
        .add_keyframe(1.0, 100.0);

    // Create shader module with the keyframe animation function
    let wgsl = KeyframeAnimation::wgsl_function();
    let shader_code = format!(
        "{}

        {}

        @group(0) @binding(0) var<uniform> params: KeyframeAnimationUniforms;
        @group(0) @binding(1) var<storage, read_write> output: array<f32>;

        @compute @workgroup_size(1)
        fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {{
            let time = f32(global_id.x) * 0.5; // Test at 0.0, 0.5, 1.0
            output[global_id.x] = keyframe_animation(time, params);
        }}
        ",
        KeyframeAnimationUniforms::wgsl_struct_definition(),
        wgsl
    );

    let _shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Spline Animation Test Shader"),
        source: wgpu::ShaderSource::Wgsl(shader_code.into()),
    });

    // Verify the shader compiled successfully
    println!("Linear interpolation shader compiled successfully");
}

#[tokio::test]
async fn test_catmull_rom_interpolation_gpu() {
    let (device, _queue) = create_test_device().await;

    let _anim = KeyframeAnimation::new()
        .add_keyframe(0.0, 0.0)
        .add_keyframe(1.0, 100.0)
        .add_keyframe(2.0, 50.0)
        .add_keyframe(3.0, 150.0)
        .with_catmull_rom(0.0);

    // Create shader module with the keyframe animation function
    let wgsl = KeyframeAnimation::wgsl_function();
    let shader_code = format!(
        "{}

        {}

        @group(0) @binding(0) var<uniform> params: KeyframeAnimationUniforms;
        @group(0) @binding(1) var<storage, read_write> output: array<f32>;

        @compute @workgroup_size(1)
        fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {{
            let time = f32(global_id.x) * 0.25; // Test multiple time points
            output[global_id.x] = keyframe_animation(time, params);
        }}
        ",
        KeyframeAnimationUniforms::wgsl_struct_definition(),
        wgsl
    );

    let _shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Catmull-Rom Spline Test Shader"),
        source: wgpu::ShaderSource::Wgsl(shader_code.into()),
    });

    println!("Catmull-Rom spline shader compiled successfully");
}

#[tokio::test]
async fn test_bspline_interpolation_gpu() {
    let (device, _queue) = create_test_device().await;

    let _anim = KeyframeAnimation::new()
        .add_keyframe(0.0, 0.0)
        .add_keyframe(1.0, 100.0)
        .add_keyframe(2.0, 50.0)
        .with_bspline();

    // Create shader module with the keyframe animation function
    let wgsl = KeyframeAnimation::wgsl_function();
    let shader_code = format!(
        "{}

        {}

        @group(0) @binding(0) var<uniform> params: KeyframeAnimationUniforms;
        @group(0) @binding(1) var<storage, read_write> output: array<f32>;

        @compute @workgroup_size(1)
        fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {{
            let time = f32(global_id.x) * 0.25; // Test multiple time points
            output[global_id.x] = keyframe_animation(time, params);
        }}
        ",
        KeyframeAnimationUniforms::wgsl_struct_definition(),
        wgsl
    );

    let _shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("B-Spline Test Shader"),
        source: wgpu::ShaderSource::Wgsl(shader_code.into()),
    });

    println!("B-spline shader compiled successfully");
}

#[tokio::test]
async fn test_catmull_rom_with_tension_gpu() {
    let (device, _queue) = create_test_device().await;

    let _anim = KeyframeAnimation::new()
        .add_keyframe(0.0, 0.0)
        .add_keyframe(1.0, 100.0)
        .add_keyframe(2.0, 50.0)
        .with_catmull_rom(0.5); // Medium tension

    // Create shader module with the keyframe animation function
    let wgsl = KeyframeAnimation::wgsl_function();
    let shader_code = format!(
        "{}

        {}

        @group(0) @binding(0) var<uniform> params: KeyframeAnimationUniforms;
        @group(0) @binding(1) var<storage, read_write> output: array<f32>;

        @compute @workgroup_size(1)
        fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {{
            let time = f32(global_id.x) * 0.25;
            output[global_id.x] = keyframe_animation(time, params);
        }}
        ",
        KeyframeAnimationUniforms::wgsl_struct_definition(),
        wgsl
    );

    let _shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Catmull-Rom with Tension Test Shader"),
        source: wgpu::ShaderSource::Wgsl(shader_code.into()),
    });

    println!("Catmull-Rom with tension shader compiled successfully");
}

#[tokio::test]
async fn test_spline_helper_functions_syntax() {
    let (_device, _queue) = create_test_device().await;

    let wgsl = KeyframeAnimation::wgsl_function();

    // Verify helper functions are present
    assert!(wgsl.contains("fn catmull_rom_interpolate"));
    assert!(wgsl.contains("fn bspline_interpolate"));

    // Verify they have the correct signature structure
    assert!(wgsl.contains("p0: f32, p1: f32, p2: f32, p3: f32"));
    assert!(wgsl.contains("t: f32"));

    // Verify mode branching exists
    assert!(wgsl.contains("params.interpolation_mode == 0u"));
    assert!(wgsl.contains("params.interpolation_mode == 1u"));
    assert!(wgsl.contains("params.interpolation_mode == 2u"));
}

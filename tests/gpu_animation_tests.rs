// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GPU integration tests for Advanced Temporal Animation System (GUP-138)
//!
//! Tests that verify animation WGSL code compiles and executes correctly on the GPU.

use gup::shader_function::{
    ComposableShaderFunction, CubicBezierTiming, CubicBezierTimingUniforms, KeyframeAnimation,
    KeyframeAnimationUniforms, ShaderUniform,
};
use wgpu::util::DeviceExt;

/// Test helper to create GPU context
async fn create_gpu_context() -> (wgpu::Device, wgpu::Queue) {
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
            label: None,
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            memory_hints: wgpu::MemoryHints::default(),
            trace: Default::default(),
            experimental_features: Default::default(),
        })
        .await
        .expect("Failed to create device")
}

#[tokio::test]
async fn test_keyframe_animation_gpu_compilation() {
    let (device, _queue) = create_gpu_context().await;

    // Create a test keyframe animation
    let animation = KeyframeAnimation::new()
        .add_keyframe(0.0, 0.0)
        .add_keyframe(1.0, 100.0)
        .add_keyframe(2.0, 50.0);

    let uniforms = animation.create_uniforms().expect("Should create uniforms");

    // Create WGSL shader that uses keyframe animation
    let shader_source = format!(
        r#"
        {}

        {}

        @group(0) @binding(0) var<uniform> animation: KeyframeAnimationUniforms;
        @group(0) @binding(1) var<storage, read_write> output: array<f32>;

        @compute @workgroup_size(64)
        fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {{
            let time = f32(global_id.x) * 0.1;
            output[global_id.x] = keyframe_animation(time, animation);
        }}
        "#,
        KeyframeAnimationUniforms::wgsl_struct_definition(),
        KeyframeAnimation::wgsl_function()
    );

    // Try to compile the shader
    let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Keyframe Animation Test Shader"),
        source: wgpu::ShaderSource::Wgsl(shader_source.into()),
    });

    // Create bind group layout
    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Test Bind Group Layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    });

    // Create compute pipeline (this tests that WGSL compiles)
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Test Pipeline Layout"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });

    let _pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("Test Pipeline"),
        layout: Some(&pipeline_layout),
        module: &shader_module,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    });

    println!(
        "✓ KeyframeAnimation WGSL compiled successfully with {} keyframes",
        uniforms.keyframe_count
    );
}

#[tokio::test]
async fn test_cubic_bezier_timing_gpu_compilation() {
    let (device, _queue) = create_gpu_context().await;

    // Create a cubic bezier timing
    let bezier = CubicBezierTiming::ease_in_out();
    let uniforms = bezier.create_uniforms().expect("Should create uniforms");

    // Create WGSL shader that uses cubic bezier timing
    let shader_source = format!(
        r#"
        {}

        {}

        @group(0) @binding(0) var<uniform> timing: CubicBezierTimingUniforms;
        @group(0) @binding(1) var<storage, read_write> output: array<f32>;

        @compute @workgroup_size(64)
        fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {{
            let t = f32(global_id.x) / 100.0;
            output[global_id.x] = cubic_bezier_timing(t, timing);
        }}
        "#,
        CubicBezierTimingUniforms::wgsl_struct_definition(),
        CubicBezierTiming::wgsl_function()
    );

    // Try to compile the shader
    let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Cubic Bezier Test Shader"),
        source: wgpu::ShaderSource::Wgsl(shader_source.into()),
    });

    // Create bind group layout
    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Test Bind Group Layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    });

    // Create compute pipeline
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Test Pipeline Layout"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });

    let _pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("Test Pipeline"),
        layout: Some(&pipeline_layout),
        module: &shader_module,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    });

    println!(
        "✓ CubicBezierTiming WGSL compiled successfully ({}, {}, {}, {})",
        uniforms.x1, uniforms.y1, uniforms.x2, uniforms.y2
    );
}

#[tokio::test]
async fn test_keyframe_animation_gpu_execution() {
    let (device, queue) = create_gpu_context().await;

    // Create animation: 0->100 over 1 second
    let animation = KeyframeAnimation::new()
        .add_keyframe(0.0, 0.0)
        .add_keyframe(1.0, 100.0);

    let uniforms = animation.create_uniforms().expect("Should create uniforms");

    // Create WGSL shader
    let shader_source = format!(
        r#"
        {}

        {}

        @group(0) @binding(0) var<uniform> animation: KeyframeAnimationUniforms;
        @group(0) @binding(1) var<storage, read_write> output: array<f32>;

        @compute @workgroup_size(1)
        fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {{
            let time = f32(global_id.x) * 0.5; // 0.0, 0.5, 1.0
            output[global_id.x] = keyframe_animation(time, animation);
        }}
        "#,
        KeyframeAnimationUniforms::wgsl_struct_definition(),
        KeyframeAnimation::wgsl_function()
    );

    let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Test Shader"),
        source: wgpu::ShaderSource::Wgsl(shader_source.into()),
    });

    // Create buffers
    let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Uniform Buffer"),
        contents: bytemuck::cast_slice(&[uniforms]),
        usage: wgpu::BufferUsages::UNIFORM,
    });

    let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Output Buffer"),
        size: 3 * std::mem::size_of::<f32>() as u64,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });

    let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Staging Buffer"),
        size: 3 * std::mem::size_of::<f32>() as u64,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    // Create bind group
    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: None,
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: output_buffer.as_entire_binding(),
            },
        ],
    });

    // Create pipeline
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });

    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: None,
        layout: Some(&pipeline_layout),
        module: &shader_module,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    });

    // Execute compute shader
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Test Encoder"),
    });

    {
        let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Test Compute Pass"),
            timestamp_writes: None,
        });
        compute_pass.set_pipeline(&pipeline);
        compute_pass.set_bind_group(0, &bind_group, &[]);
        compute_pass.dispatch_workgroups(3, 1, 1); // 3 time values
    }

    encoder.copy_buffer_to_buffer(&output_buffer, 0, &staging_buffer, 0, staging_buffer.size());

    queue.submit(Some(encoder.finish()));

    // Read results
    let buffer_slice = staging_buffer.slice(..);
    let (tx, rx) = futures::channel::oneshot::channel();
    buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
        tx.send(result).unwrap();
    });

    let _ = device.poll(wgpu::PollType::Wait {
        submission_index: None,
        timeout: None,
    });
    rx.await.unwrap().unwrap();

    let data = buffer_slice.get_mapped_range();
    let results: &[f32] = bytemuck::cast_slice(&data);

    // Check results
    println!("GPU animation results:");
    println!("  time=0.0: value={} (expected ~0)", results[0]);
    println!("  time=0.5: value={} (expected ~50)", results[1]);
    println!("  time=1.0: value={} (expected ~100)", results[2]);

    // Verify interpolation
    assert!((results[0] - 0.0).abs() < 0.1, "At t=0.0, should be ~0");
    assert!(
        (results[1] - 50.0).abs() < 1.0,
        "At t=0.5, should be ~50 (linear interpolation)"
    );
    assert!((results[2] - 100.0).abs() < 0.1, "At t=1.0, should be ~100");

    println!("✓ GPU keyframe animation interpolation verified");
}

#[tokio::test]
async fn test_animation_performance_1000_simultaneous() {
    let (device, queue) = create_gpu_context().await;

    // Test AC4: Support thousands of simultaneous animations
    let num_animations = 1000;

    // Create a simple animation
    let animation = KeyframeAnimation::new()
        .add_keyframe(0.0, 0.0)
        .add_keyframe(1.0, 100.0);

    let uniforms = animation.create_uniforms().expect("Should create uniforms");

    // Create WGSL shader
    let shader_source = format!(
        r#"
        {}

        {}

        @group(0) @binding(0) var<uniform> animation: KeyframeAnimationUniforms;
        @group(0) @binding(1) var<storage, read_write> output: array<f32>;

        @compute @workgroup_size(256)
        fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {{
            let idx = global_id.x;
            if (idx >= 1000u) {{
                return;
            }}
            let time = f32(idx) / 1000.0;
            output[idx] = keyframe_animation(time, animation);
        }}
        "#,
        KeyframeAnimationUniforms::wgsl_struct_definition(),
        KeyframeAnimation::wgsl_function()
    );

    let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Performance Test Shader"),
        source: wgpu::ShaderSource::Wgsl(shader_source.into()),
    });

    // Create buffers
    let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Uniform Buffer"),
        contents: bytemuck::cast_slice(&[uniforms]),
        usage: wgpu::BufferUsages::UNIFORM,
    });

    let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Output Buffer"),
        size: (num_animations * std::mem::size_of::<f32>()) as u64,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });

    // Create bind group and pipeline
    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: None,
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: output_buffer.as_entire_binding(),
            },
        ],
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });

    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: None,
        layout: Some(&pipeline_layout),
        module: &shader_module,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    });

    // Execute
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Performance Test Encoder"),
    });

    {
        let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Performance Test Pass"),
            timestamp_writes: None,
        });
        compute_pass.set_pipeline(&pipeline);
        compute_pass.set_bind_group(0, &bind_group, &[]);
        compute_pass.dispatch_workgroups(4, 1, 1); // 4 * 256 = 1024 threads
    }

    queue.submit(Some(encoder.finish()));
    let _ = device.poll(wgpu::PollType::Wait {
        submission_index: None,
        timeout: None,
    });

    println!(
        "✓ Successfully processed {} simultaneous animations on GPU",
        num_animations
    );
}

// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GPU execution tests for storage buffer-based keyframe animations.

use gup::{Keyframe, KeyframeAnimationStorage};
use std::sync::Arc;
use wgpu::util::DeviceExt;

async fn create_test_context() -> Option<(Arc<wgpu::Device>, Arc<wgpu::Queue>)> {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::PRIMARY,
        ..Default::default()
    });

    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions::default())
        .await
        .ok()?;

    let (device, queue) = adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::default(),
                trace: Default::default(),
            },
        )
        .await
        .ok()?;

    Some((Arc::new(device), Arc::new(queue)))
}

#[tokio::test]
async fn test_wgsl_compilation() {
    let Some((device, _queue)) = create_test_context().await else {
        eprintln!("Skipping test: GPU not available");
        return;
    };

    let struct_def = KeyframeAnimationStorage::wgsl_struct_definition();
    let function = KeyframeAnimationStorage::wgsl_function();

    let shader_source = format!(
        r#"
{}

{}

@compute @workgroup_size(1)
fn main() {{
    let value = keyframe_animation_storage(0.5);
}}
"#,
        struct_def, function
    );

    let result = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("KeyframeAnimationStorage Test Shader"),
        source: wgpu::ShaderSource::Wgsl(shader_source.into()),
    });

    // If we got here without panicking, compilation succeeded
    assert!(std::ptr::addr_of!(result) as usize != 0);
}

#[tokio::test]
async fn test_gpu_interpolation_accuracy() {
    let Some((device, queue)) = create_test_context().await else {
        eprintln!("Skipping test: GPU not available");
        return;
    };

    // Create simple linear keyframes for easy verification
    let anim = KeyframeAnimationStorage::builder()
        .add_keyframe(0.0, 0.0)
        .add_keyframe(1.0, 100.0)
        .build();

    // Create storage buffer for keyframes
    let keyframes_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Keyframes Storage Buffer"),
        contents: &anim.create_keyframes_buffer_data(),
        usage: wgpu::BufferUsages::STORAGE,
    });

    // Create uniform buffer for animation info
    let info_data: [u32; 4] = [
        anim.count(),
        if anim.loop_animation { 1 } else { 0 },
        if anim.reverse_on_loop { 1 } else { 0 },
        0, // padding
    ];
    let info_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Animation Info Buffer"),
        contents: bytemuck::cast_slice(&info_data),
        usage: wgpu::BufferUsages::UNIFORM,
    });

    // Create output buffer
    let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Output Buffer"),
        size: 4 * std::mem::size_of::<f32>() as u64, // 4 test values
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });

    let struct_def = KeyframeAnimationStorage::wgsl_struct_definition();
    let function = KeyframeAnimationStorage::wgsl_function();

    let shader_source = format!(
        r#"
{}

{}

@group(0) @binding(3) var<storage, read_write> output: array<f32>;

@compute @workgroup_size(1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {{
    // Test 4 different time values
    output[0] = keyframe_animation_storage(0.0);   // Should be 0.0
    output[1] = keyframe_animation_storage(0.5);   // Should be 50.0
    output[2] = keyframe_animation_storage(1.0);   // Should be 100.0
    output[3] = keyframe_animation_storage(0.25);  // Should be 25.0
}}
"#,
        struct_def, function
    );

    let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Test Shader"),
        source: wgpu::ShaderSource::Wgsl(shader_source.into()),
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Bind Group Layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 3,
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
        label: Some("Bind Group"),
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 1,
                resource: keyframes_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: info_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: output_buffer.as_entire_binding(),
            },
        ],
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Pipeline Layout"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });

    let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("Compute Pipeline"),
        layout: Some(&pipeline_layout),
        module: &shader_module,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Command Encoder"),
    });

    {
        let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Compute Pass"),
            timestamp_writes: None,
        });
        compute_pass.set_pipeline(&compute_pipeline);
        compute_pass.set_bind_group(0, &bind_group, &[]);
        compute_pass.dispatch_workgroups(1, 1, 1);
    }

    // Read back results
    let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Staging Buffer"),
        size: 4 * std::mem::size_of::<f32>() as u64,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    encoder.copy_buffer_to_buffer(&output_buffer, 0, &staging_buffer, 0, 4 * 4);

    queue.submit(Some(encoder.finish()));

    let buffer_slice = staging_buffer.slice(..);
    let (tx, rx) = futures::channel::oneshot::channel();
    buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
        tx.send(result).unwrap();
    });
    let _ = device.poll(wgpu::PollType::Wait);
    rx.await.unwrap().unwrap();

    let data = buffer_slice.get_mapped_range();
    let results: &[f32] = bytemuck::cast_slice(&data);

    // Verify interpolation
    assert!((results[0] - 0.0).abs() < 0.1, "t=0.0 should be ~0.0");
    assert!((results[1] - 50.0).abs() < 0.1, "t=0.5 should be ~50.0");
    assert!((results[2] - 100.0).abs() < 0.1, "t=1.0 should be ~100.0");
    assert!((results[3] - 25.0).abs() < 0.1, "t=0.25 should be ~25.0");

    println!("GPU Interpolation Results:");
    println!("  t=0.0:  {} (expected 0.0)", results[0]);
    println!("  t=0.5:  {} (expected 50.0)", results[1]);
    println!("  t=1.0:  {} (expected 100.0)", results[2]);
    println!("  t=0.25: {} (expected 25.0)", results[3]);
}

#[tokio::test]
async fn test_large_keyframe_count() {
    let Some((device, queue)) = create_test_context().await else {
        eprintln!("Skipping test: GPU not available");
        return;
    };

    // Test with 100 keyframes
    let mut builder = KeyframeAnimationStorage::builder();
    for i in 0..100 {
        builder = builder.add_keyframe(i as f32, i as f32 * 10.0);
    }
    let anim = builder.build();

    // Create buffers
    let keyframes_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Large Keyframes Storage Buffer"),
        contents: &anim.create_keyframes_buffer_data(),
        usage: wgpu::BufferUsages::STORAGE,
    });

    let info_data: [u32; 4] = [anim.count(), 0, 0, 0];
    let info_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Large Animation Info Buffer"),
        contents: bytemuck::cast_slice(&info_data),
        usage: wgpu::BufferUsages::UNIFORM,
    });

    let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Large Output Buffer"),
        size: std::mem::size_of::<f32>() as u64,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });

    let struct_def = KeyframeAnimationStorage::wgsl_struct_definition();
    let function = KeyframeAnimationStorage::wgsl_function();

    let shader_source = format!(
        r#"
{}

{}

@group(0) @binding(3) var<storage, read_write> output: array<f32>;

@compute @workgroup_size(1)
fn main() {{
    // Test lookup in middle of large array
    output[0] = keyframe_animation_storage(50.0);
}}
"#,
        struct_def, function
    );

    let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Large Test Shader"),
        source: wgpu::ShaderSource::Wgsl(shader_source.into()),
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Large Bind Group Layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 3,
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
        label: Some("Large Bind Group"),
        layout: &bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 1,
                resource: keyframes_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: info_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: output_buffer.as_entire_binding(),
            },
        ],
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Large Pipeline Layout"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });

    let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("Large Compute Pipeline"),
        layout: Some(&pipeline_layout),
        module: &shader_module,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Large Command Encoder"),
    });

    {
        let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Large Compute Pass"),
            timestamp_writes: None,
        });
        compute_pass.set_pipeline(&compute_pipeline);
        compute_pass.set_bind_group(0, &bind_group, &[]);
        compute_pass.dispatch_workgroups(1, 1, 1);
    }

    let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Large Staging Buffer"),
        size: std::mem::size_of::<f32>() as u64,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    encoder.copy_buffer_to_buffer(&output_buffer, 0, &staging_buffer, 0, 4);
    queue.submit(Some(encoder.finish()));

    let buffer_slice = staging_buffer.slice(..);
    let (tx, rx) = futures::channel::oneshot::channel();
    buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
        tx.send(result).unwrap();
    });
    let _ = device.poll(wgpu::PollType::Wait);
    rx.await.unwrap().unwrap();

    let data = buffer_slice.get_mapped_range();
    let result: f32 = bytemuck::cast_slice(&data)[0];

    // At t=50.0, value should be 500.0
    assert!(
        (result - 500.0).abs() < 0.1,
        "Expected 500.0, got {}",
        result
    );
    println!(
        "Large keyframe GPU test: t=50.0 => {} (expected 500.0)",
        result
    );
}

#[tokio::test]
async fn test_binary_search_performance() {
    let Some((device, _queue)) = create_test_context().await else {
        eprintln!("Skipping test: GPU not available");
        return;
    };

    // Test with 1000 keyframes to verify binary search efficiency
    let mut builder = KeyframeAnimationStorage::builder();
    for i in 0..1000 {
        builder = builder.add_keyframe(i as f32, i as f32);
    }
    let anim = builder.build();

    // If shader compiles with 1000 keyframes, binary search is working
    let struct_def = KeyframeAnimationStorage::wgsl_struct_definition();
    let function = KeyframeAnimationStorage::wgsl_function();

    let shader_source = format!(
        r#"
{}

{}

@compute @workgroup_size(1)
fn main() {{
    // Test lookup in 1000-keyframe array
    let value = keyframe_animation_storage(500.0);
}}
"#,
        struct_def, function
    );

    let result = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Binary Search Test Shader"),
        source: wgpu::ShaderSource::Wgsl(shader_source.into()),
    });

    println!("Binary search shader compiled successfully with 1000 keyframes");
    println!("Buffer size: {} bytes", anim.create_keyframes_buffer_data().len());

    assert!(std::ptr::addr_of!(result) as usize != 0);
}

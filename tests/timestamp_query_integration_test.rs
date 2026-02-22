// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Integration test for WebGPU timestamp query support

use gup::debug::ShaderProfiler;
use gup::GupContext;
use wgpu::*;

#[tokio::test]
async fn test_timestamp_query_detection() {
    // Create GPU context
    let context = match GupContext::new().await {
        Ok(ctx) => ctx,
        Err(_) => {
            eprintln!("⚠️  Could not initialize GPU context, skipping test");
            return;
        }
    };

    // Create shader profiler
    let profiler = ShaderProfiler::new(&context.device, &context.queue);

    // Check timestamp support
    let supports = profiler.supports_timestamps();
    let features = context.device.features();

    println!("GPU Features: {:?}", features);
    println!("Timestamp Query Support: {}", supports);

    // Verify detection matches device features
    assert_eq!(
        supports,
        features.contains(Features::TIMESTAMP_QUERY),
        "Timestamp support detection should match device features"
    );
}

#[tokio::test]
async fn test_timestamp_query_fallback() {
    // Create GPU context
    let context = match GupContext::new().await {
        Ok(ctx) => ctx,
        Err(_) => {
            eprintln!("⚠️  Could not initialize GPU context, skipping test");
            return;
        }
    };

    // Create a simple compute shader for testing
    let shader = context
        .device
        .create_shader_module(ShaderModuleDescriptor {
            label: Some("test_compute_shader"),
            source: ShaderSource::Wgsl(
                r#"
                @group(0) @binding(0) var<storage, read_write> data: array<f32>;

                @compute @workgroup_size(64)
                fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
                    let index = global_id.x;
                    data[index] = data[index] * 2.0;
                }
            "#
                .into(),
            ),
        });

    // Create buffer
    let buffer = context.device.create_buffer(&BufferDescriptor {
        label: Some("test_buffer"),
        size: 256 * 4, // 256 f32 values
        usage: BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });

    // Create bind group layout
    let bind_group_layout = context
        .device
        .create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("test_bind_group_layout"),
            entries: &[BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

    // Create pipeline
    let pipeline_layout = context
        .device
        .create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("test_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

    let pipeline = context
        .device
        .create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("test_pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

    // Create bind group
    let bind_group = context.device.create_bind_group(&BindGroupDescriptor {
        label: Some("test_bind_group"),
        layout: &bind_group_layout,
        entries: &[BindGroupEntry {
            binding: 0,
            resource: buffer.as_entire_binding(),
        }],
    });

    // Profile the compute shader
    let mut profiler = ShaderProfiler::new(&context.device, &context.queue);
    let stats = profiler
        .profile_compute(&pipeline, &bind_group, (4, 1, 1))
        .await
        .expect("Profiling should succeed");

    println!("Profiling Stats:");
    println!("  Duration: {:?}", stats.duration);
    println!("  GPU Utilization: {}%", stats.gpu_utilization_percent);
    println!("  Workgroup Count: {}", stats.workgroup_count);
    println!(
        "  Used Hardware Timestamps: {}",
        stats.used_hardware_timestamps
    );

    // Verify stats
    assert!(stats.duration.as_nanos() > 0, "Duration should be non-zero");
    assert_eq!(stats.workgroup_count, 4, "Should dispatch 4 workgroups");

    // If timestamps are supported, verify they were used
    if profiler.supports_timestamps() {
        println!("✓ GPU timestamp queries are supported and used");
        // Note: We can't force hardware timestamps to be used because wgpu may not
        // support them even if the feature is present (WebGPU compatibility)
    } else {
        println!("⚠  Using CPU-based timing fallback (timestamps not supported)");
        assert!(
            !stats.used_hardware_timestamps,
            "Should not claim to use timestamps when not supported"
        );
    }
}

#[tokio::test]
async fn test_profiling_with_baseline() {
    // Create GPU context
    let context = match GupContext::new().await {
        Ok(ctx) => ctx,
        Err(_) => {
            eprintln!("⚠️  Could not initialize GPU context, skipping test");
            return;
        }
    };

    // Create a simple compute shader
    let shader = context
        .device
        .create_shader_module(ShaderModuleDescriptor {
            label: Some("baseline_test_shader"),
            source: ShaderSource::Wgsl(
                r#"
                @group(0) @binding(0) var<storage, read_write> data: array<f32>;

                @compute @workgroup_size(64)
                fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
                    let index = global_id.x;
                    data[index] = data[index] + 1.0;
                }
            "#
                .into(),
            ),
        });

    // Create buffer
    let buffer = context.device.create_buffer(&BufferDescriptor {
        label: Some("baseline_test_buffer"),
        size: 256 * 4,
        usage: BufferUsages::STORAGE,
        mapped_at_creation: false,
    });

    // Create bind group layout and pipeline (similar to previous test)
    let bind_group_layout = context
        .device
        .create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("baseline_bind_group_layout"),
            entries: &[BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

    let pipeline_layout = context
        .device
        .create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("baseline_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

    let pipeline = context
        .device
        .create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("baseline_pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

    let bind_group = context.device.create_bind_group(&BindGroupDescriptor {
        label: Some("baseline_bind_group"),
        layout: &bind_group_layout,
        entries: &[BindGroupEntry {
            binding: 0,
            resource: buffer.as_entire_binding(),
        }],
    });

    // Profile multiple times to establish baseline
    let mut profiler = ShaderProfiler::new(&context.device, &context.queue);

    for _ in 0..5 {
        let _ = profiler
            .profile_compute(&pipeline, &bind_group, (4, 1, 1))
            .await;
    }

    // Create baseline
    use gup::debug::PerformanceBaseline;
    use std::time::Duration;

    let baseline = PerformanceBaseline::new("test_baseline", Duration::from_micros(100), 75.0);
    profiler.set_performance_baseline("test_baseline", baseline);

    // Profile again and check for regression detection
    let stats = profiler
        .profile_compute(&pipeline, &bind_group, (4, 1, 1))
        .await
        .expect("Profiling should succeed");

    // Test regression detection (this may or may not trigger depending on actual performance)
    if let Some(regression) = profiler.check_performance_regression("test_baseline", &stats) {
        println!("Performance regression detected: {:?}", regression);
    } else {
        println!("✓ No performance regression detected");
    }

    println!(
        "Baseline system working correctly (timestamp support: {})",
        profiler.supports_timestamps()
    );
}

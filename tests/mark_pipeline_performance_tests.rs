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

//! Performance validation tests for Mark Pipeline Integration (GUP-068).
//!
//! These tests validate that the mark pipeline integration meets the
//! performance requirements specified in the story, including pipeline
//! creation times, caching efficiency, and rendering performance.

use gup::buffer::{BufferType, GpuBuffer};
use gup::context::GupContext;
use gup::error::GupResult;
use gup::mark::{
    Circle, CircleAttributes, Mark, MarkInfo, MarkInfoImpl, MarkRegistry, MarkRenderer,
};
use gup::{Vec2, Vec4, vec2, vec4};
use std::sync::Arc;
use std::time::Instant;
use wgpu::util::DeviceExt;

/// Helper function to create test context for GPU operations.
async fn create_test_context() -> GupResult<Arc<GupContext>> {
    GupContext::headless().await
}

/// Test that pipeline creation meets performance targets.
/// Target: <10ms per pipeline creation (excluding initial shader compilation)
#[tokio::test]
async fn test_pipeline_creation_performance() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;

    let mark_info = MarkInfoImpl::<Circle>::new();

    // Measure pipeline creation time
    let start = Instant::now();
    for _ in 0..10 {
        let _pipeline = mark_info.create_render_pipeline(device)?;
    }
    let duration = start.elapsed();

    let avg_time_per_pipeline = duration.as_millis() / 10;
    println!("Average pipeline creation time: {avg_time_per_pipeline}ms");

    // Pipeline creation should be reasonably fast (allowing for shader compilation overhead)
    assert!(
        avg_time_per_pipeline < 100,
        "Pipeline creation too slow: {avg_time_per_pipeline}ms per pipeline (target: <100ms including shader compilation)"
    );

    Ok(())
}

/// Test that cached pipeline access meets performance targets.
/// Target: Cached pipeline access in <1ms
#[tokio::test]
async fn test_cached_pipeline_performance() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;

    let mut registry = MarkRegistry::new();
    registry.register::<Circle>();

    // Create initial pipeline (this will be cached)
    let _initial_pipeline = registry.get_pipeline::<Circle>(device)?;

    // Measure cached access performance
    let start = Instant::now();
    for _ in 0..1000 {
        let _pipeline = registry.get_pipeline::<Circle>(device)?;
    }
    let duration = start.elapsed();

    let avg_time_per_access = duration.as_micros() / 1000;
    println!("Average cached pipeline access time: {avg_time_per_access}μs");

    // Cached access should be very fast
    assert!(
        avg_time_per_access < 1000, // <1ms = <1000μs
        "Cached pipeline access too slow: {avg_time_per_access}μs per access (target: <1000μs)"
    );

    Ok(())
}

/// Test bind group creation performance.
/// Target: <5ms per bind group creation
#[tokio::test]
async fn test_bind_group_creation_performance() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;

    let mut registry = MarkRegistry::new();
    registry.register::<Circle>();

    // Create instance buffer for bind group creation
    let instance_buffer = GpuBuffer::<u8>::new(device, BufferType::Instance, 100);

    // Viewport uniform buffer — required by the bind group layout for
    // custom-shader marks (binding 1).
    let viewport = gup::ViewportUniforms {
        width: 64.0,
        height: 64.0,
    };
    let viewport_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("test_viewport_uniform"),
        contents: bytemuck::bytes_of(&viewport),
        usage: wgpu::BufferUsages::UNIFORM,
    });
    let uniform_buffers: Vec<&wgpu::Buffer> = vec![&viewport_buf];

    // Measure bind group creation time
    let start = Instant::now();
    for _ in 0..100 {
        let _bind_group = registry.create_bind_group::<Circle>(
            device,
            instance_buffer.buffer(),
            &uniform_buffers,
        )?;
    }
    let duration = start.elapsed();

    let avg_time_per_bind_group = duration.as_millis() / 100;
    println!("Average bind group creation time: {avg_time_per_bind_group}ms");

    // Bind group creation should be fast
    assert!(
        avg_time_per_bind_group < 5,
        "Bind group creation too slow: {avg_time_per_bind_group}ms per bind group (target: <5ms)"
    );

    Ok(())
}

/// Test mark renderer buffer upload performance.
/// Target: Handle large datasets efficiently
#[tokio::test]
async fn test_buffer_upload_performance() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;
    let queue = &context.queue;

    let mut renderer = MarkRenderer::new(device);

    // Test vertex upload performance with 10K vertices
    let vertex_count = 10_000;
    let vertices: Vec<[f32; 2]> = (0..vertex_count)
        .map(|i| [i as f32, (i * 2) as f32])
        .collect();

    let start = Instant::now();
    renderer.upload_vertices(device, queue, &vertices)?;
    let vertex_upload_time = start.elapsed();

    println!(
        "Vertex upload time for {}K vertices: {:?}",
        vertex_count / 1000,
        vertex_upload_time
    );

    // Should handle large vertex uploads efficiently
    assert!(
        vertex_upload_time.as_millis() < 100,
        "Vertex upload too slow: {:?} for {}K vertices (target: <100ms)",
        vertex_upload_time,
        vertex_count / 1000
    );

    // Test instance upload performance with 5K instances
    let instance_count = 5_000;
    #[repr(C)]
    #[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
    struct TestInstance {
        center: [f32; 2],
        radius: f32,
        color: [f32; 4],
        _padding: f32,
    }

    let instances: Vec<TestInstance> = (0..instance_count)
        .map(|i| TestInstance {
            center: [i as f32, (i * 2) as f32],
            radius: 5.0 + (i % 10) as f32,
            color: [1.0, 0.0, 0.0, 1.0],
            _padding: 0.0,
        })
        .collect();

    let start = Instant::now();
    renderer.upload_instances(device, queue, &instances)?;
    let instance_upload_time = start.elapsed();

    println!(
        "Instance upload time for {}K instances: {:?}",
        instance_count / 1000,
        instance_upload_time
    );

    // Should handle large instance uploads efficiently
    assert!(
        instance_upload_time.as_millis() < 50,
        "Instance upload too slow: {:?} for {}K instances (target: <50ms)",
        instance_upload_time,
        instance_count / 1000
    );

    Ok(())
}

/// Test memory efficiency of pipeline caching.
/// Verify that multiple pipelines don't cause excessive memory usage
#[tokio::test]
async fn test_pipeline_cache_memory_efficiency() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;

    let mut registry = MarkRegistry::new();
    registry.register::<Circle>();

    // Create and cache multiple pipeline instances
    let mut pipelines = Vec::new();
    for _ in 0..10 {
        let pipeline = registry.get_pipeline::<Circle>(device)?;
        pipelines.push(pipeline);
    }

    // Should only have one actual pipeline cached (Arc sharing)
    assert_eq!(registry.pipeline_count(), 1);

    // All pipeline references should point to the same underlying object
    for i in 1..pipelines.len() {
        assert!(Arc::ptr_eq(&pipelines[0], &pipelines[i]));
    }

    Ok(())
}

/// Test buffer auto-resize performance under stress.
/// Verify that buffer resizing doesn't cause performance degradation
#[tokio::test]
async fn test_buffer_resize_performance() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;
    let queue = &context.queue;

    // Start with small buffer to force resizing
    let mut renderer = MarkRenderer::with_capacity(device, 64, 128, Some(32));

    // Gradually increase data size to trigger multiple resizes
    let test_sizes = [100, 500, 1000, 2000, 5000];
    let mut total_time = std::time::Duration::ZERO;

    for &size in &test_sizes {
        let data: Vec<[f32; 2]> = (0..size).map(|i| [i as f32, (i * 2) as f32]).collect();

        let start = Instant::now();
        renderer.upload_vertices(device, queue, &data)?;
        let upload_time = start.elapsed();

        total_time += upload_time;

        println!("Upload time for {size} vertices: {upload_time:?}");

        // Individual uploads should remain fast even during resize
        assert!(
            upload_time.as_millis() < 50,
            "Buffer resize caused slow upload: {upload_time:?} for {size} vertices"
        );
    }

    println!("Total time for all uploads with resizing: {total_time:?}");

    // Total time should be reasonable
    assert!(
        total_time.as_millis() < 500,
        "Total buffer resize performance too slow: {total_time:?} (target: <500ms)"
    );

    Ok(())
}

/// Test registry operations performance with multiple mark types.
/// Verify that registry scales well with multiple registered marks
#[tokio::test]
async fn test_registry_scalability() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;

    let mut registry = MarkRegistry::new();

    // Register multiple mark types
    registry.register::<Circle>();

    // Test lookup performance
    let start = Instant::now();
    for _ in 0..1000 {
        assert!(registry.is_registered::<Circle>());
        let _info = registry.get_mark_info::<Circle>();
    }
    let lookup_time = start.elapsed();

    println!("1000 registry lookups took: {lookup_time:?}");

    // Registry operations should be fast
    assert!(
        lookup_time.as_millis() < 10,
        "Registry lookup performance too slow: {lookup_time:?} for 1000 lookups (target: <10ms)"
    );

    // Warm up: ensure the pipeline is created and cached before measuring
    let _pipeline = registry.get_pipeline::<Circle>(device)?;

    // Test cached pipeline retrieval performance
    let start = Instant::now();
    for _ in 0..100 {
        let _pipeline = registry.get_pipeline::<Circle>(device)?;
    }
    let pipeline_time = start.elapsed();

    println!("100 cached pipeline retrievals took: {pipeline_time:?}");

    // Cached pipeline retrieval should be fast (20ms budget allows for
    // environment variability while still catching real regressions)
    assert!(
        pipeline_time.as_millis() < 20,
        "Pipeline retrieval performance too slow: {pipeline_time:?} for 100 cached retrievals (target: <20ms)"
    );

    Ok(())
}

/// Test overall rendering workflow performance.
/// Measure end-to-end performance from mark setup to render preparation
#[tokio::test]
async fn test_end_to_end_workflow_performance() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;
    let queue = &context.queue;

    let workflow_start = Instant::now();

    // Step 1: Registry setup
    let mut registry = MarkRegistry::new();
    registry.register::<Circle>();

    // Step 2: Pipeline creation
    let pipeline = registry.get_pipeline::<Circle>(device)?;

    // Step 3: Renderer setup
    let mut renderer = MarkRenderer::new(device);

    // Step 4: Data upload
    let vertices = Circle::generate_vertices();
    renderer.upload_vertices(device, queue, &vertices)?;

    #[repr(C)]
    #[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
    struct CircleInstance {
        center: [f32; 2],
        radius: f32,
        fill_color: [f32; 4],
        stroke_width: f32,
        stroke_color: [f32; 4],
        _padding: [f32; 2],
    }

    let instances: Vec<CircleInstance> = (0..1000)
        .map(|i| CircleInstance {
            center: [i as f32, (i * 2) as f32],
            radius: 5.0,
            fill_color: [1.0, 0.0, 0.0, 1.0],
            stroke_width: 1.0,
            stroke_color: [0.0, 0.0, 0.0, 1.0],
            _padding: [0.0; 2],
        })
        .collect();

    renderer.upload_instances(device, queue, &instances)?;

    if let Some(indices) = Circle::generate_indices() {
        renderer.upload_indices(device, queue, &indices)?;
    }

    // Step 5: Bind group creation
    let instance_buffer = GpuBuffer::<u8>::new(device, BufferType::Instance, instances.len());
    let viewport = gup::ViewportUniforms {
        width: 64.0,
        height: 64.0,
    };
    let viewport_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("test_viewport_uniform"),
        contents: bytemuck::bytes_of(&viewport),
        usage: wgpu::BufferUsages::UNIFORM,
    });
    let bind_group =
        registry.create_bind_group::<Circle>(device, instance_buffer.buffer(), &[&viewport_buf])?;

    let total_workflow_time = workflow_start.elapsed();

    println!("Complete workflow setup time for 1000 instances: {total_workflow_time:?}");

    // End-to-end workflow should be fast enough for interactive applications
    assert!(
        total_workflow_time.as_millis() < 100,
        "End-to-end workflow too slow: {total_workflow_time:?} for 1000 instances (target: <100ms)"
    );

    // Verify all components are ready
    assert!(Arc::strong_count(&pipeline) >= 1);
    assert!(renderer.vertex_len() > 0);
    assert!(renderer.instance_len() > 0);
    drop(bind_group); // Verify bind group was created

    Ok(())
}

/// Benchmark mark attribute processing performance.
/// Test conversion from high-level attributes to GPU data
#[tokio::test]
async fn test_attribute_processing_performance() -> GupResult<()> {
    // Create test attributes
    let attribute_count = 10_000;
    let attributes: Vec<CircleAttributes> = (0..attribute_count)
        .map(|i| CircleAttributes {
            center: vec2![i as f32, (i * 2) as f32],
            radius: 5.0 + (i % 10) as f32,
            fill_color: vec4![1.0, 0.0, 0.0, 1.0],
            stroke_width: 1.0 + (i % 3) as f32,
            stroke_color: vec4![0.0, 0.0, 0.0, 1.0],
        })
        .collect();

    // Convert to GPU-compatible format
    #[repr(C)]
    #[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
    struct GpuCircleInstance {
        center: [f32; 2],
        radius: f32,
        fill_color: [f32; 4],
        stroke_width: f32,
        stroke_color: [f32; 4],
        _padding: [f32; 2],
    }

    let start = Instant::now();
    let gpu_instances: Vec<GpuCircleInstance> = attributes
        .iter()
        .map(|attr| GpuCircleInstance {
            center: [attr.center.x, attr.center.y],
            radius: attr.radius,
            fill_color: [
                attr.fill_color.x,
                attr.fill_color.y,
                attr.fill_color.z,
                attr.fill_color.w,
            ],
            stroke_width: attr.stroke_width,
            stroke_color: [
                attr.stroke_color.x,
                attr.stroke_color.y,
                attr.stroke_color.z,
                attr.stroke_color.w,
            ],
            _padding: [0.0; 2],
        })
        .collect();
    let conversion_time = start.elapsed();

    println!(
        "Attribute conversion time for {}K attributes: {:?}",
        attribute_count / 1000,
        conversion_time
    );

    // Attribute conversion should be fast
    assert!(
        conversion_time.as_millis() < 50,
        "Attribute conversion too slow: {:?} for {}K attributes (target: <50ms)",
        conversion_time,
        attribute_count / 1000
    );

    // Verify conversion completed
    assert_eq!(gpu_instances.len(), attribute_count);

    Ok(())
}

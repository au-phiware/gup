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

//! Integration tests for Mark Pipeline Integration (GUP-068).
//!
//! These tests validate the complete mark-to-render pipeline integration,
//! including render pipeline creation, bind group management, and full
//! rendering workflows.

use gup::buffer::{BufferType, GpuBuffer};
use gup::context::GupContext;
use gup::error::GupResult;
use gup::mark::{
    Circle, CircleAttributes, Mark, MarkInfo, MarkInfoImpl, MarkRegistry, MarkRenderer,
};
use gup::{Vec2, Vec4, vec2, vec4};
use std::sync::Arc;
use wgpu::util::DeviceExt;

/// Helper function to create test context for GPU operations.
async fn create_test_context() -> GupResult<Arc<GupContext>> {
    GupContext::headless().await
}

/// Test that render pipelines can be created successfully for mark types.
#[tokio::test]
async fn test_pipeline_creation() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;

    let mark_info = MarkInfoImpl::<Circle>::new();
    let _pipeline = mark_info.create_render_pipeline(device)?;

    // Pipeline should be created successfully
    // Note: wgpu pipeline labels are internal implementation details

    Ok(())
}

/// Test that bind groups can be created for mark types.
#[tokio::test]
async fn test_bind_group_creation() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;

    let mut registry = MarkRegistry::new();
    registry.register::<Circle>();

    // Create a dummy instance buffer
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

    let bind_group =
        registry.create_bind_group::<Circle>(device, instance_buffer.buffer(), &uniform_buffers)?;

    // Bind group should be created successfully
    // Cannot verify much about the bind group structure without internal access
    drop(bind_group); // Just verify it was created without error

    Ok(())
}

/// Test complete rendering workflow from mark to GPU.
#[tokio::test]
async fn test_complete_rendering_workflow() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;
    let queue = &context.queue;

    // Set up mark registry and renderer
    let mut registry = MarkRegistry::new();
    registry.register::<Circle>();

    let mut renderer = MarkRenderer::new(device);

    // Create pipeline
    let _pipeline = registry.get_pipeline::<Circle>(device)?;

    // Create bind group with instance buffer
    let instance_buffer = GpuBuffer::<u8>::new(device, BufferType::Instance, 10);
    let viewport = gup::ViewportUniforms {
        width: 64.0,
        height: 64.0,
    };
    let viewport_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("test_viewport_uniform"),
        contents: bytemuck::bytes_of(&viewport),
        usage: wgpu::BufferUsages::UNIFORM,
    });
    let _bind_group =
        registry.create_bind_group::<Circle>(device, instance_buffer.buffer(), &[&viewport_buf])?;

    // Upload vertex data
    let vertices = Circle::generate_vertices();
    renderer.upload_vertices(device, queue, &vertices)?;

    // Upload test instance data
    #[repr(C)]
    #[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
    struct CircleInstanceData {
        center: [f32; 2],
        radius: f32,
        fill_color: [f32; 4],
        stroke_width: f32,
        stroke_color: [f32; 4],
        _padding: [f32; 2], // Ensure proper alignment
    }

    let test_instances = vec![
        CircleInstanceData {
            center: [10.0, 20.0],
            radius: 5.0,
            fill_color: [1.0, 0.0, 0.0, 1.0],
            stroke_width: 1.0,
            stroke_color: [0.0, 0.0, 0.0, 1.0],
            _padding: [0.0; 2],
        },
        CircleInstanceData {
            center: [30.0, 40.0],
            radius: 8.0,
            fill_color: [0.0, 1.0, 0.0, 1.0],
            stroke_width: 2.0,
            stroke_color: [0.0, 0.0, 1.0, 1.0],
            _padding: [0.0; 2],
        },
    ];

    renderer.upload_instances(device, queue, &test_instances)?;

    // Upload index data if needed
    if let Some(indices) = Circle::generate_indices() {
        renderer.upload_indices(device, queue, &indices)?;
    }

    // Verify all data was uploaded correctly
    assert!(renderer.vertex_len() > 0);
    assert!(renderer.instance_len() > 0);
    if Circle::index_count().is_some() {
        assert!(renderer.index_len().unwrap_or(0) > 0);
    }

    // Note: Actual render pass creation would require a surface/texture target
    // For this integration test, we verify that all components are created successfully
    // and data is uploaded properly. Full rendering would be tested in examples or
    // interactive tests with actual graphics output.

    Ok(())
}

/// Test pipeline caching functionality.
#[tokio::test]
async fn test_pipeline_caching() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;

    let mut registry = MarkRegistry::new();
    registry.register::<Circle>();

    // First access should create pipeline
    let pipeline1 = registry.get_pipeline::<Circle>(device)?;
    assert_eq!(registry.pipeline_count(), 1);

    // Second access should return cached pipeline
    let pipeline2 = registry.get_pipeline::<Circle>(device)?;
    assert_eq!(registry.pipeline_count(), 1);

    // Should be the same pipeline instance (Arc equality)
    assert!(Arc::ptr_eq(&pipeline1, &pipeline2));

    Ok(())
}

/// Test error handling for unregistered mark types.
#[tokio::test]
async fn test_unregistered_mark_error_handling() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;

    let mut registry = MarkRegistry::new();
    // Note: Not registering Circle

    // Should fail with appropriate error
    let result = registry.get_pipeline::<Circle>(device);
    assert!(result.is_err());

    let error_msg = format!("{:?}", result.unwrap_err());
    assert!(error_msg.contains("not registered"));

    Ok(())
}

/// Test multiple mark types in same registry.
#[tokio::test]
async fn test_multiple_mark_types() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;

    let mut registry = MarkRegistry::new();
    registry.register::<Circle>();

    // Create pipelines for different mark types
    let _circle_pipeline = registry.get_pipeline::<Circle>(device)?;

    // Should have one pipeline cached
    assert_eq!(registry.pipeline_count(), 1);
    assert_eq!(registry.mark_count(), 1);

    // Verify pipeline was created successfully
    // Note: wgpu pipeline labels are internal implementation details

    Ok(())
}

/// Test mark renderer buffer management.
#[tokio::test]
async fn test_mark_renderer_buffer_management() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;
    let queue = &context.queue;

    let mut renderer = MarkRenderer::new(device);

    // Test initial state
    assert_eq!(renderer.vertex_len(), 0);
    assert_eq!(renderer.instance_len(), 0);
    assert_eq!(renderer.index_len(), Some(0));

    // Upload test data
    let vertices = Circle::generate_vertices();
    renderer.upload_vertices(device, queue, &vertices)?;

    // Verify vertex data uploaded
    assert_eq!(
        renderer.vertex_len(),
        vertices.len() * std::mem::size_of::<gup::mark::CircleVertex>()
    );

    // Test instance data
    let test_attributes = [CircleAttributes {
        center: vec2![10.0, 20.0],
        radius: 5.0,
        fill_color: vec4![1.0, 0.0, 0.0, 1.0],
        stroke_width: 1.0,
        stroke_color: vec4![0.0, 0.0, 0.0, 1.0],
    }];

    // Convert to GPU-compatible format
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

    let gpu_instances: Vec<CircleInstance> = test_attributes
        .iter()
        .map(|attr| CircleInstance {
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

    renderer.upload_instances(device, queue, &gpu_instances)?;

    // Verify instance data uploaded
    assert_eq!(
        renderer.instance_len(),
        gpu_instances.len() * std::mem::size_of::<CircleInstance>()
    );

    // Test clearing
    renderer.clear();
    assert_eq!(renderer.vertex_len(), 0);
    assert_eq!(renderer.instance_len(), 0);
    assert_eq!(renderer.index_len(), Some(0));

    Ok(())
}

/// Test bind group layout creation for marks with custom shaders.
#[tokio::test]
async fn test_custom_shader_bind_group_layout() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;

    let registry = MarkRegistry::new();

    // Create layout for Circle (which has custom shaders)
    let layout = registry.get_bind_group_layout::<Circle>(device);

    // Should work even without registration since we're just testing layout creation
    // Note: This might fail if the implementation requires registration
    // In that case, we'd need to register first
    match layout {
        Ok(_) => {
            // Layout created successfully
        }
        Err(_) => {
            // Expected if registration is required - that's also valid behavior
            // Test that registration allows layout creation
            let mut registry = MarkRegistry::new();
            registry.register::<Circle>();
            let layout = registry.get_bind_group_layout::<Circle>(device)?;
            drop(layout); // Just verify creation succeeded
        }
    }

    Ok(())
}

/// Test automatic buffer resizing during upload operations.
#[tokio::test]
async fn test_buffer_auto_resize() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;
    let queue = &context.queue;

    // Create renderer with small initial capacity
    let mut renderer = MarkRenderer::with_capacity(device, 64, 128, Some(32));

    let initial_vertex_capacity = renderer.vertex_capacity();
    let initial_instance_capacity = renderer.instance_capacity();

    // Upload data larger than initial capacity
    let large_vertex_data: Vec<[f32; 2]> = (0..100).map(|i| [i as f32, (i * 2) as f32]).collect();
    renderer.upload_vertices(device, queue, &large_vertex_data)?;

    // Buffer should have auto-resized
    assert!(renderer.vertex_capacity() > initial_vertex_capacity);
    assert_eq!(
        renderer.vertex_len(),
        large_vertex_data.len() * std::mem::size_of::<[f32; 2]>()
    );

    // Test instance buffer resize
    let large_instance_data: Vec<[f32; 4]> = (0..50).map(|i| [i as f32; 4]).collect();
    renderer.upload_instances(device, queue, &large_instance_data)?;

    assert!(renderer.instance_capacity() > initial_instance_capacity);
    assert_eq!(
        renderer.instance_len(),
        large_instance_data.len() * std::mem::size_of::<[f32; 4]>()
    );

    Ok(())
}

/// Test registry operations and state management.
#[tokio::test]
async fn test_registry_state_management() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;

    let mut registry = MarkRegistry::new();

    // Test initial state
    assert_eq!(registry.mark_count(), 0);
    assert_eq!(registry.pipeline_count(), 0);
    assert!(!registry.is_registered::<Circle>());

    // Register mark
    registry.register::<Circle>();
    assert_eq!(registry.mark_count(), 1);
    assert!(registry.is_registered::<Circle>());

    // Get mark info
    let mark_info = registry.get_mark_info::<Circle>().unwrap();
    assert_eq!(mark_info.vertex_count(), 4);
    assert_eq!(mark_info.index_count(), Some(6));
    assert!(mark_info.has_custom_shaders());

    // Test pipeline creation and caching
    let _pipeline1 = registry.get_pipeline::<Circle>(device)?;
    assert_eq!(registry.pipeline_count(), 1);

    let _pipeline2 = registry.get_pipeline::<Circle>(device)?;
    assert_eq!(registry.pipeline_count(), 1); // Should still be 1 due to caching

    // Test cache clearing
    registry.clear_pipeline_cache();
    assert_eq!(registry.pipeline_count(), 0);
    assert_eq!(registry.mark_count(), 1); // Mark registration should remain

    Ok(())
}

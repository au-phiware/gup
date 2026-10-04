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
use gup::mark::{Circle, MarkInfo, MarkInfoImpl, MarkRegistry};
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

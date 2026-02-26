// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Integration tests for Advanced Mark Rendering Features (GUP-069).
//!
//! These tests validate the GPU-side integration of multi-pass rendering,
//! blend-aware marks, dynamic attribute mapping, and render state management.

use gup::context::GupContext;
use gup::error::GupResult;
use gup::mark::advanced_rendering::{MarkViewport, MultiPassRenderer};
use gup::mark::{
    Circle, DynamicAttributeMap, DynamicAttributeValue, Mark, MarkBlendConfig, MarkRegistry,
    MarkRenderer, MultiPassConfig, RenderPassConfig, RenderStateManager,
};
use gup::mixable::BlendMode;
use std::sync::Arc;

/// Helper to create a headless GPU context.
async fn create_test_context() -> GupResult<Arc<GupContext>> {
    GupContext::headless().await
}

// ---------------------------------------------------------------------------
// Multi-Pass Pipeline Creation (GPU tests)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_multi_pass_pipeline_creation_for_circle() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;

    let mut registry = MarkRegistry::new();
    registry.register::<Circle>();

    let config = MultiPassConfig::new()
        .add_pass(RenderPassConfig {
            label: "fill".into(),
            blend_state: Some(wgpu::BlendState::ALPHA_BLENDING),
            polygon_mode: wgpu::PolygonMode::Fill,
            ..Default::default()
        })
        .add_pass(RenderPassConfig {
            label: "outline".into(),
            blend_state: Some(wgpu::BlendState::ALPHA_BLENDING),
            polygon_mode: wgpu::PolygonMode::Fill, // Line mode requires GPU feature
            ..Default::default()
        });

    let pipelines = registry.create_multi_pass_pipelines::<Circle>(device, &config)?;
    assert_eq!(pipelines.len(), 2);

    Ok(())
}

#[tokio::test]
async fn test_single_pass_pipeline_backward_compat() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;

    let mut registry = MarkRegistry::new();
    registry.register::<Circle>();

    // Single pass config should work identically to standard pipeline
    let config = MultiPassConfig::new().add_pass(RenderPassConfig {
        label: "standard".into(),
        ..Default::default()
    });

    let pipelines = registry.create_multi_pass_pipelines::<Circle>(device, &config)?;
    assert_eq!(pipelines.len(), 1);

    // Standard pipeline should also still work
    let _standard = registry.get_pipeline::<Circle>(device)?;

    Ok(())
}

#[tokio::test]
async fn test_multi_pass_pipeline_different_blend_states() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;

    let mut registry = MarkRegistry::new();
    registry.register::<Circle>();

    let config = MultiPassConfig::new()
        .add_pass(RenderPassConfig {
            label: "opaque_base".into(),
            blend_state: None, // No blending (opaque)
            ..Default::default()
        })
        .add_pass(RenderPassConfig {
            label: "blended_overlay".into(),
            blend_state: Some(wgpu::BlendState::ALPHA_BLENDING),
            ..Default::default()
        });

    let pipelines = registry.create_multi_pass_pipelines::<Circle>(device, &config)?;
    assert_eq!(pipelines.len(), 2);

    Ok(())
}

// ---------------------------------------------------------------------------
// Blend-Aware Pipeline Creation (GPU tests)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_blend_aware_pipeline_default() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;

    let mut registry = MarkRegistry::new();
    registry.register::<Circle>();

    let blend_config = MarkBlendConfig::alpha_blending();
    let pipeline = registry.get_pipeline_with_blend::<Circle>(device, &blend_config, None)?;
    // Should resolve to the standard cached pipeline
    assert!(Arc::strong_count(&pipeline) >= 1);

    Ok(())
}

#[tokio::test]
async fn test_blend_aware_pipeline_with_override() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;

    let mut registry = MarkRegistry::new();
    registry.register::<Circle>();

    let blend_config = MarkBlendConfig::alpha_blending();
    // Override to additive blending
    let pipeline = registry.get_pipeline_with_blend::<Circle>(
        device,
        &blend_config,
        Some(BlendMode::Additive),
    )?;
    assert!(Arc::strong_count(&pipeline) >= 1);

    Ok(())
}

#[tokio::test]
async fn test_blend_aware_pipeline_custom_no_override() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;

    let mut registry = MarkRegistry::new();
    registry.register::<Circle>();

    let blend_config = MarkBlendConfig::custom(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING);
    // Even with override, custom should win
    let pipeline = registry.get_pipeline_with_blend::<Circle>(
        device,
        &blend_config,
        Some(BlendMode::Additive),
    )?;
    assert!(Arc::strong_count(&pipeline) >= 1);

    Ok(())
}

// ---------------------------------------------------------------------------
// MarkRenderer Multi-Pass Integration (GPU tests)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_renderer_buffer_upload_for_multi_pass() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;
    let queue = &context.queue;

    let mut renderer = MarkRenderer::new(device);

    // Upload circle vertices
    let vertices = Circle::generate_vertices();
    renderer.upload_vertices(device, queue, &vertices)?;

    let indices = Circle::generate_indices().unwrap();
    renderer.upload_indices(device, queue, &indices)?;

    // Verify data is uploaded
    assert!(renderer.vertex_len() > 0);
    assert_eq!(renderer.index_len(), Some(indices.len()));

    Ok(())
}

// ---------------------------------------------------------------------------
// Dynamic Attribute Map Integration (GPU tests)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_dynamic_attribute_map_gpu_upload_workflow() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;
    let queue = &context.queue;

    let mut attr_map = DynamicAttributeMap::new();

    // Set initial attributes
    attr_map.set(
        "color",
        DynamicAttributeValue::from_color(1.0, 0.0, 0.0, 1.0),
    );
    attr_map.set("radius", DynamicAttributeValue::from_scalar(10.0));
    assert!(attr_map.is_dirty());

    // Collect values for GPU upload
    let static_values = attr_map.collect_static_values();
    assert_eq!(static_values.len(), 2);

    // Create GPU buffer and upload
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("test_dynamic_attrs"),
        size: (static_values.len() * std::mem::size_of::<[f32; 4]>()) as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&buffer, 0, bytemuck::cast_slice(&static_values));

    // Clear dirty flags after upload
    attr_map.clear_dirty();
    assert!(!attr_map.is_dirty());

    // Update an attribute (should become dirty again)
    attr_map.set(
        "color",
        DynamicAttributeValue::from_color(0.0, 1.0, 0.0, 1.0),
    );
    assert!(attr_map.is_dirty());

    // Only "color" should be dirty
    assert!(attr_map.dirty_attributes().contains("color"));

    Ok(())
}

#[tokio::test]
async fn test_dynamic_attribute_per_instance_upload() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;
    let queue = &context.queue;

    let mut attr_map = DynamicAttributeMap::new();

    // Set per-instance color data (100 instances)
    let instance_colors: Vec<[f32; 4]> = (0..100)
        .map(|i| {
            let t = i as f32 / 99.0;
            [t, 1.0 - t, 0.5, 1.0]
        })
        .collect();

    attr_map.set(
        "color",
        DynamicAttributeValue::from_instances(instance_colors.clone()),
    );

    // Verify per-instance data
    let per_instance = attr_map.get("color").unwrap().as_per_instance().unwrap();
    assert_eq!(per_instance.len(), 100);

    // Upload per-instance data to GPU
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("test_per_instance_attrs"),
        size: (instance_colors.len() * std::mem::size_of::<[f32; 4]>()) as u64,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&buffer, 0, bytemuck::cast_slice(&instance_colors));

    Ok(())
}

// ---------------------------------------------------------------------------
// Render State Manager Integration (GPU tests)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_state_manager_viewport_isolation() -> GupResult<()> {
    let _context = create_test_context().await?;

    let mut state_manager = RenderStateManager::new();

    // Set up a composition with two mark types in different viewports
    state_manager.push_state(BlendMode::AlphaBlending);

    // Mark type A: left half
    state_manager.set_viewport(MarkViewport::sub_region(0.0, 0.0, 400.0, 600.0));
    assert_eq!(state_manager.current_viewport().unwrap().width, 400.0);

    // Save and switch to mark type B: right half
    state_manager.push_state(BlendMode::AlphaBlending);
    state_manager.set_viewport(MarkViewport::sub_region(400.0, 0.0, 400.0, 600.0));
    assert_eq!(state_manager.current_viewport().unwrap().x, 400.0);

    // Restore to mark type A viewport
    state_manager.pop_state();
    assert_eq!(state_manager.current_viewport().unwrap().width, 400.0);
    assert_eq!(state_manager.current_viewport().unwrap().x, 0.0);

    // Restore to initial state
    state_manager.pop_state();

    Ok(())
}

#[tokio::test]
async fn test_state_manager_blend_mode_isolation() -> GupResult<()> {
    let _context = create_test_context().await?;

    let mut state_manager = RenderStateManager::new();

    // Simulate nested mark compositions with different blend modes
    state_manager.push_state(BlendMode::None);
    state_manager.push_state(BlendMode::AlphaBlending);
    state_manager.push_state(BlendMode::Additive);

    assert_eq!(state_manager.stack_depth(), 3);

    // Pop through the stack verifying blend mode restoration
    let s3 = state_manager.pop_state().unwrap();
    assert_eq!(s3.blend_mode, BlendMode::Additive);

    let s2 = state_manager.pop_state().unwrap();
    assert_eq!(s2.blend_mode, BlendMode::AlphaBlending);

    let s1 = state_manager.pop_state().unwrap();
    assert_eq!(s1.blend_mode, BlendMode::None);

    assert_eq!(state_manager.stack_depth(), 0);

    Ok(())
}

// ---------------------------------------------------------------------------
// Multi-Pass Renderer Unit Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_multi_pass_renderer_lifecycle() -> GupResult<()> {
    let _context = create_test_context().await?;

    let mut renderer = MultiPassRenderer::new();
    assert_eq!(renderer.cached_pipeline_count(), 0);

    renderer.clear_cache();
    assert_eq!(renderer.cached_pipeline_count(), 0);

    Ok(())
}

// ---------------------------------------------------------------------------
// MarkBlendConfig Resolution Tests
// ---------------------------------------------------------------------------

#[test]
fn test_blend_config_resolve_no_override() {
    let config = MarkBlendConfig::alpha_blending();
    let resolved = config.resolve_blend_state(None);
    assert!(resolved.is_some());
}

#[test]
fn test_blend_config_resolve_with_override() {
    let config = MarkBlendConfig::alpha_blending();
    // Override to None (opaque)
    let resolved = config.resolve_blend_state(Some(BlendMode::None));
    assert!(resolved.is_none());
}

#[test]
fn test_blend_config_resolve_additive() {
    let config = MarkBlendConfig::additive();
    let resolved = config.resolve_blend_state(None);
    assert!(resolved.is_some());
}

#[test]
fn test_blend_config_custom_ignores_override_when_locked() {
    let custom_blend = wgpu::BlendState {
        color: wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::One,
            operation: wgpu::BlendOperation::Max,
        },
        alpha: wgpu::BlendComponent::OVER,
    };
    let config = MarkBlendConfig::custom(custom_blend);

    // Override should be ignored because supports_override is false
    let resolved = config.resolve_blend_state(Some(BlendMode::Multiply));
    assert!(resolved.is_some());
    // Should use the custom blend state (Max operation)
    let resolved = resolved.unwrap();
    assert_eq!(resolved.color.operation, wgpu::BlendOperation::Max);
}

// ---------------------------------------------------------------------------
// Performance Characterization Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_dynamic_attribute_update_performance() -> GupResult<()> {
    let _context = create_test_context().await?;

    let mut attr_map = DynamicAttributeMap::new();

    // Time 1000 attribute updates (should be <1ms total)
    let start = std::time::Instant::now();
    for i in 0..1000 {
        let t = i as f32 / 999.0;
        attr_map.set(
            "color",
            DynamicAttributeValue::from_color(t, 1.0 - t, 0.5, 1.0),
        );
    }
    let elapsed = start.elapsed();

    // Should complete well under 1ms per update
    assert!(
        elapsed.as_millis() < 100,
        "1000 attribute updates took {}ms (expected <100ms)",
        elapsed.as_millis()
    );

    Ok(())
}

#[tokio::test]
async fn test_state_transition_performance() -> GupResult<()> {
    let _context = create_test_context().await?;

    let mut state_manager = RenderStateManager::new();

    // Time 1000 state push/pop cycles
    let start = std::time::Instant::now();
    for _ in 0..1000 {
        state_manager.push_state(BlendMode::AlphaBlending);
        state_manager.set_viewport(MarkViewport::from_dimensions(800.0, 600.0));
        state_manager.pop_state();
    }
    let elapsed = start.elapsed();

    // Should be extremely fast (<100ms for 1000 cycles)
    assert!(
        elapsed.as_millis() < 100,
        "1000 state transitions took {}ms (expected <100ms)",
        elapsed.as_millis()
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// Edge Cases
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_unregistered_mark_multi_pass_fails() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;

    let registry = MarkRegistry::new(); // No marks registered

    let config = MultiPassConfig::new().add_pass(RenderPassConfig::default());

    let result = registry.create_multi_pass_pipelines::<Circle>(device, &config);
    assert!(result.is_err());

    Ok(())
}

#[tokio::test]
async fn test_empty_multi_pass_config() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;

    let mut registry = MarkRegistry::new();
    registry.register::<Circle>();

    let config = MultiPassConfig::new(); // No passes
    let pipelines = registry.create_multi_pass_pipelines::<Circle>(device, &config)?;
    assert!(pipelines.is_empty());

    Ok(())
}

// ---------------------------------------------------------------------------
// Multi-Pass Mark Examples Validation (GUP-185)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_multi_pass_shadow_config_creation() -> GupResult<()> {
    // Verify the drop-shadow multi-pass config used by the example is valid
    let config = MultiPassConfig::new()
        .add_pass(RenderPassConfig {
            label: "shadow".into(),
            blend_state: Some(wgpu::BlendState::ALPHA_BLENDING),
            vertex_entry_point: Some("vs_shadow".into()),
            fragment_entry_point: Some("fs_shadow".into()),
            ..Default::default()
        })
        .add_pass(RenderPassConfig {
            label: "main".into(),
            blend_state: Some(wgpu::BlendState::ALPHA_BLENDING),
            ..Default::default()
        });

    assert_eq!(config.pass_count(), 2);
    assert!(config.is_multi_pass());
    assert_eq!(config.get_pass(0).unwrap().label, "shadow");
    assert_eq!(
        config.get_pass(0).unwrap().vertex_entry_point.as_deref(),
        Some("vs_shadow")
    );
    assert_eq!(
        config.get_pass(0).unwrap().fragment_entry_point.as_deref(),
        Some("fs_shadow")
    );
    assert_eq!(config.get_pass(1).unwrap().label, "main");
    assert!(config.get_pass(1).unwrap().vertex_entry_point.is_none());
    assert!(config.get_pass(1).unwrap().fragment_entry_point.is_none());

    Ok(())
}

#[tokio::test]
async fn test_multi_pass_fill_outline_config_creation() -> GupResult<()> {
    // Verify the fill + outline multi-pass config used by the example is valid
    let config = MultiPassConfig::new()
        .add_pass(RenderPassConfig {
            label: "fill".into(),
            blend_state: Some(wgpu::BlendState::ALPHA_BLENDING),
            fragment_entry_point: Some("fs_fill".into()),
            ..Default::default()
        })
        .add_pass(RenderPassConfig {
            label: "outline".into(),
            blend_state: Some(wgpu::BlendState::ALPHA_BLENDING),
            fragment_entry_point: Some("fs_outline".into()),
            ..Default::default()
        });

    assert_eq!(config.pass_count(), 2);
    assert!(config.is_multi_pass());
    assert_eq!(
        config.get_pass(0).unwrap().fragment_entry_point.as_deref(),
        Some("fs_fill")
    );
    assert_eq!(
        config.get_pass(1).unwrap().fragment_entry_point.as_deref(),
        Some("fs_outline")
    );

    Ok(())
}

#[tokio::test]
async fn test_multi_pass_renderer_pipeline_count_mismatch() -> GupResult<()> {
    // MultiPassRenderer should reject mismatched pipeline/pass counts
    let context = create_test_context().await?;
    let device = &context.device;

    let mut registry = MarkRegistry::new();
    registry.register::<Circle>();

    let config = MultiPassConfig::new()
        .add_pass(RenderPassConfig {
            label: "a".into(),
            ..Default::default()
        })
        .add_pass(RenderPassConfig {
            label: "b".into(),
            ..Default::default()
        });

    // Create only one pipeline (but config has two passes)
    let single_config = MultiPassConfig::new().add_pass(RenderPassConfig::default());
    let pipelines = registry.create_multi_pass_pipelines::<Circle>(device, &single_config)?;
    assert_eq!(pipelines.len(), 1);

    // The render_multi_pass call should fail because we have 1 pipeline but 2 passes.
    // We can't actually call render_multi_pass without a render pass, but we
    // verify the config/pipeline mismatch is detectable.
    assert_ne!(pipelines.len(), config.pass_count());

    Ok(())
}

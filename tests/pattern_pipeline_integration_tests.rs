// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Integration tests for Pattern Pipeline Integration (GUP-155).
//!
//! These tests validate the integration of pattern rendering into the mark pipeline,
//! including pattern-enabled pipeline creation, bind group management, and rendering
//! with patterns for accessibility.

use gup::accessibility::{Color, Pattern, PatternRenderer, PatternUniforms};
use gup::buffer::{BufferType, GpuBuffer};
use gup::context::GupContext;
use gup::error::GupResult;
use gup::mark::{Circle, Mark, MarkInfo, MarkInfoImpl, MarkRenderer};
use std::sync::Arc;

/// Helper function to create test context for GPU operations.
async fn create_test_context() -> GupResult<Arc<GupContext>> {
    GupContext::headless().await
}

/// Test that marks can report if they have pattern shader support.
#[tokio::test]
async fn test_mark_has_pattern_shader() -> GupResult<()> {
    let mark_info = MarkInfoImpl::<Circle>::new();
    
    // Circle should have pattern shader support
    assert!(mark_info.has_pattern_shader());
    
    Ok(())
}

/// Test that pattern pipelines can be created successfully.
#[tokio::test]
async fn test_pattern_pipeline_creation() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;
    
    let mark_info = MarkInfoImpl::<Circle>::new();
    let _pipeline = mark_info.create_render_pipeline_with_patterns(device)?;
    
    // Pipeline should be created successfully
    Ok(())
}

/// Test that pattern renderer can be created and uniforms updated.
#[tokio::test]
async fn test_pattern_renderer_creation() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;
    let queue = &context.queue;
    
    // Create pattern uniforms
    let pattern = Pattern::Dots { spacing: 8.0 };
    let uniforms = PatternUniforms::from_pattern(&pattern, Color::BLACK, Color::WHITE);
    
    // Create pattern renderer
    let mut renderer = PatternRenderer::new(device, uniforms);
    
    // Update with different pattern
    let new_pattern = Pattern::Lines {
        spacing: 6.0,
        angle: std::f32::consts::PI / 4.0,
    };
    let new_uniforms = PatternUniforms::from_pattern(&new_pattern, Color::RED, Color::BLUE);
    renderer.update(queue, new_uniforms);
    
    // Verify bind group is accessible
    let _bind_group = renderer.bind_group();
    
    Ok(())
}

/// Test that pattern mode can be toggled in mark renderer.
#[tokio::test]
async fn test_pattern_mode_toggle() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;
    
    let mark_info = MarkInfoImpl::<Circle>::new();
    
    // Create both standard and pattern pipelines
    let _standard_pipeline = mark_info.create_render_pipeline(device)?;
    let _pattern_pipeline = mark_info.create_render_pipeline_with_patterns(device)?;
    
    // Both should be created successfully without conflicts
    Ok(())
}

/// Test complete rendering workflow with patterns.
#[tokio::test]
async fn test_complete_pattern_rendering_workflow() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;
    let queue = &context.queue;
    
    // Create pattern renderer
    let pattern = Pattern::Dots { spacing: 8.0 };
    let uniforms = PatternUniforms::from_pattern(&pattern, Color::BLACK, Color::WHITE);
    let pattern_renderer = PatternRenderer::new(device, uniforms);
    
    // Create mark renderer
    let mut mark_renderer = MarkRenderer::new(device);
    
    // Upload vertex data
    let vertices = Circle::generate_vertices();
    mark_renderer.upload_vertices(device, queue, &vertices)?;
    
    // Upload test instance data
    #[repr(C)]
    #[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
    struct CircleInstanceData {
        center: [f32; 2],
        radius: f32,
        fill_color: [f32; 4],
        stroke_width: f32,
        stroke_color: [f32; 4],
        _padding: [f32; 2],
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
    
    mark_renderer.upload_instances(device, queue, &test_instances)?;
    
    // Upload index data if needed
    if let Some(indices) = Circle::generate_indices() {
        mark_renderer.upload_indices(device, queue, &indices)?;
    }
    
    // Verify all data was uploaded correctly
    assert!(mark_renderer.vertex_len() > 0);
    assert!(mark_renderer.instance_len() > 0);
    if Circle::index_count().is_some() {
        assert!(mark_renderer.index_len().unwrap_or(0) > 0);
    }
    
    // Verify pattern bind group is accessible
    let _pattern_bind_group = pattern_renderer.bind_group();
    
    Ok(())
}

/// Test that different pattern types generate correct uniforms.
#[tokio::test]
async fn test_pattern_type_uniforms() -> GupResult<()> {
    let patterns = vec![
        (Pattern::Solid, 0u32),
        (Pattern::Dots { spacing: 8.0 }, 1u32),
        (Pattern::Lines { spacing: 6.0, angle: 0.0 }, 2u32),
        (Pattern::Crosshatch { spacing: 8.0 }, 3u32),
    ];
    
    for (pattern, expected_type_id) in patterns {
        let uniforms = PatternUniforms::from_pattern(&pattern, Color::BLACK, Color::WHITE);
        assert_eq!(uniforms.pattern_type, expected_type_id, 
            "Pattern type ID mismatch for {:?}", pattern);
    }
    
    Ok(())
}

/// Test that pattern parameters are correctly transferred to uniforms.
#[tokio::test]
async fn test_pattern_parameters() -> GupResult<()> {
    // Test dots pattern
    let dots = Pattern::Dots { spacing: 10.0 };
    let dots_uniforms = PatternUniforms::from_pattern(&dots, Color::RED, Color::BLUE);
    assert_eq!(dots_uniforms.spacing, 10.0);
    assert_eq!(dots_uniforms.foreground_color, [1.0, 0.0, 0.0, 1.0]);
    assert_eq!(dots_uniforms.background_color, [0.0, 0.0, 1.0, 1.0]);
    
    // Test lines pattern
    let lines = Pattern::Lines {
        spacing: 6.0,
        angle: std::f32::consts::PI / 4.0,
    };
    let lines_uniforms = PatternUniforms::from_pattern(&lines, Color::BLACK, Color::WHITE);
    assert_eq!(lines_uniforms.spacing, 6.0);
    assert_eq!(lines_uniforms.angle, std::f32::consts::PI / 4.0);
    
    // Test crosshatch pattern
    let crosshatch = Pattern::Crosshatch { spacing: 8.0 };
    let crosshatch_uniforms = PatternUniforms::from_pattern(&crosshatch, Color::BLACK, Color::WHITE);
    assert_eq!(crosshatch_uniforms.spacing, 8.0);
    
    Ok(())
}

/// Test that pattern pipelines have correct bind group layout structure.
#[tokio::test]
async fn test_pattern_bind_group_layout() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;
    
    let mark_info = MarkInfoImpl::<Circle>::new();
    
    // Creating the pipeline should succeed with proper bind group layouts
    let _pipeline = mark_info.create_render_pipeline_with_patterns(device)?;
    
    // If we got here, bind group layouts were created successfully
    Ok(())
}

/// Test performance overhead of pattern rendering setup.
#[tokio::test]
async fn test_pattern_rendering_performance() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;
    
    // Time standard pipeline creation
    let start = std::time::Instant::now();
    let mark_info = MarkInfoImpl::<Circle>::new();
    let _standard_pipeline = mark_info.create_render_pipeline(device)?;
    let standard_duration = start.elapsed();
    
    // Time pattern pipeline creation
    let start = std::time::Instant::now();
    let _pattern_pipeline = mark_info.create_render_pipeline_with_patterns(device)?;
    let pattern_duration = start.elapsed();
    
    // Pattern pipeline should not take significantly longer (within 3x)
    // Only check if standard duration is non-zero to avoid division by zero
    if standard_duration.as_nanos() > 0 {
        let overhead_ratio = pattern_duration.as_secs_f64() / standard_duration.as_secs_f64();
        assert!(overhead_ratio < 3.0, 
            "Pattern pipeline creation overhead too high: {:.2}x", overhead_ratio);
    }
    
    // Both pipelines should be created quickly (< 100ms)
    assert!(standard_duration.as_millis() < 100, "Standard pipeline creation too slow");
    assert!(pattern_duration.as_millis() < 100, "Pattern pipeline creation too slow");
    
    Ok(())
}

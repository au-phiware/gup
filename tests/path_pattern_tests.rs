// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Integration tests for Path Mark Pattern Support (GUP-158).
//!
//! These tests validate that pattern rendering works correctly on Path marks.

use gup::accessibility::{Color, Pattern, PatternRenderer, PatternUniforms};
use gup::context::GupContext;
use gup::error::GupResult;
use gup::mark::{Mark, MarkInfo, MarkInfoImpl, Path};
use std::sync::Arc;

/// Helper function to create test context for GPU operations.
async fn create_test_context() -> GupResult<Arc<GupContext>> {
    GupContext::headless().await
}

/// Test that Path mark has pattern shader support.
#[tokio::test]
async fn test_path_has_pattern_shader() -> GupResult<()> {
    let mark_info = MarkInfoImpl::<Path>::new();
    assert!(
        mark_info.has_pattern_shader(),
        "Path should have pattern shader support"
    );
    Ok(())
}

/// Test that pattern shader is distinct from standard shader (when implemented).
#[tokio::test]
async fn test_path_pattern_shader_exists() -> GupResult<()> {
    // Verify that Path has a pattern fragment shader defined
    assert!(
        Path::PATTERN_FRAGMENT_SHADER.is_some(),
        "Path should have a pattern fragment shader"
    );
    Ok(())
}

/// Test that pattern pipelines can be created for Path.
#[tokio::test]
async fn test_path_pattern_pipeline_creation() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;

    let mark_info = MarkInfoImpl::<Path>::new();
    let _pipeline = mark_info.create_render_pipeline_with_patterns(device)?;

    Ok(())
}

/// Test that all pattern types work with Path mark.
#[tokio::test]
async fn test_path_all_pattern_types() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;
    let queue = &context.queue;

    let mark_info = MarkInfoImpl::<Path>::new();
    let _pipeline = mark_info.create_render_pipeline_with_patterns(device)?;

    // Test all pattern types
    let patterns = vec![
        Pattern::Solid,
        Pattern::Dots { spacing: 8.0 },
        Pattern::Lines {
            spacing: 6.0,
            angle: 0.0,
        },
        Pattern::Crosshatch { spacing: 8.0 },
    ];

    for pattern in patterns {
        let uniforms = PatternUniforms::from_pattern(&pattern, Color::BLACK, Color::WHITE);
        let mut renderer = PatternRenderer::new(device, uniforms);

        // Update should work without errors
        let new_uniforms = PatternUniforms::from_pattern(&pattern, Color::RED, Color::BLUE);
        renderer.update(queue, new_uniforms);
    }

    Ok(())
}

/// Test that pattern lines work at different angles.
#[tokio::test]
async fn test_path_pattern_lines_angles() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;
    let queue = &context.queue;

    let mark_info = MarkInfoImpl::<Path>::new();
    let _pipeline = mark_info.create_render_pipeline_with_patterns(device)?;

    // Test different angles for line patterns
    let angles = vec![0.0, std::f32::consts::PI / 4.0, std::f32::consts::PI / 2.0];

    for angle in angles {
        let pattern = Pattern::Lines {
            spacing: 6.0,
            angle,
        };
        let uniforms = PatternUniforms::from_pattern(&pattern, Color::BLACK, Color::WHITE);
        let mut renderer = PatternRenderer::new(device, uniforms);

        let new_uniforms = PatternUniforms::from_pattern(&pattern, Color::RED, Color::BLUE);
        renderer.update(queue, new_uniforms);
    }

    Ok(())
}

/// Test that pattern dots work at different spacings.
#[tokio::test]
async fn test_path_pattern_dots_spacing() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;
    let queue = &context.queue;

    let mark_info = MarkInfoImpl::<Path>::new();
    let _pipeline = mark_info.create_render_pipeline_with_patterns(device)?;

    // Test different spacings for dot patterns
    let spacings = vec![4.0, 8.0, 12.0];

    for spacing in spacings {
        let pattern = Pattern::Dots { spacing };
        let uniforms = PatternUniforms::from_pattern(&pattern, Color::BLACK, Color::WHITE);
        let mut renderer = PatternRenderer::new(device, uniforms);

        let new_uniforms = PatternUniforms::from_pattern(&pattern, Color::RED, Color::BLUE);
        renderer.update(queue, new_uniforms);
    }

    Ok(())
}

/// Test that both standard and pattern pipelines can coexist for Path.
#[tokio::test]
async fn test_path_dual_pipeline_support() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;

    let mark_info = MarkInfoImpl::<Path>::new();

    // Create both pipelines - they should coexist without conflicts
    let _standard_pipeline = mark_info.create_render_pipeline(device)?;
    let _pattern_pipeline = mark_info.create_render_pipeline_with_patterns(device)?;

    Ok(())
}

/// Test that pattern renderer can be created and updated for Path.
#[tokio::test]
async fn test_path_pattern_renderer_lifecycle() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;
    let queue = &context.queue;

    // Create initial pattern
    let pattern1 = Pattern::Dots { spacing: 8.0 };
    let uniforms1 = PatternUniforms::from_pattern(&pattern1, Color::BLACK, Color::WHITE);
    let mut renderer = PatternRenderer::new(device, uniforms1);

    // Update to different pattern
    let pattern2 = Pattern::Lines {
        spacing: 6.0,
        angle: std::f32::consts::PI / 4.0,
    };
    let uniforms2 = PatternUniforms::from_pattern(&pattern2, Color::RED, Color::BLUE);
    renderer.update(queue, uniforms2);

    // Update to crosshatch
    let pattern3 = Pattern::Crosshatch { spacing: 10.0 };
    let uniforms3 = PatternUniforms::from_pattern(&pattern3, Color::GREEN, Color::YELLOW);
    renderer.update(queue, uniforms3);

    Ok(())
}

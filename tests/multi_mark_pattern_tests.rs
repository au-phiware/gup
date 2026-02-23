// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Integration tests for Multi-Mark Pattern Support (GUP-157).
//!
//! These tests validate that pattern rendering works correctly across all mark types:
//! circles, rectangles, lines, and boxplots.

use gup::accessibility::{Color, Pattern, PatternRenderer, PatternUniforms};
use gup::context::GupContext;
use gup::error::GupResult;
use gup::mark::{BoxPlot, Circle, Line, Mark, MarkInfo, MarkInfoImpl, Rectangle};
use std::sync::Arc;

/// Helper function to create test context for GPU operations.
async fn create_test_context() -> GupResult<Arc<GupContext>> {
    GupContext::headless().await
}

/// Test that Circle mark has pattern shader support.
#[tokio::test]
async fn test_circle_has_pattern_shader() -> GupResult<()> {
    let mark_info = MarkInfoImpl::<Circle>::new();
    assert!(
        mark_info.has_pattern_shader(),
        "Circle should have pattern shader support"
    );
    Ok(())
}

/// Test that Rectangle mark has pattern shader support.
#[tokio::test]
async fn test_rectangle_has_pattern_shader() -> GupResult<()> {
    let mark_info = MarkInfoImpl::<Rectangle>::new();
    assert!(
        mark_info.has_pattern_shader(),
        "Rectangle should have pattern shader support"
    );
    Ok(())
}

/// Test that Line mark has pattern shader support.
#[tokio::test]
async fn test_line_has_pattern_shader() -> GupResult<()> {
    let mark_info = MarkInfoImpl::<Line>::new();
    assert!(
        mark_info.has_pattern_shader(),
        "Line should have pattern shader support"
    );
    Ok(())
}

/// Test that BoxPlot mark has pattern shader support.
#[tokio::test]
async fn test_boxplot_has_pattern_shader() -> GupResult<()> {
    let mark_info = MarkInfoImpl::<BoxPlot>::new();
    assert!(
        mark_info.has_pattern_shader(),
        "BoxPlot should have pattern shader support"
    );
    Ok(())
}

/// Test that pattern pipelines can be created for Rectangle.
#[tokio::test]
async fn test_rectangle_pattern_pipeline_creation() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;

    let mark_info = MarkInfoImpl::<Rectangle>::new();
    let _pipeline = mark_info.create_render_pipeline_with_patterns(device)?;

    Ok(())
}

/// Test that pattern pipelines can be created for Line.
#[tokio::test]
async fn test_line_pattern_pipeline_creation() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;

    let mark_info = MarkInfoImpl::<Line>::new();
    let _pipeline = mark_info.create_render_pipeline_with_patterns(device)?;

    Ok(())
}

/// Test that pattern pipelines can be created for BoxPlot.
#[tokio::test]
async fn test_boxplot_pattern_pipeline_creation() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;

    let mark_info = MarkInfoImpl::<BoxPlot>::new();
    let _pipeline = mark_info.create_render_pipeline_with_patterns(device)?;

    Ok(())
}

/// Test that all pattern types work with Rectangle mark.
#[tokio::test]
async fn test_rectangle_all_pattern_types() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;
    let queue = &context.queue;

    let mark_info = MarkInfoImpl::<Rectangle>::new();
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

/// Test that all pattern types work with Line mark.
#[tokio::test]
async fn test_line_all_pattern_types() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;
    let queue = &context.queue;

    let mark_info = MarkInfoImpl::<Line>::new();
    let _pipeline = mark_info.create_render_pipeline_with_patterns(device)?;

    // Test all pattern types
    let patterns = vec![
        Pattern::Solid,
        Pattern::Dots { spacing: 8.0 },
        Pattern::Lines {
            spacing: 6.0,
            angle: std::f32::consts::PI / 4.0,
        },
        Pattern::Crosshatch { spacing: 8.0 },
    ];

    for pattern in patterns {
        let uniforms = PatternUniforms::from_pattern(&pattern, Color::BLACK, Color::WHITE);
        let mut renderer = PatternRenderer::new(device, uniforms);

        let new_uniforms = PatternUniforms::from_pattern(&pattern, Color::RED, Color::BLUE);
        renderer.update(queue, new_uniforms);
    }

    Ok(())
}

/// Test that all pattern types work with BoxPlot mark.
#[tokio::test]
async fn test_boxplot_all_pattern_types() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;
    let queue = &context.queue;

    let mark_info = MarkInfoImpl::<BoxPlot>::new();
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

        let new_uniforms = PatternUniforms::from_pattern(&pattern, Color::RED, Color::BLUE);
        renderer.update(queue, new_uniforms);
    }

    Ok(())
}

/// Test that pattern shaders are distinct from standard shaders.
#[tokio::test]
async fn test_pattern_shaders_distinct_from_standard() -> GupResult<()> {
    // Verify that pattern shaders are different from standard shaders
    assert_ne!(
        Circle::FRAGMENT_SHADER,
        Circle::PATTERN_FRAGMENT_SHADER,
        "Circle pattern shader should be different from standard shader"
    );

    assert_ne!(
        Rectangle::FRAGMENT_SHADER,
        Rectangle::PATTERN_FRAGMENT_SHADER,
        "Rectangle pattern shader should be different from standard shader"
    );

    assert_ne!(
        Line::FRAGMENT_SHADER,
        Line::PATTERN_FRAGMENT_SHADER,
        "Line pattern shader should be different from standard shader"
    );

    assert_ne!(
        BoxPlot::FRAGMENT_SHADER,
        BoxPlot::PATTERN_FRAGMENT_SHADER,
        "BoxPlot pattern shader should be different from standard shader"
    );

    Ok(())
}

/// Test that both standard and pattern pipelines can coexist.
#[tokio::test]
async fn test_dual_pipeline_support() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;

    // Test for all mark types
    let marks: Vec<Box<dyn MarkInfo>> = vec![
        Box::new(MarkInfoImpl::<Circle>::new()),
        Box::new(MarkInfoImpl::<Rectangle>::new()),
        Box::new(MarkInfoImpl::<Line>::new()),
        Box::new(MarkInfoImpl::<BoxPlot>::new()),
    ];

    for mark_info in marks {
        // Create both pipelines - they should coexist without conflicts
        let _standard_pipeline = mark_info.create_render_pipeline(device)?;
        let _pattern_pipeline = mark_info.create_render_pipeline_with_patterns(device)?;
    }

    Ok(())
}

/// Test consistent pattern behavior across mark types.
#[tokio::test]
async fn test_consistent_pattern_behavior() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;
    let queue = &context.queue;

    // Create pattern renderer with dots pattern
    let pattern = Pattern::Dots { spacing: 10.0 };
    let uniforms = PatternUniforms::from_pattern(&pattern, Color::BLACK, Color::WHITE);

    // Create renderer once
    let mut renderer = PatternRenderer::new(device, uniforms);

    // Verify bind group is accessible
    let _bind_group = renderer.bind_group();

    // Update with different pattern
    let new_pattern = Pattern::Crosshatch { spacing: 12.0 };
    let new_uniforms = PatternUniforms::from_pattern(&new_pattern, Color::RED, Color::BLUE);
    renderer.update(queue, new_uniforms);

    // Bind group should still be accessible after update
    let _updated_bind_group = renderer.bind_group();

    Ok(())
}

/// Test that pattern spacing is applied consistently.
#[tokio::test]
async fn test_pattern_spacing_consistency() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;

    // Test different spacing values
    let spacings = vec![4.0, 6.0, 8.0, 10.0, 12.0];

    for spacing in spacings {
        let pattern = Pattern::Dots { spacing };
        let uniforms = PatternUniforms::from_pattern(&pattern, Color::BLACK, Color::WHITE);

        // Should create renderer successfully for each spacing
        let _renderer = PatternRenderer::new(device, uniforms);
    }

    Ok(())
}

/// Test that pattern angles work correctly for line patterns.
#[tokio::test]
async fn test_pattern_angle_variations() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;

    // Test different angles
    let angles = vec![
        0.0,
        std::f32::consts::PI / 6.0, // 30 degrees
        std::f32::consts::PI / 4.0, // 45 degrees
        std::f32::consts::PI / 3.0, // 60 degrees
        std::f32::consts::PI / 2.0, // 90 degrees
    ];

    for angle in angles {
        let pattern = Pattern::Lines {
            spacing: 6.0,
            angle,
        };
        let uniforms = PatternUniforms::from_pattern(&pattern, Color::BLACK, Color::WHITE);

        // Should create renderer successfully for each angle
        let _renderer = PatternRenderer::new(device, uniforms);
    }

    Ok(())
}

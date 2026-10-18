// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Integration tests for the axis system.
//!
//! These tests verify that the axis system integrates correctly with chart builders
//! and provides the expected functionality for axis rendering and configuration.

use gup::axis::{Axis, AxisBounds, AxisConfiguration, AxisPosition, LinearAxis};
use gup::chart_builder::ChartConfig;
use gup::shader_function::Vec2;

#[tokio::test]
async fn test_axis_configuration_defaults() {
    let config = AxisConfiguration::default();
    assert!(config.show_line);
    assert!(config.show_major_ticks);
    assert!(!config.show_minor_ticks);
    assert_eq!(config.major_tick_length, 6.0);
    assert_eq!(config.minor_tick_length, 3.0);
    assert_eq!(config.line_color, [0.2, 0.2, 0.2, 1.0]);
    assert_eq!(config.line_width, 1.0);
}

#[tokio::test]
async fn test_axis_configuration_builder_methods() {
    let config = AxisConfiguration::default()
        .with_color([1.0, 0.0, 0.0, 1.0])
        .with_line_width(2.0)
        .with_tick_lengths(8.0, 4.0)
        .without_minor_ticks()
        .without_line();

    assert_eq!(config.line_color, [1.0, 0.0, 0.0, 1.0]);
    assert_eq!(config.line_width, 2.0);
    assert_eq!(config.major_tick_length, 8.0);
    assert_eq!(config.minor_tick_length, 4.0);
    assert!(!config.show_minor_ticks);
    assert!(!config.show_line);
}

#[tokio::test]
async fn test_axis_position_properties() {
    assert!(AxisPosition::Top.is_horizontal());
    assert!(AxisPosition::Bottom.is_horizontal());
    assert!(AxisPosition::Left.is_vertical());
    assert!(AxisPosition::Right.is_vertical());

    assert!(!AxisPosition::Top.is_vertical());
    assert!(!AxisPosition::Bottom.is_vertical());
    assert!(!AxisPosition::Left.is_horizontal());
    assert!(!AxisPosition::Right.is_horizontal());
}

#[tokio::test]
async fn test_axis_bounds_calculations() {
    let start = Vec2 { x: 0.0, y: 0.0 };
    let end = Vec2 { x: 100.0, y: 0.0 };
    let bounds = AxisBounds::new(start, end, 50.0);

    assert_eq!(bounds.length(), 100.0);
    assert_eq!(bounds.available_margin, 50.0);

    let direction = bounds.direction();
    assert!((direction.x - 1.0).abs() < 0.001);
    assert!((direction.y - 0.0).abs() < 0.001);

    let normal = bounds.normal();
    assert!((normal.x - 0.0).abs() < 0.001);
    assert!((normal.y - 1.0).abs() < 0.001);
}

#[tokio::test]
async fn test_axis_bounds_vertical() {
    let start = Vec2 { x: 0.0, y: 100.0 };
    let end = Vec2 { x: 0.0, y: 0.0 };
    let bounds = AxisBounds::new(start, end, 30.0);

    assert_eq!(bounds.length(), 100.0);

    let direction = bounds.direction();
    assert!((direction.x - 0.0).abs() < 0.001);
    assert!((direction.y - (-1.0)).abs() < 0.001);

    let normal = bounds.normal();
    assert!((normal.x - 1.0).abs() < 0.001);
    assert!((normal.y - 0.0).abs() < 0.001);
}

#[tokio::test]
async fn test_linear_axis_creation() {
    let config = AxisConfiguration::default();
    let axis = LinearAxis::new(AxisPosition::Bottom, config.clone());

    assert_eq!(axis.position(), AxisPosition::Bottom);
    assert_eq!(axis.configuration().line_width, config.line_width);
    assert_eq!(axis.configuration().line_color, config.line_color);
}

#[tokio::test]
async fn test_linear_axis_with_position() {
    let axis = LinearAxis::with_position(AxisPosition::Left);
    assert_eq!(axis.position(), AxisPosition::Left);
    assert!(axis.configuration().show_line);
    assert!(axis.configuration().show_major_ticks);
}

#[tokio::test]
async fn test_linear_axis_margin_calculation() {
    let axis = LinearAxis::with_position(AxisPosition::Bottom);
    let margin = axis.calculate_margin(None);

    // Should include base margin plus tick length plus padding
    assert!(margin > 40.0); // Base margin for horizontal axis

    let left_axis = LinearAxis::with_position(AxisPosition::Left);
    let left_margin = left_axis.calculate_margin(None);
    assert!(left_margin > 60.0); // Base margin for vertical axis
}

#[tokio::test]
async fn test_linear_axis_tick_positions() {
    let axis = LinearAxis::with_position(AxisPosition::Bottom);
    let positions = axis.get_tick_positions(None, 800.0);

    // Should have basic tick positions from 0 to 1
    assert_eq!(positions.len(), 6);
    assert_eq!(positions[0], 0.0);
    assert_eq!(positions[5], 1.0);

    // Should be evenly spaced
    for i in 1..positions.len() {
        let spacing = positions[i] - positions[i - 1];
        assert!((spacing - 0.2).abs() < 0.001);
    }
}

#[tokio::test]
async fn test_linear_axis_configuration_update() {
    let mut axis = LinearAxis::with_position(AxisPosition::Top);
    let new_config = AxisConfiguration::default().with_color([0.0, 1.0, 0.0, 1.0]);

    axis.set_configuration(new_config);
    assert_eq!(axis.configuration().line_color, [0.0, 1.0, 0.0, 1.0]);
}

#[tokio::test]
async fn test_chart_config_axis_integration() {
    let config = ChartConfig::default();
    assert!(config.show_axes); // Default should show axes
    assert!(!config.show_grid); // Grid off by default
}

#[tokio::test]
async fn test_multiple_axis_positions() {
    // Test all axis position combinations
    for position in [
        AxisPosition::Top,
        AxisPosition::Bottom,
        AxisPosition::Left,
        AxisPosition::Right,
    ] {
        let axis = LinearAxis::with_position(position);
        assert_eq!(axis.position(), position);

        let margin = axis.calculate_margin(None);
        match position {
            AxisPosition::Left | AxisPosition::Right => assert!(margin >= 60.0),
            AxisPosition::Top | AxisPosition::Bottom => assert!(margin >= 40.0),
        }
    }
}

#[tokio::test]
async fn test_axis_bounds_edge_cases() {
    // Zero-length axis
    let start = Vec2 { x: 50.0, y: 50.0 };
    let end = Vec2 { x: 50.0, y: 50.0 };
    let bounds = AxisBounds::new(start, end, 20.0);

    assert_eq!(bounds.length(), 0.0);

    // Should provide default direction for zero-length axis
    let direction = bounds.direction();
    assert_eq!(direction.x, 1.0);
    assert_eq!(direction.y, 0.0);

    // Diagonal axis
    let start = Vec2 { x: 0.0, y: 0.0 };
    let end = Vec2 { x: 3.0, y: 4.0 };
    let bounds = AxisBounds::new(start, end, 10.0);

    assert_eq!(bounds.length(), 5.0); // 3-4-5 triangle

    let direction = bounds.direction();
    assert!((direction.x - 0.6).abs() < 0.001);
    assert!((direction.y - 0.8).abs() < 0.001);
}

#[tokio::test]
async fn test_axis_configuration_without_methods() {
    let config = AxisConfiguration::default().without_ticks().without_line();

    assert!(!config.show_line);
    assert!(!config.show_major_ticks);
    assert!(!config.show_minor_ticks);

    // Other properties should remain unchanged
    assert_eq!(config.major_tick_length, 6.0);
    assert_eq!(config.minor_tick_length, 3.0);
    assert_eq!(config.line_color, [0.2, 0.2, 0.2, 1.0]);
    assert_eq!(config.line_width, 1.0);
}

// Performance and memory tests
#[tokio::test]
#[ignore = "wall-clock budget: unreliable on shared CI runners; run with --ignored on a hardware GPU (GUP-398)"]
async fn test_axis_performance_with_large_tick_count() {
    let axis = LinearAxis::with_position(AxisPosition::Bottom);

    // This test ensures tick position calculation is efficient
    let start = std::time::Instant::now();
    let positions = axis.get_tick_positions(None, 800.0);
    let duration = start.elapsed();

    // Performance: tick positions should be fast.
    // Debug builds are slower; use generous thresholds.
    #[cfg(debug_assertions)]
    let threshold_ms: u128 = 50;
    #[cfg(not(debug_assertions))]
    let threshold_ms: u128 = 10;

    assert!(
        duration.as_millis() < threshold_ms,
        "Tick position calculation took {duration:?} (threshold: {threshold_ms}ms)"
    );
    assert!(!positions.is_empty());

    // Verify positions are valid
    for &pos in &positions {
        assert!((0.0..=1.0).contains(&pos));
    }
}

// Copyright (C) 2025 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for storage buffer-based ColorGradientStorage (GUP-134).
//!
//! Validates:
//! - Unlimited color stop support
//! - Builder pattern API
//! - Preset gradients
//! - Buffer data generation
//! - WGSL code generation

use gup::*;

#[test]
fn test_basic_gradient_creation() {
    let colors = vec![vec4![0.0, 0.0, 0.0, 1.0], vec4![1.0, 1.0, 1.0, 1.0]];
    let stops = vec![0.0, 1.0];

    let gradient = ColorGradientStorage::new(colors.clone(), stops.clone());

    assert_eq!(gradient.colors.len(), 2);
    assert_eq!(gradient.stops.len(), 2);
    assert_eq!(gradient.count(), 2);
}

#[test]
fn test_gradient_with_colors() {
    let colors = vec![
        vec4![1.0, 0.0, 0.0, 1.0],
        vec4![0.0, 1.0, 0.0, 1.0],
        vec4![0.0, 0.0, 1.0, 1.0],
    ];

    let gradient = ColorGradientStorage::with_colors(colors);

    assert_eq!(gradient.colors.len(), 3);
    assert_eq!(gradient.stops.len(), 3);
    assert_eq!(gradient.stops[0], 0.0);
    assert_eq!(gradient.stops[1], 0.5);
    assert_eq!(gradient.stops[2], 1.0);
}

#[test]
fn test_builder_pattern() {
    let gradient = ColorGradientStorage::builder()
        .add_rgb(0.0, 1.0, 0.0, 0.0)
        .add_rgb(0.5, 0.0, 1.0, 0.0)
        .add_rgb(1.0, 0.0, 0.0, 1.0)
        .build();

    assert_eq!(gradient.colors.len(), 3);
    assert_eq!(gradient.stops[0], 0.0);
    assert_eq!(gradient.stops[1], 0.5);
    assert_eq!(gradient.stops[2], 1.0);
}

#[test]
fn test_builder_unsorted_stops() {
    // Builder should sort stops by position
    let gradient = ColorGradientStorage::builder()
        .add_rgb(1.0, 0.0, 0.0, 1.0)
        .add_rgb(0.0, 1.0, 0.0, 0.0)
        .add_rgb(0.5, 0.0, 1.0, 0.0)
        .build();

    assert_eq!(gradient.stops[0], 0.0);
    assert_eq!(gradient.stops[1], 0.5);
    assert_eq!(gradient.stops[2], 1.0);

    // Check colors were reordered correctly
    assert_eq!(gradient.colors[0].x, 1.0); // Red at 0.0
    assert_eq!(gradient.colors[1].y, 1.0); // Green at 0.5
    assert_eq!(gradient.colors[2].z, 1.0); // Blue at 1.0
}

#[test]
fn test_builder_rgba() {
    let gradient = ColorGradientStorage::builder()
        .add_rgba(0.0, 1.0, 0.0, 0.0, 0.5)
        .add_rgba(1.0, 0.0, 0.0, 1.0, 1.0)
        .build();

    assert_eq!(gradient.colors[0].w, 0.5);
    assert_eq!(gradient.colors[1].w, 1.0);
}

#[test]
fn test_viridis_preset() {
    let gradient = ColorGradientStorage::viridis();

    assert_eq!(gradient.colors.len(), 11);
    assert_eq!(gradient.stops.len(), 11);
    assert!(gradient.stops[0] == 0.0);
    assert!(gradient.stops[10] == 1.0);
}

#[test]
fn test_plasma_preset() {
    let gradient = ColorGradientStorage::plasma();

    assert_eq!(gradient.colors.len(), 11);
    assert!(gradient.colors[0].z > 0.5); // Starts with blue
    assert!(gradient.colors[10].y > 0.9); // Ends with bright yellow
}

#[test]
fn test_inferno_preset() {
    let gradient = ColorGradientStorage::inferno();

    assert_eq!(gradient.colors.len(), 11);
    // Inferno starts very dark
    assert!(gradient.colors[0].x < 0.1);
    assert!(gradient.colors[0].y < 0.1);
    assert!(gradient.colors[0].z < 0.1);
}

#[test]
fn test_rainbow_preset() {
    let gradient = ColorGradientStorage::rainbow();

    assert_eq!(gradient.colors.len(), 7); // ROYGBIV
    assert_eq!(gradient.colors[0].x, 1.0); // Red
    assert_eq!(gradient.colors[3].y, 1.0); // Green
    assert_eq!(gradient.colors[4].z, 1.0); // Blue
}

#[test]
fn test_cool_warm_preset() {
    let gradient = ColorGradientStorage::cool_warm();

    assert_eq!(gradient.colors.len(), 5);
    assert_eq!(gradient.colors[0].z, 1.0); // Blue
    assert_eq!(gradient.colors[4].x, 1.0); // Red
}

#[test]
fn test_grayscale_preset() {
    let gradient = ColorGradientStorage::grayscale();

    assert_eq!(gradient.colors.len(), 2);
    assert_eq!(gradient.colors[0], vec4![0.0, 0.0, 0.0, 1.0]); // Black
    assert_eq!(gradient.colors[1], vec4![1.0, 1.0, 1.0, 1.0]); // White
}

#[test]
fn test_many_stops() {
    // Test with 100+ stops to verify unlimited support
    let colors: Vec<Vec4> = (0..150)
        .map(|i| {
            let t = i as f32 / 149.0;
            vec4![t, 1.0 - t, 0.5, 1.0]
        })
        .collect();

    let gradient = ColorGradientStorage::with_colors(colors);

    assert_eq!(gradient.colors.len(), 150);
    assert_eq!(gradient.stops.len(), 150);
    assert_eq!(gradient.count(), 150);
}

#[test]
fn test_colors_buffer_data() {
    let gradient = ColorGradientStorage::builder()
        .add_rgb(0.0, 1.0, 0.0, 0.0)
        .add_rgb(1.0, 0.0, 1.0, 0.0)
        .build();

    let data = gradient.create_colors_buffer_data();

    // Each vec4 is 16 bytes (4 floats)
    assert_eq!(data.len(), 2 * 16);

    // Verify first color (red)
    let r = f32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    assert_eq!(r, 1.0);
}

#[test]
fn test_stops_buffer_data() {
    let gradient = ColorGradientStorage::builder()
        .add_rgb(0.0, 1.0, 0.0, 0.0)
        .add_rgb(0.5, 0.0, 1.0, 0.0)
        .add_rgb(1.0, 0.0, 0.0, 1.0)
        .build();

    let data = gradient.create_stops_buffer_data();

    // Each f32 is 4 bytes
    assert_eq!(data.len(), 3 * 4);

    // Verify stop values
    let stop0 = f32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let stop1 = f32::from_le_bytes([data[4], data[5], data[6], data[7]]);
    let stop2 = f32::from_le_bytes([data[8], data[9], data[10], data[11]]);

    assert_eq!(stop0, 0.0);
    assert_eq!(stop1, 0.5);
    assert_eq!(stop2, 1.0);
}

#[test]
fn test_wgsl_struct_definition() {
    let definition = ColorGradientStorage::wgsl_struct_definition();

    assert!(definition.contains("ColorGradientStorage"));
    assert!(definition.contains("gradient_colors"));
    assert!(definition.contains("gradient_stops"));
    assert!(definition.contains("var<storage, read>"));
}

#[test]
fn test_wgsl_function() {
    let function = ColorGradientStorage::wgsl_function();

    assert!(function.contains("fn color_gradient_storage"));
    assert!(function.contains("clamp"));
    assert!(function.contains("mix"));
    assert!(function.contains("// Binary search"));
}

#[test]
#[should_panic(expected = "Colors and stops must have same length")]
fn test_mismatched_colors_and_stops() {
    let colors = vec![vec4![1.0, 0.0, 0.0, 1.0]];
    let stops = vec![0.0, 1.0];

    ColorGradientStorage::new(colors, stops);
}

#[test]
#[should_panic(expected = "Must have at least one color")]
fn test_empty_gradient() {
    ColorGradientStorage::new(vec![], vec![]);
}

#[test]
#[should_panic(expected = "Stop position must be between 0.0 and 1.0")]
fn test_invalid_stop_position() {
    ColorGradientStorage::builder()
        .add_rgb(1.5, 1.0, 0.0, 0.0)
        .build();
}

#[test]
#[should_panic(expected = "Gradient must have at least one stop")]
fn test_builder_empty() {
    ColorGradientStorage::builder().build();
}

#[test]
fn test_single_color_gradient() {
    let gradient = ColorGradientStorage::with_colors(vec![vec4![1.0, 0.0, 0.0, 1.0]]);

    assert_eq!(gradient.colors.len(), 1);
    assert_eq!(gradient.stops[0], 0.0);
}

#[test]
fn test_gradient_cloning() {
    let gradient1 = ColorGradientStorage::viridis();
    let gradient2 = gradient1.clone();

    assert_eq!(gradient1.colors.len(), gradient2.colors.len());
    assert_eq!(gradient1.stops, gradient2.stops);
}

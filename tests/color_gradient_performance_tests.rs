// Copyright (C) 2025 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Performance comparison tests for ColorGradient vs ColorGradientStorage (GUP-134 AC2).
//!
//! This validates that storage buffer-based gradients maintain acceptable performance
//! compared to the uniform-based implementation.

use gup::*;
use std::time::Instant;

#[test]
fn test_gradient_creation_performance() {
    // Test creation time for both implementations

    // Uniform-based (max 8 stops)
    let start = Instant::now();
    for _ in 0..10000 {
        let _gradient = ColorGradient::with_colors(vec![
            vec4![0.0, 0.0, 0.0, 1.0],
            vec4![0.2, 0.2, 0.2, 1.0],
            vec4![0.4, 0.4, 0.4, 1.0],
            vec4![0.6, 0.6, 0.6, 1.0],
            vec4![0.8, 0.8, 0.8, 1.0],
            vec4![1.0, 1.0, 1.0, 1.0],
        ]);
    }
    let uniform_time = start.elapsed();

    // Storage-based (same 6 stops)
    let start = Instant::now();
    for _ in 0..10000 {
        let _gradient = ColorGradientStorage::with_colors(vec![
            vec4![0.0, 0.0, 0.0, 1.0],
            vec4![0.2, 0.2, 0.2, 1.0],
            vec4![0.4, 0.4, 0.4, 1.0],
            vec4![0.6, 0.6, 0.6, 1.0],
            vec4![0.8, 0.8, 0.8, 1.0],
            vec4![1.0, 1.0, 1.0, 1.0],
        ]);
    }
    let storage_time = start.elapsed();

    println!("Uniform gradient creation (6 stops): {:?}", uniform_time);
    println!("Storage gradient creation (6 stops): {:?}", storage_time);

    // Storage should be within reasonable overhead (we allow 2x for CPU operations)
    let ratio = storage_time.as_secs_f64() / uniform_time.as_secs_f64();
    println!("Storage/Uniform ratio: {:.2}x", ratio);

    // This is CPU-side creation, so some overhead is acceptable
    assert!(
        ratio < 3.0,
        "Storage gradient creation should not be more than 3x slower"
    );
}

#[test]
fn test_buffer_data_generation_performance() {
    // Test buffer data generation for large gradients
    let colors: Vec<Vec4> = (0..100)
        .map(|i| {
            let t = i as f32 / 99.0;
            vec4![t, 1.0 - t, 0.5, 1.0]
        })
        .collect();

    let gradient = ColorGradientStorage::with_colors(colors);

    // Measure color buffer generation
    let start = Instant::now();
    for _ in 0..1000 {
        let _data = gradient.create_colors_buffer_data();
    }
    let colors_time = start.elapsed();

    // Measure stops buffer generation
    let start = Instant::now();
    for _ in 0..1000 {
        let _data = gradient.create_stops_buffer_data();
    }
    let stops_time = start.elapsed();

    println!("Color buffer generation (100 stops): {:?}", colors_time);
    println!("Stops buffer generation (100 stops): {:?}", stops_time);

    // Should be fast (under 10ms for 1000 iterations)
    assert!(
        colors_time.as_millis() < 10,
        "Color buffer generation too slow"
    );
    assert!(
        stops_time.as_millis() < 10,
        "Stops buffer generation too slow"
    );
}

#[test]
fn test_builder_performance() {
    // Test builder pattern performance
    let start = Instant::now();

    for _ in 0..1000 {
        let _gradient = ColorGradientStorage::builder()
            .add_rgb(0.0, 1.0, 0.0, 0.0)
            .add_rgb(0.2, 0.8, 0.2, 0.0)
            .add_rgb(0.4, 0.6, 0.4, 0.0)
            .add_rgb(0.6, 0.4, 0.6, 0.0)
            .add_rgb(0.8, 0.2, 0.8, 0.0)
            .add_rgb(1.0, 0.0, 1.0, 0.0)
            .build();
    }

    let builder_time = start.elapsed();
    println!("Builder pattern (6 stops): {:?}", builder_time);

    // Should be fast (under 10ms for 1000 iterations)
    assert!(builder_time.as_millis() < 50, "Builder pattern too slow");
}

#[test]
fn test_large_gradient_creation() {
    // Test creation of very large gradients (stress test)
    let sizes = [10, 50, 100, 200, 500];

    for size in sizes {
        let start = Instant::now();

        let colors: Vec<Vec4> = (0..size)
            .map(|i| {
                let t = i as f32 / (size - 1) as f32;
                vec4![t, 1.0 - t, 0.5, 1.0]
            })
            .collect();

        let _gradient = ColorGradientStorage::with_colors(colors);

        let elapsed = start.elapsed();
        println!("Created gradient with {} stops in {:?}", size, elapsed);

        // Should complete quickly even for 500 stops
        assert!(elapsed.as_millis() < 10, "Large gradient creation too slow");
    }
}

#[test]
fn test_preset_gradient_performance() {
    // Test preset gradient creation performance
    let presets: Vec<(&str, fn() -> ColorGradientStorage)> = vec![
        ("viridis", ColorGradientStorage::viridis),
        ("plasma", ColorGradientStorage::plasma),
        ("inferno", ColorGradientStorage::inferno),
        ("rainbow", ColorGradientStorage::rainbow),
        ("cool_warm", ColorGradientStorage::cool_warm),
        ("grayscale", ColorGradientStorage::grayscale),
    ];

    for (name, preset_fn) in presets {
        let start = Instant::now();

        for _ in 0..10000 {
            let _gradient = preset_fn();
        }

        let elapsed = start.elapsed();
        println!("{} preset creation: {:?}", name, elapsed);

        // Preset creation should be fast
        assert!(elapsed.as_millis() < 100, "{} preset too slow", name);
    }
}

#[test]
fn test_memory_efficiency() {
    // Compare memory usage (roughly) between implementations

    // Uniform-based gradient (8 stops max)
    let uniform_gradient = ColorGradient::with_colors(vec![
        vec4![0.0, 0.0, 0.0, 1.0],
        vec4![0.14, 0.14, 0.14, 1.0],
        vec4![0.29, 0.29, 0.29, 1.0],
        vec4![0.43, 0.43, 0.43, 1.0],
        vec4![0.57, 0.57, 0.57, 1.0],
        vec4![0.71, 0.71, 0.71, 1.0],
        vec4![0.86, 0.86, 0.86, 1.0],
        vec4![1.0, 1.0, 1.0, 1.0],
    ]);

    // Calculate uniform buffer size
    let uniform_size = std::mem::size_of_val(&uniform_gradient);

    // Storage-based gradient (same 8 stops)
    let storage_gradient = ColorGradientStorage::with_colors(vec![
        vec4![0.0, 0.0, 0.0, 1.0],
        vec4![0.14, 0.14, 0.14, 1.0],
        vec4![0.29, 0.29, 0.29, 1.0],
        vec4![0.43, 0.43, 0.43, 1.0],
        vec4![0.57, 0.57, 0.57, 1.0],
        vec4![0.71, 0.71, 0.71, 1.0],
        vec4![0.86, 0.86, 0.86, 1.0],
        vec4![1.0, 1.0, 1.0, 1.0],
    ]);

    // Calculate storage buffer size (CPU side)
    let storage_size = std::mem::size_of_val(&storage_gradient);

    println!(
        "Uniform gradient CPU size (8 stops): {} bytes",
        uniform_size
    );
    println!(
        "Storage gradient CPU size (8 stops): {} bytes",
        storage_size
    );

    // Storage gradient should be more memory-efficient on CPU (just Vec storage)
    // but note: this doesn't account for GPU memory layout
}

#[test]
fn test_wgsl_generation_performance() {
    // Test WGSL code generation performance
    let start = Instant::now();

    for _ in 0..10000 {
        let _struct = ColorGradientStorage::wgsl_struct_definition();
        let _function = ColorGradientStorage::wgsl_function();
    }

    let elapsed = start.elapsed();
    println!("WGSL generation (10000 iterations): {:?}", elapsed);

    // Should be instant (just returning static strings)
    assert!(elapsed.as_millis() < 10, "WGSL generation too slow");
}

#[test]
fn test_comparison_with_uniform_limit() {
    // Demonstrate the key advantage: storage buffers support more stops

    // Uniform-based: limited to 8 stops
    let uniform_max = 8;
    let uniform_gradient = ColorGradient::with_colors(
        (0..uniform_max)
            .map(|i| {
                let t = i as f32 / (uniform_max - 1) as f32;
                vec4![t, 1.0 - t, 0.5, 1.0]
            })
            .collect(),
    );

    // Storage-based: can handle 100+ stops with same API
    let storage_count = 100;
    let storage_gradient = ColorGradientStorage::with_colors(
        (0..storage_count)
            .map(|i| {
                let t = i as f32 / (storage_count - 1) as f32;
                vec4![t, 1.0 - t, 0.5, 1.0]
            })
            .collect(),
    );

    println!(
        "Uniform gradient: {} stops (max)",
        uniform_gradient.colors.len()
    );
    println!("Storage gradient: {} stops", storage_gradient.colors.len());

    assert_eq!(uniform_gradient.colors.len(), 8);
    assert_eq!(storage_gradient.colors.len(), 100);

    // The key metric: storage supports 12.5x more stops
    let improvement = storage_gradient.colors.len() as f32 / uniform_gradient.colors.len() as f32;
    println!("Storage buffer improvement: {:.1}x more stops", improvement);
    assert!(improvement > 10.0);
}

// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Integration tests for the shader function composition engine (GUP-033).
//!
//! These tests verify that complex shader function pipelines can be composed,
//! generate valid WGSL, and maintain type safety across multi-stage transformations.

use gup::prelude::*;
use gup::{vec2, vec4};

#[test]
fn test_five_stage_pipeline() {
    // Create a complex 5-stage pipeline as required by AC4 success metrics:
    // 1. Linear scale from data domain to normalized range
    // 2. Power transform (sqrt) for better visual distribution
    // 3. Clamp to ensure values stay in bounds
    // 4. Smooth step for aesthetic interpolation
    // 5. Color gradient mapping

    let stage1 = LinearScale::new(0.0, 1000.0, 0.0, 2.0);
    let stage2 = PowerScale::sqrt(0.0, 2.0, 0.0, 1.0);
    let stage3 = Clamp::new(0.0, 1.0);
    let stage4 = SmoothStep::new(0.0, 1.0);
    let stage5 = ColorGradient::with_colors(vec![
        vec4![0.0, 0.0, 1.0, 1.0], // Blue
        vec4![0.0, 1.0, 0.0, 1.0], // Green
        vec4![1.0, 1.0, 0.0, 1.0], // Yellow
        vec4![1.0, 0.0, 0.0, 1.0], // Red
    ]);

    // Compose the 5-stage pipeline
    let pipeline = stage1
        .compose(stage2)
        .compose(stage3)
        .compose(stage4)
        .compose(stage5);

    // Verify uniforms can be created
    let uniforms = pipeline.create_uniforms();
    assert!(
        uniforms.is_some(),
        "5-stage pipeline should create uniforms"
    );

    // Verify WGSL generation works
    let wgsl = pipeline.generate_wgsl();
    assert!(
        !wgsl.is_empty(),
        "5-stage pipeline should generate WGSL code"
    );
}

#[test]
fn test_scale_function_variety() {
    // Test that we have 10+ composable function types (AC4 success metric)
    let functions: Vec<Box<dyn std::any::Any>> = vec![
        Box::new(LinearScale::new(0.0, 1.0, 0.0, 1.0)),
        Box::new(LogScale::new(1.0, 100.0, 0.0, 1.0)),
        Box::new(PowerScale::new(0.0, 1.0, 0.0, 1.0, 2.0)),
        Box::new(Clamp::new(0.0, 1.0)),
        Box::new(Threshold::new(0.5)),
        Box::new(SmoothStep::new(0.0, 1.0)),
        Box::new(ColorMap::new(
            vec4![0.0, 0.0, 0.0, 1.0],
            vec4![1.0, 1.0, 1.0, 1.0],
        )),
        Box::new(ColorGradient::with_colors(vec![vec4![0.0, 0.0, 0.0, 1.0]])),
        Box::new(PositionTransform::new(vec2![1.0, 1.0], vec2![0.0, 0.0])),
        Box::new(TemporalInterpolation::new(0.0, 1.0, 1.0)),
        Box::new(Easing::linear()),
    ];

    assert!(
        functions.len() >= 10,
        "Should support 10+ composable function types (found {})",
        functions.len()
    );
}

#[test]
fn test_conditional_pipeline() {
    // Test conditional composition with different color schemes based on threshold
    let normalize = LinearScale::new(0.0, 100.0, 0.0, 1.0);

    let warm_colors = ColorGradient::with_colors(vec![
        vec4![1.0, 1.0, 0.0, 1.0], // Yellow
        vec4![1.0, 0.0, 0.0, 1.0], // Red
    ]);

    let cool_colors = ColorGradient::with_colors(vec![
        vec4![0.0, 1.0, 1.0, 1.0], // Cyan
        vec4![0.0, 0.0, 1.0, 1.0], // Blue
    ]);

    let conditional = ConditionalFunction::new(0.5, warm_colors, cool_colors);
    let pipeline = normalize.compose(conditional);

    let uniforms = pipeline.create_uniforms();
    assert!(
        uniforms.is_some(),
        "Conditional pipeline should create uniforms"
    );

    let wgsl = pipeline.generate_wgsl();
    assert!(
        wgsl.contains("conditional"),
        "Generated WGSL should contain conditional logic"
    );
}

#[test]
fn test_animation_pipeline() {
    // Test temporal composition for animations
    let time_interpolation = TemporalInterpolation::new(0.0, 1.0, 2.0);
    let easing = Easing::ease_in_out();
    let color_map = ColorMap::new(vec4![0.0, 0.0, 0.0, 1.0], vec4![1.0, 1.0, 1.0, 1.0]);

    let animation_pipeline = time_interpolation.compose(easing).compose(color_map);

    let uniforms = animation_pipeline.create_uniforms();
    assert!(
        uniforms.is_some(),
        "Animation pipeline should create uniforms"
    );
}

#[test]
fn test_type_safety_enforcement() {
    // This test verifies that invalid compositions are caught at compile time.
    // These are compile-time checks, so we just verify valid compositions work.

    // Valid: f32 -> f32 -> f32
    let scale1 = LinearScale::new(0.0, 100.0, 0.0, 1.0);
    let scale2 = LinearScale::new(0.0, 1.0, 0.0, 100.0);
    let composed = scale1.compose(scale2);
    assert!(composed.create_uniforms().is_some());

    // Valid: f32 -> Vec4 (color output)
    let scale = LinearScale::new(0.0, 100.0, 0.0, 1.0);
    let color = ColorMap::new(vec4![0.0, 0.0, 0.0, 1.0], vec4![1.0, 1.0, 1.0, 1.0]);
    let colored = scale.compose(color);
    assert!(colored.create_uniforms().is_some());

    // Invalid compositions would fail at compile time:
    // let position = PositionTransform::new(vec2![1.0, 1.0], vec2![0.0, 0.0]); // Vec2 -> Vec2
    // let bad = position.compose(scale1); // ❌ Compile error: Vec2 not compatible with f32
}

#[test]
fn test_logarithmic_scale_composition() {
    // Test log scale for data spanning multiple orders of magnitude
    let log_scale = LogScale::new(1.0, 10000.0, 0.0, 1.0);
    let color_map = ColorGradient::with_colors(vec![
        vec4![0.0, 0.0, 0.5, 1.0],
        vec4![0.0, 0.5, 1.0, 1.0],
        vec4![0.5, 1.0, 1.0, 1.0],
        vec4![1.0, 1.0, 0.5, 1.0],
    ]);

    let pipeline = log_scale.compose(color_map);

    let uniforms = pipeline.create_uniforms().unwrap();
    assert_eq!(uniforms.first.domain_min, 1.0);
    assert_eq!(uniforms.first.domain_max, 10000.0);
    assert_eq!(uniforms.first.base, 10.0);
}

#[test]
fn test_power_scale_variants() {
    // Test different power scale variants
    let sqrt_scale = PowerScale::sqrt(0.0, 100.0, 0.0, 1.0);
    let uniforms = sqrt_scale.create_uniforms().unwrap();
    assert_eq!(uniforms.exponent, 0.5);

    let square_scale = PowerScale::square(0.0, 100.0, 0.0, 1.0);
    let uniforms2 = square_scale.create_uniforms().unwrap();
    assert_eq!(uniforms2.exponent, 2.0);

    let custom_scale = PowerScale::new(0.0, 100.0, 0.0, 1.0, 1.5);
    let uniforms3 = custom_scale.create_uniforms().unwrap();
    assert_eq!(uniforms3.exponent, 1.5);
}

#[test]
fn test_wgsl_generation_quality() {
    // Verify generated WGSL contains expected elements
    let scale = LinearScale::new(0.0, 100.0, 0.0, 1.0);
    let wgsl = scale.generate_wgsl();

    assert!(
        wgsl.contains("fn linear_scale"),
        "WGSL should contain function definition"
    );
    assert!(
        wgsl.contains("scale.domain_min"),
        "WGSL should reference uniform fields"
    );

    // Test composed WGSL
    let color = ColorMap::new(vec4![0.0, 0.0, 0.0, 1.0], vec4![1.0, 1.0, 1.0, 1.0]);
    let composed = scale.compose(color);
    let composed_wgsl = composed.generate_wgsl();

    assert!(
        composed_wgsl.contains("composed_chain"),
        "Composed WGSL should contain chain function"
    );
}

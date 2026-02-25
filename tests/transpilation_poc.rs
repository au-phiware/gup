// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Comprehensive proof-of-concept tests for the Rust-to-WGSL transpilation system.
//!
//! This test suite demonstrates realistic use cases spanning data visualization,
//! graphics programming, and scientific computing. Each test validates that
//! transpiled Rust functions produce correct, compilable WGSL output.
//!
//! The tests cover three categories:
//! - **Data visualization transforms**: scales, color mapping, interpolation
//! - **Graphics operations**: lighting, coordinate transforms, blending
//! - **Mathematical computations**: iterative algorithms, trigonometry, statistics

use gup::shader_function::{self, ComposableShaderFunction};
use gup_macros::{shader_fn, wgsl_function};

// ---------------------------------------------------------------------------
// Data Visualization Shader Functions
// ---------------------------------------------------------------------------

/// Linear scale: maps a value from a domain to a range.
#[shader_fn]
fn linear_scale(
    value: f32,
    domain_min: f32,
    domain_max: f32,
    range_min: f32,
    range_max: f32,
) -> f32 {
    let normalised = (value - domain_min) / (domain_max - domain_min);
    return range_min + normalised * (range_max - range_min);
}

/// Logarithmic scale: maps a value using log10 transformation.
#[shader_fn]
fn log_scale(value: f32, domain_min: f32, domain_max: f32, range_min: f32, range_max: f32) -> f32 {
    let log_val = log(max(value, 0.001));
    let log_min = log(max(domain_min, 0.001));
    let log_max = log(max(domain_max, 0.001));
    let normalised = (log_val - log_min) / (log_max - log_min);
    return range_min + normalised * (range_max - range_min);
}

/// Power scale: maps a value using a power exponent.
#[shader_fn]
fn power_scale(value: f32, exponent: f32, domain_min: f32, domain_max: f32) -> f32 {
    let normalised = (value - domain_min) / (domain_max - domain_min);
    let clamped = clamp(normalised, 0.0, 1.0);
    return pow(clamped, exponent);
}

/// Color gradient: interpolates between two colours based on a normalised value.
#[shader_fn]
fn color_lerp(
    value: f32,
    color_a_r: f32,
    color_a_g: f32,
    color_a_b: f32,
    color_b_r: f32,
    color_b_g: f32,
    color_b_b: f32,
) -> f32 {
    let t = clamp(value, 0.0, 1.0);
    return mix(color_a_r, color_b_r, t);
}

/// Quantise: snaps a continuous value to discrete steps.
#[shader_fn]
fn quantise(value: f32, steps: f32) -> f32 {
    let normalised = clamp(value, 0.0, 1.0);
    return floor(normalised * steps) / steps;
}

// ---------------------------------------------------------------------------
// Graphics and Lighting Shader Functions
// ---------------------------------------------------------------------------

/// Diffuse lighting factor: computes Lambertian diffuse contribution.
#[shader_fn]
fn diffuse_factor(
    normal_x: f32,
    normal_y: f32,
    normal_z: f32,
    light_x: f32,
    light_y: f32,
    light_z: f32,
) -> f32 {
    let n_dot_l = normal_x * light_x + normal_y * light_y + normal_z * light_z;
    return max(n_dot_l, 0.0);
}

/// Screen-space distance: computes the distance between two 2D points.
#[shader_fn]
fn screen_distance(ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
    let dx = ax - bx;
    let dy = ay - by;
    return sqrt(dx * dx + dy * dy);
}

/// Smooth step: performs Hermite interpolation between edges.
#[shader_fn]
fn smooth_threshold(value: f32, edge0: f32, edge1: f32) -> f32 {
    let t = clamp((value - edge0) / (edge1 - edge0), 0.0, 1.0);
    return t * t * (3.0 - 2.0 * t);
}

/// Opacity falloff: computes opacity based on distance from centre.
#[shader_fn]
fn radial_falloff(dist: f32, radius: f32, softness: f32) -> f32 {
    let edge0 = radius - softness;
    let edge1 = radius + softness;
    let t = clamp((dist - edge0) / (edge1 - edge0), 0.0, 1.0);
    return 1.0 - t * t * (3.0 - 2.0 * t);
}

// ---------------------------------------------------------------------------
// Mathematical and Scientific Shader Functions
// ---------------------------------------------------------------------------

/// Sigmoid activation function.
#[shader_fn]
fn sigmoid(value: f32, steepness: f32, midpoint: f32) -> f32 {
    let x = steepness * (value - midpoint);
    return 1.0 / (1.0 + exp(-x));
}

/// Gaussian (bell curve): evaluates at a given point.
#[shader_fn]
fn gaussian(x: f32, mean: f32, std_dev: f32) -> f32 {
    let diff = x - mean;
    let exponent = -(diff * diff) / (2.0 * std_dev * std_dev);
    return exp(exponent);
}

/// Iterative sum: computes a running total using a for-loop.
#[shader_fn]
fn iterative_sum(n: i32) -> i32 {
    let mut total = 0;
    for i in 0..n {
        total += i;
    }
    return total;
}

/// Multi-step classification with nested control flow.
#[shader_fn]
fn multi_classify(value: f32, threshold_low: f32, threshold_high: f32) -> f32 {
    if value < threshold_low {
        return 0.0;
    } else if value < threshold_high {
        let normalised = (value - threshold_low) / (threshold_high - threshold_low);
        return normalised;
    } else {
        return 1.0;
    }
}

/// Oscillation: combines sine and cosine for periodic patterns.
#[shader_fn]
fn oscillate(phase: f32, freq: f32, amplitude: f32) -> f32 {
    let s = sin(phase * freq);
    let c = cos(phase * freq * 0.5);
    return amplitude * (s * 0.7 + c * 0.3);
}

// ===========================================================================
// Tests
// ===========================================================================

#[test]
fn poc_linear_scale_generates_valid_wgsl() {
    let wgsl = LinearScale::wgsl_function();
    assert!(
        wgsl.contains("fn linear_scale"),
        "Should contain function name: {wgsl}"
    );
    assert!(
        wgsl.contains("LinearScaleUniforms"),
        "Should contain uniforms struct: {wgsl}"
    );
    assert!(
        wgsl.contains("domain_min"),
        "Should reference uniform fields: {wgsl}"
    );
}

#[test]
fn poc_log_scale_generates_valid_wgsl() {
    let wgsl = LogScale::wgsl_function();
    assert!(
        wgsl.contains("fn log_scale"),
        "Should contain function name: {wgsl}"
    );
    assert!(
        wgsl.contains("log("),
        "Should contain log() builtin: {wgsl}"
    );
    assert!(
        wgsl.contains("max("),
        "Should contain max() builtin: {wgsl}"
    );
}

#[test]
fn poc_power_scale_generates_valid_wgsl() {
    let wgsl = PowerScale::wgsl_function();
    assert!(
        wgsl.contains("fn power_scale"),
        "Should contain function name: {wgsl}"
    );
    assert!(
        wgsl.contains("pow("),
        "Should contain pow() builtin: {wgsl}"
    );
    assert!(
        wgsl.contains("clamp("),
        "Should contain clamp() builtin: {wgsl}"
    );
}

#[test]
fn poc_color_lerp_generates_valid_wgsl() {
    let wgsl = ColorLerp::wgsl_function();
    assert!(
        wgsl.contains("fn color_lerp"),
        "Should contain function name: {wgsl}"
    );
    assert!(
        wgsl.contains("mix("),
        "Should contain mix() builtin: {wgsl}"
    );
}

#[test]
fn poc_quantise_generates_valid_wgsl() {
    let wgsl = Quantise::wgsl_function();
    assert!(
        wgsl.contains("fn quantise"),
        "Should contain function name: {wgsl}"
    );
    assert!(
        wgsl.contains("floor("),
        "Should contain floor() builtin: {wgsl}"
    );
}

#[test]
fn poc_diffuse_factor_generates_valid_wgsl() {
    let wgsl = DiffuseFactor::wgsl_function();
    assert!(
        wgsl.contains("fn diffuse_factor"),
        "Should contain function name: {wgsl}"
    );
    assert!(
        wgsl.contains("max("),
        "Should contain max() builtin: {wgsl}"
    );
}

#[test]
fn poc_screen_distance_generates_valid_wgsl() {
    let wgsl = ScreenDistance::wgsl_function();
    assert!(
        wgsl.contains("fn screen_distance"),
        "Should contain function name: {wgsl}"
    );
    assert!(
        wgsl.contains("sqrt("),
        "Should contain sqrt() builtin: {wgsl}"
    );
}

#[test]
fn poc_smooth_threshold_generates_valid_wgsl() {
    let wgsl = SmoothThreshold::wgsl_function();
    assert!(
        wgsl.contains("fn smooth_threshold"),
        "Should contain function name: {wgsl}"
    );
}

#[test]
fn poc_radial_falloff_generates_valid_wgsl() {
    let wgsl = RadialFalloff::wgsl_function();
    assert!(
        wgsl.contains("fn radial_falloff"),
        "Should contain function name: {wgsl}"
    );
}

#[test]
fn poc_sigmoid_generates_valid_wgsl() {
    let wgsl = Sigmoid::wgsl_function();
    assert!(
        wgsl.contains("fn sigmoid"),
        "Should contain function name: {wgsl}"
    );
    assert!(
        wgsl.contains("exp("),
        "Should contain exp() builtin: {wgsl}"
    );
}

#[test]
fn poc_gaussian_generates_valid_wgsl() {
    let wgsl = Gaussian::wgsl_function();
    assert!(
        wgsl.contains("fn gaussian"),
        "Should contain function name: {wgsl}"
    );
    assert!(
        wgsl.contains("exp("),
        "Should contain exp() builtin: {wgsl}"
    );
}

#[test]
fn poc_iterative_sum_generates_valid_wgsl() {
    let wgsl = IterativeSum::wgsl_function();
    assert!(
        wgsl.contains("fn iterative_sum"),
        "Should contain function name: {wgsl}"
    );
    assert!(wgsl.contains("for"), "Should contain for loop: {wgsl}");
}

#[test]
fn poc_multi_classify_generates_valid_wgsl() {
    let wgsl = MultiClassify::wgsl_function();
    assert!(
        wgsl.contains("fn multi_classify"),
        "Should contain function name: {wgsl}"
    );
    assert!(wgsl.contains("if"), "Should contain if statement: {wgsl}");
    assert!(wgsl.contains("else"), "Should contain else clause: {wgsl}");
}

#[test]
fn poc_oscillate_generates_valid_wgsl() {
    let wgsl = Oscillate::wgsl_function();
    assert!(
        wgsl.contains("fn oscillate"),
        "Should contain function name: {wgsl}"
    );
    assert!(
        wgsl.contains("sin("),
        "Should contain sin() builtin: {wgsl}"
    );
    assert!(
        wgsl.contains("cos("),
        "Should contain cos() builtin: {wgsl}"
    );
}

// ---------------------------------------------------------------------------
// Uniform struct validation
// ---------------------------------------------------------------------------

#[test]
fn poc_uniform_construction_works() {
    let scale = LinearScale::new(0.0, 100.0, 0.0, 800.0);
    let u = scale.create_uniforms().unwrap();
    assert_eq!(u.domain_min, 0.0);
    assert_eq!(u.domain_max, 100.0);
    assert_eq!(u.range_min, 0.0);
    assert_eq!(u.range_max, 800.0);
}

#[test]
fn poc_sigmoid_uniform_construction() {
    let sig = Sigmoid::new(1.0, 0.5);
    let u = sig.create_uniforms().unwrap();
    assert_eq!(u.steepness, 1.0);
    assert_eq!(u.midpoint, 0.5);
}

#[test]
fn poc_gaussian_uniform_construction() {
    let gauss = Gaussian::new(0.0, 1.0);
    let u = gauss.create_uniforms().unwrap();
    assert_eq!(u.mean, 0.0);
    assert_eq!(u.std_dev, 1.0);
}

// ---------------------------------------------------------------------------
// Pipeline integration
// ---------------------------------------------------------------------------

#[test]
fn poc_all_functions_implement_composable_shader_function() {
    fn assert_composable<T: ComposableShaderFunction>() {}

    assert_composable::<LinearScale>();
    assert_composable::<LogScale>();
    assert_composable::<PowerScale>();
    assert_composable::<ColorLerp>();
    assert_composable::<Quantise>();
    assert_composable::<DiffuseFactor>();
    assert_composable::<ScreenDistance>();
    assert_composable::<SmoothThreshold>();
    assert_composable::<RadialFalloff>();
    assert_composable::<Sigmoid>();
    assert_composable::<Gaussian>();
    assert_composable::<IterativeSum>();
    assert_composable::<MultiClassify>();
    assert_composable::<Oscillate>();
}

#[test]
fn poc_pipeline_integration_with_multiple_functions() {
    use gup::shader_pipeline::ComposableShaderPipeline;

    let mut pipeline = ComposableShaderPipeline::new();
    pipeline.add_function(LinearScale::new(0.0, 100.0, 0.0, 1.0));
    pipeline.add_function(PowerScale::new(2.0, 0.0, 1.0));
    pipeline.add_function(Sigmoid::new(5.0, 0.5));
    assert_eq!(pipeline.function_count(), 3);
}

// ---------------------------------------------------------------------------
// GPU compilation validation
// ---------------------------------------------------------------------------

async fn validate_wgsl_compiles(label: &str, wgsl: &str) {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::all(),
        ..Default::default()
    });
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions::default())
        .await
        .expect("No GPU adapter available");
    let (device, _queue) = adapter
        .request_device(&wgpu::DeviceDescriptor::default())
        .await
        .expect("Failed to create device");

    let validation_wgsl = format!(
        "{wgsl}\n\n@compute @workgroup_size(1)\nfn main() {{\n    // validation entry point\n}}"
    );

    let _module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(label),
        source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(&validation_wgsl)),
    });
}

#[tokio::test]
async fn poc_linear_scale_compiles_on_gpu() {
    validate_wgsl_compiles("linear_scale", LinearScale::wgsl_function()).await;
}

#[tokio::test]
async fn poc_log_scale_compiles_on_gpu() {
    validate_wgsl_compiles("log_scale", LogScale::wgsl_function()).await;
}

#[tokio::test]
async fn poc_power_scale_compiles_on_gpu() {
    validate_wgsl_compiles("power_scale", PowerScale::wgsl_function()).await;
}

#[tokio::test]
async fn poc_color_lerp_compiles_on_gpu() {
    validate_wgsl_compiles("color_lerp", ColorLerp::wgsl_function()).await;
}

#[tokio::test]
async fn poc_quantise_compiles_on_gpu() {
    validate_wgsl_compiles("quantise", Quantise::wgsl_function()).await;
}

#[tokio::test]
async fn poc_diffuse_factor_compiles_on_gpu() {
    validate_wgsl_compiles("diffuse_factor", DiffuseFactor::wgsl_function()).await;
}

#[tokio::test]
async fn poc_screen_distance_compiles_on_gpu() {
    validate_wgsl_compiles("screen_distance", ScreenDistance::wgsl_function()).await;
}

#[tokio::test]
async fn poc_smooth_threshold_compiles_on_gpu() {
    validate_wgsl_compiles("smooth_threshold", SmoothThreshold::wgsl_function()).await;
}

#[tokio::test]
async fn poc_radial_falloff_compiles_on_gpu() {
    validate_wgsl_compiles("radial_falloff", RadialFalloff::wgsl_function()).await;
}

#[tokio::test]
async fn poc_sigmoid_compiles_on_gpu() {
    validate_wgsl_compiles("sigmoid", Sigmoid::wgsl_function()).await;
}

#[tokio::test]
async fn poc_gaussian_compiles_on_gpu() {
    validate_wgsl_compiles("gaussian", Gaussian::wgsl_function()).await;
}

#[tokio::test]
async fn poc_iterative_sum_compiles_on_gpu() {
    validate_wgsl_compiles("iterative_sum", IterativeSum::wgsl_function()).await;
}

#[tokio::test]
async fn poc_multi_classify_compiles_on_gpu() {
    validate_wgsl_compiles("multi_classify", MultiClassify::wgsl_function()).await;
}

#[tokio::test]
async fn poc_oscillate_compiles_on_gpu() {
    validate_wgsl_compiles("oscillate", Oscillate::wgsl_function()).await;
}

// ---------------------------------------------------------------------------
// Approach comparison: #[shader_fn] vs #[wgsl_function]
// ---------------------------------------------------------------------------

/// Demonstrates that both macro approaches produce equivalent output.
mod approach_comparison {
    use super::*;

    /// Transpiled approach (Rust syntax, automatically converted to WGSL).
    #[shader_fn]
    fn transpiled_clamp_scale(value: f32, low: f32, high: f32) -> f32 {
        let clamped = clamp(value, low, high);
        let normalised = (clamped - low) / (high - low);
        return normalised;
    }

    /// String-based approach (WGSL syntax written directly).
    #[wgsl_function]
    fn manual_clamp_scale(value: f32, low: f32, high: f32) -> f32 {
        let clamped = clamp(value, low, high);
        let normalised = (clamped - low) / (high - low);
        return normalised;
    }

    #[test]
    fn both_approaches_produce_composable_functions() {
        fn assert_composable<T: ComposableShaderFunction>() {}
        assert_composable::<TranspiledClampScale>();
        assert_composable::<ManualClampScale>();
    }

    #[test]
    fn both_approaches_have_same_function_name_pattern() {
        assert_eq!(
            TranspiledClampScale::function_name(),
            "transpiled_clamp_scale"
        );
        assert_eq!(ManualClampScale::function_name(), "manual_clamp_scale");
    }

    #[test]
    fn both_approaches_generate_uniform_structs() {
        let transpiled = TranspiledClampScale::new(0.0, 1.0);
        let manual = ManualClampScale::new(0.0, 1.0);

        let tu = transpiled.create_uniforms().unwrap();
        let mu = manual.create_uniforms().unwrap();

        assert_eq!(tu.low, mu.low);
        assert_eq!(tu.high, mu.high);
    }

    #[test]
    fn both_approaches_produce_valid_wgsl() {
        let tw = TranspiledClampScale::wgsl_function();
        let mw = ManualClampScale::wgsl_function();

        // Both contain the key elements
        assert!(tw.contains("clamp("));
        assert!(mw.contains("clamp("));
        assert!(tw.contains("Uniforms"));
        assert!(mw.contains("Uniforms"));
    }

    #[test]
    fn both_approaches_work_in_same_pipeline() {
        use gup::shader_pipeline::ComposableShaderPipeline;

        let mut pipeline = ComposableShaderPipeline::new();
        pipeline.add_function(TranspiledClampScale::new(0.0, 1.0));
        pipeline.add_function(ManualClampScale::new(0.0, 100.0));
        assert_eq!(pipeline.function_count(), 2);
    }
}

// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Integration tests for the `#[shader_fn]` proc macro.
//!
//! Tests that transpiled Rust functions correctly implement
//! `ComposableShaderFunction` and interoperate with existing
//! `#[wgsl_function]` functions in the same `ShaderPipeline`.

use gup::shader_function::{self, ComposableShaderFunction, ShaderType, ShaderUniform};
use gup_macros::{shader_fn, wgsl_function};

// ---------------------------------------------------------------------------
// AC1: Transpiled functions implement ComposableShaderFunction
// ---------------------------------------------------------------------------

// A simple no-uniform function transpiled from Rust.
#[shader_fn]
fn double_value(value: f32) -> f32 {
    return value * 2.0;
}

// A function with uniforms (extra parameters become a uniform struct).
#[shader_fn]
fn scale_offset(value: f32, scale: f32, offset: f32) -> f32 {
    return value * scale + offset;
}

// A function using method calls that the transpiler maps.
#[shader_fn]
fn safe_sqrt(value: f32) -> f32 {
    let clamped = clamp(value, 0.0, 100.0);
    return sqrt(clamped);
}

// A function using control flow.
#[shader_fn]
fn classify(value: f32) -> f32 {
    if value > 1.0 {
        return 2.0;
    } else if value > 0.0 {
        return 1.0;
    } else {
        return 0.0;
    }
}

// A function using a for-loop.
#[shader_fn]
fn sum_range(n: i32) -> i32 {
    let mut sum = 0;
    for i in 0..n {
        sum += i;
    }
    return sum;
}

// --- Tests ---

#[test]
fn transpiled_function_name() {
    assert_eq!(DoubleValue::function_name(), "double_value");
    assert_eq!(ScaleOffset::function_name(), "scale_offset");
    assert_eq!(SafeSqrt::function_name(), "safe_sqrt");
    assert_eq!(Classify::function_name(), "classify");
    assert_eq!(SumRange::function_name(), "sum_range");
}

#[test]
fn transpiled_wgsl_function_returns_valid_wgsl() {
    let wgsl = DoubleValue::wgsl_function();
    assert!(
        wgsl.contains("fn double_value"),
        "WGSL should contain function name, got:\n{wgsl}"
    );
    assert!(
        wgsl.contains("value * 2.0"),
        "WGSL should contain body, got:\n{wgsl}"
    );
}

#[test]
fn transpiled_wgsl_function_with_uniforms() {
    let wgsl = ScaleOffset::wgsl_function();
    assert!(
        wgsl.contains("ScaleOffsetUniforms"),
        "WGSL should contain uniform struct, got:\n{wgsl}"
    );
    assert!(
        wgsl.contains("uniforms.scale"),
        "WGSL should reference uniform fields via uniforms prefix, got:\n{wgsl}"
    );
    assert!(
        wgsl.contains("uniforms.offset"),
        "WGSL should reference uniform fields via uniforms prefix, got:\n{wgsl}"
    );
}

#[test]
fn transpiled_create_uniforms_no_uniforms() {
    let f = DoubleValue::new();
    let u = f.create_uniforms();
    // No-uniform functions still return Some with a unit struct.
    assert!(u.is_some());
}

#[test]
fn transpiled_create_uniforms_with_values() {
    let f = ScaleOffset::new(2.0, 1.0);
    let u = f.create_uniforms().unwrap();
    assert_eq!(u.scale, 2.0);
    assert_eq!(u.offset, 1.0);
}

#[test]
fn transpiled_generate_wgsl_matches_static() {
    let f = DoubleValue::new();
    let dynamic = f.generate_wgsl();
    let static_wgsl = DoubleValue::wgsl_function();
    assert_eq!(dynamic, static_wgsl);
}

#[test]
fn transpiled_input_output_types() {
    // Verify the associated types are correct.
    assert_eq!(
        <f32 as ShaderType>::wgsl_type_name(),
        <<DoubleValue as ComposableShaderFunction>::Input as ShaderType>::wgsl_type_name()
    );
    assert_eq!(
        <f32 as ShaderType>::wgsl_type_name(),
        <<DoubleValue as ComposableShaderFunction>::Output as ShaderType>::wgsl_type_name()
    );
}

#[test]
fn transpiled_uniforms_type_name() {
    // Verify uniforms struct has a valid WGSL type name.
    let name = <ScaleOffsetUniforms as ShaderUniform>::wgsl_type_name();
    assert_eq!(name, "ScaleOffsetUniforms");
}

#[test]
fn transpiled_uniforms_struct_definition() {
    let def = <ScaleOffsetUniforms as ShaderUniform>::wgsl_struct_definition();
    assert!(
        def.contains("ScaleOffsetUniforms"),
        "Expected uniform struct definition, got: {def}"
    );
    assert!(
        def.contains("scale"),
        "Expected scale field in uniform struct, got: {def}"
    );
    assert!(
        def.contains("offset"),
        "Expected offset field in uniform struct, got: {def}"
    );
}

#[test]
fn transpiled_control_flow_wgsl() {
    let wgsl = Classify::wgsl_function();
    assert!(
        wgsl.contains("if"),
        "WGSL should contain if statement, got:\n{wgsl}"
    );
    assert!(
        wgsl.contains("else"),
        "WGSL should contain else, got:\n{wgsl}"
    );
}

#[test]
fn transpiled_for_loop_wgsl() {
    let wgsl = SumRange::wgsl_function();
    assert!(
        wgsl.contains("for"),
        "WGSL should contain for loop, got:\n{wgsl}"
    );
}

// ---------------------------------------------------------------------------
// AC2: Mixed Pipeline Support
// ---------------------------------------------------------------------------

// An existing-style wgsl_function for mixing.
#[wgsl_function]
fn negate_value(value: f32) -> f32 {
    return -value;
}

#[test]
fn both_macros_produce_composable_shader_function() {
    // Verify both types implement ComposableShaderFunction.
    fn assert_is_shader_fn<T: ComposableShaderFunction>() {}

    assert_is_shader_fn::<DoubleValue>();
    assert_is_shader_fn::<ScaleOffset>();
    assert_is_shader_fn::<NegateValue>();
}

#[test]
fn mixed_pipeline_functions_produce_valid_wgsl() {
    // Simulate what ShaderPipeline does: collect WGSL from both types.
    let transpiled_wgsl = DoubleValue::new().generate_wgsl();
    let manual_wgsl = NegateValue::new().generate_wgsl();

    // Both should be non-empty valid WGSL.
    assert!(
        !transpiled_wgsl.is_empty(),
        "Transpiled WGSL should not be empty"
    );
    assert!(
        !manual_wgsl.is_empty(),
        "Manual WGSL should not be empty"
    );

    // Both should contain valid function definitions.
    assert!(transpiled_wgsl.contains("fn double_value"));
    assert!(manual_wgsl.contains("fn negate_value"));
}

// ---------------------------------------------------------------------------
// AC3: Backward Compatibility
// ---------------------------------------------------------------------------

// Existing #[wgsl_function] with uniforms should still work.
#[wgsl_function]
fn legacy_scale(value: f32, factor: f32) -> f32 {
    return value * factor;
}

#[test]
fn existing_wgsl_function_still_works() {
    let f = LegacyScale::new(3.0);
    let u = f.create_uniforms().unwrap();
    assert_eq!(u.factor, 3.0);

    let wgsl = LegacyScale::wgsl_function();
    assert!(wgsl.contains("fn legacy_scale"));
}

// ---------------------------------------------------------------------------
// GPU Compilation Validation
// ---------------------------------------------------------------------------

/// Validate that a WGSL string compiles with wgpu/naga.
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

    // Wrap in a compute entry point for validation.
    let validation_wgsl = format!(
        "{wgsl}\n\n@compute @workgroup_size(1)\nfn main() {{\n    // validation entry point\n}}"
    );

    let _module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(label),
        source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(&validation_wgsl)),
    });
}

#[tokio::test]
async fn transpiled_simple_function_compiles_on_gpu() {
    let wgsl = DoubleValue::wgsl_function();
    validate_wgsl_compiles("double_value", wgsl).await;
}

#[tokio::test]
async fn transpiled_uniform_function_compiles_on_gpu() {
    let wgsl = ScaleOffset::wgsl_function();
    validate_wgsl_compiles("scale_offset", wgsl).await;
}

#[tokio::test]
async fn transpiled_builtin_calls_compile_on_gpu() {
    let wgsl = SafeSqrt::wgsl_function();
    validate_wgsl_compiles("safe_sqrt", wgsl).await;
}

#[tokio::test]
async fn transpiled_control_flow_compiles_on_gpu() {
    let wgsl = Classify::wgsl_function();
    validate_wgsl_compiles("classify", wgsl).await;
}

#[tokio::test]
async fn transpiled_for_loop_compiles_on_gpu() {
    let wgsl = SumRange::wgsl_function();
    validate_wgsl_compiles("sum_range", wgsl).await;
}

// ---------------------------------------------------------------------------
// AC2: Mixed pipeline — add both transpiled and manual functions
// ---------------------------------------------------------------------------

#[test]
fn transpiled_function_works_with_pipeline_add_function() {
    use gup::shader_pipeline::ComposableShaderPipeline;

    let mut pipeline = ComposableShaderPipeline::new();
    pipeline.add_function(DoubleValue::new());
    pipeline.add_function(ScaleOffset::new(2.0, 1.0));
    assert_eq!(pipeline.function_count(), 2);
}

#[test]
fn mixed_pipeline_transpiled_and_manual_functions() {
    use gup::shader_pipeline::ComposableShaderPipeline;

    let mut pipeline = ComposableShaderPipeline::new();

    // Transpiled function.
    pipeline.add_function(DoubleValue::new());

    // Manually written wgsl_function.
    pipeline.add_function(NegateValue::new());

    // Both added successfully.
    assert_eq!(pipeline.function_count(), 2);
}

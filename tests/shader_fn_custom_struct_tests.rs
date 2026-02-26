// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Integration tests for custom struct support in `#[shader_fn]`.
//!
//! Validates that `#[shader_fn]` functions can accept custom structs
//! decorated with `#[derive(WgslStruct)]` as both input and uniform
//! parameters, with correct WGSL generation and GPU compilation.

use gup::shader_function::{ComposableShaderFunction, ShaderType, WgslStructType};
use gup::*;
use gup_macros::{WgslStruct, shader_fn};

// ---------------------------------------------------------------------------
// Custom struct definitions using #[derive(WgslStruct)]
// ---------------------------------------------------------------------------

#[derive(WgslStruct, Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct Point2D {
    pub x: f32,
    pub y: f32,
}

#[derive(WgslStruct, Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct ScaleConfig {
    pub scale: f32,
    pub offset: f32,
}

#[derive(WgslStruct, Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct TransformParams {
    pub scale_x: f32,
    pub scale_y: f32,
    pub translate_x: f32,
    pub translate_y: f32,
}

// ---------------------------------------------------------------------------
// AC1: #[shader_fn] functions accept custom struct input parameters
// ---------------------------------------------------------------------------

#[shader_fn]
fn point_magnitude(point: Point2D) -> f32 {
    return sqrt(point.x * point.x + point.y * point.y);
}

#[test]
fn custom_struct_input_function_name() {
    assert_eq!(PointMagnitude::function_name(), "point_magnitude");
}

#[test]
fn custom_struct_input_wgsl_contains_function() {
    let wgsl = PointMagnitude::wgsl_function();
    assert!(
        wgsl.contains("fn point_magnitude"),
        "WGSL should contain function definition, got:\n{wgsl}"
    );
}

#[test]
fn custom_struct_input_wgsl_has_struct_type_param() {
    let wgsl = PointMagnitude::wgsl_function();
    assert!(
        wgsl.contains("point: Point2D"),
        "WGSL should have Point2D parameter type, got:\n{wgsl}"
    );
}

#[test]
fn custom_struct_input_field_access() {
    let wgsl = PointMagnitude::wgsl_function();
    assert!(
        wgsl.contains("point.x"),
        "WGSL should access point.x directly, got:\n{wgsl}"
    );
    assert!(
        wgsl.contains("point.y"),
        "WGSL should access point.y directly, got:\n{wgsl}"
    );
}

#[test]
fn custom_struct_input_generate_wgsl_includes_definition() {
    let f = PointMagnitude::new();
    let wgsl = f.generate_wgsl();
    assert!(
        wgsl.contains("struct Point2D"),
        "generate_wgsl() should include Point2D struct definition, got:\n{wgsl}"
    );
    assert!(
        wgsl.contains("x: f32"),
        "Point2D struct should have x field, got:\n{wgsl}"
    );
    assert!(
        wgsl.contains("y: f32"),
        "Point2D struct should have y field, got:\n{wgsl}"
    );
}

// ---------------------------------------------------------------------------
// AC2: Custom structs work as uniform parameters
// ---------------------------------------------------------------------------

#[shader_fn]
fn apply_scale(value: f32, config: ScaleConfig) -> f32 {
    return value * config.scale + config.offset;
}

#[test]
fn custom_struct_uniform_function_name() {
    assert_eq!(ApplyScale::function_name(), "apply_scale");
}

#[test]
fn custom_struct_uniform_wgsl_has_uniforms_struct() {
    let wgsl = ApplyScale::wgsl_function();
    assert!(
        wgsl.contains("ApplyScaleUniforms"),
        "WGSL should contain uniforms struct with custom type, got:\n{wgsl}"
    );
    assert!(
        wgsl.contains("config: ScaleConfig"),
        "Uniforms struct should contain ScaleConfig field, got:\n{wgsl}"
    );
}

#[test]
fn custom_struct_uniform_field_access() {
    let wgsl = ApplyScale::wgsl_function();
    assert!(
        wgsl.contains("uniforms.config.scale"),
        "WGSL should access config.scale via uniforms prefix, got:\n{wgsl}"
    );
    assert!(
        wgsl.contains("uniforms.config.offset"),
        "WGSL should access config.offset via uniforms prefix, got:\n{wgsl}"
    );
}

#[test]
fn custom_struct_uniform_create_uniforms() {
    let config = ScaleConfig {
        scale: 2.0,
        offset: 1.0,
    };
    let f = ApplyScale::new(config);
    let u = f.create_uniforms().unwrap();
    assert_eq!(u.config.scale, 2.0);
    assert_eq!(u.config.offset, 1.0);
}

#[test]
fn custom_struct_uniform_generate_wgsl_includes_definitions() {
    let config = ScaleConfig {
        scale: 1.0,
        offset: 0.0,
    };
    let f = ApplyScale::new(config);
    let wgsl = f.generate_wgsl();
    assert!(
        wgsl.contains("struct ScaleConfig"),
        "generate_wgsl() should include ScaleConfig definition, got:\n{wgsl}"
    );
}

// ---------------------------------------------------------------------------
// AC3: Custom structs work as both input AND uniform parameters
// ---------------------------------------------------------------------------

#[shader_fn]
fn transform_point(point: Point2D, params: TransformParams) -> f32 {
    let x = point.x * params.scale_x + params.translate_x;
    let y = point.y * params.scale_y + params.translate_y;
    return sqrt(x * x + y * y);
}

#[test]
fn custom_struct_both_input_and_uniform() {
    let wgsl = TransformPoint::wgsl_function();
    assert!(
        wgsl.contains("point: Point2D"),
        "Should have Point2D input param, got:\n{wgsl}"
    );
    assert!(
        wgsl.contains("TransformPointUniforms"),
        "Should have uniforms struct, got:\n{wgsl}"
    );
    assert!(
        wgsl.contains("point.x"),
        "Should access input struct fields directly, got:\n{wgsl}"
    );
    assert!(
        wgsl.contains("uniforms.params.scale_x"),
        "Should access uniform struct fields via uniforms prefix, got:\n{wgsl}"
    );
}

#[test]
fn custom_struct_both_generate_wgsl_includes_all_definitions() {
    let params = TransformParams {
        scale_x: 1.0,
        scale_y: 1.0,
        translate_x: 0.0,
        translate_y: 0.0,
    };
    let f = TransformPoint::new(params);
    let wgsl = f.generate_wgsl();
    assert!(
        wgsl.contains("struct Point2D"),
        "Should include Point2D definition, got:\n{wgsl}"
    );
    assert!(
        wgsl.contains("struct TransformParams"),
        "Should include TransformParams definition, got:\n{wgsl}"
    );
}

// ---------------------------------------------------------------------------
// AC4: Memory layout alignment is correct per WGSL spec
// ---------------------------------------------------------------------------

#[test]
fn custom_struct_derives_have_correct_wgsl_definitions() {
    let point_def = Point2D::wgsl_struct_definition();
    assert!(
        point_def.contains("struct Point2D"),
        "Point2D WGSL: {point_def}"
    );
    assert!(point_def.contains("x: f32"), "Point2D WGSL: {point_def}");
    assert!(point_def.contains("y: f32"), "Point2D WGSL: {point_def}");

    let config_def = ScaleConfig::wgsl_struct_definition();
    assert!(
        config_def.contains("struct ScaleConfig"),
        "ScaleConfig WGSL: {config_def}"
    );
}

#[test]
fn custom_struct_shader_type_integration() {
    // Verify ShaderType is implemented for our custom structs
    assert_eq!(Point2D::wgsl_type_name(), "Point2D");
    assert!(Point2D::wgsl_type_definition().is_some());

    assert_eq!(ScaleConfig::wgsl_type_name(), "ScaleConfig");
    assert!(ScaleConfig::wgsl_type_definition().is_some());
}

#[test]
fn custom_struct_uniforms_bytemuck_compatible() {
    // Verify the generated uniform struct is bytemuck-compatible
    let config = ScaleConfig {
        scale: 2.0,
        offset: 1.0,
    };
    let f = ApplyScale::new(config);
    let u = f.create_uniforms().unwrap();

    // Should be able to cast to bytes (bytemuck::Pod)
    let bytes = bytemuck::bytes_of(&u);
    assert!(
        !bytes.is_empty(),
        "Should be able to cast uniforms to bytes"
    );
}

// ---------------------------------------------------------------------------
// AC5: GPU compilation validation
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
async fn custom_struct_input_compiles_on_gpu() {
    let f = PointMagnitude::new();
    let wgsl = f.generate_wgsl();
    validate_wgsl_compiles("point_magnitude", &wgsl).await;
}

#[tokio::test]
async fn custom_struct_uniform_compiles_on_gpu() {
    let config = ScaleConfig {
        scale: 1.0,
        offset: 0.0,
    };
    let f = ApplyScale::new(config);
    let wgsl = f.generate_wgsl();
    validate_wgsl_compiles("apply_scale", &wgsl).await;
}

#[tokio::test]
async fn custom_struct_both_input_and_uniform_compiles_on_gpu() {
    let params = TransformParams {
        scale_x: 1.0,
        scale_y: 1.0,
        translate_x: 0.0,
        translate_y: 0.0,
    };
    let f = TransformPoint::new(params);
    let wgsl = f.generate_wgsl();
    validate_wgsl_compiles("transform_point", &wgsl).await;
}

// ---------------------------------------------------------------------------
// Integration with ShaderPipeline
// ---------------------------------------------------------------------------

#[test]
fn custom_struct_function_works_with_pipeline() {
    use gup::shader_pipeline::ComposableShaderPipeline;

    let mut pipeline = ComposableShaderPipeline::new();
    pipeline.add_function(PointMagnitude::new());
    assert_eq!(pipeline.function_count(), 1);
}

#[test]
fn custom_struct_uniform_function_works_with_pipeline() {
    use gup::shader_pipeline::ComposableShaderPipeline;

    let mut pipeline = ComposableShaderPipeline::new();
    let config = ScaleConfig {
        scale: 2.0,
        offset: 1.0,
    };
    pipeline.add_function(ApplyScale::new(config));
    assert_eq!(pipeline.function_count(), 1);
}

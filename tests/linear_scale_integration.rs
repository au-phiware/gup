// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Integration tests for LinearScale shader function (GUP-252).
//!
//! Validates:
//! - LinearScale → LinearScaleInvert round-trip composition via ShaderPipeline
//! - WGSL compilation of the generated linear_scale / linear_scale_invert code

use gup::error::GupResult;
use gup::shader_function::{
    ComposableFunction, ComposableShaderFunction, LinearScale, ShaderUniform,
};

// ---------------------------------------------------------------------------
// Round-trip composition: LinearScale → LinearScaleInvert
// ---------------------------------------------------------------------------

#[test]
fn linear_scale_invert_round_trip_wgsl() {
    let scale = LinearScale::new(0.0, 100.0, 0.0, 1.0);
    let inv = scale.invert();

    let composed = scale.compose(inv);
    let wgsl = composed.generate_wgsl();

    // Both forward and inverse functions must be present.
    assert!(
        wgsl.contains("fn linear_scale("),
        "Composed WGSL missing linear_scale: {wgsl}"
    );
    assert!(
        wgsl.contains("fn linear_scale_invert("),
        "Composed WGSL missing linear_scale_invert: {wgsl}"
    );
    // The composed chain wrapper must reference them.
    assert!(
        wgsl.contains("composed_chain"),
        "Composed WGSL missing composed_chain: {wgsl}"
    );
}

#[test]
fn linear_scale_compose_type_checks() {
    // LinearScale: f32 → f32, LinearScaleInvert: f32 → f32
    // Composition should type-check and produce uniforms.
    let scale = LinearScale::with_clamp(10.0, 200.0, -1.0, 1.0);
    let inv = scale.invert();
    let composed = scale.compose(inv);
    let uniforms = composed.create_uniforms();
    assert!(uniforms.is_some(), "Composed uniforms should not be None");
}

// ---------------------------------------------------------------------------
// WGSL compilation validation
// ---------------------------------------------------------------------------

#[tokio::test]
async fn linear_scale_wgsl_compiles_on_gpu() -> GupResult<()> {
    let context = gup::context::GupContext::headless().await?;
    let device = &context.device;

    let wgsl_code = LinearScale::wgsl_function();
    let struct_def =
        <gup::shader_function::LinearScaleUniforms as ShaderUniform>::wgsl_struct_definition();

    // Build a complete shader module that exercises both functions.
    let complete_shader = format!(
        r#"
{struct_def}

{wgsl_code}

@vertex
fn vs_main(@builtin(vertex_index) idx: u32) -> @builtin(position) vec4<f32> {{
    return vec4<f32>(0.0, 0.0, 0.0, 1.0);
}}

@fragment
fn fs_main() -> @location(0) vec4<f32> {{
    let u = LinearScaleUniforms(0.0, 100.0, 0.0, 1.0, 0u, 0u, 0u, 0u);
    let forward = linear_scale(50.0, u);
    let inverse = linear_scale_invert(0.5, u);
    return vec4<f32>(forward, inverse, 0.0, 1.0);
}}
"#
    );

    // This should compile without errors.
    let _shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("LinearScale WGSL Validation"),
        source: wgpu::ShaderSource::Wgsl(complete_shader.into()),
    });

    Ok(())
}

#[tokio::test]
async fn linear_scale_clamped_wgsl_compiles_on_gpu() -> GupResult<()> {
    let context = gup::context::GupContext::headless().await?;
    let device = &context.device;

    let wgsl_code = LinearScale::wgsl_function();
    let struct_def =
        <gup::shader_function::LinearScaleUniforms as ShaderUniform>::wgsl_struct_definition();

    // Clamped variant (clamp_flag = 1u).
    let complete_shader = format!(
        r#"
{struct_def}

{wgsl_code}

@vertex
fn vs_main(@builtin(vertex_index) idx: u32) -> @builtin(position) vec4<f32> {{
    return vec4<f32>(0.0, 0.0, 0.0, 1.0);
}}

@fragment
fn fs_main() -> @location(0) vec4<f32> {{
    let u = LinearScaleUniforms(0.0, 100.0, 0.0, 1.0, 1u, 0u, 0u, 0u);
    // Value outside domain should be clamped.
    let forward = linear_scale(150.0, u);
    let inverse = linear_scale_invert(1.5, u);
    return vec4<f32>(forward, inverse, 0.0, 1.0);
}}
"#
    );

    let _shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("LinearScale Clamped WGSL Validation"),
        source: wgpu::ShaderSource::Wgsl(complete_shader.into()),
    });

    Ok(())
}

// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Integration tests for LogScale shader function (GUP-253).
//!
//! Validates:
//! - LogScale → ColorMap composition via ShaderPipeline
//! - ScatterPlotBuilder with explicit LogScale for axis tick generation
//! - WGSL compilation of the generated log_scale code on the GPU

use gup::RenderContext;
use gup::chart_builder::ChartBuilder;
use gup::chart_builder::accessor::AccessorValue;
use gup::chart_builder::builders::{AccessorFunction, ScatterPlotBuilder};
use gup::error::GupResult;
use gup::shader_function::{
    ColorMap, ComposableFunction, ComposableShaderFunction, LogScale, ShaderUniform, Vec4,
};
use gup::vec4;

// ---------------------------------------------------------------------------
// AC7: LogScale → ColorMap composition
// ---------------------------------------------------------------------------

#[test]
fn log_scale_compose_with_color_map() {
    // LogScale (f32 → f32) composed with ColorMap (f32 → vec4<f32>).
    let log_scale = LogScale::new(10.0).domain(1.0, 1000.0).range(0.0, 1.0);
    let color_map = ColorMap::new(vec4![0.0, 0.0, 0.0, 1.0], vec4![1.0, 1.0, 1.0, 1.0]);

    let composed = log_scale.compose(color_map);
    let wgsl = composed.generate_wgsl();

    // Both functions must be present in the generated WGSL.
    assert!(
        wgsl.contains("fn log_scale("),
        "Composed WGSL missing log_scale: {wgsl}"
    );
    assert!(
        wgsl.contains("fn color_map("),
        "Composed WGSL missing color_map: {wgsl}"
    );
    // The chain wrapper must reference them.
    assert!(
        wgsl.contains("composed_chain"),
        "Composed WGSL missing composed_chain: {wgsl}"
    );

    // Uniforms should be populated.
    let uniforms = composed.create_uniforms();
    assert!(uniforms.is_some(), "Composed uniforms should not be None");
}

#[test]
fn log_scale_symmetric_compose_with_color_map() {
    // Symmetric LogScale composed with ColorMap.
    let log_scale = LogScale::new(10.0)
        .domain(-1000.0, 1000.0)
        .range(0.0, 1.0)
        .symmetric(true);
    let color_map = ColorMap::new(vec4![0.0, 0.0, 1.0, 1.0], vec4![1.0, 0.0, 0.0, 1.0]);

    let composed = log_scale.compose(color_map);
    let uniforms = composed.create_uniforms();
    assert!(uniforms.is_some());

    // Verify the LogScale uniforms contain symmetric flag.
    let chain = uniforms.unwrap();
    assert_eq!(chain.first.symmetric, 1);
}

// ---------------------------------------------------------------------------
// AC6: ScatterPlotBuilder with LogScale Y-axis
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct DataPoint {
    x: f32,
    y: f32,
}

#[tokio::test]
async fn scatter_builder_with_log_y_scale_generates_ticks() -> GupResult<()> {
    let render_context = std::sync::Arc::new(RenderContext::new().await?);

    let data = vec![
        DataPoint { x: 1.0, y: 1.0 },
        DataPoint { x: 2.0, y: 10.0 },
        DataPoint { x: 3.0, y: 100.0 },
        DataPoint { x: 4.0, y: 1000.0 },
    ];

    let chart = ScatterPlotBuilder::<DataPoint>::new()
        .x(AccessorFunction::new(|d: &DataPoint| {
            AccessorValue::Float(d.x)
        }))
        .y(AccessorFunction::new(|d: &DataPoint| {
            AccessorValue::Float(d.y)
        }))
        .y_scale(LogScale::new(10.0).domain(1.0, 1000.0).range(0.0, 1.0))
        .build_with_data(data, render_context)?;

    // The chart should have been built with axes.
    assert!(chart.bottom_axis.is_some(), "Should have a bottom axis");
    assert!(chart.left_axis.is_some(), "Should have a left axis");

    // Verify that axis geometry generation works with the log scale.
    let geom = chart.generate_axis_geometry_instanced();
    assert!(
        !geom.tick_instances.is_empty(),
        "Log-scale axis should produce tick instances"
    );

    // Verify the y_scale is stored as AxisScale::Log.
    assert!(chart.config.y_scale.is_some(), "y_scale should be stored");

    Ok(())
}

// ---------------------------------------------------------------------------
// GPU WGSL compilation
// ---------------------------------------------------------------------------

#[tokio::test]
async fn log_scale_wgsl_compiles_on_gpu() -> GupResult<()> {
    let context = gup::context::GupContext::headless().await?;
    let device = &context.device;

    let wgsl_code = LogScale::wgsl_function();
    let struct_def =
        <gup::shader_function::LogScaleUniforms as ShaderUniform>::wgsl_struct_definition();

    // Build a complete shader module that exercises the log_scale function.
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
    // Standard log scale: domain [1, 1000], range [0, 1], base 10, non-symmetric.
    let u = LogScaleUniforms(1.0, 1000.0, 0.0, 1.0, 10.0, 0u, 0u, 0u);
    let result = log_scale(100.0, u);
    return vec4<f32>(result, 0.0, 0.0, 1.0);
}}
"#
    );

    // This should compile without errors.
    let _shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("LogScale WGSL Validation"),
        source: wgpu::ShaderSource::Wgsl(complete_shader.into()),
    });

    Ok(())
}

#[tokio::test]
async fn log_scale_symmetric_wgsl_compiles_on_gpu() -> GupResult<()> {
    let context = gup::context::GupContext::headless().await?;
    let device = &context.device;

    let wgsl_code = LogScale::wgsl_function();
    let struct_def =
        <gup::shader_function::LogScaleUniforms as ShaderUniform>::wgsl_struct_definition();

    // Symmetric mode: domain [-1000, 1000], symmetric = 1.
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
    let u = LogScaleUniforms(-1000.0, 1000.0, -1.0, 1.0, 10.0, 1u, 0u, 0u);
    let pos = log_scale(100.0, u);
    let neg = log_scale(-100.0, u);
    let zero = log_scale(0.0, u);
    return vec4<f32>(pos, neg, zero, 1.0);
}}
"#
    );

    let _shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("LogScale Symmetric WGSL Validation"),
        source: wgpu::ShaderSource::Wgsl(complete_shader.into()),
    });

    Ok(())
}

// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Integration tests for ColorScale GPU shader function (GUP-255).
//!
//! Validates:
//! - WGSL compilation (via wgpu/naga) for all three scale kinds
//! - LinearScale → ColorScale composition WGSL validation
//! - ChartBuilder integration stores color_scale correctly
//! - Buffer data lengths for each palette preset

use gup::chart_builder::ChartConfig;
use gup::shader_function::{
    ColorScale, ColorScaleKind, ComposableFunction, ComposableShaderFunction, LinearScale,
    ShaderUniform,
};

// ---------------------------------------------------------------------------
// Helper: validate WGSL compiles on GPU
// ---------------------------------------------------------------------------

/// Wraps the given function WGSL in a minimal shader module with the required
/// storage-buffer bindings and struct definitions, then validates it via wgpu.
async fn validate_color_scale_wgsl(label: &str, wgsl: &str) {
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

    // Build a self-contained WGSL module.
    let full_wgsl = build_validation_module(wgsl);

    let _module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(label),
        source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(&full_wgsl)),
    });
}

/// Wraps function WGSL with struct definitions, storage bindings, and an
/// entry point so that naga can validate it.
fn build_validation_module(function_wgsl: &str) -> String {
    // Include struct definitions that the WGSL function body references.
    let mut module = String::new();

    // ColorScaleUniforms
    module.push_str(
        "struct ColorScaleUniforms {\n    domain_min: f32,\n    domain_max: f32,\n    midpoint: f32,\n    scale_kind: u32,\n    n_bins: u32,\n    stop_count: u32,\n    _pad0: u32,\n    _pad1: u32,\n}\n\n",
    );

    // LinearScaleUniforms (needed for composed chains)
    module.push_str(
        "struct LinearScaleUniforms {\n    domain_min: f32,\n    domain_max: f32,\n    range_min: f32,\n    range_max: f32,\n    clamp_flag: u32,\n    _pad0: u32,\n    _pad1: u32,\n    _pad2: u32,\n}\n\n",
    );

    // ChainUniforms (for composed chains)
    module.push_str(
        "struct ChainUniforms {\n    first: LinearScaleUniforms,\n    second: ColorScaleUniforms,\n}\n\n",
    );

    // Storage buffers matching the GUP-134 pattern.
    module
        .push_str("@group(0) @binding(0) var<storage, read> gradient_colors: array<vec4<f32>>;\n");
    module.push_str("@group(0) @binding(1) var<storage, read> gradient_stops: array<f32>;\n\n");

    // The function(s) under test.
    module.push_str(function_wgsl);

    // Add a compute entry point so naga has something to validate.
    module.push_str(
        "\n\n@compute @workgroup_size(1)\nfn main() {\n    // validation entry point\n}\n",
    );

    module
}

// ---------------------------------------------------------------------------
// WGSL validation tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn color_scale_continuous_wgsl_compiles() {
    let wgsl = ColorScale::wgsl_function();
    validate_color_scale_wgsl("color_scale_continuous", wgsl).await;
}

#[tokio::test]
async fn color_scale_diverging_wgsl_compiles() {
    // The diverging variant uses the same WGSL (branched on scale_kind).
    let cs = ColorScale::diverging(ColorScale::rd_bu_gradient(), -5.0, 0.0, 10.0);
    let wgsl = cs.generate_wgsl();
    validate_color_scale_wgsl("color_scale_diverging", &wgsl).await;
}

#[tokio::test]
async fn color_scale_quantize_wgsl_compiles() {
    let cs = ColorScale::quantize(ColorScale::viridis_gradient(), (0.0, 100.0), 5);
    let wgsl = cs.generate_wgsl();
    validate_color_scale_wgsl("color_scale_quantize", &wgsl).await;
}

#[tokio::test]
async fn linear_scale_compose_color_scale_wgsl_compiles() {
    let scale = LinearScale::new(0.0, 100.0, 0.0, 1.0);
    let cs = ColorScale::viridis(0.0, 1.0);
    let composed = scale.compose(cs);
    let wgsl = composed.generate_wgsl();
    validate_color_scale_wgsl("linear_scale_compose_color_scale", &wgsl).await;
}

// ---------------------------------------------------------------------------
// Composition type-check tests
// ---------------------------------------------------------------------------

#[test]
fn compose_produces_function_chain() {
    let scale = LinearScale::new(0.0, 100.0, 0.0, 1.0);
    let cs = ColorScale::viridis(0.0, 1.0);
    let composed = scale.compose(cs);

    // Verify uniforms are present.
    let uniforms = composed.create_uniforms();
    assert!(uniforms.is_some());

    // Verify the generated WGSL mentions all three functions.
    let wgsl = composed.generate_wgsl();
    assert!(wgsl.contains("fn linear_scale("));
    assert!(wgsl.contains("fn color_scale("));
    assert!(wgsl.contains("fn composed_chain("));
}

#[test]
fn compose_uniforms_struct_definition() {
    // Verify the ChainUniforms WGSL struct contains both component types.
    use gup::shader_function::ChainUniforms;
    type CU = ChainUniforms<
        gup::shader_function::LinearScaleUniforms,
        gup::shader_function::ColorScaleUniforms,
    >;
    let def = CU::wgsl_struct_definition();
    assert!(
        def.contains("first: LinearScaleUniforms"),
        "Missing first: {def}"
    );
    assert!(
        def.contains("second: ColorScaleUniforms"),
        "Missing second: {def}"
    );
}

// ---------------------------------------------------------------------------
// Buffer data length tests
// ---------------------------------------------------------------------------

#[test]
fn preset_buffer_data_lengths() {
    let presets: Vec<(&str, ColorScale)> = vec![
        ("viridis", ColorScale::viridis(0.0, 1.0)),
        ("plasma", ColorScale::plasma(0.0, 1.0)),
        ("inferno", ColorScale::inferno(0.0, 1.0)),
        ("magma", ColorScale::magma(0.0, 1.0)),
        ("rd_bu", ColorScale::rd_bu(0.0, 1.0)),
    ];
    for (name, cs) in &presets {
        let expected_stops = cs.gradient.count() as usize;
        let colors_bytes = cs.create_colors_buffer_data().len();
        let stops_bytes = cs.create_stops_buffer_data().len();
        assert_eq!(
            colors_bytes,
            expected_stops * 16,
            "{name}: colors buffer size mismatch"
        );
        assert_eq!(
            stops_bytes,
            expected_stops * 4,
            "{name}: stops buffer size mismatch"
        );
    }
}

// ---------------------------------------------------------------------------
// ChartBuilder integration
// ---------------------------------------------------------------------------

#[test]
fn chart_config_stores_color_scale() {
    let config = ChartConfig::default().with_color_scale(ColorScale::viridis(0.0, 100.0));
    assert!(config.color_scale.is_some());
    let cs = config.color_scale.unwrap();
    assert_eq!(cs.kind, ColorScaleKind::Continuous);
    assert_eq!(cs.domain_min, 0.0);
    assert_eq!(cs.domain_max, 100.0);
}

#[test]
fn chart_config_color_scale_diverging() {
    let config = ChartConfig::default().with_color_scale(ColorScale::diverging(
        ColorScale::rd_bu_gradient(),
        -1.0,
        0.0,
        1.0,
    ));
    assert!(config.color_scale.is_some());
    if let Some(cs) = config.color_scale {
        assert_eq!(cs.kind, ColorScaleKind::Diverging { midpoint: 0.0 });
    }
}

#[test]
fn scatter_builder_color_scale() {
    use gup::chart_builder::builders::scatter;

    let builder = scatter::<()>().color_scale(ColorScale::plasma(0.0, 50.0));
    // ScatterPlotBuilder stores it in its internal config.
    // (config is pub(crate), so we just verify the builder compiled.)
    let _ = builder;
}

#[test]
fn heatmap_builder_color_scale() {
    use gup::chart_builder::builders::heatmap;

    let builder = heatmap::<()>().color_scale(ColorScale::magma(0.0, 255.0));
    let _ = builder;
}

#[test]
fn line_builder_color_scale() {
    use gup::chart_builder::builders::line;

    let builder = line::<()>().color_scale(ColorScale::inferno(-10.0, 40.0));
    let _ = builder;
}

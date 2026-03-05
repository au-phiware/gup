// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Integration tests for the `CompositeChartBuilder` (GUP-251 / GUP-304).
//!
//! Verifies that multi-layer composite charts can be built against a
//! real GPU context without validation errors, including composites
//! with heterogeneous per-layer data types (GUP-304).

use gup::RenderContext;
use gup::chart_builder::ChartBuilder;
use gup::chart_builder::accessor::AccessorValue;
use gup::chart_builder::builders::composite::{composite, union_domain};
use gup::chart_builder::builders::{
    AccessorFunction, BarChartBuilder, ConfigurableBuilder, GridCapableBuilder, LineChartBuilder,
    ScatterPlotBuilder,
};
use std::sync::Arc;

// ── Shared data type ────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct Pt {
    x: f32,
    y: f32,
}

fn sample_data() -> Vec<Pt> {
    vec![
        Pt { x: 1.0, y: 2.0 },
        Pt { x: 2.0, y: 4.0 },
        Pt { x: 3.0, y: 3.0 },
        Pt { x: 4.0, y: 7.0 },
        Pt { x: 5.0, y: 5.0 },
    ]
}

fn x_accessor() -> AccessorFunction<Pt> {
    AccessorFunction::new(|d: &Pt| AccessorValue::Float(d.x))
}

fn y_accessor() -> AccessorFunction<Pt> {
    AccessorFunction::new(|d: &Pt| AccessorValue::Float(d.y))
}

// ── GPU integration tests ───────────────────────────────────────────────

#[tokio::test]
async fn composite_scatter_line_builds_ok() {
    let ctx = Arc::new(RenderContext::new().await.expect("GPU context"));

    let chart = composite::<Pt>()
        .layer(ScatterPlotBuilder::new().x(x_accessor()).y(y_accessor()))
        .layer(LineChartBuilder::new().x(x_accessor()).y(y_accessor()))
        .title("Integration: scatter + line")
        .build_with_data(sample_data(), ctx);

    assert!(chart.is_ok(), "Build failed: {:?}", chart.err());
    let chart = chart.unwrap();
    assert_eq!(chart.additional_layer_count(), 2);
    assert!(!chart.has_secondary_y_axis());
}

#[tokio::test]
async fn composite_bar_line_dual_axis_builds_ok() {
    let ctx = Arc::new(RenderContext::new().await.expect("GPU context"));

    let chart = composite::<Pt>()
        .layer(BarChartBuilder::new().x(x_accessor()).y(y_accessor()))
        .layer_with_y2(LineChartBuilder::new().x(x_accessor()).y(y_accessor()))
        .build_with_data(sample_data(), ctx);

    assert!(chart.is_ok(), "Build failed: {:?}", chart.err());
    let chart = chart.unwrap();
    assert!(chart.has_secondary_y_axis());
}

#[tokio::test]
async fn composite_no_layers_returns_error() {
    let ctx = Arc::new(RenderContext::new().await.expect("GPU context"));

    let result = composite::<Pt>().build_with_data(sample_data(), ctx);
    assert!(result.is_err());

    let err_msg = format!("{}", result.unwrap_err());
    assert!(
        err_msg.contains("no layers"),
        "Expected 'no layers' error, got: {err_msg}"
    );
}

#[tokio::test]
async fn composite_empty_data_returns_error() {
    let ctx = Arc::new(RenderContext::new().await.expect("GPU context"));

    let result = composite::<Pt>()
        .layer(ScatterPlotBuilder::new().x(x_accessor()).y(y_accessor()))
        .build_with_data(vec![], ctx);

    assert!(result.is_err());
}

#[tokio::test]
async fn composite_with_grid_builds_ok() {
    let ctx = Arc::new(RenderContext::new().await.expect("GPU context"));

    let chart = composite::<Pt>()
        .layer(ScatterPlotBuilder::new().x(x_accessor()).y(y_accessor()))
        .layer(LineChartBuilder::new().x(x_accessor()).y(y_accessor()))
        .light_grid()
        .build_with_data(sample_data(), ctx);

    assert!(chart.is_ok());
}

// ── Pure domain-unification tests (no GPU) ──────────────────────────────

#[test]
fn domain_union_five_cases() {
    // 1. Overlapping
    assert_eq!(
        union_domain(Some((0.0, 10.0)), Some((5.0, 15.0))),
        Some((0.0, 15.0))
    );

    // 2. Non-overlapping
    assert_eq!(
        union_domain(Some((0.0, 2.0)), Some((8.0, 10.0))),
        Some((0.0, 10.0))
    );

    // 3. Contained
    assert_eq!(
        union_domain(Some((0.0, 100.0)), Some((20.0, 50.0))),
        Some((0.0, 100.0))
    );

    // 4. Single-point
    assert_eq!(
        union_domain(Some((5.0, 5.0)), Some((10.0, 10.0))),
        Some((5.0, 10.0))
    );

    // 5. Negative range
    assert_eq!(
        union_domain(Some((-20.0, -5.0)), Some((-10.0, 10.0))),
        Some((-20.0, 10.0))
    );
}

// ── GPU render pipeline tests (GUP-303) ─────────────────────────────────

#[tokio::test]
async fn composite_prepare_render_succeeds() {
    let ctx = Arc::new(RenderContext::new().await.expect("GPU context"));

    let mut chart = composite::<Pt>()
        .layer(ScatterPlotBuilder::new().x(x_accessor()).y(y_accessor()))
        .layer(LineChartBuilder::new().x(x_accessor()).y(y_accessor()))
        .build_with_data(sample_data(), ctx.clone())
        .expect("build");

    let result = chart.prepare_render(
        ctx.device(),
        ctx.queue(),
        wgpu::TextureFormat::Bgra8UnormSrgb,
    );
    assert!(result.is_ok(), "prepare_render failed: {:?}", result.err());
}

#[tokio::test]
async fn composite_all_layers_render_ready_after_prepare() {
    let ctx = Arc::new(RenderContext::new().await.expect("GPU context"));

    let mut chart = composite::<Pt>()
        .layer(ScatterPlotBuilder::new().x(x_accessor()).y(y_accessor()))
        .layer(LineChartBuilder::new().x(x_accessor()).y(y_accessor()))
        .build_with_data(sample_data(), ctx.clone())
        .expect("build");

    // Some layers may already be render-ready from their inner build
    // (e.g. scatter layers that call prepare_render_bound during
    // build_with_data). Line layers need coordinate overrides applied
    // during prepare_render, so they start not-ready.
    let before = chart.layer_draw_count();
    assert!(
        before < chart.additional_layer_count(),
        "Expected at least one layer to need prepare_render"
    );

    chart
        .prepare_render(
            ctx.device(),
            ctx.queue(),
            wgpu::TextureFormat::Bgra8UnormSrgb,
        )
        .expect("prepare");

    // After prepare, all layers should be ready.
    assert_eq!(
        chart.layer_draw_count(),
        chart.additional_layer_count(),
        "Not all layers are render-ready"
    );
}

#[tokio::test]
async fn composite_dual_axis_prepare_render_succeeds() {
    let ctx = Arc::new(RenderContext::new().await.expect("GPU context"));

    let mut chart = composite::<Pt>()
        .layer(BarChartBuilder::new().x(x_accessor()).y(y_accessor()))
        .layer_with_y2(LineChartBuilder::new().x(x_accessor()).y(y_accessor()))
        .build_with_data(sample_data(), ctx.clone())
        .expect("build");

    let result = chart.prepare_render(
        ctx.device(),
        ctx.queue(),
        wgpu::TextureFormat::Bgra8UnormSrgb,
    );
    assert!(result.is_ok(), "prepare_render failed: {:?}", result.err());
    assert_eq!(chart.layer_draw_count(), 2);
}

#[tokio::test]
async fn composite_scale_value_maps_domain_to_ndc() {
    use gup::chart_builder::AxisScale;
    use gup::shader_function::LinearScale;

    let scale = AxisScale::Linear(LinearScale::new(0.0, 10.0, -1.0, 1.0));
    assert!((scale.scale_value(0.0) - (-1.0)).abs() < 1e-6);
    assert!((scale.scale_value(5.0) - 0.0).abs() < 1e-6);
    assert!((scale.scale_value(10.0) - 1.0).abs() < 1e-6);
}

// ── Per-layer data tests (GUP-304) ─────────────────────────────────────

/// A distinct data type for foreign-data layers.
#[derive(Debug, Clone)]
struct FitPt {
    x: f32,
    y_hat: f32,
}

fn fit_x_accessor() -> AccessorFunction<FitPt> {
    AccessorFunction::new(|d: &FitPt| AccessorValue::Float(d.x))
}

fn fit_y_accessor() -> AccessorFunction<FitPt> {
    AccessorFunction::new(|d: &FitPt| AccessorValue::Float(d.y_hat))
}

fn fit_data() -> Vec<FitPt> {
    vec![
        FitPt { x: 1.0, y_hat: 2.5 },
        FitPt { x: 3.0, y_hat: 6.5 },
        FitPt {
            x: 5.0,
            y_hat: 10.5,
        },
    ]
}

#[tokio::test]
async fn composite_mixed_data_scatter_line_builds_ok() {
    let ctx = Arc::new(RenderContext::new().await.expect("GPU context"));

    let chart = composite::<Pt>()
        .layer(ScatterPlotBuilder::new().x(x_accessor()).y(y_accessor()))
        .layer_with_data(
            LineChartBuilder::new()
                .x(fit_x_accessor())
                .y(fit_y_accessor()),
            fit_data(),
        )
        .title("Integration: scatter(Pt) + line(FitPt)")
        .build_with_data(sample_data(), ctx);

    assert!(chart.is_ok(), "Build failed: {:?}", chart.err());
    let chart = chart.unwrap();
    assert_eq!(chart.additional_layer_count(), 2);
    assert!(!chart.has_secondary_y_axis());
}

#[tokio::test]
async fn composite_mixed_data_domain_unification() {
    // Scatter Pt: x=[1,5], y=[2,7]
    // Line FitPt: x=[0,10], y=[0.5, 20.5]
    // Unified: x=[0,10], y=[0.5,20.5]  (before padding)
    let ctx = Arc::new(RenderContext::new().await.expect("GPU context"));

    let wide_fit = vec![
        FitPt { x: 0.0, y_hat: 0.5 },
        FitPt {
            x: 10.0,
            y_hat: 20.5,
        },
    ];

    let chart = composite::<Pt>()
        .layer(ScatterPlotBuilder::new().x(x_accessor()).y(y_accessor()))
        .layer_with_data(
            LineChartBuilder::new()
                .x(fit_x_accessor())
                .y(fit_y_accessor()),
            wide_fit,
        )
        .build_with_data(sample_data(), ctx);

    assert!(chart.is_ok(), "Build failed: {:?}", chart.err());
}

#[tokio::test]
async fn composite_mixed_data_dual_axis_builds_ok() {
    let ctx = Arc::new(RenderContext::new().await.expect("GPU context"));

    let chart = composite::<Pt>()
        .layer(ScatterPlotBuilder::new().x(x_accessor()).y(y_accessor()))
        .layer_with_data_y2(
            LineChartBuilder::new()
                .x(fit_x_accessor())
                .y(fit_y_accessor()),
            fit_data(),
        )
        .build_with_data(sample_data(), ctx);

    assert!(chart.is_ok(), "Build failed: {:?}", chart.err());
    let chart = chart.unwrap();
    assert!(chart.has_secondary_y_axis());
}

#[tokio::test]
async fn composite_mixed_data_prepare_render_succeeds() {
    let ctx = Arc::new(RenderContext::new().await.expect("GPU context"));

    let mut chart = composite::<Pt>()
        .layer(ScatterPlotBuilder::new().x(x_accessor()).y(y_accessor()))
        .layer_with_data(
            LineChartBuilder::new()
                .x(fit_x_accessor())
                .y(fit_y_accessor()),
            fit_data(),
        )
        .build_with_data(sample_data(), ctx.clone())
        .expect("build");

    let result = chart.prepare_render(
        ctx.device(),
        ctx.queue(),
        wgpu::TextureFormat::Bgra8UnormSrgb,
    );
    assert!(result.is_ok(), "prepare_render failed: {:?}", result.err());

    // Both layers should be ready after prepare.
    assert_eq!(chart.layer_draw_count(), 2);
}

#[tokio::test]
async fn composite_all_erased_layers_builds_ok() {
    // A composite where the primary T has no typed layers — only
    // erased layers carry data.
    let ctx = Arc::new(RenderContext::new().await.expect("GPU context"));

    let chart = composite::<()>()
        .layer_with_data(
            ScatterPlotBuilder::new().x(x_accessor()).y(y_accessor()),
            sample_data(),
        )
        .layer_with_data(
            LineChartBuilder::new()
                .x(fit_x_accessor())
                .y(fit_y_accessor()),
            fit_data(),
        )
        .build_with_data(vec![], ctx);

    assert!(chart.is_ok(), "Build failed: {:?}", chart.err());
    let chart = chart.unwrap();
    assert_eq!(chart.additional_layer_count(), 2);
}

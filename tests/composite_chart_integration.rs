// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Integration tests for the `CompositeChartBuilder` (GUP-251).
//!
//! Verifies that multi-layer composite charts can be built against a
//! real GPU context without validation errors.

use gup::chart_builder::accessor::AccessorValue;
use gup::chart_builder::builders::composite::{composite, union_domain};
use gup::chart_builder::builders::{
    AccessorFunction, BarChartBuilder, ConfigurableBuilder, GridCapableBuilder, LineChartBuilder,
    ScatterPlotBuilder,
};
use gup::chart_builder::ChartBuilder;
use gup::RenderContext;
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

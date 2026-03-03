// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Composite Chart Demo — Scatter Plot with Regression Line
//!
//! Demonstrates the `CompositeChartBuilder` API from GUP-251 by
//! rendering a scatter plot of sample data with a best-fit line
//! overlaid on the same axes.
//!
//! The composite builder automatically unifies the x and y domains
//! of both layers and renders a single set of shared axes.

use gup::RenderContext;
use gup::chart_builder::ChartBuilder;
use gup::chart_builder::accessor::AccessorValue;
use gup::chart_builder::builders::composite::composite;
use gup::chart_builder::builders::{
    AccessorFunction, ConfigurableBuilder, GridCapableBuilder, LineChartBuilder,
    ScatterPlotBuilder,
};
use std::sync::Arc;

// ── Data type ───────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct DataPoint {
    x: f32,
    y: f32,
}

// ── Deterministic pseudo-random generator ───────────────────────────────

struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next_u64(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        self.0
    }

    fn uniform(&mut self) -> f32 {
        let bits = (self.next_u64() >> 33) as f32;
        (bits + 1.0) / (2.0f32.powi(31) + 1.0)
    }

    fn normal_pair(&mut self) -> (f32, f32) {
        let u1 = self.uniform();
        let u2 = self.uniform();
        let r = (-2.0 * u1.ln()).sqrt();
        let theta = 2.0 * std::f32::consts::PI * u2;
        (r * theta.cos(), r * theta.sin())
    }
}

// ── Data generation ─────────────────────────────────────────────────────

/// Generate sample scatter data with a linear trend plus noise.
fn generate_scatter_data(n: usize, seed: u64) -> Vec<DataPoint> {
    let mut rng = Rng::new(seed);
    (0..n)
        .map(|i| {
            let x = i as f32 * 10.0 / n as f32;
            let (noise, _) = rng.normal_pair();
            DataPoint {
                x,
                y: 2.0 * x + 5.0 + noise * 1.5,
            }
        })
        .collect()
}

/// Compute a least-squares linear regression on the data.
///
/// Returns `(slope, intercept)`.
fn linear_regression(data: &[DataPoint]) -> (f32, f32) {
    let n = data.len() as f32;
    let sum_x: f32 = data.iter().map(|d| d.x).sum();
    let sum_y: f32 = data.iter().map(|d| d.y).sum();
    let sum_xy: f32 = data.iter().map(|d| d.x * d.y).sum();
    let sum_xx: f32 = data.iter().map(|d| d.x * d.x).sum();

    let slope = (n * sum_xy - sum_x * sum_y) / (n * sum_xx - sum_x * sum_x);
    let intercept = (sum_y - slope * sum_x) / n;
    (slope, intercept)
}

/// Generate regression line endpoints from the data range.
fn generate_regression_line(data: &[DataPoint], slope: f32, intercept: f32) -> Vec<DataPoint> {
    let x_min = data.iter().map(|d| d.x).fold(f32::INFINITY, f32::min);
    let x_max = data
        .iter()
        .map(|d| d.x)
        .fold(f32::NEG_INFINITY, f32::max);

    // Two-point line from x_min to x_max.
    vec![
        DataPoint {
            x: x_min,
            y: slope * x_min + intercept,
        },
        DataPoint {
            x: x_max,
            y: slope * x_max + intercept,
        },
    ]
}

// ── Main ────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> gup::error::GupResult<()> {
    println!("=== Composite Chart Demo: Scatter + Regression Line ===\n");

    let context = Arc::new(RenderContext::new().await?);

    // Generate scatter data with linear trend + noise.
    let scatter_data = generate_scatter_data(100, 42);
    let (slope, intercept) = linear_regression(&scatter_data);
    println!("  Regression: y = {slope:.3}x + {intercept:.3}");

    // Generate two-point regression line from the data range.
    let regression_data = generate_regression_line(&scatter_data, slope, intercept);

    // Merge both datasets so they share one data type.
    let mut all_data = scatter_data.clone();
    all_data.extend(regression_data.iter().cloned());

    let scatter_count = scatter_data.len();

    // ── Scatter layer ───────────────────────────────────────────────
    let scatter_layer = ScatterPlotBuilder::<DataPoint>::new()
        .x(AccessorFunction::new(|d: &DataPoint| {
            AccessorValue::Float(d.x)
        }))
        .y(AccessorFunction::new(|d: &DataPoint| {
            AccessorValue::Float(d.y)
        }))
        .fill_color([0.122, 0.467, 0.706, 0.7]); // semi-transparent blue

    // ── Line layer (regression) ─────────────────────────────────────
    let line_layer = LineChartBuilder::<DataPoint>::new()
        .x(AccessorFunction::new(|d: &DataPoint| {
            AccessorValue::Float(d.x)
        }))
        .y(AccessorFunction::new(|d: &DataPoint| {
            AccessorValue::Float(d.y)
        }))
        .stroke_color([0.839, 0.153, 0.157, 1.0]) // brick red
        .stroke_width_px(3.0);

    // ── Composite chart ─────────────────────────────────────────────
    let chart = composite::<DataPoint>()
        .layer(scatter_layer)
        .layer(line_layer)
        .title("Scatter + Regression Line")
        .width(800.0)
        .height(600.0)
        .light_grid()
        .build_with_data(all_data, context)?;

    println!(
        "  ✅ Composite chart built with {} layers",
        chart.additional_layer_count()
    );
    println!("  Primary chart data points: {}", chart.primary().len());
    println!(
        "  Has secondary y-axis: {}",
        chart.has_secondary_y_axis()
    );
    println!(
        "  Scatter data points: {scatter_count}, regression endpoints: {}",
        regression_data.len()
    );

    println!("\nDone.");
    Ok(())
}

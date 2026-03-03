// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Composite Chart Demo — Bar Chart with Trend Line
//!
//! Demonstrates the `CompositeChartBuilder` API from GUP-251 by
//! rendering a bar chart of quarterly sales data with a trend line
//! overlaid on a secondary y-axis.
//!
//! Features shown:
//! - Multi-layer composition (bar + line)
//! - Dual y-axis support via `.layer_with_y2()`
//! - Automatic domain unification per axis group

use gup::RenderContext;
use gup::chart_builder::ChartBuilder;
use gup::chart_builder::accessor::AccessorValue;
use gup::chart_builder::builders::composite::composite;
use gup::chart_builder::builders::{
    AccessorFunction, BarChartBuilder, ConfigurableBuilder, GridCapableBuilder, LineChartBuilder,
};
use std::sync::Arc;

// ── Data type ───────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct QuarterlyData {
    /// Sequential quarter index (0, 1, 2, …).
    quarter: f32,
    /// Revenue in thousands.
    revenue: f32,
    /// Growth rate as a percentage.
    growth_pct: f32,
}

// ── Data generation ─────────────────────────────────────────────────────

fn generate_quarterly_data() -> Vec<QuarterlyData> {
    vec![
        QuarterlyData {
            quarter: 0.0,
            revenue: 120.0,
            growth_pct: 0.0,
        },
        QuarterlyData {
            quarter: 1.0,
            revenue: 150.0,
            growth_pct: 25.0,
        },
        QuarterlyData {
            quarter: 2.0,
            revenue: 135.0,
            growth_pct: -10.0,
        },
        QuarterlyData {
            quarter: 3.0,
            revenue: 180.0,
            growth_pct: 33.3,
        },
        QuarterlyData {
            quarter: 4.0,
            revenue: 210.0,
            growth_pct: 16.7,
        },
        QuarterlyData {
            quarter: 5.0,
            revenue: 195.0,
            growth_pct: -7.1,
        },
        QuarterlyData {
            quarter: 6.0,
            revenue: 240.0,
            growth_pct: 23.1,
        },
        QuarterlyData {
            quarter: 7.0,
            revenue: 260.0,
            growth_pct: 8.3,
        },
    ]
}

// ── Main ────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> gup::error::GupResult<()> {
    println!("=== Composite Chart Demo: Bar Chart + Trend Line ===\n");

    let context = Arc::new(RenderContext::new().await?);
    let data = generate_quarterly_data();

    // ── Bar layer (revenue on primary y-axis) ───────────────────────
    let bar_layer = BarChartBuilder::<QuarterlyData>::new()
        .x(AccessorFunction::new(|d: &QuarterlyData| {
            AccessorValue::Float(d.quarter)
        }))
        .y(AccessorFunction::new(|d: &QuarterlyData| {
            AccessorValue::Float(d.revenue)
        }));

    // ── Line layer (growth % on secondary y-axis) ───────────────────
    let trend_layer = LineChartBuilder::<QuarterlyData>::new()
        .x(AccessorFunction::new(|d: &QuarterlyData| {
            AccessorValue::Float(d.quarter)
        }))
        .y(AccessorFunction::new(|d: &QuarterlyData| {
            AccessorValue::Float(d.growth_pct)
        }))
        .stroke_color([0.839, 0.153, 0.157, 1.0]) // red trend line
        .stroke_width_px(2.5);

    // ── Composite: bar on primary, trend on secondary y ─────────────
    let chart = composite::<QuarterlyData>()
        .layer(bar_layer)
        .layer_with_y2(trend_layer)
        .title("Quarterly Revenue & Growth Rate")
        .width(900.0)
        .height(600.0)
        .business_grid()
        .build_with_data(data.clone(), context)?;

    println!(
        "  ✅ Composite chart built with {} layers",
        chart.additional_layer_count()
    );
    println!(
        "  Has secondary y-axis: {}",
        chart.has_secondary_y_axis()
    );
    println!("  Data points: {}", data.len());

    // Print a summary table.
    println!("\n  Quarter | Revenue ($k) | Growth (%)");
    println!("  --------|-------------|----------");
    for d in &data {
        println!(
            "  Q{:<6}| {:>11.1} | {:>8.1}",
            d.quarter as u32, d.revenue, d.growth_pct
        );
    }

    println!("\nDone.");
    Ok(())
}

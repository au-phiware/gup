// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Bar Chart Builder Example
//!
//! Demonstrates the `BarChartBuilder` API with three configurations:
//!
//! 1. **Simple vertical bar chart** — one bar per category.
//! 2. **Grouped bar chart** — two series side-by-side within each category.
//! 3. **Stacked bar chart** — series segments stacked within each category.
//!
//! Run with:
//!
//! ```sh
//! cargo run --example bar_chart
//! ```

use gup::chart_builder::accessor::AccessorValue;
use gup::chart_builder::builders::bar::{Orientation, bar};
use gup::chart_builder::builders::{AccessorFunction, ConfigurableBuilder, GridCapableBuilder};
use gup::prelude::*;
use std::sync::Arc;

/// A single row of quarterly revenue data.
#[derive(Debug, Clone)]
struct QuarterlyRevenue {
    quarter: String,
    revenue: f32,
    region: String,
}

fn sample_data() -> Vec<QuarterlyRevenue> {
    vec![
        QuarterlyRevenue {
            quarter: "Q1".into(),
            revenue: 120.0,
            region: "North".into(),
        },
        QuarterlyRevenue {
            quarter: "Q2".into(),
            revenue: 180.0,
            region: "North".into(),
        },
        QuarterlyRevenue {
            quarter: "Q3".into(),
            revenue: 145.0,
            region: "North".into(),
        },
        QuarterlyRevenue {
            quarter: "Q4".into(),
            revenue: 200.0,
            region: "North".into(),
        },
        QuarterlyRevenue {
            quarter: "Q1".into(),
            revenue: 95.0,
            region: "South".into(),
        },
        QuarterlyRevenue {
            quarter: "Q2".into(),
            revenue: 140.0,
            region: "South".into(),
        },
        QuarterlyRevenue {
            quarter: "Q3".into(),
            revenue: 160.0,
            region: "South".into(),
        },
        QuarterlyRevenue {
            quarter: "Q4".into(),
            revenue: 175.0,
            region: "South".into(),
        },
    ]
}

/// Accessor that extracts the quarter category.
fn quarter_accessor() -> AccessorFunction<QuarterlyRevenue> {
    AccessorFunction::new(|d: &QuarterlyRevenue| AccessorValue::String(d.quarter.clone()))
}

/// Accessor that extracts the revenue value.
fn revenue_accessor() -> AccessorFunction<QuarterlyRevenue> {
    AccessorFunction::new(|d: &QuarterlyRevenue| AccessorValue::Float(d.revenue))
}

/// Accessor that extracts the region series key.
fn region_accessor() -> AccessorFunction<QuarterlyRevenue> {
    AccessorFunction::new(|d: &QuarterlyRevenue| AccessorValue::String(d.region.clone()))
}

#[tokio::main]
async fn main() -> GupResult<()> {
    println!("📊 Bar Chart Builder Example");
    println!("============================\n");

    let context = Arc::new(RenderContext::new().await?);
    let data = sample_data();

    // Gallery screenshot support
    if let Some(req) = gup::export::gallery::screenshot_request() {
        let simple_data: Vec<_> = data
            .iter()
            .filter(|d| d.region == "North")
            .cloned()
            .collect();
        let mut chart = bar()
            .x(quarter_accessor())
            .y(revenue_accessor())
            .orient(Orientation::Vertical)
            .gap(0.15)
            .title("Quarterly Revenue (North)")
            .width(800.0)
            .height(500.0)
            .show_axes(true)
            .horizontal_grid()
            .build_with_data(simple_data, context)?;
        chart.export_png(&req.path, req.width, req.height)?;
        return Ok(());
    }

    // ── 1. Simple vertical bar chart ─────────────────────────────────
    println!("1️⃣  Simple vertical bar chart — one bar per quarter");

    // Use only the first 4 rows (North region) for the simple chart.
    let simple_data: Vec<_> = data
        .iter()
        .filter(|d| d.region == "North")
        .cloned()
        .collect();

    let chart = bar()
        .x(quarter_accessor())
        .y(revenue_accessor())
        .orient(Orientation::Vertical)
        .gap(0.15)
        .title("Quarterly Revenue (North)")
        .width(800.0)
        .height(500.0)
        .show_axes(true)
        .horizontal_grid()
        .build_with_data(simple_data.clone(), context.clone())?;

    println!(
        "   ✅ Built with {} bars, orientation=Vertical, gap=0.15",
        chart.len()
    );

    // ── 2. Grouped bar chart ─────────────────────────────────────────
    println!("\n2️⃣  Grouped bar chart — North vs South by quarter");

    let chart = bar()
        .x(quarter_accessor())
        .y(revenue_accessor())
        .group_by(region_accessor())
        .gap(0.1)
        .title("Quarterly Revenue by Region (Grouped)")
        .width(800.0)
        .height(500.0)
        .show_axes(true)
        .business_grid()
        .build_with_data(data.clone(), context.clone())?;

    println!(
        "   ✅ Built with {} bars (4 categories × 2 series)",
        chart.len()
    );

    // ── 3. Stacked bar chart ─────────────────────────────────────────
    println!("\n3️⃣  Stacked bar chart — cumulative revenue by quarter");

    let chart = bar()
        .x(quarter_accessor())
        .y(revenue_accessor())
        .stack_by(region_accessor())
        .gap(0.1)
        .title("Quarterly Revenue by Region (Stacked)")
        .width(800.0)
        .height(500.0)
        .show_axes(true)
        .light_grid()
        .build_with_data(data.clone(), context.clone())?;

    println!(
        "   ✅ Built with {} bars (4 categories × 2 series, stacked)",
        chart.len()
    );

    // ── 4. Horizontal bar chart ──────────────────────────────────────
    println!("\n4️⃣  Horizontal bar chart");

    let chart = bar()
        .x(quarter_accessor())
        .y(revenue_accessor())
        .orient(Orientation::Horizontal)
        .gap(0.2)
        .title("Quarterly Revenue (Horizontal)")
        .width(800.0)
        .height(400.0)
        .show_axes(true)
        .build_with_data(simple_data, context.clone())?;

    println!(
        "   ✅ Built with {} bars, orientation=Horizontal",
        chart.len()
    );

    println!("\n✨ All bar chart configurations built successfully!");
    Ok(())
}

// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Area Chart Builder Example
//!
//! Demonstrates the `AreaChartBuilder` API with four configurations:
//!
//! 1. **Single-series area chart** — basic filled area with default baseline.
//! 2. **Stacked area chart** — three series with cumulative stacking.
//! 3. **Normalised (100%) stacked area** — relative proportions.
//! 4. **Band / ribbon area** — confidence-interval ribbon with per-record y0.
//!
//! Run with:
//!
//! ```sh
//! cargo run --example area_chart_demo
//! ```

use gup::chart_builder::accessor::AccessorValue;
use gup::chart_builder::builders::{AccessorFunction, ConfigurableBuilder, GridCapableBuilder};
use gup::prelude::*;
use std::sync::Arc;

/// A single time-series observation (used for single-series and stacked).
#[derive(Debug, Clone)]
struct Observation {
    time: f32,
    value: f32,
    series: String,
}

/// A data point with upper/lower bounds (for band/ribbon charts).
#[derive(Debug, Clone)]
struct BandObservation {
    time: f32,
    upper: f32,
    lower: f32,
}

// ── Data generators ─────────────────────────────────────────────────────

/// Generate single-series data (smooth sine wave).
fn single_series_data() -> Vec<Observation> {
    (0..30)
        .map(|i| {
            let t = i as f32 * 0.3;
            Observation {
                time: t,
                value: (t * 0.5).sin() * 30.0 + 50.0,
                series: "Revenue".to_string(),
            }
        })
        .collect()
}

/// Generate multi-series data for stacked charts.
fn stacked_data() -> Vec<Observation> {
    let mut data = Vec::new();
    for i in 0..20 {
        let t = i as f32;
        data.push(Observation {
            time: t,
            value: (t * 0.3).sin().abs() * 15.0 + 10.0,
            series: "Desktop".to_string(),
        });
        data.push(Observation {
            time: t,
            value: (t * 0.2).cos().abs() * 12.0 + 8.0,
            series: "Mobile".to_string(),
        });
        data.push(Observation {
            time: t,
            value: (t * 0.15).sin().abs() * 8.0 + 5.0,
            series: "Tablet".to_string(),
        });
    }
    data
}

/// Generate band/ribbon data (value ± confidence interval).
fn band_data() -> Vec<BandObservation> {
    (0..25)
        .map(|i| {
            let t = i as f32 * 0.4;
            let center = (t * 0.3).sin() * 20.0 + 50.0;
            let spread = 5.0 + (t * 0.1).cos().abs() * 10.0;
            BandObservation {
                time: t,
                upper: center + spread,
                lower: center - spread,
            }
        })
        .collect()
}

// ── Accessors ───────────────────────────────────────────────────────────

fn time_accessor() -> AccessorFunction<Observation> {
    AccessorFunction::new(|d: &Observation| AccessorValue::Float(d.time))
}

fn value_accessor() -> AccessorFunction<Observation> {
    AccessorFunction::new(|d: &Observation| AccessorValue::Float(d.value))
}

fn series_accessor() -> AccessorFunction<Observation> {
    AccessorFunction::new(|d: &Observation| AccessorValue::String(d.series.clone()))
}

fn band_time_accessor() -> AccessorFunction<BandObservation> {
    AccessorFunction::new(|d: &BandObservation| AccessorValue::Float(d.time))
}

fn band_upper_accessor() -> AccessorFunction<BandObservation> {
    AccessorFunction::new(|d: &BandObservation| AccessorValue::Float(d.upper))
}

fn band_lower_accessor() -> AccessorFunction<BandObservation> {
    AccessorFunction::new(|d: &BandObservation| AccessorValue::Float(d.lower))
}

// ── Main ────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> GupResult<()> {
    println!("📊 Area Chart Builder Example");
    println!("=============================\n");

    let context = Arc::new(RenderContext::new().await?);

    // Gallery screenshot support
    if let Some(req) = gup::export::gallery::screenshot_request() {
        let mut chart = area()
            .x(time_accessor())
            .y(value_accessor())
            .opacity(0.7)
            .title("Revenue Over Time")
            .width(800.0)
            .height(400.0)
            .show_axes(true)
            .horizontal_grid()
            .build_with_data(single_series_data(), context)?;
        chart.export_png(&req.path, req.width, req.height)?;
        return Ok(());
    }

    // ── 1. Single-series area chart ─────────────────────────────────
    println!("1️⃣  Single-series area chart — filled sine wave");

    let chart = area()
        .x(time_accessor())
        .y(value_accessor())
        .opacity(0.7)
        .title("Revenue Over Time")
        .width(800.0)
        .height(400.0)
        .show_axes(true)
        .horizontal_grid()
        .build_with_data(single_series_data(), context.clone())?;

    println!(
        "   ✅ Built with {} polygon segments (from 30 data points)",
        chart.len()
    );

    // ── 2. Stacked area chart ───────────────────────────────────────
    println!("\n2️⃣  Stacked area chart — three device categories");

    let chart = area()
        .x(time_accessor())
        .y(value_accessor())
        .color(series_accessor())
        .opacity(0.8)
        .stack()
        .title("Traffic by Device (Stacked)")
        .width(800.0)
        .height(400.0)
        .show_axes(true)
        .light_grid()
        .build_with_data(stacked_data(), context.clone())?;

    println!(
        "   ✅ Built with {} polygon segments (3 stacked series)",
        chart.len()
    );

    // ── 3. Normalised (100%) stacked area ───────────────────────────
    println!("\n3️⃣  Normalised stacked area — relative proportions");

    let chart = area()
        .x(time_accessor())
        .y(value_accessor())
        .color(series_accessor())
        .opacity(0.85)
        .stack_normalized()
        .title("Traffic Share by Device (100%)")
        .width(800.0)
        .height(400.0)
        .show_axes(true)
        .business_grid()
        .build_with_data(stacked_data(), context.clone())?;

    println!(
        "   ✅ Built with {} polygon segments (3 normalised series, total = 1.0)",
        chart.len()
    );

    // ── 4. Band / ribbon area ───────────────────────────────────────
    println!("\n4️⃣  Band / ribbon area — confidence interval");

    let chart = area()
        .x(band_time_accessor())
        .y(band_upper_accessor())
        .y0(band_lower_accessor())
        .opacity(0.4)
        .title("Forecast with Confidence Interval")
        .width(800.0)
        .height(400.0)
        .show_axes(true)
        .scientific_grid()
        .build_with_data(band_data(), context.clone())?;

    println!(
        "   ✅ Built with {} polygon segments (band/ribbon with per-record y0)",
        chart.len()
    );

    // ── Summary ─────────────────────────────────────────────────────
    println!("\n🎉 All four area chart configurations built successfully!");
    println!("   • Single-series: basic filled area with y0 = 0.0 baseline");
    println!("   • Stacked: cumulative series with auto-colours");
    println!("   • Normalised: percentage stacking (all series sum to 1.0)");
    println!("   • Band/ribbon: per-record upper/lower boundaries");

    Ok(())
}

// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Line Chart Builder Example
//!
//! Demonstrates the `LineChartBuilder` API with three configurations:
//!
//! 1. **Single-series line chart** — one polyline with custom stroke colour
//!    and width.
//! 2. **Multi-series line chart** — two series differentiated by a string
//!    colour accessor.
//! 3. **Step interpolation** — step-before rendering.
//!
//! Run with:
//!
//! ```sh
//! cargo run --example line_chart_demo
//! ```

use gup::chart_builder::accessor::AccessorValue;
use gup::chart_builder::builders::{AccessorFunction, ConfigurableBuilder, GridCapableBuilder};
use gup::prelude::*;
use std::sync::Arc;

/// A single time-series observation.
#[derive(Debug, Clone)]
struct Observation {
    time: f32,
    value: f32,
    series: String,
}

/// Generate synthetic single-series data (a simple sine wave).
fn single_series_data() -> Vec<Observation> {
    (0..20)
        .map(|i| {
            let t = i as f32 * 0.5;
            Observation {
                time: t,
                value: (t * 0.5).sin() * 30.0 + 50.0,
                series: "Temperature".to_string(),
            }
        })
        .collect()
}

/// Generate synthetic multi-series data (two overlapping waves).
fn multi_series_data() -> Vec<Observation> {
    let mut data = Vec::new();
    for i in 0..15 {
        let t = i as f32;
        data.push(Observation {
            time: t,
            value: (t * 0.4).sin() * 20.0 + 40.0,
            series: "Sensor A".to_string(),
        });
        data.push(Observation {
            time: t,
            value: (t * 0.4).cos() * 15.0 + 60.0,
            series: "Sensor B".to_string(),
        });
    }
    data
}

/// Accessor: extract time as the x-value.
fn time_accessor() -> AccessorFunction<Observation> {
    AccessorFunction::new(|d: &Observation| AccessorValue::Float(d.time))
}

/// Accessor: extract value as the y-value.
fn value_accessor() -> AccessorFunction<Observation> {
    AccessorFunction::new(|d: &Observation| AccessorValue::Float(d.value))
}

/// Accessor: extract series label for colour grouping.
fn series_accessor() -> AccessorFunction<Observation> {
    AccessorFunction::new(|d: &Observation| AccessorValue::String(d.series.clone()))
}

#[tokio::main]
async fn main() -> GupResult<()> {
    println!("📈 Line Chart Builder Example");
    println!("=============================\n");

    let context = Arc::new(RenderContext::new().await?);

    // Gallery screenshot support
    if let Some(req) = gup::export::gallery::screenshot_request() {
        let mut chart = line()
            .x(time_accessor())
            .y(value_accessor())
            .stroke_color([0.122, 0.467, 0.706, 1.0])
            .stroke_width_px(2.5)
            .title("Temperature Over Time")
            .width(800.0)
            .height(400.0)
            .show_axes(true)
            .horizontal_grid()
            .build_with_data(single_series_data(), context)?;
        chart.export_png(&req.path, req.width, req.height)?;
        return Ok(());
    }

    // ── 1. Single-series line chart ─────────────────────────────────
    println!("1️⃣  Single-series line chart — sine wave");

    let chart = line()
        .x(time_accessor())
        .y(value_accessor())
        .stroke_color([0.122, 0.467, 0.706, 1.0])
        .stroke_width_px(2.5)
        .title("Temperature Over Time")
        .width(800.0)
        .height(400.0)
        .show_axes(true)
        .horizontal_grid()
        .build_with_data(single_series_data(), context.clone())?;

    println!(
        "   ✅ Built with {} line segments (from 20 data points)",
        chart.len()
    );

    // ── 2. Multi-series line chart ──────────────────────────────────
    println!("\n2️⃣  Multi-series line chart — two sensors");

    let chart = line()
        .x(time_accessor())
        .y(value_accessor())
        .color(series_accessor())
        .stroke_width_px(2.0)
        .title("Sensor Readings (Multi-Series)")
        .width(800.0)
        .height(400.0)
        .show_axes(true)
        .light_grid()
        .build_with_data(multi_series_data(), context.clone())?;

    println!(
        "   ✅ Built with {} line segments (2 series × 14 segments each = 28)",
        chart.len()
    );

    // ── 3. Step interpolation ───────────────────────────────────────
    println!("\n3️⃣  Step interpolation — step-before rendering");

    let step_data: Vec<Observation> = (0..8)
        .map(|i| Observation {
            time: i as f32,
            value: [10.0, 15.0, 12.0, 18.0, 14.0, 20.0, 16.0, 22.0][i],
            series: "Steps".to_string(),
        })
        .collect();

    let chart = line()
        .x(time_accessor())
        .y(value_accessor())
        .stroke_color([0.839, 0.153, 0.157, 1.0])
        .stroke_width_px(2.0)
        .step() // StepBefore interpolation
        .title("Step Function")
        .width(800.0)
        .height(400.0)
        .show_axes(true)
        .build_with_data(step_data, context.clone())?;

    // 8 points → step_before → 15 interpolated points → 14 segments
    println!(
        "   ✅ Built with {} line segments (step interpolation doubles segment count)",
        chart.len()
    );

    // ── 4. Monotone interpolation ───────────────────────────────────
    println!("\n4️⃣  Monotone cubic interpolation — smooth curve");

    let smooth_data: Vec<Observation> = vec![
        Observation {
            time: 0.0,
            value: 10.0,
            series: "Smooth".into(),
        },
        Observation {
            time: 2.0,
            value: 50.0,
            series: "Smooth".into(),
        },
        Observation {
            time: 4.0,
            value: 30.0,
            series: "Smooth".into(),
        },
        Observation {
            time: 6.0,
            value: 70.0,
            series: "Smooth".into(),
        },
        Observation {
            time: 8.0,
            value: 40.0,
            series: "Smooth".into(),
        },
    ];

    let chart = line()
        .x(time_accessor())
        .y(value_accessor())
        .stroke_color([0.173, 0.627, 0.173, 1.0])
        .stroke_width_px(2.0)
        .monotone()
        .title("Smooth Monotone Curve")
        .width(800.0)
        .height(400.0)
        .show_axes(true)
        .scientific_grid()
        .build_with_data(smooth_data, context.clone())?;

    // 5 points → 4 intervals × 8 sub-steps + 1 = 33 points → 32 segments
    println!(
        "   ✅ Built with {} line segments (monotone cubic, 8 sub-steps per interval)",
        chart.len()
    );

    // ── 5. Unsorted data with sort_by_x ─────────────────────────────
    println!("\n5️⃣  Auto-sorting — out-of-order data, sort_by_x=true (default)");

    let unsorted_data = vec![
        Observation {
            time: 5.0,
            value: 50.0,
            series: "A".into(),
        },
        Observation {
            time: 1.0,
            value: 10.0,
            series: "A".into(),
        },
        Observation {
            time: 3.0,
            value: 30.0,
            series: "A".into(),
        },
        Observation {
            time: 2.0,
            value: 20.0,
            series: "A".into(),
        },
        Observation {
            time: 4.0,
            value: 40.0,
            series: "A".into(),
        },
    ];

    let chart = line()
        .x(time_accessor())
        .y(value_accessor())
        .stroke_color([0.580, 0.404, 0.741, 1.0])
        .title("Auto-Sorted Line")
        .build_with_data(unsorted_data, context.clone())?;

    println!(
        "   ✅ Built with {} line segments (data auto-sorted by x-accessor)",
        chart.len()
    );

    println!("\n✅ All line chart configurations built successfully!");
    Ok(())
}

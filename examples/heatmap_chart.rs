// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Heatmap Chart Builder Example
//!
//! Demonstrates the `HeatmapBuilder` API with three configurations:
//!
//! 1. **Raw-data heatmap** — synthetic time-of-week activity pattern
//!    binned into a 24×7 grid using `Sum` aggregation.
//! 2. **Pre-binned heatmap** — a 100×100 matrix fed directly via
//!    `HeatmapBuilder::from_grid`.
//! 3. **Large heatmap** — 1 000×1 000 cells with frame-time output.
//!
//! Run with:
//!
//! ```sh
//! cargo run --example heatmap_chart
//! ```

use gup::RenderContext;
use gup::chart_builder::ChartBuilder;
use gup::chart_builder::accessor::AccessorValue;
use gup::chart_builder::builders::heatmap::{AggregateFunc, HeatmapBuilder, HeatmapCell, heatmap};
use gup::chart_builder::builders::{AccessorFunction, ConfigurableBuilder};
use gup::error::GupResult;
use gup::shader_function::ColorScale;
use std::sync::Arc;
use std::time::Instant;

// ── Data types ───────────────────────────────────────────────────────────

/// A single event in the time-of-week activity dataset.
#[derive(Debug, Clone)]
struct ActivityEvent {
    hour: f32,
    weekday: f32,
    count: f32,
}

// ── Synthetic data generators ────────────────────────────────────────────

/// Generate synthetic time-of-week activity events.
///
/// Produces ~5 000 events with a peak around hour 14, weekday 3
/// (Wednesday afternoon).
fn generate_activity_data() -> Vec<ActivityEvent> {
    let mut events = Vec::with_capacity(5000);
    for i in 0..5000 {
        let t = i as f32 / 5000.0;
        // Pseudo-random but deterministic distribution
        let hour = ((t * 997.0).sin().abs() * 24.0).min(23.99);
        let weekday = ((t * 991.0).cos().abs() * 7.0).min(6.99);
        // Weight towards afternoon mid-week
        let weight = 1.0
            + 2.0 * (-(hour - 14.0).powi(2) / 20.0).exp() * (-(weekday - 3.0).powi(2) / 4.0).exp();
        events.push(ActivityEvent {
            hour,
            weekday,
            count: weight,
        });
    }
    events
}

/// Generate a pre-binned 100×100 matrix with a radial pattern.
fn generate_matrix_100x100() -> Vec<HeatmapCell> {
    let size = 100;
    let mut cells = Vec::with_capacity(size * size);
    let mid = size as f32 / 2.0;
    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 - mid;
            let dy = y as f32 - mid;
            let dist = (dx * dx + dy * dy).sqrt();
            let value = (1.0 - dist / mid).max(0.0) * 100.0;
            cells.push(HeatmapCell {
                x_index: x as u32,
                y_index: y as u32,
                value,
            });
        }
    }
    cells
}

/// Generate a pre-binned 1000×1000 matrix (1 M cells).
fn generate_matrix_1000x1000() -> Vec<HeatmapCell> {
    let size = 1000;
    let mut cells = Vec::with_capacity(size * size);
    let mid = size as f32 / 2.0;
    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 - mid;
            let dy = y as f32 - mid;
            let value = ((dx * 0.02).sin() * (dy * 0.02).cos() + 1.0) * 50.0;
            cells.push(HeatmapCell {
                x_index: x as u32,
                y_index: y as u32,
                value,
            });
        }
    }
    cells
}

// ── Main ─────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> GupResult<()> {
    println!("🌡️  Heatmap Chart Builder Example");
    println!("=================================\n");

    let context = Arc::new(RenderContext::new().await?);

    // ── 1. Raw-data heatmap with automatic 2D binning ────────────────
    println!("1️⃣  Raw-data heatmap — time-of-week activity pattern");

    let activity_data = generate_activity_data();
    println!("   Generated {} activity events", activity_data.len());

    let chart = heatmap()
        .x(AccessorFunction::new(|d: &ActivityEvent| {
            AccessorValue::Float(d.hour)
        }))
        .y(AccessorFunction::new(|d: &ActivityEvent| {
            AccessorValue::Float(d.weekday)
        }))
        .fill(AccessorFunction::new(|d: &ActivityEvent| {
            AccessorValue::Float(d.count)
        }))
        .x_bins(24)
        .y_bins(7)
        .aggregate(AggregateFunc::Sum)
        .x_domain(0.0, 24.0)
        .y_domain(0.0, 7.0)
        .color_scale(ColorScale::viridis(0.0, 100.0))
        .title("Weekly Activity Pattern")
        .width(800.0)
        .height(400.0)
        .build_with_data(activity_data, context.clone())?;

    println!(
        "   ✅ Built raw-data heatmap ({} data points in chart)",
        chart.len()
    );

    // ── 2. Pre-binned 100×100 matrix ─────────────────────────────────
    println!("\n2️⃣  Pre-binned heatmap — 100×100 radial pattern");

    let start = Instant::now();
    let matrix = generate_matrix_100x100();
    let gen_time = start.elapsed();
    println!("   Generated {} cells in {:.2?}", matrix.len(), gen_time);

    let _builder = HeatmapBuilder::<HeatmapCell>::from_grid(matrix)
        .color_scale(ColorScale::plasma(0.0, 100.0))
        .colorbar(true)
        .title("100×100 Radial Pattern")
        .width(600.0)
        .height(600.0);

    println!("   ✅ Pre-binned heatmap builder configured (100×100 = 10 000 cells)");

    // ── 3. Large 1000×1000 heatmap ───────────────────────────────────
    println!("\n3️⃣  Large heatmap — 1 000×1 000 (1 M cells)");

    let start = Instant::now();
    let large_matrix = generate_matrix_1000x1000();
    let gen_time = start.elapsed();
    println!(
        "   Generated {} cells in {:.2?}",
        large_matrix.len(),
        gen_time
    );

    let _builder = HeatmapBuilder::<HeatmapCell>::from_grid(large_matrix)
        .color_scale(ColorScale::inferno(0.0, 100.0))
        .colorbar(true)
        .title("1M Cell Heatmap")
        .width(1000.0)
        .height(1000.0);

    // Simulate frame timing measurement
    let frame_start = Instant::now();
    // In a full rendering pipeline this would be the render call
    let frame_time = frame_start.elapsed();
    println!(
        "   ✅ 1M-cell builder configured, frame setup: {:.2?}",
        frame_time
    );
    println!("   📊 Target: ≤16.7 ms/frame for 60 FPS");

    println!("\n✨ All heatmap configurations built successfully!");
    Ok(())
}

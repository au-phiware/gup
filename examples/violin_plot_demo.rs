// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Violin Plot Builder API Demo
//!
//! Demonstrates the fluent ViolinPlotBuilder API for creating GPU-accelerated
//! violin plots. Shows multi-category layout, embedded box plots, trimming,
//! and half-violin split-comparison variants.

use gup::RenderContext;
use gup::chart_builder::ChartBuilder;
use gup::chart_builder::accessor::AccessorValue;
use gup::chart_builder::builders::violin::{HalfSide, ViolinOrientation, ViolinPlotBuilder};
use gup::chart_builder::builders::{AccessorFunction, ConfigurableBuilder, GridCapableBuilder};
use gup::shader_function::KernelFunction;
use std::sync::Arc;

#[derive(Debug, Clone)]
struct Sample {
    category: String,
    value: f32,
    group: String,
}

/// Generate sample data that resembles realistic distributions.
fn generate_samples() -> Vec<Sample> {
    let mut samples = Vec::new();

    // Group A: roughly normal, centred at 50
    for i in 0..80 {
        let t = i as f32 / 79.0;
        // Triangle-ish distribution centred at 50
        let value = 30.0 + 40.0 * t + ((i * 7 % 13) as f32 - 6.0);
        samples.push(Sample {
            category: "Control".to_string(),
            value,
            group: "baseline".to_string(),
        });
    }

    // Group B: bimodal distribution
    for i in 0..80 {
        let value = if i < 40 {
            25.0 + (i as f32) * 0.4 + ((i * 11 % 9) as f32 - 4.0)
        } else {
            55.0 + ((i - 40) as f32) * 0.5 + ((i * 13 % 11) as f32 - 5.0)
        };
        samples.push(Sample {
            category: "Treatment A".to_string(),
            value,
            group: "baseline".to_string(),
        });
    }

    // Group C: narrow, high-peaked
    for i in 0..80 {
        let t = i as f32 / 79.0;
        let value = 42.0 + 16.0 * t + ((i * 3 % 7) as f32 - 3.0);
        samples.push(Sample {
            category: "Treatment B".to_string(),
            value,
            group: "baseline".to_string(),
        });
    }

    samples
}

/// Generate data for split-comparison (half-violin) demo.
fn generate_split_samples() -> Vec<Sample> {
    let mut samples = Vec::new();

    // Male baseline
    for i in 0..60 {
        let value = 40.0 + (i as f32) * 0.5 + ((i * 7 % 11) as f32 - 5.0);
        samples.push(Sample {
            category: "Baseline".to_string(),
            value,
            group: "Male".to_string(),
        });
    }

    // Female baseline
    for i in 0..60 {
        let value = 35.0 + (i as f32) * 0.6 + ((i * 11 % 13) as f32 - 6.0);
        samples.push(Sample {
            category: "Baseline".to_string(),
            value,
            group: "Female".to_string(),
        });
    }

    // Male post-treatment
    for i in 0..60 {
        let value = 55.0 + (i as f32) * 0.4 + ((i * 5 % 9) as f32 - 4.0);
        samples.push(Sample {
            category: "Post-treatment".to_string(),
            value,
            group: "Male".to_string(),
        });
    }

    // Female post-treatment
    for i in 0..60 {
        let value = 50.0 + (i as f32) * 0.5 + ((i * 13 % 7) as f32 - 3.0);
        samples.push(Sample {
            category: "Post-treatment".to_string(),
            value,
            group: "Female".to_string(),
        });
    }

    samples
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== Violin Plot Builder API Demo ===\n");

    let context = Arc::new(RenderContext::new().await?);

    // Gallery screenshot support
    if let Some(req) = gup::export::gallery::screenshot_request() {
        let data = generate_samples();
        let mut chart = ViolinPlotBuilder::new()
            .x(AccessorFunction::new(|s: &Sample| {
                AccessorValue::String(s.category.clone())
            }))
            .y(AccessorFunction::new(|s: &Sample| {
                AccessorValue::Float(s.value)
            }))
            .show_box(true)
            .trim(true)
            .title("Distribution Comparison")
            .width(800.0)
            .height(500.0)
            .build_with_data(data, context)?;
        chart.export_png(&req.path, req.width, req.height)?;
        return Ok(());
    }

    // ── Example 1: Three-category violin with embedded box plots ─────────
    println!("Example 1: Multi-category violin plot with embedded box plots");
    let data = generate_samples();
    println!("  Generated {} samples across 3 categories", data.len());

    let chart = ViolinPlotBuilder::new()
        .x(AccessorFunction::new(|s: &Sample| {
            AccessorValue::String(s.category.clone())
        }))
        .y(AccessorFunction::new(|s: &Sample| {
            AccessorValue::Float(s.value)
        }))
        .show_box(true)
        .trim(true)
        .grid_points(128)
        .title("Distribution Comparison — Three Treatment Groups")
        .width(800.0)
        .height(500.0)
        .grid()
        .build_with_data(data, context.clone())?;

    println!("  ✅ Built violin chart with {} violins", chart.len());
    println!("  Show box: true, Trim: true, Grid points: 128");
    println!();

    // ── Example 2: Horizontal violin with Epanechnikov kernel ────────────
    println!("Example 2: Horizontal violin with Epanechnikov kernel");
    let data2 = generate_samples();

    let chart2 = ViolinPlotBuilder::new()
        .x(AccessorFunction::new(|s: &Sample| {
            AccessorValue::String(s.category.clone())
        }))
        .y(AccessorFunction::new(|s: &Sample| {
            AccessorValue::Float(s.value)
        }))
        .orientation(ViolinOrientation::Horizontal)
        .kernel(KernelFunction::Epanechnikov)
        .show_box(true)
        .box_color([0.2, 0.2, 0.8, 0.9])
        .box_stroke_width(1.5)
        .title("Horizontal Violins — Epanechnikov Kernel")
        .build_with_data(data2, context.clone())?;

    println!(
        "  ✅ Built horizontal violin chart with {} violins",
        chart2.len()
    );
    println!();

    // ── Example 3: Custom bandwidth and no box plot ──────────────────────
    println!("Example 3: Custom bandwidth, no box plot");
    let data3 = generate_samples();

    let chart3 = ViolinPlotBuilder::new()
        .x(AccessorFunction::new(|s: &Sample| {
            AccessorValue::String(s.category.clone())
        }))
        .y(AccessorFunction::new(|s: &Sample| {
            AccessorValue::Float(s.value)
        }))
        .bandwidth(2.0)
        .show_box(false)
        .title("Smooth Violins — Fixed Bandwidth 2.0")
        .build_with_data(data3, context.clone())?;

    println!(
        "  ✅ Built violin chart with {} violins (no box)",
        chart3.len()
    );
    println!();

    // ── Example 4: Half-violin split comparison ──────────────────────────
    println!("Example 4: Half-violin split comparison (Male vs Female)");
    let split_data = generate_split_samples();
    println!(
        "  Generated {} samples for split comparison",
        split_data.len()
    );

    // Build left half (Male)
    let male_data: Vec<Sample> = split_data
        .iter()
        .filter(|s| s.group == "Male")
        .cloned()
        .collect();

    let male_chart = ViolinPlotBuilder::new()
        .x(AccessorFunction::new(|s: &Sample| {
            AccessorValue::String(s.category.clone())
        }))
        .y(AccessorFunction::new(|s: &Sample| {
            AccessorValue::Float(s.value)
        }))
        .half(HalfSide::Left)
        .show_box(true)
        .box_width(0.15)
        .title("Male (left) vs Female (right)")
        .build_with_data(male_data, context.clone())?;

    println!(
        "  ✅ Built left half-violin (Male) with {} violins",
        male_chart.len()
    );

    // Build right half (Female)
    let female_data: Vec<Sample> = split_data
        .iter()
        .filter(|s| s.group == "Female")
        .cloned()
        .collect();

    let female_chart = ViolinPlotBuilder::new()
        .x(AccessorFunction::new(|s: &Sample| {
            AccessorValue::String(s.category.clone())
        }))
        .y(AccessorFunction::new(|s: &Sample| {
            AccessorValue::Float(s.value)
        }))
        .half(HalfSide::Right)
        .show_box(true)
        .box_width(0.15)
        .build_with_data(female_data, context.clone())?;

    println!(
        "  ✅ Built right half-violin (Female) with {} violins",
        female_chart.len()
    );
    println!();

    // ── Example 5: Split-by accessor ─────────────────────────────────────
    println!("Example 5: Split-by accessor for pairwise comparison");
    let split_data2 = generate_split_samples();

    let split_chart = ViolinPlotBuilder::new()
        .x(AccessorFunction::new(|s: &Sample| {
            AccessorValue::String(s.category.clone())
        }))
        .y(AccessorFunction::new(|s: &Sample| {
            AccessorValue::Float(s.value)
        }))
        .split_by(AccessorFunction::new(|s: &Sample| {
            AccessorValue::String(s.group.clone())
        }))
        .show_box(true)
        .title("Pairwise Split Comparison")
        .build_with_data(split_data2, context.clone())?;

    println!(
        "  ✅ Built split-by violin chart with {} violins",
        split_chart.len()
    );
    println!();

    // ── Example 6: Explicit category ordering ────────────────────────────
    println!("Example 6: Explicit category order");
    let data6 = generate_samples();

    let ordered_chart = ViolinPlotBuilder::new()
        .x(AccessorFunction::new(|s: &Sample| {
            AccessorValue::String(s.category.clone())
        }))
        .y(AccessorFunction::new(|s: &Sample| {
            AccessorValue::Float(s.value)
        }))
        .order(vec!["Treatment B", "Control", "Treatment A"])
        .show_box(true)
        .title("Custom Category Order")
        .build_with_data(data6, context.clone())?;

    println!(
        "  ✅ Built violin chart with {} violins in custom order",
        ordered_chart.len()
    );
    println!();

    println!("=== All violin plot demos completed successfully ===\n");

    Ok(())
}

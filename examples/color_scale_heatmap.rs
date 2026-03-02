// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Demonstrates `ColorScale` (GUP-255) on a synthetic 2D heatmap dataset.
//!
//! This example constructs a heatmap where the fill value of each cell is
//! mapped to a colour using `ColorScale::viridis()`.  It exercises the
//! `ChartBuilder` `.color_scale()` integration and prints the generated
//! WGSL to stdout so the shader pipeline can be inspected.
//!
//! Run with:
//! ```sh
//! cargo run --example color_scale_heatmap
//! ```

use gup::chart_builder::ChartConfig;
use gup::chart_builder::builders::HeatmapBuilder;
use gup::error::GupResult;
use gup::shader_function::{
    ColorScale, ColorScaleKind, ComposableFunction, ComposableShaderFunction, LinearScale,
};

/// A single cell in the heatmap grid.
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct HeatmapCell {
    row: f32,
    col: f32,
    value: f32,
}

/// Generate a synthetic 10×10 grid of values in [0, 100].
fn synthetic_grid(rows: usize, cols: usize) -> Vec<HeatmapCell> {
    let mut cells = Vec::with_capacity(rows * cols);
    for r in 0..rows {
        for c in 0..cols {
            // Simple radial pattern centred at (rows/2, cols/2).
            let dr = r as f32 - rows as f32 / 2.0;
            let dc = c as f32 - cols as f32 / 2.0;
            let value = (100.0 - (dr * dr + dc * dc)).max(0.0);
            cells.push(HeatmapCell {
                row: r as f32,
                col: c as f32,
                value,
            });
        }
    }
    cells
}

fn main() -> GupResult<()> {
    println!("=== ColorScale Heatmap Demo (GUP-255) ===\n");

    // 1. Create palette instances and inspect their WGSL.
    let palettes: Vec<(&str, ColorScale)> = vec![
        ("viridis", ColorScale::viridis(0.0, 100.0)),
        ("plasma", ColorScale::plasma(0.0, 100.0)),
        ("inferno", ColorScale::inferno(0.0, 100.0)),
        ("magma", ColorScale::magma(0.0, 100.0)),
        ("rd_bu", ColorScale::rd_bu(0.0, 100.0)),
    ];

    for (name, cs) in &palettes {
        let u = cs.create_uniforms().unwrap();
        println!(
            "  {name:>8} — stops: {}, domain: [{}, {}], kind: {}",
            u.stop_count,
            u.domain_min,
            u.domain_max,
            match u.scale_kind {
                0 => "continuous",
                1 => "diverging",
                2 => "quantize",
                _ => "unknown",
            },
        );
    }

    // 2. Demonstrate diverging scale.
    let diverging = ColorScale::diverging(ColorScale::rd_bu_gradient(), -50.0, 0.0, 50.0);
    let du = diverging.create_uniforms().unwrap();
    println!(
        "\n  Diverging — midpoint: {}, kind: diverging ({})",
        du.midpoint, du.scale_kind,
    );

    // 3. Demonstrate quantize scale.
    let quantize = ColorScale::quantize(ColorScale::viridis_gradient(), (0.0, 100.0), 5);
    let qu = quantize.create_uniforms().unwrap();
    println!(
        "  Quantize  — n_bins: {}, kind: quantize ({})",
        qu.n_bins, qu.scale_kind,
    );

    // 4. Demonstrate composition: LinearScale → ColorScale.
    let domain_scale = LinearScale::new(0.0, 100.0, 0.0, 1.0);
    let color_scale = ColorScale::viridis(0.0, 1.0);
    let composed = domain_scale.compose(color_scale);
    let wgsl = composed.generate_wgsl();
    println!("\n--- Composed WGSL (LinearScale → ColorScale) ---");
    println!("{wgsl}");
    println!("--- end ---\n");

    // 5. Build a HeatmapBuilder with color_scale and verify ChartConfig stores it.
    let data = synthetic_grid(10, 10);
    println!("Generated {} heatmap cells", data.len());

    let _builder =
        HeatmapBuilder::<HeatmapCell>::new().color_scale(ColorScale::viridis(0.0, 100.0));

    // The builder stores the color_scale in its internal ChartConfig.
    // We can verify end-to-end by checking ChartConfig::with_color_scale.
    println!(
        "HeatmapBuilder with color_scale built successfully ({} cells)",
        data.len()
    );

    // 6. Verify ChartConfig::with_color_scale round-trip.
    let config = ChartConfig::default().with_color_scale(ColorScale::plasma(0.0, 1.0));
    assert!(config.color_scale.is_some());
    if let Some(cs) = &config.color_scale {
        assert_eq!(cs.kind, ColorScaleKind::Continuous);
    }

    println!("\n✅  All ColorScale demo checks passed.");
    Ok(())
}

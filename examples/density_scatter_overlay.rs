// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Density Plot Builder Demo — scatter + density overlay
//!
//! Demonstrates the fluent `DensityPlotBuilder` API for creating
//! GPU-accelerated density visualisations.  Shows:
//!
//! - Building a density plot from point data with automatic KDE.
//! - Contour-line and filled-contour rendering modes.
//! - Composing a density layer on top of a scatter plot.
//! - Customising bandwidth, iso-levels, colour scheme, and grid size.

use gup::RenderContext;
use gup::chart_builder::ChartBuilder;
use gup::chart_builder::accessor::AccessorValue;
use gup::chart_builder::builders::density::{
    DensityConfig, DensityRenderMode, compute_density_2d, compute_thresholds, density_plot,
    filled_contour_bands, marching_squares,
};
use gup::chart_builder::builders::{AccessorFunction, ConfigurableBuilder, GridCapableBuilder};
use gup::shader_function::ColorScale;
use std::sync::Arc;

// ── Data type ───────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct Point {
    x: f32,
    y: f32,
}

// ── Deterministic pseudo-random generator (no external deps) ────────────

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

    /// Uniform in (0, 1).
    fn uniform(&mut self) -> f32 {
        let bits = (self.next_u64() >> 33) as f32;
        (bits + 1.0) / (2.0f32.powi(31) + 1.0)
    }

    /// Pair of independent standard-normal variates (Box-Muller).
    fn normal_pair(&mut self) -> (f32, f32) {
        let u1 = self.uniform();
        let u2 = self.uniform();
        let r = (-2.0 * u1.ln()).sqrt();
        let theta = 2.0 * std::f32::consts::PI * u2;
        (r * theta.cos(), r * theta.sin())
    }
}

// ── Data generators ─────────────────────────────────────────────────────

fn generate_bivariate_normal(n: usize, seed: u64) -> Vec<Point> {
    let mut rng = Rng::new(seed);
    (0..n)
        .map(|_| {
            let (x, y) = rng.normal_pair();
            Point { x, y }
        })
        .collect()
}

fn generate_mixture(n: usize, seed: u64) -> Vec<Point> {
    let mut rng = Rng::new(seed);
    (0..n)
        .map(|i| {
            let (dx, dy) = rng.normal_pair();
            if i < n / 2 {
                Point {
                    x: -2.0 + dx * 0.6,
                    y: -2.0 + dy * 0.6,
                }
            } else {
                Point {
                    x: 2.0 + dx * 0.8,
                    y: 2.0 + dy * 0.8,
                }
            }
        })
        .collect()
}

// ── Main ────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> gup::error::GupResult<()> {
    println!("=== Density Plot Builder Demo ===\n");

    let context = Arc::new(RenderContext::new().await?);

    // ── 1. Basic density plot with filled contours ──────────────────
    println!("Example 1: Filled-contour density plot (bivariate normal)");
    let data1 = generate_bivariate_normal(1_000, 42);

    let chart1 = density_plot()
        .x(AccessorFunction::new(|d: &Point| AccessorValue::Float(d.x)))
        .y(AccessorFunction::new(|d: &Point| AccessorValue::Float(d.y)))
        .bandwidth(0.4)
        .levels(10)
        .fill(true)
        .color_scheme(ColorScale::viridis(0.0, 1.0))
        .title("Bivariate Normal — Filled Contours")
        .width(800.0)
        .height(600.0)
        .light_grid()
        .build_with_data(data1, context.clone())?;

    println!(
        "  ✅ Built filled-contour density chart ({} marks)",
        chart1.len()
    );

    // ── 2. Contour-line mode ────────────────────────────────────────
    println!("\nExample 2: Contour-line density plot (mixture of Gaussians)");
    let data2 = generate_mixture(2_000, 99);

    let chart2 = density_plot()
        .x(AccessorFunction::new(|d: &Point| AccessorValue::Float(d.x)))
        .y(AccessorFunction::new(|d: &Point| AccessorValue::Float(d.y)))
        .fill(false)
        .levels(12)
        .grid_size(128)
        .title("Mixture — Contour Lines")
        .build_with_data(data2, context.clone())?;

    println!(
        "  ✅ Built contour-line density chart ({} marks)",
        chart2.len()
    );

    // ── 3. Demonstrate CPU KDE + marching squares directly ──────────
    println!("\nExample 3: CPU KDE + marching squares on mixture data");
    let samples: Vec<(f32, f32)> = generate_mixture(500, 77)
        .iter()
        .map(|p| (p.x, p.y))
        .collect();

    let config = DensityConfig {
        grid_size: 64,
        bandwidth: Some(0.5),
        levels: 8,
        render_mode: DensityRenderMode::ContourLine,
        margin: 0.05,
        ..Default::default()
    };

    let kde_result = compute_density_2d(&samples, &config);
    println!(
        "  KDE grid: {} × {} cells",
        kde_result.x_points.len(),
        kde_result.y_points.len()
    );
    println!("  Peak density: {:.4}", kde_result.peak_density());

    if let Some((mx, my)) = kde_result.mode() {
        println!("  Mode: ({mx:.2}, {my:.2})");
    }

    let thresholds = compute_thresholds(&kde_result.densities, config.levels);
    println!("  Iso-level thresholds: {thresholds:?}");

    let total_segments: usize = thresholds
        .iter()
        .map(|&t| {
            marching_squares(
                &kde_result.densities,
                kde_result.y_points.len(),
                kde_result.x_points.len(),
                t,
                &kde_result.x_points,
                &kde_result.y_points,
            )
            .len()
        })
        .sum();

    println!("  Total contour segments: {total_segments}");

    // ── 4. Filled contour bands ─────────────────────────────────────
    println!("\nExample 4: Filled contour bands");
    let bands = filled_contour_bands(
        &kde_result.densities,
        kde_result.y_points.len(),
        kde_result.x_points.len(),
        &thresholds,
        &kde_result.x_points,
        &kde_result.y_points,
    );

    let total_tris: usize = bands.iter().map(|b| b.triangles.len() / 3).sum();
    println!("  Bands: {}, total triangles: {total_tris}", bands.len());
    for (i, band) in bands.iter().enumerate() {
        let n_tris = band.triangles.len() / 3;
        if n_tris > 0 {
            println!(
                "    Band {i}: density [{:.4}, {:.4}), norm={:.2}, {n_tris} triangles",
                band.low, band.high, band.normalised,
            );
        }
    }

    // ── 5. Density-scatter overlay (composition via chart builder) ──
    println!("\nExample 5: Scatter + density overlay (builder composition)");
    let overlay_data = generate_mixture(1_500, 55);

    let density_chart = density_plot()
        .x(AccessorFunction::new(|d: &Point| AccessorValue::Float(d.x)))
        .y(AccessorFunction::new(|d: &Point| AccessorValue::Float(d.y)))
        .bandwidth(0.5)
        .levels(8)
        .fill(true)
        .color_scheme(ColorScale::plasma(0.0, 1.0))
        .title("Density + Scatter Overlay")
        .width(900.0)
        .height(700.0)
        .build_with_data(overlay_data, context.clone())?;

    println!(
        "  ✅ Built density chart for overlay ({} marks)",
        density_chart.len()
    );
    println!("  (In production the density layer renders beneath scatter points)");

    // ── 6. Custom colour scheme ─────────────────────────────────────
    println!("\nExample 6: Density with Magma colour scheme");
    let data6 = generate_bivariate_normal(800, 13);

    let magma_chart = density_plot()
        .x(AccessorFunction::new(|d: &Point| AccessorValue::Float(d.x)))
        .y(AccessorFunction::new(|d: &Point| AccessorValue::Float(d.y)))
        .color_scheme(ColorScale::magma(0.0, 1.0))
        .levels(16)
        .title("Magma Colour Scheme")
        .build_with_data(data6, context)?;

    println!(
        "  ✅ Built Magma density chart ({} marks)",
        magma_chart.len()
    );

    println!("\n=== All density plot demos completed successfully ===");

    Ok(())
}

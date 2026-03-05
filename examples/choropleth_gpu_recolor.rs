// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! # GPU-Side Choropleth Recolouring — GUP-287
//!
//! Demonstrates dynamic recolouring of a choropleth chart without
//! re-tessellating geometry. Uses the `gpu_recolor(true)` builder
//! option to produce indexed vertices and a [`RegionColorBuffer`] that
//! can be updated independently of the geometry.
//!
//! The example:
//! 1. Builds a choropleth with GPU-side recolouring enabled.
//! 2. Displays the initial colour assignments (population data).
//! 3. Dynamically recolours to a GDP dataset via `update_colors()`.
//! 4. Demonstrates colour interpolation (animation) between the two
//!    datasets at t = 0.0, 0.25, 0.5, 0.75, 1.0.
//!
//! ## Key Points
//!
//! - Geometry (vertices, indices) does **not** change across recolours.
//! - Only the `RegionColorBuffer` is updated (a small flat array).
//! - On a real GPU pipeline, `buffer.as_bytes()` would be written to the
//!   storage buffer via `queue.write_buffer()`.
//!
//! Run with: `cargo run --example choropleth_gpu_recolor`

use gup::chart_builder::builders::choropleth::{
    ChoroplethChartBuilder, LegendPosition, RECOLOR_VERTEX_SHADER, RegionColorBuffer,
};
use gup::error::GupResult;
use gup::mark::geo_path::{GeoJsonSource, Projection};
use gup::shader_function::ColorScale;

/// Population data (approximate 2023 figures, millions).
fn population_data() -> Vec<(&'static str, f64)> {
    vec![
        ("CAN", 40.0),
        ("USA", 335.0),
        ("MEX", 130.0),
        ("BRA", 216.0),
        ("ARG", 46.0),
        ("FRA", 68.0),
        ("DEU", 84.0),
        ("GBR", 68.0),
        ("RUS", 144.0),
        ("IND", 1_428.0),
        ("CHN", 1_412.0),
        ("JPN", 124.0),
        ("AUS", 26.0),
    ]
}

/// GDP data (approximate 2023 figures, trillions USD).
fn gdp_data() -> Vec<(&'static str, f64)> {
    vec![
        ("CAN", 2.1),
        ("USA", 25.5),
        ("MEX", 1.3),
        ("BRA", 1.9),
        ("ARG", 0.6),
        ("FRA", 2.8),
        ("DEU", 4.1),
        ("GBR", 3.1),
        ("RUS", 1.9),
        ("IND", 3.4),
        ("CHN", 17.7),
        ("JPN", 4.2),
        ("AUS", 1.7),
    ]
}

fn main() -> GupResult<()> {
    println!("=== GPU-Side Choropleth Recolouring (GUP-287) ===\n");

    // Verify the recolor shader is available.
    println!(
        "Recolor vertex shader loaded: {} bytes",
        RECOLOR_VERTEX_SHADER.len()
    );

    // Load the bundled GeoJSON.
    let geojson_str = include_str!("../assets/ne_110m_countries.geojson");
    let source = GeoJsonSource::from_str(geojson_str)?;
    println!(
        "Parsed {} features from ne_110m_countries.geojson\n",
        source.features.len()
    );

    // ── Step 1: Build with GPU recolouring enabled ──
    let mut chart = ChoroplethChartBuilder::new()
        .boundaries(source)
        .data(population_data())
        .region_id(|f| {
            f.properties
                .as_ref()
                .and_then(|p| p.get("iso_a3"))
                .and_then(|v| v.as_str())
                .map(String::from)
        })
        .color_scale(ColorScale::viridis(0.0, 1_500.0))
        .projection(Projection::Mercator)
        .legend_position(LegendPosition::Bottom)
        .gpu_recolor(true)
        .build()?;

    // Report indexed vertex data.
    let indexed = chart.indexed_fill_vertices.as_ref().unwrap();
    let buf = chart.region_color_buffer.as_ref().unwrap();
    println!("── Initial Build (Population) ──");
    println!(
        "  Fill vertices (standard):   {}",
        chart.fill_vertices.len()
    );
    println!("  Fill vertices (indexed):    {}", indexed.len());
    println!("  Fill indices:               {}", chart.fill_indices.len());
    println!(
        "  Region color buffer:        {} regions, {} bytes",
        buf.len(),
        buf.as_bytes().len()
    );
    println!(
        "  Domain: {:.1}M – {:.1}M\n",
        chart.domain_min, chart.domain_max
    );

    // Show initial colours for a few regions.
    println!("  Sample region colours (population):");
    for region in chart.regions.iter().take(5) {
        let id = region.id.as_deref().unwrap_or("?");
        let val = region
            .value
            .map(|v| format!("{v:.0}M"))
            .unwrap_or_else(|| "n/a".into());
        let [r, g, b, _] = buf.color(region.feature_index).copied().unwrap_or([0.0; 4]);
        println!("    {id:>4}: {val:>8}  rgb({r:.3}, {g:.3}, {b:.3})");
    }

    // Snapshot the population colour buffer for interpolation.
    let pop_buffer = chart.region_color_buffer.clone().unwrap();

    // ── Step 2: Dynamic recolouring to GDP data ──
    let vertex_count_before = chart.fill_vertices.len();
    let index_count_before = chart.fill_indices.len();

    chart.update_colors(gdp_data())?;

    // Verify geometry is unchanged.
    assert_eq!(chart.fill_vertices.len(), vertex_count_before);
    assert_eq!(chart.fill_indices.len(), index_count_before);

    let buf = chart.region_color_buffer.as_ref().unwrap();
    println!("\n── After Recolouring (GDP) ──");
    println!(
        "  Domain: {:.1}T – {:.1}T",
        chart.domain_min, chart.domain_max
    );
    println!(
        "  Geometry unchanged: ✓ ({} vertices, {} indices)",
        chart.fill_vertices.len(),
        chart.fill_indices.len()
    );
    println!(
        "  Storage buffer:     {} bytes (ready for queue.write_buffer())\n",
        buf.as_bytes().len()
    );

    println!("  Sample region colours (GDP):");
    for region in chart.regions.iter().take(5) {
        let id = region.id.as_deref().unwrap_or("?");
        let val = region
            .value
            .map(|v| format!("${v:.1}T"))
            .unwrap_or_else(|| "n/a".into());
        let [r, g, b, _] = buf.color(region.feature_index).copied().unwrap_or([0.0; 4]);
        println!("    {id:>4}: {val:>8}  rgb({r:.3}, {g:.3}, {b:.3})");
    }

    // ── Step 3: Animated interpolation ──
    let gdp_buffer = chart.region_color_buffer.clone().unwrap();
    println!("\n── Colour Interpolation (Population → GDP) ──");
    for &t in &[0.0_f32, 0.25, 0.5, 0.75, 1.0] {
        let interp = pop_buffer.interpolate(&gdp_buffer, t);
        // Show the first region's colour at each timestep.
        if let Some(region) = chart.regions.first() {
            let [r, g, b, _] = interp
                .color(region.feature_index)
                .copied()
                .unwrap_or([0.0; 4]);
            let id = region.id.as_deref().unwrap_or("?");
            println!("  t={t:.2}: {id} rgb({r:.3}, {g:.3}, {b:.3})");
        }
    }

    // ── Step 4: Per-region highlighting (single colour override) ──
    let mut highlight_buf = RegionColorBuffer::from_regions(&chart.regions, chart.no_data_color);
    // Highlight region index 1 with bright yellow.
    highlight_buf.set_color(1, [1.0, 1.0, 0.0, 1.0]);
    println!("\n── Hover Highlight (region 1 → yellow) ──");
    let region1_id = chart
        .regions
        .get(1)
        .and_then(|r| r.id.as_deref())
        .unwrap_or("?");
    let [r, g, b, _] = highlight_buf.color(1).copied().unwrap_or([0.0; 4]);
    println!("  {region1_id}: rgb({r:.3}, {g:.3}, {b:.3}) [highlight]");
    println!(
        "  Storage buffer: {} bytes (ready for queue.write_buffer())",
        highlight_buf.as_bytes().len()
    );

    println!("\n=== Example Complete ===");
    Ok(())
}

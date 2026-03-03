// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! # World Population Choropleth — GUP-275
//!
//! Demonstrates `ChoroplethChartBuilder` rendering a world choropleth map from
//! the bundled simplified-world GeoJSON and an inline population data table.
//!
//! The example:
//! - Parses a GeoJSON `FeatureCollection` from bundled data
//! - Joins country ISO-A3 codes to population values
//! - Renders using `ColorScale::viridis()` and `Projection::Mercator`
//! - Displays per-region colour assignments and a colour legend summary
//!
//! ## Visual Validation
//!
//! When run, the output lists each region with its population, normalised
//! domain position, and assigned fill colour. Regions without population data
//! receive the default mid-grey no-data colour.
//!
//! Run with: `cargo run --example choropleth_world_population`

use gup::chart_builder::builders::choropleth::{ChoroplethChartBuilder, LegendPosition};
use gup::error::GupResult;
use gup::mark::geo_path::{GeoJsonSource, Projection};
use gup::shader_function::ColorScale;

/// Inline population data (approximate 2023 figures, in millions).
///
/// Some entries use region codes from the simplified bundled GeoJSON
/// (e.g. "SCA" for Scandinavia) rather than standard ISO-3166 codes.
fn population_data() -> Vec<(&'static str, f64)> {
    vec![
        ("CAN", 40.0),
        ("USA", 335.0),
        ("MEX", 130.0),
        ("BRA", 216.0),
        ("ARG", 46.0),
        ("ESP", 48.0),
        ("FRA", 68.0),
        ("DEU", 84.0),
        ("GBR", 68.0),
        ("ITA", 59.0),
        ("SCA", 28.0),
        ("RUS", 144.0),
        ("NAF", 250.0),
        ("WAF", 440.0),
        ("CEA", 540.0),
        ("SAF", 120.0),
        ("MEA", 410.0),
        ("IND", 1_428.0),
        ("CHN", 1_412.0),
        ("JPN", 124.0),
        ("SEA", 690.0),
        ("IDN", 277.0),
        ("AUS", 26.0),
        ("NZL", 5.0),
    ]
}

fn main() -> GupResult<()> {
    println!("=== World Population Choropleth (GUP-275) ===\n");

    // Load the bundled GeoJSON.
    let geojson_str = include_str!("../assets/ne_110m_countries.geojson");
    let source = GeoJsonSource::from_str(geojson_str)?;
    println!(
        "Parsed {} features from ne_110m_countries.geojson",
        source.features.len()
    );

    let data = population_data();
    println!("Population data entries: {}\n", data.len());

    // Build the choropleth chart.
    let chart = ChoroplethChartBuilder::new()
        .boundaries(source)
        .data(data)
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
        .zoom(true)
        .build()?;

    // Display per-region results.
    println!("── Region Data Join Results ──");
    for region in &chart.regions {
        let id = region.id.as_deref().unwrap_or("(no id)");
        let value_str = region
            .value
            .map(|v| format!("{v:.1}M"))
            .unwrap_or_else(|| "no data".to_string());
        let [r, g, b, a] = region.color;
        println!("  {id:>4}: {value_str:>10}  → rgba({r:.3}, {g:.3}, {b:.3}, {a:.3})");
    }

    // Summary.
    let matched = chart.regions.iter().filter(|r| r.value.is_some()).count();
    let unmatched = chart.regions.len() - matched;
    println!("\n── Summary ──");
    println!("  Regions with data:    {matched}");
    println!("  Regions without data: {unmatched}");
    println!(
        "  Domain: {:.1}M – {:.1}M",
        chart.domain_min, chart.domain_max
    );
    println!(
        "  Fill triangles:  {} vertices, {} indices",
        chart.fill_vertices.len(),
        chart.fill_indices.len()
    );
    println!(
        "  Stroke segments: {} vertices ({} line segments)",
        chart.stroke_vertices.len(),
        chart.stroke_vertices.len() / 2
    );
    println!(
        "  Legend: {} (position: {:?})",
        if chart.show_legend { "shown" } else { "hidden" },
        chart.legend_position
    );
    println!("  Projection: {:?}", chart.projection);
    println!(
        "  Zoom/Pan: {}",
        if chart.zoom_enabled {
            "enabled"
        } else {
            "disabled"
        }
    );

    println!("\n=== Example Complete ===");
    Ok(())
}

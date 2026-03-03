// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! # World Map — GeoPathMark Example (GUP-274)
//!
//! Demonstrates `GeoPathMark` rendering country outlines from a bundled
//! low-resolution GeoJSON file (derived from Natural Earth 110m).
//!
//! The example:
//! - Parses a GeoJSON `FeatureCollection` into a `GeoJsonSource`
//! - Creates a `GeoPathMark` with Mercator projection
//! - Tessellates all country polygons via ear-clipping
//! - Reports triangle counts at different simplification tolerances
//!
//! ## Visual Validation
//!
//! When run with a GPU-enabled window, the tessellated country outlines
//! should form a recognisable world map with Mercator projection. Stroke
//! outlines trace the original boundary rings.
//!
//! Run with: `cargo run --example geo_world_map`

use gup::error::GupResult;
use gup::mark::geo_path::{GeoJsonSource, GeoPathMark, Projection};

fn main() -> GupResult<()> {
    println!("=== World Map — GeoPathMark Example (GUP-274) ===\n");

    // Load the bundled GeoJSON.
    let geojson_str = include_str!("../assets/ne_110m_countries.geojson");
    let source = GeoJsonSource::from_str(geojson_str)?;

    println!(
        "Parsed {} features from ne_110m_countries.geojson",
        source.features.len()
    );

    // List parsed features.
    for feat in &source.features {
        let name = feat
            .properties
            .as_ref()
            .and_then(|p| p.get("name"))
            .and_then(|n| n.as_str())
            .unwrap_or("(unnamed)");
        let poly_count = feat.polygons.len();
        let coord_count: usize = feat.polygons.iter().map(|p| p.exterior.len()).sum();
        println!("  {name}: {poly_count} polygon(s), {coord_count} exterior coords");
    }

    // Build the GeoPathMark with Mercator projection.
    let mark = GeoPathMark::new(source.clone(), Projection::Mercator)
        .fill_color(Some([0.82, 0.87, 0.92, 1.0]))
        .stroke_color(Some([0.25, 0.25, 0.25, 1.0]))
        .stroke_width(1.0);

    // Full-resolution tessellation.
    let full_triangles = mark.triangle_count(0.0);
    println!("\n── Tessellation Results ──");
    println!("  Full resolution: {full_triangles} triangles");

    // Simplified tessellation.
    let simplified_mark =
        GeoPathMark::new(source.clone(), Projection::Mercator).simplification_tolerance(0.5);
    let simplified_triangles = simplified_mark.triangle_count(0.5);
    let reduction = if full_triangles > 0 {
        ((full_triangles - simplified_triangles) as f64 / full_triangles as f64) * 100.0
    } else {
        0.0
    };
    println!(
        "  Simplified (tolerance=0.5°): {simplified_triangles} triangles ({reduction:.1}% reduction)"
    );

    // Tessellate the full mark to verify vertex generation.
    let (fill_verts, fill_indices, stroke_verts) = mark.tessellate()?;
    println!("\n── GPU Buffer Sizes ──");
    println!(
        "  Fill vertices: {} ({} bytes)",
        fill_verts.len(),
        fill_verts.len() * std::mem::size_of::<gup::mark::GeoPathVertex>()
    );
    println!("  Fill indices: {}", fill_indices.len());
    println!(
        "  Stroke vertices: {} ({} line segments)",
        stroke_verts.len(),
        stroke_verts.len() / 2
    );

    // Demonstrate equirectangular projection variant.
    let eq_mark = GeoPathMark::new(source, Projection::Equirectangular)
        .fill_color(Some([0.9, 0.85, 0.8, 1.0]));
    let eq_triangles = eq_mark.triangle_count(0.0);
    println!("\n── Equirectangular Projection ──");
    println!("  Triangles: {eq_triangles}");

    println!("\n=== Example Complete ===");
    Ok(())
}

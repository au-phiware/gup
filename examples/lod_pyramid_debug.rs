// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Debug visualisation of LOD pyramid levels.
//!
//! Builds a 5-level LOD pyramid from a synthetic dataset and prints a summary
//! of each level (point count, cell size) to stdout. This serves as a quick
//! visual validation that the pyramid construction is working correctly.
//!
//! Run with:
//!
//! ```sh
//! cargo run --example lod_pyramid_debug
//! ```

use gup::lod::{LodPyramidBuilder, VertexData, select_lod_level};
use gup::mark::batch_renderer::Viewport2D;
use gup::render::RenderContext;

fn main() {
    env_logger::init();

    let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
    rt.block_on(async {
        let context = RenderContext::new().await.expect("Failed to create render context");

        println!("=== LOD Pyramid Debug ===\n");

        // Generate synthetic data — 100K points in a unit square.
        let n = 100_000;
        let data: Vec<VertexData> = (0..n)
            .map(|i| {
                let x = (i as f32 * 0.618_034) % 1.0;
                let y = (i as f32 * 0.414_214) % 1.0;
                VertexData::new(x, y)
            })
            .collect();

        println!("Source data: {} points\n", data.len());

        // Build a 5-level pyramid.
        let pyramid = LodPyramidBuilder::new()
            .levels(5)
            .build_cpu(context.device(), context.queue(), &data)
            .expect("Failed to build LOD pyramid");

        println!("Pyramid levels: {}", pyramid.level_count());
        println!(
            "Total GPU bytes: {} ({:.2} MB)\n",
            pyramid.allocated_bytes(),
            pyramid.allocated_bytes() as f64 / (1024.0 * 1024.0)
        );

        for i in 0..pyramid.level_count() {
            let meta = pyramid.metadata(i);
            println!(
                "  Level {}: {:>8} points | cell_size: {:.4} | bounds: [{:.2}, {:.2}, {:.2}, {:.2}]",
                i,
                meta.point_count,
                meta.cell_size,
                meta.bounds[0],
                meta.bounds[1],
                meta.bounds[2],
                meta.bounds[3],
            );
        }

        // Demonstrate level selection for different viewports.
        println!("\n=== Level Selection ===\n");

        let viewports = [
            ("1920×1080 desktop", Viewport2D { pixel_width: 1920.0, pixel_height: 1080.0, ..Default::default() }),
            ("3840×2160 4K", Viewport2D { pixel_width: 3840.0, pixel_height: 2160.0, ..Default::default() }),
            ("320×240 thumbnail", Viewport2D { pixel_width: 320.0, pixel_height: 240.0, ..Default::default() }),
        ];

        for (name, vp) in &viewports {
            let level = select_lod_level(vp, n as u64, pyramid.level_count());
            println!(
                "  {:<20} → level {} ({} points)",
                name,
                level,
                pyramid.level_point_count(level)
            );
        }

        println!("\nDone.");
    });
}

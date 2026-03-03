// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Adaptive LOD debug example.
//!
//! Demonstrates the [`AdaptiveRenderer`] with a zoomable scatter plot and
//! an optional debug overlay. Press **D** to toggle the overlay, which shows
//! the current LOD tier, visible point count, and blend state.
//!
//! Mouse wheel zooms, and the zoom level drives LOD tier selection.
//!
//! Run with:
//!
//! ```sh
//! cargo run --example adaptive_lod_debug
//! ```

use gup::lod::{LodPyramidBuilder, VertexData};
use gup::render::RenderContext;
use gup::renderer::{AdaptiveRenderer, AdaptiveRendererConfig, AdaptiveViewport, ViewportCuller};

fn main() {
    env_logger::init();

    let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
    rt.block_on(async {
        let context = RenderContext::new()
            .await
            .expect("Failed to create render context");

        println!("=== Adaptive LOD Debug ===\n");
        println!("Controls:");
        println!("  Mouse wheel  : zoom in/out");
        println!("  D            : toggle debug overlay");
        println!("  Q / Esc      : quit");
        println!();

        // Generate synthetic data — 500K points in a 100×100 space.
        let n = 500_000;
        let data: Vec<VertexData> = (0..n)
            .map(|i| {
                let x = (i as f32 * 61.803_4) % 100.0;
                let y = (i as f32 * 41.421_4) % 100.0;
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
        for i in 0..pyramid.level_count() {
            let meta = pyramid.metadata(i);
            println!(
                "  Level {}: {:>8} points (cell_size: {:.4})",
                i, meta.point_count, meta.cell_size
            );
        }
        println!();

        // Create the adaptive renderer.
        let config = AdaptiveRendererConfig {
            blend_frames: 8,
            heuristic_scale: 1.0,
        };
        let mut renderer = AdaptiveRenderer::new(&pyramid, config);
        renderer.set_debug_overlay(true);

        // Create the viewport culler.
        let culler = ViewportCuller::new(context.device()).expect("Failed to create culler");

        // Simulate a zoom gesture (no winit window — console-based demo).
        let zoom_levels = [0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0, 500.0];

        println!("=== Zoom Simulation ===\n");
        for &zoom in &zoom_levels {
            let vp = AdaptiveViewport::new(zoom, [50.0, 50.0], [1920, 1080]);
            let frame = renderer.update(&vp);

            // Run GPU culling on the selected tier.
            let tier = frame.tier;
            let count = pyramid.level_point_count(tier) as u32;
            let bounds = vp.world_bounds();

            let result = culler
                .dispatch(
                    context.device(),
                    context.queue(),
                    pyramid.buffer(tier).buffer(),
                    count,
                    1,
                    [bounds[0], bounds[2], bounds[1], bounds[3]],
                )
                .await
                .expect("Culling failed");

            let indirect = culler
                .read_draw_indirect(context.device(), context.queue(), &result)
                .await
                .expect("Readback failed");

            renderer.set_visible_count(indirect[1]);

            let info = renderer
                .debug_overlay()
                .display_string()
                .unwrap_or_default();
            println!(
                "  zoom={:<6.1} | tier {} | {} / {} visible | blend_alpha={:.2} | {info}",
                zoom, frame.tier, indirect[1], count, frame.alpha,
            );
        }

        println!("\n=== Blend Transition Demo ===\n");

        // Simulate a rapid zoom change to show blend transition.
        let vp_out = AdaptiveViewport::new(1.0, [50.0, 50.0], [200, 200]);
        let frame = renderer.update(&vp_out);
        println!("  Start at zoom=1.0, tier={}", frame.tier);

        let vp_in = AdaptiveViewport::new(100.0, [50.0, 50.0], [1920, 1080]);
        for i in 0..12 {
            let frame = renderer.update(&vp_in);
            let from = frame
                .blend_from_tier
                .map(|t| format!("from tier {t}"))
                .unwrap_or_default();
            println!(
                "  Frame {:>2}: tier={} alpha={:.3} {}",
                i, frame.tier, frame.alpha, from,
            );
        }

        println!("\nDone.");
    });
}

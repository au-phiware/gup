// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Integration tests for the adaptive viewport renderer.
//!
//! Exercises the full `AdaptiveRenderer` path: building a small LOD pyramid,
//! running tier selection across different viewport configurations, and
//! verifying blend transitions.

use gup::lod::{LodPyramidBuilder, VertexData};
use gup::renderer::{AdaptiveRenderer, AdaptiveRendererConfig, AdaptiveViewport};
use gup::test_utils::create_test_context;

/// Generate a synthetic dataset of `n` points in a 100×100 data space.
fn synthetic_data(n: usize) -> Vec<VertexData> {
    (0..n)
        .map(|i| {
            let x = (i as f32 * 61.803_4) % 100.0;
            let y = (i as f32 * 41.421_4) % 100.0;
            VertexData::new(x, y)
        })
        .collect()
}

#[tokio::test]
async fn adaptive_renderer_with_real_pyramid() {
    let guard = create_test_context().await.unwrap();
    let ctx = guard.context();

    let data = synthetic_data(100_000);
    let pyramid = LodPyramidBuilder::new()
        .levels(5)
        .build_cpu(ctx.device(), ctx.queue(), &data)
        .unwrap();

    let config = AdaptiveRendererConfig {
        blend_frames: 8,
        heuristic_scale: 1.0,
    };
    let mut renderer = AdaptiveRenderer::new(&pyramid, config);

    // Verify we can select tiers at different zoom levels.
    let zoom_out = AdaptiveViewport::new(1.0, [50.0, 50.0], [200, 200]);
    let zoom_mid = AdaptiveViewport::new(5.0, [50.0, 50.0], [1920, 1080]);
    let zoom_in = AdaptiveViewport::new(500.0, [50.0, 50.0], [1920, 1080]);

    let tier_out = renderer.select_tier(&zoom_out);
    let tier_mid = renderer.select_tier(&zoom_mid);
    let tier_in = renderer.select_tier(&zoom_in);

    // Zoomed out should select a coarser tier than zoomed in.
    assert!(
        tier_out >= tier_mid,
        "Zoom-out tier ({tier_out}) should be ≥ zoom-mid tier ({tier_mid})"
    );
    assert!(
        tier_mid >= tier_in,
        "Zoom-mid tier ({tier_mid}) should be ≥ zoom-in tier ({tier_in})"
    );

    // Update with different viewports to trigger blending.
    let frame1 = renderer.update(&zoom_out);
    assert!(frame1.tier < pyramid.level_count());

    let frame2 = renderer.update(&zoom_in);
    // If tier changed, verify transition started.
    if frame2.tier != frame1.tier {
        assert!(
            frame2.alpha < 1.0 || frame2.blend_from_tier.is_some(),
            "Tier change should trigger a blend transition"
        );
    }
}

#[tokio::test]
async fn tier_selection_monotonic_with_zoom() {
    let guard = create_test_context().await.unwrap();
    let ctx = guard.context();

    let data = synthetic_data(50_000);
    let pyramid = LodPyramidBuilder::new()
        .levels(5)
        .build_cpu(ctx.device(), ctx.queue(), &data)
        .unwrap();

    let renderer = AdaptiveRenderer::new(&pyramid, AdaptiveRendererConfig::default());

    // As zoom increases (more detail), tier should decrease (finer) or stay same.
    let mut prev_tier = usize::MAX;
    for zoom in [0.5, 1.0, 5.0, 10.0, 50.0, 100.0, 500.0, 1000.0] {
        let vp = AdaptiveViewport::new(zoom, [50.0, 50.0], [1920, 1080]);
        let tier = renderer.select_tier(&vp);
        assert!(
            tier <= prev_tier,
            "Tier should decrease (finer) with increasing zoom: \
             zoom={zoom}, tier={tier}, prev_tier={prev_tier}"
        );
        prev_tier = tier;
    }
}

#[tokio::test]
async fn blend_transition_completes() {
    let guard = create_test_context().await.unwrap();
    let ctx = guard.context();

    let data = synthetic_data(100_000);
    let pyramid = LodPyramidBuilder::new()
        .levels(5)
        .build_cpu(ctx.device(), ctx.queue(), &data)
        .unwrap();

    let config = AdaptiveRendererConfig {
        blend_frames: 4,
        heuristic_scale: 1.0,
    };
    let mut renderer = AdaptiveRenderer::new(&pyramid, config);

    // Start at one zoom level.
    let vp1 = AdaptiveViewport::new(1.0, [50.0, 50.0], [200, 200]);
    renderer.update(&vp1);
    let initial_tier = renderer.selected_tier();

    // Switch to a very different zoom to trigger a tier change.
    let vp2 = AdaptiveViewport::new(500.0, [50.0, 50.0], [1920, 1080]);
    let frame = renderer.update(&vp2);

    if frame.tier != initial_tier {
        // Tick through the transition.
        let mut settled = false;
        for _ in 0..10 {
            let f = renderer.update(&vp2);
            if f.alpha >= 1.0 && f.blend_from_tier.is_none() {
                settled = true;
                break;
            }
        }
        assert!(settled, "Blend transition should complete within 10 frames");
    }
}

#[tokio::test]
async fn debug_overlay_collects_frame_info() {
    let guard = create_test_context().await.unwrap();
    let ctx = guard.context();

    let data = synthetic_data(10_000);
    let pyramid = LodPyramidBuilder::new()
        .levels(4)
        .build_cpu(ctx.device(), ctx.queue(), &data)
        .unwrap();

    let mut renderer = AdaptiveRenderer::new(&pyramid, AdaptiveRendererConfig::default());
    renderer.set_debug_overlay(true);

    let vp = AdaptiveViewport::new(10.0, [50.0, 50.0], [1920, 1080]);
    renderer.update(&vp);

    let info = renderer.debug_overlay().info().unwrap();
    assert_eq!(info.tier_count, pyramid.level_count());
    assert!(info.total_points_in_tier > 0);

    // Test display string.
    let display = renderer.debug_overlay().display_string().unwrap();
    assert!(display.contains("LOD"), "Display should contain 'LOD': {display}");
}

#[tokio::test]
async fn viewport_2d_conversion_matches_adaptive_viewport() {
    let guard = create_test_context().await.unwrap();
    let ctx = guard.context();

    let data = synthetic_data(1_000);
    let pyramid = LodPyramidBuilder::new()
        .levels(3)
        .build_cpu(ctx.device(), ctx.queue(), &data)
        .unwrap();

    let mut renderer = AdaptiveRenderer::new(&pyramid, AdaptiveRendererConfig::default());
    let vp = AdaptiveViewport::new(100.0, [50.0, 50.0], [800, 600]);
    renderer.update(&vp);

    let v2d = renderer.viewport_2d();
    assert!((v2d.pixel_width - 800.0).abs() < f32::EPSILON);
    assert!((v2d.pixel_height - 600.0).abs() < f32::EPSILON);

    // Bounds should be within the data extent.
    let bounds = vp.world_bounds();
    assert!((v2d.min_x - bounds[0]).abs() < 1e-5);
    assert!((v2d.max_x - bounds[2]).abs() < 1e-5);
}

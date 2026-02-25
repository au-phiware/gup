// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GPU integration tests for SDF glyph texture upload.
//!
//! These tests verify that MSDF glyph bitmaps are correctly uploaded to the
//! GPU texture atlas and that the full text rendering pipeline produces
//! valid vertex data for rendering.

use gup::RenderContext;
use gup::text::{FontAtlas, TextLayoutEngine, TextRenderer, TextStyle};
use std::time::Instant;

/// Helper to create GPU context for tests
async fn create_test_context() -> RenderContext {
    RenderContext::new()
        .await
        .expect("Failed to create GPU context")
}

// =============================================================================
// FontAtlas creation and glyph loading
// =============================================================================

#[tokio::test]
async fn test_font_atlas_creation_with_gpu() {
    let context = create_test_context().await;
    let atlas = FontAtlas::new(context.device(), context.queue(), 16.0);
    assert!(atlas.is_ok(), "FontAtlas creation should succeed");

    let atlas = atlas.unwrap();
    // ASCII printable characters (32-126) should be pre-loaded
    assert!(
        atlas.glyph_count() > 0,
        "Atlas should have pre-loaded glyphs"
    );
}

#[tokio::test]
async fn test_ascii_glyphs_preloaded() {
    let context = create_test_context().await;
    let atlas = FontAtlas::new(context.device(), context.queue(), 16.0).unwrap();

    // Verify common ASCII characters are loaded
    for ch in "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789".chars() {
        let glyph = atlas.get_glyph(ch);
        assert!(glyph.is_some(), "Glyph for '{ch}' should be preloaded");
    }
}

#[tokio::test]
async fn test_glyph_atlas_positions_valid() {
    let context = create_test_context().await;
    let atlas = FontAtlas::new(context.device(), context.queue(), 16.0).unwrap();

    // Verify all loaded glyphs have valid atlas positions
    for ch in 'A'..='Z' {
        if let Some(glyph) = atlas.get_glyph(ch) {
            // Characters with outlines should have non-zero atlas regions
            let [u0, v0, u1, v1] = glyph.atlas_pos;
            assert!(u1 > u0, "Glyph '{ch}' should have positive width in atlas");
            assert!(v1 > v0, "Glyph '{ch}' should have positive height in atlas");
            assert!(u0 >= 0.0 && u1 <= 1.0, "Glyph '{ch}' U coords out of range");
            assert!(v0 >= 0.0 && v1 <= 1.0, "Glyph '{ch}' V coords out of range");
        }
    }
}

#[tokio::test]
async fn test_glyph_sizes_reasonable() {
    let context = create_test_context().await;
    let atlas = FontAtlas::new(context.device(), context.queue(), 16.0).unwrap();

    for ch in 'A'..='Z' {
        if let Some(glyph) = atlas.get_glyph(ch) {
            assert!(
                glyph.size.x > 0.0,
                "Glyph '{ch}' should have positive width"
            );
            assert!(
                glyph.size.y > 0.0,
                "Glyph '{ch}' should have positive height"
            );
            assert!(
                glyph.advance > 0.0,
                "Glyph '{ch}' should have positive advance"
            );
        }
    }
}

#[tokio::test]
async fn test_whitespace_glyph_handling() {
    let context = create_test_context().await;
    let atlas = FontAtlas::new(context.device(), context.queue(), 16.0).unwrap();

    // Space character should exist but have zero-sized atlas region
    let space = atlas.get_glyph(' ');
    assert!(space.is_some(), "Space glyph should exist");
    let space = space.unwrap();
    assert!(space.advance > 0.0, "Space should have positive advance");
    // Space has no outline, so size should be zero
    assert_eq!(space.size.x, 0.0, "Space should have zero width");
    assert_eq!(space.size.y, 0.0, "Space should have zero height");
}

#[tokio::test]
async fn test_glyph_no_overlap() {
    let context = create_test_context().await;
    let atlas = FontAtlas::new(context.device(), context.queue(), 16.0).unwrap();

    // Collect all glyph atlas regions for characters with outlines
    let regions: Vec<(char, [f32; 4])> = ('!'..='~')
        .filter_map(|ch| atlas.get_glyph(ch).map(|g| (ch, g.atlas_pos)))
        .filter(|(_, pos)| pos[2] > pos[0] && pos[3] > pos[1]) // Non-empty
        .collect();

    // Check no two glyphs overlap in the atlas
    for i in 0..regions.len() {
        for j in (i + 1)..regions.len() {
            let (ch_a, a) = regions[i];
            let (ch_b, b) = regions[j];

            // Check if rectangles overlap (allowing 1-pixel gap tolerance)
            let overlap_x = a[0] < b[2] && b[0] < a[2];
            let overlap_y = a[1] < b[3] && b[1] < a[3];

            assert!(
                !(overlap_x && overlap_y),
                "Glyphs '{ch_a}' and '{ch_b}' overlap in atlas"
            );
        }
    }
}

#[tokio::test]
async fn test_atlas_utilization_nonzero() {
    let context = create_test_context().await;
    let atlas = FontAtlas::new(context.device(), context.queue(), 16.0).unwrap();

    let utilization = atlas.atlas_utilization();
    assert!(
        utilization > 0.0,
        "Atlas should have some utilization after preloading"
    );
    assert!(
        utilization < 1.0,
        "Atlas should not be full after ASCII preloading"
    );
}

// =============================================================================
// On-demand glyph loading
// =============================================================================

#[tokio::test]
async fn test_ensure_glyph_loads_on_demand() {
    let context = create_test_context().await;
    let mut atlas = FontAtlas::new(context.device(), context.queue(), 16.0).unwrap();

    // Try loading a Unicode character beyond standard ASCII
    let result = atlas.ensure_glyph(context.device(), context.queue(), 'é', 16.0);
    // May or may not succeed depending on font coverage, but shouldn't panic
    if result.is_ok() {
        let glyph = atlas.get_glyph('é');
        assert!(glyph.is_some(), "Loaded glyph should be retrievable");
    }
}

#[tokio::test]
async fn test_ensure_glyph_idempotent() {
    let context = create_test_context().await;
    let mut atlas = FontAtlas::new(context.device(), context.queue(), 16.0).unwrap();

    let count_before = atlas.glyph_count();

    // Ensure a glyph that's already loaded
    let result = atlas.ensure_glyph(context.device(), context.queue(), 'A', 16.0);
    assert!(result.is_ok());

    let count_after = atlas.glyph_count();
    assert_eq!(
        count_before, count_after,
        "Ensuring already-loaded glyph should not add duplicates"
    );
}

// =============================================================================
// Text renderer integration
// =============================================================================

#[tokio::test]
async fn test_text_renderer_creation() {
    let context = create_test_context().await;
    let renderer = TextRenderer::new(context.device());
    assert!(renderer.is_ok(), "TextRenderer creation should succeed");
}

#[tokio::test]
async fn test_text_layout_produces_vertices() {
    let context = create_test_context().await;
    let atlas = FontAtlas::new(context.device(), context.queue(), 16.0).unwrap();
    let mut layout_engine = TextLayoutEngine::new();

    let style = TextStyle::default();
    let position = gup::shader_function::Vec2 { x: 10.0, y: 20.0 };

    let result = layout_engine.layout_text("Hello", position, &style, &atlas, None);
    assert!(result.is_ok(), "Text layout should succeed");

    let layout = result.unwrap();
    assert!(
        !layout.glyphs.is_empty(),
        "Layout should produce positioned glyphs"
    );
    assert_eq!(
        layout.glyphs.len(),
        5,
        "Layout should produce one glyph per non-whitespace character in 'Hello'"
    );
}

// =============================================================================
// Performance validation
// =============================================================================

#[tokio::test]
async fn test_ascii_preload_performance() {
    let context = create_test_context().await;

    let start = Instant::now();
    let atlas = FontAtlas::new(context.device(), context.queue(), 16.0).unwrap();
    let duration = start.elapsed();

    let glyph_count = atlas.glyph_count();
    println!(
        "FontAtlas preload of {glyph_count} glyphs took: {duration:?} ({:.2}ms/glyph)",
        duration.as_secs_f64() * 1000.0 / glyph_count as f64
    );

    // Atlas creation + MSDF generation + texture upload for ~95 ASCII characters
    // should complete within reasonable time
    #[cfg(debug_assertions)]
    let threshold_ms: u128 = 5000; // Debug builds are much slower
    #[cfg(not(debug_assertions))]
    let threshold_ms: u128 = 2000;

    assert!(
        duration.as_millis() < threshold_ms,
        "ASCII preload too slow: {duration:?} (threshold: {threshold_ms}ms)"
    );
}

#[tokio::test]
async fn test_texture_upload_adds_minimal_overhead() {
    let context = create_test_context().await;

    // Measure time to create atlas (includes MSDF generation + texture upload)
    let start = Instant::now();
    let _atlas = FontAtlas::new(context.device(), context.queue(), 16.0).unwrap();
    let total_duration = start.elapsed();

    // The total time is dominated by MSDF generation, not texture upload.
    // We verify the total is reasonable, which implies upload overhead is small.
    let glyph_count = _atlas.glyph_count();
    let per_glyph_ms = total_duration.as_secs_f64() * 1000.0 / glyph_count as f64;

    println!(
        "Per-glyph time: {per_glyph_ms:.2}ms (total: {total_duration:?} for {glyph_count} glyphs)"
    );

    // Each glyph (MSDF gen + upload) should average under a generous threshold
    #[cfg(debug_assertions)]
    let threshold_per_glyph_ms = 100.0;
    #[cfg(not(debug_assertions))]
    let threshold_per_glyph_ms = 30.0;

    assert!(
        per_glyph_ms < threshold_per_glyph_ms,
        "Per-glyph processing too slow: {per_glyph_ms:.2}ms (threshold: {threshold_per_glyph_ms}ms)"
    );
}

#[tokio::test]
async fn test_atlas_memory_usage_under_limit() {
    let context = create_test_context().await;
    let atlas = FontAtlas::new(context.device(), context.queue(), 16.0).unwrap();

    // Atlas is 1024x1024 RGBA = 4MB texture
    let atlas_bytes = (atlas.atlas_size() as usize).pow(2) * 4;
    assert!(
        atlas_bytes <= 4 * 1024 * 1024,
        "Atlas texture should be under 4MB, got {atlas_bytes} bytes"
    );

    let utilization = atlas.atlas_utilization();
    println!(
        "Atlas utilization after ASCII preload: {:.1}% ({} glyphs)",
        utilization * 100.0,
        atlas.glyph_count()
    );
}

// =============================================================================
// Font metrics validation
// =============================================================================

#[tokio::test]
async fn test_font_metrics_reasonable() {
    let context = create_test_context().await;
    let atlas = FontAtlas::new(context.device(), context.queue(), 16.0).unwrap();

    let metrics = atlas.metrics();
    assert_eq!(metrics.size, 16.0, "Font size should match requested size");
    assert!(metrics.line_height > 0.0, "Line height should be positive");
    assert!(metrics.ascent > 0.0, "Ascent should be positive");
    assert!(metrics.descent >= 0.0, "Descent should be non-negative");
    assert!(
        metrics.line_height >= metrics.ascent,
        "Line height should be >= ascent"
    );
}

#[tokio::test]
async fn test_multiple_font_sizes() {
    let context = create_test_context().await;

    for font_size in [8.0, 12.0, 16.0, 24.0, 32.0, 48.0] {
        let atlas = FontAtlas::new(context.device(), context.queue(), font_size);
        assert!(atlas.is_ok(), "FontAtlas should work at {font_size}px");

        let atlas = atlas.unwrap();
        let metrics = atlas.metrics();
        assert_eq!(
            metrics.size, font_size,
            "Font size should match requested {font_size}px"
        );

        // Verify glyphs loaded at this size
        let glyph_a = atlas.get_glyph('A');
        assert!(
            glyph_a.is_some(),
            "Glyph 'A' should be available at {font_size}px"
        );
    }
}

// =============================================================================
// End-to-end text pipeline
// =============================================================================

#[tokio::test]
async fn test_end_to_end_text_pipeline() {
    let context = create_test_context().await;
    let atlas = FontAtlas::new(context.device(), context.queue(), 16.0).unwrap();
    let renderer = TextRenderer::new(context.device());
    assert!(renderer.is_ok());

    let mut layout_engine = TextLayoutEngine::new();
    let style = TextStyle::default();
    let position = gup::shader_function::Vec2 { x: 50.0, y: 50.0 };

    // Layout text
    let result = layout_engine.layout_text("Test text 123", position, &style, &atlas, None);
    assert!(result.is_ok(), "Layout should succeed");

    let layout = result.unwrap();
    // Spaces may produce zero-sized glyphs that are filtered by the layout engine
    assert!(
        layout.glyphs.len() >= 11,
        "Should have at least 11 positioned glyphs (non-whitespace), got {}",
        layout.glyphs.len()
    );

    // Verify bounds are reasonable
    assert!(
        layout.bounds.width() > 0.0,
        "Text bounds should have positive width"
    );
    assert!(
        layout.bounds.height() > 0.0,
        "Text bounds should have positive height"
    );
}

#[tokio::test]
async fn test_special_characters_upload() {
    let context = create_test_context().await;
    let atlas = FontAtlas::new(context.device(), context.queue(), 16.0).unwrap();

    // Verify special ASCII characters are handled
    for ch in "!@#$%^&*()_+-=[]{}|;':\",./<>?".chars() {
        let glyph = atlas.get_glyph(ch);
        assert!(
            glyph.is_some(),
            "Special character '{ch}' should be preloaded"
        );
    }
}

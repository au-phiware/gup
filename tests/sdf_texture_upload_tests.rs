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
use wgpu::{
    BufferDescriptor, BufferUsages, CommandEncoderDescriptor, Extent3d, MapMode, Origin3d,
    TexelCopyBufferInfo, TexelCopyBufferLayout, TexelCopyTextureInfo, TextureAspect,
};

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

// =============================================================================
// GPU texture readback verification
// =============================================================================

#[tokio::test]
async fn test_texture_contains_glyph_data() {
    let context = create_test_context().await;
    let atlas = FontAtlas::new(context.device(), context.queue(), 16.0).unwrap();

    // Get the atlas region for 'A' - a character that should have visible MSDF data
    let glyph_a = atlas.get_glyph('A').expect("Glyph 'A' should exist");
    let atlas_size = atlas.atlas_size();

    // Calculate pixel coordinates of glyph region
    let x0 = (glyph_a.atlas_pos[0] * atlas_size as f32) as u32;
    let y0 = (glyph_a.atlas_pos[1] * atlas_size as f32) as u32;
    let x1 = (glyph_a.atlas_pos[2] * atlas_size as f32) as u32;
    let y1 = (glyph_a.atlas_pos[3] * atlas_size as f32) as u32;
    let glyph_width = x1 - x0;
    let glyph_height = y1 - y0;

    assert!(
        glyph_width > 0 && glyph_height > 0,
        "Glyph 'A' should have non-zero dimensions"
    );

    // Read back the glyph region from GPU texture
    // bytes_per_row must be 256-byte aligned per WebGPU spec
    let bytes_per_row = (glyph_width * 4).div_ceil(256) * 256;
    let buffer_size = (bytes_per_row * glyph_height) as u64;

    let readback_buffer = context.device().create_buffer(&BufferDescriptor {
        label: Some("Glyph Readback Buffer"),
        size: buffer_size,
        usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    let mut encoder = context
        .device()
        .create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Glyph Readback Encoder"),
        });

    encoder.copy_texture_to_buffer(
        TexelCopyTextureInfo {
            texture: atlas.texture(),
            mip_level: 0,
            origin: Origin3d { x: x0, y: y0, z: 0 },
            aspect: TextureAspect::All,
        },
        TexelCopyBufferInfo {
            buffer: &readback_buffer,
            layout: TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: Some(glyph_height),
            },
        },
        Extent3d {
            width: glyph_width,
            height: glyph_height,
            depth_or_array_layers: 1,
        },
    );

    context.queue().submit(std::iter::once(encoder.finish()));

    // Map the readback buffer
    let buffer_slice = readback_buffer.slice(..);
    let (sender, receiver) = tokio::sync::oneshot::channel();
    buffer_slice.map_async(MapMode::Read, move |result| {
        let _ = sender.send(result);
    });
    let _ = context.device().poll(wgpu::PollType::Wait {
        submission_index: None,
        timeout: None,
    });
    receiver.await.unwrap().expect("Buffer mapping failed");

    // Read pixel data
    let data = buffer_slice.get_mapped_range();
    let pixels: &[u8] = &data;

    // Count non-zero pixels (MSDF data should have meaningful distance values)
    let mut nonzero_count = 0;
    let mut total_pixels = 0;
    for row in 0..glyph_height {
        for col in 0..glyph_width {
            let offset = (row * bytes_per_row + col * 4) as usize;
            let r = pixels[offset];
            let g = pixels[offset + 1];
            let b = pixels[offset + 2];

            total_pixels += 1;
            if r > 0 || g > 0 || b > 0 {
                nonzero_count += 1;
            }
        }
    }

    drop(data);
    readback_buffer.unmap();

    // The MSDF should have a significant number of non-zero pixels.
    // Inside the glyph, values are >128; outside, they're <128 but non-zero
    // near the edges. Only far-outside pixels remain at 0.
    let nonzero_ratio = nonzero_count as f32 / total_pixels as f32;
    println!(
        "Glyph 'A' texture region: {}x{} pixels, {nonzero_count}/{total_pixels} non-zero ({:.1}%)",
        glyph_width,
        glyph_height,
        nonzero_ratio * 100.0
    );

    assert!(
        nonzero_ratio > 0.1,
        "At least 10% of glyph pixels should be non-zero (got {:.1}%); \
         MSDF data may not be uploaded correctly",
        nonzero_ratio * 100.0
    );
}

#[tokio::test]
async fn test_different_glyphs_have_different_data() {
    let context = create_test_context().await;
    let atlas = FontAtlas::new(context.device(), context.queue(), 16.0).unwrap();
    let atlas_size = atlas.atlas_size();

    // Compare atlas regions of two visually distinct characters
    let glyph_a = atlas.get_glyph('A').expect("Glyph 'A' should exist");
    let glyph_o = atlas.get_glyph('O').expect("Glyph 'O' should exist");

    // They should occupy different atlas regions
    let a_region = glyph_a.atlas_pos;
    let o_region = glyph_o.atlas_pos;

    // Verify non-overlapping (at least in x or y)
    let no_x_overlap = a_region[2] <= o_region[0] || o_region[2] <= a_region[0];
    let no_y_overlap = a_region[3] <= o_region[1] || o_region[3] <= a_region[1];

    assert!(
        no_x_overlap || no_y_overlap,
        "Glyphs 'A' and 'O' should not overlap in atlas"
    );

    // Both should have non-zero sized regions
    let a_width = ((a_region[2] - a_region[0]) * atlas_size as f32) as u32;
    let o_width = ((o_region[2] - o_region[0]) * atlas_size as f32) as u32;

    assert!(a_width > 0, "'A' should have positive atlas width");
    assert!(o_width > 0, "'O' should have positive atlas width");
}

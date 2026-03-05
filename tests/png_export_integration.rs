// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Integration tests for the PNG export pipeline.
//!
//! These tests exercise the full GPU render-to-PNG path: off-screen texture
//! creation, rendering, staging-buffer readback, row-padding stripping, and
//! PNG encoding via the `image` crate.

use gup::chart_builder::{ChartConfig, ComposedChart, Margins, TitleConfig};
use gup::error::GupResult;
use gup::export::png;
use gup::prelude::*;
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct TestPoint {
    x: f32,
    y: f32,
}

fn sample_data() -> Vec<TestPoint> {
    vec![
        TestPoint { x: 1.0, y: 10.0 },
        TestPoint { x: 2.0, y: 20.0 },
        TestPoint { x: 3.0, y: 30.0 },
        TestPoint { x: 4.0, y: 40.0 },
        TestPoint { x: 5.0, y: 50.0 },
    ]
}

fn build_chart(
    data: Vec<TestPoint>,
    context: Arc<gup::RenderContext>,
) -> GupResult<ComposedChart<TestPoint, gup::Circle>> {
    let config = ChartConfig {
        title_config: Some(TitleConfig::new("Test Chart")),
        width: 800.0,
        height: 600.0,
        margins: Margins {
            top: 40.0,
            right: 20.0,
            bottom: 40.0,
            left: 40.0,
        },
        show_axes: true,
        show_grid: true,
        ..ChartConfig::default()
    }
    .with_x_scale(LinearScale::new(0.0, 6.0, -1.0, 1.0))
    .with_y_scale(LinearScale::new(0.0, 60.0, -1.0, 1.0));

    let selection = gup::selection::Selection::<TestPoint, gup::Circle>::new(data, context)?;
    Ok(ComposedChart::new(selection, config).with_default_axes())
}

// ---------------------------------------------------------------------------
// GPU integration tests
// ---------------------------------------------------------------------------

/// Test that render_to_png produces a valid PNG with correct dimensions.
#[tokio::test]
async fn test_render_to_png_produces_valid_png() {
    let context = Arc::new(gup::RenderContext::new().await.unwrap());
    let mut chart = build_chart(sample_data(), context).unwrap();

    let png_bytes = chart.render_to_png(800, 600).unwrap();

    // Verify PNG magic bytes.
    assert!(
        png_bytes.len() > 8,
        "PNG output too small: {} bytes",
        png_bytes.len()
    );
    assert_eq!(
        &png_bytes[..8],
        &[137, 80, 78, 71, 13, 10, 26, 10],
        "PNG magic bytes mismatch"
    );

    // Decode with the image crate and verify dimensions.
    let decoded = image::load_from_memory_with_format(&png_bytes, image::ImageFormat::Png).unwrap();
    assert_eq!(decoded.width(), 800);
    assert_eq!(decoded.height(), 600);
}

/// Test that render_to_png_scaled with factor 2.0 produces double-size output.
#[tokio::test]
async fn test_render_to_png_scaled_2x() {
    let context = Arc::new(gup::RenderContext::new().await.unwrap());
    let mut chart = build_chart(sample_data(), context).unwrap();

    let png_bytes = chart.render_to_png_scaled(400, 300, 2.0).unwrap();

    let decoded = image::load_from_memory_with_format(&png_bytes, image::ImageFormat::Png).unwrap();
    assert_eq!(decoded.width(), 800);
    assert_eq!(decoded.height(), 600);
}

/// Test that render_to_png_scaled with factor 1.0 matches render_to_png.
#[tokio::test]
async fn test_render_to_png_scaled_1x_matches_base() {
    let context = Arc::new(gup::RenderContext::new().await.unwrap());
    let mut chart = build_chart(sample_data(), context).unwrap();

    let base = chart.render_to_png(400, 300).unwrap();
    let scaled = chart.render_to_png_scaled(400, 300, 1.0).unwrap();

    // Both should produce the same dimensions.
    let dec_base = image::load_from_memory_with_format(&base, image::ImageFormat::Png).unwrap();
    let dec_scaled = image::load_from_memory_with_format(&scaled, image::ImageFormat::Png).unwrap();
    assert_eq!(dec_base.width(), dec_scaled.width());
    assert_eq!(dec_base.height(), dec_scaled.height());
}

/// Test export_png writes a file to disk.
#[tokio::test]
async fn test_export_png_writes_file() {
    let context = Arc::new(gup::RenderContext::new().await.unwrap());
    let mut chart = build_chart(sample_data(), context).unwrap();

    let path = std::env::temp_dir().join("gup_test_export.png");
    chart.export_png(&path, 640, 480).unwrap();

    assert!(path.exists(), "PNG file was not written to disk");
    let bytes = std::fs::read(&path).unwrap();
    assert_eq!(
        &bytes[..8],
        &[137, 80, 78, 71, 13, 10, 26, 10],
        "Written file is not valid PNG"
    );

    let decoded = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png).unwrap();
    assert_eq!(decoded.width(), 640);
    assert_eq!(decoded.height(), 480);

    // Clean up.
    let _ = std::fs::remove_file(&path);
}

/// Test that the output contains RGBA data (alpha channel preserved).
#[tokio::test]
async fn test_png_has_rgba_channels() {
    let context = Arc::new(gup::RenderContext::new().await.unwrap());
    let mut chart = build_chart(sample_data(), context).unwrap();

    let png_bytes = chart.render_to_png(200, 150).unwrap();

    let decoded = image::load_from_memory_with_format(&png_bytes, image::ImageFormat::Png).unwrap();
    let rgba = decoded.to_rgba8();
    assert_eq!(rgba.len(), 200 * 150 * 4, "RGBA buffer has wrong size");
}

/// Test rendering at a non-aligned width (triggers row-padding logic).
#[tokio::test]
async fn test_render_to_png_non_aligned_width() {
    let context = Arc::new(gup::RenderContext::new().await.unwrap());
    let mut chart = build_chart(sample_data(), context).unwrap();

    // 100 pixels wide → 400 bytes/row, padded to 512; exercises stripping.
    let png_bytes = chart.render_to_png(100, 75).unwrap();

    let decoded = image::load_from_memory_with_format(&png_bytes, image::ImageFormat::Png).unwrap();
    assert_eq!(decoded.width(), 100);
    assert_eq!(decoded.height(), 75);
}

// ---------------------------------------------------------------------------
// Pure function tests (non-GPU)
// ---------------------------------------------------------------------------

/// Test OffscreenTarget can be created and queried.
#[tokio::test]
async fn test_offscreen_target_dimensions() {
    let context = gup::RenderContext::new().await.unwrap();
    let target = png::OffscreenTarget::new(context.device(), 1024, 768);

    assert_eq!(target.width(), 1024);
    assert_eq!(target.height(), 768);
}

/// Test readback from a freshly cleared off-screen texture.
#[tokio::test]
async fn test_offscreen_readback_cleared_texture() {
    let context = gup::RenderContext::new().await.unwrap();
    let device = context.device();
    let queue = context.queue();

    let target = png::OffscreenTarget::new(device, 64, 64);

    // Clear the texture to red (BGRA format → B=0, G=0, R=255, A=255).
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("test_clear_encoder"),
    });

    {
        let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("test_clear_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target.view(),
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 1.0,
                        g: 0.0,
                        b: 0.0,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
    }

    queue.submit(std::iter::once(encoder.finish()));

    // Read back as PNG and verify.
    let png_bytes = target.readback_as_png(device, queue).unwrap();
    let decoded = image::load_from_memory_with_format(&png_bytes, image::ImageFormat::Png).unwrap();
    assert_eq!(decoded.width(), 64);
    assert_eq!(decoded.height(), 64);

    // Verify pixel content: should be red (RGBA).
    let rgba = decoded.to_rgba8();
    let first_pixel = &rgba.as_raw()[..4];
    // sRGB colour space means values won't be exactly [255, 0, 0, 255],
    // but R should be dominant and B/G should be very low.
    assert!(
        first_pixel[0] > 200,
        "Expected high red, got {}",
        first_pixel[0]
    );
    assert!(
        first_pixel[1] < 10,
        "Expected low green, got {}",
        first_pixel[1]
    );
    assert!(
        first_pixel[2] < 10,
        "Expected low blue, got {}",
        first_pixel[2]
    );
    assert_eq!(first_pixel[3], 255, "Expected full alpha");
}

// ---------------------------------------------------------------------------
// Data mark rendering tests (GUP-268A)
// ---------------------------------------------------------------------------

/// Test that data marks appear in exported PNG when the Selection is prepared.
#[tokio::test]
async fn test_data_marks_appear_in_export() {
    let context = Arc::new(gup::RenderContext::new().await.unwrap());

    let data = sample_data();
    let mut selection =
        gup::selection::Selection::<TestPoint, gup::Circle>::new(data.clone(), context.clone())
            .unwrap();

    // Prepare the selection with circle instances at known positions.
    // The chart uses x_scale [0,6] → [-1,1] and y_scale [0,60] → [-1,1].
    selection
        .prepare_render(
            context.device(),
            context.queue(),
            |d: &TestPoint| gup::mark::circle::CircleInstance {
                center: [
                    d.x / 6.0 * 2.0 - 1.0,  // map x to clip space
                    d.y / 60.0 * 2.0 - 1.0,  // map y to clip space
                ],
                radius: 0.05,
                _pad0: 0.0,
                fill_color: [1.0, 0.0, 0.0, 1.0], // Red circles
                stroke_width: 0.0,
                _pad1: [0.0; 3],
                stroke_color: [0.0, 0.0, 0.0, 0.0],
            },
            None,
            None,
        )
        .unwrap();

    let config = ChartConfig {
        width: 200.0,
        height: 200.0,
        margins: Margins {
            top: 0.0,
            right: 0.0,
            bottom: 0.0,
            left: 0.0,
        },
        background_color: Some([1.0, 1.0, 1.0, 1.0]),
        show_axes: false,
        show_grid: false,
        ..ChartConfig::default()
    };

    let mut chart = ComposedChart::new(selection, config);

    let rgba = chart.render_to_rgba(200, 200).unwrap();
    assert_eq!(rgba.len(), 200 * 200 * 4);

    // Count non-white pixels. With 5 red circles drawn on a white
    // background, there should be many non-white pixels.
    let non_white = rgba
        .chunks_exact(4)
        .filter(|px| px[0] != 255 || px[1] != 255 || px[2] != 255)
        .count();

    assert!(
        non_white > 50,
        "Expected data marks to produce non-white pixels, but only found {non_white}"
    );
}

/// Test that export without prepare_render still succeeds (no marks drawn).
#[tokio::test]
async fn test_export_without_prepare_renders_no_marks() {
    let context = Arc::new(gup::RenderContext::new().await.unwrap());

    let data = sample_data();
    let selection =
        gup::selection::Selection::<TestPoint, gup::Circle>::new(data, context.clone()).unwrap();

    let config = ChartConfig {
        width: 100.0,
        height: 100.0,
        margins: Margins {
            top: 0.0,
            right: 0.0,
            bottom: 0.0,
            left: 0.0,
        },
        background_color: Some([1.0, 1.0, 1.0, 1.0]),
        show_axes: false,
        show_grid: false,
        ..ChartConfig::default()
    };

    let mut chart = ComposedChart::new(selection, config);

    // Should succeed even without prepare_render — just no marks rendered.
    let rgba = chart.render_to_rgba(100, 100).unwrap();
    assert_eq!(rgba.len(), 100 * 100 * 4);

    // All pixels should be white (background clear colour).
    let all_white = rgba.chunks_exact(4).all(|px| px[0] == 255 && px[1] == 255 && px[2] == 255);
    assert!(all_white, "Expected all-white output when selection is not prepared");
}

// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Integration tests for tooltip background rendering.
//!
//! These tests verify that `TooltipBackgroundRenderer` can be created with a
//! real GPU context and renders correctly within a render pass.

use gup::GupContext;
use gup::shader_function::Vec2;
use gup::text::hover_reveal::{
    ActiveTooltip, ArrowDirection, TooltipConfig, TooltipLayout, compute_tooltip_layout,
};
use gup::text::tooltip_bg::TooltipBackgroundRenderer;
use gup::text::{FontAtlas, TextBounds, TextLayoutEngine, TextStyle};
use std::sync::Arc;

/// Helper: create a headless GPU context.
async fn create_context() -> Arc<GupContext> {
    GupContext::headless()
        .await
        .expect("Failed to create headless GPU context")
}

// =============================================================================
// Construction
// =============================================================================

#[tokio::test]
async fn test_create_tooltip_bg_renderer() {
    let ctx = create_context().await;
    let renderer = TooltipBackgroundRenderer::new(&ctx.device);
    assert!(renderer.is_ok(), "Should create renderer without error");
}

// =============================================================================
// Queue / begin_frame
// =============================================================================

#[tokio::test]
async fn test_queue_and_begin_frame() {
    let ctx = create_context().await;
    let mut renderer = TooltipBackgroundRenderer::new(&ctx.device).unwrap();

    let config = TooltipConfig::default();
    let layout = TooltipLayout {
        background_bounds: TextBounds::new(10.0, 20.0, 200.0, 60.0),
        text_position: Vec2 { x: 16.0, y: 24.0 },
        text: "Hello".to_string(),
        opacity: 1.0,
        arrow_direction: ArrowDirection::None,
        arrow_size: 0.0,
        arrow_offset: 0.0,
    };

    renderer.begin_frame();
    assert_eq!(renderer.queued_count(), 0);

    renderer.queue(&layout, &config);
    assert_eq!(renderer.queued_count(), 1);

    renderer.queue(&layout, &config);
    assert_eq!(renderer.queued_count(), 2);

    renderer.begin_frame();
    assert_eq!(renderer.queued_count(), 0, "begin_frame should clear queue");
}

// =============================================================================
// End-to-end: queue + render in a render pass
// =============================================================================

#[tokio::test]
async fn test_render_in_render_pass() {
    let ctx = create_context().await;

    // Create an offscreen render target
    let texture = ctx.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Test RT"),
        size: wgpu::Extent3d {
            width: 256,
            height: 256,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Bgra8UnormSrgb,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

    let mut renderer = TooltipBackgroundRenderer::new(&ctx.device).unwrap();

    let config = TooltipConfig {
        corner_radius: 6.0,
        shadow_radius: 4.0,
        ..Default::default()
    };

    let layout = TooltipLayout {
        background_bounds: TextBounds::new(20.0, 30.0, 180.0, 70.0),
        text_position: Vec2 { x: 26.0, y: 34.0 },
        text: "Tooltip text".to_string(),
        opacity: 0.9,
        arrow_direction: ArrowDirection::None,
        arrow_size: 0.0,
        arrow_offset: 0.0,
    };

    renderer.begin_frame();
    renderer.queue(&layout, &config);

    let mut encoder = ctx
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Test Encoder"),
        });

    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Test Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        let result = renderer.render(&mut pass, &ctx.device, &ctx.queue, 256.0, 256.0);
        assert!(result.is_ok(), "Render should succeed: {:?}", result.err());
    }

    ctx.queue.submit(Some(encoder.finish()));
}

// =============================================================================
// Multiple tooltips in a single frame
// =============================================================================

#[tokio::test]
async fn test_multiple_tooltips_per_frame() {
    let ctx = create_context().await;
    let mut renderer = TooltipBackgroundRenderer::new(&ctx.device).unwrap();

    let config = TooltipConfig::default();

    renderer.begin_frame();
    for i in 0..5 {
        let y = 20.0 + i as f32 * 50.0;
        let layout = TooltipLayout {
            background_bounds: TextBounds::new(10.0, y, 200.0, y + 30.0),
            text_position: Vec2 {
                x: 16.0,
                y: y + 4.0,
            },
            text: format!("Tooltip #{}", i),
            opacity: 1.0,
            arrow_direction: ArrowDirection::None,
            arrow_size: 0.0,
            arrow_offset: 0.0,
        };
        renderer.queue(&layout, &config);
    }
    assert_eq!(renderer.queued_count(), 5);
}

// =============================================================================
// Config field coverage: corner_radius, shadow, border
// =============================================================================

#[tokio::test]
async fn test_config_variants() {
    let ctx = create_context().await;
    let mut renderer = TooltipBackgroundRenderer::new(&ctx.device).unwrap();

    // No border, no shadow, no corner radius
    let config_flat = TooltipConfig {
        border_width: 0.0,
        corner_radius: 0.0,
        shadow_radius: 0.0,
        ..Default::default()
    };

    // Heavy border, large radius, shadow
    let config_fancy = TooltipConfig {
        border_width: 3.0,
        corner_radius: 12.0,
        shadow_radius: 8.0,
        shadow_color: [0.0, 0.0, 0.0, 0.5],
        shadow_offset: [2.0, 4.0],
        ..Default::default()
    };

    let layout = TooltipLayout {
        background_bounds: TextBounds::new(50.0, 50.0, 200.0, 80.0),
        text_position: Vec2 { x: 56.0, y: 54.0 },
        text: "Test".to_string(),
        opacity: 1.0,
        arrow_direction: ArrowDirection::None,
        arrow_size: 0.0,
        arrow_offset: 0.0,
    };

    renderer.begin_frame();
    renderer.queue(&layout, &config_flat);
    renderer.queue(&layout, &config_fancy);
    assert_eq!(renderer.queued_count(), 2);

    // Create offscreen target and render
    let texture = ctx.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Test RT"),
        size: wgpu::Extent3d {
            width: 256,
            height: 256,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Bgra8UnormSrgb,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let mut encoder = ctx
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        let result = renderer.render(&mut pass, &ctx.device, &ctx.queue, 256.0, 256.0);
        assert!(result.is_ok());
    }
    ctx.queue.submit(Some(encoder.finish()));
}

// =============================================================================
// End-to-end: tooltip layout → background queue → text queue flow
// =============================================================================

#[tokio::test]
async fn test_tooltip_layout_to_render_flow() {
    let ctx = create_context().await;
    let font_atlas = FontAtlas::new(&ctx.device, &ctx.queue, 14.0).unwrap();
    let mut engine = TextLayoutEngine::new();

    let tooltip_text = "Revenue per Quarter (2024)";
    let style = TextStyle::new(14.0);

    // Measure text
    let measure = engine
        .layout_text(
            tooltip_text,
            Vec2 { x: 0.0, y: 0.0 },
            &style,
            &font_atlas,
            None,
        )
        .unwrap();

    let config = TooltipConfig {
        corner_radius: 4.0,
        shadow_radius: 6.0,
        ..Default::default()
    };

    let active = ActiveTooltip {
        text: tooltip_text.to_string(),
        position: Vec2 { x: 200.0, y: 80.0 },
        opacity: 0.85,
        source_bounds: TextBounds::new(150.0, 60.0, 250.0, 76.0),
    };

    let layout = compute_tooltip_layout(
        &active,
        &config,
        measure.bounds.width(),
        measure.bounds.height(),
        800.0,
        600.0,
    );

    // Verify layout is sane
    assert!(layout.background_bounds.width() > 0.0);
    assert!(layout.background_bounds.height() > 0.0);
    assert_eq!(layout.opacity, 0.85);
    assert_eq!(layout.text, tooltip_text);

    // Queue the background
    let mut renderer = TooltipBackgroundRenderer::new(&ctx.device).unwrap();
    renderer.begin_frame();
    renderer.queue(&layout, &config);
    assert_eq!(renderer.queued_count(), 1);
}

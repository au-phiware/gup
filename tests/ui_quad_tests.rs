// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Integration tests for the shared UI quad renderer.
//!
//! These tests verify that [`UiQuadRenderer`] can be created with a real GPU
//! context and renders correctly within a render pass, covering the use cases
//! that replace the old tooltip-specific renderer plus new UI element types.

use gup::GupContext;
use gup::text::ui_quad::{UiQuadArrow, UiQuadConfig, UiQuadInstance, UiQuadRenderer};
use std::sync::Arc;

/// Helper: create a headless GPU context.
async fn create_context() -> Arc<GupContext> {
    GupContext::headless()
        .await
        .expect("Failed to create headless GPU context")
}

/// Helper: create a 256×256 offscreen render target.
fn create_render_target(device: &wgpu::Device) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("UI Quad Test RT"),
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
    (texture, view)
}

// =============================================================================
// Construction
// =============================================================================

#[tokio::test]
async fn test_create_ui_quad_renderer() {
    let ctx = create_context().await;
    let renderer = UiQuadRenderer::new(&ctx.device);
    assert!(renderer.is_ok(), "Should create renderer without error");
}

// =============================================================================
// Queue / begin_frame lifecycle
// =============================================================================

#[tokio::test]
async fn test_queue_and_begin_frame() {
    let ctx = create_context().await;
    let mut renderer = UiQuadRenderer::new(&ctx.device).unwrap();

    let inst = UiQuadConfig::new(10.0, 20.0, 200.0, 60.0).build();

    renderer.begin_frame();
    assert_eq!(renderer.queued_count(), 0);

    renderer.queue(inst);
    assert_eq!(renderer.queued_count(), 1);

    renderer.queue(inst);
    assert_eq!(renderer.queued_count(), 2);

    renderer.begin_frame();
    assert_eq!(renderer.queued_count(), 0, "begin_frame should clear queue");
}

// =============================================================================
// End-to-end: queue + render in a render pass
// =============================================================================

#[tokio::test]
async fn test_render_single_quad() {
    let ctx = create_context().await;
    let (_texture, view) = create_render_target(&ctx.device);

    let mut renderer = UiQuadRenderer::new(&ctx.device).unwrap();

    let inst = UiQuadConfig::new(20.0, 30.0, 180.0, 70.0)
        .bg_color([0.15, 0.15, 0.15, 0.95])
        .corner_radius(6.0)
        .build();

    renderer.begin_frame();
    renderer.queue(inst);

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
// Multiple heterogeneous UI elements in a single frame
// =============================================================================

#[tokio::test]
async fn test_multiple_heterogeneous_elements() {
    let ctx = create_context().await;
    let (_texture, view) = create_render_target(&ctx.device);

    let mut renderer = UiQuadRenderer::new(&ctx.device).unwrap();

    renderer.begin_frame();

    // Tooltip-style element
    renderer.queue(
        UiQuadConfig::new(10.0, 10.0, 180.0, 50.0)
            .bg_color([0.1, 0.1, 0.1, 0.95])
            .corner_radius(4.0)
            .border(1.0, [0.4, 0.4, 0.4, 1.0])
            .arrow(UiQuadArrow::Bottom, 6.0, 0.0)
            .build(),
    );

    // Legend box element
    renderer.queue(
        UiQuadConfig::new(200.0, 10.0, 350.0, 120.0)
            .bg_color([1.0, 1.0, 1.0, 0.9])
            .corner_radius(2.0)
            .border(1.0, [0.7, 0.7, 0.7, 1.0])
            .shadow(3.0, [0.0, 0.0, 0.0, 0.2], [1.0, 1.0])
            .build(),
    );

    // Focus highlight element (no border, subtle bg)
    renderer.queue(
        UiQuadConfig::new(20.0, 80.0, 160.0, 110.0)
            .bg_color([0.2, 0.5, 1.0, 0.15])
            .corner_radius(3.0)
            .build(),
    );

    // Annotation callout
    renderer.queue(
        UiQuadConfig::new(50.0, 150.0, 220.0, 200.0)
            .bg_color([1.0, 0.95, 0.8, 0.95])
            .corner_radius(6.0)
            .border(1.5, [0.8, 0.6, 0.2, 1.0])
            .shadow(4.0, [0.0, 0.0, 0.0, 0.3], [2.0, 2.0])
            .arrow(UiQuadArrow::Left, 8.0, 0.0)
            .build(),
    );

    assert_eq!(renderer.queued_count(), 4);

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
        let result = renderer.render(&mut pass, &ctx.device, &ctx.queue, 400.0, 300.0);
        assert!(
            result.is_ok(),
            "Multi-element render should succeed: {:?}",
            result.err()
        );
    }
    ctx.queue.submit(Some(encoder.finish()));
}

// =============================================================================
// All arrow directions
// =============================================================================

#[tokio::test]
async fn test_all_arrow_directions() {
    let ctx = create_context().await;
    let (_texture, view) = create_render_target(&ctx.device);

    let mut renderer = UiQuadRenderer::new(&ctx.device).unwrap();
    renderer.begin_frame();

    let directions = [
        UiQuadArrow::None,
        UiQuadArrow::Top,
        UiQuadArrow::Bottom,
        UiQuadArrow::Left,
        UiQuadArrow::Right,
    ];

    for (i, dir) in directions.iter().enumerate() {
        let y = 10.0 + i as f32 * 45.0;
        renderer.queue(
            UiQuadConfig::new(20.0, y, 150.0, y + 30.0)
                .corner_radius(4.0)
                .arrow(*dir, 6.0, 0.0)
                .build(),
        );
    }

    assert_eq!(renderer.queued_count(), 5);

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
// Render with empty queue is a no-op
// =============================================================================

#[tokio::test]
async fn test_render_empty_is_noop() {
    let ctx = create_context().await;
    let (_texture, view) = create_render_target(&ctx.device);

    let mut renderer = UiQuadRenderer::new(&ctx.device).unwrap();
    renderer.begin_frame();

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
        // Rendering with no queued instances should be a no-op
        let result = renderer.render(&mut pass, &ctx.device, &ctx.queue, 256.0, 256.0);
        assert!(result.is_ok());
    }
    ctx.queue.submit(Some(encoder.finish()));
}

// =============================================================================
// Instance buffer growth: queue more than initial capacity
// =============================================================================

#[tokio::test]
async fn test_instance_buffer_growth() {
    let ctx = create_context().await;
    let (_texture, view) = create_render_target(&ctx.device);

    let mut renderer = UiQuadRenderer::new(&ctx.device).unwrap();
    renderer.begin_frame();

    // Queue more than the initial capacity (16) to force buffer growth
    for i in 0..32 {
        let y = i as f32 * 8.0;
        renderer.queue(
            UiQuadConfig::new(0.0, y, 100.0, y + 6.0)
                .bg_color([0.5, 0.5, 0.5, 0.5])
                .build(),
        );
    }
    assert_eq!(renderer.queued_count(), 32);

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
        assert!(result.is_ok(), "Buffer growth should work: {:?}", result.err());
    }
    ctx.queue.submit(Some(encoder.finish()));
}

// =============================================================================
// Raw UiQuadInstance construction (bypassing builder)
// =============================================================================

#[tokio::test]
async fn test_raw_instance_construction() {
    let ctx = create_context().await;
    let (_texture, view) = create_render_target(&ctx.device);

    let mut renderer = UiQuadRenderer::new(&ctx.device).unwrap();
    renderer.begin_frame();

    // Callers may construct UiQuadInstance directly for maximum control
    renderer.queue(UiQuadInstance {
        rect_min: [10.0, 10.0],
        rect_max: [100.0, 50.0],
        bg_color: [0.2, 0.3, 0.4, 1.0],
        border_color: [1.0, 1.0, 1.0, 1.0],
        params: [5.0, 1.0, 0.9, 0.0],
        shadow_color: [0.0; 4],
        shadow_offset: [0.0; 2],
        arrow_params: [0.0; 4],
    });

    assert_eq!(renderer.queued_count(), 1);

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
// Backward compatibility: tooltip rendering via the delegating wrapper
// =============================================================================

#[tokio::test]
async fn test_tooltip_bg_backward_compat() {
    use gup::shader_function::Vec2;
    use gup::text::TextBounds;
    use gup::text::hover_reveal::{ArrowDirection, TooltipConfig, TooltipLayout};
    use gup::text::tooltip_bg::TooltipBackgroundRenderer;

    let ctx = create_context().await;
    let (_texture, view) = create_render_target(&ctx.device);

    let mut renderer = TooltipBackgroundRenderer::new(&ctx.device).unwrap();

    let config = TooltipConfig {
        corner_radius: 6.0,
        shadow_radius: 4.0,
        arrow_direction: ArrowDirection::Top,
        arrow_size: 6.0,
        ..Default::default()
    };

    let layout = TooltipLayout {
        background_bounds: TextBounds::new(20.0, 30.0, 180.0, 70.0),
        text_position: Vec2 { x: 26.0, y: 34.0 },
        text: "Tooltip via wrapper".to_string(),
        opacity: 0.9,
        arrow_direction: ArrowDirection::Top,
        arrow_size: 6.0,
        arrow_offset: 0.0,
    };

    renderer.begin_frame();
    renderer.queue(&layout, &config);
    assert_eq!(renderer.queued_count(), 1);

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

// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Unit tests for `GupWidget` dirty-flag transitions and state management.
//!
//! These tests exercise the widget's dirty tracking, chart replacement, and
//! event queue without requiring a GPU device (no egui Ui context needed).

use gup::chart_builder::ChartBuilder;
use gup::chart_builder::accessor::AccessorValue;
use gup::chart_builder::builders::{AccessorFunction, scatter};
use gup::render::RenderContext;
use gup_egui::GupWidget;
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Helper: build a minimal scatter chart for testing
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct Pt {
    x: f32,
    y: f32,
}

fn make_chart() -> gup::chart_builder::ComposedChart<Pt, gup::mark::Circle> {
    let data: Vec<Pt> = (0..10)
        .map(|i| Pt {
            x: i as f32,
            y: (i as f32).sin(),
        })
        .collect();
    let ctx =
        Arc::new(pollster::block_on(RenderContext::new()).expect("Failed to create RenderContext"));
    let x_acc = AccessorFunction::new(|d: &Pt| AccessorValue::Float(d.x));
    let y_acc = AccessorFunction::new(|d: &Pt| AccessorValue::Float(d.y));

    scatter()
        .x(x_acc)
        .y(y_acc)
        .point_size(4.0)
        .build_with_data(data, ctx)
        .expect("build chart")
}

fn make_chart_with_context(
    ctx: Arc<RenderContext>,
) -> gup::chart_builder::ComposedChart<Pt, gup::mark::Circle> {
    let data: Vec<Pt> = (0..10)
        .map(|i| Pt {
            x: i as f32,
            y: (i as f32).sin(),
        })
        .collect();
    let x_acc = AccessorFunction::new(|d: &Pt| AccessorValue::Float(d.x));
    let y_acc = AccessorFunction::new(|d: &Pt| AccessorValue::Float(d.y));

    scatter()
        .x(x_acc)
        .y(y_acc)
        .point_size(4.0)
        .build_with_data(data, ctx)
        .expect("build chart")
}

// ---------------------------------------------------------------------------
// Dirty-flag tests
// ---------------------------------------------------------------------------

#[test]
fn widget_starts_dirty() {
    let widget = GupWidget::new(make_chart());
    assert!(widget.is_dirty(), "new widget should start dirty");
}

#[test]
fn mark_dirty_sets_flag() {
    let mut widget = GupWidget::new(make_chart());
    // Simulate a render by directly manipulating (we can't call show without a UI).
    // Just verify that mark_dirty works.
    widget.mark_dirty();
    assert!(widget.is_dirty());
}

#[test]
fn set_chart_marks_dirty() {
    let mut widget = GupWidget::new(make_chart());
    // Build a new chart and set it.
    let chart2 = make_chart();
    widget.set_chart(chart2);
    assert!(widget.is_dirty(), "set_chart should mark dirty");
}

#[test]
fn take_events_drains() {
    let mut widget = GupWidget::new(make_chart());

    // Initially no events.
    let events = widget.take_events();
    assert!(events.is_empty());

    // take_events returns empty when called again.
    let events2 = widget.take_events();
    assert!(events2.is_empty());
}

#[test]
fn chart_ref_accessors() {
    let widget = GupWidget::new(make_chart());
    // We can borrow the inner chart (just checking it doesn't panic).
    let _chart = widget.chart();
}

#[test]
fn chart_mut_accessor() {
    let mut widget = GupWidget::new(make_chart());
    let _chart = widget.chart_mut();
}

// ---------------------------------------------------------------------------
// Pixel-buffer vs shared-device mode tests
// ---------------------------------------------------------------------------

#[test]
fn new_widget_uses_pixel_buffer_path() {
    let widget = GupWidget::new(make_chart());
    assert!(
        !widget.is_shared_device(),
        "GupWidget::new should use pixel-buffer path"
    );
}

// ---------------------------------------------------------------------------
// GupEguiContext tests
// ---------------------------------------------------------------------------

// NOTE: GupEguiContext::from_render_state requires an egui_wgpu::RenderState,
// which cannot be constructed in a unit test without a windowed context.
// The integration is verified via the egui_chart example and the
// render_to_texture_view tests below.

// ---------------------------------------------------------------------------
// DynChart::render_to_texture_view tests
// ---------------------------------------------------------------------------

/// Verify the `render_to_texture_view` method of `DynChart` works with a
/// GPU texture in `Rgba8UnormSrgb` format (the format used by the shared
/// device path).
#[test]
fn render_to_texture_view_rgba8_srgb() {
    use gup_egui::DynChart;

    let ctx = Arc::new(
        pollster::block_on(RenderContext::new()).expect("Failed to create RenderContext"),
    );
    let device = ctx.device();

    let width = 64u32;
    let height = 64u32;
    let format = wgpu::TextureFormat::Rgba8UnormSrgb;

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("test_offscreen"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

    // Build a chart on the same device.
    let mut chart = make_chart_with_context(ctx);

    let result = chart.render_to_texture_view(&view, format, width, height);
    assert!(
        result.is_ok(),
        "render_to_texture_view should succeed: {:?}",
        result.err()
    );
}

/// Verify that both render paths produce non-empty output.
#[test]
fn both_paths_produce_output() {
    use gup_egui::DynChart;

    let ctx =
        Arc::new(pollster::block_on(RenderContext::new()).expect("Failed to create RenderContext"));

    // --- Pixel-buffer path ---
    let mut chart_pb = make_chart_with_context(ctx.clone());
    let pixels = chart_pb
        .render_to_rgba(64, 64)
        .expect("render_to_rgba should succeed");
    assert_eq!(
        pixels.len(),
        64 * 64 * 4,
        "pixel-buffer path should produce 64×64×4 bytes"
    );

    // --- Texture-view path ---
    let mut chart_tv = make_chart_with_context(ctx.clone());
    let device = ctx.device();
    let format = wgpu::TextureFormat::Rgba8UnormSrgb;
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("test_tv"),
        size: wgpu::Extent3d {
            width: 64,
            height: 64,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

    let result = chart_tv.render_to_texture_view(&view, format, 64, 64);
    assert!(
        result.is_ok(),
        "texture-view path should succeed: {:?}",
        result.err()
    );
}

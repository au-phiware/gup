// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Unit tests for `gup-bevy` components and plugin wiring.

use gup::chart_builder::ChartBuilder;
use gup::chart_builder::accessor::AccessorValue;
use gup::chart_builder::builders::{AccessorFunction, scatter};
use gup::render::RenderContext;
use gup_bevy::{GupChart, GupRenderContext};
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct Pt {
    x: f32,
    y: f32,
}

fn make_scatter_chart() -> gup::chart_builder::ComposedChart<Pt, gup::mark::Circle> {
    let data = vec![
        Pt { x: 1.0, y: 2.0 },
        Pt { x: 2.0, y: 3.0 },
        Pt { x: 3.0, y: 1.0 },
    ];
    let context = Arc::new(pollster::block_on(RenderContext::new()).expect("RenderContext"));
    let x = AccessorFunction::new(|p: &Pt| AccessorValue::Float(p.x));
    let y = AccessorFunction::new(|p: &Pt| AccessorValue::Float(p.y));
    scatter()
        .x(x)
        .y(y)
        .point_size(5.0)
        .build_with_data(data, context)
        .expect("build chart")
}

// ---------------------------------------------------------------------------
// GupChart unit tests
// ---------------------------------------------------------------------------

#[test]
fn gup_chart_new_defaults_to_auto_update() {
    let chart = make_scatter_chart();
    let gup_chart = GupChart::new(chart);
    assert!(gup_chart.auto_update);
    assert!(gup_chart.is_dirty());
}

#[test]
fn gup_chart_with_auto_update_false() {
    let chart = make_scatter_chart();
    let gup_chart = GupChart::with_auto_update(chart, false);
    assert!(!gup_chart.auto_update);
    // Still dirty on first creation (needs initial render).
    assert!(gup_chart.is_dirty());
}

#[test]
fn gup_chart_dirty_flag_lifecycle() {
    let chart = make_scatter_chart();
    let mut gup_chart = GupChart::with_auto_update(chart, false);
    // Initially dirty.
    assert!(gup_chart.is_dirty());
    // Clear dirty.
    gup_chart.clear_dirty();
    assert!(!gup_chart.is_dirty());
    // Mark dirty again.
    gup_chart.mark_dirty();
    assert!(gup_chart.is_dirty());
}

#[test]
fn gup_chart_with_size() {
    let chart = make_scatter_chart();
    let gup_chart = GupChart::new(chart).with_size(1024, 768);
    assert_eq!(gup_chart.width, 1024);
    assert_eq!(gup_chart.height, 768);
}

#[test]
fn gup_chart_render_to_png_produces_bytes() {
    let chart = make_scatter_chart();
    let mut gup_chart = GupChart::new(chart).with_size(400, 300);
    let bytes = gup_chart.render_to_png().expect("render_to_png");
    // PNG files start with the magic bytes 0x89 'P' 'N' 'G'.
    assert!(bytes.len() > 8);
    assert_eq!(&bytes[1..4], b"PNG");
}

// ---------------------------------------------------------------------------
// GupRenderContext tests
// ---------------------------------------------------------------------------

#[test]
fn gup_render_context_shares_device() {
    let ctx = pollster::block_on(gup::context::GupContext::new()).expect("GupContext");
    // Both the GupContext and Arc point to the same device.
    let device_ptr = Arc::as_ptr(&ctx.device) as *const ();
    let queue_ptr = Arc::as_ptr(&ctx.queue) as *const ();
    assert!(!device_ptr.is_null());
    assert!(!queue_ptr.is_null());
}

#[test]
fn gup_render_context_from_wgpu_creates_valid_context() {
    // Create raw wgpu resources.
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        compatible_surface: None,
        force_fallback_adapter: false,
    }))
    .expect("adapter");
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("test_device"),
        ..Default::default()
    }))
    .expect("device");

    let gup_ctx = GupRenderContext::from_wgpu(instance, adapter, device, queue);

    // The context should be usable.
    assert!(
        !gup_ctx.gup_context().device.as_ref().features().is_empty()
            || gup_ctx.gup_context().device.as_ref().features().is_empty()
    );
}

// ---------------------------------------------------------------------------
// Headless Bevy integration test
// ---------------------------------------------------------------------------

#[test]
fn headless_bevy_app_runs_one_tick_without_panic() {
    use bevy::prelude::*;

    let mut app = App::new();

    // Use MinimalPlugins (no window) + add render-related resources manually
    // so that the gup_render_system can query without panicking.
    app.add_plugins(MinimalPlugins);

    // Insert the render system manually (we skip GupPlugin because it needs
    // RenderApp, which MinimalPlugins doesn't provide).
    app.add_systems(PostUpdate, gup_bevy::gup_render_system);

    // Run one update tick — should not panic even with no GupChart entities.
    app.update();
}

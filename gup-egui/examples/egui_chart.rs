// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! # egui Scatter Plot Demo
//!
//! Demonstrates embedding a live-updating Gup scatter plot inside an egui
//! application using the **zero-copy shared-device** path.  The chart
//! renders on egui's own wgpu device — no second GPU adapter, no CPU pixel
//! readback.
//!
//! The plot shows an animated sine wave that updates every second.  Hover and
//! click interactions are forwarded from egui into Gup's event system.
//!
//! ## Running
//!
//! ```bash
//! cargo run -p gup-egui --example egui_chart
//! ```
//!
//! ## Integration Steps
//!
//! 1. Obtain `wgpu_render_state` from the eframe [`CreationContext`].
//! 2. Create a [`GupEguiContext`] to share the GPU device/queue with Gup.
//! 3. Build a `ComposedChart` using Gup's chart-builder API, passing the
//!    shared [`RenderContext`].
//! 4. Wrap it in a [`GupWidget`] via [`GupWidget::with_render_state`].
//! 5. Call `ui.add(&mut widget)` inside any egui panel.
//! 6. Call `widget.mark_dirty()` whenever the chart data changes.

use eframe::egui;
use gup::chart_builder::ChartBuilder;
use gup::chart_builder::accessor::AccessorValue;
use gup::chart_builder::builders::{AccessorFunction, scatter};
use gup::render::RenderContext;
use gup_egui::{GupEguiContext, GupWidget};
use std::sync::Arc;
use std::time::Instant;

// ---------------------------------------------------------------------------
// Data model
// ---------------------------------------------------------------------------

/// A simple 2-D point used as chart data.
#[derive(Debug, Clone)]
struct DataPoint {
    x: f32,
    y: f32,
}

// ---------------------------------------------------------------------------
// Application state
// ---------------------------------------------------------------------------

/// The top-level application struct.  Stores the `GupWidget` and a timer
/// that triggers data updates once per second.
struct ChartApp {
    /// The Gup chart widget (owns the chart and its texture).
    widget: GupWidget,
    /// Shared GPU context for building replacement charts on the same device.
    shared_context: Arc<RenderContext>,
    /// Wall-clock time used to animate the sine wave.
    start: Instant,
    /// The last second at which data was refreshed.
    last_update_sec: u64,
}

impl ChartApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Use the shared-device path when the wgpu backend is available.
        let render_state = cc
            .wgpu_render_state
            .as_ref()
            .expect("wgpu backend required for this example");

        let egui_ctx = GupEguiContext::from_render_state(render_state);
        let shared_context = egui_ctx.render_context().clone();

        // Build the initial scatter chart on the shared device.
        let chart = build_scatter_chart(0.0, shared_context.clone());
        let widget = GupWidget::with_render_state(chart, render_state.clone());

        Self {
            widget,
            shared_context,
            start: Instant::now(),
            last_update_sec: 0,
        }
    }
}

impl eframe::App for ChartApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Check if a full second has elapsed — if so, rebuild the chart data.
        let elapsed = self.start.elapsed().as_secs();
        if elapsed > self.last_update_sec {
            self.last_update_sec = elapsed;

            // Replace the chart with fresh data.
            let t = self.start.elapsed().as_secs_f32();
            let chart = build_scatter_chart(t, self.shared_context.clone());
            self.widget.set_chart(chart);
        }

        // --- Left side panel: controls & info ---
        egui::SidePanel::left("controls").show(ctx, |ui| {
            ui.heading("Gup × egui");
            ui.separator();
            ui.label(format!("Elapsed: {elapsed} s"));
            ui.label(format!("Dirty: {}", self.widget.is_dirty()));
            ui.label(format!(
                "Render path: {}",
                if self.widget.is_shared_device() {
                    "shared device (zero-copy)"
                } else {
                    "pixel buffer (fallback)"
                }
            ));

            ui.separator();
            ui.label("Hover or click the chart to see interaction events.");

            // Show any interaction events from the last frame.
            let events = self.widget.take_events();
            if !events.is_empty() {
                ui.separator();
                ui.label("Events:");
                for ev in &events {
                    ui.monospace(format!(
                        "  {} @ ({:.0}, {:.0})",
                        ev.interaction_type, ev.screen_position.x, ev.screen_position.y,
                    ));
                }
            }
        });

        // --- Central panel: the chart ---
        egui::CentralPanel::default().show(ctx, |ui| {
            // Display the Gup chart widget.  Because `GupWidget` implements
            // `egui::Widget` for `&mut GupWidget`, we can pass it directly
            // to `ui.add()`.
            ui.add(&mut self.widget);
        });

        // Request a repaint so the next data tick is picked up promptly.
        ctx.request_repaint();
    }
}

// ---------------------------------------------------------------------------
// Chart builder helper
// ---------------------------------------------------------------------------

/// Build a scatter chart whose data is a sine wave offset by `time`.
fn build_scatter_chart(
    time: f32,
    context: Arc<RenderContext>,
) -> gup::chart_builder::ComposedChart<DataPoint, gup::mark::Circle> {
    // Generate 60 data points along a sine wave.
    let data: Vec<DataPoint> = (0..60)
        .map(|i| {
            let x = i as f32 / 60.0 * 10.0;
            let y = (x + time).sin() * 3.0 + 5.0;
            DataPoint { x, y }
        })
        .collect();

    // Build the scatter plot using Gup's chart-builder API.
    let x_acc = AccessorFunction::new(|d: &DataPoint| AccessorValue::Float(d.x));
    let y_acc = AccessorFunction::new(|d: &DataPoint| AccessorValue::Float(d.y));

    scatter()
        .x(x_acc)
        .y(y_acc)
        .point_size(6.0)
        .fill_color([0.2, 0.5, 0.9, 1.0])
        .build_with_data(data, context)
        .expect("Failed to build scatter chart")
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

fn main() -> eframe::Result {
    env_logger::init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([900.0, 600.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Gup × egui — Animated Scatter Plot",
        options,
        Box::new(|cc| Ok(Box::new(ChartApp::new(cc)))),
    )
}

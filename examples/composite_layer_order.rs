// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Composite Chart Demo — Layer Ordering Control
//!
//! Demonstrates explicit layer ordering in composite charts (GUP-365).
//! Three layers are added in declaration order (bar, scatter, area) but
//! their rendering order is controlled via z-index values so that the
//! area is drawn first (behind), then bars, then scatter points on top.
//!
//! Features shown:
//! - `.z_index()` for per-layer rendering priority
//! - Lower z-index = drawn first (behind); higher = drawn last (on top)
//! - Default behaviour unchanged when z-index is not set
//!
//! Run with:
//!
//! ```sh
//! cargo run --example composite_layer_order
//! ```

use gup::chart_builder::ChartBuilder;
use gup::chart_builder::accessor::AccessorValue;
use gup::chart_builder::builders::composite::{CompositeChart, composite};
use gup::chart_builder::builders::{
    AccessorFunction, AreaChartBuilder, BarChartBuilder, ConfigurableBuilder, GridCapableBuilder,
    ScatterPlotBuilder,
};
use gup::{GupContext, PhysicalSize, SurfaceId};
use std::sync::Arc;
use wgpu::Color;
use winit::{
    application::ApplicationHandler,
    event::{ElementState, KeyEvent, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowAttributes, WindowId},
};

// ── Data type ───────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct DataPoint {
    x: f32,
    y: f32,
}

// ── Data generation ─────────────────────────────────────────────────────

fn generate_data() -> Vec<DataPoint> {
    (0..8)
        .map(|i| {
            let x = i as f32;
            let y = (x * 0.5).sin() * 10.0 + 15.0 + (x * 1.3).cos() * 5.0;
            DataPoint { x, y }
        })
        .collect()
}

// ── Application state ───────────────────────────────────────────────────

struct App {
    context: Option<Arc<GupContext>>,
    surface_id: Option<SurfaceId>,
    window: Option<Arc<Window>>,
    chart: Option<CompositeChart<DataPoint>>,
    prepared: bool,
}

impl App {
    fn new() -> Self {
        Self {
            context: None,
            surface_id: None,
            window: None,
            chart: None,
            prepared: false,
        }
    }

    fn render_frame(&mut self) {
        if let Some(context) = self.context.take() {
            let mut ctx = match Arc::try_unwrap(context) {
                Ok(c) => c,
                Err(arc) => {
                    self.context = Some(arc);
                    return;
                }
            };

            if !self.prepared {
                if let Some(chart) = &mut self.chart {
                    let format = ctx.surface_format();
                    if let Err(e) = chart.prepare_render(&ctx.device, &ctx.queue, format) {
                        eprintln!("Prepare error: {e}");
                    }
                }
                self.prepared = true;
            }

            let sid = self.surface_id.unwrap();
            match ctx.begin_frame_for_surface(sid) {
                Ok(mut frame) => {
                    let bg = Color {
                        r: 0.97,
                        g: 0.97,
                        b: 0.97,
                        a: 1.0,
                    };

                    {
                        let mut rp = frame.render_pass(Some(bg));
                        if let Some(chart) = &self.chart
                            && let Err(e) = chart.draw(&mut rp)
                        {
                            eprintln!("Draw error: {e}");
                        }
                    }

                    let _ = frame.finish();
                }
                Err(e) => eprintln!("Frame error: {e}"),
            }

            self.context = Some(Arc::new(ctx));
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        pollster::block_on(async {
            let gup_ctx = match GupContext::headless().await {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Failed to create GPU context: {e}");
                    event_loop.exit();
                    return;
                }
            };

            let mut ctx = match Arc::try_unwrap(gup_ctx) {
                Ok(c) => c,
                Err(_) => {
                    eprintln!("Failed to unwrap GPU context");
                    event_loop.exit();
                    return;
                }
            };

            let win_attrs = WindowAttributes::default()
                .with_title("Layer Ordering: area(z=1) → bar(z=5) → scatter(z=10)")
                .with_inner_size(winit::dpi::LogicalSize::new(800, 600));
            let window = Arc::new(event_loop.create_window(win_attrs).unwrap());
            let sid = SurfaceId::new();
            if let Err(e) = ctx.add_surface(sid, Arc::clone(&window)) {
                eprintln!("Failed to add surface: {e}");
                event_loop.exit();
                return;
            }

            let render_ctx = Arc::new(gup::RenderContext::new().await.expect("RenderContext"));

            let data = generate_data();
            println!("  Data points: {}", data.len());

            // Accessors shared by all layers.
            let x_acc = || AccessorFunction::new(|d: &DataPoint| AccessorValue::Float(d.x));
            let y_acc = || AccessorFunction::new(|d: &DataPoint| AccessorValue::Float(d.y));

            // Layers are added in declaration order: bar → scatter → area.
            // But z-index reorders them: area(1) → bar(5) → scatter(10).

            let bar_layer = BarChartBuilder::<DataPoint>::new()
                .x(x_acc())
                .y(y_acc())
                .color(AccessorFunction::new(|_: &DataPoint| {
                    AccessorValue::Color([0.682, 0.780, 0.910, 0.6])
                }));

            let scatter_layer = ScatterPlotBuilder::<DataPoint>::new()
                .x(x_acc())
                .y(y_acc())
                .fill_color([0.839, 0.153, 0.157, 1.0]);

            let area_layer = AreaChartBuilder::<DataPoint>::new()
                .x(x_acc())
                .y(y_acc())
                .color(AccessorFunction::new(|_: &DataPoint| {
                    AccessorValue::Color([0.173, 0.627, 0.173, 0.3])
                }));

            let chart = composite::<DataPoint>()
                .layer(bar_layer)
                .z_index(5) // middle: bars
                .layer(scatter_layer)
                .z_index(10) // top: scatter points
                .layer(area_layer)
                .z_index(1) // bottom: area fill
                .title("Layer Ordering Demo (z-index)")
                .width(800.0)
                .height(600.0)
                .light_grid()
                .build_with_data(data, render_ctx)
                .expect("chart build");

            println!(
                "✅ Composite built — {} layers, render order: {:?}",
                chart.additional_layer_count(),
                chart.render_order(),
            );
            println!("   Expected order: [2 (area, z=1), 0 (bar, z=5), 1 (scatter, z=10)]");

            self.context = Some(Arc::new(ctx));
            self.surface_id = Some(sid);
            self.window = Some(window);
            self.chart = Some(chart);
        });

        println!("Press ESC or Q to quit");
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let (Some(sid), Some(ctx_arc)) = (self.surface_id, self.context.take()) {
                    let mut ctx = Arc::try_unwrap(ctx_arc)
                        .unwrap_or_else(|arc| panic!("refs: {}", Arc::strong_count(&arc)));
                    let _ = ctx.resize_surface(sid, PhysicalSize::new(size.width, size.height));
                    self.context = Some(Arc::new(ctx));
                    self.prepared = false;
                }
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(KeyCode::Escape | KeyCode::KeyQ),
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } => event_loop.exit(),
            WindowEvent::RedrawRequested => self.render_frame(),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Layer Ordering Control Demo (GUP-365) ===\n");
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App::new();
    event_loop.run_app(&mut app)?;
    Ok(())
}

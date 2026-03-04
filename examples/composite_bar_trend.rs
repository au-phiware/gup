// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Composite Chart Demo — Bar Chart with Trend Line
//!
//! Demonstrates the `CompositeChartBuilder` GPU render pipeline (GUP-303)
//! by rendering a bar chart of quarterly sales data with a trend line
//! overlaid on a secondary y-axis.
//!
//! Features shown:
//! - Multi-layer composition (bar + line) with dual y-axis
//! - All layers rendered in a single render pass
//! - Secondary y-axis via `.layer_with_y2()`
//!
//! Run with:
//!
//! ```sh
//! cargo run --example composite_bar_trend
//! ```

use gup::chart_builder::ChartBuilder;
use gup::chart_builder::accessor::AccessorValue;
use gup::chart_builder::builders::composite::{CompositeChart, composite};
use gup::chart_builder::builders::{
    AccessorFunction, BarChartBuilder, ConfigurableBuilder, GridCapableBuilder, LineChartBuilder,
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
struct QuarterlyData {
    quarter: f32,
    revenue: f32,
    growth_pct: f32,
}

fn generate_quarterly_data() -> Vec<QuarterlyData> {
    vec![
        QuarterlyData {
            quarter: 0.0,
            revenue: 120.0,
            growth_pct: 0.0,
        },
        QuarterlyData {
            quarter: 1.0,
            revenue: 150.0,
            growth_pct: 25.0,
        },
        QuarterlyData {
            quarter: 2.0,
            revenue: 135.0,
            growth_pct: -10.0,
        },
        QuarterlyData {
            quarter: 3.0,
            revenue: 180.0,
            growth_pct: 33.3,
        },
        QuarterlyData {
            quarter: 4.0,
            revenue: 210.0,
            growth_pct: 16.7,
        },
        QuarterlyData {
            quarter: 5.0,
            revenue: 195.0,
            growth_pct: -7.1,
        },
        QuarterlyData {
            quarter: 6.0,
            revenue: 240.0,
            growth_pct: 23.1,
        },
        QuarterlyData {
            quarter: 7.0,
            revenue: 260.0,
            growth_pct: 8.3,
        },
    ]
}

// ── Application state ───────────────────────────────────────────────────

struct App {
    context: Option<Arc<GupContext>>,
    surface_id: Option<SurfaceId>,
    window: Option<Arc<Window>>,
    chart: Option<CompositeChart<QuarterlyData>>,
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
                        if let Some(chart) = &self.chart {
                            if let Err(e) = chart.draw(&mut rp) {
                                eprintln!("Draw error: {e}");
                            }
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
                    eprintln!("GPU context error: {e}");
                    event_loop.exit();
                    return;
                }
            };

            let mut ctx = match Arc::try_unwrap(gup_ctx) {
                Ok(c) => c,
                Err(_) => {
                    eprintln!("unwrap failed");
                    event_loop.exit();
                    return;
                }
            };

            let win_attrs = WindowAttributes::default()
                .with_title("Composite Chart: Bar + Trend Line")
                .with_inner_size(winit::dpi::LogicalSize::new(900, 600));
            let window = Arc::new(event_loop.create_window(win_attrs).unwrap());
            let sid = SurfaceId::new();
            if let Err(e) = ctx.add_surface(sid, Arc::clone(&window)) {
                eprintln!("Surface error: {e}");
                event_loop.exit();
                return;
            }

            let render_ctx = Arc::new(gup::RenderContext::new().await.expect("RenderContext"));
            let data = generate_quarterly_data();

            let bar_layer = BarChartBuilder::<QuarterlyData>::new()
                .x(AccessorFunction::new(|d: &QuarterlyData| {
                    AccessorValue::Float(d.quarter)
                }))
                .y(AccessorFunction::new(|d: &QuarterlyData| {
                    AccessorValue::Float(d.revenue)
                }));

            let trend_layer = LineChartBuilder::<QuarterlyData>::new()
                .x(AccessorFunction::new(|d: &QuarterlyData| {
                    AccessorValue::Float(d.quarter)
                }))
                .y(AccessorFunction::new(|d: &QuarterlyData| {
                    AccessorValue::Float(d.growth_pct)
                }))
                .stroke_color([0.839, 0.153, 0.157, 1.0])
                .stroke_width_px(2.5);

            let chart = composite::<QuarterlyData>()
                .layer(bar_layer)
                .layer_with_y2(trend_layer)
                .title("Quarterly Revenue & Growth Rate")
                .width(900.0)
                .height(600.0)
                .business_grid()
                .build_with_data(data, render_ctx)
                .expect("chart build");

            println!(
                "✅ Composite chart built — {} layers, dual-y={}",
                chart.additional_layer_count(),
                chart.has_secondary_y_axis()
            );

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
    println!("=== Composite Bar + Trend Line (Windowed) ===\n");
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App::new();
    event_loop.run_app(&mut app)?;
    Ok(())
}

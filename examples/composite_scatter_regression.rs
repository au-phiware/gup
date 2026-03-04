// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Composite Chart Demo — Scatter Plot with Regression Line
//!
//! Demonstrates the `CompositeChartBuilder` GPU render pipeline (GUP-303)
//! by rendering a scatter plot with an overlaid regression line in a
//! single wgpu render pass.
//!
//! Features shown:
//! - Multi-layer composition (scatter + line) with shared axes
//! - All layers rendered in a single render pass
//! - Automatic domain unification and NDC scaling
//!
//! Run with:
//!
//! ```sh
//! cargo run --example composite_scatter_regression
//! ```

use gup::chart_builder::ChartBuilder;
use gup::chart_builder::accessor::AccessorValue;
use gup::chart_builder::builders::composite::{CompositeChart, composite};
use gup::chart_builder::builders::{
    AccessorFunction, ConfigurableBuilder, GridCapableBuilder, LineChartBuilder, ScatterPlotBuilder,
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

// ── Deterministic pseudo-random generator ───────────────────────────────

struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next_u64(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        self.0
    }

    fn uniform(&mut self) -> f32 {
        let bits = (self.next_u64() >> 33) as f32;
        (bits + 1.0) / (2.0f32.powi(31) + 1.0)
    }

    fn normal_pair(&mut self) -> (f32, f32) {
        let u1 = self.uniform();
        let u2 = self.uniform();
        let r = (-2.0 * u1.ln()).sqrt();
        let theta = 2.0 * std::f32::consts::PI * u2;
        (r * theta.cos(), r * theta.sin())
    }
}

// ── Data generation ─────────────────────────────────────────────────────

fn generate_scatter_data(n: usize, seed: u64) -> Vec<DataPoint> {
    let mut rng = Rng::new(seed);
    (0..n)
        .map(|i| {
            let x = i as f32 * 10.0 / n as f32;
            let (noise, _) = rng.normal_pair();
            DataPoint {
                x,
                y: 2.0 * x + 5.0 + noise * 1.5,
            }
        })
        .collect()
}

fn linear_regression(data: &[DataPoint]) -> (f32, f32) {
    let n = data.len() as f32;
    let sum_x: f32 = data.iter().map(|d| d.x).sum();
    let sum_y: f32 = data.iter().map(|d| d.y).sum();
    let sum_xy: f32 = data.iter().map(|d| d.x * d.y).sum();
    let sum_xx: f32 = data.iter().map(|d| d.x * d.x).sum();
    let slope = (n * sum_xy - sum_x * sum_y) / (n * sum_xx - sum_x * sum_x);
    let intercept = (sum_y - slope * sum_x) / n;
    (slope, intercept)
}

fn generate_regression_line(data: &[DataPoint], slope: f32, intercept: f32) -> Vec<DataPoint> {
    let x_min = data.iter().map(|d| d.x).fold(f32::INFINITY, f32::min);
    let x_max = data.iter().map(|d| d.x).fold(f32::NEG_INFINITY, f32::max);
    vec![
        DataPoint {
            x: x_min,
            y: slope * x_min + intercept,
        },
        DataPoint {
            x: x_max,
            y: slope * x_max + intercept,
        },
    ]
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

            // Prepare GPU resources once.
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
            // 1. Create GPU context (headless first, then add surface).
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

            // 2. Create window.
            let win_attrs = WindowAttributes::default()
                .with_title("Composite Chart: Scatter + Regression Line")
                .with_inner_size(winit::dpi::LogicalSize::new(800, 600));
            let window = Arc::new(event_loop.create_window(win_attrs).unwrap());
            let sid = SurfaceId::new();
            if let Err(e) = ctx.add_surface(sid, Arc::clone(&window)) {
                eprintln!("Failed to add surface: {e}");
                event_loop.exit();
                return;
            }

            // 3. Build chart data.
            let render_ctx = Arc::new(gup::RenderContext::new().await.expect("RenderContext"));

            let scatter_data = generate_scatter_data(100, 42);
            let (slope, intercept) = linear_regression(&scatter_data);
            let regression_data = generate_regression_line(&scatter_data, slope, intercept);
            let mut all_data = scatter_data;
            all_data.extend(regression_data);

            let scatter_layer = ScatterPlotBuilder::<DataPoint>::new()
                .x(AccessorFunction::new(|d: &DataPoint| {
                    AccessorValue::Float(d.x)
                }))
                .y(AccessorFunction::new(|d: &DataPoint| {
                    AccessorValue::Float(d.y)
                }))
                .fill_color([0.122, 0.467, 0.706, 0.7]);

            let line_layer = LineChartBuilder::<DataPoint>::new()
                .x(AccessorFunction::new(|d: &DataPoint| {
                    AccessorValue::Float(d.x)
                }))
                .y(AccessorFunction::new(|d: &DataPoint| {
                    AccessorValue::Float(d.y)
                }))
                .stroke_color([0.839, 0.153, 0.157, 1.0])
                .stroke_width_px(3.0);

            let chart = composite::<DataPoint>()
                .layer(scatter_layer)
                .layer(line_layer)
                .title("Scatter + Regression Line")
                .width(800.0)
                .height(600.0)
                .light_grid()
                .build_with_data(all_data, render_ctx)
                .expect("chart build");

            println!(
                "✅ Composite chart built — {} layers",
                chart.additional_layer_count()
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
                    self.prepared = false; // re-prepare after resize
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
    println!("=== Composite Scatter + Regression Line (Windowed) ===\n");
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App::new();
    event_loop.run_app(&mut app)?;
    Ok(())
}

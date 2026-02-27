// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Chart Builder Hover Reveal Demo
//!
//! Demonstrates how the chart builder automatically supports hover reveal
//! for truncated axis labels — no manual `ClippedTextRegistry` or
//! `HoverRevealState` management required.
//!
//! Features shown:
//! - `ChartConfig::with_hover_reveal(true)` to enable hover reveal
//! - `ChartConfig::with_tooltip_config(...)` to customise tooltip style
//! - Automatic clipped-text registration during `queue_chart_text`
//! - Tooltip rendering via `queue_tooltip_text`
//! - Mouse position forwarding via `update_hover`
//!
//! Controls:
//! - Move the mouse over axis labels to see tooltip (if any are clipped)
//! - Press Esc to exit

use gup::{
    GupContext, SurfaceId,
    axis::{AxisConfiguration, AxisPosition, LinearAxis, TickPipeline},
    chart_builder::{ChartConfig, ComposedChart, TitleConfig},
    label::{LabelConstraints, LabelPositioner},
    text::{
        FontAtlasManager, FontDatabase, TextLayoutEngine, TextRenderer, TextStyle,
        hover_reveal::TooltipConfig,
    },
};
use std::sync::Arc;
use std::time::Instant;
use wgpu::util::DeviceExt;
use winit::{
    application::ApplicationHandler,
    event::{ElementState, KeyEvent, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowAttributes, WindowId},
};

const WINDOW_WIDTH: u32 = 900;
const WINDOW_HEIGHT: u32 = 700;

/// Dummy data point — chart builder requires a Selection type parameter.
#[derive(Debug, Clone)]
struct DataPoint {
    _x: f32,
    _y: f32,
}

/// Application state.
struct App {
    context: Option<Arc<GupContext>>,
    surface_id: Option<SurfaceId>,
    window: Option<Arc<Window>>,
    chart: Option<ComposedChart<DataPoint, gup::Circle>>,
    tick_pipeline: Option<TickPipeline>,
    text_renderer: Option<TextRenderer>,
    font_manager: Option<FontAtlasManager>,
    layout_engine: Option<TextLayoutEngine>,
    mouse_pos: (f32, f32),
    last_frame: Instant,
}

impl App {
    fn new() -> Self {
        Self {
            context: None,
            surface_id: None,
            window: None,
            chart: None,
            tick_pipeline: None,
            text_renderer: None,
            font_manager: None,
            layout_engine: None,
            mouse_pos: (0.0, 0.0),
            last_frame: Instant::now(),
        }
    }

    async fn create_context(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.context.is_none() {
            let context = GupContext::headless().await?;
            self.context = Some(context);
        }
        Ok(())
    }

    fn create_window(
        &mut self,
        event_loop: &ActiveEventLoop,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let attrs = WindowAttributes::default()
            .with_title("Chart Builder Hover Reveal Demo")
            .with_inner_size(winit::dpi::LogicalSize::new(WINDOW_WIDTH, WINDOW_HEIGHT));
        let window = Arc::new(event_loop.create_window(attrs)?);
        let surface_id = SurfaceId::new();

        if let Some(context) = self.context.take() {
            let mut ctx = Arc::try_unwrap(context).map_err(|_| "Failed to unwrap context")?;
            ctx.add_surface(surface_id, Arc::clone(&window))?;
            self.context = Some(Arc::new(ctx));
        }

        self.window = Some(window);
        self.surface_id = Some(surface_id);
        Ok(())
    }

    /// Build the chart with hover reveal enabled.
    fn build_chart(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let _context = self.context.as_ref().ok_or("No context")?;
        let render_ctx = Arc::new(pollster::block_on(gup::RenderContext::new())?);

        // Customise tooltip appearance
        let tooltip_config = TooltipConfig {
            font_size: 16.0,
            show_delay: 0.3,
            fade_in_duration: 0.15,
            ..TooltipConfig::default()
        };

        // Configure the chart — hover reveal enabled in one line
        let config = ChartConfig {
            width: WINDOW_WIDTH as f32,
            height: WINDOW_HEIGHT as f32,
            ..ChartConfig::default()
        }
        .with_title_config(
            TitleConfig::new("Chart Builder Hover Reveal Demo")
                .with_subtitle("Hover truncated labels to see full text"),
        )
        .with_title_style(TextStyle::new(20.0).bold())
        .with_label_style(TextStyle::new(14.0))
        .with_hover_reveal(true)
        .with_tooltip_config(tooltip_config);

        let sel = gup::selection::Selection::<DataPoint, gup::Circle>::new(vec![], render_ctx)?;

        let axis_config = AxisConfiguration::default();
        let chart = ComposedChart::new(sel, config)
            .with_bottom_axis(Box::new(LinearAxis::new(
                AxisPosition::Bottom,
                axis_config.clone(),
            )))
            .with_left_axis(Box::new(LinearAxis::new(AxisPosition::Left, axis_config)));

        self.chart = Some(chart);
        println!("✅ Chart built with hover reveal enabled");
        Ok(())
    }

    fn initialize_text_rendering(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(context) = &self.context
            && self.text_renderer.is_none()
        {
            self.text_renderer = Some(TextRenderer::new(&context.device)?);
            self.font_manager = Some(FontAtlasManager::new(FontDatabase::new(), 14.0));
            self.layout_engine = Some(TextLayoutEngine::new());
            println!("✅ Text rendering initialized");
        }
        Ok(())
    }

    fn render_frame(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.chart.is_none() {
            self.build_chart()?;
        }
        if self.text_renderer.is_none() {
            self.initialize_text_rendering()?;
        }

        let surface_id = match self.surface_id {
            Some(id) => id,
            None => return Ok(()),
        };

        // Delta time for hover animation
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame).as_secs_f32();
        self.last_frame = now;

        // Update hover state with current mouse position
        if let Some(chart) = &mut self.chart {
            chart.update_hover(self.mouse_pos.0, self.mouse_pos.1, dt);
        }

        if let Some(context) = self.context.take() {
            let mut ctx = Arc::try_unwrap(context).map_err(|_| "Failed to unwrap context")?;

            match ctx.begin_frame_for_surface(surface_id) {
                Ok(mut frame) => {
                    let device = frame.device_arc();
                    let queue = frame.queue_arc();
                    let (w, h) = (WINDOW_WIDTH as f32, WINDOW_HEIGHT as f32);

                    let chart = self.chart.as_mut().unwrap();
                    let geom = chart.generate_axis_geometry_instanced();

                    // --- Queue text (before render pass) ---
                    if let (Some(tr), Some(fm), Some(le)) = (
                        &mut self.text_renderer,
                        &mut self.font_manager,
                        &mut self.layout_engine,
                    ) {
                        tr.begin_frame();

                        let mut positioner = LabelPositioner::new();
                        let constraints = LabelConstraints::axis_labels();

                        // queue_chart_text_resolved handles hover reveal registration
                        if let Err(e) = chart.queue_chart_text_resolved(
                            &frame,
                            tr,
                            fm,
                            le,
                            &mut positioner,
                            &constraints,
                        ) {
                            eprintln!("⚠️ Chart text failed: {e}");
                        }

                        // Queue tooltip text (renders on top of everything)
                        match chart.queue_tooltip_text(&frame, tr, fm, le) {
                            Ok(true) => println!(
                                "🔍 Tooltip active: {} entries in registry",
                                chart.clipped_text_registry().len()
                            ),
                            Ok(false) => {}
                            Err(e) => eprintln!("⚠️ Tooltip failed: {e}"),
                        }
                    }

                    // --- Render pass ---
                    let has_ticks = !geom.tick_instances.is_empty();

                    // Upload tick instances
                    if has_ticks && self.tick_pipeline.is_none() {
                        self.tick_pipeline = Some(TickPipeline::new(
                            &device,
                            wgpu::TextureFormat::Bgra8UnormSrgb,
                        ));
                    }

                    let tick_draw = if has_ticks {
                        let tp = self.tick_pipeline.as_ref().unwrap();
                        let (base, inst) = tp.upload(&device, &queue, &geom.tick_instances);
                        Some((base, inst, geom.tick_instances.len() as u32))
                    } else {
                        None
                    };

                    // Upload axis-line vertices
                    let _line_buf = if !geom.line_vertices.is_empty() {
                        Some(
                            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                label: Some("axis-lines"),
                                contents: bytemuck::cast_slice(&geom.line_vertices),
                                usage: wgpu::BufferUsages::VERTEX,
                            }),
                        )
                    } else {
                        None
                    };

                    {
                        let mut pass = frame.render_pass(Some(wgpu::Color {
                            r: 0.97,
                            g: 0.97,
                            b: 0.98,
                            a: 1.0,
                        }));

                        // Draw ticks
                        if let (Some(tp), Some((base, inst, count))) =
                            (self.tick_pipeline.as_ref(), &tick_draw)
                        {
                            tp.draw(&mut pass, base, inst, *count);
                        }

                        // Draw queued text
                        if let (Some(tr), Some(fm)) =
                            (self.text_renderer.as_mut(), self.font_manager.as_ref())
                            && let Err(e) =
                                tr.render_queued_text_multi(&mut pass, &device, &queue, fm, w, h)
                        {
                            eprintln!("⚠️ Text render: {e}");
                        }
                    }

                    frame.finish()?;
                }
                Err(e) => {
                    eprintln!("Frame error: {e}");
                }
            }

            self.context = Some(Arc::new(ctx));
        }

        // Request redraw for continuous animation (hover transitions)
        if let Some(w) = &self.window {
            w.request_redraw();
        }

        Ok(())
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        pollster::block_on(self.create_context()).expect("context");
        self.create_window(event_loop).expect("window");
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _wid: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(KeyCode::Escape),
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } => event_loop.exit(),
            WindowEvent::CursorMoved { position, .. } => {
                self.mouse_pos = (position.x as f32, position.y as f32);
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => {
                if let Err(e) = self.render_frame() {
                    eprintln!("Render error: {e}");
                }
            }
            _ => {}
        }
    }
}

fn main() {
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = App::new();
    event_loop.run_app(&mut app).unwrap();
}

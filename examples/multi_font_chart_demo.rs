// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Multi-Font Chart Builder Demo
//!
//! Demonstrates how the chart builder API integrates with `FontAtlasManager`
//! so that `TextStyle.font_family` works automatically for axis labels and
//! chart titles — no manual font atlas management required.
//!
//! Features shown:
//! - Chart title rendered in a serif font
//! - Axis labels rendered in a sans-serif font
//! - Automatic font atlas creation via `FontAtlasManager`
//! - Label collision detection with `queue_chart_text_resolved`
//! - Single render pass for axes + text

use gup::{
    GupContext, PhysicalSize, SurfaceId,
    axis::{AxisConfiguration, AxisPosition, LinearAxis},
    chart_builder::{ChartConfig, ComposedChart},
    label::{LabelConstraints, LabelPositioner},
    render::Vertex,
    text::{FontAtlasManager, FontDatabase, TextLayoutEngine, TextRenderer, TextStyle},
};
use std::sync::Arc;
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

/// Application state.
struct MultiFontChartApp {
    context: Option<Arc<GupContext>>,
    surface_id: Option<SurfaceId>,
    window: Option<Arc<Window>>,

    // Chart definition
    chart: Option<ComposedChart<DataPoint, gup::Circle>>,

    // Text rendering components
    text_renderer: Option<TextRenderer>,
    font_manager: Option<FontAtlasManager>,
    layout_engine: Option<TextLayoutEngine>,
}

/// Dummy data point (chart builder requires a Selection type parameter).
#[derive(Debug, Clone)]
struct DataPoint {
    _x: f32,
    _y: f32,
}

impl MultiFontChartApp {
    fn new() -> Self {
        Self {
            context: None,
            surface_id: None,
            window: None,
            chart: None,
            text_renderer: None,
            font_manager: None,
            layout_engine: None,
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
        let window_attributes = WindowAttributes::default()
            .with_title("Gup Multi-Font Chart Builder Demo")
            .with_inner_size(winit::dpi::LogicalSize::new(WINDOW_WIDTH, WINDOW_HEIGHT));

        let window = Arc::new(event_loop.create_window(window_attributes)?);
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

    /// Build the chart with multi-font configuration.
    fn build_chart(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let _context = self.context.as_ref().ok_or("No context")?;
        let render_context = Arc::new(pollster::block_on(gup::RenderContext::new())?);

        let sel = gup::selection::Selection::<DataPoint, gup::Circle>::new(vec![], render_context)?;

        // Configure chart with multi-font styles
        let config = ChartConfig {
            width: WINDOW_WIDTH as f32,
            height: WINDOW_HEIGHT as f32,
            ..ChartConfig::default()
        }
        .with_title("Revenue by Quarter (Multi-Font Demo)")
        .with_title_style(
            TextStyle::new(20.0)
                .bold()
                .with_font_family("DejaVu Serif")
                .with_rgba(0.15, 0.15, 0.15, 1.0),
        )
        .with_label_style(
            TextStyle::new(14.0)
                .with_font_family("DejaVu Sans")
                .with_rgba(0.3, 0.3, 0.3, 1.0),
        );

        // Build chart with custom axis configurations
        let bottom_axis = LinearAxis::new(
            AxisPosition::Bottom,
            AxisConfiguration::default()
                .with_color([0.2, 0.4, 0.8, 1.0])
                .with_line_width(2.0),
        );
        let left_axis = LinearAxis::new(
            AxisPosition::Left,
            AxisConfiguration::default()
                .with_color([0.8, 0.2, 0.2, 1.0])
                .with_line_width(2.0),
        );

        let chart = ComposedChart::new(sel, config)
            .with_bottom_axis(Box::new(bottom_axis))
            .with_left_axis(Box::new(left_axis));

        self.chart = Some(chart);
        println!("✅ Chart built with multi-font configuration");
        Ok(())
    }

    fn initialize_text_rendering(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(context) = &self.context
            && self.text_renderer.is_none()
        {
            self.text_renderer = Some(TextRenderer::new(&context.device)?);
            self.font_manager = Some(FontAtlasManager::new(FontDatabase::new(), 16.0));
            self.layout_engine = Some(TextLayoutEngine::new());
            println!("✅ Text rendering initialized with FontAtlasManager");
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

        if let Some(context) = self.context.take() {
            let mut ctx = Arc::try_unwrap(context).map_err(|_| "Failed to unwrap context")?;

            match ctx.begin_frame_for_surface(surface_id) {
                Ok(mut frame) => {
                    let clear_color = wgpu::Color {
                        r: 0.97,
                        g: 0.97,
                        b: 0.98,
                        a: 1.0,
                    };

                    let device = frame.device_arc();
                    let queue = frame.queue_arc();
                    let (w, h) = (WINDOW_WIDTH as f32, WINDOW_HEIGHT as f32);

                    // --- Phase 1: Generate axis geometry and queue text ---
                    let chart = self.chart.as_ref().unwrap();
                    let (vertices, _labels) = chart.generate_axis_geometry();

                    // Queue text using the chart builder's multi-font API
                    if let (Some(text_renderer), Some(font_manager), Some(layout_engine)) = (
                        &mut self.text_renderer,
                        &mut self.font_manager,
                        &mut self.layout_engine,
                    ) {
                        text_renderer.begin_frame();

                        let mut positioner = LabelPositioner::new();
                        let constraints = LabelConstraints::axis_labels();

                        if let Err(e) = chart.queue_chart_text_resolved(
                            &frame,
                            text_renderer,
                            font_manager,
                            layout_engine,
                            &mut positioner,
                            &constraints,
                        ) {
                            eprintln!("⚠️ Failed to queue chart text: {e}");
                        }

                        // Print atlas info once
                        static PRINTED: std::sync::atomic::AtomicBool =
                            std::sync::atomic::AtomicBool::new(false);
                        if !PRINTED.swap(true, std::sync::atomic::Ordering::Relaxed) {
                            println!("📊 Font atlases loaded: {}", font_manager.atlas_count());
                            for (key, atlas) in font_manager.iter() {
                                println!(
                                    "  • {key}: {} glyphs, fallback={}",
                                    atlas.glyph_count(),
                                    atlas.is_fallback_font(),
                                );
                            }
                        }
                    }

                    // --- Phase 2: Render pass (axes + text) ---
                    if !vertices.is_empty() {
                        let vertex_buffer =
                            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                label: Some("Axis Vertex Buffer"),
                                contents: bytemuck::cast_slice(&vertices),
                                usage: wgpu::BufferUsages::VERTEX,
                            });

                        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                            label: Some("axis_shader"),
                            source: wgpu::ShaderSource::Wgsl(AXIS_SHADER_SRC.into()),
                        });

                        let pipeline_layout =
                            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                                label: Some("axis_pipeline_layout"),
                                bind_group_layouts: &[],
                                push_constant_ranges: &[],
                            });

                        let render_pipeline =
                            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                                label: Some("axis_pipeline"),
                                layout: Some(&pipeline_layout),
                                vertex: wgpu::VertexState {
                                    module: &shader,
                                    entry_point: Some("vs_main"),
                                    buffers: &[wgpu::VertexBufferLayout {
                                        array_stride: std::mem::size_of::<Vertex>()
                                            as wgpu::BufferAddress,
                                        step_mode: wgpu::VertexStepMode::Vertex,
                                        attributes: &[
                                            wgpu::VertexAttribute {
                                                offset: 0,
                                                shader_location: 0,
                                                format: wgpu::VertexFormat::Float32x2,
                                            },
                                            wgpu::VertexAttribute {
                                                offset: std::mem::size_of::<[f32; 2]>()
                                                    as wgpu::BufferAddress,
                                                shader_location: 1,
                                                format: wgpu::VertexFormat::Float32x4,
                                            },
                                        ],
                                    }],
                                    compilation_options: Default::default(),
                                },
                                fragment: Some(wgpu::FragmentState {
                                    module: &shader,
                                    entry_point: Some("fs_main"),
                                    targets: &[Some(wgpu::ColorTargetState {
                                        format: wgpu::TextureFormat::Bgra8UnormSrgb,
                                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                                        write_mask: wgpu::ColorWrites::ALL,
                                    })],
                                    compilation_options: Default::default(),
                                }),
                                primitive: wgpu::PrimitiveState {
                                    topology: wgpu::PrimitiveTopology::LineList,
                                    ..Default::default()
                                },
                                depth_stencil: None,
                                multisample: wgpu::MultisampleState::default(),
                                multiview: None,
                                cache: None,
                            });

                        let mut render_pass = frame.render_pass(Some(clear_color));

                        // Draw axis lines
                        render_pass.set_pipeline(&render_pipeline);
                        render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                        render_pass.draw(0..vertices.len() as u32, 0..1);

                        // Draw text (multi-font)
                        if let (Some(text_renderer), Some(font_manager)) =
                            (&mut self.text_renderer, &self.font_manager)
                            && let Err(e) = text_renderer.render_queued_text_multi(
                                &mut render_pass,
                                &device,
                                &queue,
                                font_manager,
                                w,
                                h,
                            ) {
                                eprintln!("⚠️ Failed to render text: {e}");
                            }
                    } else {
                        // No axis geometry, just render text
                        let mut render_pass = frame.render_pass(Some(clear_color));
                        if let (Some(text_renderer), Some(font_manager)) =
                            (&mut self.text_renderer, &self.font_manager)
                        {
                            let _ = text_renderer.render_queued_text_multi(
                                &mut render_pass,
                                &device,
                                &queue,
                                font_manager,
                                w,
                                h,
                            );
                        }
                    }

                    frame.finish()?;
                }
                Err(e) => eprintln!("❌ Frame error: {e}"),
            }

            self.context = Some(Arc::new(ctx));
        }
        Ok(())
    }
}

const AXIS_SHADER_SRC: &str = r#"
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = vec4<f32>(position, 0.0, 1.0);
    out.color = color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}
"#;

impl ApplicationHandler for MultiFontChartApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        pollster::block_on(async {
            if let Err(e) = self.create_context().await {
                eprintln!("❌ Context creation failed: {e}");
                event_loop.exit();
                return;
            }

            if let Err(e) = self.create_window(event_loop) {
                eprintln!("❌ Window creation failed: {e}");
                event_loop.exit();
                return;
            }

            println!("✅ Multi-font chart demo ready — press ESC to exit");
        });
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
                if let Some(surface_id) = self.surface_id
                    && let Some(ctx) = self.context.take()
                {
                    let mut context_mut = Arc::try_unwrap(ctx).unwrap_or_else(|arc| {
                        panic!("Failed to unwrap context: {} refs", Arc::strong_count(&arc))
                    });
                    let _ = context_mut
                        .resize_surface(surface_id, PhysicalSize::new(size.width, size.height));
                    self.context = Some(Arc::new(context_mut));
                }
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(KeyCode::Escape),
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } => event_loop.exit(),
            WindowEvent::RedrawRequested => {
                if let Err(e) = self.render_frame() {
                    eprintln!("❌ Render error: {e}");
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("🔤 Gup Multi-Font Chart Builder Demo");
    println!("=====================================");
    println!();
    println!("Demonstrates chart builder multi-font integration:");
    println!("  • Title: \"DejaVu Serif\" (bold, dark)");
    println!("  • Axis labels: \"DejaVu Sans\" (regular, gray)");
    println!("  • Automatic font atlas management via FontAtlasManager");
    println!("  • Label collision detection included");
    println!();
    println!("Controls: ESC to exit");
    println!();

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = MultiFontChartApp::new();
    event_loop.run_app(&mut app)?;

    Ok(())
}

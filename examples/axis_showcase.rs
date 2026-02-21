// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Axis System Visual Showcase Example
//!
//! This example creates a visual demonstration of the axis system infrastructure, showing:
//! - Automatic axis generation for scatter plots
//! - Custom axis configuration and styling
//! - Multiple axis positions (top, bottom, left, right)
//! - Formatted numeric labels at each tick mark
//! - Text rendering integrated in a single GPU render pass
//! - Real-time interactive axis rendering

use gup::{
    GupContext, PhysicalSize, SurfaceId,
    axis::{
        Axis, AxisBounds, AxisConfiguration, AxisPosition, AxisRenderer as GupAxisRenderer,
        LinearAxis,
    },
    label::{AxisInfo, LabelConstraints, LabelPositioner},
    render::Vertex,
    shader_function::Vec2,
    text::{FontAtlas, TextAnchor, TextLayoutEngine, TextRenderConfig, TextRenderer, TextStyle},
};
use std::sync::Arc;
use wgpu::{Color, util::DeviceExt};
use winit::{
    application::ApplicationHandler,
    event::{ElementState, KeyEvent, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowAttributes, WindowId},
};

// Sample data structure for demonstration (kept for future enhancements)
#[allow(dead_code)]
#[derive(Debug, Clone)]
struct SalesData {
    quarter: f32,
    revenue: f32,
    profit: f32,
    region: String,
}

/// Visual axis renderer that demonstrates the axis system
struct AxisRenderer {
    axes: Vec<Box<dyn Axis>>,
    #[allow(dead_code)] // Reserved for future use in coordinate transformations
    chart_bounds: AxisBounds,
    background_color: [f32; 4],
    gpu_renderer: GupAxisRenderer,
    // Text rendering components for axis labels
    text_renderer: Option<TextRenderer>,
    font_atlas: Option<FontAtlas>,
    layout_engine: Option<TextLayoutEngine>,
}

impl AxisRenderer {
    #[allow(clippy::vec_init_then_push)] // More readable for demonstration purposes
    fn new() -> Self {
        // Create demonstration axes with different configurations
        let mut axes: Vec<Box<dyn Axis>> = Vec::new();

        // Bottom axis (X-axis) - Blue theme
        axes.push(Box::new(LinearAxis::new(
            AxisPosition::Bottom,
            AxisConfiguration::default()
                .with_color([0.2, 0.4, 0.8, 1.0])  // Blue
                .with_line_width(2.0)
                .with_tick_lengths(10.0, 5.0),
        )));

        // Left axis (Y-axis) - Red theme
        axes.push(Box::new(LinearAxis::new(
            AxisPosition::Left,
            AxisConfiguration::default()
                .with_color([0.8, 0.2, 0.2, 1.0])  // Red
                .with_line_width(2.0)
                .with_tick_lengths(10.0, 5.0),
        )));

        // Top axis - Green theme (optional secondary X-axis)
        axes.push(Box::new(LinearAxis::new(
            AxisPosition::Top,
            AxisConfiguration::default()
                .with_color([0.2, 0.8, 0.2, 1.0])  // Green
                .with_line_width(1.5)
                .with_tick_lengths(8.0, 4.0)
                .without_minor_ticks(),
        )));

        // Right axis - Purple theme (optional secondary Y-axis)
        axes.push(Box::new(LinearAxis::new(
            AxisPosition::Right,
            AxisConfiguration::default()
                .with_color([0.8, 0.2, 0.8, 1.0])  // Purple
                .with_line_width(1.5)
                .with_tick_lengths(8.0, 4.0)
                .without_minor_ticks(),
        )));

        // Define chart area bounds (centered in the screen with margins)
        let chart_bounds = AxisBounds::new(
            Vec2::new(-0.6, -0.6), // Bottom-left of chart area
            Vec2::new(0.6, 0.6),   // Top-right of chart area
            50.0,                  // Margin for axis labels and ticks
        );

        Self {
            axes,
            chart_bounds,
            background_color: [0.95, 0.95, 0.95, 1.0], // Light gray
            gpu_renderer: GupAxisRenderer::new(),
            text_renderer: None,
            font_atlas: None,
            layout_engine: None,
        }
    }

    fn render(&mut self, frame: &mut gup::RenderFrame) -> Result<(), Box<dyn std::error::Error>> {
        let viewport_size = (900.0_f32, 700.0_f32);

        // Clear background
        let clear_color = Color {
            r: self.background_color[0] as f64,
            g: self.background_color[1] as f64,
            b: self.background_color[2] as f64,
            a: self.background_color[3] as f64,
        };

        // Initialize text rendering components if needed
        if self.text_renderer.is_none() {
            self.text_renderer = Some(TextRenderer::new(frame.device())?);
        }
        if self.font_atlas.is_none() {
            self.font_atlas = Some(FontAtlas::new(frame.device(), frame.queue(), 14.0)?);
        }
        if self.layout_engine.is_none() {
            self.layout_engine = Some(TextLayoutEngine::new());
        }

        // Create vertices for axis lines and ticks
        let mut vertices = Vec::new();
        for axis in &self.axes {
            let axis_vertices = self.generate_axis_vertices(axis.as_ref())?;
            vertices.extend(axis_vertices);
        }

        // Collect label data per axis and resolve collisions
        struct LabelInfo {
            text: String,
            screen_position: Vec2,
            anchor: TextAnchor,
            color: [f32; 4],
        }

        let mut all_labels = Vec::new();
        let mut positioner = LabelPositioner::new();
        let constraints = LabelConstraints::axis_labels();

        for axis in &self.axes {
            let config = axis.configuration();
            let position = axis.position();
            let bounds = Self::axis_bounds_for_position(position);

            let labels = self.gpu_renderer.generate_label_data(
                &bounds,
                config,
                position,
                None,
                viewport_size,
                None,
            );

            // Run collision resolution for this axis's labels
            let axis_info = AxisInfo::from_bounds(&bounds, position);
            let layout = positioner
                .resolve_labels(&labels, &axis_info, &constraints)
                .unwrap_or_else(|e| {
                    eprintln!("Label collision resolution failed: {e}");
                    gup::label::LabelLayout {
                        positions: Vec::new(),
                        hidden_labels: Vec::new(),
                        margin_requirements: gup::label::Margins::default(),
                        rotated: false,
                    }
                });

            for lp in layout.positions {
                all_labels.push(LabelInfo {
                    text: lp.text,
                    screen_position: lp.position,
                    anchor: lp.anchor,
                    color: config.line_color,
                });
            }
        }

        // --- Phase 1: Queue text labels BEFORE creating the render pass ---
        let text_renderer = self.text_renderer.as_mut().unwrap();
        text_renderer.begin_frame();

        let font_atlas = self.font_atlas.as_mut().unwrap();
        let layout_engine = self.layout_engine.as_mut().unwrap();

        for label in &all_labels {
            let style = TextStyle::new(14.0)
                .with_rgba(
                    label.color[0],
                    label.color[1],
                    label.color[2],
                    label.color[3],
                )
                .with_anchor(label.anchor);

            let mut text_config = TextRenderConfig {
                text: &label.text,
                position: label.screen_position,
                style: &style,
                font_atlas,
                layout_engine,
                screen_width: viewport_size.0,
                screen_height: viewport_size.1,
            };

            if let Err(e) = text_renderer.queue_text(frame, &mut text_config) {
                eprintln!("Failed to queue label '{}': {}", label.text, e);
            }
        }

        if vertices.is_empty() {
            // Just clear the screen and render any queued text
            let device = frame.device_arc();
            let queue = frame.queue_arc();
            let font_atlas = self.font_atlas.as_ref().unwrap();
            let text_renderer = self.text_renderer.as_mut().unwrap();
            let mut render_pass = frame.render_pass(Some(clear_color));
            let _ = text_renderer.render_queued_text(
                &mut render_pass,
                &device,
                &queue,
                font_atlas,
                viewport_size.0,
                viewport_size.1,
            );
            return Ok(());
        }

        // Create vertex buffer for axis lines/ticks
        let vertex_buffer = frame
            .device()
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Axis Vertex Buffer"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });

        // Create simple shader for axis line rendering
        let shader = frame
            .device()
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("axis_shader"),
                source: wgpu::ShaderSource::Wgsl(
                    r#"
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
                "#
                    .into(),
                ),
            });

        // Create render pipeline for LineList topology
        let render_pipeline_layout =
            frame
                .device()
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("axis_pipeline_layout"),
                    bind_group_layouts: &[],
                    push_constant_ranges: &[],
                });

        let render_pipeline =
            frame
                .device()
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("axis_pipeline"),
                    layout: Some(&render_pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &shader,
                        entry_point: Some("vs_main"),
                        buffers: &[wgpu::VertexBufferLayout {
                            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
                            step_mode: wgpu::VertexStepMode::Vertex,
                            attributes: &[
                                wgpu::VertexAttribute {
                                    offset: 0,
                                    shader_location: 0,
                                    format: wgpu::VertexFormat::Float32x2, // position
                                },
                                wgpu::VertexAttribute {
                                    offset: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                                    shader_location: 1,
                                    format: wgpu::VertexFormat::Float32x4, // color
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
                        strip_index_format: None,
                        front_face: wgpu::FrontFace::Ccw,
                        cull_mode: None,
                        polygon_mode: wgpu::PolygonMode::Fill,
                        unclipped_depth: false,
                        conservative: false,
                    },
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState {
                        count: 1,
                        mask: !0,
                        alpha_to_coverage_enabled: false,
                    },
                    multiview: None,
                    cache: None,
                });

        // --- Phase 2: Single render pass for lines + ticks + text labels ---
        let device = frame.device_arc();
        let queue = frame.queue_arc();
        let font_atlas = self.font_atlas.as_ref().unwrap();
        let text_renderer = self.text_renderer.as_mut().unwrap();
        {
            let mut render_pass = frame.render_pass(Some(clear_color));

            // Draw axis lines and ticks
            render_pass.set_pipeline(&render_pipeline);
            render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            render_pass.draw(0..vertices.len() as u32, 0..1);

            // Draw queued text labels in the same render pass
            if let Err(e) = text_renderer.render_queued_text(
                &mut render_pass,
                &device,
                &queue,
                font_atlas,
                viewport_size.0,
                viewport_size.1,
            ) {
                eprintln!("Failed to render axis labels: {}", e);
            }
        }

        Ok(())
    }

    /// Compute axis bounds for a given position (shared by vertex and label generation).
    fn axis_bounds_for_position(position: AxisPosition) -> AxisBounds {
        let (start, end) = match position {
            AxisPosition::Bottom => (Vec2::new(-0.6, -0.6), Vec2::new(0.6, -0.6)),
            AxisPosition::Top => (Vec2::new(-0.6, 0.6), Vec2::new(0.6, 0.6)),
            AxisPosition::Left => (Vec2::new(-0.6, -0.6), Vec2::new(-0.6, 0.6)),
            AxisPosition::Right => (Vec2::new(0.6, -0.6), Vec2::new(0.6, 0.6)),
        };
        AxisBounds::new(start, end, 50.0)
    }

    /// Generate vertices for a single axis (line and ticks) using the library's AxisRenderer.
    fn generate_axis_vertices(
        &self,
        axis: &dyn Axis,
    ) -> Result<Vec<Vertex>, Box<dyn std::error::Error>> {
        let config = axis.configuration();
        let position = axis.position();
        let bounds = Self::axis_bounds_for_position(position);

        // Delegate to the library's AxisRenderer for vertex generation
        let vertices = self.gpu_renderer.generate_axis_vertices(
            &bounds,
            config,
            position,
            None,           // No scale — uses default tick positions
            (900.0, 700.0), // Match the window size
        );

        Ok(vertices)
    }
}

/// Main application for the visual axis showcase
struct AxisShowcaseApp {
    context: Option<Arc<GupContext>>,
    window: Option<Arc<Window>>,
    surface_id: Option<SurfaceId>,
    renderer: Option<AxisRenderer>,
    current_demo: usize,
}

impl AxisShowcaseApp {
    fn new() -> Self {
        Self {
            context: None,
            window: None,
            surface_id: None,
            renderer: None,
            current_demo: 0,
        }
    }

    async fn create_context(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.context.is_none() {
            println!("🔧 Creating GPU context...");
            let context = GupContext::headless().await?;
            self.context = Some(context);
            println!("✅ GPU context created");
        }
        Ok(())
    }

    fn create_window(
        &mut self,
        event_loop: &ActiveEventLoop,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let window_attributes = WindowAttributes::default()
            .with_title("Gup Axis System Showcase - Interactive Visual Demo")
            .with_inner_size(winit::dpi::LogicalSize::new(900, 700));

        let window = Arc::new(event_loop.create_window(window_attributes)?);
        let surface_id = SurfaceId::new();

        println!("🖼️ Creating window...");

        // Add surface to context
        if let Some(context) = self.context.take() {
            let mut ctx = Arc::try_unwrap(context).map_err(|_| "Failed to get mutable context")?;
            ctx.add_surface(surface_id, Arc::clone(&window))?;
            self.context = Some(Arc::new(ctx));
            println!("✅ Surface {surface_id} added to context");
        }

        self.window = Some(window);
        self.surface_id = Some(surface_id);

        Ok(())
    }

    fn initialize_renderer(&mut self) {
        self.renderer = Some(AxisRenderer::new());
        println!("✅ Axis renderer initialized");
    }

    fn render_frame(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.renderer.is_none() {
            self.initialize_renderer();
        }

        if let Some(surface_id) = self.surface_id
            && let Some(context) = self.context.take()
        {
            let mut ctx = Arc::try_unwrap(context).map_err(|_| "Failed to get mutable context")?;

            match ctx.begin_frame_for_surface(surface_id) {
                Ok(mut frame) => {
                    if let Some(renderer) = &mut self.renderer
                        && let Err(e) = renderer.render(&mut frame)
                    {
                        eprintln!("❌ Failed to render axes: {e}");
                    }
                    frame.finish()?;
                }
                Err(e) => {
                    eprintln!("❌ Failed to render frame: {e}");
                }
            }

            self.context = Some(Arc::new(ctx));
        }
        Ok(())
    }
}

impl ApplicationHandler for AxisShowcaseApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        pollster::block_on(async {
            if let Err(e) = self.create_context().await {
                eprintln!("❌ Failed to create context: {e}");
                event_loop.exit();
                return;
            }

            if let Err(e) = self.create_window(event_loop) {
                eprintln!("❌ Failed to create window: {e}");
                event_loop.exit();
                return;
            }

            println!("✅ Visual axis showcase window created!");
            println!("🎨 Demonstrating axis system capabilities...");
            println!();
            println!("Visible Features:");
            println!("• Bottom axis (X): Blue, 2px line, major ticks + labels");
            println!("• Left axis (Y): Red, 2px line, major ticks + labels");
            println!("• Top axis: Green, 1.5px line, ticks + labels (no minor ticks)");
            println!("• Right axis: Purple, 1.5px line, ticks + labels (no minor ticks)");
            println!("• Formatted numeric labels positioned at each tick mark");
            println!();
            println!("Controls:");
            println!("  [ESC] - Exit");
            println!("  [SPACE] - Cycle through demo configurations (planned)");
        });
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                println!("👋 Closing axis showcase");
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                if let Some(surface_id) = self.surface_id
                    && let Some(ctx) = self.context.take()
                {
                    let mut context_mut = Arc::try_unwrap(ctx).unwrap_or_else(|arc| {
                        panic!(
                            "Failed to get mutable context: {} references",
                            Arc::strong_count(&arc)
                        )
                    });

                    if let Err(e) = context_mut
                        .resize_surface(surface_id, PhysicalSize::new(size.width, size.height))
                    {
                        eprintln!("❌ Failed to resize surface: {e}");
                    }

                    self.context = Some(Arc::new(context_mut));
                }
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        state: ElementState::Pressed,
                        physical_key: PhysicalKey::Code(key_code),
                        ..
                    },
                ..
            } => {
                match key_code {
                    KeyCode::Escape => {
                        println!("👋 Escape pressed, closing showcase");
                        event_loop.exit();
                    }
                    KeyCode::Space => {
                        self.current_demo = (self.current_demo + 1) % 4;
                        println!(
                            "🔄 Switching to demo configuration {}",
                            self.current_demo + 1
                        );
                        // Future: Update renderer configuration based on current_demo
                    }
                    _ => {}
                }
            }
            WindowEvent::RedrawRequested => {
                if let Err(e) = self.render_frame() {
                    eprintln!("❌ Failed to render frame: {e}");
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

    println!("🚀 Gup Axis System Visual Showcase");
    println!("===================================");
    println!();
    println!("This interactive demo showcases the core axis system infrastructure:");
    println!("• GPU-accelerated axis line rendering using Line marks");
    println!("• Multiple axis positions with custom styling");
    println!("• Configurable tick marks and line properties");
    println!("• Formatted numeric labels at tick positions");
    println!("• Text rendering integrated in the same render pass");
    println!("• Real-time axis rendering performance");
    println!();
    println!("The demo shows four axes with different configurations:");
    println!("• Bottom (Blue): Primary X-axis with major ticks and labels");
    println!("• Left (Red): Primary Y-axis with major ticks and labels");
    println!("• Top (Green): Secondary X-axis with ticks and labels (no minor ticks)");
    println!("• Right (Purple): Secondary Y-axis with ticks and labels (no minor ticks)");
    println!();
    println!("Controls:");
    println!("• ESC - Exit the showcase");
    println!("• SPACE - Cycle through demo configurations (planned)");
    println!();

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = AxisShowcaseApp::new();
    event_loop.run_app(&mut app)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_axis_renderer_creation() {
        let renderer = AxisRenderer::new();
        assert_eq!(renderer.axes.len(), 4); // Should have 4 axes (top, bottom, left, right)
        assert_eq!(renderer.background_color, [0.95, 0.95, 0.95, 1.0]);
        // Text rendering components are lazily initialized on first render
        assert!(renderer.text_renderer.is_none());
        assert!(renderer.font_atlas.is_none());
        assert!(renderer.layout_engine.is_none());
    }

    #[test]
    fn test_axis_bounds_for_position() {
        let bottom = AxisRenderer::axis_bounds_for_position(AxisPosition::Bottom);
        assert_eq!(bottom.start.y, -0.6);
        assert_eq!(bottom.end.y, -0.6);

        let left = AxisRenderer::axis_bounds_for_position(AxisPosition::Left);
        assert_eq!(left.start.x, -0.6);
        assert_eq!(left.end.x, -0.6);
    }

    #[test]
    fn test_axis_showcase_app_creation() {
        let app = AxisShowcaseApp::new();
        assert!(app.context.is_none());
        assert!(app.window.is_none());
        assert!(app.renderer.is_none());
        assert_eq!(app.current_demo, 0);
    }
}

// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Axis System Visual Showcase Example
//!
//! This example creates a visual demonstration of the axis system infrastructure, showing:
//! - Automatic axis generation for scatter plots
//! - Custom axis configuration and styling
//! - Multiple axis positions (top, bottom, left, right)
//! - Real-time interactive axis rendering

use gup::{
    GupContext, PhysicalSize, SurfaceId,
    axis::{Axis, AxisBounds, AxisConfiguration, AxisPosition, LinearAxis},
    render::Vertex,
    shader_function::Vec2,
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
            Vec2 { x: -0.6, y: -0.6 }, // Bottom-left of chart area
            Vec2 { x: 0.6, y: 0.6 },   // Top-right of chart area
            50.0,                      // Margin for axis labels and ticks
        );

        Self {
            axes,
            chart_bounds,
            background_color: [0.95, 0.95, 0.95, 1.0], // Light gray
        }
    }

    fn render(&self, frame: &mut gup::RenderFrame) -> Result<(), Box<dyn std::error::Error>> {
        // Clear background
        let clear_color = Color {
            r: self.background_color[0] as f64,
            g: self.background_color[1] as f64,
            b: self.background_color[2] as f64,
            a: self.background_color[3] as f64,
        };

        // Create vertices for axis lines and ticks
        let mut vertices = Vec::new();

        // For each axis, generate the appropriate line and tick vertices
        for axis in &self.axes {
            let axis_vertices = self.generate_axis_vertices(axis.as_ref())?;
            vertices.extend(axis_vertices);
        }

        if vertices.is_empty() {
            // Just clear the screen
            let _render_pass = frame.render_pass(Some(clear_color));
            return Ok(());
        }

        // Create vertex buffer
        let vertex_buffer = frame
            .device()
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Axis Vertex Buffer"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });

        // Create simple shader for axis rendering
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

        // Create render pipeline
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

        // Render the axes
        {
            let mut render_pass = frame.render_pass(Some(clear_color));
            render_pass.set_pipeline(&render_pipeline);
            render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            render_pass.draw(0..vertices.len() as u32, 0..1);
        }

        Ok(())
    }

    /// Generate vertices for a single axis (line and ticks)
    fn generate_axis_vertices(
        &self,
        axis: &dyn Axis,
    ) -> Result<Vec<Vertex>, Box<dyn std::error::Error>> {
        let mut vertices = Vec::new();
        let config = axis.configuration();
        let position = axis.position();

        // Define axis line endpoints based on position
        let (start, end) = match position {
            AxisPosition::Bottom => (
                Vec2 { x: -0.6, y: -0.6 }, // Bottom-left
                Vec2 { x: 0.6, y: -0.6 },  // Bottom-right
            ),
            AxisPosition::Top => (
                Vec2 { x: -0.6, y: 0.6 }, // Top-left
                Vec2 { x: 0.6, y: 0.6 },  // Top-right
            ),
            AxisPosition::Left => (
                Vec2 { x: -0.6, y: -0.6 }, // Bottom-left
                Vec2 { x: -0.6, y: 0.6 },  // Top-left
            ),
            AxisPosition::Right => (
                Vec2 { x: 0.6, y: -0.6 }, // Bottom-right
                Vec2 { x: 0.6, y: 0.6 },  // Top-right
            ),
        };

        // Main axis line
        if config.show_line {
            vertices.push(Vertex {
                position: [start.x, start.y],
                color: config.line_color,
            });
            vertices.push(Vertex {
                position: [end.x, end.y],
                color: config.line_color,
            });
        }

        // Generate ticks
        if config.show_major_ticks {
            let tick_positions = axis.get_tick_positions(None);
            let tick_length = config.major_tick_length / 500.0; // Scale to screen coordinates

            for &t in &tick_positions {
                if (0.0..=1.0).contains(&t) {
                    // Only show ticks within [0,1] range
                    let (tick_start, tick_end) = match position {
                        AxisPosition::Bottom => {
                            let x = start.x + t * (end.x - start.x);
                            (
                                Vec2 { x, y: start.y },
                                Vec2 {
                                    x,
                                    y: start.y - tick_length,
                                },
                            )
                        }
                        AxisPosition::Top => {
                            let x = start.x + t * (end.x - start.x);
                            (
                                Vec2 { x, y: start.y },
                                Vec2 {
                                    x,
                                    y: start.y + tick_length,
                                },
                            )
                        }
                        AxisPosition::Left => {
                            let y = start.y + t * (end.y - start.y);
                            (
                                Vec2 { x: start.x, y },
                                Vec2 {
                                    x: start.x - tick_length,
                                    y,
                                },
                            )
                        }
                        AxisPosition::Right => {
                            let y = start.y + t * (end.y - start.y);
                            (
                                Vec2 { x: start.x, y },
                                Vec2 {
                                    x: start.x + tick_length,
                                    y,
                                },
                            )
                        }
                    };

                    vertices.push(Vertex {
                        position: [tick_start.x, tick_start.y],
                        color: config.line_color,
                    });
                    vertices.push(Vertex {
                        position: [tick_end.x, tick_end.y],
                        color: config.line_color,
                    });
                }
            }
        }

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

        if let Some(surface_id) = self.surface_id {
            if let Some(context) = self.context.take() {
                let mut ctx =
                    Arc::try_unwrap(context).map_err(|_| "Failed to get mutable context")?;

                match ctx.begin_frame_for_surface(surface_id) {
                    Ok(mut frame) => {
                        if let Some(renderer) = &self.renderer {
                            if let Err(e) = renderer.render(&mut frame) {
                                eprintln!("❌ Failed to render axes: {e}");
                            }
                        }
                        frame.finish()?;
                    }
                    Err(e) => {
                        eprintln!("❌ Failed to render frame: {e}");
                    }
                }

                self.context = Some(Arc::new(ctx));
            }
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
            println!("• Bottom axis (X): Blue, 2px line, major ticks");
            println!("• Left axis (Y): Red, 2px line, major ticks");
            println!("• Top axis: Green, 1.5px line, no minor ticks");
            println!("• Right axis: Purple, 1.5px line, no minor ticks");
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
                if let Some(surface_id) = self.surface_id {
                    if let Some(ctx) = self.context.take() {
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
    println!("• Real-time axis rendering performance");
    println!();
    println!("The demo shows four axes with different configurations:");
    println!("• Bottom (Blue): Primary X-axis with major ticks");
    println!("• Left (Red): Primary Y-axis with major ticks");
    println!("• Top (Green): Secondary X-axis without minor ticks");
    println!("• Right (Purple): Secondary Y-axis without minor ticks");
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

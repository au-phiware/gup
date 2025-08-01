// Gup - GPU-Accelerated Data Visualization Library
// Copyright (C) 2025 Corin Lawson <corin@phiware.com.au>
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

//! Visual Blend Mode Demonstration
//!
//! This example creates a windowed application showing all 4 blend modes side-by-side
//! with interactive controls. This addresses the requirements from GUP-043.
//!
//! Features:
//! - Visual comparison of None, AlphaBlending, Additive, and Multiply modes
//! - Interactive controls to cycle blend modes
//! - Real-time alpha adjustment
//! - Performance monitoring display
//! - Cross-platform window handling

use gup::{BlendMode, GupContext, PhysicalSize, SurfaceId, Vertex};
use std::sync::Arc;
use wgpu::{Buffer, BufferUsages, Color, util::DeviceExt};
use winit::{
    application::ApplicationHandler,
    event::{ElementState, KeyEvent, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowAttributes, WindowId},
};

/// Global alpha uniform for blending operations
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct GlobalAlphaUniform {
    alpha: f32,
    _padding: [f32; 3], // Ensure 16-byte alignment
}

/// Manages the visual blend mode demonstration
struct BlendDemoApp {
    context: Option<Arc<GupContext>>,
    window: Option<Arc<Window>>,
    surface_id: Option<SurfaceId>,

    // Demo state
    current_mode: BlendMode,
    global_alpha: f32,
    demo_renderer: BlendDemoRenderer,

    // Performance tracking
    frame_count: u64,
    last_fps_update: std::time::Instant,
    current_fps: f32,
}

/// Renders the visual blend mode demonstration content
struct BlendDemoRenderer {
    // GPU buffers for rendering
    vertex_buffer: Option<Buffer>,
    num_vertices: u32,
    // Vertices for rendering
    vertices: Vec<Vertex>,
}

/// A colored quad for blend mode demonstration
#[derive(Debug, Clone)]
struct DemoQuad {
    color: [f32; 4],
    position: [f32; 2],
    size: f32,
}

impl DemoQuad {
    fn new(color: [f32; 4], position: [f32; 2], size: f32) -> Self {
        Self {
            color,
            position,
            size,
        }
    }

    /// Generate vertices for rendering this quad as two triangles
    fn generate_vertices(&self) -> Vec<Vertex> {
        let half_size = self.size / 2.0;
        let [x, y] = self.position;

        vec![
            // Triangle 1
            Vertex {
                position: [x - half_size, y - half_size],
                color: self.color,
            },
            Vertex {
                position: [x + half_size, y - half_size],
                color: self.color,
            },
            Vertex {
                position: [x - half_size, y + half_size],
                color: self.color,
            },
            // Triangle 2
            Vertex {
                position: [x + half_size, y - half_size],
                color: self.color,
            },
            Vertex {
                position: [x + half_size, y + half_size],
                color: self.color,
            },
            Vertex {
                position: [x - half_size, y + half_size],
                color: self.color,
            },
        ]
    }
}

impl BlendDemoRenderer {
    fn new() -> Self {
        // Create overlapping rectangles to demonstrate blend modes
        let quads = vec![
            // Background rectangle - red
            DemoQuad::new([0.8, 0.2, 0.2, 0.8], [-0.2, -0.2], 0.8),
            // Foreground rectangle - blue, offset to create overlap
            DemoQuad::new([0.2, 0.2, 0.8, 0.8], [0.2, 0.2], 0.8),
        ];

        // Generate vertices immediately
        let mut all_vertices = Vec::new();
        for quad in &quads {
            all_vertices.extend(quad.generate_vertices());
        }
        let num_vertices = all_vertices.len() as u32;

        Self {
            vertex_buffer: None,
            num_vertices,
            vertices: all_vertices,
        }
    }

    /// Create or update the vertex buffer with current quad data
    fn update_vertex_buffer(
        &mut self,
        device: &wgpu::Device,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Create vertex buffer using the device
        self.vertex_buffer = Some(
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Quad Vertex Buffer"),
                contents: bytemuck::cast_slice(&self.vertices),
                usage: BufferUsages::VERTEX,
            }),
        );

        Ok(())
    }

    /// Render the demonstration using GupContext
    fn render(
        &mut self,
        frame: &mut gup::RenderFrame,
        current_mode: BlendMode,
        global_alpha: f32,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Ensure vertex buffer is created
        if self.vertex_buffer.is_none() {
            self.update_vertex_buffer(frame.device())?;
        }

        // Use consistent dark background for all blend modes
        // This allows you to see how the same content blends differently
        let clear_color = Color {
            r: 0.1,
            g: 0.1,
            b: 0.1,
            a: 1.0,
        };

        // Get or create render pipeline with current blend mode
        let (pipeline, bind_group) =
            self.get_or_create_pipeline_with_alpha(frame, current_mode, global_alpha)?;

        // Create render pass and draw the quads
        {
            let mut render_pass = frame.render_pass(Some(clear_color));

            if let Some(vertex_buffer) = &self.vertex_buffer {
                render_pass.set_pipeline(&pipeline);
                render_pass.set_bind_group(0, &bind_group, &[]);
                render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                render_pass.draw(0..self.num_vertices, 0..1);
            }
        } // render_pass is dropped here, ending the pass

        Ok(())
    }

    /// Get or create a render pipeline for drawing vertices with specific blend mode and global alpha
    fn get_or_create_pipeline_with_alpha(
        &mut self,
        frame: &gup::RenderFrame,
        blend_mode: BlendMode,
        global_alpha: f32,
    ) -> Result<(wgpu::RenderPipeline, wgpu::BindGroup), Box<dyn std::error::Error>> {
        // Create global alpha uniform buffer
        let alpha_uniform = GlobalAlphaUniform {
            alpha: global_alpha,
            _padding: [0.0; 3],
        };

        let alpha_buffer = frame
            .device()
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("global_alpha_buffer"),
                contents: bytemuck::cast_slice(&[alpha_uniform]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

        // Create bind group layout for global alpha
        let bind_group_layout =
            frame
                .device()
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    }],
                    label: Some("global_alpha_bind_group_layout"),
                });

        // Create bind group
        let bind_group = frame
            .device()
            .create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: alpha_buffer.as_entire_binding(),
                }],
                label: Some("global_alpha_bind_group"),
            });

        // Create shader with global alpha support
        let shader = frame
            .device()
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("quad_shader_with_alpha"),
                source: wgpu::ShaderSource::Wgsl(
                    r#"
                struct GlobalAlpha {
                    alpha: f32,
                }

                @group(0) @binding(0)
                var<uniform> global_alpha: GlobalAlpha;

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
                    out.color = color;
                    out.clip_position = vec4<f32>(position, 0.0, 1.0);
                    return out;
                }

                @fragment
                fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
                    var color = in.color;
                    color.a *= global_alpha.alpha;
                    return color;
                }
            "#
                    .into(),
                ),
            });

        let render_pipeline_layout =
            frame
                .device()
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some(&format!("quad_pipeline_layout_{blend_mode:?}")),
                    bind_group_layouts: &[&bind_group_layout],
                    push_constant_ranges: &[],
                });

        let vertex_buffer_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        };

        // Convert BlendMode to wgpu BlendState
        let blend_state = match blend_mode {
            BlendMode::None => None,
            BlendMode::AlphaBlending => Some(wgpu::BlendState::ALPHA_BLENDING),
            BlendMode::Additive => Some(wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::Add,
                },
            }),
            BlendMode::Multiply => Some(wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::Dst,
                    dst_factor: wgpu::BlendFactor::Zero,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                    operation: wgpu::BlendOperation::Add,
                },
            }),
        };

        let render_pipeline =
            frame
                .device()
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some(&format!("quad_render_pipeline_{blend_mode:?}")),
                    layout: Some(&render_pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &shader,
                        entry_point: Some("vs_main"),
                        buffers: &[vertex_buffer_layout],
                        compilation_options: Default::default(),
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &shader,
                        entry_point: Some("fs_main"),
                        targets: &[Some(wgpu::ColorTargetState {
                            format: wgpu::TextureFormat::Bgra8UnormSrgb, // Standard surface format
                            blend: blend_state,
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                        compilation_options: Default::default(),
                    }),
                    primitive: wgpu::PrimitiveState {
                        topology: wgpu::PrimitiveTopology::TriangleList, // Changed from PointList
                        strip_index_format: None,
                        front_face: wgpu::FrontFace::Ccw,
                        cull_mode: None, // Don't cull faces for now
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

        Ok((render_pipeline, bind_group))
    }
}

impl BlendDemoApp {
    fn new() -> Self {
        Self {
            context: None,
            window: None,
            surface_id: None,
            current_mode: BlendMode::AlphaBlending,
            global_alpha: 0.75,
            demo_renderer: BlendDemoRenderer::new(),
            frame_count: 0,
            last_fps_update: std::time::Instant::now(),
            current_fps: 0.0,
        }
    }

    async fn create_context(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.context.is_none() {
            println!("Creating GPU context for blend demo...");
            let context = GupContext::headless().await?;
            self.context = Some(context);
            println!("✓ GPU context created");
        }
        Ok(())
    }

    fn create_window(
        &mut self,
        event_loop: &ActiveEventLoop,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let window_attributes = WindowAttributes::default()
            .with_title("Gup Visual Blend Mode Demo - GUP-043")
            .with_inner_size(winit::dpi::LogicalSize::new(1200, 800));

        let window = Arc::new(event_loop.create_window(window_attributes)?);
        let _window_id = window.id();
        let surface_id = SurfaceId::new();

        println!("Creating demo window...");

        // Add surface to context (following windowed_demo.rs pattern)
        if let Some(context) = self.context.take() {
            let mut ctx = Arc::try_unwrap(context).map_err(|_| "Failed to get mutable context")?;

            ctx.add_surface(surface_id, Arc::clone(&window))?;
            self.context = Some(Arc::new(ctx));

            println!("✓ Surface {surface_id} added for demo window");
        }

        self.window = Some(window);
        self.surface_id = Some(surface_id);

        Ok(())
    }

    fn handle_input(&mut self, key_code: KeyCode) {
        match key_code {
            KeyCode::Space => {
                // Cycle through blend modes
                self.current_mode = match self.current_mode {
                    BlendMode::None => BlendMode::AlphaBlending,
                    BlendMode::AlphaBlending => BlendMode::Additive,
                    BlendMode::Additive => BlendMode::Multiply,
                    BlendMode::Multiply => BlendMode::None,
                };
                println!("Switched to blend mode: {:?}", self.current_mode);
            }
            KeyCode::ArrowLeft => {
                self.global_alpha = (self.global_alpha - 0.1).max(0.0);
                println!("Global alpha: {:.2}", self.global_alpha);
            }
            KeyCode::ArrowRight => {
                self.global_alpha = (self.global_alpha + 0.1).min(1.0);
                println!("Global alpha: {:.2}", self.global_alpha);
            }
            _ => {}
        }
    }

    fn update_fps(&mut self) {
        self.frame_count += 1;
        let now = std::time::Instant::now();
        let elapsed = now.duration_since(self.last_fps_update);

        if elapsed.as_secs_f32() >= 1.0 {
            self.current_fps = self.frame_count as f32 / elapsed.as_secs_f32();
            self.frame_count = 0;
            self.last_fps_update = now;
        }
    }

    fn render_frame(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(surface_id) = self.surface_id {
            if let Some(context) = self.context.take() {
                let mut ctx =
                    Arc::try_unwrap(context).map_err(|_| "Failed to get mutable context")?;

                match ctx.begin_frame_for_surface(surface_id) {
                    Ok(mut frame) => {
                        // Render the demo content using the frame
                        self.demo_renderer.render(
                            &mut frame,
                            self.current_mode,
                            self.global_alpha,
                        )?;

                        frame.finish()?;
                        self.frame_count += 1;
                        self.update_fps();
                    }
                    Err(e) => {
                        eprintln!("Failed to render frame: {e}");
                    }
                }

                self.context = Some(Arc::new(ctx));
            }
        }
        Ok(())
    }

    fn handle_resize(
        &mut self,
        size: winit::dpi::PhysicalSize<u32>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(surface_id) = self.surface_id {
            if let Some(ctx) = self.context.take() {
                let mut context_mut =
                    Arc::try_unwrap(ctx).map_err(|_| "Failed to get mutable context")?;

                let start = std::time::Instant::now();
                context_mut
                    .resize_surface(surface_id, PhysicalSize::new(size.width, size.height))?;
                let duration = start.elapsed();

                if duration.as_millis() > 16 {
                    println!(
                        "Warning: Resize took {:.2}ms (>16ms target)",
                        duration.as_secs_f64() * 1000.0
                    );
                }

                self.context = Some(Arc::new(context_mut));
            }
        }
        Ok(())
    }

    fn print_status(&self) {
        println!("\n=== Visual Blend Demo Status ===");
        println!("Current blend mode: {:?}", self.current_mode);
        println!("Global alpha: {:.2}", self.global_alpha);
        println!("FPS: {:.1}", self.current_fps);
        println!("Controls:");
        println!("  [Space] - Cycle blend modes");
        println!("  [←] [→] - Adjust global alpha");
        println!("  [H] - Show this help");
        println!("  [Q] - Quit");
    }
}

impl ApplicationHandler for BlendDemoApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        pollster::block_on(async {
            if let Err(e) = self.create_context().await {
                eprintln!("Failed to create context: {e}");
                event_loop.exit();
                return;
            }

            if let Err(e) = self.create_window(event_loop) {
                eprintln!("Failed to create window: {e}");
                event_loop.exit();
                return;
            }

            println!("\n=== Visual Blend Mode Demo Started ===");
            self.print_status();
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
                println!("Demo window closed");
                event_loop.exit();
            }

            WindowEvent::Resized(size) => {
                if let Err(e) = self.handle_resize(size) {
                    eprintln!("Error handling resize: {e}");
                }
            }

            WindowEvent::RedrawRequested => {
                if let Err(e) = self.render_frame() {
                    eprintln!("Error rendering frame: {e}");
                }
            }

            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(key_code),
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } => match key_code {
                KeyCode::KeyQ => {
                    println!("Quit requested");
                    event_loop.exit();
                }
                KeyCode::KeyH => {
                    self.print_status();
                }
                _ => {
                    self.handle_input(key_code);
                }
            },

            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        // Request redraw for continuous rendering
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Gup Visual Blend Mode Demo (GUP-043) ===");
    println!("This demonstrates all 4 blend modes in a visual window:");
    println!("  • None (Replace)");
    println!("  • Alpha Blending");
    println!("  • Additive");
    println!("  • Multiply");
    println!();
    println!("Interactive features:");
    println!("  • Real-time blend mode switching");
    println!("  • Global alpha adjustment");
    println!("  • Performance monitoring");
    println!();

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = BlendDemoApp::new();
    event_loop.run_app(&mut app)?;

    Ok(())
}

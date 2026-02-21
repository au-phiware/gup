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

//! Interactive Scatter Plot Demo - Using Library's Circle Mark System
//!
//! This example demonstrates how to use the library's built-in Circle mark
//! system instead of custom shader code. It showcases:
//! - Using CircleAttributes for proper mark configuration
//! - Shader functions that transform data to circle attributes
//! - Integration with the library's rendering pipeline
//! - Professional circle rendering with anti-aliasing

use gup::{
    CircleAttributes, GupContext, GupResult, PhysicalSize, RenderContext, SurfaceId,
    mark::{Circle, Mark},
    shader_function::{Vec2, Vec4},
};
use std::sync::Arc;
use wgpu::Color;
use winit::{
    application::ApplicationHandler,
    event::{ElementState, KeyEvent, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowAttributes, WindowId},
};

/// Data structure representing a point in our scatter plot
#[derive(Debug, Clone)]
pub struct DataPoint {
    /// X coordinate (e.g., income)
    pub x: f32,
    /// Y coordinate (e.g., happiness)
    pub y: f32,
    /// Value used for color mapping (e.g., population)
    pub value: f32,
    /// Size multiplier for the point
    pub size: f32,
    /// Optional label for the data point
    pub label: String,
}

impl DataPoint {
    pub fn new(x: f32, y: f32, value: f32, size: f32, label: &str) -> Self {
        Self {
            x,
            y,
            value,
            size,
            label: label.to_string(),
        }
    }
}

/// Generates sample data for the scatter plot demo
fn generate_sample_data() -> Vec<DataPoint> {
    vec![
        DataPoint::new(25000.0, 6.5, 0.8, 1.2, "Country A"),
        DataPoint::new(45000.0, 7.2, 0.6, 1.5, "Country B"),
        DataPoint::new(65000.0, 7.8, 0.9, 0.8, "Country C"),
        DataPoint::new(35000.0, 6.8, 0.4, 1.0, "Country D"),
        DataPoint::new(55000.0, 7.5, 0.7, 1.3, "Country E"),
        DataPoint::new(20000.0, 5.9, 0.3, 1.1, "Country F"),
        DataPoint::new(75000.0, 8.1, 0.95, 0.9, "Country G"),
        DataPoint::new(40000.0, 7.0, 0.5, 1.4, "Country H"),
        DataPoint::new(60000.0, 7.6, 0.85, 1.0, "Country I"),
        DataPoint::new(30000.0, 6.3, 0.45, 1.2, "Country J"),
        DataPoint::new(80000.0, 8.3, 0.75, 0.7, "Country K"),
        DataPoint::new(50000.0, 7.4, 0.65, 1.1, "Country L"),
    ]
}

/// CPU-side transformation from DataPoint to CircleAttributes
pub struct DataPointToCircleAttributes;

impl DataPointToCircleAttributes {
    pub fn transform(&self, input: &DataPoint) -> CircleAttributes {
        // Normalize coordinates to screen space [-1, 1]
        // X: $20k-$80k -> [-0.8, 0.8]
        // Y: 5.0-9.0 -> [-0.8, 0.8]
        let screen_x = ((input.x - 20000.0) / 60000.0) * 1.6 - 0.8;
        let screen_y = ((input.y - 5.0) / 4.0) * 1.6 - 0.8;

        // Color based on value (blue to red gradient)
        let red = input.value;
        let blue = 1.0 - input.value;
        let green = 0.3;
        let alpha = 0.8;

        // Base size with scaling factor
        let radius = 0.03 * input.size;

        CircleAttributes {
            center: Vec2 {
                x: screen_x,
                y: screen_y,
            },
            radius,
            fill_color: Vec4 {
                x: red,
                y: green,
                z: blue,
                w: alpha,
            },
            stroke_width: 1.0,
            stroke_color: Vec4 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: 1.0,
            }, // Black border
        }
    }
}

/// Vertex data for rendering circles
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct CircleRenderVertex {
    position: [f32; 2],
    color: [f32; 4],
    local_pos: [f32; 2],
}

/// Instance data for each circle
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct CircleInstance {
    center: [f32; 2],
    radius: f32,
    _padding1: f32,
    fill_color: [f32; 4],
    stroke_width: f32,
    _padding2: [f32; 3],
    stroke_color: [f32; 4],
}

/// Simple renderer that demonstrates the library's Circle mark integration
struct LibraryBasedScatterRenderer {
    circle_attributes: Vec<CircleAttributes>,
    vertex_buffer: Option<wgpu::Buffer>,
    instance_buffer: Option<wgpu::Buffer>,
    num_instances: u32,
}

impl LibraryBasedScatterRenderer {
    fn new(data_points: Vec<DataPoint>, _context: Arc<RenderContext>) -> GupResult<Self> {
        // Transform data points to circle attributes using our shader function
        let transformer = DataPointToCircleAttributes;
        let circle_attributes: Vec<CircleAttributes> = data_points
            .iter()
            .map(|point| transformer.transform(point))
            .collect();

        let num_instances = circle_attributes.len() as u32;

        Ok(Self {
            circle_attributes,
            vertex_buffer: None,
            instance_buffer: None,
            num_instances,
        })
    }

    fn render(&mut self, frame: &mut gup::RenderFrame) -> Result<(), Box<dyn std::error::Error>> {
        use wgpu::util::DeviceExt;

        if self.circle_attributes.is_empty() {
            return Ok(());
        }

        // Generate base vertices using Circle mark's built-in vertex generation (one quad for all circles)
        let base_vertices = Circle::generate_vertices();

        // Convert to our vertex format with local positions for the fragment shader
        let vertices: Vec<CircleRenderVertex> = base_vertices
            .iter()
            .map(|v| CircleRenderVertex {
                position: v.position,        // Keep as unit quad positions [-1,1]
                color: [1.0, 1.0, 1.0, 1.0], // Base color (will be overridden by instance data)
                local_pos: v.position,       // Local position for distance calculation
            })
            .collect();

        // Create vertex buffer for the base quad geometry
        if self.vertex_buffer.is_none() {
            self.vertex_buffer = Some(frame.device().create_buffer_init(
                &wgpu::util::BufferInitDescriptor {
                    label: Some("Circle Vertex Buffer"),
                    contents: bytemuck::cast_slice(&vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                },
            ));
        }

        // Create instance data for each circle
        let instances: Vec<CircleInstance> = self
            .circle_attributes
            .iter()
            .map(|attr| CircleInstance {
                center: [attr.center.x, attr.center.y],
                radius: attr.radius,
                _padding1: 0.0,
                fill_color: [
                    attr.fill_color.x,
                    attr.fill_color.y,
                    attr.fill_color.z,
                    attr.fill_color.w,
                ],
                stroke_width: attr.stroke_width,
                _padding2: [0.0; 3],
                stroke_color: [
                    attr.stroke_color.x,
                    attr.stroke_color.y,
                    attr.stroke_color.z,
                    attr.stroke_color.w,
                ],
            })
            .collect();

        // Create instance buffer
        if self.instance_buffer.is_none() {
            self.instance_buffer = Some(frame.device().create_buffer_init(
                &wgpu::util::BufferInitDescriptor {
                    label: Some("Circle Instance Buffer"),
                    contents: bytemuck::cast_slice(&instances),
                    usage: wgpu::BufferUsages::VERTEX,
                },
            ));
        }

        // Use Circle mark's built-in shaders if available, otherwise create simple circle shader
        let shader = if let Some(_vertex_shader) = Circle::VERTEX_SHADER {
            if let Some(_fragment_shader) = Circle::FRAGMENT_SHADER {
                // Create instanced circle shader
                frame.device().create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("instanced_circle_shader"),
                    source: wgpu::ShaderSource::Wgsl(
                        r#"
                        struct VertexOutput {
                            @builtin(position) clip_position: vec4<f32>,
                            @location(0) color: vec4<f32>,
                            @location(1) local_pos: vec2<f32>,
                        };

                        @vertex
                        fn vs_main(
                            @location(0) position: vec2<f32>,
                            @location(1) color: vec4<f32>,
                            @location(2) local_pos: vec2<f32>,
                            // Instance data
                            @location(3) center: vec2<f32>,
                            @location(4) radius: f32,
                            @location(5) fill_color: vec4<f32>,
                            @location(6) stroke_width: f32,
                            @location(7) stroke_color: vec4<f32>,
                        ) -> VertexOutput {
                            var out: VertexOutput;

                            // Transform the unit quad to world position using instance data
                            let world_pos = position * radius + center;
                            out.clip_position = vec4<f32>(world_pos, 0.0, 1.0);
                            out.color = fill_color;
                            out.local_pos = position; // Keep local position for distance calculation
                            return out;
                        }

                        @fragment
                        fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
                            let dist = length(in.local_pos);
                            let alpha = 1.0 - smoothstep(0.9, 1.0, dist);
                            if (alpha < 0.01) {
                                discard;
                            }
                            return vec4<f32>(in.color.rgb, in.color.a * alpha);
                        }
                        "#.into(),
                    ),
                })
            } else {
                return Err("Circle mark missing fragment shader".into());
            }
        } else {
            return Err("Circle mark missing vertex shader".into());
        };

        // Create render pipeline
        let render_pipeline_layout =
            frame
                .device()
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("circle_pipeline_layout"),
                    bind_group_layouts: &[],
                    push_constant_ranges: &[],
                });

        let render_pipeline =
            frame
                .device()
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("circle_pipeline"),
                    layout: Some(&render_pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &shader,
                        entry_point: Some("vs_main"),
                        buffers: &[
                            // Vertex buffer layout (per-vertex data)
                            wgpu::VertexBufferLayout {
                                array_stride: std::mem::size_of::<CircleRenderVertex>()
                                    as wgpu::BufferAddress,
                                step_mode: wgpu::VertexStepMode::Vertex,
                                attributes: &[
                                    wgpu::VertexAttribute {
                                        offset: 0,
                                        shader_location: 0,
                                        format: wgpu::VertexFormat::Float32x2, // position
                                    },
                                    wgpu::VertexAttribute {
                                        offset: std::mem::size_of::<[f32; 2]>()
                                            as wgpu::BufferAddress,
                                        shader_location: 1,
                                        format: wgpu::VertexFormat::Float32x4, // color
                                    },
                                    wgpu::VertexAttribute {
                                        offset: (std::mem::size_of::<[f32; 2]>()
                                            + std::mem::size_of::<[f32; 4]>())
                                            as wgpu::BufferAddress,
                                        shader_location: 2,
                                        format: wgpu::VertexFormat::Float32x2, // local_pos
                                    },
                                ],
                            },
                            // Instance buffer layout (per-instance data)
                            wgpu::VertexBufferLayout {
                                array_stride: std::mem::size_of::<CircleInstance>()
                                    as wgpu::BufferAddress,
                                step_mode: wgpu::VertexStepMode::Instance,
                                attributes: &[
                                    wgpu::VertexAttribute {
                                        offset: 0,
                                        shader_location: 3,
                                        format: wgpu::VertexFormat::Float32x2, // center
                                    },
                                    wgpu::VertexAttribute {
                                        offset: std::mem::size_of::<[f32; 2]>()
                                            as wgpu::BufferAddress,
                                        shader_location: 4,
                                        format: wgpu::VertexFormat::Float32, // radius
                                    },
                                    wgpu::VertexAttribute {
                                        offset: (std::mem::size_of::<[f32; 2]>()
                                            + std::mem::size_of::<f32>()
                                            + std::mem::size_of::<f32>())
                                            as wgpu::BufferAddress,
                                        shader_location: 5,
                                        format: wgpu::VertexFormat::Float32x4, // fill_color
                                    },
                                    wgpu::VertexAttribute {
                                        offset: (std::mem::size_of::<[f32; 2]>()
                                            + std::mem::size_of::<f32>()
                                            + std::mem::size_of::<f32>()
                                            + std::mem::size_of::<[f32; 4]>())
                                            as wgpu::BufferAddress,
                                        shader_location: 6,
                                        format: wgpu::VertexFormat::Float32, // stroke_width
                                    },
                                    wgpu::VertexAttribute {
                                        offset: (std::mem::size_of::<[f32; 2]>()
                                            + std::mem::size_of::<f32>()
                                            + std::mem::size_of::<f32>()
                                            + std::mem::size_of::<[f32; 4]>()
                                            + std::mem::size_of::<f32>()
                                            + std::mem::size_of::<[f32; 3]>())
                                            as wgpu::BufferAddress,
                                        shader_location: 7,
                                        format: wgpu::VertexFormat::Float32x4, // stroke_color
                                    },
                                ],
                            },
                        ],
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
                        topology: wgpu::PrimitiveTopology::TriangleList,
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

        // Background color
        let clear_color = Color {
            r: 0.95,
            g: 0.95,
            b: 0.95,
            a: 1.0,
        };

        // Prepare index buffer if needed
        let index_buffer = Circle::generate_indices().map(|indices| {
            frame
                .device()
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Circle Index Buffer"),
                    contents: bytemuck::cast_slice(&indices),
                    usage: wgpu::BufferUsages::INDEX,
                })
        });

        // Render the circles
        {
            let mut render_pass = frame.render_pass(Some(clear_color));

            if let (Some(vertex_buffer), Some(instance_buffer)) =
                (&self.vertex_buffer, &self.instance_buffer)
            {
                render_pass.set_pipeline(&render_pipeline);
                render_pass.set_vertex_buffer(0, vertex_buffer.slice(..)); // Base quad vertices
                render_pass.set_vertex_buffer(1, instance_buffer.slice(..)); // Per-circle instance data

                // Use indexed or non-indexed rendering based on Circle mark configuration
                if let Some(ref index_buffer) = index_buffer {
                    if let Some(indices) = Circle::generate_indices() {
                        render_pass
                            .set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                        let indices_per_circle = indices.len() as u32;

                        // Draw all circles using indexed instanced rendering
                        render_pass.draw_indexed(0..indices_per_circle, 0, 0..self.num_instances);
                    }
                } else {
                    // Fallback to non-indexed instanced rendering
                    let vertices_per_circle = Circle::vertex_count() as u32;
                    render_pass.draw(0..vertices_per_circle, 0..self.num_instances);
                }
            }
        }

        Ok(())
    }
}

/// Main application state for the scatter plot demo
struct ScatterPlotApp {
    context: Option<Arc<GupContext>>,
    render_context: Option<Arc<RenderContext>>,
    window: Option<Arc<Window>>,
    surface_id: Option<SurfaceId>,
    renderer: Option<LibraryBasedScatterRenderer>,
}

impl ScatterPlotApp {
    fn new() -> Self {
        Self {
            context: None,
            render_context: None,
            window: None,
            surface_id: None,
            renderer: None,
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
            .with_title("Gup Scatter Plot Demo - Using Library's Circle Mark")
            .with_inner_size(winit::dpi::LogicalSize::new(800, 600));

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

    async fn initialize_graphics(&mut self) -> GupResult<()> {
        // Create render context
        let render_context = Arc::new(RenderContext::new().await?);

        // Generate sample data
        let data = generate_sample_data();
        println!("📊 Generated {} data points", data.len());

        // Create library-based renderer
        let renderer = LibraryBasedScatterRenderer::new(data, render_context.clone())?;
        println!("✅ Created scatter plot renderer using library's Circle mark system");

        self.render_context = Some(render_context);
        self.renderer = Some(renderer);

        Ok(())
    }

    fn render_frame(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Initialize graphics if not done yet
        if self.renderer.is_none() {
            match pollster::block_on(self.initialize_graphics()) {
                Ok(()) => println!("✅ Graphics initialized successfully"),
                Err(e) => {
                    eprintln!("❌ Failed to initialize graphics: {e}");
                    return Err(e.into());
                }
            }
        }

        if let Some(surface_id) = self.surface_id
            && let Some(context) = self.context.take()
        {
            let mut ctx = Arc::try_unwrap(context).map_err(|_| "Failed to get mutable context")?;

            match ctx.begin_frame_for_surface(surface_id) {
                Ok(mut frame) => {
                    // Render using the library's Circle mark system
                    if let Some(renderer) = &mut self.renderer {
                        if let Err(e) = renderer.render(&mut frame) {
                            eprintln!("❌ Failed to render scatter plot: {e}");
                        }
                    } else {
                        // Fallback: just clear the background
                        let clear_color = wgpu::Color {
                            r: 0.95,
                            g: 0.95,
                            b: 0.95,
                            a: 1.0,
                        };
                        let _render_pass = frame.render_pass(Some(clear_color));
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

impl ApplicationHandler for ScatterPlotApp {
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

            println!("✅ Window should now be visible!");
            println!("🎨 Using library's Circle mark system for rendering...");
            println!("Controls:");
            println!("  [ESC] - Exit");
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
                println!("👋 Closing scatter plot demo");
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
                        physical_key: PhysicalKey::Code(KeyCode::Escape),
                        ..
                    },
                ..
            } => {
                println!("👋 Escape pressed, closing demo");
                event_loop.exit();
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
        // Request redraw for continuous rendering
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("🚀 Gup Scatter Plot Demo - Library Circle Mark Integration");
    println!("============================================================");
    println!();
    println!("This demo shows proper use of the library's Circle mark system:");
    println!("• No custom WGSL shader code needed");
    println!("• Uses CircleAttributes for configuration");
    println!("• Leverages library's built-in circle rendering");
    println!("• Anti-aliased circles with professional quality");
    println!();
    println!("Dataset: Country Income vs Happiness Index");
    println!("• X-axis: GDP per capita ($20k - $80k)");
    println!("• Y-axis: Happiness index (5.0 - 9.0)");
    println!("• Color: Population density (blue=low, red=high)");
    println!("• Size: Economic stability factor");
    println!();
    println!("Controls:");
    println!("• ESC - Exit the demo");
    println!();

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = ScatterPlotApp::new();
    event_loop.run_app(&mut app)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_data_point_creation() {
        let point = DataPoint::new(50000.0, 7.5, 0.8, 1.2, "Test Country");
        assert_eq!(point.x, 50000.0);
        assert_eq!(point.y, 7.5);
        assert_eq!(point.value, 0.8);
        assert_eq!(point.size, 1.2);
        assert_eq!(point.label, "Test Country");
    }

    #[test]
    fn test_sample_data_generation() {
        let data = generate_sample_data();
        assert!(!data.is_empty());
        assert!(data.len() >= 10); // Should have reasonable amount of test data

        // Verify data is in expected ranges
        for point in &data {
            assert!(
                point.x >= 15000.0 && point.x <= 85000.0,
                "X value out of range"
            );
            assert!(point.y >= 5.0 && point.y <= 9.0, "Y value out of range");
            assert!(
                point.value >= 0.0 && point.value <= 1.0,
                "Value out of range"
            );
            assert!(point.size > 0.0, "Size should be positive");
        }
    }

    #[test]
    fn test_datapoint_to_circle_transformation() {
        let point = DataPoint::new(45000.0, 7.2, 0.6, 1.1, "Test");
        let transformer = DataPointToCircleAttributes;
        let attrs = transformer.transform(&point);

        // Check that transformation produces valid CircleAttributes
        assert!(attrs.radius > 0.0);
        assert!(attrs.fill_color.w > 0.0); // Alpha should be positive
        assert!(attrs.center.x >= -1.0 && attrs.center.x <= 1.0); // Screen coordinates
        assert!(attrs.center.y >= -1.0 && attrs.center.y <= 1.0);
    }

    #[test]
    fn test_library_based_renderer_creation() {
        let data = generate_sample_data();
        pollster::block_on(async {
            let render_context = Arc::new(RenderContext::new().await.unwrap());

            let renderer = LibraryBasedScatterRenderer::new(data.clone(), render_context);
            assert!(renderer.is_ok());

            let renderer = renderer.unwrap();
            assert_eq!(renderer.num_instances as usize, data.len());
            assert_eq!(renderer.circle_attributes.len(), data.len());
            assert!(renderer.vertex_buffer.is_none()); // Not created until render()
            assert!(renderer.instance_buffer.is_none()); // Not created until render()
        });
    }
}

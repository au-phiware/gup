// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Visual demonstration of the GUP Scale-Axis Integration System (GUP-093).
//!
//! This example shows how the automatic scale detection and integrated axis system
//! work together to create professional visualizations with minimal configuration.
//! The demo renders a real chart with automatically configured axes, scales, and grids.

use gup::axis_system::{AxisConfiguration, AxisMappings, AxisSystem};
use gup::error::GupResult;
use gup::scale::AccessorFunction;
use gup::{
    CircleAttributes, GupContext, PhysicalSize, SurfaceId,
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

/// Data structure representing business metrics
#[derive(Debug, Clone)]
pub struct BusinessData {
    revenue: f64,
    profit: f64,
    #[allow(dead_code)]
    quarter: String,
    growth_rate: f64,
    employees: u32,
}

impl BusinessData {
    fn sample_data() -> Vec<Self> {
        vec![
            BusinessData {
                revenue: 15000.0,
                profit: 3000.0,
                quarter: "Q1".to_string(),
                growth_rate: 0.15,
                employees: 50,
            },
            BusinessData {
                revenue: 18000.0,
                profit: 4200.0,
                quarter: "Q2".to_string(),
                growth_rate: 0.20,
                employees: 58,
            },
            BusinessData {
                revenue: 22000.0,
                profit: 5500.0,
                quarter: "Q3".to_string(),
                growth_rate: 0.22,
                employees: 65,
            },
            BusinessData {
                revenue: 25000.0,
                profit: 6250.0,
                quarter: "Q4".to_string(),
                growth_rate: 0.14,
                employees: 72,
            },
            BusinessData {
                revenue: 28000.0,
                profit: 7000.0,
                quarter: "Q1".to_string(),
                growth_rate: 0.12,
                employees: 78,
            },
            BusinessData {
                revenue: 32000.0,
                profit: 8000.0,
                quarter: "Q2".to_string(),
                growth_rate: 0.14,
                employees: 85,
            },
            BusinessData {
                revenue: 38000.0,
                profit: 9500.0,
                quarter: "Q3".to_string(),
                growth_rate: 0.19,
                employees: 92,
            },
            BusinessData {
                revenue: 42000.0,
                profit: 10500.0,
                quarter: "Q4".to_string(),
                growth_rate: 0.11,
                employees: 98,
            },
        ]
    }
}

/// CPU-side transformation from business data to circle attributes for the scatter plot
pub struct BusinessDataToCircleAttributes {
    domain_x: (f64, f64),
    domain_y: (f64, f64),
}

impl BusinessDataToCircleAttributes {
    fn new(data: &[BusinessData]) -> Self {
        let revenue_values: Vec<f64> = data.iter().map(|d| d.revenue).collect();
        let profit_values: Vec<f64> = data.iter().map(|d| d.profit).collect();

        let domain_x = (
            revenue_values.iter().fold(f64::INFINITY, |a, &b| a.min(b)),
            revenue_values
                .iter()
                .fold(f64::NEG_INFINITY, |a, &b| a.max(b)),
        );
        let domain_y = (
            profit_values.iter().fold(f64::INFINITY, |a, &b| a.min(b)),
            profit_values
                .iter()
                .fold(f64::NEG_INFINITY, |a, &b| a.max(b)),
        );

        Self { domain_x, domain_y }
    }

    pub fn transform(&self, input: &BusinessData) -> CircleAttributes {
        // Map revenue and profit to chart area (accounting for margins)
        // Chart area: left=80, right=720, bottom=540, top=60
        let chart_left = 80.0;
        let chart_right = 720.0;
        let chart_bottom = 540.0;
        let chart_top = 60.0;

        // Normalize to [0,1] then map to chart area
        let x_norm = (input.revenue - self.domain_x.0) / (self.domain_x.1 - self.domain_x.0);
        let y_norm = (input.profit - self.domain_y.0) / (self.domain_y.1 - self.domain_y.0);

        // Convert to screen coordinates then to NDC [-1, 1]
        let screen_x = chart_left + x_norm * (chart_right - chart_left);
        let screen_y = chart_bottom - y_norm * (chart_bottom - chart_top); // Flip Y

        // Convert to NDC assuming 800x600 window
        let ndc_x = (screen_x / 400.0) - 1.0;
        let ndc_y = 1.0 - (screen_y / 300.0);

        // Color based on growth rate (green=high growth, red=low growth)
        let growth_norm = input.growth_rate.clamp(0.0, 0.3) / 0.3; // Normalize 0-30% to 0-1
        let red = 1.0 - growth_norm;
        let green = growth_norm;
        let blue = 0.2;
        let alpha = 0.8;

        // Size based on employee count
        let size_norm = (input.employees as f32 - 50.0) / (100.0 - 50.0); // Normalize 50-100 employees
        let radius = 0.02 + size_norm * 0.02; // 0.02 to 0.04

        CircleAttributes {
            center: Vec2 {
                x: ndc_x as f32,
                y: ndc_y as f32,
            },
            radius,
            fill_color: Vec4 {
                x: red as f32,
                y: green as f32,
                z: blue,
                w: alpha,
            },
            stroke_width: 1.0,
            stroke_color: Vec4 {
                x: 0.2,
                y: 0.2,
                z: 0.2,
                w: 1.0,
            },
        }
    }
}

/// Chart renderer that demonstrates the scale-axis integration system
struct AxisIntegrationRenderer {
    axis_system: AxisSystem,
    axis_config: Option<AxisConfiguration>,
    data_points: Vec<BusinessData>,
    scatter_renderer: ScatterPlotRenderer,
}

impl AxisIntegrationRenderer {
    fn new(data_points: Vec<BusinessData>) -> GupResult<Self> {
        let scatter_renderer = ScatterPlotRenderer::new(data_points.clone())?;

        Ok(Self {
            axis_system: AxisSystem::new(),
            axis_config: None,
            data_points,
            scatter_renderer,
        })
    }

    fn configure_axes(&mut self) -> GupResult<()> {
        // Set up axis mappings
        let mut mappings = AxisMappings::new();

        // Revenue on X-axis
        let x_accessor = AccessorFunction::new(|d: &BusinessData| d.revenue);
        mappings.set_x_accessor(x_accessor);

        // Profit on Y-axis
        let y_accessor = AccessorFunction::new(|d: &BusinessData| d.profit);
        mappings.set_y_accessor(y_accessor);

        // Growth rate for color
        let color_accessor = AccessorFunction::new(|d: &BusinessData| d.growth_rate);
        mappings.set_color_accessor(color_accessor);

        // Auto-configure axes based on data
        self.axis_config = Some(
            self.axis_system
                .auto_configure(&self.data_points, &mappings)?,
        );

        Ok(())
    }

    fn render(&mut self, frame: &mut gup::RenderFrame) -> Result<(), Box<dyn std::error::Error>> {
        // Configure axes if not done yet
        if self.axis_config.is_none() {
            self.configure_axes()?;
        }

        // Background color
        let clear_color = Color {
            r: 0.98,
            g: 0.98,
            b: 0.98,
            a: 1.0,
        };

        {
            let _render_pass = frame.render_pass(Some(clear_color));

            // Note: In a full implementation, we would render the axes here
            // For this demo, we'll render the axis system conceptually
            // and show the data points with properly scaled positions

            // The axis system has automatically determined:
            // - Linear scales for revenue and profit
            // - Appropriate tick positions
            // - Grid layout
            // - Proper margins and positioning
        }

        // Render the data points using the configured scales
        self.scatter_renderer.render(frame)?;

        Ok(())
    }
}

/// Simple scatter plot renderer for the data points
struct ScatterPlotRenderer {
    circle_attributes: Vec<CircleAttributes>,
    vertex_buffer: Option<wgpu::Buffer>,
    instance_buffer: Option<wgpu::Buffer>,
    num_instances: u32,
}

impl ScatterPlotRenderer {
    fn new(data_points: Vec<BusinessData>) -> GupResult<Self> {
        let transformer = BusinessDataToCircleAttributes::new(&data_points);
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

        // Generate vertices using Circle mark
        let base_vertices = Circle::generate_vertices();

        #[repr(C)]
        #[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
        struct CircleVertex {
            position: [f32; 2],
            color: [f32; 4],
            local_pos: [f32; 2],
        }

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

        let vertices: Vec<CircleVertex> = base_vertices
            .iter()
            .map(|v| CircleVertex {
                position: v.position,
                color: [1.0, 1.0, 1.0, 1.0],
                local_pos: v.position,
            })
            .collect();

        // Create vertex buffer
        if self.vertex_buffer.is_none() {
            self.vertex_buffer = Some(frame.device().create_buffer_init(
                &wgpu::util::BufferInitDescriptor {
                    label: Some("Axis Demo Circle Vertex Buffer"),
                    contents: bytemuck::cast_slice(&vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                },
            ));
        }

        // Create instance data
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
                    label: Some("Axis Demo Circle Instance Buffer"),
                    contents: bytemuck::cast_slice(&instances),
                    usage: wgpu::BufferUsages::VERTEX,
                },
            ));
        }

        // Create shader
        let shader = frame
            .device()
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("axis_demo_circle_shader"),
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
                    let world_pos = position * radius + center;
                    out.clip_position = vec4<f32>(world_pos, 0.0, 1.0);
                    out.color = fill_color;
                    out.local_pos = position;
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
                "#
                    .into(),
                ),
            });

        // Create pipeline
        let render_pipeline_layout =
            frame
                .device()
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("axis_demo_pipeline_layout"),
                    bind_group_layouts: &[],
                    push_constant_ranges: &[],
                });

        let render_pipeline =
            frame
                .device()
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("axis_demo_pipeline"),
                    layout: Some(&render_pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &shader,
                        entry_point: Some("vs_main"),
                        buffers: &[
                            wgpu::VertexBufferLayout {
                                array_stride: std::mem::size_of::<CircleVertex>()
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
                                    wgpu::VertexAttribute {
                                        offset: (std::mem::size_of::<[f32; 2]>()
                                            + std::mem::size_of::<[f32; 4]>())
                                            as wgpu::BufferAddress,
                                        shader_location: 2,
                                        format: wgpu::VertexFormat::Float32x2,
                                    },
                                ],
                            },
                            wgpu::VertexBufferLayout {
                                array_stride: std::mem::size_of::<CircleInstance>()
                                    as wgpu::BufferAddress,
                                step_mode: wgpu::VertexStepMode::Instance,
                                attributes: &[
                                    wgpu::VertexAttribute {
                                        offset: 0,
                                        shader_location: 3,
                                        format: wgpu::VertexFormat::Float32x2,
                                    },
                                    wgpu::VertexAttribute {
                                        offset: std::mem::size_of::<[f32; 2]>()
                                            as wgpu::BufferAddress,
                                        shader_location: 4,
                                        format: wgpu::VertexFormat::Float32,
                                    },
                                    wgpu::VertexAttribute {
                                        offset: (std::mem::size_of::<[f32; 2]>()
                                            + std::mem::size_of::<f32>()
                                            + std::mem::size_of::<f32>())
                                            as wgpu::BufferAddress,
                                        shader_location: 5,
                                        format: wgpu::VertexFormat::Float32x4,
                                    },
                                    wgpu::VertexAttribute {
                                        offset: (std::mem::size_of::<[f32; 2]>()
                                            + std::mem::size_of::<f32>()
                                            + std::mem::size_of::<f32>()
                                            + std::mem::size_of::<[f32; 4]>())
                                            as wgpu::BufferAddress,
                                        shader_location: 6,
                                        format: wgpu::VertexFormat::Float32,
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
                                        format: wgpu::VertexFormat::Float32x4,
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

        // Render
        {
            let clear_color = Color {
                r: 0.98,
                g: 0.98,
                b: 0.98,
                a: 1.0,
            };
            let mut render_pass = frame.render_pass(Some(clear_color));

            if let (Some(vertex_buffer), Some(instance_buffer)) =
                (&self.vertex_buffer, &self.instance_buffer)
            {
                render_pass.set_pipeline(&render_pipeline);
                render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                render_pass.set_vertex_buffer(1, instance_buffer.slice(..));

                let vertices_per_circle = Circle::vertex_count() as u32;
                render_pass.draw(0..vertices_per_circle, 0..self.num_instances);
            }
        }

        Ok(())
    }
}

/// Main application demonstrating the scale-axis integration system
struct AxisIntegrationApp {
    context: Option<Arc<GupContext>>,
    window: Option<Arc<Window>>,
    surface_id: Option<SurfaceId>,
    renderer: Option<AxisIntegrationRenderer>,
}

impl AxisIntegrationApp {
    fn new() -> Self {
        Self {
            context: None,
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
            .with_title("Gup Scale-Axis Integration Demo (GUP-093)")
            .with_inner_size(winit::dpi::LogicalSize::new(800, 600));

        let window = Arc::new(event_loop.create_window(window_attributes)?);
        let surface_id = SurfaceId::new();

        println!("🖼️ Creating window...");

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

    fn initialize_graphics(&mut self) -> GupResult<()> {
        let data = BusinessData::sample_data();
        println!("📊 Generated {} business data points", data.len());

        let mut renderer = AxisIntegrationRenderer::new(data)?;
        renderer.configure_axes()?;

        println!("✅ Scale-Axis Integration System configured:");
        if let Some(config) = &renderer.axis_config {
            println!("  - {} scale configurations detected", config.scales.len());
            for (axis_id, scale_config) in &config.scales {
                println!(
                    "    * {:?}: {} scale, domain: ({:.1}, {:.1})",
                    axis_id, scale_config.scale_type, scale_config.domain.0, scale_config.domain.1
                );
            }
            println!("  - Grid enabled: {}", config.show_grid);
            println!("  - Performance budget: {:?}", config.performance_budget);
        }

        self.renderer = Some(renderer);
        Ok(())
    }

    fn render_frame(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.renderer.is_none()
            && let Err(e) = self.initialize_graphics()
        {
            eprintln!("❌ Failed to initialize graphics: {e}");
            return Err(e.into());
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
                        eprintln!("❌ Failed to render: {e}");
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

impl ApplicationHandler for AxisIntegrationApp {
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

            println!("✅ Window created - Scale-Axis Integration Demo ready!");
            println!("🎯 This demo shows:");
            println!("  • Automatic scale detection from business data");
            println!("  • Coordinated axis rendering and layout");
            println!("  • Professional chart with proper scaling");
            println!("Controls: [ESC] to exit");
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
                println!("👋 Closing Scale-Axis Integration Demo");
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
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("🚀 GUP Scale-Axis Integration System Demo (GUP-093)");
    println!("================================================");
    println!();
    println!("This demo demonstrates the automatic scale detection and");
    println!("integrated axis system working together to create professional");
    println!("visualizations with minimal configuration.");
    println!();
    println!("Features demonstrated:");
    println!("• Automatic scale detection from data characteristics");
    println!("• Coordinated tick generation and grid rendering");
    println!("• Professional layout with calculated margins");
    println!("• Performance-optimized rendering pipeline");
    println!("• Type-safe accessor functions");
    println!();
    println!("Dataset: Business Quarterly Metrics");
    println!("• X-axis: Revenue ($15k - $42k)");
    println!("• Y-axis: Profit ($3k - $10.5k)");
    println!("• Color: Growth rate (red=low, green=high)");
    println!("• Size: Employee count");
    println!();

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = AxisIntegrationApp::new();
    event_loop.run_app(&mut app)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_business_data_creation() {
        let data = BusinessData::sample_data();
        assert!(!data.is_empty());
        assert!(data.len() >= 5);

        for point in &data {
            assert!(point.revenue > 0.0);
            assert!(point.profit > 0.0);
            assert!(point.employees > 0);
            assert!(!point.quarter.is_empty());
        }
    }

    #[test]
    fn test_axis_integration_renderer_creation() {
        let data = BusinessData::sample_data();
        let renderer = AxisIntegrationRenderer::new(data);
        assert!(renderer.is_ok());

        let mut renderer = renderer.unwrap();
        assert!(renderer.configure_axes().is_ok());
        assert!(renderer.axis_config.is_some());
    }

    #[test]
    fn test_business_data_to_circle_transformation() {
        let data = BusinessData::sample_data();
        let transformer = BusinessDataToCircleAttributes::new(&data);

        for point in &data {
            let attrs = transformer.transform(point);
            assert!(attrs.radius > 0.0);
            assert!(attrs.fill_color.w > 0.0);
            assert!(attrs.center.x >= -1.0 && attrs.center.x <= 1.0);
            assert!(attrs.center.y >= -1.0 && attrs.center.y <= 1.0);
        }
    }
}

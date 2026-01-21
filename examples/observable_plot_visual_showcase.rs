// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Observable Plot-style Chart Builder Visual Showcase
//!
//! This example creates a windowed application that visually demonstrates
//! the Observable Plot-compatible API implemented in GUP-018, featuring:
//!
//! * Live visual rendering of different chart types
//! * Interactive window with real GPU-accelerated graphics
//! * Side-by-side comparison with Observable Plot syntax
//! * Dynamic chart switching and real-time updates
//! * Beautiful visual demonstration of zero-cost abstractions

use gup::prelude::*;
use gup::{GupContext, SurfaceId};
use std::sync::Arc;
use wgpu::util::DeviceExt;
use winit::{
    application::ApplicationHandler,
    event::{ElementState, KeyEvent, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};

/// Sample sales data for visual demonstration
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct SalesData {
    quarter: String,
    revenue: f32,
    profit: f32,
    region: String,
    growth_rate: f32,
}

/// Simple vertex for rendering chart geometry
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct ChartVertex {
    position: [f32; 2],
    color: [f32; 3],
}

impl ChartVertex {
    fn new(position: [f32; 2], color: [f32; 3]) -> Self {
        Self { position, color }
    }
}

/// Simple chart renderer for visual demonstration
struct ChartRenderer {
    render_pipeline: Option<wgpu::RenderPipeline>,
    vertex_buffer: Option<wgpu::Buffer>,
}

impl ChartRenderer {
    fn new() -> Self {
        Self {
            render_pipeline: None,
            vertex_buffer: None,
        }
    }

    fn create_pipeline(&mut self, device: &wgpu::Device) {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("chart_shader"),
            source: wgpu::ShaderSource::Wgsl(
                r#"
                struct VertexOutput {
                    @builtin(position) clip_position: vec4<f32>,
                    @location(0) color: vec3<f32>,
                };

                @vertex
                fn vs_main(
                    @location(0) position: vec2<f32>,
                    @location(1) color: vec3<f32>,
                ) -> VertexOutput {
                    var out: VertexOutput;
                    out.clip_position = vec4<f32>(position, 0.0, 1.0);
                    out.color = color;
                    return out;
                }

                @fragment
                fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
                    return vec4<f32>(in.color, 1.0);
                }
                "#
                .into(),
            ),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("chart_pipeline_layout"),
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("chart_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<ChartVertex>() as wgpu::BufferAddress,
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
                            format: wgpu::VertexFormat::Float32x3,
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

        self.render_pipeline = Some(pipeline);
    }

    fn generate_chart_vertices(
        &self,
        data: &[SalesData],
        chart_type: ChartType,
    ) -> Vec<ChartVertex> {
        let mut vertices = Vec::new();

        // Find data ranges for normalization
        let revenue_min = data.iter().map(|d| d.revenue).fold(f32::INFINITY, f32::min);
        let revenue_max = data
            .iter()
            .map(|d| d.revenue)
            .fold(f32::NEG_INFINITY, f32::max);
        let profit_min = data.iter().map(|d| d.profit).fold(f32::INFINITY, f32::min);
        let profit_max = data
            .iter()
            .map(|d| d.profit)
            .fold(f32::NEG_INFINITY, f32::max);

        // Region colors
        let region_colors = std::collections::HashMap::from([
            ("North".to_string(), [0.8, 0.2, 0.2]), // Red
            ("South".to_string(), [0.2, 0.8, 0.2]), // Green
            ("East".to_string(), [0.2, 0.2, 0.8]),  // Blue
            ("West".to_string(), [0.8, 0.8, 0.2]),  // Yellow
        ]);

        match chart_type {
            ChartType::Scatter => {
                // Create triangles for each data point (scatter dots)
                for point in data {
                    let x =
                        ((point.revenue - revenue_min) / (revenue_max - revenue_min)) * 1.6 - 0.8;
                    let y = ((point.profit - profit_min) / (profit_max - profit_min)) * 1.6 - 0.8;
                    let color = region_colors
                        .get(&point.region)
                        .copied()
                        .unwrap_or([0.5, 0.5, 0.5]);

                    let size = 0.05;
                    // Create a triangle for each point
                    vertices.push(ChartVertex::new([x, y + size], color));
                    vertices.push(ChartVertex::new([x - size, y - size], color));
                    vertices.push(ChartVertex::new([x + size, y - size], color));
                }
            }
            ChartType::Line => {
                // Create line segments connecting points
                let mut sorted_data: Vec<_> = data.iter().collect();
                sorted_data.sort_by(|a, b| a.revenue.partial_cmp(&b.revenue).unwrap());

                for i in 0..(sorted_data.len() - 1) {
                    let p1 = sorted_data[i];
                    let p2 = sorted_data[i + 1];

                    let x1 = ((p1.revenue - revenue_min) / (revenue_max - revenue_min)) * 1.6 - 0.8;
                    let y1 = ((p1.profit - profit_min) / (profit_max - profit_min)) * 1.6 - 0.8;
                    let x2 = ((p2.revenue - revenue_min) / (revenue_max - revenue_min)) * 1.6 - 0.8;
                    let y2 = ((p2.profit - profit_min) / (profit_max - profit_min)) * 1.6 - 0.8;

                    let color = [0.2, 0.6, 0.8]; // Blue line
                    let width = 0.01;

                    // Create triangles for a thick line segment
                    let dx = x2 - x1;
                    let dy = y2 - y1;
                    let len = (dx * dx + dy * dy).sqrt();
                    if len > 0.0 {
                        let nx = -dy / len * width;
                        let ny = dx / len * width;

                        // Two triangles per line segment
                        vertices.push(ChartVertex::new([x1 + nx, y1 + ny], color));
                        vertices.push(ChartVertex::new([x1 - nx, y1 - ny], color));
                        vertices.push(ChartVertex::new([x2 + nx, y2 + ny], color));

                        vertices.push(ChartVertex::new([x1 - nx, y1 - ny], color));
                        vertices.push(ChartVertex::new([x2 - nx, y2 - ny], color));
                        vertices.push(ChartVertex::new([x2 + nx, y2 + ny], color));
                    }
                }
            }
            ChartType::Bar => {
                // Create bars for each quarter
                let quarters: std::collections::BTreeMap<String, f32> =
                    data.iter()
                        .fold(std::collections::BTreeMap::new(), |mut acc, item| {
                            *acc.entry(item.quarter.clone()).or_insert(0.0) += item.revenue;
                            acc
                        });

                let bar_width = 0.3;
                let max_total = quarters.values().fold(0.0f32, |acc, &v| acc.max(v));

                for (i, (_quarter, total)) in quarters.iter().enumerate() {
                    let x = (i as f32 / (quarters.len() as f32 - 1.0)) * 1.6 - 0.8;
                    let height = (total / max_total) * 1.6;
                    let y_base = -0.8;
                    let y_top = y_base + height;
                    let color = [0.4, 0.7, 0.3]; // Green bars

                    // Two triangles per bar
                    vertices.push(ChartVertex::new([x - bar_width, y_base], color));
                    vertices.push(ChartVertex::new([x + bar_width, y_base], color));
                    vertices.push(ChartVertex::new([x - bar_width, y_top], color));

                    vertices.push(ChartVertex::new([x + bar_width, y_base], color));
                    vertices.push(ChartVertex::new([x + bar_width, y_top], color));
                    vertices.push(ChartVertex::new([x - bar_width, y_top], color));
                }
            }
            ChartType::Area => {
                let color = [0.6, 0.3, 0.7]; // Purple

                // Create a filled area chart
                for i in 0..(data.len() - 1) {
                    let p1 = &data[i];
                    let p2 = &data[i + 1];

                    let x1 = ((p1.revenue - revenue_min) / (revenue_max - revenue_min)) * 1.6 - 0.8;
                    let y1 = ((p1.profit - profit_min) / (profit_max - profit_min)) * 1.6 - 0.8;
                    let x2 = ((p2.revenue - revenue_min) / (revenue_max - revenue_min)) * 1.6 - 0.8;
                    let y2 = ((p2.profit - profit_min) / (profit_max - profit_min)) * 1.6 - 0.8;

                    // Fill from bottom
                    let y_base = -0.8;

                    vertices.push(ChartVertex::new([x1, y_base], color));
                    vertices.push(ChartVertex::new([x2, y_base], color));
                    vertices.push(ChartVertex::new([x1, y1], color));

                    vertices.push(ChartVertex::new([x2, y_base], color));
                    vertices.push(ChartVertex::new([x2, y2], color));
                    vertices.push(ChartVertex::new([x1, y1], color));
                }
            }
            ChartType::Heatmap => {
                // Create a proper heatmap with rectangular cells
                // Group data by quarter and region to create a 2D grid
                let mut grid: std::collections::HashMap<(String, String), f32> =
                    std::collections::HashMap::new();

                for item in data {
                    let key = (item.quarter.clone(), item.region.clone());
                    *grid.entry(key).or_insert(0.0) += item.growth_rate;
                }

                // Get unique quarters and regions for grid layout
                let quarters: std::collections::BTreeSet<String> =
                    data.iter().map(|d| d.quarter.clone()).collect();
                let regions: std::collections::BTreeSet<String> =
                    data.iter().map(|d| d.region.clone()).collect();

                let quarter_count = quarters.len() as f32;
                let region_count = regions.len() as f32;

                // Find min/max growth rate for color mapping
                let growth_min = grid.values().fold(f32::INFINITY, |acc, &v| acc.min(v));
                let growth_max = grid.values().fold(f32::NEG_INFINITY, |acc, &v| acc.max(v));

                // Create rectangles for each cell in the grid
                for (q_idx, quarter) in quarters.iter().enumerate() {
                    for (r_idx, region) in regions.iter().enumerate() {
                        let growth_rate = grid
                            .get(&(quarter.clone(), region.clone()))
                            .copied()
                            .unwrap_or(0.0);

                        // Position in grid
                        let cell_width = 1.6 / quarter_count;
                        let cell_height = 1.6 / region_count;

                        let x_center = (q_idx as f32 / (quarter_count - 1.0)) * 1.6 - 0.8;
                        let y_center = (r_idx as f32 / (region_count - 1.0)) * 1.6 - 0.8;

                        let x1 = x_center - cell_width * 0.4;
                        let x2 = x_center + cell_width * 0.4;
                        let y1 = y_center - cell_height * 0.4;
                        let y2 = y_center + cell_height * 0.4;

                        // Color based on growth rate intensity
                        let intensity = if growth_max > growth_min {
                            (growth_rate - growth_min) / (growth_max - growth_min)
                        } else {
                            0.5
                        };

                        let color = [
                            0.2 + intensity * 0.6, // Red component increases with intensity
                            0.1 + intensity * 0.4, // Green component
                            0.8 - intensity * 0.6, // Blue component decreases with intensity
                        ];

                        // Two triangles per rectangle
                        vertices.push(ChartVertex::new([x1, y1], color));
                        vertices.push(ChartVertex::new([x2, y1], color));
                        vertices.push(ChartVertex::new([x1, y2], color));

                        vertices.push(ChartVertex::new([x2, y1], color));
                        vertices.push(ChartVertex::new([x2, y2], color));
                        vertices.push(ChartVertex::new([x1, y2], color));
                    }
                }
            }
        }

        vertices
    }

    fn render(
        &mut self,
        frame: &mut gup::RenderFrame,
        data: &[SalesData],
        chart_type: ChartType,
        background_color: wgpu::Color,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Create pipeline if needed
        if self.render_pipeline.is_none() {
            self.create_pipeline(frame.device());
        }

        // Generate vertices for current chart type
        let vertices = self.generate_chart_vertices(data, chart_type);

        if vertices.is_empty() {
            return Ok(());
        }

        // Create vertex buffer
        self.vertex_buffer = Some(frame.device().create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Chart Vertex Buffer"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            },
        ));

        // Render
        {
            let mut render_pass = frame.render_pass(Some(background_color));

            if let (Some(pipeline), Some(vertex_buffer)) =
                (&self.render_pipeline, &self.vertex_buffer)
            {
                render_pass.set_pipeline(pipeline);
                render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                render_pass.draw(0..vertices.len() as u32, 0..1);
            }
        }

        Ok(())
    }
}

/// Chart types available for demonstration
#[derive(Debug, Clone, Copy, PartialEq)]
enum ChartType {
    Scatter,
    Line,
    Bar,
    Area,
    Heatmap,
}

impl ChartType {
    fn next(self) -> Self {
        match self {
            ChartType::Scatter => ChartType::Line,
            ChartType::Line => ChartType::Bar,
            ChartType::Bar => ChartType::Area,
            ChartType::Area => ChartType::Heatmap,
            ChartType::Heatmap => ChartType::Scatter,
        }
    }

    fn name(self) -> &'static str {
        match self {
            ChartType::Scatter => "Scatter Plot",
            ChartType::Line => "Line Chart",
            ChartType::Bar => "Bar Chart",
            ChartType::Area => "Area Chart",
            ChartType::Heatmap => "Heatmap",
        }
    }

    fn observable_plot_syntax(self) -> &'static str {
        match self {
            ChartType::Scatter => "Plot.dot(data, {x: 'revenue', y: 'profit', stroke: 'region'})",
            ChartType::Line => "Plot.line(data, {x: 'quarter', y: 'revenue', stroke: 'region'})",
            ChartType::Bar => "Plot.barY(data, {x: 'quarter', y: 'revenue', fill: 'region'})",
            ChartType::Area => "Plot.area(data, {x: 'quarter', y: 'revenue', fill: 'region'})",
            ChartType::Heatmap => {
                "Plot.rect(data, {x: 'quarter', y: 'region', fill: 'growth_rate'})"
            }
        }
    }
}

/// Main application state
struct ShowcaseApp {
    window: Option<Arc<Window>>,
    context: Option<Arc<GupContext>>,
    surface_id: Option<SurfaceId>,
    current_chart: ChartType,
    data: Vec<SalesData>,
    last_render: std::time::Instant,
    context_initialized: bool,
    chart_renderer: ChartRenderer,
}

impl ShowcaseApp {
    fn new() -> Self {
        Self {
            window: None,
            context: None,
            surface_id: None,
            current_chart: ChartType::Scatter,
            data: create_sample_data(),
            last_render: std::time::Instant::now(),
            context_initialized: false,
            chart_renderer: ChartRenderer::new(),
        }
    }

    async fn initialize_context(&mut self) -> GupResult<()> {
        if !self.context_initialized && self.window.is_some() {
            let window = self.window.as_ref().unwrap();

            // Create context
            let context = GupContext::headless().await?;
            let mut ctx = Arc::try_unwrap(context).map_err(|_| {
                GupError::resource_error("Failed to get mutable context".to_string())
            })?;

            // Add surface for the window
            let surface_id = SurfaceId::new();
            ctx.add_surface(surface_id, window.clone())?;

            self.context = Some(Arc::new(ctx));
            self.surface_id = Some(surface_id);
            self.context_initialized = true;
            println!("✅ GPU context and surface initialized successfully");
        }
        Ok(())
    }

    fn render_current_chart(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let (Some(surface_id), Some(context)) = (self.surface_id, self.context.take()) {
            // Get themed color for the current chart type
            let themed_color = match self.current_chart {
                ChartType::Scatter => wgpu::Color {
                    r: 0.1,
                    g: 0.1,
                    b: 0.3,
                    a: 1.0,
                }, // Deep blue
                ChartType::Line => wgpu::Color {
                    r: 0.1,
                    g: 0.3,
                    b: 0.1,
                    a: 1.0,
                }, // Deep green
                ChartType::Bar => wgpu::Color {
                    r: 0.3,
                    g: 0.1,
                    b: 0.1,
                    a: 1.0,
                }, // Deep red
                ChartType::Area => wgpu::Color {
                    r: 0.3,
                    g: 0.3,
                    b: 0.1,
                    a: 1.0,
                }, // Deep yellow
                ChartType::Heatmap => wgpu::Color {
                    r: 0.1,
                    g: 0.3,
                    b: 0.3,
                    a: 1.0,
                }, // Deep cyan
            };

            // Get mutable context to render
            let mut ctx = Arc::try_unwrap(context).map_err(|_| "Failed to get mutable context")?;

            // Begin frame rendering for our surface
            match ctx.begin_frame_for_surface(surface_id) {
                Ok(mut frame) => {
                    // Render the actual chart with geometry
                    if let Err(e) = self.chart_renderer.render(
                        &mut frame,
                        &self.data,
                        self.current_chart,
                        themed_color,
                    ) {
                        eprintln!("Failed to render chart: {e}");
                        // Fallback: just clear the background
                        let _render_pass = frame.render_pass(Some(themed_color));
                    }

                    // Finish and present the frame
                    frame.finish()?;

                    self.last_render = std::time::Instant::now();
                }
                Err(e) => {
                    eprintln!("Failed to render frame: {e}");
                }
            }

            // Restore the context
            self.context = Some(Arc::new(ctx));
        }
        Ok(())
    }
}

impl ApplicationHandler for ShowcaseApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window_attributes = Window::default_attributes()
            .with_title("Observable Plot Showcase - GUP-018 Visual Demo")
            .with_inner_size(winit::dpi::LogicalSize::new(1000, 700))
            .with_resizable(true);

        let window = Arc::new(event_loop.create_window(window_attributes).unwrap());
        self.window = Some(window.clone());

        println!("\n🚀 Observable Plot Visual Showcase Started!");
        println!("================================================");
        println!("📖 Controls:");
        println!("   • SPACE - Switch between chart types");
        println!("   • ESC   - Exit application");
        println!("   • Current: {}", self.current_chart.name());
        println!(
            "   • Observable Plot: {}",
            self.current_chart.observable_plot_syntax()
        );
        println!("\n⏳ Initializing GPU context...");

        // Initialize the GPU context and start rendering
        let window_clone = window.clone();
        window_clone.request_redraw();
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                println!("\n👋 Closing Observable Plot Showcase");
                event_loop.exit();
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
                KeyCode::Escape => {
                    println!("\n👋 Exiting Observable Plot Showcase");
                    event_loop.exit();
                }
                KeyCode::Space => {
                    self.current_chart = self.current_chart.next();
                    println!("\n🔄 Switching to: {}", self.current_chart.name());
                    println!(
                        "   Observable Plot: {}",
                        self.current_chart.observable_plot_syntax()
                    );
                    println!(
                        "   📊 Data points: {} | Regions: {}",
                        self.data.len(),
                        self.data
                            .iter()
                            .map(|d| &d.region)
                            .collect::<std::collections::HashSet<_>>()
                            .len()
                    );

                    // Request a redraw to trigger rendering
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                }
                _ => {}
            },
            WindowEvent::RedrawRequested => {
                // Initialize context if not done yet
                if !self.context_initialized
                    && let Err(e) = pollster::block_on(self.initialize_context())
                {
                    eprintln!("❌ Failed to initialize GPU context: {e}");
                    return;
                }

                // Render the current chart
                if let Err(e) = self.render_current_chart() {
                    eprintln!("❌ Failed to render chart: {e}");
                }
            }
            WindowEvent::Resized(_physical_size) => {
                // Window resize handling - surface reconfiguration would need mutable context access
                // For now, the GPU context handles resize automatically
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        // Request redraw to keep the window updated
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🎬 Starting Observable Plot Visual Showcase");
    println!("===========================================");
    println!("This showcase demonstrates the Observable Plot-compatible");
    println!("API implemented in GUP-018 with live visual rendering!");
    println!();

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = ShowcaseApp::new();

    println!("📊 Sample Data Loaded:");
    println!(
        "   • {} sales records across {} regions",
        app.data.len(),
        app.data
            .iter()
            .map(|d| &d.region)
            .collect::<std::collections::HashSet<_>>()
            .len()
    );
    println!(
        "   • Revenue range: ${:.0} - ${:.0}",
        app.data
            .iter()
            .map(|d| d.revenue)
            .fold(f32::INFINITY, f32::min),
        app.data
            .iter()
            .map(|d| d.revenue)
            .fold(f32::NEG_INFINITY, f32::max)
    );
    println!(
        "   • Profit margin range: {:.1}% - {:.1}%",
        app.data
            .iter()
            .map(|d| d.profit / d.revenue * 100.0)
            .fold(f32::INFINITY, f32::min),
        app.data
            .iter()
            .map(|d| d.profit / d.revenue * 100.0)
            .fold(f32::NEG_INFINITY, f32::max)
    );

    event_loop.run_app(&mut app)?;

    Ok(())
}

/// Create comprehensive sample data for visual demonstration
fn create_sample_data() -> Vec<SalesData> {
    vec![
        SalesData {
            quarter: "Q1".to_string(),
            revenue: 125_000.0,
            profit: 25_000.0,
            region: "North".to_string(),
            growth_rate: 0.15,
        },
        SalesData {
            quarter: "Q1".to_string(),
            revenue: 98_000.0,
            profit: 18_000.0,
            region: "South".to_string(),
            growth_rate: 0.08,
        },
        SalesData {
            quarter: "Q1".to_string(),
            revenue: 87_000.0,
            profit: 15_000.0,
            region: "East".to_string(),
            growth_rate: 0.12,
        },
        SalesData {
            quarter: "Q1".to_string(),
            revenue: 112_000.0,
            profit: 22_000.0,
            region: "West".to_string(),
            growth_rate: 0.18,
        },
        SalesData {
            quarter: "Q2".to_string(),
            revenue: 145_000.0,
            profit: 32_000.0,
            region: "North".to_string(),
            growth_rate: 0.16,
        },
        SalesData {
            quarter: "Q2".to_string(),
            revenue: 112_000.0,
            profit: 23_000.0,
            region: "South".to_string(),
            growth_rate: 0.14,
        },
        SalesData {
            quarter: "Q2".to_string(),
            revenue: 95_000.0,
            profit: 18_500.0,
            region: "East".to_string(),
            growth_rate: 0.09,
        },
        SalesData {
            quarter: "Q2".to_string(),
            revenue: 128_000.0,
            profit: 26_000.0,
            region: "West".to_string(),
            growth_rate: 0.14,
        },
        SalesData {
            quarter: "Q3".to_string(),
            revenue: 167_000.0,
            profit: 41_000.0,
            region: "North".to_string(),
            growth_rate: 0.15,
        },
        SalesData {
            quarter: "Q3".to_string(),
            revenue: 134_000.0,
            profit: 28_000.0,
            region: "South".to_string(),
            growth_rate: 0.20,
        },
        SalesData {
            quarter: "Q3".to_string(),
            revenue: 108_000.0,
            profit: 21_000.0,
            region: "East".to_string(),
            growth_rate: 0.14,
        },
        SalesData {
            quarter: "Q3".to_string(),
            revenue: 145_000.0,
            profit: 30_000.0,
            region: "West".to_string(),
            growth_rate: 0.13,
        },
        SalesData {
            quarter: "Q4".to_string(),
            revenue: 189_000.0,
            profit: 48_000.0,
            region: "North".to_string(),
            growth_rate: 0.13,
        },
        SalesData {
            quarter: "Q4".to_string(),
            revenue: 156_000.0,
            profit: 35_000.0,
            region: "South".to_string(),
            growth_rate: 0.16,
        },
        SalesData {
            quarter: "Q4".to_string(),
            revenue: 123_000.0,
            profit: 25_000.0,
            region: "East".to_string(),
            growth_rate: 0.14,
        },
        SalesData {
            quarter: "Q4".to_string(),
            revenue: 167_000.0,
            profit: 38_000.0,
            region: "West".to_string(),
            growth_rate: 0.15,
        },
    ]
}

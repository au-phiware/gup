// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Interactive Axis-Tick Integration Visual Demo
//!
//! This example demonstrates the complete integration between the axis system
//! and automatic tick generation, showing:
//! - Live axis rendering with automatically positioned ticks
//! - Scatter plot with responsive axes that adjust to data
//! - Interactive data range adjustment with real-time axis updates
//! - Performance metrics for the integrated axis+tick system

use gup::{
    CircleAttributes, GupContext, PhysicalSize, SurfaceId,
    axis::{Axis, AxisBounds, AxisConfiguration, AxisPosition, LinearAxis},
    render::Vertex,
    prelude::ShaderFunction,
    shader_function::{Vec2, Vec4},
    tick_generator::{LinearScale, LinearTickGenerator, Scale, TickGenerator},
};
use std::sync::Arc;
use std::time::{Duration, Instant};
use wgpu::{Color, util::DeviceExt};
use winit::{
    application::ApplicationHandler,
    event::{ElementState, KeyEvent, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowAttributes, WindowId},
};

/// Sample data point for the scatter plot
#[derive(Debug, Clone)]
pub struct DataPoint {
    pub x: f32,
    pub y: f32,
    pub value: f32,
    pub size: f32,
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

/// Different data sets to demonstrate axis adaptation
#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(clippy::enum_variant_names)]
enum DataSet {
    SmallRange,
    LargeRange,
    NegativeRange,
    ScientificRange,
}

impl DataSet {
    fn next(&self) -> Self {
        match self {
            DataSet::SmallRange => DataSet::LargeRange,
            DataSet::LargeRange => DataSet::NegativeRange,
            DataSet::NegativeRange => DataSet::ScientificRange,
            DataSet::ScientificRange => DataSet::SmallRange,
        }
    }

    fn name(&self) -> &'static str {
        match self {
            DataSet::SmallRange => "Small Range (0-10)",
            DataSet::LargeRange => "Large Range (0-1000)",
            DataSet::NegativeRange => "Negative Range (-50 to 50)",
            DataSet::ScientificRange => "Scientific Range (0-1M)",
        }
    }

    fn generate_data(&self) -> Vec<DataPoint> {
        match self {
            DataSet::SmallRange => vec![
                DataPoint::new(1.0, 2.0, 0.3, 1.0, "A"),
                DataPoint::new(3.0, 4.5, 0.7, 1.2, "B"),
                DataPoint::new(5.5, 6.0, 0.5, 0.8, "C"),
                DataPoint::new(2.5, 7.5, 0.9, 1.5, "D"),
                DataPoint::new(7.0, 3.0, 0.2, 0.9, "E"),
                DataPoint::new(8.5, 8.0, 0.8, 1.1, "F"),
                DataPoint::new(4.0, 1.5, 0.4, 1.3, "G"),
                DataPoint::new(6.5, 5.5, 0.6, 1.0, "H"),
            ],
            DataSet::LargeRange => vec![
                DataPoint::new(100.0, 200.0, 0.3, 1.0, "Site 1"),
                DataPoint::new(300.0, 450.0, 0.7, 1.2, "Site 2"),
                DataPoint::new(550.0, 600.0, 0.5, 0.8, "Site 3"),
                DataPoint::new(250.0, 750.0, 0.9, 1.5, "Site 4"),
                DataPoint::new(700.0, 300.0, 0.2, 0.9, "Site 5"),
                DataPoint::new(850.0, 800.0, 0.8, 1.1, "Site 6"),
                DataPoint::new(400.0, 150.0, 0.4, 1.3, "Site 7"),
                DataPoint::new(650.0, 550.0, 0.6, 1.0, "Site 8"),
            ],
            DataSet::NegativeRange => vec![
                DataPoint::new(-30.0, -20.0, 0.3, 1.0, "Cold A"),
                DataPoint::new(-10.0, 15.0, 0.7, 1.2, "Mild B"),
                DataPoint::new(25.0, 30.0, 0.5, 0.8, "Warm C"),
                DataPoint::new(-25.0, 35.0, 0.9, 1.5, "Hot D"),
                DataPoint::new(20.0, -15.0, 0.2, 0.9, "Cool E"),
                DataPoint::new(40.0, 40.0, 0.8, 1.1, "Hot F"),
                DataPoint::new(-5.0, -35.0, 0.4, 1.3, "Cold G"),
                DataPoint::new(15.0, 25.0, 0.6, 1.0, "Warm H"),
            ],
            DataSet::ScientificRange => vec![
                DataPoint::new(10000.0, 20000.0, 0.3, 1.0, "Lab 1"),
                DataPoint::new(300000.0, 450000.0, 0.7, 1.2, "Lab 2"),
                DataPoint::new(550000.0, 600000.0, 0.5, 0.8, "Lab 3"),
                DataPoint::new(250000.0, 750000.0, 0.9, 1.5, "Lab 4"),
                DataPoint::new(700000.0, 300000.0, 0.2, 0.9, "Lab 5"),
                DataPoint::new(850000.0, 800000.0, 0.8, 1.1, "Lab 6"),
                DataPoint::new(400000.0, 150000.0, 0.4, 1.3, "Lab 7"),
                DataPoint::new(650000.0, 550000.0, 0.6, 1.0, "Lab 8"),
            ],
        }
    }

    fn get_bounds(&self) -> (f32, f32, f32, f32) {
        let data = self.generate_data();
        let mut min_x = f32::INFINITY;
        let mut max_x = f32::NEG_INFINITY;
        let mut min_y = f32::INFINITY;
        let mut max_y = f32::NEG_INFINITY;

        for point in &data {
            min_x = min_x.min(point.x);
            max_x = max_x.max(point.x);
            min_y = min_y.min(point.y);
            max_y = max_y.max(point.y);
        }

        // Add small padding
        let x_padding = (max_x - min_x) * 0.1;
        let y_padding = (max_y - min_y) * 0.1;

        (
            min_x - x_padding,
            max_x + x_padding,
            min_y - y_padding,
            max_y + y_padding,
        )
    }
}

/// Shader function that transforms DataPoint to CircleAttributes
pub struct DataPointToCircleAttributes {
    x_min: f32,
    x_max: f32,
    y_min: f32,
    y_max: f32,
}

impl DataPointToCircleAttributes {
    fn new(x_min: f32, x_max: f32, y_min: f32, y_max: f32) -> Self {
        Self {
            x_min,
            x_max,
            y_min,
            y_max,
        }
    }
}

impl ShaderFunction for DataPointToCircleAttributes {
    type Input = DataPoint;
    type Output = CircleAttributes;

    fn apply(&self, input: &Self::Input) -> Self::Output {
        // Normalize coordinates to chart area [-0.6, 0.6] leaving room for axes
        let screen_x = ((input.x - self.x_min) / (self.x_max - self.x_min)) * 1.2 - 0.6;
        let screen_y = ((input.y - self.y_min) / (self.y_max - self.y_min)) * 1.2 - 0.6;

        // Color based on value (blue to red gradient)
        let red = input.value;
        let blue = 1.0 - input.value;
        let green = 0.3;
        let alpha = 0.8;

        // Base size with scaling factor
        let radius = 0.02 * input.size;

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
            },
        }
    }

    fn wgsl_code(&self) -> String {
        format!(
            r#"
            fn datapoint_to_circle(data: DataPoint) -> CircleAttributes {{
                let screen_x = ((data.x - {}) / ({} - {})) * 1.2 - 0.6;
                let screen_y = ((data.y - {}) / ({} - {})) * 1.2 - 0.6;

                let red = data.value;
                let blue = 1.0 - data.value;
                let green = 0.3;
                let alpha = 0.8;

                let radius = 0.02 * data.size;

                var attrs: CircleAttributes;
                attrs.center = vec2<f32>(screen_x, screen_y);
                attrs.radius = radius;
                attrs.fill_color = vec4<f32>(red, green, blue, alpha);
                attrs.stroke_width = 1.0;
                attrs.stroke_color = vec4<f32>(0.0, 0.0, 0.0, 1.0);
                return attrs;
            }}
            "#,
            self.x_min, self.x_max, self.x_min, self.y_min, self.y_max, self.y_min
        )
    }

    fn function_id(&self) -> String {
        "datapoint_to_circle".to_string()
    }
}

/// Integrated axis and tick visualization renderer
struct AxisTickIntegrationRenderer {
    current_dataset: DataSet,
    data_points: Vec<DataPoint>,
    circle_attributes: Vec<CircleAttributes>,
    bottom_axis: LinearAxis,
    #[allow(dead_code)]
    left_axis: LinearAxis,
    last_update: Instant,
    performance_samples: Vec<Duration>,
    background_color: [f32; 4],
}

impl AxisTickIntegrationRenderer {
    fn new() -> Self {
        let mut renderer = Self {
            current_dataset: DataSet::SmallRange,
            data_points: Vec::new(),
            circle_attributes: Vec::new(),
            bottom_axis: LinearAxis::new(
                AxisPosition::Bottom,
                AxisConfiguration::default()
                    .with_color([0.8, 0.8, 0.8, 1.0])
                    .with_line_width(2.0)
                    .with_tick_lengths(8.0, 4.0),
            ),
            left_axis: LinearAxis::new(
                AxisPosition::Left,
                AxisConfiguration::default()
                    .with_color([0.8, 0.8, 0.8, 1.0])
                    .with_line_width(2.0)
                    .with_tick_lengths(8.0, 4.0),
            ),
            last_update: Instant::now(),
            performance_samples: Vec::new(),
            background_color: [0.02, 0.02, 0.05, 1.0], // Very dark blue
        };

        renderer.switch_dataset(DataSet::SmallRange);
        renderer
    }

    fn switch_dataset(&mut self, dataset: DataSet) {
        self.current_dataset = dataset;
        self.data_points = dataset.generate_data();

        let (x_min, x_max, y_min, y_max) = dataset.get_bounds();

        // Transform data points to circle attributes
        let transformer = DataPointToCircleAttributes::new(x_min, x_max, y_min, y_max);
        self.circle_attributes = self
            .data_points
            .iter()
            .map(|point| transformer.apply(point))
            .collect();
    }

    fn generate_axis_vertices(&self) -> (Vec<Vertex>, Duration) {
        let start = Instant::now();
        let mut vertices = Vec::new();

        let (x_min, x_max, y_min, y_max) = self.current_dataset.get_bounds();

        // Create scales for tick generation
        let x_scale = LinearScale::new(x_min as f64, x_max as f64);
        let y_scale = LinearScale::new(y_min as f64, y_max as f64);

        // Define axis bounds in screen coordinates
        let bottom_axis_bounds = AxisBounds::new(
            Vec2 { x: -0.6, y: -0.6 }, // Bottom left of chart area
            Vec2 { x: 0.6, y: -0.6 },  // Bottom right of chart area
            50.0,
        );

        let left_axis_bounds = AxisBounds::new(
            Vec2 { x: -0.6, y: -0.6 }, // Bottom left of chart area
            Vec2 { x: -0.6, y: 0.6 },  // Top left of chart area
            50.0,
        );

        // Generate ticks using the integrated system
        let generator = LinearTickGenerator::default();

        // Bottom axis (X-axis)
        let x_major_ticks = generator.generate_major_ticks(&x_scale, 800.0, None);
        let x_minor_ticks = generator.generate_minor_ticks(&x_scale, &x_major_ticks, 5);

        // Main axis line
        let axis_color = self.bottom_axis.configuration().line_color;
        vertices.push(Vertex {
            position: [bottom_axis_bounds.start.x, bottom_axis_bounds.start.y],
            color: axis_color,
        });
        vertices.push(Vertex {
            position: [bottom_axis_bounds.end.x, bottom_axis_bounds.end.y],
            color: axis_color,
        });

        // X-axis major ticks
        for &tick_value in &x_major_ticks {
            let normalized_pos = x_scale.normalize(tick_value) as f32;
            let screen_x = bottom_axis_bounds.start.x + normalized_pos * 1.2; // 1.2 = width of chart area

            if (-0.7..=0.7).contains(&screen_x) {
                vertices.push(Vertex {
                    position: [screen_x, bottom_axis_bounds.start.y],
                    color: [1.0, 0.6, 0.2, 1.0], // Orange
                });
                vertices.push(Vertex {
                    position: [screen_x, bottom_axis_bounds.start.y - 0.05],
                    color: [1.0, 0.6, 0.2, 1.0],
                });
            }
        }

        // X-axis minor ticks
        for &tick_value in &x_minor_ticks {
            let normalized_pos = x_scale.normalize(tick_value) as f32;
            let screen_x = bottom_axis_bounds.start.x + normalized_pos * 1.2;

            if (-0.7..=0.7).contains(&screen_x) {
                // Skip if too close to a major tick
                let too_close_to_major = x_major_ticks.iter().any(|&major| {
                    let major_pos = x_scale.normalize(major) as f32;
                    let major_screen_x = bottom_axis_bounds.start.x + major_pos * 1.2;
                    (screen_x - major_screen_x).abs() < 0.03
                });

                if !too_close_to_major {
                    vertices.push(Vertex {
                        position: [screen_x, bottom_axis_bounds.start.y],
                        color: [0.6, 0.6, 0.8, 0.7], // Light blue, transparent
                    });
                    vertices.push(Vertex {
                        position: [screen_x, bottom_axis_bounds.start.y - 0.03],
                        color: [0.6, 0.6, 0.8, 0.7],
                    });
                }
            }
        }

        // Left axis (Y-axis)
        let y_major_ticks = generator.generate_major_ticks(&y_scale, 600.0, None);
        let y_minor_ticks = generator.generate_minor_ticks(&y_scale, &y_major_ticks, 5);

        // Main axis line
        vertices.push(Vertex {
            position: [left_axis_bounds.start.x, left_axis_bounds.start.y],
            color: axis_color,
        });
        vertices.push(Vertex {
            position: [left_axis_bounds.end.x, left_axis_bounds.end.y],
            color: axis_color,
        });

        // Y-axis major ticks
        for &tick_value in &y_major_ticks {
            let normalized_pos = y_scale.normalize(tick_value) as f32;
            let screen_y = left_axis_bounds.start.y + normalized_pos * 1.2; // 1.2 = height of chart area

            if (-0.7..=0.7).contains(&screen_y) {
                vertices.push(Vertex {
                    position: [left_axis_bounds.start.x, screen_y],
                    color: [1.0, 0.6, 0.2, 1.0], // Orange
                });
                vertices.push(Vertex {
                    position: [left_axis_bounds.start.x - 0.05, screen_y],
                    color: [1.0, 0.6, 0.2, 1.0],
                });
            }
        }

        // Y-axis minor ticks
        for &tick_value in &y_minor_ticks {
            let normalized_pos = y_scale.normalize(tick_value) as f32;
            let screen_y = left_axis_bounds.start.y + normalized_pos * 1.2;

            if (-0.7..=0.7).contains(&screen_y) {
                // Skip if too close to a major tick
                let too_close_to_major = y_major_ticks.iter().any(|&major| {
                    let major_pos = y_scale.normalize(major) as f32;
                    let major_screen_y = left_axis_bounds.start.y + major_pos * 1.2;
                    (screen_y - major_screen_y).abs() < 0.03
                });

                if !too_close_to_major {
                    vertices.push(Vertex {
                        position: [left_axis_bounds.start.x, screen_y],
                        color: [0.6, 0.6, 0.8, 0.7], // Light blue, transparent
                    });
                    vertices.push(Vertex {
                        position: [left_axis_bounds.start.x - 0.03, screen_y],
                        color: [0.6, 0.6, 0.8, 0.7],
                    });
                }
            }
        }

        let duration = start.elapsed();
        (vertices, duration)
    }

    fn render(&mut self, frame: &mut gup::RenderFrame) -> Result<(), Box<dyn std::error::Error>> {
        // Clear background
        let clear_color = Color {
            r: self.background_color[0] as f64,
            g: self.background_color[1] as f64,
            b: self.background_color[2] as f64,
            a: self.background_color[3] as f64,
        };

        // Generate axis and tick vertices
        let (axis_vertices, axis_generation_time) = self.generate_axis_vertices();

        // Track performance
        self.performance_samples.push(axis_generation_time);
        if self.performance_samples.len() > 100 {
            self.performance_samples.remove(0);
        }

        // Convert circle attributes to renderable vertices
        let mut circle_vertices = Vec::new();

        for attrs in &self.circle_attributes {
            // Simple circle approximation using triangle fan approach
            let segments = 16;
            let center = attrs.center;
            let radius = attrs.radius;
            let color = attrs.fill_color;

            for i in 0..segments {
                let angle1 = (i as f32 / segments as f32) * 2.0 * std::f32::consts::PI;
                let angle2 = ((i + 1) as f32 / segments as f32) * 2.0 * std::f32::consts::PI;

                // Triangle: center, point1, point2
                circle_vertices.push(Vertex {
                    position: [center.x, center.y],
                    color: [color.x, color.y, color.z, color.w],
                });
                circle_vertices.push(Vertex {
                    position: [
                        center.x + radius * angle1.cos(),
                        center.y + radius * angle1.sin(),
                    ],
                    color: [color.x, color.y, color.z, color.w],
                });
                circle_vertices.push(Vertex {
                    position: [
                        center.x + radius * angle2.cos(),
                        center.y + radius * angle2.sin(),
                    ],
                    color: [color.x, color.y, color.z, color.w],
                });
            }
        }

        if axis_vertices.is_empty() && circle_vertices.is_empty() {
            let _render_pass = frame.render_pass(Some(clear_color));
            return Ok(());
        }

        // Create shader for rendering
        let shader = frame
            .device()
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("axis_tick_integration_shader"),
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

        // Create render pipeline layout (shared)
        let render_pipeline_layout =
            frame
                .device()
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("axis_tick_integration_pipeline_layout"),
                    bind_group_layouts: &[],
                    push_constant_ranges: &[],
                });

        // Create vertex buffer layout (shared)
        let vertex_buffer_layout = wgpu::VertexBufferLayout {
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
        };

        // Create buffers and pipelines before render pass
        let axis_vertex_buffer = if !axis_vertices.is_empty() {
            Some(
                frame
                    .device()
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("Axis Vertex Buffer"),
                        contents: bytemuck::cast_slice(&axis_vertices),
                        usage: wgpu::BufferUsages::VERTEX,
                    }),
            )
        } else {
            None
        };

        let circle_vertex_buffer = if !circle_vertices.is_empty() {
            Some(
                frame
                    .device()
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("Circle Vertex Buffer"),
                        contents: bytemuck::cast_slice(&circle_vertices),
                        usage: wgpu::BufferUsages::VERTEX,
                    }),
            )
        } else {
            None
        };

        let line_pipeline = if !axis_vertices.is_empty() {
            Some(
                frame
                    .device()
                    .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                        label: Some("axis_line_pipeline"),
                        layout: Some(&render_pipeline_layout),
                        vertex: wgpu::VertexState {
                            module: &shader,
                            entry_point: Some("vs_main"),
                            buffers: std::slice::from_ref(&vertex_buffer_layout),
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
                    }),
            )
        } else {
            None
        };

        let triangle_pipeline = if !circle_vertices.is_empty() {
            Some(
                frame
                    .device()
                    .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                        label: Some("circle_triangle_pipeline"),
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
                    }),
            )
        } else {
            None
        };

        // Begin render pass and render
        {
            let mut render_pass = frame.render_pass(Some(clear_color));

            // Render axis lines first (LineList topology)
            if let (Some(axis_buffer), Some(line_pip)) = (&axis_vertex_buffer, &line_pipeline) {
                render_pass.set_pipeline(line_pip);
                render_pass.set_vertex_buffer(0, axis_buffer.slice(..));
                render_pass.draw(0..axis_vertices.len() as u32, 0..1);
            }

            // Render circles second (TriangleList topology)
            if let (Some(circle_buffer), Some(triangle_pip)) =
                (&circle_vertex_buffer, &triangle_pipeline)
            {
                render_pass.set_pipeline(triangle_pip);
                render_pass.set_vertex_buffer(0, circle_buffer.slice(..));
                render_pass.draw(0..circle_vertices.len() as u32, 0..1);
            }
        }

        // Print stats to console
        if self.last_update.elapsed() > Duration::from_millis(500) {
            self.last_update = Instant::now();

            let avg_time = if !self.performance_samples.is_empty() {
                self.performance_samples
                    .iter()
                    .sum::<Duration>()
                    .as_micros() as f32
                    / self.performance_samples.len() as f32
            } else {
                0.0
            };

            let (x_min, x_max, y_min, y_max) = self.current_dataset.get_bounds();

            println!(
                "\n🎯 Axis-Tick Integration Demo - {}",
                self.current_dataset.name()
            );
            println!("=================================================");
            println!("Data range: X({x_min:.1} to {x_max:.1}), Y({y_min:.1} to {y_max:.1})");
            println!(
                "Data points: {} | Circle vertices: {}",
                self.data_points.len(),
                self.circle_attributes.len() * 48
            ); // 16 triangles * 3 vertices
            println!(
                "Axis generation time: {:.1}μs (avg: {:.1}μs)",
                axis_generation_time.as_micros(),
                avg_time
            );

            println!("\nVisible elements:");
            println!("• Scatter plot data (colored circles)");
            println!("• X-axis with major ticks (orange) and minor ticks (blue)");
            println!("• Y-axis with major ticks (orange) and minor ticks (blue)");
            println!("• Responsive axis scaling based on data bounds");

            println!("\nControls:");
            println!("  [1/2/3/4] - Switch datasets | [SPACE] - Cycle datasets | [ESC] - Exit");
        }

        Ok(())
    }
}

/// Main application for the axis-tick integration visual demo
struct AxisTickIntegrationDemoApp {
    context: Option<Arc<GupContext>>,
    window: Option<Arc<Window>>,
    surface_id: Option<SurfaceId>,
    renderer: Option<AxisTickIntegrationRenderer>,
}

impl AxisTickIntegrationDemoApp {
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
            .with_title("Gup Axis-Tick Integration Visual Demo - Responsive Chart System")
            .with_inner_size(winit::dpi::LogicalSize::new(1000, 800));

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

    fn initialize_renderer(&mut self) {
        self.renderer = Some(AxisTickIntegrationRenderer::new());
        println!("✅ Axis-tick integration renderer initialized");
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
                        eprintln!("❌ Failed to render axis-tick integration: {e}");
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

impl ApplicationHandler for AxisTickIntegrationDemoApp {
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

            println!("✅ Axis-tick integration visual demo window created!");
            println!("🎨 Demonstrating complete axis+tick+data integration...");
            println!();
            println!("Visual Elements:");
            println!("• Scatter plot with responsive data visualization");
            println!("• X-axis with automatic tick generation (bottom)");
            println!("• Y-axis with automatic tick generation (left)");
            println!("• Major ticks (orange) and minor ticks (blue)");
            println!("• Real-time axis bounds adaptation to data");
            println!();
            println!("Controls:");
            println!("  [1] - Small range dataset (0-10)");
            println!("  [2] - Large range dataset (0-1000)");
            println!("  [3] - Negative range dataset (-50 to 50)");
            println!("  [4] - Scientific range dataset (0-1M)");
            println!("  [SPACE] - Cycle through datasets");
            println!("  [ESC] - Exit");
            println!();
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
                println!("👋 Closing axis-tick integration demo");
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
                if let Some(renderer) = &mut self.renderer {
                    match key_code {
                        KeyCode::Escape => {
                            println!("👋 Escape pressed, closing demo");
                            event_loop.exit();
                        }
                        KeyCode::Digit1 => {
                            renderer.switch_dataset(DataSet::SmallRange);
                            println!("🔄 Switched to {}", DataSet::SmallRange.name());
                        }
                        KeyCode::Digit2 => {
                            renderer.switch_dataset(DataSet::LargeRange);
                            println!("🔄 Switched to {}", DataSet::LargeRange.name());
                        }
                        KeyCode::Digit3 => {
                            renderer.switch_dataset(DataSet::NegativeRange);
                            println!("🔄 Switched to {}", DataSet::NegativeRange.name());
                        }
                        KeyCode::Digit4 => {
                            renderer.switch_dataset(DataSet::ScientificRange);
                            println!("🔄 Switched to {}", DataSet::ScientificRange.name());
                        }
                        KeyCode::Space => {
                            let new_dataset = renderer.current_dataset.next();
                            renderer.switch_dataset(new_dataset);
                            println!("🔄 Switched to {}", new_dataset.name());
                        }
                        _ => {}
                    }
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

    println!("🚀 Gup Axis-Tick Integration Visual Demo");
    println!("=======================================");
    println!();
    println!("This interactive demo showcases the complete integration between:");
    println!("• Automatic tick generation algorithms");
    println!("• Professional axis rendering system");
    println!("• Data visualization with responsive scaling");
    println!("• Real-time performance optimization");
    println!();
    println!("Features demonstrated:");
    println!("• Scatter plots with axes that adapt to data bounds");
    println!("• Automatic tick positioning using Wilkinson's algorithm");
    println!("• Major and minor tick generation with optimal spacing");
    println!("• Multi-range datasets (small, large, negative, scientific)");
    println!("• Performance monitoring (target: <100μs axis generation)");
    println!();

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = AxisTickIntegrationDemoApp::new();
    event_loop.run_app(&mut app)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dataset_cycling() {
        let mut dataset = DataSet::SmallRange;
        dataset = dataset.next();
        assert_eq!(dataset, DataSet::LargeRange);
        dataset = dataset.next();
        assert_eq!(dataset, DataSet::NegativeRange);
        dataset = dataset.next();
        assert_eq!(dataset, DataSet::ScientificRange);
        dataset = dataset.next();
        assert_eq!(dataset, DataSet::SmallRange);
    }

    #[test]
    fn test_dataset_data_generation() {
        let dataset = DataSet::SmallRange;
        let data = dataset.generate_data();
        assert_eq!(data.len(), 8);

        // Verify data is within expected range
        for point in &data {
            assert!(point.x >= 0.0 && point.x <= 10.0);
            assert!(point.y >= 0.0 && point.y <= 10.0);
        }
    }

    #[test]
    fn test_bounds_calculation() {
        let dataset = DataSet::SmallRange;
        let (x_min, x_max, y_min, y_max) = dataset.get_bounds();

        // Should include padding
        assert!(x_min < 1.0); // Less than minimum data point
        assert!(x_max > 8.5); // Greater than maximum data point
        assert!(y_min < 1.5); // Less than minimum data point
        assert!(y_max > 8.0); // Greater than maximum data point
    }

    #[test]
    fn test_shader_function_transformation() {
        let transformer = DataPointToCircleAttributes::new(0.0, 10.0, 0.0, 10.0);
        let point = DataPoint::new(5.0, 5.0, 0.5, 1.0, "Test");

        let attrs = transformer.apply(&point);

        // Center point (5,5) in (0,10) range should map to (0,0) in screen space
        assert!((attrs.center.x - 0.0).abs() < 0.001);
        assert!((attrs.center.y - 0.0).abs() < 0.001);

        // Verify color mapping
        assert!((attrs.fill_color.x - 0.5).abs() < 0.001); // red = value
        assert!((attrs.fill_color.z - 0.5).abs() < 0.001); // blue = 1 - value
    }

    #[test]
    fn test_axis_tick_integration_renderer_creation() {
        let renderer = AxisTickIntegrationRenderer::new();
        assert_eq!(renderer.current_dataset, DataSet::SmallRange);
        assert_eq!(renderer.data_points.len(), 8);
        assert_eq!(renderer.circle_attributes.len(), 8);
        assert_eq!(renderer.background_color, [0.02, 0.02, 0.05, 1.0]);
    }
}

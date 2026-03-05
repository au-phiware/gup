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

//! # Scatter Plot Window — GupApp Example
//!
//! This example builds on `01_hello_chart` by displaying the scatter plot
//! in a window with GPU-accelerated rendering.  It uses [`GupApp`] to
//! eliminate all winit boilerplate — compare with `hello_world.rs` for the
//! same pattern.
//!
//! ## When to use `GupApp` vs manual `ApplicationHandler`
//!
//! Use **`GupApp`** when your example or application is a straightforward
//! single-window chart that only needs rendering and the built-in keyboard
//! shortcuts (Escape/Q to quit, F/F11 for fullscreen, S for screenshot).
//!
//! Use the **manual `ApplicationHandler`** approach when you need:
//! - Multiple windows (see `windowed_demo.rs`)
//! - Custom keyboard/mouse event handling (see `treemap_window.rs`)
//! - Fine-grained control over the event loop (see `surface_events_demo.rs`)
//!
//! ## What You'll Learn
//! - How to implement [`AppRenderer`] for your chart type
//! - How to launch a windowed application in a few lines with `GupApp`
//!
//! Run with: `cargo run --example 02_scatter_window`
//!
//! Controls (built-in):
//! - ESC or Q: Quit
//! - F / F11: Toggle fullscreen
//! - S: Save screenshot (PNG)

use gup::CircleAttributes;
use gup::app::{AppRenderer, GupApp};
use gup::mark::{Circle, Mark};
use gup::shader_function::{Vec2, Vec4};
use wgpu::Color;

// ========================================
// Step 1: Define your data structure
// ========================================
#[derive(Debug, Clone)]
struct Point {
    x: f32,
    y: f32,
    value: f32, // Used for coloring
}

impl Point {
    fn new(x: f32, y: f32, value: f32) -> Self {
        Self { x, y, value }
    }
}

// ========================================
// Step 2: Create sample data
// ========================================
fn create_sample_data() -> Vec<Point> {
    vec![
        Point::new(1.0, 2.0, 0.2),
        Point::new(2.0, 4.0, 0.4),
        Point::new(3.0, 3.0, 0.6),
        Point::new(4.0, 5.0, 0.8),
        Point::new(5.0, 4.5, 1.0),
        Point::new(1.5, 3.5, 0.3),
        Point::new(2.5, 2.5, 0.5),
        Point::new(3.5, 4.5, 0.7),
        Point::new(4.5, 3.5, 0.9),
    ]
}

// ========================================
// Step 3: Transform data to circle attributes
// ========================================
fn point_to_circle(point: &Point, data_range: &DataRange) -> CircleAttributes {
    // Normalize to screen space [-0.8, 0.8]
    let screen_x =
        ((point.x - data_range.x_min) / (data_range.x_max - data_range.x_min)) * 1.6 - 0.8;
    let screen_y =
        ((point.y - data_range.y_min) / (data_range.y_max - data_range.y_min)) * 1.6 - 0.8;

    // Color gradient based on value (blue to orange)
    let r = point.value;
    let g = 0.5 * (1.0 - point.value);
    let b = 1.0 - point.value;

    CircleAttributes {
        center: Vec2 {
            x: screen_x,
            y: screen_y,
        },
        radius: 0.05, // Fixed radius for all points
        fill_color: Vec4 {
            x: r,
            y: g,
            z: b,
            w: 0.85,
        },
        stroke_width: 2.0,
        stroke_color: Vec4 {
            x: 0.2,
            y: 0.2,
            z: 0.2,
            w: 1.0,
        },
    }
}

struct DataRange {
    x_min: f32,
    x_max: f32,
    y_min: f32,
    y_max: f32,
}

impl DataRange {
    fn from_data(data: &[Point]) -> Self {
        let x_min = data.iter().map(|p| p.x).fold(f32::INFINITY, f32::min);
        let x_max = data.iter().map(|p| p.x).fold(f32::NEG_INFINITY, f32::max);
        let y_min = data.iter().map(|p| p.y).fold(f32::INFINITY, f32::min);
        let y_max = data.iter().map(|p| p.y).fold(f32::NEG_INFINITY, f32::max);
        // Add padding
        let x_pad = (x_max - x_min) * 0.1;
        let y_pad = (y_max - y_min) * 0.1;
        Self {
            x_min: x_min - x_pad,
            x_max: x_max + x_pad,
            y_min: y_min - y_pad,
            y_max: y_max + y_pad,
        }
    }
}

// ========================================
// Step 4: Circle instance for GPU rendering
// ========================================
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

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct CircleVertex {
    position: [f32; 2],
    color: [f32; 4],
    local_pos: [f32; 2],
}

// ========================================
// Step 5: Simple scatter plot renderer
// ========================================
struct ScatterRenderer {
    circle_instances: Vec<CircleInstance>,
    vertex_buffer: Option<wgpu::Buffer>,
    instance_buffer: Option<wgpu::Buffer>,
    pipeline: Option<wgpu::RenderPipeline>,
    index_buffer: Option<wgpu::Buffer>,
}

impl ScatterRenderer {
    fn new(data: &[Point]) -> Self {
        let data_range = DataRange::from_data(data);
        let circle_instances: Vec<CircleInstance> = data
            .iter()
            .map(|point| {
                let attrs = point_to_circle(point, &data_range);
                CircleInstance {
                    center: [attrs.center.x, attrs.center.y],
                    radius: attrs.radius,
                    _padding1: 0.0,
                    fill_color: [
                        attrs.fill_color.x,
                        attrs.fill_color.y,
                        attrs.fill_color.z,
                        attrs.fill_color.w,
                    ],
                    stroke_width: attrs.stroke_width,
                    _padding2: [0.0; 3],
                    stroke_color: [
                        attrs.stroke_color.x,
                        attrs.stroke_color.y,
                        attrs.stroke_color.z,
                        attrs.stroke_color.w,
                    ],
                }
            })
            .collect();

        Self {
            circle_instances,
            vertex_buffer: None,
            instance_buffer: None,
            pipeline: None,
            index_buffer: None,
        }
    }

    fn render(&mut self, frame: &mut gup::RenderFrame) {
        use wgpu::util::DeviceExt;

        if self.circle_instances.is_empty() {
            return;
        }

        // Create vertex buffer (quad vertices for instanced rendering)
        if self.vertex_buffer.is_none() {
            let base_vertices = Circle::generate_vertices();
            let vertices: Vec<CircleVertex> = base_vertices
                .iter()
                .map(|v| CircleVertex {
                    position: v.position,
                    color: [1.0, 1.0, 1.0, 1.0],
                    local_pos: v.position,
                })
                .collect();

            self.vertex_buffer = Some(frame.device().create_buffer_init(
                &wgpu::util::BufferInitDescriptor {
                    label: Some("Scatter Vertex Buffer"),
                    contents: bytemuck::cast_slice(&vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                },
            ));
        }

        // Create instance buffer
        if self.instance_buffer.is_none() {
            self.instance_buffer = Some(frame.device().create_buffer_init(
                &wgpu::util::BufferInitDescriptor {
                    label: Some("Scatter Instance Buffer"),
                    contents: bytemuck::cast_slice(&self.circle_instances),
                    usage: wgpu::BufferUsages::VERTEX,
                },
            ));
        }

        // Create index buffer
        if self.index_buffer.is_none()
            && let Some(indices) = Circle::generate_indices()
        {
            self.index_buffer = Some(frame.device().create_buffer_init(
                &wgpu::util::BufferInitDescriptor {
                    label: Some("Scatter Index Buffer"),
                    contents: bytemuck::cast_slice(&indices),
                    usage: wgpu::BufferUsages::INDEX,
                },
            ));
        }

        // Create render pipeline
        if self.pipeline.is_none() {
            let shader = frame
                .device()
                .create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("scatter_shader"),
                    source: wgpu::ShaderSource::Wgsl(CIRCLE_SHADER.into()),
                });

            let layout = frame
                .device()
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("scatter_layout"),
                    bind_group_layouts: &[],
                    push_constant_ranges: &[],
                });

            self.pipeline = Some(frame.device().create_render_pipeline(
                &wgpu::RenderPipelineDescriptor {
                    label: Some("scatter_pipeline"),
                    layout: Some(&layout),
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
                                        offset: 8,
                                        shader_location: 1,
                                        format: wgpu::VertexFormat::Float32x4,
                                    },
                                    wgpu::VertexAttribute {
                                        offset: 24,
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
                                        offset: 8,
                                        shader_location: 4,
                                        format: wgpu::VertexFormat::Float32,
                                    },
                                    wgpu::VertexAttribute {
                                        offset: 16,
                                        shader_location: 5,
                                        format: wgpu::VertexFormat::Float32x4,
                                    },
                                    wgpu::VertexAttribute {
                                        offset: 32,
                                        shader_location: 6,
                                        format: wgpu::VertexFormat::Float32,
                                    },
                                    wgpu::VertexAttribute {
                                        offset: 48,
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
                        ..Default::default()
                    },
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState::default(),
                    multiview: None,
                    cache: None,
                },
            ));
        }

        // Render
        let clear_color = Color {
            r: 0.98,
            g: 0.98,
            b: 0.98,
            a: 1.0,
        };
        {
            let mut render_pass = frame.render_pass(Some(clear_color));

            if let (Some(vb), Some(ib), Some(pipeline)) =
                (&self.vertex_buffer, &self.instance_buffer, &self.pipeline)
            {
                render_pass.set_pipeline(pipeline);
                render_pass.set_vertex_buffer(0, vb.slice(..));
                render_pass.set_vertex_buffer(1, ib.slice(..));

                if let Some(idx_buf) = &self.index_buffer {
                    if let Some(indices) = Circle::generate_indices() {
                        render_pass.set_index_buffer(idx_buf.slice(..), wgpu::IndexFormat::Uint32);
                        render_pass.draw_indexed(
                            0..indices.len() as u32,
                            0,
                            0..self.circle_instances.len() as u32,
                        );
                    }
                } else {
                    let vert_count = Circle::vertex_count() as u32;
                    render_pass.draw(0..vert_count, 0..self.circle_instances.len() as u32);
                }
            }
        }
    }
}

const CIRCLE_SHADER: &str = r#"
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
"#;

impl AppRenderer for ScatterRenderer {
    fn render(&mut self, frame: &mut gup::RenderFrame) {
        ScatterRenderer::render(self, frame);
    }
}

// ========================================
// Step 6: Launch with GupApp (≤ 5 lines)
// ========================================

fn main() -> Result<(), gup::GupError> {
    let data = create_sample_data();
    GupApp::new(ScatterRenderer::new(&data))
        .title("Gup Scatter Plot")
        .run()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sample_data_creation() {
        let data = create_sample_data();
        assert!(!data.is_empty());
        assert!(data.len() >= 5);
    }

    #[test]
    fn test_data_range_calculation() {
        let data = create_sample_data();
        let range = DataRange::from_data(&data);
        assert!(range.x_max > range.x_min);
        assert!(range.y_max > range.y_min);
    }

    #[test]
    fn test_point_to_circle_produces_valid_coords() {
        let point = Point::new(3.0, 3.5, 0.5);
        let range = DataRange {
            x_min: 0.0,
            x_max: 6.0,
            y_min: 0.0,
            y_max: 6.0,
        };
        let attrs = point_to_circle(&point, &range);

        assert!(attrs.center.x >= -1.0 && attrs.center.x <= 1.0);
        assert!(attrs.center.y >= -1.0 && attrs.center.y <= 1.0);
        assert!(attrs.radius > 0.0);
    }

    #[test]
    fn test_renderer_creation() {
        let data = create_sample_data();
        let renderer = ScatterRenderer::new(&data);
        assert_eq!(renderer.circle_instances.len(), data.len());
    }
}

// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! # Tutorial 1 — Getting Started: Scatter Chart
//!
//! Renders the five-point scatter chart from
//! [Tutorial 1: Getting Started](../../docs/tutorials/01_getting_started.md).
//!
//! This is the simplest windowed tutorial example: five data points rendered
//! as blue circles using the [`GupApp`] shell so there is no manual event-loop
//! boilerplate.
//!
//! Run with: `cargo run --example tutorial01_scatter`
//!
//! Controls (built-in):
//! - ESC or Q: Quit
//! - F / F11: Toggle fullscreen
//! - S: Save screenshot (PNG)

use gup::app::{AppRenderer, GupApp};
use gup::mark::{Circle, Mark};

// ---------------------------------------------------------------------------
// Data — exactly the struct and values from Tutorial 1
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct Point {
    x: f32,
    y: f32,
}

fn tutorial_data() -> Vec<Point> {
    vec![
        Point { x: 1.0, y: 2.0 },
        Point { x: 2.0, y: 4.0 },
        Point { x: 3.0, y: 3.0 },
        Point { x: 4.0, y: 5.0 },
        Point { x: 5.0, y: 4.5 },
    ]
}

// ---------------------------------------------------------------------------
// GPU instance type matching the shader layout
// ---------------------------------------------------------------------------

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Instance {
    center: [f32; 2],
    radius: f32,
    _pad: f32,
    color: [f32; 4],
}

// ---------------------------------------------------------------------------
// Renderer
// ---------------------------------------------------------------------------

struct ScatterRenderer {
    instances: Vec<Instance>,
    vertex_buffer: Option<wgpu::Buffer>,
    instance_buffer: Option<wgpu::Buffer>,
    index_buffer: Option<wgpu::Buffer>,
    pipeline: Option<wgpu::RenderPipeline>,
    index_count: u32,
}

impl ScatterRenderer {
    fn new(data: &[Point]) -> Self {
        // Normalise data to clip-space [-0.8, 0.8]
        let x_min = data.iter().map(|p| p.x).fold(f32::INFINITY, f32::min);
        let x_max = data.iter().map(|p| p.x).fold(f32::NEG_INFINITY, f32::max);
        let y_min = data.iter().map(|p| p.y).fold(f32::INFINITY, f32::min);
        let y_max = data.iter().map(|p| p.y).fold(f32::NEG_INFINITY, f32::max);
        let pad_x = (x_max - x_min) * 0.15;
        let pad_y = (y_max - y_min) * 0.15;

        let instances: Vec<Instance> = data
            .iter()
            .map(|p| {
                let cx = ((p.x - x_min + pad_x) / (x_max - x_min + 2.0 * pad_x)) * 1.6 - 0.8;
                let cy = ((p.y - y_min + pad_y) / (y_max - y_min + 2.0 * pad_y)) * 1.6 - 0.8;
                Instance {
                    center: [cx, cy],
                    radius: 0.05,
                    _pad: 0.0,
                    // Tutorial 1 colour: [0.2, 0.6, 0.9, 1.0]
                    color: [0.2, 0.6, 0.9, 1.0],
                }
            })
            .collect();

        Self {
            instances,
            vertex_buffer: None,
            instance_buffer: None,
            index_buffer: None,
            pipeline: None,
            index_count: 0,
        }
    }

    fn init_gpu(&mut self, device: &wgpu::Device) {
        use wgpu::util::DeviceExt;

        // Circle geometry
        let verts: Vec<[f32; 2]> = Circle::generate_vertices()
            .iter()
            .map(|v| v.position)
            .collect();
        let indices = Circle::generate_indices();
        self.index_count = indices
            .as_ref()
            .map_or(Circle::vertex_count() as u32, |i| i.len() as u32);

        self.vertex_buffer = Some(
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("tut01_vb"),
                contents: bytemuck::cast_slice(&verts),
                usage: wgpu::BufferUsages::VERTEX,
            }),
        );
        self.instance_buffer = Some(
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("tut01_ib"),
                contents: bytemuck::cast_slice(&self.instances),
                usage: wgpu::BufferUsages::VERTEX,
            }),
        );
        if let Some(ref idx) = indices {
            self.index_buffer = Some(device.create_buffer_init(
                &wgpu::util::BufferInitDescriptor {
                    label: Some("tut01_idx"),
                    contents: bytemuck::cast_slice(idx),
                    usage: wgpu::BufferUsages::INDEX,
                },
            ));
        }

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("tut01_shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("tut01_layout"),
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });
        self.pipeline = Some(
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("tut01_pipeline"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &[
                        // Per-vertex: local position
                        wgpu::VertexBufferLayout {
                            array_stride: std::mem::size_of::<[f32; 2]>() as u64,
                            step_mode: wgpu::VertexStepMode::Vertex,
                            attributes: &[wgpu::VertexAttribute {
                                offset: 0,
                                shader_location: 0,
                                format: wgpu::VertexFormat::Float32x2,
                            }],
                        },
                        // Per-instance: center, radius, _pad, color
                        wgpu::VertexBufferLayout {
                            array_stride: std::mem::size_of::<Instance>() as u64,
                            step_mode: wgpu::VertexStepMode::Instance,
                            attributes: &[
                                wgpu::VertexAttribute {
                                    offset: 0,
                                    shader_location: 1,
                                    format: wgpu::VertexFormat::Float32x2,
                                },
                                wgpu::VertexAttribute {
                                    offset: 8,
                                    shader_location: 2,
                                    format: wgpu::VertexFormat::Float32,
                                },
                                wgpu::VertexAttribute {
                                    offset: 16,
                                    shader_location: 3,
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
            }),
        );
    }
}

impl AppRenderer for ScatterRenderer {
    fn render(&mut self, frame: &mut gup::RenderFrame) {
        if self.pipeline.is_none() {
            self.init_gpu(frame.device());
        }

        let bg = wgpu::Color {
            r: 0.97,
            g: 0.97,
            b: 0.98,
            a: 1.0,
        };
        {
            let mut pass = frame.render_pass(Some(bg));
            if let (Some(pl), Some(vb), Some(ib)) =
                (&self.pipeline, &self.vertex_buffer, &self.instance_buffer)
            {
                pass.set_pipeline(pl);
                pass.set_vertex_buffer(0, vb.slice(..));
                pass.set_vertex_buffer(1, ib.slice(..));
                if let Some(idx) = &self.index_buffer {
                    pass.set_index_buffer(idx.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..self.index_count, 0, 0..self.instances.len() as u32);
                } else {
                    pass.draw(0..self.index_count, 0..self.instances.len() as u32);
                }
            }
        }
    }
}

const SHADER: &str = r"
struct Out {
    @builtin(position) pos: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) uv: vec2<f32>,
};
@vertex
fn vs_main(
    @location(0) local: vec2<f32>,
    @location(1) center: vec2<f32>,
    @location(2) radius: f32,
    @location(3) color: vec4<f32>,
) -> Out {
    var o: Out;
    o.pos = vec4<f32>(local * radius + center, 0.0, 1.0);
    o.color = color;
    o.uv = local;
    return o;
}
@fragment
fn fs_main(v: Out) -> @location(0) vec4<f32> {
    let d = length(v.uv);
    let a = 1.0 - smoothstep(0.85, 1.0, d);
    if a < 0.01 { discard; }
    return vec4<f32>(v.color.rgb, v.color.a * a);
}
";

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

fn main() -> Result<(), gup::GupError> {
    GupApp::new(ScatterRenderer::new(&tutorial_data()))
        .title("Tutorial 1 — Getting Started: Scatter Chart")
        .size(800, 600)
        .run()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tutorial_data_has_five_points() {
        assert_eq!(tutorial_data().len(), 5);
    }

    #[test]
    fn renderer_creates_correct_instance_count() {
        let data = tutorial_data();
        let renderer = ScatterRenderer::new(&data);
        assert_eq!(renderer.instances.len(), 5);
    }

    #[test]
    fn instances_have_tutorial_colour() {
        let renderer = ScatterRenderer::new(&tutorial_data());
        for inst in &renderer.instances {
            assert_eq!(inst.color, [0.2, 0.6, 0.9, 1.0]);
        }
    }
}

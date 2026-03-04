// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! # Hello World — Minimal Gup Desktop Application
//!
//! This example demonstrates [`GupApp`] — an opinionated application shell
//! that handles the entire winit event loop so you can display a chart in a
//! native window with just a few lines of code.
//!
//! ## Built-in Keyboard Shortcuts
//!
//! | Key             | Action                     |
//! |-----------------|----------------------------|
//! | `Escape` / `Q`  | Quit                       |
//! | `F` / `F11`     | Toggle fullscreen           |
//! | `S`             | Save screenshot (PNG)       |
//!
//! Run with: `cargo run --example hello_world`

use gup::RenderFrame;
use gup::app::{AppRenderer, GupApp};
use gup::mark::{Circle, Mark};

// ---------------------------------------------------------------------------
// A tiny scatter-plot renderer — all GPU setup happens on first draw.
// ---------------------------------------------------------------------------

struct ScatterChart {
    pipeline: Option<wgpu::RenderPipeline>,
    vertex_buffer: Option<wgpu::Buffer>,
    instance_buffer: Option<wgpu::Buffer>,
    index_buffer: Option<wgpu::Buffer>,
    num_instances: u32,
    index_count: u32,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    pos: [f32; 2],
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Instance {
    center: [f32; 2],
    radius: f32,
    _pad: f32,
    color: [f32; 4],
}

impl ScatterChart {
    fn new() -> Self {
        Self {
            pipeline: None,
            vertex_buffer: None,
            instance_buffer: None,
            index_buffer: None,
            num_instances: 0,
            index_count: 0,
        }
    }

    /// One-time GPU resource initialisation on first render.
    fn init(&mut self, device: &wgpu::Device) {
        use wgpu::util::DeviceExt;

        // Sample data: 9 points in a grid pattern
        let instances: Vec<Instance> = [
            (-0.6, -0.4, 0.06, [0.26, 0.56, 0.87, 0.9]),
            (-0.3, 0.2, 0.05, [0.90, 0.36, 0.27, 0.9]),
            (0.0, -0.1, 0.07, [0.18, 0.73, 0.49, 0.9]),
            (0.3, 0.4, 0.05, [0.93, 0.72, 0.15, 0.9]),
            (0.6, 0.1, 0.06, [0.62, 0.32, 0.78, 0.9]),
            (-0.4, 0.5, 0.04, [0.90, 0.49, 0.13, 0.9]),
            (0.1, 0.6, 0.05, [0.16, 0.50, 0.73, 0.9]),
            (0.5, -0.3, 0.06, [0.83, 0.33, 0.33, 0.9]),
            (-0.2, -0.5, 0.04, [0.35, 0.71, 0.46, 0.9]),
        ]
        .iter()
        .map(|&(cx, cy, r, col)| Instance {
            center: [cx, cy],
            radius: r,
            _pad: 0.0,
            color: col,
        })
        .collect();

        self.num_instances = instances.len() as u32;

        let verts: Vec<Vertex> = Circle::generate_vertices()
            .iter()
            .map(|v| Vertex { pos: v.position })
            .collect();
        let indices = Circle::generate_indices();
        self.index_count = indices
            .as_ref()
            .map_or(Circle::vertex_count() as u32, |i| i.len() as u32);

        self.vertex_buffer = Some(
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("hello_vb"),
                contents: bytemuck::cast_slice(&verts),
                usage: wgpu::BufferUsages::VERTEX,
            }),
        );
        self.instance_buffer = Some(
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("hello_ib"),
                contents: bytemuck::cast_slice(&instances),
                usage: wgpu::BufferUsages::VERTEX,
            }),
        );
        if let Some(ref idx) = indices {
            self.index_buffer = Some(device.create_buffer_init(
                &wgpu::util::BufferInitDescriptor {
                    label: Some("hello_idx"),
                    contents: bytemuck::cast_slice(idx),
                    usage: wgpu::BufferUsages::INDEX,
                },
            ));
        }

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("hello_shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("hello_layout"),
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });
        self.pipeline = Some(
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("hello_pipeline"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &[
                        wgpu::VertexBufferLayout {
                            array_stride: std::mem::size_of::<Vertex>() as u64,
                            step_mode: wgpu::VertexStepMode::Vertex,
                            attributes: &[wgpu::VertexAttribute {
                                offset: 0,
                                shader_location: 0,
                                format: wgpu::VertexFormat::Float32x2,
                            }],
                        },
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

impl AppRenderer for ScatterChart {
    fn render(&mut self, frame: &mut RenderFrame) {
        if self.pipeline.is_none() {
            self.init(frame.device());
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
                    pass.draw_indexed(0..self.index_count, 0, 0..self.num_instances);
                } else {
                    pass.draw(0..self.index_count, 0..self.num_instances);
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
// main — the GupApp one-liner
// ---------------------------------------------------------------------------

fn main() -> Result<(), gup::GupError> {
    GupApp::new(ScatterChart::new())
        .title("Hello Gup!")
        .size(800, 600)
        .run()
}

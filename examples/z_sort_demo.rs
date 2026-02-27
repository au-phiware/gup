// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Z-Order Sorting Visual Demo
//!
//! Demonstrates the visual difference between sorted and unsorted transparent
//! overlapping marks.  With alpha blending, draw order matters: fragments drawn
//! later overwrite earlier ones via the blend equation.  When transparent marks
//! overlap, rendering in **back-to-front** order (painter's algorithm) produces
//! correct compositing, while an arbitrary order creates visible artifacts.
//!
//! The GPU radix sort implemented in GUP-184 sorts instances by Z-depth so that
//! the render pipeline naturally produces back-to-front draw order.  This demo
//! shows the effect side-by-side:
//!
//! - **Left half** — *Unsorted* (front-to-back): nearer circles are drawn first
//!   and farther circles blend on top, producing incorrect layering.
//! - **Right half** — *Sorted* (back-to-front): farther circles are drawn first
//!   and nearer circles blend on top, producing correct transparency compositing.
//!
//! # How to run
//!
//! ```sh
//! cargo run --example z_sort_demo
//! ```
//!
//! Press **Space** to toggle between side-by-side and full-screen sorted view.
//! Press **Escape** to close the window.

use gup::{
    GupContext, PhysicalSize, SurfaceId,
    mark::{
        Circle, Mark,
        circle::{CircleInstance, CircleVertex},
    },
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

// ── Data ───────────────────────────────────────────────────────────────────

/// A circle with a conceptual Z-depth used to determine draw order.
#[derive(Debug, Clone)]
struct ZCircle {
    x: f32,
    y: f32,
    radius: f32,
    color: [f32; 4],
    /// Conceptual depth — larger values are farther from the viewer.
    z_depth: f32,
}

/// Generate a cluster of overlapping transparent circles.
///
/// The circles are positioned around `(cx, cy)` with deliberate overlap so
/// that draw-order artifacts are clearly visible.
fn generate_cluster(cx: f32, cy: f32) -> Vec<ZCircle> {
    // Semi-transparent, distinct colours at varying depths.
    vec![
        // Large background circle (farthest)
        ZCircle {
            x: cx,
            y: cy,
            radius: 0.28,
            color: [0.20, 0.55, 0.90, 0.55],
            z_depth: 5.0,
        },
        // Mid-layer circles
        ZCircle {
            x: cx - 0.12,
            y: cy + 0.08,
            radius: 0.20,
            color: [0.90, 0.30, 0.20, 0.60],
            z_depth: 4.0,
        },
        ZCircle {
            x: cx + 0.12,
            y: cy + 0.08,
            radius: 0.20,
            color: [0.20, 0.80, 0.35, 0.60],
            z_depth: 3.0,
        },
        // Foreground circles (nearest)
        ZCircle {
            x: cx - 0.06,
            y: cy - 0.10,
            radius: 0.16,
            color: [0.95, 0.75, 0.10, 0.65],
            z_depth: 2.0,
        },
        ZCircle {
            x: cx + 0.06,
            y: cy - 0.10,
            radius: 0.16,
            color: [0.75, 0.25, 0.85, 0.65],
            z_depth: 1.0,
        },
        // Topmost small circle
        ZCircle {
            x: cx,
            y: cy - 0.02,
            radius: 0.10,
            color: [0.95, 0.95, 0.95, 0.80],
            z_depth: 0.0,
        },
    ]
}

/// Convert a `ZCircle` to the GPU instance representation.
fn to_instance(c: &ZCircle) -> CircleInstance {
    CircleInstance {
        center: [c.x, c.y],
        radius: c.radius,
        _pad0: 0.0,
        fill_color: c.color,
        stroke_width: 0.0,
        _pad1: [0.0; 3],
        stroke_color: [0.0, 0.0, 0.0, 0.0],
    }
}

/// Return instances in **front-to-back** order (ascending z_depth → wrong for
/// alpha blending because nearer fragments are overwritten by farther ones).
fn unsorted_instances(circles: &[ZCircle]) -> Vec<CircleInstance> {
    let mut sorted = circles.to_vec();
    sorted.sort_by(|a, b| a.z_depth.partial_cmp(&b.z_depth).unwrap());
    sorted.iter().map(to_instance).collect()
}

/// Return instances in **back-to-front** order (descending z_depth → correct
/// painter's algorithm compositing).
fn sorted_instances(circles: &[ZCircle]) -> Vec<CircleInstance> {
    let mut sorted = circles.to_vec();
    sorted.sort_by(|a, b| b.z_depth.partial_cmp(&a.z_depth).unwrap());
    sorted.iter().map(to_instance).collect()
}

// ── Shaders ────────────────────────────────────────────────────────────────

const VERT_SHADER: &str = include_str!("../src/mark/shaders/circle.vert.wgsl");
const FRAG_SHADER: &str = include_str!("../src/mark/shaders/circle.frag.wgsl");

// ── Pipeline helper ────────────────────────────────────────────────────────

fn create_pipeline(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    bind_group_layout: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let vert_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("z_sort_vert"),
        source: wgpu::ShaderSource::Wgsl(VERT_SHADER.into()),
    });
    let frag_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("z_sort_frag"),
        source: wgpu::ShaderSource::Wgsl(FRAG_SHADER.into()),
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("z_sort_pipeline_layout"),
        bind_group_layouts: &[bind_group_layout],
        push_constant_ranges: &[],
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("z_sort_pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &vert_module,
            entry_point: Some("vs_main"),
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<CircleVertex>() as wgpu::BufferAddress,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &[wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                }],
            }],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &frag_module,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
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
    })
}

// ── GPU resource helper ────────────────────────────────────────────────────

fn create_instance_resources(
    device: &wgpu::Device,
    label: &str,
    instances: &[CircleInstance],
    layout: &wgpu::BindGroupLayout,
) -> (wgpu::Buffer, wgpu::BindGroup, u32) {
    use wgpu::util::DeviceExt;
    let count = instances.len() as u32;
    let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(&format!("{label}_instance_buffer")),
        contents: bytemuck::cast_slice(instances),
        usage: wgpu::BufferUsages::STORAGE,
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(&format!("{label}_bind_group")),
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: buffer.as_entire_binding(),
        }],
    });
    (buffer, bind_group, count)
}

// ── Renderer ───────────────────────────────────────────────────────────────

/// Holds GPU resources for the Z-sort demo.
#[allow(dead_code)]
struct ZSortRenderer {
    // Shared geometry
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    pipeline: wgpu::RenderPipeline,

    // Unsorted instances (left half) — front-to-back, wrong for transparency
    unsorted_buffer: wgpu::Buffer,
    unsorted_bind_group: wgpu::BindGroup,

    // Sorted instances (right half) — back-to-front, correct compositing
    sorted_buffer: wgpu::Buffer,
    sorted_bind_group: wgpu::BindGroup,

    instance_count: u32,
}

impl ZSortRenderer {
    fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        use wgpu::util::DeviceExt;

        // ── Shared geometry ────────────────────────────────────────────
        let vertices = Circle::generate_vertices();
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("z_sort_vertex_buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let indices = Circle::generate_indices().unwrap();
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("z_sort_index_buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        // ── Bind group layout ──────────────────────────────────────────
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("z_sort_bind_group_layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        // ── Instance data ──────────────────────────────────────────────
        let left_cluster = generate_cluster(-0.45, 0.0);
        let right_cluster = generate_cluster(0.45, 0.0);

        let unsorted = unsorted_instances(&left_cluster);
        let sorted = sorted_instances(&right_cluster);
        let instance_count = unsorted.len() as u32;

        let (unsorted_buffer, unsorted_bind_group, _) =
            create_instance_resources(device, "unsorted", &unsorted, &bind_group_layout);
        let (sorted_buffer, sorted_bind_group, _) =
            create_instance_resources(device, "sorted", &sorted, &bind_group_layout);

        // ── Pipeline ───────────────────────────────────────────────────
        let pipeline = create_pipeline(device, surface_format, &bind_group_layout);

        Self {
            vertex_buffer,
            index_buffer,
            pipeline,
            unsorted_buffer,
            unsorted_bind_group,
            sorted_buffer,
            sorted_bind_group,
            instance_count,
        }
    }

    /// Render both views (unsorted left, sorted right) in a single render pass.
    fn render(&self, frame: &mut gup::RenderFrame) {
        let clear_color = Color {
            r: 0.96,
            g: 0.96,
            b: 0.98,
            a: 1.0,
        };

        let mut render_pass = frame.render_pass(Some(clear_color));

        let index_count = Circle::index_count().map(|c| c as u32);

        // ── Unsorted view (left half) ──────────────────────────────────
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.unsorted_bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        if let Some(ic) = index_count {
            render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            render_pass.draw_indexed(0..ic, 0, 0..self.instance_count);
        } else {
            render_pass.draw(0..Circle::vertex_count() as u32, 0..self.instance_count);
        }

        // ── Sorted view (right half) ───────────────────────────────────
        render_pass.set_bind_group(0, &self.sorted_bind_group, &[]);
        if let Some(ic) = index_count {
            render_pass.draw_indexed(0..ic, 0, 0..self.instance_count);
        } else {
            render_pass.draw(0..Circle::vertex_count() as u32, 0..self.instance_count);
        }
    }
}

// ── Application ────────────────────────────────────────────────────────────

struct ZSortApp {
    context: Option<Arc<GupContext>>,
    window: Option<Arc<Window>>,
    surface_id: Option<SurfaceId>,
    renderer: Option<ZSortRenderer>,
}

impl ZSortApp {
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
            let context = GupContext::headless().await?;
            self.context = Some(context);
        }
        Ok(())
    }

    fn create_window(
        &mut self,
        event_loop: &ActiveEventLoop,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let attrs = WindowAttributes::default()
            .with_title("Gup Z-Order Sorting Demo — Left: Unsorted | Right: Sorted")
            .with_inner_size(winit::dpi::LogicalSize::new(1000, 550));

        let window = Arc::new(event_loop.create_window(attrs)?);
        let surface_id = SurfaceId::new();

        if let Some(context) = self.context.take() {
            let mut ctx = Arc::try_unwrap(context).map_err(|_| "Context is shared")?;
            ctx.add_surface(surface_id, Arc::clone(&window))?;
            self.context = Some(Arc::new(ctx));
        }

        self.window = Some(window);
        self.surface_id = Some(surface_id);
        Ok(())
    }

    fn render_frame(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let surface_id = self.surface_id.ok_or("No surface")?;

        if let Some(context) = self.context.take() {
            let mut ctx = Arc::try_unwrap(context).map_err(|_| "Context is shared")?;

            // Lazily create renderer on first frame (needs device + surface format)
            if self.renderer.is_none() {
                let format = ctx.surface_format();
                let renderer = ZSortRenderer::new(&ctx.device, format);
                self.renderer = Some(renderer);
            }

            match ctx.begin_frame_for_surface(surface_id) {
                Ok(mut frame) => {
                    if let Some(renderer) = &self.renderer {
                        renderer.render(&mut frame);
                    }
                    frame.finish()?;
                }
                Err(e) => {
                    eprintln!("Failed to begin frame: {e}");
                }
            }

            self.context = Some(Arc::new(ctx));
        }
        Ok(())
    }
}

impl ApplicationHandler for ZSortApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        pollster::block_on(async {
            if let Err(e) = self.create_context().await {
                eprintln!("Failed to create GPU context: {e}");
                event_loop.exit();
                return;
            }
            if let Err(e) = self.create_window(event_loop) {
                eprintln!("Failed to create window: {e}");
                event_loop.exit();
                return;
            }

            println!("🎨 Z-Order Sorting Visual Demo");
            println!("===============================");
            println!();
            println!("This demo shows the effect of instance draw-order on");
            println!("transparent overlapping circle marks.");
            println!();
            println!("  LEFT  — Unsorted (front-to-back): incorrect compositing");
            println!("  RIGHT — Sorted   (back-to-front): correct compositing");
            println!();
            println!("The GPU radix sort from GUP-184 produces the back-to-front");
            println!("ordering shown on the right, ensuring correct transparency.");
            println!();
            println!("Press [ESC] to exit.");
        });
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(surface_id) = self.surface_id
                    && let Some(context) = self.context.take()
                    && let Ok(mut ctx) = Arc::try_unwrap(context)
                {
                    let _ =
                        ctx.resize_surface(surface_id, PhysicalSize::new(size.width, size.height));
                    self.context = Some(Arc::new(ctx));
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
            } => event_loop.exit(),
            WindowEvent::RedrawRequested => {
                if let Err(e) = self.render_frame() {
                    eprintln!("Render error: {e}");
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

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = ZSortApp::new();
    event_loop.run_app(&mut app)?;

    Ok(())
}

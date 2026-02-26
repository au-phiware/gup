// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Multi-Pass Mark Rendering Demo
//!
//! Demonstrates multi-pass rendering using the `MultiPassConfig` and
//! `MultiPassRenderer` APIs from `gup::mark::advanced_rendering`.
//!
//! # What is multi-pass rendering?
//!
//! Multi-pass rendering issues **multiple draw calls** with different pipeline
//! configurations within a **single GPU render pass**.  Each draw call can use a
//! different blend state, polygon mode, or shader entry point.  This enables
//! effects like:
//!
//! - **Fill + outline**: One pass fills the interior, another draws just the
//!   stroke ring.
//! - **Drop shadow + main**: A first pass renders a blurred, offset shadow;
//!   a second pass renders the crisp main shape on top.
//!
//! The project convention is "one render pass per frame" (single
//! `begin_render_pass` call on the command encoder), so all draw calls happen
//! inside that pass — no extra render pass objects are created.
//!
//! # How to run
//!
//! ```sh
//! cargo run --example multi_pass_mark_demo
//! ```
//!
//! Press **Escape** to close the window.
//!
//! # Multi-pass pattern
//!
//! ```rust,ignore
//! use gup::mark::advanced_rendering::*;
//!
//! // 1. Configure passes with different entry points / blend states
//! let config = MultiPassConfig::new()
//!     .add_pass(RenderPassConfig {
//!         label: "shadow".into(),
//!         vertex_entry_point: Some("vs_shadow".into()),
//!         fragment_entry_point: Some("fs_shadow".into()),
//!         ..Default::default()
//!     })
//!     .add_pass(RenderPassConfig {
//!         label: "main".into(),
//!         ..Default::default()
//!     });
//!
//! // 2. Create one pipeline per pass
//! let pipelines: Vec<RenderPipeline> = config.passes().iter()
//!     .map(|p| create_pipeline(device, p))
//!     .collect();
//!
//! // 3. Render all passes in a single render pass
//! let renderer = MultiPassRenderer::new();
//! renderer.render_multi_pass(
//!     &mut render_pass, &config, &pipelines,
//!     &bind_group, &vertex_buffer,
//!     Some(&index_buffer), vertex_count, Some(index_count),
//!     instance_count,
//! )?;
//! ```

use gup::{
    GupContext, PhysicalSize, SurfaceId,
    mark::{
        Circle, Mark,
        advanced_rendering::{MultiPassConfig, MultiPassRenderer, RenderPassConfig},
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

/// Sample data point for the demo.
#[derive(Debug, Clone)]
struct DemoPoint {
    x: f32,
    y: f32,
    radius: f32,
    color: [f32; 4],
    stroke_width: f32,
    stroke_color: [f32; 4],
}

/// Generate a grid of circles positioned for the shadow demo (left half).
fn generate_shadow_data() -> Vec<DemoPoint> {
    let colors: &[[f32; 4]] = &[
        [0.90, 0.25, 0.20, 0.90], // red
        [0.20, 0.65, 0.35, 0.90], // green
        [0.20, 0.45, 0.85, 0.90], // blue
        [0.95, 0.65, 0.10, 0.90], // amber
        [0.60, 0.30, 0.80, 0.90], // purple
        [0.10, 0.75, 0.75, 0.90], // teal
    ];

    let mut points = Vec::new();
    let cols = 3;
    let rows = 3;

    for row in 0..rows {
        for col in 0..cols {
            // Left half: x in [-0.85, -0.15]
            let x = -0.85 + (col as f32 / (cols - 1) as f32) * 0.7;
            let y = -0.55 + (row as f32 / (rows - 1) as f32) * 1.1;
            let color_idx = (row * cols + col) % colors.len();
            points.push(DemoPoint {
                x,
                y,
                radius: 0.08 + 0.015 * (col as f32),
                color: colors[color_idx],
                stroke_width: 0.0, // No stroke for shadow demo
                stroke_color: [0.0, 0.0, 0.0, 0.0],
            });
        }
    }
    points
}

/// Generate a grid of circles positioned for the fill+outline demo (right half).
fn generate_stroke_data() -> Vec<DemoPoint> {
    let colors: &[[f32; 4]] = &[
        [0.35, 0.65, 0.95, 0.90], // sky blue
        [0.95, 0.50, 0.20, 0.90], // orange
        [0.40, 0.80, 0.40, 0.90], // lime
        [0.85, 0.25, 0.55, 0.90], // magenta
        [0.55, 0.55, 0.95, 0.90], // periwinkle
        [0.95, 0.85, 0.20, 0.90], // gold
    ];

    let stroke_colors: &[[f32; 4]] = &[
        [0.10, 0.25, 0.50, 1.0],
        [0.50, 0.20, 0.05, 1.0],
        [0.15, 0.35, 0.15, 1.0],
        [0.40, 0.10, 0.25, 1.0],
        [0.20, 0.20, 0.45, 1.0],
        [0.45, 0.35, 0.05, 1.0],
    ];

    let mut points = Vec::new();
    let cols = 3;
    let rows = 3;

    for row in 0..rows {
        for col in 0..cols {
            // Right half: x in [0.15, 0.85]
            let x = 0.15 + (col as f32 / (cols - 1) as f32) * 0.7;
            let y = -0.55 + (row as f32 / (rows - 1) as f32) * 1.1;
            let idx = (row * cols + col) % colors.len();
            points.push(DemoPoint {
                x,
                y,
                radius: 0.08 + 0.015 * (col as f32),
                color: colors[idx],
                // stroke_width is in the same space as radius; keep it
                // at about 15–25 % of radius for a visible ring
                stroke_width: 0.015 + 0.005 * (row as f32),
                stroke_color: stroke_colors[idx],
            });
        }
    }
    points
}

fn to_instance(p: &DemoPoint) -> CircleInstance {
    CircleInstance {
        center: [p.x, p.y],
        radius: p.radius,
        _pad0: 0.0,
        fill_color: p.color,
        stroke_width: p.stroke_width,
        _pad1: [0.0; 3],
        stroke_color: p.stroke_color,
    }
}

// ── Shader sources ─────────────────────────────────────────────────────────

const MULTI_PASS_VERT: &str = include_str!("../src/mark/shaders/circle_multi_pass.vert.wgsl");
const MULTI_PASS_FRAG: &str = include_str!("../src/mark/shaders/circle_multi_pass.frag.wgsl");

// ── Pipeline helpers ───────────────────────────────────────────────────────

/// Create a render pipeline for one pass of the multi-pass configuration.
///
/// This mirrors what `MarkInfoImpl::create_render_pipeline_for_pass` does but
/// uses our custom multi-pass shaders instead of the mark's built-in ones.
fn create_pipeline_for_pass(
    device: &wgpu::Device,
    pass_config: &RenderPassConfig,
    bind_group_layout: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let vert_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(&format!("multi_pass_vert_{}", pass_config.label)),
        source: wgpu::ShaderSource::Wgsl(MULTI_PASS_VERT.into()),
    });
    let frag_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(&format!("multi_pass_frag_{}", pass_config.label)),
        source: wgpu::ShaderSource::Wgsl(MULTI_PASS_FRAG.into()),
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(&format!("multi_pass_layout_{}", pass_config.label)),
        bind_group_layouts: &[bind_group_layout],
        push_constant_ranges: &[],
    });

    let vs_entry = pass_config
        .vertex_entry_point
        .as_deref()
        .unwrap_or("vs_main");
    let fs_entry = pass_config
        .fragment_entry_point
        .as_deref()
        .unwrap_or("fs_main");

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(&format!("multi_pass_pipeline_{}", pass_config.label)),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &vert_module,
            entry_point: Some(vs_entry),
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
            entry_point: Some(fs_entry),
            targets: &[Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Bgra8UnormSrgb,
                blend: pass_config.blend_state,
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            polygon_mode: pass_config.polygon_mode,
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

// ── Renderer ───────────────────────────────────────────────────────────────

/// Helper: create an instance storage buffer and its bind group.
fn create_instance_resources(
    device: &wgpu::Device,
    label: &str,
    data: &[DemoPoint],
    layout: &wgpu::BindGroupLayout,
) -> (wgpu::Buffer, wgpu::BindGroup, u32) {
    use wgpu::util::DeviceExt;
    let instances: Vec<CircleInstance> = data.iter().map(to_instance).collect();
    let count = instances.len() as u32;
    let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(&format!("{label}_instance_buffer")),
        contents: bytemuck::cast_slice(&instances),
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

/// Holds GPU resources for the multi-pass rendering demo.
///
/// Two independent sets of circles are rendered:
/// - **Left half** — drop-shadow effect (shadow pass + main pass)
/// - **Right half** — fill + outline effect (fill pass + outline pass)
///
/// Each set has its own instance buffer and bind group so the data
/// doesn't overlap.
#[allow(dead_code)] // instance buffers must stay alive while bind groups reference them
struct MultiPassDemoRenderer {
    // Shared geometry
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,

    // Drop-shadow demo (left half)
    shadow_instance_buffer: wgpu::Buffer,
    shadow_bind_group: wgpu::BindGroup,
    shadow_instance_count: u32,
    shadow_config: MultiPassConfig,
    shadow_pipelines: Vec<wgpu::RenderPipeline>,

    // Fill + outline demo (right half)
    stroke_instance_buffer: wgpu::Buffer,
    stroke_bind_group: wgpu::BindGroup,
    stroke_instance_count: u32,
    stroke_config: MultiPassConfig,
    stroke_pipelines: Vec<wgpu::RenderPipeline>,

    // Renderer
    multi_pass_renderer: MultiPassRenderer,
}

impl MultiPassDemoRenderer {
    fn new(device: &wgpu::Device) -> Self {
        use wgpu::util::DeviceExt;

        // ── Shared geometry buffers ────────────────────────────────────
        let vertices = Circle::generate_vertices();
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("vertex_buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let indices = Circle::generate_indices().unwrap();
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("index_buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        // ── Shared bind group layout ───────────────────────────────────
        let bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("multi_pass_bind_group_layout"),
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

        // ── Shadow demo data (left half) ───────────────────────────────
        let shadow_data = generate_shadow_data();
        let (shadow_instance_buffer, shadow_bind_group, shadow_instance_count) =
            create_instance_resources(device, "shadow", &shadow_data, &bind_group_layout);

        // ── Stroke demo data (right half) ──────────────────────────────
        let stroke_data = generate_stroke_data();
        let (stroke_instance_buffer, stroke_bind_group, stroke_instance_count) =
            create_instance_resources(device, "stroke", &stroke_data, &bind_group_layout);

        // ── Drop-shadow multi-pass config ──────────────────────────────
        //
        // Pass 1: shadow – offset + blurred disc (vs_shadow / fs_shadow)
        // Pass 2: main   – crisp circle on top   (vs_main   / fs_main)
        let shadow_config = MultiPassConfig::new()
            .add_pass(RenderPassConfig {
                label: "shadow".into(),
                blend_state: Some(wgpu::BlendState::ALPHA_BLENDING),
                vertex_entry_point: Some("vs_shadow".into()),
                fragment_entry_point: Some("fs_shadow".into()),
                ..Default::default()
            })
            .add_pass(RenderPassConfig {
                label: "main".into(),
                blend_state: Some(wgpu::BlendState::ALPHA_BLENDING),
                ..Default::default()
            });

        let shadow_pipelines: Vec<_> = shadow_config
            .passes()
            .iter()
            .map(|p| create_pipeline_for_pass(device, p, &bind_group_layout))
            .collect();

        // ── Fill + outline multi-pass config ───────────────────────────
        //
        // Pass 1: fill    – solid interior              (vs_main / fs_fill)
        // Pass 2: outline – thin stroke ring on top     (vs_main / fs_outline)
        let stroke_config = MultiPassConfig::new()
            .add_pass(RenderPassConfig {
                label: "fill".into(),
                blend_state: Some(wgpu::BlendState::ALPHA_BLENDING),
                fragment_entry_point: Some("fs_fill".into()),
                ..Default::default()
            })
            .add_pass(RenderPassConfig {
                label: "outline".into(),
                blend_state: Some(wgpu::BlendState::ALPHA_BLENDING),
                fragment_entry_point: Some("fs_outline".into()),
                ..Default::default()
            });

        let stroke_pipelines: Vec<_> = stroke_config
            .passes()
            .iter()
            .map(|p| create_pipeline_for_pass(device, p, &bind_group_layout))
            .collect();

        Self {
            vertex_buffer,
            index_buffer,
            shadow_instance_buffer,
            shadow_bind_group,
            shadow_instance_count,
            shadow_config,
            shadow_pipelines,
            stroke_instance_buffer,
            stroke_bind_group,
            stroke_instance_count,
            stroke_config,
            stroke_pipelines,
            multi_pass_renderer: MultiPassRenderer::new(),
        }
    }

    /// Render both demo sections using multi-pass draw calls.
    fn render(&self, frame: &mut gup::RenderFrame) {
        let clear_color = Color {
            r: 0.96,
            g: 0.96,
            b: 0.98,
            a: 1.0,
        };

        let mut render_pass = frame.render_pass(Some(clear_color));

        // ── Section 1: Drop-shadow circles (left half) ─────────────────
        // The MultiPassRenderer issues two draw calls within the same
        // render pass – first the shadow, then the main circle.
        if let Err(e) = self.multi_pass_renderer.render_multi_pass(
            &mut render_pass,
            &self.shadow_config,
            &self.shadow_pipelines,
            &self.shadow_bind_group,
            &self.vertex_buffer,
            Some(&self.index_buffer),
            Circle::vertex_count() as u32,
            Circle::index_count().map(|c| c as u32),
            self.shadow_instance_count,
        ) {
            eprintln!("Shadow multi-pass render error: {e}");
        }

        // ── Section 2: Fill + outline circles (right half) ─────────────
        // Same concept – two draw calls, one for the fill, one for the
        // outline ring.  Each uses a different fragment shader entry point.
        if let Err(e) = self.multi_pass_renderer.render_multi_pass(
            &mut render_pass,
            &self.stroke_config,
            &self.stroke_pipelines,
            &self.stroke_bind_group,
            &self.vertex_buffer,
            Some(&self.index_buffer),
            Circle::vertex_count() as u32,
            Circle::index_count().map(|c| c as u32),
            self.stroke_instance_count,
        ) {
            eprintln!("Stroke multi-pass render error: {e}");
        }
    }
}

// ── Application ────────────────────────────────────────────────────────────

struct MultiPassApp {
    context: Option<Arc<GupContext>>,
    window: Option<Arc<Window>>,
    surface_id: Option<SurfaceId>,
    renderer: Option<MultiPassDemoRenderer>,
}

impl MultiPassApp {
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
            .with_title("Gup Multi-Pass Mark Rendering Demo")
            .with_inner_size(winit::dpi::LogicalSize::new(900, 500));

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

            // Lazily create renderer on first frame (needs device access)
            if self.renderer.is_none() {
                let renderer = MultiPassDemoRenderer::new(&ctx.device);
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

impl ApplicationHandler for MultiPassApp {
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

            println!("🎨 Multi-Pass Mark Rendering Demo");
            println!("==================================");
            println!();
            println!("This demo renders circles using two multi-pass techniques:");
            println!();
            println!("  • Drop shadow:   shadow pass (offset, blurred) + main pass");
            println!("  • Fill + outline: fill pass (solid) + outline pass (stroke ring)");
            println!();
            println!("Both techniques issue multiple draw calls within a single");
            println!("GPU render pass, following the project's single-pass convention.");
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
                        && let Ok(mut ctx) = Arc::try_unwrap(context) {
                            let _ = ctx.resize_surface(
                                surface_id,
                                PhysicalSize::new(size.width, size.height),
                            );
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

    let mut app = MultiPassApp::new();
    event_loop.run_app(&mut app)?;

    Ok(())
}

// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! 3D Scatter Plot Example
//!
//! Demonstrates 3D rendering with `Sphere3D` marks, perspective camera,
//! Phong lighting, and an orbiting camera animation.

use gup::GupContext;
use gup::camera::Camera;
use gup::depth::{DEPTH_FORMAT, DepthBuffer};
use gup::lighting::{LightUniform, Material};
use gup::mark::Mark;
use gup::mark::sphere3d::{Sphere3D, Sphere3DInstance};
use std::sync::Arc;
use wgpu::util::DeviceExt;
use winit::{
    application::ApplicationHandler,
    event::{ElementState, KeyEvent, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowAttributes},
};

// ---------------------------------------------------------------------------
// Data generation
// ---------------------------------------------------------------------------

/// Generate `count` data points spread through 3D space.
fn generate_data(count: usize) -> Vec<Sphere3DInstance> {
    let material = Material::default();
    let mut instances = Vec::with_capacity(count);
    let golden_ratio = (1.0 + 5.0_f32.sqrt()) / 2.0;

    for i in 0..count {
        let t = i as f32 / count as f32;

        // Fibonacci sphere-ish distribution for nice spread.
        let theta = 2.0 * std::f32::consts::PI * i as f32 / golden_ratio;
        let phi = (1.0 - 2.0 * (i as f32 + 0.5) / count as f32).acos();

        let r = 1.5 * (0.3 + 0.7 * t);
        let x = r * phi.sin() * theta.cos();
        let y = r * phi.sin() * theta.sin();
        let z = r * phi.cos();

        // Colour gradient based on position.
        let cr = 0.3 + 0.7 * ((x + 1.5) / 3.0);
        let cg = 0.3 + 0.7 * ((y + 1.5) / 3.0);
        let cb = 0.3 + 0.7 * ((z + 1.5) / 3.0);

        instances.push(Sphere3DInstance {
            position: [x, y, z],
            radius: 0.02 + 0.03 * t,
            color: [cr, cg, cb, 1.0],
            material_albedo_ambient: [
                material.albedo[0],
                material.albedo[1],
                material.albedo[2],
                material.ambient,
            ],
            material_dss: [material.diffuse, material.specular, material.shininess, 0.0],
        });
    }
    instances
}

// ---------------------------------------------------------------------------
// App state
// ---------------------------------------------------------------------------

struct Scatter3DApp {
    window: Option<Arc<Window>>,
    context: Option<Arc<GupContext>>,
    // GPU resources (lazily initialised on first render).
    gpu: Option<GpuResources>,
    frame_count: u64,
    start_time: std::time::Instant,
}

struct GpuResources {
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    #[allow(dead_code)]
    instance_buffer: wgpu::Buffer,
    camera_buffer: wgpu::Buffer,
    #[allow(dead_code)]
    light_buffer: wgpu::Buffer,
    instance_bind_group: wgpu::BindGroup,
    uniform_bind_group: wgpu::BindGroup,
    depth_buffer: DepthBuffer,
    num_instances: u32,
    num_indices: u32,
}

impl Scatter3DApp {
    fn new() -> Self {
        Self {
            window: None,
            context: None,
            gpu: None,
            frame_count: 0,
            start_time: std::time::Instant::now(),
        }
    }

    // -----------------------------------------------------------------------
    // GPU resource creation
    // -----------------------------------------------------------------------

    fn create_gpu_resources(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
    ) -> GpuResources {
        let num_points: usize = 1_000;
        let instances = generate_data(num_points);
        let num_instances = instances.len() as u32;

        // ---- Vertex / index buffers ----
        let vertices = Sphere3D::generate_vertices();
        let indices = Sphere3D::generate_indices().unwrap();
        let num_indices = indices.len() as u32;

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("sphere3d_vertex"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("sphere3d_index"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        // ---- Instance storage buffer ----
        let instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("sphere3d_instances"),
            contents: bytemuck::cast_slice(&instances),
            usage: wgpu::BufferUsages::STORAGE,
        });

        // ---- Camera uniform ----
        let aspect = width as f32 / height.max(1) as f32;
        let mut camera = Camera::perspective(std::f32::consts::FRAC_PI_4, aspect, 0.1, 100.0);
        camera.look_at(
            gup::shader_function::Vec3::new(0.0, 0.0, 5.0),
            gup::shader_function::Vec3::new(0.0, 0.0, 0.0),
            gup::shader_function::Vec3::new(0.0, 1.0, 0.0),
        );
        let camera_uniform = camera.to_uniform();

        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("camera_uniform"),
            contents: bytemuck::bytes_of(&camera_uniform),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // ---- Light uniform ----
        let light = LightUniform::default();
        let light_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("light_uniform"),
            contents: bytemuck::bytes_of(&light),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // ---- Bind group layouts ----
        // Group 0: instance storage buffer
        let instance_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("sphere3d_instance_bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        // Group 1: camera uniform + light uniform
        let uniform_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("sphere3d_uniform_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        // ---- Bind groups ----
        let instance_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("sphere3d_instance_bg"),
            layout: &instance_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: instance_buffer.as_entire_binding(),
            }],
        });

        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("sphere3d_uniform_bg"),
            layout: &uniform_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: light_buffer.as_entire_binding(),
                },
            ],
        });

        // ---- Shader ----
        let vert_src = Sphere3D::VERTEX_SHADER.unwrap();
        let frag_src = Sphere3D::FRAGMENT_SHADER.unwrap();

        let vert_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("sphere3d_vert"),
            source: wgpu::ShaderSource::Wgsl(vert_src.into()),
        });
        let frag_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("sphere3d_frag"),
            source: wgpu::ShaderSource::Wgsl(frag_src.into()),
        });

        // ---- Pipeline ----
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("sphere3d_pipeline_layout"),
            bind_group_layouts: &[&instance_bgl, &uniform_bgl],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sphere3d_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &vert_module,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<gup::mark::sphere3d::Sphere3DVertex>()
                        as wgpu::BufferAddress,
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
                    format: wgpu::TextureFormat::Bgra8UnormSrgb,
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
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // ---- Depth buffer ----
        let depth_buffer = DepthBuffer::new(device, width, height);

        // ---- Upload initial light data ----
        queue.write_buffer(&light_buffer, 0, bytemuck::bytes_of(&light));

        GpuResources {
            pipeline,
            vertex_buffer,
            index_buffer,
            instance_buffer,
            camera_buffer,
            light_buffer,
            instance_bind_group,
            uniform_bind_group,
            depth_buffer,
            num_instances,
            num_indices,
        }
    }

    // -----------------------------------------------------------------------
    // Render
    // -----------------------------------------------------------------------

    fn render_frame(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let Some(context) = self.context.take() else {
            return Ok(());
        };

        let mut ctx = Arc::try_unwrap(context).map_err(|_| "cannot unwrap Arc")?;

        // Initialise GPU resources on first frame.
        if self.gpu.is_none() {
            let size = self.window.as_ref().unwrap().inner_size();
            self.gpu = Some(Self::create_gpu_resources(
                &ctx.device,
                &ctx.queue,
                size.width,
                size.height,
            ));
            println!(
                "✓ GPU resources created ({} spheres)",
                self.gpu.as_ref().unwrap().num_instances
            );
        }

        // Update camera (orbit animation).
        let elapsed = self.start_time.elapsed().as_secs_f32();
        let angle = elapsed * 0.5; // radians/sec
        let eye_dist = 4.0;
        let eye =
            gup::shader_function::Vec3::new(eye_dist * angle.cos(), 1.5, eye_dist * angle.sin());

        let size = self.window.as_ref().unwrap().inner_size();
        let aspect = size.width as f32 / size.height.max(1) as f32;
        let mut camera = Camera::perspective(std::f32::consts::FRAC_PI_4, aspect, 0.1, 100.0);
        camera.look_at(
            eye,
            gup::shader_function::Vec3::new(0.0, 0.0, 0.0),
            gup::shader_function::Vec3::new(0.0, 1.0, 0.0),
        );
        let camera_uniform = camera.to_uniform();

        let gpu = self.gpu.as_ref().unwrap();
        ctx.queue
            .write_buffer(&gpu.camera_buffer, 0, bytemuck::bytes_of(&camera_uniform));

        match ctx.begin_frame() {
            Ok(mut frame) => {
                {
                    let mut pass = frame.render_pass_with_depth(
                        Some(wgpu::Color {
                            r: 0.05,
                            g: 0.05,
                            b: 0.1,
                            a: 1.0,
                        }),
                        gpu.depth_buffer.view(),
                    );

                    pass.set_pipeline(&gpu.pipeline);
                    pass.set_bind_group(0, &gpu.instance_bind_group, &[]);
                    pass.set_bind_group(1, &gpu.uniform_bind_group, &[]);
                    pass.set_vertex_buffer(0, gpu.vertex_buffer.slice(..));
                    pass.set_index_buffer(gpu.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..gpu.num_indices, 0, 0..gpu.num_instances);
                }

                frame.finish()?;
                self.frame_count += 1;

                if self.frame_count % 120 == 0 {
                    let stats = ctx.frame_stats();
                    println!(
                        "Frame {}: {:.1} FPS, {:.2}ms avg",
                        self.frame_count,
                        stats.fps(),
                        stats.avg_frame_time,
                    );
                }
            }
            Err(e) => eprintln!("begin_frame error: {e}"),
        }

        self.context = Some(Arc::new(ctx));
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// winit ApplicationHandler
// ---------------------------------------------------------------------------

impl ApplicationHandler for Scatter3DApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        println!("=== Gup 3D Scatter Plot Demo ===");

        pollster::block_on(async {
            let attrs = WindowAttributes::default()
                .with_title("Gup 3D Scatter Plot — Q to quit")
                .with_inner_size(winit::dpi::LogicalSize::new(800, 600));

            let window = Arc::new(event_loop.create_window(attrs).expect("create window"));
            let context = GupContext::with_surface(Arc::clone(&window))
                .await
                .expect("create GPU context");

            self.window = Some(window);
            self.context = Some(context);
            println!("✓ Window + GPU context ready");
        });
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(KeyCode::KeyQ),
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(context) = self.context.take() {
                    if let Ok(mut ctx) = Arc::try_unwrap(context) {
                        let _ = ctx.resize_surface(
                            ctx.primary_surface_id().unwrap(),
                            gup::PhysicalSize::new(size.width, size.height),
                        );
                        if let Some(gpu) = self.gpu.as_mut() {
                            gpu.depth_buffer
                                .resize(&ctx.device, size.width, size.height);
                        }
                        self.context = Some(Arc::new(ctx));
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                if let Err(e) = self.render_frame() {
                    eprintln!("render error: {e}");
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

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = Scatter3DApp::new();
    event_loop.run_app(&mut app)?;
    Ok(())
}

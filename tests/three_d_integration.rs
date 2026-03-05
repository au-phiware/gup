// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GPU integration tests for 3D marks (Sphere3D, Box3D, Line3D).
//!
//! These tests exercise the full wgpu pipeline for each 3D mark type:
//! shader compilation, bind-group creation, render-pass execution with a
//! depth buffer, and pixel readback verification.  They are designed to
//! catch shader regressions and pipeline mismatches in CI.

use gup::camera::Camera;
use gup::depth::{DEPTH_FORMAT, DepthBuffer};
use gup::export::png::OffscreenTarget;
use gup::lighting::{LightUniform, Material};
use gup::mark::Mark;
use gup::mark::box3d::{Box3D, Box3DInstance, Box3DVertex};
use gup::mark::line3d::{Line3D, Line3DInstance, Line3DVertex};
use gup::mark::sphere3d::{Sphere3D, Sphere3DInstance, Sphere3DVertex};
use gup::shader_function::Vec3;
use std::time::Instant;
use wgpu::util::DeviceExt;

const WIDTH: u32 = 256;
const HEIGHT: u32 = 256;
const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8UnormSrgb;

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Create a headless render context (device + queue).
async fn headless_context() -> (wgpu::Device, wgpu::Queue) {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::PRIMARY,
        ..Default::default()
    });

    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            ..Default::default()
        })
        .await
        .expect("No suitable GPU adapter found");

    adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("3d_integration_test"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            ..Default::default()
        })
        .await
        .expect("Failed to create GPU device")
}

/// Set up a perspective camera looking at the origin from a fixed position.
fn test_camera() -> Camera {
    let mut cam = Camera::perspective(std::f32::consts::FRAC_PI_4, 1.0, 0.1, 100.0);
    cam.look_at(
        Vec3::new(0.0, 0.0, 5.0),
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(0.0, 1.0, 0.0),
    );
    cam
}

/// Create the bind-group layout for the per-instance storage buffer (group 0).
fn instance_bgl(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("3d_test_instance_bgl"),
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
    })
}

/// Create the bind-group layout for camera + light uniforms (group 1).
/// For `lit` marks both bindings are needed; for unlit marks only binding 0
/// (camera) is required, but we always create both so the layout is reusable.
fn uniform_bgl(device: &wgpu::Device, lit: bool) -> wgpu::BindGroupLayout {
    let mut entries = vec![wgpu::BindGroupLayoutEntry {
        binding: 0,
        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }];

    if lit {
        entries.push(wgpu::BindGroupLayoutEntry {
            binding: 1,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        });
    }

    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("3d_test_uniform_bgl"),
        entries: &entries,
    })
}

/// Upload camera and (optionally) light uniforms and return the bind group.
fn create_uniform_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    camera: &Camera,
    lit: bool,
) -> (wgpu::BindGroup, wgpu::Buffer) {
    let camera_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("camera_uniform"),
        contents: bytemuck::bytes_of(&camera.to_uniform()),
        usage: wgpu::BufferUsages::UNIFORM,
    });

    let mut entries = vec![wgpu::BindGroupEntry {
        binding: 0,
        resource: camera_buf.as_entire_binding(),
    }];

    let light_buf;
    if lit {
        let light = LightUniform::default();
        light_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("light_uniform"),
            contents: bytemuck::bytes_of(&light),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        entries.push(wgpu::BindGroupEntry {
            binding: 1,
            resource: light_buf.as_entire_binding(),
        });
    }

    let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("3d_test_uniform_bg"),
        layout,
        entries: &entries,
    });

    (bg, camera_buf)
}

/// Assert that the RGBA pixel buffer is not all-zero (i.e. something was
/// actually drawn). The background clear colour is typically near-black,
/// so we look for any channel that exceeds a small threshold.
fn assert_non_zero_pixels(pixels: &[u8], label: &str) {
    let total_pixels = pixels.len() / 4;
    let non_zero = pixels
        .chunks_exact(4)
        .filter(|px| px[0] > 10 || px[1] > 10 || px[2] > 10)
        .count();
    assert!(
        non_zero > 0,
        "{label}: all {total_pixels} pixels are black — nothing was drawn"
    );
}

// ---------------------------------------------------------------------------
// Sphere3D tests
// ---------------------------------------------------------------------------

/// Generate `count` sphere instances around the origin.
fn sphere_instances(count: usize) -> Vec<Sphere3DInstance> {
    let material = Material::default();
    let golden = (1.0 + 5.0_f32.sqrt()) / 2.0;
    (0..count)
        .map(|i| {
            let t = i as f32 / count as f32;
            let theta = 2.0 * std::f32::consts::PI * i as f32 / golden;
            let phi = (1.0 - 2.0 * (i as f32 + 0.5) / count as f32).acos();
            let r = 1.5 * (0.3 + 0.7 * t);
            Sphere3DInstance {
                position: [
                    r * phi.sin() * theta.cos(),
                    r * phi.sin() * theta.sin(),
                    r * phi.cos(),
                ],
                radius: 0.03 + 0.02 * t,
                color: [0.4 + 0.6 * t, 0.6, 1.0 - 0.5 * t, 1.0],
                material_albedo_ambient: [
                    material.albedo[0],
                    material.albedo[1],
                    material.albedo[2],
                    material.ambient,
                ],
                material_dss: [material.diffuse, material.specular, material.shininess, 0.0],
            }
        })
        .collect()
}

/// Build the full Sphere3D render pipeline.
fn sphere_pipeline(
    device: &wgpu::Device,
    inst_bgl: &wgpu::BindGroupLayout,
    uni_bgl: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let vert = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("sphere3d_vert"),
        source: wgpu::ShaderSource::Wgsl(Sphere3D::VERTEX_SHADER.unwrap().into()),
    });
    let frag = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("sphere3d_frag"),
        source: wgpu::ShaderSource::Wgsl(Sphere3D::FRAGMENT_SHADER.unwrap().into()),
    });

    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("sphere3d_layout"),
        bind_group_layouts: &[inst_bgl, uni_bgl],
        push_constant_ranges: &[],
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("sphere3d_pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &vert,
            entry_point: Some("vs_main"),
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<Sphere3DVertex>() as wgpu::BufferAddress,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: Sphere3D::vertex_attributes(),
            }],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &frag,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: COLOR_FORMAT,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            cull_mode: None,
            ..Default::default()
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
    })
}

/// Headless integration test: render 1000+ `Sphere3D` instances and verify
/// non-zero pixel output with no wgpu validation errors.
#[tokio::test]
async fn sphere3d_headless_1000_instances() {
    let (device, queue) = headless_context().await;

    let instances = sphere_instances(1_000);
    let num_instances = instances.len() as u32;

    // Vertex + index buffers
    let vertices = Sphere3D::generate_vertices();
    let indices = Sphere3D::generate_indices().unwrap();

    let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("sphere_vb"),
        contents: bytemuck::cast_slice(&vertices),
        usage: wgpu::BufferUsages::VERTEX,
    });
    let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("sphere_ib"),
        contents: bytemuck::cast_slice(&indices),
        usage: wgpu::BufferUsages::INDEX,
    });

    // Instance storage buffer
    let inst_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("sphere_instances"),
        contents: bytemuck::cast_slice(&instances),
        usage: wgpu::BufferUsages::STORAGE,
    });

    // Bind groups
    let inst_bgl = instance_bgl(&device);
    let uni_bgl = uniform_bgl(&device, true);

    let inst_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("sphere_inst_bg"),
        layout: &inst_bgl,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: inst_buf.as_entire_binding(),
        }],
    });

    let camera = test_camera();
    let (uni_bg, _cam_buf) = create_uniform_bind_group(&device, &uni_bgl, &camera, true);

    // Pipeline
    let pipeline = sphere_pipeline(&device, &inst_bgl, &uni_bgl);

    // Off-screen render target + depth buffer
    let target = OffscreenTarget::new(&device, WIDTH, HEIGHT);
    let depth = DepthBuffer::new(&device, WIDTH, HEIGHT);

    // Render
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("sphere_encoder"),
    });
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("sphere_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target.view(),
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth.view(),
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &inst_bg, &[]);
        pass.set_bind_group(1, &uni_bg, &[]);
        pass.set_vertex_buffer(0, vb.slice(..));
        pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..indices.len() as u32, 0, 0..num_instances);
    }
    queue.submit(std::iter::once(encoder.finish()));

    // Readback and verify
    let pixels = target
        .readback_pixels(&device, &queue)
        .expect("pixel readback failed");
    assert_eq!(pixels.len(), (WIDTH * HEIGHT * 4) as usize);
    assert_non_zero_pixels(&pixels, "Sphere3D");
}

// ---------------------------------------------------------------------------
// Box3D tests
// ---------------------------------------------------------------------------

/// Generate `count` box instances in a grid.
fn box_instances(count: usize) -> Vec<Box3DInstance> {
    let material = Material::default();
    let side = (count as f32).sqrt().ceil() as usize;
    let step = 3.0 / side.max(1) as f32;
    (0..count)
        .map(|i| {
            let x = (i % side) as f32 * step - 1.5;
            let y = (i / side) as f32 * step - 1.5;
            let t = i as f32 / count as f32;
            Box3DInstance {
                center: [x, y, 0.0],
                _pad0: 0.0,
                half_extents: [0.04, 0.04, 0.04],
                _pad1: 0.0,
                color: [0.8, 0.3 + 0.5 * t, 0.2, 1.0],
                material_albedo_ambient: [
                    material.albedo[0],
                    material.albedo[1],
                    material.albedo[2],
                    material.ambient,
                ],
                material_dss: [material.diffuse, material.specular, material.shininess, 0.0],
            }
        })
        .collect()
}

/// Build the full Box3D render pipeline.
fn box_pipeline(
    device: &wgpu::Device,
    inst_bgl: &wgpu::BindGroupLayout,
    uni_bgl: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let vert = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("box3d_vert"),
        source: wgpu::ShaderSource::Wgsl(Box3D::VERTEX_SHADER.unwrap().into()),
    });
    let frag = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("box3d_frag"),
        source: wgpu::ShaderSource::Wgsl(Box3D::FRAGMENT_SHADER.unwrap().into()),
    });

    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("box3d_layout"),
        bind_group_layouts: &[inst_bgl, uni_bgl],
        push_constant_ranges: &[],
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("box3d_pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &vert,
            entry_point: Some("vs_main"),
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<Box3DVertex>() as wgpu::BufferAddress,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: Box3D::vertex_attributes(),
            }],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &frag,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: COLOR_FORMAT,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            cull_mode: Some(wgpu::Face::Back),
            ..Default::default()
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
    })
}

/// Headless integration test: render Box3D instances and verify output.
#[tokio::test]
async fn box3d_headless_render() {
    let (device, queue) = headless_context().await;

    let instances = box_instances(100);
    let num_instances = instances.len() as u32;

    let vertices = Box3D::generate_vertices();
    let indices = Box3D::generate_indices().unwrap();

    let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("box_vb"),
        contents: bytemuck::cast_slice(&vertices),
        usage: wgpu::BufferUsages::VERTEX,
    });
    let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("box_ib"),
        contents: bytemuck::cast_slice(&indices),
        usage: wgpu::BufferUsages::INDEX,
    });
    let inst_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("box_instances"),
        contents: bytemuck::cast_slice(&instances),
        usage: wgpu::BufferUsages::STORAGE,
    });

    let ibgl = instance_bgl(&device);
    let ubgl = uniform_bgl(&device, true);

    let inst_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("box_inst_bg"),
        layout: &ibgl,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: inst_buf.as_entire_binding(),
        }],
    });

    let camera = test_camera();
    let (uni_bg, _) = create_uniform_bind_group(&device, &ubgl, &camera, true);

    let pipeline = box_pipeline(&device, &ibgl, &ubgl);
    let target = OffscreenTarget::new(&device, WIDTH, HEIGHT);
    let depth = DepthBuffer::new(&device, WIDTH, HEIGHT);

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("box_encoder"),
    });
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("box_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target.view(),
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth.view(),
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &inst_bg, &[]);
        pass.set_bind_group(1, &uni_bg, &[]);
        pass.set_vertex_buffer(0, vb.slice(..));
        pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..indices.len() as u32, 0, 0..num_instances);
    }
    queue.submit(std::iter::once(encoder.finish()));

    let pixels = target
        .readback_pixels(&device, &queue)
        .expect("pixel readback failed");
    assert_eq!(pixels.len(), (WIDTH * HEIGHT * 4) as usize);
    assert_non_zero_pixels(&pixels, "Box3D");
}

// ---------------------------------------------------------------------------
// Line3D tests
// ---------------------------------------------------------------------------

/// Generate `count` line instances radiating from the origin.
fn line_instances(count: usize) -> Vec<Line3DInstance> {
    (0..count)
        .map(|i| {
            let t = i as f32 / count as f32;
            let angle = 2.0 * std::f32::consts::PI * t;
            Line3DInstance {
                start: [0.0, 0.0, 0.0],
                width: 0.01,
                end: [angle.cos(), angle.sin(), 0.0],
                _pad: 0.0,
                color: [1.0, 1.0 - t, t, 1.0],
            }
        })
        .collect()
}

/// Build the Line3D render pipeline (unlit — no light binding).
fn line_pipeline(
    device: &wgpu::Device,
    inst_bgl: &wgpu::BindGroupLayout,
    uni_bgl: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let vert = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("line3d_vert"),
        source: wgpu::ShaderSource::Wgsl(Line3D::VERTEX_SHADER.unwrap().into()),
    });
    let frag = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("line3d_frag"),
        source: wgpu::ShaderSource::Wgsl(Line3D::FRAGMENT_SHADER.unwrap().into()),
    });

    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("line3d_layout"),
        bind_group_layouts: &[inst_bgl, uni_bgl],
        push_constant_ranges: &[],
    });

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("line3d_pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &vert,
            entry_point: Some("vs_main"),
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<Line3DVertex>() as wgpu::BufferAddress,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: Line3D::vertex_attributes(),
            }],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &frag,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: COLOR_FORMAT,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            cull_mode: None,
            ..Default::default()
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
    })
}

/// Headless integration test: render Line3D instances and verify output.
#[tokio::test]
async fn line3d_headless_render() {
    let (device, queue) = headless_context().await;

    let instances = line_instances(50);
    let num_instances = instances.len() as u32;

    let vertices = Line3D::generate_vertices();
    let indices = Line3D::generate_indices().unwrap();

    let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("line_vb"),
        contents: bytemuck::cast_slice(&vertices),
        usage: wgpu::BufferUsages::VERTEX,
    });
    let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("line_ib"),
        contents: bytemuck::cast_slice(&indices),
        usage: wgpu::BufferUsages::INDEX,
    });
    let inst_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("line_instances"),
        contents: bytemuck::cast_slice(&instances),
        usage: wgpu::BufferUsages::STORAGE,
    });

    let ibgl = instance_bgl(&device);
    // Line3D is unlit — only camera uniform in group 1.
    let ubgl = uniform_bgl(&device, false);

    let inst_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("line_inst_bg"),
        layout: &ibgl,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: inst_buf.as_entire_binding(),
        }],
    });

    let camera = test_camera();
    let (uni_bg, _) = create_uniform_bind_group(&device, &ubgl, &camera, false);

    let pipeline = line_pipeline(&device, &ibgl, &ubgl);
    let target = OffscreenTarget::new(&device, WIDTH, HEIGHT);
    let depth = DepthBuffer::new(&device, WIDTH, HEIGHT);

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("line_encoder"),
    });
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("line_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target.view(),
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth.view(),
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &inst_bg, &[]);
        pass.set_bind_group(1, &uni_bg, &[]);
        pass.set_vertex_buffer(0, vb.slice(..));
        pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..indices.len() as u32, 0, 0..num_instances);
    }
    queue.submit(std::iter::once(encoder.finish()));

    let pixels = target
        .readback_pixels(&device, &queue)
        .expect("pixel readback failed");
    assert_eq!(pixels.len(), (WIDTH * HEIGHT * 4) as usize);
    assert_non_zero_pixels(&pixels, "Line3D");
}

// ---------------------------------------------------------------------------
// Performance test
// ---------------------------------------------------------------------------

/// Assert that rendering 100K sphere instances completes in < 16ms per frame.
#[tokio::test]
async fn sphere3d_100k_performance() {
    let (device, queue) = headless_context().await;

    let instances = sphere_instances(100_000);
    let num_instances = instances.len() as u32;

    let vertices = Sphere3D::generate_vertices();
    let indices = Sphere3D::generate_indices().unwrap();

    let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("perf_sphere_vb"),
        contents: bytemuck::cast_slice(&vertices),
        usage: wgpu::BufferUsages::VERTEX,
    });
    let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("perf_sphere_ib"),
        contents: bytemuck::cast_slice(&indices),
        usage: wgpu::BufferUsages::INDEX,
    });
    let inst_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("perf_sphere_instances"),
        contents: bytemuck::cast_slice(&instances),
        usage: wgpu::BufferUsages::STORAGE,
    });

    let ibgl = instance_bgl(&device);
    let ubgl = uniform_bgl(&device, true);

    let inst_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("perf_inst_bg"),
        layout: &ibgl,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: inst_buf.as_entire_binding(),
        }],
    });

    let camera = test_camera();
    let (uni_bg, _) = create_uniform_bind_group(&device, &ubgl, &camera, true);

    let pipeline = sphere_pipeline(&device, &ibgl, &ubgl);
    let target = OffscreenTarget::new(&device, WIDTH, HEIGHT);
    let depth = DepthBuffer::new(&device, WIDTH, HEIGHT);

    // Warm up (first frame includes shader compilation + pipeline creation).
    for _ in 0..3 {
        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("warmup"),
        });
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("warmup_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target.view(),
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: depth.view(),
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &inst_bg, &[]);
            pass.set_bind_group(1, &uni_bg, &[]);
            pass.set_vertex_buffer(0, vb.slice(..));
            pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..indices.len() as u32, 0, 0..num_instances);
        }
        queue.submit(std::iter::once(enc.finish()));
        device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: None,
            })
            .unwrap();
    }

    // Timed run
    let num_frames = 10;
    let start = Instant::now();
    for _ in 0..num_frames {
        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("timed"),
        });
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("timed_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target.view(),
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: depth.view(),
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &inst_bg, &[]);
            pass.set_bind_group(1, &uni_bg, &[]);
            pass.set_vertex_buffer(0, vb.slice(..));
            pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..indices.len() as u32, 0, 0..num_instances);
        }
        queue.submit(std::iter::once(enc.finish()));
        device
            .poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: None,
            })
            .unwrap();
    }
    let elapsed = start.elapsed();
    let avg_ms = elapsed.as_secs_f64() * 1000.0 / num_frames as f64;

    println!("100K Sphere3D: {avg_ms:.2}ms average per frame ({num_frames} frames)");
    assert!(
        avg_ms < 16.0,
        "100K instance frame time {avg_ms:.2}ms exceeds 16ms budget"
    );
}

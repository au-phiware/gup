// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! WebAssembly integration tests (GUP-237).
//!
//! These tests verify that the Gup library works correctly at runtime in a
//! browser environment.  They cover three tiers:
//!
//! 1. **CPU-only** – mark vertex generation, registry operations
//! 2. **GPU initialisation** – adapter and device creation via WebGPU
//! 3. **Rendering smoke** – a minimal circle mark render pass to a texture
//!
//! # Running locally
//!
//! ```bash
//! wasm-pack test --headless --chrome -- --test wasm_integration
//! ```
//!
//! For interactive debugging in a browser window:
//!
//! ```bash
//! wasm-pack test --chrome -- --test wasm_integration
//! ```
//!
//! **Note**: GPU tests require a Chromium-based browser launched with WebGPU
//! flags.  If no GPU adapter is available the GPU tests pass with a warning.
#![cfg(target_arch = "wasm32")]

use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

// ---------------------------------------------------------------------------
// Tier 1 – CPU-only tests (no GPU required)
// ---------------------------------------------------------------------------

/// Verify that the WASM module loads and basic library types are accessible.
#[wasm_bindgen_test]
fn test_wasm_module_loads() {
    // If we get here the wasm-pack output was loaded successfully.
    // Instantiate a simple type to prove the module is alive.
    let _circle = gup::mark::circle::Circle;
    let _rect = gup::mark::rectangle::Rectangle;
}

/// Verify Circle vertex generation produces the expected unit-quad geometry.
#[wasm_bindgen_test]
fn test_circle_vertex_generation() {
    use gup::mark::Mark;
    use gup::mark::circle::Circle;

    let verts = Circle::generate_vertices();
    assert_eq!(verts.len(), 4, "Circle quad should have 4 vertices");

    // Verify unit-quad corners (order: BL, BR, TR, TL)
    let expected: &[[f32; 2]] = &[[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]];
    for (v, e) in verts.iter().zip(expected.iter()) {
        assert_eq!(v.position, *e);
    }

    let indices = Circle::generate_indices().expect("Circle should have indices");
    assert_eq!(
        indices.len(),
        6,
        "Circle quad should have 6 indices (2 tris)"
    );
    assert_eq!(&indices, &[0, 1, 2, 0, 2, 3]);
}

/// Verify Rectangle vertex generation produces the expected half-unit quad.
#[wasm_bindgen_test]
fn test_rectangle_vertex_generation() {
    use gup::mark::Mark;
    use gup::mark::rectangle::Rectangle;

    let verts = Rectangle::generate_vertices();
    assert_eq!(verts.len(), 4, "Rectangle quad should have 4 vertices");

    let indices = Rectangle::generate_indices().expect("Rectangle should have indices");
    assert_eq!(indices.len(), 6, "Rectangle quad should have 6 indices");
}

/// Verify CircleInstance can be constructed and has correct byte layout.
#[wasm_bindgen_test]
fn test_circle_instance_layout() {
    use gup::mark::circle::CircleInstance;

    let instance = CircleInstance {
        center: [0.5, 0.5],
        radius: 0.1,
        _pad0: 0.0,
        fill_color: [1.0, 0.0, 0.0, 1.0],
        stroke_width: 0.01,
        _pad1: [0.0; 3],
        stroke_color: [0.0, 0.0, 0.0, 1.0],
    };

    // Verify bytemuck round-trip
    let bytes = bytemuck::bytes_of(&instance);
    let restored: &CircleInstance = bytemuck::from_bytes(bytes);
    assert_eq!(restored.center, [0.5, 0.5]);
    assert_eq!(restored.radius, 0.1);
    assert_eq!(restored.fill_color, [1.0, 0.0, 0.0, 1.0]);
}

/// Verify MarkRegistry can register and query mark types.
#[wasm_bindgen_test]
fn test_mark_registry_operations() {
    use gup::mark::MarkRegistry;
    use gup::mark::circle::Circle;
    use gup::mark::rectangle::Rectangle;

    let mut registry = MarkRegistry::new();
    registry.register::<Circle>();
    registry.register::<Rectangle>();

    assert!(registry.is_registered::<Circle>());
    assert!(registry.is_registered::<Rectangle>());

    let types = registry.registered_types();
    assert!(types.len() >= 2);
}

/// Verify Circle shader sources are available.
#[wasm_bindgen_test]
fn test_circle_shader_sources() {
    use gup::mark::Mark;
    use gup::mark::circle::Circle;

    let vert = Circle::VERTEX_SHADER.expect("Circle should have a vertex shader");
    let frag = Circle::FRAGMENT_SHADER.expect("Circle should have a fragment shader");

    assert!(
        vert.contains("vs_main"),
        "Vertex shader should contain vs_main"
    );
    assert!(
        frag.contains("fs_main"),
        "Fragment shader should contain fs_main"
    );
    assert!(
        vert.contains("CircleInstance"),
        "Vertex shader should reference CircleInstance"
    );
}

// ---------------------------------------------------------------------------
// Tier 2 – GPU initialisation tests
// ---------------------------------------------------------------------------

/// Helper: request a WebGPU adapter, returning `None` when unavailable.
async fn try_get_adapter() -> Option<wgpu::Adapter> {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::BROWSER_WEBGPU | wgpu::Backends::GL,
        ..Default::default()
    });

    instance
        .request_adapter(&wgpu::RequestAdapterOptions::default())
        .await
        .ok()
}

/// Helper: request adapter + device, returning `None` when unavailable.
async fn try_get_device() -> Option<(wgpu::Adapter, wgpu::Device, wgpu::Queue)> {
    let adapter = try_get_adapter().await?;

    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("gup_wasm_test"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::downlevel_webgl2_defaults(),
            memory_hints: wgpu::MemoryHints::default(),
            trace: Default::default(),
        })
        .await
        .ok()?;

    Some((adapter, device, queue))
}

/// Request a WebGPU adapter in the browser.
///
/// Passes gracefully when no GPU is available (e.g. CI without hardware
/// acceleration).
#[wasm_bindgen_test]
async fn test_gpu_adapter_request() {
    match try_get_adapter().await {
        Some(adapter) => {
            let info = adapter.get_info();
            wasm_bindgen_test::console_log!(
                "✅ GPU adapter: {} (backend: {:?})",
                info.name,
                info.backend,
            );
        }
        None => {
            wasm_bindgen_test::console_log!("⚠️  No GPU adapter available – skipping GPU tests");
        }
    }
}

/// Request a WebGPU device from the adapter.
#[wasm_bindgen_test]
async fn test_gpu_device_creation() {
    let Some((_adapter, device, _queue)) = try_get_device().await else {
        wasm_bindgen_test::console_log!("⚠️  No GPU – skipping device creation test");
        return;
    };

    // Verify we can create a basic buffer (proves device is functional)
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("test_buffer"),
        size: 64,
        usage: wgpu::BufferUsages::VERTEX,
        mapped_at_creation: false,
    });
    assert!(buffer.size() >= 64);

    wasm_bindgen_test::console_log!("✅ GPU device and buffer creation succeeded");
}

// ---------------------------------------------------------------------------
// Tier 3 – Rendering smoke tests
// ---------------------------------------------------------------------------

/// Render a single circle to an off-screen texture and verify no errors.
///
/// This is the minimal end-to-end proof that the mark system works at
/// runtime in a browser.
#[wasm_bindgen_test]
async fn test_circle_render_smoke() {
    use gup::mark::Mark;
    use gup::mark::circle::{Circle, CircleInstance};

    let Some((_adapter, device, queue)) = try_get_device().await else {
        wasm_bindgen_test::console_log!("⚠️  No GPU – skipping render smoke test");
        return;
    };

    // 1. Create render target texture
    let width = 64u32;
    let height = 64u32;
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("render_target"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let target_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

    // 2. Upload circle instance data via storage buffer
    let instance = CircleInstance {
        center: [0.0, 0.0],
        radius: 0.5,
        _pad0: 0.0,
        fill_color: [1.0, 0.0, 0.0, 1.0],
        stroke_width: 0.0,
        _pad1: [0.0; 3],
        stroke_color: [0.0, 0.0, 0.0, 0.0],
    };
    let instance_bytes = bytemuck::bytes_of(&instance);
    let storage_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("instance_storage"),
        size: instance_bytes.len() as u64,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&storage_buffer, 0, instance_bytes);

    // 3. Upload vertex data
    let vertices = Circle::generate_vertices();
    let vertex_bytes = bytemuck::cast_slice(&vertices);
    let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("vertex_buffer"),
        size: vertex_bytes.len() as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&vertex_buffer, 0, vertex_bytes);

    // 4. Upload index data
    let indices = Circle::generate_indices().expect("Circle should have indices");
    let index_bytes = bytemuck::cast_slice(&indices);
    let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("index_buffer"),
        size: index_bytes.len() as u64,
        usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&index_buffer, 0, index_bytes);

    // 5. Create bind group layout + bind group for the storage buffer
    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("circle_bgl"),
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
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("circle_bg"),
        layout: &bind_group_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: storage_buffer.as_entire_binding(),
        }],
    });

    // 6. Create shader module from Circle's built-in shaders
    let vert_src = Circle::VERTEX_SHADER.expect("vertex shader");
    let frag_src = Circle::FRAGMENT_SHADER.expect("fragment shader");
    let vert_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("circle_vert"),
        source: wgpu::ShaderSource::Wgsl(vert_src.into()),
    });
    let frag_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("circle_frag"),
        source: wgpu::ShaderSource::Wgsl(frag_src.into()),
    });

    // 7. Create render pipeline
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("circle_pl"),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("circle_pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &vert_module,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<gup::mark::circle::CircleVertex>() as u64,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &[wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 0,
                    shader_location: 0,
                }],
            }],
        },
        fragment: Some(wgpu::FragmentState {
            module: &frag_module,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            ..Default::default()
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    });

    // 8. Record and submit a render pass
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("render_encoder"),
    });
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("circle_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &target_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..6, 0, 0..1);
    }
    queue.submit(std::iter::once(encoder.finish()));

    wasm_bindgen_test::console_log!("✅ Circle render pass completed without errors");
}

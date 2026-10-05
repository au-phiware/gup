// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! RFC-001 S1: `RenderContext`, `GupContext` and `gup_core::Context` draw
//! with one device per process, so GPU resources made through one can be
//! used through another (the "two contexts that can't work together" bug of
//! RFC-001 §2).

use gup::context::{GupContext, GupOptions};
use gup::render::RenderContext;

/// Whether `a` and `b` are the same device.
///
/// `wgpu::Device`'s `==` compares wgpu-core ids, and every `Instance` has
/// its own id space, so two devices from two instances are both `Id(0,1)`
/// and compare equal. Ask wgpu instead: make a bind group on `b` from a
/// layout made on `a`. On one device that is fine. A device of another
/// instance panics with "BindGroupLayout[Id(0,1)] does not exist" (the
/// `composite_*` examples' panic); another device of the same instance is a
/// validation error.
async fn same_device(a: &wgpu::Device, b: &wgpu::Device) -> bool {
    let layout = a.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("same_device probe"),
        entries: &[],
    });
    b.push_error_scope(wgpu::ErrorFilter::Validation);
    let quiet = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let made = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        b.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("same_device probe"),
            layout: &layout,
            entries: &[],
        })
    }));
    std::panic::set_hook(quiet);
    let validation = b.pop_error_scope().await;
    made.is_ok() && validation.is_none()
}

#[tokio::test]
async fn render_context_and_gup_context_share_one_device() {
    let render_context = RenderContext::new().await.expect("RenderContext");
    let gup_context = GupContext::new().await.expect("GupContext");
    let core = gup_core::Context::shared().expect("shared gup_core::Context");

    assert!(same_device(render_context.device(), &gup_context.device).await);
    assert!(same_device(&gup_context.device, render_context.device()).await);
    assert!(same_device(render_context.device(), core.device()).await);

    // A second pair is still the same device: nothing requests its own.
    let again = RenderContext::new().await.expect("RenderContext");
    let headless = GupContext::headless().await.expect("GupContext");
    assert!(same_device(again.device(), render_context.device()).await);
    assert!(same_device(&headless.device, render_context.device()).await);
}

/// A buffer and bind group made on the `RenderContext` device are used in
/// a pass on the `GupContext` queue. With two devices wgpu panics on
/// submit ("... does not exist" / "belongs to a different device").
#[tokio::test]
async fn resources_from_one_context_draw_through_the_other() {
    let render_context = RenderContext::new().await.expect("RenderContext");
    let gup_context = GupContext::new().await.expect("GupContext");
    let device = render_context.device();

    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("one_device layout"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only: false },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    });
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("one_device data"),
        size: 16,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("one_device bind group"),
        layout: &layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: buffer.as_entire_binding(),
        }],
    });
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("one_device shader"),
        source: wgpu::ShaderSource::Wgsl(
            "@group(0) @binding(0) var<storage, read_write> data: array<u32, 4>;
             @compute @workgroup_size(4) fn main(@builtin(local_invocation_index) i: u32) {
                 data[i] = i + 1u;
             }"
            .into(),
        ),
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("one_device pipeline"),
        layout: Some(
            &device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[&layout],
                push_constant_ranges: &[],
            }),
        ),
        module: &module,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    });

    // Encode and read back entirely through the GupContext's handles.
    let gup_device = gup_context.device.as_ref();
    let readback = gup_device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("one_device readback"),
        size: 16,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut encoder = gup_device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_compute_pass(&Default::default());
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(1, 1, 1);
    }
    encoder.copy_buffer_to_buffer(&buffer, 0, &readback, 0, 16);
    gup_context.queue.submit([encoder.finish()]);
    readback
        .slice(..)
        .map_async(wgpu::MapMode::Read, |r| r.unwrap());
    gup_device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("poll");
    let words: Vec<u32> = bytemuck::cast_slice(&readback.slice(..).get_mapped_range()).to_vec();
    assert_eq!(words, vec![1, 2, 3, 4]);
}

/// Options the shared device does not meet get a dedicated device, still
/// created by `gup_core`, rather than silently ignoring the request.
#[tokio::test]
async fn options_the_shared_device_cannot_meet_get_their_own_device() {
    let shared = gup_core::Context::shared().expect("shared gup_core::Context");
    let default = GupContext::with_options(GupOptions::default())
        .await
        .expect("default options");
    assert!(same_device(&default.device, shared.device()).await);

    let low_power = GupContext::with_options(GupOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        ..Default::default()
    })
    .await
    .expect("low-power options");
    // The negative control: a second device really is told apart.
    assert!(!same_device(&low_power.device, shared.device()).await);
}

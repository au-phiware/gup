// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GPU ≡ CPU conformance (RFC-001 §5): each scale's WGSL module is
//! dispatched in a compute pass over sampled inputs, stored exactly as the
//! column store stores them, and must match its f64 `CpuMirror` within
//! 0.25 px (position) or 1/255 (colour).

use super::{Linear, Log, Sequential};
use crate::channel::Px;
use crate::column::ColumnFormat;
use crate::context::Context;
use crate::encoding::{CpuMirror, DynShaderFn, Resource, ShaderFn};
use wgpu::util::DeviceExt;

const SAMPLES: usize = 1_000;

/// Run `S::ENTRY` over `inputs` (absolute values) on the GPU, returning
/// `out_components` floats per input.
fn dispatch<S: ShaderFn>(
    cx: &Context,
    func: &S,
    inputs: &[f64],
    out_components: usize,
    origin: Option<f64>,
) -> Vec<f32> {
    let relative = ShaderFn::input_format(func) == ColumnFormat::F32Relative;
    // The column store's representation: relative to the first value.
    let origin = origin.unwrap_or(if relative { inputs[0] } else { 0.0 });
    let column: Vec<f32> = inputs.iter().map(|v| (v - origin) as f32).collect();
    let base = ShaderFn::chunk_base(func, origin);
    let lut = ShaderFn::resources(func).into_iter().next();

    let (out_ty, call_tail) = if out_components == 4 {
        ("vec4<f32>", ", lut, smp")
    } else {
        ("f32", "")
    };
    let args = if relative {
        "xs[i], base.x, p"
    } else {
        "xs[i], p"
    };
    let lut_decls = if lut.is_some() {
        "@group(0) @binding(4) var lut: texture_2d<f32>;\n\
         @group(0) @binding(5) var smp: sampler;\n"
    } else {
        ""
    };
    let source = format!(
        "#import {path} as s\n\
         @group(0) @binding(0) var<uniform> p: s::Params;\n\
         @group(0) @binding(1) var<storage, read> xs: array<f32>;\n\
         @group(0) @binding(2) var<storage, read_write> out: array<{out_ty}>;\n\
         @group(0) @binding(3) var<uniform> base: vec4<f32>;\n\
         {lut_decls}\
         @compute @workgroup_size(256)\n\
         fn main(@builtin(global_invocation_id) id: vec3<u32>) {{\n\
             let i = id.x;\n\
             if (i >= arrayLength(&xs)) {{ return; }}\n\
             out[i] = s::{entry}({args}{call_tail});\n\
         }}\n",
        path = S::MODULE.import_path,
        entry = S::ENTRY,
    );
    let composed = cx
        .shaders()
        .compose("conformance", &source, &[S::MODULE])
        .unwrap_or_else(|e| panic!("{e}"));
    let device = cx.device();
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("conformance"),
        source: wgpu::ShaderSource::Naga(std::borrow::Cow::Owned(composed.module)),
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("conformance"),
        layout: None,
        module: &module,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    });

    let init = |contents: &[u8], usage| {
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents,
            usage,
        })
    };
    let params = init(
        &DynShaderFn::params_bytes(func).unwrap(),
        wgpu::BufferUsages::UNIFORM,
    );
    let xs = init(bytemuck::cast_slice(&column), wgpu::BufferUsages::STORAGE);
    let base = init(
        bytemuck::cast_slice(&[base, 0.0, 0.0, 0.0]),
        wgpu::BufferUsages::UNIFORM,
    );
    let out_size = (inputs.len() * out_components * 4) as u64;
    let out = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: out_size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: out_size,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let lut_view = lut.map(|Resource::Lut(data)| {
        device
            .create_texture_with_data(
                cx.queue(),
                &wgpu::TextureDescriptor {
                    label: None,
                    size: wgpu::Extent3d {
                        width: data.len() as u32,
                        height: 1,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                },
                wgpu::util::TextureDataOrder::LayerMajor,
                bytemuck::cast_slice(&data),
            )
            .create_view(&Default::default())
    });
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });
    let mut entries = vec![
        wgpu::BindGroupEntry {
            binding: 0,
            resource: params.as_entire_binding(),
        },
        wgpu::BindGroupEntry {
            binding: 1,
            resource: xs.as_entire_binding(),
        },
        wgpu::BindGroupEntry {
            binding: 2,
            resource: out.as_entire_binding(),
        },
    ];
    // Auto layouts drop unused bindings: only bind `base` when used.
    if relative {
        entries.push(wgpu::BindGroupEntry {
            binding: 3,
            resource: base.as_entire_binding(),
        });
    }
    if let Some(view) = &lut_view {
        entries.push(wgpu::BindGroupEntry {
            binding: 4,
            resource: wgpu::BindingResource::TextureView(view),
        });
        entries.push(wgpu::BindGroupEntry {
            binding: 5,
            resource: wgpu::BindingResource::Sampler(&sampler),
        });
    }
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &pipeline.get_bind_group_layout(0),
        entries: &entries,
    });

    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_compute_pass(&Default::default());
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(inputs.len().div_ceil(256) as u32, 1, 1);
    }
    encoder.copy_buffer_to_buffer(&out, 0, &readback, 0, out_size);
    cx.queue().submit([encoder.finish()]);
    readback
        .slice(..)
        .map_async(wgpu::MapMode::Read, |r| r.unwrap());
    cx.wait_idle().unwrap();
    let data = bytemuck::cast_slice(&readback.slice(..).get_mapped_range()).to_vec();
    readback.unmap();
    data
}

fn max_position_error<S: ShaderFn + CpuMirror<Out = Px>>(
    cx: &Context,
    func: &S,
    inputs: &[f64],
    origin: Option<f64>,
) -> f64 {
    let gpu = dispatch(cx, func, inputs, 1, origin);
    inputs
        .iter()
        .zip(gpu)
        .map(|(&x, g)| (f64::from(g) - func.eval(x)).abs())
        .fold(0.0, f64::max)
}

#[test]
fn linear_gpu_matches_cpu_mirror_on_timestamps() {
    let cx = Context::new_blocking().unwrap();
    // One day of Unix seconds onto 1000 px: absolute f32 would be off by
    // up to 64 s ≈ 0.7 px here; the relative column keeps it exact.
    let t0 = 1.7e9;
    let s = Linear::new()
        .domain(t0, t0 + 86_400.0)
        .range(Px(10.0), Px(1010.0));
    let inputs: Vec<f64> = (0..SAMPLES)
        .map(|i| t0 + 300.0 + i as f64 * 86.1234)
        .collect();
    let err = max_position_error(&cx, &s, &inputs, None);
    eprintln!("max |gpu - cpu| = {err:.2e} px");
    assert!(err <= 0.25, "max |gpu - cpu| = {err} px");
}

#[test]
fn log_gpu_matches_cpu_mirror() {
    let cx = Context::new_blocking().unwrap();
    let s = Log::new().domain(1.0, 1e9).range(Px(800.0), Px(20.0));
    let inputs: Vec<f64> = (0..SAMPLES)
        .map(|i| 10f64.powf(i as f64 / SAMPLES as f64 * 9.0))
        .collect();
    let err = max_position_error(&cx, &s, &inputs, None);
    eprintln!("max |gpu - cpu| = {err:.2e} px");
    assert!(err <= 0.25, "max |gpu - cpu| = {err} px");
}

#[test]
fn sequential_gpu_matches_cpu_mirror() {
    let cx = Context::new_blocking().unwrap();
    let s = Sequential::viridis().domain(-10.0, 40.0);
    // Includes out-of-domain values on both sides (clamped).
    let inputs: Vec<f64> = (0..SAMPLES)
        .map(|i| -15.0 + i as f64 * 60.0 / SAMPLES as f64)
        .collect();
    let gpu = dispatch(&cx, &s, &inputs, 4, None);
    let mut worst = 0.0f32;
    for (k, &x) in inputs.iter().enumerate() {
        let cpu = s.eval(x).to_array();
        for c in 0..4 {
            worst = worst.max((gpu[k * 4 + c] - cpu[c]).abs());
        }
    }
    eprintln!("max channel error = {worst:.2e}");
    assert!(worst <= 1.0 / 255.0, "max channel error {worst} (> 1/255)");
}

/// Negative control: the same timestamps stored as absolute f32 (origin 0)
/// miss by more than the tolerance, so the test above is sensitive and the
/// relative column format is what makes it pass.
#[test]
fn absolute_f32_timestamps_would_fail_the_tolerance() {
    let cx = Context::new_blocking().unwrap();
    let t0 = 1.7e9;
    let s = Linear::new()
        .domain(t0, t0 + 86_400.0)
        .range(Px(10.0), Px(1010.0));
    let inputs: Vec<f64> = (0..SAMPLES)
        .map(|i| t0 + 300.0 + i as f64 * 86.1234)
        .collect();
    let err = max_position_error(&cx, &s, &inputs, Some(0.0));
    eprintln!("absolute f32: max |gpu - cpu| = {err:.2} px");
    assert!(err > 0.25, "absolute f32 unexpectedly precise: {err} px");
}

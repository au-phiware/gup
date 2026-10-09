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
    let xs = cx
        .device()
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&column),
            usage: wgpu::BufferUsages::STORAGE,
        });
    let base = ShaderFn::chunk_base(func, origin);
    dispatch_column(
        cx,
        func,
        xs.as_entire_buffer_binding(),
        inputs.len(),
        base,
        out_components,
        relative,
    )
}

/// Run `S::ENTRY` over `n` stored `f32` values bound from `xs` (a column
/// as the column store holds it, e.g. a sub-range of an uploaded chunk),
/// with `base` as the chunk's base for relative entry points.
fn dispatch_column<S: ShaderFn>(
    cx: &Context,
    func: &S,
    xs: wgpu::BufferBinding<'_>,
    n: usize,
    base: f32,
    out_components: usize,
    relative: bool,
) -> Vec<f32> {
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
    let linked =
        crate::shader::link("conformance", &source, &[S::MODULE]).unwrap_or_else(|e| panic!("{e}"));
    let device = cx.device();
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("conformance"),
        source: wgpu::ShaderSource::Wgsl(linked.into()),
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
    let base = init(
        bytemuck::cast_slice(&[base, 0.0, 0.0, 0.0]),
        wgpu::BufferUsages::UNIFORM,
    );
    let out_size = (n * out_components * 4) as u64;
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
            resource: wgpu::BindingResource::Buffer(xs),
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
        pass.dispatch_workgroups(n.div_ceil(256) as u32, 1, 1);
    }
    encoder.copy_buffer_to_buffer(&out, 0, &readback, 0, out_size);
    cx.submit([encoder.finish()]);
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

/// RFC-001 S4a (AC4): precision at a chunk boundary. Unix-second
/// timestamps spanning three years (a stand-in for the `Time` scale of
/// S5), forced into 64-row chunks. Three chunks spread over the years,
/// then two dense chunks (80 rows a second) meet at `BOUNDARY`, then a
/// coarse tail. The x domain is a one-second window straddling the
/// boundary.
mod chunk_boundary {
    use super::*;
    use crate::column::ColumnStore;

    /// Where chunk 3 ends and chunk 4 begins (2023-11-14, Unix seconds).
    const BOUNDARY: f64 = 1.7e9;
    const CHUNK_ROWS: u32 = 64;

    fn timestamps() -> Vec<f64> {
        let t0 = 1.6e9; // 2020-09-13
        (0..384u32)
            .map(|i| match i {
                0..192 => t0 + f64::from(i) * (BOUNDARY - 10.0 - t0) / 192.0,
                192..320 => BOUNDARY + (f64::from(i) - 256.0 + 0.5) / 80.0,
                _ => BOUNDARY + 1.0 + f64::from(i - 320) * 1e6,
            })
            .collect()
    }

    /// One second over 1000 px, centred on the boundary.
    fn window() -> Linear {
        Linear::new()
            .domain(BOUNDARY - 0.5, BOUNDARY + 0.5)
            .range(Px(10.0), Px(1010.0))
    }

    /// Upload `values` as one `format` column in chunks of `chunk_rows`,
    /// run the scale's WGSL over every chunk's uploaded column with that
    /// chunk's base (as the glue does), and return the largest
    /// |GPU − CPU mirror| over the values within `half_width` seconds of
    /// the boundary (0.5: the window), with the number of chunks and of
    /// values compared.
    fn boundary_error(
        cx: &Context,
        values: &[f64],
        format: ColumnFormat,
        chunk_rows: u32,
        half_width: f64,
    ) -> (f64, usize, usize) {
        let scale = window();
        let mut store =
            ColumnStore::from_columns(vec![(format, values.to_vec())], chunk_rows).unwrap();
        store.upload(cx).unwrap();
        let (mut worst, mut compared) = (0.0f64, 0);
        for chunk in store.chunks() {
            let range = chunk.column_range(0);
            let gpu = dispatch_column(
                cx,
                &scale,
                wgpu::BufferBinding {
                    buffer: chunk.buffer(cx).unwrap(),
                    offset: range.start,
                    size: wgpu::BufferSize::new(range.end - range.start),
                },
                chunk.rows() as usize,
                ShaderFn::chunk_base(&scale, chunk.columns()[0].origin()),
                1,
                true,
            );
            let rows = chunk.row_base() as usize..;
            for (&x, g) in values[rows].iter().zip(gpu) {
                if (x - BOUNDARY).abs() <= half_width {
                    worst = worst.max((f64::from(g) - scale.eval(x)).abs());
                    compared += 1;
                }
            }
        }
        (worst, store.chunks().len(), compared)
    }

    #[test]
    fn relative_chunks_stay_within_a_quarter_pixel_at_the_boundary() {
        let cx = Context::new_blocking().unwrap();
        let values = timestamps();
        let (err, chunks, compared) =
            boundary_error(&cx, &values, ColumnFormat::F32Relative, CHUNK_ROWS, 0.5);
        eprintln!(
            "{chunks} relative chunks: max |gpu - cpu| = {err:.2e} px over {compared} points"
        );
        assert_eq!(chunks, 6);
        // 40 points on each side of the boundary, in two chunks.
        assert_eq!(compared, 80);
        assert!(err <= 0.25, "max |gpu - cpu| = {err} px");
    }

    /// Negative control (AC4): the same values as absolute `f32` (a
    /// 128 s ULP at 1.7e9) miss by far more than the tolerance.
    #[test]
    fn absolute_f32_misses_at_the_boundary() {
        let cx = Context::new_blocking().unwrap();
        let (err, _, compared) =
            boundary_error(&cx, &timestamps(), ColumnFormat::F32, CHUNK_ROWS, 0.5);
        eprintln!("absolute f32: max |gpu - cpu| = {err:.1} px over {compared} points");
        assert!(err > 0.25, "absolute f32 unexpectedly precise: {err} px");
    }

    /// Second control: relative, but in one chunk, so the origin is the
    /// first timestamp three years earlier (an 8 s ULP at ~1e8 s). The
    /// per-chunk origin, not the relative format alone, keeps the boundary
    /// inside the budget.
    #[test]
    fn one_relative_chunk_spanning_years_misses() {
        let cx = Context::new_blocking().unwrap();
        let (err, chunks, compared) = boundary_error(
            &cx,
            &timestamps(),
            ColumnFormat::F32Relative,
            ColumnStore::MAX_CHUNK_ROWS,
            0.5,
        );
        eprintln!(
            "1 relative chunk over 3 years: max |gpu - cpu| = {err:.1} px over {compared} points"
        );
        assert_eq!(chunks, 1);
        assert!(err > 0.25, "one chunk unexpectedly precise: {err} px");
    }

    /// The limit S5's `Time` scale inherits: a full default chunk (2^20
    /// rows) of one-per-second samples spans 12 days, so values near its
    /// end are ~1e6 s from the origin (a 1/16 s ULP). At the one-second
    /// window's 1000 px a second, the last 16 rows miss the quarter-pixel
    /// budget. Recorded, not a goal of S4a.
    #[test]
    fn a_full_chunk_of_seconds_misses_a_one_second_zoom() {
        let cx = Context::new_blocking().unwrap();
        let n = ColumnStore::MAX_CHUNK_ROWS;
        let values: Vec<f64> = (0..n)
            .map(|i| BOUNDARY - f64::from(n - i) + 0.3 + 0.123 * f64::from(i % 7) / 7.0)
            .collect();
        let (err, chunks, compared) =
            boundary_error(&cx, &values, ColumnFormat::F32Relative, n, 16.0);
        eprintln!(
            "2^20 one-second rows, one chunk: max |gpu - cpu| = {err:.1} px over {compared} points"
        );
        assert_eq!(chunks, 1);
        assert_eq!(compared, 16);
        assert!(err > 0.25, "{err} px");
    }
}

// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GPU ≡ CPU conformance (RFC-001 §5): each scale's WGSL module — or a
//! chain of them (`a.then(b)`) — is dispatched in a compute pass over
//! sampled inputs, stored exactly as the column store stores them, and
//! must match its f64 `CpuMirror` within 0.25 px (position) or 1/255
//! (colour).

use super::{Linear, Log, Sequential, Time};
use crate::channel::Px;
use crate::column::ColumnFormat;
use crate::context::Context;
use crate::encoding::{CpuMirror, DynShaderFn, EncodeFn, Resource};
use wgpu::util::DeviceExt;

const SAMPLES: usize = 1_000;

/// `values` as a column of `format` stores them, relative to `origin`.
fn column_bytes(format: ColumnFormat, values: &[f64], origin: f64) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(values.len() * format.stride() as usize);
    for &v in values {
        match format {
            ColumnFormat::U32 => bytes.extend((v as u32).to_le_bytes()),
            ColumnFormat::F32 => bytes.extend((v as f32).to_le_bytes()),
            ColumnFormat::F32Relative | ColumnFormat::F32x2Relative => {
                let [hi, lo] = format.split(v - origin);
                bytes.extend(hi.to_le_bytes());
                if format == ColumnFormat::F32x2Relative {
                    bytes.extend(lo.to_le_bytes());
                }
            }
        }
    }
    bytes
}

/// Run `f`'s chain over `inputs` (absolute values) on the GPU, stored as
/// its first link's column format relative to `origin` (default: the
/// first value, as a chunk does). Returns the output floats (1 or 4 per
/// input).
fn dispatch(cx: &Context, f: &impl EncodeFn, inputs: &[f64], origin: Option<f64>) -> Vec<f32> {
    let links = f.links();
    let format = links[0].input_format();
    let origin = origin.unwrap_or(if format.is_relative() { inputs[0] } else { 0.0 });
    let xs = cx
        .device()
        .create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: &column_bytes(format, inputs, origin),
            usage: wgpu::BufferUsages::STORAGE,
        });
    dispatch_links(
        cx,
        &links,
        xs.as_entire_buffer_binding(),
        inputs.len(),
        origin,
    )
}

/// Run a chain of `links` over `n` stored values bound from `xs` (a
/// column as the column store holds it, e.g. a sub-range of an uploaded
/// chunk) whose origin is `origin`. Each link is called as the glue calls
/// it: the first on the stored value with the chunk's base, each later one
/// on the previous result (as a hi/lo pair if it takes one) with the base
/// for origin 0.
fn dispatch_links(
    cx: &Context,
    links: &[&dyn DynShaderFn],
    xs: wgpu::BufferBinding<'_>,
    n: usize,
    origin: f64,
) -> Vec<f32> {
    let format = links[0].input_format();
    let out_wgsl = links.last().unwrap().out_wgsl();
    let out_components = if out_wgsl == "f32" { 1 } else { 4 };

    // Bindings: link k's params at 10 + 4k, its base at 11 + 4k and its
    // LUT and sampler at 12 + 4k, 13 + 4k. Auto layouts drop unused
    // bindings, so only used ones are bound below.
    let mut imports: Vec<&str> = Vec::new();
    let mut decls = String::new();
    let mut expr = "xs[i]".to_string();
    for (k, link) in links.iter().enumerate() {
        let path = link.module().import_path;
        let alias = match imports.iter().position(|p| *p == path) {
            Some(j) => j,
            None => {
                imports.push(path);
                imports.len() - 1
            }
        };
        let b = 10 + 4 * k;
        decls += &format!("@group(0) @binding({b}) var<uniform> p{k}: l{alias}::Params;\n");
        let link_format = link.input_format();
        if k > 0 && link_format == ColumnFormat::F32x2Relative {
            expr = format!("vec2<f32>({expr}, 0.0)");
        }
        let mut args = vec![expr];
        if link_format.is_relative() {
            decls += &format!(
                "@group(0) @binding({}) var<uniform> base{k}: vec4<f32>;\n",
                b + 1
            );
            args.push(if link_format == ColumnFormat::F32x2Relative {
                format!("base{k}.xy")
            } else {
                format!("base{k}.x")
            });
        }
        args.push(format!("p{k}"));
        if !link.resources().is_empty() {
            decls += &format!(
                "@group(0) @binding({}) var lut{k}: texture_2d<f32>;\n\
                 @group(0) @binding({}) var smp{k}: sampler;\n",
                b + 2,
                b + 3
            );
            args.push(format!("lut{k}"));
            args.push(format!("smp{k}"));
        }
        expr = format!("l{alias}::{}({})", link.entry(), args.join(", "));
    }
    let imports_src: String = imports
        .iter()
        .enumerate()
        .map(|(j, p)| format!("#import {p} as l{j}\n"))
        .collect();
    let source = format!(
        "{imports_src}\
         @group(0) @binding(1) var<storage, read> xs: array<{in_ty}>;\n\
         @group(0) @binding(2) var<storage, read_write> out: array<{out_wgsl}>;\n\
         {decls}\
         @compute @workgroup_size(256)\n\
         fn main(@builtin(global_invocation_id) id: vec3<u32>) {{\n\
             let i = id.x;\n\
             if (i >= arrayLength(&xs)) {{ return; }}\n\
             out[i] = {expr};\n\
         }}\n",
        in_ty = format.wgsl_type(),
    );
    let modules: Vec<_> = links.iter().map(|l| l.module()).collect();
    let linked = crate::shader::link("conformance", &source, &modules)
        .unwrap_or_else(|e| panic!("{e}\n{source}"));
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
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });
    // Per link: params, base and LUT view (kept alive for the bind group).
    let mut per_link = Vec::new();
    for (k, link) in links.iter().enumerate() {
        let params = init(&link.params_bytes().unwrap(), wgpu::BufferUsages::UNIFORM);
        let link_origin = if k == 0 { origin } else { 0.0 };
        let [hi, lo] = link.input_format().split(link.chunk_base(link_origin));
        let base = init(
            bytemuck::cast_slice(&[hi, lo, 0.0, 0.0]),
            wgpu::BufferUsages::UNIFORM,
        );
        let lut = link
            .resources()
            .into_iter()
            .next()
            .map(|Resource::Lut(data)| {
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
        per_link.push((params, base, lut));
    }
    let mut entries = vec![
        wgpu::BindGroupEntry {
            binding: 1,
            resource: wgpu::BindingResource::Buffer(xs),
        },
        wgpu::BindGroupEntry {
            binding: 2,
            resource: out.as_entire_binding(),
        },
    ];
    for (k, (link, (params, base, lut))) in links.iter().zip(&per_link).enumerate() {
        let b = 10 + 4 * k as u32;
        entries.push(wgpu::BindGroupEntry {
            binding: b,
            resource: params.as_entire_binding(),
        });
        if link.input_format().is_relative() {
            entries.push(wgpu::BindGroupEntry {
                binding: b + 1,
                resource: base.as_entire_binding(),
            });
        }
        if let Some(view) = lut {
            entries.push(wgpu::BindGroupEntry {
                binding: b + 2,
                resource: wgpu::BindingResource::TextureView(view),
            });
            entries.push(wgpu::BindGroupEntry {
                binding: b + 3,
                resource: wgpu::BindingResource::Sampler(&sampler),
            });
        }
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

fn max_position_error<S: CpuMirror<Output = Px>>(
    cx: &Context,
    func: &S,
    inputs: &[f64],
    origin: Option<f64>,
) -> f64 {
    let gpu = dispatch(cx, func, inputs, origin);
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
    let gpu = dispatch(&cx, &s, &inputs, None);
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
/// timestamps spanning three years, forced into 64-row chunks. Three
/// chunks spread over the years, then two dense chunks (80 rows a second)
/// meet at `BOUNDARY`, then a coarse tail. The x domain is a one-second
/// window straddling the boundary.
mod chunk_boundary {
    use super::*;
    use crate::column::ColumnStore;
    use crate::encoding::ShaderFn;

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

    /// `width` seconds over 1000 px, centred on the boundary.
    fn window(width: f64) -> Linear {
        Linear::new()
            .domain(BOUNDARY - width / 2.0, BOUNDARY + width / 2.0)
            .range(Px(10.0), Px(1010.0))
    }

    /// Upload `values` as one column of `scale`'s format in chunks of
    /// `chunk_rows`, run the scale's WGSL over every chunk's uploaded
    /// column with that chunk's base (as the glue does), and return the
    /// largest |GPU − CPU mirror| over the values within `half_width`
    /// seconds of the boundary, with the number of chunks and of values
    /// compared.
    fn boundary_error<S: ShaderFn + CpuMirror<Output = Px>>(
        cx: &Context,
        scale: &S,
        values: &[f64],
        format: ColumnFormat,
        chunk_rows: u32,
        half_width: f64,
    ) -> (f64, usize, usize) {
        let mut store =
            ColumnStore::from_columns(vec![(format, values.to_vec())], chunk_rows).unwrap();
        store.upload(cx).unwrap();
        let (mut worst, mut compared) = (0.0f64, 0);
        for chunk in store.chunks() {
            let range = chunk.column_range(0);
            let gpu = dispatch_links(
                cx,
                &[scale],
                wgpu::BufferBinding {
                    buffer: chunk.buffer(cx).unwrap(),
                    offset: range.start,
                    size: wgpu::BufferSize::new(range.end - range.start),
                },
                chunk.rows() as usize,
                chunk.columns()[0].origin(),
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

    /// An `F32Relative` column through `Linear` (the S4a store).
    fn relative_error(values: &[f64], chunk_rows: u32, half_width: f64) -> (f64, usize, usize) {
        let cx = Context::new_blocking().unwrap();
        boundary_error(
            &cx,
            &window(1.0),
            values,
            ColumnFormat::F32Relative,
            chunk_rows,
            half_width,
        )
    }

    #[test]
    fn relative_chunks_stay_within_a_quarter_pixel_at_the_boundary() {
        let values = timestamps();
        let (err, chunks, compared) = relative_error(&values, CHUNK_ROWS, 0.5);
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
        let (err, _, compared) = boundary_error(
            &cx,
            &window(1.0),
            &timestamps(),
            ColumnFormat::F32,
            CHUNK_ROWS,
            0.5,
        );
        eprintln!("absolute f32: max |gpu - cpu| = {err:.1} px over {compared} points");
        assert!(err > 0.25, "absolute f32 unexpectedly precise: {err} px");
    }

    /// Second control: relative, but in one chunk, so the origin is the
    /// first timestamp three years earlier (an 8 s ULP at ~1e8 s). The
    /// per-chunk origin, not the relative format alone, keeps the boundary
    /// inside the budget.
    #[test]
    fn one_relative_chunk_spanning_years_misses() {
        let (err, chunks, compared) =
            relative_error(&timestamps(), ColumnStore::MAX_CHUNK_ROWS, 0.5);
        eprintln!(
            "1 relative chunk over 3 years: max |gpu - cpu| = {err:.1} px over {compared} points"
        );
        assert_eq!(chunks, 1);
        assert!(err > 0.25, "one chunk unexpectedly precise: {err} px");
    }

    /// A full default chunk (2^20 rows) of one-per-second samples ending
    /// at the boundary: it spans 12 days, so values near its end are ~1e6 s
    /// from its origin. With `burst`, the last `burst` rows are replaced by
    /// points in the `width`-second window at the boundary instead.
    fn full_chunk_of_seconds(window: Option<(f64, u32)>) -> Vec<f64> {
        let n = ColumnStore::MAX_CHUNK_ROWS;
        let mut values: Vec<f64> = (0..n)
            .map(|i| BOUNDARY - f64::from(n - i) + 0.3 + 0.123 * f64::from(i % 7) / 7.0)
            .collect();
        if let Some((width, count)) = window {
            values.truncate((n - count) as usize);
            values.extend(burst(width, count));
        }
        values
    }

    /// The limit S4a recorded for `F32Relative`, kept as the negative
    /// control for `Time`'s hi/lo column: at the one-second window's 1000
    /// px a second, the last 16 rows of a full chunk of seconds (a 1/16 s
    /// ULP) miss the quarter-pixel budget.
    #[test]
    fn a_full_chunk_of_seconds_misses_a_one_second_zoom() {
        let n = ColumnStore::MAX_CHUNK_ROWS;
        let (err, chunks, compared) = relative_error(&full_chunk_of_seconds(None), n, 16.0);
        eprintln!(
            "2^20 one-second rows, one F32Relative chunk: max |gpu - cpu| = {err:.1} px over \
             {compared} points"
        );
        assert_eq!(chunks, 1);
        assert_eq!(compared, 16);
        assert!(err > 0.25, "{err} px");
    }

    /// GUP-418 AC4: the same full chunk through `Time`, whose hi/lo
    /// `F32x2Relative` column keeps the offsets from the origin exact, is
    /// within the budget at a one-second zoom.
    #[test]
    fn a_full_hi_lo_chunk_of_seconds_passes_a_one_second_zoom() {
        let cx = Context::new_blocking().unwrap();
        let n = ColumnStore::MAX_CHUNK_ROWS;
        let (err, chunks, compared) = boundary_error(
            &cx,
            &time_window(1.0),
            &full_chunk_of_seconds(None),
            ColumnFormat::F32x2Relative,
            n,
            16.0,
        );
        eprintln!(
            "2^20 one-second rows, one hi/lo chunk, 1 s zoom: max |gpu - cpu| = {err:.2e} px \
             over {compared} points"
        );
        assert_eq!((chunks, compared), (1, 16));
        assert!(err <= 0.25, "{err} px");
    }

    /// GUP-418 AC4: and at a one-millisecond zoom (1000 px a millisecond),
    /// over 256 points in the window at the far end of the chunk.
    #[test]
    fn a_full_hi_lo_chunk_of_seconds_passes_a_one_millisecond_zoom() {
        let cx = Context::new_blocking().unwrap();
        let n = ColumnStore::MAX_CHUNK_ROWS;
        let (err, chunks, compared) = boundary_error(
            &cx,
            &time_window(1e-3),
            &full_chunk_of_seconds(Some((1e-3, 256))),
            ColumnFormat::F32x2Relative,
            n,
            0.5e-3,
        );
        eprintln!(
            "2^20 one-second rows, one hi/lo chunk, 1 ms zoom: max |gpu - cpu| = {err:.2e} px \
             over {compared} points"
        );
        assert_eq!((chunks, compared), (1, 256));
        assert!(err <= 0.25, "{err} px");
    }

    /// `n` points spread over the `width`-second window centred on the
    /// boundary, at golden-ratio fractions (so no offset is a short binary
    /// fraction that would round the same way as the chunk's base).
    fn burst(width: f64, n: u32) -> impl Iterator<Item = f64> {
        (0..n).map(move |k| {
            let t = (0.1 + f64::from(k) * 0.618_033_988_749_895).fract();
            BOUNDARY + width * (t - 0.5)
        })
    }

    /// One chunk whose origin is `span` seconds before the boundary,
    /// followed by 256 points in the `width`-second window there: the
    /// compared points sit `span` seconds from their chunk's origin, the
    /// most a value-span cap of `span` allows.
    fn chunk_spanning(span: f64, width: f64) -> Vec<f64> {
        std::iter::once(BOUNDARY - span - 0.3137)
            .chain(burst(width, 256))
            .collect()
    }

    /// `Time` over the `width`-second window centred on the boundary.
    fn time_window(width: f64) -> Time {
        Time::new()
            .domain(BOUNDARY - width / 2.0, BOUNDARY + width / 2.0)
            .range(Px(10.0), Px(1010.0))
    }

    /// The GUP-418 spike's precision measurements (run with `--ignored
    /// --nocapture`): max |GPU − CPU| for `F32Relative` chunks (what a
    /// value-span cap of `span` seconds gives) and for hi/lo
    /// `F32x2Relative` chunks, at a one-second and a one-millisecond zoom
    /// over 1000 px. A full default chunk of one-per-second samples spans
    /// 2^20 s.
    #[test]
    #[ignore = "GUP-418 spike measurements; prints a table"]
    fn spike_precision_table() {
        let cx = Context::new_blocking().unwrap();
        let one = ColumnStore::MAX_CHUNK_ROWS;
        eprintln!("| chunk span (s) | zoom | F32Relative (px) | F32x2Relative (px) |");
        for (zoom, label) in [(1.0, "1 s"), (1e-3, "1 ms")] {
            let mut spans: Vec<f64> = vec![1.0e8, 1048576.0, 65536.0, 16384.0, 8192.0];
            spans.extend([4096.0, 2048.0, 1024.0, 64.0, 16.0, 8.0, 4.0, 2.0, 1.0]);
            for span in spans {
                let values = chunk_spanning(span, zoom);
                let (rel, chunks, n) = boundary_error(
                    &cx,
                    &window(zoom),
                    &values,
                    ColumnFormat::F32Relative,
                    one,
                    zoom / 2.0,
                );
                assert_eq!((chunks, n), (1, 256));
                let (pair, _, _) = boundary_error(
                    &cx,
                    &time_window(zoom),
                    &values,
                    ColumnFormat::F32x2Relative,
                    one,
                    zoom / 2.0,
                );
                eprintln!("| {span} | {label} | {rel:.3e} | {pair:.3e} |");
            }
        }
    }
}

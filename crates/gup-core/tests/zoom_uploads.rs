// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Zooming writes 0 column bytes (RFC-001 §1). 100K points, both domains
//! changed on every one of 300 frames, each resolved and drawn through
//! `Renderer::render` into an `ImageTarget` (the headless twin of the
//! window loop in `examples/zoom_bench.rs`). The upload counters on the
//! `Context` must show no column writes at all after the first frame, and
//! exactly three uniform writes per frame (encodings, chunk bases, view),
//! whether the layer is one column chunk or several (RFC-001 S4a: every
//! chunk's entry goes in one write).

mod common;

use common::scatter;
use gup_core::prelude::*;
use gup_core::{ImageTarget, Renderer};

const POINTS: usize = 100_000;
const FRAMES: usize = 300;

#[test]
fn zooming_100k_points_writes_no_column_bytes() {
    zoom(None, 1);
}

/// The same 100K points in 7 chunks of at most 16384 rows: 7 instanced
/// draws per frame, still 0 column bytes and 3 uniform writes per frame.
#[test]
fn zooming_100k_points_in_chunks_writes_no_column_bytes() {
    zoom(Some(16_384), 7);
}

fn zoom(max_chunk_rows: Option<u32>, chunks: usize) {
    let cx = Context::new_blocking().expect("headless context");
    let (mut plot, x, y) =
        scatter::plot_rows_chunked(scatter::countries_n(POINTS), Px(3.0), max_chunk_rows);
    let mut target = ImageTarget::new(&cx, 640, 400).unwrap();
    let mut renderer = Renderer::new();

    // Frame 0 evaluates the accessors and uploads each chunk once: exactly
    // the rows' bytes, one write per column per chunk.
    let first = plot.resolve(&cx, 640.0, 400.0).unwrap();
    let batch = first
        .scene
        .items
        .iter()
        .find_map(|i| match &i.kind {
            gup_core::scene::ItemKind::Marks(b) => Some(b.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(batch.chunks(), chunks);
    assert_eq!(batch.instances(), POINTS as u64);
    renderer.render(&cx, &first.scene, &mut target).unwrap();
    let after_upload = cx.upload_stats();
    assert_eq!(after_upload.columns.writes, 3 * chunks as u64);
    assert_eq!(after_upload.columns.bytes, (POINTS * 4 * 3) as u64);
    let (x0, x1) = x.read().current_domain().unwrap();
    let (y0, y1) = y.read().current_domain().unwrap();

    let mut images = Vec::new();
    for i in 1..=FRAMES {
        let k = 0.05f64.powf(i as f64 / FRAMES as f64);
        let c = x0 + 0.35 * (x1 - x0);
        x.write()
            .set_domain(c - (c - x0) * k, c + (x1 - c) * k)
            .unwrap();
        let (l0, l1) = (y0.log10(), y1.log10());
        let m = l0 + 0.6 * (l1 - l0);
        y.write()
            .set_domain(10f64.powf(m - (m - l0) * k), 10f64.powf(m + (l1 - m) * k))
            .unwrap();
        let resolved = plot.resolve(&cx, 640.0, 400.0).unwrap();
        renderer.render(&cx, &resolved.scene, &mut target).unwrap();
        // Read a few frames back to prove the zoom reached the GPU.
        if i % 100 == 0 {
            images.push(pollster::block_on(target.read(&cx)).unwrap());
        }
    }
    let written = cx.upload_stats() - after_upload;
    eprintln!("{FRAMES} zoomed frames of {POINTS} points wrote: {written:?}");

    assert_eq!(written.columns.bytes, 0, "{written:?}");
    assert_eq!(written.columns.writes, 0, "{written:?}");
    assert_eq!(written.uniforms.writes, 3 * FRAMES as u64, "{written:?}");
    // New label glyphs upload only the atlas rows they landed in, not
    // the whole 1 MB atlas.
    assert!(written.textures.bytes < 128 << 10, "{written:?}");
    assert_eq!(images.len(), 3);
    assert_ne!(images[0], images[1]);
    assert_ne!(images[1], images[2]);
}

/// CPU cost of one cached-pipeline zoom frame (GUP-410): `Plot::resolve`
/// and `Renderer::render` into a `TextureTarget`, with every pipeline
/// already cached. Measures the cost of the error scopes on the per-frame
/// path; run with
/// `cargo test -p gup-core --release --test zoom_uploads cached_frame_timings -- --ignored --nocapture`.
#[test]
#[ignore = "measurement, not a check; see GUP-410"]
fn cached_frame_timings() {
    use gup_core::{TargetOptions, TextureTarget};
    use std::time::{Duration, Instant};
    const RUNS: usize = 500;
    let cx = Context::new_blocking().expect("headless context");
    let (mut plot, x, _) = scatter::plot_rows(scatter::countries_n(POINTS), Px(3.0));
    let texture = cx.device().create_texture(&wgpu::TextureDescriptor {
        label: Some("timing target"),
        size: wgpu::Extent3d {
            width: 640,
            height: 400,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let mut target = TextureTarget::new(&cx, texture, TargetOptions::default()).unwrap();
    let mut renderer = Renderer::new();
    let (x0, x1) = {
        let first = plot.resolve(&cx, 640.0, 400.0).unwrap();
        renderer.render(&cx, &first.scene, &mut target).unwrap();
        x.read().current_domain().unwrap()
    };
    let (mut resolve, mut render) = (Vec::new(), Vec::new());
    for i in 0..RUNS {
        let k = 1.0 - 0.5 * (i % 50) as f64 / 50.0;
        x.write().set_domain(x0, x0 + (x1 - x0) * k).unwrap();
        let t = Instant::now();
        let resolved = plot.resolve(&cx, 640.0, 400.0).unwrap();
        resolve.push(t.elapsed());
        let t = Instant::now();
        renderer.render(&cx, &resolved.scene, &mut target).unwrap();
        render.push(t.elapsed());
        cx.device()
            .poll(wgpu::PollType::wait_indefinitely())
            .unwrap();
    }
    let summary = |name: &str, mut v: Vec<Duration>| {
        v.sort();
        let us = |d: Duration| d.as_secs_f64() * 1e6;
        eprintln!(
            "{name:<28} min {:>8.1} µs  median {:>8.1}  p90 {:>8.1}",
            us(v[0]),
            us(v[v.len() / 2]),
            us(v[v.len() * 9 / 10]),
        );
    };
    eprintln!(
        "profile: {}, frames: {RUNS}, pipelines created: {}",
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        },
        cx.pipeline_stats().pipelines_created
    );
    summary("Plot::resolve", resolve);
    summary("Renderer::render (cached)", render);
}

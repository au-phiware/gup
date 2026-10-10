// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Zooming writes 0 column bytes (RFC-001 §1), and nothing else a frame
//! costs grows either. 100K points, both domains changed on every one of
//! 300 frames, each resolved, prepared and drawn in one pass into an
//! `ImageTarget` (the headless twin of the window loop in
//! `examples/zoom_bench.rs`). These are the deterministic, CI-side proxies
//! for the timing budgets in `crates/gup-core/PERF_BUDGETS.md` (GUP-417):
//!
//! - **no extra upload**: no column or validity writes after the first
//!   frame, and exactly the recorded uniform bytes and writes per frame
//!   (encodings, chunk bases, view: three writes whether the layer is one
//!   column chunk or several, RFC-001 S4a);
//! - **no extra draw**: one instanced draw per column chunk, and the same,
//!   recorded number of draw calls in every frame;
//! - **no extra pipeline**: no glue program linked and no pipeline created
//!   after the first frame.

mod common;

use common::scatter;
use gup_core::prelude::*;
use gup_core::{ImageTarget, RenderTarget, Renderer, Scene};

const POINTS: usize = 100_000;
const FRAMES: usize = 300;

/// What one zoomed frame may cost, by layout.
struct Budget {
    /// Column chunks, so instanced mark draws.
    chunks: usize,
    /// Uniform bytes written per frame: `Encodings` (64 B), view (16 B)
    /// and every chunk's `Chunk` entry (8 B each, 256 B apart).
    uniform_bytes: u64,
    /// Guide instance bytes (rules, rects, glyph quads) written over the
    /// whole zoom, at most: the ticks move, so this is the recorded total.
    instance_bytes: u64,
    /// Draw calls per frame: the chunks' plus one per guide or text batch.
    draw_calls: usize,
}

#[test]
fn zooming_100k_points_writes_no_column_bytes() {
    zoom(
        None,
        &Budget {
            chunks: 1,
            uniform_bytes: 88,
            instance_bytes: 1_387_872,
            draw_calls: 4,
        },
    );
}

/// The same 100K points in 7 chunks of at most 16384 rows: 7 instanced
/// draws per frame, still 0 column bytes and 3 uniform writes per frame.
#[test]
fn zooming_100k_points_in_chunks_writes_no_column_bytes() {
    zoom(
        Some(16_384),
        &Budget {
            chunks: 7,
            uniform_bytes: 64 + 16 + 6 * 256 + 8,
            instance_bytes: 1_387_872,
            draw_calls: 10,
        },
    );
}

/// Prepare `scene` and draw it into `target` in one pass, as
/// `Renderer::render` and the window loop do; the frame's draw calls.
fn frame(cx: &Context, renderer: &mut Renderer, scene: &Scene, target: &mut ImageTarget) -> usize {
    let desc = target.desc();
    let frame = target.acquire(cx).unwrap();
    let prepared = renderer.prepare(cx, scene, &desc).unwrap();
    let mut encoder = cx.device().create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("zoom uploads"),
            color_attachments: &[Some(frame.color_attachment(prepared.clear_color()))],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        prepared.draw(&mut pass);
    }
    target.present(cx, frame, encoder.finish()).unwrap();
    prepared.draw_calls()
}

fn zoom(max_chunk_rows: Option<u32>, budget: &Budget) {
    let chunks = budget.chunks;
    let cx = Context::new_blocking().expect("headless context");
    let (mut plot, x, y) =
        scatter::plot_rows_chunked(scatter::countries_n(POINTS), Px(3.0), max_chunk_rows);
    let mut target = ImageTarget::new(&cx, 640, 400).unwrap();
    let mut renderer = Renderer::new();

    // Frame 0 evaluates the accessors, uploads each chunk once (exactly
    // the rows' bytes, one write per column per chunk) and creates the
    // pipelines.
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
    assert_eq!(
        batch.chunks(),
        chunks,
        "instanced mark draws (column chunks)"
    );
    assert_eq!(batch.instances(), POINTS as u64);
    let first_draws = frame(&cx, &mut renderer, &first.scene, &mut target);
    assert_eq!(
        first_draws, budget.draw_calls,
        "draw calls in the first frame"
    );
    let after_upload = cx.upload_stats();
    let pipelines = cx.pipeline_stats();
    assert_eq!(after_upload.columns.writes, 3 * chunks as u64);
    assert_eq!(after_upload.columns.bytes, (POINTS * 4 * 3) as u64);
    let (x0, x1) = x.read().current_domain().unwrap();
    let (y0, y1) = y.read().current_domain().unwrap();

    let mut images = Vec::new();
    let mut draw_calls = Vec::new();
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
        draw_calls.push(frame(&cx, &mut renderer, &resolved.scene, &mut target));
        // Read a few frames back to prove the zoom reached the GPU.
        if i % 100 == 0 {
            images.push(pollster::block_on(target.read(&cx)).unwrap());
        }
    }
    let written = cx.upload_stats() - after_upload;
    let after = cx.pipeline_stats();
    eprintln!("{FRAMES} zoomed frames of {POINTS} points wrote: {written:?}");

    // No extra upload.
    assert_eq!(written.columns.bytes, 0, "{written:?}");
    assert_eq!(written.columns.writes, 0, "{written:?}");
    assert_eq!(written.validity.bytes, 0, "{written:?}");
    assert_eq!(written.uniforms.writes, 3 * FRAMES as u64, "{written:?}");
    assert_eq!(
        written.uniforms.bytes,
        budget.uniform_bytes * FRAMES as u64,
        "{written:?}"
    );
    // Each guide draw writes its instances once, and no more bytes than
    // recorded.
    let guide_draws = (budget.draw_calls - chunks) as u64;
    assert_eq!(
        written.instances.writes,
        guide_draws * FRAMES as u64,
        "{written:?}"
    );
    assert!(
        written.instances.bytes <= budget.instance_bytes,
        "guide instances: {} B over {FRAMES} frames, budget {} B; if the guides changed \
         on purpose, record the new total here and in PERF_BUDGETS.md: {written:?}",
        written.instances.bytes,
        budget.instance_bytes
    );
    // New label glyphs upload only the atlas rows they landed in, not
    // the whole 1 MB atlas.
    assert!(written.textures.bytes < 128 << 10, "{written:?}");
    // No extra draw.
    assert!(
        draw_calls.iter().all(|&n| n == budget.draw_calls),
        "draw calls per frame {draw_calls:?}, expected {} every frame",
        budget.draw_calls
    );
    // No extra pipeline.
    assert_eq!(
        after.programs_linked, pipelines.programs_linked,
        "{after:?}"
    );
    assert_eq!(
        after.pipelines_created, pipelines.pipelines_created,
        "a pipeline was created after the first frame: {after:?}"
    );
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

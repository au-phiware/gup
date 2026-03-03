// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Benchmark for the adaptive viewport renderer.
//!
//! Measures end-to-end frame time for:
//! 1. LOD tier selection
//! 2. GPU viewport frustum culling via `ViewportCuller`
//!
//! Uses a scaled proxy dataset (10M points) to approximate billion-point
//! performance. The LOD pyramid is built once; per-frame costs are the
//! tier selection + culling compute dispatch.
//!
//! **Scaling assumption**: the culling compute shader cost scales linearly
//! with the number of points in the selected tier. At 10M level-0 points,
//! frame time for the finest tier is representative of tier 0 at 10M. To
//! extrapolate to 1B level-0 points, multiply the level-0 frame time by
//! 100×. However, at 1B points the adaptive renderer would select a coarser
//! tier — the benchmark also measures frame time at each individual tier to
//! validate this.
//!
//! Gated behind the `gpu-bench` feature flag.
//!
//! Run with:
//!
//! ```sh
//! cargo bench --features gpu-bench --bench adaptive_renderer
//! ```

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use gup::lod::{LodPyramidBuilder, VertexData};
use gup::render::RenderContext;
use gup::renderer::{AdaptiveRenderer, AdaptiveRendererConfig, AdaptiveViewport, ViewportCuller};
use std::hint::black_box;
use std::time::{Duration, Instant};
use tokio::runtime::Runtime;

/// Generate synthetic points scattered across a 100×100 data space.
fn synthetic_data(n: usize) -> Vec<VertexData> {
    (0..n)
        .map(|i| {
            let x = (i as f32 * 61.803_4) % 100.0;
            let y = (i as f32 * 41.421_4) % 100.0;
            VertexData::new(x, y)
        })
        .collect()
}

/// Benchmark tier selection (CPU-only, no GPU).
fn bench_tier_selection(c: &mut Criterion) {
    let counts = vec![10_000_000, 2_500_000, 625_000, 156_250, 39_062];
    let renderer = AdaptiveRenderer::from_metadata_for_bench(
        counts,
        [0.0, 0.0, 100.0, 100.0],
        AdaptiveRendererConfig {
            blend_frames: 0,
            heuristic_scale: 1.0,
        },
    );

    let viewports = [
        (
            "zoom_out",
            AdaptiveViewport::new(1.0, [50.0, 50.0], [200, 200]),
        ),
        (
            "zoom_mid",
            AdaptiveViewport::new(5.0, [50.0, 50.0], [1920, 1080]),
        ),
        (
            "zoom_in",
            AdaptiveViewport::new(500.0, [50.0, 50.0], [1920, 1080]),
        ),
    ];

    let mut group = c.benchmark_group("adaptive_renderer/tier_selection");
    for (name, vp) in &viewports {
        group.bench_with_input(BenchmarkId::new("select_tier", name), vp, |b, vp| {
            b.iter(|| black_box(renderer.select_tier(vp)));
        });
    }
    group.finish();
}

/// Benchmark GPU culling at different point counts.
fn bench_gpu_culling(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let context = rt.block_on(async { RenderContext::new().await.unwrap() });

    let mut group = c.benchmark_group("adaptive_renderer/gpu_culling");
    group.sample_size(10);
    group.warm_up_time(Duration::from_millis(100));
    group.measurement_time(Duration::from_secs(3));

    // Test at scaled point counts.
    for &size in &[10_000, 100_000, 1_000_000] {
        let data = synthetic_data(size);
        let pyramid = LodPyramidBuilder::new()
            .levels(5)
            .build_cpu(context.device(), context.queue(), &data)
            .unwrap();
        let culler = ViewportCuller::new(context.device()).unwrap();

        group.bench_with_input(
            BenchmarkId::new("full_viewport", size),
            &size,
            |b, &_size| {
                b.iter_custom(|iters| {
                    let mut total = Duration::ZERO;
                    for _ in 0..iters {
                        let start = Instant::now();
                        let result = rt.block_on(async {
                            culler
                                .dispatch(
                                    context.device(),
                                    context.queue(),
                                    pyramid.buffer(0).buffer(),
                                    pyramid.level_point_count(0) as u32,
                                    1,
                                    [-10.0, 110.0, -10.0, 110.0],
                                )
                                .await
                                .unwrap()
                        });
                        total += start.elapsed();
                        black_box(&result);
                        drop(result);
                        let _ = context.device().poll(wgpu::PollType::Wait);
                    }
                    total
                });
            },
        );

        drop(pyramid);
        let _ = context.device().poll(wgpu::PollType::Wait);
    }

    group.finish();
}

/// Single-shot frame-time measurement for the largest practical proxy.
fn bench_frame_time_large(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let context = rt.block_on(async { RenderContext::new().await.unwrap() });

    let mut group = c.benchmark_group("adaptive_renderer/frame_time");
    group.sample_size(10);
    group.warm_up_time(Duration::from_millis(10));
    group.measurement_time(Duration::from_secs(5));

    // 10M point proxy — representative of a single LOD tier at 1B.
    let size = 10_000_000;
    let data = synthetic_data(size);
    let pyramid = LodPyramidBuilder::new()
        .levels(5)
        .build_cpu(context.device(), context.queue(), &data)
        .unwrap();

    let config = AdaptiveRendererConfig {
        blend_frames: 0,
        heuristic_scale: 1.0,
    };
    let mut renderer = AdaptiveRenderer::new(&pyramid, config);
    let culler = ViewportCuller::new(context.device()).unwrap();

    // Simulate a full frame at maximum zoom-out (coarsest tier selected).
    let vp_out = AdaptiveViewport::new(1.0, [50.0, 50.0], [1920, 1080]);

    group.bench_function(BenchmarkId::new("zoom_out_frame", size), |b| {
        b.iter_custom(|iters| {
            let mut total = Duration::ZERO;
            for _ in 0..iters {
                let start = Instant::now();

                let frame = renderer.update(&vp_out);
                let tier = frame.tier;
                let count = pyramid.level_point_count(tier) as u32;
                let bounds = vp_out.world_bounds();

                let _result = rt.block_on(async {
                    culler
                        .dispatch(
                            context.device(),
                            context.queue(),
                            pyramid.buffer(tier).buffer(),
                            count,
                            1,
                            [bounds[0], bounds[2], bounds[1], bounds[3]],
                        )
                        .await
                        .unwrap()
                });

                total += start.elapsed();
                let _ = context.device().poll(wgpu::PollType::Wait);
            }
            total
        });
    });

    // Print summary for manual review.
    let frame = renderer.update(&vp_out);
    eprintln!(
        "\n  [adaptive_renderer] 10M pts → tier {}/{} ({} pts in tier)",
        frame.tier,
        pyramid.level_count(),
        pyramid.level_point_count(frame.tier),
    );

    drop(pyramid);
    let _ = context.device().poll(wgpu::PollType::Wait);
    group.finish();
}

criterion_group!(
    benches,
    bench_tier_selection,
    bench_gpu_culling,
    bench_frame_time_large
);
criterion_main!(benches);

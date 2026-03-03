// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Benchmark for LOD pyramid construction.
//!
//! Gated behind the `gpu-bench` feature flag because it requires a GPU device
//! and is not suitable for headless CI environments.
//!
//! **Note**: GPU buffer allocation in tight benchmark loops causes OOM on some
//! drivers (see buffer_benchmarks.rs for the same issue). The full pipeline
//! benchmark at 100K+ points uses single-shot timing with explicit GPU memory
//! reclamation.
//!
//! Run with:
//!
//! ```sh
//! cargo bench --features gpu-bench --bench lod_pyramid
//! ```

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use gup::RenderContext;
use gup::lod::{LodPyramidBuilder, VertexData};
use std::hint::black_box;
use std::time::{Duration, Instant};
use tokio::runtime::Runtime;

/// Generate synthetic points scattered across a unit square.
fn synthetic_data(n: usize) -> Vec<VertexData> {
    (0..n)
        .map(|i| {
            let x = (i as f32 * 0.618_034) % 1.0;
            let y = (i as f32 * 0.414_214) % 1.0;
            VertexData::new(x, y)
        })
        .collect()
}

/// Benchmark small-scale full pipeline (GPU buffer allocation + CPU
/// aggregation). Uses iter_custom with explicit memory reclamation.
fn bench_small_pipeline(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let context = rt.block_on(async { RenderContext::new().await.unwrap() });

    let mut group = c.benchmark_group("lod_pyramid");
    group.sample_size(10);
    group.warm_up_time(Duration::from_millis(100));
    group.measurement_time(Duration::from_secs(3));

    for &size in &[1_000, 10_000] {
        let data = synthetic_data(size);

        group.bench_with_input(BenchmarkId::new("5_levels", size), &data, |b, data| {
            b.iter_custom(|iters| {
                let mut total = Duration::ZERO;
                for _ in 0..iters {
                    let start = Instant::now();
                    let pyramid = LodPyramidBuilder::new()
                        .levels(5)
                        .build_cpu(context.device(), context.queue(), black_box(data))
                        .unwrap();
                    total += start.elapsed();
                    black_box(pyramid.level_count());
                    drop(pyramid);
                    let _ = context.device().poll(wgpu::PollType::Wait);
                }
                total
            });
        });
    }

    group.finish();
}

/// Single-shot timing for large-scale builds where iterative benchmarking
/// would exhaust GPU memory. Prints results to stdout alongside criterion.
fn bench_large_single_shot(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let context = rt.block_on(async { RenderContext::new().await.unwrap() });

    let mut group = c.benchmark_group("lod_pyramid_large");
    group.sample_size(10);
    group.warm_up_time(Duration::from_millis(10));
    group.measurement_time(Duration::from_secs(2));

    for &size in &[100_000, 1_000_000] {
        let data = synthetic_data(size);

        // Single-shot: build once, print result, and use the timing.
        let start = Instant::now();
        let pyramid = LodPyramidBuilder::new()
            .levels(5)
            .build_cpu(context.device(), context.queue(), &data)
            .unwrap();
        let elapsed = start.elapsed();
        let levels = pyramid.level_count();
        let points: Vec<usize> = (0..levels).map(|i| pyramid.level_point_count(i)).collect();
        drop(pyramid);
        let _ = context.device().poll(wgpu::PollType::Wait);

        eprintln!(
            "\n  [single-shot] {size} points, 5 levels: {elapsed:?} \
             (levels: {levels}, points: {points:?})",
        );

        // Still register a criterion benchmark with 1 iter per sample.
        group.bench_with_input(
            BenchmarkId::new("5_levels_single", size),
            &data,
            |b, data| {
                b.iter_custom(|_iters| {
                    let start = Instant::now();
                    let pyramid = LodPyramidBuilder::new()
                        .levels(5)
                        .build_cpu(context.device(), context.queue(), black_box(data))
                        .unwrap();
                    let elapsed = start.elapsed();
                    black_box(pyramid.level_count());
                    drop(pyramid);
                    let _ = context.device().poll(wgpu::PollType::Wait);
                    elapsed
                });
            },
        );
    }

    group.finish();
}

criterion_group!(benches, bench_small_pipeline, bench_large_single_shot);
criterion_main!(benches);

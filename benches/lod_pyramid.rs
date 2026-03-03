// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Benchmark for LOD pyramid construction.
//!
//! Gated behind the `gpu-bench` feature flag because it requires a GPU device
//! and is not suitable for headless CI environments.
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

fn bench_lod_pyramid_cpu(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let context = rt.block_on(async { RenderContext::new().await.unwrap() });

    let mut group = c.benchmark_group("lod_pyramid_cpu");

    for &size in &[1_000, 10_000, 100_000, 1_000_000] {
        let data = synthetic_data(size);

        group.bench_with_input(BenchmarkId::new("5_levels", size), &data, |b, data| {
            b.iter(|| {
                let pyramid = LodPyramidBuilder::new()
                    .levels(5)
                    .build_cpu(context.device(), context.queue(), black_box(data))
                    .unwrap();
                black_box(pyramid.level_count());
            });
        });
    }

    group.finish();
}

fn bench_lod_pyramid_large(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let context = rt.block_on(async { RenderContext::new().await.unwrap() });

    let mut group = c.benchmark_group("lod_pyramid_large");
    group.sample_size(10); // Large dataset — fewer samples.

    let data = synthetic_data(100_000_000);

    group.bench_function("100M_5_levels_cpu", |b| {
        b.iter(|| {
            let pyramid = LodPyramidBuilder::new()
                .levels(5)
                .build_cpu(context.device(), context.queue(), black_box(&data))
                .unwrap();
            black_box(pyramid.level_count());
        });
    });

    group.finish();
}

criterion_group!(benches, bench_lod_pyramid_cpu, bench_lod_pyramid_large);
criterion_main!(benches);

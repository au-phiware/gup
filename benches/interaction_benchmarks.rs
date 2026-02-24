// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Performance benchmarks for the GPU interaction system (GUP-077)
//!
//! This benchmark suite measures query performance across different dataset sizes,
//! query patterns, and API entry points. It covers:
//!
//! - Point queries: single-point hit testing at various dataset scales
//! - Region queries: rectangular area queries with varying coverage
//! - Batch queries: multiple simultaneous queries in a single GPU dispatch
//! - Streaming queries: chunked processing for very large datasets

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use gup::interaction::{GpuInteractionQuery, InteractionSystem, Rect, Renderable, Vec2};
use gup::selection::Selection;
use gup::{Circle, InteractionData, RenderContext};
use std::hint::black_box;
use std::sync::Arc;
use tokio::runtime::Runtime;

/// Test data element with position and value.
#[derive(Debug, Clone)]
struct BenchData {
    x: f32,
    y: f32,
}

impl BenchData {
    fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

impl InteractionData for BenchData {
    fn position(&self) -> [f32; 2] {
        [self.x, self.y]
    }
}

/// Generate a grid of data points spread evenly in a 1000×1000 space.
fn generate_grid_data(count: usize) -> Vec<BenchData> {
    let side = (count as f32).sqrt().ceil() as usize;
    let spacing = 1000.0 / side as f32;
    (0..count)
        .map(|i| {
            let col = i % side;
            let row = i / side;
            BenchData::new(col as f32 * spacing, row as f32 * spacing)
        })
        .collect()
}

/// Generate clustered data points (multiple dense groups).
fn generate_clustered_data(count: usize) -> Vec<BenchData> {
    let clusters = 10;
    let per_cluster = count / clusters;
    let mut data = Vec::with_capacity(count);
    for c in 0..clusters {
        let cx = (c % 4) as f32 * 250.0 + 125.0;
        let cy = (c / 4) as f32 * 333.0 + 166.0;
        for i in 0..per_cluster {
            let angle = i as f32 * std::f32::consts::TAU / per_cluster as f32;
            let r = (i as f32 * 0.5) % 50.0;
            data.push(BenchData::new(cx + r * angle.cos(), cy + r * angle.sin()));
        }
    }
    // Fill remaining
    while data.len() < count {
        data.push(BenchData::new(500.0, 500.0));
    }
    data
}

/// Create a selection from data using an async runtime.
fn create_selection(
    rt: &Runtime,
    context: &Arc<RenderContext>,
    data: Vec<BenchData>,
) -> Selection<BenchData, Circle> {
    rt.block_on(async {
        Selection::<BenchData, Circle>::new(data, Arc::clone(context))
            .expect("Failed to create selection")
    })
}

// ---------------------------------------------------------------------------
// Point query benchmarks
// ---------------------------------------------------------------------------

fn bench_point_queries(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let context = rt.block_on(async { Arc::new(RenderContext::new().await.unwrap()) });

    let mut group = c.benchmark_group("point_queries");
    // Limit sample size because each iteration involves GPU work.
    group.sample_size(20);

    for &size in &[1_000usize, 10_000, 100_000] {
        let data = generate_grid_data(size);
        let selection = create_selection(&rt, &context, data);

        group.bench_with_input(BenchmarkId::new("grid", size), &size, |b, _| {
            let mut system = rt
                .block_on(InteractionSystem::new(context.as_ref()))
                .unwrap();
            let sels: Vec<&dyn Renderable> = vec![&selection];

            b.iter(|| {
                let hits = rt
                    .block_on(system.query_point(Vec2::new(500.0, 500.0), &sels))
                    .unwrap();
                black_box(hits);
            });
        });
    }

    // Clustered data pattern
    for &size in &[1_000usize, 10_000, 100_000] {
        let data = generate_clustered_data(size);
        let selection = create_selection(&rt, &context, data);

        group.bench_with_input(BenchmarkId::new("clustered", size), &size, |b, _| {
            let mut system = rt
                .block_on(InteractionSystem::new(context.as_ref()))
                .unwrap();
            let sels: Vec<&dyn Renderable> = vec![&selection];

            b.iter(|| {
                let hits = rt
                    .block_on(system.query_point(Vec2::new(125.0, 166.0), &sels))
                    .unwrap();
                black_box(hits);
            });
        });
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Region query benchmarks
// ---------------------------------------------------------------------------

fn bench_region_queries(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let context = rt.block_on(async { Arc::new(RenderContext::new().await.unwrap()) });

    let mut group = c.benchmark_group("region_queries");
    group.sample_size(20);

    let size = 10_000usize;
    let data = generate_grid_data(size);
    let selection = create_selection(&rt, &context, data);

    // Small region (~1% coverage)
    group.bench_function("small_region_10k", |b| {
        let mut system = rt
            .block_on(InteractionSystem::new(context.as_ref()))
            .unwrap();
        let sels: Vec<&dyn Renderable> = vec![&selection];
        let region = Rect::new(Vec2::new(450.0, 450.0), Vec2::new(550.0, 550.0));

        b.iter(|| {
            let hits = rt.block_on(system.query_region(region, &sels)).unwrap();
            black_box(hits);
        });
    });

    // Medium region (~10% coverage)
    group.bench_function("medium_region_10k", |b| {
        let mut system = rt
            .block_on(InteractionSystem::new(context.as_ref()))
            .unwrap();
        let sels: Vec<&dyn Renderable> = vec![&selection];
        let region = Rect::new(Vec2::new(350.0, 350.0), Vec2::new(650.0, 650.0));

        b.iter(|| {
            let hits = rt.block_on(system.query_region(region, &sels)).unwrap();
            black_box(hits);
        });
    });

    // Large region (~50% coverage)
    group.bench_function("large_region_10k", |b| {
        let mut system = rt
            .block_on(InteractionSystem::new(context.as_ref()))
            .unwrap();
        let sels: Vec<&dyn Renderable> = vec![&selection];
        let region = Rect::new(Vec2::new(150.0, 150.0), Vec2::new(850.0, 850.0));

        b.iter(|| {
            let hits = rt.block_on(system.query_region(region, &sels)).unwrap();
            black_box(hits);
        });
    });

    // Scaling across dataset sizes
    for &ds_size in &[1_000usize, 10_000, 100_000] {
        let data = generate_grid_data(ds_size);
        let sel = create_selection(&rt, &context, data);

        group.bench_with_input(
            BenchmarkId::new("medium_region", ds_size),
            &ds_size,
            |b, _| {
                let mut system = rt
                    .block_on(InteractionSystem::new(context.as_ref()))
                    .unwrap();
                let sels: Vec<&dyn Renderable> = vec![&sel];
                let region = Rect::new(Vec2::new(350.0, 350.0), Vec2::new(650.0, 650.0));

                b.iter(|| {
                    let hits = rt.block_on(system.query_region(region, &sels)).unwrap();
                    black_box(hits);
                });
            },
        );
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Batch query benchmarks
// ---------------------------------------------------------------------------

fn bench_batch_queries(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let context = rt.block_on(async { Arc::new(RenderContext::new().await.unwrap()) });

    let mut group = c.benchmark_group("batch_queries");
    group.sample_size(20);

    let size = 10_000usize;
    let data = generate_grid_data(size);
    let selection = create_selection(&rt, &context, data);

    // Single query baseline for comparison
    group.bench_function("single_query_10k", |b| {
        let mut system = rt
            .block_on(InteractionSystem::new(context.as_ref()))
            .unwrap();
        let sels: Vec<&dyn Renderable> = vec![&selection];

        b.iter(|| {
            let hits = rt
                .block_on(system.query_point(Vec2::new(500.0, 500.0), &sels))
                .unwrap();
            black_box(hits);
        });
    });

    // Batch of 5 queries
    group.bench_function("batch_5_queries_10k", |b| {
        let mut system = rt
            .block_on(InteractionSystem::new(context.as_ref()))
            .unwrap();
        let sels: Vec<&dyn Renderable> = vec![&selection];
        let queries: Vec<GpuInteractionQuery> = (0..5)
            .map(|i| GpuInteractionQuery::point(Vec2::new(i as f32 * 200.0 + 100.0, 500.0), 1000))
            .collect();

        b.iter(|| {
            let hits = rt.block_on(system.query_batch(&queries, &sels)).unwrap();
            black_box(hits);
        });
    });

    // Batch of 10 queries
    group.bench_function("batch_10_queries_10k", |b| {
        let mut system = rt
            .block_on(InteractionSystem::new(context.as_ref()))
            .unwrap();
        let sels: Vec<&dyn Renderable> = vec![&selection];
        let queries: Vec<GpuInteractionQuery> = (0..10)
            .map(|i| GpuInteractionQuery::point(Vec2::new(i as f32 * 100.0 + 50.0, 500.0), 1000))
            .collect();

        b.iter(|| {
            let hits = rt.block_on(system.query_batch(&queries, &sels)).unwrap();
            black_box(hits);
        });
    });

    // Batch throughput: compare N individual queries vs one batch of N
    for &count in &[1usize, 5, 10, 20] {
        group.bench_with_input(
            BenchmarkId::new("individual_queries", count),
            &count,
            |b, &n| {
                let mut system = rt
                    .block_on(InteractionSystem::new(context.as_ref()))
                    .unwrap();
                let sels: Vec<&dyn Renderable> = vec![&selection];

                b.iter(|| {
                    for i in 0..n {
                        let pos = Vec2::new(i as f32 * (1000.0 / n as f32), 500.0);
                        let hits = rt.block_on(system.query_point(pos, &sels)).unwrap();
                        black_box(hits);
                    }
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("batched_queries", count),
            &count,
            |b, &n| {
                let mut system = rt
                    .block_on(InteractionSystem::new(context.as_ref()))
                    .unwrap();
                let sels: Vec<&dyn Renderable> = vec![&selection];
                let queries: Vec<GpuInteractionQuery> = (0..n)
                    .map(|i| {
                        GpuInteractionQuery::point(
                            Vec2::new(i as f32 * (1000.0 / n as f32), 500.0),
                            1000,
                        )
                    })
                    .collect();

                b.iter(|| {
                    let hits = rt.block_on(system.query_batch(&queries, &sels)).unwrap();
                    black_box(hits);
                });
            },
        );
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Streaming query benchmarks
// ---------------------------------------------------------------------------

fn bench_streaming_queries(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let context = rt.block_on(async { Arc::new(RenderContext::new().await.unwrap()) });

    let mut group = c.benchmark_group("streaming_queries");
    group.sample_size(10);

    for &size in &[10_000usize, 100_000] {
        let data = generate_grid_data(size);
        let selection = create_selection(&rt, &context, data);

        group.bench_with_input(BenchmarkId::new("stream_point", size), &size, |b, _| {
            let mut system = rt
                .block_on(InteractionSystem::new(context.as_ref()))
                .unwrap();
            let sels: Vec<&dyn Renderable> = vec![&selection];
            let query = GpuInteractionQuery::point(Vec2::new(500.0, 500.0), 100_000);

            b.iter(|| {
                let mut hit_count = 0u32;
                rt.block_on(system.query_stream(query, &sels, |_hit| {
                    hit_count += 1;
                    true // continue streaming
                }))
                .unwrap();
                black_box(hit_count);
            });
        });

        group.bench_with_input(BenchmarkId::new("stream_region", size), &size, |b, _| {
            let mut system = rt
                .block_on(InteractionSystem::new(context.as_ref()))
                .unwrap();
            let sels: Vec<&dyn Renderable> = vec![&selection];
            let region = Rect::new(Vec2::new(200.0, 200.0), Vec2::new(800.0, 800.0));
            let query = GpuInteractionQuery::region(region, 100_000);

            b.iter(|| {
                let mut hit_count = 0u32;
                rt.block_on(system.query_stream(query, &sels, |_hit| {
                    hit_count += 1;
                    true
                }))
                .unwrap();
                black_box(hit_count);
            });
        });
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Dataset scaling benchmark (measures how query time scales with element count)
// ---------------------------------------------------------------------------

fn bench_scaling(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let context = rt.block_on(async { Arc::new(RenderContext::new().await.unwrap()) });

    let mut group = c.benchmark_group("scaling");
    group.sample_size(10);

    for &size in &[100usize, 500, 1_000, 5_000, 10_000, 50_000, 100_000] {
        let data = generate_grid_data(size);
        let selection = create_selection(&rt, &context, data);

        group.bench_with_input(BenchmarkId::new("point_query", size), &size, |b, _| {
            let mut system = rt
                .block_on(InteractionSystem::new(context.as_ref()))
                .unwrap();
            let sels: Vec<&dyn Renderable> = vec![&selection];

            b.iter(|| {
                let hits = rt
                    .block_on(system.query_point(Vec2::new(500.0, 500.0), &sels))
                    .unwrap();
                black_box(hits);
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_point_queries,
    bench_region_queries,
    bench_batch_queries,
    bench_streaming_queries,
    bench_scaling,
);
criterion_main!(benches);

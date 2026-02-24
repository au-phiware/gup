// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Memory usage benchmarks for the interaction system (GUP-077)
//!
//! Measures memory overhead of spatial indexing structures and GPU buffer
//! allocation across different dataset sizes. Uses criterion for consistent
//! measurement methodology but focuses on memory metrics rather than time.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use gup::interaction::{InteractionSystem, Renderable, Vec2};
use gup::selection::Selection;
use gup::{Circle, InteractionData, RenderContext};
use std::hint::black_box;
use std::sync::Arc;
use tokio::runtime::Runtime;

/// Test data element for memory benchmarks.
#[derive(Debug, Clone)]
struct MemBenchData {
    x: f32,
    y: f32,
}

impl MemBenchData {
    fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

impl InteractionData for MemBenchData {
    fn position(&self) -> [f32; 2] {
        [self.x, self.y]
    }
}

/// Generate evenly spaced grid data.
fn generate_grid(count: usize) -> Vec<MemBenchData> {
    let side = (count as f32).sqrt().ceil() as usize;
    let spacing = 1000.0 / side as f32;
    (0..count)
        .map(|i| {
            let col = i % side;
            let row = i / side;
            MemBenchData::new(col as f32 * spacing, row as f32 * spacing)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Spatial index build benchmarks
// ---------------------------------------------------------------------------

fn bench_spatial_index_build(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let context = rt.block_on(async { Arc::new(RenderContext::new().await.unwrap()) });

    let mut group = c.benchmark_group("spatial_index_build");
    group.sample_size(10);

    for &size in &[1_000usize, 5_000, 10_000, 50_000, 100_000] {
        let data = generate_grid(size);
        let selection: Selection<MemBenchData, Circle> =
            rt.block_on(async { Selection::new(data, Arc::clone(&context)).expect("selection") });

        group.bench_with_input(BenchmarkId::new("build_index", size), &size, |b, _| {
            b.iter(|| {
                // Each iteration creates a fresh system (no cached index)
                // and performs a query that triggers index building for
                // datasets > 1000 elements.
                let mut system = rt
                    .block_on(InteractionSystem::new(context.as_ref()))
                    .unwrap();
                let sels: Vec<&dyn Renderable> = vec![&selection];
                let hits = rt
                    .block_on(system.query_point(Vec2::new(500.0, 500.0), &sels))
                    .unwrap();
                black_box(hits);
            });
        });
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Selection creation memory overhead
// ---------------------------------------------------------------------------

fn bench_selection_creation(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let context = rt.block_on(async { Arc::new(RenderContext::new().await.unwrap()) });

    let mut group = c.benchmark_group("selection_creation");
    group.sample_size(10);

    for &size in &[1_000usize, 10_000, 100_000] {
        group.bench_with_input(BenchmarkId::new("create_selection", size), &size, |b, _| {
            b.iter(|| {
                let data = generate_grid(size);
                let selection: Selection<MemBenchData, Circle> = rt.block_on(async {
                    Selection::new(data, Arc::clone(&context)).expect("selection")
                });
                black_box(&selection);
            });
        });
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Element extraction overhead (CPU work before GPU dispatch)
// ---------------------------------------------------------------------------

fn bench_element_extraction(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let context = rt.block_on(async { Arc::new(RenderContext::new().await.unwrap()) });

    let mut group = c.benchmark_group("element_extraction");
    group.sample_size(20);

    for &size in &[1_000usize, 10_000, 100_000] {
        let data = generate_grid(size);
        let selection: Selection<MemBenchData, Circle> =
            rt.block_on(async { Selection::new(data, Arc::clone(&context)).expect("selection") });

        group.bench_with_input(
            BenchmarkId::new("get_elements_for_interaction", size),
            &size,
            |b, _| {
                b.iter(|| {
                    let elements = selection.get_elements_for_interaction().unwrap();
                    black_box(elements.len());
                });
            },
        );
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Interaction system creation overhead
// ---------------------------------------------------------------------------

fn bench_system_creation(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let context = rt.block_on(async { Arc::new(RenderContext::new().await.unwrap()) });

    c.bench_function("interaction_system_creation", |b| {
        b.iter(|| {
            let system = rt
                .block_on(InteractionSystem::new(context.as_ref()))
                .unwrap();
            black_box(system);
        });
    });
}

// ---------------------------------------------------------------------------
// Repeated query caching benefit
// ---------------------------------------------------------------------------

fn bench_repeated_queries(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let context = rt.block_on(async { Arc::new(RenderContext::new().await.unwrap()) });

    let mut group = c.benchmark_group("repeated_queries");
    group.sample_size(20);

    let size = 10_000usize;
    let data = generate_grid(size);
    let selection: Selection<MemBenchData, Circle> =
        rt.block_on(async { Selection::new(data, Arc::clone(&context)).expect("selection") });

    // First query (includes index build)
    group.bench_function("first_query_10k", |b| {
        b.iter(|| {
            let mut system = rt
                .block_on(InteractionSystem::new(context.as_ref()))
                .unwrap();
            let sels: Vec<&dyn Renderable> = vec![&selection];
            let hits = rt
                .block_on(system.query_point(Vec2::new(500.0, 500.0), &sels))
                .unwrap();
            black_box(hits);
        });
    });

    // Second query (spatial index already built)
    group.bench_function("subsequent_query_10k", |b| {
        let mut system = rt
            .block_on(InteractionSystem::new(context.as_ref()))
            .unwrap();
        let sels: Vec<&dyn Renderable> = vec![&selection];
        // Warm-up: first query builds the index
        rt.block_on(system.query_point(Vec2::new(500.0, 500.0), &sels))
            .unwrap();

        b.iter(|| {
            let hits = rt
                .block_on(system.query_point(Vec2::new(500.0, 500.0), &sels))
                .unwrap();
            black_box(hits);
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_spatial_index_build,
    bench_selection_creation,
    bench_element_extraction,
    bench_system_creation,
    bench_repeated_queries,
);
criterion_main!(benches);

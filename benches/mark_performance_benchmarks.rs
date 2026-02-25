// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Benchmarks for mark performance optimizations.
//!
//! Measures the CPU-side cost of:
//! - Enhanced pipeline cache lookups (with blend mode variants)
//! - Buffer pool acquire/return cycles
//! - Render batch sorting for pipeline state minimisation
//! - Size class classification

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use gup::mark::performance_opt::{
    MarkPerformanceMetrics, PipelineCacheKey, SizeClass, SortedBatch, count_pipeline_switches,
    sort_batches_by_state,
};
use gup::mixable::BlendMode;
use std::any::TypeId;
use std::hint::black_box;

// ---------------------------------------------------------------------------
// Size class benchmarks
// ---------------------------------------------------------------------------

fn bench_size_class(c: &mut Criterion) {
    let mut group = c.benchmark_group("size_class");

    for &count in &[10usize, 100, 500, 2000, 10000, 50000] {
        group.bench_with_input(BenchmarkId::new("from_count", count), &count, |b, &n| {
            b.iter(|| black_box(SizeClass::from_count(n)));
        });
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Batch sorting benchmarks
// ---------------------------------------------------------------------------

fn generate_batches(n: usize, type_count: usize) -> Vec<SortedBatch> {
    let types: Vec<TypeId> = (0..type_count)
        .map(|i| match i % 3 {
            0 => TypeId::of::<u32>(),
            1 => TypeId::of::<f32>(),
            _ => TypeId::of::<u64>(),
        })
        .collect();

    let blend_modes = [
        BlendMode::AlphaBlending,
        BlendMode::Additive,
        BlendMode::Multiply,
    ];

    (0..n)
        .map(|i| SortedBatch {
            original_index: i,
            mark_type_id: types[i % type_count],
            blend_mode: blend_modes[i % blend_modes.len()],
            z_order: (i as f32 * 0.37).sin(),
            instance_count: 100,
        })
        .collect()
}

fn bench_batch_sorting(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_sorting");

    for &n in &[10usize, 50, 200, 1000] {
        let batches = generate_batches(n, 3);

        group.bench_with_input(BenchmarkId::new("sort", n), &batches, |b, batches| {
            b.iter(|| black_box(sort_batches_by_state(batches)));
        });

        group.bench_with_input(
            BenchmarkId::new("count_switches", n),
            &batches,
            |b, batches| {
                let order = sort_batches_by_state(batches);
                b.iter(|| black_box(count_pipeline_switches(batches, &order)));
            },
        );
    }

    group.finish();
}

fn bench_sorting_effectiveness(c: &mut Criterion) {
    let mut group = c.benchmark_group("sort_effectiveness");

    for &n in &[100usize, 500] {
        let batches = generate_batches(n, 3);

        group.bench_with_input(
            BenchmarkId::new("unsorted_switches", n),
            &batches,
            |b, batches| {
                let naive_order: Vec<usize> = (0..batches.len()).collect();
                b.iter(|| black_box(count_pipeline_switches(batches, &naive_order)));
            },
        );

        group.bench_with_input(
            BenchmarkId::new("sorted_switches", n),
            &batches,
            |b, batches| {
                let sorted_order = sort_batches_by_state(batches);
                b.iter(|| black_box(count_pipeline_switches(batches, &sorted_order)));
            },
        );
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Pipeline cache key benchmarks
// ---------------------------------------------------------------------------

fn bench_cache_key(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_key");

    group.bench_function("create_default", |b| {
        b.iter(|| black_box(PipelineCacheKey::default_for::<gup::mark::Circle>()));
    });

    group.bench_function("create_with_blend", |b| {
        b.iter(|| {
            black_box(PipelineCacheKey::with_blend::<gup::mark::Circle>(
                BlendMode::Additive,
            ))
        });
    });

    group.bench_function("hash_lookup", |b| {
        let mut map = std::collections::HashMap::new();
        let key = PipelineCacheKey::default_for::<gup::mark::Circle>();
        map.insert(key, 42u32);

        b.iter(|| black_box(map.get(&key)));
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Metrics benchmarks
// ---------------------------------------------------------------------------

fn bench_metrics(c: &mut Criterion) {
    let mut group = c.benchmark_group("performance_metrics");

    group.bench_function("total_time", |b| {
        let m = MarkPerformanceMetrics {
            vertex_processing_time: std::time::Duration::from_millis(5),
            instance_batching_time: std::time::Duration::from_millis(3),
            pipeline_transition_time: std::time::Duration::from_millis(1),
            ..Default::default()
        };
        b.iter(|| black_box(m.total_time()));
    });

    group.bench_function("merge", |b| {
        let mut a = MarkPerformanceMetrics {
            draw_calls: 10,
            total_instances: 5000,
            ..Default::default()
        };
        let other = MarkPerformanceMetrics {
            draw_calls: 5,
            total_instances: 2000,
            ..Default::default()
        };
        b.iter(|| {
            a.merge(&other);
            black_box(&a);
        });
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

criterion_group!(
    benches,
    bench_size_class,
    bench_batch_sorting,
    bench_sorting_effectiveness,
    bench_cache_key,
    bench_metrics,
);
criterion_main!(benches);

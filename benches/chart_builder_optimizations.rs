// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Benchmarks for chart builder performance optimizations.
//!
//! Demonstrates the performance impact of compile-time accessor resolution
//! and shader specialization optimizations.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use gup::chart_builder::accessor::AccessorValue;
use gup::chart_builder::optimized_accessor::{GenericAccessor, OptimizedAccessorFunction};
use gup::chart_builder::pipeline_cache::PipelineCache;
use gup::chart_builder::shader_specialization::{
    AccessorType, DataLayout, MarkType, ShaderSpecialization,
};
use std::hint::black_box;

#[derive(Clone)]
struct DataPoint {
    x: f32,
    y: f32,
    value: f32,
    category: String,
}

impl DataPoint {
    fn new(x: f32, y: f32) -> Self {
        Self {
            x,
            y,
            value: x + y,
            category: "test".to_string(),
        }
    }
}

fn create_test_data(size: usize) -> Vec<DataPoint> {
    (0..size)
        .map(|i| {
            let f = i as f32;
            DataPoint::new(f * 0.1, f * 0.2)
        })
        .collect()
}

// Benchmark accessor resolution overhead
fn bench_accessor_overhead(c: &mut Criterion) {
    let data = create_test_data(100_000);

    let mut group = c.benchmark_group("accessor_overhead");

    // Baseline: Direct field access
    group.bench_function("direct_field_access", |b| {
        b.iter(|| {
            let sum: f32 = data.iter().map(|d| black_box(d.x + d.y)).sum();
            black_box(sum);
        })
    });

    // Standard accessor with Box<dyn Fn>
    group.bench_function("boxed_accessor", |b| {
        let accessor: Box<dyn Fn(&DataPoint) -> AccessorValue + Send + Sync> =
            Box::new(|d: &DataPoint| AccessorValue::Float(d.x + d.y));

        b.iter(|| {
            let sum: f32 = data.iter().map(|d| black_box(accessor(d).as_f32())).sum();
            black_box(sum);
        })
    });

    // Optimized accessor with generics
    group.bench_function("generic_accessor", |b| {
        let accessor = GenericAccessor::new(|d: &DataPoint| d.x + d.y);

        b.iter(|| {
            let sum: f32 = data.iter().map(|d| black_box(accessor.extract(d))).sum();
            black_box(sum);
        })
    });

    // Optimized accessor function
    group.bench_function("optimized_accessor_function", |b| {
        let accessor = OptimizedAccessorFunction::new(|d: &DataPoint| d.x + d.y);

        b.iter(|| {
            let sum: f32 = data.iter().map(|d| black_box(accessor.apply(d))).sum();
            black_box(sum);
        })
    });

    group.finish();
}

// Benchmark simple field access patterns
fn bench_simple_field_access(c: &mut Criterion) {
    let data = create_test_data(100_000);

    let mut group = c.benchmark_group("simple_field_access");

    group.bench_function("direct_x_field", |b| {
        b.iter(|| {
            let sum: f32 = data.iter().map(|d| black_box(d.x)).sum();
            black_box(sum);
        })
    });

    group.bench_function("generic_accessor_x", |b| {
        let accessor = GenericAccessor::new(|d: &DataPoint| d.x);
        b.iter(|| {
            let sum: f32 = data.iter().map(|d| black_box(accessor.extract(d))).sum();
            black_box(sum);
        })
    });

    group.finish();
}

// Benchmark pipeline caching
fn bench_pipeline_caching(c: &mut Criterion) {
    let mut group = c.benchmark_group("pipeline_caching");

    let spec1 = ShaderSpecialization::new(
        DataLayout::SimpleFloat2,
        vec![AccessorType::DirectField, AccessorType::DirectField],
        MarkType::Circle,
    );

    let spec2 = ShaderSpecialization::new(
        DataLayout::Float2WithColor,
        vec![AccessorType::DirectField],
        MarkType::Rectangle,
    );

    // Benchmark cache hit performance
    group.bench_function("cache_hit", |b| {
        let mut cache = PipelineCache::new();
        // Pre-populate cache
        cache.get_or_create(&spec1);

        b.iter(|| {
            let pipeline = cache.get_or_create(&spec1);
            black_box(pipeline);
        })
    });

    // Benchmark cache miss (shader compilation)
    group.bench_function("cache_miss_compile", |b| {
        b.iter(|| {
            let mut cache = PipelineCache::new();
            let pipeline = cache.get_or_create(black_box(&spec1));
            black_box(pipeline);
        })
    });

    // Benchmark multiple specializations
    group.bench_function("multiple_specializations", |b| {
        b.iter(|| {
            let mut cache = PipelineCache::new();
            let p1 = cache.get_or_create(&spec1);
            let p2 = cache.get_or_create(&spec2);
            let p1_again = cache.get_or_create(&spec1);
            let p2_again = cache.get_or_create(&spec2);
            black_box((p1, p2, p1_again, p2_again));
        })
    });

    group.finish();
}

// Benchmark shader specialization
fn bench_shader_specialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("shader_specialization");

    let layouts = vec![
        DataLayout::SimpleFloat2,
        DataLayout::Float2WithColor,
        DataLayout::Float2WithColorAndSize,
    ];

    for layout in layouts {
        group.bench_with_input(
            BenchmarkId::new("generate_shader", format!("{:?}", layout)),
            &layout,
            |b, &layout| {
                let spec = ShaderSpecialization::new(
                    layout,
                    vec![AccessorType::DirectField, AccessorType::DirectField],
                    MarkType::Circle,
                );

                b.iter(|| {
                    let shader = spec.generate_specialized_shader();
                    black_box(shader);
                })
            },
        );
    }

    group.finish();
}

// Benchmark cache key generation
fn bench_cache_key_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_key_generation");

    let spec = ShaderSpecialization::new(
        DataLayout::SimpleFloat2,
        vec![AccessorType::DirectField, AccessorType::DirectField],
        MarkType::Circle,
    );

    group.bench_function("simple_specialization", |b| {
        b.iter(|| {
            let key = spec.cache_key();
            black_box(key);
        })
    });

    let complex_spec = ShaderSpecialization::new(
        DataLayout::Float2WithColorAndSize,
        vec![
            AccessorType::DirectField,
            AccessorType::Computed,
            AccessorType::Constant,
            AccessorType::DirectField,
        ],
        MarkType::Circle,
    );

    group.bench_function("complex_specialization", |b| {
        b.iter(|| {
            let key = complex_spec.cache_key();
            black_box(key);
        })
    });

    group.finish();
}

// Benchmark realistic chart builder scenario
fn bench_chart_builder_scenario(c: &mut Criterion) {
    let data = create_test_data(10_000);

    let mut group = c.benchmark_group("chart_builder_scenario");

    // Simulates building a chart with optimized accessors
    group.bench_function("optimized_chart_building", |b| {
        b.iter(|| {
            let x_accessor = GenericAccessor::new(|d: &DataPoint| d.x);
            let y_accessor = GenericAccessor::new(|d: &DataPoint| d.y);

            let mut cache = PipelineCache::new();
            let spec = ShaderSpecialization::new(
                DataLayout::SimpleFloat2,
                vec![AccessorType::DirectField, AccessorType::DirectField],
                MarkType::Circle,
            );
            let pipeline = cache.get_or_create(&spec);

            // Simulate applying accessors to data
            let processed: Vec<_> = data
                .iter()
                .map(|d| {
                    let x = x_accessor.extract(d);
                    let y = y_accessor.extract(d);
                    (x, y)
                })
                .collect();

            black_box((pipeline, processed));
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_accessor_overhead,
    bench_simple_field_access,
    bench_pipeline_caching,
    bench_shader_specialization,
    bench_cache_key_generation,
    bench_chart_builder_scenario,
);

criterion_main!(benches);

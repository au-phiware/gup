// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Benchmarks for error handling performance optimizations.
//!
//! These benchmarks measure the overhead of error creation, context generation,
//! and caching to validate the <2% performance overhead target.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use gup::error::{ErrorContextCache, GupError, LazyErrorContext};
use std::hint::black_box;

/// Benchmark error creation in hot paths.
fn bench_error_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("error_creation");

    // Benchmark lightweight error creation (hot path)
    group.bench_function("performance_target_missed", |b| {
        b.iter(|| {
            let error = GupError::performance_target_missed(16.67, 20.0);
            black_box(error)
        });
    });

    // Benchmark heavy error creation
    group.bench_function("gpu_memory_exhausted", |b| {
        b.iter(|| {
            let error = GupError::gpu_memory_exhausted(2048, 1024);
            black_box(error)
        });
    });

    // Benchmark shader compilation error
    group.bench_function("shader_compilation_failed", |b| {
        b.iter(|| {
            let error = GupError::shader_compilation_failed("vertex", "syntax error");
            black_box(error)
        });
    });

    group.finish();
}

/// Benchmark lazy error context creation.
fn bench_lazy_context(c: &mut Criterion) {
    let mut group = c.benchmark_group("lazy_context");

    // Benchmark lazy context creation (should be very fast)
    group.bench_function("lazy_creation", |b| {
        b.iter(|| {
            let error = GupError::performance_target_missed(16.67, 20.0);
            let lazy = LazyErrorContext::new(error);
            black_box(lazy)
        });
    });

    // Benchmark lazy context without forcing creation
    group.bench_function("lazy_error_access", |b| {
        let error = GupError::performance_target_missed(16.67, 20.0);
        let lazy = LazyErrorContext::new(error);

        b.iter(|| {
            let _err = lazy.error();
            black_box(_err)
        });
    });

    // Benchmark lazy context with forced creation (expensive)
    group.bench_function("lazy_forced_creation", |b| {
        b.iter(|| {
            let error = GupError::gpu_memory_exhausted(2048, 1024);
            let lazy = LazyErrorContext::new(error);
            // Clone to avoid borrow issues
            let ctx = lazy.context().clone();
            black_box(ctx)
        });
    });

    group.finish();
}

/// Benchmark error context caching.
fn bench_context_caching(c: &mut Criterion) {
    let mut group = c.benchmark_group("context_caching");

    // Benchmark cache miss (first access)
    group.bench_function("cache_miss", |b| {
        let cache = ErrorContextCache::new();
        let mut counter = 0;

        b.iter(|| {
            // Create unique errors to force cache misses
            let error = GupError::ConfigurationError {
                parameter: format!("param_{}", counter),
                message: "test".to_string(),
            };
            counter += 1;
            let ctx = cache.get_or_create_context(&error);
            black_box(ctx)
        });
    });

    // Benchmark cache hit (subsequent access)
    group.bench_function("cache_hit", |b| {
        let cache = ErrorContextCache::new();
        let error = GupError::gpu_memory_exhausted(2048, 1024);

        // Prime the cache
        let _ctx = cache.get_or_create_context(&error);

        b.iter(|| {
            let ctx = cache.get_or_create_context(&error);
            black_box(ctx)
        });
    });

    // Benchmark cache with varying hit rates
    for hit_rate in [50, 75, 90, 95] {
        group.bench_with_input(
            BenchmarkId::new("mixed_access", format!("{}%_hits", hit_rate)),
            &hit_rate,
            |b, &hit_rate| {
                let cache = ErrorContextCache::new();
                let common_error = GupError::gpu_memory_exhausted(2048, 1024);

                // Prime cache with common error
                let _ctx = cache.get_or_create_context(&common_error);

                let mut counter = 0;
                b.iter(|| {
                    let use_common = (counter % 100) < hit_rate;
                    counter += 1;

                    let error = if use_common {
                        common_error.clone()
                    } else {
                        GupError::ConfigurationError {
                            parameter: format!("unique_{}", counter),
                            message: "test".to_string(),
                        }
                    };

                    let ctx = cache.get_or_create_context(&error);
                    black_box(ctx)
                });
            },
        );
    }

    group.finish();
}

/// Benchmark fast-path error classification.
fn bench_error_classification(c: &mut Criterion) {
    let mut group = c.benchmark_group("error_classification");

    let error = GupError::gpu_memory_exhausted(2048, 1024);

    // Benchmark fast classification
    group.bench_function("category_fast", |b| {
        b.iter(|| {
            let category = error.category_fast();
            black_box(category)
        });
    });

    // Benchmark standard classification
    group.bench_function("category", |b| {
        b.iter(|| {
            let category = error.category();
            black_box(category)
        });
    });

    // Benchmark needs_full_context check
    group.bench_function("needs_full_context", |b| {
        b.iter(|| {
            let needs = error.needs_full_context();
            black_box(needs)
        });
    });

    // Benchmark is_hot_path_error check
    group.bench_function("is_hot_path_error", |b| {
        b.iter(|| {
            let is_hot = error.is_hot_path_error();
            black_box(is_hot)
        });
    });

    group.finish();
}

/// Benchmark complete error handling workflow.
fn bench_error_workflow(c: &mut Criterion) {
    let mut group = c.benchmark_group("error_workflow");

    // Hot path: frequent error without context
    group.bench_function("hot_path_no_context", |b| {
        b.iter(|| {
            let error = GupError::performance_target_missed(16.67, 20.0);
            let category = error.category_fast();
            let needs_context = error.needs_full_context();
            black_box((category, needs_context))
        });
    });

    // Hot path with lazy context
    group.bench_function("hot_path_lazy_context", |b| {
        b.iter(|| {
            let error = GupError::performance_target_missed(16.67, 20.0);
            let lazy = LazyErrorContext::new(error);
            let category = lazy.error().category_fast();
            black_box((lazy, category))
        });
    });

    // Critical path with cached context
    group.bench_function("critical_path_cached", |b| {
        let cache = ErrorContextCache::new();
        let error = GupError::gpu_memory_exhausted(2048, 1024);

        // Prime cache
        let _ctx = cache.get_or_create_context(&error);

        b.iter(|| {
            let ctx = cache.get_or_create_context(&error);
            let _suggestions = &ctx.recovery_suggestions;
            black_box(ctx)
        });
    });

    group.finish();
}

/// Benchmark memory allocation patterns.
fn bench_memory_allocation(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_allocation");

    // Measure allocation count for error creation
    group.bench_function("error_allocations", |b| {
        b.iter(|| {
            let error = GupError::gpu_memory_exhausted(2048, 1024);
            black_box(error)
        });
    });

    // Measure allocation count for lazy context
    group.bench_function("lazy_context_allocations", |b| {
        b.iter(|| {
            let error = GupError::gpu_memory_exhausted(2048, 1024);
            let lazy = LazyErrorContext::new(error);
            black_box(lazy)
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_error_creation,
    bench_lazy_context,
    bench_context_caching,
    bench_error_classification,
    bench_error_workflow,
    bench_memory_allocation,
);

criterion_main!(benches);

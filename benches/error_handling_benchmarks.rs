// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Benchmarks for error handling performance optimizations.
//!
//! These benchmarks measure the overhead of error creation and classification.

use criterion::{Criterion, criterion_group, criterion_main};
use gup::error::GupError;
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

    group.finish();
}

criterion_group!(benches, bench_error_creation, bench_error_classification);

criterion_main!(benches);

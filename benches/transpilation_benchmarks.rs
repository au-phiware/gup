// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Transpilation system benchmarks for the Rust-to-WGSL pipeline.
//!
//! Compares transpiled `#[shader_fn]` functions against hand-written
//! `#[wgsl_function]` equivalents to validate that the transpilation
//! approach introduces no measurable overhead.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use gup::shader_function::{self, ComposableShaderFunction};
use gup_macros::{shader_fn, wgsl_function};
use std::hint::black_box;

// ---------------------------------------------------------------------------
// Define matching pairs: transpiled vs manual
// ---------------------------------------------------------------------------

#[shader_fn]
fn transpiled_scale(value: f32, scale: f32, offset: f32) -> f32 {
    let result = value * scale + offset;
    return clamp(result, 0.0, 1.0);
}

#[wgsl_function]
fn manual_scale(value: f32, scale: f32, offset: f32) -> f32 {
    let result = value * scale + offset;
    return clamp(result, 0.0, 1.0);
}

#[shader_fn]
fn transpiled_complex(value: f32, freq: f32, amp: f32) -> f32 {
    let s = sin(value * freq);
    let c = cos(value * freq * 0.5);
    let combined = s * 0.7 + c * 0.3;
    return clamp(combined * amp, -1.0, 1.0);
}

#[wgsl_function]
fn manual_complex(value: f32, freq: f32, amp: f32) -> f32 {
    let s = sin(value * freq);
    let c = cos(value * freq * 0.5);
    let combined = s * 0.7 + c * 0.3;
    return clamp(combined * amp, -1.0, 1.0);
}

#[shader_fn]
fn transpiled_math_heavy(value: f32, a: f32, b: f32) -> f32 {
    let x = abs(value - a);
    let y = sqrt(max(x, 0.001));
    let z = pow(y, b);
    let w = exp(-z);
    return clamp(w, 0.0, 1.0);
}

#[wgsl_function]
fn manual_math_heavy(value: f32, a: f32, b: f32) -> f32 {
    let x = abs(value - a);
    let y = sqrt(max(x, 0.001));
    let z = pow(y, b);
    let w = exp(-z);
    return clamp(w, 0.0, 1.0);
}

// ---------------------------------------------------------------------------
// Benchmarks
// ---------------------------------------------------------------------------

fn bench_wgsl_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("transpilation_wgsl_generation");

    group.bench_function("transpiled_scale_generate", |b| {
        b.iter(|| {
            black_box(TranspiledScale::wgsl_function());
        });
    });

    group.bench_function("manual_scale_generate", |b| {
        b.iter(|| {
            black_box(ManualScale::wgsl_function());
        });
    });

    group.bench_function("transpiled_complex_generate", |b| {
        b.iter(|| {
            black_box(TranspiledComplex::wgsl_function());
        });
    });

    group.bench_function("manual_complex_generate", |b| {
        b.iter(|| {
            black_box(ManualComplex::wgsl_function());
        });
    });

    group.bench_function("transpiled_math_heavy_generate", |b| {
        b.iter(|| {
            black_box(TranspiledMathHeavy::wgsl_function());
        });
    });

    group.bench_function("manual_math_heavy_generate", |b| {
        b.iter(|| {
            black_box(ManualMathHeavy::wgsl_function());
        });
    });

    group.finish();
}

fn bench_uniform_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("transpilation_uniform_creation");

    group.bench_function("transpiled_scale_uniforms", |b| {
        let f = TranspiledScale::new(2.0, 1.0);
        b.iter(|| {
            black_box(f.create_uniforms());
        });
    });

    group.bench_function("manual_scale_uniforms", |b| {
        let f = ManualScale::new(2.0, 1.0);
        b.iter(|| {
            black_box(f.create_uniforms());
        });
    });

    group.bench_function("transpiled_complex_uniforms", |b| {
        let f = TranspiledComplex::new(2.0, 1.0);
        b.iter(|| {
            black_box(f.create_uniforms());
        });
    });

    group.bench_function("manual_complex_uniforms", |b| {
        let f = ManualComplex::new(2.0, 1.0);
        b.iter(|| {
            black_box(f.create_uniforms());
        });
    });

    group.finish();
}

fn bench_pipeline_composition(c: &mut Criterion) {
    let mut group = c.benchmark_group("transpilation_pipeline_composition");

    for &count in &[1, 3, 5, 10] {
        group.bench_with_input(
            BenchmarkId::new("transpiled_functions", count),
            &count,
            |b, &count| {
                b.iter(|| {
                    let mut pipeline = gup::shader_pipeline::ComposableShaderPipeline::new();
                    for _ in 0..count {
                        pipeline.add_function(TranspiledScale::new(2.0, 1.0));
                    }
                    black_box(pipeline.function_count());
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("manual_functions", count),
            &count,
            |b, &count| {
                b.iter(|| {
                    let mut pipeline = gup::shader_pipeline::ComposableShaderPipeline::new();
                    for _ in 0..count {
                        pipeline.add_function(ManualScale::new(2.0, 1.0));
                    }
                    black_box(pipeline.function_count());
                });
            },
        );
    }

    group.finish();
}

fn bench_wgsl_output_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("transpilation_wgsl_quality");

    // Measure the size of generated WGSL (smaller is better for shader compilation)
    group.bench_function("transpiled_scale_wgsl_size", |b| {
        b.iter(|| {
            let wgsl = TranspiledScale::wgsl_function();
            black_box(wgsl.len());
        });
    });

    group.bench_function("manual_scale_wgsl_size", |b| {
        b.iter(|| {
            let wgsl = ManualScale::wgsl_function();
            black_box(wgsl.len());
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_wgsl_generation,
    bench_uniform_creation,
    bench_pipeline_composition,
    bench_wgsl_output_comparison
);
criterion_main!(benches);

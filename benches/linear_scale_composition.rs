// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Criterion benchmarks for LinearScale composition throughput (GUP-252).
//!
//! Measures the cost of composing `LinearScale` instances into a shader
//! pipeline. Target: ≤ 100 µs for 1 000 compositions.

use criterion::{Criterion, criterion_group, criterion_main};
use gup::shader_function::{ComposableFunction, ComposableShaderFunction, LinearScale};
use std::hint::black_box;

/// Benchmark composing 1 000 LinearScale instances via `.compose()`.
fn bench_linear_scale_composition(c: &mut Criterion) {
    let mut group = c.benchmark_group("linear_scale_composition");

    group.bench_function("compose_1000", |b| {
        b.iter(|| {
            let mut wgsl_total_len = 0usize;
            for i in 0..1_000 {
                let a = LinearScale::new(0.0, (i + 1) as f32, 0.0, 1.0);
                let b = LinearScale::new(0.0, 1.0, 0.0, (i + 1) as f32);
                let composed = a.compose(b);
                wgsl_total_len += composed.generate_wgsl().len();
            }
            black_box(wgsl_total_len);
        });
    });

    group.bench_function("compose_with_clamp_1000", |b| {
        b.iter(|| {
            let mut wgsl_total_len = 0usize;
            for i in 0..1_000 {
                let a = LinearScale::with_clamp(0.0, (i + 1) as f32, 0.0, 1.0);
                let b = LinearScale::with_clamp(0.0, 1.0, 0.0, (i + 1) as f32);
                let composed = a.compose(b);
                wgsl_total_len += composed.generate_wgsl().len();
            }
            black_box(wgsl_total_len);
        });
    });

    group.bench_function("compose_forward_invert_1000", |b| {
        b.iter(|| {
            let mut wgsl_total_len = 0usize;
            for i in 0..1_000 {
                let scale = LinearScale::new(0.0, (i + 1) as f32, 0.0, 1.0);
                let inv = scale.invert();
                let composed = scale.compose(inv);
                wgsl_total_len += composed.generate_wgsl().len();
            }
            black_box(wgsl_total_len);
        });
    });

    group.bench_function("create_uniforms_1000", |b| {
        b.iter(|| {
            let mut total = 0u32;
            for i in 0..1_000 {
                let scale = LinearScale::new(0.0, (i + 1) as f32, 0.0, 1.0);
                if let Some(u) = scale.create_uniforms() {
                    total += u.clamp;
                }
            }
            black_box(total);
        });
    });

    group.finish();
}

criterion_group!(benches, bench_linear_scale_composition);
criterion_main!(benches);

// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! CPU vs GPU brush region query benchmark (GUP-286).
//!
//! Compares [`MarkSelectionSystem::filter_by_rect`] (CPU) against
//! [`MarkSelectionSystem::rect_hit_test_gpu`] (GPU) for 100K, 500K, and
//! 1M mark datasets.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use gup::RenderContext;
use gup::interaction::{InteractionSystem, Rect, Vec2};
use gup::mark_selection::MarkSelectionSystem;
use tokio::runtime::Runtime;

/// Generate `n` pseudo-random positions in [-1.0, 1.0].
fn generate_positions(n: usize) -> Vec<[f32; 2]> {
    let mut rng: u32 = 0xDEAD_BEEF;
    let mut next = || -> f32 {
        rng ^= rng << 13;
        rng ^= rng >> 17;
        rng ^= rng << 5;
        (rng as f32 / u32::MAX as f32) * 2.0 - 1.0
    };
    (0..n).map(|_| [next(), next()]).collect()
}

/// Benchmark CPU filter_by_rect across dataset sizes.
fn bench_cpu_region_query(c: &mut Criterion) {
    let mut group = c.benchmark_group("brush_region_query_cpu");
    group.sample_size(30);

    for &size in &[100_000, 500_000, 1_000_000] {
        let positions = generate_positions(size);
        // Query a rect covering ~25% of the area.
        let rect = Rect::new(Vec2::new(-0.5, -0.5), Vec2::new(0.5, 0.5));

        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
            b.iter(|| {
                let ids = MarkSelectionSystem::filter_by_rect(&rect, &positions);
                std::hint::black_box(ids);
            });
        });
    }
    group.finish();
}

/// Benchmark GPU rect_hit_test_gpu across dataset sizes.
fn bench_gpu_region_query(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let (context, mut interaction) = rt.block_on(async {
        let ctx = RenderContext::new().await.unwrap();
        let is = InteractionSystem::new(&ctx).await.unwrap();
        (ctx, is)
    });
    // Suppress unused-variable warning; context must stay alive for GPU.
    let _ = &context;

    let mut group = c.benchmark_group("brush_region_query_gpu");
    group.sample_size(20);

    for &size in &[100_000, 500_000, 1_000_000] {
        let positions = generate_positions(size);
        let mut system = MarkSelectionSystem::new(size);
        system.set_positions(positions);

        let rect = Rect::new(Vec2::new(-0.5, -0.5), Vec2::new(0.5, 0.5));

        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
            b.iter(|| {
                let ids = rt.block_on(async {
                    system
                        .rect_hit_test_gpu(&rect, &mut interaction)
                        .await
                        .unwrap()
                });
                std::hint::black_box(ids);
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_cpu_region_query, bench_gpu_region_query);
criterion_main!(benches);

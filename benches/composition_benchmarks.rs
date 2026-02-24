// Copyright (C) 2025 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Performance benchmarks for composition optimizations (GUP-028)

#![allow(dead_code)]

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use gup::mixable::{BlendMode, Mixable, MixableExt};
use gup::{RenderContext, Viewport};
use pollster::FutureExt;
use std::hint::black_box;

/// Simple test visualization for benchmarking
#[derive(Debug)]
struct BenchVisualization {
    name: String,
}

impl BenchVisualization {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
        }
    }
}

impl Mixable for BenchVisualization {
    type Output = ();

    fn render(&mut self, _context: &mut RenderContext) -> gup::error::GupResult<Self::Output> {
        // Minimal work to focus on composition overhead
        Ok(())
    }
}

/// Benchmark viewport calculation caching
fn bench_viewport_caching(c: &mut Criterion) {
    let mut group = c.benchmark_group("viewport_caching");

    for size in [100, 500, 1000, 2000].iter() {
        // Benchmark with cache (repeated renders)
        group.bench_with_input(BenchmarkId::new("cached", size), size, |b, &size| {
            b.iter(|| {
                let mut context = RenderContext::new().block_on().unwrap();
                context
                    .set_viewport(Viewport {
                        width: size,
                        height: size,
                        scale_factor: 1.0,
                    })
                    .unwrap();

                let viz1 = BenchVisualization::new("viz1");
                let viz2 = BenchVisualization::new("viz2");
                let mut composition = viz1.beside(viz2);

                // First render populates cache
                composition.render(&mut context).unwrap();

                // Measure repeated renders (should hit cache)
                for _ in 0..10 {
                    composition.render(&mut context).unwrap();
                    black_box(());
                }
            });
        });
    }

    group.finish();
}

/// Benchmark nested composition depth scaling
fn bench_nested_composition_depth(c: &mut Criterion) {
    let mut group = c.benchmark_group("nested_composition");

    // Test different composition depths
    group.bench_function("depth_1", |b| {
        b.iter(|| {
            let mut context = RenderContext::new().block_on().unwrap();
            let viz1 = BenchVisualization::new("viz1");
            let viz2 = BenchVisualization::new("viz2");
            let mut composition = viz1.overlay(viz2);
            composition.render(&mut context).unwrap();
            black_box(());
        });
    });

    group.bench_function("depth_2", |b| {
        b.iter(|| {
            let mut context = RenderContext::new().block_on().unwrap();
            let viz1 = BenchVisualization::new("viz1");
            let viz2 = BenchVisualization::new("viz2");
            let viz3 = BenchVisualization::new("viz3");
            let mut composition = viz1.overlay(viz2).overlay(viz3);
            composition.render(&mut context).unwrap();
            black_box(());
        });
    });

    group.bench_function("depth_5", |b| {
        b.iter(|| {
            let mut context = RenderContext::new().block_on().unwrap();
            let viz1 = BenchVisualization::new("viz1");
            let viz2 = BenchVisualization::new("viz2");
            let viz3 = BenchVisualization::new("viz3");
            let viz4 = BenchVisualization::new("viz4");
            let viz5 = BenchVisualization::new("viz5");
            let viz6 = BenchVisualization::new("viz6");
            let mut composition = viz1
                .overlay(viz2)
                .overlay(viz3)
                .overlay(viz4)
                .overlay(viz5)
                .overlay(viz6);
            composition.render(&mut context).unwrap();
            black_box(());
        });
    });

    group.finish();
}

/// Benchmark pipeline cache effectiveness
fn bench_pipeline_cache(c: &mut Criterion) {
    let mut group = c.benchmark_group("pipeline_cache");

    // Benchmark with frequent blend mode changes
    group.bench_function("blend_mode_changes", |b| {
        b.iter(|| {
            let mut context = RenderContext::new().block_on().unwrap();

            // Repeatedly change between blend modes
            for i in 0..100 {
                let mode = match i % 4 {
                    0 => BlendMode::None,
                    1 => BlendMode::AlphaBlending,
                    2 => BlendMode::Additive,
                    _ => BlendMode::Multiply,
                };
                context.set_blend_mode(mode).unwrap();
                black_box(());
                // First time through creates pipelines, subsequent uses cache
                let _ = black_box(context.get_pipeline_with_blend(mode));
            }

            // Check that cache was effective
            let stats = context.pipeline_cache_stats();
            assert!(stats.hit_rate() > 0.9); // Should have >90% hit rate
        });
    });

    group.finish();
}

/// Benchmark state change batching
fn bench_state_batching(c: &mut Criterion) {
    let mut group = c.benchmark_group("state_batching");

    // Benchmark individual state changes
    group.bench_function("individual_changes", |b| {
        b.iter(|| {
            let mut context = RenderContext::new().block_on().unwrap();

            for i in 0..100 {
                context.set_blend_mode(BlendMode::AlphaBlending).unwrap();
                black_box(());
                context
                    .set_viewport(Viewport {
                        width: 800 + i,
                        height: 600 + i,
                        scale_factor: 1.0,
                    })
                    .unwrap();
                black_box(());
                context.set_global_alpha(0.5).unwrap();
                black_box(());
            }
        });
    });

    // Benchmark batched state changes
    group.bench_function("batched_changes", |b| {
        b.iter(|| {
            let mut context = RenderContext::new().block_on().unwrap();

            for i in 0..100 {
                context
                    .begin_state_batch()
                    .set_blend_mode(BlendMode::AlphaBlending)
                    .set_viewport(Viewport {
                        width: 800 + i,
                        height: 600 + i,
                        scale_factor: 1.0,
                    })
                    .set_global_alpha(0.5)
                    .commit()
                    .unwrap();
                black_box(());
            }
        });
    });

    group.finish();
}

/// Benchmark composition overhead vs direct rendering
fn bench_composition_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("composition_overhead");

    // Benchmark direct rendering
    group.bench_function("direct", |b| {
        b.iter(|| {
            let mut context = RenderContext::new().block_on().unwrap();
            let mut viz = BenchVisualization::new("direct");
            viz.render(&mut context).unwrap();
            black_box(());
        });
    });

    // Benchmark overlay composition (simplest)
    group.bench_function("overlay", |b| {
        b.iter(|| {
            let mut context = RenderContext::new().block_on().unwrap();
            let viz1 = BenchVisualization::new("viz1");
            let viz2 = BenchVisualization::new("viz2");
            let mut composition = viz1.overlay(viz2);
            composition.render(&mut context).unwrap();
            black_box(());
        });
    });

    // Benchmark side-by-side composition (with viewport splitting)
    group.bench_function("side_by_side", |b| {
        b.iter(|| {
            let mut context = RenderContext::new().block_on().unwrap();
            let viz1 = BenchVisualization::new("viz1");
            let viz2 = BenchVisualization::new("viz2");
            let mut composition = viz1.beside(viz2);
            composition.render(&mut context).unwrap();
            black_box(());
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_viewport_caching,
    bench_nested_composition_depth,
    bench_pipeline_cache,
    bench_state_batching,
    bench_composition_overhead
);
criterion_main!(benches);

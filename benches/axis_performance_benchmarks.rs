// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Performance benchmarks for the axis rendering system (GUP-094).
//!
//! Measures:
//! * Vertex generation for axis lines and ticks
//! * LOD selection overhead
//! * Geometry cache hit vs miss paths
//! * Grid line generation and caching
//! * Label data generation
//! * Resource pool acquire/release cycle

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use gup::axis::{AxisBounds, AxisConfiguration, AxisPosition, AxisRenderer};
use gup::axis_performance::{AxisLODManager, AxisResourcePool, ViewportBounds};
use gup::grid::{ChartBounds, GridConfiguration, GridRenderer};
use gup::shader_function::Vec2;
use std::hint::black_box;
use std::time::Duration;

/// Benchmark vertex generation for a typical bottom axis.
fn bench_axis_vertex_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("axis_vertex_generation");

    let renderer = AxisRenderer::new();
    let bounds = AxisBounds::new(Vec2 { x: -0.8, y: -0.8 }, Vec2 { x: 0.8, y: -0.8 }, 50.0);
    let viewport = (800.0, 600.0);

    // Line only
    let config_line = AxisConfiguration::default().without_ticks();
    group.bench_function("line_only", |b| {
        b.iter(|| {
            black_box(renderer.generate_axis_vertices(
                &bounds,
                &config_line,
                AxisPosition::Bottom,
                None,
                viewport,
            ))
        })
    });

    // Line + major ticks (default)
    let config_default = AxisConfiguration::default();
    group.bench_function("line_and_major_ticks", |b| {
        b.iter(|| {
            black_box(renderer.generate_axis_vertices(
                &bounds,
                &config_default,
                AxisPosition::Bottom,
                None,
                viewport,
            ))
        })
    });

    // All features enabled (minor ticks too)
    let config_full = AxisConfiguration::default()
        .with_minor_subdivisions(5)
        .with_tick_count(10);
    let mut config_full_minor = config_full;
    config_full_minor.show_minor_ticks = true;
    group.bench_function("full_with_minor_ticks", |b| {
        b.iter(|| {
            black_box(renderer.generate_axis_vertices(
                &bounds,
                &config_full_minor,
                AxisPosition::Bottom,
                None,
                viewport,
            ))
        })
    });

    group.finish();
}

/// Benchmark cached vs uncached vertex generation.
fn bench_axis_cache_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("axis_cache");

    let bounds = AxisBounds::new(Vec2 { x: -0.8, y: -0.8 }, Vec2 { x: 0.8, y: -0.8 }, 50.0);
    let config = AxisConfiguration::default();
    let viewport = (800.0, 600.0);

    // Cache miss (first call)
    group.bench_function("miss", |b| {
        b.iter(|| {
            let mut renderer = AxisRenderer::new();
            let verts = renderer.generate_axis_vertices_cached(
                &bounds,
                &config,
                AxisPosition::Bottom,
                None,
                viewport,
                None,
            );
            black_box(verts.len())
        })
    });

    // Cache hit (second call with same params)
    group.bench_function("hit", |b| {
        let mut renderer = AxisRenderer::new();
        // Prime the cache
        let _ = renderer.generate_axis_vertices_cached(
            &bounds,
            &config,
            AxisPosition::Bottom,
            None,
            viewport,
            None,
        );

        b.iter(|| {
            let verts = renderer.generate_axis_vertices_cached(
                &bounds,
                &config,
                AxisPosition::Bottom,
                None,
                viewport,
                None,
            );
            black_box(verts.len())
        })
    });

    group.finish();
}

/// Benchmark LOD calculation.
fn bench_lod_selection(c: &mut Criterion) {
    let mut group = c.benchmark_group("lod_selection");

    let lod_manager = AxisLODManager::default();

    group.bench_function("size_based", |b| {
        b.iter(|| black_box(lod_manager.calculate_lod(black_box(500.0), None)))
    });

    group.bench_function("with_render_time", |b| {
        b.iter(|| {
            black_box(lod_manager.calculate_lod(black_box(500.0), Some(Duration::from_micros(500))))
        })
    });

    group.finish();
}

/// Benchmark grid fingerprint computation with varying tick counts.
fn bench_grid_fingerprint(c: &mut Criterion) {
    let mut group = c.benchmark_group("grid_fingerprint");
    let bounds = ChartBounds::new(50.0, 750.0, 50.0, 550.0);
    let config = GridConfiguration::default();

    for tick_count in [5, 10, 20, 50] {
        let h_ticks: Vec<f64> = (0..tick_count)
            .map(|i| 50.0 + (i as f64 / tick_count as f64) * 700.0)
            .collect();
        let v_ticks: Vec<f64> = (0..tick_count)
            .map(|i| 50.0 + (i as f64 / tick_count as f64) * 500.0)
            .collect();

        group.bench_with_input(
            BenchmarkId::new("tick_count", tick_count),
            &tick_count,
            |b, _| {
                b.iter(|| {
                    // Fingerprint computation is the fast path we want to benchmark
                    black_box(GridRenderer::compute_fingerprint_public(
                        &h_ticks,
                        &v_ticks,
                        &[],
                        &[],
                        bounds,
                        &config,
                    ));
                })
            },
        );
    }

    group.finish();
}

/// Benchmark label data generation.
fn bench_label_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("label_generation");

    let renderer = AxisRenderer::new();
    let bounds = AxisBounds::new(Vec2 { x: -0.8, y: -0.8 }, Vec2 { x: 0.8, y: -0.8 }, 50.0);
    let config = AxisConfiguration::default();
    let viewport = (800.0, 600.0);

    group.bench_function("default_6_labels", |b| {
        b.iter(|| {
            black_box(renderer.generate_label_data(
                &bounds,
                &config,
                AxisPosition::Bottom,
                None,
                viewport,
                None,
            ))
        })
    });

    group.finish();
}

/// Benchmark label culling.
fn bench_label_culling(c: &mut Criterion) {
    let mut group = c.benchmark_group("label_culling");

    let viewport = ViewportBounds::from_size(800.0, 600.0);

    for count in [10, 50, 100, 500] {
        let positions: Vec<[f32; 2]> = (0..count)
            .map(|i| {
                let x = (i as f32 / count as f32) * 1000.0 - 100.0;
                let y = 300.0;
                [x, y]
            })
            .collect();

        group.bench_with_input(BenchmarkId::new("labels", count), &count, |b, _| {
            b.iter(|| {
                black_box(gup::axis_performance::cull_label_indices(
                    &positions, &viewport, 10.0,
                ))
            })
        });
    }

    group.finish();
}

/// Benchmark resource pool acquire/release cycle.
fn bench_resource_pool(c: &mut Criterion) {
    let mut group = c.benchmark_group("resource_pool");

    group.bench_function("acquire_release_4", |b| {
        b.iter(|| {
            let mut pool = AxisResourcePool::new(4, 64);
            for _ in 0..4 {
                let buf = pool.acquire(64);
                pool.release(buf);
            }
            black_box(&pool);
        })
    });

    group.bench_function("acquire_release_16", |b| {
        b.iter(|| {
            let mut pool = AxisResourcePool::new(16, 64);
            let mut bufs = Vec::new();
            for _ in 0..16 {
                bufs.push(pool.acquire(64));
            }
            for buf in bufs {
                pool.release(buf);
            }
            black_box(&pool);
        })
    });

    group.finish();
}

/// Benchmark a complete 4-axis system (simulated vertex generation).
fn bench_complete_axis_system(c: &mut Criterion) {
    let mut group = c.benchmark_group("complete_axis_system");

    let viewport = (800.0, 600.0);
    let configs = [
        (
            AxisPosition::Bottom,
            Vec2 { x: -0.8, y: -0.8 },
            Vec2 { x: 0.8, y: -0.8 },
        ),
        (
            AxisPosition::Left,
            Vec2 { x: -0.8, y: -0.8 },
            Vec2 { x: -0.8, y: 0.8 },
        ),
        (
            AxisPosition::Top,
            Vec2 { x: -0.8, y: 0.8 },
            Vec2 { x: 0.8, y: 0.8 },
        ),
        (
            AxisPosition::Right,
            Vec2 { x: 0.8, y: -0.8 },
            Vec2 { x: 0.8, y: 0.8 },
        ),
    ];

    let config = AxisConfiguration::default();

    group.bench_function("4_axes_uncached", |b| {
        b.iter(|| {
            let renderer = AxisRenderer::new();
            let mut total_verts = 0;
            for &(pos, start, end) in &configs {
                let bounds = AxisBounds::new(start, end, 50.0);
                let verts = renderer.generate_axis_vertices(&bounds, &config, pos, None, viewport);
                total_verts += verts.len();
            }
            black_box(total_verts);
        })
    });

    group.bench_function("4_axes_cached", |b| {
        let mut renderers: Vec<AxisRenderer> = (0..4).map(|_| AxisRenderer::new()).collect();
        // Prime caches
        for (i, &(pos, start, end)) in configs.iter().enumerate() {
            let bounds = AxisBounds::new(start, end, 50.0);
            let _ = renderers[i]
                .generate_axis_vertices_cached(&bounds, &config, pos, None, viewport, None);
        }

        b.iter(|| {
            let mut total_verts = 0;
            for (i, &(pos, start, end)) in configs.iter().enumerate() {
                let bounds = AxisBounds::new(start, end, 50.0);
                let verts = renderers[i]
                    .generate_axis_vertices_cached(&bounds, &config, pos, None, viewport, None);
                total_verts += verts.len();
            }
            black_box(total_verts);
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_axis_vertex_generation,
    bench_axis_cache_performance,
    bench_lod_selection,
    bench_grid_fingerprint,
    bench_label_generation,
    bench_label_culling,
    bench_resource_pool,
    bench_complete_axis_system,
);
criterion_main!(benches);

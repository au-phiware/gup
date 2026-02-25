// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Comprehensive performance benchmarks for the grid rendering system (GUP-096).
//!
//! Measures:
//! * Grid line generation for varying line counts (10, 20, 50, 100, 500)
//! * Horizontal vs vertical line generation performance
//! * Configuration impact analysis across grid themes
//! * Cache hit vs cache miss performance
//! * Fingerprint computation overhead
//! * Multi-grid scenarios (major + minor simultaneously)
//! * Memory usage estimation for grid data structures
//! * End-to-end grid system generation pipeline

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use gup::LineAttributes;
use gup::grid::{ChartBounds, GridConfiguration, GridLineConfig, GridRenderer, GridSystem};
use std::hint::black_box;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Generate evenly spaced tick positions within chart bounds.
fn make_ticks(count: usize, min: f64, max: f64) -> Vec<f64> {
    if count == 0 {
        return Vec::new();
    }
    let step = (max - min) / (count as f64 + 1.0);
    (1..=count).map(|i| min + step * i as f64).collect()
}

/// Standard chart bounds used across benchmarks (800×600 chart area).
fn standard_bounds() -> ChartBounds {
    ChartBounds::new(50.0, 750.0, 50.0, 550.0)
}

// ---------------------------------------------------------------------------
// Grid Line Generation Benchmarks
// ---------------------------------------------------------------------------

/// Benchmark horizontal line generation across different line counts.
fn bench_horizontal_line_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("grid_horizontal_generation");
    let bounds = standard_bounds();
    let config = GridLineConfig::default();

    for count in [5, 10, 20, 50, 100, 500] {
        let y_ticks = make_ticks(count, bounds.top as f64, bounds.bottom as f64);

        group.bench_with_input(BenchmarkId::new("lines", count), &count, |b, _| {
            let mut output = Vec::with_capacity(count);
            b.iter(|| {
                output.clear();
                GridRenderer::generate_horizontal_lines_static(
                    black_box(&y_ticks),
                    black_box(bounds),
                    black_box(&config),
                    &mut output,
                )
                .unwrap();
                black_box(output.len())
            });
        });
    }

    group.finish();
}

/// Benchmark vertical line generation across different line counts.
fn bench_vertical_line_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("grid_vertical_generation");
    let bounds = standard_bounds();
    let config = GridLineConfig::default();

    for count in [5, 10, 20, 50, 100, 500] {
        let x_ticks = make_ticks(count, bounds.left as f64, bounds.right as f64);

        group.bench_with_input(BenchmarkId::new("lines", count), &count, |b, _| {
            let mut output = Vec::with_capacity(count);
            b.iter(|| {
                output.clear();
                GridRenderer::generate_vertical_lines_static(
                    black_box(&x_ticks),
                    black_box(bounds),
                    black_box(&config),
                    &mut output,
                )
                .unwrap();
                black_box(output.len())
            });
        });
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// End-to-End Grid Generation (no GPU)
// ---------------------------------------------------------------------------

/// Benchmark the full grid generation pipeline (major only) at various scales.
fn bench_full_grid_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("grid_full_generation");
    let bounds = standard_bounds();
    let config = GridConfiguration::default(); // major only

    for count in [10, 20, 50, 100, 500] {
        let h_ticks = make_ticks(count / 2, bounds.left as f64, bounds.right as f64);
        let v_ticks = make_ticks(count / 2, bounds.top as f64, bounds.bottom as f64);

        group.bench_with_input(BenchmarkId::new("total_lines", count), &count, |b, _| {
            let mut renderer = GridRenderer::new();
            b.iter(|| {
                renderer.invalidate_cache();
                let n = renderer
                    .generate_grid_lines(
                        black_box(&h_ticks),
                        black_box(&v_ticks),
                        &[],
                        &[],
                        black_box(bounds),
                        black_box(&config),
                    )
                    .unwrap();
                black_box(n)
            });
        });
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Multi-Grid Scenarios (Major + Minor)
// ---------------------------------------------------------------------------

/// Benchmark generation with both major and minor grid lines enabled.
fn bench_multi_grid_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("grid_multi_grid");
    let bounds = standard_bounds();
    let config = GridConfiguration::default().with_minor_grid();

    for major_count in [10, 20, 50] {
        // Minor grids: 4× the number of major ticks (typical subdivision)
        let minor_count = major_count * 4;

        let h_major = make_ticks(major_count / 2, bounds.left as f64, bounds.right as f64);
        let v_major = make_ticks(major_count / 2, bounds.top as f64, bounds.bottom as f64);
        let h_minor = make_ticks(minor_count / 2, bounds.left as f64, bounds.right as f64);
        let v_minor = make_ticks(minor_count / 2, bounds.top as f64, bounds.bottom as f64);

        let label = format!("major_{}_minor_{}", major_count, minor_count);

        group.bench_function(&label, |b| {
            let mut renderer = GridRenderer::new();
            b.iter(|| {
                renderer.invalidate_cache();
                let n = renderer
                    .generate_grid_lines(
                        black_box(&h_major),
                        black_box(&v_major),
                        black_box(&h_minor),
                        black_box(&v_minor),
                        black_box(bounds),
                        black_box(&config),
                    )
                    .unwrap();
                black_box(n)
            });
        });
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Configuration Impact Analysis
// ---------------------------------------------------------------------------

/// Benchmark how different grid themes affect generation performance.
fn bench_configuration_impact(c: &mut Criterion) {
    let mut group = c.benchmark_group("grid_config_impact");
    let bounds = standard_bounds();

    let h_ticks = make_ticks(10, bounds.left as f64, bounds.right as f64);
    let v_ticks = make_ticks(10, bounds.top as f64, bounds.bottom as f64);
    let h_minor = make_ticks(40, bounds.left as f64, bounds.right as f64);
    let v_minor = make_ticks(40, bounds.top as f64, bounds.bottom as f64);

    let themes: Vec<(&str, GridConfiguration)> = vec![
        ("default", GridConfiguration::default()),
        ("light_theme", GridConfiguration::light_theme()),
        ("dark_theme", GridConfiguration::dark_theme()),
        ("scientific", GridConfiguration::scientific()),
        ("business", GridConfiguration::business()),
        ("minimal", GridConfiguration::minimal()),
        ("high_contrast", GridConfiguration::high_contrast()),
        ("horizontal_only", GridConfiguration::horizontal_only()),
        ("vertical_only", GridConfiguration::vertical_only()),
    ];

    for (name, config) in &themes {
        group.bench_function(*name, |b| {
            let mut renderer = GridRenderer::new();
            b.iter(|| {
                renderer.invalidate_cache();
                let n = renderer
                    .generate_grid_lines(
                        black_box(&h_ticks),
                        black_box(&v_ticks),
                        black_box(&h_minor),
                        black_box(&v_minor),
                        black_box(bounds),
                        black_box(config),
                    )
                    .unwrap();
                black_box(n)
            });
        });
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Cache Performance
// ---------------------------------------------------------------------------

/// Benchmark cache hit vs cache miss performance.
fn bench_cache_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("grid_cache");
    let bounds = standard_bounds();
    let config = GridConfiguration::default();

    let h_ticks = make_ticks(10, bounds.left as f64, bounds.right as f64);
    let v_ticks = make_ticks(10, bounds.top as f64, bounds.bottom as f64);

    // Cache miss: invalidate before each call
    group.bench_function("miss_20_lines", |b| {
        let mut renderer = GridRenderer::new();
        b.iter(|| {
            renderer.invalidate_cache();
            let n = renderer
                .generate_grid_lines(
                    black_box(&h_ticks),
                    black_box(&v_ticks),
                    &[],
                    &[],
                    black_box(bounds),
                    black_box(&config),
                )
                .unwrap();
            black_box(n)
        });
    });

    // Cache hit: prime once, then repeated identical calls
    group.bench_function("hit_20_lines", |b| {
        let mut renderer = GridRenderer::new();
        // Prime the cache
        renderer
            .generate_grid_lines(&h_ticks, &v_ticks, &[], &[], bounds, &config)
            .unwrap();

        b.iter(|| {
            let n = renderer
                .generate_grid_lines(
                    black_box(&h_ticks),
                    black_box(&v_ticks),
                    &[],
                    &[],
                    black_box(bounds),
                    black_box(&config),
                )
                .unwrap();
            black_box(n)
        });
    });

    // Cache miss with large grid (100 lines)
    let h_large = make_ticks(50, bounds.left as f64, bounds.right as f64);
    let v_large = make_ticks(50, bounds.top as f64, bounds.bottom as f64);

    group.bench_function("miss_100_lines", |b| {
        let mut renderer = GridRenderer::new();
        b.iter(|| {
            renderer.invalidate_cache();
            let n = renderer
                .generate_grid_lines(
                    black_box(&h_large),
                    black_box(&v_large),
                    &[],
                    &[],
                    black_box(bounds),
                    black_box(&config),
                )
                .unwrap();
            black_box(n)
        });
    });

    group.bench_function("hit_100_lines", |b| {
        let mut renderer = GridRenderer::new();
        renderer
            .generate_grid_lines(&h_large, &v_large, &[], &[], bounds, &config)
            .unwrap();

        b.iter(|| {
            let n = renderer
                .generate_grid_lines(
                    black_box(&h_large),
                    black_box(&v_large),
                    &[],
                    &[],
                    black_box(bounds),
                    black_box(&config),
                )
                .unwrap();
            black_box(n)
        });
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Fingerprint Computation
// ---------------------------------------------------------------------------

/// Benchmark fingerprint computation at different scales.
fn bench_fingerprint_computation(c: &mut Criterion) {
    let mut group = c.benchmark_group("grid_fingerprint");
    let bounds = standard_bounds();
    let config = GridConfiguration::default();

    for tick_count in [5, 10, 20, 50, 100, 500] {
        let h_ticks = make_ticks(tick_count, bounds.left as f64, bounds.right as f64);
        let v_ticks = make_ticks(tick_count, bounds.top as f64, bounds.bottom as f64);

        group.bench_with_input(
            BenchmarkId::new("ticks", tick_count),
            &tick_count,
            |b, _| {
                b.iter(|| {
                    black_box(GridRenderer::compute_fingerprint_public(
                        black_box(&h_ticks),
                        black_box(&v_ticks),
                        &[],
                        &[],
                        black_box(bounds),
                        black_box(&config),
                    ))
                });
            },
        );
    }

    // With minor ticks
    let h_major = make_ticks(10, bounds.left as f64, bounds.right as f64);
    let v_major = make_ticks(10, bounds.top as f64, bounds.bottom as f64);
    let h_minor = make_ticks(40, bounds.left as f64, bounds.right as f64);
    let v_minor = make_ticks(40, bounds.top as f64, bounds.bottom as f64);

    group.bench_function("with_minor_100_ticks", |b| {
        b.iter(|| {
            black_box(GridRenderer::compute_fingerprint_public(
                black_box(&h_major),
                black_box(&v_major),
                black_box(&h_minor),
                black_box(&v_minor),
                black_box(bounds),
                black_box(&GridConfiguration::default().with_minor_grid()),
            ))
        });
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Memory Usage Estimation
// ---------------------------------------------------------------------------

/// Benchmark and estimate memory usage for grid data structures.
fn bench_memory_usage(c: &mut Criterion) {
    let mut group = c.benchmark_group("grid_memory");
    let bounds = standard_bounds();
    let config = GridConfiguration::default().with_minor_grid();

    let line_size = std::mem::size_of::<LineAttributes>();

    for total in [20, 50, 100, 200, 500] {
        let major_count = total / 5;
        let minor_count = total - major_count;

        let h_major = make_ticks(major_count / 2, bounds.left as f64, bounds.right as f64);
        let v_major = make_ticks(major_count / 2, bounds.top as f64, bounds.bottom as f64);
        let h_minor = make_ticks(minor_count / 2, bounds.left as f64, bounds.right as f64);
        let v_minor = make_ticks(minor_count / 2, bounds.top as f64, bounds.bottom as f64);

        group.bench_with_input(BenchmarkId::new("lines", total), &total, |b, _| {
            let mut renderer = GridRenderer::new();
            b.iter(|| {
                renderer.invalidate_cache();
                let n = renderer
                    .generate_grid_lines(
                        black_box(&h_major),
                        black_box(&v_major),
                        black_box(&h_minor),
                        black_box(&v_minor),
                        black_box(bounds),
                        black_box(&config),
                    )
                    .unwrap();
                // Report estimated memory: count × size_of::<LineAttributes>
                black_box(n * line_size)
            });
        });
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// GridSystem End-to-End
// ---------------------------------------------------------------------------

/// Benchmark the GridSystem coordinator end-to-end.
fn bench_grid_system_e2e(c: &mut Criterion) {
    let mut group = c.benchmark_group("grid_system");
    let bounds = standard_bounds();

    // Typical chart scenario: 10 major ticks per axis
    let h_ticks = make_ticks(10, bounds.left as f64, bounds.right as f64);
    let v_ticks = make_ticks(10, bounds.top as f64, bounds.bottom as f64);

    group.bench_function("create_and_generate_20", |b| {
        b.iter(|| {
            let config = GridConfiguration::default();
            let mut system = GridSystem::new(config);
            let n = system
                .renderer
                .generate_grid_lines(
                    black_box(&h_ticks),
                    black_box(&v_ticks),
                    &[],
                    &[],
                    black_box(bounds),
                    black_box(&system.config),
                )
                .unwrap();
            black_box(n)
        });
    });

    // Config update + regeneration (simulates runtime config change)
    group.bench_function("config_change_regenerate", |b| {
        let mut system = GridSystem::new(GridConfiguration::default());
        system
            .renderer
            .generate_grid_lines(&h_ticks, &v_ticks, &[], &[], bounds, &system.config)
            .unwrap();

        b.iter(|| {
            system.set_configuration(GridConfiguration::scientific());
            system.renderer.invalidate_cache();
            let n = system
                .renderer
                .generate_grid_lines(
                    black_box(&h_ticks),
                    black_box(&v_ticks),
                    &[],
                    &[],
                    black_box(bounds),
                    black_box(&system.config),
                )
                .unwrap();
            black_box(n)
        });
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Scalability: Stress Test
// ---------------------------------------------------------------------------

/// Benchmark scalability with very large grid line counts.
fn bench_scalability(c: &mut Criterion) {
    let mut group = c.benchmark_group("grid_scalability");
    group.sample_size(50); // Reduce samples for large inputs

    let bounds = standard_bounds();
    let config = GridConfiguration::default();

    for count in [100, 500, 1000, 5000] {
        let h_ticks = make_ticks(count / 2, bounds.left as f64, bounds.right as f64);
        let v_ticks = make_ticks(count / 2, bounds.top as f64, bounds.bottom as f64);

        group.bench_with_input(BenchmarkId::new("total_lines", count), &count, |b, _| {
            let mut renderer = GridRenderer::new();
            b.iter(|| {
                renderer.invalidate_cache();
                let n = renderer
                    .generate_grid_lines(
                        black_box(&h_ticks),
                        black_box(&v_ticks),
                        &[],
                        &[],
                        black_box(bounds),
                        black_box(&config),
                    )
                    .unwrap();
                black_box(n)
            });
        });
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Criterion Groups
// ---------------------------------------------------------------------------

criterion_group!(
    benches,
    bench_horizontal_line_generation,
    bench_vertical_line_generation,
    bench_full_grid_generation,
    bench_multi_grid_generation,
    bench_configuration_impact,
    bench_cache_performance,
    bench_fingerprint_computation,
    bench_memory_usage,
    bench_grid_system_e2e,
    bench_scalability,
);
criterion_main!(benches);

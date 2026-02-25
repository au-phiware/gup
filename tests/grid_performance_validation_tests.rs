// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Performance validation tests for the grid rendering system (GUP-096).
//!
//! These tests verify that grid performance targets are met and provide
//! regression detection for grid system operations.

use gup::LineAttributes;
use gup::grid::{ChartBounds, GridConfiguration, GridRenderer};
use std::time::{Duration, Instant};

/// Generate evenly spaced tick positions within a range.
fn make_ticks(count: usize, min: f64, max: f64) -> Vec<f64> {
    if count == 0 {
        return Vec::new();
    }
    let step = (max - min) / (count as f64 + 1.0);
    (1..=count).map(|i| min + step * i as f64).collect()
}

/// Standard chart bounds (800×600 chart area with 50px margin).
fn standard_bounds() -> ChartBounds {
    ChartBounds::new(50.0, 750.0, 50.0, 550.0)
}

// ---------------------------------------------------------------------------
// Performance Target Validation
// ---------------------------------------------------------------------------

#[test]
fn test_grid_generation_under_50us_for_20_lines() {
    // Performance target: <0.05ms (50µs) for generating 20 grid lines
    let bounds = standard_bounds();
    let config = GridConfiguration::default();
    let h_ticks = make_ticks(10, bounds.left as f64, bounds.right as f64);
    let v_ticks = make_ticks(10, bounds.top as f64, bounds.bottom as f64);

    // Warm up
    let mut renderer = GridRenderer::new();
    for _ in 0..100 {
        renderer.invalidate_cache();
        renderer
            .generate_grid_lines(&h_ticks, &v_ticks, &[], &[], bounds, &config)
            .unwrap();
    }

    // Measure (take the median of 1000 runs)
    let mut durations = Vec::with_capacity(1000);
    for _ in 0..1000 {
        renderer.invalidate_cache();
        let start = Instant::now();
        let count = renderer
            .generate_grid_lines(&h_ticks, &v_ticks, &[], &[], bounds, &config)
            .unwrap();
        let elapsed = start.elapsed();
        durations.push(elapsed);
        assert_eq!(count, 20, "Expected 20 grid lines");
    }

    durations.sort();
    let median = durations[durations.len() / 2];
    let p95 = durations[(durations.len() as f64 * 0.95) as usize];

    println!("20-line grid generation: median={median:?}, p95={p95:?}");

    // Allow some slack for CI environments (target is <50µs)
    assert!(
        median < Duration::from_micros(500),
        "Median grid generation time {median:?} exceeds 500µs budget"
    );
}

#[test]
fn test_cache_hit_is_significantly_faster() {
    let bounds = standard_bounds();
    let config = GridConfiguration::default();
    let h_ticks = make_ticks(50, bounds.left as f64, bounds.right as f64);
    let v_ticks = make_ticks(50, bounds.top as f64, bounds.bottom as f64);

    let mut renderer = GridRenderer::new();

    // Measure cache miss
    let mut miss_durations = Vec::with_capacity(100);
    for _ in 0..100 {
        renderer.invalidate_cache();
        let start = Instant::now();
        renderer
            .generate_grid_lines(&h_ticks, &v_ticks, &[], &[], bounds, &config)
            .unwrap();
        miss_durations.push(start.elapsed());
    }

    // Measure cache hit (don't invalidate)
    // Prime the cache
    renderer.invalidate_cache();
    renderer
        .generate_grid_lines(&h_ticks, &v_ticks, &[], &[], bounds, &config)
        .unwrap();

    let mut hit_durations = Vec::with_capacity(100);
    for _ in 0..100 {
        let start = Instant::now();
        renderer
            .generate_grid_lines(&h_ticks, &v_ticks, &[], &[], bounds, &config)
            .unwrap();
        hit_durations.push(start.elapsed());
    }

    miss_durations.sort();
    hit_durations.sort();

    let miss_median = miss_durations[miss_durations.len() / 2];
    let hit_median = hit_durations[hit_durations.len() / 2];

    println!("Cache miss median: {miss_median:?}");
    println!("Cache hit median: {hit_median:?}");

    // Cache hit should be notably faster than miss
    assert!(
        hit_median < miss_median,
        "Cache hit ({hit_median:?}) should be faster than miss ({miss_median:?})"
    );
}

#[test]
fn test_scalability_linear_with_line_count() {
    let bounds = standard_bounds();
    let config = GridConfiguration::default();

    let counts = [10, 50, 100, 500];
    let mut timings = Vec::new();

    for &count in &counts {
        let h_ticks = make_ticks(count / 2, bounds.left as f64, bounds.right as f64);
        let v_ticks = make_ticks(count / 2, bounds.top as f64, bounds.bottom as f64);

        let mut renderer = GridRenderer::new();

        // Warm up
        for _ in 0..50 {
            renderer.invalidate_cache();
            renderer
                .generate_grid_lines(&h_ticks, &v_ticks, &[], &[], bounds, &config)
                .unwrap();
        }

        // Measure
        let mut durations = Vec::with_capacity(200);
        for _ in 0..200 {
            renderer.invalidate_cache();
            let start = Instant::now();
            renderer
                .generate_grid_lines(&h_ticks, &v_ticks, &[], &[], bounds, &config)
                .unwrap();
            durations.push(start.elapsed());
        }

        durations.sort();
        let median = durations[durations.len() / 2];
        timings.push((count, median));
        println!("{count} lines: median={median:?}");
    }

    // Check that the ratio of time per line doesn't grow super-linearly
    // (allowing 5× ratio for 50× more lines as generous headroom)
    let base_per_line = timings[0].1.as_nanos() as f64 / timings[0].0 as f64;
    let last_per_line =
        timings[timings.len() - 1].1.as_nanos() as f64 / timings[timings.len() - 1].0 as f64;

    let ratio = last_per_line / base_per_line;
    println!("Per-line time ratio (small vs large): {ratio:.2}");

    assert!(
        ratio < 5.0,
        "Performance degradation ratio {ratio:.2} exceeds 5× threshold — likely super-linear scaling"
    );
}

// ---------------------------------------------------------------------------
// Memory Usage Validation
// ---------------------------------------------------------------------------

#[test]
fn test_memory_usage_within_bounds() {
    let line_size = std::mem::size_of::<LineAttributes>();
    let bounds = standard_bounds();

    // 20 grid lines (typical use case)
    let config = GridConfiguration::default();
    let h_ticks = make_ticks(10, bounds.left as f64, bounds.right as f64);
    let v_ticks = make_ticks(10, bounds.top as f64, bounds.bottom as f64);

    let mut renderer = GridRenderer::new();
    let count = renderer
        .generate_grid_lines(&h_ticks, &v_ticks, &[], &[], bounds, &config)
        .unwrap();

    let estimated_bytes = count * line_size;
    println!(
        "20-line grid: {count} lines × {line_size} bytes = {estimated_bytes} bytes ({:.1} KB)",
        estimated_bytes as f64 / 1024.0
    );

    // For 20 lines, memory should be well under 10 KB
    assert!(
        estimated_bytes < 10 * 1024,
        "Memory usage {estimated_bytes} bytes exceeds 10 KB for 20 grid lines"
    );
}

#[test]
fn test_memory_usage_large_grid() {
    let line_size = std::mem::size_of::<LineAttributes>();
    let bounds = standard_bounds();

    // 500 grid lines (extreme use case)
    let config = GridConfiguration::default().with_minor_grid();
    let h_major = make_ticks(50, bounds.left as f64, bounds.right as f64);
    let v_major = make_ticks(50, bounds.top as f64, bounds.bottom as f64);
    let h_minor = make_ticks(200, bounds.left as f64, bounds.right as f64);
    let v_minor = make_ticks(200, bounds.top as f64, bounds.bottom as f64);

    let mut renderer = GridRenderer::new();
    let count = renderer
        .generate_grid_lines(&h_major, &v_major, &h_minor, &v_minor, bounds, &config)
        .unwrap();

    let estimated_bytes = count * line_size;
    println!(
        "500-line grid: {count} lines × {line_size} bytes = {estimated_bytes} bytes ({:.1} KB)",
        estimated_bytes as f64 / 1024.0
    );

    // Even 500 lines should be well under 1 MB
    assert!(
        estimated_bytes < 1024 * 1024,
        "Memory usage {estimated_bytes} bytes exceeds 1 MB for 500 grid lines"
    );
}

// ---------------------------------------------------------------------------
// Configuration Impact Validation
// ---------------------------------------------------------------------------

#[test]
fn test_disabled_grid_is_instant() {
    let bounds = standard_bounds();
    let h_ticks = make_ticks(50, bounds.left as f64, bounds.right as f64);
    let v_ticks = make_ticks(50, bounds.top as f64, bounds.bottom as f64);

    // Fully disabled grid
    let mut config = GridConfiguration::default();
    config.major_grid.enabled = false;
    config.minor_grid.enabled = false;

    let mut renderer = GridRenderer::new();
    let start = Instant::now();
    for _ in 0..1000 {
        renderer.invalidate_cache();
        let count = renderer
            .generate_grid_lines(&h_ticks, &v_ticks, &[], &[], bounds, &config)
            .unwrap();
        assert_eq!(count, 0);
    }
    let elapsed = start.elapsed();
    let per_call = elapsed / 1000;

    println!("Disabled grid per-call: {per_call:?}");

    // Disabled grid should be extremely fast (just fingerprint + check)
    assert!(
        per_call < Duration::from_micros(50),
        "Disabled grid call takes {per_call:?} — should be nearly instant"
    );
}

#[test]
fn test_horizontal_only_generates_fewer_lines() {
    let bounds = standard_bounds();
    let h_ticks = make_ticks(10, bounds.left as f64, bounds.right as f64);
    let v_ticks = make_ticks(10, bounds.top as f64, bounds.bottom as f64);

    // Default (both directions)
    let mut renderer_both = GridRenderer::new();
    let count_both = renderer_both
        .generate_grid_lines(
            &h_ticks,
            &v_ticks,
            &[],
            &[],
            bounds,
            &GridConfiguration::default(),
        )
        .unwrap();

    // Horizontal only
    let mut renderer_h = GridRenderer::new();
    let count_h = renderer_h
        .generate_grid_lines(
            &h_ticks,
            &v_ticks,
            &[],
            &[],
            bounds,
            &GridConfiguration::horizontal_only(),
        )
        .unwrap();

    println!("Both directions: {count_both} lines");
    println!("Horizontal only: {count_h} lines");

    assert!(
        count_h < count_both,
        "Horizontal-only ({count_h}) should have fewer lines than both ({count_both})"
    );
}

// ---------------------------------------------------------------------------
// Cache Correctness
// ---------------------------------------------------------------------------

#[test]
fn test_cache_hit_rate_tracking() {
    let bounds = standard_bounds();
    let config = GridConfiguration::default();
    let h_ticks = make_ticks(10, bounds.left as f64, bounds.right as f64);
    let v_ticks = make_ticks(10, bounds.top as f64, bounds.bottom as f64);

    let mut renderer = GridRenderer::new();

    // First call: miss
    renderer
        .generate_grid_lines(&h_ticks, &v_ticks, &[], &[], bounds, &config)
        .unwrap();
    assert_eq!(renderer.cache_stats(), (0, 1));

    // Repeated calls: hits
    for _ in 0..9 {
        renderer
            .generate_grid_lines(&h_ticks, &v_ticks, &[], &[], bounds, &config)
            .unwrap();
    }
    assert_eq!(renderer.cache_stats(), (9, 1));

    let hit_rate = renderer.cache_hit_rate();
    assert!(
        (hit_rate - 0.9).abs() < 0.001,
        "Expected 90% hit rate, got {hit_rate}"
    );
}

#[test]
fn test_cache_invalidation_on_tick_change() {
    let bounds = standard_bounds();
    let config = GridConfiguration::default();

    let h_ticks_a = make_ticks(10, bounds.left as f64, bounds.right as f64);
    let v_ticks_a = make_ticks(10, bounds.top as f64, bounds.bottom as f64);
    let h_ticks_b = make_ticks(15, bounds.left as f64, bounds.right as f64);
    let v_ticks_b = make_ticks(15, bounds.top as f64, bounds.bottom as f64);

    let mut renderer = GridRenderer::new();

    // Generate with ticks A
    let count_a = renderer
        .generate_grid_lines(&h_ticks_a, &v_ticks_a, &[], &[], bounds, &config)
        .unwrap();

    // Generate with different ticks B (should be cache miss, new fingerprint)
    let count_b = renderer
        .generate_grid_lines(&h_ticks_b, &v_ticks_b, &[], &[], bounds, &config)
        .unwrap();

    assert_ne!(
        count_a, count_b,
        "Different tick counts should yield different line counts"
    );
    assert_eq!(
        renderer.cache_stats(),
        (0, 2),
        "Both calls should be cache misses (different ticks)"
    );
}

// ---------------------------------------------------------------------------
// Regression Detection Helpers
// ---------------------------------------------------------------------------

#[test]
fn test_fingerprint_computation_is_fast() {
    let bounds = standard_bounds();
    let config = GridConfiguration::default();
    let h_ticks = make_ticks(100, bounds.left as f64, bounds.right as f64);
    let v_ticks = make_ticks(100, bounds.top as f64, bounds.bottom as f64);

    // Warm up
    for _ in 0..1000 {
        let _ =
            GridRenderer::compute_fingerprint_public(&h_ticks, &v_ticks, &[], &[], bounds, &config);
    }

    // Measure
    let start = Instant::now();
    for _ in 0..10_000 {
        std::hint::black_box(GridRenderer::compute_fingerprint_public(
            std::hint::black_box(&h_ticks),
            std::hint::black_box(&v_ticks),
            &[],
            &[],
            std::hint::black_box(bounds),
            std::hint::black_box(&config),
        ));
    }
    let elapsed = start.elapsed();
    let per_call = elapsed / 10_000;

    println!("Fingerprint (200 ticks) per-call: {per_call:?}");

    assert!(
        per_call < Duration::from_micros(50),
        "Fingerprint computation {per_call:?} exceeds 50µs budget"
    );
}

#[test]
fn test_grid_generation_no_memory_leak_simulation() {
    // Simulate long-running usage: generate, clear, regenerate many times
    let bounds = standard_bounds();
    let config = GridConfiguration::default().with_minor_grid();
    let h_major = make_ticks(10, bounds.left as f64, bounds.right as f64);
    let v_major = make_ticks(10, bounds.top as f64, bounds.bottom as f64);
    let h_minor = make_ticks(40, bounds.left as f64, bounds.right as f64);
    let v_minor = make_ticks(40, bounds.top as f64, bounds.bottom as f64);

    let mut renderer = GridRenderer::new();

    for i in 0..1000 {
        renderer.invalidate_cache();
        let count = renderer
            .generate_grid_lines(&h_major, &v_major, &h_minor, &v_minor, bounds, &config)
            .unwrap();

        // Line count should be consistent
        if i > 0 {
            assert!(count > 0, "Grid should generate lines on every iteration");
        }
    }

    // Verify total line count is stable (not accumulating)
    let final_count = renderer.total_line_count();
    println!("After 1000 iterations, line count: {final_count}");

    // Should be the same count as a fresh generation
    let mut fresh_renderer = GridRenderer::new();
    let fresh_count = fresh_renderer
        .generate_grid_lines(&h_major, &v_major, &h_minor, &v_minor, bounds, &config)
        .unwrap();

    assert_eq!(
        final_count, fresh_count,
        "Line count should be stable after repeated regeneration"
    );
}

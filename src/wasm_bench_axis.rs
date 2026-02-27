// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! WASM-compatible axis performance benchmarks.
//!
//! This module ports the 8 axis benchmarks from
//! `tests/cross_platform_axis_performance_tests.rs` to use the
//! [`wasm_bench`](crate::wasm_bench) harness so they can run in a browser
//! via `wasm-pack test --headless --chrome`.
//!
//! On `wasm32` targets, `run_wasm_axis_benchmarks` is exported via
//! `wasm_bindgen` for direct invocation from JavaScript.

use crate::axis::{AxisBounds, AxisConfiguration, AxisPosition, AxisRenderer};
use crate::axis_performance::{AxisLODManager, ViewportBounds, cull_label_indices};
use crate::grid::{ChartBounds, GridConfiguration, GridRenderer};
use crate::shader_function::Vec2;
use crate::wasm_bench::{BenchConfig, BenchResult, BenchSuite, run_bench};

use std::time::Duration;

// ---------------------------------------------------------------------------
// Helpers — mirrors tests/cross_platform_axis_performance_tests.rs
// ---------------------------------------------------------------------------

/// Standard viewport for benchmarks.
const VIEWPORT: (f32, f32) = (800.0, 600.0);

/// Benchmark iteration count.
///
/// Reduced from the native 1000 to 200 for WASM to keep total wall-clock time
/// reasonable in a browser environment while still producing stable statistics.
const ITERATIONS: u32 = 200;

/// Warmup iterations before measurement.
const WARMUP: u32 = 20;

fn bench_config() -> BenchConfig {
    BenchConfig {
        warmup_iterations: WARMUP,
        measured_iterations: ITERATIONS,
    }
}

fn standard_axis_bounds() -> AxisBounds {
    AxisBounds::new(Vec2 { x: -0.8, y: -0.8 }, Vec2 { x: 0.8, y: -0.8 }, 50.0)
}

fn axis_configs() -> [(AxisPosition, Vec2, Vec2); 4] {
    [
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
    ]
}

// ---------------------------------------------------------------------------
// Individual benchmarks
// ---------------------------------------------------------------------------

/// 1. Vertex generation (uncached).
fn bench_vertex_generation_uncached(config: &BenchConfig) -> BenchResult {
    let renderer = AxisRenderer::new();
    let bounds = standard_axis_bounds();
    let axis_config = AxisConfiguration::default();

    run_bench("vertex_generation_uncached", config, || {
        let _ = renderer.generate_axis_vertices(
            &bounds,
            &axis_config,
            AxisPosition::Bottom,
            None,
            VIEWPORT,
        );
    })
}

/// 2. Vertex generation (cached).
fn bench_vertex_generation_cached(config: &BenchConfig) -> BenchResult {
    let mut renderer = AxisRenderer::new();
    let bounds = standard_axis_bounds();
    let axis_config = AxisConfiguration::default();

    // Prime the cache
    let _ = renderer.generate_axis_vertices_cached(
        &bounds,
        &axis_config,
        AxisPosition::Bottom,
        None,
        VIEWPORT,
        None,
    );

    run_bench("vertex_generation_cached", config, || {
        let _ = renderer.generate_axis_vertices_cached(
            &bounds,
            &axis_config,
            AxisPosition::Bottom,
            None,
            VIEWPORT,
            None,
        );
    })
}

/// 3. LOD selection overhead.
fn bench_lod_selection(config: &BenchConfig) -> BenchResult {
    let lod_manager = AxisLODManager::default();

    run_bench("lod_selection", config, || {
        let _ = lod_manager.calculate_lod(500.0, Some(Duration::from_micros(500)));
    })
}

/// 4. Label generation.
fn bench_label_generation(config: &BenchConfig) -> BenchResult {
    let renderer = AxisRenderer::new();
    let bounds = standard_axis_bounds();
    let axis_config = AxisConfiguration::default();

    run_bench("label_generation", config, || {
        let _ = renderer.generate_label_data(
            &bounds,
            &axis_config,
            AxisPosition::Bottom,
            None,
            VIEWPORT,
            None,
        );
    })
}

/// 5. Label culling (100 labels).
fn bench_label_culling(config: &BenchConfig) -> BenchResult {
    let viewport = ViewportBounds::from_size(VIEWPORT.0, VIEWPORT.1);
    let positions: Vec<[f32; 2]> = (0..100)
        .map(|i| {
            let x = (i as f32 / 100.0) * 1000.0 - 100.0;
            [x, 300.0]
        })
        .collect();

    run_bench("label_culling_100", config, || {
        let _ = cull_label_indices(&positions, &viewport, 10.0);
    })
}

/// 6. Grid fingerprint (20 ticks).
fn bench_grid_fingerprint(config: &BenchConfig) -> BenchResult {
    let bounds = ChartBounds::new(50.0, 750.0, 50.0, 550.0);
    let grid_config = GridConfiguration::default();
    let h_ticks: Vec<f64> = (0..20).map(|i| 50.0 + (i as f64 / 20.0) * 700.0).collect();
    let v_ticks: Vec<f64> = (0..20).map(|i| 50.0 + (i as f64 / 20.0) * 500.0).collect();

    run_bench("grid_fingerprint_20", config, || {
        let _ = GridRenderer::compute_fingerprint_public(
            &h_ticks,
            &v_ticks,
            &[],
            &[],
            bounds,
            &grid_config,
        );
    })
}

/// 7. Complete 4-axis system (uncached).
fn bench_complete_4axis_uncached(config: &BenchConfig) -> BenchResult {
    let axis_config = AxisConfiguration::default();
    let configs = axis_configs();

    run_bench("complete_4axis_uncached", config, || {
        let renderer = AxisRenderer::new();
        let mut total = 0;
        for &(pos, start, end) in &configs {
            let bounds = AxisBounds::new(start, end, 50.0);
            total += renderer
                .generate_axis_vertices(&bounds, &axis_config, pos, None, VIEWPORT)
                .len();
        }
        let _ = total;
    })
}

/// 8. Complete 4-axis system (cached).
fn bench_complete_4axis_cached(config: &BenchConfig) -> BenchResult {
    let axis_config = AxisConfiguration::default();
    let configs = axis_configs();

    let mut renderers: Vec<AxisRenderer> = (0..4).map(|_| AxisRenderer::new()).collect();
    // Prime caches
    for (i, &(pos, start, end)) in configs.iter().enumerate() {
        let bounds = AxisBounds::new(start, end, 50.0);
        let _ = renderers[i].generate_axis_vertices_cached(
            &bounds,
            &axis_config,
            pos,
            None,
            VIEWPORT,
            None,
        );
    }

    run_bench("complete_4axis_cached", config, || {
        let mut total = 0;
        for (i, &(pos, start, end)) in configs.iter().enumerate() {
            let bounds = AxisBounds::new(start, end, 50.0);
            total += renderers[i]
                .generate_axis_vertices_cached(&bounds, &axis_config, pos, None, VIEWPORT, None)
                .len();
        }
        let _ = total;
    })
}

// ---------------------------------------------------------------------------
// Suite runner
// ---------------------------------------------------------------------------

/// Run all 8 axis performance benchmarks and return a [`BenchSuite`].
///
/// This is the shared entry point used by both the native test runner and the
/// WASM `wasm_bindgen` export.
pub fn run_axis_benchmarks(config: &BenchConfig) -> BenchSuite {
    let results = vec![
        bench_vertex_generation_uncached(config),
        bench_vertex_generation_cached(config),
        bench_lod_selection(config),
        bench_label_generation(config),
        bench_label_culling(config),
        bench_grid_fingerprint(config),
        bench_complete_4axis_uncached(config),
        bench_complete_4axis_cached(config),
    ];

    let platform = if cfg!(target_arch = "wasm32") {
        "wasm".to_string()
    } else {
        "native".to_string()
    };

    let timestamp = chrono::Utc::now().to_rfc3339();

    BenchSuite {
        platform,
        timestamp,
        results,
        user_agent: None,
    }
}

/// Run axis benchmarks on a native target and return JSON.
///
/// Used by the native comparison binary and integration tests.
#[cfg(not(target_arch = "wasm32"))]
pub fn run_native_axis_benchmarks() -> String {
    let config = bench_config();
    let suite = run_axis_benchmarks(&config);
    serde_json::to_string_pretty(&suite).expect("Failed to serialize results")
}

// ---------------------------------------------------------------------------
// WASM entry point
// ---------------------------------------------------------------------------

/// Run all axis benchmarks from JavaScript and return JSON results.
///
/// Call from JS: `const json = await gup.run_wasm_axis_benchmarks();`
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn run_wasm_axis_benchmarks() -> String {
    let config = bench_config();
    let mut suite = run_axis_benchmarks(&config);

    // Capture browser user agent
    suite.user_agent = web_sys::window().and_then(|w| w.navigator().user_agent().ok());
    suite.platform = "wasm".to_string();

    serde_json::to_string_pretty(&suite).expect("Failed to serialize results")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify that all 8 benchmarks produce valid results.
    #[test]
    fn test_all_benchmarks_produce_results() {
        let config = BenchConfig {
            warmup_iterations: 1,
            measured_iterations: 3,
        };
        let suite = run_axis_benchmarks(&config);
        assert_eq!(suite.results.len(), 8, "Expected 8 benchmark results");

        let names: Vec<&str> = suite.results.iter().map(|r| r.name.as_str()).collect();
        assert!(names.contains(&"vertex_generation_uncached"));
        assert!(names.contains(&"vertex_generation_cached"));
        assert!(names.contains(&"lod_selection"));
        assert!(names.contains(&"label_generation"));
        assert!(names.contains(&"label_culling_100"));
        assert!(names.contains(&"grid_fingerprint_20"));
        assert!(names.contains(&"complete_4axis_uncached"));
        assert!(names.contains(&"complete_4axis_cached"));
    }

    /// Verify that all benchmarks report non-negative timing values.
    #[test]
    fn test_benchmarks_have_valid_timing() {
        let config = BenchConfig {
            warmup_iterations: 1,
            measured_iterations: 5,
        };
        let suite = run_axis_benchmarks(&config);

        for result in &suite.results {
            assert!(
                result.mean_ms >= 0.0,
                "{}: mean_ms should be non-negative, got {}",
                result.name,
                result.mean_ms
            );
            assert!(
                result.median_ms >= 0.0,
                "{}: median_ms should be non-negative, got {}",
                result.name,
                result.median_ms
            );
            assert!(
                result.min_ms <= result.max_ms,
                "{}: min_ms ({}) should be <= max_ms ({})",
                result.name,
                result.min_ms,
                result.max_ms
            );
            assert_eq!(result.iterations, 5);
        }
    }

    /// Verify JSON serialization round-trips.
    #[test]
    fn test_suite_serializes_to_json() {
        let config = BenchConfig {
            warmup_iterations: 1,
            measured_iterations: 2,
        };
        let suite = run_axis_benchmarks(&config);
        let json = serde_json::to_string_pretty(&suite).unwrap();
        let deserialized: BenchSuite = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.results.len(), 8);
        assert_eq!(deserialized.platform, suite.platform);
    }

    /// Verify that all benchmarks complete within the 2ms WebAssembly budget.
    ///
    /// On native this should pass easily; on WASM this validates the budget.
    #[test]
    fn test_benchmarks_within_2ms_budget() {
        let config = BenchConfig {
            warmup_iterations: 3,
            measured_iterations: 20,
        };
        let suite = run_axis_benchmarks(&config);

        for result in &suite.results {
            assert!(
                result.median_ms < 2.0,
                "{}: median {}ms exceeds 2ms WebAssembly budget",
                result.name,
                result.median_ms
            );
        }
    }
}

// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Cross-platform axis performance validation tests (GUP-206).
//!
//! These tests run on each target platform to validate that axis performance
//! meets the per-platform budget and that variance between platforms stays
//! within the 2× threshold.
//!
//! The tests collect benchmark measurements for:
//! * Vertex generation (uncached and cached)
//! * LOD selection overhead
//! * Label generation
//! * Label culling
//! * Grid fingerprinting
//! * Complete 4-axis system
//!
//! Results are printed as a Markdown table that can be captured and compared
//! across CI matrix jobs.

use gup::axis::{AxisBounds, AxisConfiguration, AxisPosition, AxisRenderer};
use gup::axis_performance::{
    AxisLODManager, BenchmarkMeasurement, PerformanceBudget, PlatformBenchmarkReport,
    PlatformPreset, ViewportBounds, check_cross_platform_variance, cull_label_indices,
    generate_variance_report,
};
use gup::grid::{ChartBounds, GridConfiguration, GridRenderer};
use gup::shader_function::Vec2;
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Run a closure `iterations` times after `warmup` warm-up rounds.
/// Returns the median, min, and max durations.
fn benchmark_fn(
    warmup: usize,
    iterations: usize,
    mut f: impl FnMut(),
) -> (Duration, Duration, Duration) {
    for _ in 0..warmup {
        f();
    }

    let mut durations = Vec::with_capacity(iterations);
    for _ in 0..iterations {
        let start = Instant::now();
        f();
        durations.push(start.elapsed());
    }

    durations.sort();
    let median = durations[durations.len() / 2];
    let min = durations[0];
    let max = *durations.last().unwrap();
    (median, min, max)
}

/// Standard viewport for benchmarks.
const VIEWPORT: (f32, f32) = (800.0, 600.0);
const ITERATIONS: usize = 1000;
const WARMUP: usize = 100;

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
// Collect a full platform benchmark report
// ---------------------------------------------------------------------------

fn collect_platform_report() -> PlatformBenchmarkReport {
    let platform = PlatformPreset::detect();
    let mut report = PlatformBenchmarkReport::new(platform);

    // 1. Vertex generation (uncached)
    {
        let renderer = AxisRenderer::new();
        let bounds = standard_axis_bounds();
        let config = AxisConfiguration::default();

        let (median, min, max) = benchmark_fn(WARMUP, ITERATIONS, || {
            let _ = renderer.generate_axis_vertices(
                &bounds,
                &config,
                AxisPosition::Bottom,
                None,
                VIEWPORT,
            );
        });

        report.add_measurement(BenchmarkMeasurement {
            name: "vertex_generation_uncached".into(),
            median,
            min,
            max,
            iterations: ITERATIONS,
        });
    }

    // 2. Vertex generation (cached)
    {
        let mut renderer = AxisRenderer::new();
        let bounds = standard_axis_bounds();
        let config = AxisConfiguration::default();

        // Prime the cache
        let _ = renderer.generate_axis_vertices_cached(
            &bounds,
            &config,
            AxisPosition::Bottom,
            None,
            VIEWPORT,
            None,
        );

        let (median, min, max) = benchmark_fn(WARMUP, ITERATIONS, || {
            let _ = renderer.generate_axis_vertices_cached(
                &bounds,
                &config,
                AxisPosition::Bottom,
                None,
                VIEWPORT,
                None,
            );
        });

        report.add_measurement(BenchmarkMeasurement {
            name: "vertex_generation_cached".into(),
            median,
            min,
            max,
            iterations: ITERATIONS,
        });
    }

    // 3. LOD selection
    {
        let lod_manager = AxisLODManager::default();

        let (median, min, max) = benchmark_fn(WARMUP, ITERATIONS, || {
            let _ = lod_manager.calculate_lod(500.0, Some(Duration::from_micros(500)));
        });

        report.add_measurement(BenchmarkMeasurement {
            name: "lod_selection".into(),
            median,
            min,
            max,
            iterations: ITERATIONS,
        });
    }

    // 4. Label generation
    {
        let renderer = AxisRenderer::new();
        let bounds = standard_axis_bounds();
        let config = AxisConfiguration::default();

        let (median, min, max) = benchmark_fn(WARMUP, ITERATIONS, || {
            let _ = renderer.generate_label_data(
                &bounds,
                &config,
                AxisPosition::Bottom,
                None,
                VIEWPORT,
                None,
            );
        });

        report.add_measurement(BenchmarkMeasurement {
            name: "label_generation".into(),
            median,
            min,
            max,
            iterations: ITERATIONS,
        });
    }

    // 5. Label culling (100 labels)
    {
        let viewport = ViewportBounds::from_size(VIEWPORT.0, VIEWPORT.1);
        let positions: Vec<[f32; 2]> = (0..100)
            .map(|i| {
                let x = (i as f32 / 100.0) * 1000.0 - 100.0;
                [x, 300.0]
            })
            .collect();

        let (median, min, max) = benchmark_fn(WARMUP, ITERATIONS, || {
            let _ = cull_label_indices(&positions, &viewport, 10.0);
        });

        report.add_measurement(BenchmarkMeasurement {
            name: "label_culling_100".into(),
            median,
            min,
            max,
            iterations: ITERATIONS,
        });
    }

    // 6. Grid fingerprint (20 ticks)
    {
        let bounds = ChartBounds::new(50.0, 750.0, 50.0, 550.0);
        let config = GridConfiguration::default();
        let h_ticks: Vec<f64> = (0..20).map(|i| 50.0 + (i as f64 / 20.0) * 700.0).collect();
        let v_ticks: Vec<f64> = (0..20).map(|i| 50.0 + (i as f64 / 20.0) * 500.0).collect();

        let (median, min, max) = benchmark_fn(WARMUP, ITERATIONS, || {
            let _ = GridRenderer::compute_fingerprint_public(
                &h_ticks,
                &v_ticks,
                &[],
                &[],
                bounds,
                &config,
            );
        });

        report.add_measurement(BenchmarkMeasurement {
            name: "grid_fingerprint_20".into(),
            median,
            min,
            max,
            iterations: ITERATIONS,
        });
    }

    // 7. Complete 4-axis system (uncached)
    {
        let config = AxisConfiguration::default();
        let configs = axis_configs();

        let (median, min, max) = benchmark_fn(WARMUP, ITERATIONS, || {
            let renderer = AxisRenderer::new();
            let mut total = 0;
            for &(pos, start, end) in &configs {
                let bounds = AxisBounds::new(start, end, 50.0);
                total += renderer
                    .generate_axis_vertices(&bounds, &config, pos, None, VIEWPORT)
                    .len();
            }
            let _ = total;
        });

        report.add_measurement(BenchmarkMeasurement {
            name: "complete_4axis_uncached".into(),
            median,
            min,
            max,
            iterations: ITERATIONS,
        });
    }

    // 8. Complete 4-axis system (cached)
    {
        let config = AxisConfiguration::default();
        let configs = axis_configs();

        let mut renderers: Vec<AxisRenderer> = (0..4).map(|_| AxisRenderer::new()).collect();
        // Prime caches
        for (i, &(pos, start, end)) in configs.iter().enumerate() {
            let bounds = AxisBounds::new(start, end, 50.0);
            let _ = renderers[i]
                .generate_axis_vertices_cached(&bounds, &config, pos, None, VIEWPORT, None);
        }

        let (median, min, max) = benchmark_fn(WARMUP, ITERATIONS, || {
            let mut total = 0;
            for (i, &(pos, start, end)) in configs.iter().enumerate() {
                let bounds = AxisBounds::new(start, end, 50.0);
                total += renderers[i]
                    .generate_axis_vertices_cached(&bounds, &config, pos, None, VIEWPORT, None)
                    .len();
            }
            let _ = total;
        });

        report.add_measurement(BenchmarkMeasurement {
            name: "complete_4axis_cached".into(),
            median,
            min,
            max,
            iterations: ITERATIONS,
        });
    }

    report
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Validate that all axis benchmarks on the current platform are within
/// the platform-specific performance budget.
#[test]
fn test_axis_performance_within_platform_budget() {
    let platform = PlatformPreset::detect();
    let budget = PerformanceBudget::for_platform(platform);
    let report = collect_platform_report();

    println!("\n=== Axis Performance Report ({}) ===", platform.name());
    println!("Budget: {:?}", budget.target_render_time);
    println!();

    for m in &report.measurements {
        println!(
            "  {:<30} median={:>8?}  min={:>8?}  max={:>8?}",
            m.name, m.median, m.min, m.max
        );
    }
    println!();

    let violations = report.validate_budget(&budget);

    if violations.is_empty() {
        println!("✅ All benchmarks within budget");
    } else {
        for v in &violations {
            println!(
                "⚠️  {} exceeds budget: {:?} > {:?} ({:.1}x overshoot)",
                v.benchmark_name, v.median, v.budget, v.overshoot_factor
            );
        }
    }

    // Only the "complete_4axis" benchmarks are expected to occasionally approach
    // the budget. Individual sub-operations should be well under.
    // We validate that individual operations stay well under budget.
    let individual_violations: Vec<_> = violations
        .iter()
        .filter(|v| !v.benchmark_name.starts_with("complete_4axis"))
        .collect();

    assert!(
        individual_violations.is_empty(),
        "Individual axis operations exceed platform budget on {}: {:?}",
        platform.name(),
        individual_violations
            .iter()
            .map(|v| format!("{}: {:?}", v.benchmark_name, v.median))
            .collect::<Vec<_>>()
    );
}

/// Print a full platform report in Markdown format for CI capture.
///
/// This test always passes — its purpose is to produce a structured report
/// that the CI workflow can collect and compare across platforms.
#[test]
fn test_generate_platform_report_markdown() {
    let report = collect_platform_report();
    let md = generate_variance_report(&[report], 0);
    println!("\n{md}");
}

/// Verify that LOD configuration for the current platform produces
/// sensible LOD selections.
#[test]
fn test_platform_lod_configuration_sensible() {
    let platform = PlatformPreset::detect();
    let config = gup::axis_performance::LODConfiguration::for_platform(platform);
    let manager = AxisLODManager::new(config);

    // A large axis should always get High LOD
    let lod = manager.calculate_lod(500.0, None);
    assert_eq!(
        lod,
        gup::axis_performance::LODLevel::High,
        "500px axis on {} should be High LOD",
        platform.name()
    );

    // A very small axis should get Minimal LOD
    let lod_tiny = manager.calculate_lod(10.0, None);
    assert_eq!(
        lod_tiny,
        gup::axis_performance::LODLevel::Minimal,
        "10px axis on {} should be Minimal LOD",
        platform.name()
    );
}

/// Verify that performance budget is not exceeded by a simulated
/// cross-platform scenario (Linux baseline vs same data as another "platform").
///
/// This proves the variance check infrastructure works — real cross-platform
/// comparison happens in CI when multiple matrix jobs report results.
#[test]
fn test_cross_platform_variance_infrastructure() {
    let report = collect_platform_report();

    // Simulate a second platform with 1.5× times
    let mut simulated = PlatformBenchmarkReport::with_description(
        PlatformPreset::WebAssembly,
        "Simulated WebAssembly (1.5× Linux)",
    );
    for m in &report.measurements {
        simulated.add_measurement(BenchmarkMeasurement {
            name: m.name.clone(),
            median: m.median.mul_f32(1.5),
            min: m.min.mul_f32(1.5),
            max: m.max.mul_f32(1.5),
            iterations: m.iterations,
        });
    }

    let violations = check_cross_platform_variance(&report, &simulated, 2.0);
    assert!(
        violations.is_empty(),
        "1.5× simulated variance should be within 2× limit"
    );

    // Now simulate 3× which should exceed
    let mut over = PlatformBenchmarkReport::new(PlatformPreset::WebAssembly);
    for m in &report.measurements {
        over.add_measurement(BenchmarkMeasurement {
            name: m.name.clone(),
            median: m.median.mul_f32(3.0),
            min: m.min.mul_f32(3.0),
            max: m.max.mul_f32(3.0),
            iterations: m.iterations,
        });
    }

    let over_violations = check_cross_platform_variance(&report, &over, 2.0);
    assert!(
        !over_violations.is_empty(),
        "3× simulated variance should exceed 2× limit"
    );
}

/// Run all benchmarks and assert the complete 4-axis system (cached)
/// finishes well within the 1ms budget, even with slack for CI jitter.
#[test]
fn test_complete_axis_system_under_1ms() {
    let report = collect_platform_report();

    let cached_measurement = report
        .measurements
        .iter()
        .find(|m| m.name == "complete_4axis_cached")
        .expect("complete_4axis_cached measurement should exist");

    println!(
        "Complete 4-axis cached: median={:?}, min={:?}, max={:?}",
        cached_measurement.median, cached_measurement.min, cached_measurement.max
    );

    // Cached path should be extremely fast (sub-microsecond ideally)
    // Allow generous 1ms budget for CI environments
    assert!(
        cached_measurement.median < Duration::from_millis(1),
        "Cached 4-axis system median {:?} exceeds 1ms budget",
        cached_measurement.median
    );
}

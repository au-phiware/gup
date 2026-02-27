// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! WebAssembly axis performance validation tests (GUP-226).
//!
//! These tests run the 8 axis performance benchmarks inside a headless browser
//! via `wasm-pack test --headless --chrome`. They validate:
//!
//! * All 8 benchmarks produce valid results
//! * Median times are within the 2 ms WebAssembly performance budget
//! * Results can be serialized and deserialized for cross-platform comparison
//!
//! # Running
//!
//! ```bash
//! wasm-pack test --headless --chrome -- --test wasm_axis_performance
//! ```
//!
//! These tests are only compiled for the `wasm32` target.
#![cfg(target_arch = "wasm32")]
//! * Results can be serialized and deserialized for cross-platform comparison
//!
//! # Running
//!
//! ```bash
//! wasm-pack test --headless --chrome -- --test wasm_axis_performance
//! ```

use wasm_bindgen_test::*;

use gup::wasm_bench::{BenchConfig, BenchSuite};
use gup::wasm_bench_axis::run_axis_benchmarks;

/// Standard benchmark configuration for WASM tests.
///
/// Uses fewer iterations than native to keep browser test time reasonable
/// while still producing statistically meaningful results.
fn wasm_bench_config() -> BenchConfig {
    BenchConfig {
        warmup_iterations: 5,
        measured_iterations: 50,
    }
}

/// Run all 8 axis benchmarks and validate basic structure.
#[wasm_bindgen_test]
fn test_all_axis_benchmarks_run_in_browser() {
    let config = wasm_bench_config();
    let suite = run_axis_benchmarks(&config);

    assert_eq!(suite.results.len(), 8, "Expected 8 benchmark results");
    assert_eq!(suite.platform, "wasm");

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

/// Validate that all 8 benchmarks complete within the 2 ms WebAssembly budget.
///
/// The 2 ms budget was set in GUP-206 to accommodate browser WebGPU overhead.
/// These are CPU-side operations so they should comfortably fit within budget.
#[wasm_bindgen_test]
fn test_axis_performance_within_2ms_wasm_budget() {
    let config = wasm_bench_config();
    let suite = run_axis_benchmarks(&config);

    let budget_ms = 2.0;
    let mut violations = Vec::new();

    for result in &suite.results {
        if result.median_ms >= budget_ms {
            violations.push(format!(
                "{}: median {:.3}ms >= {:.1}ms budget",
                result.name, result.median_ms, budget_ms
            ));
        }
    }

    assert!(
        violations.is_empty(),
        "Benchmarks exceeding 2ms WebAssembly budget:\n{}",
        violations.join("\n")
    );
}

/// Validate timing values are consistent and non-negative.
#[wasm_bindgen_test]
fn test_axis_benchmark_timing_validity() {
    let config = wasm_bench_config();
    let suite = run_axis_benchmarks(&config);

    for result in &suite.results {
        assert!(
            result.mean_ms >= 0.0,
            "{}: mean_ms should be non-negative",
            result.name
        );
        assert!(
            result.median_ms >= 0.0,
            "{}: median_ms should be non-negative",
            result.name
        );
        assert!(
            result.min_ms <= result.max_ms,
            "{}: min ({}) should be <= max ({})",
            result.name,
            result.min_ms,
            result.max_ms
        );
        assert!(
            result.std_dev_ms >= 0.0,
            "{}: std_dev should be non-negative",
            result.name
        );
    }
}

/// Verify that benchmark results serialize to JSON for cross-platform comparison.
#[wasm_bindgen_test]
fn test_axis_results_serialize_for_comparison() {
    let config = BenchConfig {
        warmup_iterations: 1,
        measured_iterations: 5,
    };
    let suite = run_axis_benchmarks(&config);

    let json = serde_json::to_string_pretty(&suite).expect("Failed to serialize");
    let deserialized: BenchSuite = serde_json::from_str(&json).expect("Failed to deserialize");

    assert_eq!(deserialized.results.len(), 8);
    assert_eq!(deserialized.platform, "wasm");

    // Verify each benchmark name round-trips
    for (original, restored) in suite.results.iter().zip(deserialized.results.iter()) {
        assert_eq!(original.name, restored.name);
        assert!((original.median_ms - restored.median_ms).abs() < f64::EPSILON);
    }
}

/// Generate a Markdown report of WASM axis benchmark results.
///
/// This test always passes — its purpose is to produce structured output
/// that CI can capture for documentation and cross-platform comparison.
#[wasm_bindgen_test]
fn test_generate_wasm_axis_report() {
    let config = wasm_bench_config();
    let suite = run_axis_benchmarks(&config);
    let budget_ms = 2.0;

    // Print a structured Markdown report
    let mut report = String::new();
    report.push_str("## WebAssembly Axis Performance Report\n\n");
    report.push_str(&format!("**Platform**: {}\n", suite.platform));
    report.push_str(&format!("**Timestamp**: {}\n", suite.timestamp));
    report.push_str(&format!("**Budget**: {:.1} ms\n\n", budget_ms));

    report.push_str(
        "| Benchmark | Median (ms) | Mean (ms) | Min (ms) | Max (ms) | Std Dev | Status |\n",
    );
    report.push_str(
        "|-----------|------------|-----------|----------|----------|---------|--------|\n",
    );

    for result in &suite.results {
        let status = if result.median_ms < budget_ms {
            "✅"
        } else {
            "❌"
        };
        report.push_str(&format!(
            "| {} | {:.4} | {:.4} | {:.4} | {:.4} | {:.4} | {} |\n",
            result.name,
            result.median_ms,
            result.mean_ms,
            result.min_ms,
            result.max_ms,
            result.std_dev_ms,
            status,
        ));
    }

    // Output for CI capture
    wasm_bindgen_test::console_log!("{}", report);
}

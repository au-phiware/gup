// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! WebAssembly benchmark harness for cross-platform performance measurement.
//!
//! This module provides a lightweight benchmark runner that uses the browser's
//! `Performance.now()` API for high-resolution timing. It is designed to produce
//! results comparable to criterion benchmarks on native targets.
//!
//! # Architecture
//!
//! The harness runs each benchmark function multiple times (warmup + measured
//! iterations) and collects timing statistics. Results are serialized to JSON
//! for automated comparison with native baselines.
//!
//! # Usage
//!
//! This module is only compiled for `wasm32` targets. The exported
//! `wasm_bindgen` functions can be called from JavaScript in a browser
//! environment with WebGPU support.

use serde::{Deserialize, Serialize};

/// Result of a single benchmark measurement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchResult {
    /// Human-readable benchmark name (e.g. "point_queries/grid/1000").
    pub name: String,
    /// Number of measured iterations.
    pub iterations: u32,
    /// Total elapsed time across all iterations (milliseconds).
    pub total_ms: f64,
    /// Mean time per iteration (milliseconds).
    pub mean_ms: f64,
    /// Minimum observed iteration time (milliseconds).
    pub min_ms: f64,
    /// Maximum observed iteration time (milliseconds).
    pub max_ms: f64,
    /// Median iteration time (milliseconds).
    pub median_ms: f64,
    /// Standard deviation of iteration times (milliseconds).
    pub std_dev_ms: f64,
}

/// Collection of benchmark results from a full suite run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchSuite {
    /// Platform identifier (e.g. "wasm-chrome", "native-linux").
    pub platform: String,
    /// ISO 8601 timestamp of when the suite was run.
    pub timestamp: String,
    /// Individual benchmark results.
    pub results: Vec<BenchResult>,
    /// Browser/runtime user agent string (if available).
    pub user_agent: Option<String>,
}

/// Configuration for the benchmark runner.
#[derive(Debug, Clone)]
pub struct BenchConfig {
    /// Number of warmup iterations before measurement.
    pub warmup_iterations: u32,
    /// Number of measured iterations.
    pub measured_iterations: u32,
}

impl Default for BenchConfig {
    fn default() -> Self {
        Self {
            warmup_iterations: 3,
            measured_iterations: 10,
        }
    }
}

/// High-resolution timer abstraction.
///
/// On WASM targets, uses `Performance.now()`. On native targets, uses
/// `std::time::Instant` for testing.
pub struct Timer {
    #[cfg(target_arch = "wasm32")]
    performance: web_sys::Performance,
    #[cfg(not(target_arch = "wasm32"))]
    _phantom: std::marker::PhantomData<()>,
}

impl Timer {
    /// Create a new timer instance.
    #[cfg(target_arch = "wasm32")]
    pub fn new() -> Self {
        let performance = web_sys::window()
            .expect("no global window")
            .performance()
            .expect("no performance API");
        Self { performance }
    }

    /// Create a new timer instance (native fallback for testing).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new() -> Self {
        Self {
            _phantom: std::marker::PhantomData,
        }
    }

    /// Get current time in milliseconds.
    #[cfg(target_arch = "wasm32")]
    pub fn now_ms(&self) -> f64 {
        self.performance.now()
    }

    /// Get current time in milliseconds (native fallback).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn now_ms(&self) -> f64 {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs_f64()
            * 1000.0
    }
}

/// Run a benchmark function and collect timing statistics.
///
/// The function `f` is called `warmup + measured` times. Only the last
/// `measured` iterations contribute to the reported statistics.
pub fn run_bench<F>(name: &str, config: &BenchConfig, mut f: F) -> BenchResult
where
    F: FnMut(),
{
    let timer = Timer::new();

    // Warmup phase
    for _ in 0..config.warmup_iterations {
        f();
    }

    // Measurement phase
    let mut timings = Vec::with_capacity(config.measured_iterations as usize);
    for _ in 0..config.measured_iterations {
        let start = timer.now_ms();
        f();
        let elapsed = timer.now_ms() - start;
        timings.push(elapsed);
    }

    compute_stats(name, &timings)
}

/// Run an async benchmark function and collect timing statistics.
///
/// Same as [`run_bench`] but for async functions. Required for GPU operations
/// that use async APIs.
pub async fn run_bench_async<F, Fut>(name: &str, config: &BenchConfig, mut f: F) -> BenchResult
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = ()>,
{
    let timer = Timer::new();

    // Warmup phase
    for _ in 0..config.warmup_iterations {
        f().await;
    }

    // Measurement phase
    let mut timings = Vec::with_capacity(config.measured_iterations as usize);
    for _ in 0..config.measured_iterations {
        let start = timer.now_ms();
        f().await;
        let elapsed = timer.now_ms() - start;
        timings.push(elapsed);
    }

    compute_stats(name, &timings)
}

/// Collect timing measurements into a [`BenchResult`].
///
/// This is a convenience function for benchmarks that manage their own
/// timing loop (e.g. when async closures would cause borrow issues).
/// Pass in a vector of per-iteration durations in milliseconds.
pub fn from_timings(name: &str, timings: &[f64]) -> BenchResult {
    compute_stats(name, timings)
}

/// Compute descriptive statistics from a vector of timing measurements.
fn compute_stats(name: &str, timings: &[f64]) -> BenchResult {
    let n = timings.len() as f64;
    let total: f64 = timings.iter().sum();
    let mean = total / n;
    let min = timings.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = timings.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

    let mut sorted = timings.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = if sorted.len() % 2 == 0 {
        (sorted[sorted.len() / 2 - 1] + sorted[sorted.len() / 2]) / 2.0
    } else {
        sorted[sorted.len() / 2]
    };

    let variance = timings.iter().map(|t| (t - mean).powi(2)).sum::<f64>() / n;
    let std_dev = variance.sqrt();

    BenchResult {
        name: name.to_string(),
        iterations: timings.len() as u32,
        total_ms: total,
        mean_ms: mean,
        min_ms: min,
        max_ms: max,
        median_ms: median,
        std_dev_ms: std_dev,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_stats_basic() {
        let timings = vec![10.0, 20.0, 30.0, 40.0, 50.0];
        let result = compute_stats("test_bench", &timings);

        assert_eq!(result.name, "test_bench");
        assert_eq!(result.iterations, 5);
        assert!((result.total_ms - 150.0).abs() < f64::EPSILON);
        assert!((result.mean_ms - 30.0).abs() < f64::EPSILON);
        assert!((result.min_ms - 10.0).abs() < f64::EPSILON);
        assert!((result.max_ms - 50.0).abs() < f64::EPSILON);
        assert!((result.median_ms - 30.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_compute_stats_even_count() {
        let timings = vec![10.0, 20.0, 30.0, 40.0];
        let result = compute_stats("even", &timings);

        // Median of [10, 20, 30, 40] = (20+30)/2 = 25
        assert!((result.median_ms - 25.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_compute_stats_std_dev() {
        let timings = vec![10.0, 10.0, 10.0];
        let result = compute_stats("constant", &timings);

        assert!((result.std_dev_ms).abs() < f64::EPSILON);
    }

    #[test]
    fn test_run_bench_executes() {
        let config = BenchConfig {
            warmup_iterations: 1,
            measured_iterations: 3,
        };
        let mut call_count = 0u32;
        let result = run_bench("counter", &config, || {
            call_count += 1;
        });

        // warmup (1) + measured (3) = 4 total calls
        assert_eq!(call_count, 4);
        assert_eq!(result.iterations, 3);
        assert!(result.mean_ms >= 0.0);
    }

    #[test]
    fn test_bench_config_default() {
        let config = BenchConfig::default();
        assert_eq!(config.warmup_iterations, 3);
        assert_eq!(config.measured_iterations, 10);
    }

    #[test]
    fn test_bench_result_serialization() {
        let result = BenchResult {
            name: "test".to_string(),
            iterations: 10,
            total_ms: 100.0,
            mean_ms: 10.0,
            min_ms: 5.0,
            max_ms: 15.0,
            median_ms: 10.0,
            std_dev_ms: 3.0,
        };

        let json = serde_json::to_string(&result).unwrap();
        let deserialized: BenchResult = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "test");
        assert_eq!(deserialized.iterations, 10);
    }

    #[test]
    fn test_bench_suite_serialization() {
        let suite = BenchSuite {
            platform: "native-test".to_string(),
            timestamp: "2025-01-01T00:00:00Z".to_string(),
            results: vec![BenchResult {
                name: "bench1".to_string(),
                iterations: 5,
                total_ms: 50.0,
                mean_ms: 10.0,
                min_ms: 8.0,
                max_ms: 12.0,
                median_ms: 10.0,
                std_dev_ms: 1.5,
            }],
            user_agent: Some("test-agent".to_string()),
        };

        let json = serde_json::to_string_pretty(&suite).unwrap();
        let deserialized: BenchSuite = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.platform, "native-test");
        assert_eq!(deserialized.results.len(), 1);
    }
}

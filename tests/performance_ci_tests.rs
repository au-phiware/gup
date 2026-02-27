// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! CI/CD integration tests for automated performance regression detection.
//!
//! This test suite is designed to run in CI/CD pipelines and detect performance
//! regressions by comparing against stored baselines.
//!
//! Requires native (non-WASM) target due to `Send` + `Sync` bounds on GPU futures.
#![cfg(not(target_arch = "wasm32"))]

use gup::GupContext;
use gup::debug::{
    BaselineStorage, CiConfig, CiPerformanceRunner, GpuDebugContext, PerformanceSnapshot,
    PerformanceTestSuite,
};
use std::path::PathBuf;

/// Create test configuration for CI environment
fn create_ci_config() -> CiConfig {
    CiConfig {
        baseline_dir: PathBuf::from("baselines/performance"),
        fail_on_regression: true,
        max_suite_duration_secs: 300,
        ..Default::default()
    }
}

/// Basic rendering performance test
async fn test_basic_rendering(_ctx: &mut GpuDebugContext) -> gup::GupResult<PerformanceSnapshot> {
    // Simulate a basic rendering operation
    let start = std::time::Instant::now();

    // In a real test, this would render something
    std::thread::sleep(std::time::Duration::from_millis(5));

    let elapsed = start.elapsed();

    Ok(PerformanceSnapshot::new(
        elapsed.as_secs_f32() * 1000.0,
        1024 * 1024, // 1MB memory usage
    ))
}

/// Large dataset rendering performance test
async fn test_large_dataset_rendering(
    _ctx: &mut GpuDebugContext,
) -> gup::GupResult<PerformanceSnapshot> {
    let start = std::time::Instant::now();

    // Simulate rendering 100K points
    std::thread::sleep(std::time::Duration::from_millis(15));

    let elapsed = start.elapsed();

    Ok(PerformanceSnapshot::new(
        elapsed.as_secs_f32() * 1000.0,
        10 * 1024 * 1024, // 10MB memory usage
    ))
}

/// Shader compilation performance test
async fn test_shader_compilation(
    _ctx: &mut GpuDebugContext,
) -> gup::GupResult<PerformanceSnapshot> {
    let start = std::time::Instant::now();

    // Simulate shader compilation
    std::thread::sleep(std::time::Duration::from_millis(8));

    let elapsed = start.elapsed();

    Ok(PerformanceSnapshot::new(
        elapsed.as_secs_f32() * 1000.0,
        512 * 1024, // 512KB memory usage
    ))
}

/// Buffer upload performance test
async fn test_buffer_upload(_ctx: &mut GpuDebugContext) -> gup::GupResult<PerformanceSnapshot> {
    let start = std::time::Instant::now();

    // Simulate buffer upload
    std::thread::sleep(std::time::Duration::from_millis(3));

    let elapsed = start.elapsed();

    Ok(PerformanceSnapshot::new(
        elapsed.as_secs_f32() * 1000.0,
        5 * 1024 * 1024, // 5MB memory usage
    ))
}

#[tokio::test]
async fn run_ci_performance_suite() {
    // Initialize GPU context
    let context = GupContext::new()
        .await
        .expect("Failed to create GPU context");

    let debug_context = GpuDebugContext::new(&context.device, &context.queue);
    let config = create_ci_config();

    // Detect platform information
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::all(),
        ..Default::default()
    });

    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        })
        .await
        .expect("Failed to find adapter");

    let platform_info = gup::debug::PlatformInfo::from_adapter(&adapter);
    println!("\n🖥️  Testing on platform: {}", platform_info.description());

    // Create performance runner with platform info
    let mut runner =
        CiPerformanceRunner::new(debug_context, config).with_platform_info(platform_info.clone());

    // Build test suite
    let test_suite = PerformanceTestSuite::new("Gup Core Performance Suite")
        .add_test("basic_rendering", "rendering", |ctx| {
            Box::pin(test_basic_rendering(ctx))
        })
        .add_test("large_dataset_rendering", "rendering", |ctx| {
            Box::pin(test_large_dataset_rendering(ctx))
        })
        .add_test("shader_compilation", "compilation", |ctx| {
            Box::pin(test_shader_compilation(ctx))
        })
        .add_test("buffer_upload", "gpu_transfer", |ctx| {
            Box::pin(test_buffer_upload(ctx))
        });

    // Run the test suite
    let report = runner
        .run_performance_suite(test_suite)
        .await
        .expect("Failed to run performance suite");

    // Export reports for CI artifacts
    runner
        .export_report(&report, &PathBuf::from("performance_report.json"))
        .expect("Failed to export JSON report");

    let markdown_report = runner.export_report_markdown(&report);
    std::fs::write("performance_report.md", markdown_report)
        .expect("Failed to write Markdown report");

    println!("\n📊 Performance Test Summary:");
    println!("  Tests run: {}", report.test_results.len());
    let passed = report.test_results.iter().filter(|r| r.passed).count();
    println!("  Passed: {}", passed);
    println!("  Failed: {}", report.test_results.len() - passed);
    println!("  Duration: {}ms", report.duration_ms);

    // Check for regressions
    let regressions = runner.check_regressions(&report);
    if !regressions.is_empty() {
        println!("\n⚠️  Performance Regressions Detected:");
        for regression in &regressions {
            println!(
                "  - {} ({:?}): frame time {:+.1}%, memory {:+.1}%",
                regression.test_name,
                regression.severity,
                regression.frame_time_delta_percent,
                regression.memory_delta_percent
            );
        }

        // In CI, we would fail here if configured to do so
        if report.config.fail_on_regression {
            panic!("Performance regressions detected!");
        }
    } else {
        println!("\n✅ No performance regressions detected");
    }

    // Update baselines if this is a baseline update run
    if std::env::var("UPDATE_BASELINES").is_ok() {
        println!("\n📝 Updating performance baselines...");
        runner
            .update_baselines(&report)
            .expect("Failed to update baselines");
        println!("✅ Baselines updated successfully");
    }

    // Print individual test results
    println!("\n📈 Individual Test Results:");
    for result in &report.test_results {
        let status = if result.passed { "✅" } else { "❌" };
        println!(
            "  {} {} - {:.2}ms ({}KB)",
            status,
            result.test_name,
            result.snapshot.frame_time_ms,
            result.snapshot.memory_usage_bytes / 1024
        );

        if let Some(comparison) = &result.baseline_comparison {
            println!(
                "       vs baseline: {:+.1}% frame time, {:+.1}% memory",
                comparison.frame_time_delta_percent, comparison.memory_delta_percent
            );
        }
    }
}

#[tokio::test]
async fn test_baseline_management() {
    let storage = BaselineStorage::new(PathBuf::from("/tmp/gup_test_baselines"));

    // Clean up any previous test data
    let _ = std::fs::remove_dir_all("/tmp/gup_test_baselines");
    std::fs::create_dir_all("/tmp/gup_test_baselines/test_category")
        .expect("Failed to create test directory");

    // Create and save a baseline
    let baseline = gup::debug::ci_performance::PerformanceBaseline {
        test_name: "test_foo".to_string(),
        category: "test_category".to_string(),
        avg_frame_time_ms: 10.0,
        avg_memory_usage_bytes: 1024 * 1024,
        sample_count: 10,
        last_updated: chrono::Utc::now(),
        metadata: std::collections::HashMap::new(),
        platform_id: "default".to_string(),
    };

    storage
        .save_baseline("test_foo", "test_category", "default", &baseline)
        .expect("Failed to save baseline");

    // Load it back
    let loaded = storage
        .load_baseline("test_foo", "test_category", "default")
        .expect("Failed to load baseline");

    assert_eq!(loaded.test_name, "test_foo");
    assert_eq!(loaded.avg_frame_time_ms, 10.0);
    assert_eq!(loaded.avg_memory_usage_bytes, 1024 * 1024);

    // List baselines
    let baselines = storage.list_baselines().expect("Failed to list baselines");
    assert_eq!(baselines.len(), 1);
    assert_eq!(
        baselines[0],
        (
            "default".to_string(),
            "test_category".to_string(),
            "test_foo".to_string()
        )
    );

    // Clean up
    std::fs::remove_dir_all("/tmp/gup_test_baselines").expect("Failed to clean up test directory");
}

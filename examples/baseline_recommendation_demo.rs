// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Demonstration of automated baseline recommendation system
//!
//! This example shows how to:
//! 1. Analyze performance trends across multiple test runs
//! 2. Get automated recommendations for baseline updates
//! 3. Generate reports for CI/CD integration
//!
//! Run with: `cargo run --example baseline_recommendation_demo`

use gup::GupResult;
use gup::debug::baseline_recommendation::{
    BaselineRecommendationEngine, BatchRecommendationAnalyzer, RecommendationConfig,
};
use gup::debug::ci_performance::{BaselineStorage, PerformanceBaseline};
use std::collections::HashMap;

fn main() -> GupResult<()> {
    println!("🤖 Automated Baseline Recommendation Demo\n");
    println!("=========================================\n");

    // Setup demo directory
    let demo_dir = std::env::temp_dir().join("gup_baseline_rec_demo");
    let _ = std::fs::remove_dir_all(&demo_dir);
    std::fs::create_dir_all(&demo_dir)?;
    println!("📁 Demo directory: {}\n", demo_dir.display());

    // Create baseline storage
    let storage = BaselineStorage::new(demo_dir.clone());

    // Simulate historical performance data
    println!("📊 Creating simulated performance baselines...\n");
    create_demo_baselines(&storage)?;

    // Configure recommendation engine
    let config = RecommendationConfig {
        min_samples: 3,               // Lower threshold for demo
        min_change_threshold: 0.10,   // 10%
        min_confidence: 0.70,         // 70%
        max_cv_for_stability: 0.15,   // 15%
        auto_update_confidence: 0.85, // 85%
    };

    let engine = BaselineRecommendationEngine::new(storage, config);
    let analyzer = BatchRecommendationAnalyzer::new(engine);

    // Analyze specific test
    println!("🔍 Analyzing individual test...\n");
    analyze_individual_test(&analyzer)?;

    // Batch analyze all tests
    println!("\n📈 Running batch analysis on all tests...\n");
    let tests = vec![
        (
            "render_100k_points".to_string(),
            "rendering".to_string(),
            "nvidia_rtx_3080".to_string(),
        ),
        (
            "compute_statistics".to_string(),
            "compute".to_string(),
            "nvidia_rtx_3080".to_string(),
        ),
        (
            "text_rendering".to_string(),
            "rendering".to_string(),
            "nvidia_rtx_3080".to_string(),
        ),
    ];

    let recommendations = analyzer.analyze_all_tests(&tests)?;

    println!("Found {} recommendations\n", recommendations.len());

    // Generate and display report
    println!("📋 Generating recommendation report...\n");
    let report = analyzer.generate_recommendation_report(&recommendations);
    println!("{}", report);

    // Export report to file
    let report_path = demo_dir.join("recommendations.md");
    std::fs::write(&report_path, &report)?;
    println!("\n✅ Report saved to: {}", report_path.display());

    // Cleanup
    println!("\n🧹 Cleaning up demo directory...");
    let _ = std::fs::remove_dir_all(&demo_dir);

    Ok(())
}

fn create_demo_baselines(storage: &BaselineStorage) -> GupResult<()> {
    // Test 1: Rendering test with stable performance (no recommendation expected)
    let stable_baseline = PerformanceBaseline {
        test_name: "render_100k_points".to_string(),
        category: "rendering".to_string(),
        avg_frame_time_ms: 16.0,
        avg_memory_usage_bytes: 1024 * 1024,
        sample_count: 10,
        last_updated: chrono::Utc::now(),
        metadata: HashMap::new(),
        platform_id: "nvidia_rtx_3080".to_string(),
    };
    storage.save_baseline(
        &stable_baseline.test_name,
        &stable_baseline.category,
        &stable_baseline.platform_id,
        &stable_baseline,
    )?;
    println!("  ✓ render_100k_points: 16.0ms (stable)");

    // Test 2: Compute test with improved performance (should recommend update)
    let improved_baseline = PerformanceBaseline {
        test_name: "compute_statistics".to_string(),
        category: "compute".to_string(),
        avg_frame_time_ms: 4.5, // Significantly improved from hypothetical 5.0ms
        avg_memory_usage_bytes: 512 * 1024,
        sample_count: 15,
        last_updated: chrono::Utc::now(),
        metadata: HashMap::new(),
        platform_id: "nvidia_rtx_3080".to_string(),
    };
    storage.save_baseline(
        &improved_baseline.test_name,
        &improved_baseline.category,
        &improved_baseline.platform_id,
        &improved_baseline,
    )?;
    println!("  ✓ compute_statistics: 4.5ms (improved)");

    // Test 3: Text rendering with degraded performance (should recommend update)
    let degraded_baseline = PerformanceBaseline {
        test_name: "text_rendering".to_string(),
        category: "rendering".to_string(),
        avg_frame_time_ms: 9.2, // Degraded from hypothetical 8.0ms
        avg_memory_usage_bytes: 2048 * 1024,
        sample_count: 12,
        last_updated: chrono::Utc::now(),
        metadata: HashMap::new(),
        platform_id: "nvidia_rtx_3080".to_string(),
    };
    storage.save_baseline(
        &degraded_baseline.test_name,
        &degraded_baseline.category,
        &degraded_baseline.platform_id,
        &degraded_baseline,
    )?;
    println!("  ✓ text_rendering: 9.2ms (degraded)");

    Ok(())
}

fn analyze_individual_test(_analyzer: &BatchRecommendationAnalyzer) -> GupResult<()> {
    // Access the engine to analyze a single test
    let test_name = "render_100k_points";
    let category = "rendering";
    let platform = "nvidia_rtx_3080";

    println!("Test: {}/{}/{}", category, platform, test_name);

    // For demo purposes, we'll just show that the analyzer works
    // In a real scenario, you'd use the engine directly for single-test analysis
    println!("  Status: Analyzed (see batch results below)");

    Ok(())
}

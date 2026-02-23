// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Integration tests for automated baseline recommendation system

use gup::GupResult;
use gup::debug::baseline_recommendation::{
    BaselineRecommendationEngine, BatchRecommendationAnalyzer, RecommendationConfig,
};
use gup::debug::ci_performance::{BaselineStorage, PerformanceBaseline};
use std::collections::HashMap;

#[test]
fn test_baseline_recommendation_workflow() -> GupResult<()> {
    // Setup test directory
    let test_dir = std::env::temp_dir().join("gup_baseline_rec_workflow");
    let _ = std::fs::remove_dir_all(&test_dir);
    std::fs::create_dir_all(&test_dir)?;

    let storage = BaselineStorage::new(test_dir.clone());

    // Create initial baseline
    let initial_baseline = PerformanceBaseline {
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
        &initial_baseline.test_name,
        &initial_baseline.category,
        &initial_baseline.platform_id,
        &initial_baseline,
    )?;

    // Create engine
    let engine = BaselineRecommendationEngine::with_defaults(storage);

    // Analyze trend (should not recommend update with single sample)
    let analysis_result =
        engine.analyze_performance_trend("render_100k_points", "rendering", "nvidia_rtx_3080");

    assert!(
        analysis_result.is_ok(),
        "Analysis should succeed: {:?}",
        analysis_result
    );
    let analysis = analysis_result.unwrap();

    // Should not recommend update with insufficient samples
    let recommendation = engine.recommend_baseline_update(&analysis);
    assert!(
        recommendation.is_none(),
        "Should not recommend with single sample"
    );

    // Cleanup
    let _ = std::fs::remove_dir_all(&test_dir);
    Ok(())
}

#[test]
fn test_batch_recommendation_analyzer() -> GupResult<()> {
    // Setup test directory
    let test_dir = std::env::temp_dir().join("gup_batch_rec_test");
    let _ = std::fs::remove_dir_all(&test_dir);
    std::fs::create_dir_all(&test_dir)?;

    let storage = BaselineStorage::new(test_dir.clone());

    // Create multiple baselines
    let tests = vec![
        ("test_a", "category_a", 10.0),
        ("test_b", "category_b", 20.0),
        ("test_c", "category_c", 30.0),
    ];

    for (test_name, category, frame_time) in &tests {
        let baseline = PerformanceBaseline {
            test_name: test_name.to_string(),
            category: category.to_string(),
            avg_frame_time_ms: *frame_time,
            avg_memory_usage_bytes: 1024 * 1024,
            sample_count: 10,
            last_updated: chrono::Utc::now(),
            metadata: HashMap::new(),
            platform_id: "test_platform".to_string(),
        };
        storage.save_baseline(test_name, category, "test_platform", &baseline)?;
    }

    // Create engine and batch analyzer
    let engine = BaselineRecommendationEngine::with_defaults(storage);
    let analyzer = BatchRecommendationAnalyzer::new(engine);

    // Analyze all tests
    let test_list: Vec<_> = tests
        .iter()
        .map(|(name, cat, _)| {
            (
                name.to_string(),
                cat.to_string(),
                "test_platform".to_string(),
            )
        })
        .collect();

    let recommendations = analyzer.analyze_all_tests(&test_list)?;

    // Should not have recommendations (insufficient data for trend)
    assert_eq!(
        recommendations.len(),
        0,
        "Should have no recommendations with insufficient data"
    );

    // Generate report
    let report = analyzer.generate_recommendation_report(&recommendations);
    assert!(
        report.contains("No baseline updates recommended"),
        "Report should indicate no recommendations"
    );

    // Cleanup
    let _ = std::fs::remove_dir_all(&test_dir);
    Ok(())
}

#[test]
fn test_confidence_scoring_factors() -> GupResult<()> {
    let test_dir = std::env::temp_dir().join("gup_confidence_test");
    let _ = std::fs::remove_dir_all(&test_dir);
    std::fs::create_dir_all(&test_dir)?;

    let storage = BaselineStorage::new(test_dir.clone());
    let config = RecommendationConfig {
        min_samples: 5,
        min_change_threshold: 0.15,   // 15%
        min_confidence: 0.70,         // 70%
        max_cv_for_stability: 0.15,   // 15%
        auto_update_confidence: 0.85, // 85%
    };
    let engine = BaselineRecommendationEngine::new(storage, config);

    // Test high-confidence scenario
    let high_confidence_analysis = gup::debug::baseline_recommendation::TrendAnalysis {
        test_name: "test".to_string(),
        category: "cat".to_string(),
        platform_id: "platform".to_string(),
        sample_count: 20, // Many samples
        current_baseline: 10.0,
        recent_average: 12.0, // 20% increase
        std_deviation: 0.3,
        coefficient_of_variation: 0.025, // 2.5% - very stable
        significant_shift: true,
        percent_change: 0.20, // 20%
        is_stable: true,
    };

    let recommendation = engine.recommend_baseline_update(&high_confidence_analysis);
    assert!(
        recommendation.is_some(),
        "Should recommend with high confidence"
    );

    let rec = recommendation.unwrap();
    assert!(
        rec.confidence > 0.80,
        "Confidence should be high: {}",
        rec.confidence
    );
    assert_eq!(rec.recommended_baseline, 12.0);
    assert!(
        rec.rationale.contains("increased"),
        "Rationale should mention increase"
    );

    // Test low-confidence scenario (high variance)
    let low_confidence_analysis = gup::debug::baseline_recommendation::TrendAnalysis {
        test_name: "test".to_string(),
        category: "cat".to_string(),
        platform_id: "platform".to_string(),
        sample_count: 10,
        current_baseline: 10.0,
        recent_average: 12.0,
        std_deviation: 2.0,
        coefficient_of_variation: 0.167, // 16.7% - unstable
        significant_shift: true,
        percent_change: 0.20,
        is_stable: false, // High variance
    };

    let recommendation = engine.recommend_baseline_update(&low_confidence_analysis);
    assert!(
        recommendation.is_none(),
        "Should not recommend with unstable performance"
    );

    // Cleanup
    let _ = std::fs::remove_dir_all(&test_dir);
    Ok(())
}

#[test]
fn test_auto_update_threshold() -> GupResult<()> {
    let test_dir = std::env::temp_dir().join("gup_auto_update_test");
    let _ = std::fs::remove_dir_all(&test_dir);
    std::fs::create_dir_all(&test_dir)?;

    let storage = BaselineStorage::new(test_dir.clone());
    let config = RecommendationConfig {
        min_samples: 5,
        min_change_threshold: 0.10,
        min_confidence: 0.70,
        max_cv_for_stability: 0.10,
        auto_update_confidence: 0.95, // Very high threshold for auto-update
    };
    let engine = BaselineRecommendationEngine::new(storage, config);

    // Create analysis that meets confidence threshold but not auto-update threshold
    let analysis = gup::debug::baseline_recommendation::TrendAnalysis {
        test_name: "test".to_string(),
        category: "cat".to_string(),
        platform_id: "platform".to_string(),
        sample_count: 15,
        current_baseline: 10.0,
        recent_average: 11.0,
        std_deviation: 0.4,
        coefficient_of_variation: 0.036, // ~3.6% - stable
        significant_shift: true,
        percent_change: 0.10,
        is_stable: true,
    };

    let recommendation = engine.recommend_baseline_update(&analysis);
    assert!(
        recommendation.is_some(),
        "Should recommend with sufficient confidence"
    );

    let rec = recommendation.unwrap();
    assert!(
        rec.confidence >= 0.70,
        "Should meet minimum confidence threshold"
    );
    assert!(
        !rec.should_auto_update,
        "Should not auto-update below 95% confidence"
    );

    // Cleanup
    let _ = std::fs::remove_dir_all(&test_dir);
    Ok(())
}

#[test]
fn test_recommendation_report_formatting() -> GupResult<()> {
    let test_dir = std::env::temp_dir().join("gup_report_format_test");
    let _ = std::fs::remove_dir_all(&test_dir);
    std::fs::create_dir_all(&test_dir)?;

    let storage = BaselineStorage::new(test_dir.clone());
    let engine = BaselineRecommendationEngine::with_defaults(storage);
    let analyzer = BatchRecommendationAnalyzer::new(engine);

    // Create mock recommendations
    let mut recommendations = HashMap::new();

    recommendations.insert(
        "rendering/platform_a/test_high_conf".to_string(),
        gup::debug::baseline_recommendation::BaselineRecommendation {
            current_baseline: 10.0,
            recommended_baseline: 11.0,
            confidence: 0.95,
            sample_count: 20,
            stability_score: 0.90,
            rationale: "High confidence recommendation".to_string(),
            should_auto_update: true,
        },
    );

    recommendations.insert(
        "rendering/platform_a/test_med_conf".to_string(),
        gup::debug::baseline_recommendation::BaselineRecommendation {
            current_baseline: 20.0,
            recommended_baseline: 22.0,
            confidence: 0.80,
            sample_count: 15,
            stability_score: 0.75,
            rationale: "Medium confidence recommendation".to_string(),
            should_auto_update: false,
        },
    );

    recommendations.insert(
        "rendering/platform_a/test_low_conf".to_string(),
        gup::debug::baseline_recommendation::BaselineRecommendation {
            current_baseline: 30.0,
            recommended_baseline: 33.0,
            confidence: 0.65,
            sample_count: 10,
            stability_score: 0.60,
            rationale: "Low confidence recommendation".to_string(),
            should_auto_update: false,
        },
    );

    // Generate report
    let report = analyzer.generate_recommendation_report(&recommendations);

    // Verify report structure
    assert!(
        report.contains("# Baseline Update Recommendations"),
        "Should have main title"
    );
    assert!(
        report.contains("## High Confidence"),
        "Should have high confidence section"
    );
    assert!(
        report.contains("## Medium Confidence"),
        "Should have medium confidence section"
    );
    assert!(
        report.contains("## Low Confidence"),
        "Should have low confidence section"
    );

    // Verify test names appear
    assert!(report.contains("test_high_conf"));
    assert!(report.contains("test_med_conf"));
    assert!(report.contains("test_low_conf"));

    // Verify recommendation details
    assert!(report.contains("**Current**: 10.00ms"));
    assert!(report.contains("**Recommended**: 11.00ms"));
    assert!(report.contains("**Auto-update**: ✅ Yes"));
    assert!(report.contains("**Auto-update**: ❌ No"));

    // Cleanup
    let _ = std::fs::remove_dir_all(&test_dir);
    Ok(())
}

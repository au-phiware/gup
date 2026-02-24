// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Automated baseline recommendation engine for performance testing.
//!
//! This module provides statistical analysis of performance trends to automatically
//! recommend when baselines should be updated, reducing manual intervention and
//! improving the accuracy of regression detection.

use crate::debug::ci_performance::{BaselineStorage, PerformanceBaseline};
use crate::error::{GupError, GupResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Statistical analysis of performance trends for a specific test
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendAnalysis {
    /// Test name being analyzed
    pub test_name: String,
    /// Performance category
    pub category: String,
    /// Platform identifier
    pub platform_id: String,
    /// Number of samples analyzed
    pub sample_count: usize,
    /// Current baseline value (milliseconds)
    pub current_baseline: f32,
    /// Average performance over recent samples (milliseconds)
    pub recent_average: f32,
    /// Standard deviation of recent samples
    pub std_deviation: f32,
    /// Coefficient of variation (std_dev / mean) - lower is more stable
    pub coefficient_of_variation: f32,
    /// Whether performance has significantly shifted from baseline
    pub significant_shift: bool,
    /// Percentage change from baseline
    pub percent_change: f32,
    /// Whether the trend shows stable performance
    pub is_stable: bool,
}

/// Recommendation for updating a performance baseline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaselineRecommendation {
    /// Current baseline value (milliseconds)
    pub current_baseline: f32,
    /// Recommended new baseline value (milliseconds)
    pub recommended_baseline: f32,
    /// Confidence score (0.0 - 1.0)
    pub confidence: f32,
    /// Number of samples used in analysis
    pub sample_count: usize,
    /// Stability score (0.0 - 1.0) - higher is more stable
    pub stability_score: f32,
    /// Human-readable rationale for the recommendation
    pub rationale: String,
    /// Whether this should trigger an auto-update
    pub should_auto_update: bool,
}

/// Configuration for baseline recommendation engine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecommendationConfig {
    /// Minimum number of samples required for recommendation
    pub min_samples: usize,
    /// Minimum percentage change to consider significant (default 10%)
    pub min_change_threshold: f32,
    /// Minimum confidence score for recommendations (default 0.8 = 80%)
    pub min_confidence: f32,
    /// Maximum coefficient of variation for stability (default 0.1 = 10%)
    pub max_cv_for_stability: f32,
    /// Confidence threshold for auto-update (default 0.9 = 90%)
    pub auto_update_confidence: f32,
}

impl Default for RecommendationConfig {
    fn default() -> Self {
        Self {
            min_samples: 10,
            min_change_threshold: 0.10,   // 10%
            min_confidence: 0.80,         // 80%
            max_cv_for_stability: 0.10,   // 10% CV
            auto_update_confidence: 0.90, // 90%
        }
    }
}

/// Engine for analyzing performance trends and recommending baseline updates
pub struct BaselineRecommendationEngine {
    baseline_storage: BaselineStorage,
    config: RecommendationConfig,
}

impl BaselineRecommendationEngine {
    /// Create a new baseline recommendation engine
    pub fn new(baseline_storage: BaselineStorage, config: RecommendationConfig) -> Self {
        Self {
            baseline_storage,
            config,
        }
    }

    /// Create with default configuration
    pub fn with_defaults(baseline_storage: BaselineStorage) -> Self {
        Self::new(baseline_storage, RecommendationConfig::default())
    }

    /// Analyze performance trend for a specific test
    ///
    /// Loads historical baseline data and performs statistical analysis to determine
    /// if the performance characteristics have changed significantly.
    pub fn analyze_performance_trend(
        &self,
        test_name: &str,
        category: &str,
        platform_id: &str,
    ) -> GupResult<TrendAnalysis> {
        // Load current baseline
        let current_baseline =
            self.baseline_storage
                .load_baseline(test_name, category, platform_id)?;

        // Load historical data (we'll use the baseline storage's list functionality)
        let historical_data = self.load_historical_data(test_name, category, platform_id)?;

        if historical_data.is_empty() {
            return Err(GupError::validation_error(format!(
                "No historical data available for test '{}'",
                test_name
            )));
        }

        // Calculate statistics on recent samples
        let recent_average = self.calculate_average(&historical_data);
        let std_deviation = self.calculate_std_deviation(&historical_data, recent_average);
        let coefficient_of_variation = if recent_average > 0.0 {
            std_deviation / recent_average
        } else {
            f32::MAX
        };

        // Determine if performance is stable
        let is_stable = coefficient_of_variation <= self.config.max_cv_for_stability;

        // Calculate percentage change from baseline
        let percent_change = if current_baseline.avg_frame_time_ms > 0.0 {
            (recent_average - current_baseline.avg_frame_time_ms)
                / current_baseline.avg_frame_time_ms
        } else {
            0.0
        };

        // Check if shift is significant
        let significant_shift = percent_change.abs() >= self.config.min_change_threshold;

        Ok(TrendAnalysis {
            test_name: test_name.to_string(),
            category: category.to_string(),
            platform_id: platform_id.to_string(),
            sample_count: historical_data.len(),
            current_baseline: current_baseline.avg_frame_time_ms,
            recent_average,
            std_deviation,
            coefficient_of_variation,
            significant_shift,
            percent_change,
            is_stable,
        })
    }

    /// Recommend baseline update based on trend analysis
    ///
    /// Analyzes the trend and returns a recommendation if the performance has
    /// stabilized at a new level with sufficient confidence.
    pub fn recommend_baseline_update(
        &self,
        analysis: &TrendAnalysis,
    ) -> Option<BaselineRecommendation> {
        // Check minimum sample requirement
        if analysis.sample_count < self.config.min_samples {
            return None;
        }

        // Check if change is significant
        if !analysis.significant_shift {
            return None;
        }

        // Check if performance is stable
        if !analysis.is_stable {
            return None;
        }

        // Calculate confidence score based on multiple factors
        let confidence = self.calculate_confidence(analysis);

        // Check if confidence meets minimum threshold
        if confidence < self.config.min_confidence {
            return None;
        }

        // Calculate stability score (inverse of CV, normalized to 0-1)
        let stability_score = (1.0 - analysis.coefficient_of_variation.min(1.0)).max(0.0);

        // Generate rationale
        let direction = if analysis.percent_change > 0.0 {
            "increased"
        } else {
            "decreased"
        };
        let rationale = format!(
            "Performance has {} by {:.1}% and stabilized (CV: {:.1}%) \
             over {} samples with {:.0}% confidence",
            direction,
            analysis.percent_change.abs() * 100.0,
            analysis.coefficient_of_variation * 100.0,
            analysis.sample_count,
            confidence * 100.0
        );

        // Determine if auto-update should be triggered
        let should_auto_update = confidence >= self.config.auto_update_confidence;

        Some(BaselineRecommendation {
            current_baseline: analysis.current_baseline,
            recommended_baseline: analysis.recent_average,
            confidence,
            sample_count: analysis.sample_count,
            stability_score,
            rationale,
            should_auto_update,
        })
    }

    /// Calculate confidence score for a recommendation
    ///
    /// Confidence is based on:
    /// - Number of samples (more = higher confidence)
    /// - Stability (lower CV = higher confidence)
    /// - Magnitude of change (larger = higher confidence, but capped)
    fn calculate_confidence(&self, analysis: &TrendAnalysis) -> f32 {
        // Sample size factor (0.5 to 1.0)
        let sample_factor =
            (analysis.sample_count as f32 / (self.config.min_samples as f32 * 2.0)).clamp(0.5, 1.0);

        // Stability factor (0.0 to 1.0)
        let stability_factor = (1.0 - analysis.coefficient_of_variation).max(0.0);

        // Change magnitude factor (0.5 to 1.0)
        // Larger changes are more confident, but cap at 2x the threshold
        let change_magnitude = analysis.percent_change.abs() / self.config.min_change_threshold;
        let change_factor = (change_magnitude / 2.0).clamp(0.5, 1.0);

        // Weighted average: stability is most important
        let confidence = stability_factor * 0.5 + sample_factor * 0.3 + change_factor * 0.2;

        confidence.clamp(0.0, 1.0)
    }

    /// Calculate average of frame times
    fn calculate_average(&self, baselines: &[PerformanceBaseline]) -> f32 {
        if baselines.is_empty() {
            return 0.0;
        }

        let sum: f32 = baselines.iter().map(|b| b.avg_frame_time_ms).sum();
        sum / baselines.len() as f32
    }

    /// Calculate standard deviation of frame times
    fn calculate_std_deviation(&self, baselines: &[PerformanceBaseline], mean: f32) -> f32 {
        if baselines.len() < 2 {
            return 0.0;
        }

        let variance: f32 = baselines
            .iter()
            .map(|b| {
                let diff = b.avg_frame_time_ms - mean;
                diff * diff
            })
            .sum::<f32>()
            / (baselines.len() - 1) as f32;

        variance.sqrt()
    }

    /// Load historical baseline data for analysis
    ///
    /// This loads all available baseline files for the given test.
    /// In a real implementation, this would load time-series data.
    /// For now, we simulate by reading the single baseline file multiple times
    /// (in practice, CI would store historical snapshots).
    fn load_historical_data(
        &self,
        test_name: &str,
        category: &str,
        platform_id: &str,
    ) -> GupResult<Vec<PerformanceBaseline>> {
        // For now, just return the single baseline
        // In a full implementation, this would query a history of baselines
        let baseline = self
            .baseline_storage
            .load_baseline(test_name, category, platform_id)?;

        Ok(vec![baseline])
    }
}

/// Batch analyzer for processing multiple tests
pub struct BatchRecommendationAnalyzer {
    engine: BaselineRecommendationEngine,
}

impl BatchRecommendationAnalyzer {
    /// Create a new batch analyzer
    pub fn new(engine: BaselineRecommendationEngine) -> Self {
        Self { engine }
    }

    /// Analyze all tests and generate recommendations
    ///
    /// Returns a map of test names to recommendations (only for tests that have recommendations)
    pub fn analyze_all_tests(
        &self,
        tests: &[(String, String, String)], // (test_name, category, platform_id)
    ) -> GupResult<HashMap<String, BaselineRecommendation>> {
        let mut recommendations = HashMap::new();

        for (test_name, category, platform_id) in tests {
            // Analyze trend
            match self
                .engine
                .analyze_performance_trend(test_name, category, platform_id)
            {
                Ok(analysis) => {
                    // Check if recommendation should be made
                    if let Some(recommendation) = self.engine.recommend_baseline_update(&analysis) {
                        let key = format!("{}/{}/{}", category, platform_id, test_name);
                        recommendations.insert(key, recommendation);
                    }
                }
                Err(_) => {
                    // Skip tests that fail analysis (e.g., no historical data)
                    continue;
                }
            }
        }

        Ok(recommendations)
    }

    /// Generate a report of all recommendations
    pub fn generate_recommendation_report(
        &self,
        recommendations: &HashMap<String, BaselineRecommendation>,
    ) -> String {
        if recommendations.is_empty() {
            return "No baseline updates recommended at this time.".to_string();
        }

        let mut report = String::from("# Baseline Update Recommendations\n\n");

        // Group by confidence level
        let mut high_confidence: Vec<_> = recommendations
            .iter()
            .filter(|(_, r)| r.confidence >= 0.9)
            .collect();
        let mut medium_confidence: Vec<_> = recommendations
            .iter()
            .filter(|(_, r)| r.confidence >= 0.7 && r.confidence < 0.9)
            .collect();
        let mut low_confidence: Vec<_> = recommendations
            .iter()
            .filter(|(_, r)| r.confidence < 0.7)
            .collect();

        // Sort each group by test name
        high_confidence.sort_by_key(|(k, _)| k.as_str());
        medium_confidence.sort_by_key(|(k, _)| k.as_str());
        low_confidence.sort_by_key(|(k, _)| k.as_str());

        if !high_confidence.is_empty() {
            report.push_str("## High Confidence (≥90%)\n\n");
            report.push_str("These updates are recommended for automatic approval:\n\n");
            for (test, rec) in high_confidence {
                report.push_str(&self.format_recommendation(test, rec));
            }
        }

        if !medium_confidence.is_empty() {
            report.push_str("## Medium Confidence (70-89%)\n\n");
            report.push_str("These updates should be reviewed before applying:\n\n");
            for (test, rec) in medium_confidence {
                report.push_str(&self.format_recommendation(test, rec));
            }
        }

        if !low_confidence.is_empty() {
            report.push_str("## Low Confidence (<70%)\n\n");
            report.push_str("These updates require careful review:\n\n");
            for (test, rec) in low_confidence {
                report.push_str(&self.format_recommendation(test, rec));
            }
        }

        report
    }

    /// Format a single recommendation for the report
    fn format_recommendation(&self, test: &str, rec: &BaselineRecommendation) -> String {
        format!(
            "### {}\n\n\
             - **Current**: {:.2}ms\n\
             - **Recommended**: {:.2}ms\n\
             - **Change**: {:.1}%\n\
             - **Confidence**: {:.0}%\n\
             - **Stability**: {:.0}%\n\
             - **Samples**: {}\n\
             - **Auto-update**: {}\n\
             - **Rationale**: {}\n\n",
            test,
            rec.current_baseline,
            rec.recommended_baseline,
            ((rec.recommended_baseline - rec.current_baseline) / rec.current_baseline) * 100.0,
            rec.confidence * 100.0,
            rec.stability_score * 100.0,
            rec.sample_count,
            if rec.should_auto_update {
                "✅ Yes"
            } else {
                "❌ No"
            },
            rec.rationale
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn create_test_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("gup_baseline_rec_{}", name));
        let _ = std::fs::create_dir_all(&dir);
        dir
    }

    #[test]
    fn test_recommendation_config_defaults() {
        let config = RecommendationConfig::default();
        assert_eq!(config.min_samples, 10);
        assert_eq!(config.min_change_threshold, 0.10);
        assert_eq!(config.min_confidence, 0.80);
        assert_eq!(config.max_cv_for_stability, 0.10);
        assert_eq!(config.auto_update_confidence, 0.90);
    }

    #[test]
    fn test_calculate_confidence() {
        let temp_dir = create_test_dir("confidence");
        let storage = BaselineStorage::new(temp_dir);
        let engine = BaselineRecommendationEngine::with_defaults(storage);

        let analysis = TrendAnalysis {
            test_name: "test".to_string(),
            category: "cat".to_string(),
            platform_id: "platform".to_string(),
            sample_count: 20, // 2x minimum
            current_baseline: 10.0,
            recent_average: 11.0,
            std_deviation: 0.5,
            coefficient_of_variation: 0.05, // 5% - very stable
            significant_shift: true,
            percent_change: 0.10, // 10% change
            is_stable: true,
        };

        let confidence = engine.calculate_confidence(&analysis);
        assert!(confidence > 0.8, "Expected high confidence: {}", confidence);
        assert!(
            confidence <= 1.0,
            "Confidence should not exceed 1.0: {}",
            confidence
        );
    }

    #[test]
    fn test_calculate_average() {
        let temp_dir = create_test_dir("average");
        let storage = BaselineStorage::new(temp_dir);
        let engine = BaselineRecommendationEngine::with_defaults(storage);

        let baselines = vec![
            PerformanceBaseline {
                test_name: "test".to_string(),
                category: "cat".to_string(),
                avg_frame_time_ms: 10.0,
                avg_memory_usage_bytes: 1000,
                sample_count: 1,
                last_updated: chrono::Utc::now(),
                metadata: HashMap::new(),
                platform_id: "platform".to_string(),
            },
            PerformanceBaseline {
                test_name: "test".to_string(),
                category: "cat".to_string(),
                avg_frame_time_ms: 12.0,
                avg_memory_usage_bytes: 1000,
                sample_count: 1,
                last_updated: chrono::Utc::now(),
                metadata: HashMap::new(),
                platform_id: "platform".to_string(),
            },
            PerformanceBaseline {
                test_name: "test".to_string(),
                category: "cat".to_string(),
                avg_frame_time_ms: 14.0,
                avg_memory_usage_bytes: 1000,
                sample_count: 1,
                last_updated: chrono::Utc::now(),
                metadata: HashMap::new(),
                platform_id: "platform".to_string(),
            },
        ];

        let avg = engine.calculate_average(&baselines);
        assert_eq!(avg, 12.0);
    }

    #[test]
    fn test_calculate_std_deviation() {
        let temp_dir = create_test_dir("stddev");
        let storage = BaselineStorage::new(temp_dir);
        let engine = BaselineRecommendationEngine::with_defaults(storage);

        let baselines = vec![
            PerformanceBaseline {
                test_name: "test".to_string(),
                category: "cat".to_string(),
                avg_frame_time_ms: 10.0,
                avg_memory_usage_bytes: 1000,
                sample_count: 1,
                last_updated: chrono::Utc::now(),
                metadata: HashMap::new(),
                platform_id: "platform".to_string(),
            },
            PerformanceBaseline {
                test_name: "test".to_string(),
                category: "cat".to_string(),
                avg_frame_time_ms: 12.0,
                avg_memory_usage_bytes: 1000,
                sample_count: 1,
                last_updated: chrono::Utc::now(),
                metadata: HashMap::new(),
                platform_id: "platform".to_string(),
            },
            PerformanceBaseline {
                test_name: "test".to_string(),
                category: "cat".to_string(),
                avg_frame_time_ms: 14.0,
                avg_memory_usage_bytes: 1000,
                sample_count: 1,
                last_updated: chrono::Utc::now(),
                metadata: HashMap::new(),
                platform_id: "platform".to_string(),
            },
        ];

        let avg = 12.0;
        let std_dev = engine.calculate_std_deviation(&baselines, avg);
        assert!(
            std_dev > 1.9 && std_dev < 2.1,
            "Expected ~2.0, got {}",
            std_dev
        );
    }

    #[test]
    fn test_recommendation_report_empty() {
        let temp_dir = create_test_dir("empty_report");
        let storage = BaselineStorage::new(temp_dir);
        let engine = BaselineRecommendationEngine::with_defaults(storage);
        let analyzer = BatchRecommendationAnalyzer::new(engine);

        let recommendations = HashMap::new();
        let report = analyzer.generate_recommendation_report(&recommendations);
        assert!(report.contains("No baseline updates recommended"));
    }
}

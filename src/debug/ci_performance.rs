// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! CI/CD integration for automated performance testing and regression detection.
//!
//! This module provides tools for integrating GPU debug capabilities into CI/CD pipelines,
//! including baseline management, trend analysis, and automated performance testing.

use crate::debug::{
    GpuDebugContext, PerformanceSnapshot, PerformanceSummary, PerformanceThresholds,
};
use crate::error::{GupError, GupResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// CI/CD performance testing runner
#[derive(Debug)]
pub struct CiPerformanceRunner {
    /// Debug context for profiling
    debug_context: GpuDebugContext,
    /// Storage for performance baselines
    baseline_storage: BaselineStorage,
    /// Configuration for CI testing
    config: CiConfig,
}

impl CiPerformanceRunner {
    /// Create a new CI performance runner
    pub fn new(debug_context: GpuDebugContext, config: CiConfig) -> Self {
        let baseline_storage = BaselineStorage::new(config.baseline_dir.clone());
        Self {
            debug_context,
            baseline_storage,
            config,
        }
    }

    /// Run the complete performance test suite
    pub async fn run_performance_suite(
        &mut self,
        test_suite: PerformanceTestSuite,
    ) -> GupResult<PerformanceReport> {
        let mut test_results = Vec::new();
        let start_time = chrono::Utc::now();

        for test in test_suite.tests {
            let result = self.run_single_test(&test).await?;
            test_results.push(result);
        }

        let end_time = chrono::Utc::now();
        let duration = (end_time - start_time).num_milliseconds() as u64;

        Ok(PerformanceReport {
            timestamp: start_time,
            duration_ms: duration,
            test_results,
            summary: self.debug_context.get_performance_summary(),
            config: self.config.clone(),
        })
    }

    /// Run a single performance test
    async fn run_single_test(&mut self, test: &PerformanceTest) -> GupResult<TestResult> {
        let start_time = std::time::Instant::now();

        // Execute the test function
        let snapshot = (test.test_fn)(&mut self.debug_context).await?;

        let elapsed = start_time.elapsed();

        // Check against baseline if available
        let baseline_comparison = if let Ok(baseline) = self
            .baseline_storage
            .load_baseline(&test.name, &test.category)
        {
            Some(self.compare_against_baseline(&snapshot, &baseline))
        } else {
            None
        };

        Ok(TestResult {
            test_name: test.name.clone(),
            category: test.category.clone(),
            snapshot,
            passed: baseline_comparison
                .as_ref()
                .map(|c| !c.is_regression)
                .unwrap_or(true),
            baseline_comparison,
            elapsed_ms: elapsed.as_millis() as u64,
        })
    }

    /// Compare current performance against baseline
    fn compare_against_baseline(
        &self,
        current: &PerformanceSnapshot,
        baseline: &PerformanceBaseline,
    ) -> BaselineComparison {
        let frame_time_delta_percent = if baseline.avg_frame_time_ms > 0.0 {
            ((current.frame_time_ms - baseline.avg_frame_time_ms) / baseline.avg_frame_time_ms
                * 100.0) as f64
        } else {
            0.0
        };

        let memory_delta_percent = if baseline.avg_memory_usage_bytes > 0 {
            (current.memory_usage_bytes as f64 - baseline.avg_memory_usage_bytes as f64)
                / baseline.avg_memory_usage_bytes as f64
                * 100.0
        } else {
            0.0
        };

        let is_regression = frame_time_delta_percent
            > self.config.thresholds.regression_threshold_percent.into()
            || memory_delta_percent > self.config.thresholds.regression_threshold_percent.into();

        BaselineComparison {
            baseline_frame_time_ms: baseline.avg_frame_time_ms,
            current_frame_time_ms: current.frame_time_ms,
            frame_time_delta_percent,
            baseline_memory_bytes: baseline.avg_memory_usage_bytes,
            current_memory_bytes: current.memory_usage_bytes,
            memory_delta_percent,
            is_regression,
        }
    }

    /// Check for performance regressions in a report
    pub fn check_regressions(&self, report: &PerformanceReport) -> Vec<PerformanceRegression> {
        let mut regressions = Vec::new();

        for result in &report.test_results {
            if let Some(comparison) = &result.baseline_comparison
                && comparison.is_regression {
                    regressions.push(PerformanceRegression {
                        test_name: result.test_name.clone(),
                        category: result.category.clone(),
                        frame_time_delta_percent: comparison.frame_time_delta_percent,
                        memory_delta_percent: comparison.memory_delta_percent,
                        severity: self.determine_regression_severity(comparison),
                    });
                }
        }

        regressions
    }

    /// Determine the severity of a regression
    fn determine_regression_severity(&self, comparison: &BaselineComparison) -> RegressionSeverity {
        let max_delta = comparison
            .frame_time_delta_percent
            .max(comparison.memory_delta_percent);

        let threshold = self.config.thresholds.regression_threshold_percent as f64;

        if max_delta > threshold * 3.0 {
            RegressionSeverity::Critical
        } else if max_delta > threshold * 2.0 {
            RegressionSeverity::High
        } else if max_delta > threshold {
            RegressionSeverity::Medium
        } else {
            RegressionSeverity::Low
        }
    }

    /// Update baselines with approved results
    pub fn update_baselines(&mut self, report: &PerformanceReport) -> GupResult<()> {
        for result in &report.test_results {
            let baseline = PerformanceBaseline {
                test_name: result.test_name.clone(),
                category: result.category.clone(),
                avg_frame_time_ms: result.snapshot.frame_time_ms,
                avg_memory_usage_bytes: result.snapshot.memory_usage_bytes,
                sample_count: 1,
                last_updated: chrono::Utc::now(),
                metadata: result.snapshot.metadata.clone(),
            };

            self.baseline_storage
                .save_baseline(&result.test_name, &result.category, &baseline)?;
        }

        Ok(())
    }

    /// Export performance report for CI artifacts
    pub fn export_report(&self, report: &PerformanceReport, output_path: &Path) -> GupResult<()> {
        let json = serde_json::to_string_pretty(report).map_err(|e| {
            GupError::validation_error(format!("Failed to serialize performance report: {e}"))
        })?;

        std::fs::write(output_path, json)
            .map_err(|e| GupError::resource_error(format!("Failed to write report: {e}")))?;

        Ok(())
    }

    /// Export performance report as Markdown for GitHub comments
    pub fn export_report_markdown(&self, report: &PerformanceReport) -> String {
        let mut md = String::new();
        md.push_str("# Performance Test Report\n\n");
        md.push_str(&format!(
            "**Timestamp**: {}\n\n",
            report.timestamp.format("%Y-%m-%d %H:%M:%S UTC")
        ));
        md.push_str(&format!("**Duration**: {}ms\n\n", report.duration_ms));

        // Summary
        md.push_str("## Summary\n\n");
        md.push_str(&format!("- **Tests Run**: {}\n", report.test_results.len()));
        let passed = report.test_results.iter().filter(|r| r.passed).count();
        md.push_str(&format!("- **Passed**: {}\n", passed));
        md.push_str(&format!(
            "- **Failed**: {}\n\n",
            report.test_results.len() - passed
        ));

        // Regressions
        let regressions = self.check_regressions(report);
        if !regressions.is_empty() {
            md.push_str("## ⚠️ Performance Regressions Detected\n\n");
            md.push_str("| Test | Severity | Frame Time Δ | Memory Δ |\n");
            md.push_str("|------|----------|--------------|----------|\n");

            for regression in &regressions {
                md.push_str(&format!(
                    "| {} | {:?} | {:+.1}% | {:+.1}% |\n",
                    regression.test_name,
                    regression.severity,
                    regression.frame_time_delta_percent,
                    regression.memory_delta_percent
                ));
            }
            md.push('\n');
        } else {
            md.push_str("## ✅ No Performance Regressions\n\n");
        }

        // Test Results
        md.push_str("## Test Results\n\n");
        md.push_str("| Test | Status | Frame Time | Memory | Elapsed |\n");
        md.push_str("|------|--------|------------|--------|----------|\n");

        for result in &report.test_results {
            let status = if result.passed { "✅" } else { "❌" };
            md.push_str(&format!(
                "| {} | {} | {:.2}ms | {}KB | {}ms |\n",
                result.test_name,
                status,
                result.snapshot.frame_time_ms,
                result.snapshot.memory_usage_bytes / 1024,
                result.elapsed_ms
            ));
        }

        md
    }
}

/// Configuration for CI/CD performance testing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CiConfig {
    /// Directory for storing performance baselines
    pub baseline_dir: PathBuf,
    /// Performance thresholds for regression detection
    pub thresholds: PerformanceThresholds,
    /// Whether to fail CI on regressions
    pub fail_on_regression: bool,
    /// Maximum test suite duration in seconds
    pub max_suite_duration_secs: u64,
}

impl Default for CiConfig {
    fn default() -> Self {
        Self {
            baseline_dir: PathBuf::from("baselines/performance"),
            thresholds: PerformanceThresholds::default(),
            fail_on_regression: true,
            max_suite_duration_secs: 300, // 5 minutes
        }
    }
}

/// Storage for performance baselines
#[derive(Debug)]
pub struct BaselineStorage {
    base_dir: PathBuf,
}

impl BaselineStorage {
    /// Create a new baseline storage
    pub fn new(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    /// Load a baseline from storage
    pub fn load_baseline(&self, test_name: &str, category: &str) -> GupResult<PerformanceBaseline> {
        let path = self.baseline_path(test_name, category);

        let json = std::fs::read_to_string(&path).map_err(|e| {
            GupError::resource_error(format!("Failed to load baseline from {path:?}: {e}"))
        })?;

        serde_json::from_str(&json).map_err(|e| {
            GupError::validation_error(format!("Failed to parse baseline from {path:?}: {e}"))
        })
    }

    /// Save a baseline to storage
    pub fn save_baseline(
        &self,
        test_name: &str,
        category: &str,
        baseline: &PerformanceBaseline,
    ) -> GupResult<()> {
        let path = self.baseline_path(test_name, category);

        // Ensure directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                GupError::resource_error(format!("Failed to create baseline directory: {e}"))
            })?;
        }

        let json = serde_json::to_string_pretty(baseline).map_err(|e| {
            GupError::validation_error(format!("Failed to serialize baseline: {e}"))
        })?;

        std::fs::write(&path, json)
            .map_err(|e| GupError::resource_error(format!("Failed to write baseline: {e}")))?;

        Ok(())
    }

    /// Get the path for a baseline file
    fn baseline_path(&self, test_name: &str, category: &str) -> PathBuf {
        self.base_dir
            .join(category)
            .join(format!("{}.json", test_name))
    }

    /// List all available baselines
    pub fn list_baselines(&self) -> GupResult<Vec<(String, String)>> {
        let mut baselines = Vec::new();

        if !self.base_dir.exists() {
            return Ok(baselines);
        }

        for category_entry in std::fs::read_dir(&self.base_dir).map_err(|e| {
            GupError::resource_error(format!("Failed to read baseline directory: {e}"))
        })? {
            let category_entry = category_entry.map_err(|e| {
                GupError::resource_error(format!("Failed to read category entry: {e}"))
            })?;

            if category_entry.path().is_dir() {
                let category = category_entry.file_name().to_string_lossy().to_string();

                for baseline_entry in std::fs::read_dir(category_entry.path()).map_err(|e| {
                    GupError::resource_error(format!("Failed to read baseline files: {e}"))
                })? {
                    let baseline_entry = baseline_entry.map_err(|e| {
                        GupError::resource_error(format!("Failed to read baseline entry: {e}"))
                    })?;

                    if let Some(file_name) = baseline_entry.file_name().to_str()
                        && file_name.ends_with(".json") {
                            let test_name = file_name.strip_suffix(".json").unwrap().to_string();
                            baselines.push((category.clone(), test_name));
                        }
                }
            }
        }

        Ok(baselines)
    }
}

/// Performance baseline for regression detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceBaseline {
    pub test_name: String,
    pub category: String,
    pub avg_frame_time_ms: f32,
    pub avg_memory_usage_bytes: u64,
    pub sample_count: usize,
    pub last_updated: chrono::DateTime<chrono::Utc>,
    pub metadata: HashMap<String, String>,
}

/// Complete performance report from a CI run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceReport {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub duration_ms: u64,
    pub test_results: Vec<TestResult>,
    pub summary: PerformanceSummary,
    pub config: CiConfig,
}

/// Result of a single performance test
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    pub test_name: String,
    pub category: String,
    pub snapshot: PerformanceSnapshot,
    pub baseline_comparison: Option<BaselineComparison>,
    pub elapsed_ms: u64,
    pub passed: bool,
}

/// Comparison between current performance and baseline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaselineComparison {
    pub baseline_frame_time_ms: f32,
    pub current_frame_time_ms: f32,
    pub frame_time_delta_percent: f64,
    pub baseline_memory_bytes: u64,
    pub current_memory_bytes: u64,
    pub memory_delta_percent: f64,
    pub is_regression: bool,
}

/// Performance regression detected in testing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceRegression {
    pub test_name: String,
    pub category: String,
    pub frame_time_delta_percent: f64,
    pub memory_delta_percent: f64,
    pub severity: RegressionSeverity,
}

/// Severity level of a performance regression
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RegressionSeverity {
    Low,
    Medium,
    High,
    Critical,
}

/// Test suite containing multiple performance tests
pub struct PerformanceTestSuite {
    pub name: String,
    pub tests: Vec<PerformanceTest>,
}

/// Individual performance test
pub struct PerformanceTest {
    pub name: String,
    pub category: String,
    pub test_fn: Box<
        dyn for<'a> Fn(
                &'a mut GpuDebugContext,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = GupResult<PerformanceSnapshot>> + Send + 'a>,
            > + Send
            + Sync,
    >,
}

impl PerformanceTestSuite {
    /// Create a new test suite
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            tests: Vec::new(),
        }
    }

    /// Add a test to the suite using a closure
    pub fn add_test<F>(
        mut self,
        name: impl Into<String>,
        category: impl Into<String>,
        test_fn: F,
    ) -> Self
    where
        F: for<'a> Fn(
                &'a mut GpuDebugContext,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = GupResult<PerformanceSnapshot>> + Send + 'a>,
            > + Send
            + Sync
            + 'static,
    {
        self.tests.push(PerformanceTest {
            name: name.into(),
            category: category.into(),
            test_fn: Box::new(test_fn),
        });
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_config() -> CiConfig {
        CiConfig {
            baseline_dir: PathBuf::from("/tmp/gup_test_baselines"),
            ..Default::default()
        }
    }

    #[test]
    fn test_ci_config_default() {
        let config = CiConfig::default();
        assert!(config.fail_on_regression);
        assert_eq!(config.max_suite_duration_secs, 300);
        assert_eq!(config.thresholds.regression_threshold_percent, 20.0);
    }

    #[test]
    fn test_baseline_storage_path() {
        let storage = BaselineStorage::new(PathBuf::from("/tmp/baselines"));
        let path = storage.baseline_path("test_foo", "rendering");
        assert_eq!(
            path,
            PathBuf::from("/tmp/baselines/rendering/test_foo.json")
        );
    }

    #[tokio::test]
    async fn test_baseline_comparison_no_regression() {
        let baseline = PerformanceBaseline {
            test_name: "test".to_string(),
            category: "test".to_string(),
            avg_frame_time_ms: 10.0,
            avg_memory_usage_bytes: 1000,
            sample_count: 1,
            last_updated: chrono::Utc::now(),
            metadata: HashMap::new(),
        };

        let snapshot = PerformanceSnapshot::new(10.5, 1050);

        // 5% increase should not be a regression (default threshold is 20%)
        let config = create_test_config();
        let context = crate::GupContext::new()
            .await
            .expect("Failed to create context");
        let debug_ctx = GpuDebugContext::new(&context.device, &context.queue);
        let runner = CiPerformanceRunner::new(debug_ctx, config);

        let comparison = runner.compare_against_baseline(&snapshot, &baseline);
        assert!(!comparison.is_regression);
    }

    #[tokio::test]
    async fn test_regression_severity_determination() {
        let config = create_test_config();
        let context = crate::GupContext::new()
            .await
            .expect("Failed to create context");
        let debug_ctx = GpuDebugContext::new(&context.device, &context.queue);
        let runner = CiPerformanceRunner::new(debug_ctx, config);

        // Low severity (< 20%)
        let comparison = BaselineComparison {
            baseline_frame_time_ms: 10.0,
            current_frame_time_ms: 11.0,
            frame_time_delta_percent: 10.0,
            baseline_memory_bytes: 1000,
            current_memory_bytes: 1100,
            memory_delta_percent: 10.0,
            is_regression: false,
        };
        assert_eq!(
            runner.determine_regression_severity(&comparison),
            RegressionSeverity::Low
        );

        // Medium severity (20-40%)
        let comparison = BaselineComparison {
            baseline_frame_time_ms: 10.0,
            current_frame_time_ms: 13.0,
            frame_time_delta_percent: 30.0,
            baseline_memory_bytes: 1000,
            current_memory_bytes: 1300,
            memory_delta_percent: 30.0,
            is_regression: true,
        };
        assert_eq!(
            runner.determine_regression_severity(&comparison),
            RegressionSeverity::Medium
        );

        // High severity (40-60%)
        let comparison = BaselineComparison {
            baseline_frame_time_ms: 10.0,
            current_frame_time_ms: 15.0,
            frame_time_delta_percent: 50.0,
            baseline_memory_bytes: 1000,
            current_memory_bytes: 1500,
            memory_delta_percent: 50.0,
            is_regression: true,
        };
        assert_eq!(
            runner.determine_regression_severity(&comparison),
            RegressionSeverity::High
        );

        // Critical severity (> 60%)
        let comparison = BaselineComparison {
            baseline_frame_time_ms: 10.0,
            current_frame_time_ms: 20.0,
            frame_time_delta_percent: 100.0,
            baseline_memory_bytes: 1000,
            current_memory_bytes: 2000,
            memory_delta_percent: 100.0,
            is_regression: true,
        };
        assert_eq!(
            runner.determine_regression_severity(&comparison),
            RegressionSeverity::Critical
        );
    }
}

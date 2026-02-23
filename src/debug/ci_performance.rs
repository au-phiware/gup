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
use wgpu::{Adapter, AdapterInfo};

/// GPU vendor for platform identification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GpuVendor {
    /// NVIDIA GPUs
    Nvidia,
    /// AMD GPUs
    Amd,
    /// Intel GPUs
    Intel,
    /// Apple Silicon GPUs
    Apple,
    /// Software/CPU fallback renderer
    Software,
    /// Unknown or other vendor
    Unknown,
}

impl From<u32> for GpuVendor {
    fn from(vendor_id: u32) -> Self {
        match vendor_id {
            0x10DE => GpuVendor::Nvidia,
            0x1002 => GpuVendor::Amd,
            0x8086 => GpuVendor::Intel,
            0x106B => GpuVendor::Apple,
            0x1414 | 0x5143 => GpuVendor::Software, // Microsoft Basic Render Driver, Qualcomm
            _ => GpuVendor::Unknown,
        }
    }
}

/// Platform information for multi-platform testing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformInfo {
    /// GPU vendor
    pub vendor: GpuVendor,
    /// GPU model/device name
    pub model: String,
    /// Driver version or description
    pub driver: String,
    /// Backend being used (Vulkan, Metal, DX12, etc.)
    pub backend: String,
    /// Platform identifier for baseline organization
    pub platform_id: String,
}

impl PlatformInfo {
    /// Detect platform information from wgpu adapter
    pub fn from_adapter(adapter: &Adapter) -> Self {
        let info = adapter.get_info();
        Self::from_adapter_info(&info)
    }

    /// Create platform info from adapter info
    pub fn from_adapter_info(info: &AdapterInfo) -> Self {
        let vendor = GpuVendor::from(info.vendor);
        let model = info.name.clone();
        let driver = info.driver.clone();
        let backend = format!("{:?}", info.backend);

        // Create a sanitized platform ID for file paths
        let platform_id = Self::create_platform_id(&vendor, &model);

        Self {
            vendor,
            model,
            driver,
            backend,
            platform_id,
        }
    }

    /// Create a filesystem-safe platform identifier
    fn create_platform_id(vendor: &GpuVendor, model: &str) -> String {
        let vendor_str = format!("{:?}", vendor).to_lowercase();

        // Sanitize model name for filesystem
        let model_str = model
            .to_lowercase()
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '_' })
            .collect::<String>();

        // Trim excessive underscores
        let model_str = model_str
            .split('_')
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("_");

        format!("{}_{}", vendor_str, model_str)
    }

    /// Get a human-readable platform description
    pub fn description(&self) -> String {
        format!("{:?} {} ({})", self.vendor, self.model, self.backend)
    }
}

/// CI/CD performance testing runner
#[derive(Debug)]
pub struct CiPerformanceRunner {
    /// Debug context for profiling
    debug_context: GpuDebugContext,
    /// Storage for performance baselines
    baseline_storage: BaselineStorage,
    /// Configuration for CI testing
    config: CiConfig,
    /// Platform information for multi-platform testing
    platform_info: Option<PlatformInfo>,
}

impl CiPerformanceRunner {
    /// Create a new CI performance runner
    pub fn new(debug_context: GpuDebugContext, config: CiConfig) -> Self {
        let baseline_storage = BaselineStorage::new(config.baseline_dir.clone());
        Self {
            debug_context,
            baseline_storage,
            config,
            platform_info: None,
        }
    }

    /// Set platform information for multi-platform testing
    pub fn with_platform_info(mut self, platform_info: PlatformInfo) -> Self {
        self.platform_info = Some(platform_info);
        self
    }

    /// Get the current platform identifier, or "default" if not set
    fn get_platform_id(&self) -> &str {
        self.platform_info
            .as_ref()
            .map(|p| p.platform_id.as_str())
            .unwrap_or("default")
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
            platform_info: self.platform_info.clone(),
        })
    }

    /// Run a single performance test
    async fn run_single_test(&mut self, test: &PerformanceTest) -> GupResult<TestResult> {
        let start_time = std::time::Instant::now();

        // Execute the test function
        let snapshot = (test.test_fn)(&mut self.debug_context).await?;

        let elapsed = start_time.elapsed();

        let platform_id = self.get_platform_id();

        // Check against baseline if available
        let baseline_comparison = if let Ok(baseline) =
            self.baseline_storage
                .load_baseline(&test.name, &test.category, platform_id)
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
                && comparison.is_regression
            {
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
        let platform_id = self.get_platform_id();

        for result in &report.test_results {
            let baseline = PerformanceBaseline {
                test_name: result.test_name.clone(),
                category: result.category.clone(),
                avg_frame_time_ms: result.snapshot.frame_time_ms,
                avg_memory_usage_bytes: result.snapshot.memory_usage_bytes,
                sample_count: 1,
                last_updated: chrono::Utc::now(),
                metadata: result.snapshot.metadata.clone(),
                platform_id: platform_id.to_string(),
            };

            self.baseline_storage.save_baseline(
                &result.test_name,
                &result.category,
                platform_id,
                &baseline,
            )?;
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

        // Platform info
        if let Some(platform_info) = &report.platform_info {
            md.push_str(&format!(
                "**Platform**: {}\n\n",
                platform_info.description()
            ));
        }

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

    /// Load a baseline from storage for a specific platform
    pub fn load_baseline(
        &self,
        test_name: &str,
        category: &str,
        platform_id: &str,
    ) -> GupResult<PerformanceBaseline> {
        let path = self.baseline_path(test_name, category, platform_id);

        let json = std::fs::read_to_string(&path).map_err(|e| {
            GupError::resource_error(format!("Failed to load baseline from {path:?}: {e}"))
        })?;

        serde_json::from_str(&json).map_err(|e| {
            GupError::validation_error(format!("Failed to parse baseline from {path:?}: {e}"))
        })
    }

    /// Save a baseline to storage for a specific platform
    pub fn save_baseline(
        &self,
        test_name: &str,
        category: &str,
        platform_id: &str,
        baseline: &PerformanceBaseline,
    ) -> GupResult<()> {
        let path = self.baseline_path(test_name, category, platform_id);

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
    fn baseline_path(&self, test_name: &str, category: &str, platform_id: &str) -> PathBuf {
        self.base_dir
            .join(platform_id)
            .join(category)
            .join(format!("{}.json", test_name))
    }

    /// List all available baselines (returns (platform_id, category, test_name) tuples)
    pub fn list_baselines(&self) -> GupResult<Vec<(String, String, String)>> {
        let mut baselines = Vec::new();

        if !self.base_dir.exists() {
            return Ok(baselines);
        }

        // Iterate through platform directories
        for platform_entry in std::fs::read_dir(&self.base_dir).map_err(|e| {
            GupError::resource_error(format!("Failed to read baseline directory: {e}"))
        })? {
            let platform_entry = platform_entry.map_err(|e| {
                GupError::resource_error(format!("Failed to read platform entry: {e}"))
            })?;

            if platform_entry.path().is_dir() {
                let platform_id = platform_entry.file_name().to_string_lossy().to_string();

                // Iterate through category directories within each platform
                for category_entry in std::fs::read_dir(platform_entry.path()).map_err(|e| {
                    GupError::resource_error(format!("Failed to read category directory: {e}"))
                })? {
                    let category_entry = category_entry.map_err(|e| {
                        GupError::resource_error(format!("Failed to read category entry: {e}"))
                    })?;

                    if category_entry.path().is_dir() {
                        let category = category_entry.file_name().to_string_lossy().to_string();

                        // Iterate through baseline files
                        for baseline_entry in
                            std::fs::read_dir(category_entry.path()).map_err(|e| {
                                GupError::resource_error(format!(
                                    "Failed to read baseline files: {e}"
                                ))
                            })?
                        {
                            let baseline_entry = baseline_entry.map_err(|e| {
                                GupError::resource_error(format!(
                                    "Failed to read baseline entry: {e}"
                                ))
                            })?;

                            if let Some(file_name) = baseline_entry.file_name().to_str()
                                && file_name.ends_with(".json")
                            {
                                let test_name =
                                    file_name.strip_suffix(".json").unwrap().to_string();
                                baselines.push((platform_id.clone(), category.clone(), test_name));
                            }
                        }
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
    pub platform_id: String,
}

/// Complete performance report from a CI run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceReport {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub duration_ms: u64,
    pub test_results: Vec<TestResult>,
    pub summary: PerformanceSummary,
    pub config: CiConfig,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub platform_info: Option<PlatformInfo>,
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

/// Type alias for the complex async test function type
type AsyncTestFn = Box<
    dyn for<'a> Fn(
            &'a mut GpuDebugContext,
        ) -> std::pin::Pin<
            Box<dyn std::future::Future<Output = GupResult<PerformanceSnapshot>> + Send + 'a>,
        > + Send
        + Sync,
>;

/// Individual performance test
pub struct PerformanceTest {
    pub name: String,
    pub category: String,
    pub test_fn: AsyncTestFn,
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

/// Cross-platform performance comparison helper
pub struct CrossPlatformComparison {
    /// Reports from different platforms
    reports: Vec<PerformanceReport>,
}

impl CrossPlatformComparison {
    /// Create a new cross-platform comparison
    pub fn new() -> Self {
        Self {
            reports: Vec::new(),
        }
    }

    /// Add a report for comparison
    pub fn add_report(mut self, report: PerformanceReport) -> Self {
        self.reports.push(report);
        self
    }

    /// Generate a cross-platform comparison report
    pub fn generate_markdown_report(&self) -> String {
        if self.reports.is_empty() {
            return "No reports available for comparison".to_string();
        }

        let mut md = String::new();
        md.push_str("# Cross-Platform Performance Comparison\n\n");

        // Platform summary
        md.push_str("## Platforms Tested\n\n");
        for report in &self.reports {
            if let Some(platform_info) = &report.platform_info {
                md.push_str(&format!("- {}\n", platform_info.description()));
            } else {
                md.push_str("- Unknown platform\n");
            }
        }
        md.push('\n');

        // Collect all unique test names
        let mut test_names = std::collections::HashSet::new();
        for report in &self.reports {
            for result in &report.test_results {
                test_names.insert(result.test_name.clone());
            }
        }

        let mut test_names: Vec<_> = test_names.into_iter().collect();
        test_names.sort();

        // Create comparison table for each test
        md.push_str("## Performance by Platform\n\n");
        md.push_str("| Test | ");
        for report in &self.reports {
            if let Some(platform_info) = &report.platform_info {
                md.push_str(&format!("{:?} | ", platform_info.vendor));
            } else {
                md.push_str("Unknown | ");
            }
        }
        md.push('\n');

        md.push_str("|------|");
        for _ in &self.reports {
            md.push_str("--------|");
        }
        md.push('\n');

        for test_name in &test_names {
            md.push_str(&format!("| {} | ", test_name));
            for report in &self.reports {
                if let Some(result) = report
                    .test_results
                    .iter()
                    .find(|r| r.test_name == *test_name)
                {
                    md.push_str(&format!("{:.2}ms | ", result.snapshot.frame_time_ms));
                } else {
                    md.push_str("N/A | ");
                }
            }
            md.push('\n');
        }

        md.push('\n');

        // Find performance deltas across platforms
        md.push_str("## Performance Variations\n\n");
        for test_name in &test_names {
            let mut times: Vec<f32> = Vec::new();
            for report in &self.reports {
                if let Some(result) = report
                    .test_results
                    .iter()
                    .find(|r| r.test_name == *test_name)
                {
                    times.push(result.snapshot.frame_time_ms);
                }
            }

            if !times.is_empty() {
                let min_time = times
                    .iter()
                    .min_by(|a, b| a.partial_cmp(b).unwrap())
                    .unwrap();
                let max_time = times
                    .iter()
                    .max_by(|a, b| a.partial_cmp(b).unwrap())
                    .unwrap();

                let variation = ((max_time - min_time) / min_time * 100.0).abs();
                if variation > 10.0 {
                    md.push_str(&format!(
                        "- **{}**: {:.1}% variation ({:.2}ms - {:.2}ms)\n",
                        test_name, variation, min_time, max_time
                    ));
                }
            }
        }

        md
    }
}

impl Default for CrossPlatformComparison {
    fn default() -> Self {
        Self::new()
    }
}

//==============================================================================
// Performance Trend Visualization
//==============================================================================

/// Performance trend visualizer for CI/CD pipelines
///
/// Generates visual trend charts from historical performance baseline data,
/// useful for understanding performance evolution over time and identifying
/// gradual performance degradation.
pub struct PerformanceTrendVisualizer {
    baseline_storage: BaselineStorage,
}

impl PerformanceTrendVisualizer {
    /// Create a new trend visualizer
    pub fn new(baseline_dir: PathBuf) -> Self {
        Self {
            baseline_storage: BaselineStorage::new(baseline_dir),
        }
    }

    /// Generate trend charts for all available baselines
    ///
    /// Returns a map of test names to their SVG chart representations
    pub fn generate_all_trend_charts(&self) -> GupResult<HashMap<String, String>> {
        let mut charts = HashMap::new();

        let baselines = self.baseline_storage.list_baselines()?;

        // Group baselines by test name across all platforms and categories
        let mut test_baselines: HashMap<String, Vec<(String, PerformanceBaseline)>> =
            HashMap::new();

        for (platform_id, category, test_name) in baselines {
            let baseline = self
                .baseline_storage
                .load_baseline(&test_name, &category, &platform_id)?;

            test_baselines
                .entry(test_name.clone())
                .or_default()
                .push((platform_id, baseline));
        }

        // Generate a chart for each test
        for (test_name, baselines_vec) in test_baselines {
            // Sort by timestamp
            let mut baselines_vec = baselines_vec;
            baselines_vec.sort_by_key(|(_, b)| b.last_updated);

            let svg = self.generate_trend_chart(&test_name, &baselines_vec)?;
            charts.insert(test_name, svg);
        }

        Ok(charts)
    }

    /// Generate a trend chart for a specific test
    pub fn generate_trend_chart(
        &self,
        test_name: &str,
        baselines: &[(String, PerformanceBaseline)],
    ) -> GupResult<String> {
        if baselines.is_empty() {
            return Err(GupError::validation_error(format!(
                "No baseline data available for test: {}",
                test_name
            )));
        }

        // Convert baselines to PerformanceSnapshot format for rendering
        let snapshots: Vec<PerformanceSnapshot> = baselines
            .iter()
            .map(|(_, baseline)| PerformanceSnapshot {
                timestamp: baseline.last_updated,
                frame_time_ms: baseline.avg_frame_time_ms,
                memory_usage_bytes: baseline.avg_memory_usage_bytes,
                gpu_utilization_percent: 0.0, // Not tracked in baselines
                query_time_us: 0.0,            // Not tracked in baselines
                metadata: baseline.metadata.clone(),
            })
            .collect();

        // Generate SVG directly
        let title = format!("Performance Trend: {}", test_name);
        Ok(generate_performance_trend_svg(&snapshots, &title, 800, 600))
    }

    /// Export trend charts to files
    pub fn export_charts_to_directory(&self, output_dir: &Path) -> GupResult<Vec<PathBuf>> {
        std::fs::create_dir_all(output_dir).map_err(|e| {
            GupError::resource_error(format!("Failed to create output directory: {e}"))
        })?;

        let charts = self.generate_all_trend_charts()?;
        let mut paths = Vec::new();

        for (test_name, svg_content) in charts {
            let file_name = format!("{}.svg", test_name.replace(' ', "_"));
            let path = output_dir.join(&file_name);

            std::fs::write(&path, svg_content).map_err(|e| {
                GupError::resource_error(format!("Failed to write chart file {}: {e}", file_name))
            })?;

            paths.push(path);
        }

        Ok(paths)
    }

    /// Generate a summary dashboard HTML with all trend charts
    pub fn generate_dashboard_html(&self) -> GupResult<String> {
        let charts = self.generate_all_trend_charts()?;

        let mut html = String::from(
            r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Performance Trend Dashboard</title>
    <style>
        body {
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif;
            margin: 0;
            padding: 20px;
            background-color: #f5f5f5;
        }
        .container {
            max-width: 1400px;
            margin: 0 auto;
            background-color: white;
            padding: 30px;
            border-radius: 8px;
            box-shadow: 0 2px 8px rgba(0,0,0,0.1);
        }
        h1 {
            color: #333;
            border-bottom: 3px solid #4CAF50;
            padding-bottom: 10px;
        }
        .chart-grid {
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(800px, 1fr));
            gap: 30px;
            margin-top: 30px;
        }
        .chart-container {
            border: 1px solid #ddd;
            border-radius: 4px;
            padding: 20px;
            background-color: #fafafa;
        }
        .chart-container h2 {
            margin-top: 0;
            color: #555;
            font-size: 18px;
        }
        .timestamp {
            color: #888;
            font-size: 14px;
            margin-top: 10px;
        }
    </style>
</head>
<body>
    <div class="container">
        <h1>📊 Performance Trend Dashboard</h1>
        <p class="timestamp">Generated: "#,
        );

        html.push_str(&format!(
            "{}</p>\n        <p>Total tests tracked: {}</p>\n",
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC"),
            charts.len()
        ));

        html.push_str(r#"        <div class="chart-grid">"#);
        html.push('\n');

        let mut test_names: Vec<_> = charts.keys().collect();
        test_names.sort();

        for test_name in test_names {
            let svg_content = &charts[test_name];

            html.push_str(&format!(
                r#"            <div class="chart-container">
                <h2>{}</h2>
                {}
            </div>
"#,
                test_name, svg_content
            ));
        }

        html.push_str(
            r#"        </div>
    </div>
</body>
</html>"#,
        );

        Ok(html)
    }

    /// Export dashboard HTML to a file
    pub fn export_dashboard(&self, output_path: &Path) -> GupResult<()> {
        let html = self.generate_dashboard_html()?;

        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                GupError::resource_error(format!("Failed to create output directory: {e}"))
            })?;
        }

        std::fs::write(output_path, html).map_err(|e| {
            GupError::resource_error(format!("Failed to write dashboard HTML: {e}"))
        })?;

        Ok(())
    }
}

/// Generate an SVG performance trend chart (internal helper)
fn generate_performance_trend_svg(
    snapshots: &[PerformanceSnapshot],
    title: &str,
    width: u32,
    height: u32,
) -> String {
    if snapshots.is_empty() {
        return String::from("<svg></svg>");
    }

    let margin = 60.0;
    let chart_width = width as f64 - 2.0 * margin;
    let chart_height = height as f64 - 2.0 * margin;

    // Calculate data ranges
    let frame_times: Vec<f32> = snapshots.iter().map(|s| s.frame_time_ms).collect();
    let min_frame = frame_times.iter().fold(f32::INFINITY, |a, &b| a.min(b));
    let max_frame = frame_times.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));
    let frame_range = max_frame - min_frame;

    // Generate SVG
    let mut svg = String::new();
    svg.push_str(&format!(
        r#"<svg width="{}" height="{}" xmlns="http://www.w3.org/2000/svg">"#,
        width, height
    ));

    // Background
    svg.push_str(&format!(
        r#"<rect width="{}" height="{}" fill="white"/>"#,
        width, height
    ));

    // Title
    svg.push_str(&format!(
        r#"<text x="{}" y="30" font-size="20" font-weight="bold" text-anchor="middle" fill="black">{}</text>"#,
        width as f64 / 2.0,
        title
    ));

    // Chart area border
    svg.push_str(&format!(
        r#"<rect x="{}" y="{}" width="{}" height="{}" fill="none" stroke="gray" stroke-width="1"/>"#,
        margin, margin, chart_width, chart_height
    ));

    // Y-axis labels and grid lines
    for i in 0..=5 {
        let y = margin + (i as f64 / 5.0) * chart_height;
        let value = max_frame - (i as f32 / 5.0) * frame_range;

        // Grid line
        svg.push_str(&format!(
            r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="lightgray" stroke-width="1" stroke-dasharray="2,2"/>"#,
            margin,
            y,
            margin + chart_width,
            y
        ));

        // Label
        svg.push_str(&format!(
            r#"<text x="{}" y="{}" font-size="12" text-anchor="end" alignment-baseline="middle" fill="black">{:.2}ms</text>"#,
            margin - 10.0,
            y,
            value
        ));
    }

    // X-axis label
    svg.push_str(&format!(
        r#"<text x="{}" y="{}" font-size="14" text-anchor="middle" fill="black">Time</text>"#,
        margin + chart_width / 2.0,
        height as f64 - 10.0
    ));

    // Y-axis label
    svg.push_str(&format!(
        r#"<text x="20" y="{}" font-size="14" text-anchor="middle" transform="rotate(-90, 20, {})" fill="black">Frame Time (ms)</text>"#,
        margin + chart_height / 2.0,
        margin + chart_height / 2.0
    ));

    // Plot data points and line
    if snapshots.len() > 1 {
        let mut path = String::from("M");

        for (i, snapshot) in snapshots.iter().enumerate() {
            let x = margin + (i as f64 / (snapshots.len() - 1) as f64) * chart_width;
            let normalized = if frame_range > 0.0 {
                (snapshot.frame_time_ms - min_frame) / frame_range
            } else {
                0.5
            };
            let y = margin + chart_height - (normalized as f64 * chart_height);

            if i == 0 {
                path.push_str(&format!(" {} {}", x, y));
            } else {
                path.push_str(&format!(" L {} {}", x, y));
            }

            // Data point circle
            svg.push_str(&format!(
                r#"<circle cx="{}" cy="{}" r="3" fill="steelblue"/>"#,
                x, y
            ));
        }

        // Line
        svg.push_str(&format!(
            r#"<path d="{}" fill="none" stroke="steelblue" stroke-width="2"/>"#,
            path
        ));
    }

    // Legend
    svg.push_str(&format!(
        r#"<text x="{}" y="{}" font-size="12" fill="gray">Data points: {}</text>"#,
        margin,
        height as f64 - margin + 40.0,
        snapshots.len()
    ));

    svg.push_str("</svg>");
    svg
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
        let path = storage.baseline_path("test_foo", "rendering", "default");
        assert_eq!(
            path,
            PathBuf::from("/tmp/baselines/default/rendering/test_foo.json")
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
            platform_id: "default".to_string(),
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

    #[test]
    fn test_performance_trend_visualizer() {
        // Test creating visualizer
        let visualizer = PerformanceTrendVisualizer::new(PathBuf::from("/tmp/test_baselines"));

        // Test generating empty charts (should return error for empty data)
        let result = visualizer.generate_all_trend_charts();
        // Empty baselines directory should return empty map, not error
        assert!(result.is_ok());
    }

    #[test]
    fn test_generate_performance_trend_svg() {
        use std::collections::HashMap;

        let snapshots = vec![
            PerformanceSnapshot {
                timestamp: chrono::Utc::now(),
                frame_time_ms: 10.0,
                memory_usage_bytes: 1000,
                gpu_utilization_percent: 50.0,
                query_time_us: 100.0,
                metadata: HashMap::new(),
            },
            PerformanceSnapshot {
                timestamp: chrono::Utc::now(),
                frame_time_ms: 12.0,
                memory_usage_bytes: 1100,
                gpu_utilization_percent: 55.0,
                query_time_us: 110.0,
                metadata: HashMap::new(),
            },
            PerformanceSnapshot {
                timestamp: chrono::Utc::now(),
                frame_time_ms: 11.0,
                memory_usage_bytes: 1050,
                gpu_utilization_percent: 52.0,
                query_time_us: 105.0,
                metadata: HashMap::new(),
            },
        ];

        let svg = generate_performance_trend_svg(&snapshots, "Test Chart", 800, 600);

        // Verify SVG structure
        assert!(svg.contains("<svg"));
        assert!(svg.contains("</svg>"));
        assert!(svg.contains("Test Chart"));
        assert!(svg.contains("Frame Time (ms)"));
        assert!(svg.contains("Data points: 3"));
        assert!(svg.contains("<path"));
        assert!(svg.contains("<circle"));
    }

    #[test]
    fn test_generate_performance_trend_svg_empty() {
        let snapshots: Vec<PerformanceSnapshot> = vec![];
        let svg = generate_performance_trend_svg(&snapshots, "Empty Chart", 800, 600);

        // Should return minimal SVG
        assert!(svg.contains("<svg"));
        assert!(svg.contains("</svg>"));
    }
}

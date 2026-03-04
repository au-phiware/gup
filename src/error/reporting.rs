// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Error reporting and telemetry system for comprehensive error tracking.

use std::collections::HashMap;
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};

use super::{ErrorCategory, ErrorContext, ErrorSeverity, GupError, GupResult, RecoveryAttempt};
use crate::{MaybeSend, MaybeSync};

/// Main error reporting system with aggregation and rate limiting.
#[derive(Debug)]
pub struct ErrorReporter {
    error_sink: Box<dyn ErrorSink>,
    aggregator: ErrorAggregator,
    rate_limiter: RateLimiter,
    config: ReportingConfig,
}

/// Configuration for error reporting behavior.
#[derive(Debug, Clone)]
pub struct ReportingConfig {
    /// Whether telemetry reporting is enabled.
    pub enable_telemetry: bool,
    /// Maximum number of error reports allowed per minute.
    pub max_reports_per_minute: usize,
    /// Number of reports to batch before sending.
    pub batch_size: usize,
    /// Number of retry attempts for failed report delivery.
    pub retry_attempts: usize,
    /// Time window for aggregating similar errors.
    pub aggregation_window: Duration,
}

/// Trait for error reporting destinations.
pub trait ErrorSink: std::fmt::Debug + MaybeSend + MaybeSync {
    /// Send an error report to the configured destination.
    fn send_error(&self, context: &ErrorContext) -> GupResult<()>;

    /// Send a batch of error reports.
    fn send_batch(&self, contexts: &[ErrorContext]) -> GupResult<()> {
        for context in contexts {
            self.send_error(context)?;
        }
        Ok(())
    }

    /// Check if the sink is available for sending.
    fn is_available(&self) -> bool {
        true
    }
}

/// Aggregates similar errors to reduce noise.
#[derive(Debug)]
pub struct ErrorAggregator {
    error_groups: HashMap<ErrorSignature, AggregatedError>,
    aggregation_window: Duration,
    last_cleanup: SystemTime,
}

/// Unique signature for grouping similar errors.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ErrorSignature {
    /// Error category for grouping.
    pub category: ErrorCategory,
    /// Severity level of the error.
    pub severity: ErrorSeverity,
    /// Discriminant-based error type identifier.
    pub error_type: String,
    /// Key context extracted from the error for deduplication.
    pub key_context: String,
}

/// Aggregated error information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedError {
    /// Unique signature identifying this error group.
    pub signature: ErrorSignature,
    /// Number of occurrences of this error.
    pub count: usize,
    /// Timestamp of the first occurrence.
    pub first_seen: SystemTime,
    /// Timestamp of the most recent occurrence.
    pub last_seen: SystemTime,
    /// Sample error contexts for analysis.
    pub sample_contexts: Vec<ErrorContext>,
    /// Names of systems affected by this error.
    pub affected_systems: Vec<String>,
}

/// Rate limiter to prevent error report spam.
#[derive(Debug)]
pub struct RateLimiter {
    error_counts: HashMap<ErrorSignature, usize>,
    window_start: SystemTime,
    window_duration: Duration,
    max_reports_per_window: usize,
}

/// Comprehensive error report with aggregated data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorReport {
    /// Unique identifier for this report.
    pub report_id: uuid::Uuid,
    /// Timestamp when the report was generated.
    pub timestamp: SystemTime,
    /// Context of the reported error.
    pub error_context: ErrorContext,
    /// Number of times this error occurred.
    pub frequency: usize,
    /// Timestamp of the first occurrence.
    pub first_occurrence: SystemTime,
    /// Timestamp of the last occurrence.
    pub last_occurrence: SystemTime,
    /// Recovery attempts made for this error.
    pub recovery_attempts: Vec<RecoveryAttempt>,
    /// Assessment of system impact.
    pub system_impact: SystemImpact,
    /// Recommended actions to address this error.
    pub recommendations: Vec<String>,
}

/// System impact assessment for errors.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemImpact {
    /// Estimated performance degradation as a fraction.
    pub performance_degradation: f32,
    /// Impact on user experience.
    pub user_experience_impact: ImpactLevel,
    /// Risk to system stability.
    pub stability_risk: ImpactLevel,
    /// Risk to data integrity.
    pub data_integrity_risk: ImpactLevel,
    /// Names of components affected by this error.
    pub affected_components: Vec<String>,
}

/// Impact severity levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImpactLevel {
    /// No measurable impact.
    None,
    /// Minor impact unlikely to be noticed.
    Low,
    /// Moderate impact that may affect some workflows.
    Medium,
    /// Significant impact affecting core functionality.
    High,
    /// Severe impact requiring immediate attention.
    Critical,
}

/// Summary of errors over a time period.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorSummary {
    /// Duration of the analysis window.
    pub time_window: Duration,
    /// Total number of errors in the window.
    pub total_errors: usize,
    /// Error counts grouped by category.
    pub error_categories: HashMap<ErrorCategory, usize>,
    /// Error counts grouped by severity.
    pub severity_distribution: HashMap<ErrorSeverity, usize>,
    /// Most frequently occurring error, if any.
    pub most_frequent: Option<AggregatedError>,
    /// Ratio of successful recoveries to total recoverable errors.
    pub recovery_success_rate: f32,
    /// System-level recommendations based on error patterns.
    pub recommendations: Vec<SystemRecommendation>,
    /// Trend analysis for error metrics.
    pub trends: ErrorTrends,
}

/// System-level recommendations based on error patterns.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemRecommendation {
    /// Category of the recommendation.
    pub recommendation_type: RecommendationType,
    /// Priority level for this recommendation.
    pub priority: RecommendationPriority,
    /// Human-readable description of the recommendation.
    pub description: String,
    /// Estimated impact as a fraction if the recommendation is applied.
    pub estimated_impact: f32,
    /// Estimated effort to implement the recommendation.
    pub implementation_effort: EffortLevel,
}

/// Types of system recommendations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecommendationType {
    /// Adjust configuration settings.
    ConfigurationChange,
    /// Upgrade hardware or system resources.
    ResourceUpgrade,
    /// Optimise application code paths.
    CodeOptimization,
    /// Add or improve monitoring and alerting.
    MonitoringImprovement,
    /// Provide guidance or documentation to end users.
    UserEducation,
    /// Perform routine system maintenance.
    SystemMaintenance,
}

/// Priority levels for recommendations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RecommendationPriority {
    /// Low-priority recommendation.
    Low,
    /// Medium-priority recommendation.
    Medium,
    /// High-priority recommendation.
    High,
    /// Critical-priority recommendation requiring immediate action.
    Critical,
}

/// Effort levels for implementing recommendations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EffortLevel {
    /// Trivial change requiring almost no effort.
    Minimal,
    /// Small change with low effort.
    Low,
    /// Moderate change requiring some planning.
    Medium,
    /// Large change requiring significant work.
    High,
    /// Major undertaking requiring substantial resources.
    Significant,
}

/// Error trend analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorTrends {
    /// Trend direction for overall error rate.
    pub error_rate_trend: TrendDirection,
    /// Trend direction for error severity.
    pub severity_trend: TrendDirection,
    /// Trend direction for recovery success rate.
    pub recovery_rate_trend: TrendDirection,
    /// Number of newly observed error types.
    pub new_error_types: usize,
    /// Number of error types no longer occurring.
    pub resolved_error_types: usize,
}

/// Trend direction indicators.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrendDirection {
    /// Metric is decreasing over time.
    Decreasing,
    /// Metric is stable over time.
    Stable,
    /// Metric is increasing over time.
    Increasing,
    /// Metric is increasing rapidly over time.
    RapidlyIncreasing,
}

impl Default for ReportingConfig {
    fn default() -> Self {
        Self {
            enable_telemetry: true,
            max_reports_per_minute: 100,
            batch_size: 10,
            retry_attempts: 3,
            aggregation_window: Duration::from_secs(300), // 5 minutes
        }
    }
}

impl ErrorReporter {
    /// Create a new error reporter with console logging.
    pub fn new() -> Self {
        Self::with_sink(Box::new(ConsoleErrorSink::new()))
    }

    /// Create an error reporter with a custom error sink.
    pub fn with_sink(sink: Box<dyn ErrorSink>) -> Self {
        let config = ReportingConfig::default();
        Self {
            error_sink: sink,
            aggregator: ErrorAggregator::new(config.aggregation_window),
            rate_limiter: RateLimiter::new(Duration::from_secs(60), config.max_reports_per_minute),
            config,
        }
    }

    /// Create an error reporter with custom configuration.
    pub fn with_config(sink: Box<dyn ErrorSink>, config: ReportingConfig) -> Self {
        Self {
            aggregator: ErrorAggregator::new(config.aggregation_window),
            rate_limiter: RateLimiter::new(Duration::from_secs(60), config.max_reports_per_minute),
            error_sink: sink,
            config,
        }
    }

    /// Report an error with context.
    pub fn report_error(&mut self, context: ErrorContext) {
        let signature = ErrorSignature::from_context(&context);

        // Check rate limiting
        if !self.rate_limiter.should_report(&signature) {
            log::debug!("Error report rate limited: {signature:?}");
            return;
        }

        // Aggregate similar errors
        self.aggregator.add_error(context.clone());

        // Send to error sink if telemetry is enabled
        if self.config.enable_telemetry
            && self.error_sink.is_available()
            && let Err(sink_error) = self.error_sink.send_error(&context)
        {
            log::error!("Failed to report error: {sink_error}");
        }

        // Log locally based on severity
        self.log_error(&context);
    }

    /// Generate comprehensive error summary for a time window.
    pub fn generate_error_summary(&self, time_window: Duration) -> ErrorSummary {
        let cutoff = SystemTime::now() - time_window;
        let recent_errors = self.aggregator.get_errors_since(cutoff);

        let total_errors = recent_errors.iter().map(|e| e.count).sum();

        let error_categories = self.categorize_errors(&recent_errors);
        let severity_distribution = self.analyze_severity_distribution(&recent_errors);
        let most_frequent = self.find_most_frequent_error(&recent_errors);
        let recovery_success_rate = self.calculate_recovery_rate(&recent_errors);
        let recommendations = self.generate_system_recommendations(&recent_errors);
        let trends = self.analyze_error_trends(&recent_errors);

        ErrorSummary {
            time_window,
            total_errors,
            error_categories,
            severity_distribution,
            most_frequent,
            recovery_success_rate,
            recommendations,
            trends,
        }
    }

    /// Get current error statistics.
    pub fn error_stats(&self) -> ErrorStats {
        ErrorStats {
            total_aggregated_errors: self.aggregator.error_groups.len(),
            reports_in_current_window: self.rate_limiter.current_window_count(),
            last_report_time: self.aggregator.last_error_time(),
            sink_available: self.error_sink.is_available(),
        }
    }

    fn log_error(&self, context: &ErrorContext) {
        match context.error.severity() {
            ErrorSeverity::Critical => {
                log::error!("CRITICAL: {} ({})", context.error, context.error_id);
            }
            ErrorSeverity::High => {
                log::error!("HIGH: {} ({})", context.error, context.error_id);
            }
            ErrorSeverity::Medium => {
                log::warn!("MEDIUM: {} ({})", context.error, context.error_id);
            }
            ErrorSeverity::Low => {
                log::info!("LOW: {} ({})", context.error, context.error_id);
            }
        }
    }

    fn categorize_errors(&self, errors: &[AggregatedError]) -> HashMap<ErrorCategory, usize> {
        let mut categories = HashMap::new();
        for error in errors {
            *categories.entry(error.signature.category).or_insert(0) += error.count;
        }
        categories
    }

    fn analyze_severity_distribution(
        &self,
        errors: &[AggregatedError],
    ) -> HashMap<ErrorSeverity, usize> {
        let mut distribution = HashMap::new();
        for error in errors {
            *distribution.entry(error.signature.severity).or_insert(0) += error.count;
        }
        distribution
    }

    fn find_most_frequent_error(&self, errors: &[AggregatedError]) -> Option<AggregatedError> {
        errors.iter().max_by_key(|e| e.count).cloned()
    }

    fn calculate_recovery_rate(&self, errors: &[AggregatedError]) -> f32 {
        let total_recoverable = errors
            .iter()
            .filter(|e| {
                e.sample_contexts
                    .iter()
                    .any(|ctx| ctx.error.is_recoverable())
            })
            .map(|e| e.count)
            .sum::<usize>();

        if total_recoverable == 0 {
            return 1.0; // No recoverable errors means 100% success
        }

        // In a real implementation, this would track actual recovery attempts
        // For now, we use a heuristic based on error types
        let estimated_successful_recoveries = errors
            .iter()
            .filter(|e| {
                e.sample_contexts
                    .iter()
                    .any(|ctx| ctx.can_auto_recover())
            })
            .map(|e| (e.count as f32 * 0.8) as usize) // Assume 80% success rate
            .sum::<usize>();

        estimated_successful_recoveries as f32 / total_recoverable as f32
    }

    fn generate_system_recommendations(
        &self,
        errors: &[AggregatedError],
    ) -> Vec<SystemRecommendation> {
        let mut recommendations = Vec::new();

        // Analyze error patterns and generate recommendations
        for error in errors {
            match error.signature.category {
                ErrorCategory::ResourceExhaustion => {
                    recommendations.push(SystemRecommendation {
                        recommendation_type: RecommendationType::ResourceUpgrade,
                        priority: RecommendationPriority::High,
                        description:
                            "Consider increasing GPU memory limits or optimizing resource usage"
                                .to_string(),
                        estimated_impact: 0.7,
                        implementation_effort: EffortLevel::Medium,
                    });
                }
                ErrorCategory::Performance => {
                    recommendations.push(SystemRecommendation {
                        recommendation_type: RecommendationType::CodeOptimization,
                        priority: RecommendationPriority::Medium,
                        description: "Optimize rendering pipeline to improve performance"
                            .to_string(),
                        estimated_impact: 0.5,
                        implementation_effort: EffortLevel::High,
                    });
                }
                ErrorCategory::PlatformCompatibility => {
                    recommendations.push(SystemRecommendation {
                        recommendation_type: RecommendationType::ConfigurationChange,
                        priority: RecommendationPriority::High,
                        description: "Enable fallback rendering modes for better compatibility"
                            .to_string(),
                        estimated_impact: 0.8,
                        implementation_effort: EffortLevel::Low,
                    });
                }
                _ => {}
            }
        }

        recommendations
    }

    fn analyze_error_trends(&self, _errors: &[AggregatedError]) -> ErrorTrends {
        // In a real implementation, this would analyze historical data
        // For now, we return stable trends
        ErrorTrends {
            error_rate_trend: TrendDirection::Stable,
            severity_trend: TrendDirection::Stable,
            recovery_rate_trend: TrendDirection::Stable,
            new_error_types: 0,
            resolved_error_types: 0,
        }
    }
}

/// Error reporting statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorStats {
    /// Number of distinct aggregated error groups.
    pub total_aggregated_errors: usize,
    /// Number of reports sent in the current rate-limit window.
    pub reports_in_current_window: usize,
    /// Timestamp of the most recent error report.
    pub last_report_time: Option<SystemTime>,
    /// Whether the configured error sink is available.
    pub sink_available: bool,
}

impl ErrorAggregator {
    fn new(window: Duration) -> Self {
        Self {
            error_groups: HashMap::new(),
            aggregation_window: window,
            last_cleanup: SystemTime::now(),
        }
    }

    fn add_error(&mut self, context: ErrorContext) {
        let signature = ErrorSignature::from_context(&context);

        let aggregated = self
            .error_groups
            .entry(signature.clone())
            .or_insert_with(|| AggregatedError {
                signature: signature.clone(),
                count: 0,
                first_seen: context.timestamp,
                last_seen: context.timestamp,
                sample_contexts: Vec::new(),
                affected_systems: Vec::new(),
            });

        aggregated.count += 1;
        aggregated.last_seen = context.timestamp;

        // Keep sample contexts for analysis (limited number)
        if aggregated.sample_contexts.len() < 5 {
            aggregated.sample_contexts.push(context);
        }

        self.cleanup_if_needed();
    }

    fn get_errors_since(&self, cutoff: SystemTime) -> Vec<AggregatedError> {
        self.error_groups
            .values()
            .filter(|error| error.last_seen >= cutoff)
            .cloned()
            .collect()
    }

    fn last_error_time(&self) -> Option<SystemTime> {
        self.error_groups.values().map(|e| e.last_seen).max()
    }

    fn cleanup_if_needed(&mut self) {
        let now = SystemTime::now();
        if now.duration_since(self.last_cleanup).unwrap_or_default() > Duration::from_secs(3600) {
            self.cleanup_old_errors();
            self.last_cleanup = now;
        }
    }

    fn cleanup_old_errors(&mut self) {
        let cutoff = SystemTime::now() - self.aggregation_window;
        self.error_groups
            .retain(|_, error| error.last_seen >= cutoff);
    }
}

impl ErrorSignature {
    fn from_context(context: &ErrorContext) -> Self {
        Self {
            category: context.error.category(),
            severity: context.error.severity(),
            error_type: format!("{:?}", std::mem::discriminant(&context.error)),
            key_context: Self::extract_key_context(&context.error),
        }
    }

    fn extract_key_context(error: &GupError) -> String {
        match error {
            GupError::ShaderCompilationError { shader_type, .. } => shader_type.clone(),
            GupError::GpuResourceCreationError { resource_type, .. } => resource_type.clone(),
            GupError::PlatformNotSupported { platform, .. } => platform.clone(),
            _ => "general".to_string(),
        }
    }
}

impl RateLimiter {
    fn new(window: Duration, max_reports: usize) -> Self {
        Self {
            error_counts: HashMap::new(),
            window_start: SystemTime::now(),
            window_duration: window,
            max_reports_per_window: max_reports,
        }
    }

    fn should_report(&mut self, signature: &ErrorSignature) -> bool {
        self.update_window_if_needed();

        let current_count = *self.error_counts.get(signature).unwrap_or(&0);
        if current_count >= self.max_reports_per_window {
            return false;
        }

        *self.error_counts.entry(signature.clone()).or_insert(0) += 1;
        true
    }

    fn current_window_count(&self) -> usize {
        self.error_counts.values().sum()
    }

    fn update_window_if_needed(&mut self) {
        let now = SystemTime::now();
        if now.duration_since(self.window_start).unwrap_or_default() >= self.window_duration {
            self.error_counts.clear();
            self.window_start = now;
        }
    }
}

/// Console-based error sink for development.
#[derive(Debug)]
pub struct ConsoleErrorSink {
    verbose: bool,
}

impl Default for ConsoleErrorSink {
    fn default() -> Self {
        Self::new()
    }
}

impl ConsoleErrorSink {
    /// Create a new console error sink with default verbosity.
    pub fn new() -> Self {
        Self { verbose: false }
    }

    /// Create a new console error sink with the specified verbosity.
    pub fn with_verbose(verbose: bool) -> Self {
        Self { verbose }
    }
}

impl ErrorSink for ConsoleErrorSink {
    fn send_error(&self, context: &ErrorContext) -> GupResult<()> {
        if self.verbose {
            println!("ERROR REPORT: {context:#?}");
        } else {
            println!("Error {}: {}", context.error_id, context.error);
        }
        Ok(())
    }
}

/// File-based error sink for persistent logging.
#[derive(Debug)]
pub struct FileErrorSink {
    file_path: std::path::PathBuf,
}

impl FileErrorSink {
    /// Create a new file error sink that writes to the given path.
    pub fn new(file_path: std::path::PathBuf) -> Self {
        Self { file_path }
    }
}

impl ErrorSink for FileErrorSink {
    fn send_error(&self, context: &ErrorContext) -> GupResult<()> {
        use std::io::Write;

        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.file_path)
            .map_err(|e| GupError::FileError {
                path: self.file_path.display().to_string(),
                error: e.to_string(),
            })?;

        let json = serde_json::to_string(context).map_err(GupError::from)?;

        writeln!(file, "{json}").map_err(|e| GupError::FileError {
            path: self.file_path.display().to_string(),
            error: e.to_string(),
        })?;

        Ok(())
    }
}

impl Default for ErrorReporter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_reporter_creation() {
        let reporter = ErrorReporter::new();
        assert!(reporter.error_sink.is_available());
    }

    #[test]
    fn test_error_aggregation() {
        let mut aggregator = ErrorAggregator::new(Duration::from_secs(300));

        let error = GupError::gpu_memory_exhausted(2048, 1024);
        let context1 = ErrorContext::new(error.clone());
        let context2 = ErrorContext::new(error);

        aggregator.add_error(context1);
        aggregator.add_error(context2);

        assert_eq!(aggregator.error_groups.len(), 1);
        let aggregated = aggregator.error_groups.values().next().unwrap();
        assert_eq!(aggregated.count, 2);
    }

    #[test]
    fn test_rate_limiting() {
        let mut limiter = RateLimiter::new(Duration::from_secs(60), 2);

        let signature = ErrorSignature {
            category: ErrorCategory::ResourceExhaustion,
            severity: ErrorSeverity::High,
            error_type: "GpuMemoryExhausted".to_string(),
            key_context: "general".to_string(),
        };

        assert!(limiter.should_report(&signature)); // First report - allowed
        assert!(limiter.should_report(&signature)); // Second report - allowed
        assert!(!limiter.should_report(&signature)); // Third report - rate limited
    }

    #[test]
    fn test_error_summary_generation() {
        let mut reporter = ErrorReporter::new();

        // Add some errors
        let errors = vec![
            GupError::gpu_memory_exhausted(2048, 1024),
            GupError::shader_compilation_failed("vertex", "syntax error"),
            GupError::gpu_memory_exhausted(1024, 512),
        ];

        for error in errors {
            let context = ErrorContext::new(error);
            reporter.report_error(context);
        }

        let summary = reporter.generate_error_summary(Duration::from_secs(300));
        assert!(summary.total_errors >= 3);
        assert!(!summary.error_categories.is_empty());
    }

    #[test]
    fn test_console_error_sink() {
        let sink = ConsoleErrorSink::new();
        let error = GupError::gpu_initialization_failed("Test error");
        let context = ErrorContext::new(error);

        // Should not panic
        let result = sink.send_error(&context);
        assert!(result.is_ok());
    }
}

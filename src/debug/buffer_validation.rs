// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Buffer validation rules and runtime checking.
//!
//! This module provides extensible validation rules for GPU buffer contents,
//! enabling automated detection of common issues like NaN values, out-of-range
//! data, and buffer utilization problems.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Severity level for validation issues
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValidationSeverity {
    /// Informational message
    Info,
    /// Potential issue that should be reviewed
    Warning,
    /// Significant problem that may cause incorrect behavior
    Error,
    /// Critical issue that will likely cause failures
    Critical,
}

impl fmt::Display for ValidationSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValidationSeverity::Info => write!(f, "INFO"),
            ValidationSeverity::Warning => write!(f, "WARNING"),
            ValidationSeverity::Error => write!(f, "ERROR"),
            ValidationSeverity::Critical => write!(f, "CRITICAL"),
        }
    }
}

/// Result of a validation check
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    /// Whether the validation passed
    pub passed: bool,
    /// Severity of the issue if validation failed
    pub severity: ValidationSeverity,
    /// Human-readable message describing the issue
    pub message: String,
    /// Indices of affected elements (limited to avoid large output)
    pub affected_indices: Vec<usize>,
    /// Optional suggestion for how to fix the issue
    pub suggested_fix: Option<String>,
}

impl ValidationResult {
    /// Create a passing validation result
    pub fn pass(message: impl Into<String>) -> Self {
        Self {
            passed: true,
            severity: ValidationSeverity::Info,
            message: message.into(),
            affected_indices: Vec::new(),
            suggested_fix: None,
        }
    }

    /// Create a failing validation result
    pub fn fail(
        severity: ValidationSeverity,
        message: impl Into<String>,
        affected_indices: Vec<usize>,
        suggested_fix: Option<String>,
    ) -> Self {
        Self {
            passed: false,
            severity,
            message: message.into(),
            affected_indices,
            suggested_fix,
        }
    }
}

/// Metadata about a buffer being validated
#[derive(Debug, Clone)]
pub struct BufferMetadata {
    /// Total capacity of the buffer in elements
    pub capacity: usize,
    /// Current number of elements in the buffer
    pub len: usize,
    /// Size of each element in bytes
    pub element_size: usize,
    /// Total buffer size in bytes
    pub buffer_size: u64,
}

impl BufferMetadata {
    /// Calculate buffer utilization as a percentage
    pub fn utilization(&self) -> f32 {
        if self.capacity == 0 {
            0.0
        } else {
            (self.len as f32 / self.capacity as f32) * 100.0
        }
    }
}

/// Trait for validation rules that can be applied to buffer data
pub trait ValidationRule<T>: Send + Sync {
    /// Validate the buffer data
    fn validate(&self, data: &[T], metadata: &BufferMetadata) -> ValidationResult;

    /// Get a description of what this rule validates
    fn description(&self) -> &'static str;

    /// Get the severity level for failures of this rule
    fn severity(&self) -> ValidationSeverity;
}

/// Validation rule for detecting finite values (no NaN or infinity)
pub struct FiniteValueRule;

impl ValidationRule<f32> for FiniteValueRule {
    fn validate(&self, data: &[f32], _metadata: &BufferMetadata) -> ValidationResult {
        let invalid_indices: Vec<usize> = data
            .iter()
            .enumerate()
            .filter(|&(_, &val)| !val.is_finite())
            .map(|(i, _)| i)
            .take(100) // Limit to first 100 issues
            .collect();

        if invalid_indices.is_empty() {
            ValidationResult::pass(format!("All {} values are finite", data.len()))
        } else {
            let total_invalid = data.iter().filter(|&&v| !v.is_finite()).count();
            ValidationResult::fail(
                ValidationSeverity::Error,
                format!(
                    "Found {} non-finite values (NaN or infinity) out of {}",
                    total_invalid,
                    data.len()
                ),
                invalid_indices,
                Some("Check data source for NaN/infinity generation".to_string()),
            )
        }
    }

    fn description(&self) -> &'static str {
        "Validates that all f32 values are finite (not NaN or infinity)"
    }

    fn severity(&self) -> ValidationSeverity {
        ValidationSeverity::Error
    }
}

/// Validation rule for range checking
pub struct RangeValidationRule<T> {
    min_value: T,
    max_value: T,
    field_name: String,
}

impl<T> RangeValidationRule<T> {
    /// Create a new range validation rule
    pub fn new(min_value: T, max_value: T, field_name: impl Into<String>) -> Self {
        Self {
            min_value,
            max_value,
            field_name: field_name.into(),
        }
    }
}

impl ValidationRule<f32> for RangeValidationRule<f32> {
    fn validate(&self, data: &[f32], _metadata: &BufferMetadata) -> ValidationResult {
        let out_of_range: Vec<usize> = data
            .iter()
            .enumerate()
            .filter(|&(_, &val)| val < self.min_value || val > self.max_value)
            .map(|(i, _)| i)
            .take(100)
            .collect();

        if out_of_range.is_empty() {
            ValidationResult::pass(format!(
                "All {} values in range [{}, {}]",
                data.len(),
                self.min_value,
                self.max_value
            ))
        } else {
            let total_out_of_range = data
                .iter()
                .filter(|&&v| v < self.min_value || v > self.max_value)
                .count();
            ValidationResult::fail(
                ValidationSeverity::Warning,
                format!(
                    "Found {} values outside range [{}, {}] for field '{}'",
                    total_out_of_range, self.min_value, self.max_value, self.field_name
                ),
                out_of_range,
                Some(format!(
                    "Clamp values to range [{}, {}] or adjust scale",
                    self.min_value, self.max_value
                )),
            )
        }
    }

    fn description(&self) -> &'static str {
        "Validates that values are within expected range"
    }

    fn severity(&self) -> ValidationSeverity {
        ValidationSeverity::Warning
    }
}

/// Validation rule for buffer utilization
pub struct UtilizationValidationRule {
    min_utilization: f32,
}

impl UtilizationValidationRule {
    /// Create a new utilization validation rule
    pub fn new(min_utilization: f32) -> Self {
        Self { min_utilization }
    }
}

impl<T> ValidationRule<T> for UtilizationValidationRule {
    fn validate(&self, _data: &[T], metadata: &BufferMetadata) -> ValidationResult {
        let utilization = metadata.utilization();

        if utilization >= self.min_utilization {
            ValidationResult::pass(format!("Buffer utilization: {:.1}%", utilization))
        } else {
            ValidationResult::fail(
                ValidationSeverity::Warning,
                format!(
                    "Low buffer utilization: {:.1}% (capacity: {}, used: {})",
                    utilization, metadata.capacity, metadata.len
                ),
                Vec::new(),
                Some(format!(
                    "Consider reducing buffer capacity from {} to around {}",
                    metadata.capacity,
                    (metadata.len as f32 * 1.2) as usize
                )),
            )
        }
    }

    fn description(&self) -> &'static str {
        "Validates buffer memory utilization efficiency"
    }

    fn severity(&self) -> ValidationSeverity {
        ValidationSeverity::Warning
    }
}

/// Validation rule for detecting buffer size mismatches
pub struct BufferSizeValidationRule {
    expected_min_size: usize,
    expected_max_size: usize,
}

impl BufferSizeValidationRule {
    /// Create a new buffer size validation rule
    pub fn new(expected_min_size: usize, expected_max_size: usize) -> Self {
        Self {
            expected_min_size,
            expected_max_size,
        }
    }
}

impl<T> ValidationRule<T> for BufferSizeValidationRule {
    fn validate(&self, _data: &[T], metadata: &BufferMetadata) -> ValidationResult {
        if metadata.len < self.expected_min_size {
            ValidationResult::fail(
                ValidationSeverity::Error,
                format!(
                    "Buffer size {} is below minimum expected size {}",
                    metadata.len, self.expected_min_size
                ),
                Vec::new(),
                Some("Ensure data is fully populated before rendering".to_string()),
            )
        } else if metadata.len > self.expected_max_size {
            ValidationResult::fail(
                ValidationSeverity::Warning,
                format!(
                    "Buffer size {} exceeds maximum expected size {}",
                    metadata.len, self.expected_max_size
                ),
                Vec::new(),
                Some("Consider implementing data pagination or filtering".to_string()),
            )
        } else {
            ValidationResult::pass(format!(
                "Buffer size {} within expected range [{}, {}]",
                metadata.len, self.expected_min_size, self.expected_max_size
            ))
        }
    }

    fn description(&self) -> &'static str {
        "Validates buffer size is within expected bounds"
    }

    fn severity(&self) -> ValidationSeverity {
        ValidationSeverity::Warning
    }
}

/// Complete validation report for a buffer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationReport {
    /// Individual validation results
    pub results: Vec<ValidationResult>,
    /// Timestamp when validation was performed
    pub timestamp: String,
}

impl ValidationReport {
    /// Create a new validation report
    pub fn new(results: Vec<ValidationResult>) -> Self {
        Self {
            results,
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }

    /// Check if any validation failed
    pub fn has_failures(&self) -> bool {
        self.results.iter().any(|r| !r.passed)
    }

    /// Check if there are any errors
    pub fn has_errors(&self) -> bool {
        self.results.iter().any(|r| {
            !r.passed
                && (r.severity == ValidationSeverity::Error
                    || r.severity == ValidationSeverity::Critical)
        })
    }

    /// Check if there are any warnings
    pub fn has_warnings(&self) -> bool {
        self.results
            .iter()
            .any(|r| !r.passed && r.severity == ValidationSeverity::Warning)
    }

    /// Get count of issues by severity
    pub fn count_by_severity(&self, severity: ValidationSeverity) -> usize {
        self.results
            .iter()
            .filter(|r| !r.passed && r.severity == severity)
            .count()
    }

    /// Format the report as a human-readable string
    pub fn format_report(&self) -> String {
        let mut output = String::new();
        output.push_str("=== Buffer Validation Report ===\n");
        output.push_str(&format!("Timestamp: {}\n", self.timestamp));
        output.push_str(&format!("Total checks: {}\n", self.results.len()));
        output.push_str(&format!(
            "Passed: {}\n",
            self.results.iter().filter(|r| r.passed).count()
        ));
        output.push_str(&format!(
            "Critical: {}\n",
            self.count_by_severity(ValidationSeverity::Critical)
        ));
        output.push_str(&format!(
            "Errors: {}\n",
            self.count_by_severity(ValidationSeverity::Error)
        ));
        output.push_str(&format!(
            "Warnings: {}\n",
            self.count_by_severity(ValidationSeverity::Warning)
        ));
        output.push('\n');

        // Show failed checks
        for result in &self.results {
            if !result.passed {
                output.push_str(&format!("[{}] {}\n", result.severity, result.message));
                if !result.affected_indices.is_empty() {
                    output.push_str(&format!(
                        "  Affected indices: {:?}...\n",
                        &result.affected_indices[..result.affected_indices.len().min(10)]
                    ));
                }
                if let Some(fix) = &result.suggested_fix {
                    output.push_str(&format!("  Suggestion: {}\n", fix));
                }
                output.push('\n');
            }
        }

        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_finite_value_rule_valid() {
        let rule = FiniteValueRule;
        let data = vec![1.0, 2.0, 3.0, 4.0];
        let metadata = BufferMetadata {
            capacity: 10,
            len: 4,
            element_size: 4,
            buffer_size: 40,
        };

        let result = rule.validate(&data, &metadata);
        assert!(result.passed);
        assert_eq!(result.severity, ValidationSeverity::Info);
    }

    #[test]
    fn test_finite_value_rule_nan() {
        let rule = FiniteValueRule;
        let data = vec![1.0, f32::NAN, 3.0, f32::INFINITY];
        let metadata = BufferMetadata {
            capacity: 10,
            len: 4,
            element_size: 4,
            buffer_size: 40,
        };

        let result = rule.validate(&data, &metadata);
        assert!(!result.passed);
        assert_eq!(result.severity, ValidationSeverity::Error);
        assert_eq!(result.affected_indices.len(), 2);
    }

    #[test]
    fn test_range_validation_rule() {
        let rule = RangeValidationRule::new(0.0, 1.0, "normalized");
        let data = vec![0.0, 0.5, 1.0, 1.5]; // Last value out of range
        let metadata = BufferMetadata {
            capacity: 10,
            len: 4,
            element_size: 4,
            buffer_size: 40,
        };

        let result = rule.validate(&data, &metadata);
        assert!(!result.passed);
        assert_eq!(result.severity, ValidationSeverity::Warning);
        assert_eq!(result.affected_indices.len(), 1);
        assert_eq!(result.affected_indices[0], 3);
    }

    #[test]
    fn test_utilization_rule_low() {
        let rule = UtilizationValidationRule::new(50.0);
        let metadata = BufferMetadata {
            capacity: 1000,
            len: 100, // 10% utilization
            element_size: 4,
            buffer_size: 4000,
        };

        let data: Vec<u8> = vec![0; 100];
        let result = rule.validate(&data, &metadata);
        assert!(!result.passed);
        assert_eq!(result.severity, ValidationSeverity::Warning);
    }

    #[test]
    fn test_utilization_rule_good() {
        let rule = UtilizationValidationRule::new(50.0);
        let metadata = BufferMetadata {
            capacity: 100,
            len: 80, // 80% utilization
            element_size: 4,
            buffer_size: 400,
        };

        let data: Vec<u8> = vec![0; 80];
        let result = rule.validate(&data, &metadata);
        assert!(result.passed);
    }

    #[test]
    fn test_buffer_size_rule_too_small() {
        let rule = BufferSizeValidationRule::new(100, 1000);
        let metadata = BufferMetadata {
            capacity: 100,
            len: 50, // Below minimum
            element_size: 4,
            buffer_size: 400,
        };

        let data: Vec<u8> = vec![0; 50];
        let result = rule.validate(&data, &metadata);
        assert!(!result.passed);
        assert_eq!(result.severity, ValidationSeverity::Error);
    }

    #[test]
    fn test_buffer_size_rule_too_large() {
        let rule = BufferSizeValidationRule::new(100, 1000);
        let metadata = BufferMetadata {
            capacity: 2000,
            len: 1500, // Above maximum
            element_size: 4,
            buffer_size: 8000,
        };

        let data: Vec<u8> = vec![0; 1500];
        let result = rule.validate(&data, &metadata);
        assert!(!result.passed);
        assert_eq!(result.severity, ValidationSeverity::Warning);
    }

    #[test]
    fn test_validation_report() {
        let results = vec![
            ValidationResult::pass("Check 1 passed".to_string()),
            ValidationResult::fail(
                ValidationSeverity::Warning,
                "Check 2 warned".to_string(),
                vec![1, 2, 3],
                None,
            ),
            ValidationResult::fail(
                ValidationSeverity::Error,
                "Check 3 failed".to_string(),
                vec![],
                Some("Fix it".to_string()),
            ),
        ];

        let report = ValidationReport::new(results);
        assert!(report.has_failures());
        assert!(report.has_errors());
        assert!(report.has_warnings());
        assert_eq!(report.count_by_severity(ValidationSeverity::Warning), 1);
        assert_eq!(report.count_by_severity(ValidationSeverity::Error), 1);
    }
}

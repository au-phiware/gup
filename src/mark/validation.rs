// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Mark validation framework for verifying custom mark implementations.
//!
//! This module provides automated testing and validation tools for custom mark
//! types, checking vertex/index consistency, memory layout, attribute types,
//! and generating detailed validation reports with actionable suggestions.
//!
//! # Example
//!
//! ```rust
//! use gup::mark::{Circle, Mark};
//! use gup::mark::validation::{MarkValidator, ValidationSeverity};
//!
//! let report = MarkValidator::<Circle>::validate();
//! assert!(report.is_passing());
//! assert_eq!(report.critical_issues().count(), 0);
//! ```

use crate::error::GupResult;
use crate::mark::Mark;
use std::fmt;
use std::marker::PhantomData;
use std::time::{Duration, Instant};

/// Severity level for validation issues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ValidationSeverity {
    /// Informational note; not a problem.
    Info,
    /// Potential issue that may impact performance or correctness.
    Warning,
    /// Definite problem that should be fixed.
    Error,
    /// Critical problem that will prevent correct rendering.
    Critical,
}

impl fmt::Display for ValidationSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Info => write!(f, "INFO"),
            Self::Warning => write!(f, "WARN"),
            Self::Error => write!(f, "ERROR"),
            Self::Critical => write!(f, "CRITICAL"),
        }
    }
}

/// A single validation issue found during mark validation.
#[derive(Debug, Clone)]
pub struct ValidationIssue {
    /// Severity of the issue.
    pub severity: ValidationSeverity,
    /// Category of the check (e.g., "geometry", "memory", "attributes").
    pub category: String,
    /// Human-readable description of the issue.
    pub message: String,
    /// Actionable suggestion for how to fix it.
    pub suggestion: Option<String>,
}

impl fmt::Display for ValidationIssue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}: {}", self.severity, self.category, self.message)?;
        if let Some(suggestion) = &self.suggestion {
            write!(f, " — Suggestion: {suggestion}")?;
        }
        Ok(())
    }
}

/// A named section of the validation report.
#[derive(Debug, Clone)]
pub struct ValidationSection {
    /// Name of the section.
    pub name: String,
    /// Issues found in this section.
    pub issues: Vec<ValidationIssue>,
    /// Whether the section passed (no errors or critical issues).
    pub passed: bool,
    /// Time taken for this section's checks.
    pub duration: Duration,
}

/// Complete validation report for a mark type.
#[derive(Debug, Clone)]
pub struct ValidationReport {
    /// Name of the mark type being validated.
    pub mark_type: String,
    /// Individual validation sections.
    pub sections: Vec<ValidationSection>,
    /// Total time for all validation checks.
    pub total_duration: Duration,
}

impl ValidationReport {
    /// Returns true if no critical or error-level issues were found.
    pub fn is_passing(&self) -> bool {
        self.sections.iter().all(|s| s.passed)
    }

    /// Iterator over all critical issues across all sections.
    pub fn critical_issues(&self) -> impl Iterator<Item = &ValidationIssue> {
        self.sections.iter().flat_map(|s| {
            s.issues
                .iter()
                .filter(|i| i.severity == ValidationSeverity::Critical)
        })
    }

    /// Iterator over all issues at error level or above.
    pub fn errors(&self) -> impl Iterator<Item = &ValidationIssue> {
        self.sections.iter().flat_map(|s| {
            s.issues
                .iter()
                .filter(|i| i.severity >= ValidationSeverity::Error)
        })
    }

    /// Iterator over all issues.
    pub fn all_issues(&self) -> impl Iterator<Item = &ValidationIssue> {
        self.sections.iter().flat_map(|s| s.issues.iter())
    }

    /// Total number of issues found.
    pub fn issue_count(&self) -> usize {
        self.sections.iter().map(|s| s.issues.len()).sum()
    }

    /// Produce a human-readable summary string.
    pub fn summary(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "=== Validation Report for {} ===\n",
            self.mark_type
        ));
        out.push_str(&format!(
            "Result: {}\n",
            if self.is_passing() { "PASS" } else { "FAIL" }
        ));
        out.push_str(&format!("Duration: {:?}\n\n", self.total_duration));

        for section in &self.sections {
            let status = if section.passed { "✅" } else { "❌" };
            out.push_str(&format!(
                "{} {} ({:?})\n",
                status, section.name, section.duration
            ));
            for issue in &section.issues {
                out.push_str(&format!("  {issue}\n"));
            }
        }

        let critical = self.critical_issues().count();
        let errors = self.errors().count();
        let total = self.issue_count();
        out.push_str(&format!(
            "\nSummary: {total} issues ({critical} critical, {errors} errors)\n"
        ));
        out
    }
}

/// Validator for custom mark implementations.
///
/// Performs comprehensive validation checks on a mark type including
/// geometry consistency, memory layout, and attribute type correctness.
///
/// # Example
///
/// ```rust
/// use gup::mark::{Circle, Mark};
/// use gup::mark::validation::MarkValidator;
///
/// let report = MarkValidator::<Circle>::validate();
/// println!("{}", report.summary());
/// assert!(report.is_passing());
/// ```
pub struct MarkValidator<M: Mark> {
    _phantom: PhantomData<M>,
}

impl<M: Mark> MarkValidator<M> {
    /// Run all validation checks and return a comprehensive report.
    pub fn validate() -> ValidationReport {
        let start = Instant::now();
        let mark_type = std::any::type_name::<M>().to_string();

        let sections = vec![
            Self::validate_geometry(),
            Self::validate_memory_layout(),
            Self::validate_attribute_types(),
            Self::validate_shader_support(),
        ];

        ValidationReport {
            mark_type,
            sections,
            total_duration: start.elapsed(),
        }
    }

    /// Validate geometry consistency: vertices, indices, and counts.
    fn validate_geometry() -> ValidationSection {
        let start = Instant::now();
        let mut issues = Vec::new();

        // Check vertex count matches generated vertices
        let expected_count = M::vertex_count();
        let vertices = M::generate_vertices();
        let actual_count = vertices.len();

        if actual_count != expected_count {
            issues.push(ValidationIssue {
                severity: ValidationSeverity::Critical,
                category: "geometry".to_string(),
                message: format!(
                    "vertex_count() returns {expected_count} but generate_vertices() \
                     produces {actual_count} vertices"
                ),
                suggestion: Some(format!(
                    "Update vertex_count() to return {actual_count}, or fix \
                     generate_vertices() to produce {expected_count} vertices."
                )),
            });
        }

        if expected_count == 0 {
            issues.push(ValidationIssue {
                severity: ValidationSeverity::Critical,
                category: "geometry".to_string(),
                message: "vertex_count() returns 0 — mark has no geometry".to_string(),
                suggestion: Some(
                    "A mark must have at least 3 vertices (for a triangle).".to_string(),
                ),
            });
        } else if expected_count < 3 {
            issues.push(ValidationIssue {
                severity: ValidationSeverity::Warning,
                category: "geometry".to_string(),
                message: format!(
                    "vertex_count() returns {expected_count} — \
                     fewer than 3 vertices may not form a renderable shape"
                ),
                suggestion: Some(
                    "Consider using at least 3 vertices for triangle-based rendering.".to_string(),
                ),
            });
        }

        // Check index consistency
        let expected_index_count = M::index_count();
        let indices = M::generate_indices();

        match (expected_index_count, &indices) {
            (Some(expected), Some(actual_indices)) => {
                if actual_indices.len() != expected {
                    issues.push(ValidationIssue {
                        severity: ValidationSeverity::Critical,
                        category: "geometry".to_string(),
                        message: format!(
                            "index_count() returns Some({expected}) but \
                             generate_indices() produces {} indices",
                            actual_indices.len()
                        ),
                        suggestion: Some(format!(
                            "Update index_count() to return Some({}), or fix \
                             generate_indices() to produce {expected} indices.",
                            actual_indices.len()
                        )),
                    });
                }

                // Validate indices are in bounds
                for (i, &idx) in actual_indices.iter().enumerate() {
                    if idx as usize >= actual_count {
                        issues.push(ValidationIssue {
                            severity: ValidationSeverity::Critical,
                            category: "geometry".to_string(),
                            message: format!(
                                "Index {idx} at position {i} is out of bounds \
                                 (vertex count: {actual_count})"
                            ),
                            suggestion: Some(format!(
                                "All indices must be in range 0..{actual_count}."
                            )),
                        });
                        break; // Only report first out-of-bounds
                    }
                }

                // Check triangle alignment
                if actual_indices.len() % 3 != 0 {
                    issues.push(ValidationIssue {
                        severity: ValidationSeverity::Warning,
                        category: "geometry".to_string(),
                        message: format!(
                            "Index count ({}) is not a multiple of 3 — \
                             not aligned for TriangleList topology",
                            actual_indices.len()
                        ),
                        suggestion: Some(
                            "For TriangleList topology, index count should be a multiple of 3."
                                .to_string(),
                        ),
                    });
                }
            }
            (Some(expected), None) => {
                issues.push(ValidationIssue {
                    severity: ValidationSeverity::Critical,
                    category: "geometry".to_string(),
                    message: format!(
                        "index_count() returns Some({expected}) but \
                         generate_indices() returns None"
                    ),
                    suggestion: Some(
                        "Either implement generate_indices() to return Some(...), \
                         or set index_count() to return None."
                            .to_string(),
                    ),
                });
            }
            (None, Some(actual_indices)) => {
                issues.push(ValidationIssue {
                    severity: ValidationSeverity::Warning,
                    category: "geometry".to_string(),
                    message: format!(
                        "index_count() returns None but generate_indices() returns \
                         Some({} indices) — indices will be ignored",
                        actual_indices.len()
                    ),
                    suggestion: Some(
                        "Set index_count() to return Some(N) to enable indexed rendering."
                            .to_string(),
                    ),
                });
            }
            (None, None) => {
                // Both None — non-indexed rendering, which is valid
                issues.push(ValidationIssue {
                    severity: ValidationSeverity::Info,
                    category: "geometry".to_string(),
                    message: "Using non-indexed rendering".to_string(),
                    suggestion: None,
                });
            }
        }

        let passed = !issues
            .iter()
            .any(|i| i.severity >= ValidationSeverity::Error);

        ValidationSection {
            name: "Geometry Validation".to_string(),
            issues,
            passed,
            duration: start.elapsed(),
        }
    }

    /// Validate memory layout of the vertex type.
    fn validate_memory_layout() -> ValidationSection {
        let start = Instant::now();
        let mut issues = Vec::new();

        let vertex_size = std::mem::size_of::<M::Vertex>();
        let vertex_align = std::mem::align_of::<M::Vertex>();

        // Check vertex size is reasonable
        if vertex_size == 0 {
            issues.push(ValidationIssue {
                severity: ValidationSeverity::Critical,
                category: "memory".to_string(),
                message: "Vertex type has zero size — cannot upload to GPU".to_string(),
                suggestion: Some(
                    "Add at least a position field (e.g., `position: [f32; 2]`) \
                     to the vertex type."
                        .to_string(),
                ),
            });
        }

        // Check alignment is GPU-compatible
        if vertex_align < 4 {
            issues.push(ValidationIssue {
                severity: ValidationSeverity::Warning,
                category: "memory".to_string(),
                message: format!(
                    "Vertex alignment ({vertex_align}) is less than 4 bytes — \
                     may cause issues on some GPUs"
                ),
                suggestion: Some(
                    "Ensure vertex type uses #[repr(C)] for proper alignment.".to_string(),
                ),
            });
        }

        // Check vertex size is a multiple of alignment
        if vertex_size % vertex_align != 0 {
            issues.push(ValidationIssue {
                severity: ValidationSeverity::Warning,
                category: "memory".to_string(),
                message: format!(
                    "Vertex size ({vertex_size}) is not a multiple of \
                     alignment ({vertex_align})"
                ),
                suggestion: Some("Add padding fields to align vertex size.".to_string()),
            });
        }

        // Warn about large vertices
        if vertex_size > 256 {
            issues.push(ValidationIssue {
                severity: ValidationSeverity::Warning,
                category: "memory".to_string(),
                message: format!(
                    "Vertex size ({vertex_size} bytes) is large — \
                     consider using instance data for per-element properties"
                ),
                suggestion: Some(
                    "Keep vertex data minimal (position only) and use \
                     storage buffers for instance data."
                        .to_string(),
                ),
            });
        } else {
            issues.push(ValidationIssue {
                severity: ValidationSeverity::Info,
                category: "memory".to_string(),
                message: format!(
                    "Vertex size: {vertex_size} bytes, alignment: {vertex_align} bytes"
                ),
                suggestion: None,
            });
        }

        // Verify bytemuck compatibility by round-tripping through bytes
        let vertices = M::generate_vertices();
        if !vertices.is_empty() {
            let bytes = bytemuck::cast_slice::<M::Vertex, u8>(&vertices);
            let expected_bytes = vertices.len() * vertex_size;
            if bytes.len() != expected_bytes {
                issues.push(ValidationIssue {
                    severity: ValidationSeverity::Critical,
                    category: "memory".to_string(),
                    message: format!(
                        "Bytemuck cast produced {actual} bytes but expected {expected_bytes} \
                         ({count} vertices × {vertex_size} bytes)",
                        actual = bytes.len(),
                        count = vertices.len()
                    ),
                    suggestion: Some("Ensure vertex type derives bytemuck::Pod and bytemuck::Zeroable correctly.".to_string()),
                });
            }
        }

        let passed = !issues
            .iter()
            .any(|i| i.severity >= ValidationSeverity::Error);

        ValidationSection {
            name: "Memory Layout Validation".to_string(),
            issues,
            passed,
            duration: start.elapsed(),
        }
    }

    /// Validate attribute type declarations.
    fn validate_attribute_types() -> ValidationSection {
        let start = Instant::now();
        let mut issues = Vec::new();

        // Check common attribute names that all marks should support
        let common_attrs = [("position", "vec2<f32>"), ("color", "vec4<f32>")];

        for (attr_name, expected_type) in &common_attrs {
            match M::get_attribute_type(attr_name) {
                Ok(actual_type) => {
                    if actual_type != *expected_type {
                        issues.push(ValidationIssue {
                            severity: ValidationSeverity::Info,
                            category: "attributes".to_string(),
                            message: format!(
                                "Attribute '{attr_name}' has type '{actual_type}' \
                                 (common default is '{expected_type}')"
                            ),
                            suggestion: None,
                        });
                    }
                }
                Err(_) => {
                    issues.push(ValidationIssue {
                        severity: ValidationSeverity::Info,
                        category: "attributes".to_string(),
                        message: format!(
                            "Common attribute '{attr_name}' not defined — \
                             mark may not support standard shader function bindings"
                        ),
                        suggestion: Some(format!(
                            "Consider adding '{attr_name}' to get_attribute_type() \
                             for shader function compatibility."
                        )),
                    });
                }
            }
        }

        let passed = !issues
            .iter()
            .any(|i| i.severity >= ValidationSeverity::Error);

        ValidationSection {
            name: "Attribute Type Validation".to_string(),
            issues,
            passed,
            duration: start.elapsed(),
        }
    }

    /// Validate shader support configuration.
    fn validate_shader_support() -> ValidationSection {
        let start = Instant::now();
        let mut issues = Vec::new();

        let has_vertex = M::VERTEX_SHADER.is_some();
        let has_fragment = M::FRAGMENT_SHADER.is_some();
        let has_pattern = M::PATTERN_FRAGMENT_SHADER.is_some();

        if has_vertex != has_fragment {
            issues.push(ValidationIssue {
                severity: ValidationSeverity::Error,
                category: "shaders".to_string(),
                message: format!(
                    "Mismatched shader constants: VERTEX_SHADER is {}, \
                     FRAGMENT_SHADER is {}",
                    if has_vertex { "Some" } else { "None" },
                    if has_fragment { "Some" } else { "None" }
                ),
                suggestion: Some(
                    "Either provide both VERTEX_SHADER and FRAGMENT_SHADER, \
                     or set both to None for generated shaders."
                        .to_string(),
                ),
            });
        }

        if has_vertex && has_fragment {
            issues.push(ValidationIssue {
                severity: ValidationSeverity::Info,
                category: "shaders".to_string(),
                message: "Using hand-optimized shaders".to_string(),
                suggestion: None,
            });
        } else {
            issues.push(ValidationIssue {
                severity: ValidationSeverity::Info,
                category: "shaders".to_string(),
                message: "Using generated shaders (default implementation)".to_string(),
                suggestion: None,
            });
        }

        if has_pattern {
            issues.push(ValidationIssue {
                severity: ValidationSeverity::Info,
                category: "shaders".to_string(),
                message: "Pattern fragment shader available for accessibility rendering"
                    .to_string(),
                suggestion: None,
            });
        }

        let passed = !issues
            .iter()
            .any(|i| i.severity >= ValidationSeverity::Error);

        ValidationSection {
            name: "Shader Support Validation".to_string(),
            issues,
            passed,
            duration: start.elapsed(),
        }
    }
}

/// Performance profiler for mark implementations.
///
/// Measures vertex generation performance and memory efficiency
/// to help identify optimization opportunities.
///
/// # Example
///
/// ```rust
/// use gup::mark::{Circle, Mark};
/// use gup::mark::validation::MarkProfiler;
///
/// let profile = MarkProfiler::<Circle>::profile();
/// println!("{}", profile.summary());
/// assert!(profile.vertex_generation_time.as_millis() < 100);
/// ```
pub struct MarkProfiler<M: Mark> {
    _phantom: PhantomData<M>,
}

/// Results from profiling a mark implementation.
#[derive(Debug, Clone)]
pub struct ProfileReport {
    /// Name of the mark type profiled.
    pub mark_type: String,
    /// Time to generate vertices once.
    pub vertex_generation_time: Duration,
    /// Time to generate indices once (if applicable).
    pub index_generation_time: Option<Duration>,
    /// Number of vertices generated.
    pub vertex_count: usize,
    /// Number of indices generated (if applicable).
    pub index_count: Option<usize>,
    /// Size of each vertex in bytes.
    pub vertex_size_bytes: usize,
    /// Total vertex buffer size in bytes.
    pub total_vertex_bytes: usize,
    /// Total index buffer size in bytes (if applicable).
    pub total_index_bytes: Option<usize>,
    /// Average time per vertex generation (over multiple iterations).
    pub avg_vertex_gen_per_iteration: Duration,
    /// Performance classification.
    pub classification: PerformanceClass,
}

/// Performance classification for a mark.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PerformanceClass {
    /// Excellent: vertex generation < 1μs
    Excellent,
    /// Good: vertex generation < 100μs
    Good,
    /// Acceptable: vertex generation < 1ms
    Acceptable,
    /// NeedsWork: vertex generation >= 1ms
    NeedsWork,
}

impl fmt::Display for PerformanceClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Excellent => write!(f, "Excellent"),
            Self::Good => write!(f, "Good"),
            Self::Acceptable => write!(f, "Acceptable"),
            Self::NeedsWork => write!(f, "Needs Work"),
        }
    }
}

impl ProfileReport {
    /// Produce a human-readable summary.
    pub fn summary(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "=== Performance Profile for {} ===\n",
            self.mark_type
        ));
        out.push_str(&format!("Classification: {}\n", self.classification));
        out.push_str(&format!(
            "Vertex generation: {:?}\n",
            self.vertex_generation_time
        ));
        out.push_str(&format!(
            "Avg per iteration (100 runs): {:?}\n",
            self.avg_vertex_gen_per_iteration
        ));
        out.push_str(&format!(
            "Vertices: {} ({} bytes each, {} bytes total)\n",
            self.vertex_count, self.vertex_size_bytes, self.total_vertex_bytes
        ));
        if let (Some(idx_count), Some(idx_bytes)) = (self.index_count, self.total_index_bytes) {
            out.push_str(&format!("Indices: {idx_count} ({idx_bytes} bytes total)\n"));
        }
        out
    }
}

impl<M: Mark> MarkProfiler<M> {
    /// Profile the mark implementation's performance.
    pub fn profile() -> ProfileReport {
        let mark_type = std::any::type_name::<M>().to_string();

        // Warmup
        let _ = M::generate_vertices();

        // Single-shot vertex generation
        let start = Instant::now();
        let vertices = M::generate_vertices();
        let vertex_generation_time = start.elapsed();

        // Single-shot index generation
        let index_start = Instant::now();
        let indices = M::generate_indices();
        let index_generation_time = indices.as_ref().map(|_| index_start.elapsed());

        // Multi-iteration average
        let iterations = 100;
        let avg_start = Instant::now();
        for _ in 0..iterations {
            let _ = M::generate_vertices();
        }
        let avg_vertex_gen_per_iteration = avg_start.elapsed() / iterations;

        let vertex_size_bytes = std::mem::size_of::<M::Vertex>();
        let total_vertex_bytes = vertices.len() * vertex_size_bytes;
        let total_index_bytes = indices
            .as_ref()
            .map(|i| i.len() * std::mem::size_of::<u32>());

        // Classify performance
        let classification = if avg_vertex_gen_per_iteration.as_micros() < 1 {
            PerformanceClass::Excellent
        } else if avg_vertex_gen_per_iteration.as_micros() < 100 {
            PerformanceClass::Good
        } else if avg_vertex_gen_per_iteration.as_millis() < 1 {
            PerformanceClass::Acceptable
        } else {
            PerformanceClass::NeedsWork
        };

        ProfileReport {
            mark_type,
            vertex_generation_time,
            index_generation_time,
            vertex_count: vertices.len(),
            index_count: indices.as_ref().map(|i| i.len()),
            vertex_size_bytes,
            total_vertex_bytes,
            total_index_bytes,
            avg_vertex_gen_per_iteration,
            classification,
        }
    }
}

/// Convenience function: validate a mark and return an error if it fails.
///
/// Useful for integration tests or CI gates.
///
/// # Example
///
/// ```rust
/// use gup::mark::Circle;
/// use gup::mark::validation::assert_mark_valid;
///
/// assert_mark_valid::<Circle>().unwrap();
/// ```
pub fn assert_mark_valid<M: Mark>() -> GupResult<()> {
    let report = MarkValidator::<M>::validate();
    if report.is_passing() {
        Ok(())
    } else {
        let errors: Vec<String> = report.errors().map(|e| e.to_string()).collect();
        Err(crate::error::GupError::validation_error(format!(
            "Mark validation failed for {}:\n{}",
            std::any::type_name::<M>(),
            errors.join("\n")
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mark::{Circle, Line, Rectangle};

    #[test]
    fn test_circle_validation_passes() {
        let report = MarkValidator::<Circle>::validate();
        assert!(
            report.is_passing(),
            "Circle validation failed:\n{}",
            report.summary()
        );
    }

    #[test]
    fn test_rectangle_validation_passes() {
        let report = MarkValidator::<Rectangle>::validate();
        assert!(
            report.is_passing(),
            "Rectangle validation failed:\n{}",
            report.summary()
        );
    }

    #[test]
    fn test_line_validation_passes() {
        let report = MarkValidator::<Line>::validate();
        assert!(
            report.is_passing(),
            "Line validation failed:\n{}",
            report.summary()
        );
    }

    #[test]
    fn test_assert_mark_valid_circle() {
        assert_mark_valid::<Circle>().unwrap();
    }

    #[test]
    fn test_assert_mark_valid_rectangle() {
        assert_mark_valid::<Rectangle>().unwrap();
    }

    #[test]
    fn test_validation_report_summary() {
        let report = MarkValidator::<Circle>::validate();
        let summary = report.summary();
        assert!(summary.contains("Validation Report"));
        assert!(summary.contains("PASS"));
        assert!(summary.contains("Geometry"));
        assert!(summary.contains("Memory"));
    }

    #[test]
    fn test_validation_report_sections() {
        let report = MarkValidator::<Circle>::validate();
        assert_eq!(report.sections.len(), 4);
        assert_eq!(report.sections[0].name, "Geometry Validation");
        assert_eq!(report.sections[1].name, "Memory Layout Validation");
        assert_eq!(report.sections[2].name, "Attribute Type Validation");
        assert_eq!(report.sections[3].name, "Shader Support Validation");
    }

    #[test]
    fn test_circle_has_custom_shaders() {
        let report = MarkValidator::<Circle>::validate();
        let shader_section = &report.sections[3];
        let has_hand_optimized = shader_section
            .issues
            .iter()
            .any(|i| i.message.contains("hand-optimized"));
        assert!(
            has_hand_optimized,
            "Circle should be detected as having hand-optimized shaders"
        );
    }

    #[test]
    fn test_circle_profile() {
        let profile = MarkProfiler::<Circle>::profile();
        assert_eq!(profile.vertex_count, 4);
        assert_eq!(profile.index_count, Some(6));
        assert!(profile.vertex_size_bytes > 0);
        assert!(profile.total_vertex_bytes > 0);
        // Vertex generation should be fast (under 10ms)
        assert!(
            profile.vertex_generation_time.as_millis() < 10,
            "Vertex generation took {:?}",
            profile.vertex_generation_time
        );
    }

    #[test]
    fn test_profile_report_summary() {
        let profile = MarkProfiler::<Circle>::profile();
        let summary = profile.summary();
        assert!(summary.contains("Performance Profile"));
        assert!(summary.contains("Classification"));
    }

    #[test]
    fn test_derived_mark_validation() {
        // Use a derive-macro-generated mark
        #[derive(Debug, Clone, gup_macros::Mark)]
        #[mark(primitive = "quad")]
        #[allow(dead_code)]
        struct TestDerivedMark {
            pub center: crate::shader_function::Vec2,
            pub color: crate::shader_function::Vec4,
        }

        let report = MarkValidator::<TestDerivedMark>::validate();
        assert!(
            report.is_passing(),
            "Derived mark validation failed:\n{}",
            report.summary()
        );
    }

    #[test]
    fn test_derived_mark_profiling() {
        #[derive(Debug, Clone, gup_macros::Mark)]
        #[mark(primitive = "triangle")]
        #[allow(dead_code)]
        struct ProfileTriangle {
            pub position: crate::shader_function::Vec2,
            pub size: f32,
        }

        let profile = MarkProfiler::<ProfileTriangle>::profile();
        assert_eq!(profile.vertex_count, 3);
        assert_eq!(profile.index_count, None);
    }

    // Test with a deliberately broken mark to verify validation catches errors
    #[derive(Debug, Clone)]
    struct BrokenMark;

    #[repr(C)]
    #[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    struct BrokenVertex {
        position: [f32; 2],
    }

    impl Mark for BrokenMark {
        type Vertex = BrokenVertex;
        type AttributeValue = ();

        fn vertex_count() -> usize {
            4 // Says 4 vertices...
        }

        fn index_count() -> Option<usize> {
            Some(6)
        }

        fn generate_vertices() -> Vec<Self::Vertex> {
            // ...but only generates 2 — mismatch!
            vec![
                BrokenVertex {
                    position: [-1.0, -1.0],
                },
                BrokenVertex {
                    position: [1.0, 1.0],
                },
            ]
        }

        fn generate_indices() -> Option<Vec<u32>> {
            // Indices reference out-of-bounds vertex 3
            Some(vec![0, 1, 2, 0, 2, 3])
        }
    }

    #[test]
    fn test_broken_mark_fails_validation() {
        let report = MarkValidator::<BrokenMark>::validate();
        assert!(!report.is_passing(), "Broken mark should fail validation");
        assert!(
            report.critical_issues().count() > 0,
            "Broken mark should have critical issues"
        );
    }

    #[test]
    fn test_broken_mark_catches_vertex_mismatch() {
        let report = MarkValidator::<BrokenMark>::validate();
        let geom = &report.sections[0];
        let has_mismatch = geom.issues.iter().any(|i| {
            i.message.contains("vertex_count()") && i.message.contains("generate_vertices()")
        });
        assert!(has_mismatch, "Should detect vertex count mismatch");
    }

    #[test]
    fn test_broken_mark_catches_index_out_of_bounds() {
        let report = MarkValidator::<BrokenMark>::validate();
        let geom = &report.sections[0];
        let has_oob = geom
            .issues
            .iter()
            .any(|i| i.message.contains("out of bounds"));
        assert!(has_oob, "Should detect out-of-bounds indices");
    }

    #[test]
    fn test_assert_mark_valid_broken() {
        let result = assert_mark_valid::<BrokenMark>();
        assert!(result.is_err());
    }
}

// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Integration tests for the mark validation CI runner.
//!
//! Validates that the `validate_marks` binary correctly validates all
//! built-in mark types and that the validation infrastructure works
//! end-to-end.

use gup::mark::validation::{MarkProfiler, MarkValidator, PerformanceClass};
use gup::mark::{BoxPlot, Circle, Line, Path, Rectangle, Text};

/// All built-in marks must pass validation.
#[test]
fn all_built_in_marks_pass_validation() {
    let marks_passing = [
        MarkValidator::<Circle>::validate().is_passing(),
        MarkValidator::<Rectangle>::validate().is_passing(),
        MarkValidator::<Line>::validate().is_passing(),
        MarkValidator::<Path>::validate().is_passing(),
        MarkValidator::<BoxPlot>::validate().is_passing(),
        MarkValidator::<Text>::validate().is_passing(),
    ];

    for (i, passing) in marks_passing.iter().enumerate() {
        assert!(
            passing,
            "Built-in mark index {i} failed validation. \
             All built-in marks must pass."
        );
    }
}

/// No built-in mark should have critical issues.
#[test]
fn no_critical_issues_in_built_in_marks() {
    let reports = vec![
        MarkValidator::<Circle>::validate(),
        MarkValidator::<Rectangle>::validate(),
        MarkValidator::<Line>::validate(),
        MarkValidator::<Path>::validate(),
        MarkValidator::<BoxPlot>::validate(),
        MarkValidator::<Text>::validate(),
    ];

    for report in &reports {
        let critical_count = report.critical_issues().count();
        assert_eq!(
            critical_count, 0,
            "Mark {} has {critical_count} critical issues",
            report.mark_type
        );
    }
}

/// All built-in marks should have reasonable performance.
#[test]
fn built_in_marks_performance_acceptable() {
    let profiles = vec![
        MarkProfiler::<Circle>::profile(),
        MarkProfiler::<Rectangle>::profile(),
        MarkProfiler::<Line>::profile(),
        MarkProfiler::<Path>::profile(),
        MarkProfiler::<BoxPlot>::profile(),
        MarkProfiler::<Text>::profile(),
    ];

    for profile in &profiles {
        assert_ne!(
            profile.classification,
            PerformanceClass::NeedsWork,
            "Mark {} has unacceptable performance: {:?}",
            profile.mark_type,
            profile.avg_vertex_gen_per_iteration
        );
    }
}

/// Validation reports should have exactly 4 sections.
#[test]
fn validation_reports_have_expected_sections() {
    let report = MarkValidator::<Circle>::validate();
    assert_eq!(report.sections.len(), 4);
    assert_eq!(report.sections[0].name, "Geometry Validation");
    assert_eq!(report.sections[1].name, "Memory Layout Validation");
    assert_eq!(report.sections[2].name, "Attribute Type Validation");
    assert_eq!(report.sections[3].name, "Shader Support Validation");
}

/// Validation report summary should be human-readable.
#[test]
fn validation_report_produces_readable_summary() {
    let report = MarkValidator::<Circle>::validate();
    let summary = report.summary();

    assert!(summary.contains("Validation Report"));
    assert!(summary.contains("PASS"));
    assert!(summary.contains("Geometry"));
    assert!(summary.contains("Memory"));
    assert!(summary.contains("Attribute"));
    assert!(summary.contains("Shader"));
    assert!(summary.contains("Summary:"));
}

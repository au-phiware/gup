// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! CLI tool for validating all built-in mark types.
//!
//! Runs `MarkValidator` and `MarkProfiler` on every built-in mark, printing a
//! human-readable report and exiting with a non-zero code if any mark fails
//! validation.
//!
//! Usage: `cargo run --bin validate_marks`
//! Or via mask: `mask validate-marks`

use gup::mark::validation::{MarkProfiler, MarkValidator, PerformanceClass, ValidationReport};
use gup::mark::{BoxPlot, Circle, Line, Path, Rectangle, Text};

use std::process::ExitCode;

/// Validate a single mark type and print the report.
///
/// Returns `true` if the mark passes validation.
fn validate_and_profile<M: gup::mark::Mark>(reports: &mut Vec<ValidationReport>) -> bool {
    let report = MarkValidator::<M>::validate();
    let profile = MarkProfiler::<M>::profile();

    let status = if report.is_passing() { "✅" } else { "❌" };
    let mark_name = report
        .mark_type
        .rsplit("::")
        .next()
        .unwrap_or(&report.mark_type);

    println!("{status} {mark_name}");

    // Print sections with issues
    for section in &report.sections {
        let section_status = if section.passed { "  ✅" } else { "  ❌" };
        println!("{section_status} {} ({:?})", section.name, section.duration);
        for issue in &section.issues {
            println!("     {issue}");
        }
    }

    // Print performance profile
    println!("  📊 Performance: {}", profile.classification);
    println!(
        "     Vertices: {} ({} bytes each, {} total)",
        profile.vertex_count, profile.vertex_size_bytes, profile.total_vertex_bytes
    );
    if let (Some(idx_count), Some(idx_bytes)) = (profile.index_count, profile.total_index_bytes) {
        println!("     Indices: {idx_count} ({idx_bytes} bytes total)");
    }
    println!(
        "     Generation time: {:?} (avg {:?}/iter over 100 runs)",
        profile.vertex_generation_time, profile.avg_vertex_gen_per_iteration
    );
    if profile.classification == PerformanceClass::NeedsWork {
        println!("     ⚠️  Performance needs improvement (>= 1ms per vertex generation)");
    }
    println!();

    let passing = report.is_passing();
    reports.push(report);
    passing
}

fn main() -> ExitCode {
    println!("╔══════════════════════════════════════╗");
    println!("║   Gup Mark Validation Report         ║");
    println!("╚══════════════════════════════════════╝");
    println!();

    let mut reports: Vec<ValidationReport> = Vec::new();
    let mut all_passing = true;

    // Validate all built-in mark types
    all_passing &= validate_and_profile::<Circle>(&mut reports);
    all_passing &= validate_and_profile::<Rectangle>(&mut reports);
    all_passing &= validate_and_profile::<Line>(&mut reports);
    all_passing &= validate_and_profile::<Path>(&mut reports);
    all_passing &= validate_and_profile::<BoxPlot>(&mut reports);
    all_passing &= validate_and_profile::<Text>(&mut reports);

    // Summary
    println!("═══════════════════════════════════════");
    let total = reports.len();
    let passed = reports.iter().filter(|r| r.is_passing()).count();
    let failed = total - passed;
    let total_issues: usize = reports.iter().map(|r| r.issue_count()).sum();
    let critical: usize = reports.iter().map(|r| r.critical_issues().count()).sum();
    let errors: usize = reports.iter().map(|r| r.errors().count()).sum();

    println!("Marks validated: {total}");
    println!("  Passed: {passed}");
    println!("  Failed: {failed}");
    println!("Total issues: {total_issues} ({critical} critical, {errors} errors)");

    if all_passing {
        println!("\n✅ All marks passed validation.");
        ExitCode::SUCCESS
    } else {
        println!("\n❌ Some marks failed validation!");
        ExitCode::FAILURE
    }
}

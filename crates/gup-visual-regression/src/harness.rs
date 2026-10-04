// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Runs every check for a case and reconciles the result with the tracked
//! expected-failure list.

use crate::checks::{
    Check, CheckFailure, Tolerances, check_color_present, check_marks_confined,
    check_marks_present, check_not_blank, check_text_present, is_applicable,
};
use crate::expected::{ExpectedFailures, Verdict};
use crate::golden::{BlessReason, GoldenOutcome, GoldenStore};
use crate::image::RgbaImage;
use crate::layout::LayoutMetadata;
use std::fmt;

/// Golden store + expected failures + structural tolerances.
#[derive(Clone, Debug)]
pub struct Harness {
    golden: GoldenStore,
    expected: ExpectedFailures,
    tolerances: Tolerances,
}

impl Harness {
    /// Create a harness with default structural tolerances.
    pub fn new(golden: GoldenStore, expected: ExpectedFailures) -> Self {
        Self {
            golden,
            expected,
            tolerances: Tolerances::default(),
        }
    }

    /// Override the structural tolerances.
    pub fn with_tolerances(mut self, tolerances: Tolerances) -> Self {
        self.tolerances = tolerances;
        self
    }

    /// The golden store.
    pub fn golden(&self) -> &GoldenStore {
        &self.golden
    }

    /// The tracked expected failures.
    pub fn expected(&self) -> &ExpectedFailures {
        &self.expected
    }

    /// The structural tolerances.
    pub fn tolerances(&self) -> &Tolerances {
        &self.tolerances
    }

    /// Run every applicable check (structural and golden) on a capture.
    ///
    /// `capture` is the renderer adapter's result: an image plus its layout
    /// metadata, or a description of why no image could be produced (which
    /// is a [`Check::Render`] failure).
    pub fn run(
        &self,
        case: &str,
        capture: Result<(RgbaImage, LayoutMetadata), String>,
    ) -> CaseReport {
        self.run_checks(case, capture, &Check::ALL)
    }

    /// Like [`run`](Self::run), but only for the listed checks
    /// ([`Check::Render`] always runs).
    pub fn run_checks(
        &self,
        case: &str,
        capture: Result<(RgbaImage, LayoutMetadata), String>,
        checks: &[Check],
    ) -> CaseReport {
        let mut executed = vec![Check::Render];
        let mut failures = Vec::new();
        let mut golden = None;

        match capture {
            Err(message) => failures.push(CheckFailure::new(Check::Render, message)),
            Ok((image, layout)) => {
                let tol = &self.tolerances;
                for &check in checks {
                    if check == Check::Render || !is_applicable(check, &layout) {
                        continue;
                    }
                    executed.push(check);
                    let result = match check {
                        Check::Render => unreachable!(),
                        Check::NotBlank => check_not_blank(&image, &layout, tol),
                        Check::MarksPresent => check_marks_present(&image, &layout, tol),
                        Check::TextPresent => check_text_present(&image, &layout, tol),
                        Check::MarksConfined => check_marks_confined(&image, &layout, tol),
                        Check::ColorPresent => check_color_present(&image, &layout, tol),
                        Check::Golden => self.golden.check(case, &image).map(|outcome| {
                            golden = Some(outcome);
                        }),
                    };
                    if let Err(failure) = result {
                        failures.push(failure);
                    }
                }
            }
        }

        let verdict = self.expected.reconcile(case, &executed, &failures);
        CaseReport {
            case: case.to_string(),
            executed,
            failures,
            verdict,
            golden,
        }
    }
}

/// Everything that happened when a case ran.
#[derive(Clone, Debug)]
pub struct CaseReport {
    /// The case name.
    pub case: String,
    /// Checks that ran.
    pub executed: Vec<Check>,
    /// Checks that failed (expected or not).
    pub failures: Vec<CheckFailure>,
    /// The failures classified against the expected-failure list.
    pub verdict: Verdict,
    /// The golden comparison result, if it ran and did not fail.
    pub golden: Option<GoldenOutcome>,
}

impl CaseReport {
    /// True when nothing unexpected happened.
    pub fn is_ok(&self) -> bool {
        self.verdict.is_ok()
    }

    /// Panic with the full report unless [`is_ok`](Self::is_ok); otherwise
    /// print the report (so expected failures stay visible in test output).
    pub fn assert_ok(&self) {
        if self.is_ok() {
            eprintln!("{self}");
        } else {
            panic!("{self}");
        }
    }
}

impl fmt::Display for CaseReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "visual case {}:", self.case)?;
        for &check in &self.executed {
            if let Some(fail) = self
                .verdict
                .unexpected_failures
                .iter()
                .find(|x| x.check == check)
            {
                writeln!(f, "  FAIL  {check}: {}", fail.message)?;
            } else if let Some((entry, fail)) = self
                .verdict
                .expected_failures
                .iter()
                .find(|(_, x)| x.check == check)
            {
                writeln!(
                    f,
                    "  XFAIL {check}: {} [tracked: {} — {}]",
                    fail.message, entry.tracking, entry.reason
                )?;
            } else if let Some(entry) = self
                .verdict
                .unexpected_passes
                .iter()
                .find(|e| e.check == check)
            {
                writeln!(
                    f,
                    "  XPASS {check}: passes now; remove the expected-failure entry for {:?} [{}] (tracked: {})",
                    entry.case, entry.check, entry.tracking
                )?;
            } else {
                match (check, &self.golden) {
                    (Check::Golden, Some(GoldenOutcome::Matched(stats))) => {
                        writeln!(f, "  PASS  {check}: {stats}")?
                    }
                    (Check::Golden, Some(GoldenOutcome::Blessed { path, reason })) => {
                        let why = match reason {
                            BlessReason::Missing => "new".to_string(),
                            BlessReason::SizeChanged => "size changed".to_string(),
                            BlessReason::Changed(stats) => format!("changed: {stats}"),
                        };
                        writeln!(f, "  BLESS {check}: wrote {} ({why})", path.display())?
                    }
                    _ => writeln!(f, "  PASS  {check}")?,
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::Rgba8;
    use crate::layout::{PxRect, TextRole};

    fn harness(expected: &str, name: &str) -> (Harness, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "gup-visual-regression-harness-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let store = GoldenStore::new(dir.join("golden"), dir.join("out")).with_bless(true);
        (
            Harness::new(store, ExpectedFailures::parse(expected).unwrap()),
            dir,
        )
    }

    /// Plot rect with a mark but a title region that has no text.
    fn capture() -> (RgbaImage, LayoutMetadata) {
        let mut img = RgbaImage::filled(60, 40, Rgba8::WHITE);
        img.fill_rect(
            PxRect::new(20.0, 15.0, 10.0, 10.0),
            Rgba8::from_hex(0xd62728),
        );
        let layout = LayoutMetadata::new(PxRect::new(10.0, 10.0, 40.0, 25.0))
            .with_text(
                TextRole::Title,
                "Title",
                PxRect::new(15.0, 0.0, 30.0, 8.0),
                Rgba8::BLACK,
            )
            .with_expected_color("fill", Rgba8::from_hex(0xd62728));
        (img, layout)
    }

    #[test]
    fn untracked_failure_fails_the_case() {
        let (h, dir) = harness("", "untracked");
        let report = h.run("suite/no_text", Ok(capture()));
        assert!(!report.is_ok());
        assert_eq!(
            report.verdict.unexpected_failures[0].check,
            Check::TextPresent
        );
        assert!(report.to_string().contains("FAIL  text_present"));
        // The golden image was blessed despite the structural failure.
        assert!(h.golden().golden_path("suite/no_text").exists());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn tracked_failure_is_reported_but_passes() {
        let (h, dir) = harness(
            "[[expected]]\ncase = \"suite/*\"\ncheck = \"text_present\"\nreason = \"no text\"\ntracking = \"S0a\"",
            "tracked",
        );
        let report = h.run("suite/no_text", Ok(capture()));
        assert!(report.is_ok(), "{report}");
        assert!(report.to_string().contains("XFAIL text_present"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn render_error_is_a_render_failure_and_skips_other_checks() {
        let (h, dir) = harness(
            "[[expected]]\ncase = \"suite/broken\"\ncheck = \"render\"\nreason = \"no render path\"\ntracking = \"S3\"",
            "render",
        );
        let report = h.run("suite/broken", Err("no raster render path".into()));
        assert_eq!(report.executed, vec![Check::Render]);
        assert!(report.is_ok(), "{report}");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn fixed_bug_is_an_unexpected_pass() {
        let (h, dir) = harness(
            "[[expected]]\ncase = \"suite/fine\"\ncheck = \"color_present\"\nreason = \"double gamma\"\ntracking = \"S0a\"",
            "xpass",
        );
        let (img, layout) = capture();
        let report = h.run_checks(
            "suite/fine",
            Ok((img, layout)),
            &[Check::NotBlank, Check::ColorPresent],
        );
        assert!(!report.is_ok());
        assert!(
            report.to_string().contains("XPASS color_present"),
            "{report}"
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}

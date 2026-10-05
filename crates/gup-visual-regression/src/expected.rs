// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tracked expected failures.
//!
//! Known-broken output is recorded in a TOML file instead of being hidden by
//! loosened thresholds or `#[ignore]`. Every case still runs every check;
//! the expected-failure list only changes how a failure is *reported*:
//!
//! - a failure with a matching entry is an **expected failure** (reported,
//!   not fatal);
//! - a failure without one is an **unexpected failure** (fatal);
//! - an entry whose check ran and passed is an **unexpected pass** (fatal:
//!   the bug was fixed, so the entry must be removed in the same change).
//!
//! ```toml
//! [[expected]]
//! case = "chart_builders/*"     # exact case name, or a prefix ending in '*'
//! check = "text_present"        # a `Check` name
//! reason = "PNG/texture output never draws text"
//! tracking = "RFC-001 S0a"      # the story or RFC step that removes this entry
//! ```

use crate::checks::{Check, CheckFailure};
use crate::golden::validate_case_name;
use std::fmt;
use std::path::Path;

/// One tracked expected failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExpectedFailure {
    /// Exact case name, or a prefix pattern ending in `*`.
    pub case: String,
    /// The check expected to fail.
    pub check: Check,
    /// What is broken.
    pub reason: String,
    /// The story or RFC step whose completion removes this entry.
    pub tracking: String,
}

impl ExpectedFailure {
    /// Whether this entry's case pattern matches `case`.
    pub fn matches_case(&self, case: &str) -> bool {
        match self.case.strip_suffix('*') {
            Some(prefix) => case.starts_with(prefix),
            None => self.case == case,
        }
    }
}

impl fmt::Display for ExpectedFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} [{}]: {} (tracked by {})",
            self.case, self.check, self.reason, self.tracking
        )
    }
}

/// A parse or validation error in an expected-failure file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExpectedFailuresError(pub String);

impl fmt::Display for ExpectedFailuresError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ExpectedFailuresError {}

/// The full tracked list.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExpectedFailures {
    entries: Vec<ExpectedFailure>,
}

impl ExpectedFailures {
    /// An empty list.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Load and validate a TOML file.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, ExpectedFailuresError> {
        let path = path.as_ref();
        let text = std::fs::read_to_string(path)
            .map_err(|e| ExpectedFailuresError(format!("{}: {e}", path.display())))?;
        Self::parse(&text)
            .map_err(|e| ExpectedFailuresError(format!("{}: {}", path.display(), e.0)))
    }

    /// Parse and validate TOML text.
    pub fn parse(text: &str) -> Result<Self, ExpectedFailuresError> {
        let err = |m: String| ExpectedFailuresError(m);
        let doc: toml_edit::DocumentMut = text.parse().map_err(|e| err(format!("{e}")))?;
        for (key, _) in doc.iter() {
            if key != "expected" {
                return Err(err(format!("unknown top-level key {key:?}")));
            }
        }
        let mut entries = Vec::new();
        if let Some(item) = doc.get("expected") {
            let tables = item.as_array_of_tables().ok_or_else(|| {
                err("`expected` must be an array of tables ([[expected]])".into())
            })?;
            for (i, table) in tables.iter().enumerate() {
                let field = |name: &str| -> Result<String, ExpectedFailuresError> {
                    let v = table
                        .get(name)
                        .and_then(|v| v.as_str())
                        .map(str::trim)
                        .unwrap_or_default();
                    if v.is_empty() {
                        Err(err(format!("entry {}: missing or empty `{name}`", i + 1)))
                    } else {
                        Ok(v.to_string())
                    }
                };
                for (key, _) in table.iter() {
                    if !matches!(key, "case" | "check" | "reason" | "tracking") {
                        return Err(err(format!("entry {}: unknown key {key:?}", i + 1)));
                    }
                }
                let case = field("case")?;
                let pattern = case.strip_suffix('*').unwrap_or(&case);
                let pattern = pattern.strip_suffix('/').unwrap_or(pattern);
                validate_case_name(pattern).map_err(|e| err(format!("entry {}: {e}", i + 1)))?;
                let check_name = field("check")?;
                let check = Check::from_name(&check_name).ok_or_else(|| {
                    err(format!(
                        "entry {}: unknown check {check_name:?} (expected one of: {})",
                        i + 1,
                        Check::ALL.map(Check::name).join(", ")
                    ))
                })?;
                let entry = ExpectedFailure {
                    case,
                    check,
                    reason: field("reason")?,
                    tracking: field("tracking")?,
                };
                if entries
                    .iter()
                    .any(|e: &ExpectedFailure| e.case == entry.case && e.check == entry.check)
                {
                    return Err(err(format!(
                        "entry {}: duplicate entry for {} [{}]",
                        i + 1,
                        entry.case,
                        entry.check
                    )));
                }
                entries.push(entry);
            }
        }
        Ok(Self { entries })
    }

    /// All entries.
    pub fn entries(&self) -> &[ExpectedFailure] {
        &self.entries
    }

    /// The entry (if any) expecting `check` to fail for `case`.
    pub fn lookup(&self, case: &str, check: Check) -> Option<&ExpectedFailure> {
        self.entries
            .iter()
            .find(|e| e.check == check && e.matches_case(case))
    }

    /// Classify the outcome of running `executed` checks on `case`, of which
    /// `failures` failed.
    pub fn reconcile(&self, case: &str, executed: &[Check], failures: &[CheckFailure]) -> Verdict {
        let mut verdict = Verdict::default();
        for failure in failures {
            match self.lookup(case, failure.check) {
                Some(entry) => verdict
                    .expected_failures
                    .push((entry.clone(), failure.clone())),
                None => verdict.unexpected_failures.push(failure.clone()),
            }
        }
        for &check in executed {
            if failures.iter().any(|f| f.check == check) {
                continue;
            }
            if let Some(entry) = self.lookup(case, check) {
                verdict.unexpected_passes.push(entry.clone());
            }
        }
        verdict
    }
}

/// The classified outcome of one case.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Verdict {
    /// Failures that match a tracked entry.
    pub expected_failures: Vec<(ExpectedFailure, CheckFailure)>,
    /// Failures with no tracked entry.
    pub unexpected_failures: Vec<CheckFailure>,
    /// Tracked entries whose check ran and passed.
    pub unexpected_passes: Vec<ExpectedFailure>,
}

impl Verdict {
    /// True when there are no unexpected failures and no unexpected passes.
    pub fn is_ok(&self) -> bool {
        self.unexpected_failures.is_empty() && self.unexpected_passes.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
# Known-broken output.
[[expected]]
case = "chart_builders/*"
check = "text_present"
reason = "no text in PNG output"
tracking = "RFC-001 S0a"

[[expected]]
case = "chart_builders/area"
check = "marks_confined"
reason = "area fan"
tracking = "GUP-393"
"#;

    #[test]
    fn parses_entries_and_matches_prefix_patterns() {
        let list = ExpectedFailures::parse(SAMPLE).unwrap();
        assert_eq!(list.entries().len(), 2);
        assert!(
            list.lookup("chart_builders/bar", Check::TextPresent)
                .is_some()
        );
        assert!(list.lookup("examples/bar", Check::TextPresent).is_none());
        assert!(
            list.lookup("chart_builders/area", Check::MarksConfined)
                .is_some()
        );
        assert!(
            list.lookup("chart_builders/area_x", Check::MarksConfined)
                .is_none()
        );
    }

    #[test]
    fn rejects_bad_entries() {
        let bad = [
            (
                "[[expected]]\ncase = \"a\"\ncheck = \"nope\"\nreason = \"r\"\ntracking = \"t\"",
                "unknown check",
            ),
            (
                "[[expected]]\ncase = \"a\"\ncheck = \"golden\"\nreason = \"\"\ntracking = \"t\"",
                "empty `reason`",
            ),
            (
                "[[expected]]\ncase = \"a\"\ncheck = \"golden\"\nreason = \"r\"",
                "`tracking`",
            ),
            (
                "[[expected]]\ncase = \"a b\"\ncheck = \"golden\"\nreason = \"r\"\ntracking = \"t\"",
                "invalid case",
            ),
            (
                "[[expected]]\ncase = \"a\"\ncheck = \"golden\"\nreason = \"r\"\ntracking = \"t\"\nextra = 1",
                "unknown key",
            ),
            (
                "[[expected]]\ncase = \"a\"\ncheck = \"golden\"\nreason = \"r\"\ntracking = \"t\"\n[[expected]]\ncase = \"a\"\ncheck = \"golden\"\nreason = \"r\"\ntracking = \"t\"",
                "duplicate",
            ),
            ("other = 1", "unknown top-level"),
        ];
        for (text, needle) in bad {
            let e = ExpectedFailures::parse(text).unwrap_err();
            assert!(e.0.contains(needle), "{needle:?} not in {e}");
        }
    }

    #[test]
    fn reconcile_classifies_expected_unexpected_and_fixed() {
        let list = ExpectedFailures::parse(SAMPLE).unwrap();
        let text = CheckFailure::new(Check::TextPresent, "no text");
        let blank = CheckFailure::new(Check::NotBlank, "blank");

        // Text missing (tracked) and marks confined (tracked) but passed.
        let v = list.reconcile(
            "chart_builders/area",
            &[Check::NotBlank, Check::TextPresent, Check::MarksConfined],
            std::slice::from_ref(&text),
        );
        assert_eq!(v.expected_failures.len(), 1);
        assert!(v.unexpected_failures.is_empty());
        assert_eq!(v.unexpected_passes.len(), 1, "fixed bug must be reported");
        assert!(!v.is_ok());

        // An untracked failure is fatal.
        let v = list.reconcile("chart_builders/bar", &[Check::NotBlank], &[blank]);
        assert_eq!(v.unexpected_failures.len(), 1);
        assert!(!v.is_ok());

        // A tracked check that did not run is neither a pass nor a failure.
        let v = list.reconcile("chart_builders/bar", &[Check::NotBlank], &[]);
        assert!(v.is_ok());
    }
}

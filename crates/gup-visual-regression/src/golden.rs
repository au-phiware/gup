// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Golden-image storage, comparison and the `GUP_BLESS` workflow.

use crate::checks::{Check, CheckFailure};
use crate::diff::{DiffStats, DiffTolerance, diff_image, perceptual_diff};
use crate::image::RgbaImage;
use std::path::{Path, PathBuf};

/// Environment variable that switches golden comparison into bless mode.
pub const BLESS_ENV: &str = "GUP_BLESS";

/// Whether bless mode was requested (`GUP_BLESS=1`, `true` or `yes`).
pub fn bless_requested() -> bool {
    std::env::var(BLESS_ENV)
        .is_ok_and(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes"))
}

/// Why a golden image was (re)written in bless mode.
#[derive(Clone, Debug, PartialEq)]
pub enum BlessReason {
    /// There was no golden image.
    Missing,
    /// The golden image has different dimensions.
    SizeChanged,
    /// The golden image differed beyond tolerance.
    Changed(DiffStats),
}

/// Result of a successful golden comparison.
#[derive(Clone, Debug, PartialEq)]
pub enum GoldenOutcome {
    /// The image matched its golden image within tolerance.
    Matched(DiffStats),
    /// Bless mode wrote a new golden image.
    Blessed {
        /// The golden file written.
        path: PathBuf,
        /// Why it was written.
        reason: BlessReason,
    },
}

/// A directory of golden PNGs keyed by case name, plus a scratch directory
/// for the latest actual renders and diff images.
///
/// Case names are `/`-separated paths of `[A-Za-z0-9_-]` segments, e.g.
/// `chart_builders/scatter` → `<golden_dir>/chart_builders/scatter.png`.
#[derive(Clone, Debug)]
pub struct GoldenStore {
    golden_dir: PathBuf,
    artifact_dir: PathBuf,
    tolerance: DiffTolerance,
    bless: bool,
}

impl GoldenStore {
    /// Create a store. Bless mode is read from [`BLESS_ENV`].
    pub fn new(golden_dir: impl Into<PathBuf>, artifact_dir: impl Into<PathBuf>) -> Self {
        Self {
            golden_dir: golden_dir.into(),
            artifact_dir: artifact_dir.into(),
            tolerance: DiffTolerance::default(),
            bless: bless_requested(),
        }
    }

    /// Override the perceptual-diff tolerance.
    pub fn with_tolerance(mut self, tolerance: DiffTolerance) -> Self {
        self.tolerance = tolerance;
        self
    }

    /// Force bless mode on or off (instead of reading the environment).
    pub fn with_bless(mut self, bless: bool) -> Self {
        self.bless = bless;
        self
    }

    /// The perceptual-diff tolerance in use.
    pub fn tolerance(&self) -> &DiffTolerance {
        &self.tolerance
    }

    /// Whether bless mode is on.
    pub fn is_blessing(&self) -> bool {
        self.bless
    }

    /// Where the golden image for `case` lives.
    pub fn golden_path(&self, case: &str) -> PathBuf {
        self.golden_dir.join(format!("{case}.png"))
    }

    /// Where the latest actual render for `case` is written.
    pub fn actual_path(&self, case: &str) -> PathBuf {
        self.artifact_dir.join(format!("{case}.png"))
    }

    /// Where the diff image for `case` is written on mismatch.
    pub fn diff_path(&self, case: &str) -> PathBuf {
        self.artifact_dir.join(format!("{case}.diff.png"))
    }

    /// Compare `actual` with the golden image for `case`.
    ///
    /// The actual render is always saved to [`actual_path`](Self::actual_path)
    /// so it can be inspected. In bless mode a missing or out-of-tolerance
    /// golden image is overwritten (an in-tolerance one is left alone, to
    /// avoid churn from backend noise). Otherwise a missing or mismatched
    /// golden image is a [`Check::Golden`] failure, and a diff image is
    /// written next to the actual render.
    pub fn check(&self, case: &str, actual: &RgbaImage) -> Result<GoldenOutcome, CheckFailure> {
        let fail = |msg: String| CheckFailure::new(Check::Golden, msg);
        validate_case_name(case).map_err(fail)?;

        let actual_path = self.actual_path(case);
        actual
            .save_png(&actual_path)
            .map_err(|e| fail(format!("could not save actual render: {e}")))?;

        let golden_path = self.golden_path(case);
        let golden = if golden_path.exists() {
            Some(
                RgbaImage::load_png(&golden_path)
                    .map_err(|e| fail(format!("could not read golden image: {e}")))?,
            )
        } else {
            None
        };

        let comparison = golden
            .as_ref()
            .map(|g| perceptual_diff(g, actual, &self.tolerance));
        let reason = match comparison {
            None => BlessReason::Missing,
            Some(Err(_)) => BlessReason::SizeChanged,
            Some(Ok(stats)) if stats.within(&self.tolerance) => {
                return Ok(GoldenOutcome::Matched(stats));
            }
            Some(Ok(stats)) => BlessReason::Changed(stats),
        };

        if self.bless {
            actual
                .save_png(&golden_path)
                .map_err(|e| fail(format!("could not bless golden image: {e}")))?;
            return Ok(GoldenOutcome::Blessed {
                path: golden_path,
                reason,
            });
        }

        let hint = format!(
            "If the change is intended, re-run with {BLESS_ENV}=1 and commit the updated golden image."
        );
        Err(fail(match (reason, golden) {
            (BlessReason::Missing, _) => format!(
                "no golden image at {}; actual render saved to {}. {hint}",
                golden_path.display(),
                actual_path.display()
            ),
            (BlessReason::SizeChanged, Some(g)) => format!(
                "golden {} is {}x{} but render is {}x{}; actual saved to {}. {hint}",
                golden_path.display(),
                g.width(),
                g.height(),
                actual.width(),
                actual.height(),
                actual_path.display()
            ),
            (BlessReason::Changed(stats), Some(g)) => {
                let diff_path = self.diff_path(case);
                let diff_note = match diff_image(&g, actual, &self.tolerance)
                    .map_err(|e| e.to_string())
                    .and_then(|d| d.save_png(&diff_path).map_err(|e| e.to_string()))
                {
                    Ok(()) => format!("diff {}", diff_path.display()),
                    Err(e) => format!("diff image not written: {e}"),
                };
                format!(
                    "render differs from golden beyond tolerance (ΔE > {}, max {:.2}% of pixels): {stats}. golden {}, actual {}, {diff_note}. {hint}",
                    self.tolerance.pixel_delta_e,
                    self.tolerance.max_differing_fraction * 100.0,
                    golden_path.display(),
                    actual_path.display(),
                )
            }
            (_, None) => unreachable!("size/changed reasons imply a golden image"),
        }))
    }
}

/// Validate a case name: `/`-separated, non-empty `[A-Za-z0-9_-]` segments.
pub fn validate_case_name(case: &str) -> Result<(), String> {
    let ok = !case.is_empty()
        && case.split('/').all(|seg| {
            !seg.is_empty()
                && seg
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        });
    if ok {
        Ok(())
    } else {
        Err(format!(
            "invalid case name {case:?}: use '/'-separated [A-Za-z0-9_-] segments"
        ))
    }
}

/// Resolve the directory for scratch artifacts under a Cargo target
/// directory: `<target dir>/visual-regression` (see [`target_dir`]).
pub fn default_artifact_dir(workspace_root: &Path) -> PathBuf {
    target_dir(workspace_root).join("visual-regression")
}

/// The Cargo target directory: `$CARGO_TARGET_DIR` (or
/// `$CARGO_BUILD_TARGET_DIR`) if set, resolved against `workspace_root` when
/// relative, else `<workspace_root>/target`.
///
/// This is where Cargo puts final artifacts under their own names (example
/// binaries in `<profile>/examples/`). Do not derive it from a test binary's
/// location: with a separate build directory (`CARGO_BUILD_BUILD_DIR`) test
/// binaries and other intermediate artifacts live there instead.
pub fn target_dir(workspace_root: &Path) -> PathBuf {
    std::env::var_os("CARGO_TARGET_DIR")
        .or_else(|| std::env::var_os("CARGO_BUILD_TARGET_DIR"))
        .map(|dir| workspace_root.join(dir))
        .unwrap_or_else(|| workspace_root.join("target"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::Rgba8;
    use crate::layout::PxRect;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "gup-visual-regression-test-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn image(offset: f32) -> RgbaImage {
        let mut img = RgbaImage::filled(40, 30, Rgba8::WHITE);
        img.fill_rect(PxRect::new(5.0 + offset, 5.0, 10.0, 10.0), Rgba8::BLACK);
        img
    }

    #[test]
    fn missing_golden_fails_then_bless_creates_it_then_it_matches() {
        let dir = scratch("bless");
        let store = GoldenStore::new(dir.join("golden"), dir.join("out")).with_bless(false);

        let err = store.check("suite/case_a", &image(0.0)).unwrap_err();
        assert_eq!(err.check, Check::Golden);
        assert!(err.message.contains("no golden image"), "{err}");
        assert!(err.message.contains(BLESS_ENV), "{err}");
        assert!(store.actual_path("suite/case_a").exists());

        let blessing = store.clone().with_bless(true);
        let outcome = blessing.check("suite/case_a", &image(0.0)).unwrap();
        assert!(matches!(
            outcome,
            GoldenOutcome::Blessed {
                reason: BlessReason::Missing,
                ..
            }
        ));
        assert!(store.golden_path("suite/case_a").exists());

        assert!(matches!(
            store.check("suite/case_a", &image(0.0)).unwrap(),
            GoldenOutcome::Matched(_)
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn changed_render_fails_with_diff_and_bless_rewrites() {
        let dir = scratch("changed");
        let store = GoldenStore::new(dir.join("golden"), dir.join("out")).with_bless(true);
        store.check("c", &image(0.0)).unwrap();

        let store = store.with_bless(false);
        let err = store.check("c", &image(15.0)).unwrap_err();
        assert!(err.message.contains("differs from golden"), "{err}");
        assert!(store.diff_path("c").exists());

        let outcome = store
            .clone()
            .with_bless(true)
            .check("c", &image(15.0))
            .unwrap();
        assert!(matches!(
            outcome,
            GoldenOutcome::Blessed {
                reason: BlessReason::Changed(_),
                ..
            }
        ));
        assert!(matches!(
            store.check("c", &image(15.0)).unwrap(),
            GoldenOutcome::Matched(_)
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn size_change_is_reported() {
        let dir = scratch("size");
        let store = GoldenStore::new(dir.join("golden"), dir.join("out")).with_bless(true);
        store.check("s", &image(0.0)).unwrap();
        let err = store
            .with_bless(false)
            .check("s", &RgbaImage::filled(10, 10, Rgba8::WHITE))
            .unwrap_err();
        assert!(
            err.message.contains("is 40x30 but render is 10x10"),
            "{err}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn case_names_are_validated() {
        assert!(validate_case_name("chart_builders/scatter").is_ok());
        assert!(validate_case_name("examples/03_line-chart").is_ok());
        for bad in ["", "a//b", "../x", "a b", "/a", "a/"] {
            assert!(validate_case_name(bad).is_err(), "{bad:?}");
        }
    }
}

// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Target-agnostic visual regression harness.
//!
//! Every check takes a plain [`RgbaImage`] and a [`LayoutMetadata`] that
//! describes where the renderer *intended* things to be (plot rectangle,
//! text regions, guide geometry, configured colours). The crate has no
//! dependency on any renderer, so the same harness validates the current
//! chart builders today and any future core render path, each through a
//! small adapter of its own that produces those two values.
//!
//! The pieces:
//!
//! - [`checks`]: structural assertions (not blank, marks present, text
//!   present, marks confined to the plot, configured colour present);
//! - [`diff`]: per-pixel CIEDE2000 perceptual diff;
//! - [`golden`]: golden-image storage and the `GUP_BLESS=1` workflow;
//! - [`expected`]: the tracked expected-failure list;
//! - [`Harness`]: runs all of the above for a named case and reports.
//!
//! ```
//! use gup_visual_regression::*;
//!
//! let mut image = RgbaImage::filled(100, 60, Rgba8::WHITE);
//! image.fill_rect(PxRect::new(40.0, 20.0, 10.0, 10.0), Rgba8::from_hex(0x1f77b4));
//! let layout = LayoutMetadata::new(PxRect::new(10.0, 10.0, 80.0, 40.0))
//!     .with_expected_color("point fill", Rgba8::from_hex(0x1f77b4));
//!
//! let tol = Tolerances::default();
//! checks::check_not_blank(&image, &layout, &tol).unwrap();
//! checks::check_marks_confined(&image, &layout, &tol).unwrap();
//! checks::check_color_present(&image, &layout, &tol).unwrap();
//! ```

pub mod checks;
pub mod color;
pub mod diff;
pub mod expected;
pub mod golden;
mod harness;
pub mod image;
pub mod layout;

pub use checks::{Check, CheckFailure, Tolerances};
pub use color::Rgba8;
pub use diff::{DiffStats, DiffTolerance};
pub use expected::{ExpectedFailure, ExpectedFailures, Verdict};
pub use golden::{BLESS_ENV, GoldenOutcome, GoldenStore};
pub use harness::{CaseReport, Harness};
pub use image::RgbaImage;
pub use layout::{ExpectedColor, LayoutMetadata, PxRect, TextRegion, TextRole};

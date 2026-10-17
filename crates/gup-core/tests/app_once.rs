// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! `show` runs once per process; the second call is an error, not a panic.
//! Needs a display, so it is ignored by default:
//!
//! ```text
//! cargo test -p gup-core --test app_once -- --ignored
//! ```
//!
//! winit allows one event loop per process, so this file holds one test.

#![cfg(feature = "window")]

use gup_core::prelude::*;

fn chart() -> Plot {
    let mut plot = Plot::new();
    plot.circles([(1.0_f64, 2.0_f64), (2.0, 3.0)])
        .x(|p| p.0)
        .y(|p| p.1);
    plot
}

#[test]
#[ignore = "needs a display"]
fn show_twice_fails_cleanly() {
    let path = std::env::temp_dir().join("gup_app_once.png");
    // SAFETY: this file holds one test, so no other thread reads the environment.
    unsafe { std::env::set_var("GUP_SCREENSHOT_PATH", &path) };
    gup_core::show(chart()).expect("the first show");
    let err = gup_core::show(chart()).expect_err("the second show fails");
    assert!(
        matches!(err, gup_core::Error::Configuration { .. })
            && err.to_string().contains("one event loop per process"),
        "{err}"
    );
}

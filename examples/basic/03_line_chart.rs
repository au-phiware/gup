// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! A line chart with two series, split by colour.
//!
//! Set `GUP_SCREENSHOT_PATH=out.png` to render one frame headless.

use gup_core::prelude::*;

struct Sample {
    t: f64,
    value: f64,
    series: &'static str,
}

fn main() -> gup_core::Result<()> {
    let rows = (0..40).flat_map(|i| {
        let t = f64::from(i) / 4.0;
        [
            Sample {
                t,
                value: t.sin(),
                series: "sin",
            },
            Sample {
                t,
                value: t.cos(),
                series: "cos",
            },
        ]
    });
    let chart = gup_core::line(rows)
        .x(|s| s.t)
        .y(|s| s.value)
        .color(|s| s.series)
        .width(2.0)
        .grid(Axes::Y)
        .title("Two series");
    gup_core::show(chart)
}

// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! One plot, two row types: observations as circles and a fitted line of tuples.
//!
//! Set `GUP_SCREENSHOT_PATH=out.png` to render one frame headless.

use gup::prelude::*;

struct Observation {
    x: f64,
    y: f64,
}

fn main() -> gup::Result<()> {
    let observations: Vec<Observation> = (0..60)
        .map(|i| {
            let x = f64::from(i) / 6.0;
            Observation {
                x,
                y: 2.0 * x + 5.0 + 3.0 * ((f64::from(i) * 12.9898).sin() * 43758.5).fract(),
            }
        })
        .collect();
    let fit = [(0.0, 5.0), (10.0, 25.0)];

    let mut plot = Plot::new();
    plot.title("Observations and fit");
    plot.circles(observations)
        .x(|o| o.x)
        .y(|o| o.y)
        .radius(Px(3.0));
    plot.line(fit).x(|f| f.0).y(|f| f.1).width(Px(2.0));
    gup::show(plot)
}

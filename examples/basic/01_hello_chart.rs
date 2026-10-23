// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! The simplest Gup example: a scatter chart of five points.
//!
//! Set `GUP_SCREENSHOT_PATH=out.png` to render one frame headless.

struct Point {
    x: f64,
    y: f64,
}

fn main() -> gup::Result<()> {
    let data =
        [(1.0, 2.0), (2.0, 4.0), (3.0, 3.0), (4.0, 5.0), (5.0, 4.5)].map(|(x, y)| Point { x, y });
    let chart = gup::scatter(data)
        .x(|p| p.x)
        .y(|p| p.y)
        .radius(5.0)
        .title("Hello, chart");
    gup::show(chart)
}

// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Hello, Gup: a sine wave in a window. Hover a point, scroll to zoom, Escape to close.

use gup::prelude::*;

fn main() -> gup::Result<()> {
    let points = (0..50).map(|i| f64::from(i) / 5.0).map(|x| (x, x.sin()));
    let mut plot = Plot::new();
    plot.circles(points)
        .x(|p| p.0)
        .y(|p| p.1)
        .tooltip(|p| format!("sin({:.1}) = {:.2}", p.0, p.1));
    gup::show(plot)
}

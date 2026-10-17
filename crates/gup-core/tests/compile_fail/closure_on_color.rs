// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// A plain closure drives a position channel through the plot's scale
// (`.x(|r| r.t)`), but a colour channel has no such default: it needs a
// scale, `scale.encode(..)`, or a constant colour.
use gup_core::prelude::*;

struct Row {
    v: f64,
}

fn main() {
    let mut plot = Plot::new();
    plot.circles(vec![Row { v: 1.0 }]).fill(|r: &Row| r.v);
}

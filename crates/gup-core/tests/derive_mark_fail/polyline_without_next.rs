// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// A `Polyline` mark whose WGSL input struct does not read its position
// channels at the next row (`x_next`, `y_next`).
use gup_core::prelude::*;

#[derive(Debug, Mark)]
#[mark(module = "gup::marks::segment", geometry = Polyline)]
struct Segment {
    #[channel(role = X)]
    x: Px,
    #[channel(role = Y)]
    y: Px,
    x_end: Px,
    y_end: Px,
    width: Px,
    color: Color,
}

fn main() {}

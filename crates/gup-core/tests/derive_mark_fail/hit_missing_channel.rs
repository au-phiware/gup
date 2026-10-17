// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// `hit = Disc` reads `radius`, which this mark does not have.
use gup_core::prelude::*;

#[derive(Debug, Mark)]
#[mark(module = "gup::marks::segment", geometry = Quad, hit = Disc)]
struct Segment {
    #[channel(role = X)]
    x: Px,
    #[channel(role = Y)]
    y: Px,
    #[channel(role = X)]
    x_end: Px,
    #[channel(role = Y)]
    y_end: Px,
    width: Px,
    color: Color,
}

fn main() {}

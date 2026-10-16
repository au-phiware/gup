// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// A channel the WGSL `CircleIn` lacks.
use gup_core::prelude::*;

#[derive(Debug, Mark)]
#[mark(module = "gup::marks::circle", geometry = Quad)]
struct Circle {
    x: Px,
    y: Px,
    radius: Px,
    fill: Color,
    fill_opacity: Unit,
    stroke: Color,
    stroke_width: Px,
    outline: Color,
}

fn main() {}

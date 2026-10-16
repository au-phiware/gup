// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// A `Px` field where the WGSL has `vec4<f32>`.
use gup_core::prelude::*;

#[derive(Debug, Mark)]
#[mark(module = "gup::marks::circle", geometry = Quad)]
struct Circle {
    x: Px,
    y: Px,
    radius: Px,
    fill: Px,
}

fn main() {}

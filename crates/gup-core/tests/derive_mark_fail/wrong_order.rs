// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// Channels in a different order than the WGSL members.
use gup_core::prelude::*;

#[derive(Debug, Mark)]
#[mark(module = "gup::marks::circle", geometry = Quad)]
struct Circle {
    x: Px,
    y: Px,
    fill: Color,
    radius: Px,
}

fn main() {}

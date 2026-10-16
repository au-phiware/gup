// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// A module that is not in Gup's library.
use gup_core::prelude::*;

#[derive(Debug, Mark)]
#[mark(module = "gup::marks::nope", geometry = Quad)]
struct Circle {
    x: Px,
    y: Px,
    radius: Px,
    fill: Color,
}

fn main() {}

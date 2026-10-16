// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// A field named like a `Selection` method.
use gup_core::prelude::*;

#[derive(Debug, Mark)]
#[mark(module = "gup::marks::circle", geometry = Quad)]
struct Circle {
    x: Px,
    y: Px,
    len: Px,
    fill: Color,
}

fn main() {}

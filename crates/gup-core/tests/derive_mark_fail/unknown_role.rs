// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// A `role` that no channel accepts: the error lists every accepted role.
use gup_core::prelude::*;

#[derive(Debug, Mark)]
#[mark(module = "gup::marks::circle", geometry = Quad)]
struct Circle {
    #[channel(role = X)]
    x: Px,
    #[channel(role = Depth)]
    y: Px,
    radius: Px,
    fill: Color,
}

fn main() {}

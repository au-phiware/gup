// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// `hit = Area` reads `y_end`, which this mark does not have.
use gup_core::prelude::*;

#[derive(Debug, Mark)]
#[mark(module = "gup::marks::line", geometry = Polyline, hit = Area)]
struct Line {
    #[channel(role = X)]
    x: Px,
    #[channel(role = Y)]
    y: Px,
    width: Px,
    color: Color,
}

fn main() {}

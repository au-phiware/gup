// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// A colour scale cannot drive a pixel-valued channel.
use gup_core::prelude::*;

struct Row {
    temp: f64,
}

fn main() {
    let mut sel = Selection::<Row, Circle>::new(vec![Row { temp: 1.0 }]);
    sel.attr(Circle::RADIUS, Sequential::viridis().encode(|r: &Row| r.temp));
}

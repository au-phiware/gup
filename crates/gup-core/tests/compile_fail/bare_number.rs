// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// A bare number has no unit: pixel channels need `Px(..)`. This is where
// `#[diagnostic::on_unimplemented]` on `IntoEncoding` applies.
use gup_core::prelude::*;

struct Row;

fn main() {
    let mut sel = Selection::<Row, Circle>::new(vec![Row]);
    sel.attr(Circle::RADIUS, 3.0f32);
}

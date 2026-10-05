// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// A misspelt channel is "no associated item", not a silently ignored name.
use gup_core::prelude::*;

struct Row;

fn main() {
    let mut sel = Selection::<Row, Circle>::new(vec![Row]);
    sel.attr(Circle::RADUIS, Px(3.0));
}

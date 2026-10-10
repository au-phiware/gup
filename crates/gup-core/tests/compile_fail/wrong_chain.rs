// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// A chain's links must fit: a colour cannot feed a scale over numbers.
use gup_core::prelude::*;

fn main() {
    let _ = Sequential::viridis().then(Linear::new());
}

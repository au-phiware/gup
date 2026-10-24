// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// RFC-001 §12 risk 4: `encode`'s accessor is `Fn(&T) -> D`, whose `D`
// cannot borrow from the row, and a string is not a column value. The same
// closure compiles through `encode_key` (tests/compile_pass/key_accessor.rs).
// The encoding is not passed to `attr`, so the bound fails once, in `encode`:
// a second error there would carry an `IntoEncoding` hint that changes with
// every new `IntoEncoding` impl (GUP-511).
use gup_core::prelude::*;

struct Row {
    continent: String,
}

fn main() {
    let _ = Categorical::okabe_ito().encode(|r: &Row| r.continent.as_str());
}

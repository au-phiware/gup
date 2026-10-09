// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// RFC-001 §12 risk 4, fixed: `encode_key`'s accessor is
// `for<'a> Fn(&'a T) -> &'a str`, so the closure that fails through
// `encode` (tests/compile_fail/borrowing_accessor.rs) compiles, and so does
// its nullable form.
use gup_core::prelude::*;

struct Row {
    continent: String,
    region: Option<String>,
}

fn main() {
    let mut sel = Selection::<Row, Circle>::new(vec![Row {
        continent: "Asia".into(),
        region: None,
    }]);
    sel.attr(
        Circle::FILL,
        Categorical::okabe_ito().encode_key(|r: &Row| r.continent.as_str()),
    );
    sel.attr(
        Circle::FILL,
        Categorical::okabe_ito().encode_nullable_key(|r: &Row| r.region.as_deref()),
    );
    assert_eq!(sel.len(), 1);
}

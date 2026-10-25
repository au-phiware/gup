// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// GUP-537: a builder key accessor returns an owned key, so a closure that
// borrows its key from the row (`r.name.as_str()`) fails. rustc gives no
// custom message here: `#[diagnostic::on_unimplemented]` fires only on an
// unmet trait bound, and type checking erases lifetimes, so `&'a str` meets
// `DictionaryKey` exactly as `&'static str` does. The error is the borrow
// checker's, at the closure's return expression. The docs of every key
// setter carry the advice: return `r.name.clone()`.

struct Row {
    name: String,
}

fn main() {
    let _ = gup_core::bar(Vec::<Row>::new()).x(|r| r.name.as_str());
}

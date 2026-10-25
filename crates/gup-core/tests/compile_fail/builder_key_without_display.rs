// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// GUP-537: a builder key must be a `DictionaryKey` (`Hash + Eq + Clone +
// Display`). An enum without `Display` gets the custom "not a category key"
// message from the attribute on `DictionaryKey`. (`f64` would print it twice,
// once for `Hash` and once for `Eq`, each with rustc's std-type list.)

#[derive(Clone, Copy, Hash, PartialEq, Eq)]
enum Region {
    North,
}

struct Row {
    region: Region,
}

fn main() {
    let _ = gup_core::bar(Vec::<Row>::new()).x(|r| r.region);
}

// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// GUP-537: every builder key setter takes an owned key. An integer, a `Copy`
// enum with `Display`, a cloned `String`, a `&'static str` field and a literal
// all compile with no closure annotations and no turbofish.
use std::fmt;

use gup_core::prelude::*;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Region {
    North,
    South,
}

impl fmt::Display for Region {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Region::North => "North",
            Region::South => "South",
        })
    }
}

struct Row {
    year: i32,
    region: Region,
    name: String,
    tag: &'static str,
    v: f64,
}

fn main() {
    let rows = || Vec::<Row>::new();

    let _ = gup_core::scatter(rows()).x(|r| r.v).y(|r| r.v).color(|r| r.year);
    let _ = gup_core::scatter(rows()).color(|r| r.region);
    let _ = gup_core::scatter(rows()).color(|r| r.name.clone());
    let _ = gup_core::scatter(rows()).color(|r| r.tag);
    let _ = gup_core::scatter(rows()).color(|_| "all");

    let _ = gup_core::line(rows()).x(|r| r.v).y(|r| r.v).color(|r| r.year);
    let _ = gup_core::line(rows()).color(|r| r.region);
    let _ = gup_core::line(rows()).color(|r| r.name.clone());
    let _ = gup_core::line(rows()).color(|r| r.tag);
    let _ = gup_core::line(rows()).color(|_| "all");

    let _ = gup_core::bar(rows()).x(|r| r.year).y(|r| r.v);
    let _ = gup_core::bar(rows()).x(|r| r.region);
    let _ = gup_core::bar(rows()).x(|r| r.name.clone());
    let _ = gup_core::bar(rows()).x(|r| r.tag);
    let _ = gup_core::bar(rows()).x(|_| "all");

    let _ = gup_core::bar(rows()).color(|r| r.year);
    let _ = gup_core::bar(rows()).color(|r| r.region);
    let _ = gup_core::bar(rows()).color(|r| r.name.clone());
    let _ = gup_core::bar(rows()).color(|r| r.tag);
    let _ = gup_core::bar(rows()).color(|_| "all");

    let _ = gup_core::bar(rows()).group(|r| r.year);
    let _ = gup_core::bar(rows()).group(|r| r.region);
    let _ = gup_core::bar(rows()).group(|r| r.name.clone());
    let _ = gup_core::bar(rows()).group(|r| r.tag);
    let _ = gup_core::bar(rows()).group(|_| "all");

    let _ = gup_core::bar(rows()).stack(|r| r.year);
    let _ = gup_core::bar(rows()).stack(|r| r.region);
    let _ = gup_core::bar(rows()).stack(|r| r.name.clone());
    let _ = gup_core::bar(rows()).stack(|r| r.tag);
    let _ = gup_core::bar(rows()).stack(|_| "all");
}

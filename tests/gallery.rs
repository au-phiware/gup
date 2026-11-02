// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! The gallery examples (`examples/gallery/`), checked as goldens at the
//! gallery's reading size. Bless with
//! `GUP_BLESS=1 cargo test -p gup --test gallery -- --test-threads=1`.

mod common;

use common::check;

#[path = "../examples/gallery/scatter_bubbles.rs"]
#[allow(dead_code)]
mod scatter_bubbles;

#[path = "../examples/gallery/line_monotone.rs"]
#[allow(dead_code)]
mod line_monotone;

#[path = "../examples/gallery/bar_grouped_labels.rs"]
#[allow(dead_code)]
mod bar_grouped_labels;

#[path = "../examples/gallery/barh_ranked.rs"]
#[allow(dead_code)]
mod barh_ranked;

#[path = "../examples/gallery/area_stacked.rs"]
#[allow(dead_code)]
mod area_stacked;

#[test]
fn scatter_bubbles() {
    check(
        "gallery/scatter_bubbles",
        &mut scatter_bubbles::chart(),
        800,
        500,
    );
}

#[test]
fn line_monotone() {
    check(
        "gallery/line_monotone",
        &mut line_monotone::chart(),
        800,
        500,
    );
}

#[test]
fn bar_grouped_labels() {
    check(
        "gallery/bar_grouped_labels",
        &mut bar_grouped_labels::chart(),
        800,
        500,
    );
}

#[test]
fn barh_ranked() {
    check("gallery/barh_ranked", &mut barh_ranked::chart(), 800, 500);
}

#[test]
fn area_stacked() {
    check("gallery/area_stacked", &mut area_stacked::chart(), 800, 500);
}

// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! The gallery examples (`examples/gallery/`), checked as goldens at the
//! gallery's reading size. Bless with
//! `GUP_BLESS=1 cargo test -p gup --test gallery -- --test-threads=1`.

mod common;

use common::{check, check_on};
use gup::prelude::*;
use gup_visual_regression::Rgba8;

/// The dark theme's background, as the harness's layout background.
fn dark_background() -> Rgba8 {
    let [r, g, b, _] = Theme::dark().background.to_rgba8();
    Rgba8::rgb(r, g, b)
}

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

#[path = "../examples/gallery/histogram_stacked.rs"]
#[allow(dead_code)]
mod histogram_stacked;

#[path = "../examples/gallery/heatmap_weekly.rs"]
#[allow(dead_code)]
mod heatmap_weekly;

#[path = "../examples/gallery/boxplot_latency.rs"]
#[allow(dead_code)]
mod boxplot_latency;

#[path = "../examples/gallery/annotated_prices.rs"]
#[allow(dead_code)]
mod annotated_prices;

#[path = "../examples/gallery/dark_multiline.rs"]
#[allow(dead_code)]
mod dark_multiline;

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

#[test]
fn histogram_stacked() {
    check(
        "gallery/histogram_stacked",
        &mut histogram_stacked::chart(),
        800,
        500,
    );
}

#[test]
fn heatmap_weekly() {
    check(
        "gallery/heatmap_weekly",
        &mut heatmap_weekly::chart(),
        800,
        500,
    );
}

#[test]
fn boxplot_latency() {
    check(
        "gallery/boxplot_latency",
        &mut boxplot_latency::chart(),
        800,
        500,
    );
}

#[test]
fn annotated_prices() {
    check(
        "gallery/annotated_prices",
        &mut annotated_prices::chart(),
        800,
        500,
    );
}

#[test]
fn dark_multiline() {
    check_on(
        "gallery/dark_multiline",
        &mut dark_multiline::chart(),
        800,
        500,
        dark_background(),
    );
}

/// The README hero: the bubble chart at the common social-preview size.
#[test]
fn hero() {
    check("gallery/hero", &mut scatter_bubbles::chart(), 1200, 630);
}

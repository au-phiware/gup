// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Chapter 7 of the tour: labels that stay legible on a crowded scatter.

use gup::prelude::*;

/// Sixty points, each with an id: a line plus noise from a small linear
/// congruential generator, so that every run draws the same chart.
fn stations() -> Vec<(u32, f64, f64)> {
    let mut state = 12_345_u32;
    let mut next = move || {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        f64::from(state >> 8) / f64::from(1_u32 << 24)
    };
    (1..=60)
        .map(|id| {
            let x = next() * 100.0;
            (id, x, 20.0 + 0.6 * x + 25.0 * next())
        })
        .collect()
}

pub fn labels() -> impl Chart {
    // tour:begin labels
    let rows = stations();
    gup::scatter(rows.clone())
        .x(|s| s.1)
        .y(|s| s.2)
        .radius(3.0)
        .title("Sixty stations")
        .x_title("Distance (km)")
        .y_title("Reading")
        .text(rows, |t| {
            t.x(|s| s.1)
                .y(|s| s.2)
                .text(|s| s.0.to_string())
                .anchor(Anchor::BOTTOM)
                .dy(-5.0)
                .halo(Px(2.0))
                .hide_overlaps(true)
                .fallbacks([
                    (Anchor::LEFT, 5.0, 0.0),
                    (Anchor::RIGHT, -5.0, 0.0),
                    (Anchor::TOP, 0.0, 6.0),
                ])
                .avoid_points(Px(3.0));
        })
    // tour:end labels
}

fn main() -> gup::Result<()> {
    gup::show(labels())
}

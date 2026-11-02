// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! A histogram stacked by category: commute times by mode (illustrative data).

use gup::builder::HistogramChart;
use gup::prelude::*;

pub struct Trip {
    mode: &'static str,
    minutes: f64,
}

/// About 600 trips: bikes are quick, buses slow and spread out.
pub fn chart() -> HistogramChart<Trip> {
    let modes = [
        ("Walk", 25.0_f64, 7.0, 150),
        ("Bike", 16.0, 5.0, 220),
        ("Bus", 34.0, 11.0, 230),
    ];
    let mut state = 12_345_u64;
    let mut uniform = move || {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (state >> 33) as f64 / f64::from(1_u32 << 31)
    };
    let mut trips = Vec::new();
    for (mode, mean, spread, count) in modes {
        for _ in 0..count {
            // The sum of four uniforms is close enough to a bell.
            let bell = (0..4).map(|_| uniform()).sum::<f64>() - 2.0;
            trips.push(Trip {
                mode,
                minutes: (mean + spread * bell * 1.7).max(1.0),
            });
        }
    }

    gup::histogram(trips)
        .x(|t| t.minutes)
        .color(|t| t.mode)
        .bins(Bins::Width(5.0))
        .title("Commute times")
        .subtitle("Minutes door to door, by mode (illustrative data)")
        .x_title("Minutes")
}

fn main() -> gup::Result<()> {
    gup::show(chart())
}

// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Three smooth lines that never overshoot their data (illustrative data).

use gup::prelude::*;

pub struct Reading {
    month: f64,
    city: &'static str,
    celsius: f64,
}

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// Mean temperatures: a yearly cosine around each city's average.
pub fn chart() -> LineChart<Reading> {
    let cities = [
        ("Perth", 18.5, 5.5, 0.0),
        ("Oslo", 6.0, 10.5, 6.0),
        ("Singapore", 27.5, 0.9, 4.0),
    ];
    let rows = cities.into_iter().flat_map(|(city, mean, swing, shift)| {
        (1..=12).map(move |month| {
            let m = f64::from(month);
            // Southern cities peak in January, northern ones in July.
            let year = (std::f64::consts::TAU * (m - 1.0 - shift) / 12.0).cos();
            Reading {
                month: m,
                city,
                celsius: mean + swing * year,
            }
        })
    });

    gup::line(rows)
        .x(|r| r.month)
        .y(|r| r.celsius)
        .color(|r| r.city)
        .curve(Curve::MonotoneX)
        .x_tick_format(|v| MONTHS[(v.round() as usize).clamp(1, 12) - 1].to_string())
        .title("Monthly mean temperature")
        .subtitle("Three cities, smoothed with a monotone curve")
        .x_title("Month")
        .y_title("°C")
        .grid(Axes::Y)
}

fn main() -> gup::Result<()> {
    gup::show(chart())
}

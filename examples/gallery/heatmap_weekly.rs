// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! A heatmap of weekday by hour, through the viridis scale (illustrative data).

use gup::prelude::*;

pub struct Visits {
    hour: String,
    day: &'static str,
    visits: f64,
}

/// Seven days by 24 hours: a lunchtime and an evening peak, quieter weekends.
pub fn chart() -> HeatmapChart<Visits> {
    let days = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    let cells = days.into_iter().enumerate().flat_map(|(d, day)| {
        (0..24_u32).map(move |h| {
            let hour = f64::from(h);
            let peak = |centre: f64, width: f64| (-((hour - centre) / width).powi(2)).exp();
            let weekend = if d >= 5 { 0.6 } else { 1.0 };
            let wobble = 1.0 + 0.08 * f64::from(h * 7 + d as u32 * 13).sin();
            Visits {
                hour: format!("{h:02}"),
                day,
                visits: 4.0 + weekend * wobble * (70.0 * peak(12.5, 2.0) + 95.0 * peak(20.5, 2.5)),
            }
        })
    });

    gup::heatmap(cells)
        .x(|v| v.hour.clone())
        .y(|v| v.day)
        .color_value(|v| v.visits)
        .color_value_scale(Sequential::viridis())
        .legend_title("Visits")
        .title("Website visits by hour")
        .subtitle("Mean visits per hour, weekday by hour of day (illustrative data)")
        .x_title("Hour")
}

fn main() -> gup::Result<()> {
    gup::show(chart())
}

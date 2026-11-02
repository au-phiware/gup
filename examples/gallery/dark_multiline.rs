// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Five lines in the dark theme: server load over a day (illustrative data).

use gup::builder::LineChart;
use gup::prelude::*;

pub struct Sample {
    hour: f64,
    host: &'static str,
    cpu: f64,
}

/// 96 fifteen-minute samples for each of five hosts.
pub fn chart() -> LineChart<Sample> {
    let hosts = [
        ("web-1", 0.0_f64, 55.0),
        ("web-2", 0.7, 50.0),
        ("db", 1.6, 40.0),
        ("cache", 2.4, 25.0),
        ("batch", 4.0, 30.0),
    ];
    let samples = hosts.into_iter().flat_map(|(host, phase, base)| {
        (0..96_u32).map(move |i| {
            let hour = f64::from(i) / 4.0;
            let wave = (std::f64::consts::TAU * hour / 24.0 - phase).sin();
            let ripple = 4.0 * f64::from(i * 11 % 17) / 17.0;
            Sample {
                hour,
                host,
                cpu: (base + 28.0 * wave + ripple).clamp(2.0, 98.0),
            }
        })
    });

    gup::line(samples)
        .x(|s| s.hour)
        .y(|s| s.cpu)
        .color(|s| s.host)
        .x_scale(Linear::new().domain(0.0, 24.0))
        .theme(Theme::dark())
        .grid(Axes::Y)
        .y_tick_format(|v| format!("{v:.0}%"))
        .title("Server load")
        .subtitle("CPU use of five hosts over a day, in Theme::dark()")
        .x_title("Hour")
}

fn main() -> gup::Result<()> {
    gup::show(chart())
}

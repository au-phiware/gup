// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Chapter 4 of the tour: time axes, a second y axis and tick labels you
//! write yourself.

use gup::prelude::*;

/// Share of a survey who agree, daily from 1 January 2020 (Unix seconds).
fn survey() -> Vec<(f64, f64)> {
    let t0 = 1_577_836_800.0;
    (0..900)
        .map(|d| {
            let d = f64::from(d);
            (
                t0 + d * 86_400.0,
                40.0 + 15.0 * (d / 70.0).sin() + d / 40.0 + 5.0 * (d / 9.0).sin(),
            )
        })
        .collect()
}

pub fn time_axis() -> impl Chart {
    // tour:begin time_axis
    gup::line(survey())
        .x(|r| r.0)
        .y(|r| r.1)
        .x_scale(Time::new())
        .y_tick_format(|v| format!("{v:.0}%"))
        .title("Support for the proposal")
        .subtitle("Daily survey, 2020 to mid 2022")
    // tour:end time_axis
}

pub fn dual_axis() -> impl Chart {
    // tour:begin dual_axis
    // Warm in January, wet in July.
    let months: Vec<(f64, f64, f64)> = (1..=12)
        .map(|n| {
            let season = (f64::from(n - 1) * std::f64::consts::TAU / 12.0).cos();
            (f64::from(n), 17.0 + 8.0 * season, 90.0 - 80.0 * season)
        })
        .collect();
    let mut plot = Plot::new();
    plot.legends(false)
        .title("Temperature and rainfall")
        .y_title("Temperature (°C)")
        .y_right_title("Rainfall (mm)");
    let x = plot.x(Linear::new());
    let left = plot.y(Linear::new());
    let right = plot.y_right(Linear::new());
    plot.line(months.clone())
        .x_with(x.encode(|m: &(f64, f64, f64)| m.0))
        .y_with(left.encode(|m: &(f64, f64, f64)| m.1))
        .color(Color::hex(0xd55e00));
    plot.line(months)
        .x_with(x.encode(|m: &(f64, f64, f64)| m.0))
        .y_with(right.encode(|m: &(f64, f64, f64)| m.2))
        .color(Color::hex(0x0072b2));
    plot
    // tour:end dual_axis
}

// tour:begin dollars
/// A dollar label: thousands take a `k`, smaller values are written out.
fn dollars(v: f64) -> String {
    if v.abs() < 1000.0 {
        format!("${v}")
    } else {
        format!("${}k", v / 1000.0)
    }
}
// tour:end dollars

pub fn bar_labels() -> impl Chart {
    // tour:begin bar_labels
    let revenue = [
        ("North", 480.0),
        ("South", 1250.0),
        ("East", 920.0),
        ("West", 1730.0),
    ];
    gup::bar(revenue)
        .x(|r| r.0)
        .y(|r| r.1)
        .y_tick_format(dollars)
        .value_labels_with(dollars)
        .title("Revenue by region")
        .y_title("Revenue")
    // tour:end bar_labels
}

fn main() -> gup::Result<()> {
    gup::show(time_axis())
}

// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Chapter 4 of the tour: time axes, a second y axis and tick labels you
//! write yourself.

use gup::prelude::*;

struct Survey {
    /// Unix seconds.
    day: f64,
    /// Percent who agree.
    support: f64,
}

/// Daily from 1 January 2020.
fn survey() -> Vec<Survey> {
    let t0 = 1_577_836_800.0;
    (0..900)
        .map(|d| {
            let d = f64::from(d);
            Survey {
                day: t0 + d * 86_400.0,
                support: 40.0 + 15.0 * (d / 70.0).sin() + d / 40.0 + 5.0 * (d / 9.0).sin(),
            }
        })
        .collect()
}

#[derive(Clone)]
struct Climate {
    month: f64,
    celsius: f64,
    rain_mm: f64,
}

/// Warm in January, wet in July.
fn climate() -> Vec<Climate> {
    (1..=12)
        .map(|n| {
            let season = (f64::from(n - 1) * std::f64::consts::TAU / 12.0).cos();
            Climate {
                month: f64::from(n),
                celsius: 17.0 + 8.0 * season,
                rain_mm: 90.0 - 80.0 * season,
            }
        })
        .collect()
}

struct Revenue {
    region: &'static str,
    dollars: f64,
}

fn revenue() -> [Revenue; 4] {
    [
        Revenue {
            region: "North",
            dollars: 480.0,
        },
        Revenue {
            region: "South",
            dollars: 1250.0,
        },
        Revenue {
            region: "East",
            dollars: 920.0,
        },
        Revenue {
            region: "West",
            dollars: 1730.0,
        },
    ]
}

pub fn time_axis() -> impl Chart {
    // tour:begin time_axis
    gup::line(survey())
        .x(|s| s.day)
        .y(|s| s.support)
        .x_scale(Time::new())
        .y_tick_format(|v| format!("{v:.0}%"))
        .title("Support for the proposal")
        .subtitle("Daily survey, 2020 to mid 2022")
    // tour:end time_axis
}

pub fn dual_axis() -> impl Chart {
    // tour:begin dual_axis
    let mut plot = Plot::new();
    plot.legends(false)
        .title("Temperature and rainfall")
        .y_title("Temperature (°C)")
        .y_right_title("Rainfall (mm)");
    let x = plot.x(Linear::new());
    let left = plot.y(Linear::new());
    let right = plot.y_right(Linear::new());
    plot.line(climate())
        .x_with(x.encode(|c: &Climate| c.month))
        .y_with(left.encode(|c: &Climate| c.celsius))
        .color(OKABE_ITO_DISTINCT[1]);
    plot.line(climate())
        .x_with(x.encode(|c: &Climate| c.month))
        .y_with(right.encode(|c: &Climate| c.rain_mm))
        .color(OKABE_ITO_DISTINCT[0]);
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
    gup::bar(revenue())
        .x(|r| r.region)
        .y(|r| r.dollars)
        .y_tick_format(dollars)
        .value_labels_with(dollars)
        .title("Revenue by region")
        .y_title("Revenue")
    // tour:end bar_labels
}

fn main() -> gup::Result<()> {
    gup::show(time_axis())
}

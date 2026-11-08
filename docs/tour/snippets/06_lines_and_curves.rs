// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Chapter 6 of the tour: one set of rows drawn as a step, a smooth curve
//! and a wide translucent line.

use gup::prelude::*;

struct Reading {
    hour: f64,
    value: f64,
}

/// Eight readings of a sensor, one per hour.
fn readings() -> Vec<Reading> {
    [2.0, 6.0, 3.0, 8.0, 5.0, 9.0, 4.0, 7.0]
        .iter()
        .enumerate()
        .map(|(hour, &value)| Reading {
            hour: hour as f64,
            value,
        })
        .collect()
}

pub fn curves() -> impl Chart {
    // tour:begin curves
    let mut plot = Plot::new();
    plot.legends(false)
        .title("The same eight readings, three ways")
        .x_title("Hour")
        .y_title("Reading");
    plot.line(readings())
        .x(|r| r.hour)
        .y(|r| r.value)
        .curve(Curve::StepAfter)
        .color(OKABE_ITO_DISTINCT[0]);
    plot.line(readings())
        .x(|r| r.hour)
        .y(|r| r.value)
        .curve(Curve::MonotoneX)
        .color(OKABE_ITO_DISTINCT[1]);
    let translucent = Color {
        a: 0.4,
        ..OKABE_ITO_DISTINCT[3]
    };
    plot.line(readings())
        .x(|r| r.hour)
        .y(|r| r.value)
        .width(Px(12.0))
        .color(translucent);
    plot
    // tour:end curves
}

fn main() -> gup::Result<()> {
    gup::show(curves())
}

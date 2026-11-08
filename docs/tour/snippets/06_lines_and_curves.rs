// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Chapter 6 of the tour: one set of rows drawn as a step, a smooth curve
//! and a wide translucent line.

use gup::prelude::*;

/// Eight readings of a sensor, one per hour.
fn readings() -> Vec<(f64, f64)> {
    [2.0, 6.0, 3.0, 8.0, 5.0, 9.0, 4.0, 7.0]
        .iter()
        .enumerate()
        .map(|(hour, &value)| (hour as f64, value))
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
        .x(|r| r.0)
        .y(|r| r.1)
        .curve(Curve::StepAfter)
        .color(Color::hex(0x0072b2));
    plot.line(readings())
        .x(|r| r.0)
        .y(|r| r.1)
        .curve(Curve::MonotoneX)
        .color(Color::hex(0xd55e00));
    let translucent = Color {
        a: 0.4,
        ..Color::hex(0x009e73)
    };
    plot.line(readings())
        .x(|r| r.0)
        .y(|r| r.1)
        .width(Px(12.0))
        .color(translucent);
    plot
    // tour:end curves
}

fn main() -> gup::Result<()> {
    gup::show(curves())
}

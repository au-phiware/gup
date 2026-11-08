// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Chapter 8 of the tour: a tooltip and a brush, driven by `Input` values
//! so that the chart can be drawn without a window.

use gup::prelude::*;
use gup::{Button, Input, Layout};

/// Nine birds: name, mass (kg), wingspan (m).
fn birds() -> [(&'static str, f64, f64); 9] {
    [
        ("Bellbird", 0.03, 0.20),
        ("Tui", 0.11, 0.35),
        ("Morepork", 0.17, 0.55),
        ("Kaka", 0.50, 0.75),
        ("Kea", 0.90, 0.95),
        ("Harrier", 1.00, 1.30),
        ("Pukeko", 1.50, 0.55),
        ("Weka", 2.10, 0.30),
        ("Takahe", 2.70, 0.40),
    ]
}

// tour:begin pixel
/// Where the data point `(x, y)` is drawn, in the chart's pixels.
fn pixel(layout: &Layout, x: f64, y: f64) -> (f32, f32) {
    let along = |ticks: &[(f64, f32)], v: f64| {
        let (first, last) = (ticks[0], ticks[ticks.len() - 1]);
        first.1 + ((v - first.0) * f64::from(last.1 - first.1) / (last.0 - first.0)) as f32
    };
    (along(&layout.x_ticks, x), along(&layout.y_ticks, y))
}
// tour:end pixel

pub fn tooltip() -> gup::Result<impl Chart> {
    // tour:begin tooltip
    let mut chart = gup::scatter(birds())
        .x(|b| b.1)
        .y(|b| b.2)
        .radius(5.0)
        .title("Birds of Aotearoa")
        .x_title("Mass (kg)")
        .y_title("Wingspan (m)")
        .tooltip(|b| format!("{}\n{} kg, {} m", b.0, b.1, b.2));
    let cx = Context::shared()?;
    chart.resolve(&cx, 640.0, 400.0)?;

    // Zoom about the Kea, then drag the view a little.
    let kea = pixel(chart.layout().unwrap(), 0.90, 0.95);
    let _ = chart.handle(Input::wheel(kea, 1.0));
    let _ = chart.handle(Input::pointer_down(kea, Button::Primary));
    let _ = chart.handle(Input::pointer_moved((kea.0 - 30.0, kea.1 + 20.0)));
    let _ = chart.handle(Input::pointer_up(
        (kea.0 - 30.0, kea.1 + 20.0),
        Button::Primary,
    ));
    chart.resolve(&cx, 640.0, 400.0)?;

    // Point at the Kea where it is now: the next resolve draws the tooltip.
    let kea = pixel(chart.layout().unwrap(), 0.90, 0.95);
    let _ = chart.handle(Input::pointer_moved(kea));
    chart.resolve(&cx, 640.0, 400.0)?;
    Ok(chart)
    // tour:end tooltip
}

pub fn brush() -> gup::Result<impl Chart> {
    // tour:begin brush
    let mut chart = gup::scatter(birds())
        .x(|b| b.1)
        .y(|b| b.2)
        .radius(6.0)
        .title("Birds of Aotearoa")
        .x_title("Mass (kg)")
        .y_title("Wingspan (m)");
    chart.plot_mut().drag(Drag::Brush);
    let cx = Context::shared()?;
    let plot = chart.resolve(&cx, 640.0, 400.0)?.layout.plot;

    // Drag from the bottom-left corner to just right of the Kea.
    let right = pixel(chart.layout().unwrap(), 0.95, 0.0).0;
    let from = (plot.left() + 1.0, plot.bottom() - 1.0);
    let to = (right, plot.top() + 1.0);
    let _ = chart.handle(Input::pointer_down(from, Button::Primary));
    let _ = chart.handle(Input::pointer_moved(to));
    let _ = chart.handle(Input::pointer_up(to, Button::Primary));
    Ok(chart)
    // tour:end brush
}

fn main() -> gup::Result<()> {
    gup::show(tooltip()?)
}

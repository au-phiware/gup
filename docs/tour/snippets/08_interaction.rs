// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Chapter 8 of the tour: a tooltip and a brush, driven by `Input` values
//! so that the chart can be drawn without a window.

use gup::geom::Point;
use gup::prelude::*;
use gup::{Button, Input};

struct Bird {
    name: &'static str,
    /// Kilograms.
    mass: f64,
    /// Metres.
    wingspan: f64,
}

/// Twelve birds of Aotearoa (rounded figures).
fn birds() -> Vec<Bird> {
    [
        ("Bellbird", 0.03, 0.20),
        ("Tui", 0.11, 0.35),
        ("Morepork", 0.17, 0.55),
        ("Kokako", 0.25, 0.60),
        ("Kaka", 0.50, 0.75),
        ("Kereru", 0.65, 0.80),
        ("Kea", 0.90, 0.95),
        ("Weka", 1.00, 0.55),
        ("Pukeko", 1.10, 0.80),
        ("Bittern", 1.00, 1.05),
        ("Harrier", 1.10, 1.30),
        ("Takahe", 2.70, 0.40),
    ]
    .map(|(name, mass, wingspan)| Bird {
        name,
        mass,
        wingspan,
    })
    .into()
}

pub fn tooltip() -> gup::Result<impl Chart> {
    // tour:begin tooltip
    let mut chart = gup::scatter(birds())
        .x(|b| b.mass)
        .y(|b| b.wingspan)
        .radius(5.0)
        .title("Birds of Aotearoa")
        .x_title("Mass (kg)")
        .y_title("Wingspan (m)")
        .tooltip(|b| format!("{}\n{} kg, {} m", b.name, b.mass, b.wingspan));
    let cx = Context::shared()?;
    chart.resolve(&cx, 640.0, 400.0)?;

    // Zoom with the wheel over the flock, then drag the view to centre it.
    let flock = chart.pixel(0.7, 0.8).unwrap();
    let _ = chart.handle(Input::wheel(flock, 5.0));
    let to = Point::new(flock.x + 110.0, flock.y);
    let _ = chart.handle(Input::pointer_down(flock, Button::Primary));
    let _ = chart.handle(Input::pointer_moved(to));
    let _ = chart.handle(Input::pointer_up(to, Button::Primary));
    chart.resolve(&cx, 640.0, 400.0)?;

    // Point at the Kereru where it is now: the next resolve draws the tooltip.
    let kereru = chart.pixel(0.65, 0.80).unwrap();
    let _ = chart.handle(Input::pointer_moved(kereru));
    chart.resolve(&cx, 640.0, 400.0)?;
    Ok(chart)
    // tour:end tooltip
}

pub fn brush() -> gup::Result<impl Chart> {
    // tour:begin brush
    let mut chart = gup::scatter(birds())
        .x(|b| b.mass)
        .y(|b| b.wingspan)
        .radius(6.0)
        .title("Birds of Aotearoa")
        .x_title("Mass (kg)")
        .y_title("Wingspan (m)");
    chart.plot_mut().drag(Drag::Brush);
    let cx = Context::shared()?;
    let plot = chart.resolve(&cx, 640.0, 400.0)?.layout.plot;

    // Drag from the bottom-left corner to just right of the Kea.
    let right = chart.pixel(0.95, 0.0).unwrap().x;
    let from = Point::new(plot.left() + 1.0, plot.bottom() - 1.0);
    let to = Point::new(right, plot.top() + 1.0);
    let _ = chart.handle(Input::pointer_down(from, Button::Primary));
    let _ = chart.handle(Input::pointer_moved(to));
    let _ = chart.handle(Input::pointer_up(to, Button::Primary));
    Ok(chart)
    // tour:end brush
}

fn main() -> gup::Result<()> {
    gup::show(tooltip()?)
}

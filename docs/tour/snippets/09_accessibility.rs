// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Chapter 9 of the tour: keyboard focus, and the text a screen reader needs.

use gup::prelude::*;
use gup::{Input, Key, Modifiers, SvgTarget, VectorTarget};
use std::fmt::Write as _;

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

pub fn keys() -> gup::Result<impl Chart> {
    // tour:begin keys
    let mut chart = gup::scatter(birds())
        .x(|b| b.mass)
        .y(|b| b.wingspan)
        .radius(6.0)
        .title("Birds of Aotearoa")
        .x_title("Mass (kg)")
        .y_title("Wingspan (m)")
        .tooltip(|b| format!("{}\n{} kg, {} m", b.name, b.mass, b.wingspan));
    let cx = Context::shared()?;
    chart.resolve(&cx, 640.0, 400.0)?;

    // Zoom in about the centre of the plot, then pan left with Shift.
    let _ = chart.handle(Input::key(Key::ZoomIn));
    let _ = chart.handle(Input::key_with(Key::Left, Modifiers::SHIFT));
    let _ = chart.handle(Input::key_with(Key::Left, Modifiers::SHIFT));
    // Right focuses the first row in view, in x order, then the next.
    let _ = chart.handle(Input::key(Key::Right));
    let _ = chart.handle(Input::key(Key::Right));
    chart.resolve(&cx, 640.0, 400.0)?;
    Ok(chart)
    // tour:end keys
}

pub fn read_aloud_output() -> gup::Result<String> {
    // tour:begin read_aloud
    let mut chart = keys()?;
    let cx = Context::shared()?;
    let scene = chart.resolve(&cx, 640.0, 400.0)?.scene.clone();
    let mut svg = SvgTarget::new();
    svg.render(&scene)?;

    let mut out = String::new();
    writeln!(out, "focus: {}", chart.focus_text().unwrap_or("none")).unwrap();
    let d = chart.describe().expect("the chart is resolved");
    writeln!(out, "title: {}\ntext: {}", d.title, d.text).unwrap();
    // The SVG's first children are the same words.
    let svg = svg.svg();
    let between = |open: &str, close: &str| {
        let from = svg.find(open).unwrap() + open.len();
        &svg[from..from + svg[from..].find(close).unwrap()]
    };
    writeln!(out, "svg title: {}", between("<title>", "</title>")).unwrap();
    writeln!(out, "svg desc: {}", between("<desc>", "</desc>")).unwrap();
    // The hidden table of the browser, as data.
    let table = chart.data_table(1_000).expect("the chart is resolved");
    writeln!(out, "table caption: {}", table.caption).unwrap();
    writeln!(out, "table headers: {}", table.headers.join(", ")).unwrap();
    write!(out, "table row 1: {}", table.rows[0].join(", ")).unwrap();
    Ok(out)
    // tour:end read_aloud
}

fn main() -> gup::Result<()> {
    println!("{}", read_aloud_output()?);
    gup::show(keys()?)
}

// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Chapter 1 of the tour: from rows to a chart, and out to a file.

use gup::prelude::*;
use std::path::Path;

/// Twelve countries: region, GDP per person (US$), life expectancy (years),
/// population (illustrative figures).
fn countries() -> [(&'static str, f64, f64, f64); 12] {
    [
        ("Africa", 2.1e3, 53.0, 200e6),
        ("Africa", 1.7e3, 67.0, 54e6),
        ("Africa", 0.9e3, 66.0, 120e6),
        ("Europe", 5.1e4, 81.0, 83e6),
        ("Europe", 4.4e4, 82.0, 67e6),
        ("Europe", 1.8e4, 77.0, 38e6),
        ("Asia", 1.2e4, 77.0, 1400e6),
        ("Asia", 2.3e3, 70.0, 1380e6),
        ("Asia", 4.0e4, 85.0, 126e6),
        ("Americas", 6.3e4, 79.0, 330e6),
        ("Americas", 7.5e3, 75.0, 212e6),
        ("Americas", 9.9e3, 75.0, 128e6),
    ]
}

pub fn five_lines() -> impl Chart {
    // tour:begin five_lines
    gup::scatter(countries())
        .x(|c| c.1)
        .y(|c| c.2)
        .color(|c| c.0)
        .x_title("GDP per person (US$)")
        .y_title("Life expectancy (years)")
    // tour:end five_lines
}

// tour:begin save
pub fn save(dir: &Path) -> gup::Result<()> {
    let cx = Context::shared()?;
    let mut chart = five_lines();
    chart.save_png(&cx, dir.join("chart.png"), 640, 400)?;
    chart.save_svg(&cx, dir.join("chart.svg"), 640, 400)
}
// tour:end save

fn main() -> gup::Result<()> {
    gup::show(five_lines())
}

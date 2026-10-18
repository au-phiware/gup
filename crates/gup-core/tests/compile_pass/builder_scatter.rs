// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// GUP-462: the builder's north star, with owned rows and no closure
// annotations, from a crate outside gup-core.
use gup_core::prelude::*;

struct Row {
    gdp: f64,
    life_exp: f64,
    continent: String,
    population: f64,
}

fn main() {
    let data: Vec<Row> = Vec::new();
    let chart = gup_core::scatter(data)
        .x(|d| d.gdp)
        .y(|d| d.life_exp)
        .color(|d| d.continent.as_str())
        .size(|d| d.population)
        .grid(Axes::Both);
    let _: &dyn Chart = &chart;
}

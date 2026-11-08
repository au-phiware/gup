// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Chapter 3 of the tour: position, size and colour from the data, each
//! through a scale.

use gup::prelude::*;

struct Country {
    gdp: f64,
    life: f64,
    people: f64,
    schooling: f64,
}

/// Twelve countries: GDP per person (US$), life expectancy (years),
/// population, years of schooling (illustrative figures).
fn countries() -> Vec<Country> {
    [
        (2.1e3, 53.0, 200e6, 6.0),
        (1.7e3, 67.0, 54e6, 7.5),
        (3.0e2, 62.0, 120e6, 4.5),
        (5.1e4, 81.0, 83e6, 14.0),
        (4.4e4, 82.0, 67e6, 13.5),
        (1.8e4, 77.0, 38e6, 12.0),
        (1.2e4, 77.0, 1400e6, 8.0),
        (2.3e3, 70.0, 1380e6, 6.5),
        (4.0e4, 85.0, 126e6, 12.5),
        (1.2e5, 84.0, 5e6, 13.5),
        (7.5e3, 75.0, 212e6, 8.5),
        (9.9e3, 75.0, 128e6, 9.0),
    ]
    .map(|(gdp, life, people, schooling)| Country {
        gdp,
        life,
        people,
        schooling,
    })
    .into()
}

pub fn encodings() -> impl Chart {
    // tour:begin encodings
    gup::scatter(countries())
        .x(|c| c.gdp)
        .y(|c| c.life)
        .size(|c| c.people)
        .color_value(|c| c.schooling)
        .color_value_scale(Sequential::viridis())
        .legend_title("Years of schooling")
        .x_scale(Log::new())
        .x_title("GDP per person (US$)")
        .y_title("Life expectancy (years)")
    // tour:end encodings
}

fn main() -> gup::Result<()> {
    gup::show(encodings())
}

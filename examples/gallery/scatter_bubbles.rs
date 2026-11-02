// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! A bubble chart: size, colour and a log axis (illustrative data).

use gup::builder::ScatterChart;
use gup::prelude::*;

pub struct Country {
    region: &'static str,
    gdp: f64,
    life: f64,
    people: f64,
}

/// Forty invented countries over five regions: richer ones live longer.
pub fn chart() -> ScatterChart<Country> {
    let regions = [
        ("Africa", 900.0_f64, 9_000.0, 190.0),
        ("Asia", 1_500.0, 45_000.0, 380.0),
        ("Europe", 9_000.0, 70_000.0, 60.0),
        ("Americas", 3_000.0, 60_000.0, 110.0),
        ("Oceania", 2_500.0, 55_000.0, 25.0),
    ];
    let countries = regions
        .into_iter()
        .enumerate()
        .flat_map(|(r, (region, lo, hi, pop))| {
            (0..8_u32).map(move |i| {
                let spread = f64::from((i * 5 + r as u32 * 3) % 8) / 7.0;
                let gdp = lo * (hi / lo).powf(spread);
                let jitter = 3.5 * f64::from(i * 7 + r as u32 * 3).sin();
                Country {
                    region,
                    gdp,
                    life: (58.0 + 5.2 * (gdp / 1_000.0).ln() + jitter).min(84.0),
                    people: pop * 1e6 * (0.15 + f64::from((i * 3 + r as u32) % 7) / 4.0),
                }
            })
        });

    gup::scatter(countries)
        .x(|c| c.gdp)
        .y(|c| c.life)
        .color(|c| c.region)
        .size(|c| c.people)
        .x_scale(Log::new())
        .x_tick_format(|v| format!("${}k", v / 1000.0))
        .title("Wealth and health of nations")
        .subtitle("GDP per person against life expectancy, 2020 (illustrative data)")
        .x_title("GDP per person")
        .y_title("Life expectancy (years)")
        .grid(Axes::Both)
}

fn main() -> gup::Result<()> {
    gup::show(chart())
}

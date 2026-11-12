// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! A bubble chart: size, colour and a log axis (illustrative data).

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
        .size_scale(Pow::sqrt().range(Px(4.0), Px(16.0)))
        .x_scale(Log::new())
        .x_tick_format(dollars)
        .title("Wealth and health of nations")
        .subtitle("GDP per person against life expectancy, 2020 (illustrative data)")
        .x_title("GDP per person")
        .y_title("Life expectancy (years)")
        .grid(Axes::Both)
}

/// A dollar tick label: thousands take a `k`, smaller values are written out.
fn dollars(v: f64) -> String {
    if v.abs() < 1000.0 {
        format!("${v}")
    } else {
        format!("${}k", v / 1000.0)
    }
}

fn main() -> gup::Result<()> {
    gup::show(chart())
}

#[cfg(test)]
mod tests {
    use super::dollars;

    #[test]
    fn dollars_below_a_thousand_have_no_k() {
        assert_eq!(dollars(0.5), "$0.5");
        assert_eq!(dollars(500.0), "$500");
        assert_eq!(dollars(999.0), "$999");
        assert_eq!(dollars(1000.0), "$1k");
        assert_eq!(dollars(2500.0), "$2.5k");
        assert_eq!(dollars(1e6), "$1000k");
    }
}

// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Stacked areas with an outline: a shifting energy mix (illustrative data).

use gup::prelude::*;

pub struct Output {
    year: f64,
    source: &'static str,
    twh: f64,
}

type Source = fn(f64) -> f64;

/// Twenty years of generation: coal fades, wind and solar grow.
pub fn chart() -> AreaChart<Output> {
    let sources: [(&str, Source); 4] = [
        ("Coal", |t| 160.0 - 5.5 * t),
        ("Gas", |t| 60.0 + 1.5 * t + 6.0 * (t / 3.0).sin()),
        ("Wind", |t| 8.0 + t * t / 10.0 + t),
        ("Solar", |t| 1.0 + 0.15 * t * t),
    ];
    let rows = (0..20).flat_map(move |i| {
        let t = f64::from(i);
        sources.map(|(source, twh)| Output {
            year: 2005.0 + t,
            source,
            twh: twh(t),
        })
    });

    gup::area(rows)
        .x(|o| o.year)
        .y(|o| o.twh)
        .stack(|o| o.source)
        .outline(1.0)
        .x_tick_format(|v| format!("{v:.0}"))
        .title("Electricity generation by source")
        .subtitle("Stacked areas, 2005–2024 (illustrative data)")
        .x_title("Year")
        .y_title("TWh")
}

fn main() -> gup::Result<()> {
    gup::show(chart())
}

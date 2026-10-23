// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! A grouped bar chart; pass `--stacked` to stack the groups instead.
//!
//! Set `GUP_SCREENSHOT_PATH=out.png` to render one frame headless.

struct Sale {
    quarter: &'static str,
    region: &'static str,
    sales: f64,
}

fn main() -> gup::Result<()> {
    let rows = ["Q1", "Q2", "Q3", "Q4"]
        .into_iter()
        .enumerate()
        .flat_map(|(i, quarter)| {
            [("North", 40.0), ("South", 25.0), ("East", 32.0)].map(|(region, base)| Sale {
                quarter,
                region,
                sales: base + 6.0 * i as f64,
            })
        });
    let chart = gup::bar(rows)
        .x(|s| s.quarter)
        .y(|s| s.sales)
        .color(|s| s.region)
        .padding(0.15)
        .title("Sales by quarter and region");
    if std::env::args().any(|a| a == "--stacked") {
        gup::show(chart.stack(|s| s.region))
    } else {
        gup::show(chart.group(|s| s.region))
    }
}

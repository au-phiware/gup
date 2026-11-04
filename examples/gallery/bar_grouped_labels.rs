// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Grouped bars with a value label on every bar (illustrative data).

use gup::builder::BarChart;
use gup::prelude::*;

pub struct Sale {
    quarter: &'static str,
    region: &'static str,
    revenue: f64,
}

/// Four quarters of revenue in three regions, in thousands of dollars.
pub fn chart() -> BarChart<Sale> {
    let regions = [("North", 420.0), ("South", 310.0), ("West", 365.0)];
    let sales = ["Q1", "Q2", "Q3", "Q4"]
        .into_iter()
        .enumerate()
        .flat_map(move |(q, quarter)| {
            regions.map(|(region, base)| Sale {
                quarter,
                region,
                revenue: base * (1.0 + 0.12 * q as f64) + 25.0 * (q as f64 * 5.0 + base).sin(),
            })
        });

    gup::bar(sales)
        .x(|s| s.quarter)
        .y(|s| s.revenue)
        .group(|s| s.region)
        .group_padding(0.15)
        .value_labels_with(|v| format!("{v:.0}"))
        .title("Quarterly revenue by region")
        .subtitle("Grouped bars with value labels")
        .y_title("Revenue ($k)")
        .grid(Axes::Y)
}

fn main() -> gup::Result<()> {
    gup::show(chart())
}

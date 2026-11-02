// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Box plots of latency per service, with outliers (illustrative data).

use gup::builder::BoxPlotChart;
use gup::prelude::*;

pub struct Request {
    service: &'static str,
    ms: f64,
}

/// Five services, 60 requests each; three of them have a few slow outliers.
pub fn chart() -> BoxPlotChart<Request> {
    let services = [
        ("auth", 45.0_f64, 12.0, &[120.0, 140.0][..]),
        ("search", 95.0, 35.0, &[260.0, 290.0, 310.0]),
        ("cart", 65.0, 18.0, &[]),
        ("checkout", 130.0, 32.0, &[320.0, 350.0]),
        ("images", 28.0, 9.0, &[]),
    ];
    let requests = services
        .into_iter()
        .flat_map(|(service, median, spread, slow)| {
            let typical = (0..60 - slow.len()).map(move |i| {
                let bell = (0..3)
                    .map(|k| ((i * 17 + k * 29 + 7) % 41) as f64 / 41.0)
                    .sum::<f64>()
                    - 1.5;
                Request {
                    service,
                    ms: (median + spread * bell * 2.4).max(5.0),
                }
            });
            typical.chain(slow.iter().map(move |&ms| Request { service, ms }))
        });

    gup::boxplot(requests)
        .x(|r| r.service)
        .y(|r| r.ms)
        .title("Response time by service")
        .subtitle("Box plots, whiskers at 1.5 IQR, outliers as points (illustrative data)")
        .y_title("Latency (ms)")
        .grid(Axes::Y)
}

fn main() -> gup::Result<()> {
    gup::show(chart())
}

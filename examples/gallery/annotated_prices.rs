// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! The low-level `Plot` API: a step line with event markers and text labels
//! (illustrative data).

use gup::prelude::*;

/// 120 days of a share price, with three events marked and named.
pub fn chart() -> Plot {
    let prices: Vec<(f64, f64)> = (0..120_u32)
        .map(|d| {
            let day = f64::from(d);
            let jump = if d >= 40 { 9.0 } else { 0.0 } - if d >= 85 { 14.0 } else { 0.0 };
            let bump = 5.0 * (-((day - 103.0) / 4.0).powi(2)).exp();
            let drift =
                100.0 + 0.08 * day + 3.0 * (day / 6.0).sin() + 1.5 * (day * 2.3).sin() + bump;
            (day, (drift + jump).round())
        })
        .collect();
    // Each event sits on the highest day of its window, so that its label
    // has clear space above it.
    let peak = |lo: usize, hi: usize| {
        (lo..hi)
            .max_by(|&a, &b| prices[a].1.total_cmp(&prices[b].1))
            .expect("a window has days")
    };
    let events = [
        (peak(30, 60), "Product launch"),
        (peak(70, 90), "Profit warning"),
        (peak(96, 108), "New CEO"),
    ]
    .map(|(day, label)| (prices[day].0, prices[day].1, label));

    let mut plot = Plot::new();
    plot.title("Share price and company events")
        .subtitle("A step line with event markers and text labels (illustrative data)")
        .x_title("Day")
        .y_title("Price ($)")
        .grid(Axes::Y);
    plot.line(prices)
        .x(|p| p.0)
        .y(|p| p.1)
        .curve(Curve::StepAfter)
        .width(Px(2.0));
    plot.circles(events).x(|e| e.0).y(|e| e.1).radius(Px(5.0));
    plot.text(events)
        .x(|e| e.0)
        .y(|e| e.1)
        .text(|e| e.2)
        .anchor(Anchor::BOTTOM)
        .dy(-8.0);
    plot
}

fn main() -> gup::Result<()> {
    gup::show(chart())
}

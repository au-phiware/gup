// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GUP-612: the x axis of the line and area builders ends at the data, not
//! at the next round number, while an explicit domain still wins and a
//! single distinct x still gets a domain.

use gup_core::geom::Point;
use gup_core::prelude::*;

const W: f32 = 640.0;
const H: f32 = 400.0;

fn years() -> Vec<(f64, f64)> {
    (2005..=2024)
        .map(|y| (f64::from(y), f64::from(y % 7)))
        .collect()
}

/// The x values at the left and right edges of the plot rect, and its width.
macro_rules! x_ends {
    ($chart:expr, $cx:expr) => {{
        let rect = $chart.resolve($cx, W, H).unwrap().layout.plot;
        let mid = rect.top() + rect.height / 2.0;
        let plot = $chart.plot_mut();
        let lo = plot.value(Point::new(rect.left(), mid)).unwrap().0;
        let hi = plot.value(Point::new(rect.right(), mid)).unwrap().0;
        (lo, hi, rect.width)
    }};
}

#[test]
fn an_area_x_axis_ends_exactly_at_the_data() {
    let cx = Context::new_blocking().unwrap();
    let mut chart = gup_core::area(years()).x(|r| r.0).y(|r| r.1);
    let (lo, hi, width) = x_ends!(chart, &cx);
    // Only the outline's overhang (a few px) widens the extent.
    let slack = f64::from(8.0 / width) * (2024.0 - 2005.0);
    assert!(lo <= 2005.0 && 2005.0 - lo <= slack, "left end {lo}");
    assert!(hi >= 2024.0 && hi - 2024.0 <= slack, "right end {hi}");
}

#[test]
fn a_line_x_axis_ends_within_a_line_width_of_the_data() {
    let cx = Context::new_blocking().unwrap();
    let mut chart = gup_core::line(years()).x(|r| r.0).y(|r| r.1).width(6.0);
    let (lo, hi, width) = x_ends!(chart, &cx);
    // Half a line width of cap on each side, in data units (with slack).
    let slack = f64::from(6.0 / width) * (2024.0 - 2005.0) * 1.5;
    assert!(lo <= 2005.0 && 2005.0 - lo <= slack, "left end {lo}");
    assert!(hi >= 2024.0 && hi - 2024.0 <= slack, "right end {hi}");
}

#[test]
fn the_policy_survives_x_scale_and_an_explicit_domain_wins() {
    let cx = Context::new_blocking().unwrap();
    let mut chart = gup_core::area(years())
        .x(|r| r.0)
        .y(|r| r.1)
        .x_scale(Linear::new());
    let (lo, hi, width) = x_ends!(chart, &cx);
    let slack = f64::from(8.0 / width) * (2024.0 - 2005.0);
    assert!(2005.0 - lo <= slack && hi - 2024.0 <= slack, "{lo}..{hi}");

    let mut chart = gup_core::area(years())
        .x(|r| r.0)
        .y(|r| r.1)
        .x_scale(Linear::new().domain(2000.0, 2030.0));
    let (lo, hi, width) = x_ends!(chart, &cx);
    // An explicit domain is inset by the overhang, never refitted to the data.
    let slack = f64::from(8.0 / width) * 30.0;
    assert!(
        (lo - 2000.0).abs() <= slack && (hi - 2030.0).abs() <= slack,
        "{lo}..{hi}"
    );
}

#[test]
fn a_single_x_still_gets_a_domain() {
    let cx = Context::new_blocking().unwrap();
    let mut chart = gup_core::line(vec![(5.0, 1.0), (5.0, 2.0)])
        .x(|r| r.0)
        .y(|r| r.1);
    let (lo, hi, _) = x_ends!(chart, &cx);
    assert!(lo < 5.0 && hi > 5.0, "{lo}..{hi}");
}

#[test]
fn y_axes_still_round_out() {
    let cx = Context::new_blocking().unwrap();
    let rows: Vec<(f64, f64)> = (0..10)
        .map(|i| (f64::from(i), 0.3 + f64::from(i) * 0.93))
        .collect();
    let mut chart = gup_core::area(rows).x(|r| r.0).y(|r| r.1);
    let layout = chart.resolve(&cx, W, H).unwrap().layout.clone();
    let first = layout.y_ticks.first().unwrap().0;
    let last = layout.y_ticks.last().unwrap().0;
    assert!(first <= 0.3 && last >= 0.3 + 9.0 * 0.93);
}

// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GUP-685: `Chart::pixel` and `Chart::value` map data to pixels and back
//! through the scales themselves.

use gup_core::geom::Point;
use gup_core::prelude::*;

const SIZE: (f32, f32) = (480.0, 320.0);

struct Row {
    x: f64,
    y: f64,
}

fn rows(xs: &[f64]) -> Vec<Row> {
    xs.iter()
        .enumerate()
        .map(|(i, &x)| Row {
            x,
            y: 1.0 + i as f64,
        })
        .collect()
}

/// A resolved scatter with the given x scale, over `xs`.
fn plot_with<S: PositionScale>(scale: S, xs: &[f64]) -> Plot {
    let cx = Context::new_blocking().unwrap();
    let mut plot = Plot::new();
    plot.x(scale);
    plot.circles(rows(xs)).x(|r| r.x).y(|r| r.y);
    plot.resolve(&cx, SIZE.0, SIZE.1).unwrap();
    plot
}

/// `value` then `pixel` returns the pixel to 0.01 px; `pixel` then `value`
/// returns the data to a thousandth of what one pixel spans (a `Point` is
/// `f32`, so a relative 1e-9 on the data is out of reach).
fn round_trip(plot: &Plot, points: &[(f64, f64)]) {
    for &(x, y) in points {
        let at = plot.pixel(x, y).unwrap();
        let (vx, vy) = plot.value(at).unwrap();
        let again = plot.pixel(vx, vy).unwrap();
        assert!((again.x - at.x).abs() < 0.01, "x {x}: {at:?} -> {again:?}");
        assert!((again.y - at.y).abs() < 0.01, "y {y}: {at:?} -> {again:?}");
        let one = plot.value(Point::new(at.x + 1.0, at.y - 1.0)).unwrap();
        let (tx, ty) = ((one.0 - vx).abs() * 1e-3, (one.1 - vy).abs() * 1e-3);
        assert!((vx - x).abs() <= tx.max(1e-9 * x.abs()), "{x} -> {vx}");
        assert!((vy - y).abs() <= ty.max(1e-9 * y.abs()), "{y} -> {vy}");
    }
}

#[test]
fn linear_round_trips() {
    let plot = plot_with(Linear::new(), &[0.0, 10.0, 20.0]);
    round_trip(&plot, &[(0.0, 1.0), (7.5, 2.0), (20.0, 3.0)]);
}

#[test]
fn log_round_trips() {
    let plot = plot_with(Log::new(), &[1.0, 10.0, 1000.0]);
    round_trip(&plot, &[(1.0, 1.0), (3.0, 2.0), (1000.0, 3.0)]);
    // Equal ratios are equal pixels apart: a tick interpolation is wrong here.
    let (a, b, c) = (
        plot.pixel(1.0, 1.0).unwrap().x,
        plot.pixel(10.0, 1.0).unwrap().x,
        plot.pixel(100.0, 1.0).unwrap().x,
    );
    assert!(((b - a) - (c - b)).abs() < 0.01, "{a} {b} {c}");
}

#[test]
fn symlog_round_trips() {
    let plot = plot_with(Symlog::new(), &[-100.0, 0.0, 1000.0]);
    round_trip(
        &plot,
        &[(-100.0, 1.0), (-0.5, 2.0), (0.0, 2.0), (1000.0, 3.0)],
    );
}

#[test]
fn pow_round_trips() {
    let plot = plot_with(Pow::new(0.5), &[0.0, 4.0, 100.0]);
    round_trip(&plot, &[(0.0, 1.0), (4.0, 2.0), (100.0, 3.0), (30.0, 2.5)]);
}

#[test]
fn time_round_trips() {
    let t0 = 1_700_000_000.0;
    let plot = plot_with(Time::new(), &[t0, t0 + 86_400.0, t0 + 5.0 * 86_400.0]);
    round_trip(
        &plot,
        &[(t0, 1.0), (t0 + 3_600.0, 2.0), (t0 + 5.0 * 86_400.0, 3.0)],
    );
    // Epoch seconds land on the time axis's own ticks.
    let layout = plot.layout().unwrap();
    for &(v, px) in &layout.x_ticks {
        let at = plot.pixel(v, 1.0).unwrap();
        assert!((at.x - px).abs() < 0.01, "{v}: {} vs {px}", at.x);
    }
}

#[test]
fn linear_agrees_with_the_tick_interpolation() {
    let plot = plot_with(Linear::new(), &[0.0, 10.0, 20.0]);
    let layout = plot.layout().unwrap();
    let along = |ticks: &[(f64, f32)], v: f64| {
        let (a, b) = (ticks[0], ticks[ticks.len() - 1]);
        f64::from(a.1) + (v - a.0) * f64::from(b.1 - a.1) / (b.0 - a.0)
    };
    for (x, y) in [(0.0, 1.0), (7.5, 2.0), (20.0, 3.0)] {
        let at = plot.pixel(x, y).unwrap();
        assert!((f64::from(at.x) - along(&layout.x_ticks, x)).abs() < 0.01);
        assert!((f64::from(at.y) - along(&layout.y_ticks, y)).abs() < 0.01);
    }
}

struct Sale {
    quarter: &'static str,
}

const QUARTERS: [&str; 3] = ["Q1", "Q2", "Q3"];

#[test]
fn band_pixel_is_the_band_centre_and_value_the_nearest_code() {
    let cx = Context::new_blocking().unwrap();
    let mut plot = Plot::new();
    let x = plot.x(Band::new());
    plot.y(Linear::new());
    let rows: Vec<Sale> = QUARTERS.iter().map(|&quarter| Sale { quarter }).collect();
    plot.circles(rows)
        .x_with(x.encode_key(|r: &Sale| r.quarter).domain(QUARTERS))
        .y(|_: &Sale| 1.0);
    plot.resolve(&cx, SIZE.0, SIZE.1).unwrap();
    let layout = plot.layout().unwrap().clone();
    assert_eq!(layout.x_ticks.len(), 3);
    for (code, &(tick, px)) in layout.x_ticks.iter().enumerate() {
        assert_eq!(tick, code as f64);
        let at = plot.pixel(code as f64, 1.0).unwrap();
        assert!((at.x - px).abs() < 0.01, "{code}: {} vs {px}", at.x);
    }
    let (c0, c1) = (layout.x_ticks[0].1, layout.x_ticks[1].1);
    let y = plot.pixel(0.0, 1.0).unwrap().y;
    let code_at = |px: f32| plot.value(Point::new(px, y)).unwrap().0;
    assert_eq!(code_at(c0 + 0.4 * (c1 - c0)), 0.0);
    assert_eq!(code_at(c0 + 0.6 * (c1 - c0)), 1.0);
    // Beyond the ends the code clamps to the first or last.
    assert_eq!(code_at(-500.0), 0.0);
    assert_eq!(code_at(5000.0), 2.0);
    // A non-finite code cannot be placed.
    assert!(plot.pixel(f64::NAN, 1.0).is_none());
}

#[test]
fn the_right_axis_has_its_own_scale() {
    let cx = Context::new_blocking().unwrap();
    let mut plot = Plot::new();
    plot.y_right(Linear::new().domain(0.0, 1000.0));
    plot.circles(rows(&[0.0, 10.0, 20.0])).x(|r| r.x).y(|r| r.y);
    assert!(plot.pixel_right(0.0, 0.0).is_none(), "unresolved");
    plot.resolve(&cx, SIZE.0, SIZE.1).unwrap();
    let layout = plot.layout().unwrap().clone();
    assert!(!layout.y_right_ticks.is_empty());
    for &(v, px) in &layout.y_right_ticks {
        let at = plot.pixel_right(5.0, v).unwrap();
        assert!((at.y - px).abs() < 0.01, "{v}: {} vs {px}", at.y);
        let (_, back) = plot.value_right(at).unwrap();
        assert!((back - v).abs() < 1.0, "{v} {back}");
    }
    // The left axis is a different map.
    let (left, right) = (plot.pixel(5.0, 500.0), plot.pixel_right(5.0, 500.0));
    assert!((left.unwrap().y - right.unwrap().y).abs() > 1.0);
    // No right axis, no answer.
    let plain = plot_with(Linear::new(), &[0.0, 1.0]);
    assert!(plain.pixel_right(0.0, 1.0).is_none());
    assert!(plain.value_right(Point::new(10.0, 10.0)).is_none());
}

#[test]
fn outside_the_plot_is_extrapolated_and_unmappable_is_none() {
    let plot = plot_with(Linear::new(), &[0.0, 10.0, 20.0]);
    let rect = plot.layout().unwrap().plot;
    let far = plot.pixel(1000.0, -1000.0).unwrap();
    assert!(far.x > rect.right() && far.y > rect.bottom(), "{far:?}");
    let (vx, _) = plot
        .value(Point::new(rect.left() - 100.0, rect.top()))
        .unwrap();
    assert!(vx < 0.0, "{vx}");

    let log = plot_with(Log::new(), &[1.0, 10.0, 100.0]);
    assert!(log.pixel(0.0, 1.0).is_none());
    assert!(log.pixel(-5.0, 1.0).is_none());
    assert!(log.pixel(f64::NAN, 1.0).is_none());
    assert!(log.pixel(1.0, f64::NAN).is_none());
    assert!(log.pixel(1.0, 1.0).is_some());
}

#[test]
fn nothing_before_a_resolve_and_the_chart_trait_forwards() {
    let mut plot = Plot::new();
    plot.circles(rows(&[0.0, 10.0])).x(|r| r.x).y(|r| r.y);
    assert!(Chart::pixel(&plot, 1.0, 1.0).is_none());
    assert!(Chart::value(&plot, Point::new(1.0, 1.0)).is_none());

    // A builder forwards to its plot, through `dyn Chart` too.
    let cx = Context::new_blocking().unwrap();
    let mut chart = gup_core::scatter(rows(&[0.0, 10.0, 20.0]))
        .x(|r| r.x)
        .y(|r| r.y);
    assert!(Chart::pixel(&chart, 5.0, 2.0).is_none());
    chart.resolve(&cx, SIZE.0, SIZE.1).unwrap();
    let at = Chart::pixel(&chart, 5.0, 2.0).unwrap();
    let dynamic: &dyn Chart = &chart;
    assert_eq!(dynamic.pixel(5.0, 2.0), Some(at));
    let (x, y) = dynamic.value(at).unwrap();
    assert!((x - 5.0).abs() < 1e-3 && (y - 2.0).abs() < 1e-3, "{x} {y}");
}

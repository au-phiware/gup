// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GUP-699: a live line keeps zero on its value axis, the chart says what
//! view it has and whether the user moved it, and an automatic axis refits
//! while the view is home.

mod common;

use common::vr::{harness, layout_metadata};
use gup_core::prelude::*;
use gup_core::{Appender, ChartExt, Input, line};
use gup_visual_regression::RgbaImage;

const W: u32 = 640;
const H: u32 = 400;
const N: usize = 1024;

type Row = (f64, f64);
type Case = (Vec<Row>, fn(f64, f64) -> bool);

/// A population that rises to 5,000 over rows 0 to 2,000, falls to about 500
/// by row 2,100 and then wobbles around it.
fn population(i: usize) -> f64 {
    let t = i as f64;
    if i < 2_000 {
        500.0 + 4_500.0 * t / 2_000.0
    } else if i < 2_100 {
        5_000.0 - 4_500.0 * (t - 2_000.0) / 100.0
    } else {
        500.0 + 20.0 * (t * 0.3).sin()
    }
}

/// A live line over the last `N` rows, with a feed.
fn live(zero: bool) -> (LineChart<Row>, Appender<Row>) {
    let mut chart = line(Vec::<Row>::new())
        .x(|r: &Row| r.0)
        .y(|r: &Row| r.1)
        .window(RowWindow::LastRows(N))
        .title("Population");
    if zero {
        chart = chart.y_scale(Linear::new().include_zero());
    }
    let feed = chart.appender();
    (chart, feed)
}

fn resolve(chart: &mut impl Chart, cx: &Context) {
    chart.resolve(cx, W as f32, H as f32).unwrap();
}

/// Feed `rows` in 500-row batches, resolving after each.
fn run(
    chart: &mut impl Chart,
    feed: &Appender<Row>,
    cx: &Context,
    rows: std::ops::Range<usize>,
    f: impl Fn(usize) -> f64,
) {
    let mut at = rows.start;
    while at < rows.end {
        let to = (at + 500).min(rows.end);
        for i in at..to {
            feed.push((i as f64, f(i))).unwrap();
        }
        resolve(chart, cx);
        at = to;
    }
}

fn wheel(chart: &mut impl Chart, lines: f32) {
    let r = chart.layout().unwrap().plot;
    let at = Point::new(r.left() + r.width / 2.0, r.top() + r.height / 2.0);
    assert!(chart.handle(Input::wheel(at, lines)).redraw);
}

fn small(rows: Vec<Row>) -> LineChart<Row> {
    line(rows).x(|r: &Row| r.0).y(|r: &Row| r.1)
}

/// AC1: without the flag the settled line is clear of the axis line, but
/// only just.
#[test]
fn the_settled_line_without_the_flag_is_just_clear_of_the_axis() {
    let cx = Context::new_blocking().unwrap();
    let (mut chart, feed) = live(false);
    run(&mut chart, &feed, &cx, 0..8_000, population);
    let lowest = chart.pixel(7_999.0, 480.0).unwrap().y;
    let bottom = chart.layout().unwrap().plot.bottom();
    let gap = bottom - lowest;
    println!("GUP-699 AC1: gap {gap} px");
    // Measured: a line has no overhang, so nothing widens the axis and the
    // lowest row is exactly on the axis line (the story expected a few px).
    assert!(gap.abs() < 1.0, "gap {gap}");
}

/// AC2: the flag grows an automatic domain to contain 0.
#[test]
fn include_zero_grows_the_fit() {
    let cx = Context::new_blocking().unwrap();
    let cases: [Case; 3] = [
        (vec![(0.0, 500.0), (1.0, 5_000.0)], |lo, hi| {
            lo <= 0.0 && hi >= 5_000.0
        }),
        (vec![(0.0, -500.0), (1.0, -5_000.0)], |lo, hi| {
            lo <= -5_000.0 && hi >= 0.0
        }),
        (vec![(0.0, 500.0), (1.0, 500.0)], |lo, hi| hi > lo),
    ];
    for (rows, check) in cases {
        let mut chart = small(rows).y_scale(Linear::new().include_zero());
        resolve(&mut chart, &cx);
        let (lo, hi) = chart.y_view().unwrap();
        assert!(check(lo, hi), "y view {lo}..{hi}");
    }
    let mut chart = small(vec![(0.0, 500.0), (1.0, 5_000.0)]).y_scale(Linear::new().include_zero());
    resolve(&mut chart, &cx);
    let ticks = &chart.layout().unwrap().y_ticks;
    assert!(ticks.iter().any(|t| t.0 == 0.0), "{ticks:?}");
}

#[test]
fn an_explicit_domain_wins_in_either_order() {
    let cx = Context::new_blocking().unwrap();
    for scale in [
        Linear::new().include_zero().domain(100.0, 200.0),
        Linear::new().domain(100.0, 200.0).include_zero(),
    ] {
        let mut chart = small(vec![(0.0, 150.0), (1.0, 160.0)]).y_scale(scale);
        resolve(&mut chart, &cx);
        let (lo, hi) = chart.y_view().unwrap();
        println!("explicit {lo} {hi}");
        assert!(
            (lo - 100.0).abs() < 5.0 && (hi - 200.0).abs() < 5.0,
            "{lo}..{hi}"
        );
    }
}

#[test]
fn the_flag_survives_zoom_and_reset() {
    let cx = Context::new_blocking().unwrap();
    let mut chart = small(vec![(0.0, 500.0), (1.0, 5_000.0)]).y_scale(Linear::new().include_zero());
    resolve(&mut chart, &cx);
    wheel(&mut chart, 3.0);
    resolve(&mut chart, &cx);
    assert!(chart.y_view().unwrap().0 > 0.0, "the zoom moved the view");
    assert!(chart.handle(Input::key(Key::ResetView)).redraw);
    resolve(&mut chart, &cx);
    assert!(chart.y_view().unwrap().0 <= 0.0);
}

/// AC3: the views equal what `value` gives at the plot rect's edges.
#[test]
fn the_views_are_the_domains_at_the_plot_edges() {
    let cx = Context::new_blocking().unwrap();
    let mut chart = small(vec![(0.0, 500.0), (10.0, 5_000.0)]);
    assert_eq!((chart.x_view(), chart.y_view()), (None, None));
    resolve(&mut chart, &cx);
    let r = chart.layout().unwrap().plot;
    let (mx, my) = (r.left() + r.width / 2.0, r.top() + r.height / 2.0);
    let xl = chart.value(Point::new(r.left(), my)).unwrap().0;
    let xr = chart.value(Point::new(r.right(), my)).unwrap().0;
    let yb = chart.value(Point::new(mx, r.bottom())).unwrap().1;
    let yt = chart.value(Point::new(mx, r.top())).unwrap().1;
    let ((x0, x1), (y0, y1)) = (chart.x_view().unwrap(), chart.y_view().unwrap());
    println!("x {x0} {x1} edges {xl} {xr}; y {y0} {y1} edges {yb} {yt}");
    assert!((x0 - xl).abs() < 1e-6 * (x1 - x0) && (x1 - xr).abs() < 1e-6 * (x1 - x0));
    assert!((y0 - yb).abs() < 1e-6 * (y1 - y0) && (y1 - yt).abs() < 1e-6 * (y1 - y0));
    assert_eq!(chart.plot_mut().y_right_view(), None);
}

#[test]
fn view_is_home_tracks_zoom_drag_and_reset() {
    let cx = Context::new_blocking().unwrap();
    let mut chart = small(vec![(0.0, 500.0), (10.0, 5_000.0)]);
    assert!(chart.view_is_home());
    resolve(&mut chart, &cx);
    assert!(chart.view_is_home());
    wheel(&mut chart, 1.0);
    assert!(!chart.view_is_home());
    let _ = chart.handle(Input::key(Key::ResetView));
    assert!(chart.view_is_home());
    let r = chart.layout().unwrap().plot;
    let a = Point::new(r.left() + 50.0, r.top() + 50.0);
    let b = Point::new(a.x + 40.0, a.y + 20.0);
    let _ = chart.handle(Input::pointer_down(a, Button::Primary));
    let _ = chart.handle(Input::pointer_moved(b));
    let _ = chart.handle(Input::pointer_up(b, Button::Primary));
    assert!(!chart.view_is_home());
    resolve(&mut chart, &cx);
    chart.plot_mut().reset_view();
    assert!(chart.view_is_home());
}

/// AC4: refit while home, hold while moved.
#[test]
fn a_live_view_refits_while_home_and_holds_when_moved() {
    let cx = Context::new_blocking().unwrap();
    let (mut chart, feed) = live(true);
    run(&mut chart, &feed, &cx, 0..600, |_| 1_000.0);
    let first = chart.y_view().unwrap().1;
    // Rising.
    run(&mut chart, &feed, &cx, 600..1_100, |_| 4_000.0);
    let up = chart.y_view().unwrap().1;
    assert!(up >= 4_000.0 && up > first && chart.view_is_home(), "{up}");
    // Falling: the peak leaves the window.
    let end = 1_100 + N + 2 * 1024 + 600;
    run(&mut chart, &feed, &cx, 1_100..end, |_| 1_000.0);
    let down = chart.y_view().unwrap().1;
    assert!(down < 4_000.0, "{down}");
    // Moved.
    wheel(&mut chart, 1.0);
    resolve(&mut chart, &cx);
    let held = (chart.x_view().unwrap(), chart.y_view().unwrap());
    assert!(!chart.view_is_home());
    run(&mut chart, &feed, &cx, end..end + 500, |_| 9_000.0);
    let now = (chart.x_view().unwrap(), chart.y_view().unwrap());
    let bits = |v: ((f64, f64), (f64, f64))| [v.0.0, v.0.1, v.1.0, v.1.1].map(f64::to_bits);
    assert_eq!(bits(now), bits(held));
    assert!(!chart.view_is_home());
    // Reset.
    let _ = chart.handle(Input::key(Key::ResetView));
    resolve(&mut chart, &cx);
    assert!(chart.y_view().unwrap().1 >= 9_000.0 && chart.view_is_home());
}

/// AC5: the settled live line, zero on its axis.
#[test]
fn live_line_settled() {
    let cx = Context::new_blocking().unwrap();
    let (mut chart, feed) = live(true);
    run(&mut chart, &feed, &cx, 0..8_000, population);
    let (image, layout) = chart.render_resolved(&cx, W, H).unwrap();
    let vr = RgbaImage::new(image.width(), image.height(), image.as_raw().clone()).unwrap();
    let meta = layout_metadata(&layout);
    harness()
        .run("gup_core/live_line_settled", Ok((vr, meta)))
        .assert_ok();
}

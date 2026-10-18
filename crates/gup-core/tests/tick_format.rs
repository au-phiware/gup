// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GUP-474: `ScaleRef::tick_format` labels axis ticks (and legend ticks),
//! is measured by thinning and the held plot rect, and reaches the SVG.

mod common;

use common::svg_marks;
use common::vr::{harness, layout_metadata};
use gup_core::geom::{Point, Rect};
use gup_core::prelude::*;
use gup_core::scene::{HAlign, TextRole};
use gup_core::{Input, Layout};
use gup_visual_regression::RgbaImage;

const W: u32 = 560;
const H: u32 = 360;

fn labels(layout: &Layout, on_x: bool) -> Vec<String> {
    let plot = layout.plot;
    layout
        .texts
        .iter()
        .filter(|t| t.run.role == TextRole::TickLabel)
        .filter(|t| (t.bounds.y >= plot.bottom()) == on_x)
        .map(|t| t.run.text.to_string())
        .collect()
}

fn y_ink(layout: &Layout) -> (f32, f32) {
    layout
        .texts
        .iter()
        .filter(|t| t.run.role == TextRole::TickLabel && t.run.anchor.h == HAlign::End)
        .fold((0.0, f32::MAX), |(w, l), t| {
            (w.max(t.bounds.width), l.min(t.bounds.x))
        })
}

type Fmt = Option<fn(f64) -> String>;

/// A scatter plot with the scales' domains and tick formats as given
/// (`None` domains are automatic).
fn xy_plot(
    (xs, xf): (Linear, Fmt),
    (ys, yf): (Linear, Fmt),
    rows: Vec<(f64, f64)>,
) -> (Plot, ScaleRef<Linear>, ScaleRef<Linear>) {
    let mut plot = Plot::new();
    plot.legends(false);
    let (x, y) = (plot.x(xs), plot.y(ys));
    if let Some(f) = xf {
        x.tick_format(f);
    }
    if let Some(f) = yf {
        y.tick_format(f);
    }
    plot.add(Selection::<(f64, f64), Circle>::new(rows))
        .attr(Circle::X, x.encode(|r: &(f64, f64)| r.0))
        .attr(Circle::Y, y.encode(|r: &(f64, f64)| r.1))
        .attr(Circle::RADIUS, Px(4.0))
        .attr(Circle::FILL, Color::hex(0x0072b2));
    (plot, x, y)
}

const WIDE: fn(f64) -> String = |v| format!("{v:.0} million dollars");

#[test]
fn a_format_labels_ticks_and_clones_share_it() {
    let x = ScaleRef::new(Linear::new().domain(0.0, 3.0e6));
    let before = x.tick_labels(3);
    let clone = x.clone();
    clone.tick_format(|v| format!("${:.1}M", v / 1e6));
    assert_eq!(x.tick_labels(3), ["$0.0M", "$1.0M", "$2.0M", "$3.0M"]);
    assert_ne!(before, x.tick_labels(3));
    // Time's formatter receives Unix seconds.
    let t = ScaleRef::new(Time::new().domain(0.0, 4.0 * 86_400.0));
    t.tick_format(|s| format!("{}", s as i64));
    let labels = t.tick_labels(4);
    assert!(
        labels.iter().all(|l| l.parse::<i64>().is_ok()),
        "{labels:?}"
    );
}

/// Without a format, a plot resolves exactly as before; a very wide format
/// makes thinning drop ticks it keeps with the default labels.
#[test]
fn thinning_measures_the_formatted_strings() {
    let cx = Context::new_blocking().unwrap();
    let rows = vec![(0.0, 0.0), (100.0, 100.0)];
    let count = |wide: bool| {
        let x = (Linear::new().domain(0.0, 100.0), wide.then_some(WIDE));
        let y = (Linear::new().domain(0.0, 100.0), None);
        let (mut plot, ..) = xy_plot(x, y, rows.clone());
        let layout = plot.resolve(&cx, 480.0, 300.0).unwrap().layout.clone();
        labels(&layout, true)
    };
    let (plain, wide) = (count(false), count(true));
    assert!(wide.iter().all(|l| l.ends_with(" million dollars")));
    assert!(
        wide.len() < plain.len(),
        "{} vs {}",
        wide.len(),
        plain.len()
    );
}

/// The rect held during a wheel gesture comes from the widest formatted
/// label, as a fresh layout of the same domain would size it.
#[test]
fn the_held_rect_is_sized_by_the_formatted_labels() {
    let cx = Context::new_blocking().unwrap();
    let rows = vec![(0.0, 0.0), (100.0, 8.0)];
    let build = |format: bool, dy: (f64, f64)| {
        let x = (Linear::new().domain(0.0, 100.0), None);
        let y = (Linear::new().domain(dy.0, dy.1), format.then_some(WIDE));
        xy_plot(x, y, rows.clone())
    };
    let size = (400.0, 300.0);
    let rect = |p: &mut Plot| -> Rect { p.resolve(&cx, size.0, size.1).unwrap().layout.plot };
    let default_rect = rect(&mut build(false, (0.0, 8.0)).0);
    let (mut plot, _, y) = build(true, (0.0, 8.0));
    let first = rect(&mut plot);
    assert!(
        first.x > default_rect.x + 50.0,
        "{first:?} {default_rect:?}"
    );
    let at = Point::new(150.0, 120.0);
    assert!(plot.handle(Input::Wheel { at, lines: 2.0 }).redraw);
    let held = rect(&mut plot);
    let (width, left) = y_ink(plot.layout().unwrap());
    assert!(left >= 0.0, "label clipped at {left}");
    assert_eq!(held.x.to_bits(), first.x.to_bits());
    // A fresh layout of the zoomed domain needs the same left margin
    // (up to the hold's padding).
    let dy = y.read().current_domain().unwrap();
    let fresh = rect(&mut build(true, dy).0);
    assert!(held.x >= fresh.x - 0.01, "{held:?} {fresh:?}");
    assert!(width > 60.0, "{width}");
}

fn golden(case: &str, cx: &Context, plot: &mut Plot, expect: &[&str]) {
    let (image, layout) = plot.render_resolved(cx, W, H).unwrap();
    let all: Vec<String> = [labels(&layout, true), labels(&layout, false)].concat();
    for e in expect {
        assert!(all.iter().any(|l| l == e), "{e:?} not in {all:?}");
    }
    let plot_rect = layout.plot;
    for t in layout
        .texts
        .iter()
        .filter(|t| t.run.role == TextRole::TickLabel)
    {
        let b = t.bounds;
        assert!(b.left() >= 0.0 && b.right() <= W as f32, "{b:?} clipped");
        let apart = b.right() <= plot_rect.left()
            || b.bottom() <= plot_rect.top()
            || b.top() >= plot_rect.bottom() - 0.01;
        assert!(apart, "{:?} overlaps the plot", t.run.text);
    }
    let scene = plot.resolve(cx, W as f32, H as f32).unwrap().scene.clone();
    let meta = layout_metadata(&layout);
    let vr = RgbaImage::new(W, H, image.into_raw()).unwrap();
    harness()
        .run(case, Ok((vr.clone(), meta.clone())))
        .assert_ok();
    let svg = svg_marks::assert_matches(case, &scene, &vr, &meta);
    for e in expect {
        assert!(svg.contains(e), "{e:?} missing from the SVG");
    }
}

#[test]
fn currency_axis() {
    let cx = Context::new_blocking().unwrap();
    let rows = (0..40)
        .map(|i| {
            (
                f64::from(i),
                200_000.0 + 1_000_000.0 * (f64::from(i) * 0.4).sin().abs(),
            )
        })
        .collect();
    let (mut plot, ..) = xy_plot(
        (Linear::new(), None),
        (
            Linear::new().domain(0.0, 1.4e6),
            Some(|v| format!("${:.1}M", v / 1e6)),
        ),
        rows,
    );
    golden("gup_core/tick_format_currency", &cx, &mut plot, &["$1.2M"]);
}

#[test]
fn percent_axis() {
    let cx = Context::new_blocking().unwrap();
    let rows = (0..30)
        .map(|i| (f64::from(i), (f64::from(i) * 0.7).sin() * 0.12 + 0.45))
        .collect();
    let (mut plot, ..) = xy_plot(
        (Linear::new(), None),
        (
            Linear::new().domain(0.3, 0.6),
            Some(|v| format!("{:.0}%", v * 100.0)),
        ),
        rows,
    );
    golden("gup_core/tick_format_percent", &cx, &mut plot, &["45%"]);
}

/// "Mar 2020" for Unix seconds `s` (proleptic Gregorian, UTC).
fn month_year(s: f64) -> String {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let z = (s / 86_400.0).floor() as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let month = (mp + 2) % 12;
    let year = yoe + era * 400 + i64::from(month < 2);
    format!("{} {year}", MONTHS[month as usize])
}

#[test]
fn date_axis() {
    let cx = Context::new_blocking().unwrap();
    let t0 = 1_577_836_800.0; // 2020-01-01
    let rows: Vec<(f64, f64)> = (0..900)
        .map(|d| (t0 + f64::from(d) * 86_400.0, 40.0 + f64::from(d) / 40.0))
        .collect();
    let mut plot = Plot::new();
    plot.legends(false);
    let x = plot.x(Time::new());
    x.tick_format(month_year);
    let y = plot.y(Linear::new());
    plot.add(Selection::<(f64, f64), Circle>::new(rows))
        .attr(Circle::X, x.encode(|r: &(f64, f64)| r.0))
        .attr(Circle::Y, y.encode(|r: &(f64, f64)| r.1))
        .attr(Circle::RADIUS, Px(1.5))
        .attr(Circle::FILL, Color::hex(0x0072b2));
    assert_eq!(month_year(t0), "Jan 2020");
    golden("gup_core/tick_format_date", &cx, &mut plot, &["Jan 2021"]);
}

/// A colour chain that starts at a formatted scale labels its legend ramp
/// ticks (in the data's units) with that format.
#[test]
fn legend_ramp_ticks_use_the_format() {
    let s = ScaleRef::new(Linear::new());
    s.tick_format(|v| format!("${:.0}M", v / 1e6));
    let mut chain = s.then(Sequential::viridis());
    chain.fit((0.0, 4e7)).unwrap();
    let Some(gup_core::Legend::Ramp(ramp)) = chain.legend() else {
        panic!("a ramp");
    };
    let labels: Vec<&str> = ramp.ticks().iter().map(|t| &*t.label).collect();
    assert_eq!(labels, ["$0M", "$10M", "$20M", "$30M", "$40M"]);
}

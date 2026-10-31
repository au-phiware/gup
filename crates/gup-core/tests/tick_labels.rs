// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! RFC-001 S7f (GUP-435): tick labels never overlap, and automatic
//! domains widen to cover mark overhang so ticks span the axis.

use gup_core::Resolved;
use gup_core::geom::Rect;
use gup_core::prelude::*;
use gup_core::scene::{ItemKind, TextRole};

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

#[derive(Clone)]
struct Row {
    key: String,
    v: f64,
}

fn keyed(keys: Vec<String>) -> Vec<Row> {
    keys.into_iter()
        .enumerate()
        .map(|(i, key)| Row { key, v: i as f64 })
        .collect()
}

/// Ink boxes of the tick labels whose centres lie along the axis, sorted.
fn tick_boxes(resolved: &Resolved, horizontal: bool) -> Vec<Rect> {
    let plot = resolved.layout.plot;
    let mut boxes: Vec<Rect> = resolved
        .layout
        .texts
        .iter()
        .filter(|t| t.run.role == TextRole::TickLabel)
        .filter(|t| {
            let b = t.bounds;
            if horizontal {
                b.y >= plot.bottom()
            } else {
                b.right() <= plot.left()
            }
        })
        .map(|t| t.bounds)
        .collect();
    boxes.sort_by(|a, b| {
        if horizontal {
            a.x.total_cmp(&b.x)
        } else {
            a.y.total_cmp(&b.y)
        }
    });
    boxes
}

fn assert_separated(resolved: &Resolved, horizontal: bool, what: &str) {
    let gap = Theme::default().label_gap.0;
    let boxes = tick_boxes(resolved, horizontal);
    assert!(boxes.len() >= 2, "{what}: {} labels", boxes.len());
    for w in boxes.windows(2) {
        let sep = if horizontal {
            w[1].x - w[0].right()
        } else {
            w[1].y - w[0].bottom()
        };
        assert!(sep >= gap, "{what}: labels {} px apart, need {gap}", sep);
    }
}

fn band_x(keys: Vec<String>, width: f32, height: f32) -> Resolved {
    let cx = Context::new_blocking().unwrap();
    let mut plot = Plot::new();
    let (x, y) = (plot.x(Band::new()), plot.y(Linear::new()));
    plot.add(Selection::<Row, Circle>::new(keyed(keys)))
        .attr(Circle::X, x.encode_key(|r: &Row| r.key.as_str()))
        .attr(Circle::Y, y.encode(|r: &Row| r.v))
        .attr(Circle::RADIUS, Px(3.0));
    plot.resolve(&cx, width, height).unwrap().clone()
}

#[test]
fn twelve_month_names_on_a_narrow_plot_keep_apart() {
    let keys = MONTHS.iter().map(|m| m.to_string()).collect();
    assert_separated(&band_x(keys, 240.0, 200.0), true, "12 months at 240");
}

#[test]
fn sixty_short_keys_keep_apart() {
    let keys = (0..60).map(|i| format!("k{i}")).collect();
    assert_separated(&band_x(keys, 720.0, 450.0), true, "60 keys at 720");
}

#[test]
fn a_linear_x_on_a_tiny_plot_keeps_apart() {
    let cx = Context::new_blocking().unwrap();
    let mut plot = Plot::new();
    let (x, y) = (plot.x(Linear::new()), plot.y(Linear::new()));
    plot.add(Selection::<(f64, f64), Circle>::new(vec![
        (0.0, 0.0),
        (1000.0, 1.0),
    ]))
    .attr(Circle::X, x.encode(|r: &(f64, f64)| r.0))
    .attr(Circle::Y, y.encode(|r: &(f64, f64)| r.1));
    let r = plot.resolve(&cx, 120.0, 120.0).unwrap();
    assert_separated(r, true, "linear");
}

#[test]
fn forty_keys_on_a_short_band_y_keep_apart() {
    let cx = Context::new_blocking().unwrap();
    let mut plot = Plot::new();
    let (x, y) = (plot.x(Linear::new()), plot.y(Band::new()));
    let keys = (0..40).map(|i| format!("key {i}")).collect();
    plot.add(Selection::<Row, Circle>::new(keyed(keys)))
        .attr(Circle::X, x.encode(|r: &Row| r.v))
        .attr(Circle::Y, y.encode_key(|r: &Row| r.key.as_str()))
        .attr(Circle::RADIUS, Px(3.0));
    assert_separated(plot.resolve(&cx, 720.0, 200.0).unwrap(), false, "band y");
}

/// An automatic domain grows to cover the overhang, so the first and last
/// tick lie within a step of the axis line's ends; a fixed one does not grow,
/// so its ticks stay on 0 and 100 and the range is inset instead.
#[test]
fn an_automatic_domain_widens_and_a_fixed_one_does_not() {
    let cx = Context::new_blocking().unwrap();
    let resolve = |fixed: bool| {
        let mut plot = Plot::new();
        let (x, y) = if fixed {
            (
                plot.x(Linear::new().domain(0.0, 100.0)),
                plot.y(Linear::new().domain(0.0, 100.0)),
            )
        } else {
            (plot.x(Linear::new()), plot.y(Linear::new()))
        };
        plot.add(Selection::<(f64, f64), Circle>::new(vec![
            (0.0, 0.0),
            (100.0, 100.0),
        ]))
        .attr(Circle::X, x.encode(|r: &(f64, f64)| r.0))
        .attr(Circle::Y, y.encode(|r: &(f64, f64)| r.1))
        .attr(Circle::RADIUS, Px(30.0));
        let resolved = plot.resolve(&cx, 480.0, 320.0).unwrap().clone();
        let axes: Vec<_> = resolved
            .scene
            .items
            .iter()
            .filter_map(|i| match &i.kind {
                ItemKind::Rules(r) => Some(r),
                _ => None,
            })
            .flatten()
            .take(2)
            .cloned()
            .collect();
        (resolved, axes)
    };
    for fixed in [false, true] {
        let (r, axes) = resolve(fixed);
        let (xt, yt) = (&r.layout.x_ticks, &r.layout.y_ticks);
        let xs = (xt[1].1 - xt[0].1).abs();
        let ys = (yt[1].1 - yt[0].1).abs();
        let (x0, x1) = (
            axes[0].p0.x.min(axes[0].p1.x),
            axes[0].p0.x.max(axes[0].p1.x),
        );
        let (y0, y1) = (
            axes[1].p0.y.min(axes[1].p1.y),
            axes[1].p0.y.max(axes[1].p1.y),
        );
        assert!((xt[0].1 - x0).abs() <= xs, "fixed {fixed}: first x tick");
        assert!(
            (xt[xt.len() - 1].1 - x1).abs() <= xs,
            "fixed {fixed}: last x"
        );
        let (ya, yb) = (
            yt[0].1.min(yt[yt.len() - 1].1),
            yt[0].1.max(yt[yt.len() - 1].1),
        );
        assert!(
            (ya - y0).abs() <= ys && (yb - y1).abs() <= ys,
            "fixed {fixed}: y"
        );
        // Data 0 sits a mark's overhang inside the widened axis, and the same
        // overhang inside the fixed one's line (GUP-540).
        let lead = xt[0].1 - x0;
        if fixed {
            assert!((lead - 30.0).abs() < 1.0, "fixed: lead {lead}");
        } else {
            assert!(lead >= 30.0, "automatic: lead {lead}");
        }
    }
}

/// A line's automatic domain is never grown by a tick step: the end ticks
/// stay on 0 and 200, at the plot's edges (plus the far end's padding).
#[test]
fn a_line_keeps_its_end_ticks_on_the_data_extent() {
    let cx = Context::new_blocking().unwrap();
    let mut plot = Plot::new();
    let (x, y) = (plot.x(Linear::new()), plot.y(Linear::new()));
    plot.add(Selection::<(f64, f64), Line>::new(vec![
        (0.0, 0.0),
        (200.0, 10.0),
    ]))
    .x_with(x.encode(|r: &(f64, f64)| r.0))
    .y_with(y.encode(|r: &(f64, f64)| r.1))
    .width(Px(2.0));
    let r = plot.resolve(&cx, 720.0, 400.0).unwrap();
    let t = &r.layout.x_ticks;
    let (first, last) = (t[0], t[t.len() - 1]);
    assert_eq!((first.0, last.0), (0.0, 200.0));
    assert!(first.1 - r.layout.plot.left() < 2.0, "{first:?}");
    // A line's width has no Size role, so its overhang is 0 and the domain
    // is not widened: the far end only keeps the plot padding.
    let pad = Theme::default().plot_padding.0;
    assert!(r.layout.plot.right() - last.1 <= pad + 0.5, "{last:?}");
}

/// The y tick labels of a plot of circles at the given `(x, y)` layers.
fn y_labels(layers: &[&[(f64, f64)]]) -> Vec<String> {
    let cx = Context::new_blocking().unwrap();
    let mut plot = Plot::new();
    let (x, y) = (plot.x(Linear::new()), plot.y(Linear::new()));
    for data in layers {
        plot.add(Selection::<(f64, f64), Circle>::new(data.to_vec()))
            .attr(Circle::X, x.encode(|r: &(f64, f64)| r.0))
            .attr(Circle::Y, y.encode(|r: &(f64, f64)| r.1))
            .attr(Circle::RADIUS, Px(3.0));
    }
    let r = plot.resolve(&cx, 640.0, 400.0).unwrap();
    let left = r.layout.plot.left();
    r.layout
        .texts
        .iter()
        .filter(|t| t.run.role == TextRole::TickLabel && t.bounds.right() <= left)
        .map(|t| t.run.text.to_string())
        .collect()
}

/// Whole-number data tick in whole steps; the flag is the AND over every
/// layer fitted in the pass; a new plot starts clean.
#[test]
fn whole_number_data_gets_whole_tick_labels() {
    let whole: &[(f64, f64)] = &[(0.0, 0.0), (1.0, 1.0), (2.0, 2.0), (3.0, 3.0)];
    let labels = y_labels(&[whole]);
    assert!(labels.len() >= 2, "{labels:?}");
    assert!(labels.iter().all(|l| !l.contains('.')), "{labels:?}");

    let labels = y_labels(&[whole, &[(1.0, 1.5)]]);
    assert!(labels.iter().any(|l| l.contains('.')), "{labels:?}");

    let labels = y_labels(&[whole]);
    assert!(labels.iter().all(|l| !l.contains('.')), "{labels:?}");
}

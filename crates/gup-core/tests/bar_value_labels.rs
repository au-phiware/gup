// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GUP-544: value labels on the bar builders: placement at the bar's end,
//! centring in a stacked segment, room at the plot edge, and the goldens.

mod common;

use common::vr::{harness, layout_metadata, rgba8};
use gup_core::geom::Rect;
use gup_core::prelude::*;
use gup_core::scene::TextRole;
use gup_core::{BarChart, Layout, OKABE_ITO};
use gup_visual_regression::RgbaImage;

/// The annotation (label) texts with their text, in no particular order.
fn labels(layout: &Layout) -> Vec<(&str, Rect)> {
    layout
        .texts
        .iter()
        .filter(|t| t.run.role == TextRole::Annotation)
        .map(|t| (&*t.run.text, t.bounds))
        .collect()
}

fn label(layout: &Layout, text: &str) -> Rect {
    labels(layout)
        .into_iter()
        .find(|(t, _)| *t == text)
        .unwrap_or_else(|| panic!("no label {text:?} in {:?}", labels(layout)))
        .1
}

/// The pixel of data value `v`, by the first and last tick.
fn at(ticks: &[(f64, f32)], v: f64) -> f64 {
    let (a, b) = (ticks[0], ticks[ticks.len() - 1]);
    f64::from(a.1) + (v - a.0) * f64::from(b.1 - a.1) / (b.0 - a.0)
}

/// The centre of the axis text `text` (a category's tick label).
fn axis_centre(layout: &Layout, text: &str) -> (f32, f32) {
    let t = layout
        .texts
        .iter()
        .find(|t| t.run.role != TextRole::Annotation && &*t.run.text == text)
        .unwrap_or_else(|| panic!("no axis text {text:?}"));
    (
        t.bounds.x + t.bounds.width / 2.0,
        t.bounds.y + t.bounds.height / 2.0,
    )
}

fn near(a: f64, b: f64, tol: f64, what: &str) {
    assert!((a - b).abs() <= tol, "{what}: {a} vs {b} (±{tol})");
}

struct Row {
    cat: &'static str,
    key: &'static str,
    v: f64,
}

fn row(cat: &'static str, key: &'static str, v: f64) -> Row {
    Row { cat, key, v }
}

fn simple() -> Vec<Row> {
    vec![
        row("A", "", 30.0),
        row("B", "", 50.0),
        row("C", "", -20.0),
        row("D", "", 40.0),
    ]
}

fn render(chart: &mut impl Chart, w: u32, h: u32) -> (image::RgbaImage, Layout) {
    let cx = Context::new_blocking().unwrap();
    chart.render_resolved(&cx, w, h).unwrap()
}

#[test]
fn vertical_labels_sit_four_pixels_past_the_bar_end() {
    let mut chart = gup_core::bar(simple())
        .x(|r| r.cat)
        .y(|r| r.v)
        .value_labels();
    let (_, l) = render(&mut chart, 640, 400);
    for (cat, text, v) in [
        ("A", "30", 30.0),
        ("B", "50", 50.0),
        ("D", "40", 40.0),
        ("C", "\u{2212}20", -20.0),
    ] {
        let b = label(&l, text);
        let end = at(&l.y_ticks, v);
        near(
            f64::from(b.x + b.width / 2.0),
            f64::from(axis_centre(&l, cat).0),
            2.0,
            text,
        );
        if v >= 0.0 {
            near(f64::from(b.bottom()), end - 4.0, 1.0, text);
        } else {
            near(f64::from(b.y), end + 4.0, 1.0, text);
        }
    }
}

#[test]
fn grouped_labels_are_centred_on_their_own_slot() {
    let rows = vec![
        row("A", "x", 30.0),
        row("A", "y", 45.0),
        row("B", "x", 25.0),
        row("B", "y", 50.0),
    ];
    let mut chart = gup_core::bar(rows)
        .x(|r| r.cat)
        .y(|r| r.v)
        .group(|r| r.key)
        .value_labels();
    let (image, l) = render(&mut chart, 640, 400);
    // The bars, as runs of non-white pixels just above the baseline.
    let y = (l.plot.bottom() - 3.0) as u32;
    let mut runs: Vec<(u32, u32)> = Vec::new();
    for x in l.plot.x as u32..l.plot.right() as u32 {
        let on = image.get_pixel(x, y).0 != [255, 255, 255, 255];
        match (on, runs.last_mut()) {
            (true, Some(last)) if last.1 + 1 == x => last.1 = x,
            (true, _) => runs.push((x, x)),
            _ => {}
        }
    }
    assert_eq!(runs.len(), 4, "{runs:?}");
    for (run, text) in runs.iter().zip(["30", "45", "25", "50"]) {
        let b = label(&l, text);
        let centre = f64::from(run.0 + run.1 + 1) / 2.0;
        near(f64::from(b.x + b.width / 2.0), centre, 1.0, text);
    }
}

#[test]
fn horizontal_labels_sit_beside_the_bar_end() {
    let mut chart = gup_core::barh(simple())
        .y(|r| r.cat)
        .x(|r| r.v)
        .value_labels();
    let (_, l) = render(&mut chart, 640, 400);
    for (cat, text, v) in [
        ("A", "30", 30.0),
        ("D", "40", 40.0),
        ("C", "\u{2212}20", -20.0),
    ] {
        let b = label(&l, text);
        let end = at(&l.x_ticks, v);
        if v >= 0.0 {
            near(f64::from(b.x), end + 4.0, 1.0, text);
        } else {
            near(f64::from(b.right()), end - 4.0, 1.0, text);
        }
        near(
            f64::from(b.y + b.height / 2.0),
            f64::from(axis_centre(&l, cat).1),
            1.0,
            text,
        );
    }
}

fn stacked_rows() -> Vec<Row> {
    vec![
        row("A", "x", 20.0),
        row("A", "y", 0.4),
        row("A", "z", 15.0),
        row("B", "x", 10.0),
        row("B", "y", 12.0),
        row("B", "z", 18.0),
    ]
}

#[test]
fn stacked_labels_are_centred_in_their_segment_and_thin_ones_hide() {
    let mut chart = gup_core::bar(stacked_rows())
        .x(|r| r.cat)
        .y(|r| r.v)
        .stack(|r| r.key)
        .value_labels();
    let (_, l) = render(&mut chart, 640, 400);
    // The 0.4 segment is about 3 px tall.
    let thin = at(&l.y_ticks, 0.0) - at(&l.y_ticks, 0.4);
    assert!((2.0..4.0).contains(&thin), "{thin}");
    let texts: Vec<&str> = labels(&l).iter().map(|(t, _)| *t).collect();
    assert!(!texts.contains(&"0.4"), "{texts:?}");
    assert_eq!(texts.len(), 5, "{texts:?}");
    for (text, base, top) in [
        ("20", 0.0, 20.0),
        ("15", 20.4, 35.4),
        ("10", 0.0, 10.0),
        ("12", 10.0, 22.0),
        ("18", 22.0, 40.0),
    ] {
        let b = label(&l, text);
        let mid = (at(&l.y_ticks, base) + at(&l.y_ticks, top)) / 2.0;
        near(f64::from(b.y + b.height / 2.0), mid, 1.0, text);
    }
}

#[test]
fn stacked_horizontal_labels_are_centred_across_their_segment() {
    let mut chart = gup_core::barh(stacked_rows())
        .y(|r| r.cat)
        .x(|r| r.v)
        .stack(|r| r.key)
        .value_labels();
    let (_, l) = render(&mut chart, 640, 400);
    let b = label(&l, "12");
    let mid = (at(&l.x_ticks, 10.0) + at(&l.x_ticks, 22.0)) / 2.0;
    near(f64::from(b.x + b.width / 2.0), mid, 1.0, "12");
    assert!(!labels(&l).iter().any(|(t, _)| *t == "0.4"));
}

#[test]
fn the_tallest_label_fits_inside_the_plot_at_either_height() {
    for h in [160, 400] {
        for (rows, name) in [
            (vec![row("A", "", 30.0), row("B", "", 50.0)], "positive"),
            (vec![row("A", "", -30.0), row("B", "", -50.0)], "negative"),
            (vec![row("A", "", 30.0), row("B", "", -50.0)], "mixed"),
        ] {
            let mut chart = gup_core::bar(rows).x(|r| r.cat).y(|r| r.v).value_labels();
            let (_, l) = render(&mut chart, 640, h);
            let ls = labels(&l);
            assert_eq!(ls.len(), 2, "{name} {h}");
            // A clipped label has a shorter box than its whole digits.
            let whole = ls.iter().map(|(_, b)| b.height).fold(0.0, f32::max);
            for (t, b) in &ls {
                assert!(b.height >= whole - 0.01, "{name} {h}: {t} clipped, {b:?}");
                assert!(
                    b.y >= l.plot.y - 0.01 && b.bottom() <= l.plot.bottom() + 0.01,
                    "{name} {h}: {t} outside {:?}: {b:?}",
                    l.plot
                );
            }
            if name == "positive" {
                near(
                    at(&l.y_ticks, 0.0),
                    f64::from(l.plot.bottom()),
                    0.5,
                    "baseline",
                );
            }
        }
    }
}

#[test]
fn the_tick_format_labels_the_bars_and_the_last_call_wins() {
    let build = || {
        gup_core::bar(simple())
            .x(|r| r.cat)
            .y(|r| r.v)
            .y_tick_format(|v| format!("{v:.0}%"))
    };
    let (_, l) = render(&mut build().value_labels(), 640, 400);
    label(&l, "30%");
    let (_, l) = render(
        &mut build()
            .value_labels()
            .value_labels_with(|v| format!("<{v}>")),
        640,
        400,
    );
    label(&l, "<30>");
    let (_, l) = render(
        &mut build()
            .value_labels_with(|v| format!("<{v}>"))
            .value_labels(),
        640,
        400,
    );
    label(&l, "30%");
}

#[test]
fn a_chart_without_labels_has_no_annotations() {
    let mut chart = gup_core::bar(simple()).x(|r| r.cat).y(|r| r.v);
    let (_, l) = render(&mut chart, 320, 200);
    assert!(labels(&l).is_empty());
}

fn golden<T: Send + Sync + 'static, O: gup_core::builder::Orientation>(
    name: &str,
    chart: &mut BarChart<T, O>,
    colours: &[Color],
    in_fill: bool,
) {
    let (image, layout) = render(chart, 640, 400);
    let mut plain = layout.clone();
    if in_fill {
        // The harness's text check models text over the background; a label
        // inside a segment sits on its fill. Check those here instead: each
        // label's box holds enough pixels of white or `#111111`.
        plain.texts.retain(|t| t.run.role != TextRole::Annotation);
        for (text, b) in labels(&layout) {
            let ink = (b.y as u32..b.bottom() as u32)
                .flat_map(|y| (b.x as u32..b.right() as u32).map(move |x| (x, y)))
                .filter(|&(x, y)| {
                    let p = image.get_pixel(x, y).0;
                    [[255u8; 3], [17; 3]]
                        .iter()
                        .any(|t| (0..3).all(|c| (i32::from(p[c]) - i32::from(t[c])).abs() <= 40))
                })
                .count();
            assert!(ink >= 5, "label {text:?} has {ink} text pixels in {b:?}");
        }
    }
    let mut meta = layout_metadata(&plain);
    for (i, c) in colours.iter().enumerate() {
        meta = meta.with_expected_color(format!("fill {i}"), rgba8(*c));
    }
    let vr = RgbaImage::new(640, 400, image.into_raw()).unwrap();
    harness().run(name, Ok((vr, meta))).assert_ok();
}

#[test]
fn golden_grouped_bars_with_a_negative_value() {
    let mut rows = Vec::new();
    for (q, vals) in [
        ("Q1", [32.5, 24.0, -12.5]),
        ("Q2", [45.0, 51.25, 22.0]),
        ("Q3", [28.0, -9.0, 30.0]),
        ("Q4", [50.0, 41.0, 36.75]),
    ] {
        for (k, v) in ["North", "South", "East"].into_iter().zip(vals) {
            rows.push(row(q, k, v));
        }
    }
    let mut chart = gup_core::bar(rows)
        .x(|r| r.cat)
        .y(|r| r.v)
        .group(|r| r.key)
        .legend_title("Region")
        .y_title("Sales")
        .value_labels();
    golden(
        "gup_core/bar_value_labels",
        &mut chart,
        &OKABE_ITO[..3],
        false,
    );
}

#[test]
fn golden_stacked_horizontal_bars_with_a_hidden_thin_segment() {
    let mut chart = gup_core::barh(vec![
        row("North", "Online", 24.0),
        row("North", "Retail", 0.4),
        row("North", "Wholesale", 15.0),
        row("South", "Online", 12.0),
        row("South", "Retail", 31.0),
        row("South", "Wholesale", 18.0),
        row("East", "Online", 20.0),
        row("East", "Retail", 14.5),
        row("East", "Wholesale", 9.0),
    ])
    .y(|r| r.cat)
    .x(|r| r.v)
    .stack(|r| r.key)
    .legend_title("Channel")
    .x_title("Units")
    .value_labels();
    golden(
        "gup_core/bar_value_labels_stacked",
        &mut chart,
        &OKABE_ITO[..3],
        true,
    );
}

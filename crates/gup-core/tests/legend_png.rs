// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! RFC-001 S7c (GUP-426): colour legends placed by `Plot::resolve`: the
//! layout invariants, titles, overflow ("+N more") and several legends.

mod common;

use common::vr::{harness, layout_metadata};
use gup_core::Layout;
use gup_core::geom::Rect;
use gup_core::prelude::*;
use gup_core::scene::TextRole;
use gup_visual_regression::RgbaImage;

#[derive(Clone)]
struct Row {
    x: f64,
    y: f64,
    group: String,
}

fn rows(keys: usize, per_key: usize) -> Vec<Row> {
    (0..keys * per_key)
        .map(|i| Row {
            x: (i % per_key) as f64 + (i / per_key) as f64 * 0.3,
            y: ((i * 7919) % 101) as f64 / 10.0 + (i / per_key) as f64 * 0.1,
            group: format!("group {:02}", i / per_key),
        })
        .collect()
}

fn categorical_plot(keys: usize, title: Option<&str>) -> (Plot, ScaleRef<Categorical>) {
    let mut plot = Plot::new();
    let (x, y) = (plot.x(Linear::new()), plot.y(Linear::new()));
    let colour = ScaleRef::new(Categorical::okabe_ito());
    let layer = plot
        .add(Selection::<Row, Circle>::new(rows(keys, 6)))
        .attr(Circle::X, x.encode(|r: &Row| r.x))
        .attr(Circle::Y, y.encode(|r: &Row| r.y))
        .attr(Circle::RADIUS, Px(3.5))
        .attr(
            Circle::FILL,
            colour.encode_owned_key(|r: &Row| r.group.clone()),
        );
    if let Some(t) = title {
        layer.legend_title(Circle::FILL, t);
    }
    (plot, colour)
}

fn intersects(a: &Rect, b: &Rect) -> bool {
    a.left() < b.right() && b.left() < a.right() && a.top() < b.bottom() && b.top() < a.bottom()
}

fn canvas(layout: &Layout) -> Rect {
    Rect::new(0.0, 0.0, layout.width, layout.height)
}

fn golden(cx: &Context, case: &str, plot: &mut Plot, w: u32, h: u32) -> (Layout, image::RgbaImage) {
    let (image, layout) = plot.render_resolved(cx, w, h).unwrap();
    let vr = RgbaImage::new(image.width(), image.height(), image.as_raw().clone()).unwrap();
    harness()
        .run(case, Ok((vr, layout_metadata(&layout))))
        .assert_ok();
    (layout, image)
}

fn assert_texts_disjoint_and_inside(layout: &Layout) {
    let canvas = canvas(layout);
    for (i, a) in layout.texts.iter().enumerate() {
        assert!(
            canvas.contains_rect(&a.bounds),
            "{:?} leaves the canvas",
            a.run.text
        );
        for b in &layout.texts[i + 1..] {
            assert!(
                !intersects(&a.bounds, &b.bounds),
                "{:?} overlaps {:?}",
                a.run.text,
                b.run.text
            );
        }
    }
}

/// AC1: the legend column is `Some` exactly when a legend is drawn, lies
/// inside the canvas and clear of the plot; `legends(false)` gives the
/// plot rect it would have without any.
#[test]
fn the_legend_column_is_inside_the_canvas_and_clear_of_the_plot() {
    let cx = Context::new_blocking().unwrap();
    let (mut plot, _) = categorical_plot(5, None);
    let with = plot.resolve(&cx, 640.0, 400.0).unwrap().layout.clone();
    let legend = with.legend.expect("a categorical fill has a legend");
    assert!(canvas(&with).contains_rect(&legend), "{legend:?}");
    assert!(
        !intersects(&legend, &with.plot),
        "{legend:?} {:?}",
        with.plot
    );
    assert!(legend.left() > with.plot.right());

    plot.legends(false);
    let without = plot.resolve(&cx, 640.0, 400.0).unwrap().layout.clone();
    assert_eq!(without.legend, None);
    assert!(without.plot.width > with.plot.width);
    assert!(
        without.texts.iter().all(|t| t.run.role != TextRole::Legend),
        "no legend text"
    );

    // A plot with no colour scale has no legend, and the plot rect is
    // the same as with `legends(false)`.
    let mut plain = Plot::new();
    let (x, y) = (plot_scale(&mut plain, true), plot_scale(&mut plain, false));
    plain
        .add(Selection::<Row, Circle>::new(rows(5, 6)))
        .attr(Circle::X, x.encode(|r: &Row| r.x))
        .attr(Circle::Y, y.encode(|r: &Row| r.y))
        .attr(Circle::RADIUS, Px(3.5));
    let plain = plain.resolve(&cx, 640.0, 400.0).unwrap().layout.clone();
    assert_eq!(plain.legend, None);
    assert_eq!(plain.plot, without.plot);
}

fn plot_scale(plot: &mut Plot, x: bool) -> ScaleRef<Linear> {
    if x {
        plot.x(Linear::new())
    } else {
        plot.y(Linear::new())
    }
}

/// AC2: a chart title, axis titles and a titled legend, none overlapping.
#[test]
fn titles_and_a_titled_legend_do_not_overlap() {
    let cx = Context::new_blocking().unwrap();
    let (mut plot, _) = categorical_plot(6, Some("Group"));
    plot.title("Readings by group")
        .x_title("Time (s)")
        .y_title("Level (mV)");
    let (layout, _) = golden(&cx, "gup_core/legend_guides", &mut plot, 720, 450);
    let legend_texts: Vec<_> = layout
        .texts
        .iter()
        .filter(|t| t.run.role == TextRole::Legend)
        .collect();
    assert_eq!(&*legend_texts[0].run.text, "Group");
    assert_eq!(legend_texts.len(), 1 + 6);
    assert_texts_disjoint_and_inside(&layout);
}

/// AC3: 60 keys on a short plot: as many rows as fit, then "+N more", and
/// nothing below the canvas.
#[test]
fn a_long_domain_shows_what_fits_and_counts_the_rest() {
    let cx = Context::new_blocking().unwrap();
    let (mut plot, colour) = categorical_plot(60, None);
    let layout = plot.resolve(&cx, 720.0, 300.0).unwrap().layout.clone();
    let keys = colour.read().legend().unwrap().swatches().len();
    assert_eq!(keys, 60);
    let legend = layout.legend.unwrap();
    assert!(legend.bottom() <= layout.plot.bottom() + 0.5, "{legend:?}");
    assert!(canvas(&layout).contains_rect(&legend));
    let texts: Vec<&str> = layout
        .texts
        .iter()
        .filter(|t| t.run.role == TextRole::Legend)
        .map(|t| &*t.run.text)
        .collect();
    let (more, shown) = texts.split_last().unwrap();
    assert!(
        !shown.is_empty() && shown.len() < 59,
        "{} rows",
        shown.len()
    );
    assert_eq!(*more, format!("+{} more", keys - shown.len()));
    assert_texts_disjoint_and_inside(&layout);
}

/// AC4: a categorical and a sequential fill stack two legends without
/// overlap; two layers sharing one categorical scale give one.
#[test]
fn two_layers_stack_two_legends_and_a_shared_scale_gives_one() {
    let cx = Context::new_blocking().unwrap();
    let mut plot = Plot::new();
    let (x, y) = (plot.x(Linear::new()), plot.y(Linear::new()));
    let group = ScaleRef::new(Categorical::okabe_ito());
    let heat = ScaleRef::new(Sequential::viridis());
    plot.title("Two encodings")
        .add(Selection::<Row, Circle>::new(rows(4, 6)))
        .attr(Circle::X, x.encode(|r: &Row| r.x))
        .attr(Circle::Y, y.encode(|r: &Row| r.y))
        .attr(Circle::RADIUS, Px(4.0))
        .attr(
            Circle::FILL,
            group.encode_owned_key(|r: &Row| r.group.clone()),
        )
        .legend_title(Circle::FILL, "Group");
    plot.add(Selection::<Row, Circle>::new(rows(4, 6)))
        .attr(Circle::X, x.encode(|r: &Row| r.x + 0.4))
        .attr(Circle::Y, y.encode(|r: &Row| r.y + 0.4))
        .attr(Circle::RADIUS, Px(2.5))
        .attr(Circle::FILL, heat.encode(|r: &Row| r.y))
        .legend_title(Circle::FILL, "Level");
    let (layout, image) = golden(&cx, "gup_core/legend_two_layers", &mut plot, 720, 450);
    let legend = layout.legend.unwrap();
    let swatches: Vec<_> = layout
        .guides
        .iter()
        .filter(|g| g.width == 12.0 && legend.contains_rect(g))
        .collect();
    let bars: Vec<_> = layout
        .guides
        .iter()
        .filter(|g| g.width == 14.0 && legend.contains_rect(g))
        .collect();
    assert_eq!((swatches.len(), bars.len()), (4, 1));
    assert!(
        swatches.iter().all(|s| s.bottom() <= bars[0].top()),
        "the ramp is below the swatches"
    );
    assert_texts_disjoint_and_inside(&layout);

    // The ramp's ticks sit where the scale's CPU mirror puts their values:
    // the bar's colour on a tick's row is the mirror's colour for it, and
    // the ticks are evenly spaced (a linear scale, a round step).
    let bar = *bars[0];
    let ticks: Vec<&Rect> = layout
        .guides
        .iter()
        .filter(|g| g.width == 5.0 && g.height == 1.0 && legend.inflate(1.0).contains_rect(g))
        .collect();
    let ramp = heat.read().ramp();
    assert_eq!(ticks.len(), ramp.ticks().len());
    let mut centres = Vec::new();
    for (tick, rect) in ramp.ticks().iter().zip(&ticks) {
        let y = rect.y + rect.height / 2.0;
        let want = bar.bottom() - tick.t as f32 * bar.height;
        assert!(
            (y - want).abs() <= 0.5,
            "tick {}: {y} vs {want}",
            tick.label
        );
        let got = image.get_pixel((bar.x + 7.0) as u32, y as u32).0;
        let mirror = heat.read().eval(tick.value).to_rgba8();
        for k in 0..3 {
            assert!(
                got[k].abs_diff(mirror[k]) <= 4,
                "{}: {got:?} vs {mirror:?}",
                tick.label
            );
        }
        centres.push(y);
    }
    let gaps: Vec<f32> = centres.windows(2).map(|w| w[0] - w[1]).collect();
    assert!(
        gaps.iter().all(|g| (g - gaps[0]).abs() <= 1.0),
        "uneven ticks {gaps:?}"
    );
    for g in swatches.iter().chain(&bars) {
        for t in &layout.texts {
            assert!(!intersects(g, &t.bounds), "{g:?} overlaps {:?}", t.run.text);
        }
    }

    // The same scale on both layers: one legend of its four keys.
    let mut shared = Plot::new();
    let (x, y) = (shared.x(Linear::new()), shared.y(Linear::new()));
    let group = ScaleRef::new(Categorical::okabe_ito());
    for dy in [0.0, 0.4] {
        shared
            .add(Selection::<Row, Circle>::new(rows(4, 6)))
            .attr(Circle::X, x.encode(|r: &Row| r.x))
            .attr(Circle::Y, y.encode(move |r: &Row| r.y + dy))
            .attr(Circle::RADIUS, Px(3.0))
            .attr(
                Circle::FILL,
                group.encode_owned_key(|r: &Row| r.group.clone()),
            );
    }
    let layout = shared.resolve(&cx, 720.0, 450.0).unwrap().layout.clone();
    let legend_rows = layout
        .texts
        .iter()
        .filter(|t| t.run.role == TextRole::Legend)
        .count();
    assert_eq!(legend_rows, 4);
}

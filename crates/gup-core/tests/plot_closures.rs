// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GUP-447: default axes, the mark adders on `Plot`, and plain closures on
//! position channels, which encode through the plot's own scales.

mod common;

use common::vr::rgba8;
use gup_core::geom::Point;
use gup_core::prelude::*;
use gup_core::{ImageTarget, Layout};

const W: u32 = 320;
const H: u32 = 200;
const FILL: Color = Color::hex(0x0072b2);

type Row = (f64, f64);

const ROWS: [Row; 4] = [(1.0, 2.0), (2.0, 5.0), (3.0, 3.0), (4.0, 8.0)];

fn render(cx: &Context, plot: &mut Plot) -> (image::RgbaImage, Layout) {
    let resolved = plot.resolve(cx, W as f32, H as f32).unwrap();
    let image = ImageTarget::new(cx, W, H)
        .unwrap()
        .render_blocking(cx, &resolved.scene)
        .unwrap();
    (image, resolved.layout)
}

/// The pixel at `value` along an axis, interpolated from its ticks.
fn at(ticks: &[(f64, f32)], value: f64) -> f64 {
    let (a, b) = (ticks[0], *ticks.last().unwrap());
    f64::from(a.1) + (value - a.0) * f64::from(b.1 - a.1) / (b.0 - a.0)
}

fn pixel(image: &image::RgbaImage, x: f64, y: f64) -> gup_visual_regression::Rgba8 {
    let p = image.get_pixel(x.floor() as u32, y.floor() as u32).0;
    gup_visual_regression::Rgba8::rgb(p[0], p[1], p[2])
}

fn assert_fill(image: &image::RgbaImage, x: f64, y: f64) {
    let got = pixel(image, x, y);
    let de = got.delta_e(rgba8(FILL));
    assert!(de < 2.0, "({x}, {y}): {got:?} is dE {de} from the fill");
}

fn assert_white(image: &image::RgbaImage, x: f64, y: f64) {
    let got = pixel(image, x, y);
    let de = got.delta_e(gup_visual_regression::Rgba8::rgb(255, 255, 255));
    assert!(de < 2.0, "({x}, {y}): {got:?} is dE {de} from white");
}

fn short_circles() -> Plot {
    let mut plot = Plot::new();
    plot.circles(ROWS).x(|r| r.0).y(|r| r.1);
    plot
}

#[test]
fn short_form_draws_each_row_at_its_position() {
    let cx = Context::new_blocking().unwrap();
    let (image, layout) = render(&cx, &mut short_circles());
    for (a, b) in ROWS {
        assert_fill(&image, at(&layout.x_ticks, a), at(&layout.y_ticks, b));
    }
}

#[test]
fn short_form_equals_long_form_byte_for_byte() {
    let cx = Context::new_blocking().unwrap();
    let (short, short_layout) = render(&cx, &mut short_circles());

    let mut plot = Plot::new();
    let (x, y) = (plot.x(Linear::new()), plot.y(Linear::new()));
    plot.add(Selection::<Row, Circle>::new(ROWS))
        .x_with(x.encode(|p: &Row| p.0))
        .y_with(y.encode(|p: &Row| p.1));
    let (long, long_layout) = render(&cx, &mut plot);

    assert_eq!(short_layout, long_layout);
    assert_eq!(short.as_raw(), long.as_raw());
}

#[test]
fn rect_edges_and_segment_endpoints_take_closures() {
    let cx = Context::new_blocking().unwrap();

    // One box from (2, 10) to (6, 30).
    let mut plot = Plot::new();
    plot.rects([(2.0, 10.0, 6.0, 30.0)])
        .x(|r| r.0)
        .y(|r| r.1)
        .x_end(|r| r.2)
        .y_end(|r| r.3);
    let (image, layout) = render(&cx, &mut plot);
    let (x0, x1) = (at(&layout.x_ticks, 2.0), at(&layout.x_ticks, 6.0));
    let (y0, y1) = (at(&layout.y_ticks, 30.0), at(&layout.y_ticks, 10.0));
    assert_fill(&image, x0 + 3.0, y0 + 3.0);
    assert_fill(&image, x1 - 3.0, y1 - 3.0);
    assert_white(&image, x0 - 3.0, y0 - 3.0);
    assert_white(&image, x1 + 3.0, y1 + 3.0);

    // One segment from (1, 1) to (9, 9), three pixels wide.
    let mut plot = Plot::new();
    plot.segments([(1.0, 1.0, 9.0, 9.0)])
        .x(|r| r.0)
        .y(|r| r.1)
        .x_end(|r| r.2)
        .y_end(|r| r.3)
        .width(Px(3.0));
    let (image, layout) = render(&cx, &mut plot);
    for t in [3.0, 7.0] {
        assert_fill(&image, at(&layout.x_ticks, t), at(&layout.y_ticks, t));
    }
    assert_white(&image, at(&layout.x_ticks, 3.0), at(&layout.y_ticks, 7.0));
}

#[test]
fn line_takes_closures_and_equals_the_long_form() {
    let cx = Context::new_blocking().unwrap();
    let mut plot = Plot::new();
    plot.line(ROWS).x(|r| r.0).y(|r| r.1).width(Px(3.0));
    let (short, layout) = render(&cx, &mut plot);
    // The end vertices sit on the axes; the middle ones are in the open.
    for (a, b) in &ROWS[1..3] {
        assert_fill(&short, at(&layout.x_ticks, *a), at(&layout.y_ticks, *b));
    }

    let mut plot = Plot::new();
    let (x, y) = (plot.x(Linear::new()), plot.y(Linear::new()));
    plot.add(Selection::<Row, Line>::new(ROWS))
        .x_with(x.encode(|p: &Row| p.0))
        .y_with(y.encode(|p: &Row| p.1))
        .width(Px(3.0));
    let (long, _) = render(&cx, &mut plot);
    assert_eq!(short.as_raw(), long.as_raw());
}

#[test]
fn replacing_the_scale_rebinds_once_and_zoom_uploads_no_columns() {
    let cx = Context::new_blocking().unwrap();
    let mut plot = short_circles();
    let (before, layout) = render(&cx, &mut plot);
    let linear_x = at(&layout.x_ticks, 3.0);
    assert_fill(&before, linear_x, at(&layout.y_ticks, 3.0));

    let mark = cx.upload_stats();
    plot.x(Log::new());
    let (after, layout) = render(&cx, &mut plot);
    // Log ticks are in data units: interpolate in log space.
    let ticks: Vec<(f64, f32)> = layout.x_ticks.iter().map(|&(v, p)| (v.ln(), p)).collect();
    let log_x = at(&ticks, 3.0f64.ln());
    assert!((log_x - linear_x).abs() > 3.0, "the point did not move");
    assert_fill(&after, log_x, at(&layout.y_ticks, 3.0));
    let rebound = cx.upload_stats() - mark;
    assert!(rebound.columns.bytes > 0, "{rebound:?}");

    // Same generation: nothing is bound or uploaded again.
    let mark = cx.upload_stats();
    plot.zoom(Point::new(100.0, 100.0), 0.5).unwrap();
    render(&cx, &mut plot);
    let zoomed = cx.upload_stats() - mark;
    assert_eq!(zoomed.columns.bytes, 0, "{zoomed:?}");
}

#[test]
fn closures_where_there_is_no_scale_fail_at_resolve() {
    let cx = Context::new_blocking().unwrap();

    // A size channel has no plot scale to encode through.
    let mut plot = Plot::new();
    plot.circles(ROWS)
        .x(|r| r.0)
        .y(|r| r.1)
        .attr_axis(Circle::RADIUS, |r: &Row| r.0);
    let err = plot.resolve(&cx, 300.0, 200.0).unwrap_err().to_string();
    assert!(err.contains("Circle::RADIUS"), "{err}");
    assert!(err.contains("has no plot scale"), "{err}");
    assert!(err.contains("Px(4.0)"), "{err}");

    // A key scale reads dictionary keys.
    let mut plot = Plot::new();
    plot.x(Band::new());
    plot.circles(ROWS).x(|r| r.0).y(|r| r.1);
    let err = plot.resolve(&cx, 300.0, 200.0).unwrap_err().to_string();
    assert!(err.contains("plot x"), "{err}");
    assert!(err.contains("encode_key"), "{err}");
}

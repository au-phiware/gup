// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! RFC-001 S7a (GUP-424): several layers in one `Plot`, with domains
//! unioned across layers and drawn in z-order.

mod common;

use common::svg_marks;
use common::vr::{harness, layout_metadata, rgba8};
use gup_core::ScaleRef;
use gup_core::prelude::*;
use gup_visual_regression::RgbaImage;

const WIDTH: u32 = 720;
const HEIGHT: u32 = 450;

type Row = (f64, f64);

fn circle_layer<'a>(
    plot: &'a mut Plot,
    (x, y): (&ScaleRef<Linear>, &ScaleRef<Linear>),
    rows: Vec<Row>,
    radius: f32,
    color: Color,
) -> &'a mut Selection<Row, Circle> {
    plot.add(Selection::<Row, Circle>::new(rows))
        .attr(Circle::X, x.encode(|r: &Row| r.0))
        .attr(Circle::Y, y.encode(|r: &Row| r.1))
        .attr(Circle::RADIUS, Px(radius))
        .attr(Circle::FILL, color)
}

#[test]
fn two_layers_union_domains_and_overhang() {
    let cx = Context::new_blocking().unwrap();
    let mut plot = Plot::new();
    let (x, y) = (plot.x(Linear::new()), plot.y(Linear::new()));
    let a: Vec<_> = (0..=10).map(|i| (f64::from(i), f64::from(i))).collect();
    let b: Vec<_> = (5..=30)
        .map(|i| (f64::from(i), 30.0 - f64::from(i)))
        .collect();
    circle_layer(&mut plot, (&x, &y), a, 3.0, Color::BLACK);
    circle_layer(&mut plot, (&x, &y), b, 9.0, Color::BLACK);
    let (_, layout) = plot.render_resolved(&cx, WIDTH, HEIGHT).unwrap();
    let (lo, hi) = x.read().current_domain().unwrap();
    assert!(lo <= 0.0 && hi >= 30.0, "x domain {lo}..{hi}");
    assert_eq!(layout.mark_overhang, 9.0);
    let p = layout.plot;
    for (r, rows) in [(3.0f32, 0..=10), (9.0, 5..=30)] {
        for i in rows {
            let px = x.read().eval(f64::from(i)) as f32;
            assert!(px - r >= p.left() && px + r <= p.right(), "x {i}: {px}");
        }
    }
}

#[test]
fn no_layers_is_an_error() {
    let cx = Context::new_blocking().unwrap();
    let mut plot = Plot::new();
    plot.x(Linear::new());
    plot.y(Linear::new());
    let err = plot.resolve(&cx, 300.0, 200.0).unwrap_err();
    assert!(err.to_string().contains("plot.add(..)"), "{err}");
}

#[test]
fn z_out_of_range_is_an_error() {
    let cx = Context::new_blocking().unwrap();
    for z in [-100, 100] {
        let mut plot = Plot::new();
        let (x, y) = (plot.x(Linear::new()), plot.y(Linear::new()));
        let rows = vec![(0.0, 0.0), (1.0, 1.0)];
        circle_layer(&mut plot, (&x, &y), rows, 4.0, Color::BLACK).z(z);
        let err = plot.resolve(&cx, 300.0, 200.0).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("layer z") && msg.contains("-99..=99"), "{msg}");
    }
}

/// The pixel at the centre of the overlap shows the layer on top.
#[test]
fn z_orders_layers() {
    let cx = Context::new_blocking().unwrap();
    let red = Color::hex(0xd62728);
    let blue = Color::hex(0x1f77b4);
    let mut shown = Vec::new();
    for first_z in [0, 1] {
        let mut plot = Plot::new();
        let (x, y) = (plot.x(Linear::new()), plot.y(Linear::new()));
        // Both layers have the same rows, so their middle discs coincide.
        let rows = vec![(0.0, 0.0), (5.0, 5.0), (10.0, 10.0)];
        circle_layer(&mut plot, (&x, &y), rows.clone(), 12.0, red).z(first_z);
        circle_layer(&mut plot, (&x, &y), rows, 12.0, blue);
        let (image, _) = plot.render_resolved(&cx, WIDTH, HEIGHT).unwrap();
        let px = x.read().eval(5.0).round() as u32;
        let py = y.read().eval(5.0).round() as u32;
        shown.push(image.get_pixel(px, py).0);
    }
    assert_eq!(
        shown[0],
        blue.to_rgba8(),
        "default z: the second layer is on top"
    );
    assert_eq!(shown[1], red.to_rgba8(), "z(1): the first layer is on top");
}

/// A forecast of small grey circles under larger observed blue points.
#[test]
fn composite_two_layers() {
    let cx = Context::new_blocking().unwrap();
    let grey = Color::hex(0x9a9a9a);
    let blue = Color::hex(0x1f77b4);
    let mut plot = Plot::new();
    let (x, y) = (plot.x(Linear::new()), plot.y(Linear::new()));
    let curve = |t: f64| 10.0 + 6.0 * (t * 0.7).sin() + 0.5 * t;
    let forecast: Vec<_> = (0..=48)
        .map(|i| f64::from(i) * 0.25)
        .map(|t| (t, curve(t)))
        .collect();
    let observed: Vec<_> = (0..=12)
        .map(f64::from)
        .map(|t| (t, curve(t) + 1.5 * (t * 2.3).cos()))
        .collect();
    plot.title("Forecast and observed");
    circle_layer(&mut plot, (&x, &y), forecast, 2.0, grey);
    circle_layer(&mut plot, (&x, &y), observed, 5.0, blue).z(1);
    let (image, layout) = plot.render_resolved(&cx, WIDTH, HEIGHT).unwrap();
    let scene = plot
        .resolve(&cx, WIDTH as f32, HEIGHT as f32)
        .unwrap()
        .scene
        .clone();
    let meta = layout_metadata(&layout)
        .with_expected_color("forecast", rgba8(grey))
        .with_expected_color("observed", rgba8(blue));
    let vr = RgbaImage::new(image.width(), image.height(), image.into_raw()).unwrap();
    harness()
        .run(
            "gup_core/composite_two_layers",
            Ok((vr.clone(), meta.clone())),
        )
        .assert_ok();
    // GUP-467: the SVG draws the same marks.
    svg_marks::assert_matches("gup_core/composite_two_layers", &scene, &vr, &meta);
}

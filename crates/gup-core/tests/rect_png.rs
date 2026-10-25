// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! RFC-001 S6c (GUP-429): the `Rect` mark and scale-derived constants,
//! rendered headlessly. Edges are measured on the pixels: a bar's ink
//! runs along a row or column through its centre.

mod common;

use common::svg_marks;
use common::vr::{harness, layout_metadata, rgba8};
use gup_core::ImageTarget;
use gup_core::prelude::*;
use gup_visual_regression::RgbaImage;
use std::sync::{Arc, Mutex};

const WIDTH: u32 = 720;
const HEIGHT: u32 = 450;

fn render(cx: &Context, plot: &mut Plot, width: u32, height: u32) -> image::RgbaImage {
    let resolved = plot.resolve(cx, width as f32, height as f32).unwrap();
    ImageTarget::new(cx, width, height)
        .unwrap()
        .render_blocking(cx, &resolved.scene)
        .unwrap()
}

/// How far a pixel is from the background (white) towards a fill with a
/// red channel of `fill_red`, in 0..1.
fn coverage(image: &image::RgbaImage, x: u32, y: u32, fill_red: u8) -> f64 {
    let r = f64::from(image.get_pixel(x, y).0[0]);
    ((255.0 - r) / (255.0 - f64::from(fill_red))).clamp(0.0, 1.0)
}

/// The contiguous run of pixels (red below 128) through `(x, y)`, along x
/// when `horizontal`: `(first, one past the last)`.
fn run(image: &image::RgbaImage, x: u32, y: u32, horizontal: bool) -> (u32, u32) {
    let ink = |i: i64| {
        let (px, py) = if horizontal {
            (i, i64::from(y))
        } else {
            (i64::from(x), i)
        };
        px >= 0
            && py >= 0
            && px < i64::from(image.width())
            && py < i64::from(image.height())
            && image.get_pixel(px as u32, py as u32).0[0] < 128
    };
    let at = i64::from(if horizontal { x } else { y });
    assert!(ink(at), "no ink at ({x}, {y})");
    let (mut lo, mut hi) = (at, at + 1);
    while ink(lo - 1) {
        lo -= 1;
    }
    while ink(hi) {
        hi += 1;
    }
    (lo as u32, hi as u32)
}

/// Pixel `(x, y)` is the fill (within 2/255) or the background.
fn assert_pixel(image: &image::RgbaImage, x: u32, y: u32, fill: Option<Color>) {
    let want = fill.map_or([255; 4], Color::to_rgba8);
    let got = image.get_pixel(x, y).0;
    for k in 0..4 {
        assert!(
            got[k].abs_diff(want[k]) <= 2,
            "({x}, {y}): {got:?}, wanted {want:?}"
        );
    }
}

/// AC1: a rect from a centre and a size, and one from two edges given in
/// reverse order. The pixels 1 px inside every edge are the fill, those
/// 1 px outside the background.
#[test]
fn centre_and_edge_forms_cover_exactly_their_pixels() {
    let cx = Context::new_blocking().unwrap();
    let fill = Rect::DEFAULT_FILL;
    let mut plot = Plot::new();
    plot.x(Linear::new().domain(0.0, 1.0));
    plot.y(Linear::new().domain(0.0, 1.0));
    plot.add(Selection::<(), Rect>::new(vec![()]))
        .attr(Rect::X, Px(100.0))
        .attr(Rect::Y, Px(100.0))
        .attr(Rect::WIDTH, Px(60.0))
        .attr(Rect::HEIGHT, Px(40.0));
    plot.add(Selection::<(), Rect>::new(vec![()]))
        .attr(Rect::X, Px(300.0))
        .attr(Rect::X_END, Px(220.0))
        .attr(Rect::Y, Px(150.0))
        .attr(Rect::Y_END, Px(110.0));
    let image = render(&cx, &mut plot, 400, 300);
    // x 70..130, y 80..120; and x 220..300, y 110..150.
    for (x0, x1, y0, y1) in [(70, 130, 80, 120), (220, 300, 110, 150)] {
        let (xm, ym) = ((x0 + x1) / 2, (y0 + y1) / 2);
        assert_pixel(&image, x0, ym, Some(fill));
        assert_pixel(&image, x0 - 1, ym, None);
        assert_pixel(&image, x1 - 1, ym, Some(fill));
        assert_pixel(&image, x1, ym, None);
        assert_pixel(&image, xm, y0, Some(fill));
        assert_pixel(&image, xm, y0 - 1, None);
        assert_pixel(&image, xm, y1 - 1, Some(fill));
        assert_pixel(&image, xm, y1, None);
    }
}

/// AC2: a `Derived` channel is recomputed on every resolve and written
/// as a uniform: the disc's measured radius follows the shared value and
/// no column bytes are uploaded.
#[test]
fn a_derived_radius_follows_its_closure_without_column_writes() {
    let cx = Context::new_blocking().unwrap();
    let r = Arc::new(Mutex::new(10.0f32));
    let shared = Arc::clone(&r);
    let mut plot = Plot::new();
    plot.x(Linear::new().domain(0.0, 1.0));
    plot.y(Linear::new().domain(0.0, 1.0));
    plot.add(Selection::<(), Circle>::new(vec![()]))
        .attr(Circle::X, Px(150.0))
        .attr(Circle::Y, Px(100.0))
        .attr(Circle::FILL, Color::hex(0x000000))
        .attr(
            Circle::RADIUS,
            Derived::new(move || Px(*shared.lock().unwrap())),
        );
    let measured = |image: &image::RgbaImage| -> f64 {
        (100..200).map(|x| coverage(image, x, 100, 0)).sum::<f64>() / 2.0
    };
    let first = render(&cx, &mut plot, 300, 200);
    assert!(
        (measured(&first) - 10.0).abs() < 0.5,
        "{}",
        measured(&first)
    );

    *r.lock().unwrap() = 25.0;
    // The closure reads state the plot cannot see: touch the layer so the
    // cached resolve is not reused (see `Derived`).
    let _ = plot.layer_mut::<(), Circle>();
    let start = cx.upload_stats();
    let second = render(&cx, &mut plot, 300, 200);
    let written = cx.upload_stats() - start;
    assert!(
        (measured(&second) - 25.0).abs() < 0.5,
        "{}",
        measured(&second)
    );
    assert_eq!(written.columns.bytes, 0, "{written:?}");
}

struct Sale {
    month: &'static str,
    total: f64,
}

const MONTHS: [&str; 6] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun"];

fn sales() -> Vec<Sale> {
    [32.0, 45.0, 28.0, 51.0, 60.0, 39.0]
        .iter()
        .zip(MONTHS)
        .map(|(&total, month)| Sale { month, total })
        .collect()
}

/// AC4: six monthly totals as bars sized by the band scale. Each bar's
/// edges are within a pixel of its band's centre minus and plus half
/// `band_width`, and the top of each is at its value.
#[test]
fn monthly_bars_are_as_wide_as_their_band() {
    let cx = Context::new_blocking().unwrap();
    let mut plot = Plot::new();
    let (x, y) = (plot.x(Band::new()), plot.y(Linear::new().domain(0.0, 60.0)));
    let baseline = y.clone();
    plot.title("Monthly sales")
        .add(Selection::<Sale, Rect>::new(sales()))
        .attr(Rect::X, x.encode_key(|s: &Sale| s.month).domain(MONTHS))
        .attr(Rect::WIDTH, x.band_width_px())
        .attr(Rect::Y, y.encode(|s: &Sale| s.total))
        .attr(
            Rect::Y_END,
            Derived::new(move || Px(baseline.read().eval(0.0) as f32)),
        );
    let resolved = plot.resolve(&cx, WIDTH as f32, HEIGHT as f32).unwrap();
    let image = ImageTarget::new(&cx, WIDTH, HEIGHT)
        .unwrap()
        .render_blocking(&cx, &resolved.scene)
        .unwrap();
    let vr = RgbaImage::new(image.width(), image.height(), image.as_raw().clone()).unwrap();
    let meta = layout_metadata(&resolved.layout)
        .with_expected_color("bar fill", rgba8(Rect::DEFAULT_FILL));
    harness()
        .run("gup_core/rect_bars", Ok((vr.clone(), meta.clone())))
        .assert_ok();
    // GUP-467: the SVG draws the same marks.
    svg_marks::assert_matches("gup_core/rect_bars", &resolved.scene, &vr, &meta);

    // GUP-435: the tallest bar stops short of the plot rect's edge.
    let top = y.read().eval(60.0) as f32;
    assert!(
        top >= resolved.layout.plot.top() + 4.0,
        "top bar at {top}, plot top {}",
        resolved.layout.plot.top()
    );
    // GUP-435: bars sit on the x axis line, and the axes meet.
    let axes: Vec<&gup_core::scene::Rule> = resolved
        .scene
        .items
        .iter()
        .filter_map(|i| match &i.kind {
            gup_core::scene::ItemKind::Rules(r) => Some(r),
            _ => None,
        })
        .flatten()
        .take(2)
        .collect();
    let (x_axis, y_axis) = (axes[0], axes[1]);
    assert!(
        (y.read().eval(0.0) as f32 - x_axis.p0.y).abs() <= 0.5,
        "bars float"
    );
    assert_eq!(x_axis.p0, y_axis.p1, "axis lines do not meet at the corner");
    let band = x.read().band_width();
    let floor = y.read().eval(0.0);
    for (k, sale) in sales().iter().enumerate() {
        let centre = x.read().eval(k as f64);
        let (left, right) = run(&image, centre as u32, (floor - 3.0) as u32, true);
        assert!(
            (f64::from(left) - (centre - band / 2.0)).abs() <= 1.0
                && (f64::from(right) - (centre + band / 2.0)).abs() <= 1.0,
            "{}: ink {left}..{right}, band {centre} ± {}",
            sale.month,
            band / 2.0
        );
        let (top, _) = run(&image, centre as u32, (floor - 3.0) as u32, false);
        // Off the tick that hangs under the band's centre.
        let (_, bottom) = run(&image, (centre - 4.0) as u32, (floor - 3.0) as u32, false);
        assert!(
            (f64::from(top) - y.read().eval(sale.total)).abs() <= 1.0,
            "{}: top {top}",
            sale.month
        );
        assert!(
            (f64::from(bottom) - floor).abs() <= 2.0,
            "{}: bottom {bottom}, axis {floor}",
            sale.month
        );
    }
}

/// AC5: horizontal bars, a band on y and the value on x with a derived
/// baseline.
#[test]
fn horizontal_bars_measure_to_their_band_and_value() {
    let cx = Context::new_blocking().unwrap();
    let rows: Vec<(&str, f64)> = vec![("A", 4.0), ("B", 10.0), ("C", 7.0)];
    let mut plot = Plot::new();
    let (x, y) = (plot.x(Linear::new().domain(0.0, 10.0)), plot.y(Band::new()));
    let baseline = x.clone();
    plot.add(Selection::<(&str, f64), Rect>::new(rows.clone()))
        .attr(
            Rect::Y,
            y.encode_key(|r: &(&str, f64)| r.0).domain(["A", "B", "C"]),
        )
        .attr(Rect::HEIGHT, y.band_width_px())
        .attr(Rect::X, x.encode(|r: &(&str, f64)| r.1))
        .attr(
            Rect::X_END,
            Derived::new(move || Px(baseline.read().eval(0.0) as f32)),
        );
    let image = render(&cx, &mut plot, 400, 300);
    let band = y.read().band_width().abs();
    let (x0, x1) = (x.read().eval(0.0), x.read().eval(10.0));
    for (k, (_, v)) in rows.iter().enumerate() {
        let centre = y.read().eval(k as f64);
        let inside = x.read().eval(*v) - 3.0;
        let (top, bottom) = run(&image, inside as u32, centre as u32, false);
        let (_, right) = run(&image, (x0 + 3.0) as u32, centre as u32, true);
        // Off the tick that sticks out of the band's centre row.
        let (left, _) = run(&image, (x0 + 3.0) as u32, (centre + 4.0) as u32, true);
        assert!(
            (f64::from(bottom - top) - band).abs() <= 1.0
                && (f64::from(top + bottom) / 2.0 - centre).abs() <= 1.0,
            "bar {k}: rows {top}..{bottom}, band {centre} ± {}",
            band / 2.0
        );
        assert!(
            (f64::from(left) - x0).abs() <= 1.0
                && (f64::from(right) - x.read().eval(*v)).abs() <= 1.0,
            "bar {k}: columns {left}..{right}, expected {x0}..{}",
            x.read().eval(*v)
        );
        assert!(x1 >= f64::from(right) - 1.0);
    }
}

/// GUP-494: a constant bar width on a numeric axis reports half itself as
/// overhang, so the end bars are whole inside the plot rect, not clipped
/// to half; and a `Derived` baseline of 0 joins the automatic domain, so
/// the bar from it is drawn.
#[test]
fn numeric_axis_bars_fit_and_a_derived_baseline_is_in_the_domain() {
    let cx = Context::new_blocking().unwrap();
    let rows: Vec<(f64, f64)> = (1..=5)
        .map(|i| (f64::from(i), 20.0 + 5.0 * f64::from(i)))
        .collect();
    let mut plot = Plot::new();
    let (x, y) = (plot.x(Linear::new()), plot.y(Linear::new()));
    let floor = y.clone();
    plot.add(Selection::<(f64, f64), Rect>::new(rows))
        .attr(Rect::X, x.encode(|r: &(f64, f64)| r.0))
        .attr(Rect::WIDTH, Px(30.0))
        .attr(Rect::Y, y.encode(|r: &(f64, f64)| r.1))
        .attr(
            Rect::Y_END,
            Derived::new(move || Px(floor.read().eval(0.0) as f32)),
        );
    let resolved = plot.resolve(&cx, WIDTH as f32, HEIGHT as f32).unwrap();
    let image = ImageTarget::new(&cx, WIDTH, HEIGHT)
        .unwrap()
        .render_blocking(&cx, &resolved.scene)
        .unwrap();
    let p = resolved.layout.plot;
    let (y0, _) = y.read().current_domain().unwrap();
    assert!(y0 <= 0.0, "the baseline 0 is in the y domain: {y0}");
    // Every bar, the end ones included, is 30 px wide, inside the plot rect.
    let row = (y.read().eval(0.0) - 4.0) as u32;
    for i in [1.0, 3.0, 5.0] {
        let c = x.read().eval(i) as u32;
        let (a, b) = run(&image, c, row, true);
        // The axis line may touch the first bar; a clipped one is 15 px.
        assert!((30..=32).contains(&(b - a)), "bar at {i}: ink {a}..{b}");
        assert!(
            a as f32 >= p.left() - 2.0 && b as f32 <= p.right(),
            "{a}..{b} in {p:?}"
        );
    }
}

/// GUP-540: bars of a constant 20 px width on a numeric x axis with an
/// explicit domain starting at the first bar's x: the first bar is whole,
/// and the bars still stand on the x axis (y has no overhang).
#[test]
fn the_first_bar_on_an_explicit_x_domain_is_whole_and_on_the_axis() {
    let cx = Context::new_blocking().unwrap();
    let rows: Vec<(f64, f64)> = (0..5)
        .map(|i| (f64::from(i), 40.0 + 10.0 * f64::from(i)))
        .collect();
    let mut plot = Plot::new();
    let x = plot.x(Linear::new().domain(0.0, 4.0));
    let y = plot.y(Linear::new().domain(0.0, 100.0));
    plot.legends(false);
    plot.add(Selection::<(f64, f64), Rect>::new(rows))
        .attr(Rect::X, x.encode(|r: &(f64, f64)| r.0))
        .attr(Rect::WIDTH, Px(20.0))
        .attr(Rect::Y, y.encode(|r: &(f64, f64)| r.1))
        .attr(Rect::Y_END, y.encode(|_: &(f64, f64)| 0.0))
        .attr(Rect::FILL, Color::hex(0x000000));
    let resolved = plot.resolve(&cx, WIDTH as f32, HEIGHT as f32).unwrap();
    let image = ImageTarget::new(&cx, WIDTH, HEIGHT)
        .unwrap()
        .render_blocking(&cx, &resolved.scene)
        .unwrap();
    let p = resolved.layout.plot;
    let base = y.read().eval(0.0);
    let row = (base - 3.0) as u32;
    for i in [0.0, 2.0, 4.0] {
        let c = x.read().eval(i) as u32;
        let (a, b) = run(&image, c, row, true);
        assert!((19..=22).contains(&(b - a)), "bar at {i}: ink {a}..{b}");
        assert!(
            a as f32 >= p.left() - 2.0 && (b as f32) <= p.right(),
            "bar at {i}: {a}..{b} in {p:?}"
        );
        // The row just above the axis line is bar colour across the bar.
        for xx in a + 1..b - 1 {
            assert!(image.get_pixel(xx, row).0[0] < 128, "bar {i} gap at {xx}");
        }
    }
    let first = x.read().eval(0.0) as f32;
    assert!(
        first - 10.0 >= p.left() - 0.5,
        "first bar's left edge {} vs plot {p:?}",
        first - 10.0
    );
}

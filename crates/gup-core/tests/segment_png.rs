// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! RFC-001 S6b (GUP-423): the `Segment` mark rendered headlessly. Ink is
//! measured on the pixels: with a red segment on white, a pixel's coverage
//! is `(red - green) / 255`, which grey guides and text do not disturb.

mod common;

use common::vr::{harness, layout_metadata, rgba8};
use gup_core::prelude::*;
use gup_core::{ImageTarget, OKABE_ITO};
use gup_visual_regression::RgbaImage;

const RED: Color = Color::hex(0xff0000);

type Row = (f64, &'static str, f64);

fn render(cx: &Context, plot: &mut Plot, width: u32, height: u32) -> image::RgbaImage {
    let resolved = plot.resolve(cx, width as f32, height as f32).unwrap();
    ImageTarget::new(cx, width, height)
        .unwrap()
        .render_blocking(cx, &resolved.scene)
        .unwrap()
}

/// Red ink at a pixel, 0..1.
fn ink(image: &image::RgbaImage, x: u32, y: u32) -> f64 {
    let p = image.get_pixel(x, y).0;
    (f64::from(p[0]) - f64::from(p[1])) / 255.0
}

/// Pixels spanned by the 0..1000 domains of `plot()` at `w` by `h`.
fn plot_size(cx: &Context, w: f32, h: f32) -> (f64, f64) {
    let (mut p, x, y) = plot();
    add_segments(&mut p, &x, &y, vec![[0.0; 4]], 1.0);
    p.resolve(cx, w, h).unwrap();
    (
        (x.read().eval(1000.0) - x.read().eval(0.0)).abs(),
        (y.read().eval(1000.0) - y.read().eval(0.0)).abs(),
    )
}

/// A plot with 0..1000 domains on both axes.
fn plot() -> (Plot, ScaleRef<Linear>, ScaleRef<Linear>) {
    let mut p = Plot::new();
    let x = p.x(Linear::new().domain(0.0, 1000.0));
    let y = p.y(Linear::new().domain(0.0, 1000.0));
    (p, x, y)
}

fn add_segments(
    p: &mut Plot,
    x: &ScaleRef<Linear>,
    y: &ScaleRef<Linear>,
    rows: Vec<[f64; 4]>,
    width: f32,
) {
    p.add(Selection::<[f64; 4], Segment>::new(rows))
        .attr(Segment::X, x.encode(|r: &[f64; 4]| r[0]))
        .attr(Segment::Y, y.encode(|r: &[f64; 4]| r[1]))
        .attr(Segment::X_END, x.encode(|r: &[f64; 4]| r[2]))
        .attr(Segment::Y_END, y.encode(|r: &[f64; 4]| r[3]))
        .attr(Segment::WIDTH, Px(width))
        .attr(Segment::COLOR, RED);
}

/// AC2: a 100 px horizontal segment, 4 px wide, fills its box (the segment
/// extended 2 px at each end and 2 px each side) and nothing outside it.
#[test]
fn a_horizontal_segment_fills_its_box() {
    let cx = Context::new_blocking().unwrap();
    let (w, h) = (500u32, 400u32);
    let (pw, _) = plot_size(&cx, w as f32, h as f32);
    let (a, b) = (300.0, 300.0 + 100.0 * 1000.0 / pw);
    let (mut p, x, y) = plot();
    add_segments(&mut p, &x, &y, vec![[a, 500.0, b, 500.0]], 4.0);
    let image = render(&cx, &mut p, w, h);
    let (x0, x1) = (x.read().eval(a), x.read().eval(b));
    let yc = y.read().eval(500.0);
    assert!(((x1 - x0) - 100.0).abs() < 0.1, "{}", x1 - x0);
    let (left, right, top, bottom) = (x0 - 2.0, x1 + 2.0, yc - 2.0, yc + 2.0);
    let mut inside = 0;
    for py in 0..h {
        for px in 0..w {
            let (cx_, cy) = (f64::from(px) + 0.5, f64::from(py) + 0.5);
            let depth = (cx_ - left).min(right - cx_).min(cy - top).min(bottom - cy);
            let c = ink(&image, px, py);
            if depth >= 0.5 {
                inside += 1;
                assert!(c > 1.0 - 2.0 / 255.0, "({px}, {py}) inside: {c}");
            } else if depth <= -1.5 {
                assert!(c.abs() < 2.0 / 255.0, "({px}, {py}) outside: {c}");
            }
        }
    }
    assert!(inside > 300, "{inside}");
}

/// AC2: a 45 degree segment of length 100 px and width 3 px holds
/// (100 + 3) * 3 of ink: the caps are included.
#[test]
fn a_diagonal_segment_holds_its_area_of_ink() {
    let cx = Context::new_blocking().unwrap();
    let (w, h) = (500u32, 400u32);
    let (pw, ph) = plot_size(&cx, w as f32, h as f32);
    let d = 100.0 / 2f64.sqrt();
    let (a, b) = (300.0, 300.0 + d * 1000.0 / pw);
    let (c, e) = (300.0, 300.0 + d * 1000.0 / ph);
    let (mut p, x, y) = plot();
    add_segments(&mut p, &x, &y, vec![[a, c, b, e]], 3.0);
    let image = render(&cx, &mut p, w, h);
    let (dx, dy) = (
        x.read().eval(b) - x.read().eval(a),
        y.read().eval(e) - y.read().eval(c),
    );
    assert!((dx.hypot(dy) - 100.0).abs() < 0.2, "{dx} {dy}");
    let total: f64 = (0..h)
        .flat_map(|py| (0..w).map(move |px| (px, py)))
        .map(|(px, py)| ink(&image, px, py))
        .sum();
    assert!((total - 309.0).abs() <= 6.0, "ink {total}");
}

/// Coverage of `colour` at a pixel: the projection of its distance from
/// white onto the colour's own.
fn ink_of(image: &image::RgbaImage, x: u32, y: u32, colour: Color) -> f64 {
    let p = image.get_pixel(x, y).0;
    let c = colour.to_rgba8();
    let (mut num, mut den) = (0.0, 0.0);
    for k in 0..3 {
        let t = 255.0 - f64::from(c[k]);
        num += (255.0 - f64::from(p[k])) * t;
        den += t * t;
    }
    num / den
}

/// AC4: width through a Linear range and colour through a Categorical.
#[test]
fn width_and_colour_are_encoded_per_row() {
    let cx = Context::new_blocking().unwrap();
    let (w, h) = (500u32, 400u32);
    let (mut p, x, y) = plot();
    let width = ScaleRef::new(Linear::new().domain(0.0, 10.0).range(Px(1.0), Px(6.0)));
    let colour = ScaleRef::new(Categorical::okabe_ito());
    let rows: Vec<Row> = vec![
        (0.0, "a", 200.0),
        (5.0, "b", 400.0),
        (7.5, "c", 600.0),
        (10.0, "d", 800.0),
    ];
    p.add(Selection::<Row, Segment>::new(rows.clone()))
        .attr(Segment::X, x.encode(|_: &Row| 200.0))
        .attr(Segment::X_END, x.encode(|_: &Row| 700.0))
        .attr(Segment::Y, y.encode(|r: &Row| r.2))
        .attr(Segment::Y_END, y.encode(|r: &Row| r.2))
        .attr(Segment::WIDTH, width.encode(|r: &Row| r.0))
        .attr(Segment::COLOR, colour.encode_key(|r: &Row| r.1));
    let image = render(&cx, &mut p, w, h);
    let xm = x.read().eval(450.0) as u32;
    for (i, (v, _, yv)) in rows.iter().enumerate() {
        let yc = y.read().eval(*yv);
        let want = 1.0 + 5.0 * v / 10.0;
        let lo = (yc - 10.0) as u32;
        let sum: f64 = (lo..lo + 20)
            .map(|py| ink_of(&image, xm, py, OKABE_ITO[i]))
            .sum();
        assert!((sum - want).abs() <= 0.5, "row {i}: {sum} vs {want}");
        // The centre pixel is the category's colour at its own coverage.
        let px = image.get_pixel(xm, yc as u32).0;
        let col = OKABE_ITO[i].to_rgba8();
        let c = ink_of(&image, xm, yc as u32, OKABE_ITO[i]);
        for k in 0..3 {
            let expect = 255.0 * (1.0 - c) + f64::from(col[k]) * c;
            assert!(
                (f64::from(px[k]) - expect).abs() <= 2.0,
                "row {i}: {px:?} vs {col:?} at coverage {c}"
            );
        }
    }
}

/// AC5: a row with a NaN end is not drawn; the others are.
#[test]
fn a_nan_end_hides_the_row() {
    let cx = Context::new_blocking().unwrap();
    let (w, h) = (500u32, 400u32);
    let (mut p, x, y) = plot();
    add_segments(
        &mut p,
        &x,
        &y,
        vec![
            [200.0, 300.0, 400.0, 300.0],
            [200.0, 600.0, f64::NAN, 600.0],
            [200.0, 800.0, 400.0, 800.0],
        ],
        3.0,
    );
    let image = render(&cx, &mut p, w, h);
    let region = |x0: f64, y0: f64, r: f64| -> f64 {
        let (cx_, cy) = (x.read().eval(x0), y.read().eval(y0));
        let mut s = 0.0;
        for py in (cy - r) as u32..=(cy + r) as u32 {
            for px in (cx_ - r) as u32..=(cx_ + r) as u32 {
                s += ink(&image, px, py);
            }
        }
        s
    };
    assert!(region(200.0, 300.0, 3.0) > 5.0);
    assert!(region(200.0, 800.0, 3.0) > 5.0);
    assert!(region(200.0, 600.0, 10.0) < 0.01);
}

/// AC5: `x` data in 0..10 and `x_end` data in 5..30 give a domain covering 0..30.
#[test]
fn both_ends_of_a_layer_union_into_the_domain() {
    let cx = Context::new_blocking().unwrap();
    let mut p = Plot::new();
    let x = p.x(Linear::new());
    let y = p.y(Linear::new());
    p.add(Selection::<(f64, f64), Segment>::new(vec![
        (0.0, 5.0),
        (10.0, 30.0),
    ]))
    .attr(Segment::X, x.encode(|r: &(f64, f64)| r.0))
    .attr(Segment::X_END, x.encode(|r: &(f64, f64)| r.1))
    .attr(Segment::Y, y.encode(|r: &(f64, f64)| r.0))
    .attr(Segment::Y_END, y.encode(|r: &(f64, f64)| r.1));
    p.resolve(&cx, 400.0, 300.0).unwrap();
    let (d0, d1) = x.read().current_domain().unwrap();
    assert!(d0 <= 0.0 && d1 >= 30.0, "{d0}..{d1}");
}

/// AC3: twenty vertical error bars and a fan of diagonal segments of four
/// widths.
#[test]
fn error_bars_and_a_fan() {
    let cx = Context::new_blocking().unwrap();
    let (w, h) = (720u32, 450u32);
    let mut p = Plot::new();
    let x = p.x(Linear::new().domain(0.0, 100.0));
    let y = p.y(Linear::new().domain(0.0, 100.0));
    let bars: Vec<[f64; 4]> = (0..20)
        .map(|i| {
            let t = f64::from(i);
            let mid = 60.0 + 12.0 * (t * 0.5).sin() + t * 0.5;
            let span = 4.0 + 3.0 * (t * 1.7).cos().abs();
            [2.0 + t * 2.4, mid - span, 2.0 + t * 2.4, mid + span]
        })
        .collect();
    let widths = [0.5, 1.0, 2.0, 4.0];
    let fan: Vec<([f64; 4], f64)> = (0..8)
        .map(|i| {
            let a = 0.15 + f64::from(i) * 0.17;
            (
                [60.0, 8.0, 60.0 + 36.0 * a.cos(), 8.0 + 36.0 * a.sin()],
                widths[i as usize % 4],
            )
        })
        .collect();
    let fan_width = ScaleRef::new(Linear::new().domain(0.0, 4.0).range(Px(0.0), Px(4.0)));
    type Fan = ([f64; 4], f64);
    p.title("Error bars and a fan of segments");
    p.add(Selection::<[f64; 4], Segment>::new(bars))
        .attr(Segment::X, x.encode(|r: &[f64; 4]| r[0]))
        .attr(Segment::Y, y.encode(|r: &[f64; 4]| r[1]))
        .attr(Segment::X_END, x.encode(|r: &[f64; 4]| r[2]))
        .attr(Segment::Y_END, y.encode(|r: &[f64; 4]| r[3]))
        .attr(Segment::WIDTH, Px(1.5));
    p.add(Selection::<Fan, Segment>::new(fan))
        .attr(Segment::X, x.encode(|r: &Fan| r.0[0]))
        .attr(Segment::Y, y.encode(|r: &Fan| r.0[1]))
        .attr(Segment::X_END, x.encode(|r: &Fan| r.0[2]))
        .attr(Segment::Y_END, y.encode(|r: &Fan| r.0[3]))
        .attr(Segment::WIDTH, fan_width.encode(|r: &Fan| r.1))
        .attr(Segment::COLOR, Color::hex(0xd55e00));
    let resolved = p.resolve(&cx, w as f32, h as f32).unwrap();
    let image = ImageTarget::new(&cx, w, h)
        .unwrap()
        .render_blocking(&cx, &resolved.scene)
        .unwrap();
    let vr = RgbaImage::new(image.width(), image.height(), image.as_raw().clone()).unwrap();
    let meta = layout_metadata(&resolved.layout)
        .with_expected_color("error bar", rgba8(Segment::DEFAULT_COLOR));
    harness()
        .run("gup_core/segment_mark", Ok((vr, meta)))
        .assert_ok();
}

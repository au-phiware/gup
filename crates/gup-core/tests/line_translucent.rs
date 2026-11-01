// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GUP-439: a translucent `Line` has one opacity along its whole length.
//! Its joins, step corners, monotone piece joints and self-crossings are
//! blended once, through an offscreen target, and its SVG is a group with
//! the layer's opacity. Ink is measured as in `line_png.rs`: with a red
//! line on white, a pixel's coverage is `(red - green) / 255`.

mod common;

use common::svg_marks;
use common::vr::{harness, layout_metadata, rgba8};
use gup_core::prelude::*;
use gup_core::{ImageTarget, SvgTarget, VectorTarget};
use gup_visual_regression::RgbaImage;

type Row = (f64, f64);

const RED: Color = Color::hex(0xff0000);
const HALF_RED: Color = Color { a: 0.5, ..RED };
const W: u32 = 500;
const H: u32 = 400;
/// AC1's tolerance.
const TOL: f64 = 2.0 / 255.0;

/// Red ink at a pixel, 0..1.
fn ink(image: &image::RgbaImage, x: u32, y: u32) -> f64 {
    let p = image.get_pixel(x, y).0;
    (f64::from(p[0]) - f64::from(p[1])) / 255.0
}

/// A plot of one line through `rows` over fixed 0..500 × 0..400 domains,
/// `width` px wide, in `color` and `curve`, and a map from data to pixels.
fn plot(
    rows: &[Row],
    width: f32,
    color: Color,
    curve: Curve,
) -> (Plot, impl Fn((f64, f64)) -> (f64, f64)) {
    let mut p = Plot::new();
    p.legends(false);
    let x = p.x(Linear::new().domain(0.0, 500.0));
    let y = p.y(Linear::new().domain(0.0, 400.0));
    p.add(Selection::<Row, Line>::new(rows.to_vec()))
        .x_with(x.encode(|r: &Row| r.0))
        .y_with(y.encode(|r: &Row| r.1))
        .width(Px(width))
        .color(color)
        .curve(curve);
    (p, move |(a, b)| (x.read().eval(a), y.read().eval(b)))
}

fn render(cx: &Context, p: &mut Plot) -> image::RgbaImage {
    let resolved = p.resolve(cx, W as f32, H as f32).unwrap();
    ImageTarget::new(cx, W, H)
        .unwrap()
        .render_blocking(cx, &resolved.scene)
        .unwrap()
}

/// How the translucent render of a line compares with its opaque render.
#[derive(Debug)]
struct Evenness {
    /// The most ink on any pixel.
    most: f64,
    /// Pixels the opaque line covers fully, where the translucent one must
    /// have exactly its alpha.
    full: usize,
    /// The largest difference from the alpha on those pixels, and where.
    worst: (f64, (u32, u32)),
    /// Of `full`, the pixels within 5 px of a point of interest (a join,
    /// a crossing).
    near: usize,
}

/// Draw `rows` opaque and at alpha 0.5 and compare: the translucent line
/// must have half the ink wherever the opaque one is full, and never more
/// than half anywhere.
fn evenness(rows: &[Row], curve: Curve, width: f32, points: &[(f64, f64)]) -> Evenness {
    let cx = Context::new_blocking().unwrap();
    let opaque = render(&cx, &mut plot(rows, width, RED, curve).0);
    let (mut half_plot, to_px) = plot(rows, width, HALF_RED, curve);
    let half = render(&cx, &mut half_plot);
    let points: Vec<_> = points.iter().map(|&p| to_px(p)).collect();
    let mut e = Evenness {
        most: 0.0,
        full: 0,
        worst: (0.0, (0, 0)),
        near: 0,
    };
    for py in 0..H {
        for px in 0..W {
            let k = ink(&half, px, py);
            e.most = e.most.max(k);
            if ink(&opaque, px, py) < 1.0 - 0.5 / 255.0 {
                continue;
            }
            e.full += 1;
            let off = (k - 0.5).abs();
            if off > e.worst.0 {
                e.worst = (off, (px, py));
            }
            let c = (f64::from(px) + 0.5, f64::from(py) + 0.5);
            if points.iter().any(|p| (p.0 - c.0).hypot(p.1 - c.1) <= 5.0) {
                e.near += 1;
            }
        }
    }
    e
}

fn assert_even(case: &str, e: &Evenness) {
    assert!(e.full > 500, "{case}: only {} full pixels", e.full);
    assert!(
        e.near > 10,
        "{case}: only {} full pixels near joins",
        e.near
    );
    assert!(
        e.worst.0 <= TOL,
        "{case}: ink {:.4} from 0.5 at {:?} (a join blended twice?) {e:?}",
        e.worst.0,
        e.worst.1
    );
    assert!(
        e.most <= 0.5 + TOL,
        "{case}: a pixel has ink {:.4}, more than the arms' 0.5 {e:?}",
        e.most
    );
}

/// A "V" with its apex at data (250, 200), `angle` degrees between its
/// arms, each 150 data units long; and its apex.
fn vee(angle: f64) -> (Vec<Row>, (f64, f64)) {
    let half = angle.to_radians() / 2.0;
    let apex = (250.0, 200.0);
    let (dx, dy) = (150.0 * half.sin(), 150.0 * half.cos());
    (
        vec![(apex.0 - dx, apex.1 + dy), apex, (apex.0 + dx, apex.1 + dy)],
        apex,
    )
}

/// AC1: joins of 30°, 90° and 170° turns have the arms' ink.
#[test]
fn translucent_joins_have_the_arms_ink() {
    for angle in [30.0, 90.0, 170.0] {
        let (rows, apex) = vee(angle);
        let e = evenness(&rows, Curve::Linear, 6.0, &[apex]);
        assert_even(&format!("{angle}° join"), &e);
    }
}

/// AC1: where a line crosses itself, and where a segment runs back over
/// the one before it, the overlap has the arms' ink.
#[test]
fn a_translucent_self_crossing_has_the_arms_ink() {
    // A bow tie: the first and third segments cross at (250, 200).
    let bow = [
        (100.0, 100.0),
        (400.0, 300.0),
        (400.0, 100.0),
        (100.0, 300.0),
    ];
    let e = evenness(&bow, Curve::Linear, 6.0, &[(250.0, 200.0)]);
    assert_even("self-crossing", &e);
    // Back and forth along one line: every segment overlaps another.
    let back = [
        (100.0, 200.0),
        (400.0, 200.0),
        (200.0, 200.0),
        (300.0, 200.0),
    ];
    let e = evenness(&back, Curve::Linear, 6.0, &[(200.0, 200.0), (300.0, 200.0)]);
    assert_even("doubling back", &e);
}

/// AC1 (Decisions): a `StepAfter` line's corners and a `MonotoneX` line's
/// piece joints and rows have the arms' ink.
#[test]
fn translucent_curves_have_one_opacity_at_corners_and_piece_joints() {
    let zig = [
        (50.0, 100.0),
        (200.0, 300.0),
        (350.0, 150.0),
        (450.0, 250.0),
    ];
    // StepAfter's corners: under each next row's x, at the row's y.
    let corners: Vec<_> = zig.windows(2).map(|w| (w[1].0, w[0].1)).collect();
    let e = evenness(&zig, Curve::StepAfter, 6.0, &corners);
    assert_even("StepAfter corners", &e);
    // Wide segments, so each is 16 pieces: 15 piece joints per segment.
    let e = evenness(&zig, Curve::MonotoneX, 6.0, &zig[1..3]);
    assert_even("MonotoneX joints", &e);
}

/// A scene that needs MSAA (an area) composites its translucent line at
/// four samples with the same result.
#[test]
fn a_translucent_line_in_a_multisampled_scene_is_even() {
    let cx = Context::new_blocking().unwrap();
    let (rows, _) = vee(60.0);
    let mut p = Plot::new();
    p.legends(false);
    let x = p.x(Linear::new().domain(0.0, 500.0));
    let y = p.y(Linear::new().domain(0.0, 400.0));
    p.add(Selection::<Row, Area>::new(vec![
        (0.0, 40.0),
        (500.0, 80.0),
    ]))
    .attr(Area::X, x.encode(|r: &Row| r.0))
    .attr(Area::Y, y.encode(|r: &Row| r.1))
    .attr(Area::Y_END, y.encode(|_: &Row| 0.0));
    p.add(Selection::<Row, Line>::new(rows))
        .x_with(x.encode(|r: &Row| r.0))
        .y_with(y.encode(|r: &Row| r.1))
        .width(Px(6.0))
        .color(HALF_RED);
    let resolved = p.resolve(&cx, W as f32, H as f32).unwrap();
    assert_eq!(resolved.scene.min_samples(), 4);
    let image = ImageTarget::new(&cx, W, H)
        .unwrap()
        .render_blocking(&cx, &resolved.scene)
        .unwrap();
    let most = (0..H)
        .flat_map(|py| (0..W).map(move |px| (px, py)))
        .map(|(px, py)| ink(&image, px, py))
        .fold(0.0, f64::max);
    assert!((most - 0.5).abs() <= TOL, "most ink {most}");
}

/// The SVG of a translucent line is a group with the line's opacity
/// around opaque strokes, so a crossing between two runs (split by a
/// null) is blended once, as on the GPU.
#[test]
fn a_translucent_line_is_one_translucent_svg_group() {
    let cx = Context::new_blocking().unwrap();
    let rows = [
        (100.0, 100.0),
        (400.0, 300.0),
        (400.0, f64::NAN),
        (400.0, 100.0),
        (100.0, 300.0),
    ];
    let (mut p, to_px) = plot(&rows, 8.0, HALF_RED, Curve::Linear);
    let resolved = p.resolve(&cx, W as f32, H as f32).unwrap();
    let image = ImageTarget::new(&cx, W, H)
        .unwrap()
        .render_blocking(&cx, &resolved.scene)
        .unwrap();
    let mut target = SvgTarget::new();
    target.render(&resolved.scene).unwrap();
    let svg = target.svg();
    assert!(svg.contains(r#"<g opacity="0.5">"#), "{svg}");
    assert_eq!(svg.matches("<polyline ").count(), 2, "{svg}");
    assert!(!svg.contains("stroke-opacity"), "{svg}");
    // The GPU's crossing is even, and the SVG's matches it.
    let (px, py) = to_px((250.0, 200.0));
    let (cx_, cy) = (px as u32, py as u32);
    let k = ink(&image, cx_, cy);
    assert!((k - 0.5).abs() <= TOL, "GPU crossing ink {k}");
    let vr = RgbaImage::new(image.width(), image.height(), image.as_raw().clone()).unwrap();
    let meta = layout_metadata(&resolved.layout);
    svg_marks::assert_matches(
        "gup_core/line_translucent_crossing",
        &resolved.scene,
        &vr,
        &meta,
    );
    let raster = svg_marks::rasterise(svg);
    let p = raster.pixel(cx_, cy);
    let k = (f64::from(p.r) - f64::from(p.g)) / 255.0;
    assert!((k - 0.5).abs() <= 0.02, "SVG crossing ink {k}");
}

/// Twelve overplotted translucent series, one layer each: every series
/// is even along its length, and series blend over one another, so
/// density shows where they bunch.
#[test]
fn line_translucent_golden() {
    let cx = Context::new_blocking().unwrap();
    let (w, h) = (720u32, 400u32);
    let mut p = Plot::new();
    p.legends(false);
    p.title("Twelve translucent series");
    let x = p.x(Linear::new());
    let y = p.y(Linear::new());
    let color = Color {
        a: 0.35,
        ..Line::DEFAULT_COLOR
    };
    for s in 0..12 {
        let k = f64::from(s);
        let rows: Vec<Row> = (0..60)
            .map(|i| {
                let t = f64::from(i);
                let v = 50.0
                    + 20.0 * (t / (6.0 + k * 0.7) + k).sin()
                    + 8.0 * (t / 3.1 + 2.0 * k).cos()
                    + 0.3 * k * (t / 9.0).sin();
                (t, v)
            })
            .collect();
        p.add(Selection::<Row, Line>::new(rows))
            .x_with(x.encode(|r: &Row| r.0))
            .y_with(y.encode(|r: &Row| r.1))
            .width(Px(3.0))
            .color(color);
    }
    let resolved = p.resolve(&cx, w as f32, h as f32).unwrap();
    let image = ImageTarget::new(&cx, w, h)
        .unwrap()
        .render_blocking(&cx, &resolved.scene)
        .unwrap();
    let vr = RgbaImage::new(image.width(), image.height(), image.as_raw().clone()).unwrap();
    // One series alone over the white plot, as the eye sees it.
    let over_white = |c: f32| c * color.a + (1.0 - color.a);
    let seen = Color {
        r: over_white(color.r),
        g: over_white(color.g),
        b: over_white(color.b),
        a: 1.0,
    };
    let meta = layout_metadata(&resolved.layout).with_expected_color("line", rgba8(seen));
    harness()
        .run("gup_core/line_translucent", Ok((vr.clone(), meta.clone())))
        .assert_ok();
    let svg = svg_marks::assert_matches("gup_core/line_translucent", &resolved.scene, &vr, &meta);
    assert_eq!(svg.matches(r#"<g opacity="0.35">"#).count(), 12);
}

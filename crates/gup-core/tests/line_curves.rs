// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GUP-550: a line's `Curve`. Straight lines keep the glue they had before
//! curves existed; step curves draw their corners exactly, in pixel space,
//! and picks and SVG output follow the same path. Ink is measured as in
//! `line_png.rs`: with a red line on white, a pixel's coverage is
//! `(red - green) / 255`.

mod common;

use common::svg_marks;
use common::vr::{harness, layout_metadata, rgba8};
use gup_core::geom::Point as Pt;
use gup_core::prelude::*;
use gup_core::{ImageTarget, OKABE_ITO, RowId, SvgTarget, VectorTarget};
use gup_visual_regression::RgbaImage;
use std::path::Path;

type Row = (f64, f64);

const RED: Color = Color::hex(0xff0000);
const W: u32 = 500;
const H: u32 = 400;
const STEPS: [Curve; 3] = [Curve::StepAfter, Curve::StepBefore, Curve::Step];

/// Compare `actual` with the fixture `name`, writing it when it is missing
/// or under `GUP_BLESS`.
fn check_fixture(name: &str, actual: &str) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    if std::env::var_os("GUP_BLESS").is_some() || !path.exists() {
        std::fs::write(&path, actual).unwrap();
    }
    let expected = std::fs::read_to_string(&path).unwrap();
    assert_eq!(
        actual,
        expected,
        "{} changed; re-run with GUP_BLESS=1 and review the diff",
        path.display()
    );
}

/// A line over `rows` with column x and y, resolved; its glue signature
/// and source.
fn line_glue(rows: Vec<Row>, curve: Option<Curve>) -> (String, String) {
    let cx = Context::new_blocking().unwrap();
    let mut p = Plot::new();
    let x = p.x(Linear::new());
    let y = p.y(Linear::new());
    let line = p.add(Selection::<Row, Line>::new(rows));
    line.x_with(x.encode(|r: &Row| r.0))
        .y_with(y.encode(|r: &Row| r.1));
    if let Some(c) = curve {
        line.curve(c);
    }
    p.resolve(&cx, 320.0, 200.0).unwrap();
    p.glue_sources().remove(0)
}

fn four() -> Vec<Row> {
    vec![(0.0, 1.0), (1.0, 3.0), (2.0, 2.0), (3.0, 4.0)]
}

/// AC2: a straight line's glue is the glue built before curves existed
/// (the fixtures were written at the story's first commit), with or
/// without nulls, and setting `Curve::Linear` changes nothing. Each step
/// curve has its own signature and calls `vertex_variant`.
#[test]
fn a_linear_line_keeps_its_glue() {
    let mut nulls = four();
    nulls[1].1 = f64::NAN;
    for (curve, rows, name) in [
        (None, four(), "line_glue.wgsl"),
        (Some(Curve::Linear), four(), "line_glue.wgsl"),
        (None, nulls.clone(), "line_glue_nulls.wgsl"),
        (Some(Curve::Linear), nulls, "line_glue_nulls.wgsl"),
    ] {
        let (signature, source) = line_glue(rows, curve);
        assert!(
            signature.starts_with("Line {x: f32rel→gup::scale::linear::map_rel, ")
                && !signature.contains("curve"),
            "{signature}"
        );
        check_fixture(name, &source);
    }
    let mut signatures = Vec::new();
    for (curve, value) in [
        (Curve::Step, 1),
        (Curve::StepBefore, 2),
        (Curve::StepAfter, 3),
    ] {
        let (signature, source) = line_glue(four(), Some(curve));
        assert!(signature.contains("} curve step"), "{signature}");
        let call = format!(
            "line::vertex_variant(m, vertex_index, chunk.row_base + instance_index, u_view, \
             {value}u)"
        );
        assert!(source.contains(&call), "{source}");
        signatures.push(signature);
    }
    signatures.dedup();
    assert_eq!(signatures.len(), 3, "{signatures:?}");
}

/// A rendered line and its rows' positions in pixels.
struct Drawn {
    image: image::RgbaImage,
    points: Vec<Option<(f64, f64)>>,
}

/// Options for [`draw`].
#[derive(Clone, Copy)]
struct Opts {
    curve: Curve,
    width: f32,
    chunk: Option<u32>,
    log_y: bool,
}

impl Opts {
    fn new(curve: Curve) -> Self {
        Self {
            curve,
            width: 3.0,
            chunk: None,
            log_y: false,
        }
    }
}

/// Draw `rows` as one red line over x domain `xd` and y domain `yd`.
fn draw(rows: &[Row], xd: (f64, f64), yd: (f64, f64), o: Opts) -> Drawn {
    let cx = Context::new_blocking().unwrap();
    let mut p = Plot::new();
    p.legends(false);
    let x = p.x(Linear::new().domain(xd.0, xd.1));
    let mut sel = Selection::<Row, Line>::new(rows.to_vec());
    let eval: Box<dyn Fn(f64) -> f64> = if o.log_y {
        let y = p.y(Log::new().domain(yd.0, yd.1));
        sel.y_with(y.encode(|r: &Row| r.1));
        Box::new(move |v| y.read().eval(v))
    } else {
        let y = p.y(Linear::new().domain(yd.0, yd.1));
        sel.y_with(y.encode(|r: &Row| r.1));
        Box::new(move |v| y.read().eval(v))
    };
    sel.x_with(x.encode(|r: &Row| r.0))
        .width(Px(o.width))
        .color(RED)
        .curve(o.curve);
    if let Some(n) = o.chunk {
        sel.max_chunk_rows(n);
    }
    p.add(sel);
    let resolved = p.resolve(&cx, W as f32, H as f32).unwrap();
    let image = ImageTarget::new(&cx, W, H)
        .unwrap()
        .render_blocking(&cx, &resolved.scene)
        .unwrap();
    let points = rows
        .iter()
        .map(|&(a, b)| (a.is_finite() && b.is_finite()).then(|| (x.read().eval(a), eval(b))))
        .collect();
    Drawn { image, points }
}

/// Red ink at a pixel, 0..1.
fn ink(image: &image::RgbaImage, x: u32, y: u32) -> f64 {
    let p = image.get_pixel(x, y).0;
    (f64::from(p[0]) - f64::from(p[1])) / 255.0
}

/// Red ink at the pixel holding `p`.
fn ink_at(d: &Drawn, p: (f64, f64)) -> f64 {
    ink(&d.image, p.0.floor() as u32, p.1.floor() as u32)
}

/// The path of the step from `a` to `b`, written out independently of
/// the crate's.
fn step_path(curve: Curve, a: (f64, f64), b: (f64, f64)) -> Vec<(f64, f64)> {
    let m = (a.0 + b.0) / 2.0;
    match curve {
        Curve::StepAfter => vec![a, (b.0, a.1), b],
        Curve::StepBefore => vec![a, (a.0, b.1), b],
        Curve::Step => vec![a, (m, a.1), (m, b.1), b],
        _ => vec![a, b],
    }
}

/// The pieces of every drawn segment's path (both rows non-null).
fn pieces(d: &Drawn, curve: Curve) -> Vec<((f64, f64), (f64, f64))> {
    d.points
        .windows(2)
        .filter_map(|w| Some((w[0]?, w[1]?)))
        .flat_map(|(a, b)| {
            let path = step_path(curve, a, b);
            path.windows(2).map(|s| (s[0], s[1])).collect::<Vec<_>>()
        })
        .collect()
}

/// The least coverage among the pixels under every 0.5 px sample of the
/// drawn path, and where.
fn path_min(d: &Drawn, curve: Curve) -> (f64, (f64, f64)) {
    let mut worst = (f64::INFINITY, (0.0, 0.0));
    for (a, b) in pieces(d, curve) {
        let len = (b.0 - a.0).hypot(b.1 - a.1);
        let steps = (len / 0.5).ceil().max(1.0) as usize;
        for s in 0..=steps {
            let t = s as f64 / steps as f64;
            let p = (a.0 + t * (b.0 - a.0), a.1 + t * (b.1 - a.1));
            let c = ink_at(d, p);
            if c < worst.0 {
                worst = (c, p);
            }
        }
    }
    worst
}

/// Distance from `p` to the segment `a`–`b`.
fn dist(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dy * dy;
    let t = if len2 > 0.0 {
        (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (p.0 - a.0 - t * dx).hypot(p.1 - a.1 - t * dy)
}

/// Four rows with distinct y, far apart in pixels.
fn steps_rows() -> Vec<Row> {
    vec![(50.0, 80.0), (180.0, 300.0), (300.0, 160.0), (450.0, 340.0)]
}

/// AC3: each step curve inks its whole path (the levels at the rows, the
/// verticals at its turn, no gap at a corner), and not the straight
/// diagonal.
#[test]
fn steps_ink_their_path_and_not_the_diagonal() {
    let clear = 2.0 / 255.0;
    for curve in STEPS {
        let d = draw(&steps_rows(), (0.0, 500.0), (0.0, 400.0), Opts::new(curve));
        let (least, at) = path_min(&d, curve);
        assert!(least >= 0.9, "{curve:?}: coverage {least} at {at:?}");
        for (i, w) in d.points.windows(2).enumerate() {
            let (a, b) = (w[0].unwrap(), w[1].unwrap());
            let mid = ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
            let quarter = (a.0 + (b.0 - a.0) / 4.0, a.1 + (b.1 - a.1) / 4.0);
            match curve {
                // The diagonal's midpoint and the other step's corner are
                // off the path (the corner only on the outer segments,
                // where no neighbour's vertical runs through it).
                Curve::StepAfter | Curve::StepBefore => {
                    let other = if curve == Curve::StepAfter {
                        (i == 0).then_some((a.0, b.1))
                    } else {
                        (i + 2 == d.points.len()).then_some((b.0, a.1))
                    };
                    for p in [Some(mid), Some(quarter), other].into_iter().flatten() {
                        let k = ink_at(&d, p);
                        assert!(k < clear, "{curve:?}: ink {k} at {p:?}");
                    }
                }
                // Step turns at the midpoint x: the diagonal's midpoint is
                // on its vertical, the quarter point and both one-step
                // corners are not.
                _ => {
                    for p in [quarter, (b.0, a.1), (a.0, b.1)] {
                        let k = ink_at(&d, p);
                        assert!(k < clear, "Step: ink {k} at {p:?}");
                    }
                    let k = ink_at(&d, mid);
                    assert!(k >= 0.9, "Step: ink {k} at the midpoint");
                }
            }
        }
        // Past a corner, beyond the round cap (1.5 px) and the ramp, there
        // is no ink.
        let (a, b) = (d.points[0].unwrap(), d.points[1].unwrap());
        let r = 1.5 + 1.5;
        let outside = match curve {
            Curve::StepAfter => (b.0 + r, a.1),
            Curve::StepBefore => (a.0, b.1 + r * (b.1 - a.1).signum()),
            _ => ((a.0 + b.0) / 2.0 + r, a.1),
        };
        let k = ink_at(&d, outside);
        assert!(
            k < clear,
            "{curve:?}: ink {k} past the corner at {outside:?}"
        );
    }
}

/// The ink-weighted mean y of the pixels in column `x` within `reach` of
/// `y`.
fn ink_row(d: &Drawn, x: f64, y: f64, reach: f64) -> f64 {
    let px = x.floor() as u32;
    let (mut sum, mut weight) = (0.0, 0.0);
    for py in (y - reach).floor() as u32..=(y + reach).ceil() as u32 {
        let k = ink(&d.image, px, py).max(0.0);
        sum += k * (f64::from(py) + 0.5);
        weight += k;
    }
    assert!(weight > 1.0, "no ink in column {px} near y {y}");
    sum / weight
}

/// AC3: under a log y scale a step's levels stay level: the same pixel
/// row at both ends of each horizontal piece, within 0.5 px.
#[test]
fn steps_stay_level_on_a_log_scale() {
    let rows = vec![(50.0, 2.0), (180.0, 300.0), (300.0, 15.0), (450.0, 900.0)];
    for curve in STEPS {
        let o = Opts {
            log_y: true,
            ..Opts::new(curve)
        };
        let d = draw(&rows, (0.0, 500.0), (1.0, 1000.0), o);
        let (least, at) = path_min(&d, curve);
        assert!(least >= 0.9, "{curve:?}: coverage {least} at {at:?}");
        let mut levels = 0;
        for (a, b) in pieces(&d, curve) {
            if a.1 != b.1 || (b.0 - a.0).abs() < 20.0 {
                continue;
            }
            let (x0, x1) = (a.0.min(b.0) + 6.0, a.0.max(b.0) - 6.0);
            let (r0, r1) = (ink_row(&d, x0, a.1, 4.0), ink_row(&d, x1, a.1, 4.0));
            assert!(
                (r0 - r1).abs() <= 0.5 && (r0 - a.1).abs() <= 0.5,
                "{curve:?}: level at {} drawn at {r0} and {r1}",
                a.1
            );
            levels += 1;
        }
        assert!(levels >= 3, "{curve:?}: {levels} levels");
    }
}

/// Unix-second timestamps a minute apart, a zigzag in y, so each chunk
/// stores x relative to its own origin.
fn zigzag(n: usize) -> Vec<Row> {
    (0..n)
        .map(|i| {
            let t = 1.7e9 + 60.0 * i as f64;
            (t, if i % 2 == 0 { 20.0 } else { 80.0 } + 2.0 * i as f64)
        })
        .collect()
}

/// AC3: a 10-row step line in 4-row chunks (two bridges) matches the
/// one-chunk render within 8 pixels at 2/255, and its path has no gap.
#[test]
fn chunked_steps_match_one_chunk() {
    let rows = zigzag(10);
    let xd = (1.7e9 - 60.0, 1.7e9 + 600.0);
    for curve in STEPS {
        let chunked = Opts {
            chunk: Some(4),
            ..Opts::new(curve)
        };
        let a = draw(&rows, xd, (0.0, 120.0), chunked);
        let (least, at) = path_min(&a, curve);
        assert!(least >= 0.9, "{curve:?}: coverage {least} at {at:?}");
        let b = draw(&rows, xd, (0.0, 120.0), Opts::new(curve));
        let differ = a
            .image
            .pixels()
            .zip(b.image.pixels())
            .filter(|(p, q)| p.0.iter().zip(q.0).any(|(x, y)| x.abs_diff(y) > 2))
            .count();
        assert!(differ <= 8, "{curve:?}: {differ} pixels differ");
    }
}

/// AC3: a null row breaks a step line as it breaks a straight one:
/// between the rows either side of it, only the pieces of other segments
/// have ink, in one chunk or with the null at a chunk's end.
#[test]
fn a_null_row_breaks_a_step_line() {
    let rows = vec![
        (40.0, 100.0),
        (120.0, 300.0),
        (200.0, f64::NAN),
        (280.0, 120.0),
        (360.0, 320.0),
        (440.0, 100.0),
    ];
    for curve in STEPS {
        for chunk in [None, Some(3)] {
            let o = Opts {
                chunk,
                ..Opts::new(curve)
            };
            let d = draw(&rows, (0.0, 500.0), (0.0, 400.0), o);
            let (least, at) = path_min(&d, curve);
            assert!(least >= 0.9, "{curve:?}: coverage {least} at {at:?}");
            let (b, e) = (d.points[1].unwrap(), d.points[3].unwrap());
            let drawn = pieces(&d, curve);
            let near = |p: (f64, f64)| {
                drawn
                    .iter()
                    .map(|&(s, t)| dist(p, s, t))
                    .fold(f64::INFINITY, f64::min)
            };
            let mut checked = 0;
            for py in 0..H {
                for px in b.0 as u32..e.0 as u32 {
                    let p = (f64::from(px) + 0.5, f64::from(py) + 0.5);
                    if near(p) <= 1.5 + 1.5 {
                        continue;
                    }
                    checked += 1;
                    let k = ink(&d.image, px, py);
                    assert!(
                        k < 2.0 / 255.0,
                        "{curve:?} {chunk:?}: ink {k} at ({px}, {py}) in the gap"
                    );
                }
            }
            assert!(checked > 1000, "only {checked} gap pixels");
        }
    }
}

/// AC4: CPU and GPU picks on a step line report the curve's row: for a
/// point on segment `i`'s path, `StepAfter` row `i`, `StepBefore` row
/// `i + 1`, `Step` row `i` left of the midpoint and `i + 1` right of it.
/// Points near the rows themselves, where two segments meet, and on
/// `Step`'s vertical are left out.
#[test]
fn step_picks_follow_the_curve() {
    let cx = Context::new_blocking().unwrap();
    let rows = steps_rows();
    for curve in STEPS {
        let mut p = Plot::new();
        let x = p.x(Linear::new().domain(0.0, 500.0));
        let y = p.y(Linear::new().domain(0.0, 400.0));
        p.add(Selection::<Row, Line>::new(rows.clone()))
            .x_with(x.encode(|r: &Row| r.0))
            .y_with(y.encode(|r: &Row| r.1))
            .width(Px(3.0))
            .curve(curve);
        p.resolve(&cx, W as f32, H as f32).unwrap();
        let pts: Vec<(f64, f64)> = rows
            .iter()
            .map(|r| (x.read().eval(r.0), y.read().eval(r.1)))
            .collect();
        let mut checked = 0;
        for i in 0..rows.len() - 1 {
            let (a, b) = (pts[i], pts[i + 1]);
            let mid = (a.0 + b.0) / 2.0;
            for (s, t) in step_path(curve, a, b).windows(2).map(|w| (w[0], w[1])) {
                for k in 0..=20 {
                    let f = f64::from(k) / 20.0;
                    // On the path, and 1 px off it.
                    for off in [0.0, 1.0] {
                        let q = (s.0 + f * (t.0 - s.0) + off, s.1 + f * (t.1 - s.1) + off);
                        let clear = 1.5 + 3.0 + 2.0;
                        if dist(q, a, a) < clear
                            || dist(q, b, b) < clear
                            || (curve == Curve::Step && (q.0 - mid).abs() < 1.0)
                        {
                            continue;
                        }
                        let want = match curve {
                            Curve::StepAfter => i,
                            Curve::StepBefore => i + 1,
                            _ if q.0 < mid => i,
                            _ => i + 1,
                        };
                        let at = Pt::new(q.0 as f32, q.1 as f32);
                        let want = Some(RowId {
                            layer: 0,
                            row: want,
                        });
                        assert_eq!(p.pick_row(at), want, "{curve:?} CPU at {q:?}, segment {i}");
                        assert_eq!(
                            p.pick_row_gpu(&cx, at).unwrap(),
                            want,
                            "{curve:?} GPU at {q:?}, segment {i}"
                        );
                        checked += 1;
                    }
                }
            }
            // The straight diagonal's quarter point is on no step.
            let q = (a.0 + (b.0 - a.0) / 4.0, a.1 + (b.1 - a.1) / 4.0);
            let at = Pt::new(q.0 as f32, q.1 as f32);
            assert_eq!(p.pick_row(at), None, "{curve:?} CPU off the path");
            assert_eq!(p.pick_row_gpu(&cx, at).unwrap(), None, "{curve:?} GPU");
        }
        assert!(checked > 100, "{curve:?}: only {checked} picks");
    }
}

/// AC4: `SvgTarget` writes a step line as one `<polyline>` through the
/// corners.
#[test]
fn svg_writes_a_step_line_as_one_polyline_with_corners() {
    let cx = Context::new_blocking().unwrap();
    for (curve, points) in [
        (Curve::StepAfter, 7),
        (Curve::StepBefore, 7),
        (Curve::Step, 10),
        (Curve::Linear, 4),
    ] {
        let mut p = Plot::new();
        p.legends(false);
        let x = p.x(Linear::new().domain(0.0, 500.0));
        let y = p.y(Linear::new().domain(0.0, 400.0));
        p.add(Selection::<Row, Line>::new(steps_rows()))
            .x_with(x.encode(|r: &Row| r.0))
            .y_with(y.encode(|r: &Row| r.1))
            .curve(curve);
        let resolved = p.resolve(&cx, W as f32, H as f32).unwrap();
        let mut target = SvgTarget::new();
        target.render(&resolved.scene).unwrap();
        let svg = target.svg();
        assert_eq!(svg.matches("<polyline ").count(), 1, "{curve:?}");
        let list = svg
            .split("<polyline ")
            .nth(1)
            .and_then(|s| s.split("points=\"").nth(1))
            .and_then(|s| s.split('"').next())
            .unwrap();
        assert_eq!(list.split_whitespace().count(), points, "{curve:?}: {list}");
    }
}

/// AC5: the same eight points drawn as `StepAfter`, `StepBefore` and
/// `Step` in three colours with a legend (three layers; each series is
/// raised by 6 so all three show), against its golden, and its SVG against
/// the GPU render.
#[test]
fn line_steps_golden() {
    let cx = Context::new_blocking().unwrap();
    let (w, h) = (640u32, 400u32);
    let ys = [3.0, 7.0, 5.0, 9.0, 8.5, 4.0, 6.0, 10.0];
    let series = [
        ("StepAfter", Curve::StepAfter, 0.0),
        ("StepBefore", Curve::StepBefore, 6.0),
        ("Step", Curve::Step, 12.0),
    ];
    let rows: Vec<(f64, f64, &str)> = series
        .iter()
        .flat_map(|&(name, _, lift)| {
            ys.iter()
                .enumerate()
                .map(move |(i, v)| (i as f64, v + lift, name))
        })
        .collect();
    let mut chart = gup_core::line(rows)
        .x(|r| r.0)
        .y(|r| r.1)
        .color(|r| r.2)
        .width(2.5)
        .legend_title("curve")
        .title("Step curves");
    for (layer, (_, curve, _)) in chart.layers_mut().into_iter().zip(series) {
        layer.curve(curve);
    }
    let resolved = chart.resolve(&cx, w as f32, h as f32).unwrap();
    let image = ImageTarget::new(&cx, w, h)
        .unwrap()
        .render_blocking(&cx, &resolved.scene)
        .unwrap();
    let vr = RgbaImage::new(image.width(), image.height(), image.as_raw().clone()).unwrap();
    let mut meta = layout_metadata(&resolved.layout);
    for (i, (name, ..)) in series.iter().enumerate() {
        meta = meta.with_expected_color(*name, rgba8(OKABE_ITO[i]));
    }
    harness()
        .run("gup_core/line_steps", Ok((vr.clone(), meta.clone())))
        .assert_ok();
    let svg = svg_marks::assert_matches("gup_core/line_steps", &resolved.scene, &vr, &meta);
    assert_eq!(svg.matches("<polyline ").count(), 3, "one run per series");
}

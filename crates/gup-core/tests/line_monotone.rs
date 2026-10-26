// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GUP-551: `Curve::MonotoneX`. The control values are computed on the CPU
//! as two more columns, read through the y scale, and each segment is
//! drawn as 16 pieces of its cubic. Ink is measured as in `line_curves.rs`:
//! with a red line on white, a pixel's coverage is `(red - green) / 255`.

mod common;

use gup_core::ImageTarget;
use gup_core::prelude::*;

type Row = (f64, f64);

const RED: Color = Color::hex(0xff0000);
const W: u32 = 500;
const H: u32 = 400;

/// Steffen's tangents and the cubic of each segment, written out
/// independently of the crate's (one run: finite rows, x increasing).
fn expected_curve(pts: &[(f64, f64)]) -> impl Fn(usize, f64) -> (f64, f64) + '_ {
    let n = pts.len();
    let h: Vec<f64> = (0..n - 1).map(|i| pts[i + 1].0 - pts[i].0).collect();
    let s: Vec<f64> = (0..n - 1)
        .map(|i| (pts[i + 1].1 - pts[i].1) / h[i])
        .collect();
    let end = |s0: f64, s1: f64, h0: f64, h1: f64| {
        let p = s0 * (1.0 + h0 / (h0 + h1)) - s1 * h0 / (h0 + h1);
        if p * s0 <= 0.0 {
            0.0
        } else if p.abs() > 2.0 * s0.abs() {
            2.0 * s0
        } else {
            p
        }
    };
    let m: Vec<f64> = (0..n)
        .map(|i| {
            if n == 2 {
                s[0]
            } else if i == 0 {
                end(s[0], s[1], h[0], h[1])
            } else if i == n - 1 {
                end(s[n - 2], s[n - 3], h[n - 2], h[n - 3])
            } else {
                let p = (s[i - 1] * h[i] + s[i] * h[i - 1]) / (h[i - 1] + h[i]);
                (s[i - 1].signum() * f64::from(s[i - 1] != 0.0)
                    + s[i].signum() * f64::from(s[i] != 0.0))
                    * s[i - 1].abs().min(s[i].abs()).min(p.abs() / 2.0)
            }
        })
        .collect();
    move |i, t| {
        let (a, b) = (pts[i], pts[i + 1]);
        let c1 = a.1 + m[i] * h[i] / 3.0;
        let c2 = b.1 - m[i + 1] * h[i] / 3.0;
        let u = 1.0 - t;
        (
            a.0 + t * h[i],
            u * u * u * a.1 + 3.0 * u * u * t * c1 + 3.0 * u * t * t * c2 + t * t * t * b.1,
        )
    }
}

/// The unit tangent of `curve` (of `n` rows) at row `i`.
fn tangent(curve: &impl Fn(usize, f64) -> (f64, f64), i: usize, n: usize) -> (f64, f64) {
    let (a, b) = if i + 1 < n {
        (curve(i, 0.0), curve(i, 1e-4))
    } else {
        (curve(i - 1, 1.0 - 1e-4), curve(i - 1, 1.0))
    };
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len = dx.hypot(dy);
    (dx / len, dy / len)
}

/// A rendered line and its rows' positions in pixels.
struct Drawn {
    image: image::RgbaImage,
    points: Vec<Option<(f64, f64)>>,
}

/// Draw `rows` as one red `curve` line, `width` wide, over x domain `xd`
/// and y domain `yd` (a log y scale with `log_y`).
fn draw(rows: &[Row], xd: (f64, f64), yd: (f64, f64), curve: Curve, log_y: bool) -> Drawn {
    let cx = Context::new_blocking().unwrap();
    let mut p = Plot::new();
    p.legends(false);
    let x = p.x(Linear::new().domain(xd.0, xd.1));
    let mut sel = Selection::<Row, Line>::new(rows.to_vec());
    let eval: Box<dyn Fn(f64) -> f64> = if log_y {
        let y = p.y(Log::new().domain(yd.0, yd.1));
        sel.y_with(y.encode(|r: &Row| r.1));
        Box::new(move |v| y.read().eval(v))
    } else {
        let y = p.y(Linear::new().domain(yd.0, yd.1));
        sel.y_with(y.encode(|r: &Row| r.1));
        Box::new(move |v| y.read().eval(v))
    };
    sel.x_with(x.encode(|r: &Row| r.0))
        .width(Px(3.0))
        .color(RED)
        .curve(curve);
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

/// The ink-weighted centre of pixel column `px` between `y0` and `y1`
/// (pixel rows), or `None` without ink.
fn column_centre(image: &image::RgbaImage, px: u32, y0: u32, y1: u32) -> Option<f64> {
    let (mut sum, mut weight) = (0.0, 0.0);
    for py in y0..y1.min(image.height()) {
        let c = ink(image, px, py);
        sum += c * (f64::from(py) + 0.5);
        weight += c;
    }
    (weight > 0.5).then(|| sum / weight)
}

/// The drawn centreline's y at `x` (pixels): the column centres either
/// side, interpolated.
fn centre_at(image: &image::RgbaImage, x: f64) -> Option<f64> {
    let left = (x - 0.5).floor();
    let f = x - 0.5 - left;
    let a = column_centre(image, left as u32, 0, image.height())?;
    let b = column_centre(image, left as u32 + 1, 0, image.height())?;
    Some(a + f * (b - a))
}

/// How far the drawn centreline passes from `p` across a curve whose
/// tangent at `p` is `tangent` (unit): the ink-weighted mean offset along
/// the normal of the pixels within 0.75 px of the normal through `p`. A
/// round cap at a line's end does not move it.
fn offset_across(image: &image::RgbaImage, p: (f64, f64), tangent: (f64, f64)) -> f64 {
    let normal = (-tangent.1, tangent.0);
    let (mut sum, mut weight) = (0.0, 0.0);
    for py in (p.1 - 6.0).floor() as u32..=(p.1 + 6.0) as u32 {
        for px in (p.0 - 6.0).floor() as u32..=(p.0 + 6.0) as u32 {
            let q = (f64::from(px) + 0.5 - p.0, f64::from(py) + 0.5 - p.1);
            let along = q.0 * tangent.0 + q.1 * tangent.1;
            if along.abs() > 0.75 {
                continue;
            }
            let c = ink(image, px, py);
            sum += c * (q.0 * normal.0 + q.1 * normal.1);
            weight += c;
        }
    }
    sum / weight
}

/// AC2: the glue of a monotone line reads two control columns through
/// the y scale and calls `vertex_controls`, in its own pipeline.
#[test]
fn a_monotone_line_reads_its_control_columns() {
    let cx = Context::new_blocking().unwrap();
    let mut p = Plot::new();
    let x = p.x(Linear::new());
    let y = p.y(Linear::new());
    p.add(Selection::<Row, Line>::new(vec![
        (0.0, 1.0),
        (1.0, 3.0),
        (2.0, 2.0),
    ]))
    .x_with(x.encode(|r: &Row| r.0))
    .y_with(y.encode(|r: &Row| r.1))
    .curve(Curve::MonotoneX);
    p.resolve(&cx, 320.0, 200.0).unwrap();
    let (signature, source) = p.glue_sources().remove(0);
    assert!(signature.ends_with("} curve monotone-x"), "{signature}");
    // After the column channels x and y, before the next-row columns.
    assert!(source.contains("@location(2) y_out: f32"), "{source}");
    assert!(source.contains("@location(3) y_in: f32"), "{source}");
    assert!(source.contains("@location(4) x_next: f32"), "{source}");
    assert!(
        source.contains(
            "let controls = vec2<f32>(linear::map_rel(col.y_out, chunk.y_out_base, enc.y), \
             linear::map_rel(col.y_in, chunk.y_in_base, enc.y));"
        ),
        "{source}"
    );
    assert!(
        source.contains(
            "line::vertex_controls(m, controls, vertex_index, chunk.row_base + instance_index, \
             u_view, 4u, enc.row_count)"
        ),
        "{source}"
    );
}

/// Monotone rows, far apart in pixels, with flat and steep stretches.
fn rising() -> Vec<Row> {
    vec![
        (0.0, 0.0),
        (1.0, 0.5),
        (2.0, 3.0),
        (3.0, 3.2),
        (4.0, 8.0),
        (5.0, 8.0),
        (6.0, 10.0),
    ]
}

/// AC2: the drawn curve passes within 0.5 px of every row, follows the
/// cubic described in the story, and for monotone data never leaves each
/// segment's y range between rows (sampled every 0.5 px).
#[test]
fn the_curve_passes_through_every_row_without_overshoot() {
    let rows = rising();
    let d = draw(&rows, (-0.5, 6.5), (-1.0, 11.0), Curve::MonotoneX, false);
    d.image.save("/tmp/gup551_rising.png").ok();
    let pts: Vec<(f64, f64)> = d.points.iter().map(|p| p.unwrap()).collect();
    let curve = expected_curve(&pts);
    for (i, &p) in pts.iter().enumerate() {
        let e = offset_across(&d.image, p, tangent(&curve, i, pts.len()));
        assert!(
            e.abs() <= 0.5,
            "row {p:?}: {e} px from the drawn centreline"
        );
    }
    let mut worst: (f64, (f64, f64)) = (0.0, (0.0, 0.0));
    for i in 0..pts.len() - 1 {
        let (a, b) = (pts[i], pts[i + 1]);
        let (lo, hi) = (a.1.min(b.1), a.1.max(b.1));
        let steps = ((b.0 - a.0) / 0.5).ceil() as usize;
        for k in 1..steps {
            let t = k as f64 / steps as f64;
            let (x, want) = curve(i, t);
            let c = centre_at(&d.image, x).unwrap();
            assert!(
                c >= lo - 0.25 && c <= hi + 0.25,
                "segment {i} at x {x}: centre {c} outside {lo}..{hi}"
            );
            if (c - want).abs() > worst.0 {
                worst = ((c - want).abs(), (x, want));
            }
        }
    }
    assert!(
        worst.0 <= 0.5,
        "the drawn curve is {worst:?} from the cubic"
    );
}

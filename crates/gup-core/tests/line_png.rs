// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! RFC-001 S6f (GUP-432): the `Line` mark rendered headlessly. Ink is
//! measured on the pixels: with a red line on white, a pixel's coverage is
//! `(red - green) / 255`, which grey guides and text do not disturb.

mod common;

use common::vr::{harness, layout_metadata, rgba8};
use gup_core::ImageTarget;
use gup_core::prelude::*;
use gup_visual_regression::RgbaImage;

const RED: Color = Color::hex(0xff0000);
const W: u32 = 500;
const H: u32 = 400;

type Row = (f64, f64);

/// Red ink at a pixel, 0..1.
fn ink(image: &image::RgbaImage, x: u32, y: u32) -> f64 {
    let p = image.get_pixel(x, y).0;
    (f64::from(p[0]) - f64::from(p[1])) / 255.0
}

/// A rendered line through `rows` (fixed domains, so data maps the same
/// whatever the rows), and the rows' positions in pixels.
struct Drawn {
    image: image::RgbaImage,
    points: Vec<Option<(f64, f64)>>,
}

/// Draw `rows` as one red line `width` px wide over x domain `xd` and y
/// domain `yd`, in chunks of `chunk` rows if given.
fn draw(
    rows: &[Row],
    xd: (f64, f64),
    yd: (f64, f64),
    width: f32,
    color: Color,
    chunk: Option<u32>,
) -> Drawn {
    let cx = Context::new_blocking().unwrap();
    let mut p = Plot::new();
    p.legends(false);
    let x = p.x(Linear::new().domain(xd.0, xd.1));
    let y = p.y(Linear::new().domain(yd.0, yd.1));
    let line = p.add(Selection::<Row, Line>::new(rows.to_vec()));
    line.x_with(x.encode(|r: &Row| r.0))
        .y_with(y.encode(|r: &Row| r.1))
        .width(Px(width))
        .color(color);
    if let Some(n) = chunk {
        line.max_chunk_rows(n);
    }
    let resolved = p.resolve(&cx, W as f32, H as f32).unwrap();
    let image = ImageTarget::new(&cx, W, H)
        .unwrap()
        .render_blocking(&cx, &resolved.scene)
        .unwrap();
    let points = rows
        .iter()
        .map(|&(a, b)| {
            (a.is_finite() && b.is_finite()).then(|| (x.read().eval(a), y.read().eval(b)))
        })
        .collect();
    Drawn { image, points }
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

/// The pixel holding point `p`.
fn pixel(p: (f64, f64)) -> (u32, u32) {
    (p.0.floor() as u32, p.1.floor() as u32)
}

/// Every pixel on the centreline of each drawn segment, sampled every
/// 0.5 px, and the least coverage among them.
fn centreline_min(d: &Drawn) -> (f64, (f64, f64)) {
    let mut worst = (f64::INFINITY, (0.0, 0.0));
    for pair in d.points.windows(2) {
        let (Some(a), Some(b)) = (pair[0], pair[1]) else {
            continue;
        };
        let len = (b.0 - a.0).hypot(b.1 - a.1);
        let steps = (len / 0.5).ceil() as usize;
        for s in 0..=steps {
            let t = s as f64 / steps as f64;
            let p = (a.0 + t * (b.0 - a.0), a.1 + t * (b.1 - a.1));
            let (px, py) = pixel(p);
            let c = ink(&d.image, px, py);
            if c < worst.0 {
                worst = (c, p);
            }
        }
    }
    worst
}

/// Unix-second timestamps a minute apart from 2023-11-14, a zigzag in y:
/// each chunk stores x relative to its own origin.
fn zigzag(n: usize) -> Vec<Row> {
    (0..n)
        .map(|i| {
            let t = 1.7e9 + 60.0 * i as f64;
            (t, if i % 2 == 0 { 20.0 } else { 80.0 } + 2.0 * i as f64)
        })
        .collect()
}

/// AC2: ten rows in chunks of four draw nine segments with no gap at the
/// two chunk boundaries: every centreline pixel is covered.
#[test]
fn a_chunked_line_has_no_gap_at_chunk_boundaries() {
    let rows = zigzag(10);
    let xd = (1.7e9 - 60.0, 1.7e9 + 600.0);
    let chunked = draw(&rows, xd, (0.0, 120.0), 3.0, RED, Some(4));
    let (least, at) = centreline_min(&chunked);
    assert!(least >= 0.9, "coverage {least} at {at:?}");
    // The same image as one chunk, to within the per-chunk origins'
    // rounding.
    let whole = draw(&rows, xd, (0.0, 120.0), 3.0, RED, None);
    let differ = chunked
        .image
        .pixels()
        .zip(whole.image.pixels())
        .filter(|(a, b)| a.0.iter().zip(b.0).any(|(x, y)| x.abs_diff(y) > 2))
        .count();
    assert!(
        differ <= 8,
        "{differ} pixels differ from the one-chunk line"
    );
}

/// Ink of `d` within `r` px of point `p`, by pixel: the least ink among
/// pixels whose centre lies within `r` of the polyline and `radius` of `p`.
fn least_ink_near(d: &Drawn, p: (f64, f64), radius: f64, r: f64) -> (f64, (u32, u32)) {
    let segs: Vec<_> = d
        .points
        .windows(2)
        .filter_map(|w| Some((w[0]?, w[1]?)))
        .collect();
    let mut worst = (f64::INFINITY, (0, 0));
    let (x0, x1) = ((p.0 - radius) as u32, (p.0 + radius) as u32);
    let (y0, y1) = ((p.1 - radius) as u32, (p.1 + radius) as u32);
    for py in y0..=y1 {
        for px in x0..=x1 {
            let c = (f64::from(px) + 0.5, f64::from(py) + 0.5);
            if (c.0 - p.0).hypot(c.1 - p.1) > radius {
                continue;
            }
            if segs
                .iter()
                .map(|&(a, b)| dist(c, a, b))
                .fold(f64::INFINITY, f64::min)
                > r
            {
                continue;
            }
            let k = ink(&d.image, px, py);
            if k < worst.0 {
                worst = (k, (px, py));
            }
        }
    }
    worst
}

/// Pixels per data unit along x and y (signed) over the 0..500, 0..400
/// domains the join tests use.
fn pixels_per_unit() -> (f64, f64) {
    let probe = draw(
        &[(0.0, 0.0), (100.0, 100.0)],
        (0.0, 500.0),
        (0.0, 400.0),
        1.0,
        RED,
        None,
    );
    let (a, b) = (probe.points[0].unwrap(), probe.points[1].unwrap());
    ((b.0 - a.0) / 100.0, (b.1 - a.1) / 100.0)
}

/// A "V" with its apex at data (250, 200) and an angle of `angle` degrees
/// between its arms, each arm 120 px long, measured in pixels.
fn vee(angle: f64) -> Vec<Row> {
    let (kx, ky) = pixels_per_unit();
    let half = angle.to_radians() / 2.0;
    let apex = (250.0, 200.0);
    let arm = 120.0;
    let (dx, dy) = (arm * half.sin() / kx, arm * half.cos() / ky);
    vec![(apex.0 - dx, apex.1 - dy), apex, (apex.0 + dx, apex.1 - dy)]
}

/// AC3: a 30 degree "V" and a shallow 170 degree turn, 6 px wide, are
/// covered across their whole width at the join: the centreline, the outer
/// edge of the round join and the inside of the stroke, with no seam.
#[test]
fn joins_have_no_wedge_gap_and_no_seam() {
    for angle in [30.0, 170.0] {
        let rows = vee(angle);
        let probe = draw(&rows, (0.0, 500.0), (0.0, 400.0), 6.0, RED, None);
        let apex = probe.points[1].unwrap();
        let (least, at) = centreline_min(&probe);
        assert!(
            least >= 0.9,
            "{angle}°: centreline coverage {least} at {at:?}"
        );
        // Inside the stroke near the join: within 2 px of the polyline
        // (the stroke is 3 px either side) every pixel is full.
        let (least, at) = least_ink_near(&probe, apex, 12.0, 2.0);
        assert!(
            least > 1.0 - 2.0 / 255.0,
            "{angle}°: seam inside the stroke, ink {least} at {at:?}"
        );
        // The outer edge of the round join: every pixel centred within
        // 2.5 px of the apex (the stroke's edge is 3 px out), where a
        // butt join leaves a wedge on the outer side.
        let (least, at) = least_ink_near(&probe, apex, 2.5, 2.5);
        assert!(
            least >= 0.9,
            "{angle}°: outer join coverage {least} at {at:?}"
        );
    }
}

/// AC3, documented behaviour: a translucent line blends twice where two
/// segments overlap at a join, so the join is darker than the arms.
#[test]
fn a_translucent_join_blends_twice() {
    let half_red = Color { a: 0.5, ..RED };
    let rows = vee(90.0);
    let d = draw(&rows, (0.0, 500.0), (0.0, 400.0), 6.0, half_red, None);
    let (apex, a) = (d.points[1].unwrap(), d.points[0].unwrap());
    let arm = pixel(((apex.0 + a.0) / 2.0, (apex.1 + a.1) / 2.0));
    let (join, arm) = (
        ink(&d.image, apex.0 as u32, apex.1 as u32),
        ink(&d.image, arm.0, arm.1),
    );
    assert!((arm - 0.5).abs() < 0.02, "arm {arm}");
    assert!((join - 0.75).abs() < 0.02, "join {join}");
}

/// Assert that `d` has no ink within `width` of the gap from point `from`
/// to point `to` except near the drawn segments.
fn assert_gap(d: &Drawn, from: usize, to: usize, width: f64) {
    let (b, c) = (d.points[from].unwrap(), d.points[to].unwrap());
    let segs: Vec<_> = d
        .points
        .windows(2)
        .filter_map(|w| Some((w[0]?, w[1]?)))
        .collect();
    let mut checked = 0;
    for py in 0..H {
        for px in 0..W {
            let p = (f64::from(px) + 0.5, f64::from(py) + 0.5);
            if dist(p, b, c) > width {
                continue;
            }
            let near = segs
                .iter()
                .map(|&(a, e)| dist(p, a, e))
                .fold(f64::INFINITY, f64::min);
            if near <= width / 2.0 + 1.5 {
                continue;
            }
            checked += 1;
            let k = ink(&d.image, px, py);
            assert!(
                k < 2.0 / 255.0,
                "ink {k} at ({px}, {py}) in the gap {from}–{to}"
            );
        }
    }
    assert!(checked > 50, "only {checked} gap pixels");
}

/// AC4: a NaN in the middle of a series draws two disconnected pieces.
#[test]
fn a_null_row_breaks_the_line() {
    let rows = vec![
        (50.0, 100.0),
        (150.0, 300.0),
        (250.0, f64::NAN),
        (350.0, 100.0),
        (450.0, 300.0),
    ];
    let width = 4.0;
    let d = draw(&rows, (0.0, 500.0), (0.0, 400.0), width as f32, RED, None);
    assert_gap(&d, 1, 3, width);
    // Both pieces are drawn.
    let (least, at) = centreline_min(&d);
    assert!(least >= 0.9, "coverage {least} at {at:?}");
}

/// AC4 across chunks: nulls at the last row of a chunk, mid-chunk and the
/// first row of a chunk each break the line, and the pieces between them
/// join across the other boundaries.
#[test]
fn null_rows_break_the_line_at_chunk_boundaries() {
    let mut rows: Vec<Row> = (0..14)
        .map(|i| {
            (
                20.0 + 33.0 * i as f64,
                if i % 2 == 0 { 100.0 } else { 300.0 },
            )
        })
        .collect();
    for i in [3, 6, 8] {
        rows[i].1 = f64::NAN;
    }
    let width = 3.0;
    let d = draw(
        &rows,
        (0.0, 500.0),
        (0.0, 400.0),
        width as f32,
        RED,
        Some(4),
    );
    assert_gap(&d, 2, 4, width);
    assert_gap(&d, 5, 7, width);
    assert_gap(&d, 7, 9, width);
    let (least, at) = centreline_min(&d);
    assert!(least >= 0.9, "coverage {least} at {at:?}");
}

/// AC5: a 200-point series, 2 px wide, with one null gap, in 64-row
/// chunks (three bridges).
#[test]
fn line_series_golden() {
    let cx = Context::new_blocking().unwrap();
    let (w, h) = (720u32, 400u32);
    let rows: Vec<Row> = (0..200)
        .map(|i| {
            let t = f64::from(i);
            let v = 50.0
                + 18.0 * (t / 17.0).sin()
                + 7.0 * (t / 5.3).cos()
                + 3.0 * (t * 1.7).sin()
                + 0.08 * t;
            (t, if (120..126).contains(&i) { f64::NAN } else { v })
        })
        .collect();
    let mut p = Plot::new();
    p.legends(false);
    p.title("A 200-point series with a gap");
    let x = p.x(Linear::new());
    let y = p.y(Linear::new());
    p.add(Selection::<Row, Line>::new(rows))
        .x_with(x.encode(|r: &Row| r.0))
        .y_with(y.encode(|r: &Row| r.1))
        .width(Px(2.0))
        .max_chunk_rows(64);
    let resolved = p.resolve(&cx, w as f32, h as f32).unwrap();
    let image = ImageTarget::new(&cx, w, h)
        .unwrap()
        .render_blocking(&cx, &resolved.scene)
        .unwrap();
    let vr = RgbaImage::new(image.width(), image.height(), image.as_raw().clone()).unwrap();
    let meta =
        layout_metadata(&resolved.layout).with_expected_color("line", rgba8(Line::DEFAULT_COLOR));
    harness()
        .run("gup_core/line_series", Ok((vr, meta)))
        .assert_ok();
}

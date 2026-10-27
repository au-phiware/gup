// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GUP-551: `Curve::MonotoneX`. The control values are computed on the CPU
//! as two more columns, read through the y scale, and each segment is
//! drawn as 16 pieces of its cubic. Ink is measured as in `line_curves.rs`:
//! with a red line on white, a pixel's coverage is `(red - green) / 255`.

mod common;

use common::svg_marks;
use common::vr::{harness, layout_metadata, rgba8};
use gup_core::geom::Point as Pt;
use gup_core::prelude::*;
use gup_core::{ImageTarget, OKABE_ITO, RowId, SvgTarget, VectorTarget};
use gup_visual_regression::RgbaImage;

type Row = (f64, f64);
type Px2 = (f64, f64);

const RED: Color = Color::hex(0xff0000);
const W: u32 = 500;
const H: u32 = 400;
/// The pieces each segment is drawn as (`marks::MONOTONE_PIECES`).
const PIECES: u32 = 16;

/// Steffen's tangents of one run (finite rows, x increasing), written out
/// independently of the crate's.
fn steffen(rows: &[Row]) -> Vec<f64> {
    let n = rows.len();
    let h: Vec<f64> = (0..n - 1).map(|i| rows[i + 1].0 - rows[i].0).collect();
    let s: Vec<f64> = (0..n - 1)
        .map(|i| (rows[i + 1].1 - rows[i].1) / h[i])
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
    let sign = |v: f64| if v == 0.0 { 0.0 } else { v.signum() };
    (0..n)
        .map(|i| {
            if n == 2 {
                s[0]
            } else if i == 0 {
                end(s[0], s[1], h[0], h[1])
            } else if i == n - 1 {
                end(s[n - 2], s[n - 3], h[n - 2], h[n - 3])
            } else {
                let p = (s[i - 1] * h[i] + s[i] * h[i - 1]) / (h[i - 1] + h[i]);
                (sign(s[i - 1]) + sign(s[i])) * s[i - 1].abs().min(s[i].abs()).min(p.abs() / 2.0)
            }
        })
        .collect()
}

/// The pixel-space cubic of each segment of one run: the control values
/// computed in data units, mapped through the scales like the rows, at
/// one and two thirds of the segment in x.
struct Expected {
    segments: Vec<[Px2; 4]>,
}

impl Expected {
    fn new(rows: &[Row], fx: &dyn Fn(f64) -> f64, fy: &dyn Fn(f64) -> f64) -> Self {
        let m = steffen(rows);
        let segments = (0..rows.len() - 1)
            .map(|i| {
                let (a, b) = (rows[i], rows[i + 1]);
                let h = b.0 - a.0;
                let (c1, c2) = (a.1 + m[i] * h / 3.0, b.1 - m[i + 1] * h / 3.0);
                let (x0, x1) = (fx(a.0), fx(b.0));
                let dx = x1 - x0;
                [
                    (x0, fy(a.1)),
                    (x0 + dx / 3.0, fy(c1)),
                    (x0 + dx * 2.0 / 3.0, fy(c2)),
                    (x1, fy(b.1)),
                ]
            })
            .collect();
        Self { segments }
    }

    /// Segment `i` at `t`.
    fn at(&self, i: usize, t: f64) -> Px2 {
        let p = self.segments[i];
        let u = 1.0 - t;
        let (b0, b1, b2, b3) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
        (
            b0 * p[0].0 + b1 * p[1].0 + b2 * p[2].0 + b3 * p[3].0,
            b0 * p[0].1 + b1 * p[1].1 + b2 * p[2].1 + b3 * p[3].1,
        )
    }

    /// The rows, in pixels.
    fn rows(&self) -> Vec<Px2> {
        let mut out: Vec<Px2> = self.segments.iter().map(|s| s[0]).collect();
        out.push(self.segments.last().unwrap()[3]);
        out
    }

    /// The unit tangent at row `i`.
    fn tangent(&self, i: usize) -> Px2 {
        let (a, b) = if i < self.segments.len() {
            (self.at(i, 0.0), self.at(i, 1e-4))
        } else {
            (self.at(i - 1, 1.0 - 1e-4), self.at(i - 1, 1.0))
        };
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let len = dx.hypot(dy);
        (dx / len, dy / len)
    }

    /// The curve as a polyline of `pieces` per segment, at the Chebyshev
    /// nodes `(1 - cos(pi k / pieces)) / 2` of the cubic's parameter (as
    /// drawn, for 16).
    fn polyline(&self, pieces: u32) -> Vec<Px2> {
        let node =
            |k: u32| 0.5 - 0.5 * (std::f64::consts::PI * f64::from(k) / f64::from(pieces)).cos();
        let mut out = vec![self.segments[0][0]];
        for i in 0..self.segments.len() {
            out.extend((1..=pieces).map(|k| self.at(i, node(k))));
        }
        out
    }
}

/// Options for [`Fixture::new`].
#[derive(Clone, Copy)]
struct Opts {
    curve: Curve,
    width: f32,
    log_y: bool,
    chunk: Option<u32>,
    retain: Retain,
    color: Color,
}

impl Default for Opts {
    fn default() -> Self {
        Self {
            curve: Curve::MonotoneX,
            width: 3.0,
            log_y: false,
            chunk: None,
            retain: Retain::Auto,
            color: RED,
        }
    }
}

/// One line over fixed domains, and its scales' maps.
struct Fixture {
    cx: Context,
    plot: Plot,
    fx: Box<dyn Fn(f64) -> f64>,
    fy: Box<dyn Fn(f64) -> f64>,
}

impl Fixture {
    fn new(rows: &[Row], xd: (f64, f64), yd: (f64, f64), o: Opts) -> Self {
        let cx = Context::new_blocking().unwrap();
        let mut plot = Plot::new();
        plot.legends(false);
        let x = plot.x(Linear::new().domain(xd.0, xd.1));
        let mut sel = Selection::<Row, Line>::new(rows.to_vec());
        let fy: Box<dyn Fn(f64) -> f64> = if o.log_y {
            let y = plot.y(Log::new().domain(yd.0, yd.1));
            sel.y_with(y.encode(|r: &Row| r.1));
            Box::new(move |v| y.read().eval(v))
        } else {
            let y = plot.y(Linear::new().domain(yd.0, yd.1));
            sel.y_with(y.encode(|r: &Row| r.1));
            Box::new(move |v| y.read().eval(v))
        };
        sel.x_with(x.encode(|r: &Row| r.0))
            .width(Px(o.width))
            .color(o.color)
            .curve(o.curve)
            .retain(o.retain);
        if let Some(n) = o.chunk {
            sel.max_chunk_rows(n);
        }
        plot.add(sel);
        let mut f = Self {
            cx,
            plot,
            fx: Box::new(move |v| x.read().eval(v)),
            fy,
        };
        // Resolve once, so the scales' maps are set.
        f.image();
        f
    }

    fn image(&mut self) -> image::RgbaImage {
        let resolved = self.plot.resolve(&self.cx, W as f32, H as f32).unwrap();
        ImageTarget::new(&self.cx, W, H)
            .unwrap()
            .render_blocking(&self.cx, &resolved.scene)
            .unwrap()
    }

    fn expected(&self, rows: &[Row]) -> Expected {
        Expected::new(rows, &self.fx, &self.fy)
    }

    fn append(&mut self, rows: &[Row]) {
        self.plot
            .layer_mut::<Row, Line>()
            .unwrap()
            .append(rows.to_vec())
            .unwrap();
    }
}

/// Red ink at a pixel, 0..1.
fn ink(image: &image::RgbaImage, x: u32, y: u32) -> f64 {
    let p = image.get_pixel(x, y).0;
    (f64::from(p[0]) - f64::from(p[1])) / 255.0
}

/// The ink-weighted centre of pixel column `px`, or `None` without ink.
fn column_centre(image: &image::RgbaImage, px: u32) -> Option<f64> {
    let (mut sum, mut weight) = (0.0, 0.0);
    for py in 0..image.height() {
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
    let a = column_centre(image, left as u32)?;
    let b = column_centre(image, left as u32 + 1)?;
    Some(a + f * (b - a))
}

/// How far the drawn centreline passes from `p` across a curve whose
/// tangent at `p` is `tangent` (unit): the ink-weighted mean offset along
/// the normal of the pixels within 0.75 px of the normal through `p` and
/// 4 px of `p` across it (so a neighbouring segment does not count). A
/// round cap at a line's end does not move it.
fn offset_across(image: &image::RgbaImage, p: Px2, tangent: Px2) -> f64 {
    let normal = (-tangent.1, tangent.0);
    let (mut sum, mut weight) = (0.0, 0.0);
    for py in (p.1 - 6.0).floor() as u32..=(p.1 + 6.0) as u32 {
        for px in (p.0 - 6.0).floor() as u32..=(p.0 + 6.0) as u32 {
            let q = (f64::from(px) + 0.5 - p.0, f64::from(py) + 0.5 - p.1);
            let along = q.0 * tangent.0 + q.1 * tangent.1;
            let across = q.0 * normal.0 + q.1 * normal.1;
            if along.abs() > 0.75 || across.abs() > 4.0 {
                continue;
            }
            let c = ink(image, px, py);
            sum += c * across;
            weight += c;
        }
    }
    sum / weight
}

/// Distance from `p` to the segment `a`–`b`.
fn dist(p: Px2, a: Px2, b: Px2) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dy * dy;
    let t = if len2 > 0.0 {
        (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (p.0 - a.0 - t * dx).hypot(p.1 - a.1 - t * dy)
}

/// The fraction of the one-pixel footprint at distance `t` from a line
/// inside a stroke of half width `half` (the shaders' `span`).
fn span(t: f64, half: f64) -> f64 {
    ((t + 0.5).min(half) - (t - 0.5).max(-half)).clamp(0.0, 1.0)
}

/// The largest difference between each pixel's ink and the coverage of a
/// stroke `width` wide along `path` (the shaders' footprint: `span` of the
/// distance to the path), over the pixels within `width / 2 + 1.5` of it,
/// and where.
fn worst_coverage(image: &image::RgbaImage, path: &[Px2], width: f64) -> (f64, (u32, u32)) {
    let reach = width / 2.0 + 1.5;
    let (lo, hi) = path.iter().fold(
        (
            (f64::INFINITY, f64::INFINITY),
            (f64::NEG_INFINITY, f64::NEG_INFINITY),
        ),
        |(lo, hi), p| {
            (
                (lo.0.min(p.0), lo.1.min(p.1)),
                (hi.0.max(p.0), hi.1.max(p.1)),
            )
        },
    );
    let mut worst = (0.0, (0, 0));
    for py in (lo.1 - reach).floor() as u32..=(hi.1 + reach) as u32 {
        for px in (lo.0 - reach).floor() as u32..=(hi.0 + reach) as u32 {
            let q = (f64::from(px) + 0.5, f64::from(py) + 0.5);
            let d = path
                .windows(2)
                .map(|s| dist(q, s[0], s[1]))
                .fold(f64::INFINITY, f64::min);
            if d > reach {
                continue;
            }
            let e = (ink(image, px, py) - span(d, width / 2.0)).abs();
            if e > worst.0 {
                worst = (e, (px, py));
            }
        }
    }
    worst
}

/// AC1: the worst case, a monotone line with column x, y, width and
/// colour, binds 8 vertex columns (4 channels, 2 control columns, 2
/// next-row columns) and draws.
#[test]
fn the_worst_case_binds_eight_vertex_columns() {
    let cx = Context::new_blocking().unwrap();
    let mut p = Plot::new();
    let x = p.x(Linear::new());
    let y = p.y(Linear::new());
    p.add(Selection::<Row, Line>::new(monthly()))
        .x_with(x.encode(|r: &Row| r.0))
        .y_with(y.encode(|r: &Row| r.1))
        .attr(
            Line::WIDTH,
            Linear::new()
                .domain(0.0, 10.0)
                .range(Px(1.0), Px(4.0))
                .encode(|r: &Row| r.1),
        )
        .attr(Line::COLOR, Sequential::viridis().encode(|r: &Row| r.1))
        .curve(Curve::MonotoneX);
    let image = p.render(&cx, 320, 200).unwrap();
    assert!(image.pixels().any(|q| q.0 != [255, 255, 255, 255]));
    let (_, source) = p.glue_sources().remove(0);
    assert!(source.contains("@location(7) y_next: f32"), "{source}");
    assert!(!source.contains("@location(8)"), "{source}");
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

/// Twelve noisy monthly values (the golden's).
fn monthly() -> Vec<Row> {
    [4.2, 5.1, 3.4, 6.8, 7.2, 6.9, 9.5, 8.1, 8.4, 5.0, 6.2, 4.4]
        .iter()
        .enumerate()
        .map(|(i, &v)| (i as f64 + 1.0, v))
        .collect()
}

/// Every row lies within `limit` px of the drawn centreline, measured
/// across the expected curve.
fn assert_through_rows(image: &image::RgbaImage, e: &Expected, limit: f64) {
    for (i, p) in e.rows().into_iter().enumerate() {
        let off = offset_across(image, p, e.tangent(i));
        assert!(
            off.abs() <= limit,
            "row {i} {p:?}: {off} px from the drawn centreline"
        );
    }
}

/// AC2: the drawn curve passes within 0.5 px of every row, follows the
/// cubic described in the story, and for monotone data never leaves each
/// segment's y range between rows (sampled every 0.5 px).
#[test]
fn the_curve_passes_through_every_row_without_overshoot() {
    let rows = rising();
    let mut f = Fixture::new(&rows, (-0.5, 6.5), (-1.0, 11.0), Opts::default());
    let image = f.image();
    let e = f.expected(&rows);
    assert_through_rows(&image, &e, 0.5);
    let pts = e.rows();
    let mut worst: (f64, Px2) = (0.0, (0.0, 0.0));
    for i in 0..pts.len() - 1 {
        let (a, b) = (pts[i], pts[i + 1]);
        let (lo, hi) = (a.1.min(b.1), a.1.max(b.1));
        let steps = ((b.0 - a.0) / 0.5).ceil() as usize;
        for k in 1..steps {
            let (x, want) = e.at(i, k as f64 / steps as f64);
            let c = centre_at(&image, x).unwrap();
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

    // Noisy data: through every row too.
    let rows = monthly();
    let mut f = Fixture::new(&rows, (0.5, 12.5), (2.0, 11.0), Opts::default());
    let image = f.image();
    assert_through_rows(&image, &f.expected(&rows), 0.5);
}

/// The most a polyline of 16 pieces per segment (as drawn) strays from
/// the cubic, in pixels: sampled at 64 points per piece.
fn tessellation_error(e: &Expected) -> f64 {
    let pieces = e.polyline(PIECES);
    let mut worst = 0.0f64;
    for i in 0..e.segments.len() {
        for k in 0..PIECES * 64 {
            let p = e.at(i, f64::from(k) / f64::from(PIECES * 64));
            let near = pieces[i * PIECES as usize..=(i + 1) * PIECES as usize]
                .windows(2)
                .map(|s| dist(p, s[0], s[1]))
                .fold(f64::INFINITY, f64::min);
            worst = worst.max(near);
        }
    }
    worst
}

/// No seam, hole or darker joint: every pixel near the curve has the
/// coverage of a round-joined stroke along the drawn pieces (16 per
/// segment, the shaders' footprint: `span` of the distance), so no pixel
/// at a joint between pieces, or at a row between segments, is drawn
/// twice or left out. Widths 1.5, 3 and 6. Measured (2026-10-27): within
/// 0.004 everywhere but the inner side of `monthly`'s sharpest valley at
/// width 6 (0.065), where pieces that are not neighbours overlap because
/// the curve bends tighter than the stroke's half width. The pieces stray
/// from the cubic by under a quarter pixel, even at the sharp knees of
/// `rising`.
#[test]
fn joints_are_seamless() {
    for rows in [rising(), monthly()] {
        let xd = (rows[0].0 - 0.5, rows[rows.len() - 1].0 + 0.5);
        for width in [1.5, 3.0, 6.0] {
            let o = Opts {
                width,
                ..Opts::default()
            };
            let mut f = Fixture::new(&rows, xd, (-1.0, 11.0), o);
            let image = f.image();
            let expected = f.expected(&rows);
            let t = tessellation_error(&expected);
            let path = expected.polyline(PIECES);
            let (e, at) = worst_coverage(&image, &path, f64::from(width));
            assert!(t < 0.25, "the pieces stray {t} px from the cubic");
            assert!(e <= 0.08, "width {width}: ink off by {e} at {at:?}");
        }
    }
}

/// AC2: on a log y scale the curve still passes through every row and
/// stays within each segment's pixel y range (the control values lie
/// between the rows' values, and the scale is monotone).
#[test]
fn on_a_log_scale_the_curve_stays_between_its_rows() {
    let rows: Vec<Row> = [3.0, 4.0, 40.0, 55.0, 600.0, 650.0, 900.0, 5000.0]
        .iter()
        .enumerate()
        .map(|(i, &v)| (i as f64, v))
        .collect();
    let o = Opts {
        log_y: true,
        ..Opts::default()
    };
    let mut f = Fixture::new(&rows, (-0.5, 7.5), (1.0, 10_000.0), o);
    let image = f.image();
    let e = f.expected(&rows);
    assert_through_rows(&image, &e, 0.5);
    // The two segments at a row each cut their pieces at the normal of
    // their own tangent there, which a log scale bends apart; no seam
    // shows (measured 0.004 on 2026-10-27).
    let (worst, at) = worst_coverage(&image, &e.polyline(PIECES), 3.0);
    assert!(worst <= 0.08, "ink off by {worst} at {at:?}");
    let pts = e.rows();
    for i in 0..pts.len() - 1 {
        let (a, b) = (pts[i], pts[i + 1]);
        let (lo, hi) = (a.1.min(b.1), a.1.max(b.1));
        let steps = ((b.0 - a.0) / 0.5).ceil() as usize;
        for k in 1..steps {
            let (x, _) = e.at(i, k as f64 / steps as f64);
            let c = centre_at(&image, x).unwrap();
            assert!(
                c >= lo - 0.25 && c <= hi + 0.25,
                "segment {i} at x {x}: centre {c} outside {lo}..{hi}"
            );
        }
    }
}

/// AC2: a segment whose x does not increase is drawn straight: a vertical
/// one (x equal) and one that runs back in x, each inked along its chord
/// and centred on it.
#[test]
fn non_increasing_x_draws_a_straight_segment() {
    let rows = vec![
        (0.0, 1.0),
        (2.0, 2.0),
        (2.0, 6.0),
        (4.0, 7.0),
        (3.0, 9.0),
        (5.0, 10.0),
    ];
    let mut f = Fixture::new(&rows, (-0.5, 5.5), (0.0, 11.0), Opts::default());
    let image = f.image();
    let px: Vec<Px2> = rows.iter().map(|r| ((f.fx)(r.0), (f.fy)(r.1))).collect();
    for (i, j) in [(1, 2), (3, 4)] {
        let (a, b) = (px[i], px[j]);
        let len = (b.0 - a.0).hypot(b.1 - a.1);
        let dir = ((b.0 - a.0) / len, (b.1 - a.1) / len);
        let steps = (len / 0.5) as usize;
        for k in 0..=steps {
            let s = k as f64 * 0.5;
            if s < 10.0 || s > len - 10.0 {
                continue;
            }
            let p = (a.0 + dir.0 * s, a.1 + dir.1 * s);
            let c = ink(&image, p.0 as u32, p.1 as u32);
            assert!(c >= 0.9, "segment {i}: ink {c} at {p:?} on the chord");
            let off = offset_across(&image, p, dir);
            assert!(
                off.abs() <= 0.3,
                "segment {i}: {off} px off the chord at {p:?}"
            );
        }
    }
}

/// The number of pixels where `a` and `b` differ by more than 2/255 in
/// any channel, among those `keep` accepts.
fn differing(a: &image::RgbaImage, b: &image::RgbaImage, keep: impl Fn(u32, u32) -> bool) -> usize {
    a.enumerate_pixels()
        .filter(|(x, y, p)| {
            keep(*x, *y)
                && p.0
                    .iter()
                    .zip(b.get_pixel(*x, *y).0)
                    .any(|(&u, v)| u.abs_diff(v) > 2)
        })
        .count()
}

/// AC2: a null row breaks the curve and restarts its tangents: each run
/// is drawn as it is alone (away from the rows next to the null, whose
/// ends meet the null flat rather than round), and nothing is drawn
/// across the gap.
#[test]
fn a_null_breaks_the_curve_and_restarts_its_tangents() {
    let rows = monthly();
    let mut with_null = rows.clone();
    with_null[5].1 = f64::NAN;
    let (xd, yd) = ((0.5, 12.5), (2.0, 11.0));
    let mut f = Fixture::new(&with_null, xd, yd, Opts::default());
    let image = f.image();
    let left = Fixture::new(&rows[..5], xd, yd, Opts::default()).image();
    let right = Fixture::new(&rows[6..], xd, yd, Opts::default()).image();
    let (x4, x6) = ((f.fx)(rows[4].0), (f.fx)(rows[6].0));
    let n = differing(&image, &left, |x, _| f64::from(x) < x4 - 4.0);
    assert!(n <= 8, "{n} pixels differ from the first run alone");
    let n = differing(&image, &right, |x, _| f64::from(x) > x6 + 4.0);
    assert!(n <= 8, "{n} pixels differ from the second run alone");
    for x in (x4 + 4.0) as u32..(x6 - 4.0) as u32 {
        assert_eq!(column_centre(&image, x), None, "ink in the gap at x {x}");
    }
}

/// Twenty-one noisy rows.
fn wavy() -> Vec<Row> {
    (0..21)
        .map(|i| {
            let t = f64::from(i);
            (t, 5.0 + 3.0 * (t * 0.8).sin() + (t * 2.3).cos())
        })
        .collect()
}

/// AC2: appending rows (in batches, across 4-row chunks, so bridges and
/// the rewritten rows fall in earlier chunks) draws the curve of all the
/// rows built at once, within 8 pixels at 2/255. Under `Retain::GpuOnly`
/// the full chunks have dropped their CPU copies, so the rewrite of the
/// two rows before an append goes to the GPU alone.
#[test]
fn appends_draw_the_curve_of_all_rows_at_once() {
    let rows = wavy();
    let (xd, yd) = ((-0.5, 20.5), (0.0, 10.0));
    for retain in [Retain::Auto, Retain::GpuOnly] {
        let o = Opts {
            chunk: Some(4),
            retain,
            ..Opts::default()
        };
        let whole = Fixture::new(&rows, xd, yd, o).image();
        let mut f = Fixture::new(&rows[..9], xd, yd, o);
        for batch in rows[9..].chunks(3) {
            f.append(batch);
            f.image();
        }
        let appended = f.image();
        let n = differing(&appended, &whole, |_, _| true);
        if n > 8 {
            appended.save("/tmp/gup551/appended.png").ok();
            whole.save("/tmp/gup551/whole.png").ok();
        }
        assert!(
            n <= 8,
            "{retain:?}: {n} pixels differ from all rows at once"
        );
    }
}

/// AC3: CPU and GPU picks on the curve report the row nearer in x, as for
/// straight lines: on the curve, and 1 px off it, away from the rows and
/// the midpoints. A point well off the curve picks nothing.
#[test]
fn picks_report_the_row_nearer_in_x() {
    let rows = monthly();
    let mut f = Fixture::new(&rows, (0.5, 12.5), (2.0, 11.0), Opts::default());
    let e = f.expected(&rows);
    let pts = e.rows();
    let mut checked = 0;
    for i in 0..pts.len() - 1 {
        let mid = (pts[i].0 + pts[i + 1].0) / 2.0;
        for k in 1..20 {
            let (x, y) = e.at(i, f64::from(k) / 20.0);
            for off in [0.0, 1.0] {
                let q = (x, y + off);
                let clear = 1.5 + 3.0 + 2.0;
                if dist(q, pts[i], pts[i]) < clear
                    || dist(q, pts[i + 1], pts[i + 1]) < clear
                    || (q.0 - mid).abs() < 1.0
                {
                    continue;
                }
                let want = Some(RowId {
                    layer: 0,
                    row: if q.0 < mid { i } else { i + 1 },
                });
                let at = Pt::new(q.0 as f32, q.1 as f32);
                assert_eq!(f.plot.pick_row(at), want, "CPU at {q:?}, segment {i}");
                assert_eq!(
                    f.plot.pick_row_gpu(&f.cx, at).unwrap(),
                    want,
                    "GPU at {q:?}, segment {i}"
                );
                checked += 1;
            }
        }
    }
    assert!(checked > 200, "only {checked} picks");
    // Well below the curve: nothing.
    let at = Pt::new(pts[6].0 as f32, (pts[6].1 + 40.0) as f32);
    assert_eq!(f.plot.pick_row(at), None);
    assert_eq!(f.plot.pick_row_gpu(&f.cx, at).unwrap(), None);
}

/// The numbers in an SVG path's data.
fn path_numbers(d: &str) -> Vec<f64> {
    d.split([' ', ',', 'M', 'C'])
        .filter(|s| !s.is_empty())
        .map(|s| s.parse().unwrap())
        .collect()
}

/// AC3: `SvgTarget` writes a 5-row monotone line as one `<path>` of four
/// cubic Bezier commands with the curve's control points.
#[test]
fn svg_writes_cubic_bezier_commands() {
    let rows = vec![(0.0, 1.0), (1.0, 4.0), (2.0, 3.0), (3.0, 8.0), (4.0, 9.0)];
    let mut f = Fixture::new(&rows, (-0.5, 4.5), (0.0, 10.0), Opts::default());
    let resolved = f.plot.resolve(&f.cx, W as f32, H as f32).unwrap();
    let mut target = SvgTarget::new();
    target.render(&resolved.scene).unwrap();
    let svg = target.svg().to_owned();
    assert_eq!(svg.matches("<polyline ").count(), 0, "{svg}");
    let paths: Vec<&str> = svg
        .split("<path d=\"")
        .skip(1)
        .filter_map(|s| s.split('"').next())
        .filter(|d| d.contains(" C"))
        .collect();
    assert_eq!(paths.len(), 1, "{svg}");
    let d = paths[0];
    assert_eq!(d.matches(" C").count(), 4, "{d}");
    let e = f.expected(&rows);
    let want: Vec<f64> = std::iter::once(e.segments[0][0])
        .chain(e.segments.iter().flat_map(|s| [s[1], s[2], s[3]]))
        .flat_map(|p| [p.0, p.1])
        .collect();
    let got = path_numbers(d);
    assert_eq!(got.len(), want.len(), "{d}");
    for (g, w) in got.iter().zip(&want) {
        assert!((g - w).abs() <= 0.01, "{d}: {g} against {w}");
    }
}

/// AC3: twelve noisy monthly values drawn `MonotoneX` over their points,
/// beside the same data `Linear` in a lighter layer, against its golden,
/// and its SVG against the GPU render.
#[test]
fn line_monotone_golden() {
    let cx = Context::new_blocking().unwrap();
    let (w, h) = (640u32, 400u32);
    let rows = monthly();
    let blue = OKABE_ITO[3];
    let light = Color { a: 0.35, ..blue };
    let mut plot = Plot::new();
    plot.title("Monotone curve").legends(false);
    let x = plot.x(Linear::new());
    let y = plot.y(Linear::new());
    plot.add(Selection::<Row, Line>::new(rows.clone()))
        .x_with(x.encode(|r: &Row| r.0))
        .y_with(y.encode(|r: &Row| r.1))
        .width(Px(2.0))
        .color(light);
    plot.add(Selection::<Row, Line>::new(rows.clone()))
        .x_with(x.encode(|r: &Row| r.0))
        .y_with(y.encode(|r: &Row| r.1))
        .width(Px(2.5))
        .color(blue)
        .curve(Curve::MonotoneX);
    plot.add(Selection::<Row, Circle>::new(rows))
        .x_with(x.encode(|r: &Row| r.0))
        .y_with(y.encode(|r: &Row| r.1))
        .radius(Px(3.5))
        .fill(blue)
        .stroke(Color::WHITE)
        .stroke_width(Px(1.0));
    let resolved = plot.resolve(&cx, w as f32, h as f32).unwrap();
    let image = ImageTarget::new(&cx, w, h)
        .unwrap()
        .render_blocking(&cx, &resolved.scene)
        .unwrap();
    let vr = RgbaImage::new(image.width(), image.height(), image.as_raw().clone()).unwrap();
    let meta = layout_metadata(&resolved.layout).with_expected_color("MonotoneX", rgba8(blue));
    harness()
        .run("gup_core/line_monotone", Ok((vr.clone(), meta.clone())))
        .assert_ok();
    let svg = svg_marks::assert_matches("gup_core/line_monotone", &resolved.scene, &vr, &meta);
    assert_eq!(svg.matches("<polyline ").count(), 1, "the linear layer");
    assert_eq!(
        svg.matches(" C").count(),
        11,
        "the monotone layer's segments"
    );
}

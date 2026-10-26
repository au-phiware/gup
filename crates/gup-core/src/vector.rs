// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! A layer's marks on the CPU (GUP-467): what a vector target
//! ([`SvgTarget`](crate::SvgTarget)) writes for an
//! [`ItemKind::Marks`](crate::scene::ItemKind::Marks) item.
//!
//! A [`MarkBatch`](crate::scene::MarkBatch) carries a [`LayerVector`]: a
//! snapshot, taken when the layer is prepared, of what it takes to
//! evaluate the layer's rows again without the GPU. The snapshot shares
//! the column store's chunk bytes (nothing is copied; an append to the
//! last chunk copies it first) and holds a frozen copy of each column
//! encoding's function, so it evaluates positions, sizes and colours with
//! the CPU mirrors picking uses, and reads the same instances the GPU
//! draws: the batch's visible spans, in draw order. Nothing is evaluated
//! until a target asks.
//!
//! A layer has no snapshot (and a vector target writes a comment in its
//! place) when its mark is not one of the library's, when its [`Retain`]
//! policy drops the CPU columns, or when a channel's function has no CPU
//! mirror.
//!
//! [`Retain`]: crate::Retain

use crate::Color;
use crate::channel::{ConstValue, Role};
use crate::column::CpuColumns;
use crate::encoding::ColumnMirror;
use crate::geom::Point;
use crate::marks::Curve;
use crate::order::DrawEntry;
use crate::render::Span;
use std::sync::Arc;

/// The library mark a layer draws, with the geometry a vector target
/// writes for it.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum MarkKind {
    Circle,
    Rect,
    Segment,
    /// A line, its segments drawn as the curve's path.
    Line(Curve),
    Area,
}

/// The names of a monotone line's control channels in
/// [`CpuLayer::chans`]: the control values leaving each row and entering
/// the next, read through the y channel's mirror.
pub(crate) const CONTROLS: [&str; 2] = ["y_out", "y_in"];

/// What a batch carries for vector targets.
#[derive(Clone)]
pub(crate) enum LayerVector {
    /// The layer can be evaluated on the CPU.
    Cpu(Arc<CpuLayer>),
    /// It cannot, for this reason.
    Skipped(String),
}

impl std::fmt::Debug for LayerVector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LayerVector::Cpu(l) => write!(f, "Cpu({:?})", l.kind),
            LayerVector::Skipped(why) => write!(f, "Skipped({why})"),
        }
    }
}

/// How a channel gets its value.
pub(crate) enum VChan {
    /// The same for every row (a constant or a derived value, as it was
    /// when the layer was prepared).
    Const(ConstValue),
    /// Column `column` of the snapshot, through `mirror`.
    Column {
        column: usize,
        mirror: Arc<dyn ColumnMirror>,
    },
}

/// A frozen, CPU-evaluable layer.
pub(crate) struct CpuLayer {
    pub kind: MarkKind,
    pub cols: CpuColumns,
    pub entries: Vec<DrawEntry>,
    /// One per mark channel, in channel order.
    pub chans: Vec<(&'static str, Option<Role>, VChan)>,
    /// The theme's colour for a null in a numeric colour column.
    pub null_color: Color,
}

/// One mark, in pixels, as a vector target draws it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum VectorMark {
    Circle {
        c: [f32; 2],
        r: f32,
        /// The fill with `fill_opacity` multiplied into its alpha.
        fill: Color,
        stroke: Color,
        stroke_width: f32,
    },
    Rect {
        lo: [f32; 2],
        hi: [f32; 2],
        fill: Color,
        stroke: Color,
        stroke_width: f32,
    },
    Segment {
        a: [f32; 2],
        b: [f32; 2],
        width: f32,
        color: Color,
    },
    /// Consecutive segments of one width and colour.
    Polyline {
        points: Vec<Point>,
        width: f32,
        color: Color,
    },
    /// Consecutive cubic Bezier segments of one width and colour, from
    /// `start`: each segment's two control points, then its end.
    Cubic {
        start: Point,
        segments: Vec<[Point; 3]>,
        width: f32,
        color: Color,
    },
    /// Consecutive trapezoids of one fill: the top edge left to right, and
    /// the baseline under it.
    Area {
        top: Vec<Point>,
        base: Vec<Point>,
        /// The fill with `fill_opacity` multiplied into its alpha.
        fill: Color,
    },
}

/// A channel's values by stored row.
enum Vals {
    Const(f64),
    Col(Vec<f64>),
}

impl Vals {
    fn at(&self, row: usize) -> f64 {
        match self {
            Vals::Const(v) => *v,
            Vals::Col(v) => v[row],
        }
    }
}

enum Colors {
    Const(Color),
    Col(Vec<Color>),
}

impl Colors {
    fn at(&self, row: usize) -> Color {
        match self {
            Colors::Const(c) => *c,
            Colors::Col(v) => v[row],
        }
    }
}

fn px(v: ConstValue) -> f64 {
    match v {
        ConstValue::F32(v) => f64::from(v),
        ConstValue::Vec4(_) => f64::NAN,
    }
}

impl CpuLayer {
    fn chan(&self, name: &str) -> &(&'static str, Option<Role>, VChan) {
        self.chans
            .iter()
            .find(|c| c.0 == name)
            .unwrap_or_else(|| panic!("a library mark has a `{name}` channel"))
    }

    /// Channel `name`'s numbers by stored row; rows of chunks `needed`
    /// rejects are NaN.
    fn numbers(&self, name: &str, needed: &[bool]) -> Result<Vals, String> {
        match &self.chan(name).2 {
            VChan::Const(v) => Ok(Vals::Const(px(*v))),
            VChan::Column { column, mirror } => {
                let raw = self.cols.values_where(*column, |k| needed[k]);
                let mut out = Vec::with_capacity(raw.len());
                for v in raw {
                    out.push(if v.is_nan() {
                        f64::NAN
                    } else {
                        mirror.number(v).ok_or_else(|| {
                            format!("channel `{name}` has no CPU mirror for its encoding")
                        })?
                    });
                }
                Ok(Vals::Col(out))
            }
        }
    }

    fn colors(&self, name: &str, needed: &[bool]) -> Result<Colors, String> {
        match &self.chan(name).2 {
            VChan::Const(ConstValue::Vec4(c)) => Ok(Colors::Const(Color::from(*c))),
            VChan::Const(ConstValue::F32(_)) => Ok(Colors::Const(Color::TRANSPARENT)),
            VChan::Column { column, mirror } => {
                let raw = self.cols.values_where(*column, |k| needed[k]);
                // A dictionary column's null code is the colour function's
                // to colour; a numeric column's null is the theme's.
                let codes = self.cols.is_codes(*column);
                let mut out = Vec::with_capacity(raw.len());
                for v in raw {
                    out.push(if v.is_nan() && !codes {
                        self.null_color
                    } else {
                        mirror.color(v).ok_or_else(|| {
                            format!("channel `{name}` has no CPU mirror for its colour function")
                        })?
                    });
                }
                Ok(Colors::Col(out))
            }
        }
    }

    /// Whether stored row `row` is hidden by a null: any column-encoded
    /// channel that is not a colour is NaN there (as the GPU hides it).
    fn valid_rows(&self, needed: &[bool], skip: &[&str]) -> Result<Vec<bool>, String> {
        let total = (0..self.cols.chunks())
            .map(|k| self.cols.rows_of(k))
            .sum::<usize>();
        let mut valid = vec![true; total];
        for (name, role, chan) in &self.chans {
            let VChan::Column { .. } = chan else { continue };
            if *role == Some(Role::Color) || skip.contains(name) {
                continue;
            }
            if let Vals::Col(v) = self.numbers(name, needed)? {
                for (ok, v) in valid.iter_mut().zip(v) {
                    *ok &= !v.is_nan();
                }
            }
        }
        Ok(valid)
    }

    /// The marks the batch draws: `spans` (its visible spans, in draw
    /// order), instance by instance, without the rows a null hides.
    /// `Err` says why the layer cannot be evaluated.
    pub(crate) fn marks(&self, spans: &[Span]) -> Result<Vec<VectorMark>, String> {
        let mut needed = vec![false; self.cols.chunks()];
        for s in spans {
            let e = &self.entries[s.chunk as usize];
            needed[e.this] = true;
            needed[e.next] = true;
        }
        let valid = self.valid_rows(&needed, &[])?;
        let n = valid.len();
        let row = |chunk: usize, local: u32| self.cols.row_base(chunk) as usize + local as usize;
        let f = |v: f64| v as f32;
        let mut out = Vec::new();
        match self.kind {
            MarkKind::Circle => {
                let (x, y, r) = (
                    self.numbers("x", &needed)?,
                    self.numbers("y", &needed)?,
                    self.numbers("radius", &needed)?,
                );
                let (fill, opacity) = (
                    self.colors("fill", &needed)?,
                    self.numbers("fill_opacity", &needed)?,
                );
                let (stroke, sw) = (
                    self.colors("stroke", &needed)?,
                    self.numbers("stroke_width", &needed)?,
                );
                for s in spans {
                    let e = &self.entries[s.chunk as usize];
                    for i in s.rows.clone() {
                        let d = row(e.this, e.first + i);
                        if d >= n || !valid[d] {
                            continue;
                        }
                        let (cx, cy, rad) = (f(x.at(d)), f(y.at(d)), f(r.at(d)));
                        if !(cx.is_finite() && cy.is_finite() && rad.is_finite() && rad >= 0.0) {
                            continue;
                        }
                        out.push(VectorMark::Circle {
                            c: [cx, cy],
                            r: rad,
                            fill: with_opacity(fill.at(d), opacity.at(d)),
                            stroke: stroke.at(d),
                            stroke_width: f(sw.at(d)),
                        });
                    }
                }
            }
            MarkKind::Rect => {
                let c = |name| self.numbers(name, &needed);
                let (x, y, w, h, xe, ye) = (
                    c("x")?,
                    c("y")?,
                    c("width")?,
                    c("height")?,
                    c("x_end")?,
                    c("y_end")?,
                );
                let (fill, opacity) = (
                    self.colors("fill", &needed)?,
                    self.numbers("fill_opacity", &needed)?,
                );
                let (stroke, sw) = (
                    self.colors("stroke", &needed)?,
                    self.numbers("stroke_width", &needed)?,
                );
                // A non-zero extent centres the box; zero spans to the far
                // edge, in either order.
                let span = |centre: f32, extent: f32, edge: f32| {
                    if extent == 0.0 {
                        (centre.min(edge), centre.max(edge))
                    } else {
                        let h = extent.abs() / 2.0;
                        (centre - h, centre + h)
                    }
                };
                for s in spans {
                    let e = &self.entries[s.chunk as usize];
                    for i in s.rows.clone() {
                        let d = row(e.this, e.first + i);
                        if d >= n || !valid[d] {
                            continue;
                        }
                        let v = [&x, &y, &w, &h, &xe, &ye].map(|c| f(c.at(d)));
                        if !v.iter().all(|v| v.is_finite()) {
                            continue;
                        }
                        let (x0, x1) = span(v[0], v[2], v[4]);
                        let (y0, y1) = span(v[1], v[3], v[5]);
                        out.push(VectorMark::Rect {
                            lo: [x0, y0],
                            hi: [x1, y1],
                            fill: with_opacity(fill.at(d), opacity.at(d)),
                            stroke: stroke.at(d),
                            stroke_width: f(sw.at(d)),
                        });
                    }
                }
            }
            MarkKind::Segment => {
                let c = |name| self.numbers(name, &needed);
                let (x, y, xe, ye, w) = (c("x")?, c("y")?, c("x_end")?, c("y_end")?, c("width")?);
                let color = self.colors("color", &needed)?;
                for s in spans {
                    let e = &self.entries[s.chunk as usize];
                    for i in s.rows.clone() {
                        let d = row(e.this, e.first + i);
                        if d >= n || !valid[d] {
                            continue;
                        }
                        let v = [&x, &y, &xe, &ye, &w].map(|c| f(c.at(d)));
                        if v.iter().all(|v| v.is_finite()) && v[4] >= 0.0 {
                            out.push(VectorMark::Segment {
                                a: [v[0], v[1]],
                                b: [v[2], v[3]],
                                width: v[4],
                                color: color.at(d),
                            });
                        }
                    }
                }
            }
            MarkKind::Line(Curve::MonotoneX) => {
                let (x, y, w) = (
                    self.numbers("x", &needed)?,
                    self.numbers("y", &needed)?,
                    self.numbers("width", &needed)?,
                );
                let (c1, c2) = (
                    self.numbers(CONTROLS[0], &needed)?,
                    self.numbers(CONTROLS[1], &needed)?,
                );
                let color = self.colors("color", &needed)?;
                // The open run: its start, its segments, and their width
                // and colour (taken from each segment's first row).
                type Run = (Point, Vec<[Point; 3]>, f32, Color);
                let mut run: Option<Run> = None;
                let flush = |run: &mut Option<Run>, out: &mut Vec<VectorMark>| {
                    if let Some((start, segments, width, color)) = run.take() {
                        out.push(VectorMark::Cubic {
                            start,
                            segments,
                            width,
                            color,
                        });
                    }
                };
                for s in spans {
                    let e = &self.entries[s.chunk as usize];
                    for i in s.rows.clone() {
                        let (d, nx) = (row(e.this, e.first + i), row(e.next, e.next_first + i));
                        let ok = d < n && nx < n && valid[d] && valid[nx];
                        let p = [
                            f(x.at(d)),
                            f(y.at(d)),
                            f(x.at(nx)),
                            f(y.at(nx)),
                            f(w.at(d)),
                            f(c1.at(d)),
                            f(c2.at(d)),
                        ];
                        if !(ok && p.iter().all(|v| v.is_finite()) && p[4] >= 0.0) {
                            flush(&mut run, &mut out);
                            continue;
                        }
                        let q = crate::monotone::control_points(
                            [p[0], p[1]],
                            [p[2], p[3]],
                            [p[5], p[6]],
                        );
                        let [a, k1, k2, b] = q.map(|q| Point::new(q[0], q[1]));
                        let col = color.at(d);
                        match &mut run {
                            Some((start, segments, width, c))
                                if segments.last().map_or(*start, |s| s[2]) == a
                                    && *width == p[4]
                                    && *c == col =>
                            {
                                segments.push([k1, k2, b]);
                            }
                            _ => {
                                flush(&mut run, &mut out);
                                run = Some((a, vec![[k1, k2, b]], p[4], col));
                            }
                        }
                    }
                }
                flush(&mut run, &mut out);
            }
            MarkKind::Line(curve) => {
                let (x, y, w) = (
                    self.numbers("x", &needed)?,
                    self.numbers("y", &needed)?,
                    self.numbers("width", &needed)?,
                );
                let color = self.colors("color", &needed)?;
                // The open run: its points, and the width and colour of its
                // segments (taken from each segment's first row).
                let mut run: Option<(Vec<Point>, f32, Color)> = None;
                let flush = |run: &mut Option<(Vec<Point>, f32, Color)>,
                             out: &mut Vec<VectorMark>| {
                    if let Some((points, width, color)) = run.take() {
                        out.push(VectorMark::Polyline {
                            points,
                            width,
                            color,
                        });
                    }
                };
                for s in spans {
                    let e = &self.entries[s.chunk as usize];
                    for i in s.rows.clone() {
                        let (d, nx) = (row(e.this, e.first + i), row(e.next, e.next_first + i));
                        let ok = d < n && nx < n && valid[d] && valid[nx];
                        let p = [f(x.at(d)), f(y.at(d)), f(x.at(nx)), f(y.at(nx)), f(w.at(d))];
                        if !(ok && p.iter().all(|v| v.is_finite()) && p[4] >= 0.0) {
                            flush(&mut run, &mut out);
                            continue;
                        }
                        let a = Point::new(p[0], p[1]);
                        // The segment's path after `a`: `b`, or a step's
                        // corners then `b`.
                        let (path, k) = curve.path([p[0], p[1]], [p[2], p[3]], [0.0; 2]);
                        let rest = path[1..k].iter().map(|q| Point::new(q[0], q[1]));
                        let col = color.at(d);
                        match &mut run {
                            Some((points, width, c))
                                if points.last() == Some(&a) && *width == p[4] && *c == col =>
                            {
                                points.extend(rest);
                            }
                            _ => {
                                flush(&mut run, &mut out);
                                run = Some((std::iter::once(a).chain(rest).collect(), p[4], col));
                            }
                        }
                    }
                }
                flush(&mut run, &mut out);
            }
            MarkKind::Area => {
                let c = |name| self.numbers(name, &needed);
                let (x, y, ye) = (c("x")?, c("y")?, c("y_end")?);
                let (fill, opacity) = (
                    self.colors("fill", &needed)?,
                    self.numbers("fill_opacity", &needed)?,
                );
                // The open run: its top edge and baseline, and its fill
                // (taken from each segment's first row).
                let mut run: Option<(Vec<Point>, Vec<Point>, Color)> = None;
                let flush = |run: &mut Option<(Vec<Point>, Vec<Point>, Color)>,
                             out: &mut Vec<VectorMark>| {
                    if let Some((top, base, fill)) = run.take() {
                        out.push(VectorMark::Area { top, base, fill });
                    }
                };
                for s in spans {
                    let e = &self.entries[s.chunk as usize];
                    for i in s.rows.clone() {
                        let (d, nx) = (row(e.this, e.first + i), row(e.next, e.next_first + i));
                        let ok = d < n && nx < n && valid[d] && valid[nx];
                        let p = [
                            f(x.at(d)),
                            f(y.at(d)),
                            f(ye.at(d)),
                            f(x.at(nx)),
                            f(y.at(nx)),
                            f(ye.at(nx)),
                        ];
                        if !(ok && p.iter().all(|v| v.is_finite())) {
                            flush(&mut run, &mut out);
                            continue;
                        }
                        let (a, a0) = (Point::new(p[0], p[1]), Point::new(p[0], p[2]));
                        let (b, b0) = (Point::new(p[3], p[4]), Point::new(p[3], p[5]));
                        let col = with_opacity(fill.at(d), opacity.at(d));
                        match &mut run {
                            Some((top, base, c))
                                if top.last() == Some(&a)
                                    && base.last() == Some(&a0)
                                    && *c == col =>
                            {
                                top.push(b);
                                base.push(b0);
                            }
                            _ => {
                                flush(&mut run, &mut out);
                                run = Some((vec![a, b], vec![a0, b0], col));
                            }
                        }
                    }
                }
                flush(&mut run, &mut out);
            }
        }
        Ok(out)
    }
}

/// `c` with its alpha multiplied by `opacity` (a NaN or missing opacity
/// leaves it as it was).
fn with_opacity(c: Color, opacity: f64) -> Color {
    Color {
        a: c.a * (opacity as f32).clamp(0.0, 1.0),
        ..c
    }
}

// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! CPU picking (RFC-001 §9): which row is drawn under a point.
//!
//! A plot builds a [`PickIndex`] lazily from every layer's retained
//! columns: one [`PickItem`] per drawn mark (per segment, for a polyline),
//! in a uniform grid of pixel cells. A query hit-tests the items of one
//! cell with the mark's signed distance and takes the topmost.

use crate::channel::{HitShape, Px};
use crate::geom::{Point, Rect};

/// A row of a plot layer: `layer` is the index in `Plot::add` order, `row`
/// the index into the rows the `Selection` was given (appended rows
/// continue it).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RowId {
    /// Index of the layer, in `Plot::add` order.
    pub layer: usize,
    /// Index into the rows the layer's `Selection` was given.
    pub row: usize,
}

/// How far outside a mark a point still picks it.
pub const PICK_TOLERANCE: Px = Px(3.0);

/// Side of a grid cell, in logical pixels.
const CELL: f32 = 32.0;

/// The geometry of one drawn mark, in logical pixels.
#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) enum Shape {
    Disc {
        c: [f32; 2],
        r: f32,
    },
    Box {
        lo: [f32; 2],
        hi: [f32; 2],
    },
    /// Square-capped stroke from `a` to `b`, half-width `h`.
    Segment {
        a: [f32; 2],
        b: [f32; 2],
        h: f32,
    },
    /// Capsule from the row's point `a` to the next row's `b`.
    Capsule {
        a: [f32; 2],
        b: [f32; 2],
        h: f32,
    },
}

fn hypot(x: f32, y: f32) -> f32 {
    x.hypot(y)
}

/// Signed distance from `p` to the aligned box of centre `c` and
/// half-extents `half`.
fn box_sd(p: [f32; 2], c: [f32; 2], half: [f32; 2]) -> f32 {
    let q = [(p[0] - c[0]).abs() - half[0], (p[1] - c[1]).abs() - half[1]];
    hypot(q[0].max(0.0), q[1].max(0.0)) + q[0].max(q[1]).min(0.0)
}

/// Distance from `p` to the segment `a`–`b`.
fn segment_distance(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len2 = dx * dx + dy * dy;
    let t = if len2 > 0.0 {
        (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    hypot(p[0] - (a[0] + t * dx), p[1] - (a[1] + t * dy))
}

impl Shape {
    /// The signed distance from `p` to the mark (≤ 0 inside).
    pub(crate) fn distance(&self, p: [f32; 2]) -> f32 {
        match *self {
            Shape::Disc { c, r } => hypot(p[0] - c[0], p[1] - c[1]) - r,
            Shape::Box { lo, hi } => box_sd(
                p,
                [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0],
                [(hi[0] - lo[0]) / 2.0, (hi[1] - lo[1]) / 2.0],
            ),
            Shape::Segment { a, b, h } => {
                let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
                let len = hypot(dx, dy);
                let mid = [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
                if len == 0.0 {
                    return box_sd(p, mid, [h, h]);
                }
                let (ux, uy) = (dx / len, dy / len);
                let (rx, ry) = (p[0] - mid[0], p[1] - mid[1]);
                let (u, v) = (rx * ux + ry * uy, -rx * uy + ry * ux);
                let q = [u.abs() - (len / 2.0 + h), v.abs() - h];
                hypot(q[0].max(0.0), q[1].max(0.0)) + q[0].max(q[1]).min(0.0)
            }
            Shape::Capsule { a, b, h } => segment_distance(p, a, b) - h,
        }
    }

    /// Where a tooltip for the row picked at `p` attaches.
    fn anchor(&self, p: [f32; 2], row: RowId) -> (RowId, Point) {
        let at = |q: [f32; 2]| Point::new(q[0], q[1]);
        match *self {
            Shape::Disc { c, .. } => (row, at(c)),
            Shape::Box { lo, hi } => (row, at([(lo[0] + hi[0]) / 2.0, lo[1]])),
            Shape::Segment { a, b, .. } => (row, at([(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0])),
            Shape::Capsule { a, b, .. } => {
                let (da, db) = (
                    hypot(p[0] - a[0], p[1] - a[1]),
                    hypot(p[0] - b[0], p[1] - b[1]),
                );
                if da <= db {
                    (row, at(a))
                } else {
                    (
                        RowId {
                            row: row.row + 1,
                            ..row
                        },
                        at(b),
                    )
                }
            }
        }
    }

    /// The bounding box, grown by `by`.
    fn bounds(&self, by: f32) -> ([f32; 2], [f32; 2]) {
        let (lo, hi) = match *self {
            Shape::Disc { c, r } => ([c[0] - r, c[1] - r], [c[0] + r, c[1] + r]),
            Shape::Box { lo, hi } => (lo, hi),
            Shape::Segment { a, b, h } => {
                let g = h * std::f32::consts::SQRT_2;
                (
                    [a[0].min(b[0]) - g, a[1].min(b[1]) - g],
                    [a[0].max(b[0]) + g, a[1].max(b[1]) + g],
                )
            }
            Shape::Capsule { a, b, h } => (
                [a[0].min(b[0]) - h, a[1].min(b[1]) - h],
                [a[0].max(b[0]) + h, a[1].max(b[1]) + h],
            ),
        };
        ([lo[0] - by, lo[1] - by], [hi[0] + by, hi[1] + by])
    }
}

/// One drawn mark of one layer.
#[derive(Copy, Clone, Debug)]
pub(crate) struct PickItem {
    /// The row (a capsule's first row).
    pub row: RowId,
    /// The layer's z.
    pub z: i32,
    /// Position in the layer's draw order.
    pub rank: u64,
    pub shape: Shape,
}

impl PickItem {
    fn key(&self) -> (i32, usize, u64) {
        (self.z, self.row.layer, self.rank)
    }
}

/// A channel's values in pixels, by data row.
pub(crate) enum Chan {
    /// Not read by the shape.
    Unused,
    Const(f64),
    /// One value per data row; NaN for a null.
    Col(Vec<f64>),
}

impl Chan {
    fn at(&self, row: usize) -> f32 {
        (match self {
            Chan::Unused => f64::NAN,
            Chan::Const(v) => *v,
            Chan::Col(v) => v[row],
        }) as f32
    }
}

/// The channels a hit shape reads.
pub(crate) struct Chans {
    pub x: Chan,
    pub y: Chan,
    pub radius: Chan,
    pub width: Chan,
    pub height: Chan,
    pub x_end: Chan,
    pub y_end: Chan,
}

/// The items of one layer of `slots.len()` stored rows. `slots` lists, per
/// stored row, its data row (counted from the layer's first kept row, the
/// index into `c`) and its rank in the draw order; ids add `first`, the
/// rows a window evicted.
pub(crate) fn build_items(
    hit: HitShape,
    layer: usize,
    z: i32,
    first: usize,
    slots: &[(usize, u64)],
    c: &Chans,
    out: &mut Vec<PickItem>,
) {
    let finite = |vs: &[f32]| vs.iter().all(|v| v.is_finite());
    let mut push = |row: usize, rank: u64, shape: Shape| {
        out.push(PickItem {
            row: RowId {
                layer,
                row: first + row,
            },
            z,
            rank,
            shape,
        });
    };
    match hit {
        HitShape::Unpickable => {}
        HitShape::Disc => {
            for &(d, rank) in slots {
                let (x, y, r) = (c.x.at(d), c.y.at(d), c.radius.at(d));
                if finite(&[x, y, r]) && r >= 0.0 {
                    push(d, rank, Shape::Disc { c: [x, y], r });
                }
            }
        }
        HitShape::AlignedBox => {
            let span = |centre: f32, extent: f32, edge: f32| {
                if extent != 0.0 {
                    let h = extent.abs() / 2.0;
                    (centre - h, centre + h)
                } else {
                    (centre.min(edge), centre.max(edge))
                }
            };
            for &(d, rank) in slots {
                let v = [
                    c.x.at(d),
                    c.y.at(d),
                    c.width.at(d),
                    c.height.at(d),
                    c.x_end.at(d),
                    c.y_end.at(d),
                ];
                if finite(&v) {
                    let (x0, x1) = span(v[0], v[2], v[4]);
                    let (y0, y1) = span(v[1], v[3], v[5]);
                    push(
                        d,
                        rank,
                        Shape::Box {
                            lo: [x0, y0],
                            hi: [x1, y1],
                        },
                    );
                }
            }
        }
        HitShape::Segment => {
            for &(d, rank) in slots {
                let v = [
                    c.x.at(d),
                    c.y.at(d),
                    c.x_end.at(d),
                    c.y_end.at(d),
                    c.width.at(d),
                ];
                if finite(&v) && v[4] >= 0.0 {
                    push(
                        d,
                        rank,
                        Shape::Segment {
                            a: [v[0], v[1]],
                            b: [v[2], v[3]],
                            h: v[4] / 2.0,
                        },
                    );
                }
            }
        }
        HitShape::Polyline => {
            for pair in slots.windows(2) {
                let ((d, rank), (e, _)) = (pair[0], pair[1]);
                let v = [c.x.at(d), c.y.at(d), c.x.at(e), c.y.at(e), c.width.at(d)];
                if finite(&v) && v[4] >= 0.0 {
                    push(
                        d,
                        rank,
                        Shape::Capsule {
                            a: [v[0], v[1]],
                            b: [v[2], v[3]],
                            h: v[4] / 2.0,
                        },
                    );
                }
            }
        }
    }
}

/// The result of a pick.
#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) struct Hit {
    pub row: RowId,
    pub anchor: Point,
}

/// A uniform grid of 32 × 32 px cells over the plot rectangle.
#[derive(Debug)]
pub(crate) struct PickIndex {
    plot: Rect,
    cols: usize,
    rows: usize,
    cells: Vec<Vec<u32>>,
    items: Vec<PickItem>,
}

impl PickIndex {
    /// Number of items indexed.
    pub(crate) fn len(&self) -> usize {
        self.items.len()
    }

    pub(crate) fn new(plot: Rect, items: Vec<PickItem>) -> Self {
        let cols = ((plot.width / CELL).ceil() as usize).max(1);
        let rows = ((plot.height / CELL).ceil() as usize).max(1);
        let mut index = Self {
            plot,
            cols,
            rows,
            cells: vec![Vec::new(); cols * rows],
            items,
        };
        let tol = PICK_TOLERANCE.0;
        for (i, item) in index.items.iter().enumerate() {
            let (lo, hi) = item.shape.bounds(tol);
            let (Some(c0), Some(c1)) = (
                index.cell_range(lo[0] - plot.x, hi[0] - plot.x, index.cols),
                index.cell_range(lo[1] - plot.y, hi[1] - plot.y, index.rows),
            ) else {
                continue;
            };
            for cy in c1.0..=c1.1 {
                for cx in c0.0..=c0.1 {
                    index.cells[cy * index.cols + cx].push(i as u32);
                }
            }
        }
        index
    }

    /// The cells `lo..=hi` (relative to the plot) overlap, if any.
    fn cell_range(&self, lo: f32, hi: f32, n: usize) -> Option<(usize, usize)> {
        let extent = n as f32 * CELL;
        if !(lo.is_finite() && hi.is_finite()) || hi < 0.0 || lo > extent {
            return None;
        }
        let cell = |v: f32| ((v / CELL).floor().max(0.0) as usize).min(n - 1);
        Some((cell(lo), cell(hi)))
    }

    pub(crate) fn rect(&self) -> Rect {
        self.plot
    }

    /// The topmost mark under `at`: among marks containing it, the topmost;
    /// else the nearest within tolerance (ties to the topmost).
    pub(crate) fn query(&self, at: Point) -> Option<Hit> {
        let p = self.plot;
        if !(at.x >= p.x && at.x <= p.x + p.width && at.y >= p.y && at.y <= p.y + p.height) {
            return None;
        }
        let cx = (((at.x - p.x) / CELL) as usize).min(self.cols - 1);
        let cy = (((at.y - p.y) / CELL) as usize).min(self.rows - 1);
        let q = [at.x, at.y];
        let tol = PICK_TOLERANCE.0;
        let mut best: Option<(f32, &PickItem)> = None;
        for &i in &self.cells[cy * self.cols + cx] {
            let item = &self.items[i as usize];
            let d = item.shape.distance(q);
            if d > tol {
                continue;
            }
            let better = match best {
                None => true,
                Some((bd, b)) => match (d <= 0.0, bd <= 0.0) {
                    (true, true) => item.key() > b.key(),
                    (true, false) => true,
                    (false, true) => false,
                    (false, false) => d < bd || (d == bd && item.key() > b.key()),
                },
            };
            if better {
                best = Some((d, item));
            }
        }
        let (_, item) = best?;
        let (row, anchor) = item.shape.anchor(q, item.row);
        Some(Hit { row, anchor })
    }

    /// The rows whose mark position lies inside `r` (edges included),
    /// sorted by (layer, row), without duplicates. The position is a disc's
    /// or a box's centre, a segment's midpoint, or a line's vertex.
    pub(crate) fn rows_in(&self, r: Rect) -> Vec<RowId> {
        let p = self.plot;
        let (Some(c0), Some(c1)) = (
            self.cell_range(r.x - p.x, r.x + r.width - p.x, self.cols),
            self.cell_range(r.y - p.y, r.y + r.height - p.y, self.rows),
        ) else {
            return Vec::new();
        };
        let inside = |q: [f32; 2]| {
            q[0] >= r.x && q[0] <= r.x + r.width && q[1] >= r.y && q[1] <= r.y + r.height
        };
        let mut out = Vec::new();
        for cy in c1.0..=c1.1 {
            for cx in c0.0..=c0.1 {
                for &i in &self.cells[cy * self.cols + cx] {
                    let item = &self.items[i as usize];
                    match item.shape {
                        Shape::Disc { c, .. } => {
                            if inside(c) {
                                out.push(item.row);
                            }
                        }
                        Shape::Box { lo, hi } => {
                            if inside([(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0]) {
                                out.push(item.row);
                            }
                        }
                        Shape::Segment { a, b, .. } => {
                            if inside([(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0]) {
                                out.push(item.row);
                            }
                        }
                        Shape::Capsule { a, b, .. } => {
                            if inside(a) {
                                out.push(item.row);
                            }
                            if inside(b) {
                                out.push(RowId {
                                    row: item.row.row + 1,
                                    ..item.row
                                });
                            }
                        }
                    }
                }
            }
        }
        out.sort_unstable();
        out.dedup();
        out
    }

    /// Every item within `reach` of `at` (tests): its signed distance, the
    /// row a pick of it reports, and for a capsule how much nearer one end
    /// is than the other.
    #[cfg(test)]
    pub(crate) fn hits_at(&self, at: Point, reach: f32) -> Vec<(f32, RowId, Option<f32>)> {
        let q = [at.x, at.y];
        self.items
            .iter()
            .filter_map(|item| {
                let d = item.shape.distance(q);
                (d <= reach).then(|| {
                    let (row, _) = item.shape.anchor(q, item.row);
                    let gap = match item.shape {
                        Shape::Capsule { a, b, .. } => Some(
                            (hypot(q[0] - a[0], q[1] - a[1]) - hypot(q[0] - b[0], q[1] - b[1]))
                                .abs(),
                        ),
                        _ => None,
                    };
                    (d, row, gap)
                })
            })
            .collect()
    }

    /// Where a tooltip for `row` attaches, if it is drawn.
    pub(crate) fn anchor(&self, row: RowId) -> Option<Point> {
        self.items.iter().find_map(|item| {
            if item.row.layer != row.layer {
                return None;
            }
            match item.shape {
                Shape::Capsule { a, b, .. } => {
                    let q = if item.row.row == row.row {
                        a
                    } else if item.row.row + 1 == row.row {
                        b
                    } else {
                        return None;
                    };
                    Some(Point::new(q[0], q[1]))
                }
                _ if item.row.row == row.row => Some(item.shape.anchor([0.0; 2], item.row).1),
                _ => None,
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disc_distance_is_signed() {
        let s = Shape::Disc {
            c: [10.0, 10.0],
            r: 4.0,
        };
        assert_eq!(s.distance([10.0, 10.0]), -4.0);
        assert!((s.distance([13.5, 10.0]) + 0.5).abs() < 1e-6);
        assert!((s.distance([17.0, 10.0]) - 3.0).abs() < 1e-6);
    }

    #[test]
    fn box_distance_is_zero_on_the_edge() {
        let s = Shape::Box {
            lo: [0.0, 0.0],
            hi: [10.0, 4.0],
        };
        assert_eq!(s.distance([5.0, 2.0]), -2.0);
        assert_eq!(s.distance([10.0, 2.0]), 0.0);
        assert_eq!(s.distance([14.0, 2.0]), 4.0);
        assert!((s.distance([13.0, 8.0]) - 5.0).abs() < 1e-6, "corner");
    }

    #[test]
    fn segment_has_square_caps() {
        let s = Shape::Segment {
            a: [0.0, 0.0],
            b: [10.0, 0.0],
            h: 2.0,
        };
        assert!(s.distance([11.5, 0.0]) < 0.0, "inside the cap");
        assert!((s.distance([12.0, 0.0])).abs() < 1e-6);
        assert!((s.distance([5.0, 5.5]) - 3.5).abs() < 1e-6);
        // Diagonal: the rectangle's corner, not a round cap.
        let d = Shape::Segment {
            a: [0.0, 0.0],
            b: [10.0, 10.0],
            h: 1.0,
        };
        let corner = 10.0 + std::f32::consts::SQRT_2;
        assert!(
            d.distance([10.0, corner]).abs() < 1e-5,
            "{}",
            d.distance([10.0, corner])
        );
        assert!(
            d.distance([corner, corner]) > 0.5,
            "not a round or square-extended box"
        );
        // Zero length: an aligned square of side 2h.
        let z = Shape::Segment {
            a: [5.0, 5.0],
            b: [5.0, 5.0],
            h: 2.0,
        };
        assert!(z.distance([6.9, 6.9]) < 0.0 && z.distance([7.1, 5.0]) > 0.0);
    }

    #[test]
    fn capsule_is_round_and_picks_the_nearer_endpoint() {
        let s = Shape::Capsule {
            a: [0.0, 0.0],
            b: [10.0, 0.0],
            h: 1.0,
        };
        assert!((s.distance([5.0, 3.0]) - 2.0).abs() < 1e-6);
        assert!((s.distance([12.0, 0.0]) - 1.0).abs() < 1e-6);
        let row = RowId { layer: 0, row: 7 };
        assert_eq!(s.anchor([6.0, 0.0], row).0.row, 8);
        assert_eq!(s.anchor([5.0, 0.0], row).0.row, 7, "a tie is the first row");
    }

    fn disc(layer: usize, row: usize, z: i32, rank: u64, x: f32, r: f32) -> PickItem {
        PickItem {
            row: RowId { layer, row },
            z,
            rank,
            shape: Shape::Disc { c: [x, 20.0], r },
        }
    }

    #[test]
    fn query_prefers_containing_topmost_then_nearest() {
        let plot = Rect::new(10.0, 10.0, 100.0, 60.0);
        let items = vec![
            disc(0, 0, 0, 0, 40.0, 10.0),
            disc(0, 1, 0, 1, 50.0, 4.0),
            disc(1, 0, 0, 0, 80.0, 2.0),
        ];
        let index = PickIndex::new(plot, items);
        let row = |x: f32, y: f32| index.query(Point::new(x, y)).map(|h| h.row);
        assert_eq!(
            row(48.0, 20.0),
            Some(RowId { layer: 0, row: 1 }),
            "later row on top"
        );
        assert_eq!(row(36.0, 20.0), Some(RowId { layer: 0, row: 0 }));
        assert_eq!(row(30.0, 20.0), Some(RowId { layer: 0, row: 0 }));
        // Outside row 0 (edge 50), within tolerance of row 1 only.
        assert_eq!(row(57.0, 20.0), Some(RowId { layer: 0, row: 1 }));
        assert_eq!(row(80.0, 20.0), Some(RowId { layer: 1, row: 0 }));
        assert_eq!(row(95.0, 20.0), None);
        assert_eq!(row(5.0, 20.0), None, "outside the plot");
    }

    #[test]
    fn inside_a_lower_mark_beats_tolerance_of_the_topmost() {
        let plot = Rect::new(0.0, 0.0, 100.0, 60.0);
        let items = vec![disc(0, 0, 0, 0, 40.0, 12.0), disc(0, 1, 0, 1, 58.0, 4.0)];
        let index = PickIndex::new(plot, items);
        // 0.5 inside row 0, 2.5 from row 1's edge.
        assert_eq!(
            index.query(Point::new(51.5, 20.0)).map(|h| h.row.row),
            Some(0)
        );
    }

    #[test]
    fn higher_z_wins_over_a_later_layer() {
        let plot = Rect::new(0.0, 0.0, 100.0, 60.0);
        let items = vec![disc(0, 0, 5, 0, 30.0, 10.0), disc(1, 0, 0, 0, 30.0, 10.0)];
        let index = PickIndex::new(plot, items);
        assert_eq!(
            index.query(Point::new(30.0, 20.0)).map(|h| h.row.layer),
            Some(0)
        );
    }

    struct D {
        x: f64,
        y: f64,
        size: f64,
    }

    /// A size-encoded layer draws largest first; an appended batch sorts
    /// into the middle of that order, and picks still return data rows and
    /// still favour the row drawn later.
    #[test]
    fn append_into_a_size_ordered_layer_keeps_data_rows_and_draw_rank() {
        use crate::prelude::{
            Circle, Context, CpuMirror, EncodeFn, Linear, Plot, Pow, ScaleRef, Selection,
        };
        let cx = Context::new_blocking().unwrap();
        let mut plot = Plot::new();
        let x = plot.x(Linear::new().domain(0.0, 10.0));
        let y = plot.y(Linear::new().domain(0.0, 10.0));
        let size = ScaleRef::new(Pow::sqrt().domain(0.0, 10.0).range(Px(4.0), Px(12.0)));
        let d = |x, y, size| D { x, y, size };
        plot.add(Selection::<D, Circle>::new(vec![
            d(1.0, 2.0, 1.0),
            d(4.0, 2.0, 5.0),
            d(7.0, 2.0, 9.0),
        ]))
        .attr(Circle::X, x.encode(|r: &D| r.x))
        .attr(Circle::Y, y.encode(|r: &D| r.y))
        .attr(Circle::RADIUS, size.encode(|r: &D| r.size));
        plot.resolve(&cx, 480.0, 320.0).unwrap();
        plot.layer_mut::<D, Circle>()
            .unwrap()
            .append([d(9.0, 8.0, 7.0), d(7.2, 2.0, 1.0)])
            .unwrap();
        plot.resolve(&cx, 480.0, 320.0).unwrap();
        let rows = [(1.0, 2.0), (4.0, 2.0), (7.0, 2.0), (9.0, 8.0), (7.2, 2.0)];
        let centre = |i: usize| {
            Point::new(
                x.read().eval(rows[i].0) as f32,
                y.read().eval(rows[i].1) as f32,
            )
        };
        for (i, &(rx, _)) in rows.iter().enumerate() {
            let got = plot.pick_row(centre(i));
            assert_eq!(got, Some(RowId { layer: 0, row: i }), "row {i} at x {rx}");
        }
        // In the overlap of rows 2 and 4, the smaller, row 4, draws last.
        let between = Point::new((centre(2).x + centre(4).x) / 2.0 + 1.0, centre(2).y);
        assert_eq!(plot.pick_row(between).map(|r| r.row), Some(4));
        // Row 2's own side of the overlap.
        let left = Point::new(centre(2).x - 9.0, centre(2).y);
        assert_eq!(plot.pick_row(left).map(|r| r.row), Some(2));
    }
}

// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Marks: [`Circle`], [`Rect`], [`Segment`] and [`Line`] (RFC-001 S6).

use crate::channel::{Color, Px, Unit};
use crate::scale::OKABE_ITO;

/// A filled disc centred on `(X, Y)`, sized in logical pixels so it stays
/// round on any aspect ratio.
///
/// The `#[derive(Mark)]` generates the channel constants
/// ([`Circle::X`], [`Circle::Y`], [`Circle::RADIUS`], [`Circle::FILL`]), the
/// `Mark` impl and the [`CircleAttrs`] setters, which the prelude brings
/// into scope:
///
/// ```
/// use gup_core::prelude::*;
///
/// # fn main() -> gup_core::Result<()> {
/// struct Row {
///     x: f64,
///     y: f64,
/// }
/// let rows = vec![Row { x: 1.0, y: 2.0 }, Row { x: 3.0, y: 1.0 }];
///
/// let x = Linear::new().domain(0.0, 4.0);
/// let y = Linear::new().domain(0.0, 4.0);
/// let mut points = Selection::<Row, Circle>::new(rows);
/// points
///     .x_with(x.encode(|r: &Row| r.x))
///     .y_with(y.encode(|r: &Row| r.y))
///     .radius(Px(4.0))
///     .fill(Color::hex(0xd55e00));
/// # let _ = points;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, gup_core_macros::Mark)]
#[mark(module = "gup::marks::circle", geometry = Quad, hit = Disc, gpu_pick)]
pub struct Circle {
    /// Centre x (logical pixels).
    #[channel(role = X)]
    x: Px,
    /// Centre y (logical pixels, y down).
    #[channel(role = Y)]
    y: Px,
    /// Radius (logical pixels). Default 3.5.
    #[channel(role = Size, default = Px(3.5))]
    radius: Px,
    /// Fill colour. Default Okabe-Ito blue.
    #[channel(role = Color, default = Circle::DEFAULT_FILL)]
    fill: Color,
    /// Fill opacity, multiplied into the fill colour's alpha. Default 1.
    #[channel(default = Unit(1.0))]
    fill_opacity: Unit,
    /// Stroke colour, drawn inside the edge. Default transparent.
    #[channel(role = Color, default = Color::TRANSPARENT)]
    stroke: Color,
    /// Stroke width (logical pixels), inside the edge. Default 0.
    #[channel(default = Px(0.0))]
    stroke_width: Px,
}

impl Circle {
    /// The default fill (`#0072B2`).
    pub const DEFAULT_FILL: Color = OKABE_ITO[3];
}

/// A filled axis-aligned box in logical pixels, with analytically
/// antialiased edges.
///
/// On each axis a non-zero extent (`width`, `height`) gives a box centred on
/// `x` (`y`); a zero extent gives the box between the two edges `x` and `x_end`
/// (`y` and `y_end`), in either order. A bar's width comes from its band scale,
/// and its far edge is a constant derived from the value scale:
///
/// ```
/// use gup_core::prelude::*;
///
/// # fn main() -> gup_core::Result<()> {
/// struct Sale {
///     month: &'static str,
///     total: f64,
/// }
/// let sales = vec![Sale { month: "Jan", total: 3.0 }, Sale { month: "Feb", total: 5.0 }];
///
/// let x = ScaleRef::new(Band::new());
/// let y = ScaleRef::new(Linear::new().domain(0.0, 5.0));
/// let mut bars = Selection::<Sale, Rect>::new(sales);
/// bars.x_with(x.encode_key(|s: &Sale| s.month).domain(["Jan", "Feb"]))
///     .width(x.band_width_px())
///     .y_with(y.encode(|s: &Sale| s.total))
///     .y_end_with({
///         let y = y.clone();
///         Derived::new(move || Px(y.read().eval(0.0) as f32))
///     });
/// # let _ = bars;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, gup_core_macros::Mark)]
#[mark(module = "gup::marks::rect", geometry = Quad, hit = AlignedBox, gpu_pick)]
pub struct Rect {
    /// Centre x, or the first edge when `width` is 0 (logical pixels).
    #[channel(role = X)]
    x: Px,
    /// Centre y, or the first edge when `height` is 0 (logical pixels).
    #[channel(role = Y)]
    y: Px,
    /// Width (logical pixels); 0 means the span `x..x_end`.
    #[channel(role = Width, default = Px(0.0))]
    width: Px,
    /// Height (logical pixels); 0 means the span `y..y_end`.
    #[channel(role = Height, default = Px(0.0))]
    height: Px,
    /// Second x edge, used when `width` is 0.
    #[channel(role = X)]
    x_end: Px,
    /// Second y edge, used when `height` is 0.
    #[channel(role = Y)]
    y_end: Px,
    /// Fill colour. Default Okabe-Ito blue.
    #[channel(role = Color, default = Rect::DEFAULT_FILL)]
    fill: Color,
    /// Fill opacity, multiplied into the fill colour's alpha. Default 1.
    #[channel(default = Unit(1.0))]
    fill_opacity: Unit,
    /// Stroke colour, drawn inside the edge. Default transparent.
    #[channel(role = Color, default = Color::TRANSPARENT)]
    stroke: Color,
    /// Stroke width (logical pixels), inside the edge. Default 0.
    #[channel(default = Px(0.0))]
    stroke_width: Px,
}

impl Rect {
    /// The default fill (`#0072B2`).
    pub const DEFAULT_FILL: Color = OKABE_ITO[3];
}

/// A straight line from `(x, y)` to `(x_end, y_end)` in logical pixels, with a
/// real width and square caps (it extends `width / 2` past each end), edges
/// antialiased analytically.
///
/// A reference rule is a segment with constant endpoints. A zero-length segment
/// draws as a `width` square. Error bars:
///
/// ```
/// use gup_core::prelude::*;
///
/// # fn main() -> gup_core::Result<()> {
/// struct Obs {
///     year: f64,
///     lo: f64,
///     hi: f64,
/// }
/// let rows = vec![Obs { year: 2020.0, lo: 1.0, hi: 3.0 }, Obs { year: 2021.0, lo: 2.0, hi: 5.0 }];
///
/// let mut plot = Plot::new();
/// let (x, y) = (plot.x(Linear::new()), plot.y(Linear::new()));
/// plot.add(Selection::<Obs, Segment>::new(rows))
///     .attr(Segment::X, x.encode(|o: &Obs| o.year))
///     .attr(Segment::X_END, x.encode(|o: &Obs| o.year))
///     .attr(Segment::Y, y.encode(|o: &Obs| o.lo))
///     .attr(Segment::Y_END, y.encode(|o: &Obs| o.hi));
/// let cx = Context::new_blocking()?;
/// let _ = plot.resolve(&cx, 400.0, 300.0)?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, gup_core_macros::Mark)]
#[mark(module = "gup::marks::segment", geometry = Quad, hit = Segment, gpu_pick)]
pub struct Segment {
    /// Start x (logical pixels).
    #[channel(role = X)]
    x: Px,
    /// Start y (logical pixels).
    #[channel(role = Y)]
    y: Px,
    /// End x (logical pixels).
    #[channel(role = X)]
    x_end: Px,
    /// End y (logical pixels).
    #[channel(role = Y)]
    y_end: Px,
    /// Line width (logical pixels).
    #[channel(default = Px(1.0))]
    width: Px,
    /// Colour, its alpha the opacity. Default Okabe-Ito blue.
    #[channel(role = Color, default = Segment::DEFAULT_COLOR)]
    color: Color,
}

impl Segment {
    /// The default colour (`#0072B2`).
    pub const DEFAULT_COLOR: Color = OKABE_ITO[3];
}

/// A polyline through the rows in data order: one segment from each row to
/// the next, `N - 1` for `N` rows, in logical pixels, with analytically
/// antialiased edges.
///
/// Ends and joins are round: each segment is a capsule (every point within
/// `width / 2` of it), so consecutive segments overlap in a disc at their
/// shared row. An opaque line shows no seam there; a translucent one is
/// darker where segments meet, by the overlap blended twice. A row with a
/// null position breaks the line: neither segment touching it is drawn, so a
/// lone row between two nulls draws nothing. Width and colour come from a
/// segment's first row. Rows with a column split across buffers (chunks)
/// join as if they were one (RFC-001 "GUP-432 findings").
///
/// ```
/// use gup_core::prelude::*;
///
/// # fn main() -> gup_core::Result<()> {
/// let temps = vec![(0.0, 12.5), (1.0, 14.0), (2.0, f64::NAN), (3.0, 13.0), (4.0, 15.5)];
///
/// let mut plot = Plot::new();
/// let (x, y) = (plot.x(Linear::new()), plot.y(Linear::new()));
/// plot.add(Selection::<(f64, f64), Line>::new(temps))
///     .x_with(x.encode(|r: &(f64, f64)| r.0))
///     .y_with(y.encode(|r: &(f64, f64)| r.1))
///     .width(Px(2.0));
/// let cx = Context::new_blocking()?;
/// let _ = plot.resolve(&cx, 400.0, 300.0)?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, gup_core_macros::Mark)]
#[mark(module = "gup::marks::line", geometry = Polyline, hit = Polyline, gpu_pick)]
pub struct Line {
    /// x (logical pixels).
    #[channel(role = X)]
    x: Px,
    /// y (logical pixels).
    #[channel(role = Y)]
    y: Px,
    /// Line width (logical pixels). Default 1.5.
    #[channel(default = Px(1.5))]
    width: Px,
    /// Colour, its alpha the opacity. Default Okabe-Ito blue.
    #[channel(role = Color, default = Line::DEFAULT_COLOR)]
    color: Color,
}

impl Line {
    /// The default colour (`#0072B2`).
    pub const DEFAULT_COLOR: Color = OKABE_ITO[3];
}

/// How a [`Line`] runs from one row to the next: straight, in steps that
/// hold a value until the next row changes it, or along a smooth curve
/// that never overshoots the data ([`MonotoneX`](Curve::MonotoneX)).
///
/// A step is drawn exactly in pixel space from the two rows' positions,
/// under any scale: its horizontal pieces are level and its vertical
/// pieces plumb, with round corners `width` wide, like a straight line's
/// joins. A step, and a monotone curve, stays inside its two rows'
/// bounding box. Nulls break a stepped or curved line as they break a
/// straight one.
///
/// ```
/// use gup_core::prelude::*;
///
/// # fn main() -> gup_core::Result<()> {
/// // A price that holds between changes.
/// let prices = vec![(0.0, 10.0), (2.0, 12.5), (3.0, 11.0), (6.0, 14.0)];
///
/// let mut plot = Plot::new();
/// let (x, y) = (plot.x(Linear::new()), plot.y(Linear::new()));
/// plot.add(Selection::<(f64, f64), Line>::new(prices))
///     .x_with(x.encode(|r: &(f64, f64)| r.0))
///     .y_with(y.encode(|r: &(f64, f64)| r.1))
///     .curve(Curve::StepAfter);
/// let cx = Context::new_blocking()?;
/// let _ = plot.resolve(&cx, 400.0, 300.0)?;
/// # Ok(())
/// # }
/// ```
#[non_exhaustive]
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum Curve {
    /// A straight segment from each row to the next.
    #[default]
    Linear,
    /// Horizontal to the midpoint between rows, vertical, then horizontal.
    Step,
    /// Vertical at each row, then horizontal to the next row.
    StepBefore,
    /// Horizontal from each row to the next row's x, then vertical.
    StepAfter,
    /// A monotone cubic in x through every row; never overshoots.
    ///
    /// Each segment is a cubic Bezier from one row to the next, with its
    /// tangents chosen by Steffen's method (1990) over each run of rows
    /// whose x strictly increases: between two rows the curve rises or
    /// falls with the data and stays within the two rows' values, so it
    /// never invents a peak or a dip. A local extremum, or a flat stretch,
    /// has a level tangent. Rows must be sorted by x: a segment whose x
    /// does not increase is drawn straight, and starts a new run, as a
    /// null does. x and y must be numbers, not keys.
    ///
    /// The control values are computed on the CPU when the layer's
    /// columns are evaluated, and stored as two more columns read through
    /// the y scale, so the curve passes through every row under any y
    /// scale (on a log scale it stays within each segment's pixel range).
    /// Appending rows rewrites the two rows before the first new row, so
    /// the curve after an append is the curve of all the rows at once.
    /// The line draws 16 pieces per segment.
    ///
    /// ```
    /// use gup_core::prelude::*;
    ///
    /// # fn main() -> gup_core::Result<()> {
    /// let rainfall = vec![(1.0, 82.0), (2.0, 64.0), (3.0, 71.0), (4.0, 30.0), (5.0, 33.0)];
    ///
    /// let mut plot = Plot::new();
    /// let (x, y) = (plot.x(Linear::new()), plot.y(Linear::new()));
    /// plot.add(Selection::<(f64, f64), Line>::new(rainfall))
    ///     .x_with(x.encode(|r: &(f64, f64)| r.0))
    ///     .y_with(y.encode(|r: &(f64, f64)| r.1))
    ///     .curve(Curve::MonotoneX);
    /// let cx = Context::new_blocking()?;
    /// let image = plot.render(&cx, 320, 200)?;
    /// assert!(image.pixels().any(|p| p.0 != [255, 255, 255, 255]));
    /// # Ok(())
    /// # }
    /// ```
    MonotoneX,
}

/// The most points [`Curve::path`] returns: a monotone curve's 16 pieces.
pub(crate) const PATH_POINTS: usize = MONOTONE_PIECES as usize + 1;

/// The pieces a [`Curve::MonotoneX`] segment is drawn as (`line.wgsl`'s
/// `MONOTONE_PIECES`).
pub(crate) const MONOTONE_PIECES: u32 = 16;

impl Curve {
    /// The straight pieces drawn between two rows: 1, 2 or 3 (16 for a
    /// monotone curve).
    pub(crate) const fn pieces(self) -> u32 {
        match self {
            Curve::Linear => 1,
            Curve::StepBefore | Curve::StepAfter => 2,
            Curve::Step => 3,
            Curve::MonotoneX => MONOTONE_PIECES,
        }
    }

    /// Whether the curve reads per-row control values (two columns after
    /// the channels', see [`crate::monotone`]).
    pub(crate) const fn has_controls(self) -> bool {
        matches!(self, Curve::MonotoneX)
    }

    /// The value `line.wgsl`'s `vertex_variant` reads (its `CURVE_*`
    /// constants), and the glue signature's label: `None` for
    /// [`Linear`](Curve::Linear), which keeps the plain `vertex` entry.
    pub(crate) const fn variant(self) -> Option<(u32, &'static str)> {
        match self {
            Curve::Linear => None,
            Curve::Step => Some((1, "curve step")),
            Curve::StepBefore => Some((2, "curve step-before")),
            Curve::StepAfter => Some((3, "curve step-after")),
            Curve::MonotoneX => Some((4, "curve monotone-x")),
        }
    }

    /// The x where a step turns from row `a`'s level to row `b`'s; `None`
    /// for [`Linear`](Curve::Linear).
    fn turn(self, a: [f32; 2], b: [f32; 2]) -> Option<f32> {
        match self {
            Curve::Linear | Curve::MonotoneX => None,
            Curve::Step => Some((a[0] + b[0]) * 0.5),
            Curve::StepBefore => Some(a[0]),
            Curve::StepAfter => Some(b[0]),
        }
    }

    /// The path from `a` to `b` (pixels): its `pieces() + 1` points, the
    /// ends included, in the order `line.wgsl` draws them. For a step,
    /// point `k` lies at `a`'s x for `k = 0`, `b`'s for the last and the
    /// turn's between, and at `a`'s level up to the first vertical piece,
    /// `b`'s after. For a monotone curve, point `k` is the segment's cubic
    /// at `k / 16`, its control values `ctrl` (pixels; ignored by other
    /// curves). SVG paths, CPU picks and later outline geometry read it.
    pub(crate) fn path(
        self,
        a: [f32; 2],
        b: [f32; 2],
        ctrl: [f32; 2],
    ) -> ([[f32; 2]; PATH_POINTS], usize) {
        let n = self.pieces() as usize;
        if self == Curve::MonotoneX {
            let p = crate::monotone::control_points(a, b, ctrl);
            let mut out = [b; PATH_POINTS];
            for (k, q) in out.iter_mut().enumerate() {
                *q = crate::monotone::bezier(p, k as f32 / MONOTONE_PIECES as f32);
            }
            // The ends exactly.
            (out[0], out[n]) = (a, b);
            return (out, n + 1);
        }
        let Some(turn) = self.turn(a, b) else {
            let mut out = [b; PATH_POINTS];
            out[0] = a;
            return (out, 2);
        };
        // Points at `a`'s level: before StepBefore's first (vertical)
        // piece, through the first horizontal piece otherwise.
        let level = usize::from(self != Curve::StepBefore);
        let mut out = [b; PATH_POINTS];
        for (k, p) in out.iter_mut().enumerate().take(n + 1) {
            let x = if k == 0 {
                a[0]
            } else if k == n {
                b[0]
            } else {
                turn
            };
            *p = [x, if k <= level { a[1] } else { b[1] }];
        }
        (out, n + 1)
    }

    /// Whether a point at `x` on the path from `a` (row `i`) to `b`
    /// (row `i + 1`) picks row `i + 1`: never for
    /// [`StepAfter`](Curve::StepAfter), always for
    /// [`StepBefore`](Curve::StepBefore), right of the midpoint for
    /// [`Step`](Curve::Step), and for [`MonotoneX`](Curve::MonotoneX) the
    /// row nearer in x (past the midpoint towards `b`); the nearer end for
    /// [`Linear`](Curve::Linear) is the caller's.
    pub(crate) fn picks_next(self, a: [f32; 2], b: [f32; 2], x: f32) -> Option<bool> {
        match self {
            Curve::Linear => None,
            Curve::MonotoneX => Some((x - (a[0] + b[0]) * 0.5) * (b[0] - a[0]) > 0.0),
            Curve::StepAfter => Some(false),
            Curve::StepBefore => Some(true),
            Curve::Step => Some(x - (a[0] + b[0]) * 0.5 > 0.0),
        }
    }
}

/// A filled region between a series and a baseline, in logical pixels: for
/// each pair of consecutive rows, the trapezoid between the top edge (`y`) and
/// the baseline (`y_end`; set it, e.g. `.y_end(|_| 0.0)`). For a range, set
/// `y` to the high series and `y_end` to the low one.
///
/// Adjacent trapezoids share their edge exactly, so a translucent area has no
/// seam and nothing is blended twice. Where the top edge crosses the baseline
/// within a segment, the two triangles either side of the crossing are
/// filled. A row with a null position breaks the area: neither segment
/// touching it is drawn. The fill comes from a segment's first row. Rows with
/// a column split across buffers (chunks) join as if they were one.
///
/// A pointer inside a segment's region picks the row nearer in x (the first at
/// or before the segment's middle, else the next); the tooltip attaches to that
/// row's top point, and a brush selects rows by their top points. The GPU pick
/// draws the region unchanged, so unlike the other marks it has no margin: the
/// CPU pick also hits within [`PICK_TOLERANCE`](crate::PICK_TOLERANCE) outside
/// the region.
///
/// An area draws with 4 samples (`Antialias::Msaa`). A host that draws in its
/// own pass (`Prepared::draw`, egui, Bevy) chooses the sample count; at 1
/// sample the edges alias.
///
/// ```
/// use gup_core::prelude::*;
///
/// # fn main() -> gup_core::Result<()> {
/// let rain = vec![(0.0, 2.0), (1.0, 5.0), (2.0, 3.0), (3.0, 4.0)];
///
/// let mut plot = Plot::new();
/// plot.x(Linear::new().domain(0.0, 3.0));
/// plot.y(Linear::new().domain(0.0, 6.0));
/// plot.area(rain)
///     .x(|r: &(f64, f64)| r.0)
///     .y(|r: &(f64, f64)| r.1)
///     .y_end(|_| 0.0)
///     .fill(Color::hex(0xcc0000));
/// let cx = Context::new_blocking()?;
/// let (image, _) = plot.render_resolved(&cx, 320, 200)?;
/// // Just above the bottom of the plot, mid-way along the area.
/// let inside = image.get_pixel(160, 150).0;
/// assert_eq!(inside, [0xcc, 0x00, 0x00, 0xff]);
/// # Ok(())
/// # }
/// ```
#[derive(Debug, gup_core_macros::Mark)]
#[mark(module = "gup::marks::area", geometry = Polyline, hit = Area, gpu_pick, antialias = Msaa)]
pub struct Area {
    /// x (logical pixels).
    #[channel(role = X)]
    x: Px,
    /// The top edge (logical pixels).
    #[channel(role = Y)]
    y: Px,
    /// The baseline (logical pixels); set it, e.g. `.y_end(|_| 0.0)`.
    #[channel(role = Y)]
    y_end: Px,
    /// Fill colour. Default Okabe-Ito blue.
    #[channel(role = Color, default = Area::DEFAULT_FILL)]
    fill: Color,
    /// Fill opacity, multiplied into the fill's alpha. Default 1.
    #[channel(default = Unit(1.0))]
    fill_opacity: Unit,
}

impl Area {
    /// The default fill (`#0072B2`).
    pub const DEFAULT_FILL: Color = Color::hex(0x0072b2);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::channel::{Antialias, ConstValue, Mark, Role};
    use crate::shader::MARKS_CIRCLE;

    /// Each curve's path, with the pieces and corners `line.wgsl` draws.
    #[test]
    fn curve_paths_turn_where_their_names_say() {
        let (a, b) = ([10.0, 50.0], [30.0, 20.0]);
        let path = |c: Curve| {
            let (p, n) = c.path(a, b, [0.0; 2]);
            assert_eq!(n as u32, c.pieces() + 1, "{c:?}");
            p[..n].to_vec()
        };
        assert_eq!(path(Curve::Linear), vec![a, b]);
        assert_eq!(path(Curve::StepAfter), vec![a, [30.0, 50.0], b]);
        assert_eq!(path(Curve::StepBefore), vec![a, [10.0, 20.0], b]);
        assert_eq!(path(Curve::Step), vec![a, [20.0, 50.0], [20.0, 20.0], b]);
        assert_eq!(Curve::Linear.variant(), None);
        assert_eq!(Curve::default(), Curve::Linear);
        assert_eq!(Curve::Step.picks_next(a, b, 19.0), Some(false));
        assert_eq!(Curve::Step.picks_next(a, b, 21.0), Some(true));
        assert_eq!(Curve::StepAfter.picks_next(a, b, 29.0), Some(false));
        assert_eq!(Curve::StepBefore.picks_next(a, b, 11.0), Some(true));

        // A monotone curve: 17 points on the cubic, from `a` to `b`, x
        // evenly spaced (the controls lie at thirds in x).
        let (p, n) = Curve::MonotoneX.path(a, b, [45.0, 25.0]);
        assert_eq!(n, 17);
        assert_eq!((p[0], p[16]), (a, b));
        for (k, q) in p[..n].iter().enumerate() {
            assert!(
                (q[0] - (10.0 + 20.0 * k as f32 / 16.0)).abs() < 1e-4,
                "{k}: {q:?}"
            );
            assert!((20.0..=50.0).contains(&q[1]), "{k}: {q:?}");
        }
        assert!(p[..n].windows(2).all(|w| w[1][1] <= w[0][1]), "{p:?}");
        assert_eq!(Curve::MonotoneX.picks_next(a, b, 19.0), Some(false));
        assert_eq!(Curve::MonotoneX.picks_next(a, b, 21.0), Some(true));
        assert_eq!(Curve::MonotoneX.picks_next(b, a, 21.0), Some(false));
    }

    /// The derive reproduces the table Circle had when it was hand-written.
    #[test]
    fn derive_reproduces_the_hand_written_table() {
        let expected = [
            ("x", Some(Role::X), "f32", ConstValue::F32(0.0)),
            ("y", Some(Role::Y), "f32", ConstValue::F32(0.0)),
            ("radius", Some(Role::Size), "f32", ConstValue::F32(3.5)),
            (
                "fill",
                Some(Role::Color),
                "vec4<f32>",
                ConstValue::Vec4(Circle::DEFAULT_FILL.to_array()),
            ),
            ("fill_opacity", None, "f32", ConstValue::F32(1.0)),
            (
                "stroke",
                Some(Role::Color),
                "vec4<f32>",
                ConstValue::Vec4(Color::TRANSPARENT.to_array()),
            ),
            ("stroke_width", None, "f32", ConstValue::F32(0.0)),
        ];
        assert_eq!(Circle::CHANNELS.len(), expected.len());
        for (desc, (name, role, wgsl_type, default)) in Circle::CHANNELS.iter().zip(expected) {
            assert_eq!(desc.name, name);
            assert_eq!(desc.role, role);
            assert_eq!(desc.wgsl_type, wgsl_type);
            assert_eq!(desc.default, default);
        }
        assert_eq!(Circle::NAME, "Circle");
        assert_eq!(Circle::VERTICES_PER_INSTANCE, 6);
        assert_eq!(Circle::ANTIALIAS, Antialias::Analytic);
    }

    #[test]
    fn channel_consts_index_the_descriptor_table() {
        let slots = [
            (Circle::X.slot(), Circle::X.name()),
            (Circle::Y.slot(), Circle::Y.name()),
            (Circle::RADIUS.slot(), Circle::RADIUS.name()),
            (Circle::FILL.slot(), Circle::FILL.name()),
        ];
        for (slot, name) in slots {
            assert_eq!(Circle::CHANNELS[slot as usize].name, name);
        }
    }

    /// The WGSL `CircleIn` struct has exactly the channels, in order.
    #[test]
    fn wgsl_input_struct_matches_channels() {
        use crate::shader::testing::{parse, struct_layout};
        let module = parse(
            "circle",
            &crate::shader::link("circle", "#import gup::marks::circle\n", &[&MARKS_CIRCLE])
                .unwrap(),
        );
        let layout = struct_layout(&module, "gup_marks_circle_CircleIn").expect("CircleIn");
        let names: Vec<_> = layout.members.iter().map(|(n, _)| n.clone()).collect();
        let channels: Vec<_> = Circle::CHANNELS.iter().map(|c| c.name).collect();
        assert_eq!(names, channels);
    }

    /// `Line` is a polyline: its `LineIn` is its channels, then its two
    /// position channels read at the next row.
    #[test]
    fn line_reads_its_positions_at_the_next_row() {
        use crate::channel::Geometry;
        use crate::shader::MARKS_LINE;
        use crate::shader::testing::{parse, struct_layout};
        assert_eq!(Line::GEOMETRY, Geometry::Polyline);
        assert_eq!(Segment::GEOMETRY, Geometry::Quad);
        let module = parse(
            "line",
            &crate::shader::link("line", "#import gup::marks::line\n", &[&MARKS_LINE]).unwrap(),
        );
        let layout = struct_layout(&module, "gup_marks_line_LineIn").expect("LineIn");
        let names: Vec<_> = layout.members.iter().map(|(n, _)| n.clone()).collect();
        assert_eq!(names, ["x", "y", "width", "color", "x_next", "y_next"]);
    }

    /// `Area` is a MSAA polyline: its `AreaIn` is its channels, then its
    /// position channels read at the next row.
    #[test]
    fn area_reads_its_positions_at_the_next_row() {
        use crate::channel::Geometry;
        use crate::shader::MARKS_AREA;
        use crate::shader::testing::{parse, struct_layout};
        assert_eq!(Area::GEOMETRY, Geometry::Polyline);
        assert_eq!(Area::ANTIALIAS, Antialias::Msaa);
        let module = parse(
            "area",
            &crate::shader::link("area", "#import gup::marks::area\n", &[&MARKS_AREA]).unwrap(),
        );
        let layout = struct_layout(&module, "gup_marks_area_AreaIn").expect("AreaIn");
        let names: Vec<_> = layout.members.iter().map(|(n, _)| n.clone()).collect();
        assert_eq!(
            names,
            [
                "x",
                "y",
                "y_end",
                "fill",
                "fill_opacity",
                "x_next",
                "y_next",
                "y_end_next"
            ]
        );
    }
}

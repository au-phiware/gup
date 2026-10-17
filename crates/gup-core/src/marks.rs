// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Marks: [`Circle`], [`Rect`], [`Segment`] and [`Line`] (RFC-001 S6).

use crate::channel::{Color, Px, Unit};

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
#[mark(module = "gup::marks::circle", geometry = Quad)]
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
    pub const DEFAULT_FILL: Color = Color::hex(0x0072b2);
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
#[mark(module = "gup::marks::rect", geometry = Quad)]
pub struct Rect {
    /// Centre x, or the first edge when `width` is 0 (logical pixels).
    #[channel(role = X)]
    x: Px,
    /// Centre y, or the first edge when `height` is 0 (logical pixels).
    #[channel(role = Y)]
    y: Px,
    /// Width (logical pixels); 0 means the span `x..x_end`.
    #[channel(default = Px(0.0))]
    width: Px,
    /// Height (logical pixels); 0 means the span `y..y_end`.
    #[channel(default = Px(0.0))]
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
    pub const DEFAULT_FILL: Color = Color::hex(0x0072b2);
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
#[mark(module = "gup::marks::segment", geometry = Quad)]
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
    pub const DEFAULT_COLOR: Color = Color::hex(0x0072b2);
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
#[mark(module = "gup::marks::line", geometry = Polyline)]
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
    pub const DEFAULT_COLOR: Color = Color::hex(0x0072b2);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::channel::{Antialias, ConstValue, Mark, Role};
    use crate::shader::MARKS_CIRCLE;

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
                ConstValue::Vec4(Color::hex(0x0072b2).to_array()),
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
}

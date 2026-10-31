// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Text layers (GUP-542): labels at data positions, laid out on the CPU.
//!
//! A text layer is not a mark. A mark draws a fixed number of vertices per
//! row from GPU columns; a label has as many glyphs as its text, laid out
//! by `gup-text`. So a [`TextLayer`] evaluates its rows on the CPU, maps
//! them through the plot's scales' CPU mirrors (exact twins of the GPU
//! functions) once the plot has set their ranges, and hands the plot
//! [`TextRun`]s, which every target draws like a tick label. It suits tens
//! to thousands of labels.
//!
//! Labels never fit a domain by luck, so the plot makes room for them (see
//! [`Plot::text`](crate::Plot::text)): a label's ink box relative to its
//! position is a constant number of pixels, measured once and cached, and
//! the plot widens an automatic axis or insets an explicit one by the room
//! each side needs, up to a quarter of the axis.

use crate::Color;
use crate::channel::Px;
use crate::column::DictionaryKey;
use crate::encoding::{ColumnValue, Derived};
use crate::error::{Error, Result};
use crate::geom::{Point, Rect};
use crate::plot::PlacedText;
use crate::plot::inset::{Edge, EdgeAxis};
use crate::scale::DynPositionScale;
use crate::scene::{Anchor, TextRole, TextRun, TextStyle, Weight, Z_AXES};
use crate::theme::Theme;
use gup_text::{Bounds, TextSystem};
use std::any::Any;
use std::fmt::Display;
use std::sync::Arc;

/// A closure from a row to a value of `V`, shared so the layer stays
/// cheap to configure.
type RowFn<T, V> = Arc<dyn Fn(&T) -> V + Send + Sync>;

/// How a label is positioned along one axis.
enum Position<T> {
    /// A data value, mapped through the axis's scale.
    Value(RowFn<T, f64>),
    /// A key, placed at the centre of its band (or point).
    Key(RowFn<T, Arc<str>>),
}

impl<T> Position<T> {
    fn is_key(&self) -> bool {
        matches!(self, Position::Key(_))
    }
}

/// A pixel offset: constant, or read at every full resolve.
enum Offset {
    Const(f32),
    Derived(Derived<Px>),
}

impl Offset {
    fn get(&self) -> f32 {
        match self {
            Offset::Const(px) => *px,
            Offset::Derived(d) => d.get().0,
        }
    }
}

/// The label colour.
enum Paint<T> {
    /// The theme's label colour.
    Theme,
    Const(Color),
    By(RowFn<T, Color>),
}

/// One axis's evaluated positions.
enum Coords {
    Values(Vec<f64>),
    Keys(Vec<Arc<str>>),
}

/// What the closures returned, once per row: kept until a setter changes
/// the layer.
struct Evaluated {
    text: Vec<Arc<str>>,
    x: Coords,
    y: Coords,
    color: Option<Vec<Color>>,
    within_x: Option<Vec<(f64, f64)>>,
    within_y: Option<Vec<(f64, f64)>>,
    /// The labels' ink boxes at the origin; cleared with the rows.
    measured: Option<Measured>,
}

/// Each label's ink box with its anchor at `(0, 0)`, at one font size: the
/// box, and the baseline origin before rounding. `None` for a label with no
/// ink. Where the label really is, the box is this one shifted by whole
/// pixels (`shift`), because the layout rounds the origin to whole pixels
/// and places glyphs on integers from it.
struct Measured {
    size: f32,
    boxes: Vec<Option<(Bounds, [f32; 2])>>,
}

/// The ink box of a label at `at`, from its box and origin at `(0, 0)`.
fn shift(b: Bounds, off: [f32; 2], at: Point) -> Bounds {
    let dx = (at.x + off[0]).round() - off[0].round();
    let dy = (at.y + off[1]).round() - off[1].round();
    Bounds {
        x: b.x + dx,
        y: b.y + dy,
        ..b
    }
}

/// A layer of labels at data positions: one line of text per row, drawn
/// above the data layers and below the axes, clipped to the plot rect.
/// Made by [`Plot::text`](crate::Plot::text), which documents it.
///
/// Every setter takes `&mut self` and returns `&mut Self`, as a
/// [`Selection`](crate::Selection)'s do. The closures run once per row on
/// the first resolve after the layer changed; every full resolve after
/// that (a zoom, a resize) maps the cached values to pixels again, and a
/// cached resolve does no text-layer work at all.
pub struct TextLayer<T> {
    rows: Vec<T>,
    x: Option<Position<T>>,
    y: Option<Position<T>>,
    text: Option<RowFn<T, Arc<str>>>,
    anchor: Anchor,
    dx: Offset,
    dy: Offset,
    size: Option<f32>,
    color: Paint<T>,
    weight: Weight,
    within_x: Option<RowFn<T, (f64, f64)>>,
    within_y: Option<RowFn<T, (f64, f64)>>,
    z: i32,
    /// Bumped by every setter; part of the plot's revision sum.
    revision: u64,
    /// The closures' results, until a setter clears them.
    evaluated: Option<Evaluated>,
}

impl<T: Send + Sync + 'static> TextLayer<T> {
    pub(crate) fn new(rows: Vec<T>) -> Self {
        Self {
            rows,
            x: None,
            y: None,
            text: None,
            anchor: Anchor::CENTER,
            dx: Offset::Const(0.0),
            dy: Offset::Const(0.0),
            size: None,
            color: Paint::Theme,
            weight: Weight::Regular,
            within_x: None,
            within_y: None,
            z: Z_AXES - 1,
            revision: 0,
            evaluated: None,
        }
    }

    /// Count a change: the next resolve evaluates the rows again.
    fn changed(&mut self) -> &mut Self {
        self.revision = self.revision.wrapping_add(1);
        self.evaluated = None;
        self
    }

    /// Place each label at the data value `f` returns, mapped through the
    /// plot's x scale. On a band or point x axis use
    /// [`x_key`](Self::x_key): a value there is an error at resolve.
    pub fn x<D: ColumnValue>(&mut self, f: impl Fn(&T) -> D + Send + Sync + 'static) -> &mut Self {
        self.x = Some(Position::Value(Arc::new(move |r| f(r).to_f64())));
        self.changed()
    }

    /// Place each label at the data value `f` returns, mapped through the
    /// plot's y scale. On a band or point y axis use
    /// [`y_key`](Self::y_key): a value there is an error at resolve.
    pub fn y<D: ColumnValue>(&mut self, f: impl Fn(&T) -> D + Send + Sync + 'static) -> &mut Self {
        self.y = Some(Position::Value(Arc::new(move |r| f(r).to_f64())));
        self.changed()
    }

    /// Place each label at the centre of the band (or point) of the key
    /// `key` returns, on a [`Band`](crate::Band) or [`Point`](crate::Point)
    /// x axis. The key is matched by its `Display` label, as the axis shows
    /// it; a key no other layer put in the axis's domain draws nothing (a
    /// label never adds a key). On a numeric x axis this is an error at
    /// resolve.
    pub fn x_key<K: DictionaryKey>(
        &mut self,
        key: impl Fn(&T) -> K + Send + Sync + 'static,
    ) -> &mut Self {
        self.x = Some(Position::Key(Arc::new(move |r| key(r).to_string().into())));
        self.changed()
    }

    /// Place each label at the centre of the band (or point) of the key
    /// `key` returns, on a [`Band`](crate::Band) or [`Point`](crate::Point)
    /// y axis; see [`x_key`](Self::x_key).
    pub fn y_key<K: DictionaryKey>(
        &mut self,
        key: impl Fn(&T) -> K + Send + Sync + 'static,
    ) -> &mut Self {
        self.y = Some(Position::Key(Arc::new(move |r| key(r).to_string().into())));
        self.changed()
    }

    /// Each row's label: `f`'s result as `Display` writes it, so
    /// `.text(|r| r.count)` and `.text(|r| r.name)` need no `format!`. An
    /// empty label draws nothing.
    pub fn text<S: Display>(&mut self, f: impl Fn(&T) -> S + Send + Sync + 'static) -> &mut Self {
        self.text = Some(Arc::new(move |r| f(r).to_string().into()));
        self.changed()
    }

    /// Where the label's position sits on its text (default
    /// [`Anchor::CENTER`]): [`Anchor::BOTTOM`] puts the text above its
    /// point, [`Anchor::LEFT`] to its right.
    pub fn anchor(&mut self, anchor: Anchor) -> &mut Self {
        self.anchor = anchor;
        self.changed()
    }

    /// Move every label `px` logical pixels right (left when negative),
    /// after the position is mapped (default 0).
    pub fn dx(&mut self, px: f32) -> &mut Self {
        self.dx = Offset::Const(px);
        self.changed()
    }

    /// Move every label `px` logical pixels down (up when negative), after
    /// the position is mapped (default 0): `.anchor(Anchor::BOTTOM).dy(-6.0)`
    /// leaves 6 px between a point and its label.
    pub fn dy(&mut self, px: f32) -> &mut Self {
        self.dy = Offset::Const(px);
        self.changed()
    }

    /// Like [`dx`](Self::dx), with the offset read at every full resolve,
    /// after the plot set its scales' ranges (half a band's width, say).
    pub fn dx_with(&mut self, px: Derived<Px>) -> &mut Self {
        self.dx = Offset::Derived(px);
        self.changed()
    }

    /// Like [`dy`](Self::dy), with the offset read at every full resolve,
    /// after the plot set its scales' ranges.
    pub fn dy_with(&mut self, px: Derived<Px>) -> &mut Self {
        self.dy = Offset::Derived(px);
        self.changed()
    }

    /// The font size in logical pixels (default the theme's
    /// [`tick_size`](crate::Theme::tick_size)).
    pub fn size(&mut self, px: f32) -> &mut Self {
        self.size = Some(px);
        self.changed()
    }

    /// One colour for every label (default the theme's
    /// [`label`](crate::Theme::label) colour). Replaces
    /// [`color_by`](Self::color_by): the later call of the two wins.
    pub fn color(&mut self, color: Color) -> &mut Self {
        self.color = Paint::Const(color);
        self.changed()
    }

    /// Each row's label colour. Replaces [`color`](Self::color): the later
    /// call of the two wins.
    pub fn color_by(&mut self, f: impl Fn(&T) -> Color + Send + Sync + 'static) -> &mut Self {
        self.color = Paint::By(Arc::new(f));
        self.changed()
    }

    /// The face that draws the labels (default [`Weight::Regular`]).
    pub fn weight(&mut self, weight: Weight) -> &mut Self {
        self.weight = weight;
        self.changed()
    }

    /// Draw a row's label only if it fits across the span `(lo, hi)` (in x
    /// data units) that `span` returns: its ink must be at most the span's
    /// pixel width less 2 px. How a value label stays inside its bar. On a
    /// band or point x axis the span is ignored.
    pub fn within_x(
        &mut self,
        span: impl Fn(&T) -> (f64, f64) + Send + Sync + 'static,
    ) -> &mut Self {
        self.within_x = Some(Arc::new(span));
        self.changed()
    }

    /// Draw a row's label only if it fits up the span `(lo, hi)` (in y
    /// data units) that `span` returns: its ink must be at most the span's
    /// pixel height less 2 px. On a band or point y axis the span is
    /// ignored.
    pub fn within_y(
        &mut self,
        span: impl Fn(&T) -> (f64, f64) + Send + Sync + 'static,
    ) -> &mut Self {
        self.within_y = Some(Arc::new(span));
        self.changed()
    }

    /// The layer's draw order among the plot's layers (default 99, the
    /// highest a layer may have: above the data layers, below the axes),
    /// within `-99..=99` like a [`Selection`](crate::Selection)'s.
    pub fn z(&mut self, z: i32) -> &mut Self {
        self.z = z;
        self.changed()
    }

    /// How many rows (labels) the layer has.
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Whether the layer has no rows.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Run every closure once per row.
    fn evaluate(&self, x: &Position<T>, y: &Position<T>, text: &RowFn<T, Arc<str>>) -> Evaluated {
        let coords = |p: &Position<T>| match p {
            Position::Value(f) => Coords::Values(self.rows.iter().map(|r| f(r)).collect()),
            Position::Key(f) => Coords::Keys(self.rows.iter().map(|r| f(r)).collect()),
        };
        let spans = |f: &Option<RowFn<T, (f64, f64)>>| {
            f.as_ref().map(|f| self.rows.iter().map(|r| f(r)).collect())
        };
        Evaluated {
            text: self.rows.iter().map(|r| text(r)).collect(),
            x: coords(x),
            y: coords(y),
            color: match &self.color {
                Paint::By(f) => Some(self.rows.iter().map(|r| f(r)).collect()),
                _ => None,
            },
            within_x: spans(&self.within_x),
            within_y: spans(&self.within_y),
            measured: None,
        }
    }

    /// Evaluate the rows if a setter cleared them, and measure the labels
    /// if they were not measured at this size. `false` when the layer lacks
    /// x, y or text.
    fn prepare(&mut self, text: &TextSystem, theme: &Theme) -> bool {
        let (Some(px), Some(py), Some(label)) = (&self.x, &self.y, &self.text) else {
            return false;
        };
        if self.evaluated.is_none() {
            self.evaluated = Some(self.evaluate(px, py, label));
        }
        let size = self.size.unwrap_or(theme.tick_size.0);
        let (anchor, weight) = (self.anchor, self.weight);
        let ev = self.evaluated.as_mut().expect("evaluated above");
        if ev.measured.as_ref().is_none_or(|m| m.size != size) {
            let font = text.font_weight(weight);
            let boxes = ev
                .text
                .iter()
                .map(|s| {
                    if s.is_empty() {
                        return None;
                    }
                    let run = gup_text::Run {
                        text: s,
                        size,
                        at: [0.0, 0.0],
                        anchor,
                        weight,
                        features: gup_text::Features::default(),
                    };
                    let b = font.ink_bounds(&run);
                    (b.width > 0.0 && b.height > 0.0).then(|| (b, font.baseline_origin(&run)))
                })
                .collect();
            ev.measured = Some(Measured { size, boxes });
        }
        true
    }
}

/// An error when a position and its axis disagree: a value on a band or
/// point axis, or a key on a numeric one.
fn check_axis<T>(
    axis: &'static str,
    pos: Option<&Position<T>>,
    scale: &dyn DynPositionScale,
) -> Result<()> {
    let Some(pos) = pos else { return Ok(()) };
    match (pos.is_key(), scale.reads_keys()) {
        (false, true) => Err(Error::config(
            "text layer position",
            format!(
                "the {axis} axis is a band or point scale, which places categories; \
                 give the text layer its key with `{axis}_key(..)`, not `{axis}(..)`"
            ),
        )),
        (true, false) => Err(Error::config(
            "text layer position",
            format!(
                "`{axis}_key(..)` places a label at a category, but the {axis} axis is \
                 numeric; give the text layer a value with `{axis}(..)`"
            ),
        )),
        _ => Ok(()),
    }
}

/// A coordinate in pixels, from a cached value or key.
fn pixel(coords: &Coords, i: usize, scale: &dyn DynPositionScale) -> f64 {
    match coords {
        Coords::Values(v) => scale.eval(v[i]),
        Coords::Keys(k) => scale.key_position(&k[i]).unwrap_or(f64::NAN),
    }
}

/// Whether `ink` (the label's extent along an axis, px) fits the `span`'s
/// pixel extent on `scale`, less 2 px.
fn fits(ink: f32, span: (f64, f64), scale: &dyn DynPositionScale) -> bool {
    let room = (scale.eval(span.1) - scale.eval(span.0)).abs() - 2.0;
    f64::from(ink) <= room
}

/// The overlap of two rectangles, if they share any area.
fn intersect(a: Rect, b: Rect) -> Option<Rect> {
    let (l, t) = (a.left().max(b.left()), a.top().max(b.top()));
    let (r, bo) = (a.right().min(b.right()), a.bottom().min(b.bottom()));
    (r > l && bo > t).then(|| Rect::from_edges(l, t, r, bo))
}

/// A plot's text layer, without its row type (the plot keeps them as
/// trait objects; internal).
pub(crate) trait AnnotationLayer: Send + Sync {
    /// Bumped by every setter.
    fn revision(&self) -> u64;
    /// The layer's z-order among the plot's items.
    fn z(&self) -> i32;
    /// The layer's labels for the plot rect `plot`, with the ranges of `x`
    /// and `y` set: each run with its ink box, cut to the plot rect. Rows
    /// with a non-finite position, an unknown key, an anchor more than
    /// 4 × size px outside the plot rect, no ink inside it, or a label
    /// wider (taller) than its `within` span draw nothing.
    fn runs(
        &mut self,
        text: &TextSystem,
        x: &dyn DynPositionScale,
        y: &dyn DynPositionScale,
        plot: Rect,
        theme: &Theme,
    ) -> Result<Vec<PlacedText>>;
    /// Evaluate and measure (cached), then the largest px any label reaches
    /// past its position along `[x, y]`, at least 0; `[0, 0]` for a layer
    /// missing x, y or text, and 0 along an axis placed by keys.
    fn reach(&mut self, text: &TextSystem, theme: &Theme) -> [f32; 2];
    /// Push a `(t, near, far)` candidate per label onto `out[0]` (x) and
    /// `out[1]` (y) for each `Some` axis, mapping positions through `x` and
    /// `y` at their current (provisional) ranges. Uses the cache `reach`
    /// filled; pushes nothing if it is empty.
    fn edges(
        &self,
        x: &dyn DynPositionScale,
        y: &dyn DynPositionScale,
        axes: [Option<&EdgeAxis>; 2],
        out: &mut [Vec<Edge>; 2],
    );
    /// Drop the evaluated labels so the next resolve evaluates them again
    /// (something they read outside the layer changed).
    fn invalidate(&mut self);
    /// The layer, to downcast to its row type.
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

impl<T: Send + Sync + 'static> AnnotationLayer for TextLayer<T> {
    fn revision(&self) -> u64 {
        self.revision
    }

    fn z(&self) -> i32 {
        self.z
    }

    fn invalidate(&mut self) {
        self.changed();
    }

    fn runs(
        &mut self,
        text: &TextSystem,
        x: &dyn DynPositionScale,
        y: &dyn DynPositionScale,
        plot: Rect,
        theme: &Theme,
    ) -> Result<Vec<PlacedText>> {
        check_axis("x", self.x.as_ref(), x)?;
        check_axis("y", self.y.as_ref(), y)?;
        if !self.prepare(text, theme) {
            return Ok(Vec::new());
        }
        let ev = self.evaluated.as_ref().expect("prepared above");
        let measured = ev.measured.as_ref().expect("prepared above");
        let (dx, dy) = (self.dx.get(), self.dy.get());
        let size = self.size.unwrap_or(theme.tick_size.0);
        let reach = plot.inflate(4.0 * size);
        let color = match &self.color {
            Paint::Const(c) => Some(*c),
            _ => None,
        };
        let mut out = Vec::new();
        for (i, s) in ev.text.iter().enumerate() {
            if s.is_empty() {
                continue;
            }
            let at = Point::new(
                (pixel(&ev.x, i, x) as f32) + dx,
                (pixel(&ev.y, i, y) as f32) + dy,
            );
            if !(at.x.is_finite() && at.y.is_finite())
                || at.x < reach.left()
                || at.x > reach.right()
                || at.y < reach.top()
                || at.y > reach.bottom()
            {
                continue;
            }
            let run = TextRun {
                text: Arc::clone(s),
                at,
                anchor: self.anchor,
                style: TextStyle {
                    size: Px(size),
                    color: color
                        .or_else(|| ev.color.as_ref().map(|c| c[i]))
                        .unwrap_or(theme.label),
                    weight: self.weight,
                },
                role: TextRole::Annotation,
            };
            let Some((box0, off)) = measured.boxes[i] else {
                continue;
            };
            let ink = shift(box0, off, at);
            if let (Some(spans), Coords::Values(_)) = (&ev.within_x, &ev.x)
                && !fits(ink.width, spans[i], x)
            {
                continue;
            }
            if let (Some(spans), Coords::Values(_)) = (&ev.within_y, &ev.y)
                && !fits(ink.height, spans[i], y)
            {
                continue;
            }
            let Some(bounds) = intersect(ink.into(), plot) else {
                continue;
            };
            out.push(PlacedText { run, bounds });
        }
        Ok(out)
    }

    fn reach(&mut self, text: &TextSystem, theme: &Theme) -> [f32; 2] {
        if !self.prepare(text, theme) {
            return [0.0, 0.0];
        }
        let ev = self.evaluated.as_ref().expect("prepared above");
        let measured = ev.measured.as_ref().expect("prepared above");
        let (dx, dy) = (self.dx.get(), self.dy.get());
        let keyed = [
            self.x.as_ref().is_none_or(Position::is_key),
            self.y.as_ref().is_none_or(Position::is_key),
        ];
        let mut reach = [0.0f32; 2];
        for (b, off) in measured.boxes.iter().flatten() {
            let [(xn, xf), (yn, yf)] = extents(b, off, dx, dy);
            for (i, e) in [xn.max(xf), yn.max(yf)].into_iter().enumerate() {
                if !keyed[i] && e > reach[i] {
                    reach[i] = e;
                }
            }
        }
        reach
    }

    fn edges(
        &self,
        x: &dyn DynPositionScale,
        y: &dyn DynPositionScale,
        axes: [Option<&EdgeAxis>; 2],
        out: &mut [Vec<Edge>; 2],
    ) {
        let Some(measured) = self.evaluated.as_ref().and_then(|e| e.measured.as_ref()) else {
            return;
        };
        let ev = self.evaluated.as_ref().expect("measured implies evaluated");
        let (dx, dy) = (self.dx.get(), self.dy.get());
        let scales = [x, y];
        let coords = [&ev.x, &ev.y];
        for (i, (b, off)) in measured
            .boxes
            .iter()
            .enumerate()
            .filter_map(|(i, b)| b.as_ref().map(|b| (i, b)))
        {
            let ext = extents(b, off, dx, dy);
            for a in 0..2 {
                let (Some(axis), Coords::Values(v)) = (axes[a], coords[a]) else {
                    continue;
                };
                if scales[a].reads_keys() {
                    continue;
                }
                if let Some(t) = axis.t(scales[a].eval(v[i])) {
                    axis.push_edge(&mut out[a], t, ext[a].0, ext[a].1);
                }
            }
        }
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// The px a label with ink box `b` (at the origin, with baseline origin
/// `off`) reaches past its position, `[(left, right), (bottom, top)]`, with
/// 1 px for the origin's rounding at the real position.
fn extents(b: &Bounds, off: &[f32; 2], dx: f32, dy: f32) -> [(f32, f32); 2] {
    let (rx, ry) = (off[0] - off[0].round(), off[1] - off[1].round());
    let (left, right) = (b.x + rx, b.x + rx + b.width);
    let (top, bottom) = (b.y + ry, b.y + ry + b.height);
    [
        (-(dx + left) + 1.0, dx + right + 1.0),
        (dy + bottom + 1.0, -(dy + top) + 1.0),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scale::{Band, Linear, ScaleRef};

    /// The `within` rule: the ink fits when it is at most the span's pixel
    /// extent less 2 px, whichever way round the span and the range run.
    #[test]
    fn a_label_fits_a_span_less_two_pixels() {
        // 10 px per data unit, on a reversed (y-like) range.
        let y = ScaleRef::new(Linear::new().domain(0.0, 10.0).range(Px(100.0), Px(0.0)));
        assert!(fits(18.0, (0.0, 2.0), &y));
        assert!(!fits(18.5, (0.0, 2.0), &y));
        assert!(fits(18.0, (2.0, 0.0), &y));
        assert!(!fits(1.0, (f64::NAN, 2.0), &y));
    }

    /// GUP-555 AC1: a label's ink box anywhere is its box at the origin
    /// shifted by whole pixels, to the last bit, so `runs` needs no layout.
    #[test]
    fn a_translated_ink_box_equals_the_laid_out_one() {
        let anchors = [
            Anchor::CENTER,
            Anchor::TOP,
            Anchor::BOTTOM,
            Anchor::LEFT,
            Anchor::RIGHT,
            Anchor::TOP_LEFT,
            Anchor::TOP_RIGHT,
            Anchor::BOTTOM_LEFT,
            Anchor::BOTTOM_RIGHT,
        ];
        let mut state = 0x9e37_79b9_7f4a_7c15u64;
        let mut next = move || {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            ((state >> 33) as f32) / ((1u64 << 31) as f32)
        };
        let mut checked = 0;
        for weight in [Weight::Regular, Weight::SemiBold] {
            let font = match weight {
                Weight::Regular => gup_text::Font::inter(),
                Weight::SemiBold => gup_text::Font::inter_semibold(),
            };
            for anchor in anchors {
                for size in [11.0, 12.0, 16.0] {
                    for text in ["Sydney", "Alice Springs", "\u{2212}0.5"] {
                        let run = |at: [f32; 2]| gup_text::Run {
                            text,
                            size,
                            at,
                            anchor,
                            weight,
                            features: gup_text::Features::default(),
                        };
                        let box0 = font.ink_bounds(&run([0.0, 0.0]));
                        let off = font.baseline_origin(&run([0.0, 0.0]));
                        for _ in 0..50 {
                            let at = [next() * 800.0, next() * 800.0];
                            let want = font.ink_bounds(&run(at));
                            let got = shift(box0, off, Point::new(at[0], at[1]));
                            assert_eq!(
                                got, want,
                                "{text:?} {anchor:?} {size} {weight:?} at {at:?}"
                            );
                            checked += 1;
                        }
                    }
                }
            }
        }
        assert_eq!(checked, 2 * 9 * 3 * 3 * 50);
    }

    /// A value on a band axis and a key on a numeric one are errors that
    /// name the setter to use; matching ones and missing ones are not.
    #[test]
    fn positions_must_suit_their_axis() {
        let band = ScaleRef::new(Band::new());
        let lin = ScaleRef::new(Linear::new());
        let value: Position<()> = Position::Value(Arc::new(|_| 0.0));
        let key: Position<()> = Position::Key(Arc::new(|_| "a".into()));
        let err = check_axis("x", Some(&value), &band).unwrap_err();
        assert!(err.to_string().contains("`x_key(..)`"), "{err}");
        let err = check_axis("y", Some(&key), &lin).unwrap_err();
        assert!(err.to_string().contains("`y(..)`"), "{err}");
        assert!(check_axis("x", Some(&key), &band).is_ok());
        assert!(check_axis("x", Some(&value), &lin).is_ok());
        assert!(check_axis::<()>("x", None, &band).is_ok());
    }
}

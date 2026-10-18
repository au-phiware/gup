// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! `Plot`: shared x/y scales, data layers, axes and a title, resolved
//! into a [`Layout`] and a [`Scene`] (RFC-001 §8–9, S0a subset).
//!
//! Any number of layers share the x/y scales, whose automatic domains
//! union across layers and channels. Left and bottom axes with tick
//! labels, an optional title, axis titles, a [`Theme`](crate::Theme) and
//! colour legends at the right (found through
//! [`ShaderFn::legend`](crate::ShaderFn::legend)); grids and the
//! object-safe `Chart` trait are later RFC-001 S7 steps.

use crate::channel::{Mark, Px};
use crate::context::{Context, ContextId};
use crate::cull;
use crate::error::{Error, Result};
use crate::geom::{Point, Rect};
use crate::gpu_pick::{GpuPicker, PickTicket};
use crate::input::{Axes, Brush, Button, Drag, Event, EventInfo, Input, Response};
use crate::marks::{Circle, Line, Rect as MarkRect, Segment};
use crate::pick::{PickIndex, RowId};
use crate::render::{LayerGpu, Span};
use crate::scale::{DynPositionScale, Linear, PositionScale, ScaleRef, Ticks};
use crate::scene::{
    Anchor, HAlign, Item, ItemKind, RectPrim, Rule, Scene, TextRole, TextRun, TextStyle, VAlign,
    Weight, Z_AXES, Z_BRUSH, Z_GRID, Z_TITLE,
};
use crate::selection::{Layer, Selection};
use crate::theme::Theme;
use gup_text::TextSystem;
use std::sync::Arc;

/// A text run placed by layout, with its ink box.
#[derive(Clone, Debug, PartialEq)]
pub struct PlacedText {
    /// The run, as it appears in the scene.
    pub run: TextRun,
    /// Where its glyphs are drawn.
    pub bounds: Rect,
}

/// The resolved layout: where everything went, in logical pixels.
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    /// Logical width.
    pub width: f32,
    /// Logical height.
    pub height: f32,
    /// The plot (data) rectangle. Marks are clipped to it.
    pub plot: Rect,
    /// Every text run with its ink box.
    pub texts: Vec<PlacedText>,
    /// The rectangles covered by guide rules.
    pub guides: Vec<Rect>,
    /// How far marks extend past their positions (e.g. a radius).
    pub mark_overhang: f32,
    /// x tick values and their pixel positions.
    pub x_ticks: Vec<(f64, f32)>,
    /// y tick values and their pixel positions.
    pub y_ticks: Vec<(f64, f32)>,
    /// The legend column, `Some` exactly when a legend is drawn. Lies
    /// inside the canvas, right of [`plot`](Self::plot).
    pub legend: Option<Rect>,
    /// The hover tooltip's box, `Some` exactly while one is drawn. Its text
    /// runs are in the scene (role [`TextRole::Tooltip`]) but not in
    /// [`texts`](Self::texts).
    pub tooltip: Option<Rect>,
}

/// A layout plus the scene drawn from it.
#[derive(Clone, Debug)]
pub struct Resolved {
    /// Where everything went.
    pub layout: Layout,
    /// What to draw.
    pub scene: Scene,
}

/// A chart with shared x/y position scales, data layers, axes and an
/// optional title. A new plot has automatic linear scales, so a layer and
/// two closures make a chart:
///
/// ```
/// use gup_core::ImageTarget;
/// use gup_core::prelude::*;
///
/// struct Row { a: f64, b: f64 }
/// let rows = [(1.0, 2.0), (2.0, 5.0), (3.0, 3.0), (4.0, 8.0)]
///     .map(|(a, b)| Row { a, b });
///
/// let cx = Context::new_blocking()?;
/// let mut plot = Plot::new();
/// plot.circles(rows).x(|r| r.a).y(|r| r.b);
/// let resolved = plot.resolve(&cx, 320.0, 200.0)?;
/// let image = ImageTarget::new(&cx, 320, 200)?.render_blocking(&cx, &resolved.scene)?;
///
/// // Each row is a dot of the default fill (#0072B2) at its position.
/// let at = |ticks: &[(f64, f32)], v: f64| {
///     let (a, b) = (ticks[0], ticks[ticks.len() - 1]);
///     f64::from(a.1) + (v - a.0) * f64::from(b.1 - a.1) / (b.0 - a.0)
/// };
/// for (a, b) in [(1.0, 2.0), (2.0, 5.0), (3.0, 3.0), (4.0, 8.0)] {
///     let (x, y) = (at(&resolved.layout.x_ticks, a), at(&resolved.layout.y_ticks, b));
///     let p = image.get_pixel(x as u32, y as u32).0;
///     assert!(p[0] < 10 && (100..130).contains(&p[1]) && p[2] > 160, "{p:?}");
/// }
/// # Ok::<(), gup_core::Error>(())
/// ```
///
/// A closure on `x`, `y`, `x_end` or `y_end` is encoded through the plot's
/// scale for that axis. For a constant (`.y_end_with(0.0)`) or an encoding
/// through another scale (`.x_with(log.encode(..))`), use the `_with` form.
/// [`x`](Self::x) and [`y`](Self::y) replace the default scales, and
/// return handles for encoding colour and size:
///
/// ```
/// use gup_core::prelude::*;
///
/// #[derive(Clone)]
/// struct Reading { gdp: f64, life_exp: f64, temp: f64 }
/// let readings: Vec<Reading> = (1..=50)
///     .map(|i| {
///         let i = f64::from(i);
///         Reading { gdp: i * 1_000.0, life_exp: 40.0 + i, temp: i % 30.0 }
///     })
///     .collect();
///
/// let cx = Context::new_blocking()?;
/// let mut plot = Plot::new();
/// let (x, y) = (plot.x(Linear::new()), plot.y(Log::new()));
/// let heat = Sequential::viridis();
/// plot.title("Wealth and health")
///     .add(Selection::<Reading, Circle>::new(readings))
///     .attr(Circle::X, x.encode(|r: &Reading| r.gdp))
///     .attr(Circle::Y, y.encode(|r: &Reading| r.life_exp))
///     .attr(Circle::FILL, heat.encode(|r: &Reading| r.temp))
///     .attr(Circle::RADIUS, Px(4.0));
/// // .attr(Circle::RADIUS, heat.encode(..))  // ✗ Sequential produces Color, RADIUS needs Px
/// let image = plot.render(&cx, 640, 400)?;
/// assert_eq!(image.dimensions(), (640, 400));
/// # Ok::<(), gup_core::Error>(())
/// ```
pub struct Plot {
    title: Option<String>,
    x: Box<dyn DynPositionScale>,
    y: Box<dyn DynPositionScale>,
    /// Bumped whenever `x` or `y` is replaced, so layers re-bind the
    /// closures they encode through the plot's scales (starts at 1).
    axes_generation: u64,
    pub(crate) layers: Vec<Box<dyn Layer>>,
    x_title: Option<String>,
    y_title: Option<String>,
    theme: Theme,
    /// Set by `legends(false)`.
    legends_off: bool,
    /// Bumped by every `&mut self` method that changes the plot, except
    /// `resolve` and `handle`.
    revision: u64,
    /// The last successful resolve and what it was resolved for.
    cache: Option<(CacheKey, Resolved)>,
    /// How many full (uncached) resolves have succeeded.
    pub(crate) generation: u64,
    /// Whether each axis (x, y) was widened by the last resolve: its range
    /// is the whole plot rect, not inset at the far end.
    widened: [bool; 2],
    /// The domain a zoom left on each widened axis. While the axis still
    /// has it, the axis keeps the widened range policy, so zooming moves
    /// no pixel. A domain set any other way gets the fixed-domain policy.
    zoomed: [Option<(f64, f64)>; 2],
    /// The pick index, and the (generation, revision sum) it was built at.
    pick: Option<((u64, u64), PickIndex)>,
    /// Each layer's z and GPU state at the last resolve, by layer index:
    /// what a GPU pick draws.
    pub(crate) drawn: Vec<(i32, Arc<LayerGpu>, Arc<[Span]>)>,
    /// Each layer's visible draw entries at the last resolve (empty: all),
    /// shared by draws, CPU picks and GPU picks.
    visible: Vec<Vec<bool>>,
    /// Whether chunks outside the plot rect are skipped.
    cull: bool,
    /// GPU pick state, made on the first `request_pick`.
    pub(crate) gpu_pick: Option<GpuPicker>,
    /// The hovered row, where its tooltip attaches, and the generation that
    /// anchor was found at.
    hover: Option<(RowId, Point, u64)>,
    /// Where the primary button went down, until it comes up.
    press: Option<Point>,
    /// The last pointer position seen.
    last_at: Point,
    /// The cached resolve with the tooltip composed on, and the generation
    /// of the base it was made from. Dropped when the hover changes.
    overlay: Option<(u64, Resolved)>,
    /// The plot rect's horizontal margins, held while the pointer drives the
    /// view so a change in tick-label width does not move the rect.
    hold: Option<Hold>,
    /// Which axes the wheel zooms and a drag pans.
    view_axes: Axes,
    /// What a primary-button drag does.
    drag: Drag,
    /// The domains the drag started from, once the pointer left the click
    /// slop; panning is always from these, by the total movement.
    dragging: Option<[Option<(f64, f64)>; 2]>,
    /// How each axis was before the first view change since the last
    /// [`reset_view`](Self::reset_view).
    home: Option<[Home; 2]>,
    /// The brush, in data units, with its rows.
    brush: Option<Brush>,
    /// Called with the new brush on release, and with `None` when cleared.
    brush_handlers: Vec<BrushHandler>,
    /// The rectangle being dragged (logical pixels, clamped to the plot
    /// rect), once the pointer left the click slop in `Brush` or `Zoom` mode.
    marquee: Option<Rect>,
    /// Whether the last resolve found a `gpu_pick` layer the CPU cannot
    /// read: then hover and click are picked on the GPU, for every layer.
    gpu_mode: bool,
    /// Pending and in-flight GPU hover and click picks.
    gpu_hover: GpuHover,
    /// Called for every `Enter`, `Leave` and `Click`, with the row's id.
    row_handlers: Vec<RowHandler>,
}

type BrushHandler = Box<dyn FnMut(Option<&Brush>) + Send + Sync>;
type RowHandler = Box<dyn FnMut(&EventInfo) + Send + Sync>;

/// Hover and click picks asked of the GPU (see `Plot::gpu_mode`).
#[derive(Default)]
struct GpuHover {
    /// The latest pointer position not yet submitted.
    hover: Option<Point>,
    /// The hover pick in flight. A result with another ticket is dropped.
    hover_ticket: Option<PickTicket>,
    /// Clicks not yet submitted, oldest first.
    clicks: Vec<Point>,
    /// The click pick in flight.
    click_ticket: Option<PickTicket>,
}

impl GpuHover {
    fn busy(&self) -> bool {
        self.hover.is_some()
            || self.hover_ticket.is_some()
            || !self.clicks.is_empty()
            || self.click_ticket.is_some()
    }

    /// Forget the hover pick, pending and in flight (its result is dropped).
    fn cancel_hover(&mut self) {
        self.hover = None;
        self.hover_ticket = None;
    }
}

/// The brush rectangle `rect` (logical pixels) as a scene item: a
/// translucent fill inside a 1 px border, on whole pixels.
fn brush_item(theme: &Theme, rect: Rect) -> Item {
    let (l, t) = (rect.left().round(), rect.top().round());
    let (r, b) = (rect.right().round(), rect.bottom().round());
    let (w, h) = (r - l, b - t);
    let prim = |x, y, w, h, color| RectPrim {
        rect: Rect::new(x, y, w, h),
        color,
    };
    let rects = if w < 2.0 || h < 2.0 {
        vec![prim(l, t, w, h, theme.brush_stroke)]
    } else {
        let s = theme.brush_stroke;
        vec![
            prim(l + 1.0, t + 1.0, w - 2.0, h - 2.0, theme.brush_fill),
            prim(l, t, w, 1.0, s),
            prim(l, b - 1.0, w, 1.0, s),
            prim(l, t + 1.0, 1.0, h - 2.0, s),
            prim(r - 1.0, t + 1.0, 1.0, h - 2.0, s),
        ]
    };
    Item {
        z: Z_BRUSH,
        clip: None,
        kind: ItemKind::Rects(rects),
    }
}

/// An axis as it was before the view moved.
#[derive(Copy, Clone, Debug)]
enum Home {
    /// Data-driven: fit again.
    Auto,
    /// Explicit domain.
    Fixed(f64, f64),
}

/// The horizontal margins held for one chart size (see `Plot::hold_layout`).
#[derive(Copy, Clone, Debug)]
struct Hold {
    /// The size, as `f32::to_bits` (as `CacheKey` stores it).
    width: u32,
    height: u32,
    /// The plot rect's left edge.
    left: f32,
    /// The plot rect's right edge. Equivalent to the right margin
    /// (`width - right`) for one size, and exact.
    right: f32,
}

/// How far (px) the pointer may move between press and release for the
/// pair to be a click.
const CLICK_SLOP: f32 = 4.0;

/// What a cached [`Resolved`] is valid for.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
struct CacheKey {
    context: ContextId,
    width: u32,
    height: u32,
    /// Sum of every revision counter that the plot reads.
    revisions: u64,
}

/// Zoom factor per wheel line (scrolling up zooms in).
const ZOOM_PER_LINE: f64 = 0.85;

impl crate::chart::Chart for Plot {
    fn resolve(&mut self, cx: &Context, width: f32, height: f32) -> Result<&Resolved> {
        Plot::resolve(self, cx, width, height)
    }
    fn layout(&self) -> Option<&Layout> {
        Plot::layout(self)
    }
    fn handle(&mut self, input: Input) -> Response {
        Plot::handle(self, input)
    }
    fn pick_row(&mut self, at: Point) -> Option<RowId> {
        Plot::pick_row(self, at)
    }
    fn needs_frame(&self) -> bool {
        self.gpu_hover.busy()
    }
    fn title_text(&self) -> Option<&str> {
        Plot::title_text(self)
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl std::fmt::Debug for Plot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Plot")
            .field("title", &self.title)
            .field("layers", &self.layers.len())
            .finish_non_exhaustive()
    }
}

/// `s`, or its longest prefix followed by "…" that fits in `max` pixels
/// at `size` (U+2026 is in the bundled font subset).
fn fit_text(text: &TextSystem, s: &str, size: f32, max: f32) -> String {
    if text.measure(s, size).width <= max {
        return s.to_owned();
    }
    let mut kept: Vec<char> = s.chars().collect();
    while !kept.is_empty() {
        kept.pop();
        let candidate: String = kept.iter().collect::<String>().trim_end().to_owned() + "…";
        if text.measure(&candidate, size).width <= max {
            return candidate;
        }
    }
    "…".to_owned()
}

/// Ticks and the plot rect they leave room for.
struct TickLayout {
    x_ticks: Ticks,
    y_ticks: Ticks,
    y_label_w: f32,
    plot: Rect,
}

impl Default for Plot {
    fn default() -> Self {
        Self::new()
    }
}

impl Plot {
    /// An empty plot with automatic linear x and y scales, fitted to the
    /// data of its layers. [`x`](Self::x) and [`y`](Self::y) replace them.
    pub fn new() -> Self {
        Self {
            title: None,
            x: Box::new(ScaleRef::new(Linear::new())),
            y: Box::new(ScaleRef::new(Linear::new())),
            axes_generation: 1,
            layers: Vec::new(),
            x_title: None,
            y_title: None,
            theme: Theme::default(),
            legends_off: false,
            revision: 0,
            hold: None,
            cache: None,
            generation: 0,
            widened: [false; 2],
            zoomed: [None; 2],
            pick: None,
            drawn: Vec::new(),
            visible: Vec::new(),
            cull: true,
            gpu_pick: None,
            hover: None,
            press: None,
            view_axes: Axes::Both,
            drag: Drag::Pan,
            dragging: None,
            home: None,
            brush: None,
            brush_handlers: Vec::new(),
            marquee: None,
            gpu_mode: false,
            gpu_hover: GpuHover::default(),
            row_handlers: Vec::new(),
            last_at: Point::new(0.0, 0.0),
            overlay: None,
        }
    }

    /// Skip (`true`, the default) or draw and pick every chunk, whether or
    /// not it can reach the plot rect. For tests and benchmarks: the image
    /// is the same either way (RFC-001 S9f).
    #[doc(hidden)]
    pub fn chunk_culling(&mut self, on: bool) -> &mut Self {
        self.revision = self.revision.wrapping_add(1);
        self.cull = on;
        self
    }

    /// Set the title.
    pub fn title(&mut self, title: impl Into<String>) -> &mut Self {
        self.revision = self.revision.wrapping_add(1);
        self.title = Some(title.into());
        self
    }

    /// Set the x axis title, centred under the x tick labels.
    pub fn x_title(&mut self, title: impl Into<String>) -> &mut Self {
        self.revision = self.revision.wrapping_add(1);
        self.x_title = Some(title.into());
        self
    }

    /// Set the y axis title: horizontal, left-aligned above the y axis.
    pub fn y_title(&mut self, title: impl Into<String>) -> &mut Self {
        self.revision = self.revision.wrapping_add(1);
        self.y_title = Some(title.into());
        self
    }

    /// Draw the colour legends beside the plot (default), or not: with
    /// `false` the plot rect is as if there were none, for callers who
    /// place their own.
    pub fn legends(&mut self, on: bool) -> &mut Self {
        self.revision = self.revision.wrapping_add(1);
        self.legends_off = !on;
        self
    }

    /// Style the plot with `theme`.
    pub fn theme(&mut self, theme: Theme) -> &mut Self {
        self.revision = self.revision.wrapping_add(1);
        self.theme = theme;
        self
    }

    /// The title, if set.
    pub fn title_text(&self) -> Option<&str> {
        self.title.as_deref()
    }

    /// Use `scale` for the x axis and return a handle to encode with.
    /// Closures on x channels (`.x(|r| r.t)`) re-bind to the new scale at
    /// the next resolve; an explicit encoding through another handle is
    /// drawn against this axis as it is.
    pub fn x<S: PositionScale>(&mut self, scale: S) -> ScaleRef<S> {
        self.revision = self.revision.wrapping_add(1);
        let handle = ScaleRef::new(scale);
        self.x = Box::new(handle.clone());
        self.axes_generation += 1;
        handle
    }

    /// Use `scale` for the y axis and return a handle to encode with.
    pub fn y<S: PositionScale>(&mut self, scale: S) -> ScaleRef<S> {
        self.revision = self.revision.wrapping_add(1);
        let handle = ScaleRef::new(scale);
        self.y = Box::new(handle.clone());
        self.axes_generation += 1;
        handle
    }

    /// Add a layer of circles over `rows` and return it.
    pub fn circles<T: Send + Sync + 'static>(
        &mut self,
        rows: impl IntoIterator<Item = T>,
    ) -> &mut Selection<T, Circle> {
        self.add(Selection::new(rows.into_iter().collect::<Vec<_>>()))
    }

    /// Add a layer of rectangles over `rows` and return it.
    pub fn rects<T: Send + Sync + 'static>(
        &mut self,
        rows: impl IntoIterator<Item = T>,
    ) -> &mut Selection<T, MarkRect> {
        self.add(Selection::new(rows.into_iter().collect::<Vec<_>>()))
    }

    /// Add a layer of line segments over `rows` and return it.
    pub fn segments<T: Send + Sync + 'static>(
        &mut self,
        rows: impl IntoIterator<Item = T>,
    ) -> &mut Selection<T, Segment> {
        self.add(Selection::new(rows.into_iter().collect::<Vec<_>>()))
    }

    /// Add one polyline through `rows`, in order, and return its layer.
    pub fn line<T: Send + Sync + 'static>(
        &mut self,
        rows: impl IntoIterator<Item = T>,
    ) -> &mut Selection<T, Line> {
        self.add(Selection::new(rows.into_iter().collect::<Vec<_>>()))
    }

    /// Add a data layer and return it for further `attr` calls.
    pub fn add<T: Send + Sync + 'static, M: Mark>(
        &mut self,
        layer: Selection<T, M>,
    ) -> &mut Selection<T, M> {
        self.revision = self.revision.wrapping_add(1);
        self.layers.push(Box::new(layer));
        self.layers
            .last_mut()
            .and_then(|l| l.as_any_mut().downcast_mut())
            .expect("the layer just pushed has this type")
    }

    /// The first layer of type `Selection<T, M>`, or `None`.
    pub fn layer_mut<T: Send + Sync + 'static, M: Mark>(&mut self) -> Option<&mut Selection<T, M>> {
        self.revision = self.revision.wrapping_add(1);
        self.layers
            .iter_mut()
            .find_map(|l| l.as_any_mut().downcast_mut())
    }

    /// The layer at `index`, as a test reaches it (counts as a change).
    #[cfg(test)]
    fn layer_at_mut<T: Send + Sync + 'static, M: Mark>(
        &mut self,
        index: usize,
    ) -> &mut Selection<T, M> {
        self.revision = self.revision.wrapping_add(1);
        self.layers[index]
            .as_any_mut()
            .downcast_mut()
            .expect("the layer has this type")
    }

    /// Zoom both position scales by `factor` about the logical-pixel point
    /// `at` (`factor < 1` zooms in). The values under `at` stay where they
    /// are; a log scale zooms in log space. Only the scales' domains
    /// change, so the next [`resolve`](Self::resolve) writes uniforms and
    /// no column bytes. Does nothing before the first resolve has fitted
    /// and ranged the scales.
    pub fn zoom(&mut self, at: Point, factor: f64) -> Result<()> {
        self.zoom_axes(at, factor, [true, true])
    }

    fn zoom_axes(&mut self, at: Point, factor: f64, which: [bool; 2]) -> Result<()> {
        self.revision = self.revision.wrapping_add(1);
        if !(factor.is_finite() && factor > 0.0) {
            return Err(Error::config(
                "zoom factor",
                format!("{factor} must be finite and positive"),
            ));
        }
        self.record_home();
        if which[0] {
            self.x.zoom(f64::from(at.x), factor)?;
        }
        if which[1] {
            self.y.zoom(f64::from(at.y), factor)?;
        }
        self.record_zoomed();
        Ok(())
    }

    /// Move the view so the content moves `(dx, dy)` logical pixels, on
    /// both axes (like [`zoom`](Self::zoom), a direct call ignores
    /// [`view_axes`](Self::view_axes)). Each domain becomes the values now
    /// drawn that far before the range ends, so a log scale pans in log
    /// space. Does nothing before the first resolve has fitted and ranged
    /// the scales; a band or point axis is an error.
    pub fn pan(&mut self, dx: f32, dy: f32) -> Result<()> {
        let base = [self.x.current_domain(), self.y.current_domain()];
        self.pan_from(base, (dx, dy), [true, true])
    }

    /// Set the `which` axes to `base` panned by `d` pixels.
    fn pan_from(
        &mut self,
        base: [Option<(f64, f64)>; 2],
        d: (f32, f32),
        which: [bool; 2],
    ) -> Result<()> {
        self.revision = self.revision.wrapping_add(1);
        self.record_home();
        let axes = [(&self.x, d.0), (&self.y, d.1)];
        for (i, (axis, dpx)) in axes.into_iter().enumerate() {
            if let (true, Some((d0, d1))) = (which[i], base[i]) {
                axis.set_domain(d0, d1)?;
                axis.pan(f64::from(dpx))?;
            }
        }
        self.record_zoomed();
        Ok(())
    }

    /// Remember a widened axis's domain after a view change (see `zoomed`).
    fn record_zoomed(&mut self) {
        for (i, axis) in [&self.x, &self.y].into_iter().enumerate() {
            self.zoomed[i] = Some(axis)
                .filter(|_| self.widened[i])
                .and_then(|a| a.current_domain());
        }
    }

    /// Note how the axes are before the first view change.
    fn record_home(&mut self) {
        if self.home.is_none() {
            let home = |s: &dyn DynPositionScale| match s.current_domain() {
                Some((a, b)) if !s.is_auto() => Home::Fixed(a, b),
                _ => Home::Auto,
            };
            self.home = Some([home(&*self.x), home(&*self.y)]);
        }
    }

    /// Back to the domains before the first zoom or pan: automatic axes fit
    /// the data again, explicit ones get their domain back. Does nothing
    /// (and invalidates nothing) when the view has not moved.
    pub fn reset_view(&mut self) -> &mut Self {
        let Some(home) = self.home.take() else {
            return self;
        };
        for (axis, home) in [&self.x, &self.y].into_iter().zip(home) {
            match home {
                Home::Auto => axis.clear_domain(),
                // The domain was valid when it was recorded.
                Home::Fixed(a, b) => drop(axis.set_domain(a, b)),
            }
        }
        self.zoomed = [None; 2];
        self.dragging = None;
        self.press = None;
        self.hold = None;
        self.revision = self.revision.wrapping_add(1);
        self
    }

    /// Axes the wheel zooms and a drag pans (default both). A band or
    /// point axis is never moved by input.
    ///
    /// ```
    /// use gup_core::geom::Point;
    /// use gup_core::prelude::*;
    /// use gup_core::{Button, Input};
    ///
    /// let cx = Context::new_blocking()?;
    /// let mut plot = Plot::new();
    /// let (x, y) = (plot.x(Linear::new()), plot.y(Linear::new()));
    /// plot.add(Selection::<(f64, f64), Circle>::new(vec![(1.0, 1.0), (9.0, 3.0)]))
    ///     .attr(Circle::X, x.encode(|r: &(f64, f64)| r.0))
    ///     .attr(Circle::Y, y.encode(|r: &(f64, f64)| r.1))
    ///     .attr(Circle::RADIUS, Px(4.0));
    /// plot.view_axes(Axes::X);
    /// let r = plot.resolve(&cx, 400.0, 300.0)?.layout.plot;
    /// let (dx, dy) = (x.read().current_domain(), y.read().current_domain());
    /// let p = Point::new(r.x + 50.0, r.y + 50.0);
    /// let button = Button::Primary;
    /// plot.handle(Input::PointerDown { at: p, button });
    /// plot.handle(Input::PointerMoved { at: Point::new(p.x + 20.0, p.y + 10.0) });
    /// let moved = plot.handle(Input::PointerMoved { at: Point::new(p.x + 40.0, p.y + 20.0) });
    /// plot.handle(Input::PointerUp { at: Point::new(p.x + 40.0, p.y + 20.0), button });
    /// assert!(moved.redraw);
    /// assert_ne!(x.read().current_domain(), dx);
    /// assert_eq!(y.read().current_domain(), dy);
    /// # Ok::<(), gup_core::Error>(())
    /// ```
    pub fn view_axes(&mut self, axes: Axes) -> &mut Self {
        if self.view_axes != axes {
            self.view_axes = axes;
            self.revision = self.revision.wrapping_add(1);
        }
        self
    }

    /// What a primary-button drag does (default [`Drag::Pan`]).
    pub fn drag(&mut self, drag: Drag) -> &mut Self {
        if self.drag != drag {
            self.drag = drag;
            self.revision = self.revision.wrapping_add(1);
        }
        self
    }

    /// The axes input may move: [`view_axes`](Self::view_axes) less band and
    /// point axes.
    fn movable(&self) -> [bool; 2] {
        let [x, y] = self.view_axes.flags();
        [x && !self.x.reads_keys(), y && !self.y.reads_keys()]
    }

    /// The data values drawn at `at` (logical pixels) as of the current
    /// scales: `None` before the first resolve or outside the plot rect
    /// (edges inclusive). Band and point axes give a code.
    ///
    /// ```
    /// use gup_core::geom::Point;
    /// use gup_core::prelude::*;
    ///
    /// let cx = Context::new_blocking()?;
    /// let mut plot = Plot::new();
    /// let (x, y) = (
    ///     plot.x(Linear::new()),
    ///     plot.y(Linear::new()),
    /// );
    /// plot.add(Selection::<(f64, f64), Circle>::new(vec![(1.0, 1.0), (9.0, 3.0)]))
    ///     .attr(Circle::X, x.encode(|r: &(f64, f64)| r.0))
    ///     .attr(Circle::Y, y.encode(|r: &(f64, f64)| r.1))
    ///     .attr(Circle::RADIUS, Px(4.0));
    /// assert!(plot.invert(Point::new(100.0, 100.0)).is_none());
    /// let r = plot.resolve(&cx, 400.0, 300.0)?.layout.plot;
    /// let c = Point::new(r.x + r.width / 2.0, r.y + r.height / 2.0);
    /// let (vx, vy) = plot.invert(c).unwrap();
    /// // Fitted domains are widened, so the centre is the domain's midpoint.
    /// let mid = |s: &ScaleRef<Linear>| {
    ///     let (a, b) = s.read().current_domain().unwrap();
    ///     (a + b) / 2.0
    /// };
    /// let (mx, my) = (mid(&x), mid(&y));
    /// assert!((vx - mx).abs() <= 1e-9 * mx.abs().max(1.0), "{vx} {mx}");
    /// assert!((vy - my).abs() <= 1e-9 * my.abs().max(1.0), "{vy} {my}");
    /// assert!(plot.invert(Point::new(r.x - 1.0, c.y)).is_none());
    /// # Ok::<(), gup_core::Error>(())
    /// ```
    pub fn invert(&self, at: Point) -> Option<(f64, f64)> {
        let plot = self.layout()?.plot;
        let inside = (plot.left()..=plot.right()).contains(&at.x)
            && (plot.top()..=plot.bottom()).contains(&at.y);
        inside.then(|| {
            (
                self.x.invert(f64::from(at.x)),
                self.y.invert(f64::from(at.y)),
            )
        })
    }

    /// Resolve domains, ticks, text, the plot rect and GPU state for a
    /// `width × height` logical-pixel chart.
    ///
    /// Cached: when nothing changed since the last successful resolve
    /// (same context, size, plot settings, layers and scale writes) this
    /// returns the previous result and does no work and no GPU writes.
    pub fn resolve(&mut self, cx: &Context, width: f32, height: f32) -> Result<&Resolved> {
        let key = |plot: &Self| CacheKey {
            context: cx.id(),
            width: width.to_bits(),
            height: height.to_bits(),
            revisions: plot.revision_sum(),
        };
        if self.cache.as_ref().is_some_and(|(k, _)| *k == key(self)) {
            // A browser reports errors late.
            cx.take_gpu_errors()?;
        } else {
            self.cache = None;
            if self
                .hold
                .is_some_and(|h| (h.width, h.height) != (key(self).width, key(self).height))
            {
                self.hold = None;
            }
            let resolved = self.resolve_full(cx, width, height)?;
            self.generation += 1;
            // After resolving: it writes the scales.
            self.cache = Some((key(self), resolved));
        }
        self.gpu_step(cx)?;
        self.sync_overlay(cx);
        match (&self.overlay, &self.cache) {
            (Some((g, composed)), _) if *g == self.generation => Ok(composed),
            (_, Some((_, resolved))) => Ok(resolved),
            _ => unreachable!("the cache was just filled"),
        }
    }

    /// Apply finished GPU hover and click picks, then submit the pending
    /// ones (one hover and one click in flight at a time).
    fn gpu_step(&mut self, cx: &Context) -> Result<()> {
        self.gpu_mode = self
            .layers
            .iter()
            .any(|l| l.gpu_pickable() && !l.cpu_readable());
        if !self.gpu_mode {
            self.gpu_hover = GpuHover::default();
            return Ok(());
        }
        if !self.gpu_hover.busy() {
            return Ok(());
        }
        for pick in self.poll_picks(cx) {
            if Some(pick.ticket) == self.gpu_hover.hover_ticket {
                self.gpu_hover.hover_ticket = None;
                if pick.generation != self.generation {
                    // The view changed since: look again where the pointer is.
                    if self.gpu_hover.hover.is_none()
                        && self.dragging.is_none()
                        && self.marquee.is_none()
                    {
                        self.gpu_hover.hover = Some(self.last_at);
                    }
                } else if self.gpu_hover.hover.is_none() && pick.row != self.hovered() {
                    self.set_hover(pick.row, pick.at);
                }
            } else if Some(pick.ticket) == self.gpu_hover.click_ticket {
                // The user clicked what they saw: applied whatever the view
                // is now.
                self.gpu_hover.click_ticket = None;
                if let Some(row) = pick.row {
                    self.dispatch(Event::Click, pick.at, row);
                }
            }
        }
        if self.gpu_hover.hover_ticket.is_none()
            && let Some(at) = self.gpu_hover.hover.take()
        {
            self.gpu_hover.hover_ticket = Some(self.request_pick(cx, at)?);
        }
        if self.gpu_hover.click_ticket.is_none() && !self.gpu_hover.clicks.is_empty() {
            let at = self.gpu_hover.clicks.remove(0);
            self.gpu_hover.click_ticket = Some(self.request_pick(cx, at)?);
        }
        Ok(())
    }

    /// Bring the hover and the composed overlay scene (brush and tooltip)
    /// up to date with the base resolve: re-anchor after a full resolve, and
    /// compose the overlay when it is missing. Costs nothing when neither
    /// changed.
    fn sync_overlay(&mut self, cx: &Context) {
        if let Some((row, anchor, at_gen)) = self.hover
            && at_gen != self.generation
        {
            self.overlay = None;
            match self.pick_anchor(row) {
                Some(a) => self.hover = Some((row, a, self.generation)),
                // A GPU-picked row has no anchor but the pointer's.
                None if self.gpu_mode => self.hover = Some((row, anchor, self.generation)),
                None => self.hover = None,
            }
        }
        if self.hover.is_none() && self.brush.is_none() && self.marquee.is_none() {
            self.overlay = None;
            return;
        }
        if self
            .overlay
            .as_ref()
            .is_some_and(|(g, _)| *g == self.generation)
        {
            return;
        }
        self.overlay = None;
        let brush_px = self.marquee.or_else(|| self.brush_rect());
        let tooltip = self.hover.and_then(|(row, anchor, _)| {
            let text = self.layers.get(row.layer)?.tooltip_text(row.row)?;
            Some((text, anchor))
        });
        let Some((_, base)) = &self.cache else {
            return;
        };
        if brush_px.is_none() && tooltip.is_none() {
            return;
        }
        let mut composed = base.clone();
        if let Some(rect) = brush_px {
            composed.scene.push(brush_item(&self.theme, rect));
        }
        if let Some((text, anchor)) = tooltip {
            let (width, height) = (base.layout.width, base.layout.height);
            let (rect, items) =
                crate::tooltip::overlay(&cx.text(), &self.theme, &text, anchor, width, height);
            for item in items {
                composed.scene.push(item);
            }
            composed.layout.tooltip = Some(rect);
        }
        self.overlay = Some((self.generation, composed));
    }

    /// The layout of the last successful resolve (with the tooltip, while
    /// one is shown).
    pub fn layout(&self) -> Option<&Layout> {
        match (&self.overlay, &self.cache) {
            (Some((g, composed)), _) if *g == self.generation => Some(&composed.layout),
            (_, Some((_, r))) => Some(&r.layout),
            _ => None,
        }
    }

    /// The row the pointer is over, as of the last
    /// [`handle`](Self::handle).
    pub fn hovered(&self) -> Option<RowId> {
        self.hover.map(|(row, _, _)| row)
    }

    /// React to `input`.
    ///
    /// - Moving the pointer onto another row (or off the rows) calls the
    ///   `Leave` handlers of the old row, then the `Enter` handlers of the
    ///   new one, and updates [`hovered`](Self::hovered).
    /// - The primary button going down and up within a few pixels calls the
    ///   `Click` handlers of the row under the pointer.
    /// - A wheel turn zooms about the pointer ([`zoom`](Self::zoom)),
    ///   leaving the hovered row first.
    /// - A primary-button drag that goes beyond the click distance pans
    ///   ([`pan`](Self::pan)): the values under the pointer at the press stay
    ///   under it. The hovered row is left when the drag starts, and the
    ///   release is not a click. [`drag`](Self::drag)(`Drag::Off`) turns this
    ///   off.
    ///
    /// The wheel and a drag move the axes of [`view_axes`](Self::view_axes)
    /// except band and point axes; [`reset_view`](Self::reset_view) undoes
    /// them.
    ///
    /// `redraw` is true when the tooltip shown changes (a hovered row of a
    /// layer with a [`tooltip`](Selection::tooltip), or leaving one) or the
    /// scales moved. Handlers need the rows: a layer that dropped them
    /// ([`Retain`](crate::Retain)) runs none.
    pub fn handle(&mut self, input: Input) -> Response {
        let mut redraw = false;
        match input {
            Input::PointerMoved { at } => {
                self.last_at = at;
                let which = self.movable();
                let drags = self.drag == Drag::Pan && (which[0] || which[1]);
                if let (Some(down), true, true) = (self.press, drags, self.cache.is_some()) {
                    if self.dragging.is_none() && (at.x - down.x).hypot(at.y - down.y) > CLICK_SLOP
                    {
                        self.set_hover(None, at);
                        self.gpu_hover.cancel_hover();
                        self.hold_layout();
                        self.dragging = Some([self.x.current_domain(), self.y.current_domain()]);
                    }
                    if let Some(base) = self.dragging {
                        redraw = self
                            .pan_from(base, (at.x - down.x, at.y - down.y), which)
                            .is_ok();
                    }
                }
                if let (Some(down), true, true) = (
                    self.press,
                    matches!(self.drag, Drag::Brush | Drag::Zoom),
                    self.cache.is_some(),
                ) && (self.marquee.is_some()
                    || (at.x - down.x).hypot(at.y - down.y) > CLICK_SLOP)
                {
                    if self.marquee.is_none() {
                        self.set_hover(None, at);
                        self.gpu_hover.cancel_hover();
                    }
                    self.marquee = self.clamped_rect(down, at);
                    self.overlay = None;
                    redraw = true;
                }
                if self.dragging.is_none() && self.marquee.is_none() {
                    if self.gpu_mode {
                        // Picked at the next resolve; the latest point wins.
                        self.gpu_hover.hover = Some(at);
                        redraw = true;
                    } else {
                        let row = self.pick_row(at);
                        if row != self.hovered() {
                            redraw = self.set_hover(row, at);
                        }
                    }
                }
            }
            Input::PointerLeft => {
                redraw = self.set_hover(None, self.last_at);
                self.gpu_hover.cancel_hover();
                self.press = None;
                self.dragging = None;
                if self.marquee.take().is_some() {
                    self.overlay = None;
                    redraw = true;
                }
                if self.hold.take().is_some() {
                    self.revision = self.revision.wrapping_add(1);
                    redraw = true;
                }
            }
            Input::PointerDown {
                at,
                button: Button::Primary,
            } => {
                self.last_at = at;
                self.press = Some(at);
            }
            Input::PointerUp {
                at,
                button: Button::Primary,
            } => {
                self.last_at = at;
                let dragged = self.dragging.take().is_some();
                let down = self.press.take();
                if let Some(down) = down
                    && self.marquee.take().is_some()
                {
                    self.overlay = None;
                    redraw = true;
                    match self.drag {
                        Drag::Brush => redraw |= self.finish_brush(down, at),
                        Drag::Zoom => redraw |= self.finish_zoom(down, at),
                        Drag::Pan | Drag::Off => {}
                    }
                } else if let Some(down) = down
                    && !dragged
                {
                    let (dx, dy) = (at.x - down.x, at.y - down.y);
                    if dx.hypot(dy) <= CLICK_SLOP {
                        redraw |= self.drop_brush();
                        if self.gpu_mode {
                            self.gpu_hover.clicks.push(at);
                            redraw = true;
                        } else if let Some(row) = self.pick_row(at) {
                            self.dispatch(Event::Click, at, row);
                        }
                    }
                }
            }
            Input::Wheel { at, lines }
                if lines.is_finite() && lines != 0.0 && self.cache.is_some() =>
            {
                self.last_at = at;
                let which = self.movable();
                if which[0] || which[1] {
                    self.set_hover(None, at);
                    self.gpu_hover.cancel_hover();
                    self.hold_layout();
                    redraw = self
                        .zoom_axes(at, ZOOM_PER_LINE.powf(f64::from(lines)), which)
                        .is_ok();
                }
            }
            _ => {}
        }
        Response { redraw }
    }

    /// The rectangle from `a` to `b` clamped to the plot rect.
    fn clamped_rect(&self, a: Point, b: Point) -> Option<Rect> {
        let plot = self.layout()?.plot;
        let cx = |v: f32| v.clamp(plot.left(), plot.right());
        let cy = |v: f32| v.clamp(plot.top(), plot.bottom());
        Some(Rect::from_edges(
            cx(a.x.min(b.x)),
            cy(a.y.min(b.y)),
            cx(a.x.max(b.x)),
            cy(a.y.max(b.y)),
        ))
    }

    /// Release in `Brush` mode: set the brush from the rectangle, or clear
    /// it when the rectangle is empty. Whether the overlay changed.
    fn finish_brush(&mut self, down: Point, up: Point) -> bool {
        let Some(r) = self.clamped_rect(down, up) else {
            return false;
        };
        if r.width <= 0.0 || r.height <= 0.0 {
            self.drop_brush();
            return true;
        }
        let Some(rows) = self.pick_index().map(|i| i.rows_in(r)) else {
            return false;
        };
        let (x0, x1) = (
            self.x.invert(f64::from(r.left())),
            self.x.invert(f64::from(r.right())),
        );
        let (y0, y1) = (
            self.y.invert(f64::from(r.top())),
            self.y.invert(f64::from(r.bottom())),
        );
        self.brush = Some(Brush {
            x: (x0.min(x1), x0.max(x1)),
            y: (y0.min(y1), y0.max(y1)),
            rows,
        });
        self.overlay = None;
        self.call_brush_handlers();
        true
    }

    /// Release in `Zoom` mode: the view axes take the rectangle's extent
    /// when it is wider and taller than the click slop.
    fn finish_zoom(&mut self, down: Point, up: Point) -> bool {
        let Some(r) = self.clamped_rect(down, up) else {
            return false;
        };
        let which = self.movable();
        if !(which[0] || which[1]) || r.width <= CLICK_SLOP || r.height <= CLICK_SLOP {
            return false;
        }
        self.hold_layout();
        self.record_home();
        let ends = [
            (r.left(), r.right()),
            // The y range runs bottom to top.
            (r.bottom(), r.top()),
        ];
        for (i, axis) in [&self.x, &self.y].into_iter().enumerate() {
            if which[i] {
                let (p0, p1) = ends[i];
                // A rectangle inside the plot is a valid domain.
                drop(axis.set_domain(axis.invert(f64::from(p0)), axis.invert(f64::from(p1))));
            }
        }
        self.record_zoomed();
        self.revision = self.revision.wrapping_add(1);
        true
    }

    fn call_brush_handlers(&mut self) {
        for handler in &mut self.brush_handlers {
            handler(self.brush.as_ref());
        }
    }

    /// Clear the brush, calling the handlers if there was one. Whether it
    /// did.
    fn drop_brush(&mut self) -> bool {
        if self.brush.take().is_none() {
            return false;
        }
        self.overlay = None;
        self.call_brush_handlers();
        true
    }

    /// The current brush, if any (see [`Drag::Brush`]).
    ///
    /// ```
    /// use gup_core::geom::Point;
    /// use gup_core::prelude::*;
    /// use gup_core::{Button, Input, RowId};
    ///
    /// struct Row { id: u32, x: f64, y: f64 }
    /// # fn main() -> gup_core::Result<()> {
    /// let rows = vec![
    ///     Row { id: 10, x: 1.0, y: 1.0 },
    ///     Row { id: 11, x: 2.0, y: 2.0 },
    ///     Row { id: 12, x: 3.0, y: 3.0 },
    /// ];
    /// let cx = Context::new_blocking()?;
    /// let mut plot = Plot::new();
    /// plot.circles(rows).x(|r| r.x).y(|r| r.y);
    /// plot.drag(Drag::Brush);
    /// let layout = plot.resolve(&cx, 320.0, 200.0)?.layout.clone();
    /// let r = layout.plot;
    /// // Press at the bottom-left, drag to the middle of the plot: the
    /// // two lower-left points are inside.
    /// let from = Point::new(r.left() + 1.0, r.bottom() - 1.0);
    /// let to = Point::new(r.left() + r.width * 0.62, r.top() + r.height * 0.38);
    /// let button = Button::Primary;
    /// plot.handle(Input::PointerDown { at: from, button });
    /// plot.handle(Input::PointerMoved { at: to });
    /// plot.handle(Input::PointerUp { at: to, button });
    /// let brush = plot.brush().expect("a brush");
    /// let ids: Vec<u32> = brush
    ///     .rows
    ///     .iter()
    ///     .map(|&id| plot.row::<Row>(id).unwrap().id)
    ///     .collect();
    /// assert_eq!(ids, [10, 11]);
    /// # Ok(())
    /// # }
    /// ```
    pub fn brush(&self) -> Option<&Brush> {
        self.brush.as_ref()
    }

    /// Call `f` with the new brush on release, and with `None` when the
    /// brush is cleared (a click, or [`clear_brush`](Self::clear_brush)).
    /// Handlers run in registration order.
    pub fn on_brush(&mut self, f: impl FnMut(Option<&Brush>) + Send + Sync + 'static) -> &mut Self {
        self.brush_handlers.push(Box::new(f));
        self
    }

    /// Remove the brush, calling the handlers with `None` only if there
    /// was one.
    pub fn clear_brush(&mut self) -> &mut Self {
        self.drop_brush();
        self
    }

    /// The brush's rectangle under the current scales (logical pixels),
    /// clipped to the plot rect; `None` without a brush, before the first
    /// resolve, or when it is out of view.
    pub fn brush_rect(&self) -> Option<Rect> {
        let b = self.brush.as_ref()?;
        let plot = self.layout()?.plot;
        let px = |s: &dyn DynPositionScale, (a, b): (f64, f64)| {
            let (a, b) = (s.eval(a) as f32, s.eval(b) as f32);
            (a.min(b), a.max(b))
        };
        let (x0, x1) = px(&*self.x, b.x);
        let (y0, y1) = px(&*self.y, b.y);
        let (l, t) = (x0.max(plot.left()), y0.max(plot.top()));
        let (r, bt) = (x1.min(plot.right()), y1.min(plot.bottom()));
        (l <= r && t <= bt).then(|| Rect::from_edges(l, t, r, bt))
    }

    /// Row `id`'s data, if its layer holds rows of type `T` and kept them.
    pub fn row<T: 'static>(&self, id: RowId) -> Option<&T> {
        self.layers
            .get(id.layer)?
            .row_any(id.row)?
            .downcast_ref::<T>()
    }

    /// Hold the cached layout's horizontal margins until the pointer leaves
    /// or the size changes. Does nothing without a cached layout or with a
    /// hold already taken.
    fn hold_layout(&mut self) {
        if self.hold.is_some() {
            return;
        }
        if let Some((key, resolved)) = &self.cache {
            let plot = resolved.layout.plot;
            self.hold = Some(Hold {
                width: key.width,
                height: key.height,
                left: plot.left(),
                right: plot.right(),
            });
        }
    }

    /// Move the hover to `row`: `Leave` the old row, `Enter` the new one.
    /// Whether the tooltip shown changes.
    fn set_hover(&mut self, row: Option<RowId>, at: Point) -> bool {
        let old = self.hover.take();
        // A GPU-picked row without an index entry attaches at the pointer.
        let anchor = row.and_then(|r| self.pick_anchor(r).or(self.gpu_mode.then_some(at)));
        self.hover = row.zip(anchor).map(|(r, a)| (r, a, self.generation));
        self.overlay = None;
        if let Some((old, _, _)) = old {
            self.dispatch(Event::Leave, at, old);
        }
        let new = self.hover.map(|(r, _, _)| r);
        if let Some(new) = new {
            self.dispatch(Event::Enter, at, new);
        }
        let has_tooltip = |plot: &Self, r: Option<RowId>| {
            r.is_some_and(|r| {
                plot.layers
                    .get(r.layer)
                    .is_some_and(|l| l.tooltip_text(r.row).is_some())
            })
        };
        has_tooltip(self, old.map(|(r, _, _)| r)) || has_tooltip(self, new)
    }

    fn dispatch(&mut self, event: Event, at: Point, row: RowId) {
        if let Some(layer) = self.layers.get_mut(row.layer) {
            layer.dispatch(&EventInfo { event, at, row });
        }
        let info = EventInfo { event, at, row };
        for handler in &mut self.row_handlers {
            handler(&info);
        }
    }

    /// Call `f` for every `Enter`, `Leave` and `Click`, with the row's id,
    /// whether or not its layer kept the rows (unlike
    /// [`Selection::on`](crate::Selection::on), which runs no handler for a
    /// layer that dropped them). Handlers run after the layer's own, in
    /// registration order.
    ///
    /// When a plot has a layer the CPU cannot read
    /// ([`Retain::GpuOnly`](crate::Retain::GpuOnly) past one chunk), hover
    /// and click are picked on the GPU, and the handlers run from
    /// [`resolve`](Self::resolve) a frame or two after the input
    /// ([`Chart::needs_frame`](crate::Chart::needs_frame)); then do not call
    /// [`poll_picks`](Self::poll_picks) yourself.
    ///
    /// ```
    /// use gup_core::geom::Point;
    /// use gup_core::prelude::*;
    /// use gup_core::{Event, Input, RowId};
    /// use std::sync::{Arc, Mutex};
    ///
    /// struct Row { x: f64, y: f64 }
    /// # fn main() -> gup_core::Result<()> {
    /// let rows = vec![Row { x: 1.0, y: 1.0 }, Row { x: 2.0, y: 3.0 }];
    /// let cx = Context::new_blocking()?;
    /// let mut plot = Plot::new();
    /// let seen = Arc::new(Mutex::new(Vec::new()));
    /// let log = Arc::clone(&seen);
    /// plot.on_row(move |info| log.lock().unwrap().push((info.event, info.row)));
    /// plot.circles(rows).x(|r| r.x).y(|r| r.y);
    /// plot.resolve(&cx, 320.0, 200.0)?;
    /// let at = plot.pick_anchor(RowId { layer: 0, row: 1 }).expect("drawn");
    /// plot.handle(Input::PointerMoved { at });
    /// assert_eq!(seen.lock().unwrap()[0], (Event::Enter, RowId { layer: 0, row: 1 }));
    /// # Ok(())
    /// # }
    /// ```
    pub fn on_row(&mut self, f: impl FnMut(&EventInfo) + Send + Sync + 'static) -> &mut Self {
        self.row_handlers.push(Box::new(f));
        self
    }

    /// The topmost row drawn under `at` (logical pixels), or `None`: before
    /// the first resolve, outside [`Layout::plot`] (marks are clipped to
    /// it), when nothing is within [`PICK_TOLERANCE`](crate::PICK_TOLERANCE), or when a layer
    /// cannot be read (see below).
    ///
    /// The hit test runs on the CPU over the retained columns, in the same
    /// pixels the marks were drawn at: a disc for `Circle`, an aligned box
    /// for `Rect`, a square-capped stroke for `Segment`, and a chain of
    /// capsules for `Line` (which picks the nearer end of the segment
    /// under `at`). Among marks containing `at` the topmost wins, else the
    /// nearest within tolerance. "Topmost" follows the draw: the layer's
    /// `z`, the layer, then the row's place in the draw order (a layer
    /// sorted by size draws the smallest last). Marks without a declared
    /// hit shape are never picked.
    ///
    /// A layer is read from its CPU columns, else by running its
    /// accessors over its rows; with neither (`Retain::GpuOnly` above one
    /// chunk) its rows are not picked. The index is built on the first
    /// pick after a resolve or a zoom.
    pub fn pick_row(&mut self, at: Point) -> Option<RowId> {
        self.pick_hit(at).map(|h| h.row)
    }

    /// The data of the topmost row drawn under `at` (see
    /// [`pick_row`](Self::pick_row)), if its layer holds rows of type `T`
    /// and kept them ([`Retain::Auto`](crate::Retain::Auto),
    /// [`Rows`](crate::Retain::Rows)). It looks at the topmost row only:
    /// if that is another type, it is `None`.
    ///
    /// ```
    /// use gup_core::prelude::*;
    /// use gup_core::geom::Point;
    ///
    /// struct Row { name: &'static str, x: f64, y: f64 }
    /// # fn main() -> gup_core::Result<()> {
    /// let rows = vec![
    ///     Row { name: "a", x: 1.0, y: 1.0 },
    ///     Row { name: "b", x: 2.0, y: 3.0 },
    ///     Row { name: "c", x: 3.0, y: 2.0 },
    /// ];
    /// let cx = Context::new_blocking()?;
    /// let mut plot = Plot::new();
    /// plot.circles(rows).x(|r| r.x).y(|r| r.y);
    /// let resolved = plot.resolve(&cx, 320.0, 200.0)?;
    /// let layout = &resolved.layout;
    ///
    /// // The second row's position, from the tick pixels.
    /// let at = |ticks: &[(f64, f32)], v: f64| {
    ///     let (a, b) = (ticks[0], ticks[ticks.len() - 1]);
    ///     f64::from(a.1) + (v - a.0) * f64::from(b.1 - a.1) / (b.0 - a.0)
    /// };
    /// let p = Point::new(at(&layout.x_ticks, 2.0) as f32, at(&layout.y_ticks, 3.0) as f32);
    /// let row = plot.pick::<Row>(p).expect("a row is drawn there");
    /// assert_eq!(row.name, "b");
    /// # Ok(())
    /// # }
    /// ```
    pub fn pick<T: 'static>(&mut self, at: Point) -> Option<&T> {
        let id = self.pick_row(at)?;
        self.layers
            .get(id.layer)?
            .row_any(id.row)?
            .downcast_ref::<T>()
    }

    /// Where a tooltip for `row` attaches: a disc's centre, the middle of
    /// a box's top edge, a segment's midpoint, a line vertex.
    #[doc(hidden)]
    pub fn pick_anchor(&mut self, row: RowId) -> Option<Point> {
        self.pick_index()?;
        self.pick.as_ref()?.1.anchor(row)
    }

    /// How many items the CPU pick index holds for the current resolve.
    #[doc(hidden)]
    pub fn pick_item_count(&mut self) -> Option<usize> {
        self.pick_index().map(PickIndex::len)
    }

    fn pick_hit(&mut self, at: Point) -> Option<crate::pick::Hit> {
        self.pick_index()?.query(at)
    }

    /// The pick index for the current resolve, built on first use.
    pub(crate) fn pick_index(&mut self) -> Option<&PickIndex> {
        let plot = self.layout()?.plot;
        let key = (self.generation, self.revision_sum());
        if self
            .pick
            .as_ref()
            .is_none_or(|(k, i)| *k != key || i.rect() != plot)
        {
            self.pick = None;
            let mut items = Vec::new();
            for (i, layer) in self.layers.iter_mut().enumerate() {
                let visible = self.visible.get(i).map_or(&[][..], Vec::as_slice);
                layer.hit_items(i, visible, &mut items).ok()?;
            }
            self.pick = Some((key, PickIndex::new(plot, items)));
        }
        self.pick.as_ref().map(|(_, i)| i)
    }

    /// How many full (uncached) resolves have succeeded. Internal: tests
    /// and pick indexes use it to see that a resolve did work.
    #[doc(hidden)]
    pub fn resolve_generation(&self) -> u64 {
        self.generation
    }

    /// The sum of every counter that rises when something the resolve
    /// reads changes.
    fn revision_sum(&self) -> u64 {
        let mut sum = self.revision;
        for scale in [&self.x, &self.y] {
            sum = sum.wrapping_add(scale.revision());
        }
        for layer in &self.layers {
            sum = sum.wrapping_add(layer.revision());
        }
        sum
    }

    fn resolve_full(&mut self, cx: &Context, width: f32, height: f32) -> Result<Resolved> {
        // Errors that a browser reported after an earlier call returned.
        cx.take_gpu_errors()?;
        if self.layers.is_empty() {
            return Err(Error::config(
                "plot layers",
                "the plot has no layers; add one with `plot.add(..)`",
            ));
        }
        for (i, layer) in self.layers.iter().enumerate() {
            if !(Z_GRID + 1..Z_AXES).contains(&layer.z()) {
                return Err(Error::config(
                    "layer z",
                    format!(
                        "layer {i} has z {}; marks must stay between the grid and the axes, \
                         so z is in {}..={}",
                        layer.z(),
                        Z_GRID + 1,
                        Z_AXES - 1
                    ),
                ));
            }
        }

        for layer in &mut self.layers {
            layer.bind_axes(self.axes_generation, self.x.as_ref(), self.y.as_ref())?;
        }
        let (x, y) = (&self.x, &self.y);

        let th = self.theme.clone();
        for layer in &mut self.layers {
            layer.apply_theme(&th);
        }

        // 1–2. Domains from column stats, then nice.
        // Every scale clears its union once, then every layer fits into it.
        for layer in &self.layers {
            layer.begin_fit();
        }
        for layer in &mut self.layers {
            layer.fit_domains(cx)?;
        }
        for s in [x, y] {
            if s.is_auto() {
                s.nice();
            }
        }

        // Legends read the fitted scales (a categorical's keys).
        let legends = if self.legends_off {
            Vec::new()
        } else {
            crate::guide::dedup(self.layers.iter().flat_map(|l| l.legends()))
        };

        // 3. Ticks and label measurements give the margins.
        let tick_style = TextStyle {
            size: Px(th.tick_size.0),
            color: th.label,
            weight: Weight::Regular,
        };
        let legend_w = if legends.is_empty() {
            0.0
        } else {
            crate::guide::column_width(&cx.text(), &th, &legends) + crate::guide::COLUMN_GAP
        };
        let (title_h, axis_title_h, x_label_h) = {
            let text = cx.text();
            let axis_title_h = {
                let m = text.measure("0", th.axis_title_size.0);
                m.cap_height + m.descent
            };
            let title_h = self.title.as_ref().map_or(0.0, |t| {
                let m = text.measure(t, th.title_size.0);
                m.cap_height + m.descent
            });
            let m = text.measure("0", th.tick_size.0);
            (title_h, axis_title_h, m.cap_height + m.descent)
        };
        let (has_title, has_x_title, has_y_title) = (
            self.title.is_some(),
            self.x_title.is_some(),
            self.y_title.is_some(),
        );
        // 4. The plot rect, on whole pixels, from the ticks' label sizes.
        // A hold for this size keeps each horizontal margin unless a label
        // would not fit even with the outer padding (GUP-452).
        let held = self
            .hold
            .filter(|h| (h.width, h.height) == (width.to_bits(), height.to_bits()));
        let layout = |x_count: usize, y_count: usize| -> Result<TickLayout> {
            let x_ticks = x.ticks(x_count);
            let y_ticks = y.ticks(y_count);
            let text = cx.text();
            let width_of = |ticks: &Ticks, i: usize| {
                text.measure_weight(&ticks.labels[i], th.tick_size.0, ticks.weight(i))
                    .width
            };
            let y_label_w = (0..y_ticks.labels.len())
                .map(|i| width_of(&y_ticks, i))
                .fold(0.0, f32::max);
            let last_x_label_w = x_ticks
                .labels
                .len()
                .checked_sub(1)
                .map_or(0.0, |i| width_of(&x_ticks, i));
            let top = th.pad.0
                + if has_title { title_h + th.pad.0 } else { 0.0 }
                + if has_y_title {
                    axis_title_h + th.label_gap.0
                } else {
                    0.0
                };
            let mut left = th.pad.0 + y_label_w + th.label_gap.0 + th.tick_len.0;
            if let Some(h) = held
                && left - th.pad.0 <= h.left
            {
                left = h.left;
            }
            let x_title_h = if has_x_title {
                axis_title_h + th.label_gap.0
            } else {
                0.0
            };
            let bottom =
                height - (th.pad.0 + x_label_h + th.label_gap.0 + th.tick_len.0 + x_title_h);
            let mut right = width - th.pad.0.max(last_x_label_w / 2.0 + 2.0) - legend_w;
            if let Some(h) = held
                && (width - right) - th.pad.0 <= width - h.right
            {
                right = h.right;
            }
            let plot = Rect::from_edges(left.round(), top.round(), right.round(), bottom.round());
            if plot.width < 10.0 || plot.height < 10.0 {
                return Err(Error::config(
                    "plot size",
                    format!("{width}×{height} leaves no room for the plot area"),
                ));
            }
            Ok(TickLayout {
                x_ticks,
                y_ticks,
                y_label_w,
                plot,
            })
        };
        // 5. Ranges, inset so marks on the domain edge stay inside.
        let mark_overhang = self.layers.iter().map(|l| l.overhang()).fold(0.0, f32::max);
        let inset = mark_overhang + th.plot_padding.0;
        // A widened axis already holds the inset in its domain, so its
        // range is the whole plot rect. Any other axis insets only its far
        // end (right, top), so the axes meet at the corner and a baseline
        // sits on its axis.
        let set_ranges = |plot: Rect, widened: (bool, bool)| {
            let (ix, iy) = (
                if widened.0 { 0.0 } else { inset },
                if widened.1 { 0.0 } else { inset },
            );
            x.set_range(Px(plot.left()), Px(plot.right() - ix));
            y.set_range(Px(plot.bottom()), Px(plot.top() + iy));
        };
        let mut counts = (
            ((width / th.x_tick_spacing.0) as usize).max(2),
            ((height / th.y_tick_spacing.0) as usize).max(2),
        );
        let mut lay = layout(counts.0, counts.1)?;
        // Automatic domains grow in data units to cover the overhang and
        // padding, so the ticks run to both ends of each axis (GUP-435).
        // A zoom keeps the policy its axis had (see `Plot::zoomed`).
        let kept = |i: usize, s: &dyn DynPositionScale| {
            !s.is_auto() && self.zoomed[i].is_some() && self.zoomed[i] == s.current_domain()
        };
        let widened = (
            x.widen(lay.plot.left(), lay.plot.right(), mark_overhang) || kept(0, &**x),
            y.widen(lay.plot.bottom(), lay.plot.top(), mark_overhang) || kept(1, &**y),
        );
        self.widened = [widened.0, widened.1];
        if widened.0 || widened.1 {
            // Tick values stay on the rounded numbers; the margins may change.
            lay = layout(counts.0, counts.1)?;
        }
        set_ranges(lay.plot, widened);
        // Thin ticks until no two labels collide; relayout at most once.
        let gap = th.label_gap.0;
        let thin = |lay: &mut TickLayout, counts: &mut (usize, usize)| {
            let text = cx.text();
            let collides = |ticks: &Ticks, s: &dyn DynPositionScale, vertical: bool| {
                let mut boxes: Vec<(f32, f32)> = ticks
                    .values
                    .iter()
                    .zip(&ticks.labels)
                    .enumerate()
                    .map(|(i, (v, l))| {
                        let c = s.eval(*v) as f32;
                        let h = if vertical {
                            x_label_h
                        } else {
                            text.measure_weight(l, th.tick_size.0, ticks.weight(i))
                                .width
                        } / 2.0;
                        (c - h, c + h)
                    })
                    .collect();
                boxes.sort_by(|a, b| a.0.total_cmp(&b.0));
                boxes.windows(2).any(|w| w[1].0 - w[0].1 < gap)
            };
            while counts.0 > 1 && collides(&lay.x_ticks, &**x, false) {
                counts.0 -= 1;
                lay.x_ticks = x.ticks(counts.0);
            }
            while counts.1 > 1 && collides(&lay.y_ticks, &**y, true) {
                counts.1 -= 1;
                lay.y_ticks = y.ticks(counts.1);
            }
        };
        let before = counts;
        thin(&mut lay, &mut counts);
        if counts != before {
            let reserved = lay.y_label_w;
            let again = layout(counts.0, counts.1)?;
            if again.y_label_w != reserved || again.plot != lay.plot {
                lay = again;
                set_ranges(lay.plot, widened);
                thin(&mut lay, &mut counts);
            }
        }
        let TickLayout {
            x_ticks,
            y_ticks,
            plot,
            ..
        } = lay;
        if self.hold.is_some() {
            // Only ever grows: the held margins, or what a label needed.
            self.hold = Some(Hold {
                width: width.to_bits(),
                height: height.to_bits(),
                left: plot.left(),
                right: plot.right(),
            });
        }

        // 6. Uniforms and columns, inside a GPU error scope (GUP-410).
        let mut batches = Vec::with_capacity(self.layers.len());
        let mut visible = Vec::with_capacity(self.layers.len());
        for layer in &mut self.layers {
            let batch = cx.scoped(
                || "layer resolution (column upload, uniforms)".to_owned(),
                || layer.prepare(cx, &th),
            )?;
            // Only after the upload succeeded: CPU data the layer's
            // `Retain` policy does not keep.
            layer.release();
            // Chunks that cannot reach the plot rect are neither drawn nor
            // picked (RFC-001 S9f).
            let mask = if self.cull {
                layer.visible_entries(plot)
            } else {
                Vec::new()
            };
            let mut batch = batch;
            if !mask.is_empty() {
                batch.visible = cull::visible_spans(&batch.gpu.spans, &mask).into();
            }
            visible.push(mask);
            batches.push((layer.z(), batch));
        }
        self.visible = visible;
        self.drawn = batches
            .iter()
            .map(|(z, b)| (*z, Arc::clone(&b.gpu), Arc::clone(&b.visible)))
            .collect();

        // 7. The scene.
        let mut scene = Scene::new(width, height, th.background);
        let mut rules = Vec::new();
        let mut labels = Vec::new();
        let axis = |p0: Point, p1: Point| Rule {
            p0,
            p1,
            width: Px(1.0),
            color: th.axis,
        };
        // Axis lines just outside the plot rect, on pixel centres.
        let (ax_y, ay_x) = (plot.bottom() + 0.5, plot.left() - 0.5);
        // A domain the user fixed is not widened, so its axis line covers
        // only the range (no bare stretch past the end ticks).
        let (x_from, x_to) = if x.is_auto() || widened.0 {
            (ay_x, plot.right() - 0.5)
        } else {
            (ay_x, plot.right() - inset)
        };
        let (y_from, y_to) = if y.is_auto() || widened.1 {
            (plot.top() + 0.5, ax_y)
        } else {
            (plot.top() + inset, ax_y)
        };
        rules.push(axis(Point::new(x_from, ax_y), Point::new(x_to, ax_y)));
        rules.push(axis(Point::new(ay_x, y_from), Point::new(ay_x, y_to)));
        let mut x_placed = Vec::new();
        for (i, (v, label)) in x_ticks.values.iter().zip(&x_ticks.labels).enumerate() {
            let px = x.eval(*v) as f32;
            let cx_ = px.floor() + 0.5;
            rules.push(axis(
                Point::new(cx_, ax_y),
                Point::new(cx_, ax_y + th.tick_len.0),
            ));
            labels.push(TextRun {
                text: label.as_str().into(),
                at: Point::new(px, ax_y + th.tick_len.0 + th.label_gap.0),
                anchor: Anchor::new(HAlign::Middle, VAlign::Top),
                style: TextStyle {
                    weight: x_ticks.weight(i),
                    ..tick_style
                },
                role: TextRole::TickLabel,
            });
            x_placed.push((*v, px));
        }
        let mut y_placed = Vec::new();
        for (i, (v, label)) in y_ticks.values.iter().zip(&y_ticks.labels).enumerate() {
            let py = y.eval(*v) as f32;
            let cy = py.floor() + 0.5;
            rules.push(axis(
                Point::new(ay_x - th.tick_len.0, cy),
                Point::new(ay_x, cy),
            ));
            labels.push(TextRun {
                text: label.as_str().into(),
                at: Point::new(ay_x - th.tick_len.0 - th.label_gap.0, py),
                anchor: Anchor::new(HAlign::End, VAlign::Middle),
                style: TextStyle {
                    weight: y_ticks.weight(i),
                    ..tick_style
                },
                role: TextRole::TickLabel,
            });
            y_placed.push((*v, py));
        }
        let (title, axis_titles) = {
            let text = cx.text();
            let title = self.title.as_ref().map(|t| TextRun {
                text: fit_text(&text, t, th.title_size.0, plot.width).into(),
                at: Point::new(plot.x + plot.width / 2.0, th.pad.0),
                anchor: Anchor::new(HAlign::Middle, VAlign::Top),
                style: TextStyle {
                    size: th.title_size,
                    color: th.title,
                    weight: Weight::Regular,
                },
                role: TextRole::Title,
            });
            let axis_style = TextStyle {
                size: th.axis_title_size,
                color: th.label,
                weight: Weight::Regular,
            };
            let mut axis_titles = Vec::new();
            if let Some(t) = &self.x_title {
                axis_titles.push(TextRun {
                    text: fit_text(&text, t, th.axis_title_size.0, plot.width).into(),
                    at: Point::new(
                        plot.x + plot.width / 2.0,
                        ax_y + th.tick_len.0 + th.label_gap.0 + x_label_h + th.label_gap.0,
                    ),
                    anchor: Anchor::new(HAlign::Middle, VAlign::Top),
                    style: axis_style,
                    role: TextRole::AxisTitle,
                });
            }
            if let Some(t) = &self.y_title {
                axis_titles.push(TextRun {
                    text: fit_text(&text, t, th.axis_title_size.0, width - 2.0 * th.pad.0).into(),
                    at: Point::new(th.pad.0, plot.top() - th.label_gap.0),
                    anchor: Anchor::new(HAlign::Start, VAlign::Bottom),
                    style: axis_style,
                    role: TextRole::AxisTitle,
                });
            }
            (title, axis_titles)
        };

        let legend = (!legends.is_empty()).then(|| {
            crate::guide::place(
                &cx.text(),
                &th,
                &legends,
                plot.right() + crate::guide::COLUMN_GAP,
                plot,
            )
        });
        let legend_texts: &[TextRun] = legend.as_ref().map_or(&[], |l| &l.texts);
        let texts = {
            let text = cx.text();
            labels
                .iter()
                .chain(&title)
                .chain(&axis_titles)
                .chain(legend_texts)
                .map(|run| PlacedText {
                    run: run.clone(),
                    bounds: text.ink_bounds(&run.layout_run()).into(),
                })
                .collect()
        };
        let mut guides: Vec<Rect> = rules.iter().map(Rule::bounds).collect();
        guides.extend(legend.iter().flat_map(|l| l.guides.iter().copied()));

        let clip = scene.add_clip(plot);
        // Insertion order; `Scene::push` breaks z ties by it.
        for (z, batch) in batches {
            scene.push(Item {
                z,
                clip: Some(clip),
                kind: ItemKind::Marks(batch),
            });
        }
        scene.push(Item {
            z: Z_AXES,
            clip: None,
            kind: ItemKind::Rules(rules),
        });
        scene.push(Item {
            z: Z_AXES,
            clip: None,
            kind: ItemKind::Text(labels),
        });
        let titles: Vec<TextRun> = title.into_iter().chain(axis_titles).collect();
        if !titles.is_empty() {
            scene.push(Item {
                z: Z_TITLE,
                clip: None,
                kind: ItemKind::Text(titles),
            });
        }

        let legend_bounds = legend.as_ref().map(|l| l.bounds);
        if let Some(l) = legend {
            for item in l.items {
                scene.push(item);
            }
        }

        Ok(Resolved {
            layout: Layout {
                width,
                height,
                plot,
                texts,
                guides,
                mark_overhang,
                x_ticks: x_placed,
                y_ticks: y_placed,
                legend: legend_bounds,
                tooltip: None,
            },
            scene,
        })
    }

    /// The generated glue WGSL of each layer (diagnostics).
    pub fn glue_sources(&self) -> Vec<(String, String)> {
        self.layers
            .iter()
            .map(|l| {
                let g = l.glue_source();
                (g.signature, g.source)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chart::ChartExt;
    use crate::encoding::{CpuMirror, EncodeFn};
    use crate::marks::Circle;
    use crate::scale::{Linear, Log, ScaleRef};
    use crate::scene::ItemKind;
    use crate::selection::Retain;
    use crate::target::ImageTarget;
    use std::sync::Arc;

    #[test]
    fn fit_text_keeps_what_fits_and_truncates_the_rest() {
        let cx = Context::new_blocking().unwrap();
        let text = cx.text();
        assert_eq!(fit_text(&text, "short", 12.0, 500.0), "short");
        let long = "x".repeat(100);
        for max in [30.0, 80.0, 200.0] {
            let fitted = fit_text(&text, &long, 12.0, max);
            assert!(fitted.ends_with('…') && fitted.len() < long.len());
            assert!(text.measure(&fitted, 12.0).width <= max);
        }
        assert_eq!(fit_text(&text, &long, 12.0, 1.0), "…");
    }

    fn batch(r: &Resolved) -> Arc<crate::render::LayerGpu> {
        r.scene
            .items
            .iter()
            .find_map(|i| match &i.kind {
                ItemKind::Marks(b) => Some(Arc::clone(&b.gpu)),
                _ => None,
            })
            .unwrap()
    }

    /// Changing a domain re-resolves with new uniforms only: the glue is
    /// not relinked, the pipeline is a cache hit, the layer's GPU state
    /// (column chunk, uniform buffers, bind groups) is reused and the
    /// upload counter shows 0 column bytes and only uniform writes.
    #[test]
    fn rescale_reuses_program_pipeline_and_columns() {
        let cx = Context::new_blocking().unwrap();
        let mut plot = Plot::new();
        let (x, y) = (plot.x(Linear::new()), plot.y(Log::new()));
        plot.add(Selection::<(f64, f64), Circle>::new(vec![
            (1.0, 1.0),
            (5.0, 100.0),
        ]))
        .attr(Circle::X, x.encode(|r: &(f64, f64)| r.0))
        .attr(Circle::Y, y.encode(|r: &(f64, f64)| r.1));
        let first = plot.resolve(&cx, 300.0, 200.0).unwrap().clone();
        let mut target = ImageTarget::new(&cx, 300, 200).unwrap();
        let image = target.render_blocking(&cx, &first.scene).unwrap();
        let before = cx.pipelines().stats;
        let uploads = cx.upload_stats();

        // Tick labels reuse the first render's glyphs, so no atlas texels are written.
        x.write().set_domain(1.0, 5.0).unwrap();
        let second = plot.resolve(&cx, 300.0, 200.0).unwrap();
        let zoomed = target.render_blocking(&cx, &second.scene).unwrap();
        let after = cx.pipelines().stats;
        let written = cx.upload_stats() - uploads;

        assert_eq!(before.programs_linked, 1);
        assert_eq!(after.programs_linked, 1, "glue relinked");
        assert_eq!(after.pipelines_created, before.pipelines_created);
        assert!(after.pipeline_hits > before.pipeline_hits);
        assert!(
            Arc::ptr_eq(&batch(&first), &batch(second)),
            "layer GPU state rebuilt"
        );
        assert_ne!(first.layout.x_ticks, second.layout.x_ticks);
        assert_ne!(image, zoomed, "the new domain did not reach the GPU");

        assert_eq!(written.columns, Default::default(), "{written:?}");
        assert_eq!(written.textures, Default::default(), "{written:?}");
        // Encodings + chunk uniforms, then the view uniform.
        assert_eq!(written.uniforms.writes, 3, "{written:?}");
    }

    /// Zooming the plot about a point keeps that point's data values fixed
    /// on both axes and narrows both domains.
    #[test]
    fn zoom_narrows_both_domains_about_the_anchor() {
        let cx = Context::new_blocking().unwrap();
        let mut plot = Plot::new();
        let (x, y) = (plot.x(Linear::new()), plot.y(Log::new()));
        plot.add(Selection::<(f64, f64), Circle>::new(vec![
            (0.0, 1.0),
            (100.0, 1e4),
        ]))
        .attr(Circle::X, x.encode(|r: &(f64, f64)| r.0))
        .attr(Circle::Y, y.encode(|r: &(f64, f64)| r.1));
        let r = plot.resolve(&cx, 400.0, 300.0).unwrap();
        let at = Point::new(
            r.layout.plot.x + 0.3 * r.layout.plot.width,
            r.layout.plot.y + 0.6 * r.layout.plot.height,
        );
        let (xv, yv) = (
            x.read().invert(f64::from(at.x)),
            y.read().invert(f64::from(at.y)),
        );
        let (xd, yd) = (
            x.read().current_domain().unwrap(),
            y.read().current_domain().unwrap(),
        );
        plot.zoom(at, 0.5).unwrap();
        let (zx, zy) = (
            x.read().current_domain().unwrap(),
            y.read().current_domain().unwrap(),
        );
        assert!(((zx.1 - zx.0) / (xd.1 - xd.0) - 0.5).abs() < 1e-9, "{zx:?}");
        assert!(((zy.1 / zy.0).log10() / (yd.1 / yd.0).log10() - 0.5).abs() < 1e-9);
        assert!((x.read().invert(f64::from(at.x)) - xv).abs() < 1e-9);
        assert!((y.read().invert(f64::from(at.y)) / yv - 1.0).abs() < 1e-9);
        // Domains are now explicit, so re-resolving keeps them.
        plot.resolve(&cx, 400.0, 300.0).unwrap();
        assert_eq!(x.read().current_domain().unwrap(), zx);
        assert!(plot.zoom(at, 0.0).is_err());
    }

    type Pt = (f64, f64);

    /// A scatter of `rows` (linear x, log y) in chunks of at most 64 rows.
    fn chunked(rows: Vec<Pt>) -> (Plot, ScaleRef<Linear>) {
        let mut plot = Plot::new();
        let (x, y) = (plot.x(Linear::new()), plot.y(Log::new()));
        plot.add(Selection::<Pt, Circle>::new(rows))
            .attr(Circle::X, x.encode(|r: &Pt| r.0))
            .attr(Circle::Y, y.encode(|r: &Pt| r.1))
            .max_chunk_rows(64);
        (plot, x)
    }

    fn points(range: std::ops::Range<u32>) -> Vec<Pt> {
        range
            .map(|i| {
                let i = f64::from(i);
                (1.7e9 + i * 3.5, 1.0 + (i * 0.37) % 9.0 * 100.0)
            })
            .collect()
    }

    /// RFC-001 S4a (AC5): appending rows in batches writes exactly their
    /// column bytes (no chunk is uploaded again), grows the auto x domain
    /// through the merged stats, and renders exactly what a plot built
    /// from every row at once renders.
    #[test]
    fn appended_rows_upload_only_their_bytes() {
        let cx = Context::new_blocking().unwrap();
        let (mut plot, x) = chunked(points(0..100));
        let mut target = ImageTarget::new(&cx, 400, 300).unwrap();
        let first = plot.resolve(&cx, 400.0, 300.0).unwrap();
        let before = target.render_blocking(&cx, &first.scene).unwrap();
        let domain = x.read().current_domain().unwrap();
        let start = cx.upload_stats();

        let mut appended = 0;
        let mut end = 100;
        for n in [1, 27, 36, 100, 300, 5] {
            let layer: &mut Selection<Pt, Circle> = plot.layer_at_mut(0);
            layer.append(points(end..end + n)).unwrap();
            end += n;
            appended += u64::from(n);
            let resolved = plot.resolve(&cx, 400.0, 300.0).unwrap();
            assert_eq!(batch(resolved).instances(), u64::from(end));
            target.render_blocking(&cx, &resolved.scene).unwrap();
        }
        let written = cx.upload_stats() - start;
        // Two f32 columns: exactly the appended rows' bytes.
        assert_eq!(written.columns.bytes, appended * 8, "{written:?}");
        let grown = x.read().current_domain().unwrap();
        assert!(grown.0 <= domain.0, "{domain:?} → {grown:?}");
        assert!(grown.1 > domain.1 + 1000.0, "{domain:?} → {grown:?}");

        let resolved = plot.resolve(&cx, 400.0, 300.0).unwrap();
        assert_eq!(batch(resolved).chunks.len(), (end as usize).div_ceil(64));
        let after = target.render_blocking(&cx, &resolved.scene).unwrap();
        assert_ne!(before, after);
        let (mut fresh, _) = chunked(points(0..end));
        let expected = fresh.render(&cx, 400, 300).unwrap();
        assert!(after == expected, "appending differs from building at once");
    }

    /// GUP-432: appending to a line extends it, through the last chunk's
    /// tail and across new chunks (new bridges), writing only the new rows'
    /// bytes, and renders what a line built from every row at once does.
    #[test]
    fn appended_rows_extend_a_line() {
        use crate::Line;
        let line = |rows: Vec<Pt>| {
            let mut plot = Plot::new();
            let (x, y) = (plot.x(Linear::new()), plot.y(Linear::new()));
            plot.add(Selection::<Pt, Line>::new(rows))
                .attr(Line::X, x.encode(|r: &Pt| r.0))
                .attr(Line::Y, y.encode(|r: &Pt| r.1))
                .attr(Line::WIDTH, Px(2.0))
                .max_chunk_rows(8);
            plot
        };
        let cx = Context::new_blocking().unwrap();
        let mut plot = line(points(0..10));
        let mut target = ImageTarget::new(&cx, 400, 300).unwrap();
        plot.resolve(&cx, 400.0, 300.0).unwrap();
        let start = cx.upload_stats();
        let mut end = 10;
        for n in [3, 8, 1] {
            let layer: &mut Selection<Pt, Line> = plot.layer_at_mut(0);
            layer.append(points(end..end + n)).unwrap();
            end += n;
            let resolved = plot.resolve(&cx, 400.0, 300.0).unwrap();
            assert_eq!(batch(resolved).instances(), u64::from(end - 1));
        }
        let written = cx.upload_stats() - start;
        assert_eq!(
            written.columns.bytes,
            u64::from(end - 10) * 8,
            "{written:?}"
        );
        let resolved = plot.resolve(&cx, 400.0, 300.0).unwrap();
        // Chunks of 8, 8 and 6 rows, and two bridges.
        assert_eq!(batch(resolved).chunks.len(), 5);
        let after = target.render_blocking(&cx, &resolved.scene).unwrap();
        let expected = line(points(0..end)).render(&cx, 400, 300).unwrap();
        assert!(after == expected, "appending differs from building at once");
    }

    /// GUP-434: `layer_mut` finds a layer by its row and mark types.
    #[test]
    fn layer_mut_matches_layers_by_type() {
        type A = (f64, f64);
        type B = [f64; 2];
        let mut plot = Plot::new();
        let (x, y) = (plot.x(Linear::new()), plot.y(Linear::new()));
        plot.add(Selection::<A, Circle>::new(vec![(1.0, 2.0)]))
            .attr(Circle::X, x.encode(|r: &A| r.0))
            .attr(Circle::Y, y.encode(|r: &A| r.1));
        plot.add(Selection::<B, Circle>::new(vec![[3.0, 4.0]]))
            .attr(Circle::X, x.encode(|r: &B| r[0]))
            .attr(Circle::Y, y.encode(|r: &B| r[1]));
        assert!(plot.layer_mut::<A, Circle>().is_some());
        assert!(plot.layer_mut::<B, Circle>().is_some());
        assert!(plot.layer_mut::<A, crate::marks::Rect>().is_none());
        assert!(plot.layer_mut::<u8, Circle>().is_none());
        plot.layer_mut::<B, Circle>()
            .unwrap()
            .attr(Circle::X, x.encode(|r: &B| r[1]));
        let cx = Context::new_blocking().unwrap();
        plot.resolve(&cx, 300.0, 200.0).unwrap();
    }

    /// RFC-001 S7d (GUP-433): `DictionarySlot` hands every user the same
    /// dictionary, and reports a key type mismatch.
    #[test]
    fn dictionary_slot_shares_one_dictionary_per_key_type() {
        use crate::encoding::DictionarySlot;
        let slot = DictionarySlot::default();
        let first = slot.get_or_insert::<Arc<str>>().unwrap();
        first.lock().unwrap().encode(Arc::from("a")).unwrap();
        let second = slot.get_or_insert::<Arc<str>>().unwrap();
        assert!(Arc::ptr_eq(&first, &second));
        assert_eq!(second.lock().unwrap().labels().len(), 1);
        let (existing, requested) = slot.get_or_insert::<u32>().unwrap_err();
        assert!(existing.contains("str") && requested == "u32");
    }

    /// RFC-001 S7d (GUP-433): a new key appended to one layer of a shared
    /// band axis uploads only that layer's new rows.
    #[test]
    fn appending_a_new_key_uploads_only_the_appended_layer() {
        use crate::scale::Band;
        type R = (&'static str, f64);
        let cx = Context::new_blocking().unwrap();
        let mut plot = Plot::new();
        let (x, y) = (plot.x(Band::new()), plot.y(Linear::new()));
        for rows in [vec![("a", 1.0), ("b", 2.0)], vec![("b", 3.0), ("a", 4.0)]] {
            plot.add(Selection::<R, Circle>::new(rows))
                .attr(Circle::X, x.encode_key(|r: &R| r.0))
                .attr(Circle::Y, y.encode(|r: &R| r.1));
        }
        plot.resolve(&cx, 400.0, 300.0).unwrap();
        let start = cx.upload_stats();
        let layer: &mut Selection<R, Circle> = plot.layer_at_mut(1);
        layer
            .append(vec![("c", 5.0), ("a", 6.0), ("c", 7.0)])
            .unwrap();
        plot.resolve(&cx, 400.0, 300.0).unwrap();
        let written = cx.upload_stats() - start;
        // A u32 code column and an f32 column: only the three new rows.
        assert_eq!(written.columns.bytes, 3 * 8, "{written:?}");
        assert_eq!(x.read().keys().len(), 3);
    }

    #[test]
    fn resolve_errors_name_what_is_missing() {
        let cx = Context::new_blocking().unwrap();
        // A new plot has default axes: only the missing layer is an error.
        let err = Plot::new().resolve(&cx, 300.0, 200.0).unwrap_err();
        assert!(err.to_string().contains("plot.add(..)"), "{err}");
        let mut plot = Plot::new();
        plot.x(Linear::new());
        plot.y(Log::new());
        let err = plot.resolve(&cx, 300.0, 200.0).unwrap_err();
        assert!(err.to_string().contains("plot.add(..)"), "{err}");
    }

    /// A row that counts its live copies through a shared token.
    struct Tracked {
        x: f64,
        y: f64,
        _token: Arc<()>,
    }

    fn tracked(range: std::ops::Range<u32>, token: &Arc<()>) -> Vec<Tracked> {
        points(range)
            .into_iter()
            .map(|(x, y)| Tracked {
                x,
                y,
                _token: Arc::clone(token),
            })
            .collect()
    }

    /// A scatter of tracked rows in chunks of at most 64 rows.
    fn tracked_plot(rows: Vec<Tracked>, retain: Retain) -> (Plot, ScaleRef<Linear>) {
        let mut plot = Plot::new();
        let (x, y) = (plot.x(Linear::new()), plot.y(Log::new()));
        plot.add(Selection::<Tracked, Circle>::new(rows))
            .retain(retain)
            .attr(Circle::X, x.encode(|r: &Tracked| r.x))
            .attr(Circle::Y, y.encode(|r: &Tracked| r.y))
            .max_chunk_rows(64);
        (plot, x)
    }

    fn tracked_layer(plot: &mut Plot) -> &mut Selection<Tracked, Circle> {
        plot.layer_at_mut(0)
    }

    /// Which chunks of the layer still have their CPU copy.
    fn kept(plot: &mut Plot) -> Vec<bool> {
        let store = tracked_layer(plot).store().unwrap();
        store.chunks().iter().map(|c| c.bytes().is_some()).collect()
    }

    #[test]
    fn retain_policies_keep_what_they_say() {
        use Retain::*;
        let max = Retain::AUTO_MAX_ROWS;
        assert_eq!(Retain::default(), Auto);
        for (policy, rows, keeps) in [
            (Auto, max, (true, true)),
            (Auto, max + 1, (false, false)),
            (Rows, max + 1, (true, false)),
            (Columns, 1, (false, true)),
            (GpuOnly, 1, (false, false)),
        ] {
            assert_eq!(
                (policy.keeps_rows(rows), policy.keeps_columns(rows)),
                keeps,
                "{policy:?} at {rows} rows"
            );
        }
    }

    /// S4b (AC5): under `GpuOnly`, a successful resolve drops every row
    /// (each `T` is dropped, not just unused) and the CPU copy of every
    /// full chunk, keeping stats and GPU buffers: zooming still works and
    /// renders what a plot that kept everything renders. Appended rows are
    /// dropped after their upload too. Resolving on another context, or
    /// re-encoding a channel, is an error naming the policy.
    #[test]
    fn gpu_only_drops_rows_and_full_chunks_after_upload() {
        let cx = Context::new_blocking().unwrap();
        let token = Arc::new(());
        let (mut plot, x) = tracked_plot(tracked(0..150, &token), Retain::GpuOnly);
        assert_eq!(Arc::strong_count(&token), 151);
        let resolved = plot.resolve(&cx, 400.0, 300.0).unwrap().clone();
        assert_eq!(Arc::strong_count(&token), 1, "every row was dropped");
        assert!(tracked_layer(&mut plot).rows_released());
        assert_eq!(tracked_layer(&mut plot).len(), 150);
        assert_eq!(kept(&mut plot), [false, false, true]);
        let stats = tracked_layer(&mut plot).store().unwrap().stats(0).unwrap();
        assert_eq!(stats.extent(), (points(0..1)[0].0, points(149..150)[0].0));

        // The same picture as a plot that keeps everything.
        let mut target = ImageTarget::new(&cx, 400, 300).unwrap();
        let released = target.render_blocking(&cx, &resolved.scene).unwrap();
        let (mut keep, _) = tracked_plot(tracked(0..150, &token), Retain::Auto);
        let kept_image = keep.render(&cx, 400, 300).unwrap();
        assert!(released == kept_image, "releasing changed the render");
        drop(keep);

        // Zooming writes uniforms only.
        let start = cx.upload_stats();
        plot.zoom(Point::new(200.0, 150.0), 0.5).unwrap();
        let zoomed = plot.resolve(&cx, 400.0, 300.0).unwrap();
        assert_eq!(batch(zoomed).instances(), 150);
        assert_eq!((cx.upload_stats() - start).columns, Default::default());

        // Appending: the new rows are evaluated, uploaded, then dropped.
        tracked_layer(&mut plot)
            .append(tracked(150..170, &token))
            .unwrap();
        assert_eq!(Arc::strong_count(&token), 21);
        let grown = plot.resolve(&cx, 400.0, 300.0).unwrap();
        assert_eq!(batch(grown).instances(), 170);
        assert_eq!(Arc::strong_count(&token), 1);
        assert_eq!(kept(&mut plot), [false, false, true]);

        let other = Context::from_wgpu(cx.device().clone(), cx.queue().clone());
        let err = plot.resolve(&other, 400.0, 300.0).unwrap_err().to_string();
        assert!(
            err.contains("on another context and their CPU copy was dropped")
                && err.contains("Retain::GpuOnly"),
            "{err}"
        );

        tracked_layer(&mut plot).attr(Circle::X, x.encode(|r: &Tracked| r.x * 2.0));
        let err = plot.resolve(&cx, 400.0, 300.0).unwrap_err().to_string();
        assert!(
            err.contains("Circle layer: its columns must be evaluated again")
                && err.contains("170 rows were dropped after upload (Retain::GpuOnly)"),
            "{err}"
        );
    }

    /// S4b: `Rows` keeps the rows and re-evaluates them for another
    /// context; `Columns` drops the rows but uploads its CPU columns to
    /// another context; `Auto` keeps both at this size.
    #[test]
    fn rows_and_columns_policies_survive_another_context() {
        let cx = Context::new_blocking().unwrap();
        let other = Context::from_wgpu(cx.device().clone(), cx.queue().clone());
        let token = Arc::new(());
        for (policy, rows_kept, chunks_kept) in [
            (Retain::Rows, true, [false, false, true]),
            (Retain::Columns, false, [true, true, true]),
            (Retain::Auto, true, [true, true, true]),
        ] {
            let (mut plot, _) = tracked_plot(tracked(0..150, &token), policy);
            let here = plot.render(&cx, 400, 300).unwrap();
            assert_eq!(Arc::strong_count(&token) > 1, rows_kept, "{policy:?}");
            assert_eq!(kept(&mut plot), chunks_kept, "{policy:?}");
            let there = plot.render(&other, 400, 300).unwrap();
            assert!(
                here == there,
                "{policy:?}: another context renders differently"
            );
        }
    }

    /// S4b (AC1): the vertex stage hides a row by its validity bit, never
    /// by testing the value. Row 0 keeps a finite position, but with its x
    /// bit cleared (the store already has a null, row 2, so it uploads
    /// bits) its disc vanishes while row 1 still draws.
    #[test]
    fn a_cleared_validity_bit_hides_a_finite_row() {
        let cx = Context::new_blocking().unwrap();
        let mut plot = Plot::new();
        let (x, y) = (
            plot.x(Linear::new().domain(0.0, 10.0)),
            plot.y(Linear::new().domain(0.0, 10.0)),
        );
        plot.add(Selection::<Pt, Circle>::new(vec![
            (2.0, 5.0),
            (8.0, 5.0),
            (f64::NAN, 5.0),
        ]))
        .attr(Circle::X, x.encode(|r: &Pt| r.0))
        .attr(Circle::Y, y.encode(|r: &Pt| r.1))
        .attr(Circle::RADIUS, Px(10.0));
        let white = image::Rgba([255, 255, 255, 255]);
        let (before, _) = plot.render_resolved(&cx, 300, 200).unwrap();
        let centre = |v: f64| (x.read().eval(v) as u32, y.read().eval(5.0) as u32);
        let ((x0, y0), (x1, y1)) = (centre(2.0), centre(8.0));
        assert_ne!(*before.get_pixel(x0, y0), white);
        assert_ne!(*before.get_pixel(x1, y1), white);

        let layer: &mut Selection<Pt, Circle> = plot.layer_at_mut(0);
        layer.store_mut().unwrap().clear_valid(0, 0);
        let (after, _) = plot.render_resolved(&cx, 300, 200).unwrap();
        assert_eq!(*after.get_pixel(x0, y0), white, "row 0 still drawn");
        assert_eq!(*after.get_pixel(x1, y1), *before.get_pixel(x1, y1));
    }

    /// GUP-432 (AC4): a line hides both segments touching a row by its
    /// validity bit, including across chunk boundaries (the bridge reads
    /// the next chunk's bits). Rows 3 (the last of chunk 0), 5 (mid-chunk
    /// in chunk 1) and 8 (the first of chunk 2) keep finite positions but
    /// have their y bits cleared; the NaN in row 11 makes the store upload
    /// bits.
    #[test]
    fn a_cleared_validity_bit_breaks_a_line() {
        use crate::Line;
        let cx = Context::new_blocking().unwrap();
        let mut plot = Plot::new();
        let (x, y) = (
            plot.x(Linear::new().domain(-1.0, 12.0)),
            plot.y(Linear::new().domain(0.0, 10.0)),
        );
        let mut rows: Vec<Pt> = (0..12)
            .map(|i| (f64::from(i), if i % 2 == 0 { 2.0 } else { 8.0 }))
            .collect();
        rows[11].1 = f64::NAN;
        plot.add(Selection::<Pt, Line>::new(rows.clone()))
            .attr(Line::X, x.encode(|r: &Pt| r.0))
            .attr(Line::Y, y.encode(|r: &Pt| r.1))
            .attr(Line::WIDTH, Px(3.0))
            .max_chunk_rows(4);
        let white = image::Rgba([255, 255, 255, 255]);
        let mid = |i: usize| {
            let (a, b) = (rows[i], rows[i + 1]);
            (
                x.read().eval((a.0 + b.0) / 2.0) as u32,
                y.read().eval((a.1 + b.1) / 2.0) as u32,
            )
        };
        let (before, _) = plot.render_resolved(&cx, 400, 300).unwrap();
        for i in 0..10 {
            let (px, py) = mid(i);
            assert_ne!(*before.get_pixel(px, py), white, "segment {i} not drawn");
        }

        let layer: &mut Selection<Pt, Line> = plot.layer_at_mut(0);
        let store = layer.store_mut().unwrap();
        for row in [3, 5, 8] {
            store.clear_valid(row, 1);
        }
        let (after, _) = plot.render_resolved(&cx, 400, 300).unwrap();
        for i in 0..10 {
            let (px, py) = mid(i);
            let hidden = [2, 3, 4, 5, 7, 8].contains(&i);
            if hidden {
                assert_eq!(*after.get_pixel(px, py), white, "segment {i} still drawn");
            } else {
                assert_eq!(
                    *after.get_pixel(px, py),
                    *before.get_pixel(px, py),
                    "segment {i}"
                );
            }
        }
    }

    /// GUP-419 AC1: the dictionary hook end to end. Appending rows with a
    /// new key grows the categorical domain and its legend; the GPU learns
    /// of it through one uniform (the key count): the column writes are
    /// exactly the appended rows' bytes, no texture (palette) is written,
    /// and the layer's GPU state is kept. The new key draws in its palette
    /// colour, not the null colour a code outside the domain gets, so the
    /// uniform did reach the GPU.
    #[test]
    fn a_new_key_grows_the_domain_with_a_uniform_write_only() {
        use crate::ShaderFn;
        use crate::scale::{Categorical, OKABE_ITO};
        type Row = (f64, f64, &'static str);
        let cx = Context::new_blocking().unwrap();
        let rows: Vec<Row> = (0..30)
            .map(|i| {
                (
                    f64::from(i),
                    f64::from(i % 5),
                    ["a", "b", "c"][i as usize % 3],
                )
            })
            .collect();
        let mut plot = Plot::new();
        let (x, y) = (
            plot.x(Linear::new().domain(-1.0, 40.0)),
            plot.y(Linear::new().domain(-1.0, 5.0)),
        );
        let colour = ScaleRef::new(Categorical::okabe_ito());
        // No legend: a new key's label would write glyphs to the atlas,
        // and this test counts texture writes.
        plot.legends(false);
        plot.add(Selection::<Row, Circle>::new(rows))
            .attr(Circle::X, x.encode(|r: &Row| r.0))
            .attr(Circle::Y, y.encode(|r: &Row| r.1))
            .attr(Circle::RADIUS, Px(4.0))
            .attr(Circle::FILL, colour.encode_key(|r: &Row| r.2));
        let mut target = ImageTarget::new(&cx, 400, 300).unwrap();
        let first = plot.resolve(&cx, 400.0, 300.0).unwrap().clone();
        target.render_blocking(&cx, &first.scene).unwrap();
        let labels = |c: &ScaleRef<Categorical>| -> Vec<String> {
            let legend = c.read().legend().unwrap();
            legend
                .swatches()
                .iter()
                .map(|s| s.label.to_string())
                .collect()
        };
        assert_eq!(labels(&colour), ["a", "b", "c"]);

        let start = cx.upload_stats();
        let linked = cx.pipelines().stats.programs_linked;
        let layer: &mut Selection<Row, Circle> = plot.layer_at_mut(0);
        layer.append([(35.0, 2.0, "d"), (36.0, 3.0, "a")]).unwrap();
        let second = plot.resolve(&cx, 400.0, 300.0).unwrap();
        let image = target.render_blocking(&cx, &second.scene).unwrap();
        let written = cx.upload_stats() - start;

        assert_eq!(labels(&colour), ["a", "b", "c", "d"]);
        assert_eq!(
            colour.read().legend().unwrap().swatches()[3].color,
            OKABE_ITO[3]
        );
        // x, y (4 bytes each) and the code (4 bytes) of 2 rows.
        assert_eq!(written.columns.bytes, 2 * 12, "{written:?}");
        assert_eq!(written.textures, Default::default(), "{written:?}");
        // Same program and pipeline: the domain is a uniform, not WGSL.
        assert_eq!(cx.pipelines().stats.programs_linked, linked);
        assert_eq!(
            batch(&first).program.glue.signature,
            batch(second).program.glue.signature
        );
        let at = |v: (f64, f64)| {
            image
                .get_pixel(x.read().eval(v.0) as u32, y.read().eval(v.1) as u32)
                .0
        };
        assert_eq!(at((35.0, 2.0)), OKABE_ITO[3].to_rgba8(), "the new key");
        assert_eq!(at((36.0, 3.0)), OKABE_ITO[0].to_rgba8(), "an old key");
    }

    /// GUP-429 AC3: a bar's width comes from the band scale each resolve.
    /// Appending a row with a new key narrows every bar through a uniform
    /// write: the second resolve's column writes are the appended rows'
    /// bytes only, and `width` has no column.
    #[test]
    fn a_bar_width_follows_an_appended_key() {
        type Row = (&'static str, f64);
        let cx = Context::new_blocking().unwrap();
        let rows: Vec<Row> = vec![("A", 5.0), ("B", 8.0), ("C", 6.0)];
        let mut plot = Plot::new();
        let (x, y) = (
            plot.x(crate::Band::new()),
            plot.y(Linear::new().domain(0.0, 10.0)),
        );
        let floor = y.clone();
        plot.add(Selection::<Row, crate::Rect>::new(rows))
            .attr(crate::Rect::X, x.encode_key(|r: &Row| r.0))
            .attr(crate::Rect::WIDTH, x.band_width_px())
            .attr(crate::Rect::Y, y.encode(|r: &Row| r.1))
            .attr(
                crate::Rect::Y_END,
                crate::Derived::new(move || Px(floor.read().eval(0.0) as f32)),
            );
        let mut target = ImageTarget::new(&cx, 400, 300).unwrap();
        // Ink widths along a row just above the baseline, one run per bar.
        let widths = |image: &image::RgbaImage| -> Vec<u32> {
            let row = (y.read().eval(0.0) - 3.0) as u32;
            let mut runs = Vec::new();
            let mut start = None;
            for px in 0..image.width() {
                let ink = image.get_pixel(px, row).0[0] < 128;
                match (ink, start) {
                    (true, None) => start = Some(px),
                    (false, Some(s)) => {
                        // Runs of a few pixels are the axis, not a bar.
                        if px - s > 5 {
                            runs.push(px - s);
                        }
                        start = None;
                    }
                    _ => {}
                }
            }
            runs
        };
        let first = plot.resolve(&cx, 400.0, 300.0).unwrap();
        let before = target.render_blocking(&cx, &first.scene).unwrap();
        let w3 = x.read().band_width();
        let runs = widths(&before);
        assert_eq!(runs.len(), 3, "{runs:?}");
        for r in &runs {
            assert!((f64::from(*r) - w3.round()).abs() <= 1.0, "{r} vs {w3}");
        }

        let start = cx.upload_stats();
        let layer: &mut Selection<Row, crate::Rect> = plot.layer_at_mut(0);
        layer.append([("D", 7.0), ("A", 9.0)]).unwrap();
        let second = plot.resolve(&cx, 400.0, 300.0).unwrap();
        let after = target.render_blocking(&cx, &second.scene).unwrap();
        let written = cx.upload_stats() - start;
        // y (4 bytes) and the key code (4 bytes) of 2 rows.
        assert_eq!(written.columns.bytes, 2 * 8, "{written:?}");
        let w4 = x.read().band_width();
        assert!(w4 < w3, "{w4} vs {w3}");
        let runs = widths(&after);
        assert_eq!(runs.len(), 4, "{runs:?}");
        for r in &runs {
            assert!((f64::from(*r) - w4.round()).abs() <= 1.0, "{r} vs {w4}");
        }
    }

    /// GUP-419 AC4: a null key on a band position is hidden by the glue's
    /// `NULL_CODE` comparison, not by landing off-screen. The x channel is
    /// a `Band` with an empty range (step 0), so the GPU maps every code,
    /// the null code too, to x = 150, as the dispatch shows. Rows with a
    /// key draw there; the row without one does not, on x and on y.
    #[test]
    fn a_null_band_key_is_hidden_not_placed() {
        use crate::scale::Band;
        let cx = Context::new_blocking().unwrap();
        let on_screen = Band::new().range(Px(150.0), Px(150.0));
        let gpu = crate::scale::conformance::dispatch(
            &cx,
            &on_screen,
            &[0.0, f64::from(crate::column::NULL_CODE)],
            None,
        );
        assert_eq!(gpu, [150.0, 150.0], "the null code maps on-screen");

        type Row = (Option<&'static str>, f64);
        let rows: Vec<Row> = vec![(Some("a"), 2.0), (None, 5.0), (Some("b"), 8.0)];
        let mut plot = Plot::new();
        let (x, y) = (
            plot.x(Linear::new().domain(0.0, 10.0)),
            plot.y(Linear::new().domain(0.0, 10.0)),
        );
        plot.add(Selection::<Row, Circle>::new(rows))
            .attr(Circle::X, on_screen.encode_nullable_key(|r: &Row| r.0))
            .attr(Circle::Y, y.encode(|r: &Row| r.1))
            .attr(Circle::RADIUS, Px(8.0));
        let resolved = plot.resolve(&cx, 300.0, 200.0).unwrap().clone();
        let glue = &plot.glue_sources()[0].1;
        assert!(
            glue.contains("let drawn = u32(col.x != 4294967295u);"),
            "{glue}"
        );
        let image = ImageTarget::new(&cx, 300, 200)
            .unwrap()
            .render_blocking(&cx, &resolved.scene)
            .unwrap();
        let white = image::Rgba([255, 255, 255, 255]);
        let at = |v: f64| *image.get_pixel(150, y.read().eval(v) as u32);
        assert_ne!(at(2.0), white, "key a");
        assert_ne!(at(8.0), white, "key b");
        assert_eq!(at(5.0), white, "the null key was drawn");
        let _ = x;

        // The same on y, with the x column holding a null too: both
        // compares AND together.
        let rows: Vec<(f64, Option<&'static str>)> = vec![(2.0, Some("p")), (5.0, None)];
        let mut plot = Plot::new();
        let (x, _y) = (
            plot.x(Linear::new().domain(0.0, 10.0)),
            plot.y(Linear::new().domain(0.0, 10.0)),
        );
        plot.add(Selection::<(f64, Option<&'static str>), Circle>::new(rows))
            .attr(Circle::X, x.encode(|r: &(f64, Option<&str>)| r.0))
            .attr(
                Circle::Y,
                Band::new()
                    .range(Px(100.0), Px(100.0))
                    .encode_nullable_key(|r: &(f64, Option<&'static str>)| r.1),
            )
            .attr(Circle::RADIUS, Px(8.0));
        let image = plot.render(&cx, 300, 200).unwrap();
        let at = |v: f64| *image.get_pixel(x.read().eval(v) as u32, 100);
        assert_ne!(at(2.0), white);
        assert_eq!(at(5.0), white, "the null y key was drawn");
    }

    /// S4b: a non-finite value driving a numeric colour scale draws the
    /// point in the null colour (a `select` on its validity bit), not a
    /// palette colour, and the point is still drawn.
    #[test]
    fn a_null_sequential_input_draws_in_the_null_colour() {
        let cx = Context::new_blocking().unwrap();
        let mut plot = Plot::new();
        let (x, y) = (
            plot.x(Linear::new().domain(0.0, 10.0)),
            plot.y(Linear::new().domain(0.0, 10.0)),
        );
        let rows: Vec<(f64, f64)> = vec![(2.0, 1.0), (5.0, f64::NAN), (8.0, 3.0)];
        plot.add(Selection::<Pt, Circle>::new(rows))
            .attr(Circle::X, x.encode(|r: &Pt| r.0))
            .attr(Circle::Y, y.encode(|r: &Pt| r.0))
            .attr(
                Circle::FILL,
                crate::scale::Sequential::viridis().encode(|r: &Pt| r.1),
            )
            .attr(Circle::RADIUS, Px(8.0));
        let (image, _) = plot.render_resolved(&cx, 300, 200).unwrap();
        let at = |v: f64| {
            let (px, py) = (x.read().eval(v), y.read().eval(v));
            image.get_pixel(px as u32, py as u32).0
        };
        assert_eq!(at(5.0), crate::scale::NULL_COLOR.to_rgba8());
        let viridis = crate::scale::Sequential::viridis().domain(1.0, 3.0);
        assert_eq!(at(2.0), viridis.eval(1.0).to_rgba8());
        assert_eq!(at(8.0), viridis.eval(3.0).to_rgba8());
    }
}

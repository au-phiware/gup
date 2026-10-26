// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! `Selection<T, M>`: rows of `T` drawn as mark `M`, with typed channels
//! (RFC-001 §4). S0a scope: data and `attr`; events, transitions and
//! append are later S-stories.

use crate::append::{Appender, FeedRx};
use crate::channel::{Channel, ChannelDesc, ConstValue, Geometry, HitShape, Mark, Role, Visual};
use crate::column::{ColumnData, ColumnFormat, ColumnStore};
use crate::context::Context;
use crate::cull::{self, PixelBox};
use crate::encoding::{AxisEncoding, Encoding, IntoEncoding, Resource};
use crate::error::{Error, Result};
use crate::geom::Rect;
use crate::input::{Event, EventInfo};
use crate::marks::{Curve, Line};
use crate::order::{DrawEntry, DrawOrder, chunk_spans, draw_entries, polyline_spans};
use crate::pick::PICK_TOLERANCE;
use crate::pick::{Chan, Chans, PickItem, build_items};
use crate::plot::inset::EdgeAxis;
use crate::render::{
    ChunkDraw, Draws, LayerGpu, LayerUniforms, chunk_uniform_offset, chunk_uniforms,
};
use crate::scale::{DynPositionScale, Legend};
use crate::scene::MarkBatch;
use crate::shader::glue::{self, ChannelSource, ChunkBase, Glue, GlueChannel, GlueSpec};
use crate::theme::{Theme, Unbrushed};
#[cfg(feature = "svg")]
use crate::vector::{CpuLayer, LayerVector, MarkKind, VChan};
use std::any::Any;
use std::marker::PhantomData;
use std::sync::Arc;
use std::task::Waker;

type Handler<T> = Box<dyn FnMut(&EventInfo, &T) + Send + Sync>;
type TooltipFn<T> = Box<dyn Fn(&T) -> String + Send + Sync>;

/// The palette LUTs a layer's GPU state was built with.
type Luts = Vec<Vec<[u8; 4]>>;

/// How much CPU data a [`Selection`] keeps once its columns are on the
/// GPU (RFC-001 §3).
///
/// Retained data serves picking (returning `&T`), vector output,
/// accessibility descriptions, re-encoding a channel and uploading to
/// another [`Context`]. Every policy keeps the column stats (which drive
/// domains) and dictionaries, and the last, unfilled chunk's CPU copy, so
/// rows can still be appended. Data is released after each successful
/// [`Plot::resolve`](crate::Plot::resolve).
///
/// | Policy    | Rows (`T`)       | CPU columns      |
/// | --------- | ---------------- | ---------------- |
/// | `Auto`    | ≤ 10M rows: kept | ≤ 10M rows: kept |
/// | `Rows`    | kept             | dropped          |
/// | `Columns` | dropped          | kept             |
/// | `GpuOnly` | dropped          | dropped          |
///
/// Without rows, re-encoding a channel (or resolving on a device that
/// needs a different chunk size) is an error: there is nothing to run the
/// new accessor on. Without rows or CPU columns, so under `GpuOnly`,
/// resolving on another context is an error too. With rows but no CPU
/// columns (`Rows`), another context re-evaluates the columns from the
/// rows.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum Retain {
    /// Keep everything up to [`Retain::AUTO_MAX_ROWS`] rows, and behave
    /// as [`GpuOnly`](Retain::GpuOnly) above that.
    #[default]
    Auto,
    /// Keep the rows; drop the CPU columns of full, uploaded chunks.
    Rows,
    /// Keep the CPU columns; drop the rows.
    Columns,
    /// Keep only stats and dictionaries (and the last chunk's CPU copy).
    GpuOnly,
}

impl Retain {
    /// The most rows [`Retain::Auto`] keeps on the CPU (RFC-001 §3).
    pub const AUTO_MAX_ROWS: u64 = 10_000_000;

    /// Whether a selection of `rows` rows keeps them after upload.
    pub const fn keeps_rows(self, rows: u64) -> bool {
        match self {
            Retain::Auto => rows <= Self::AUTO_MAX_ROWS,
            Retain::Rows => true,
            Retain::Columns | Retain::GpuOnly => false,
        }
    }

    /// Whether a selection of `rows` rows keeps its CPU columns after
    /// upload.
    pub const fn keeps_columns(self, rows: u64) -> bool {
        match self {
            Retain::Auto => rows <= Self::AUTO_MAX_ROWS,
            Retain::Columns => true,
            Retain::Rows | Retain::GpuOnly => false,
        }
    }
}

/// How many rows a selection keeps as rows are appended (RFC-001 §3).
///
/// A windowed layer caps its chunks at `max(1024, n / 4)` rows (or the
/// device's cap, if smaller), so whole chunks can leave: between `n` and
/// about `1.25 × n` rows are held, plus up to one chunk the last append
/// has not filled.
///
/// ```
/// use gup_core::prelude::*;
/// # fn main() -> gup_core::Result<()> {
/// let cx = Context::new_blocking()?;
/// let mut plot = Plot::new();
/// plot.circles(Vec::<(f64, f64)>::new())
///     .x(|p: &(f64, f64)| p.0)
///     .y(|p: &(f64, f64)| p.1)
///     .window(Window::LastRows(2_000));
/// for k in 0..10 {
///     let layer = plot.layer_mut::<(f64, f64), Circle>().unwrap();
///     layer.append((0..1_000).map(|i| (f64::from(k * 1_000 + i), 1.0)))?;
///     plot.resolve(&cx, 320.0, 200.0)?;
/// }
/// // 10,000 rows appended; whole 1,024-row chunks left the front.
/// let layer = plot.layer_mut::<(f64, f64), Circle>().unwrap();
/// assert!((2_000..=2_000 + 2 * 1_024).contains(&layer.len()), "{}", layer.len());
/// # Ok(())
/// # }
/// ```
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum Window {
    /// Keep every row (the default).
    #[default]
    All,
    /// Keep at least the newest `n` rows. Older rows go a whole chunk at a
    /// time, so between `n` and about `1.25 × n` rows are held.
    LastRows(usize),
}

impl Window {
    /// The fewest rows per chunk a window allows (RFC-001 S12c).
    const MIN_CHUNK_ROWS: u32 = 1024;

    /// The cap on rows per chunk: whole-chunk eviction with device-sized
    /// chunks (millions of rows) would never evict a small window.
    fn chunk_cap(self) -> u32 {
        match self {
            Window::All => u32::MAX,
            Window::LastRows(n) => {
                u32::try_from(n.div_ceil(4)).map_or(u32::MAX, |c| c.max(Self::MIN_CHUNK_ROWS))
            }
        }
    }
}

/// Rows of `T` drawn as mark `M`.
///
/// ```
/// use gup_core::prelude::*;
///
/// struct Reading { t: f64, value: f64, temp: f64 }
/// let mut sel = Selection::<Reading, Circle>::new(vec![
///     Reading { t: 0.0, value: 1.0, temp: 10.0 },
/// ]);
/// sel.attr(Circle::X, Linear::new().encode(|r: &Reading| r.t))
///     .attr(Circle::FILL, Sequential::viridis().encode(|r: &Reading| r.temp))
///     .attr(Circle::RADIUS, Px(2.5));
/// ```
pub struct Selection<T, M: Mark> {
    /// The rows not yet released: rows `released..` of the selection.
    rows: Vec<T>,
    /// How many leading rows are not in `rows`: dropped after upload
    /// ([`Retain`]) or evicted ([`Window`]). At least `evicted`.
    released: usize,
    /// How many leading rows the [`Window`] evicted. Row ids count from
    /// the selection's first row ever, so an id stays valid until its row
    /// leaves; the column store numbers the kept rows from 0.
    evicted: u64,
    /// Bumped whenever stored rows are renumbered (an eviction, or the
    /// columns evaluated again), so a GPU pick drawn before it is dropped.
    epoch: u64,
    /// Whether the brushed-row bits must be written whole at the next
    /// prepare (the stored rows were renumbered).
    bits_stale: bool,
    window: Window,
    retain: Retain,
    /// Draw order among the plot's items (default 0).
    z: i32,
    encodings: Vec<Option<Encoding<T>>>,
    /// Closures on position channels, by channel slot, waiting to be
    /// encoded through the plot's scales (GUP-447). A slot with one has no
    /// encoding until [`Layer::bind_axes`] makes it a column.
    axes: Vec<Option<AxisFn<T>>>,
    /// The plot's axes generation the closures were last bound at (0: not).
    axes_bound: u64,
    /// Legend titles, by channel slot.
    legend_titles: Vec<Option<String>>,
    /// Event handlers, in registration order.
    handlers: Vec<(Event, Handler<T>)>,
    /// The tooltip text of a hovered row.
    tooltip: Option<TooltipFn<T>>,
    /// Evaluated columns, one per column-encoded channel in channel order.
    columns: Option<ColumnStore>,
    /// The size-descending draw order of the stored rows, when the mark's
    /// size channel is a column (GUP-431).
    order: Option<DrawOrder>,
    /// A cap on rows per chunk below the device's (a test seam).
    max_chunk_rows: Option<u32>,
    /// GPU state from the last `prepare` and the LUTs it holds. Later
    /// prepares with the same program, context and columns only write
    /// uniforms into it.
    gpu: Option<(Arc<LayerGpu>, Luts)>,
    /// Whether the last `prepare` generated the brushed glue (rows outside
    /// the brush dim, GUP-458).
    brushable: bool,
    /// The theme's opacity for unbrushed rows at the last `prepare`.
    unbrushed: Unbrushed,
    /// The brushed data rows, sorted, while the plot has a brush.
    brush: Option<Vec<usize>>,
    /// Rows other threads pushed through [`appender`](Self::appender), and
    /// the waker given by the host.
    feed: Option<FeedRx<T>>,
    waker: Option<Waker>,
    /// How many appends happened (part of the revision).
    appended: u64,
    /// A line's curve ([`Linear`](Curve::Linear) for every other mark):
    /// the glue's variant and the CPU geometry of picks and SVG.
    curve: Curve,
    /// The vertices drawn per instance (GUP-550):
    /// [`Mark::VERTICES_PER_INSTANCE`], times the pieces of a curve.
    vertices: u32,
    /// For a curve with control values ([`Curve::MonotoneX`]), the x and y
    /// (data units) of the last three stored rows: an append recomputes
    /// the control values of the two rows before its first row from them
    /// (GUP-551).
    tail: Vec<[f64; 2]>,
    _mark: PhantomData<fn() -> M>,
}

/// Where a line's control values come from (GUP-551): the column indices
/// of its x and y.
#[derive(Copy, Clone, Debug)]
struct ControlSource {
    x: usize,
    y: usize,
}

/// The numbers of an evaluated numeric column.
fn numbers(data: &ColumnData) -> &[f64] {
    match data {
        ColumnData::Values(v) => v,
        ColumnData::Codes(_) => unreachable!("control sources are numeric columns"),
    }
}

/// The last three of `x`, `y`'s rows, after `tail`'s.
fn tail_of(tail: &[[f64; 2]], x: &[f64], y: &[f64]) -> Vec<[f64; 2]> {
    let rows: Vec<[f64; 2]> = tail
        .iter()
        .copied()
        .chain(x.iter().zip(y).map(|(&x, &y)| [x, y]))
        .collect();
    rows[rows.len().saturating_sub(3)..].to_vec()
}

impl<T, M: Mark> std::fmt::Debug for Selection<T, M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Selection")
            .field("mark", &M::NAME)
            .field(
                "rows",
                &(self.released + self.rows.len() - self.evicted as usize),
            )
            .field("window", &self.window)
            .field("retain", &self.retain)
            .field("encodings", &self.encodings)
            .finish_non_exhaustive()
    }
}

impl<T: Send + Sync + 'static> Selection<T, Line> {
    /// How the line runs from each row to the next (default
    /// [`Curve::Linear`]). A step curve draws each segment as 2 or 3
    /// straight pieces, and a monotone curve as 16 pieces of a cubic;
    /// picks and SVG output follow the same path. A straight line draws
    /// exactly as before curves existed. Switching to or from
    /// [`Curve::MonotoneX`] evaluates the columns again at the next
    /// resolve (its control values are two more columns), which needs the
    /// rows (see [`Retain`]).
    ///
    /// ```
    /// use gup_core::prelude::*;
    ///
    /// # fn main() -> gup_core::Result<()> {
    /// let stock = vec![(0.0, 40.0), (3.0, 25.0), (5.0, 60.0), (9.0, 35.0)];
    ///
    /// let mut plot = Plot::new();
    /// let (x, y) = (plot.x(Linear::new()), plot.y(Linear::new()));
    /// plot.add(Selection::<(f64, f64), Line>::new(stock))
    ///     .x_with(x.encode(|r: &(f64, f64)| r.0))
    ///     .y_with(y.encode(|r: &(f64, f64)| r.1))
    ///     .curve(Curve::StepAfter);
    /// let cx = Context::new_blocking()?;
    /// let image = plot.render(&cx, 320, 200)?;
    /// assert!(image.pixels().any(|p| p.0 != [255, 255, 255, 255]));
    /// # Ok(())
    /// # }
    /// ```
    pub fn curve(&mut self, curve: Curve) -> &mut Self {
        if curve != self.curve {
            if curve.has_controls() != self.curve.has_controls() {
                self.columns = None;
            }
            self.curve = curve;
            self.vertices = Line::VERTICES_PER_INSTANCE * curve.pieces();
            self.gpu = None;
        }
        self
    }
}

impl<T: Send + Sync + 'static, M: Mark> Selection<T, M> {
    /// A selection over `rows`. Channels not set with [`attr`](Self::attr)
    /// use the mark's defaults.
    pub fn new(rows: impl Into<Vec<T>>) -> Self {
        Self {
            rows: rows.into(),
            released: 0,
            evicted: 0,
            epoch: 0,
            bits_stale: false,
            window: Window::All,
            retain: Retain::Auto,
            z: 0,
            encodings: M::CHANNELS.iter().map(|_| None).collect(),
            axes: M::CHANNELS.iter().map(|_| None).collect(),
            axes_bound: 0,
            legend_titles: vec![None; M::CHANNELS.len()],
            handlers: Vec::new(),
            tooltip: None,
            columns: None,
            order: None,
            max_chunk_rows: None,
            gpu: None,
            brushable: false,
            unbrushed: Unbrushed::Fade(1.0),
            brush: None,
            feed: None,
            waker: None,
            appended: 0,
            curve: Curve::Linear,
            vertices: M::VERTICES_PER_INSTANCE,
            tail: Vec::new(),
            _mark: PhantomData,
        }
    }

    /// Cap the rows per column chunk below what the device allows, so a
    /// small selection spans several chunks (and several draws). A test
    /// seam for the multi-chunk paths (RFC-001 S4a), not a tuning knob.
    #[doc(hidden)]
    pub fn max_chunk_rows(&mut self, rows: u32) -> &mut Self {
        self.max_chunk_rows = Some(rows);
        self.columns = None;
        self.gpu = None;
        self
    }

    /// Append `rows`: only they are evaluated, and the next resolve of the
    /// plot holding the selection uploads only their bytes, at the tail of
    /// the last chunk (or in new chunks). Auto domains refit to the grown
    /// stats on that resolve (a uniform write), and new keys get the next
    /// dictionary codes. It works under every [`Retain`] policy: the last
    /// chunk keeps its CPU copy, and appended rows are released after their
    /// upload like the rest. To append from another thread, see
    /// [`appender`](Self::appender).
    ///
    /// ```
    /// use gup_core::prelude::*;
    /// # fn main() -> gup_core::Result<()> {
    /// let mut plot = Plot::new();
    /// plot.circles(vec![(0.0, 1.0)]).x(|p: &(f64, f64)| p.0).y(|p: &(f64, f64)| p.1);
    /// plot.layer_mut::<(f64, f64), Circle>().unwrap().append([(1.0, 2.0)])?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn append(&mut self, rows: impl IntoIterator<Item = T>) -> Result<()> {
        self.appended = self.appended.wrapping_add(1);
        let start = self.rows.len();
        self.rows.extend(rows);
        let controls = match self.columns {
            Some(_) => self.control_source()?,
            None => None,
        };
        if let Some(store) = &mut self.columns {
            let new = &self.rows[start..];
            // The same columns, in the same order, as `column_encodings`.
            let mut values = self
                .encodings
                .iter_mut()
                .filter_map(|e| match e {
                    Some(Encoding::Column(c)) => Some(c.evaluate(new)),
                    _ => None,
                })
                .collect::<Result<Vec<_>>>()?;
            if let Some(order) = &mut self.order {
                let runs = runs(self.window, store, new.len());
                order.push_runs(&mut values, &runs);
            }
            // A monotone line's control values: the new rows', computed
            // with the last stored rows before them, which also gives the
            // new values of the two rows before the first new row (their
            // tangents see the new rows; a row's tangent depends on the
            // rows within two of it).
            let mut rewrite = Vec::new();
            if let Some(c) = controls {
                let (xs, ys) = (numbers(&values[c.x]), numbers(&values[c.y]));
                let t = self.tail.len();
                let wx: Vec<f64> = self
                    .tail
                    .iter()
                    .map(|p| p[0])
                    .chain(xs.iter().copied())
                    .collect();
                let wy: Vec<f64> = self
                    .tail
                    .iter()
                    .map(|p| p[1])
                    .chain(ys.iter().copied())
                    .collect();
                let (c1, c2) = crate::monotone::controls(&wx, &wy);
                let base = store.rows() - t as u64;
                rewrite.extend((t.saturating_sub(2)..t).map(|k| (base + k as u64, c1[k], c2[k])));
                self.tail = tail_of(&self.tail, xs, ys);
                values.push(ColumnData::Values(c1[t..].to_vec()));
                values.push(ColumnData::Values(c2[t..].to_vec()));
            }
            store.append(new.len(), &values)?;
            let k = store.formats().len() - 2;
            for (row, c1, c2) in rewrite {
                store.rewrite(row, k, c1);
                store.rewrite(row, k + 1, c2);
            }
            // Appends interleaved with the stored rows into many draws:
            // evaluate every row again, in one sorted run (while the rows
            // are kept).
            if self.all_rows_held() && self.order.as_ref().is_some_and(DrawOrder::needs_compaction)
            {
                self.columns = None;
                return Ok(());
            }
            self.evict();
        }
        Ok(())
    }

    /// Keep at least the newest `n` rows (default [`Window::All`]): after
    /// each append, whole chunks whose rows are all older than the newest
    /// `n` leave the front. Kept rows keep their [`RowId`](crate::RowId)s
    /// (counted from the first row ever); an evicted row's id picks and
    /// reads nothing. Changing the window changes the chunk size, so the
    /// columns are evaluated again at the next resolve: set it before the
    /// first resolve, or keep the rows (see [`Retain`]).
    ///
    /// See [`Window`] for an example.
    pub fn window(&mut self, window: Window) -> &mut Self {
        self.window = window;
        self
    }

    /// Whether every row not evicted is still in `rows` (none released).
    /// Whether position channel `name` is ignored: a box in
    /// centre-plus-extent mode (a constant, non-zero width or height) draws
    /// centre +- extent / 2, and ignores its far edge.
    fn ignored_end(&self, name: &str) -> bool {
        let extent_set = |name: &str| {
            M::CHANNELS
                .iter()
                .position(|d| d.name == name)
                .is_some_and(|i| match &self.encodings[i] {
                    Some(Encoding::Const(v)) => matches!(v, ConstValue::F32(w) if *w != 0.0),
                    Some(Encoding::Derived(f)) => {
                        matches!(f(), ConstValue::F32(w) if w != 0.0)
                    }
                    _ => false,
                })
        };
        M::HIT == HitShape::AlignedBox
            && match name {
                "x_end" => extent_set("width"),
                "y_end" => extent_set("height"),
                _ => false,
            }
    }

    fn all_rows_held(&self) -> bool {
        self.released as u64 == self.evicted
    }

    /// Evict the whole chunks the [`Window`] lets go, from the front, and
    /// renumber what stays (RFC-001 S12c): chunk row bases, the draw order
    /// and, at the next prepare, the brushed-row bits. Writes no column
    /// bytes.
    fn evict(&mut self) {
        let (Window::LastRows(n), Some(store)) = (self.window, &mut self.columns) else {
            return;
        };
        let chunks = store.evictable(n as u64);
        if chunks == 0 {
            return;
        }
        let rows = store.evict_front(chunks);
        if let Some(order) = &mut self.order {
            order.evict(rows);
        }
        self.evicted += rows;
        let drop = (self.evicted as usize).saturating_sub(self.released);
        self.rows.drain(..drop.min(self.rows.len()));
        self.released = self.released.max(self.evicted as usize);
        self.epoch = self.epoch.wrapping_add(1);
        self.bits_stale = true;
    }

    /// A handle that appends rows to this selection from any thread. Rows
    /// pushed between two frames are drained into one [`append`](Self::append)
    /// by the next resolve of the plot holding the selection.
    pub fn appender(&mut self) -> Appender<T> {
        let feed = self.feed.get_or_insert_with(FeedRx::new);
        if let Some(w) = &self.waker {
            feed.set_waker(Some(w));
        }
        feed.appender()
    }

    /// Number of rows, including any dropped after upload but not those
    /// the [`Window`] evicted.
    pub fn len(&self) -> usize {
        self.released + self.rows.len() - self.evicted as usize
    }

    /// Whether there are no rows.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Set how much CPU data to keep once the columns are on the GPU
    /// (default [`Retain::Auto`]). Policies that drop the CPU columns
    /// inset the edges of an explicit-domain axis from chunk statistics, so
    /// a few pixels more than the exact rows would need (GUP-528).
    pub fn retain(&mut self, policy: Retain) -> &mut Self {
        self.retain = policy;
        self
    }

    /// Set the layer's draw order in a [`Plot`](crate::Plot) (default 0,
    /// within `-99..=99`): higher draws on top; equal `z` draws in the
    /// order the layers were added.
    ///
    /// Within a layer, rows draw in data order, or largest first when the
    /// mark's size is encoded from a column. To draw emphasised rows (a
    /// highlighted category, say) above the rest, give them a layer of
    /// their own, over the same scales, with a higher `z`.
    pub fn z(&mut self, z: i32) -> &mut Self {
        self.z = z;
        self
    }

    /// Call `handler` with the row when `event` happens on one of this
    /// layer's marks. Handlers for one event run in registration order. Not
    /// called for rows dropped by [`Retain`] (there is no `&T`).
    ///
    /// ```
    /// use gup_core::prelude::*;
    ///
    /// let mut sel = Selection::<(f64, f64), Circle>::new(vec![(1.0, 2.0)]);
    /// sel.on(Event::Click, |ev, d: &(f64, f64)| println!("{:?} {:?}", ev.row, d));
    /// ```
    pub fn on(
        &mut self,
        event: Event,
        handler: impl FnMut(&EventInfo, &T) + Send + Sync + 'static,
    ) -> &mut Self {
        self.handlers.push((event, Box::new(handler)));
        self
    }

    /// Show `text(row)` beside a hovered row; `'\n'` separates lines. An
    /// empty text shows nothing. Not shown for rows dropped by [`Retain`].
    pub fn tooltip(&mut self, text: impl Fn(&T) -> String + Send + Sync + 'static) -> &mut Self {
        self.tooltip = Some(Box::new(text));
        self
    }

    /// Whether the rows were dropped after upload (see [`Retain`]).
    pub fn rows_released(&self) -> bool {
        !self.all_rows_held()
    }

    /// The evaluated column store, for tests.
    #[cfg(test)]
    pub(crate) fn store(&self) -> Option<&ColumnStore> {
        self.columns.as_ref()
    }

    /// The evaluated column store, mutably, for tests.
    #[cfg(test)]
    pub(crate) fn store_mut(&mut self) -> Option<&mut ColumnStore> {
        self.columns.as_mut()
    }

    /// Drive `channel` with a constant or an encoding. The encoding's
    /// output type must be the channel's visual type, or this does not
    /// compile.
    pub fn attr<V: Visual, E, K>(&mut self, channel: Channel<M, V>, encoding: E) -> &mut Self
    where
        E: IntoEncoding<T, V, K>,
    {
        let slot = usize::from(channel.slot());
        self.encodings[slot] = Some(encoding.into_encoding());
        self.axes[slot] = None;
        // S0a re-evaluates every column; per-column invalidation is S4.
        self.columns = None;
        self.gpu = None;
        self
    }

    /// Drive a position `channel` with a closure that the plot encodes
    /// through its own scale for that axis, when the plot resolves. This
    /// backs the plain setters `#[derive(Mark)]` generates for `x`, `y`,
    /// `x_end` and `y_end`.
    #[doc(hidden)]
    pub fn attr_axis<F, D>(&mut self, channel: Channel<M, crate::channel::Px>, f: F) -> &mut Self
    where
        F: Fn(&T) -> D + Send + Sync + 'static,
        D: crate::ColumnValue,
    {
        let slot = usize::from(channel.slot());
        self.encodings[slot] = None;
        self.axes[slot] = Some(Arc::new(move |r| f(r).to_f64()));
        self.axes_bound = 0;
        self.columns = None;
        self.gpu = None;
        self
    }

    /// Title the legend of the colour encoded on `channel`, drawn above it
    /// when the plot places the legend. Without a title, the legend has
    /// none.
    pub fn legend_title(
        &mut self,
        channel: Channel<M, crate::channel::Color>,
        title: impl Into<String>,
    ) -> &mut Self {
        self.legend_titles[usize::from(channel.slot())] = Some(title.into());
        self
    }

    fn column_encodings(&self) -> impl Iterator<Item = (usize, &dyn crate::DynColumnEncoding<T>)> {
        self.encodings
            .iter()
            .enumerate()
            .filter_map(|(i, e)| match e {
                Some(Encoding::Column(c)) => Some((i, c.as_ref())),
                _ => None,
            })
    }

    /// Where the curve's control values come from, if it has them
    /// ([`Curve::MonotoneX`]): x and y must be numeric columns.
    fn control_source(&self) -> Result<Option<ControlSource>> {
        if !self.curve.has_controls() {
            return Ok(None);
        }
        let (mut x, mut y) = (None, None);
        for (k, (slot, c)) in self.column_encodings().enumerate() {
            if !c.links()[0].input_format().has_validity() {
                continue;
            }
            match M::CHANNELS[slot].name {
                "x" => x = Some(k),
                "y" => y = Some(k),
                _ => {}
            }
        }
        match (x, y) {
            (Some(x), Some(y)) => Ok(Some(ControlSource { x, y })),
            _ => Err(Error::config(
                "curve",
                format!(
                    "{} layer: Curve::MonotoneX computes its tangents from the rows' x and y, \
                     so both must be encoded from numeric columns (not constants or keys)",
                    M::NAME
                ),
            )),
        }
    }

    /// Run the accessors (once) into a column store chunked for `cx`'s
    /// device. A monotone line's store has two more columns after the
    /// channels': each row's control values ([`crate::monotone`]), in y's
    /// format.
    fn evaluate(&mut self, cx: &Context) -> Result<&mut ColumnStore> {
        let controls = self.control_source()?;
        let mut formats: Vec<_> = self
            .column_encodings()
            .map(|(_, c)| c.links()[0].input_format())
            .collect();
        if let Some(c) = controls {
            formats.extend([formats[c.y]; 2]);
        }
        let chunk_rows = ColumnStore::chunk_rows_for(&cx.caps().limits, &formats)
            .min(self.window.chunk_cap())
            .min(self.max_chunk_rows.unwrap_or(u32::MAX));
        let stale = match &self.columns {
            None => Some(
                "a channel was encoded, `max_chunk_rows` changed, or the curve now reads control \
                 values",
            ),
            Some(c) if c.chunk_rows() != chunk_rows => Some(
                "this device (or the window, or `max_chunk_rows`) needs a different chunk size",
            ),
            // Released CPU columns (`Retain::Rows`, `GpuOnly`) cannot go to
            // another context; with the rows, evaluate them again.
            Some(c) if !c.uploadable_to(cx) => {
                Some("the columns are on another context and their CPU copy was dropped")
            }
            Some(_) => None,
        };
        if let Some(reason) = stale {
            if !self.all_rows_held() {
                return Err(Error::config(
                    "layer",
                    format!(
                        "{} layer: its columns must be evaluated again ({reason}), but its {} \
                         rows were dropped after upload (Retain::{:?}); keep them with \
                         `.retain(Retain::Rows)` or `Retain::Columns`",
                        M::NAME,
                        self.len(),
                        self.retain
                    ),
                ));
            }
            let rows = &self.rows;
            let mut values = self
                .encodings
                .iter_mut()
                .filter_map(|e| match e {
                    Some(Encoding::Column(c)) => Some(c.evaluate(rows)),
                    _ => None,
                })
                .collect::<Result<Vec<_>>>()?;
            self.order = self.size_order(&values);
            if let Some(c) = controls {
                let (xs, ys) = (numbers(&values[c.x]), numbers(&values[c.y]));
                let (c1, c2) = crate::monotone::controls(xs, ys);
                self.tail = tail_of(&[], xs, ys);
                values.push(ColumnData::Values(c1));
                values.push(ColumnData::Values(c2));
            }
            let mut store = ColumnStore::new(formats, chunk_rows)?;
            if let Some(order) = &mut self.order {
                order.push_runs(&mut values, &runs(self.window, &store, self.rows.len()));
                order.mark_evaluated();
            }
            store.append(self.rows.len(), &values)?;
            self.columns = Some(store);
            // Stored rows (and so GPU pick ids) are new.
            self.epoch = self.epoch.wrapping_add(1);
            self.bits_stale = true;
            self.evict();
        }
        Ok(self.columns.as_mut().expect("evaluated above"))
    }

    /// The draw order for columns `values`, when the mark's size channel
    /// is one of them: largest first, by value, or by negated value when
    /// the encoding maps a larger value to a smaller size (its image of
    /// the values' extent, end by end; an encoding without an image is
    /// taken as increasing).
    fn size_order(&self, values: &[crate::column::ColumnData]) -> Option<DrawOrder> {
        // A polyline joins rows in data order, whatever their size.
        if M::GEOMETRY == Geometry::Polyline {
            return None;
        }
        let mut column = 0;
        for (d, e) in M::CHANNELS.iter().zip(&self.encodings) {
            let Some(Encoding::Column(c)) = e else {
                continue;
            };
            if d.role == Some(Role::Size) {
                let (lo, hi) = match &values[column] {
                    crate::column::ColumnData::Values(v) => v
                        .iter()
                        .filter(|v| v.is_finite())
                        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), &v| {
                            (lo.min(v), hi.max(v))
                        }),
                    crate::column::ColumnData::Codes(_) => (0.0, 1.0),
                };
                let end = |v: f64| c.image((v, v)).map(|(a, _)| a);
                let reversed = matches!((end(lo), end(hi)), (Some(a), Some(b)) if a > b);
                return Some(DrawOrder::new(column, reversed));
            }
            column += 1;
        }
        None
    }

    /// Two different shader functions with one WGSL import path (two
    /// `#[wgsl_function(path = "…")]`s, say) would share the glue's import
    /// alias, so one would silently call the other's WGSL. Refuse them,
    /// naming both.
    fn check_module_paths(&self) -> Result<()> {
        let mut seen: Vec<&dyn crate::encoding::DynShaderFn> = Vec::new();
        for enc in self.encodings.iter().flatten() {
            let Encoding::Column(c) = enc else { continue };
            for func in c.links() {
                let path = func.module().import_path;
                let clash = seen.iter().find(|s| {
                    s.module().import_path == path && !std::ptr::eq(s.module(), func.module())
                });
                if let Some(other) = clash {
                    return Err(Error::Compose {
                        module: format!("{} layer", M::NAME),
                        report: format!(
                            "`{}` and `{}` are different WGSL modules with one import path, \
                             `{path}`; import paths must be unique, so give one of them its \
                             own `#[wgsl_function(path = \"…\")]`",
                            other.type_name(),
                            func.type_name()
                        ),
                    });
                }
                seen.push(func);
            }
        }
        Ok(())
    }

    /// The weight of unbrushed rows' own colour: 1.0 while there is no
    /// brush; with one, the opacity of `Fade` or 0.0 for `Grey`.
    fn brush_dim(&self) -> f32 {
        match (&self.brush, self.unbrushed) {
            (None, _) => 1.0,
            (Some(_), Unbrushed::Fade(o)) => o,
            (Some(_), Unbrushed::Grey(_)) => 0.0,
        }
    }

    /// Write the brushed-row bits of `gpu` from the layer's brush (nothing
    /// without a brush or bits; an unbrushed layer needs none, its factor
    /// is 1.0).
    fn write_bits(&self, cx: &Context, gpu: &LayerGpu) {
        let (Some(brushed), Some(words), Some(store)) =
            (&self.brush, gpu.brush_words(), &self.columns)
        else {
            return;
        };
        let mut bits = vec![0u32; words];
        let mut set = |stored: u64| bits[(stored / 32) as usize] |= 1 << (stored % 32);
        // Brushed rows are row ids; stored rows count from the first kept.
        let first = self.evicted;
        match &self.order {
            // Stored rows are in size order: find each one's data row.
            Some(order) => {
                for s in 0..store.rows() {
                    if brushed
                        .binary_search(&((first + order.row_of(s)) as usize))
                        .is_ok()
                    {
                        set(s);
                    }
                }
            }
            None => {
                for &r in brushed {
                    if let Some(s) = (r as u64).checked_sub(first)
                        && s < store.rows()
                    {
                        set(s);
                    }
                }
            }
        }
        gpu.write_brush_bits(cx, &bits);
    }

    /// The widest `width` (logical pixels) the layer draws: a constant's,
    /// or the image of a column's extent; 0 for a mark without `width`.
    fn max_width(&self) -> f32 {
        let mut column = 0;
        for (d, e) in M::CHANNELS.iter().zip(&self.encodings) {
            let is_width = d.name == "width";
            let value = |v: &ConstValue| match v {
                ConstValue::F32(w) => *w,
                ConstValue::Vec4(_) => 0.0,
            };
            match e {
                Some(Encoding::Column(c)) => {
                    column += 1;
                    if is_width {
                        return self
                            .columns
                            .as_ref()
                            .and_then(|s| s.stats(column - 1))
                            .and_then(|s| c.image(s.extent()))
                            .map_or(0.0, |(lo, hi)| lo.abs().max(hi.abs()) as f32);
                    }
                }
                _ if !is_width => {}
                Some(Encoding::Const(v)) => return value(v),
                Some(Encoding::Derived(f)) => return value(&f()),
                None => return value(&d.default),
            }
        }
        0.0
    }

    fn glue(&self) -> Glue {
        let channels = M::CHANNELS
            .iter()
            .zip(&self.encodings)
            .map(|(desc, enc)| GlueChannel {
                name: desc.name,
                wgsl_type: desc.wgsl_type,
                role: desc.role,
                source: match enc {
                    Some(Encoding::Column(c)) => ChannelSource::Column(c.links()),
                    _ => ChannelSource::Const,
                },
            })
            .collect();
        glue::emit(&GlueSpec {
            mark_name: M::NAME,
            mark_module: M::MODULE,
            channels,
            validity: self.columns.as_ref().is_some_and(ColumnStore::has_nulls),
            polyline: M::GEOMETRY == Geometry::Polyline,
            gpu_pick: M::GPU_PICK,
            brushed: self.brushable.then_some(match self.unbrushed {
                Unbrushed::Fade(_) => glue::BrushStyle::Fade,
                Unbrushed::Grey(_) => glue::BrushStyle::Grey,
            }),
            variant: self.curve.variant(),
            controls: self.curve.has_controls().then(|| {
                M::CHANNELS
                    .iter()
                    .position(|d| d.name == "y")
                    .expect("a mark with control values has y")
            }),
        })
    }

    /// The `Chunk` uniform entries (laid out as `layout`) of every draw
    /// entry of the evaluated store ([`draw_entries`]: one per chunk, plus
    /// a polyline's bridges): the entry's first row and each relative
    /// link's base in `relative`. A channel's first link reads the stored
    /// column, so its base is `(origin - d0)` from the f64 origin of the
    /// chunk it reads (a polyline's next-row base, the next chunk's in a
    /// bridge); a later link reads an absolute value (origin 0). A hi/lo
    /// link gets its base as a hi/lo pair.
    fn chunk_uniform_bytes(
        &self,
        layout: &crate::shader::StructLayout,
        relative: &[ChunkBase],
    ) -> Result<Vec<u8>> {
        let store = self.columns.as_ref().ok_or_else(|| {
            Error::config("layer", "prepare called before the columns were evaluated")
        })?;
        let entries = draw_entries(M::GEOMETRY, store.chunks());
        let channel_columns = self.column_encodings().count();
        // (base, column index, link function) per relative link; a control
        // column's base is its own column's origin.
        let relative: Vec<_> = relative
            .iter()
            .map(|b| {
                let (col, (_, c)) = self
                    .column_encodings()
                    .enumerate()
                    .find(|(_, (i, _))| *i == b.channel)
                    .expect("relative bases belong to column channels");
                let col = b.control.map_or(col, |k| channel_columns + k);
                (b, col, c.links()[b.link])
            })
            .collect();
        let mut bytes = chunk_uniforms(entries.len(), layout.span);
        for (e, d) in entries.iter().enumerate() {
            let (chunk, next) = (&store.chunks()[d.this], &store.chunks()[d.next]);
            let entry = &mut bytes[chunk_uniform_offset(e)..][..layout.span as usize];
            let first = chunk.row_base() + u64::from(d.first);
            let row_base = u32::try_from(first).map_err(|_| {
                Error::config(
                    "column store",
                    format!(
                        "{}: chunk {} starts at row {first}, past the u32 row index",
                        M::NAME,
                        d.this
                    ),
                )
            })?;
            write_field(entry, layout, "row_base", &row_base.to_le_bytes())?;
            if layout.offset("valid_offset").is_some() {
                write_field(entry, layout, "valid_offset", &d.first.to_le_bytes())?;
                write_field(entry, layout, "next_offset", &d.next_first.to_le_bytes())?;
            }
            for (base, col, func) in &relative {
                let origin = match (base.link, base.next) {
                    (0, false) => chunk.columns()[*col].origin(),
                    (0, true) => next.columns()[*col].origin(),
                    _ => 0.0,
                };
                let [hi, lo] = base.format.split(func.chunk_base(origin));
                write_field(entry, layout, &base.member, &hi.to_le_bytes())?;
                if base.format == ColumnFormat::F32x2Relative {
                    let member = format!("{}_lo", base.member);
                    write_field(entry, layout, &member, &lo.to_le_bytes())?;
                }
            }
        }
        Ok(bytes)
    }
}

type AxisFn<T> = Arc<dyn Fn(&T) -> f64 + Send + Sync>;

/// How far a layer's marks reach past their positions, in pixels
/// ([`Layer::overhang`]).
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub(crate) struct Overhang {
    /// Every size channel (a radius, a stroke width): both axes.
    pub size: f32,
    /// Half a constant or derived width: the x axis only.
    pub x: f32,
    /// Half a constant or derived height: the y axis only.
    pub y: f32,
}

/// The runs a batch of `rows` rows appended to `store` is sorted in (see
/// [`DrawOrder::push_runs`]): one under [`Window::All`]; under a window,
/// one per chunk the batch fills, so each chunk holds a contiguous run of
/// data rows and whole chunks leave in data order.
fn runs(window: Window, store: &ColumnStore, rows: usize) -> Vec<usize> {
    if window == Window::All {
        return vec![rows];
    }
    // Every chunk but the last is full, and the first starts at row 0.
    let chunk = store.chunk_rows() as usize;
    let mut room = chunk - (store.rows() % chunk as u64) as usize;
    let mut left = rows;
    let mut out = Vec::with_capacity(rows.div_ceil(chunk) + 1);
    while left > 0 {
        let take = left.min(room);
        out.push(take);
        left -= take;
        room = chunk;
    }
    out
}

/// A plot layer: the object-safe face of a [`Selection`] (internal; the
/// public `Layer`/`Chart` traits are RFC-001 S7).
pub(crate) trait Layer: wgpu::WasmNotSendSync {
    /// Start a fit pass on every link of every column encoding, so a scale
    /// shared by several channels or layers unions from scratch. Called
    /// on every layer before any [`fit_domains`](Self::fit_domains).
    fn begin_fit(&self);
    /// The wrapping sum of the revisions of every link of every column
    /// encoding (see [`ShaderFn::revision`](crate::ShaderFn::revision)).
    fn revision(&self) -> u64;

    /// Encode the closures on position channels through the plot's scales
    /// (see [`Selection::attr_axis`]), unless that was done at this
    /// `generation` of the plot's scales.
    fn bind_axes(
        &mut self,
        generation: u64,
        x: &dyn DynPositionScale,
        y: &dyn DynPositionScale,
    ) -> Result<()>;
    /// The layer's z-order among the plot's items.
    fn z(&self) -> i32;
    /// Append every row queued through the layer's appenders as one batch
    /// (so one draw-order batch). True when rows came.
    fn drain_feed(&mut self) -> Result<bool>;
    /// Whether rows are queued and not yet drained.
    fn feed_pending(&self) -> bool;
    /// Set the host's waker, which appenders wake when rows arrive.
    fn set_waker(&mut self, waker: Option<&Waker>);
    /// Evaluate accessors (chunked for `cx`'s device) and fit every
    /// data-driven domain to its column's stats across every chunk.
    fn fit_domains(&mut self, cx: &Context) -> Result<()>;
    /// The identity ([`DynPositionScale::identity`]) of the scale that
    /// encodes the layer's first column on a y channel, if any.
    fn y_axis_id(&self) -> Option<usize>;
    /// The constant and derived position values (`Px`), as (is x, pixel):
    /// they join the automatic domains (GUP-494).
    fn fixed_positions(&self) -> Vec<(bool, f32)>;
    /// How far marks may extend past their position (e.g. a constant
    /// radius), in pixels.
    fn overhang(&self) -> Overhang;
    /// The largest value any non-position `f32` channel can take (a
    /// radius, a width, a stroke width, an opacity): how far a mark can
    /// reach from its position. `INFINITY` when a channel's range is not
    /// known.
    fn cull_margin(&self) -> f32;
    /// Which of the layer's draw entries (`order::draw_entries`) can put a
    /// pixel inside `plot`, from the chunks' position stats and the
    /// current scales. Empty when the layer has no column store.
    fn visible_entries(&self, plot: Rect) -> Vec<bool>;
    /// GUP-528: the marks that can reach within an axis's edge allowance,
    /// pushed onto `out[i]` as `(t, h)` for each asked axis `axes[i]`
    /// (x, y, right y): `t` is the mark's position normalized to the axis
    /// range, `h` its half-extent in pixels along the axis. Rows are
    /// decoded while `budget` lasts, else chunk statistics bound them.
    fn edge_marks(
        &self,
        axes: &[Option<EdgeAxis>; 3],
        budget: &mut usize,
        out: &mut [Vec<(f32, f32)>; 3],
    );
    /// Generate the glue, link it (cached), write uniforms and upload.
    ///
    /// `brushable`: the glue dims rows outside the plot's brush
    /// ([`write_brush`](Self::write_brush) says which).
    fn prepare(&mut self, cx: &Context, theme: &Theme, brushable: bool) -> Result<MarkBatch>;
    /// Set the layer's brushed data rows (`None`: no brush) and write them,
    /// as one bit per stored row, into the prepared GPU state. A layer
    /// whose glue does not dim writes nothing; its next `prepare` writes
    /// the bits again if the GPU state was rebuilt.
    fn write_brush(&mut self, cx: &Context, rows: Option<&[usize]>) -> Result<()>;
    /// Apply the plot's theme to every column encoding (see
    /// [`ShaderFn::apply_theme`](crate::ShaderFn::apply_theme)). Called
    /// before fitting.
    fn apply_theme(&mut self, theme: &Theme);
    /// Drop the CPU data the [`Retain`] policy does not keep. Called after
    /// a successful [`prepare`](Self::prepare), so everything dropped is
    /// on the GPU.
    fn release(&mut self);
    /// The legends of the layer's colour encodings, with their titles
    /// (RFC-001 S7): each `Color`-role channel encoded by a column of one
    /// link that has a legend. Chains have none.
    fn legends(&self) -> Vec<(Option<String>, Legend)>;
    /// Push the layer's drawn marks as pick items (RFC-001 §9): nothing for
    /// an unpickable mark, an unevaluated layer, or one whose rows and CPU
    /// columns were both released.
    ///
    /// `visible` is [`visible_entries`](Self::visible_entries)'s result
    /// (empty: every entry): rows of culled entries are skipped.
    fn hit_items(&mut self, layer: usize, visible: &[bool], out: &mut Vec<PickItem>) -> Result<()>;
    /// Whether the layer's mark has a GPU pick (`#[mark(.., gpu_pick)]`).
    fn gpu_pickable(&self) -> bool;
    /// Whether the CPU can pick the layer: it kept its CPU columns or its
    /// rows (or has nothing to pick).
    fn cpu_readable(&self) -> bool;
    /// Row `row` (an index into the rows the selection was given), if the
    /// layer still holds it.
    fn row_any(&self, row: usize) -> Option<&dyn Any>;
    /// The layer's draw order, if it draws by size.
    #[cfg(test)]
    fn draw_order(&self) -> Option<&DrawOrder>;

    /// The row id of stored row `stored`: through the draw order, if the
    /// layer draws by size, from the first row the window kept. How a GPU
    /// pick maps a drawn instance (`row_base + instance_index`) to a row.
    fn row_id(&self, stored: u64) -> usize;

    /// Bumped whenever the stored rows are renumbered (an eviction, or the
    /// columns evaluated again): a GPU pick drawn before is dropped.
    fn epoch(&self) -> u64;

    /// Whether row id `row` is one the layer holds (not evicted, not past
    /// the end).
    fn holds(&self, row: usize) -> bool;
    /// Run the layer's handlers for `info.event` on the row `info.row`.
    /// Does nothing when the layer no longer holds the row.
    fn dispatch(&mut self, info: &EventInfo);
    /// The tooltip text for `row`: `None` without a tooltip, for a row the
    /// layer no longer holds, or for empty text.
    fn tooltip_text(&self, row: usize) -> Option<String>;
    /// The generated glue (for tests and diagnostics).
    fn glue_source(&self) -> Glue;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

/// The plot scale a closure-encoded channel reads through, or the error that
/// says why it has none. Not generic, so every `Selection` row type shares one
/// copy (GUP-459).
fn axis_scale_for<'s>(
    mark: &'static str,
    desc: &ChannelDesc,
    x: &'s dyn DynPositionScale,
    y: &'s dyn DynPositionScale,
) -> Result<&'s dyn DynPositionScale> {
    let channel = format!("{}::{}", mark, desc.name.to_ascii_uppercase());
    let (scale, axis, what) = match desc.role {
        Some(Role::X) => (x, "x", "plot x"),
        Some(Role::Y) => (y, "y", "plot y"),
        _ => {
            return Err(Error::config(
                "channel",
                format!(
                    "`{channel}` has no plot scale to encode a closure through; give it                              a scale, e.g. `.{}(Pow::sqrt().range(Px(2.0), Px(12.0)).encode(|r:                              &T| ...))`, or a constant such as `Px(4.0)`",
                    desc.name
                ),
            ));
        }
    };
    if scale.reads_keys() {
        return Err(Error::config(
            what,
            format!(
                "the {axis} scale reads dictionary keys (Band or Point); encode keys                          with `{axis}.encode_key(|r: &T| r.key.as_str())` (`{channel}`)"
            ),
        ));
    }
    Ok(scale)
}

impl<T: Send + Sync + 'static, M: Mark> Selection<T, M> {
    /// The batch's vector slot: the snapshot with the `svg` feature.
    #[cfg(feature = "svg")]
    fn vector_slot(&self, theme: &Theme, entries: &[DrawEntry]) -> Arc<LayerVector> {
        Arc::new(self.vector(theme, entries))
    }

    /// The batch's vector slot: nothing without the `svg` feature.
    #[cfg(not(feature = "svg"))]
    #[expect(clippy::unused_self, reason = "the `svg` build's slot reads the layer")]
    fn vector_slot(&self, _theme: &Theme, _entries: &[DrawEntry]) -> crate::scene::VectorSlot {
        crate::scene::VectorSlot
    }

    /// What a vector target needs to write the layer's marks (GUP-467), as
    /// it is now: see [`LayerVector`].
    #[cfg(feature = "svg")]
    fn vector(&self, theme: &Theme, entries: &[DrawEntry]) -> LayerVector {
        use crate::marks::{Area, Circle, Line, Rect, Segment};
        use std::any::TypeId;
        let id = TypeId::of::<M>();
        let kind = if id == TypeId::of::<Circle>() {
            MarkKind::Circle
        } else if id == TypeId::of::<Rect>() {
            MarkKind::Rect
        } else if id == TypeId::of::<Segment>() {
            MarkKind::Segment
        } else if id == TypeId::of::<Line>() {
            MarkKind::Line(self.curve)
        } else if id == TypeId::of::<Area>() {
            MarkKind::Area
        } else {
            return LayerVector::Skipped(format!(
                "`{}` is a user-defined mark with no CPU description",
                M::NAME
            ));
        };
        let rows = self.len() as u64;
        if !self.retain.keeps_columns(rows) {
            return LayerVector::Skipped(format!(
                "its CPU columns are not retained (Retain::{:?}, {rows} rows)",
                self.retain
            ));
        }
        let Some(cols) = self.columns.as_ref().and_then(ColumnStore::cpu_snapshot) else {
            return LayerVector::Skipped("its CPU columns were released".to_owned());
        };
        let mut column = 0;
        let chans = M::CHANNELS
            .iter()
            .zip(&self.encodings)
            .map(|(desc, e)| {
                let chan = match e {
                    None => VChan::Const(desc.default),
                    Some(Encoding::Const(v)) => VChan::Const(*v),
                    Some(Encoding::Derived(f)) => VChan::Const(f()),
                    Some(Encoding::Column(c)) => {
                        column += 1;
                        VChan::Column {
                            column: column - 1,
                            mirror: c.mirror(),
                        }
                    }
                };
                (desc.name, desc.role, chan)
            })
            .collect::<Vec<_>>();
        // A monotone line's control columns, through y's mirror.
        let mut chans = chans;
        if self.curve.has_controls()
            && let Some(Some(Encoding::Column(c))) = M::CHANNELS
                .iter()
                .position(|d| d.name == "y")
                .map(|s| &self.encodings[s])
        {
            for (k, name) in crate::vector::CONTROLS.iter().enumerate() {
                chans.push((
                    *name,
                    None,
                    VChan::Column {
                        column: column + k,
                        mirror: c.mirror(),
                    },
                ));
            }
        }
        LayerVector::Cpu(Arc::new(CpuLayer {
            kind,
            cols,
            entries: entries.to_vec(),
            chans,
            null_color: theme.null_color,
        }))
    }
}

impl<T: Send + Sync + 'static, M: Mark> Layer for Selection<T, M> {
    fn bind_axes(
        &mut self,
        generation: u64,
        x: &dyn DynPositionScale,
        y: &dyn DynPositionScale,
    ) -> Result<()> {
        if self.axes_bound == generation || self.axes.iter().all(Option::is_none) {
            return Ok(());
        }
        for (slot, f) in self.axes.iter().enumerate() {
            let Some(f) = f else { continue };
            let scale = axis_scale_for(M::NAME, &M::CHANNELS[slot], x, y)?;
            self.encodings[slot] = Some(Encoding::Column(Box::new(AxisEncoding::new(
                Arc::clone(f),
                scale.dyn_encode(),
            ))));
        }
        self.axes_bound = generation;
        self.columns = None;
        self.gpu = None;
        Ok(())
    }

    fn begin_fit(&self) {
        for enc in self.encodings.iter().flatten() {
            if let Encoding::Column(c) = enc {
                for link in c.links() {
                    link.begin_fit();
                }
            }
        }
    }

    fn revision(&self) -> u64 {
        let mut sum = self.appended;
        for enc in self.encodings.iter().flatten() {
            if let Encoding::Column(c) = enc {
                for link in c.links() {
                    sum = sum.wrapping_add(link.revision());
                }
            }
        }
        sum
    }

    fn z(&self) -> i32 {
        self.z
    }

    fn drain_feed(&mut self) -> Result<bool> {
        let rows = match &self.feed {
            Some(feed) if feed.pending() => feed.drain(),
            _ => return Ok(false),
        };
        if rows.is_empty() {
            return Ok(false);
        }
        self.append(rows)?;
        Ok(true)
    }

    fn feed_pending(&self) -> bool {
        self.feed.as_ref().is_some_and(FeedRx::pending)
    }

    fn set_waker(&mut self, waker: Option<&Waker>) {
        self.waker = waker.cloned();
        if let Some(feed) = &self.feed {
            feed.set_waker(waker);
        }
    }

    fn fit_domains(&mut self, cx: &Context) -> Result<()> {
        let store = self.evaluate(cx)?;
        let stats: Vec<_> = (0..store.formats().len()).map(|k| store.stats(k)).collect();
        let mut k = 0;
        for (i, enc) in self.encodings.iter_mut().enumerate() {
            if let Some(Encoding::Column(c)) = enc {
                c.fit(stats[k])
                    .map_err(|e| context(e, M::NAME, M::CHANNELS[i].name))?;
                k += 1;
            }
        }
        Ok(())
    }

    fn y_axis_id(&self) -> Option<usize> {
        M::CHANNELS.iter().zip(&self.encodings).find_map(|(d, e)| {
            let (Some(Role::Y), Some(Encoding::Column(c))) = (d.role, e) else {
                return None;
            };
            scale_identity(c.as_ref())
        })
    }

    fn fixed_positions(&self) -> Vec<(bool, f32)> {
        let px = |v: &crate::ConstValue| match v {
            crate::ConstValue::F32(r) => Some(*r),
            crate::ConstValue::Vec4(_) => None,
        };
        let mut out = Vec::new();
        for (d, e) in M::CHANNELS.iter().zip(&self.encodings) {
            let is_x = match d.role {
                Some(Role::X) => true,
                Some(Role::Y) => false,
                _ => continue,
            };
            let v = match e {
                Some(Encoding::Const(v)) => px(v),
                Some(Encoding::Derived(f)) => px(&f()),
                _ => None,
            };
            out.extend(v.filter(|v| v.is_finite()).map(|v| (is_x, v)));
        }
        out
    }

    /// The largest size: a constant's, or an encoded channel's image of
    /// its column's extent (GUP-418: a `Sqrt` radius reaches its range's
    /// far end).
    fn overhang(&self) -> Overhang {
        let size = |v: &crate::ConstValue| match v {
            crate::ConstValue::F32(r) => *r,
            crate::ConstValue::Vec4(_) => 0.0,
        };
        let mut column = 0;
        let mut overhang = Overhang::default();
        for (d, e) in M::CHANNELS.iter().zip(&self.encodings) {
            // A width or height reaches half of itself past the position,
            // along its own axis; a column-encoded one is not counted.
            let half = if matches!(d.role, Some(Role::Width | Role::Height)) {
                0.5
            } else {
                1.0
            };
            let r = match e {
                Some(Encoding::Column(c)) => {
                    column += 1;
                    if d.role != Some(Role::Size) {
                        continue;
                    }
                    self.columns
                        .as_ref()
                        .and_then(|s| s.stats(column - 1))
                        .and_then(|s| c.image(s.extent()))
                        .map_or(0.0, |(lo, hi)| lo.abs().max(hi.abs()) as f32)
                }
                _ if !matches!(d.role, Some(Role::Size | Role::Width | Role::Height)) => continue,
                Some(Encoding::Const(v)) => size(v) * half,
                Some(Encoding::Derived(f)) => size(&f()) * half,
                None => size(&d.default) * half,
            };
            let slot = match d.role {
                Some(Role::Width) => &mut overhang.x,
                Some(Role::Height) => &mut overhang.y,
                _ => &mut overhang.size,
            };
            *slot = slot.max(r);
        }
        // A step line's riser can sit on a row at the end of the x domain,
        // and its ends are capped: it reaches half its width past the row
        // along x (GUP-550). A straight line still starts at the axis.
        if !matches!(self.curve, Curve::Linear | Curve::MonotoneX) {
            overhang.x = overhang.x.max(self.max_width() * 0.5);
        }
        overhang
    }

    fn cull_margin(&self) -> f32 {
        let mut column = 0;
        let mut margin = 0.0f32;
        for (d, e) in M::CHANNELS.iter().zip(&self.encodings) {
            let is_column = matches!(e, Some(Encoding::Column(_)));
            let position = matches!(d.role, Some(Role::X | Role::Y));
            if is_column {
                column += 1;
            }
            if position || d.wgsl_type != "f32" {
                continue;
            }
            let f32_of = |v: &ConstValue| match v {
                ConstValue::F32(r) => r.abs(),
                ConstValue::Vec4(_) => 0.0,
            };
            let m = match e {
                Some(Encoding::Column(c)) => self
                    .columns
                    .as_ref()
                    .and_then(|s| s.stats(column - 1))
                    .map_or(Some(0.0), |s| {
                        c.image(s.extent())
                            .map(|(lo, hi)| lo.abs().max(hi.abs()) as f32)
                    })
                    .unwrap_or(f32::INFINITY),
                Some(Encoding::Const(v)) => f32_of(v),
                Some(Encoding::Derived(f)) => f32_of(&f()),
                None => f32_of(&d.default),
            };
            margin = margin.max(m);
        }
        margin
    }

    fn visible_entries(&self, plot: Rect) -> Vec<bool> {
        let Some(store) = &self.columns else {
            return Vec::new();
        };
        let stored = store.chunks();
        let px = |v: &ConstValue| match v {
            ConstValue::F32(r) => (f64::from(*r), f64::from(*r)),
            ConstValue::Vec4(_) => (f64::NEG_INFINITY, f64::INFINITY),
        };
        let ignored = |name: &str| self.ignored_end(name);
        let boxes: Vec<Option<PixelBox>> = (0..stored.len())
            .map(|k| {
                let mut b = PixelBox::EMPTY;
                let mut column = 0;
                for (d, e) in M::CHANNELS.iter().zip(&self.encodings) {
                    let axis = match d.role {
                        Some(Role::X | Role::Y) if ignored(d.name) => {
                            if matches!(e, Some(Encoding::Column(_))) {
                                column += 1;
                            }
                            continue;
                        }
                        Some(Role::X) => 0,
                        Some(Role::Y) => 1,
                        _ => {
                            if matches!(e, Some(Encoding::Column(_))) {
                                column += 1;
                            }
                            continue;
                        }
                    };
                    let v = match e {
                        Some(Encoding::Column(c)) => {
                            column += 1;
                            // A column with no finite value draws nothing.
                            let stats = stored[k].columns()[column - 1].stats()?;
                            c.image(stats.extent())
                                .map_or((f64::NEG_INFINITY, f64::INFINITY), |(a, b)| {
                                    (a.min(b), a.max(b))
                                })
                        }
                        Some(Encoding::Const(v)) => px(v),
                        Some(Encoding::Derived(f)) => px(&f()),
                        None => px(&d.default),
                    };
                    b.grow(axis, v);
                }
                Some(b)
            })
            .collect();
        cull::visible_entries(
            &boxes,
            &draw_entries(M::GEOMETRY, stored),
            self.cull_margin() + PICK_TOLERANCE.0 + 1.0,
            plot,
        )
    }

    fn edge_marks(
        &self,
        axes: &[Option<EdgeAxis>; 3],
        budget: &mut usize,
        out: &mut [Vec<(f32, f32)>; 3],
    ) {
        type Col<'c, T> = &'c dyn crate::encoding::DynColumnEncoding<T>;
        let Some(store) = &self.columns else { return };
        // The position columns bound to each asked axis, and the size
        // columns (which reach along every axis), by column index.
        let mut positions: Vec<(usize, usize, Col<T>)> = Vec::new();
        let mut size_cols: Vec<(usize, Col<T>)> = Vec::new();
        let mut size_const = 0.0f32;
        let mut column = 0;
        for (d, e) in M::CHANNELS.iter().zip(&self.encodings) {
            let value = |v: &ConstValue| match v {
                ConstValue::F32(r) => *r,
                ConstValue::Vec4(_) => 0.0,
            };
            match e {
                Some(Encoding::Column(c)) => {
                    column += 1;
                    match d.role {
                        Some(Role::X | Role::Y) if self.ignored_end(d.name) => {}
                        Some(role @ (Role::X | Role::Y)) => {
                            let id = scale_identity(c.as_ref());
                            let axes_of: &[usize] = if role == Role::X { &[0] } else { &[1, 2] };
                            for &a in axes_of {
                                if axes[a].is_some_and(|x| Some(x.identity) == id) {
                                    positions.push((a, column - 1, c.as_ref()));
                                }
                            }
                        }
                        Some(Role::Size) => size_cols.push((column - 1, c.as_ref())),
                        _ => {}
                    }
                }
                _ if d.role == Some(Role::Size) => {
                    let v = match e {
                        Some(Encoding::Const(v)) => value(v),
                        Some(Encoding::Derived(f)) => value(&f()),
                        _ => value(&d.default),
                    };
                    size_const = size_const.max(v);
                }
                _ => {}
            }
        }
        if positions.is_empty() {
            return;
        }
        let o = self.overhang();
        // Per axis: what a mark reaches regardless of its row.
        let fixed = [
            size_const.max(o.x),
            size_const.max(o.y),
            size_const.max(o.y),
        ];
        let exact_ok = self.retain.keeps_columns(store.rows());
        for (k, chunk) in store.chunks().iter().enumerate() {
            let rows = chunk.rows() as usize;
            // The chunk's statistics bound its marks; a chunk they say
            // cannot reach an edge is not decoded.
            let mut stat: [Vec<(f32, f32)>; 3] = Default::default();
            {
                // The chunk's statistics bound its marks.
                let stat_reach = |a: usize| -> f32 {
                    let mut reach = fixed[a];
                    for &(col, c) in &size_cols {
                        let image = chunk.columns()[col]
                            .stats()
                            .map_or(Some((0.0, 0.0)), |s| c.image(s.extent()));
                        reach = reach.max(
                            image.map_or(f32::INFINITY, |(lo, hi)| lo.abs().max(hi.abs()) as f32),
                        );
                    }
                    reach
                };
                for &(a, col, c) in &positions {
                    let Some(axis) = axes[a].as_ref() else {
                        continue;
                    };
                    let Some(stats) = chunk.columns()[col].stats() else {
                        continue;
                    };
                    let h = stat_reach(a);
                    let span = c
                        .image(stats.extent())
                        .and_then(|(p, q)| Some((axis.t(p)?, axis.t(q)?)))
                        .map(|(p, q)| (p.min(q), p.max(q)));
                    let (lo, hi) = match span {
                        Some((lo, hi)) if hi < 0.0 || lo > 1.0 => continue,
                        Some((lo, hi)) => (lo.clamp(0.0, 1.0), hi.clamp(0.0, 1.0)),
                        None => (0.0, 1.0),
                    };
                    axis.push(&mut stat[a], lo, h);
                    axis.push(&mut stat[a], hi, h);
                }
            }
            if stat.iter().all(Vec::is_empty) {
                continue;
            }
            let mut tmp: [Vec<(f32, f32)>; 3] = Default::default();
            let mut done = false;
            if exact_ok && rows <= *budget {
                // Only the axes the statistics say may bind are decoded.
                let live: Vec<_> = positions.iter().filter(|p| !stat[p.0].is_empty()).collect();
                done = (|| -> Option<()> {
                    let mut decoded: Vec<Option<Vec<f64>>> = vec![None; store.formats().len()];
                    for &&(_, col, _) in &live {
                        decoded[col] = Some(store.chunk_values(k, col)?);
                    }
                    for &(col, _) in &size_cols {
                        decoded[col] = Some(store.chunk_values(k, col)?);
                    }
                    if size_cols.is_empty() {
                        // Every row reaches `fixed`, so only the two rows
                        // nearest the ends can bind.
                        for &&(a, col, c) in &live {
                            let axis = axes[a].as_ref()?;
                            let stats = chunk.columns()[col].stats()?;
                            let values = decoded[col].as_ref()?;
                            let (lo, hi) = in_domain_extremes(values, axis, c, stats.extent())?;
                            for t in [lo, hi].into_iter().flatten() {
                                axis.push(&mut tmp[a], t, fixed[a]);
                            }
                        }
                        return Some(());
                    }
                    for r in 0..rows {
                        let mut reach = 0.0f64;
                        for &(col, c) in &size_cols {
                            let v = decoded[col].as_ref()?[r];
                            if !v.is_nan() {
                                reach = reach.max(c.cpu_eval(v)?.abs());
                            }
                        }
                        for &&(a, col, c) in &live {
                            let v = decoded[col].as_ref()?[r];
                            if v.is_nan() {
                                continue;
                            }
                            let axis = axes[a].as_ref()?;
                            let t = axis.t(c.cpu_eval(v)?)?;
                            axis.push(&mut tmp[a], t, reach.max(f64::from(fixed[a])) as f32);
                        }
                    }
                    Some(())
                })()
                .is_some();
                if done {
                    *budget -= rows;
                } else {
                    tmp = Default::default();
                }
            }
            let found = if done { tmp } else { stat };
            for (o, t) in out.iter_mut().zip(found) {
                o.extend(t);
            }
        }
    }

    fn legends(&self) -> Vec<(Option<String>, Legend)> {
        let mut out = Vec::new();
        for (i, enc) in self.encodings.iter().enumerate() {
            if M::CHANNELS[i].role != Some(Role::Color) {
                continue;
            }
            if let Some(Encoding::Column(c)) = enc
                && let Some(legend) = c.legend()
            {
                out.push((self.legend_titles[i].clone(), legend));
            }
        }
        out
    }

    fn apply_theme(&mut self, theme: &Theme) {
        for enc in self.encodings.iter_mut().flatten() {
            if let Encoding::Column(c) = enc {
                c.apply_theme(theme);
            }
        }
    }

    fn prepare(&mut self, cx: &Context, theme: &Theme, brushable: bool) -> Result<MarkBatch> {
        self.check_module_paths()?;
        self.brushable = brushable;
        self.unbrushed = theme.unbrushed;
        let glue = self.glue();
        let program = cx.program(&glue)?;

        // The `Encodings` uniform: each channel's Params or constant at the
        // offset the glue emitter laid out (encase sizes, 16-byte fields).
        let mut encodings = vec![0u8; program.encodings.span as usize];
        let mut luts: Vec<Vec<[u8; 4]>> = Vec::new();
        for (i, desc) in M::CHANNELS.iter().enumerate() {
            match &self.encodings[i] {
                // One field per link of the chain.
                Some(Encoding::Column(c)) => {
                    for (k, func) in c.links().into_iter().enumerate() {
                        let field = glue::link_field(desc.name, k);
                        let bytes = func.params_bytes()?;
                        write_field(&mut encodings, &program.encodings, &field, &bytes)?;
                    }
                }
                Some(Encoding::Const(v)) => {
                    write_field(&mut encodings, &program.encodings, desc.name, &v.bytes())?;
                }
                Some(Encoding::Derived(f)) => {
                    write_field(&mut encodings, &program.encodings, desc.name, &f().bytes())?;
                }
                None => {
                    let bytes = desc.default.bytes();
                    write_field(&mut encodings, &program.encodings, desc.name, &bytes)?;
                }
            }
        }
        // The theme's null colour, when the glue reads one.
        if program.encodings.offset(glue::NULL_COLOR_FIELD).is_some() {
            let bytes = bytemuck::bytes_of(&theme.null_color);
            write_field(
                &mut encodings,
                &program.encodings,
                glue::NULL_COLOR_FIELD,
                bytes,
            )?;
        }
        if let (Some(_), Unbrushed::Grey(grey)) = (
            program.encodings.offset(glue::BRUSH_GREY_FIELD),
            self.unbrushed,
        ) {
            write_field(
                &mut encodings,
                &program.encodings,
                glue::BRUSH_GREY_FIELD,
                bytemuck::bytes_of(&grey),
            )?;
        }
        if program.encodings.offset(glue::ROW_COUNT_FIELD).is_some() {
            let rows = self.columns.as_ref().map_or(0, ColumnStore::rows) as u32;
            write_field(
                &mut encodings,
                &program.encodings,
                glue::ROW_COUNT_FIELD,
                bytemuck::bytes_of(&rows),
            )?;
        }
        if program.encodings.offset(glue::BRUSH_DIM_FIELD).is_some() {
            write_field(
                &mut encodings,
                &program.encodings,
                glue::BRUSH_DIM_FIELD,
                bytemuck::bytes_of(&self.brush_dim()),
            )?;
        }
        // The LUTs, in binding order.
        for lut in &glue.luts {
            let Some(Encoding::Column(c)) = &self.encodings[lut.channel] else {
                unreachable!("LUTs belong to column channels")
            };
            for r in c.links()[lut.link].resources() {
                let Resource::Lut(data) = r;
                luts.push(data);
            }
        }

        let chunks = self.chunk_uniform_bytes(&program.chunk, &glue.relative)?;

        let limits = &cx.caps().limits;
        // A polyline reads two rows' validity bits (RFC-001 "GUP-432
        // findings").
        let storage = if glue.polyline { 2 } else { 1 };
        if glue.validity && limits.max_storage_buffers_per_shader_stage < storage {
            return Err(Error::config(
                "layer",
                format!(
                    "{}: its columns have nulls, whose validity bits are read from {storage} \
                     storage buffer(s) in the vertex stage, and this device allows {}",
                    M::NAME,
                    limits.max_storage_buffers_per_shader_stage
                ),
            ));
        }
        // Every column (and a polyline's next-row column) is a vertex
        // buffer (RFC-001 §12 risk 7).
        let slots = glue.columns.len() + glue.successors.len();
        if slots > limits.max_vertex_buffers as usize {
            let controls = if glue.controls { 2 } else { 0 };
            return Err(Error::config(
                "layer",
                format!(
                    "{}: {slots} vertex columns ({} channels read from columns{}{}), and this \
                     device has {} vertex buffer slots; make some channels constants",
                    M::NAME,
                    glue.columns.len() - controls,
                    if glue.successors.is_empty() {
                        String::new()
                    } else {
                        format!(", {} read again at the next row", glue.successors.len())
                    },
                    if glue.controls {
                        ", 2 control columns of the monotone curve"
                    } else {
                        ""
                    },
                    limits.max_vertex_buffers
                ),
            ));
        }
        let store = self.columns.as_mut().expect("checked above");
        store.upload(cx)?;
        let store = self.columns.as_ref().expect("checked above");
        // Validity buffers exist (and are bound) only when the glue reads
        // them, which is exactly when the store has a null.
        let reads_validity = glue.validity;
        let validity = |c| chunk_validity(reads_validity, c, cx);
        let stored = store.chunks();
        let brush_words = glue
            .brush_binding
            .map(|_| crate::render::brush_words(store.rows()));
        let entries = draw_entries(M::GEOMETRY, stored);
        let vector = self.vector_slot(theme, &entries);
        let uploaded = || {
            entries.iter().map(|d| {
                let c = &stored[d.this];
                (
                    c.buffer(cx).expect("uploaded above"),
                    validity(c),
                    d.instances,
                )
            })
        };
        // A polyline in data order, bridges between its chunks; else
        // largest first when the size is a column, or each chunk in
        // upload order.
        let spans = match &self.order {
            _ if glue.polyline => polyline_spans(stored),
            Some(order) => order.spans(stored),
            None => chunk_spans(stored),
        };
        let chunk_draws = || -> Vec<ChunkDraw> {
            entries
                .iter()
                .enumerate()
                .map(|(e, d)| chunk_draw(cx, store, &glue, d, e, reads_validity))
                .collect()
        };

        if let Some((gpu, held)) = &self.gpu
            && gpu.reusable(cx, &program)
            && *held == luts
        {
            // Zoom, pan and resize land here every frame: write the new
            // uniform values into the existing buffers.
            if gpu.draws_chunks(uploaded(), &spans) {
                gpu.write_uniforms(cx, &encodings, &chunks);
                return Ok(MarkBatch::new(Arc::clone(gpu), M::ANTIALIAS, vector));
            }
            // Appended rows: new chunk draws and `Chunk` entries over the
            // same encodings, which take their new values in place.
            let draws = Draws {
                chunks: chunk_draws(),
                spans,
            };
            #[cfg_attr(
                target_arch = "wasm32",
                expect(
                    clippy::arc_with_non_send_sync,
                    reason = "`Arc`, not `Rc`, because `Layer` must be `Send + Sync` on \
                              native; on wasm32 wgpu's handles are neither"
                )
            )]
            let grown = Arc::new(gpu.with_chunks(cx, &chunks, draws, brush_words)?);
            grown.write_encodings(cx, &encodings);
            if grown.brush_words() != gpu.brush_words() || self.bits_stale {
                self.write_bits(cx, &grown);
            }
            self.bits_stale = false;
            self.gpu = Some((Arc::clone(&grown), luts));
            return Ok(MarkBatch::new(grown, M::ANTIALIAS, vector));
        }
        #[cfg_attr(
            target_arch = "wasm32",
            expect(
                clippy::arc_with_non_send_sync,
                reason = "`Arc`, not `Rc`, because `Layer` must be `Send + Sync` on native; \
                          on wasm32 wgpu's handles are not `Send`/`Sync`, so the `Arc` is \
                          neither and cannot cross a thread"
            )
        )]
        let gpu = Arc::new(
            LayerUniforms {
                program,
                encodings,
                chunks,
                luts: luts.iter().map(Vec::as_slice).collect(),
                brush_words,
            }
            .build(
                cx,
                Draws {
                    chunks: chunk_draws(),
                    spans,
                },
                self.vertices,
            )?,
        );
        self.write_bits(cx, &gpu);
        self.bits_stale = false;
        self.gpu = Some((Arc::clone(&gpu), luts));
        Ok(MarkBatch::new(gpu, M::ANTIALIAS, vector))
    }

    fn write_brush(&mut self, cx: &Context, rows: Option<&[usize]>) -> Result<()> {
        let had = self.brush.is_some();
        self.brush = rows.map(|r| {
            let mut r = r.to_vec();
            r.sort_unstable();
            r.dedup();
            r
        });
        if !self.brushable {
            return Ok(());
        }
        let Some((gpu, _)) = &self.gpu else {
            return Ok(());
        };
        let gpu = Arc::clone(gpu);
        self.write_bits(cx, &gpu);
        if had != self.brush.is_some() {
            gpu.write_brush_dim(cx, self.brush_dim());
        }
        Ok(())
    }

    fn release(&mut self) {
        let rows = self.len() as u64;
        let Some(store) = &mut self.columns else {
            return;
        };
        if !self.retain.keeps_columns(rows) {
            store.release();
        }
        if !self.retain.keeps_rows(rows) {
            // Drops every `T`: the store holds what the GPU needs.
            self.released += self.rows.len();
            self.rows = Vec::new();
        }
    }

    fn hit_items(&mut self, layer: usize, visible: &[bool], out: &mut Vec<PickItem>) -> Result<()> {
        if M::HIT == HitShape::Unpickable {
            return Ok(());
        }
        let Some(store) = &self.columns else {
            return Ok(());
        };
        let n = store.rows() as usize;
        if n == 0 {
            return Ok(());
        }
        let wanted: &[&str] = match M::HIT {
            HitShape::Unpickable => &[],
            HitShape::Disc => &["x", "y", "radius"],
            HitShape::AlignedBox => &["x", "y", "width", "height", "x_end", "y_end"],
            HitShape::Segment => &["x", "y", "x_end", "y_end", "width"],
            HitShape::Polyline => &["x", "y", "width"],
            HitShape::Area => &["x", "y", "y_end"],
        };
        // The stored rows of the visible draw entries (RFC-001 S9f). A
        // polyline's ranges are read in pairs of consecutive rows.
        let polyline = M::GEOMETRY == Geometry::Polyline;
        let chunk_rows: Vec<(usize, usize)> = store
            .chunks()
            .iter()
            .map(|c| (c.row_base() as usize, c.rows() as usize))
            .collect();
        let ranges = if visible.is_empty() {
            std::iter::once(0..n).collect()
        } else {
            cull::pick_ranges(polyline, &chunk_rows, visible)
        };
        let mut chunk_needed = vec![visible.is_empty(); chunk_rows.len()];
        for r in &ranges {
            let first = chunk_rows.partition_point(|&(b, rows)| b + rows <= r.start);
            for (k, &(b, _)) in chunk_rows.iter().enumerate().skip(first) {
                if b >= r.end {
                    break;
                }
                chunk_needed[k] = true;
            }
        }
        let slot_of = |name: &str| M::CHANNELS.iter().position(|d| d.name == name);
        // Column index of each column-encoded slot.
        let mut col_of = vec![None; M::CHANNELS.len()];
        let mut k = 0;
        for (slot, e) in self.encodings.iter().enumerate() {
            if matches!(e, Some(Encoding::Column(_))) {
                col_of[slot] = Some(k);
                k += 1;
            }
        }
        let needed: Vec<(usize, usize)> = wanted
            .iter()
            .filter_map(|w| slot_of(w))
            .filter_map(|slot| col_of[slot].map(|k| (slot, k)))
            .collect();

        // Values per needed channel, by data row (RFC-001 S8b): the CPU
        // columns if every chunk kept them, else the accessors over the
        // rows, else nothing to pick.
        let mut raw: Vec<Vec<f64>> = Vec::with_capacity(needed.len());
        let from_store: Option<Vec<Vec<f64>>> = needed
            .iter()
            .map(|&(_, k)| store.cpu_values_where(k, |c| chunk_needed[c]))
            .collect();
        // A monotone line's control values by data row (a polyline's
        // stored rows are its data rows): the stored columns, or computed
        // from the rows' x and y below.
        let mut controls: Option<[Vec<f64>; 2]> = None;
        if self.curve.has_controls() && from_store.is_some() {
            let k = store.formats().len() - 2;
            let column = |k| store.cpu_values_where(k, |c| chunk_needed[c]);
            if let (Some(c1), Some(c2)) = (column(k), column(k + 1)) {
                controls = Some([c1, c2]);
            }
        }
        if let Some(stored) = from_store {
            for v in stored {
                raw.push(match &self.order {
                    Some(order) => {
                        let mut data = vec![f64::NAN; n];
                        for r in &ranges {
                            for s in r.clone() {
                                data[order.row_of(s as u64) as usize] = v[s];
                            }
                        }
                        data
                    }
                    None => v,
                });
            }
        } else if self.all_rows_held() && self.rows.len() == n {
            let rows = &self.rows;
            for &(slot, _) in &needed {
                let Some(Encoding::Column(c)) = &mut self.encodings[slot] else {
                    unreachable!("a column slot")
                };
                raw.push(match c.evaluate(rows)? {
                    crate::column::ColumnData::Values(v) => v,
                    crate::column::ColumnData::Codes(codes) => codes
                        .into_iter()
                        .map(|c| {
                            if c == crate::column::NULL_CODE {
                                f64::NAN
                            } else {
                                f64::from(c)
                            }
                        })
                        .collect(),
                });
            }
            if self.curve.has_controls() {
                let at = |name| {
                    let slot = slot_of(name).expect("a line has x and y");
                    needed.iter().position(|&(s, _)| s == slot)
                };
                if let (Some(x), Some(y)) = (at("x"), at("y")) {
                    let (c1, c2) = crate::monotone::controls(&raw[x], &raw[y]);
                    controls = Some([c1, c2]);
                }
            }
        } else {
            return Ok(());
        }

        let as_px = |v: ConstValue| match v {
            ConstValue::F32(v) => f64::from(v),
            ConstValue::Vec4(_) => f64::NAN,
        };
        let chan = |name: &str| -> Chan {
            if !wanted.contains(&name) {
                return Chan::Unused;
            }
            let Some(slot) = slot_of(name) else {
                return Chan::Unused;
            };
            match &self.encodings[slot] {
                None => Chan::Const(as_px(M::CHANNELS[slot].default)),
                Some(Encoding::Const(c)) => Chan::Const(as_px(*c)),
                Some(Encoding::Derived(f)) => Chan::Const(as_px(f())),
                Some(Encoding::Column(c)) => {
                    let at = needed
                        .iter()
                        .position(|&(s, _)| s == slot)
                        .expect("a needed column");
                    let mut col = vec![f64::NAN; n];
                    for r in &ranges {
                        for s in r.clone() {
                            let d = match &self.order {
                                Some(order) => order.row_of(s as u64) as usize,
                                None => s,
                            };
                            let v = raw[at][d];
                            if !v.is_nan() {
                                col[d] = c.cpu_eval(v).unwrap_or(f64::NAN);
                            }
                        }
                    }
                    Chan::Col(col)
                }
            }
        };
        // The control values in pixels, through y's encoding.
        let ctrl = match (controls, slot_of("y").map(|s| &self.encodings[s])) {
            (Some(values), Some(Some(Encoding::Column(c)))) => values.map(|v| {
                Chan::Col(
                    v.into_iter()
                        .map(|v| {
                            if v.is_nan() {
                                f64::NAN
                            } else {
                                c.cpu_eval(v).unwrap_or(f64::NAN)
                            }
                        })
                        .collect(),
                )
            }),
            _ => [Chan::Unused, Chan::Unused],
        };
        let chans = Chans {
            x: chan("x"),
            y: chan("y"),
            radius: chan("radius"),
            width: chan("width"),
            height: chan("height"),
            x_end: chan("x_end"),
            y_end: chan("y_end"),
            curve: self.curve,
            ctrl,
        };

        // Stored rows with their data row (from the first kept row; ids
        // count from `first`) and rank in the draw order.
        let first = self.evicted as usize;
        let rank = self.order.as_ref().map(|order| {
            let mut rank = vec![0u64; n];
            let mut next = 0;
            for seg in order.segments() {
                for s in seg.clone() {
                    rank[s as usize] = next;
                    next += 1;
                }
            }
            rank
        });
        for r in ranges {
            let slots: Vec<(usize, u64)> = match (&self.order, &rank) {
                (Some(order), Some(rank)) => r
                    .map(|s| (order.row_of(s as u64) as usize, rank[s]))
                    .collect(),
                _ => r.map(|s| (s, s as u64)).collect(),
            };
            build_items(M::HIT, layer, self.z, first, &slots, &chans, out);
        }
        Ok(())
    }

    #[cfg(test)]
    fn draw_order(&self) -> Option<&DrawOrder> {
        self.order.as_ref()
    }

    fn row_id(&self, stored: u64) -> usize {
        let data = self.order.as_ref().map_or(stored, |o| o.row_of(stored));
        (self.evicted + data) as usize
    }

    fn epoch(&self) -> u64 {
        self.epoch
    }

    fn holds(&self, row: usize) -> bool {
        (self.evicted as usize..self.released + self.rows.len()).contains(&row)
    }

    fn gpu_pickable(&self) -> bool {
        M::GPU_PICK
    }

    fn cpu_readable(&self) -> bool {
        self.columns
            .as_ref()
            .is_none_or(|store| store.cpu_kept() || !self.rows.is_empty())
    }

    fn row_any(&self, row: usize) -> Option<&dyn Any> {
        let row = row.checked_sub(self.released)?;
        self.rows.get(row).map(|r| r as &dyn Any)
    }

    fn dispatch(&mut self, info: &EventInfo) {
        let Some(i) = info.row.row.checked_sub(self.released) else {
            return;
        };
        let Some(row) = self.rows.get(i) else { return };
        for (event, handler) in &mut self.handlers {
            if *event == info.event {
                handler(info, row);
            }
        }
    }

    fn tooltip_text(&self, row: usize) -> Option<String> {
        let f = self.tooltip.as_ref()?;
        let row = self.rows.get(row.checked_sub(self.released)?)?;
        Some(f(row)).filter(|s| !s.is_empty())
    }

    fn glue_source(&self) -> Glue {
        self.glue()
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// The draw of entry `d` (at index `e` among the layer's entries): its
/// columns bound from row `d.first` of its chunk, then (a polyline) its
/// next-row columns from row `d.next_first` of chunk `d.next`.
fn chunk_draw(
    cx: &Context,
    store: &ColumnStore,
    glue: &Glue,
    d: &DrawEntry,
    e: usize,
    reads_validity: bool,
) -> ChunkDraw {
    let (this, next) = (&store.chunks()[d.this], &store.chunks()[d.next]);
    let buffer = |c: &crate::column::Chunk| c.buffer(cx).expect("uploaded above").clone();
    let slot = |c: &crate::column::Chunk, i: usize, first: u32| {
        let range = c.column_range(i);
        let start = range.start + u64::from(first) * store.formats()[i].stride();
        (buffer(c), start..range.end)
    };
    let mut slots: Vec<_> = (0..store.formats().len())
        .map(|i| slot(this, i, d.first))
        .collect();
    slots.extend(glue.successors.iter().map(|&i| slot(next, i, d.next_first)));
    ChunkDraw {
        columns: buffer(this),
        slots,
        instances: d.instances,
        dynamic_offset: chunk_uniform_offset(e) as u32,
        validity: chunk_validity(reads_validity, this, cx).cloned(),
        next_validity: (glue.polyline && reads_validity)
            .then(|| chunk_validity(true, next, cx).cloned())
            .flatten(),
    }
}

/// A chunk's uploaded validity buffer, if the program reads one. (A
/// function, not a closure: a closure cannot return a borrow of its
/// argument, RFC-001 §12 risk 4.)
fn chunk_validity<'c>(
    reads: bool,
    chunk: &'c crate::column::Chunk,
    cx: &Context,
) -> Option<&'c wgpu::Buffer> {
    reads.then(|| chunk.validity_buffer(cx).expect("uploaded above"))
}

/// The smallest and largest normalized position among the rows of
/// `values` that lie in the domain (`0..=1`), where `c` maps a value to
/// pixels and `extent` is the values' (min, max). Every library scale is
/// monotonic, so a row between two rows already seen in the domain cannot
/// be an extreme, and one beyond a row seen outside cannot be inside: only
/// a few rows are evaluated. `None` if `c` has no CPU mirror.
fn in_domain_extremes<T>(
    values: &[f64],
    axis: &EdgeAxis,
    c: &dyn crate::encoding::DynColumnEncoding<T>,
    extent: (f64, f64),
) -> Option<(Option<f32>, Option<f32>)> {
    let t_of = |v: f64| axis.t(c.cpu_eval(v)?);
    let (t0, t1) = (t_of(extent.0)?, t_of(extent.1)?);
    if t0 == t1 {
        // A constant column, or a degenerate range: one row decides.
        let t = (0.0..=1.0).contains(&t0).then_some(t0);
        return Some((t, t));
    }
    // `key` grows with the normalized position.
    let sign = if t1 > t0 { 1.0 } else { -1.0 };
    let (mut out_below, mut out_above) = (f64::NEG_INFINITY, f64::INFINITY);
    let (mut in_lo, mut in_hi) = ((f64::INFINITY, 0.0f32), (f64::NEG_INFINITY, 0.0f32));
    for &v in values {
        let k = v * sign;
        if v.is_nan() || k <= out_below || k >= out_above || (k >= in_lo.0 && k <= in_hi.0) {
            continue;
        }
        let t = t_of(v)?;
        if t < 0.0 {
            out_below = out_below.max(k);
        } else if t > 1.0 {
            out_above = out_above.min(k);
        } else {
            if k < in_lo.0 {
                in_lo = (k, t);
            }
            if k > in_hi.0 {
                in_hi = (k, t);
            }
        }
    }
    let found = in_lo.0 <= in_hi.0;
    Some(if found {
        (Some(in_lo.1), Some(in_hi.1))
    } else {
        (None, None)
    })
}

/// The identity ([`DynPositionScale::identity`]) of the scale a column
/// encoding's first link is, if it is one.
fn scale_identity<T>(c: &dyn crate::encoding::DynColumnEncoding<T>) -> Option<usize> {
    c.links()
        .first()?
        .dictionary_slot()
        .map(|slot| std::ptr::from_ref(slot) as usize)
}

fn context(e: Error, mark: &str, channel: &str) -> Error {
    match e {
        Error::Configuration { what, detail } => Error::Configuration {
            what,
            detail: format!("{mark}::{} channel: {detail}", channel.to_ascii_uppercase()),
        },
        other => other,
    }
}

/// Copy `bytes` into member `name` of a uniform struct, checking it fits
/// before the next member (or the end of the struct).
fn write_field(
    dst: &mut [u8],
    layout: &crate::shader::StructLayout,
    name: &str,
    bytes: &[u8],
) -> Result<()> {
    let offset = layout.offset(name).ok_or_else(|| {
        Error::config(
            "generated uniform layout",
            format!("member `{name}` missing from {:?}", layout.members),
        )
    })? as usize;
    let end = layout
        .members
        .iter()
        .map(|&(_, o)| o as usize)
        .filter(|&o| o > offset)
        .min()
        .unwrap_or(dst.len());
    if offset + bytes.len() > end {
        return Err(Error::config(
            "generated uniform layout",
            format!(
                "`{name}` needs {} bytes but has {} (encase size vs generated layout mismatch)",
                bytes.len(),
                end - offset
            ),
        ));
    }
    dst[offset..offset + bytes.len()].copy_from_slice(bytes);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::channel::Px;
    use crate::column::ColumnFormat;
    use crate::encoding::EncodeFn;
    use crate::marks::Circle;
    use crate::scale::{Categorical, Linear, Log, Pow, ScaleRef, Sequential, Symlog, Time};
    use crate::shader::link;
    use crate::shader::testing::{parse, struct_layout};
    use std::path::Path;

    pub(crate) struct Row {
        x: f64,
        y: f64,
        v: f64,
    }

    /// The reference signature of RFC-001 §6: linear x, log y, sequential
    /// fill, constant radius.
    pub(crate) fn reference() -> Selection<Row, Circle> {
        let rows = (1..=20)
            .map(|i| {
                let i = f64::from(i);
                Row {
                    x: 1.7e9 + i * 60.0,
                    y: i * i,
                    v: i,
                }
            })
            .collect::<Vec<_>>();
        let mut sel = Selection::<Row, Circle>::new(rows);
        sel.attr(
            Circle::X,
            ScaleRef::new(Linear::new()).encode(|r: &Row| r.x),
        )
        .attr(Circle::Y, Log::new().encode(|r: &Row| r.y))
        .attr(Circle::FILL, Sequential::viridis().encode(|r: &Row| r.v))
        .attr(Circle::RADIUS, Px(4.0));
        sel
    }

    /// GUP-482 AC4: 10,000,000 rows in batches of 10,000 through
    /// `LastRows(100_000)`. The chunk count stays within
    /// `ceil(1.25 × 100_000 / chunk_rows) + 2` and no `row_base` written to
    /// the GPU reaches the rows held, however many rows went before.
    #[test]
    fn a_long_stream_stays_bounded() {
        const N: usize = 100_000;
        const BATCH: usize = 10_000;
        let cx = Context::new_blocking().unwrap();
        let start = std::time::Instant::now();
        let mut sel = Selection::<Row, Circle>::new(Vec::new());
        sel.attr(
            Circle::X,
            ScaleRef::new(Linear::new()).encode(|r: &Row| r.x),
        )
        .attr(
            Circle::Y,
            ScaleRef::new(Linear::new()).encode(|r: &Row| r.y),
        )
        .attr(Circle::RADIUS, Px(1.0))
        .window(Window::LastRows(N));
        sel.fit_domains(&cx).unwrap();
        let theme = Theme::default();
        let chunk_rows = sel.columns.as_ref().unwrap().chunk_rows();
        assert_eq!(chunk_rows, 25_000, "max(1024, n / 4)");
        let max_chunks = (1.25 * N as f64 / f64::from(chunk_rows)).ceil() as usize + 2;
        let (mut most, mut bytes, mut uniforms_bytes) = (0, 0, 0);
        let mut steady = std::time::Duration::ZERO;
        for b in 0..1_000 {
            let rows = (b * BATCH..(b + 1) * BATCH).map(|i| Row {
                x: i as f64,
                y: (i % 997) as f64,
                v: 0.0,
            });
            let rows: Vec<Row> = rows.collect();
            let before = cx.upload_stats();
            let t = std::time::Instant::now();
            sel.append(rows).unwrap();
            sel.fit_domains(&cx).unwrap();
            sel.prepare(&cx, &theme, false).unwrap();
            sel.release();
            if b >= 100 {
                steady += t.elapsed();
            }
            let written = cx.upload_stats() - before;
            bytes += written.columns.bytes;
            uniforms_bytes += written.uniforms.bytes;
            let store = sel.columns.as_ref().unwrap();
            let held = store.rows();
            assert_eq!(held as usize, sel.len());
            assert!(held >= N.min((b + 1) * BATCH) as u64, "batch {b}: {held}");
            most = most.max(store.chunks().len());
            assert!(store.chunks().len() <= max_chunks, "batch {b}");
            // The row bases the prepare wrote.
            let glue = sel.glue();
            let program = cx.program(&glue).unwrap();
            let uniforms = sel
                .chunk_uniform_bytes(&program.chunk, &glue.relative)
                .unwrap();
            for k in 0..store.chunks().len() {
                let at = chunk_uniform_offset(k);
                let row_base = u32::from_le_bytes(uniforms[at..at + 4].try_into().unwrap());
                assert!(
                    u64::from(row_base) < held,
                    "batch {b}: {row_base} of {held}"
                );
            }
        }
        assert_eq!(sel.evicted as usize + sel.len(), 10_000_000);
        assert_eq!(bytes, 10_000_000 * 8, "every row uploaded once");
        eprintln!(
            "10M rows through LastRows(100k): at most {most} chunks, {:?}; \
             per 10k-row batch after the first 100: {:?} append + evict + prepare, \
             {} B uniforms on average",
            start.elapsed(),
            steady / 900,
            uniforms_bytes / 1_000
        );
    }

    /// AC2: one `Chunk` entry per chunk, 256 bytes apart, holding the
    /// chunk's first row and the x base from the chunk's own origin; one
    /// draw per chunk at that entry's dynamic offset.
    #[test]
    fn chunk_uniforms_hold_each_chunks_row_base_and_base() {
        let cx = Context::new_blocking().unwrap();
        let mut sel = reference();
        sel.max_chunk_rows(8);
        sel.fit_domains(&cx).unwrap();
        let glue = sel.glue();
        let program = cx.program(&glue).unwrap();
        let bytes = sel
            .chunk_uniform_bytes(&program.chunk, &glue.relative)
            .unwrap();
        let store = sel.columns.as_ref().unwrap();
        assert_eq!(
            store.chunks().iter().map(|c| c.rows()).collect::<Vec<_>>(),
            [8, 8, 4]
        );
        assert_eq!(bytes.len(), 2 * 256 + program.chunk.span as usize);
        let Some(Encoding::Column(x)) = &sel.encodings[0] else {
            panic!()
        };
        let x_base = program.chunk.offset("x_base").unwrap() as usize;
        let mut bases = Vec::new();
        for (k, chunk) in store.chunks().iter().enumerate() {
            let entry = &bytes[k * 256..];
            let row_base = u32::from_le_bytes(entry[..4].try_into().unwrap());
            assert_eq!(u64::from(row_base), chunk.row_base());
            assert_eq!(row_base, 8 * k as u32);
            let origin = chunk.columns()[0].origin();
            assert_eq!(origin, 1.7e9 + f64::from(8 * k as u32 + 1) * 60.0);
            let base = f32::from_le_bytes(entry[x_base..x_base + 4].try_into().unwrap());
            assert_eq!(base, x.links()[0].chunk_base(origin) as f32);
            bases.push(base);
        }
        assert!(bases[0] < bases[1] && bases[1] < bases[2], "{bases:?}");

        let batch = sel.prepare(&cx, &Theme::default(), false).unwrap();
        let offsets: Vec<_> = batch.gpu.chunks.iter().map(|c| c.dynamic_offset).collect();
        assert_eq!(offsets, [0, 256, 512]);
        assert_eq!(batch.instances(), 20);
    }

    /// GUP-432: a line in chunks of 8, 8 and 4 rows draws each chunk's own
    /// segments and one bridge per boundary, in data order. A bridge's
    /// `Chunk` entry starts at its chunk's last row, takes x from its own
    /// chunk's origin and the next x from the next chunk's, and reads
    /// validity from the last row of one chunk and the first of the next.
    /// The glue links and its `Chunk` layout matches naga's.
    #[test]
    fn line_bridges_chunks_with_each_chunks_origin() {
        use crate::Line;
        use crate::render::Span;
        use crate::shader::link;
        use crate::shader::testing::{parse, struct_layout};
        let cx = Context::new_blocking().unwrap();
        let rows: Vec<(f64, f64)> = (0..20)
            .map(|i| {
                (
                    1.7e9 + f64::from(i) * 60.0,
                    if i == 13 { f64::NAN } else { f64::from(i) },
                )
            })
            .collect();
        let mut sel = Selection::<(f64, f64), Line>::new(rows);
        sel.attr(
            Line::X,
            ScaleRef::new(Linear::new()).encode(|r: &(f64, f64)| r.0),
        )
        .attr(
            Line::Y,
            ScaleRef::new(Linear::new()).encode(|r: &(f64, f64)| r.1),
        )
        .max_chunk_rows(8);
        sel.fit_domains(&cx).unwrap();
        let glue = sel.glue();
        assert!(glue.polyline && glue.validity);
        assert_eq!(glue.successors, [0, 1]);
        let members: Vec<_> = glue
            .relative
            .iter()
            .map(|b| (b.member.as_str(), b.next))
            .collect();
        assert_eq!(
            members,
            [
                ("x_base", false),
                ("y_base", false),
                ("x_next_base", true),
                ("y_next_base", true)
            ]
        );
        assert!(
            glue.source
                .contains("m.x_next = linear::map_rel(col.x_next, chunk.x_next_base, enc.x);")
        );
        let linked = parse(
            &glue.signature,
            &link(&glue.signature, &glue.source, &glue.modules).unwrap(),
        );
        assert_eq!(Some(glue.chunk.clone()), struct_layout(&linked, "Chunk"));

        let program = cx.program(&glue).unwrap();
        let layout = &program.chunk;
        let bytes = sel.chunk_uniform_bytes(layout, &glue.relative).unwrap();
        assert_eq!(bytes.len(), 4 * 256 + layout.span as usize);
        let read = |e: usize, member: &str| -> [u8; 4] {
            let at = e * 256 + layout.offset(member).unwrap() as usize;
            bytes[at..at + 4].try_into().unwrap()
        };
        let u = |e, m| u32::from_le_bytes(read(e, m));
        let f = |e, m| f32::from_le_bytes(read(e, m));
        // Chunk 1: its own rows and origin for both ends.
        assert_eq!(
            (u(1, "row_base"), u(1, "valid_offset"), u(1, "next_offset")),
            (8, 0, 1)
        );
        assert_eq!(f(1, "x_next_base"), f(1, "x_base"));
        // Bridge 0 (entry 3): chunk 0's row 7 to chunk 1's row 0.
        assert_eq!(
            (u(3, "row_base"), u(3, "valid_offset"), u(3, "next_offset")),
            (7, 7, 0)
        );
        assert_eq!(f(3, "x_base"), f(0, "x_base"));
        assert_eq!(f(3, "x_next_base"), f(1, "x_base"));
        assert_ne!(f(0, "x_base"), f(1, "x_base"));
        // Bridge 1 (entry 4): chunk 1's row 15 to chunk 2's row 16.
        assert_eq!(u(4, "row_base"), 15);
        assert_eq!(f(4, "x_next_base"), f(2, "x_base"));

        let batch = sel.prepare(&cx, &Theme::default(), false).unwrap();
        let gpu = &batch.gpu;
        let span = |chunk, n| Span { chunk, rows: 0..n };
        assert_eq!(
            gpu.spans,
            [span(0, 7), span(3, 1), span(1, 7), span(4, 1), span(2, 3)]
        );
        assert_eq!(batch.instances(), 19);
        assert!(
            gpu.chunks
                .iter()
                .all(|c| c.validity.is_some() && c.next_validity.is_some())
        );
        // A bridge binds one row of chunk 0 and the first row of chunk 1.
        let bridge = &gpu.chunks[3];
        assert_eq!(bridge.slots[0].1.end - bridge.slots[0].1.start, 4);
        assert_eq!(bridge.slots[2].0, gpu.chunks[1].columns);
        assert_eq!(bridge.slots[2].1.start, gpu.chunks[1].slots[0].1.start);
    }

    /// AC5 at the layer level: an append evaluates only the new rows,
    /// writes only their bytes and reuses the encodings' GPU state (no
    /// LUT upload); the next zoom-like prepare writes uniforms only.
    #[test]
    fn append_reuses_the_encodings_and_writes_new_rows_only() {
        let cx = Context::new_blocking().unwrap();
        let mut sel = reference();
        sel.max_chunk_rows(16);
        sel.fit_domains(&cx).unwrap();
        let first = sel.prepare(&cx, &Theme::default(), false).unwrap();
        let start = cx.upload_stats();
        sel.append((21..=40).map(|i| {
            let i = f64::from(i);
            Row {
                x: 1.7e9 + i * 60.0,
                y: i * i,
                v: i,
            }
        }))
        .unwrap();
        sel.fit_domains(&cx).unwrap();
        let grown = sel.prepare(&cx, &Theme::default(), false).unwrap();
        let written = cx.upload_stats() - start;
        // Three columns of 20 rows.
        assert_eq!(written.columns.bytes, 20 * 3 * 4, "{written:?}");
        assert_eq!(written.textures.writes, 0, "{written:?}");
        // The new chunk uniform buffer, then the encodings in place.
        assert_eq!(written.uniforms.writes, 2, "{written:?}");
        assert_eq!(grown.instances(), 40);
        assert_eq!(grown.gpu.chunks.len(), 3);
        assert!(!Arc::ptr_eq(&first.gpu, &grown.gpu));
        assert!(Arc::ptr_eq(first.gpu.encodings(), grown.gpu.encodings()));

        let start = cx.upload_stats();
        let again = sel.prepare(&cx, &Theme::default(), false).unwrap();
        let written = cx.upload_stats() - start;
        assert!(Arc::ptr_eq(&grown.gpu, &again.gpu));
        assert_eq!(written.columns, Default::default());
        assert_eq!(written.uniforms.writes, 2, "{written:?}");
    }

    /// Compare `actual` with a checked-in fixture; `GUP_BLESS=1` rewrites
    /// it.
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

    /// Two threads that miss the pipeline cache at once link the program
    /// once: the cache lock is held across the link.
    #[test]
    fn pipeline_cache_miss_links_once() {
        let host = Context::shared().expect("shared context");
        // A fresh cache on the shared device, so this misses.
        let cx = Context::from_wgpu(host.device().clone(), host.queue().clone());
        let glue = reference().glue();
        std::thread::scope(|s| {
            for _ in 0..2 {
                s.spawn(|| {
                    let program = cx.program(&glue).unwrap();
                    drop(cx.text());
                    program
                });
            }
        });
        let stats = cx.pipelines().stats;
        assert_eq!(stats.programs_linked, 1, "{stats:?}");
    }

    /// The brushed glue's style shows in its signature, field and WGSL
    /// (GUP-477); `Fade` is unchanged.
    #[test]
    fn brushed_glue_per_style() {
        let mut sel = reference();
        sel.brushable = true;
        sel.unbrushed = Unbrushed::Fade(0.3);
        let fade = sel.glue();
        assert!(fade.signature.ends_with(" brushed"), "{}", fade.signature);
        assert!(fade.source.contains("brush_dim"));
        assert!(!fade.source.contains("brush_grey"));
        sel.unbrushed = Unbrushed::Grey(crate::Color::hex(0xbfbfbf));
        let grey = sel.glue();
        assert!(
            grey.signature.ends_with(" brushed grey"),
            "{}",
            grey.signature
        );
        assert!(grey.source.contains("brush_grey: vec4<f32>"));
        assert!(
            grey.source
                .contains("mix(enc.brush_grey.rgb, m.fill.rgb, brush_k)")
        );
        assert_ne!(fade.signature, grey.signature);
    }

    /// The glue emitter's output is unchanged by build-time composition
    /// (GUP-406), and what wgpu compiles on every target is checked in.
    #[test]
    fn reference_glue_matches_fixtures() {
        let glue = reference().glue();
        assert_eq!(
            glue.signature,
            "Circle {x: f32rel→gup::scale::linear::map_rel, y: f32→gup::scale::log::map, \
             radius: const f32, fill: f32→gup::color::sequential::map(lut)\
             , fill_opacity: const f32, stroke: const vec4<f32>, stroke_width: const f32}"
        );
        assert_eq!(
            glue.columns,
            vec![
                (0, ColumnFormat::F32Relative),
                (1, ColumnFormat::F32),
                (3, ColumnFormat::F32)
            ]
        );
        assert_eq!(
            glue.relative,
            vec![glue::ChunkBase {
                channel: 0,
                link: 0,
                format: ColumnFormat::F32Relative,
                member: "x_base".into(),
                next: false,
                control: None,
            }]
        );
        check_fixture("scatter_glue.wgsl", &glue.source);
        let linked = link(&glue.signature, &glue.source, &glue.modules).unwrap();
        check_fixture("scatter_linked.wgsl", &linked);
    }

    /// The run-time path (glue linked to the library flattened at build
    /// time) against naga_oil composing the same glue directly: the same
    /// entry points and every struct laid out the same. naga_oil is a
    /// dev-dependency here; native tests are the only place it can check
    /// what wasm runs.
    #[test]
    fn linked_glue_matches_naga_oil_composition() {
        let glue = reference().glue();
        let linked = parse(
            &glue.signature,
            &link(&glue.signature, &glue.source, &glue.modules).unwrap(),
        );
        let mut oracle = gup_wgsl::compose::Library::new(&gup_wgsl::compose::library()).unwrap();
        let composed = oracle
            .compose(&glue.signature, &glue.source)
            .unwrap_or_else(|e| panic!("{e}"));
        let entry_points = |m: &naga::Module| {
            m.entry_points
                .iter()
                .map(|e| (e.name.clone(), e.stage))
                .collect::<Vec<_>>()
        };
        assert_eq!(entry_points(&linked), entry_points(&composed));
        assert_eq!(entry_points(&linked).len(), 2);
        let mut a = gup_wgsl::compose::struct_layouts(&linked);
        let mut b = gup_wgsl::compose::struct_layouts(&composed);
        a.sort_by(|x, y| x.0.cmp(&y.0));
        b.sort_by(|x, y| x.0.cmp(&y.0));
        assert_eq!(a, b);
        assert_eq!(a.len(), 9, "{a:?}");
    }

    /// The uniform offsets gup-core writes come from encase sizes under
    /// the 16-byte `Params` rule (no naga at run time); naga's layout of the
    /// linked module must agree, so the two never drift silently.
    #[test]
    fn uniform_offsets_match_naga_layout() {
        let glue = reference().glue();
        let linked = parse(
            &glue.signature,
            &link(&glue.signature, &glue.source, &glue.modules).unwrap(),
        );
        assert_eq!(
            Some(glue.encodings.clone()),
            struct_layout(&linked, "Encodings")
        );
        assert_eq!(Some(glue.chunk.clone()), struct_layout(&linked, "Chunk"));
        // Every Encodings field starts on a 16-byte boundary.
        assert!(
            glue.encodings
                .members
                .iter()
                .filter(|(name, _)| !name.contains("_pad_"))
                .all(|(_, o)| o % 16 == 0),
            "{:?}",
            glue.encodings
        );
        assert_eq!(glue.encodings.span, 112);
    }

    /// A layer whose channels are chains (GUP-418): a hi/lo `Time` x, a
    /// `Symlog` y, a three-link fill (`Linear`, `Sqrt`, viridis) and a
    /// three-link radius whose second link is a relative entry point
    /// reading an absolute value.
    fn chained() -> Selection<Row, Circle> {
        let rows = (1..=20)
            .map(|i| {
                let i = f64::from(i);
                Row {
                    x: 1.7e9 + i * 60.0,
                    y: (i - 10.0) * 1e3,
                    v: 1.7e9 + i,
                }
            })
            .collect::<Vec<_>>();
        let mut sel = Selection::<Row, Circle>::new(rows);
        let size = Linear::new()
            .range(Px(0.0), Px(100.0))
            .then(Linear::new().domain(0.0, 100.0).range(Px(0.0), Px(400.0)))
            .then(Pow::sqrt().range(Px(2.0), Px(12.0)));
        sel.attr(Circle::X, Time::new().encode(|r: &Row| r.x))
            .attr(Circle::Y, Symlog::new().encode(|r: &Row| r.y))
            .attr(Circle::RADIUS, size.encode(|r: &Row| r.v))
            .attr(
                Circle::FILL,
                Linear::new()
                    .range(Px(0.0), Px(1.0))
                    .then(Pow::sqrt())
                    .then(Sequential::viridis())
                    .encode(|r: &Row| r.v),
            );
        sel
    }

    /// GUP-418 AC1: each link of a chain has its own `Encodings` field
    /// (`<channel>`, `<channel>_link<k>`), the signature names every link,
    /// a LUT belongs to its link, and only first links read the chunk's
    /// origin; naga's layout of the linked module agrees.
    #[test]
    fn chained_glue_has_a_field_and_signature_per_link() {
        let glue = chained().glue();
        let fields: Vec<&str> = glue
            .encodings
            .members
            .iter()
            .map(|(n, _)| n.as_str())
            .filter(|n| !n.contains("_pad_"))
            .collect();
        assert_eq!(
            fields,
            [
                "x",
                "y",
                "radius",
                "radius_link1",
                "radius_link2",
                "fill",
                "fill_link1",
                "fill_link2",
                "fill_opacity",
                "stroke",
                "stroke_width"
            ]
        );
        assert!(
            glue.signature.contains(
                "fill: f32rel→gup::scale::linear::map_rel then f32→gup::scale::pow::map then \
                 f32→gup::color::sequential::map(lut)"
            ),
            "{}",
            glue.signature
        );
        assert!(
            glue.signature
                .contains("x: f32x2rel→gup::scale::time::map_rel, y: f32→gup::scale::symlog::map"),
            "{}",
            glue.signature
        );
        assert_eq!(
            glue.luts,
            vec![glue::LutBinding {
                channel: 3,
                link: 2,
                binding: 1
            }]
        );
        let chunk: Vec<&str> = glue.chunk.members.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(
            chunk,
            [
                "row_base",
                "x_base",
                "x_base_lo",
                "radius_base",
                "radius_link1_base",
                "fill_base"
            ]
        );
        assert!(
            glue.source.contains(
                "m.fill = sequential::map(pow::map(linear::map_rel(col.fill, chunk.fill_base, \
                 enc.fill), enc.fill_link1), enc.fill_link2, fill_link2_lut, fill_link2_smp);"
            ),
            "{}",
            glue.source
        );
        assert!(
            glue.source.contains(
                "m.x = time::map_rel(col.x, vec2<f32>(chunk.x_base, chunk.x_base_lo), enc.x);"
            ),
            "{}",
            glue.source
        );
        assert!(glue.source.contains("@location(0) x: vec2<f32>,"));
        let linked = parse(
            &glue.signature,
            &link(&glue.signature, &glue.source, &glue.modules).unwrap(),
        );
        assert_eq!(
            Some(glue.encodings.clone()),
            struct_layout(&linked, "Encodings")
        );
        assert_eq!(Some(glue.chunk.clone()), struct_layout(&linked, "Chunk"));
    }

    /// GUP-418 AC1: a first link's base comes from the chunk's origin, a
    /// later relative link's from origin 0, and a hi/lo base is split
    /// into words that sum to the f64 base; the layer prepares (its
    /// pipeline validates) and auto domains fit through the chain.
    #[test]
    fn chained_chunk_bases_apply_the_origin_once() {
        let cx = Context::new_blocking().unwrap();
        let mut sel = chained();
        sel.max_chunk_rows(8);
        sel.fit_domains(&cx).unwrap();
        let glue = sel.glue();
        let program = cx.program(&glue).unwrap();
        let bytes = sel
            .chunk_uniform_bytes(&program.chunk, &glue.relative)
            .unwrap();
        let store = sel.columns.as_ref().unwrap();
        let link = |ch: usize, k: usize| {
            let Some(Encoding::Column(c)) = &sel.encodings[ch] else {
                panic!()
            };
            c.links()[k].chunk_base(0.0)
        };
        let read = |entry: &[u8], member: &str| {
            let at = program.chunk.offset(member).unwrap() as usize;
            f32::from_le_bytes(entry[at..at + 4].try_into().unwrap())
        };
        for (k, chunk) in store.chunks().iter().enumerate() {
            let entry = &bytes[k * 256..];
            // x: Time over a hi/lo column: base = origin − d0, split.
            let origin = chunk.columns()[0].origin();
            let x_d0 = -link(0, 0);
            let (hi, lo) = (read(entry, "x_base"), read(entry, "x_base_lo"));
            assert_eq!(f64::from(hi) + f64::from(lo), origin - x_d0);
            // radius: the first link (relative) from the chunk origin.
            let origin = chunk.columns()[2].origin();
            let r_d0 = -link(2, 0);
            assert_eq!(read(entry, "radius_base"), (origin - r_d0) as f32);
            // The radius's second link reads an absolute value: origin 0.
            assert_eq!(read(entry, "radius_link1_base"), link(2, 1) as f32);
            assert_eq!(link(2, 1), 0.0);
        }
        // The fill chain's later links fitted to the image of the data
        // through the links before them: Linear's range, then its sqrt.
        let Some(Encoding::Column(fill)) = &sel.encodings[3] else {
            panic!()
        };
        let p = fill.links()[1].params_bytes().unwrap();
        let lo = f32::from_le_bytes(p[4..8].try_into().unwrap());
        let k = f32::from_le_bytes(p[8..12].try_into().unwrap());
        assert_eq!((lo, k), (0.0, 1.0), "sqrt fitted to [0, 1]");
        // The radius chain ends in `Sqrt` onto 2..12 px: marks overhang
        // their positions by up to 12 px.
        assert!((sel.overhang().size - 12.0).abs() < 1e-4);
        let batch = sel.prepare(&cx, &Theme::default(), false).unwrap();
        assert_eq!(batch.instances(), 20);
    }

    #[test]
    fn rust_params_match_wgsl_params() {
        use crate::shader::{
            COLOR_CATEGORICAL, COLOR_DIVERGING, COLOR_SEQUENTIAL, SCALE_LINEAR, SCALE_LOG,
            SCALE_OFFSET, SCALE_POW, SCALE_SYMLOG, SCALE_TIME,
        };
        use encase::ShaderType;
        for (module, size) in [
            (
                &COLOR_CATEGORICAL,
                crate::scale::CategoricalParams::min_size().get(),
            ),
            (&SCALE_LINEAR, crate::scale::LinearParams::min_size().get()),
            (&SCALE_LOG, crate::scale::LogParams::min_size().get()),
            (&SCALE_TIME, crate::scale::LinearParams::min_size().get()),
            (&SCALE_OFFSET, crate::scale::OffsetParams::min_size().get()),
            (&SCALE_POW, crate::scale::PowParams::min_size().get()),
            (&SCALE_SYMLOG, crate::scale::SymlogParams::min_size().get()),
            (
                &COLOR_SEQUENTIAL,
                crate::scale::SequentialParams::min_size().get(),
            ),
            (
                &COLOR_DIVERGING,
                crate::scale::DivergingParams::min_size().get(),
            ),
        ] {
            let wgsl = parse(module.import_path, module.wgsl);
            let params = gup_wgsl::flat_name(module.import_path, "Params");
            let layout = struct_layout(&wgsl, &params).unwrap();
            assert_eq!(u64::from(layout.span), size, "{}", module.import_path);
        }
    }

    /// A row whose fields may be missing.
    pub(crate) struct Place {
        x: f64,
        y: f64,
        v: f64,
        continent: Option<String>,
    }

    /// 40 places: x is NaN at row 3, y is +∞ at row 7, v is NaN at row 9
    /// and the continent is missing at row 5. Fill by continent (a
    /// dictionary column) or by `v` (a numeric column).
    pub(crate) fn places(fill_by_key: bool) -> Selection<Place, Circle> {
        let rows = (0..40)
            .map(|i| Place {
                x: if i == 3 { f64::NAN } else { f64::from(i) },
                y: if i == 7 {
                    f64::INFINITY
                } else {
                    f64::from(i + 1)
                },
                v: if i == 9 { f64::NAN } else { f64::from(i % 4) },
                continent: (i != 5).then(|| ["Asia", "Europe", "Africa"][i as usize % 3].into()),
            })
            .collect::<Vec<_>>();
        let mut sel = Selection::<Place, Circle>::new(rows);
        sel.attr(Circle::X, Linear::new().encode(|p: &Place| p.x))
            .attr(Circle::Y, Log::new().encode(|p: &Place| p.y))
            .attr(Circle::RADIUS, Px(3.0));
        if fill_by_key {
            sel.attr(
                Circle::FILL,
                Categorical::okabe_ito().encode_nullable_key(|p: &Place| p.continent.as_deref()),
            );
        } else {
            sel.attr(Circle::FILL, Sequential::viridis().encode(|p: &Place| p.v));
        }
        sel
    }

    /// S4b: once the columns have a null, the glue binds the validity bits,
    /// hides rows with a null position (a degenerate quad), and reads the
    /// fill from a `u32` dictionary column whose null code the categorical
    /// function resolves. The linked module validates and its uniform
    /// layouts match naga's.
    #[test]
    fn null_glue_matches_fixture_and_validates() {
        let cx = Context::new_blocking().unwrap();
        let mut sel = places(true);
        assert!(!sel.glue().validity, "no columns evaluated yet");
        sel.fit_domains(&cx).unwrap();
        let glue = sel.glue();
        assert!(glue.validity);
        assert_eq!(
            glue.signature,
            "Circle {x: f32rel→gup::scale::linear::map_rel, y: f32→gup::scale::log::map, \
             radius: const f32, fill: u32→gup::color::categorical::map(lut)\
             , fill_opacity: const f32, stroke: const vec4<f32>, stroke_width: const f32} with nulls"
        );
        assert_eq!(
            glue.columns,
            vec![
                (0, ColumnFormat::F32Relative),
                (1, ColumnFormat::F32),
                (3, ColumnFormat::U32)
            ]
        );
        check_fixture("nulls_glue.wgsl", &glue.source);
        let linked = parse(
            &glue.signature,
            &link(&glue.signature, &glue.source, &glue.modules).unwrap(),
        );
        assert_eq!(
            Some(glue.encodings.clone()),
            struct_layout(&linked, "Encodings")
        );
        assert_eq!(Some(glue.chunk.clone()), struct_layout(&linked, "Chunk"));
        // Two planes (x, y); the dictionary column has none.
        assert!(glue.source.contains("(instance_index / 32u) * 2u"));
        let batch = sel.prepare(&cx, &Theme::default(), false).unwrap();
        assert_eq!(batch.instances(), 40);
        assert!(batch.gpu.chunks.iter().all(|c| c.validity.is_some()));
    }

    /// S4b: a null in a numeric colour column draws in the null colour
    /// (a `select` on its validity bit) rather than hiding the row.
    #[test]
    fn numeric_colour_nulls_select_the_null_colour() {
        let cx = Context::new_blocking().unwrap();
        let mut sel = places(false);
        sel.fit_domains(&cx).unwrap();
        let glue = sel.glue();
        assert!(glue.source.contains(
            "m.fill = select(enc.null_color, sequential::map(col.fill, enc.fill, \
             fill_lut, fill_smp), ((validity[valid_group + 2u] >> valid_bit) & 1u) == 1u);"
        ));
        assert!(glue.source.contains(
            "let drawn = ((validity[valid_group + 0u] & validity[valid_group + 1u]) >> valid_bit) & 1u;"
        ));
        parse(
            &glue.signature,
            &link(&glue.signature, &glue.source, &glue.modules).unwrap(),
        );
        sel.prepare(&cx, &Theme::default(), false).unwrap();
    }

    /// The categorical null colour and key count land at naga's offsets,
    /// and the palette is its LUT resource (Okabe-Ito first).
    #[test]
    fn categorical_params_match_the_wgsl_layout() {
        use crate::encoding::{DynShaderFn, ShaderFn};
        use crate::scale::{NULL_COLOR, OKABE_ITO};
        use crate::shader::COLOR_CATEGORICAL;
        let module = parse(COLOR_CATEGORICAL.import_path, COLOR_CATEGORICAL.wgsl);
        let layout = struct_layout(
            &module,
            &gup_wgsl::flat_name(COLOR_CATEGORICAL.import_path, "Params"),
        )
        .unwrap();
        let mut c = Categorical::okabe_ito();
        let keys: Vec<std::sync::Arc<str>> = ["a", "b", "c"].map(Into::into).to_vec();
        c.fit_keys(&keys).unwrap();
        let bytes = DynShaderFn::params_bytes(&c).unwrap();
        let at = layout.offset("null_color").unwrap() as usize;
        let null: [f32; 4] = bytemuck::pod_read_unaligned(&bytes[at..at + 16]);
        assert_eq!(null, NULL_COLOR.to_array());
        let count = layout.offset("count").unwrap() as usize;
        assert_eq!(bytes[count..count + 4], 3u32.to_le_bytes());
        let [Resource::Lut(lut)] = &ShaderFn::resources(&c)[..] else {
            panic!("one LUT")
        };
        let rgba: Vec<[u8; 4]> = OKABE_ITO.iter().map(|c| c.to_rgba8()).collect();
        assert_eq!(lut[..8], rgba[..]);
    }
}

/// Wall-clock cost of creating the reference pipeline (RFC-001 §12 risk
/// 2): run with
/// `cargo test -p gup-core --lib pipeline_timings -- --ignored --nocapture`
/// (add `--release` for release numbers). `mask perf-budget` runs it in
/// release and compares its `metric` lines with `PERF_BUDGETS.md`.
#[cfg(test)]
mod timings {
    use super::tests::reference;
    use crate::context::Context;
    use crate::render::TargetDesc;
    use std::time::{Duration, Instant};

    fn summary(name: &str, mut v: Vec<Duration>) {
        v.sort();
        let ms = |d: Duration| d.as_secs_f64() * 1e3;
        eprintln!(
            "{name:<36} min {:>8.3} ms  median {:>8.3}  max {:>8.3}",
            ms(v[0]),
            ms(v[v.len() / 2]),
            ms(v[v.len() - 1]),
        );
    }

    #[test]
    #[ignore = "measurement, not a check; see RFC-001 S0a and GUP-406 findings"]
    fn pipeline_timings() {
        const RUNS: usize = 20;
        let host = Context::new_blocking().unwrap();
        let info = host
            .adapter_info()
            .map(|i| format!("{} ({:?}, {})", i.name, i.backend, i.driver_info))
            .unwrap_or_default();
        let sel = reference();
        let desc = TargetDesc {
            format: wgpu::TextureFormat::Rgba8Unorm,
            width: 640,
            height: 400,
            dpr: 1.0,
            samples: 1,
        };
        let (mut context, mut emit, mut link, mut create, mut total) =
            (vec![], vec![], vec![], vec![], vec![]);
        let mut first = None;
        for _ in 0..RUNS {
            // A fresh pipeline cache on the same device.
            let t = Instant::now();
            let cx = Context::from_wgpu(host.device().clone(), host.queue().clone());
            context.push(t.elapsed());
            let t = Instant::now();
            let glue = sel.glue();
            emit.push(t.elapsed());
            let program = cx.program(&glue).unwrap();
            let _pipeline = cx.mark_pipeline(&program, &desc).unwrap();
            let stats = cx.pipelines().stats;
            link.push(stats.last_link);
            create.push(stats.last_create);
            total.push(stats.last_link + stats.last_create);
            first.get_or_insert(stats.last_link + stats.last_create);
        }
        eprintln!(
            "profile: {}, runs: {RUNS}, adapter: {info}",
            if cfg!(debug_assertions) {
                "debug"
            } else {
                "release"
            }
        );
        summary("context creation (from_wgpu)", context);
        summary("glue emit", emit);
        summary("link (gup_wgsl::link)", link);
        summary("create_shader_module + pipeline", create);
        eprintln!(
            "link + create, first run             {:>8.3} ms",
            first.unwrap().as_secs_f64() * 1e3
        );
        summary("link + create", total.clone());
        // For `scripts/perf_budget.sh` (`mask perf-budget`, PERF_BUDGETS.md).
        total.sort();
        let ms = |d: Duration| d.as_secs_f64() * 1e3;
        println!(
            "metric pipeline.link_create.median_ms {:.3}",
            ms(total[total.len() / 2])
        );
        println!(
            "metric pipeline.link_create.cold_ms {:.3}",
            ms(first.unwrap())
        );
        println!(
            "metric pipeline.link_create.max_ms {:.3}",
            ms(total[total.len() - 1])
        );
    }
}

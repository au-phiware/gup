// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! `Selection<T, M>`: rows of `T` drawn as mark `M`, with typed channels
//! (RFC-001 §4). S0a scope: data and `attr`; events, transitions and
//! append are later S-stories.

use crate::channel::{Channel, ConstValue, Geometry, HitShape, Mark, Role, Visual};
use crate::column::{ColumnFormat, ColumnStore};
use crate::context::Context;
use crate::cull::{self, PixelBox};
use crate::encoding::{AxisEncoding, Encoding, IntoEncoding, Resource};
use crate::error::{Error, Result};
use crate::geom::Rect;
use crate::input::{Event, EventInfo};
use crate::order::{DrawEntry, DrawOrder, chunk_spans, draw_entries, polyline_spans};
use crate::pick::PICK_TOLERANCE;
use crate::pick::{Chan, Chans, PickItem, build_items};
use crate::render::{
    ChunkDraw, Draws, LayerGpu, LayerUniforms, chunk_uniform_offset, chunk_uniforms,
};
use crate::scale::{DynPositionScale, Legend};
use crate::scene::MarkBatch;
use crate::shader::glue::{self, ChannelSource, ChunkBase, Glue, GlueChannel, GlueSpec};
use crate::theme::Theme;
use std::any::Any;
use std::marker::PhantomData;
use std::sync::Arc;

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
    /// How many leading rows were dropped after upload ([`Retain`]).
    released: usize,
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
    _mark: PhantomData<fn() -> M>,
}

impl<T, M: Mark> std::fmt::Debug for Selection<T, M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Selection")
            .field("mark", &M::NAME)
            .field("rows", &(self.released + self.rows.len()))
            .field("retain", &self.retain)
            .field("encodings", &self.encodings)
            .finish_non_exhaustive()
    }
}

impl<T: Send + Sync + 'static, M: Mark> Selection<T, M> {
    /// A selection over `rows`. Channels not set with [`attr`](Self::attr)
    /// use the mark's defaults.
    pub fn new(rows: impl Into<Vec<T>>) -> Self {
        Self {
            rows: rows.into(),
            released: 0,
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

    /// Append `rows`: only they are evaluated, and the next resolve writes
    /// only their bytes, at the tail of the last chunk (or in new chunks).
    /// Auto domains refit to the grown stats on that resolve, and new keys
    /// get the next dictionary codes. It works under every [`Retain`]
    /// policy: the last chunk keeps its CPU copy, and appended rows are
    /// released after their upload like the rest. (A handle to append to a
    /// selection already in a plot is RFC-001 S12.)
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "the public append handle is RFC-001 S12")
    )]
    pub(crate) fn append(&mut self, rows: impl IntoIterator<Item = T>) -> Result<()> {
        let start = self.rows.len();
        self.rows.extend(rows);
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
                order.push_batch(&mut values);
            }
            store.append(new.len(), &values)?;
            // Appends interleaved with the stored rows into many draws:
            // evaluate every row again, in one sorted run (while the rows
            // are kept).
            if self.released == 0 && self.order.as_ref().is_some_and(DrawOrder::needs_compaction) {
                self.columns = None;
            }
        }
        Ok(())
    }

    /// Number of rows, including any dropped after upload.
    pub fn len(&self) -> usize {
        self.released + self.rows.len()
    }

    /// Whether there are no rows.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Set how much CPU data to keep once the columns are on the GPU
    /// (default [`Retain::Auto`]).
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
        self.released > 0
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

    /// Run the accessors (once) into a column store chunked for `cx`'s
    /// device.
    fn evaluate(&mut self, cx: &Context) -> Result<&mut ColumnStore> {
        let formats: Vec<_> = self
            .column_encodings()
            .map(|(_, c)| c.links()[0].input_format())
            .collect();
        let chunk_rows = ColumnStore::chunk_rows_for(&cx.caps().limits, &formats)
            .min(self.max_chunk_rows.unwrap_or(u32::MAX));
        let stale = match &self.columns {
            None => Some("a channel was encoded, or `max_chunk_rows` changed"),
            Some(c) if c.chunk_rows() != chunk_rows => {
                Some("this device (or `max_chunk_rows`) needs a different chunk size")
            }
            // Released CPU columns (`Retain::Rows`, `GpuOnly`) cannot go to
            // another context; with the rows, evaluate them again.
            Some(c) if !c.uploadable_to(cx) => {
                Some("the columns are on another context and their CPU copy was dropped")
            }
            Some(_) => None,
        };
        if let Some(reason) = stale {
            if self.released > 0 {
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
            if let Some(order) = &mut self.order {
                order.push_batch(&mut values);
            }
            let mut store = ColumnStore::new(formats, chunk_rows)?;
            store.append(self.rows.len(), &values)?;
            self.columns = Some(store);
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
        // (base, column index, link function) per relative link.
        let relative: Vec<_> = relative
            .iter()
            .map(|b| {
                let (col, (_, c)) = self
                    .column_encodings()
                    .enumerate()
                    .find(|(_, (i, _))| *i == b.channel)
                    .expect("relative bases belong to column channels");
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
    /// Evaluate accessors (chunked for `cx`'s device) and fit every
    /// data-driven domain to its column's stats across every chunk.
    fn fit_domains(&mut self, cx: &Context) -> Result<()>;
    /// How far marks may extend past their position (e.g. a constant
    /// radius), in pixels.
    fn overhang(&self) -> f32;
    /// The largest value any non-position `f32` channel can take (a
    /// radius, a width, a stroke width, an opacity): how far a mark can
    /// reach from its position. `INFINITY` when a channel's range is not
    /// known.
    fn cull_margin(&self) -> f32;
    /// Which of the layer's draw entries (`order::draw_entries`) can put a
    /// pixel inside `plot`, from the chunks' position stats and the
    /// current scales. Empty when the layer has no column store.
    fn visible_entries(&self, plot: Rect) -> Vec<bool>;
    /// Generate the glue, link it (cached), write uniforms and upload.
    fn prepare(&mut self, cx: &Context, theme: &Theme) -> Result<MarkBatch>;
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
    /// The layer's draw order, if it draws by size: how a GPU pick maps a
    /// stored row back to its data row (`None` maps a row to itself).
    fn draw_order(&self) -> Option<&DrawOrder>;
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
            let desc = &M::CHANNELS[slot];
            let channel = format!("{}::{}", M::NAME, desc.name.to_ascii_uppercase());
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
        let mut sum = 0u64;
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

    /// The largest size: a constant's, or an encoded channel's image of
    /// its column's extent (GUP-418: a `Sqrt` radius reaches its range's
    /// far end).
    fn overhang(&self) -> f32 {
        let size = |v: &crate::ConstValue| match v {
            crate::ConstValue::F32(r) => *r,
            crate::ConstValue::Vec4(_) => 0.0,
        };
        let mut column = 0;
        let mut overhang = 0.0f32;
        for (d, e) in M::CHANNELS.iter().zip(&self.encodings) {
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
                _ if d.role != Some(Role::Size) => continue,
                Some(Encoding::Const(v)) => size(v),
                Some(Encoding::Derived(f)) => size(&f()),
                None => size(&d.default),
            };
            overhang = overhang.max(r);
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
        let boxes: Vec<Option<PixelBox>> = (0..stored.len())
            .map(|k| {
                let mut b = PixelBox::EMPTY;
                let mut column = 0;
                for (d, e) in M::CHANNELS.iter().zip(&self.encodings) {
                    let axis = match d.role {
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

    fn prepare(&mut self, cx: &Context, theme: &Theme) -> Result<MarkBatch> {
        self.check_module_paths()?;
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
            return Err(Error::config(
                "layer",
                format!(
                    "{}: {slots} vertex columns ({} channels read from columns{}), and this \
                     device has {} vertex buffer slots; make some channels constants",
                    M::NAME,
                    glue.columns.len(),
                    if glue.successors.is_empty() {
                        String::new()
                    } else {
                        format!(", {} read again at the next row", glue.successors.len())
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
        let entries = draw_entries(M::GEOMETRY, stored);
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
                return Ok(MarkBatch::new(Arc::clone(gpu), M::ANTIALIAS));
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
            let grown = Arc::new(gpu.with_chunks(cx, &chunks, draws)?);
            grown.write_encodings(cx, &encodings);
            self.gpu = Some((Arc::clone(&grown), luts));
            return Ok(MarkBatch::new(grown, M::ANTIALIAS));
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
            }
            .build(
                cx,
                Draws {
                    chunks: chunk_draws(),
                    spans,
                },
                M::VERTICES_PER_INSTANCE,
            )?,
        );
        self.gpu = Some((Arc::clone(&gpu), luts));
        Ok(MarkBatch::new(gpu, M::ANTIALIAS))
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
        } else if self.released == 0 && self.rows.len() == n {
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
        let chans = Chans {
            x: chan("x"),
            y: chan("y"),
            radius: chan("radius"),
            width: chan("width"),
            height: chan("height"),
            x_end: chan("x_end"),
            y_end: chan("y_end"),
        };

        // Stored rows with their data row and rank in the draw order.
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
            build_items(M::HIT, layer, self.z, &slots, &chans, out);
        }
        Ok(())
    }

    fn draw_order(&self) -> Option<&DrawOrder> {
        self.order.as_ref()
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

        let batch = sel.prepare(&cx, &Theme::default()).unwrap();
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

        let batch = sel.prepare(&cx, &Theme::default()).unwrap();
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
        let first = sel.prepare(&cx, &Theme::default()).unwrap();
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
        let grown = sel.prepare(&cx, &Theme::default()).unwrap();
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
        let again = sel.prepare(&cx, &Theme::default()).unwrap();
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
        assert!((sel.overhang() - 12.0).abs() < 1e-4, "{}", sel.overhang());
        let batch = sel.prepare(&cx, &Theme::default()).unwrap();
        assert_eq!(batch.instances(), 20);
    }

    #[test]
    fn rust_params_match_wgsl_params() {
        use crate::shader::{
            COLOR_CATEGORICAL, COLOR_DIVERGING, COLOR_SEQUENTIAL, SCALE_LINEAR, SCALE_LOG,
            SCALE_POW, SCALE_SYMLOG, SCALE_TIME,
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
        let batch = sel.prepare(&cx, &Theme::default()).unwrap();
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
        sel.prepare(&cx, &Theme::default()).unwrap();
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

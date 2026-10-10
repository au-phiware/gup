// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! `Selection<T, M>`: rows of `T` drawn as mark `M`, with typed channels
//! (RFC-001 §4). S0a scope: data and `attr`; events, transitions and
//! append are later S-stories.

use crate::channel::{Channel, Mark, Role, Visual};
use crate::column::{ColumnData, ColumnFormat, ColumnStore};
use crate::context::Context;
use crate::encoding::{Encoding, IntoEncoding, Resource};
use crate::error::{Error, Result};
use crate::render::{ChunkDraw, LayerGpu, LayerUniforms, chunk_uniform_offset, chunk_uniforms};
use crate::scene::MarkBatch;
use crate::shader::glue::{self, ChannelSource, ChunkBase, Glue, GlueChannel, GlueSpec};
use std::any::Any;
use std::marker::PhantomData;
use std::sync::Arc;

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
    encodings: Vec<Option<Encoding<T>>>,
    /// Evaluated columns, one per column-encoded channel in channel order.
    columns: Option<ColumnStore>,
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
            encodings: M::CHANNELS.iter().map(|_| None).collect(),
            columns: None,
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
            let values: Vec<ColumnData<'_>> = self
                .encodings
                .iter()
                .filter_map(|e| match e {
                    Some(Encoding::Column(c)) => Some(c.evaluate(new)),
                    _ => None,
                })
                .collect();
            store.append(new.len(), &values)?;
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
        self.encodings[usize::from(channel.slot())] = Some(encoding.into_encoding());
        // S0a re-evaluates every column; per-column invalidation is S4.
        self.columns = None;
        self.gpu = None;
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
            let values: Vec<ColumnData<'_>> = self
                .column_encodings()
                .map(|(_, c)| c.evaluate(&self.rows))
                .collect();
            let mut store = ColumnStore::new(formats, chunk_rows)?;
            store.append(self.rows.len(), &values)?;
            self.columns = Some(store);
        }
        Ok(self.columns.as_mut().expect("evaluated above"))
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
        })
    }

    /// The `Chunk` uniform entries (laid out as `layout`) of every chunk
    /// of the evaluated store: each chunk's first row and each relative
    /// link's base in `relative`. A channel's first link reads the stored
    /// column, so its base is `(origin - d0)` from the chunk's own f64
    /// origin; a later link reads an absolute value (origin 0). A hi/lo
    /// link gets its base as a hi/lo pair.
    fn chunk_uniform_bytes(
        &self,
        layout: &crate::shader::StructLayout,
        relative: &[ChunkBase],
    ) -> Result<Vec<u8>> {
        let store = self.columns.as_ref().ok_or_else(|| {
            Error::config("layer", "prepare called before the columns were evaluated")
        })?;
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
        let mut bytes = chunk_uniforms(store.chunks().len(), layout.span);
        for (k, chunk) in store.chunks().iter().enumerate() {
            let entry = &mut bytes[chunk_uniform_offset(k)..][..layout.span as usize];
            let row_base = u32::try_from(chunk.row_base()).map_err(|_| {
                Error::config(
                    "column store",
                    format!(
                        "{}: chunk {k} starts at row {}, past the u32 row index",
                        M::NAME,
                        chunk.row_base()
                    ),
                )
            })?;
            write_field(entry, layout, "row_base", &row_base.to_le_bytes())?;
            for (base, col, func) in &relative {
                let origin = if base.link == 0 {
                    chunk.columns()[*col].origin()
                } else {
                    0.0
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

/// A plot layer: the object-safe face of a [`Selection`] (internal; the
/// public `Layer`/`Chart` traits are RFC-001 S7).
pub(crate) trait Layer: wgpu::WasmNotSendSync {
    /// Evaluate accessors (chunked for `cx`'s device) and fit every
    /// data-driven domain to its column's stats across every chunk.
    fn fit_domains(&mut self, cx: &Context) -> Result<()>;
    /// How far marks may extend past their position (e.g. a constant
    /// radius), in pixels.
    fn overhang(&self) -> f32;
    /// Generate the glue, link it (cached), write uniforms and upload.
    fn prepare(&mut self, cx: &Context) -> Result<MarkBatch>;
    /// Drop the CPU data the [`Retain`] policy does not keep. Called after
    /// a successful [`prepare`](Self::prepare), so everything dropped is
    /// on the GPU.
    fn release(&mut self);
    /// The generated glue (for tests and diagnostics).
    fn glue_source(&self) -> Glue;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

impl<T: Send + Sync + 'static, M: Mark> Layer for Selection<T, M> {
    fn fit_domains(&mut self, cx: &Context) -> Result<()> {
        let store = self.evaluate(cx)?;
        let stats: Vec<_> = (0..store.formats().len()).map(|k| store.stats(k)).collect();
        let mut k = 0;
        for (i, enc) in self.encodings.iter_mut().enumerate() {
            if let Some(Encoding::Column(c)) = enc {
                if let Some(s) = stats[k] {
                    c.fit(s.extent())
                        .map_err(|e| context(e, M::NAME, M::CHANNELS[i].name))?;
                }
                k += 1;
            }
        }
        Ok(())
    }

    fn overhang(&self) -> f32 {
        M::CHANNELS
            .iter()
            .zip(&self.encodings)
            .filter(|(d, _)| d.role == Some(Role::Size))
            .map(|(d, e)| match e {
                Some(Encoding::Const(v)) => v,
                _ => &d.default,
            })
            .map(|v| match v {
                crate::ConstValue::F32(r) => *r,
                crate::ConstValue::Vec4(_) => 0.0,
            })
            .fold(0.0, f32::max)
    }

    fn prepare(&mut self, cx: &Context) -> Result<MarkBatch> {
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
                None => {
                    let bytes = desc.default.bytes();
                    write_field(&mut encodings, &program.encodings, desc.name, &bytes)?;
                }
            }
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

        if glue.validity && cx.caps().limits.max_storage_buffers_per_shader_stage == 0 {
            return Err(Error::config(
                "layer",
                format!(
                    "{}: its columns have nulls, whose validity bits are read from a storage \
                     buffer in the vertex stage, and this device allows none",
                    M::NAME
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
        let uploaded = || {
            store
                .chunks()
                .iter()
                .map(|c| (c.buffer(cx).expect("uploaded above"), validity(c), c.rows()))
        };
        let draws = || -> Vec<ChunkDraw> {
            store
                .chunks()
                .iter()
                .enumerate()
                .map(|(k, c)| ChunkDraw {
                    columns: c.buffer(cx).expect("uploaded above").clone(),
                    column_ranges: (0..store.formats().len())
                        .map(|i| c.column_range(i))
                        .collect(),
                    instances: c.rows(),
                    dynamic_offset: chunk_uniform_offset(k) as u32,
                    validity: validity(c).cloned(),
                })
                .collect()
        };

        if let Some((gpu, held)) = &self.gpu
            && gpu.reusable(cx, &program)
            && *held == luts
        {
            // Zoom, pan and resize land here every frame: write the new
            // uniform values into the existing buffers.
            if gpu.draws_chunks(uploaded()) {
                gpu.write_uniforms(cx, &encodings, &chunks);
                return Ok(MarkBatch {
                    gpu: Arc::clone(gpu),
                });
            }
            // Appended rows: new chunk draws and `Chunk` entries over the
            // same encodings, which take their new values in place.
            #[cfg_attr(
                target_arch = "wasm32",
                expect(
                    clippy::arc_with_non_send_sync,
                    reason = "`Arc`, not `Rc`, because `Layer` must be `Send + Sync` on \
                              native; on wasm32 wgpu's handles are neither"
                )
            )]
            let grown = Arc::new(gpu.with_chunks(cx, &chunks, draws())?);
            grown.write_encodings(cx, &encodings);
            self.gpu = Some((Arc::clone(&grown), luts));
            return Ok(MarkBatch { gpu: grown });
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
            .build(cx, draws(), M::VERTICES_PER_INSTANCE)?,
        );
        self.gpu = Some((Arc::clone(&gpu), luts));
        Ok(MarkBatch { gpu })
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

    fn glue_source(&self) -> Glue {
        self.glue()
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
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
            detail: format!("{mark}::{} channel: {detail}", channel.to_uppercase()),
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

        let batch = sel.prepare(&cx).unwrap();
        let offsets: Vec<_> = batch.gpu.chunks.iter().map(|c| c.dynamic_offset).collect();
        assert_eq!(offsets, [0, 256, 512]);
        assert_eq!(batch.instances(), 20);
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
        let first = sel.prepare(&cx).unwrap();
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
        let grown = sel.prepare(&cx).unwrap();
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
        let again = sel.prepare(&cx).unwrap();
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
             radius: const f32, fill: f32→gup::color::sequential::map(lut)}"
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
        let dir = gup_wgsl::compose::read_dir(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("src/shaders"),
            "src/shaders",
        )
        .unwrap();
        let mut oracle = gup_wgsl::compose::Library::new(&dir.modules).unwrap();
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
        assert_eq!(glue.encodings.span, 64);
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
                "fill_link2"
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
        let batch = sel.prepare(&cx).unwrap();
        assert_eq!(batch.instances(), 20);
    }

    #[test]
    fn rust_params_match_wgsl_params() {
        use crate::shader::{
            COLOR_CATEGORICAL, COLOR_SEQUENTIAL, SCALE_LINEAR, SCALE_LOG, SCALE_POW, SCALE_SYMLOG,
            SCALE_TIME,
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
             radius: const f32, fill: u32→gup::color::categorical::map} with nulls"
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
        let batch = sel.prepare(&cx).unwrap();
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
            "m.fill = select(vec4<f32>(0.6, 0.6, 0.6, 1.0), sequential::map(col.fill, enc.fill, \
             fill_lut, fill_smp), ((validity[valid_group + 2u] >> valid_bit) & 1u) == 1u);"
        ));
        assert!(glue.source.contains(
            "let drawn = ((validity[valid_group + 0u] & validity[valid_group + 1u]) >> valid_bit) & 1u;"
        ));
        parse(
            &glue.signature,
            &link(&glue.signature, &glue.source, &glue.modules).unwrap(),
        );
        sel.prepare(&cx).unwrap();
    }

    /// The categorical palette and null colour land at naga's offsets.
    #[test]
    fn categorical_params_match_the_wgsl_layout() {
        use crate::encoding::DynShaderFn;
        use crate::scale::{NULL_COLOR, OKABE_ITO};
        use crate::shader::COLOR_CATEGORICAL;
        let module = parse(COLOR_CATEGORICAL.import_path, COLOR_CATEGORICAL.wgsl);
        let layout = struct_layout(
            &module,
            &gup_wgsl::flat_name(COLOR_CATEGORICAL.import_path, "Params"),
        )
        .unwrap();
        let bytes = DynShaderFn::params_bytes(&Categorical::okabe_ito()).unwrap();
        let vec4 = |at: u32| -> [f32; 4] {
            bytemuck::pod_read_unaligned(&bytes[at as usize..at as usize + 16])
        };
        let colors = layout.offset("colors").unwrap();
        for (i, c) in OKABE_ITO.iter().enumerate() {
            assert_eq!(vec4(colors + 16 * i as u32), c.to_array());
        }
        assert_eq!(
            vec4(layout.offset("null_color").unwrap()),
            NULL_COLOR.to_array()
        );
        let count = layout.offset("count").unwrap() as usize;
        assert_eq!(bytes[count..count + 4], 8u32.to_le_bytes());
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

// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! The GPU column store (RFC-001 §3).
//!
//! Accessors run once; their outputs are stored as structure-of-arrays in
//! **one buffer per chunk**, each column at a 256-byte-aligned sub-range,
//! and bound as instance-rate vertex buffers. Relative columns store
//! `(v - origin) as f32` against a per-chunk f64 origin so that large
//! values (timestamps) keep f32 precision.
//!
//! A store holds [`chunk_rows`](ColumnStore::chunk_rows) rows per chunk,
//! `min(2^20, max_buffer_size / Σ column stride)` (see
//! [`ColumnStore::chunk_rows_for`]); every chunk but the last is full.
//! Each chunk is drawn by one instanced draw with its own uniform entry
//! (`row_base` and each relative column's base).
//!
//! [`append`](ColumnStore::append) evaluates only the new rows. It fills
//! the last chunk, then opens new ones with fresh origins, and the next
//! [`upload`](ColumnStore::upload) writes only the appended rows' bytes.
//!
//! ## Nulls (RFC-001 S4b)
//!
//! A null is a non-finite value (NaN or ±∞) in a numeric column, or a
//! missing key in a [`U32`](ColumnFormat::U32) dictionary column. Shaders
//! never test a value for NaN:
//!
//! - **Numeric columns** get validity bits: one bit per row and numeric
//!   column, set when the value is finite. A chunk's bits are laid out in
//!   32-row groups, one `u32` word per numeric column (a *plane*) in each
//!   group: row `r` of plane `p` is bit `r % 32` of word
//!   `(r / 32) × planes + p`. Growing a chunk only extends the array, and
//!   a tail write rewrites the last partial group. The bits live in a
//!   small storage buffer per chunk, created only once the store has a
//!   null, so data without nulls pays nothing on the GPU; being a storage
//!   binding, it uses none of the 8 vertex-buffer slots (RFC-001 §12 risk
//!   7). A null row's value bytes are unspecified, and stats skip it.
//! - **Dictionary columns** store [`NULL_CODE`] for a missing key, which
//!   the colour function resolves to its null colour.
//!
//! ## Retention
//!
//! [`release`](ColumnStore::release) drops the CPU copy of every full
//! chunk whose rows are uploaded (the `Retain` policies of RFC-001 §3).
//! Stats, origins and dictionaries stay. A released chunk can still be
//! drawn and appended after, but not uploaded to another context:
//! [`upload`](ColumnStore::upload) says so with an error.

use crate::context::{Context, ContextId, Upload};
use crate::error::{Error, Result};
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;

/// How a column's values are stored on the GPU.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum ColumnFormat {
    /// Absolute `f32`.
    F32,
    /// `f32` relative to the chunk's f64 origin.
    F32Relative,
    /// `u32` dictionary codes (band and categorical scales): each distinct
    /// key gets the next code in first-seen order, and a missing key is
    /// [`NULL_CODE`].
    U32,
}

impl ColumnFormat {
    /// Bytes per row.
    pub const fn stride(self) -> u64 {
        4
    }

    /// Whether nulls in a column of this format are recorded as validity
    /// bits: numeric formats are, while dictionary codes reserve
    /// [`NULL_CODE`] instead.
    pub const fn has_validity(self) -> bool {
        matches!(self, Self::F32 | Self::F32Relative)
    }

    /// The WGSL type of a value in the column.
    pub(crate) const fn wgsl_type(self) -> &'static str {
        match self {
            Self::F32 | Self::F32Relative => "f32",
            Self::U32 => "u32",
        }
    }

    /// The vertex format the column is fetched with.
    pub(crate) const fn vertex_format(self) -> wgpu::VertexFormat {
        match self {
            Self::F32 | Self::F32Relative => wgpu::VertexFormat::Float32,
            Self::U32 => wgpu::VertexFormat::Uint32,
        }
    }
}

/// The code a [`U32`](ColumnFormat::U32) column stores for a null (missing)
/// key. No key is ever given this code.
pub const NULL_CODE: u32 = u32::MAX;

/// Keys to `u32` codes, in first-seen order: the domain of a dictionary
/// column. One per [`U32`](ColumnFormat::U32) column of a store, and
/// append-only, so appended rows never renumber a code; a new key only
/// grows the domain.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Dictionary {
    codes: HashMap<Arc<str>, u32>,
    keys: Vec<Arc<str>>,
}

impl Dictionary {
    /// An empty dictionary.
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of distinct keys.
    pub fn len(&self) -> usize {
        self.keys.len()
    }

    /// Whether there are no keys.
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    /// The code of `key`, if it has been seen.
    pub fn code(&self, key: &str) -> Option<u32> {
        self.codes.get(key).copied()
    }

    /// The key with `code`, if any ([`NULL_CODE`] has none).
    pub fn key(&self, code: u32) -> Option<&str> {
        self.keys.get(code as usize).map(|k| &**k)
    }

    /// The keys in code order (first-seen order): the domain.
    pub fn keys(&self) -> impl ExactSizeIterator<Item = &str> {
        self.keys.iter().map(|k| &**k)
    }

    /// The code of `key`, giving it the next code if it is new.
    pub(crate) fn encode(&mut self, key: &str) -> Result<u32> {
        if let Some(&code) = self.codes.get(key) {
            return Ok(code);
        }
        let code = u32::try_from(self.keys.len())
            .ok()
            .filter(|&c| c != NULL_CODE)
            .ok_or_else(|| {
                Error::config(
                    "dictionary column",
                    format!(
                        "more than {} distinct keys; the last code is reserved for null",
                        NULL_CODE
                    ),
                )
            })?;
        let key: Arc<str> = key.into();
        self.codes.insert(Arc::clone(&key), code);
        self.keys.push(key);
        Ok(code)
    }
}

/// New rows of one column, as an accessor produced them.
#[derive(Clone, Debug, PartialEq)]
pub enum ColumnData<'a> {
    /// Numbers, for an [`F32`](ColumnFormat::F32) or
    /// [`F32Relative`](ColumnFormat::F32Relative) column. Non-finite
    /// values are nulls.
    Values(Vec<f64>),
    /// Keys, for a [`U32`](ColumnFormat::U32) column, encoded through the
    /// column's [`Dictionary`]. `None` is a null.
    Keys(Vec<Option<&'a str>>),
}

impl ColumnData<'_> {
    fn len(&self) -> usize {
        match self {
            Self::Values(v) => v.len(),
            Self::Keys(k) => k.len(),
        }
    }

    /// Whether this data can fill a column of `format`.
    fn fits(&self, format: ColumnFormat) -> bool {
        match self {
            Self::Values(_) => format.has_validity(),
            Self::Keys(_) => format == ColumnFormat::U32,
        }
    }
}

impl From<Vec<f64>> for ColumnData<'_> {
    fn from(values: Vec<f64>) -> Self {
        Self::Values(values)
    }
}

/// f64 statistics of a column's non-null values in one chunk, or (from
/// [`ColumnStore::stats`]) across every chunk. They drive automatic
/// domains (and, from RFC-001 S9, chunk culling). For a dictionary column
/// the values are codes.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ColumnStats {
    /// Smallest finite value.
    pub min: f64,
    /// Largest finite value.
    pub max: f64,
    /// Number of nulls: non-finite values (NaN or ±∞), or missing keys.
    pub non_finite: u32,
}

impl ColumnStats {
    /// `(min, max)`.
    pub fn extent(&self) -> (f64, f64) {
        (self.min, self.max)
    }
}

/// Running statistics: empty while `min > max`, so the non-finite count
/// survives chunks (and appends) that have no finite value yet.
#[derive(Copy, Clone, Debug, PartialEq)]
struct Acc {
    min: f64,
    max: f64,
    non_finite: u32,
}

impl Acc {
    const EMPTY: Self = Self {
        min: f64::INFINITY,
        max: f64::NEG_INFINITY,
        non_finite: 0,
    };

    fn push(&mut self, v: f64) {
        if v.is_finite() {
            self.min = self.min.min(v);
            self.max = self.max.max(v);
        } else {
            self.non_finite += 1;
        }
    }

    fn merge(self, other: Self) -> Self {
        Self {
            min: self.min.min(other.min),
            max: self.max.max(other.max),
            non_finite: self.non_finite + other.non_finite,
        }
    }

    fn stats(self) -> Option<ColumnStats> {
        (self.min <= self.max).then_some(ColumnStats {
            min: self.min,
            max: self.max,
            non_finite: self.non_finite,
        })
    }
}

/// One column of one chunk.
#[derive(Clone, Debug)]
pub struct ChunkColumn {
    format: ColumnFormat,
    /// Byte offset of the column in the chunk buffer (256-aligned).
    offset: u64,
    /// The origin subtracted from every value: the chunk's first finite
    /// value for relative columns (`None` until there is one), 0 for
    /// absolute columns.
    origin: Option<f64>,
    stats: Acc,
}

impl ChunkColumn {
    /// The storage format.
    pub fn format(&self) -> ColumnFormat {
        self.format
    }

    /// The f64 origin values are stored relative to (0 for absolute
    /// columns, and for relative columns with no finite value yet).
    pub fn origin(&self) -> f64 {
        self.origin.unwrap_or(0.0)
    }

    /// Statistics of the non-null values; `None` if there are none.
    pub fn stats(&self) -> Option<ColumnStats> {
        self.stats.stats()
    }

    /// Number of null rows.
    pub fn nulls(&self) -> u32 {
        self.stats.non_finite
    }

    /// Store `values` as rows `at..` of this numeric column in `bytes`
    /// (the chunk's CPU copy), updating the origin and stats.
    fn write(&mut self, bytes: &mut [u8], at: u32, values: &[f64]) {
        if self.origin.is_none() {
            // Rows already stored are non-finite, so the origin they were
            // written against does not matter.
            self.origin = match self.format {
                ColumnFormat::F32Relative => values.iter().copied().find(|v| v.is_finite()),
                ColumnFormat::F32 | ColumnFormat::U32 => Some(0.0),
            };
        }
        let origin = self.origin();
        let stride = self.format.stride() as usize;
        let start = self.offset as usize + at as usize * stride;
        let dst = &mut bytes[start..start + values.len() * stride];
        for (cell, &v) in dst.chunks_exact_mut(stride).zip(values) {
            cell.copy_from_slice(&((v - origin) as f32).to_le_bytes());
            self.stats.push(v);
        }
    }

    /// Store dictionary `codes` as rows `at..` of this `U32` column.
    fn write_codes(&mut self, bytes: &mut [u8], at: u32, codes: &[u32]) {
        self.origin = Some(0.0);
        let stride = self.format.stride() as usize;
        let start = self.offset as usize + at as usize * stride;
        let dst = &mut bytes[start..start + codes.len() * stride];
        for (cell, &code) in dst.chunks_exact_mut(stride).zip(codes) {
            cell.copy_from_slice(&code.to_le_bytes());
            self.stats.push(if code == NULL_CODE {
                f64::NAN
            } else {
                f64::from(code)
            });
        }
    }
}

/// Validity words for `rows` rows of `planes` numeric columns.
fn validity_words(rows: u32, planes: usize) -> usize {
    rows.div_ceil(32) as usize * planes
}

/// A chunk's CPU copy: what uploads read and appends write. Released
/// (dropped) by [`ColumnStore::release`] once the chunk is full and
/// uploaded.
#[derive(Debug)]
struct ChunkCpu {
    /// The columns, laid out for the chunk's capacity.
    bytes: Vec<u8>,
    /// Validity bits, laid out for the chunk's capacity (see the module
    /// docs).
    validity: Vec<u32>,
}

/// A chunk's validity buffer on one context.
#[derive(Debug)]
struct ValidityGpu {
    buffer: wgpu::Buffer,
    /// The capacity the buffer was sized for.
    capacity: u32,
    /// Rows whose bits are in the buffer.
    rows: u32,
}

/// A chunk's buffers on one context.
#[derive(Debug)]
struct ChunkGpu {
    context: ContextId,
    buffer: wgpu::Buffer,
    /// The capacity the buffer was laid out for.
    capacity: u32,
    /// Rows written to the buffer; rows `rows..len` are the dirty tail.
    rows: u32,
    /// Validity bits, once the store has a null.
    validity: Option<ValidityGpu>,
}

/// Up to [`ColumnStore::chunk_rows`] rows: one GPU buffer, one instanced
/// draw, and per column an f64 origin and stats.
#[derive(Debug)]
pub struct Chunk {
    /// Index of the chunk's first row in the store.
    row_base: u64,
    /// Rows filled.
    len: u32,
    /// Rows the column sub-ranges have room for (at most `chunk_rows`).
    capacity: u32,
    columns: Vec<ChunkColumn>,
    /// The CPU copy, until released.
    cpu: Option<ChunkCpu>,
    gpu: Option<ChunkGpu>,
}

impl Chunk {
    fn new(formats: &[ColumnFormat], planes: usize, row_base: u64, capacity: u32) -> Self {
        let (offsets, size) = layout(formats, capacity);
        Self {
            row_base,
            len: 0,
            capacity,
            columns: formats
                .iter()
                .zip(offsets)
                .map(|(&format, offset)| ChunkColumn {
                    format,
                    offset,
                    origin: None,
                    stats: Acc::EMPTY,
                })
                .collect(),
            cpu: Some(ChunkCpu {
                bytes: vec![0; size as usize],
                validity: vec![0; validity_words(capacity, planes)],
            }),
            gpu: None,
        }
    }

    /// Index of the chunk's first row in the store.
    pub fn row_base(&self) -> u64 {
        self.row_base
    }

    /// Rows in the chunk.
    pub fn rows(&self) -> u32 {
        self.len
    }

    /// Rows the chunk's buffer has room for before it grows.
    pub fn capacity(&self) -> u32 {
        self.capacity
    }

    /// The chunk's columns: format, origin and stats.
    pub fn columns(&self) -> &[ChunkColumn] {
        &self.columns
    }

    /// The chunk's bytes as uploaded: each column at its 256-aligned
    /// offset, laid out for [`capacity`](Self::capacity) rows. `None` once
    /// [`ColumnStore::release`] has dropped the CPU copy.
    pub fn bytes(&self) -> Option<&[u8]> {
        self.cpu.as_ref().map(|c| c.bytes.as_slice())
    }

    /// The chunk's validity bits (see the [module docs](self)), laid out
    /// for [`capacity`](Self::capacity) rows. `None` once released.
    pub fn validity(&self) -> Option<&[u32]> {
        self.cpu.as_ref().map(|c| c.validity.as_slice())
    }

    /// Whether any numeric column of the chunk has a null.
    pub fn has_nulls(&self) -> bool {
        self.columns
            .iter()
            .any(|c| c.format.has_validity() && c.nulls() > 0)
    }

    /// The byte range of column `index`'s filled rows in the chunk buffer.
    pub(crate) fn column_range(&self, index: usize) -> std::ops::Range<u64> {
        let col = &self.columns[index];
        col.offset..col.offset + u64::from(self.len) * col.format.stride()
    }

    /// The chunk's buffer on `cx`, if [`ColumnStore::upload`] put it there.
    pub(crate) fn buffer(&self, cx: &Context) -> Option<&wgpu::Buffer> {
        self.gpu
            .as_ref()
            .filter(|g| g.context == cx.id() && g.rows == self.len)
            .map(|g| &g.buffer)
    }

    /// The chunk's validity buffer on `cx`, if [`ColumnStore::upload`] put
    /// it there (only once the store has a null).
    pub(crate) fn validity_buffer(&self, cx: &Context) -> Option<&wgpu::Buffer> {
        self.gpu
            .as_ref()
            .filter(|g| g.context == cx.id())
            .and_then(|g| g.validity.as_ref())
            .filter(|v| v.rows == self.len)
            .map(|v| &v.buffer)
    }

    /// Lay the chunk out for `capacity` rows, keeping its filled rows.
    fn grow(&mut self, formats: &[ColumnFormat], planes: usize, capacity: u32) {
        let (offsets, size) = layout(formats, capacity);
        let len = self.len;
        let cpu = self.cpu.as_mut().expect("only full chunks are released");
        let mut bytes = vec![0; size as usize];
        for (col, offset) in self.columns.iter_mut().zip(offsets) {
            let n = (u64::from(len) * col.format.stride()) as usize;
            bytes[offset as usize..][..n].copy_from_slice(&cpu.bytes[col.offset as usize..][..n]);
            col.offset = offset;
        }
        cpu.bytes = bytes;
        // Row groups are whole words, so growing only extends the array.
        cpu.validity.resize(validity_words(capacity, planes), 0);
        self.capacity = capacity;
    }

    /// Store rows `range` of `columns` as the chunk's rows `len..`,
    /// encoding keys through `dictionaries`.
    fn write(
        &mut self,
        columns: &[ColumnData<'_>],
        range: std::ops::Range<usize>,
        dictionaries: &mut [Option<Dictionary>],
        planes: usize,
    ) -> Result<()> {
        let at = self.len;
        let cpu = self.cpu.as_mut().expect("only full chunks are released");
        let mut plane = 0;
        for ((col, data), dictionary) in self.columns.iter_mut().zip(columns).zip(dictionaries) {
            match data {
                ColumnData::Values(values) => {
                    let values = &values[range.clone()];
                    col.write(&mut cpu.bytes, at, values);
                    for (i, v) in values.iter().enumerate() {
                        if v.is_finite() {
                            let row = at as usize + i;
                            cpu.validity[row / 32 * planes + plane] |= 1 << (row % 32);
                        }
                    }
                    plane += 1;
                }
                ColumnData::Keys(keys) => {
                    let dictionary = dictionary.as_mut().expect("U32 columns have a dictionary");
                    let codes = keys[range.clone()]
                        .iter()
                        .map(|k| k.map_or(Ok(NULL_CODE), |k| dictionary.encode(k)))
                        .collect::<Result<Vec<_>>>()?;
                    col.write_codes(&mut cpu.bytes, at, &codes);
                }
            }
        }
        Ok(())
    }

    /// Whether [`upload`](Self::upload) to `cx` can bring this chunk up to
    /// date: it has its CPU copy, or its rows are already there.
    fn uploadable_to(&self, cx: &Context) -> bool {
        self.cpu.is_some() || self.buffer(cx).is_some()
    }

    /// Bring the chunk's buffer on `cx` up to date: create it once per
    /// context, then write only the rows it lacks (all of them the first
    /// time), column by column, so the column bytes written are always
    /// rows × stride and never padding. A grown chunk first moves its
    /// uploaded rows into the larger buffer on the GPU. With `planes`
    /// (the store has a null), also bring its validity buffer up to date.
    fn upload(
        &mut self,
        cx: &Context,
        index: usize,
        formats: &[ColumnFormat],
        planes: Option<usize>,
    ) -> Result<()> {
        let has_nulls = self.has_nulls();
        if !self.uploadable_to(cx) {
            return Err(Error::config(
                "column store",
                format!(
                    "chunk {index} (rows {}..{}) released its CPU copy after uploading to \
                     another context (Retain::GpuOnly or Retain::Rows); it cannot be uploaded to \
                     context {:?}",
                    self.row_base,
                    self.row_base + u64::from(self.len),
                    cx.id()
                ),
            ));
        }
        let gpu = match &mut self.gpu {
            Some(gpu) if gpu.context == cx.id() => {
                if gpu.capacity != self.capacity {
                    let size = layout(formats, self.capacity).1;
                    let buffer = chunk_buffer(cx, size);
                    let (old, _) = layout(formats, gpu.capacity);
                    let mut encoder =
                        cx.device()
                            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                                label: Some("gup column chunk growth"),
                            });
                    for (col, from) in self.columns.iter().zip(old) {
                        let n = u64::from(gpu.rows) * col.format.stride();
                        if n > 0 {
                            encoder.copy_buffer_to_buffer(
                                &gpu.buffer,
                                from,
                                &buffer,
                                col.offset,
                                n,
                            );
                        }
                    }
                    cx.submit([encoder.finish()]);
                    gpu.buffer = buffer;
                    gpu.capacity = self.capacity;
                }
                gpu
            }
            gpu => gpu.insert(ChunkGpu {
                context: cx.id(),
                buffer: chunk_buffer(cx, layout(formats, self.capacity).1),
                capacity: self.capacity,
                rows: 0,
                validity: None,
            }),
        };
        if gpu.rows < self.len {
            let cpu = self.cpu.as_ref().expect("checked by uploadable_to");
            for col in &self.columns {
                let stride = col.format.stride();
                let at = col.offset + u64::from(gpu.rows) * stride;
                let end = col.offset + u64::from(self.len) * stride;
                // The only place column bytes are written (counted as
                // `Upload::Column`).
                cx.write_buffer(
                    Upload::Column,
                    &gpu.buffer,
                    at,
                    &cpu.bytes[at as usize..end as usize],
                );
            }
            gpu.rows = self.len;
        }
        let Some(planes) = planes else {
            return Ok(());
        };
        let validity = match &mut gpu.validity {
            Some(v) if v.capacity == self.capacity => v,
            // New, or the chunk grew: a new buffer, written in full below.
            // Only an unfilled chunk grows, and it keeps its CPU copy.
            slot => slot.insert(ValidityGpu {
                buffer: validity_buffer(cx, validity_words(self.capacity, planes)),
                capacity: self.capacity,
                rows: 0,
            }),
        };
        if validity.rows < self.len {
            // From the group holding the first missing row, so a tail
            // write rewrites the last partial word of each plane.
            let from = (validity.rows / 32) as usize * planes;
            let to = validity_words(self.len, planes);
            let words: Cow<'_, [u32]> = match &self.cpu {
                Some(cpu) => Cow::Borrowed(&cpu.validity[from..to]),
                // A released chunk had no null when the store had none
                // (else its bits were uploaded before release): all valid.
                None if !has_nulls => Cow::Owned(vec![u32::MAX; to - from]),
                None => unreachable!("chunks with nulls release after their bits upload"),
            };
            cx.write_buffer(
                Upload::Validity,
                &validity.buffer,
                from as u64 * 4,
                bytemuck::cast_slice(&words),
            );
            validity.rows = self.len;
        }
        Ok(())
    }

    /// Drop the CPU copy if the chunk is full (`chunk_rows` rows) and its
    /// rows, and any nulls' bits, are uploaded. Returns whether it did.
    fn release(&mut self, chunk_rows: u32) -> bool {
        let uploaded = self.gpu.as_ref().is_some_and(|g| {
            g.rows == self.len
                && (!self.has_nulls() || g.validity.as_ref().is_some_and(|v| v.rows == self.len))
        });
        if self.cpu.is_some() && self.len == chunk_rows && uploaded {
            self.cpu = None;
            return true;
        }
        false
    }
}

/// A chunk buffer: `VERTEX | STORAGE | COPY_DST | COPY_SRC`, so the same
/// bytes serve instanced draws, (from S9) compute passes and growth.
fn chunk_buffer(cx: &Context, size: u64) -> wgpu::Buffer {
    cx.device().create_buffer(&wgpu::BufferDescriptor {
        label: Some("gup column chunk"),
        size,
        usage: wgpu::BufferUsages::VERTEX
            | wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_DST
            | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    })
}

/// A chunk's validity buffer of `words` words, bound read-only as storage
/// in the vertex stage.
fn validity_buffer(cx: &Context, words: usize) -> wgpu::Buffer {
    cx.device().create_buffer(&wgpu::BufferDescriptor {
        label: Some("gup column validity"),
        size: (words as u64 * 4).max(4),
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_DST
            | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    })
}

/// A column store split into chunks.
#[derive(Debug)]
pub struct ColumnStore {
    rows: u64,
    chunk_rows: u32,
    formats: Vec<ColumnFormat>,
    chunks: Vec<Chunk>,
    /// One per column: a dictionary for each `U32` column.
    dictionaries: Vec<Option<Dictionary>>,
}

/// Column sub-ranges are aligned for both vertex fetch and storage binding.
const COLUMN_ALIGN: u64 = 256;

/// Rows per 256-byte block of a 4-byte column. Chunk capacities are whole
/// blocks (or `chunk_rows`), so the padding a column needs anyway is room
/// to append into.
const ROW_BLOCK: u32 = (COLUMN_ALIGN / 4) as u32;

fn align_up(v: u64, a: u64) -> u64 {
    v.div_ceil(a) * a
}

/// Column offsets and the buffer size of a chunk with room for
/// `capacity` rows.
fn layout(formats: &[ColumnFormat], capacity: u32) -> (Vec<u64>, u64) {
    let mut offset = 0;
    let offsets = formats
        .iter()
        .map(|f| {
            let at = offset;
            offset = align_up(offset + u64::from(capacity) * f.stride(), COLUMN_ALIGN);
            at
        })
        .collect();
    (offsets, offset.max(COLUMN_ALIGN))
}

impl ColumnStore {
    /// The most rows a chunk holds, whatever the device allows (RFC-001
    /// §3).
    pub const MAX_CHUNK_ROWS: u32 = 1 << 20;

    /// Rows per chunk for columns of `formats` on a device with `limits`:
    /// `min(2^20, max_buffer_size / Σ stride)`, rounded down to whole
    /// 256-byte column blocks so a full chunk needs no padding. (Validity
    /// bits are a separate, 128× smaller buffer.)
    pub fn chunk_rows_for(limits: &wgpu::Limits, formats: &[ColumnFormat]) -> u32 {
        let stride: u64 = formats.iter().map(|f| f.stride()).sum();
        let by_size = limits
            .max_buffer_size
            .checked_div(stride)
            .unwrap_or(u64::MAX);
        let rows = by_size.min(u64::from(Self::MAX_CHUNK_ROWS));
        (rows / u64::from(ROW_BLOCK) * u64::from(ROW_BLOCK)).max(u64::from(ROW_BLOCK)) as u32
    }

    /// An empty store of columns in `formats`, `chunk_rows` rows per chunk.
    pub fn new(formats: Vec<ColumnFormat>, chunk_rows: u32) -> Result<Self> {
        if chunk_rows == 0 {
            return Err(Error::config("column store", "chunk_rows must be positive"));
        }
        Ok(Self {
            rows: 0,
            chunk_rows,
            dictionaries: formats
                .iter()
                .map(|f| (*f == ColumnFormat::U32).then(Dictionary::new))
                .collect(),
            formats,
            chunks: Vec::new(),
        })
    }

    /// Build a store from evaluated accessor outputs, one
    /// `(format, data)` pair per column, `chunk_rows` rows per chunk.
    /// All columns must have the same length.
    pub fn from_columns<'a, D: Into<ColumnData<'a>>>(
        columns: Vec<(ColumnFormat, D)>,
        chunk_rows: u32,
    ) -> Result<Self> {
        let (formats, data): (Vec<_>, Vec<ColumnData<'a>>) =
            columns.into_iter().map(|(f, d)| (f, d.into())).unzip();
        let rows = data.first().map_or(0, ColumnData::len);
        let mut store = Self::new(formats, chunk_rows)?;
        store.append(rows, &data)?;
        Ok(store)
    }

    /// Validity planes: the number of numeric columns.
    pub fn validity_planes(&self) -> usize {
        self.formats.iter().filter(|f| f.has_validity()).count()
    }

    /// Append `rows` rows, given as one [`ColumnData`] of `rows` entries
    /// per column (none for a store without columns, whose rows only
    /// count instances). Fills the last chunk, growing its buffer, then
    /// opens new chunks with fresh origins and stats; full chunks are
    /// never touched. Keys are encoded through each column's append-only
    /// dictionary. The next [`upload`](Self::upload) writes only the new
    /// rows' bytes.
    pub fn append(&mut self, rows: usize, columns: &[ColumnData<'_>]) -> Result<()> {
        if columns.len() != self.formats.len() {
            return Err(Error::config(
                "column store",
                format!(
                    "{} columns appended to a store of {}",
                    columns.len(),
                    self.formats.len()
                ),
            ));
        }
        for (i, (c, format)) in columns.iter().zip(&self.formats).enumerate() {
            if c.len() != rows {
                return Err(Error::config(
                    "column store",
                    format!("column {i} has {} rows, {} expected", c.len(), rows),
                ));
            }
            if !c.fits(*format) {
                return Err(Error::config(
                    "column store",
                    format!(
                        "column {i} is {format:?} but was given {}",
                        match c {
                            ColumnData::Values(_) => "numbers",
                            ColumnData::Keys(_) => "keys",
                        }
                    ),
                ));
            }
        }
        let planes = self.validity_planes();
        let n = rows;
        let mut done = 0;
        while done < n {
            if self
                .chunks
                .last()
                .is_none_or(|last| last.len == self.chunk_rows)
            {
                let take = (n - done).min(self.chunk_rows as usize) as u32;
                let capacity = take.next_multiple_of(ROW_BLOCK).min(self.chunk_rows);
                self.chunks
                    .push(Chunk::new(&self.formats, planes, self.rows, capacity));
            }
            let chunk = self.chunks.last_mut().expect("at least one chunk");
            let take = (n - done).min((self.chunk_rows - chunk.len) as usize) as u32;
            if chunk.len + take > chunk.capacity {
                // Double, so a stream of small appends copies each row
                // O(1) times on average.
                let capacity = (chunk.len + take)
                    .next_multiple_of(ROW_BLOCK)
                    .max(chunk.capacity.saturating_mul(2))
                    .min(self.chunk_rows);
                chunk.grow(&self.formats, planes, capacity);
            }
            chunk.write(
                columns,
                done..done + take as usize,
                &mut self.dictionaries,
                planes,
            )?;
            chunk.len += take;
            self.rows += u64::from(take);
            done += take as usize;
        }
        Ok(())
    }

    /// Total rows across every chunk.
    pub fn rows(&self) -> u64 {
        self.rows
    }

    /// Rows per chunk (every chunk but the last is full).
    pub fn chunk_rows(&self) -> u32 {
        self.chunk_rows
    }

    /// The storage format of each column.
    pub fn formats(&self) -> &[ColumnFormat] {
        &self.formats
    }

    /// The chunks, in row order: per-chunk origins, stats and row counts.
    pub fn chunks(&self) -> &[Chunk] {
        &self.chunks
    }

    /// Statistics of column `index` across every chunk; `None` if it has
    /// no non-null value.
    pub fn stats(&self, index: usize) -> Option<ColumnStats> {
        self.chunks
            .iter()
            .map(|c| c.columns[index].stats)
            .fold(Acc::EMPTY, Acc::merge)
            .stats()
    }

    /// The dictionary of column `index`, if it is a
    /// [`U32`](ColumnFormat::U32) column. Kept by every retention policy.
    pub fn dictionary(&self, index: usize) -> Option<&Dictionary> {
        self.dictionaries.get(index)?.as_ref()
    }

    /// Whether any numeric column has a null, so the store uploads (and
    /// draws read) validity bits.
    pub fn has_nulls(&self) -> bool {
        self.chunks.iter().any(Chunk::has_nulls)
    }

    /// Whether [`upload`](Self::upload) to `cx` can succeed: every chunk
    /// either kept its CPU copy or is already on `cx`.
    pub fn uploadable_to(&self, cx: &Context) -> bool {
        self.chunks.iter().all(|c| c.uploadable_to(cx))
    }

    /// Bring every chunk's buffers on `cx` up to date. The first upload to
    /// a context writes each chunk once; after an
    /// [`append`](Self::append) it writes only the appended rows, at each
    /// column's tail. Every write is counted as [`Upload::Column`] (or
    /// [`Upload::Validity`] for null bits, once the store has a null).
    ///
    /// A chunk [released](Self::release) on one context cannot be uploaded
    /// to another: that is an error.
    pub fn upload(&mut self, cx: &Context) -> Result<()> {
        let max = cx.caps().limits.max_buffer_size;
        let planes = self.has_nulls().then(|| self.validity_planes());
        for (k, chunk) in self.chunks.iter_mut().enumerate() {
            let size = layout(&self.formats, chunk.capacity).1;
            if size > max {
                return Err(Error::config(
                    "column store",
                    format!(
                        "chunk of {size} bytes exceeds the device's max_buffer_size ({max}); \
                         rebuild the store with ColumnStore::chunk_rows_for this device"
                    ),
                ));
            }
            chunk.upload(cx, k, &self.formats, planes)?;
        }
        Ok(())
    }

    /// Drop the CPU copy of every full chunk whose rows (and null bits)
    /// are uploaded, keeping its stats and origins; the store's
    /// dictionaries stay too. The last, unfilled chunk keeps its copy so
    /// appends can fill it. Returns the number of chunks released by this
    /// call.
    pub fn release(&mut self) -> usize {
        let chunk_rows = self.chunk_rows;
        self.chunks
            .iter_mut()
            .filter_map(|c| c.release(chunk_rows).then_some(()))
            .count()
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    const ALL: u32 = ColumnStore::MAX_CHUNK_ROWS;

    fn read(chunk: &Chunk, col: usize, row: usize) -> f32 {
        let at = chunk.columns[col].offset as usize + row * 4;
        f32::from_le_bytes(chunk.bytes().unwrap()[at..at + 4].try_into().unwrap())
    }

    #[test]
    fn relative_columns_keep_precision_for_large_values() {
        // Unix seconds a day apart: absolute f32 has a 128 s ULP here.
        let t0 = 1.7e9;
        let values: Vec<f64> = (0..5).map(|i| t0 + f64::from(i) * 0.25).collect();
        let store = ColumnStore::from_columns(
            vec![
                (ColumnFormat::F32Relative, values.clone()),
                (ColumnFormat::F32, values.clone()),
            ],
            ALL,
        )
        .unwrap();
        let [chunk] = store.chunks() else { panic!() };
        let rel = &chunk.columns()[0];
        let abs = &chunk.columns()[1];
        assert_eq!(rel.origin(), t0);
        assert_eq!(abs.origin(), 0.0);
        assert_eq!(rel.offset % COLUMN_ALIGN, 0);
        assert_eq!(abs.offset % COLUMN_ALIGN, 0);
        for (i, v) in values.iter().enumerate() {
            // Relative: exact to f32 precision of the small offset.
            assert_eq!(f64::from(read(chunk, 0, i)) + rel.origin(), *v);
        }
        // Absolute f32 collapses all five values to one.
        assert_eq!(read(chunk, 1, 0), read(chunk, 1, 4));
        assert_eq!(rel.stats().unwrap().extent(), (t0, t0 + 1.0));
    }

    #[test]
    fn stats_skip_non_finite_values() {
        let store = ColumnStore::from_columns(
            vec![(ColumnFormat::F32, vec![f64::NAN, 3.0, -1.0, 2.0])],
            ALL,
        )
        .unwrap();
        let s = store.stats(0).unwrap();
        assert_eq!((s.min, s.max, s.non_finite), (-1.0, 3.0, 1));
    }

    #[test]
    fn mismatched_lengths_are_a_configuration_error() {
        let err = ColumnStore::from_columns(
            vec![
                (ColumnFormat::F32, vec![1.0, 2.0]),
                (ColumnFormat::F32, vec![1.0]),
            ],
            ALL,
        )
        .unwrap_err();
        assert!(
            err.to_string().contains("column 1 has 1 rows, 2 expected"),
            "{err}"
        );
    }

    /// The S0a single-chunk layout, verbatim: each column at
    /// `align_up(rows × 4, 256)` steps, `(v - first finite) as f32` for
    /// relative columns, at least 256 bytes.
    fn s0a_bytes(columns: &[(ColumnFormat, Vec<f64>)]) -> Vec<u8> {
        let rows = columns[0].1.len() as u64;
        let mut offset = 0;
        let mut meta = Vec::new();
        for (format, _) in columns {
            meta.push(offset);
            offset = align_up(offset + rows * format.stride(), COLUMN_ALIGN);
        }
        let mut bytes = vec![0u8; offset.max(COLUMN_ALIGN) as usize];
        for ((format, values), offset) in columns.iter().zip(meta) {
            let origin = match format {
                ColumnFormat::F32 | ColumnFormat::U32 => 0.0,
                ColumnFormat::F32Relative => values
                    .iter()
                    .copied()
                    .find(|v| v.is_finite())
                    .unwrap_or(0.0),
            };
            let dst = &mut bytes[offset as usize..][..values.len() * 4];
            for (cell, v) in dst.chunks_exact_mut(4).zip(values) {
                cell.copy_from_slice(&((v - origin) as f32).to_le_bytes());
            }
        }
        bytes
    }

    /// AC1: data that fits one chunk gives today's single-chunk store:
    /// the same bytes, stats and origins, whatever the chunk size.
    #[test]
    fn one_chunk_matches_the_s0a_store() {
        for rows in [1usize, 63, 64, 65, 120, 1000, 4096] {
            let columns: Vec<(ColumnFormat, Vec<f64>)> = vec![
                (
                    ColumnFormat::F32Relative,
                    (0..rows).map(|i| 1.7e9 + i as f64 * 37.5).collect(),
                ),
                (
                    ColumnFormat::F32,
                    (0..rows).map(|i| (i as f64).powi(2) + 1.0).collect(),
                ),
                (
                    ColumnFormat::F32,
                    (0..rows)
                        .map(|i| if i % 7 == 3 { f64::NAN } else { i as f64 })
                        .collect(),
                ),
            ];
            for chunk_rows in [rows as u32, 4096, ALL] {
                let store = ColumnStore::from_columns(columns.clone(), chunk_rows).unwrap();
                let [chunk] = store.chunks() else {
                    panic!("{rows} rows in chunks of {chunk_rows}: {store:?}")
                };
                assert_eq!(store.rows(), rows as u64);
                assert_eq!(chunk.bytes().unwrap(), s0a_bytes(&columns), "{rows} rows");
                for (k, (format, values)) in columns.iter().enumerate() {
                    let col = &chunk.columns()[k];
                    let first = values.iter().copied().find(|v| v.is_finite()).unwrap();
                    let origin = if *format == ColumnFormat::F32 {
                        0.0
                    } else {
                        first
                    };
                    assert_eq!(col.origin(), origin);
                    let finite = values.iter().copied().filter(|v| v.is_finite());
                    assert_eq!(
                        col.stats(),
                        Some(ColumnStats {
                            min: finite.clone().fold(f64::INFINITY, f64::min),
                            max: finite.fold(f64::NEG_INFINITY, f64::max),
                            non_finite: values.iter().filter(|v| !v.is_finite()).count() as u32,
                        })
                    );
                    assert_eq!(col.stats(), store.stats(k));
                }
            }
        }
    }

    /// AC1: a forced small chunk size splits the rows; every chunk has its
    /// own origin (its first value) and stats, and the store's stats are
    /// the union.
    #[test]
    fn small_chunks_split_rows_with_their_own_origins_and_stats() {
        let values: Vec<f64> = (0..250).map(|i| 1.7e9 + f64::from(i) * 10.0).collect();
        let store =
            ColumnStore::from_columns(vec![(ColumnFormat::F32Relative, values.clone())], 64)
                .unwrap();
        assert_eq!(store.rows(), 250);
        let lens: Vec<_> = store.chunks().iter().map(Chunk::rows).collect();
        assert_eq!(lens, [64, 64, 64, 58]);
        for (k, chunk) in store.chunks().iter().enumerate() {
            let first = values[k * 64];
            let last = values[(k * 64 + 63).min(249)];
            assert_eq!(chunk.row_base(), k as u64 * 64);
            assert_eq!(chunk.columns()[0].origin(), first);
            assert_eq!(chunk.columns()[0].stats().unwrap().extent(), (first, last));
            // Relative to its own origin: small offsets, stored exactly.
            for i in 0..chunk.rows() as usize {
                let v = values[k * 64 + i];
                assert_eq!(f64::from(read(chunk, 0, i)) + first, v);
            }
        }
        assert_eq!(store.stats(0).unwrap().extent(), (values[0], values[249]));
        // A chunk size that is not a whole number of 64-row blocks.
        let store =
            ColumnStore::from_columns(vec![(ColumnFormat::F32, values.clone())], 40).unwrap();
        let lens: Vec<_> = store.chunks().iter().map(Chunk::rows).collect();
        assert_eq!(lens, [40, 40, 40, 40, 40, 40, 10]);
        assert!(store.chunks().iter().all(|c| c.capacity() == 40));
    }

    #[test]
    fn chunk_rows_follow_the_device_limit() {
        let mut limits = wgpu::Limits::default();
        let three = [ColumnFormat::F32; 3];
        assert_eq!(ColumnStore::chunk_rows_for(&limits, &three), 1 << 20);
        // 1 MiB buffers: 1 MiB / 12 B = 87381 rows, down to whole blocks.
        limits.max_buffer_size = 1 << 20;
        assert_eq!(ColumnStore::chunk_rows_for(&limits, &three), 87_360);
        assert_eq!(ColumnStore::chunk_rows_for(&limits, &[]), 1 << 20);
    }

    /// Appends fill the tail, grow the last chunk, then open new chunks
    /// with fresh origins; stats update incrementally.
    #[test]
    fn append_fills_grows_then_opens_chunks() {
        let mut store = ColumnStore::new(vec![ColumnFormat::F32Relative], 256).unwrap();
        let mut all = Vec::new();
        for batch in 0..10 {
            let values: Vec<f64> = (0..50)
                .map(|i| 1.6e9 + f64::from(batch * 50 + i) * 0.5)
                .collect();
            store
                .append(values.len(), &[ColumnData::Values(values.clone())])
                .unwrap();
            all.extend(values);
        }
        assert_eq!(store.rows(), 500);
        let lens: Vec<_> = store.chunks().iter().map(Chunk::rows).collect();
        assert_eq!(lens, [256, 244]);
        assert_eq!(store.chunks()[1].row_base(), 256);
        assert_eq!(store.chunks()[1].columns()[0].origin(), all[256]);
        assert_eq!(store.stats(0).unwrap().extent(), (all[0], all[499]));
        for (k, chunk) in store.chunks().iter().enumerate() {
            let origin = chunk.columns()[0].origin();
            for i in 0..chunk.rows() as usize {
                assert_eq!(f64::from(read(chunk, 0, i)) + origin, all[k * 256 + i]);
            }
        }
        // A NaN-only start takes its origin from the first finite append.
        let mut store = ColumnStore::new(vec![ColumnFormat::F32Relative], 256).unwrap();
        store
            .append(1, &[ColumnData::Values(vec![f64::NAN])])
            .unwrap();
        store
            .append(2, &[ColumnData::Values(vec![1.7e9, 1.7e9 + 0.25])])
            .unwrap();
        let col = &store.chunks()[0].columns()[0];
        assert_eq!(col.origin(), 1.7e9);
        assert!(read(&store.chunks()[0], 0, 0).is_nan());
        assert_eq!(read(&store.chunks()[0], 0, 2), 0.25);
        assert_eq!(col.stats().unwrap().non_finite, 1);
    }

    /// Read a buffer back from the GPU.
    fn read_buffer(cx: &Context, buffer: &wgpu::Buffer) -> Vec<u8> {
        let readback = cx.device().create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: buffer.size(),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut enc = cx.device().create_command_encoder(&Default::default());
        enc.copy_buffer_to_buffer(buffer, 0, &readback, 0, buffer.size());
        cx.submit([enc.finish()]);
        readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, |r| r.unwrap());
        cx.wait_idle().unwrap();
        readback.slice(..).get_mapped_range().to_vec()
    }

    #[test]
    fn upload_writes_the_chunk_once_per_context() {
        let cx = Context::new_blocking().unwrap();
        let mut store = ColumnStore::from_columns(
            vec![
                (ColumnFormat::F32Relative, vec![10.0, 11.0, 12.5]),
                (ColumnFormat::F32, vec![1.0, 2.0, 3.0]),
            ],
            ALL,
        )
        .unwrap();
        store.upload(&cx).unwrap();
        let buffer = store.chunks()[0].buffer(&cx).unwrap().clone();
        assert!(buffer.usage().contains(
            wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::STORAGE
        ));
        let bytes = read_buffer(&cx, &buffer);
        let f = |at: usize| f32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!([f(0), f(4), f(8)], [0.0, 1.0, 2.5]);
        assert_eq!([f(256), f(260), f(264)], [1.0, 2.0, 3.0]);
        // A second upload to the same context reuses the buffer.
        let before = cx.upload_stats();
        store.upload(&cx).unwrap();
        assert_eq!(store.chunks()[0].buffer(&cx).unwrap(), &buffer);
        assert_eq!(cx.upload_stats(), before);
    }

    /// AC5: appends write exactly the appended rows' bytes, at each
    /// column's tail; growth moves rows on the GPU; full chunks are never
    /// written again; the GPU bytes match the CPU copy.
    #[test]
    fn append_uploads_only_the_new_rows() {
        let cx = Context::new_blocking().unwrap();
        let formats = [ColumnFormat::F32Relative, ColumnFormat::F32];
        let batch = |b: u32, n: u32| -> Vec<Vec<f64>> {
            let rows = (0..n).map(|i| f64::from(b * 1000 + i));
            vec![rows.clone().map(|v| 1.7e9 + v).collect(), rows.collect()]
        };
        let mut store =
            ColumnStore::from_columns(formats.iter().copied().zip(batch(0, 100)).collect(), 192)
                .unwrap();
        store.upload(&cx).unwrap();
        let first = store.chunks()[0].buffer(&cx).unwrap().clone();
        let submits = cx.submissions();
        let start = cx.upload_stats();
        let mut appended = 0u64;
        for (b, n) in [(1, 20), (2, 10), (3, 100), (4, 300), (5, 7)] {
            let data: Vec<_> = batch(b, n).into_iter().map(ColumnData::Values).collect();
            store.append(n as usize, &data).unwrap();
            store.upload(&cx).unwrap();
            appended += u64::from(n);
        }
        let written = cx.upload_stats() - start;
        assert_eq!(store.rows(), 100 + appended);
        assert_eq!(written.columns.bytes, appended * 8, "{written:?}");
        // No nulls: no validity buffer, no validity bytes.
        assert_eq!(written.validity, Default::default());
        let lens: Vec<_> = store.chunks().iter().map(Chunk::rows).collect();
        assert_eq!(lens, [192, 192, 153]);
        // The first chunk grew (128 → 192 rows) once, on the GPU.
        assert_ne!(store.chunks()[0].buffer(&cx).unwrap(), &first);
        assert_eq!(cx.submissions() - submits, 2, "chunks 0 and 1 grew");
        for chunk in store.chunks() {
            let gpu = read_buffer(&cx, chunk.buffer(&cx).unwrap());
            for k in 0..formats.len() {
                let r = chunk.column_range(k);
                assert_eq!(
                    gpu[r.start as usize..r.end as usize],
                    chunk.bytes().unwrap()[r.start as usize..r.end as usize]
                );
            }
        }
    }

    /// Bit `row` of validity plane `plane` of a chunk with `planes` planes.
    fn bit(chunk: &Chunk, planes: usize, plane: usize, row: usize) -> bool {
        (chunk.validity().unwrap()[row / 32 * planes + plane] >> (row % 32)) & 1 == 1
    }

    /// The codes stored in column `col` of every chunk, in row order.
    fn codes(store: &ColumnStore, col: usize) -> Vec<u32> {
        store
            .chunks()
            .iter()
            .flat_map(|c| {
                let at = c.columns[col].offset as usize;
                let bytes = &c.bytes().unwrap()[at..at + c.rows() as usize * 4];
                bytes
                    .chunks_exact(4)
                    .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    /// S4b: one validity bit per row and numeric column, set for finite
    /// values, one word per plane in each 32-row group; a dictionary column
    /// has no plane, and nulls are counted in the stats.
    #[test]
    fn validity_bits_mark_finite_values_per_numeric_column() {
        let n = 70;
        let x: Vec<f64> = (0..n)
            .map(|i| if i % 9 == 4 { f64::NAN } else { i as f64 })
            .collect();
        let y: Vec<f64> = (0..n)
            .map(|i| if i == 33 { f64::INFINITY } else { 1.0 })
            .collect();
        let keys: Vec<Option<&str>> = (0..n)
            .map(|i| (i % 5 != 0).then_some(["a", "b", "c"][i % 3]))
            .collect();
        let store = ColumnStore::from_columns(
            vec![
                (ColumnFormat::F32Relative, ColumnData::Values(x.clone())),
                (ColumnFormat::U32, ColumnData::Keys(keys)),
                (ColumnFormat::F32, ColumnData::Values(y.clone())),
            ],
            ALL,
        )
        .unwrap();
        assert_eq!(store.validity_planes(), 2);
        assert!(store.has_nulls());
        let [chunk] = store.chunks() else { panic!() };
        // Room for 128 rows: 4 groups of 2 words.
        assert_eq!(chunk.validity().unwrap().len(), 8);
        for row in 0..n {
            assert_eq!(bit(chunk, 2, 0, row), x[row].is_finite(), "x row {row}");
            assert_eq!(bit(chunk, 2, 1, row), y[row].is_finite(), "y row {row}");
        }
        // Rows past the end are unset.
        assert!(!bit(chunk, 2, 0, n) && !bit(chunk, 2, 1, 127));
        assert_eq!(store.stats(0).unwrap().non_finite, 8);
        assert_eq!(store.stats(1).unwrap().non_finite, 14);
        assert_eq!(store.stats(2).unwrap().non_finite, 1);

        // Without a non-finite value there is nothing to upload.
        let clean =
            ColumnStore::from_columns(vec![(ColumnFormat::F32, vec![1.0, 2.0])], ALL).unwrap();
        assert!(!clean.has_nulls());
    }

    /// S4b: keys get codes in first-seen order, across appends and chunks;
    /// a missing key is `NULL_CODE`, which is not a validity bit.
    #[test]
    fn dictionary_codes_follow_first_seen_order_and_never_renumber() {
        let mut store = ColumnStore::new(vec![ColumnFormat::U32], 64).unwrap();
        store
            .append(
                4,
                &[ColumnData::Keys(vec![
                    Some("Europe"),
                    Some("Asia"),
                    None,
                    Some("Europe"),
                ])],
            )
            .unwrap();
        assert_eq!(codes(&store, 0), [0, 1, NULL_CODE, 0]);
        let more: Vec<Option<&str>> = (0..70)
            .map(|i| Some(["Africa", "Asia", "Oceania"][i % 3]))
            .collect();
        store.append(70, &[ColumnData::Keys(more)]).unwrap();
        assert_eq!(store.chunks().len(), 2);
        let dict = store.dictionary(0).unwrap();
        assert_eq!(
            dict.keys().collect::<Vec<_>>(),
            ["Europe", "Asia", "Africa", "Oceania"]
        );
        assert_eq!((dict.code("Oceania"), dict.key(1)), (Some(3), Some("Asia")));
        assert_eq!(dict.key(NULL_CODE), None);
        let all = codes(&store, 0);
        assert_eq!(&all[..7], [0, 1, NULL_CODE, 0, 2, 1, 3]);
        assert_eq!(all[73], [2, 1, 3][69 % 3]);
        let stats = store.stats(0).unwrap();
        assert_eq!((stats.min, stats.max, stats.non_finite), (0.0, 3.0, 1));
        assert_eq!(store.validity_planes(), 0);
        assert!(!store.has_nulls(), "a missing key is a code, not a bit");

        let err = store
            .append(1, &[ColumnData::Values(vec![1.0])])
            .unwrap_err();
        assert!(
            err.to_string()
                .contains("column 0 is U32 but was given numbers"),
            "{err}"
        );
    }

    /// S4b: the validity buffer appears with the first null and is
    /// written in full; a later append rewrites only from the last partial
    /// group, unless the chunk grew (a new buffer, written in full). The
    /// GPU words always equal the CPU words.
    #[test]
    fn validity_uploads_with_the_first_null_then_only_the_tail() {
        let cx = Context::new_blocking().unwrap();
        let mut store =
            ColumnStore::new(vec![ColumnFormat::F32Relative, ColumnFormat::F32], 256).unwrap();
        let rows = |from: u32, n: u32, nan: Option<u32>| -> Vec<ColumnData<'static>> {
            let v: Vec<f64> = (from..from + n)
                .map(|i| {
                    if Some(i) == nan {
                        f64::NAN
                    } else {
                        f64::from(i)
                    }
                })
                .collect();
            vec![ColumnData::Values(v.clone()), ColumnData::Values(v)]
        };
        let step = |store: &mut ColumnStore, data: Vec<ColumnData<'_>>| {
            let n = data[0].len();
            store.append(n, &data).unwrap();
            let start = cx.upload_stats();
            store.upload(&cx).unwrap();
            cx.upload_stats() - start
        };
        let written = step(&mut store, rows(0, 40, None));
        assert_eq!(written.validity, Default::default(), "no null yet");
        assert!(store.chunks()[0].validity_buffer(&cx).is_none());

        // The first null: rows 0..50 in 2 groups of 2 words.
        let written = step(&mut store, rows(40, 10, Some(45)));
        assert_eq!((written.validity.bytes, written.validity.writes), (16, 1));
        assert_eq!(written.columns.bytes, 10 * 8);
        // 70 rows outgrow 64: a new buffer, written in full (3 groups).
        let written = step(&mut store, rows(50, 20, None));
        assert_eq!(written.validity.bytes, 24);
        // 75 rows: from group 2 (rows 64..), rewriting its partial words.
        let written = step(&mut store, rows(70, 5, Some(72)));
        assert_eq!(written.validity.bytes, 8);

        let chunk = &store.chunks()[0];
        let gpu = read_buffer(&cx, chunk.validity_buffer(&cx).unwrap());
        let words = validity_words(chunk.rows(), 2);
        assert_eq!(
            &gpu[..words * 4],
            bytemuck::cast_slice::<u32, u8>(&chunk.validity().unwrap()[..words])
        );
        for row in 0..75 {
            let null = row == 45 || row == 72;
            assert_eq!(bit(chunk, 2, 0, row), !null, "row {row}");
            assert_eq!(bit(chunk, 2, 1, row), !null, "row {row}");
        }
    }

    /// S4b: `release` drops the CPU copy of full, uploaded chunks only,
    /// keeping stats, dictionaries and the GPU buffers; appends still work
    /// on the same context, and uploading to another context is an error.
    #[test]
    fn release_drops_full_uploaded_chunks_only() {
        let cx = Context::new_blocking().unwrap();
        let values: Vec<f64> = (0..150)
            .map(|i| if i == 70 { f64::NAN } else { f64::from(i) })
            .collect();
        let keys: Vec<Option<&str>> = (0..150).map(|i| Some(["p", "q"][i % 2])).collect();
        let mut store = ColumnStore::from_columns(
            vec![
                (ColumnFormat::F32, ColumnData::Values(values)),
                (ColumnFormat::U32, ColumnData::Keys(keys)),
            ],
            64,
        )
        .unwrap();
        let stats = store.stats(0);
        assert_eq!(store.release(), 0, "nothing is uploaded yet");
        store.upload(&cx).unwrap();
        assert_eq!(store.release(), 2);
        let chunks = store.chunks();
        assert!(chunks[0].bytes().is_none() && chunks[1].validity().is_none());
        assert!(chunks[2].bytes().is_some(), "the last chunk keeps its copy");
        assert_eq!(store.stats(0), stats);
        assert_eq!(store.dictionary(1).unwrap().len(), 2);
        assert!(chunks[1].buffer(&cx).is_some() && chunks[1].validity_buffer(&cx).is_some());

        // Appending fills the last chunk and opens another; only the new
        // rows are written.
        let more: Vec<f64> = (150..200).map(f64::from).collect();
        let keys: Vec<Option<&str>> = vec![Some("r"); 50];
        store
            .append(50, &[ColumnData::Values(more), ColumnData::Keys(keys)])
            .unwrap();
        let start = cx.upload_stats();
        store.upload(&cx).unwrap();
        assert_eq!((cx.upload_stats() - start).columns.bytes, 50 * 8);
        assert_eq!(store.release(), 1);
        assert_eq!(store.dictionary(1).unwrap().code("r"), Some(2));

        let other = Context::from_wgpu(cx.device().clone(), cx.queue().clone());
        assert!(store.uploadable_to(&cx) && !store.uploadable_to(&other));
        let err = store.upload(&other).unwrap_err().to_string();
        assert!(
            err.contains("chunk 0 (rows 0..64) released its CPU copy"),
            "{err}"
        );
        assert!(store.chunks()[0].buffer(&cx).is_some(), "left in place");
    }
}

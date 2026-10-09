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
//! Retention policies, validity bits and dictionary encoding are RFC-001
//! S4b.

use crate::context::{Context, ContextId, Upload};
use crate::error::{Error, Result};

/// How a column's values are stored on the GPU.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum ColumnFormat {
    /// Absolute `f32`.
    F32,
    /// `f32` relative to the chunk's f64 origin.
    F32Relative,
}

impl ColumnFormat {
    /// Bytes per row.
    pub const fn stride(self) -> u64 {
        4
    }
}

/// f64 statistics of a column's finite values in one chunk, or (from
/// [`ColumnStore::stats`]) across every chunk. They drive automatic
/// domains (and, from RFC-001 S9, chunk culling).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ColumnStats {
    /// Smallest finite value.
    pub min: f64,
    /// Largest finite value.
    pub max: f64,
    /// Number of non-finite values (NaN or ±∞).
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

    /// Statistics of the finite values; `None` if there are none.
    pub fn stats(&self) -> Option<ColumnStats> {
        self.stats.stats()
    }

    /// Store `values` as rows `at..` of this column in `bytes` (the chunk's
    /// CPU copy), updating the origin and stats.
    fn write(&mut self, bytes: &mut [u8], at: u32, values: &[f64]) {
        if self.origin.is_none() {
            // Rows already stored are non-finite, so the origin they were
            // written against does not matter.
            self.origin = match self.format {
                ColumnFormat::F32 => Some(0.0),
                ColumnFormat::F32Relative => values.iter().copied().find(|v| v.is_finite()),
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
}

/// A chunk's buffer on one context.
#[derive(Debug)]
struct ChunkGpu {
    context: ContextId,
    buffer: wgpu::Buffer,
    /// The capacity the buffer was laid out for.
    capacity: u32,
    /// Rows written to the buffer; rows `rows..len` are the dirty tail.
    rows: u32,
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
    /// The CPU copy of the buffer, laid out for `capacity`.
    bytes: Vec<u8>,
    gpu: Option<ChunkGpu>,
}

impl Chunk {
    fn new(formats: &[ColumnFormat], row_base: u64, capacity: u32) -> Self {
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
            bytes: vec![0; size as usize],
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
    /// offset, laid out for [`capacity`](Self::capacity) rows.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
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

    /// Lay the chunk out for `capacity` rows, keeping its filled rows.
    fn grow(&mut self, formats: &[ColumnFormat], capacity: u32) {
        let (offsets, size) = layout(formats, capacity);
        let mut bytes = vec![0; size as usize];
        for (col, offset) in self.columns.iter_mut().zip(offsets) {
            let n = (u64::from(self.len) * col.format.stride()) as usize;
            bytes[offset as usize..][..n].copy_from_slice(&self.bytes[col.offset as usize..][..n]);
            col.offset = offset;
        }
        self.bytes = bytes;
        self.capacity = capacity;
    }

    /// Bring the chunk's buffer on `cx` up to date: create it once per
    /// context, then write only the rows it lacks (all of them the first
    /// time), column by column, so the column bytes written are always
    /// rows × stride and never padding. A grown chunk first moves its
    /// uploaded rows into the larger buffer on the GPU.
    fn upload(&mut self, cx: &Context, formats: &[ColumnFormat]) {
        let gpu = match &mut self.gpu {
            Some(gpu) if gpu.context == cx.id() => {
                if gpu.capacity != self.capacity {
                    let buffer = chunk_buffer(cx, self.bytes.len() as u64);
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
                buffer: chunk_buffer(cx, self.bytes.len() as u64),
                capacity: self.capacity,
                rows: 0,
            }),
        };
        if gpu.rows < self.len {
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
                    &self.bytes[at as usize..end as usize],
                );
            }
            gpu.rows = self.len;
        }
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

/// A column store split into chunks.
#[derive(Debug)]
pub struct ColumnStore {
    rows: u64,
    chunk_rows: u32,
    formats: Vec<ColumnFormat>,
    chunks: Vec<Chunk>,
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
    /// 256-byte column blocks so a full chunk needs no padding.
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
            formats,
            chunks: Vec::new(),
        })
    }

    /// Build a store from evaluated accessor outputs, one
    /// `(format, values)` pair per column, `chunk_rows` rows per chunk.
    /// All columns must have the same length.
    pub fn from_columns(columns: Vec<(ColumnFormat, Vec<f64>)>, chunk_rows: u32) -> Result<Self> {
        let rows = columns.first().map_or(0, |(_, v)| v.len());
        let (formats, values): (Vec<_>, Vec<_>) = columns.into_iter().unzip();
        let mut store = Self::new(formats, chunk_rows)?;
        store.append(rows, &values)?;
        Ok(store)
    }

    /// Append `rows` rows, given as one slice of `rows` values per column
    /// (none for a store without columns, whose rows only count
    /// instances). Fills the last chunk, growing its buffer, then opens
    /// new chunks with fresh origins and stats; full chunks are never
    /// touched. The next [`upload`](Self::upload) writes only the new
    /// rows' bytes.
    pub fn append(&mut self, rows: usize, columns: &[impl AsRef<[f64]>]) -> Result<()> {
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
        if let Some((i, c)) = columns
            .iter()
            .enumerate()
            .find(|(_, c)| c.as_ref().len() != rows)
        {
            return Err(Error::config(
                "column store",
                format!(
                    "column {i} has {} rows, {} expected",
                    c.as_ref().len(),
                    rows
                ),
            ));
        }
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
                    .push(Chunk::new(&self.formats, self.rows, capacity));
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
                chunk.grow(&self.formats, capacity);
            }
            let rows = done..done + take as usize;
            for (col, values) in chunk.columns.iter_mut().zip(columns) {
                col.write(&mut chunk.bytes, chunk.len, &values.as_ref()[rows.clone()]);
            }
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
    /// no finite value.
    pub fn stats(&self, index: usize) -> Option<ColumnStats> {
        self.chunks
            .iter()
            .map(|c| c.columns[index].stats)
            .fold(Acc::EMPTY, Acc::merge)
            .stats()
    }

    /// Bring every chunk's buffer on `cx` up to date. The first upload to
    /// a context writes each chunk once; after an
    /// [`append`](Self::append) it writes only the appended rows, at each
    /// column's tail. Every write is counted as [`Upload::Column`].
    pub fn upload(&mut self, cx: &Context) -> Result<()> {
        let max = cx.caps().limits.max_buffer_size;
        for chunk in &mut self.chunks {
            let size = chunk.bytes.len() as u64;
            if size > max {
                return Err(Error::config(
                    "column store",
                    format!(
                        "chunk of {size} bytes exceeds the device's max_buffer_size ({max}); \
                         rebuild the store with ColumnStore::chunk_rows_for this device"
                    ),
                ));
            }
            chunk.upload(cx, &self.formats);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: u32 = ColumnStore::MAX_CHUNK_ROWS;

    fn read(chunk: &Chunk, col: usize, row: usize) -> f32 {
        let at = chunk.columns[col].offset as usize + row * 4;
        f32::from_le_bytes(chunk.bytes[at..at + 4].try_into().unwrap())
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
                ColumnFormat::F32 => 0.0,
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
                assert_eq!(chunk.bytes(), s0a_bytes(&columns), "{rows} rows");
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
            store.append(values.len(), &[&values]).unwrap();
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
        store.append(1, &[[f64::NAN]]).unwrap();
        store.append(2, &[[1.7e9, 1.7e9 + 0.25]]).unwrap();
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
            store.append(n as usize, &batch(b, n)).unwrap();
            store.upload(&cx).unwrap();
            appended += u64::from(n);
        }
        let written = cx.upload_stats() - start;
        assert_eq!(store.rows(), 100 + appended);
        assert_eq!(written.columns.bytes, appended * 8, "{written:?}");
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
                    chunk.bytes()[r.start as usize..r.end as usize]
                );
            }
        }
    }
}

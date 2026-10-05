// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! The GPU column store (RFC-001 §3), single-chunk form.
//!
//! Accessors run once; their outputs are stored as structure-of-arrays in
//! **one buffer per chunk**, each column at a 256-byte-aligned sub-range,
//! and bound as instance-rate vertex buffers. Relative columns store
//! `(v - origin) as f32` against a per-chunk f64 origin so that large
//! values (timestamps) keep f32 precision.
//!
//! S0a scope: exactly one chunk. Chunking, append, retention policies,
//! validity bits and dictionary encoding are RFC-001 S4.

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

/// f64 statistics of a column's finite values in one chunk. They drive
/// automatic domains (and, from S4, chunk culling).
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
    fn of(values: &[f64]) -> Option<Self> {
        let mut stats: Option<Self> = None;
        let mut non_finite = 0;
        for &v in values {
            if !v.is_finite() {
                non_finite += 1;
                continue;
            }
            let s = stats.get_or_insert(Self {
                min: v,
                max: v,
                non_finite: 0,
            });
            s.min = s.min.min(v);
            s.max = s.max.max(v);
        }
        stats.map(|s| Self { non_finite, ..s })
    }

    /// `(min, max)`.
    pub fn extent(&self) -> (f64, f64) {
        (self.min, self.max)
    }
}

/// One column of one chunk.
#[derive(Clone, Debug)]
pub struct ChunkColumn {
    format: ColumnFormat,
    /// Byte offset of the column in the chunk buffer (256-aligned).
    offset: u64,
    /// The origin subtracted from every value (0 for absolute columns).
    origin: f64,
    stats: Option<ColumnStats>,
}

impl ChunkColumn {
    /// The storage format.
    pub fn format(&self) -> ColumnFormat {
        self.format
    }

    /// The f64 origin values are stored relative to (0 for absolute).
    pub fn origin(&self) -> f64 {
        self.origin
    }

    /// Statistics of the finite values; `None` if there are none.
    pub fn stats(&self) -> Option<ColumnStats> {
        self.stats
    }
}

/// A single-chunk column store.
pub struct ColumnStore {
    rows: u32,
    columns: Vec<ChunkColumn>,
    /// CPU staging bytes for the whole chunk, kept until uploaded.
    bytes: Vec<u8>,
    buffer: Option<(ContextId, wgpu::Buffer)>,
}

impl std::fmt::Debug for ColumnStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ColumnStore")
            .field("rows", &self.rows)
            .field("columns", &self.columns)
            .field("uploaded", &self.buffer.is_some())
            .finish()
    }
}

/// Column sub-ranges are aligned for both vertex fetch and storage binding.
const COLUMN_ALIGN: u64 = 256;

fn align_up(v: u64, a: u64) -> u64 {
    v.div_ceil(a) * a
}

impl ColumnStore {
    /// Build a one-chunk store from evaluated accessor outputs, one
    /// `(format, values)` pair per column. All columns must have the same
    /// length.
    pub fn from_columns(columns: Vec<(ColumnFormat, Vec<f64>)>) -> Result<Self> {
        let rows = columns.first().map_or(0, |(_, v)| v.len());
        if let Some((i, (_, v))) = columns
            .iter()
            .enumerate()
            .find(|(_, (_, v))| v.len() != rows)
        {
            return Err(Error::config(
                "column store",
                format!("column {i} has {} rows, column 0 has {rows}", v.len()),
            ));
        }
        let rows = u32::try_from(rows).map_err(|_| {
            Error::config(
                "column store",
                format!("{rows} rows exceed one chunk; chunking arrives in RFC-001 S4"),
            )
        })?;

        let mut offset = 0;
        let mut meta = Vec::with_capacity(columns.len());
        for (format, _) in &columns {
            meta.push(offset);
            offset = align_up(offset + u64::from(rows) * format.stride(), COLUMN_ALIGN);
        }
        let mut bytes = vec![0u8; offset.max(COLUMN_ALIGN) as usize];
        let mut out = Vec::with_capacity(columns.len());
        for ((format, values), offset) in columns.into_iter().zip(meta) {
            let origin = match format {
                ColumnFormat::F32 => 0.0,
                // The chunk's first finite value.
                ColumnFormat::F32Relative => values
                    .iter()
                    .copied()
                    .find(|v| v.is_finite())
                    .unwrap_or(0.0),
            };
            let dst = &mut bytes[offset as usize..][..values.len() * 4];
            for (chunk, v) in dst.chunks_exact_mut(4).zip(&values) {
                chunk.copy_from_slice(&((v - origin) as f32).to_le_bytes());
            }
            out.push(ChunkColumn {
                format,
                offset,
                origin,
                stats: ColumnStats::of(&values),
            });
        }
        Ok(Self {
            rows,
            columns: out,
            bytes,
            buffer: None,
        })
    }

    /// Number of rows.
    pub fn rows(&self) -> u32 {
        self.rows
    }

    /// The columns of the (single) chunk.
    pub fn columns(&self) -> &[ChunkColumn] {
        &self.columns
    }

    /// Upload the chunk to `cx` (once per context) and return its buffer.
    ///
    /// The buffer has `VERTEX | STORAGE | COPY_DST | COPY_SRC` usage, so the
    /// same bytes serve instanced draws and (from S4/S9) compute passes.
    pub fn upload(&mut self, cx: &Context) -> Result<&wgpu::Buffer> {
        let size = self.bytes.len() as u64;
        if size > cx.caps().limits.max_buffer_size {
            return Err(Error::config(
                "column store",
                format!(
                    "chunk of {size} bytes exceeds the device's max_buffer_size ({}); \
                     chunking arrives in RFC-001 S4",
                    cx.caps().limits.max_buffer_size
                ),
            ));
        }
        if !matches!(&self.buffer, Some((id, _)) if *id == cx.id()) {
            let buffer = cx.device().create_buffer(&wgpu::BufferDescriptor {
                label: Some("gup column chunk"),
                size,
                usage: wgpu::BufferUsages::VERTEX
                    | wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::COPY_DST
                    | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            });
            // The only place column bytes are written (counted as
            // `Upload::Column`).
            cx.write_buffer(Upload::Column, &buffer, 0, &self.bytes);
            self.buffer = Some((cx.id(), buffer));
        }
        Ok(&self.buffer.as_ref().expect("uploaded above").1)
    }

    /// The byte range of column `index` in the chunk buffer.
    pub(crate) fn column_range(&self, index: usize) -> std::ops::Range<u64> {
        let col = &self.columns[index];
        col.offset..col.offset + u64::from(self.rows) * col.format.stride()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_columns_keep_precision_for_large_values() {
        // Unix seconds a day apart: absolute f32 has a 128 s ULP here.
        let t0 = 1.7e9;
        let values: Vec<f64> = (0..5).map(|i| t0 + f64::from(i) * 0.25).collect();
        let store = ColumnStore::from_columns(vec![
            (ColumnFormat::F32Relative, values.clone()),
            (ColumnFormat::F32, values.clone()),
        ])
        .unwrap();
        let rel = &store.columns()[0];
        let abs = &store.columns()[1];
        assert_eq!(rel.origin(), t0);
        assert_eq!(abs.origin(), 0.0);
        assert_eq!(rel.offset % COLUMN_ALIGN, 0);
        assert_eq!(abs.offset % COLUMN_ALIGN, 0);
        let read = |c: &ChunkColumn, i: usize| {
            let at = c.offset as usize + i * 4;
            f32::from_le_bytes(store.bytes[at..at + 4].try_into().unwrap())
        };
        for (i, v) in values.iter().enumerate() {
            // Relative: exact to f32 precision of the small offset.
            assert_eq!(f64::from(read(rel, i)) + rel.origin(), *v);
        }
        // Absolute f32 collapses all five values to one.
        assert_eq!(read(abs, 0), read(abs, 4));
        assert_eq!(rel.stats().unwrap().extent(), (t0, t0 + 1.0));
    }

    #[test]
    fn stats_skip_non_finite_values() {
        let store =
            ColumnStore::from_columns(vec![(ColumnFormat::F32, vec![f64::NAN, 3.0, -1.0, 2.0])])
                .unwrap();
        let s = store.columns()[0].stats().unwrap();
        assert_eq!((s.min, s.max, s.non_finite), (-1.0, 3.0, 1));
    }

    #[test]
    fn mismatched_lengths_are_a_configuration_error() {
        let err = ColumnStore::from_columns(vec![
            (ColumnFormat::F32, vec![1.0, 2.0]),
            (ColumnFormat::F32, vec![1.0]),
        ])
        .unwrap_err();
        assert!(err.to_string().contains("column 1 has 1 rows"), "{err}");
    }

    #[test]
    fn upload_writes_the_chunk_once_per_context() {
        let cx = Context::new_blocking().unwrap();
        let mut store = ColumnStore::from_columns(vec![
            (ColumnFormat::F32Relative, vec![10.0, 11.0, 12.5]),
            (ColumnFormat::F32, vec![1.0, 2.0, 3.0]),
        ])
        .unwrap();
        let buffer = store.upload(&cx).unwrap().clone();
        assert!(buffer.usage().contains(
            wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::STORAGE
        ));
        // Read the chunk back from the GPU.
        let readback = cx.device().create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: buffer.size(),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut enc = cx.device().create_command_encoder(&Default::default());
        enc.copy_buffer_to_buffer(&buffer, 0, &readback, 0, buffer.size());
        cx.submit([enc.finish()]);
        readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, |r| r.unwrap());
        cx.wait_idle().unwrap();
        let bytes = readback.slice(..).get_mapped_range().to_vec();
        let f = |at: usize| f32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!([f(0), f(4), f(8)], [0.0, 1.0, 2.5]);
        assert_eq!([f(256), f(260), f(264)], [1.0, 2.0, 3.0]);
        // A second upload to the same context reuses the buffer.
        let again = store.upload(&cx).unwrap();
        assert_eq!(again, &buffer);
    }
}

// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Core [`StreamingBuffer<T>`] with keyed insert/update/remove, dirty-region
//! tracking, double-buffered GPU swap, and partial-flush of only mutated byte
//! ranges.

use std::collections::HashMap;
use std::time::Instant;

use crate::error::{GupError, GupResult};

use super::dirty_region::{BufferRegion, DirtyRegionTracker};
use super::latency::{LatencyTracker, LatencyTrackerConfig};
use super::ring_buffer::RingBuffer;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for a [`StreamingBuffer`].
#[derive(Debug, Clone)]
pub struct StreamingBufferConfig {
    /// Maximum number of data items the buffer can hold.
    /// When capacity is reached, the oldest items are evicted (ring-buffer
    /// semantics).
    pub capacity: usize,

    /// Size of the rolling latency-tracker window (number of samples).
    pub latency_window: usize,
}

impl Default for StreamingBufferConfig {
    fn default() -> Self {
        Self {
            capacity: 10_000,
            latency_window: 1024,
        }
    }
}

// ---------------------------------------------------------------------------
// Update type
// ---------------------------------------------------------------------------

/// Describes a single mutation to apply to a [`StreamingBuffer`].
#[derive(Debug, Clone)]
pub enum StreamUpdate<T> {
    /// Insert a new item with the given key.
    Insert { key: u64, data: T },
    /// Update an existing item identified by key.
    Update { key: u64, data: T },
    /// Remove the item with the given key.
    Remove { key: u64 },
    /// Apply a batch of updates atomically.
    Batch { updates: Vec<StreamUpdate<T>> },
}

// ---------------------------------------------------------------------------
// StreamingBuffer
// ---------------------------------------------------------------------------

/// A double-buffered, keyed GPU data store with dirty-region tracking.
///
/// `T` must be [`bytemuck::Pod`] so it can be memcpy'd to a GPU buffer.
///
/// # Data model
///
/// Items are identified by a `u64` key. Internally they occupy a slot in a
/// fixed-capacity [`RingBuffer`]. When the ring buffer is full the oldest slot
/// is evicted, along with its key mapping.
///
/// # Double buffering
///
/// Two GPU buffers are maintained: the *active* buffer (currently used for
/// rendering) and the *staging* buffer (where dirty writes are applied).
/// Calling [`flush`](Self::flush) writes only the dirty byte ranges to the
/// staging buffer, then swaps the two so that the next render picks up the
/// fresh data.
pub struct StreamingBuffer<T: bytemuck::Pod + bytemuck::Zeroable> {
    // -- CPU-side data --
    ring: RingBuffer<T>,
    key_to_index: HashMap<u64, usize>,

    // -- GPU buffers (double-buffered) --
    buffers: [wgpu::Buffer; 2],
    /// Index into `buffers` that is currently used for rendering.
    active_idx: usize,
    /// Byte capacity of each GPU buffer.
    gpu_byte_capacity: u64,

    // -- Dirty tracking --
    dirty: DirtyRegionTracker,

    // -- Metrics --
    latency: LatencyTracker,
    flush_count: u64,
}

impl<T: bytemuck::Pod + bytemuck::Zeroable> StreamingBuffer<T> {
    /// Create a new streaming buffer, allocating two GPU buffers on `device`.
    pub fn new(device: &wgpu::Device, config: StreamingBufferConfig) -> Self {
        let capacity = config.capacity.max(1);
        let element_size = std::mem::size_of::<T>() as u64;
        let byte_cap = element_size * capacity as u64;

        let usage = wgpu::BufferUsages::VERTEX
            | wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_DST
            | wgpu::BufferUsages::COPY_SRC;

        let make_buf = |label: &str| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: byte_cap,
                usage,
                mapped_at_creation: false,
            })
        };

        let buffers = [
            make_buf("streaming_buffer_a"),
            make_buf("streaming_buffer_b"),
        ];

        Self {
            ring: RingBuffer::new(capacity),
            key_to_index: HashMap::new(),
            buffers,
            active_idx: 0,
            gpu_byte_capacity: byte_cap,
            dirty: DirtyRegionTracker::new(),
            latency: LatencyTracker::new(LatencyTrackerConfig {
                window_size: config.latency_window,
            }),
            flush_count: 0,
        }
    }

    // -- Keyed mutations (CPU side) -----------------------------------------

    /// Insert a new item or overwrite an existing one with the same key.
    ///
    /// Returns `true` if an existing item was overwritten, `false` if this is a
    /// fresh insert.
    pub fn insert(&mut self, key: u64, data: T) -> bool {
        let start = Instant::now();

        let overwritten = if let Some(&idx) = self.key_to_index.get(&key) {
            // Key already present – update in place.
            self.ring.replace(idx, data);
            self.mark_index_dirty(idx);
            true
        } else {
            // New key – push into ring buffer.
            let idx = self.ring.next_write_index();

            // If pushing will evict an old item we must remove its key mapping.
            if self.ring.is_full() {
                // The slot at `idx` is about to be overwritten.
                self.key_to_index.retain(|_, v| *v != idx);
            }

            self.ring.push(data);
            self.key_to_index.insert(key, idx);
            self.mark_index_dirty(idx);
            false
        };

        self.latency.record(start.elapsed());
        overwritten
    }

    /// Update the data for an existing key.
    ///
    /// Returns `Err` if the key does not exist.
    pub fn update(&mut self, key: u64, data: T) -> GupResult<()> {
        let start = Instant::now();

        let idx = *self.key_to_index.get(&key).ok_or_else(|| {
            GupError::buffer_error(format!("StreamingBuffer: key {key} not found for update"))
        })?;

        self.ring.replace(idx, data);
        self.mark_index_dirty(idx);
        self.latency.record(start.elapsed());
        Ok(())
    }

    /// Remove the item with the given key.
    ///
    /// The slot is zeroed on the GPU side during the next flush.
    /// Returns the removed data, or `Err` if the key was not found.
    pub fn remove(&mut self, key: u64) -> GupResult<T> {
        let start = Instant::now();

        let idx = self.key_to_index.remove(&key).ok_or_else(|| {
            GupError::buffer_error(format!("StreamingBuffer: key {key} not found for remove"))
        })?;

        let removed = self.ring.remove(idx).ok_or_else(|| {
            GupError::buffer_error(format!(
                "StreamingBuffer: ring slot {idx} was unexpectedly empty"
            ))
        })?;

        // Mark the slot dirty so the GPU sees a zeroed-out element.
        self.mark_index_dirty(idx);
        self.latency.record(start.elapsed());
        Ok(removed)
    }

    /// Apply a batch of [`StreamUpdate`]s.
    ///
    /// Updates are applied in order. The latency recorded is for the entire
    /// batch, attributed to every item.
    pub fn apply_batch(&mut self, updates: Vec<StreamUpdate<T>>) -> GupResult<()> {
        let start = Instant::now();
        let count = Self::count_updates(&updates);

        for update in updates {
            match update {
                StreamUpdate::Insert { key, data } => {
                    self.insert(key, data);
                }
                StreamUpdate::Update { key, data } => {
                    self.update(key, data)?;
                }
                StreamUpdate::Remove { key } => {
                    self.remove(key)?;
                }
                StreamUpdate::Batch { updates } => {
                    self.apply_batch(updates)?;
                }
            }
        }

        // Override per-item latency records with the batch latency.
        self.latency.record_batch(start.elapsed(), count);
        Ok(())
    }

    // -- GPU flush ----------------------------------------------------------

    /// Flush dirty regions to the *staging* GPU buffer, then swap active and
    /// staging so the next render sees the fresh data.
    ///
    /// Only byte ranges that were modified since the last flush are written,
    /// keeping GPU bus traffic to a minimum.
    ///
    /// Returns the number of bytes written.
    pub fn flush(&mut self, _device: &wgpu::Device, queue: &wgpu::Queue) -> usize {
        if !self.dirty.is_dirty() {
            return 0;
        }

        let staging_idx = 1 - self.active_idx;
        let staging_buf = &self.buffers[staging_idx];

        let regions = self.dirty.drain();
        let element_size = std::mem::size_of::<T>();
        let mut bytes_written = 0usize;

        for region in &regions {
            // Clamp to actual GPU capacity.
            let end = region.end().min(self.gpu_byte_capacity as usize);
            if region.offset >= end {
                continue;
            }
            let clamped_len = end - region.offset;

            // Build a byte slice from the CPU-side ring buffer data covering
            // the requested range.
            let elem_start = region.offset / element_size;
            let elem_end = end.div_ceil(element_size); // round up

            let mut bytes: Vec<u8> = Vec::with_capacity(clamped_len);
            let zero = T::zeroed();
            for i in elem_start..elem_end {
                let item = self.ring.get(i).unwrap_or(&zero);
                let item_bytes = bytemuck::bytes_of(item);
                bytes.extend_from_slice(item_bytes);
            }

            // Trim to exactly the dirty byte range.
            let trim_start = region.offset - elem_start * element_size;
            let trimmed = &bytes[trim_start..trim_start + clamped_len];

            queue.write_buffer(staging_buf, region.offset as u64, trimmed);
            bytes_written += clamped_len;
        }

        // Swap active ↔ staging.
        self.active_idx = staging_idx;
        self.flush_count += 1;

        bytes_written
    }

    // -- Accessors ----------------------------------------------------------

    /// The GPU buffer currently used for rendering.
    pub fn active_buffer(&self) -> &wgpu::Buffer {
        &self.buffers[self.active_idx]
    }

    /// Number of live items.
    pub fn len(&self) -> usize {
        self.ring.len()
    }

    /// Whether the buffer has no items.
    pub fn is_empty(&self) -> bool {
        self.ring.is_empty()
    }

    /// Maximum capacity (items, not bytes).
    pub fn capacity(&self) -> usize {
        self.ring.capacity()
    }

    /// Whether there are unflushed dirty regions.
    pub fn is_dirty(&self) -> bool {
        self.dirty.is_dirty()
    }

    /// Number of bytes that are currently dirty.
    pub fn dirty_bytes(&self) -> usize {
        self.dirty.dirty_bytes()
    }

    /// Number of disjoint dirty regions.
    pub fn dirty_region_count(&self) -> usize {
        self.dirty.region_count()
    }

    /// Total number of flushes performed.
    pub fn flush_count(&self) -> u64 {
        self.flush_count
    }

    /// Reference to the latency tracker.
    pub fn latency(&self) -> &LatencyTracker {
        &self.latency
    }

    /// Look up the data for a given key on the CPU side.
    pub fn get(&self, key: u64) -> Option<&T> {
        self.key_to_index
            .get(&key)
            .and_then(|&idx| self.ring.get(idx))
    }

    /// Check whether a key is present.
    pub fn contains_key(&self, key: u64) -> bool {
        self.key_to_index.contains_key(&key)
    }

    /// Iterate over all live `(key, &T)` pairs.
    pub fn iter(&self) -> impl Iterator<Item = (u64, &T)> {
        // Build inverse index → key map for iteration.
        let index_to_key: HashMap<usize, u64> =
            self.key_to_index.iter().map(|(&k, &v)| (v, k)).collect();
        self.ring
            .iter()
            .filter_map(move |(idx, val)| index_to_key.get(&idx).map(|&key| (key, val)))
    }

    // -- Internals ----------------------------------------------------------

    /// Mark the byte range of a single element at `index` as dirty.
    fn mark_index_dirty(&mut self, index: usize) {
        let element_size = std::mem::size_of::<T>();
        self.dirty
            .mark_dirty(BufferRegion::new(index * element_size, element_size));
    }

    /// Count the total number of leaf updates in a (possibly nested) batch.
    fn count_updates(updates: &[StreamUpdate<T>]) -> usize {
        updates
            .iter()
            .map(|u| match u {
                StreamUpdate::Batch { updates } => Self::count_updates(updates),
                _ => 1,
            })
            .sum()
    }
}

impl<T: bytemuck::Pod + bytemuck::Zeroable> std::fmt::Debug for StreamingBuffer<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StreamingBuffer")
            .field("len", &self.ring.len())
            .field("capacity", &self.ring.capacity())
            .field("active_idx", &self.active_idx)
            .field("dirty_regions", &self.dirty.region_count())
            .field("flush_count", &self.flush_count)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::RenderContext;
    use std::time::Duration;

    // A simple Pod type for testing.
    #[repr(C)]
    #[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
    struct TestItem {
        value: f32,
        _pad: f32,
    }

    impl TestItem {
        fn new(v: f32) -> Self {
            Self {
                value: v,
                _pad: 0.0,
            }
        }
    }

    /// Helper to obtain a wgpu device for tests.
    async fn test_device() -> (wgpu::Device, wgpu::Queue) {
        let ctx = RenderContext::new().await.unwrap();
        let device = ctx.device().clone();
        let queue = ctx.queue().clone();
        (device, queue)
    }

    #[tokio::test]
    async fn insert_and_get() {
        let (device, _queue) = test_device().await;
        let mut buf = StreamingBuffer::<TestItem>::new(
            &device,
            StreamingBufferConfig {
                capacity: 100,
                ..Default::default()
            },
        );

        assert!(!buf.insert(1, TestItem::new(1.0)));
        assert!(!buf.insert(2, TestItem::new(2.0)));

        assert_eq!(buf.len(), 2);
        assert_eq!(buf.get(1).unwrap().value, 1.0);
        assert_eq!(buf.get(2).unwrap().value, 2.0);
        assert!(buf.is_dirty());
    }

    #[tokio::test]
    async fn insert_overwrite() {
        let (device, _queue) = test_device().await;
        let mut buf = StreamingBuffer::<TestItem>::new(
            &device,
            StreamingBufferConfig {
                capacity: 100,
                ..Default::default()
            },
        );

        buf.insert(1, TestItem::new(1.0));
        assert!(buf.insert(1, TestItem::new(9.0)));
        assert_eq!(buf.len(), 1);
        assert_eq!(buf.get(1).unwrap().value, 9.0);
    }

    #[tokio::test]
    async fn update_existing() {
        let (device, _queue) = test_device().await;
        let mut buf = StreamingBuffer::<TestItem>::new(
            &device,
            StreamingBufferConfig {
                capacity: 100,
                ..Default::default()
            },
        );

        buf.insert(1, TestItem::new(1.0));
        buf.update(1, TestItem::new(5.0)).unwrap();
        assert_eq!(buf.get(1).unwrap().value, 5.0);
    }

    #[tokio::test]
    async fn update_missing_key_errors() {
        let (device, _queue) = test_device().await;
        let mut buf = StreamingBuffer::<TestItem>::new(
            &device,
            StreamingBufferConfig {
                capacity: 100,
                ..Default::default()
            },
        );

        let result = buf.update(99, TestItem::new(1.0));
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn remove_existing() {
        let (device, _queue) = test_device().await;
        let mut buf = StreamingBuffer::<TestItem>::new(
            &device,
            StreamingBufferConfig {
                capacity: 100,
                ..Default::default()
            },
        );

        buf.insert(1, TestItem::new(1.0));
        let removed = buf.remove(1).unwrap();
        assert_eq!(removed.value, 1.0);
        assert_eq!(buf.len(), 0);
        assert!(!buf.contains_key(1));
    }

    #[tokio::test]
    async fn remove_missing_key_errors() {
        let (device, _queue) = test_device().await;
        let mut buf = StreamingBuffer::<TestItem>::new(
            &device,
            StreamingBufferConfig {
                capacity: 100,
                ..Default::default()
            },
        );

        assert!(buf.remove(99).is_err());
    }

    #[tokio::test]
    async fn capacity_eviction() {
        let (device, _queue) = test_device().await;
        let mut buf = StreamingBuffer::<TestItem>::new(
            &device,
            StreamingBufferConfig {
                capacity: 3,
                ..Default::default()
            },
        );

        buf.insert(1, TestItem::new(1.0));
        buf.insert(2, TestItem::new(2.0));
        buf.insert(3, TestItem::new(3.0));
        assert_eq!(buf.len(), 3);

        // 4th insert evicts key 1
        buf.insert(4, TestItem::new(4.0));
        assert_eq!(buf.len(), 3);
        assert!(!buf.contains_key(1));
        assert!(buf.contains_key(4));
    }

    #[tokio::test]
    async fn flush_writes_dirty_regions() {
        let (device, queue) = test_device().await;
        let mut buf = StreamingBuffer::<TestItem>::new(
            &device,
            StreamingBufferConfig {
                capacity: 100,
                ..Default::default()
            },
        );

        buf.insert(1, TestItem::new(1.0));
        buf.insert(2, TestItem::new(2.0));
        assert!(buf.is_dirty());

        let bytes = buf.flush(&device, &queue);
        assert!(bytes > 0);
        assert!(!buf.is_dirty());
        assert_eq!(buf.flush_count(), 1);
    }

    #[tokio::test]
    async fn flush_noop_when_clean() {
        let (device, queue) = test_device().await;
        let mut buf = StreamingBuffer::<TestItem>::new(
            &device,
            StreamingBufferConfig {
                capacity: 100,
                ..Default::default()
            },
        );

        let bytes = buf.flush(&device, &queue);
        assert_eq!(bytes, 0);
        assert_eq!(buf.flush_count(), 0);
    }

    #[tokio::test]
    async fn double_buffer_swaps() {
        let (device, queue) = test_device().await;
        let mut buf = StreamingBuffer::<TestItem>::new(
            &device,
            StreamingBufferConfig {
                capacity: 100,
                ..Default::default()
            },
        );

        let buf_a = buf.active_buffer() as *const wgpu::Buffer;
        buf.insert(1, TestItem::new(1.0));
        buf.flush(&device, &queue);
        let buf_b = buf.active_buffer() as *const wgpu::Buffer;

        // After flush, active buffer should have swapped.
        assert_ne!(buf_a, buf_b);

        // Flush again swaps back.
        buf.insert(2, TestItem::new(2.0));
        buf.flush(&device, &queue);
        let buf_c = buf.active_buffer() as *const wgpu::Buffer;
        assert_eq!(buf_a, buf_c);
    }

    #[tokio::test]
    async fn batch_updates() {
        let (device, _queue) = test_device().await;
        let mut buf = StreamingBuffer::<TestItem>::new(
            &device,
            StreamingBufferConfig {
                capacity: 100,
                ..Default::default()
            },
        );

        let updates = vec![
            StreamUpdate::Insert {
                key: 1,
                data: TestItem::new(1.0),
            },
            StreamUpdate::Insert {
                key: 2,
                data: TestItem::new(2.0),
            },
            StreamUpdate::Update {
                key: 1,
                data: TestItem::new(10.0),
            },
        ];

        buf.apply_batch(updates).unwrap();
        assert_eq!(buf.len(), 2);
        assert_eq!(buf.get(1).unwrap().value, 10.0);
        assert_eq!(buf.get(2).unwrap().value, 2.0);
    }

    #[tokio::test]
    async fn latency_is_tracked() {
        let (device, _queue) = test_device().await;
        let mut buf = StreamingBuffer::<TestItem>::new(
            &device,
            StreamingBufferConfig {
                capacity: 100,
                ..Default::default()
            },
        );

        for i in 0..50 {
            buf.insert(i, TestItem::new(i as f32));
        }

        let snap = buf.latency().snapshot();
        assert!(snap.total_ops >= 50);
        assert!(snap.mean.is_some());
        // CPU-only insert should be well under 1ms.
        assert!(snap.mean.unwrap() < Duration::from_millis(1));
    }

    #[tokio::test]
    async fn iter_returns_all_live_items() {
        let (device, _queue) = test_device().await;
        let mut buf = StreamingBuffer::<TestItem>::new(
            &device,
            StreamingBufferConfig {
                capacity: 100,
                ..Default::default()
            },
        );

        buf.insert(10, TestItem::new(10.0));
        buf.insert(20, TestItem::new(20.0));
        buf.insert(30, TestItem::new(30.0));
        buf.remove(20).unwrap();

        let items: Vec<(u64, f32)> = buf.iter().map(|(k, v)| (k, v.value)).collect();
        assert_eq!(items.len(), 2);
        assert!(items.iter().any(|(k, _)| *k == 10));
        assert!(items.iter().any(|(k, _)| *k == 30));
    }

    #[tokio::test]
    async fn dirty_region_merging() {
        let (device, _queue) = test_device().await;
        let mut buf = StreamingBuffer::<TestItem>::new(
            &device,
            StreamingBufferConfig {
                capacity: 100,
                ..Default::default()
            },
        );

        // Insert three consecutive items – their dirty regions should merge.
        buf.insert(0, TestItem::new(0.0));
        buf.insert(1, TestItem::new(1.0));
        buf.insert(2, TestItem::new(2.0));

        // Adjacent regions should have been merged into 1 (or at most a few).
        assert!(buf.dirty_region_count() <= 1);
    }
}

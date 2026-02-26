// Gup - GPU-Accelerated Data Visualization Library
// Copyright (C) 2025 Corin Lawson <corin@phiware.com.au>
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

//! GPU buffer management system for efficient GPU memory handling.
//!
//! This module provides type-safe, high-performance buffer management with automatic
//! resizing, memory pooling, and lifecycle management. It forms the foundation for
//! all GPU-accelerated data transformations in Gup.

use crate::error::{GupError, GupResult};
use std::collections::{HashMap, VecDeque};
use std::marker::PhantomData;
use std::sync::Arc;
use std::time::{Duration, Instant};
use wgpu::*;

/// Types of GPU buffers with specific usage patterns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BufferType {
    /// Vertex attributes and geometry data
    Vertex,
    /// Per-instance data for instanced rendering
    Instance,
    /// Shader uniforms (small, frequently updated)
    Uniform,
    /// Large datasets accessed by compute shaders
    Storage,
    /// Index data for indexed rendering
    Index,
    /// Staging buffers for GPU-to-CPU readback (MAP_READ)
    Staging,
}

impl BufferType {
    /// Get the appropriate wgpu buffer usage flags for this buffer type.
    pub fn usage_flags(self) -> BufferUsages {
        match self {
            BufferType::Vertex => {
                BufferUsages::VERTEX | BufferUsages::COPY_DST | BufferUsages::COPY_SRC
            }
            BufferType::Instance => {
                BufferUsages::VERTEX
                    | BufferUsages::STORAGE
                    | BufferUsages::COPY_DST
                    | BufferUsages::COPY_SRC
            }
            BufferType::Uniform => {
                BufferUsages::UNIFORM | BufferUsages::COPY_DST | BufferUsages::COPY_SRC
            }
            BufferType::Storage => {
                BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC
            }
            BufferType::Index => {
                BufferUsages::INDEX | BufferUsages::COPY_DST | BufferUsages::COPY_SRC
            }
            BufferType::Staging => BufferUsages::COPY_DST | BufferUsages::MAP_READ,
        }
    }

    /// Get the minimum alignment requirement for this buffer type.
    pub fn alignment(self) -> u64 {
        match self {
            BufferType::Vertex | BufferType::Instance => 4, // 4-byte alignment
            BufferType::Uniform => 256,                     // Uniform buffer alignment
            BufferType::Storage | BufferType::Index => 4,   // Storage/index buffer alignment
            BufferType::Staging => 4,                       // Staging buffer alignment
        }
    }
}

/// Type-safe GPU buffer with automatic resizing and lifecycle management.
pub struct GpuBuffer<T> {
    buffer: Buffer,
    capacity: usize,
    len: usize,
    buffer_type: BufferType,
    usage: BufferUsages,
    _phantom: PhantomData<T>,
}

impl<T> GpuBuffer<T>
where
    T: bytemuck::Pod + bytemuck::Zeroable,
{
    /// Create a new GPU buffer with the specified capacity.
    pub fn new(device: &Device, buffer_type: BufferType, capacity: usize) -> Self {
        let usage = buffer_type.usage_flags();
        let size = Self::calculate_buffer_size(capacity, buffer_type);

        let buffer = device.create_buffer(&BufferDescriptor {
            label: Some(&format!("{buffer_type:?}_buffer")),
            size,
            usage,
            mapped_at_creation: false,
        });

        Self {
            buffer,
            capacity,
            len: 0,
            buffer_type,
            usage,
            _phantom: PhantomData,
        }
    }

    /// Upload data to the GPU buffer, resizing if necessary.
    pub fn upload(&mut self, device: &Device, queue: &Queue, data: &[T]) -> GupResult<()> {
        if data.len() > self.capacity {
            self.resize(device, queue, data.len())?;
        }

        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(data));
        self.len = data.len();
        Ok(())
    }

    /// Upload data to a specific range in the buffer.
    pub fn upload_range(&mut self, queue: &Queue, data: &[T], offset: usize) -> GupResult<()> {
        if offset + data.len() > self.capacity {
            return Err(GupError::buffer_error(format!(
                "Range upload exceeds buffer capacity: offset={}, data_len={}, capacity={}",
                offset,
                data.len(),
                self.capacity
            )));
        }

        let byte_offset = (offset * std::mem::size_of::<T>()) as u64;
        queue.write_buffer(&self.buffer, byte_offset, bytemuck::cast_slice(data));

        // Update length to include the new data
        self.len = self.len.max(offset + data.len());
        Ok(())
    }

    /// Download data from the GPU buffer to CPU memory.
    ///
    /// This method creates a staging buffer, copies the GPU buffer contents to it,
    /// maps it for reading, and returns the data as a Vec<T>. Primarily used for
    /// debugging, validation, and CPU-side post-processing.
    ///
    /// # Performance Note
    /// Buffer downloads involve GPU-to-CPU synchronization and can be slow for large
    /// buffers. Use sparingly in performance-critical code.
    pub async fn download(&self, device: &Device, queue: &Queue) -> GupResult<Vec<T>> {
        if self.len == 0 {
            return Ok(Vec::new());
        }

        self.download_range(device, queue, 0, self.len).await
    }

    /// Download data from the GPU buffer using a pooled staging buffer.
    ///
    /// Like [`download`](Self::download) but reuses staging buffers from the pool
    /// instead of creating new ones, reducing allocation overhead for repeated
    /// readback operations.
    pub async fn download_pooled(
        &self,
        device: &Device,
        queue: &Queue,
        pool: &mut BufferPool,
    ) -> GupResult<Vec<T>> {
        if self.len == 0 {
            return Ok(Vec::new());
        }

        self.download_range_pooled(device, queue, 0, self.len, pool)
            .await
    }

    /// Download a range of data from the GPU buffer.
    ///
    /// Downloads only a specific range of elements from the buffer, which can be
    /// more efficient than downloading the entire buffer when only a subset is needed.
    ///
    /// # Arguments
    /// * `device` - The wgpu device
    /// * `queue` - The wgpu queue
    /// * `offset` - Starting element index
    /// * `len` - Number of elements to download
    ///
    /// # Errors
    /// Returns an error if the range is invalid or if buffer mapping fails.
    pub async fn download_range(
        &self,
        device: &Device,
        queue: &Queue,
        offset: usize,
        len: usize,
    ) -> GupResult<Vec<T>> {
        if len == 0 {
            return Ok(Vec::new());
        }

        if offset + len > self.len {
            return Err(GupError::buffer_error(format!(
                "Download range exceeds buffer length: offset={}, len={}, buffer_len={}",
                offset, len, self.len
            )));
        }

        let element_size = std::mem::size_of::<T>() as u64;
        let byte_offset = (offset as u64) * element_size;
        let byte_size = (len as u64) * element_size;

        // Create staging buffer for GPU-to-CPU transfer
        let staging_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("download_staging_buffer"),
            size: byte_size,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        // Copy from GPU buffer to staging buffer
        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("download_encoder"),
        });

        encoder.copy_buffer_to_buffer(&self.buffer, byte_offset, &staging_buffer, 0, byte_size);

        queue.submit(Some(encoder.finish()));

        // Map the staging buffer for reading
        let buffer_slice = staging_buffer.slice(..);
        let (sender, receiver) = tokio::sync::oneshot::channel();

        buffer_slice.map_async(MapMode::Read, move |result| {
            let _ = sender.send(result);
        });

        // Poll the device to complete the mapping operation
        let _ = device.poll(PollType::Wait);

        // Wait for the mapping to complete
        receiver
            .await
            .map_err(|_| GupError::buffer_error("Buffer mapping callback was dropped"))?
            .map_err(|e| {
                GupError::buffer_error(format!("Failed to map buffer for reading: {:?}", e))
            })?;

        // Read the data
        let data = buffer_slice.get_mapped_range();
        let result: Vec<T> = bytemuck::cast_slice(&data).to_vec();

        // Clean up
        drop(data);
        staging_buffer.unmap();

        Ok(result)
    }

    /// Download a range of data from the GPU buffer using a pooled staging buffer.
    ///
    /// Like [`download_range`](Self::download_range) but reuses staging buffers
    /// from the pool instead of allocating new ones each time. The staging buffer
    /// is returned to the pool after use, enabling reuse in subsequent calls.
    ///
    /// This provides significant performance benefits for repeated readback
    /// operations by eliminating per-call buffer allocation overhead.
    pub async fn download_range_pooled(
        &self,
        device: &Device,
        queue: &Queue,
        offset: usize,
        len: usize,
        pool: &mut BufferPool,
    ) -> GupResult<Vec<T>> {
        if len == 0 {
            return Ok(Vec::new());
        }

        if offset + len > self.len {
            return Err(GupError::buffer_error(format!(
                "Download range exceeds buffer length: offset={}, len={}, buffer_len={}",
                offset, len, self.len
            )));
        }

        let element_size = std::mem::size_of::<T>() as u64;
        let byte_offset = (offset as u64) * element_size;
        let byte_size = (len as u64) * element_size;

        // Allocate staging buffer from pool (reuses existing if available)
        let (staging_buffer, size_class) =
            pool.allocate_raw(BufferType::Staging, byte_size as usize);

        // Copy from GPU buffer to staging buffer
        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("download_pooled_encoder"),
        });

        encoder.copy_buffer_to_buffer(&self.buffer, byte_offset, &staging_buffer, 0, byte_size);

        queue.submit(Some(encoder.finish()));

        // Map the staging buffer for reading
        let buffer_slice = staging_buffer.slice(..byte_size);
        let (sender, receiver) = tokio::sync::oneshot::channel();

        buffer_slice.map_async(MapMode::Read, move |result| {
            let _ = sender.send(result);
        });

        // Poll the device to complete the mapping operation
        let _ = device.poll(PollType::Wait);

        // Wait for the mapping to complete
        let map_result = receiver
            .await
            .map_err(|_| GupError::buffer_error("Buffer mapping callback was dropped"))?
            .map_err(|e| {
                GupError::buffer_error(format!("Failed to map buffer for reading: {:?}", e))
            });

        if let Err(e) = map_result {
            // Return buffer to pool even on error to prevent leaks
            pool.deallocate_raw(staging_buffer, BufferType::Staging, size_class);
            return Err(e);
        }

        // Read the data
        let data = buffer_slice.get_mapped_range();
        let result: Vec<T> = bytemuck::cast_slice(&data).to_vec();

        // Clean up and return buffer to pool
        drop(data);
        staging_buffer.unmap();
        pool.deallocate_raw(staging_buffer, BufferType::Staging, size_class);

        Ok(result)
    }

    /// Check if this buffer supports download operations.
    ///
    /// Returns true if the buffer was created with COPY_SRC usage, which is
    /// required for copying data from this buffer to a staging buffer.
    pub fn can_download(&self) -> bool {
        self.usage.contains(BufferUsages::COPY_SRC)
    }

    /// Get the raw wgpu buffer for shader binding.
    pub fn raw_buffer(&self) -> &Buffer {
        &self.buffer
    }

    /// Get the wgpu buffer for shader binding (alias for raw_buffer).
    pub fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    /// Get the current number of elements in the buffer.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Get the maximum number of elements the buffer can hold.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Check if the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get the buffer type.
    pub fn buffer_type(&self) -> BufferType {
        self.buffer_type
    }

    /// Clear the buffer contents without deallocating.
    ///
    /// This method resets the length to zero, effectively clearing the buffer
    /// without deallocating the GPU memory.
    pub fn clear(&mut self) {
        self.len = 0;
    }

    /// Resize the buffer to accommodate at least the specified capacity.
    fn resize(&mut self, device: &Device, queue: &Queue, min_capacity: usize) -> GupResult<()> {
        // Use 1.5x growth factor for optimal memory vs. performance trade-off
        let new_capacity = ((min_capacity as f64 * 1.5) as usize).max(min_capacity);
        let new_size = Self::calculate_buffer_size(new_capacity, self.buffer_type);

        let new_buffer = device.create_buffer(&BufferDescriptor {
            label: Some(&format!("{:?}_buffer_resized", self.buffer_type)),
            size: new_size,
            usage: self.usage,
            mapped_at_creation: false,
        });

        // Copy existing data if any
        if self.len > 0 {
            let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
                label: Some("buffer_resize_encoder"),
            });

            encoder.copy_buffer_to_buffer(
                &self.buffer,
                0,
                &new_buffer,
                0,
                (self.len * std::mem::size_of::<T>()) as u64,
            );

            let submission_index = queue.submit(Some(encoder.finish()));

            // Wait for the copy operation to complete before proceeding
            // This prevents race conditions in benchmarks and tight loops
            let _ = device.poll(PollType::WaitForSubmissionIndex(submission_index));
        }

        self.buffer = new_buffer;
        self.capacity = new_capacity;
        Ok(())
    }

    /// Calculate the appropriate buffer size with alignment considerations.
    fn calculate_buffer_size(capacity: usize, buffer_type: BufferType) -> u64 {
        let element_size = std::mem::size_of::<T>() as u64;
        let total_size = capacity as u64 * element_size;
        let alignment = buffer_type.alignment();

        // Round up to the nearest alignment boundary
        total_size.div_ceil(alignment) * alignment
    }
}

/// Statistics for buffer pool allocation tracking.
#[derive(Debug, Default, Clone)]
pub struct AllocationStats {
    /// Total number of buffers allocated
    pub total_allocated: usize,
    /// Total number of buffers deallocated
    pub total_deallocated: usize,
    /// Current number of active buffers
    pub active_buffers: usize,
    /// Current number of pooled buffers
    pub pooled_buffers: usize,
    /// Total bytes allocated across all buffers
    pub total_bytes_allocated: u64,
    /// Pool hit rate (0.0 to 1.0)
    pub pool_hit_rate: f32,
    /// Pool misses (allocations that required new buffer creation)
    pub pool_misses: usize,
    /// Pool hits (allocations satisfied from pool)
    pub pool_hits: usize,
}

impl AllocationStats {
    /// Calculate the pool efficiency as a percentage.
    pub fn pool_efficiency(&self) -> f32 {
        if self.total_allocated == 0 {
            0.0
        } else {
            (self.pooled_buffers as f32 / self.total_allocated as f32) * 100.0
        }
    }

    /// Calculate the current pool hit rate.
    pub fn hit_rate(&self) -> f32 {
        let total_requests = self.pool_hits + self.pool_misses;
        if total_requests == 0 {
            0.0
        } else {
            (self.pool_hits as f32 / total_requests as f32) * 100.0
        }
    }
}

/// Configuration for buffer pool behavior.
#[derive(Debug, Clone)]
pub struct BufferPoolConfig {
    /// Maximum number of buffers to keep in each pool
    pub max_buffers_per_pool: usize,
    /// Maximum total GPU memory to use for pooled buffers (in bytes)
    pub max_total_memory: Option<u64>,
    /// Time after which unused buffers are evicted
    pub eviction_timeout: Duration,
    /// Whether to enable LRU eviction
    pub enable_lru: bool,
    /// Memory pressure thresholds
    pub pressure_thresholds: PressureThresholds,
    /// Maximum size of allocation history
    pub usage_history_size: usize,
    /// Whether to enable adaptive pool sizing
    pub enable_adaptive_sizing: bool,
}

impl Default for BufferPoolConfig {
    fn default() -> Self {
        Self {
            max_buffers_per_pool: 10,
            max_total_memory: Some(256 * 1024 * 1024), // 256 MB default limit
            eviction_timeout: Duration::from_secs(60), // 1 minute
            enable_lru: true,
            pressure_thresholds: PressureThresholds::default(),
            usage_history_size: 1000,
            enable_adaptive_sizing: true,
        }
    }
}

/// Entry in the LRU cache for tracking buffer usage.
#[derive(Debug)]
struct PooledBufferEntry {
    buffer: Buffer,
    last_used: Instant,
    size: u64,
}

/// Memory pressure levels for adaptive cleanup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PressureLevel {
    /// Normal operation - no memory pressure
    Normal,
    /// Warning level (>80%) - gentle cleanup
    Warning,
    /// Critical level (>90%) - aggressive cleanup
    Critical,
    /// Emergency level (>95%) - emergency cleanup
    Emergency,
}

/// Thresholds for memory pressure levels.
#[derive(Debug, Clone)]
pub struct PressureThresholds {
    /// Warning level threshold (0.0 to 1.0)
    pub warning_level: f32,
    /// Critical level threshold (0.0 to 1.0)
    pub critical_level: f32,
    /// Emergency level threshold (0.0 to 1.0)
    pub emergency_level: f32,
}

impl Default for PressureThresholds {
    fn default() -> Self {
        Self {
            warning_level: 0.8,
            critical_level: 0.9,
            emergency_level: 0.95,
        }
    }
}

/// Operation type for tracking buffer pool events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoolOperation {
    /// Buffer allocated from pool (hit)
    Hit,
    /// Buffer created because pool was empty (miss)
    Miss,
    /// Buffer returned to pool
    Deallocate,
}

/// Event recorded in the allocation history.
#[derive(Debug, Clone)]
pub struct BufferAllocationEvent {
    /// When the event occurred
    pub timestamp: Instant,
    /// Type of buffer involved
    pub buffer_type: BufferType,
    /// Size class of the buffer
    pub size: usize,
    /// Operation that occurred
    pub operation: PoolOperation,
}

/// Statistics for tracking usage frequency of a specific size class.
#[derive(Debug, Clone)]
pub struct FrequencyStats {
    /// Total number of accesses
    pub count: u64,
    /// When this size was last accessed
    pub last_access: Instant,
    /// Average interval between accesses
    pub access_interval_avg: Duration,
    /// Score for retention priority (higher = keep longer)
    pub retention_score: f32,
}

impl FrequencyStats {
    fn new() -> Self {
        Self {
            count: 0,
            last_access: Instant::now(),
            access_interval_avg: Duration::from_secs(0),
            retention_score: 0.0,
        }
    }

    /// Update stats with a new access.
    fn record_access(&mut self) {
        let now = Instant::now();
        let interval = now.duration_since(self.last_access);

        // Update moving average of access interval
        if self.count > 0 {
            let old_avg_secs = self.access_interval_avg.as_secs_f32();
            let new_interval_secs = interval.as_secs_f32();
            let new_avg_secs = (old_avg_secs * (self.count as f32) + new_interval_secs)
                / ((self.count + 1) as f32);
            self.access_interval_avg = Duration::from_secs_f32(new_avg_secs);
        } else {
            self.access_interval_avg = interval;
        }

        self.count += 1;
        self.last_access = now;

        // Update retention score based on frequency and recency
        // Higher count and shorter interval = higher score
        let frequency_factor = (self.count as f32).ln();
        let recency_factor = 1.0 / (interval.as_secs_f32() + 1.0);
        self.retention_score = frequency_factor * recency_factor;
    }

    /// Check if this size class has been idle for too long.
    fn is_idle(&self, threshold: Duration) -> bool {
        Instant::now().duration_since(self.last_access) > threshold
    }
}

/// Tracks buffer usage patterns to enable adaptive pool management.
#[derive(Debug)]
pub struct UsagePatternTracker {
    /// Recent allocation history (circular buffer via VecDeque)
    allocation_history: VecDeque<BufferAllocationEvent>,
    /// Frequency statistics per buffer type and size
    size_frequency: HashMap<(BufferType, usize), FrequencyStats>,
    /// Maximum history size
    max_history_size: usize,
}

impl UsagePatternTracker {
    fn new(max_history_size: usize) -> Self {
        Self {
            allocation_history: VecDeque::with_capacity(max_history_size),
            size_frequency: HashMap::new(),
            max_history_size,
        }
    }

    /// Record an allocation event.
    fn record_event(&mut self, event: BufferAllocationEvent) {
        // Add to history
        if self.allocation_history.len() >= self.max_history_size {
            self.allocation_history.pop_front();
        }
        self.allocation_history.push_back(event.clone());

        // Update frequency stats
        let key = (event.buffer_type, event.size);
        self.size_frequency
            .entry(key)
            .or_insert_with(FrequencyStats::new)
            .record_access();
    }

    /// Get the most frequently used size classes.
    fn popular_sizes(&self, limit: usize) -> Vec<((BufferType, usize), u64)> {
        let mut sizes: Vec<_> = self
            .size_frequency
            .iter()
            .map(|(key, stats)| (*key, stats.count))
            .collect();
        sizes.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
        sizes.into_iter().take(limit).collect()
    }

    /// Get retention score for a size class.
    fn retention_score(&self, buffer_type: BufferType, size: usize) -> f32 {
        self.size_frequency
            .get(&(buffer_type, size))
            .map(|stats| stats.retention_score)
            .unwrap_or(0.0)
    }

    /// Get sizes that haven't been used recently.
    fn idle_sizes(&self, threshold: Duration) -> Vec<(BufferType, usize)> {
        self.size_frequency
            .iter()
            .filter(|(_, stats)| stats.is_idle(threshold))
            .map(|(key, _)| *key)
            .collect()
    }

    /// Get allocation statistics for the last N events.
    fn recent_stats(&self, last_n: usize) -> (usize, usize) {
        let events: Vec<_> = self.allocation_history.iter().rev().take(last_n).collect();

        let hits = events
            .iter()
            .filter(|e| e.operation == PoolOperation::Hit)
            .count();
        let misses = events
            .iter()
            .filter(|e| e.operation == PoolOperation::Miss)
            .count();

        (hits, misses)
    }
}

/// Buffer pool for efficient resource reuse and memory management with LRU eviction.
#[derive(Debug)]
pub struct BufferPool {
    pools: HashMap<(BufferType, usize), VecDeque<PooledBufferEntry>>,
    device: Arc<Device>,
    allocation_stats: AllocationStats,
    config: BufferPoolConfig,
    usage_tracker: UsagePatternTracker,
}

impl BufferPool {
    /// Create a new buffer pool with the given device.
    pub fn new(device: Arc<Device>) -> Self {
        Self::with_config(device, BufferPoolConfig::default())
    }

    /// Create a new buffer pool with custom configuration.
    pub fn with_config(device: Arc<Device>, config: BufferPoolConfig) -> Self {
        let usage_tracker = UsagePatternTracker::new(config.usage_history_size);
        Self {
            pools: HashMap::new(),
            device,
            allocation_stats: AllocationStats::default(),
            usage_tracker,
            config,
        }
    }

    /// Allocate a buffer from the pool or create a new one.
    pub fn allocate<T>(&mut self, buffer_type: BufferType, capacity: usize) -> GpuBuffer<T>
    where
        T: bytemuck::Pod + bytemuck::Zeroable,
    {
        let size_class = self.calculate_size_class(capacity);
        let key = (buffer_type, size_class);

        let (buffer, operation) = if let Some(pool) = self.pools.get_mut(&key) {
            if let Some(entry) = pool.pop_front() {
                self.allocation_stats.pooled_buffers -= 1;
                self.allocation_stats.pool_hits += 1;
                (entry.buffer, PoolOperation::Hit)
            } else {
                self.allocation_stats.pool_misses += 1;
                (
                    self.create_new_buffer(buffer_type, size_class),
                    PoolOperation::Miss,
                )
            }
        } else {
            self.allocation_stats.pool_misses += 1;
            (
                self.create_new_buffer(buffer_type, size_class),
                PoolOperation::Miss,
            )
        };

        // Track allocation event for adaptive learning
        if self.config.enable_adaptive_sizing {
            self.usage_tracker.record_event(BufferAllocationEvent {
                timestamp: Instant::now(),
                buffer_type,
                size: size_class,
                operation,
            });
        }

        self.allocation_stats.total_allocated += 1;
        self.allocation_stats.active_buffers += 1;

        GpuBuffer {
            buffer,
            capacity: size_class,
            len: 0,
            buffer_type,
            usage: buffer_type.usage_flags(),
            _phantom: PhantomData,
        }
    }

    /// Allocate a raw `wgpu::Buffer` from the pool, sized in bytes.
    ///
    /// Unlike [`allocate`](Self::allocate), which returns a typed
    /// [`GpuBuffer<T>`], this returns the underlying `wgpu::Buffer` directly.
    /// This is useful when the caller works with untyped byte slices (e.g.
    /// instance buffers that store `bytemuck::cast_slice` output).
    ///
    /// The caller must track `buffer_type` and the returned byte capacity
    /// (rounded up to a power-of-two size class) so that the buffer can later
    /// be returned via [`deallocate_raw`](Self::deallocate_raw).
    pub fn allocate_raw(&mut self, buffer_type: BufferType, byte_size: usize) -> (Buffer, usize) {
        let size_class = self.calculate_size_class(byte_size.max(1));
        let key = (buffer_type, size_class);

        let (buffer, operation) = if let Some(pool) = self.pools.get_mut(&key) {
            if let Some(entry) = pool.pop_front() {
                self.allocation_stats.pooled_buffers -= 1;
                self.allocation_stats.pool_hits += 1;
                (entry.buffer, PoolOperation::Hit)
            } else {
                self.allocation_stats.pool_misses += 1;
                (
                    self.create_new_buffer(buffer_type, size_class),
                    PoolOperation::Miss,
                )
            }
        } else {
            self.allocation_stats.pool_misses += 1;
            (
                self.create_new_buffer(buffer_type, size_class),
                PoolOperation::Miss,
            )
        };

        if self.config.enable_adaptive_sizing {
            self.usage_tracker.record_event(BufferAllocationEvent {
                timestamp: Instant::now(),
                buffer_type,
                size: size_class,
                operation,
            });
        }

        self.allocation_stats.total_allocated += 1;
        self.allocation_stats.active_buffers += 1;

        (buffer, size_class)
    }

    /// Return a raw buffer to the pool for reuse.
    ///
    /// `byte_capacity` must be the size-class value returned by
    /// [`allocate_raw`](Self::allocate_raw).
    pub fn deallocate_raw(
        &mut self,
        buffer: Buffer,
        buffer_type: BufferType,
        byte_capacity: usize,
    ) {
        let key = (buffer_type, byte_capacity);

        if self.config.enable_adaptive_sizing {
            self.usage_tracker.record_event(BufferAllocationEvent {
                timestamp: Instant::now(),
                buffer_type,
                size: byte_capacity,
                operation: PoolOperation::Deallocate,
            });
        }

        let size = self.calculate_buffer_size::<u8>(byte_capacity, buffer_type);
        let entry = PooledBufferEntry {
            buffer,
            last_used: Instant::now(),
            size,
        };

        self.pools.entry(key).or_default().push_back(entry);

        self.allocation_stats.total_deallocated += 1;
        self.allocation_stats.active_buffers -= 1;
        self.allocation_stats.pooled_buffers += 1;

        self.check_memory_pressure();
    }

    /// Return a buffer to the pool for reuse.
    pub fn deallocate<T>(&mut self, mut buffer: GpuBuffer<T>) {
        let key = (buffer.buffer_type, buffer.capacity);

        // Reset buffer state
        buffer.len = 0;

        // Track deallocation event for adaptive learning
        if self.config.enable_adaptive_sizing {
            self.usage_tracker.record_event(BufferAllocationEvent {
                timestamp: Instant::now(),
                buffer_type: buffer.buffer_type,
                size: buffer.capacity,
                operation: PoolOperation::Deallocate,
            });
        }

        let size = self.calculate_buffer_size::<T>(buffer.capacity, buffer.buffer_type);
        let entry = PooledBufferEntry {
            buffer: buffer.buffer,
            last_used: Instant::now(),
            size,
        };

        // Add to appropriate pool (at the back for LRU)
        self.pools.entry(key).or_default().push_back(entry);

        self.allocation_stats.total_deallocated += 1;
        self.allocation_stats.active_buffers -= 1;
        self.allocation_stats.pooled_buffers += 1;

        // Check if we need to evict based on memory pressure
        self.check_memory_pressure();
    }

    /// Check for memory pressure and evict buffers if necessary.
    fn check_memory_pressure(&mut self) {
        if let Some(max_memory) = self.config.max_total_memory {
            let current_pooled_memory = self.calculate_pooled_memory();
            let pressure_level = self.calculate_pressure_level(current_pooled_memory, max_memory);

            if self.config.enable_adaptive_sizing {
                // Use adaptive cleanup based on pressure level
                self.intelligent_cleanup(pressure_level);
            } else if current_pooled_memory > max_memory {
                // Fall back to simple eviction
                self.evict_lru_buffers(current_pooled_memory - max_memory);
            }
        }
    }

    /// Calculate the current memory pressure level.
    fn calculate_pressure_level(&self, current: u64, max: u64) -> PressureLevel {
        let usage_ratio = current as f32 / max as f32;
        let thresholds = &self.config.pressure_thresholds;

        if usage_ratio >= thresholds.emergency_level {
            PressureLevel::Emergency
        } else if usage_ratio >= thresholds.critical_level {
            PressureLevel::Critical
        } else if usage_ratio >= thresholds.warning_level {
            PressureLevel::Warning
        } else {
            PressureLevel::Normal
        }
    }

    /// Intelligent cleanup based on memory pressure level and usage patterns.
    fn intelligent_cleanup(&mut self, pressure_level: PressureLevel) {
        match pressure_level {
            PressureLevel::Normal => (),
            PressureLevel::Warning => self.gentle_cleanup(),
            PressureLevel::Critical => self.aggressive_cleanup(),
            PressureLevel::Emergency => self.emergency_cleanup(),
        }
    }

    /// Gentle cleanup - remove buffers unused for >30 minutes, keep frequently used sizes.
    fn gentle_cleanup(&mut self) {
        let idle_threshold = Duration::from_secs(30 * 60); // 30 minutes
        let idle_sizes = self.usage_tracker.idle_sizes(idle_threshold);

        // Remove idle buffers
        for (buffer_type, size) in idle_sizes {
            let key = (buffer_type, size);
            if let Some(pool) = self.pools.get_mut(&key) {
                let removed = pool.len();
                pool.clear();
                self.allocation_stats.pooled_buffers -= removed;
            }
        }

        // Remove empty pools
        self.pools.retain(|_, pool| !pool.is_empty());
    }

    /// Aggressive cleanup - remove buffers unused for >10 minutes.
    fn aggressive_cleanup(&mut self) {
        let idle_threshold = Duration::from_secs(10 * 60); // 10 minutes
        let idle_sizes = self.usage_tracker.idle_sizes(idle_threshold);

        // Remove idle buffers
        for (buffer_type, size) in idle_sizes {
            let key = (buffer_type, size);
            if let Some(pool) = self.pools.get_mut(&key) {
                let removed = pool.len();
                pool.clear();
                self.allocation_stats.pooled_buffers -= removed;
            }
        }

        // Also evict some LRU buffers to ensure we free enough memory
        if let Some(max_memory) = self.config.max_total_memory {
            let current = self.calculate_pooled_memory();
            if current > max_memory {
                let target = (current - max_memory).max(max_memory / 10); // Free at least 10%
                self.evict_lru_buffers(target);
            }
        }

        self.pools.retain(|_, pool| !pool.is_empty());
    }

    /// Emergency cleanup - remove all buffers unused for >1 minute.
    fn emergency_cleanup(&mut self) {
        let idle_threshold = Duration::from_secs(60); // 1 minute
        let now = Instant::now();

        // Remove all idle buffers aggressively
        for pool in self.pools.values_mut() {
            let original_len = pool.len();
            pool.retain(|entry| now.duration_since(entry.last_used) < idle_threshold);
            let removed = original_len - pool.len();
            self.allocation_stats.pooled_buffers -= removed;
        }

        // If still over limit, evict everything
        if let Some(max_memory) = self.config.max_total_memory {
            let current = self.calculate_pooled_memory();
            if current > max_memory {
                // Clear all pools in emergency
                for pool in self.pools.values_mut() {
                    self.allocation_stats.pooled_buffers -= pool.len();
                    pool.clear();
                }
            }
        }

        self.pools.retain(|_, pool| !pool.is_empty());
    }

    /// Calculate total memory used by pooled buffers.
    fn calculate_pooled_memory(&self) -> u64 {
        self.pools
            .values()
            .flat_map(|pool| pool.iter())
            .map(|entry| entry.size)
            .sum()
    }

    /// Evict buffers using LRU strategy until target_bytes are freed.
    fn evict_lru_buffers(&mut self, target_bytes: u64) {
        if !self.config.enable_lru {
            return;
        }

        let mut freed_bytes = 0u64;
        let mut pools_to_clean = Vec::new();

        // Collect all entries with their pool keys, sorted by last_used (oldest first)
        let mut all_entries: Vec<((BufferType, usize), usize, Instant)> = Vec::new();
        for (key, pool) in &self.pools {
            for (idx, entry) in pool.iter().enumerate() {
                all_entries.push((*key, idx, entry.last_used));
            }
        }
        all_entries.sort_by_key(|(_, _, last_used)| *last_used);

        // Evict oldest entries until we've freed enough memory
        for (key, _idx, _) in all_entries {
            if freed_bytes >= target_bytes {
                break;
            }

            if let Some(pool) = self.pools.get_mut(&key)
                && let Some(entry) = pool.pop_front()
            {
                freed_bytes += entry.size;
                self.allocation_stats.pooled_buffers -= 1;

                if pool.is_empty() {
                    pools_to_clean.push(key);
                }
            }
        }

        // Remove empty pools
        for key in pools_to_clean {
            self.pools.remove(&key);
        }
    }

    /// Clean up unused buffers to free memory.
    /// Removes buffers that haven't been used for longer than eviction_timeout.
    pub fn cleanup_unused(&mut self) {
        let now = Instant::now();
        let eviction_timeout = self.config.eviction_timeout;

        for pool in self.pools.values_mut() {
            // Remove buffers that haven't been used recently
            let original_len = pool.len();
            pool.retain(|entry| now.duration_since(entry.last_used) < eviction_timeout);
            let removed = original_len - pool.len();
            self.allocation_stats.pooled_buffers -= removed;
        }

        // Also enforce max buffers per pool
        // Remove from the front (oldest) to keep the most recently used
        for pool in self.pools.values_mut() {
            let excess = pool.len().saturating_sub(self.config.max_buffers_per_pool);
            if excess > 0 {
                // Drain from the front (oldest in LRU)
                for _ in 0..excess {
                    pool.pop_front();
                }
                self.allocation_stats.pooled_buffers -= excess;
            }
        }

        // Remove empty pools
        self.pools.retain(|_, pool| !pool.is_empty());
    }

    /// Get current allocation statistics.
    pub fn get_stats(&self) -> &AllocationStats {
        &self.allocation_stats
    }

    /// Get current configuration.
    pub fn config(&self) -> &BufferPoolConfig {
        &self.config
    }

    /// Update configuration (affects future allocations).
    pub fn set_config(&mut self, config: BufferPoolConfig) {
        self.config = config;
    }

    /// Check if memory usage is approaching configured limits.
    /// Returns Some(percentage) if max_total_memory is configured, None otherwise.
    pub fn memory_usage_percentage(&self) -> Option<f32> {
        self.config.max_total_memory.map(|max| {
            let current = self.calculate_pooled_memory();
            (current as f32 / max as f32) * 100.0
        })
    }

    /// Returns true if memory usage is above the given threshold percentage.
    pub fn is_memory_pressure(&self, threshold_percent: f32) -> bool {
        self.memory_usage_percentage()
            .map(|usage| usage > threshold_percent)
            .unwrap_or(false)
    }

    /// Get the current memory pressure level.
    pub fn current_pressure_level(&self) -> PressureLevel {
        if let Some(max_memory) = self.config.max_total_memory {
            let current = self.calculate_pooled_memory();
            self.calculate_pressure_level(current, max_memory)
        } else {
            PressureLevel::Normal
        }
    }

    /// Get the most popular buffer sizes.
    pub fn popular_sizes(&self, limit: usize) -> Vec<((BufferType, usize), u64)> {
        self.usage_tracker.popular_sizes(limit)
    }

    /// Get recent hit/miss statistics.
    pub fn recent_hit_rate(&self, last_n: usize) -> f32 {
        let (hits, misses) = self.usage_tracker.recent_stats(last_n);
        let total = hits + misses;
        if total == 0 {
            0.0
        } else {
            (hits as f32 / total as f32) * 100.0
        }
    }

    /// Get retention score for a specific buffer size.
    pub fn retention_score(&self, buffer_type: BufferType, size: usize) -> f32 {
        self.usage_tracker.retention_score(buffer_type, size)
    }

    /// Calculate the size class for a given capacity (power of 2 rounding up).
    fn calculate_size_class(&self, capacity: usize) -> usize {
        if capacity == 0 {
            return 1;
        }

        // Round up to the next power of 2
        let mut size_class = 1;
        while size_class < capacity {
            size_class *= 2;
        }
        size_class
    }

    /// Create a new buffer with the specified parameters.
    fn create_new_buffer(&mut self, buffer_type: BufferType, capacity: usize) -> Buffer {
        let usage = buffer_type.usage_flags();
        let size = self.calculate_buffer_size::<u8>(capacity, buffer_type);

        let buffer = self.device.create_buffer(&BufferDescriptor {
            label: Some(&format!("{buffer_type:?}_pooled_buffer")),
            size,
            usage,
            mapped_at_creation: false,
        });

        self.allocation_stats.total_bytes_allocated += size;
        buffer
    }

    /// Calculate buffer size with proper alignment.
    fn calculate_buffer_size<T>(&self, capacity: usize, buffer_type: BufferType) -> u64 {
        let element_size = std::mem::size_of::<T>() as u64;
        let total_size = capacity as u64 * element_size;
        let alignment = buffer_type.alignment();

        total_size.div_ceil(alignment) * alignment
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::RenderContext;

    async fn create_test_context() -> RenderContext {
        RenderContext::new().await.unwrap()
    }

    #[tokio::test]
    async fn test_buffer_creation() {
        let context = create_test_context().await;
        let buffer = GpuBuffer::<f32>::new(context.device(), BufferType::Vertex, 1000);

        assert_eq!(buffer.capacity(), 1000);
        assert_eq!(buffer.len(), 0);
        assert!(buffer.is_empty());
        assert_eq!(buffer.buffer_type(), BufferType::Vertex);
    }

    #[tokio::test]
    async fn test_buffer_upload() {
        let context = create_test_context().await;
        let mut buffer = GpuBuffer::new(context.device(), BufferType::Vertex, 100);

        let data = vec![1.0f32, 2.0, 3.0, 4.0];
        buffer
            .upload(context.device(), context.queue(), &data)
            .unwrap();

        assert_eq!(buffer.len(), 4);
        assert!(!buffer.is_empty());
    }

    #[tokio::test]
    async fn test_buffer_resize() {
        let context = create_test_context().await;
        let mut buffer = GpuBuffer::new(context.device(), BufferType::Vertex, 10);

        let large_data = vec![0.0f32; 100];
        buffer
            .upload(context.device(), context.queue(), &large_data)
            .unwrap();

        assert!(buffer.capacity() >= 100);
        assert_eq!(buffer.len(), 100);
    }

    #[tokio::test]
    async fn test_buffer_range_upload() {
        let context = create_test_context().await;
        let mut buffer = GpuBuffer::new(context.device(), BufferType::Storage, 100);

        let data1 = vec![1.0f32, 2.0, 3.0];
        let data2 = vec![4.0f32, 5.0, 6.0];

        buffer.upload_range(context.queue(), &data1, 0).unwrap();
        buffer.upload_range(context.queue(), &data2, 10).unwrap();

        assert_eq!(buffer.len(), 13); // max(3, 10+3)
    }

    #[tokio::test]
    async fn test_buffer_pool_allocation() {
        let context = create_test_context().await;
        let device = Arc::new(context.device().clone());
        let mut pool = BufferPool::new(device);

        let buffer1 = pool.allocate::<f32>(BufferType::Vertex, 100);
        let _buffer2 = pool.allocate::<f32>(BufferType::Vertex, 100);

        assert_eq!(pool.get_stats().active_buffers, 2);
        assert_eq!(pool.get_stats().total_allocated, 2);

        pool.deallocate(buffer1);
        assert_eq!(pool.get_stats().active_buffers, 1);
        assert_eq!(pool.get_stats().pooled_buffers, 1);

        // Should reuse the pooled buffer
        let _buffer3 = pool.allocate::<f32>(BufferType::Vertex, 100);
        assert_eq!(pool.get_stats().pooled_buffers, 0);
    }

    #[tokio::test]
    async fn test_buffer_type_usage_flags() {
        assert!(
            BufferType::Vertex
                .usage_flags()
                .contains(BufferUsages::VERTEX)
        );
        assert!(
            BufferType::Instance
                .usage_flags()
                .contains(BufferUsages::VERTEX)
        );
        assert!(
            BufferType::Uniform
                .usage_flags()
                .contains(BufferUsages::UNIFORM)
        );
        assert!(
            BufferType::Storage
                .usage_flags()
                .contains(BufferUsages::STORAGE)
        );
    }

    #[tokio::test]
    async fn test_size_class_calculation() {
        let context = create_test_context().await;
        let device = Arc::new(context.device().clone());
        let pool = BufferPool::new(device);

        assert_eq!(pool.calculate_size_class(0), 1);
        assert_eq!(pool.calculate_size_class(1), 1);
        assert_eq!(pool.calculate_size_class(2), 2);
        assert_eq!(pool.calculate_size_class(3), 4);
        assert_eq!(pool.calculate_size_class(100), 128);
        assert_eq!(pool.calculate_size_class(1000), 1024);
    }

    #[tokio::test]
    async fn test_buffer_pool_max_buffers_per_pool() {
        let context = create_test_context().await;
        let device = Arc::new(context.device().clone());

        let config = BufferPoolConfig {
            max_buffers_per_pool: 3,
            max_total_memory: None,
            eviction_timeout: Duration::from_secs(3600), // Long timeout so buffers don't expire
            enable_lru: true,
            ..Default::default()
        };
        let mut pool = BufferPool::with_config(device, config);

        // Allocate 5 buffers first (don't deallocate yet)
        let mut buffers = Vec::new();
        for _ in 0..5 {
            buffers.push(pool.allocate::<f32>(BufferType::Vertex, 100));
        }

        // Now deallocate them all
        for buffer in buffers {
            pool.deallocate(buffer);
        }

        // Pool should have 5 buffers
        assert_eq!(pool.get_stats().pooled_buffers, 5);

        // Cleanup should reduce to max_buffers_per_pool (3)
        pool.cleanup_unused();
        assert_eq!(pool.get_stats().pooled_buffers, 3);
    }

    #[tokio::test]
    async fn test_buffer_pool_timeout_eviction() {
        let context = create_test_context().await;
        let device = Arc::new(context.device().clone());

        let config = BufferPoolConfig {
            max_buffers_per_pool: 10,
            max_total_memory: None,
            eviction_timeout: Duration::from_millis(1), // Very short timeout
            enable_lru: true,
            ..Default::default()
        };
        let mut pool = BufferPool::with_config(device, config);

        // Allocate 3 buffers first (don't deallocate yet)
        let mut buffers = Vec::new();
        for _ in 0..3 {
            buffers.push(pool.allocate::<f32>(BufferType::Vertex, 100));
        }

        // Deallocate them all
        for buffer in buffers {
            pool.deallocate(buffer);
        }

        // Pool should have 3 buffers
        assert_eq!(pool.get_stats().pooled_buffers, 3);

        // Wait for timeout to expire
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Cleanup should remove all expired buffers
        pool.cleanup_unused();
        assert_eq!(pool.get_stats().pooled_buffers, 0);
    }

    #[tokio::test]
    async fn test_buffer_pool_memory_pressure() {
        let context = create_test_context().await;
        let device = Arc::new(context.device().clone());

        let config = BufferPoolConfig {
            max_buffers_per_pool: 100,
            max_total_memory: Some(1024), // Very small limit to trigger eviction
            eviction_timeout: Duration::from_secs(60),
            enable_lru: true,
            ..Default::default()
        };
        let mut pool = BufferPool::with_config(device, config);

        // Allocate several large buffers
        let buffers: Vec<_> = (0..5)
            .map(|_| pool.allocate::<f32>(BufferType::Vertex, 100))
            .collect();

        // Deallocate them
        for buffer in buffers {
            pool.deallocate(buffer);
        }

        // Memory pressure should have triggered eviction
        assert!(pool.get_stats().pooled_buffers < 5);
    }

    #[tokio::test]
    async fn test_buffer_pool_hit_rate() {
        let context = create_test_context().await;
        let device = Arc::new(context.device().clone());
        let mut pool = BufferPool::new(device);

        // First allocation is a miss
        let buffer1 = pool.allocate::<f32>(BufferType::Vertex, 100);
        assert_eq!(pool.get_stats().pool_misses, 1);
        assert_eq!(pool.get_stats().pool_hits, 0);

        // Deallocate and reallocate - should be a hit
        pool.deallocate(buffer1);
        let _buffer2 = pool.allocate::<f32>(BufferType::Vertex, 100);
        assert_eq!(pool.get_stats().pool_hits, 1);
        assert_eq!(pool.get_stats().hit_rate(), 50.0); // 1 hit out of 2 requests
    }

    #[tokio::test]
    async fn test_buffer_pool_memory_usage_percentage() {
        let context = create_test_context().await;
        let device = Arc::new(context.device().clone());

        let config = BufferPoolConfig {
            max_buffers_per_pool: 10,
            max_total_memory: Some(10_000),
            eviction_timeout: Duration::from_secs(60),
            enable_lru: true,
            ..Default::default()
        };
        let mut pool = BufferPool::with_config(device, config);

        // No buffers yet
        assert_eq!(pool.memory_usage_percentage().unwrap(), 0.0);

        // Allocate and deallocate a buffer
        let buffer = pool.allocate::<f32>(BufferType::Vertex, 100);
        pool.deallocate(buffer);

        // Should have some memory usage now
        assert!(pool.memory_usage_percentage().unwrap() > 0.0);
    }

    #[tokio::test]
    async fn test_buffer_pool_config_update() {
        let context = create_test_context().await;
        let device = Arc::new(context.device().clone());
        let mut pool = BufferPool::new(device);

        // Default config
        assert_eq!(pool.config().max_buffers_per_pool, 10);

        // Update config
        let new_config = BufferPoolConfig {
            max_buffers_per_pool: 5,
            ..Default::default()
        };
        pool.set_config(new_config);

        assert_eq!(pool.config().max_buffers_per_pool, 5);
    }

    #[tokio::test]
    async fn test_buffer_download() {
        let context = create_test_context().await;
        let mut buffer = GpuBuffer::new(context.device(), BufferType::Vertex, 100);

        // Upload test data
        let data = vec![1.0f32, 2.0, 3.0, 4.0, 5.0];
        buffer
            .upload(context.device(), context.queue(), &data)
            .unwrap();

        // Download and verify
        let downloaded = buffer
            .download(context.device(), context.queue())
            .await
            .unwrap();

        assert_eq!(downloaded, data);
    }

    #[tokio::test]
    async fn test_buffer_download_range() {
        let context = create_test_context().await;
        let mut buffer = GpuBuffer::new(context.device(), BufferType::Storage, 100);

        // Upload test data
        let data: Vec<f32> = (0..50).map(|i| i as f32).collect();
        buffer
            .upload(context.device(), context.queue(), &data)
            .unwrap();

        // Download a range
        let downloaded = buffer
            .download_range(context.device(), context.queue(), 10, 10)
            .await
            .unwrap();

        assert_eq!(downloaded.len(), 10);
        assert_eq!(downloaded, &data[10..20]);
    }

    #[tokio::test]
    async fn test_buffer_download_empty() {
        let context = create_test_context().await;
        let buffer = GpuBuffer::<f32>::new(context.device(), BufferType::Vertex, 100);

        // Download from empty buffer should return empty vec
        let downloaded = buffer
            .download(context.device(), context.queue())
            .await
            .unwrap();

        assert!(downloaded.is_empty());
    }

    #[tokio::test]
    async fn test_buffer_download_range_invalid() {
        let context = create_test_context().await;
        let mut buffer = GpuBuffer::new(context.device(), BufferType::Vertex, 100);

        let data = vec![1.0f32, 2.0, 3.0];
        buffer
            .upload(context.device(), context.queue(), &data)
            .unwrap();

        // Try to download beyond buffer length
        let result = buffer
            .download_range(context.device(), context.queue(), 0, 10)
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_buffer_download_range_offset() {
        let context = create_test_context().await;
        let mut buffer = GpuBuffer::new(context.device(), BufferType::Storage, 100);

        let data: Vec<u32> = vec![100, 200, 300, 400, 500];
        buffer
            .upload(context.device(), context.queue(), &data)
            .unwrap();

        // Download from offset
        let downloaded = buffer
            .download_range(context.device(), context.queue(), 2, 2)
            .await
            .unwrap();

        assert_eq!(downloaded, vec![300, 400]);
    }

    #[tokio::test]
    async fn test_buffer_can_download() {
        let context = create_test_context().await;

        // All buffer types should support download (they have COPY_SRC)
        let vertex_buffer = GpuBuffer::<f32>::new(context.device(), BufferType::Vertex, 10);
        assert!(vertex_buffer.can_download());

        let instance_buffer = GpuBuffer::<f32>::new(context.device(), BufferType::Instance, 10);
        assert!(instance_buffer.can_download());

        let uniform_buffer = GpuBuffer::<f32>::new(context.device(), BufferType::Uniform, 10);
        assert!(uniform_buffer.can_download());

        let storage_buffer = GpuBuffer::<f32>::new(context.device(), BufferType::Storage, 10);
        assert!(storage_buffer.can_download());
    }

    #[tokio::test]
    async fn test_buffer_round_trip() {
        let context = create_test_context().await;
        let mut buffer = GpuBuffer::new(context.device(), BufferType::Storage, 100);

        // Test with different data types
        let float_data = vec![1.5f32, 2.5, 3.5, 4.5];
        buffer
            .upload(context.device(), context.queue(), &float_data)
            .unwrap();

        let downloaded = buffer
            .download(context.device(), context.queue())
            .await
            .unwrap();

        assert_eq!(downloaded, float_data);
    }

    #[tokio::test]
    async fn test_buffer_download_large() {
        let context = create_test_context().await;
        let mut buffer = GpuBuffer::new(context.device(), BufferType::Storage, 10000);

        // Upload large dataset
        let data: Vec<f32> = (0..5000).map(|i| i as f32 * 0.5).collect();
        buffer
            .upload(context.device(), context.queue(), &data)
            .unwrap();

        // Download and verify
        let downloaded = buffer
            .download(context.device(), context.queue())
            .await
            .unwrap();

        assert_eq!(downloaded.len(), data.len());
        assert_eq!(downloaded, data);
    }

    #[tokio::test]
    async fn test_buffer_download_after_resize() {
        let context = create_test_context().await;
        let mut buffer = GpuBuffer::new(context.device(), BufferType::Vertex, 10);

        // Upload data that triggers resize
        let data: Vec<f32> = (0..100).map(|i| i as f32).collect();
        buffer
            .upload(context.device(), context.queue(), &data)
            .unwrap();

        // Download should still work after resize
        let downloaded = buffer
            .download(context.device(), context.queue())
            .await
            .unwrap();

        assert_eq!(downloaded, data);
    }

    #[tokio::test]
    async fn test_buffer_download_multiple_uploads() {
        let context = create_test_context().await;
        let mut buffer = GpuBuffer::new(context.device(), BufferType::Storage, 100);

        // Multiple uploads
        let data1 = vec![1.0f32, 2.0, 3.0];
        buffer
            .upload(context.device(), context.queue(), &data1)
            .unwrap();

        let data2 = vec![4.0f32, 5.0, 6.0, 7.0, 8.0];
        buffer
            .upload(context.device(), context.queue(), &data2)
            .unwrap();

        // Download should get the latest data
        let downloaded = buffer
            .download(context.device(), context.queue())
            .await
            .unwrap();

        assert_eq!(downloaded, data2);
    }

    #[tokio::test]
    async fn test_download_performance_10k() {
        use std::time::Instant;

        let context = create_test_context().await;
        let mut buffer = GpuBuffer::new(context.device(), BufferType::Storage, 10000);

        // Upload 10K elements
        let data: Vec<f32> = (0..10000).map(|i| i as f32).collect();
        buffer
            .upload(context.device(), context.queue(), &data)
            .unwrap();

        // Measure download time
        let start = Instant::now();
        let downloaded = buffer
            .download(context.device(), context.queue())
            .await
            .unwrap();
        let elapsed = start.elapsed();

        assert_eq!(downloaded.len(), 10000);
        println!("Downloaded 10K elements in {:?}", elapsed);

        // Performance: GPU buffer download should be fast.
        // Debug builds are slower; use generous thresholds.
        #[cfg(debug_assertions)]
        let threshold_ms: u128 = 200;
        #[cfg(not(debug_assertions))]
        let threshold_ms: u128 = 50;

        assert!(
            elapsed.as_millis() < threshold_ms,
            "Download took too long: {:?} (threshold: {}ms)",
            elapsed,
            threshold_ms
        );
    }

    // === Adaptive Buffer Pool Tests ===

    #[tokio::test]
    async fn test_adaptive_usage_tracking() {
        let context = create_test_context().await;
        let device = Arc::new(context.device().clone());
        let mut pool = BufferPool::new(device);

        // Allocate and deallocate several buffers
        let mut buffers = Vec::new();
        for _ in 0..5 {
            buffers.push(pool.allocate::<f32>(BufferType::Vertex, 100));
        }

        for buffer in buffers {
            pool.deallocate(buffer);
        }

        // Check that popular sizes are tracked
        let popular = pool.popular_sizes(5);
        assert!(!popular.is_empty());

        // The size class for 100 elements should be popular
        let size_class = pool.calculate_size_class(100);
        let vertex_hit = popular
            .iter()
            .any(|((t, s), _)| *t == BufferType::Vertex && *s == size_class);
        assert!(vertex_hit);
    }

    #[tokio::test]
    async fn test_pressure_level_calculation() {
        let context = create_test_context().await;
        let device = Arc::new(context.device().clone());

        // Use a reasonable memory limit
        let config = BufferPoolConfig {
            max_total_memory: Some(5000),  // 5KB limit
            enable_adaptive_sizing: false, // Disable to test pressure calculation directly
            ..Default::default()
        };
        let mut pool = BufferPool::with_config(device, config);

        // Initially should be normal pressure
        assert_eq!(pool.current_pressure_level(), PressureLevel::Normal);

        // Allocate and deallocate buffers to fill the pool
        // Each f32 buffer with size class 128 will be 128 * 4 = 512 bytes
        let mut buffers = Vec::new();
        for _ in 0..15 {
            // This should create ~7.5KB of pooled buffers
            buffers.push(pool.allocate::<f32>(BufferType::Vertex, 100));
        }

        // Deallocate to fill the pool
        for buffer in buffers {
            pool.deallocate(buffer);
        }

        // Check memory usage
        let usage = pool.memory_usage_percentage();
        assert!(usage.is_some());
        let usage_pct = usage.unwrap();

        // Should now have memory pressure (>80% of 5KB)
        let pressure = pool.current_pressure_level();
        assert!(
            pressure != PressureLevel::Normal,
            "Expected memory pressure at {}% usage but got Normal",
            usage_pct
        );
    }

    #[tokio::test]
    async fn test_intelligent_cleanup_gentle() {
        let context = create_test_context().await;
        let device = Arc::new(context.device().clone());

        let config = BufferPoolConfig {
            max_total_memory: Some(10_000),
            enable_adaptive_sizing: true,
            ..Default::default()
        };
        let mut pool = BufferPool::with_config(device, config);

        // Allocate and deallocate buffers to build pool
        let mut buffers = Vec::new();
        for _ in 0..10 {
            buffers.push(pool.allocate::<f32>(BufferType::Vertex, 100));
        }

        for buffer in buffers {
            pool.deallocate(buffer);
        }

        let _initial_pooled = pool.get_stats().pooled_buffers;

        // Trigger gentle cleanup (warning level)
        pool.gentle_cleanup();

        // Since buffers were just used, gentle cleanup shouldn't remove much
        let after_gentle = pool.get_stats().pooled_buffers;
        assert!(
            after_gentle > 0,
            "Gentle cleanup should keep recent buffers"
        );
    }

    #[tokio::test]
    async fn test_recent_hit_rate() {
        let context = create_test_context().await;
        let device = Arc::new(context.device().clone());
        let mut pool = BufferPool::new(device);

        // First allocations will all miss
        let mut buffers = Vec::new();
        for _ in 0..5 {
            buffers.push(pool.allocate::<f32>(BufferType::Vertex, 100));
        }

        // Deallocate them
        for buffer in buffers {
            pool.deallocate(buffer);
        }

        // Now allocate again - should hit
        let mut buffers2 = Vec::new();
        for _ in 0..5 {
            buffers2.push(pool.allocate::<f32>(BufferType::Vertex, 100));
        }

        // Recent hit rate should be high for the last 5 allocations
        let hit_rate = pool.recent_hit_rate(5);
        assert!(
            hit_rate > 80.0,
            "Expected high hit rate but got {}%",
            hit_rate
        );
    }

    #[tokio::test]
    async fn test_retention_score() {
        let context = create_test_context().await;
        let device = Arc::new(context.device().clone());
        let mut pool = BufferPool::new(device);

        // Allocate the same size multiple times
        for _ in 0..10 {
            let buffer = pool.allocate::<f32>(BufferType::Vertex, 100);
            pool.deallocate(buffer);
        }

        // Check retention score
        let size_class = pool.calculate_size_class(100);
        let score = pool.retention_score(BufferType::Vertex, size_class);

        // Frequently used sizes should have higher scores
        assert!(score > 0.0, "Retention score should be positive");
    }

    #[tokio::test]
    async fn test_adaptive_sizing_can_be_disabled() {
        let context = create_test_context().await;
        let device = Arc::new(context.device().clone());

        let config = BufferPoolConfig {
            enable_adaptive_sizing: false,
            max_total_memory: Some(1000),
            ..Default::default()
        };
        let mut pool = BufferPool::with_config(device, config);

        // Allocate buffers to trigger memory pressure
        let mut buffers = Vec::new();
        for _ in 0..20 {
            buffers.push(pool.allocate::<f32>(BufferType::Vertex, 100));
        }

        for buffer in buffers {
            pool.deallocate(buffer);
        }

        // With adaptive sizing disabled, it should still work but use simple LRU
        assert!(pool.get_stats().pooled_buffers > 0);
    }

    #[tokio::test]
    async fn test_allocate_raw_and_deallocate_raw() {
        let context = create_test_context().await;
        let device = Arc::new(context.device().clone());
        let mut pool = BufferPool::new(device);

        // First allocation is a miss.
        let (buf1, sc1) = pool.allocate_raw(BufferType::Storage, 256);
        assert!(sc1 >= 256); // power-of-2 rounding
        assert_eq!(pool.get_stats().pool_misses, 1);
        assert_eq!(pool.get_stats().pool_hits, 0);
        assert_eq!(pool.get_stats().active_buffers, 1);

        // Write some data to prove the buffer works.
        context.queue().write_buffer(&buf1, 0, &[42u8; 128]);

        // Return to pool.
        pool.deallocate_raw(buf1, BufferType::Storage, sc1);
        assert_eq!(pool.get_stats().active_buffers, 0);
        assert_eq!(pool.get_stats().pooled_buffers, 1);

        // Second allocation should hit the pool.
        let (_buf2, sc2) = pool.allocate_raw(BufferType::Storage, 256);
        assert_eq!(sc2, sc1); // same size class
        assert_eq!(pool.get_stats().pool_hits, 1);
        assert_eq!(pool.get_stats().pooled_buffers, 0);
    }
}

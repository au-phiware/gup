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
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Arc;
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
}

impl BufferType {
    /// Get the appropriate wgpu buffer usage flags for this buffer type.
    pub fn usage_flags(self) -> BufferUsages {
        match self {
            BufferType::Vertex => {
                BufferUsages::VERTEX | BufferUsages::COPY_DST | BufferUsages::COPY_SRC
            }
            BufferType::Instance => {
                BufferUsages::VERTEX | BufferUsages::COPY_DST | BufferUsages::COPY_SRC
            }
            BufferType::Uniform => {
                BufferUsages::UNIFORM | BufferUsages::COPY_DST | BufferUsages::COPY_SRC
            }
            BufferType::Storage => {
                BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC
            }
        }
    }

    /// Get the minimum alignment requirement for this buffer type.
    pub fn alignment(self) -> u64 {
        match self {
            BufferType::Vertex | BufferType::Instance => 4, // 4-byte alignment
            BufferType::Uniform => 256,                     // Uniform buffer alignment
            BufferType::Storage => 4,                       // Storage buffer alignment
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
            return Err(GupError::BufferError(format!(
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

    /// Download data from the GPU buffer (primarily for debugging/validation).
    ///
    /// Note: This is a simplified implementation. A full implementation would
    /// use proper async buffer mapping with wgpu's callback system.
    pub async fn download(&self, _device: &Device, _queue: &Queue) -> GupResult<Vec<T>> {
        // For now, return an empty vector as this functionality is not critical
        // for the core buffer management story. A full implementation would:
        // 1. Create a staging buffer
        // 2. Copy GPU buffer to staging buffer
        // 3. Map the staging buffer for reading
        // 4. Read the data and return it

        Err(GupError::BufferError(
            "Buffer download not yet implemented - use for upload/rendering only".to_string(),
        ))
    }

    /// Get the raw wgpu buffer for shader binding.
    pub fn raw_buffer(&self) -> &Buffer {
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

            queue.submit(Some(encoder.finish()));
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
}

/// Buffer pool for efficient resource reuse and memory management.
pub struct BufferPool {
    pools: HashMap<(BufferType, usize), Vec<Buffer>>,
    device: Arc<Device>,
    allocation_stats: AllocationStats,
}

impl BufferPool {
    /// Create a new buffer pool with the given device.
    pub fn new(device: Arc<Device>) -> Self {
        Self {
            pools: HashMap::new(),
            device,
            allocation_stats: AllocationStats::default(),
        }
    }

    /// Allocate a buffer from the pool or create a new one.
    pub fn allocate<T>(&mut self, buffer_type: BufferType, capacity: usize) -> GpuBuffer<T>
    where
        T: bytemuck::Pod + bytemuck::Zeroable,
    {
        let size_class = self.calculate_size_class(capacity);
        let key = (buffer_type, size_class);

        let buffer = if let Some(pool) = self.pools.get_mut(&key) {
            if let Some(buffer) = pool.pop() {
                self.allocation_stats.pooled_buffers -= 1;
                buffer
            } else {
                self.create_new_buffer(buffer_type, size_class)
            }
        } else {
            self.create_new_buffer(buffer_type, size_class)
        };

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

    /// Return a buffer to the pool for reuse.
    pub fn deallocate<T>(&mut self, mut buffer: GpuBuffer<T>) {
        let key = (buffer.buffer_type, buffer.capacity);

        // Reset buffer state
        buffer.len = 0;

        // Add to appropriate pool
        self.pools.entry(key).or_default().push(buffer.buffer);

        self.allocation_stats.total_deallocated += 1;
        self.allocation_stats.active_buffers -= 1;
        self.allocation_stats.pooled_buffers += 1;
    }

    /// Clean up unused buffers to free memory.
    pub fn cleanup_unused(&mut self) {
        const MAX_POOL_SIZE: usize = 10; // Maximum buffers per pool

        for pool in self.pools.values_mut() {
            let excess = pool.len().saturating_sub(MAX_POOL_SIZE);
            if excess > 0 {
                pool.drain(0..excess);
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
}

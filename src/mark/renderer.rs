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

//! High-level mark renderer for efficient batch rendering.
//!
//! The MarkRenderer provides a streamlined interface for rendering marks efficiently,
//! handling vertex and instance buffer management, and optimizing GPU state changes.

use crate::buffer::{BufferType, GpuBuffer};
use crate::error::GupResult;
use crate::mark::Mark;
use wgpu::{Device, Queue, RenderPass};

/// High-level renderer for mark-based visualizations.
///
/// The MarkRenderer manages GPU buffers and provides efficient batch rendering
/// for mark instances. It handles both indexed and non-indexed rendering modes
/// and optimizes buffer usage for performance.
///
/// # Examples
///
/// ```rust
/// use gup::mark::{Circle, Mark, MarkRenderer};
/// use gup::GupContext;
/// use std::sync::Arc;
///
/// async fn example() -> Result<(), Box<dyn std::error::Error>> {
///     let context = Arc::new(GupContext::headless().await?);
///     let device = &context.device;
///     
///     let mut renderer = MarkRenderer::new(device);
///     
///     // Upload vertex data
///     let vertices = Circle::generate_vertices();
///     renderer.upload_vertices(device, &context.queue, &vertices)?;
///     
///     // Render in a render pass
///     // let mut render_pass = ...;
///     // renderer.render_marks::<Circle>(&mut render_pass, &pipeline, &bind_group, 100)?;
///     
///     Ok(())
/// }
/// ```
pub struct MarkRenderer {
    vertex_buffer: GpuBuffer<u8>,
    instance_buffer: GpuBuffer<u8>,
    index_buffer: Option<GpuBuffer<u32>>,
}

impl MarkRenderer {
    /// Create a new mark renderer with default buffer capacities.
    pub fn new(device: &Device) -> Self {
        Self {
            vertex_buffer: GpuBuffer::new(device, BufferType::Vertex, 4096), // 4KB initial capacity
            instance_buffer: GpuBuffer::new(device, BufferType::Instance, 8192), // 8KB initial capacity
            index_buffer: Some(GpuBuffer::new(device, BufferType::Storage, 2048)), // 2KB for indices
        }
    }

    /// Create a new mark renderer with custom buffer capacities.
    pub fn with_capacity(
        device: &Device,
        vertex_capacity: usize,
        instance_capacity: usize,
        index_capacity: Option<usize>,
    ) -> Self {
        Self {
            vertex_buffer: GpuBuffer::new(device, BufferType::Vertex, vertex_capacity),
            instance_buffer: GpuBuffer::new(device, BufferType::Instance, instance_capacity),
            index_buffer: index_capacity
                .map(|cap| GpuBuffer::new(device, BufferType::Storage, cap)),
        }
    }

    /// Upload vertex data to the GPU.
    ///
    /// This method uploads the base geometry for a mark type to the vertex buffer.
    /// The vertex data should come from `Mark::generate_vertices()`.
    pub fn upload_vertices<T>(
        &mut self,
        device: &Device,
        queue: &Queue,
        vertices: &[T],
    ) -> GupResult<()>
    where
        T: bytemuck::Pod + bytemuck::Zeroable,
    {
        let vertex_data = bytemuck::cast_slice(vertices);
        self.vertex_buffer.upload(device, queue, vertex_data)
    }

    /// Upload instance data to the GPU.
    ///
    /// This method uploads per-instance data (like circle centers, radii, colors)
    /// to the instance buffer for instanced rendering.
    pub fn upload_instances<T>(
        &mut self,
        device: &Device,
        queue: &Queue,
        instances: &[T],
    ) -> GupResult<()>
    where
        T: bytemuck::Pod + bytemuck::Zeroable,
    {
        let instance_data = bytemuck::cast_slice(instances);
        self.instance_buffer.upload(device, queue, instance_data)
    }

    /// Upload index data to the GPU for indexed rendering.
    ///
    /// This method uploads index data for marks that use indexed rendering.
    /// The indices should come from `Mark::generate_indices()`.
    pub fn upload_indices(
        &mut self,
        device: &Device,
        queue: &Queue,
        indices: &[u32],
    ) -> GupResult<()> {
        if let Some(ref mut index_buffer) = self.index_buffer {
            index_buffer.upload(device, queue, indices)
        } else {
            Err(crate::error::GupError::render_error(
                "Index buffer not available".to_string(),
            ))
        }
    }

    /// Render mark instances using the current buffers.
    ///
    /// This method performs the actual GPU rendering using the uploaded vertex,
    /// instance, and index data. It automatically chooses between indexed and
    /// non-indexed rendering based on the mark type.
    pub fn render_marks<M: Mark>(
        &self,
        render_pass: &mut RenderPass,
        pipeline: &wgpu::RenderPipeline,
        bind_group: &wgpu::BindGroup,
        instance_count: u32,
    ) -> GupResult<()> {
        // Set pipeline and bind groups
        render_pass.set_pipeline(pipeline);
        render_pass.set_bind_group(0, bind_group, &[]);

        // Set vertex buffer
        render_pass.set_vertex_buffer(0, self.vertex_buffer.buffer().slice(..));

        // Render based on mark characteristics
        if let Some(index_count) = M::index_count() {
            // Indexed rendering
            if let Some(ref index_buffer) = self.index_buffer {
                render_pass
                    .set_index_buffer(index_buffer.buffer().slice(..), wgpu::IndexFormat::Uint32);
                render_pass.draw_indexed(0..index_count as u32, 0, 0..instance_count);
            } else {
                return Err(crate::error::GupError::render_error(
                    "Mark requires indexed rendering but no index buffer available".to_string(),
                ));
            }
        } else {
            // Non-indexed rendering
            render_pass.draw(0..M::vertex_count() as u32, 0..instance_count);
        }

        Ok(())
    }

    /// Get the current vertex buffer capacity in bytes.
    pub fn vertex_capacity(&self) -> usize {
        self.vertex_buffer.capacity()
    }

    /// Get the current instance buffer capacity in bytes.
    pub fn instance_capacity(&self) -> usize {
        self.instance_buffer.capacity()
    }

    /// Get the current index buffer capacity in indices (if available).
    pub fn index_capacity(&self) -> Option<usize> {
        self.index_buffer.as_ref().map(|buf| buf.capacity())
    }

    /// Get the current vertex buffer length in bytes.
    pub fn vertex_len(&self) -> usize {
        self.vertex_buffer.len()
    }

    /// Get the current instance buffer length in bytes.
    pub fn instance_len(&self) -> usize {
        self.instance_buffer.len()
    }

    /// Get the current index buffer length in indices (if available).
    pub fn index_len(&self) -> Option<usize> {
        self.index_buffer.as_ref().map(|buf| buf.len())
    }

    /// Clear all buffer data without deallocating.
    ///
    /// This method resets the buffer lengths to zero, effectively clearing
    /// the data without deallocating the GPU memory.
    pub fn clear(&mut self) {
        self.vertex_buffer.clear();
        self.instance_buffer.clear();
        if let Some(ref mut index_buffer) = self.index_buffer {
            index_buffer.clear();
        }
    }

    /// Access to the underlying vertex buffer for advanced operations.
    pub fn vertex_buffer(&self) -> &GpuBuffer<u8> {
        &self.vertex_buffer
    }

    /// Access to the underlying instance buffer for advanced operations.
    pub fn instance_buffer(&self) -> &GpuBuffer<u8> {
        &self.instance_buffer
    }

    /// Access to the underlying index buffer for advanced operations.
    pub fn index_buffer(&self) -> Option<&GpuBuffer<u32>> {
        self.index_buffer.as_ref()
    }
}

impl Default for MarkRenderer {
    fn default() -> Self {
        // Cannot implement Default properly without Device
        // This implementation will panic - users should use new() instead
        panic!("MarkRenderer::default() cannot be used - use MarkRenderer::new(device) instead")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper function to create test context - would be implemented in integration tests
    async fn create_test_context() -> GupResult<std::sync::Arc<crate::context::GupContext>> {
        crate::context::GupContext::headless().await
    }

    #[tokio::test]
    async fn test_mark_renderer_creation() -> GupResult<()> {
        let context = create_test_context().await?;
        let device = &context.device;

        let renderer = MarkRenderer::new(device);

        // Verify initial capacities
        assert!(renderer.vertex_capacity() > 0);
        assert!(renderer.instance_capacity() > 0);
        assert!(renderer.index_capacity().is_some());

        // Verify initial lengths are zero
        assert_eq!(renderer.vertex_len(), 0);
        assert_eq!(renderer.instance_len(), 0);
        assert_eq!(renderer.index_len(), Some(0));

        Ok(())
    }

    #[tokio::test]
    async fn test_mark_renderer_with_capacity() -> GupResult<()> {
        let context = create_test_context().await?;
        let device = &context.device;

        let vertex_cap = 1024;
        let instance_cap = 2048;
        let index_cap = Some(512);

        let renderer = MarkRenderer::with_capacity(device, vertex_cap, instance_cap, index_cap);

        assert_eq!(renderer.vertex_capacity(), vertex_cap);
        assert_eq!(renderer.instance_capacity(), instance_cap);
        assert_eq!(renderer.index_capacity(), index_cap);

        Ok(())
    }

    #[tokio::test]
    async fn test_upload_vertices() -> GupResult<()> {
        let context = create_test_context().await?;
        let device = &context.device;
        let queue = &context.queue;

        let mut renderer = MarkRenderer::new(device);

        // Test vertex data
        #[repr(C)]
        #[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
        struct TestVertex {
            position: [f32; 2],
        }

        let vertices = vec![
            TestVertex {
                position: [0.0, 0.0],
            },
            TestVertex {
                position: [1.0, 0.0],
            },
            TestVertex {
                position: [0.0, 1.0],
            },
        ];

        renderer.upload_vertices(device, queue, &vertices)?;

        assert_eq!(
            renderer.vertex_len(),
            vertices.len() * std::mem::size_of::<TestVertex>()
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_upload_instances() -> GupResult<()> {
        let context = create_test_context().await?;
        let device = &context.device;
        let queue = &context.queue;

        let mut renderer = MarkRenderer::new(device);

        // Test instance data
        #[repr(C)]
        #[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
        struct TestInstance {
            center: [f32; 2],
            radius: f32,
            _padding: f32, // Alignment padding
        }

        let instances = vec![
            TestInstance {
                center: [10.0, 20.0],
                radius: 5.0,
                _padding: 0.0,
            },
            TestInstance {
                center: [30.0, 40.0],
                radius: 8.0,
                _padding: 0.0,
            },
        ];

        renderer.upload_instances(device, queue, &instances)?;

        assert_eq!(
            renderer.instance_len(),
            instances.len() * std::mem::size_of::<TestInstance>()
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_upload_indices() -> GupResult<()> {
        let context = create_test_context().await?;
        let device = &context.device;
        let queue = &context.queue;

        let mut renderer = MarkRenderer::new(device);

        let indices = vec![0, 1, 2, 0, 2, 3];

        renderer.upload_indices(device, queue, &indices)?;

        assert_eq!(renderer.index_len(), Some(indices.len()));

        Ok(())
    }

    #[tokio::test]
    async fn test_clear_buffers() -> GupResult<()> {
        let context = create_test_context().await?;
        let device = &context.device;
        let queue = &context.queue;

        let mut renderer = MarkRenderer::new(device);

        // Upload some test data
        let vertices = vec![[0.0f32, 1.0f32]; 10];
        let instances = vec![[2.0f32, 3.0f32]; 5];
        let indices = vec![0u32, 1, 2];

        renderer.upload_vertices(device, queue, &vertices)?;
        renderer.upload_instances(device, queue, &instances)?;
        renderer.upload_indices(device, queue, &indices)?;

        // Verify data was uploaded
        assert!(renderer.vertex_len() > 0);
        assert!(renderer.instance_len() > 0);
        assert_eq!(renderer.index_len(), Some(3));

        // Clear all buffers
        renderer.clear();

        // Verify all lengths are zero
        assert_eq!(renderer.vertex_len(), 0);
        assert_eq!(renderer.instance_len(), 0);
        assert_eq!(renderer.index_len(), Some(0));

        Ok(())
    }
}

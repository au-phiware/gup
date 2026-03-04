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
    /// Running performance counters.
    metrics: super::performance_opt::MarkPerformanceMetrics,
    /// Default identity viewport transform bind group (lazily created).
    /// Set at @group(1) to satisfy the pipeline layout when no zoom is active.
    vt_bind_group: Option<wgpu::BindGroup>,
    /// Buffer backing the default viewport transform bind group.
    _vt_buffer: Option<wgpu::Buffer>,
}

impl MarkRenderer {
    /// Create a new mark renderer with default buffer capacities.
    pub fn new(device: &Device) -> Self {
        let (vt_buffer, vt_bind_group) = Self::create_default_vt_bind_group(device);
        Self {
            vertex_buffer: GpuBuffer::new(device, BufferType::Vertex, 4096), // 4KB initial capacity
            instance_buffer: GpuBuffer::new(device, BufferType::Instance, 8192), // 8KB initial capacity
            index_buffer: Some(GpuBuffer::new(device, BufferType::Index, 2048)), // 2KB for indices
            metrics: Default::default(),
            vt_bind_group: Some(vt_bind_group),
            _vt_buffer: Some(vt_buffer),
        }
    }

    /// Create a new mark renderer with custom buffer capacities.
    pub fn with_capacity(
        device: &Device,
        vertex_capacity: usize,
        instance_capacity: usize,
        index_capacity: Option<usize>,
    ) -> Self {
        let (vt_buffer, vt_bind_group) = Self::create_default_vt_bind_group(device);
        Self {
            vertex_buffer: GpuBuffer::new(device, BufferType::Vertex, vertex_capacity),
            instance_buffer: GpuBuffer::new(device, BufferType::Instance, instance_capacity),
            index_buffer: index_capacity
                .map(|cap| GpuBuffer::new(device, BufferType::Storage, cap)),
            metrics: Default::default(),
            vt_bind_group: Some(vt_bind_group),
            _vt_buffer: Some(vt_buffer),
        }
    }

    /// Create a default identity viewport transform bind group.
    fn create_default_vt_bind_group(device: &Device) -> (wgpu::Buffer, wgpu::BindGroup) {
        use wgpu::util::DeviceExt;
        let identity = crate::zoom::GpuViewportTransform::IDENTITY;
        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mark_renderer_default_vt_uniform"),
            contents: bytemuck::bytes_of(&identity),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("mark_renderer_vt_bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mark_renderer_default_vt_bind_group"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });
        (buffer, bind_group)
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

        // Set default viewport transform at group 1 (identity).
        if let Some(ref vt_bg) = self.vt_bind_group {
            render_pass.set_bind_group(1, vt_bg, &[]);
        }

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

    /// Render mark instances with pattern support for accessibility.
    ///
    /// This method extends the basic rendering to include pattern bind groups
    /// for accessibility features. The pattern bind group is set at group 2
    /// (group 1 is reserved for the viewport transform).
    pub fn render_marks_with_patterns<M: Mark>(
        &self,
        render_pass: &mut RenderPass,
        pipeline: &wgpu::RenderPipeline,
        bind_group: &wgpu::BindGroup,
        pattern_bind_group: &wgpu::BindGroup,
        instance_count: u32,
    ) -> GupResult<()> {
        // Set pipeline and bind groups
        render_pass.set_pipeline(pipeline);
        render_pass.set_bind_group(0, bind_group, &[]);
        // Viewport transform at group 1 (identity).
        if let Some(ref vt_bg) = self.vt_bind_group {
            render_pass.set_bind_group(1, vt_bg, &[]);
        }
        render_pass.set_bind_group(2, pattern_bind_group, &[]);

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

    /// Render mark instances using multiple pipelines (multi-pass rendering).
    ///
    /// Each pipeline in `pipelines` corresponds to a pass in the multi-pass
    /// configuration. All passes are issued within the same render pass,
    /// following the single render pass per frame pattern.
    ///
    /// # Arguments
    ///
    /// * `render_pass` - The active GPU render pass
    /// * `config` - Multi-pass configuration describing each pass
    /// * `pipelines` - One pipeline per pass (must match `config.pass_count()`)
    /// * `bind_group` - Shared bind group for all passes
    /// * `instance_count` - Number of instances to draw per pass
    pub fn render_marks_multi_pass<M: Mark>(
        &self,
        render_pass: &mut RenderPass<'_>,
        config: &super::advanced_rendering::MultiPassConfig,
        pipelines: &[wgpu::RenderPipeline],
        bind_group: &wgpu::BindGroup,
        instance_count: u32,
    ) -> GupResult<()> {
        if pipelines.len() != config.pass_count() {
            return Err(crate::error::GupError::render_error(format!(
                "Pipeline count ({}) doesn't match pass count ({})",
                pipelines.len(),
                config.pass_count()
            )));
        }

        for (i, (pass_config, pipeline)) in config.passes().iter().zip(pipelines.iter()).enumerate()
        {
            render_pass.set_pipeline(pipeline);
            render_pass.set_bind_group(0, bind_group, &[]);
            // Viewport transform at group 1 (identity).
            if let Some(ref vt_bg) = self.vt_bind_group {
                render_pass.set_bind_group(1, vt_bg, &[]);
            }
            render_pass.set_vertex_buffer(0, self.vertex_buffer.buffer().slice(..));

            if let Some(stencil_ref) = pass_config.stencil_reference {
                render_pass.set_stencil_reference(stencil_ref);
            }

            if let Some(index_count) = M::index_count() {
                if let Some(ref index_buffer) = self.index_buffer {
                    render_pass.set_index_buffer(
                        index_buffer.buffer().slice(..),
                        wgpu::IndexFormat::Uint32,
                    );
                    render_pass.draw_indexed(0..index_count as u32, 0, 0..instance_count);
                } else {
                    return Err(crate::error::GupError::render_error(format!(
                        "Mark requires indexed rendering but no index buffer (pass {i} '{}')",
                        pass_config.label,
                    )));
                }
            } else {
                render_pass.draw(0..M::vertex_count() as u32, 0..instance_count);
            }
        }

        Ok(())
    }

    /// Render marks with state isolation using a [`RenderStateManager`](super::advanced_rendering::RenderStateManager).
    ///
    /// This method saves the current render state, applies the mark-specific
    /// viewport/scissor configuration, renders, then restores the previous state.
    /// This prevents mark types from interfering with each other in compositions.
    pub fn render_marks_with_state<M: Mark>(
        &self,
        render_pass: &mut RenderPass<'_>,
        pipeline: &wgpu::RenderPipeline,
        bind_group: &wgpu::BindGroup,
        instance_count: u32,
        state_manager: &super::advanced_rendering::RenderStateManager,
    ) -> GupResult<()> {
        // Apply state (viewport/scissor) before rendering
        state_manager.apply_to_render_pass(render_pass);

        // Delegate to standard render
        self.render_marks::<M>(render_pass, pipeline, bind_group, instance_count)
    }

    /// Render marks with dynamic attribute buffers bound.
    ///
    /// This method extends [`render_marks`](Self::render_marks) by also binding the dynamic attribute
    /// bind group at the specified group index. The dynamic attribute buffers are
    /// managed by a [`DynamicAttributeBufferManager`](super::advanced_rendering::DynamicAttributeBufferManager).
    ///
    /// # Arguments
    ///
    /// * `render_pass` - The active GPU render pass
    /// * `pipeline` - The render pipeline
    /// * `bind_group` - The primary bind group (group 0)
    /// * `dynamic_attr_bind_group` - The dynamic attribute bind group
    /// * `dynamic_attr_group_index` - The bind group slot for dynamic attributes (typically 1 or 2)
    /// * `instance_count` - Number of instances to draw
    pub fn render_marks_with_dynamic_attrs<M: Mark>(
        &self,
        render_pass: &mut RenderPass,
        pipeline: &wgpu::RenderPipeline,
        bind_group: &wgpu::BindGroup,
        dynamic_attr_bind_group: &wgpu::BindGroup,
        dynamic_attr_group_index: u32,
        instance_count: u32,
    ) -> GupResult<()> {
        // Set pipeline and bind groups
        render_pass.set_pipeline(pipeline);
        render_pass.set_bind_group(0, bind_group, &[]);
        // Viewport transform at group 1 (identity).
        if let Some(ref vt_bg) = self.vt_bind_group {
            render_pass.set_bind_group(1, vt_bg, &[]);
        }
        render_pass.set_bind_group(dynamic_attr_group_index, dynamic_attr_bind_group, &[]);

        // Set vertex buffer
        render_pass.set_vertex_buffer(0, self.vertex_buffer.buffer().slice(..));

        // Render based on mark characteristics
        if let Some(index_count) = M::index_count() {
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
            render_pass.draw(0..M::vertex_count() as u32, 0..instance_count);
        }

        Ok(())
    }

    // ------------------------------------------------------------------
    // Tracked render methods (automatic metrics accumulation)
    // ------------------------------------------------------------------

    /// Render mark instances, automatically tracking draw-call and instance
    /// metrics.
    ///
    /// This is the same as [`render_marks`](Self::render_marks) but takes `&mut self` so it can
    /// increment the internal [`MarkPerformanceMetrics`](super::performance_opt::MarkPerformanceMetrics) counters:
    ///
    /// * `draw_calls += 1`
    /// * `total_instances += instance_count`
    ///
    /// Use [`reset_performance_counters`](Self::reset_performance_counters) at the start of each frame and
    /// [`get_performance_metrics`](Self::get_performance_metrics) at the end to obtain per-frame statistics.
    pub fn render_marks_tracked<M: Mark>(
        &mut self,
        render_pass: &mut RenderPass,
        pipeline: &wgpu::RenderPipeline,
        bind_group: &wgpu::BindGroup,
        instance_count: u32,
    ) -> GupResult<()> {
        self.render_marks::<M>(render_pass, pipeline, bind_group, instance_count)?;
        self.metrics.draw_calls += 1;
        self.metrics.total_instances += instance_count;
        Ok(())
    }

    /// Render mark instances with accessibility patterns, automatically
    /// tracking draw-call and instance metrics.
    ///
    /// Tracked variant of [`render_marks_with_patterns`](Self::render_marks_with_patterns).
    pub fn render_marks_with_patterns_tracked<M: Mark>(
        &mut self,
        render_pass: &mut RenderPass,
        pipeline: &wgpu::RenderPipeline,
        bind_group: &wgpu::BindGroup,
        pattern_bind_group: &wgpu::BindGroup,
        instance_count: u32,
    ) -> GupResult<()> {
        self.render_marks_with_patterns::<M>(
            render_pass,
            pipeline,
            bind_group,
            pattern_bind_group,
            instance_count,
        )?;
        self.metrics.draw_calls += 1;
        self.metrics.total_instances += instance_count;
        Ok(())
    }

    /// Render marks in multiple passes, automatically tracking draw-call,
    /// instance, and pipeline-switch metrics.
    ///
    /// Tracked variant of [`render_marks_multi_pass`](Self::render_marks_multi_pass).  Each configured
    /// pass contributes one draw call and one pipeline switch.
    pub fn render_marks_multi_pass_tracked<M: Mark>(
        &mut self,
        render_pass: &mut RenderPass<'_>,
        config: &super::advanced_rendering::MultiPassConfig,
        pipelines: &[wgpu::RenderPipeline],
        bind_group: &wgpu::BindGroup,
        instance_count: u32,
    ) -> GupResult<()> {
        let pass_count = config.pass_count() as u32;
        self.render_marks_multi_pass::<M>(
            render_pass,
            config,
            pipelines,
            bind_group,
            instance_count,
        )?;
        self.metrics.draw_calls += pass_count;
        self.metrics.total_instances += instance_count * pass_count;
        self.metrics.pipeline_switches += pass_count;
        Ok(())
    }

    /// Render marks with state isolation, automatically tracking metrics.
    ///
    /// Tracked variant of [`render_marks_with_state`](Self::render_marks_with_state).
    pub fn render_marks_with_state_tracked<M: Mark>(
        &mut self,
        render_pass: &mut RenderPass<'_>,
        pipeline: &wgpu::RenderPipeline,
        bind_group: &wgpu::BindGroup,
        instance_count: u32,
        state_manager: &super::advanced_rendering::RenderStateManager,
    ) -> GupResult<()> {
        self.render_marks_with_state::<M>(
            render_pass,
            pipeline,
            bind_group,
            instance_count,
            state_manager,
        )?;
        self.metrics.draw_calls += 1;
        self.metrics.total_instances += instance_count;
        Ok(())
    }

    /// Render marks with dynamic attribute buffers, automatically tracking
    /// metrics.
    ///
    /// Tracked variant of [`render_marks_with_dynamic_attrs`](Self::render_marks_with_dynamic_attrs).
    pub fn render_marks_with_dynamic_attrs_tracked<M: Mark>(
        &mut self,
        render_pass: &mut RenderPass,
        pipeline: &wgpu::RenderPipeline,
        bind_group: &wgpu::BindGroup,
        dynamic_attr_bind_group: &wgpu::BindGroup,
        dynamic_attr_group_index: u32,
        instance_count: u32,
    ) -> GupResult<()> {
        self.render_marks_with_dynamic_attrs::<M>(
            render_pass,
            pipeline,
            bind_group,
            dynamic_attr_bind_group,
            dynamic_attr_group_index,
            instance_count,
        )?;
        self.metrics.draw_calls += 1;
        self.metrics.total_instances += instance_count;
        Ok(())
    }

    // ------------------------------------------------------------------
    // Performance metrics
    // ------------------------------------------------------------------

    /// Get current performance metrics.
    ///
    /// Returns a snapshot of the accumulated performance counters since the
    /// last call to [`reset_performance_counters`](Self::reset_performance_counters).
    pub fn get_performance_metrics(&self) -> &super::performance_opt::MarkPerformanceMetrics {
        &self.metrics
    }

    /// Get a mutable reference to the metrics for external accumulation.
    ///
    /// Call this to update metrics from outside the renderer (e.g. after
    /// buffer uploads or draw calls).
    pub fn metrics_mut(&mut self) -> &mut super::performance_opt::MarkPerformanceMetrics {
        &mut self.metrics
    }

    /// Reset all performance counters to zero.
    ///
    /// Call at the start of each frame to get per-frame metrics, or
    /// leave running for cumulative statistics.
    pub fn reset_performance_counters(&mut self) {
        self.metrics = Default::default();
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
    use wgpu::util::DeviceExt;

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

    #[tokio::test]
    async fn test_metrics_initial_state() -> GupResult<()> {
        let context = create_test_context().await?;
        let renderer = MarkRenderer::new(&context.device);

        let m = renderer.get_performance_metrics();
        assert_eq!(m.draw_calls, 0);
        assert_eq!(m.total_instances, 0);
        assert_eq!(m.pipeline_switches, 0);

        Ok(())
    }

    #[tokio::test]
    async fn test_metrics_manual_accumulation() -> GupResult<()> {
        let context = create_test_context().await?;
        let mut renderer = MarkRenderer::new(&context.device);

        renderer.metrics_mut().draw_calls += 3;
        renderer.metrics_mut().total_instances += 100;
        renderer.metrics_mut().pipeline_switches += 2;

        let m = renderer.get_performance_metrics();
        assert_eq!(m.draw_calls, 3);
        assert_eq!(m.total_instances, 100);
        assert_eq!(m.pipeline_switches, 2);

        Ok(())
    }

    #[tokio::test]
    async fn test_reset_performance_counters() -> GupResult<()> {
        let context = create_test_context().await?;
        let mut renderer = MarkRenderer::new(&context.device);

        renderer.metrics_mut().draw_calls = 5;
        renderer.metrics_mut().total_instances = 500;
        renderer.metrics_mut().pipeline_switches = 3;

        renderer.reset_performance_counters();

        let m = renderer.get_performance_metrics();
        assert_eq!(m.draw_calls, 0);
        assert_eq!(m.total_instances, 0);
        assert_eq!(m.pipeline_switches, 0);

        Ok(())
    }

    /// Helper to set up a complete rendering context (pipeline, bind group,
    /// render target) for Circle marks.
    async fn create_circle_render_context(
        context: &std::sync::Arc<crate::context::GupContext>,
    ) -> GupResult<(
        MarkRenderer,
        std::sync::Arc<wgpu::RenderPipeline>,
        wgpu::BindGroup,
        wgpu::Texture,
    )> {
        use crate::CircleInstance;
        use crate::mark::{Circle, Mark, MarkRegistry};

        let device = &context.device;
        let queue = &context.queue;

        let mut registry = MarkRegistry::new();
        registry.register::<Circle>();

        let mut renderer = MarkRenderer::new(device);

        // Upload Circle geometry
        let vertices = Circle::generate_vertices();
        renderer.upload_vertices(device, queue, &vertices)?;

        let indices = Circle::generate_indices().unwrap();
        renderer.upload_indices(device, queue, &indices)?;

        // Create instance data
        let instances = vec![
            CircleInstance {
                center: [0.0, 0.0],
                radius: 0.1,
                _pad0: 0.0,
                fill_color: [1.0, 0.0, 0.0, 1.0],
                stroke_width: 0.01,
                _pad1: [0.0; 3],
                stroke_color: [0.0, 0.0, 0.0, 1.0],
            };
            5
        ];
        renderer.upload_instances(device, queue, &instances)?;

        // Storage buffer for bind group
        let instance_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("test_instance_storage"),
            contents: bytemuck::cast_slice(&instances),
            usage: wgpu::BufferUsages::STORAGE,
        });

        // Viewport uniform buffer — required by the bind group layout for
        // custom-shader marks (binding 1).
        let viewport = crate::selection::ViewportUniforms {
            width: 64.0,
            height: 64.0,
        };
        let viewport_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("test_viewport_uniform"),
            contents: bytemuck::bytes_of(&viewport),
            usage: wgpu::BufferUsages::UNIFORM,
        });

        let pipeline = registry.get_pipeline::<Circle>(device)?;
        let bind_group =
            registry.create_bind_group::<Circle>(device, &instance_buf, &[&viewport_buf])?;

        // Offscreen render target
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("test_render_target"),
            size: wgpu::Extent3d {
                width: 64,
                height: 64,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Bgra8UnormSrgb,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });

        Ok((renderer, pipeline, bind_group, texture))
    }

    #[tokio::test]
    async fn test_render_marks_tracked_draw_calls() -> GupResult<()> {
        use crate::mark::Circle;

        let context = create_test_context().await?;
        let (mut renderer, pipeline, bind_group, texture) =
            create_circle_render_context(&context).await?;

        let view = texture.create_view(&Default::default());
        let mut encoder = context
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        {
            let mut pass = begin_test_render_pass(&mut encoder, &view);

            // Render three batches
            for _ in 0..3 {
                renderer.render_marks_tracked::<Circle>(&mut pass, &pipeline, &bind_group, 5)?;
            }
        }

        context.queue.submit(Some(encoder.finish()));

        let m = renderer.get_performance_metrics();
        assert_eq!(m.draw_calls, 3, "should record 3 draw calls");
        assert_eq!(m.total_instances, 15, "should record 5×3 = 15 instances");

        Ok(())
    }

    #[tokio::test]
    async fn test_render_marks_tracked_accumulates_across_frames() -> GupResult<()> {
        use crate::mark::Circle;

        let context = create_test_context().await?;
        let (mut renderer, pipeline, bind_group, texture) =
            create_circle_render_context(&context).await?;

        let view = texture.create_view(&Default::default());

        // --- Frame 1 ---
        let mut encoder = context.device.create_command_encoder(&Default::default());
        {
            let mut pass = begin_test_render_pass(&mut encoder, &view);
            renderer.render_marks_tracked::<Circle>(&mut pass, &pipeline, &bind_group, 10)?;
        }
        context.queue.submit(Some(encoder.finish()));

        assert_eq!(renderer.get_performance_metrics().draw_calls, 1);
        assert_eq!(renderer.get_performance_metrics().total_instances, 10);

        // --- Reset for Frame 2 ---
        renderer.reset_performance_counters();

        let mut encoder = context.device.create_command_encoder(&Default::default());
        {
            let mut pass = begin_test_render_pass(&mut encoder, &view);
            renderer.render_marks_tracked::<Circle>(&mut pass, &pipeline, &bind_group, 20)?;
            renderer.render_marks_tracked::<Circle>(&mut pass, &pipeline, &bind_group, 30)?;
        }
        context.queue.submit(Some(encoder.finish()));

        let m = renderer.get_performance_metrics();
        assert_eq!(m.draw_calls, 2, "frame 2 should have exactly 2 draw calls");
        assert_eq!(
            m.total_instances, 50,
            "frame 2 should have 20+30 = 50 instances"
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_non_tracked_render_does_not_update_metrics() -> GupResult<()> {
        use crate::mark::Circle;

        let context = create_test_context().await?;
        let (renderer, pipeline, bind_group, texture) =
            create_circle_render_context(&context).await?;

        let view = texture.create_view(&Default::default());
        let mut encoder = context.device.create_command_encoder(&Default::default());

        {
            let mut pass = begin_test_render_pass(&mut encoder, &view);

            // Use the non-tracked variant
            renderer.render_marks::<Circle>(&mut pass, &pipeline, &bind_group, 100)?;
        }

        context.queue.submit(Some(encoder.finish()));

        let m = renderer.get_performance_metrics();
        assert_eq!(
            m.draw_calls, 0,
            "non-tracked render should not touch counters"
        );
        assert_eq!(m.total_instances, 0);

        Ok(())
    }

    /// Begin a simple render pass for test use.
    fn begin_test_render_pass<'a>(
        encoder: &'a mut wgpu::CommandEncoder,
        view: &'a wgpu::TextureView,
    ) -> RenderPass<'a> {
        encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("test_render_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        })
    }
}

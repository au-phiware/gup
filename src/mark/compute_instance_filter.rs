// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GPU compute shader pipeline for instance culling, LOD classification,
//! and Z-order sorting.
//!
//! For datasets exceeding ~100K instances the CPU-side [`CullingManager`]
//! becomes a bottleneck. This module moves the entire filter / compact /
//! sort pipeline to GPU compute shaders, producing a compact output buffer
//! of visible [`InstanceAttributes`] along with [`wgpu::DrawIndirectArgs`]
//! so the render pass can issue a single `draw_indirect` call with zero
//! CPU readback.
//!
//! # Pipeline
//!
//! 1. **Cull & classify** — per-instance frustum test and LOD level
//!    computation. Writes a `visibility[i] ∈ {0, 1}` flag array.
//! 2. **Prefix sum** — Blelloch-style parallel exclusive scan over the
//!    visibility flags, producing output offsets.
//! 3. **Compact** — scatter visible instances into a dense output buffer
//!    at their prefix-sum offsets.
//!
//! The prefix sum is implemented in three sub-passes to handle arrays
//! larger than a single 256-thread workgroup:
//!   a. Per-workgroup scan (writes block totals).
//!   b. Scan of block totals (single workgroup for up to 256 blocks →
//!      64K instances per level; recursion extends to any size).
//!   c. Add block offsets back to per-element sums.
//!
//! # Fallback
//!
//! When compute shaders are unavailable (e.g. WebGL), the existing
//! [`CullingManager`] CPU path is used transparently.
//!
//! [`CullingManager`]: super::CullingManager

use crate::error::{GupError, GupResult};
use std::sync::Arc;
use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingType, Buffer, BufferBindingType, BufferDescriptor, BufferUsages,
    CommandEncoderDescriptor, ComputePassDescriptor, ComputePipeline, ComputePipelineDescriptor,
    Device, PipelineLayoutDescriptor, PollType, Queue, ShaderModuleDescriptor, ShaderSource,
    ShaderStages,
};

use super::{InstanceAttributes, Viewport2D};

// ---------------------------------------------------------------------------
// GPU-side config uniform (must match WGSL `FilterConfig`)
// ---------------------------------------------------------------------------

/// Configuration uniform uploaded to the GPU for the filter compute shader.
///
/// Layout must exactly match the WGSL `FilterConfig` struct.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct FilterConfig {
    pub min_x: f32,
    pub max_x: f32,
    pub min_y: f32,
    pub max_y: f32,
    pub pixel_width: f32,
    pub pixel_height: f32,
    pub lod_full: f32,
    pub lod_simplified: f32,
    pub lod_point: f32,
    pub instance_count: u32,
    pub vertex_count: u32,
    pub enable_sort: u32,
}

impl FilterConfig {
    /// Create a `FilterConfig` from a viewport and LOD thresholds.
    pub fn from_viewport(
        viewport: &Viewport2D,
        lod_thresholds: &[f32; 3],
        instance_count: u32,
        vertex_count: u32,
    ) -> Self {
        Self {
            min_x: viewport.min_x,
            max_x: viewport.max_x,
            min_y: viewport.min_y,
            max_y: viewport.max_y,
            pixel_width: viewport.pixel_width,
            pixel_height: viewport.pixel_height,
            lod_full: lod_thresholds[0],
            lod_simplified: lod_thresholds[1],
            lod_point: lod_thresholds[2],
            instance_count,
            vertex_count,
            enable_sort: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// Filter result
// ---------------------------------------------------------------------------

/// Result of a GPU-side filter dispatch.
///
/// Contains the output buffer of compacted visible instances and the
/// indirect draw parameter buffer. Both can be passed directly to a
/// render pass without any CPU readback.
pub struct FilterResult {
    /// Dense buffer of visible [`InstanceAttributes`].
    pub output_buffer: Arc<Buffer>,
    /// `DrawIndirect` parameter buffer (vertex_count, instance_count, 0, 0).
    pub draw_indirect_buffer: Arc<Buffer>,
}

// ---------------------------------------------------------------------------
// Compute instance filter
// ---------------------------------------------------------------------------

/// Workgroup size used by all compute entry points in the shader.
const WORKGROUP_SIZE: u32 = 256;

/// Maximum number of instances supported per filter dispatch.
///
/// Limited by the two-level prefix-sum approach: 256 workgroups × 256
/// threads = 65 536 instances per level. Two levels gives 256 × 65 536
/// = 16 M instances. For this implementation we limit to ~16M which is
/// practical for visualization.
pub const MAX_INSTANCES: u32 = WORKGROUP_SIZE * WORKGROUP_SIZE * WORKGROUP_SIZE;

/// GPU compute pipeline for instance filtering, culling, and compaction.
///
/// Replaces the CPU-side [`CullingManager`] for large datasets (>100K
/// instances) by running frustum culling, LOD classification, and stream
/// compaction entirely on the GPU.
///
/// # Example
///
/// ```rust,ignore
/// let filter = ComputeInstanceFilter::new(&device)?;
/// let result = filter.dispatch(
///     &device, &queue,
///     &instance_buffer,    // storage buffer of InstanceAttributes
///     instance_count,
///     vertex_count,
///     &viewport,
///     &lod_thresholds,
/// ).await?;
///
/// // Use result in render pass:
/// render_pass.set_vertex_buffer(1, result.output_buffer.slice(..));
/// render_pass.draw_indirect(&result.draw_indirect_buffer, 0);
/// ```
pub struct ComputeInstanceFilter {
    /// Cull + classify pass.
    cull_pipeline: ComputePipeline,
    /// Per-workgroup prefix sum pass.
    prefix_sum_pipeline: ComputePipeline,
    /// Scan block totals pass.
    prefix_sum_blocks_pipeline: ComputePipeline,
    /// Add block offsets pass.
    prefix_sum_add_offsets_pipeline: ComputePipeline,
    /// Compact visible instances pass.
    compact_pipeline: ComputePipeline,
    /// Bind group layout shared by all passes.
    bind_group_layout: BindGroupLayout,
}

impl ComputeInstanceFilter {
    /// Create a new compute instance filter.
    ///
    /// Compiles the WGSL shader and creates all five compute pipelines.
    pub fn new(device: &Device) -> GupResult<Self> {
        let shader_source = include_str!("../shaders/instance_filter.compute.wgsl");

        let shader_module = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("instance_filter_compute"),
            source: ShaderSource::Wgsl(shader_source.into()),
        });

        // Explicit bind group layout matching the WGSL bindings.
        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("instance_filter_bgl"),
            entries: &[
                // binding 0: instances (read-only storage)
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding 1: output_instances (read-write storage)
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding 2: visibility flags (read-write storage)
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding 3: prefix_sums (read-write storage)
                BindGroupLayoutEntry {
                    binding: 3,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding 4: draw_indirect (read-write storage)
                BindGroupLayoutEntry {
                    binding: 4,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // binding 5: config (uniform)
                BindGroupLayoutEntry {
                    binding: 5,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("instance_filter_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let make_pipeline = |label: &'static str, entry: &'static str| {
            device.create_compute_pipeline(&ComputePipelineDescriptor {
                label: Some(label),
                layout: Some(&pipeline_layout),
                module: &shader_module,
                entry_point: Some(entry),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                cache: None,
            })
        };

        let cull_pipeline = make_pipeline("cull_and_classify_pipeline", "cull_and_classify");
        let prefix_sum_pipeline =
            make_pipeline("prefix_sum_workgroup_pipeline", "prefix_sum_workgroup");
        let prefix_sum_blocks_pipeline =
            make_pipeline("prefix_sum_blocks_pipeline", "prefix_sum_blocks");
        let prefix_sum_add_offsets_pipeline = make_pipeline(
            "prefix_sum_add_offsets_pipeline",
            "prefix_sum_add_block_offsets",
        );
        let compact_pipeline = make_pipeline("compact_instances_pipeline", "compact_instances");

        Ok(Self {
            cull_pipeline,
            prefix_sum_pipeline,
            prefix_sum_blocks_pipeline,
            prefix_sum_add_offsets_pipeline,
            compact_pipeline,
            bind_group_layout,
        })
    }

    /// Run the full filter pipeline on the GPU.
    ///
    /// `input_buffer` must be a storage buffer containing `instance_count`
    /// contiguous [`InstanceAttributes`] structs.
    ///
    /// Returns a [`FilterResult`] with the compacted output buffer and
    /// draw-indirect parameter buffer — no CPU readback required.
    #[allow(clippy::too_many_arguments)]
    pub async fn dispatch(
        &self,
        device: &Device,
        queue: &Queue,
        input_buffer: &Buffer,
        instance_count: u32,
        vertex_count: u32,
        viewport: &Viewport2D,
        lod_thresholds: &[f32; 3],
    ) -> GupResult<FilterResult> {
        if instance_count == 0 {
            return Err(GupError::invalid_operation(
                "Cannot filter zero instances".to_string(),
            ));
        }

        if instance_count > MAX_INSTANCES {
            return Err(GupError::invalid_operation(format!(
                "Instance count {instance_count} exceeds maximum {MAX_INSTANCES}"
            )));
        }

        let instance_size = std::mem::size_of::<InstanceAttributes>() as u64;
        let num_workgroups = instance_count.div_ceil(WORKGROUP_SIZE);

        // --- Allocate transient GPU buffers ---

        let output_buffer = Arc::new(device.create_buffer(&BufferDescriptor {
            label: Some("filter_output_instances"),
            size: instance_count as u64 * instance_size,
            usage: BufferUsages::STORAGE | BufferUsages::VERTEX | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        }));

        let visibility_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("filter_visibility"),
            size: instance_count as u64 * 4,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let prefix_size = (instance_count + num_workgroups) as u64 * 4;
        let prefix_sums_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("filter_prefix_sums"),
            size: prefix_size,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let draw_indirect_buffer = Arc::new(device.create_buffer(&BufferDescriptor {
            label: Some("filter_draw_indirect"),
            size: 16, // 4 × u32
            usage: BufferUsages::STORAGE | BufferUsages::INDIRECT | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        }));

        let config_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("filter_config"),
            size: std::mem::size_of::<FilterConfig>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // --- Encode & submit ---

        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("instance_filter_encoder"),
        });

        self.encode(
            device,
            queue,
            &mut encoder,
            input_buffer,
            &output_buffer,
            &visibility_buffer,
            &prefix_sums_buffer,
            &draw_indirect_buffer,
            &config_buffer,
            instance_count,
            vertex_count,
            viewport,
            lod_thresholds,
        );

        queue.submit([encoder.finish()]);

        Ok(FilterResult {
            output_buffer,
            draw_indirect_buffer,
        })
    }

    /// Encode the filter pipeline into the given command encoder using
    /// pre-existing buffers.
    ///
    /// This is the core implementation shared by both the allocating
    /// [`dispatch`](Self::dispatch) path and
    /// [`PooledComputeInstanceFilter::dispatch`].
    #[allow(clippy::too_many_arguments)]
    fn encode(
        &self,
        device: &Device,
        queue: &Queue,
        encoder: &mut wgpu::CommandEncoder,
        input_buffer: &Buffer,
        output_buffer: &Buffer,
        visibility_buffer: &Buffer,
        prefix_sums_buffer: &Buffer,
        draw_indirect_buffer: &Buffer,
        config_buffer: &Buffer,
        instance_count: u32,
        vertex_count: u32,
        viewport: &Viewport2D,
        lod_thresholds: &[f32; 3],
    ) {
        let bind_group = self.create_bind_group(
            device,
            input_buffer,
            output_buffer,
            visibility_buffer,
            prefix_sums_buffer,
            draw_indirect_buffer,
            config_buffer,
        );

        self.encode_with_bind_group(
            queue,
            encoder,
            &bind_group,
            config_buffer,
            instance_count,
            vertex_count,
            viewport,
            lod_thresholds,
        );
    }

    /// Create a bind group referencing the given buffers.
    #[allow(clippy::too_many_arguments)]
    fn create_bind_group(
        &self,
        device: &Device,
        input_buffer: &Buffer,
        output_buffer: &Buffer,
        visibility_buffer: &Buffer,
        prefix_sums_buffer: &Buffer,
        draw_indirect_buffer: &Buffer,
        config_buffer: &Buffer,
    ) -> BindGroup {
        device.create_bind_group(&BindGroupDescriptor {
            label: Some("instance_filter_bg"),
            layout: &self.bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: input_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: output_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: visibility_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 3,
                    resource: prefix_sums_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 4,
                    resource: draw_indirect_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 5,
                    resource: config_buffer.as_entire_binding(),
                },
            ],
        })
    }

    /// Encode compute passes using a pre-created bind group.
    ///
    /// The config uniform is uploaded via `queue.write_buffer` before the
    /// passes are recorded.
    #[allow(clippy::too_many_arguments)]
    fn encode_with_bind_group(
        &self,
        queue: &Queue,
        encoder: &mut wgpu::CommandEncoder,
        bind_group: &BindGroup,
        config_buffer: &Buffer,
        instance_count: u32,
        vertex_count: u32,
        viewport: &Viewport2D,
        lod_thresholds: &[f32; 3],
    ) {
        let num_workgroups = instance_count.div_ceil(WORKGROUP_SIZE);

        // Upload config.
        let config =
            FilterConfig::from_viewport(viewport, lod_thresholds, instance_count, vertex_count);
        queue.write_buffer(config_buffer, 0, bytemuck::bytes_of(&config));

        // Compute passes.
        {
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("cull_and_classify"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.cull_pipeline);
            pass.set_bind_group(0, bind_group, &[]);
            pass.dispatch_workgroups(num_workgroups, 1, 1);
        }

        {
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("prefix_sum_workgroup"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.prefix_sum_pipeline);
            pass.set_bind_group(0, bind_group, &[]);
            pass.dispatch_workgroups(num_workgroups, 1, 1);
        }

        {
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("prefix_sum_blocks"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.prefix_sum_blocks_pipeline);
            pass.set_bind_group(0, bind_group, &[]);
            pass.dispatch_workgroups(1, 1, 1);
        }

        if num_workgroups > 1 {
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("prefix_sum_add_offsets"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.prefix_sum_add_offsets_pipeline);
            pass.set_bind_group(0, bind_group, &[]);
            pass.dispatch_workgroups(num_workgroups, 1, 1);
        }

        {
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("compact_instances"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.compact_pipeline);
            pass.set_bind_group(0, bind_group, &[]);
            pass.dispatch_workgroups(num_workgroups, 1, 1);
        }
    }

    /// Read back the draw-indirect parameters from the GPU.
    ///
    /// This is primarily useful for testing and diagnostics — in production
    /// the draw-indirect buffer is consumed directly by the render pass.
    pub async fn read_draw_indirect(
        device: &Device,
        queue: &Queue,
        draw_indirect_buffer: &Buffer,
    ) -> GupResult<[u32; 4]> {
        let staging = device.create_buffer(&BufferDescriptor {
            label: Some("draw_indirect_staging"),
            size: 16,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("draw_indirect_readback"),
        });
        encoder.copy_buffer_to_buffer(draw_indirect_buffer, 0, &staging, 0, 16);
        let sub_idx = queue.submit([encoder.finish()]);
        let _ = device.poll(PollType::WaitForSubmissionIndex(sub_idx));

        let slice = staging.slice(..);
        let (sender, receiver) = futures_channel::oneshot::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = sender.send(r);
        });
        let _ = device.poll(PollType::Wait);

        receiver
            .await
            .map_err(|_| GupError::render_error("Draw indirect readback channel closed"))?
            .map_err(|e| GupError::render_error(format!("Draw indirect map failed: {e:?}")))?;

        let data = slice.get_mapped_range();
        let args: &[u32] = bytemuck::cast_slice(&data);
        let result = [args[0], args[1], args[2], args[3]];
        drop(data);
        staging.unmap();

        Ok(result)
    }

    /// Read back the compacted output instances from the GPU.
    ///
    /// Reads up to `max_count` instances. Primarily for testing.
    pub async fn read_output_instances(
        device: &Device,
        queue: &Queue,
        output_buffer: &Buffer,
        max_count: u32,
    ) -> GupResult<Vec<InstanceAttributes>> {
        let instance_size = std::mem::size_of::<InstanceAttributes>() as u64;
        let read_size = max_count as u64 * instance_size;
        let buffer_size = output_buffer.size().min(read_size);

        let staging = device.create_buffer(&BufferDescriptor {
            label: Some("output_instances_staging"),
            size: buffer_size,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("output_instances_readback"),
        });
        encoder.copy_buffer_to_buffer(output_buffer, 0, &staging, 0, buffer_size);
        let sub_idx = queue.submit([encoder.finish()]);
        let _ = device.poll(PollType::WaitForSubmissionIndex(sub_idx));

        let slice = staging.slice(..);
        let (sender, receiver) = futures_channel::oneshot::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = sender.send(r);
        });
        let _ = device.poll(PollType::Wait);

        receiver
            .await
            .map_err(|_| GupError::render_error("Output instances readback channel closed"))?
            .map_err(|e| GupError::render_error(format!("Output instances map failed: {e:?}")))?;

        let data = slice.get_mapped_range();
        let instances: &[InstanceAttributes] = bytemuck::cast_slice(&data);
        let result = instances.to_vec();
        drop(data);
        staging.unmap();

        Ok(result)
    }
}

// ---------------------------------------------------------------------------
// Pooled compute instance filter
// ---------------------------------------------------------------------------

/// Pre-allocated GPU buffer pool for [`ComputeInstanceFilter`].
///
/// Eliminates per-dispatch buffer allocation by pre-allocating the output,
/// visibility, prefix-sum, draw-indirect, and config buffers for a
/// configurable maximum instance count. Buffers are reused across
/// [`dispatch`](Self::dispatch) calls; if the instance count exceeds the
/// current capacity the buffers are automatically grown.
///
/// # Example
///
/// ```rust,ignore
/// let filter = ComputeInstanceFilter::new(&device)?;
/// let mut pooled = PooledComputeInstanceFilter::new(&device, filter, 100_000);
///
/// loop {
///     // Steady-state: zero buffer allocations per frame.
///     let result = pooled.dispatch(
///         &device, &queue,
///         &instance_buffer,
///         instance_count, vertex_count,
///         &viewport, &lod_thresholds,
///     ).await?;
///     // result.output_buffer and result.draw_indirect_buffer are
///     // borrowed from the pool — valid until the next dispatch().
/// }
/// ```
pub struct PooledComputeInstanceFilter {
    /// Underlying pipeline (pipelines + bind group layout).
    inner: ComputeInstanceFilter,
    /// Dense buffer of visible [`InstanceAttributes`].
    output_buffer: Arc<Buffer>,
    /// Per-instance binary visibility flags.
    visibility_buffer: Buffer,
    /// Prefix-sum offsets + block totals.
    prefix_sums_buffer: Buffer,
    /// `DrawIndirect` parameter buffer.
    draw_indirect_buffer: Arc<Buffer>,
    /// Uniform config buffer (always 48 bytes).
    config_buffer: Buffer,
    /// Current maximum instance capacity.
    capacity: u32,
    /// Cached bind group, reused when the input buffer is stable.
    cached_bind_group: Option<CachedBindGroup>,
}

/// A cached bind group along with the input buffer pointer used to
/// create it, so we can detect when the cache is stale.
struct CachedBindGroup {
    /// The bind group itself.
    bind_group: BindGroup,
    /// Identity of the input buffer that was used to create this bind group.
    /// Compared by raw pointer address of the `wgpu::Buffer`.
    input_buffer_id: *const Buffer,
}

// SAFETY: `input_buffer_id` is only used for pointer identity comparison
// (never dereferenced). All other fields (`BindGroup`) are `Send + Sync`.
unsafe impl Send for CachedBindGroup {}
unsafe impl Sync for CachedBindGroup {}

impl PooledComputeInstanceFilter {
    /// Create a new pooled filter with pre-allocated buffers for up to
    /// `max_instances` instances.
    ///
    /// # Panics
    ///
    /// Panics if `max_instances` is zero.
    pub fn new(device: &Device, inner: ComputeInstanceFilter, max_instances: u32) -> Self {
        assert!(max_instances > 0, "max_instances must be > 0");
        let (
            output_buffer,
            visibility_buffer,
            prefix_sums_buffer,
            draw_indirect_buffer,
            config_buffer,
        ) = Self::allocate_buffers(device, max_instances);

        Self {
            inner,
            output_buffer,
            visibility_buffer,
            prefix_sums_buffer,
            draw_indirect_buffer,
            config_buffer,
            capacity: max_instances,
            cached_bind_group: None,
        }
    }

    /// Current buffer capacity in instances.
    pub fn capacity(&self) -> u32 {
        self.capacity
    }

    /// Returns `true` if a bind group is currently cached.
    ///
    /// The cache is populated after the first [`dispatch`](Self::dispatch)
    /// call and remains valid as long as the same input buffer is passed
    /// and no buffer growth occurs.
    pub fn has_cached_bind_group(&self) -> bool {
        self.cached_bind_group.is_some()
    }

    /// Explicitly invalidate the cached bind group.
    ///
    /// The next [`dispatch`](Self::dispatch) call will create a fresh
    /// bind group regardless of input buffer identity.
    pub fn invalidate_bind_group_cache(&mut self) {
        self.cached_bind_group = None;
    }

    /// Access the underlying [`ComputeInstanceFilter`].
    pub fn inner(&self) -> &ComputeInstanceFilter {
        &self.inner
    }

    /// Run the full filter pipeline, reusing pre-allocated buffers.
    ///
    /// If `instance_count` exceeds the current [`capacity`](Self::capacity),
    /// the internal buffers are grown to fit. Otherwise no GPU allocations
    /// are performed.
    ///
    /// When the same `input_buffer` is passed across consecutive dispatches
    /// (and no buffer growth occurs), the wgpu bind group is cached and
    /// reused, eliminating one allocation per frame.
    ///
    /// The returned [`FilterResult`] borrows the pool's buffers via
    /// `Arc` — they remain valid until the next call to `dispatch` (or
    /// [`reserve`](Self::reserve)).
    #[allow(clippy::too_many_arguments)]
    pub async fn dispatch(
        &mut self,
        device: &Device,
        queue: &Queue,
        input_buffer: &Buffer,
        instance_count: u32,
        vertex_count: u32,
        viewport: &Viewport2D,
        lod_thresholds: &[f32; 3],
    ) -> GupResult<FilterResult> {
        if instance_count == 0 {
            return Err(GupError::invalid_operation(
                "Cannot filter zero instances".to_string(),
            ));
        }

        if instance_count > MAX_INSTANCES {
            return Err(GupError::invalid_operation(format!(
                "Instance count {instance_count} exceeds maximum {MAX_INSTANCES}"
            )));
        }

        // Grow buffers if needed (invalidates cached bind group).
        if instance_count > self.capacity {
            self.grow(device, instance_count);
        }

        // Resolve bind group: reuse cached or create new.
        let input_ptr: *const Buffer = input_buffer;
        let cache_hit = self
            .cached_bind_group
            .as_ref()
            .is_some_and(|c| std::ptr::eq(input_ptr, c.input_buffer_id));

        if !cache_hit {
            let bind_group = self.inner.create_bind_group(
                device,
                input_buffer,
                &self.output_buffer,
                &self.visibility_buffer,
                &self.prefix_sums_buffer,
                &self.draw_indirect_buffer,
                &self.config_buffer,
            );
            self.cached_bind_group = Some(CachedBindGroup {
                bind_group,
                input_buffer_id: input_ptr,
            });
        }

        // SAFETY: we just ensured cached_bind_group is Some above.
        let bind_group = &self.cached_bind_group.as_ref().unwrap().bind_group;

        // --- Encode & submit ---

        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("pooled_instance_filter_encoder"),
        });

        self.inner.encode_with_bind_group(
            queue,
            &mut encoder,
            bind_group,
            &self.config_buffer,
            instance_count,
            vertex_count,
            viewport,
            lod_thresholds,
        );

        queue.submit([encoder.finish()]);

        Ok(FilterResult {
            output_buffer: Arc::clone(&self.output_buffer),
            draw_indirect_buffer: Arc::clone(&self.draw_indirect_buffer),
        })
    }

    /// Ensure the pool can hold at least `min_instances` without
    /// reallocating during [`dispatch`](Self::dispatch).
    pub fn reserve(&mut self, device: &Device, min_instances: u32) {
        if min_instances > self.capacity {
            self.grow(device, min_instances);
        }
    }

    // ---------------------------------------------------------------
    // Internal helpers
    // ---------------------------------------------------------------

    /// Grow buffers to hold at least `new_min` instances.
    ///
    /// Invalidates the cached bind group since the underlying buffers change.
    fn grow(&mut self, device: &Device, new_min: u32) {
        // Round up to next power-of-two to amortise future growth.
        let new_capacity = new_min.next_power_of_two().min(MAX_INSTANCES);
        let (output, visibility, prefix_sums, draw_indirect, config) =
            Self::allocate_buffers(device, new_capacity);
        self.output_buffer = output;
        self.visibility_buffer = visibility;
        self.prefix_sums_buffer = prefix_sums;
        self.draw_indirect_buffer = draw_indirect;
        self.config_buffer = config;
        self.capacity = new_capacity;
        // Buffers changed — invalidate cached bind group.
        self.cached_bind_group = None;
    }

    /// Allocate a full set of transient buffers for `cap` instances.
    fn allocate_buffers(
        device: &Device,
        cap: u32,
    ) -> (Arc<Buffer>, Buffer, Buffer, Arc<Buffer>, Buffer) {
        let instance_size = std::mem::size_of::<InstanceAttributes>() as u64;
        let num_workgroups = cap.div_ceil(WORKGROUP_SIZE);

        let output_buffer = Arc::new(device.create_buffer(&BufferDescriptor {
            label: Some("pooled_filter_output_instances"),
            size: cap as u64 * instance_size,
            usage: BufferUsages::STORAGE | BufferUsages::VERTEX | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        }));

        let visibility_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("pooled_filter_visibility"),
            size: cap as u64 * 4,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let prefix_size = (cap + num_workgroups) as u64 * 4;
        let prefix_sums_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("pooled_filter_prefix_sums"),
            size: prefix_size,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let draw_indirect_buffer = Arc::new(device.create_buffer(&BufferDescriptor {
            label: Some("pooled_filter_draw_indirect"),
            size: 16,
            usage: BufferUsages::STORAGE | BufferUsages::INDIRECT | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        }));

        let config_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("pooled_filter_config"),
            size: std::mem::size_of::<FilterConfig>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        (
            output_buffer,
            visibility_buffer,
            prefix_sums_buffer,
            draw_indirect_buffer,
            config_buffer,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::GupContext;

    /// Helper: create a storage buffer from a slice of InstanceAttributes.
    fn create_instance_buffer(device: &Device, instances: &[InstanceAttributes]) -> Buffer {
        let data: &[u8] = bytemuck::cast_slice(instances);
        device.create_buffer(&BufferDescriptor {
            label: Some("test_input_instances"),
            size: data.len() as u64,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    fn upload_instances(queue: &Queue, buffer: &Buffer, instances: &[InstanceAttributes]) {
        queue.write_buffer(buffer, 0, bytemuck::cast_slice(instances));
    }

    // ------------------------------------------------------------------
    // Unit tests (no GPU)
    // ------------------------------------------------------------------

    #[test]
    fn test_filter_config_from_viewport() {
        let vp = Viewport2D::default();
        let thresholds = [4.0, 1.0, 0.25];
        let cfg = FilterConfig::from_viewport(&vp, &thresholds, 1000, 6);
        assert_eq!(cfg.min_x, -1.0);
        assert_eq!(cfg.max_x, 1.0);
        assert_eq!(cfg.instance_count, 1000);
        assert_eq!(cfg.vertex_count, 6);
        assert_eq!(cfg.lod_full, 4.0);
    }

    #[test]
    fn test_filter_config_size_alignment() {
        // FilterConfig must be Pod-safe and a multiple of 4 bytes.
        let size = std::mem::size_of::<FilterConfig>();
        assert_eq!(size % 4, 0, "FilterConfig size must be 4-byte aligned");
        assert_eq!(size, 48, "FilterConfig should be 12 × f32/u32 = 48 bytes");
    }

    // ------------------------------------------------------------------
    // GPU integration tests
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn test_compute_filter_creation() {
        let ctx = match GupContext::headless().await {
            Ok(c) => c,
            Err(_) => return, // No GPU available
        };
        let filter = ComputeInstanceFilter::new(&ctx.device);
        assert!(filter.is_ok(), "Pipeline creation should succeed");
    }

    #[tokio::test]
    async fn test_all_visible_instances() {
        let ctx = match GupContext::headless().await {
            Ok(c) => c,
            Err(_) => return,
        };
        let filter = ComputeInstanceFilter::new(&ctx.device).unwrap();

        // 4 circles, all inside the default viewport.
        let instances: Vec<InstanceAttributes> = (0..4)
            .map(|i| {
                let x = (i as f32 - 1.5) * 0.3;
                InstanceAttributes::from_circle([x, 0.0], 0.1, [1.0, 0.0, 0.0, 1.0])
            })
            .collect();

        let input_buf = create_instance_buffer(&ctx.device, &instances);
        upload_instances(&ctx.queue, &input_buf, &instances);

        let viewport = Viewport2D::default();
        let thresholds = [4.0, 1.0, 0.25];

        let result = filter
            .dispatch(
                &ctx.device,
                &ctx.queue,
                &input_buf,
                4,
                6,
                &viewport,
                &thresholds,
            )
            .await
            .unwrap();

        let args = ComputeInstanceFilter::read_draw_indirect(
            &ctx.device,
            &ctx.queue,
            &result.draw_indirect_buffer,
        )
        .await
        .unwrap();

        // All 4 should be visible.
        assert_eq!(args[0], 6, "vertex_count");
        assert_eq!(args[1], 4, "instance_count (all visible)");
    }

    #[tokio::test]
    async fn test_partial_culling() {
        let ctx = match GupContext::headless().await {
            Ok(c) => c,
            Err(_) => return,
        };
        let filter = ComputeInstanceFilter::new(&ctx.device).unwrap();

        // 4 circles: 2 inside viewport, 2 far outside.
        let instances = vec![
            InstanceAttributes::from_circle([0.0, 0.0], 0.1, [1.0, 0.0, 0.0, 1.0]),
            InstanceAttributes::from_circle([5.0, 5.0], 0.05, [0.0, 1.0, 0.0, 1.0]), // outside
            InstanceAttributes::from_circle([0.5, 0.5], 0.1, [0.0, 0.0, 1.0, 1.0]),
            InstanceAttributes::from_circle([-5.0, -5.0], 0.05, [1.0, 1.0, 0.0, 1.0]), // outside
        ];

        let input_buf = create_instance_buffer(&ctx.device, &instances);
        upload_instances(&ctx.queue, &input_buf, &instances);

        let viewport = Viewport2D::default();
        let thresholds = [4.0, 1.0, 0.25];

        let result = filter
            .dispatch(
                &ctx.device,
                &ctx.queue,
                &input_buf,
                4,
                6,
                &viewport,
                &thresholds,
            )
            .await
            .unwrap();

        let args = ComputeInstanceFilter::read_draw_indirect(
            &ctx.device,
            &ctx.queue,
            &result.draw_indirect_buffer,
        )
        .await
        .unwrap();

        assert_eq!(args[1], 2, "Only 2 instances should be visible");

        // Read output and verify colours match the visible instances.
        let output = ComputeInstanceFilter::read_output_instances(
            &ctx.device,
            &ctx.queue,
            &result.output_buffer,
            4,
        )
        .await
        .unwrap();

        assert_eq!(output.len(), 4); // buffer has space for 4, but only 2 valid
        // First visible: red circle at (0,0)
        assert_eq!(output[0].color[0], 1.0);
        assert_eq!(output[0].color[1], 0.0);
        // Second visible: blue circle at (0.5, 0.5)
        assert_eq!(output[1].color[2], 1.0);
    }

    #[tokio::test]
    async fn test_all_culled() {
        let ctx = match GupContext::headless().await {
            Ok(c) => c,
            Err(_) => return,
        };
        let filter = ComputeInstanceFilter::new(&ctx.device).unwrap();

        // All instances far outside viewport.
        let instances: Vec<InstanceAttributes> = (0..8)
            .map(|i| {
                InstanceAttributes::from_circle([10.0 + i as f32, 10.0], 0.01, [1.0, 1.0, 1.0, 1.0])
            })
            .collect();

        let input_buf = create_instance_buffer(&ctx.device, &instances);
        upload_instances(&ctx.queue, &input_buf, &instances);

        let viewport = Viewport2D::default();
        let thresholds = [4.0, 1.0, 0.25];

        let result = filter
            .dispatch(
                &ctx.device,
                &ctx.queue,
                &input_buf,
                8,
                6,
                &viewport,
                &thresholds,
            )
            .await
            .unwrap();

        let args = ComputeInstanceFilter::read_draw_indirect(
            &ctx.device,
            &ctx.queue,
            &result.draw_indirect_buffer,
        )
        .await
        .unwrap();

        assert_eq!(args[1], 0, "No instances should be visible");
    }

    #[tokio::test]
    async fn test_lod_culling_tiny_instances() {
        let ctx = match GupContext::headless().await {
            Ok(c) => c,
            Err(_) => return,
        };
        let filter = ComputeInstanceFilter::new(&ctx.device).unwrap();

        // Instances inside viewport but too small to render (below point threshold).
        // With pixel_width=800, clip_radius of 0.0001 → 0.0001*400 = 0.04 pixels < 0.25
        let instances: Vec<InstanceAttributes> = (0..4)
            .map(|_| InstanceAttributes::from_circle([0.0, 0.0], 0.0001, [1.0, 0.0, 0.0, 1.0]))
            .collect();

        let input_buf = create_instance_buffer(&ctx.device, &instances);
        upload_instances(&ctx.queue, &input_buf, &instances);

        let viewport = Viewport2D::default();
        let thresholds = [4.0, 1.0, 0.25];

        let result = filter
            .dispatch(
                &ctx.device,
                &ctx.queue,
                &input_buf,
                4,
                6,
                &viewport,
                &thresholds,
            )
            .await
            .unwrap();

        let args = ComputeInstanceFilter::read_draw_indirect(
            &ctx.device,
            &ctx.queue,
            &result.draw_indirect_buffer,
        )
        .await
        .unwrap();

        assert_eq!(args[1], 0, "Tiny instances should be LOD-culled");
    }

    #[tokio::test]
    async fn test_zero_instances_error() {
        let ctx = match GupContext::headless().await {
            Ok(c) => c,
            Err(_) => return,
        };
        let filter = ComputeInstanceFilter::new(&ctx.device).unwrap();

        let input_buf = ctx.device.create_buffer(&BufferDescriptor {
            label: Some("empty"),
            size: 96, // at least 1 instance
            usage: BufferUsages::STORAGE,
            mapped_at_creation: false,
        });

        let viewport = Viewport2D::default();
        let thresholds = [4.0, 1.0, 0.25];

        let result = filter
            .dispatch(
                &ctx.device,
                &ctx.queue,
                &input_buf,
                0,
                6,
                &viewport,
                &thresholds,
            )
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_multi_workgroup_prefix_sum() {
        let ctx = match GupContext::headless().await {
            Ok(c) => c,
            Err(_) => return,
        };
        let filter = ComputeInstanceFilter::new(&ctx.device).unwrap();

        // 512 instances = 2 workgroups, all visible → tests multi-workgroup prefix sum.
        let count = 512u32;
        let instances: Vec<InstanceAttributes> = (0..count)
            .map(|i| {
                let x = (i as f32 / count as f32) * 1.8 - 0.9;
                InstanceAttributes::from_circle([x, 0.0], 0.05, [1.0, 0.0, 0.0, 1.0])
            })
            .collect();

        let input_buf = create_instance_buffer(&ctx.device, &instances);
        upload_instances(&ctx.queue, &input_buf, &instances);

        let viewport = Viewport2D::default();
        let thresholds = [4.0, 1.0, 0.25];

        let result = filter
            .dispatch(
                &ctx.device,
                &ctx.queue,
                &input_buf,
                count,
                6,
                &viewport,
                &thresholds,
            )
            .await
            .unwrap();

        let args = ComputeInstanceFilter::read_draw_indirect(
            &ctx.device,
            &ctx.queue,
            &result.draw_indirect_buffer,
        )
        .await
        .unwrap();

        // All 512 should be visible (all within [-0.9, 0.9] range, viewport is [-1, 1]).
        assert_eq!(args[0], 6, "vertex_count");
        assert_eq!(
            args[1], count,
            "All {count} instances should be visible across 2 workgroups"
        );
    }

    #[tokio::test]
    async fn test_gpu_matches_cpu_culling() {
        let ctx = match GupContext::headless().await {
            Ok(c) => c,
            Err(_) => return,
        };
        let filter = ComputeInstanceFilter::new(&ctx.device).unwrap();

        // Create a mix of visible and non-visible instances.
        let instances = vec![
            InstanceAttributes::from_circle([0.0, 0.0], 0.2, [1.0, 0.0, 0.0, 1.0]),
            InstanceAttributes::from_circle([3.0, 0.0], 0.1, [0.0, 1.0, 0.0, 1.0]), // outside
            InstanceAttributes::from_circle([-0.5, 0.5], 0.15, [0.0, 0.0, 1.0, 1.0]),
            InstanceAttributes::from_circle([0.0, -3.0], 0.1, [1.0, 1.0, 0.0, 1.0]), // outside
            InstanceAttributes::from_circle([0.9, 0.9], 0.2, [0.5, 0.5, 0.5, 1.0]),
        ];

        // CPU path.
        let viewport = Viewport2D::default();
        let thresholds = [4.0, 1.0, 0.25];

        use super::super::CullingManager;
        use crate::mark::batch_renderer::BatchRendererConfig;
        let cfg = BatchRendererConfig {
            lod_thresholds: thresholds,
            ..Default::default()
        };
        let cm = CullingManager::new(&cfg);
        let mut cpu_visible = Vec::new();
        for inst in &instances {
            let pos = inst.position();
            let radius = inst.scale()[0]; // for circles, uniform scale = radius
            if cm.is_visible(pos[0], pos[1], radius) {
                let lod = cm.compute_lod(radius);
                if lod != super::super::LodLevel::Culled {
                    cpu_visible.push(*inst);
                }
            }
        }

        // GPU path.
        let input_buf = create_instance_buffer(&ctx.device, &instances);
        upload_instances(&ctx.queue, &input_buf, &instances);

        let result = filter
            .dispatch(
                &ctx.device,
                &ctx.queue,
                &input_buf,
                instances.len() as u32,
                6,
                &viewport,
                &thresholds,
            )
            .await
            .unwrap();

        let args = ComputeInstanceFilter::read_draw_indirect(
            &ctx.device,
            &ctx.queue,
            &result.draw_indirect_buffer,
        )
        .await
        .unwrap();

        assert_eq!(
            args[1] as usize,
            cpu_visible.len(),
            "GPU visible count should match CPU: got {} expected {}",
            args[1],
            cpu_visible.len()
        );

        // Verify output instances match CPU path.
        let gpu_output = ComputeInstanceFilter::read_output_instances(
            &ctx.device,
            &ctx.queue,
            &result.output_buffer,
            args[1],
        )
        .await
        .unwrap();

        for (i, (gpu, cpu)) in gpu_output.iter().zip(cpu_visible.iter()).enumerate() {
            let gpu_pos = gpu.position();
            let cpu_pos = cpu.position();
            assert!(
                (gpu_pos[0] - cpu_pos[0]).abs() < 1e-5 && (gpu_pos[1] - cpu_pos[1]).abs() < 1e-5,
                "Instance {i}: GPU position {:?} != CPU position {:?}",
                gpu_pos,
                cpu_pos
            );
        }
    }

    #[tokio::test]
    async fn test_large_scale_1k_correctness() {
        let ctx = match GupContext::headless().await {
            Ok(c) => c,
            Err(_) => return,
        };
        let filter = ComputeInstanceFilter::new(&ctx.device).unwrap();

        // 1024 instances — 4 workgroups, mix of visible and culled.
        let count = 1024u32;
        let instances: Vec<InstanceAttributes> = (0..count)
            .map(|i| {
                let t = i as f32 / count as f32;
                let x = (t * 37.0).sin() * 1.5; // some exceed [-1,1]
                let y = (t * 53.0).cos() * 1.5;
                let r = 0.001 + (t * 19.0).sin().abs() * 0.05;
                InstanceAttributes::from_circle([x, y], r, [t, 1.0 - t, 0.5, 1.0])
            })
            .collect();

        let input_buf = create_instance_buffer(&ctx.device, &instances);
        upload_instances(&ctx.queue, &input_buf, &instances);

        let viewport = Viewport2D::default();
        let thresholds = [4.0, 1.0, 0.25];

        // CPU reference.
        use super::super::CullingManager;
        use crate::mark::batch_renderer::BatchRendererConfig;
        let cfg = BatchRendererConfig {
            lod_thresholds: thresholds,
            ..Default::default()
        };
        let cm = CullingManager::new(&cfg);
        let cpu_count: usize = instances
            .iter()
            .filter(|inst| {
                let pos = inst.position();
                let radius = inst.scale()[0].max(inst.scale()[1]);
                cm.is_visible(pos[0], pos[1], radius)
                    && cm.compute_lod(radius) != super::super::LodLevel::Culled
            })
            .count();

        // GPU path.
        let result = filter
            .dispatch(
                &ctx.device,
                &ctx.queue,
                &input_buf,
                count,
                6,
                &viewport,
                &thresholds,
            )
            .await
            .unwrap();

        let args = ComputeInstanceFilter::read_draw_indirect(
            &ctx.device,
            &ctx.queue,
            &result.draw_indirect_buffer,
        )
        .await
        .unwrap();

        assert_eq!(
            args[1] as usize, cpu_count,
            "GPU visible count ({}) should match CPU count ({}) for {} instances",
            args[1], cpu_count, count
        );
    }

    #[tokio::test]
    async fn test_single_instance() {
        let ctx = match GupContext::headless().await {
            Ok(c) => c,
            Err(_) => return,
        };
        let filter = ComputeInstanceFilter::new(&ctx.device).unwrap();

        let instances = vec![InstanceAttributes::from_circle(
            [0.0, 0.0],
            0.1,
            [1.0, 0.0, 0.0, 1.0],
        )];

        let input_buf = create_instance_buffer(&ctx.device, &instances);
        upload_instances(&ctx.queue, &input_buf, &instances);

        let viewport = Viewport2D::default();
        let thresholds = [4.0, 1.0, 0.25];

        let result = filter
            .dispatch(
                &ctx.device,
                &ctx.queue,
                &input_buf,
                1,
                6,
                &viewport,
                &thresholds,
            )
            .await
            .unwrap();

        let args = ComputeInstanceFilter::read_draw_indirect(
            &ctx.device,
            &ctx.queue,
            &result.draw_indirect_buffer,
        )
        .await
        .unwrap();

        assert_eq!(args[0], 6, "vertex_count");
        assert_eq!(args[1], 1, "Single visible instance");
    }

    #[tokio::test]
    async fn test_exactly_256_instances() {
        let ctx = match GupContext::headless().await {
            Ok(c) => c,
            Err(_) => return,
        };
        let filter = ComputeInstanceFilter::new(&ctx.device).unwrap();

        // Exactly one workgroup, all visible.
        let count = 256u32;
        let instances: Vec<InstanceAttributes> = (0..count)
            .map(|i| {
                let x = (i as f32 / count as f32) * 1.8 - 0.9;
                InstanceAttributes::from_circle([x, 0.0], 0.05, [1.0, 0.0, 0.0, 1.0])
            })
            .collect();

        let input_buf = create_instance_buffer(&ctx.device, &instances);
        upload_instances(&ctx.queue, &input_buf, &instances);

        let viewport = Viewport2D::default();
        let thresholds = [4.0, 1.0, 0.25];

        let result = filter
            .dispatch(
                &ctx.device,
                &ctx.queue,
                &input_buf,
                count,
                6,
                &viewport,
                &thresholds,
            )
            .await
            .unwrap();

        let args = ComputeInstanceFilter::read_draw_indirect(
            &ctx.device,
            &ctx.queue,
            &result.draw_indirect_buffer,
        )
        .await
        .unwrap();

        assert_eq!(
            args[1], count,
            "All 256 instances in 1 workgroup should be visible"
        );
    }

    // ------------------------------------------------------------------
    // PooledComputeInstanceFilter tests
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn test_pooled_filter_creation() {
        let ctx = match GupContext::headless().await {
            Ok(c) => c,
            Err(_) => return,
        };
        let filter = ComputeInstanceFilter::new(&ctx.device).unwrap();
        let pooled = PooledComputeInstanceFilter::new(&ctx.device, filter, 1024);
        assert_eq!(pooled.capacity(), 1024);
    }

    #[tokio::test]
    async fn test_pooled_filter_all_visible() {
        let ctx = match GupContext::headless().await {
            Ok(c) => c,
            Err(_) => return,
        };
        let filter = ComputeInstanceFilter::new(&ctx.device).unwrap();
        let mut pooled = PooledComputeInstanceFilter::new(&ctx.device, filter, 256);

        let instances: Vec<InstanceAttributes> = (0..4)
            .map(|i| {
                let x = (i as f32 - 1.5) * 0.3;
                InstanceAttributes::from_circle([x, 0.0], 0.1, [1.0, 0.0, 0.0, 1.0])
            })
            .collect();

        let input_buf = create_instance_buffer(&ctx.device, &instances);
        upload_instances(&ctx.queue, &input_buf, &instances);

        let viewport = Viewport2D::default();
        let thresholds = [4.0, 1.0, 0.25];

        let result = pooled
            .dispatch(
                &ctx.device,
                &ctx.queue,
                &input_buf,
                4,
                6,
                &viewport,
                &thresholds,
            )
            .await
            .unwrap();

        let args = ComputeInstanceFilter::read_draw_indirect(
            &ctx.device,
            &ctx.queue,
            &result.draw_indirect_buffer,
        )
        .await
        .unwrap();

        assert_eq!(args[0], 6, "vertex_count");
        assert_eq!(args[1], 4, "instance_count (all visible)");
    }

    #[tokio::test]
    async fn test_pooled_filter_reuse_across_dispatches() {
        let ctx = match GupContext::headless().await {
            Ok(c) => c,
            Err(_) => return,
        };
        let filter = ComputeInstanceFilter::new(&ctx.device).unwrap();
        let mut pooled = PooledComputeInstanceFilter::new(&ctx.device, filter, 256);

        let viewport = Viewport2D::default();
        let thresholds = [4.0, 1.0, 0.25];

        // Dispatch 1: 4 circles, 2 visible.
        let instances_1 = vec![
            InstanceAttributes::from_circle([0.0, 0.0], 0.1, [1.0, 0.0, 0.0, 1.0]),
            InstanceAttributes::from_circle([5.0, 5.0], 0.05, [0.0, 1.0, 0.0, 1.0]),
            InstanceAttributes::from_circle([0.5, 0.5], 0.1, [0.0, 0.0, 1.0, 1.0]),
            InstanceAttributes::from_circle([-5.0, -5.0], 0.05, [1.0, 1.0, 0.0, 1.0]),
        ];

        let input_buf_1 = create_instance_buffer(&ctx.device, &instances_1);
        upload_instances(&ctx.queue, &input_buf_1, &instances_1);

        let result_1 = pooled
            .dispatch(
                &ctx.device,
                &ctx.queue,
                &input_buf_1,
                4,
                6,
                &viewport,
                &thresholds,
            )
            .await
            .unwrap();

        let args_1 = ComputeInstanceFilter::read_draw_indirect(
            &ctx.device,
            &ctx.queue,
            &result_1.draw_indirect_buffer,
        )
        .await
        .unwrap();
        assert_eq!(args_1[1], 2, "Dispatch 1: 2 visible");

        // Dispatch 2: 8 circles, all visible — reuses same buffers.
        let instances_2: Vec<InstanceAttributes> = (0..8)
            .map(|i| {
                let x = (i as f32 - 3.5) * 0.2;
                InstanceAttributes::from_circle([x, 0.0], 0.08, [0.0, 1.0, 1.0, 1.0])
            })
            .collect();

        let input_buf_2 = create_instance_buffer(&ctx.device, &instances_2);
        upload_instances(&ctx.queue, &input_buf_2, &instances_2);

        // Capacity should not have changed.
        assert_eq!(pooled.capacity(), 256);

        let result_2 = pooled
            .dispatch(
                &ctx.device,
                &ctx.queue,
                &input_buf_2,
                8,
                6,
                &viewport,
                &thresholds,
            )
            .await
            .unwrap();

        let args_2 = ComputeInstanceFilter::read_draw_indirect(
            &ctx.device,
            &ctx.queue,
            &result_2.draw_indirect_buffer,
        )
        .await
        .unwrap();
        assert_eq!(args_2[1], 8, "Dispatch 2: all 8 visible");
        assert_eq!(pooled.capacity(), 256, "Capacity unchanged");
    }

    #[tokio::test]
    async fn test_pooled_filter_auto_grow() {
        let ctx = match GupContext::headless().await {
            Ok(c) => c,
            Err(_) => return,
        };
        let filter = ComputeInstanceFilter::new(&ctx.device).unwrap();
        // Start very small.
        let mut pooled = PooledComputeInstanceFilter::new(&ctx.device, filter, 4);
        assert_eq!(pooled.capacity(), 4);

        // Dispatch with 16 instances — exceeds capacity, triggers growth.
        let instances: Vec<InstanceAttributes> = (0..16)
            .map(|i| {
                let x = (i as f32 / 16.0) * 1.8 - 0.9;
                InstanceAttributes::from_circle([x, 0.0], 0.05, [1.0, 0.0, 0.0, 1.0])
            })
            .collect();

        let input_buf = create_instance_buffer(&ctx.device, &instances);
        upload_instances(&ctx.queue, &input_buf, &instances);

        let viewport = Viewport2D::default();
        let thresholds = [4.0, 1.0, 0.25];

        let result = pooled
            .dispatch(
                &ctx.device,
                &ctx.queue,
                &input_buf,
                16,
                6,
                &viewport,
                &thresholds,
            )
            .await
            .unwrap();

        // Capacity should have grown to next power of two.
        assert_eq!(pooled.capacity(), 16);

        let args = ComputeInstanceFilter::read_draw_indirect(
            &ctx.device,
            &ctx.queue,
            &result.draw_indirect_buffer,
        )
        .await
        .unwrap();
        assert_eq!(args[0], 6, "vertex_count");
        assert_eq!(args[1], 16, "All 16 visible after grow");
    }

    #[tokio::test]
    async fn test_pooled_filter_reserve() {
        let ctx = match GupContext::headless().await {
            Ok(c) => c,
            Err(_) => return,
        };
        let filter = ComputeInstanceFilter::new(&ctx.device).unwrap();
        let mut pooled = PooledComputeInstanceFilter::new(&ctx.device, filter, 4);
        assert_eq!(pooled.capacity(), 4);

        // Reserve more than current capacity.
        pooled.reserve(&ctx.device, 1000);
        assert_eq!(pooled.capacity(), 1024, "Should round to next power of two");

        // Reserve less than current — no change.
        pooled.reserve(&ctx.device, 512);
        assert_eq!(pooled.capacity(), 1024, "Should not shrink");
    }

    #[tokio::test]
    async fn test_pooled_filter_correctness_matches_unpooled() {
        let ctx = match GupContext::headless().await {
            Ok(c) => c,
            Err(_) => return,
        };

        let count = 512u32;
        let instances: Vec<InstanceAttributes> = (0..count)
            .map(|i| {
                let t = i as f32 / count as f32;
                let x = (t * 37.0).sin() * 1.5;
                let y = (t * 53.0).cos() * 1.5;
                let r = 0.001 + (t * 19.0).sin().abs() * 0.05;
                InstanceAttributes::from_circle([x, y], r, [t, 1.0 - t, 0.5, 1.0])
            })
            .collect();

        let viewport = Viewport2D::default();
        let thresholds = [4.0, 1.0, 0.25];

        // Unpooled.
        let filter = ComputeInstanceFilter::new(&ctx.device).unwrap();
        let input_buf = create_instance_buffer(&ctx.device, &instances);
        upload_instances(&ctx.queue, &input_buf, &instances);

        let unpooled_result = filter
            .dispatch(
                &ctx.device,
                &ctx.queue,
                &input_buf,
                count,
                6,
                &viewport,
                &thresholds,
            )
            .await
            .unwrap();

        let unpooled_args = ComputeInstanceFilter::read_draw_indirect(
            &ctx.device,
            &ctx.queue,
            &unpooled_result.draw_indirect_buffer,
        )
        .await
        .unwrap();

        // Pooled.
        let filter2 = ComputeInstanceFilter::new(&ctx.device).unwrap();
        let mut pooled = PooledComputeInstanceFilter::new(&ctx.device, filter2, count);

        let input_buf2 = create_instance_buffer(&ctx.device, &instances);
        upload_instances(&ctx.queue, &input_buf2, &instances);

        let pooled_result = pooled
            .dispatch(
                &ctx.device,
                &ctx.queue,
                &input_buf2,
                count,
                6,
                &viewport,
                &thresholds,
            )
            .await
            .unwrap();

        let pooled_args = ComputeInstanceFilter::read_draw_indirect(
            &ctx.device,
            &ctx.queue,
            &pooled_result.draw_indirect_buffer,
        )
        .await
        .unwrap();

        // Results should match.
        assert_eq!(
            unpooled_args, pooled_args,
            "Pooled and unpooled should produce identical draw-indirect params"
        );

        // Also compare output instances.
        let visible_count = unpooled_args[1];

        let unpooled_output = ComputeInstanceFilter::read_output_instances(
            &ctx.device,
            &ctx.queue,
            &unpooled_result.output_buffer,
            visible_count,
        )
        .await
        .unwrap();

        let pooled_output = ComputeInstanceFilter::read_output_instances(
            &ctx.device,
            &ctx.queue,
            &pooled_result.output_buffer,
            visible_count,
        )
        .await
        .unwrap();

        for (i, (u, p)) in unpooled_output.iter().zip(pooled_output.iter()).enumerate() {
            let u_pos = u.position();
            let p_pos = p.position();
            assert!(
                (u_pos[0] - p_pos[0]).abs() < 1e-5 && (u_pos[1] - p_pos[1]).abs() < 1e-5,
                "Instance {i}: unpooled {:?} != pooled {:?}",
                u_pos,
                p_pos
            );
        }
    }

    #[tokio::test]
    async fn test_pooled_filter_zero_instances_error() {
        let ctx = match GupContext::headless().await {
            Ok(c) => c,
            Err(_) => return,
        };
        let filter = ComputeInstanceFilter::new(&ctx.device).unwrap();
        let mut pooled = PooledComputeInstanceFilter::new(&ctx.device, filter, 256);

        let input_buf = ctx.device.create_buffer(&BufferDescriptor {
            label: Some("empty"),
            size: 96,
            usage: BufferUsages::STORAGE,
            mapped_at_creation: false,
        });

        let viewport = Viewport2D::default();
        let thresholds = [4.0, 1.0, 0.25];

        let result = pooled
            .dispatch(
                &ctx.device,
                &ctx.queue,
                &input_buf,
                0,
                6,
                &viewport,
                &thresholds,
            )
            .await;

        assert!(result.is_err(), "Zero instances should return error");
    }
}

// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GPU compute-shader-based occlusion culling using a Hierarchical Z-Buffer.
//!
//! For dense datasets (>100K overlapping instances) where frustum culling
//! alone is insufficient, this module tests whether each instance is fully
//! hidden behind other instances and marks occluded instances as invisible.
//!
//! # Algorithm
//!
//! 1. **Build coverage** — Each instance writes its z-value (based on draw
//!    order / instance index) to a coarse coverage map via `atomicMax`.
//! 2. **Generate Hi-Z** — Successive mip levels store the *minimum* z of
//!    their 2×2 children, so a cell value represents the shallowest
//!    (earliest-drawn) mark in the region.
//! 3. **Occlusion test** — Each instance's bounding box is tested against
//!    the Hi-Z at an appropriate mip level. If the instance's z is less
//!    than all covering cells' Hi-Z values, it is fully behind later-drawn
//!    marks and can be culled.
//!
//! # Example
//!
//! ```rust,ignore
//! let culler = OcclusionCuller::new(&device)?;
//! let result = culler.dispatch(
//!     &device, &queue,
//!     &instance_buffer,
//!     100_000,          // instance count
//!     &viewport,
//!     &OcclusionParams::default(),
//! ).await?;
//!
//! // result.visibility_buffer[i] == 0 → occluded, 1 → visible
//! ```

use crate::error::{GupError, GupResult};
use std::sync::Arc;
use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingType, Buffer, BufferBindingType, BufferDescriptor, BufferUsages,
    CommandEncoderDescriptor, ComputePassDescriptor, ComputePipeline, ComputePipelineDescriptor,
    Device, PipelineLayoutDescriptor, PollType, Queue, ShaderModuleDescriptor, ShaderSource,
    ShaderStages,
};

use super::batch_renderer::Viewport2D;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Workgroup size used by all compute entry points.
const WORKGROUP_SIZE: u32 = 256;

/// Maximum number of Hi-Z mip levels (supports up to 4096×4096 base resolution).
const MAX_HIZ_LEVELS: usize = 12;

// ---------------------------------------------------------------------------
// GPU-side config uniform (must match WGSL `OcclusionConfig`)
// ---------------------------------------------------------------------------

/// Configuration uniform uploaded to the GPU for the occlusion compute shader.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct OcclusionGpuConfig {
    pub base_width: u32,
    pub base_height: u32,
    pub num_levels: u32,
    pub instance_count: u32,
    pub viewport_min_x: f32,
    pub viewport_max_x: f32,
    pub viewport_min_y: f32,
    pub viewport_max_y: f32,
    pub pixel_width: f32,
    pub pixel_height: f32,
    pub conservative_margin: f32,
    pub current_level: u32,
    /// Level offsets packed as 3 × `[u32; 4]` (matching `vec4<u32>` in WGSL).
    pub level_offsets_0: [u32; 4],
    pub level_offsets_1: [u32; 4],
    pub level_offsets_2: [u32; 4],
}

// ---------------------------------------------------------------------------
// User-facing parameters
// ---------------------------------------------------------------------------

/// Parameters controlling occlusion culling behaviour.
#[derive(Debug, Clone)]
pub struct OcclusionParams {
    /// Coverage map tile size in pixels (default: 4).
    /// Smaller values give finer culling but use more GPU memory.
    pub tile_size: u32,
    /// Conservative margin added to bounding boxes in clip-space units
    /// (default: 0.01). Larger values reduce false positives at the cost
    /// of fewer culled instances.
    pub conservative_margin: f32,
}

impl Default for OcclusionParams {
    fn default() -> Self {
        Self {
            tile_size: 4,
            conservative_margin: 0.01,
        }
    }
}

// ---------------------------------------------------------------------------
// Result
// ---------------------------------------------------------------------------

/// Result of an occlusion culling dispatch.
pub struct OcclusionResult {
    /// Per-instance visibility flags: `1` = visible, `0` = occluded.
    pub visibility_buffer: Arc<Buffer>,
    /// Number of instances that were tested.
    pub instance_count: u32,
    /// Hi-Z buffer (for debugging / visualisation).
    pub hiz_buffer: Arc<Buffer>,
}

// ---------------------------------------------------------------------------
// Hi-Z layout helpers
// ---------------------------------------------------------------------------

/// Compute the width or height of a mip level using ceiling division.
pub(crate) fn level_dim(base: u32, level: u32) -> u32 {
    base.div_ceil(1u32 << level).max(1)
}

/// Compute byte offsets (in u32 elements) for each mip level in the
/// concatenated Hi-Z buffer.
pub(crate) fn compute_level_offsets(
    base_width: u32,
    base_height: u32,
    num_levels: u32,
) -> [u32; MAX_HIZ_LEVELS] {
    let mut offsets = [0u32; MAX_HIZ_LEVELS];
    let mut offset = 0u32;
    for level in 0..num_levels.min(MAX_HIZ_LEVELS as u32) {
        offsets[level as usize] = offset;
        let w = level_dim(base_width, level);
        let h = level_dim(base_height, level);
        offset += w * h;
    }
    offsets
}

/// Total number of u32 cells across all mip levels.
pub(crate) fn total_hiz_cells(base_width: u32, base_height: u32, num_levels: u32) -> u32 {
    let mut total = 0u32;
    for level in 0..num_levels.min(MAX_HIZ_LEVELS as u32) {
        total += level_dim(base_width, level) * level_dim(base_height, level);
    }
    total
}

/// Number of mip levels needed for the given base dimensions (until 1×1).
pub(crate) fn mip_count(base_width: u32, base_height: u32) -> u32 {
    let max_dim = base_width.max(base_height);
    if max_dim == 0 {
        return 1;
    }
    (32 - max_dim.leading_zeros()).min(MAX_HIZ_LEVELS as u32)
}

// ---------------------------------------------------------------------------
// OcclusionCuller
// ---------------------------------------------------------------------------

/// GPU compute pipeline for hierarchical Z-buffer occlusion culling.
///
/// Identifies instances that are fully hidden behind other instances in
/// screen space, based on draw order (instance index). Designed for dense
/// datasets (>100K overlapping marks) where frustum culling alone is not
/// enough.
pub struct OcclusionCuller {
    build_coverage_pipeline: ComputePipeline,
    generate_hiz_pipeline: ComputePipeline,
    occlusion_test_pipeline: ComputePipeline,
    /// Combined occlusion test that preserves existing visibility flags.
    occlusion_test_combined_pipeline: ComputePipeline,
    bind_group_layout: BindGroupLayout,
}

impl OcclusionCuller {
    /// Create a new occlusion culler by compiling the WGSL shader.
    pub fn new(device: &Device) -> GupResult<Self> {
        let shader_source = include_str!("../shaders/occlusion_culling.compute.wgsl");

        let shader_module = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("occlusion_culling_compute"),
            source: ShaderSource::Wgsl(shader_source.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("occlusion_culling_bgl"),
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
                // binding 1: hiz_buffer (read-write storage)
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
                // binding 2: visibility (read-write storage)
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
                // binding 3: config (uniform)
                BindGroupLayoutEntry {
                    binding: 3,
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
            label: Some("occlusion_culling_layout"),
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

        let build_coverage_pipeline = make_pipeline("build_coverage_pipeline", "build_coverage");
        let generate_hiz_pipeline =
            make_pipeline("generate_hiz_level_pipeline", "generate_hiz_level");
        let occlusion_test_pipeline = make_pipeline("occlusion_test_pipeline", "occlusion_test");
        let occlusion_test_combined_pipeline =
            make_pipeline("occlusion_test_combined_pipeline", "occlusion_test_combined");

        Ok(Self {
            build_coverage_pipeline,
            generate_hiz_pipeline,
            occlusion_test_pipeline,
            occlusion_test_combined_pipeline,
            bind_group_layout,
        })
    }

    /// Run the full occlusion culling pipeline on the GPU.
    ///
    /// `input_buffer` must be a storage buffer of `InstanceAttributes`.
    /// Returns an [`OcclusionResult`] with per-instance visibility flags.
    pub async fn dispatch(
        &self,
        device: &Device,
        queue: &Queue,
        input_buffer: &Buffer,
        instance_count: u32,
        viewport: &Viewport2D,
        params: &OcclusionParams,
    ) -> GupResult<OcclusionResult> {
        if instance_count == 0 {
            return Err(GupError::invalid_operation(
                "Cannot run occlusion culling on zero instances".to_string(),
            ));
        }

        // Compute Hi-Z dimensions.
        let base_width = (viewport.pixel_width as u32)
            .div_ceil(params.tile_size)
            .max(1);
        let base_height = (viewport.pixel_height as u32)
            .div_ceil(params.tile_size)
            .max(1);
        let num_levels = mip_count(base_width, base_height);
        let offsets = compute_level_offsets(base_width, base_height, num_levels);
        let total_cells = total_hiz_cells(base_width, base_height, num_levels);

        // --- Allocate GPU buffers ---

        let hiz_buffer = Arc::new(device.create_buffer(&BufferDescriptor {
            label: Some("occlusion_hiz_buffer"),
            // Minimum 4 bytes to satisfy wgpu validation.
            size: (total_cells as u64 * 4).max(4),
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        }));

        let visibility_buffer = Arc::new(device.create_buffer(&BufferDescriptor {
            label: Some("occlusion_visibility"),
            size: instance_count as u64 * 4,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        }));

        let config_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("occlusion_config"),
            size: std::mem::size_of::<OcclusionGpuConfig>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Pre-stage level numbers for copy_buffer_to_buffer.
        let level_data: Vec<u32> = (0..num_levels).collect();
        let level_staging = device.create_buffer(&BufferDescriptor {
            label: Some("occlusion_level_staging"),
            size: (num_levels as u64 * 4).max(4),
            usage: BufferUsages::COPY_SRC | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&level_staging, 0, bytemuck::cast_slice(&level_data));

        // Build initial config (current_level = 0, used by build_coverage).
        let gpu_config = OcclusionGpuConfig {
            base_width,
            base_height,
            num_levels,
            instance_count,
            viewport_min_x: viewport.min_x,
            viewport_max_x: viewport.max_x,
            viewport_min_y: viewport.min_y,
            viewport_max_y: viewport.max_y,
            pixel_width: viewport.pixel_width,
            pixel_height: viewport.pixel_height,
            conservative_margin: params.conservative_margin,
            current_level: 0,
            level_offsets_0: [offsets[0], offsets[1], offsets[2], offsets[3]],
            level_offsets_1: [offsets[4], offsets[5], offsets[6], offsets[7]],
            level_offsets_2: [offsets[8], offsets[9], offsets[10], offsets[11]],
        };
        queue.write_buffer(&config_buffer, 0, bytemuck::bytes_of(&gpu_config));

        // Create bind group.
        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("occlusion_culling_bg"),
            layout: &self.bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: input_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: hiz_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: visibility_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 3,
                    resource: config_buffer.as_entire_binding(),
                },
            ],
        });

        // --- Encode compute passes ---

        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("occlusion_culling_encoder"),
        });

        // Clear Hi-Z buffer to zero.
        encoder.clear_buffer(&hiz_buffer, 0, None);

        let num_instance_workgroups = instance_count.div_ceil(WORKGROUP_SIZE);

        // Pass 1: Build coverage (level 0).
        {
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("build_coverage"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.build_coverage_pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(num_instance_workgroups, 1, 1);
        }

        // Pass 2: Generate Hi-Z mip levels 1..num_levels.
        // Byte offset of `current_level` in OcclusionGpuConfig.
        const CURRENT_LEVEL_OFFSET: u64 = 44;
        for level in 1..num_levels {
            // Update current_level in the config buffer.
            encoder.copy_buffer_to_buffer(
                &level_staging,
                level as u64 * 4,
                &config_buffer,
                CURRENT_LEVEL_OFFSET,
                4,
            );
            let level_cells = level_dim(base_width, level) * level_dim(base_height, level);
            let workgroups = level_cells.div_ceil(WORKGROUP_SIZE);
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("generate_hiz_level"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.generate_hiz_pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(workgroups, 1, 1);
        }

        // Pass 3: Occlusion test.
        {
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("occlusion_test"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.occlusion_test_pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(num_instance_workgroups, 1, 1);
        }

        queue.submit([encoder.finish()]);

        Ok(OcclusionResult {
            visibility_buffer,
            instance_count,
            hiz_buffer,
        })
    }

    /// Encode occlusion culling passes into an existing command encoder.
    ///
    /// This is used by the unified culling pipeline to combine frustum and
    /// occlusion culling in a single command encoder. Unlike [`dispatch`],
    /// this method does not create its own encoder or submit work.
    ///
    /// The `visibility_buffer` is expected to already contain frustum-cull
    /// flags from a prior pass. The combined occlusion test will only clear
    /// visible instances to 0 if they are occluded — it never writes 1.
    ///
    /// `hiz_buffer` must be pre-allocated with at least `total_hiz_cells()`
    /// elements. `occlusion_config_buffer` must be pre-allocated for
    /// [`OcclusionGpuConfig`].
    ///
    /// [`dispatch`]: Self::dispatch
    #[allow(clippy::too_many_arguments)]
    pub fn encode_combined(
        &self,
        queue: &Queue,
        encoder: &mut wgpu::CommandEncoder,
        bind_group: &BindGroup,
        occlusion_config_buffer: &Buffer,
        level_staging: &Buffer,
        hiz_buffer: &Buffer,
        instance_count: u32,
        viewport: &Viewport2D,
        params: &OcclusionParams,
    ) {
        let base_width = (viewport.pixel_width as u32)
            .div_ceil(params.tile_size)
            .max(1);
        let base_height = (viewport.pixel_height as u32)
            .div_ceil(params.tile_size)
            .max(1);
        let num_levels = mip_count(base_width, base_height);
        let offsets = compute_level_offsets(base_width, base_height, num_levels);

        // Upload level staging data.
        let level_data: Vec<u32> = (0..num_levels).collect();
        queue.write_buffer(level_staging, 0, bytemuck::cast_slice(&level_data));

        // Upload config.
        let gpu_config = OcclusionGpuConfig {
            base_width,
            base_height,
            num_levels,
            instance_count,
            viewport_min_x: viewport.min_x,
            viewport_max_x: viewport.max_x,
            viewport_min_y: viewport.min_y,
            viewport_max_y: viewport.max_y,
            pixel_width: viewport.pixel_width,
            pixel_height: viewport.pixel_height,
            conservative_margin: params.conservative_margin,
            current_level: 0,
            level_offsets_0: [offsets[0], offsets[1], offsets[2], offsets[3]],
            level_offsets_1: [offsets[4], offsets[5], offsets[6], offsets[7]],
            level_offsets_2: [offsets[8], offsets[9], offsets[10], offsets[11]],
        };
        queue.write_buffer(occlusion_config_buffer, 0, bytemuck::bytes_of(&gpu_config));

        // Clear Hi-Z buffer.
        encoder.clear_buffer(hiz_buffer, 0, None);

        let num_instance_workgroups = instance_count.div_ceil(WORKGROUP_SIZE);

        // Build coverage (level 0).
        {
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("unified_build_coverage"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.build_coverage_pipeline);
            pass.set_bind_group(0, bind_group, &[]);
            pass.dispatch_workgroups(num_instance_workgroups, 1, 1);
        }

        // Generate Hi-Z mip levels.
        const CURRENT_LEVEL_OFFSET: u64 = 44;
        for level in 1..num_levels {
            encoder.copy_buffer_to_buffer(
                level_staging,
                level as u64 * 4,
                occlusion_config_buffer,
                CURRENT_LEVEL_OFFSET,
                4,
            );
            let level_cells = level_dim(base_width, level) * level_dim(base_height, level);
            let workgroups = level_cells.div_ceil(WORKGROUP_SIZE);
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("unified_generate_hiz_level"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.generate_hiz_pipeline);
            pass.set_bind_group(0, bind_group, &[]);
            pass.dispatch_workgroups(workgroups, 1, 1);
        }

        // Combined occlusion test (preserves existing visibility flags).
        {
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("unified_occlusion_test_combined"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.occlusion_test_combined_pipeline);
            pass.set_bind_group(0, bind_group, &[]);
            pass.dispatch_workgroups(num_instance_workgroups, 1, 1);
        }
    }

    /// Create a bind group for occlusion passes.
    ///
    /// Used by [`UnifiedCullingPipeline`] to create a bind group that
    /// references the filter's visibility buffer.
    ///
    /// [`UnifiedCullingPipeline`]: super::unified_culling_pipeline::UnifiedCullingPipeline
    pub fn create_bind_group(
        &self,
        device: &Device,
        input_buffer: &Buffer,
        hiz_buffer: &Buffer,
        visibility_buffer: &Buffer,
        config_buffer: &Buffer,
    ) -> BindGroup {
        device.create_bind_group(&BindGroupDescriptor {
            label: Some("occlusion_culling_bg"),
            layout: &self.bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: input_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: hiz_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: visibility_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 3,
                    resource: config_buffer.as_entire_binding(),
                },
            ],
        })
    }

    /// Read back the per-instance visibility flags from the GPU.
    ///
    /// Primarily useful for testing and diagnostics.
    pub async fn read_visibility(
        device: &Device,
        queue: &Queue,
        visibility_buffer: &Buffer,
        count: u32,
    ) -> GupResult<Vec<u32>> {
        let size = count as u64 * 4;
        let staging = device.create_buffer(&BufferDescriptor {
            label: Some("occlusion_visibility_staging"),
            size,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("occlusion_visibility_readback"),
        });
        encoder.copy_buffer_to_buffer(visibility_buffer, 0, &staging, 0, size);
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
            .map_err(|_| GupError::render_error("Visibility readback channel closed"))?
            .map_err(|e| GupError::render_error(format!("Visibility map failed: {e:?}")))?;

        let data = slice.get_mapped_range();
        let flags: &[u32] = bytemuck::cast_slice(&data);
        let result = flags.to_vec();
        drop(data);
        staging.unmap();

        Ok(result)
    }

    /// Read back the Hi-Z buffer from the GPU.
    ///
    /// Primarily useful for testing and diagnostics.
    pub async fn read_hiz_buffer(
        device: &Device,
        queue: &Queue,
        hiz_buffer: &Buffer,
        total_cells: u32,
    ) -> GupResult<Vec<u32>> {
        let size = total_cells as u64 * 4;
        let staging = device.create_buffer(&BufferDescriptor {
            label: Some("occlusion_hiz_staging"),
            size,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("occlusion_hiz_readback"),
        });
        encoder.copy_buffer_to_buffer(hiz_buffer, 0, &staging, 0, size);
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
            .map_err(|_| GupError::render_error("Hi-Z readback channel closed"))?
            .map_err(|e| GupError::render_error(format!("Hi-Z map failed: {e:?}")))?;

        let data = slice.get_mapped_range();
        let values: &[u32] = bytemuck::cast_slice(&data);
        let result = values.to_vec();
        drop(data);
        staging.unmap();

        Ok(result)
    }
}

// ---------------------------------------------------------------------------
// Pooled occlusion culler
// ---------------------------------------------------------------------------

/// Pre-allocated GPU buffer pool for [`OcclusionCuller`].
///
/// Eliminates per-dispatch buffer allocation by pre-allocating the Hi-Z,
/// visibility, and config buffers. Buffers are grown automatically when
/// the instance count or viewport exceeds the current capacity.
pub struct PooledOcclusionCuller {
    inner: OcclusionCuller,
    hiz_buffer: Arc<Buffer>,
    visibility_buffer: Arc<Buffer>,
    config_buffer: Buffer,
    level_staging: Buffer,
    /// Current maximum instance capacity.
    instance_capacity: u32,
    /// Current Hi-Z cell capacity.
    hiz_capacity: u32,
    /// Cached bind group.
    cached_bind_group: Option<CachedOcclusionBindGroup>,
}

struct CachedOcclusionBindGroup {
    bind_group: BindGroup,
    input_buffer_id: *const Buffer,
}

// SAFETY: input_buffer_id is only used for pointer identity comparison.
unsafe impl Send for CachedOcclusionBindGroup {}
unsafe impl Sync for CachedOcclusionBindGroup {}

impl PooledOcclusionCuller {
    /// Create a new pooled occlusion culler.
    pub fn new(
        device: &Device,
        inner: OcclusionCuller,
        max_instances: u32,
        viewport: &Viewport2D,
        params: &OcclusionParams,
    ) -> Self {
        let base_width = (viewport.pixel_width as u32)
            .div_ceil(params.tile_size)
            .max(1);
        let base_height = (viewport.pixel_height as u32)
            .div_ceil(params.tile_size)
            .max(1);
        let num_levels = mip_count(base_width, base_height);
        let total_cells = total_hiz_cells(base_width, base_height, num_levels);

        let hiz_buffer = Arc::new(device.create_buffer(&BufferDescriptor {
            label: Some("pooled_occlusion_hiz"),
            size: (total_cells as u64 * 4).max(4),
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        }));

        let visibility_buffer = Arc::new(device.create_buffer(&BufferDescriptor {
            label: Some("pooled_occlusion_visibility"),
            size: max_instances as u64 * 4,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        }));

        let config_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("pooled_occlusion_config"),
            size: std::mem::size_of::<OcclusionGpuConfig>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let max_levels = mip_count(
            (viewport.pixel_width as u32)
                .div_ceil(params.tile_size)
                .max(1),
            (viewport.pixel_height as u32)
                .div_ceil(params.tile_size)
                .max(1),
        );
        let level_staging = device.create_buffer(&BufferDescriptor {
            label: Some("pooled_occlusion_level_staging"),
            size: (max_levels as u64 * 4).max(4),
            usage: BufferUsages::COPY_SRC | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            inner,
            hiz_buffer,
            visibility_buffer,
            config_buffer,
            level_staging,
            instance_capacity: max_instances,
            hiz_capacity: total_cells,
            cached_bind_group: None,
        }
    }

    /// Current instance capacity.
    pub fn instance_capacity(&self) -> u32 {
        self.instance_capacity
    }

    /// Access the underlying culler.
    pub fn inner(&self) -> &OcclusionCuller {
        &self.inner
    }

    /// Run occlusion culling, reusing pre-allocated buffers.
    #[allow(clippy::too_many_arguments)]
    pub async fn dispatch(
        &mut self,
        device: &Device,
        queue: &Queue,
        input_buffer: &Buffer,
        instance_count: u32,
        viewport: &Viewport2D,
        params: &OcclusionParams,
    ) -> GupResult<OcclusionResult> {
        if instance_count == 0 {
            return Err(GupError::invalid_operation(
                "Cannot run occlusion culling on zero instances".to_string(),
            ));
        }

        let base_width = (viewport.pixel_width as u32)
            .div_ceil(params.tile_size)
            .max(1);
        let base_height = (viewport.pixel_height as u32)
            .div_ceil(params.tile_size)
            .max(1);
        let num_levels = mip_count(base_width, base_height);
        let total_cells = total_hiz_cells(base_width, base_height, num_levels);

        // Grow buffers if needed.
        if instance_count > self.instance_capacity || total_cells > self.hiz_capacity {
            self.grow(device, instance_count, total_cells, num_levels);
        }

        let offsets = compute_level_offsets(base_width, base_height, num_levels);

        // Upload level staging data.
        let level_data: Vec<u32> = (0..num_levels).collect();
        queue.write_buffer(&self.level_staging, 0, bytemuck::cast_slice(&level_data));

        // Upload config.
        let gpu_config = OcclusionGpuConfig {
            base_width,
            base_height,
            num_levels,
            instance_count,
            viewport_min_x: viewport.min_x,
            viewport_max_x: viewport.max_x,
            viewport_min_y: viewport.min_y,
            viewport_max_y: viewport.max_y,
            pixel_width: viewport.pixel_width,
            pixel_height: viewport.pixel_height,
            conservative_margin: params.conservative_margin,
            current_level: 0,
            level_offsets_0: [offsets[0], offsets[1], offsets[2], offsets[3]],
            level_offsets_1: [offsets[4], offsets[5], offsets[6], offsets[7]],
            level_offsets_2: [offsets[8], offsets[9], offsets[10], offsets[11]],
        };
        queue.write_buffer(&self.config_buffer, 0, bytemuck::bytes_of(&gpu_config));

        // Resolve bind group.
        let input_ptr: *const Buffer = input_buffer;
        let cache_hit = self
            .cached_bind_group
            .as_ref()
            .is_some_and(|c| std::ptr::eq(input_ptr, c.input_buffer_id));

        if !cache_hit {
            let bind_group = device.create_bind_group(&BindGroupDescriptor {
                label: Some("pooled_occlusion_bg"),
                layout: &self.inner.bind_group_layout,
                entries: &[
                    BindGroupEntry {
                        binding: 0,
                        resource: input_buffer.as_entire_binding(),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: self.hiz_buffer.as_entire_binding(),
                    },
                    BindGroupEntry {
                        binding: 2,
                        resource: self.visibility_buffer.as_entire_binding(),
                    },
                    BindGroupEntry {
                        binding: 3,
                        resource: self.config_buffer.as_entire_binding(),
                    },
                ],
            });
            self.cached_bind_group = Some(CachedOcclusionBindGroup {
                bind_group,
                input_buffer_id: input_ptr,
            });
        }

        let bind_group = &self.cached_bind_group.as_ref().unwrap().bind_group;

        // Encode compute passes.
        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("pooled_occlusion_encoder"),
        });

        encoder.clear_buffer(&self.hiz_buffer, 0, None);

        let num_instance_workgroups = instance_count.div_ceil(WORKGROUP_SIZE);

        {
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("build_coverage"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.inner.build_coverage_pipeline);
            pass.set_bind_group(0, bind_group, &[]);
            pass.dispatch_workgroups(num_instance_workgroups, 1, 1);
        }

        const CURRENT_LEVEL_OFFSET: u64 = 44;
        for level in 1..num_levels {
            encoder.copy_buffer_to_buffer(
                &self.level_staging,
                level as u64 * 4,
                &self.config_buffer,
                CURRENT_LEVEL_OFFSET,
                4,
            );
            let level_cells = level_dim(base_width, level) * level_dim(base_height, level);
            let workgroups = level_cells.div_ceil(WORKGROUP_SIZE);
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("generate_hiz_level"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.inner.generate_hiz_pipeline);
            pass.set_bind_group(0, bind_group, &[]);
            pass.dispatch_workgroups(workgroups, 1, 1);
        }

        {
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("occlusion_test"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.inner.occlusion_test_pipeline);
            pass.set_bind_group(0, bind_group, &[]);
            pass.dispatch_workgroups(num_instance_workgroups, 1, 1);
        }

        queue.submit([encoder.finish()]);

        Ok(OcclusionResult {
            visibility_buffer: Arc::clone(&self.visibility_buffer),
            instance_count,
            hiz_buffer: Arc::clone(&self.hiz_buffer),
        })
    }

    fn grow(
        &mut self,
        device: &Device,
        new_instance_count: u32,
        new_hiz_cells: u32,
        num_levels: u32,
    ) {
        let inst_cap = new_instance_count.next_power_of_two();
        let hiz_cap = new_hiz_cells.next_power_of_two();

        self.hiz_buffer = Arc::new(device.create_buffer(&BufferDescriptor {
            label: Some("pooled_occlusion_hiz"),
            size: (hiz_cap as u64 * 4).max(4),
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        }));

        self.visibility_buffer = Arc::new(device.create_buffer(&BufferDescriptor {
            label: Some("pooled_occlusion_visibility"),
            size: inst_cap as u64 * 4,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        }));

        self.level_staging = device.create_buffer(&BufferDescriptor {
            label: Some("pooled_occlusion_level_staging"),
            size: (num_levels as u64 * 4).max(4),
            usage: BufferUsages::COPY_SRC | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        self.instance_capacity = inst_cap;
        self.hiz_capacity = hiz_cap;
        self.cached_bind_group = None;
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::GupContext;
    use crate::mark::InstanceAttributes;

    /// Helper: create a storage buffer from a slice of InstanceAttributes.
    fn create_instance_buffer(device: &Device, instances: &[InstanceAttributes]) -> Buffer {
        let data: &[u8] = bytemuck::cast_slice(instances);
        device.create_buffer(&BufferDescriptor {
            label: Some("test_occlusion_input"),
            size: data.len() as u64,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    fn upload_instances(queue: &Queue, buffer: &Buffer, instances: &[InstanceAttributes]) {
        queue.write_buffer(buffer, 0, bytemuck::cast_slice(instances));
    }

    // --- Unit tests (no GPU) ---

    #[test]
    fn test_level_dim() {
        assert_eq!(level_dim(200, 0), 200);
        assert_eq!(level_dim(200, 1), 100);
        assert_eq!(level_dim(200, 2), 50);
        assert_eq!(level_dim(150, 0), 150);
        assert_eq!(level_dim(150, 1), 75);
        assert_eq!(level_dim(150, 2), 38);
        assert_eq!(level_dim(150, 3), 19);
        assert_eq!(level_dim(1, 0), 1);
        assert_eq!(level_dim(1, 1), 1);
    }

    #[test]
    fn test_mip_count() {
        assert_eq!(mip_count(200, 150), 8);
        assert_eq!(mip_count(1, 1), 1);
        assert_eq!(mip_count(2, 2), 2);
        assert_eq!(mip_count(256, 256), 9);
    }

    #[test]
    fn test_compute_level_offsets() {
        let offsets = compute_level_offsets(4, 4, 3);
        assert_eq!(offsets[0], 0); // Level 0: 4×4 = 16
        assert_eq!(offsets[1], 16); // Level 1: 2×2 = 4
        assert_eq!(offsets[2], 20); // Level 2: 1×1 = 1
    }

    #[test]
    fn test_total_hiz_cells() {
        assert_eq!(total_hiz_cells(4, 4, 3), 21); // 16 + 4 + 1
        assert_eq!(total_hiz_cells(200, 150, 1), 30000);
    }

    #[test]
    fn test_gpu_config_size() {
        let size = std::mem::size_of::<OcclusionGpuConfig>();
        assert_eq!(size, 96, "OcclusionGpuConfig must be 96 bytes");
        assert_eq!(size % 16, 0, "Must be 16-byte aligned for uniform buffer");
    }

    #[test]
    fn test_occlusion_params_default() {
        let params = OcclusionParams::default();
        assert_eq!(params.tile_size, 4);
        assert!((params.conservative_margin - 0.01).abs() < f32::EPSILON);
    }

    // --- GPU integration tests ---

    #[tokio::test]
    async fn test_occlusion_culler_creation() {
        let ctx = match GupContext::headless().await {
            Ok(c) => c,
            Err(_) => return,
        };
        let culler = OcclusionCuller::new(&ctx.device);
        assert!(culler.is_ok(), "Pipeline creation should succeed");
    }

    #[tokio::test]
    async fn test_no_occlusion_sparse_instances() {
        let ctx = match GupContext::headless().await {
            Ok(c) => c,
            Err(_) => return,
        };
        let culler = OcclusionCuller::new(&ctx.device).unwrap();

        // 4 non-overlapping circles spread across the viewport.
        let instances = vec![
            InstanceAttributes::from_circle([-0.5, -0.5], 0.1, [1.0, 0.0, 0.0, 1.0]),
            InstanceAttributes::from_circle([0.5, -0.5], 0.1, [0.0, 1.0, 0.0, 1.0]),
            InstanceAttributes::from_circle([-0.5, 0.5], 0.1, [0.0, 0.0, 1.0, 1.0]),
            InstanceAttributes::from_circle([0.5, 0.5], 0.1, [1.0, 1.0, 0.0, 1.0]),
        ];

        let input_buf = create_instance_buffer(&ctx.device, &instances);
        upload_instances(&ctx.queue, &input_buf, &instances);

        let viewport = Viewport2D::default();
        let params = OcclusionParams::default();

        let result = culler
            .dispatch(&ctx.device, &ctx.queue, &input_buf, 4, &viewport, &params)
            .await
            .unwrap();

        let flags =
            OcclusionCuller::read_visibility(&ctx.device, &ctx.queue, &result.visibility_buffer, 4)
                .await
                .unwrap();

        // All 4 should be visible (no overlap).
        assert_eq!(
            flags,
            vec![1, 1, 1, 1],
            "Sparse instances should all be visible"
        );
    }

    #[tokio::test]
    async fn test_occlusion_stacked_instances() {
        let ctx = match GupContext::headless().await {
            Ok(c) => c,
            Err(_) => return,
        };
        let culler = OcclusionCuller::new(&ctx.device).unwrap();

        // Many circles stacked at the same position. Later instances (higher
        // index) are "on top" and should occlude earlier ones.
        let n = 50;
        let instances: Vec<InstanceAttributes> = (0..n)
            .map(|_i| InstanceAttributes::from_circle([0.0, 0.0], 0.2, [1.0, 0.0, 0.0, 1.0]))
            .collect();

        let input_buf = create_instance_buffer(&ctx.device, &instances);
        upload_instances(&ctx.queue, &input_buf, &instances);

        let viewport = Viewport2D::default();
        let params = OcclusionParams {
            tile_size: 4,
            conservative_margin: 0.0, // No margin for exact testing.
        };

        let result = culler
            .dispatch(&ctx.device, &ctx.queue, &input_buf, n, &viewport, &params)
            .await
            .unwrap();

        let flags =
            OcclusionCuller::read_visibility(&ctx.device, &ctx.queue, &result.visibility_buffer, n)
                .await
                .unwrap();

        // The last instance (highest z) should always be visible.
        assert_eq!(
            flags[(n - 1) as usize],
            1,
            "Last instance (front) must be visible"
        );

        // At least some earlier instances should be culled.
        let visible_count: u32 = flags.iter().sum();
        let culled_count = n - visible_count;
        assert!(
            culled_count > 0,
            "Stacked instances should have some occlusion culling: visible={visible_count}, culled={culled_count}"
        );
    }

    #[tokio::test]
    async fn test_transparent_marks_not_occluders() {
        let ctx = match GupContext::headless().await {
            Ok(c) => c,
            Err(_) => return,
        };
        let culler = OcclusionCuller::new(&ctx.device).unwrap();

        // Two circles at the same position: one transparent (on top), one opaque (below).
        // The transparent mark should NOT occlude the opaque one.
        let instances = vec![
            InstanceAttributes::from_circle([0.0, 0.0], 0.2, [1.0, 0.0, 0.0, 1.0]), // opaque, z=1
            InstanceAttributes::from_circle([0.0, 0.0], 0.2, [0.0, 1.0, 0.0, 0.5]), // transparent, z=2
        ];

        let input_buf = create_instance_buffer(&ctx.device, &instances);
        upload_instances(&ctx.queue, &input_buf, &instances);

        let viewport = Viewport2D::default();
        let params = OcclusionParams {
            tile_size: 4,
            conservative_margin: 0.0,
        };

        let result = culler
            .dispatch(&ctx.device, &ctx.queue, &input_buf, 2, &viewport, &params)
            .await
            .unwrap();

        let flags =
            OcclusionCuller::read_visibility(&ctx.device, &ctx.queue, &result.visibility_buffer, 2)
                .await
                .unwrap();

        // Both should be visible: the opaque mark is not occluded because
        // the transparent mark doesn't write to the coverage map.
        assert_eq!(flags[0], 1, "Opaque mark should be visible");
        assert_eq!(flags[1], 1, "Transparent mark should be visible");
    }

    #[tokio::test]
    async fn test_pooled_occlusion_culler() {
        let ctx = match GupContext::headless().await {
            Ok(c) => c,
            Err(_) => return,
        };
        let inner = OcclusionCuller::new(&ctx.device).unwrap();
        let viewport = Viewport2D::default();
        let params = OcclusionParams::default();
        let mut pooled = PooledOcclusionCuller::new(&ctx.device, inner, 100, &viewport, &params);

        let instances: Vec<InstanceAttributes> = (0..10)
            .map(|i| {
                InstanceAttributes::from_circle(
                    [(i as f32 - 4.5) * 0.1, 0.0],
                    0.05,
                    [1.0, 1.0, 1.0, 1.0],
                )
            })
            .collect();

        let input_buf = create_instance_buffer(&ctx.device, &instances);
        upload_instances(&ctx.queue, &input_buf, &instances);

        let result = pooled
            .dispatch(&ctx.device, &ctx.queue, &input_buf, 10, &viewport, &params)
            .await
            .unwrap();

        let flags = OcclusionCuller::read_visibility(
            &ctx.device,
            &ctx.queue,
            &result.visibility_buffer,
            10,
        )
        .await
        .unwrap();

        // Sparse — all should be visible.
        let visible_count: u32 = flags.iter().sum();
        assert_eq!(visible_count, 10, "All sparse instances should be visible");
    }

    #[tokio::test]
    async fn test_dense_cluster_culling_rate() {
        let ctx = match GupContext::headless().await {
            Ok(c) => c,
            Err(_) => return,
        };
        let culler = OcclusionCuller::new(&ctx.device).unwrap();

        // 200 circles densely packed at roughly the same position.
        let n = 200u32;
        let instances: Vec<InstanceAttributes> = (0..n)
            .map(|i| {
                let offset = (i as f32 - n as f32 / 2.0) * 0.001;
                InstanceAttributes::from_circle([offset, offset], 0.15, [1.0, 0.0, 0.0, 1.0])
            })
            .collect();

        let input_buf = create_instance_buffer(&ctx.device, &instances);
        upload_instances(&ctx.queue, &input_buf, &instances);

        let viewport = Viewport2D::default();
        let params = OcclusionParams {
            tile_size: 4,
            conservative_margin: 0.0,
        };

        let result = culler
            .dispatch(&ctx.device, &ctx.queue, &input_buf, n, &viewport, &params)
            .await
            .unwrap();

        let flags =
            OcclusionCuller::read_visibility(&ctx.device, &ctx.queue, &result.visibility_buffer, n)
                .await
                .unwrap();

        let visible_count: u32 = flags.iter().sum();
        let culled_count = n - visible_count;
        let cull_rate = culled_count as f32 / n as f32;

        // For 200 heavily overlapping marks we expect significant culling.
        assert!(
            cull_rate >= 0.3,
            "Dense cluster should achieve ≥30% culling rate, got {:.1}% ({culled_count}/{n})",
            cull_rate * 100.0,
        );
    }
}

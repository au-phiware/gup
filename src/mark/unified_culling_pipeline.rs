// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Unified frustum + occlusion culling pipeline.
//!
//! Combines [`ComputeInstanceFilter`] (frustum culling, LOD classification,
//! prefix-sum, compaction) with [`OcclusionCuller`] (Hi-Z coverage,
//! occlusion test) into a single dispatch that shares the visibility buffer
//! and produces a compacted output with `DrawIndirect` parameters.
//!
//! # Pipeline
//!
//! 1. **Cull & classify** — per-instance frustum test and LOD classification
//!    (writes `visibility[i] ∈ {0, 1}`).
//! 2. **Build coverage** — opaque frustum-visible instances populate the
//!    Hi-Z coverage map via `atomicMax`.
//! 3. **Generate Hi-Z** — successive mip levels store the minimum z of
//!    their 2×2 children.
//! 4. **Combined occlusion test** — tests frustum-visible instances against
//!    Hi-Z; occluded instances have their visibility cleared to 0.
//! 5. **Prefix sum + compact** — Blelloch-style scan and scatter of
//!    visible instances into a dense output buffer.
//!
//! This avoids the overhead of two separate dispatches and buffer
//! allocations when both culling strategies are needed.
//!
//! # Example
//!
//! ```rust,ignore
//! let pipeline = UnifiedCullingPipeline::new(&device, 100_000, &viewport, &occlusion_params)?;
//!
//! let result = pipeline.dispatch(
//!     &device, &queue,
//!     &instance_buffer,
//!     instance_count, vertex_count,
//!     &viewport, &lod_thresholds,
//!     &occlusion_params,
//! ).await?;
//!
//! render_pass.set_vertex_buffer(1, result.output_buffer.slice(..));
//! render_pass.draw_indirect(&result.draw_indirect_buffer, 0);
//! ```
//!
//! [`ComputeInstanceFilter`]: super::compute_instance_filter::ComputeInstanceFilter
//! [`OcclusionCuller`]: super::occlusion_culler::OcclusionCuller

use crate::error::{GupError, GupResult};
use std::sync::Arc;
use wgpu::{
    BindGroup, Buffer, BufferDescriptor, BufferUsages, CommandEncoderDescriptor, Device, Queue,
};

use super::batch_renderer::Viewport2D;
use super::compute_instance_filter::{
    ComputeInstanceFilter, FilterResult, MAX_INSTANCES, PooledComputeInstanceFilter,
};
use super::occlusion_culler::{
    OcclusionCuller, OcclusionGpuConfig, OcclusionParams, mip_count, total_hiz_cells,
};

// ---------------------------------------------------------------------------
// Unified Culling Pipeline
// ---------------------------------------------------------------------------

/// Unified frustum + occlusion culling pipeline with pre-allocated buffers.
///
/// Combines both culling strategies in a single GPU dispatch, sharing the
/// visibility buffer and producing a compacted output buffer with
/// `DrawIndirect` parameters.
///
/// Backward compatible: when occlusion is disabled (the default), the
/// pipeline behaves identically to [`PooledComputeInstanceFilter`].
pub struct UnifiedCullingPipeline {
    /// Frustum culling + prefix-sum + compaction.
    filter: PooledComputeInstanceFilter,
    /// Occlusion culling pipelines.
    occlusion: OcclusionCuller,
    /// Pre-allocated Hi-Z buffer.
    hiz_buffer: Arc<Buffer>,
    /// Pre-allocated occlusion config uniform buffer.
    occlusion_config_buffer: Buffer,
    /// Pre-allocated level staging buffer for mip generation.
    level_staging: Buffer,
    /// Current Hi-Z cell capacity.
    hiz_capacity: u32,
    /// Cached occlusion bind group.
    cached_occlusion_bind_group: Option<CachedOcclusionBg>,
}

/// Cached occlusion bind group with input buffer identity.
struct CachedOcclusionBg {
    bind_group: BindGroup,
    input_buffer_id: *const Buffer,
    /// Visibility buffer pointer when the bind group was created.
    visibility_buffer_id: *const Buffer,
}

// SAFETY: input_buffer_id and visibility_buffer_id are only used for
// pointer identity comparison (never dereferenced).
unsafe impl Send for CachedOcclusionBg {}
unsafe impl Sync for CachedOcclusionBg {}

impl UnifiedCullingPipeline {
    /// Create a new unified culling pipeline.
    ///
    /// Pre-allocates buffers for up to `max_instances` instances. The
    /// `viewport` and `occlusion_params` are used to size the Hi-Z buffer.
    pub fn new(
        device: &Device,
        max_instances: u32,
        viewport: &Viewport2D,
        occlusion_params: &OcclusionParams,
    ) -> GupResult<Self> {
        let filter_inner = ComputeInstanceFilter::new(device)?;
        let filter = PooledComputeInstanceFilter::new(device, filter_inner, max_instances);
        let occlusion = OcclusionCuller::new(device)?;

        let base_width = (viewport.pixel_width as u32)
            .div_ceil(occlusion_params.tile_size)
            .max(1);
        let base_height = (viewport.pixel_height as u32)
            .div_ceil(occlusion_params.tile_size)
            .max(1);
        let num_levels = mip_count(base_width, base_height);
        let total_cells = total_hiz_cells(base_width, base_height, num_levels);

        let hiz_buffer = Arc::new(device.create_buffer(&BufferDescriptor {
            label: Some("unified_hiz_buffer"),
            size: (total_cells as u64 * 4).max(4),
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        }));

        let occlusion_config_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("unified_occlusion_config"),
            size: std::mem::size_of::<OcclusionGpuConfig>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let level_staging = device.create_buffer(&BufferDescriptor {
            label: Some("unified_level_staging"),
            size: (num_levels as u64 * 4).max(4),
            usage: BufferUsages::COPY_SRC | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Ok(Self {
            filter,
            occlusion,
            hiz_buffer,
            occlusion_config_buffer,
            level_staging,
            hiz_capacity: total_cells,
            cached_occlusion_bind_group: None,
        })
    }

    /// Current instance buffer capacity.
    pub fn capacity(&self) -> u32 {
        self.filter.capacity()
    }

    /// Access the underlying [`PooledComputeInstanceFilter`].
    pub fn filter(&self) -> &PooledComputeInstanceFilter {
        &self.filter
    }

    /// Run the unified frustum + occlusion culling pipeline.
    ///
    /// A single `dispatch` call applies frustum culling, then occlusion
    /// culling on frustum-visible instances, followed by prefix-sum
    /// compaction into a dense output buffer with `DrawIndirect` parameters.
    ///
    /// When `occlusion_params` is `None`, occlusion culling is skipped and
    /// the pipeline behaves identically to [`PooledComputeInstanceFilter::dispatch`].
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
        occlusion_params: Option<&OcclusionParams>,
    ) -> GupResult<FilterResult> {
        // Delegate to filter-only path when occlusion is disabled.
        if occlusion_params.is_none() {
            return self
                .filter
                .dispatch(
                    device,
                    queue,
                    input_buffer,
                    instance_count,
                    vertex_count,
                    viewport,
                    lod_thresholds,
                )
                .await;
        }

        let params = occlusion_params.unwrap();

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

        // Ensure filter buffers are large enough.
        self.filter.reserve(device, instance_count);

        // Ensure Hi-Z buffer is large enough.
        self.ensure_hiz_capacity(device, viewport, params);

        // Resolve filter bind group (reuses PooledComputeInstanceFilter internals).
        // We need to get access to the filter's internal buffers to create the
        // occlusion bind group that shares the visibility buffer.
        let filter_buffers = self.filter.buffer_refs();

        // Resolve occlusion bind group (shares filter's visibility buffer).
        let input_ptr: *const Buffer = input_buffer;
        let vis_ptr: *const Buffer = filter_buffers.visibility_buffer;
        let occlusion_cache_hit = self.cached_occlusion_bind_group.as_ref().is_some_and(|c| {
            std::ptr::eq(input_ptr, c.input_buffer_id)
                && std::ptr::eq(vis_ptr, c.visibility_buffer_id)
        });

        if !occlusion_cache_hit {
            let bind_group = self.occlusion.create_bind_group(
                device,
                input_buffer,
                &self.hiz_buffer,
                filter_buffers.visibility_buffer,
                &self.occlusion_config_buffer,
            );
            self.cached_occlusion_bind_group = Some(CachedOcclusionBg {
                bind_group,
                input_buffer_id: input_ptr,
                visibility_buffer_id: vis_ptr,
            });
        }

        let occlusion_bind_group = &self
            .cached_occlusion_bind_group
            .as_ref()
            .unwrap()
            .bind_group;

        // --- Encode everything into a single command encoder ---

        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("unified_culling_encoder"),
        });

        // Phase 1: Frustum cull_and_classify (writes visibility[]).
        // Phase 2+3+4: Occlusion passes (modifies visibility[] in-place).
        // Phase 5: Prefix sum + compact (reads visibility[], writes output).
        //
        // We encode the filter's cull pass, then occlusion, then the
        // filter's prefix-sum and compact passes.
        self.filter.encode_frustum_cull(
            device,
            queue,
            &mut encoder,
            input_buffer,
            instance_count,
            vertex_count,
            viewport,
            lod_thresholds,
        );

        self.occlusion.encode_combined(
            queue,
            &mut encoder,
            occlusion_bind_group,
            &self.occlusion_config_buffer,
            &self.level_staging,
            &self.hiz_buffer,
            instance_count,
            viewport,
            params,
        );

        self.filter
            .encode_prefix_sum_and_compact(&mut encoder, instance_count);

        queue.submit([encoder.finish()]);

        Ok(FilterResult {
            output_buffer: self.filter.output_buffer_arc(),
            draw_indirect_buffer: self.filter.draw_indirect_buffer_arc(),
        })
    }

    /// Ensure the Hi-Z buffer is large enough for the given viewport.
    fn ensure_hiz_capacity(
        &mut self,
        device: &Device,
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
        let total_cells = total_hiz_cells(base_width, base_height, num_levels);

        if total_cells > self.hiz_capacity {
            let new_cap = total_cells.next_power_of_two();
            self.hiz_buffer = Arc::new(device.create_buffer(&BufferDescriptor {
                label: Some("unified_hiz_buffer"),
                size: (new_cap as u64 * 4).max(4),
                usage: BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            }));

            self.level_staging = device.create_buffer(&BufferDescriptor {
                label: Some("unified_level_staging"),
                size: (num_levels as u64 * 4).max(4),
                usage: BufferUsages::COPY_SRC | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });

            self.hiz_capacity = new_cap;
            self.cached_occlusion_bind_group = None;
        }
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
    use crate::mark::compute_instance_filter::ComputeInstanceFilter;

    fn create_instance_buffer(device: &Device, instances: &[InstanceAttributes]) -> Buffer {
        let data: &[u8] = bytemuck::cast_slice(instances);
        device.create_buffer(&BufferDescriptor {
            label: Some("test_unified_input"),
            size: data.len() as u64,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    fn upload_instances(queue: &Queue, buffer: &Buffer, instances: &[InstanceAttributes]) {
        queue.write_buffer(buffer, 0, bytemuck::cast_slice(instances));
    }

    #[tokio::test]
    async fn test_unified_pipeline_creation() {
        let ctx = match GupContext::headless().await {
            Ok(c) => c,
            Err(_) => return,
        };
        let viewport = Viewport2D::default();
        let params = OcclusionParams::default();
        let pipeline = UnifiedCullingPipeline::new(&ctx.device, 1000, &viewport, &params);
        assert!(pipeline.is_ok(), "Unified pipeline creation should succeed");
    }

    #[tokio::test]
    async fn test_unified_frustum_only() {
        let ctx = match GupContext::headless().await {
            Ok(c) => c,
            Err(_) => return,
        };
        let viewport = Viewport2D::default();
        let params = OcclusionParams::default();
        let mut pipeline =
            UnifiedCullingPipeline::new(&ctx.device, 100, &viewport, &params).unwrap();

        // 4 circles: 2 inside viewport, 2 outside.
        let instances = vec![
            InstanceAttributes::from_circle([0.0, 0.0], 0.1, [1.0, 0.0, 0.0, 1.0]),
            InstanceAttributes::from_circle([5.0, 5.0], 0.05, [0.0, 1.0, 0.0, 1.0]),
            InstanceAttributes::from_circle([0.5, 0.5], 0.1, [0.0, 0.0, 1.0, 1.0]),
            InstanceAttributes::from_circle([-5.0, -5.0], 0.05, [1.0, 1.0, 0.0, 1.0]),
        ];

        let input_buf = create_instance_buffer(&ctx.device, &instances);
        upload_instances(&ctx.queue, &input_buf, &instances);

        let thresholds = [4.0, 1.0, 0.25];

        // No occlusion — should behave like plain filter.
        let result = pipeline
            .dispatch(
                &ctx.device,
                &ctx.queue,
                &input_buf,
                4,
                6,
                &viewport,
                &thresholds,
                None,
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
            args[1], 2,
            "Only 2 instances should be visible (frustum only)"
        );
    }

    #[tokio::test]
    async fn test_unified_with_occlusion() {
        let ctx = match GupContext::headless().await {
            Ok(c) => c,
            Err(_) => return,
        };
        let viewport = Viewport2D::default();
        let params = OcclusionParams {
            tile_size: 4,
            conservative_margin: 0.0,
        };
        let mut pipeline =
            UnifiedCullingPipeline::new(&ctx.device, 1000, &viewport, &params).unwrap();

        // 50 stacked opaque circles at the same position.
        let n = 50u32;
        let instances: Vec<InstanceAttributes> = (0..n)
            .map(|_| InstanceAttributes::from_circle([0.0, 0.0], 0.2, [1.0, 0.0, 0.0, 1.0]))
            .collect();

        let input_buf = create_instance_buffer(&ctx.device, &instances);
        upload_instances(&ctx.queue, &input_buf, &instances);

        let thresholds = [4.0, 1.0, 0.25];

        let result = pipeline
            .dispatch(
                &ctx.device,
                &ctx.queue,
                &input_buf,
                n,
                6,
                &viewport,
                &thresholds,
                Some(&params),
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

        let visible = args[1];
        let culled = n - visible;

        // The last instance (top) should always be visible.
        assert!(visible >= 1, "At least the top instance should be visible");

        // Some earlier instances should be culled by occlusion.
        assert!(
            culled > 0,
            "Stacked instances should have some occlusion culling: visible={visible}, total={n}"
        );
    }

    #[tokio::test]
    async fn test_unified_sparse_no_occlusion_culling() {
        let ctx = match GupContext::headless().await {
            Ok(c) => c,
            Err(_) => return,
        };
        let viewport = Viewport2D::default();
        let params = OcclusionParams::default();
        let mut pipeline =
            UnifiedCullingPipeline::new(&ctx.device, 100, &viewport, &params).unwrap();

        // 4 non-overlapping circles within the viewport.
        let instances = vec![
            InstanceAttributes::from_circle([-0.5, -0.5], 0.1, [1.0, 0.0, 0.0, 1.0]),
            InstanceAttributes::from_circle([0.5, -0.5], 0.1, [0.0, 1.0, 0.0, 1.0]),
            InstanceAttributes::from_circle([-0.5, 0.5], 0.1, [0.0, 0.0, 1.0, 1.0]),
            InstanceAttributes::from_circle([0.5, 0.5], 0.1, [1.0, 1.0, 0.0, 1.0]),
        ];

        let input_buf = create_instance_buffer(&ctx.device, &instances);
        upload_instances(&ctx.queue, &input_buf, &instances);

        let thresholds = [4.0, 1.0, 0.25];

        let result = pipeline
            .dispatch(
                &ctx.device,
                &ctx.queue,
                &input_buf,
                4,
                6,
                &viewport,
                &thresholds,
                Some(&params),
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
            args[1], 4,
            "All 4 sparse instances should be visible (no occlusion)"
        );
    }

    #[tokio::test]
    async fn test_unified_mixed_frustum_and_occlusion() {
        let ctx = match GupContext::headless().await {
            Ok(c) => c,
            Err(_) => return,
        };
        let viewport = Viewport2D::default();
        let params = OcclusionParams {
            tile_size: 4,
            conservative_margin: 0.0,
        };
        let mut pipeline =
            UnifiedCullingPipeline::new(&ctx.device, 1000, &viewport, &params).unwrap();

        // Mix: some outside viewport (frustum-culled), some stacked (occlusion-culled),
        // some visible.
        let mut instances = Vec::new();
        // 2 outside viewport
        instances.push(InstanceAttributes::from_circle(
            [5.0, 5.0],
            0.1,
            [1.0, 0.0, 0.0, 1.0],
        ));
        instances.push(InstanceAttributes::from_circle(
            [-5.0, -5.0],
            0.1,
            [0.0, 1.0, 0.0, 1.0],
        ));
        // 20 stacked at center (some will be occluded)
        for _ in 0..20 {
            instances.push(InstanceAttributes::from_circle(
                [0.0, 0.0],
                0.2,
                [0.0, 0.0, 1.0, 1.0],
            ));
        }
        // 2 isolated visible
        instances.push(InstanceAttributes::from_circle(
            [-0.7, 0.7],
            0.1,
            [1.0, 1.0, 0.0, 1.0],
        ));
        instances.push(InstanceAttributes::from_circle(
            [0.7, -0.7],
            0.1,
            [1.0, 0.0, 1.0, 1.0],
        ));

        let n = instances.len() as u32;
        let input_buf = create_instance_buffer(&ctx.device, &instances);
        upload_instances(&ctx.queue, &input_buf, &instances);

        let thresholds = [4.0, 1.0, 0.25];

        let result = pipeline
            .dispatch(
                &ctx.device,
                &ctx.queue,
                &input_buf,
                n,
                6,
                &viewport,
                &thresholds,
                Some(&params),
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

        let visible = args[1];
        // 2 frustum-culled, some of 20 stacked should be occlusion-culled, 2 isolated visible.
        // At minimum: 2 isolated + 1 top of stack = 3; at most: 2 isolated + 20 stacked = 22.
        assert!(
            (3..=22).contains(&visible),
            "Expected 3..=22 visible instances, got {visible}"
        );
        // Should be fewer than all 22 in-viewport instances.
        assert!(
            visible < 22,
            "Occlusion should cull some stacked instances: visible={visible}"
        );
    }

    #[tokio::test]
    async fn test_unified_matches_separate_pipelines() {
        let ctx = match GupContext::headless().await {
            Ok(c) => c,
            Err(_) => return,
        };
        let viewport = Viewport2D::default();
        let params = OcclusionParams {
            tile_size: 4,
            conservative_margin: 0.0,
        };

        // Create unified pipeline.
        let mut unified =
            UnifiedCullingPipeline::new(&ctx.device, 1000, &viewport, &params).unwrap();

        // Create separate filter for reference.
        let filter = ComputeInstanceFilter::new(&ctx.device).unwrap();

        // All instances inside viewport (no frustum culling needed).
        let instances: Vec<InstanceAttributes> = (0..20)
            .map(|_| InstanceAttributes::from_circle([0.0, 0.0], 0.2, [1.0, 0.0, 0.0, 1.0]))
            .collect();
        let n = instances.len() as u32;

        let input_buf = create_instance_buffer(&ctx.device, &instances);
        upload_instances(&ctx.queue, &input_buf, &instances);

        let thresholds = [4.0, 1.0, 0.25];

        // Separate: filter only (no occlusion).
        let filter_result = filter
            .dispatch(
                &ctx.device,
                &ctx.queue,
                &input_buf,
                n,
                6,
                &viewport,
                &thresholds,
            )
            .await
            .unwrap();

        let filter_args = ComputeInstanceFilter::read_draw_indirect(
            &ctx.device,
            &ctx.queue,
            &filter_result.draw_indirect_buffer,
        )
        .await
        .unwrap();

        // Unified with occlusion.
        let unified_result = unified
            .dispatch(
                &ctx.device,
                &ctx.queue,
                &input_buf,
                n,
                6,
                &viewport,
                &thresholds,
                Some(&params),
            )
            .await
            .unwrap();

        let unified_args = ComputeInstanceFilter::read_draw_indirect(
            &ctx.device,
            &ctx.queue,
            &unified_result.draw_indirect_buffer,
        )
        .await
        .unwrap();

        // The unified pipeline should have <= visible instances than filter-only
        // (since occlusion culling removes additional instances).
        assert!(
            unified_args[1] <= filter_args[1],
            "Unified should cull >= filter-only: unified={}, filter={}",
            unified_args[1],
            filter_args[1]
        );

        // Both should have same vertex_count.
        assert_eq!(unified_args[0], 6, "vertex_count should be 6");
        assert_eq!(filter_args[0], 6, "vertex_count should be 6");
    }

    #[tokio::test]
    async fn test_unified_zero_instances_error() {
        let ctx = match GupContext::headless().await {
            Ok(c) => c,
            Err(_) => return,
        };
        let viewport = Viewport2D::default();
        let params = OcclusionParams::default();
        let mut pipeline =
            UnifiedCullingPipeline::new(&ctx.device, 100, &viewport, &params).unwrap();

        let input_buf = ctx.device.create_buffer(&BufferDescriptor {
            label: Some("empty"),
            size: 96,
            usage: BufferUsages::STORAGE,
            mapped_at_creation: false,
        });

        let thresholds = [4.0, 1.0, 0.25];

        let result = pipeline
            .dispatch(
                &ctx.device,
                &ctx.queue,
                &input_buf,
                0,
                6,
                &viewport,
                &thresholds,
                Some(&params),
            )
            .await;

        assert!(result.is_err(), "Zero instances should return an error");
    }
}

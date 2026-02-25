// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Instanced batch rendering for marks with performance optimizations.
//!
//! This module provides the [`InstancedBatchRenderer`] that groups marks by pipeline
//! for efficient GPU instanced rendering, reducing state changes and draw calls.
//! It also provides [`CullingManager`] for viewport-based frustum culling and
//! level-of-detail (LOD) selection.
//!
//! # Performance Features
//!
//! - **Batch grouping**: Marks sharing the same pipeline are rendered together
//! - **Buffer reuse**: Instance buffers are pooled and reused across frames
//! - **Frustum culling**: Off-screen instances are skipped before GPU upload
//! - **LOD selection**: Distant/small marks use simplified geometry
//! - **Statistics tracking**: Draw calls, instances, and cache metrics per frame

use crate::buffer::{BufferType, GpuBuffer};
use crate::error::{GupError, GupResult};
use crate::mark::{Mark, MarkInfo, MarkInfoImpl};
use std::any::TypeId;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use wgpu::{Device, Queue, RenderPass, RenderPipeline};

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for the instanced batch renderer.
#[derive(Debug, Clone)]
pub struct BatchRendererConfig {
    /// Maximum number of instances per draw call (default: 10_000).
    pub max_instances_per_draw: u32,
    /// Initial instance buffer capacity in elements (default: 1024).
    pub initial_instance_capacity: usize,
    /// Enable frustum culling (default: true).
    pub enable_culling: bool,
    /// Enable level-of-detail selection (default: true).
    pub enable_lod: bool,
    /// Screen-space pixel thresholds for LOD transitions.
    /// `[full → simplified, simplified → point, point → culled]`
    pub lod_thresholds: [f32; 3],
}

impl Default for BatchRendererConfig {
    fn default() -> Self {
        Self {
            max_instances_per_draw: 10_000,
            initial_instance_capacity: 1024,
            enable_culling: true,
            enable_lod: true,
            lod_thresholds: [4.0, 1.0, 0.25],
        }
    }
}

// ---------------------------------------------------------------------------
// Level of Detail
// ---------------------------------------------------------------------------

/// Level of detail for mark rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LodLevel {
    /// Full geometry with all details.
    Full,
    /// Simplified geometry (e.g., fewer triangles).
    Simplified,
    /// Single pixel / point sprite.
    Point,
    /// Culled — not rendered.
    Culled,
}

// ---------------------------------------------------------------------------
// Viewport frustum
// ---------------------------------------------------------------------------

/// Axis-aligned viewport rectangle used for frustum culling in 2D.
#[derive(Debug, Clone, Copy)]
pub struct Viewport2D {
    /// Minimum x in clip-space (typically -1.0).
    pub min_x: f32,
    /// Maximum x in clip-space (typically 1.0).
    pub max_x: f32,
    /// Minimum y in clip-space (typically -1.0).
    pub min_y: f32,
    /// Maximum y in clip-space (typically 1.0).
    pub max_y: f32,
    /// Viewport width in physical pixels (for LOD).
    pub pixel_width: f32,
    /// Viewport height in physical pixels (for LOD).
    pub pixel_height: f32,
}

impl Default for Viewport2D {
    fn default() -> Self {
        Self {
            min_x: -1.0,
            max_x: 1.0,
            min_y: -1.0,
            max_y: 1.0,
            pixel_width: 800.0,
            pixel_height: 600.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Culling manager
// ---------------------------------------------------------------------------

/// Manages frustum culling and LOD selection for 2D marks.
#[derive(Debug, Clone)]
pub struct CullingManager {
    viewport: Viewport2D,
    config: BatchRendererConfig,
}

impl CullingManager {
    /// Create a new culling manager.
    pub fn new(config: &BatchRendererConfig) -> Self {
        Self {
            viewport: Viewport2D::default(),
            config: config.clone(),
        }
    }

    /// Update the viewport used for culling.
    pub fn set_viewport(&mut self, viewport: Viewport2D) {
        self.viewport = viewport;
    }

    /// Get current viewport.
    pub fn viewport(&self) -> &Viewport2D {
        &self.viewport
    }

    /// Test whether a circle (center + radius) is visible in the viewport.
    ///
    /// Works for any mark that can be bounded by a circle. Returns `true` if
    /// the bounding circle intersects the viewport rectangle.
    pub fn is_visible(&self, cx: f32, cy: f32, radius: f32) -> bool {
        // Expand viewport by radius to account for partial overlap
        cx + radius >= self.viewport.min_x
            && cx - radius <= self.viewport.max_x
            && cy + radius >= self.viewport.min_y
            && cy - radius <= self.viewport.max_y
    }

    /// Compute LOD level for a mark based on its screen-space size.
    ///
    /// `clip_radius` is the radius of the bounding circle in clip-space units.
    pub fn compute_lod(&self, clip_radius: f32) -> LodLevel {
        if !self.config.enable_lod {
            return LodLevel::Full;
        }

        // Convert clip-space radius to approximate pixel size.
        // Clip space goes from -1..1, so 2.0 corresponds to full viewport width.
        let pixel_size = clip_radius * self.viewport.pixel_width / 2.0;

        if pixel_size >= self.config.lod_thresholds[0] {
            LodLevel::Full
        } else if pixel_size >= self.config.lod_thresholds[1] {
            LodLevel::Simplified
        } else if pixel_size >= self.config.lod_thresholds[2] {
            LodLevel::Point
        } else {
            LodLevel::Culled
        }
    }

    /// Filter and classify a set of circular instances.
    ///
    /// Returns the indices of visible instances grouped by LOD level.
    /// Each entry in the returned `HashMap` maps an `LodLevel` to the indices
    /// of instances at that level. Culled instances are omitted.
    pub fn classify_circles(
        &self,
        centers: &[[f32; 2]],
        radii: &[f32],
    ) -> HashMap<LodLevel, Vec<usize>> {
        let mut result: HashMap<LodLevel, Vec<usize>> = HashMap::new();
        for (i, (center, &radius)) in centers.iter().zip(radii.iter()).enumerate() {
            if !self.config.enable_culling || self.is_visible(center[0], center[1], radius) {
                let lod = self.compute_lod(radius);
                if lod != LodLevel::Culled {
                    result.entry(lod).or_default().push(i);
                }
            }
        }
        result
    }
}

// ---------------------------------------------------------------------------
// Render batch
// ---------------------------------------------------------------------------

/// A single draw call batch: a contiguous range of instances sharing a pipeline.
#[derive(Debug, Clone)]
pub struct RenderBatch {
    /// Type of mark rendered by this batch.
    pub mark_type_id: TypeId,
    /// Human-readable mark type name (for debugging).
    pub mark_type_name: &'static str,
    /// Number of instances in this batch.
    pub instance_count: u32,
    /// LOD level used for this batch.
    pub lod_level: LodLevel,
    /// Z-order for depth sorting.
    pub z_order: f32,
}

// ---------------------------------------------------------------------------
// Per-frame statistics
// ---------------------------------------------------------------------------

/// Statistics collected during a single frame of batch rendering.
#[derive(Debug, Clone, Default)]
pub struct BatchFrameStats {
    /// Total number of draw calls issued.
    pub draw_calls: u32,
    /// Total number of instances rendered across all draw calls.
    pub total_instances: u32,
    /// Number of instances culled by frustum culling.
    pub culled_instances: u32,
    /// Number of pipeline state changes.
    pub pipeline_switches: u32,
    /// Pipeline cache hit count.
    pub cache_hits: u32,
    /// Pipeline cache miss count.
    pub cache_misses: u32,
    /// CPU time spent preparing batches (microseconds).
    pub batch_prepare_us: u64,
    /// CPU time spent uploading buffers (microseconds).
    pub buffer_upload_us: u64,
}

// ---------------------------------------------------------------------------
// Instanced batch renderer
// ---------------------------------------------------------------------------

/// High-performance batch renderer using GPU instancing.
///
/// Groups mark instances by pipeline to minimise state changes, manages
/// instance buffer allocation via a buffer pool, and provides optional
/// frustum culling and LOD selection.
///
/// # Usage
///
/// ```rust,ignore
/// let mut renderer = InstancedBatchRenderer::new(&device, Default::default());
///
/// // Each frame:
/// renderer.begin_frame();
/// renderer.submit_instances::<Circle>(&device, &queue, &circle_instances)?;
/// renderer.submit_instances::<Rectangle>(&device, &queue, &rect_instances)?;
/// let stats = renderer.end_frame();
/// ```
pub struct InstancedBatchRenderer {
    config: BatchRendererConfig,
    culling: CullingManager,

    /// Cached vertex buffers per mark type (uploaded once and reused).
    vertex_buffers: HashMap<TypeId, GpuBuffer<u8>>,
    /// Cached index buffers per mark type (uploaded once and reused).
    index_buffers: HashMap<TypeId, GpuBuffer<u32>>,

    /// Instance buffers allocated for the current frame (by mark type).
    instance_buffers: HashMap<TypeId, GpuBuffer<u8>>,

    /// Pipeline cache (mark TypeId → RenderPipeline).
    pipeline_cache: HashMap<TypeId, Arc<RenderPipeline>>,

    /// Batches queued for the current frame.
    batches: Vec<RenderBatch>,

    /// Stats for the most recent completed frame.
    last_frame_stats: BatchFrameStats,
    /// Stats being accumulated for the current frame.
    current_stats: BatchFrameStats,
    /// Timestamp when the current frame began.
    frame_start: Option<Instant>,
}

impl InstancedBatchRenderer {
    /// Create a new instanced batch renderer.
    pub fn new(config: BatchRendererConfig) -> Self {
        let culling = CullingManager::new(&config);
        Self {
            config,
            culling,
            vertex_buffers: HashMap::new(),
            index_buffers: HashMap::new(),
            instance_buffers: HashMap::new(),
            pipeline_cache: HashMap::new(),
            batches: Vec::new(),
            last_frame_stats: BatchFrameStats::default(),
            current_stats: BatchFrameStats::default(),
            frame_start: None,
        }
    }

    /// Access the culling manager to update the viewport or change LOD thresholds.
    pub fn culling_mut(&mut self) -> &mut CullingManager {
        &mut self.culling
    }

    /// Read-only access to culling manager.
    pub fn culling(&self) -> &CullingManager {
        &self.culling
    }

    /// Access the renderer config.
    pub fn config(&self) -> &BatchRendererConfig {
        &self.config
    }

    // ------------------------------------------------------------------
    // Frame lifecycle
    // ------------------------------------------------------------------

    /// Begin a new frame — clears queued batches and resets stats.
    pub fn begin_frame(&mut self) {
        self.batches.clear();
        self.current_stats = BatchFrameStats::default();
        self.frame_start = Some(Instant::now());
    }

    /// End the current frame — finalise stats.
    pub fn end_frame(&mut self) -> BatchFrameStats {
        if let Some(start) = self.frame_start.take() {
            self.current_stats.batch_prepare_us = start.elapsed().as_micros() as u64;
        }
        self.last_frame_stats = self.current_stats.clone();
        self.last_frame_stats.clone()
    }

    /// Get the stats from the most recently completed frame.
    pub fn last_frame_stats(&self) -> &BatchFrameStats {
        &self.last_frame_stats
    }

    // ------------------------------------------------------------------
    // Vertex / index buffer caching
    // ------------------------------------------------------------------

    /// Ensure that vertex (and optionally index) buffers for mark type `M`
    /// have been uploaded. Uploads only happen once per mark type.
    pub fn ensure_geometry<M: Mark>(&mut self, device: &Device, queue: &Queue) -> GupResult<()> {
        let type_id = TypeId::of::<M>();
        if self.vertex_buffers.contains_key(&type_id) {
            return Ok(());
        }

        // Upload vertex data
        let vertices = M::generate_vertices();
        let vertex_bytes: &[u8] = bytemuck::cast_slice(&vertices);
        let mut vb = GpuBuffer::<u8>::new(device, BufferType::Vertex, vertex_bytes.len());
        vb.upload(device, queue, vertex_bytes)?;
        self.vertex_buffers.insert(type_id, vb);

        // Upload index data if present
        if let Some(indices) = M::generate_indices() {
            let mut ib = GpuBuffer::<u32>::new(device, BufferType::Storage, indices.len());
            ib.upload(device, queue, &indices)?;
            self.index_buffers.insert(type_id, ib);
        }

        Ok(())
    }

    // ------------------------------------------------------------------
    // Pipeline management
    // ------------------------------------------------------------------

    /// Get or create a render pipeline for mark type `M`.
    pub fn get_or_create_pipeline<M: Mark>(
        &mut self,
        device: &Device,
    ) -> GupResult<Arc<RenderPipeline>> {
        let type_id = TypeId::of::<M>();
        if let Some(pipeline) = self.pipeline_cache.get(&type_id) {
            self.current_stats.cache_hits += 1;
            return Ok(Arc::clone(pipeline));
        }

        self.current_stats.cache_misses += 1;

        let mark_info: Box<dyn MarkInfo> = Box::new(MarkInfoImpl::<M>::new());
        let pipeline = mark_info.create_render_pipeline(device)?;
        let arc = Arc::new(pipeline);
        self.pipeline_cache.insert(type_id, Arc::clone(&arc));
        Ok(arc)
    }

    /// Clear all cached pipelines (e.g., on device lost).
    pub fn clear_pipeline_cache(&mut self) {
        self.pipeline_cache.clear();
    }

    /// Number of cached pipelines.
    pub fn pipeline_cache_size(&self) -> usize {
        self.pipeline_cache.len()
    }

    // ------------------------------------------------------------------
    // Instance submission
    // ------------------------------------------------------------------

    /// Upload instance data for mark type `M` and queue a batch.
    ///
    /// The instance data is uploaded to a GPU buffer and a [`RenderBatch`]
    /// is queued for rendering. Instances are split into sub-batches
    /// of at most `max_instances_per_draw` to respect GPU limits.
    pub fn submit_instances<M, I>(
        &mut self,
        device: &Device,
        queue: &Queue,
        instances: &[I],
    ) -> GupResult<()>
    where
        M: Mark,
        I: bytemuck::Pod + bytemuck::Zeroable,
    {
        if instances.is_empty() {
            return Ok(());
        }

        let upload_start = Instant::now();
        let type_id = TypeId::of::<M>();

        let instance_bytes: &[u8] = bytemuck::cast_slice(instances);

        // Allocate or reuse instance buffer
        let ib = self.instance_buffers.entry(type_id).or_insert_with(|| {
            let cap = std::cmp::max(
                self.config.initial_instance_capacity * std::mem::size_of::<I>(),
                instance_bytes.len(),
            );
            GpuBuffer::<u8>::new(device, BufferType::Instance, cap)
        });

        ib.upload(device, queue, instance_bytes)?;

        self.current_stats.buffer_upload_us += upload_start.elapsed().as_micros() as u64;

        // Queue batches (split if exceeding per-draw limit)
        let total = instances.len() as u32;
        let max_per_draw = self.config.max_instances_per_draw;
        let mut remaining = total;
        while remaining > 0 {
            let count = remaining.min(max_per_draw);
            self.batches.push(RenderBatch {
                mark_type_id: type_id,
                mark_type_name: std::any::type_name::<M>(),
                instance_count: count,
                lod_level: LodLevel::Full,
                z_order: 0.0,
            });
            remaining -= count;
        }

        Ok(())
    }

    /// Submit instances with frustum culling applied.
    ///
    /// `centers` and `radii` describe the bounding circles for each instance.
    /// Only visible instances are uploaded.
    ///
    /// Returns the number of instances actually submitted (after culling).
    pub fn submit_with_culling<M, I>(
        &mut self,
        device: &Device,
        queue: &Queue,
        instances: &[I],
        centers: &[[f32; 2]],
        radii: &[f32],
    ) -> GupResult<u32>
    where
        M: Mark,
        I: bytemuck::Pod + bytemuck::Zeroable + Copy,
    {
        assert_eq!(instances.len(), centers.len());
        assert_eq!(instances.len(), radii.len());

        let classified = self.culling.classify_circles(centers, radii);

        let mut visible: Vec<I> = Vec::new();
        let total = instances.len() as u32;

        for (&_lod, indices) in &classified {
            for &idx in indices {
                visible.push(instances[idx]);
            }
        }

        let submitted = visible.len() as u32;
        self.current_stats.culled_instances += total - submitted;

        if !visible.is_empty() {
            self.submit_instances::<M, I>(device, queue, &visible)?;
        }

        Ok(submitted)
    }

    // ------------------------------------------------------------------
    // Rendering
    // ------------------------------------------------------------------

    /// Render all queued batches into the given render pass.
    ///
    /// Requires that pipelines, geometry, and bind groups have been set up.
    /// This is the low-level draw method; callers must set bind groups
    /// themselves.
    pub fn render_batches<M: Mark>(
        &mut self,
        render_pass: &mut RenderPass,
        pipeline: &RenderPipeline,
        bind_group: &wgpu::BindGroup,
    ) -> GupResult<()> {
        let type_id = TypeId::of::<M>();

        let vb = self.vertex_buffers.get(&type_id).ok_or_else(|| {
            GupError::render_error(format!(
                "No vertex buffer for mark type {}",
                std::any::type_name::<M>()
            ))
        })?;

        let ib_opt = self.instance_buffers.get(&type_id);
        let _ib = ib_opt.ok_or_else(|| {
            GupError::render_error(format!(
                "No instance buffer for mark type {}",
                std::any::type_name::<M>()
            ))
        })?;

        render_pass.set_pipeline(pipeline);
        self.current_stats.pipeline_switches += 1;

        render_pass.set_bind_group(0, bind_group, &[]);
        render_pass.set_vertex_buffer(0, vb.buffer().slice(..));

        // Set instance buffer at slot 1 if using vertex-step instance data
        // (the current architecture uses storage buffers in bind groups instead)

        let index_buffer = self.index_buffers.get(&type_id);

        for batch in &self.batches {
            if batch.mark_type_id != type_id {
                continue;
            }

            if let Some(idx_count) = M::index_count() {
                if let Some(idx_buf) = index_buffer {
                    render_pass
                        .set_index_buffer(idx_buf.buffer().slice(..), wgpu::IndexFormat::Uint32);
                    render_pass.draw_indexed(0..idx_count as u32, 0, 0..batch.instance_count);
                } else {
                    return Err(GupError::render_error(
                        "Mark requires indexed rendering but no index buffer available".to_string(),
                    ));
                }
            } else {
                render_pass.draw(0..M::vertex_count() as u32, 0..batch.instance_count);
            }

            self.current_stats.draw_calls += 1;
            self.current_stats.total_instances += batch.instance_count;
        }

        Ok(())
    }

    // ------------------------------------------------------------------
    // Queries
    // ------------------------------------------------------------------

    /// Number of batches currently queued.
    pub fn queued_batch_count(&self) -> usize {
        self.batches.len()
    }

    /// Iterate over queued batches.
    pub fn queued_batches(&self) -> &[RenderBatch] {
        &self.batches
    }

    /// Check whether geometry for a mark type has been uploaded.
    pub fn has_geometry<M: Mark>(&self) -> bool {
        self.vertex_buffers.contains_key(&TypeId::of::<M>())
    }

    /// Get the raw instance buffer for a mark type (for bind group creation).
    pub fn instance_buffer<M: Mark>(&self) -> Option<&GpuBuffer<u8>> {
        self.instance_buffers.get(&TypeId::of::<M>())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mark::Circle;
    use crate::mark::circle::{CircleAttributes, CircleInstance};

    // ------------------------------------------------------------------
    // CullingManager unit tests (no GPU required)
    // ------------------------------------------------------------------

    #[test]
    fn test_default_viewport_covers_clip_space() {
        let vp = Viewport2D::default();
        assert_eq!(vp.min_x, -1.0);
        assert_eq!(vp.max_x, 1.0);
        assert_eq!(vp.min_y, -1.0);
        assert_eq!(vp.max_y, 1.0);
    }

    #[test]
    fn test_culling_visible_center() {
        let cfg = BatchRendererConfig::default();
        let cm = CullingManager::new(&cfg);
        // A circle at the origin is fully inside the clip rect
        assert!(cm.is_visible(0.0, 0.0, 0.1));
    }

    #[test]
    fn test_culling_partially_visible() {
        let cfg = BatchRendererConfig::default();
        let cm = CullingManager::new(&cfg);
        // Circle centered at x=1.05, radius 0.1 => overlaps right edge
        assert!(cm.is_visible(1.05, 0.0, 0.1));
    }

    #[test]
    fn test_culling_fully_outside() {
        let cfg = BatchRendererConfig::default();
        let cm = CullingManager::new(&cfg);
        // Circle far outside
        assert!(!cm.is_visible(5.0, 0.0, 0.1));
        assert!(!cm.is_visible(0.0, -5.0, 0.1));
    }

    #[test]
    fn test_lod_full_for_large_marks() {
        let cfg = BatchRendererConfig::default();
        let cm = CullingManager::new(&cfg);
        // Radius 0.1 in clip space → 0.1 * 800/2 = 40 pixels → Full
        assert_eq!(cm.compute_lod(0.1), LodLevel::Full);
    }

    #[test]
    fn test_lod_simplified_for_small_marks() {
        let cfg = BatchRendererConfig::default();
        let cm = CullingManager::new(&cfg);
        // Radius 0.005 → 0.005 * 400 = 2 pixels → Simplified
        assert_eq!(cm.compute_lod(0.005), LodLevel::Simplified);
    }

    #[test]
    fn test_lod_point_for_tiny_marks() {
        let cfg = BatchRendererConfig::default();
        let cm = CullingManager::new(&cfg);
        // Radius 0.002 → 0.002 * 400 = 0.8 pixels → Point
        assert_eq!(cm.compute_lod(0.002), LodLevel::Point);
    }

    #[test]
    fn test_lod_culled_for_subpixel() {
        let cfg = BatchRendererConfig::default();
        let cm = CullingManager::new(&cfg);
        // Radius 0.0001 → 0.04 pixels → Culled
        assert_eq!(cm.compute_lod(0.0001), LodLevel::Culled);
    }

    #[test]
    fn test_lod_disabled_always_full() {
        let cfg = BatchRendererConfig {
            enable_lod: false,
            ..Default::default()
        };
        let cm = CullingManager::new(&cfg);
        assert_eq!(cm.compute_lod(0.0001), LodLevel::Full);
    }

    #[test]
    fn test_classify_circles_mixed() {
        let cfg = BatchRendererConfig::default();
        let cm = CullingManager::new(&cfg);

        let centers = [[0.0f32, 0.0], [0.0, 0.0], [5.0, 5.0]]; // last one is off-screen
        let radii = [0.1f32, 0.001, 0.1]; // full, point, off-screen

        let classified = cm.classify_circles(&centers, &radii);

        // First instance → Full
        assert!(classified.get(&LodLevel::Full).unwrap().contains(&0));
        // Second instance → Point (0.001 * 400 = 0.4)
        assert!(classified.get(&LodLevel::Point).unwrap().contains(&1));
        // Third instance → culled by frustum
        let total_visible: usize = classified.values().map(|v| v.len()).sum();
        assert_eq!(total_visible, 2);
    }

    #[test]
    fn test_classify_circles_culling_disabled() {
        let cfg = BatchRendererConfig {
            enable_culling: false,
            enable_lod: false,
            ..Default::default()
        };
        let cm = CullingManager::new(&cfg);

        let centers = [[5.0f32, 5.0]]; // off-screen
        let radii = [0.1f32];

        let classified = cm.classify_circles(&centers, &radii);
        // Should still be present (culling disabled, LOD disabled → Full)
        assert!(classified.get(&LodLevel::Full).unwrap().contains(&0));
    }

    #[test]
    fn test_batch_renderer_config_defaults() {
        let cfg = BatchRendererConfig::default();
        assert_eq!(cfg.max_instances_per_draw, 10_000);
        assert_eq!(cfg.initial_instance_capacity, 1024);
        assert!(cfg.enable_culling);
        assert!(cfg.enable_lod);
    }

    #[test]
    fn test_render_batch_fields() {
        let batch = RenderBatch {
            mark_type_id: TypeId::of::<u32>(),
            mark_type_name: "test",
            instance_count: 42,
            lod_level: LodLevel::Simplified,
            z_order: 1.5,
        };
        assert_eq!(batch.instance_count, 42);
        assert_eq!(batch.lod_level, LodLevel::Simplified);
        assert_eq!(batch.z_order, 1.5);
    }

    #[test]
    fn test_frame_stats_default() {
        let stats = BatchFrameStats::default();
        assert_eq!(stats.draw_calls, 0);
        assert_eq!(stats.total_instances, 0);
        assert_eq!(stats.culled_instances, 0);
    }

    // ------------------------------------------------------------------
    // GPU integration tests (require headless context)
    // ------------------------------------------------------------------

    async fn create_test_context() -> GupResult<std::sync::Arc<crate::context::GupContext>> {
        crate::context::GupContext::headless().await
    }

    #[tokio::test]
    async fn test_batch_renderer_creation() {
        let renderer = InstancedBatchRenderer::new(BatchRendererConfig::default());
        assert_eq!(renderer.queued_batch_count(), 0);
        assert_eq!(renderer.pipeline_cache_size(), 0);
    }

    #[tokio::test]
    async fn test_ensure_geometry_uploads_once() -> GupResult<()> {
        let ctx = create_test_context().await?;
        let device = &ctx.device;
        let queue = &ctx.queue;

        let mut renderer = InstancedBatchRenderer::new(BatchRendererConfig::default());

        assert!(!renderer.has_geometry::<Circle>());
        renderer.ensure_geometry::<Circle>(device, queue)?;
        assert!(renderer.has_geometry::<Circle>());

        // Second call is a no-op
        renderer.ensure_geometry::<Circle>(device, queue)?;
        Ok(())
    }

    #[tokio::test]
    async fn test_submit_instances_queues_batch() -> GupResult<()> {
        let ctx = create_test_context().await?;
        let device = &ctx.device;
        let queue = &ctx.queue;

        let mut renderer = InstancedBatchRenderer::new(BatchRendererConfig::default());
        renderer.begin_frame();

        let instances = vec![CircleInstance::from(&CircleAttributes::default()); 100];

        renderer.submit_instances::<Circle, _>(device, queue, &instances)?;

        assert_eq!(renderer.queued_batch_count(), 1);
        assert_eq!(renderer.queued_batches()[0].instance_count, 100);

        let _stats = renderer.end_frame();
        Ok(())
    }

    #[tokio::test]
    async fn test_submit_large_batch_splits() -> GupResult<()> {
        let ctx = create_test_context().await?;
        let device = &ctx.device;
        let queue = &ctx.queue;

        let config = BatchRendererConfig {
            max_instances_per_draw: 50,
            ..Default::default()
        };
        let mut renderer = InstancedBatchRenderer::new(config);
        renderer.begin_frame();

        let instances = vec![CircleInstance::from(&CircleAttributes::default()); 120];

        renderer.submit_instances::<Circle, _>(device, queue, &instances)?;

        // 120 / 50 = 3 batches (50, 50, 20)
        assert_eq!(renderer.queued_batch_count(), 3);
        assert_eq!(renderer.queued_batches()[0].instance_count, 50);
        assert_eq!(renderer.queued_batches()[1].instance_count, 50);
        assert_eq!(renderer.queued_batches()[2].instance_count, 20);

        renderer.end_frame();
        Ok(())
    }

    #[tokio::test]
    async fn test_submit_with_culling_filters_offscreen() -> GupResult<()> {
        let ctx = create_test_context().await?;
        let device = &ctx.device;
        let queue = &ctx.queue;

        let mut renderer = InstancedBatchRenderer::new(BatchRendererConfig::default());
        renderer.begin_frame();

        let instances = vec![CircleInstance::from(&CircleAttributes::default()); 3];
        let centers = [[0.0f32, 0.0], [0.0, 0.0], [5.0, 5.0]];
        let radii = [0.1f32, 0.1, 0.1];

        let submitted = renderer
            .submit_with_culling::<Circle, _>(device, queue, &instances, &centers, &radii)?;

        assert_eq!(submitted, 2);

        let stats = renderer.end_frame();
        assert_eq!(stats.culled_instances, 1);
        Ok(())
    }

    #[tokio::test]
    async fn test_pipeline_caching() -> GupResult<()> {
        let ctx = create_test_context().await?;
        let device = &ctx.device;

        let mut renderer = InstancedBatchRenderer::new(BatchRendererConfig::default());
        renderer.begin_frame();

        let _p1 = renderer.get_or_create_pipeline::<Circle>(device)?;
        assert_eq!(renderer.pipeline_cache_size(), 1);
        assert_eq!(renderer.current_stats.cache_misses, 1);

        let _p2 = renderer.get_or_create_pipeline::<Circle>(device)?;
        assert_eq!(renderer.pipeline_cache_size(), 1);
        assert_eq!(renderer.current_stats.cache_hits, 1);

        renderer.end_frame();
        Ok(())
    }

    #[tokio::test]
    async fn test_clear_pipeline_cache() -> GupResult<()> {
        let ctx = create_test_context().await?;
        let device = &ctx.device;

        let mut renderer = InstancedBatchRenderer::new(BatchRendererConfig::default());
        let _ = renderer.get_or_create_pipeline::<Circle>(device)?;
        assert_eq!(renderer.pipeline_cache_size(), 1);

        renderer.clear_pipeline_cache();
        assert_eq!(renderer.pipeline_cache_size(), 0);
        Ok(())
    }

    #[tokio::test]
    async fn test_rectangle_instancing() -> GupResult<()> {
        use crate::mark::Rectangle;
        use crate::mark::rectangle::{RectangleAttributes, RectangleInstance};

        let ctx = create_test_context().await?;
        let device = &ctx.device;
        let queue = &ctx.queue;

        let mut renderer = InstancedBatchRenderer::new(BatchRendererConfig::default());
        renderer.begin_frame();

        let instances = vec![RectangleInstance::from(&RectangleAttributes::default()); 500];

        renderer.ensure_geometry::<Rectangle>(device, queue)?;
        renderer.submit_instances::<Rectangle, _>(device, queue, &instances)?;

        assert_eq!(renderer.queued_batch_count(), 1);
        assert_eq!(renderer.queued_batches()[0].instance_count, 500);
        assert!(renderer.has_geometry::<Rectangle>());

        renderer.end_frame();
        Ok(())
    }

    #[tokio::test]
    async fn test_multi_mark_type_batching() -> GupResult<()> {
        use crate::mark::Rectangle;
        use crate::mark::rectangle::{RectangleAttributes, RectangleInstance};

        let ctx = create_test_context().await?;
        let device = &ctx.device;
        let queue = &ctx.queue;

        let mut renderer = InstancedBatchRenderer::new(BatchRendererConfig::default());
        renderer.begin_frame();

        // Submit circles
        let circle_instances = vec![CircleInstance::from(&CircleAttributes::default()); 100];
        renderer.submit_instances::<Circle, _>(device, queue, &circle_instances)?;

        // Submit rectangles
        let rect_instances = vec![RectangleInstance::from(&RectangleAttributes::default()); 200];
        renderer.submit_instances::<Rectangle, _>(device, queue, &rect_instances)?;

        // Should have 2 batches (one per mark type)
        assert_eq!(renderer.queued_batch_count(), 2);
        assert_eq!(renderer.queued_batches()[0].instance_count, 100);
        assert_eq!(renderer.queued_batches()[1].instance_count, 200);

        renderer.end_frame();
        Ok(())
    }

    #[tokio::test]
    async fn test_pipeline_caching_multiple_types() -> GupResult<()> {
        use crate::mark::Rectangle;

        let ctx = create_test_context().await?;
        let device = &ctx.device;

        let mut renderer = InstancedBatchRenderer::new(BatchRendererConfig::default());

        let _p1 = renderer.get_or_create_pipeline::<Circle>(device)?;
        let _p2 = renderer.get_or_create_pipeline::<Rectangle>(device)?;

        assert_eq!(renderer.pipeline_cache_size(), 2);

        // Hits on second retrieval
        let _p3 = renderer.get_or_create_pipeline::<Circle>(device)?;
        let _p4 = renderer.get_or_create_pipeline::<Rectangle>(device)?;
        assert_eq!(renderer.pipeline_cache_size(), 2);

        Ok(())
    }

    #[tokio::test]
    async fn test_large_instance_count() -> GupResult<()> {
        let ctx = create_test_context().await?;
        let device = &ctx.device;
        let queue = &ctx.queue;

        let mut renderer = InstancedBatchRenderer::new(BatchRendererConfig::default());
        renderer.begin_frame();

        // 10K instances - should fit in a single batch (max is 10_000)
        let instances = vec![CircleInstance::from(&CircleAttributes::default()); 10_000];

        renderer.submit_instances::<Circle, _>(device, queue, &instances)?;

        assert_eq!(renderer.queued_batch_count(), 1);
        assert_eq!(renderer.queued_batches()[0].instance_count, 10_000);

        let stats = renderer.end_frame();
        assert!(stats.buffer_upload_us > 0);
        Ok(())
    }

    #[tokio::test]
    async fn test_begin_frame_clears_batches() -> GupResult<()> {
        let ctx = create_test_context().await?;
        let device = &ctx.device;
        let queue = &ctx.queue;

        let mut renderer = InstancedBatchRenderer::new(BatchRendererConfig::default());
        renderer.begin_frame();

        let instances = vec![CircleInstance::from(&CircleAttributes::default()); 50];
        renderer.submit_instances::<Circle, _>(device, queue, &instances)?;
        assert_eq!(renderer.queued_batch_count(), 1);

        // New frame clears previous batches
        renderer.begin_frame();
        assert_eq!(renderer.queued_batch_count(), 0);

        renderer.end_frame();
        Ok(())
    }

    #[tokio::test]
    async fn test_culling_with_small_viewport() -> GupResult<()> {
        let ctx = create_test_context().await?;
        let device = &ctx.device;
        let queue = &ctx.queue;

        let mut renderer = InstancedBatchRenderer::new(BatchRendererConfig::default());
        renderer.culling_mut().set_viewport(Viewport2D {
            min_x: -0.5,
            max_x: 0.5,
            min_y: -0.5,
            max_y: 0.5,
            pixel_width: 800.0,
            pixel_height: 600.0,
        });

        renderer.begin_frame();

        let instances = vec![CircleInstance::from(&CircleAttributes::default()); 4];
        // Two inside, two outside
        let centers = [
            [0.0f32, 0.0], // inside
            [0.1, -0.1],   // inside
            [2.0, 0.0],    // outside
            [-3.0, 3.0],   // outside
        ];
        let radii = [0.05f32, 0.05, 0.05, 0.05];

        let submitted = renderer
            .submit_with_culling::<Circle, _>(device, queue, &instances, &centers, &radii)?;

        assert_eq!(submitted, 2);

        let stats = renderer.end_frame();
        assert_eq!(stats.culled_instances, 2);
        Ok(())
    }
}

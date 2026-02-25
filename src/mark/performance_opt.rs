// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Mark performance optimizations for GPU-accelerated rendering.
//!
//! This module provides performance enhancements for the mark rendering system:
//!
//! - **Enhanced pipeline caching**: Caches pipelines by (mark type, blend mode) pairs
//!   for fast lookup of blend mode variants without full pipeline recreation.
//! - **Mark buffer pool**: Type-aware buffer pooling with size classes for reduced
//!   allocation overhead when rendering similar mark batches.
//! - **Render state sorting**: Sorts render batches by pipeline to minimise GPU
//!   state transitions during rendering.
//! - **Performance metrics**: Tracks vertex processing, batching, pipeline transition,
//!   and memory allocation costs per frame.

use crate::error::GupResult;
use crate::mark::{Mark, MarkInfo, MarkInfoImpl};
use crate::mixable::BlendMode;
use std::any::TypeId;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use wgpu::{
    BlendState, ColorTargetState, ColorWrites, Device, FragmentState, FrontFace, MultisampleState,
    PipelineLayoutDescriptor, PolygonMode, PrimitiveState, PrimitiveTopology, RenderPipeline,
    RenderPipelineDescriptor, ShaderModuleDescriptor, ShaderSource, TextureFormat, VertexState,
    VertexStepMode,
};

// ---------------------------------------------------------------------------
// Enhanced pipeline cache key
// ---------------------------------------------------------------------------

/// Cache key combining mark type and blend mode.
///
/// Pipelines compiled for different blend states cannot be reused, so each
/// unique `(TypeId, BlendMode)` combination requires its own pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PipelineCacheKey {
    /// The mark type (e.g. `Circle`, `Rectangle`).
    pub mark_type_id: TypeId,
    /// The blend mode used for the colour target.
    pub blend_mode: BlendMode,
}

impl PipelineCacheKey {
    /// Create a key for the default alpha-blending mode.
    pub fn default_for<M: Mark>() -> Self {
        Self {
            mark_type_id: TypeId::of::<M>(),
            blend_mode: BlendMode::AlphaBlending,
        }
    }

    /// Create a key for a specific blend mode.
    pub fn with_blend<M: Mark>(blend_mode: BlendMode) -> Self {
        Self {
            mark_type_id: TypeId::of::<M>(),
            blend_mode,
        }
    }
}

// ---------------------------------------------------------------------------
// Enhanced pipeline cache
// ---------------------------------------------------------------------------

/// Statistics for the enhanced pipeline cache.
#[derive(Debug, Clone, Default)]
pub struct EnhancedCacheStats {
    /// Number of cache hits (pipeline reused).
    pub hits: u64,
    /// Number of cache misses (pipeline created).
    pub misses: u64,
    /// Number of full cache clears.
    pub invalidations: u64,
    /// Total time spent creating pipelines.
    pub total_creation_time: Duration,
    /// Average pipeline creation time (updated on each miss).
    pub avg_creation_time: Duration,
}

impl EnhancedCacheStats {
    /// Cache hit rate as a percentage (0.0–100.0).
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            (self.hits as f64 / total as f64) * 100.0
        }
    }
}

/// Pipeline cache that indexes by `(mark_type, blend_mode)` pairs.
///
/// This extends the basic [`PipelineCache`](crate::pipeline_cache::PipelineCache)
/// by caching blend-mode variants separately. A circle rendered with additive
/// blending gets its own pipeline entry distinct from a circle with normal
/// alpha blending.
///
/// # Example
///
/// ```rust,ignore
/// use gup::mark::performance_opt::EnhancedPipelineCache;
/// use gup::mixable::BlendMode;
///
/// let mut cache = EnhancedPipelineCache::new();
///
/// // Get default pipeline
/// let p1 = cache.get_or_create::<Circle>(&device, BlendMode::AlphaBlending)?;
/// // Different blend mode → separate pipeline
/// let p2 = cache.get_or_create::<Circle>(&device, BlendMode::Additive)?;
/// assert_ne!(Arc::as_ptr(&p1), Arc::as_ptr(&p2));
///
/// // Same blend mode → cache hit
/// let p3 = cache.get_or_create::<Circle>(&device, BlendMode::AlphaBlending)?;
/// assert!(Arc::ptr_eq(&p1, &p3));
/// ```
pub struct EnhancedPipelineCache {
    pipelines: HashMap<PipelineCacheKey, Arc<RenderPipeline>>,
    surface_format: Option<TextureFormat>,
    stats: EnhancedCacheStats,
}

impl EnhancedPipelineCache {
    /// Create a new empty cache.
    pub fn new() -> Self {
        Self {
            pipelines: HashMap::new(),
            surface_format: None,
            stats: EnhancedCacheStats::default(),
        }
    }

    /// Return an existing pipeline or create, cache, and return a new one.
    pub fn get_or_create<M: Mark>(
        &mut self,
        device: &Device,
        blend_mode: BlendMode,
    ) -> GupResult<Arc<RenderPipeline>> {
        let key = PipelineCacheKey::with_blend::<M>(blend_mode);

        if let Some(pipeline) = self.pipelines.get(&key) {
            self.stats.hits += 1;
            return Ok(Arc::clone(pipeline));
        }

        // Cache miss — create pipeline with specified blend mode.
        self.stats.misses += 1;
        let start = Instant::now();

        let pipeline = create_pipeline_with_blend::<M>(device, blend_mode)?;
        let elapsed = start.elapsed();

        self.stats.total_creation_time += elapsed;
        let total = self.stats.hits + self.stats.misses;
        self.stats.avg_creation_time = self.stats.total_creation_time / total as u32;

        let arc = Arc::new(pipeline);
        self.pipelines.insert(key, Arc::clone(&arc));
        Ok(arc)
    }

    /// Warm the cache by pre-creating pipelines for common combinations.
    ///
    /// Call during initialisation to avoid pipeline-creation stalls on the
    /// first frame.
    pub fn warm<M: Mark>(&mut self, device: &Device, blend_modes: &[BlendMode]) -> GupResult<()> {
        for &mode in blend_modes {
            let _ = self.get_or_create::<M>(device, mode)?;
        }
        Ok(())
    }

    /// Clear all cached pipelines (e.g. on device loss).
    pub fn clear(&mut self) {
        self.pipelines.clear();
        self.surface_format = None;
        self.stats.invalidations += 1;
    }

    /// Clear the cache if the surface format changed.
    pub fn invalidate_for_format(&mut self, format: TextureFormat) -> bool {
        match self.surface_format {
            Some(prev) if prev == format => false,
            _ => {
                self.pipelines.clear();
                self.surface_format = Some(format);
                self.stats.invalidations += 1;
                true
            }
        }
    }

    /// Current cache statistics.
    pub fn stats(&self) -> &EnhancedCacheStats {
        &self.stats
    }

    /// Number of cached pipelines.
    pub fn len(&self) -> usize {
        self.pipelines.len()
    }

    /// Whether the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.pipelines.is_empty()
    }

    /// Check whether a pipeline for the given key is cached.
    pub fn contains_key(&self, key: &PipelineCacheKey) -> bool {
        self.pipelines.contains_key(key)
    }
}

impl Default for EnhancedPipelineCache {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for EnhancedPipelineCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EnhancedPipelineCache")
            .field("cached_pipelines", &self.pipelines.len())
            .field("surface_format", &self.surface_format)
            .field("stats", &self.stats)
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Blend-mode pipeline creation helper
// ---------------------------------------------------------------------------

/// Convert a [`BlendMode`] to a wgpu [`BlendState`].
fn blend_mode_to_state(mode: BlendMode) -> Option<BlendState> {
    match mode {
        BlendMode::AlphaBlending => Some(BlendState::ALPHA_BLENDING),
        BlendMode::Additive => Some(wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::SrcAlpha,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            },
        }),
        BlendMode::Multiply => Some(wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::Dst,
                dst_factor: wgpu::BlendFactor::Zero,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::DstAlpha,
                dst_factor: wgpu::BlendFactor::Zero,
                operation: wgpu::BlendOperation::Add,
            },
        }),
        BlendMode::None => None,
    }
}

/// Create a render pipeline for mark `M` with a specific blend mode.
fn create_pipeline_with_blend<M: Mark>(
    device: &Device,
    blend_mode: BlendMode,
) -> GupResult<RenderPipeline> {
    let mark_info = MarkInfoImpl::<M>::new();

    // Get shader sources
    let (vertex_src, fragment_src) = if M::VERTEX_SHADER.is_some() && M::FRAGMENT_SHADER.is_some() {
        (
            M::VERTEX_SHADER.unwrap().to_string(),
            M::FRAGMENT_SHADER.unwrap().to_string(),
        )
    } else {
        let pipeline = crate::shader_pipeline::ComposableShaderPipeline::new();
        (
            M::generate_vertex_shader(&pipeline),
            M::generate_fragment_shader(&pipeline),
        )
    };

    let vertex_module = device.create_shader_module(ShaderModuleDescriptor {
        label: Some(&format!(
            "{}_vertex_{:?}",
            mark_info.type_name(),
            blend_mode
        )),
        source: ShaderSource::Wgsl(vertex_src.into()),
    });

    let fragment_module = device.create_shader_module(ShaderModuleDescriptor {
        label: Some(&format!(
            "{}_fragment_{:?}",
            mark_info.type_name(),
            blend_mode
        )),
        source: ShaderSource::Wgsl(fragment_src.into()),
    });

    // Bind group layout (reuse from MarkInfoImpl)
    let bind_group_layout = mark_info.create_bind_group_layout(device)?;

    let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some(&format!(
            "{}_{:?}_pipeline_layout",
            mark_info.type_name(),
            blend_mode
        )),
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });

    let vertex_buffer_layout = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<M::Vertex>() as wgpu::BufferAddress,
        step_mode: VertexStepMode::Vertex,
        attributes: M::vertex_attributes(),
    };

    let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
        label: Some(&format!(
            "{}_{:?}_pipeline",
            mark_info.type_name(),
            blend_mode
        )),
        layout: Some(&pipeline_layout),
        vertex: VertexState {
            module: &vertex_module,
            entry_point: Some("vs_main"),
            buffers: &[vertex_buffer_layout],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(FragmentState {
            module: &fragment_module,
            entry_point: Some("fs_main"),
            targets: &[Some(ColorTargetState {
                format: TextureFormat::Bgra8UnormSrgb,
                blend: blend_mode_to_state(blend_mode),
                write_mask: ColorWrites::ALL,
            })],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        primitive: PrimitiveState {
            topology: PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: FrontFace::Ccw,
            cull_mode: None,
            unclipped_depth: false,
            polygon_mode: PolygonMode::Fill,
            conservative: false,
        },
        depth_stencil: None,
        multisample: MultisampleState {
            count: 1,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        multiview: None,
        cache: None,
    });

    Ok(pipeline)
}

// ---------------------------------------------------------------------------
// Mark buffer pool with size classes
// ---------------------------------------------------------------------------

/// Size classes for efficient buffer pooling.
///
/// Rather than caching one buffer per exact byte size, instances are
/// rounded up to the nearest size class. This dramatically improves pool
/// hit rates for workloads with slightly varying instance counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SizeClass {
    /// Tiny: up to 64 elements.
    Tiny,
    /// Small: 65–256 elements.
    Small,
    /// Medium: 257–1024 elements.
    Medium,
    /// Large: 1025–4096 elements.
    Large,
    /// Huge: 4097–16384 elements.
    Huge,
    /// Massive: 16385+ elements.
    Massive,
}

impl SizeClass {
    /// Classify an element count into a size class.
    pub fn from_count(count: usize) -> Self {
        match count {
            0..=64 => SizeClass::Tiny,
            65..=256 => SizeClass::Small,
            257..=1024 => SizeClass::Medium,
            1025..=4096 => SizeClass::Large,
            4097..=16384 => SizeClass::Huge,
            _ => SizeClass::Massive,
        }
    }

    /// Get the buffer capacity for this size class (in elements).
    ///
    /// Buffers are allocated to the upper bound of their class so that
    /// any request within the class fits without reallocation.
    pub fn capacity(self) -> usize {
        match self {
            SizeClass::Tiny => 64,
            SizeClass::Small => 256,
            SizeClass::Medium => 1024,
            SizeClass::Large => 4096,
            SizeClass::Huge => 16384,
            SizeClass::Massive => 65536,
        }
    }
}

/// A pooled GPU buffer entry with usage tracking.
struct PooledBuffer {
    buffer: wgpu::Buffer,
    /// Capacity in bytes.
    byte_capacity: u64,
    /// When this buffer was last returned to the pool.
    last_returned: Instant,
}

/// Pool key combining mark type and size class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct PoolKey {
    mark_type_id: TypeId,
    size_class: SizeClass,
}

/// Mark-specific buffer pool for reducing allocation overhead.
///
/// Buffers are pooled by `(mark_type, size_class)` and reused across frames.
/// When a buffer is no longer needed, it is returned to the pool for future
/// reuse instead of being dropped.
///
/// # Size Classes
///
/// Rather than caching one buffer per exact byte count, element counts are
/// rounded up to the nearest [`SizeClass`]. This improves hit rates for
/// workloads with slightly varying instance counts.
///
/// # Eviction
///
/// Buffers that sit in the pool for longer than the configured timeout are
/// evicted to free GPU memory.
pub struct MarkBufferPool {
    /// Free buffers indexed by (mark type, size class).
    pools: HashMap<PoolKey, Vec<PooledBuffer>>,
    /// Maximum number of buffers per pool slot.
    max_per_slot: usize,
    /// Eviction timeout for idle buffers.
    eviction_timeout: Duration,
    /// Allocation statistics.
    pub stats: MarkBufferPoolStats,
}

/// Statistics for the mark buffer pool.
#[derive(Debug, Clone, Default)]
pub struct MarkBufferPoolStats {
    /// Number of times a pooled buffer was reused (hit).
    pub hits: u64,
    /// Number of times a new buffer had to be allocated (miss).
    pub misses: u64,
    /// Number of buffers returned to the pool.
    pub returns: u64,
    /// Number of buffers evicted due to timeout.
    pub evictions: u64,
    /// Total bytes currently held in the pool.
    pub pooled_bytes: u64,
}

impl MarkBufferPoolStats {
    /// Pool hit rate as a percentage.
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            (self.hits as f64 / total as f64) * 100.0
        }
    }
}

impl MarkBufferPool {
    /// Create a new pool with default settings.
    pub fn new() -> Self {
        Self {
            pools: HashMap::new(),
            max_per_slot: 4,
            eviction_timeout: Duration::from_secs(10),
            stats: MarkBufferPoolStats::default(),
        }
    }

    /// Create a new pool with custom settings.
    pub fn with_config(max_per_slot: usize, eviction_timeout: Duration) -> Self {
        Self {
            pools: HashMap::new(),
            max_per_slot,
            eviction_timeout,
            stats: MarkBufferPoolStats::default(),
        }
    }

    /// Acquire an instance buffer for the given mark type and element count.
    ///
    /// If a suitably sized buffer is available in the pool it is reused (hit),
    /// otherwise a new buffer is created (miss).
    pub fn acquire_instance_buffer<M: Mark>(
        &mut self,
        device: &Device,
        element_count: usize,
        element_size: usize,
    ) -> wgpu::Buffer {
        let size_class = SizeClass::from_count(element_count);
        let key = PoolKey {
            mark_type_id: TypeId::of::<M>(),
            size_class,
        };

        // Try to reuse a pooled buffer.
        if let Some(pool) = self.pools.get_mut(&key)
            && let Some(entry) = pool.pop()
        {
            self.stats.hits += 1;
            self.stats.pooled_bytes -= entry.byte_capacity;
            return entry.buffer;
        }

        // Pool miss — allocate a new buffer.
        self.stats.misses += 1;
        let capacity_elements = size_class.capacity().max(element_count);
        let byte_size = (capacity_elements * element_size) as u64;
        // Ensure at least 4-byte alignment for storage buffers.
        let byte_size = byte_size.div_ceil(4) * 4;

        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(&format!(
                "{}_instance_pool_{:?}",
                std::any::type_name::<M>(),
                size_class
            )),
            size: byte_size,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        })
    }

    /// Return a buffer to the pool for future reuse.
    pub fn return_buffer<M: Mark>(&mut self, buffer: wgpu::Buffer, element_count: usize) {
        let size_class = SizeClass::from_count(element_count);
        let key = PoolKey {
            mark_type_id: TypeId::of::<M>(),
            size_class,
        };

        let byte_capacity = buffer.size();
        let pool = self.pools.entry(key).or_default();

        if pool.len() < self.max_per_slot {
            self.stats.returns += 1;
            self.stats.pooled_bytes += byte_capacity;
            pool.push(PooledBuffer {
                buffer,
                byte_capacity,
                last_returned: Instant::now(),
            });
        }
        // else: pool full — buffer is dropped (deallocated).
    }

    /// Evict buffers that have been idle longer than the timeout.
    ///
    /// Call periodically (e.g. once per second) to free unused GPU memory.
    /// Returns the number of evicted buffers.
    pub fn evict_idle(&mut self) -> usize {
        let timeout = self.eviction_timeout;
        let now = Instant::now();
        let mut evicted = 0;

        for pool in self.pools.values_mut() {
            let before = pool.len();
            pool.retain(|entry| {
                let keep = now.duration_since(entry.last_returned) < timeout;
                if !keep {
                    self.stats.pooled_bytes -= entry.byte_capacity;
                }
                keep
            });
            evicted += before - pool.len();
        }

        self.stats.evictions += evicted as u64;
        evicted
    }

    /// Remove all pooled buffers.
    pub fn clear(&mut self) {
        let total_evicted: u64 = self.pools.values().map(|p| p.len() as u64).sum();
        self.stats.evictions += total_evicted;
        self.stats.pooled_bytes = 0;
        self.pools.clear();
    }

    /// Number of buffers currently in the pool.
    pub fn pooled_count(&self) -> usize {
        self.pools.values().map(|p| p.len()).sum()
    }

    /// Total bytes currently pooled.
    pub fn pooled_bytes(&self) -> u64 {
        self.stats.pooled_bytes
    }
}

impl Default for MarkBufferPool {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Render batch sorting
// ---------------------------------------------------------------------------

/// Sort key for render batches to minimise GPU state transitions.
///
/// Batches sharing the same pipeline should be rendered consecutively to
/// avoid expensive pipeline switches. Within the same pipeline, depth
/// ordering is preserved.
#[derive(Debug, Clone)]
pub struct SortedBatch {
    /// Original index in the unsorted batch list.
    pub original_index: usize,
    /// Mark type for pipeline grouping.
    pub mark_type_id: TypeId,
    /// Blend mode (affects pipeline selection).
    pub blend_mode: BlendMode,
    /// Depth/z-order for sorting within the same pipeline.
    pub z_order: f32,
    /// Number of instances in this batch.
    pub instance_count: u32,
}

/// Sort batches to minimise pipeline state transitions.
///
/// Primary sort key is `(mark_type_id, blend_mode)` to group batches
/// sharing the same pipeline. Secondary sort is by `z_order` for correct
/// depth ordering within a pipeline group.
///
/// Returns the sorted batch indices (into the original batch list).
pub fn sort_batches_by_state(batches: &[SortedBatch]) -> Vec<usize> {
    let mut indexed: Vec<(usize, &SortedBatch)> = batches.iter().enumerate().collect();

    indexed.sort_by(|(_, a), (_, b)| {
        // Primary: group by mark type (use hash-based ordering for TypeId)
        let type_a = blend_sort_key(a.mark_type_id, a.blend_mode);
        let type_b = blend_sort_key(b.mark_type_id, b.blend_mode);
        let key_cmp = type_a.cmp(&type_b);
        if key_cmp != std::cmp::Ordering::Equal {
            return key_cmp;
        }

        // Secondary: z-order (back to front for correct alpha blending)
        a.z_order
            .partial_cmp(&b.z_order)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    indexed.into_iter().map(|(i, _)| i).collect()
}

/// Compute a deterministic sort key from a TypeId and BlendMode.
///
/// TypeId doesn't implement Ord, so we hash it to produce a stable u64 key.
/// The blend mode discriminant is appended to separate blend variants.
fn blend_sort_key(type_id: TypeId, blend_mode: BlendMode) -> (u64, u8) {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    type_id.hash(&mut hasher);
    let type_hash = hasher.finish();
    let blend_disc = match blend_mode {
        BlendMode::None => 0,
        BlendMode::AlphaBlending => 1,
        BlendMode::Additive => 2,
        BlendMode::Multiply => 3,
    };
    (type_hash, blend_disc)
}

/// Count the number of pipeline switches in a sorted batch sequence.
///
/// A pipeline switch occurs when consecutive batches use different
/// `(mark_type_id, blend_mode)` pairs.
pub fn count_pipeline_switches(batches: &[SortedBatch], order: &[usize]) -> u32 {
    if order.len() <= 1 {
        return 0;
    }

    let mut switches = 0;
    for pair in order.windows(2) {
        let a = &batches[pair[0]];
        let b = &batches[pair[1]];
        if a.mark_type_id != b.mark_type_id || a.blend_mode != b.blend_mode {
            switches += 1;
        }
    }
    switches
}

// ---------------------------------------------------------------------------
// Per-frame performance metrics
// ---------------------------------------------------------------------------

/// Comprehensive performance metrics for mark rendering.
///
/// Collects timing and count data for a single frame, providing insight
/// into rendering bottlenecks.
#[derive(Debug, Clone, Default)]
pub struct MarkPerformanceMetrics {
    /// Time spent processing vertex data (buffer uploads).
    pub vertex_processing_time: Duration,
    /// Time spent sorting and preparing instance batches.
    pub instance_batching_time: Duration,
    /// Time spent on pipeline state transitions.
    pub pipeline_transition_time: Duration,
    /// Number of buffer allocations this frame.
    pub memory_allocation_count: u64,
    /// Number of buffer pool hits this frame.
    pub pool_hits: u64,
    /// Number of buffer pool misses this frame.
    pub pool_misses: u64,
    /// Number of draw calls issued.
    pub draw_calls: u32,
    /// Total instances rendered.
    pub total_instances: u32,
    /// Number of pipeline switches.
    pub pipeline_switches: u32,
    /// Cache hit rates for pipeline and buffer pools.
    pub cache_hit_rates: HashMap<String, f64>,
}

impl MarkPerformanceMetrics {
    /// Total frame processing time.
    pub fn total_time(&self) -> Duration {
        self.vertex_processing_time + self.instance_batching_time + self.pipeline_transition_time
    }

    /// Merge another metrics snapshot into this one (for accumulation).
    pub fn merge(&mut self, other: &MarkPerformanceMetrics) {
        self.vertex_processing_time += other.vertex_processing_time;
        self.instance_batching_time += other.instance_batching_time;
        self.pipeline_transition_time += other.pipeline_transition_time;
        self.memory_allocation_count += other.memory_allocation_count;
        self.pool_hits += other.pool_hits;
        self.pool_misses += other.pool_misses;
        self.draw_calls += other.draw_calls;
        self.total_instances += other.total_instances;
        self.pipeline_switches += other.pipeline_switches;
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --------------------------------------------------
    // SizeClass tests
    // --------------------------------------------------

    #[test]
    fn test_size_class_from_count() {
        assert_eq!(SizeClass::from_count(0), SizeClass::Tiny);
        assert_eq!(SizeClass::from_count(64), SizeClass::Tiny);
        assert_eq!(SizeClass::from_count(65), SizeClass::Small);
        assert_eq!(SizeClass::from_count(256), SizeClass::Small);
        assert_eq!(SizeClass::from_count(257), SizeClass::Medium);
        assert_eq!(SizeClass::from_count(1024), SizeClass::Medium);
        assert_eq!(SizeClass::from_count(1025), SizeClass::Large);
        assert_eq!(SizeClass::from_count(4096), SizeClass::Large);
        assert_eq!(SizeClass::from_count(4097), SizeClass::Huge);
        assert_eq!(SizeClass::from_count(16384), SizeClass::Huge);
        assert_eq!(SizeClass::from_count(16385), SizeClass::Massive);
        assert_eq!(SizeClass::from_count(100_000), SizeClass::Massive);
    }

    #[test]
    fn test_size_class_capacity_covers_upper_bound() {
        assert!(SizeClass::Tiny.capacity() >= 64);
        assert!(SizeClass::Small.capacity() >= 256);
        assert!(SizeClass::Medium.capacity() >= 1024);
        assert!(SizeClass::Large.capacity() >= 4096);
        assert!(SizeClass::Huge.capacity() >= 16384);
        assert!(SizeClass::Massive.capacity() >= 65536);
    }

    // --------------------------------------------------
    // PipelineCacheKey tests
    // --------------------------------------------------

    #[test]
    fn test_cache_key_equality() {
        use crate::mark::circle::Circle;

        let k1 = PipelineCacheKey::default_for::<Circle>();
        let k2 = PipelineCacheKey::with_blend::<Circle>(BlendMode::AlphaBlending);
        assert_eq!(k1, k2);

        let k3 = PipelineCacheKey::with_blend::<Circle>(BlendMode::Additive);
        assert_ne!(k1, k3);
    }

    #[test]
    fn test_cache_key_hash_consistency() {
        use crate::mark::circle::Circle;
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let k1 = PipelineCacheKey::default_for::<Circle>();
        let k2 = PipelineCacheKey::default_for::<Circle>();

        let mut h1 = DefaultHasher::new();
        k1.hash(&mut h1);
        let mut h2 = DefaultHasher::new();
        k2.hash(&mut h2);
        assert_eq!(h1.finish(), h2.finish());
    }

    // --------------------------------------------------
    // EnhancedCacheStats tests
    // --------------------------------------------------

    #[test]
    fn test_enhanced_cache_stats_hit_rate() {
        let stats = EnhancedCacheStats {
            hits: 7,
            misses: 3,
            ..Default::default()
        };
        assert!((stats.hit_rate() - 70.0).abs() < 0.01);
    }

    #[test]
    fn test_enhanced_cache_stats_empty() {
        let stats = EnhancedCacheStats::default();
        assert_eq!(stats.hit_rate(), 0.0);
    }

    // --------------------------------------------------
    // EnhancedPipelineCache non-GPU tests
    // --------------------------------------------------

    #[test]
    fn test_new_cache_is_empty() {
        let cache = EnhancedPipelineCache::new();
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn test_clear_increments_invalidations() {
        let mut cache = EnhancedPipelineCache::new();
        cache.clear();
        assert_eq!(cache.stats().invalidations, 1);
        cache.clear();
        assert_eq!(cache.stats().invalidations, 2);
    }

    #[test]
    fn test_invalidate_for_format_same() {
        let mut cache = EnhancedPipelineCache::new();
        let inv = cache.invalidate_for_format(TextureFormat::Bgra8UnormSrgb);
        assert!(inv);
        let inv2 = cache.invalidate_for_format(TextureFormat::Bgra8UnormSrgb);
        assert!(!inv2);
        assert_eq!(cache.stats().invalidations, 1);
    }

    #[test]
    fn test_invalidate_for_format_change() {
        let mut cache = EnhancedPipelineCache::new();
        cache.invalidate_for_format(TextureFormat::Bgra8UnormSrgb);
        let inv = cache.invalidate_for_format(TextureFormat::Rgba8Unorm);
        assert!(inv);
        assert_eq!(cache.stats().invalidations, 2);
    }

    // --------------------------------------------------
    // MarkBufferPool non-GPU tests
    // --------------------------------------------------

    #[test]
    fn test_pool_stats_hit_rate() {
        let stats = MarkBufferPoolStats {
            hits: 8,
            misses: 2,
            ..Default::default()
        };
        assert!((stats.hit_rate() - 80.0).abs() < 0.01);
    }

    #[test]
    fn test_pool_default_empty() {
        let pool = MarkBufferPool::new();
        assert_eq!(pool.pooled_count(), 0);
        assert_eq!(pool.pooled_bytes(), 0);
    }

    // --------------------------------------------------
    // Batch sorting tests
    // --------------------------------------------------

    #[test]
    fn test_sort_batches_groups_by_type() {
        let type_a = TypeId::of::<u32>();
        let type_b = TypeId::of::<f32>();

        let batches = vec![
            SortedBatch {
                original_index: 0,
                mark_type_id: type_b,
                blend_mode: BlendMode::AlphaBlending,
                z_order: 0.0,
                instance_count: 10,
            },
            SortedBatch {
                original_index: 1,
                mark_type_id: type_a,
                blend_mode: BlendMode::AlphaBlending,
                z_order: 0.0,
                instance_count: 20,
            },
            SortedBatch {
                original_index: 2,
                mark_type_id: type_b,
                blend_mode: BlendMode::AlphaBlending,
                z_order: 1.0,
                instance_count: 5,
            },
        ];

        let order = sort_batches_by_state(&batches);
        // type_a and type_b should be grouped, type_b items adjacent
        let types: Vec<_> = order.iter().map(|&i| batches[i].mark_type_id).collect();

        // All items of the same type must be contiguous
        let first_change = types.windows(2).position(|w| w[0] != w[1]);
        if let Some(pos) = first_change {
            // After the change, remaining items should all be the same type
            let remaining_type = types[pos + 1];
            assert!(types[pos + 1..].iter().all(|t| *t == remaining_type));
        }
    }

    #[test]
    fn test_sort_batches_preserves_z_order_within_group() {
        let type_a = TypeId::of::<u32>();

        let batches = vec![
            SortedBatch {
                original_index: 0,
                mark_type_id: type_a,
                blend_mode: BlendMode::AlphaBlending,
                z_order: 3.0,
                instance_count: 10,
            },
            SortedBatch {
                original_index: 1,
                mark_type_id: type_a,
                blend_mode: BlendMode::AlphaBlending,
                z_order: 1.0,
                instance_count: 10,
            },
            SortedBatch {
                original_index: 2,
                mark_type_id: type_a,
                blend_mode: BlendMode::AlphaBlending,
                z_order: 2.0,
                instance_count: 10,
            },
        ];

        let order = sort_batches_by_state(&batches);
        let z_orders: Vec<f32> = order.iter().map(|&i| batches[i].z_order).collect();
        assert_eq!(z_orders, vec![1.0, 2.0, 3.0]);
    }

    #[test]
    fn test_count_pipeline_switches_no_batches() {
        assert_eq!(count_pipeline_switches(&[], &[]), 0);
    }

    #[test]
    fn test_count_pipeline_switches_same_pipeline() {
        let type_a = TypeId::of::<u32>();
        let batches = vec![
            SortedBatch {
                original_index: 0,
                mark_type_id: type_a,
                blend_mode: BlendMode::AlphaBlending,
                z_order: 0.0,
                instance_count: 10,
            },
            SortedBatch {
                original_index: 1,
                mark_type_id: type_a,
                blend_mode: BlendMode::AlphaBlending,
                z_order: 1.0,
                instance_count: 10,
            },
        ];
        assert_eq!(count_pipeline_switches(&batches, &[0, 1]), 0);
    }

    #[test]
    fn test_count_pipeline_switches_different_types() {
        let type_a = TypeId::of::<u32>();
        let type_b = TypeId::of::<f32>();

        let batches = vec![
            SortedBatch {
                original_index: 0,
                mark_type_id: type_a,
                blend_mode: BlendMode::AlphaBlending,
                z_order: 0.0,
                instance_count: 10,
            },
            SortedBatch {
                original_index: 1,
                mark_type_id: type_b,
                blend_mode: BlendMode::AlphaBlending,
                z_order: 0.0,
                instance_count: 10,
            },
            SortedBatch {
                original_index: 2,
                mark_type_id: type_a,
                blend_mode: BlendMode::AlphaBlending,
                z_order: 0.0,
                instance_count: 10,
            },
        ];

        // Without sorting: a → b → a = 2 switches
        assert_eq!(count_pipeline_switches(&batches, &[0, 1, 2]), 2);

        // With sorting: grouped = at most 1 switch
        let sorted = sort_batches_by_state(&batches);
        assert!(count_pipeline_switches(&batches, &sorted) <= 1);
    }

    #[test]
    fn test_count_pipeline_switches_different_blend_modes() {
        let type_a = TypeId::of::<u32>();

        let batches = vec![
            SortedBatch {
                original_index: 0,
                mark_type_id: type_a,
                blend_mode: BlendMode::AlphaBlending,
                z_order: 0.0,
                instance_count: 10,
            },
            SortedBatch {
                original_index: 1,
                mark_type_id: type_a,
                blend_mode: BlendMode::Additive,
                z_order: 0.0,
                instance_count: 10,
            },
        ];

        assert_eq!(count_pipeline_switches(&batches, &[0, 1]), 1);
    }

    // --------------------------------------------------
    // MarkPerformanceMetrics tests
    // --------------------------------------------------

    #[test]
    fn test_metrics_total_time() {
        let m = MarkPerformanceMetrics {
            vertex_processing_time: Duration::from_millis(10),
            instance_batching_time: Duration::from_millis(5),
            pipeline_transition_time: Duration::from_millis(2),
            ..Default::default()
        };
        assert_eq!(m.total_time(), Duration::from_millis(17));
    }

    #[test]
    fn test_metrics_merge() {
        let mut a = MarkPerformanceMetrics {
            draw_calls: 3,
            total_instances: 100,
            ..Default::default()
        };
        let b = MarkPerformanceMetrics {
            draw_calls: 2,
            total_instances: 50,
            ..Default::default()
        };
        a.merge(&b);
        assert_eq!(a.draw_calls, 5);
        assert_eq!(a.total_instances, 150);
    }
}

// ---------------------------------------------------------------------------
// GPU integration tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod gpu_tests {
    use super::*;
    use crate::mark::circle::Circle;
    use crate::mark::rectangle::Rectangle;

    #[test]
    fn gpu_enhanced_cache_circle_normal() {
        pollster::block_on(async {
            let context = match crate::GupContext::headless().await {
                Ok(ctx) => ctx,
                Err(_) => {
                    eprintln!("Skipping GPU test — no adapter");
                    return;
                }
            };

            let mut cache = EnhancedPipelineCache::new();
            let p1 = cache
                .get_or_create::<Circle>(&context.device, BlendMode::AlphaBlending)
                .expect("create circle pipeline");
            let p2 = cache
                .get_or_create::<Circle>(&context.device, BlendMode::AlphaBlending)
                .expect("get cached pipeline");

            assert!(Arc::ptr_eq(&p1, &p2));
            assert_eq!(cache.stats().hits, 1);
            assert_eq!(cache.stats().misses, 1);
            assert_eq!(cache.len(), 1);
        });
    }

    #[test]
    fn gpu_enhanced_cache_different_blend_modes() {
        pollster::block_on(async {
            let context = match crate::GupContext::headless().await {
                Ok(ctx) => ctx,
                Err(_) => {
                    eprintln!("Skipping GPU test — no adapter");
                    return;
                }
            };

            let mut cache = EnhancedPipelineCache::new();
            let normal = cache
                .get_or_create::<Circle>(&context.device, BlendMode::AlphaBlending)
                .expect("normal");
            let additive = cache
                .get_or_create::<Circle>(&context.device, BlendMode::Additive)
                .expect("additive");

            // Different blend modes → different pipelines
            assert!(!Arc::ptr_eq(&normal, &additive));
            assert_eq!(cache.len(), 2);
            assert_eq!(cache.stats().misses, 2);
        });
    }

    #[test]
    fn gpu_enhanced_cache_different_mark_types() {
        pollster::block_on(async {
            let context = match crate::GupContext::headless().await {
                Ok(ctx) => ctx,
                Err(_) => {
                    eprintln!("Skipping GPU test — no adapter");
                    return;
                }
            };

            let mut cache = EnhancedPipelineCache::new();
            let circle = cache
                .get_or_create::<Circle>(&context.device, BlendMode::AlphaBlending)
                .expect("circle");
            let rect = cache
                .get_or_create::<Rectangle>(&context.device, BlendMode::AlphaBlending)
                .expect("rect");

            assert!(!Arc::ptr_eq(&circle, &rect));
            assert_eq!(cache.len(), 2);
        });
    }

    #[test]
    fn gpu_enhanced_cache_warm() {
        pollster::block_on(async {
            let context = match crate::GupContext::headless().await {
                Ok(ctx) => ctx,
                Err(_) => {
                    eprintln!("Skipping GPU test — no adapter");
                    return;
                }
            };

            let mut cache = EnhancedPipelineCache::new();
            cache
                .warm::<Circle>(
                    &context.device,
                    &[
                        BlendMode::AlphaBlending,
                        BlendMode::Additive,
                        BlendMode::Multiply,
                    ],
                )
                .expect("warm");

            assert_eq!(cache.len(), 3);
            assert_eq!(cache.stats().misses, 3);

            // All subsequent gets should be hits.
            let _ = cache
                .get_or_create::<Circle>(&context.device, BlendMode::AlphaBlending)
                .expect("hit");
            let _ = cache
                .get_or_create::<Circle>(&context.device, BlendMode::Additive)
                .expect("hit");
            assert_eq!(cache.stats().hits, 2);
        });
    }

    #[test]
    fn gpu_buffer_pool_acquire_and_return() {
        pollster::block_on(async {
            let context = match crate::GupContext::headless().await {
                Ok(ctx) => ctx,
                Err(_) => {
                    eprintln!("Skipping GPU test — no adapter");
                    return;
                }
            };

            let mut pool = MarkBufferPool::new();

            // First acquire → miss
            let buf1 = pool.acquire_instance_buffer::<Circle>(
                &context.device,
                100,
                std::mem::size_of::<crate::mark::circle::CircleInstance>(),
            );
            assert_eq!(pool.stats.misses, 1);
            assert_eq!(pool.stats.hits, 0);

            // Return to pool
            pool.return_buffer::<Circle>(buf1, 100);
            assert_eq!(pool.pooled_count(), 1);

            // Second acquire → hit (same size class as first)
            let _buf2 = pool.acquire_instance_buffer::<Circle>(
                &context.device,
                80, // Different count, same size class (Small: 65–256)
                std::mem::size_of::<crate::mark::circle::CircleInstance>(),
            );
            assert_eq!(pool.stats.hits, 1);
            assert_eq!(pool.pooled_count(), 0);
        });
    }

    #[test]
    fn gpu_buffer_pool_eviction() {
        pollster::block_on(async {
            let context = match crate::GupContext::headless().await {
                Ok(ctx) => ctx,
                Err(_) => {
                    eprintln!("Skipping GPU test — no adapter");
                    return;
                }
            };

            let mut pool = MarkBufferPool::with_config(4, Duration::from_millis(1));

            let buf = pool.acquire_instance_buffer::<Circle>(
                &context.device,
                10,
                std::mem::size_of::<crate::mark::circle::CircleInstance>(),
            );
            pool.return_buffer::<Circle>(buf, 10);
            assert_eq!(pool.pooled_count(), 1);

            // Wait for eviction timeout
            std::thread::sleep(Duration::from_millis(5));

            let evicted = pool.evict_idle();
            assert_eq!(evicted, 1);
            assert_eq!(pool.pooled_count(), 0);
        });
    }

    #[test]
    fn gpu_enhanced_cache_creation_time_tracked() {
        pollster::block_on(async {
            let context = match crate::GupContext::headless().await {
                Ok(ctx) => ctx,
                Err(_) => {
                    eprintln!("Skipping GPU test — no adapter");
                    return;
                }
            };

            let mut cache = EnhancedPipelineCache::new();
            let _ = cache
                .get_or_create::<Circle>(&context.device, BlendMode::AlphaBlending)
                .expect("create");

            // Pipeline creation should take some non-zero time
            assert!(cache.stats().total_creation_time > Duration::ZERO);
            assert!(cache.stats().avg_creation_time > Duration::ZERO);
        });
    }
}

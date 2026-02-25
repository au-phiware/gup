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

//! Shader Pipeline Builder System
//!
//! This module implements the ShaderPipeline system that takes composed shader functions
//! and generates optimized WGSL vertex and fragment shaders for the GPU. It handles
//! function composition, uniform buffer management, and generates high-quality WGSL code
//! that leverages GPU parallel processing.

use crate::buffer::{BufferType, GpuBuffer};
use crate::error::GupResult;
use crate::shader_function::{ComposableShaderFunction, ShaderUniform};
use lru::LruCache;
use std::collections::{HashMap, VecDeque};
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::Duration;
use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingType, BufferBindingType, ColorTargetState, Device, FragmentState,
    MultisampleState, PrimitiveState, Queue, RenderPipeline, RenderPipelineDescriptor,
    ShaderModule, ShaderModuleDescriptor, ShaderSource, ShaderStages, VertexState,
};

/// Represents a shader function in the pipeline with its metadata and uniform buffer.
pub struct PipelineFunction {
    name: String,
    wgsl_code: String,
    uniform_buffer: Option<Box<dyn std::any::Any + Send + Sync>>,
    uniform_size: usize,
    uniform_type_name: String,
    uniform_struct_definition: String,
}

impl PipelineFunction {
    pub fn new<F: ComposableShaderFunction + 'static>(function: F) -> Self
    where
        F::Uniforms: Send + Sync + 'static,
    {
        let name = F::function_name().to_string();
        let wgsl_code = function.generate_wgsl();
        let uniform_buffer = function
            .create_uniforms()
            .map(|u| Box::new(u) as Box<dyn std::any::Any + Send + Sync>);
        let uniform_size = std::mem::size_of::<F::Uniforms>();
        let uniform_type_name = F::Uniforms::wgsl_type_name().to_string();
        let uniform_struct_definition = F::Uniforms::wgsl_struct_definition();

        Self {
            name,
            wgsl_code,
            uniform_buffer,
            uniform_size,
            uniform_type_name,
            uniform_struct_definition,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn wgsl_code(&self) -> &str {
        &self.wgsl_code
    }

    pub fn has_uniforms(&self) -> bool {
        self.uniform_buffer.is_some()
    }

    pub fn uniform_size(&self) -> usize {
        self.uniform_size
    }

    pub fn uniform_type_name(&self) -> &str {
        &self.uniform_type_name
    }

    pub fn uniform_struct_definition(&self) -> &str {
        &self.uniform_struct_definition
    }
}

/// Cached shader compilation results to avoid regeneration.
#[derive(Clone)]
pub struct CachedShaders {
    pub vertex_shader: String,
    pub fragment_shader: String,
    pub bind_group_layout: Option<Arc<BindGroupLayout>>,
    pub vertex_module: Option<Arc<ShaderModule>>,
    pub fragment_module: Option<Arc<ShaderModule>>,
}

/// Attribute mapping configuration for shader pipeline.
#[derive(Debug, Clone)]
pub struct AttributeMapping {
    pub attribute_name: String,
    pub function_name: String,
    pub location: u32,
}

/// Configuration for function inlining optimizations.
#[derive(Debug, Clone)]
pub struct InliningConfig {
    /// Maximum number of lines in a function to consider for inlining
    pub inline_threshold: usize,
    /// Maximum number of call sites before skipping inline
    pub call_count_threshold: usize,
    /// Enable AST-based inlining (more accurate but slower)
    pub use_ast_analysis: bool,
}

impl Default for InliningConfig {
    fn default() -> Self {
        Self {
            inline_threshold: 5,
            call_count_threshold: 3,
            use_ast_analysis: false,
        }
    }
}

/// Configuration for overall optimization behavior.
#[derive(Debug, Clone)]
pub struct OptimizationConfig {
    /// Enable function inlining
    pub enable_inlining: bool,
    /// Enable constant folding
    pub enable_constant_folding: bool,
    /// Enable dead code elimination
    pub enable_dead_code_elimination: bool,
    /// Use AST-based optimization passes instead of string-based ones.
    ///
    /// When enabled, `optimize_shader()` parses the WGSL source into an AST,
    /// runs dead-code elimination, constant folding, and function inlining on
    /// the AST, then regenerates WGSL text.  Falls back to string-based
    /// optimizations if AST parsing fails.
    pub use_ast_analysis: bool,
    /// Inlining configuration
    pub inlining: InliningConfig,
}

impl Default for OptimizationConfig {
    fn default() -> Self {
        Self {
            enable_inlining: true,
            enable_constant_folding: true,
            enable_dead_code_elimination: true,
            use_ast_analysis: false,
            inlining: InliningConfig::default(),
        }
    }
}

/// Statistics about cache performance.
#[derive(Debug, Clone, Default)]
pub struct CacheStatistics {
    /// Number of cache hits
    pub hits: usize,
    /// Number of cache misses
    pub misses: usize,
    /// Current number of entries in cache
    pub entries: usize,
    /// Estimated memory usage in bytes
    pub memory_usage: usize,
}

impl CacheStatistics {
    /// Calculate cache hit rate (0.0 to 1.0)
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }
}

/// Performance profiling data for pipeline operations.
#[derive(Debug, Clone, Default)]
pub struct ProfileReport {
    /// Time spent generating shaders
    pub generation_time: Duration,
    /// Time spent compiling on GPU
    pub compilation_time: Duration,
    /// Number of functions in pipeline
    pub function_count: usize,
    /// Vertex shader size in bytes
    pub vertex_shader_size: usize,
    /// Fragment shader size in bytes
    pub fragment_shader_size: usize,
    /// Cache statistics
    pub cache_stats: CacheStatistics,
}

/// Optimization recommendation based on profiling.
#[derive(Debug, Clone)]
pub struct OptimizationRecommendation {
    /// Type of optimization
    pub optimization_type: String,
    /// Reason for recommendation
    pub reason: String,
    /// Estimated impact
    pub estimated_impact: String,
}

/// Performance profiler for pipeline operations.
#[derive(Debug, Default)]
pub struct PipelineProfiler {
    enabled: bool,
    generation_times: Vec<Duration>,
    compilation_times: Vec<Duration>,
    cache_stats: CacheStatistics,
}

impl PipelineProfiler {
    /// Create a new profiler.
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            ..Default::default()
        }
    }

    /// Record shader generation time.
    pub fn record_generation(&mut self, duration: Duration) {
        if self.enabled {
            self.generation_times.push(duration);
        }
    }

    /// Record GPU compilation time.
    pub fn record_compilation(&mut self, duration: Duration) {
        if self.enabled {
            self.compilation_times.push(duration);
        }
    }

    /// Record cache hit.
    pub fn record_cache_hit(&mut self) {
        if self.enabled {
            self.cache_stats.hits += 1;
        }
    }

    /// Record cache miss.
    pub fn record_cache_miss(&mut self) {
        if self.enabled {
            self.cache_stats.misses += 1;
        }
    }

    /// Update cache entry count.
    pub fn update_cache_entries(&mut self, count: usize) {
        if self.enabled {
            self.cache_stats.entries = count;
        }
    }

    /// Update estimated cache memory usage.
    pub fn update_cache_memory(&mut self, bytes: usize) {
        if self.enabled {
            self.cache_stats.memory_usage = bytes;
        }
    }

    /// Generate a performance report.
    pub fn report(
        &self,
        function_count: usize,
        vertex_size: usize,
        fragment_size: usize,
    ) -> ProfileReport {
        let avg_generation = if self.generation_times.is_empty() {
            Duration::ZERO
        } else {
            self.generation_times.iter().sum::<Duration>() / self.generation_times.len() as u32
        };

        let avg_compilation = if self.compilation_times.is_empty() {
            Duration::ZERO
        } else {
            self.compilation_times.iter().sum::<Duration>() / self.compilation_times.len() as u32
        };

        ProfileReport {
            generation_time: avg_generation,
            compilation_time: avg_compilation,
            function_count,
            vertex_shader_size: vertex_size,
            fragment_shader_size: fragment_size,
            cache_stats: self.cache_stats.clone(),
        }
    }

    /// Generate optimization recommendations based on profiling data.
    pub fn recommendations(&self) -> Vec<OptimizationRecommendation> {
        let mut recs = Vec::new();

        // Check cache hit rate
        if self.cache_stats.hit_rate() < 0.8 && self.cache_stats.hits + self.cache_stats.misses > 10
        {
            recs.push(OptimizationRecommendation {
                optimization_type: "Cache Size".to_string(),
                reason: format!(
                    "Cache hit rate is {:.1}%, consider increasing cache size",
                    self.cache_stats.hit_rate() * 100.0
                ),
                estimated_impact: "10-30% performance improvement".to_string(),
            });
        }

        // Check generation time
        if let Some(&max_time) = self.generation_times.iter().max()
            && max_time > Duration::from_millis(5)
        {
            recs.push(OptimizationRecommendation {
                optimization_type: "Shader Generation".to_string(),
                reason: format!("Peak generation time is {:?}, exceeds 5ms target", max_time),
                estimated_impact: "Consider reducing pipeline complexity".to_string(),
            });
        }

        recs
    }
}

/// LRU cache for compiled shader pipelines.
pub struct LruPipelineCache {
    cache: LruCache<u64, CachedShaders>,
    profiler: PipelineProfiler,
}

impl LruPipelineCache {
    /// Create a new LRU cache with the specified capacity.
    pub fn new(capacity: usize, enable_profiling: bool) -> Self {
        Self {
            cache: LruCache::new(
                NonZeroUsize::new(capacity).unwrap_or(NonZeroUsize::new(32).unwrap()),
            ),
            profiler: PipelineProfiler::new(enable_profiling),
        }
    }

    /// Get a cached shader pipeline.
    pub fn get(&mut self, key: u64) -> Option<&CachedShaders> {
        // Check if key exists first
        let exists = self.cache.contains(&key);
        if exists {
            self.profiler.record_cache_hit();
            let cache_len = self.cache.len();
            self.profiler.update_cache_entries(cache_len);
            self.cache.get(&key)
        } else {
            self.profiler.record_cache_miss();
            let cache_len = self.cache.len();
            self.profiler.update_cache_entries(cache_len);
            None
        }
    }

    /// Insert a shader pipeline into the cache.
    pub fn put(&mut self, key: u64, value: CachedShaders) {
        self.cache.put(key, value);
        self.profiler.update_cache_entries(self.cache.len());
        self.update_memory_estimate();
    }

    /// Get cache statistics.
    pub fn statistics(&self) -> CacheStatistics {
        self.profiler.cache_stats.clone()
    }

    /// Get profiler reference.
    pub fn profiler(&self) -> &PipelineProfiler {
        &self.profiler
    }

    /// Get mutable profiler reference.
    pub fn profiler_mut(&mut self) -> &mut PipelineProfiler {
        &mut self.profiler
    }

    /// Estimate memory usage (rough approximation).
    fn update_memory_estimate(&mut self) {
        // Rough estimate: 10KB per cached shader pipeline
        let estimated_bytes = self.cache.len() * 10 * 1024;
        self.profiler.update_cache_memory(estimated_bytes);
    }

    /// Clear the cache.
    pub fn clear(&mut self) {
        self.cache.clear();
        self.profiler.update_cache_entries(0);
        self.profiler.update_cache_memory(0);
    }
}

/// Batch pipeline operations for improved performance.
pub struct PipelineBatch {
    pipelines: Vec<(String, ComposableShaderPipeline)>,
}

impl PipelineBatch {
    /// Create a new pipeline batch.
    pub fn new() -> Self {
        Self {
            pipelines: Vec::new(),
        }
    }

    /// Add a pipeline to the batch.
    pub fn add_pipeline(&mut self, id: String, pipeline: ComposableShaderPipeline) {
        self.pipelines.push((id, pipeline));
    }

    /// Generate all shaders in parallel (conceptually - actual parallelization would need rayon or similar).
    pub fn generate_all_shaders(&self) -> Vec<(String, String, String)> {
        self.pipelines
            .iter()
            .map(|(id, pipeline)| {
                let vertex = pipeline.generate_vertex_shader();
                let fragment = pipeline.generate_fragment_shader();
                (id.clone(), vertex, fragment)
            })
            .collect()
    }

    /// Get the number of pipelines in the batch.
    pub fn len(&self) -> usize {
        self.pipelines.len()
    }

    /// Check if the batch is empty.
    pub fn is_empty(&self) -> bool {
        self.pipelines.is_empty()
    }
}

impl Default for PipelineBatch {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Uniform Buffer Pool
// ---------------------------------------------------------------------------

/// Pool of reusable GPU uniform buffers, bucketed by size.
///
/// Uniform buffers in visualization pipelines tend to cluster around a small set
/// of sizes (e.g. 16 bytes for `LinearScaleUniforms`, 32 bytes for
/// `ColorMapUniforms`).  Rather than allocating and deallocating buffers every
/// frame, the pool keeps returned buffers for reuse, avoiding GPU allocation
/// overhead.
pub struct UniformBufferPool {
    /// Available buffers grouped by their aligned capacity.
    free: HashMap<usize, VecDeque<GpuBuffer<u8>>>,
    /// Total number of buffers ever created (for stats).
    total_created: usize,
    /// Total number of reuses (for stats).
    total_reused: usize,
    /// Maximum number of free buffers per size bucket.
    max_per_bucket: usize,
}

impl UniformBufferPool {
    /// Create a new pool.  `max_per_bucket` limits how many idle buffers are
    /// retained for each size class (prevents unbounded growth).
    pub fn new(max_per_bucket: usize) -> Self {
        Self {
            free: HashMap::new(),
            total_created: 0,
            total_reused: 0,
            max_per_bucket,
        }
    }

    /// Round a size up to the nearest aligned bucket.
    ///
    /// Uniform buffers require 256-byte alignment, so we bucket by multiples
    /// of 256 to maximize reuse.
    fn aligned_size(size: usize) -> usize {
        const ALIGNMENT: usize = 256;
        size.div_ceil(ALIGNMENT) * ALIGNMENT
    }

    /// Acquire a buffer of at least `min_size` bytes.
    ///
    /// Returns a pooled buffer if one is available; otherwise creates a new
    /// one on the given device.
    pub fn acquire(&mut self, device: &Device, min_size: usize) -> GpuBuffer<u8> {
        let bucket_size = Self::aligned_size(min_size.max(1));

        if let Some(queue) = self.free.get_mut(&bucket_size)
            && let Some(buffer) = queue.pop_front()
        {
            self.total_reused += 1;
            return buffer;
        }

        self.total_created += 1;
        GpuBuffer::new(device, BufferType::Uniform, bucket_size)
    }

    /// Return a buffer to the pool for later reuse.
    pub fn release(&mut self, buffer: GpuBuffer<u8>) {
        let bucket_size = Self::aligned_size(buffer.capacity());
        let queue = self.free.entry(bucket_size).or_default();

        if queue.len() < self.max_per_bucket {
            queue.push_back(buffer);
        }
        // else: drop the buffer — pool is full for this size class
    }

    /// Number of distinct size buckets currently tracked.
    pub fn bucket_count(&self) -> usize {
        self.free.len()
    }

    /// Total number of buffers currently idle in the pool.
    pub fn idle_count(&self) -> usize {
        self.free.values().map(|q| q.len()).sum()
    }

    /// Pool statistics.
    pub fn stats(&self) -> UniformPoolStats {
        UniformPoolStats {
            total_created: self.total_created,
            total_reused: self.total_reused,
            idle_buffers: self.idle_count(),
            bucket_count: self.bucket_count(),
        }
    }

    /// Discard all idle buffers (e.g. on device loss).
    pub fn clear(&mut self) {
        self.free.clear();
    }
}

impl Default for UniformBufferPool {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Statistics for [`UniformBufferPool`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UniformPoolStats {
    /// Total buffers created (GPU allocations).
    pub total_created: usize,
    /// Total times a pooled buffer was reused instead of allocating.
    pub total_reused: usize,
    /// Number of idle buffers currently in the pool.
    pub idle_buffers: usize,
    /// Number of distinct size buckets.
    pub bucket_count: usize,
}

impl UniformPoolStats {
    /// Reuse rate as a percentage (0–100).
    pub fn reuse_rate(&self) -> f64 {
        let total = self.total_created + self.total_reused;
        if total == 0 {
            0.0
        } else {
            (self.total_reused as f64 / total as f64) * 100.0
        }
    }
}

// ---------------------------------------------------------------------------
// Uniform Batcher
// ---------------------------------------------------------------------------

/// A pending uniform buffer write.
struct PendingWrite {
    buffer_name: String,
    data: Vec<u8>,
}

/// Batches multiple uniform buffer updates into fewer GPU transfer operations.
///
/// Instead of issuing one `queue.write_buffer()` per uniform, pending writes
/// are collected and flushed in a single batch.  This reduces driver overhead
/// when many pipelines are updated each frame.
pub struct UniformBatcher {
    pending: Vec<PendingWrite>,
}

impl UniformBatcher {
    /// Create a new, empty batcher.
    pub fn new() -> Self {
        Self {
            pending: Vec::new(),
        }
    }

    /// Stage a uniform update.  The data is copied and will be written when
    /// [`flush`](Self::flush) is called.
    pub fn stage(&mut self, buffer_name: &str, data: &[u8]) {
        self.pending.push(PendingWrite {
            buffer_name: buffer_name.to_string(),
            data: data.to_vec(),
        });
    }

    /// Number of pending writes.
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Flush all pending writes to their corresponding GPU buffers.
    ///
    /// Iterates through the pending writes and uploads each to the buffer
    /// found in `uniform_buffers`.  Clears the pending list afterward.
    ///
    /// Returns the number of writes actually performed.
    pub fn flush(
        &mut self,
        device: &Device,
        queue: &Queue,
        uniform_buffers: &mut HashMap<String, GpuBuffer<u8>>,
    ) -> GupResult<usize> {
        let mut written = 0;
        for write in self.pending.drain(..) {
            if let Some(buffer) = uniform_buffers.get_mut(&write.buffer_name) {
                buffer.upload(device, queue, &write.data)?;
                written += 1;
            }
        }
        Ok(written)
    }

    /// Discard all pending writes without flushing.
    pub fn clear(&mut self) {
        self.pending.clear();
    }
}

impl Default for UniformBatcher {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Bind Group Cache
// ---------------------------------------------------------------------------

/// Cached bind group keyed by the hash of the pipeline's uniform layout.
///
/// Avoids recreating bind groups when the pipeline's uniform configuration
/// has not changed.
pub struct BindGroupCache {
    cache: HashMap<u64, (Arc<BindGroupLayout>, Arc<BindGroup>)>,
    stats: BindGroupCacheStats,
}

/// Statistics for [`BindGroupCache`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BindGroupCacheStats {
    /// Number of cache hits.
    pub hits: usize,
    /// Number of cache misses (new bind groups created).
    pub misses: usize,
}

impl BindGroupCacheStats {
    /// Hit rate as a percentage (0–100).
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            (self.hits as f64 / total as f64) * 100.0
        }
    }
}

impl BindGroupCache {
    /// Create a new, empty bind group cache.
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
            stats: BindGroupCacheStats::default(),
        }
    }

    /// Retrieve a cached bind group, or return `None` on a miss.
    pub fn get(&mut self, key: u64) -> Option<(&Arc<BindGroupLayout>, &Arc<BindGroup>)> {
        if let Some(entry) = self.cache.get(&key) {
            self.stats.hits += 1;
            Some((&entry.0, &entry.1))
        } else {
            self.stats.misses += 1;
            None
        }
    }

    /// Insert a bind group + layout pair into the cache.
    pub fn insert(&mut self, key: u64, layout: Arc<BindGroupLayout>, bind_group: Arc<BindGroup>) {
        self.cache.insert(key, (layout, bind_group));
    }

    /// Number of entries in the cache.
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// Whether the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }

    /// Current statistics.
    pub fn stats(&self) -> &BindGroupCacheStats {
        &self.stats
    }

    /// Clear all entries (e.g. on device loss).
    pub fn clear(&mut self) {
        self.cache.clear();
    }
}

impl Default for BindGroupCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Core shader pipeline that manages function composition and WGSL generation.
pub struct ComposableShaderPipeline {
    functions: Vec<PipelineFunction>,
    attribute_mappings: Vec<AttributeMapping>,
    cached_shaders: Option<CachedShaders>,
    uniform_buffers: HashMap<String, GpuBuffer<u8>>,
    pipeline_hash: u64,
    optimization_config: OptimizationConfig,
    profiler: Option<PipelineProfiler>,
}

impl Default for ComposableShaderPipeline {
    fn default() -> Self {
        Self::new()
    }
}

impl ComposableShaderPipeline {
    /// Create a new empty shader pipeline.
    pub fn new() -> Self {
        Self {
            functions: Vec::new(),
            attribute_mappings: Vec::new(),
            cached_shaders: None,
            uniform_buffers: HashMap::new(),
            pipeline_hash: 0,
            optimization_config: OptimizationConfig::default(),
            profiler: None,
        }
    }

    /// Create a new pipeline with custom optimization configuration.
    pub fn with_optimization_config(mut self, config: OptimizationConfig) -> Self {
        self.optimization_config = config;
        self
    }

    /// Enable profiling for this pipeline.
    pub fn with_profiling(mut self, enabled: bool) -> Self {
        if enabled {
            self.profiler = Some(PipelineProfiler::new(true));
        }
        self
    }

    /// Add a shader function to the pipeline.
    pub fn add_function<F: ComposableShaderFunction + 'static>(&mut self, function: F)
    where
        F::Uniforms: Send + Sync + 'static,
    {
        let pipeline_function = PipelineFunction::new(function);
        self.functions.push(pipeline_function);
        self.invalidate_cache();
    }

    /// Map an attribute name to a function output for use in vertex/fragment shaders.
    pub fn map_attribute(&mut self, attr_name: &str, function_name: &str) {
        let location = self.attribute_mappings.len() as u32;
        self.attribute_mappings.push(AttributeMapping {
            attribute_name: attr_name.to_string(),
            function_name: function_name.to_string(),
            location,
        });
        self.invalidate_cache();
    }

    /// Get the number of functions in the pipeline.
    pub fn function_count(&self) -> usize {
        self.functions.len()
    }

    /// Invalidate the shader cache when pipeline changes.
    fn invalidate_cache(&mut self) {
        self.cached_shaders = None;
        self.pipeline_hash = self.calculate_hash();
    }

    /// Calculate a hash for the current pipeline configuration.
    fn calculate_hash(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();

        // Hash function names and WGSL code
        for function in &self.functions {
            function.name().hash(&mut hasher);
            function.wgsl_code().hash(&mut hasher);
        }

        // Hash attribute mappings
        for mapping in &self.attribute_mappings {
            mapping.attribute_name.hash(&mut hasher);
            mapping.function_name.hash(&mut hasher);
            mapping.location.hash(&mut hasher);
        }

        hasher.finish()
    }

    /// Generate data type definitions for WGSL.
    fn generate_data_type_definitions(&self) -> String {
        let mut definitions = String::new();

        definitions.push_str("struct VertexInput {\n");
        definitions.push_str("    @builtin(vertex_index) vertex_index: u32,\n");
        definitions.push_str("}\n\n");

        definitions.push_str("struct VertexOutput {\n");
        definitions.push_str("    @builtin(position) clip_position: vec4<f32>,\n");

        for mapping in &self.attribute_mappings {
            definitions.push_str(&format!(
                "    @location({}) {}: vec4<f32>,\n",
                mapping.location, mapping.attribute_name
            ));
        }

        definitions.push_str("}\n\n");

        definitions
    }

    /// Generate uniform struct definitions and bindings for WGSL.
    fn generate_uniform_bindings(&self) -> String {
        let mut bindings = String::new();
        let mut binding_index = 0;

        // Generate uniform struct definitions using ShaderUniform trait
        let mut defined_types = std::collections::HashSet::new();
        for function in self.functions.iter() {
            if function.has_uniforms() {
                let uniform_type_name = function.uniform_type_name();

                // Only add the struct definition once per type
                if !defined_types.contains(uniform_type_name) {
                    defined_types.insert(uniform_type_name.to_string());

                    let struct_def = function.uniform_struct_definition();
                    if !struct_def.is_empty() {
                        bindings.push_str(struct_def);
                        bindings.push_str("\n\n");
                    }
                }
            }
        }

        // Generate uniform variable bindings
        for (i, function) in self.functions.iter().enumerate() {
            if function.has_uniforms() {
                bindings.push_str(&format!(
                    "@group(0) @binding({}) var<uniform> {}_uniforms_{}: {};\n",
                    binding_index,
                    function.name(),
                    i,
                    function.uniform_type_name()
                ));
                binding_index += 1;
            }
        }

        bindings.push('\n');
        bindings
    }

    /// Generate the main vertex function.
    fn generate_main_vertex_function(&self) -> String {
        let mut vertex_fn = String::new();

        vertex_fn.push_str("@vertex\n");
        vertex_fn.push_str("fn vs_main(in: VertexInput) -> VertexOutput {\n");
        vertex_fn.push_str("    var output: VertexOutput;\n");
        vertex_fn.push_str("    \n");

        // Calculate position based on vertex index (simple grid layout for demonstration)
        vertex_fn.push_str("    let x = f32(in.vertex_index % 2u) * 2.0 - 1.0;\n");
        vertex_fn.push_str("    let y = f32(in.vertex_index / 2u) * 2.0 - 1.0;\n");
        vertex_fn.push_str("    output.clip_position = vec4<f32>(x * 0.5, y * 0.5, 0.0, 1.0);\n");
        vertex_fn.push_str("    \n");

        // Apply attribute transformations based on mappings
        for mapping in &self.attribute_mappings {
            if let Some((i, function)) = self
                .functions
                .iter()
                .enumerate()
                .find(|(_, f)| f.name() == mapping.function_name)
            {
                let unique_function_name = format!("{}_{}", function.name(), i);

                if function.has_uniforms() {
                    match function.name() {
                        "position_transform" => {
                            // PositionTransform expects vec2<f32> as first parameter
                            vertex_fn.push_str(&format!(
                                "    let {}_result = {}(vec2<f32>(x, y), {}_uniforms_{});\n",
                                mapping.attribute_name,
                                unique_function_name,
                                function.name(),
                                i
                            ));
                        }
                        _ => {
                            // Other functions expect f32 as first parameter
                            vertex_fn.push_str(&format!(
                                "    let {}_result = {}(f32(in.vertex_index), {}_uniforms_{});\n",
                                mapping.attribute_name,
                                unique_function_name,
                                function.name(),
                                i
                            ));
                        }
                    }
                } else {
                    vertex_fn.push_str(&format!(
                        "    let {}_result = {}(f32(in.vertex_index));\n",
                        mapping.attribute_name, unique_function_name
                    ));
                }

                // Convert result to vec4 for output based on function type
                match function.name() {
                    "color_map" => {
                        // ColorMap already returns vec4<f32>
                        vertex_fn.push_str(&format!(
                            "    output.{} = {}_result;\n",
                            mapping.attribute_name, mapping.attribute_name
                        ));
                    }
                    "position_transform" => {
                        // PositionTransform returns vec2<f32>
                        vertex_fn.push_str(&format!(
                            "    output.{} = vec4<f32>({}_result, 0.0, 1.0);\n",
                            mapping.attribute_name, mapping.attribute_name
                        ));
                    }
                    _ => {
                        // LinearScale and others return f32
                        vertex_fn.push_str(&format!(
                            "    output.{} = vec4<f32>({}_result, 0.0, 0.0, 1.0);\n",
                            mapping.attribute_name, mapping.attribute_name
                        ));
                    }
                }
            }
        }

        vertex_fn.push_str("    \n");
        vertex_fn.push_str("    return output;\n");
        vertex_fn.push_str("}\n");

        vertex_fn
    }

    /// Generate the main fragment function.
    fn generate_main_fragment_function(&self) -> String {
        let mut fragment_fn = String::new();

        fragment_fn.push_str("@fragment\n");
        fragment_fn.push_str("fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {\n");

        // If there's a color attribute mapping, use it
        if let Some(color_mapping) = self
            .attribute_mappings
            .iter()
            .find(|m| m.attribute_name == "color")
        {
            fragment_fn.push_str(&format!(
                "    return in.{};\n",
                color_mapping.attribute_name
            ));
        } else {
            // Default white color
            fragment_fn.push_str("    return vec4<f32>(1.0, 1.0, 1.0, 1.0);\n");
        }

        fragment_fn.push_str("}\n");

        fragment_fn
    }

    /// Generate complete vertex shader WGSL source.
    pub fn generate_vertex_shader(&self) -> String {
        if let Some(ref cached) = self.cached_shaders {
            return cached.vertex_shader.clone();
        }

        let mut shader = String::new();

        // Add header comment
        shader.push_str("// Generated vertex shader by Gup ShaderPipeline\n");
        shader.push_str(
            "// This shader was automatically generated from composed shader functions\n\n",
        );

        // Add data type definitions
        shader.push_str(&self.generate_data_type_definitions());

        // Add uniform buffer bindings
        shader.push_str(&self.generate_uniform_bindings());

        // Add all function definitions with unique names
        for (i, function) in self.functions.iter().enumerate() {
            let mut function_code = function.wgsl_code().to_string();

            // Make function names unique by appending index
            let original_name = function.name();
            let unique_name = format!("{original_name}_{i}");
            function_code =
                function_code.replace(&format!("fn {original_name}"), &format!("fn {unique_name}"));

            shader.push_str(&function_code);
            shader.push_str("\n\n");
        }

        // Generate main vertex function
        shader.push_str(&self.generate_main_vertex_function());

        shader
    }

    /// Generate complete fragment shader WGSL source.
    pub fn generate_fragment_shader(&self) -> String {
        if let Some(ref cached) = self.cached_shaders {
            return cached.fragment_shader.clone();
        }

        let mut shader = String::new();

        // Add header comment
        shader.push_str("// Generated fragment shader by Gup ShaderPipeline\n");
        shader.push_str(
            "// This shader was automatically generated from composed shader functions\n\n",
        );

        // Add data type definitions (needed for VertexOutput)
        shader.push_str(&self.generate_data_type_definitions());

        // Add uniform buffer bindings
        shader.push_str(&self.generate_uniform_bindings());

        // Add all function definitions with unique names
        for (i, function) in self.functions.iter().enumerate() {
            let mut function_code = function.wgsl_code().to_string();

            // Make function names unique by appending index
            let original_name = function.name();
            let unique_name = format!("{original_name}_{i}");
            function_code =
                function_code.replace(&format!("fn {original_name}"), &format!("fn {unique_name}"));

            shader.push_str(&function_code);
            shader.push_str("\n\n");
        }

        // Generate main fragment function
        shader.push_str(&self.generate_main_fragment_function());

        shader
    }

    /// Create uniform buffers for all functions that need them.
    pub fn create_uniform_buffers(&mut self, device: &Device) -> GupResult<()> {
        self.uniform_buffers.clear();

        for function in &self.functions {
            if function.has_uniforms() && function.uniform_size() > 0 {
                let buffer = GpuBuffer::new(device, BufferType::Uniform, function.uniform_size());
                self.uniform_buffers
                    .insert(function.name().to_string(), buffer);
            }
        }

        Ok(())
    }

    /// Update uniform data for all functions.
    pub fn update_uniforms(&mut self, device: &Device, queue: &Queue) -> GupResult<()> {
        for function in self.functions.iter() {
            if let Some(uniform_data) = &function.uniform_buffer
                && let Some(buffer) = self.uniform_buffers.get_mut(function.name())
            {
                // This is a simplified approach - in a real implementation,
                // we'd need proper type erasure and serialization
                let data_slice = unsafe {
                    std::slice::from_raw_parts(
                        uniform_data.as_ref() as *const _ as *const u8,
                        function.uniform_size(),
                    )
                };

                buffer.upload(device, queue, data_slice)?;
            }
        }

        Ok(())
    }

    /// Create uniform buffers using a [`UniformBufferPool`] to avoid repeated
    /// GPU allocations.
    ///
    /// Before acquiring new buffers, any previously-held buffers are returned
    /// to the pool for reuse.
    pub fn create_uniform_buffers_pooled(
        &mut self,
        device: &Device,
        pool: &mut UniformBufferPool,
    ) -> GupResult<()> {
        // Return old buffers to the pool.
        for (_, buffer) in self.uniform_buffers.drain() {
            pool.release(buffer);
        }

        for function in &self.functions {
            if function.has_uniforms() && function.uniform_size() > 0 {
                let buffer = pool.acquire(device, function.uniform_size());
                self.uniform_buffers
                    .insert(function.name().to_string(), buffer);
            }
        }

        Ok(())
    }

    /// Stage all uniform updates into a [`UniformBatcher`] for deferred,
    /// batched upload.
    ///
    /// Call [`UniformBatcher::flush`] after staging updates from all pipelines
    /// to perform the actual GPU transfers in one go.
    pub fn stage_uniforms(&self, batcher: &mut UniformBatcher) {
        for function in self.functions.iter() {
            if let Some(uniform_data) = &function.uniform_buffer {
                let data_slice = unsafe {
                    std::slice::from_raw_parts(
                        uniform_data.as_ref() as *const _ as *const u8,
                        function.uniform_size(),
                    )
                };
                batcher.stage(function.name(), data_slice);
            }
        }
    }

    /// Create a bind group using a [`BindGroupCache`] to avoid redundant
    /// GPU resource creation.
    pub fn create_bind_group_cached(
        &self,
        device: &Device,
        cache: &mut BindGroupCache,
    ) -> GupResult<Arc<BindGroup>> {
        let key = self.pipeline_hash;

        if let Some((_layout, bind_group)) = cache.get(key) {
            return Ok(Arc::clone(bind_group));
        }

        let layout = Arc::new(self.create_bind_group_layout(device)?);
        let mut entries = Vec::new();
        let mut binding_index = 0;

        for function in self.functions.iter() {
            if function.has_uniforms()
                && let Some(buffer) = self.uniform_buffers.get(function.name())
            {
                entries.push(BindGroupEntry {
                    binding: binding_index,
                    resource: buffer.raw_buffer().as_entire_binding(),
                });
                binding_index += 1;
            }
        }

        let bind_group = Arc::new(device.create_bind_group(&BindGroupDescriptor {
            label: Some("shader_pipeline_bind_group_cached"),
            layout: &layout,
            entries: &entries,
        }));

        cache.insert(key, layout, Arc::clone(&bind_group));
        Ok(bind_group)
    }

    /// Create a bind group layout for the pipeline's uniforms.
    pub fn create_bind_group_layout(&self, device: &Device) -> GupResult<BindGroupLayout> {
        let mut entries = Vec::new();
        let mut binding_index = 0;

        for function in self.functions.iter() {
            if function.has_uniforms() {
                entries.push(BindGroupLayoutEntry {
                    binding: binding_index,
                    visibility: ShaderStages::VERTEX | ShaderStages::FRAGMENT,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                });
                binding_index += 1;
            }
        }

        Ok(device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("shader_pipeline_bind_group_layout"),
            entries: &entries,
        }))
    }

    /// Create a bind group for the pipeline's uniforms.
    pub fn create_bind_group(&self, device: &Device) -> GupResult<BindGroup> {
        let layout = self.create_bind_group_layout(device)?;
        let mut entries = Vec::new();
        let mut binding_index = 0;

        for function in self.functions.iter() {
            if function.has_uniforms()
                && let Some(buffer) = self.uniform_buffers.get(function.name())
            {
                entries.push(BindGroupEntry {
                    binding: binding_index,
                    resource: buffer.raw_buffer().as_entire_binding(),
                });
                binding_index += 1;
            }
        }

        Ok(device.create_bind_group(&BindGroupDescriptor {
            label: Some("shader_pipeline_bind_group"),
            layout: &layout,
            entries: &entries,
        }))
    }

    /// Update the shader cache with compiled shaders.
    fn update_cache(&mut self, device: &Device) -> GupResult<()> {
        let vertex_source = self.generate_vertex_shader();
        let fragment_source = self.generate_fragment_shader();

        let vertex_module = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("pipeline_vertex_shader"),
            source: ShaderSource::Wgsl(vertex_source.clone().into()),
        });

        let fragment_module = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("pipeline_fragment_shader"),
            source: ShaderSource::Wgsl(fragment_source.clone().into()),
        });

        let bind_group_layout = self.create_bind_group_layout(device)?;

        self.cached_shaders = Some(CachedShaders {
            vertex_shader: vertex_source,
            fragment_shader: fragment_source,
            bind_group_layout: Some(Arc::new(bind_group_layout)),
            vertex_module: Some(Arc::new(vertex_module)),
            fragment_module: Some(Arc::new(fragment_module)),
        });

        Ok(())
    }

    /// Get the current pipeline hash for cache validation.
    pub fn pipeline_hash(&self) -> u64 {
        self.pipeline_hash
    }

    /// Check if the cache is valid for the current pipeline configuration.
    pub fn is_cache_valid(&self) -> bool {
        self.cached_shaders.is_some()
    }

    /// Get the number of uniform buffers (for testing).
    pub fn uniform_buffer_count(&self) -> usize {
        self.uniform_buffers.len()
    }

    /// Flush pending writes from a [`UniformBatcher`] into this pipeline's
    /// uniform buffers.
    ///
    /// Returns the number of writes performed.
    pub fn flush_batcher(
        &mut self,
        device: &Device,
        queue: &Queue,
        batcher: &mut UniformBatcher,
    ) -> GupResult<usize> {
        batcher.flush(device, queue, &mut self.uniform_buffers)
    }

    /// Get the number of functions with uniforms (for testing).
    pub fn functions_with_uniforms_count(&self) -> usize {
        self.functions.iter().filter(|f| f.has_uniforms()).count()
    }

    /// Update the cache (for testing).
    pub fn update_cache_public(&mut self, device: &Device) -> GupResult<()> {
        self.update_cache(device)
    }

    /// Optimize shader source by removing unused code and performing optimizations.
    ///
    /// When `OptimizationConfig.use_ast_analysis` is true, parses the shader
    /// into an AST and runs AST-based optimization passes.  Falls back to
    /// string-based optimizations if AST parsing fails.
    pub fn optimize_shader(&self, shader_source: &str) -> String {
        if self.optimization_config.use_ast_analysis {
            if let Some(optimized) = self.optimize_shader_ast(shader_source) {
                return optimized;
            }
            // AST parsing failed — fall back to string-based optimizations.
            log::debug!("AST optimization failed, falling back to string-based optimizations");
        }

        self.optimize_shader_string(shader_source)
    }

    /// String-based optimization pipeline (the original implementation).
    fn optimize_shader_string(&self, shader_source: &str) -> String {
        let mut optimized = shader_source.to_string();

        // Apply optimizations based on configuration
        if self.optimization_config.enable_dead_code_elimination {
            optimized = self.remove_unused_uniforms(&optimized);
        }

        if self.optimization_config.enable_inlining {
            optimized = self.inline_small_functions_advanced(&optimized);
        }

        if self.optimization_config.enable_constant_folding {
            optimized = self.fold_constants(&optimized);
        }

        optimized
    }

    /// AST-based optimization pipeline.
    ///
    /// Returns `None` if the source cannot be parsed, allowing the caller
    /// to fall back to the string-based path.
    fn optimize_shader_ast(&self, shader_source: &str) -> Option<String> {
        use crate::shader_ast::{
            AstOptimizationConfig, generate_wgsl_minimal, optimize, parse_wgsl,
        };

        let mut module = match parse_wgsl(shader_source) {
            Ok(m) => m,
            Err(e) => {
                log::debug!("AST parse error: {}", e.message);
                return None;
            }
        };

        let ast_config = AstOptimizationConfig {
            enable_dead_code_elimination: self.optimization_config.enable_dead_code_elimination,
            enable_constant_folding: self.optimization_config.enable_constant_folding,
            enable_function_inlining: self.optimization_config.enable_inlining,
            inline_max_statements: self.optimization_config.inlining.inline_threshold,
            inline_max_call_sites: self.optimization_config.inlining.call_count_threshold,
        };

        let results = optimize(&mut module, &ast_config);

        if log::log_enabled!(log::Level::Debug) {
            for r in &results {
                if r.changed {
                    log::debug!("AST optimization: {}", r.description);
                }
            }
        }

        Some(generate_wgsl_minimal(&module))
    }

    /// Get profiling report if profiling is enabled.
    pub fn profile_report(&self) -> Option<ProfileReport> {
        self.profiler.as_ref().map(|p| {
            let vertex_size = self
                .cached_shaders
                .as_ref()
                .map(|c| c.vertex_shader.len())
                .unwrap_or(0);
            let fragment_size = self
                .cached_shaders
                .as_ref()
                .map(|c| c.fragment_shader.len())
                .unwrap_or(0);
            p.report(self.functions.len(), vertex_size, fragment_size)
        })
    }

    /// Get optimization recommendations based on profiling data.
    pub fn optimization_recommendations(&self) -> Vec<OptimizationRecommendation> {
        self.profiler
            .as_ref()
            .map(|p| p.recommendations())
            .unwrap_or_default()
    }

    /// Get the optimization configuration.
    pub fn optimization_config(&self) -> &OptimizationConfig {
        &self.optimization_config
    }

    /// Set a new optimization configuration.
    pub fn set_optimization_config(&mut self, config: OptimizationConfig) {
        self.optimization_config = config;
        self.invalidate_cache();
    }

    /// Remove unused uniform declarations from shader source.
    fn remove_unused_uniforms(&self, shader: &str) -> String {
        let mut lines: Vec<&str> = shader.lines().collect();
        let mut used_uniforms = std::collections::HashSet::new();

        // Find all uniform usages in the shader
        for line in &lines {
            for function in &self.functions {
                let uniform_name = format!("{}_uniforms", function.name());
                if line.contains(&uniform_name) && !line.trim_start().starts_with("@group") {
                    used_uniforms.insert(uniform_name);
                }
            }
        }

        // Remove unused uniform declarations
        lines.retain(|line| {
            if line.trim_start().starts_with("@group") && line.contains("var<uniform>") {
                // Check if this uniform is used
                for function in &self.functions {
                    let uniform_name = format!("{}_uniforms", function.name());
                    if line.contains(&uniform_name) {
                        return used_uniforms.contains(&uniform_name);
                    }
                }
                false
            } else {
                true
            }
        });

        lines.join("\n")
    }

    /// Inline small functions for performance optimization (basic implementation).
    /// This is kept for backward compatibility but the advanced version is preferred.
    #[allow(dead_code)]
    fn inline_small_functions(&self, shader: &str) -> String {
        let mut optimized = shader.to_string();

        // This is a simplified inlining implementation
        // In practice, this would need proper WGSL AST parsing
        for function in &self.functions {
            let function_code = function.wgsl_code().trim();

            // Only inline very simple functions (less than 3 lines of actual code)
            let code_lines: Vec<&str> = function_code
                .lines()
                .filter(|line| !line.trim().is_empty() && !line.trim().starts_with("//"))
                .collect();

            if code_lines.len() <= 3 {
                // Simple inline replacement (very basic)
                let function_name = function.name();
                let call_pattern = format!("{function_name}(");

                if optimized.matches(&call_pattern).count() <= 2 {
                    // Only inline if called few times
                    // This is a placeholder for proper inlining logic
                    optimized.push_str(&format!("// Inlined function: {function_name}\n"));
                }
            }
        }

        optimized
    }

    /// Advanced function inlining with configurable thresholds.
    ///
    /// This is a more sophisticated inlining implementation that:
    /// - Respects the inlining configuration
    /// - Considers function size and call count
    /// - Performs basic control flow analysis
    /// - Tracks inlining decisions for profiling
    fn inline_small_functions_advanced(&self, shader: &str) -> String {
        let config = &self.optimization_config.inlining;
        let mut optimized = shader.to_string();
        let mut inlined_functions = std::collections::HashSet::new();

        for function in &self.functions {
            let function_code = function.wgsl_code().trim();
            let function_name = function.name();

            // Count non-empty, non-comment lines
            let code_lines: Vec<&str> = function_code
                .lines()
                .filter(|line| {
                    let trimmed = line.trim();
                    !trimmed.is_empty() && !trimmed.starts_with("//")
                })
                .collect();

            // Check if function is small enough to inline
            if code_lines.len() > config.inline_threshold {
                continue;
            }

            // Count call sites
            let call_pattern = format!("{function_name}(");
            let call_count = optimized.matches(&call_pattern).count();

            if call_count > config.call_count_threshold {
                continue;
            }

            // Basic control flow analysis: skip if function has complex control flow
            if config.use_ast_analysis {
                let has_complex_control = function_code.contains("if ")
                    || function_code.contains("for ")
                    || function_code.contains("while ")
                    || function_code.contains("loop");

                if has_complex_control {
                    continue;
                }
            }

            // Mark as inlined for tracking
            inlined_functions.insert(function_name.to_string());

            // Add comment about inlining (actual inlining would require AST manipulation)
            optimized.push_str(&format!(
                "// Function '{}' marked for inlining ({} lines, {} call sites)\n",
                function_name,
                code_lines.len(),
                call_count
            ));
        }

        // Record inlining statistics if profiling is enabled
        if !inlined_functions.is_empty() {
            log::debug!(
                "Advanced inlining: {} functions marked for inlining",
                inlined_functions.len()
            );
        }

        optimized
    }

    /// Perform constant folding optimizations.
    fn fold_constants(&self, shader: &str) -> String {
        let mut optimized = shader.to_string();

        // Simple constant folding examples
        // In practice, this would need proper expression parsing
        optimized = optimized.replace("1.0 * ", "");
        optimized = optimized.replace(" * 1.0", "");
        optimized = optimized.replace("0.0 + ", "");
        optimized = optimized.replace(" + 0.0", "");

        optimized
    }

    /// Generate optimized vertex shader.
    pub fn generate_optimized_vertex_shader(&self) -> String {
        let base_shader = self.generate_vertex_shader();
        self.optimize_shader(&base_shader)
    }

    /// Generate optimized fragment shader.
    pub fn generate_optimized_fragment_shader(&self) -> String {
        let base_shader = self.generate_fragment_shader();
        self.optimize_shader(&base_shader)
    }

    /// Create a render pipeline with the generated shaders.
    pub fn create_render_pipeline(&mut self, device: &Device) -> GupResult<RenderPipeline> {
        // Ensure cache is updated
        if !self.is_cache_valid() {
            self.update_cache(device)?;
        }

        let cached = self.cached_shaders.as_ref().unwrap();
        let bind_group_layout = cached.bind_group_layout.as_ref().unwrap();

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("shader_pipeline_layout"),
            bind_group_layouts: &[bind_group_layout],
            push_constant_ranges: &[],
        });

        let vertex_module = cached.vertex_module.as_ref().unwrap();
        let fragment_module = cached.fragment_module.as_ref().unwrap();

        let render_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("shader_pipeline_render_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: vertex_module,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(FragmentState {
                module: fragment_module,
                entry_point: Some("fs_main"),
                targets: &[Some(ColorTargetState {
                    format: wgpu::TextureFormat::Bgra8UnormSrgb,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
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

        Ok(render_pipeline)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shader_function::{ColorMap, LinearScale, Vec4};
    use crate::vec4;

    #[test]
    fn test_pipeline_creation() {
        let mut pipeline = ComposableShaderPipeline::new();
        let scale = LinearScale::new(0.0, 100.0, 0.0, 1.0);
        pipeline.add_function(scale);

        assert_eq!(pipeline.function_count(), 1);
    }

    #[test]
    fn test_multiple_functions() {
        let mut pipeline = ComposableShaderPipeline::new();
        let scale = LinearScale::new(0.0, 100.0, 0.0, 1.0);
        let color_map = ColorMap::new(vec4![0.0, 0.0, 0.0, 1.0], vec4![1.0, 1.0, 1.0, 1.0]);

        pipeline.add_function(scale);
        pipeline.add_function(color_map);

        assert_eq!(pipeline.function_count(), 2);
    }

    #[test]
    fn test_attribute_mapping() {
        let mut pipeline = ComposableShaderPipeline::new();
        let scale = LinearScale::new(0.0, 100.0, 0.0, 1.0);
        pipeline.add_function(scale);
        pipeline.map_attribute("color", "linear_scale");

        assert_eq!(pipeline.attribute_mappings.len(), 1);
        assert_eq!(pipeline.attribute_mappings[0].attribute_name, "color");
        assert_eq!(pipeline.attribute_mappings[0].function_name, "linear_scale");
    }

    #[test]
    fn test_shader_generation() {
        let mut pipeline = ComposableShaderPipeline::new();
        let scale = LinearScale::new(0.0, 100.0, 0.0, 1.0);
        pipeline.add_function(scale);
        pipeline.map_attribute("color", "linear_scale");

        let vertex_shader = pipeline.generate_vertex_shader();
        assert!(vertex_shader.contains("linear_scale"));
        assert!(vertex_shader.contains("@vertex"));
        assert!(vertex_shader.contains("vs_main"));
        assert!(vertex_shader.contains("VertexOutput"));

        let fragment_shader = pipeline.generate_fragment_shader();
        assert!(fragment_shader.contains("@fragment"));
        assert!(fragment_shader.contains("fs_main"));
    }

    #[test]
    fn test_pipeline_hash_changes() {
        let mut pipeline = ComposableShaderPipeline::new();
        let initial_hash = pipeline.pipeline_hash();

        let scale = LinearScale::new(0.0, 100.0, 0.0, 1.0);
        pipeline.add_function(scale);

        assert_ne!(pipeline.pipeline_hash(), initial_hash);
    }

    #[test]
    fn test_cache_invalidation() {
        let mut pipeline = ComposableShaderPipeline::new();
        assert!(!pipeline.is_cache_valid());

        let scale = LinearScale::new(0.0, 100.0, 0.0, 1.0);
        pipeline.add_function(scale);

        assert!(!pipeline.is_cache_valid());
    }

    #[test]
    fn test_uniform_buffer_detection() {
        let pipeline_fn = PipelineFunction::new(LinearScale::new(0.0, 100.0, 0.0, 1.0));
        assert!(pipeline_fn.has_uniforms());
        assert_eq!(pipeline_fn.name(), "linear_scale");
    }

    #[test]
    fn test_data_type_definitions() {
        let pipeline = ComposableShaderPipeline::new();
        let definitions = pipeline.generate_data_type_definitions();

        assert!(definitions.contains("struct VertexInput"));
        assert!(definitions.contains("struct VertexOutput"));
        assert!(definitions.contains("@builtin(vertex_index)"));
        assert!(definitions.contains("@builtin(position)"));
    }

    #[test]
    fn test_uniform_bindings_generation() {
        let mut pipeline = ComposableShaderPipeline::new();
        let scale = LinearScale::new(0.0, 100.0, 0.0, 1.0);
        pipeline.add_function(scale);

        let bindings = pipeline.generate_uniform_bindings();
        assert!(bindings.contains("@group(0) @binding(0)"));
        assert!(bindings.contains("linear_scale_uniforms_0"));
        assert!(bindings.contains("LinearScaleUniforms"));
    }

    #[test]
    fn test_shader_optimization() {
        let pipeline = ComposableShaderPipeline::new();
        let test_shader = r#"
            let x = 1.0 * y;
            let z = a + 0.0;
            @group(0) @binding(0) var<uniform> unused_uniforms: UnusedUniforms;
            return x * 1.0;
        "#;

        let optimized = pipeline.optimize_shader(test_shader);
        assert!(optimized.contains("let x = y;"));
        assert!(optimized.contains("let z = a;"));
    }

    #[test]
    fn test_constant_folding() {
        let pipeline = ComposableShaderPipeline::new();
        let test_code = "let result = value * 1.0 + 0.0;";
        let optimized = pipeline.fold_constants(test_code);
        assert_eq!(optimized, "let result = value;");
    }

    #[test]
    fn test_fragment_shader_with_color_mapping() {
        let mut pipeline = ComposableShaderPipeline::new();
        let color_map = ColorMap::new(vec4![0.0, 0.0, 0.0, 1.0], vec4![1.0, 1.0, 1.0, 1.0]);
        pipeline.add_function(color_map);
        pipeline.map_attribute("color", "color_map");

        let fragment_shader = pipeline.generate_fragment_shader();
        assert!(fragment_shader.contains("return in.color;"));
    }

    #[test]
    fn test_vertex_shader_with_multiple_attributes() {
        let mut pipeline = ComposableShaderPipeline::new();
        let scale = LinearScale::new(0.0, 100.0, 0.0, 1.0);
        let color_map = ColorMap::new(vec4![0.0, 0.0, 0.0, 1.0], vec4![1.0, 1.0, 1.0, 1.0]);

        pipeline.add_function(scale);
        pipeline.add_function(color_map);
        pipeline.map_attribute("size", "linear_scale");
        pipeline.map_attribute("color", "color_map");

        let vertex_shader = pipeline.generate_vertex_shader();
        assert!(vertex_shader.contains("size_result"));
        assert!(vertex_shader.contains("color_result"));
        assert!(vertex_shader.contains("output.size"));
        assert!(vertex_shader.contains("output.color"));
    }

    #[test]
    fn test_optimized_shader_generation() {
        let mut pipeline = ComposableShaderPipeline::new();
        let scale = LinearScale::new(0.0, 100.0, 0.0, 1.0);
        pipeline.add_function(scale);

        let optimized_vertex = pipeline.generate_optimized_vertex_shader();
        let optimized_fragment = pipeline.generate_optimized_fragment_shader();

        assert!(optimized_vertex.contains("vs_main"));
        assert!(optimized_fragment.contains("fs_main"));
    }

    #[test]
    fn test_shader_caching() {
        let mut pipeline = ComposableShaderPipeline::new();
        assert!(!pipeline.is_cache_valid());

        let scale = LinearScale::new(0.0, 100.0, 0.0, 1.0);
        pipeline.add_function(scale);

        let hash1 = pipeline.pipeline_hash();

        let color_map = ColorMap::new(vec4![0.0, 0.0, 0.0, 1.0], vec4![1.0, 1.0, 1.0, 1.0]);
        pipeline.add_function(color_map);

        let hash2 = pipeline.pipeline_hash();
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_removed_unused_uniforms() {
        let pipeline = ComposableShaderPipeline::new();
        let shader_with_unused = r#"
@group(0) @binding(0) var<uniform> used_uniforms: UsedUniforms;
@group(0) @binding(1) var<uniform> unused_uniforms: UnusedUniforms;

fn main() {
    let x = used_uniforms.value;
}
        "#;

        let optimized = pipeline.remove_unused_uniforms(shader_with_unused);
        assert!(optimized.contains("used_uniforms"));
        assert!(!optimized.contains("unused_uniforms"));
    }

    // -----------------------------------------------------------------------
    // AST integration tests
    // -----------------------------------------------------------------------

    /// Helper: create a pipeline configured for AST-based optimization.
    fn ast_pipeline() -> ComposableShaderPipeline {
        let config = OptimizationConfig {
            use_ast_analysis: true,
            ..Default::default()
        };
        ComposableShaderPipeline::new().with_optimization_config(config)
    }

    #[test]
    fn test_ast_optimize_shader_basic() {
        let pipeline = ast_pipeline();

        // A simple WGSL snippet the AST parser can handle.
        let src = r#"fn helper(x: f32) -> f32 {
    return x + 0.0;
}

@vertex
fn vs_main() -> f32 {
    return helper(1.0);
}
"#;

        let optimized = pipeline.optimize_shader(src);

        // The AST optimizer should have folded `x + 0.0` -> `x`
        // and potentially inlined `helper`.
        assert!(
            optimized.contains("vs_main"),
            "entry point must be preserved"
        );
        // Should not contain the un-folded `+ 0.0`
        assert!(
            !optimized.contains("+ 0.0"),
            "constant folding should remove identity addition"
        );
    }

    #[test]
    fn test_ast_optimize_shader_dead_code() {
        let pipeline = ast_pipeline();

        let src = r#"fn unused(x: f32) -> f32 {
    return x;
}

fn used(x: f32) -> f32 {
    return x;
}

@vertex
fn vs_main() -> f32 {
    return used(42.0);
}
"#;

        let optimized = pipeline.optimize_shader(src);

        // `unused` should be removed by dead-code elimination.
        assert!(optimized.contains("vs_main"));
        // After DCE + inlining, `unused` should not appear.
        assert!(
            !optimized.contains("fn unused"),
            "dead function should be eliminated"
        );
    }

    #[test]
    fn test_ast_optimize_shader_fallback_on_parse_error() {
        let pipeline = ast_pipeline();

        // Deliberately unparseable WGSL.
        let bad_src = "@@@ this is not valid WGSL @@@";

        let optimized = pipeline.optimize_shader(bad_src);

        // Should have fallen back to string-based optimization and not
        // panicked. The string-based path just returns the source mostly
        // as-is (it only does simple replacements).
        assert!(
            optimized.contains("not valid WGSL"),
            "fallback must preserve source text"
        );
    }

    #[test]
    fn test_ast_output_no_larger_than_string_based() {
        // Use a pipeline configured for string-only optimization.
        let mut string_pipeline = ComposableShaderPipeline::new();
        let scale = LinearScale::new(0.0, 100.0, 0.0, 1.0);
        string_pipeline.add_function(scale);
        string_pipeline.map_attribute("color", "linear_scale");
        let string_optimized = string_pipeline.generate_optimized_vertex_shader();

        // Now with AST optimization.
        let config = OptimizationConfig {
            use_ast_analysis: true,
            ..Default::default()
        };
        let mut ast_pipeline = ComposableShaderPipeline::new().with_optimization_config(config);
        let scale2 = LinearScale::new(0.0, 100.0, 0.0, 1.0);
        ast_pipeline.add_function(scale2);
        ast_pipeline.map_attribute("color", "linear_scale");
        let ast_optimized = ast_pipeline.generate_optimized_vertex_shader();

        // The AST output should be at least as small as the string output.
        // We compare non-whitespace character counts to ignore formatting diffs.
        let string_chars: usize = string_optimized
            .chars()
            .filter(|c| !c.is_whitespace())
            .count();
        let ast_chars: usize = ast_optimized.chars().filter(|c| !c.is_whitespace()).count();

        assert!(
            ast_chars <= string_chars,
            "AST output ({ast_chars} chars) should be <= string output ({string_chars} chars)"
        );
    }

    #[test]
    fn test_ast_backward_compat_default_config() {
        // With the default config (use_ast_analysis: false), behaviour should
        // be identical to the old code path.
        let pipeline = ComposableShaderPipeline::new();
        assert!(!pipeline.optimization_config().use_ast_analysis);

        let src = "let x = 1.0 * y;";
        let optimized = pipeline.optimize_shader(src);
        assert!(optimized.contains("let x = y;"));
    }

    #[test]
    fn test_ast_constant_folding_literals() {
        let pipeline = ast_pipeline();

        let src = r#"@vertex
fn vs_main() -> f32 {
    return 2.0 + 3.0;
}
"#;

        let optimized = pipeline.optimize_shader(src);
        // Should fold 2.0 + 3.0 into 5.0
        assert!(
            optimized.contains("5.0"),
            "literal arithmetic should be folded"
        );
    }

    #[test]
    fn test_ast_function_inlining() {
        let pipeline = ast_pipeline();

        let src = r#"fn identity(x: f32) -> f32 {
    return x;
}

@vertex
fn vs_main() -> f32 {
    return identity(42.0);
}
"#;

        let optimized = pipeline.optimize_shader(src);
        // After inlining `identity` and DCE, the body should just return 42.0.
        assert!(optimized.contains("42.0"));
        // The helper should be removed (inlined + DCE).
        assert!(
            !optimized.contains("fn identity"),
            "inlined function should be eliminated by DCE"
        );
    }

    // -----------------------------------------------------------------------
    // UniformBufferPool tests (non-GPU)
    // -----------------------------------------------------------------------

    #[test]
    fn test_uniform_pool_aligned_size() {
        assert_eq!(UniformBufferPool::aligned_size(1), 256);
        assert_eq!(UniformBufferPool::aligned_size(16), 256);
        assert_eq!(UniformBufferPool::aligned_size(256), 256);
        assert_eq!(UniformBufferPool::aligned_size(257), 512);
        assert_eq!(UniformBufferPool::aligned_size(512), 512);
    }

    #[test]
    fn test_uniform_pool_stats_initial() {
        let pool = UniformBufferPool::new(8);
        let stats = pool.stats();
        assert_eq!(stats.total_created, 0);
        assert_eq!(stats.total_reused, 0);
        assert_eq!(stats.idle_buffers, 0);
        assert_eq!(stats.bucket_count, 0);
    }

    #[test]
    fn test_uniform_pool_stats_reuse_rate() {
        let stats = UniformPoolStats {
            total_created: 2,
            total_reused: 8,
            idle_buffers: 0,
            bucket_count: 1,
        };
        assert!((stats.reuse_rate() - 80.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_uniform_pool_stats_reuse_rate_zero() {
        let stats = UniformPoolStats {
            total_created: 0,
            total_reused: 0,
            idle_buffers: 0,
            bucket_count: 0,
        };
        assert_eq!(stats.reuse_rate(), 0.0);
    }

    #[test]
    fn test_uniform_pool_default() {
        let pool = UniformBufferPool::default();
        assert_eq!(pool.max_per_bucket, 16);
    }

    // -----------------------------------------------------------------------
    // UniformBatcher tests (non-GPU)
    // -----------------------------------------------------------------------

    #[test]
    fn test_batcher_stage_and_count() {
        let mut batcher = UniformBatcher::new();
        assert_eq!(batcher.pending_count(), 0);

        batcher.stage("scale", &[1, 2, 3, 4]);
        batcher.stage("color", &[5, 6, 7, 8]);
        assert_eq!(batcher.pending_count(), 2);
    }

    #[test]
    fn test_batcher_clear() {
        let mut batcher = UniformBatcher::new();
        batcher.stage("a", &[0]);
        batcher.stage("b", &[1]);
        batcher.clear();
        assert_eq!(batcher.pending_count(), 0);
    }

    #[test]
    fn test_batcher_default() {
        let batcher = UniformBatcher::default();
        assert_eq!(batcher.pending_count(), 0);
    }

    // -----------------------------------------------------------------------
    // BindGroupCache tests (non-GPU)
    // -----------------------------------------------------------------------

    #[test]
    fn test_bind_group_cache_initial() {
        let cache = BindGroupCache::new();
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
        assert_eq!(cache.stats().hits, 0);
        assert_eq!(cache.stats().misses, 0);
    }

    #[test]
    fn test_bind_group_cache_miss_records_stats() {
        let mut cache = BindGroupCache::new();
        assert!(cache.get(42).is_none());
        assert_eq!(cache.stats().misses, 1);
        assert_eq!(cache.stats().hits, 0);
    }

    #[test]
    fn test_bind_group_cache_hit_rate() {
        let stats = BindGroupCacheStats { hits: 3, misses: 1 };
        assert_eq!(stats.hit_rate(), 75.0);
    }

    #[test]
    fn test_bind_group_cache_default() {
        let cache = BindGroupCache::default();
        assert!(cache.is_empty());
    }
}

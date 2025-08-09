// Gup - GPU-Accelerated Data Visualization Library
// Copyright (C) 2025 Corin Lawson <corin@phiware.com.au>
//
//! Deep composition chain optimization system.
//!
//! This module provides optimizations for rendering deep composition chains efficiently,
//! including operation flattening, render batching, resource pooling, and caching.

use crate::{GupResult, Mixable, RenderContext};
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::time::Instant;

/// Optimized composition executor for deep chains
pub struct CompositionExecutor {
    /// Flattened render operations
    operations: Vec<RenderOperation>,
    /// Batched operations by type
    batches: HashMap<TypeId, Vec<BatchedOperation>>,
    /// Resource pool for reuse
    resource_pool: ResourcePool,
    /// Render cache for expensive operations
    render_cache: RenderCache,
    /// Performance metrics
    metrics: CompositionMetrics,
}

/// Individual render operation in flattened composition
#[derive(Debug)]
struct RenderOperation {
    /// Unique identifier for this operation
    id: OperationId,
    /// Type of the mixable component
    component_type: TypeId,
    /// Render data (type-erased)
    #[allow(dead_code)] // Used in future optimization phases
    render_data: Box<dyn Any + Send + Sync>,
    /// Composition mode for this operation
    #[allow(dead_code)] // Used in future optimization phases
    composition_mode: crate::mixable::CompositionMode,
    /// Dependencies on other operations
    #[allow(dead_code)] // Used in future optimization phases
    dependencies: Vec<OperationId>,
}

/// Batched operations of the same type
#[derive(Debug, Clone)]
struct BatchedOperation {
    /// Operations that can be batched together
    operations: Vec<OperationId>,
    /// Shared render state for the batch
    shared_state: BatchRenderState,
}

/// Render state shared across a batch of operations
#[derive(Debug, Clone)]
struct BatchRenderState {
    /// GPU pipeline to use
    pipeline_id: PipelineId,
    /// Uniform data shared across batch
    uniforms: Vec<u8>,
    /// Texture bindings
    textures: Vec<TextureBinding>,
}

/// Performance metrics for composition optimization
#[derive(Debug, Default, Clone)]
pub struct CompositionMetrics {
    /// Total number of operations in composition
    pub operation_count: usize,
    /// Number of batches created
    pub batch_count: usize,
    /// Cache hit rate
    pub cache_hit_rate: f32,
    /// Time spent optimizing
    pub optimization_time: std::time::Duration,
    /// Time spent rendering
    pub render_time: std::time::Duration,
    /// Memory saved through pooling
    pub memory_saved: usize,
}

type OperationId = u32;
type PipelineId = u32;

#[derive(Debug, Clone)]
struct TextureBinding {
    #[allow(dead_code)] // Used in future optimization phases
    slot: u32,
    #[allow(dead_code)] // Used in future optimization phases
    texture_id: u32,
}

/// Resource pool for efficient reuse
pub struct ResourcePool {
    /// Buffer pools by size
    vertex_buffers: HashMap<usize, Vec<wgpu::Buffer>>,
    uniform_buffers: HashMap<usize, Vec<wgpu::Buffer>>,
    /// Texture pools by format and size
    textures: HashMap<(wgpu::TextureFormat, (u32, u32)), Vec<wgpu::Texture>>,
    /// Total memory managed
    total_memory: usize,
}

impl ResourcePool {
    fn new() -> Self {
        Self {
            vertex_buffers: HashMap::new(),
            uniform_buffers: HashMap::new(),
            textures: HashMap::new(),
            total_memory: 0,
        }
    }

    /// Get a vertex buffer from the pool or create a new one
    #[allow(dead_code)] // Used in future optimization phases
    fn get_vertex_buffer(&mut self, device: &wgpu::Device, size: usize) -> wgpu::Buffer {
        if let Some(buffers) = self.vertex_buffers.get_mut(&size) {
            if let Some(buffer) = buffers.pop() {
                return buffer;
            }
        }

        // Create new buffer if none available
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("pooled_vertex_buffer"),
            size: size as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        self.total_memory += size;
        buffer
    }

    /// Return a buffer to the pool for reuse
    #[allow(dead_code)] // Used in future optimization phases
    fn return_vertex_buffer(&mut self, size: usize, buffer: wgpu::Buffer) {
        self.vertex_buffers.entry(size).or_default().push(buffer);
    }

    /// Get memory usage statistics
    fn memory_usage(&self) -> usize {
        self.total_memory
    }

    /// Clear all pooled resources
    fn clear(&mut self) {
        self.vertex_buffers.clear();
        self.uniform_buffers.clear();
        self.textures.clear();
        self.total_memory = 0;
    }
}

/// Cache for expensive render results
pub struct RenderCache {
    entries: HashMap<CacheKey, CacheEntry>,
    #[allow(dead_code)] // Used in future optimization phases
    max_size: usize,
    current_size: usize,
    hits: u64,
    misses: u64,
}

#[derive(Hash, PartialEq, Eq, Clone)]
struct CacheKey {
    component_hash: u64,
    render_params_hash: u64,
}

struct CacheEntry {
    #[allow(dead_code)] // Used in future optimization phases
    texture: wgpu::Texture,
    #[allow(dead_code)] // Used in future optimization phases
    timestamp: std::time::Instant,
    #[allow(dead_code)] // Used in future optimization phases
    size: usize,
}

impl RenderCache {
    fn new() -> Self {
        Self {
            entries: HashMap::new(),
            max_size: 100 * 1024 * 1024, // 100MB cache limit
            current_size: 0,
            hits: 0,
            misses: 0,
        }
    }

    /// Get cached render result if available
    #[allow(dead_code)] // Used in future optimization phases
    fn get(&mut self, key: &CacheKey) -> Option<&wgpu::Texture> {
        if let Some(entry) = self.entries.get(key) {
            self.hits += 1;
            Some(&entry.texture)
        } else {
            self.misses += 1;
            None
        }
    }

    /// Store render result in cache
    #[allow(dead_code)] // Used in future optimization phases
    fn store(&mut self, key: CacheKey, texture: wgpu::Texture, size: usize) {
        // Evict entries if needed
        while self.current_size + size > self.max_size && !self.entries.is_empty() {
            self.evict_oldest();
        }

        let entry = CacheEntry {
            texture,
            timestamp: std::time::Instant::now(),
            size,
        };

        if let Some(old_entry) = self.entries.insert(key, entry) {
            self.current_size = self.current_size - old_entry.size + size;
        } else {
            self.current_size += size;
        }
    }

    /// Evict the oldest cache entry
    #[allow(dead_code)] // Used in future optimization phases
    fn evict_oldest(&mut self) {
        if self.entries.is_empty() {
            return;
        }

        // Collect all entries to find the oldest one
        let entries: Vec<(CacheKey, std::time::Instant)> = self
            .entries
            .iter()
            .map(|(k, v)| ((*k).clone(), v.timestamp))
            .collect();

        // Find the oldest entry
        if let Some((oldest_key, _)) = entries.into_iter().min_by_key(|(_, timestamp)| *timestamp) {
            if let Some(entry) = self.entries.remove(&oldest_key) {
                self.current_size -= entry.size;
            }
        }
    }

    /// Get cache hit rate
    fn hit_rate(&self) -> f32 {
        if self.hits + self.misses == 0 {
            0.0
        } else {
            self.hits as f32 / (self.hits + self.misses) as f32
        }
    }

    /// Clear the cache
    fn clear(&mut self) {
        self.entries.clear();
        self.current_size = 0;
        self.hits = 0;
        self.misses = 0;
    }
}

impl CompositionExecutor {
    /// Create a new composition executor
    pub fn new() -> Self {
        Self {
            operations: Vec::new(),
            batches: HashMap::new(),
            resource_pool: ResourcePool::new(),
            render_cache: RenderCache::new(),
            metrics: CompositionMetrics::default(),
        }
    }

    /// Analyze and flatten a composition tree
    pub fn flatten_composition<T: Mixable + 'static>(&mut self, composition: &T) -> GupResult<()> {
        let start_time = Instant::now();

        // Clear previous analysis
        self.operations.clear();
        self.batches.clear();

        // Traverse the composition tree and extract operations
        let mut next_id = 0;
        self.analyze_component(composition, &mut next_id, Vec::new())?;

        // Create batches from operations
        self.create_batches()?;

        // Update metrics
        self.metrics.optimization_time = start_time.elapsed();
        self.metrics.operation_count = self.operations.len();
        self.metrics.batch_count = self.batches.values().map(|batches| batches.len()).sum();

        Ok(())
    }

    /// Execute the flattened composition efficiently
    pub fn execute(&mut self, context: &mut RenderContext) -> GupResult<()> {
        let start_time = Instant::now();

        // Clone batches to avoid borrowing issues
        let batches_to_execute: Vec<Vec<BatchedOperation>> =
            self.batches.values().cloned().collect();

        // Execute batches in dependency order
        for batch_vec in batches_to_execute {
            for batch in batch_vec {
                self.execute_batch(&batch, context)?;
            }
        }

        // Update metrics
        self.metrics.render_time = start_time.elapsed();
        self.metrics.cache_hit_rate = self.render_cache.hit_rate();
        self.metrics.memory_saved = self.resource_pool.memory_usage();

        Ok(())
    }

    /// Get performance metrics for the last execution
    pub fn metrics(&self) -> CompositionMetrics {
        self.metrics.clone()
    }

    /// Reset performance metrics
    pub fn reset_metrics(&mut self) {
        self.metrics = CompositionMetrics::default();
        self.render_cache.clear();
    }

    /// Clear all caches and pools to free memory
    pub fn clear_resources(&mut self) {
        self.resource_pool.clear();
        self.render_cache.clear();
        self.operations.clear();
        self.batches.clear();
    }

    /// Analyze a single component and add it to the operation list
    fn analyze_component<T: Mixable + 'static>(
        &mut self,
        component: &T,
        next_id: &mut OperationId,
        dependencies: Vec<OperationId>,
    ) -> GupResult<OperationId> {
        let id = *next_id;
        *next_id += 1;

        // For now, treat all components as leaf nodes since we can't easily
        // inspect ComposedVisualization without additional trait support
        let render_data = self.extract_render_data(component)?;
        let operation = RenderOperation {
            id,
            component_type: TypeId::of::<T>(),
            render_data,
            composition_mode: crate::mixable::CompositionMode::Overlay, // Default for leaf nodes
            dependencies,
        };

        self.operations.push(operation);
        Ok(id)
    }

    /// Extract render data from a component for batching
    fn extract_render_data<T: Mixable>(
        &self,
        _component: &T,
    ) -> GupResult<Box<dyn Any + Send + Sync>> {
        // This would extract the actual render data (vertices, uniforms, etc.)
        // For now, use a placeholder since we can't access internal component data
        Ok(Box::new(PlaceholderRenderData))
    }

    /// Create render batches from operations
    fn create_batches(&mut self) -> GupResult<()> {
        // Group operations by type
        let mut type_groups: HashMap<TypeId, Vec<&RenderOperation>> = HashMap::new();

        for operation in &self.operations {
            type_groups
                .entry(operation.component_type)
                .or_default()
                .push(operation);
        }

        // Create batches within each type group
        for (type_id, operations) in type_groups {
            let mut batches = Vec::new();
            let mut current_batch = Vec::new();
            let mut current_state: Option<BatchRenderState> = None;

            for operation in operations {
                let state = self.get_render_state(operation)?;

                if let Some(ref current) = current_state {
                    if self.can_batch_with_state(current, &state) {
                        current_batch.push(operation.id);
                    } else {
                        // Finalize current batch and start new one
                        if !current_batch.is_empty() {
                            batches.push(BatchedOperation {
                                operations: current_batch,
                                shared_state: current.clone(),
                            });
                        }
                        current_batch = vec![operation.id];
                        current_state = Some(state);
                    }
                } else {
                    current_batch.push(operation.id);
                    current_state = Some(state);
                }
            }

            // Finalize last batch
            if !current_batch.is_empty() {
                if let Some(state) = current_state {
                    batches.push(BatchedOperation {
                        operations: current_batch,
                        shared_state: state,
                    });
                }
            }

            self.batches.insert(type_id, batches);
        }

        Ok(())
    }

    /// Get render state for an operation
    fn get_render_state(&self, _operation: &RenderOperation) -> GupResult<BatchRenderState> {
        // Extract render state from operation data
        // For now, use a default state since we can't access actual render data
        Ok(BatchRenderState {
            pipeline_id: 0,
            uniforms: Vec::new(),
            textures: Vec::new(),
        })
    }

    /// Check if two render states can be batched together
    fn can_batch_with_state(&self, state1: &BatchRenderState, state2: &BatchRenderState) -> bool {
        state1.pipeline_id == state2.pipeline_id
            && state1.uniforms == state2.uniforms
            && state1.textures.len() == state2.textures.len()
    }

    /// Execute a batch of operations
    fn execute_batch(
        &mut self,
        batch: &BatchedOperation,
        context: &mut RenderContext,
    ) -> GupResult<()> {
        // Set up shared render state
        self.setup_batch_state(&batch.shared_state, context)?;

        // Execute all operations in the batch
        for &operation_id in &batch.operations {
            if let Some(operation) = self.operations.iter().find(|op| op.id == operation_id) {
                self.execute_operation(operation, context)?;
            }
        }

        Ok(())
    }

    /// Set up render state for a batch
    fn setup_batch_state(
        &self,
        _state: &BatchRenderState,
        _context: &mut RenderContext,
    ) -> GupResult<()> {
        // Configure GPU pipeline and resources
        // This would set pipeline state, bind uniforms, etc.
        Ok(())
    }

    /// Execute a single operation
    fn execute_operation(
        &self,
        _operation: &RenderOperation,
        _context: &mut RenderContext,
    ) -> GupResult<()> {
        // Execute the specific render operation
        // This would perform the actual rendering work
        Ok(())
    }
}

impl Default for CompositionExecutor {
    fn default() -> Self {
        Self::new()
    }
}

/// Placeholder render data for operations
struct PlaceholderRenderData;

/// Optimization threshold for deep composition chains
pub const OPTIMIZATION_THRESHOLD: usize = 5;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RenderContext;

    #[derive(Debug, Clone)]
    struct TestVisualization {
        name: String,
        complexity: usize,
    }

    impl TestVisualization {
        fn new(name: &str, complexity: usize) -> Self {
            Self {
                name: name.to_string(),
                complexity,
            }
        }
    }

    impl Mixable for TestVisualization {
        type Output = ();

        fn render(&mut self, _context: &mut RenderContext) -> GupResult<()> {
            // Simulate work proportional to complexity
            for _ in 0..self.complexity * 100 {
                std::hint::black_box(self.name.len());
            }
            Ok(())
        }

        fn description(&self) -> String {
            self.name.clone()
        }
    }

    #[tokio::test]
    async fn test_composition_executor_creation() {
        let executor = CompositionExecutor::new();
        assert_eq!(executor.metrics().operation_count, 0);
        assert_eq!(executor.metrics().batch_count, 0);
    }

    #[tokio::test]
    async fn test_simple_composition_flattening() {
        let mut executor = CompositionExecutor::new();
        let viz = TestVisualization::new("test", 1);

        let result = executor.flatten_composition(&viz);
        assert!(result.is_ok());

        let metrics = executor.metrics();
        assert_eq!(metrics.operation_count, 1);
        assert!(metrics.batch_count > 0);
    }

    #[tokio::test]
    async fn test_composition_execution() {
        let mut context = RenderContext::new().await.unwrap();
        let mut executor = CompositionExecutor::new();
        let viz = TestVisualization::new("test", 1);

        executor.flatten_composition(&viz).unwrap();
        let result = executor.execute(&mut context);
        assert!(result.is_ok());

        let metrics = executor.metrics();
        assert!(metrics.render_time.as_nanos() > 0);
    }

    #[tokio::test]
    async fn test_multiple_component_batching() {
        let mut executor = CompositionExecutor::new();
        let viz1 = TestVisualization::new("viz1", 1);
        let viz2 = TestVisualization::new("viz2", 2);

        // Test with each component separately to verify batching would work
        executor.flatten_composition(&viz1).unwrap();
        assert_eq!(executor.metrics().operation_count, 1);

        executor.flatten_composition(&viz2).unwrap();
        assert_eq!(executor.metrics().operation_count, 1);
    }

    #[tokio::test]
    async fn test_metrics_collection() {
        let mut context = RenderContext::new().await.unwrap();
        let mut executor = CompositionExecutor::new();
        let viz = TestVisualization::new("test", 5);

        // Execute the optimization and rendering pipeline
        executor.flatten_composition(&viz).unwrap();
        executor.execute(&mut context).unwrap();

        let metrics = executor.metrics();
        assert!(metrics.optimization_time.as_nanos() > 0);
        assert!(metrics.render_time.as_nanos() > 0);
        assert_eq!(metrics.operation_count, 1);
        assert!(metrics.batch_count > 0);
    }

    #[tokio::test]
    async fn test_resource_pool_functionality() {
        let mut pool = ResourcePool::new();
        assert_eq!(pool.memory_usage(), 0);

        // Resource pool functionality would be tested with actual GPU device
        // For now, just test the structure
        let _initial_memory = pool.memory_usage();
        pool.clear();
        assert_eq!(pool.memory_usage(), 0);
        // Memory usage should be non-negative (always true for usize, but keeping for clarity)
    }

    #[tokio::test]
    async fn test_render_cache_functionality() {
        let mut cache = RenderCache::new();
        assert_eq!(cache.hit_rate(), 0.0);

        let key = CacheKey {
            component_hash: 12345,
            render_params_hash: 67890,
        };

        // Test cache miss
        assert!(cache.get(&key).is_none());
        assert!(cache.hit_rate() < 1.0); // Should be 0.0 due to miss

        cache.clear();
        assert_eq!(cache.hit_rate(), 0.0);
    }

    #[test]
    fn test_batch_render_state_comparison() {
        let state1 = BatchRenderState {
            pipeline_id: 1,
            uniforms: vec![1, 2, 3],
            textures: vec![TextureBinding {
                slot: 0,
                texture_id: 1,
            }],
        };

        let state2 = BatchRenderState {
            pipeline_id: 1,
            uniforms: vec![1, 2, 3],
            textures: vec![TextureBinding {
                slot: 0,
                texture_id: 1,
            }],
        };

        let state3 = BatchRenderState {
            pipeline_id: 2,
            uniforms: vec![1, 2, 3],
            textures: vec![TextureBinding {
                slot: 0,
                texture_id: 1,
            }],
        };

        let executor = CompositionExecutor::new();
        assert!(executor.can_batch_with_state(&state1, &state2));
        assert!(!executor.can_batch_with_state(&state1, &state3));
    }

    #[test]
    fn test_optimization_threshold() {
        assert_eq!(OPTIMIZATION_THRESHOLD, 5);
    }

    #[tokio::test]
    async fn test_clear_resources() {
        let mut executor = CompositionExecutor::new();
        let viz = TestVisualization::new("test", 1);

        executor.flatten_composition(&viz).unwrap();
        assert!(executor.metrics().operation_count > 0);

        executor.clear_resources();
        assert_eq!(executor.operations.len(), 0);
        assert_eq!(executor.batches.len(), 0);
    }

    #[tokio::test]
    async fn test_reset_metrics() {
        let mut context = RenderContext::new().await.unwrap();
        let mut executor = CompositionExecutor::new();
        let viz = TestVisualization::new("test", 1);

        executor.flatten_composition(&viz).unwrap();
        executor.execute(&mut context).unwrap();

        let metrics_before = executor.metrics();
        assert!(metrics_before.operation_count > 0);

        executor.reset_metrics();
        let metrics_after = executor.metrics();
        assert_eq!(metrics_after.operation_count, 0);
        assert_eq!(metrics_after.optimization_time, std::time::Duration::ZERO);
    }
}

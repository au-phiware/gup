# GUP-028: Composition Performance Optimization

## Story Overview

**Title**: Optimize Composition Performance with Caching and State Management  
**Epic**: Phase 1 Initiative 1 - Core GPU Primitives and Selection API  
**Priority**: Medium  
**Story Points**: 4  

## Context

Current composition implementation in GUP-021 recalculates viewport splits and blend states on every render. For complex nested compositions or animations, this overhead can accumulate. This story implements caching and optimization strategies to minimize composition overhead.

## User Story

**As a** visualization developer working with complex compositions  
**I want** composition operations to have minimal performance impact  
**So that** I can create deeply nested or animated compositions without performance degradation  

## Acceptance Criteria

### Performance Targets

- [ ] **Viewport Calculation Caching**: Repeated viewport splits cached when configuration unchanged
- [ ] **Blend State Optimization**: Minimize render pipeline state changes
- [ ] **Nested Composition Efficiency**: O(n) performance for n-deep composition nesting
- [ ] **Memory Usage**: Caching adds <1MB memory overhead per composition

### Optimization Features

- [ ] **Viewport Cache**: Cache calculated viewport splits with invalidation
- [ ] **Render Pipeline Pool**: Reuse pipelines with same blend configurations
- [ ] **State Change Batching**: Batch multiple state changes into single operations
- [ ] **Lazy Evaluation**: Defer expensive calculations until actually needed

### Benchmarking

- [ ] **Composition Overhead**: Target <0.5% overhead for simple compositions
- [ ] **Cache Hit Rate**: >90% cache hit rate for stable compositions
- [ ] **Memory Efficiency**: Bounded cache sizes with LRU eviction
- [ ] **Animation Performance**: Maintain 60fps for animated compositions

## Technical Design

### Viewport Calculation Caching

```rust
use std::collections::HashMap;

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct ViewportCacheKey {
    original_viewport: Viewport,
    config_hash: u64, // Hash of SideBySideConfig
}

#[derive(Debug, Clone)]
struct CachedViewportSplit {
    first_viewport: Viewport,
    second_viewport: Viewport,
    generation: u64, // For cache invalidation
}

impl<A: Mixable, B: Mixable> ComposedVisualization<A, B> {
    // Cache for viewport calculations
    viewport_cache: HashMap<ViewportCacheKey, CachedViewportSplit>,
    cache_generation: u64,
    
    /// Calculate viewport splits with caching
    fn calculate_split_viewports_cached(&mut self, original: Viewport) -> (Viewport, Viewport) {
        let config_hash = self.calculate_config_hash();
        let cache_key = ViewportCacheKey {
            original_viewport: original,
            config_hash,
        };
        
        // Check cache first
        if let Some(cached) = self.viewport_cache.get(&cache_key) {
            if cached.generation == self.cache_generation {
                return (cached.first_viewport, cached.second_viewport);
            }
        }
        
        // Calculate and cache
        let (first_vp, second_vp) = self.calculate_split_viewports(original);
        
        self.viewport_cache.insert(cache_key, CachedViewportSplit {
            first_viewport: first_vp,
            second_viewport: second_vp,
            generation: self.cache_generation,
        });
        
        (first_vp, second_vp)
    }
    
    /// Invalidate viewport cache when configuration changes
    pub fn invalidate_viewport_cache(&mut self) {
        self.cache_generation += 1;
        // Old cache entries will be ignored due to generation mismatch
    }
}
```

### Render Pipeline Pooling

```rust
use std::sync::Arc;

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct PipelineKey {
    surface_format: TextureFormat,
    blend_mode: BlendMode,
    vertex_layout_hash: u64,
}

pub struct RenderPipelinePool {
    pipelines: HashMap<PipelineKey, Arc<RenderPipeline>>,
    device: Arc<Device>,
    max_cached_pipelines: usize,
    access_order: Vec<PipelineKey>, // For LRU eviction
}

impl RenderPipelinePool {
    pub fn get_or_create_pipeline(&mut self, key: PipelineKey) -> Arc<RenderPipeline> {
        // Update access order for LRU
        if let Some(pos) = self.access_order.iter().position(|k| k == &key) {
            let key = self.access_order.remove(pos);
            self.access_order.push(key);
        }
        
        // Return cached pipeline if available
        if let Some(pipeline) = self.pipelines.get(&key) {
            return pipeline.clone();
        }
        
        // Create new pipeline
        let pipeline = Arc::new(self.create_pipeline_for_key(&key));
        
        // Cache management - evict LRU if at capacity
        if self.pipelines.len() >= self.max_cached_pipelines {
            if let Some(lru_key) = self.access_order.first().cloned() {
                self.pipelines.remove(&lru_key);
                self.access_order.remove(0);
            }
        }
        
        self.pipelines.insert(key.clone(), pipeline.clone());
        self.access_order.push(key);
        
        pipeline
    }
}
```

### State Change Batching

```rust
#[derive(Debug, Clone)]
pub struct BatchedStateChange {
    blend_mode: Option<BlendMode>,
    viewport: Option<Viewport>,
    global_alpha: Option<f32>,
}

impl RenderContext {
    /// Begin a batch of state changes
    pub fn begin_state_batch(&mut self) -> StateBatch {
        StateBatch {
            changes: BatchedStateChange::default(),
            context: self,
        }
    }
}

pub struct StateBatch<'a> {
    changes: BatchedStateChange,
    context: &'a mut RenderContext,
}

impl<'a> StateBatch<'a> {
    /// Queue blend mode change
    pub fn set_blend_mode(mut self, mode: BlendMode) -> Self {
        self.changes.blend_mode = Some(mode);
        self
    }
    
    /// Queue viewport change
    pub fn set_viewport(mut self, viewport: Viewport) -> Self {
        self.changes.viewport = Some(viewport);
        self
    }
    
    /// Queue global alpha change
    pub fn set_global_alpha(mut self, alpha: f32) -> Self {
        self.changes.global_alpha = Some(alpha);
        self
    }
    
    /// Apply all batched changes at once
    pub fn commit(self) -> GupResult<()> {
        // Apply all changes in optimal order
        if let Some(blend_mode) = self.changes.blend_mode {
            self.context.set_blend_mode_internal(blend_mode)?;
        }
        
        if let Some(viewport) = self.changes.viewport {
            self.context.set_viewport_internal(viewport)?;
        }
        
        if let Some(alpha) = self.changes.global_alpha {
            self.context.set_global_alpha_internal(alpha)?;
        }
        
        Ok(())
    }
}
```

### Lazy Configuration Hash Calculation

```rust
impl SideBySideConfig {
    fn calculate_hash(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        self.hash(&mut hasher);
        hasher.finish()
    }
}

// Implement Hash for configuration types
impl Hash for SideBySideConfig {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.direction.hash(state);
        // Use integer representation for float hashing
        (self.split_ratio.to_bits()).hash(state);
        (self.padding.to_bits()).hash(state);
    }
}
```

## Dependencies

### Prerequisite Stories

- GUP-021: Advanced Composition Mode Implementation (provides composition framework)
- GUP-027: GPU Blend State Integration (provides pipeline state management)

### Enables Stories

- Complex nested composition scenarios
- Real-time animated compositions
- Performance-critical visualization applications

## Testing Strategy

### Performance Benchmarks

```rust
use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};

fn bench_viewport_caching(c: &mut Criterion) {
    let mut group = c.benchmark_group("viewport_caching");
    
    for size in [100, 1000, 10000].iter() {
        group.bench_with_input(BenchmarkId::new("uncached", size), size, |b, &size| {
            let mut composition = create_side_by_side_composition();
            let viewport = Viewport { width: *size, height: *size, scale_factor: 1.0 };
            
            b.iter(|| {
                composition.invalidate_viewport_cache(); // Force recalculation
                black_box(composition.calculate_split_viewports_cached(viewport));
            });
        });
        
        group.bench_with_input(BenchmarkId::new("cached", size), size, |b, &size| {
            let mut composition = create_side_by_side_composition();
            let viewport = Viewport { width: *size, height: *size, scale_factor: 1.0 };
            
            // Pre-warm cache
            composition.calculate_split_viewports_cached(viewport);
            
            b.iter(|| {
                black_box(composition.calculate_split_viewports_cached(viewport));
            });
        });
    }
    
    group.finish();
}

fn bench_nested_composition_depth(c: &mut Criterion) {
    let mut group = c.benchmark_group("nested_composition");
    
    for depth in [1, 5, 10, 20].iter() {
        group.bench_with_input(BenchmarkId::new("depth", depth), depth, |b, &depth| {
            let composition = create_nested_composition(*depth);
            let mut context = create_test_context();
            
            b.iter(|| {
                black_box(composition.clone().render(&mut context)).unwrap();
            });
        });
    }
    
    group.finish();
}
```

### Cache Effectiveness Tests

```rust
#[test]
fn test_viewport_cache_hit_rate() {
    let mut composition = create_side_by_side_composition();
    let viewport = Viewport { width: 800, height: 600, scale_factor: 1.0 };
    
    // Multiple calls with same parameters should hit cache
    let _split1 = composition.calculate_split_viewports_cached(viewport);
    let _split2 = composition.calculate_split_viewports_cached(viewport);
    let _split3 = composition.calculate_split_viewports_cached(viewport);
    
    // Verify cache statistics
    assert_eq!(composition.cache_stats().hit_rate(), 0.67); // 2/3 calls hit cache
}

#[test]
fn test_pipeline_pool_lru_eviction() {
    let mut pool = RenderPipelinePool::new(device, 2); // Max 2 cached pipelines
    
    let key1 = PipelineKey { /* ... */ };
    let key2 = PipelineKey { /* ... */ };
    let key3 = PipelineKey { /* ... */ };
    
    let _pipeline1 = pool.get_or_create_pipeline(key1.clone());
    let _pipeline2 = pool.get_or_create_pipeline(key2.clone());
    
    // This should evict key1 (LRU)
    let _pipeline3 = pool.get_or_create_pipeline(key3.clone());
    
    assert!(!pool.contains_key(&key1));
    assert!(pool.contains_key(&key2));
    assert!(pool.contains_key(&key3));
}
```

### Memory Usage Tests

```rust
#[test]
fn test_bounded_cache_memory() {
    let mut composition = create_large_composition_tree();
    
    let initial_memory = get_memory_usage();
    
    // Exercise caching extensively
    for i in 0..1000 {
        let viewport = Viewport { 
            width: 800 + i, 
            height: 600 + i, 
            scale_factor: 1.0 
        };
        composition.calculate_split_viewports_cached(viewport);
    }
    
    let final_memory = get_memory_usage();
    let memory_increase = final_memory - initial_memory;
    
    // Verify memory usage is bounded (cache should evict old entries)
    assert!(memory_increase < 1_000_000); // <1MB
}
```

## Implementation Phases

### Phase 1: Viewport Caching

- Implement hash-based caching for viewport calculations
- Add cache invalidation mechanisms
- Basic performance benchmarks

### Phase 2: Pipeline Pooling

- Render pipeline caching with LRU eviction
- State change optimization
- Memory usage monitoring

### Phase 3: Advanced Optimizations

- State change batching
- Lazy evaluation patterns
- Comprehensive performance validation

## Success Metrics

### Performance Improvements

- [ ] **Viewport Calculation**: 10x faster for cache hits
- [ ] **Pipeline Creation**: Reduce pipeline creation by 90% through caching
- [ ] **Nested Composition**: Linear scaling with composition depth
- [ ] **Overall Overhead**: <0.5% composition overhead vs direct rendering

### Resource Efficiency

- [ ] **Memory Usage**: Bounded cache sizes with configurable limits
- [ ] **Cache Hit Rate**: >90% for typical usage patterns
- [ ] **GPU Resource Usage**: No increase in GPU memory usage

## Definition of Done

- [ ] Viewport calculation caching implemented with hash-based keys
- [ ] Render pipeline pooling with LRU eviction strategy
- [ ] State change batching system for optimal GPU state management
- [ ] Comprehensive performance benchmarks showing improvements
- [ ] Memory usage tests confirming bounded cache behavior
- [ ] Cache hit rate monitoring and optimization
- [ ] Documentation of performance characteristics and tuning options
- [ ] Integration tests with complex nested compositions
- [ ] Backwards compatibility maintained with existing composition API

# GUP-074: Mark Performance Optimization

**Story ID**: GUP-074  
**Title**: Mark Performance Optimization  
**Status**: Planned  
**Priority**: High  
**Effort**: 5 story points  
**Created**: 2025-08-04  
**Dependencies**: GUP-011 (Mark-Shader Integration)

## Summary

Optimize mark rendering performance through GPU instancing, batch processing,
and advanced caching to achieve sub-millisecond rendering for 100K+ data points.

## Background

Current mark rendering in GUP-011 processes data points individually, which
limits performance for large datasets. While the basic functionality works,
performance optimization is needed for production use cases involving:

- Real-time data streaming (financial tickers, IoT sensors)
- Large scientific datasets (>100K points)
- Interactive visualizations requiring <16ms frame times
- Mobile devices with limited GPU resources

Key performance bottlenecks identified:

- Individual vertex buffer uploads per mark
- Lack of GPU instancing for repeated geometry
- Inefficient pipeline state changes
- No geometry culling for off-screen marks

## Requirements

### Functional Requirements

1. **GPU Instancing**

   - Implement instanced rendering for marks with identical geometry
   - Support per-instance attributes (position, color, size)
   - Batch multiple mark types in single draw call where possible

2. **Advanced Caching**

   - Cache compiled shader pipelines across frames
   - Implement geometry caching for complex marks
   - Add attribute buffer pooling to reduce allocations

3. **Culling and LOD**

   - Frustum culling to skip off-screen marks
   - Level-of-detail based on screen space size
   - Occlusion culling for overlapped marks

4. **Batch Processing**
   - Group marks by shader pipeline for efficient rendering
   - Minimize GPU state changes between batches
   - Support mixed mark types in single render pass

### Non-Functional Requirements

1. **Performance Targets**:

   - 100K points: \<1ms render time
   - 1M points: \<10ms render time
   - Memory usage: \<50MB for 1M points
   - Frame rate: 60fps for interactive scenarios

2. **Quality**: Maintain visual accuracy - optimizations should not affect
   output
3. **Compatibility**: Work with existing mark implementations from GUP-011

## Acceptance Criteria

1. **Instancing Implementation**

   - [ ] GPU instancing for Circle, Rectangle, and Line marks
   - [ ] Per-instance attribute buffers (position, color, size, rotation)
   - [ ] Batch rendering of up to 10K instances per draw call
   - [ ] Benchmark shows 10x performance improvement for repeated geometry

2. **Caching System**

   - [ ] Pipeline cache with LRU eviction policy
   - [ ] Geometry buffer pooling with automatic resizing
   - [ ] Attribute buffer reuse across frames
   - [ ] Cache hit rate >90% for typical use cases

3. **Culling and LOD**

   - [ ] Frustum culling reduces processed marks by 50-80% for typical views
   - [ ] LOD system with 3 levels (full, simplified, point)
   - [ ] Automatic LOD selection based on screen space size
   - [ ] Occlusion culling for dense point clouds

4. **Performance Benchmarks**

   - [ ] 100K circles render in \<1ms (instanced)
   - [ ] 1M points with mixed marks render in \<10ms
   - [ ] Memory usage scales linearly with data size
   - [ ] CPU overhead \<5% of total frame time

5. **Quality Assurance**
   - [ ] Visual regression tests ensure identical output
   - [ ] All existing mark functionality preserved
   - [ ] Performance improvements don't break existing APIs

## Technical Design

### Instanced Rendering Architecture

```rust
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct InstanceAttributes {
    pub transform: [f32; 16],    // 4x4 matrix for position/scale/rotation
    pub color: [f32; 4],         // RGBA color
    pub custom_data: [f32; 4],   // Mark-specific data (radius, line width, etc.)
}

pub struct InstancedMarkRenderer<M: Mark> {
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,        // Shared geometry
    instance_buffer: wgpu::Buffer,      // Per-instance attributes
    instance_capacity: usize,
    instances: Vec<InstanceAttributes>,
}
```

### Batching System

```rust
pub struct MarkBatchRenderer {
    batches: HashMap<TypeId, Box<dyn InstancedRenderer>>,
    render_queue: Vec<RenderBatch>,
    frame_stats: PerformanceStats,
}

pub struct RenderBatch {
    pub mark_type: TypeId,
    pub shader_hash: u64,
    pub instance_range: Range<usize>,
    pub z_order: f32,
}
```

### Culling System

```rust
pub struct CullingManager {
    frustum: Frustum,
    lod_thresholds: [f32; 4], // Screen space size thresholds
    occlusion_query: wgpu::QuerySet,
}

impl CullingManager {
    pub fn cull_marks(&self, marks: &[MarkInstance]) -> Vec<CulledMark> {
        // Implementation for frustum + occlusion culling
    }

    pub fn compute_lod(&self, mark: &MarkInstance, view_matrix: &Mat4) -> LODLevel {
        // Compute appropriate level of detail
    }
}
```

## Implementation Plan

### Phase 1: Instancing Infrastructure (2 points)

- Implement InstancedMarkRenderer for basic marks
- Add per-instance attribute buffers
- Create instanced shader variants for Circle, Rectangle, Line
- Add basic performance benchmarks

### Phase 2: Batching and Caching (2 points)

- Implement MarkBatchRenderer for efficient state management
- Add pipeline caching with LRU eviction
- Create geometry buffer pooling system
- Optimize attribute buffer reuse

### Phase 3: Culling and LOD (1 point)

- Implement frustum culling with view matrix
- Add level-of-detail system with screen space metrics
- Create simplified geometry for distant marks
- Add occlusion culling for dense datasets

## Performance Targets

| Dataset Size | Current (GUP-011) | Target (GUP-074) | Improvement |
| ------------ | ----------------- | ---------------- | ----------- |
| 1K points    | ~0.1ms            | ~0.05ms          | 2x          |
| 10K points   | ~1ms              | ~0.2ms           | 5x          |
| 100K points  | ~10ms             | ~1ms             | 10x         |
| 1M points    | ~100ms            | ~10ms            | 10x         |

## Risks and Mitigations

1. **Risk**: GPU memory limits for large instance buffers

   - **Mitigation**: Implement streaming for datasets >1M points, compress
     instance data

2. **Risk**: Complexity of batching different mark types

   - **Mitigation**: Start with single mark type batching, expand incrementally

3. **Risk**: LOD affecting visual quality
   - **Mitigation**: Implement smooth transitions, make LOD thresholds
     configurable

## Success Metrics

- 10x performance improvement for large datasets (>100K points)
- Memory usage remains linear with dataset size
- Zero visual regressions in automated tests
- 60fps maintained for interactive scenarios up to 1M points
- CPU usage \<5% of total frame rendering time

## Future Considerations

- Compute shader-based culling for very large datasets
- Temporal coherence optimizations for animated data
- Multi-GPU rendering for datasets >10M points
- Integration with spatial indexing structures (octrees, R-trees)

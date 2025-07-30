# GUP-012: GPU Interaction System

## Story Overview

**Title**: Implement GPU-Accelerated Interaction System  
**Epic**: Phase 1 Initiative 4 - Interaction System and Performance  
**Priority**: Critical  
**Story Points**: 13  

## Context

The interaction system must handle hit testing, picking, and event handling for massive datasets using GPU acceleration. Traditional CPU-based hit testing becomes a bottleneck with 100K+ points, so the system must perform spatial queries and collision detection in parallel on the GPU while maintaining responsiveness and accuracy.

## User Story

**As a** visualization developer  
**I want** GPU-accelerated interaction handling for large datasets  
**So that** I can provide responsive hover, click, and selection interactions even with millions of data points  

## Acceptance Criteria

### Core Interaction Features

- [ ] **GPU Hit Testing**: Parallel hit detection using compute shaders
- [ ] **Spatial Queries**: Efficient point-in-shape and region selection
- [ ] **Event Integration**: Connect GPU interactions with high-level event handlers
- [ ] **Performance Target**: <1ms hit testing for 1M+ points

### Interaction Types

```rust
pub enum InteractionType {
    Hover(Vec2),           // Mouse hover at position
    Click(Vec2),           // Mouse click at position
    Drag(Vec2, Vec2),      // Drag from start to end position
    RegionSelect(Rect),    // Rectangular selection region
    Custom(Box<dyn InteractionQuery>), // Custom interaction queries
}
```

### Query System

- [ ] **Point Queries**: Find elements at specific screen coordinates
- [ ] **Region Queries**: Find all elements within rectangular or polygonal regions
- [ ] **Custom Queries**: Extensible system for complex spatial queries
- [ ] **Batch Queries**: Process multiple queries efficiently in single GPU dispatch

## Technical Tasks

### 1. GPU Hit Testing Core

- [ ] Design compute shader for parallel hit testing
- [ ] Implement spatial data structures on GPU
- [ ] Create query buffer management system
- [ ] Add result collection and CPU readback

### 2. Spatial Query System

- [ ] Implement point-in-shape testing for all mark types
- [ ] Create region intersection algorithms
- [ ] Add distance-based queries for nearest neighbors
- [ ] Support complex polygon selection regions

### 3. Event System Integration

- [ ] Connect GPU query results with event handlers
- [ ] Implement event propagation and bubbling
- [ ] Add event filtering and priority systems
- [ ] Create async event processing pipeline

### 4. Performance Optimization

- [ ] Implement spatial indexing for query acceleration
- [ ] Add level-of-detail for interaction at different zoom levels
- [ ] Create query batching and coalescing
- [ ] Optimize GPU-CPU synchronization

## Detailed Requirements

### Core Interaction System

```rust
pub struct InteractionSystem {
    // GPU compute resources
    hit_test_pipeline: wgpu::ComputePipeline,
    spatial_index_buffer: GpuBuffer<SpatialNode>,
    query_buffer: GpuBuffer<InteractionQuery>,
    result_buffer: GpuBuffer<InteractionResult>,
    
    // CPU-side management
    event_handlers: HashMap<String, Vec<Box<dyn EventHandler>>>,
    active_queries: Vec<PendingQuery>,
    
    // Performance monitoring
    query_stats: QueryStats,
}

impl InteractionSystem {
    pub async fn query_point(&mut self, position: Vec2, selections: &[&dyn Renderable]) -> Vec<ElementHit> {
        // Create GPU query
        let query = InteractionQuery::Point { position };
        let query_id = self.submit_query(query, selections).await;
        
        // Collect results
        self.collect_query_results(query_id).await
    }
    
    pub async fn query_region(&mut self, region: Rect, selections: &[&dyn Renderable]) -> Vec<ElementHit> {
        let query = InteractionQuery::Region { bounds: region };
        let query_id = self.submit_query(query, selections).await;
        
        self.collect_query_results(query_id).await
    }
    
    pub fn register_event_handler<F>(&mut self, event_type: &str, handler: F)
    where F: Fn(&InteractionEvent) + Send + Sync + 'static
    {
        self.event_handlers
            .entry(event_type.to_string())
            .or_default()
            .push(Box::new(handler));
    }
}
```

### GPU Hit Testing Compute Shader

```wgsl
// hit_test.compute.wgsl
struct InteractionQuery {
    query_type: u32,      // 0 = point, 1 = region, 2 = custom
    position: vec2<f32>,  // Query position or region center
    region_size: vec2<f32>, // For region queries
    max_results: u32,
}

struct ElementData {
    position: vec2<f32>,
    size: vec2<f32>,
    mark_type: u32,
    element_id: u32,
}

struct InteractionResult {
    element_id: u32,
    distance: f32,
    intersection_point: vec2<f32>,
    is_hit: u32,
}

@group(0) @binding(0) var<storage, read> elements: array<ElementData>;
@group(0) @binding(1) var<storage, read> queries: array<InteractionQuery>;
@group(0) @binding(2) var<storage, read_write> results: array<InteractionResult>;

@compute @workgroup_size(256)
fn hit_test_main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let element_index = global_id.x;
    let query_index = global_id.y;
    
    if (element_index >= arrayLength(&elements) || query_index >= arrayLength(&queries)) {
        return;
    }
    
    let element = elements[element_index];
    let query = queries[query_index];
    
    var result: InteractionResult;
    result.element_id = element.element_id;
    result.is_hit = 0u;
    
    // Perform hit test based on mark type and query type
    switch (element.mark_type) {
        case 0u: { // Circle
            result.is_hit = test_circle_hit(element, query);
        }
        case 1u: { // Rectangle
            result.is_hit = test_rectangle_hit(element, query);
        }
        case 2u: { // Line
            result.is_hit = test_line_hit(element, query);
        }
        default: {
            result.is_hit = 0u;
        }
    }
    
    if (result.is_hit != 0u) {
        let distance = length(query.position - element.position);
        result.distance = distance;
        result.intersection_point = element.position;
    }
    
    let result_index = element_index * arrayLength(&queries) + query_index;
    results[result_index] = result;
}

fn test_circle_hit(element: ElementData, query: InteractionQuery) -> u32 {
    let distance = length(query.position - element.position);
    let radius = element.size.x * 0.5;
    
    switch (query.query_type) {
        case 0u: { // Point query
            return select(0u, 1u, distance <= radius);
        }
        case 1u: { // Region query
            let region_bounds = vec4<f32>(
                query.position - query.region_size * 0.5,
                query.position + query.region_size * 0.5
            );
            return select(0u, 1u, circle_intersects_rect(element.position, radius, region_bounds));
        }
        default: {
            return 0u;
        }
    }
}
```

### Selection Integration

```rust
impl<T, M: Mark> Selection<T, M> {
    pub fn on<F>(&mut self, event_type: &str, handler: F) -> &mut Self
    where F: Fn(InteractionEvent, &T) + Send + Sync + 'static
    {
        // Register mark type for hit testing
        self.interaction_system.register_mark_type::<M>();
        
        // Create event handler that filters for this selection's elements
        let selection_id = self.id();
        let wrapped_handler = move |event: &InteractionEvent| {
            if let Some(hit) = &event.hit {
                if hit.selection_id == selection_id {
                    if let Some(data) = self.get_data_for_element(hit.element_id) {
                        handler(event.clone(), data);
                    }
                }
            }
        };
        
        self.interaction_system.register_event_handler(event_type, wrapped_handler);
        self
    }
    
    pub fn query_at_position(&self, position: Vec2) -> Option<(usize, &T)> {
        // Synchronous query for immediate results (uses cached spatial index)
        self.interaction_system.query_selection_sync(self.id(), position)
    }
}
```

### Spatial Index System

```rust
pub struct SpatialIndex {
    // GPU-resident spatial data structure
    nodes: GpuBuffer<SpatialNode>,
    elements: GpuBuffer<SpatialElement>,
    
    // Index configuration
    grid_size: Vec2,
    cell_size: Vec2,
    max_elements_per_cell: usize,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct SpatialNode {
    bounds: [f32; 4],        // min_x, min_y, max_x, max_y
    element_start: u32,      // Index into elements array
    element_count: u32,      // Number of elements in this node
    child_nodes: [u32; 4],   // Indices of child nodes (quadtree)
}

impl SpatialIndex {
    pub fn build_from_selection<T, M: Mark>(&mut self, selection: &Selection<T, M>) {
        // Extract spatial information from selection data
        let spatial_elements = self.extract_spatial_elements(selection);
        
        // Build spatial hierarchy on CPU
        let root_node = self.build_quadtree(&spatial_elements);
        
        // Upload to GPU
        self.upload_spatial_data(&root_node);
    }
    
    pub async fn query_region(&self, region: Rect) -> Vec<u32> {
        // Use compute shader to traverse spatial index and find intersecting elements
        let query = SpatialQuery { region, max_results: 10000 };
        self.execute_spatial_query(query).await
    }
}
```

## Dependencies

### Prerequisite Stories

- GUP-002: Core Selection Type (provides selections to interact with)
- GUP-009: Core Mark Trait (defines marks for hit testing)
- GUP-010: Basic Mark Implementations (provides marks to test against)
- GUP-003: GPU Buffer Management (for query and result buffers)

### Enables Stories

- GUP-013: Event Handling System (uses interaction results)
- GUP-014: Performance Validation (validates interaction performance)
- All interactive visualization features

## Testing Strategy

### Unit Tests

```rust
#[test]
fn test_interaction_system_creation() {
    let device = create_test_device();
    let interaction_system = InteractionSystem::new(&device);
    
    assert!(interaction_system.is_initialized());
}

#[test]
async fn test_point_query() {
    let mut system = create_test_interaction_system();
    let selection = create_test_circle_selection();
    
    let hits = system.query_point(Vec2::new(50.0, 50.0), &[&selection]).await;
    
    // Should find circle at (50, 50) with radius 10
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].element_id, 0);
}

#[test]
async fn test_region_query() {
    let mut system = create_test_interaction_system();
    let selection = create_test_circle_selection();
    
    let region = Rect::new(Vec2::new(0.0, 0.0), Vec2::new(100.0, 100.0));
    let hits = system.query_region(region, &[&selection]).await;
    
    // Should find all circles within region
    assert!(hits.len() > 0);
}
```

### Performance Tests

```rust
#[bench]
async fn bench_gpu_hit_testing_10k_points(b: &mut Bencher) {
    let mut system = create_test_interaction_system();
    let selection = create_large_selection(10_000);
    
    b.iter(|| async {
        let _hits = system.query_point(Vec2::new(500.0, 500.0), &[&selection]).await;
    });
}

#[bench]
async fn bench_cpu_vs_gpu_hit_testing(b: &mut Bencher) {
    let data = create_test_data(100_000);
    
    // Benchmark GPU approach
    let gpu_time = bench_gpu_hit_testing(&data).await;
    
    // Benchmark CPU approach
    let cpu_time = bench_cpu_hit_testing(&data);
    
    // GPU should be significantly faster for large datasets
    assert!(gpu_time < cpu_time / 10.0);
}
```

### Accuracy Tests

```rust
#[test]
async fn test_hit_testing_accuracy() {
    let mut system = create_test_interaction_system();
    
    // Create selection with known element positions
    let selection = create_precise_test_selection();
    
    // Test hits at exact element positions
    for (i, position) in known_positions.iter().enumerate() {
        let hits = system.query_point(*position, &[&selection]).await;
        assert_eq!(hits.len(), 1, "Should hit exactly one element at {}", position);
        assert_eq!(hits[0].element_id, i as u32, "Should hit element {}", i);
    }
    
    // Test misses at positions between elements
    for position in miss_positions {
        let hits = system.query_point(position, &[&selection]).await;
        assert_eq!(hits.len(), 0, "Should miss at position {}", position);
    }
}

#[test]
async fn test_different_mark_types() {
    let mut system = create_test_interaction_system();
    
    // Test hit testing works correctly for all mark types
    let circle_selection = create_circle_selection();
    let rect_selection = create_rectangle_selection();
    let line_selection = create_line_selection();
    
    let test_position = Vec2::new(50.0, 50.0);
    let hits = system.query_point(test_position, &[
        &circle_selection,
        &rect_selection, 
        &line_selection
    ]).await;
    
    // Verify correct marks are hit based on their geometry
    verify_mark_hit_accuracy(&hits, test_position);
}
```

### Integration Tests

```rust
#[test]
async fn test_event_handler_integration() {
    let mut system = create_test_interaction_system();
    let mut selection = create_test_selection();
    
    let mut event_fired = false;
    selection.on("click", |event, data| {
        event_fired = true;
        assert_eq!(data.id, 42); // Verify correct data is passed
    });
    
    // Simulate click event
    let click_position = Vec2::new(50.0, 50.0);
    system.process_click_event(click_position).await;
    
    assert!(event_fired, "Click event handler should have been called");
}
```

## Success Metrics

### Performance Requirements

- [ ] **Query Speed**: <1ms for point queries on 1M+ points
- [ ] **Region Query Speed**: <10ms for region queries on 1M+ points  
- [ ] **Throughput**: Handle 1000+ queries per second
- [ ] **Memory Efficiency**: <10MB GPU memory for spatial indexing 1M points

### Accuracy Requirements

- [ ] **Hit Testing Precision**: 100% accuracy for simple shapes
- [ ] **Edge Case Handling**: Correct behavior at shape boundaries
- [ ] **Multi-Selection**: Accurate results when querying multiple selections
- [ ] **Coordinate Precision**: Pixel-perfect accuracy at all zoom levels

### Integration Requirements

- [ ] **Event System**: Seamless integration with high-level event handlers
- [ ] **Selection Compatibility**: Works with all mark types and data types
- [ ] **Real-Time Performance**: Maintains 60 FPS during interactive usage
- [ ] **Cross-Platform**: Identical behavior across all supported platforms

## Risk Assessment

### Technical Risks

- **High**: GPU hit testing complexity could introduce bugs or performance issues
- **Medium**: Spatial indexing overhead might not provide expected speedup
- **Medium**: GPU-CPU synchronization could create latency bottlenecks

### Mitigation Strategies

- **Reference Implementation**: Compare against CPU-based hit testing for accuracy validation
- **Performance Profiling**: Comprehensive benchmarking at different data scales
- **Fallback Strategy**: CPU fallback for cases where GPU approach fails

## Implementation Notes

### Design Decisions

- Use compute shaders for maximum parallelization of hit testing
- Implement spatial indexing on GPU to reduce query complexity
- Async query processing to avoid blocking main thread
- Batch multiple queries together for efficiency

### GPU Hit Testing Strategy

- Parallel processing of all elements against query in single compute dispatch
- Use shared memory for query data to reduce memory bandwidth
- Implement early termination for queries with maximum result limits
- Sort results by distance for consistent ordering

### Spatial Index Strategy

- Build quadtree/octree structure for hierarchical spatial queries
- Update spatial index incrementally when data changes
- Use GPU-resident spatial data structures to avoid CPU-GPU transfers
- Implement level-of-detail for different zoom levels

### Event Integration Strategy

- Maintain mapping between GPU element IDs and CPU data objects
- Use weak references to avoid preventing selection cleanup
- Implement event priority and filtering systems
- Support both synchronous and asynchronous event processing

## Definition of Done

- [ ] GPU hit testing working for all basic mark types (Circle, Rectangle, Line)
- [ ] Point and region queries implemented and tested
- [ ] Spatial indexing system providing performance improvements
- [ ] Event handler integration allowing high-level interaction programming
- [ ] Performance benchmarks meeting <1ms target for 1M+ points
- [ ] Accuracy tests validating pixel-perfect hit testing
- [ ] Integration with Selection system working correctly
- [ ] Cross-platform compatibility verified
- [ ] Memory usage within acceptable limits for large datasets
- [ ] Documentation complete with interaction examples
- [ ] Code review completed and approved

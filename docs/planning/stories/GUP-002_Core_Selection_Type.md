# GUP-002: Core Selection Type

## Story Overview

**Title**: Implement Core Selection<T, M> Type **Epic**: Phase 1 Initiative 1 -
Core GPU Primitives and Selection API **Priority**: Critical **Story Points**:
13

## Context

The `Selection<T, M>` type is the heart of Gup's composability system, directly
inspired by D3.js selections. It represents a collection of data bound to visual
marks with GPU-accelerated attribute mappings. This type must provide all the
power of D3 selections while leveraging GPU parallel processing.

## User Story

**As a** visualization developer **I want** a Selection type that binds data to
visual marks with GPU acceleration **So that** I can create complex, performant
visualizations using familiar D3-style patterns

## Acceptance Criteria

### AC1: Core Type Definition

```rust
pub struct Selection<T, M: Mark> {
    // Raw data stored on GPU
    data: Vec<T>,
    mark_type: PhantomData<M>,

    // GPU resources
    vertex_buffer: GpuBuffer<M::Vertex>,
    instance_buffer: GpuBuffer<InstanceData>,

    // Shader function pipeline for attribute mapping
    shader_pipeline: ShaderPipeline,
    attribute_mappings: HashMap<String, String>,

    // Rendering context
    context: Arc<GupContext>,
}
```

### AC2: API Requirements

- [ ] **Data Binding**: Seamlessly bind any Rust data type to visual marks
- [ ] **Attribute Mapping**: Map data fields to visual attributes using shader
      functions
- [ ] **GPU Acceleration**: All data transformations happen on GPU in parallel
- [ ] **Type Safety**: Compile-time validation of data-to-attribute mappings

### AC3: Core Methods

```rust
impl<T, M: Mark> Selection<T, M> {
    // Create new selection with data
    pub fn new(data: Vec<T>, context: Arc<GupContext>) -> Self;

    // Bind shader functions to visual attributes
    pub fn attr<F>(&mut self, name: &str, shader_func: F) -> &mut Self
    where
        F: ShaderFunction + 'static,
        F::Input: Compatible<T>,
        F::Output: Compatible<M::AttributeValue>;

    // Event handling
    pub fn on<H>(&mut self, event: &str, handler: H) -> &mut Self
    where H: Fn(InteractionEvent, &T) + Send + Sync + 'static;

    // Render selection to current render target
    pub fn render(&self) -> Result<(), GupError>;
}
```

## Technical Tasks

### 1. Core Selection Structure

- [ ] Define Selection struct with generic parameters for data and mark types
- [ ] Implement GPU buffer management for vertex and instance data
- [ ] Create shader pipeline integration for attribute mapping
- [ ] Add reference counting for shared context

### 2. Data Binding System

- [ ] Implement data upload to GPU storage buffers
- [ ] Create automatic data type to WGSL struct mapping
- [ ] Add incremental data update mechanisms
- [ ] Handle variable-length data efficiently

### 3. Attribute Mapping

- [ ] Build shader function composition system
- [ ] Implement compile-time type validation for attribute bindings
- [ ] Create automatic uniform buffer management
- [ ] Add runtime attribute binding validation

### 4. GPU Resource Management

- [ ] Implement efficient buffer allocation and reuse
- [ ] Add automatic buffer resizing for data updates
- [ ] Create resource cleanup and lifecycle management
- [ ] Optimize GPU memory usage patterns

## Behavior Specifications

### Data Handling

- [ ] **Immutable Data**: Original data never modified, only transformed in
      shaders
- [ ] **Incremental Updates**: Support adding/removing data points efficiently
- [ ] **Type Preservation**: Data type information maintained through GPU
      pipeline
- [ ] **Memory Safety**: No data races or memory leaks in GPU buffer management

### Attribute Binding

- [ ] **Shader Function Integration**: Attributes map to shader functions, not
      CPU closures
- [ ] **Type Validation**: Invalid attribute mappings caught at compile time
- [ ] **Performance**: Attribute updates trigger GPU pipeline regeneration
- [ ] **Composability**: Attribute shader functions compose with other shader
      functions

### Rendering Pipeline

- [ ] **Lazy Evaluation**: Shader pipeline generated only when needed
- [ ] **Caching**: Generated shaders cached until attributes change
- [ ] **Error Handling**: Clear error messages for invalid configurations
- [ ] **Performance**: <1ms render time for 10K+ points

## Dependencies

### Prerequisite Stories

- GUP-001: Build Mixable Trait
- GUP-003: GPU Buffer Management (can be developed in parallel)
- GUP-004: Basic Render Context (can be developed in parallel)

### Enables Stories

- GUP-005: Shader Function Composition
- GUP-008: Mark System Integration
- GUP-011: Event Handling System

## Testing Strategy

### Unit Tests

```rust
#[test]
fn test_selection_creation() {
    let data = vec![TestData { x: 1.0, y: 2.0 }];
    let context = create_test_context();
    let selection = Selection::<TestData, Circle>::new(data, context);
    assert!(selection.is_valid());
}

#[test]
fn test_attribute_binding() {
    let mut selection = create_test_selection();
    selection.attr("position", position_shader_func);
    selection.attr("color", color_shader_func);

    assert!(selection.has_attribute("position"));
    assert!(selection.has_attribute("color"));
}

#[test]
fn test_type_safety() {
    let mut selection = create_test_selection();

    // This should compile
    selection.attr("position", valid_position_func);

    // This should NOT compile (type mismatch)
    // selection.attr("position", color_func);
}
```

### Integration Tests

- [ ] Test with various data types and mark combinations
- [ ] Verify GPU resource lifecycle management
- [ ] Test performance with large datasets (100K+ points)
- [ ] Validate cross-platform consistency

### Performance Tests

```rust
#[bench]
fn bench_selection_render_10k_points(b: &mut Bencher) {
    let selection = create_large_selection(10_000);
    b.iter(|| {
        selection.render().unwrap();
    });
}
```

## Success Metrics

### Functional Requirements

- [ ] **Type Safety**: 100% of type mismatches caught at compile time
- [ ] **Performance**: 10K points render in <1ms on mid-range GPU
- [ ] **Memory Efficiency**: GPU memory usage scales linearly with data size
- [ ] **API Usability**: Selection API passes developer usability testing

### Quality Requirements

- [ ] **Test Coverage**: >90% test coverage for all public methods
- [ ] **Documentation**: Complete rustdoc with usage examples
- [ ] **Error Handling**: All error conditions have clear, actionable messages
- [ ] **Cross-Platform**: Identical behavior on Windows, macOS, Linux, and
      WebAssembly

## Risk Assessment

### Technical Risks

- **High**: GPU buffer management complexity could introduce memory leaks
- **Medium**: Shader pipeline generation may have performance bottlenecks
- **Medium**: Type system integration might be overly complex

### Mitigation Strategies

- **Automated Testing**: Comprehensive memory leak detection in CI
- **Performance Monitoring**: Continuous benchmarking of shader generation
- **Incremental Implementation**: Start with simple cases, add complexity
  gradually

## Implementation Notes

### Design Decisions

- Use `Arc<GupContext>` for shared rendering context across selections
- Store original data in `Vec<T>` on CPU for easy access and updates
- Implement lazy shader pipeline generation for performance
- Use phantom data for mark type to maintain type information

### Memory Management Strategy

- GPU buffers allocated on first render, resized as needed
- Automatic cleanup when selection dropped
- Reference counting for shared resources
- Buffer reuse for selections with same data types

### Performance Optimizations

- Batch multiple attribute updates before regenerating shader pipeline
- Cache generated shaders by attribute signature
- Use storage buffers for large datasets
- Implement compute shader fallback for complex transformations

## Definition of Done

- [ ] Selection type compiles and passes all tests
- [ ] GPU buffer management works correctly with no memory leaks
- [ ] Attribute binding system validates types at compile time
- [ ] Performance benchmarks meet <1ms render target for 10K points
- [ ] Integration tests pass with multiple data and mark types
- [ ] Documentation includes comprehensive usage examples
- [ ] Code review completed and approved

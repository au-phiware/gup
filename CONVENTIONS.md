# Gup Development Conventions

This document captures key learnings, patterns, and conventions discovered
during the development of the Gup visualization library.

## Rust Design Patterns

### Prefer Enums Over Trait Objects for Known Sets

**Learning**: When implementing extensible behavior with a finite, known set of
variants, prefer enums over trait objects (`Box<dyn Trait>`).

**Example**: In GUP-021, we initially tried:

```rust
// ❌ Problematic - trait not object-safe due to generic methods
trait CustomCompositionBehavior {
    fn compose<A: Mixable, B: Mixable>(...) -> GupResult<()>;
}
custom_behavior: Option<Box<dyn CustomCompositionBehavior>>,
```

**Solution**:

```rust
// ✅ Better - enum-based approach
#[derive(Debug, Clone)]
enum CustomCompositionBehavior {
    CrossFade(CrossFadeComposition),
    GridLayout(GridLayoutComposition),
}
```

**Benefits**:

- Compile-time type safety
- Better performance (no vtable indirection)
- Easier to serialize/deserialize
- Pattern matching exhaustiveness checking

**When to use trait objects**: When you need true open extensibility where
external crates can add implementations.

### Generic Method Limitations

**Learning**: Traits with generic methods cannot be made into trait objects due
to Rust's object safety rules.

**Guideline**: If you need both trait objects and generic methods, consider:

1. Separate the generic methods into a different trait
2. Use an enum-based approach for known variants
3. Use associated types instead of generic parameters where possible

## API Design Patterns

### Fluent APIs with Backward Compatibility

**Learning**: When extending existing APIs, maintain backward compatibility
while providing new convenience methods.

**Pattern**:

```rust
// Existing API continues to work
let composed = chart1.mix(chart2);

// New convenience methods added via extension traits
let overlay = chart1.overlay(chart2);
let beside = chart1.beside_with_config(chart2, config);
```

**Guidelines**:

- Use extension traits for new convenience methods
- Keep core trait minimal and stable
- Provide both simple defaults and configurable variants

### Configuration Structs with Defaults

**Learning**: Complex configuration is best handled with dedicated structs that
implement `Default`.

**Pattern**:

```rust
#[derive(Debug, Clone)]
pub struct SideBySideConfig {
    pub direction: LayoutDirection,
    pub split_ratio: f32,
    pub padding: f32,
}

impl Default for SideBySideConfig {
    fn default() -> Self {
        Self {
            direction: LayoutDirection::Horizontal,
            split_ratio: 0.5,
            padding: 10.0,
        }
    }
}
```

**Benefits**:

- Easy to extend without breaking changes
- Clear documentation of options
- Sensible defaults reduce API complexity

## Graphics Programming Patterns

### Viewport State Management

**Learning**: Rendering operations that modify global state (like viewport) must
restore original state.

**Pattern**:

```rust
fn render_with_viewport(&mut self, context: &mut RenderContext) -> GupResult<()> {
    let original_viewport = context.viewport();

    // Modify viewport for this operation
    context.set_viewport(new_viewport)?;

    // Do rendering work
    self.component.render(context)?;

    // Always restore original state
    context.set_viewport(original_viewport)?;

    Ok(())
}
```

**Guidelines**:

- Always capture original state before modifications
- Use RAII patterns where possible (consider viewport guards)
- Document state modifications clearly

### Coordinate System Conventions

**Learning**: Be explicit about coordinate system conventions and document them
clearly.

**Convention**: Grid layouts use `(row, col)` indexing where:

- Rows: 0 to `num_rows - 1` (top to bottom)
- Columns: 0 to `num_cols - 1` (left to right)

**Pattern**:

```rust
pub struct GridPosition {
    pub row: u32,    // 0-based row index
    pub col: u32,    // 0-based column index
}

// Always validate bounds
fn is_valid_position(&self, pos: GridPosition) -> bool {
    pos.row < self.rows && pos.col < self.cols
}
```

## Testing Patterns

### Comprehensive Composition Testing

**Learning**: When implementing composition systems, test all combinations and
edge cases.

**Test Categories**:

1. **Basic functionality** - Each mode works independently
2. **Configuration validation** - Invalid configs are rejected
3. **State management** - Viewport/state restoration works
4. **Error propagation** - Failures are handled correctly
5. **Integration** - Different modes work together
6. **Edge cases** - Boundary conditions and limits

**Pattern**:

```rust
#[tokio::test]
async fn test_viewport_restoration() {
    let mut context = RenderContext::new().await.unwrap();
    let original_viewport = context.viewport();

    let mut composition = create_side_by_side_composition();
    composition.render(&mut context).unwrap();

    // Verify viewport was restored
    assert_eq!(context.viewport(), original_viewport);
}
```

### Performance Regression Testing

**Learning**: When adding abstraction layers, benchmark to ensure performance
overhead stays minimal.

**Guidelines**:

- Benchmark direct operations vs. composed operations
- Target <1% overhead for composition layers
- Use `cargo bench` with consistent test data
- Monitor for performance regressions in CI

## Error Handling Patterns

### Context-Rich Error Messages

**Learning**: Composition errors should provide context about which component
failed and why.

**Pattern**:

```rust
// ❌ Not helpful
Err(GupError::RenderError("Component invalid".to_string()))

// ✅ Better - includes context
Err(GupError::CompositionError(format!(
    "First component is invalid: {}",
    self.first.description()
)))
```

**Guidelines**:

- Include component descriptions in error messages
- Specify which part of a composition failed
- Provide actionable information where possible

## Documentation Standards

### Code Examples in Documentation

**Learning**: Complex APIs benefit from comprehensive examples showing common
usage patterns.

**Pattern**:

````rust
/// Example: Creating compositions with different modes
///
/// ```rust
/// use gup::*;
///
/// // Basic overlay
/// let overlay = chart1.overlay(chart2);
///
/// // Configured side-by-side
/// let config = SideBySideConfig {
///     direction: LayoutDirection::Horizontal,
///     split_ratio: 0.6,
///     padding: 20.0,
/// };
/// let beside = chart1.beside_with_config(chart2, config);
/// ```
````

**Guidelines**:

- Provide runnable examples in doc comments
- Show both simple and advanced usage
- Include common configuration patterns
- Test examples with `cargo test --doc`

## Performance Considerations

### Lazy Evaluation Patterns

**Learning**: Composition systems benefit from lazy evaluation - defer expensive
operations until render time.

**Pattern**:

```rust
// ✅ Composition is cheap - just stores components
let composition = chart1.mix(chart2).mix(chart3);

// ✅ Expensive work happens only at render time
composition.render(&mut context)?;
```

### Viewport Calculation Caching

**Learning**: Repeated viewport calculations can be expensive - consider caching
when viewport doesn't change.

**Future Optimization**:

```rust
struct CachedViewportCalculation {
    original_viewport: Viewport,
    split_viewports: Option<(Viewport, Viewport)>,
    config_hash: u64,
}
```

## Architecture Decisions

### Composition Over Inheritance

**Learning**: Rust's trait system encourages composition patterns over
inheritance hierarchies.

**Pattern**: The `Mixable` trait enables universal composability:

```rust
// Any two Mixable types can be composed
let result = anything.mix(anything_else);

// Compositions are themselves Mixable
let complex = a.mix(b).mix(c.mix(d));
```

### Type System as Documentation

**Learning**: Well-designed types serve as documentation and prevent errors.

**Example**:

```rust
// ✅ Intent is clear from types
pub fn beside_with_config(
    self,
    other: T,
    config: SideBySideConfig
) -> ComposedVisualization<Self, T>

// ❌ Less clear
pub fn beside_with_config(
    self,
    other: T,
    direction: u8,
    ratio: f32,
    padding: f32
) -> ComposedVisualization<Self, T>
```

## GPU Programming Patterns (GUP-002 Learnings)

### Type-Safe Shader Function System

**Learning**: GPU programming benefits from compile-time type validation to
prevent runtime shader errors.

**Pattern**: Use phantom types to preserve type information across GPU
boundaries:

```rust
// ✅ Phantom types preserve T for compile-time validation
pub struct PositionShaderFunction<F, T> {
    extractor: F,
    _phantom: PhantomData<T>,
}

impl<F, T> ShaderFunction for PositionShaderFunction<F, T>
where
    F: Fn(&T) -> [f32; 2] + Send + Sync + 'static,
    T: Send + Sync + 'static,  // 'static required for GPU storage
{
    type Input = T;
    type Output = [f32; 2];
}
```

**Benefits**:

- Compile-time validation of data-to-attribute mappings
- Type safety across CPU-GPU boundary
- Clear error messages for invalid shader function bindings

### Buffer Growth Strategies (GUP-002 Pattern)

**Learning**: GPU buffers should grow efficiently to handle dynamic data sizes
without frequent reallocations.

**Pattern**: Use growth factor of 1.5x for optimal memory vs. performance
trade-off:

```rust
pub fn write(&mut self, device: &Device, queue: &Queue, data: &[T]) -> GupResult<()> {
    if data.len() > self.capacity {
        // 1.5x growth factor balances memory usage and reallocation frequency
        let new_capacity = (data.len() as f64 * 1.5) as usize;
        self.resize_buffer(device, new_capacity)?;
    }
    queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(data));
    Ok(())
}
```

**Guidelines**:

- Provide both auto-resize and fixed-size options
- Use exponential growth (1.5x) to minimize allocations
- Always validate buffer capacity before writing
- Return clear error messages on capacity failures

### GPU Resource Context Sharing

**Learning**: WebGPU resources (Device, Queue) must be shared efficiently to
avoid resource conflicts.

**Problem**: Creating multiple contexts causes buffer ownership issues:

```rust
// ❌ Problematic - multiple contexts
let device1 = &RenderContext::new().await.unwrap().device().clone();
let device2 = &RenderContext::new().await.unwrap().device().clone();
// Results in: "Buffer does not exist" errors
```

**Solution**: Reuse single context for related operations:

```rust
// ✅ Better - single shared context
let context = RenderContext::new().await.unwrap();
let device = context.device();
let queue = context.queue();
```

### Lifetime Management for GPU Traits

**Learning**: GPU-bound traits require `'static` lifetimes for safe storage and
transfer.

**Pattern**: Add `'static` bounds to all GPU-destined types:

```rust
// ✅ Explicit 'static for GPU safety
impl<F, T> ShaderFunction for PositionShaderFunction<F, T>
where
    F: Fn(&T) -> [f32; 2] + Send + Sync + 'static,
    T: Send + Sync + 'static,  // Required for GPU storage
```

**Rationale**:

- GPU operations are asynchronous and may outlive stack frames
- Shader functions must be stored for pipeline compilation
- Static lifetimes ensure data remains valid during GPU processing

### Performance Benchmarking for GPU Code

**Learning**: GPU code requires different benchmarking approaches than CPU-only
code.

**Pattern**: Test both small-scale and large-scale performance:

```rust
#[tokio::test]
async fn bench_selection_render_10k_points() {
    let large_data: Vec<Data> = generate_test_data(10_000);
    let mut selection = Selection::new(large_data, context).unwrap();

    let start = std::time::Instant::now();
    let result = selection.render();
    let duration = start.elapsed();

    assert!(result.is_ok());
    // GPU operations should handle large datasets efficiently
    assert!(duration.as_secs() < 1);
}
```

**Guidelines**:

- Test with datasets of 1K, 10K, and 100K+ elements
- Measure both CPU preparation and GPU execution time
- Include buffer allocation overhead in benchmarks
- Set realistic performance targets based on GPU capabilities

### Type Compatibility Systems

**Learning**: Flexible type compatibility checks enable gradual typing while
maintaining safety.

**Pattern**: Use marker traits for opt-in compatibility validation:

```rust
// ✅ Flexible compatibility with reasonable defaults
pub trait Compatible<T> {
    fn is_compatible() -> bool { true }  // Permissive default
}

// Blanket implementation allows most combinations
impl<T> Compatible<T> for T {}

// Can be specialized for strict validation when needed
impl Compatible<CircleAttributes> for [f32; 2] {
    fn is_compatible() -> bool { false }  // Strict validation
}
```

## Testing Strategies for GPU Code

### Context Reuse in Test Suites

**Learning**: GPU tests must carefully manage context creation to avoid resource
conflicts.

**Pattern**: Create context once per test, reuse for all operations:

```rust
#[tokio::test]
async fn test_gpu_operations() {
    let context = RenderContext::new().await.unwrap();  // Single context
    let device = context.device();
    let queue = context.queue();

    // All operations use same context
    let mut buffer1 = GpuBuffer::new(device, 100, usage);
    let mut buffer2 = GpuBuffer::new(device, 200, usage);
}
```

### Comprehensive GPU Resource Testing

**Learning**: GPU resource management requires testing edge cases that don't
exist in CPU-only code.

**Test Categories**:

1. **Resource Creation**: Buffers, textures, pipelines
2. **Resource Growth**: Auto-resizing, capacity limits
3. **Resource Sharing**: Multiple components using same context
4. **Resource Cleanup**: Proper disposal, memory leaks
5. **Error Conditions**: Out of memory, invalid operations

## GPU Buffer Management Patterns (GUP-003 Learnings)

### Type-Safe Buffer Management with Enums

**Learning**: Use enums to categorize buffer types rather than generic
parameters, enabling type-safe buffer creation with proper wgpu usage flags.

**Pattern**: Buffer type enum with associated behavior:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BufferType {
    Vertex,    // Vertex attributes and geometry
    Instance,  // Per-instance data for instanced rendering
    Uniform,   // Shader uniforms (small, frequently updated)
    Storage,   // Large datasets accessed by shaders
}

impl BufferType {
    pub fn usage_flags(self) -> BufferUsages {
        match self {
            BufferType::Vertex => BufferUsages::VERTEX | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
            BufferType::Instance => BufferUsages::VERTEX | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
            BufferType::Uniform => BufferUsages::UNIFORM | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
            BufferType::Storage => BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
        }
    }
}
```

**Benefits**:

- Automatic correct usage flags for each buffer type
- Compile-time validation of buffer operations
- Clear categorization of buffer purposes
- Easy to extend with new buffer types

### GPU Buffer Auto-Resizing Strategy

**Learning**: GPU buffers require COPY_SRC usage flags for resize operations,
and 1.5x growth provides optimal balance.

**Pattern**: Auto-resizing buffer with proper usage flags:

```rust
pub struct GpuBuffer<T> {
    buffer: Buffer,
    capacity: usize,
    len: usize,
    buffer_type: BufferType,
    usage: BufferUsages, // Must include COPY_SRC for resize operations
    _phantom: PhantomData<T>,
}

impl<T: bytemuck::Pod + bytemuck::Zeroable> GpuBuffer<T> {
    pub fn upload(&mut self, device: &Device, queue: &Queue, data: &[T]) -> GupResult<()> {
        if data.len() > self.capacity {
            self.resize(device, queue, data.len())?;  // 1.5x growth factor
        }
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(data));
        self.len = data.len();
        Ok(())
    }
}
```

**Critical Requirements**:

- All buffer types need `COPY_SRC` usage for resize operations
- Use 1.5x growth factor for optimal memory vs performance trade-off
- Always validate capacity before operations
- Provide both auto-resize and fixed-size options

### Buffer Pool Memory Management

**Learning**: Size-class based allocation with power-of-2 rounding provides
efficient memory reuse while minimizing fragmentation.

**Pattern**: Pool with size classes and statistics:

```rust
pub struct BufferPool {
    pools: HashMap<(BufferType, usize), Vec<Buffer>>,
    device: Arc<Device>,
    allocation_stats: AllocationStats,
}

impl BufferPool {
    fn calculate_size_class(&self, capacity: usize) -> usize {
        if capacity == 0 { return 1; }
        // Round up to next power of 2
        let mut size_class = 1;
        while size_class < capacity {
            size_class *= 2;
        }
        size_class
    }
}
```

**Benefits**:

- Reduces allocation overhead through reuse
- Power-of-2 size classes minimize fragmentation
- Statistics enable pool efficiency monitoring
- Automatic cleanup prevents memory leaks

### wgpu API Compatibility Patterns

**Learning**: Different wgpu versions have different API signatures and async
patterns that require careful handling.

**Key Considerations**:

- Buffer download operations are complex and may not be needed for core
  functionality
- Device polling APIs vary between wgpu versions
- Always include proper usage flags from the start to avoid refactoring
- Use staging buffers for CPU-GPU data transfers

**Defensive Pattern**:

```rust
// Start with comprehensive usage flags to avoid later issues
let usage = BufferUsages::VERTEX | BufferUsages::COPY_DST | BufferUsages::COPY_SRC;

// Provide simplified download API that can be implemented later
pub async fn download(&self, _device: &Device, _queue: &Queue) -> GupResult<Vec<T>> {
    Err(GupError::BufferError(
        "Buffer download not yet implemented - use for upload/rendering only".to_string()
    ))
}
```

### Performance Testing for GPU Code

**Learning**: GPU buffer operations require different benchmarking approaches
and realistic test scenarios.

**Testing Strategy**:

```rust
#[tokio::test]
async fn test_buffer_auto_resize_performance() {
    let context = create_test_context().await;
    let mut buffer = GpuBuffer::new(context.device(), BufferType::Storage, 100);

    // Test with progressively larger datasets
    for size in [1_000, 10_000, 100_000] {
        let data: Vec<f32> = (0..size).map(|i| i as f32).collect();
        let start = std::time::Instant::now();
        buffer.upload(context.device(), context.queue(), &data).unwrap();
        let duration = start.elapsed();

        // GPU operations should handle large datasets efficiently
        assert!(duration.as_millis() < 100);
    }
}
```

**Guidelines**:

- Test with realistic dataset sizes (1K, 10K, 100K+ elements)
- Measure both buffer creation and upload performance
- Include resize overhead in benchmarks
- Test pool efficiency with allocation/deallocation cycles

### Integration Testing with Existing Systems

**Learning**: When adding new systems, carefully refactor existing code to use
new APIs while maintaining backward compatibility.

**Pattern**: Gradual migration strategy:

```rust
// 1. Create new API alongside old
use crate::buffer::GpuBuffer as BufferGpuBuffer;

// 2. Update struct to use new type
pub struct Selection<T, M: Mark> {
    vertex_buffer: Option<BufferGpuBuffer<M::Vertex>>,  // New API
    instance_buffer: Option<BufferGpuBuffer<InstanceData>>,  // New API
}

// 3. Update method calls to new API
vertex_buffer.upload(device, queue, &vertices)?;  // New simplified API
```

**Migration Benefits**:

- Maintains existing functionality during transition
- Allows testing new system independently
- Reduces risk of breaking changes
- Enables gradual adoption

---

_This document is a living record of learnings. Update it as new patterns and
conventions are discovered._

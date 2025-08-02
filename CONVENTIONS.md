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

## Render Context Architecture Patterns (GUP-004 Learnings)

### Unified Context Design

**Learning**: A single, comprehensive render context provides better resource
management and developer experience than multiple specialized contexts.

**Pattern**: The `GupContext` unifies all GPU resources under one management
system:

```rust
// ✅ Unified approach - single context manages everything
pub struct GupContext {
    pub device: Arc<Device>,
    pub queue: Arc<Queue>,
    surface: Option<Surface<'static>>,
    buffer_pool: BufferPool,
    texture_pool: TexturePool,
    frame_stats: FrameStats,
}

// ✅ Simple, consistent API across all components
let context = GupContext::headless().await?;
let buffer = context.create_buffer(BufferType::Vertex, 1000);
let frame = context.begin_frame()?;
```

**Benefits**:

- Single point of resource management
- Consistent API across all GPU operations
- Easier debugging and performance monitoring
- Simplified sharing between components

### Arc-Based Resource Sharing

**Learning**: Use `Arc<Device>` and `Arc<Queue>` for safe sharing of GPU
resources across components without lifetime complications.

**Pattern**: Wrap core GPU resources in Arc for sharing:

```rust
// ✅ Arc enables sharing without lifetime parameters
pub struct GupContext {
    pub device: Arc<Device>,
    pub queue: Arc<Queue>,
}

// Resources can be safely shared across components
let device_ref = Arc::clone(&context.device);
let buffer_pool = BufferPool::new(device_ref);
```

**Guidelines**:

- Use Arc for device and queue to enable sharing
- Keep Arc clones lightweight - they only increment reference counts
- Avoid Arc for resources that don't need sharing (textures, buffers)

### Multi-Modal Initialization Strategy

**Learning**: Provide multiple initialization paths to handle different use
cases (headless, windowed, custom options) with sensible defaults.

**Pattern**: Multiple constructors with clear naming:

```rust
impl GupContext {
    // Simple default initialization
    pub async fn new() -> GupResult<Arc<Self>>;

    // Specific use cases
    pub async fn headless() -> GupResult<Arc<Self>>;
    pub async fn with_surface<W>(window: Arc<W>) -> GupResult<Arc<Self>>;

    // Advanced customization
    pub async fn with_options(options: GupOptions) -> GupResult<Arc<Self>>;
}
```

**Benefits**:

- Clear intent from method names
- Sensible defaults reduce complexity
- Advanced options available when needed
- Consistent async patterns

### Frame Lifecycle Management

**Learning**: RAII (Resource Acquisition Is Initialization) patterns work well
for GPU frame management, ensuring proper cleanup.

**Pattern**: Frame objects that enforce proper lifecycle:

```rust
// ✅ Frame lifecycle enforced by type system
pub struct RenderFrame<'a> {
    context: &'a mut GupContext,
    surface_texture: Option<SurfaceTexture>,
    command_encoder: CommandEncoder,
}

impl<'a> RenderFrame<'a> {
    pub fn finish(self) -> GupResult<()> {
        // Automatic cleanup and presentation
        let command_buffer = self.command_encoder.finish();
        self.context.queue.submit(Some(command_buffer));
        if let Some(output) = self.surface_texture {
            output.present();
        }
        self.context.finish_frame(); // Update stats
        Ok(())
    }
}
```

**Guidelines**:

- Use consuming methods (`finish(self)`) to enforce single-use
- Combine resource cleanup with lifecycle methods
- Update performance statistics automatically
- Provide both surface and offscreen rendering paths

### Cross-Platform WebGPU Patterns

**Learning**: WebAssembly requires different backend selection and feature
detection than native platforms.

**Pattern**: Conditional compilation for platform-specific behavior:

```rust
impl Default for GupOptions {
    fn default() -> Self {
        Self {
            #[cfg(target_arch = "wasm32")]
            backends: Backends::BROWSER_WEBGPU | Backends::GL,
            #[cfg(not(target_arch = "wasm32"))]
            backends: Backends::PRIMARY,
            // ... other fields
        }
    }
}
```

**Testing Strategy**: Separate test functions for different platforms:

```rust
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen_test::wasm_bindgen_test]
async fn test_wasm_context_creation() { /* ... */ }

#[cfg(not(target_arch = "wasm32"))]
#[tokio::test]
async fn test_native_context_creation() { /* ... */ }
```

### Performance Monitoring Integration

**Learning**: Built-in performance monitoring provides valuable insights without
requiring external profiling tools.

**Pattern**: Integrated statistics collection with moving averages:

```rust
pub struct FrameStats {
    pub frames_rendered: u64,
    pub avg_frame_time: f32,
    pub current_frame_time: f32,
    pub gpu_memory_usage: u64,
}

impl FrameStats {
    pub fn update_frame_time(&mut self, frame_time: Duration) {
        let frame_time_ms = frame_time.as_secs_f32() * 1000.0;
        // Moving average: 90% old + 10% new
        self.avg_frame_time = (self.avg_frame_time * 0.9) + (frame_time_ms * 0.1);
        // ... update other stats
    }
}
```

**Benefits**:

- Real-time performance feedback during development
- Built-in FPS calculation
- Memory usage tracking from buffer pools
- No external dependencies required

### Resource Pool Integration Strategies

**Learning**: Integrating resource pools directly into the context provides
convenience while maintaining performance.

**Pattern**: Context owns pools and provides convenient access:

```rust
impl GupContext {
    // Direct pool access for advanced use
    pub fn buffer_pool(&mut self) -> &mut BufferPool;

    // Convenience methods for common operations
    pub fn create_buffer<T>(&mut self, buffer_type: BufferType, capacity: usize) -> GpuBuffer<T> {
        self.buffer_pool.allocate(buffer_type, capacity)
    }
}
```

**Guidelines**:

- Provide both direct pool access and convenience methods
- Update performance statistics from pool metrics
- Use pools for automatic memory management
- Clean up unused resources periodically

### Error Handling for GPU Operations

**Learning**: GPU operations have unique failure modes that require specific
error handling strategies.

**Pattern**: Structured error types with context:

```rust
#[derive(Debug, Clone)]
pub enum GupError {
    WebGpuError(String),    // GPU/adapter failures
    ResourceError(String),  // Resource allocation failures
    RenderError(String),    // Rendering operation failures
}

// Provide context in error messages
.map_err(|e| GupError::WebGpuError(format!("Failed to create device: {e}")))?;
```

**Guidelines**:

- Include original error information
- Provide actionable error messages
- Use Result types consistently
- Handle async errors properly

### Testing Strategies for GPU Code (GUP-004)

**Learning**: GPU code requires different testing approaches than pure CPU code.

**Test Categories**:

1. **Resource Creation**: Verify contexts and resources are created successfully
2. **Lifecycle Management**: Test proper cleanup and state management
3. **Performance Validation**: Ensure operations meet performance targets
4. **Cross-Platform Compatibility**: Separate tests for native and WebAssembly
5. **Integration Testing**: Verify interaction with existing systems

**Pattern**: Context reuse and performance validation:

```rust
#[tokio::test]
async fn test_frame_stats_tracking() {
    let context = GupContext::headless().await.unwrap();
    let mut ctx = Arc::try_unwrap(context).unwrap();

    // Render multiple frames
    for _ in 0..3 {
        let frame = ctx.begin_frame().unwrap();
        frame.finish().unwrap();
    }

    let stats = ctx.frame_stats();
    assert_eq!(stats.frames_rendered, 3);
    assert!(stats.avg_frame_time >= 0.0);
}
```

## Multi-Window Surface Management Patterns (GUP-039 Learnings)

### Unique Identifier Systems for Resources

**Learning**: Multi-resource management requires robust ID systems with atomic
generation and clear display formatting.

**Pattern**: Use atomic counters with type-safe wrappers:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SurfaceId(pub u64);

impl SurfaceId {
    pub fn new() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        Self(COUNTER.fetch_add(1, Ordering::Relaxed))
    }
}

impl std::fmt::Display for SurfaceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Surface({})", self.0)
    }
}
```

**Benefits**:

- Thread-safe ID generation
- Type safety prevents ID confusion
- Clear debugging with Display trait
- Hash/Eq enable HashMap usage

### Multi-Resource State Management

**Learning**: Managing multiple similar resources requires careful state
tracking and primary resource concepts for backward compatibility.

**Pattern**: Use HashMap with optional primary selection:

```rust
pub struct ResourceManager<T> {
    resources: HashMap<ResourceId, ManagedResource<T>>,
    primary_id: Option<ResourceId>,
}

impl<T> ResourceManager<T> {
    pub fn add(&mut self, id: ResourceId, resource: T) -> Result<()> {
        // Set as primary if first resource
        if self.primary_id.is_none() {
            self.primary_id = Some(id);
        }
        self.resources.insert(id, ManagedResource::new(resource));
    }

    pub fn remove(&mut self, id: ResourceId) -> Result<()> {
        self.resources.remove(&id);
        // Update primary if removed
        if self.primary_id == Some(id) {
            self.primary_id = self.resources.keys().next().copied();
        }
    }
}
```

**Guidelines**:

- Provide both ID-specific and primary resource access
- Handle primary resource updates automatically
- Include comprehensive error messages with resource IDs

### Surface Format Negotiation Strategies

**Learning**: Cross-platform graphics requires robust format negotiation with
clear preference hierarchies and fallback strategies.

**Pattern**: Preference-based negotiation with fallbacks:

```rust
fn negotiate_surface_format(&self, caps: &SurfaceCapabilities) -> GupResult<TextureFormat> {
    // Prefer sRGB formats for color accuracy
    let preferred_formats = [
        TextureFormat::Bgra8UnormSrgb,
        TextureFormat::Rgba8UnormSrgb,
        TextureFormat::Bgra8Unorm,
        TextureFormat::Rgba8Unorm,
    ];

    for format in &preferred_formats {
        if caps.formats.contains(format) {
            return Ok(*format);
        }
    }

    // Fallback to first available format
    caps.formats.first().copied().ok_or_else(|| {
        GupError::WebGpuError("No supported surface formats found".to_string())
    })
}
```

**Benefits**:

- Consistent format selection across platforms
- Clear preference ordering for quality
- Graceful degradation with fallbacks
- Informative error messages

### Performance-Critical API Design

**Learning**: Multi-window systems must meet strict performance requirements
(<16ms resize) through efficient resource management.

**Performance Requirements**:

- Surface resize: <16ms for responsive UI
- Frame rendering: Target 60+ FPS (16.67ms budget)
- Resource lookup: O(1) through HashMap usage
- Memory allocation: Minimize during frame rendering

**Pattern**: Performance validation in tests:

```rust
#[tokio::test]
async fn test_surface_resize_performance() {
    let start = std::time::Instant::now();
    ctx.resize_surface(id, PhysicalSize::new(1024, 768))?;
    let duration = start.elapsed();

    // Should complete well under 16ms for responsive UI
    assert!(duration.as_millis() < 16);
}
```

### Arc-Based Resource Sharing Patterns

**Learning**: Multi-window applications require careful resource sharing
patterns to avoid lifetime complications while enabling efficient access.

**Problem**: Multiple windows need access to shared GPU context:

```rust
// ❌ Problematic - lifetime parameters propagate
struct WindowManager<'a> {
    context: &'a mut GupContext,
    windows: HashMap<WindowId, Window>,
}
```

**Solution**: Use Arc with take/restore pattern:

```rust
// ✅ Better - Arc enables sharing without lifetimes
struct WindowManager {
    context: Option<Arc<GupContext>>,
    windows: HashMap<WindowId, WindowInfo>,
}

impl WindowManager {
    fn operation(&mut self) -> Result<()> {
        if let Some(context) = self.context.take() {
            let mut ctx = Arc::try_unwrap(context)?;
            // Perform mutable operations
            ctx.some_operation()?;
            self.context = Some(Arc::new(ctx));
        }
        Ok(())
    }
}
```

**Guidelines**:

- Use Arc for shared GPU resources (Device, Queue)
- Use take/restore pattern for mutable access
- Validate Arc::try_unwrap success for exclusive access
- Provide clear error messages for sharing violations

### Comprehensive Error Context for Multi-Resource Systems

**Learning**: Multi-resource systems require rich error context including
resource IDs and operation context.

**Pattern**: Structured errors with resource context:

```rust
// ✅ Rich error context with resource information
fn resize_surface(&mut self, id: SurfaceId, size: PhysicalSize<u32>) -> GupResult<()> {
    let surface = self.surfaces.get_mut(&id).ok_or_else(|| {
        GupError::ResourceError(format!("Surface with ID {id} not found"))
    })?;

    surface.resize(&self.device, size.width, size.height);
    Ok(())
}
```

**Benefits**:

- Clear identification of which resource failed
- Actionable error messages for debugging
- Consistent error formatting across operations

### Testing Strategies for Multi-Resource Systems

**Learning**: Multi-resource systems require comprehensive testing of resource
lifecycle, error conditions, and performance requirements.

**Test Categories**:

1. **Resource Lifecycle**: Creation, modification, removal
2. **Error Handling**: Invalid IDs, conflicting operations
3. **Performance Validation**: Response time requirements
4. **Cross-Platform Compatibility**: Format negotiation, capabilities
5. **Concurrent Access**: Resource sharing patterns

**Pattern**: Comprehensive test coverage:

```rust
#[tokio::test]
async fn test_multi_resource_lifecycle() {
    let mut manager = ResourceManager::new();

    // Test creation
    let id1 = ResourceId::new();
    assert!(manager.add(id1, resource1).is_ok());

    // Test error conditions
    assert!(manager.remove(ResourceId::new()).is_err());

    // Test performance
    let start = std::time::Instant::now();
    manager.operation(id1)?;
    assert!(start.elapsed().as_millis() < 16);
}
```

### Real-World Application Integration Patterns

**Learning**: Graphics libraries must provide both low-level control and
high-level convenience APIs for different use cases.

**Pattern**: Multi-level API design:

```rust
// Low-level: Full control
ctx.add_surface(surface_id, window)?;
ctx.begin_frame_for_surface(surface_id)?;

// High-level: Convenience with reasonable defaults
let context = GupContext::with_surface(window).await?;
let frame = ctx.begin_frame()?; // Uses primary surface
```

**Benefits**:

- Experts can optimize with low-level APIs
- Beginners can use high-level convenience methods
- Backward compatibility through primary resource concept
- Clear migration path from simple to complex usage

## GPU Blend State Integration Patterns (GUP-027 Learnings)

### Hash-Capable Enums for Pipeline Caching

**Learning**: WebGPU pipeline caching requires blend mode enums that implement
`Hash` for efficient `HashMap<BlendMode, RenderPipeline>` storage.

**Pattern**: Add Hash derive to GPU-related enums:

```rust
// ✅ Hash derive enables efficient pipeline caching
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum BlendMode {
    #[default]
    None,
    AlphaBlending,
    Additive,
    Multiply,
}

// Pipeline cache becomes efficient HashMap lookup
pipeline_cache: HashMap<BlendMode, RenderPipeline>
```

**Benefits**:

- O(1) pipeline lookups by blend mode
- Automatic cache management through HashMap
- Type-safe pipeline associations
- Easy extension with new blend modes

### GPU State Stack Management

**Learning**: Complex rendering systems require state stack management to handle
nested compositions with proper state restoration.

**Pattern**: Push/pop state management with automatic restoration:

```rust
// ✅ RAII-style state management
impl RenderContext {
    pub fn push_blend_state(&mut self) -> GupResult<()> {
        self.blend_state_stack.push(self.current_blend_mode);
        Ok(())
    }

    pub fn pop_blend_state(&mut self) -> GupResult<()> {
        if let Some(previous_mode) = self.blend_state_stack.pop() {
            self.set_blend_mode(previous_mode)?;
        }
        Ok(())
    }
}

// Usage in composition systems
context.push_blend_state()?;
context.set_blend_mode(BlendMode::AlphaBlending)?;
// ... render operations
context.pop_blend_state()?; // Automatic restoration
```

**Guidelines**:

- Always push state before modifications
- Use stack-based management for nested operations
- Handle empty stack gracefully (no-op for robustness)
- Consider RAII guards for automatic cleanup

### WebGPU BlendState Configuration Patterns

**Learning**: WebGPU blend state configuration requires careful mapping from
high-level blend modes to low-level BlendComponent configurations.

**Pattern**: Comprehensive blend mode to WebGPU mapping:

```rust
fn blend_mode_to_wgpu(blend_mode: BlendMode) -> Option<BlendState> {
    match blend_mode {
        BlendMode::None => None, // No blending
        BlendMode::AlphaBlending => Some(BlendState::ALPHA_BLENDING),
        BlendMode::Additive => Some(BlendState {
            color: BlendComponent {
                src_factor: BlendFactor::One,
                dst_factor: BlendFactor::One,
                operation: BlendOperation::Add,
            },
            alpha: BlendComponent {
                src_factor: BlendFactor::One,
                dst_factor: BlendFactor::One,
                operation: BlendOperation::Add,
            },
        }),
        BlendMode::Multiply => Some(BlendState {
            color: BlendComponent {
                src_factor: BlendFactor::Dst,      // Multiply source by destination
                dst_factor: BlendFactor::Zero,      // Don't add destination
                operation: BlendOperation::Add,
            },
            alpha: BlendComponent {
                src_factor: BlendFactor::One,
                dst_factor: BlendFactor::OneMinusSrcAlpha,
                operation: BlendOperation::Add,
            },
        }),
    }
}
```

**Critical Considerations**:

- Different blend operations require different factor combinations
- Alpha and color channels may need separate treatment
- Test visual results, not just API compliance
- Consider performance implications of complex blend modes

### Global Alpha Uniform Buffer Management

**Learning**: Cross-fade and global alpha effects require uniform buffer systems
with proper alignment and efficient updates.

**Pattern**: Aligned uniform structures with lazy buffer creation:

```rust
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct GlobalAlphaUniform {
    alpha: f32,
    _padding: [f32; 3], // Ensure 16-byte alignment for uniforms
}

impl RenderContext {
    pub fn set_global_alpha(&mut self, alpha: f32) -> GupResult<()> {
        // Lazy buffer creation pattern
        if self.global_alpha_buffer.is_none() {
            self.create_global_alpha_buffer()?;
        }

        let uniform = GlobalAlphaUniform { alpha, _padding: [0.0; 3] };
        self.queue.write_buffer(
            self.global_alpha_buffer.as_ref().unwrap(),
            0,
            bytemuck::cast_slice(&[uniform])
        );
        Ok(())
    }
}
```

**Guidelines**:

- Always align uniform structures to 16-byte boundaries
- Use lazy creation to avoid unnecessary resource allocation
- Update buffers efficiently with write_buffer
- Include uniform buffers in bind group layouts

### Performance-Critical GPU State Changes

**Learning**: Blend state changes must be highly optimized as they occur
frequently during complex rendering.

**Performance Requirements**:

- Target: <0.1ms (100 microseconds) per blend state change
- Achieved: ~15ns average (well under target)
- Method: Early return for unchanged state + efficient pipeline caching

**Pattern**: Optimized state change with early returns:

```rust
pub fn set_blend_mode(&mut self, mode: BlendMode) -> GupResult<()> {
    // ✅ Early return prevents unnecessary work
    if self.current_blend_mode == mode {
        return Ok(());
    }

    self.current_blend_mode = mode;
    // Additional pipeline switching logic...
    Ok(())
}

// Performance testing in benchmarks
#[tokio::test]
async fn test_blend_mode_performance() {
    let start = std::time::Instant::now();
    for i in 0..1000 {
        let mode = match i % 4 {
            0 => BlendMode::None,
            1 => BlendMode::AlphaBlending,
            2 => BlendMode::Additive,
            _ => BlendMode::Multiply,
        };
        context.set_blend_mode(mode)?;
    }
    let duration = start.elapsed();
    assert!(duration.as_millis() < 1); // <1ms for 1000 changes
}
```

### Shader Integration with Global State

**Learning**: GPU shaders need careful integration with global state like alpha
uniforms while maintaining flexibility.

**Pattern**: Modular shader design with optional global state:

```wgsl
// Global alpha uniform (optional binding)
@group(0) @binding(0)
var<uniform> global_alpha: GlobalAlpha;

struct GlobalAlpha {
    alpha: f32,
    _padding: vec3<f32>,
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    var color = in.color;

    // Apply global alpha modulation
    color.a *= global_alpha.alpha;

    return color;
}
```

**Guidelines**:

- Design shaders to work with optional global state
- Use consistent binding group layouts across pipelines
- Include padding for proper uniform alignment
- Test shader compilation across different blend modes

### Testing Strategies for GPU Blend Systems

**Learning**: GPU blend state systems require comprehensive testing including
visual validation, performance testing, and resource management verification.

**Test Categories**:

1. **Functional Testing**: All blend modes work correctly
2. **State Management**: Push/pop operations maintain consistency
3. **Performance Testing**: Blend state changes meet timing requirements
4. **Resource Management**: Pipeline caching and buffer allocation
5. **Integration Testing**: Composition system integration
6. **Edge Cases**: Empty stacks, rapid state changes

**Pattern**: Multi-threaded test execution considerations:

```rust
// ✅ GPU tests may need single-threaded execution to avoid resource conflicts
cargo test -- --test-threads=1

// Test resource management
#[tokio::test]
async fn test_pipeline_caching() {
    let mut context = RenderContext::new().await.unwrap();

    // First access should create pipeline
    let _pipeline1 = context.get_pipeline_with_blend(BlendMode::AlphaBlending)?;
    assert_eq!(context.pipeline_cache_size(), 1);

    // Second access should reuse cached pipeline
    let _pipeline2 = context.get_pipeline_with_blend(BlendMode::AlphaBlending)?;
    assert_eq!(context.pipeline_cache_size(), 1); // No growth
}
```

**Testing Insights**:

- GPU tests may require single-threaded execution (`--test-threads=1`)
- Test both functional correctness and performance characteristics
- Validate resource cleanup and memory management
- Include integration tests with existing systems

### Composition System Integration Patterns

**Learning**: Blend state systems must integrate seamlessly with existing
composition systems while maintaining backward compatibility.

**Pattern**: Transparent integration with automatic state management:

```rust
// ✅ Overlay composition automatically manages blend state
impl<A: Mixable, B: Mixable> ComposedVisualization<A, B> {
    fn render_overlay(&mut self, context: &mut RenderContext) -> GupResult<()> {
        // Automatic state management - no user intervention required
        context.push_blend_state()?;

        self.first.render(context)?;                    // Background
        context.set_blend_mode(BlendMode::AlphaBlending)?;
        self.second.render(context)?;                   // Foreground with blending

        context.pop_blend_state()?;                     // Automatic restoration
        Ok(())
    }
}

// User API remains simple
let overlay = chart1.overlay(chart2); // Blend mode handled automatically
```

**Benefits**:

- Zero-configuration experience for common use cases
- Automatic state management prevents user errors
- Existing APIs continue to work unchanged
- Advanced users can still access low-level blend controls

### Debugging and Observability Patterns

**Learning**: Complex GPU state systems benefit from built-in observability for
debugging and performance optimization.

**Pattern**: Accessor methods for internal state inspection:

```rust
impl RenderContext {
    // ✅ Testing accessors for internal state inspection
    pub fn has_global_alpha_buffer(&self) -> bool {
        self.global_alpha_buffer.is_some()
    }

    pub fn pipeline_cache_size(&self) -> usize {
        self.pipeline_cache.len()
    }

    pub fn current_blend_mode(&self) -> BlendMode {
        self.current_blend_mode
    }
}
```

**Guidelines**:

- Provide read-only access to internal state for testing
- Include performance metrics (cache hit rates, timing)
- Use clear naming conventions for debugging methods
- Consider debug formatting for complex state structures

## Shader Function System Patterns (GUP-005 Learnings)

### Trait Naming Strategy for Avoiding Conflicts

**Learning**: When implementing similar traits in different modules, use
descriptive prefixes to avoid naming conflicts.

**Problem**: The `ShaderFunction` trait name conflicted with an existing trait
in the selection module.

**Solution**: Use specific naming like `ComposableShaderFunction` to indicate
the trait's purpose:

```rust
// ❌ Problematic - generic name causes conflicts
pub trait ShaderFunction { /* ... */ }

// ✅ Better - descriptive name indicates purpose
pub trait ComposableShaderFunction { /* ... */ }
```

**Guidelines**:

- Use descriptive trait names that indicate their specific purpose
- Check for existing trait names before implementation
- Consider module-specific prefixes when traits serve similar but distinct
  purposes

### Associated Type Defaults Stability

**Learning**: Associated type defaults are still unstable in Rust and should be
avoided in production code.

**Problem**: Using `type Uniforms: ... = ();` causes compilation errors.

**Solution**: Require explicit uniform types without defaults:

```rust
// ❌ Unstable feature
pub trait ComposableShaderFunction {
    type Uniforms: bytemuck::Pod + bytemuck::Zeroable = ();
}

// ✅ Stable - require explicit types
pub trait ComposableShaderFunction {
    type Uniforms: bytemuck::Pod + bytemuck::Zeroable;
}
```

### Generic Struct Trait Derivation Limitations

**Learning**: `bytemuck::Pod` and `bytemuck::Zeroable` cannot be automatically
derived for generic structs due to padding verification requirements.

**Problem**: Automatic derivation fails for `ChainUniforms<A, B>`.

**Solution**: Implement traits manually with proper bounds:

```rust
// ❌ Cannot derive for generic types
#[derive(bytemuck::Pod, bytemuck::Zeroable)]
pub struct ChainUniforms<A, B> { /* ... */ }

// ✅ Manual implementation with proper bounds
unsafe impl<A: bytemuck::Pod, B: bytemuck::Pod> bytemuck::Pod for ChainUniforms<A, B>
where
    A: bytemuck::Zeroable + Copy,
    B: bytemuck::Zeroable + Copy {}

unsafe impl<A: bytemuck::Zeroable, B: bytemuck::Zeroable> bytemuck::Zeroable for ChainUniforms<A, B>
where
    A: Copy,
    B: Copy {}
```

### Type System Composition Validation

**Learning**: Rust's type system can provide compile-time validation for complex
composition scenarios through trait bounds.

**Pattern**: Use marker traits for compatibility checking:

```rust
// ✅ Type-safe composition validation
pub trait TypeCompatible<T> {
    fn is_compatible() -> bool { true }
}

// Automatic compatibility for same types
impl<T> TypeCompatible<T> for T {}

// Composition only works with compatible types
pub struct FunctionChain<A: ComposableShaderFunction, B: ComposableShaderFunction>
where
    A::Output: TypeCompatible<B::Input>,
{
    // Compile-time guaranteed compatibility
}
```

**Benefits**:

- Zero runtime overhead for type validation
- Clear compiler errors for invalid compositions
- Extensible validation system for custom compatibility rules

### Phantom Type Pattern for Type Information

**Learning**: Use phantom types to carry type information without runtime cost
in complex generic structures.

**Pattern**: Phantom data for type safety without storage:

```rust
pub struct FunctionChain<A: ComposableShaderFunction, B: ComposableShaderFunction>
where
    A::Output: TypeCompatible<B::Input>,
{
    first: A,
    second: B,
    _phantom: PhantomData<(A::Output, B::Input)>, // Carries type info without cost
}
```

**Guidelines**:

- Use phantom types to preserve type information across API boundaries
- Name phantom fields with `_phantom` prefix for clarity
- Include all relevant type parameters in phantom type tuples

### GPU Uniform Buffer Layout Considerations

**Learning**: GPU uniform buffers require careful attention to `Copy` bounds and
alignment requirements for composed uniform structures.

**Pattern**: Explicit Copy bounds for uniform composition:

```rust
// ✅ Proper bounds for GPU uniform buffers
impl<A: ComposableShaderFunction, B: ComposableShaderFunction> ComposableShaderFunction for FunctionChain<A, B>
where
    A::Output: TypeCompatible<B::Input>,
    A::Uniforms: bytemuck::Pod + bytemuck::Zeroable + Copy, // Copy required for GPU
    B::Uniforms: bytemuck::Pod + bytemuck::Zeroable + Copy,
{
    type Uniforms = ChainUniforms<A::Uniforms, B::Uniforms>;
}
```

**Critical Requirements**:

- All GPU-bound uniform types must implement `Copy`
- Ensure 16-byte alignment for complex uniform structures
- Test uniform buffer uploads with actual GPU context
- Validate uniform sizes match WGSL expectations

### Modular Testing Strategy for Complex Systems

**Learning**: Complex shader function systems benefit from layered testing: unit
tests for individual components, integration tests with GPU operations, and
performance validation.

**Testing Categories**:

1. **Unit Tests**: Individual trait and function behavior
2. **Integration Tests**: GPU context interaction and buffer management
3. **Performance Tests**: Composition overhead and timing validation
4. **Compilation Tests**: WGSL generation and shader compilation

**Pattern**: Separate test modules for different concerns:

```rust
// Unit tests in module
#[cfg(test)]
mod tests {
    // Test individual functions and traits
}

// Integration tests in separate file
// tests/shader_function_integration.rs
#[tokio::test]
async fn test_gpu_integration() {
    let context = GupContext::headless().await?;
    // Test with actual GPU resources
}
```

### Performance-Critical API Design for Composition

**Learning**: Function composition APIs must be zero-cost abstractions that can
be optimized away at compile time.

**Performance Requirements**:

- Function composition: <100ms for 1000 compositions
- Type validation: Zero runtime overhead
- Uniform buffer creation: Minimal allocation overhead

**Pattern**: Lazy evaluation with compile-time optimization:

```rust
// ✅ Zero-cost composition
let composed = scale.compose(color_map).compose(position_transform);
// No work done until actually used
```

### Cross-Module API Compatibility

**Learning**: When adding new systems, carefully consider interactions with
existing module APIs to avoid breaking changes.

**Strategy**: Use unique naming and explicit imports:

```rust
// ✅ Avoid global naming conflicts
use crate::shader_function::{ComposableShaderFunction, TypeCompatible};
// Instead of glob imports that might conflict
```

## Procedural Macro Development (GUP-006 Learnings)

### Separate Proc-Macro Crates Required

**Learning**: Procedural macros must be in separate crates with
`proc-macro = true` and cannot be mixed with regular library code.

**Problem**: Initially tried to add proc-macro to main library:

```toml
# ❌ This doesn't work - proc-macro crates can't export regular functions
[lib]
proc-macro = true  # Conflicts with regular library functions
```

**Solution**: Create dedicated workspace member:

```toml
# ✅ Separate gup-macros crate
[workspace]
members = [".", "gup-macros"]

# gup-macros/Cargo.toml
[lib]
proc-macro = true
```

**Guidelines**:

- Always use separate crates for procedural macros
- Use workspace configuration for shared dependencies
- Import macros explicitly: `use gup_macros::wgsl_function;`

### GPU Type Compatibility Requirements

**Learning**: Types used in GPU uniforms must implement
`bytemuck::Pod + Zeroable` for safe memory transfer to GPU.

**Problem**: Vec types lacked required traits:

```rust
// ❌ Missing required derives for GPU compatibility
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}
```

**Solution**: Add GPU compatibility derives and proper alignment:

```rust
// ✅ GPU-compatible with proper alignment
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub _padding: f32, // Ensure 16-byte alignment for GPU
}
```

**Guidelines**:

- Always use `#[repr(C)]` for GPU data structures
- Add `bytemuck::Pod + Zeroable` derives for uniform compatibility
- Consider GPU alignment requirements (Vec3 needs padding to 16 bytes)

### Type Conversion for GPU Uniforms

**Learning**: GPU uniforms often require different representations than Rust
types (e.g., Vec2 → [f32; 2]).

**Pattern**: Automatic type conversion in generated code:

```rust
// Generated uniform struct uses arrays instead of Vec types
match ty {
    Type::Path(type_path) if type_path.path.segments.len() == 1 => {
        match type_path.path.segments[0].ident.to_string().as_str() {
            "Vec2" => quote! { #name: [f32; 2] },
            "Vec3" => quote! { #name: [f32; 3] },
            "Vec4" => quote! { #name: [f32; 4] },
            _ => quote! { #name: #ty },
        }
    }
}
```

**Benefits**:

- User-friendly Vec types in Rust code
- GPU-compatible array types in uniforms
- Automatic conversion handled by generated code

### Comprehensive Error Handling in Macros

**Learning**: Procedural macros should provide clear, actionable error messages
with suggestions for fixes.

**Pattern**: Context-aware error messages:

```rust
// ✅ Helpful error with context and suggestions
rust_type_to_wgsl_type(ty).map_err(|e| {
    Error::new_spanned(
        ty,
        format!("Unsupported uniform parameter type in parameter {}: {}. Only types that implement bytemuck::Pod + bytemuck::Zeroable are supported.", i + 2, e)
    )
})?;
```

**Guidelines**:

- Use `Error::new_spanned()` to highlight problematic code
- Include parameter position and context in error messages
- Suggest concrete solutions (e.g., "Use f32, i32, u32, Vec2, Vec3, Vec4")
- Validate early and fail fast with clear diagnostics

### Testing Strategy for Procedural Macros

**Learning**: Procedural macros require both unit tests (for parsing logic) and
integration tests (for generated code functionality).

**Pattern**: Multi-layer testing approach:

```rust
// Unit tests for parsing and validation logic
#[test]
fn test_parse_simple_function() {
    let input = quote! {
        fn linear_scale(value: f32, scale: f32) -> f32 {
            return value * scale;
        }
    };
    let parsed: WgslFunctionInfo = parse2(input).unwrap();
    assert_eq!(parsed.function_name, "linear_scale");
}

// Integration tests for generated code functionality
#[test]
fn test_macro_generated_linear_scale() {
    let scale_func = TestLinearScale::new(2.0, 1.0);
    assert_eq!(scale_func.scale, 2.0);
    let uniforms = scale_func.create_uniforms().unwrap();
    assert_eq!(uniforms.scale, 2.0);
}
```

**Guidelines**:

- Test parsing logic separately from code generation
- Verify generated code compiles and functions correctly
- Test error cases with invalid input
- Use integration tests to verify trait implementations work

### Import Path Complexities

**Learning**: Procedural macros cannot be easily re-exported and require
explicit imports due to Rust's macro resolution rules.

**Problem**: Attempted re-export conflicts with existing macros:

```rust
// ❌ Naming conflict with existing macro
pub use gup_macros::wgsl_function; // Conflicts with existing wgsl_function!
```

**Solution**: Document explicit import requirements:

```rust
// ✅ Clear documentation for users
// Note: Procedural macros from gup_macros must be imported directly
// with `use gup_macros::wgsl_function;` due to Rust limitations
```

**Guidelines**:

- Document import requirements clearly in crate documentation
- Avoid naming conflicts between procedural and declarative macros
- Consider using different names if conflicts arise

## Shader Pipeline System Design (GUP-007 Learnings)

### WGSL Code Generation with Type-Aware Output

**Learning**: Different shader functions return different types (f32, vec2, vec4) 
that require type-aware conversion for GPU vertex attributes.

**Problem**: All function results were wrapped in `vec4<f32>(result, 0.0, 0.0, 1.0)`
causing compilation errors when `result` was already a vec4.

**Solution**: Type-aware output generation based on function semantics:

```rust
// ✅ Handle different return types correctly
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
```

**Guidelines**:
- Know the output types of each shader function
- Generate type-appropriate WGSL conversion code
- Test with actual GPU compilation to catch type errors early

### Uniform Struct Definition Generation

**Learning**: GPU shaders require explicit struct definitions that match the
uniform data layout, not just uniform variable declarations.

**Problem**: Generated WGSL referenced `LinearScaleUniforms` without defining the struct:

```wgsl
// ❌ Reference without definition causes compilation error
@group(0) @binding(0) var<uniform> linear_scale_uniforms_0: LinearScaleUniforms;
```

**Solution**: Generate struct definitions before uniform bindings:

```rust
// ✅ Generate struct definitions first
match uniform_type_name {
    "LinearScaleUniforms" => {
        bindings.push_str("struct LinearScaleUniforms {\n");
        bindings.push_str("    domain_min: f32,\n");
        bindings.push_str("    domain_max: f32,\n");
        bindings.push_str("    range_min: f32,\n");
        bindings.push_str("    range_max: f32,\n");
        bindings.push_str("}\n\n");
    }
    // ... other uniform types
}

// Then generate uniform bindings
bindings.push_str(&format!(
    "@group(0) @binding({}) var<uniform> {}_uniforms_{}: {};\n",
    binding_index, function.name(), i, uniform_type_name
));
```

**Critical Requirements**:
- Define all uniform structs before using them in bindings
- Match field names between Rust uniforms and WGSL structs
- Use deduplicated struct definitions to avoid redefinition errors

### Function Parameter Type Matching

**Learning**: Generated WGSL function calls must match the exact parameter types
expected by shader functions, not use generic types for all functions.

**Problem**: All functions were called with `f32(in.vertex_index)` even when they
expected different parameter types like `vec2<f32>`.

**Solution**: Type-aware parameter generation:

```rust
match function.name() {
    "position_transform" => {
        // PositionTransform expects vec2<f32> as first parameter
        vertex_fn.push_str(&format!(
            "    let {}_result = {}(vec2<f32>(x, y), {}_uniforms_{});\n",
            mapping.attribute_name, unique_function_name, function.name(), i
        ));
    }
    _ => {
        // Other functions expect f32 as first parameter
        vertex_fn.push_str(&format!(
            "    let {}_result = {}(f32(in.vertex_index), {}_uniforms_{});\n",
            mapping.attribute_name, unique_function_name, function.name(), i
        ));
    }
}
```

**Guidelines**:
- Understand the signature of each shader function
- Generate appropriate parameter types for function calls
- Test generated WGSL with actual GPU compilation early and often

### Unique Function Naming for Multiple Instances

**Learning**: When multiple instances of the same function are used in a pipeline,
they must have unique names to avoid WGSL compilation conflicts.

**Pattern**: Append indices to create unique function names:

```rust
// ✅ Generate unique names for multiple instances
let unique_function_name = format!("{}_{}", function.name(), i);
function_code = function_code.replace(
    &format!("fn {}", original_name), 
    &format!("fn {}", unique_name)
);
```

**Benefits**:
- Enables multiple linear scales, color maps, etc. in same pipeline
- Clear naming convention for debugging generated shaders
- Maintains function isolation and prevents naming conflicts

### Performance Target Achievement Strategy

**Learning**: Complex shader generation systems can still meet aggressive
performance targets through efficient implementation patterns.

**Target**: <5ms shader generation time for complex pipelines
**Achieved**: 0.141ms average (35x better than target)

**Key Optimizations**:
- Lazy evaluation - defer expensive work until needed
- String concatenation instead of complex AST manipulation
- Efficient function lookup using name-based matching
- Minimal allocation during generation process

**Performance Validation Pattern**:

```rust
#[tokio::test]
async fn test_performance_target() {
    let start = Instant::now();
    let _vertex_shader = pipeline.generate_vertex_shader();
    let _fragment_shader = pipeline.generate_fragment_shader();
    let generation_time = start.elapsed();

    assert!(
        generation_time.as_millis() < 5,
        "Shader generation took {:?}, exceeding 5ms target",
        generation_time
    );
}
```

### Shader Optimization Integration

**Learning**: Shader optimization systems should be modular and provide
measurable benefits without impacting core functionality.

**Pattern**: Separate optimization methods with measurable results:

```rust
// ✅ Basic generation always works
let vertex_shader = pipeline.generate_vertex_shader();

// ✅ Optimization is optional but measurable
let optimized_vertex = pipeline.generate_optimized_vertex_shader();

// Measure optimization impact
let size_reduction = vertex_shader.len() as f64 - optimized_vertex.len() as f64;
let reduction_percentage = (size_reduction / vertex_shader.len() as f64) * 100.0;
```

**Optimization Categories**:
- **Dead code elimination**: Remove unused uniform declarations
- **Constant folding**: Replace `1.0 * x` with `x`, `0.0 + x` with `x`
- **Function inlining**: Inline simple functions called few times

### Error Context for GPU Development

**Learning**: GPU compilation errors require rich context including generated
shader source and line numbers for effective debugging.

**Pattern**: Preserve shader source for debugging:

```rust
// ✅ Generate shader first, then test compilation
let vertex_source = pipeline.generate_vertex_shader();
println!("Generated shader:\n{}", vertex_source);

// GPU compilation provides line-specific errors
let vertex_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
    label: Some("debug_vertex"),
    source: wgpu::ShaderSource::Wgsl(vertex_source.into()),
});
```

**Debugging Benefits**:
- See exact generated WGSL that fails to compile
- GPU errors include line numbers in generated code
- Performance metrics show generation vs compilation time
- Integration tests validate complete pipeline functionality

### Caching Strategy for Complex Generation

**Learning**: Shader pipeline caching provides significant performance benefits
for repeated operations without compromising correctness.

**Pattern**: Hash-based cache invalidation with lazy regeneration:

```rust
// ✅ Cache with automatic invalidation
pub struct ComposableShaderPipeline {
    cached_shaders: Option<CachedShaders>,
    pipeline_hash: u64,
}

impl ComposableShaderPipeline {
    fn invalidate_cache(&mut self) {
        self.cached_shaders = None;
        self.pipeline_hash = self.calculate_hash();
    }

    pub fn generate_vertex_shader(&self) -> String {
        if let Some(ref cached) = self.cached_shaders {
            return cached.vertex_shader.clone(); // 14.9x faster
        }
        // Generate new shader...
    }
}
```

**Cache Performance**:
- Cold generation: 0.021ms
- Cached generation: 0.001ms  
- **Speedup: 14.9x faster** for repeated shader access

### Integration Testing for GPU Systems

**Learning**: GPU shader systems require comprehensive integration testing that
validates both code generation and actual GPU compilation.

**Test Categories**:

1. **Generation Tests**: Verify WGSL syntax and structure
2. **Compilation Tests**: Actual GPU device compilation validation
3. **Pipeline Tests**: Complete render pipeline creation
4. **Performance Tests**: Timing and optimization validation

**Integration Test Pattern**:

```rust
#[tokio::test]
async fn test_complete_pipeline_workflow() {
    let context = create_test_context().await;
    let device = &context.device;

    // Build complex pipeline
    let mut pipeline = ComposableShaderPipeline::new();
    pipeline.add_function(LinearScale::new(0.0, 100.0, 0.0, 1.0));
    pipeline.add_function(ColorMap::new(Vec4::new(0.0, 0.0, 0.0, 1.0), Vec4::new(1.0, 1.0, 1.0, 1.0)));
    pipeline.map_attribute("size", "linear_scale");
    pipeline.map_attribute("color", "color_map");

    // Test generation
    let vertex_shader = pipeline.generate_vertex_shader();
    let fragment_shader = pipeline.generate_fragment_shader();

    // Test actual GPU compilation
    let vertex_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("test_vertex"),
        source: wgpu::ShaderSource::Wgsl(vertex_shader.into()),
    });

    // Test complete pipeline creation
    let render_pipeline = pipeline.create_render_pipeline(device).unwrap();
}
```

**Guidelines**:
- Test with real GPU context, not just string generation
- Validate complete render pipeline creation
- Include performance timing in integration tests
- Test multiple shader functions together, not just individually

---

_This document is a living record of learnings. Update it as new patterns and
conventions are discovered._

# Mark System API Reference

Complete API documentation for the core mark system types.

## Mark Trait

The fundamental trait that all visual primitives implement.

```rust
pub trait Mark: Clone + Send + Sync + 'static {
    type Vertex: bytemuck::Pod + bytemuck::Zeroable + Send + Sync + 'static;
    type AttributeValue: Send + Sync + 'static;

    // Shader constants (default: None — use generated shaders)
    const VERTEX_SHADER: Option<&'static str> = None;
    const FRAGMENT_SHADER: Option<&'static str> = None;
    const PATTERN_FRAGMENT_SHADER: Option<&'static str> = None;

    // Required methods
    fn vertex_count() -> usize;
    fn generate_vertices() -> Vec<Self::Vertex>;

    // Optional methods with defaults
    fn index_count() -> Option<usize> { None }
    fn generate_indices() -> Option<Vec<u32>> { None }
    fn vertex_attributes() -> &'static [VertexAttribute] { /* single vec2 */ }
    fn get_attribute_type(attribute_name: &str) -> GupResult<&'static str> { ... }
    fn is_attribute_compatible(attribute_name: &str, output_type: &str) -> bool { ... }
    fn generate_vertex_shader(pipeline: &ComposableShaderPipeline) -> String { ... }
    fn generate_fragment_shader(pipeline: &ComposableShaderPipeline) -> String { ... }
    fn generate_vertex_shader_with_functions(
        pipeline: &ComposableShaderPipeline,
        attribute_functions: &HashMap<String, String>,
    ) -> String { ... }
    fn generate_fragment_shader_with_functions(
        pipeline: &ComposableShaderPipeline,
        attribute_functions: &HashMap<String, String>,
    ) -> String { ... }
}
```

### Associated Types

#### `Vertex`

The GPU vertex type. Must be `#[repr(C)]` and implement
`bytemuck::Pod + bytemuck::Zeroable` for safe memory-mapped GPU transfer.

```rust
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CircleVertex {
    pub position: [f32; 2],  // 8 bytes, 4-byte aligned
}
```

**Requirements:**

- `#[repr(C)]` for predictable memory layout matching GPU expectations
- Fields must use GPU-compatible types (`[f32; 2]`, `[f32; 4]`, etc.)
- `vec2<f32>` requires 8-byte alignment; `vec4<f32>` requires 16-byte alignment

#### `AttributeValue`

High-level configuration type for mark instances. Not sent to the GPU directly —
it describes how to configure instances from user data.

```rust
#[derive(Debug, Clone)]
pub struct CircleAttributes {
    pub center: [f32; 2],
    pub radius: f32,
    pub fill_color: [f32; 4],
    pub stroke_color: [f32; 4],
    pub stroke_width: f32,
}
```

### Shader Constants

| Constant                  | Purpose                               |
| ------------------------- | ------------------------------------- |
| `VERTEX_SHADER`           | Hand-optimized vertex shader (WGSL)   |
| `FRAGMENT_SHADER`         | Hand-optimized fragment shader (WGSL) |
| `PATTERN_FRAGMENT_SHADER` | Accessibility pattern shader (WGSL)   |

When both `VERTEX_SHADER` and `FRAGMENT_SHADER` are `Some`, the system uses
hand-optimized shaders. Otherwise, it calls `generate_vertex_shader()` and
`generate_fragment_shader()`.

### Required Methods

#### `vertex_count() -> usize`

Returns the number of vertices in the base geometry. Common values:

- `3` — Triangle (arrows, indicators)
- `4` — Quad (circles, rectangles, most marks)
- `7+` — Complex shapes (hexagons, stars)

#### `generate_vertices() -> Vec<Self::Vertex>`

Creates the base geometry. This is called once and uploaded to a GPU vertex
buffer. Instance-specific data (position, color) comes from the instance buffer.

### Optional Methods

#### `index_count() -> Option<usize>`

Returns `Some(n)` for indexed rendering, `None` for direct rendering. Indexed
rendering is more efficient for shapes that reuse vertices (e.g., a quad uses 4
vertices but 6 indices for 2 triangles).

#### `generate_indices() -> Option<Vec<u32>>`

Creates triangle indices. Standard quad indices: `[0, 1, 2, 0, 2, 3]`.

#### `vertex_attributes() -> &'static [VertexAttribute]`

Defines the vertex buffer layout for the GPU. Defaults to a single `vec2<f32>`
position at location 0. Override for marks with additional vertex data (e.g.,
texture coordinates, normals).

#### `get_attribute_type(name: &str) -> GupResult<&'static str>`

Maps attribute names to WGSL types for shader function validation:

| Attribute Name       | Default WGSL Type |
| -------------------- | ----------------- |
| `"position"`         | `vec2<f32>`       |
| `"color"`            | `vec4<f32>`       |
| `"size"`, `"radius"` | `f32`             |

Override to support additional attributes specific to your mark type.

#### `is_attribute_compatible(name: &str, output_type: &str) -> bool`

Checks whether a shader function's output type matches the mark's expected
attribute type. Used for compile-time validation in the shader function
pipeline.

---

## MarkRegistry

Type-safe runtime registry for mark types and their render pipelines.

```rust
pub struct MarkRegistry {
    marks: HashMap<TypeId, Box<dyn MarkInfo>>,
    pipelines: HashMap<TypeId, Arc<RenderPipeline>>,
}
```

### Methods

#### `new() -> Self`

Creates an empty registry.

#### `register::<M: Mark>(&mut self)`

Registers a mark type with the registry. Stores type metadata
(`MarkInfoImpl<M>`) indexed by `TypeId`. Registration is idempotent.

```rust
let mut registry = MarkRegistry::new();
registry.register::<Circle>();
registry.register::<Rectangle>();
registry.register::<Line>();
```

#### `is_registered::<M: Mark>(&self) -> bool`

Checks whether a mark type has been registered.

#### `get_mark_info::<M: Mark>(&self) -> Option<&dyn MarkInfo>`

Returns type-erased metadata for a registered mark. Returns `None` if the mark
type is not registered.

#### `get_pipeline::<M: Mark>(&mut self, device: &Device) -> GupResult<Arc<RenderPipeline>>`

Gets or creates a render pipeline for a mark type. On first call, creates the
pipeline (~15ms), caches it, and returns an `Arc`. Subsequent calls return a
clone of the cached `Arc` (~0.015ms).

```rust
// First call: creates and caches the pipeline
let pipeline = registry.get_pipeline::<Circle>(&device)?;

// Second call: returns cached Arc clone (near-instant)
let pipeline2 = registry.get_pipeline::<Circle>(&device)?;
```

#### `get_pipeline_with_blend::<M: Mark>(&mut self, device, blend_config, context_blend)`

Gets a pipeline with custom blend state. Falls back to the standard cached
pipeline when the resolved blend state matches default alpha blending.

#### `create_multi_pass_pipelines::<M: Mark>(&self, device, config) -> GupResult<Vec<RenderPipeline>>`

Creates one pipeline per pass in a `MultiPassConfig`. Each pipeline has its own
blend state, polygon mode, and shader entry points.

#### `create_bind_group::<M: Mark>(&self, device, instance_buffer, uniform_buffers)`

Creates a bind group matching the mark's pipeline layout with the provided
buffers.

#### `get_bind_group_layout::<M: Mark>(&self, device) -> GupResult<BindGroupLayout>`

Returns the bind group layout for a mark type (for creating compatible bind
groups externally).

#### `clear_pipeline_cache(&mut self)`

Clears all cached pipelines. Useful when GPU resources need recreation (e.g.,
device lost, surface format change).

#### `mark_count() -> usize` / `pipeline_count() -> usize`

Returns the number of registered marks or cached pipelines.

#### `registered_types() -> Vec<&'static str>`

Returns type names of all registered marks (for debugging).

---

## MarkRenderer

High-level renderer managing GPU buffers for mark rendering.

```rust
pub struct MarkRenderer {
    vertex_buffer: GpuBuffer<u8>,
    instance_buffer: GpuBuffer<u8>,
    index_buffer: Option<GpuBuffer<u32>>,
    metrics: MarkPerformanceMetrics,
}
```

### Construction

#### `new(device: &Device) -> Self`

Creates a renderer with default buffer capacities (4 KB vertex, 8 KB instance, 2
KB index).

#### `with_capacity(device, vertex_capacity, instance_capacity, index_capacity) -> Self`

Creates a renderer with custom buffer sizes. Use when you know the expected data
size to avoid reallocations.

```rust
// Pre-allocate for 10,000 circle instances (each ~32 bytes)
let renderer = MarkRenderer::with_capacity(
    &device,
    1024,              // vertex data (small, shared geometry)
    320_000,           // instance data (10K × 32 bytes)
    Some(256),         // index data (6 indices × 4 bytes)
);
```

### Data Upload

#### `upload_vertices::<T: Pod>(&mut self, device, queue, vertices) -> GupResult<()>`

Uploads base geometry to the vertex buffer. Call once per mark type.

#### `upload_instances::<T: Pod>(&mut self, device, queue, instances) -> GupResult<()>`

Uploads per-instance data to the instance buffer. Call each frame with updated
data.

#### `upload_indices(&mut self, device, queue, indices) -> GupResult<()>`

Uploads index data for indexed rendering. Call once per mark type.

### Rendering

#### `render_marks::<M: Mark>(render_pass, pipeline, bind_group, instance_count)`

Renders mark instances. Automatically selects indexed or non-indexed rendering
based on `M::index_count()`.

```rust
renderer.render_marks::<Circle>(
    &mut render_pass,
    &pipeline,
    &bind_group,
    1000,  // render 1000 circle instances
)?;
```

#### `render_marks_with_patterns::<M>(render_pass, pipeline, bind_group, pattern_bind_group, count)`

Renders with accessibility pattern support. Sets the pattern bind group at
slot 1.

#### `render_marks_multi_pass::<M>(render_pass, config, pipelines, bind_group, count)`

Renders through multiple passes with different pipeline configurations.

#### `render_marks_with_state::<M>(render_pass, pipeline, bind_group, state_manager, viewport, count)`

Renders with state isolation — saves and restores viewport/scissor state to
prevent marks from interfering with each other in compositions.

### Buffer Access

| Method                | Returns                   | Description                |
| --------------------- | ------------------------- | -------------------------- |
| `vertex_capacity()`   | `usize`                   | Vertex buffer capacity     |
| `instance_capacity()` | `usize`                   | Instance buffer capacity   |
| `index_capacity()`    | `Option<usize>`           | Index buffer capacity      |
| `vertex_len()`        | `usize`                   | Current vertex data length |
| `instance_len()`      | `usize`                   | Current instance data len  |
| `index_len()`         | `Option<usize>`           | Current index data length  |
| `vertex_buffer()`     | `&GpuBuffer<u8>`          | Raw vertex buffer access   |
| `instance_buffer()`   | `&GpuBuffer<u8>`          | Raw instance buffer access |
| `index_buffer()`      | `Option<&GpuBuffer<u32>>` | Raw index buffer access    |
| `clear()`             | `()`                      | Reset lengths to zero      |

---

## MarkInfo Trait

Type-erased (dyn-compatible) interface for runtime mark operations.

```rust
pub trait MarkInfo: Send + Sync {
    fn type_name(&self) -> &'static str;
    fn vertex_size(&self) -> usize;
    fn attribute_size(&self) -> usize;
    fn has_custom_shaders(&self) -> bool;
    fn has_pattern_shader(&self) -> bool;
    fn create_render_pipeline(&self, device: &Device) -> GupResult<RenderPipeline>;
    fn create_render_pipeline_with_patterns(&self, device: &Device) -> GupResult<RenderPipeline>;
    fn vertex_count(&self) -> usize;
    fn index_count(&self) -> Option<usize>;
    fn generate_vertices_boxed(&self) -> Vec<u8>;
    fn generate_indices_boxed(&self) -> Option<Vec<u32>>;
    fn as_any(&self) -> &dyn std::any::Any;
}
```

This trait exists to support `MarkRegistry`'s heterogeneous storage. You rarely
interact with it directly — use `MarkRegistry` and the `Mark` trait instead.

---

## MarkTypeIdProvider Trait

Provides stable mark type IDs for the GPU interaction system.

```rust
pub trait MarkTypeIdProvider {
    fn mark_type_id() -> u32;
}
```

Derived automatically with `#[derive(MarkTypeId)]`:

```rust
use gup_macros::MarkTypeId;

#[derive(Clone, MarkTypeId)]
#[mark_type_id = 0]
pub struct Circle;
```

The ID must match the corresponding value in GPU compute shaders (e.g.,
`hit_test.compute.wgsl`).

---

## AttributeBinding

Type-safe binding between shader functions and mark attributes.

```rust
pub struct AttributeBinding<T, M: Mark> {
    attribute_name: String,
    function_name: String,
    wgsl_code: String,
    uniform_buffer: Option<wgpu::Buffer>,
}
```

### Methods

| Method                                 | Description                         |
| -------------------------------------- | ----------------------------------- |
| `new(name, function_name, wgsl_code)`  | Create binding with type validation |
| `create_uniform_buffer(device, size)`  | Allocate GPU uniform buffer         |
| `update_uniforms(queue, data)`         | Write uniform data to GPU           |
| `get_wgsl_function_call()`             | Generate WGSL call expression       |
| `attribute_name()` / `function_name()` | Accessor methods                    |
| `has_uniforms()` / `uniform_buffer()`  | Check/access uniform buffer         |

---

## Advanced Rendering Types

### MultiPassRenderer

Manages state for multi-pass mark rendering (e.g., shadow pass + fill pass).

### MultiPassConfig

Configuration for multi-pass rendering. Contains a vector of `RenderPassConfig`
entries, each specifying:

- `label` — Pass name for debugging
- `blend_state` — Optional custom blend state
- `polygon_mode` — Fill, Line, or Point
- `vertex_entry_point` / `fragment_entry_point` — Optional shader overrides
- `stencil_reference` — Optional stencil test value

### DynamicAttributeBufferManager

Manages runtime attribute-to-buffer mappings for marks with configurable
attributes. Supports uploading heterogeneous attribute data to separate GPU
buffers.

### RenderStateManager

Saves and restores GPU render state (viewport, scissor) for state isolation
between mark types in compositions.

---

## Batch Rendering Types

### InstancedBatchRenderer

Groups mark instances by pipeline to minimize GPU state changes across draw
calls.

### CullingManager

Performs viewport-based frustum culling to skip invisible instances. Supports
level-of-detail selection:

| LOD Level    | Description                 |
| ------------ | --------------------------- |
| `Full`       | Render at full detail       |
| `Simplified` | Reduced vertex count        |
| `Point`      | Single-pixel representation |
| `Culled`     | Skip rendering entirely     |

### Viewport2D

2D viewport bounds in clip space with pixel dimensions for culling calculations.

---

## Performance Optimization Types

### EnhancedPipelineCache

Key-value pipeline cache with configurable eviction. Uses `PipelineCacheKey` for
composite keys beyond simple `TypeId`.

### MarkBufferPool

Pre-allocated buffer pool organized by `SizeClass` for efficient buffer reuse
without GPU allocation churn.

### MarkPerformanceMetrics

Frame-level performance counters:

- `draw_calls` — Number of draw calls issued
- `instances_rendered` — Total instances drawn
- `pipeline_switches` — Number of pipeline changes
- `upload_time` / `render_time` — Timing measurements

---

## Compute Instance Filter

### ComputeInstanceFilter

GPU compute shader-based instance filtering. Filters instances on the GPU before
rendering, returning a `FilterResult` with the indices of visible instances and
filtering statistics.

### FilterConfig

Configuration for compute-based filtering including viewport bounds, visibility
thresholds, and workgroup size.

---

## Error Handling

Mark system errors use the project-wide `GupError` type:

| Error Kind        | When                                     |
| ----------------- | ---------------------------------------- |
| `RenderError`     | Pipeline creation failure, buffer errors |
| `ValidationError` | Unknown attribute name, type mismatch    |

All fallible methods return `GupResult<T>` (alias for `Result<T, GupError>`).
Error messages include the mark type name and operation context for diagnostics.

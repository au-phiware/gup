# Mark System Architecture

This document describes the design principles, component relationships, and data
flow of the Gup mark system.

## Design Principles

### 1. Dual Shader Strategy

The mark system supports two shader approaches within the same trait system:

- **Hand-optimized shaders** — Pre-written WGSL compiled into the binary via
  `include_str!`. These achieve maximum GPU performance and are used by all
  built-in mark types (Circle, Rectangle, Line, etc.).

- **Generated shaders** — Dynamically composed from the
  `ComposableShaderPipeline`. These integrate with the shader function system
  for flexible attribute mapping at the cost of some compile-time optimization.

Marks declare their strategy through trait constants and methods:

```rust
// Hand-optimized (fastest)
const VERTEX_SHADER: Option<&'static str> = Some(include_str!("shaders/circle.vert.wgsl"));
const FRAGMENT_SHADER: Option<&'static str> = Some(include_str!("shaders/circle.frag.wgsl"));

// Generated (flexible) — default when constants are None
fn generate_vertex_shader(pipeline: &ComposableShaderPipeline) -> String { ... }
fn generate_fragment_shader(pipeline: &ComposableShaderPipeline) -> String { ... }
```

The system automatically selects the appropriate strategy: if both shader
constants are `Some`, it uses hand-optimized shaders; otherwise it falls back to
generation.

### 2. Type-Safe GPU Integration

Rust's type system enforces GPU compatibility at compile time:

- **Vertex types** must implement `bytemuck::Pod + bytemuck::Zeroable` and use
  `#[repr(C)]` for predictable memory layout.
- **Associated types** on the `Mark` trait (`Vertex`, `AttributeValue`) ensure
  each mark has well-defined GPU data structures.
- **Attribute type validation** via `get_attribute_type()` maps attribute names
  to WGSL types (e.g., `"position"` → `"vec2<f32>"`).

### 3. Arc-Based Resource Sharing

Render pipelines are expensive to create (~15ms) but cheap to share. The
`MarkRegistry` caches pipelines as `Arc<RenderPipeline>`, enabling multiple
renderers and compositions to share the same pipeline without lifetime
complications.

### 4. Comprehensive Buffer Management

The `MarkRenderer` manages three GPU buffer types that automatically resize as
needed:

| Buffer   | Type     | Purpose                                | Default Size |
| -------- | -------- | -------------------------------------- | ------------ |
| Vertex   | Vertex   | Base geometry (e.g., unit quad)        | 4 KB         |
| Instance | Instance | Per-instance data (position, color)    | 8 KB         |
| Index    | Storage  | Triangle indices for indexed rendering | 2 KB         |

Buffers use a 1.5× growth factor when resizing, balancing memory usage with
allocation frequency.

## Component Architecture

### Core Type Hierarchy

```text
Mark trait (generic: Vertex, AttributeValue)
│
├── MarkInfo trait (type-erased, dyn-compatible)
│   └── MarkInfoImpl<M: Mark>
│       ├── create_render_pipeline()     → RenderPipeline
│       ├── create_bind_group_layout()   → BindGroupLayout
│       ├── generate_vertices_boxed()    → Vec<u8>
│       └── create_render_pipeline_for_pass() → RenderPipeline
│
├── MarkRegistry
│   ├── marks: HashMap<TypeId, Box<dyn MarkInfo>>
│   ├── pipelines: HashMap<TypeId, Arc<RenderPipeline>>
│   ├── register::<M>()
│   ├── get_pipeline::<M>()
│   ├── get_pipeline_with_blend::<M>()
│   └── create_multi_pass_pipelines::<M>()
│
├── MarkRenderer
│   ├── vertex_buffer: GpuBuffer<u8>
│   ├── instance_buffer: GpuBuffer<u8>
│   ├── index_buffer: Option<GpuBuffer<u32>>
│   ├── upload_vertices() / upload_instances() / upload_indices()
│   ├── render_marks::<M>()
│   ├── render_marks_multi_pass::<M>()
│   └── render_marks_with_patterns::<M>()
│
└── Built-in Implementations
    ├── Circle      (SDF circle, instanced quads)
    ├── Rectangle   (SDF rounded rect, instanced quads)
    ├── Line        (width, dash/dot styles)
    ├── BoxPlot     (statistical visualization)
    ├── Text        (SDF glyph atlas)
    ├── Path        (GPU tessellation)
    └── CompositeMark (nested sub-marks)
```

### Type Erasure Pattern

The mark system uses a two-level type system:

1. **Generic level** — The `Mark` trait with associated types provides
   compile-time safety. Code that knows the concrete mark type works with `Mark`
   directly.

2. **Erased level** — The `MarkInfo` trait provides a dyn-compatible interface
   for runtime operations. `MarkInfoImpl<M>` bridges the two levels by
   implementing `MarkInfo` for any `M: Mark`.

This pattern enables the `MarkRegistry` to store heterogeneous mark types in a
single `HashMap<TypeId, Box<dyn MarkInfo>>` while preserving type safety through
`TypeId` lookups and `downcast_ref`.

## Data Flow

### Pipeline Creation

```text
1. User calls registry.register::<Circle>()
   └─ Stores MarkInfoImpl<Circle> in HashMap

2. User calls registry.get_pipeline::<Circle>(device)
   ├─ Cache hit?  → Return Arc::clone(cached_pipeline)
   └─ Cache miss? → Create new pipeline:
      ├─ Select shader source (hand-optimized or generated)
      ├─ Create shader modules (vertex + fragment)
      ├─ Create bind group layout (instance buffer + optional uniforms)
      ├─ Create pipeline layout
      ├─ Create RenderPipeline with:
      │   ├─ TriangleList topology
      │   ├─ Alpha blending
      │   ├─ No depth testing (2D)
      │   └─ No face culling (double-sided)
      ├─ Wrap in Arc and cache
      └─ Return Arc clone
```

### Render Loop

```text
1. Upload phase (CPU → GPU):
   ├─ renderer.upload_vertices(device, queue, &vertices)
   ├─ renderer.upload_instances(device, queue, &instances)
   └─ renderer.upload_indices(device, queue, &indices)

2. Draw phase (GPU):
   └─ renderer.render_marks::<M>(render_pass, pipeline, bind_group, count)
       ├─ Set pipeline
       ├─ Set bind group at slot 0
       ├─ Set vertex buffer at slot 0
       ├─ If indexed: set index buffer, draw_indexed()
       └─ If non-indexed: draw()
```

### Bind Group Layout

The bind group layout varies based on shader strategy:

**Hand-optimized shaders** (1 binding):

| Binding | Stage           | Type           | Content       |
| ------- | --------------- | -------------- | ------------- |
| 0       | Vertex+Fragment | Storage (read) | Instance data |

**Generated shaders** (3 bindings):

| Binding | Stage           | Type           | Content             |
| ------- | --------------- | -------------- | ------------------- |
| 0       | Vertex+Fragment | Storage (read) | Instance data       |
| 1       | Vertex          | Uniform        | Position transforms |
| 2       | Fragment        | Uniform        | Color transforms    |

## Advanced Features

### Multi-Pass Rendering

Marks can be rendered in multiple passes for effects like shadow + fill or
outline + interior. Each pass has its own:

- Blend state (e.g., multiply for shadows, alpha for fill)
- Polygon mode (e.g., wireframe for outlines)
- Shader entry points
- Stencil reference

All passes execute within a single render pass, following the project's single
render pass per frame pattern.

### Pattern Rendering (Accessibility)

For colorblind accessibility, marks can provide a `PATTERN_FRAGMENT_SHADER` that
encodes data categories using geometric patterns (dots, lines, crosshatch)
instead of color alone. Pattern rendering uses a second bind group (slot 1) for
pattern-specific uniforms.

### Batch Rendering

The `InstancedBatchRenderer` groups marks by pipeline to minimize GPU state
changes. It includes:

- **Viewport culling** — Skip instances outside the visible area
- **Level of detail** — Simplify rendering for distant/small marks
- **Geometry caching** — Reuse vertex data across frames

### Compute Shader Filtering

The `ComputeInstanceFilter` uses GPU compute shaders to filter instances before
rendering. This enables operations like:

- Viewport-based culling on the GPU
- Data-driven visibility filtering
- Threshold-based instance selection

## Integration Points

### Shader Function System

Marks integrate with the `ComposableShaderPipeline` for dynamic attribute
mapping. Shader functions compose data transformations (scales, projections,
color mappings) that execute entirely on the GPU.

### Selection API

The Selection API uses marks as its rendering backend. When a `Selection<T, M>`
binds attributes via `.attr()`, it maps shader functions to mark attributes with
type validation.

### Interaction System

Marks implement `MarkTypeIdProvider` (via `#[derive(MarkTypeId)]`) to enable
GPU-based hit testing. The interaction system uses compute shaders to identify
which mark instance was clicked or hovered based on the mark's type ID and
geometry.

### Composition System

Marks participate in the `Mixable` composition system. Multiple mark-based
visualizations can be overlaid, placed side-by-side, or blended using the
`MarkBlendConfig` system which resolves blend states through a priority chain
(mark-level → context-level → default alpha blending).

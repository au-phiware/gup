# Mark System

The Gup mark system provides a high-level abstraction for GPU-accelerated
rendering of visual primitives. It bridges user-friendly APIs with
high-performance GPU operations through a carefully designed trait hierarchy.

## Quick Start

```rust
use gup::mark::{Circle, Mark, MarkRegistry, MarkRenderer};
use gup::GupContext;
use std::sync::Arc;

async fn render_circles() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Initialize GPU context
    let context = Arc::new(GupContext::headless().await?);

    // 2. Register mark types
    let mut registry = MarkRegistry::new();
    registry.register::<Circle>();

    // 3. Create renderer and upload geometry
    let mut renderer = MarkRenderer::new(&context.device);
    let vertices = Circle::generate_vertices();
    renderer.upload_vertices(&context.device, &context.queue, &vertices)?;

    // 4. Get cached render pipeline
    let pipeline = registry.get_pipeline::<Circle>(&context.device)?;

    Ok(())
}
```

## Documentation

| Document                            | Description                               |
| ----------------------------------- | ----------------------------------------- |
| [Architecture](architecture.md)     | System design, components, and data flow  |
| [API Reference](api-reference.md)   | Complete API documentation for core types |
| [Performance Guide](performance.md) | Optimization strategies and benchmarks    |

For creating custom marks, see the
[Custom Mark Development Guide](../CUSTOM_MARK_GUIDE.md).

## Built-in Mark Types

| Mark        | Shape             | Vertices  | Indexed  | Key Features                           |
| ----------- | ----------------- | --------- | -------- | -------------------------------------- |
| `Circle`    | Filled circle     | 4 (quad)  | Yes (6)  | SDF rendering, radius, fill/stroke     |
| `Rectangle` | Rounded rectangle | 4 (quad)  | Yes (6)  | Corner radius, fill/stroke             |
| `Line`      | Styled line       | 4 (quad)  | Yes (6)  | Width, dash/dot styles                 |
| `BoxPlot`   | Statistical box   | 4 (quad)  | Yes (6)  | Min/Q1/median/Q3/max, outliers         |
| `Text`      | SDF text glyph    | 4 (quad)  | Yes (6)  | Font atlas, anchor, SDF anti-aliasing  |
| `Path`      | SVG-like path     | 4 (quad)  | Yes (6)  | GPU tessellation, stroke, SVG commands |
| `Composite` | Grouped sub-marks | 4 (quad)  | Yes (6)  | Nested marks with transforms           |
| `Sphere3D`  | 3D billboard sphere | 4 (quad) | Yes (6) | SDF + frag_depth, Phong lighting       |
| `Box3D`     | 3D axis-aligned box | 24 (cube) | Yes (36) | 6 faces, Phong lighting              |
| `Line3D`    | 3D line segment   | 4 (quad)  | Yes (6)  | Camera-facing quad, unlit              |

## System Overview

```text
┌─────────────────────────────────────────────────────┐
│                   User Code                         │
│  registry.register::<Circle>();                     │
│  renderer.upload_vertices(...);                     │
│  renderer.render_marks::<Circle>(...);              │
└───────────────┬─────────────────────────────────────┘
                │
┌───────────────▼─────────────────────────────────────┐
│              MarkRegistry                           │
│  • Type-safe mark registration                      │
│  • Pipeline caching (Arc<RenderPipeline>)            │
│  • Bind group creation                              │
└───────────────┬─────────────────────────────────────┘
                │
┌───────────────▼─────────────────────────────────────┐
│              MarkRenderer                           │
│  • Vertex/instance/index buffer management          │
│  • Indexed and non-indexed draw calls               │
│  • Multi-pass and pattern rendering                 │
└───────────────┬─────────────────────────────────────┘
                │
┌───────────────▼─────────────────────────────────────┐
│           Mark Trait + MarkInfo                      │
│  • Vertex type and geometry generation              │
│  • Shader selection (hand-optimized or generated)   │
│  • Attribute type validation                        │
└───────────────┬─────────────────────────────────────┘
                │
┌───────────────▼─────────────────────────────────────┐
│         GPU (wgpu)                                  │
│  • Render pipelines, shader modules                 │
│  • Storage buffers (instances), vertex buffers      │
│  • Draw calls with instancing                       │
└─────────────────────────────────────────────────────┘
```

## Key Concepts

### Instanced Rendering

All marks use GPU instancing. The base geometry (e.g., a unit quad) is uploaded
once. Per-instance data (position, color, size) is stored in a GPU storage
buffer. The GPU renders thousands of instances in a single draw call.

### Dual Shader Strategy

Marks support two shader approaches:

1. **Hand-optimized** — Set `VERTEX_SHADER` and `FRAGMENT_SHADER` constants with
   pre-written WGSL for maximum performance.
2. **Generated** — Override `generate_vertex_shader()` /
   `generate_fragment_shader()` for flexibility and integration with the shader
   function pipeline.

### Pipeline Caching

The `MarkRegistry` caches compiled `wgpu::RenderPipeline` objects using
`Arc<RenderPipeline>`. Pipeline creation is expensive (~15ms); cached access is
near-instant (~0.015ms).

### Type Safety

The `Mark` trait uses associated types (`Vertex`, `AttributeValue`) to enforce
GPU memory layout at compile time. Vertex types must be `#[repr(C)]` with
`bytemuck::Pod` for safe GPU transfer.

# Custom Mark Development Guide

This guide explains how to create custom marks in Gup, from simple shapes to
complex procedural marks.

## The Mark Trait

All visual primitives in Gup implement the `Mark` trait. This trait bridges
high-level visualization concepts with GPU rendering.

### Basic Mark Structure

```rust
use gup::mark::Mark;
use gup::error::GupResult;
use gup::shader_pipeline::ComposableShaderPipeline;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct MyCustomMark;

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MyCustomVertex {
    pub position: [f32; 2],
    // Add other vertex attributes as needed
}

#[derive(Debug, Clone)]
pub struct MyCustomAttributes {
    pub center: [f32; 2],
    pub size: f32,
    pub color: [f32; 4],
}

impl Mark for MyCustomMark {
    type Vertex = MyCustomVertex;
    type AttributeValue = MyCustomAttributes;

    // Define how many vertices per mark instance
    fn vertex_count() -> usize {
        4 // For a quad
    }

    // Define index count if using indexed rendering
    fn index_count() -> Option<usize> {
        Some(6) // Two triangles
    }

    // Generate the base geometry
    fn generate_vertices() -> Vec<Self::Vertex> {
        vec![
            MyCustomVertex { position: [-1.0, -1.0] },
            MyCustomVertex { position: [ 1.0, -1.0] },
            MyCustomVertex { position: [ 1.0,  1.0] },
            MyCustomVertex { position: [-1.0,  1.0] },
        ]
    }

    // Generate indices for the geometry
    fn generate_indices() -> Option<Vec<u32>> {
        Some(vec![0, 1, 2, 0, 2, 3])
    }
}
```

## Vertex Requirements

### The `bytemuck` Traits

Vertex types must implement `bytemuck::Pod` and `bytemuck::Zeroable` for safe
GPU transfer:

```rust
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CustomVertex {
    pub position: [f32; 2],
    pub uv: [f32; 2],
}
```

Key requirements:

- Use `#[repr(C)]` for predictable memory layout
- Only use types that are `Pod` (Plain Old Data)
- Align fields to GPU requirements (vec2<f32> = 8 bytes, vec4<f32> = 16 bytes)

### Common Vertex Patterns

**Simple quad for shape rendering:**

```rust
pub struct QuadVertex {
    pub position: [f32; 2], // Corner position (-1 to 1)
}
```

**Textured quad:**

```rust
pub struct TexturedVertex {
    pub position: [f32; 2],
    pub tex_coords: [f32; 2],
}
```

**Line with normals:**

```rust
pub struct LineVertex {
    pub position: [f32; 2],
    pub normal: [f32; 2],  // For line width expansion
}
```

## Shader Generation

### Using Hand-Optimized Shaders

For maximum performance, provide pre-written WGSL shaders:

```rust
impl Mark for MyCustomMark {
    type Vertex = MyCustomVertex;
    type AttributeValue = MyCustomAttributes;

    const VERTEX_SHADER: Option<&'static str> =
        Some(include_str!("shaders/my_mark.vert.wgsl"));
    const FRAGMENT_SHADER: Option<&'static str> =
        Some(include_str!("shaders/my_mark.frag.wgsl"));

    // ... rest of implementation
}
```

### Using Generated Shaders

For flexibility, generate shaders dynamically:

```rust
impl Mark for MyCustomMark {
    // ... vertex and attribute types

    fn generate_vertex_shader_with_functions(
        pipeline: &ComposableShaderPipeline,
        attribute_functions: &HashMap<String, String>,
    ) -> String {
        format!(
            r#"
struct MyInstance {{
    center: vec2<f32>,
    size: f32,
    color: vec4<f32>,
}}

@group(0) @binding(0)
var<storage, read> instances: array<MyInstance>;

struct VertexOutput {{
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
}}

@vertex
fn vs_main(
    @location(0) vertex_pos: vec2<f32>,
    @builtin(instance_index) instance_index: u32,
) -> VertexOutput {{
    let instance = instances[instance_index];
    var output: VertexOutput;

    // Transform vertex position
    let world_pos = instance.center + vertex_pos * instance.size;
    output.position = vec4<f32>(world_pos, 0.0, 1.0);
    output.color = instance.color;

    return output;
}}
"#
        )
    }

    fn generate_fragment_shader_with_functions(
        pipeline: &ComposableShaderPipeline,
        attribute_functions: &HashMap<String, String>,
    ) -> String {
        format!(
            r#"
struct FragmentInput {{
    @location(0) color: vec4<f32>,
}}

@fragment
fn fs_main(input: FragmentInput) -> @location(0) vec4<f32> {{
    return input.color;
}}
"#
        )
    }
}
```

## Attribute Type Validation

Implement attribute type checking for compile-time safety:

```rust
impl Mark for MyCustomMark {
    // ... other methods

    fn get_attribute_type(attribute_name: &str) -> GupResult<&'static str> {
        match attribute_name {
            "center" | "position" => Ok("vec2<f32>"),
            "color" => Ok("vec4<f32>"),
            "size" | "radius" | "width" | "height" => Ok("f32"),
            _ => Err(GupError::validation_error(format!(
                "Unknown attribute: {attribute_name}"
            ))),
        }
    }
}
```

## Advanced Patterns

### SDF-Based Rendering

For smooth anti-aliased edges, use signed distance fields:

```wgsl
@fragment
fn fs_main(input: FragmentInput) -> @location(0) vec4<f32> {
    // Calculate distance from edge
    let dist = length(input.local_pos) - input.radius;

    // Anti-aliased edge (1px smoothing)
    let smoothing = fwidth(dist);
    let alpha = 1.0 - smoothstep(-smoothing, smoothing, dist);

    return vec4<f32>(input.color.rgb, input.color.a * alpha);
}
```

### Instanced Rendering

The mark system automatically uses instanced rendering for efficiency:

```rust
// Each instance has its own attributes
pub struct MarkInstance {
    pub position: Vec2,
    pub size: f32,
    pub color: Vec4,
}

// GPU storage buffer layout
@group(0) @binding(0)
var<storage, read> instances: array<MarkInstance>;
```

### Complex Geometries

For marks with more than 4 vertices:

```rust
impl Mark for ComplexMark {
    fn vertex_count() -> usize {
        12  // Complex shape with 12 vertices
    }

    fn index_count() -> Option<usize> {
        Some(18)  // 6 triangles
    }

    fn generate_vertices() -> Vec<Self::Vertex> {
        // Generate your complex geometry here
        vec![
            // ... 12 vertices
        ]
    }

    fn generate_indices() -> Option<Vec<u32>> {
        Some(vec![
            // ... triangle indices
        ])
    }
}
```

## Performance Best Practices

### 1. Use Instanced Rendering

The mark system handles instancing automatically. Focus on defining efficient
per-instance data:

```rust
// Good: Compact instance data
pub struct InstanceData {
    pub transform: [[f32; 2]; 2],  // 16 bytes
    pub color: [f32; 4],             // 16 bytes
}  // Total: 32 bytes per instance

// Avoid: Excessive per-instance data
pub struct BloatedInstance {
    pub data1: [f32; 100],  // 400 bytes!
    // ...
}
```

### 2. Minimize Vertex Count

Use the minimum vertices needed:

```rust
// Good: 4 vertices for a quad
fn vertex_count() -> usize { 4 }

// Avoid: Unnecessary subdivision
fn vertex_count() -> usize { 64 }  // Unless needed for deformation
```

### 3. Use Indexed Rendering

Always use indices for efficiency:

```rust
fn index_count() -> Option<usize> {
    Some(6)  // Reuse vertices
}
```

### 4. Cache Shader Pipelines

The mark system caches compiled shaders automatically. Avoid generating unique
shaders per instance.

## Testing Your Mark

Write comprehensive tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vertex_layout() {
        let vertex = MyCustomVertex { position: [1.0, 2.0] };
        let bytes = bytemuck::bytes_of(&vertex);
        assert_eq!(bytes.len(), std::mem::size_of::<MyCustomVertex>());
    }

    #[test]
    fn test_vertex_generation() {
        let vertices = MyCustomMark::generate_vertices();
        assert_eq!(vertices.len(), MyCustomMark::vertex_count());
    }

    #[test]
    fn test_index_generation() {
        let indices = MyCustomMark::generate_indices().unwrap();
        assert_eq!(indices.len(), MyCustomMark::index_count().unwrap());
    }

    #[test]
    fn test_attribute_types() {
        assert_eq!(
            MyCustomMark::get_attribute_type("position").unwrap(),
            "vec2<f32>"
        );
    }
}
```

## Examples

### Example 1: Star Mark

```rust
#[derive(Debug, Clone)]
pub struct Star;

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct StarVertex {
    pub position: [f32; 2],
}

#[derive(Debug, Clone)]
pub struct StarAttributes {
    pub center: Vec2,
    pub outer_radius: f32,
    pub inner_radius: f32,
    pub points: u32,
    pub color: Vec4,
}

impl Mark for Star {
    type Vertex = StarVertex;
    type AttributeValue = StarAttributes;

    fn vertex_count() -> usize {
        // Star with 5 points = 10 vertices (5 outer + 5 inner) + center
        11
    }

    fn index_count() -> Option<usize> {
        Some(15)  // 5 triangles for star points
    }

    fn generate_vertices() -> Vec<Self::Vertex> {
        // Generate star geometry...
        todo!()
    }

    fn generate_indices() -> Option<Vec<u32>> {
        // Generate triangle indices...
        todo!()
    }
}
```

### Example 2: Arrow Mark

```rust
#[derive(Debug, Clone)]
pub struct Arrow;

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ArrowVertex {
    pub position: [f32; 2],
}

#[derive(Debug, Clone)]
pub struct ArrowAttributes {
    pub start: Vec2,
    pub end: Vec2,
    pub head_size: f32,
    pub shaft_width: f32,
    pub color: Vec4,
}

impl Mark for Arrow {
    type Vertex = ArrowVertex;
    type AttributeValue = ArrowAttributes;

    // Arrow: shaft (4 vertices) + head (3 vertices) = 7 vertices
    fn vertex_count() -> usize {
        7
    }

    fn index_count() -> Option<usize> {
        Some(9)  // 3 triangles
    }

    fn generate_vertices() -> Vec<Self::Vertex> {
        // Generate arrow geometry...
        todo!()
    }

    fn generate_indices() -> Option<Vec<u32>> {
        todo!()
    }
}
```

## Integration with Selection API

Once your mark is defined, use it with the Selection API:

```rust
use gup::selection::Selection;

let data = vec![/* your data */];
let mut selection = Selection::<MyData, MyCustomMark>::new(data, context);

selection
    .attr("position", position_transform)
    .attr("color", color_scale);

selection.render()?;
```

## Quick Start with Derive Macro

The fastest way to create a custom mark is with `#[derive(Mark)]`:

```rust
use gup_macros::Mark;
use gup::shader_function::{Vec2, Vec4};

#[derive(Debug, Clone, Mark)]
#[mark(primitive = "quad")]
pub struct Diamond {
    pub center: Vec2,
    pub size: f32,
    pub color: Vec4,
    pub angle: f32,
}
```

This single declaration generates:

- A `DiamondVertex` struct with `#[repr(C)]` and `bytemuck` derives
- A complete `Mark` implementation with quad geometry (4 vertices, 6 indices)
- Attribute type validation for all fields

### Supported Primitives

| Primitive    | Vertices | Indices | Use Case                         |
| ------------ | -------- | ------- | -------------------------------- |
| `"quad"`     | 4        | 6       | Rectangles, circles, diamonds    |
| `"triangle"` | 3        | None    | Arrows, indicators, simple marks |

Quad is the default if no `#[mark(primitive = ...)]` is specified.

### Supported Field Types

| Rust Type | WGSL Type     | Typical Use     |
| --------- | ------------- | --------------- |
| `f32`     | `f32`         | Size, radius    |
| `i32`     | `i32`         | Counts, indices |
| `u32`     | `u32`         | Flags, IDs      |
| `Vec2`    | `vec2<f32>`   | Position        |
| `Vec3`    | `vec3<f32>`   | 3D position     |
| `Vec4`    | `vec4<f32>`   | Color           |
| `Mat2`    | `mat2x2<f32>` | 2D transforms   |
| `Mat3`    | `mat3x3<f32>` | 3D transforms   |
| `Mat4`    | `mat4x4<f32>` | Full transforms |

## Validating Your Mark

Use the `MarkValidator` to automatically check your mark for common issues:

```rust
use gup::mark::validation::{MarkValidator, assert_mark_valid};

// Full validation report
let report = MarkValidator::<Diamond>::validate();
println!("{}", report.summary());
assert!(report.is_passing());

// Quick pass/fail check (returns GupResult)
assert_mark_valid::<Diamond>().unwrap();
```

### What Gets Validated

- **Geometry**: Vertex count matches `generate_vertices()`, index count matches
  `generate_indices()`, indices are in bounds, triangle alignment
- **Memory Layout**: Vertex size is non-zero, alignment is GPU-compatible,
  bytemuck round-trip succeeds
- **Attributes**: Common attribute names resolve to correct WGSL types
- **Shaders**: Vertex and fragment shader constants are paired correctly

### Example Validation Output

```text
=== Validation Report for Diamond ===
Result: PASS
Duration: 17.2µs

✅ Geometry Validation (3.2µs)
✅ Memory Layout Validation (7.3µs)
  [INFO] memory: Vertex size: 8 bytes, alignment: 4 bytes
✅ Attribute Type Validation (1.1µs)
✅ Shader Support Validation (0.8µs)
  [INFO] shaders: Using generated shaders (default implementation)

Summary: 2 issues (0 critical, 0 errors)
```

## Profiling Your Mark

Use `MarkProfiler` to measure vertex generation performance:

```rust
use gup::mark::validation::MarkProfiler;

let profile = MarkProfiler::<Diamond>::profile();
println!("{}", profile.summary());
assert!(profile.vertex_generation_time.as_millis() < 10);
```

### Performance Classification

| Class      | Vertex Gen Time | Meaning                    |
| ---------- | --------------- | -------------------------- |
| Excellent  | < 1μs           | Optimal for GPU rendering  |
| Good       | < 100μs         | Suitable for most uses     |
| Acceptable | < 1ms           | May need optimization      |
| Needs Work | ≥ 1ms           | Optimize vertex generation |

## Migration Path: Derive to Manual

Start with the derive macro for rapid prototyping, then move to a manual
implementation when you need:

- Custom shaders (`VERTEX_SHADER` / `FRAGMENT_SHADER` constants)
- Complex geometry (more than a quad or triangle)
- Pattern-based rendering for accessibility
- Custom vertex attributes beyond position

```rust
// Step 1: Start with derive (quick, <10 lines)
#[derive(Debug, Clone, Mark)]
#[mark(primitive = "quad")]
pub struct MyMark {
    pub center: Vec2,
    pub color: Vec4,
}

// Step 2: When you need more control, implement manually
#[derive(Debug, Clone)]
pub struct MyMark;

impl Mark for MyMark {
    type Vertex = MyMarkVertex;
    type AttributeValue = MyMarkAttributes;

    const VERTEX_SHADER: Option<&'static str> =
        Some(include_str!("shaders/my_mark.vert.wgsl"));
    const FRAGMENT_SHADER: Option<&'static str> =
        Some(include_str!("shaders/my_mark.frag.wgsl"));

    fn vertex_count() -> usize { 4 }
    fn index_count() -> Option<usize> { Some(6) }
    fn generate_vertices() -> Vec<Self::Vertex> { /* ... */ }
    fn generate_indices() -> Option<Vec<u32>> { /* ... */ }
}
```

## Summary

Creating custom marks involves:

1. **Quick path**: Use `#[derive(Mark)]` with a primitive type
2. **Manual path**: Define mark type, vertex type, attributes, and implement the
   `Mark` trait
3. **Validate**: Run `MarkValidator` to catch common issues
4. **Profile**: Use `MarkProfiler` to verify performance
5. Optionally provide hand-optimized shaders for maximum performance

The mark system handles:

- Instanced rendering
- GPU buffer management
- Shader pipeline caching
- Attribute type validation
- Integration with the selection API

Focus on defining clean, efficient mark geometries, and let the system handle
the GPU complexity.

# GUP-010: Basic Mark Implementations

## Story Overview

**Title**: Implement Core Visual Marks (Circle, Rectangle, Line) **Epic**: Phase
1 Initiative 3 - Mark System and Type Integration **Priority**: Critical **Story
Points**: 13

## Context

The core visual marks (Circle, Rectangle, Line) are the fundamental building
blocks for all data visualizations. These implementations must demonstrate the
Mark trait's capabilities while providing high-performance, GPU-optimized
rendering. They serve as reference implementations for custom marks and must
handle common visualization patterns efficiently.

## User Story

**As a** visualization developer **I want** high-performance implementations of
basic visual marks **So that** I can build scatter plots, bar charts, line
charts, and other common visualizations with optimal GPU performance

## Acceptance Criteria

### AC1: Core Mark Types

- [ ] **Circle**: For scatter plots, bubble charts, and point visualizations
- [ ] **Rectangle**: For bar charts, heatmaps, and rectangular regions
- [ ] **Line**: For line charts, connections, and path visualizations

### AC2: Performance Requirements

- [ ] **High Performance**: Hand-optimized WGSL shaders for maximum efficiency
- [ ] **GPU Instancing**: Support rendering thousands of instances efficiently
- [ ] **Attribute Flexibility**: Support position, color, size, and style
      attributes
- [ ] **Memory Efficiency**: Minimal vertex data with instanced attributes

### AC3: Quality Features

- [ ] **Anti-aliasing**: Smooth edges for professional visual quality
- [ ] **Style Options**: Fill, stroke, and combination styling
- [ ] **Accessibility**: High contrast and pattern support for accessibility
- [ ] **Animation Support**: Smooth interpolation for attribute changes

## Technical Tasks

### 1. Circle Mark Implementation

- [ ] Design efficient vertex layout for instanced circle rendering
- [ ] Write hand-optimized WGSL vertex and fragment shaders
- [ ] Implement anti-aliased circle rendering using distance fields
- [ ] Add support for filled, stroked, and outlined circles

### 2. Rectangle Mark Implementation

- [ ] Create vertex layout for instanced rectangle rendering
- [ ] Write optimized WGSL shaders for rectangle drawing
- [ ] Support rounded corners and border radius
- [ ] Implement efficient rectangle clipping and bounds checking

### 3. Line Mark Implementation

- [ ] Design line segment representation with width support
- [ ] Implement anti-aliased line rendering with joins and caps
- [ ] Support different line styles (solid, dashed, dotted)
- [ ] Handle line width scaling and viewport-independent thickness

### 4. Attribute System Integration

- [ ] Define attribute structures for each mark type
- [ ] Implement shader function compatibility
- [ ] Create attribute validation and type checking
- [ ] Add attribute interpolation support for animations

## Detailed Requirements

### Circle Mark

```rust
#[derive(Debug, Clone)]
pub struct Circle;

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CircleVertex {
    // Unit square vertices for instanced rendering
    position: [f32; 2],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CircleAttributes {
    center: [f32; 2],
    radius: f32,
    fill_color: [f32; 4],
    stroke_color: [f32; 4],
    stroke_width: f32,
    _padding: [f32; 3], // Ensure 16-byte alignment
}

impl Mark for Circle {
    type Vertex = CircleVertex;
    type AttributeValue = CircleAttributes;

    const VERTEX_SHADER: Option<&'static str> = Some(include_str!("shaders/circle.vert.wgsl"));
    const FRAGMENT_SHADER: Option<&'static str> = Some(include_str!("shaders/circle.frag.wgsl"));

    fn vertex_count() -> usize { 4 }
    fn index_count() -> Option<usize> { Some(6) }

    fn generate_vertices() -> Vec<Self::Vertex> {
        vec![
            CircleVertex { position: [-1.0, -1.0] },
            CircleVertex { position: [ 1.0, -1.0] },
            CircleVertex { position: [ 1.0,  1.0] },
            CircleVertex { position: [-1.0,  1.0] },
        ]
    }

    fn generate_indices() -> Option<Vec<u32>> {
        Some(vec![0, 1, 2, 0, 2, 3])
    }
}
```

### Circle Shader (WGSL)

```wgsl
// circle.vert.wgsl
struct VertexInput {
    @location(0) position: vec2<f32>,
}

struct InstanceInput {
    @location(1) center: vec2<f32>,
    @location(2) radius: f32,
    @location(3) fill_color: vec4<f32>,
    @location(4) stroke_color: vec4<f32>,
    @location(5) stroke_width: f32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) local_pos: vec2<f32>,
    @location(1) fill_color: vec4<f32>,
    @location(2) stroke_color: vec4<f32>,
    @location(3) stroke_width: f32,
    @location(4) radius: f32,
}

@vertex
fn vs_main(vertex: VertexInput, instance: InstanceInput) -> VertexOutput {
    let world_pos = vertex.position * instance.radius + instance.center;

    var output: VertexOutput;
    output.clip_position = vec4<f32>(world_pos, 0.0, 1.0);
    output.local_pos = vertex.position;
    output.fill_color = instance.fill_color;
    output.stroke_color = instance.stroke_color;
    output.stroke_width = instance.stroke_width;
    output.radius = instance.radius;
    return output;
}

// circle.frag.wgsl
@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let distance = length(input.local_pos);
    let radius = 1.0; // Normalized radius in local space

    // Anti-aliased circle with smooth edges
    let edge_width = fwidth(distance) * 0.5;
    let alpha = 1.0 - smoothstep(radius - edge_width, radius + edge_width, distance);

    // Handle stroke
    if (input.stroke_width > 0.0) {
        let stroke_inner = radius - input.stroke_width / input.radius;
        let stroke_alpha = smoothstep(stroke_inner - edge_width, stroke_inner + edge_width, distance);

        let fill_alpha = alpha * (1.0 - stroke_alpha);
        let stroke_alpha_final = alpha * stroke_alpha;

        return input.fill_color * fill_alpha + input.stroke_color * stroke_alpha_final;
    } else {
        return input.fill_color * alpha;
    }
}
```

### Rectangle Mark

```rust
#[derive(Debug, Clone)]
pub struct Rectangle;

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct RectangleVertex {
    position: [f32; 2],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct RectangleAttributes {
    center: [f32; 2],
    size: [f32; 2],
    fill_color: [f32; 4],
    stroke_color: [f32; 4],
    stroke_width: f32,
    corner_radius: f32,
    _padding: [f32; 2],
}

impl Mark for Rectangle {
    type Vertex = RectangleVertex;
    type AttributeValue = RectangleAttributes;

    const VERTEX_SHADER: Option<&'static str> = Some(include_str!("shaders/rectangle.vert.wgsl"));
    const FRAGMENT_SHADER: Option<&'static str> = Some(include_str!("shaders/rectangle.frag.wgsl"));

    fn vertex_count() -> usize { 4 }
    fn index_count() -> Option<usize> { Some(6) }

    fn generate_vertices() -> Vec<Self::Vertex> {
        vec![
            RectangleVertex { position: [-0.5, -0.5] },
            RectangleVertex { position: [ 0.5, -0.5] },
            RectangleVertex { position: [ 0.5,  0.5] },
            RectangleVertex { position: [-0.5,  0.5] },
        ]
    }

    fn generate_indices() -> Option<Vec<u32>> {
        Some(vec![0, 1, 2, 0, 2, 3])
    }
}
```

### Line Mark

```rust
#[derive(Debug, Clone)]
pub struct Line;

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LineVertex {
    // Line segment with width - 4 vertices per segment
    position: [f32; 2],
    normal: [f32; 2],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LineAttributes {
    start: [f32; 2],
    end: [f32; 2],
    color: [f32; 4],
    width: f32,
    style: u32, // 0 = solid, 1 = dashed, 2 = dotted
    _padding: [f32; 2],
}

impl Mark for Line {
    type Vertex = LineVertex;
    type AttributeValue = LineAttributes;

    const VERTEX_SHADER: Option<&'static str> = Some(include_str!("shaders/line.vert.wgsl"));
    const FRAGMENT_SHADER: Option<&'static str> = Some(include_str!("shaders/line.frag.wgsl"));

    fn vertex_count() -> usize { 4 } // Quad per line segment
    fn index_count() -> Option<usize> { Some(6) }

    fn generate_vertices() -> Vec<Self::Vertex> {
        vec![
            LineVertex { position: [0.0, -0.5], normal: [0.0, -1.0] },
            LineVertex { position: [1.0, -0.5], normal: [0.0, -1.0] },
            LineVertex { position: [1.0,  0.5], normal: [0.0,  1.0] },
            LineVertex { position: [0.0,  0.5], normal: [0.0,  1.0] },
        ]
    }

    fn generate_indices() -> Option<Vec<u32>> {
        Some(vec![0, 1, 2, 0, 2, 3])
    }
}
```

## Dependencies

### Prerequisite Stories

- GUP-009: Core Mark Trait (defines interface being implemented)
- GUP-003: GPU Buffer Management (for vertex and instance buffers)
- GUP-004: Basic Render Context (for shader compilation)

### Enables Stories

- GUP-002: Core Selection Type (uses marks for rendering)
- All visualization implementations that use basic marks

## Testing Strategy

### Unit Tests

```rust
#[test]
fn test_circle_mark() {
    let vertices = Circle::generate_vertices();
    assert_eq!(vertices.len(), 4);

    let indices = Circle::generate_indices().unwrap();
    assert_eq!(indices.len(), 6);

    // Verify vertex data is valid
    for vertex in &vertices {
        assert!(vertex.position[0].abs() <= 1.0);
        assert!(vertex.position[1].abs() <= 1.0);
    }
}

#[test]
fn test_circle_attributes() {
    let attrs = CircleAttributes {
        center: [100.0, 200.0],
        radius: 25.0,
        fill_color: [1.0, 0.0, 0.0, 1.0],
        stroke_color: [0.0, 0.0, 0.0, 1.0],
        stroke_width: 2.0,
        _padding: [0.0; 3],
    };

    // Verify bytemuck conversion
    let bytes = bytemuck::cast_slice(&[attrs]);
    assert_eq!(bytes.len(), std::mem::size_of::<CircleAttributes>());
}

#[test]
fn test_rectangle_mark() {
    let vertices = Rectangle::generate_vertices();
    assert_eq!(vertices.len(), 4);

    // Verify rectangle covers unit square
    let positions: Vec<[f32; 2]> = vertices.iter().map(|v| v.position).collect();
    assert!(positions.contains(&[-0.5, -0.5]));
    assert!(positions.contains(&[0.5, 0.5]));
}
```

### Shader Compilation Tests

```rust
#[test]
async fn test_circle_shader_compilation() {
    let device = create_test_device();

    let vertex_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("circle_vertex"),
        source: wgpu::ShaderSource::Wgsl(Circle::VERTEX_SHADER.unwrap().into()),
    });

    let fragment_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("circle_fragment"),
        source: wgpu::ShaderSource::Wgsl(Circle::FRAGMENT_SHADER.unwrap().into()),
    });

    // If these don't panic, shaders compiled successfully
}

#[test]
async fn test_all_mark_shaders_compile() {
    let device = create_test_device();

    for mark_type in [Circle::VERTEX_SHADER, Rectangle::VERTEX_SHADER, Line::VERTEX_SHADER] {
        if let Some(shader_source) = mark_type {
            let _shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("test_shader"),
                source: wgpu::ShaderSource::Wgsl(shader_source.into()),
            });
        }
    }
}
```

### Visual Tests

```rust
#[test]
async fn test_circle_rendering() {
    let device = create_test_device();
    let mut selection = Selection::<TestData, Circle>::new(test_data, context);

    selection.attr("center", |d| Vec2::new(d.x, d.y));
    selection.attr("radius", |d| d.size);
    selection.attr("color", |d| Vec4::new(d.r, d.g, d.b, 1.0));

    let rendered_texture = selection.render_to_texture(&device).await;

    // Verify rendered output has expected properties
    assert!(validate_circle_rendering(&rendered_texture));
}
```

### Performance Tests

```rust
#[bench]
fn bench_circle_vertex_generation(b: &mut Bencher) {
    b.iter(|| {
        let _vertices = Circle::generate_vertices();
    });
}

#[bench]
fn bench_circle_instance_rendering(b: &mut Bencher) {
    let device = create_bench_device();
    let selection = create_large_circle_selection(10_000);

    b.iter(|| {
        selection.render().unwrap();
    });
}
```

## Success Metrics

### Performance Requirements

- [ ] **Rendering Speed**: 10K circles at 60 FPS on mid-range GPU
- [ ] **Memory Efficiency**: <1KB GPU memory per 1000 instances
- [ ] **Shader Performance**: Comparable to hand-optimized single-purpose
      shaders
- [ ] **Anti-aliasing Quality**: Smooth edges at all zoom levels

### Quality Requirements

- [ ] **Visual Accuracy**: Pixel-perfect rendering compared to reference
      implementations
- [ ] **Cross-Platform**: Identical visual output across all supported platforms
- [ ] **Accessibility**: High contrast mode and pattern support
- [ ] **Professional Quality**: Publication-ready visual output

### Functionality Requirements

- [ ] **Attribute Support**: All common attributes (position, size, color,
      stroke) working
- [ ] **Style Variants**: Fill-only, stroke-only, and combined styles
- [ ] **Edge Cases**: Proper handling of zero-size, extreme aspect ratios, etc.
- [ ] **Animation Ready**: Smooth interpolation for all attribute changes

## Risk Assessment

### Technical Risks

- **Medium**: Anti-aliasing quality might not be consistent across different
  GPUs
- **Medium**: Line rendering complexity could impact performance
- **Low**: Shader compilation issues on specific platforms

### Mitigation Strategies

- **Reference Testing**: Compare output against known-good reference
  implementations
- **Platform Testing**: Test on variety of GPU hardware and drivers
- **Performance Monitoring**: Continuous benchmarking to detect regressions

## Implementation Notes

### Design Decisions

- Use instanced rendering for maximum performance with large datasets
- Implement anti-aliasing using fragment shader distance calculations
- Store vertex data as unit shapes, use instance data for positioning and
  scaling
- Prioritize visual quality while maintaining performance

### Shader Design Strategy

- Use signed distance functions for smooth anti-aliased edges
- Implement stroke and fill in single fragment shader pass
- Use derivative functions (fwidth) for resolution-independent anti-aliasing
- Optimize for GPU instruction parallelism

### Memory Layout Optimization

- Align all attribute structures to 16-byte boundaries for GPU efficiency
- Pack color data as `vec4<f32>` for SIMD optimization
- Use minimal vertex data with maximum instance data for memory efficiency
- Implement proper padding for WGSL std430 layout compliance

## Definition of Done

- [ ] Circle, Rectangle, and Line marks fully implemented
- [ ] Hand-optimized WGSL shaders for all three marks
- [ ] Anti-aliasing working with smooth edges at all scales
- [ ] Instanced rendering supporting thousands of instances efficiently
- [ ] All mark attributes (position, size, color, stroke) functional
- [ ] Cross-platform shader compilation verified
- [ ] Performance benchmarks meet 10K instances at 60 FPS target
- [ ] Visual tests confirm pixel-perfect rendering quality
- [ ] Integration with Selection system working correctly
- [ ] Documentation complete with usage examples
- [ ] Code review completed and approved

# GUP-067: Rectangle and Line Mark Implementations

## Story Overview

**Title**: Implement Rectangle and Line Mark Types  
**Epic**: Phase 1 Initiative 3 - Mark System and Type Integration  
**Priority**: High  
**Story Points**: 5

## Context

Following the successful implementation of the core Mark trait system in
GUP-009, we need to implement the remaining basic mark types: Rectangle and
Line. These fundamental primitives will complete the basic mark system and
provide essential building blocks for data visualization.

## User Story

**As a** visualization developer  
**I want** to use rectangle and line marks alongside circles  
**So that** I can create bar charts, scatter plots with error bars, and other
essential visualization types

## Acceptance Criteria

### AC1: Rectangle Mark Implementation

- [ ] **Rectangle struct**: Zero-sized mark type following Circle pattern
- [ ] **RectangleVertex**: GPU-compatible vertex data for quad rendering
- [ ] **RectangleAttributes**: User-friendly attribute configuration (center,
      width, height, fill_color, stroke_width, stroke_color)
- [ ] **Hand-optimized shaders**: High-performance WGSL shaders for rectangle
      rendering
- [ ] **Generated shader integration**: Works with shader function pipeline

### AC2: Line Mark Implementation

- [ ] **Line struct**: Zero-sized mark type for line segments
- [ ] **LineVertex**: GPU vertex data for line rendering
- [ ] **LineAttributes**: Line configuration (start_point, end_point, color,
      width, dash_pattern)
- [ ] **Anti-aliased rendering**: Smooth line rendering with proper width
      handling
- [ ] **Dash pattern support**: Configurable dash patterns for different line
      styles

### AC3: Integration and Testing

- [ ] **Mark registry compatibility**: Both marks work with MarkRegistry system
- [ ] **Performance validation**: Meet same performance targets as Circle mark
- [ ] **Custom shader support**: Both manual and generated shader paths work
- [ ] **GPU compilation validation**: All shaders compile successfully on actual
      GPU hardware

## Technical Tasks

### 1. Rectangle Mark Implementation

```rust
#[derive(Debug, Clone)]
pub struct Rectangle;

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct RectangleVertex {
    pub position: [f32; 2], // Corner position within unit rectangle
}

#[derive(Debug, Clone)]
pub struct RectangleAttributes {
    pub center: Vec2,
    pub width: f32,
    pub height: f32,
    pub fill_color: Vec4,
    pub stroke_width: f32,
    pub stroke_color: Vec4,
    pub corner_radius: f32, // For rounded rectangles
}

impl Mark for Rectangle {
    type Vertex = RectangleVertex;
    type AttributeValue = RectangleAttributes;

    const VERTEX_SHADER: Option<&'static str> = Some(include_str!("shaders/rectangle.vert.wgsl"));
    const FRAGMENT_SHADER: Option<&'static str> = Some(include_str!("shaders/rectangle.frag.wgsl"));

    fn vertex_count() -> usize { 4 }
    fn index_count() -> Option<usize> { Some(6) }
}
```

### 2. Line Mark Implementation

```rust
#[derive(Debug, Clone)]
pub struct Line;

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LineVertex {
    pub position: [f32; 2],
    pub direction: [f32; 2], // For width calculation
}

#[derive(Debug, Clone)]
pub struct LineAttributes {
    pub start_point: Vec2,
    pub end_point: Vec2,
    pub color: Vec4,
    pub width: f32,
    pub dash_pattern: Option<DashPattern>,
}

#[derive(Debug, Clone)]
pub struct DashPattern {
    pub dash_length: f32,
    pub gap_length: f32,
    pub offset: f32,
}

impl Mark for Line {
    type Vertex = LineVertex;
    type AttributeValue = LineAttributes;

    const VERTEX_SHADER: Option<&'static str> = Some(include_str!("shaders/line.vert.wgsl"));
    const FRAGMENT_SHADER: Option<&'static str> = Some(include_str!("shaders/line.frag.wgsl"));

    fn vertex_count() -> usize { 4 } // Quad for anti-aliased line
    fn index_count() -> Option<usize> { Some(6) }
}
```

## Dependencies

### Prerequisite Stories

- GUP-009: Core Mark Trait (provides Mark trait and registry system)

### Enables Stories

- Advanced mark features (gradients, textures, animations)
- Composite mark system (combining multiple marks)
- Basic visualization examples using all three mark types

## Testing Strategy

### Unit Tests

```rust
#[test]
fn test_rectangle_mark_implementation() {
    assert_eq!(Rectangle::vertex_count(), 4);
    assert_eq!(Rectangle::index_count(), Some(6));

    let vertices = Rectangle::generate_vertices();
    assert_eq!(vertices.len(), 4);

    // Test rectangle-specific vertex positions
    let expected_positions = [
        [-1.0, -1.0], // Bottom-left
        [1.0, -1.0],  // Bottom-right
        [1.0, 1.0],   // Top-right
        [-1.0, 1.0],  // Top-left
    ];

    for (i, vertex) in vertices.iter().enumerate() {
        assert_eq!(vertex.position, expected_positions[i]);
    }
}

#[test]
fn test_line_mark_implementation() {
    assert_eq!(Line::vertex_count(), 4);
    assert_eq!(Line::index_count(), Some(6));

    let vertices = Line::generate_vertices();
    assert_eq!(vertices.len(), 4);

    // Verify line vertex data includes direction information
    for vertex in &vertices {
        assert!(vertex.position[0].is_finite());
        assert!(vertex.position[1].is_finite());
        assert!(vertex.direction[0].is_finite());
        assert!(vertex.direction[1].is_finite());
    }
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_rectangle_gpu_compilation() -> GupResult<()> {
    let context = create_test_context().await?;
    let device = &context.device;

    // Test hand-optimized shaders compile
    if let Some(vertex_shader) = Rectangle::VERTEX_SHADER {
        let _vertex_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("test_rectangle_vertex"),
            source: wgpu::ShaderSource::Wgsl(vertex_shader.into()),
        });
    }

    if let Some(fragment_shader) = Rectangle::FRAGMENT_SHADER {
        let _fragment_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("test_rectangle_fragment"),
            source: wgpu::ShaderSource::Wgsl(fragment_shader.into()),
        });
    }

    Ok(())
}

#[tokio::test]
async fn test_line_performance_targets() -> GupResult<()> {
    // Test line vertex generation performance
    let start = std::time::Instant::now();
    for _ in 0..1000 {
        let _vertices = Line::generate_vertices();
    }
    let duration = start.elapsed();

    assert!(
        duration.as_millis() < 1,
        "Line vertex generation took {:?}",
        duration
    );

    Ok(())
}
```

## Success Metrics

### Functional Requirements

- [ ] **Complete mark set**: Circle, Rectangle, and Line marks all fully
      functional
- [ ] **Shader compilation**: All hand-optimized shaders compile successfully on
      GPU
- [ ] **Performance consistency**: Rectangle and Line marks meet same
      performance targets as Circle
- [ ] **API consistency**: All marks follow same implementation patterns and
      conventions

### Quality Requirements

- [ ] **Visual quality**: Anti-aliased rendering produces smooth, high-quality
      output
- [ ] **Documentation**: Complete rustdoc with visual examples and usage
      patterns
- [ ] **Test coverage**: Comprehensive unit and integration test coverage
- [ ] **Cross-platform**: Identical behavior across all supported platforms

## Implementation Notes

### Design Decisions

- **Quad-based rendering**: Use 4-vertex quads for all marks to enable
  anti-aliasing and complex effects
- **Instance-friendly design**: All marks designed for efficient instanced
  rendering
- **Consistent attribute patterns**: Similar attribute structures across all
  mark types
- **Shader optimization**: Hand-tuned shaders for maximum performance on
  integrated and discrete GPUs

### Rectangle Specific Considerations

- **Corner radius support**: Enable rounded rectangles through shader parameters
- **Aspect ratio handling**: Ensure rectangles maintain correct proportions
  across different scales
- **Stroke rendering**: Proper stroke handling that works with corner radius

### Line Specific Considerations

- **Anti-aliasing strategy**: Use distance fields for smooth line edges
- **Width handling**: Consistent line width regardless of line angle
- **Dash pattern implementation**: Efficient dash pattern calculation in
  fragment shader
- **End cap styles**: Support for different line end cap styles (round, square,
  none)

## Risk Assessment

### Technical Risks

- **Medium**: Line anti-aliasing complexity could impact performance
- **Low**: Rectangle corner radius implementation complexity
- **Low**: Shader compilation differences across GPU vendors

### Mitigation Strategies

- **Performance testing**: Benchmark line rendering against simple rectangle
  marks
- **Fallback shaders**: Provide simplified shader variants for older hardware
- **Cross-platform validation**: Test on multiple GPU vendors and driver
  versions

## Definition of Done

- [ ] Rectangle mark fully implemented with all features
- [ ] Line mark fully implemented with anti-aliasing and dash patterns
- [ ] All hand-optimized shaders compile and render correctly
- [ ] Generated shader integration works for both marks
- [ ] Performance targets met for both vertex generation and rendering
- [ ] Comprehensive test coverage including GPU compilation validation
- [ ] Documentation complete with examples and best practices
- [ ] Visual validation completed manually
- [ ] Code review completed and approved

# GUP-009: Core Mark Trait

## Story Overview

**Title**: Implement Core Mark Trait and System **Epic**: Phase 1 Initiative 3 -
Mark System and Type Integration **Priority**: Critical **Story Points**: 8

## Context

The Mark trait defines the interface that all visual primitives implement. It
bridges high-level visualization concepts (circles, rectangles, lines) with
low-level GPU rendering. The trait must support both hand-optimized shaders for
performance and generated shaders for flexibility, while integrating seamlessly
with the shader function system.

## User Story

**As a** visualization developer **I want** a unified interface for all visual
primitives **So that** I can use circles, rectangles, lines, and custom marks
interchangeably with consistent performance and behavior

## Acceptance Criteria

### AC1: Core Mark Trait Definition

```rust
pub trait Mark: Clone + Send + Sync + 'static {
    type Vertex: bytemuck::Pod + bytemuck::Zeroable;
    type AttributeValue;

    // Pre-written shaders (fastest) or generated shaders (flexible)
    const VERTEX_SHADER: Option<&'static str> = None;
    const FRAGMENT_SHADER: Option<&'static str> = None;

    // Shader generation for dynamic attribute mapping
    fn generate_vertex_shader(pipeline: &ShaderPipeline) -> String {
        pipeline.generate_vertex_shader() // Default implementation
    }

    fn generate_fragment_shader(pipeline: &ShaderPipeline) -> String {
        pipeline.generate_fragment_shader() // Default implementation
    }

    // Geometry generation
    fn vertex_count() -> usize;
    fn index_count() -> Option<usize> { None }

    // Vertex buffer generation
    fn generate_vertices() -> Vec<Self::Vertex>;
    fn generate_indices() -> Option<Vec<u32>> { None }
}
```

### AC2: Mark Capabilities

- [ ] **Flexible Rendering**: Support both manual and generated shaders
- [ ] **Type Safety**: Vertex and attribute types validated at compile time
- [ ] **Performance Options**: Hand-optimized shaders for maximum performance
- [ ] **Extensibility**: Easy to implement custom marks with the trait

### AC3: Integration Requirements

- [ ] **Shader Function Compatibility**: Works seamlessly with shader function
      system
- [ ] **Selection Integration**: Compatible with Selection<T, M> type system
- [ ] **Pipeline Integration**: Integrates with ShaderPipeline for generated
      shaders
- [ ] **GPU Resource Management**: Efficient vertex buffer and pipeline
      management

## Technical Tasks

### 1. Core Trait Definition

- [ ] Define Mark trait with essential associated types and methods
- [ ] Create vertex and attribute type requirements
- [ ] Implement shader integration hooks
- [ ] Add geometry generation interface

### 2. Mark Registration System

- [ ] Create mark registry for runtime mark management
- [ ] Implement mark type identification and lookup
- [ ] Add mark capabilities querying
- [ ] Create mark metadata system

### 3. Rendering Integration

- [ ] Integrate marks with render pipeline creation
- [ ] Handle vertex buffer generation from mark geometry
- [ ] Create attribute binding validation
- [ ] Implement mark-specific render state management

### 4. Extensibility Framework

- [ ] Create helper macros for implementing custom marks
- [ ] Add validation for mark implementations
- [ ] Provide debugging tools for mark development
- [ ] Create mark testing utilities

## Detailed Requirements

### Mark Implementation Pattern

```rust
// Example: Circle mark implementation
#[derive(Debug, Clone)]
pub struct Circle;

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CircleVertex {
    position: [f32; 2],  // Local vertex position within unit circle
}

#[derive(Debug, Clone)]
pub struct CircleAttributes {
    pub center: Vec2,
    pub radius: f32,
    pub color: Vec4,
    pub stroke_width: f32,
    pub stroke_color: Vec4,
}

impl Mark for Circle {
    type Vertex = CircleVertex;
    type AttributeValue = CircleAttributes;

    // High-performance hand-written shaders
    const VERTEX_SHADER: Option<&'static str> = Some(include_str!("shaders/circle.vert.wgsl"));
    const FRAGMENT_SHADER: Option<&'static str> = Some(include_str!("shaders/circle.frag.wgsl"));

    fn vertex_count() -> usize { 4 } // Quad for instanced circle rendering
    fn index_count() -> Option<usize> { Some(6) } // Two triangles

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

### Mark Registry System

```rust
pub struct MarkRegistry {
    marks: HashMap<TypeId, Box<dyn MarkInfo>>,
    pipelines: HashMap<TypeId, wgpu::RenderPipeline>,
}

trait MarkInfo: Send + Sync {
    fn type_name(&self) -> &'static str;
    fn vertex_size(&self) -> usize;
    fn attribute_size(&self) -> usize;
    fn has_custom_shaders(&self) -> bool;
    fn create_render_pipeline(&self, device: &wgpu::Device) -> wgpu::RenderPipeline;
}

impl MarkRegistry {
    pub fn register<M: Mark>(&mut self) {
        let type_id = TypeId::of::<M>();
        let info = Box::new(MarkInfoImpl::<M>::new());
        self.marks.insert(type_id, info);
    }

    pub fn get_pipeline<M: Mark>(&self, device: &wgpu::Device) -> &wgpu::RenderPipeline {
        let type_id = TypeId::of::<M>();
        self.pipelines.entry(type_id)
            .or_insert_with(|| self.create_pipeline_for_mark::<M>(device))
    }
}
```

### Shader Integration

```rust
impl Mark for Circle {
    // When using generated shaders instead of hand-written ones
    fn generate_vertex_shader(pipeline: &ShaderPipeline) -> String {
        let base_shader = r#"
        struct CircleInstance {
            center: vec2<f32>,
            radius: f32,
            color: vec4<f32>,
        }

        @vertex
        fn vs_main(
            @location(0) vertex_pos: vec2<f32>,
            @builtin(instance_index) instance_index: u32
        ) -> VertexOutput {
            let instance = instance_buffer[instance_index];

            // Apply shader functions to instance data
            let world_pos = vertex_pos * instance.radius + instance.center;
            let final_pos = position_transform(world_pos, position_uniforms);
            let final_color = color_transform(instance.color, color_uniforms);

            return VertexOutput {
                @builtin(position) clip_position: vec4<f32>(final_pos, 0.0, 1.0),
                @location(0) color: final_color,
            };
        }
        "#;

        // Integrate with pipeline functions
        pipeline.integrate_vertex_shader(base_shader)
    }
}
```

## Dependencies

### Prerequisite Stories

- GUP-005: Shader Function Trait (for shader integration)
- GUP-007: Shader Pipeline Builder (for generated shaders)
- GUP-003: GPU Buffer Management (for vertex buffers)

### Enables Stories

- GUP-010: Basic Mark Implementations (Circle, Rectangle, Line)
- GUP-002: Core Selection Type (uses marks for rendering)
- GUP-011: Custom Mark System (extensibility features)

## Testing Strategy

### Unit Tests

```rust
#[test]
fn test_mark_trait_implementation() {
    // Test that Circle implements Mark correctly
    assert_eq!(Circle::vertex_count(), 4);
    assert_eq!(Circle::index_count(), Some(6));

    let vertices = Circle::generate_vertices();
    assert_eq!(vertices.len(), 4);

    let indices = Circle::generate_indices().unwrap();
    assert_eq!(indices.len(), 6);
}

#[test]
fn test_mark_registry() {
    let mut registry = MarkRegistry::new();
    registry.register::<Circle>();
    registry.register::<Rectangle>();

    assert!(registry.is_registered::<Circle>());
    assert!(registry.is_registered::<Rectangle>());
    assert!(!registry.is_registered::<Line>());
}

#[test]
fn test_vertex_buffer_generation() {
    let vertices = Circle::generate_vertices();

    // Verify vertex data is valid for GPU upload
    for vertex in &vertices {
        assert!(vertex.position[0].is_finite());
        assert!(vertex.position[1].is_finite());
    }

    // Verify bytemuck conversion works
    let bytes = bytemuck::cast_slice(&vertices);
    assert_eq!(bytes.len(), vertices.len() * std::mem::size_of::<CircleVertex>());
}
```

### Integration Tests

```rust
#[test]
async fn test_mark_render_pipeline() {
    let device = create_test_device();
    let mut registry = MarkRegistry::new();
    registry.register::<Circle>();

    let pipeline = registry.get_pipeline::<Circle>(&device);
    assert!(pipeline.is_valid());
}

#[test]
async fn test_mark_with_shader_functions() {
    let device = create_test_device();
    let mut pipeline = ShaderPipeline::new();
    pipeline.add_function(LinearScale::new(0.0, 1.0, 0.0, 1.0));

    let vertex_shader = Circle::generate_vertex_shader(&pipeline);

    // Test that generated shader compiles
    let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("test_circle_shader"),
        source: wgpu::ShaderSource::Wgsl(vertex_shader.into()),
    });

    // If this doesn't panic, the shader compiled successfully
}
```

### Custom Mark Tests

```rust
#[test]
fn test_custom_mark_implementation() {
    // Test implementing a custom mark
    #[derive(Debug, Clone)]
    struct Triangle;

    #[repr(C)]
    #[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
    struct TriangleVertex {
        position: [f32; 2],
    }

    impl Mark for Triangle {
        type Vertex = TriangleVertex;
        type AttributeValue = TriangleAttributes;

        fn vertex_count() -> usize { 3 }

        fn generate_vertices() -> Vec<Self::Vertex> {
            vec![
                TriangleVertex { position: [0.0, 1.0] },
                TriangleVertex { position: [-1.0, -1.0] },
                TriangleVertex { position: [1.0, -1.0] },
            ]
        }
    }

    // Test the custom mark works with the system
    let vertices = Triangle::generate_vertices();
    assert_eq!(vertices.len(), 3);
}
```

## Success Metrics

### Functional Requirements

- [ ] **Mark Completeness**: All planned marks (Circle, Rectangle, Line)
      implement trait successfully
- [ ] **Shader Integration**: Both manual and generated shaders work correctly
- [ ] **Performance**: Hand-optimized shaders perform within 5% of direct GPU
      code
- [ ] **Extensibility**: Custom marks can be implemented with <50 lines of code

### Quality Requirements

- [ ] **Type Safety**: Invalid mark configurations caught at compile time
- [ ] **Documentation**: Complete rustdoc with implementation examples
- [ ] **Testing**: All mark implementations have comprehensive test coverage
- [ ] **Cross-Platform**: Identical mark behavior across all supported platforms

## Risk Assessment

### Technical Risks

- **Medium**: Shader integration complexity could make trait difficult to
  implement
- **Medium**: Performance overhead from trait abstraction
- **Low**: Mark registry system could have lookup performance issues

### Mitigation Strategies

- **Reference Implementation**: Create Circle mark as reference for other
  implementations
- **Performance Testing**: Benchmark trait overhead against direct
  implementation
- **Simple Design**: Keep trait interface minimal and focused

## Implementation Notes

### Design Decisions

- Use associated types rather than generics for cleaner APIs
- Support both manual and generated shaders for flexibility vs performance
  trade-offs
- Include geometry generation in trait for consistency
- Use bytemuck for safe GPU data transfer

### Shader Strategy

- Prioritize hand-optimized shaders for built-in marks
- Provide generated shader fallback for custom marks
- Allow marks to override generation for specific optimizations
- Include shader validation during mark registration

### Vertex Buffer Strategy

- Generate vertex data on demand rather than caching
- Use consistent vertex layout patterns across marks
- Support both indexed and non-indexed rendering
- Optimize for instanced rendering patterns

## Definition of Done

- [ ] Mark trait compiles and provides expected interface
- [ ] Mark registry system working with type-safe lookup
- [ ] Integration with shader pipeline and function systems verified
- [ ] Hand-optimized and generated shader paths both functional
- [ ] Custom mark implementation verified with test example
- [ ] Performance benchmarks meet overhead targets
- [ ] Cross-platform compatibility validated
- [ ] Documentation complete with implementation guide
- [ ] Code review completed and approved

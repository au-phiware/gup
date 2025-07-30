# GUP-007: Shader Pipeline Builder

## Story Overview

**Title**: Implement Shader Pipeline Builder System **Epic**: Phase 1 Initiative
2 - Unified Shader Function System **Priority**: Critical **Story Points**: 13

## Context

The ShaderPipeline is responsible for taking composed shader functions and
generating optimized WGSL vertex and fragment shaders for the GPU. It must
handle function composition, uniform buffer management, and generate
high-quality WGSL code that leverages GPU parallel processing.

## User Story

**As a** Gup library developer **I want** an automatic shader pipeline
generation system **So that** composed shader functions are efficiently
translated to optimized GPU shaders without manual WGSL coding

## Acceptance Criteria

### AC1: Core Pipeline Structure

```rust
pub struct ShaderPipeline {
    functions: Vec<Box<dyn ShaderFunction>>,
    uniform_buffers: HashMap<String, wgpu::Buffer>,
    attribute_mappings: HashMap<String, String>,
    cached_shaders: Option<CachedShaders>,
}

struct CachedShaders {
    vertex_shader: String,
    fragment_shader: String,
    bind_group_layout: wgpu::BindGroupLayout,
}
```

### AC2: Pipeline Capabilities

- [ ] **Function Composition**: Combine multiple shader functions into unified
      shaders
- [ ] **Automatic WGSL Generation**: Generate complete vertex and fragment
      shaders
- [ ] **Uniform Management**: Handle uniform buffer creation and binding
- [ ] **Pipeline Caching**: Cache generated shaders until functions change

### AC3: Generated Shader Quality

- [ ] **Optimization**: Generated WGSL is efficient and well-structured
- [ ] **Correctness**: Generated shaders compile and run correctly on all
      platforms
- [ ] **Readability**: Generated WGSL is readable for debugging purposes
- [ ] **Performance**: Shader generation adds <5ms overhead for typical
      pipelines

## Technical Tasks

### 1. Core Pipeline Implementation

- [ ] Define ShaderPipeline struct with function storage
- [ ] Implement function registration and composition
- [ ] Create shader generation pipeline
- [ ] Add caching system for generated shaders

### 2. WGSL Generation Engine

- [ ] Build vertex shader generation from function composition
- [ ] Create fragment shader generation system
- [ ] Implement uniform buffer layout generation
- [ ] Add bind group layout creation

### 3. Uniform Buffer Management

- [ ] Automatic uniform buffer creation from function uniforms
- [ ] Efficient uniform buffer packing and alignment
- [ ] Dynamic uniform buffer updates
- [ ] Bind group creation and management

### 4. Pipeline Optimization

- [ ] Dead code elimination for unused uniforms
- [ ] Function inlining for performance
- [ ] Constant folding where possible
- [ ] GPU-specific optimizations

## Detailed Requirements

### Shader Generation API

```rust
impl ShaderPipeline {
    pub fn new() -> Self;

    // Add shader function to pipeline
    pub fn add_function<F: ShaderFunction + 'static>(&mut self, function: F);

    // Map attribute names to function outputs
    pub fn map_attribute(&mut self, attr_name: &str, function_name: &str);

    // Generate complete shader source
    pub fn generate_vertex_shader(&self) -> String;
    pub fn generate_fragment_shader(&self) -> String;

    // Create wgpu resources
    pub fn create_render_pipeline(&self, device: &wgpu::Device) -> wgpu::RenderPipeline;
    pub fn create_bind_group(&self, device: &wgpu::Device) -> wgpu::BindGroup;

    // Update uniform data
    pub fn update_uniforms(&mut self, queue: &wgpu::Queue);
}
```

### Vertex Shader Generation

```rust
impl ShaderPipeline {
    pub fn generate_vertex_shader(&self) -> String {
        let mut shader = String::new();

        // Add data type definitions
        shader.push_str(&self.generate_data_type_definitions());

        // Add uniform buffer bindings
        shader.push_str(&self.generate_uniform_bindings());

        // Add all function definitions
        for function in &self.functions {
            shader.push_str(function.wgsl_function());
            shader.push_str("\n\n");
        }

        // Generate main vertex function
        shader.push_str(&self.generate_main_vertex_function());

        shader
    }

    fn generate_main_vertex_function(&self) -> String {
        format!(r#"
        @vertex
        fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {{
            let data = data_buffer[vertex_index];

            // Apply attribute transformations
            let position = {}(data, position_uniforms);
            let color = {}(data, color_uniforms);
            let size = {}(data, size_uniforms);

            return VertexOutput {{
                @builtin(position) clip_position: vec4<f32>(position, 0.0, 1.0),
                @location(0) color: color,
                @location(1) size: size,
            }};
        }}
        "#,
            self.get_function_name("position"),
            self.get_function_name("color"),
            self.get_function_name("size")
        )
    }
}
```

### Uniform Buffer Layout

```rust
impl ShaderPipeline {
    fn generate_uniform_bindings(&self) -> String {
        let mut bindings = String::new();

        for (i, (name, function)) in self.functions.iter().enumerate() {
            if function.has_uniforms() {
                bindings.push_str(&format!(
                    "@group(0) @binding({}) var<uniform> {}_uniforms: {}Uniforms;\n",
                    i, name, name
                ));
            }
        }

        bindings
    }

    fn create_uniform_buffers(&self, device: &wgpu::Device) -> Vec<wgpu::Buffer> {
        self.functions.iter()
            .filter_map(|function| function.create_uniforms())
            .map(|uniforms| {
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("uniform_buffer"),
                    contents: bytemuck::cast_slice(&[uniforms]),
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                })
            })
            .collect()
    }
}
```

### Shader Optimization

```rust
impl ShaderPipeline {
    fn optimize_shader(&self, shader_source: &str) -> String {
        let mut optimized = shader_source.to_string();

        // Dead code elimination
        optimized = self.remove_unused_uniforms(&optimized);

        // Function inlining for small functions
        optimized = self.inline_small_functions(&optimized);

        // Constant folding
        optimized = self.fold_constants(&optimized);

        optimized
    }

    fn remove_unused_uniforms(&self, shader: &str) -> String {
        // Parse shader and remove unused uniform declarations
        // Implementation uses WGSL AST analysis
        shader.to_string() // Simplified for now
    }
}
```

## Dependencies

### Prerequisite Stories

- GUP-005: Shader Function Trait (provides functions to compose)
- GUP-006: WGSL Function Macro (generates shader functions)
- GUP-003: GPU Buffer Management (for uniform buffers)

### Enables Stories

- GUP-002: Core Selection Type (uses pipeline for rendering)
- GUP-008: Mark System Integration (provides pipeline for marks)
- GUP-010: Example Shader Functions (validated through pipeline)

## Testing Strategy

### Unit Tests

```rust
#[test]
fn test_pipeline_creation() {
    let mut pipeline = ShaderPipeline::new();
    pipeline.add_function(LinearScale::new(0.0, 100.0, 0.0, 1.0));
    pipeline.add_function(ColorMap::new(color_palette));

    assert_eq!(pipeline.function_count(), 2);
}

#[test]
fn test_shader_generation() {
    let mut pipeline = ShaderPipeline::new();
    pipeline.add_function(LinearScale::new(0.0, 100.0, 0.0, 1.0));

    let vertex_shader = pipeline.generate_vertex_shader();
    assert!(vertex_shader.contains("linear_scale"));
    assert!(vertex_shader.contains("@vertex"));
    assert!(vertex_shader.contains("vs_main"));
}

#[test]
fn test_uniform_management() {
    let mut pipeline = ShaderPipeline::new();
    let scale_func = LinearScale::new(0.0, 100.0, 0.0, 1.0);
    pipeline.add_function(scale_func);

    let device = create_test_device();
    let uniforms = pipeline.create_uniform_buffers(&device);
    assert_eq!(uniforms.len(), 1);
}
```

### Integration Tests

```rust
#[test]
async fn test_complete_pipeline() {
    let mut pipeline = ShaderPipeline::new();
    pipeline.add_function(PositionTransform::new());
    pipeline.add_function(ColorMapping::new());

    let device = create_test_device();

    // Test that generated shader compiles
    let vertex_source = pipeline.generate_vertex_shader();
    let vertex_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("test_vertex"),
        source: wgpu::ShaderSource::Wgsl(vertex_source.into()),
    });

    // Test render pipeline creation
    let render_pipeline = pipeline.create_render_pipeline(&device);
    assert!(render_pipeline.is_valid());
}
```

### Performance Tests

```rust
#[bench]
fn bench_shader_generation(b: &mut Bencher) {
    let mut pipeline = create_complex_pipeline(); // 10+ functions

    b.iter(|| {
        let _shader = pipeline.generate_vertex_shader();
    });
}

#[bench]
fn bench_pipeline_caching(b: &mut Bencher) {
    let mut pipeline = create_complex_pipeline();

    // First generation should be slow
    let _first = pipeline.generate_vertex_shader();

    // Subsequent generations should be fast (cached)
    b.iter(|| {
        let _cached = pipeline.generate_vertex_shader();
    });
}
```

### WGSL Validation Tests

```rust
#[test]
fn test_generated_wgsl_validity() {
    let test_functions = create_all_test_functions();

    for function_set in test_functions {
        let mut pipeline = ShaderPipeline::new();
        for func in function_set {
            pipeline.add_function(func);
        }

        let vertex_shader = pipeline.generate_vertex_shader();
        assert!(validate_wgsl_syntax(&vertex_shader));
    }
}
```

## Success Metrics

### Functional Requirements

- [ ] **Shader Quality**: Generated WGSL compiles on all target platforms
- [ ] **Performance**: Shader generation <5ms for typical pipelines (5-10
      functions)
- [ ] **Correctness**: Generated shaders produce expected visual output
- [ ] **Optimization**: Generated shaders perform within 10% of hand-optimized
      equivalents

### Quality Requirements

- [ ] **Test Coverage**: >90% test coverage for pipeline generation logic
- [ ] **Error Handling**: Clear error messages for invalid function compositions
- [ ] **Documentation**: Complete rustdoc with generation examples
- [ ] **Caching Efficiency**: >95% cache hit rate for repeated shader generation

## Risk Assessment

### Technical Risks

- **High**: WGSL generation complexity could produce invalid or inefficient
  shaders
- **Medium**: Uniform buffer layout might not match WGSL requirements
- **Medium**: Caching system could become stale or inconsistent

### Mitigation Strategies

- **Validation Testing**: Comprehensive WGSL validation on multiple platforms
- **Reference Implementation**: Compare generated output against known-good
  shaders
- **Incremental Development**: Start with simple generation, add optimization
  gradually

## Implementation Notes

### Design Decisions

- Generate complete shaders rather than shader fragments for easier debugging
- Use string templates with parameter substitution for consistent output
- Implement caching at shader level rather than function level for simplicity
- Prioritize correctness over optimization in initial implementation

### WGSL Generation Strategy

- Template-based generation with consistent naming conventions
- Automatic function ordering based on dependencies
- Proper WGSL formatting for readability and debugging
- Include generation metadata as WGSL comments

### Uniform Buffer Strategy

- Pack uniforms efficiently following WGSL std430 layout rules
- Group related uniforms to minimize bind group complexity
- Use dynamic offsets for frequently updated uniforms
- Implement automatic alignment and padding

### Caching Strategy

- Hash function list and configuration for cache keys
- Invalidate cache when any function changes
- Store both WGSL source and compiled pipeline objects
- Implement LRU eviction for cache size management

## Definition of Done

- [ ] Pipeline generates valid WGSL for all shader function combinations
- [ ] Generated shaders compile successfully on all target platforms
- [ ] Uniform buffer management works correctly with proper alignment
- [ ] Caching system provides expected performance improvements
- [ ] Integration tests pass with real GPU compilation and execution
- [ ] Performance benchmarks meet <5ms generation target
- [ ] Documentation includes complete shader generation examples
- [ ] Code review completed and approved

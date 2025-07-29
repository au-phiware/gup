# Phase 1: Low-Level Foundation with Unified Shader Functions

## Initiative 1: Core GPU Primitives and Selection API

### Initiative 1: Goals

- Build the fundamental composable primitives that everything else depends on
- Establish rock-solid wgpu integration and Selection system
- Create universal composability trait for D3-style flexibility

### Initiative 1: Core Deliverables

**Universal Composability Foundation**:

```rust
// The fundamental composable unit - everything can be combined
pub trait Composable {
    type Output;

    fn compose<T: Composable>(self, other: T) -> ComposedVisualization<Self, T>
    where Self: Sized;

    fn render(&self, context: &mut RenderContext) -> Result<(), GupError>;
}

// Core selection system - the heart of composability
pub struct Selection<T, M: Mark> {
    data: Vec<T>,
    mark_type: PhantomData<M>,

    // GPU resources
    vertex_buffer: GpuBuffer<M::Vertex>,
    instance_buffer: GpuBuffer<InstanceData>,

    // Shader function pipeline
    shader_pipeline: ShaderPipeline,
    attribute_mappings: HashMap<String, String>,

    context: Arc<GupContext>,
}

impl<T, M: Mark> Selection<T, M> {
    // Attributes are now shader functions, not CPU closures
    pub fn attr<F>(&mut self, name: &str, shader_func: F) -> &mut Self
    where
        F: ShaderFunction + 'static,
        F::Input: Compatible<T>,
        F::Output: Compatible<M::AttributeValue>,
    {
        // Add shader function to the pipeline
        self.shader_pipeline.add_function(shader_func);

        // Map attribute name to function for vertex shader generation
        self.attribute_mappings.insert(
            name.to_string(),
            F::function_name().to_string()
        );

        self.mark_dirty();
        self
    }
}
```

## Initiative 2: Unified Shader Function System

### Initiative 2: Goals

- Implement the `ShaderFunction` trait and composition system
- Create the `#[wgsl_function]` macro for writing WGSL functions
- Build core library of scale, color, and transform functions

### Initiative 2: Core Deliverables

**Universal ShaderFunction Trait**:

```rust
pub trait ShaderFunction {
    type Input;
    type Output;
    type Uniforms: bytemuck::Pod + bytemuck::Zeroable = ();

    // The WGSL function implementation
    fn wgsl_function() -> &'static str;

    // Optional uniform data for GPU
    fn create_uniforms(&self) -> Option<Self::Uniforms> { None }

    // Function name and type info for composition validation
    fn function_name() -> &'static str;
    fn input_type() -> ShaderType;
    fn output_type() -> ShaderType;
}
```

**WGSL Function Macro**:

```rust
// Write functions in WGSL syntax, generate Rust traits automatically
#[wgsl_function]
fn linear_scale(value: f32, scale: LinearScaleUniforms) -> f32 {
    let normalized = (value - scale.domain_min) / (scale.domain_max - scale.domain_min);
    return scale.range_min + normalized * (scale.range_max - scale.range_min);
}

#[wgsl_function]
fn categorical_color(category: u32, colors: ColorPalette) -> vec4<f32> {
    return colors.palette[category % colors.count];
}

#[wgsl_function]
fn polar_to_cartesian(angle: f32, radius: f32) -> vec2<f32> {
    return vec2<f32>(radius * cos(angle), radius * sin(angle));
}
```

**Shader Pipeline Composition**:

```rust
pub struct ShaderPipeline {
    functions: Vec<Box<dyn ShaderFunction>>,
    uniform_buffers: HashMap<String, wgpu::Buffer>,
    function_graph: DependencyGraph,
}

impl ShaderPipeline {
    pub fn add_function<F: ShaderFunction + 'static>(&mut self, func: F) -> &mut Self {
        self.functions.push(Box::new(func));

        // Create uniform buffer if needed
        if let Some(uniforms) = func.create_uniforms() {
            let buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                contents: bytemuck::cast_slice(&[uniforms]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });
            self.uniform_buffers.insert(F::function_name().to_string(), buffer);
        }

        self
    }

    pub fn generate_vertex_shader(&self) -> String {
        let mut shader = String::from("// Auto-generated Gup vertex shader\n\n");

        // Add uniform buffer bindings
        for (i, (name, _)) in self.uniform_buffers.iter().enumerate() {
            shader.push_str(&format!(
                "@group(0) @binding({}) var<uniform> {}: {}Uniforms;\n",
                i, name, name
            ));
        }

        // Add all function definitions
        for func in &self.functions {
            shader.push_str(func.wgsl_function());
            shader.push_str("\n\n");
        }

        // Generate optimized main function
        shader.push_str(&self.generate_main_vertex_function());

        shader
    }
}
```

## Initiative 3: Mark System with Generated Shaders

### Initiative 3: Goals

- Integrate mark system with shader function pipeline
- Enable both manual and generated shaders for marks
- Create library of common visual primitives

### Initiative 3: Core Deliverables

**Enhanced Mark Trait**:

```rust
pub trait Mark: Clone + Send + Sync + 'static {
    type Vertex: bytemuck::Pod + bytemuck::Zeroable;
    type AttributeValue;

    // Option 1: Pre-written shaders (fastest, most control)
    const VERTEX_SHADER: Option<&'static str> = None;
    const FRAGMENT_SHADER: Option<&'static str> = None;

    // Option 2: Generated shaders (more flexible)
    fn generate_vertex_shader(pipeline: &ShaderPipeline) -> String {
        pipeline.generate_vertex_shader() // Default implementation
    }

    fn generate_fragment_shader(pipeline: &ShaderPipeline) -> String {
        pipeline.generate_fragment_shader() // Default implementation
    }

    // Convert attribute functions to GPU vertex format
    fn create_vertex(attributes: Self::AttributeValue) -> Self::Vertex;
}

// Circle with both manual and generated shader support
#[derive(Clone)]
pub struct Circle;

impl Mark for Circle {
    type Vertex = CircleVertex;
    type AttributeValue = CircleAttributes;

    // Manual shaders for maximum performance
    const VERTEX_SHADER: Option<&'static str> = Some(include_str!("shaders/circle.vert.wgsl"));
    const FRAGMENT_SHADER: Option<&'static str> = Some(include_str!("shaders/circle.frag.wgsl"));

    // Fallback to generated shaders if manual ones not provided
    fn generate_vertex_shader(pipeline: &ShaderPipeline) -> String {
        // Generate vertex shader that applies all pipeline functions
        pipeline.generate_vertex_shader_for_mark::<Self>()
    }
}
```

**Usage Examples**:

```rust
// Complex visualization with shader function composition
chart.select_all::<Circle>()
    .data(weather_data)
    // Position: geographic coordinates -> screen coordinates
    .attr("position",
        mercator_projection::new(viewport)
            .compose(screen_transform::new(dimensions))
    )
    // Color: temperature -> color with seasonal adjustment
    .attr("color",
        temperature_scale::new(-10.0, 40.0)
            .compose(seasonal_color_adjustment::new())
            .compose(color_interpolation::new(temp_palette))
    )
    // Size: humidity -> radius with logarithmic scaling
    .attr("radius",
        log_scale::new(0.0, 100.0)
            .compose(clamp_function::new(2.0, 20.0))
    );
```

## Initiative 4: Interaction System and Performance Optimization

### Initiative 4: Goals

- GPU-based spatial queries for interaction
- Performance optimization through batching
- Polish and dog-fooding validation

### Initiative 4: Core Deliverables

**GPU-Based Interaction System**:

```rust
pub struct InteractionSystem {
    // Compute shader for parallel hit testing
    spatial_compute: wgpu::ComputePipeline,
    query_buffer: wgpu::Buffer,
    result_buffer: wgpu::Buffer,
}

impl InteractionSystem {
    pub async fn pick_elements(&mut self,
        position: [f32; 2],
        selections: &[&dyn Renderable]
    ) -> Vec<ElementId> {
        // Use compute shader for parallel hit testing across all selections
        for selection in selections {
            self.query_selection_gpu(selection, position).await;
        }

        // Collect results from GPU memory
        self.collect_hit_results().await
    }
}
```

**Shader Function Interaction Support**:

```rust
// Interaction queries can use shader functions too
#[wgsl_function]
fn point_in_circle(query_pos: vec2<f32>, circle_center: vec2<f32>, radius: f32) -> u32 {
    let distance = length(query_pos - circle_center);
    return select(0u, 1u, distance <= radius);
}

// Automatically generate hit testing for any mark type
impl<T, M: Mark> Selection<T, M> {
    pub fn on<F>(&mut self, event: &str, handler: F) -> &mut Self
    where F: Fn(InteractionEvent, &T) + Send + Sync + 'static
    {
        // Register event handler and generate appropriate hit testing shader
        self.interaction_system.register_hit_test_for_mark::<M>();
        self.event_handlers.insert(event.to_string(), Box::new(handler));
        self
    }
}
```

## Phase 1: Success Metrics

### Technical Validation

- **Shader Function Completeness**: All common transformations (scales, colors,
  coordinates) work as composable WGSL functions
- **Performance Foundation**: 100K points at 60 FPS with complex shader
  function pipelines
- **Composability Proof**: Complex visualizations built by composing simple
  shader functions
- **GPU Integration**: All transformations happen on GPU with no CPU
  bottlenecks

### Dog-fooding Validation

- **Self-hosting**: All internal features built using shader function system
- **Macro Validation**: `#[wgsl_function]` macro works reliably for common
  cases
- **Performance Scalability**: Shader composition doesn't degrade performance
- **Type Safety**: Shader function composition catches type mismatches at
  compile time

### API Ergonomics

- **Natural Composition**: Shader functions compose as naturally as D3 method
  chaining
- **Error Messages**: Clear feedback when shader function types don't match
- **Debug Support**: Generated shaders can be inspected and debugged
- **Flexibility**: Both manual WGSL and macro-generated functions work
  seamlessly

This unified shader function approach ensures that Gup will be uniquely
powerful - every data transformation happens on GPU in parallel, everything
composes naturally like D3, and users can extend the system with custom WGSL
functions when needed.

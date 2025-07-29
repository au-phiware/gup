# Corrected Implementation Strategy: Low-Level Foundation First

## Critical Architectural Insight

**The refined roadmap is fundamentally flawed** - you cannot build a reliable
high-level API without first having a solid, well-tested low-level foundation.
We must **"dog-food" our own low-level API** to ensure it's powerful and
composable enough to support any high-level abstraction.

## Revised Core Principle: Engineering Excellence First

### Why Low-Level API Must Come First

1. **Dog-fooding Requirement**: We must use our own low-level API to build
   everything else
2. **Composability Foundation**: D3's power comes from composable primitives -
   we need those first
3. **Performance Architecture**: GPU optimization requires low-level control
   from the start
4. **API Stability**: High-level APIs built on unstable foundations are brittle

### Universal Composability Trait

Following D3's composability model, everything in Gup should be built around a
universal composable type:

```rust
// The fundamental composable unit - everything can be combined
pub trait Mixable {
    type Output;

    fn mix<T: Mixable>(self, other: T) -> ComposedVisualization<Self, T>
    where
        Self: Sized;

    fn render(&self, context: &mut RenderContext) -> Result<(), GupError>;
}

// Core composable types
impl Mixable for Selection<T, M> { /* ... */ }
impl Mixable for Scale { /* ... */ }
impl Mixable for Axis { /* ... */ }
impl Mixable for Legend { /* ... */ }
impl Mixable for Layout { /* ... */ }

// Composition allows unlimited flexibility
let visualization = chart
    .select_all::<Circle>()
    .data(data)
    .mix(x_axis)
    .mix(y_axis)
    .mix(legend)
    .mix(title);
```

## Revised Phase 1: Low-Level Foundation

### Core GPU Primitives and Selection API

**Focus**: Build the fundamental composable primitives that everything else
depends on.

```rust
// Core selection system - the heart of composability
pub struct Selection<T, M: Mark> {
    data: Vec<T>,
    mark_type: PhantomData<M>,

    // GPU resources
    vertex_buffer: GpuBuffer<M::Vertex>,
    instance_buffer: GpuBuffer<InstanceData>,

    // Attribute bindings
    attribute_functions: HashMap<String, AttributeFunction<T>>,

    // Event handlers
    event_handlers: HashMap<String, EventHandler<T>>,

    context: Arc<GupContext>,
}

impl<T, M: Mark> Selection<T, M> {
    // D3-style data binding
    pub fn data<U>(self, data: Vec<U>) -> Selection<U, M> {
        Selection::new(data, self.context)
    }

    // Enter-update-exit pattern
    pub fn enter(&mut self) -> EnterSelection<T, M> {
        EnterSelection::new(self)
    }

    pub fn exit(&mut self) -> ExitSelection<T, M> {
        ExitSelection::new(self)
    }

    // Attribute binding with type safety
    pub fn attr<F, V>(&mut self, name: &str, value: F) -> &mut Self
    where
        F: Fn(&T) -> V + Send + Sync + 'static,
        V: Into<M::AttributeValue>,
    {
        let attr_fn = AttributeFunction::new(value);
        self.attribute_functions.insert(name.to_string(), attr_fn);
        self.mark_dirty(); // Trigger GPU buffer update
        self
    }

    // Event handling
    pub fn on<F>(&mut self, event: &str, handler: F) -> &mut Self
    where
        F: Fn(InteractionEvent, &T) + Send + Sync + 'static,
    {
        self.event_handlers.insert(event.to_string(), Box::new(handler));
        self
    }
}

// Universal composability
impl<T, M: Mark> Mixable for Selection<T, M> {
    type Output = RenderedSelection;

    fn mix<U: Mixable>(self, other: U) -> ComposedVisualization<Self, U> {
        ComposedVisualization::new(self, other)
    }

    fn render(&self, context: &mut RenderContext) -> Result<(), GupError> {
        // Convert attribute functions to GPU vertex data
        let vertices = self.data.iter()
            .map(|datum| self.create_vertex_from_datum(datum))
            .collect::<Vec<_>>();

        // Upload to GPU and render
        self.vertex_buffer.write(&vertices);
        context.draw_instanced(&self.vertex_buffer, &self.instance_buffer)
    }
}
```

### Scale System and GPU Integration

```rust
// Scales as first-class composable objects
pub trait Scale: Mixable {
    type Domain;
    type Range;

    fn apply(&self, value: Self::Domain) -> Self::Range;
    fn invert(&self, value: Self::Range) -> Self::Domain;

    // GPU optimization
    fn upload_to_gpu(&mut self, device: &wgpu::Device);
    fn as_uniform_buffer(&self) -> &wgpu::Buffer;
}

pub struct LinearScale {
    domain: [f32; 2],
    range: [f32; 2],

    // GPU resources
    uniform_buffer: Option<wgpu::Buffer>,
    bind_group: Option<wgpu::BindGroup>,
}

impl Scale for LinearScale {
    type Domain = f32;
    type Range = f32;

    fn apply(&self, value: f32) -> f32 {
        let t = (value - self.domain[0]) / (self.domain[1] - self.domain[0]);
        self.range[0] + t * (self.range[1] - self.range[0])
    }

    fn upload_to_gpu(&mut self, device: &wgpu::Device) {
        // Create GPU uniform buffer for shader access
        let uniforms = LinearScaleUniforms {
            domain_min: self.domain[0],
            domain_max: self.domain[1],
            range_min: self.range[0],
            range_max: self.range[1],
        };

        self.uniform_buffer = Some(device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Linear Scale Uniforms"),
            contents: bytemuck::cast_slice(&[uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        }));
    }
}

// Scales mix with selections
let chart = Selection::<DataPoint, Circle>::new(data, context)
    .attr("x", |d| x_scale.apply(d.x))  // Scale applied in attribute function
    .attr("y", |d| y_scale.apply(d.y))
    .mix(x_scale)  // Scale becomes part of composed visualization
    .mix(y_scale);
```

### Mark System and Shader Architecture

```rust
// Mark trait defines renderable primitives
pub trait Mark: Clone + Send + Sync + 'static {
    type Vertex: bytemuck::Pod + bytemuck::Zeroable;
    type AttributeValue;

    // Shader specifications
    const VERTEX_SHADER: &'static str;
    const FRAGMENT_SHADER: &'static str;

    // GPU pipeline creation
    fn create_render_pipeline(device: &wgpu::Device, format: wgpu::TextureFormat) -> wgpu::RenderPipeline;

    // Convert data to GPU vertex format
    fn create_vertex(attributes: Self::AttributeValue) -> Self::Vertex;
}

// Circle mark implementation
#[derive(Clone)]
pub struct Circle;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CircleVertex {
    position: [f32; 2],
    radius: f32,
    color: [f32; 4],
}

pub struct CircleAttributes {
    pub position: [f32; 2],
    pub radius: f32,
    pub color: [f32; 4],
}

impl Mark for Circle {
    type Vertex = CircleVertex;
    type AttributeValue = CircleAttributes;

    const VERTEX_SHADER: &'static str = include_str!("shaders/circle.vert.wgsl");
    const FRAGMENT_SHADER: &'static str = include_str!("shaders/circle.frag.wgsl");

    fn create_vertex(attributes: CircleAttributes) -> CircleVertex {
        CircleVertex {
            position: attributes.position,
            radius: attributes.radius,
            color: attributes.color,
        }
    }

    fn create_render_pipeline(device: &wgpu::Device, format: wgpu::TextureFormat) -> wgpu::RenderPipeline {
        // Create GPU pipeline for circle rendering
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Circle Shader"),
            source: wgpu::ShaderSource::Wgsl(format!(
                "{}\n{}",
                Self::VERTEX_SHADER,
                Self::FRAGMENT_SHADER
            ).into()),
        });

        // ... pipeline creation details
    }
}
```

### Interaction System and Performance Optimization

```rust
// GPU-based interaction system
pub struct InteractionSystem {
    spatial_compute: wgpu::ComputePipeline,
    query_buffer: wgpu::Buffer,
    result_buffer: wgpu::Buffer,
}

impl InteractionSystem {
    pub async fn pick_elements(&mut self,
        position: [f32; 2],
        selections: &[&dyn Renderable]
    ) -> Vec<ElementId> {
        // Use compute shader for parallel hit testing
        for selection in selections {
            // Query each selection's vertex buffer
            self.query_selection(selection, position).await;
        }

        // Collect results from GPU
        self.collect_hit_results().await
    }
}

// Performance optimization through batching
pub struct RenderBatch {
    selections: Vec<Box<dyn Renderable>>,
    render_pipeline: wgpu::RenderPipeline,
}

impl RenderBatch {
    pub fn render(&self, encoder: &mut wgpu::CommandEncoder) {
        // Batch multiple selections into single render pass
        let mut render_pass = encoder.begin_render_pass(/* ... */);

        for selection in &self.selections {
            selection.draw(&mut render_pass);
        }
    }
}
```

## Revised Success Metrics for Phase 1

### Technical Validation

- **Core API Completeness**: Selection, data binding, scales, marks working
- **Performance Foundation**: 100K points at 60 FPS with basic interaction
- **Composability Proof**: Complex visualizations built by composing primitives
- **GPU Integration**: All rendering happening on GPU, no CPU bottlenecks

### Dog-fooding Validation

- **Self-hosting**: All high-level features built using low-level API
- **API Ergonomics**: Internal usage reveals pain points and improvements
- **Performance Scalability**: Architecture handles increasing complexity
  gracefully
- **Composability Stress Test**: Complex nested compositions work reliably

## Phase 2: High-Level Convenience APIs

**Only after Phase 1 is rock-solid**, build convenience APIs on top:

```rust
// High-level APIs built on proven low-level foundation
pub fn scatter_plot<T>() -> ScatterPlotBuilder<T> {
    ScatterPlotBuilder::new()
}

impl<T> ScatterPlotBuilder<T> {
    pub fn data(self, data: Vec<T>) -> BoundScatterPlot<T> {
        // Internally uses Selection API we've already validated
        let selection = Selection::<T, Circle>::new(data, context);
        BoundScatterPlot { selection }
    }
}

impl<T> BoundScatterPlot<T> {
    pub fn x<F>(mut self, accessor: F) -> Self
    where F: Fn(&T) -> f32 + Send + Sync + 'static
    {
        // Uses proven attribute binding system
        self.selection.attr("x", accessor);
        self
    }

    pub fn render(self) -> Result<Chart, GupError> {
        // Composes proven primitives
        let x_scale = LinearScale::new().domain_auto(&self.data).range([0.0, 800.0]);
        let y_scale = LinearScale::new().domain_auto(&self.data).range([600.0, 0.0]);

        self.selection
            .mix(x_scale)
            .mix(y_scale)
            .render()
    }
}
```

## Benefits of This Approach

### Engineering Excellence

1. **Solid Foundation**: Every high-level feature built on proven primitives
2. **Performance by Design**: GPU optimization from day one, not retrofitted
3. **Composability Validation**: D3-style flexibility proven through internal usage
4. **API Stability**: Low-level API tested extensively before public exposure

### Competitive Advantage

1. **True D3 Successor**: Captures D3's composability with GPU performance
2. **Performance Moat**: Architecture designed for billion-point datasets
3. **Developer Trust**: Engineering excellence builds confidence in the platform
4. **Extensibility**: Solid foundation enables community contributions

### Market Positioning

1. **Technical Credibility**: Demonstrates deep understanding of visualization architecture
2. **Performance Proof**: Early demos show unprecedented scale and speed
3. **Flexibility Showcase**: Complex visualizations possible from simple primitives
4. **Professional Quality**: Enterprise-ready from initial release

## Revised Timeline

### Phase 1: Low-Level Foundation

- Selection API and basic marks
- Scale system and GPU integration
- Interaction system and performance optimization
- Polish, testing, and dog-fooding validation

### Phase 2: High-Level APIs

- Observable Plot-style convenience APIs
- Documentation, examples, and external validation

### Phase 3: Advanced Features

- Complex layouts, 3D support, statistical computing
- Built on proven foundation with confidence

This corrected approach ensures we build a library with the engineering
excellence and composability that made D3 successful, while achieving the GPU
performance that will make Gup revolutionary.

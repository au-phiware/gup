## Phase 1: Low-Level Foundation (5 months) - Version 0.1.0

### Month 1-2: Core GPU Primitives and Selection API

#### Goals

- Build the fundamental composable primitives that everything else depends on
- Establish rock-solid wgpu integration and Selection system
- Create universal composability trait for D3-style flexibility

#### Core Deliverables

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
    where F: Fn(InteractionEvent, &T) + Send + Sync + 'static
    {
        self.event_handlers.insert(event.to_string(), Box::new(handler));
        self
    }
}

// Universal composability
impl<T, M: Mark> Composable for Selection<T, M> {
    type Output = RenderedSelection;
    
    fn compose<U: Composable>(self, other: U) -> ComposedVisualization<Self, U> {
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

### Month 2-3: Scale System and GPU Integration

**Scales as First-Class Composable Objects**:

```rust
// Scales as composable objects
pub trait Scale: Composable {
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

// Usage: Scales compose with selections
let chart = Selection::<DataPoint, Circle>::new(data, context)
    .attr("x", |d| x_scale.apply(d.x))  // Scale applied in attribute function
    .attr("y", |d| y_scale.apply(d.y))
    .compose(x_scale)  // Scale becomes part of composed visualization
    .compose(y_scale);
```

### Month 3-4: Mark System and Shader Architecture

**Mark Trait Defines Renderable Primitives**:

```rust
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

### Month 4-5: Interaction System and Performance Optimization

**GPU-Based Interaction System**:

```rust
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

### Phase 1 Success Metrics

#### Technical Validation

- **Core API Completeness**: Selection, data binding, scales, marks working
- **Performance Foundation**: 100K points at 60 FPS with basic interaction
- **Composability Proof**: Complex visualizations built by composing primitives
- **GPU Integration**: All rendering happening on GPU, no CPU bottlenecks

#### Dog-fooding Validation

- **Self-hosting**: All internal features built using low-level API
- **API Ergonomics**: Internal usage reveals pain points and improvements
- **Performance Scalability**: Architecture handles increasing complexity gracefully
- **Composability Stress Test**: Complex nested compositions work reliably

## Phase 2: High-Level Convenience APIs (3 months) - Version 0.2.0

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
            .compose(x_scale)
            .compose(y_scale)
            .render()
    }
}

// Observable Plot-style convenience
gup::scatter_plot()
    .data(sales_data)
    .x(|d| d.revenue)
    .y(|d| d.profit)
    .color(|d| d.region)
    .render()?;  // Built on proven Selection primitives
```

### Phase 2 Deliverables

- [ ] Observable Plot-equivalent convenience APIs for common chart types
- [ ] All high-level APIs built using low-level Selection and Scale primitives
- [ ] Migration guide from D3.js showing low-level API power
- [ ] Performance maintained: 100K+ points with high-level APIs
- [ ] Documentation showcasing both API levels

This corrected approach ensures we build a library with the engineering excellence and composability that made D3 successful, while achieving the GPU performance that will make Gup revolutionary.

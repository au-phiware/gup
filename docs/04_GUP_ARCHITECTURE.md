# Gup: GPU-Accelerated Data Visualization Library Architecture

## Vision Statement

**Gup** (GPU Update Pattern) is a high-performance, GPU-first data
visualization library for Rust that brings D3.js-style declarative data binding
to WebGPU/wgpu. It enables real-time visualization of massive datasets with
elegant APIs that feel familiar to D3 users while leveraging the full power of
modern GPU hardware.

## Core Design Principles

### 1. GPU-First Architecture

- **Direct wgpu Integration**: Built on wgpu primitives from the ground up
- **Shader-Based Rendering**: Custom WGSL shaders for all visual encoding
- **Vertex Buffer Centric**: Data stored in GPU-optimized vertex buffer formats
- **Compute Shader Support**: GPU-accelerated data transformations and
  interactions

### 2. Declarative Data Binding

- **D3-Inspired API**: Familiar selection, data binding, and method chaining
  patterns
- **Type-Safe Bindings**: Rust's type system ensures correctness at compile
  time
- **Functional Composition**: Small, composable functions following D3's
  modular philosophy
- **Immutable Data Flow**: Functional transformations with clear data lineage

### 3. Real-Time Performance

- **60+ FPS Capable**: Designed for smooth animation and interaction
- **Massive Scale**: Handle millions of data points efficiently
- **Incremental Updates**: Efficient partial buffer updates for changing data
- **Batched Operations**: Minimize GPU state changes and draw calls

### 4. Cross-Platform Consistency

- **Single API**: Identical behavior on native desktop, web, and mobile
- **WebAssembly Optimized**: Efficient WASM deployment with GPU acceleration
- **No Platform Abstractions**: Direct wgpu access for optimal performance

## Architectural Overview

### Core Components

```ascii
┌─────────────────────────────────────────────────────────────┐
│                      Application Layer                      │
├─────────────────────────────────────────────────────────────┤
│  Chart API  │  Selection API  │  Scale API  │  Interaction  │
├─────────────────────────────────────────────────────────────┤
│                     Data Binding Engine                     │
│  • Data Streams    • Update Pattern    • Transition System  │
├─────────────────────────────────────────────────────────────┤
│                       Rendering Engine                      │
│  • Vertex Buffers  • Shader Pipeline   • Draw Commands      │
├─────────────────────────────────────────────────────────────┤
│                         wgpu Backend                        │
│  • Device/Queue    • Buffers/Textures  • Compute Shaders    │
└─────────────────────────────────────────────────────────────┘
```

## Core API Design

### Data Binding System

> **Note**: Gup uses a unified shader function system where all data
> transformations are composable WGSL functions. See
> `C3_UNIFIED_SHADER_ARCHITECTURE.md` for complete details on how scales, color
> mappings, and custom transforms all work through the same powerful
> abstraction.

#### Selection and Data Binding

```rust
use gup::prelude::*;

// D3-style selection and data binding
let chart = Canvas::new(&device, &config)
    .select_all::<Circle>()           // Select all circle instances
    .data(dataset)                    // Bind data array
    .enter()                          // Handle new data points
    .instance_attributes(|d| CircleAttributes {
        position: [x_scale.apply(d.x), y_scale.apply(d.y)],
        radius: size_scale.apply(d.value),
        color: color_scale.apply(d.category),
    })
    .exit()                           // Handle removed data points
    .remove()
    .render(&mut render_pass);        // Single GPU draw call
```

#### Generic Data Binding

```rust
// Type-safe data binding with compile-time validation
struct SalesData {
    date: DateTime<Utc>,
    revenue: f32,
    region: String,
}

let visualization = chart
    .select_all::<Line>()
    .data::<SalesData>(sales_data)    // Type parameter ensures compatibility
    .enter()
    .attributes(|d: &SalesData| LineAttributes {
        start: [date_scale.apply(d.date), 0.0],
        end: [date_scale.apply(d.date), revenue_scale.apply(d.revenue)],
        color: region_color.apply(&d.region),
        width: 2.0,
    });
```

### Scale System

#### GPU-Accelerated Scales

```rust
// Scales as GPU resources for maximum performance
let x_scale = LinearScale::new()
    .domain([0.0, 100.0])
    .range([0.0, canvas_width])
    .upload_to_gpu(&device);          // Becomes shader uniform

let color_scale = OrdinalScale::new()
    .domain(["A", "B", "C"])
    .range([RED, GREEN, BLUE])
    .upload_to_gpu(&device);

// Scales applied in parallel across all vertices in shaders
```

#### Scale Composition

```rust
// Compose scales functionally
let composite_scale = x_scale
    .compose(log_transform)
    .compose(clamp(0.0, 1.0))
    .invert();                        // Reversible transformations

// Time scales with automatic formatting
let time_scale = TimeScale::new()
    .domain([start_date, end_date])
    .range([0.0, width])
    .nice()                           // Round to nice time boundaries
    .ticks(10);                       // Generate tick positions
```

### Mark System (Visual Primitives)

#### Built-in Mark Types

```rust
// Extensible mark system with GPU-optimized primitives
pub trait Mark {
    type Attributes: VertexAttributes;
    const VERTEX_SHADER: &'static str;
    const FRAGMENT_SHADER: &'static str;

    fn create_vertex_buffer(
        device: &Device,
        attributes: &[Self::Attributes]
    ) -> Buffer;
}

// Pre-built mark implementations
impl Mark for Circle { /* GPU-optimized circle rendering */ }
impl Mark for Rectangle { /* Instanced rectangle rendering */ }
impl Mark for Line { /* Anti-aliased line rendering */ }
impl Mark for Text { /* SDF text rendering */ }
impl Mark for Path { /* Bezier curve rendering */ }
```

#### Custom Mark Creation

```rust
// Define custom marks for specialized visualizations
#[derive(Mark)]
#[vertex_shader = "custom_vertex.wgsl"]
#[fragment_shader = "custom_fragment.wgsl"]
pub struct Hexagon {
    #[attribute(location = 0)]
    center: [f32; 2],
    #[attribute(location = 1)]
    size: f32,
    #[attribute(location = 2)]
    color: [f32; 4],
}

// Use custom marks in visualizations
chart.select_all::<Hexagon>()
    .data(hex_data)
    .enter()
    .attributes(|d| Hexagon {
        center: [d.x, d.y],
        size: d.value * 10.0,
        color: category_colors[d.category],
    });
```

### Interaction System

#### GPU-Based Hit Testing

```rust
// Efficient interaction through compute shaders
impl InteractionHandler {
    fn setup_picking(&mut self, device: &Device) {
        self.picking_compute = ComputePipeline::new(device, "picking.wgsl");
        self.query_buffer = Buffer::new(device, BufferUsages::STORAGE);
    }

    async fn pick_at_position(&mut self, pos: Vec2) -> Option<DataIndex> {
        // Use compute shader for parallel spatial queries
        let query = PickingQuery { position: pos, radius: 5.0 };
        self.query_buffer.write(&[query]);

        let mut encoder = self.device.create_command_encoder();
        self.picking_compute.dispatch(&mut encoder, 1, 1, 1);
        encoder.finish();

        // Read results from GPU
        let results: Vec<PickingResult> = self.query_buffer.read().await;
        results.first().and_then(|r| r.hit_index)
    }
}
```

#### Declarative Event Handling

```rust
// D3-style event handling with GPU optimization
chart.select_all::<Circle>()
    .data(data)
    .on("click", |event, datum| {
        println!("Clicked on: {:?}", datum);
        // Trigger state updates that batch to GPU
    })
    .on("hover", |event, datum| {
        // Highlight through shader uniform updates
        highlight_uniform.set_target(datum.id);
    })
    .transition()
    .duration(200)
    .attr("radius", |d| d.highlighted_size);
```

### Animation and Transition System

#### GPU-Accelerated Transitions

```rust
// Smooth animations through shader interpolation
chart.select_all::<Circle>()
    .data(new_data)
    .transition()
    .duration(Duration::from_millis(1000))
    .ease(EaseFunction::CubicInOut)
    .attr("position", |d| [new_x_scale.apply(d.x), new_y_scale.apply(d.y)])
    .attr("color", |d| new_color_scale.apply(d.category))
    .on_end(|| println!("Transition complete"));

// Implementation: GPU lerps between keyframes
// No per-element JavaScript calculations required
```

#### Timeline-Based Animations

```rust
// Complex animation sequences
let timeline = Timeline::new()
    .at(0.0, |chart| {
        chart.attr("opacity", 0.0)
             .attr("scale", 0.0)
    })
    .at(0.5, |chart| {
        chart.attr("opacity", 1.0)
             .attr("scale", 1.2)  // Overshoot
    })
    .at(1.0, |chart| {
        chart.attr("scale", 1.0)  // Settle
    })
    .play(&mut render_context);
```

## Advanced Features

### Compute Shader Integration

#### Data Processing on GPU

```rust
// Statistical computations on GPU
let histogram = chart
    .select_data(numeric_data)
    .compute_histogram(50)            // 50 bins
    .await;                           // Async GPU computation

let clusters = chart
    .select_data(point_data)
    .k_means_clustering(5)            // 5 clusters
    .iterations(100)
    .await;                           // Async GPU computation

// After await completes, results are available for visualization
chart.select_all::<Circle>()
    .data(clusters.points)           // clusters is now available
    .attr("color", |d| cluster_colors[d.cluster_id]);

// Alternative: Async data binding for non-blocking visualization
chart.select_all::<Circle>()
    .data_async(chart
        .select_data(point_data)
        .k_means_clustering(5)       // Returns Future<KMeansResult>
        .iterations(100))            // chart updates when ready
    .attr("color", |d| cluster_colors[d.cluster_id]);
```

#### Real-Time Aggregations

```rust
// Live data aggregation for streaming visualizations
let aggregator = chart
    .create_aggregator::<StreamingData>()
    .window(Duration::from_secs(60))   // 1-minute sliding window
    .group_by(|d| d.category)
    .aggregate(|group| AggregateOps {
        count: group.len(),
        mean: group.mean(|d| d.value),
        max: group.max(|d| d.value),
    });

// Automatically updates visualization as data streams in
aggregator.on_update(|results| {
    chart.select_all::<Bar>()
        .data(results)
        .attr("height", |d| height_scale.apply(d.count));
});
```

### Layout System

#### GPU-Accelerated Layouts

```rust
// Force-directed layout computed on GPU
let force_layout = ForceLayout::new()
    .charge(-50.0)
    .link_distance(30.0)
    .iterations(100)
    .compute_on_gpu(&device, &graph_data)
    .await;

// Results directly used for positioning
chart.select_all::<Circle>()
    .data(force_layout.nodes)
    .attr("position", |node| [node.x, node.y]);

chart.select_all::<Line>()
    .data(force_layout.edges)
    .attr("start", |edge| edge.source.position)
    .attr("end", |edge| edge.target.position);
```

#### Hierarchical Layouts

```rust
// Treemap layout with GPU parallel computation
let treemap = TreemapLayout::new()
    .size([width, height])
    .padding(2.0)
    .squarify()                       // Optimal aspect ratios
    .compute(&hierarchy_data);

// Render as nested rectangles
chart.select_all::<Rectangle>()
    .data(treemap.leaves())
    .attr("bounds", |d| d.bounds)
    .attr("color", |d| depth_color_scale.apply(d.depth));
```

## Performance Characteristics

### Memory Efficiency

- **Compact Vertex Data**: Optimized data layouts for GPU memory
- **Streaming Updates**: Partial buffer updates for dynamic data
- **Automatic Culling**: GPU-based frustum and detail culling
- **Level of Detail**: Automatic quality reduction for distant/small
  elements

### Cross-Platform Performance

- **Native**: Direct GPU access through wgpu
- **WebAssembly**: Hardware-accelerated WebGPU with WASM performance
- **Mobile**: Optimized for mobile GPU architectures
- **Headless**: Server-side rendering for chart generation

## Integration Examples

### Bevy Game Engine Integration

```rust
use bevy::prelude::*;
use gup::bevy::*;

fn setup_dashboard(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut gup_context: ResMut<GupContext>,
) {
    // Embed Gup charts in Bevy UI
    commands.spawn((
        NodeBundle {
            style: Style {
                width: Val::Px(800.0),
                height: Val::Px(600.0),
                ..default()
            },
            ..default()
        },
        GupChart::new()
            .data(game_metrics)
            .chart_type(LineChart::new())
            .real_time_updates(true),
    ));
}
```

### Web Application Integration

```rust
// WASM integration with web frameworks
#[wasm_bindgen]
pub struct WebDashboard {
    gup_context: GupContext,
    canvas: web_sys::HtmlCanvasElement,
}

#[wasm_bindgen]
impl WebDashboard {
    #[wasm_bindgen(constructor)]
    pub fn new(canvas_id: &str) -> Self {
        let canvas = get_canvas_by_id(canvas_id);
        let gup_context = GupContext::new_web(&canvas).await;

        Self { gup_context, canvas }
    }

    #[wasm_bindgen]
    pub fn update_data(&mut self, data: &JsValue) {
        let parsed_data: Vec<DataPoint> = data.into_serde().unwrap();
        self.gup_context.update_chart_data(parsed_data);
    }
}
```

### Desktop Application Integration

```rust
use winit::*;
use gup::winit::*;

fn main() -> Result<()> {
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("Gup Dashboard")
        .build(&event_loop)?;

    let mut gup_app = GupApplication::new(&window).await?;

    // Setup visualization
    let chart = gup_app.create_chart()
        .data(load_csv_data("data.csv")?)
        .mark::<Scatter>()
        .encode(|d| ScatterAttributes {
            x: d.revenue,
            y: d.profit,
            color: d.region,
            size: d.market_cap,
        });

    event_loop.run(move |event, _, control_flow| {
        match event {
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                *control_flow = ControlFlow::Exit;
            }
            Event::RedrawRequested(_) => {
                gup_app.render().unwrap();
            }
            _ => {}
        }
    });
}
```

## Comparison with Existing Solutions

### vs. D3.js

**Advantages**:

- 10-1000x better performance for large datasets
- Real-time animation capabilities
- Type safety eliminates runtime errors
- Cross-platform native and web deployment

**Trade-offs**:

- Smaller ecosystem
- Learning curve for GPU concepts
- Less web-specific DOM integration

### vs. Plotters

**Advantages**:

- Dynamic, real-time visualizations
- Interactive capabilities
- GPU acceleration
- Consistent cross-platform behavior

**Trade-offs**:

- More complex for simple static charts
- Higher initial setup complexity
- GPU dependency

### vs. Three.js/WebGL Libraries

**Advantages**:

- Specialized for data visualization
- Declarative data binding API
- Built-in scales, axes, and chart components
- Type safety and performance of Rust

**Trade-offs**:

- Less general-purpose 3D capabilities
- Newer ecosystem

## Development Roadmap

### Phase 1: Core Foundation (0.1.0)

- [ ] Basic wgpu integration and device management
- [ ] Core selection and data binding API
- [ ] Linear and ordinal scale implementations
- [ ] Circle, rectangle, and line mark types
- [ ] Simple transition system

### Phase 2: Advanced Rendering (0.2.0)

- [ ] Text rendering with SDF fonts
- [ ] Path rendering for complex shapes
- [ ] Compute shader integration
- [ ] Advanced easing and animation timelines
- [ ] Basic interaction system

### Phase 3: Layout and Complex Charts (0.3.0)

- [ ] Force-directed layout
- [ ] Hierarchical layouts (treemap, pack, partition)
- [ ] Geographic projections
- [ ] Multi-series chart support
- [ ] Axis and legend components

### Phase 4: Performance and Ecosystem (0.4.0)

- [ ] Level-of-detail rendering
- [ ] Streaming data support
- [ ] Integration packages (Bevy, egui, winit)
- [ ] WebAssembly optimization
- [ ] Comprehensive documentation and examples

### Phase 5: Advanced Features (0.5.0)

- [ ] 3D visualization support
- [ ] Advanced statistical computations
- [ ] Real-time data connectors
- [ ] Export capabilities (PNG, SVG, PDF)
- [ ] Accessibility features

This architecture positions Gup as the premier choice for
high-performance, interactive data visualization in Rust
applications, combining the declarative elegance of D3.js with the
raw power of modern GPU computing.

# Refined Implementation Roadmap: Gup GPU-Accelerated Visualization

> **Architecture Update**: Gup uses a unified shader function system where all
> data transformations (scales, color mappings, coordinate transforms, etc.)
> are composable WGSL functions that run on the GPU. See
> `C3_UNIFIED_SHADER_ARCHITECTURE.md` for complete technical details.

## Strategic Development Approach (Corrected)

### Core Philosophy: "Engineering Excellence First, Composability Always"

1. **Start with Low-Level Foundation**: Build rock-solid GPU primitives and
   Selection API
2. **Dog-food Our Own API**: Use low-level API internally to ensure it's
   powerful enough
3. **Universal Composability**: Everything must compose naturally like D3's
   primitives
4. **High-Level APIs Second**: Build Observable Plot-style convenience on
   proven foundation

### Quality Gates (Enhanced)

Each phase must demonstrate:

- **Composability Proof**: Complex visualizations built by composing simple
  primitives
- **Dog-fooding Success**: All higher-level features built using our own
  low-level API
- **Performance Foundation**: GPU-optimized architecture handling 100K+ points
  smoothly
- **Cross-Platform Identical**: Native, web, and mobile work identically
- **Engineering Excellence**: Clean, well-tested, reliable foundation for
  building upon

## Phase 1: Low-Level Foundation (5 months) - Version 0.1.0

### Month 1-2: Core GPU Primitives and Selection API

#### Goals

- Build the fundamental composable primitives that everything else depends on
- Establish rock-solid wgpu integration and Selection system
- Create universal composability trait for D3-style flexibility

#### Phase 1: Core Deliverables

**High-Level API Implementation**:

```rust
// Basic infrastructure
pub struct GupContext {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface_manager: SurfaceManager,
    memory_pool: GpuMemoryPool,
}

// Observable Plot-style API
pub fn plot() -> PlotBuilder {
    PlotBuilder::new()
}

impl PlotBuilder {
    pub fn data<T: ChartData>(self, data: Vec<T>) -> DataBoundPlot<T> {
        DataBoundPlot::new(data)
    }
}

impl<T: ChartData> DataBoundPlot<T> {
    // One-line scatter plot
    pub fn scatter(self, x: impl FieldSelector<T>, y: impl FieldSelector<T>) -> ScatterPlot<T> {
        ScatterPlot::new(self.data, x, y)
    }

    // One-line line chart
    pub fn line(self, x: impl FieldSelector<T>, y: impl FieldSelector<T>) -> LinePlot<T> {
        LinePlot::new(self.data, x, y)
    }

    // One-line histogram
    pub fn histogram(self, field: impl FieldSelector<T>, bins: u32) -> Histogram<T> {
        Histogram::new(self.data, field, bins)
    }
}

// Usage examples
gup::plot().data(sales_data).scatter(x("revenue"), y("profit")).render()?;
gup::plot().data(time_series).line(x("date"), y("value")).render()?;
gup::plot().data(measurements).histogram("value", 50).render()?;
```

**Performance Foundation**:

```rust
// GPU-optimized rendering primitives
pub struct GpuRenderer {
    // Instanced rendering for maximum performance
    instance_buffer_pool: BufferPool<InstanceData>,

    // Pre-compiled shader variants
    shader_cache: ShaderCache,

    // Batched draw call system
    batch_renderer: BatchRenderer,
}

impl GpuRenderer {
    pub fn render_scatter_instanced(&mut self, points: &[Point2D]) -> RenderResult {
        // Convert points to instance data
        let instances: Vec<InstanceData> = points.iter()
            .map(|p| InstanceData {
                position: [p.x, p.y],
                color: p.color,
                size: p.size,
            })
            .collect();

        // Single GPU draw call for all points
        self.batch_renderer.draw_instanced(
            &self.circle_mesh,
            &instances,
            &self.default_material
        )
    }
}
```

#### Phase 1: Success Metrics

- **API Simplicity**: Scatter plot in 1 line (Observable Plot parity)
- **Performance**: 100,000 points at 60 FPS (100x better than D3)
- **Cross-Platform**: Identical behavior on native Windows, Mac, Linux, and web
- **Build Time**: <30 seconds for all examples and demos

### Accessibility + Developer Experience

#### Accessibility-First Implementation

```rust
// Accessibility foundation
pub struct AccessibilityManager {
    // Screen reader support from day one
    aria_announcer: AriaLiveAnnouncer,
    semantic_analyzer: DataSemanticAnalyzer,

    // Keyboard navigation
    keyboard_navigator: KeyboardNavigator,
    focus_manager: FocusManager,

    // Visual accessibility
    high_contrast_renderer: HighContrastRenderer,
    color_blind_support: ColorBlindnessSupport,
}

impl AccessibilityManager {
    pub fn describe_chart(&self, chart: &Chart) -> String {
        let analysis = self.semantic_analyzer.analyze(&chart.data);
        format!(
            "Scatter plot with {} data points. X-axis shows {} ranging from {} to {}. Y-axis shows {} ranging from {} to {}. Main trend: {}.",
            chart.data.len(),
            chart.x_axis.label,
            analysis.x_range.min,
            analysis.x_range.max,
            chart.y_axis.label,
            analysis.y_range.min,
            analysis.y_range.max,
            analysis.primary_trend
        )
    }

    pub fn enable_keyboard_navigation(&mut self, chart: &mut Chart) {
        // Arrow keys navigate between data points
        // Space announces current point
        // Enter activates/selects point
        self.keyboard_navigator.attach_to_chart(chart);
    }
}
```

#### Developer Experience Tools

```rust
// Debug and development tools
#[cfg(debug_assertions)]
pub struct GupDebugger {
    performance_monitor: PerformanceMonitor,
    memory_inspector: MemoryInspector,

    // Visual debugging
    wireframe_renderer: WireframeRenderer,
    buffer_visualizer: BufferVisualizer,
}

impl GupDebugger {
    pub fn performance_report(&self) -> PerformanceReport {
        PerformanceReport {
            frame_time: self.performance_monitor.average_frame_time(),
            gpu_utilization: self.performance_monitor.gpu_utilization(),
            memory_usage: self.memory_inspector.current_usage(),
            bottlenecks: self.identify_bottlenecks(),
        }
    }
}
```

### First Major Validation

#### External Testing Program

- Partner with 5-10 Rust projects needing visualization
- Collect feedback on API ergonomics and performance
- Validate cross-platform deployment process
- Test accessibility features with actual screen reader users

#### Performance Benchmarking

- Formal benchmarks against D3.js, Observable Plot, Plotters
- Memory usage profiling and optimization
- Mobile device performance validation
- WebAssembly bundle size optimization

### Phase 1 Deliverables

- [ ] Observable Plot-equivalent API for scatter, line, and histogram charts
- [ ] 100,000+ points rendering at 60 FPS on moderate hardware
- [ ] Full accessibility support (screen reader, keyboard, high contrast)
- [ ] Cross-platform builds (Windows, macOS, Linux, web)
- [ ] Debug tools and performance monitoring
- [ ] Comprehensive documentation with migration guide from Observable Plot/D3
- [ ] 10+ external validation projects

## Phase 2: D3-Style API + Real-Time Capabilities - Version 0.2.0

### Phase 2: Goals

- Add low-level D3-style API for customization
- Implement real-time data streaming and updates
- Add interaction system with GPU-based hit testing
- Enable smooth transitions and animations

### D3-Style Low-Level API

```rust
// D3-inspired selection and data binding
pub struct Selection<T, M: Mark> {
    data: Vec<T>,
    mark_type: PhantomData<M>,
    context: Arc<GupContext>,
}

impl<T, M: Mark> Selection<T, M> {
    pub fn data<U>(self, data: Vec<U>) -> Selection<U, M> {
        Selection::new(data, self.context)
    }

    pub fn enter(&mut self) -> EnterSelection<T, M> {
        EnterSelection::new(self)
    }

    pub fn exit(&mut self) -> ExitSelection<T, M> {
        ExitSelection::new(self)
    }

    pub fn attr<F, V>(&mut self, name: &str, value: F) -> &mut Self
    where
        F: Fn(&T) -> V + 'static,
        V: Into<M::AttributeValue>,
    {
        self.attribute_functions.insert(name.to_string(), Box::new(value));
        self
    }

    pub fn on<F>(&mut self, event: &str, handler: F) -> &mut Self
    where
        F: Fn(InteractionEvent, &T) + 'static,
    {
        self.event_handlers.insert(event.to_string(), Box::new(handler));
        self
    }
}

// Seamless integration with high-level API
impl DataBoundPlot<T> {
    pub fn customize<F>(self, customizer: F) -> CustomChart<T>
    where
        F: FnOnce(Selection<T, Circle>) -> Selection<T, Circle>,
    {
        let selection = Selection::new(self.data, self.context);
        let customized = customizer(selection);
        CustomChart::new(customized)
    }
}

// Usage: Start simple, add complexity
let chart = gup::plot()
    .data(data)
    .scatter(x("x"), y("y"))
    .customize(|selection| {
        selection
            .attr("radius", |d| size_scale.apply(d.value))
            .on("click", |event, datum| {
                println!("Clicked: {:?}", datum);
            })
    });
```

### Real-Time Data Streaming

```rust
// High-performance streaming data support
pub struct DataStream<T> {
    buffer: CircularGpuBuffer<T>,
    aggregator: Option<Box<dyn StreamAggregator<T>>>,
    update_frequency: Duration,
}

impl<T: ChartData> DataStream<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: CircularGpuBuffer::new(capacity),
            aggregator: None,
            update_frequency: Duration::from_millis(16), // 60 FPS default
        }
    }

    pub fn push(&mut self, item: T) {
        self.buffer.push(item);

        // Trigger chart update if needed
        if self.should_update() {
            self.notify_subscribers();
        }
    }

    pub fn subscribe<F>(&mut self, callback: F)
    where
        F: Fn(&[T]) + Send + 'static,
    {
        self.subscribers.push(Box::new(callback));
    }
}

// Real-time visualization
let mut stream = DataStream::<SensorReading>::new(10_000);
let chart = gup::plot()
    .data_stream(&stream)
    .line(x("timestamp"), y("value"))
    .render_live()?; // Updates automatically as data arrives

// Feed real-time data
loop {
    let reading = sensor.read_value();
    stream.push(reading); // Chart updates automatically
}
```

### GPU-Accelerated Interactions

```rust
// GPU-based spatial queries for interaction
pub struct InteractionSystem {
    spatial_query_compute: ComputePipeline,
    interaction_buffer: Buffer,
    event_dispatcher: EventDispatcher,
}

impl InteractionSystem {
    pub async fn pick_at_position(&mut self,
        position: [f32; 2],
        radius: f32
    ) -> Vec<DataIndex> {
        // Use compute shader for parallel hit testing
        let query = PickQuery { position, radius };
        self.interaction_buffer.write(&[query]);

        // Dispatch compute shader
        let mut encoder = self.device.create_command_encoder();
        let mut compute_pass = encoder.begin_compute_pass();
        compute_pass.set_pipeline(&self.spatial_query_compute);
        compute_pass.dispatch_workgroups(
            (self.data_count + 63) / 64, 1, 1
        );
        drop(compute_pass);

        // Read results (typically <1ms for millions of points)
        self.read_interaction_results().await
    }

    pub fn enable_hover(&mut self, callback: impl Fn(HoverEvent) + 'static) {
        self.hover_callback = Some(Box::new(callback));
    }
}
```

### Phase 2: Deliverables

- [ ] Complete D3-style low-level API with selections, data binding, and events
- [ ] Real-time data streaming with <1ms update latency
- [ ] GPU-accelerated interaction system (hover, click, brush)
- [ ] Smooth transitions and animations at 60 FPS
- [ ] 1,000,000+ points with full interactivity
- [ ] Migration tools from D3.js code to Gup

## Phase 3: Advanced Visualizations + Billion-Point Performance - Version 0.3.0

### Phase 3: Goals

- Achieve billion-point rendering performance
- Implement complex layout algorithms on GPU
- Add geographic and network visualization support
- Create professional chart components (axes, legends)

### Billion-Point Architecture

```rust
// Hierarchical level-of-detail for extreme scale
pub struct BillionPointRenderer {
    // Multi-resolution data pyramid
    lod_pyramid: Vec<GpuBuffer<VertexData>>,

    // Adaptive rendering based on viewport and performance
    adaptive_renderer: AdaptiveRenderer,

    // Streaming data management
    streaming_manager: StreamingDataManager,
}

impl BillionPointRenderer {
    pub fn build_lod_pyramid(&mut self, data: &[DataPoint]) {
        // Build multi-resolution representations
        let mut current_data = data.to_vec();
        let mut lod_level = 0;

        while current_data.len() > 1000 {
            // Upload current level to GPU
            self.lod_pyramid.push(
                GpuBuffer::from_data(&current_data)
            );

            // Generate next level (reduce by ~75%)
            current_data = self.downsample(&current_data, 0.25);
            lod_level += 1;
        }
    }

    pub fn render_adaptive(&mut self, viewport: Viewport) -> RenderResult {
        // Select appropriate LOD based on viewport and data density
        let lod_level = self.calculate_optimal_lod(viewport);
        let data_buffer = &self.lod_pyramid[lod_level];

        // Use compute shader for frustum culling
        let visible_indices = self.frustum_cull_compute(
            data_buffer,
            viewport
        ).await;

        // Render visible subset
        self.render_indexed(data_buffer, &visible_indices)
    }
}
```

### Advanced Layout Algorithms

```rust
// GPU-accelerated layout computations
pub struct LayoutEngine {
    compute_device: Arc<wgpu::Device>,

    // Pre-compiled layout algorithms
    force_directed_pipeline: ComputePipeline,
    treemap_pipeline: ComputePipeline,
    graph_layout_pipeline: ComputePipeline,
}

impl LayoutEngine {
    pub async fn force_directed_layout(&self,
        nodes: &[Node],
        edges: &[Edge],
        iterations: u32
    ) -> LayoutResult {
        // GPU-parallel force simulation
        let mut node_buffer = GpuBuffer::from_data(nodes);
        let edge_buffer = GpuBuffer::from_data(edges);

        for _ in 0..iterations {
            // Compute forces in parallel
            self.compute_forces(&mut node_buffer, &edge_buffer).await;

            // Update positions in parallel
            self.update_positions(&mut node_buffer).await;

            // Check convergence (optional early termination)
            if self.has_converged(&node_buffer).await {
                break;
            }
        }

        // Results ready for immediate rendering
        LayoutResult {
            nodes: node_buffer.read().await,
            iterations_completed: iterations,
        }
    }
}
```

### Geographic and Network Visualization

```rust
// Geographic projection system
pub struct GeoProjectionEngine {
    projection_compute: ComputePipeline,
    tile_manager: TileManager,
}

impl GeoProjectionEngine {
    pub fn project_coordinates(&self,
        coordinates: &[GeoCoordinate],
        projection: ProjectionType
    ) -> Vec<[f32; 2]> {
        // GPU-parallel coordinate projection
        match projection {
            ProjectionType::Mercator => self.mercator_project_compute(coordinates),
            ProjectionType::AlbersUSA => self.albers_project_compute(coordinates),
            // ... additional projections
        }
    }
}

// Network visualization with edge bundling
pub struct NetworkRenderer {
    edge_bundling_compute: ComputePipeline,
    node_clustering_compute: ComputePipeline,
}

impl NetworkRenderer {
    pub async fn render_network_with_bundling(&mut self,
        nodes: &[NetworkNode],
        edges: &[NetworkEdge]
    ) -> NetworkVisualization {
        // Compute edge bundling on GPU
        let bundled_edges = self.compute_edge_bundling(edges).await;

        // Render nodes and bundled edges
        NetworkVisualization {
            nodes: self.render_nodes(nodes),
            edges: self.render_bundled_edges(&bundled_edges),
        }
    }
}
```

### Professional Chart Components

```rust
// Professional axis system
pub struct Axis {
    scale: Box<dyn Scale>,
    orientation: AxisOrientation,
    tick_generator: TickGenerator,
    label_renderer: TextRenderer,
}

impl Axis {
    pub fn render(&self, bounds: Rect) -> Vec<RenderCommand> {
        let ticks = self.tick_generator.generate_ticks(
            &self.scale,
            bounds.width_or_height()
        );

        let mut commands = Vec::new();

        // Render axis line
        commands.push(self.render_axis_line(bounds));

        // Render tick marks and labels
        for tick in ticks {
            commands.push(self.render_tick_mark(tick, bounds));
            commands.push(self.render_tick_label(tick, bounds));
        }

        commands
    }
}

// Intelligent legend system
pub struct Legend {
    color_scale: Option<ColorScale>,
    size_scale: Option<SizeScale>,
    symbol_scale: Option<SymbolScale>,
    layout: LegendLayout,
}

impl Legend {
    pub fn auto_generate(chart: &Chart) -> Self {
        // Automatically generate legend based on chart encodings
        let mut legend = Legend::new();

        if let Some(color_encoding) = chart.color_encoding() {
            legend.add_color_scale(color_encoding.scale);
        }

        if let Some(size_encoding) = chart.size_encoding() {
            legend.add_size_scale(size_encoding.scale);
        }

        legend
    }
}
```

### Phase 3: Deliverables

- [ ] 1 billion points rendering at 30+ FPS
- [ ] GPU-accelerated force-directed layout for 100K+ nodes
- [ ] Geographic visualization with multiple projection support
- [ ] Network diagrams with automatic edge bundling
- [ ] Professional axes, legends, and annotations
- [ ] Real-world validation with scientific datasets

## Phase 4: Ecosystem Integration + Production Readiness - Version 0.4.0

### Phase 4: Goals

- Create integration packages for major Rust frameworks
- Implement export and serialization capabilities
- Add comprehensive testing and quality assurance
- Optimize for production deployment scenarios

### Framework Integrations

```rust
// Bevy game engine integration
pub mod bevy {
    use bevy::prelude::*;

    #[derive(Component)]
    pub struct GupChart {
        chart: Chart,
        auto_update: bool,
        data_source: Option<Entity>,
    }

    pub struct GupPlugin;

    impl Plugin for GupPlugin {
        fn build(&self, app: &mut App) {
            app.add_systems(Update, (
                gup_chart_update_system,
                gup_chart_render_system,
                gup_interaction_system,
            ));
        }
    }

    fn gup_chart_render_system(
        mut charts: Query<&mut GupChart>,
        mut gup_context: ResMut<GupContext>,
    ) {
        for mut chart in charts.iter_mut() {
            chart.render(&mut gup_context);
        }
    }
}

// egui integration
pub mod egui {
    use egui::*;

    pub struct GupWidget {
        chart: Chart,
        size: Vec2,
        interactive: bool,
    }

    impl Widget for GupWidget {
        fn ui(self, ui: &mut Ui) -> Response {
            let (rect, response) = ui.allocate_exact_size(
                self.size,
                Sense::click_and_drag()
            );

            // Render Gup chart into egui texture
            let texture = self.chart.render_to_texture(rect.size());
            ui.painter().image(texture.id(), rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);

            // Handle interactions
            if response.clicked() {
                let click_pos = response.interact_pointer_pos().unwrap();
                self.chart.handle_click(click_pos);
            }

            response
        }
    }
}

// Tauri desktop application integration
pub mod tauri {
    use tauri::prelude::*;

    #[tauri::command]
    async fn create_chart(data: Vec<serde_json::Value>) -> Result<String, String> {
        let chart = gup::plot()
            .data(data)
            .scatter(x("x"), y("y"))
            .build()
            .map_err(|e| e.to_string())?;

        let chart_id = CHART_MANAGER.register_chart(chart);
        Ok(chart_id)
    }

    #[tauri::command]
    async fn render_chart(chart_id: String, width: u32, height: u32) -> Result<Vec<u8>, String> {
        let chart = CHART_MANAGER.get_chart(&chart_id)
            .ok_or("Chart not found")?;

        let image_data = chart.render_to_png(width, height)
            .map_err(|e| e.to_string())?;

        Ok(image_data)
    }
}
```

### Export and Serialization

```rust
// High-quality export system
pub struct ExportEngine {
    // Vector format support
    svg_renderer: SvgRenderer,
    pdf_renderer: PdfRenderer,

    // Raster format support
    png_renderer: PngRenderer,
    webp_renderer: WebpRenderer,

    // Interactive formats
    html_renderer: HtmlRenderer,
    webgl_renderer: WebglRenderer,
}

impl ExportEngine {
    pub async fn export_svg(&self, chart: &Chart, options: SvgExportOptions) -> Vec<u8> {
        // Convert GPU rendering to vector paths
        let paths = self.extract_vector_paths(chart);
        let svg_doc = self.svg_renderer.create_svg_document(paths, options);
        svg_doc.to_bytes()
    }

    pub async fn export_interactive_html(&self,
        chart: &Chart,
        options: HtmlExportOptions
    ) -> String {
        // Generate standalone HTML with embedded WebGL viewer
        let webgl_code = self.webgl_renderer.generate_viewer_code(chart);
        let html_template = include_str!("templates/interactive_chart.html");

        html_template
            .replace("{{CHART_DATA}}", &chart.serialize_data())
            .replace("{{WEBGL_CODE}}", &webgl_code)
            .replace("{{CHART_CONFIG}}", &chart.serialize_config())
    }

    pub async fn export_animation(&self,
        chart: &Chart,
        keyframes: &[AnimationKeyframe],
        format: AnimationFormat
    ) -> Vec<u8> {
        match format {
            AnimationFormat::Gif => self.render_animated_gif(chart, keyframes).await,
            AnimationFormat::Mp4 => self.render_mp4_video(chart, keyframes).await,
            AnimationFormat::WebM => self.render_webm_video(chart, keyframes).await,
        }
    }
}
```

### Production Optimization and Testing

```rust
// Comprehensive testing framework
#[cfg(test)]
mod tests {
    use super::*;

    // Performance regression tests
    #[tokio::test]
    async fn test_million_point_performance() {
        let data = generate_test_data(1_000_000);
        let start = Instant::now();

        let chart = gup::plot()
            .data(data)
            .scatter(x("x"), y("y"))
            .render()
            .await
            .unwrap();

        let render_time = start.elapsed();
        assert!(render_time < Duration::from_millis(16)); // < 1 frame at 60 FPS
    }

    // Cross-platform compatibility tests
    #[test]
    fn test_cross_platform_consistency() {
        let data = load_test_data();

        let native_result = render_native(&data);
        let web_result = render_web(&data);
        let mobile_result = render_mobile(&data);

        assert_eq!(native_result.hash(), web_result.hash());
        assert_eq!(native_result.hash(), mobile_result.hash());
    }

    // Accessibility compliance tests
    #[test]
    fn test_accessibility_compliance() {
        let chart = create_test_chart();
        let accessibility_report = chart.audit_accessibility();

        assert!(accessibility_report.meets_wcag_aa());
        assert!(accessibility_report.supports_screen_readers());
        assert!(accessibility_report.supports_keyboard_navigation());
    }
}

// Production monitoring and telemetry
pub struct ProductionMonitor {
    performance_collector: PerformanceCollector,
    error_reporter: ErrorReporter,
    usage_analytics: UsageAnalytics,
}

impl ProductionMonitor {
    pub fn collect_performance_metrics(&mut self, chart: &Chart) {
        let metrics = PerformanceMetrics {
            render_time: self.performance_collector.last_render_time(),
            memory_usage: self.performance_collector.memory_usage(),
            gpu_utilization: self.performance_collector.gpu_utilization(),
            data_point_count: chart.data_point_count(),
        };

        self.performance_collector.record_metrics(metrics);
    }

    pub fn report_error(&self, error: &GupError) {
        self.error_reporter.report(ErrorReport {
            error_type: error.error_type(),
            stack_trace: error.stack_trace(),
            context: error.context(),
            timestamp: Utc::now(),
        });
    }
}
```

### Phase 4: Deliverables

- [ ] Production-ready integrations for Bevy, egui, Tauri, and winit
- [ ] High-quality export to SVG, PDF, PNG, HTML, and video formats
- [ ] Comprehensive test suite with performance regression detection
- [ ] Production monitoring and telemetry system
- [ ] Performance optimization for mobile and low-end devices
- [ ] Enterprise deployment documentation and support

## Phase 5: Advanced Features + Market Expansion - Version 0.5.0

### Phase 5: Goals

- Add 3D visualization capabilities
- Implement advanced statistical computing on GPU
- Create visual programming interface
- Expand to additional language bindings

### 3D Visualization Extension

```rust
// 3D visualization capabilities
pub struct Chart3D {
    camera: Camera3D,
    lighting: LightingSystem,
    renderer_3d: Renderer3D,
}

impl Chart3D {
    pub fn scatter_3d(&mut self) -> Scatter3DBuilder {
        Scatter3DBuilder::new(self)
    }

    pub fn surface_plot(&mut self) -> SurfacePlotBuilder {
        SurfacePlotBuilder::new(self)
    }

    pub fn volume_rendering(&mut self) -> VolumeRenderingBuilder {
        VolumeRenderingBuilder::new(self)
    }
}

// 3D-specific marks
#[derive(Mark3D)]
pub struct Sphere {
    #[attribute(location = 0)]
    position: [f32; 3],
    #[attribute(location = 1)]
    radius: f32,
    #[attribute(location = 2)]
    color: [f32; 4],
    #[attribute(location = 3)]
    metallic: f32,
    #[attribute(location = 4)]
    roughness: f32,
}

// Usage
let chart_3d = Chart3D::new(&context);
chart_3d.scatter_3d()
    .data(data_3d)
    .x(|d| d.x)
    .y(|d| d.y)
    .z(|d| d.z)
    .size(|d| d.value)
    .color(|d| d.category)
    .render()?;
```

### Advanced Statistical Computing

```rust
// GPU-accelerated statistical analysis
pub struct StatisticalEngine {
    // Machine learning algorithms
    clustering_algorithms: ClusteringAlgorithms,
    regression_algorithms: RegressionAlgorithms,

    // Statistical tests
    hypothesis_testing: HypothesisTestingEngine,

    // Time series analysis
    time_series_analyzer: TimeSeriesAnalyzer,
}

impl StatisticalEngine {
    pub async fn principal_component_analysis(&self,
        data: &[Vec<f32>]
    ) -> PcaResult {
        // GPU-accelerated PCA computation
        let covariance_matrix = self.compute_covariance_matrix_gpu(data).await;
        let eigenvalues = self.compute_eigenvalues_gpu(&covariance_matrix).await;
        let eigenvectors = self.compute_eigenvectors_gpu(&covariance_matrix).await;

        PcaResult {
            eigenvalues,
            eigenvectors,
            explained_variance_ratio: self.calculate_explained_variance(&eigenvalues),
        }
    }

    pub async fn anomaly_detection(&self,
        data: &[DataPoint],
        algorithm: AnomalyDetectionAlgorithm
    ) -> AnomalyDetectionResult {
        match algorithm {
            AnomalyDetectionAlgorithm::IsolationForest => {
                self.isolation_forest_gpu(data).await
            }
            AnomalyDetectionAlgorithm::OneClassSvm => {
                self.one_class_svm_gpu(data).await
            }
            AnomalyDetectionAlgorithm::LocalOutlierFactor => {
                self.lof_gpu(data).await
            }
        }
    }
}
```

### Visual Programming Interface

```rust
// Visual node-based chart construction
pub struct VisualProgrammingEngine {
    node_graph: NodeGraph,
    execution_engine: ExecutionEngine,
    ui_renderer: NodeEditorRenderer,
}

pub enum NodeType {
    // Data nodes
    DataSource(DataSourceNode),
    Filter(FilterNode),
    Transform(TransformNode),
    Aggregate(AggregateNode),

    // Visualization nodes
    ScatterPlot(ScatterPlotNode),
    LinePlot(LinePlotNode),
    Histogram(HistogramNode),

    // Scale nodes
    LinearScale(LinearScaleNode),
    LogScale(LogScaleNode),
    ColorScale(ColorScaleNode),

    // Layout nodes
    ForceDirected(ForceDirectedNode),
    Treemap(TreemapNode),
}

impl VisualProgrammingEngine {
    pub fn create_node(&mut self, node_type: NodeType) -> NodeId {
        let node = Node::new(node_type);
        self.node_graph.add_node(node)
    }

    pub fn connect_nodes(&mut self,
        output_node: NodeId,
        output_port: &str,
        input_node: NodeId,
        input_port: &str
    ) -> Result<(), ConnectionError> {
        // Type-checked connections between nodes
        let output_type = self.node_graph.get_output_type(output_node, output_port)?;
        let input_type = self.node_graph.get_input_type(input_node, input_port)?;

        if !output_type.is_compatible_with(&input_type) {
            return Err(ConnectionError::IncompatibleTypes {
                output_type,
                input_type,
            });
        }

        self.node_graph.connect(output_node, output_port, input_node, input_port)
    }

    pub async fn execute_graph(&mut self) -> Result<Chart, ExecutionError> {
        // Execute node graph to generate final visualization
        let execution_plan = self.execution_engine.create_execution_plan(&self.node_graph)?;

        for stage in execution_plan.stages {
            self.execute_stage(stage).await?;
        }

        // Extract final chart from output node
        let output_node = self.node_graph.find_output_node()?;
        output_node.get_chart()
    }
}
```

### Language Bindings and Web Platform

```rust
// Python bindings
#[pymodule]
fn gup_python(_py: Python, m: &PyModule) -> PyResult<()> {
    #[pyfn(m)]
    fn scatter_plot(data: Vec<PyDict>) -> PyResult<Chart> {
        let rust_data: Vec<DataPoint> = data.into_iter()
            .map(|dict| DataPoint::from_py_dict(dict))
            .collect::<Result<Vec<_>, _>>()?;

        let chart = gup::plot()
            .data(rust_data)
            .scatter(x("x"), y("y"))
            .build()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;

        Ok(chart)
    }

    Ok(())
}

// JavaScript bindings (via WASM)
#[wasm_bindgen]
pub struct WasmChart {
    inner: Chart,
}

#[wasm_bindgen]
impl WasmChart {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            inner: Chart::new(),
        }
    }

    #[wasm_bindgen]
    pub fn scatter_plot(&mut self, data: JsValue) -> Result<(), JsValue> {
        let data: Vec<DataPoint> = data.into_serde()
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        self.inner = gup::plot()
            .data(data)
            .scatter(x("x"), y("y"))
            .build()
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        Ok(())
    }

    #[wasm_bindgen]
    pub fn render_to_canvas(&self, canvas_id: &str) -> Result<(), JsValue> {
        let canvas = web_sys::window()
            .unwrap()
            .document()
            .unwrap()
            .get_element_by_id(canvas_id)
            .unwrap()
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .unwrap();

        self.inner.render_to_canvas(&canvas)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }
}
```

### Phase 5: Deliverables

- [ ] 3D visualization with volume rendering and surface plots
- [ ] GPU-accelerated machine learning integration (PCA, clustering, regression)
- [ ] Visual programming interface for non-programmers
- [ ] Python and JavaScript language bindings
- [ ] Web-based chart editor and sharing platform
- [ ] Advanced export capabilities (interactive 3D, VR/AR formats)

## Success Metrics and Validation

### Technical Performance Targets

- **Billion Points**: 1B points at 30+ FPS by Phase 3
- **Real-Time Latency**: <1ms data update latency by Phase 2
- **Memory Efficiency**: <5GB memory for 1B points by Phase 3
- **Cross-Platform**: Identical performance within 10% across platforms
- **Accessibility**: 100% WCAG 2.1 AA compliance from Phase 1

### Market Adoption Targets

- **Phase 1**: 10+ external validation projects
- **Phase 2**: 50+ GitHub stars, 5+ blog posts/articles
- **Phase 3**: 500+ GitHub stars, 10+ production deployments
- **Phase 4**: 2000+ GitHub stars, conference presentations
- **Phase 5**: 5000+ GitHub stars, 100+ production deployments

### Ecosystem Health Metrics

- **Documentation**: >90% API coverage with examples
- **Performance**: Automated benchmarks prevent regressions
- **Community**: Active Discord/forum with <24h response time
- **Quality**: <1% bug rate in releases, comprehensive testing

This refined roadmap provides a concrete path to building Gup into the
definitive GPU-accelerated data visualization library, with clear milestones,
success metrics, and validation checkpoints throughout the development process.

# GUP-018: Observable Plot-Style Chart Builders

## Story Overview

**Title**: Implement Observable Plot-Style Chart Builder APIs  
**Epic**: Phase 2 Initiative 1 - Observable Plot-Style Chart Builders  
**Priority**: Critical  
**Story Points**: 21  

## Context

This is Gup's primary developer-facing API that must achieve Observable Plot's legendary simplicity while maintaining GPU performance. The chart builders provide one-line chart creation for common visualization types, seamless interoperability with low-level APIs, and serve as the main validation that Phase 1's foundation is powerful enough to support high-level convenience APIs.

## User Story

**As a** data analyst or developer  
**I want** Observable Plot-style one-line chart creation with GPU performance  
**So that** I can quickly create professional visualizations without sacrificing performance for large datasets  

## Acceptance Criteria

### Core Chart Builder Features

- [ ] **One-Line Creation**: Common charts created with single fluent API call
- [ ] **Observable Plot Parity**: Feature-complete equivalent for most common Plot use cases
- [ ] **GPU Performance**: Maintain 100K+ point rendering at 60 FPS
- [ ] **Seamless Interoperability**: Drop down to low-level APIs when needed

### Chart Types

```rust
// Scatter plots
gup::plot()
    .data(sales_data)
    .scatter(x("revenue"), y("profit"))
    .color("region")
    .size("employees")
    .render()?;

// Line charts  
gup::plot()
    .data(time_series)
    .line(x("date"), y("value"))
    .color("series")
    .render()?;

// Bar charts
gup::plot()
    .data(categories)
    .bar(x("category"), y("count"))
    .fill("category")
    .render()?;

// Area charts
gup::plot()
    .data(time_data)
    .area(x("date"), y("value"))
    .fill("steelblue")
    .render()?;

// Heatmaps
gup::plot()
    .data(matrix_data)
    .heatmap(x("col"), y("row"), fill("value"))
    .render()?;
```

### API Design Requirements

- [ ] **Fluent Interface**: Natural method chaining that reads like English
- [ ] **Type Safety**: Compile-time validation of data mappings and attribute compatibility
- [ ] **Performance**: Zero overhead abstractions over Phase 1 primitives
- [ ] **Extensibility**: Easy to add new chart types and customizations

## Technical Tasks

### 1. Core Chart Builder Infrastructure
- [ ] Design base `ChartBuilder` trait and implementation
- [ ] Create fluent API framework with method chaining
- [ ] Implement data binding system for automatic type inference
- [ ] Add chart configuration and option management

### 2. Individual Chart Type Implementations  
- [ ] Implement `ScatterPlotBuilder` with Observable Plot compatibility
- [ ] Create `LineChartBuilder` with time series optimization
- [ ] Build `BarChartBuilder` with categorical data handling
- [ ] Develop `AreaChartBuilder` with stacking support
- [ ] Implement `HeatmapBuilder` with 2D data mapping

### 3. Data Mapping and Scales Integration
- [ ] Create automatic scale inference from data types
- [ ] Implement accessor function system for data field mapping
- [ ] Add automatic domain/range calculation
- [ ] Integrate with Phase 1 shader function system

### 4. Builder-to-Selection Bridge
- [ ] Create seamless conversion from builders to low-level selections
- [ ] Implement builder state preservation during conversion
- [ ] Add incremental compilation for performance
- [ ] Support mixed high/low-level API usage

## Detailed Requirements

### Core Builder Infrastructure

```rust
pub trait ChartBuilder<T> {
    type Output: Renderable;
    
    fn data(self, data: Vec<T>) -> BoundChartBuilder<Self, T> where Self: Sized;
    fn build(self) -> Result<Self::Output, ChartError>;
    fn render(self) -> Result<RenderedChart, ChartError> where Self: Sized {
        self.build()?.render()
    }
}

pub struct BoundChartBuilder<B: ChartBuilder<T>, T> {
    builder: B,
    data: Vec<T>,
    context: Arc<GupContext>,
    _phantom: PhantomData<T>,
}

impl<B: ChartBuilder<T>, T> BoundChartBuilder<B, T> {
    pub fn build(self) -> Result<B::Output, ChartError> {
        self.builder.build_with_data(self.data, self.context)
    }
    
    // Allow dropping to low-level API
    pub fn into_selection<M: Mark>(self) -> Selection<T, M> {
        let selection = Selection::new(self.data, self.context);
        // Apply builder configuration to selection
        self.builder.configure_selection(selection)
    }
}
```

### Scatter Plot Builder

```rust
pub struct ScatterPlotBuilder {
    x_accessor: Option<AccessorFunction>,
    y_accessor: Option<AccessorFunction>, 
    color_accessor: Option<AccessorFunction>,
    size_accessor: Option<AccessorFunction>,
    
    // Chart configuration
    title: Option<String>,
    width: f32,
    height: f32,
    margins: Margins,
    
    // Styling
    opacity: f32,
    stroke_width: f32,
    
    // Performance optimizations
    point_budget: Option<usize>, // For very large datasets
}

impl<T> ChartBuilder<T> for ScatterPlotBuilder {
    type Output = ScatterPlot<T>;
    
    fn build_with_data(self, data: Vec<T>, context: Arc<GupContext>) -> Result<Self::Output, ChartError> {
        // Create underlying selection
        let mut selection = Selection::<T, Circle>::new(data, context);
        
        // Apply accessors as shader functions
        if let Some(x_fn) = self.x_accessor {
            let x_shader = AccessorShaderFunction::new(x_fn);
            selection.attr("x", x_shader);
        }
        
        if let Some(y_fn) = self.y_accessor {
            let y_shader = AccessorShaderFunction::new(y_fn);
            selection.attr("y", y_shader);
        }
        
        if let Some(color_fn) = self.color_accessor {
            let color_shader = AccessorShaderFunction::new(color_fn);
            selection.attr("fill", color_shader);
        }
        
        if let Some(size_fn) = self.size_accessor {
            let size_shader = AccessorShaderFunction::new(size_fn);
            selection.attr("r", size_shader);
        }
        
        // Create chart with automatic scales and axes
        let x_scale = self.create_x_scale(&selection)?;
        let y_scale = self.create_y_scale(&selection)?;
        
        Ok(ScatterPlot {
            selection,
            x_scale,
            y_scale,
            configuration: self.into_config(),
        })
    }
}

impl ScatterPlotBuilder {
    pub fn new() -> Self {
        Self {
            x_accessor: None,
            y_accessor: None,
            color_accessor: None,
            size_accessor: None,
            title: None,
            width: 800.0,
            height: 600.0,
            margins: Margins::default(),
            opacity: 1.0,
            stroke_width: 0.0,
            point_budget: None,
        }
    }
    
    pub fn x<F>(mut self, accessor: F) -> Self 
    where F: Fn(&T) -> f32 + Send + Sync + 'static
    {
        self.x_accessor = Some(AccessorFunction::new(accessor));
        self
    }
    
    pub fn y<F>(mut self, accessor: F) -> Self
    where F: Fn(&T) -> f32 + Send + Sync + 'static
    {
        self.y_accessor = Some(AccessorFunction::new(accessor));
        self
    }
    
    pub fn color<F>(mut self, accessor: F) -> Self
    where F: Fn(&T) -> Color + Send + Sync + 'static
    {
        self.color_accessor = Some(AccessorFunction::new(accessor));
        self
    }
    
    pub fn size<F>(mut self, accessor: F) -> Self
    where F: Fn(&T) -> f32 + Send + Sync + 'static
    {
        self.size_accessor = Some(AccessorFunction::new(accessor));
        self
    }
    
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }
    
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }
    
    pub fn height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }
}
```

### Accessor Function System

```rust
pub struct AccessorFunction {
    function: Box<dyn Fn(&Any) -> AccessorValue + Send + Sync>,
    output_type: AccessorType,
}

#[derive(Debug, Clone)]
pub enum AccessorValue {
    Float(f32),
    Color(Color),
    String(String),
    Date(chrono::DateTime<chrono::Utc>),
}

#[derive(Debug, Clone)]
pub enum AccessorType {
    Numeric,
    Color,
    Categorical,
    Temporal,
}

impl AccessorFunction {
    pub fn new<T, V>(f: impl Fn(&T) -> V + Send + Sync + 'static) -> Self 
    where 
        T: 'static,
        V: Into<AccessorValue> + 'static
    {
        let function = Box::new(move |data: &Any| {
            if let Some(typed_data) = data.downcast_ref::<T>() {
                f(typed_data).into()
            } else {
                panic!("Type mismatch in accessor function")
            }
        });
        
        Self {
            function,
            output_type: V::accessor_type(),
        }
    }
    
    pub fn to_shader_function<T>(self) -> AccessorShaderFunction<T> {
        AccessorShaderFunction::new(self)
    }
}

// Bridge accessor functions to shader functions
pub struct AccessorShaderFunction<T> {
    accessor: AccessorFunction,
    _phantom: PhantomData<T>,
}

impl<T> ShaderFunction for AccessorShaderFunction<T> 
where T: ShaderType + 'static
{
    type Input = T;
    type Output = f32; // Simplified for demonstration
    type Uniforms = ();
    
    fn wgsl_function() -> &'static str {
        // Generate WGSL based on accessor type
        r#"
        fn accessor_function(data: T) -> f32 {
            // Generated code based on accessor field access
            return data.field_name;
        }
        "#
    }
    
    fn function_name() -> &'static str {
        "accessor_function"
    }
}
```

### High-Level Chart API

```rust
// Top-level convenience functions
pub fn plot() -> PlotBuilder {
    PlotBuilder::new()
}

pub fn scatter<T>() -> ScatterPlotBuilder {
    ScatterPlotBuilder::new()
}

pub fn line<T>() -> LineChartBuilder {
    LineChartBuilder::new()
}

pub fn bar<T>() -> BarChartBuilder {
    BarChartBuilder::new()
}

pub struct PlotBuilder {
    context: Option<Arc<GupContext>>,
}

impl PlotBuilder {
    pub fn new() -> Self {
        Self { context: None }
    }
    
    pub fn with_context(mut self, context: Arc<GupContext>) -> Self {
        self.context = Some(context);
        self
    }
    
    pub fn data<T>(self, data: Vec<T>) -> BoundPlotBuilder<T> {
        let context = self.context.unwrap_or_else(|| {
            Arc::new(GupContext::new().expect("Failed to create context"))
        });
        
        BoundPlotBuilder {
            data,
            context,
        }
    }
}

pub struct BoundPlotBuilder<T> {
    data: Vec<T>,
    context: Arc<GupContext>,
}

impl<T> BoundPlotBuilder<T> {
    pub fn scatter(self, x: impl Accessor<T>, y: impl Accessor<T>) -> ConfiguredScatterPlot<T> {
        let mut builder = ScatterPlotBuilder::new();
        builder = builder.x(x.into_fn()).y(y.into_fn());
        
        ConfiguredScatterPlot {
            data: self.data,
            context: self.context,
            builder,
        }
    }
    
    pub fn line(self, x: impl Accessor<T>, y: impl Accessor<T>) -> ConfiguredLineChart<T> {
        let mut builder = LineChartBuilder::new();
        builder = builder.x(x.into_fn()).y(y.into_fn());
        
        ConfiguredLineChart {
            data: self.data,
            context: self.context,
            builder,
        }
    }
    
    pub fn bar(self, x: impl Accessor<T>, y: impl Accessor<T>) -> ConfiguredBarChart<T> {
        let mut builder = BarChartBuilder::new();
        builder = builder.x(x.into_fn()).y(y.into_fn());
        
        ConfiguredBarChart {
            data: self.data,
            context: self.context, 
            builder,
        }
    }
}
```

### Observable Plot Compatibility Layer

```rust
// Observable Plot-style accessor syntax
pub fn x(field: &str) -> FieldAccessor {
    FieldAccessor::new(field)
}

pub fn y(field: &str) -> FieldAccessor {
    FieldAccessor::new(field)
}

pub fn color(field: &str) -> FieldAccessor {
    FieldAccessor::new(field)
}

pub fn size(field: &str) -> FieldAccessor {
    FieldAccessor::new(field)
}

pub struct FieldAccessor {
    field_name: String,
}

impl FieldAccessor {
    pub fn new(field: &str) -> Self {
        Self {
            field_name: field.to_string(),
        }
    }
}

// Macro for Observable Plot-style data field access
#[macro_export]
macro_rules! plot {
    ($chart_type:ident, data: $data:expr, x: $x:expr, y: $y:expr $(, $attr:ident: $value:expr)*) => {
        {
            let mut builder = $chart_type::new();
            builder = builder.data($data).x($x).y($y);
            $(
                builder = builder.$attr($value);
            )*
            builder.render()
        }
    };
}

// Usage: plot!(scatter, data: sales_data, x: "revenue", y: "profit", color: "region")
```

## Dependencies

### Prerequisite Stories
- GUP-001: Build Mixable Trait (composability foundation)
- GUP-002: Core Selection Type (underlying selection system)
- GUP-005: Shader Function Trait (data transformation system)
- GUP-009: Core Mark Trait (visual primitives)
- GUP-010: Basic Mark Implementations (Circle, Rectangle, Line)

### Enables Stories
- GUP-019: Automatic Scale and Axis System (scales integration)
- GUP-020: Color Systems and Themes (color integration)
- All subsequent Phase 2 stories

## Testing Strategy

### API Usability Tests
```rust
#[test]
fn test_observable_plot_compatibility() {
    let sales_data = load_test_sales_data();
    
    // Test Observable Plot-style API
    let chart = gup::plot()
        .data(sales_data)
        .scatter(x("revenue"), y("profit"))
        .color("region")
        .size("employees")
        .render()
        .expect("Chart creation should succeed");
    
    assert!(chart.is_valid());
    assert_eq!(chart.mark_count(), sales_data.len());
}

#[test]
fn test_fluent_api_chaining() {
    let data = create_test_data();
    
    let chart = scatter()
        .data(data.clone())
        .x(|d| d.x)
        .y(|d| d.y)
        .color(|d| if d.category == "A" { Color::RED } else { Color::BLUE })
        .size(|d| d.value * 10.0)
        .title("Test Scatter Plot")
        .width(800.0)
        .height(600.0)
        .render()
        .expect("Fluent API should work");
    
    assert_eq!(chart.title(), Some("Test Scatter Plot"));
    assert_eq!(chart.width(), 800.0);
    assert_eq!(chart.height(), 600.0);
}

#[test]
fn test_type_safety() {
    let numeric_data = vec![NumericData { x: 1.0, y: 2.0 }];
    
    // This should compile
    let valid_chart = scatter()
        .data(numeric_data.clone())
        .x(|d| d.x)  // f32 -> f32 is valid
        .y(|d| d.y)  // f32 -> f32 is valid
        .render();
    
    assert!(valid_chart.is_ok());
    
    // This should fail at compile time:
    // let invalid_chart = scatter()
    //     .data(numeric_data)
    //     .x(|d| d.name)  // String -> f32 is invalid
    //     .render();
}
```

### Performance Tests
```rust
#[test]
fn test_large_dataset_performance() {
    let large_dataset = create_test_data(100_000);
    
    let start = std::time::Instant::now();
    
    let chart = scatter()
        .data(large_dataset)
        .x(|d| d.x)
        .y(|d| d.y)
        .color(|d| d.category_color())
        .render()
        .expect("Large dataset should render successfully");
    
    let creation_time = start.elapsed();
    
    // Chart creation should be fast
    assert!(creation_time < std::time::Duration::from_millis(100));
    
    // Rendering should be fast
    let render_start = std::time::Instant::now();
    chart.render_frame().expect("Rendering should succeed");
    let render_time = render_start.elapsed();
    
    // Should render 100K points in <16ms (60 FPS)
    assert!(render_time < std::time::Duration::from_millis(16));
}

#[test]
fn test_memory_efficiency() {
    let data = create_test_data(10_000);
    let data_size = std::mem::size_of_val(&data[0]) * data.len();
    
    let chart = scatter()
        .data(data)
        .x(|d| d.x)
        .y(|d| d.y)
        .render()
        .expect("Chart creation should succeed");
    
    let chart_memory = chart.memory_usage();
    let overhead = (chart_memory as f32 - data_size as f32) / data_size as f32;
    
    // Overhead should be less than 50%
    assert!(overhead < 0.5, "Memory overhead too high: {:.1}%", overhead * 100.0);
}
```

### Builder Integration Tests
```rust
#[test]
fn test_builder_to_selection_conversion() {
    let data = create_test_data();
    
    let builder = scatter()
        .data(data.clone())
        .x(|d| d.x)
        .y(|d| d.y)
        .color(|d| d.color);
    
    // Convert to low-level selection
    let mut selection: Selection<TestData, Circle> = builder.into_selection();
    
    // Should be able to use low-level API
    selection.on("click", |event, data| {
        println!("Clicked: {:?}", data);
    });
    
    let rendered = selection.render().expect("Selection should render");
    assert!(rendered.is_valid());
}

#[test]
fn test_mixed_api_usage() {
    let data = create_test_data();
    
    // Start with high-level API
    let mut chart = gup::plot()
        .data(data)
        .scatter(x("x"), y("y"))
        .build()
        .expect("Chart creation should succeed");
    
    // Access underlying selection for custom behavior
    chart.select_all::<Circle>()
        .on("hover", |event, data| {
            // Custom hover behavior
        })
        .transition()
        .duration(300)
        .attr("stroke_width", 2.0);
    
    let rendered = chart.render().expect("Mixed API usage should work");
    assert!(rendered.is_valid());
}
```

### Observable Plot Migration Tests
```rust
#[test]
fn test_observable_plot_migration_examples() {
    // Test common Observable Plot patterns
    
    // Observable Plot: Plot.dot(data, {x: "x", y: "y", fill: "category"})
    let plot_equivalent = gup::plot()
        .data(test_data())
        .scatter(x("x"), y("y"))
        .color("category")
        .render()
        .expect("Observable Plot equivalent should work");
    
    // Observable Plot: Plot.line(data, {x: "date", y: "value", stroke: "series"})
    let line_equivalent = gup::plot()
        .data(time_series_data())
        .line(x("date"), y("value"))
        .color("series")
        .render()
        .expect("Line chart equivalent should work");
    
    // Observable Plot: Plot.barY(data, {x: "category", y: "count"})
    let bar_equivalent = gup::plot()
        .data(categorical_data())
        .bar(x("category"), y("count"))
        .render()
        .expect("Bar chart equivalent should work");
    
    assert!(plot_equivalent.is_valid());
    assert!(line_equivalent.is_valid());
    assert!(bar_equivalent.is_valid());
}
```

## Success Metrics

### API Usability Requirements
- [ ] **One-Line Creation**: 90% of common charts creatable with single fluent call
- [ ] **Observable Plot Parity**: Support for 80% of Observable Plot's most common use cases
- [ ] **Type Safety**: 100% of type mismatches caught at compile time
- [ ] **IDE Support**: Full autocomplete and documentation in popular IDEs

### Performance Requirements
- [ ] **Large Dataset Performance**: 100K points render in <16ms (60 FPS)
- [ ] **Creation Speed**: Chart builders create charts in <10ms for typical datasets
- [ ] **Memory Efficiency**: <20% memory overhead vs direct Selection usage
- [ ] **Compilation Time**: Builder APIs add <500ms to compile time

### Developer Experience Requirements
- [ ] **Learning Curve**: Developers familiar with Observable Plot can use API immediately
- [ ] **Error Messages**: Clear, actionable errors with suggestions for common mistakes
- [ ] **Documentation**: Complete examples for every chart type and configuration option
- [ ] **Migration Guide**: Step-by-step guide for converting Observable Plot code

## Risk Assessment

### Technical Risks
- **High**: Builder complexity could impact compile times and error message quality
- **Medium**: Performance overhead from high-level abstractions
- **Medium**: Type inference complexity could make error messages confusing

### Mitigation Strategies
- **Incremental Implementation**: Start with simple builders, add complexity gradually
- **Performance Monitoring**: Continuous benchmarking of builder overhead
- **User Testing**: Regular feedback from developers migrating from Observable Plot

## Implementation Notes

### Design Decisions
- Use zero-cost abstractions where possible to maintain GPU performance
- Implement builders as thin layers over Phase 1 Selection system
- Support both closure-based and string-based field accessors for flexibility
- Prioritize Observable Plot compatibility over novel API design

### Performance Strategy
- Lazy evaluation of builder configuration until render time
- Direct compilation to Phase 1 primitives without intermediate representations
- Automatic optimization of common patterns (e.g., identical accessor functions)
- Streaming-aware builders for real-time data updates

### Observable Plot Compatibility Strategy
- Maintain 1:1 mapping for common Observable Plot chart types
- Use similar method names and parameter patterns
- Support Observable Plot's data accessor patterns
- Provide migration utilities for automatic code conversion

## Definition of Done

- [ ] All major chart types (scatter, line, bar, area, heatmap) implemented
- [ ] Observable Plot compatibility validated with real migration examples
- [ ] Performance targets met for 100K+ point datasets
- [ ] Type safety working with clear error messages
- [ ] Seamless interoperability with low-level Selection API
- [ ] Fluent API supporting method chaining and builder patterns
- [ ] Cross-platform compatibility verified
- [ ] Comprehensive documentation with migration guides
- [ ] Integration tests passing with Phase 1 components
- [ ] User acceptance testing completed with Observable Plot users
- [ ] Code review completed and approved
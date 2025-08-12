# GUP-093: Scale-Axis Integration System

## Story Overview

**Epic**: Phase 2 - High-Level Convenience APIs  
**Theme**: Automatic Scale and Axis System  
**Priority**: Medium  
**Story Points**: 6  
**Status**: 📋 Planned

## Problem Statement

The axis rendering system (GUP-089), tick generation (GUP-090), grid lines
(GUP-091), and labels (GUP-092) exist as separate components but need seamless
integration with the scale system to provide automatic, professional axis
generation. Chart builders currently have placeholder axis functionality that
needs to automatically detect appropriate scales, generate ticks, render grids,
and format labels based on data characteristics. This integration story
coordinates all axis components into a unified, easy-to-use system.

## Business Context

Users expect charts to "just work" with professional-looking axes without manual
configuration. The integration system must automatically detect scale types from
data (numeric, temporal, categorical), apply appropriate formatting, and
coordinate all axis components. This is the capstone story that delivers the
complete "automatic scale and axis system" promised in Phase 2 planning.

## Acceptance Criteria

### Automatic Scale Detection

- [ ] **Data type analysis** automatically selects appropriate scale types
      (linear, log, time, ordinal)
- [ ] **Range calculation** determines optimal domain/range from data
      characteristics
- [ ] **Scale configuration** applies sensible defaults while allowing
      customization
- [ ] **Multi-scale support** handles different scales on different axes (e.g.,
      time x-axis, linear y-axis)
- [ ] **Dynamic adaptation** updates scales when data changes without breaking
      layout

### Coordinated Axis Generation

- [ ] **Automatic tick generation** using detected scales and viewport
      constraints
- [ ] **Synchronized grid rendering** with tick positions and scale mappings
- [ ] **Formatted label display** using scale-appropriate formatters (dates for
      time scales, etc.)
- [ ] **Layout coordination** calculating margins needed for labels and
      configuring chart layout
- [ ] **Performance optimization** rendering complete axis system efficiently

### Chart Builder Integration

- [ ] **Zero-configuration default** - axes appear automatically with sensible
      defaults
- [ ] **Progressive customization** - users can override any aspect of automatic
      axis generation
- [ ] **Type safety** - scale/data type mismatches caught at compile time where
      possible
- [ ] **Fluent API consistency** - axis configuration feels natural within chart
      builder patterns
- [ ] **Error handling** - clear messages when automatic detection fails or
      configuration is invalid

### Scale System Coordination

- [ ] **Bidirectional integration** - scales provide tick positions, axes
      provide layout constraints
- [ ] **Domain/range management** - automatic domain calculation with manual
      override capability
- [ ] **Transform composition** - scales work correctly with shader function
      pipeline
- [ ] **Update coordination** - scale changes properly propagate through entire
      axis system
- [ ] **Memory efficiency** - shared resources between scales and axis
      components

## Technical Requirements

### Integrated Axis System Architecture

```rust
pub struct AxisSystem {
    /// Coordinate scale systems
    scales: HashMap<AxisId, Box<dyn Scale>>,
    /// Axis rendering components
    axes: HashMap<AxisId, Box<dyn Axis>>,
    /// Label formatting system
    formatters: HashMap<AxisId, Box<dyn LabelFormatter>>,
    /// Layout coordinator
    layout: AxisLayoutManager,
    /// Performance coordinator
    performance: AxisPerformanceManager,
}

#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
pub enum AxisId {
    XAxis, YAxis, ColorAxis, SizeAxis
}

impl AxisSystem {
    /// Automatically configure axes based on data analysis
    pub fn auto_configure<T>(
        &mut self,
        data: &[T],
        mappings: &AxisMappings<T>
    ) -> GupResult<AxisConfiguration> {
        // 1. Analyze data to determine appropriate scale types
        let scale_specs = self.analyze_data_for_scales(data, mappings)?;

        // 2. Create scales with optimal domains/ranges
        for (axis_id, spec) in scale_specs {
            let scale = self.create_scale_from_spec(spec)?;
            self.scales.insert(axis_id, scale);
        }

        // 3. Configure tick generation for each axis
        self.configure_tick_generation()?;

        // 4. Set up label formatting appropriate for each scale type
        self.configure_label_formatting()?;

        // 5. Calculate layout requirements and coordinate positioning
        let layout = self.calculate_layout()?;

        Ok(AxisConfiguration {
            layout,
            scales: self.scales.clone(),
            // ... other config
        })
    }

    /// Render complete integrated axis system
    pub fn render_complete_axis_system(
        &mut self,
        context: &mut RenderContext,
        chart_bounds: ChartBounds,
        config: &AxisConfiguration,
    ) -> GupResult<()> {
        // 1. Generate tick positions from scales
        let tick_positions = self.generate_all_tick_positions()?;

        // 2. Render grid lines first (behind everything)
        if config.show_grid {
            self.render_coordinated_grid(context, &tick_positions, chart_bounds)?;
        }

        // 3. Render axis lines and tick marks
        self.render_axis_structures(context, &tick_positions, chart_bounds)?;

        // 4. Render formatted labels
        self.render_formatted_labels(context, &tick_positions, chart_bounds)?;

        Ok(())
    }
}

/// Data analysis for automatic scale detection
pub struct DataAnalyzer;

impl DataAnalyzer {
    pub fn analyze_field<T, F>(&self, data: &[T], accessor: F) -> DataCharacteristics
    where F: Fn(&T) -> DataValue
    {
        let mut characteristics = DataCharacteristics::new();

        for item in data {
            match accessor(item) {
                DataValue::Numeric(value) => {
                    characteristics.add_numeric_value(value);
                },
                DataValue::Temporal(timestamp) => {
                    characteristics.add_temporal_value(timestamp);
                },
                DataValue::Categorical(category) => {
                    characteristics.add_categorical_value(category);
                },
            }
        }

        characteristics.finalize()
    }
}

#[derive(Debug)]
pub struct DataCharacteristics {
    pub data_type: DataType,
    pub min_value: Option<f64>,
    pub max_value: Option<f64>,
    pub distribution: Distribution,
    pub temporal_range: Option<TemporalRange>,
    pub categories: Option<Vec<String>>,
    pub recommended_scale: ScaleType,
}

#[derive(Debug, Clone)]
pub enum ScaleType {
    Linear { nice_domain: bool },
    Logarithmic { base: f64 },
    Temporal { unit: TimeUnit },
    Ordinal { categories: Vec<String> },
    Band { categories: Vec<String>, padding: f32 },
}
```

### Enhanced Chart Builder Implementation

```rust
// Enhanced chart builder with integrated axis system
impl<T> ScatterPlotBuilder<T> {
    pub fn build_with_integrated_axes(
        self,
        data: Vec<T>,
        context: Arc<GupContext>
    ) -> GupResult<IntegratedChart<T>> {
        // 1. Create Selection as before
        let mut selection = Selection::<T, Circle>::new(data.clone(), context.clone())?;

        // 2. Analyze data and auto-configure axes
        let mut axis_system = AxisSystem::new();
        let axis_config = axis_system.auto_configure(&data, &self.mappings)?;

        // 3. Apply scales to selection attributes
        if let Some(x_scale) = axis_config.scales.get(&AxisId::XAxis) {
            selection.attr("x", x_scale.as_shader_function());
        }
        if let Some(y_scale) = axis_config.scales.get(&AxisId::YAxis) {
            selection.attr("y", y_scale.as_shader_function());
        }

        // 4. Create integrated chart with axis system
        Ok(IntegratedChart {
            selection,
            axis_system,
            axis_config,
            chart_config: self.config,
        })
    }
}

pub struct IntegratedChart<T> {
    selection: Selection<T, Circle>,
    axis_system: AxisSystem,
    axis_config: AxisConfiguration,
    chart_config: ChartConfig,
}

impl<T> Mixable for IntegratedChart<T> {
    type Output = Self;

    fn mix<Other: Mixable>(self, other: Other) -> ComposedVisualization<Self, Other> {
        ComposedVisualization::new(self, other)
    }

    fn render(&self, context: &mut RenderContext) -> GupResult<()> {
        // Calculate layout with axis margins
        let chart_bounds = self.calculate_chart_bounds_with_axes(context.viewport())?;

        // Render axes first
        self.axis_system.render_complete_axis_system(
            context,
            chart_bounds,
            &self.axis_config
        )?;

        // Render data visualization with adjusted bounds
        self.selection.render_in_bounds(context, chart_bounds.data_area)?;

        Ok(())
    }
}
```

### Scale Integration Interface

```rust
pub trait Scale: Send + Sync + 'static {
    /// Map data value to coordinate space (0.0 to 1.0)
    fn scale_value(&self, value: f64) -> f64;

    /// Inverse mapping from coordinate to data value
    fn invert_value(&self, coordinate: f64) -> f64;

    /// Get domain (input range) of this scale
    fn domain(&self) -> (f64, f64);

    /// Set domain, returns new scale instance
    fn with_domain(self, domain: (f64, f64)) -> Self where Self: Sized;

    /// Get range (output range) of this scale
    fn range(&self) -> (f32, f32);

    /// Set range, returns new scale instance
    fn with_range(self, range: (f32, f32)) -> Self where Self: Sized;

    /// Convert to shader function for GPU processing
    fn as_shader_function(&self) -> Box<dyn ShaderFunction>;

    /// Generate appropriate tick positions for this scale
    fn generate_ticks(&self, target_count: Option<usize>) -> Vec<f64>;

    /// Get appropriate label formatter for this scale type
    fn default_formatter(&self) -> Box<dyn LabelFormatter>;
}

// Implementations for different scale types
pub struct LinearScale {
    domain: (f64, f64),
    range: (f32, f32),
    nice: bool,
}

pub struct LogarithmicScale {
    domain: (f64, f64),
    range: (f32, f32),
    base: f64,
}

pub struct TemporalScale {
    domain: (DateTime, DateTime),
    range: (f32, f32),
    time_zone: Option<TimeZone>,
}

pub struct OrdinalScale {
    domain: Vec<String>,
    range: (f32, f32),
    padding: f32,
}
```

## Dependencies

### Required Stories (Must Complete First)

- **GUP-089**: Core Axis System Infrastructure (provides axis rendering
  foundation)
- **GUP-090**: Automatic Tick Generation Algorithm (provides intelligent tick
  spacing)
- **GUP-091**: Grid Line Rendering System (provides grid coordinate system)
- **GUP-092**: Label Formatting and Positioning (provides text rendering and
  formatting)

### Related Stories

- **GUP-005**: Shader Function Trait ✅ (scales must integrate with shader
  pipeline)
- **GUP-018**: Observable Plot Chart Builders ✅ (primary integration target)

## User Stories

### As a Business Analyst

> "I want my charts to automatically show appropriate axes without any
> configuration so that I can focus on data insights rather than formatting
> details."

**Scenario**: Creating scatter plot from sales data CSV  
**Expected**: Chart automatically detects numeric columns, applies currency
formatting, shows appropriate tick intervals  
**Acceptance**: Professional-looking chart with zero manual axis configuration

### As a Data Scientist

> "I want different axis scales (linear, log, time) to automatically get
> appropriate formatting and tick generation so that my diverse datasets are
> properly visualized."

**Scenario**: Multi-panel dashboard with linear sales data, log-scale bacterial
growth, and time-series temperature  
**Expected**: Each chart type gets appropriate scale, formatting, and tick
generation automatically  
**Acceptance**: Linear uses standard number formatting, log shows scientific
notation, time shows date labels

### As a Frontend Developer

> "I want to be able to override automatic axis decisions when needed so that
> charts can meet specific design requirements while keeping automatic behavior
> as default."

**Scenario**: Dashboard needs specific color scheme and custom tick intervals
for brand consistency  
**Expected**: Can override automatic decisions while keeping automatic behavior
for uncustomized aspects  
**Acceptance**: Progressive customization API allows granular control without
breaking automatic behavior

## Implementation Approach

### Phase 1: Data Analysis and Scale Selection (2 days)

1. **Implement DataAnalyzer** with automatic scale type detection
2. **Scale factory system** creating appropriate scales from data analysis
3. **Domain calculation** algorithms for optimal data range coverage
4. **Basic integration testing** with simple datasets

### Phase 2: Coordinated Rendering System (2 days)

1. **AxisSystem implementation** coordinating all axis components
2. **Integrated rendering pipeline** for complete axis system
3. **Layout calculation** with margin requirements from all components
4. **Performance optimization** for coordinated rendering

### Phase 3: Chart Builder Integration (2 days)

1. **Enhanced chart builders** with automatic axis generation
2. **Progressive customization API** for axis override capability
3. **Type safety improvements** catching scale/data mismatches
4. **Integration testing** across all chart types

## Testing Strategy

### Unit Tests

- Data analysis accuracy for different data types
- Scale selection logic for various datasets
- Coordinate system integration
- Layout calculation correctness

### Integration Tests

- Complete axis system rendering
- Chart builder automatic behavior
- Scale and shader function integration
- Performance with large datasets

### End-to-End Tests

- Real-world dataset processing
- Cross-platform rendering consistency
- Memory usage and performance profiling
- User workflow simulation

### Regression Tests

- Existing chart functionality preservation
- API compatibility maintenance
- Performance baseline maintenance

## Success Metrics

### Automatic Behavior Quality

- ✅ **Scale detection accuracy** - >95% appropriate scale selection for common
  datasets
- ✅ **Professional appearance** - automatic charts look as good as manually
  configured ones
- ✅ **Zero-configuration usability** - users can create professional charts
  without axis configuration
- ✅ **Edge case handling** - system gracefully handles unusual data patterns

### Performance Targets

- ✅ **Complete axis rendering <2ms** for typical chart configurations
- ✅ **Memory efficiency** - <5MB total axis system overhead per chart
- ✅ **Scalability** - performance remains consistent with large datasets (100K+
  points)
- ✅ **Cross-platform consistency** - <10% performance variance across platforms

### Integration Success

- ✅ **Chart builder integration** - all builders support automatic axes with
  override capability
- ✅ **API consistency** - axis configuration follows established fluent API
  patterns
- ✅ **Backward compatibility** - existing chart code continues to work
- ✅ **Type safety** - compile-time prevention of common scale/data mismatches

### User Experience

- ✅ **Documentation completeness** - comprehensive examples and customization
  guides
- ✅ **Error message quality** - clear guidance when automatic detection fails
- ✅ **Customization discoverability** - users can easily find and use override
  options
- ✅ **Performance transparency** - system performance meets user expectations

## Risks and Mitigations

### Automatic Detection Accuracy Risk

**Risk**: Automatic scale detection produces inappropriate results for some
datasets  
**Likelihood**: Medium  
**Impact**: High  
**Mitigation**: Extensive testing with diverse datasets, clear override
mechanisms, user feedback integration

### Integration Performance Risk

**Risk**: Coordinating multiple axis components creates performance
bottlenecks  
**Likelihood**: Medium  
**Impact**: Medium  
**Mitigation**: Performance profiling throughout development, batched rendering
optimization, component-level performance budgets

### API Complexity Risk

**Risk**: Integration API becomes too complex for users to understand and
customize  
**Likelihood**: Medium  
**Impact**: Medium  
**Mitigation**: Progressive disclosure in API design, comprehensive
documentation, user testing of common workflows

### Backward Compatibility Risk

**Risk**: Integration changes break existing chart functionality  
**Likelihood**: Low  
**Impact**: High  
**Mitigation**: Comprehensive regression testing, careful API design, gradual
migration path

## Follow-up Stories

This story enables:

- **GUP-094**: Axis Performance Optimization (optimizing the complete integrated
  system)

This story completes:

- **Phase 2 Initiative 2**: Automatic Scale and Axis System (primary
  deliverable)

This story may identify need for:

- **GUP-095**: Advanced Scale Types (if additional scale types are needed)
- **GUP-096**: Axis Customization UI (if advanced customization interface is
  needed)
- **GUP-097**: Multi-Axis Charts (if complex multi-axis scenarios require
  dedicated support)

## Definition of Done

- [ ] All acceptance criteria verified through automated tests
- [ ] Data analysis accuracy validated with diverse datasets
- [ ] Complete axis system rendering performance meets targets
- [ ] Chart builder integration complete and tested
- [ ] Cross-platform consistency validated
- [ ] Backward compatibility maintained and verified
- [ ] Documentation with automatic behavior and customization examples
- [ ] User acceptance testing with target workflows completed
- [ ] Code review completed with team approval
- [ ] Performance regression testing baseline established

---

**Business Value**: Delivers the complete "automatic scale and axis system" that
enables professional data visualization without configuration burden. This is a
key differentiator that makes Gup competitive with established visualization
tools while maintaining GPU performance advantages.

**Technical Value**: Establishes cohesive, high-performance axis system that
demonstrates successful integration of all Phase 2 axis components while
providing foundation for advanced visualization features in Phase 3.

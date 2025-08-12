# GUP-092: Label Formatting and Positioning

## Story Overview

**Epic**: Phase 2 - High-Level Convenience APIs  
**Theme**: Automatic Scale and Axis System  
**Priority**: Medium  
**Story Points**: 10  
**Status**: 📋 Planned

## Problem Statement

Professional data visualizations require properly formatted and positioned axis
labels that help users interpret data values. Current axis infrastructure can
render tick marks but lacks the text rendering system needed for numeric labels,
date formatting, and intelligent label positioning. Without formatted labels,
charts are difficult to interpret and appear incomplete. Labels must be
GPU-accelerated, support various number formats, handle text overlap
intelligently, and integrate seamlessly with the existing axis and tick
generation systems.

## Business Context

Axis labels are fundamental to data visualization usability. Users need to see
formatted values (currency, percentages, scientific notation, dates) positioned
clearly without overlapping. Professional tools like Excel, Tableau, and D3.js
provide sophisticated label formatting that users expect. This is a complex
story due to text rendering requirements and the variety of formatting needs
across different data types.

## Acceptance Criteria

### Text Rendering Infrastructure

- [ ] **GPU-accelerated text rendering** using SDF (Signed Distance Field) or
      similar technique
- [ ] **Font loading and caching** system for consistent typography across
      platforms
- [ ] **High-quality text output** that looks crisp at all scale factors and
      display densities
- [ ] **Performance at scale** - hundreds of labels render without performance
      degradation
- [ ] **Memory efficient** text atlas management with automatic cleanup

### Label Formatting System

- [ ] **Numeric formatting** with locale-aware thousands separators and decimal
      places
- [ ] **Currency formatting** supporting various currencies and locales
      ($1,234.56, €1.234,56)
- [ ] **Percentage formatting** with automatic percentage conversion and display
- [ ] **Scientific notation** for very large or very small numbers (1.2e+6)
- [ ] **Date/time formatting** with configurable patterns (MM/DD/YYYY,
      DD-MMM-YYYY, etc.)
- [ ] **Custom formatting** functions for domain-specific label needs

### Intelligent Positioning

- [ ] **Overlap detection** and automatic label spacing or rotation to prevent
      collisions
- [ ] **Rotation support** for labels that don't fit horizontally (45°, 90°
      rotations)
- [ ] **Alignment options** for different axis positions (left, right, center,
      top, bottom)
- [ ] **Margin calculation** providing space requirements for layout system
- [ ] **Responsive behavior** that adapts label density and formatting to
      available space

### Integration Requirements

- [ ] **Tick position coordination** with GUP-090 tick generation system
- [ ] **Scale value mapping** from tick positions to formatted display values
- [ ] **Chart builder integration** with automatic label formatting based on
      data types
- [ ] **Accessibility support** providing screen reader compatible label text
- [ ] **Customization API** allowing override of automatic formatting decisions

## Technical Requirements

### Text Rendering Architecture

```rust
pub struct TextRenderer {
    /// SDF font atlas for GPU text rendering
    font_atlas: FontAtlas,
    /// Text rendering pipeline
    text_pipeline: RenderPipeline,
    /// Cached glyph geometry for efficient reuse
    glyph_cache: GlyphCache,
}

pub struct FontAtlas {
    /// GPU texture containing SDF glyph data
    atlas_texture: wgpu::Texture,
    /// Glyph metadata (positions, sizes, metrics)
    glyph_info: HashMap<char, GlyphInfo>,
    /// Font metrics (line height, baseline, etc.)
    font_metrics: FontMetrics,
}

#[derive(Debug, Clone)]
pub struct GlyphInfo {
    /// Position in atlas texture (UV coordinates)
    atlas_pos: [f32; 4],
    /// Glyph dimensions in pixels
    size: [f32; 2],
    /// Bearing (offset from baseline)
    bearing: [f32; 2],
    /// Horizontal advance for cursor positioning
    advance: f32,
}

impl TextRenderer {
    /// Render text at specified position with formatting
    pub fn render_text(
        &mut self,
        context: &mut RenderContext,
        text: &str,
        position: Vec2,
        style: &TextStyle,
    ) -> GupResult<TextBounds> {
        let glyphs = self.layout_text(text, position, style)?;
        self.render_glyphs(context, &glyphs, style)?;
        Ok(TextBounds::from_glyphs(&glyphs))
    }

    /// Calculate text bounds without rendering (for layout)
    pub fn measure_text(&self, text: &str, style: &TextStyle) -> TextBounds {
        // Implementation for layout calculations
    }
}
```

### Label Formatter Implementation

```rust
pub trait LabelFormatter: Send + Sync + 'static {
    /// Format a numeric value for display
    fn format_value(&self, value: f64) -> String;

    /// Get preferred label spacing for this formatter
    fn preferred_spacing(&self) -> f32;

    /// Estimate label width for layout calculations
    fn estimate_width(&self, value: f64) -> f32;
}

pub struct NumericFormatter {
    /// Number of decimal places to show
    pub precision: usize,
    /// Whether to use thousands separators
    pub use_thousands_separator: bool,
    /// Locale for formatting rules
    pub locale: Locale,
    /// Minimum significant digits
    pub min_significant_digits: Option<usize>,
}

impl LabelFormatter for NumericFormatter {
    fn format_value(&self, value: f64) -> String {
        if value.abs() >= 1e6 || value.abs() <= 1e-4 {
            format!("{:.precision$e}", value, precision = self.precision)
        } else {
            self.format_standard_notation(value)
        }
    }

    fn preferred_spacing(&self) -> f32 {
        // Based on typical character width and label length
        60.0 // pixels
    }
}

pub struct DateTimeFormatter {
    pub pattern: String, // "MM/DD/YYYY", "DD-MMM-YYYY", etc.
    pub locale: Locale,
}

pub struct CurrencyFormatter {
    pub currency: Currency,
    pub locale: Locale,
    pub precision: usize,
}

pub struct PercentageFormatter {
    pub precision: usize,
    pub multiply_by_100: bool, // true if input is 0.25 -> "25%"
}
```

### Label Positioning System

```rust
pub struct LabelPositioner {
    /// Collision detection and resolution
    collision_detector: CollisionDetector,
    /// Text measurement for layout
    text_measurer: TextMeasurer,
}

pub struct LabelLayout {
    /// Final positions for each label
    pub positions: Vec<LabelPosition>,
    /// Labels that were hidden due to space constraints
    pub hidden_labels: Vec<usize>,
    /// Total space required for labels
    pub margin_requirements: Margins,
}

#[derive(Debug, Clone)]
pub struct LabelPosition {
    pub position: Vec2,
    pub rotation: f32, // Rotation in radians
    pub anchor: TextAnchor,
    pub text: String,
}

#[derive(Debug, Clone, Copy)]
pub enum TextAnchor {
    TopLeft, TopCenter, TopRight,
    CenterLeft, Center, CenterRight,
    BottomLeft, BottomCenter, BottomRight,
}

impl LabelPositioner {
    pub fn layout_labels(
        &mut self,
        tick_positions: &[f64],
        axis_info: &AxisInfo,
        formatter: &dyn LabelFormatter,
        constraints: &LayoutConstraints,
    ) -> GupResult<LabelLayout> {
        // 1. Generate initial label positions at tick marks
        let mut labels = self.generate_initial_labels(tick_positions, axis_info, formatter)?;

        // 2. Detect collisions and adjust positioning
        self.resolve_collisions(&mut labels, constraints)?;

        // 3. Apply rotation if needed for space constraints
        if self.labels_still_overlap(&labels) {
            self.apply_rotation(&mut labels, constraints.rotation_options)?;
        }

        // 4. Hide labels if still overlapping after rotation
        if self.labels_still_overlap(&labels) {
            self.hide_overlapping_labels(&mut labels);
        }

        Ok(LabelLayout {
            positions: labels,
            hidden_labels: vec![], // Track which labels were hidden
            margin_requirements: self.calculate_margins(&labels),
        })
    }
}
```

### Integration with Chart Builders

```rust
// Extend chart builders with label formatting options
pub trait LabelCapableBuilder: ChartBuilder {
    /// Set number format for axis labels
    fn number_format(self, formatter: Box<dyn LabelFormatter>) -> Self;

    /// Set date format pattern
    fn date_format(self, pattern: &str) -> Self;

    /// Set currency formatting
    fn currency_format(self, currency: Currency, locale: Locale) -> Self;

    /// Set percentage formatting
    fn percentage_format(self, precision: usize) -> Self;

    /// Allow label rotation for space constraints
    fn allow_label_rotation(self, allow: bool) -> Self;

    /// Custom label formatter function
    fn custom_labels<F>(self, formatter: F) -> Self
    where F: Fn(f64) -> String + Send + Sync + 'static;
}

impl<T> LabelCapableBuilder for ScatterPlotBuilder<T> {
    fn number_format(mut self, formatter: Box<dyn LabelFormatter>) -> Self {
        self.axis_config.label_formatter = Some(formatter);
        self
    }

    // ... other implementations
}
```

## Dependencies

### Required Stories (Must Complete First)

- **GUP-089**: Core Axis System Infrastructure (provides axis positioning and
  coordinate system)
- **GUP-090**: Automatic Tick Generation Algorithm (provides tick positions for
  label placement)

### Strongly Related Stories

- **GUP-091**: Grid Line Rendering System (labels coordinate with grid system)
- **GUP-093**: Scale-Axis Integration System (provides scale-to-value mapping)

### Text Rendering Dependencies

This story requires implementing or integrating a GPU text rendering system,
which is substantial work:

- Font loading and SDF atlas generation
- GPU text rendering shaders
- Glyph caching and layout systems
- Cross-platform font handling

## User Stories

### As a Financial Analyst

> "I want currency values to display with proper formatting so that stakeholders
> can quickly understand monetary amounts in our revenue charts."

**Scenario**: Creating quarterly revenue charts with values like 1234567.89  
**Expected**: Labels display as "$1,234,568" with proper currency symbols and
thousands separators  
**Acceptance**: Numbers formatted according to locale conventions, readable
without manual parsing

### As a Data Scientist

> "I want scientific notation for very large or small numbers so that my
> statistical charts remain readable across extreme value ranges."

**Scenario**: Plotting concentration data ranging from 0.0001 to 10000000  
**Expected**: Small values show as "1.0e-4" and large values as "1.0e+7"  
**Acceptance**: Automatic scientific notation threshold, consistent precision
display

### As a Business Dashboard Developer

> "I want axis labels to automatically rotate when they would overlap so that
> charts look professional at all sizes without manual intervention."

**Scenario**: Monthly sales chart on mobile device with limited horizontal
space  
**Expected**: Month labels automatically rotate 45° to fit without overlapping  
**Acceptance**: Intelligent collision detection and automatic rotation fallback

### As an International User

> "I want date and number formatting to respect my locale settings so that
> charts display data in familiar formats."

**Scenario**: European user viewing charts with dates and decimal numbers  
**Expected**: Dates show as "DD-MM-YYYY" and numbers use comma as decimal
separator  
**Acceptance**: Proper locale-aware formatting for dates, numbers, and currency

## Implementation Approach

### Phase 1: Text Rendering Foundation (4 days)

1. **Research text rendering approaches** (SDF, bitmap, vector) and select
   optimal solution
2. **Implement font loading** and atlas generation system
3. **Create GPU text rendering pipeline** with basic positioning
4. **Basic glyph caching** for performance optimization

### Phase 2: Label Formatting System (3 days)

1. **Implement LabelFormatter trait** and basic numeric formatter
2. **Add specialized formatters** for currency, percentage, scientific notation
3. **Date/time formatting** with pattern support
4. **Locale-aware formatting** integration

### Phase 3: Positioning and Layout (2 days)

1. **Collision detection** algorithms for label overlap
2. **Automatic rotation** system for space constraints
3. **Margin calculation** for layout integration
4. **Integration with tick generation** system

### Phase 4: Integration and Polish (1 day)

1. **Chart builder integration** with formatting options
2. **Performance optimization** and memory management
3. **Cross-platform testing** for font consistency
4. **Documentation and examples**

## Testing Strategy

### Unit Tests

- Label formatting accuracy across different number ranges
- Date/time pattern parsing and formatting
- Collision detection algorithms
- Text measurement accuracy

### Visual Tests

- Label positioning at different chart sizes
- Rotation behavior with space constraints
- Font rendering quality across platforms
- Locale-specific formatting validation

### Performance Tests

- Text rendering performance with hundreds of labels
- Memory usage of font atlas and glyph cache
- Label layout calculation speed
- Cross-platform performance comparison

### Integration Tests

- Axis system coordination
- Tick position alignment with labels
- Chart builder API integration
- Accessibility text extraction

## Success Metrics

### Text Rendering Quality

- ✅ **Crisp text rendering** at all display scales and DPI settings
- ✅ **Cross-platform consistency** - fonts look identical across platforms
- ✅ **Performance targets** - 500+ labels render in <5ms
- ✅ **Memory efficiency** - font atlas <10MB, efficient glyph caching

### Formatting Accuracy

- ✅ **Locale compliance** - formatting matches platform conventions
- ✅ **Edge case handling** - extreme values, special numbers (NaN, infinity)
- ✅ **Format consistency** - consistent precision and style within chart
- ✅ **Custom formatting** - user-defined formatters work correctly

### Positioning Intelligence

- ✅ **Collision avoidance** - no overlapping labels in reasonable scenarios
- ✅ **Rotation effectiveness** - labels remain readable when rotated
- ✅ **Space utilization** - maximum labels shown while maintaining readability
- ✅ **Margin accuracy** - layout system gets correct space requirements

### Integration Success

- ✅ **Automatic behavior** - good defaults without configuration
- ✅ **Customization capability** - formatting can be overridden when needed
- ✅ **Chart builder integration** - all builders support label formatting
- ✅ **Accessibility support** - labels accessible to screen readers

## Risks and Mitigations

### Text Rendering Complexity Risk

**Risk**: GPU text rendering is complex and may be difficult to implement
correctly  
**Likelihood**: High  
**Impact**: High  
**Mitigation**: Consider integrating existing text rendering library
(wgpu_glyph, fontdue) rather than building from scratch

### Cross-Platform Font Risk

**Risk**: Font handling differs significantly between platforms  
**Likelihood**: Medium  
**Impact**: Medium  
**Mitigation**: Use embedded fonts for consistency, extensive cross-platform
testing

### Text Performance Risk

**Risk**: Text rendering impacts chart performance significantly  
**Likelihood**: Medium  
**Impact**: High  
**Mitigation**: Aggressive caching, batched rendering, performance profiling
throughout development

### Formatting Complexity Risk

**Risk**: Locale-aware formatting is complex and error-prone  
**Likelihood**: Medium  
**Impact**: Medium  
**Mitigation**: Use established formatting libraries where possible,
comprehensive test coverage for different locales

## Follow-up Stories

This story enables:

- **GUP-093**: Scale-Axis Integration System (complete axis system with labels)
- **GUP-094**: Axis Performance Optimization (including text rendering
  optimization)

This story may spawn additional stories if text rendering proves more complex:

- **GUP-095**: Advanced Text Rendering System (if text system needs to be
  separated)
- **GUP-096**: Font Management and Loading System
- **GUP-097**: Text Layout and Typography Engine

## Definition of Done

- [ ] All acceptance criteria verified through automated tests
- [ ] Text rendering quality validated across all target platforms
- [ ] Label formatting accuracy verified for all supported number types
- [ ] Collision detection and positioning working correctly
- [ ] Performance targets met with comprehensive benchmarking
- [ ] Integration complete with axis system and chart builders
- [ ] Accessibility compliance verified
- [ ] Documentation with formatting examples published
- [ ] Cross-platform testing completed
- [ ] Code review completed with team approval

---

**Business Value**: Provides professional-quality axis labels that are essential
for chart interpretation and user adoption. Proper formatting and positioning
significantly improve user experience and chart credibility.

**Technical Value**: Establishes reusable text rendering and formatting systems
that can be leveraged throughout the visualization library while maintaining GPU
acceleration benefits and cross-platform consistency.

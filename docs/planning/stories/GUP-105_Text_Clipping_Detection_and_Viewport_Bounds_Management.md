# GUP-105: Text Clipping Detection and Viewport Bounds Management

## Story Overview

**Epic**: Phase 2 - High-Level Convenience APIs  
**Theme**: Advanced Text Layout and Rendering  
**Priority**: Medium  
**Story Points**: 8  
**Status**: 📋 Planned  
**Dependencies**: GUP-099 (GPU Text Rendering), GUP-104 (SDF Glyph Texture
Upload)

## Problem Statement

The current text rendering system successfully renders text at specified
positions but lacks awareness of viewport boundaries and container constraints.
Text elements can extend beyond visible areas, get cut off at screen edges, or
overflow their designated containers without any indication or automatic
handling. This creates poor user experience in data visualizations where labels,
titles, or annotations may become partially or completely invisible.

Professional data visualization tools automatically handle text clipping through
various strategies including truncation with ellipsis, dynamic font size
reduction, text wrapping, and clipping indicators. Users expect text to remain
readable and appropriately sized within available space without manual
intervention.

## Business Context

In real-world data visualizations, text elements frequently encounter space
constraints:

- **Chart Titles**: May exceed chart width on small screens or when dynamically
  generated
- **Axis Labels**: Can extend beyond plot margins, especially with long
  categorical names
- **Data Point Labels**: May overflow beyond chart boundaries when positioned
  near edges
- **Legends**: Can become cut off when containing many items or long
  descriptions
- **Tooltips**: May extend beyond viewport when triggered near screen edges

Modern visualization libraries like D3.js, Chart.js, and Plotly.js provide
sophisticated text clipping management that adapts to container constraints.
Users expect similar intelligent behavior from professional visualization tools.

## User Stories

### Primary User Story

**As a** data visualization developer  
**I want** text elements to automatically adapt to container boundaries  
**So that** all text remains readable and appropriately positioned within
available space

### Secondary User Stories

**As a** dashboard designer  
**I want** long axis labels to truncate gracefully with ellipsis  
**So that** chart layouts remain clean and readable

**As a** chart user  
**I want** to see visual indicators when text has been clipped  
**So that** I know additional information is available

**As a** mobile application developer  
**I want** text to dynamically resize based on viewport constraints  
**So that** visualizations work properly across different screen sizes

## Success Criteria

### 1. **Viewport Boundary Detection**

- Accurate detection when text extends beyond visible viewport
- Support for custom container bounds (not just full viewport)
- Real-time boundary checking during interactive operations (zoom, pan)
- Integration with existing text positioning and anchor systems

### 2. **Automatic Clipping Strategies**

- **Truncation with Ellipsis**: Intelligent text shortening with "..." indicator
- **Dynamic Font Scaling**: Automatic font size reduction to fit available space
- **Text Wrapping**: Multi-line text wrapping within container boundaries
- **Position Adjustment**: Smart repositioning to keep text within bounds

### 3. **Visual Feedback Systems**

- Clipping indicators showing truncated content availability
- Hover/interaction to reveal full text content
- Optional overflow scrolling for long text content
- Consistent visual styling for clipped text elements

### 4. **Performance Requirements**

- Clipping detection adds <2% overhead to text rendering performance
- Real-time boundary checking during interactive operations
- Efficient algorithms for large numbers of text elements (500+ labels)
- Minimal memory footprint for boundary tracking data structures

### 5. **Developer Experience**

- Simple API for enabling/configuring clipping behavior
- Configurable clipping strategies per text element or globally
- Integration with existing TextStyle and TextRenderConfig APIs
- Clear documentation and examples for common use cases

## Technical Approach

### Core Architecture Components

#### 1. **Viewport Bounds Manager**

```rust
pub struct ViewportBounds {
    /// Visible area coordinates
    pub viewport_rect: Rect,
    /// Container-specific bounds (optional)
    pub container_bounds: Option<Rect>,
    /// Margin requirements for text padding
    pub text_margins: TextMargins,
}

pub struct TextMargins {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl ViewportBounds {
    fn check_text_clipping(&self, text_bounds: &TextBounds) -> ClippingResult;
    fn calculate_available_space(&self, anchor_point: Vec2) -> Rect;
    fn suggest_optimal_position(&self, text_bounds: &TextBounds) -> Option<Vec2>;
}
```

#### 2. **Clipping Detection Engine**

```rust
#[derive(Debug, Clone)]
pub enum ClippingResult {
    NoClipping,
    PartialClipping {
        clipped_edges: Vec<ClippedEdge>,
        visible_area: f32, // Percentage of text visible (0.0-1.0)
    },
    CompletelyClipped,
}

#[derive(Debug, Clone)]
pub enum ClippedEdge {
    Top { overflow_pixels: f32 },
    Right { overflow_pixels: f32 },
    Bottom { overflow_pixels: f32 },
    Left { overflow_pixels: f32 },
}

pub struct ClippingDetector {
    viewport_bounds: ViewportBounds,
    strategy_config: ClippingStrategyConfig,
}

impl ClippingDetector {
    fn detect_clipping(&self, text_bounds: &TextBounds) -> ClippingResult;
    fn calculate_optimal_font_size(&self, text: &str, available_space: Rect, style: &TextStyle) -> f32;
    fn suggest_truncation_point(&self, text: &str, available_width: f32, style: &TextStyle) -> usize;
}
```

#### 3. **Clipping Strategy System**

```rust
#[derive(Debug, Clone)]
pub struct ClippingStrategyConfig {
    pub primary_strategy: ClippingStrategy,
    pub fallback_strategies: Vec<ClippingStrategy>,
    pub minimum_visible_percentage: f32, // Don't render if less than X% visible
    pub enable_hover_reveal: bool,
}

#[derive(Debug, Clone)]
pub enum ClippingStrategy {
    /// Truncate text with ellipsis
    TruncateWithEllipsis {
        ellipsis_text: String, // Default: "..."
        preserve_words: bool,  // Try to break at word boundaries
    },
    /// Reduce font size to fit
    DynamicFontScaling {
        min_font_size: f32,
        scale_factor: f32,     // How aggressively to scale
    },
    /// Wrap text to multiple lines
    TextWrapping {
        max_lines: Option<usize>,
        line_spacing_factor: f32,
    },
    /// Move text to stay within bounds
    RepositionText {
        prefer_directions: Vec<Vec2>, // Preferred offset directions
        max_offset_distance: f32,
    },
    /// Hide text completely if it doesn't fit
    HideIfClipped {
        min_visible_threshold: f32, // Hide if less than X% visible
    },
}
```

#### 4. **Enhanced TextLayoutEngine Integration**

```rust
// Extend existing TextLayoutEngine
impl TextLayoutEngine {
    /// Enhanced layout with viewport boundary awareness
    pub fn layout_text_with_clipping(
        &mut self,
        text: &str,
        position: Vec2,
        style: &TextStyle,
        font_atlas: &FontAtlas,
        constraints: Option<&LabelConstraints>,
        viewport_bounds: &ViewportBounds,
        clipping_config: &ClippingStrategyConfig,
    ) -> GupResult<LayoutResult> {
        // 1. Perform initial layout
        let initial_layout = self.layout_text(text, position, style, font_atlas, constraints)?;

        // 2. Check for clipping
        let clipping_result = self.check_clipping(&initial_layout.bounds, viewport_bounds);

        // 3. Apply clipping strategies if needed
        match clipping_result {
            ClippingResult::NoClipping => Ok(initial_layout),
            ClippingResult::PartialClipping { .. } | ClippingResult::CompletelyClipped => {
                self.apply_clipping_strategies(text, position, style, font_atlas,
                                             viewport_bounds, clipping_config)
            }
        }
    }

    fn apply_clipping_strategies(
        &mut self,
        text: &str,
        position: Vec2,
        style: &TextStyle,
        font_atlas: &FontAtlas,
        viewport_bounds: &ViewportBounds,
        config: &ClippingStrategyConfig,
    ) -> GupResult<LayoutResult>;
}
```

### Implementation Strategy

#### Phase 1: Core Clipping Detection (3 days)

1. **Boundary Detection Infrastructure**

   - Implement ViewportBounds and TextMargins structures
   - Create ClippingDetector with accurate boundary checking algorithms
   - Add clipping result types and edge detection logic
   - Unit tests for boundary detection accuracy

2. **Integration with Existing Text System**
   - Extend TextLayoutEngine with clipping awareness
   - Update LayoutResult to include clipping information
   - Modify TextRenderConfig to accept viewport bounds
   - Ensure backward compatibility with existing APIs

#### Phase 2: Clipping Strategy Implementation (3 days)

1. **Truncation with Ellipsis**

   - Implement intelligent text truncation algorithms
   - Word boundary preservation logic
   - Configurable ellipsis styling and positioning
   - Support for different text directions and anchors

2. **Dynamic Font Scaling**

   - Binary search algorithm for optimal font size
   - Minimum font size constraints and validation
   - Integration with existing TextStyle font size system
   - Performance optimization for real-time scaling

3. **Text Repositioning**
   - Smart position adjustment algorithms
   - Preferred direction handling with fallback options
   - Collision detection integration (when GUP-101 is available)
   - Boundary constraint validation

#### Phase 3: Advanced Features (2 days)

1. **Text Wrapping Implementation**

   - Multi-line text layout algorithms
   - Line height and spacing calculations
   - Word wrapping with hyphenation considerations
   - Maximum line limit enforcement

2. **Visual Feedback Systems**
   - Clipping indicator rendering
   - Hover interaction for revealing full content
   - Consistent styling for clipped elements
   - Integration with existing text rendering pipeline

### Technical Implementation Details

#### Boundary Detection Algorithm

```rust
impl ClippingDetector {
    fn detect_clipping(&self, text_bounds: &TextBounds) -> ClippingResult {
        let viewport = &self.viewport_bounds.viewport_rect;
        let container = self.viewport_bounds.container_bounds.as_ref().unwrap_or(viewport);

        let mut clipped_edges = Vec::new();
        let mut total_area = text_bounds.width() * text_bounds.height();
        let mut visible_area = total_area;

        // Check each edge for clipping
        if text_bounds.left < container.left {
            let overflow = container.left - text_bounds.left;
            clipped_edges.push(ClippedEdge::Left { overflow_pixels: overflow });
            visible_area -= overflow * text_bounds.height();
        }

        if text_bounds.right > container.right {
            let overflow = text_bounds.right - container.right;
            clipped_edges.push(ClippedEdge::Right { overflow_pixels: overflow });
            visible_area -= overflow * text_bounds.height();
        }

        if text_bounds.top < container.top {
            let overflow = container.top - text_bounds.top;
            clipped_edges.push(ClippedEdge::Top { overflow_pixels: overflow });
            visible_area -= text_bounds.width() * overflow;
        }

        if text_bounds.bottom > container.bottom {
            let overflow = text_bounds.bottom - container.bottom;
            clipped_edges.push(ClippedEdge::Bottom { overflow_pixels: overflow });
            visible_area -= text_bounds.width() * overflow;
        }

        if visible_area <= 0.0 {
            ClippingResult::CompletelyClipped
        } else if clipped_edges.is_empty() {
            ClippingResult::NoClipping
        } else {
            ClippingResult::PartialClipping {
                clipped_edges,
                visible_area: visible_area / total_area,
            }
        }
    }
}
```

#### Truncation Algorithm

```rust
impl ClippingDetector {
    fn suggest_truncation_point(&self, text: &str, available_width: f32, style: &TextStyle) -> usize {
        if text.is_empty() { return 0; }

        let ellipsis = "...";
        let ellipsis_width = self.measure_text_width(ellipsis, style);
        let target_width = available_width - ellipsis_width;

        if target_width <= 0.0 { return 0; }

        // Binary search for optimal truncation point
        let mut left = 0;
        let mut right = text.chars().count();
        let mut best_fit = 0;

        while left <= right {
            let mid = (left + right) / 2;
            let text_slice: String = text.chars().take(mid).collect();
            let width = self.measure_text_width(&text_slice, style);

            if width <= target_width {
                best_fit = mid;
                left = mid + 1;
            } else {
                right = mid - 1;
            }
        }

        // Adjust for word boundaries if configured
        if self.strategy_config.preserve_words {
            self.adjust_for_word_boundary(text, best_fit)
        } else {
            best_fit
        }
    }

    fn adjust_for_word_boundary(&self, text: &str, truncate_at: usize) -> usize {
        let chars: Vec<char> = text.chars().collect();
        if truncate_at >= chars.len() { return truncate_at; }

        // Look backward for whitespace
        for i in (0..truncate_at).rev() {
            if chars[i].is_whitespace() {
                return i;
            }
        }

        // If no whitespace found, use original truncation point
        truncate_at
    }
}
```

## Acceptance Criteria

### Functional Requirements

#### Must Have (MVP)

- [ ] **Accurate Boundary Detection**: Correctly identifies when text extends
      beyond viewport/container bounds
- [ ] **Truncation with Ellipsis**: Implements intelligent text truncation with
      configurable ellipsis
- [ ] **Dynamic Font Scaling**: Automatically reduces font size to fit available
      space
- [ ] **API Integration**: Seamlessly integrates with existing TextLayoutEngine
      and TextRenderConfig
- [ ] **Backward Compatibility**: All existing text rendering functionality
      continues to work unchanged

#### Should Have

- [ ] **Text Repositioning**: Smart position adjustment to keep text within
      bounds
- [ ] **Word Boundary Preservation**: Truncation respects word boundaries when
      possible
- [ ] **Multiple Container Support**: Supports both viewport and custom
      container bounds
- [ ] **Clipping Indicators**: Visual feedback when text has been clipped or
      truncated

#### Could Have

- [ ] **Text Wrapping**: Multi-line text layout within container boundaries
- [ ] **Hover Interactions**: Reveal full text content on hover for truncated
      text
- [ ] **Animation Support**: Smooth transitions when applying clipping
      strategies
- [ ] **Advanced Typography**: Support for text direction and complex script
      clipping

### Performance Requirements

- [ ] **Clipping Detection Performance**: <1ms for boundary checking on 100 text
      elements
- [ ] **Real-time Responsiveness**: Smooth clipping updates during interactive
      zoom/pan operations
- [ ] **Memory Efficiency**: Clipping detection adds <5% to text rendering
      memory usage
- [ ] **Rendering Performance**: Overall text rendering performance impact <2%

### Quality Requirements

- [ ] **Visual Consistency**: Clipped text maintains appropriate styling and
      readability
- [ ] **Accurate Measurements**: Text width/height calculations account for font
      metrics correctly
- [ ] **Edge Case Handling**: Graceful handling of edge cases (empty text,
      zero-width containers, etc.)
- [ ] **Cross-Platform Compatibility**: Consistent behavior across native and
      WebAssembly targets

### Integration Requirements

- [ ] **TextStyle Integration**: Clipping behavior configurable through
      TextStyle or TextRenderConfig
- [ ] **Chart Builder Compatibility**: Works seamlessly with Observable
      Plot-style chart builders
- [ ] **Demo Enhancement**: Enhanced text_rendering_demo showing clipping
      capabilities
- [ ] **Documentation**: Complete API documentation with practical examples

## Technical Constraints

### Performance Constraints

- Clipping detection must not significantly impact real-time rendering
  performance
- Boundary checking algorithms optimized for large numbers of text elements
- Memory usage for viewport tracking must remain minimal

### Compatibility Constraints

- Must work with existing fontdue-based font rasterization
- Integration with current SDF rendering pipeline required
- No breaking changes to public text rendering APIs

### Platform Constraints

- Consistent behavior across native and WebAssembly platforms
- Font measurement accuracy across different operating systems
- Proper handling of high-DPI displays and scaling factors

## Testing Strategy

### Unit Tests

- [ ] Boundary detection accuracy with various text sizes and positions
- [ ] Truncation algorithm correctness with different fonts and styles
- [ ] Font scaling algorithm precision and performance
- [ ] Edge case handling (empty text, zero bounds, extreme font sizes)

### Integration Tests

- [ ] End-to-end clipping workflows with real text rendering
- [ ] Chart builder integration and API compatibility
- [ ] Performance testing with large numbers of clipped text elements
- [ ] Cross-platform behavior validation

### Visual Tests

- [ ] Clipping visual quality and consistency
- [ ] Truncation ellipsis positioning and styling
- [ ] Font scaling visual appearance
- [ ] Interactive clipping behavior during zoom/pan operations

### Performance Tests

- [ ] Clipping detection performance benchmarks
- [ ] Memory usage profiling for viewport tracking
- [ ] Real-time interaction performance validation
- [ ] Scalability testing with 500+ text elements

## Success Metrics

### Functional Success

- [ ] All text elements respect container boundaries automatically
- [ ] Truncated text displays appropriate ellipsis indicators
- [ ] Dynamic font scaling maintains readability within constraints
- [ ] Enhanced demos showcase clipping capabilities effectively

### Performance Success

- [ ] Clipping detection completes in <1ms for 100 text elements
- [ ] Overall rendering performance degradation <2%
- [ ] Memory usage increase <5% for text rendering operations
- [ ] Smooth interactive performance maintained during viewport changes

### Quality Success

- [ ] All existing text rendering tests continue to pass
- [ ] New clipping functionality achieves >95% test coverage
- [ ] Zero regressions in text positioning or layout
- [ ] Clean compilation without warnings

## Risks and Mitigation Strategies

### High Risk: Font Measurement Accuracy

**Risk**: Inaccurate text width/height calculations leading to incorrect
clipping decisions  
**Impact**: Text may be unnecessarily clipped or overflow despite clipping
logic  
**Mitigation**:

- Comprehensive testing with various fonts and sizes
- Use fontdue's precise metrics calculations
- Add validation tests comparing calculated vs. actual rendered bounds
- Implement measurement caching for performance

### Medium Risk: Performance Impact

**Risk**: Clipping detection adds significant overhead to text rendering  
**Impact**: Reduced frame rates in text-heavy visualizations  
**Mitigation**:

- Optimize boundary checking algorithms with spatial indexing
- Implement result caching for static text elements
- Use efficient data structures for viewport tracking
- Profile extensively during development

### Medium Risk: Complex Integration

**Risk**: Clipping system conflicts with existing collision detection or
positioning logic  
**Impact**: Inconsistent text behavior or API conflicts  
**Mitigation**:

- Design clear API boundaries between clipping and collision systems
- Comprehensive integration testing with existing text features
- Backward compatibility validation
- Clear documentation of interaction patterns

### Low Risk: Cross-Platform Differences

**Risk**: Font rendering differences cause inconsistent clipping behavior  
**Impact**: Text clips differently on different platforms  
**Mitigation**:

- Use fontdue for consistent cross-platform font handling
- Extensive testing on native and WebAssembly targets
- Platform-specific test suites
- Fallback strategies for edge cases

## Dependencies

### Internal Dependencies

- **GUP-099**: GPU Text Rendering Pipeline ✅ (provides core text rendering
  infrastructure)
- **GUP-104**: SDF Glyph Texture Upload ✅ (provides working text display)
- **Existing TextLayoutEngine**: Core layout functionality
- **Existing TextStyle System**: Font and styling configuration

### External Dependencies

- **fontdue 0.9**: Font metrics and text measurement
- **wgpu 26.0**: Viewport and rendering context
- **Current font atlas system**: SDF generation and texture management

### Future Integration Opportunities

- **GUP-101**: Label Collision Detection Enhancement (clipping + collision =
  comprehensive text layout)
- **Chart Builder APIs**: Enhanced text clipping configuration through chart
  builders
- **Interactive Systems**: Hover/tooltip integration for revealing clipped
  content

## Definition of Done

### Implementation Complete

- [ ] All core clipping detection functionality implemented
- [ ] Truncation with ellipsis working for all text styles
- [ ] Dynamic font scaling integrated with existing font system
- [ ] API integration complete with backward compatibility maintained

### Testing Complete

- [ ] Comprehensive unit test suite (>95% coverage)
- [ ] Integration tests with existing text rendering system
- [ ] Performance benchmarks meet acceptance criteria
- [ ] Cross-platform validation completed

### Documentation Complete

- [ ] API documentation with practical examples
- [ ] Enhanced text_rendering_demo showcasing clipping features
- [ ] CLAUDE.md updated with clipping patterns and best practices
- [ ] Developer guide for configuring clipping behavior

### Quality Assurance Complete

- [ ] Code review completed and approved
- [ ] All tests passing in CI/CD pipeline
- [ ] Performance validation meets requirements
- [ ] Zero regressions in existing functionality

## Business Value

**Impact**: Medium-High - Significantly improves text layout quality and user
experience  
**Effort**: High - Complex text layout algorithms and extensive integration
work  
**Value/Effort**: Medium - Important feature with substantial implementation
complexity

### Value Delivered

- **Professional Text Layout**: Automatic boundary-aware text positioning
- **Enhanced User Experience**: Text always remains readable within available
  space
- **Developer Productivity**: Automatic clipping reduces manual text positioning
  work
- **Foundation for Advanced Features**: Enables sophisticated text layout
  capabilities

### Strategic Alignment

This story completes the professional-grade text rendering system by adding the
final missing piece of boundary awareness. Combined with existing collision
detection (GUP-101), it provides comprehensive text layout management comparable
to industry-leading visualization tools.

---

**Story Created**: 2025-01-27  
**Business Analyst**: Expert BA Analysis  
**Estimated Effort**: 8 story points (8 days)  
**Priority**: Medium (enhances existing text system)  
**Dependencies**: GUP-099 ✅, GUP-104 ✅

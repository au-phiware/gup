# GUP-021: Advanced Composition Mode Implementation

## Story Overview

**Title**: Implement Advanced Composition Modes for Mixable Visualizations  
**Epic**: Phase 1 Initiative 1 - Core GPU Primitives and Selection API  
**Priority**: High  
**Story Points**: 6  

## Context

The current Mixable trait implementation treats all composition modes (Overlay, Merge, SideBySide, Custom) identically with placeholder implementations. This story implements the distinct behaviors for each composition mode, enabling rich composition semantics that provide meaningful value to visualization developers.

## User Story

**As a** visualization developer  
**I want** different composition modes to have distinct behaviors  
**So that** I can combine visualizations in semantically meaningful ways (overlaying, merging data, side-by-side layout, custom composition)  

## Acceptance Criteria

### Core Composition Mode Behaviors

- [ ] **Overlay Mode**: Renders second component on top of first with proper depth/blending
- [ ] **Merge Mode**: Combines data sources and renders as unified visualization  
- [ ] **SideBySide Mode**: Automatically partitions viewport to position components adjacently
- [ ] **Custom Mode**: Provides framework for user-defined composition behaviors

### Technical Requirements

- [ ] **Render State Management**: Each mode manages GPU render state appropriately
- [ ] **Viewport Management**: SideBySide mode handles viewport partitioning correctly
- [ ] **Data Integration**: Merge mode combines datasets without data loss
- [ ] **Performance**: Mode-specific optimizations maintain rendering performance

### API Consistency

- [ ] **Uniform Interface**: All modes work through same Mixable trait interface
- [ ] **Mode Switching**: Compositions can change modes without reconstruction
- [ ] **Error Handling**: Mode-specific validation and error reporting
- [ ] **Extensibility**: Framework supports adding new composition modes

## Technical Tasks

### 1. Overlay Mode Implementation

- [ ] Implement proper depth testing and blending for layered rendering
- [ ] Add transparency and alpha compositing support
- [ ] Handle overlapping geometry rendering order
- [ ] Optimize for cases where components don't overlap

### 2. Merge Mode Implementation

- [ ] Design data source combination algorithms
- [ ] Implement data deduplication and merging strategies
- [ ] Create unified shader pipelines for merged data
- [ ] Handle heterogeneous data type combinations

### 3. SideBySide Mode Implementation

- [ ] Implement automatic viewport partitioning algorithms
- [ ] Add layout management for component positioning
- [ ] Handle dynamic resizing and responsive layouts
- [ ] Support both horizontal and vertical splitting

### 4. Custom Mode Framework

- [ ] Define trait/interface for custom composition behaviors
- [ ] Provide composition utilities and helper functions
- [ ] Create examples of common custom composition patterns
- [ ] Document custom mode development guidelines

## Detailed Requirements

### Enhanced ComposedVisualization Implementation

```rust
// Gup - GPU-Accelerated Data Visualization Library
// Copyright (C) 2025 Corin Lawson <corin@phiware.com.au>
//
//! Advanced composition mode implementations for Mixable trait.

use crate::{GupError, GupResult, RenderContext, Viewport};

/// Composition modes define how two mixable components are combined.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CompositionMode {
    /// Render second component on top of first (default)
    #[default]
    Overlay,
    /// Combine data sources and render as unified visualization
    Merge,
    /// Position components adjacent to each other
    SideBySide,
    /// User-defined composition behavior
    Custom,
}

/// Layout direction for SideBySide composition
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutDirection {
    Horizontal,
    Vertical,
}

/// Configuration for SideBySide composition mode
#[derive(Debug, Clone)]
pub struct SideBySideConfig {
    pub direction: LayoutDirection,
    pub split_ratio: f32, // 0.0 to 1.0, proportion allocated to first component
    pub padding: f32,     // Padding between components in pixels
}

impl Default for SideBySideConfig {
    fn default() -> Self {
        Self {
            direction: LayoutDirection::Horizontal,
            split_ratio: 0.5,
            padding: 10.0,
        }
    }
}

/// Custom composition behavior trait
pub trait CustomCompositionBehavior: Send + Sync {
    /// Apply custom composition logic
    fn compose<A: Mixable, B: Mixable>(
        &self,
        first: &A,
        second: &B,
        context: &mut RenderContext,
    ) -> GupResult<()>;

    /// Validate that this custom behavior can handle the given component types
    fn can_compose<A: Mixable, B: Mixable>(&self, first: &A, second: &B) -> bool;

    /// Get a description of this composition behavior
    fn description(&self) -> String;
}

/// Enhanced composition container with mode-specific behaviors
#[derive(Debug)]
pub struct ComposedVisualization<A: Mixable, B: Mixable> {
    /// The first component in the composition
    first: A,
    /// The second component in the composition
    second: B,
    /// How the two components should be combined
    composition_mode: CompositionMode,
    /// Configuration for SideBySide mode
    side_by_side_config: SideBySideConfig,
    /// Custom composition behavior (for Custom mode)
    custom_behavior: Option<Box<dyn CustomCompositionBehavior>>,
}

impl<A: Mixable, B: Mixable> ComposedVisualization<A, B> {
    /// Create a new composed visualization with default overlay mode
    pub fn new(first: A, second: B) -> Self {
        Self {
            first,
            second,
            composition_mode: CompositionMode::default(),
            side_by_side_config: SideBySideConfig::default(),
            custom_behavior: None,
        }
    }

    /// Create a new composed visualization with side-by-side configuration
    pub fn side_by_side(first: A, second: B, config: SideBySideConfig) -> Self {
        Self {
            first,
            second,
            composition_mode: CompositionMode::SideBySide,
            side_by_side_config: config,
            custom_behavior: None,
        }
    }

    /// Create a new composed visualization with custom behavior
    pub fn custom(
        first: A,
        second: B,
        behavior: Box<dyn CustomCompositionBehavior>,
    ) -> Self {
        Self {
            first,
            second,
            composition_mode: CompositionMode::Custom,
            side_by_side_config: SideBySideConfig::default(),
            custom_behavior: Some(behavior),
        }
    }

    /// Configure side-by-side layout parameters
    pub fn with_side_by_side_config(mut self, config: SideBySideConfig) -> Self {
        self.side_by_side_config = config;
        self
    }
}

impl<A: Mixable, B: Mixable> Mixable for ComposedVisualization<A, B> {
    type Output = ();

    fn render(&self, context: &mut RenderContext) -> GupResult<()> {
        // Validate both components before rendering
        if !self.first.is_valid() {
            return Err(GupError::CompositionError(format!(
                "First component is invalid: {}",
                self.first.description()
            )));
        }
        if !self.second.is_valid() {
            return Err(GupError::CompositionError(format!(
                "Second component is invalid: {}",
                self.second.description()
            )));
        }

        // Render based on composition mode
        match self.composition_mode {
            CompositionMode::Overlay => self.render_overlay(context),
            CompositionMode::Merge => self.render_merge(context),
            CompositionMode::SideBySide => self.render_side_by_side(context),
            CompositionMode::Custom => self.render_custom(context),
        }
    }

    fn is_valid(&self) -> bool {
        self.first.is_valid() && self.second.is_valid()
    }

    fn description(&self) -> String {
        format!(
            "ComposedVisualization({:?}, {} + {})",
            self.composition_mode,
            self.first.description(),
            self.second.description()
        )
    }
}

impl<A: Mixable, B: Mixable> ComposedVisualization<A, B> {
    /// Render in overlay mode with proper depth and blending
    fn render_overlay(&self, context: &mut RenderContext) -> GupResult<()> {
        // Enable depth testing and alpha blending for proper layering
        let original_viewport = context.viewport();
        
        // Render first component (background layer)
        self.first.render(context)?;
        
        // Configure blending for overlay
        context.set_blend_mode(BlendMode::AlphaBlending)?;
        
        // Render second component (foreground layer)
        self.second.render(context)?;
        
        // Restore original blend mode
        context.set_blend_mode(BlendMode::default())?;
        
        Ok(())
    }

    /// Render in merge mode by combining data sources
    fn render_merge(&self, context: &mut RenderContext) -> GupResult<()> {
        // For merge mode, we need to extract and combine the underlying data
        // This is a simplified implementation - real merge would depend on data types
        
        // Check if components can be merged (same data types, compatible formats)
        if !self.can_merge_components() {
            return Err(GupError::CompositionError(
                "Components cannot be merged - incompatible data types".to_string()
            ));
        }

        // Extract data from both components (this would be component-specific)
        let merged_data = self.extract_and_merge_data()?;
        
        // Create a temporary merged visualization
        let merged_viz = self.create_merged_visualization(merged_data)?;
        
        // Render the merged visualization
        merged_viz.render(context)?;
        
        Ok(())
    }

    /// Render in side-by-side mode with viewport partitioning
    fn render_side_by_side(&self, context: &mut RenderContext) -> GupResult<()> {
        let original_viewport = context.viewport();
        
        let (first_viewport, second_viewport) = self.calculate_split_viewports(original_viewport);
        
        // Render first component in its viewport
        context.set_viewport(first_viewport)?;
        self.first.render(context)?;
        
        // Render second component in its viewport  
        context.set_viewport(second_viewport)?;
        self.second.render(context)?;
        
        // Restore original viewport
        context.set_viewport(original_viewport)?;
        
        Ok(())
    }

    /// Render using custom composition behavior
    fn render_custom(&self, context: &mut RenderContext) -> GupResult<()> {
        if let Some(custom_behavior) = &self.custom_behavior {
            if !custom_behavior.can_compose(&self.first, &self.second) {
                return Err(GupError::CompositionError(
                    format!("Custom behavior '{}' cannot compose these component types", 
                           custom_behavior.description())
                ));
            }
            
            custom_behavior.compose(&self.first, &self.second, context)
        } else {
            Err(GupError::CompositionError(
                "Custom composition mode requires custom behavior".to_string()
            ))
        }
    }

    /// Check if components can be merged based on their data types
    fn can_merge_components(&self) -> bool {
        // This would be implemented based on specific component types
        // For now, return true as a placeholder
        true
    }

    /// Extract and merge data from both components
    fn extract_and_merge_data(&self) -> GupResult<MergedData> {
        // This would extract actual data from components and merge it
        // Implementation depends on specific component types and data structures
        Ok(MergedData::placeholder())
    }

    /// Create a visualization from merged data
    fn create_merged_visualization(&self, _data: MergedData) -> GupResult<Box<dyn Mixable<Output = ()>>> {
        // This would create an appropriate visualization type for the merged data
        // Implementation depends on the specific component types being merged
        Err(GupError::CompositionError(
            "Merged visualization creation not yet implemented".to_string()
        ))
    }

    /// Calculate viewport splits for side-by-side rendering
    fn calculate_split_viewports(&self, original: Viewport) -> (Viewport, Viewport) {
        match self.side_by_side_config.direction {
            LayoutDirection::Horizontal => {
                let split_x = (original.width as f32 * self.side_by_side_config.split_ratio) as u32;
                let padding = self.side_by_side_config.padding as u32;
                
                let first_viewport = Viewport {
                    width: split_x.saturating_sub(padding / 2),
                    height: original.height,
                    scale_factor: original.scale_factor,
                };
                
                let second_viewport = Viewport {
                    width: original.width.saturating_sub(split_x).saturating_sub(padding / 2),
                    height: original.height,
                    scale_factor: original.scale_factor,
                };
                
                (first_viewport, second_viewport)
            }
            LayoutDirection::Vertical => {
                let split_y = (original.height as f32 * self.side_by_side_config.split_ratio) as u32;
                let padding = self.side_by_side_config.padding as u32;
                
                let first_viewport = Viewport {
                    width: original.width,
                    height: split_y.saturating_sub(padding / 2),
                    scale_factor: original.scale_factor,
                };
                
                let second_viewport = Viewport {
                    width: original.width,
                    height: original.height.saturating_sub(split_y).saturating_sub(padding / 2),
                    scale_factor: original.scale_factor,
                };
                
                (first_viewport, second_viewport)
            }
        }
    }
}

/// Placeholder for merged data - would be replaced with actual data structures
struct MergedData;

impl MergedData {
    fn placeholder() -> Self {
        Self
    }
}

/// Blend modes for overlay composition
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlendMode {
    None,
    AlphaBlending,
    Additive,
    Multiply,
}

impl Default for BlendMode {
    fn default() -> Self {
        BlendMode::None
    }
}

/// Enhanced RenderContext methods for composition support
impl RenderContext {
    /// Set blend mode for rendering operations
    pub fn set_blend_mode(&mut self, _mode: BlendMode) -> GupResult<()> {
        // This would configure GPU blend state
        // Implementation depends on the WebGPU integration from GUP-020
        Ok(())
    }

    /// Set viewport for component rendering
    pub fn set_viewport(&mut self, viewport: Viewport) -> GupResult<()> {
        // This would update the viewport and scissor test
        // Implementation depends on the WebGPU integration from GUP-020
        self.viewport = viewport;
        Ok(())
    }
}
```

### Custom Composition Examples

```rust
/// Example: Cross-fade composition behavior
pub struct CrossFadeComposition {
    fade_factor: f32, // 0.0 = first only, 1.0 = second only
}

impl CustomCompositionBehavior for CrossFadeComposition {
    fn compose<A: Mixable, B: Mixable>(
        &self,
        first: &A,
        second: &B,
        context: &mut RenderContext,
    ) -> GupResult<()> {
        // Render first component with (1.0 - fade_factor) alpha
        context.set_global_alpha(1.0 - self.fade_factor)?;
        first.render(context)?;
        
        // Render second component with fade_factor alpha
        context.set_global_alpha(self.fade_factor)?;
        second.render(context)?;
        
        // Restore alpha
        context.set_global_alpha(1.0)?;
        
        Ok(())
    }

    fn can_compose<A: Mixable, B: Mixable>(&self, _first: &A, _second: &B) -> bool {
        // Cross-fade can compose any two components
        true
    }

    fn description(&self) -> String {
        format!("CrossFade(factor: {:.2})", self.fade_factor)
    }
}

/// Example: Grid layout composition behavior
pub struct GridLayoutComposition {
    rows: u32,
    cols: u32,
    cell_index_first: (u32, u32),
    cell_index_second: (u32, u32),
}

impl CustomCompositionBehavior for GridLayoutComposition {
    fn compose<A: Mixable, B: Mixable>(
        &self,
        first: &A,
        second: &B,
        context: &mut RenderContext,
    ) -> GupResult<()> {
        let original_viewport = context.viewport();
        
        let cell_width = original_viewport.width / self.cols;
        let cell_height = original_viewport.height / self.rows;
        
        // Render first component in its grid cell
        let first_viewport = Viewport {
            width: cell_width,
            height: cell_height,
            scale_factor: original_viewport.scale_factor,
        };
        context.set_viewport(first_viewport)?;
        first.render(context)?;
        
        // Render second component in its grid cell
        let second_viewport = Viewport {
            width: cell_width,
            height: cell_height,
            scale_factor: original_viewport.scale_factor,
        };
        context.set_viewport(second_viewport)?;
        second.render(context)?;
        
        // Restore original viewport
        context.set_viewport(original_viewport)?;
        
        Ok(())
    }

    fn can_compose<A: Mixable, B: Mixable>(&self, _first: &A, _second: &B) -> bool {
        // Check that cell indices are within grid bounds
        self.cell_index_first.0 < self.cols && self.cell_index_first.1 < self.rows &&
        self.cell_index_second.0 < self.cols && self.cell_index_second.1 < self.rows
    }

    fn description(&self) -> String {
        format!("GridLayout({}x{}, cells: {:?}, {:?})", 
                self.rows, self.cols, self.cell_index_first, self.cell_index_second)
    }
}
```

### Enhanced MixableExt with Configuration

```rust
/// Enhanced convenience methods for composition modes
pub trait MixableExt: Mixable + Sized {
    /// Compose with overlay mode (explicit)
    fn overlay<T: Mixable>(self, other: T) -> ComposedVisualization<Self, T> {
        ComposedVisualization::new(self, other)
    }

    /// Compose with merge mode
    fn merge<T: Mixable>(self, other: T) -> ComposedVisualization<Self, T> {
        self.mix_with_mode(other, CompositionMode::Merge)
    }

    /// Compose with side-by-side mode using default configuration
    fn beside<T: Mixable>(self, other: T) -> ComposedVisualization<Self, T> {
        ComposedVisualization::side_by_side(self, other, SideBySideConfig::default())
    }

    /// Compose with side-by-side mode using custom configuration
    fn beside_with_config<T: Mixable>(
        self,
        other: T,
        config: SideBySideConfig,
    ) -> ComposedVisualization<Self, T> {
        ComposedVisualization::side_by_side(self, other, config)
    }

    /// Compose with custom behavior
    fn custom_compose<T: Mixable>(
        self,
        other: T,
        behavior: Box<dyn CustomCompositionBehavior>,
    ) -> ComposedVisualization<Self, T> {
        ComposedVisualization::custom(self, other, behavior)
    }

    /// Compose with cross-fade behavior
    fn cross_fade<T: Mixable>(self, other: T, fade_factor: f32) -> ComposedVisualization<Self, T> {
        let behavior = Box::new(CrossFadeComposition { fade_factor });
        self.custom_compose(other, behavior)
    }
}
```

## Dependencies

### Prerequisite Stories

- GUP-001: Build Mixable Trait (provides basic composition framework)
- GUP-020: WebGPU Integration for RenderContext (provides GPU rendering capabilities)

### Enables Stories

- Enhanced visualization composition capabilities for all subsequent stories
- Rich composition patterns for Phase 2 high-level APIs

## Testing Strategy

### Composition Mode Tests

```rust
#[tokio::test]
async fn test_overlay_composition() {
    let mut context = RenderContext::new().await.unwrap();
    
    let background = create_test_visualization("background", [1.0, 0.0, 0.0, 0.5]);
    let foreground = create_test_visualization("foreground", [0.0, 1.0, 0.0, 0.7]);
    
    let composed = background.overlay(foreground);
    let result = composed.render(&mut context);
    
    assert!(result.is_ok());
    // Additional assertions would verify proper layering
}

#[tokio::test]
async fn test_side_by_side_composition() {
    let mut context = RenderContext::new().await.unwrap();
    
    let left = create_test_visualization("left", [1.0, 0.0, 0.0, 1.0]);
    let right = create_test_visualization("right", [0.0, 1.0, 0.0, 1.0]);
    
    let config = SideBySideConfig {
        direction: LayoutDirection::Horizontal,
        split_ratio: 0.3,
        padding: 20.0,
    };
    
    let composed = left.beside_with_config(right, config);
    let result = composed.render(&mut context);
    
    assert!(result.is_ok());
    // Additional assertions would verify viewport splitting
}

#[tokio::test]
async fn test_custom_composition() {
    let mut context = RenderContext::new().await.unwrap();
    
    let viz1 = create_test_visualization("viz1", [1.0, 0.0, 0.0, 1.0]);
    let viz2 = create_test_visualization("viz2", [0.0, 1.0, 0.0, 1.0]);
    
    let composed = viz1.cross_fade(viz2, 0.3);
    let result = composed.render(&mut context);
    
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_merge_composition() {
    // This test would require components with compatible data types
    // Implementation depends on specific visualization types
}
```

### Viewport Management Tests

```rust
#[test]
fn test_viewport_splitting() {
    let original = Viewport {
        width: 800,
        height: 600,
        scale_factor: 1.0,
    };
    
    let config = SideBySideConfig {
        direction: LayoutDirection::Horizontal,
        split_ratio: 0.6,
        padding: 10.0,
    };
    
    let composition = ComposedVisualization::side_by_side(
        create_mock_component(),
        create_mock_component(),
        config,
    );
    
    let (first_vp, second_vp) = composition.calculate_split_viewports(original);
    
    assert_eq!(first_vp.width, 475);  // 800 * 0.6 - 5 (half padding)
    assert_eq!(second_vp.width, 315); // 800 * 0.4 - 5 (half padding)
    assert_eq!(first_vp.height, 600);
    assert_eq!(second_vp.height, 600);
}
```

## Success Metrics

### Functional Requirements

- [ ] **Mode Differentiation**: Each composition mode produces visually distinct results
- [ ] **Viewport Management**: SideBySide mode correctly partitions screen space
- [ ] **Data Integration**: Merge mode successfully combines compatible data sources
- [ ] **Custom Framework**: Custom composition behaviors work as expected

### Performance Requirements

- [ ] **Rendering Performance**: Mode-specific rendering maintains 60fps for typical scenarios
- [ ] **Memory Efficiency**: Composition modes don't duplicate data unnecessarily
- [ ] **Mode Switching**: Changing composition modes has minimal performance impact

### Quality Requirements

- [ ] **Visual Correctness**: Compositions produce expected visual output
- [ ] **Error Handling**: Invalid compositions provide clear error messages
- [ ] **API Consistency**: All modes work through the same Mixable interface

## Risk Assessment

### Technical Risks

- **Medium**: Merge mode complexity may require significant data type analysis
- **Medium**: Viewport management could be complex with nested compositions
- **Low**: Custom composition framework might be over-engineered

### Mitigation Strategies

- **Incremental Implementation**: Start with simpler modes (Overlay, SideBySide) before tackling Merge
- **Clear Abstractions**: Design clear interfaces between composition logic and rendering
- **Extensive Testing**: Test all modes with various component combinations

## Implementation Notes

### Design Decisions

- Use enum for built-in composition modes with trait for custom behaviors
- Implement viewport management through render context rather than global state
- Defer complex data merging until specific use cases are identified
- Provide convenience methods while maintaining core trait simplicity

### Performance Considerations

- Minimize render state changes between components
- Cache viewport calculations for repeated compositions
- Use GPU-accelerated blending where available
- Optimize for common composition patterns

## Definition of Done

- [ ] All four composition modes (Overlay, Merge, SideBySide, Custom) implemented with distinct behaviors
- [ ] Viewport partitioning works correctly for SideBySide mode with configurable layouts
- [ ] Custom composition framework supports user-defined behaviors
- [ ] Overlay mode implements proper depth testing and alpha blending
- [ ] Merge mode provides basic data combination capabilities (implementation may be limited)
- [ ] API provides convenient methods for common composition patterns
- [ ] Comprehensive tests validate each composition mode's behavior
- [ ] Error handling provides clear diagnostics for invalid compositions
- [ ] Performance benchmarks confirm acceptable overhead for all modes
- [ ] Integration with WebGPU rendering context (depends on GUP-020)
- [ ] Code review completed and approved
- [ ] Documentation updated with composition mode examples and usage patterns

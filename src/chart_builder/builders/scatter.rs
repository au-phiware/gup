// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Scatter plot builder with Observable Plot compatibility.
//!
//! Provides fluent API for creating GPU-accelerated scatter plots with
//! automatic scale inference and shader function integration.

use super::{
    AccessorFunction, ConfigurableBuilder, GridCapableBuilder, apply_accessors_to_selection,
    validate_required_accessors,
};
use crate::Circle;
use crate::RenderContext;
use crate::chart_builder::{ChartBuilder, ChartBuilderError, ChartConfig, ComposedChart};
use crate::error::GupResult;
use crate::grid::{GridConfiguration, GridLineConfig};
use crate::selection::Selection;
use crate::shader_function::LinearScale;
use crate::{MaybeSend, MaybeSync};
use std::marker::PhantomData;
use std::sync::Arc;

/// Scatter plot builder providing Observable Plot-style API.
///
/// # Examples
///
/// ```rust,no_run
/// use gup::prelude::*;
/// use gup::chart_builder::accessor::AccessorValue;
/// use gup::chart_builder::builders::AccessorFunction;
///
/// #[derive(Debug, Clone)]
/// struct SalesPoint {
///     revenue: f32,
///     profit: f32,
///     region: String,
/// }
///
/// # async fn example() -> GupResult<()> {
/// # let context = std::sync::Arc::new(RenderContext::new().await?);
/// let sales_data = vec![
///     SalesPoint { revenue: 100.0, profit: 20.0, region: "North".to_string() },
///     SalesPoint { revenue: 200.0, profit: 45.0, region: "South".to_string() },
/// ];
///
/// // Observable Plot-style API  
/// let chart = scatter()
///     .x(AccessorFunction::new(|d: &SalesPoint| AccessorValue::Float(d.revenue)))
///     .y(AccessorFunction::new(|d: &SalesPoint| AccessorValue::Float(d.profit)))
///     .build_with_data(sales_data, context)?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct ScatterPlotBuilder<T> {
    pub(crate) x_accessor: Option<AccessorFunction<T>>,
    pub(crate) y_accessor: Option<AccessorFunction<T>>,
    pub(crate) color_accessor: Option<AccessorFunction<T>>,
    pub(crate) size_accessor: Option<AccessorFunction<T>>,
    pub(crate) opacity_accessor: Option<AccessorFunction<T>>,
    pub(crate) config: ChartConfig,
    pub(crate) _phantom: PhantomData<T>,
}

impl<T> ScatterPlotBuilder<T> {
    /// Create a new scatter plot builder.
    pub fn new() -> Self {
        Self {
            x_accessor: None,
            y_accessor: None,
            color_accessor: None,
            size_accessor: None,
            opacity_accessor: None,
            config: ChartConfig::default(),
            _phantom: PhantomData,
        }
    }

    /// Set the X-axis accessor function.
    pub fn x<A>(mut self, accessor: A) -> Self
    where
        A: Into<AccessorFunction<T>>,
    {
        self.x_accessor = Some(accessor.into());
        self
    }

    /// Set the Y-axis accessor function.
    pub fn y<A>(mut self, accessor: A) -> Self
    where
        A: Into<AccessorFunction<T>>,
    {
        self.y_accessor = Some(accessor.into());
        self
    }

    /// Set the color accessor function.
    pub fn color<A>(mut self, accessor: A) -> Self
    where
        A: Into<AccessorFunction<T>>,
    {
        self.color_accessor = Some(accessor.into());
        self
    }

    /// Set the size accessor function.
    pub fn size<A>(mut self, accessor: A) -> Self
    where
        A: Into<AccessorFunction<T>>,
    {
        self.size_accessor = Some(accessor.into());
        self
    }

    /// Set the opacity accessor function.
    pub fn opacity<A>(mut self, accessor: A) -> Self
    where
        A: Into<AccessorFunction<T>>,
    {
        self.opacity_accessor = Some(accessor.into());
        self
    }

    /// Set a fixed point size for all data points.
    pub fn point_size(mut self, size: f32) -> Self {
        use crate::chart_builder::accessor::AccessorValue;
        self.size_accessor = Some(AccessorFunction::new(move |_: &T| {
            AccessorValue::Float(size)
        }));
        self
    }

    /// Set a fixed color for all data points.
    pub fn fill_color(mut self, color: [f32; 4]) -> Self {
        use crate::chart_builder::accessor::AccessorValue;
        self.color_accessor = Some(AccessorFunction::new(move |_: &T| {
            AccessorValue::Color(color)
        }));
        self
    }

    /// Set a fixed opacity for all data points.
    pub fn fill_opacity(mut self, opacity: f32) -> Self {
        use crate::chart_builder::accessor::AccessorValue;
        self.opacity_accessor = Some(AccessorFunction::new(move |_: &T| {
            AccessorValue::Float(opacity.clamp(0.0, 1.0))
        }));
        self
    }

    /// Enable or disable anti-aliasing for point rendering.
    pub fn anti_alias(self, _enable: bool) -> Self {
        // Anti-aliasing configuration would be stored here
        // For now, this is a no-op as it would be handled in the shader pipeline
        self
    }

    /// Set point budget for very large datasets (performance optimization).
    pub fn point_budget(self, _max_points: usize) -> Self {
        // Point budget would trigger level-of-detail rendering for massive datasets
        // This is a future optimization feature
        self
    }

    /// Set the X-axis scale.
    ///
    /// The scale's domain is used to auto-configure axis tick generation and
    /// the `linear_scale` WGSL function is included in the shader pipeline.
    pub fn x_scale(mut self, scale: LinearScale) -> Self {
        self.config.x_scale = Some(scale);
        self
    }

    /// Set the Y-axis scale.
    ///
    /// The scale's domain is used to auto-configure axis tick generation and
    /// the `linear_scale` WGSL function is included in the shader pipeline.
    pub fn y_scale(mut self, scale: LinearScale) -> Self {
        self.config.y_scale = Some(scale);
        self
    }
}

impl<T> Default for ScatterPlotBuilder<T> {
    fn default() -> Self {
        Self::new()
    }
}

// Implement configurable builder methods
impl<T> ConfigurableBuilder for ScatterPlotBuilder<T> {
    fn title(mut self, title: impl Into<String>) -> Self {
        self.config.title_config = Some(crate::chart_builder::TitleConfig::new(title));
        self
    }

    fn width(mut self, width: f32) -> Self {
        self.config.width = width;
        self
    }

    fn height(mut self, height: f32) -> Self {
        self.config.height = height;
        self
    }

    fn background(mut self, color: [f32; 4]) -> Self {
        self.config.background_color = Some(color);
        self
    }

    fn show_axes(mut self, show: bool) -> Self {
        self.config.show_axes = show;
        self
    }

    fn show_grid(mut self, show: bool) -> Self {
        self.config.show_grid = show;
        self
    }

    fn hover_reveal(mut self, enabled: bool) -> Self {
        self.config.hover_reveal = enabled;
        self
    }

    fn tooltip_config(mut self, config: crate::text::hover_reveal::TooltipConfig) -> Self {
        self.config = self.config.with_tooltip_config(config);
        self
    }
}

// Implement advanced grid configuration methods
impl<T> GridCapableBuilder for ScatterPlotBuilder<T> {
    fn major_grid_style(mut self, config: GridLineConfig) -> Self {
        self.config.grid_config.major_grid = config;
        self
    }

    fn minor_grid_style(mut self, config: GridLineConfig) -> Self {
        self.config.grid_config.minor_grid = config;
        self
    }

    fn horizontal_grid_only(mut self) -> Self {
        self.config.grid_config.show_horizontal = true;
        self.config.grid_config.show_vertical = false;
        self.config.show_grid = true; // Enable grid display
        self
    }

    fn vertical_grid_only(mut self) -> Self {
        self.config.grid_config.show_horizontal = false;
        self.config.grid_config.show_vertical = true;
        self.config.show_grid = true; // Enable grid display
        self
    }

    fn with_minor_grid(mut self) -> Self {
        self.config.grid_config.minor_grid.enabled = true;
        self
    }

    fn without_minor_grid(mut self) -> Self {
        self.config.grid_config.minor_grid.enabled = false;
        self
    }

    fn grid_configuration(mut self, config: GridConfiguration) -> Self {
        self.config.grid_config = config;
        self
    }
}

impl<T> ChartBuilder<T> for ScatterPlotBuilder<T>
where
    T: Clone + MaybeSend + MaybeSync + std::fmt::Debug + 'static,
{
    type Output = ComposedChart<T, Circle>;

    fn build_with_data(self, data: Vec<T>, context: Arc<RenderContext>) -> GupResult<Self::Output> {
        // Validate required accessors
        validate_required_accessors(&self.x_accessor, &self.y_accessor)?;

        if data.is_empty() {
            return Err(ChartBuilderError::EmptyData.into());
        }

        // Create selection with Circle marks
        let mut selection = Selection::<T, Circle>::new(data, context)?;

        // Apply accessor functions to selection
        apply_accessors_to_selection(
            &mut selection,
            &self.x_accessor,
            &self.y_accessor,
            &self.color_accessor,
            &self.size_accessor,
        )?;

        // Apply opacity if specified
        if self.opacity_accessor.is_some() {
            // Opacity would be applied as a shader function
            // For now, this is noted as a future feature
        }

        // Create composed chart with axes based on configuration
        let composed_chart = ComposedChart::new(selection, self.config).with_default_axes();

        Ok(composed_chart)
    }
}

/// Convenience function to create a new scatter plot builder.
///
/// # Examples
///
/// ```rust,no_run
/// use gup::prelude::*;
/// use gup::chart_builder::accessor::AccessorValue;
/// use gup::chart_builder::builders::AccessorFunction;
///
/// #[derive(Debug, Clone)]
/// struct DataPoint { x: f32, y: f32 }
///
/// # async fn example() -> GupResult<()> {
/// # let context = std::sync::Arc::new(RenderContext::new().await?);
/// # let data = vec![DataPoint { x: 1.0, y: 2.0 }];
/// let chart = scatter()
///     .x(AccessorFunction::new(|d: &DataPoint| AccessorValue::Float(d.x)))
///     .y(AccessorFunction::new(|d: &DataPoint| AccessorValue::Float(d.y)))
///     .build_with_data(data, context)?;
/// # Ok(())
/// # }
/// ```
pub fn scatter<T>() -> ScatterPlotBuilder<T> {
    ScatterPlotBuilder::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RenderContext;
    use crate::chart_builder::accessor::{AccessorValue, x, y};

    #[derive(Debug, Clone)]
    struct TestPoint {
        x_val: f32,
        y_val: f32,
        size: f32,
        category: String,
    }

    #[tokio::test]
    async fn test_scatter_plot_builder_basic() {
        let data = vec![
            TestPoint {
                x_val: 1.0,
                y_val: 2.0,
                size: 5.0,
                category: "A".to_string(),
            },
            TestPoint {
                x_val: 3.0,
                y_val: 4.0,
                size: 10.0,
                category: "B".to_string(),
            },
        ];

        let context = Arc::new(RenderContext::new().await.unwrap());

        let builder = scatter()
            .x(AccessorFunction::new(|d: &TestPoint| {
                AccessorValue::Float(d.x_val)
            }))
            .y(AccessorFunction::new(|d: &TestPoint| {
                AccessorValue::Float(d.y_val)
            }))
            .title("Test Scatter Plot")
            .width(600.0)
            .height(400.0);

        let result = builder.build_with_data(data, context);
        assert!(result.is_ok());

        let selection = result.unwrap();
        assert_eq!(selection.len(), 2);
        assert!(!selection.is_empty());
    }

    #[tokio::test]
    async fn test_scatter_plot_builder_field_accessors() {
        let data = vec![TestPoint {
            x_val: 5.0,
            y_val: 10.0,
            size: 3.0,
            category: "Test".to_string(),
        }];

        let context = Arc::new(RenderContext::new().await.unwrap());

        let builder = scatter()
            .x(x("x_val"))
            .y(y("y_val"))
            .point_size(8.0)
            .fill_color([1.0, 0.0, 0.0, 1.0]);

        let result = builder.build_with_data(data, context);
        assert!(result.is_ok());
    }

    #[test]
    fn test_scatter_plot_builder_configuration() {
        let builder = scatter::<TestPoint>()
            .title("My Scatter Plot")
            .width(1000.0)
            .height(800.0)
            .background([0.9, 0.9, 0.9, 1.0])
            .show_axes(true)
            .show_grid(false);

        assert_eq!(builder.config.title(), Some("My Scatter Plot"));
        assert_eq!(builder.config.width, 1000.0);
        assert_eq!(builder.config.height, 800.0);
        assert_eq!(builder.config.background_color, Some([0.9, 0.9, 0.9, 1.0]));
        assert!(builder.config.show_axes);
        assert!(!builder.config.show_grid);
    }

    #[tokio::test]
    async fn test_scatter_plot_builder_validation_errors() {
        let data = vec![TestPoint {
            x_val: 1.0,
            y_val: 2.0,
            size: 5.0,
            category: "A".to_string(),
        }];

        let context = Arc::new(RenderContext::new().await.unwrap());

        // Missing Y accessor should fail
        let builder = scatter().x(x("x_val"));
        let result = builder.build_with_data(data.clone(), context.clone());
        assert!(result.is_err());
        let error_str = format!("{:?}", result.unwrap_err());
        assert!(error_str.contains("Missing required accessor"));

        // Missing X accessor should fail
        let builder = scatter().y(y("y_val"));
        let result = builder.build_with_data(data.clone(), context.clone());
        assert!(result.is_err());

        // Empty data should fail
        let empty_data: Vec<TestPoint> = vec![];
        let builder = scatter().x(x("x_val")).y(y("y_val"));
        let result = builder.build_with_data(empty_data, context);
        assert!(result.is_err());
        let error_str = format!("{:?}", result.unwrap_err());
        assert!(error_str.contains("no data provided") || error_str.contains("EmptyData"));
    }

    #[test]
    fn test_scatter_plot_builder_accessor_chaining() {
        let builder = scatter::<TestPoint>()
            .x(AccessorFunction::new(|d: &TestPoint| {
                AccessorValue::Float(d.x_val)
            }))
            .y(AccessorFunction::new(|d: &TestPoint| {
                AccessorValue::Float(d.y_val)
            }))
            .color(AccessorFunction::new(|d: &TestPoint| {
                if d.category == "A" {
                    AccessorValue::Color([1.0, 0.0, 0.0, 1.0])
                } else {
                    AccessorValue::Color([0.0, 0.0, 1.0, 1.0])
                }
            }))
            .size(AccessorFunction::new(|d: &TestPoint| {
                AccessorValue::Float(d.size)
            }))
            .opacity(AccessorFunction::new(|_: &TestPoint| {
                AccessorValue::Float(0.8)
            }));

        assert!(builder.x_accessor.is_some());
        assert!(builder.y_accessor.is_some());
        assert!(builder.color_accessor.is_some());
        assert!(builder.size_accessor.is_some());
        assert!(builder.opacity_accessor.is_some());
    }

    #[test]
    fn test_scatter_plot_convenience_functions() {
        let builder = scatter::<TestPoint>()
            .point_size(15.0)
            .fill_color([0.5, 0.8, 0.2, 1.0])
            .fill_opacity(0.6);

        // Check that convenience functions set the appropriate accessors
        assert!(builder.size_accessor.is_some());
        assert!(builder.color_accessor.is_some());
        assert!(builder.opacity_accessor.is_some());

        // Test the accessor functions work correctly
        let test_point = TestPoint {
            x_val: 0.0,
            y_val: 0.0,
            size: 0.0,
            category: "test".to_string(),
        };

        if let Some(size_acc) = &builder.size_accessor {
            let size_value = size_acc.apply(&test_point);
            assert_eq!(size_value, AccessorValue::Float(15.0));
        }

        if let Some(color_acc) = &builder.color_accessor {
            let color_value = color_acc.apply(&test_point);
            assert_eq!(color_value, AccessorValue::Color([0.5, 0.8, 0.2, 1.0]));
        }

        if let Some(opacity_acc) = &builder.opacity_accessor {
            let opacity_value = opacity_acc.apply(&test_point);
            assert_eq!(opacity_value, AccessorValue::Float(0.6));
        }
    }

    #[test]
    fn test_scatter_plot_builder_default() {
        let builder = ScatterPlotBuilder::<TestPoint>::default();
        assert!(builder.x_accessor.is_none());
        assert!(builder.y_accessor.is_none());
        assert!(builder.color_accessor.is_none());
        assert!(builder.size_accessor.is_none());
        assert!(builder.opacity_accessor.is_none());
        assert_eq!(builder.config.width, 800.0); // Default config values
        assert_eq!(builder.config.height, 600.0);
    }

    #[test]
    fn test_scatter_plot_builder_feature_flags() {
        let builder = scatter::<TestPoint>()
            .anti_alias(true)
            .point_budget(100_000);

        // These are future feature flags that don't currently change behavior
        // but should compile and not panic
        assert!(builder.x_accessor.is_none()); // Basic sanity check
    }

    // Tests for enhanced grid API (GUP-097)
    #[test]
    fn test_scatter_plot_enhanced_grid_api() {
        // Test simple grid enabling
        let builder = scatter::<TestPoint>().grid();
        assert!(builder.config.show_grid);

        // Test theme presets
        let light_builder = scatter::<TestPoint>().light_grid();
        assert!(light_builder.config.show_grid);
        assert_eq!(light_builder.config.grid_config.major_grid.color[0], 0.0); // Light theme black

        let dark_builder = scatter::<TestPoint>().dark_grid();
        assert!(dark_builder.config.show_grid);
        assert_eq!(dark_builder.config.grid_config.major_grid.color[0], 1.0); // Dark theme white

        let scientific_builder = scatter::<TestPoint>().scientific_grid();
        assert!(scientific_builder.config.show_grid);
        assert!(scientific_builder.config.grid_config.minor_grid.enabled); // Scientific has minor grids

        let business_builder = scatter::<TestPoint>().business_grid();
        assert!(business_builder.config.show_grid);
        assert!(!business_builder.config.grid_config.show_vertical); // Business typically horizontal only

        let minimal_builder = scatter::<TestPoint>().minimal_grid();
        assert!(minimal_builder.config.show_grid);
        assert_eq!(
            minimal_builder.config.grid_config.major_grid.line_width,
            0.25
        ); // Thin lines

        let high_contrast_builder = scatter::<TestPoint>().high_contrast_grid();
        assert!(high_contrast_builder.config.show_grid);
        assert_eq!(
            high_contrast_builder
                .config
                .grid_config
                .major_grid
                .line_width,
            1.0
        ); // Thick lines
    }

    #[test]
    fn test_scatter_plot_grid_styling_shortcuts() {
        use crate::grid::Color;

        // Test grid color with hex string
        let builder = scatter::<TestPoint>().grid_color("#ff6b6b");
        assert!(builder.config.show_grid);
        let red_component = builder.config.grid_config.major_grid.color[0];
        assert!((red_component - 1.0).abs() < 0.01); // Should be close to 1.0 (red)

        // Test grid color with Color struct
        let color = Color::new(0.3, 0.6, 0.9, 1.0);
        let builder = scatter::<TestPoint>().grid_color(color);
        assert_eq!(builder.config.grid_config.major_grid.color[0], 0.3);
        assert_eq!(builder.config.grid_config.major_grid.color[1], 0.6);
        assert_eq!(builder.config.grid_config.major_grid.color[2], 0.9);

        // Test grid opacity
        let builder = scatter::<TestPoint>().grid_opacity(0.5);
        assert!(builder.config.show_grid);
        assert_eq!(builder.config.grid_config.major_grid.opacity, 0.5);

        // Test grid width
        let builder = scatter::<TestPoint>().grid_width(2.0);
        assert!(builder.config.show_grid);
        assert_eq!(builder.config.grid_config.major_grid.line_width, 2.0);
    }

    #[test]
    fn test_scatter_plot_directional_grid_shortcuts() {
        // Test horizontal grid
        let builder = scatter::<TestPoint>().horizontal_grid();
        assert!(builder.config.show_grid);
        assert!(builder.config.grid_config.show_horizontal);
        assert!(!builder.config.grid_config.show_vertical);

        // Test vertical grid
        let builder = scatter::<TestPoint>().vertical_grid();
        assert!(builder.config.show_grid);
        assert!(!builder.config.grid_config.show_horizontal);
        assert!(builder.config.grid_config.show_vertical);
    }

    #[test]
    fn test_scatter_plot_grid_api_chaining() {
        // Test that enhanced API methods can be chained
        let builder = scatter::<TestPoint>()
            .x(AccessorFunction::new(|d: &TestPoint| {
                AccessorValue::Float(d.x_val)
            }))
            .y(AccessorFunction::new(|d: &TestPoint| {
                AccessorValue::Float(d.y_val)
            }))
            .light_grid()
            .point_size(10.0);

        assert!(builder.x_accessor.is_some());
        assert!(builder.y_accessor.is_some());
        assert!(builder.size_accessor.is_some());
        assert!(builder.config.show_grid);

        // Test individual color setting separately to avoid chaining overrides
        let color_builder = scatter::<TestPoint>().grid_color("#336699");
        let blue_component = color_builder.config.grid_config.major_grid.color[2];
        assert!((blue_component - 0.6).abs() < 0.01); // Should be close to 0.6 (blue from #336699)
    }

    #[test]
    fn test_scatter_plot_grid_backwards_compatibility() {
        // Test that existing grid methods still work
        let builder = scatter::<TestPoint>()
            .show_grid(true)
            .horizontal_grid_only()
            .with_minor_grid();

        assert!(builder.config.show_grid);
        assert!(builder.config.grid_config.show_horizontal);
        assert!(!builder.config.grid_config.show_vertical);
        assert!(builder.config.grid_config.minor_grid.enabled);
    }
}

// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Observable Plot-style chart builders for GPU-accelerated visualizations.
//!
//! This module provides high-level, fluent APIs for creating common chart types
//! while maintaining full GPU performance and seamless interoperability with
//! the low-level Selection system.
//!
//! # Key Features
//!
//! * **One-line chart creation** with Observable Plot compatibility
//! * **Type-safe accessor functions** with compile-time validation
//! * **Zero-cost abstractions** over Phase 1 Selection primitives
//! * **Seamless conversion** to low-level APIs for advanced customization
//!
//! # Examples
//!
//! ```rust,no_run
//! use gup::prelude::*;
//!
//! #[derive(Debug, Clone)]
//! struct SalesPoint {
//!     revenue: f32,
//!     profit: f32,
//!     region: String,
//! }
//!
//! # async fn example() -> GupResult<()> {
//! let sales_data = vec![
//!     SalesPoint { revenue: 100.0, profit: 20.0, region: "North".to_string() },
//!     SalesPoint { revenue: 200.0, profit: 45.0, region: "South".to_string() },
//! ];
//!
//! // Observable Plot-style API
//! let chart = gup::plot()
//!     .data(sales_data)
//!     .scatter(x("revenue"), y("profit"))
//!     .color(color("region"));
//! # Ok(())
//! # }
//! ```

pub mod accessor;
pub mod builders;
pub mod labels;
pub mod plot_api;

pub use accessor::*;
pub use builders::*;
pub use labels::*;
pub use plot_api::*;

use crate::RenderContext;
use crate::axis::{Axis, AxisBounds, AxisConfiguration, AxisPosition, LinearAxis};
use crate::error::{GupError, GupResult};
use crate::grid::GridConfiguration;
use crate::selection::Selection;
use crate::shader_function::Vec2;
use std::marker::PhantomData;
use std::sync::Arc;

/// Core trait for all chart builders providing fluent API construction.
///
/// This trait enables type-safe chart building with compile-time validation
/// and automatic conversion to GPU-accelerated Selection instances.
///
/// # Type Parameters
///
/// * `T` - The data type for chart elements
/// * `Output` - The rendered chart type (typically a Selection)
///
/// # Design Philosophy
///
/// Chart builders are **zero-cost abstractions** that compile down to
/// efficient Selection operations. The fluent API provides Observable Plot
/// compatibility while maintaining full GPU performance.
pub trait ChartBuilder<T>: Sized {
    /// The output type after building (typically Selection<T, M>)
    type Output;

    /// Build the chart with the provided data and context.
    ///
    /// This method transforms the high-level chart configuration into
    /// GPU-accelerated Selection primitives.
    fn build_with_data(self, data: Vec<T>, context: Arc<RenderContext>) -> GupResult<Self::Output>;

    /// Convenience method to build and render in one step.
    fn render_with_data(self, data: Vec<T>, context: Arc<RenderContext>) -> GupResult<Self::Output>
    where
        Self::Output: crate::Mixable,
    {
        let output = self.build_with_data(data, context)?;
        // The render method is called during the mixable render process
        Ok(output)
    }
}

/// A chart builder that has been bound to specific data.
///
/// This intermediate type enables method chaining while preserving type
/// information about the data and chart configuration.
pub struct BoundChartBuilder<B, T>
where
    B: ChartBuilder<T>,
{
    pub(crate) builder: B,
    pub(crate) data: Vec<T>,
    pub(crate) context: Arc<RenderContext>,
    pub(crate) _phantom: PhantomData<T>,
}

impl<B, T> BoundChartBuilder<B, T>
where
    B: ChartBuilder<T>,
{
    /// Create a new bound chart builder.
    pub fn new(builder: B, data: Vec<T>, context: Arc<RenderContext>) -> Self {
        Self {
            builder,
            data,
            context,
            _phantom: PhantomData,
        }
    }

    /// Build the chart using the bound data and context.
    pub fn build(self) -> GupResult<B::Output> {
        self.builder.build_with_data(self.data, self.context)
    }

    /// Build and render the chart in one step.
    pub fn render(self) -> GupResult<B::Output>
    where
        B::Output: crate::Mixable,
    {
        self.builder.render_with_data(self.data, self.context)
    }

    /// Convert this builder to a low-level Selection for advanced customization.
    ///
    /// This enables seamless transition from high-level builder APIs to
    /// low-level Selection operations when needed.
    pub fn into_selection<M>(self) -> GupResult<Selection<T, M>>
    where
        T: Clone + Send + Sync + std::fmt::Debug + 'static,
        M: crate::selection::Mark,
        M::AttributeValue: Default + Clone,
    {
        Selection::new(self.data, self.context)
    }

    /// Access the underlying data for inspection.
    pub fn data(&self) -> &[T] {
        &self.data
    }

    /// Get the number of data points.
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Check if the dataset is empty.
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

/// Chart configuration that applies to all chart types.
#[derive(Debug, Clone)]
pub struct ChartConfig {
    /// Chart title (optional)
    pub title: Option<String>,

    /// Chart width in pixels
    pub width: f32,

    /// Chart height in pixels
    pub height: f32,

    /// Chart margins
    pub margins: Margins,

    /// Background color (optional)
    pub background_color: Option<[f32; 4]>,

    /// Whether to show axes
    pub show_axes: bool,

    /// Whether to show grid lines
    pub show_grid: bool,

    /// Grid system configuration
    pub grid_config: GridConfiguration,
}

/// Chart margin specification.
#[derive(Debug, Clone, Copy)]
pub struct Margins {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl Default for ChartConfig {
    fn default() -> Self {
        Self {
            title: None,
            width: 800.0,
            height: 600.0,
            margins: Margins::default(),
            background_color: None,
            show_axes: true,
            show_grid: false,
            grid_config: GridConfiguration::default(),
        }
    }
}

impl Default for Margins {
    fn default() -> Self {
        Self {
            top: 40.0,
            right: 40.0,
            bottom: 60.0,
            left: 60.0,
        }
    }
}

impl Margins {
    /// Create uniform margins on all sides.
    pub fn uniform(margin: f32) -> Self {
        Self {
            top: margin,
            right: margin,
            bottom: margin,
            left: margin,
        }
    }

    /// Create symmetric margins (top/bottom and left/right).
    pub fn symmetric(vertical: f32, horizontal: f32) -> Self {
        Self {
            top: vertical,
            right: horizontal,
            bottom: vertical,
            left: horizontal,
        }
    }
}

/// A composed chart that includes both data visualization and axis rendering.
///
/// This struct represents a complete chart with both the main visualization
/// (e.g., scatter plot, line chart) and optional axis components.
#[derive(Debug)]
pub struct ComposedChart<T, M>
where
    T: Clone + Send + Sync + std::fmt::Debug + 'static,
    M: crate::selection::Mark,
{
    /// The main data visualization
    pub visualization: Selection<T, M>,
    /// Bottom axis (X-axis)
    pub bottom_axis: Option<Box<dyn Axis>>,
    /// Left axis (Y-axis)
    pub left_axis: Option<Box<dyn Axis>>,
    /// Top axis (secondary X-axis)
    pub top_axis: Option<Box<dyn Axis>>,
    /// Right axis (secondary Y-axis)
    pub right_axis: Option<Box<dyn Axis>>,
    /// Chart configuration
    pub config: ChartConfig,
}

impl<T, M> ComposedChart<T, M>
where
    T: Clone + Send + Sync + std::fmt::Debug + 'static,
    M: crate::selection::Mark,
{
    /// Create a new composed chart from a visualization and configuration.
    pub fn new(visualization: Selection<T, M>, config: ChartConfig) -> Self {
        Self {
            visualization,
            bottom_axis: None,
            left_axis: None,
            top_axis: None,
            right_axis: None,
            config,
        }
    }

    /// Add axes to the chart based on configuration.
    pub fn with_default_axes(mut self) -> Self {
        if self.config.show_axes {
            let axis_config = AxisConfiguration::default();

            // Add bottom axis (X-axis)
            self.bottom_axis = Some(Box::new(LinearAxis::new(
                AxisPosition::Bottom,
                axis_config.clone(),
            )));

            // Add left axis (Y-axis)
            self.left_axis = Some(Box::new(LinearAxis::new(AxisPosition::Left, axis_config)));
        }
        self
    }

    /// Add a custom bottom axis.
    pub fn with_bottom_axis(mut self, axis: Box<dyn Axis>) -> Self {
        self.bottom_axis = Some(axis);
        self
    }

    /// Add a custom left axis.
    pub fn with_left_axis(mut self, axis: Box<dyn Axis>) -> Self {
        self.left_axis = Some(axis);
        self
    }

    /// Add a custom top axis.
    pub fn with_top_axis(mut self, axis: Box<dyn Axis>) -> Self {
        self.top_axis = Some(axis);
        self
    }

    /// Add a custom right axis.
    pub fn with_right_axis(mut self, axis: Box<dyn Axis>) -> Self {
        self.right_axis = Some(axis);
        self
    }

    /// Get the number of data elements in the visualization.
    pub fn len(&self) -> usize {
        self.visualization.data().len()
    }

    /// Check if the visualization has no data elements.
    pub fn is_empty(&self) -> bool {
        self.visualization.data().is_empty()
    }

    /// Render the complete chart including axes.
    pub fn render(&mut self, context: &mut RenderContext) -> GupResult<()> {
        // Calculate chart area based on margins and axis requirements
        let chart_area = self.calculate_chart_area();

        // Render axes if present
        if let Some(axis) = &self.bottom_axis {
            let bounds = self.calculate_axis_bounds(AxisPosition::Bottom, &chart_area);
            axis.render(context, bounds)?;
        }

        if let Some(axis) = &self.left_axis {
            let bounds = self.calculate_axis_bounds(AxisPosition::Left, &chart_area);
            axis.render(context, bounds)?;
        }

        if let Some(axis) = &self.top_axis {
            let bounds = self.calculate_axis_bounds(AxisPosition::Top, &chart_area);
            axis.render(context, bounds)?;
        }

        if let Some(axis) = &self.right_axis {
            let bounds = self.calculate_axis_bounds(AxisPosition::Right, &chart_area);
            axis.render(context, bounds)?;
        }

        // Render the main visualization
        // Note: In a complete implementation, this would use the Mixable render system
        // For now, we acknowledge that the visualization is prepared for rendering
        Ok(())
    }

    /// Calculate the available chart area after accounting for margins and axes.
    fn calculate_chart_area(&self) -> ChartArea {
        let total_width = self.config.width;
        let total_height = self.config.height;

        let mut margins = self.config.margins;

        // Adjust margins for axes
        if let Some(axis) = &self.bottom_axis {
            margins.bottom += axis.calculate_margin(None);
        }
        if let Some(axis) = &self.left_axis {
            margins.left += axis.calculate_margin(None);
        }
        if let Some(axis) = &self.top_axis {
            margins.top += axis.calculate_margin(None);
        }
        if let Some(axis) = &self.right_axis {
            margins.right += axis.calculate_margin(None);
        }

        ChartArea {
            x: margins.left,
            y: margins.top,
            width: total_width - margins.left - margins.right,
            height: total_height - margins.top - margins.bottom,
            margins,
        }
    }

    /// Calculate axis bounds for a specific position.
    fn calculate_axis_bounds(&self, position: AxisPosition, chart_area: &ChartArea) -> AxisBounds {
        match position {
            AxisPosition::Bottom => AxisBounds::new(
                Vec2 {
                    x: chart_area.x,
                    y: chart_area.y + chart_area.height,
                },
                Vec2 {
                    x: chart_area.x + chart_area.width,
                    y: chart_area.y + chart_area.height,
                },
                chart_area.margins.bottom,
            ),
            AxisPosition::Left => AxisBounds::new(
                Vec2 {
                    x: chart_area.x,
                    y: chart_area.y + chart_area.height,
                },
                Vec2 {
                    x: chart_area.x,
                    y: chart_area.y,
                },
                chart_area.margins.left,
            ),
            AxisPosition::Top => AxisBounds::new(
                Vec2 {
                    x: chart_area.x,
                    y: chart_area.y,
                },
                Vec2 {
                    x: chart_area.x + chart_area.width,
                    y: chart_area.y,
                },
                chart_area.margins.top,
            ),
            AxisPosition::Right => AxisBounds::new(
                Vec2 {
                    x: chart_area.x + chart_area.width,
                    y: chart_area.y + chart_area.height,
                },
                Vec2 {
                    x: chart_area.x + chart_area.width,
                    y: chart_area.y,
                },
                chart_area.margins.right,
            ),
        }
    }
}

/// Calculated chart area after accounting for margins and axes.
#[derive(Debug, Clone)]
struct ChartArea {
    /// X position of chart area
    pub x: f32,
    /// Y position of chart area
    pub y: f32,
    /// Width of chart area
    pub width: f32,
    /// Height of chart area
    pub height: f32,
    /// Final margins used
    pub margins: Margins,
}

/// Error types specific to chart building operations.
#[derive(Debug, Clone)]
pub enum ChartBuilderError {
    /// No data provided for chart creation
    EmptyData,

    /// Required accessor function not provided
    MissingAccessor { attribute: String },

    /// Incompatible accessor function type
    IncompatibleAccessor {
        attribute: String,
        expected_type: String,
        actual_type: String,
    },

    /// Chart configuration error
    ConfigurationError { message: String },

    /// Render context initialization failed
    ContextError { source: GupError },
}

impl std::fmt::Display for ChartBuilderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChartBuilderError::EmptyData => {
                write!(f, "Cannot create chart: no data provided")
            }
            ChartBuilderError::MissingAccessor { attribute } => {
                write!(
                    f,
                    "Missing required accessor function for attribute: {attribute}"
                )
            }
            ChartBuilderError::IncompatibleAccessor {
                attribute,
                expected_type,
                actual_type,
            } => {
                write!(
                    f,
                    "Incompatible accessor for {attribute}: expected {expected_type}, got {actual_type}"
                )
            }
            ChartBuilderError::ConfigurationError { message } => {
                write!(f, "Chart configuration error: {message}")
            }
            ChartBuilderError::ContextError { source } => {
                write!(f, "Render context error: {source}")
            }
        }
    }
}

impl std::error::Error for ChartBuilderError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ChartBuilderError::ContextError { source } => Some(source),
            _ => None,
        }
    }
}

impl From<ChartBuilderError> for GupError {
    fn from(err: ChartBuilderError) -> Self {
        GupError::validation_error(format!("Chart builder error: {err}"))
    }
}

/// Helper trait to convert various accessor types to internal representations.
pub trait IntoAccessor<T> {
    /// The output type of the accessor function
    type Output;

    /// Convert to an internal accessor function representation
    #[allow(clippy::type_complexity)]
    fn into_accessor_fn(self) -> Box<dyn Fn(&T) -> Self::Output + Send + Sync>;
}

// Implementation for closure-based accessors
impl<T, F, Output> IntoAccessor<T> for F
where
    F: Fn(&T) -> Output + Send + Sync + 'static,
    Output: Send + Sync + 'static,
{
    type Output = Output;

    fn into_accessor_fn(self) -> Box<dyn Fn(&T) -> Self::Output + Send + Sync> {
        Box::new(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RenderContext;

    #[derive(Debug, Clone)]
    #[allow(dead_code)]
    struct TestData {
        x: f32,
        y: f32,
        value: f32,
        category: String,
    }

    #[test]
    fn test_chart_config_defaults() {
        let config = ChartConfig::default();
        assert_eq!(config.width, 800.0);
        assert_eq!(config.height, 600.0);
        assert!(config.show_axes);
        assert!(!config.show_grid);
        assert!(config.title.is_none());
        assert!(config.background_color.is_none());
    }

    #[test]
    fn test_margins() {
        let uniform_margins = Margins::uniform(20.0);
        assert_eq!(uniform_margins.top, 20.0);
        assert_eq!(uniform_margins.right, 20.0);
        assert_eq!(uniform_margins.bottom, 20.0);
        assert_eq!(uniform_margins.left, 20.0);

        let symmetric_margins = Margins::symmetric(30.0, 40.0);
        assert_eq!(symmetric_margins.top, 30.0);
        assert_eq!(symmetric_margins.right, 40.0);
        assert_eq!(symmetric_margins.bottom, 30.0);
        assert_eq!(symmetric_margins.left, 40.0);
    }

    #[test]
    fn test_chart_builder_error_display() {
        let error = ChartBuilderError::EmptyData;
        assert_eq!(format!("{error}"), "Cannot create chart: no data provided");

        let error = ChartBuilderError::MissingAccessor {
            attribute: "position".to_string(),
        };
        assert!(format!("{error}").contains("position"));

        let error = ChartBuilderError::IncompatibleAccessor {
            attribute: "color".to_string(),
            expected_type: "Color".to_string(),
            actual_type: "f32".to_string(),
        };
        let error_str = format!("{error}");
        assert!(error_str.contains("color"));
        assert!(error_str.contains("Color"));
        assert!(error_str.contains("f32"));
    }

    #[tokio::test]
    async fn test_bound_chart_builder_data_access() {
        let data = vec![
            TestData {
                x: 1.0,
                y: 2.0,
                value: 10.0,
                category: "A".to_string(),
            },
            TestData {
                x: 3.0,
                y: 4.0,
                value: 20.0,
                category: "B".to_string(),
            },
        ];

        let context = Arc::new(RenderContext::new().await.unwrap());

        // Create a mock builder for testing
        struct MockBuilder;
        impl ChartBuilder<TestData> for MockBuilder {
            type Output = ();

            fn build_with_data(
                self,
                _data: Vec<TestData>,
                _context: Arc<RenderContext>,
            ) -> GupResult<Self::Output> {
                Ok(())
            }
        }

        let bound_builder = BoundChartBuilder::new(MockBuilder, data.clone(), context);

        assert_eq!(bound_builder.len(), 2);
        assert!(!bound_builder.is_empty());
        assert_eq!(bound_builder.data().len(), 2);
        assert_eq!(bound_builder.data()[0].x, 1.0);
        assert_eq!(bound_builder.data()[1].category, "B");
    }

    #[test]
    fn test_into_accessor_trait() {
        let data = TestData {
            x: 5.0,
            y: 10.0,
            value: 15.0,
            category: "Test".to_string(),
        };

        // Test closure-based accessor
        let accessor: Box<dyn Fn(&TestData) -> f32 + Send + Sync> =
            (|d: &TestData| d.x).into_accessor_fn();
        assert_eq!(accessor(&data), 5.0);

        let string_accessor: Box<dyn Fn(&TestData) -> String + Send + Sync> =
            (|d: &TestData| d.category.clone()).into_accessor_fn();
        assert_eq!(string_accessor(&data), "Test");
    }
}

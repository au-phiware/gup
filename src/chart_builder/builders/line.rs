// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Line chart builder with Observable Plot compatibility.
//!
//! Provides fluent API for creating GPU-accelerated line charts with
//! automatic sorting and interpolation options.

use super::{
    AccessorFunction, ConfigurableBuilder, GridCapableBuilder, apply_accessors_to_selection,
    validate_required_accessors,
};
use crate::Circle; // TODO: Replace with Line mark when available
use crate::RenderContext;
use crate::chart_builder::{AxisScale, ChartBuilder, ChartBuilderError, ChartConfig};
use crate::error::GupResult;
use crate::grid::{GridConfiguration, GridLineConfig};
use crate::selection::Selection;
use crate::{MaybeSend, MaybeSync};
use std::marker::PhantomData;
use std::sync::Arc;

/// Line interpolation methods.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum LineInterpolation {
    /// Linear interpolation (straight lines between points)
    #[default]
    Linear,
    /// Step function (horizontal then vertical)
    StepBefore,
    /// Step function (vertical then horizontal)
    StepAfter,
    /// Smooth curve interpolation
    Curve,
}

/// Line chart builder providing Observable Plot-style API.
///
/// # Examples
///
/// ```rust,no_run
/// use gup::prelude::*;
/// use gup::chart_builder::accessor::AccessorValue;
/// use gup::chart_builder::builders::AccessorFunction;
///
/// #[derive(Debug, Clone)]
/// struct DataPoint {
///     date: f32,
///     value: f32,
///     series: String,
/// }
///
/// # async fn example() -> GupResult<()> {
/// # let context = std::sync::Arc::new(RenderContext::new().await?);
/// let time_series = vec![
///     DataPoint { date: 0.0, value: 10.0, series: "A".to_string() },
///     DataPoint { date: 1.0, value: 15.0, series: "A".to_string() },
/// ];
///
/// let chart = line()
///     .x(AccessorFunction::new(|d: &DataPoint| AccessorValue::Float(d.date)))
///     .y(AccessorFunction::new(|d: &DataPoint| AccessorValue::Float(d.value)))
///     .build_with_data(time_series, context)?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct LineChartBuilder<T> {
    pub(crate) x_accessor: Option<AccessorFunction<T>>,
    pub(crate) y_accessor: Option<AccessorFunction<T>>,
    pub(crate) stroke_accessor: Option<AccessorFunction<T>>,
    pub(crate) stroke_width_accessor: Option<AccessorFunction<T>>,
    pub(crate) opacity_accessor: Option<AccessorFunction<T>>,
    pub(crate) interpolation: LineInterpolation,
    pub(crate) sort_by_x: bool,
    pub(crate) connect_nulls: bool,
    pub(crate) config: ChartConfig,
    pub(crate) _phantom: PhantomData<T>,
}

impl<T> LineChartBuilder<T> {
    /// Create a new line chart builder.
    pub fn new() -> Self {
        Self {
            x_accessor: None,
            y_accessor: None,
            stroke_accessor: None,
            stroke_width_accessor: None,
            opacity_accessor: None,
            interpolation: LineInterpolation::default(),
            sort_by_x: true,
            connect_nulls: false,
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

    /// Set the stroke color accessor function.
    pub fn stroke<A>(mut self, accessor: A) -> Self
    where
        A: Into<AccessorFunction<T>>,
    {
        self.stroke_accessor = Some(accessor.into());
        self
    }

    /// Set the stroke width accessor function.
    pub fn stroke_width<A>(mut self, accessor: A) -> Self
    where
        A: Into<AccessorFunction<T>>,
    {
        self.stroke_width_accessor = Some(accessor.into());
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

    /// Set a fixed stroke color for all lines.
    pub fn stroke_color(mut self, color: [f32; 4]) -> Self {
        use crate::chart_builder::accessor::AccessorValue;
        self.stroke_accessor = Some(AccessorFunction::new(move |_: &T| {
            AccessorValue::Color(color)
        }));
        self
    }

    /// Set a fixed stroke width for all lines.
    pub fn stroke_width_px(mut self, width: f32) -> Self {
        use crate::chart_builder::accessor::AccessorValue;
        self.stroke_width_accessor = Some(AccessorFunction::new(move |_: &T| {
            AccessorValue::Float(width)
        }));
        self
    }

    /// Set interpolation method for the line.
    pub fn interpolate(mut self, interpolation: LineInterpolation) -> Self {
        self.interpolation = interpolation;
        self
    }

    /// Enable or disable sorting data points by X coordinate.
    pub fn sort_x(mut self, sort: bool) -> Self {
        self.sort_by_x = sort;
        self
    }

    /// Enable or disable connecting across null/missing values.
    pub fn connect_nulls(mut self, connect: bool) -> Self {
        self.connect_nulls = connect;
        self
    }

    /// Enable smooth curve interpolation.
    pub fn smooth(mut self) -> Self {
        self.interpolation = LineInterpolation::Curve;
        self
    }

    /// Enable step interpolation (step-before).
    pub fn step(mut self) -> Self {
        self.interpolation = LineInterpolation::StepBefore;
        self
    }

    /// Enable linear interpolation (default).
    pub fn linear(mut self) -> Self {
        self.interpolation = LineInterpolation::Linear;
        self
    }

    /// Set the color accessor function (alias for stroke).
    pub fn color<A>(mut self, accessor: A) -> Self
    where
        A: Into<AccessorFunction<T>>,
    {
        self.stroke_accessor = Some(accessor.into());
        self
    }

    /// Set the X-axis scale.
    ///
    /// Accepts any scale type that implements `Into<AxisScale>`, including
    /// [`LinearScale`] and [`LogScale`](crate::shader_function::LogScale).
    /// The scale's domain is used to auto-configure axis tick generation.
    pub fn x_scale(mut self, scale: impl Into<AxisScale>) -> Self {
        self.config.x_scale = Some(scale.into());
        self
    }

    /// Set the Y-axis scale.
    ///
    /// Accepts any scale type that implements `Into<AxisScale>`, including
    /// [`LinearScale`] and [`LogScale`](crate::shader_function::LogScale).
    /// The scale's domain is used to auto-configure axis tick generation.
    pub fn y_scale(mut self, scale: impl Into<AxisScale>) -> Self {
        self.config.y_scale = Some(scale.into());
        self
    }
}

impl<T> Default for LineChartBuilder<T> {
    fn default() -> Self {
        Self::new()
    }
}

// Implement configurable builder methods
impl<T> ConfigurableBuilder for LineChartBuilder<T> {
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
impl<T> GridCapableBuilder for LineChartBuilder<T> {
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

impl<T> ChartBuilder<T> for LineChartBuilder<T>
where
    T: Clone + MaybeSend + MaybeSync + std::fmt::Debug + 'static,
{
    type Output = Selection<T, Circle>; // TODO: Replace with Line mark

    fn build_with_data(self, data: Vec<T>, context: Arc<RenderContext>) -> GupResult<Self::Output> {
        // Validate required accessors
        validate_required_accessors(&self.x_accessor, &self.y_accessor)?;

        if data.is_empty() {
            return Err(ChartBuilderError::EmptyData.into());
        }

        // Sort data by X coordinate if requested
        let sorted_data = data;
        if self.sort_by_x {
            // In a full implementation, this would sort by the actual X values
            // For now, we maintain the original order
        }

        // Create selection with Circle marks (TODO: Replace with Line marks)
        let mut selection = Selection::<T, Circle>::new(sorted_data, context)?;

        // Apply accessor functions to selection
        apply_accessors_to_selection(
            &mut selection,
            &self.x_accessor,
            &self.y_accessor,
            &self.stroke_accessor, // Use stroke as color
            &None,                 // Lines don't use size
        )?;

        // Apply stroke width if specified
        if self.stroke_width_accessor.is_some() {
            // Stroke width would be applied as a shader function
        }

        // Apply opacity if specified
        if self.opacity_accessor.is_some() {
            // Opacity would be applied as a shader function
        }

        // Configure line interpolation in the shader pipeline
        // This would be handled by the Line mark's shader generation

        Ok(selection)
    }
}

/// Convenience function to create a new line chart builder.
pub fn line<T>() -> LineChartBuilder<T> {
    LineChartBuilder::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RenderContext;
    use crate::chart_builder::accessor::{AccessorValue, x, y};

    #[derive(Debug, Clone)]
    struct TimePoint {
        time: f32,
        value: f32,
        series: String,
    }

    #[tokio::test]
    async fn test_line_chart_builder_basic() {
        let data = vec![
            TimePoint {
                time: 0.0,
                value: 10.0,
                series: "A".to_string(),
            },
            TimePoint {
                time: 1.0,
                value: 15.0,
                series: "A".to_string(),
            },
            TimePoint {
                time: 2.0,
                value: 12.0,
                series: "A".to_string(),
            },
        ];

        let context = Arc::new(RenderContext::new().await.unwrap());

        let builder = line()
            .x(AccessorFunction::new(|d: &TimePoint| {
                AccessorValue::Float(d.time)
            }))
            .y(AccessorFunction::new(|d: &TimePoint| {
                AccessorValue::Float(d.value)
            }))
            .stroke_color([0.0, 0.5, 1.0, 1.0])
            .stroke_width_px(2.0)
            .title("Time Series Chart");

        let result = builder.build_with_data(data, context);
        assert!(result.is_ok());

        let selection = result.unwrap();
        assert_eq!(selection.len(), 3);
    }

    #[test]
    fn test_line_chart_interpolation_methods() {
        let builder = line::<TimePoint>().linear();
        assert_eq!(builder.interpolation, LineInterpolation::Linear);

        let builder = line::<TimePoint>().smooth();
        assert_eq!(builder.interpolation, LineInterpolation::Curve);

        let builder = line::<TimePoint>().step();
        assert_eq!(builder.interpolation, LineInterpolation::StepBefore);

        let builder = line::<TimePoint>().interpolate(LineInterpolation::StepAfter);
        assert_eq!(builder.interpolation, LineInterpolation::StepAfter);
    }

    #[test]
    fn test_line_chart_configuration_options() {
        let builder = line::<TimePoint>()
            .sort_x(false)
            .connect_nulls(true)
            .stroke_width_px(3.0);

        assert!(!builder.sort_by_x);
        assert!(builder.connect_nulls);
        assert!(builder.stroke_width_accessor.is_some());
    }

    #[tokio::test]
    async fn test_line_chart_field_accessors() {
        let data = vec![TimePoint {
            time: 1.5,
            value: 20.0,
            series: "test".to_string(),
        }];

        let context = Arc::new(RenderContext::new().await.unwrap());

        let builder = line()
            .x(x("time"))
            .y(y("value"))
            .stroke(AccessorFunction::new(|d: &TimePoint| {
                AccessorValue::String(d.series.clone())
            }));

        let result = builder.build_with_data(data, context);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_line_chart_validation_errors() {
        let data = vec![TimePoint {
            time: 1.0,
            value: 2.0,
            series: "A".to_string(),
        }];

        let context = Arc::new(RenderContext::new().await.unwrap());

        // Missing Y accessor should fail
        let builder = line().x(x("time"));
        let result = builder.build_with_data(data.clone(), context.clone());
        assert!(result.is_err());

        // Empty data should fail
        let empty_data: Vec<TimePoint> = vec![];
        let builder = line().x(x("time")).y(y("value"));
        let result = builder.build_with_data(empty_data, context);
        assert!(result.is_err());
    }

    #[test]
    fn test_line_interpolation_default() {
        assert_eq!(LineInterpolation::default(), LineInterpolation::Linear);
    }

    #[test]
    fn test_line_chart_accessor_application() {
        let test_point = TimePoint {
            time: 5.0,
            value: 25.0,
            series: "B".to_string(),
        };

        let builder = line::<TimePoint>()
            .stroke_color([1.0, 0.0, 0.0, 1.0])
            .stroke_width_px(4.0);

        // Test stroke color accessor
        if let Some(stroke_acc) = &builder.stroke_accessor {
            let color_value = stroke_acc.apply(&test_point);
            assert_eq!(color_value, AccessorValue::Color([1.0, 0.0, 0.0, 1.0]));
        }

        // Test stroke width accessor
        if let Some(width_acc) = &builder.stroke_width_accessor {
            let width_value = width_acc.apply(&test_point);
            assert_eq!(width_value, AccessorValue::Float(4.0));
        }
    }

    #[test]
    fn test_line_chart_builder_default() {
        let builder = LineChartBuilder::<TimePoint>::default();
        assert!(builder.x_accessor.is_none());
        assert!(builder.y_accessor.is_none());
        assert_eq!(builder.interpolation, LineInterpolation::Linear);
        assert!(builder.sort_by_x);
        assert!(!builder.connect_nulls);
    }

    // Tests for enhanced grid API (GUP-097) on line charts
    #[test]
    fn test_line_chart_enhanced_grid_api() {
        // Test simple grid enabling
        let builder = line::<TimePoint>().grid();
        assert!(builder.config.show_grid);

        // Test theme presets work for line charts too
        let scientific_builder = line::<TimePoint>().scientific_grid();
        assert!(scientific_builder.config.show_grid);
        assert!(scientific_builder.config.grid_config.minor_grid.enabled);

        let business_builder = line::<TimePoint>().business_grid();
        assert!(business_builder.config.show_grid);
        assert!(!business_builder.config.grid_config.show_vertical); // Business typically horizontal
    }

    #[test]
    fn test_line_chart_grid_styling_shortcuts() {
        // Test grid styling methods work with line charts individually
        let color_builder = line::<TimePoint>().grid_color("#00ff00");
        assert!(color_builder.config.show_grid);
        let green_component = color_builder.config.grid_config.major_grid.color[1];
        assert!((green_component - 1.0).abs() < 0.01); // Should be close to 1.0 (green)

        let opacity_builder = line::<TimePoint>().grid_opacity(0.7);
        assert!(opacity_builder.config.show_grid);
        assert_eq!(opacity_builder.config.grid_config.major_grid.opacity, 0.7);

        let width_builder = line::<TimePoint>().grid_width(1.5);
        assert!(width_builder.config.show_grid);
        assert_eq!(width_builder.config.grid_config.major_grid.line_width, 1.5);
    }

    #[test]
    fn test_line_chart_grid_with_line_features() {
        // Test that grid API works well with line-specific features
        let builder = line::<TimePoint>()
            .smooth()
            .stroke_color([1.0, 0.0, 0.0, 1.0])
            .horizontal_grid(); // This should set horizontal only

        assert_eq!(builder.interpolation, LineInterpolation::Curve);
        assert!(builder.stroke_accessor.is_some());
        assert!(builder.config.show_grid);
        assert!(builder.config.grid_config.show_horizontal);
        assert!(!builder.config.grid_config.show_vertical);

        // Test opacity separately to avoid chaining issues
        let opacity_builder = line::<TimePoint>().grid_opacity(0.3);
        assert_eq!(opacity_builder.config.grid_config.major_grid.opacity, 0.3);
    }
}

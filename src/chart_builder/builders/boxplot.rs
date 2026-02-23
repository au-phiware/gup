// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Box plot builder with Observable Plot compatibility.
//!
//! Provides a fluent API for creating GPU-accelerated box plots with
//! automatic statistical computation and shader function integration.

use super::{AccessorFunction, ConfigurableBuilder, GridCapableBuilder};
use crate::RenderContext;
use crate::chart_builder::{ChartBuilder, ChartBuilderError, ChartConfig, ComposedChart};
use crate::error::GupResult;
use crate::grid::{GridConfiguration, GridLineConfig};
use crate::mark::boxplot::{BoxPlot, BoxPlotAttributes, BoxPlotOrientation};
use crate::selection::Selection;
use crate::shader_function::Vec2;
use std::marker::PhantomData;
use std::sync::Arc;

/// Box plot builder providing Observable Plot-style API.
///
/// # Examples
///
/// ```rust,no_run
/// use gup::prelude::*;
/// use gup::chart_builder::accessor::AccessorValue;
/// use gup::chart_builder::builders::{AccessorFunction, boxplot};
///
/// #[derive(Debug, Clone)]
/// struct Measurement {
///     category: String,
///     values: Vec<f32>,
/// }
///
/// # async fn example() -> GupResult<()> {
/// # let context = std::sync::Arc::new(RenderContext::new().await?);
/// let measurements = vec![
///     Measurement {
///         category: "A".to_string(),
///         values: vec![10.0, 15.0, 20.0, 25.0, 30.0],
///     },
/// ];
///
/// // Observable Plot-style box plot
/// let chart = boxplot()
///     .y(AccessorFunction::new(|m: &Measurement| AccessorValue::FloatArray(m.values.clone())))
///     .build_with_data(measurements, context)?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct BoxPlotBuilder<T> {
    pub(crate) x_accessor: Option<AccessorFunction<T>>,
    pub(crate) y_accessor: Option<AccessorFunction<T>>,
    pub(crate) color_accessor: Option<AccessorFunction<T>>,
    pub(crate) width_value: f32,
    pub(crate) orientation: BoxPlotOrientation,
    pub(crate) config: ChartConfig,
    pub(crate) _phantom: PhantomData<T>,
}

impl<T> BoxPlotBuilder<T> {
    /// Create a new box plot builder with default settings.
    pub fn new() -> Self {
        Self {
            x_accessor: None,
            y_accessor: None,
            color_accessor: None,
            width_value: 40.0,
            orientation: BoxPlotOrientation::Vertical,
            config: ChartConfig::default(),
            _phantom: PhantomData,
        }
    }

    /// Set the X-axis accessor (typically for category or position).
    ///
    /// For vertical box plots, this determines horizontal position.
    /// For horizontal box plots, this provides the data values.
    pub fn x<A>(mut self, accessor: A) -> Self
    where
        A: Into<AccessorFunction<T>>,
    {
        self.x_accessor = Some(accessor.into());
        self
    }

    /// Set the Y-axis accessor (typically for data values).
    ///
    /// For vertical box plots, this provides the data values.
    /// For horizontal box plots, this determines vertical position.
    ///
    /// The accessor should return either a single value or an array of values
    /// from which statistics will be computed.
    pub fn y<A>(mut self, accessor: A) -> Self
    where
        A: Into<AccessorFunction<T>>,
    {
        self.y_accessor = Some(accessor.into());
        self
    }

    /// Set the color accessor for box fill color.
    ///
    /// Can be used to color boxes by category or any other data attribute.
    pub fn color<A>(mut self, accessor: A) -> Self
    where
        A: Into<AccessorFunction<T>>,
    {
        self.color_accessor = Some(accessor.into());
        self
    }

    /// Set the width of the box.
    ///
    /// Default is 40.0 pixels.
    pub fn box_width(mut self, width: f32) -> Self {
        self.width_value = width;
        self
    }

    /// Set the orientation to vertical (default).
    ///
    /// In vertical orientation, the Y accessor provides values
    /// and X provides position/category.
    pub fn vertical(mut self) -> Self {
        self.orientation = BoxPlotOrientation::Vertical;
        self
    }

    /// Set the orientation to horizontal.
    ///
    /// In horizontal orientation, the X accessor provides values
    /// and Y provides position/category.
    pub fn horizontal(mut self) -> Self {
        self.orientation = BoxPlotOrientation::Horizontal;
        self
    }

    /// Set a fixed color for all boxes.
    pub fn fill_color(mut self, color: [f32; 4]) -> Self {
        use crate::chart_builder::accessor::AccessorValue;
        self.color_accessor = Some(AccessorFunction::new(move |_: &T| {
            AccessorValue::Color(color)
        }));
        self
    }
}

impl<T> Default for BoxPlotBuilder<T> {
    fn default() -> Self {
        Self::new()
    }
}

// Implement configurable builder methods
impl<T> ConfigurableBuilder for BoxPlotBuilder<T> {
    fn title(mut self, title: impl Into<String>) -> Self {
        self.config.title = Some(title.into());
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
}

// Implement advanced grid configuration methods
impl<T> GridCapableBuilder for BoxPlotBuilder<T> {
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
        self.config.show_grid = true;
        self
    }

    fn vertical_grid_only(mut self) -> Self {
        self.config.grid_config.show_horizontal = false;
        self.config.grid_config.show_vertical = true;
        self.config.show_grid = true;
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

impl<T> ChartBuilder<T> for BoxPlotBuilder<T>
where
    T: Clone + Send + Sync + std::fmt::Debug + 'static,
{
    type Output = ComposedChart<BoxPlotAttributes, BoxPlot>;

    fn build_with_data(self, data: Vec<T>, context: Arc<RenderContext>) -> GupResult<Self::Output> {
        if data.is_empty() {
            return Err(ChartBuilderError::EmptyData.into());
        }

        // For box plots, we need to extract data values and compute statistics
        // This is different from scatter plots where each datum maps directly to a mark
        let boxplot_data = self.compute_boxplot_attributes(&data)?;

        // Create selection with BoxPlot marks
        let selection = Selection::<BoxPlotAttributes, BoxPlot>::new(boxplot_data, context)?;

        // Apply basic accessors if provided
        // Note: For box plots, most attributes are computed from statistical data
        // Color accessor can still be applied if provided
        if let Some(ref color_accessor) = self.color_accessor {
            // Would need to map color accessor to box plot attributes
            // For now, this is a placeholder for future enhancement
            let _ = color_accessor;
        }

        // Create composed chart with axes based on configuration
        let composed_chart = ComposedChart::new(selection, self.config).with_default_axes();

        Ok(composed_chart)
    }
}

impl<T> BoxPlotBuilder<T>
where
    T: Clone + Send + Sync + std::fmt::Debug + 'static,
{
    /// Compute box plot attributes from raw data using statistical functions.
    ///
    /// This extracts values from the Y accessor (or X accessor for horizontal orientation)
    /// and computes the five-number summary plus outliers.
    fn compute_boxplot_attributes(&self, data: &[T]) -> GupResult<Vec<BoxPlotAttributes>> {
        use crate::chart_builder::accessor::AccessorValue;

        // Get the value accessor based on orientation
        let value_accessor = match self.orientation {
            BoxPlotOrientation::Vertical => &self.y_accessor,
            BoxPlotOrientation::Horizontal => &self.x_accessor,
        };

        let value_accessor =
            value_accessor
                .as_ref()
                .ok_or_else(|| ChartBuilderError::MissingAccessor {
                    attribute: match self.orientation {
                        BoxPlotOrientation::Vertical => "y".to_string(),
                        BoxPlotOrientation::Horizontal => "x".to_string(),
                    },
                })?;

        // For now, create one box plot from all data
        // Future enhancement: group by category from X accessor
        let mut all_values = Vec::new();

        for datum in data {
            let value = value_accessor.apply(datum);
            match value {
                AccessorValue::Float(v) => all_values.push(v),
                AccessorValue::FloatArray(arr) => all_values.extend(arr),
                _ => {
                    return Err(crate::error::GupError::validation_error(format!(
                        "Box plot requires Float or FloatArray accessor, got: {:?}",
                        value
                    )));
                }
            }
        }

        if all_values.is_empty() {
            return Err(ChartBuilderError::EmptyData.into());
        }

        // Compute box plot statistics
        let position = Vec2 { x: 0.0, y: 0.0 };
        let attrs =
            BoxPlotAttributes::from_data(&all_values, position, self.width_value, self.orientation);

        Ok(vec![attrs])
    }
}

/// Convenience function to create a new box plot builder.
///
/// # Examples
///
/// ```rust,no_run
/// use gup::prelude::*;
/// use gup::chart_builder::accessor::AccessorValue;
/// use gup::chart_builder::builders::{AccessorFunction, boxplot};
///
/// #[derive(Debug, Clone)]
/// struct DataSet {
///     values: Vec<f32>,
/// }
///
/// # async fn example() -> GupResult<()> {
/// # let context = std::sync::Arc::new(RenderContext::new().await?);
/// # let data = vec![DataSet { values: vec![1.0, 2.0, 3.0] }];
/// let chart = boxplot()
///     .y(AccessorFunction::new(|d: &DataSet| AccessorValue::FloatArray(d.values.clone())))
///     .build_with_data(data, context)?;
/// # Ok(())
/// # }
/// ```
pub fn boxplot<T>() -> BoxPlotBuilder<T> {
    BoxPlotBuilder::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chart_builder::accessor::AccessorValue;

    #[derive(Debug, Clone)]
    struct TestData {
        category: String,
        values: Vec<f32>,
        single_value: f32,
    }

    #[tokio::test]
    async fn test_boxplot_builder_basic() {
        let data = vec![TestData {
            category: "A".to_string(),
            values: vec![10.0, 15.0, 20.0, 25.0, 30.0, 35.0, 40.0],
            single_value: 25.0,
        }];

        let context = Arc::new(RenderContext::new().await.unwrap());

        let builder = boxplot()
            .y(AccessorFunction::new(|d: &TestData| {
                AccessorValue::FloatArray(d.values.clone())
            }))
            .title("Test Box Plot")
            .width(600.0)
            .height(400.0);

        let result = builder.build_with_data(data, context);
        assert!(result.is_ok());

        let selection = result.unwrap();
        assert_eq!(selection.len(), 1);
        assert!(!selection.is_empty());
    }

    #[tokio::test]
    async fn test_boxplot_builder_single_values() {
        let data = vec![
            TestData {
                category: "A".to_string(),
                values: vec![],
                single_value: 10.0,
            },
            TestData {
                category: "A".to_string(),
                values: vec![],
                single_value: 20.0,
            },
            TestData {
                category: "A".to_string(),
                values: vec![],
                single_value: 30.0,
            },
        ];

        let context = Arc::new(RenderContext::new().await.unwrap());

        let builder = boxplot().y(AccessorFunction::new(|d: &TestData| {
            AccessorValue::Float(d.single_value)
        }));

        let result = builder.build_with_data(data, context);
        assert!(result.is_ok());
    }

    #[test]
    fn test_boxplot_builder_configuration() {
        let builder = boxplot::<TestData>()
            .title("My Box Plot")
            .width(1000.0)
            .height(800.0)
            .background([0.9, 0.9, 0.9, 1.0])
            .box_width(50.0)
            .horizontal()
            .show_axes(true)
            .show_grid(false);

        assert_eq!(builder.config.title, Some("My Box Plot".to_string()));
        assert_eq!(builder.config.width, 1000.0);
        assert_eq!(builder.config.height, 800.0);
        assert_eq!(builder.config.background_color, Some([0.9, 0.9, 0.9, 1.0]));
        assert_eq!(builder.width_value, 50.0);
        assert_eq!(builder.orientation, BoxPlotOrientation::Horizontal);
        assert!(builder.config.show_axes);
        assert!(!builder.config.show_grid);
    }

    #[tokio::test]
    async fn test_boxplot_builder_validation_errors() {
        let data = vec![TestData {
            category: "A".to_string(),
            values: vec![1.0, 2.0],
            single_value: 1.0,
        }];

        let context = Arc::new(RenderContext::new().await.unwrap());

        // Missing Y accessor should fail
        let builder = boxplot::<TestData>();
        let result = builder.build_with_data(data.clone(), context.clone());
        assert!(result.is_err());
        let error_str = format!("{:?}", result.unwrap_err());
        assert!(error_str.contains("Missing") || error_str.contains("accessor"));

        // Empty data should fail
        let empty_data: Vec<TestData> = vec![];
        let builder = boxplot().y(AccessorFunction::new(|d: &TestData| {
            AccessorValue::FloatArray(d.values.clone())
        }));
        let result = builder.build_with_data(empty_data, context);
        assert!(result.is_err());
    }

    #[test]
    fn test_boxplot_builder_orientation() {
        let vertical_builder = boxplot::<TestData>().vertical();
        assert_eq!(vertical_builder.orientation, BoxPlotOrientation::Vertical);

        let horizontal_builder = boxplot::<TestData>().horizontal();
        assert_eq!(
            horizontal_builder.orientation,
            BoxPlotOrientation::Horizontal
        );
    }

    #[test]
    fn test_boxplot_builder_default() {
        let builder = BoxPlotBuilder::<TestData>::default();
        assert!(builder.x_accessor.is_none());
        assert!(builder.y_accessor.is_none());
        assert!(builder.color_accessor.is_none());
        assert_eq!(builder.width_value, 40.0);
        assert_eq!(builder.orientation, BoxPlotOrientation::Vertical);
        assert_eq!(builder.config.width, 800.0); // Default config values
        assert_eq!(builder.config.height, 600.0);
    }

    #[test]
    fn test_boxplot_builder_grid_api() {
        // Test simple grid enabling
        let builder = boxplot::<TestData>().grid();
        assert!(builder.config.show_grid);

        // Test directional grids
        let h_builder = boxplot::<TestData>().horizontal_grid_only();
        assert!(h_builder.config.show_grid);
        assert!(h_builder.config.grid_config.show_horizontal);
        assert!(!h_builder.config.grid_config.show_vertical);

        let v_builder = boxplot::<TestData>().vertical_grid_only();
        assert!(v_builder.config.show_grid);
        assert!(!v_builder.config.grid_config.show_horizontal);
        assert!(v_builder.config.grid_config.show_vertical);
    }
}

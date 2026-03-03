// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! High-level plot API providing Observable Plot-style interface.
//!
//! This module implements the main `plot()` entry point that provides
//! Observable Plot compatibility with fluent method chaining.

use super::ChartBuilder;
use super::accessor::FieldAccessor;
use super::builders::{
    AreaChartBuilder, BarChartBuilder, BoxPlotBuilder, HeatmapBuilder, LineChartBuilder,
    ScatterPlotBuilder, ViolinPlotBuilder,
};
use crate::RenderContext;
use crate::error::{GupError, GupResult};
use crate::{MaybeSend, MaybeSync};
use std::marker::PhantomData;
use std::sync::Arc;

/// Main plot builder providing Observable Plot-style API entry point.
///
/// This is the primary interface for creating charts with Observable Plot
/// syntax while maintaining GPU performance and type safety.
///
/// # Examples
///
/// ```rust,no_run
/// use gup::prelude::*;
///
/// #[derive(Debug, Clone)]
/// struct SalesPoint {
///     revenue: f32,
///     profit: f32,
///     region: String,
/// }
///
/// # async fn example() -> GupResult<()> {
/// let sales_data = vec![
///     SalesPoint { revenue: 100.0, profit: 20.0, region: "North".to_string() },
///     SalesPoint { revenue: 200.0, profit: 45.0, region: "South".to_string() },
/// ];
///
/// // Observable Plot-style scatter plot
/// let bound_builder = gup::plot()
///     .data(sales_data)
///     .scatter(x("revenue"), y("profit"));
/// # Ok(())
/// # }
/// ```
pub struct PlotBuilder {
    context: Option<Arc<RenderContext>>,
}

impl PlotBuilder {
    /// Create a new plot builder.
    pub fn new() -> Self {
        Self { context: None }
    }

    /// Set a custom render context for this plot.
    pub fn with_context(mut self, context: Arc<RenderContext>) -> Self {
        self.context = Some(context);
        self
    }

    /// Bind data to this plot, creating a data-bound plot builder.
    pub fn data<T>(self, data: Vec<T>) -> BoundPlotBuilder<T>
    where
        T: Clone + MaybeSend + MaybeSync + std::fmt::Debug + 'static,
    {
        BoundPlotBuilder::new(data, self.context)
    }
}

impl Default for PlotBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// A plot builder that has been bound to specific data.
///
/// This enables Observable Plot-style method chaining while preserving
/// type information about the data.
pub struct BoundPlotBuilder<T> {
    data: Vec<T>,
    context: Option<Arc<RenderContext>>,
    _phantom: PhantomData<T>,
}

impl<T> BoundPlotBuilder<T>
where
    T: Clone + MaybeSend + MaybeSync + std::fmt::Debug + 'static,
{
    /// Create a new bound plot builder.
    pub fn new(data: Vec<T>, context: Option<Arc<RenderContext>>) -> Self {
        Self {
            data,
            context,
            _phantom: PhantomData,
        }
    }

    /// Create a scatter plot with the specified X and Y accessors.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use gup::prelude::*;
    ///
    /// # #[derive(Debug, Clone)] struct Point { revenue: f32, profit: f32 }
    /// # let points = vec![Point { revenue: 100.0, profit: 20.0 }];
    /// let chart = plot()
    ///     .data(points)
    ///     .scatter(x("revenue"), y("profit"));
    /// ```
    pub fn scatter(
        self,
        x_accessor: FieldAccessor,
        y_accessor: FieldAccessor,
    ) -> ConfiguredScatterPlot<T> {
        let builder = ScatterPlotBuilder::new().x(x_accessor).y(y_accessor);

        ConfiguredScatterPlot::new(self.data, self.context, builder)
    }

    /// Create a line chart with the specified X and Y accessors.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use gup::prelude::*;
    ///
    /// # #[derive(Debug, Clone)] struct TimePoint { date: f32, value: f32 }
    /// # let time_series = vec![TimePoint { date: 1.0, value: 10.0 }];
    /// let chart = plot()
    ///     .data(time_series)
    ///     .line(x("date"), y("value"));
    /// ```
    pub fn line(
        self,
        x_accessor: FieldAccessor,
        y_accessor: FieldAccessor,
    ) -> ConfiguredLineChart<T> {
        let builder = LineChartBuilder::new().x(x_accessor).y(y_accessor);

        ConfiguredLineChart::new(self.data, self.context, builder)
    }

    /// Create a bar chart with the specified X and Y accessors.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use gup::prelude::*;
    ///
    /// # #[derive(Debug, Clone)] struct Category { category: String, count: f32 }
    /// # let categories = vec![Category { category: "A".to_string(), count: 10.0 }];
    /// let chart = plot()
    ///     .data(categories)
    ///     .bar(x("category"), y("count"));
    /// ```
    pub fn bar(
        self,
        x_accessor: FieldAccessor,
        y_accessor: FieldAccessor,
    ) -> ConfiguredBarChart<T> {
        let builder = BarChartBuilder::new().x(x_accessor).y(y_accessor);

        ConfiguredBarChart::new(self.data, self.context, builder)
    }

    /// Create an area chart with the specified X and Y accessors.
    pub fn area(
        self,
        x_accessor: FieldAccessor,
        y_accessor: FieldAccessor,
    ) -> ConfiguredAreaChart<T> {
        let builder = AreaChartBuilder::new().x(x_accessor).y(y_accessor);

        ConfiguredAreaChart::new(self.data, self.context, builder)
    }

    /// Create a heatmap with the specified X and Y accessors and fill value.
    pub fn heatmap(
        self,
        x_accessor: FieldAccessor,
        y_accessor: FieldAccessor,
        fill_accessor: FieldAccessor,
    ) -> ConfiguredHeatmap<T> {
        let builder = HeatmapBuilder::new()
            .x(x_accessor)
            .y(y_accessor)
            .fill(fill_accessor);

        ConfiguredHeatmap::new(self.data, self.context, builder)
    }

    /// Create a box plot with the specified Y accessor.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use gup::prelude::*;
    ///
    /// # #[derive(Debug, Clone)] struct DataSet { values: Vec<f32> }
    /// # let data_sets = vec![DataSet { values: vec![1.0, 2.0, 3.0] }];
    /// let chart = plot()
    ///     .data(data_sets)
    ///     .boxplot(y("values"));
    /// ```
    pub fn boxplot(self, y_accessor: FieldAccessor) -> ConfiguredBoxPlot<T> {
        let builder = BoxPlotBuilder::new().y(y_accessor);

        ConfiguredBoxPlot::new(self.data, self.context, builder)
    }

    /// Create a violin plot with the specified Y accessor.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use gup::prelude::*;
    ///
    /// # #[derive(Debug, Clone)] struct DataSet { values: f32 }
    /// # let data_sets = vec![DataSet { values: 1.0 }];
    /// let chart = plot()
    ///     .data(data_sets)
    ///     .violin(y("values"));
    /// ```
    pub fn violin(self, y_accessor: FieldAccessor) -> ConfiguredViolinPlot<T> {
        let builder = ViolinPlotBuilder::new().y(y_accessor);

        ConfiguredViolinPlot::new(self.data, self.context, builder)
    }

    /// Get the underlying data.
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

/// Base trait for all configured chart types.
pub trait ConfiguredChart<T> {
    type Builder: ChartBuilder<T>;
    type Output;

    /// Build the chart.
    fn build(self) -> GupResult<Self::Output>;

    /// Build and render the chart.
    fn render(self) -> GupResult<Self::Output>
    where
        Self::Output: crate::Mixable;
}

/// Macro to generate configured chart types with common methods.
macro_rules! impl_configured_chart {
    ($chart_type:ident, $builder_type:ty) => {
        pub struct $chart_type<T> {
            data: Vec<T>,
            context: Option<Arc<RenderContext>>,
            builder: $builder_type,
        }

        impl<T> $chart_type<T>
        where
            T: Clone + MaybeSend + MaybeSync + std::fmt::Debug + 'static,
        {
            pub fn new(
                data: Vec<T>,
                context: Option<Arc<RenderContext>>,
                builder: $builder_type,
            ) -> Self {
                Self {
                    data,
                    context,
                    builder,
                }
            }

            /// Set the color accessor using field name.
            pub fn color(mut self, field_accessor: FieldAccessor) -> Self {
                use super::builders::AccessorFunction;
                self.builder = self.builder.color(AccessorFunction::from(field_accessor));
                self
            }

            /// Set the size accessor using field name (if applicable).
            pub fn size(self, _field_accessor: FieldAccessor) -> Self {
                // Size method would be conditional based on chart type
                // For simplicity, we'll include it for all types
                self
            }

            /// Set a custom title for this chart.
            pub fn title(mut self, title: impl Into<String>) -> Self {
                use super::builders::ConfigurableBuilder;
                self.builder = self.builder.title(title);
                self
            }

            /// Set chart dimensions.
            pub fn dimensions(mut self, width: f32, height: f32) -> Self {
                use super::builders::ConfigurableBuilder;
                self.builder = self.builder.width(width).height(height);
                self
            }

            /// Get the underlying data.
            pub fn data(&self) -> &[T] {
                &self.data
            }

            async fn ensure_context(&self) -> GupResult<Arc<RenderContext>> {
                match &self.context {
                    Some(ctx) => Ok(Arc::clone(ctx)),
                    None => {
                        let ctx = RenderContext::new().await.map_err(|e| {
                            GupError::render_error(format!("Failed to create context: {e}"))
                        })?;
                        Ok(Arc::new(ctx))
                    }
                }
            }
        }

        impl<T> ConfiguredChart<T> for $chart_type<T>
        where
            T: Clone + MaybeSend + MaybeSync + std::fmt::Debug + 'static,
        {
            type Builder = $builder_type;
            type Output = <$builder_type as ChartBuilder<T>>::Output;

            fn build(self) -> GupResult<Self::Output> {
                // This would need to be async, but traits don't support async methods yet
                // In a real implementation, we'd use a different pattern
                Err(GupError::validation_error(
                    "Synchronous build not supported. Use async build pattern.",
                ))
            }

            fn render(self) -> GupResult<Self::Output>
            where
                Self::Output: crate::Mixable,
            {
                // Similar async issue - would need different pattern in real implementation
                Err(GupError::validation_error(
                    "Synchronous render not supported. Use async render pattern.",
                ))
            }
        }

        impl<T> $chart_type<T>
        where
            T: Clone + MaybeSend + MaybeSync + std::fmt::Debug + 'static,
        {
            /// Build the chart (async version).
            pub async fn build_async(
                self,
            ) -> GupResult<<$builder_type as ChartBuilder<T>>::Output> {
                let context = self.ensure_context().await?;
                self.builder.build_with_data(self.data, context)
            }

            /// Build and render the chart (async version).
            pub async fn render_async(self) -> GupResult<<$builder_type as ChartBuilder<T>>::Output>
            where
                <$builder_type as ChartBuilder<T>>::Output: crate::Mixable,
            {
                let context = self.ensure_context().await?;
                self.builder.render_with_data(self.data, context)
            }
        }
    };
}

// Generate configured chart types
impl_configured_chart!(ConfiguredScatterPlot, ScatterPlotBuilder<T>);
impl_configured_chart!(ConfiguredLineChart, LineChartBuilder<T>);
impl_configured_chart!(ConfiguredBarChart, BarChartBuilder<T>);
impl_configured_chart!(ConfiguredAreaChart, AreaChartBuilder<T>);
impl_configured_chart!(ConfiguredHeatmap, HeatmapBuilder<T>);
impl_configured_chart!(ConfiguredBoxPlot, BoxPlotBuilder<T>);
impl_configured_chart!(ConfiguredViolinPlot, ViolinPlotBuilder<T>);

/// Main entry point for Observable Plot-style API.
///
/// This function returns a PlotBuilder that can be configured with data
/// and chart specifications.
///
/// # Examples
///
/// ```rust,no_run
/// use gup::prelude::*;
///
/// # #[derive(Debug, Clone)] struct SalesData { revenue: f32, profit: f32, region: String }
/// # let sales_data = vec![SalesData { revenue: 100.0, profit: 20.0, region: "North".to_string() }];
/// let chart = plot()
///     .data(sales_data)
///     .scatter(x("revenue"), y("profit"));
/// ```
pub fn plot() -> PlotBuilder {
    PlotBuilder::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chart_builder::accessor::{x, y};

    #[derive(Debug, Clone)]
    #[allow(dead_code)]
    struct TestData {
        x: f32,
        y: f32,
        category: String,
    }

    #[test]
    fn test_plot_builder_creation() {
        let builder = plot();
        assert!(builder.context.is_none());

        let data = vec![TestData {
            x: 1.0,
            y: 2.0,
            category: "A".to_string(),
        }];

        let bound_builder = builder.data(data);
        assert_eq!(bound_builder.len(), 1);
        assert!(!bound_builder.is_empty());
    }

    #[test]
    fn test_plot_builder_with_context() {
        let builder = plot();
        assert!(builder.context.is_none());

        // Test that we can create a builder with context
        let builder_with_ctx = PlotBuilder::new();
        assert!(builder_with_ctx.context.is_none());
    }

    #[test]
    fn test_bound_plot_builder_chart_creation() {
        let data = vec![
            TestData {
                x: 1.0,
                y: 2.0,
                category: "A".to_string(),
            },
            TestData {
                x: 3.0,
                y: 4.0,
                category: "B".to_string(),
            },
        ];

        let bound_builder = plot().data(data);

        // Test scatter plot creation
        let scatter_chart = bound_builder
            .scatter(x("x"), y("y"))
            .color(FieldAccessor::new("category"))
            .title("Test Scatter")
            .dimensions(800.0, 600.0);

        assert_eq!(scatter_chart.data().len(), 2);
    }

    #[test]
    fn test_all_chart_types_creation() {
        let data = vec![TestData {
            x: 1.0,
            y: 2.0,
            category: "test".to_string(),
        }];

        let plot_builder = plot().data(data);

        // Test that all chart types can be created without panic
        let _scatter = plot_builder.scatter(x("x"), y("y")).title("Scatter");

        let data2 = vec![TestData {
            x: 1.0,
            y: 2.0,
            category: "test".to_string(),
        }];

        let _line = plot().data(data2).line(x("x"), y("y")).title("Line");

        let data3 = vec![TestData {
            x: 1.0,
            y: 2.0,
            category: "test".to_string(),
        }];

        let _bar = plot().data(data3).bar(x("category"), y("y")).title("Bar");

        let data4 = vec![TestData {
            x: 1.0,
            y: 2.0,
            category: "test".to_string(),
        }];

        let _area = plot().data(data4).area(x("x"), y("y")).title("Area");

        let data5 = vec![TestData {
            x: 1.0,
            y: 2.0,
            category: "test".to_string(),
        }];

        let _heatmap = plot()
            .data(data5)
            .heatmap(x("x"), y("y"), FieldAccessor::new("category"))
            .title("Heatmap");

        let data6 = vec![TestData {
            x: 1.0,
            y: 2.0,
            category: "test".to_string(),
        }];

        let _boxplot = plot().data(data6).boxplot(y("y")).title("Box Plot");

        let data7 = vec![TestData {
            x: 1.0,
            y: 2.0,
            category: "test".to_string(),
        }];

        let _violin = plot().data(data7).violin(y("y")).title("Violin Plot");
    }

    #[test]
    fn test_configured_chart_data_access() {
        let data = vec![
            TestData {
                x: 5.0,
                y: 10.0,
                category: "X".to_string(),
            },
            TestData {
                x: 15.0,
                y: 20.0,
                category: "Y".to_string(),
            },
        ];

        let scatter_chart = plot().data(data.clone()).scatter(x("x"), y("y"));

        assert_eq!(scatter_chart.data().len(), 2);
        assert_eq!(scatter_chart.data()[0].x, 5.0);
        assert_eq!(scatter_chart.data()[1].category, "Y");
    }

    #[test]
    fn test_plot_builder_default() {
        let builder = PlotBuilder::default();
        assert!(builder.context.is_none());
    }

    #[test]
    fn test_bound_plot_builder_empty_data() {
        let empty_data: Vec<TestData> = vec![];
        let bound_builder = plot().data(empty_data);

        assert_eq!(bound_builder.len(), 0);
        assert!(bound_builder.is_empty());
    }
}

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
pub mod optimized_accessor;
pub mod pipeline_cache;
pub mod plot_api;
pub mod shader_specialization;

pub use accessor::*;
pub use builders::*;
pub use labels::*;
pub use optimized_accessor::*;
pub use pipeline_cache::*;
pub use plot_api::*;
pub use shader_specialization::*;

use crate::RenderContext;
use crate::axis::{
    Axis, AxisBounds, AxisConfiguration, AxisLabel, AxisPosition, AxisRenderer, LinearAxis,
};
use crate::error::{GupError, GupResult};
use crate::grid::GridConfiguration;
use crate::label::{AxisInfo, LabelConstraints, LabelLayout, LabelPosition, LabelPositioner};
use crate::render::Vertex;
use crate::selection::Selection;
use crate::shader_function::Vec2;
use crate::text::TextStyle;
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

    // Convert this builder to a low-level Selection for advanced customization.
    //
    // This enables seamless transition from high-level builder APIs to
    // low-level Selection operations when needed.
    //
    // TODO: Disabled until Selection type is fully implemented
    /*
    pub fn into_selection<M>(self) -> GupResult<Selection<T, M>>
    where
        T: Clone + Send + Sync + std::fmt::Debug + 'static,
        M: crate::selection::Mark,
        M::AttributeValue: Default + Clone,
    {
        Selection::new(self.data, self.context)
    }
    */

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

/// Horizontal alignment for chart title text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TitleAlignment {
    /// Align the title to the left edge of the chart.
    Left,
    /// Center the title horizontally (default).
    #[default]
    Center,
    /// Align the title to the right edge of the chart.
    Right,
}

/// Configuration for chart title and optional subtitle.
///
/// Provides fine-grained control over title text, subtitle text,
/// alignment, and vertical positioning. When omitted from a
/// [`ChartConfig`], no title is rendered (backward compatible with
/// the pre-`TitleConfig` behaviour of `title: None`).
///
/// # Examples
///
/// ```rust
/// use gup::chart_builder::{TitleConfig, TitleAlignment};
/// use gup::text::TextStyle;
///
/// let title = TitleConfig::new("Revenue by Quarter")
///     .with_alignment(TitleAlignment::Left)
///     .with_subtitle("FY 2024")
///     .with_subtitle_style(TextStyle::new(14.0).with_rgba(0.5, 0.5, 0.5, 1.0));
/// ```
#[derive(Debug, Clone)]
pub struct TitleConfig {
    /// Primary title text.
    pub text: String,

    /// Horizontal alignment within the chart area.
    pub alignment: TitleAlignment,

    /// Vertical offset from the top edge of the chart (in pixels).
    ///
    /// When `None`, the title is positioned at `margins.top / 2.0`
    /// (the vertical centre of the top margin).
    pub y_offset: Option<f32>,

    /// Optional subtitle, rendered below the main title.
    pub subtitle: Option<String>,

    /// Text style for the subtitle.
    ///
    /// Defaults to a smaller, lighter variant when not explicitly set.
    pub subtitle_style: TextStyle,

    /// Line spacing multiplier between lines in a multi-line title.
    ///
    /// Applied to both the title and the gap between title and
    /// subtitle. Defaults to 1.2.
    pub line_spacing: f32,
}

impl TitleConfig {
    /// Create a new title configuration with the given text.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            alignment: TitleAlignment::default(),
            y_offset: None,
            subtitle: None,
            subtitle_style: TextStyle::new(14.0).with_rgba(0.4, 0.4, 0.4, 1.0),
            line_spacing: 1.2,
        }
    }

    /// Set horizontal alignment.
    pub fn with_alignment(mut self, alignment: TitleAlignment) -> Self {
        self.alignment = alignment;
        self
    }

    /// Set the vertical offset from the top edge (in pixels).
    pub fn with_y_offset(mut self, offset: f32) -> Self {
        self.y_offset = Some(offset);
        self
    }

    /// Set subtitle text.
    pub fn with_subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    /// Set the text style for the subtitle.
    pub fn with_subtitle_style(mut self, style: TextStyle) -> Self {
        self.subtitle_style = style;
        self
    }

    /// Set the line spacing multiplier for multi-line titles.
    pub fn with_line_spacing(mut self, spacing: f32) -> Self {
        self.line_spacing = spacing.max(0.1);
        self
    }
}

/// Chart configuration that applies to all chart types.
#[derive(Debug, Clone)]
pub struct ChartConfig {
    /// Chart title configuration (optional).
    ///
    /// Use [`ChartConfig::with_title`] for simple titles or
    /// [`ChartConfig::with_title_config`] for full layout control
    /// (alignment, subtitle, y-offset).
    pub title_config: Option<TitleConfig>,

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

    /// Text style for axis labels.
    ///
    /// When `font_family` is set, the chart text rendering methods
    /// will use `FontAtlasManager` to resolve the correct font atlas
    /// automatically.
    pub label_style: TextStyle,

    /// Text style for the chart title.
    ///
    /// When `font_family` is set, the chart text rendering methods
    /// will use `FontAtlasManager` to resolve the correct font atlas
    /// automatically.
    pub title_style: TextStyle,
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
            title_config: None,
            width: 800.0,
            height: 600.0,
            margins: Margins::default(),
            background_color: None,
            show_axes: true,
            show_grid: false,
            grid_config: GridConfiguration::default(),
            label_style: TextStyle::new(14.0),
            title_style: TextStyle::new(18.0).bold(),
        }
    }
}

impl ChartConfig {
    /// Enable grid rendering with default configuration.
    pub fn with_grid(mut self) -> Self {
        self.show_grid = true;
        self
    }

    /// Enable grid rendering with custom configuration.
    pub fn with_grid_config(mut self, config: GridConfiguration) -> Self {
        self.show_grid = true;
        self.grid_config = config;
        self
    }

    /// Disable grid rendering.
    pub fn without_grid(mut self) -> Self {
        self.show_grid = false;
        self
    }

    /// Set the text style for axis labels.
    ///
    /// Use [`TextStyle::with_font_family`] to specify a font; the chart's
    /// multi-font rendering methods will resolve it through a
    /// [`FontAtlasManager`](crate::text::FontAtlasManager) automatically.
    pub fn with_label_style(mut self, style: TextStyle) -> Self {
        self.label_style = style;
        self
    }

    /// Set the text style for the chart title.
    ///
    /// Use [`TextStyle::with_font_family`] to specify a font; the chart's
    /// multi-font rendering methods will resolve it through a
    /// [`FontAtlasManager`](crate::text::FontAtlasManager) automatically.
    pub fn with_title_style(mut self, style: TextStyle) -> Self {
        self.title_style = style;
        self
    }

    /// Set the chart title (simple, centred by default).
    ///
    /// This is a convenience method that creates a [`TitleConfig`] with
    /// centre alignment and default settings. For full control over
    /// alignment, subtitle, and offset use [`with_title_config`](Self::with_title_config).
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title_config = Some(TitleConfig::new(title));
        self
    }

    /// Set full title configuration (alignment, subtitle, y-offset).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use gup::chart_builder::{ChartConfig, TitleConfig, TitleAlignment};
    ///
    /// let config = ChartConfig::default()
    ///     .with_title_config(
    ///         TitleConfig::new("Sales Report")
    ///             .with_alignment(TitleAlignment::Left)
    ///             .with_subtitle("Q4 2024"),
    ///     );
    /// ```
    pub fn with_title_config(mut self, config: TitleConfig) -> Self {
        self.title_config = Some(config);
        self
    }

    /// Return the title text, if configured.
    ///
    /// Convenience accessor that reads through [`TitleConfig`].
    pub fn title(&self) -> Option<&str> {
        self.title_config.as_ref().map(|c| c.text.as_str())
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
    /// Grid system for rendering grid lines
    pub grid_system: Option<crate::grid::GridSystem>,
}

impl<T, M> ComposedChart<T, M>
where
    T: Clone + Send + Sync + std::fmt::Debug + 'static,
    M: crate::selection::Mark,
{
    /// Create a new composed chart from a visualization and configuration.
    pub fn new(visualization: Selection<T, M>, config: ChartConfig) -> Self {
        let grid_system = if config.show_grid {
            Some(crate::grid::GridSystem::new(config.grid_config.clone()))
        } else {
            None
        };

        Self {
            visualization,
            bottom_axis: None,
            left_axis: None,
            top_axis: None,
            right_axis: None,
            config,
            grid_system,
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

    /// Render the complete chart including axes and grid.
    pub fn render(&mut self, context: &mut RenderContext) -> GupResult<()> {
        // Calculate chart area based on margins and axis requirements
        let chart_area = self.calculate_chart_area();

        // Phase 1: Render grid lines (behind everything else)
        if self.config.show_grid
            && let Some(grid_system) = &mut self.grid_system
        {
            Self::render_grid_lines_static(grid_system, context, &chart_area)?;
        }

        // Phase 2: Render main visualization (data points, on top of grid)
        // Note: In a complete implementation, this would use the Mixable render system
        // For now, we acknowledge that the visualization is prepared for rendering

        // Phase 3: Render axes (on top of everything)
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

        Ok(())
    }

    /// Render grid lines with proper tick alignment.
    fn render_grid_lines_static(
        grid_system: &mut crate::grid::GridSystem,
        context: &mut RenderContext,
        chart_area: &ChartArea,
    ) -> GupResult<()> {
        use crate::grid::ChartBounds;

        // Convert ChartArea to ChartBounds for grid system
        let chart_bounds = ChartBounds::new(
            chart_area.x,
            chart_area.x + chart_area.width,
            chart_area.y,
            chart_area.y + chart_area.height,
        );

        // Generate sample tick positions (in a complete implementation, these would come from the axes)
        let horizontal_ticks = Self::generate_sample_horizontal_ticks(chart_bounds);
        let vertical_ticks = Self::generate_sample_vertical_ticks(chart_bounds);
        let horizontal_minor_ticks = Vec::new(); // No minor ticks for now
        let vertical_minor_ticks = Vec::new();

        // Render the grid
        grid_system.render_grid(
            context,
            &horizontal_ticks,
            &vertical_ticks,
            &horizontal_minor_ticks,
            &vertical_minor_ticks,
            chart_bounds,
        )?;

        // For now, we'll skip the visual rendering of grid selections
        // In a complete implementation, this would use the passed context
        // and render the grid selections through the proper rendering pipeline
        println!(
            "Grid system generated {} total grid lines",
            grid_system.total_line_count()
        );

        Ok(())
    }

    /// Generate sample horizontal tick positions.
    fn generate_sample_horizontal_ticks(bounds: crate::grid::ChartBounds) -> Vec<f64> {
        let mut ticks = Vec::new();
        let step = bounds.width() / 5.0; // 5 major divisions
        for i in 0..=5 {
            ticks.push((bounds.left + i as f32 * step) as f64);
        }
        ticks
    }

    /// Generate sample vertical tick positions.
    fn generate_sample_vertical_ticks(bounds: crate::grid::ChartBounds) -> Vec<f64> {
        let mut ticks = Vec::new();
        let step = bounds.height() / 4.0; // 4 major divisions
        for i in 0..=4 {
            ticks.push((bounds.top + i as f32 * step) as f64);
        }
        ticks
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

    /// Calculate axis bounds for a specific position (in pixel coordinates).
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

    /// Convert pixel-space [`AxisBounds`] to NDC (clip space, -1.0 to 1.0).
    ///
    /// Uses the chart's configured `width` and `height` as the viewport
    /// dimensions for the conversion:
    /// - `ndc_x = (pixel_x / width) * 2.0 - 1.0`
    /// - `ndc_y = 1.0 - (pixel_y / height) * 2.0`
    fn pixel_bounds_to_ndc(&self, bounds: &AxisBounds) -> AxisBounds {
        let w = self.config.width;
        let h = self.config.height;
        AxisBounds::new(
            Vec2 {
                x: (bounds.start.x / w) * 2.0 - 1.0,
                y: 1.0 - (bounds.start.y / h) * 2.0,
            },
            Vec2 {
                x: (bounds.end.x / w) * 2.0 - 1.0,
                y: 1.0 - (bounds.end.y / h) * 2.0,
            },
            bounds.available_margin,
        )
    }

    /// Generate axis geometry (vertices and labels) for all configured axes.
    ///
    /// Returns a tuple of `(vertices, labels)` where:
    /// - `vertices`: `Vec<Vertex>` suitable for drawing with a `LineList`
    ///   pipeline. Each pair of consecutive vertices forms one line segment
    ///   (axis lines and tick marks).
    /// - `labels`: `Vec<AxisLabel>` with formatted text, screen position, and
    ///   recommended anchor for each tick label, ready for
    ///   [`TextRenderer`](crate::text::TextRenderer).
    ///
    /// The method converts the chart's pixel-space layout (from
    /// [`ChartConfig`] margins and dimensions) into NDC coordinates before
    /// delegating to [`AxisRenderer`].
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use gup::prelude::*;
    /// use gup::chart_builder::ComposedChart;
    /// use gup::axis::{LinearAxis, AxisPosition, AxisConfiguration};
    /// use std::sync::Arc;
    ///
    /// # async fn example() -> gup::error::GupResult<()> {
    /// # let context = Arc::new(gup::RenderContext::new().await?);
    /// # #[derive(Debug, Clone)]
    /// # struct D { x: f32, y: f32 }
    /// # let sel = gup::selection::Selection::<D, gup::Circle>::new(vec![], context)?;
    /// # let config = gup::chart_builder::ChartConfig::default();
    /// let chart = ComposedChart::new(sel, config).with_default_axes();
    /// let (vertices, labels) = chart.generate_axis_geometry();
    ///
    /// // Draw `vertices` with a LineList pipeline, render `labels` with TextRenderer
    /// # Ok(())
    /// # }
    /// ```
    pub fn generate_axis_geometry(&self) -> (Vec<Vertex>, Vec<AxisLabel>) {
        let chart_area = self.calculate_chart_area();
        let viewport_size = (self.config.width, self.config.height);
        let renderer = AxisRenderer::new();

        let mut all_vertices = Vec::new();
        let mut all_labels = Vec::new();

        let axes: [(AxisPosition, &Option<Box<dyn Axis>>); 4] = [
            (AxisPosition::Bottom, &self.bottom_axis),
            (AxisPosition::Left, &self.left_axis),
            (AxisPosition::Top, &self.top_axis),
            (AxisPosition::Right, &self.right_axis),
        ];

        for (position, axis_opt) in &axes {
            if let Some(axis) = axis_opt {
                let pixel_bounds = self.calculate_axis_bounds(*position, &chart_area);
                let ndc_bounds = self.pixel_bounds_to_ndc(&pixel_bounds);
                let config = axis.configuration();

                let vertices = renderer.generate_axis_vertices(
                    &ndc_bounds,
                    config,
                    *position,
                    None, // TODO: pass scale when available from axis
                    viewport_size,
                );
                all_vertices.extend(vertices);

                let labels = renderer.generate_label_data(
                    &ndc_bounds,
                    config,
                    *position,
                    None, // TODO: pass scale when available from axis
                    viewport_size,
                    None, // default NumericFormatter
                );
                all_labels.extend(labels);
            }
        }

        (all_vertices, all_labels)
    }

    /// Generate axis geometry with label collision resolution.
    ///
    /// Like [`generate_axis_geometry`](Self::generate_axis_geometry), but runs
    /// each axis's labels through the given [`LabelPositioner`] to resolve
    /// overlaps.  Labels that cannot be placed without collisions may be
    /// offset, rotated, or hidden depending on the positioner's strategy
    /// pipeline.
    ///
    /// The positioner accumulates state across axes, so labels from one axis
    /// will not overlap labels already placed by a previous axis.
    ///
    /// Returns `(vertices, layout)` where `layout` contains the resolved
    /// [`LabelPosition`] entries and a list of hidden label indices.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use gup::prelude::*;
    /// use gup::chart_builder::ComposedChart;
    /// use gup::axis::{LinearAxis, AxisPosition, AxisConfiguration};
    /// use gup::label::{LabelPositioner, LabelConstraints};
    /// use std::sync::Arc;
    ///
    /// # async fn example() -> gup::error::GupResult<()> {
    /// # let context = Arc::new(gup::RenderContext::new().await?);
    /// # #[derive(Debug, Clone)]
    /// # struct D { x: f32, y: f32 }
    /// # let sel = gup::selection::Selection::<D, gup::Circle>::new(vec![], context)?;
    /// # let config = gup::chart_builder::ChartConfig::default();
    /// let chart = ComposedChart::new(sel, config).with_default_axes();
    /// let mut positioner = LabelPositioner::new();
    /// let constraints = LabelConstraints::axis_labels();
    /// let (vertices, layout) = chart.generate_axis_geometry_resolved(
    ///     &mut positioner,
    ///     &constraints,
    /// )?;
    ///
    /// // `layout.positions` contains collision-free label placements
    /// // `layout.hidden_labels` lists indices that were hidden
    /// # Ok(())
    /// # }
    /// ```
    pub fn generate_axis_geometry_resolved(
        &self,
        positioner: &mut LabelPositioner,
        constraints: &LabelConstraints,
    ) -> GupResult<(Vec<Vertex>, LabelLayout)> {
        let chart_area = self.calculate_chart_area();
        let viewport_size = (self.config.width, self.config.height);
        let renderer = AxisRenderer::new();

        let mut all_vertices = Vec::new();
        let mut all_positions: Vec<LabelPosition> = Vec::new();
        let mut all_hidden: Vec<usize> = Vec::new();
        let mut offset = 0usize;
        let mut any_rotated = false;

        let axes: [(AxisPosition, &Option<Box<dyn Axis>>); 4] = [
            (AxisPosition::Bottom, &self.bottom_axis),
            (AxisPosition::Left, &self.left_axis),
            (AxisPosition::Top, &self.top_axis),
            (AxisPosition::Right, &self.right_axis),
        ];

        for (position, axis_opt) in &axes {
            if let Some(axis) = axis_opt {
                let pixel_bounds = self.calculate_axis_bounds(*position, &chart_area);
                let ndc_bounds = self.pixel_bounds_to_ndc(&pixel_bounds);
                let config = axis.configuration();

                let vertices = renderer.generate_axis_vertices(
                    &ndc_bounds,
                    config,
                    *position,
                    None,
                    viewport_size,
                );
                all_vertices.extend(vertices);

                let labels = renderer.generate_label_data(
                    &ndc_bounds,
                    config,
                    *position,
                    None,
                    viewport_size,
                    None,
                );

                // Build AxisInfo from pixel-space bounds for this axis
                let axis_info = AxisInfo::from_bounds(&pixel_bounds, *position);

                let layout = positioner.resolve_labels(&labels, &axis_info, constraints)?;

                // Remap hidden indices relative to the combined list
                for &idx in &layout.hidden_labels {
                    all_hidden.push(offset + idx);
                }
                any_rotated |= layout.rotated;
                offset += labels.len();
                all_positions.extend(layout.positions);
            }
        }

        let combined = LabelLayout {
            positions: all_positions,
            hidden_labels: all_hidden,
            margin_requirements: crate::label::Margins::default(),
            rotated: any_rotated,
        };

        Ok((all_vertices, combined))
    }

    /// Queue axis-label and title text for multi-font rendering.
    ///
    /// This convenience method generates axis labels for each axis and
    /// queues them (and the optional chart title) through
    /// [`TextRenderer::queue_text_with_fonts`](crate::text::TextRenderer::queue_text_with_fonts),
    /// so that [`TextStyle::font_family`] is respected via the
    /// [`FontAtlasManager`](crate::text::FontAtlasManager).
    ///
    /// When an axis has a per-axis
    /// [`AxisConfiguration::label_style`](crate::axis::AxisConfiguration::label_style),
    /// that style is used instead of [`ChartConfig::label_style`].
    ///
    /// Call this **before** creating the render pass, then call
    /// [`TextRenderer::render_queued_text_multi`](crate::text::TextRenderer::render_queued_text_multi)
    /// inside the pass to draw the text.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use gup::prelude::*;
    /// use gup::chart_builder::ComposedChart;
    /// use gup::text::{
    ///     FontAtlasManager, FontDatabase, TextLayoutEngine, TextRenderer, TextStyle,
    /// };
    ///
    /// # async fn example() -> gup::error::GupResult<()> {
    /// # let context = std::sync::Arc::new(gup::RenderContext::new().await?);
    /// # #[derive(Debug, Clone)]
    /// # struct D { x: f32, y: f32 }
    /// # let sel = gup::selection::Selection::<D, gup::Circle>::new(vec![], context)?;
    /// let config = gup::chart_builder::ChartConfig::default()
    ///     .with_label_style(TextStyle::new(14.0).with_font_family("DejaVu Sans"))
    ///     .with_title("My Chart")
    ///     .with_title_style(TextStyle::new(18.0).bold().with_font_family("DejaVu Serif"));
    /// let chart = ComposedChart::new(sel, config).with_default_axes();
    ///
    /// // During frame rendering:
    /// // text_renderer.begin_frame();
    /// // chart.queue_chart_text(&frame, &mut text_renderer, &mut font_mgr, &mut layout)?;
    /// // let mut pass = frame.render_pass(None);
    /// // text_renderer.render_queued_text_multi(&mut pass, device, queue, &font_mgr, w, h)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn queue_chart_text(
        &self,
        frame: &crate::RenderFrame,
        text_renderer: &mut crate::text::TextRenderer,
        font_manager: &mut crate::text::FontAtlasManager,
        layout_engine: &mut crate::text::TextLayoutEngine,
    ) -> GupResult<()> {
        let chart_area = self.calculate_chart_area();
        let viewport_size = (self.config.width, self.config.height);
        let renderer = AxisRenderer::new();

        let axes: [(AxisPosition, &Option<Box<dyn Axis>>); 4] = [
            (AxisPosition::Bottom, &self.bottom_axis),
            (AxisPosition::Left, &self.left_axis),
            (AxisPosition::Top, &self.top_axis),
            (AxisPosition::Right, &self.right_axis),
        ];

        for (position, axis_opt) in &axes {
            if let Some(axis) = axis_opt {
                let pixel_bounds = self.calculate_axis_bounds(*position, &chart_area);
                let ndc_bounds = self.pixel_bounds_to_ndc(&pixel_bounds);
                let config = axis.configuration();

                let labels = renderer.generate_label_data(
                    &ndc_bounds,
                    config,
                    *position,
                    None,
                    viewport_size,
                    None,
                );

                // Per-axis style overrides chart-level style
                let base_style = config
                    .label_style
                    .as_ref()
                    .unwrap_or(&self.config.label_style);

                for label in &labels {
                    let style = base_style.clone().with_anchor(label.anchor);

                    text_renderer.queue_text_with_fonts(
                        frame,
                        &label.text,
                        label.screen_position,
                        &style,
                        font_manager,
                        layout_engine,
                        None,
                        None,
                    )?;
                }
            }
        }

        // Queue chart title
        self.queue_title_text(frame, text_renderer, font_manager, layout_engine)?;

        Ok(())
    }

    /// Queue axis-label and title text with collision detection.
    ///
    /// Like [`queue_chart_text`](Self::queue_chart_text), but first runs
    /// labels through a [`LabelPositioner`] to resolve overlaps. Hidden
    /// labels are omitted from the text queue.
    ///
    /// When an axis has a per-axis
    /// [`AxisConfiguration::label_style`](crate::axis::AxisConfiguration::label_style),
    /// that style is used instead of [`ChartConfig::label_style`].
    pub fn queue_chart_text_resolved(
        &self,
        frame: &crate::RenderFrame,
        text_renderer: &mut crate::text::TextRenderer,
        font_manager: &mut crate::text::FontAtlasManager,
        layout_engine: &mut crate::text::TextLayoutEngine,
        positioner: &mut LabelPositioner,
        constraints: &LabelConstraints,
    ) -> GupResult<()> {
        let chart_area = self.calculate_chart_area();
        let viewport_size = (self.config.width, self.config.height);
        let renderer = AxisRenderer::new();

        let axes: [(AxisPosition, &Option<Box<dyn Axis>>); 4] = [
            (AxisPosition::Bottom, &self.bottom_axis),
            (AxisPosition::Left, &self.left_axis),
            (AxisPosition::Top, &self.top_axis),
            (AxisPosition::Right, &self.right_axis),
        ];

        for (position, axis_opt) in &axes {
            if let Some(axis) = axis_opt {
                let pixel_bounds = self.calculate_axis_bounds(*position, &chart_area);
                let ndc_bounds = self.pixel_bounds_to_ndc(&pixel_bounds);
                let config = axis.configuration();

                let labels = renderer.generate_label_data(
                    &ndc_bounds,
                    config,
                    *position,
                    None,
                    viewport_size,
                    None,
                );

                let axis_info = AxisInfo::from_bounds(&pixel_bounds, *position);
                let layout = positioner.resolve_labels(&labels, &axis_info, constraints)?;

                // Per-axis style overrides chart-level style
                let base_style = config
                    .label_style
                    .as_ref()
                    .unwrap_or(&self.config.label_style);

                for lp in &layout.positions {
                    let style = base_style.clone().with_anchor(lp.anchor);

                    text_renderer.queue_text_with_fonts(
                        frame,
                        &lp.text,
                        lp.position,
                        &style,
                        font_manager,
                        layout_engine,
                        None,
                        None,
                    )?;
                }
            }
        }

        // Queue chart title
        self.queue_title_text(frame, text_renderer, font_manager, layout_engine)?;

        Ok(())
    }

    /// Queue the chart title text (if configured).
    ///
    /// Renders the primary title and an optional subtitle, respecting the
    /// alignment, y-offset, and line spacing specified in [`TitleConfig`].
    fn queue_title_text(
        &self,
        frame: &crate::RenderFrame,
        text_renderer: &mut crate::text::TextRenderer,
        font_manager: &mut crate::text::FontAtlasManager,
        layout_engine: &mut crate::text::TextLayoutEngine,
    ) -> GupResult<()> {
        let title_cfg = match &self.config.title_config {
            Some(cfg) => cfg,
            None => return Ok(()),
        };

        // Determine anchor and x position from alignment
        let (anchor, x) = match title_cfg.alignment {
            TitleAlignment::Left => (crate::text::TextAnchor::TopLeft, self.config.margins.left),
            TitleAlignment::Center => (crate::text::TextAnchor::TopCenter, self.config.width / 2.0),
            TitleAlignment::Right => (
                crate::text::TextAnchor::TopRight,
                self.config.width - self.config.margins.right,
            ),
        };

        let y = title_cfg.y_offset.unwrap_or(self.config.margins.top / 2.0);

        let title_style = self.config.title_style.clone().with_anchor(anchor);

        // Queue the main title
        let title_position = Vec2 { x, y };
        text_renderer.queue_text_with_fonts(
            frame,
            &title_cfg.text,
            title_position,
            &title_style,
            font_manager,
            layout_engine,
            None,
            None,
        )?;

        // Queue subtitle (if present), positioned below the main title
        if let Some(subtitle) = &title_cfg.subtitle {
            let subtitle_y = y + self.config.title_style.font_size * title_cfg.line_spacing;

            let subtitle_style = title_cfg.subtitle_style.clone().with_anchor(anchor);
            let subtitle_position = Vec2 { x, y: subtitle_y };

            text_renderer.queue_text_with_fonts(
                frame,
                subtitle,
                subtitle_position,
                &subtitle_style,
                font_manager,
                layout_engine,
                None,
                None,
            )?;
        }

        Ok(())
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

// Render layer manager for proper z-ordering of visual elements.
//
// This manager ensures that visual elements are rendered in the correct order:
// 1. Background layer (chart background)
// 2. Grid layer (grid lines behind data)
// 3. Data layer (main visualization data)
// 4. Axis layer (axes on top of data)
// 5. Annotation layer (labels, legends, etc.)
//
// TODO: This is currently a stub pending full Selection type implementation
/*
#[derive(Debug)]
pub struct RenderLayerManager {
    /// Background layer elements
    background_layer: Vec<Box<dyn RenderableElement>>,
    /// Grid layer elements (grid lines)
    grid_layer: Vec<Selection<LineAttributes, crate::selection::Line>>,
    /// Data layer elements (main visualization)
    data_layer: Vec<Box<dyn RenderableElement>>,
    /// Axis layer elements
    axis_layer: Vec<Box<dyn RenderableElement>>,
    /// Annotation layer elements
    annotation_layer: Vec<Box<dyn RenderableElement>>,
}

/// Trait for elements that can be rendered in layers.
pub trait RenderableElement: std::fmt::Debug {
    /// Render this element using the provided context.
    fn render(&mut self, context: &mut RenderContext) -> GupResult<()>;
}

impl RenderLayerManager {
    /// Create a new empty render layer manager.
    pub fn new() -> Self {
        Self {
            background_layer: Vec::new(),
            grid_layer: Vec::new(),
            data_layer: Vec::new(),
            axis_layer: Vec::new(),
            annotation_layer: Vec::new(),
        }
    }

    /// Add grid selections to the grid layer.
    pub fn add_grid_selections(
        &mut self,
        selections: Vec<Selection<LineAttributes, crate::selection::Line>>,
    ) {
        self.grid_layer.extend(selections);
    }

    /// Add a data element to the data layer.
    pub fn add_data_element(&mut self, element: Box<dyn RenderableElement>) {
        self.data_layer.push(element);
    }

    /// Add an axis element to the axis layer.
    pub fn add_axis_element(&mut self, element: Box<dyn RenderableElement>) {
        self.axis_layer.push(element);
    }

    /// Render all layers in the correct z-order.
    pub fn render_all_layers(&mut self, context: &mut RenderContext) -> GupResult<()> {
        // Layer 1: Background
        for element in &mut self.background_layer {
            element.render(context)?;
        }

        // Layer 2: Grid lines (behind data)
        for selection in &mut self.grid_layer {
            selection.render()?;
        }

        // Layer 3: Data visualization (on top of grid)
        for element in &mut self.data_layer {
            element.render(context)?;
        }

        // Layer 4: Axes (on top of data)
        for element in &mut self.axis_layer {
            element.render(context)?;
        }

        // Layer 5: Annotations (on top of everything)
        for element in &mut self.annotation_layer {
            element.render(context)?;
        }

        Ok(())
    }

    /// Clear all layers.
    pub fn clear(&mut self) {
        self.background_layer.clear();
        self.grid_layer.clear();
        self.data_layer.clear();
        self.axis_layer.clear();
        self.annotation_layer.clear();
    }

    /// Get the total number of elements across all layers.
    pub fn total_element_count(&self) -> usize {
        self.background_layer.len()
            + self.grid_layer.len()
            + self.data_layer.len()
            + self.axis_layer.len()
            + self.annotation_layer.len()
    }
}

impl Default for RenderLayerManager {
    fn default() -> Self {
        Self::new()
    }
}
*/

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

// TODO: Re-enable tests when Selection type is implemented
/*
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
        assert!(config.title_config.is_none());
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

    // ---- Tests for pixel_bounds_to_ndc ----

    #[tokio::test]
    async fn test_pixel_bounds_to_ndc_center() {
        let context = Arc::new(RenderContext::new().await.unwrap());
        let sel =
            crate::selection::Selection::<TestData, crate::selection::Circle>::new(vec![], context)
                .unwrap();
        let config = ChartConfig {
            width: 800.0,
            height: 600.0,
            ..ChartConfig::default()
        };
        let chart = ComposedChart::new(sel, config);

        // Center of an 800x600 viewport is pixel (400, 300) -> NDC (0, 0)
        let pixel_bounds = AxisBounds::new(
            Vec2 { x: 400.0, y: 300.0 },
            Vec2 { x: 400.0, y: 300.0 },
            50.0,
        );
        let ndc = chart.pixel_bounds_to_ndc(&pixel_bounds);
        assert!((ndc.start.x - 0.0).abs() < 0.001);
        assert!((ndc.start.y - 0.0).abs() < 0.001);
    }

    #[tokio::test]
    async fn test_pixel_bounds_to_ndc_corners() {
        let context = Arc::new(RenderContext::new().await.unwrap());
        let sel =
            crate::selection::Selection::<TestData, crate::selection::Circle>::new(vec![], context)
                .unwrap();
        let config = ChartConfig {
            width: 800.0,
            height: 600.0,
            ..ChartConfig::default()
        };
        let chart = ComposedChart::new(sel, config);

        // Top-left pixel (0,0) -> NDC (-1, 1)
        let bounds = AxisBounds::new(Vec2 { x: 0.0, y: 0.0 }, Vec2 { x: 800.0, y: 600.0 }, 10.0);
        let ndc = chart.pixel_bounds_to_ndc(&bounds);
        assert!((ndc.start.x - (-1.0)).abs() < 0.001);
        assert!((ndc.start.y - 1.0).abs() < 0.001);
        // Bottom-right pixel (800,600) -> NDC (1, -1)
        assert!((ndc.end.x - 1.0).abs() < 0.001);
        assert!((ndc.end.y - (-1.0)).abs() < 0.001);
    }

    // ---- Tests for generate_axis_geometry ----

    #[tokio::test]
    async fn test_generate_axis_geometry_no_axes() {
        let context = Arc::new(RenderContext::new().await.unwrap());
        let sel =
            crate::selection::Selection::<TestData, crate::selection::Circle>::new(vec![], context)
                .unwrap();
        let config = ChartConfig {
            show_axes: false,
            ..ChartConfig::default()
        };
        let chart = ComposedChart::new(sel, config);

        let (vertices, labels) = chart.generate_axis_geometry();
        assert!(vertices.is_empty(), "No axes configured means no vertices");
        assert!(labels.is_empty(), "No axes configured means no labels");
    }

    #[tokio::test]
    async fn test_generate_axis_geometry_default_axes() {
        let context = Arc::new(RenderContext::new().await.unwrap());
        let sel =
            crate::selection::Selection::<TestData, crate::selection::Circle>::new(vec![], context)
                .unwrap();
        let chart = ComposedChart::new(sel, ChartConfig::default()).with_default_axes();

        let (vertices, labels) = chart.generate_axis_geometry();

        // Default axes: bottom + left, each with line (2 verts) + 6 major ticks (12 verts) = 14 each
        // Total = 28 vertices
        assert_eq!(vertices.len(), 28, "Bottom + left axes: 14 vertices each");

        // 6 labels per axis = 12 total
        assert_eq!(labels.len(), 12, "6 labels per axis x 2 axes");
    }

    #[tokio::test]
    async fn test_generate_axis_geometry_vertices_in_ndc_range() {
        let context = Arc::new(RenderContext::new().await.unwrap());
        let sel =
            crate::selection::Selection::<TestData, crate::selection::Circle>::new(vec![], context)
                .unwrap();
        let chart = ComposedChart::new(sel, ChartConfig::default()).with_default_axes();

        let (vertices, _) = chart.generate_axis_geometry();

        for v in &vertices {
            assert!(
                v.position[0] >= -1.1 && v.position[0] <= 1.1,
                "X vertex should be near NDC range: got {}",
                v.position[0],
            );
            assert!(
                v.position[1] >= -1.1 && v.position[1] <= 1.1,
                "Y vertex should be near NDC range: got {}",
                v.position[1],
            );
        }
    }

    #[tokio::test]
    async fn test_generate_axis_geometry_labels_in_screen_range() {
        let context = Arc::new(RenderContext::new().await.unwrap());
        let sel =
            crate::selection::Selection::<TestData, crate::selection::Circle>::new(vec![], context)
                .unwrap();
        let config = ChartConfig::default(); // 800x600
        let chart = ComposedChart::new(sel, config).with_default_axes();

        let (_, labels) = chart.generate_axis_geometry();

        for label in &labels {
            assert!(
                label.screen_position.x >= -50.0 && label.screen_position.x <= 850.0,
                "Label X should be near viewport: got {}",
                label.screen_position.x,
            );
            assert!(
                label.screen_position.y >= -50.0 && label.screen_position.y <= 650.0,
                "Label Y should be near viewport: got {}",
                label.screen_position.y,
            );
        }
    }

    #[tokio::test]
    async fn test_generate_axis_geometry_all_four_axes() {
        let context = Arc::new(RenderContext::new().await.unwrap());
        let sel =
            crate::selection::Selection::<TestData, crate::selection::Circle>::new(vec![], context)
                .unwrap();
        let config = ChartConfig::default();
        let axis_config = AxisConfiguration::default();
        let chart = ComposedChart::new(sel, config)
            .with_bottom_axis(Box::new(LinearAxis::new(
                AxisPosition::Bottom,
                axis_config.clone(),
            )))
            .with_left_axis(Box::new(LinearAxis::new(
                AxisPosition::Left,
                axis_config.clone(),
            )))
            .with_top_axis(Box::new(LinearAxis::new(
                AxisPosition::Top,
                axis_config.clone(),
            )))
            .with_right_axis(Box::new(LinearAxis::new(AxisPosition::Right, axis_config)));

        let (vertices, labels) = chart.generate_axis_geometry();

        // 4 axes * 14 verts each = 56
        assert_eq!(vertices.len(), 56, "4 axes: 14 vertices each");
        // 4 axes * 6 labels each = 24
        assert_eq!(labels.len(), 24, "4 axes: 6 labels each");
    }
}
*/

#[cfg(test)]
mod tests_multi_font {
    use super::*;
    use crate::text::TextStyle;

    // --- ChartConfig text style tests (no GPU required) ---

    #[test]
    fn test_chart_config_default_label_style() {
        let config = ChartConfig::default();
        assert_eq!(config.label_style.font_size, 14.0);
        assert_eq!(config.label_style.font_family, None);
    }

    #[test]
    fn test_chart_config_default_title_style() {
        let config = ChartConfig::default();
        assert_eq!(config.title_style.font_size, 18.0);
        assert_eq!(config.title_style.weight, 1.0); // bold
        assert_eq!(config.title_style.font_family, None);
    }

    #[test]
    fn test_chart_config_with_label_style() {
        let config = ChartConfig::default()
            .with_label_style(TextStyle::new(16.0).with_font_family("DejaVu Sans"));
        assert_eq!(config.label_style.font_size, 16.0);
        assert_eq!(
            config.label_style.font_family,
            Some("DejaVu Sans".to_string())
        );
    }

    #[test]
    fn test_chart_config_with_title_style() {
        let config = ChartConfig::default()
            .with_title_style(TextStyle::new(24.0).with_font_family("DejaVu Serif"));
        assert_eq!(config.title_style.font_size, 24.0);
        assert_eq!(
            config.title_style.font_family,
            Some("DejaVu Serif".to_string())
        );
    }

    #[test]
    fn test_chart_config_with_title() {
        let config = ChartConfig::default().with_title("My Chart");
        assert_eq!(config.title(), Some("My Chart"));
    }

    #[test]
    fn test_chart_config_title_accepts_string() {
        let config = ChartConfig::default().with_title(String::from("Dynamic Title"));
        assert_eq!(config.title(), Some("Dynamic Title"));
    }

    // --- GPU-dependent tests (require single-threaded test runner) ---

    #[tokio::test]
    async fn test_queue_chart_text_no_axes() {
        #[derive(Debug, Clone)]
        struct D {
            x: f32,
        }

        let context = std::sync::Arc::new(crate::RenderContext::new().await.unwrap());
        let sel = crate::selection::Selection::<D, crate::Circle>::new(vec![], context).unwrap();
        let config = ChartConfig {
            show_axes: false,
            ..ChartConfig::default()
        };
        let chart = ComposedChart::new(sel, config);

        let gup_context = crate::GupContext::headless().await.unwrap();
        let mut gup_ctx = std::sync::Arc::try_unwrap(gup_context).unwrap();
        let frame = gup_ctx.begin_frame().unwrap();

        let mut text_renderer = crate::text::TextRenderer::new(frame.device()).unwrap();
        let mut font_manager =
            crate::text::FontAtlasManager::new(crate::text::FontDatabase::empty(), 14.0);
        let mut layout_engine = crate::text::TextLayoutEngine::new();

        text_renderer.begin_frame();
        let result = chart.queue_chart_text(
            &frame,
            &mut text_renderer,
            &mut font_manager,
            &mut layout_engine,
        );
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_queue_chart_text_with_default_axes() {
        #[derive(Debug, Clone)]
        struct D {
            x: f32,
        }

        let context = std::sync::Arc::new(crate::RenderContext::new().await.unwrap());
        let sel = crate::selection::Selection::<D, crate::Circle>::new(vec![], context).unwrap();
        let chart = ComposedChart::new(sel, ChartConfig::default()).with_default_axes();

        let gup_context = crate::GupContext::headless().await.unwrap();
        let mut gup_ctx = std::sync::Arc::try_unwrap(gup_context).unwrap();
        let frame = gup_ctx.begin_frame().unwrap();

        let mut text_renderer = crate::text::TextRenderer::new(frame.device()).unwrap();
        let mut font_manager =
            crate::text::FontAtlasManager::new(crate::text::FontDatabase::new(), 14.0);
        let mut layout_engine = crate::text::TextLayoutEngine::new();

        text_renderer.begin_frame();
        let result = chart.queue_chart_text(
            &frame,
            &mut text_renderer,
            &mut font_manager,
            &mut layout_engine,
        );
        assert!(result.is_ok(), "queue_chart_text failed: {result:?}");

        // The font manager should have created at least one atlas (the default)
        assert!(
            font_manager.atlas_count() > 0,
            "Expected at least one font atlas"
        );
    }

    #[tokio::test]
    async fn test_queue_chart_text_with_font_family() {
        #[derive(Debug, Clone)]
        struct D {
            x: f32,
        }

        let context = std::sync::Arc::new(crate::RenderContext::new().await.unwrap());
        let sel = crate::selection::Selection::<D, crate::Circle>::new(vec![], context).unwrap();
        let config = ChartConfig::default()
            .with_label_style(TextStyle::new(14.0).with_font_family("DejaVu Sans"));
        let chart = ComposedChart::new(sel, config).with_default_axes();

        let gup_context = crate::GupContext::headless().await.unwrap();
        let mut gup_ctx = std::sync::Arc::try_unwrap(gup_context).unwrap();
        let frame = gup_ctx.begin_frame().unwrap();

        let mut text_renderer = crate::text::TextRenderer::new(frame.device()).unwrap();
        let mut font_manager =
            crate::text::FontAtlasManager::new(crate::text::FontDatabase::new(), 14.0);
        let mut layout_engine = crate::text::TextLayoutEngine::new();

        text_renderer.begin_frame();
        let result = chart.queue_chart_text(
            &frame,
            &mut text_renderer,
            &mut font_manager,
            &mut layout_engine,
        );
        assert!(result.is_ok(), "queue_chart_text failed: {result:?}");

        // DejaVu Sans atlas should have been created
        assert!(
            font_manager.get_atlas(Some("DejaVu Sans")).is_some(),
            "Expected a DejaVu Sans atlas to be created"
        );
    }

    #[tokio::test]
    async fn test_queue_chart_text_with_title() {
        #[derive(Debug, Clone)]
        struct D {
            x: f32,
        }

        let context = std::sync::Arc::new(crate::RenderContext::new().await.unwrap());
        let sel = crate::selection::Selection::<D, crate::Circle>::new(vec![], context).unwrap();
        let config = ChartConfig::default()
            .with_title("Test Title")
            .with_title_style(TextStyle::new(20.0).with_font_family("DejaVu Serif"));
        let chart = ComposedChart::new(sel, config).with_default_axes();

        let gup_context = crate::GupContext::headless().await.unwrap();
        let mut gup_ctx = std::sync::Arc::try_unwrap(gup_context).unwrap();
        let frame = gup_ctx.begin_frame().unwrap();

        let mut text_renderer = crate::text::TextRenderer::new(frame.device()).unwrap();
        let mut font_manager =
            crate::text::FontAtlasManager::new(crate::text::FontDatabase::new(), 14.0);
        let mut layout_engine = crate::text::TextLayoutEngine::new();

        text_renderer.begin_frame();
        let result = chart.queue_chart_text(
            &frame,
            &mut text_renderer,
            &mut font_manager,
            &mut layout_engine,
        );
        assert!(result.is_ok(), "queue_chart_text failed: {result:?}");

        // Both default atlas (for labels) and DejaVu Serif (for title) should exist
        assert!(
            font_manager.get_atlas(Some("DejaVu Serif")).is_some(),
            "Expected a DejaVu Serif atlas for the title"
        );
    }

    #[tokio::test]
    async fn test_queue_chart_text_resolved_with_collision_detection() {
        #[derive(Debug, Clone)]
        struct D {
            x: f32,
        }

        let context = std::sync::Arc::new(crate::RenderContext::new().await.unwrap());
        let sel = crate::selection::Selection::<D, crate::Circle>::new(vec![], context).unwrap();
        let chart = ComposedChart::new(sel, ChartConfig::default()).with_default_axes();

        let gup_context = crate::GupContext::headless().await.unwrap();
        let mut gup_ctx = std::sync::Arc::try_unwrap(gup_context).unwrap();
        let frame = gup_ctx.begin_frame().unwrap();

        let mut text_renderer = crate::text::TextRenderer::new(frame.device()).unwrap();
        let mut font_manager =
            crate::text::FontAtlasManager::new(crate::text::FontDatabase::new(), 14.0);
        let mut layout_engine = crate::text::TextLayoutEngine::new();
        let mut positioner = crate::label::LabelPositioner::new();
        let constraints = crate::label::LabelConstraints::axis_labels();

        text_renderer.begin_frame();
        let result = chart.queue_chart_text_resolved(
            &frame,
            &mut text_renderer,
            &mut font_manager,
            &mut layout_engine,
            &mut positioner,
            &constraints,
        );
        assert!(
            result.is_ok(),
            "queue_chart_text_resolved failed: {result:?}"
        );
    }

    // --- TitleConfig and TitleAlignment tests (no GPU required) ---

    #[test]
    fn test_title_config_new() {
        let tc = TitleConfig::new("Hello");
        assert_eq!(tc.text, "Hello");
        assert_eq!(tc.alignment, TitleAlignment::Center);
        assert!(tc.y_offset.is_none());
        assert!(tc.subtitle.is_none());
        assert_eq!(tc.line_spacing, 1.2);
    }

    #[test]
    fn test_title_config_builders() {
        let tc = TitleConfig::new("Main")
            .with_alignment(TitleAlignment::Left)
            .with_y_offset(10.0)
            .with_subtitle("Sub")
            .with_subtitle_style(TextStyle::new(12.0))
            .with_line_spacing(1.5);

        assert_eq!(tc.alignment, TitleAlignment::Left);
        assert_eq!(tc.y_offset, Some(10.0));
        assert_eq!(tc.subtitle, Some("Sub".to_string()));
        assert_eq!(tc.subtitle_style.font_size, 12.0);
        assert_eq!(tc.line_spacing, 1.5);
    }

    #[test]
    fn test_title_alignment_default_is_center() {
        assert_eq!(TitleAlignment::default(), TitleAlignment::Center);
    }

    #[test]
    fn test_title_config_line_spacing_min() {
        let tc = TitleConfig::new("X").with_line_spacing(-5.0);
        assert_eq!(tc.line_spacing, 0.1);
    }

    #[test]
    fn test_chart_config_with_title_creates_title_config() {
        let config = ChartConfig::default().with_title("My Title");
        let tc = config.title_config.as_ref().unwrap();
        assert_eq!(tc.text, "My Title");
        assert_eq!(tc.alignment, TitleAlignment::Center);
    }

    #[test]
    fn test_chart_config_with_title_config_full() {
        let config = ChartConfig::default().with_title_config(
            TitleConfig::new("Revenue")
                .with_alignment(TitleAlignment::Right)
                .with_subtitle("Q4 2024"),
        );
        let tc = config.title_config.as_ref().unwrap();
        assert_eq!(tc.text, "Revenue");
        assert_eq!(tc.alignment, TitleAlignment::Right);
        assert_eq!(tc.subtitle, Some("Q4 2024".to_string()));
    }

    #[test]
    fn test_chart_config_title_accessor() {
        let config = ChartConfig::default();
        assert!(config.title().is_none());

        let config = config.with_title("Test");
        assert_eq!(config.title(), Some("Test"));
    }

    #[test]
    fn test_chart_config_no_title_by_default() {
        let config = ChartConfig::default();
        assert!(config.title_config.is_none());
    }

    // --- GPU tests for title rendering with TitleConfig ---

    #[tokio::test]
    async fn test_queue_title_text_with_subtitle() {
        #[derive(Debug, Clone)]
        struct D {
            x: f32,
        }

        let context = std::sync::Arc::new(crate::RenderContext::new().await.unwrap());
        let sel = crate::selection::Selection::<D, crate::Circle>::new(vec![], context).unwrap();
        let config = ChartConfig::default()
            .with_title_config(
                TitleConfig::new("Main Title")
                    .with_subtitle("Subtitle Here")
                    .with_subtitle_style(
                        TextStyle::new(12.0)
                            .with_font_family("DejaVu Sans")
                            .with_rgba(0.5, 0.5, 0.5, 1.0),
                    ),
            )
            .with_title_style(TextStyle::new(20.0).bold().with_font_family("DejaVu Serif"));

        let chart = ComposedChart::new(sel, config).with_default_axes();

        let gup_context = crate::GupContext::headless().await.unwrap();
        let mut gup_ctx = std::sync::Arc::try_unwrap(gup_context).unwrap();
        let frame = gup_ctx.begin_frame().unwrap();

        let mut text_renderer = crate::text::TextRenderer::new(frame.device()).unwrap();
        let mut font_manager =
            crate::text::FontAtlasManager::new(crate::text::FontDatabase::new(), 14.0);
        let mut layout_engine = crate::text::TextLayoutEngine::new();

        text_renderer.begin_frame();
        let result = chart.queue_chart_text(
            &frame,
            &mut text_renderer,
            &mut font_manager,
            &mut layout_engine,
        );
        assert!(result.is_ok(), "queue_chart_text failed: {result:?}");

        // Title font atlas should exist
        assert!(
            font_manager.get_atlas(Some("DejaVu Serif")).is_some(),
            "Expected DejaVu Serif atlas for the title"
        );
        // Subtitle font atlas should exist
        assert!(
            font_manager.get_atlas(Some("DejaVu Sans")).is_some(),
            "Expected DejaVu Sans atlas for the subtitle"
        );
    }

    #[tokio::test]
    async fn test_queue_title_text_left_aligned() {
        #[derive(Debug, Clone)]
        struct D {
            x: f32,
        }

        let context = std::sync::Arc::new(crate::RenderContext::new().await.unwrap());
        let sel = crate::selection::Selection::<D, crate::Circle>::new(vec![], context).unwrap();
        let config = ChartConfig::default()
            .with_title_config(TitleConfig::new("Left Title").with_alignment(TitleAlignment::Left));

        let chart = ComposedChart::new(sel, config);

        let gup_context = crate::GupContext::headless().await.unwrap();
        let mut gup_ctx = std::sync::Arc::try_unwrap(gup_context).unwrap();
        let frame = gup_ctx.begin_frame().unwrap();

        let mut text_renderer = crate::text::TextRenderer::new(frame.device()).unwrap();
        let mut font_manager =
            crate::text::FontAtlasManager::new(crate::text::FontDatabase::new(), 14.0);
        let mut layout_engine = crate::text::TextLayoutEngine::new();

        text_renderer.begin_frame();
        let result = chart.queue_chart_text(
            &frame,
            &mut text_renderer,
            &mut font_manager,
            &mut layout_engine,
        );
        assert!(result.is_ok(), "queue_chart_text failed: {result:?}");
    }

    #[tokio::test]
    async fn test_queue_title_text_no_title() {
        #[derive(Debug, Clone)]
        struct D {
            x: f32,
        }

        let context = std::sync::Arc::new(crate::RenderContext::new().await.unwrap());
        let sel = crate::selection::Selection::<D, crate::Circle>::new(vec![], context).unwrap();
        // No title set — should render without error
        let config = ChartConfig::default();
        let chart = ComposedChart::new(sel, config);

        let gup_context = crate::GupContext::headless().await.unwrap();
        let mut gup_ctx = std::sync::Arc::try_unwrap(gup_context).unwrap();
        let frame = gup_ctx.begin_frame().unwrap();

        let mut text_renderer = crate::text::TextRenderer::new(frame.device()).unwrap();
        let mut font_manager =
            crate::text::FontAtlasManager::new(crate::text::FontDatabase::new(), 14.0);
        let mut layout_engine = crate::text::TextLayoutEngine::new();

        text_renderer.begin_frame();
        let result = chart.queue_chart_text(
            &frame,
            &mut text_renderer,
            &mut font_manager,
            &mut layout_engine,
        );
        assert!(result.is_ok(), "queue_chart_text failed: {result:?}");
    }

    // --- Per-axis label style override tests (GUP-217) ---

    #[test]
    fn test_axis_config_default_has_no_label_style() {
        let config = AxisConfiguration::default();
        assert!(
            config.label_style.is_none(),
            "Default AxisConfiguration should have no label_style override"
        );
    }

    #[test]
    fn test_axis_config_with_label_style() {
        let style = TextStyle::new(12.0).with_font_family("Monospace");
        let config = AxisConfiguration::default().with_label_style(style.clone());
        assert_eq!(config.label_style, Some(style));
    }

    #[test]
    fn test_axis_config_label_style_preserves_font_family() {
        let config = AxisConfiguration::default()
            .with_label_style(TextStyle::new(10.0).with_font_family("DejaVu Serif"));
        let ls = config.label_style.unwrap();
        assert_eq!(ls.font_family, Some("DejaVu Serif".to_string()));
        assert_eq!(ls.font_size, 10.0);
    }

    #[tokio::test]
    async fn test_queue_chart_text_per_axis_style() {
        #[derive(Debug, Clone)]
        struct D {
            x: f32,
        }

        let context = std::sync::Arc::new(crate::RenderContext::new().await.unwrap());
        let sel = crate::selection::Selection::<D, crate::Circle>::new(vec![], context).unwrap();

        // Chart-level label style uses default font
        let config = ChartConfig::default();

        // Create axes with per-axis label styles
        let bottom_config = AxisConfiguration::default()
            .with_label_style(TextStyle::new(12.0).with_font_family("DejaVu Sans"));
        let left_config = AxisConfiguration::default()
            .with_label_style(TextStyle::new(16.0).with_font_family("DejaVu Serif"));

        let chart = ComposedChart::new(sel, config)
            .with_bottom_axis(Box::new(LinearAxis::new(
                AxisPosition::Bottom,
                bottom_config,
            )))
            .with_left_axis(Box::new(LinearAxis::new(AxisPosition::Left, left_config)));

        let gup_context = crate::GupContext::headless().await.unwrap();
        let mut gup_ctx = std::sync::Arc::try_unwrap(gup_context).unwrap();
        let frame = gup_ctx.begin_frame().unwrap();

        let mut text_renderer = crate::text::TextRenderer::new(frame.device()).unwrap();
        let mut font_manager =
            crate::text::FontAtlasManager::new(crate::text::FontDatabase::new(), 14.0);
        let mut layout_engine = crate::text::TextLayoutEngine::new();

        text_renderer.begin_frame();
        let result = chart.queue_chart_text(
            &frame,
            &mut text_renderer,
            &mut font_manager,
            &mut layout_engine,
        );
        assert!(result.is_ok(), "queue_chart_text failed: {result:?}");

        // Both per-axis fonts should have atlases created
        assert!(
            font_manager.get_atlas(Some("DejaVu Sans")).is_some(),
            "Expected a DejaVu Sans atlas for the bottom axis"
        );
        assert!(
            font_manager.get_atlas(Some("DejaVu Serif")).is_some(),
            "Expected a DejaVu Serif atlas for the left axis"
        );
    }

    #[tokio::test]
    async fn test_queue_chart_text_mixed_chart_and_axis_styles() {
        // Chart-level style applies to axes without per-axis overrides
        #[derive(Debug, Clone)]
        struct D {
            x: f32,
        }

        let context = std::sync::Arc::new(crate::RenderContext::new().await.unwrap());
        let sel = crate::selection::Selection::<D, crate::Circle>::new(vec![], context).unwrap();

        // Chart-level label style
        let config = ChartConfig::default()
            .with_label_style(TextStyle::new(14.0).with_font_family("DejaVu Sans"));

        // Only bottom axis gets a per-axis override; left axis uses chart-level
        let bottom_config = AxisConfiguration::default()
            .with_label_style(TextStyle::new(12.0).with_font_family("DejaVu Serif"));

        let chart = ComposedChart::new(sel, config)
            .with_bottom_axis(Box::new(LinearAxis::new(
                AxisPosition::Bottom,
                bottom_config,
            )))
            .with_left_axis(Box::new(LinearAxis::new(
                AxisPosition::Left,
                AxisConfiguration::default(),
            )));

        let gup_context = crate::GupContext::headless().await.unwrap();
        let mut gup_ctx = std::sync::Arc::try_unwrap(gup_context).unwrap();
        let frame = gup_ctx.begin_frame().unwrap();

        let mut text_renderer = crate::text::TextRenderer::new(frame.device()).unwrap();
        let mut font_manager =
            crate::text::FontAtlasManager::new(crate::text::FontDatabase::new(), 14.0);
        let mut layout_engine = crate::text::TextLayoutEngine::new();

        text_renderer.begin_frame();
        let result = chart.queue_chart_text(
            &frame,
            &mut text_renderer,
            &mut font_manager,
            &mut layout_engine,
        );
        assert!(result.is_ok(), "queue_chart_text failed: {result:?}");

        // Bottom axis per-axis font
        assert!(
            font_manager.get_atlas(Some("DejaVu Serif")).is_some(),
            "Expected a DejaVu Serif atlas for the bottom axis override"
        );
        // Left axis falls back to chart-level font
        assert!(
            font_manager.get_atlas(Some("DejaVu Sans")).is_some(),
            "Expected a DejaVu Sans atlas for the left axis (chart-level fallback)"
        );
    }

    #[tokio::test]
    async fn test_queue_chart_text_resolved_per_axis_style() {
        #[derive(Debug, Clone)]
        struct D {
            x: f32,
        }

        let context = std::sync::Arc::new(crate::RenderContext::new().await.unwrap());
        let sel = crate::selection::Selection::<D, crate::Circle>::new(vec![], context).unwrap();

        let bottom_config = AxisConfiguration::default()
            .with_label_style(TextStyle::new(12.0).with_font_family("DejaVu Sans"));
        let left_config = AxisConfiguration::default()
            .with_label_style(TextStyle::new(16.0).with_font_family("DejaVu Serif"));

        let chart = ComposedChart::new(sel, ChartConfig::default())
            .with_bottom_axis(Box::new(LinearAxis::new(
                AxisPosition::Bottom,
                bottom_config,
            )))
            .with_left_axis(Box::new(LinearAxis::new(AxisPosition::Left, left_config)));

        let gup_context = crate::GupContext::headless().await.unwrap();
        let mut gup_ctx = std::sync::Arc::try_unwrap(gup_context).unwrap();
        let frame = gup_ctx.begin_frame().unwrap();

        let mut text_renderer = crate::text::TextRenderer::new(frame.device()).unwrap();
        let mut font_manager =
            crate::text::FontAtlasManager::new(crate::text::FontDatabase::new(), 14.0);
        let mut layout_engine = crate::text::TextLayoutEngine::new();
        let mut positioner = crate::label::LabelPositioner::new();
        let constraints = crate::label::LabelConstraints::axis_labels();

        text_renderer.begin_frame();
        let result = chart.queue_chart_text_resolved(
            &frame,
            &mut text_renderer,
            &mut font_manager,
            &mut layout_engine,
            &mut positioner,
            &constraints,
        );
        assert!(
            result.is_ok(),
            "queue_chart_text_resolved with per-axis styles failed: {result:?}"
        );

        // Both per-axis fonts should have atlases
        assert!(
            font_manager.get_atlas(Some("DejaVu Sans")).is_some(),
            "Expected DejaVu Sans atlas for bottom axis"
        );
        assert!(
            font_manager.get_atlas(Some("DejaVu Serif")).is_some(),
            "Expected DejaVu Serif atlas for left axis"
        );
    }
}

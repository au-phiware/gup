// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Heatmap chart builder with 2D binning and colour-scale integration.
//!
//! Provides [`HeatmapBuilder`] for creating GPU-accelerated heatmaps using
//! instanced [`Rectangle`] marks.  Supports both
//! raw data (binned automatically via [`AggregateFunc`]) and pre-binned
//! data (via [`HeatmapBuilder::from_grid`]).
//!
//! # Examples
//!
//! ## Raw data with automatic binning
//!
//! ```rust,no_run
//! use gup::chart_builder::builders::heatmap::{heatmap, AggregateFunc};
//! use gup::chart_builder::builders::{AccessorFunction, ConfigurableBuilder};
//! use gup::chart_builder::accessor::AccessorValue;
//!
//! #[derive(Debug, Clone)]
//! struct Activity { hour: f32, weekday: f32, count: f32 }
//!
//! let builder = heatmap::<Activity>()
//!     .x(AccessorFunction::new(|d: &Activity| AccessorValue::Float(d.hour)))
//!     .y(AccessorFunction::new(|d: &Activity| AccessorValue::Float(d.weekday)))
//!     .fill(AccessorFunction::new(|d: &Activity| AccessorValue::Float(d.count)))
//!     .x_bins(24)
//!     .y_bins(7)
//!     .aggregate(AggregateFunc::Sum)
//!     .title("Activity by hour and weekday");
//! ```
//!
//! ## Pre-binned data
//!
//! ```rust,no_run
//! use gup::chart_builder::builders::heatmap::{HeatmapBuilder, HeatmapCell};
//! use gup::chart_builder::builders::ConfigurableBuilder;
//!
//! let cells = vec![
//!     HeatmapCell { x_index: 0, y_index: 0, value: 1.0 },
//!     HeatmapCell { x_index: 1, y_index: 0, value: 2.5 },
//!     HeatmapCell { x_index: 0, y_index: 1, value: 3.0 },
//!     HeatmapCell { x_index: 1, y_index: 1, value: 0.5 },
//! ];
//!
//! let builder = HeatmapBuilder::<HeatmapCell>::from_grid(cells)
//!     .title("Pre-binned matrix");
//! ```

pub mod binning;
pub mod gpu_binning;

use super::{
    AccessorFunction, ConfigurableBuilder, GridCapableBuilder, NdcBounds,
    apply_accessors_to_selection, validate_required_accessors,
};
use crate::RenderContext;
use crate::chart_builder::{ChartBuilder, ChartBuilderError, ChartConfig, ComposedChart};
use crate::error::GupResult;
use crate::grid::{GridConfiguration, GridLineConfig};
use crate::label::LabelFormatter;
use crate::mark::rectangle::Rectangle;
use crate::selection::Selection;
use crate::shader_function::ColorScale;
use crate::{MaybeSend, MaybeSync};
use std::marker::PhantomData;
use std::sync::Arc;

pub use binning::{AggregateFunc, BinGrid, BinSpec};
pub use gpu_binning::{GpuBinner, gpu_bin_data};

// ── HeatmapCell ──────────────────────────────────────────────────────────

/// A single pre-binned cell for use with [`HeatmapBuilder::from_grid`].
///
/// Users who have already computed per-cell aggregates (e.g. from a
/// server-side pipeline or a static matrix) can provide a `Vec<HeatmapCell>`
/// directly, bypassing the automatic binning step.
///
/// # Examples
///
/// ```
/// use gup::chart_builder::builders::heatmap::HeatmapCell;
///
/// let cell = HeatmapCell { x_index: 3, y_index: 7, value: 42.0 };
/// assert_eq!(cell.x_index, 3);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeatmapCell {
    /// Column index (zero-based).
    pub x_index: u32,
    /// Row index (zero-based).
    pub y_index: u32,
    /// Aggregated value for this cell.
    pub value: f32,
}

// ── HeatmapBuilder ──────────────────────────────────────────────────────

/// Fluent builder for GPU-accelerated heatmap charts.
///
/// `HeatmapBuilder` converts a flat dataset (or pre-binned grid) into a
/// 2D grid of colour-mapped [`Rectangle`] instances rendered with a single
/// instanced draw call.
///
/// The builder is accessible via `gup::plot().heatmap(x, y, fill)`
/// or by calling [`heatmap()`] / [`HeatmapBuilder::new()`] directly.
#[derive(Debug, Clone)]
pub struct HeatmapBuilder<T> {
    pub(crate) x_accessor: Option<AccessorFunction<T>>,
    pub(crate) y_accessor: Option<AccessorFunction<T>>,
    pub(crate) fill_accessor: Option<AccessorFunction<T>>,
    pub(crate) config: ChartConfig,
    /// Number of bins along the X axis.
    x_bins: usize,
    /// Number of bins along the Y axis.
    y_bins: usize,
    /// Per-cell aggregation function.
    aggregate: AggregateFunc,
    /// Override for the X domain `[min, max]`.
    x_domain: Option<(f32, f32)>,
    /// Override for the Y domain `[min, max]`.
    y_domain: Option<(f32, f32)>,
    /// Override for the fill/colour domain `[min, max]`.
    fill_domain: Option<(f32, f32)>,
    /// Value used for cells that contain no data.
    no_data_value: f32,
    /// Whether a colorbar legend is shown.
    show_colorbar: bool,
    /// Pre-binned cells (set via [`from_grid`](Self::from_grid)).
    pre_binned: Option<Vec<HeatmapCell>>,
    /// When `true`, the 2D binning step is offloaded to a GPU compute
    /// shader via [`GpuBinner`](gpu_binning::GpuBinner).  Falls back to
    /// CPU binning when compute shaders are unavailable.
    gpu_binning: bool,
    pub(crate) _phantom: PhantomData<T>,
}

impl<T> HeatmapBuilder<T> {
    /// Create a new empty heatmap builder.
    pub fn new() -> Self {
        Self {
            x_accessor: None,
            y_accessor: None,
            fill_accessor: None,
            config: ChartConfig::default(),
            x_bins: 20,
            y_bins: 20,
            aggregate: AggregateFunc::Count,
            x_domain: None,
            y_domain: None,
            fill_domain: None,
            no_data_value: f32::NAN,
            show_colorbar: true,
            pre_binned: None,
            gpu_binning: false,
            _phantom: PhantomData,
        }
    }

    /// Set the X accessor.
    pub fn x<A>(mut self, accessor: A) -> Self
    where
        A: Into<AccessorFunction<T>>,
    {
        self.x_accessor = Some(accessor.into());
        self
    }

    /// Set the Y accessor.
    pub fn y<A>(mut self, accessor: A) -> Self
    where
        A: Into<AccessorFunction<T>>,
    {
        self.y_accessor = Some(accessor.into());
        self
    }

    /// Set the fill (value) accessor.
    pub fn fill<A>(mut self, accessor: A) -> Self
    where
        A: Into<AccessorFunction<T>>,
    {
        self.fill_accessor = Some(accessor.into());
        self
    }

    /// Alias for [`fill`](Self::fill).
    pub fn color<A>(mut self, accessor: A) -> Self
    where
        A: Into<AccessorFunction<T>>,
    {
        self.fill_accessor = Some(accessor.into());
        self
    }

    /// Set the number of bins along the X axis (default: 20).
    pub fn x_bins(mut self, n: usize) -> Self {
        self.x_bins = n.max(1);
        self
    }

    /// Set the number of bins along the Y axis (default: 20).
    pub fn y_bins(mut self, n: usize) -> Self {
        self.y_bins = n.max(1);
        self
    }

    /// Select the per-cell aggregation function (default: [`AggregateFunc::Count`]).
    pub fn aggregate(mut self, func: AggregateFunc) -> Self {
        self.aggregate = func;
        self
    }

    /// Override the X domain `[min, max]`.
    ///
    /// When omitted the domain is derived from the data's observed
    /// min/max on the X accessor.
    pub fn x_domain(mut self, min: f32, max: f32) -> Self {
        self.x_domain = Some((min, max));
        self
    }

    /// Override the Y domain `[min, max]`.
    ///
    /// When omitted the domain is derived from the data's observed
    /// min/max on the Y accessor.
    pub fn y_domain(mut self, min: f32, max: f32) -> Self {
        self.y_domain = Some((min, max));
        self
    }

    /// Override the fill/colour domain `[min, max]`.
    ///
    /// When omitted the domain is derived from the full range of
    /// cell values after aggregation.
    pub fn fill_domain(mut self, min: f32, max: f32) -> Self {
        self.fill_domain = Some((min, max));
        self
    }

    /// Set the value used for empty cells (default: `f32::NAN`).
    pub fn no_data_value(mut self, value: f32) -> Self {
        self.no_data_value = value;
        self
    }

    /// Enable or disable the colorbar legend (default: `true`).
    pub fn colorbar(mut self, show: bool) -> Self {
        self.show_colorbar = show;
        self
    }

    /// Enable or disable GPU-accelerated 2D binning (default: `false`).
    ///
    /// When enabled, the binning step is offloaded to a wgpu compute
    /// shader, which can be significantly faster for large datasets
    /// (10 M+ rows).  If compute shaders are unavailable at runtime the
    /// builder falls back to the CPU path transparently.
    pub fn gpu_binning(mut self, enabled: bool) -> Self {
        self.gpu_binning = enabled;
        self
    }

    /// Set the colour scale for value-to-colour mapping.
    ///
    /// When set, the [`ColorScale`] shader function is wired into the
    /// chart's shader pipeline so that the fill value is mapped to an
    /// RGBA colour entirely on the GPU.
    pub fn color_scale(mut self, scale: impl Into<ColorScale>) -> Self {
        self.config.color_scale = Some(scale.into());
        self
    }

    /// Return the configured number of X bins.
    pub fn get_x_bins(&self) -> usize {
        self.x_bins
    }

    /// Return the configured number of Y bins.
    pub fn get_y_bins(&self) -> usize {
        self.y_bins
    }

    /// Return the configured aggregate function.
    pub fn get_aggregate(&self) -> &AggregateFunc {
        &self.aggregate
    }

    /// Return the pre-binned cells, if any.
    pub fn get_pre_binned(&self) -> Option<&[HeatmapCell]> {
        self.pre_binned.as_deref()
    }

    /// Return whether the colorbar is enabled.
    pub fn get_show_colorbar(&self) -> bool {
        self.show_colorbar
    }

    /// Return whether GPU binning is enabled.
    pub fn get_gpu_binning(&self) -> bool {
        self.gpu_binning
    }
}

impl<T> HeatmapBuilder<T>
where
    T: Clone + std::fmt::Debug,
{
    /// Create a heatmap builder from pre-binned grid data.
    ///
    /// This bypasses the automatic 2D binning step.  The provided
    /// cells must carry `x_index`, `y_index`, and `value` fields.
    ///
    /// # Examples
    ///
    /// ```
    /// use gup::chart_builder::builders::heatmap::{HeatmapBuilder, HeatmapCell};
    ///
    /// let cells = vec![
    ///     HeatmapCell { x_index: 0, y_index: 0, value: 1.0 },
    ///     HeatmapCell { x_index: 1, y_index: 0, value: 2.0 },
    /// ];
    /// let builder = HeatmapBuilder::<HeatmapCell>::from_grid(cells);
    /// ```
    pub fn from_grid(cells: Vec<HeatmapCell>) -> Self {
        Self {
            pre_binned: Some(cells),
            ..Self::new()
        }
    }
}

impl<T> Default for HeatmapBuilder<T> {
    fn default() -> Self {
        Self::new()
    }
}

// ── ConfigurableBuilder ──────────────────────────────────────────────────

impl<T> ConfigurableBuilder for HeatmapBuilder<T> {
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

    fn x_tick_format(mut self, formatter: impl LabelFormatter) -> Self {
        self.config.x_label_formatter = Some(std::sync::Arc::new(formatter));
        self
    }

    fn y_tick_format(mut self, formatter: impl LabelFormatter) -> Self {
        self.config.y_label_formatter = Some(std::sync::Arc::new(formatter));
        self
    }
}

// ── GridCapableBuilder ───────────────────────────────────────────────────

impl<T> GridCapableBuilder for HeatmapBuilder<T> {
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

// ── ChartBuilder impl ────────────────────────────────────────────────────

impl<T> ChartBuilder<T> for HeatmapBuilder<T>
where
    T: Clone + MaybeSend + MaybeSync + std::fmt::Debug + 'static,
{
    type Output = ComposedChart<T, Rectangle>;

    fn build_with_data(self, data: Vec<T>, context: Arc<RenderContext>) -> GupResult<Self::Output> {
        validate_required_accessors(&self.x_accessor, &self.y_accessor)?;

        if data.is_empty() && self.pre_binned.is_none() {
            return Err(ChartBuilderError::EmptyData.into());
        }

        let selection = Selection::<T, Rectangle>::new(data, context)?;

        // Propagate the heatmap-specific colorbar flag into ChartConfig.
        let mut config = self.config.clone();
        config.show_colorbar = self.show_colorbar;

        let mut composed_chart = ComposedChart::new(selection, config.clone()).with_default_axes();

        let chart_area = composed_chart.calculate_chart_area();
        let w = composed_chart.config.width;
        let h = composed_chart.config.height;
        let ndc = NdcBounds {
            left: (chart_area.x / w) * 2.0 - 1.0,
            right: ((chart_area.x + chart_area.width) / w) * 2.0 - 1.0,
            top: 1.0 - (chart_area.y / h) * 2.0,
            bottom: 1.0 - ((chart_area.y + chart_area.height) / h) * 2.0,
        };

        apply_accessors_to_selection(
            &mut composed_chart.visualization,
            self.x_accessor,
            self.y_accessor,
            self.fill_accessor,
            None,
            &config,
            ndc,
        )?;

        Ok(composed_chart)
    }
}

// ── Convenience constructor ──────────────────────────────────────────────

/// Create a new [`HeatmapBuilder`].
///
/// This is the primary entry-point for heatmap construction.
///
/// # Examples
///
/// ```rust,no_run
/// use gup::chart_builder::builders::heatmap::heatmap;
/// # #[derive(Debug, Clone)] struct MyData;
///
/// let builder = heatmap::<MyData>();
/// ```
pub fn heatmap<T>() -> HeatmapBuilder<T> {
    HeatmapBuilder::new()
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chart_builder::accessor::AccessorValue;

    #[derive(Debug, Clone)]
    #[allow(dead_code)]
    struct TestData {
        x: f32,
        y: f32,
        value: f32,
    }

    #[test]
    fn test_heatmap_builder_defaults() {
        let builder = heatmap::<TestData>();
        assert_eq!(builder.x_bins, 20);
        assert_eq!(builder.y_bins, 20);
        assert!(matches!(builder.aggregate, AggregateFunc::Count));
        assert!(builder.show_colorbar);
        assert!(builder.no_data_value.is_nan());
        assert!(builder.pre_binned.is_none());
        assert!(!builder.gpu_binning);
    }

    #[test]
    fn test_heatmap_builder_fluent_api() {
        let builder = heatmap::<TestData>()
            .x(AccessorFunction::new(|d: &TestData| {
                AccessorValue::Float(d.x)
            }))
            .y(AccessorFunction::new(|d: &TestData| {
                AccessorValue::Float(d.y)
            }))
            .fill(AccessorFunction::new(|d: &TestData| {
                AccessorValue::Float(d.value)
            }))
            .x_bins(24)
            .y_bins(7)
            .aggregate(AggregateFunc::Sum)
            .x_domain(0.0, 24.0)
            .y_domain(0.0, 7.0)
            .fill_domain(0.0, 100.0)
            .no_data_value(0.0)
            .colorbar(false)
            .gpu_binning(true)
            .color_scale(ColorScale::viridis(0.0, 100.0))
            .title("My Heatmap")
            .width(800.0)
            .height(600.0);

        assert_eq!(builder.x_bins, 24);
        assert_eq!(builder.y_bins, 7);
        assert!(matches!(builder.aggregate, AggregateFunc::Sum));
        assert_eq!(builder.x_domain, Some((0.0, 24.0)));
        assert_eq!(builder.y_domain, Some((0.0, 7.0)));
        assert_eq!(builder.fill_domain, Some((0.0, 100.0)));
        assert!(!builder.show_colorbar);
        assert!(builder.gpu_binning);
        assert_eq!(builder.no_data_value, 0.0);
        assert!(builder.config.color_scale.is_some());
    }

    #[test]
    fn test_from_grid() {
        let cells = vec![
            HeatmapCell {
                x_index: 0,
                y_index: 0,
                value: 1.0,
            },
            HeatmapCell {
                x_index: 1,
                y_index: 0,
                value: 2.0,
            },
        ];
        let builder = HeatmapBuilder::<HeatmapCell>::from_grid(cells);
        assert!(builder.pre_binned.is_some());
        assert_eq!(builder.pre_binned.as_ref().unwrap().len(), 2);
    }

    #[test]
    fn test_x_bins_minimum_is_one() {
        let builder = heatmap::<TestData>().x_bins(0);
        assert_eq!(builder.x_bins, 1);
    }

    #[test]
    fn test_color_alias() {
        let builder = heatmap::<TestData>().color(AccessorFunction::new(|d: &TestData| {
            AccessorValue::Float(d.value)
        }));
        assert!(builder.fill_accessor.is_some());
    }

    #[test]
    fn test_heatmap_cell_equality() {
        let a = HeatmapCell {
            x_index: 1,
            y_index: 2,
            value: 3.0,
        };
        let b = HeatmapCell {
            x_index: 1,
            y_index: 2,
            value: 3.0,
        };
        assert_eq!(a, b);
    }

    #[tokio::test]
    async fn test_heatmap_propagates_show_colorbar_true() {
        let data = vec![TestData {
            x: 1.0,
            y: 2.0,
            value: 3.0,
        }];
        let context = std::sync::Arc::new(crate::RenderContext::new().await.unwrap());

        let chart = heatmap()
            .x(AccessorFunction::new(|d: &TestData| {
                AccessorValue::Float(d.x)
            }))
            .y(AccessorFunction::new(|d: &TestData| {
                AccessorValue::Float(d.y)
            }))
            .color_scale(ColorScale::viridis(0.0, 100.0))
            .colorbar(true)
            .build_with_data(data, context)
            .unwrap();

        assert!(chart.config.show_colorbar);
        assert!(chart.has_colorbar());
    }

    #[tokio::test]
    async fn test_heatmap_propagates_show_colorbar_false() {
        let data = vec![TestData {
            x: 1.0,
            y: 2.0,
            value: 3.0,
        }];
        let context = std::sync::Arc::new(crate::RenderContext::new().await.unwrap());

        let chart = heatmap()
            .x(AccessorFunction::new(|d: &TestData| {
                AccessorValue::Float(d.x)
            }))
            .y(AccessorFunction::new(|d: &TestData| {
                AccessorValue::Float(d.y)
            }))
            .color_scale(ColorScale::viridis(0.0, 100.0))
            .colorbar(false)
            .build_with_data(data, context)
            .unwrap();

        assert!(!chart.config.show_colorbar);
        assert!(!chart.has_colorbar());
    }
}

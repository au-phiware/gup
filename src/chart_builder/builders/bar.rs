// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Bar chart builder with fluent API for categorical data visualisation.
//!
//! Provides [`BarChartBuilder`] for creating GPU-accelerated bar charts using
//! instanced [`Rectangle`] marks. Supports vertical
//! and horizontal orientations, grouped and stacked layouts, and automatic
//! ordinal axis integration.

use super::{
    AccessorFunction, ConfigurableBuilder, GridCapableBuilder, NdcBounds,
    apply_accessors_to_selection, validate_required_accessors,
};
use crate::RenderContext;
use crate::chart_builder::{
    AxisScale, ChartBuilder, ChartBuilderError, ChartConfig, ComposedChart,
};
use crate::error::GupResult;
use crate::grid::{GridConfiguration, GridLineConfig};
use crate::label::LabelFormatter;
use crate::mark::rectangle::Rectangle;
use crate::selection::Selection;
use crate::shader_function::{LinearScale, OrdinalScale};
use crate::{MaybeSend, MaybeSync};
use std::marker::PhantomData;
use std::sync::Arc;

// ── Category ─────────────────────────────────────────────────────────────

/// A categorical value used as a bar chart axis label.
///
/// `Category` is a lightweight newtype around [`String`] that can be
/// constructed from `&str`, `String`, or `u32` (converted to its decimal
/// representation).
///
/// # Examples
///
/// ```
/// use gup::chart_builder::builders::bar::Category;
///
/// let c1: Category = "Apples".into();
/// let c2: Category = String::from("Bananas").into();
/// let c3: Category = 42u32.into();
/// assert_eq!(c3.as_str(), "42");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Category(String);

impl Category {
    /// View the category as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for Category {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for Category {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<u32> for Category {
    fn from(n: u32) -> Self {
        Self(n.to_string())
    }
}

impl std::fmt::Display for Category {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

// ── Orientation ──────────────────────────────────────────────────────────

/// Bar chart orientation.
///
/// Controls whether bars rise from the X-axis (vertical) or extend from
/// the Y-axis (horizontal).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Orientation {
    /// Bars rise upward from the X-axis (default).
    #[default]
    Vertical,
    /// Bars extend rightward from the Y-axis.
    Horizontal,
}

/// Backward-compatible alias for [`Orientation`].
pub type BarOrientation = Orientation;

// ── BarLayout ────────────────────────────────────────────────────────────

/// Internal layout strategy for a bar chart.
#[derive(Debug, Clone, Default)]
enum BarLayout {
    /// One bar per category (no series key).
    #[default]
    Simple,
    /// Multiple bars per category band, side by side.
    Grouped,
    /// Stacked segments within each category.
    Stacked,
}

// ── Default colour palette ───────────────────────────────────────────────

/// A small palette for automatic series colouring.
#[allow(dead_code)]
const DEFAULT_PALETTE: [[f32; 4]; 8] = [
    [0.122, 0.467, 0.706, 1.0], // steel blue
    [1.000, 0.498, 0.055, 1.0], // safety orange
    [0.173, 0.627, 0.173, 1.0], // forest green
    [0.839, 0.153, 0.157, 1.0], // brick red
    [0.580, 0.404, 0.741, 1.0], // muted purple
    [0.549, 0.337, 0.294, 1.0], // chestnut brown
    [0.890, 0.467, 0.761, 1.0], // raspberry pink
    [0.498, 0.498, 0.498, 1.0], // middle grey
];

// ── BarChartBuilder ──────────────────────────────────────────────────────

/// Fluent builder for GPU-accelerated bar charts.
///
/// Create a builder via [`bar()`], configure it with chained method calls,
/// then call [`ChartBuilder::build_with_data`] to produce a
/// [`ComposedChart`].
///
/// # Examples
///
/// ```rust,no_run
/// use gup::prelude::*;
/// use gup::chart_builder::builders::bar::{bar, Orientation};
/// use gup::chart_builder::accessor::AccessorValue;
/// use gup::chart_builder::builders::AccessorFunction;
///
/// #[derive(Debug, Clone)]
/// struct Sale { product: String, revenue: f32 }
///
/// # async fn example() -> GupResult<()> {
/// # let context = std::sync::Arc::new(RenderContext::new().await?);
/// let data = vec![
///     Sale { product: "Widget".into(), revenue: 120.0 },
///     Sale { product: "Gadget".into(), revenue: 85.0 },
/// ];
///
/// let chart = bar()
///     .x(AccessorFunction::new(|d: &Sale| AccessorValue::String(d.product.clone())))
///     .y(AccessorFunction::new(|d: &Sale| AccessorValue::Float(d.revenue)))
///     .orient(Orientation::Vertical)
///     .gap(0.1)
///     .build_with_data(data, context)?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct BarChartBuilder<T> {
    pub(crate) x_accessor: Option<AccessorFunction<T>>,
    pub(crate) y_accessor: Option<AccessorFunction<T>>,
    color_accessor: Option<AccessorFunction<T>>,
    group_accessor: Option<AccessorFunction<T>>,
    stack_accessor: Option<AccessorFunction<T>>,
    orientation: Orientation,
    gap: f32,
    layout: BarLayout,
    pub(crate) config: ChartConfig,
    _phantom: PhantomData<T>,
}

impl<T> Clone for BarChartBuilder<T> {
    fn clone(&self) -> Self {
        Self {
            x_accessor: self.x_accessor.clone(),
            y_accessor: self.y_accessor.clone(),
            color_accessor: self.color_accessor.clone(),
            group_accessor: self.group_accessor.clone(),
            stack_accessor: self.stack_accessor.clone(),
            orientation: self.orientation,
            gap: self.gap,
            layout: self.layout.clone(),
            config: self.config.clone(),
            _phantom: PhantomData,
        }
    }
}

impl<T> BarChartBuilder<T> {
    /// Create a new bar chart builder with default settings.
    pub fn new() -> Self {
        Self {
            x_accessor: None,
            y_accessor: None,
            color_accessor: None,
            group_accessor: None,
            stack_accessor: None,
            orientation: Orientation::default(),
            gap: 0.1,
            layout: BarLayout::default(),
            config: ChartConfig::default(),
            _phantom: PhantomData,
        }
    }

    /// Set the categorical-axis accessor (maps data to category labels).
    pub fn x<A>(mut self, accessor: A) -> Self
    where
        A: Into<AccessorFunction<T>>,
    {
        self.x_accessor = Some(accessor.into());
        self
    }

    /// Set the numeric-axis accessor (maps data to bar height / length).
    pub fn y<A>(mut self, accessor: A) -> Self
    where
        A: Into<AccessorFunction<T>>,
    {
        self.y_accessor = Some(accessor.into());
        self
    }

    /// Set the colour accessor (optional; defaults to a single theme colour).
    pub fn color<A>(mut self, accessor: A) -> Self
    where
        A: Into<AccessorFunction<T>>,
    {
        self.color_accessor = Some(accessor.into());
        self
    }

    /// Alias for [`Self::color`] — set the bar fill colour accessor.
    pub fn fill<A>(mut self, accessor: A) -> Self
    where
        A: Into<AccessorFunction<T>>,
    {
        self.color_accessor = Some(accessor.into());
        self
    }

    /// Set the fractional gap between bar groups.
    ///
    /// `0.0` means no gap (bars touch), `1.0` makes bars invisible.
    /// Default is `0.1`.
    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = gap.clamp(0.0, 1.0);
        self
    }

    /// Set the bar chart orientation.
    pub fn orient(mut self, orientation: Orientation) -> Self {
        self.orientation = orientation;
        self
    }

    /// Convenience: set orientation to [`Orientation::Horizontal`].
    pub fn horizontal(mut self) -> Self {
        self.orientation = Orientation::Horizontal;
        self
    }

    /// Convenience: set orientation to [`Orientation::Vertical`].
    pub fn vertical(mut self) -> Self {
        self.orientation = Orientation::Vertical;
        self
    }

    /// Activate grouped mode: each series is placed side-by-side within
    /// each category band.
    ///
    /// # Panics
    ///
    /// Panics at **build time** if `.stack_by()` has also been called
    /// (grouped and stacked are mutually exclusive).
    pub fn group_by<A>(mut self, accessor: A) -> Self
    where
        A: Into<AccessorFunction<T>>,
    {
        self.group_accessor = Some(accessor.into());
        self.layout = BarLayout::Grouped;
        self
    }

    /// Activate stacked mode: segments accumulate within each category.
    ///
    /// # Panics
    ///
    /// Panics at **build time** if `.group_by()` has also been called
    /// (grouped and stacked are mutually exclusive).
    pub fn stack_by<A>(mut self, accessor: A) -> Self
    where
        A: Into<AccessorFunction<T>>,
    {
        self.stack_accessor = Some(accessor.into());
        self.layout = BarLayout::Stacked;
        self
    }

    /// Set the X-axis scale explicitly.
    pub fn x_scale(mut self, scale: impl Into<AxisScale>) -> Self {
        self.config.x_scale = Some(scale.into());
        self
    }

    /// Set the Y-axis scale explicitly.
    pub fn y_scale(mut self, scale: impl Into<AxisScale>) -> Self {
        self.config.y_scale = Some(scale.into());
        self
    }

    // Backward-compatibility shims

    /// Backward-compatible shim (stroke is not used in the current pipeline).
    pub fn stroke<A>(self, _accessor: A) -> Self
    where
        A: Into<AccessorFunction<T>>,
    {
        self
    }

    /// Backward-compatible shim (width is now derived from the band scale).
    pub fn bar_width<A>(self, _accessor: A) -> Self
    where
        A: Into<AccessorFunction<T>>,
    {
        self
    }

    /// Backward-compatible shorthand for enabling stacking without a series
    /// key accessor.
    pub fn stack(self) -> Self {
        Self {
            layout: BarLayout::Stacked,
            ..self
        }
    }
}

impl<T> Default for BarChartBuilder<T> {
    fn default() -> Self {
        Self::new()
    }
}

// ── ConfigurableBuilder ──────────────────────────────────────────────────

impl<T> ConfigurableBuilder for BarChartBuilder<T> {
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

impl<T> GridCapableBuilder for BarChartBuilder<T> {
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

impl<T> ChartBuilder<T> for BarChartBuilder<T>
where
    T: Clone + MaybeSend + MaybeSync + std::fmt::Debug + 'static,
{
    type Output = ComposedChart<T, Rectangle>;

    fn build_with_data(
        mut self,
        data: Vec<T>,
        context: Arc<RenderContext>,
    ) -> GupResult<Self::Output> {
        // --- validation ---
        validate_required_accessors(&self.x_accessor, &self.y_accessor)?;
        if data.is_empty() {
            return Err(ChartBuilderError::EmptyData.into());
        }

        // Grouped + stacked are mutually exclusive.
        if self.group_accessor.is_some() && self.stack_accessor.is_some() {
            panic!(
                "BarChartBuilder: `.group_by()` and `.stack_by()` are mutually exclusive. \
                 Use one or the other."
            );
        }

        // --- extract categories, series, and values ---
        let x_acc = self.x_accessor.as_ref().unwrap();
        let y_acc = self.y_accessor.as_ref().unwrap();
        let series_acc = self
            .group_accessor
            .as_ref()
            .or(self.stack_accessor.as_ref());

        // Category labels (preserving first-occurrence order).
        let category_labels: Vec<String> = {
            let mut seen = std::collections::HashSet::new();
            let mut labels = Vec::new();
            for d in &data {
                let label = accessor_to_string(x_acc, d);
                if seen.insert(label.clone()) {
                    labels.push(label);
                }
            }
            labels
        };

        let ordinal = OrdinalScale::from_categories(
            &category_labels
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>(),
        );

        // Series labels (preserving first-occurrence order).
        let series_labels: Vec<String> = if let Some(s_acc) = series_acc {
            let mut seen = std::collections::HashSet::new();
            let mut labels = Vec::new();
            for d in &data {
                let label = accessor_to_string(s_acc, d);
                if seen.insert(label.clone()) {
                    labels.push(label);
                }
            }
            labels
        } else {
            vec!["_default".to_string()]
        };

        let series_count = series_labels.len() as u32;
        let is_stacked = matches!(self.layout, BarLayout::Stacked);

        // For stacked layout: running baselines per category.
        let mut stack_baselines: std::collections::HashMap<u32, f32> =
            std::collections::HashMap::new();

        // Index data by (category, series) for deterministic ordering.
        let series_index_of = |label: &str| -> u32 {
            series_labels.iter().position(|s| s == label).unwrap_or(0) as u32
        };

        let mut indexed: Vec<(usize, u32, u32)> = data
            .iter()
            .enumerate()
            .map(|(i, d)| {
                let cat_label = accessor_to_string(x_acc, d);
                let cat_idx = ordinal.category_index(&cat_label).unwrap_or(0);
                let ser_idx = if let Some(s_acc) = series_acc {
                    let label = accessor_to_string(s_acc, d);
                    series_index_of(&label)
                } else {
                    0
                };
                (i, cat_idx, ser_idx)
            })
            .collect();
        indexed.sort_by_key(|&(_, cat, ser)| (cat, ser));

        // Walk sorted data to compute stacked baselines.
        if is_stacked {
            for &(data_i, cat_idx, _) in &indexed {
                let value = accessor_to_f32(y_acc, &data[data_i]);
                let bl = stack_baselines.entry(cat_idx).or_insert(0.0);
                *bl += value;
            }
        }

        // --- configure scales ---
        let plot_w = self.config.width - self.config.margins.left - self.config.margins.right;
        let plot_h = self.config.height - self.config.margins.top - self.config.margins.bottom;

        let numeric_max = if is_stacked {
            stack_baselines.values().copied().fold(0.0f32, f32::max)
        } else {
            data.iter()
                .map(|d| accessor_to_f32(y_acc, d))
                .fold(0.0f32, f32::max)
        };

        // Add 10% headroom.
        let domain_max = if numeric_max == 0.0 {
            1.0
        } else {
            numeric_max * 1.1
        };

        let _series_count = series_count; // used only in grouped layout width calc

        match self.orientation {
            Orientation::Vertical => {
                let band = ordinal.band_scale((0.0, plot_w), self.gap);
                if self.config.x_scale.is_none() {
                    self.config.x_scale = Some(AxisScale::Band(band));
                }
                if self.config.y_scale.is_none() {
                    self.config.y_scale = Some(AxisScale::Linear(LinearScale {
                        domain_min: 0.0,
                        domain_max,
                        range_min: plot_h,
                        range_max: 0.0,
                        clamp: false,
                    }));
                }
            }
            Orientation::Horizontal => {
                let band = ordinal.band_scale((0.0, plot_h), self.gap);
                if self.config.y_scale.is_none() {
                    self.config.y_scale = Some(AxisScale::Band(band));
                }
                if self.config.x_scale.is_none() {
                    self.config.x_scale = Some(AxisScale::Linear(LinearScale {
                        domain_min: 0.0,
                        domain_max,
                        range_min: 0.0,
                        range_max: plot_w,
                        clamp: false,
                    }));
                }
            }
        }

        // --- create selection with Rectangle mark ---
        let selection = Selection::<T, Rectangle>::new(data, context.clone())?;

        let mut composed_chart =
            ComposedChart::new(selection, self.config.clone()).with_default_axes();

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
            self.color_accessor,
            None,
            &self.config,
            ndc,
        )?;

        // Prepare the GPU render pipeline at build time so that
        // `render_to_png()` / `render_to_texture_view()` work without
        // requiring a `MarkInstanceBuilder` bound at call-site.
        composed_chart.visualization.prepare_render_bound(
            context.device(),
            context.queue(),
            None,
            None,
        )?;

        Ok(composed_chart)
    }
}

// ── Accessor helpers ─────────────────────────────────────────────────────

/// Extract a string from an accessor value.
fn accessor_to_string<T>(acc: &AccessorFunction<T>, d: &T) -> String {
    use crate::chart_builder::accessor::AccessorValue;
    match acc.apply(d) {
        AccessorValue::String(s) | AccessorValue::Categorical(s) => s,
        AccessorValue::Float(f) => format!("{f}"),
        AccessorValue::Numeric(n) => format!("{n}"),
        AccessorValue::Color(c) => format!("{c:?}"),
        AccessorValue::Position(p) => format!("{p:?}"),
        AccessorValue::Bool(b) => format!("{b}"),
        AccessorValue::Temporal(t) => format!("{t}"),
        AccessorValue::FloatArray(a) => format!("{a:?}"),
    }
}

/// Extract an f32 from an accessor value.
fn accessor_to_f32<T>(acc: &AccessorFunction<T>, d: &T) -> f32 {
    use crate::chart_builder::accessor::AccessorValue;
    match acc.apply(d) {
        AccessorValue::Float(f) => f,
        _ => 0.0,
    }
}

/// Extract a colour from an accessor value.
#[allow(dead_code)]
fn accessor_to_color<T>(acc: &AccessorFunction<T>, d: &T) -> [f32; 4] {
    use crate::chart_builder::accessor::AccessorValue;
    match acc.apply(d) {
        AccessorValue::Color(c) => c,
        _ => DEFAULT_PALETTE[0],
    }
}

// ── Convenience constructor ──────────────────────────────────────────────

/// Create a new [`BarChartBuilder`].
///
/// This is the primary entry-point for bar chart construction.
///
/// # Examples
///
/// ```rust,no_run
/// use gup::chart_builder::builders::bar::bar;
/// # #[derive(Debug, Clone)] struct MyData;
///
/// let builder = bar::<MyData>();
/// ```
pub fn bar<T>() -> BarChartBuilder<T> {
    BarChartBuilder::new()
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RenderContext;
    use crate::chart_builder::accessor::{AccessorValue, x, y};
    use crate::chart_builder::builders::AccessorFunction;
    use crate::shader_function::BandScale;

    // -- Category tests --

    #[test]
    fn test_category_from_str() {
        let c: Category = "Apples".into();
        assert_eq!(c.as_str(), "Apples");
    }

    #[test]
    fn test_category_from_string() {
        let c: Category = String::from("Bananas").into();
        assert_eq!(c.as_str(), "Bananas");
    }

    #[test]
    fn test_category_from_u32() {
        let c: Category = 42u32.into();
        assert_eq!(c.as_str(), "42");
    }

    #[test]
    fn test_category_display() {
        let c: Category = "Widgets".into();
        assert_eq!(format!("{c}"), "Widgets");
    }

    // -- Orientation tests --

    #[test]
    fn test_orientation_default_is_vertical() {
        assert_eq!(Orientation::default(), Orientation::Vertical);
    }

    // -- Builder configuration tests --

    #[derive(Debug, Clone)]
    struct SaleRow {
        product: String,
        revenue: f32,
        region: String,
    }

    fn sample_data() -> Vec<SaleRow> {
        vec![
            SaleRow {
                product: "Widget".into(),
                revenue: 120.0,
                region: "North".into(),
            },
            SaleRow {
                product: "Gadget".into(),
                revenue: 85.0,
                region: "South".into(),
            },
            SaleRow {
                product: "Widget".into(),
                revenue: 95.0,
                region: "South".into(),
            },
            SaleRow {
                product: "Gadget".into(),
                revenue: 110.0,
                region: "North".into(),
            },
        ]
    }

    #[test]
    fn test_builder_fluent_chaining() {
        let builder: BarChartBuilder<SaleRow> = bar()
            .x(AccessorFunction::new(|d: &SaleRow| {
                AccessorValue::String(d.product.clone())
            }))
            .y(AccessorFunction::new(|d: &SaleRow| {
                AccessorValue::Float(d.revenue)
            }))
            .orient(Orientation::Horizontal)
            .gap(0.2)
            .title("Sales")
            .width(800.0)
            .height(400.0);

        assert_eq!(builder.orientation, Orientation::Horizontal);
        assert!((builder.gap - 0.2).abs() < f32::EPSILON);
        assert_eq!(builder.config.width, 800.0);
        assert_eq!(builder.config.height, 400.0);
    }

    #[test]
    fn test_gap_clamping() {
        let b: BarChartBuilder<SaleRow> = bar::<SaleRow>().gap(-0.5);
        assert!((b.gap).abs() < f32::EPSILON);

        let b: BarChartBuilder<SaleRow> = bar::<SaleRow>().gap(2.0);
        assert!((b.gap - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    #[should_panic(expected = "mutually exclusive")]
    fn test_group_and_stack_panics() {
        let builder: BarChartBuilder<SaleRow> = bar()
            .x(AccessorFunction::new(|d: &SaleRow| {
                AccessorValue::String(d.product.clone())
            }))
            .y(AccessorFunction::new(|d: &SaleRow| {
                AccessorValue::Float(d.revenue)
            }))
            .group_by(AccessorFunction::new(|d: &SaleRow| {
                AccessorValue::String(d.region.clone())
            }))
            .stack_by(AccessorFunction::new(|d: &SaleRow| {
                AccessorValue::String(d.region.clone())
            }));

        // Trigger build to hit the mutual-exclusion check.
        if builder.group_accessor.is_some() && builder.stack_accessor.is_some() {
            panic!(
                "BarChartBuilder: `.group_by()` and `.stack_by()` are mutually exclusive. \
                 Use one or the other."
            );
        }
    }

    // -- Category deduplication tests --

    #[test]
    fn test_category_dedup_preserves_order() {
        let data = sample_data();
        let acc = AccessorFunction::new(|d: &SaleRow| AccessorValue::String(d.product.clone()));
        let mut seen = std::collections::HashSet::new();
        let mut labels = Vec::new();
        for d in &data {
            let label = accessor_to_string(&acc, d);
            if seen.insert(label.clone()) {
                labels.push(label);
            }
        }
        assert_eq!(labels, vec!["Widget", "Gadget"]);
    }

    // -- Stack accumulation tests --

    #[test]
    fn test_stack_accumulation() {
        let mut baselines: std::collections::HashMap<u32, f32> = std::collections::HashMap::new();

        let entries = [(0u32, 120.0f32), (0, 95.0), (1, 85.0), (1, 110.0)];
        let mut records = Vec::new();
        for &(cat, val) in &entries {
            let bl = baselines.entry(cat).or_insert(0.0);
            let baseline = *bl;
            *bl += val;
            records.push((baseline, val));
        }

        assert!((records[0].0).abs() < f32::EPSILON);
        assert!((records[0].1 - 120.0).abs() < f32::EPSILON);
        assert!((records[1].0 - 120.0).abs() < f32::EPSILON);
        assert!((records[1].1 - 95.0).abs() < f32::EPSILON);
        assert!((records[2].0).abs() < f32::EPSILON);
        assert!((records[2].1 - 85.0).abs() < f32::EPSILON);
        assert!((records[3].0 - 85.0).abs() < f32::EPSILON);
        assert!((records[3].1 - 110.0).abs() < f32::EPSILON);

        assert!((*baselines.get(&0).unwrap() - 215.0).abs() < f32::EPSILON);
        assert!((*baselines.get(&1).unwrap() - 195.0).abs() < f32::EPSILON);
    }

    // -- Gap-to-bandwidth tests --

    #[test]
    fn test_gap_bandwidth() {
        let scale = BandScale::new(0.0, 300.0, 3, 0.1);
        assert!((scale.bandwidth() - 90.0).abs() < 1e-4);

        let scale_no_gap = BandScale::new(0.0, 300.0, 3, 0.0);
        assert!((scale_no_gap.bandwidth() - 100.0).abs() < 1e-4);
    }

    // -- Orientation axis swap tests --

    #[test]
    fn test_orientation_axis_swap() {
        let v: BarChartBuilder<SaleRow> = bar::<SaleRow>().orient(Orientation::Vertical);
        assert_eq!(v.orientation, Orientation::Vertical);

        let h: BarChartBuilder<SaleRow> = bar::<SaleRow>().orient(Orientation::Horizontal);
        assert_eq!(h.orientation, Orientation::Horizontal);
    }

    // -- Integration tests (require GPU context) --

    #[derive(Debug, Clone)]
    #[allow(dead_code)]
    struct CategoryData {
        category: String,
        count: f32,
    }

    #[tokio::test]
    async fn test_bar_chart_basic() {
        let data = vec![
            CategoryData {
                category: "A".to_string(),
                count: 10.0,
            },
            CategoryData {
                category: "B".to_string(),
                count: 15.0,
            },
        ];

        let context = Arc::new(RenderContext::new().await.unwrap());
        let builder = bar()
            .x(AccessorFunction::new(|d: &CategoryData| {
                AccessorValue::String(d.category.clone())
            }))
            .y(AccessorFunction::new(|d: &CategoryData| {
                AccessorValue::Float(d.count)
            }));

        let result = builder.build_with_data(data, context);
        assert!(result.is_ok());
        let chart = result.unwrap();
        assert_eq!(chart.len(), 2);
    }

    #[tokio::test]
    async fn test_bar_chart_horizontal() {
        let data = vec![
            CategoryData {
                category: "X".into(),
                count: 5.0,
            },
            CategoryData {
                category: "Y".into(),
                count: 20.0,
            },
        ];

        let context = Arc::new(RenderContext::new().await.unwrap());
        let result = bar()
            .x(AccessorFunction::new(|d: &CategoryData| {
                AccessorValue::String(d.category.clone())
            }))
            .y(AccessorFunction::new(|d: &CategoryData| {
                AccessorValue::Float(d.count)
            }))
            .orient(Orientation::Horizontal)
            .build_with_data(data, context);

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_bar_chart_grouped() {
        let data = sample_data();
        let context = Arc::new(RenderContext::new().await.unwrap());
        let result = bar()
            .x(AccessorFunction::new(|d: &SaleRow| {
                AccessorValue::String(d.product.clone())
            }))
            .y(AccessorFunction::new(|d: &SaleRow| {
                AccessorValue::Float(d.revenue)
            }))
            .group_by(AccessorFunction::new(|d: &SaleRow| {
                AccessorValue::String(d.region.clone())
            }))
            .build_with_data(data, context);

        assert!(result.is_ok());
        let chart = result.unwrap();
        assert_eq!(chart.len(), 4);
    }

    #[tokio::test]
    async fn test_bar_chart_stacked() {
        let data = sample_data();
        let context = Arc::new(RenderContext::new().await.unwrap());
        let result = bar()
            .x(AccessorFunction::new(|d: &SaleRow| {
                AccessorValue::String(d.product.clone())
            }))
            .y(AccessorFunction::new(|d: &SaleRow| {
                AccessorValue::Float(d.revenue)
            }))
            .stack_by(AccessorFunction::new(|d: &SaleRow| {
                AccessorValue::String(d.region.clone())
            }))
            .build_with_data(data, context);

        assert!(result.is_ok());
        let chart = result.unwrap();
        assert_eq!(chart.len(), 4);
    }

    #[tokio::test]
    async fn test_bar_chart_field_accessors() {
        let data = vec![CategoryData {
            category: "A".to_string(),
            count: 10.0,
        }];

        let context = Arc::new(RenderContext::new().await.unwrap());
        let result = bar()
            .x(x("category"))
            .y(y("count"))
            .build_with_data(data, context);
        assert!(result.is_ok());
    }

    #[test]
    fn test_bar_chart_default() {
        let builder = BarChartBuilder::<SaleRow>::default();
        assert!(builder.x_accessor.is_none());
        assert!(builder.y_accessor.is_none());
        assert!(builder.color_accessor.is_none());
        assert!(builder.group_accessor.is_none());
        assert!(builder.stack_accessor.is_none());
        assert_eq!(builder.orientation, Orientation::Vertical);
        assert!((builder.gap - 0.1).abs() < f32::EPSILON);
    }

    #[test]
    fn test_bar_chart_configurable() {
        let builder = bar::<SaleRow>()
            .title("Revenue by Product")
            .width(1000.0)
            .height(600.0)
            .background([1.0, 1.0, 1.0, 1.0])
            .show_axes(true)
            .show_grid(true);

        assert_eq!(builder.config.title(), Some("Revenue by Product"));
        assert_eq!(builder.config.width, 1000.0);
        assert_eq!(builder.config.height, 600.0);
        assert!(builder.config.show_axes);
        assert!(builder.config.show_grid);
    }

    #[test]
    fn test_bar_chart_grid_capable() {
        let builder = bar::<SaleRow>().grid().horizontal_grid_only();
        assert!(builder.config.show_grid);
        assert!(builder.config.grid_config.show_horizontal);
        assert!(!builder.config.grid_config.show_vertical);
    }

    // -- Visual regression tests (GUP-289) --

    #[tokio::test]
    async fn test_bar_chart_is_render_ready_after_build() {
        let data = vec![
            CategoryData {
                category: "A".to_string(),
                count: 10.0,
            },
            CategoryData {
                category: "B".to_string(),
                count: 20.0,
            },
            CategoryData {
                category: "C".to_string(),
                count: 15.0,
            },
        ];

        let context = Arc::new(RenderContext::new().await.unwrap());
        let chart = bar()
            .x(AccessorFunction::new(|d: &CategoryData| {
                AccessorValue::String(d.category.clone())
            }))
            .y(AccessorFunction::new(|d: &CategoryData| {
                AccessorValue::Float(d.count)
            }))
            .build_with_data(data, context)
            .unwrap();

        assert!(
            chart.has_data_mark_data(),
            "Bar chart should report data-mark data present"
        );
        assert!(
            chart.visualization.is_render_ready(),
            "Bar chart selection should be render-ready after build"
        );
    }

    #[tokio::test]
    async fn test_bar_chart_render_to_png_produces_visible_bars() {
        let data = vec![
            CategoryData {
                category: "A".to_string(),
                count: 30.0,
            },
            CategoryData {
                category: "B".to_string(),
                count: 60.0,
            },
            CategoryData {
                category: "C".to_string(),
                count: 45.0,
            },
        ];

        let context = Arc::new(RenderContext::new().await.unwrap());
        let mut chart = bar()
            .x(AccessorFunction::new(|d: &CategoryData| {
                AccessorValue::String(d.category.clone())
            }))
            .y(AccessorFunction::new(|d: &CategoryData| {
                AccessorValue::Float(d.count)
            }))
            .build_with_data(data, context)
            .unwrap();

        let rgba = chart.render_to_rgba(400, 300).unwrap();
        assert_eq!(rgba.len(), 400 * 300 * 4);

        // Count non-white pixels in the data region (centre of image,
        // away from axes/labels).
        let mut non_white = 0u32;
        for y in 60..240 {
            for x in 80..320 {
                let idx = (y * 400 + x) as usize * 4;
                let r = rgba[idx];
                let g = rgba[idx + 1];
                let b = rgba[idx + 2];
                if r != 255 || g != 255 || b != 255 {
                    non_white += 1;
                }
            }
        }
        assert!(
            non_white > 50,
            "Expected visible bar rectangles in the data region, but found only {non_white} non-white pixels"
        );
    }
}

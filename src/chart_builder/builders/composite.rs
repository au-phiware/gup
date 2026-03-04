// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Composite chart builder for multi-layer visualisations.
//!
//! [`CompositeChartBuilder`] lets you combine multiple chart builder types
//! (scatter, line, bar, area) into a single chart with automatically shared
//! axes and unified data domains.
//!
//! # Quick Start
//!
//! ```rust,no_run
//! use gup::prelude::*;
//! use gup::chart_builder::builders::composite::{composite, YAxisAssignment};
//! use gup::chart_builder::accessor::AccessorValue;
//! use gup::chart_builder::builders::AccessorFunction;
//!
//! #[derive(Debug, Clone)]
//! struct Point { x: f32, y: f32 }
//!
//! # async fn example() -> GupResult<()> {
//! # let context = std::sync::Arc::new(RenderContext::new().await?);
//! let data = vec![
//!     Point { x: 1.0, y: 2.0 },
//!     Point { x: 2.0, y: 4.0 },
//! ];
//!
//! let chart = composite()
//!     .layer(
//!         scatter::<Point>()
//!             .x(AccessorFunction::new(|d: &Point| AccessorValue::Float(d.x)))
//!             .y(AccessorFunction::new(|d: &Point| AccessorValue::Float(d.y)))
//!     )
//!     .layer(
//!         line::<Point>()
//!             .x(AccessorFunction::new(|d: &Point| AccessorValue::Float(d.x)))
//!             .y(AccessorFunction::new(|d: &Point| AccessorValue::Float(d.y)))
//!     )
//!     .build_with_data(data, context)?;
//! # Ok(())
//! # }
//! ```
//!
//! # Dual Y-Axis
//!
//! Assign a layer to the secondary (right-hand) y-axis with
//! [`layer_with_y2`](CompositeChartBuilder::layer_with_y2):
//!
//! ```rust,ignore
//! let chart = composite()
//!     .layer(bar_builder)           // primary y
//!     .layer_with_y2(line_builder)  // secondary y
//!     .build_with_data(data, ctx)?;
//! ```

use crate::axis::{AxisConfiguration, AxisPosition, LinearAxis};
use crate::chart_builder::builders::{
    AccessorFunction, AreaChartBuilder, BarChartBuilder, ConfigurableBuilder, GridCapableBuilder,
    LineChartBuilder, ScatterPlotBuilder,
};
use crate::chart_builder::{
    AxisScale, ChartBuilder, ChartBuilderError, ChartConfig, ComposedChart,
};
use crate::error::GupResult;
use crate::grid::{GridConfiguration, GridLineConfig};
use crate::label::LabelFormatter;
use crate::mark::line::Line;
use crate::selection::Selection;
use crate::shader_function::LinearScale;
use crate::{Circle, MaybeSend, MaybeSync, Rectangle};
use std::marker::PhantomData;
use std::sync::Arc;

use super::area::AreaSegment;
use super::line::LineSegment;

// ── Y-axis assignment ───────────────────────────────────────────────────

/// Which y-axis a layer is rendered against.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum YAxisAssignment {
    /// Primary (left) y-axis — default.
    #[default]
    Primary,
    /// Secondary (right) y-axis.
    Secondary,
}

// ── LayerKind enum (enum-over-trait-objects) ─────────────────────────────

/// A chart layer variant — one per supported builder type.
///
/// Using an enum rather than a trait object keeps compile-time type
/// safety, enables pattern-matching exhaustiveness checks, and avoids
/// object-safety constraints.
#[derive(Debug, Clone)]
pub enum LayerKind<T> {
    /// A scatter plot layer.
    Scatter(ScatterPlotBuilder<T>),
    /// A line chart layer.
    Line(LineChartBuilder<T>),
    /// A bar chart layer.
    Bar(BarChartBuilder<T>),
    /// An area chart layer.
    Area(AreaChartBuilder<T>),
}

// ── IntoChartLayer conversion trait ─────────────────────────────────────

/// Convert a chart builder into a [`LayerKind`] for use with
/// [`CompositeChartBuilder::layer`].
pub trait IntoChartLayer<T> {
    /// Convert this builder into a layer variant.
    fn into_layer(self) -> LayerKind<T>;
}

impl<T> IntoChartLayer<T> for ScatterPlotBuilder<T> {
    fn into_layer(self) -> LayerKind<T> {
        LayerKind::Scatter(self)
    }
}

impl<T> IntoChartLayer<T> for LineChartBuilder<T> {
    fn into_layer(self) -> LayerKind<T> {
        LayerKind::Line(self)
    }
}

impl<T> IntoChartLayer<T> for BarChartBuilder<T> {
    fn into_layer(self) -> LayerKind<T> {
        LayerKind::Bar(self)
    }
}

impl<T> IntoChartLayer<T> for AreaChartBuilder<T> {
    fn into_layer(self) -> LayerKind<T> {
        LayerKind::Area(self)
    }
}

// ── Internal layer record ───────────────────────────────────────────────

/// A layer together with its y-axis assignment.
#[derive(Debug, Clone)]
struct CompositeLayer<T> {
    kind: LayerKind<T>,
    y_axis: YAxisAssignment,
}

// ── Domain helpers (pure, no GPU) ───────────────────────────────────────

/// Compute the (min, max) data range for an accessor applied to `data`.
///
/// Returns `None` if `accessor` is `None` or if `data` is empty.
fn compute_domain<T>(accessor: &Option<AccessorFunction<T>>, data: &[T]) -> Option<(f32, f32)> {
    let acc = accessor.as_ref()?;
    if data.is_empty() {
        return None;
    }

    let mut min = f32::INFINITY;
    let mut max = f32::NEG_INFINITY;
    for item in data {
        let val = acc.apply(item).as_f32();
        if val.is_finite() {
            min = min.min(val);
            max = max.max(val);
        }
    }

    if min.is_finite() && max.is_finite() {
        Some((min, max))
    } else {
        None
    }
}

/// Merge two optional `(min, max)` ranges into their union.
pub fn union_domain(a: Option<(f32, f32)>, b: Option<(f32, f32)>) -> Option<(f32, f32)> {
    match (a, b) {
        (Some((a_min, a_max)), Some((b_min, b_max))) => Some((a_min.min(b_min), a_max.max(b_max))),
        (Some(d), None) | (None, Some(d)) => Some(d),
        (None, None) => None,
    }
}

/// Add a small amount of padding to a domain so that data does not sit
/// exactly on the axis edge.  When the range is zero (single-point
/// domain) we expand by ±1 to avoid a degenerate scale.
fn pad_domain(domain: (f32, f32)) -> (f32, f32) {
    let (mut lo, mut hi) = domain;
    if (hi - lo).abs() < f32::EPSILON {
        // Single-point domain — expand symmetrically.
        lo -= 1.0;
        hi += 1.0;
    } else {
        let margin = (hi - lo) * 0.05;
        lo -= margin;
        hi += margin;
    }
    (lo, hi)
}

// ── CompositeChartBuilder ───────────────────────────────────────────────

/// Builder for multi-layer composite charts.
///
/// Layers are rendered in the order they were added (first layer at the
/// bottom, last layer on top).  By default all layers share a single
/// x-axis and a single (left) y-axis whose domains are the union of all
/// layers' data ranges.
///
/// Use [`layer_with_y2`](Self::layer_with_y2) to assign a layer to an
/// independent right-hand y-axis.
#[derive(Debug, Clone)]
pub struct CompositeChartBuilder<T> {
    layers: Vec<CompositeLayer<T>>,
    config: ChartConfig,
    _phantom: PhantomData<T>,
}

impl<T> CompositeChartBuilder<T> {
    /// Create a new, empty composite builder.
    pub fn new() -> Self {
        Self {
            layers: Vec::new(),
            config: ChartConfig::default(),
            _phantom: PhantomData,
        }
    }

    /// Append a layer on the primary (left) y-axis.
    pub fn layer(mut self, builder: impl IntoChartLayer<T>) -> Self {
        self.layers.push(CompositeLayer {
            kind: builder.into_layer(),
            y_axis: YAxisAssignment::Primary,
        });
        self
    }

    /// Append a layer on the secondary (right) y-axis.
    pub fn layer_with_y2(mut self, builder: impl IntoChartLayer<T>) -> Self {
        self.layers.push(CompositeLayer {
            kind: builder.into_layer(),
            y_axis: YAxisAssignment::Secondary,
        });
        self
    }

    /// Return the number of layers currently registered.
    pub fn layer_count(&self) -> usize {
        self.layers.len()
    }
}

// ── ConfigurableBuilder / GridCapableBuilder ────────────────────────────

impl<T> ConfigurableBuilder for CompositeChartBuilder<T> {
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

impl<T> GridCapableBuilder for CompositeChartBuilder<T> {
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

impl<T> Default for CompositeChartBuilder<T> {
    fn default() -> Self {
        Self::new()
    }
}

// ── Domain introspection per layer ──────────────────────────────────────

impl<T> LayerKind<T> {
    /// Return the x-domain `(min, max)` of this layer for the given data.
    fn x_domain(&self, data: &[T]) -> Option<(f32, f32)> {
        match self {
            LayerKind::Scatter(b) => compute_domain(&b.x_accessor, data),
            LayerKind::Line(b) => compute_domain(&b.x_accessor, data),
            LayerKind::Bar(b) => compute_domain(&b.x_accessor, data),
            LayerKind::Area(b) => compute_domain(&b.x_accessor, data),
        }
    }

    /// Return the y-domain `(min, max)` of this layer for the given data.
    fn y_domain(&self, data: &[T]) -> Option<(f32, f32)> {
        match self {
            LayerKind::Scatter(b) => compute_domain(&b.y_accessor, data),
            LayerKind::Line(b) => compute_domain(&b.y_accessor, data),
            LayerKind::Bar(b) => compute_domain(&b.y_accessor, data),
            LayerKind::Area(b) => compute_domain(&b.y_accessor, data),
        }
    }
}

// ── CompositeChart output type ──────────────────────────────────────────

/// A layer that has been built into its concrete Selection type.
///
/// Uses an enum to hold the different `Selection<T, M>` variants produced
/// by the individual chart builders.
#[derive(Debug)]
#[allow(dead_code)]
enum BuiltLayer<T>
where
    T: Clone + MaybeSend + MaybeSync + std::fmt::Debug + 'static,
{
    Scatter(Selection<T, Circle>),
    Line(Selection<LineSegment<T>, Line>),
    Bar(Selection<T, Rectangle>),
    Area(Selection<AreaSegment<T>, Line>),
}

/// The output of [`CompositeChartBuilder::build_with_data`].
///
/// Contains all built layers together with shared axis and grid state.
/// The first layer is designated as the "primary" and owns the
/// [`ComposedChart`] that manages axis/grid pipelines and draw commands.
#[derive(Debug)]
pub struct CompositeChart<T>
where
    T: Clone + MaybeSend + MaybeSync + std::fmt::Debug + 'static,
{
    /// Primary chart (first layer) — owns axis and grid rendering.
    primary: ComposedChart<T, Circle>,
    /// Additional built layers rendered after the primary.
    additional_layers: Vec<BuiltLayer<T>>,
    /// Whether a secondary (right) y-axis is in use.
    has_secondary_y: bool,
}

impl<T> CompositeChart<T>
where
    T: Clone + MaybeSend + MaybeSync + std::fmt::Debug + 'static,
{
    /// Access the primary [`ComposedChart`].
    pub fn primary(&self) -> &ComposedChart<T, Circle> {
        &self.primary
    }

    /// Access the primary [`ComposedChart`] mutably (for rendering).
    pub fn primary_mut(&mut self) -> &mut ComposedChart<T, Circle> {
        &mut self.primary
    }

    /// Return the number of additional layers beyond the primary.
    pub fn additional_layer_count(&self) -> usize {
        self.additional_layers.len()
    }

    /// Whether the secondary y-axis is in use.
    pub fn has_secondary_y_axis(&self) -> bool {
        self.has_secondary_y
    }
}

// ── ChartBuilder implementation ─────────────────────────────────────────

impl<T> ChartBuilder<T> for CompositeChartBuilder<T>
where
    T: Clone + MaybeSend + MaybeSync + std::fmt::Debug + 'static,
{
    type Output = CompositeChart<T>;

    fn build_with_data(
        self,
        data: Vec<T>,
        context: Arc<crate::RenderContext>,
    ) -> GupResult<Self::Output> {
        if self.layers.is_empty() {
            return Err(ChartBuilderError::ConfigurationError {
                message: "CompositeChartBuilder has no layers".to_string(),
            }
            .into());
        }

        if data.is_empty() {
            return Err(ChartBuilderError::EmptyData.into());
        }

        // ── 1. Compute unified domains ──────────────────────────────
        let mut primary_x: Option<(f32, f32)> = None;
        let mut primary_y: Option<(f32, f32)> = None;
        let mut secondary_y: Option<(f32, f32)> = None;

        for layer in &self.layers {
            let lx = layer.kind.x_domain(&data);
            primary_x = union_domain(primary_x, lx);

            let ly = layer.kind.y_domain(&data);
            match layer.y_axis {
                YAxisAssignment::Primary => {
                    primary_y = union_domain(primary_y, ly);
                }
                YAxisAssignment::Secondary => {
                    secondary_y = union_domain(secondary_y, ly);
                }
            }
        }

        let has_secondary = secondary_y.is_some();

        // ── 2. Build scales from unified domains ────────────────────
        let (x_lo, x_hi) = pad_domain(primary_x.unwrap_or((0.0, 1.0)));
        let (py_lo, py_hi) = pad_domain(primary_y.unwrap_or((0.0, 1.0)));

        let x_scale = AxisScale::Linear(LinearScale::new(x_lo, x_hi, -1.0, 1.0));
        let y_scale = AxisScale::Linear(LinearScale::new(py_lo, py_hi, -1.0, 1.0));

        let y2_scale = secondary_y.map(|sy| {
            let (sy_lo, sy_hi) = pad_domain(sy);
            AxisScale::Linear(LinearScale::new(sy_lo, sy_hi, -1.0, 1.0))
        });

        // ── 3. Build each layer's selection ─────────────────────────
        let mut built_layers: Vec<(BuiltLayer<T>, YAxisAssignment)> = Vec::new();

        for layer in self.layers {
            let effective_y_scale = match layer.y_axis {
                YAxisAssignment::Primary => &y_scale,
                YAxisAssignment::Secondary => y2_scale.as_ref().unwrap_or(&y_scale),
            };

            let built = build_layer(
                layer.kind,
                data.clone(),
                Arc::clone(&context),
                &x_scale,
                effective_y_scale,
            )?;

            built_layers.push((built, layer.y_axis));
        }

        // ── 4. Assemble CompositeChart ──────────────────────────────
        // The first layer becomes the primary ComposedChart that owns
        // axis rendering.  We pop it off and use a scatter fallback
        // selection for the primary wrapper.

        // Build a config for the shared chart frame.
        let mut config = self.config;
        config.x_scale = Some(x_scale.clone());
        config.y_scale = Some(y_scale.clone());

        // Create a minimal scatter selection to anchor the primary
        // ComposedChart (axis + grid owner).
        let anchor_selection = Selection::<T, Circle>::new(data.clone(), Arc::clone(&context))?;
        let mut primary_chart = ComposedChart::new(anchor_selection, config);

        // Set up shared axes.
        if primary_chart.config.show_axes {
            let axis_config = AxisConfiguration::default();

            primary_chart.bottom_axis = Some(Box::new(LinearAxis::new(
                AxisPosition::Bottom,
                axis_config.clone(),
            )));
            primary_chart.left_axis = Some(Box::new(LinearAxis::new(
                AxisPosition::Left,
                axis_config.clone(),
            )));

            if has_secondary {
                primary_chart.right_axis =
                    Some(Box::new(LinearAxis::new(AxisPosition::Right, axis_config)));
            }
        }

        // Collect additional layers (all of them — including the first).
        let additional: Vec<BuiltLayer<T>> = built_layers.into_iter().map(|(bl, _)| bl).collect();

        Ok(CompositeChart {
            primary: primary_chart,
            additional_layers: additional,
            has_secondary_y: has_secondary,
        })
    }
}

// ── Per-layer build helper ──────────────────────────────────────────────

/// Build a single layer variant, injecting unified scales.
fn build_layer<T>(
    kind: LayerKind<T>,
    data: Vec<T>,
    context: Arc<crate::RenderContext>,
    x_scale: &AxisScale,
    y_scale: &AxisScale,
) -> GupResult<BuiltLayer<T>>
where
    T: Clone + MaybeSend + MaybeSync + std::fmt::Debug + 'static,
{
    match kind {
        LayerKind::Scatter(mut builder) => {
            builder.config.show_axes = false;
            builder.config.x_scale = Some(x_scale.clone());
            builder.config.y_scale = Some(y_scale.clone());
            let composed = builder.build_with_data(data, context)?;
            Ok(BuiltLayer::Scatter(composed.visualization))
        }
        LayerKind::Line(mut builder) => {
            builder.config.show_axes = false;
            builder.config.x_scale = Some(x_scale.clone());
            builder.config.y_scale = Some(y_scale.clone());
            let composed = builder.build_with_data(data, context)?;
            Ok(BuiltLayer::Line(composed.visualization))
        }
        LayerKind::Bar(mut builder) => {
            builder.config.show_axes = false;
            builder.config.x_scale = Some(x_scale.clone());
            builder.config.y_scale = Some(y_scale.clone());
            let composed = builder.build_with_data(data, context)?;
            Ok(BuiltLayer::Bar(composed.visualization))
        }
        LayerKind::Area(mut builder) => {
            builder.config.show_axes = false;
            builder.config.x_scale = Some(x_scale.clone());
            builder.config.y_scale = Some(y_scale.clone());
            let composed = builder.build_with_data(data, context)?;
            Ok(BuiltLayer::Area(composed.visualization))
        }
    }
}

// ── Convenience constructor ─────────────────────────────────────────────

/// Create a new [`CompositeChartBuilder`].
///
/// # Examples
///
/// ```rust,no_run
/// use gup::chart_builder::builders::composite::composite;
/// use gup::chart_builder::builders::{scatter, line};
///
/// #[derive(Debug, Clone)]
/// struct P { x: f32, y: f32 }
///
/// let builder = composite::<P>()
///     .layer(scatter::<P>())
///     .layer(line::<P>());
/// ```
pub fn composite<T>() -> CompositeChartBuilder<T> {
    CompositeChartBuilder::new()
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chart_builder::accessor::AccessorValue;

    #[derive(Debug, Clone)]
    struct Pt {
        x: f32,
        y: f32,
    }

    // ── Domain helper tests (pure, no GPU) ──────────────────────────

    #[test]
    fn union_domain_both_some() {
        let a = Some((1.0, 5.0));
        let b = Some((3.0, 8.0));
        assert_eq!(union_domain(a, b), Some((1.0, 8.0)));
    }

    #[test]
    fn union_domain_non_overlapping() {
        let a = Some((1.0, 2.0));
        let b = Some((5.0, 10.0));
        assert_eq!(union_domain(a, b), Some((1.0, 10.0)));
    }

    #[test]
    fn union_domain_one_none() {
        assert_eq!(union_domain(Some((1.0, 5.0)), None), Some((1.0, 5.0)));
        assert_eq!(union_domain(None, Some((2.0, 7.0))), Some((2.0, 7.0)));
    }

    #[test]
    fn union_domain_both_none() {
        assert_eq!(union_domain(None, None), None);
    }

    #[test]
    fn union_domain_single_point() {
        let a = Some((3.0, 3.0));
        let b = Some((3.0, 3.0));
        assert_eq!(union_domain(a, b), Some((3.0, 3.0)));
    }

    #[test]
    fn pad_domain_normal_range() {
        let (lo, hi) = pad_domain((0.0, 100.0));
        assert!(lo < 0.0);
        assert!(hi > 100.0);
    }

    #[test]
    fn pad_domain_single_point_does_not_panic() {
        let (lo, hi) = pad_domain((5.0, 5.0));
        assert!(lo < 5.0);
        assert!(hi > 5.0);
    }

    #[test]
    fn compute_domain_with_accessor() {
        let data = vec![
            Pt { x: 1.0, y: 10.0 },
            Pt { x: 5.0, y: 20.0 },
            Pt { x: 3.0, y: 15.0 },
        ];
        let acc = Some(AccessorFunction::new(|d: &Pt| AccessorValue::Float(d.x)));
        let domain = compute_domain(&acc, &data);
        assert_eq!(domain, Some((1.0, 5.0)));
    }

    #[test]
    fn compute_domain_empty_data() {
        let data: Vec<Pt> = vec![];
        let acc = Some(AccessorFunction::new(|d: &Pt| AccessorValue::Float(d.x)));
        assert_eq!(compute_domain(&acc, &data), None);
    }

    #[test]
    fn compute_domain_no_accessor() {
        let data = vec![Pt { x: 1.0, y: 2.0 }];
        let acc: Option<AccessorFunction<Pt>> = None;
        assert_eq!(compute_domain(&acc, &data), None);
    }

    // ── Builder construction tests (no GPU) ─────────────────────────

    #[test]
    fn composite_builder_default_empty() {
        let builder = composite::<Pt>();
        assert_eq!(builder.layer_count(), 0);
    }

    #[test]
    fn composite_builder_layer_count() {
        let builder = composite::<Pt>()
            .layer(ScatterPlotBuilder::<Pt>::new())
            .layer(LineChartBuilder::<Pt>::new())
            .layer_with_y2(BarChartBuilder::<Pt>::new());
        assert_eq!(builder.layer_count(), 3);
    }

    #[test]
    fn composite_builder_configurable() {
        let builder = composite::<Pt>()
            .title("My Composite")
            .width(1200.0)
            .height(800.0)
            .show_axes(true)
            .show_grid(true)
            .layer(ScatterPlotBuilder::<Pt>::new());

        assert_eq!(builder.config.width, 1200.0);
        assert_eq!(builder.config.height, 800.0);
        assert!(builder.config.show_axes);
        assert!(builder.config.show_grid);
        assert_eq!(builder.layer_count(), 1);
    }

    // ── Five distinct domain-unification test cases ─────────────────

    #[test]
    fn domain_unification_overlapping() {
        // Ranges [0, 10] and [5, 15] → [0, 15]
        assert_eq!(
            union_domain(Some((0.0, 10.0)), Some((5.0, 15.0))),
            Some((0.0, 15.0))
        );
    }

    #[test]
    fn domain_unification_non_overlapping() {
        // [0, 2] and [8, 10] → [0, 10]
        assert_eq!(
            union_domain(Some((0.0, 2.0)), Some((8.0, 10.0))),
            Some((0.0, 10.0))
        );
    }

    #[test]
    fn domain_unification_contained() {
        // [0, 100] and [10, 50] → [0, 100]
        assert_eq!(
            union_domain(Some((0.0, 100.0)), Some((10.0, 50.0))),
            Some((0.0, 100.0))
        );
    }

    #[test]
    fn domain_unification_single_point_ranges() {
        // [5, 5] and [10, 10] → [5, 10]
        assert_eq!(
            union_domain(Some((5.0, 5.0)), Some((10.0, 10.0))),
            Some((5.0, 10.0))
        );
    }

    #[test]
    fn domain_unification_negative_ranges() {
        // [-20, -5] and [-10, 10] → [-20, 10]
        assert_eq!(
            union_domain(Some((-20.0, -5.0)), Some((-10.0, 10.0))),
            Some((-20.0, 10.0))
        );
    }
}

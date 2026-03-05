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
use crate::{Circle, MaybeSend, MaybeSync, Rectangle, RenderContext};
use std::marker::PhantomData;
use std::sync::Arc;
use wgpu::{Device, Queue, RenderPass};

use super::area::AreaSegment;
use super::line::LineSegment;

// ── Type-erased layer interfaces ────────────────────────────────────────

/// A built layer whose concrete data type has been erased.
///
/// Used by [`CompositeChart`] to render layers with foreign data types
/// alongside the composite's primary `T`.
trait ErasedBuiltLayer: std::fmt::Debug {
    /// Prepare GPU resources for rendering.
    fn erased_prepare(
        &mut self,
        device: &Device,
        queue: &Queue,
        format: wgpu::TextureFormat,
    ) -> GupResult<()>;

    /// Record draw commands into the render pass.
    fn erased_draw<'a>(&'a self, render_pass: &mut RenderPass<'a>) -> GupResult<()>;

    /// Returns `true` when GPU resources have been prepared.
    fn erased_is_render_ready(&self) -> bool;
}

impl<T> ErasedBuiltLayer for BuiltLayer<T>
where
    T: Clone + MaybeSend + MaybeSync + std::fmt::Debug + 'static,
{
    fn erased_prepare(
        &mut self,
        device: &Device,
        queue: &Queue,
        format: wgpu::TextureFormat,
    ) -> GupResult<()> {
        self.prepare(device, queue, format)
    }

    fn erased_draw<'a>(&'a self, render_pass: &mut RenderPass<'a>) -> GupResult<()> {
        self.draw(render_pass)
    }

    fn erased_is_render_ready(&self) -> bool {
        self.is_render_ready()
    }
}

/// A deferred layer specification that captures a builder, its data, and
/// pre-computed domain information.  The concrete data type is hidden
/// behind this trait so that layers with different `T` can coexist in a
/// single composite builder.
trait ErasedLayerSpec: std::fmt::Debug {
    /// Pre-computed x-domain `(min, max)` of this layer's data.
    fn erased_x_domain(&self) -> Option<(f32, f32)>;

    /// Pre-computed y-domain `(min, max)` of this layer's data.
    fn erased_y_domain(&self) -> Option<(f32, f32)>;

    /// Which y-axis this layer is assigned to.
    fn erased_y_axis(&self) -> YAxisAssignment;

    /// Consume this specification and produce a type-erased built layer,
    /// injecting the unified scales determined by the composite.
    fn build_erased(
        self: Box<Self>,
        context: Arc<RenderContext>,
        x_scale: &AxisScale,
        y_scale: &AxisScale,
    ) -> GupResult<Box<dyn ErasedBuiltLayer>>;
}

/// Concrete implementation of [`ErasedLayerSpec`] for a given data type
/// `T2` which may differ from the composite's primary `T`.
struct TypedLayerSpec<T2>
where
    T2: Clone + MaybeSend + MaybeSync + std::fmt::Debug + 'static,
{
    kind: LayerKind<T2>,
    data: Vec<T2>,
    y_axis: YAxisAssignment,
    cached_x_domain: Option<(f32, f32)>,
    cached_y_domain: Option<(f32, f32)>,
}

impl<T2> std::fmt::Debug for TypedLayerSpec<T2>
where
    T2: Clone + MaybeSend + MaybeSync + std::fmt::Debug + 'static,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TypedLayerSpec")
            .field("kind", &self.kind)
            .field("data_len", &self.data.len())
            .field("y_axis", &self.y_axis)
            .field("cached_x_domain", &self.cached_x_domain)
            .field("cached_y_domain", &self.cached_y_domain)
            .finish()
    }
}

impl<T2> ErasedLayerSpec for TypedLayerSpec<T2>
where
    T2: Clone + MaybeSend + MaybeSync + std::fmt::Debug + 'static,
{
    fn erased_x_domain(&self) -> Option<(f32, f32)> {
        self.cached_x_domain
    }

    fn erased_y_domain(&self) -> Option<(f32, f32)> {
        self.cached_y_domain
    }

    fn erased_y_axis(&self) -> YAxisAssignment {
        self.y_axis
    }

    fn build_erased(
        self: Box<Self>,
        context: Arc<RenderContext>,
        x_scale: &AxisScale,
        y_scale: &AxisScale,
    ) -> GupResult<Box<dyn ErasedBuiltLayer>> {
        let built = build_layer(self.kind, self.data, context, x_scale, y_scale)?;
        Ok(Box::new(built))
    }
}

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

/// A layer entry in the composite builder — either sharing the
/// composite's primary data type `T` or carrying its own (type-erased).
enum AnyCompositeLayer<T> {
    /// Layer that shares the composite's data type `T`.
    Typed(CompositeLayer<T>),
    /// Layer with a foreign data type, captured behind a trait object.
    Erased(Box<dyn ErasedLayerSpec>),
}

impl<T> std::fmt::Debug for AnyCompositeLayer<T>
where
    T: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AnyCompositeLayer::Typed(cl) => f.debug_tuple("Typed").field(cl).finish(),
            AnyCompositeLayer::Erased(spec) => f.debug_tuple("Erased").field(spec).finish(),
        }
    }
}

/// A built layer in the output [`CompositeChart`] — either typed
/// (matching the composite's `T`) or type-erased (foreign `T2`).
enum AnyBuiltLayer<T>
where
    T: Clone + MaybeSend + MaybeSync + std::fmt::Debug + 'static,
{
    /// Built layer with the same data type as the composite.
    Typed(BuiltLayer<T>),
    /// Built layer with a foreign data type, type-erased.
    Erased(Box<dyn ErasedBuiltLayer>),
}

impl<T> std::fmt::Debug for AnyBuiltLayer<T>
where
    T: Clone + MaybeSend + MaybeSync + std::fmt::Debug + 'static,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AnyBuiltLayer::Typed(bl) => f.debug_tuple("Typed").field(bl).finish(),
            AnyBuiltLayer::Erased(bl) => f.debug_tuple("Erased").field(bl).finish(),
        }
    }
}

impl<T> AnyBuiltLayer<T>
where
    T: Clone + MaybeSend + MaybeSync + std::fmt::Debug + 'static,
{
    fn prepare(
        &mut self,
        device: &Device,
        queue: &Queue,
        format: wgpu::TextureFormat,
    ) -> GupResult<()> {
        match self {
            AnyBuiltLayer::Typed(bl) => bl.prepare(device, queue, format),
            AnyBuiltLayer::Erased(bl) => bl.erased_prepare(device, queue, format),
        }
    }

    fn draw<'a>(&'a self, render_pass: &mut RenderPass<'a>) -> GupResult<()> {
        match self {
            AnyBuiltLayer::Typed(bl) => bl.draw(render_pass),
            AnyBuiltLayer::Erased(bl) => bl.erased_draw(render_pass),
        }
    }

    fn is_render_ready(&self) -> bool {
        match self {
            AnyBuiltLayer::Typed(bl) => bl.is_render_ready(),
            AnyBuiltLayer::Erased(bl) => bl.erased_is_render_ready(),
        }
    }
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
///
/// Use [`layer_with_data`](Self::layer_with_data) to add a layer that
/// carries its own data set with a different type from the composite's
/// primary `T`.
#[derive(Debug)]
pub struct CompositeChartBuilder<T> {
    layers: Vec<AnyCompositeLayer<T>>,
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
        self.layers.push(AnyCompositeLayer::Typed(CompositeLayer {
            kind: builder.into_layer(),
            y_axis: YAxisAssignment::Primary,
        }));
        self
    }

    /// Append a layer on the secondary (right) y-axis.
    pub fn layer_with_y2(mut self, builder: impl IntoChartLayer<T>) -> Self {
        self.layers.push(AnyCompositeLayer::Typed(CompositeLayer {
            kind: builder.into_layer(),
            y_axis: YAxisAssignment::Secondary,
        }));
        self
    }

    /// Append a layer with its own data set on the primary (left) y-axis.
    ///
    /// The layer's data type `T2` may differ from the composite's
    /// primary type `T`.  The layer's x and y domains are computed
    /// immediately and will participate in domain unification at build
    /// time.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use gup::chart_builder::builders::composite::composite;
    /// use gup::chart_builder::builders::{scatter, line, AccessorFunction};
    /// use gup::chart_builder::accessor::AccessorValue;
    ///
    /// #[derive(Debug, Clone)]
    /// struct Observation { x: f32, y: f32 }
    ///
    /// #[derive(Debug, Clone)]
    /// struct FitPoint { x: f32, y_hat: f32 }
    ///
    /// let observations = vec![Observation { x: 1.0, y: 2.0 }];
    /// let fit = vec![FitPoint { x: 1.0, y_hat: 2.1 }];
    ///
    /// let builder = composite::<Observation>()
    ///     .layer(
    ///         scatter::<Observation>()
    ///             .x(AccessorFunction::new(|d: &Observation| AccessorValue::Float(d.x)))
    ///             .y(AccessorFunction::new(|d: &Observation| AccessorValue::Float(d.y)))
    ///     )
    ///     .layer_with_data(
    ///         line::<FitPoint>()
    ///             .x(AccessorFunction::new(|d: &FitPoint| AccessorValue::Float(d.x)))
    ///             .y(AccessorFunction::new(|d: &FitPoint| AccessorValue::Float(d.y_hat))),
    ///         fit,
    ///     );
    /// ```
    pub fn layer_with_data<T2, B>(mut self, builder: B, data: Vec<T2>) -> Self
    where
        T2: Clone + MaybeSend + MaybeSync + std::fmt::Debug + 'static,
        B: IntoChartLayer<T2>,
    {
        let kind = builder.into_layer();
        let cached_x_domain = kind.x_domain(&data);
        let cached_y_domain = kind.y_domain(&data);
        self.layers
            .push(AnyCompositeLayer::Erased(Box::new(TypedLayerSpec {
                kind,
                data,
                y_axis: YAxisAssignment::Primary,
                cached_x_domain,
                cached_y_domain,
            })));
        self
    }

    /// Append a layer with its own data set on the secondary (right)
    /// y-axis.
    ///
    /// See [`layer_with_data`](Self::layer_with_data) for details.
    pub fn layer_with_data_y2<T2, B>(mut self, builder: B, data: Vec<T2>) -> Self
    where
        T2: Clone + MaybeSend + MaybeSync + std::fmt::Debug + 'static,
        B: IntoChartLayer<T2>,
    {
        let kind = builder.into_layer();
        let cached_x_domain = kind.x_domain(&data);
        let cached_y_domain = kind.y_domain(&data);
        self.layers
            .push(AnyCompositeLayer::Erased(Box::new(TypedLayerSpec {
                kind,
                data,
                y_axis: YAxisAssignment::Secondary,
                cached_x_domain,
                cached_y_domain,
            })));
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
enum BuiltLayer<T>
where
    T: Clone + MaybeSend + MaybeSync + std::fmt::Debug + 'static,
{
    Scatter(Selection<T, Circle>),
    Line(Selection<LineSegment<T>, Line>),
    Bar(Selection<T, Rectangle>),
    Area(Selection<AreaSegment<T>, Line>),
}

impl<T> BuiltLayer<T>
where
    T: Clone + MaybeSend + MaybeSync + std::fmt::Debug + 'static,
{
    /// Prepare this layer's selection for GPU rendering.
    ///
    /// Must be called before [`draw`](Self::draw).
    fn prepare(
        &mut self,
        device: &Device,
        queue: &Queue,
        _format: wgpu::TextureFormat,
    ) -> GupResult<()> {
        match self {
            BuiltLayer::Scatter(sel) => sel.prepare_render_bound(device, queue, None, None),
            BuiltLayer::Line(sel) => sel.prepare_render_bound(device, queue, None, None),
            BuiltLayer::Bar(sel) => sel.prepare_render_bound(device, queue, None, None),
            BuiltLayer::Area(sel) => sel.prepare_render_bound(device, queue, None, None),
        }
    }

    /// Issue draw commands for this layer into the given render pass.
    fn draw<'a>(&'a self, render_pass: &mut RenderPass<'a>) -> GupResult<()> {
        match self {
            BuiltLayer::Scatter(sel) => sel.render(render_pass),
            BuiltLayer::Line(sel) => sel.render(render_pass),
            BuiltLayer::Bar(sel) => sel.render(render_pass),
            BuiltLayer::Area(sel) => sel.render(render_pass),
        }
    }

    /// Returns `true` when the layer has been prepared for rendering.
    fn is_render_ready(&self) -> bool {
        match self {
            BuiltLayer::Scatter(sel) => sel.is_render_ready(),
            BuiltLayer::Line(sel) => sel.is_render_ready(),
            BuiltLayer::Bar(sel) => sel.is_render_ready(),
            BuiltLayer::Area(sel) => sel.is_render_ready(),
        }
    }
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
    /// All built layers rendered after the primary, in declaration order.
    /// Includes both same-type and type-erased foreign-data layers.
    additional_layers: Vec<AnyBuiltLayer<T>>,
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

    /// Prepare all GPU resources required for rendering.
    ///
    /// This uploads instance buffers for every layer and prepares the
    /// axis/grid pipelines on the primary chart.  Call once before the
    /// first frame, or whenever data changes.
    pub fn prepare_render(
        &mut self,
        device: &Device,
        queue: &Queue,
        format: wgpu::TextureFormat,
    ) -> GupResult<()> {
        // Prepare axis/grid pipelines on the primary chart.
        self.primary.prepare_draw_commands(device, queue, format);

        // Prepare each data layer for rendering.
        for layer in &mut self.additional_layers {
            if !layer.is_render_ready() {
                layer.prepare(device, queue, format)?;
            }
        }

        Ok(())
    }

    /// Record all draw commands into a single render pass.
    ///
    /// Draw order:
    /// 1. Grid lines (behind everything)
    /// 2. Data layers in declaration order (first added = bottom)
    /// 3. Axis lines and tick marks (on top)
    ///
    /// # Errors
    ///
    /// Returns an error if a layer has not been prepared.
    pub fn draw<'a>(&'a self, render_pass: &mut RenderPass<'a>) -> GupResult<()> {
        // 1. Grid lines (behind data).
        self.primary.draw_grid_lines(render_pass);

        // 2. Data layers in declaration order.
        for layer in &self.additional_layers {
            layer.draw(render_pass)?;
        }

        // 3. Axis infrastructure on top.
        self.primary.draw_axis_lines(render_pass);
        self.primary.draw_ticks(render_pass);

        Ok(())
    }

    /// Returns the total number of draw-producing layers (useful for tests).
    pub fn layer_draw_count(&self) -> usize {
        self.additional_layers
            .iter()
            .filter(|l| l.is_render_ready())
            .count()
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

        // At least the typed layers need data; erased layers carry their
        // own.  We allow empty `data` only when all layers are erased.
        let has_typed_layers = self
            .layers
            .iter()
            .any(|l| matches!(l, AnyCompositeLayer::Typed(_)));
        if data.is_empty() && has_typed_layers {
            return Err(ChartBuilderError::EmptyData.into());
        }

        // ── 1. Compute unified domains ──────────────────────────────
        let mut primary_x: Option<(f32, f32)> = None;
        let mut primary_y: Option<(f32, f32)> = None;
        let mut secondary_y: Option<(f32, f32)> = None;

        for layer in &self.layers {
            match layer {
                AnyCompositeLayer::Typed(cl) => {
                    let lx = cl.kind.x_domain(&data);
                    primary_x = union_domain(primary_x, lx);

                    let ly = cl.kind.y_domain(&data);
                    match cl.y_axis {
                        YAxisAssignment::Primary => {
                            primary_y = union_domain(primary_y, ly);
                        }
                        YAxisAssignment::Secondary => {
                            secondary_y = union_domain(secondary_y, ly);
                        }
                    }
                }
                AnyCompositeLayer::Erased(spec) => {
                    let lx = spec.erased_x_domain();
                    primary_x = union_domain(primary_x, lx);

                    let ly = spec.erased_y_domain();
                    match spec.erased_y_axis() {
                        YAxisAssignment::Primary => {
                            primary_y = union_domain(primary_y, ly);
                        }
                        YAxisAssignment::Secondary => {
                            secondary_y = union_domain(secondary_y, ly);
                        }
                    }
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
        let mut built_layers: Vec<AnyBuiltLayer<T>> = Vec::new();

        for layer in self.layers {
            match layer {
                AnyCompositeLayer::Typed(cl) => {
                    let effective_y_scale = match cl.y_axis {
                        YAxisAssignment::Primary => &y_scale,
                        YAxisAssignment::Secondary => y2_scale.as_ref().unwrap_or(&y_scale),
                    };

                    let built = build_layer(
                        cl.kind,
                        data.clone(),
                        Arc::clone(&context),
                        &x_scale,
                        effective_y_scale,
                    )?;

                    built_layers.push(AnyBuiltLayer::Typed(built));
                }
                AnyCompositeLayer::Erased(spec) => {
                    let effective_y_scale = match spec.erased_y_axis() {
                        YAxisAssignment::Primary => &y_scale,
                        YAxisAssignment::Secondary => y2_scale.as_ref().unwrap_or(&y_scale),
                    };

                    let built =
                        spec.build_erased(Arc::clone(&context), &x_scale, effective_y_scale)?;

                    built_layers.push(AnyBuiltLayer::Erased(built));
                }
            }
        }

        // ── 4. Assemble CompositeChart ──────────────────────────────
        // Build a config for the shared chart frame.
        let mut config = self.config;
        config.x_scale = Some(x_scale.clone());
        config.y_scale = Some(y_scale.clone());

        // Create a minimal scatter selection to anchor the primary
        // ComposedChart (axis + grid owner).
        let anchor_data = if data.is_empty() {
            // All layers are erased — create an empty anchor selection.
            Vec::new()
        } else {
            data.clone()
        };
        let anchor_selection = Selection::<T, Circle>::new(anchor_data, Arc::clone(&context))?;
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

        Ok(CompositeChart {
            primary: primary_chart,
            additional_layers: built_layers,
            has_secondary_y: has_secondary,
        })
    }
}

// ── Per-layer build helper ──────────────────────────────────────────────

/// Build a single layer variant, injecting unified scales.
///
/// Each inner builder receives the composite's unified x/y scales via
/// its config.  The builder's own `build_with_data` maps segment
/// positions through those scales to NDC, so no post-build attribute
/// overrides are needed.
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
            let sel = composed.visualization;

            Ok(BuiltLayer::Scatter(sel))
        }
        LayerKind::Line(mut builder) => {
            builder.config.show_axes = false;
            builder.config.x_scale = Some(x_scale.clone());
            builder.config.y_scale = Some(y_scale.clone());
            let composed = builder.build_with_data(data, context)?;
            let sel = composed.visualization;

            Ok(BuiltLayer::Line(sel))
        }
        LayerKind::Bar(mut builder) => {
            builder.config.show_axes = false;
            builder.config.x_scale = Some(x_scale.clone());
            builder.config.y_scale = Some(y_scale.clone());
            let composed = builder.build_with_data(data, context)?;
            let sel = composed.visualization;

            Ok(BuiltLayer::Bar(sel))
        }
        LayerKind::Area(mut builder) => {
            builder.config.show_axes = false;
            builder.config.x_scale = Some(x_scale.clone());
            builder.config.y_scale = Some(y_scale.clone());
            let composed = builder.build_with_data(data, context)?;
            let sel = composed.visualization;

            Ok(BuiltLayer::Area(sel))
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

    // ── Per-layer data tests (GUP-304, no GPU) ─────────────────────

    /// A second data type to test heterogeneous layer composition.
    #[derive(Debug, Clone)]
    struct FitPt {
        x: f32,
        y_hat: f32,
    }

    #[test]
    fn layer_with_data_adds_erased_layer() {
        let builder = composite::<Pt>()
            .layer(
                ScatterPlotBuilder::<Pt>::new()
                    .x(AccessorFunction::new(|d: &Pt| AccessorValue::Float(d.x)))
                    .y(AccessorFunction::new(|d: &Pt| AccessorValue::Float(d.y))),
            )
            .layer_with_data(
                LineChartBuilder::<FitPt>::new()
                    .x(AccessorFunction::new(|d: &FitPt| AccessorValue::Float(d.x)))
                    .y(AccessorFunction::new(|d: &FitPt| {
                        AccessorValue::Float(d.y_hat)
                    })),
                vec![
                    FitPt { x: 1.0, y_hat: 2.0 },
                    FitPt {
                        x: 5.0,
                        y_hat: 10.0,
                    },
                ],
            );
        assert_eq!(builder.layer_count(), 2);
    }

    #[test]
    fn layer_with_data_y2_adds_erased_secondary_layer() {
        let builder = composite::<Pt>()
            .layer(ScatterPlotBuilder::<Pt>::new())
            .layer_with_data_y2(
                LineChartBuilder::<FitPt>::new()
                    .x(AccessorFunction::new(|d: &FitPt| AccessorValue::Float(d.x)))
                    .y(AccessorFunction::new(|d: &FitPt| {
                        AccessorValue::Float(d.y_hat)
                    })),
                vec![FitPt { x: 1.0, y_hat: 2.0 }],
            );
        assert_eq!(builder.layer_count(), 2);
    }

    #[test]
    fn erased_layer_spec_domain_computation() {
        let fit_data = vec![
            FitPt { x: 0.0, y_hat: 5.0 },
            FitPt {
                x: 10.0,
                y_hat: 25.0,
            },
        ];

        let kind = LineChartBuilder::<FitPt>::new()
            .x(AccessorFunction::new(|d: &FitPt| AccessorValue::Float(d.x)))
            .y(AccessorFunction::new(|d: &FitPt| {
                AccessorValue::Float(d.y_hat)
            }))
            .into_layer();

        let x_dom = kind.x_domain(&fit_data);
        let y_dom = kind.y_domain(&fit_data);

        assert_eq!(x_dom, Some((0.0, 10.0)));
        assert_eq!(y_dom, Some((5.0, 25.0)));
    }

    #[test]
    fn typed_layer_spec_caches_domain() {
        let data = vec![
            FitPt { x: 2.0, y_hat: 8.0 },
            FitPt {
                x: 6.0,
                y_hat: 12.0,
            },
        ];
        let kind = ScatterPlotBuilder::<FitPt>::new()
            .x(AccessorFunction::new(|d: &FitPt| AccessorValue::Float(d.x)))
            .y(AccessorFunction::new(|d: &FitPt| {
                AccessorValue::Float(d.y_hat)
            }))
            .into_layer();

        let spec = TypedLayerSpec {
            cached_x_domain: kind.x_domain(&data),
            cached_y_domain: kind.y_domain(&data),
            kind,
            data,
            y_axis: YAxisAssignment::Primary,
        };

        assert_eq!(spec.erased_x_domain(), Some((2.0, 6.0)));
        assert_eq!(spec.erased_y_domain(), Some((8.0, 12.0)));
        assert_eq!(spec.erased_y_axis(), YAxisAssignment::Primary);
    }

    #[test]
    fn mixed_domain_unification_typed_and_erased() {
        // Simulate the domain unification that build_with_data performs.
        // Typed layer: Pt with x=[1,5], y=[2,7]
        // Erased layer: FitPt with x=[0,10], y=[5,25]
        let typed_x = Some((1.0, 5.0));
        let typed_y = Some((2.0, 7.0));
        let erased_x = Some((0.0, 10.0));
        let erased_y = Some((5.0, 25.0));

        let unified_x = union_domain(typed_x, erased_x);
        let unified_y = union_domain(typed_y, erased_y);

        assert_eq!(unified_x, Some((0.0, 10.0)));
        assert_eq!(unified_y, Some((2.0, 25.0)));
    }

    #[test]
    fn existing_layer_api_unchanged() {
        // Verify the original .layer() and .layer_with_y2() still work.
        let builder = composite::<Pt>()
            .layer(ScatterPlotBuilder::<Pt>::new())
            .layer(LineChartBuilder::<Pt>::new())
            .layer_with_y2(BarChartBuilder::<Pt>::new());
        assert_eq!(builder.layer_count(), 3);
    }
}

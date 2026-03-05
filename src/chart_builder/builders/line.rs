// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Line chart builder with fluent API for polyline visualisation.
//!
//! Provides [`LineChartBuilder`] for creating GPU-accelerated line charts
//! with automatic x-sorting, multi-series support, step and monotone
//! interpolation, and integrated axes.
//!
//! # Performance
//!
//! The builder evaluates accessors CPU-side to produce `N − 1` line
//! segments from `N` data points (or more when step/monotone
//! interpolation doubles/triples the segment count). Sorting is
//! `O(N log N)` via a stable sort. For datasets beyond ~100 k points
//! consider disabling `sort_by_x` if data is already ordered.

use super::{
    AccessorFunction, ConfigurableBuilder, GridCapableBuilder, NdcBounds,
    validate_required_accessors,
};
use crate::RenderContext;
use crate::chart_builder::accessor::AccessorValue;
use crate::chart_builder::{
    AxisScale, ChartBuilder, ChartBuilderError, ChartConfig, ComposedChart,
};
use crate::error::GupResult;
use crate::grid::{GridConfiguration, GridLineConfig};
use crate::label::LabelFormatter;
use crate::mark::line::Line;
use crate::selection::Selection;
use crate::shader_function::ColorScale;
use crate::{MaybeSend, MaybeSync};
use std::marker::PhantomData;
use std::sync::Arc;

// ── Categorical colour palette ──────────────────────────────────────────

/// Default categorical palette (same colours used by bar charts).
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

// ── LineSegment wrapper ─────────────────────────────────────────────────

/// A single segment in a polyline, derived from two adjacent data points.
///
/// `LineSegment<T>` stores the original data point at the start of the
/// segment together with the pre-computed start and end positions, colour,
/// and width.  It is the data item type held by the `Selection<LineSegment<T>, Line>`
/// returned from [`LineChartBuilder::build_with_data`].
#[derive(Debug, Clone)]
pub struct LineSegment<T> {
    /// The original data point at the segment's start.
    pub data: T,
    /// Start position `[x, y]` in data-space coordinates.
    pub start_pos: [f32; 2],
    /// End position `[x, y]` in data-space coordinates.
    pub end_pos: [f32; 2],
    /// Segment colour `[r, g, b, a]`.
    pub color: [f32; 4],
    /// Segment width in pixels.
    pub width: f32,
}

// ── Line interpolation ──────────────────────────────────────────────────

/// Line interpolation methods.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum LineInterpolation {
    /// Linear interpolation (straight segments between points).
    #[default]
    Linear,
    /// Step function — vertical step *before* each horizontal run
    /// (y changes before x changes).
    StepBefore,
    /// Step function — vertical step *after* each horizontal run
    /// (x changes before y changes).
    StepAfter,
    /// Smooth monotone cubic (Fritsch–Carlson) interpolation.
    ///
    /// Generates intermediate points on the CPU so that the resulting
    /// polyline passes through all original data points with monotone
    /// tangent slopes, avoiding overshoot.
    Monotone,
    /// Legacy alias for [`Monotone`](Self::Monotone).
    #[deprecated(since = "0.1.0", note = "Use `Monotone` instead")]
    Curve,
}

/// Line chart builder providing fluent API for polyline visualisation.
///
/// # Performance
///
/// Segment construction is `O(N log N)` when `sort_by_x` is enabled (the
/// default), dominated by the stable sort. When sorting is disabled the
/// cost is `O(N)`. For datasets beyond ~100 k points consider:
///
/// - Disabling `sort_by_x` if data is already in x-order.
/// - Using `LineInterpolation::Linear` (step and monotone modes multiply
///   the segment count by 2× and 8× respectively).
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

    /// Enable smooth monotone cubic interpolation.
    pub fn smooth(mut self) -> Self {
        self.interpolation = LineInterpolation::Monotone;
        self
    }

    /// Enable monotone cubic interpolation (alias for [`smooth`](Self::smooth)).
    pub fn monotone(mut self) -> Self {
        self.interpolation = LineInterpolation::Monotone;
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
    /// [`LinearScale`](crate::shader_function::LinearScale) and [`LogScale`](crate::shader_function::LogScale).
    /// The scale's domain is used to auto-configure axis tick generation.
    pub fn x_scale(mut self, scale: impl Into<AxisScale>) -> Self {
        self.config.x_scale = Some(scale.into());
        self
    }

    /// Set the Y-axis scale.
    ///
    /// Accepts any scale type that implements `Into<AxisScale>`, including
    /// [`LinearScale`](crate::shader_function::LinearScale) and [`LogScale`](crate::shader_function::LogScale).
    /// The scale's domain is used to auto-configure axis tick generation.
    pub fn y_scale(mut self, scale: impl Into<AxisScale>) -> Self {
        self.config.y_scale = Some(scale.into());
        self
    }

    /// Set the colour scale for value-to-colour mapping.
    ///
    /// When set, the [`ColorScale`] shader function is wired into the
    /// chart's shader pipeline so that a numeric data value is mapped to
    /// an RGBA colour entirely on the GPU.
    pub fn color_scale(mut self, scale: impl Into<ColorScale>) -> Self {
        self.config.color_scale = Some(scale.into());
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

    fn x_tick_format(mut self, formatter: impl LabelFormatter) -> Self {
        self.config.x_label_formatter = Some(std::sync::Arc::new(formatter));
        self
    }

    fn y_tick_format(mut self, formatter: impl LabelFormatter) -> Self {
        self.config.y_label_formatter = Some(std::sync::Arc::new(formatter));
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
    type Output = ComposedChart<LineSegment<T>, Line>;

    fn build_with_data(self, data: Vec<T>, context: Arc<RenderContext>) -> GupResult<Self::Output> {
        // Validate required accessors
        validate_required_accessors(&self.x_accessor, &self.y_accessor)?;

        if data.is_empty() {
            return Err(ChartBuilderError::EmptyData.into());
        }

        let x_acc = self.x_accessor.as_ref().unwrap();
        let y_acc = self.y_accessor.as_ref().unwrap();

        // ── Evaluate accessor values for each data point ────────────
        struct EvaluatedPoint<T> {
            data: T,
            x: f32,
            y: f32,
            series_key: Option<String>,
            color: Option<[f32; 4]>,
        }

        let mut points: Vec<EvaluatedPoint<T>> = data
            .into_iter()
            .map(|d| {
                let x_val = x_acc.apply(&d).as_f32();
                let y_val = y_acc.apply(&d).as_f32();
                let (series_key, color) = if let Some(stroke_acc) = &self.stroke_accessor {
                    match stroke_acc.apply(&d) {
                        AccessorValue::Color(c) => (None, Some(c)),
                        AccessorValue::String(s) | AccessorValue::Categorical(s) => (Some(s), None),
                        other => (Some(format!("{}", other.as_f32())), None),
                    }
                } else {
                    (None, None)
                };
                EvaluatedPoint {
                    data: d,
                    x: x_val,
                    y: y_val,
                    series_key,
                    color,
                }
            })
            .collect();

        // ── Sort by x when requested ────────────────────────────────
        if self.sort_by_x {
            points.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal));
        }

        // ── Compute data domain from raw points ─────────────────────
        // Must be done before `self.config` is moved into ComposedChart.
        let (x_min, x_max) = if let Some(scale) = &self.config.x_scale {
            (scale.domain_min(), scale.domain_max())
        } else {
            auto_domain_from_iter(points.iter().map(|p| p.x))
        };
        let (y_min, y_max) = if let Some(scale) = &self.config.y_scale {
            (scale.domain_min(), scale.domain_max())
        } else {
            auto_domain_from_iter(points.iter().map(|p| p.y))
        };

        // ── Determine default stroke width ──────────────────────────
        let default_width: f32 = if let Some(ref w_acc) = self.stroke_width_accessor {
            // Evaluate on first point to get a constant width
            w_acc.apply(&points[0].data).as_f32()
        } else {
            2.0 // default 2px
        };

        // ── Group by series ─────────────────────────────────────────
        // Each group maps a series label to an ordered list of indices
        // into `points`.  When no stroke accessor is set, all points
        // belong to a single default series.

        let groups: Vec<(String, Vec<usize>)> = {
            let mut label_order: Vec<String> = Vec::new();
            let mut label_indices: std::collections::HashMap<String, Vec<usize>> =
                std::collections::HashMap::new();

            for (i, pt) in points.iter().enumerate() {
                let key = pt.series_key.clone().unwrap_or_default();
                label_indices
                    .entry(key.clone())
                    .or_insert_with(|| {
                        label_order.push(key);
                        Vec::new()
                    })
                    .push(i);
            }

            label_order
                .into_iter()
                .map(|label| {
                    let indices = label_indices.remove(&label).unwrap();
                    (label, indices)
                })
                .collect()
        };

        // ── Assign colours per series ───────────────────────────────
        let series_colors: Vec<[f32; 4]> = groups
            .iter()
            .enumerate()
            .map(|(series_idx, (_label, indices))| {
                // If the first point in this series has a literal colour, use it.
                if let Some(c) = points[indices[0]].color {
                    return c;
                }
                // Otherwise auto-assign from the categorical palette.
                DEFAULT_PALETTE[series_idx % DEFAULT_PALETTE.len()]
            })
            .collect();

        // ── Build segments ──────────────────────────────────────────
        let mut segments: Vec<LineSegment<T>> = Vec::new();

        for (series_idx, (_label, indices)) in groups.iter().enumerate() {
            // Collect (x, y) coordinates for this series
            let series_pts: Vec<(f32, f32)> = indices
                .iter()
                .map(|&i| (points[i].x, points[i].y))
                .collect();

            if series_pts.len() < 2 {
                continue;
            }

            // Apply interpolation to produce the final polyline coords
            #[allow(deprecated)]
            let interpolated = match self.interpolation {
                LineInterpolation::Linear => series_pts.clone(),
                LineInterpolation::StepBefore => interpolate_step_before(&series_pts),
                LineInterpolation::StepAfter => interpolate_step_after(&series_pts),
                LineInterpolation::Monotone | LineInterpolation::Curve => {
                    interpolate_monotone(&series_pts)
                }
            };

            let color = series_colors[series_idx];

            // Build segments from adjacent pairs
            for pair in interpolated.windows(2) {
                let (sx, sy) = pair[0];
                let (ex, ey) = pair[1];

                // For the data item we pick the original start point of
                // the enclosing original-data pair.  For interpolated
                // points (step / monotone) that may be intermediate, we
                // simply use the first data item of the series as a
                // reasonable fallback.
                let data_idx = indices[0]; // fallback
                segments.push(LineSegment {
                    data: points[data_idx].data.clone(),
                    start_pos: [sx, sy],
                    end_pos: [ex, ey],
                    color,
                    width: default_width,
                });
            }
        }

        if segments.is_empty() {
            return Err(ChartBuilderError::EmptyData.into());
        }

        // ── Create Selection<LineSegment<T>, Line> ──────────────────
        let selection = Selection::<LineSegment<T>, Line>::new(segments, context.clone())?;

        // ── Wrap in ComposedChart with axes ─────────────────────────
        // Axes must be added *before* computing NDC bounds so that
        // axis margins are accounted for in the chart area.
        let mut composed_chart = ComposedChart::new(selection, self.config).with_default_axes();

        // ── Compute chart area → NDC bounds ─────────────────────────
        let chart_area = composed_chart.calculate_chart_area();
        let w = composed_chart.config.width;
        let h = composed_chart.config.height;
        let ndc = NdcBounds {
            left: (chart_area.x / w) * 2.0 - 1.0,
            right: ((chart_area.x + chart_area.width) / w) * 2.0 - 1.0,
            top: 1.0 - (chart_area.y / h) * 2.0,
            bottom: 1.0 - ((chart_area.y + chart_area.height) / h) * 2.0,
        };

        // ── NDC mapping helpers ─────────────────────────────────────
        let x_span = x_max - x_min;
        let y_span = y_max - y_min;

        // Convert stroke width from logical pixels to NDC units.
        let ndc_width_per_pixel = 2.0 / w;

        // ── Attr bindings with data→NDC mapping ─────────────────────
        composed_chart
            .visualization
            .attr("start", move |seg: &LineSegment<T>| {
                let tx = if x_span.abs() < f32::EPSILON {
                    0.5
                } else {
                    (seg.start_pos[0] - x_min) / x_span
                };
                let ty = if y_span.abs() < f32::EPSILON {
                    0.5
                } else {
                    (seg.start_pos[1] - y_min) / y_span
                };
                [
                    ndc.left + tx * (ndc.right - ndc.left),
                    ndc.bottom + ty * (ndc.top - ndc.bottom),
                ]
            });
        composed_chart
            .visualization
            .attr("end", move |seg: &LineSegment<T>| {
                let tx = if x_span.abs() < f32::EPSILON {
                    0.5
                } else {
                    (seg.end_pos[0] - x_min) / x_span
                };
                let ty = if y_span.abs() < f32::EPSILON {
                    0.5
                } else {
                    (seg.end_pos[1] - y_min) / y_span
                };
                [
                    ndc.left + tx * (ndc.right - ndc.left),
                    ndc.bottom + ty * (ndc.top - ndc.bottom),
                ]
            });
        composed_chart
            .visualization
            .attr("color", |seg: &LineSegment<T>| seg.color);
        composed_chart
            .visualization
            .attr("width", move |seg: &LineSegment<T>| {
                seg.width * ndc_width_per_pixel
            });

        // ── Prepare GPU render pipeline at build time ───────────────
        // This makes the Selection render-ready so that
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

// ── Data-domain helpers ──────────────────────────────────────────────────

/// Compute `(min, max)` with 5 % padding from an iterator of float values.
///
/// Mirrors the `auto_domain` logic used by the scatter chart builder so
/// that line charts get the same axis padding.
fn auto_domain_from_iter(values: impl Iterator<Item = f32>) -> (f32, f32) {
    let mut lo = f32::INFINITY;
    let mut hi = f32::NEG_INFINITY;
    for v in values {
        lo = lo.min(v);
        hi = hi.max(v);
    }
    let span = hi - lo;
    if span.abs() < f32::EPSILON {
        // All values identical — give a ±1 range.
        (lo - 1.0, hi + 1.0)
    } else {
        let pad = span * 0.05;
        (lo - pad, hi + pad)
    }
}

// ── Interpolation helpers ───────────────────────────────────────────────

/// Step-before: y changes *before* x changes.
///
/// For each consecutive pair `(x0, y0) → (x1, y1)` inserts an intermediate
/// point `(x0, y1)` so the polyline steps vertically first.
///
/// Input:  `[(x0,y0), (x1,y1), (x2,y2)]`
/// Output: `[(x0,y0), (x0,y1), (x1,y1), (x1,y2), (x2,y2)]`
fn interpolate_step_before(pts: &[(f32, f32)]) -> Vec<(f32, f32)> {
    let mut out = Vec::with_capacity(pts.len() * 2 - 1);
    out.push(pts[0]);
    for i in 1..pts.len() {
        // Vertical step at x of previous point
        out.push((pts[i - 1].0, pts[i].1));
        out.push(pts[i]);
    }
    out
}

/// Step-after: x changes *before* y changes.
///
/// For each consecutive pair `(x0, y0) → (x1, y1)` inserts an intermediate
/// point `(x1, y0)` so the polyline runs horizontally first.
///
/// Input:  `[(x0,y0), (x1,y1), (x2,y2)]`
/// Output: `[(x0,y0), (x1,y0), (x1,y1), (x2,y1), (x2,y2)]`
fn interpolate_step_after(pts: &[(f32, f32)]) -> Vec<(f32, f32)> {
    let mut out = Vec::with_capacity(pts.len() * 2 - 1);
    out.push(pts[0]);
    for i in 1..pts.len() {
        // Horizontal run to the next x
        out.push((pts[i].0, pts[i - 1].1));
        out.push(pts[i]);
    }
    out
}

/// Monotone cubic (Fritsch–Carlson) interpolation.
///
/// Generates smooth intermediate points between each data-point pair
/// while preserving monotonicity (no overshoot).  Uses 8 sub-segments
/// per original interval.
fn interpolate_monotone(pts: &[(f32, f32)]) -> Vec<(f32, f32)> {
    let n = pts.len();
    if n < 2 {
        return pts.to_vec();
    }
    if n == 2 {
        // Only two points — linear is the only option
        return pts.to_vec();
    }

    // 1. Compute slopes of secant lines (deltas)
    let mut dx: Vec<f32> = Vec::with_capacity(n - 1);
    let mut dy: Vec<f32> = Vec::with_capacity(n - 1);
    let mut slopes: Vec<f32> = Vec::with_capacity(n - 1);
    for i in 0..n - 1 {
        let d_x = pts[i + 1].0 - pts[i].0;
        let d_y = pts[i + 1].1 - pts[i].1;
        dx.push(d_x);
        dy.push(d_y);
        slopes.push(if d_x.abs() < 1e-12 { 0.0 } else { d_y / d_x });
    }

    // 2. Compute initial tangent slopes (Fritsch–Carlson)
    let mut tangents: Vec<f32> = vec![0.0; n];
    tangents[0] = slopes[0];
    tangents[n - 1] = slopes[n - 2];
    for i in 1..n - 1 {
        if slopes[i - 1].signum() != slopes[i].signum()
            || slopes[i - 1].abs() < 1e-12
            || slopes[i].abs() < 1e-12
        {
            tangents[i] = 0.0;
        } else {
            tangents[i] = (slopes[i - 1] + slopes[i]) / 2.0;
        }
    }

    // 3. Adjust tangents to ensure monotonicity
    for i in 0..n - 1 {
        if slopes[i].abs() < 1e-12 {
            tangents[i] = 0.0;
            tangents[i + 1] = 0.0;
        } else {
            let alpha = tangents[i] / slopes[i];
            let beta = tangents[i + 1] / slopes[i];
            // Clamp to the monotonicity region
            let s = alpha * alpha + beta * beta;
            if s > 9.0 {
                let tau = 3.0 / s.sqrt();
                tangents[i] = tau * alpha * slopes[i];
                tangents[i + 1] = tau * beta * slopes[i];
            }
        }
    }

    // 4. Evaluate the Hermite spline at sub-steps
    const STEPS: usize = 8;
    let mut out = Vec::with_capacity((n - 1) * STEPS + 1);
    for i in 0..n - 1 {
        let h = dx[i];
        for s in 0..STEPS {
            let t = s as f32 / STEPS as f32;
            let t2 = t * t;
            let t3 = t2 * t;
            // Hermite basis functions
            let h00 = 2.0 * t3 - 3.0 * t2 + 1.0;
            let h10 = t3 - 2.0 * t2 + t;
            let h01 = -2.0 * t3 + 3.0 * t2;
            let h11 = t3 - t2;

            let x = pts[i].0 + t * h;
            let y = h00 * pts[i].1
                + h10 * h * tangents[i]
                + h01 * pts[i + 1].1
                + h11 * h * tangents[i + 1];
            out.push((x, y));
        }
    }
    // Push the final point
    out.push(pts[n - 1]);
    out
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

    // ── Basic builder tests ─────────────────────────────────────────

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

        let chart = result.unwrap();
        // 3 data points → 2 segments
        assert_eq!(chart.len(), 2);
    }

    // ── Interpolation configuration tests ───────────────────────────

    #[test]
    fn test_line_chart_interpolation_methods() {
        let builder = line::<TimePoint>().linear();
        assert_eq!(builder.interpolation, LineInterpolation::Linear);

        let builder = line::<TimePoint>().smooth();
        assert_eq!(builder.interpolation, LineInterpolation::Monotone);

        let builder = line::<TimePoint>().monotone();
        assert_eq!(builder.interpolation, LineInterpolation::Monotone);

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
        let data = vec![
            TimePoint {
                time: 1.0,
                value: 20.0,
                series: "test".to_string(),
            },
            TimePoint {
                time: 2.0,
                value: 30.0,
                series: "test".to_string(),
            },
        ];

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

    // ── Segment count tests (AC2) ───────────────────────────────────

    #[tokio::test]
    async fn test_segment_count_n_minus_one() {
        let context = Arc::new(RenderContext::new().await.unwrap());

        // 5 points → 4 segments
        let data: Vec<TimePoint> = (0..5)
            .map(|i| TimePoint {
                time: i as f32,
                value: (i * 10) as f32,
                series: "A".to_string(),
            })
            .collect();

        let chart = line()
            .x(AccessorFunction::new(|d: &TimePoint| {
                AccessorValue::Float(d.time)
            }))
            .y(AccessorFunction::new(|d: &TimePoint| {
                AccessorValue::Float(d.value)
            }))
            .build_with_data(data, context)
            .unwrap();

        assert_eq!(chart.len(), 4);
    }

    #[tokio::test]
    async fn test_segments_span_correct_start_end() {
        let context = Arc::new(RenderContext::new().await.unwrap());

        // Unsorted data — builder should sort by x
        let data = vec![
            TimePoint {
                time: 3.0,
                value: 30.0,
                series: "A".to_string(),
            },
            TimePoint {
                time: 1.0,
                value: 10.0,
                series: "A".to_string(),
            },
            TimePoint {
                time: 2.0,
                value: 20.0,
                series: "A".to_string(),
            },
        ];

        let chart = line()
            .x(AccessorFunction::new(|d: &TimePoint| {
                AccessorValue::Float(d.time)
            }))
            .y(AccessorFunction::new(|d: &TimePoint| {
                AccessorValue::Float(d.value)
            }))
            .build_with_data(data, context)
            .unwrap();

        let segments = chart.visualization.data();
        assert_eq!(segments.len(), 2);

        // After sorting: (1,10) → (2,20) → (3,30)
        assert_eq!(segments[0].start_pos, [1.0, 10.0]);
        assert_eq!(segments[0].end_pos, [2.0, 20.0]);
        assert_eq!(segments[1].start_pos, [2.0, 20.0]);
        assert_eq!(segments[1].end_pos, [3.0, 30.0]);
    }

    #[tokio::test]
    async fn test_sort_by_x_disabled_preserves_order() {
        let context = Arc::new(RenderContext::new().await.unwrap());

        let data = vec![
            TimePoint {
                time: 3.0,
                value: 30.0,
                series: "A".to_string(),
            },
            TimePoint {
                time: 1.0,
                value: 10.0,
                series: "A".to_string(),
            },
            TimePoint {
                time: 2.0,
                value: 20.0,
                series: "A".to_string(),
            },
        ];

        let chart = line()
            .x(AccessorFunction::new(|d: &TimePoint| {
                AccessorValue::Float(d.time)
            }))
            .y(AccessorFunction::new(|d: &TimePoint| {
                AccessorValue::Float(d.value)
            }))
            .sort_x(false)
            .build_with_data(data, context)
            .unwrap();

        let segments = chart.visualization.data();
        // Original order: (3,30) → (1,10) → (2,20)
        assert_eq!(segments[0].start_pos, [3.0, 30.0]);
        assert_eq!(segments[0].end_pos, [1.0, 10.0]);
        assert_eq!(segments[1].start_pos, [1.0, 10.0]);
        assert_eq!(segments[1].end_pos, [2.0, 20.0]);
    }

    // ── Multi-series tests (AC3) ────────────────────────────────────

    #[tokio::test]
    async fn test_multi_series_produces_separate_polylines() {
        let context = Arc::new(RenderContext::new().await.unwrap());

        let data = vec![
            TimePoint {
                time: 0.0,
                value: 10.0,
                series: "A".to_string(),
            },
            TimePoint {
                time: 1.0,
                value: 20.0,
                series: "A".to_string(),
            },
            TimePoint {
                time: 2.0,
                value: 15.0,
                series: "A".to_string(),
            },
            TimePoint {
                time: 0.0,
                value: 5.0,
                series: "B".to_string(),
            },
            TimePoint {
                time: 1.0,
                value: 25.0,
                series: "B".to_string(),
            },
            TimePoint {
                time: 2.0,
                value: 30.0,
                series: "B".to_string(),
            },
        ];

        let chart = line()
            .x(AccessorFunction::new(|d: &TimePoint| {
                AccessorValue::Float(d.time)
            }))
            .y(AccessorFunction::new(|d: &TimePoint| {
                AccessorValue::Float(d.value)
            }))
            .color(AccessorFunction::new(|d: &TimePoint| {
                AccessorValue::String(d.series.clone())
            }))
            .build_with_data(data, context)
            .unwrap();

        let segments = chart.visualization.data();
        // 3 points per series → 2 segments per series → 4 total
        assert_eq!(segments.len(), 4);

        // Series A and series B should have distinct colours
        let color_a = segments[0].color;
        let color_b = segments[2].color;
        assert_ne!(
            color_a, color_b,
            "Multi-series should have distinct colours"
        );
    }

    #[tokio::test]
    async fn test_single_series_no_stroke_accessor() {
        let context = Arc::new(RenderContext::new().await.unwrap());

        let data = vec![
            TimePoint {
                time: 0.0,
                value: 10.0,
                series: "A".to_string(),
            },
            TimePoint {
                time: 1.0,
                value: 20.0,
                series: "A".to_string(),
            },
        ];

        let chart = line()
            .x(AccessorFunction::new(|d: &TimePoint| {
                AccessorValue::Float(d.time)
            }))
            .y(AccessorFunction::new(|d: &TimePoint| {
                AccessorValue::Float(d.value)
            }))
            .build_with_data(data, context)
            .unwrap();

        // Should work fine — 1 segment, default palette colour
        assert_eq!(chart.len(), 1);
    }

    // ── Interpolation mode tests (AC5) ──────────────────────────────

    #[test]
    fn test_step_before_interpolation() {
        let pts = vec![(0.0, 0.0), (1.0, 1.0), (2.0, 0.5)];
        let result = interpolate_step_before(&pts);
        // Expected: (0,0), (0,1), (1,1), (1,0.5), (2,0.5)
        assert_eq!(result.len(), 5);
        assert_eq!(result[0], (0.0, 0.0));
        assert_eq!(result[1], (0.0, 1.0)); // vertical step at x=0
        assert_eq!(result[2], (1.0, 1.0));
        assert_eq!(result[3], (1.0, 0.5)); // vertical step at x=1
        assert_eq!(result[4], (2.0, 0.5));
    }

    #[test]
    fn test_step_after_interpolation() {
        let pts = vec![(0.0, 0.0), (1.0, 1.0), (2.0, 0.5)];
        let result = interpolate_step_after(&pts);
        // Expected: (0,0), (1,0), (1,1), (2,1), (2,0.5)
        assert_eq!(result.len(), 5);
        assert_eq!(result[0], (0.0, 0.0));
        assert_eq!(result[1], (1.0, 0.0)); // horizontal run to x=1
        assert_eq!(result[2], (1.0, 1.0));
        assert_eq!(result[3], (2.0, 1.0)); // horizontal run to x=2
        assert_eq!(result[4], (2.0, 0.5));
    }

    #[test]
    fn test_monotone_interpolation_passes_through_data_points() {
        let pts = vec![(0.0, 0.0), (1.0, 1.0), (2.0, 0.5)];
        let result = interpolate_monotone(&pts);

        // Monotone uses 8 sub-steps per interval plus final point
        // 2 intervals × 8 steps + 1 = 17 points → 16 segments
        assert_eq!(result.len(), 17);

        // First point matches
        assert!((result[0].0 - 0.0).abs() < 1e-5);
        assert!((result[0].1 - 0.0).abs() < 1e-5);

        // Point at boundary of interval 0→1 (index 8)
        assert!((result[8].0 - 1.0).abs() < 1e-5);
        assert!((result[8].1 - 1.0).abs() < 1e-5);

        // Final point matches
        assert!((result[16].0 - 2.0).abs() < 1e-5);
        assert!((result[16].1 - 0.5).abs() < 1e-5);
    }

    #[tokio::test]
    async fn test_step_before_segment_count() {
        let context = Arc::new(RenderContext::new().await.unwrap());

        let data = vec![
            TimePoint {
                time: 0.0,
                value: 0.0,
                series: "A".to_string(),
            },
            TimePoint {
                time: 1.0,
                value: 1.0,
                series: "A".to_string(),
            },
            TimePoint {
                time: 2.0,
                value: 0.5,
                series: "A".to_string(),
            },
        ];

        let chart = line()
            .x(AccessorFunction::new(|d: &TimePoint| {
                AccessorValue::Float(d.time)
            }))
            .y(AccessorFunction::new(|d: &TimePoint| {
                AccessorValue::Float(d.value)
            }))
            .step()
            .build_with_data(data, context)
            .unwrap();

        // 3 points → step_before produces 5 interpolated points → 4 segments
        assert_eq!(chart.len(), 4);
    }

    #[tokio::test]
    async fn test_monotone_segment_count() {
        let context = Arc::new(RenderContext::new().await.unwrap());

        let data = vec![
            TimePoint {
                time: 0.0,
                value: 0.0,
                series: "A".to_string(),
            },
            TimePoint {
                time: 1.0,
                value: 1.0,
                series: "A".to_string(),
            },
            TimePoint {
                time: 2.0,
                value: 0.5,
                series: "A".to_string(),
            },
        ];

        let chart = line()
            .x(AccessorFunction::new(|d: &TimePoint| {
                AccessorValue::Float(d.time)
            }))
            .y(AccessorFunction::new(|d: &TimePoint| {
                AccessorValue::Float(d.value)
            }))
            .monotone()
            .build_with_data(data, context)
            .unwrap();

        // 3 points → monotone: 2 intervals × 8 steps + 1 = 17 points → 16 segments
        assert_eq!(chart.len(), 16);
    }

    // ── Grid API tests (GUP-097) ────────────────────────────────────

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

        assert_eq!(builder.interpolation, LineInterpolation::Monotone);
        assert!(builder.stroke_accessor.is_some());
        assert!(builder.config.show_grid);
        assert!(builder.config.grid_config.show_horizontal);
        assert!(!builder.config.grid_config.show_vertical);

        // Test opacity separately to avoid chaining issues
        let opacity_builder = line::<TimePoint>().grid_opacity(0.3);
        assert_eq!(opacity_builder.config.grid_config.major_grid.opacity, 0.3);
    }

    #[test]
    fn test_line_chart_tick_format_fluent() {
        use crate::label::{DateTimeFormatter, PercentFormatter};

        let builder = line::<TimePoint>()
            .x_tick_format(DateTimeFormatter::date_only())
            .y_tick_format(PercentFormatter::with_precision(1));

        assert!(builder.config.x_label_formatter.is_some());
        assert!(builder.config.y_label_formatter.is_some());

        let y_fmt = builder.config.y_label_formatter.as_ref().unwrap();
        assert_eq!(y_fmt.format_value(0.333), "33.3%");
    }

    // ── Data-mark rendering tests (GUP-286) ─────────────────────────

    #[tokio::test]
    async fn test_line_chart_has_data_mark_data() {
        let context = Arc::new(RenderContext::new().await.unwrap());
        let data = vec![
            TimePoint {
                time: 0.0,
                value: 10.0,
                series: "A".to_string(),
            },
            TimePoint {
                time: 1.0,
                value: 20.0,
                series: "A".to_string(),
            },
        ];

        let chart = line()
            .x(AccessorFunction::new(|d: &TimePoint| {
                AccessorValue::Float(d.time)
            }))
            .y(AccessorFunction::new(|d: &TimePoint| {
                AccessorValue::Float(d.value)
            }))
            .build_with_data(data, context)
            .unwrap();

        assert!(
            chart.has_data_mark_data(),
            "Line chart should report data-mark data present"
        );
        assert!(
            chart.visualization.is_render_ready(),
            "Line chart selection should be render-ready after build"
        );
    }

    #[tokio::test]
    async fn test_line_chart_render_to_png_produces_visible_lines() {
        let context = Arc::new(RenderContext::new().await.unwrap());
        let data = vec![
            TimePoint {
                time: 0.0,
                value: 10.0,
                series: "A".to_string(),
            },
            TimePoint {
                time: 1.0,
                value: 50.0,
                series: "A".to_string(),
            },
            TimePoint {
                time: 2.0,
                value: 30.0,
                series: "A".to_string(),
            },
            TimePoint {
                time: 3.0,
                value: 70.0,
                series: "A".to_string(),
            },
        ];

        let mut chart = line()
            .x(AccessorFunction::new(|d: &TimePoint| {
                AccessorValue::Float(d.time)
            }))
            .y(AccessorFunction::new(|d: &TimePoint| {
                AccessorValue::Float(d.value)
            }))
            .stroke_width_px(4.0)
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
            "Expected visible line segments in the data region, but found only {non_white} non-white pixels"
        );
    }
}

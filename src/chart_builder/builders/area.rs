// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Area chart builder with fluent API for filled area visualisation.
//!
//! Provides [`AreaChartBuilder`] for creating GPU-accelerated area charts
//! including single-series, stacked, normalised-stacked (100%), and
//! band/ribbon variants.
//!
//! # Stacking
//!
//! When [`.stack()`](AreaChartBuilder::stack) or
//! [`.stack_normalized()`](AreaChartBuilder::stack_normalized) is enabled
//! the builder performs a CPU-side cumulative-sum pre-pass (via
//! [`compute_stack_offsets`]) before uploading geometry.
//!
//! # Band / Ribbon
//!
//! When [`.y0()`](AreaChartBuilder::y0) receives a per-record accessor
//! the area fills between `y` and `y0` per data point — useful for
//! confidence-interval ribbons.
//!
//! # Performance
//!
//! Point evaluation and sorting are `O(N log N)` when `sort_by_x` is
//! enabled (the default). The area polygon is rendered as line segments
//! forming the closed outline.

use super::{
    AccessorFunction, ConfigurableBuilder, GridCapableBuilder, validate_required_accessors,
};
use crate::RenderContext;
use crate::chart_builder::accessor::AccessorValue;
use crate::chart_builder::{
    AxisScale, ChartBuilder, ChartBuilderError, ChartConfig, ComposedChart,
};
use crate::error::GupResult;
use crate::grid::{GridConfiguration, GridLineConfig};
use crate::mark::line::Line;
use crate::selection::Selection;
use crate::shader_function::ColorScale;
use crate::{MaybeSend, MaybeSync};
use std::marker::PhantomData;
use std::sync::Arc;

// ── Categorical colour palette ──────────────────────────────────────────

/// Default categorical palette (same colours used by line and bar charts).
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

// ── AreaSegment wrapper ─────────────────────────────────────────────────

/// A single line segment in an area polygon outline.
///
/// `AreaSegment<T>` stores the original data point together with
/// pre-computed start/end positions, colour, and width. It is the data
/// item type held by the `Selection<AreaSegment<T>, Line>` returned from
/// [`AreaChartBuilder::build_with_data`].
#[derive(Debug, Clone)]
pub struct AreaSegment<T> {
    /// The original data point.
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

// ── Stack mode ──────────────────────────────────────────────────────────

/// Stacking mode for area charts.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum StackMode {
    /// No stacking — each series uses its own y0 baseline.
    #[default]
    None,
    /// Cumulative stacking: each series' y0 is the cumulative sum of
    /// all preceding series at the same x value.
    Stacked,
    /// Normalised (100%) stacking: each series is scaled so that the
    /// total at every x bin is exactly 1.0.
    Normalized,
}

// ── Baseline specification ──────────────────────────────────────────────

/// Specifies the lower boundary (y0) for the area polygon.
pub(crate) enum Baseline<T> {
    /// A constant y0 value (default: 0.0).
    Constant(f32),
    /// A per-record accessor function.
    Accessor(AccessorFunction<T>),
}

impl<T> std::fmt::Debug for Baseline<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Constant(v) => write!(f, "Constant({v})"),
            Self::Accessor(_) => write!(f, "Accessor(<fn>)"),
        }
    }
}

impl<T> Default for Baseline<T> {
    fn default() -> Self {
        Self::Constant(0.0)
    }
}

// ── AreaChartBuilder ────────────────────────────────────────────────────

/// Area chart builder providing fluent API for filled area visualisation.
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
/// let data = vec![
///     DataPoint { date: 0.0, value: 10.0, series: "A".to_string() },
///     DataPoint { date: 1.0, value: 15.0, series: "A".to_string() },
/// ];
///
/// let chart = area()
///     .x(AccessorFunction::new(|d: &DataPoint| AccessorValue::Float(d.date)))
///     .y(AccessorFunction::new(|d: &DataPoint| AccessorValue::Float(d.value)))
///     .opacity(0.8)
///     .build_with_data(data, context)?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct AreaChartBuilder<T> {
    pub(crate) x_accessor: Option<AccessorFunction<T>>,
    pub(crate) y_accessor: Option<AccessorFunction<T>>,
    pub(crate) y0_baseline: Baseline<T>,
    pub(crate) fill_accessor: Option<AccessorFunction<T>>,
    pub(crate) series_accessor: Option<AccessorFunction<T>>,
    pub(crate) fill_opacity: f32,
    pub(crate) stack_mode: StackMode,
    pub(crate) gradient_scale: Option<ColorScale>,
    pub(crate) sort_by_x: bool,
    pub(crate) config: ChartConfig,
    pub(crate) _phantom: PhantomData<T>,
}

impl<T> Clone for AreaChartBuilder<T> {
    fn clone(&self) -> Self {
        Self {
            x_accessor: self.x_accessor.clone(),
            y_accessor: self.y_accessor.clone(),
            y0_baseline: Baseline::default(),
            fill_accessor: self.fill_accessor.clone(),
            series_accessor: self.series_accessor.clone(),
            fill_opacity: self.fill_opacity,
            stack_mode: self.stack_mode,
            gradient_scale: self.gradient_scale.clone(),
            sort_by_x: self.sort_by_x,
            config: self.config.clone(),
            _phantom: PhantomData,
        }
    }
}

impl<T> AreaChartBuilder<T> {
    /// Create a new area chart builder.
    pub fn new() -> Self {
        Self {
            x_accessor: None,
            y_accessor: None,
            y0_baseline: Baseline::default(),
            fill_accessor: None,
            series_accessor: None,
            fill_opacity: 0.8,
            stack_mode: StackMode::None,
            gradient_scale: None,
            sort_by_x: true,
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

    /// Set the Y-axis (upper boundary) accessor function.
    pub fn y<A>(mut self, accessor: A) -> Self
    where
        A: Into<AccessorFunction<T>>,
    {
        self.y_accessor = Some(accessor.into());
        self
    }

    /// Set the baseline (y0) to a constant value.
    ///
    /// When omitted, defaults to `0.0`.
    pub fn y0_constant(mut self, value: f32) -> Self {
        self.y0_baseline = Baseline::Constant(value);
        self
    }

    /// Set the baseline (y0) to a per-record accessor (band/ribbon mode).
    ///
    /// When this is set, the area fills between `y` and `y0` per data
    /// point, enabling confidence-interval ribbons.
    pub fn y0<A>(mut self, accessor: A) -> Self
    where
        A: Into<AccessorFunction<T>>,
    {
        self.y0_baseline = Baseline::Accessor(accessor.into());
        self
    }

    /// Set the fill colour accessor (also acts as series key for stacking).
    pub fn fill<A>(mut self, accessor: A) -> Self
    where
        A: Into<AccessorFunction<T>>,
    {
        self.fill_accessor = Some(accessor.into());
        self
    }

    /// Set the colour accessor (alias for [`fill`](Self::fill)).
    ///
    /// When the accessor produces categorical values, it doubles as the
    /// series key for stacking.
    pub fn color<A>(mut self, accessor: A) -> Self
    where
        A: Into<AccessorFunction<T>>,
    {
        self.fill_accessor = Some(accessor.into());
        self
    }

    /// Set a dedicated series accessor for stacking.
    ///
    /// When set, this takes precedence over the colour accessor for
    /// determining series grouping.
    pub fn series<A>(mut self, accessor: A) -> Self
    where
        A: Into<AccessorFunction<T>>,
    {
        self.series_accessor = Some(accessor.into());
        self
    }

    /// Set the fill opacity (default: 0.8).
    ///
    /// The stroke (outline) remains fully opaque.
    pub fn opacity(mut self, opacity: f32) -> Self {
        self.fill_opacity = opacity.clamp(0.0, 1.0);
        self
    }

    /// Enable cumulative stacking mode.
    ///
    /// Requires a series key accessor set via `.color()` or `.series()`.
    pub fn stack(mut self) -> Self {
        self.stack_mode = StackMode::Stacked;
        self
    }

    /// Enable normalised (100%) stacking mode.
    ///
    /// Each series is scaled so that the total at every x bin is 1.0.
    pub fn stack_normalized(mut self) -> Self {
        self.stack_mode = StackMode::Normalized;
        self
    }

    /// Set a gradient colour scale for y-mapped fills.
    ///
    /// Gradient colours are interpolated per vertex by the GPU.
    pub fn gradient(mut self, color_scale: impl Into<ColorScale>) -> Self {
        self.gradient_scale = Some(color_scale.into());
        self
    }

    /// Enable or disable sorting data points by X coordinate.
    pub fn sort_x(mut self, sort: bool) -> Self {
        self.sort_by_x = sort;
        self
    }

    /// Set the X-axis scale.
    pub fn x_scale(mut self, scale: impl Into<AxisScale>) -> Self {
        self.config.x_scale = Some(scale.into());
        self
    }

    /// Set the Y-axis scale.
    pub fn y_scale(mut self, scale: impl Into<AxisScale>) -> Self {
        self.config.y_scale = Some(scale.into());
        self
    }

    /// Set the colour scale for the chart config.
    pub fn color_scale(mut self, scale: impl Into<ColorScale>) -> Self {
        self.config.color_scale = Some(scale.into());
        self
    }
}

impl<T> Default for AreaChartBuilder<T> {
    fn default() -> Self {
        Self::new()
    }
}

// ── ConfigurableBuilder ─────────────────────────────────────────────────

impl<T> ConfigurableBuilder for AreaChartBuilder<T> {
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

// ── GridCapableBuilder ──────────────────────────────────────────────────

impl<T> GridCapableBuilder for AreaChartBuilder<T> {
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

// ── Stacking helpers ────────────────────────────────────────────────────

/// A single point after stack offset computation.
#[derive(Debug, Clone)]
pub struct StackedPoint {
    /// X value.
    pub x: f32,
    /// Upper Y boundary (after stacking offset).
    pub y: f32,
    /// Lower Y boundary (the cumulative baseline).
    pub y0: f32,
    /// Series key.
    pub series: String,
    /// Index into the original data array.
    pub original_index: usize,
}

/// Compute stacking offsets for multi-series data.
///
/// Performs a cumulative-sum pre-pass: for each unique x value, the
/// series are stacked in the order they first appear. In `Normalized`
/// mode the values are further divided by the per-x total so that all
/// series together sum to 1.0.
///
/// # Arguments
///
/// * `points` — evaluated `(x, y, series_key, original_index)` tuples,
///   assumed to already be sorted by x within each series.
/// * `series_order` — ordered list of unique series keys.
/// * `mode` — the [`StackMode`] to apply.
///
/// # Returns
///
/// A `Vec<StackedPoint>` with `y` and `y0` adjusted for stacking.
pub fn compute_stack_offsets(
    points: &[(f32, f32, String, usize)],
    series_order: &[String],
    mode: StackMode,
) -> Vec<StackedPoint> {
    use std::collections::BTreeMap;

    if mode == StackMode::None || series_order.len() <= 1 {
        // No stacking needed — just pass through.
        return points
            .iter()
            .map(|(x, y, series, idx)| StackedPoint {
                x: *x,
                y: *y,
                y0: 0.0,
                series: series.clone(),
                original_index: *idx,
            })
            .collect();
    }

    // Group points by series, then by x.
    // series_key → BTreeMap<OrderedFloat(x), (y, original_index)>
    let mut series_data: std::collections::HashMap<String, BTreeMap<i64, (f32, usize)>> =
        std::collections::HashMap::new();

    for (x, y, series, idx) in points {
        let x_key = float_to_key(*x);
        series_data
            .entry(series.clone())
            .or_default()
            .insert(x_key, (*y, *idx));
    }

    // Collect all unique x keys in sorted order.
    let mut all_x_keys: Vec<i64> = series_data
        .values()
        .flat_map(|m| m.keys().copied())
        .collect();
    all_x_keys.sort_unstable();
    all_x_keys.dedup();

    // Compute per-x totals for normalisation.
    let x_totals: BTreeMap<i64, f32> = if mode == StackMode::Normalized {
        all_x_keys
            .iter()
            .map(|&x_key| {
                let total: f32 = series_order
                    .iter()
                    .filter_map(|s| series_data.get(s)?.get(&x_key).map(|(y, _)| *y))
                    .sum();
                (x_key, if total.abs() < 1e-12 { 1.0 } else { total })
            })
            .collect()
    } else {
        BTreeMap::new()
    };

    // Build stacked points series by series, bottom to top.
    let mut result = Vec::with_capacity(points.len());
    // cumulative[x_key] = running baseline for each x.
    let mut cumulative: BTreeMap<i64, f32> = BTreeMap::new();

    for series_key in series_order {
        if let Some(x_data) = series_data.get(series_key) {
            for (&x_key, &(raw_y, original_index)) in x_data {
                let baseline = cumulative.get(&x_key).copied().unwrap_or(0.0);
                let y_val = match mode {
                    StackMode::Normalized => {
                        let total = x_totals.get(&x_key).copied().unwrap_or(1.0);
                        raw_y / total
                    }
                    _ => raw_y,
                };
                let y0 = baseline;
                let y = baseline + y_val;

                *cumulative.entry(x_key).or_insert(0.0) = y;

                result.push(StackedPoint {
                    x: key_to_float(x_key),
                    y,
                    y0,
                    series: series_key.clone(),
                    original_index,
                });
            }
        }
    }

    result
}

/// Convert an f32 to a deterministic i64 key for use in BTreeMap.
/// This gives us total ordering on floats for grouping by x value.
fn float_to_key(f: f32) -> i64 {
    let bits = f.to_bits();
    // IEEE 754 floats: positive floats have bit patterns that sort
    // like unsigned integers, but negative floats are reversed.
    // Flip all bits for negative values; flip only sign bit for positive.

    if (bits >> 31) == 1 {
        !(bits as i64)
    } else {
        (bits as i64) | (1i64 << 31)
    }
}

/// Convert an i64 key back to f32.
fn key_to_float(key: i64) -> f32 {
    let bits = if (key >> 31) & 1 == 1 {
        // Was positive float: clear the sign-flip bit.
        (key & !(1i64 << 31)) as u32
    } else {
        // Was negative float: complement back.
        !(key) as u32
    };
    f32::from_bits(bits)
}

// ── Polygon closing helper ──────────────────────────────────────────────

/// Close an area polygon by connecting the upper path to the reversed
/// lower path, producing a list of `(start, end)` line segments.
///
/// # Arguments
///
/// * `upper` — the upper boundary points `[(x, y), ...]` in x-order.
/// * `lower` — the lower boundary points `[(x, y0), ...]` in x-order.
///
/// # Returns
///
/// A `Vec<([f32; 2], [f32; 2])>` of segment (start, end) pairs forming
/// the closed polygon outline.
pub fn close_area_polygon(upper: &[[f32; 2]], lower: &[[f32; 2]]) -> Vec<([f32; 2], [f32; 2])> {
    if upper.is_empty() || lower.is_empty() {
        return Vec::new();
    }

    // For a single point, we can't form a polygon.
    if upper.len() == 1 && lower.len() == 1 {
        return Vec::new();
    }

    let mut segments = Vec::new();

    // 1. Upper path: left to right.
    for pair in upper.windows(2) {
        segments.push((pair[0], pair[1]));
    }

    // 2. Connect last upper to last lower.
    let last_upper = *upper.last().unwrap();
    let last_lower = *lower.last().unwrap();
    segments.push((last_upper, last_lower));

    // 3. Lower path: right to left (reversed).
    for pair in lower.windows(2).rev() {
        segments.push((pair[1], pair[0]));
    }

    // 4. Connect first lower back to first upper to close.
    let first_upper = upper[0];
    let first_lower = lower[0];
    segments.push((first_lower, first_upper));

    segments
}

// ── ChartBuilder implementation ─────────────────────────────────────────

impl<T> ChartBuilder<T> for AreaChartBuilder<T>
where
    T: Clone + MaybeSend + MaybeSync + std::fmt::Debug + 'static,
{
    type Output = ComposedChart<AreaSegment<T>, Line>;

    fn build_with_data(self, data: Vec<T>, context: Arc<RenderContext>) -> GupResult<Self::Output> {
        validate_required_accessors(&self.x_accessor, &self.y_accessor)?;

        if data.is_empty() {
            return Err(ChartBuilderError::EmptyData.into());
        }

        let x_acc = self.x_accessor.as_ref().unwrap();
        let y_acc = self.y_accessor.as_ref().unwrap();
        let fill_opacity = self.fill_opacity;

        // Determine the series accessor: prefer explicit series, then fill/color.
        let series_acc = self
            .series_accessor
            .as_ref()
            .or(self.fill_accessor.as_ref());

        // ── Evaluate accessor values for each data point ────────────
        struct EvaluatedPoint<T> {
            data: T,
            x: f32,
            y: f32,
            y0: f32,
            series_key: Option<String>,
            color: Option<[f32; 4]>,
        }

        let mut points: Vec<EvaluatedPoint<T>> = data
            .into_iter()
            .map(|d| {
                let x_val = x_acc.apply(&d).as_f32();
                let y_val = y_acc.apply(&d).as_f32();
                let y0_val = match &self.y0_baseline {
                    Baseline::Constant(c) => *c,
                    Baseline::Accessor(acc) => acc.apply(&d).as_f32(),
                };
                let (series_key, color) = if let Some(acc) = series_acc {
                    match acc.apply(&d) {
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
                    y0: y0_val,
                    series_key,
                    color,
                }
            })
            .collect();

        // ── Sort by x when requested ────────────────────────────────
        if self.sort_by_x {
            points.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal));
        }

        // ── Group by series ─────────────────────────────────────────
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
                if let Some(c) = points[indices[0]].color {
                    return c;
                }
                DEFAULT_PALETTE[series_idx % DEFAULT_PALETTE.len()]
            })
            .collect();

        // ── Apply stacking if needed ────────────────────────────────
        let use_stacking = self.stack_mode != StackMode::None && groups.len() > 1;

        // If stacking, compute offsets across all series.
        let stacked_points: Option<Vec<StackedPoint>> = if use_stacking {
            let flat_points: Vec<(f32, f32, String, usize)> = points
                .iter()
                .enumerate()
                .map(|(i, pt)| (pt.x, pt.y, pt.series_key.clone().unwrap_or_default(), i))
                .collect();

            let series_order: Vec<String> = groups.iter().map(|(label, _)| label.clone()).collect();
            Some(compute_stack_offsets(
                &flat_points,
                &series_order,
                self.stack_mode,
            ))
        } else {
            None
        };

        // ── Build area segments ─────────────────────────────────────
        let mut segments: Vec<AreaSegment<T>> = Vec::new();

        for (series_idx, (series_label, indices)) in groups.iter().enumerate() {
            if indices.is_empty() {
                continue;
            }

            let color = {
                let mut c = series_colors[series_idx];
                c[3] = fill_opacity;
                c
            };

            // Gather upper and lower boundaries for this series.
            let (upper, lower): (Vec<[f32; 2]>, Vec<[f32; 2]>) =
                if let Some(ref stacked) = stacked_points {
                    // Find stacked points for this series.
                    let series_stacked: Vec<&StackedPoint> = stacked
                        .iter()
                        .filter(|sp| &sp.series == series_label)
                        .collect();

                    let u: Vec<[f32; 2]> = series_stacked.iter().map(|sp| [sp.x, sp.y]).collect();
                    let l: Vec<[f32; 2]> = series_stacked.iter().map(|sp| [sp.x, sp.y0]).collect();
                    (u, l)
                } else {
                    // Non-stacked: use individual y and y0 values.
                    let u: Vec<[f32; 2]> = indices
                        .iter()
                        .map(|&i| [points[i].x, points[i].y])
                        .collect();
                    let l: Vec<[f32; 2]> = indices
                        .iter()
                        .map(|&i| [points[i].x, points[i].y0])
                        .collect();
                    (u, l)
                };

            if upper.len() < 2 {
                continue;
            }

            // Close the polygon and generate segments.
            let polygon_segments = close_area_polygon(&upper, &lower);

            // Use the first data point in the series as the representative.
            let data_idx = indices[0];

            for (start, end) in polygon_segments {
                segments.push(AreaSegment {
                    data: points[data_idx].data.clone(),
                    start_pos: start,
                    end_pos: end,
                    color,
                    width: 1.5,
                });
            }
        }

        if segments.is_empty() {
            return Err(ChartBuilderError::EmptyData.into());
        }

        // ── Create Selection<AreaSegment<T>, Line> ──────────────────
        let mut selection = Selection::<AreaSegment<T>, Line>::new(segments, context)?;

        selection.attr("start", |seg: &AreaSegment<T>| seg.start_pos);
        selection.attr("end", |seg: &AreaSegment<T>| seg.end_pos);
        selection.attr("color", |seg: &AreaSegment<T>| seg.color);
        selection.attr("width", |seg: &AreaSegment<T>| seg.width);

        // ── Wrap in ComposedChart with axes ─────────────────────────
        let composed_chart = ComposedChart::new(selection, self.config).with_default_axes();

        Ok(composed_chart)
    }
}

/// Convenience function to create a new area chart builder.
pub fn area<T>() -> AreaChartBuilder<T> {
    AreaChartBuilder::new()
}

// ── Tests ───────────────────────────────────────────────────────────────

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

    #[derive(Debug, Clone)]
    struct BandPoint {
        time: f32,
        upper: f32,
        lower: f32,
    }

    // ── compute_stack_offsets tests ──────────────────────────────────

    #[test]
    fn test_stack_offsets_single_series() {
        let points = vec![
            (0.0, 10.0, "A".to_string(), 0),
            (1.0, 20.0, "A".to_string(), 1),
            (2.0, 30.0, "A".to_string(), 2),
        ];
        let series_order = vec!["A".to_string()];
        let result = compute_stack_offsets(&points, &series_order, StackMode::Stacked);

        // Single series — y0 should all be 0.0, y unchanged.
        assert_eq!(result.len(), 3);
        for sp in &result {
            assert_eq!(sp.y0, 0.0);
        }
        assert_eq!(result[0].y, 10.0);
        assert_eq!(result[1].y, 20.0);
        assert_eq!(result[2].y, 30.0);
    }

    #[test]
    fn test_stack_offsets_two_series() {
        let points = vec![
            (0.0, 10.0, "A".to_string(), 0),
            (1.0, 20.0, "A".to_string(), 1),
            (0.0, 5.0, "B".to_string(), 2),
            (1.0, 15.0, "B".to_string(), 3),
        ];
        let series_order = vec!["A".to_string(), "B".to_string()];
        let result = compute_stack_offsets(&points, &series_order, StackMode::Stacked);

        assert_eq!(result.len(), 4);

        // Series A: y0=0, y=original.
        let a_points: Vec<&StackedPoint> = result.iter().filter(|sp| sp.series == "A").collect();
        assert_eq!(a_points.len(), 2);
        assert_eq!(a_points[0].y0, 0.0);
        assert_eq!(a_points[0].y, 10.0);
        assert_eq!(a_points[1].y0, 0.0);
        assert_eq!(a_points[1].y, 20.0);

        // Series B: y0 = cumulative of A, y = y0 + B's value.
        let b_points: Vec<&StackedPoint> = result.iter().filter(|sp| sp.series == "B").collect();
        assert_eq!(b_points.len(), 2);
        assert_eq!(b_points[0].y0, 10.0); // A contributed 10 at x=0
        assert_eq!(b_points[0].y, 15.0); // 10 + 5
        assert_eq!(b_points[1].y0, 20.0); // A contributed 20 at x=1
        assert_eq!(b_points[1].y, 35.0); // 20 + 15
    }

    #[test]
    fn test_stack_offsets_normalized() {
        let points = vec![
            (0.0, 30.0, "A".to_string(), 0),
            (1.0, 40.0, "A".to_string(), 1),
            (0.0, 70.0, "B".to_string(), 2),
            (1.0, 60.0, "B".to_string(), 3),
        ];
        let series_order = vec!["A".to_string(), "B".to_string()];
        let result = compute_stack_offsets(&points, &series_order, StackMode::Normalized);

        // At x=0: total=100, A=30/100=0.3, B=70/100=0.7
        let b_at_0: Vec<&StackedPoint> = result
            .iter()
            .filter(|sp| sp.series == "B" && (sp.x - 0.0).abs() < 1e-5)
            .collect();
        assert!((b_at_0[0].y - 1.0).abs() < 1e-5, "Total should be ~1.0");

        let a_at_0: Vec<&StackedPoint> = result
            .iter()
            .filter(|sp| sp.series == "A" && (sp.x - 0.0).abs() < 1e-5)
            .collect();
        assert!((a_at_0[0].y - 0.3).abs() < 1e-5, "A should be ~0.3 at x=0");
    }

    #[test]
    fn test_stack_offsets_zero_values() {
        let points = vec![
            (0.0, 0.0, "A".to_string(), 0),
            (0.0, 0.0, "B".to_string(), 1),
        ];
        let series_order = vec!["A".to_string(), "B".to_string()];
        let result = compute_stack_offsets(&points, &series_order, StackMode::Stacked);

        // Zero values should produce zero stacking.
        for sp in &result {
            assert_eq!(sp.y0, 0.0);
            assert_eq!(sp.y, 0.0);
        }
    }

    #[test]
    fn test_stack_offsets_normalized_zero_total() {
        let points = vec![
            (0.0, 0.0, "A".to_string(), 0),
            (0.0, 0.0, "B".to_string(), 1),
        ];
        let series_order = vec!["A".to_string(), "B".to_string()];
        let result = compute_stack_offsets(&points, &series_order, StackMode::Normalized);

        // Zero total should not produce NaN/Inf.
        for sp in &result {
            assert!(sp.y.is_finite());
            assert!(sp.y0.is_finite());
        }
    }

    #[test]
    fn test_stack_offsets_none_mode() {
        let points = vec![
            (0.0, 10.0, "A".to_string(), 0),
            (0.0, 5.0, "B".to_string(), 1),
        ];
        let series_order = vec!["A".to_string(), "B".to_string()];
        let result = compute_stack_offsets(&points, &series_order, StackMode::None);

        // No stacking — y0 should be 0.0 for all.
        for sp in &result {
            assert_eq!(sp.y0, 0.0);
        }
        assert_eq!(result[0].y, 10.0);
        assert_eq!(result[1].y, 5.0);
    }

    // ── close_area_polygon tests ────────────────────────────────────

    #[test]
    fn test_close_polygon_basic() {
        let upper = vec![[0.0, 1.0], [1.0, 2.0], [2.0, 1.5]];
        let lower = vec![[0.0, 0.0], [1.0, 0.0], [2.0, 0.0]];
        let segments = close_area_polygon(&upper, &lower);

        // Upper: 2 segments + connect-right + lower reversed: 2 segments + connect-left
        // = 2 + 1 + 2 + 1 = 6 segments
        assert_eq!(segments.len(), 6);

        // First segment is upper[0] → upper[1]
        assert_eq!(segments[0], ([0.0, 1.0], [1.0, 2.0]));
        // Last segment is lower[0] → upper[0] (closing)
        assert_eq!(segments[5], ([0.0, 0.0], [0.0, 1.0]));
    }

    #[test]
    fn test_close_polygon_single_point() {
        let upper = vec![[5.0, 10.0]];
        let lower = vec![[5.0, 0.0]];
        let segments = close_area_polygon(&upper, &lower);

        // Can't form a polygon from a single point.
        assert!(segments.is_empty());
    }

    #[test]
    fn test_close_polygon_two_points() {
        let upper = vec![[0.0, 5.0], [1.0, 10.0]];
        let lower = vec![[0.0, 0.0], [1.0, 0.0]];
        let segments = close_area_polygon(&upper, &lower);

        // Upper: 1 seg + connect-right: 1 seg + lower reversed: 1 seg + connect-left: 1 seg = 4
        assert_eq!(segments.len(), 4);
    }

    #[test]
    fn test_close_polygon_variable_baseline() {
        // Band/ribbon scenario where y0 varies per point.
        let upper = vec![[0.0, 10.0], [1.0, 12.0], [2.0, 8.0]];
        let lower = vec![[0.0, 5.0], [1.0, 3.0], [2.0, 6.0]];
        let segments = close_area_polygon(&upper, &lower);

        assert_eq!(segments.len(), 6);
        // Connect right: upper last → lower last
        assert_eq!(segments[2], ([2.0, 8.0], [2.0, 6.0]));
        // Lower reversed: lower[2] → lower[1], lower[1] → lower[0]
        assert_eq!(segments[3], ([2.0, 6.0], [1.0, 3.0]));
        assert_eq!(segments[4], ([1.0, 3.0], [0.0, 5.0]));
    }

    #[test]
    fn test_close_polygon_empty_input() {
        let empty: Vec<[f32; 2]> = vec![];
        let lower = vec![[0.0, 0.0]];
        assert!(close_area_polygon(&empty, &lower).is_empty());
        assert!(close_area_polygon(&lower, &empty).is_empty());
    }

    #[test]
    fn test_close_polygon_crossing_boundaries() {
        // y0 crosses y for some points.
        let upper = vec![[0.0, 5.0], [1.0, 3.0], [2.0, 7.0]];
        let lower = vec![[0.0, 2.0], [1.0, 6.0], [2.0, 4.0]];
        let segments = close_area_polygon(&upper, &lower);

        // Should still produce a valid polygon regardless of crossing.
        assert_eq!(segments.len(), 6);
    }

    // ── float_to_key round-trip tests ───────────────────────────────

    #[test]
    fn test_float_key_roundtrip() {
        let values = [0.0, 1.0, -1.0, 0.5, -0.5, 100.0, -100.0, f32::MIN_POSITIVE];
        for &v in &values {
            let key = float_to_key(v);
            let back = key_to_float(key);
            assert!(
                (v - back).abs() < 1e-10,
                "Round-trip failed for {v}: got {back}"
            );
        }
    }

    #[test]
    fn test_float_key_ordering() {
        let values = [-2.0, -1.0, 0.0, 0.5, 1.0, 2.0];
        let keys: Vec<i64> = values.iter().map(|&v| float_to_key(v)).collect();
        for pair in keys.windows(2) {
            assert!(
                pair[0] < pair[1],
                "Keys not ordered: {} >= {} for values",
                pair[0],
                pair[1]
            );
        }
    }

    // ── Builder API tests ───────────────────────────────────────────

    #[test]
    fn test_area_builder_defaults() {
        let builder = AreaChartBuilder::<TimePoint>::new();
        assert!(builder.x_accessor.is_none());
        assert!(builder.y_accessor.is_none());
        assert_eq!(builder.fill_opacity, 0.8);
        assert_eq!(builder.stack_mode, StackMode::None);
        assert!(builder.sort_by_x);
    }

    #[test]
    fn test_area_builder_fluent_api() {
        let builder = area::<TimePoint>()
            .opacity(0.6)
            .stack()
            .sort_x(false)
            .title("My Area Chart")
            .width(1024.0)
            .height(768.0);

        assert_eq!(builder.fill_opacity, 0.6);
        assert_eq!(builder.stack_mode, StackMode::Stacked);
        assert!(!builder.sort_by_x);
        assert!(builder.config.title_config.is_some());
    }

    #[test]
    fn test_area_builder_stack_normalized() {
        let builder = area::<TimePoint>().stack_normalized();
        assert_eq!(builder.stack_mode, StackMode::Normalized);
    }

    #[test]
    fn test_area_builder_opacity_clamping() {
        let builder = area::<TimePoint>().opacity(1.5);
        assert_eq!(builder.fill_opacity, 1.0);

        let builder = area::<TimePoint>().opacity(-0.5);
        assert_eq!(builder.fill_opacity, 0.0);
    }

    // ── Integration build tests (require GPU) ───────────────────────

    #[tokio::test]
    async fn test_area_chart_basic_build() {
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

        let builder = area()
            .x(AccessorFunction::new(|d: &TimePoint| {
                AccessorValue::Float(d.time)
            }))
            .y(AccessorFunction::new(|d: &TimePoint| {
                AccessorValue::Float(d.value)
            }))
            .title("Basic Area Chart");

        let result = builder.build_with_data(data, context);
        assert!(result.is_ok());

        let chart = result.unwrap();
        // 3 points → upper: 2 segs + connect right: 1 + lower reversed: 2 + close: 1 = 6
        assert_eq!(chart.len(), 6);
    }

    #[tokio::test]
    async fn test_area_chart_y0_default_zero() {
        let data = vec![
            TimePoint {
                time: 0.0,
                value: 5.0,
                series: "A".to_string(),
            },
            TimePoint {
                time: 1.0,
                value: 10.0,
                series: "A".to_string(),
            },
        ];

        let context = Arc::new(RenderContext::new().await.unwrap());

        let chart = area()
            .x(AccessorFunction::new(|d: &TimePoint| {
                AccessorValue::Float(d.time)
            }))
            .y(AccessorFunction::new(|d: &TimePoint| {
                AccessorValue::Float(d.value)
            }))
            .build_with_data(data, context)
            .unwrap();

        let segments = chart.visualization.data();
        // Polygon: upper(0,5)→(1,10), connect(1,10)→(1,0), lower(1,0)→(0,0), close(0,0)→(0,5)
        assert_eq!(segments.len(), 4);
        // Verify lower boundary uses y0=0
        assert_eq!(segments[2].start_pos, [1.0, 0.0]);
        assert_eq!(segments[2].end_pos, [0.0, 0.0]);
    }

    #[tokio::test]
    async fn test_area_chart_band_mode() {
        let data = vec![
            BandPoint {
                time: 0.0,
                upper: 10.0,
                lower: 5.0,
            },
            BandPoint {
                time: 1.0,
                upper: 12.0,
                lower: 3.0,
            },
            BandPoint {
                time: 2.0,
                upper: 8.0,
                lower: 6.0,
            },
        ];

        let context = Arc::new(RenderContext::new().await.unwrap());

        let chart = area()
            .x(AccessorFunction::new(|d: &BandPoint| {
                AccessorValue::Float(d.time)
            }))
            .y(AccessorFunction::new(|d: &BandPoint| {
                AccessorValue::Float(d.upper)
            }))
            .y0(AccessorFunction::new(|d: &BandPoint| {
                AccessorValue::Float(d.lower)
            }))
            .build_with_data(data, context)
            .unwrap();

        let segments = chart.visualization.data();
        assert_eq!(segments.len(), 6); // 3 points → 6 polygon segments
    }

    #[tokio::test]
    async fn test_area_chart_stacked() {
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
                time: 0.0,
                value: 5.0,
                series: "B".to_string(),
            },
            TimePoint {
                time: 1.0,
                value: 15.0,
                series: "B".to_string(),
            },
        ];

        let context = Arc::new(RenderContext::new().await.unwrap());

        let chart = area()
            .x(AccessorFunction::new(|d: &TimePoint| {
                AccessorValue::Float(d.time)
            }))
            .y(AccessorFunction::new(|d: &TimePoint| {
                AccessorValue::Float(d.value)
            }))
            .color(AccessorFunction::new(|d: &TimePoint| {
                AccessorValue::String(d.series.clone())
            }))
            .stack()
            .build_with_data(data, context)
            .unwrap();

        // 2 series × 2 points each → 2 series × 4 polygon segments = 8
        assert_eq!(chart.len(), 8);

        // Verify distinct colours for the two series.
        let segments = chart.visualization.data();
        let color_a = segments[0].color;
        let color_b = segments[4].color;
        // At minimum, RGB should differ (alpha may both be 0.8)
        assert_ne!(
            [color_a[0], color_a[1], color_a[2]],
            [color_b[0], color_b[1], color_b[2]],
            "Stacked series should have distinct colours"
        );
    }

    #[tokio::test]
    async fn test_area_chart_validation_errors() {
        let data = vec![TimePoint {
            time: 1.0,
            value: 2.0,
            series: "A".to_string(),
        }];

        let context = Arc::new(RenderContext::new().await.unwrap());

        // Missing Y accessor should fail.
        let builder = area().x(x("time"));
        let result = builder.build_with_data(data.clone(), context.clone());
        assert!(result.is_err());

        // Empty data should fail.
        let empty_data: Vec<TimePoint> = vec![];
        let builder = area().x(x("time")).y(y("value"));
        let result = builder.build_with_data(empty_data, context);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_area_chart_field_accessors() {
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

        let builder = area().x(x("time")).y(y("value"));
        let result = builder.build_with_data(data, context);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_area_chart_fill_opacity() {
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

        let context = Arc::new(RenderContext::new().await.unwrap());

        let chart = area()
            .x(AccessorFunction::new(|d: &TimePoint| {
                AccessorValue::Float(d.time)
            }))
            .y(AccessorFunction::new(|d: &TimePoint| {
                AccessorValue::Float(d.value)
            }))
            .opacity(0.5)
            .build_with_data(data, context)
            .unwrap();

        // All segments should have alpha = 0.5
        for seg in chart.visualization.data() {
            assert!(
                (seg.color[3] - 0.5).abs() < 1e-5,
                "Expected alpha 0.5, got {}",
                seg.color[3]
            );
        }
    }

    // ── Grid API tests ──────────────────────────────────────────────

    #[test]
    fn test_area_chart_grid_api() {
        let builder = area::<TimePoint>().grid();
        assert!(builder.config.show_grid);

        let builder = area::<TimePoint>().scientific_grid();
        assert!(builder.config.show_grid);
        assert!(builder.config.grid_config.minor_grid.enabled);

        let builder = area::<TimePoint>().horizontal_grid();
        assert!(builder.config.show_grid);
        assert!(builder.config.grid_config.show_horizontal);
        assert!(!builder.config.grid_config.show_vertical);
    }
}

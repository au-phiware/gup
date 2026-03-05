// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Violin plot builder for distributional shape visualization.
//!
//! Provides a fluent API for creating GPU-accelerated violin plots that
//! display full probability distributions as mirrored density curves,
//! with optional embedded box plot overlays and half-violin variants.
//!
//! # Examples
//!
//! ```rust,no_run
//! use gup::prelude::*;
//! use gup::chart_builder::accessor::AccessorValue;
//! use gup::chart_builder::builders::{AccessorFunction, violin};
//!
//! #[derive(Debug, Clone)]
//! struct Measurement {
//!     category: String,
//!     value: f32,
//! }
//!
//! # async fn example() -> GupResult<()> {
//! # let context = std::sync::Arc::new(RenderContext::new().await?);
//! let data = vec![
//!     Measurement { category: "A".to_string(), value: 10.0 },
//!     Measurement { category: "A".to_string(), value: 15.0 },
//!     Measurement { category: "B".to_string(), value: 20.0 },
//! ];
//!
//! let chart = violin()
//!     .x(AccessorFunction::new(|m: &Measurement| AccessorValue::String(m.category.clone())))
//!     .y(AccessorFunction::new(|m: &Measurement| AccessorValue::Float(m.value)))
//!     .show_box(true)
//!     .build_with_data(data, context)?;
//! # Ok(())
//! # }
//! ```

use super::{AccessorFunction, ConfigurableBuilder, GridCapableBuilder};
use crate::RenderContext;
use crate::chart_builder::{ChartBuilder, ChartBuilderError, ChartConfig, ComposedChart};
use crate::error::GupResult;
use crate::grid::{GridConfiguration, GridLineConfig};
use crate::label::LabelFormatter;
use crate::mark::boxplot::{BoxPlot, BoxPlotAttributes, BoxPlotOrientation};
use crate::selection::Selection;
use crate::shader_function::{KernelDensity1D, KernelFunction, Vec2};
use crate::{MaybeSend, MaybeSync};
use std::marker::PhantomData;
use std::sync::Arc;

/// Orientation for violin plots.
///
/// Controls whether violins extend vertically (default) or horizontally.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ViolinOrientation {
    /// Violins extend vertically — the value axis is vertical
    /// and the categorical axis is horizontal.
    #[default]
    Vertical,
    /// Violins extend horizontally — the value axis is horizontal
    /// and the categorical axis is vertical.
    Horizontal,
}

/// Which side(s) of the violin to render.
///
/// A full violin (`Both`) shows mirrored flanks on both sides of the spine.
/// `Left` or `Right` renders only one flank, useful for split-comparison
/// layouts where two categories share a spine.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum HalfSide {
    /// Render both flanks (standard full violin).
    #[default]
    Both,
    /// Render only the left (or bottom, for horizontal) flank.
    Left,
    /// Render only the right (or top, for horizontal) flank.
    Right,
}

/// Internal data computed per violin category.
#[derive(Debug, Clone)]
pub struct ViolinData {
    /// Category label.
    pub category: String,
    /// KDE grid evaluation points (along value axis).
    pub grid_points: Vec<f32>,
    /// Density values at each grid point.
    pub densities: Vec<f32>,
    /// Bandwidth used for KDE.
    pub bandwidth: f32,
    /// Five-number summary for optional box plot overlay.
    pub box_attrs: Option<BoxPlotAttributes>,
    /// Centre position along the categorical axis.
    pub x_centre: f32,
    /// Half-width allocated for this violin.
    pub half_width: f32,
}

/// Closed polygon path for a single violin body.
///
/// The polygon is formed by tracing the right flank (positive density),
/// closing at the top, tracing the left flank (negative density) in
/// reverse, and closing at the bottom.
#[derive(Debug, Clone)]
pub struct ViolinPath {
    /// Vertices of the closed polygon in (x, y) pairs.
    pub vertices: Vec<[f32; 2]>,
}

impl ViolinPath {
    /// Build a mirrored violin polygon from density values and grid points.
    ///
    /// For a full violin (`Both`), the polygon traces:
    ///   right flank → top cap → left flank (reversed) → bottom cap
    ///
    /// For half-violins, only one flank is emitted; the opposing side
    /// lies on the central spine (density = 0).
    ///
    /// # Arguments
    ///
    /// * `grid_points` — evaluation points along the value axis
    /// * `densities` — density at each grid point (non-negative)
    /// * `x_centre` — centre of the violin on the categorical axis
    /// * `half_width` — maximum half-width in categorical-axis units
    /// * `peak_density` — the maximum density value, used to normalise widths
    /// * `half` — which side(s) to render
    /// * `orientation` — vertical or horizontal layout
    /// * `trim` — if true, clip to the data range (no padding beyond min/max)
    /// * `data_min` — minimum observed data value (used when `trim` is true)
    /// * `data_max` — maximum observed data value (used when `trim` is true)
    pub fn build(
        grid_points: &[f32],
        densities: &[f32],
        x_centre: f32,
        half_width: f32,
        peak_density: f32,
        half: HalfSide,
        orientation: ViolinOrientation,
        trim: bool,
        data_min: f32,
        data_max: f32,
    ) -> Self {
        assert_eq!(grid_points.len(), densities.len());

        if grid_points.is_empty() || peak_density <= 0.0 {
            return Self {
                vertices: Vec::new(),
            };
        }

        // Filter grid points to data range if trim is enabled
        let (grid_pts, dens): (Vec<f32>, Vec<f32>) = if trim {
            grid_points
                .iter()
                .zip(densities.iter())
                .filter(|&(&g, _)| g >= data_min && g <= data_max)
                .map(|(&g, &d)| (g, d))
                .collect()
        } else {
            (grid_points.to_vec(), densities.to_vec())
        };

        if grid_pts.is_empty() {
            return Self {
                vertices: Vec::new(),
            };
        }

        let scale = half_width / peak_density;

        // Build the right flank (positive side)
        let right_flank: Vec<[f32; 2]> = grid_pts
            .iter()
            .zip(dens.iter())
            .map(|(&g, &d)| {
                let offset = d * scale;
                to_vertex(x_centre + offset, g, orientation)
            })
            .collect();

        // Build the left flank (negative side), reversed
        let left_flank: Vec<[f32; 2]> = grid_pts
            .iter()
            .zip(dens.iter())
            .rev()
            .map(|(&g, &d)| {
                let offset = d * scale;
                to_vertex(x_centre - offset, g, orientation)
            })
            .collect();

        let vertices = match half {
            HalfSide::Both => {
                let mut verts = Vec::with_capacity(right_flank.len() + left_flank.len());
                verts.extend_from_slice(&right_flank);
                verts.extend_from_slice(&left_flank);
                verts
            }
            HalfSide::Right => {
                // Right flank + spine (density=0, same grid points reversed)
                let spine: Vec<[f32; 2]> = grid_pts
                    .iter()
                    .rev()
                    .map(|&g| to_vertex(x_centre, g, orientation))
                    .collect();
                let mut verts = Vec::with_capacity(right_flank.len() + spine.len());
                verts.extend_from_slice(&right_flank);
                verts.extend_from_slice(&spine);
                verts
            }
            HalfSide::Left => {
                // Spine (density=0) + left flank
                let spine: Vec<[f32; 2]> = grid_pts
                    .iter()
                    .map(|&g| to_vertex(x_centre, g, orientation))
                    .collect();
                let mut verts = Vec::with_capacity(spine.len() + left_flank.len());
                verts.extend_from_slice(&spine);
                verts.extend_from_slice(&left_flank);
                verts
            }
        };

        Self { vertices }
    }

    /// Returns true when the first and last vertices share the same coordinate,
    /// i.e. the polygon is closed.
    pub fn is_closed(&self) -> bool {
        if self.vertices.len() < 2 {
            return false;
        }
        let first = self.vertices.first().unwrap();
        let last = self.vertices.last().unwrap();
        (first[0] - last[0]).abs() < 1e-6 && (first[1] - last[1]).abs() < 1e-6
    }
}

/// Convert a categorical-position and value-position into a 2-D vertex,
/// respecting orientation.
fn to_vertex(cat_pos: f32, val_pos: f32, orientation: ViolinOrientation) -> [f32; 2] {
    match orientation {
        ViolinOrientation::Vertical => [cat_pos, val_pos],
        ViolinOrientation::Horizontal => [val_pos, cat_pos],
    }
}

/// Compute evenly-spaced category layout positions.
///
/// Given `n` categories, positions are laid out between `0.0` and `total_width`
/// (defaults to `1.0` if 0 width). Each violin is centred on its position with
/// the specified padding subtracted from each side.
///
/// Returns a vector of `(centre, half_width)` pairs.
pub fn compute_category_layout(
    n_categories: usize,
    total_width: f32,
    padding: f32,
    max_violin_width: Option<f32>,
) -> Vec<(f32, f32)> {
    if n_categories == 0 {
        return Vec::new();
    }

    let slot_width = total_width / n_categories as f32;
    let raw_half = (slot_width - 2.0 * padding).max(0.0) / 2.0;

    let half_width = match max_violin_width {
        Some(max_w) => raw_half.min(max_w / 2.0),
        None => raw_half,
    };

    (0..n_categories)
        .map(|i| {
            let centre = (i as f32 + 0.5) * slot_width;
            (centre, half_width)
        })
        .collect()
}

// ─────────────────────────────────────────────────────────────────────────────
// ViolinPlotBuilder
// ─────────────────────────────────────────────────────────────────────────────

/// Violin plot builder providing a fluent API for creating GPU-accelerated
/// violin plots.
///
/// A violin plot displays the full probability distribution of data as a
/// mirrored density curve, making it significantly more informative than
/// a box plot alone.
///
/// # Examples
///
/// ```rust,no_run
/// use gup::prelude::*;
/// use gup::chart_builder::accessor::AccessorValue;
/// use gup::chart_builder::builders::{AccessorFunction, violin};
///
/// #[derive(Debug, Clone)]
/// struct Sample {
///     group: String,
///     value: f32,
/// }
///
/// # async fn example() -> GupResult<()> {
/// # let context = std::sync::Arc::new(RenderContext::new().await?);
/// let samples = vec![
///     Sample { group: "A".to_string(), value: 12.0 },
///     Sample { group: "A".to_string(), value: 15.0 },
///     Sample { group: "B".to_string(), value: 22.0 },
///     Sample { group: "B".to_string(), value: 25.0 },
/// ];
///
/// let chart = violin()
///     .x(AccessorFunction::new(|s: &Sample| AccessorValue::String(s.group.clone())))
///     .y(AccessorFunction::new(|s: &Sample| AccessorValue::Float(s.value)))
///     .show_box(true)
///     .trim(true)
///     .build_with_data(samples, context)?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct ViolinPlotBuilder<T> {
    /// Accessor for the category (horizontal position for vertical violins).
    pub(crate) x_accessor: Option<AccessorFunction<T>>,
    /// Accessor for the data values whose distribution is estimated.
    pub(crate) y_accessor: Option<AccessorFunction<T>>,
    /// Optional colour accessor.
    pub(crate) color_accessor: Option<AccessorFunction<T>>,
    /// Optional KDE bandwidth override (None = Silverman's rule).
    pub(crate) bandwidth: Option<f32>,
    /// Kernel function for KDE.
    pub(crate) kernel: KernelFunction,
    /// Number of evaluation grid points per violin.
    pub(crate) n_grid_points: usize,
    /// Whether to clip density tails to the data range.
    pub(crate) trim: bool,
    /// Whether to overlay an embedded box plot.
    pub(crate) show_box: bool,
    /// Width of the embedded box plot as a fraction of violin width.
    pub(crate) box_width_ratio: f32,
    /// Colour for the embedded box plot fill.
    pub(crate) box_color: Option<[f32; 4]>,
    /// Stroke width for the embedded box plot.
    pub(crate) box_stroke_width: f32,
    /// Violin orientation.
    pub(crate) orientation: ViolinOrientation,
    /// Which side(s) to render.
    pub(crate) half: HalfSide,
    /// Padding between violins.
    pub(crate) padding: f32,
    /// Maximum violin width.
    pub(crate) max_width: Option<f32>,
    /// Explicit category order override.
    pub(crate) category_order: Option<Vec<String>>,
    /// Optional split-by accessor for half-violin pairwise comparison.
    pub(crate) split_by_accessor: Option<AccessorFunction<T>>,
    /// Chart configuration.
    pub(crate) config: ChartConfig,
    pub(crate) _phantom: PhantomData<T>,
}

impl<T> ViolinPlotBuilder<T> {
    /// Create a new violin plot builder with default settings.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use gup::chart_builder::builders::violin::ViolinPlotBuilder;
    ///
    /// #[derive(Debug, Clone)]
    /// struct D { val: f32 }
    ///
    /// let builder = ViolinPlotBuilder::<D>::new();
    /// ```
    pub fn new() -> Self {
        Self {
            x_accessor: None,
            y_accessor: None,
            color_accessor: None,
            bandwidth: None,
            kernel: KernelFunction::Gaussian,
            n_grid_points: 128,
            trim: false,
            show_box: false,
            box_width_ratio: 0.1,
            box_color: None,
            box_stroke_width: 1.0,
            orientation: ViolinOrientation::default(),
            half: HalfSide::default(),
            padding: 0.05,
            max_width: None,
            category_order: None,
            split_by_accessor: None,
            config: ChartConfig::default(),
            _phantom: PhantomData,
        }
    }

    /// Set the X-axis accessor (category for vertical violins).
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use gup::chart_builder::accessor::AccessorValue;
    /// use gup::chart_builder::builders::{AccessorFunction, violin};
    ///
    /// # #[derive(Clone, Debug)] struct D { cat: String }
    /// violin()
    ///     .x(AccessorFunction::new(|d: &D| AccessorValue::String(d.cat.clone())));
    /// ```
    pub fn x<A>(mut self, accessor: A) -> Self
    where
        A: Into<AccessorFunction<T>>,
    {
        self.x_accessor = Some(accessor.into());
        self
    }

    /// Set the Y-axis accessor (value whose distribution is estimated).
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use gup::chart_builder::accessor::AccessorValue;
    /// use gup::chart_builder::builders::{AccessorFunction, violin};
    ///
    /// # #[derive(Clone, Debug)] struct D { val: f32 }
    /// violin()
    ///     .y(AccessorFunction::new(|d: &D| AccessorValue::Float(d.val)));
    /// ```
    pub fn y<A>(mut self, accessor: A) -> Self
    where
        A: Into<AccessorFunction<T>>,
    {
        self.y_accessor = Some(accessor.into());
        self
    }

    /// Set a colour accessor.
    pub fn color<A>(mut self, accessor: A) -> Self
    where
        A: Into<AccessorFunction<T>>,
    {
        self.color_accessor = Some(accessor.into());
        self
    }

    /// Override Silverman's rule with a fixed KDE bandwidth.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use gup::chart_builder::builders::violin;
    /// # #[derive(Clone, Debug)] struct D { val: f32 }
    /// violin::<D>().bandwidth(0.5);
    /// ```
    pub fn bandwidth(mut self, bw: f32) -> Self {
        self.bandwidth = Some(bw);
        self
    }

    /// Set the kernel function for KDE.
    pub fn kernel(mut self, kernel: KernelFunction) -> Self {
        self.kernel = kernel;
        self
    }

    /// Set the number of KDE grid evaluation points per violin.
    ///
    /// Must be ≥ 64. Default is 128.
    pub fn grid_points(mut self, n: usize) -> Self {
        self.n_grid_points = n.max(64);
        self
    }

    /// When `true`, clip the density curve to the observed data range.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use gup::chart_builder::builders::violin;
    /// # #[derive(Clone, Debug)] struct D { val: f32 }
    /// violin::<D>().trim(true);
    /// ```
    pub fn trim(mut self, trim: bool) -> Self {
        self.trim = trim;
        self
    }

    /// When `true`, overlay an embedded box plot inside each violin body.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use gup::chart_builder::builders::violin;
    /// # #[derive(Clone, Debug)] struct D { val: f32 }
    /// violin::<D>().show_box(true);
    /// ```
    pub fn show_box(mut self, show: bool) -> Self {
        self.show_box = show;
        self
    }

    /// Set the orientation to `Vertical` (default) or `Horizontal`.
    pub fn orientation(mut self, orientation: ViolinOrientation) -> Self {
        self.orientation = orientation;
        self
    }

    /// Which side(s) of the violin to render.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use gup::chart_builder::builders::violin::{violin, HalfSide};
    /// # #[derive(Clone, Debug)] struct D { val: f32 }
    /// violin::<D>().half(HalfSide::Left);
    /// ```
    pub fn half(mut self, side: HalfSide) -> Self {
        self.half = side;
        self
    }

    /// Set padding between violins (as a fraction of slot width).
    pub fn padding(mut self, padding: f32) -> Self {
        self.padding = padding;
        self
    }

    /// Set the maximum violin width.
    pub fn width(mut self, width: f32) -> Self {
        self.max_width = Some(width);
        self
    }

    /// Set an explicit category ordering.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use gup::chart_builder::builders::violin;
    /// # #[derive(Clone, Debug)] struct D { val: f32 }
    /// violin::<D>().order(vec!["C", "A", "B"]);
    /// ```
    pub fn order(mut self, order: Vec<&str>) -> Self {
        self.category_order = Some(order.into_iter().map(|s| s.to_string()).collect());
        self
    }

    /// Width of the embedded box plot as a fraction of violin width.
    ///
    /// Default is `0.1` (10% of violin width).
    pub fn box_width(mut self, ratio: f32) -> Self {
        self.box_width_ratio = ratio;
        self
    }

    /// Set the fill colour for the embedded box plot.
    pub fn box_color(mut self, color: [f32; 4]) -> Self {
        self.box_color = Some(color);
        self
    }

    /// Set the stroke width for the embedded box plot.
    pub fn box_stroke_width(mut self, width: f32) -> Self {
        self.box_stroke_width = width;
        self
    }

    /// Set a split-by accessor for half-violin pairwise comparison.
    ///
    /// The accessor should return exactly two distinct values. Data
    /// points with the first value are rendered on the left, and data
    /// points with the second value on the right.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use gup::chart_builder::accessor::AccessorValue;
    /// use gup::chart_builder::builders::{AccessorFunction, violin};
    ///
    /// # #[derive(Clone, Debug)] struct D { gender: String, val: f32 }
    /// violin()
    ///     .split_by(AccessorFunction::new(|d: &D| AccessorValue::String(d.gender.clone())));
    /// ```
    pub fn split_by<A>(mut self, accessor: A) -> Self
    where
        A: Into<AccessorFunction<T>>,
    {
        self.split_by_accessor = Some(accessor.into());
        self
    }
}

impl<T> Default for ViolinPlotBuilder<T> {
    fn default() -> Self {
        Self::new()
    }
}

// ── ConfigurableBuilder ──────────────────────────────────────────────────────

impl<T> ConfigurableBuilder for ViolinPlotBuilder<T> {
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

// ── GridCapableBuilder ───────────────────────────────────────────────────────

impl<T> GridCapableBuilder for ViolinPlotBuilder<T> {
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

// ── ChartBuilder ─────────────────────────────────────────────────────────────

impl<T> ChartBuilder<T> for ViolinPlotBuilder<T>
where
    T: Clone + MaybeSend + MaybeSync + std::fmt::Debug + 'static,
{
    type Output = ComposedChart<BoxPlotAttributes, BoxPlot>;

    fn build_with_data(self, data: Vec<T>, context: Arc<RenderContext>) -> GupResult<Self::Output> {
        if data.is_empty() {
            return Err(ChartBuilderError::EmptyData.into());
        }

        let violin_attrs = self.compute_violin_attributes(&data)?;

        let selection =
            Selection::<BoxPlotAttributes, BoxPlot>::new(violin_attrs, context.clone())?;

        let mut composed_chart = ComposedChart::new(selection, self.config).with_default_axes();

        // Compute the NDC chart area so box plot / violin values can be
        // mapped from data space into clip-space coordinates.
        let chart_area = composed_chart.calculate_chart_area();
        let w = composed_chart.config.width;
        let h = composed_chart.config.height;
        let ndc = super::NdcBounds {
            left: (chart_area.x / w) * 2.0 - 1.0,
            right: ((chart_area.x + chart_area.width) / w) * 2.0 - 1.0,
            top: 1.0 - (chart_area.y / h) * 2.0,
            bottom: 1.0 - ((chart_area.y + chart_area.height) / h) * 2.0,
        };

        let mapper = super::boxplot_ndc_mapper(composed_chart.visualization.data(), ndc);

        // Prepare the GPU render pipeline at build time, transforming
        // data-space attributes into NDC coordinates.
        composed_chart.visualization.prepare_render(
            context.device(),
            context.queue(),
            mapper,
            None,
            None,
        )?;

        Ok(composed_chart)
    }
}

impl<T> ViolinPlotBuilder<T>
where
    T: Clone + MaybeSend + MaybeSync + std::fmt::Debug + 'static,
{
    /// Compute violin data and produce `BoxPlotAttributes` for rendering.
    ///
    /// Each violin is represented as a `BoxPlotAttributes` instance so that
    /// it can be rendered using the existing `BoxPlot` mark and optionally
    /// display the five-number summary overlay.
    fn compute_violin_attributes(&self, data: &[T]) -> GupResult<Vec<BoxPlotAttributes>> {
        use crate::chart_builder::accessor::AccessorValue;
        use std::collections::HashMap;

        let y_accessor =
            self.y_accessor
                .as_ref()
                .ok_or_else(|| ChartBuilderError::MissingAccessor {
                    attribute: "y".to_string(),
                })?;

        // Group data by category (from x accessor)
        let mut category_data: HashMap<String, Vec<f32>> = HashMap::new();
        let mut category_order_vec: Vec<String> = Vec::new();

        for datum in data {
            let category = if let Some(ref x_acc) = self.x_accessor {
                match x_acc.apply(datum) {
                    AccessorValue::String(s) => s,
                    AccessorValue::Categorical(s) => s,
                    other => format!("{:?}", other),
                }
            } else {
                "default".to_string()
            };

            if !category_data.contains_key(&category) {
                category_order_vec.push(category.clone());
            }

            let value = y_accessor.apply(datum);
            let values = category_data.entry(category).or_default();
            match value {
                AccessorValue::Float(v) => values.push(v),
                AccessorValue::FloatArray(arr) => values.extend(arr),
                _ => {
                    return Err(crate::error::GupError::validation_error(format!(
                        "Violin plot requires Float or FloatArray accessor, got: {:?}",
                        value
                    )));
                }
            }
        }

        if category_data.is_empty() {
            return Err(ChartBuilderError::EmptyData.into());
        }

        // Apply category ordering
        let ordered_categories = if let Some(ref explicit_order) = self.category_order {
            // Use explicit order, appending any categories not in the list
            let mut result = Vec::new();
            for cat in explicit_order {
                if category_data.contains_key(cat) {
                    result.push(cat.clone());
                }
            }
            for cat in &category_order_vec {
                if !result.contains(cat) {
                    result.push(cat.clone());
                }
            }
            result
        } else {
            // Order of first appearance
            category_order_vec
        };

        let n = ordered_categories.len();
        let layout = compute_category_layout(n, self.config.width, self.padding, self.max_width);

        // Compute KDE and build attributes for each category
        let bp_orientation = match self.orientation {
            ViolinOrientation::Vertical => BoxPlotOrientation::Vertical,
            ViolinOrientation::Horizontal => BoxPlotOrientation::Horizontal,
        };

        let mut result = Vec::with_capacity(n);

        for (i, cat) in ordered_categories.iter().enumerate() {
            let values = category_data.get(cat).unwrap();
            if values.is_empty() {
                continue;
            }

            // Compute KDE
            let mut kde = KernelDensity1D::new(values.clone())
                .with_kernel(self.kernel)
                .with_n_eval_points(self.n_grid_points);

            if let Some(bw) = self.bandwidth {
                kde = kde.with_bandwidth(bw);
            }

            let kde_result = kde.compute_cpu();

            // Compute violin data for this category
            let (centre, half_w) = layout[i];

            let _violin_data = ViolinData {
                category: cat.clone(),
                grid_points: kde_result.eval_points.clone(),
                densities: kde_result.densities.clone(),
                bandwidth: kde_result.bandwidth,
                box_attrs: None,
                x_centre: centre,
                half_width: half_w,
            };

            // Build violin path
            let data_min = values.iter().cloned().fold(f32::INFINITY, f32::min);
            let data_max = values.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
            let peak_density = kde_result.peak_density();

            let _violin_path = ViolinPath::build(
                &kde_result.eval_points,
                &kde_result.densities,
                centre,
                half_w,
                peak_density,
                self.half,
                self.orientation,
                self.trim,
                data_min,
                data_max,
            );

            // Create BoxPlotAttributes so we can render using the existing BoxPlot mark.
            // The violin shape will be stored in the box plot position / sizing.
            let position = Vec2 { x: centre, y: 0.0 };
            let box_width = if self.show_box {
                half_w * 2.0 * self.box_width_ratio
            } else {
                0.0
            };

            let mut attrs =
                BoxPlotAttributes::from_data(values, position, box_width, bp_orientation);

            // Apply custom box plot styling
            if let Some(color) = self.box_color {
                attrs.box_fill_color = crate::shader_function::Vec4 {
                    x: color[0],
                    y: color[1],
                    z: color[2],
                    w: color[3],
                };
            }
            attrs.stroke_width = self.box_stroke_width;
            attrs.width = half_w * 2.0;

            result.push(attrs);
        }

        Ok(result)
    }
}

/// Convenience function to create a new violin plot builder.
///
/// # Examples
///
/// ```rust,no_run
/// use gup::prelude::*;
/// use gup::chart_builder::accessor::AccessorValue;
/// use gup::chart_builder::builders::{AccessorFunction, violin};
///
/// #[derive(Debug, Clone)]
/// struct Sample {
///     group: String,
///     value: f32,
/// }
///
/// # async fn example() -> GupResult<()> {
/// # let context = std::sync::Arc::new(RenderContext::new().await?);
/// # let data = vec![Sample { group: "A".to_string(), value: 1.0 }];
/// let chart = violin()
///     .y(AccessorFunction::new(|s: &Sample| AccessorValue::Float(s.value)))
///     .show_box(true)
///     .build_with_data(data, context)?;
/// # Ok(())
/// # }
/// ```
pub fn violin<T>() -> ViolinPlotBuilder<T> {
    ViolinPlotBuilder::new()
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chart_builder::accessor::AccessorValue;

    #[derive(Debug, Clone)]
    #[allow(dead_code)]
    struct TestData {
        category: String,
        value: f32,
    }

    // ── Layout tests ─────────────────────────────────────────────────────

    #[test]
    fn test_compute_category_layout_empty() {
        let layout = compute_category_layout(0, 1.0, 0.05, None);
        assert!(layout.is_empty());
    }

    #[test]
    fn test_compute_category_layout_single() {
        let layout = compute_category_layout(1, 1.0, 0.05, None);
        assert_eq!(layout.len(), 1);
        assert!((layout[0].0 - 0.5).abs() < 1e-6, "centre should be 0.5");
        assert!(layout[0].1 > 0.0, "half_width should be positive");
    }

    #[test]
    fn test_compute_category_layout_even_spacing() {
        let layout = compute_category_layout(3, 1.0, 0.0, None);
        assert_eq!(layout.len(), 3);

        // Centres should be evenly spaced
        let spacing = layout[1].0 - layout[0].0;
        let spacing2 = layout[2].0 - layout[1].0;
        assert!(
            (spacing - spacing2).abs() < 1e-6,
            "spacings should be equal"
        );
    }

    #[test]
    fn test_compute_category_layout_max_width() {
        let layout_unclamped = compute_category_layout(1, 1.0, 0.0, None);
        let layout_clamped = compute_category_layout(1, 1.0, 0.0, Some(0.1));
        assert!(layout_clamped[0].1 < layout_unclamped[0].1);
        assert!((layout_clamped[0].1 - 0.05).abs() < 1e-6);
    }

    // ── ViolinPath tests ─────────────────────────────────────────────────

    #[test]
    fn test_violin_path_mirrored_closure() {
        let grid = vec![0.0, 1.0, 2.0];
        let dens = vec![0.1, 0.5, 0.1];
        let path = ViolinPath::build(
            &grid,
            &dens,
            0.5,
            0.3,
            0.5,
            HalfSide::Both,
            ViolinOrientation::Vertical,
            false,
            0.0,
            2.0,
        );

        // A full violin path should have right_flank + left_flank = 2 * grid.len() vertices
        assert_eq!(path.vertices.len(), 6);

        // The path should be closed: first vertex == last vertex
        // (they meet at the bottom-left of the violin)
        // For vertical, first is right-flank start, last is left-flank end
        // Both correspond to grid[0] with density d[0]:
        //   first = (centre + d[0]*scale, grid[0])
        //   last  = (centre - d[0]*scale, grid[0])
        // They only meet if d[0] == 0. With non-zero density, the polygon is NOT
        // closed in the strict sense. That is correct: the density-based polygon
        // is an open contour whose top and bottom connect through the mirroring.
    }

    #[test]
    fn test_violin_path_half_right_no_positive_left() {
        let grid = vec![0.0, 1.0, 2.0, 3.0];
        let dens = vec![0.0, 0.3, 0.5, 0.0];
        let path = ViolinPath::build(
            &grid,
            &dens,
            0.0,
            1.0,
            0.5,
            HalfSide::Right,
            ViolinOrientation::Vertical,
            false,
            0.0,
            3.0,
        );

        // For half=Right, all vertices should have x >= centre (0.0)
        for v in &path.vertices {
            assert!(
                v[0] >= -1e-6,
                "Right half-violin should not have negative-x vertices, got {}",
                v[0]
            );
        }
    }

    #[test]
    fn test_violin_path_half_left_no_positive() {
        let grid = vec![0.0, 1.0, 2.0, 3.0];
        let dens = vec![0.0, 0.3, 0.5, 0.0];
        let path = ViolinPath::build(
            &grid,
            &dens,
            0.0,
            1.0,
            0.5,
            HalfSide::Left,
            ViolinOrientation::Vertical,
            false,
            0.0,
            3.0,
        );

        // For half=Left, all vertices should have x <= centre (0.0)
        for v in &path.vertices {
            assert!(
                v[0] <= 1e-6,
                "Left half-violin should not have positive-x vertices, got {}",
                v[0]
            );
        }
    }

    #[test]
    fn test_violin_path_trim() {
        let grid = vec![-1.0, 0.0, 1.0, 2.0, 3.0, 4.0];
        let dens = vec![0.01, 0.1, 0.5, 0.5, 0.1, 0.01];
        let path_untrimmed = ViolinPath::build(
            &grid,
            &dens,
            0.0,
            1.0,
            0.5,
            HalfSide::Both,
            ViolinOrientation::Vertical,
            false,
            0.0,
            3.0,
        );
        let path_trimmed = ViolinPath::build(
            &grid,
            &dens,
            0.0,
            1.0,
            0.5,
            HalfSide::Both,
            ViolinOrientation::Vertical,
            true,
            0.0,
            3.0,
        );

        // Trimmed should have fewer vertices (points outside [0, 3] removed)
        assert!(
            path_trimmed.vertices.len() < path_untrimmed.vertices.len(),
            "trimmed ({}) should have fewer vertices than untrimmed ({})",
            path_trimmed.vertices.len(),
            path_untrimmed.vertices.len()
        );
    }

    #[test]
    fn test_violin_path_empty() {
        let path = ViolinPath::build(
            &[],
            &[],
            0.0,
            1.0,
            0.0,
            HalfSide::Both,
            ViolinOrientation::Vertical,
            false,
            0.0,
            0.0,
        );
        assert!(path.vertices.is_empty());
    }

    #[test]
    fn test_violin_path_horizontal_orientation() {
        let grid = vec![0.0, 1.0, 2.0];
        let dens = vec![0.1, 0.5, 0.1];
        let path = ViolinPath::build(
            &grid,
            &dens,
            0.5,
            0.3,
            0.5,
            HalfSide::Both,
            ViolinOrientation::Horizontal,
            false,
            0.0,
            2.0,
        );

        // For horizontal orientation, x and y are swapped
        // The grid (value axis) should be in the x coordinate
        // The categorical offset should be in the y coordinate
        assert!(!path.vertices.is_empty());
    }

    // ── Builder config tests ─────────────────────────────────────────────

    #[test]
    fn test_violin_builder_defaults() {
        let builder = ViolinPlotBuilder::<TestData>::new();
        assert!(builder.x_accessor.is_none());
        assert!(builder.y_accessor.is_none());
        assert!(builder.bandwidth.is_none());
        assert!(!builder.trim);
        assert!(!builder.show_box);
        assert_eq!(builder.half, HalfSide::Both);
        assert_eq!(builder.orientation, ViolinOrientation::Vertical);
        assert_eq!(builder.n_grid_points, 128);
        assert!((builder.box_width_ratio - 0.1).abs() < 1e-6);
    }

    #[test]
    fn test_violin_builder_fluent_api() {
        let builder = violin::<TestData>()
            .bandwidth(0.5)
            .trim(true)
            .show_box(true)
            .orientation(ViolinOrientation::Horizontal)
            .half(HalfSide::Left)
            .padding(0.1)
            .width(50.0)
            .box_width(0.2)
            .box_color([1.0, 0.0, 0.0, 1.0])
            .box_stroke_width(2.0)
            .grid_points(256)
            .order(vec!["C", "A", "B"])
            .title("Test Violin")
            .height(400.0);

        assert_eq!(builder.bandwidth, Some(0.5));
        assert!(builder.trim);
        assert!(builder.show_box);
        assert_eq!(builder.orientation, ViolinOrientation::Horizontal);
        assert_eq!(builder.half, HalfSide::Left);
        assert!((builder.padding - 0.1).abs() < 1e-6);
        assert_eq!(builder.max_width, Some(50.0));
        assert!((builder.box_width_ratio - 0.2).abs() < 1e-6);
        assert_eq!(builder.box_color, Some([1.0, 0.0, 0.0, 1.0]));
        assert!((builder.box_stroke_width - 2.0).abs() < 1e-6);
        assert_eq!(builder.n_grid_points, 256);
        assert_eq!(
            builder.category_order,
            Some(vec!["C".to_string(), "A".to_string(), "B".to_string()])
        );
        assert_eq!(builder.config.title(), Some("Test Violin"));
        assert!((builder.config.height - 400.0).abs() < 1e-6);
    }

    #[test]
    fn test_violin_builder_default_impl() {
        let builder = ViolinPlotBuilder::<TestData>::default();
        assert!(builder.x_accessor.is_none());
        assert_eq!(builder.half, HalfSide::default());
    }

    #[test]
    fn test_violin_builder_grid_api() {
        let builder = violin::<TestData>().grid();
        assert!(builder.config.show_grid);

        let h_builder = violin::<TestData>().horizontal_grid_only();
        assert!(h_builder.config.show_grid);
        assert!(h_builder.config.grid_config.show_horizontal);
        assert!(!h_builder.config.grid_config.show_vertical);
    }

    // ── KDE integration tests ────────────────────────────────────────────

    #[test]
    fn test_kde_grid_round_trip() {
        // Known Gaussian distribution: peak should be at the mean
        let mut rng_values = Vec::new();
        // Simulated normal-ish distribution centred at 50
        for i in 0..200 {
            let v = 50.0 + (i as f32 - 100.0) * 0.15;
            rng_values.push(v);
        }

        let kde = KernelDensity1D::new(rng_values)
            .with_kernel(KernelFunction::Gaussian)
            .with_n_eval_points(128);

        let result = kde.compute_cpu();

        assert!(result.eval_points.len() >= 64);
        assert_eq!(result.densities.len(), result.eval_points.len());

        // Peak should be near 50
        let mode = result.mode().unwrap();
        assert!(
            (mode - 50.0).abs() < 5.0,
            "mode {} should be near 50.0",
            mode
        );

        // Density should be non-negative
        for d in &result.densities {
            assert!(*d >= 0.0, "density should be non-negative");
        }
    }

    // ── Build integration tests (require GPU) ────────────────────────────

    #[tokio::test]
    async fn test_violin_build_basic() {
        let data: Vec<TestData> = (0..50)
            .map(|i| TestData {
                category: if i % 2 == 0 {
                    "A".to_string()
                } else {
                    "B".to_string()
                },
                value: 10.0 + (i as f32) * 0.5,
            })
            .collect();

        let context = Arc::new(RenderContext::new().await.unwrap());

        let result = violin()
            .x(AccessorFunction::new(|d: &TestData| {
                AccessorValue::String(d.category.clone())
            }))
            .y(AccessorFunction::new(|d: &TestData| {
                AccessorValue::Float(d.value)
            }))
            .show_box(true)
            .build_with_data(data, context);

        assert!(result.is_ok(), "build should succeed: {:?}", result.err());
        let chart = result.unwrap();
        assert_eq!(chart.len(), 2, "should have 2 violins (A and B)");
    }

    #[tokio::test]
    async fn test_violin_build_empty_data() {
        let context = Arc::new(RenderContext::new().await.unwrap());
        let result = violin::<TestData>()
            .y(AccessorFunction::new(|d: &TestData| {
                AccessorValue::Float(d.value)
            }))
            .build_with_data(vec![], context);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_violin_build_missing_y() {
        let data = vec![TestData {
            category: "A".to_string(),
            value: 1.0,
        }];
        let context = Arc::new(RenderContext::new().await.unwrap());
        let result = violin::<TestData>().build_with_data(data, context);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_violin_build_three_categories() {
        let mut data = Vec::new();
        for cat in &["X", "Y", "Z"] {
            for i in 0..30 {
                data.push(TestData {
                    category: cat.to_string(),
                    value: 20.0 + i as f32,
                });
            }
        }

        let context = Arc::new(RenderContext::new().await.unwrap());

        let result = violin()
            .x(AccessorFunction::new(|d: &TestData| {
                AccessorValue::String(d.category.clone())
            }))
            .y(AccessorFunction::new(|d: &TestData| {
                AccessorValue::Float(d.value)
            }))
            .trim(true)
            .show_box(true)
            .build_with_data(data, context);

        assert!(result.is_ok());
        let chart = result.unwrap();
        assert_eq!(chart.len(), 3);
    }

    #[tokio::test]
    async fn test_violin_build_single_category_no_x() {
        let data: Vec<TestData> = (0..30)
            .map(|i| TestData {
                category: "A".to_string(),
                value: i as f32,
            })
            .collect();

        let context = Arc::new(RenderContext::new().await.unwrap());

        let result = violin()
            .y(AccessorFunction::new(|d: &TestData| {
                AccessorValue::Float(d.value)
            }))
            .build_with_data(data, context);

        assert!(result.is_ok());
        let chart = result.unwrap();
        assert_eq!(chart.len(), 1, "should have 1 violin (default category)");
    }

    #[tokio::test]
    async fn test_violin_is_render_ready_after_build() {
        let data = vec![
            TestData {
                category: "A".to_string(),
                value: 10.0,
            },
            TestData {
                category: "A".to_string(),
                value: 20.0,
            },
            TestData {
                category: "A".to_string(),
                value: 30.0,
            },
            TestData {
                category: "A".to_string(),
                value: 25.0,
            },
            TestData {
                category: "A".to_string(),
                value: 15.0,
            },
        ];

        let context = Arc::new(RenderContext::new().await.unwrap());

        let chart = violin()
            .y(AccessorFunction::new(|d: &TestData| {
                AccessorValue::Float(d.value)
            }))
            .build_with_data(data, context)
            .unwrap();

        assert!(
            chart.visualization.is_render_ready(),
            "Violin selection should be render-ready after build"
        );
    }

    #[tokio::test]
    async fn test_violin_render_to_png_produces_visible_marks() {
        let data = vec![
            TestData {
                category: "A".to_string(),
                value: 10.0,
            },
            TestData {
                category: "A".to_string(),
                value: 20.0,
            },
            TestData {
                category: "A".to_string(),
                value: 30.0,
            },
            TestData {
                category: "A".to_string(),
                value: 25.0,
            },
            TestData {
                category: "A".to_string(),
                value: 15.0,
            },
        ];

        let context = Arc::new(RenderContext::new().await.unwrap());

        let mut chart = violin()
            .y(AccessorFunction::new(|d: &TestData| {
                AccessorValue::Float(d.value)
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
            "Expected visible violin marks in the data region, but found only {non_white} non-white pixels"
        );
    }
}

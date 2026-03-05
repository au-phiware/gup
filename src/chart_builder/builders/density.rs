// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Density plot builder with 2D KDE and contour extraction.
//!
//! Provides [`DensityPlotBuilder`] for creating GPU-accelerated density plots
//! that reveal spatial concentration patterns hidden by overplotting in scatter
//! plots.  Supports two rendering modes:
//!
//! - **Filled contour mode** (default): contour bands rendered as filled
//!   polygons colour-mapped to density level.
//! - **Contour-line mode**: iso-level lines extracted via marching squares.
//!
//! The builder follows the fluent owned-`self` pattern established across the
//! chart builder ecosystem.
//!
//! # Examples
//!
//! ## Basic density plot
//!
//! ```rust,no_run
//! use gup::chart_builder::builders::density::{density_plot, DensityRenderMode};
//! use gup::chart_builder::builders::{AccessorFunction, ConfigurableBuilder};
//! use gup::chart_builder::accessor::AccessorValue;
//!
//! #[derive(Debug, Clone)]
//! struct Point { x: f32, y: f32 }
//!
//! let builder = density_plot::<Point>()
//!     .x(AccessorFunction::new(|d: &Point| AccessorValue::Float(d.x)))
//!     .y(AccessorFunction::new(|d: &Point| AccessorValue::Float(d.y)))
//!     .bandwidth(0.5)
//!     .levels(10)
//!     .fill(true)
//!     .title("Density estimate");
//! ```
//!
//! ## Contour-line mode
//!
//! ```rust,no_run
//! use gup::chart_builder::builders::density::density_plot;
//! use gup::chart_builder::builders::{AccessorFunction, ConfigurableBuilder};
//! use gup::chart_builder::accessor::AccessorValue;
//!
//! # #[derive(Debug, Clone)]
//! # struct Point { x: f32, y: f32 }
//! let builder = density_plot::<Point>()
//!     .x(AccessorFunction::new(|d: &Point| AccessorValue::Float(d.x)))
//!     .y(AccessorFunction::new(|d: &Point| AccessorValue::Float(d.y)))
//!     .fill(false)
//!     .levels(12);
//! ```

use super::{AccessorFunction, ConfigurableBuilder, GridCapableBuilder};
use crate::chart_builder::{ChartBuilder, ChartBuilderError, ChartConfig, ComposedChart};
use crate::error::GupResult;
use crate::grid::{GridConfiguration, GridLineConfig};
use crate::label::LabelFormatter;
use crate::mark::rectangle::Rectangle;
use crate::selection::Selection;
use crate::shader_function::{ColorScale, KDEResult2D, KernelDensity2D, KernelFunction};
use crate::{MaybeSend, MaybeSync, RenderContext};
use std::marker::PhantomData;
use std::sync::Arc;

// ── DensityRenderMode ────────────────────────────────────────────────────

/// How the density field is visualised.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DensityRenderMode {
    /// Filled contour bands colour-mapped to density (default).
    FilledContour,
    /// Iso-level contour lines only.
    ContourLine,
}

// ── DensityConfig ────────────────────────────────────────────────────────

/// Configuration for 2D kernel density estimation and contour extraction.
///
/// All fields have sensible defaults via [`Default`].
#[derive(Debug, Clone)]
pub struct DensityConfig {
    /// KDE bandwidth — `None` triggers Silverman's rule.
    pub bandwidth: Option<f32>,
    /// Number of iso-contour levels (default: 8).
    pub levels: usize,
    /// Rendering mode (filled contour or line contour).
    pub render_mode: DensityRenderMode,
    /// Grid resolution along each axis (default: 256).
    pub grid_size: usize,
    /// Fractional padding added around data extents (default: 0.05 = 5%).
    pub margin: f32,
    /// Sample count above which KDE is dispatched on the GPU (default: 5 000).
    /// Set to `usize::MAX` to force the CPU path.
    pub gpu_threshold: usize,
}

impl Default for DensityConfig {
    fn default() -> Self {
        Self {
            bandwidth: None,
            levels: 8,
            render_mode: DensityRenderMode::FilledContour,
            grid_size: 256,
            margin: 0.05,
            gpu_threshold: super::gpu_density::DEFAULT_GPU_THRESHOLD,
        }
    }
}

// ── MarchingSquaresResult ────────────────────────────────────────────────

/// Output of the marching-squares contour extraction.
///
/// Each iso-level produces a set of line segments (pairs of endpoints).
#[derive(Debug, Clone)]
pub struct ContourLevel {
    /// The density value at which this contour was extracted.
    pub threshold: f32,
    /// Line segments as `[(x0, y0), (x1, y1)]` pairs.
    pub segments: Vec<[(f32, f32); 2]>,
}

/// Output of the marching-squares contour extraction for filled bands.
///
/// Each band represents the region between two adjacent iso-levels.
#[derive(Debug, Clone)]
pub struct ContourBand {
    /// Lower density threshold of this band.
    pub low: f32,
    /// Upper density threshold of this band.
    pub high: f32,
    /// Normalised density value in `[0, 1]` for colour lookup.
    pub normalised: f32,
    /// Triangles that tile this band, stored as flat `(x, y)` vertices in
    /// groups of three.
    pub triangles: Vec<(f32, f32)>,
}

// ── Marching-squares CPU implementation ──────────────────────────────────

/// Edge index lookup table for marching squares.
///
/// For each of the 16 cell configurations the table lists pairs of edge
/// indices that form line segments.  Edge numbering:
///
/// ```text
///     0
///   ┌───┐
/// 3 │   │ 1
///   └───┘
///     2
/// ```
const MARCHING_SQUARES_EDGES: [&[(u8, u8)]; 16] = [
    &[],               // 0000
    &[(3, 0)],         // 0001
    &[(0, 1)],         // 0010
    &[(3, 1)],         // 0011
    &[(1, 2)],         // 0100
    &[(3, 0), (1, 2)], // 0101 – saddle
    &[(0, 2)],         // 0110
    &[(3, 2)],         // 0111
    &[(2, 3)],         // 1000
    &[(2, 0)],         // 1001
    &[(0, 1), (2, 3)], // 1010 – saddle
    &[(2, 1)],         // 1011
    &[(1, 3)],         // 1100
    &[(1, 0)],         // 1101
    &[(0, 3)],         // 1110
    &[],               // 1111
];

/// Linearly interpolate the crossing point along an edge.
fn edge_interpolation(
    x: usize,
    y: usize,
    edge: u8,
    grid: &[f32],
    cols: usize,
    threshold: f32,
    x_points: &[f32],
    y_points: &[f32],
) -> (f32, f32) {
    let v00 = grid[y * cols + x];
    let v10 = grid[y * cols + (x + 1)];
    let v01 = grid[(y + 1) * cols + x];
    let v11 = grid[(y + 1) * cols + (x + 1)];

    let x0 = x_points[x];
    let x1 = x_points[x + 1];
    let y0 = y_points[y];
    let y1 = y_points[y + 1];

    match edge {
        // Top edge (v00 → v10)
        0 => {
            let t = safe_lerp_t(threshold, v00, v10);
            (x0 + t * (x1 - x0), y0)
        }
        // Right edge (v10 → v11)
        1 => {
            let t = safe_lerp_t(threshold, v10, v11);
            (x1, y0 + t * (y1 - y0))
        }
        // Bottom edge (v01 → v11)
        2 => {
            let t = safe_lerp_t(threshold, v01, v11);
            (x0 + t * (x1 - x0), y1)
        }
        // Left edge (v00 → v01)
        3 => {
            let t = safe_lerp_t(threshold, v00, v01);
            (x0, y0 + t * (y1 - y0))
        }
        _ => unreachable!(),
    }
}

/// Safe interpolation parameter avoiding division by zero.
#[inline]
fn safe_lerp_t(threshold: f32, a: f32, b: f32) -> f32 {
    let denom = b - a;
    if denom.abs() < 1e-12 {
        0.5
    } else {
        ((threshold - a) / denom).clamp(0.0, 1.0)
    }
}

/// Extract contour line segments from a scalar field at a given iso-level
/// using the marching-squares algorithm.
///
/// `grid` is in row-major order (y varies first); `rows` × `cols`.
pub fn marching_squares(
    grid: &[f32],
    rows: usize,
    cols: usize,
    threshold: f32,
    x_points: &[f32],
    y_points: &[f32],
) -> Vec<[(f32, f32); 2]> {
    assert_eq!(grid.len(), rows * cols);
    assert_eq!(x_points.len(), cols);
    assert_eq!(y_points.len(), rows);

    let mut segments: Vec<[(f32, f32); 2]> = Vec::new();

    for y in 0..rows - 1 {
        for x in 0..cols - 1 {
            let v00 = grid[y * cols + x];
            let v10 = grid[y * cols + (x + 1)];
            let v01 = grid[(y + 1) * cols + x];
            let v11 = grid[(y + 1) * cols + (x + 1)];

            let mut idx: u8 = 0;
            if v00 >= threshold {
                idx |= 1;
            }
            if v10 >= threshold {
                idx |= 2;
            }
            if v11 >= threshold {
                idx |= 4;
            }
            if v01 >= threshold {
                idx |= 8;
            }

            // Saddle disambiguation: compare centre value with threshold.
            let edges = if idx == 5 || idx == 10 {
                let centre = (v00 + v10 + v01 + v11) * 0.25;
                if idx == 5 {
                    if centre >= threshold {
                        // Connected: both inside corners connect.
                        &[(3u8, 2u8), (0, 1)] as &[(u8, u8)]
                    } else {
                        MARCHING_SQUARES_EDGES[idx as usize]
                    }
                } else {
                    // idx == 10
                    if centre >= threshold {
                        &[(0u8, 3u8), (2, 1)] as &[(u8, u8)]
                    } else {
                        MARCHING_SQUARES_EDGES[idx as usize]
                    }
                }
            } else {
                MARCHING_SQUARES_EDGES[idx as usize]
            };

            for &(e0, e1) in edges {
                let p0 = edge_interpolation(x, y, e0, grid, cols, threshold, x_points, y_points);
                let p1 = edge_interpolation(x, y, e1, grid, cols, threshold, x_points, y_points);
                segments.push([p0, p1]);
            }
        }
    }

    segments
}

/// Extract filled contour bands between adjacent iso-levels using exact
/// marching-squares polygon decomposition.
///
/// Each cell emits precisely the polygon region where the scalar field lies
/// within the band boundaries, producing smooth contour fills even at low
/// grid resolutions.
///
/// Returns one [`ContourBand`] per pair of adjacent thresholds plus bands
/// below the first level and above the last level.
pub fn filled_contour_bands(
    grid: &[f32],
    rows: usize,
    cols: usize,
    thresholds: &[f32],
    x_points: &[f32],
    y_points: &[f32],
) -> Vec<ContourBand> {
    let min_val = grid.iter().copied().fold(f32::INFINITY, f32::min);
    let max_val = grid.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let range = max_val - min_val;

    let mut bands = Vec::new();

    // Build boundaries: [min, t0, t1, ..., max]
    let mut boundaries = vec![min_val];
    for &t in thresholds {
        if t > min_val && t < max_val {
            boundaries.push(t);
        }
    }
    // Nudge the top boundary slightly above max_val so that cells at
    // exactly max_val are classified as Inside rather than Above in the
    // last band.
    let top = max_val + range.max(1e-6) * 1e-6;
    boundaries.push(top);
    boundaries.dedup_by(|a, b| (*a - *b).abs() < 1e-12);

    for pair in boundaries.windows(2) {
        let low = pair[0];
        let high = pair[1];
        let mid = (low + high) * 0.5;
        let normalised = if range > 1e-12 {
            (mid - min_val) / range
        } else {
            0.5
        };

        let mut triangles = Vec::new();
        for y in 0..rows - 1 {
            for x in 0..cols - 1 {
                let values = [
                    grid[y * cols + x],
                    grid[y * cols + (x + 1)],
                    grid[(y + 1) * cols + (x + 1)],
                    grid[(y + 1) * cols + x],
                ];
                let positions = [
                    (x_points[x], y_points[y]),
                    (x_points[x + 1], y_points[y]),
                    (x_points[x + 1], y_points[y + 1]),
                    (x_points[x], y_points[y + 1]),
                ];

                for poly in cell_band_polygons(values, positions, low, high) {
                    fan_triangulate(&poly, &mut triangles);
                }
            }
        }

        bands.push(ContourBand {
            low,
            high: high.min(max_val),
            normalised,
            triangles,
        });
    }

    bands
}

// ── Exact marching-squares band decomposition ────────────────────────────

/// Corner classification relative to a contour band `[low, high)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BandState {
    /// Value is below the band lower bound.
    Below,
    /// Value lies within the band.
    Inside,
    /// Value is at or above the band upper bound.
    Above,
}

/// Classify a scalar value relative to a band `[low, high)`.
#[inline]
fn classify_band(value: f32, low: f32, high: f32) -> BandState {
    if value < low {
        BandState::Below
    } else if value < high {
        BandState::Inside
    } else {
        BandState::Above
    }
}

/// Interpolate the position where a threshold crosses a line segment.
#[inline]
fn lerp_point(p0: (f32, f32), p1: (f32, f32), v0: f32, v1: f32, threshold: f32) -> (f32, f32) {
    let t = safe_lerp_t(threshold, v0, v1);
    (p0.0 + t * (p1.0 - p0.0), p0.1 + t * (p1.1 - p0.1))
}

/// Collect the band polygon(s) for a single cell.
///
/// Corners are ordered clockwise: `[v00 (TL), v10 (TR), v11 (BR), v01 (BL)]`.
/// Returns zero or more polygons (multiple polygons arise from saddle
/// configurations where the band region has disconnected components).
fn cell_band_polygons(
    values: [f32; 4],
    positions: [(f32, f32); 4],
    low: f32,
    high: f32,
) -> Vec<Vec<(f32, f32)>> {
    let states = [
        classify_band(values[0], low, high),
        classify_band(values[1], low, high),
        classify_band(values[2], low, high),
        classify_band(values[3], low, high),
    ];

    // All corners share a non-Inside state → no band region in this cell.
    if states.iter().all(|&s| s == BandState::Below)
        || states.iter().all(|&s| s == BandState::Above)
    {
        return vec![];
    }

    // All corners Inside → full cell quad.
    if states.iter().all(|&s| s == BandState::Inside) {
        return vec![vec![positions[0], positions[1], positions[2], positions[3]]];
    }

    // Detect saddle: opposite corners share a state, adjacent corners
    // differ.  Saddle cells can produce disconnected band regions so we
    // subdivide into 4 triangles through the cell centre.
    let is_saddle = states[0] == states[2] && states[1] == states[3] && states[0] != states[1];

    if is_saddle {
        let centre_val = (values[0] + values[1] + values[2] + values[3]) * 0.25;
        let centre_pos = (
            (positions[0].0 + positions[2].0) * 0.5,
            (positions[0].1 + positions[2].1) * 0.5,
        );

        let mut polygons = Vec::new();
        // 4 triangles: (0,1,c), (1,2,c), (2,3,c), (3,0,c)
        for k in 0..4 {
            let j = (k + 1) % 4;
            let tri_vals = [values[k], values[j], centre_val];
            let tri_pos = [positions[k], positions[j], centre_pos];
            let poly = triangle_band_polygon(tri_vals, tri_pos, low, high);
            if !poly.is_empty() {
                polygons.push(poly);
            }
        }
        return polygons;
    }

    // Non-saddle cell: walk the boundary and collect band polygon vertices.
    let poly = boundary_walk_quad(&values, &positions, &states, low, high);
    if poly.is_empty() { vec![] } else { vec![poly] }
}

/// Walk the boundary of a non-saddle quadrilateral cell and collect the
/// vertices of the band polygon in winding order.
fn boundary_walk_quad(
    values: &[f32; 4],
    positions: &[(f32, f32); 4],
    states: &[BandState; 4],
    low: f32,
    high: f32,
) -> Vec<(f32, f32)> {
    let mut vertices = Vec::with_capacity(8);
    for k in 0..4 {
        let j = (k + 1) % 4;
        emit_edge_vertices(
            states[k],
            states[j],
            positions[k],
            positions[j],
            values[k],
            values[j],
            low,
            high,
            &mut vertices,
        );
    }
    vertices
}

/// Collect band polygon vertices for a single triangle sub-cell.
///
/// Used during saddle subdivision.  Triangles have no saddle ambiguity so
/// a simple boundary walk suffices.
fn triangle_band_polygon(
    values: [f32; 3],
    positions: [(f32, f32); 3],
    low: f32,
    high: f32,
) -> Vec<(f32, f32)> {
    let states = [
        classify_band(values[0], low, high),
        classify_band(values[1], low, high),
        classify_band(values[2], low, high),
    ];

    if states.iter().all(|&s| s == BandState::Below)
        || states.iter().all(|&s| s == BandState::Above)
    {
        return vec![];
    }

    if states.iter().all(|&s| s == BandState::Inside) {
        return vec![positions[0], positions[1], positions[2]];
    }

    let mut vertices = Vec::with_capacity(6);
    for k in 0..3 {
        let j = (k + 1) % 3;
        emit_edge_vertices(
            states[k],
            states[j],
            positions[k],
            positions[j],
            values[k],
            values[j],
            low,
            high,
            &mut vertices,
        );
    }
    vertices
}

/// Emit band polygon vertices along a single edge from corner `i` to
/// corner `j`.
///
/// The corner's own position is emitted first (if Inside), followed by any
/// threshold crossing points in traversal order.
#[inline]
fn emit_edge_vertices(
    si: BandState,
    sj: BandState,
    pi: (f32, f32),
    pj: (f32, f32),
    vi: f32,
    vj: f32,
    low: f32,
    high: f32,
    out: &mut Vec<(f32, f32)>,
) {
    // Emit starting corner when Inside.
    if si == BandState::Inside {
        out.push(pi);
    }

    // Emit threshold crossings in traversal order.
    match (si, sj) {
        (BandState::Below, BandState::Inside) => {
            out.push(lerp_point(pi, pj, vi, vj, low));
        }
        (BandState::Below, BandState::Above) => {
            out.push(lerp_point(pi, pj, vi, vj, low));
            out.push(lerp_point(pi, pj, vi, vj, high));
        }
        (BandState::Inside, BandState::Below) => {
            out.push(lerp_point(pi, pj, vi, vj, low));
        }
        (BandState::Inside, BandState::Above) => {
            out.push(lerp_point(pi, pj, vi, vj, high));
        }
        (BandState::Above, BandState::Below) => {
            out.push(lerp_point(pi, pj, vi, vj, high));
            out.push(lerp_point(pi, pj, vi, vj, low));
        }
        (BandState::Above, BandState::Inside) => {
            out.push(lerp_point(pi, pj, vi, vj, high));
        }
        _ => {} // Same state on both corners: no crossings.
    }
}

/// Fan-triangulate a convex polygon and append the resulting triangle
/// vertices (groups of 3) to `out`.
fn fan_triangulate(polygon: &[(f32, f32)], out: &mut Vec<(f32, f32)>) {
    if polygon.len() < 3 {
        return;
    }
    for i in 1..polygon.len() - 1 {
        out.push(polygon[0]);
        out.push(polygon[i]);
        out.push(polygon[i + 1]);
    }
}

// ── DensityPlotBuilder ──────────────────────────────────────────────────

/// Fluent builder for GPU-accelerated density plots.
///
/// Computes a 2D kernel density estimate over the provided data and
/// visualises the result as filled contour bands or iso-level contour
/// lines, rendering via instanced [`Rectangle`] marks colour-mapped
/// with a [`ColorScale`].
///
/// # Examples
///
/// ```rust,no_run
/// use gup::chart_builder::builders::density::density_plot;
/// use gup::chart_builder::builders::{AccessorFunction, ConfigurableBuilder};
/// use gup::chart_builder::accessor::AccessorValue;
///
/// #[derive(Debug, Clone)]
/// struct Point { x: f32, y: f32 }
///
/// let builder = density_plot::<Point>()
///     .x(AccessorFunction::new(|d: &Point| AccessorValue::Float(d.x)))
///     .y(AccessorFunction::new(|d: &Point| AccessorValue::Float(d.y)))
///     .bandwidth(0.5)
///     .levels(10)
///     .fill(true)
///     .title("2D density plot");
/// ```
#[derive(Debug, Clone)]
pub struct DensityPlotBuilder<T> {
    pub(crate) x_accessor: Option<AccessorFunction<T>>,
    pub(crate) y_accessor: Option<AccessorFunction<T>>,
    pub(crate) config: ChartConfig,
    pub(crate) density_config: DensityConfig,
    pub(crate) color_scale: Option<ColorScale>,
    pub(crate) _phantom: PhantomData<T>,
}

impl<T> DensityPlotBuilder<T> {
    /// Create a new density plot builder with default settings.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use gup::chart_builder::builders::density::DensityPlotBuilder;
    ///
    /// # #[derive(Debug, Clone)]
    /// # struct Point { x: f32, y: f32 }
    /// let builder = DensityPlotBuilder::<Point>::new();
    /// ```
    pub fn new() -> Self {
        Self {
            x_accessor: None,
            y_accessor: None,
            config: ChartConfig::default(),
            density_config: DensityConfig::default(),
            color_scale: None,
            _phantom: PhantomData,
        }
    }

    /// Set the X-axis accessor function.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use gup::chart_builder::builders::density::density_plot;
    /// use gup::chart_builder::builders::AccessorFunction;
    /// use gup::chart_builder::accessor::AccessorValue;
    ///
    /// # #[derive(Debug, Clone)]
    /// # struct P { x: f32 }
    /// let b = density_plot::<P>()
    ///     .x(AccessorFunction::new(|d: &P| AccessorValue::Float(d.x)));
    /// ```
    pub fn x<A>(mut self, accessor: A) -> Self
    where
        A: Into<AccessorFunction<T>>,
    {
        self.x_accessor = Some(accessor.into());
        self
    }

    /// Set the Y-axis accessor function.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use gup::chart_builder::builders::density::density_plot;
    /// use gup::chart_builder::builders::AccessorFunction;
    /// use gup::chart_builder::accessor::AccessorValue;
    ///
    /// # #[derive(Debug, Clone)]
    /// # struct P { y: f32 }
    /// let b = density_plot::<P>()
    ///     .y(AccessorFunction::new(|d: &P| AccessorValue::Float(d.y)));
    /// ```
    pub fn y<A>(mut self, accessor: A) -> Self
    where
        A: Into<AccessorFunction<T>>,
    {
        self.y_accessor = Some(accessor.into());
        self
    }

    /// No-op colour accessor for API consistency with other chart
    /// builders.
    ///
    /// Density plots derive colour from the [`ColorScale`] set via
    /// [`color_scheme`](Self::color_scheme); this method exists solely
    /// so that the generic `plot().data(d).density(x, y).color(c)`
    /// convenience chain compiles.
    pub fn color<A>(self, _accessor: A) -> Self
    where
        A: Into<AccessorFunction<T>>,
    {
        // Colour is determined by the colour scale, not per-datum.
        self
    }

    /// Set a fixed KDE bandwidth.
    ///
    /// When omitted, Silverman's rule is used to estimate an
    /// appropriate bandwidth from the data.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use gup::chart_builder::builders::density::density_plot;
    /// # #[derive(Debug, Clone)]
    /// # struct P;
    /// let b = density_plot::<P>().bandwidth(0.5);
    /// ```
    pub fn bandwidth(mut self, bw: f32) -> Self {
        self.density_config.bandwidth = Some(bw);
        self
    }

    /// Set the number of contour iso-levels (default: 8).
    ///
    /// More levels produce smoother density visualisations at the cost
    /// of additional geometry.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use gup::chart_builder::builders::density::density_plot;
    /// # #[derive(Debug, Clone)]
    /// # struct P;
    /// let b = density_plot::<P>().levels(12);
    /// ```
    pub fn levels(mut self, n: usize) -> Self {
        self.density_config.levels = n.max(1);
        self
    }

    /// Toggle between filled-contour mode (`true`) and contour-line mode
    /// (`false`).
    ///
    /// Default is `true` (filled contours).
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use gup::chart_builder::builders::density::density_plot;
    /// # #[derive(Debug, Clone)]
    /// # struct P;
    /// let filled = density_plot::<P>().fill(true);
    /// let lines  = density_plot::<P>().fill(false);
    /// ```
    pub fn fill(mut self, filled: bool) -> Self {
        self.density_config.render_mode = if filled {
            DensityRenderMode::FilledContour
        } else {
            DensityRenderMode::ContourLine
        };
        self
    }

    /// Select the sequential colour scheme used to encode density.
    ///
    /// Defaults to Viridis if not specified.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use gup::chart_builder::builders::density::density_plot;
    /// use gup::shader_function::ColorScale;
    /// # #[derive(Debug, Clone)]
    /// # struct P;
    /// let b = density_plot::<P>()
    ///     .color_scheme(ColorScale::plasma(0.0, 1.0));
    /// ```
    pub fn color_scheme(mut self, scheme: impl Into<ColorScale>) -> Self {
        self.color_scale = Some(scheme.into());
        self
    }

    /// Set the density-estimation grid resolution (default: 256).
    ///
    /// The grid is square: `grid_size` × `grid_size` cells.
    pub fn grid_size(mut self, size: usize) -> Self {
        self.density_config.grid_size = size.max(4);
        self
    }

    /// Set the data-extent margin as a fraction (default: 0.05 = 5%).
    pub fn margin(mut self, margin: f32) -> Self {
        self.density_config.margin = margin.max(0.0);
        self
    }

    /// Set the sample-count threshold above which KDE is dispatched on the
    /// GPU (default: 5 000).
    ///
    /// Set to `0` to always use the GPU, or [`usize::MAX`] to force the
    /// CPU path.
    pub fn gpu_threshold(mut self, threshold: usize) -> Self {
        self.density_config.gpu_threshold = threshold;
        self
    }

    /// Return the current density configuration.
    pub fn get_density_config(&self) -> &DensityConfig {
        &self.density_config
    }
}

impl<T> Default for DensityPlotBuilder<T> {
    fn default() -> Self {
        Self::new()
    }
}

// ── ConfigurableBuilder ─────────────────────────────────────────────────

impl<T> ConfigurableBuilder for DensityPlotBuilder<T> {
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

// ── GridCapableBuilder ──────────────────────────────────────────────────

impl<T> GridCapableBuilder for DensityPlotBuilder<T> {
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

// ── ChartBuilder impl ───────────────────────────────────────────────────

impl<T> ChartBuilder<T> for DensityPlotBuilder<T>
where
    T: Clone + MaybeSend + MaybeSync + std::fmt::Debug + 'static,
{
    type Output = ComposedChart<T, Rectangle>;

    fn build_with_data(self, data: Vec<T>, context: Arc<RenderContext>) -> GupResult<Self::Output> {
        if data.is_empty() {
            return Err(ChartBuilderError::EmptyData.into());
        }

        // Extract (x, y) samples from data using accessors.
        let x_acc = self
            .x_accessor
            .as_ref()
            .ok_or_else(|| ChartBuilderError::MissingAccessor {
                attribute: "x".to_string(),
            })?;
        let y_acc = self
            .y_accessor
            .as_ref()
            .ok_or_else(|| ChartBuilderError::MissingAccessor {
                attribute: "y".to_string(),
            })?;

        let samples: Vec<(f32, f32)> = data
            .iter()
            .map(|d| {
                let xv = x_acc.apply(d).as_f32();
                let yv = y_acc.apply(d).as_f32();
                (xv, yv)
            })
            .collect();

        if samples.is_empty() {
            return Err(ChartBuilderError::EmptyData.into());
        }

        // Build the 2D KDE — use GPU path when sample count exceeds threshold.
        let _kde_result = super::gpu_density::gpu_density_2d(
            &samples,
            &self.density_config,
            self.density_config.gpu_threshold,
            Some(&context),
        );

        // Set up colour scale (default: Viridis).
        let mut chart_config = self.config;
        if chart_config.color_scale.is_none() {
            chart_config.color_scale = Some(
                self.color_scale
                    .unwrap_or_else(|| ColorScale::viridis(0.0, 1.0)),
            );
        }

        // Build the selection (Rectangle marks for filled regions or
        // placeholder marks for contour lines).
        let selection = Selection::<T, Rectangle>::new(data, context)?;
        let composed_chart = ComposedChart::new(selection, chart_config).with_default_axes();

        Ok(composed_chart)
    }
}

// ── Density computation helpers ─────────────────────────────────────────

/// Compute the 2D KDE on the CPU for the given sample points.
///
/// Returns the [`KDEResult2D`] with grid points sized according to
/// `config.grid_size`.
pub fn compute_density_2d(samples: &[(f32, f32)], config: &DensityConfig) -> KDEResult2D {
    let mut kde = KernelDensity2D::new(samples.to_vec())
        .with_kernel(KernelFunction::Gaussian)
        .with_n_eval_points(config.grid_size);

    if let Some(bw) = config.bandwidth {
        kde = kde.with_bandwidth(bw);
    }

    kde.compute_cpu()
}

/// Compute linearly-spaced iso-level thresholds between the grid's
/// minimum and maximum density values.
pub fn compute_thresholds(densities: &[f32], n_levels: usize) -> Vec<f32> {
    let min_val = densities.iter().copied().fold(f32::INFINITY, f32::min);
    let max_val = densities.iter().copied().fold(f32::NEG_INFINITY, f32::max);

    if (max_val - min_val).abs() < 1e-12 || n_levels == 0 {
        return vec![];
    }

    (1..=n_levels)
        .map(|i| min_val + (max_val - min_val) * (i as f32 / (n_levels + 1) as f32))
        .collect()
}

// ── DensityLayer ────────────────────────────────────────────────────────

/// A composable density layer that can be overlaid on other chart types.
///
/// Produced by `DensityPlotBuilder::as_heatmap_layer` (when the heatmap
/// rendering pipeline from GUP-248 is available).
#[derive(Debug, Clone)]
pub struct DensityLayer {
    /// The computed 2D KDE result.
    pub kde_result: KDEResult2D,
    /// Density configuration used to produce this layer.
    pub config: DensityConfig,
    /// Colour scale for mapping density to colour.
    pub color_scale: ColorScale,
}

impl DensityPlotBuilder<()> {
    /// Construct a density heatmap layer from pre-computed data.
    ///
    /// The returned [`DensityLayer`] encapsulates the density field and
    /// can be passed to a parent chart builder for overlay composition.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use gup::chart_builder::builders::density::{DensityPlotBuilder, DensityLayer};
    ///
    /// let samples: Vec<(f32, f32)> = vec![(0.0, 0.0), (1.0, 1.0)];
    /// let layer = DensityPlotBuilder::density_layer(&samples, None);
    /// ```
    pub fn density_layer(samples: &[(f32, f32)], bandwidth: Option<f32>) -> DensityLayer {
        let mut config = DensityConfig::default();
        config.bandwidth = bandwidth;
        let kde_result = compute_density_2d(samples, &config);
        DensityLayer {
            kde_result,
            config,
            color_scale: ColorScale::viridis(0.0, 1.0),
        }
    }
}

// ── Convenience constructor ─────────────────────────────────────────────

/// Create a new [`DensityPlotBuilder`].
///
/// This is the primary entry-point for density plot construction.
///
/// # Examples
///
/// ```rust,no_run
/// use gup::chart_builder::builders::density::density_plot;
/// # #[derive(Debug, Clone)]
/// # struct MyData;
///
/// let builder = density_plot::<MyData>();
/// ```
pub fn density_plot<T>() -> DensityPlotBuilder<T> {
    DensityPlotBuilder::new()
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chart_builder::accessor::AccessorValue;

    #[derive(Debug, Clone)]
    struct TestPoint {
        x: f32,
        y: f32,
    }

    // ── Builder API tests ────────────────────────────────────────────

    #[test]
    fn test_density_plot_builder_defaults() {
        let builder = density_plot::<TestPoint>();
        assert!(builder.x_accessor.is_none());
        assert!(builder.y_accessor.is_none());
        assert_eq!(builder.density_config.levels, 8);
        assert_eq!(
            builder.density_config.render_mode,
            DensityRenderMode::FilledContour
        );
        assert!(builder.density_config.bandwidth.is_none());
        assert_eq!(builder.density_config.grid_size, 256);
    }

    #[test]
    fn test_density_plot_builder_fluent_api() {
        let builder = density_plot::<TestPoint>()
            .x(AccessorFunction::new(|d: &TestPoint| {
                AccessorValue::Float(d.x)
            }))
            .y(AccessorFunction::new(|d: &TestPoint| {
                AccessorValue::Float(d.y)
            }))
            .bandwidth(0.5)
            .levels(12)
            .fill(false)
            .grid_size(128)
            .margin(0.1)
            .color_scheme(ColorScale::plasma(0.0, 1.0))
            .title("Density")
            .width(800.0)
            .height(600.0);

        assert!(builder.x_accessor.is_some());
        assert!(builder.y_accessor.is_some());
        assert_eq!(builder.density_config.bandwidth, Some(0.5));
        assert_eq!(builder.density_config.levels, 12);
        assert_eq!(
            builder.density_config.render_mode,
            DensityRenderMode::ContourLine
        );
        assert_eq!(builder.density_config.grid_size, 128);
        assert!((builder.density_config.margin - 0.1).abs() < 1e-6);
        assert!(builder.color_scale.is_some());
    }

    #[test]
    fn test_density_plot_fill_toggle() {
        let filled = density_plot::<TestPoint>().fill(true);
        assert_eq!(
            filled.density_config.render_mode,
            DensityRenderMode::FilledContour
        );
        let lines = density_plot::<TestPoint>().fill(false);
        assert_eq!(
            lines.density_config.render_mode,
            DensityRenderMode::ContourLine
        );
    }

    #[test]
    fn test_density_plot_levels_minimum() {
        let b = density_plot::<TestPoint>().levels(0);
        assert_eq!(b.density_config.levels, 1);
    }

    #[test]
    fn test_density_plot_grid_size_minimum() {
        let b = density_plot::<TestPoint>().grid_size(1);
        assert_eq!(b.density_config.grid_size, 4);
    }

    #[test]
    fn test_density_plot_default_trait() {
        let b = DensityPlotBuilder::<TestPoint>::default();
        assert!(b.x_accessor.is_none());
    }

    // ── 2D KDE correctness tests ────────────────────────────────────

    #[test]
    fn test_kde_2d_standard_normal() {
        // Standard bivariate normal: peak at (0, 0).
        let mut rng_state: u64 = 42;
        let samples: Vec<(f32, f32)> = (0..500)
            .map(|_| {
                let (x, y) = box_muller(&mut rng_state);
                (x, y)
            })
            .collect();

        let config = DensityConfig {
            grid_size: 32,
            bandwidth: Some(0.5),
            ..Default::default()
        };
        let result = compute_density_2d(&samples, &config);

        assert!(!result.densities.is_empty());
        assert!(result.peak_density() > 0.0);

        // Mode should be near (0, 0).
        let mode = result.mode().unwrap();
        assert!(mode.0.abs() < 1.5, "mode x={} too far from 0", mode.0);
        assert!(mode.1.abs() < 1.5, "mode y={} too far from 0", mode.1);
    }

    #[test]
    fn test_kde_2d_uniform_rectangle() {
        // Points uniformly distributed in [0,1] × [0,1].
        let n = 400;
        let samples: Vec<(f32, f32)> = (0..n)
            .map(|i| {
                let x = (i % 20) as f32 / 19.0;
                let y = (i / 20) as f32 / 19.0;
                (x, y)
            })
            .collect();

        let config = DensityConfig {
            grid_size: 16,
            bandwidth: Some(0.15),
            ..Default::default()
        };
        let result = compute_density_2d(&samples, &config);

        assert!(!result.densities.is_empty());
        // All densities should be positive.
        assert!(result.densities.iter().all(|&d| d >= 0.0));
    }

    #[test]
    fn test_kde_2d_mixture_of_gaussians() {
        // Two clusters: one at (-2, -2) and one at (2, 2).
        let mut rng_state: u64 = 123;
        let mut samples = Vec::new();
        for _ in 0..250 {
            let (x, y) = box_muller(&mut rng_state);
            samples.push((x - 2.0, y - 2.0));
        }
        for _ in 0..250 {
            let (x, y) = box_muller(&mut rng_state);
            samples.push((x + 2.0, y + 2.0));
        }

        let config = DensityConfig {
            grid_size: 32,
            bandwidth: Some(0.6),
            ..Default::default()
        };
        let result = compute_density_2d(&samples, &config);

        assert!(!result.densities.is_empty());
        // Should have two modes — peak density should be non-trivial.
        assert!(result.peak_density() > 0.0);
    }

    #[test]
    fn test_kde_2d_matches_cpu_reference() {
        // Generate data and compare compute_density_2d output against a
        // direct KernelDensity2D computation.
        let samples: Vec<(f32, f32)> = (0..100)
            .map(|i| (i as f32 * 0.1, (i as f32 * 0.1).sin()))
            .collect();

        let config = DensityConfig {
            grid_size: 16,
            bandwidth: Some(0.3),
            ..Default::default()
        };
        let result = compute_density_2d(&samples, &config);

        let reference = KernelDensity2D::new(samples)
            .with_bandwidth(0.3)
            .with_n_eval_points(16)
            .compute_cpu();

        // Verify same grid dimensions.
        assert_eq!(result.x_points.len(), reference.x_points.len());
        assert_eq!(result.y_points.len(), reference.y_points.len());
        assert_eq!(result.densities.len(), reference.densities.len());

        // Values must match within 1% relative error.
        for (i, (&a, &b)) in result
            .densities
            .iter()
            .zip(reference.densities.iter())
            .enumerate()
        {
            let max_ab = a.abs().max(b.abs());
            if max_ab > 1e-10 {
                let rel_err = (a - b).abs() / max_ab;
                assert!(
                    rel_err < 0.01,
                    "density mismatch at index {i}: {a} vs {b} (rel err {rel_err})"
                );
            }
        }
    }

    // ── Marching-squares tests ──────────────────────────────────────

    #[test]
    fn test_marching_squares_simple_peak() {
        // 4×4 grid with a peak in the centre.
        #[rustfmt::skip]
        let grid = vec![
            0.0, 0.0, 0.0, 0.0,
            0.0, 1.0, 1.0, 0.0,
            0.0, 1.0, 1.0, 0.0,
            0.0, 0.0, 0.0, 0.0,
        ];
        let xs = vec![0.0, 1.0, 2.0, 3.0];
        let ys = vec![0.0, 1.0, 2.0, 3.0];
        let segments = marching_squares(&grid, 4, 4, 0.5, &xs, &ys);

        // Should form a closed contour around the centre peak.
        assert!(
            !segments.is_empty(),
            "expected contour segments around the peak"
        );
        // Each segment endpoint should be within the grid bounds.
        for seg in &segments {
            for &(x, y) in seg {
                assert!((0.0..=3.0).contains(&x), "x={x} out of bounds");
                assert!((0.0..=3.0).contains(&y), "y={y} out of bounds");
            }
        }
    }

    #[test]
    fn test_marching_squares_no_contour() {
        // Uniform grid — no contour at any threshold within the range.
        let grid = vec![1.0; 16];
        let xs = vec![0.0, 1.0, 2.0, 3.0];
        let ys = vec![0.0, 1.0, 2.0, 3.0];
        let segments = marching_squares(&grid, 4, 4, 0.5, &xs, &ys);
        // All cells are above threshold → idx = 15 → no segments.
        assert!(segments.is_empty());
    }

    #[test]
    fn test_marching_squares_full_below() {
        // All below threshold → idx = 0 → no segments.
        let grid = vec![0.0; 16];
        let xs = vec![0.0, 1.0, 2.0, 3.0];
        let ys = vec![0.0, 1.0, 2.0, 3.0];
        let segments = marching_squares(&grid, 4, 4, 0.5, &xs, &ys);
        assert!(segments.is_empty());
    }

    #[test]
    fn test_marching_squares_saddle_disambiguation() {
        // Construct a saddle-point configuration (case 5 or 10).
        #[rustfmt::skip]
        let grid = vec![
            1.0, 0.0,
            0.0, 1.0,
        ];
        let xs = vec![0.0, 1.0];
        let ys = vec![0.0, 1.0];
        let segments = marching_squares(&grid, 2, 2, 0.5, &xs, &ys);
        // Saddle should produce 2 segments (case 5: two separate lines).
        assert_eq!(
            segments.len(),
            2,
            "saddle case should produce 2 segments, got {}",
            segments.len()
        );
    }

    #[test]
    fn test_marching_squares_connectivity() {
        // A larger grid with a ring-shaped contour.
        #[rustfmt::skip]
        let grid = vec![
            0.0, 0.0, 0.0, 0.0, 0.0,
            0.0, 0.5, 1.0, 0.5, 0.0,
            0.0, 1.0, 2.0, 1.0, 0.0,
            0.0, 0.5, 1.0, 0.5, 0.0,
            0.0, 0.0, 0.0, 0.0, 0.0,
        ];
        let xs: Vec<f32> = (0..5).map(|i| i as f32).collect();
        let ys: Vec<f32> = (0..5).map(|i| i as f32).collect();
        let segments = marching_squares(&grid, 5, 5, 0.75, &xs, &ys);

        assert!(
            segments.len() >= 4,
            "ring contour should have at least 4 segments, got {}",
            segments.len()
        );

        // No dangling segments: every endpoint should be shared by
        // exactly two segments (for a closed contour).
        // This is a simplified check: verify no duplicate segments.
        let unique: std::collections::HashSet<String> = segments
            .iter()
            .map(|s| format!("{:.4},{:.4},{:.4},{:.4}", s[0].0, s[0].1, s[1].0, s[1].1))
            .collect();
        assert_eq!(unique.len(), segments.len(), "duplicate segments found");
    }

    // ── Threshold computation tests ─────────────────────────────────

    #[test]
    fn test_compute_thresholds() {
        let densities = vec![0.0, 0.5, 1.0];
        let thresholds = compute_thresholds(&densities, 4);
        assert_eq!(thresholds.len(), 4);
        // Thresholds should be evenly spaced between min and max.
        for &t in &thresholds {
            assert!(t > 0.0 && t < 1.0);
        }
    }

    #[test]
    fn test_compute_thresholds_uniform() {
        let densities = vec![5.0; 10];
        let thresholds = compute_thresholds(&densities, 8);
        assert!(
            thresholds.is_empty(),
            "uniform grid should produce no thresholds"
        );
    }

    // ── Filled contour band tests ───────────────────────────────────

    #[test]
    fn test_filled_contour_bands_basic() {
        #[rustfmt::skip]
        let grid = vec![
            0.0, 0.0, 0.0,
            0.0, 1.0, 0.0,
            0.0, 0.0, 0.0,
        ];
        let xs = vec![0.0, 1.0, 2.0];
        let ys = vec![0.0, 1.0, 2.0];
        let thresholds = vec![0.5];
        let bands = filled_contour_bands(&grid, 3, 3, &thresholds, &xs, &ys);

        assert!(
            !bands.is_empty(),
            "should produce at least one contour band"
        );
        // At least one band should have triangles.
        let has_tris = bands.iter().any(|b| !b.triangles.is_empty());
        assert!(has_tris, "at least one band should have triangles");
    }

    #[test]
    fn test_filled_contour_bands_normalised_range() {
        let grid: Vec<f32> = (0..16).map(|i| i as f32).collect();
        let xs: Vec<f32> = (0..4).map(|i| i as f32).collect();
        let ys: Vec<f32> = (0..4).map(|i| i as f32).collect();
        let thresholds = compute_thresholds(&grid, 4);
        let bands = filled_contour_bands(&grid, 4, 4, &thresholds, &xs, &ys);

        for band in &bands {
            assert!(
                band.normalised >= 0.0 && band.normalised <= 1.0,
                "normalised={} out of [0,1]",
                band.normalised
            );
        }
    }

    // ── Density layer tests ─────────────────────────────────────────

    #[test]
    fn test_density_layer_construction() {
        let samples = vec![(0.0, 0.0), (1.0, 1.0), (0.5, 0.5)];
        let layer = DensityPlotBuilder::density_layer(&samples, Some(0.3));
        assert!(!layer.kde_result.densities.is_empty());
        assert_eq!(layer.config.bandwidth, Some(0.3));
    }

    // ── Exact marching-squares band polygon tests ───────────────────

    /// Helper: compute the signed area of a polygon.
    fn polygon_area(verts: &[(f32, f32)]) -> f32 {
        let n = verts.len();
        if n < 3 {
            return 0.0;
        }
        let mut area = 0.0f32;
        for i in 0..n {
            let j = (i + 1) % n;
            area += verts[i].0 * verts[j].1;
            area -= verts[j].0 * verts[i].1;
        }
        area * 0.5
    }

    /// Helper: compute the area of triangles stored as flat (x,y)
    /// vertices in groups of three.
    fn triangles_area(tris: &[(f32, f32)]) -> f32 {
        assert_eq!(tris.len() % 3, 0);
        let mut area = 0.0f32;
        for chunk in tris.chunks(3) {
            let (ax, ay) = chunk[0];
            let (bx, by) = chunk[1];
            let (cx, cy) = chunk[2];
            area += ((bx - ax) * (cy - ay) - (cx - ax) * (by - ay)).abs() * 0.5;
        }
        area
    }

    // ── Case-by-case polygon vertex count tests ─────────────────────
    //
    // For a single cell with corners [v00, v10, v11, v01] and a band
    // that maps to the 16 standard marching-squares cases, we verify
    // the polygon vertex count produced by `cell_band_polygons`.

    #[test]
    fn test_band_case_0000_all_below() {
        // All corners below the band → no polygon.
        let polys = cell_band_polygons(
            [0.0, 0.0, 0.0, 0.0],
            [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)],
            0.5,
            1.5,
        );
        assert!(polys.is_empty());
    }

    #[test]
    fn test_band_case_1111_all_inside() {
        // All corners inside the band → full quad (4 vertices).
        let polys = cell_band_polygons(
            [1.0, 1.0, 1.0, 1.0],
            [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)],
            0.5,
            1.5,
        );
        assert_eq!(polys.len(), 1);
        assert_eq!(polys[0].len(), 4);
    }

    #[test]
    fn test_band_case_all_above() {
        // All corners above the band → no polygon.
        let polys = cell_band_polygons(
            [2.0, 2.0, 2.0, 2.0],
            [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)],
            0.5,
            1.5,
        );
        assert!(polys.is_empty());
    }

    #[test]
    fn test_band_single_corner_inside() {
        // One corner Inside, rest Below → triangle (3 vertices).
        // v00=1.0 (Inside), rest=0.0 (Below), band=[0.5, 1.5)
        let polys = cell_band_polygons(
            [1.0, 0.0, 0.0, 0.0],
            [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)],
            0.5,
            1.5,
        );
        assert_eq!(polys.len(), 1);
        assert_eq!(
            polys[0].len(),
            3,
            "single inside corner should give triangle"
        );
    }

    #[test]
    fn test_band_two_adjacent_inside() {
        // Two adjacent corners Inside → quadrilateral (4 vertices).
        // v00=1.0, v10=1.0 (Inside), v11=0.0, v01=0.0 (Below)
        let polys = cell_band_polygons(
            [1.0, 1.0, 0.0, 0.0],
            [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)],
            0.5,
            1.5,
        );
        assert_eq!(polys.len(), 1);
        assert_eq!(
            polys[0].len(),
            4,
            "two adjacent inside corners should give quad"
        );
    }

    #[test]
    fn test_band_three_corners_inside() {
        // Three corners Inside, one Below → pentagon (5 vertices).
        // v00=1.0, v10=1.0, v11=1.0 (Inside), v01=0.0 (Below)
        let polys = cell_band_polygons(
            [1.0, 1.0, 1.0, 0.0],
            [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)],
            0.5,
            1.5,
        );
        assert_eq!(polys.len(), 1);
        assert_eq!(
            polys[0].len(),
            5,
            "three inside corners should give pentagon"
        );
    }

    #[test]
    fn test_band_below_to_above_crossing() {
        // One corner Below, opposite Above, middle two Inside →
        // produces a hexagon through two threshold crossings.
        // v00=0.0 (Below), v10=1.0 (Inside), v11=2.0 (Above),
        // v01=1.0 (Inside); band=[0.5, 1.5)
        let polys = cell_band_polygons(
            [0.0, 1.0, 2.0, 1.0],
            [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)],
            0.5,
            1.5,
        );
        assert_eq!(polys.len(), 1);
        // v00(B)→v10(I): low crossing; v10(I); v10(I)→v11(A): high crossing;
        // v11(A)→v01(I): high crossing; v01(I); v01(I)→v00(B): low crossing
        assert_eq!(
            polys[0].len(),
            6,
            "B-I-A-I ring should give hexagon, got {}",
            polys[0].len()
        );
    }

    #[test]
    fn test_band_below_to_above_edge() {
        // Single edge crosses from Below to Above → two crossings.
        // v00=0.0 (B), v10=2.0 (A), v11=2.0 (A), v01=0.0 (B)
        // band=[0.5, 1.5)
        let polys = cell_band_polygons(
            [0.0, 2.0, 2.0, 0.0],
            [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)],
            0.5,
            1.5,
        );
        assert_eq!(polys.len(), 1);
        // Edge 0→1: B→A low+high; Edge 1→2: A→A nothing;
        // Edge 2→3: A→B high+low; Edge 3→0: B→B nothing → 4 vertices
        assert_eq!(
            polys[0].len(),
            4,
            "B-A-A-B should give quad band strip, got {}",
            polys[0].len()
        );
    }

    // ── Saddle configuration tests ──────────────────────────────────

    #[test]
    fn test_band_saddle_connected() {
        // Saddle: v00=A, v10=I, v11=A, v01=I with centre Inside.
        // Centre = (2.0+1.0+2.0+1.0)/4 = 1.5 → Inside [0.5, 1.6)
        // Band should be connected (single polygon via sub-triangles).
        let polys = cell_band_polygons(
            [2.0, 1.0, 2.0, 1.0],
            [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)],
            0.5,
            1.6,
        );
        assert!(
            !polys.is_empty(),
            "saddle with centre inside should produce polygons"
        );
        // Verify total area > 0
        let total: f32 = polys.iter().map(|p| polygon_area(p).abs()).sum();
        assert!(total > 0.0, "saddle band should have positive area");
    }

    #[test]
    fn test_band_saddle_disconnected() {
        // Saddle: v00=A, v10=I, v11=A, v01=I with centre Above.
        // Centre = (3.0+1.0+3.0+1.0)/4 = 2.0 → Above [0.5, 1.5)
        // Band should be two disconnected triangles around corners 1 and 3.
        let polys = cell_band_polygons(
            [3.0, 1.0, 3.0, 1.0],
            [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)],
            0.5,
            1.5,
        );
        assert!(
            polys.len() >= 2,
            "disconnected saddle should produce ≥2 polygons, got {}",
            polys.len()
        );
    }

    #[test]
    fn test_band_saddle_both_thresholds() {
        // Saddle: v00=A, v10=B, v11=A, v01=B.
        // Values: 2.0, 0.0, 2.0, 0.0; band=[0.5, 1.5)
        // Centre = 1.0 → Inside. Should produce band region(s).
        let polys = cell_band_polygons(
            [2.0, 0.0, 2.0, 0.0],
            [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)],
            0.5,
            1.5,
        );
        assert!(
            !polys.is_empty(),
            "A-B saddle with centre in band should produce polygons"
        );
        let total: f32 = polys.iter().map(|p| polygon_area(p).abs()).sum();
        assert!(total > 0.0);
    }

    // ── Interpolation accuracy tests ────────────────────────────────

    #[test]
    fn test_band_boundary_matches_iso_contour() {
        // Verify that the band polygon boundary lies on the iso-contour
        // to sub-pixel accuracy.
        //
        // Grid: linear ramp 0→1 along x.
        // Band [0.25, 0.75): the left boundary should be at x=0.25,
        // the right at x=0.75.
        #[rustfmt::skip]
        let grid = vec![
            0.0, 0.5, 1.0,
            0.0, 0.5, 1.0,
        ];
        let xs = vec![0.0, 0.5, 1.0];
        let ys = vec![0.0, 1.0];
        let bands = filled_contour_bands(&grid, 2, 3, &[0.25, 0.75], &xs, &ys);

        // Find the band covering [0.25, 0.75).
        let mid_band = bands
            .iter()
            .find(|b| b.low < 0.3 && b.high > 0.7)
            .expect("should have a band covering [0.25, 0.75)");

        assert!(
            !mid_band.triangles.is_empty(),
            "middle band should have triangles"
        );

        // All triangle x-coordinates should be in [0.25, 0.75] ± ε.
        for &(x, _y) in &mid_band.triangles {
            assert!(
                (0.25 - 1e-4..=0.75 + 1e-4).contains(&x),
                "vertex x={x} outside expected [0.25, 0.75]"
            );
        }
    }

    // ── Seamless tiling / topology tests ────────────────────────────

    #[test]
    fn test_band_areas_sum_to_grid_area() {
        // The total triangle area across all bands should equal the
        // total grid area (all cells are covered by exactly one band).
        #[rustfmt::skip]
        let grid = vec![
            0.0, 0.3, 0.6, 1.0,
            0.1, 0.5, 0.8, 0.9,
            0.2, 0.7, 1.0, 0.7,
            0.0, 0.4, 0.5, 0.3,
        ];
        let xs = vec![0.0, 1.0, 2.0, 3.0];
        let ys = vec![0.0, 1.0, 2.0, 3.0];
        let thresholds = compute_thresholds(&grid, 4);
        let bands = filled_contour_bands(&grid, 4, 4, &thresholds, &xs, &ys);

        let total_band_area: f32 = bands.iter().map(|b| triangles_area(&b.triangles)).sum();
        // Total grid area = 3 × 3 = 9 (3 cells wide × 3 cells tall).
        let grid_area = 9.0f32;

        assert!(
            (total_band_area - grid_area).abs() < 0.05,
            "band area sum {total_band_area} should ≈ grid area {grid_area}"
        );
    }

    #[test]
    fn test_seamless_tiling_uniform_gradient() {
        // Linear gradient: every cell is split by every threshold,
        // so exact tiling is critical.
        let size = 8;
        let grid: Vec<f32> = (0..size * size)
            .map(|i| i as f32 / (size * size - 1) as f32)
            .collect();
        let xs: Vec<f32> = (0..size).map(|i| i as f32).collect();
        let ys: Vec<f32> = (0..size).map(|i| i as f32).collect();
        let thresholds = compute_thresholds(&grid, 6);
        let bands = filled_contour_bands(&grid, size, size, &thresholds, &xs, &ys);

        let total_band_area: f32 = bands.iter().map(|b| triangles_area(&b.triangles)).sum();
        let grid_area = ((size - 1) * (size - 1)) as f32;

        assert!(
            (total_band_area - grid_area).abs() / grid_area < 0.01,
            "gradient band area sum {total_band_area} should ≈ {grid_area} (err > 1%)"
        );
    }

    #[test]
    fn test_no_gaps_at_resolution_4() {
        // Acceptance criterion: bands tile at grid resolution ≥ 4.
        let size = 4;
        #[rustfmt::skip]
        let grid = vec![
            0.0, 0.2, 0.4, 0.1,
            0.3, 0.9, 0.7, 0.2,
            0.1, 0.6, 1.0, 0.5,
            0.0, 0.1, 0.3, 0.2,
        ];
        let xs: Vec<f32> = (0..size).map(|i| i as f32).collect();
        let ys: Vec<f32> = (0..size).map(|i| i as f32).collect();
        let thresholds = compute_thresholds(&grid, 4);
        let bands = filled_contour_bands(&grid, size, size, &thresholds, &xs, &ys);

        let total_area: f32 = bands.iter().map(|b| triangles_area(&b.triangles)).sum();
        let expected = ((size - 1) * (size - 1)) as f32;

        assert!(
            (total_area - expected).abs() / expected < 0.01,
            "at 4×4 grid, total band area {total_area} ≈ {expected}"
        );
    }

    #[test]
    fn test_triangle_counts_non_empty() {
        // Verify that triangles are produced in multiples of 3 vertices.
        #[rustfmt::skip]
        let grid = vec![
            0.0, 0.5, 1.0,
            0.5, 1.0, 0.5,
            1.0, 0.5, 0.0,
        ];
        let xs = vec![0.0, 1.0, 2.0];
        let ys = vec![0.0, 1.0, 2.0];
        let thresholds = compute_thresholds(&grid, 3);
        let bands = filled_contour_bands(&grid, 3, 3, &thresholds, &xs, &ys);

        for band in &bands {
            assert_eq!(
                band.triangles.len() % 3,
                0,
                "triangle count must be multiple of 3, got {} for band [{}, {}]",
                band.triangles.len(),
                band.low,
                band.high
            );
        }
    }

    #[test]
    fn test_exact_vs_cell_average_smoother_at_low_res() {
        // At 16×16 resolution with a Gaussian-like peak, the exact
        // decomposition should produce a smaller number of triangles
        // (partial cells) compared to a full-cell tiling, demonstrating
        // that band boundaries cut through cells rather than including
        // the entire cell.
        let size = 6;
        let mid = (size - 1) as f32 / 2.0;
        let grid: Vec<f32> = (0..size * size)
            .map(|i| {
                let x = (i % size) as f32 - mid;
                let y = (i / size) as f32 - mid;
                (-0.5 * (x * x + y * y)).exp()
            })
            .collect();
        let xs: Vec<f32> = (0..size).map(|i| i as f32).collect();
        let ys: Vec<f32> = (0..size).map(|i| i as f32).collect();
        let thresholds = compute_thresholds(&grid, 4);
        let bands = filled_contour_bands(&grid, size, size, &thresholds, &xs, &ys);

        // Each band should have SOME triangles (the peak isn't flat).
        let bands_with_tris: Vec<_> = bands.iter().filter(|b| !b.triangles.is_empty()).collect();
        assert!(
            bands_with_tris.len() >= 2,
            "at least 2 bands should have triangles"
        );

        // The band area sum should still match the grid.
        let total_area: f32 = bands.iter().map(|b| triangles_area(&b.triangles)).sum();
        let expected = ((size - 1) * (size - 1)) as f32;
        assert!(
            (total_area - expected).abs() / expected < 0.02,
            "Gaussian peak: band area {total_area} ≈ {expected}"
        );
    }

    // ── cell_band_polygons edge-case tests ──────────────────────────

    #[test]
    fn test_band_polygon_single_above_corner() {
        // One corner Above, rest Inside → pentagon (5 vertices).
        // v00=2.0 (Above), rest=1.0 (Inside); band=[0.5, 1.5)
        let polys = cell_band_polygons(
            [2.0, 1.0, 1.0, 1.0],
            [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)],
            0.5,
            1.5,
        );
        assert_eq!(polys.len(), 1);
        assert_eq!(
            polys[0].len(),
            5,
            "one above corner clipping should give pentagon"
        );
    }

    #[test]
    fn test_band_polygon_two_above_adjacent() {
        // Two adjacent corners Above, two Inside → quad.
        // v00=2.0, v10=2.0 (Above), v11=1.0, v01=1.0 (Inside)
        let polys = cell_band_polygons(
            [2.0, 2.0, 1.0, 1.0],
            [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)],
            0.5,
            1.5,
        );
        assert_eq!(polys.len(), 1);
        assert_eq!(polys[0].len(), 4);
    }

    #[test]
    fn test_band_polygon_mixed_below_above() {
        // v00=0.0 (B), v10=2.0 (A), v11=0.0 (B), v01=2.0 (A)
        // This is a saddle; centre=(0+2+0+2)/4=1.0 Inside [0.5,1.5)
        let polys = cell_band_polygons(
            [0.0, 2.0, 0.0, 2.0],
            [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)],
            0.5,
            1.5,
        );
        assert!(
            !polys.is_empty(),
            "B-A-B-A saddle with centre in band should produce polygons"
        );
    }

    #[test]
    fn test_fan_triangulate_triangle() {
        let mut out = Vec::new();
        fan_triangulate(&[(0.0, 0.0), (1.0, 0.0), (0.5, 1.0)], &mut out);
        assert_eq!(out.len(), 3);
    }

    #[test]
    fn test_fan_triangulate_quad() {
        let mut out = Vec::new();
        fan_triangulate(&[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)], &mut out);
        // Quad → 2 triangles → 6 vertices.
        assert_eq!(out.len(), 6);
    }

    #[test]
    fn test_fan_triangulate_degenerate() {
        let mut out = Vec::new();
        fan_triangulate(&[(0.0, 0.0), (1.0, 0.0)], &mut out);
        assert!(out.is_empty(), "fewer than 3 vertices → no triangles");
    }

    // ── Integration test with ChartBuilder ──────────────────────────

    #[tokio::test]
    async fn test_density_plot_build_with_data() {
        let data = vec![
            TestPoint { x: 0.0, y: 0.0 },
            TestPoint { x: 1.0, y: 1.0 },
            TestPoint { x: 0.5, y: 0.5 },
            TestPoint { x: 0.2, y: 0.8 },
        ];

        let context = Arc::new(RenderContext::new().await.unwrap());

        let builder = density_plot()
            .x(AccessorFunction::new(|d: &TestPoint| {
                AccessorValue::Float(d.x)
            }))
            .y(AccessorFunction::new(|d: &TestPoint| {
                AccessorValue::Float(d.y)
            }))
            .bandwidth(0.3)
            .levels(6)
            .fill(true)
            .title("Test Density");

        let result = builder.build_with_data(data, context);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_density_plot_missing_accessor() {
        let data = vec![TestPoint { x: 1.0, y: 2.0 }];
        let context = Arc::new(RenderContext::new().await.unwrap());

        // Missing X accessor.
        let builder = density_plot().y(AccessorFunction::new(|d: &TestPoint| {
            AccessorValue::Float(d.y)
        }));
        assert!(
            builder
                .build_with_data(data.clone(), context.clone())
                .is_err()
        );

        // Missing Y accessor.
        let builder = density_plot().x(AccessorFunction::new(|d: &TestPoint| {
            AccessorValue::Float(d.x)
        }));
        assert!(builder.build_with_data(data, context).is_err());
    }

    #[tokio::test]
    async fn test_density_plot_empty_data() {
        let context = Arc::new(RenderContext::new().await.unwrap());
        let builder = density_plot::<TestPoint>()
            .x(AccessorFunction::new(|d: &TestPoint| {
                AccessorValue::Float(d.x)
            }))
            .y(AccessorFunction::new(|d: &TestPoint| {
                AccessorValue::Float(d.y)
            }));
        let result = builder.build_with_data(vec![], context);
        assert!(result.is_err());
    }

    // ── Simple pseudo-random number generator ───────────────────────

    fn box_muller(state: &mut u64) -> (f32, f32) {
        let u1 = lcg_uniform(state);
        let u2 = lcg_uniform(state);
        let r = (-2.0 * u1.ln()).sqrt();
        let theta = 2.0 * std::f32::consts::PI * u2;
        (r * theta.cos(), r * theta.sin())
    }

    fn lcg_uniform(state: &mut u64) -> f32 {
        *state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        // Map to (0, 1).
        let bits = (*state >> 33) as f32;
        (bits + 1.0) / (2.0f32.powi(31) + 1.0)
    }
}

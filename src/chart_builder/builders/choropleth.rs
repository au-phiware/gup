// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Choropleth map builder.
//!
//! Provides a fluent API for creating GPU-accelerated choropleth maps from
//! GeoJSON boundary data and a numeric value mapping. Each geographic region
//! is filled with a colour derived from its associated value via a
//! [`ColorScale`].
//!
//! # Example
//!
//! ```rust,ignore
//! use gup::chart_builder::builders::choropleth::ChoroplethChartBuilder;
//! use gup::mark::geo_path::{GeoJsonSource, Projection};
//! use gup::shader_function::ColorScale;
//!
//! let source = GeoJsonSource::from_str(geojson_str)?;
//! let chart = ChoroplethChartBuilder::new()
//!     .boundaries(source)
//!     .data(vec![("USA", 331_000_000.0), ("CHN", 1_412_000_000.0)])
//!     .region_id(|f| {
//!         f.properties
//!             .as_ref()
//!             .and_then(|p| p.get("iso_a3"))
//!             .and_then(|v| v.as_str())
//!             .map(String::from)
//!     })
//!     .color_scale(ColorScale::viridis(0.0, 1_500_000_000.0))
//!     .projection(Projection::Mercator)
//!     .build()?;
//! ```

use std::collections::HashMap;

use crate::error::{GupError, GupResult};
use crate::mark::geo_path::{GeoFeature, GeoJsonSource, Projection};
use crate::shader_function::ColorScale;

// ---------------------------------------------------------------------------
// Legend position
// ---------------------------------------------------------------------------

/// Position of the colour legend relative to the map.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LegendPosition {
    /// Horizontal bar below the map (default).
    #[default]
    Bottom,
    /// Horizontal bar above the map.
    Top,
    /// Vertical bar to the right of the map.
    Right,
    /// Vertical bar to the left of the map.
    Left,
}

// ---------------------------------------------------------------------------
// Hover highlight style
// ---------------------------------------------------------------------------

/// Visual highlight style applied to the hovered choropleth region.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HoverHighlight {
    /// Brighten the fill colour by a factor (e.g. `0.3` = +30% brightness).
    Brighten(f32),
    /// Reduce opacity of *non-hovered* regions by a factor (e.g. `0.4`).
    Dim(f32),
    /// No visual highlighting.
    None,
}

impl Default for HoverHighlight {
    fn default() -> Self {
        Self::Brighten(0.3)
    }
}

// ---------------------------------------------------------------------------
// GPU-side recolouring WGSL shaders
// ---------------------------------------------------------------------------

/// Vertex shader source for GPU-side choropleth recolouring.
///
/// This shader reads per-vertex `(lonlat, region_index)` and looks up
/// the fill colour from a `storage` buffer of `vec4<f32>` region colours
/// bound at `@group(0) @binding(1)`.
pub const RECOLOR_VERTEX_SHADER: &str =
    include_str!("../../mark/shaders/choropleth_recolor.vert.wgsl");

/// Fragment shader source for GPU-side choropleth recolouring.
///
/// Identical in structure to the standard `geo_path` fragment shader —
/// selects fill or stroke colour based on `edge_flag`.
pub const RECOLOR_FRAGMENT_SHADER: &str =
    include_str!("../../mark/shaders/choropleth_recolor.frag.wgsl");

// ---------------------------------------------------------------------------
// Choropleth vertex
// ---------------------------------------------------------------------------

/// Per-vertex data for choropleth rendering (position + colour).
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct ChoroplethVertex {
    /// Longitude, latitude in degrees (projected in vertex shader).
    pub position: [f32; 2],
    /// Fill colour for this vertex (RGBA).
    pub color: [f32; 4],
}

// ---------------------------------------------------------------------------
// GPU-side recolouring vertex
// ---------------------------------------------------------------------------

/// Per-vertex data for GPU-side recolouring (position + region index).
///
/// Unlike [`ChoroplethVertex`] which bakes the colour into each vertex, this
/// variant stores only the region index. The fragment shader reads the actual
/// colour from a storage buffer ([`RegionColorBuffer`]) using this index.
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct IndexedChoroplethVertex {
    /// Longitude, latitude in degrees (projected in vertex shader).
    pub position: [f32; 2],
    /// Index into the [`RegionColorBuffer`] colour array.
    pub region_index: u32,
}

// ---------------------------------------------------------------------------
// RegionColorBuffer
// ---------------------------------------------------------------------------

/// A CPU-side colour buffer for per-region RGBA colours, indexed by feature
/// index.
///
/// This is the core data structure for GPU-side choropleth recolouring.
/// Instead of baking colours into every vertex, the colours are stored in a
/// flat array that mirrors a GPU storage buffer. The fragment shader reads
/// the colour for each fragment by indexing into this buffer using the
/// per-vertex `region_index`.
///
/// # Usage
///
/// ```rust,ignore
/// let buffer = RegionColorBuffer::new(region_count, no_data_color);
/// buffer.set_color(0, [1.0, 0.0, 0.0, 1.0]); // Region 0 → red
///
/// // Recolour from new data without re-tessellation:
/// buffer.update_from_data(&data, &regions, &color_scale);
///
/// // On the GPU side, call buffer.as_bytes() to write to the storage buffer.
/// ```
#[derive(Debug, Clone)]
pub struct RegionColorBuffer {
    /// Per-region RGBA colours, indexed by feature index.
    colors: Vec<[f32; 4]>,
    /// Fallback colour for regions with no data.
    no_data_color: [f32; 4],
}

impl RegionColorBuffer {
    /// Create a new buffer with `count` regions, all initialised to
    /// `no_data_color`.
    pub fn new(count: usize, no_data_color: [f32; 4]) -> Self {
        Self {
            colors: vec![no_data_color; count],
            no_data_color,
        }
    }

    /// Create a buffer from existing region records (copies each region's
    /// current colour).
    pub fn from_regions(regions: &[RegionRecord], no_data_color: [f32; 4]) -> Self {
        Self {
            colors: regions.iter().map(|r| r.color).collect(),
            no_data_color,
        }
    }

    /// Number of regions in the buffer.
    pub fn len(&self) -> usize {
        self.colors.len()
    }

    /// Returns `true` if the buffer contains no regions.
    pub fn is_empty(&self) -> bool {
        self.colors.is_empty()
    }

    /// Get the colour for a region by index.
    pub fn color(&self, index: usize) -> Option<&[f32; 4]> {
        self.colors.get(index)
    }

    /// Set the colour for a single region by index.
    pub fn set_color(&mut self, index: usize, color: [f32; 4]) {
        if index < self.colors.len() {
            self.colors[index] = color;
        }
    }

    /// Get the no-data fallback colour.
    pub fn no_data_color(&self) -> [f32; 4] {
        self.no_data_color
    }

    /// Recompute all region colours from a new data map and colour scale.
    ///
    /// This is the main method for dynamic recolouring: pass new data values
    /// and the region records (for ID lookup) and the buffer is updated
    /// in-place. No geometry is re-tessellated.
    pub fn update_from_data(
        &mut self,
        data: &HashMap<String, f64>,
        regions: &[RegionRecord],
        color_scale: &ColorScale,
    ) {
        let (domain_min, domain_max) = if data.is_empty() {
            (0.0_f64, 1.0_f64)
        } else {
            let mut min_val = f64::INFINITY;
            let mut max_val = f64::NEG_INFINITY;
            for &v in data.values() {
                if v < min_val {
                    min_val = v;
                }
                if v > max_val {
                    max_val = v;
                }
            }
            (min_val, max_val)
        };

        for region in regions {
            let color = region
                .id
                .as_ref()
                .and_then(|id| data.get(id))
                .map(|&v| sample_color_scale(color_scale, v, domain_min, domain_max))
                .unwrap_or(self.no_data_color);
            if region.feature_index < self.colors.len() {
                self.colors[region.feature_index] = color;
            }
        }
    }

    /// Linearly interpolate between the current buffer and a target buffer.
    ///
    /// `t` should be in `[0.0, 1.0]` where 0.0 returns the current colours
    /// and 1.0 returns the target colours. This enables smooth animated
    /// colour transitions between datasets.
    pub fn interpolate(&self, target: &RegionColorBuffer, t: f32) -> RegionColorBuffer {
        let t = t.clamp(0.0, 1.0);
        let count = self.colors.len().min(target.colors.len());
        let mut result = RegionColorBuffer::new(count, self.no_data_color);
        for i in 0..count {
            let [r0, g0, b0, a0] = self.colors[i];
            let [r1, g1, b1, a1] = target.colors[i];
            result.colors[i] = [
                r0 + (r1 - r0) * t,
                g0 + (g1 - g0) * t,
                b0 + (b1 - b0) * t,
                a0 + (a1 - a0) * t,
            ];
        }
        result
    }

    /// Return the colour data as a byte slice suitable for
    /// `queue.write_buffer()`.
    pub fn as_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(&self.colors)
    }

    /// Return a reference to the raw colour array.
    pub fn colors(&self) -> &[[f32; 4]] {
        &self.colors
    }
}

// ---------------------------------------------------------------------------
// Region record produced by the data join
// ---------------------------------------------------------------------------

/// Resolved data for a single geographic region after the data join.
#[derive(Debug, Clone)]
pub struct RegionRecord {
    /// Identifier extracted from the GeoJSON feature.
    pub id: Option<String>,
    /// Numeric value (if found in the data map).
    pub value: Option<f64>,
    /// Computed fill colour.
    pub color: [f32; 4],
    /// Index of the source `GeoFeature` in the boundary data.
    pub feature_index: usize,
}

// ---------------------------------------------------------------------------
// ChoroplethChart (the built product)
// ---------------------------------------------------------------------------

/// A fully resolved choropleth map ready for rendering.
///
/// Produced by [`ChoroplethChartBuilder::build`]. The chart holds
/// pre-tessellated, per-vertex coloured geometry for the fill and stroke
/// layers.
pub struct ChoroplethChart {
    /// Tessellated fill triangles with per-vertex colours.
    pub fill_vertices: Vec<ChoroplethVertex>,
    /// Index buffer for the fill triangles.
    pub fill_indices: Vec<u32>,
    /// Tessellated stroke line-list vertices with per-vertex colours.
    pub stroke_vertices: Vec<ChoroplethVertex>,
    /// Per-region data records (for legend, tooltips, etc.).
    pub regions: Vec<RegionRecord>,
    /// The colour scale used.
    pub color_scale: ColorScale,
    /// The projection used.
    pub projection: Projection,
    /// Whether the legend should be shown.
    pub show_legend: bool,
    /// Legend position.
    pub legend_position: LegendPosition,
    /// Domain minimum value.
    pub domain_min: f64,
    /// Domain maximum value.
    pub domain_max: f64,
    /// Whether zoom/pan is enabled.
    pub zoom_enabled: bool,
    /// No-data fallback colour.
    pub no_data_color: [f32; 4],
    /// Stroke colour.
    pub stroke_color: [f32; 4],
    /// Stroke opacity.
    pub stroke_opacity: f32,
    // -- GPU-side recolouring (opt-in) ------------------------------------
    /// When GPU-side recolouring is enabled, fill vertices with a region
    /// index instead of a baked colour. The fragment shader reads the actual
    /// colour from the [`RegionColorBuffer`].
    pub indexed_fill_vertices: Option<Vec<IndexedChoroplethVertex>>,
    /// Per-region colour buffer for GPU-side recolouring.
    ///
    /// Present only when `.gpu_recolor(true)` was called on the builder.
    pub region_color_buffer: Option<RegionColorBuffer>,

    // -- Hover / tooltip interaction --------------------------------------
    /// Whether the tooltip is enabled.
    pub tooltip_enabled: bool,
    /// How the hovered region is visually highlighted.
    pub highlight_style: HoverHighlight,
    /// Projected polygon exterior rings per region for CPU-side hit-testing.
    ///
    /// `region_polygons[region_idx]` is a list of exterior rings (one per
    /// polygon in that feature). Each ring is a list of `[x, y]` positions
    /// in the same coordinate space as the fill vertices.
    pub region_polygons: Vec<Vec<Vec<[f32; 2]>>>,

    /// Custom tooltip formatter closure.
    ///
    /// When `Some`, called with a [`RegionRecord`] reference to produce the
    /// tooltip text. When `None`, a default format is used
    /// (`"<name>: <value>"`).
    #[cfg(not(target_arch = "wasm32"))]
    tooltip_formatter: Option<Box<dyn Fn(&RegionRecord) -> String + Send + Sync>>,
    /// Custom tooltip formatter closure.
    #[cfg(target_arch = "wasm32")]
    tooltip_formatter: Option<Box<dyn Fn(&RegionRecord) -> String>>,
}

impl std::fmt::Debug for ChoroplethChart {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChoroplethChart")
            .field("fill_vertices", &self.fill_vertices.len())
            .field("fill_indices", &self.fill_indices.len())
            .field("stroke_vertices", &self.stroke_vertices.len())
            .field("regions", &self.regions.len())
            .field("tooltip_enabled", &self.tooltip_enabled)
            .field("highlight_style", &self.highlight_style)
            .field("has_tooltip_formatter", &self.tooltip_formatter.is_some())
            .field("region_polygons", &self.region_polygons.len())
            .finish()
    }
}

impl ChoroplethChart {
    /// Update region colours from a new data mapping without
    /// re-tessellating geometry.
    ///
    /// This only works when GPU-side recolouring is enabled (i.e. the chart
    /// was built with `.gpu_recolor(true)`). Returns an error if recolouring
    /// is not enabled.
    ///
    /// After calling this method, use
    /// [`region_color_buffer`](Self::region_color_buffer) to obtain the
    /// updated byte data for `queue.write_buffer()`.
    pub fn update_colors<I, K>(&mut self, new_data: I) -> GupResult<()>
    where
        I: IntoIterator<Item = (K, f64)>,
        K: Into<String>,
    {
        let buffer = self.region_color_buffer.as_mut().ok_or_else(|| {
            GupError::validation_error(
                "ChoroplethChart::update_colors requires GPU-side recolouring \
                 (build with .gpu_recolor(true))",
            )
        })?;

        let data: HashMap<String, f64> = new_data.into_iter().map(|(k, v)| (k.into(), v)).collect();
        buffer.update_from_data(&data, &self.regions, &self.color_scale);

        // Also update the domain bounds from the new data.
        if !data.is_empty() {
            let mut min_val = f64::INFINITY;
            let mut max_val = f64::NEG_INFINITY;
            for &v in data.values() {
                if v < min_val {
                    min_val = v;
                }
                if v > max_val {
                    max_val = v;
                }
            }
            self.domain_min = min_val;
            self.domain_max = max_val;
        }

        // Update per-region value records.
        for region in &mut self.regions {
            region.value = region.id.as_ref().and_then(|id| data.get(id)).copied();
            region.color = buffer
                .color(region.feature_index)
                .copied()
                .unwrap_or(buffer.no_data_color());
        }

        Ok(())
    }

    /// Produce an interpolated [`RegionColorBuffer`] between the current
    /// colours and a target colour set.
    ///
    /// This is useful for animating colour transitions between datasets.
    /// `t` should be in `[0.0, 1.0]`.
    ///
    /// Returns `None` if GPU-side recolouring is not enabled.
    pub fn interpolate_colors(
        &self,
        target: &RegionColorBuffer,
        t: f32,
    ) -> Option<RegionColorBuffer> {
        self.region_color_buffer
            .as_ref()
            .map(|buf| buf.interpolate(target, t))
    }

    // -- Hover / tooltip interaction --------------------------------------

    /// Find which region (if any) contains the given point.
    ///
    /// Uses CPU-side ray-casting point-in-polygon on the projected polygon
    /// rings stored during build. Returns the index into [`regions`](Self::regions).
    pub fn region_at_point(&self, x: f32, y: f32) -> Option<usize> {
        for (region_idx, rings) in self.region_polygons.iter().enumerate() {
            for ring in rings {
                if point_in_ring(x, y, ring) {
                    return Some(region_idx);
                }
            }
        }
        None
    }

    /// Get the formatted tooltip content for a region.
    ///
    /// Returns `None` if the tooltip is disabled or the region index is out
    /// of bounds. When a custom formatter was set via
    /// [`ChoroplethChartBuilder::tooltip_format`], it is used; otherwise the
    /// default format `"<name>: <value>"` is produced.
    pub fn tooltip_content(&self, region_index: usize) -> Option<String> {
        if !self.tooltip_enabled {
            return None;
        }
        let region = self.regions.get(region_index)?;

        if let Some(fmt) = &self.tooltip_formatter {
            Some(fmt(region))
        } else {
            // Default format: "RegionID: value" or just "RegionID" if no data.
            let name = region.id.as_deref().unwrap_or("Unknown");
            match region.value {
                Some(v) => Some(format!("{name}: {v}")),
                None => Some(format!("{name}: no data")),
            }
        }
    }

    /// Compute the highlighted colour for a region given the current hover
    /// state.
    ///
    /// `is_hovered` indicates whether this region is the one under the
    /// pointer. The transformation depends on [`highlight_style`](Self::highlight_style):
    ///
    /// - [`HoverHighlight::Brighten`] — adds a brightness offset to the
    ///   hovered region.
    /// - [`HoverHighlight::Dim`] — reduces alpha of non-hovered regions.
    /// - [`HoverHighlight::None`] — returns the base colour unchanged.
    pub fn highlighted_color(&self, region_index: usize, is_hovered: bool) -> [f32; 4] {
        let base = self
            .regions
            .get(region_index)
            .map(|r| r.color)
            .unwrap_or(self.no_data_color);

        match self.highlight_style {
            HoverHighlight::Brighten(amount) => {
                if is_hovered {
                    [
                        (base[0] + amount).min(1.0),
                        (base[1] + amount).min(1.0),
                        (base[2] + amount).min(1.0),
                        base[3],
                    ]
                } else {
                    base
                }
            }
            HoverHighlight::Dim(factor) => {
                if is_hovered {
                    base
                } else {
                    [base[0], base[1], base[2], base[3] * factor]
                }
            }
            HoverHighlight::None => base,
        }
    }
}

// ---------------------------------------------------------------------------
// ChoroplethChartBuilder
// ---------------------------------------------------------------------------

/// Fluent builder for choropleth maps.
///
/// See the [module-level documentation](self) for usage examples.
pub struct ChoroplethChartBuilder {
    boundaries: Option<GeoJsonSource>,
    data: HashMap<String, f64>,
    #[cfg(not(target_arch = "wasm32"))]
    region_id_fn: Option<Box<dyn Fn(&GeoFeature) -> Option<String> + Send + Sync>>,
    #[cfg(target_arch = "wasm32")]
    region_id_fn: Option<Box<dyn Fn(&GeoFeature) -> Option<String>>>,
    color_scale: Option<ColorScale>,
    projection: Projection,
    no_data_color: [f32; 4],
    stroke_color: [f32; 4],
    stroke_opacity: f32,
    show_legend: bool,
    legend_position: LegendPosition,
    zoom_enabled: bool,
    simplification_tolerance: f32,
    gpu_recolor: bool,
    // -- Hover / tooltip interaction --------------------------------------
    tooltip_enabled: bool,
    #[cfg(not(target_arch = "wasm32"))]
    tooltip_formatter: Option<Box<dyn Fn(&RegionRecord) -> String + Send + Sync>>,
    #[cfg(target_arch = "wasm32")]
    tooltip_formatter: Option<Box<dyn Fn(&RegionRecord) -> String>>,
    highlight_style: HoverHighlight,
}

impl std::fmt::Debug for ChoroplethChartBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChoroplethChartBuilder")
            .field("has_boundaries", &self.boundaries.is_some())
            .field("data_count", &self.data.len())
            .field("has_region_id_fn", &self.region_id_fn.is_some())
            .field("has_color_scale", &self.color_scale.is_some())
            .field("projection", &self.projection)
            .field("no_data_color", &self.no_data_color)
            .field("stroke_color", &self.stroke_color)
            .field("stroke_opacity", &self.stroke_opacity)
            .field("show_legend", &self.show_legend)
            .field("legend_position", &self.legend_position)
            .field("zoom_enabled", &self.zoom_enabled)
            .field("gpu_recolor", &self.gpu_recolor)
            .field("tooltip_enabled", &self.tooltip_enabled)
            .field("has_tooltip_formatter", &self.tooltip_formatter.is_some())
            .field("highlight_style", &self.highlight_style)
            .finish()
    }
}

impl Default for ChoroplethChartBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ChoroplethChartBuilder {
    /// Create a new, empty choropleth builder with sensible defaults.
    pub fn new() -> Self {
        Self {
            boundaries: None,
            data: HashMap::new(),
            region_id_fn: None,
            color_scale: None,
            projection: Projection::Mercator,
            no_data_color: [0.75, 0.75, 0.75, 1.0], // mid-grey
            stroke_color: [1.0, 1.0, 1.0, 0.4],     // thin white, opacity 0.4
            stroke_opacity: 0.4,
            show_legend: true,
            legend_position: LegendPosition::default(),
            zoom_enabled: true,
            simplification_tolerance: 0.0,
            gpu_recolor: false,
            tooltip_enabled: false,
            tooltip_formatter: None,
            highlight_style: HoverHighlight::default(),
        }
    }

    // -- Builder methods ---------------------------------------------------

    /// Set the GeoJSON boundary data.
    pub fn boundaries(mut self, source: GeoJsonSource) -> Self {
        self.boundaries = Some(source);
        self
    }

    /// Load region → value data from an iterator of `(key, value)` pairs.
    pub fn data<I, K>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = (K, f64)>,
        K: Into<String>,
    {
        self.data = values.into_iter().map(|(k, v)| (k.into(), v)).collect();
        self
    }

    /// Load data from a collection of struct records, extracting region
    /// identifiers and values with the provided closures.
    ///
    /// This is an alternative to [`data`](Self::data) for when you have a
    /// collection of typed structs rather than pre-keyed `(key, value)` pairs.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// struct CountryStats { iso: String, population: f64, gdp: f64 }
    ///
    /// ChoroplethChartBuilder::new()
    ///     .data_from_records(
    ///         stats_vec,
    ///         |s| s.iso.clone(),
    ///         |s| s.population,
    ///     )
    /// ```
    pub fn data_from_records<T, I, K, V>(mut self, records: I, key: K, value: V) -> Self
    where
        I: IntoIterator<Item = T>,
        K: Fn(&T) -> String,
        V: Fn(&T) -> f64,
    {
        self.data = records.into_iter().map(|r| (key(&r), value(&r))).collect();
        self
    }

    /// Configure how a region identifier is extracted from each GeoJSON
    /// feature.
    ///
    /// The closure receives a [`GeoFeature`] reference and should return
    /// `Some(id_string)` that matches the keys passed to [`data`](Self::data).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn region_id<F>(mut self, accessor: F) -> Self
    where
        F: Fn(&GeoFeature) -> Option<String> + Send + Sync + 'static,
    {
        self.region_id_fn = Some(Box::new(accessor));
        self
    }

    /// Configure how a region identifier is extracted from each GeoJSON
    /// feature.
    ///
    /// The closure receives a [`GeoFeature`] reference and should return
    /// `Some(id_string)` that matches the keys passed to [`data`](Self::data).
    #[cfg(target_arch = "wasm32")]
    pub fn region_id<F>(mut self, accessor: F) -> Self
    where
        F: Fn(&GeoFeature) -> Option<String> + 'static,
    {
        self.region_id_fn = Some(Box::new(accessor));
        self
    }

    /// Set the colour scale used to map numeric values to fill colours.
    pub fn color_scale(mut self, scale: ColorScale) -> Self {
        self.color_scale = Some(scale);
        self
    }

    /// Set the geographic projection (default: `Projection::Mercator`).
    pub fn projection(mut self, projection: Projection) -> Self {
        self.projection = projection;
        self
    }

    /// Set the fill colour used for regions with no matching data value
    /// (default: mid-grey `[0.75, 0.75, 0.75, 1.0]`).
    pub fn no_data_color(mut self, color: [f32; 4]) -> Self {
        self.no_data_color = color;
        self
    }

    /// Set the stroke (border) colour for polygon boundaries
    /// (default: white at 0.4 opacity).
    pub fn stroke_color(mut self, color: [f32; 4]) -> Self {
        self.stroke_color = color;
        self
    }

    /// Set the stroke opacity (default: 0.4).
    pub fn stroke_opacity(mut self, opacity: f32) -> Self {
        self.stroke_opacity = opacity;
        self.stroke_color[3] = opacity;
        self
    }

    /// Show or hide the colour legend (default: `true`).
    pub fn legend(mut self, show: bool) -> Self {
        self.show_legend = show;
        self
    }

    /// Set the legend position (default: [`LegendPosition::Bottom`]).
    pub fn legend_position(mut self, position: LegendPosition) -> Self {
        self.legend_position = position;
        self
    }

    /// Enable or disable zoom and pan (default: `true`).
    pub fn zoom(mut self, enabled: bool) -> Self {
        self.zoom_enabled = enabled;
        self
    }

    /// Set simplification tolerance in degrees (0.0 = no simplification).
    pub fn simplification_tolerance(mut self, tolerance: f32) -> Self {
        self.simplification_tolerance = tolerance;
        self
    }

    /// Enable GPU-side recolouring (default: `false`).
    ///
    /// When enabled, the chart additionally produces
    /// [`IndexedChoroplethVertex`] data (with per-vertex region indices) and
    /// a [`RegionColorBuffer`]. The fragment shader can then read region
    /// colours from a storage buffer, allowing dynamic recolouring via
    /// [`ChoroplethChart::update_colors`] without re-tessellating geometry.
    ///
    /// The existing CPU-side per-vertex coloured geometry
    /// ([`ChoroplethVertex`]) is **always** produced regardless of this
    /// setting, so the caller can choose which rendering path to use.
    pub fn gpu_recolor(mut self, enabled: bool) -> Self {
        self.gpu_recolor = enabled;
        self
    }

    // -- Hover / tooltip interaction --------------------------------------

    /// Enable or disable the tooltip (default: `false`).
    ///
    /// When enabled, [`ChoroplethChart::tooltip_content`] will return
    /// formatted text for a given region.
    pub fn tooltip(mut self, enabled: bool) -> Self {
        self.tooltip_enabled = enabled;
        self
    }

    /// Set a custom tooltip format closure.
    ///
    /// The closure receives a [`RegionRecord`] reference and should return
    /// the full tooltip string. Implicitly enables the tooltip.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// builder.tooltip_format(|region| {
    ///     let name = region.id.as_deref().unwrap_or("??");
    ///     let pop = region.value.map(|v| format!("{:.0}", v)).unwrap_or_default();
    ///     format!("{name}\nPopulation: {pop}")
    /// })
    /// ```
    #[cfg(not(target_arch = "wasm32"))]
    pub fn tooltip_format<F>(mut self, formatter: F) -> Self
    where
        F: Fn(&RegionRecord) -> String + Send + Sync + 'static,
    {
        self.tooltip_formatter = Some(Box::new(formatter));
        self.tooltip_enabled = true;
        self
    }

    /// Set a custom tooltip format closure.
    ///
    /// See the non-wasm documentation for details.
    #[cfg(target_arch = "wasm32")]
    pub fn tooltip_format<F>(mut self, formatter: F) -> Self
    where
        F: Fn(&RegionRecord) -> String + 'static,
    {
        self.tooltip_formatter = Some(Box::new(formatter));
        self.tooltip_enabled = true;
        self
    }

    /// Set the hover highlight style (default: [`HoverHighlight::Brighten(0.3)`]).
    ///
    /// Controls how the hovered region is visually distinguished. Use
    /// [`HoverHighlight::None`] to disable highlighting.
    pub fn highlight_style(mut self, style: HoverHighlight) -> Self {
        self.highlight_style = style;
        self
    }

    // -- Build -------------------------------------------------------------

    /// Resolve the builder into a renderable [`ChoroplethChart`].
    ///
    /// This performs the data join (matching GeoJSON features to data values),
    /// normalises values against the domain, tessellates all polygons, and
    /// assigns per-vertex colours.
    pub fn build(self) -> GupResult<ChoroplethChart> {
        // Validate required fields.
        let source = self.boundaries.ok_or_else(|| {
            GupError::validation_error(
                "ChoroplethChartBuilder: boundaries not set — call .boundaries() before .build()",
            )
        })?;

        let color_scale = self.color_scale.ok_or_else(|| {
            GupError::validation_error(
                "ChoroplethChartBuilder: color_scale not set — call .color_scale() before .build()",
            )
        })?;

        // Default region_id accessor: look up `iso_a3` property.
        let region_id_fn: Box<dyn Fn(&GeoFeature) -> Option<String>> =
            self.region_id_fn.unwrap_or_else(|| {
                Box::new(|f: &GeoFeature| {
                    f.properties
                        .as_ref()
                        .and_then(|p| p.get("iso_a3"))
                        .and_then(|v| v.as_str())
                        .map(String::from)
                })
            });

        // -- Data join: resolve per-feature colours -------------------------

        // Determine domain from data values (if not empty).
        let (domain_min, domain_max) = if self.data.is_empty() {
            (0.0_f64, 1.0_f64)
        } else {
            let mut min_val = f64::INFINITY;
            let mut max_val = f64::NEG_INFINITY;
            for &v in self.data.values() {
                if v < min_val {
                    min_val = v;
                }
                if v > max_val {
                    max_val = v;
                }
            }
            (min_val, max_val)
        };

        let mut regions: Vec<RegionRecord> = Vec::with_capacity(source.features.len());

        for (i, feature) in source.features.iter().enumerate() {
            let id = region_id_fn(feature);
            let value = id.as_ref().and_then(|k| self.data.get(k)).copied();

            let color = match value {
                Some(v) => sample_color_scale(&color_scale, v, domain_min, domain_max),
                None => self.no_data_color,
            };

            regions.push(RegionRecord {
                id,
                value,
                color,
                feature_index: i,
            });
        }

        // -- Tessellate geometry with per-vertex colours --------------------

        let tolerance = self.simplification_tolerance as f64;
        let mut fill_vertices: Vec<ChoroplethVertex> = Vec::new();
        let mut fill_indices: Vec<u32> = Vec::new();
        let mut stroke_vertices: Vec<ChoroplethVertex> = Vec::new();
        let mut indexed_fill_vertices: Vec<IndexedChoroplethVertex> = Vec::new();
        let mut region_polygons: Vec<Vec<Vec<[f32; 2]>>> =
            Vec::with_capacity(source.features.len());

        for (region_idx, feature) in source.features.iter().enumerate() {
            let fill_color = regions[region_idx].color;
            let stroke_color = self.stroke_color;
            let mut rings_for_region: Vec<Vec<[f32; 2]>> =
                Vec::with_capacity(feature.polygons.len());

            for polygon in &feature.polygons {
                let exterior = if tolerance > 0.0 {
                    simplify_ring(&polygon.exterior, tolerance)
                } else {
                    polygon.exterior.clone()
                };

                // Store projected exterior ring for hit-testing.
                let ring_f32: Vec<[f32; 2]> = exterior
                    .iter()
                    .map(|p| [p[0] as f32, p[1] as f32])
                    .collect();
                rings_for_region.push(ring_f32);

                // Fill tessellation (ear-clipping).
                let tri_verts = earclip_tessellate(&exterior);
                let base = fill_vertices.len() as u32;
                for (i, v) in tri_verts.iter().enumerate() {
                    fill_vertices.push(ChoroplethVertex {
                        position: *v,
                        color: fill_color,
                    });
                    fill_indices.push(base + i as u32);

                    if self.gpu_recolor {
                        indexed_fill_vertices.push(IndexedChoroplethVertex {
                            position: *v,
                            region_index: region_idx as u32,
                        });
                    }
                }

                // Stroke generation (line list).
                let ring = if tolerance > 0.0 {
                    simplify_ring(&polygon.exterior, tolerance)
                } else {
                    polygon.exterior.clone()
                };
                for i in 0..ring.len() {
                    let j = (i + 1) % ring.len();
                    stroke_vertices.push(ChoroplethVertex {
                        position: [ring[i][0] as f32, ring[i][1] as f32],
                        color: stroke_color,
                    });
                    stroke_vertices.push(ChoroplethVertex {
                        position: [ring[j][0] as f32, ring[j][1] as f32],
                        color: stroke_color,
                    });
                }
            }

            region_polygons.push(rings_for_region);
        }

        // Build the GPU recolouring data structures if enabled.
        let (indexed_opt, color_buffer_opt) = if self.gpu_recolor {
            let color_buffer = RegionColorBuffer::from_regions(&regions, self.no_data_color);
            (Some(indexed_fill_vertices), Some(color_buffer))
        } else {
            (None, None)
        };

        Ok(ChoroplethChart {
            fill_vertices,
            fill_indices,
            stroke_vertices,
            regions,
            color_scale,
            projection: self.projection,
            show_legend: self.show_legend,
            legend_position: self.legend_position,
            domain_min,
            domain_max,
            zoom_enabled: self.zoom_enabled,
            no_data_color: self.no_data_color,
            stroke_color: self.stroke_color,
            stroke_opacity: self.stroke_opacity,
            indexed_fill_vertices: indexed_opt,
            region_color_buffer: color_buffer_opt,
            tooltip_enabled: self.tooltip_enabled,
            highlight_style: self.highlight_style,
            region_polygons,
            tooltip_formatter: self.tooltip_formatter,
        })
    }
}

// ---------------------------------------------------------------------------
// CPU-side colour scale sampling
// ---------------------------------------------------------------------------

/// Sample a [`ColorScale`] on the CPU for a single value.
///
/// Normalises `value` into `[0, 1]` using the provided domain bounds and
/// linearly interpolates the gradient's colour stops.
pub fn sample_color_scale(
    scale: &ColorScale,
    value: f64,
    domain_min: f64,
    domain_max: f64,
) -> [f32; 4] {
    // Normalise to [0, 1].
    let t = if (domain_max - domain_min).abs() < f64::EPSILON {
        0.5
    } else {
        ((value - domain_min) / (domain_max - domain_min)).clamp(0.0, 1.0)
    };

    // Sample the gradient.
    let colors = &scale.gradient.colors;
    let stops = &scale.gradient.stops;

    if colors.is_empty() {
        return [0.0, 0.0, 0.0, 1.0];
    }

    let vec4_to_arr = |v: &crate::shader_function::Vec4| [v.x, v.y, v.z, v.w];

    if colors.len() == 1 || t <= stops[0] as f64 {
        return vec4_to_arr(&colors[0]);
    }
    let last = colors.len() - 1;
    if t >= stops[last] as f64 {
        return vec4_to_arr(&colors[last]);
    }

    // Binary search for the bracketing stops.
    let mut low = 0usize;
    let mut high = last;
    while low + 1 < high {
        let mid = (low + high) / 2;
        if (stops[mid] as f64) <= t {
            low = mid;
        } else {
            high = mid;
        }
    }

    let t0 = stops[low] as f64;
    let t1 = stops[high] as f64;
    let local_t = if (t1 - t0).abs() < f64::EPSILON {
        0.0
    } else {
        ((t - t0) / (t1 - t0)) as f32
    };

    let c0 = vec4_to_arr(&colors[low]);
    let c1 = vec4_to_arr(&colors[high]);
    [
        c0[0] + (c1[0] - c0[0]) * local_t,
        c0[1] + (c1[1] - c0[1]) * local_t,
        c0[2] + (c1[2] - c0[2]) * local_t,
        c0[3] + (c1[3] - c0[3]) * local_t,
    ]
}

// ---------------------------------------------------------------------------
// Geometry helpers (reused from geo_path module)
// ---------------------------------------------------------------------------

/// Ray-casting point-in-polygon test for a closed ring of `[f32; 2]` points.
///
/// Returns `true` if the point `(px, py)` lies inside the polygon defined
/// by `ring`. The algorithm counts the number of times a ray from the point
/// to +∞ along the x-axis crosses an edge of the polygon (odd = inside).
fn point_in_ring(px: f32, py: f32, ring: &[[f32; 2]]) -> bool {
    if ring.len() < 3 {
        return false;
    }
    let mut inside = false;
    let n = ring.len();
    let mut j = n - 1;
    for i in 0..n {
        let (xi, yi) = (ring[i][0], ring[i][1]);
        let (xj, yj) = (ring[j][0], ring[j][1]);

        // Does the edge from j→i straddle the horizontal ray from (px, py)?
        if ((yi > py) != (yj > py)) && (px < (xj - xi) * (py - yi) / (yj - yi) + xi) {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Ramer–Douglas–Peucker ring simplification.
fn simplify_ring(ring: &[[f64; 2]], tolerance: f64) -> Vec<[f64; 2]> {
    if ring.len() <= 3 || tolerance <= 0.0 {
        return ring.to_vec();
    }
    let mut keep = vec![false; ring.len()];
    keep[0] = true;
    keep[ring.len() - 1] = true;
    rdp_recurse(ring, 0, ring.len() - 1, tolerance * tolerance, &mut keep);
    ring.iter()
        .zip(keep.iter())
        .filter_map(|(pt, &k)| if k { Some(*pt) } else { None })
        .collect()
}

fn rdp_recurse(ring: &[[f64; 2]], start: usize, end: usize, tol_sq: f64, keep: &mut [bool]) {
    if end <= start + 1 {
        return;
    }
    let (ax, ay) = (ring[start][0], ring[start][1]);
    let (bx, by) = (ring[end][0], ring[end][1]);
    let mut max_dist = 0.0_f64;
    let mut max_idx = start;
    for i in (start + 1)..end {
        let d = point_line_dist_sq(ring[i][0], ring[i][1], ax, ay, bx, by);
        if d > max_dist {
            max_dist = d;
            max_idx = i;
        }
    }
    if max_dist > tol_sq {
        keep[max_idx] = true;
        rdp_recurse(ring, start, max_idx, tol_sq, keep);
        rdp_recurse(ring, max_idx, end, tol_sq, keep);
    }
}

fn point_line_dist_sq(px: f64, py: f64, ax: f64, ay: f64, bx: f64, by: f64) -> f64 {
    let dx = bx - ax;
    let dy = by - ay;
    let len_sq = dx * dx + dy * dy;
    if len_sq < f64::EPSILON {
        let ex = px - ax;
        let ey = py - ay;
        return ex * ex + ey * ey;
    }
    let t = ((px - ax) * dx + (py - ay) * dy) / len_sq;
    let t = t.clamp(0.0, 1.0);
    let proj_x = ax + t * dx;
    let proj_y = ay + t * dy;
    let ex = px - proj_x;
    let ey = py - proj_y;
    ex * ex + ey * ey
}

/// Ear-clipping tessellation of a polygon ring into triangle vertices.
///
/// Returns a flat list of `[f32; 2]` positions (3 per triangle).
fn earclip_tessellate(ring: &[[f64; 2]]) -> Vec<[f32; 2]> {
    // Remove duplicate closing vertex if present.
    let pts: Vec<[f64; 2]> = if ring.len() >= 2
        && (ring[0][0] - ring[ring.len() - 1][0]).abs() < f64::EPSILON
        && (ring[0][1] - ring[ring.len() - 1][1]).abs() < f64::EPSILON
    {
        ring[..ring.len() - 1].to_vec()
    } else {
        ring.to_vec()
    };

    if pts.len() < 3 {
        return Vec::new();
    }

    let mut indices: Vec<usize> = (0..pts.len()).collect();
    let mut result = Vec::new();

    // Ensure consistent winding (CCW).
    let area = signed_area(&pts);
    if area < 0.0 {
        indices.reverse();
    }

    let mut safety = pts.len() * pts.len();
    while indices.len() > 2 && safety > 0 {
        safety -= 1;
        let n = indices.len();
        let mut ear_found = false;
        for i in 0..n {
            let prev = indices[(i + n - 1) % n];
            let curr = indices[i];
            let next = indices[(i + 1) % n];

            if !is_convex(&pts[prev], &pts[curr], &pts[next]) {
                continue;
            }

            let mut ear = true;
            for j in 0..n {
                let idx = indices[j];
                if idx == prev || idx == curr || idx == next {
                    continue;
                }
                if point_in_triangle(&pts[idx], &pts[prev], &pts[curr], &pts[next]) {
                    ear = false;
                    break;
                }
            }

            if ear {
                result.push([pts[prev][0] as f32, pts[prev][1] as f32]);
                result.push([pts[curr][0] as f32, pts[curr][1] as f32]);
                result.push([pts[next][0] as f32, pts[next][1] as f32]);
                indices.remove(i);
                ear_found = true;
                break;
            }
        }
        if !ear_found {
            break;
        }
    }

    result
}

fn signed_area(pts: &[[f64; 2]]) -> f64 {
    let n = pts.len();
    let mut area = 0.0;
    for i in 0..n {
        let j = (i + 1) % n;
        area += pts[i][0] * pts[j][1];
        area -= pts[j][0] * pts[i][1];
    }
    area * 0.5
}

fn is_convex(a: &[f64; 2], b: &[f64; 2], c: &[f64; 2]) -> bool {
    let cross = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
    cross > 0.0
}

fn point_in_triangle(p: &[f64; 2], a: &[f64; 2], b: &[f64; 2], c: &[f64; 2]) -> bool {
    let d1 = sign(p, a, b);
    let d2 = sign(p, b, c);
    let d3 = sign(p, c, a);
    let has_neg = (d1 < 0.0) || (d2 < 0.0) || (d3 < 0.0);
    let has_pos = (d1 > 0.0) || (d2 > 0.0) || (d3 > 0.0);
    !(has_neg && has_pos)
}

fn sign(p1: &[f64; 2], p2: &[f64; 2], p3: &[f64; 2]) -> f64 {
    (p1[0] - p3[0]) * (p2[1] - p3[1]) - (p2[0] - p3[0]) * (p1[1] - p3[1])
}

// ---------------------------------------------------------------------------
// Top-level constructor
// ---------------------------------------------------------------------------

/// Create a new [`ChoroplethChartBuilder`] (convenience shorthand).
pub fn choropleth() -> ChoroplethChartBuilder {
    ChoroplethChartBuilder::new()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Synthetic three-feature GeoJSON for testing.
    fn synthetic_geojson() -> GeoJsonSource {
        let json = serde_json::json!({
            "type": "FeatureCollection",
            "features": [
                {
                    "type": "Feature",
                    "properties": { "iso_a3": "AAA", "name": "Region A" },
                    "geometry": {
                        "type": "Polygon",
                        "coordinates": [[[0.0,0.0],[10.0,0.0],[10.0,10.0],[0.0,10.0],[0.0,0.0]]]
                    }
                },
                {
                    "type": "Feature",
                    "properties": { "iso_a3": "BBB", "name": "Region B" },
                    "geometry": {
                        "type": "Polygon",
                        "coordinates": [[[10.0,0.0],[20.0,0.0],[20.0,10.0],[10.0,10.0],[10.0,0.0]]]
                    }
                },
                {
                    "type": "Feature",
                    "properties": { "iso_a3": "CCC", "name": "Region C" },
                    "geometry": {
                        "type": "Polygon",
                        "coordinates": [[[20.0,0.0],[30.0,0.0],[30.0,10.0],[20.0,10.0],[20.0,0.0]]]
                    }
                }
            ]
        });
        GeoJsonSource::from_value(&json).unwrap()
    }

    #[test]
    fn test_data_join_assigns_no_data_color_for_missing_region() {
        let source = synthetic_geojson();
        let chart = ChoroplethChartBuilder::new()
            .boundaries(source)
            .data(vec![("AAA", 10.0), ("CCC", 90.0)])
            .color_scale(ColorScale::viridis(10.0, 90.0))
            .build()
            .unwrap();

        // Region BBB has no data → should get the no-data colour.
        let bbb = &chart.regions[1];
        assert_eq!(bbb.id.as_deref(), Some("BBB"));
        assert!(bbb.value.is_none());
        assert_eq!(bbb.color, [0.75, 0.75, 0.75, 1.0]); // default no-data
    }

    #[test]
    fn test_data_join_normalises_values() {
        let source = synthetic_geojson();
        let chart = ChoroplethChartBuilder::new()
            .boundaries(source)
            .data(vec![("AAA", 0.0), ("BBB", 50.0), ("CCC", 100.0)])
            .color_scale(ColorScale::viridis(0.0, 100.0))
            .build()
            .unwrap();

        // AAA should be at domain min (t=0), CCC at domain max (t=1).
        let aaa = &chart.regions[0];
        let ccc = &chart.regions[2];
        assert!(aaa.value == Some(0.0));
        assert!(ccc.value == Some(100.0));
        // Colors should differ (viridis maps 0→dark purple, 1→yellow).
        assert_ne!(aaa.color, ccc.color);
    }

    #[test]
    fn test_data_join_with_custom_region_id() {
        let source = synthetic_geojson();
        let chart = ChoroplethChartBuilder::new()
            .boundaries(source)
            .data(vec![
                ("Region A", 10.0),
                ("Region B", 50.0),
                ("Region C", 90.0),
            ])
            .region_id(|f| {
                f.properties
                    .as_ref()
                    .and_then(|p| p.get("name"))
                    .and_then(|v| v.as_str())
                    .map(String::from)
            })
            .color_scale(ColorScale::viridis(10.0, 90.0))
            .build()
            .unwrap();

        // All three regions should have values.
        assert!(chart.regions.iter().all(|r| r.value.is_some()));
    }

    #[test]
    fn test_build_produces_non_empty_geometry() {
        let source = synthetic_geojson();
        let chart = ChoroplethChartBuilder::new()
            .boundaries(source)
            .data(vec![("AAA", 10.0)])
            .color_scale(ColorScale::viridis(0.0, 100.0))
            .build()
            .unwrap();

        assert!(!chart.fill_vertices.is_empty());
        assert!(!chart.fill_indices.is_empty());
        assert!(!chart.stroke_vertices.is_empty());
    }

    #[test]
    fn test_build_fails_without_boundaries() {
        let result = ChoroplethChartBuilder::new()
            .data(vec![("AAA", 10.0)])
            .color_scale(ColorScale::viridis(0.0, 100.0))
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn test_build_fails_without_color_scale() {
        let source = synthetic_geojson();
        let result = ChoroplethChartBuilder::new()
            .boundaries(source)
            .data(vec![("AAA", 10.0)])
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn test_legend_toggle() {
        let source = synthetic_geojson();
        let chart = ChoroplethChartBuilder::new()
            .boundaries(source)
            .data(vec![("AAA", 10.0)])
            .color_scale(ColorScale::viridis(0.0, 100.0))
            .legend(false)
            .build()
            .unwrap();
        assert!(!chart.show_legend);
    }

    #[test]
    fn test_zoom_toggle() {
        let source = synthetic_geojson();
        let chart = ChoroplethChartBuilder::new()
            .boundaries(source)
            .data(vec![("AAA", 10.0)])
            .color_scale(ColorScale::viridis(0.0, 100.0))
            .zoom(false)
            .build()
            .unwrap();
        assert!(!chart.zoom_enabled);
    }

    #[test]
    fn test_custom_no_data_color() {
        let source = synthetic_geojson();
        let custom_color = [1.0, 0.0, 0.0, 1.0]; // red
        let chart = ChoroplethChartBuilder::new()
            .boundaries(source)
            .data(vec![("AAA", 10.0)]) // BBB and CCC have no data
            .color_scale(ColorScale::viridis(0.0, 100.0))
            .no_data_color(custom_color)
            .build()
            .unwrap();

        let bbb = &chart.regions[1];
        assert_eq!(bbb.color, custom_color);
    }

    #[test]
    fn test_sample_color_scale_boundaries() {
        let scale = ColorScale::viridis(0.0, 100.0);

        // At domain min.
        let c_min = sample_color_scale(&scale, 0.0, 0.0, 100.0);
        // At domain max.
        let c_max = sample_color_scale(&scale, 100.0, 0.0, 100.0);

        // Colours should differ (viridis is a multi-hue scale).
        assert_ne!(c_min, c_max);

        // Below domain min → clamped to first stop.
        let c_below = sample_color_scale(&scale, -10.0, 0.0, 100.0);
        assert_eq!(c_below, c_min);

        // Above domain max → clamped to last stop.
        let c_above = sample_color_scale(&scale, 200.0, 0.0, 100.0);
        assert_eq!(c_above, c_max);
    }

    #[test]
    fn test_sample_color_scale_equal_domain() {
        let scale = ColorScale::viridis(50.0, 50.0);
        // When domain_min == domain_max, should return midpoint colour.
        let color = sample_color_scale(&scale, 50.0, 50.0, 50.0);
        // Should not panic and should return a valid colour.
        assert!(color.iter().all(|c| (0.0..=1.0).contains(c)));
    }

    #[test]
    fn test_data_from_records() {
        struct CountryData {
            code: String,
            population: f64,
        }
        let records = vec![
            CountryData {
                code: "AAA".into(),
                population: 100.0,
            },
            CountryData {
                code: "BBB".into(),
                population: 200.0,
            },
            CountryData {
                code: "CCC".into(),
                population: 300.0,
            },
        ];

        let source = synthetic_geojson();
        let chart = ChoroplethChartBuilder::new()
            .boundaries(source)
            .data_from_records(records, |r| r.code.clone(), |r| r.population)
            .color_scale(ColorScale::viridis(100.0, 300.0))
            .build()
            .unwrap();

        // All three regions should have values.
        assert!(chart.regions.iter().all(|r| r.value.is_some()));
        assert_eq!(chart.regions[0].value, Some(100.0));
        assert_eq!(chart.regions[1].value, Some(200.0));
        assert_eq!(chart.regions[2].value, Some(300.0));
    }

    #[test]
    fn test_data_from_records_partial_coverage() {
        struct Record {
            iso: String,
            val: f64,
        }
        let records = vec![Record {
            iso: "AAA".into(),
            val: 42.0,
        }];

        let source = synthetic_geojson();
        let chart = ChoroplethChartBuilder::new()
            .boundaries(source)
            .data_from_records(records, |r| r.iso.clone(), |r| r.val)
            .color_scale(ColorScale::viridis(0.0, 100.0))
            .build()
            .unwrap();

        // Only AAA has data; BBB and CCC should get no-data colour.
        assert!(chart.regions[0].value.is_some());
        assert!(chart.regions[1].value.is_none());
        assert!(chart.regions[2].value.is_none());
        assert_eq!(chart.regions[1].color, [0.75, 0.75, 0.75, 1.0]);
    }

    #[test]
    fn test_legend_position_variants() {
        let source = synthetic_geojson();
        for pos in [
            LegendPosition::Bottom,
            LegendPosition::Top,
            LegendPosition::Left,
            LegendPosition::Right,
        ] {
            let chart = ChoroplethChartBuilder::new()
                .boundaries(source.clone())
                .data(vec![("AAA", 10.0)])
                .color_scale(ColorScale::viridis(0.0, 100.0))
                .legend_position(pos)
                .build()
                .unwrap();
            assert_eq!(chart.legend_position, pos);
        }
    }

    #[test]
    fn test_projection_variants() {
        let source = synthetic_geojson();
        for proj in [Projection::Mercator, Projection::Equirectangular] {
            let chart = ChoroplethChartBuilder::new()
                .boundaries(source.clone())
                .data(vec![("AAA", 10.0)])
                .color_scale(ColorScale::viridis(0.0, 100.0))
                .projection(proj)
                .build()
                .unwrap();
            assert_eq!(chart.projection, proj);
        }
    }

    #[test]
    fn test_simplification_reduces_vertices() {
        let source = synthetic_geojson();
        let full = ChoroplethChartBuilder::new()
            .boundaries(source.clone())
            .data(vec![("AAA", 10.0)])
            .color_scale(ColorScale::viridis(0.0, 100.0))
            .build()
            .unwrap();

        let simplified = ChoroplethChartBuilder::new()
            .boundaries(source)
            .data(vec![("AAA", 10.0)])
            .color_scale(ColorScale::viridis(0.0, 100.0))
            .simplification_tolerance(5.0)
            .build()
            .unwrap();

        // With a large tolerance, simplified should have <= full vertices.
        assert!(simplified.stroke_vertices.len() <= full.stroke_vertices.len());
    }

    #[test]
    fn test_stroke_opacity_propagates() {
        let source = synthetic_geojson();
        let chart = ChoroplethChartBuilder::new()
            .boundaries(source)
            .data(vec![("AAA", 10.0)])
            .color_scale(ColorScale::viridis(0.0, 100.0))
            .stroke_opacity(0.8)
            .build()
            .unwrap();
        assert!((chart.stroke_opacity - 0.8).abs() < f32::EPSILON);
        assert!((chart.stroke_color[3] - 0.8).abs() < f32::EPSILON);
    }

    #[test]
    fn test_domain_auto_computed_from_data() {
        let source = synthetic_geojson();
        let chart = ChoroplethChartBuilder::new()
            .boundaries(source)
            .data(vec![("AAA", 5.0), ("BBB", 50.0), ("CCC", 500.0)])
            .color_scale(ColorScale::viridis(0.0, 1.0))
            .build()
            .unwrap();

        assert!((chart.domain_min - 5.0).abs() < f64::EPSILON);
        assert!((chart.domain_max - 500.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_empty_data_uses_default_domain() {
        let source = synthetic_geojson();
        let chart = ChoroplethChartBuilder::new()
            .boundaries(source)
            .data(Vec::<(&str, f64)>::new())
            .color_scale(ColorScale::viridis(0.0, 1.0))
            .build()
            .unwrap();

        // Empty data → default domain [0, 1].
        assert!((chart.domain_min - 0.0).abs() < f64::EPSILON);
        assert!((chart.domain_max - 1.0).abs() < f64::EPSILON);
        // All regions should get no-data colour.
        assert!(chart.regions.iter().all(|r| r.value.is_none()));
    }

    #[test]
    fn test_default_builder_values() {
        let builder = ChoroplethChartBuilder::new();
        assert!(builder.zoom_enabled);
        assert!(builder.show_legend);
        assert_eq!(builder.legend_position, LegendPosition::Bottom);
        assert_eq!(builder.no_data_color, [0.75, 0.75, 0.75, 1.0]);
        assert!((builder.stroke_opacity - 0.4).abs() < f32::EPSILON);
    }

    #[test]
    fn test_choropleth_convenience_constructor() {
        // Verify the module-level `choropleth()` function.
        let builder = choropleth();
        assert!(builder.zoom_enabled);
        assert!(builder.show_legend);
    }

    // -----------------------------------------------------------------------
    // GPU-side recolouring tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_region_color_buffer_new() {
        let buf = RegionColorBuffer::new(5, [0.5, 0.5, 0.5, 1.0]);
        assert_eq!(buf.len(), 5);
        assert!(!buf.is_empty());
        for i in 0..5 {
            assert_eq!(*buf.color(i).unwrap(), [0.5, 0.5, 0.5, 1.0]);
        }
        assert_eq!(buf.no_data_color(), [0.5, 0.5, 0.5, 1.0]);
    }

    #[test]
    fn test_region_color_buffer_set_color() {
        let mut buf = RegionColorBuffer::new(3, [0.0, 0.0, 0.0, 1.0]);
        buf.set_color(1, [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(*buf.color(0).unwrap(), [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(*buf.color(1).unwrap(), [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(*buf.color(2).unwrap(), [0.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn test_region_color_buffer_set_out_of_bounds_is_noop() {
        let mut buf = RegionColorBuffer::new(2, [0.0, 0.0, 0.0, 1.0]);
        buf.set_color(99, [1.0, 0.0, 0.0, 1.0]); // should not panic
        assert_eq!(buf.len(), 2);
    }

    #[test]
    fn test_region_color_buffer_from_regions() {
        let regions = vec![
            RegionRecord {
                id: Some("A".into()),
                value: Some(10.0),
                color: [1.0, 0.0, 0.0, 1.0],
                feature_index: 0,
            },
            RegionRecord {
                id: Some("B".into()),
                value: None,
                color: [0.5, 0.5, 0.5, 1.0],
                feature_index: 1,
            },
        ];
        let buf = RegionColorBuffer::from_regions(&regions, [0.75, 0.75, 0.75, 1.0]);
        assert_eq!(buf.len(), 2);
        assert_eq!(*buf.color(0).unwrap(), [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(*buf.color(1).unwrap(), [0.5, 0.5, 0.5, 1.0]);
    }

    #[test]
    fn test_region_color_buffer_update_from_data() {
        let regions = vec![
            RegionRecord {
                id: Some("A".into()),
                value: Some(0.0),
                color: [0.0; 4],
                feature_index: 0,
            },
            RegionRecord {
                id: Some("B".into()),
                value: None,
                color: [0.0; 4],
                feature_index: 1,
            },
            RegionRecord {
                id: Some("C".into()),
                value: Some(100.0),
                color: [0.0; 4],
                feature_index: 2,
            },
        ];
        let no_data = [0.75, 0.75, 0.75, 1.0];
        let mut buf = RegionColorBuffer::new(3, no_data);
        let mut data = HashMap::new();
        data.insert("A".to_string(), 0.0);
        data.insert("C".to_string(), 100.0);
        let scale = ColorScale::viridis(0.0, 100.0);

        buf.update_from_data(&data, &regions, &scale);

        // A (value=0) and C (value=100) should have different colours.
        let color_a = *buf.color(0).unwrap();
        let color_c = *buf.color(2).unwrap();
        assert_ne!(color_a, color_c);
        // B (no data) should have the no-data colour.
        assert_eq!(*buf.color(1).unwrap(), no_data);
    }

    #[test]
    fn test_region_color_buffer_interpolate() {
        let buf_a = RegionColorBuffer {
            colors: vec![[0.0, 0.0, 0.0, 1.0], [1.0, 1.0, 1.0, 1.0]],
            no_data_color: [0.0; 4],
        };
        let buf_b = RegionColorBuffer {
            colors: vec![[1.0, 0.0, 0.0, 1.0], [0.0, 1.0, 0.0, 1.0]],
            no_data_color: [0.0; 4],
        };

        // t=0 → same as buf_a
        let r0 = buf_a.interpolate(&buf_b, 0.0);
        assert_eq!(r0.colors()[0], [0.0, 0.0, 0.0, 1.0]);

        // t=1 → same as buf_b
        let r1 = buf_a.interpolate(&buf_b, 1.0);
        assert_eq!(r1.colors()[0], [1.0, 0.0, 0.0, 1.0]);

        // t=0.5 → midpoint
        let r_mid = buf_a.interpolate(&buf_b, 0.5);
        for c in r_mid.colors()[0].iter() {
            assert!(*c >= 0.0 && *c <= 1.0);
        }
        assert!((r_mid.colors()[0][0] - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_region_color_buffer_interpolate_clamps() {
        let buf = RegionColorBuffer::new(1, [0.0, 0.0, 0.0, 1.0]);
        let target = RegionColorBuffer::new(1, [1.0, 1.0, 1.0, 1.0]);

        // Out-of-range t values should clamp.
        let r_neg = buf.interpolate(&target, -0.5);
        assert_eq!(r_neg.colors()[0], [0.0, 0.0, 0.0, 1.0]);
        let r_over = buf.interpolate(&target, 2.0);
        assert_eq!(r_over.colors()[0], [1.0, 1.0, 1.0, 1.0]);
    }

    #[test]
    fn test_region_color_buffer_as_bytes() {
        let buf = RegionColorBuffer::new(2, [1.0, 0.5, 0.25, 1.0]);
        let bytes = buf.as_bytes();
        // 2 regions × 4 f32 × 4 bytes = 32 bytes.
        assert_eq!(bytes.len(), 32);
    }

    #[test]
    fn test_gpu_recolor_disabled_by_default() {
        let source = synthetic_geojson();
        let chart = ChoroplethChartBuilder::new()
            .boundaries(source)
            .data(vec![("AAA", 10.0)])
            .color_scale(ColorScale::viridis(0.0, 100.0))
            .build()
            .unwrap();

        assert!(chart.indexed_fill_vertices.is_none());
        assert!(chart.region_color_buffer.is_none());
    }

    #[test]
    fn test_gpu_recolor_produces_indexed_vertices() {
        let source = synthetic_geojson();
        let chart = ChoroplethChartBuilder::new()
            .boundaries(source)
            .data(vec![("AAA", 10.0), ("BBB", 50.0), ("CCC", 90.0)])
            .color_scale(ColorScale::viridis(0.0, 100.0))
            .gpu_recolor(true)
            .build()
            .unwrap();

        // Indexed vertices should be present and match fill vertices count.
        let indexed = chart.indexed_fill_vertices.as_ref().unwrap();
        assert_eq!(indexed.len(), chart.fill_vertices.len());

        // Region color buffer should have one entry per region.
        let buf = chart.region_color_buffer.as_ref().unwrap();
        assert_eq!(buf.len(), 3);
    }

    #[test]
    fn test_gpu_recolor_indexed_vertices_have_correct_region_indices() {
        let source = synthetic_geojson();
        let chart = ChoroplethChartBuilder::new()
            .boundaries(source)
            .data(vec![("AAA", 10.0), ("BBB", 50.0), ("CCC", 90.0)])
            .color_scale(ColorScale::viridis(0.0, 100.0))
            .gpu_recolor(true)
            .build()
            .unwrap();

        let indexed = chart.indexed_fill_vertices.as_ref().unwrap();
        // All region indices should be in [0, 2].
        for v in indexed {
            assert!(v.region_index <= 2);
        }
        // At least some vertices should have different region indices
        // (we have 3 regions each producing fill triangles).
        let unique: std::collections::HashSet<u32> =
            indexed.iter().map(|v| v.region_index).collect();
        assert!(unique.len() > 1);
    }

    #[test]
    fn test_gpu_recolor_color_buffer_matches_regions() {
        let source = synthetic_geojson();
        let chart = ChoroplethChartBuilder::new()
            .boundaries(source)
            .data(vec![("AAA", 10.0), ("CCC", 90.0)]) // BBB has no data
            .color_scale(ColorScale::viridis(10.0, 90.0))
            .gpu_recolor(true)
            .build()
            .unwrap();

        let buf = chart.region_color_buffer.as_ref().unwrap();
        // Each region's colour in the buffer should match its RegionRecord.
        for region in &chart.regions {
            assert_eq!(*buf.color(region.feature_index).unwrap(), region.color);
        }
    }

    #[test]
    fn test_update_colors_recolours_without_retessellation() {
        let source = synthetic_geojson();
        let mut chart = ChoroplethChartBuilder::new()
            .boundaries(source)
            .data(vec![("AAA", 10.0), ("BBB", 50.0), ("CCC", 90.0)])
            .color_scale(ColorScale::viridis(0.0, 100.0))
            .gpu_recolor(true)
            .build()
            .unwrap();

        // Record original state.
        let orig_vertex_count = chart.fill_vertices.len();
        let orig_index_count = chart.fill_indices.len();
        let orig_color_a = *chart
            .region_color_buffer
            .as_ref()
            .unwrap()
            .color(0)
            .unwrap();

        // Update with new data (different values).
        chart
            .update_colors(vec![("AAA", 90.0), ("BBB", 50.0), ("CCC", 10.0)])
            .unwrap();

        // Geometry should be unchanged.
        assert_eq!(chart.fill_vertices.len(), orig_vertex_count);
        assert_eq!(chart.fill_indices.len(), orig_index_count);

        // Colours should have changed.
        let new_color_a = *chart
            .region_color_buffer
            .as_ref()
            .unwrap()
            .color(0)
            .unwrap();
        assert_ne!(orig_color_a, new_color_a);
    }

    #[test]
    fn test_update_colors_fails_without_gpu_recolor() {
        let source = synthetic_geojson();
        let mut chart = ChoroplethChartBuilder::new()
            .boundaries(source)
            .data(vec![("AAA", 10.0)])
            .color_scale(ColorScale::viridis(0.0, 100.0))
            .build()
            .unwrap();

        let result = chart.update_colors(vec![("AAA", 50.0)]);
        assert!(result.is_err());
    }

    #[test]
    fn test_update_colors_updates_domain_and_region_values() {
        let source = synthetic_geojson();
        let mut chart = ChoroplethChartBuilder::new()
            .boundaries(source)
            .data(vec![("AAA", 10.0), ("BBB", 50.0), ("CCC", 90.0)])
            .color_scale(ColorScale::viridis(0.0, 100.0))
            .gpu_recolor(true)
            .build()
            .unwrap();

        chart
            .update_colors(vec![("AAA", 200.0), ("CCC", 800.0)])
            .unwrap();

        // Domain should update to new data range.
        assert!((chart.domain_min - 200.0).abs() < f64::EPSILON);
        assert!((chart.domain_max - 800.0).abs() < f64::EPSILON);

        // BBB should now have no data.
        let bbb = &chart.regions[1];
        assert!(bbb.value.is_none());
    }

    #[test]
    fn test_interpolate_colors_returns_none_without_gpu_recolor() {
        let source = synthetic_geojson();
        let chart = ChoroplethChartBuilder::new()
            .boundaries(source)
            .data(vec![("AAA", 10.0)])
            .color_scale(ColorScale::viridis(0.0, 100.0))
            .build()
            .unwrap();

        let target = RegionColorBuffer::new(3, [1.0, 0.0, 0.0, 1.0]);
        assert!(chart.interpolate_colors(&target, 0.5).is_none());
    }

    #[test]
    fn test_interpolate_colors_returns_interpolated_buffer() {
        let source = synthetic_geojson();
        let chart = ChoroplethChartBuilder::new()
            .boundaries(source)
            .data(vec![("AAA", 0.0), ("BBB", 50.0), ("CCC", 100.0)])
            .color_scale(ColorScale::viridis(0.0, 100.0))
            .gpu_recolor(true)
            .build()
            .unwrap();

        let target = RegionColorBuffer::new(3, [1.0, 0.0, 0.0, 1.0]);
        let result = chart.interpolate_colors(&target, 0.5);
        assert!(result.is_some());
        let buf = result.unwrap();
        assert_eq!(buf.len(), 3);
    }

    #[test]
    fn test_recolor_shaders_are_non_empty() {
        assert!(!RECOLOR_VERTEX_SHADER.is_empty());
        assert!(!RECOLOR_FRAGMENT_SHADER.is_empty());
        assert!(RECOLOR_VERTEX_SHADER.contains("region_colors"));
        assert!(RECOLOR_VERTEX_SHADER.contains("vs_main"));
        assert!(RECOLOR_FRAGMENT_SHADER.contains("fs_main"));
    }

    #[test]
    fn test_indexed_vertex_layout_is_pod() {
        // Verify bytemuck compatibility.
        let v = IndexedChoroplethVertex {
            position: [1.0, 2.0],
            region_index: 42,
        };
        let bytes: &[u8] = bytemuck::bytes_of(&v);
        // 2 × f32 + 1 × u32 = 12 bytes.
        assert_eq!(bytes.len(), 12);
    }

    // -----------------------------------------------------------------------
    // Point-in-ring / hit-testing tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_point_in_ring_inside() {
        // Unit square [0,0] → [10,10].
        let ring = vec![
            [0.0_f32, 0.0],
            [10.0, 0.0],
            [10.0, 10.0],
            [0.0, 10.0],
            [0.0, 0.0],
        ];
        assert!(point_in_ring(5.0, 5.0, &ring));
        assert!(point_in_ring(1.0, 1.0, &ring));
        assert!(point_in_ring(9.0, 9.0, &ring));
    }

    #[test]
    fn test_point_in_ring_outside() {
        let ring = vec![
            [0.0_f32, 0.0],
            [10.0, 0.0],
            [10.0, 10.0],
            [0.0, 10.0],
            [0.0, 0.0],
        ];
        assert!(!point_in_ring(-1.0, 5.0, &ring));
        assert!(!point_in_ring(11.0, 5.0, &ring));
        assert!(!point_in_ring(5.0, -1.0, &ring));
        assert!(!point_in_ring(5.0, 11.0, &ring));
    }

    #[test]
    fn test_point_in_ring_triangle() {
        let ring = vec![[0.0_f32, 0.0], [10.0, 0.0], [5.0, 10.0], [0.0, 0.0]];
        assert!(point_in_ring(5.0, 3.0, &ring));
        assert!(!point_in_ring(0.5, 9.0, &ring)); // outside, near top-left
    }

    #[test]
    fn test_point_in_ring_degenerate() {
        // Fewer than 3 points → always false.
        assert!(!point_in_ring(0.0, 0.0, &[]));
        assert!(!point_in_ring(0.0, 0.0, &[[0.0, 0.0]]));
        assert!(!point_in_ring(0.0, 0.0, &[[0.0, 0.0], [1.0, 1.0]]));
    }

    // -----------------------------------------------------------------------
    // region_at_point tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_region_at_point_finds_correct_region() {
        let source = synthetic_geojson();
        let chart = ChoroplethChartBuilder::new()
            .boundaries(source)
            .data(vec![("AAA", 10.0), ("BBB", 50.0), ("CCC", 90.0)])
            .color_scale(ColorScale::viridis(0.0, 100.0))
            .build()
            .unwrap();

        // Point inside region AAA (0..10, 0..10).
        assert_eq!(chart.region_at_point(5.0, 5.0), Some(0));
        // Point inside region BBB (10..20, 0..10).
        assert_eq!(chart.region_at_point(15.0, 5.0), Some(1));
        // Point inside region CCC (20..30, 0..10).
        assert_eq!(chart.region_at_point(25.0, 5.0), Some(2));
    }

    #[test]
    fn test_region_at_point_returns_none_outside() {
        let source = synthetic_geojson();
        let chart = ChoroplethChartBuilder::new()
            .boundaries(source)
            .data(vec![("AAA", 10.0)])
            .color_scale(ColorScale::viridis(0.0, 100.0))
            .build()
            .unwrap();

        assert!(chart.region_at_point(-5.0, 5.0).is_none());
        assert!(chart.region_at_point(35.0, 5.0).is_none());
        assert!(chart.region_at_point(5.0, -5.0).is_none());
        assert!(chart.region_at_point(5.0, 15.0).is_none());
    }

    // -----------------------------------------------------------------------
    // Tooltip tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_tooltip_disabled_by_default() {
        let source = synthetic_geojson();
        let chart = ChoroplethChartBuilder::new()
            .boundaries(source)
            .data(vec![("AAA", 10.0)])
            .color_scale(ColorScale::viridis(0.0, 100.0))
            .build()
            .unwrap();

        assert!(!chart.tooltip_enabled);
        assert!(chart.tooltip_content(0).is_none());
    }

    #[test]
    fn test_tooltip_enabled() {
        let source = synthetic_geojson();
        let chart = ChoroplethChartBuilder::new()
            .boundaries(source)
            .data(vec![("AAA", 10.0), ("BBB", 50.0)])
            .color_scale(ColorScale::viridis(0.0, 100.0))
            .tooltip(true)
            .build()
            .unwrap();

        assert!(chart.tooltip_enabled);
        let content = chart.tooltip_content(0).unwrap();
        assert!(content.contains("AAA"));
        assert!(content.contains("10"));
    }

    #[test]
    fn test_tooltip_no_data_region() {
        let source = synthetic_geojson();
        let chart = ChoroplethChartBuilder::new()
            .boundaries(source)
            .data(vec![("AAA", 10.0)]) // BBB and CCC have no data
            .color_scale(ColorScale::viridis(0.0, 100.0))
            .tooltip(true)
            .build()
            .unwrap();

        let content = chart.tooltip_content(1).unwrap();
        assert!(content.contains("BBB"));
        assert!(content.contains("no data"));
    }

    #[test]
    fn test_tooltip_format_custom() {
        let source = synthetic_geojson();
        let chart = ChoroplethChartBuilder::new()
            .boundaries(source)
            .data(vec![("AAA", 42.0)])
            .color_scale(ColorScale::viridis(0.0, 100.0))
            .tooltip_format(|region| format!("Custom: {}", region.id.as_deref().unwrap_or("?")))
            .build()
            .unwrap();

        // tooltip_format implicitly enables tooltip.
        assert!(chart.tooltip_enabled);
        let content = chart.tooltip_content(0).unwrap();
        assert_eq!(content, "Custom: AAA");
    }

    #[test]
    fn test_tooltip_out_of_bounds() {
        let source = synthetic_geojson();
        let chart = ChoroplethChartBuilder::new()
            .boundaries(source)
            .data(vec![("AAA", 10.0)])
            .color_scale(ColorScale::viridis(0.0, 100.0))
            .tooltip(true)
            .build()
            .unwrap();

        assert!(chart.tooltip_content(999).is_none());
    }

    // -----------------------------------------------------------------------
    // Hover highlight tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_highlight_brighten_hovered() {
        let source = synthetic_geojson();
        let chart = ChoroplethChartBuilder::new()
            .boundaries(source)
            .data(vec![("AAA", 10.0)])
            .color_scale(ColorScale::viridis(0.0, 100.0))
            .highlight_style(HoverHighlight::Brighten(0.2))
            .build()
            .unwrap();

        let base = chart.regions[0].color;
        let highlighted = chart.highlighted_color(0, true);

        // RGB channels should be brighter (or clamped to 1.0).
        assert!(highlighted[0] >= base[0]);
        assert!(highlighted[1] >= base[1]);
        assert!(highlighted[2] >= base[2]);
        // Alpha unchanged.
        assert!((highlighted[3] - base[3]).abs() < f32::EPSILON);
    }

    #[test]
    fn test_highlight_brighten_not_hovered() {
        let source = synthetic_geojson();
        let chart = ChoroplethChartBuilder::new()
            .boundaries(source)
            .data(vec![("AAA", 10.0)])
            .color_scale(ColorScale::viridis(0.0, 100.0))
            .highlight_style(HoverHighlight::Brighten(0.2))
            .build()
            .unwrap();

        let base = chart.regions[0].color;
        let color = chart.highlighted_color(0, false);

        // Non-hovered should be unchanged.
        assert_eq!(color, base);
    }

    #[test]
    fn test_highlight_dim_non_hovered() {
        let source = synthetic_geojson();
        let chart = ChoroplethChartBuilder::new()
            .boundaries(source)
            .data(vec![("AAA", 10.0)])
            .color_scale(ColorScale::viridis(0.0, 100.0))
            .highlight_style(HoverHighlight::Dim(0.4))
            .build()
            .unwrap();

        let base = chart.regions[0].color;
        let dimmed = chart.highlighted_color(0, false);

        // Alpha should be reduced.
        assert!((dimmed[3] - base[3] * 0.4).abs() < f32::EPSILON);
        // RGB unchanged.
        assert_eq!(dimmed[0], base[0]);
    }

    #[test]
    fn test_highlight_dim_hovered() {
        let source = synthetic_geojson();
        let chart = ChoroplethChartBuilder::new()
            .boundaries(source)
            .data(vec![("AAA", 10.0)])
            .color_scale(ColorScale::viridis(0.0, 100.0))
            .highlight_style(HoverHighlight::Dim(0.4))
            .build()
            .unwrap();

        let base = chart.regions[0].color;
        let color = chart.highlighted_color(0, true);

        // Hovered region should be unchanged.
        assert_eq!(color, base);
    }

    #[test]
    fn test_highlight_none() {
        let source = synthetic_geojson();
        let chart = ChoroplethChartBuilder::new()
            .boundaries(source)
            .data(vec![("AAA", 10.0)])
            .color_scale(ColorScale::viridis(0.0, 100.0))
            .highlight_style(HoverHighlight::None)
            .build()
            .unwrap();

        let base = chart.regions[0].color;
        assert_eq!(chart.highlighted_color(0, true), base);
        assert_eq!(chart.highlighted_color(0, false), base);
    }

    #[test]
    fn test_default_highlight_is_brighten() {
        let builder = ChoroplethChartBuilder::new();
        match builder.highlight_style {
            HoverHighlight::Brighten(v) => assert!((v - 0.3).abs() < f32::EPSILON),
            _ => panic!("Expected default HoverHighlight::Brighten(0.3)"),
        }
    }

    // -----------------------------------------------------------------------
    // Region polygon storage tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_region_polygons_stored_during_build() {
        let source = synthetic_geojson();
        let chart = ChoroplethChartBuilder::new()
            .boundaries(source)
            .data(vec![("AAA", 10.0)])
            .color_scale(ColorScale::viridis(0.0, 100.0))
            .build()
            .unwrap();

        // Three regions → three polygon entries.
        assert_eq!(chart.region_polygons.len(), 3);
        // Each region has exactly one polygon (from our synthetic data).
        for rings in &chart.region_polygons {
            assert_eq!(rings.len(), 1);
            assert!(!rings[0].is_empty());
        }
    }
}

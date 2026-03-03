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
#[derive(Debug, Clone)]
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

        for (region_idx, feature) in source.features.iter().enumerate() {
            let fill_color = regions[region_idx].color;
            let stroke_color = self.stroke_color;

            for polygon in &feature.polygons {
                let exterior = if tolerance > 0.0 {
                    simplify_ring(&polygon.exterior, tolerance)
                } else {
                    polygon.exterior.clone()
                };

                // Fill tessellation (ear-clipping).
                let tri_verts = earclip_tessellate(&exterior);
                let base = fill_vertices.len() as u32;
                for (i, v) in tri_verts.iter().enumerate() {
                    fill_vertices.push(ChoroplethVertex {
                        position: *v,
                        color: fill_color,
                    });
                    fill_indices.push(base + i as u32);
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
        }

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
}

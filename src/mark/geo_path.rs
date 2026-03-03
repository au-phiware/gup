// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Geographic path mark for rendering GeoJSON polygons.
//!
//! The `GeoPathMark` renders country and region outlines from GeoJSON boundary
//! data. It supports `Feature` and `FeatureCollection` documents containing
//! `Polygon` and `MultiPolygon` geometries, applies a geographic projection
//! (Mercator, Equirectangular, etc.) to convert spherical coordinates to
//! screen-space positions, and tessellates filled regions via ear-clipping.
//!
//! # Topology Simplification
//!
//! An optional Ramer–Douglas–Peucker simplification pass reduces polygon vertex
//! count at small display scales to maintain interactive frame rates for
//! world-scale datasets. The tolerance is specified in degrees.
//!
//! # Example
//!
//! ```rust,ignore
//! use gup::mark::geo_path::{GeoPathMark, GeoJsonSource, Projection};
//!
//! let source = GeoJsonSource::from_str(geojson_str)?;
//! let mark = GeoPathMark::new(source, Projection::Mercator)
//!     .fill_color([0.8, 0.85, 0.9, 1.0])
//!     .stroke_color([0.3, 0.3, 0.3, 1.0])
//!     .stroke_width(1.0)
//!     .simplification_tolerance(0.5);
//! ```

use super::Mark;
use crate::error::{GupError, GupResult};
use crate::shader_pipeline::ComposableShaderPipeline;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Projection Identifier
// ---------------------------------------------------------------------------

/// Geographic projection to apply when rendering coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Projection {
    /// Mercator (Web Mercator) — conformal cylindrical.
    Mercator,
    /// Equirectangular (Plate Carrée) — equidistant cylindrical.
    Equirectangular,
}

// ---------------------------------------------------------------------------
// GeoJSON Types
// ---------------------------------------------------------------------------

/// A ring of coordinates: a closed loop of `[longitude, latitude]` pairs.
pub type Ring = Vec<[f64; 2]>;

/// A single polygon consisting of an exterior ring and zero or more holes.
#[derive(Debug, Clone)]
pub struct GeoPolygon {
    /// Outer boundary ring.
    pub exterior: Ring,
    /// Interior hole rings (may be empty).
    pub holes: Vec<Ring>,
}

/// A parsed geographic feature containing one or more polygons.
#[derive(Debug, Clone)]
pub struct GeoFeature {
    /// Feature properties (kept as raw JSON for downstream use).
    pub properties: Option<serde_json::Value>,
    /// Polygons belonging to this feature.
    pub polygons: Vec<GeoPolygon>,
}

// ---------------------------------------------------------------------------
// GeoJsonSource
// ---------------------------------------------------------------------------

/// Parsed GeoJSON data source providing geographic polygon features.
///
/// Constructed from a raw GeoJSON string or a `serde_json::Value`. Only
/// `Polygon` and `MultiPolygon` geometries are supported; other geometry
/// types are explicitly rejected with a descriptive error.
#[derive(Debug, Clone)]
pub struct GeoJsonSource {
    /// Parsed features.
    pub features: Vec<GeoFeature>,
}

impl GeoJsonSource {
    /// Parse a GeoJSON string into a `GeoJsonSource`.
    ///
    /// Accepts `Feature`, `FeatureCollection`, `Polygon`, and `MultiPolygon`
    /// at the top level.
    pub fn from_str(s: &str) -> GupResult<Self> {
        let value: serde_json::Value =
            serde_json::from_str(s).map_err(|e| GupError::InvalidDataFormat {
                message: format!("Malformed GeoJSON: {e}"),
            })?;
        Self::from_value(&value)
    }

    /// Parse a `serde_json::Value` into a `GeoJsonSource`.
    pub fn from_value(value: &serde_json::Value) -> GupResult<Self> {
        let geo_type = value.get("type").and_then(|t| t.as_str()).ok_or_else(|| {
            GupError::InvalidDataFormat {
                message: "GeoJSON object missing \"type\" field".to_string(),
            }
        })?;

        match geo_type {
            "FeatureCollection" => {
                let arr = value
                    .get("features")
                    .and_then(|f| f.as_array())
                    .ok_or_else(|| GupError::InvalidDataFormat {
                        message: "FeatureCollection missing \"features\" array".to_string(),
                    })?;
                let mut features = Vec::with_capacity(arr.len());
                for (i, feat_val) in arr.iter().enumerate() {
                    match Self::parse_feature(feat_val) {
                        Ok(f) => features.push(f),
                        Err(e) => {
                            return Err(GupError::InvalidDataFormat {
                                message: format!("Error in feature[{i}]: {e}"),
                            });
                        }
                    }
                }
                Ok(Self { features })
            }
            "Feature" => {
                let feature = Self::parse_feature(value)?;
                Ok(Self {
                    features: vec![feature],
                })
            }
            "Polygon" => {
                let polygons = vec![Self::parse_polygon(value)?];
                Ok(Self {
                    features: vec![GeoFeature {
                        properties: None,
                        polygons,
                    }],
                })
            }
            "MultiPolygon" => {
                let polygons = Self::parse_multi_polygon(value)?;
                Ok(Self {
                    features: vec![GeoFeature {
                        properties: None,
                        polygons,
                    }],
                })
            }
            "Point" | "MultiPoint" | "LineString" | "MultiLineString" | "GeometryCollection" => {
                Err(GupError::InvalidDataFormat {
                    message: format!(
                        "Unsupported geometry type \"{geo_type}\": \
                     GeoPathMark only supports Polygon and MultiPolygon \
                     geometries"
                    ),
                })
            }
            _ => Err(GupError::InvalidDataFormat {
                message: format!("Unknown GeoJSON type \"{geo_type}\""),
            }),
        }
    }

    // -- internal helpers --------------------------------------------------

    fn parse_feature(value: &serde_json::Value) -> GupResult<GeoFeature> {
        let properties = value.get("properties").cloned();
        let geometry = value
            .get("geometry")
            .ok_or_else(|| GupError::InvalidDataFormat {
                message: "Feature missing \"geometry\" field".to_string(),
            })?;

        if geometry.is_null() {
            // A Feature with null geometry is valid GeoJSON but has no shapes.
            return Ok(GeoFeature {
                properties,
                polygons: Vec::new(),
            });
        }

        let geo_type = geometry
            .get("type")
            .and_then(|t| t.as_str())
            .ok_or_else(|| GupError::InvalidDataFormat {
                message: "Geometry missing \"type\" field".to_string(),
            })?;

        let polygons = match geo_type {
            "Polygon" => vec![Self::parse_polygon(geometry)?],
            "MultiPolygon" => Self::parse_multi_polygon(geometry)?,
            "Point" | "MultiPoint" | "LineString" | "MultiLineString" | "GeometryCollection" => {
                return Err(GupError::InvalidDataFormat {
                    message: format!(
                        "Unsupported geometry type \"{geo_type}\": \
                         GeoPathMark only supports Polygon and \
                         MultiPolygon geometries"
                    ),
                });
            }
            _ => {
                return Err(GupError::InvalidDataFormat {
                    message: format!("Unknown geometry type \"{geo_type}\""),
                });
            }
        };

        Ok(GeoFeature {
            properties,
            polygons,
        })
    }

    fn parse_polygon(value: &serde_json::Value) -> GupResult<GeoPolygon> {
        let coords = value
            .get("coordinates")
            .and_then(|c| c.as_array())
            .ok_or_else(|| GupError::InvalidDataFormat {
                message: "Polygon missing \"coordinates\" array".to_string(),
            })?;

        if coords.is_empty() {
            return Err(GupError::InvalidDataFormat {
                message: "Polygon has empty coordinates array".to_string(),
            });
        }

        let exterior = Self::parse_ring(&coords[0])?;
        let holes: Vec<Ring> = coords[1..]
            .iter()
            .map(Self::parse_ring)
            .collect::<GupResult<Vec<_>>>()?;

        Ok(GeoPolygon { exterior, holes })
    }

    fn parse_multi_polygon(value: &serde_json::Value) -> GupResult<Vec<GeoPolygon>> {
        let coords = value
            .get("coordinates")
            .and_then(|c| c.as_array())
            .ok_or_else(|| GupError::InvalidDataFormat {
                message: "MultiPolygon missing \"coordinates\" array".to_string(),
            })?;

        let mut polygons = Vec::with_capacity(coords.len());
        for (i, poly_coords) in coords.iter().enumerate() {
            let rings = poly_coords
                .as_array()
                .ok_or_else(|| GupError::InvalidDataFormat {
                    message: format!("MultiPolygon coordinate[{i}] is not an array"),
                })?;
            if rings.is_empty() {
                continue;
            }
            let exterior = Self::parse_ring(&rings[0])?;
            let holes: Vec<Ring> = rings[1..]
                .iter()
                .map(Self::parse_ring)
                .collect::<GupResult<Vec<_>>>()?;
            polygons.push(GeoPolygon { exterior, holes });
        }
        Ok(polygons)
    }

    fn parse_ring(value: &serde_json::Value) -> GupResult<Ring> {
        let arr = value
            .as_array()
            .ok_or_else(|| GupError::InvalidDataFormat {
                message: "Ring is not an array".to_string(),
            })?;
        let mut ring = Vec::with_capacity(arr.len());
        for (i, coord) in arr.iter().enumerate() {
            let pair = coord
                .as_array()
                .ok_or_else(|| GupError::InvalidDataFormat {
                    message: format!("Ring coordinate[{i}] is not an array"),
                })?;
            if pair.len() < 2 {
                return Err(GupError::InvalidDataFormat {
                    message: format!("Ring coordinate[{i}] has fewer than 2 elements"),
                });
            }
            let lon = pair[0]
                .as_f64()
                .ok_or_else(|| GupError::InvalidDataFormat {
                    message: format!("Ring coordinate[{i}][0] is not a number"),
                })?;
            let lat = pair[1]
                .as_f64()
                .ok_or_else(|| GupError::InvalidDataFormat {
                    message: format!("Ring coordinate[{i}][1] is not a number"),
                })?;
            ring.push([lon, lat]);
        }
        Ok(ring)
    }
}

// ---------------------------------------------------------------------------
// Ramer–Douglas–Peucker Simplification
// ---------------------------------------------------------------------------

/// Simplify a ring of coordinates using the Ramer–Douglas–Peucker algorithm.
///
/// `tolerance` is in the same units as the coordinates (degrees for
/// geographic data). A tolerance of `0.0` returns the original ring
/// unchanged.
pub fn simplify_ring(ring: &[[f64; 2]], tolerance: f64) -> Vec<[f64; 2]> {
    if tolerance <= 0.0 || ring.len() < 3 {
        return ring.to_vec();
    }
    let mut keep = vec![false; ring.len()];
    keep[0] = true;
    keep[ring.len() - 1] = true;
    rdp_recurse(ring, 0, ring.len() - 1, tolerance, &mut keep);
    ring.iter()
        .zip(keep.iter())
        .filter(|(_, k)| **k)
        .map(|(pt, _)| *pt)
        .collect()
}

fn rdp_recurse(ring: &[[f64; 2]], start: usize, end: usize, tolerance: f64, keep: &mut [bool]) {
    if end <= start + 1 {
        return;
    }
    let mut max_dist = 0.0_f64;
    let mut max_idx = start;
    let a = ring[start];
    let b = ring[end];
    for i in (start + 1)..end {
        let d = perpendicular_distance(ring[i], a, b);
        if d > max_dist {
            max_dist = d;
            max_idx = i;
        }
    }
    if max_dist > tolerance {
        keep[max_idx] = true;
        rdp_recurse(ring, start, max_idx, tolerance, keep);
        rdp_recurse(ring, max_idx, end, tolerance, keep);
    }
}

/// Perpendicular distance from point `p` to the line segment `a`–`b`.
fn perpendicular_distance(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let dx = b[0] - a[0];
    let dy = b[1] - a[1];
    let len_sq = dx * dx + dy * dy;
    if len_sq < 1e-20 {
        // Degenerate segment — return Euclidean distance to `a`.
        let ex = p[0] - a[0];
        let ey = p[1] - a[1];
        return (ex * ex + ey * ey).sqrt();
    }
    let numerator = ((b[0] - a[0]) * (a[1] - p[1]) - (a[0] - p[0]) * (b[1] - a[1])).abs();
    numerator / len_sq.sqrt()
}

// ---------------------------------------------------------------------------
// Ear-Clipping Tessellation
// ---------------------------------------------------------------------------

/// Tessellate a simple polygon (no holes) into triangles using ear clipping.
///
/// Returns a flat list of `[x, y]` triangle vertex positions.
/// The input ring should be in counter-clockwise order (standard for
/// GeoJSON exterior rings). Clockwise rings are detected and reversed.
pub fn earclip_tessellate(ring: &[[f64; 2]]) -> Vec<[f32; 2]> {
    if ring.len() < 3 {
        return Vec::new();
    }

    // Remove the closing duplicate if present.
    let pts: Vec<[f64; 2]> = if ring.len() > 3
        && (ring.first().unwrap()[0] - ring.last().unwrap()[0]).abs() < 1e-12
        && (ring.first().unwrap()[1] - ring.last().unwrap()[1]).abs() < 1e-12
    {
        ring[..ring.len() - 1].to_vec()
    } else {
        ring.to_vec()
    };

    if pts.len() < 3 {
        return Vec::new();
    }

    // Ensure CCW winding.
    let mut poly = pts;
    if signed_area(&poly) < 0.0 {
        poly.reverse();
    }

    let n = poly.len();
    let mut indices: Vec<usize> = (0..n).collect();
    let mut triangles: Vec<[f32; 2]> = Vec::with_capacity((n - 2) * 3);
    let mut remaining = n;

    let mut fail_count = 0;
    let mut i = 0;

    while remaining > 2 {
        if fail_count >= remaining {
            // Unable to find an ear — degenerate polygon, bail out.
            break;
        }

        let prev = indices[(i + remaining - 1) % remaining];
        let curr = indices[i % remaining];
        let next = indices[(i + 1) % remaining];

        if is_ear(&poly, &indices, remaining, prev, curr, next) {
            triangles.push([poly[prev][0] as f32, poly[prev][1] as f32]);
            triangles.push([poly[curr][0] as f32, poly[curr][1] as f32]);
            triangles.push([poly[next][0] as f32, poly[next][1] as f32]);
            indices.remove(i % remaining);
            remaining -= 1;
            if remaining > 0 {
                i %= remaining;
            }
            fail_count = 0;
        } else {
            i = (i + 1) % remaining;
            fail_count += 1;
        }
    }

    triangles
}

/// Signed area of a polygon (positive = CCW, negative = CW).
fn signed_area(ring: &[[f64; 2]]) -> f64 {
    let n = ring.len();
    let mut area = 0.0;
    for i in 0..n {
        let j = (i + 1) % n;
        area += ring[i][0] * ring[j][1];
        area -= ring[j][0] * ring[i][1];
    }
    area * 0.5
}

/// Check whether the triangle (prev, curr, next) is a valid ear.
fn is_ear(
    poly: &[[f64; 2]],
    indices: &[usize],
    count: usize,
    prev: usize,
    curr: usize,
    next: usize,
) -> bool {
    let a = poly[prev];
    let b = poly[curr];
    let c = poly[next];

    // Must be convex (CCW triangle).
    if cross_2d(a, b, c) <= 0.0 {
        return false;
    }

    // No other vertex inside the triangle.
    for k in 0..count {
        let idx = indices[k];
        if idx == prev || idx == curr || idx == next {
            continue;
        }
        if point_in_triangle(poly[idx], a, b, c) {
            return false;
        }
    }
    true
}

/// 2D cross product of vectors (b-a) × (c-a).
fn cross_2d(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> f64 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}

/// Point-in-triangle test using barycentric coordinates.
fn point_in_triangle(p: [f64; 2], a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> bool {
    let d1 = cross_2d(a, b, p);
    let d2 = cross_2d(b, c, p);
    let d3 = cross_2d(c, a, p);
    let has_neg = (d1 < 0.0) || (d2 < 0.0) || (d3 < 0.0);
    let has_pos = (d1 > 0.0) || (d2 > 0.0) || (d3 > 0.0);
    !(has_neg && has_pos)
}

// ---------------------------------------------------------------------------
// GeoPathMark
// ---------------------------------------------------------------------------

/// GPU vertex for geo path rendering.
///
/// Stores position as a longitude/latitude pair that the vertex shader will
/// project to clip-space using the selected projection function.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GeoPathVertex {
    /// Longitude, latitude in degrees (will be projected in vertex shader).
    pub position: [f32; 2],
    /// Barycentric / UV coordinate for stroke detection (0 = interior, 1 = edge).
    pub edge_flag: f32,
    /// Padding for 4-byte alignment.
    pub _pad: f32,
}

/// High-level attributes for `GeoPathMark`.
#[derive(Debug, Clone)]
pub struct GeoPathAttributes {
    /// Fill colour (RGBA). `None` means no fill.
    pub fill_color: Option<[f32; 4]>,
    /// Stroke colour (RGBA). `None` means no stroke.
    pub stroke_color: Option<[f32; 4]>,
    /// Stroke width in pixels.
    pub stroke_width: f32,
}

impl Default for GeoPathAttributes {
    fn default() -> Self {
        Self {
            fill_color: Some([0.75, 0.82, 0.88, 1.0]),
            stroke_color: Some([0.3, 0.3, 0.3, 1.0]),
            stroke_width: 1.0,
        }
    }
}

/// A mark that renders GeoJSON polygon features with a geographic projection.
///
/// `GeoPathMark` implements the [`Mark`] trait and produces tessellated
/// triangle geometry from GeoJSON `Polygon` / `MultiPolygon` features.
/// The vertex shader applies a geographic projection to convert
/// `(longitude, latitude)` to clip-space coordinates.
#[derive(Debug, Clone)]
pub struct GeoPathMark {
    /// Source GeoJSON data.
    source: GeoJsonSource,
    /// Projection to apply.
    projection: Projection,
    /// Fill colour.
    fill_color: Option<[f32; 4]>,
    /// Stroke colour.
    stroke_color: Option<[f32; 4]>,
    /// Stroke width in pixels.
    stroke_width: f32,
    /// Ramer–Douglas–Peucker tolerance in degrees (0.0 = no simplification).
    simplification_tolerance: f32,
}

impl GeoPathMark {
    /// Create a new `GeoPathMark` with the given source and projection.
    pub fn new(source: GeoJsonSource, projection: Projection) -> Self {
        Self {
            source,
            projection,
            fill_color: Some([0.75, 0.82, 0.88, 1.0]),
            stroke_color: Some([0.3, 0.3, 0.3, 1.0]),
            stroke_width: 1.0,
            simplification_tolerance: 0.0,
        }
    }

    /// Set the fill colour. Pass `None` to disable fill.
    pub fn fill_color(mut self, color: Option<[f32; 4]>) -> Self {
        self.fill_color = color;
        self
    }

    /// Set the stroke colour. Pass `None` to disable stroke.
    pub fn stroke_color(mut self, color: Option<[f32; 4]>) -> Self {
        self.stroke_color = color;
        self
    }

    /// Set the stroke width in pixels.
    pub fn stroke_width(mut self, width: f32) -> Self {
        self.stroke_width = width;
        self
    }

    /// Set the Ramer–Douglas–Peucker simplification tolerance in degrees.
    ///
    /// A tolerance of `0.0` (the default) disables simplification and uses
    /// the original GeoJSON coordinates verbatim. Typical values:
    ///
    /// - `0.5` — coarse world maps
    /// - `0.05` — regional detail
    pub fn simplification_tolerance(mut self, tolerance: f32) -> Self {
        self.simplification_tolerance = tolerance;
        self
    }

    /// Access the underlying `GeoJsonSource`.
    pub fn source(&self) -> &GeoJsonSource {
        &self.source
    }

    /// Access the selected projection.
    pub fn projection(&self) -> Projection {
        self.projection
    }

    /// Tessellate all features into triangle vertices and stroke-line vertices.
    ///
    /// Returns `(fill_vertices, fill_indices, stroke_vertices)`.
    ///
    /// *  `fill_vertices` / `fill_indices` — triangle mesh for filled polygons.
    /// *  `stroke_vertices` — line-list vertices for boundary outlines.
    pub fn tessellate(&self) -> GupResult<(Vec<GeoPathVertex>, Vec<u32>, Vec<GeoPathVertex>)> {
        let tolerance = self.simplification_tolerance as f64;

        let mut fill_verts: Vec<GeoPathVertex> = Vec::new();
        let mut fill_indices: Vec<u32> = Vec::new();
        let mut stroke_verts: Vec<GeoPathVertex> = Vec::new();

        for feature in &self.source.features {
            for polygon in &feature.polygons {
                let exterior = if tolerance > 0.0 {
                    simplify_ring(&polygon.exterior, tolerance)
                } else {
                    polygon.exterior.clone()
                };

                // -- Fill tessellation (ear clipping) ----------------------
                let tri_verts = earclip_tessellate(&exterior);
                let base = fill_verts.len() as u32;
                for (i, v) in tri_verts.iter().enumerate() {
                    fill_verts.push(GeoPathVertex {
                        position: *v,
                        edge_flag: 0.0,
                        _pad: 0.0,
                    });
                    fill_indices.push(base + i as u32);
                }

                // -- Stroke generation (line list) -------------------------
                let ring = if tolerance > 0.0 {
                    simplify_ring(&polygon.exterior, tolerance)
                } else {
                    polygon.exterior.clone()
                };
                for i in 0..ring.len() {
                    let j = (i + 1) % ring.len();
                    stroke_verts.push(GeoPathVertex {
                        position: [ring[i][0] as f32, ring[i][1] as f32],
                        edge_flag: 1.0,
                        _pad: 0.0,
                    });
                    stroke_verts.push(GeoPathVertex {
                        position: [ring[j][0] as f32, ring[j][1] as f32],
                        edge_flag: 1.0,
                        _pad: 0.0,
                    });
                }
            }
        }

        Ok((fill_verts, fill_indices, stroke_verts))
    }

    /// Count total triangles produced by tessellation at the given tolerance.
    pub fn triangle_count(&self, tolerance: f32) -> usize {
        let tol = tolerance as f64;
        let mut count = 0usize;
        for feature in &self.source.features {
            for polygon in &feature.polygons {
                let exterior = if tol > 0.0 {
                    simplify_ring(&polygon.exterior, tol)
                } else {
                    polygon.exterior.clone()
                };
                let tri_verts = earclip_tessellate(&exterior);
                count += tri_verts.len() / 3;
            }
        }
        count
    }
}

// ---------------------------------------------------------------------------
// Mark Trait Implementation
// ---------------------------------------------------------------------------

impl Mark for GeoPathMark {
    type Vertex = GeoPathVertex;
    type AttributeValue = GeoPathAttributes;

    /// Hand-written vertex shader for geo path rendering.
    const VERTEX_SHADER: Option<&'static str> = Some(include_str!("shaders/geo_path.vert.wgsl"));

    /// Hand-written fragment shader for geo path rendering.
    const FRAGMENT_SHADER: Option<&'static str> = Some(include_str!("shaders/geo_path.frag.wgsl"));

    fn vertex_count() -> usize {
        // Dynamic — determined at tessellation time.
        4
    }

    fn index_count() -> Option<usize> {
        Some(6)
    }

    fn generate_vertices() -> Vec<Self::Vertex> {
        // Base quad placeholder — real geometry is produced by `tessellate()`.
        vec![
            GeoPathVertex {
                position: [-1.0, -1.0],
                edge_flag: 0.0,
                _pad: 0.0,
            },
            GeoPathVertex {
                position: [1.0, -1.0],
                edge_flag: 0.0,
                _pad: 0.0,
            },
            GeoPathVertex {
                position: [1.0, 1.0],
                edge_flag: 0.0,
                _pad: 0.0,
            },
            GeoPathVertex {
                position: [-1.0, 1.0],
                edge_flag: 0.0,
                _pad: 0.0,
            },
        ]
    }

    fn generate_indices() -> Option<Vec<u32>> {
        Some(vec![0, 1, 2, 0, 2, 3])
    }

    fn vertex_attributes() -> &'static [wgpu::VertexAttribute] {
        &[
            wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x2, // position (lon, lat)
            },
            wgpu::VertexAttribute {
                offset: 8,
                shader_location: 1,
                format: wgpu::VertexFormat::Float32, // edge_flag
            },
        ]
    }

    fn generate_vertex_shader_with_functions(
        pipeline: &ComposableShaderPipeline,
        _attribute_functions: &HashMap<String, String>,
    ) -> String {
        let base_shader = pipeline.generate_vertex_shader();

        format!(
            r#"
// Geo path vertex shader (generated)

struct GeoPathInstance {{
    fill_color: vec4<f32>,
    stroke_color: vec4<f32>,
    stroke_width: f32,
    _pad: vec3<f32>,
}}

@group(0) @binding(0)
var<storage, read> instances: array<GeoPathInstance>;

struct VertexOutput {{
    @builtin(position) position: vec4<f32>,
    @location(0) fill_color: vec4<f32>,
    @location(1) stroke_color: vec4<f32>,
    @location(2) edge_flag: f32,
    @location(3) stroke_width: f32,
}}

@vertex
fn vs_main(
    @location(0) lonlat: vec2<f32>,
    @location(1) edge_flag: f32,
    @builtin(instance_index) instance_index: u32,
) -> VertexOutput {{
    let instance = instances[instance_index];
    var output: VertexOutput;
    // Placeholder projection — replaced by hand-written shader in practice.
    output.position = vec4<f32>(lonlat / 180.0, 0.0, 1.0);
    output.fill_color = instance.fill_color;
    output.stroke_color = instance.stroke_color;
    output.edge_flag = edge_flag;
    output.stroke_width = instance.stroke_width;
    return output;
}}

{base_shader}
"#
        )
    }

    fn generate_fragment_shader_with_functions(
        pipeline: &ComposableShaderPipeline,
        _attribute_functions: &HashMap<String, String>,
    ) -> String {
        let base_shader = pipeline.generate_fragment_shader();

        format!(
            r#"
// Geo path fragment shader (generated)

struct FragmentInput {{
    @location(0) fill_color: vec4<f32>,
    @location(1) stroke_color: vec4<f32>,
    @location(2) edge_flag: f32,
    @location(3) stroke_width: f32,
}}

@fragment
fn fs_main(input: FragmentInput) -> @location(0) vec4<f32> {{
    if (input.edge_flag > 0.5) {{
        return input.stroke_color;
    }}
    return input.fill_color;
}}

{base_shader}
"#
        )
    }

    fn get_attribute_type(attribute_name: &str) -> GupResult<&'static str> {
        match attribute_name {
            "position" => Ok("vec2<f32>"),
            "fill_color" | "stroke_color" => Ok("vec4<f32>"),
            "stroke_width" | "edge_flag" => Ok("f32"),
            _ => Err(GupError::validation_error(format!(
                "Unknown geo path attribute: {attribute_name}"
            ))),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- GeoJSON Parsing ---------------------------------------------------

    #[test]
    fn test_parse_polygon() {
        let geojson = r#"{
            "type": "Polygon",
            "coordinates": [
                [[0, 0], [10, 0], [10, 10], [0, 10], [0, 0]]
            ]
        }"#;
        let source = GeoJsonSource::from_str(geojson).unwrap();
        assert_eq!(source.features.len(), 1);
        assert_eq!(source.features[0].polygons.len(), 1);
        assert_eq!(source.features[0].polygons[0].exterior.len(), 5);
    }

    #[test]
    fn test_parse_multi_polygon() {
        let geojson = r#"{
            "type": "MultiPolygon",
            "coordinates": [
                [[[0, 0], [1, 0], [1, 1], [0, 0]]],
                [[[2, 2], [3, 2], [3, 3], [2, 2]]]
            ]
        }"#;
        let source = GeoJsonSource::from_str(geojson).unwrap();
        assert_eq!(source.features.len(), 1);
        assert_eq!(source.features[0].polygons.len(), 2);
    }

    #[test]
    fn test_parse_feature() {
        let geojson = r#"{
            "type": "Feature",
            "properties": {"name": "TestLand"},
            "geometry": {
                "type": "Polygon",
                "coordinates": [[[0, 0], [10, 0], [10, 10], [0, 10], [0, 0]]]
            }
        }"#;
        let source = GeoJsonSource::from_str(geojson).unwrap();
        assert_eq!(source.features.len(), 1);
        let props = source.features[0].properties.as_ref().unwrap();
        assert_eq!(props["name"], "TestLand");
    }

    #[test]
    fn test_parse_feature_collection() {
        let geojson = r#"{
            "type": "FeatureCollection",
            "features": [
                {
                    "type": "Feature",
                    "properties": {"name": "A"},
                    "geometry": {
                        "type": "Polygon",
                        "coordinates": [[[0, 0], [1, 0], [1, 1], [0, 0]]]
                    }
                },
                {
                    "type": "Feature",
                    "properties": {"name": "B"},
                    "geometry": {
                        "type": "MultiPolygon",
                        "coordinates": [
                            [[[2, 2], [3, 2], [3, 3], [2, 2]]],
                            [[[4, 4], [5, 4], [5, 5], [4, 4]]]
                        ]
                    }
                }
            ]
        }"#;
        let source = GeoJsonSource::from_str(geojson).unwrap();
        assert_eq!(source.features.len(), 2);
        assert_eq!(source.features[0].polygons.len(), 1);
        assert_eq!(source.features[1].polygons.len(), 2);
    }

    #[test]
    fn test_parse_malformed_json() {
        let result = GeoJsonSource::from_str("{not valid json");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Malformed GeoJSON"));
    }

    #[test]
    fn test_parse_unsupported_point() {
        let geojson = r#"{"type": "Point", "coordinates": [0, 0]}"#;
        let result = GeoJsonSource::from_str(geojson);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("Unsupported geometry type"),
            "got: {err}"
        );
    }

    #[test]
    fn test_parse_unsupported_linestring() {
        let geojson = r#"{"type": "LineString", "coordinates": [[0, 0], [1, 1]]}"#;
        let result = GeoJsonSource::from_str(geojson);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Unsupported geometry type")
        );
    }

    #[test]
    fn test_parse_unsupported_multipoint() {
        let geojson = r#"{"type": "MultiPoint", "coordinates": [[0, 0], [1, 1]]}"#;
        let result = GeoJsonSource::from_str(geojson);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Unsupported geometry type")
        );
    }

    #[test]
    fn test_parse_unsupported_multilinestring() {
        let geojson = r#"{
            "type": "MultiLineString",
            "coordinates": [[[0, 0], [1, 1]], [[2, 2], [3, 3]]]
        }"#;
        let result = GeoJsonSource::from_str(geojson);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Unsupported geometry type")
        );
    }

    #[test]
    fn test_parse_unsupported_geometry_collection() {
        let geojson = r#"{
            "type": "GeometryCollection",
            "geometries": []
        }"#;
        let result = GeoJsonSource::from_str(geojson);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Unsupported geometry type")
        );
    }

    #[test]
    fn test_parse_from_value() {
        let value: serde_json::Value = serde_json::json!({
            "type": "Polygon",
            "coordinates": [[[0, 0], [10, 0], [10, 10], [0, 10], [0, 0]]]
        });
        let source = GeoJsonSource::from_value(&value).unwrap();
        assert_eq!(source.features.len(), 1);
    }

    #[test]
    fn test_parse_feature_null_geometry() {
        let geojson = r#"{
            "type": "Feature",
            "properties": {"name": "Empty"},
            "geometry": null
        }"#;
        let source = GeoJsonSource::from_str(geojson).unwrap();
        assert_eq!(source.features.len(), 1);
        assert!(source.features[0].polygons.is_empty());
    }

    // -- RDP Simplification ------------------------------------------------

    #[test]
    fn test_simplify_zero_tolerance() {
        let ring = vec![[0.0, 0.0], [1.0, 0.0], [2.0, 0.1], [3.0, 0.0]];
        let simplified = simplify_ring(&ring, 0.0);
        assert_eq!(simplified.len(), ring.len());
    }

    #[test]
    fn test_simplify_removes_collinear() {
        // Midpoint is very close to the line between start and end.
        let ring = vec![[0.0, 0.0], [5.0, 0.001], [10.0, 0.0]];
        let simplified = simplify_ring(&ring, 0.01);
        assert_eq!(simplified.len(), 2, "collinear point should be removed");
    }

    #[test]
    fn test_simplify_keeps_significant_deviation() {
        let ring = vec![
            [0.0, 0.0],
            [5.0, 5.0], // large deviation
            [10.0, 0.0],
        ];
        let simplified = simplify_ring(&ring, 0.1);
        assert_eq!(simplified.len(), 3, "significant point should be kept");
    }

    // -- Ear-Clipping Tessellation -----------------------------------------

    #[test]
    fn test_earclip_triangle() {
        let ring = vec![[0.0, 0.0], [10.0, 0.0], [5.0, 10.0], [0.0, 0.0]];
        let triangles = earclip_tessellate(&ring);
        assert_eq!(triangles.len(), 3, "one triangle = 3 vertices");
    }

    #[test]
    fn test_earclip_square() {
        let ring = vec![
            [0.0, 0.0],
            [10.0, 0.0],
            [10.0, 10.0],
            [0.0, 10.0],
            [0.0, 0.0],
        ];
        let triangles = earclip_tessellate(&ring);
        assert_eq!(triangles.len(), 6, "two triangles = 6 vertices");
    }

    #[test]
    fn test_earclip_degenerate_line() {
        let ring = vec![[0.0, 0.0], [1.0, 0.0]];
        let triangles = earclip_tessellate(&ring);
        assert!(triangles.is_empty());
    }

    // -- GeoPathMark -------------------------------------------------------

    #[test]
    fn test_geo_path_mark_builder() {
        let geojson = r#"{
            "type": "Polygon",
            "coordinates": [[[0, 0], [10, 0], [10, 10], [0, 10], [0, 0]]]
        }"#;
        let source = GeoJsonSource::from_str(geojson).unwrap();
        let mark = GeoPathMark::new(source, Projection::Mercator)
            .fill_color(Some([1.0, 0.0, 0.0, 1.0]))
            .stroke_color(Some([0.0, 0.0, 0.0, 1.0]))
            .stroke_width(2.0)
            .simplification_tolerance(0.5);
        assert_eq!(mark.projection(), Projection::Mercator);
        assert_eq!(mark.stroke_width, 2.0);
        assert_eq!(mark.simplification_tolerance, 0.5);
    }

    #[test]
    fn test_geo_path_mark_tessellation() {
        let geojson = r#"{
            "type": "Polygon",
            "coordinates": [[[0, 0], [10, 0], [10, 10], [0, 10], [0, 0]]]
        }"#;
        let source = GeoJsonSource::from_str(geojson).unwrap();
        let mark = GeoPathMark::new(source, Projection::Equirectangular);
        let (fill_verts, fill_indices, stroke_verts) = mark.tessellate().unwrap();
        assert!(!fill_verts.is_empty(), "should produce fill vertices");
        assert!(!fill_indices.is_empty(), "should produce fill indices");
        assert!(!stroke_verts.is_empty(), "should produce stroke vertices");
    }

    #[test]
    fn test_triangle_count_with_simplification() {
        // Build a polygon with many intermediate points that can be simplified.
        let mut coords = Vec::new();
        for i in 0..100 {
            let angle = 2.0 * std::f64::consts::PI * (i as f64) / 100.0;
            coords.push([angle.cos() * 10.0, angle.sin() * 10.0]);
        }
        coords.push(coords[0]); // close

        let geojson = serde_json::json!({
            "type": "Polygon",
            "coordinates": [coords]
        });
        let source = GeoJsonSource::from_value(&geojson).unwrap();
        let mark = GeoPathMark::new(source, Projection::Mercator);

        let full = mark.triangle_count(0.0);
        let simplified = mark.triangle_count(2.0);
        assert!(
            simplified < full,
            "simplification should reduce triangle count: \
             full={full}, simplified={simplified}"
        );
    }

    // -- Mark Trait ---------------------------------------------------------

    #[test]
    fn test_geo_path_vertex_layout() {
        let vertex = GeoPathVertex {
            position: [1.0, 2.0],
            edge_flag: 0.0,
            _pad: 0.0,
        };
        let bytes = bytemuck::bytes_of(&vertex);
        assert_eq!(bytes.len(), std::mem::size_of::<GeoPathVertex>());
        assert_eq!(std::mem::size_of::<GeoPathVertex>(), 16);
    }

    #[test]
    fn test_geo_path_mark_generates_vertices() {
        let vertices = GeoPathMark::generate_vertices();
        assert_eq!(vertices.len(), GeoPathMark::vertex_count());
    }

    #[test]
    fn test_geo_path_mark_generates_indices() {
        let indices = GeoPathMark::generate_indices();
        assert!(indices.is_some());
        assert_eq!(indices.unwrap().len(), GeoPathMark::index_count().unwrap());
    }

    #[test]
    fn test_geo_path_attribute_types() {
        assert_eq!(
            GeoPathMark::get_attribute_type("position").unwrap(),
            "vec2<f32>"
        );
        assert_eq!(
            GeoPathMark::get_attribute_type("fill_color").unwrap(),
            "vec4<f32>"
        );
        assert_eq!(
            GeoPathMark::get_attribute_type("stroke_width").unwrap(),
            "f32"
        );
        assert!(GeoPathMark::get_attribute_type("invalid").is_err());
    }
}

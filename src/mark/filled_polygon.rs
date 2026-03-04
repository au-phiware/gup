// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Filled polygon mark for rendering tessellated polygon fills.
//!
//! The `FilledPolygon` mark converts closed polygon outlines into filled
//! triangle meshes using CPU-side ear-clipping tessellation, then renders
//! them via instanced triangles with per-vertex colour interpolation.
//!
//! # Architecture
//!
//! Each polygon is tessellated into triangles. Every triangle becomes one
//! GPU instance whose three vertex positions and three vertex colours are
//! stored in a [`TriangleInstance`]. The vertex shader uses barycentric
//! coordinates to interpolate position and colour, and the GPU rasteriser
//! produces smooth gradient fills between vertices automatically.
//!
//! # Example
//!
//! ```rust,ignore
//! use gup::mark::filled_polygon::{FilledPolygon, tessellate_polygon};
//!
//! let vertices = vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
//! let color = [0.2, 0.5, 0.8, 1.0];
//! let triangles = tessellate_polygon(&vertices, None, color);
//! // Each element is a TriangleInstance ready for GPU upload.
//! ```

use crate::mark::Mark;
use crate::selection::{AttrValue, MarkInstanceBuilder};
use crate::shader_function::{Vec2, Vec4};
use crate::shader_pipeline::ComposableShaderPipeline;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Mark struct
// ---------------------------------------------------------------------------

/// Filled polygon mark for rendering tessellated filled regions.
///
/// Unlike point marks (Circle, Rectangle) which stamp a fixed template,
/// `FilledPolygon` uses instanced *triangles*: each instance stores three
/// vertex positions and three vertex colours, allowing arbitrary convex and
/// concave polygon fills with per-vertex gradient interpolation.
#[derive(Debug, Clone, gup_macros::MarkTypeId)]
#[mark_type_id = 10]
pub struct FilledPolygon;

// ---------------------------------------------------------------------------
// GPU vertex type
// ---------------------------------------------------------------------------

/// GPU vertex for the base triangle geometry.
///
/// The three vertices of the unit triangle encode barycentric coordinates:
/// - `(0, 0)` — maps to instance vertex 0
/// - `(1, 0)` — maps to instance vertex 1
/// - `(0, 1)` — maps to instance vertex 2
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct FilledPolygonVertex {
    /// Barycentric position within the unit triangle.
    pub position: [f32; 2],
}

// ---------------------------------------------------------------------------
// Attribute value type
// ---------------------------------------------------------------------------

/// High-level attributes for a single tessellated triangle.
///
/// Each triangle carries three vertex positions and three vertex colours.
/// The vertex shader interpolates between them.
#[derive(Debug, Clone)]
pub struct FilledPolygonAttributes {
    /// Position of triangle vertex 0.
    pub v0: Vec2,
    /// Position of triangle vertex 1.
    pub v1: Vec2,
    /// Position of triangle vertex 2.
    pub v2: Vec2,
    /// Colour at vertex 0.
    pub color0: Vec4,
    /// Colour at vertex 1.
    pub color1: Vec4,
    /// Colour at vertex 2.
    pub color2: Vec4,
}

// ---------------------------------------------------------------------------
// GPU instance type
// ---------------------------------------------------------------------------

/// GPU-ready instance data for one tessellated triangle.
///
/// Layout matches the WGSL `TriangleInstance` struct in
/// `filled_polygon.vert.wgsl`. Fields are aligned to satisfy WGSL
/// storage-buffer alignment rules (vec4 → 16-byte aligned).
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TriangleInstance {
    /// Position of vertex 0.
    pub v0: [f32; 2],
    /// Position of vertex 1.
    pub v1: [f32; 2],
    /// Position of vertex 2.
    pub v2: [f32; 2],
    /// Padding to align `color0` to 16 bytes.
    pub _pad0: [f32; 2],
    /// Colour at vertex 0 (RGBA).
    pub color0: [f32; 4],
    /// Colour at vertex 1 (RGBA).
    pub color1: [f32; 4],
    /// Colour at vertex 2 (RGBA).
    pub color2: [f32; 4],
}

impl From<&FilledPolygonAttributes> for TriangleInstance {
    fn from(attrs: &FilledPolygonAttributes) -> Self {
        Self {
            v0: [attrs.v0.x, attrs.v0.y],
            v1: [attrs.v1.x, attrs.v1.y],
            v2: [attrs.v2.x, attrs.v2.y],
            _pad0: [0.0; 2],
            color0: [
                attrs.color0.x,
                attrs.color0.y,
                attrs.color0.z,
                attrs.color0.w,
            ],
            color1: [
                attrs.color1.x,
                attrs.color1.y,
                attrs.color1.z,
                attrs.color1.w,
            ],
            color2: [
                attrs.color2.x,
                attrs.color2.y,
                attrs.color2.z,
                attrs.color2.w,
            ],
        }
    }
}

impl From<FilledPolygonAttributes> for TriangleInstance {
    fn from(attrs: FilledPolygonAttributes) -> Self {
        Self::from(&attrs)
    }
}

// ---------------------------------------------------------------------------
// Mark trait implementation
// ---------------------------------------------------------------------------

impl Mark for FilledPolygon {
    type Vertex = FilledPolygonVertex;
    type AttributeValue = FilledPolygonAttributes;

    const VERTEX_SHADER: Option<&'static str> =
        Some(include_str!("shaders/filled_polygon.vert.wgsl"));

    const FRAGMENT_SHADER: Option<&'static str> =
        Some(include_str!("shaders/filled_polygon.frag.wgsl"));

    fn generate_vertex_shader(pipeline: &ComposableShaderPipeline) -> String {
        Self::generate_vertex_shader_with_functions(pipeline, &HashMap::new())
    }

    fn generate_vertex_shader_with_functions(
        pipeline: &ComposableShaderPipeline,
        _attribute_functions: &HashMap<String, String>,
    ) -> String {
        let mut shader = String::new();
        shader.push_str(
            "// Generated FilledPolygon vertex shader with shader function integration\n\n",
        );

        shader.push_str("struct TriangleInstance {\n");
        shader.push_str("    v0: vec2<f32>,\n");
        shader.push_str("    v1: vec2<f32>,\n");
        shader.push_str("    v2: vec2<f32>,\n");
        shader.push_str("    _pad: vec2<f32>,\n");
        shader.push_str("    color0: vec4<f32>,\n");
        shader.push_str("    color1: vec4<f32>,\n");
        shader.push_str("    color2: vec4<f32>,\n");
        shader.push_str("}\n\n");

        shader.push_str(
            "@group(0) @binding(0) var<storage, read> instances: array<TriangleInstance>;\n\n",
        );

        shader.push_str("struct VertexInput {\n");
        shader.push_str("    @location(0) position: vec2<f32>,\n");
        shader.push_str("    @builtin(instance_index) instance_index: u32,\n");
        shader.push_str("}\n\n");

        shader.push_str("struct VertexOutput {\n");
        shader.push_str("    @builtin(position) clip_position: vec4<f32>,\n");
        shader.push_str("    @location(0) color: vec4<f32>,\n");
        shader.push_str("}\n\n");

        let pipeline_functions = pipeline.generate_vertex_shader();
        if !pipeline_functions.is_empty() {
            shader.push_str("// Shader function definitions\n");
            shader.push_str(&pipeline_functions);
            shader.push_str("\n\n");
        }

        shader.push_str("@vertex\n");
        shader.push_str("fn vs_main(input: VertexInput) -> VertexOutput {\n");
        shader.push_str("    let instance = instances[input.instance_index];\n");
        shader.push_str("    let w0 = 1.0 - input.position.x - input.position.y;\n");
        shader.push_str("    let w1 = input.position.x;\n");
        shader.push_str("    let w2 = input.position.y;\n");
        shader.push_str("    let pos = instance.v0 * w0 + instance.v1 * w1 + instance.v2 * w2;\n");
        shader.push_str(
            "    let color = instance.color0 * w0 + instance.color1 * w1 + instance.color2 * w2;\n",
        );
        shader.push_str("    var output: VertexOutput;\n");
        shader.push_str("    output.clip_position = vec4<f32>(pos, 0.0, 1.0);\n");
        shader.push_str("    output.color = color;\n");
        shader.push_str("    return output;\n");
        shader.push_str("}\n");

        shader
    }

    fn generate_fragment_shader(pipeline: &ComposableShaderPipeline) -> String {
        Self::generate_fragment_shader_with_functions(pipeline, &HashMap::new())
    }

    fn generate_fragment_shader_with_functions(
        pipeline: &ComposableShaderPipeline,
        _attribute_functions: &HashMap<String, String>,
    ) -> String {
        let mut shader = String::new();
        shader.push_str("// Generated FilledPolygon fragment shader\n\n");

        shader.push_str("struct VertexOutput {\n");
        shader.push_str("    @builtin(position) clip_position: vec4<f32>,\n");
        shader.push_str("    @location(0) color: vec4<f32>,\n");
        shader.push_str("}\n\n");

        let pipeline_functions = pipeline.generate_fragment_shader();
        if !pipeline_functions.is_empty() {
            shader.push_str("// Shader function definitions\n");
            shader.push_str(&pipeline_functions);
            shader.push_str("\n\n");
        }

        shader.push_str("@fragment\n");
        shader.push_str("fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {\n");
        shader.push_str("    return input.color;\n");
        shader.push_str("}\n");

        shader
    }

    fn get_attribute_type(attribute_name: &str) -> crate::error::GupResult<&'static str> {
        match attribute_name {
            "v0" | "v1" | "v2" | "position" => Ok("vec2<f32>"),
            "color0" | "color1" | "color2" | "color" => Ok("vec4<f32>"),
            _ => Err(crate::error::GupError::validation_error(format!(
                "Unknown FilledPolygon attribute: {attribute_name}"
            ))),
        }
    }

    fn is_attribute_compatible(attribute_name: &str, output_type: &str) -> bool {
        match Self::get_attribute_type(attribute_name) {
            Ok(expected_type) => expected_type == output_type,
            Err(_) => false,
        }
    }

    fn vertex_count() -> usize {
        3
    }

    fn index_count() -> Option<usize> {
        Some(3)
    }

    fn generate_vertices() -> Vec<Self::Vertex> {
        vec![
            FilledPolygonVertex {
                position: [0.0, 0.0],
            }, // Vertex 0 (barycentric w0 = 1)
            FilledPolygonVertex {
                position: [1.0, 0.0],
            }, // Vertex 1 (barycentric w1 = 1)
            FilledPolygonVertex {
                position: [0.0, 1.0],
            }, // Vertex 2 (barycentric w2 = 1)
        ]
    }

    fn generate_indices() -> Option<Vec<u32>> {
        Some(vec![0, 1, 2])
    }

    fn svg_element(&self) -> Option<crate::export::svg::SvgElement> {
        Some(crate::export::svg::SvgElement::Polygon {
            points: "0,0 1,0 0,1".to_string(),
            fill: "rgb(0,0,0)".to_string(),
            stroke: None,
            stroke_width: None,
        })
    }
}

// ---------------------------------------------------------------------------
// Default attributes
// ---------------------------------------------------------------------------

impl Default for FilledPolygonAttributes {
    fn default() -> Self {
        Self {
            v0: Vec2 { x: 0.0, y: 0.0 },
            v1: Vec2 { x: 1.0, y: 0.0 },
            v2: Vec2 { x: 0.0, y: 1.0 },
            color0: Vec4 {
                x: 1.0,
                y: 1.0,
                z: 1.0,
                w: 1.0,
            },
            color1: Vec4 {
                x: 1.0,
                y: 1.0,
                z: 1.0,
                w: 1.0,
            },
            color2: Vec4 {
                x: 1.0,
                y: 1.0,
                z: 1.0,
                w: 1.0,
            },
        }
    }
}

// ---------------------------------------------------------------------------
// MarkInstanceBuilder
// ---------------------------------------------------------------------------

impl MarkInstanceBuilder for FilledPolygon {
    type Instance = TriangleInstance;

    fn default_instance() -> Self::Instance {
        TriangleInstance::from(&FilledPolygonAttributes::default())
    }

    fn build_instance(attrs: &[(&str, AttrValue)]) -> Self::Instance {
        let mut instance = Self::default_instance();
        for &(name, value) in attrs {
            match name {
                "v0" => {
                    if let AttrValue::Vec2(v) = value {
                        instance.v0 = v;
                    }
                }
                "v1" => {
                    if let AttrValue::Vec2(v) = value {
                        instance.v1 = v;
                    }
                }
                "v2" => {
                    if let AttrValue::Vec2(v) = value {
                        instance.v2 = v;
                    }
                }
                "color0" | "color" => {
                    if let AttrValue::Vec4(v) = value {
                        instance.color0 = v;
                        // If setting "color", apply to all vertices for uniform fill.
                        if name == "color" {
                            instance.color1 = v;
                            instance.color2 = v;
                        }
                    }
                }
                "color1" => {
                    if let AttrValue::Vec4(v) = value {
                        instance.color1 = v;
                    }
                }
                "color2" => {
                    if let AttrValue::Vec4(v) = value {
                        instance.color2 = v;
                    }
                }
                _ => {} // Ignore unknown attributes.
            }
        }
        instance
    }
}

// ---------------------------------------------------------------------------
// AccessibleMark
// ---------------------------------------------------------------------------

impl crate::selection::AccessibleMark for FilledPolygon {
    fn describe_point(
        index: usize,
        total: usize,
        attrs: &[(&str, crate::selection::AttrValue)],
    ) -> String {
        use crate::selection::AttrValue;

        let mut parts = vec![format!("Polygon triangle {} of {}", index + 1, total)];
        for &(name, value) in attrs {
            match (name, value) {
                ("color", AttrValue::Vec4(c)) => {
                    parts.push(format!(
                        "colour rgba({:.0}%, {:.0}%, {:.0}%, {:.2})",
                        c[0] * 100.0,
                        c[1] * 100.0,
                        c[2] * 100.0,
                        c[3]
                    ));
                }
                _ => {}
            }
        }
        parts.join(", ")
    }

    fn describe_mark_type() -> &'static str {
        "filled polygon"
    }
}

// ---------------------------------------------------------------------------
// Tessellation
// ---------------------------------------------------------------------------

/// Tessellate a closed polygon into [`TriangleInstance`]s using ear clipping.
///
/// # Arguments
///
/// * `vertices` — closed polygon vertices in winding order. The last vertex
///   is automatically connected back to the first; do **not** repeat the
///   first vertex at the end.
/// * `vertex_colors` — optional per-vertex colours. When `Some`, must have
///   the same length as `vertices`; colours are interpolated per-triangle.
///   When `None`, every vertex receives `default_color`.
/// * `default_color` — fallback RGBA colour when `vertex_colors` is `None`.
///
/// # Returns
///
/// A `Vec<TriangleInstance>` ready for GPU upload. Returns an empty vec for
/// degenerate polygons (fewer than 3 unique vertices).
pub fn tessellate_polygon(
    vertices: &[[f32; 2]],
    vertex_colors: Option<&[[f32; 4]]>,
    default_color: [f32; 4],
) -> Vec<TriangleInstance> {
    if vertices.len() < 3 {
        return Vec::new();
    }

    // Remove closing duplicate if present.
    let pts: &[[f32; 2]] = if vertices.len() > 3
        && (vertices.first().unwrap()[0] - vertices.last().unwrap()[0]).abs() < 1e-6
        && (vertices.first().unwrap()[1] - vertices.last().unwrap()[1]).abs() < 1e-6
    {
        &vertices[..vertices.len() - 1]
    } else {
        vertices
    };

    if pts.len() < 3 {
        return Vec::new();
    }

    // Ensure CCW winding.
    let area = signed_area_f32(pts);
    let mut poly: Vec<[f32; 2]> = pts.to_vec();
    let mut colors: Vec<[f32; 4]> = match vertex_colors {
        Some(c) => {
            let slice = if c.len() > pts.len() {
                &c[..pts.len()]
            } else {
                c
            };
            let mut v = slice.to_vec();
            v.resize(pts.len(), default_color);
            v
        }
        None => vec![default_color; pts.len()],
    };

    if area < 0.0 {
        poly.reverse();
        colors.reverse();
    }

    let n = poly.len();
    let mut indices: Vec<usize> = (0..n).collect();
    let mut instances: Vec<TriangleInstance> = Vec::with_capacity(n.saturating_sub(2));
    let mut remaining = n;
    let mut fail_count = 0;
    let mut i = 0;

    while remaining > 2 {
        if fail_count >= remaining {
            break; // Degenerate polygon — bail out.
        }

        let prev = indices[(i + remaining - 1) % remaining];
        let curr = indices[i % remaining];
        let next = indices[(i + 1) % remaining];

        if is_ear_f32(&poly, &indices, remaining, prev, curr, next) {
            instances.push(TriangleInstance {
                v0: poly[prev],
                v1: poly[curr],
                v2: poly[next],
                _pad0: [0.0; 2],
                color0: colors[prev],
                color1: colors[curr],
                color2: colors[next],
            });
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

    instances
}

/// Signed area of a polygon (positive = CCW, negative = CW).
fn signed_area_f32(ring: &[[f32; 2]]) -> f32 {
    let n = ring.len();
    let mut area: f32 = 0.0;
    for i in 0..n {
        let j = (i + 1) % n;
        area += ring[i][0] * ring[j][1];
        area -= ring[j][0] * ring[i][1];
    }
    area * 0.5
}

/// Check whether the triangle (prev, curr, next) is a valid ear.
fn is_ear_f32(
    poly: &[[f32; 2]],
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
    if cross_2d_f32(a, b, c) <= 0.0 {
        return false;
    }

    // No other vertex inside the triangle.
    for k in 0..count {
        let idx = indices[k];
        if idx == prev || idx == curr || idx == next {
            continue;
        }
        if point_in_triangle_f32(poly[idx], a, b, c) {
            return false;
        }
    }

    true
}

/// 2D cross product of vectors AB and AC.
fn cross_2d_f32(a: [f32; 2], b: [f32; 2], c: [f32; 2]) -> f32 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}

/// Test whether point P lies strictly inside triangle ABC.
fn point_in_triangle_f32(p: [f32; 2], a: [f32; 2], b: [f32; 2], c: [f32; 2]) -> bool {
    let d1 = cross_2d_f32(a, b, p);
    let d2 = cross_2d_f32(b, c, p);
    let d3 = cross_2d_f32(c, a, p);
    let has_neg = (d1 < 0.0) || (d2 < 0.0) || (d3 < 0.0);
    let has_pos = (d1 > 0.0) || (d2 > 0.0) || (d3 > 0.0);
    !(has_neg && has_pos)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mark::Mark;

    #[test]
    fn vertex_count_is_three() {
        assert_eq!(FilledPolygon::vertex_count(), 3);
    }

    #[test]
    fn generate_vertices_returns_barycentric_triangle() {
        let verts = FilledPolygon::generate_vertices();
        assert_eq!(verts.len(), 3);
        assert_eq!(verts[0].position, [0.0, 0.0]);
        assert_eq!(verts[1].position, [1.0, 0.0]);
        assert_eq!(verts[2].position, [0.0, 1.0]);
    }

    #[test]
    fn generate_indices_returns_single_triangle() {
        let indices = FilledPolygon::generate_indices().unwrap();
        assert_eq!(indices, vec![0, 1, 2]);
    }

    // ── Tessellation tests ──────────────────────────────────────────

    #[test]
    fn tessellate_triangle_produces_one_instance() {
        let verts = vec![[0.0, 0.0], [1.0, 0.0], [0.5, 1.0]];
        let color = [1.0, 0.0, 0.0, 1.0];
        let instances = tessellate_polygon(&verts, None, color);
        assert_eq!(instances.len(), 1);
        assert_eq!(instances[0].color0, color);
        assert_eq!(instances[0].color1, color);
        assert_eq!(instances[0].color2, color);
    }

    #[test]
    fn tessellate_square_produces_two_triangles() {
        let verts = vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
        let color = [0.0, 1.0, 0.0, 1.0];
        let instances = tessellate_polygon(&verts, None, color);
        assert_eq!(instances.len(), 2);
    }

    #[test]
    fn tessellate_pentagon_produces_three_triangles() {
        // Regular pentagon (approx).
        let verts = vec![
            [0.0, 1.0],
            [-0.95, 0.31],
            [-0.59, -0.81],
            [0.59, -0.81],
            [0.95, 0.31],
        ];
        let color = [0.0, 0.0, 1.0, 1.0];
        let instances = tessellate_polygon(&verts, None, color);
        assert_eq!(instances.len(), 3); // n - 2 = 5 - 2
    }

    #[test]
    fn tessellate_with_per_vertex_colors() {
        let verts = vec![[0.0, 0.0], [1.0, 0.0], [0.5, 1.0]];
        let colors = vec![
            [1.0, 0.0, 0.0, 1.0], // red
            [0.0, 1.0, 0.0, 1.0], // green
            [0.0, 0.0, 1.0, 1.0], // blue
        ];
        let instances = tessellate_polygon(&verts, Some(&colors), [0.0; 4]);
        assert_eq!(instances.len(), 1);
        // All three colours should be present (order may vary due to winding).
        let all_colors = [
            instances[0].color0,
            instances[0].color1,
            instances[0].color2,
        ];
        assert!(all_colors.contains(&[1.0, 0.0, 0.0, 1.0]));
        assert!(all_colors.contains(&[0.0, 1.0, 0.0, 1.0]));
        assert!(all_colors.contains(&[0.0, 0.0, 1.0, 1.0]));
    }

    #[test]
    fn tessellate_degenerate_polygon_returns_empty() {
        // Fewer than 3 vertices.
        assert!(tessellate_polygon(&[], None, [1.0; 4]).is_empty());
        assert!(tessellate_polygon(&[[0.0, 0.0]], None, [1.0; 4]).is_empty());
        assert!(tessellate_polygon(&[[0.0, 0.0], [1.0, 0.0]], None, [1.0; 4]).is_empty());
    }

    #[test]
    fn tessellate_removes_closing_duplicate() {
        // Square with closing vertex duplicate.
        let verts = vec![
            [0.0, 0.0],
            [1.0, 0.0],
            [1.0, 1.0],
            [0.0, 1.0],
            [0.0, 0.0], // duplicate
        ];
        let instances = tessellate_polygon(&verts, None, [1.0; 4]);
        assert_eq!(instances.len(), 2);
    }

    #[test]
    fn tessellate_clockwise_polygon_is_handled() {
        // CW winding order — should be reversed internally.
        let verts = vec![[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]];
        let color = [1.0, 0.5, 0.0, 1.0];
        let instances = tessellate_polygon(&verts, None, color);
        assert_eq!(instances.len(), 2);
    }

    #[test]
    fn tessellate_concave_polygon() {
        // L-shaped polygon (concave).
        let verts = vec![
            [0.0, 0.0],
            [2.0, 0.0],
            [2.0, 1.0],
            [1.0, 1.0],
            [1.0, 2.0],
            [0.0, 2.0],
        ];
        let color = [0.5, 0.5, 0.5, 1.0];
        let instances = tessellate_polygon(&verts, None, color);
        assert_eq!(instances.len(), 4); // 6 - 2
    }

    // ── Instance builder tests ──────────────────────────────────────

    #[test]
    fn build_instance_default() {
        let instance = FilledPolygon::default_instance();
        assert_eq!(instance.v0, [0.0, 0.0]);
        assert_eq!(instance.v1, [1.0, 0.0]);
        assert_eq!(instance.v2, [0.0, 1.0]);
        assert_eq!(instance.color0, [1.0, 1.0, 1.0, 1.0]);
    }

    #[test]
    fn build_instance_with_color_sets_all_vertices() {
        let instance =
            FilledPolygon::build_instance(&[("color", AttrValue::Vec4([0.5, 0.0, 0.0, 1.0]))]);
        assert_eq!(instance.color0, [0.5, 0.0, 0.0, 1.0]);
        assert_eq!(instance.color1, [0.5, 0.0, 0.0, 1.0]);
        assert_eq!(instance.color2, [0.5, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn build_instance_with_per_vertex_positions() {
        let instance = FilledPolygon::build_instance(&[
            ("v0", AttrValue::Vec2([10.0, 20.0])),
            ("v1", AttrValue::Vec2([30.0, 40.0])),
            ("v2", AttrValue::Vec2([50.0, 60.0])),
        ]);
        assert_eq!(instance.v0, [10.0, 20.0]);
        assert_eq!(instance.v1, [30.0, 40.0]);
        assert_eq!(instance.v2, [50.0, 60.0]);
    }

    // ── Alignment / Pod safety ──────────────────────────────────────

    #[test]
    fn triangle_instance_size_and_alignment() {
        assert_eq!(
            std::mem::size_of::<TriangleInstance>(),
            80,
            "TriangleInstance should be 80 bytes"
        );
        assert_eq!(std::mem::offset_of!(TriangleInstance, v0), 0);
        assert_eq!(std::mem::offset_of!(TriangleInstance, v1), 8);
        assert_eq!(std::mem::offset_of!(TriangleInstance, v2), 16);
        assert_eq!(std::mem::offset_of!(TriangleInstance, _pad0), 24);
        assert_eq!(std::mem::offset_of!(TriangleInstance, color0), 32);
        assert_eq!(std::mem::offset_of!(TriangleInstance, color1), 48);
        assert_eq!(std::mem::offset_of!(TriangleInstance, color2), 64);
    }

    // ── Attribute type checks ───────────────────────────────────────

    #[test]
    fn attribute_types_correct() {
        assert_eq!(
            FilledPolygon::get_attribute_type("v0").unwrap(),
            "vec2<f32>"
        );
        assert_eq!(
            FilledPolygon::get_attribute_type("color0").unwrap(),
            "vec4<f32>"
        );
        assert!(FilledPolygon::get_attribute_type("unknown").is_err());
    }

    // ── Large polygon stress test ───────────────────────────────────

    #[test]
    fn tessellate_large_polygon_10000_vertices() {
        // Circle approximation with 10 000 vertices.
        let n = 10_000;
        let verts: Vec<[f32; 2]> = (0..n)
            .map(|i| {
                let angle = 2.0 * std::f32::consts::PI * i as f32 / n as f32;
                [angle.cos(), angle.sin()]
            })
            .collect();

        let start = std::time::Instant::now();
        let instances = tessellate_polygon(&verts, None, [1.0, 1.0, 1.0, 1.0]);
        let elapsed = start.elapsed();

        assert_eq!(instances.len(), n - 2);
        // Should complete in well under 1 second on any reasonable hardware.
        assert!(
            elapsed.as_secs() < 5,
            "Tessellation of {n} vertices took {elapsed:?}, expected <5s"
        );
    }
}

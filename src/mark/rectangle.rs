// Gup - GPU-Accelerated Data Visualization Library
// Copyright (C) 2025 Corin Lawson <corin@phiware.com.au>
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

//! Rectangle mark implementation for efficient rectangular visualizations.
//!
//! The Rectangle mark provides optimized rendering of rectangular shapes with support for
//! instanced rendering, custom colors, stroke properties, rounded corners, and GPU-accelerated
//! attribute transformations.

use crate::mark::Mark;
use crate::shader_function::{Vec2, Vec4};
use crate::shader_pipeline::ComposableShaderPipeline;
use std::collections::HashMap;

/// Rectangle mark for rendering rectangular shapes and bars.
///
/// This mark is designed for efficient visualization of rectangular data where each
/// data point is represented as a rectangle. It supports:
/// - Variable size per rectangle
/// - Fill and stroke colors
/// - Rounded corners with configurable radius
/// - Instanced GPU rendering for high performance
/// - Integration with shader function system
///
/// # Examples
///
/// ```rust
/// use gup::mark::{Rectangle, RectangleAttributes, Mark};
/// use gup::{vec2, vec4, Vec2, Vec4};
///
/// // Create rectangle attributes
/// let attrs = RectangleAttributes {
///     center: vec2![100.0, 200.0],
///     size: vec2![50.0, 30.0],
///     fill_color: vec4![0.0, 1.0, 0.0, 1.0], // Green
///     stroke_width: 1.0,
///     stroke_color: vec4![0.0, 0.0, 0.0, 1.0], // Black border
///     corner_radius: 5.0,
/// };
///
/// // Rectangle vertices are generated automatically
/// let vertices = Rectangle::generate_vertices();
/// assert_eq!(vertices.len(), 4); // Quad for instanced rendering
/// ```
#[derive(Debug, Clone, gup_macros::MarkTypeId)]
#[mark_type_id = 1]
pub struct Rectangle;

/// GPU vertex data for rectangle rendering.
///
/// Each vertex represents a corner of the quad used for instanced rectangle rendering.
/// The actual rectangle shape and rounded corners are computed in the fragment shader.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct RectangleVertex {
    /// Local position within the unit quad (-0.5 to 0.5 in both dimensions)
    pub position: [f32; 2],
}

/// High-level attributes for configuring rectangle appearance.
///
/// These attributes define the visual properties of each rectangle instance.
/// The data is processed by shader functions to generate final GPU vertex data.
#[derive(Debug, Clone)]
pub struct RectangleAttributes {
    /// Center position of the rectangle in world coordinates
    pub center: Vec2,
    /// Size of the rectangle (width, height) in world units
    pub size: Vec2,
    /// Fill color (RGBA values from 0.0 to 1.0)
    pub fill_color: Vec4,
    /// Stroke width in pixels
    pub stroke_width: f32,
    /// Stroke color (RGBA values from 0.0 to 1.0)
    pub stroke_color: Vec4,
    /// Corner radius for rounded rectangles (0.0 = sharp corners)
    pub corner_radius: f32,
}

/// GPU-ready instance data for rectangle rendering.
///
/// This struct matches the WGSL `RectangleInstance` layout in `rectangle.vert.wgsl`
/// and is suitable for upload to a storage buffer. Fields are aligned to
/// satisfy WGSL storage buffer alignment rules (vec4 → 16-byte aligned).
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct RectangleInstance {
    /// Center position in clip space
    pub center: [f32; 2],
    /// Size (width, height) in clip space units
    pub size: [f32; 2],
    /// Fill color (RGBA)
    pub fill_color: [f32; 4],
    /// Stroke width in clip space units
    pub stroke_width: f32,
    /// Padding for vec4 alignment of stroke_color
    pub _pad1: [f32; 3],
    /// Stroke color (RGBA)
    pub stroke_color: [f32; 4],
    /// Corner radius for rounded rectangles
    pub corner_radius: f32,
    /// Padding to match WGSL struct _padding field
    pub _padding: f32,
    /// Padding to align struct size to 16 bytes
    pub _pad2: [f32; 2],
}

impl From<&RectangleAttributes> for RectangleInstance {
    fn from(attrs: &RectangleAttributes) -> Self {
        Self {
            center: [attrs.center.x, attrs.center.y],
            size: [attrs.size.x, attrs.size.y],
            fill_color: [
                attrs.fill_color.x,
                attrs.fill_color.y,
                attrs.fill_color.z,
                attrs.fill_color.w,
            ],
            stroke_width: attrs.stroke_width,
            _pad1: [0.0; 3],
            stroke_color: [
                attrs.stroke_color.x,
                attrs.stroke_color.y,
                attrs.stroke_color.z,
                attrs.stroke_color.w,
            ],
            corner_radius: attrs.corner_radius,
            _padding: 0.0,
            _pad2: [0.0; 2],
        }
    }
}

impl From<RectangleAttributes> for RectangleInstance {
    fn from(attrs: RectangleAttributes) -> Self {
        Self::from(&attrs)
    }
}

impl Mark for Rectangle {
    type Vertex = RectangleVertex;
    type AttributeValue = RectangleAttributes;

    /// High-performance hand-optimized vertex shader for rectangles.
    ///
    /// This shader uses instanced rendering with a unit quad and performs
    /// rectangle computations in the fragment shader for smooth edges and rounded corners.
    const VERTEX_SHADER: Option<&'static str> = Some(include_str!("shaders/rectangle.vert.wgsl"));

    /// High-performance hand-optimized fragment shader for rectangles.
    ///
    /// Uses distance field calculations for anti-aliased rectangles with
    /// proper stroke rendering and rounded corner support.
    const FRAGMENT_SHADER: Option<&'static str> = Some(include_str!("shaders/rectangle.frag.wgsl"));

    /// Pattern-enabled fragment shader for rectangles.
    ///
    /// Integrates pattern rendering for accessibility support while maintaining
    /// all standard rectangle features (stroke, rounded corners, anti-aliasing).
    const PATTERN_FRAGMENT_SHADER: Option<&'static str> =
        Some(include_str!("shaders/rectangle_pattern.frag.wgsl"));

    /// Generate vertex shader with shader function integration.
    ///
    /// When using generated shaders, this method creates WGSL that integrates
    /// with the shader function pipeline for dynamic attribute mapping.
    fn generate_vertex_shader(pipeline: &ComposableShaderPipeline) -> String {
        Self::generate_vertex_shader_with_functions(pipeline, &HashMap::new())
    }

    /// Generate vertex shader with specific shader function mappings.
    ///
    /// This implementation creates a vertex shader that applies shader functions
    /// to transform data into rectangle attributes (center, size, colors).
    fn generate_vertex_shader_with_functions(
        pipeline: &ComposableShaderPipeline,
        attribute_functions: &HashMap<String, String>,
    ) -> String {
        // Generate data structures
        let mut shader = String::new();
        shader
            .push_str("// Generated Rectangle vertex shader with shader function integration\n\n");

        // Data input structure
        shader.push_str("struct DataInput {\n");
        shader.push_str("    index: u32,\n");
        shader.push_str("}\n\n");

        // Rectangle instance structure
        shader.push_str("struct RectangleInstance {\n");
        shader.push_str("    center: vec2<f32>,\n");
        shader.push_str("    size: vec2<f32>,\n");
        shader.push_str("    fill_color: vec4<f32>,\n");
        shader.push_str("    stroke_width: f32,\n");
        shader.push_str("    stroke_color: vec4<f32>,\n");
        shader.push_str("    corner_radius: f32,\n");
        shader.push_str("}\n\n");

        // Storage buffers
        shader
            .push_str("@group(0) @binding(0) var<storage, read> data_buffer: array<DataInput>;\n");
        shader.push_str(
            "@group(1) @binding(0) var<storage, read> instances: array<RectangleInstance>;\n\n",
        );

        // Vertex input/output structures
        shader.push_str("struct VertexInput {\n");
        shader.push_str("    @location(0) position: vec2<f32>,\n");
        shader.push_str("    @builtin(instance_index) instance_index: u32,\n");
        shader.push_str("}\n\n");

        shader.push_str("struct VertexOutput {\n");
        shader.push_str("    @builtin(position) clip_position: vec4<f32>,\n");
        shader.push_str("    @location(0) world_position: vec2<f32>,\n");
        shader.push_str("    @location(1) local_position: vec2<f32>,\n");
        shader.push_str("    @location(2) fill_color: vec4<f32>,\n");
        shader.push_str("    @location(3) stroke_color: vec4<f32>,\n");
        shader.push_str("    @location(4) size: vec2<f32>,\n");
        shader.push_str("    @location(5) stroke_width: f32,\n");
        shader.push_str("    @location(6) corner_radius: f32,\n");
        shader.push_str("}\n\n");

        // Add shader function definitions from pipeline
        let pipeline_functions = pipeline.generate_vertex_shader();
        if !pipeline_functions.is_empty() {
            shader.push_str("// Shader function definitions\n");
            shader.push_str(&pipeline_functions);
            shader.push_str("\n\n");
        }

        // Main vertex function
        shader.push_str("@vertex\n");
        shader.push_str("fn vs_main(input: VertexInput) -> VertexOutput {\n");
        shader.push_str("    let data = data_buffer[input.instance_index];\n");
        shader.push_str("    let instance = instances[input.instance_index];\n\n");

        // Apply shader functions to transform attributes
        let mut transformed_center = "instance.center".to_string();
        let mut transformed_size = "instance.size".to_string();
        let mut transformed_fill_color = "instance.fill_color".to_string();

        if let Some(position_fn) = attribute_functions.get("position") {
            shader.push_str(&format!(
                "    let transformed_center = {position_fn}(data, position_uniforms);\n"
            ));
            transformed_center = "transformed_center".to_string();
        }

        if let Some(size_fn) = attribute_functions.get("size") {
            shader.push_str(&format!(
                "    let transformed_size = {size_fn}(data, size_uniforms);\n"
            ));
            transformed_size = "transformed_size".to_string();
        }

        if let Some(color_fn) = attribute_functions.get("color") {
            shader.push_str(&format!(
                "    let transformed_fill_color = {color_fn}(data, color_uniforms);\n"
            ));
            transformed_fill_color = "transformed_fill_color".to_string();
        }

        shader.push('\n');

        // Calculate final world position
        shader.push_str(&format!(
            "    let world_pos = input.position * {transformed_size} + {transformed_center};\n"
        ));
        shader.push('\n');

        // Generate output
        shader.push_str("    var output: VertexOutput;\n");
        shader.push_str("    output.clip_position = vec4<f32>(world_pos, 0.0, 1.0);\n");
        shader.push_str("    output.world_position = world_pos;\n");
        shader.push_str("    output.local_position = input.position;\n");
        shader.push_str(&format!(
            "    output.fill_color = {transformed_fill_color};\n"
        ));
        shader.push_str("    output.stroke_color = instance.stroke_color;\n");
        shader.push_str(&format!("    output.size = {transformed_size};\n"));
        shader.push_str("    output.stroke_width = instance.stroke_width;\n");
        shader.push_str("    output.corner_radius = instance.corner_radius;\n");
        shader.push('\n');
        shader.push_str("    return output;\n");
        shader.push_str("}\n");

        shader
    }

    /// Generate fragment shader with anti-aliasing and rounded corners support.
    ///
    /// Creates a fragment shader that renders smooth rectangles using distance
    /// field calculations and integrates with the shader function system.
    fn generate_fragment_shader(pipeline: &ComposableShaderPipeline) -> String {
        Self::generate_fragment_shader_with_functions(pipeline, &HashMap::new())
    }

    /// Generate fragment shader with specific shader function mappings.
    ///
    /// This implementation creates an anti-aliased rectangle fragment shader that
    /// uses attributes computed by shader functions in the vertex stage.
    fn generate_fragment_shader_with_functions(
        pipeline: &ComposableShaderPipeline,
        _attribute_functions: &HashMap<String, String>,
    ) -> String {
        let mut shader = String::new();
        shader.push_str("// Generated Rectangle fragment shader with anti-aliased rendering\n\n");

        // Add vertex output structure (must match vertex shader)
        shader.push_str("struct VertexOutput {\n");
        shader.push_str("    @builtin(position) clip_position: vec4<f32>,\n");
        shader.push_str("    @location(0) world_position: vec2<f32>,\n");
        shader.push_str("    @location(1) local_position: vec2<f32>,\n");
        shader.push_str("    @location(2) fill_color: vec4<f32>,\n");
        shader.push_str("    @location(3) stroke_color: vec4<f32>,\n");
        shader.push_str("    @location(4) size: vec2<f32>,\n");
        shader.push_str("    @location(5) stroke_width: f32,\n");
        shader.push_str("    @location(6) corner_radius: f32,\n");
        shader.push_str("}\n\n");

        // Add shader function definitions if needed
        let pipeline_functions = pipeline.generate_fragment_shader();
        if !pipeline_functions.is_empty() {
            shader.push_str("// Shader function definitions\n");
            shader.push_str(&pipeline_functions);
            shader.push_str("\n\n");
        }

        // SDF function for rounded rectangles
        shader.push_str("// Signed distance function for rounded rectangle\n");
        shader.push_str("fn sdf_rounded_rectangle(pos: vec2<f32>, size: vec2<f32>, corner_radius: f32) -> f32 {\n");
        shader.push_str("    let half_size = size * 0.5;\n");
        shader.push_str("    let corner_offset = max(abs(pos) - (half_size - corner_radius), vec2<f32>(0.0));\n");
        shader.push_str("    let distance_to_corner = length(corner_offset);\n");
        shader.push_str("    let distance_to_edge = max(\n");
        shader.push_str("        max(abs(pos).x - half_size.x, abs(pos).y - half_size.y),\n");
        shader.push_str("        distance_to_corner - corner_radius\n");
        shader.push_str("    );\n");
        shader.push_str("    return distance_to_edge;\n");
        shader.push_str("}\n\n");

        // Main fragment function with anti-aliased rectangle rendering
        shader.push_str("@fragment\n");
        shader.push_str("fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {\n");
        shader.push_str("    // Calculate position in rectangle coordinate system\n");
        shader.push_str("    let pos = input.local_position * input.size;\n");
        shader.push('\n');
        shader.push_str("    // Calculate distance to rectangle edge using SDF\n");
        shader.push_str("    let distance_to_edge = sdf_rounded_rectangle(pos, input.size, input.corner_radius);\n");
        shader.push('\n');
        shader.push_str("    // Calculate stroke boundaries\n");
        shader.push_str("    let outer_distance = distance_to_edge;\n");
        shader.push_str("    let inner_distance = distance_to_edge + input.stroke_width;\n");
        shader.push('\n');
        shader.push_str("    // Anti-aliasing edge width\n");
        shader.push_str("    let edge_width = 0.5;\n");
        shader.push('\n');
        shader.push_str("    // Calculate alpha values with smooth transitions\n");
        shader.push_str(
            "    let outer_alpha = 1.0 - smoothstep(-edge_width, edge_width, outer_distance);\n",
        );
        shader.push('\n');
        shader.push_str("    // Handle stroke rendering\n");
        shader.push_str("    if (input.stroke_width > 0.0) {\n");
        shader.push_str(
            "        let inner_alpha = smoothstep(-edge_width, edge_width, inner_distance);\n",
        );
        shader.push_str("        let stroke_alpha = outer_alpha * (1.0 - inner_alpha);\n");
        shader.push_str("        let fill_alpha = outer_alpha * inner_alpha;\n");
        shader.push('\n');
        shader.push_str("        // Blend stroke and fill colors\n");
        shader.push_str("        let final_color = input.stroke_color * stroke_alpha + input.fill_color * fill_alpha;\n");
        shader.push_str(
            "        return vec4<f32>(final_color.rgb, max(stroke_alpha, fill_alpha));\n",
        );
        shader.push_str("    } else {\n");
        shader.push_str("        // No stroke - just fill\n");
        shader.push_str(
            "        return vec4<f32>(input.fill_color.rgb, input.fill_color.a * outer_alpha);\n",
        );
        shader.push_str("    }\n");
        shader.push_str("}\n");

        shader
    }

    /// Number of vertices in the rectangle quad (4 vertices for instanced rendering)
    fn vertex_count() -> usize {
        4
    }

    /// Number of indices for the rectangle quad (6 indices for 2 triangles)
    fn index_count() -> Option<usize> {
        Some(6)
    }

    /// Generate vertices for a unit quad used in instanced rectangle rendering.
    ///
    /// The quad covers the range [-0.5, 0.5] in both dimensions, centered at origin.
    /// The actual rectangle shape and rounded corners are computed in the fragment shader.
    fn generate_vertices() -> Vec<Self::Vertex> {
        vec![
            RectangleVertex {
                position: [-0.5, -0.5],
            }, // Bottom-left
            RectangleVertex {
                position: [0.5, -0.5],
            }, // Bottom-right
            RectangleVertex {
                position: [0.5, 0.5],
            }, // Top-right
            RectangleVertex {
                position: [-0.5, 0.5],
            }, // Top-left
        ]
    }

    /// Generate indices for the rectangle quad (two triangles).
    ///
    /// Uses counter-clockwise winding order for proper face culling.
    fn generate_indices() -> Option<Vec<u32>> {
        Some(vec![
            0, 1, 2, // First triangle: bottom-left, bottom-right, top-right
            0, 2, 3, // Second triangle: bottom-left, top-right, top-left
        ])
    }

    /// Get the WGSL type name for a Rectangle attribute.
    fn get_attribute_type(attribute_name: &str) -> crate::error::GupResult<&'static str> {
        match attribute_name {
            "center" | "position" => Ok("vec2<f32>"),
            "size" | "width" | "height" => Ok("vec2<f32>"),
            "fill_color" | "color" | "stroke_color" => Ok("vec4<f32>"),
            "stroke_width" | "corner_radius" => Ok("f32"),
            _ => Err(crate::error::GupError::validation_error(format!(
                "Unknown Rectangle attribute: {attribute_name}"
            ))),
        }
    }

    /// Check if a shader function output type is compatible with a Rectangle attribute.
    fn is_attribute_compatible(attribute_name: &str, output_type: &str) -> bool {
        match Self::get_attribute_type(attribute_name) {
            Ok(expected_type) => expected_type == output_type,
            Err(_) => false,
        }
    }
}

impl Default for RectangleAttributes {
    /// Default rectangle attributes for testing and prototyping.
    fn default() -> Self {
        Self {
            center: Vec2 { x: 0.0, y: 0.0 },
            size: Vec2 { x: 10.0, y: 10.0 },
            fill_color: Vec4 {
                x: 1.0,
                y: 1.0,
                z: 1.0,
                w: 1.0,
            }, // White
            stroke_width: 1.0,
            stroke_color: Vec4 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: 1.0,
            }, // Black
            corner_radius: 0.0, // Sharp corners by default
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{vec2, vec4};

    #[test]
    fn test_rectangle_mark_implementation() {
        // Test basic mark trait methods
        assert_eq!(Rectangle::vertex_count(), 4);
        assert_eq!(Rectangle::index_count(), Some(6));

        let vertices = Rectangle::generate_vertices();
        assert_eq!(vertices.len(), 4);

        // Verify vertex positions
        assert_eq!(vertices[0].position, [-0.5, -0.5]);
        assert_eq!(vertices[1].position, [0.5, -0.5]);
        assert_eq!(vertices[2].position, [0.5, 0.5]);
        assert_eq!(vertices[3].position, [-0.5, 0.5]);

        let indices = Rectangle::generate_indices().unwrap();
        assert_eq!(indices.len(), 6);
        assert_eq!(indices, vec![0, 1, 2, 0, 2, 3]);
    }

    #[test]
    fn test_rectangle_shaders() {
        // Verify that custom shaders are provided
        assert!(Rectangle::VERTEX_SHADER.is_some());
        assert!(Rectangle::FRAGMENT_SHADER.is_some());
    }

    #[test]
    fn test_rectangle_attributes() {
        let attrs = RectangleAttributes {
            center: vec2![10.0, 20.0],
            size: vec2![50.0, 30.0],
            fill_color: vec4![0.0, 1.0, 0.0, 1.0],
            stroke_width: 2.0,
            stroke_color: vec4![0.0, 0.0, 0.0, 1.0],
            corner_radius: 5.0,
        };

        assert_eq!(attrs.center.x, 10.0);
        assert_eq!(attrs.center.y, 20.0);
        assert_eq!(attrs.size.x, 50.0);
        assert_eq!(attrs.size.y, 30.0);
        assert_eq!(attrs.fill_color.y, 1.0); // Green
        assert_eq!(attrs.stroke_width, 2.0);
        assert_eq!(attrs.stroke_color.w, 1.0); // Opaque
        assert_eq!(attrs.corner_radius, 5.0);
    }

    #[test]
    fn test_rectangle_attributes_default() {
        let default_attrs = RectangleAttributes::default();
        assert_eq!(default_attrs.center.x, 0.0);
        assert_eq!(default_attrs.center.y, 0.0);
        assert_eq!(default_attrs.size.x, 10.0);
        assert_eq!(default_attrs.size.y, 10.0);
        assert_eq!(default_attrs.fill_color.x, 1.0); // White
        assert_eq!(default_attrs.stroke_width, 1.0);
        assert_eq!(default_attrs.corner_radius, 0.0); // Sharp corners
    }

    #[test]
    fn test_vertex_buffer_compatibility() {
        let vertices = Rectangle::generate_vertices();

        // Verify vertex data is valid for GPU upload
        for vertex in &vertices {
            assert!(vertex.position[0].is_finite());
            assert!(vertex.position[1].is_finite());
            assert!(vertex.position[0] >= -0.5 && vertex.position[0] <= 0.5);
            assert!(vertex.position[1] >= -0.5 && vertex.position[1] <= 0.5);
        }

        // Verify bytemuck conversion works
        let bytes: &[u8] = bytemuck::cast_slice(&vertices);
        assert_eq!(
            bytes.len(),
            vertices.len() * std::mem::size_of::<RectangleVertex>()
        );
    }

    #[test]
    fn test_rectangle_vertex_properties() {
        // Verify vertex type implements required traits
        let vertex = RectangleVertex {
            position: [0.25, -0.25],
        };

        // Should be able to clone and debug
        let _cloned = vertex;
        println!("{vertex:?}");

        // Bytemuck conversion should work
        let bytes = bytemuck::bytes_of(&vertex);
        assert_eq!(bytes.len(), std::mem::size_of::<RectangleVertex>());
    }

    #[test]
    fn test_rectangle_attributes_properties() {
        // Test that attributes can be constructed with vec macros
        let attrs = RectangleAttributes {
            center: vec2![1.0, 2.0],
            size: vec2![20.0, 15.0],
            fill_color: vec4![0.5, 0.5, 0.5, 0.8],
            stroke_width: 1.5,
            stroke_color: vec4![0.2, 0.2, 0.2, 1.0],
            corner_radius: 3.0,
        };

        // Verify values are set correctly
        assert_eq!(attrs.center.x, 1.0);
        assert_eq!(attrs.center.y, 2.0);
        assert_eq!(attrs.size.x, 20.0);
        assert_eq!(attrs.size.y, 15.0);
        assert_eq!(attrs.fill_color.w, 0.8); // Alpha
        assert_eq!(attrs.stroke_width, 1.5);
        assert_eq!(attrs.corner_radius, 3.0);
    }

    #[test]
    fn test_rectangle_attribute_type_validation() {
        // Test that Rectangle provides correct attribute types
        assert_eq!(
            Rectangle::get_attribute_type("center").unwrap(),
            "vec2<f32>"
        );
        assert_eq!(
            Rectangle::get_attribute_type("position").unwrap(),
            "vec2<f32>"
        );
        assert_eq!(Rectangle::get_attribute_type("size").unwrap(), "vec2<f32>");
        assert_eq!(Rectangle::get_attribute_type("width").unwrap(), "vec2<f32>");
        assert_eq!(
            Rectangle::get_attribute_type("height").unwrap(),
            "vec2<f32>"
        );
        assert_eq!(
            Rectangle::get_attribute_type("fill_color").unwrap(),
            "vec4<f32>"
        );
        assert_eq!(Rectangle::get_attribute_type("color").unwrap(), "vec4<f32>");
        assert_eq!(
            Rectangle::get_attribute_type("stroke_color").unwrap(),
            "vec4<f32>"
        );
        assert_eq!(
            Rectangle::get_attribute_type("stroke_width").unwrap(),
            "f32"
        );
        assert_eq!(
            Rectangle::get_attribute_type("corner_radius").unwrap(),
            "f32"
        );

        // Test unknown attribute
        assert!(Rectangle::get_attribute_type("unknown").is_err());
    }

    #[test]
    fn test_rectangle_attribute_compatibility() {
        // Test compatible attribute types
        assert!(Rectangle::is_attribute_compatible("center", "vec2<f32>"));
        assert!(Rectangle::is_attribute_compatible("position", "vec2<f32>"));
        assert!(Rectangle::is_attribute_compatible("size", "vec2<f32>"));
        assert!(Rectangle::is_attribute_compatible(
            "fill_color",
            "vec4<f32>"
        ));
        assert!(Rectangle::is_attribute_compatible("color", "vec4<f32>"));
        assert!(Rectangle::is_attribute_compatible(
            "stroke_color",
            "vec4<f32>"
        ));
        assert!(Rectangle::is_attribute_compatible("stroke_width", "f32"));
        assert!(Rectangle::is_attribute_compatible("corner_radius", "f32"));

        // Test incompatible types
        assert!(!Rectangle::is_attribute_compatible("center", "f32"));
        assert!(!Rectangle::is_attribute_compatible("size", "vec4<f32>"));
        assert!(!Rectangle::is_attribute_compatible(
            "fill_color",
            "vec2<f32>"
        ));
        assert!(!Rectangle::is_attribute_compatible(
            "stroke_width",
            "vec4<f32>"
        ));

        // Test unknown attribute
        assert!(!Rectangle::is_attribute_compatible("unknown", "f32"));
    }

    #[test]
    fn test_rectangle_shader_generation() {
        use crate::shader_pipeline::ComposableShaderPipeline;
        use std::collections::HashMap;

        let pipeline = ComposableShaderPipeline::new();
        let mut attribute_functions = HashMap::new();
        attribute_functions.insert("position".to_string(), "position_transform".to_string());
        attribute_functions.insert("color".to_string(), "color_mapping".to_string());
        attribute_functions.insert("size".to_string(), "size_transform".to_string());

        // Test vertex shader generation
        let vertex_shader =
            Rectangle::generate_vertex_shader_with_functions(&pipeline, &attribute_functions);
        assert!(vertex_shader.contains("vs_main"));
        assert!(vertex_shader.contains("RectangleInstance"));
        assert!(vertex_shader.contains("VertexOutput"));
        assert!(vertex_shader.contains("position_transform"));
        assert!(vertex_shader.contains("color_mapping"));
        assert!(vertex_shader.contains("size_transform"));
        assert!(vertex_shader.contains("corner_radius"));

        // Test fragment shader generation
        let fragment_shader =
            Rectangle::generate_fragment_shader_with_functions(&pipeline, &attribute_functions);
        assert!(fragment_shader.contains("fs_main"));
        assert!(fragment_shader.contains("sdf_rounded_rectangle"));
        assert!(fragment_shader.contains("smoothstep"));
        assert!(fragment_shader.contains("anti-aliased"));
    }

    #[test]
    fn test_rectangle_shader_generation_without_functions() {
        use crate::shader_pipeline::ComposableShaderPipeline;
        use std::collections::HashMap;

        let pipeline = ComposableShaderPipeline::new();
        let attribute_functions = HashMap::new(); // No shader functions

        // Test that shaders are still generated correctly without shader functions
        let vertex_shader =
            Rectangle::generate_vertex_shader_with_functions(&pipeline, &attribute_functions);
        assert!(vertex_shader.contains("vs_main"));
        assert!(vertex_shader.contains("instance.center")); // Uses default instance data

        let fragment_shader =
            Rectangle::generate_fragment_shader_with_functions(&pipeline, &attribute_functions);
        assert!(fragment_shader.contains("fs_main"));
        assert!(fragment_shader.contains("sdf_rounded_rectangle"));
    }
}

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

//! Line mark implementation for efficient line segment visualizations.
//!
//! The Line mark provides optimized rendering of line segments with support for
//! instanced rendering, custom colors, variable width, line styles (solid, dashed, dotted),
//! and GPU-accelerated attribute transformations.

use crate::mark::Mark;
use crate::shader_function::{Vec2, Vec4};
use crate::shader_pipeline::ComposableShaderPipeline;
use std::collections::HashMap;

/// Line mark for rendering line segments and connections.
///
/// This mark is designed for efficient visualization of line data where each
/// data element is represented as a line segment. It supports:
/// - Variable width per line
/// - Different line styles (solid, dashed, dotted)
/// - Custom colors
/// - Instanced GPU rendering for high performance
/// - Integration with shader function system
///
/// # Examples
///
/// ```rust
/// use gup::mark::{Line, LineAttributes, LineStyle, Mark};
/// use gup::{vec2, vec4, Vec2, Vec4};
///
/// // Create line attributes
/// let attrs = LineAttributes {
///     start: vec2![0.0, 0.0],
///     end: vec2![100.0, 50.0],
///     color: vec4![1.0, 0.0, 0.0, 1.0], // Red
///     width: 2.0,
///     style: LineStyle::Solid,
/// };
///
/// // Line vertices are generated automatically
/// let vertices = Line::generate_vertices();
/// assert_eq!(vertices.len(), 4); // Quad for instanced rendering
/// ```
#[derive(Debug, Clone)]
pub struct Line;

/// Line style enumeration for different line appearances.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
#[derive(Default)]
pub enum LineStyle {
    /// Solid continuous line
    #[default]
    Solid = 0,
    /// Dashed line with regular gaps
    Dashed = 1,
    /// Dotted line with small regular gaps
    Dotted = 2,
}

/// GPU vertex data for line rendering.
///
/// Each vertex represents a corner of the quad used for instanced line rendering.
/// The actual line shape and style are computed in the fragment shader.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LineVertex {
    /// Local position within the line quad (0 to 1 along line, -0.5 to 0.5 across)
    pub position: [f32; 2],
    /// Normal vector for width expansion
    pub normal: [f32; 2],
}

/// High-level attributes for configuring line appearance.
///
/// These attributes define the visual properties of each line instance.
/// The data is processed by shader functions to generate final GPU vertex data.
#[derive(Debug, Clone)]
pub struct LineAttributes {
    /// Start position of the line in world coordinates
    pub start: Vec2,
    /// End position of the line in world coordinates
    pub end: Vec2,
    /// Line color (RGBA values from 0.0 to 1.0)
    pub color: Vec4,
    /// Line width in pixels
    pub width: f32,
    /// Line style (solid, dashed, dotted)
    pub style: LineStyle,
}

impl Mark for Line {
    type Vertex = LineVertex;
    type AttributeValue = LineAttributes;

    /// High-performance hand-optimized vertex shader for lines.
    ///
    /// This shader uses instanced rendering with a unit quad and performs
    /// line computations including proper width expansion and orientation.
    const VERTEX_SHADER: Option<&'static str> = Some(include_str!("shaders/line.vert.wgsl"));

    /// High-performance hand-optimized fragment shader for lines.
    ///
    /// Uses distance field calculations for anti-aliased lines with
    /// proper style rendering (solid, dashed, dotted).
    const FRAGMENT_SHADER: Option<&'static str> = Some(include_str!("shaders/line.frag.wgsl"));

    /// Pattern-enabled fragment shader for lines.
    ///
    /// Integrates pattern rendering for accessibility support while maintaining
    /// all standard line features (style, anti-aliasing).
    const PATTERN_FRAGMENT_SHADER: Option<&'static str> =
        Some(include_str!("shaders/line_pattern.frag.wgsl"));

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
    /// to transform data into line attributes (start, end, colors, width).
    fn generate_vertex_shader_with_functions(
        pipeline: &ComposableShaderPipeline,
        attribute_functions: &HashMap<String, String>,
    ) -> String {
        // Generate data structures
        let mut shader = String::new();
        shader.push_str("// Generated Line vertex shader with shader function integration\n\n");

        // Data input structure
        shader.push_str("struct DataInput {\n");
        shader.push_str("    index: u32,\n");
        shader.push_str("}\n\n");

        // Line instance structure
        shader.push_str("struct LineInstance {\n");
        shader.push_str("    start: vec2<f32>,\n");
        shader.push_str("    end: vec2<f32>,\n");
        shader.push_str("    color: vec4<f32>,\n");
        shader.push_str("    width: f32,\n");
        shader.push_str("    style: u32,\n");
        shader.push_str("}\n\n");

        // Storage buffers
        shader
            .push_str("@group(0) @binding(0) var<storage, read> data_buffer: array<DataInput>;\n");
        shader.push_str(
            "@group(1) @binding(0) var<storage, read> instances: array<LineInstance>;\n\n",
        );

        // Vertex input/output structures
        shader.push_str("struct VertexInput {\n");
        shader.push_str("    @location(0) position: vec2<f32>,\n");
        shader.push_str("    @location(1) normal: vec2<f32>,\n");
        shader.push_str("    @builtin(instance_index) instance_index: u32,\n");
        shader.push_str("}\n\n");

        shader.push_str("struct VertexOutput {\n");
        shader.push_str("    @builtin(position) clip_position: vec4<f32>,\n");
        shader.push_str("    @location(0) world_position: vec2<f32>,\n");
        shader.push_str("    @location(1) local_position: vec2<f32>,\n");
        shader.push_str("    @location(2) color: vec4<f32>,\n");
        shader.push_str("    @location(3) width: f32,\n");
        shader.push_str("    @location(4) style: u32,\n");
        shader.push_str("    @location(5) line_length: f32,\n");
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
        let mut transformed_start = "instance.start".to_string();
        let mut transformed_end = "instance.end".to_string();
        let mut transformed_color = "instance.color".to_string();
        let mut transformed_width = "instance.width".to_string();

        if let Some(start_fn) = attribute_functions.get("start") {
            shader.push_str(&format!(
                "    let transformed_start = {start_fn}(data, start_uniforms);\n"
            ));
            transformed_start = "transformed_start".to_string();
        }

        if let Some(end_fn) = attribute_functions.get("end") {
            shader.push_str(&format!(
                "    let transformed_end = {end_fn}(data, end_uniforms);\n"
            ));
            transformed_end = "transformed_end".to_string();
        }

        if let Some(color_fn) = attribute_functions.get("color") {
            shader.push_str(&format!(
                "    let transformed_color = {color_fn}(data, color_uniforms);\n"
            ));
            transformed_color = "transformed_color".to_string();
        }

        if let Some(width_fn) = attribute_functions.get("width") {
            shader.push_str(&format!(
                "    let transformed_width = {width_fn}(data, width_uniforms);\n"
            ));
            transformed_width = "transformed_width".to_string();
        }

        shader.push('\n');

        // Calculate line direction and length
        shader.push_str(&format!(
            "    let line_vec = {transformed_end} - {transformed_start};\n"
        ));
        shader.push_str("    let line_length = length(line_vec);\n");
        shader.push_str("    let line_dir = normalize(line_vec);\n");
        shader.push_str("    let line_normal = vec2<f32>(-line_dir.y, line_dir.x);\n\n");

        // Calculate world position along the line
        shader.push_str(&format!(
            "    let along_line = {transformed_start} + line_dir * (input.position.x * line_length);\n"
        ));
        shader.push_str(&format!(
            "    let across_line = line_normal * (input.normal.y * {transformed_width} * 0.5);\n"
        ));
        shader.push_str("    let world_pos = along_line + across_line;\n\n");

        // Generate output
        shader.push_str("    var output: VertexOutput;\n");
        shader.push_str("    output.clip_position = vec4<f32>(world_pos, 0.0, 1.0);\n");
        shader.push_str("    output.world_position = world_pos;\n");
        shader.push_str("    output.local_position = input.position;\n");
        shader.push_str(&format!("    output.color = {transformed_color};\n"));
        shader.push_str(&format!("    output.width = {transformed_width};\n"));
        shader.push_str("    output.style = instance.style;\n");
        shader.push_str("    output.line_length = line_length;\n");
        shader.push('\n');
        shader.push_str("    return output;\n");
        shader.push_str("}\n");

        shader
    }

    /// Generate fragment shader with anti-aliasing and line style support.
    ///
    /// Creates a fragment shader that renders smooth lines using distance
    /// field calculations and integrates with the shader function system.
    fn generate_fragment_shader(pipeline: &ComposableShaderPipeline) -> String {
        Self::generate_fragment_shader_with_functions(pipeline, &HashMap::new())
    }

    /// Generate fragment shader with specific shader function mappings.
    ///
    /// This implementation creates an anti-aliased line fragment shader that
    /// uses attributes computed by shader functions in the vertex stage.
    fn generate_fragment_shader_with_functions(
        pipeline: &ComposableShaderPipeline,
        _attribute_functions: &HashMap<String, String>,
    ) -> String {
        let mut shader = String::new();
        shader.push_str("// Generated Line fragment shader with anti-aliased rendering\n\n");

        // Add vertex output structure (must match vertex shader)
        shader.push_str("struct VertexOutput {\n");
        shader.push_str("    @builtin(position) clip_position: vec4<f32>,\n");
        shader.push_str("    @location(0) world_position: vec2<f32>,\n");
        shader.push_str("    @location(1) local_position: vec2<f32>,\n");
        shader.push_str("    @location(2) color: vec4<f32>,\n");
        shader.push_str("    @location(3) width: f32,\n");
        shader.push_str("    @location(4) style: u32,\n");
        shader.push_str("    @location(5) line_length: f32,\n");
        shader.push_str("}\n\n");

        // Add shader function definitions if needed
        let pipeline_functions = pipeline.generate_fragment_shader();
        if !pipeline_functions.is_empty() {
            shader.push_str("// Shader function definitions\n");
            shader.push_str(&pipeline_functions);
            shader.push_str("\n\n");
        }

        // Main fragment function with anti-aliased line rendering
        shader.push_str("@fragment\n");
        shader.push_str("fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {\n");
        shader.push_str("    // Calculate distance from line center\n");
        shader.push_str("    let distance_from_center = abs(input.local_position.y);\n");
        shader.push('\n');
        shader.push_str("    // Calculate base alpha for line width with anti-aliasing\n");
        shader.push_str("    let half_width = 0.5;\n");
        shader.push_str("    let edge_width = 0.02;\n");
        shader.push_str("    let base_alpha = 1.0 - smoothstep(half_width - edge_width, half_width + edge_width, distance_from_center);\n");
        shader.push('\n');
        shader.push_str("    // Apply line style patterns\n");
        shader.push_str("    var style_alpha = 1.0;\n");
        shader.push_str("    let pattern_coord = input.local_position.x * input.line_length;\n");
        shader.push('\n');
        shader.push_str("    if (input.style == 1u) { // Dashed\n");
        shader.push_str("        let dash_size = 10.0;\n");
        shader.push_str("        let gap_size = 5.0;\n");
        shader.push_str("        let cycle = dash_size + gap_size;\n");
        shader.push_str("        let position_in_cycle = fract(pattern_coord / cycle) * cycle;\n");
        shader.push_str("        style_alpha = select(0.0, 1.0, position_in_cycle < dash_size);\n");
        shader.push('\n');
        shader.push_str("        // Smooth dash transitions\n");
        shader.push_str("        let transition_width = 1.0;\n");
        shader.push_str("        if (position_in_cycle < transition_width) {\n");
        shader.push_str(
            "            style_alpha *= smoothstep(0.0, transition_width, position_in_cycle);\n",
        );
        shader.push_str("        } else if (position_in_cycle > dash_size - transition_width) {\n");
        shader.push_str("            style_alpha *= smoothstep(dash_size, dash_size - transition_width, position_in_cycle);\n");
        shader.push_str("        }\n");
        shader.push_str("    } else if (input.style == 2u) { // Dotted\n");
        shader.push_str("        let dot_spacing = 8.0;\n");
        shader.push_str("        let dot_size = 3.0;\n");
        shader.push_str(
            "        let position_in_cycle = fract(pattern_coord / dot_spacing) * dot_spacing;\n",
        );
        shader.push_str(
            "        let distance_from_dot_center = abs(position_in_cycle - dot_spacing * 0.5);\n",
        );
        shader.push_str("        style_alpha = 1.0 - smoothstep(dot_size * 0.5 - 0.5, dot_size * 0.5 + 0.5, distance_from_dot_center);\n");
        shader.push_str("    }\n");
        shader.push('\n');
        shader.push_str("    // Combine alpha values\n");
        shader.push_str("    let final_alpha = base_alpha * style_alpha;\n");
        shader.push('\n');
        shader.push_str("    return vec4<f32>(input.color.rgb, input.color.a * final_alpha);\n");
        shader.push_str("}\n");

        shader
    }

    /// Number of vertices in the line quad (4 vertices for instanced rendering)
    fn vertex_count() -> usize {
        4
    }

    /// Number of indices for the line quad (6 indices for 2 triangles)
    fn index_count() -> Option<usize> {
        Some(6)
    }

    /// Generate vertices for a unit quad used in instanced line rendering.
    ///
    /// The quad represents the line segment from 0 to 1 along its length,
    /// with width expansion handled by normals.
    fn generate_vertices() -> Vec<Self::Vertex> {
        vec![
            LineVertex {
                position: [0.0, -0.5],
                normal: [0.0, -1.0],
            }, // Start bottom
            LineVertex {
                position: [1.0, -0.5],
                normal: [0.0, -1.0],
            }, // End bottom
            LineVertex {
                position: [1.0, 0.5],
                normal: [0.0, 1.0],
            }, // End top
            LineVertex {
                position: [0.0, 0.5],
                normal: [0.0, 1.0],
            }, // Start top
        ]
    }

    /// Generate indices for the line quad (two triangles).
    ///
    /// Uses counter-clockwise winding order for proper face culling.
    fn generate_indices() -> Option<Vec<u32>> {
        Some(vec![
            0, 1, 2, // First triangle: start-bottom, end-bottom, end-top
            0, 2, 3, // Second triangle: start-bottom, end-top, start-top
        ])
    }

    /// Get the WGSL type name for a Line attribute.
    fn get_attribute_type(attribute_name: &str) -> crate::error::GupResult<&'static str> {
        match attribute_name {
            "start" | "end" | "position" => Ok("vec2<f32>"),
            "color" => Ok("vec4<f32>"),
            "width" | "size" => Ok("f32"),
            "style" => Ok("u32"),
            _ => Err(crate::error::GupError::validation_error(format!(
                "Unknown Line attribute: {attribute_name}"
            ))),
        }
    }

    /// Check if a shader function output type is compatible with a Line attribute.
    fn is_attribute_compatible(attribute_name: &str, output_type: &str) -> bool {
        match Self::get_attribute_type(attribute_name) {
            Ok(expected_type) => expected_type == output_type,
            Err(_) => false,
        }
    }
}

impl Default for LineAttributes {
    /// Default line attributes for testing and prototyping.
    fn default() -> Self {
        Self {
            start: Vec2 { x: 0.0, y: 0.0 },
            end: Vec2 { x: 10.0, y: 0.0 },
            color: Vec4 {
                x: 1.0,
                y: 1.0,
                z: 1.0,
                w: 1.0,
            }, // White
            width: 1.0,
            style: LineStyle::Solid,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{vec2, vec4};

    #[test]
    fn test_line_mark_implementation() {
        // Test basic mark trait methods
        assert_eq!(Line::vertex_count(), 4);
        assert_eq!(Line::index_count(), Some(6));

        let vertices = Line::generate_vertices();
        assert_eq!(vertices.len(), 4);

        // Verify vertex positions
        assert_eq!(vertices[0].position, [0.0, -0.5]);
        assert_eq!(vertices[1].position, [1.0, -0.5]);
        assert_eq!(vertices[2].position, [1.0, 0.5]);
        assert_eq!(vertices[3].position, [0.0, 0.5]);

        // Verify normals
        assert_eq!(vertices[0].normal, [0.0, -1.0]);
        assert_eq!(vertices[1].normal, [0.0, -1.0]);
        assert_eq!(vertices[2].normal, [0.0, 1.0]);
        assert_eq!(vertices[3].normal, [0.0, 1.0]);

        let indices = Line::generate_indices().unwrap();
        assert_eq!(indices.len(), 6);
        assert_eq!(indices, vec![0, 1, 2, 0, 2, 3]);
    }

    #[test]
    fn test_line_shaders() {
        // Verify that custom shaders are provided
        assert!(Line::VERTEX_SHADER.is_some());
        assert!(Line::FRAGMENT_SHADER.is_some());
    }

    #[test]
    fn test_line_style_enum() {
        assert_eq!(LineStyle::Solid as u32, 0);
        assert_eq!(LineStyle::Dashed as u32, 1);
        assert_eq!(LineStyle::Dotted as u32, 2);

        let default_style = LineStyle::default();
        assert_eq!(default_style, LineStyle::Solid);
    }

    #[test]
    fn test_line_attributes() {
        let attrs = LineAttributes {
            start: vec2![10.0, 20.0],
            end: vec2![50.0, 80.0],
            color: vec4![1.0, 0.0, 0.0, 1.0],
            width: 3.0,
            style: LineStyle::Dashed,
        };

        assert_eq!(attrs.start.x, 10.0);
        assert_eq!(attrs.start.y, 20.0);
        assert_eq!(attrs.end.x, 50.0);
        assert_eq!(attrs.end.y, 80.0);
        assert_eq!(attrs.color.x, 1.0); // Red
        assert_eq!(attrs.width, 3.0);
        assert_eq!(attrs.style, LineStyle::Dashed);
    }

    #[test]
    fn test_line_attributes_default() {
        let default_attrs = LineAttributes::default();
        assert_eq!(default_attrs.start.x, 0.0);
        assert_eq!(default_attrs.start.y, 0.0);
        assert_eq!(default_attrs.end.x, 10.0);
        assert_eq!(default_attrs.end.y, 0.0);
        assert_eq!(default_attrs.color.x, 1.0); // White
        assert_eq!(default_attrs.width, 1.0);
        assert_eq!(default_attrs.style, LineStyle::Solid);
    }

    #[test]
    fn test_vertex_buffer_compatibility() {
        let vertices = Line::generate_vertices();

        // Verify vertex data is valid for GPU upload
        for vertex in &vertices {
            assert!(vertex.position[0].is_finite());
            assert!(vertex.position[1].is_finite());
            assert!(vertex.normal[0].is_finite());
            assert!(vertex.normal[1].is_finite());

            // Check position bounds
            assert!(vertex.position[0] >= 0.0 && vertex.position[0] <= 1.0);
            assert!(vertex.position[1] >= -0.5 && vertex.position[1] <= 0.5);

            // Check normal magnitude
            let normal_length =
                (vertex.normal[0] * vertex.normal[0] + vertex.normal[1] * vertex.normal[1]).sqrt();
            assert!((normal_length - 1.0).abs() < 0.001); // Should be unit normal
        }

        // Verify bytemuck conversion works
        let bytes: &[u8] = bytemuck::cast_slice(&vertices);
        assert_eq!(
            bytes.len(),
            vertices.len() * std::mem::size_of::<LineVertex>()
        );
    }

    #[test]
    fn test_line_vertex_properties() {
        // Verify vertex type implements required traits
        let vertex = LineVertex {
            position: [0.5, 0.0],
            normal: [0.0, 1.0],
        };

        // Should be able to clone and debug
        let _cloned = vertex;
        println!("{vertex:?}");

        // Bytemuck conversion should work
        let bytes = bytemuck::bytes_of(&vertex);
        assert_eq!(bytes.len(), std::mem::size_of::<LineVertex>());
    }

    #[test]
    fn test_line_attributes_properties() {
        // Test that attributes can be constructed with vec macros
        let attrs = LineAttributes {
            start: vec2![1.0, 2.0],
            end: vec2![10.0, 20.0],
            color: vec4![0.5, 0.5, 0.5, 0.8],
            width: 2.5,
            style: LineStyle::Dotted,
        };

        // Verify values are set correctly
        assert_eq!(attrs.start.x, 1.0);
        assert_eq!(attrs.start.y, 2.0);
        assert_eq!(attrs.end.x, 10.0);
        assert_eq!(attrs.end.y, 20.0);
        assert_eq!(attrs.color.w, 0.8); // Alpha
        assert_eq!(attrs.width, 2.5);
        assert_eq!(attrs.style, LineStyle::Dotted);
    }

    #[test]
    fn test_line_attribute_type_validation() {
        // Test that Line provides correct attribute types
        assert_eq!(Line::get_attribute_type("start").unwrap(), "vec2<f32>");
        assert_eq!(Line::get_attribute_type("end").unwrap(), "vec2<f32>");
        assert_eq!(Line::get_attribute_type("position").unwrap(), "vec2<f32>");
        assert_eq!(Line::get_attribute_type("color").unwrap(), "vec4<f32>");
        assert_eq!(Line::get_attribute_type("width").unwrap(), "f32");
        assert_eq!(Line::get_attribute_type("size").unwrap(), "f32");
        assert_eq!(Line::get_attribute_type("style").unwrap(), "u32");

        // Test unknown attribute
        assert!(Line::get_attribute_type("unknown").is_err());
    }

    #[test]
    fn test_line_attribute_compatibility() {
        // Test compatible attribute types
        assert!(Line::is_attribute_compatible("start", "vec2<f32>"));
        assert!(Line::is_attribute_compatible("end", "vec2<f32>"));
        assert!(Line::is_attribute_compatible("position", "vec2<f32>"));
        assert!(Line::is_attribute_compatible("color", "vec4<f32>"));
        assert!(Line::is_attribute_compatible("width", "f32"));
        assert!(Line::is_attribute_compatible("size", "f32"));
        assert!(Line::is_attribute_compatible("style", "u32"));

        // Test incompatible types
        assert!(!Line::is_attribute_compatible("start", "f32"));
        assert!(!Line::is_attribute_compatible("end", "vec4<f32>"));
        assert!(!Line::is_attribute_compatible("color", "vec2<f32>"));
        assert!(!Line::is_attribute_compatible("width", "vec4<f32>"));
        assert!(!Line::is_attribute_compatible("style", "f32"));

        // Test unknown attribute
        assert!(!Line::is_attribute_compatible("unknown", "f32"));
    }

    #[test]
    fn test_line_shader_generation() {
        use crate::shader_pipeline::ComposableShaderPipeline;
        use std::collections::HashMap;

        let pipeline = ComposableShaderPipeline::new();
        let mut attribute_functions = HashMap::new();
        attribute_functions.insert("start".to_string(), "start_transform".to_string());
        attribute_functions.insert("end".to_string(), "end_transform".to_string());
        attribute_functions.insert("color".to_string(), "color_mapping".to_string());
        attribute_functions.insert("width".to_string(), "width_transform".to_string());

        // Test vertex shader generation
        let vertex_shader =
            Line::generate_vertex_shader_with_functions(&pipeline, &attribute_functions);
        assert!(vertex_shader.contains("vs_main"));
        assert!(vertex_shader.contains("LineInstance"));
        assert!(vertex_shader.contains("VertexOutput"));
        assert!(vertex_shader.contains("start_transform"));
        assert!(vertex_shader.contains("end_transform"));
        assert!(vertex_shader.contains("color_mapping"));
        assert!(vertex_shader.contains("width_transform"));
        assert!(vertex_shader.contains("line_length"));

        // Test fragment shader generation
        let fragment_shader =
            Line::generate_fragment_shader_with_functions(&pipeline, &attribute_functions);
        assert!(fragment_shader.contains("fs_main"));
        assert!(fragment_shader.contains("distance_from_center"));
        assert!(fragment_shader.contains("smoothstep"));
        assert!(fragment_shader.contains("style_alpha"));
        assert!(fragment_shader.contains("Dashed"));
        assert!(fragment_shader.contains("Dotted"));
    }

    #[test]
    fn test_line_shader_generation_without_functions() {
        use crate::shader_pipeline::ComposableShaderPipeline;
        use std::collections::HashMap;

        let pipeline = ComposableShaderPipeline::new();
        let attribute_functions = HashMap::new(); // No shader functions

        // Test that shaders are still generated correctly without shader functions
        let vertex_shader =
            Line::generate_vertex_shader_with_functions(&pipeline, &attribute_functions);
        assert!(vertex_shader.contains("vs_main"));
        assert!(vertex_shader.contains("instance.start")); // Uses default instance data
        assert!(vertex_shader.contains("instance.end"));

        let fragment_shader =
            Line::generate_fragment_shader_with_functions(&pipeline, &attribute_functions);
        assert!(fragment_shader.contains("fs_main"));
        assert!(fragment_shader.contains("anti-aliased"));
        assert!(fragment_shader.contains("style == 1u")); // Dashed style check
        assert!(fragment_shader.contains("style == 2u")); // Dotted style check
    }
}

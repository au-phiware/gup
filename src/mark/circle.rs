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

//! Circle mark implementation for efficient circular visualizations.
//!
//! The Circle mark provides optimized rendering of circular shapes with support for
//! instanced rendering, custom colors, stroke properties, and GPU-accelerated attribute
//! transformations.

use crate::mark::Mark;
use crate::selection::{AttrValue, MarkInstanceBuilder};
use crate::shader_function::{Vec2, Vec4};
use crate::shader_pipeline::ComposableShaderPipeline;
use std::collections::HashMap;

/// Circle mark for rendering circular points and shapes.
///
/// This mark is designed for efficient visualization of point data where each
/// data point is represented as a circle. It supports:
/// - Variable radius per circle
/// - Fill and stroke colors
/// - Instanced GPU rendering for high performance
/// - Integration with shader function system
///
/// # Examples
///
/// ```rust
/// use gup::mark::{Circle, CircleAttributes, Mark};
/// use gup::{vec2, vec4, Vec2, Vec4};
///
/// // Create circle attributes
/// let attrs = CircleAttributes {
///     center: vec2![100.0, 200.0],
///     radius: 15.0,
///     fill_color: vec4![1.0, 0.0, 0.0, 1.0], // Red
///     stroke_width: 2.0,
///     stroke_color: vec4![0.0, 0.0, 0.0, 1.0], // Black border
/// };
///
/// // Circle vertices are generated automatically
/// let vertices = Circle::generate_vertices();
/// assert_eq!(vertices.len(), 4); // Quad for instanced rendering
/// ```
#[derive(Debug, Clone, gup_macros::MarkTypeId)]
#[mark_type_id = 0]
pub struct Circle;

/// GPU vertex data for circle rendering.
///
/// Each vertex represents a corner of the quad used for instanced circle rendering.
/// The actual circle shape is computed in the fragment shader using distance functions.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CircleVertex {
    /// Local position within the unit quad (-1 to 1 in both dimensions)
    pub position: [f32; 2],
}

/// High-level attributes for configuring circle appearance.
///
/// These attributes define the visual properties of each circle instance.
/// The data is processed by shader functions to generate final GPU vertex data.
#[derive(Debug, Clone)]
pub struct CircleAttributes {
    /// Center position of the circle in world coordinates
    pub center: Vec2,
    /// Radius of the circle in world units
    pub radius: f32,
    /// Fill color (RGBA values from 0.0 to 1.0)
    pub fill_color: Vec4,
    /// Stroke width in pixels
    pub stroke_width: f32,
    /// Stroke color (RGBA values from 0.0 to 1.0)
    pub stroke_color: Vec4,
}

/// GPU-ready instance data for circle rendering.
///
/// This struct matches the WGSL `CircleInstance` layout in `circle.vert.wgsl`
/// and is suitable for upload to a storage buffer. Fields are aligned to
/// satisfy WGSL storage buffer alignment rules (vec4 → 16-byte aligned).
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CircleInstance {
    /// Center position in clip space
    pub center: [f32; 2],
    /// Radius in clip space units
    pub radius: f32,
    /// Padding for vec4 alignment of fill_color
    pub _pad0: f32,
    /// Fill color (RGBA)
    pub fill_color: [f32; 4],
    /// Stroke width in clip space units
    pub stroke_width: f32,
    /// Padding for vec4 alignment of stroke_color
    pub _pad1: [f32; 3],
    /// Stroke color (RGBA)
    pub stroke_color: [f32; 4],
}

impl From<&CircleAttributes> for CircleInstance {
    fn from(attrs: &CircleAttributes) -> Self {
        Self {
            center: [attrs.center.x, attrs.center.y],
            radius: attrs.radius,
            _pad0: 0.0,
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
        }
    }
}

impl From<CircleAttributes> for CircleInstance {
    fn from(attrs: CircleAttributes) -> Self {
        Self::from(&attrs)
    }
}

impl Mark for Circle {
    type Vertex = CircleVertex;
    type AttributeValue = CircleAttributes;

    /// High-performance hand-optimized vertex shader for circles.
    ///
    /// This shader uses instanced rendering with a unit quad and performs
    /// circle computations in the fragment shader for smooth edges.
    const VERTEX_SHADER: Option<&'static str> = Some(include_str!("shaders/circle.vert.wgsl"));

    /// High-performance hand-optimized fragment shader for circles.
    ///
    /// Uses distance field calculations for anti-aliased circles with
    /// proper stroke rendering.
    const FRAGMENT_SHADER: Option<&'static str> = Some(include_str!("shaders/circle.frag.wgsl"));

    /// Pattern-enabled fragment shader for accessibility rendering.
    ///
    /// This shader integrates pattern-based rendering for colorblind users,
    /// using texture patterns instead of colors for data encoding.
    const PATTERN_FRAGMENT_SHADER: Option<&'static str> =
        Some(include_str!("shaders/circle_pattern.frag.wgsl"));

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
    /// to transform data into circle attributes (center, radius, colors).
    fn generate_vertex_shader_with_functions(
        pipeline: &ComposableShaderPipeline,
        attribute_functions: &HashMap<String, String>,
    ) -> String {
        // Generate data structures
        let mut shader = String::new();
        shader.push_str("// Generated Circle vertex shader with shader function integration\n\n");

        // Data input structure
        shader.push_str("struct DataInput {\n");
        shader.push_str("    index: u32,\n");
        shader.push_str("}\n\n");

        // Circle instance structure
        shader.push_str("struct CircleInstance {\n");
        shader.push_str("    center: vec2<f32>,\n");
        shader.push_str("    radius: f32,\n");
        shader.push_str("    fill_color: vec4<f32>,\n");
        shader.push_str("    stroke_width: f32,\n");
        shader.push_str("    stroke_color: vec4<f32>,\n");
        shader.push_str("}\n\n");

        // Storage buffers
        shader
            .push_str("@group(0) @binding(0) var<storage, read> data_buffer: array<DataInput>;\n");
        shader.push_str(
            "@group(1) @binding(0) var<storage, read> instances: array<CircleInstance>;\n\n",
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
        shader.push_str("    @location(4) radius: f32,\n");
        shader.push_str("    @location(5) stroke_width: f32,\n");
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
        let mut transformed_radius = "instance.radius".to_string();
        let mut transformed_fill_color = "instance.fill_color".to_string();

        if let Some(position_fn) = attribute_functions.get("position") {
            shader.push_str(&format!(
                "    let transformed_center = {position_fn}(data, position_uniforms);\n"
            ));
            transformed_center = "transformed_center".to_string();
        }

        if let Some(size_fn) = attribute_functions.get("size") {
            shader.push_str(&format!(
                "    let transformed_radius = {size_fn}(data, size_uniforms);\n"
            ));
            transformed_radius = "transformed_radius".to_string();
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
            "    let world_pos = input.position * {transformed_radius} + {transformed_center};\n"
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
        shader.push_str(&format!("    output.radius = {transformed_radius};\n"));
        shader.push_str("    output.stroke_width = instance.stroke_width;\n");
        shader.push('\n');
        shader.push_str("    return output;\n");
        shader.push_str("}\n");

        shader
    }

    /// Generate fragment shader with anti-aliasing support.
    ///
    /// Creates a fragment shader that renders smooth circles using distance
    /// field calculations and integrates with the shader function system.
    fn generate_fragment_shader(pipeline: &ComposableShaderPipeline) -> String {
        Self::generate_fragment_shader_with_functions(pipeline, &HashMap::new())
    }

    /// Generate fragment shader with specific shader function mappings.
    ///
    /// This implementation creates an anti-aliased circle fragment shader that
    /// uses attributes computed by shader functions in the vertex stage.
    fn generate_fragment_shader_with_functions(
        pipeline: &ComposableShaderPipeline,
        _attribute_functions: &HashMap<String, String>,
    ) -> String {
        let mut shader = String::new();
        shader.push_str("// Generated Circle fragment shader with anti-aliased rendering\n\n");

        // Add vertex output structure (must match vertex shader)
        shader.push_str("struct VertexOutput {\n");
        shader.push_str("    @builtin(position) clip_position: vec4<f32>,\n");
        shader.push_str("    @location(0) world_position: vec2<f32>,\n");
        shader.push_str("    @location(1) local_position: vec2<f32>,\n");
        shader.push_str("    @location(2) fill_color: vec4<f32>,\n");
        shader.push_str("    @location(3) stroke_color: vec4<f32>,\n");
        shader.push_str("    @location(4) radius: f32,\n");
        shader.push_str("    @location(5) stroke_width: f32,\n");
        shader.push_str("}\n\n");

        // Add shader function definitions if needed
        let pipeline_functions = pipeline.generate_fragment_shader();
        if !pipeline_functions.is_empty() {
            shader.push_str("// Shader function definitions\n");
            shader.push_str(&pipeline_functions);
            shader.push_str("\n\n");
        }

        // Main fragment function with anti-aliased circle rendering
        shader.push_str("@fragment\n");
        shader.push_str("fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {\n");
        shader.push_str("    let distance_from_center = length(input.local_position);\n");
        shader.push('\n');
        shader.push_str("    // Anti-aliased circle with stroke\n");
        shader.push_str("    let outer_radius = 1.0;\n");
        shader.push_str("    let inner_radius = max(0.0, outer_radius - (input.stroke_width / input.radius));\n");
        shader.push('\n');
        shader.push_str("    // Smooth step for anti-aliasing\n");
        shader.push_str("    let edge_width = 0.02;\n");
        shader.push('\n');
        shader.push_str("    // Calculate fill alpha\n");
        shader.push_str("    let fill_alpha = 1.0 - smoothstep(inner_radius - edge_width, inner_radius + edge_width, distance_from_center);\n");
        shader.push('\n');
        shader.push_str("    // Calculate stroke alpha\n");
        shader.push_str("    var stroke_alpha = 1.0 - smoothstep(outer_radius - edge_width, outer_radius + edge_width, distance_from_center);\n");
        shader.push_str("    stroke_alpha = stroke_alpha - fill_alpha;\n");
        shader.push('\n');
        shader.push_str("    // Combine fill and stroke\n");
        shader.push_str("    let final_color = mix(\n");
        shader.push_str("        input.fill_color,\n");
        shader.push_str("        input.stroke_color,\n");
        shader.push_str("        stroke_alpha\n");
        shader.push_str("    );\n");
        shader.push('\n');
        shader.push_str("    // Apply overall alpha\n");
        shader.push_str("    let total_alpha = max(fill_alpha, stroke_alpha);\n");
        shader.push_str("    \n");
        shader.push_str("    // Discard fragments outside the circle\n");
        shader.push_str("    if (total_alpha < 0.01) {\n");
        shader.push_str("        discard;\n");
        shader.push_str("    }\n");
        shader.push('\n');
        shader.push_str("    return vec4<f32>(final_color.rgb, final_color.a * total_alpha);\n");
        shader.push_str("}\n");

        shader
    }

    /// Number of vertices in the circle quad (4 vertices for instanced rendering)
    fn vertex_count() -> usize {
        4
    }

    /// Number of indices for the circle quad (6 indices for 2 triangles)
    fn index_count() -> Option<usize> {
        Some(6)
    }

    /// Generate vertices for a unit quad used in instanced circle rendering.
    ///
    /// The quad covers the range [-1, 1] in both dimensions, and the actual
    /// circle shape is computed in the fragment shader.
    fn generate_vertices() -> Vec<Self::Vertex> {
        vec![
            CircleVertex {
                position: [-1.0, -1.0],
            }, // Bottom-left
            CircleVertex {
                position: [1.0, -1.0],
            }, // Bottom-right
            CircleVertex {
                position: [1.0, 1.0],
            }, // Top-right
            CircleVertex {
                position: [-1.0, 1.0],
            }, // Top-left
        ]
    }

    /// Generate indices for the circle quad (two triangles).
    ///
    /// Uses counter-clockwise winding order for proper face culling.
    fn generate_indices() -> Option<Vec<u32>> {
        Some(vec![
            0, 1, 2, // First triangle: bottom-left, bottom-right, top-right
            0, 2, 3, // Second triangle: bottom-left, top-right, top-left
        ])
    }

    /// Get the WGSL type name for a Circle attribute.
    fn get_attribute_type(attribute_name: &str) -> crate::error::GupResult<&'static str> {
        match attribute_name {
            "center" | "position" => Ok("vec2<f32>"),
            "radius" | "size" | "stroke_width" => Ok("f32"),
            "fill_color" | "color" | "stroke_color" => Ok("vec4<f32>"),
            _ => Err(crate::error::GupError::validation_error(format!(
                "Unknown Circle attribute: {attribute_name}"
            ))),
        }
    }

    /// Check if a shader function output type is compatible with a Circle attribute.
    fn is_attribute_compatible(attribute_name: &str, output_type: &str) -> bool {
        match Self::get_attribute_type(attribute_name) {
            Ok(expected_type) => expected_type == output_type,
            Err(_) => false,
        }
    }
}

impl Default for CircleAttributes {
    /// Default circle attributes for testing and prototyping.
    fn default() -> Self {
        Self {
            center: Vec2 { x: 0.0, y: 0.0 },
            radius: 5.0,
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
        }
    }
}

impl MarkInstanceBuilder for Circle {
    type Instance = CircleInstance;

    fn default_instance() -> Self::Instance {
        CircleInstance::from(&CircleAttributes::default())
    }

    fn build_instance(attrs: &[(&str, AttrValue)]) -> Self::Instance {
        let mut instance = Self::default_instance();
        for &(name, value) in attrs {
            match name {
                "center" | "position" => {
                    if let AttrValue::Vec2(v) = value {
                        instance.center = v;
                    }
                }
                "radius" | "size" => {
                    if let AttrValue::Float(v) = value {
                        instance.radius = v;
                    }
                }
                "fill_color" | "color" => {
                    if let AttrValue::Vec4(v) = value {
                        instance.fill_color = v;
                    }
                }
                "stroke_width" => {
                    if let AttrValue::Float(v) = value {
                        instance.stroke_width = v;
                    }
                }
                "stroke_color" => {
                    if let AttrValue::Vec4(v) = value {
                        instance.stroke_color = v;
                    }
                }
                _ => {} // Ignore unknown attributes
            }
        }
        instance
    }
}

// ---------------------------------------------------------------------------
// AccessibleMark implementation for Circle
// ---------------------------------------------------------------------------

impl crate::selection::AccessibleMark for Circle {
    fn describe_point(
        index: usize,
        total: usize,
        attrs: &[(&str, crate::selection::AttrValue)],
    ) -> String {
        use crate::selection::AttrValue;

        let mut parts = vec![format!("Point {} of {}", index + 1, total)];

        for &(name, value) in attrs {
            match (name, value) {
                ("center" | "position", AttrValue::Vec2(pos)) => {
                    parts.push(format!("at ({:.1}, {:.1})", pos[0], pos[1]));
                }
                ("radius" | "size", AttrValue::Float(r)) => {
                    parts.push(format!("radius {:.1}", r));
                }
                ("fill_color" | "color", AttrValue::Vec4(c)) => {
                    parts.push(format!(
                        "color {}",
                        crate::color_descriptor::describe_color(c)
                    ));
                }
                _ => {}
            }
        }

        parts.join(", ")
    }

    fn describe_mark_type() -> &'static str {
        "circle"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{vec2, vec4};

    #[test]
    fn test_circle_mark_implementation() {
        // Test basic mark trait methods
        assert_eq!(Circle::vertex_count(), 4);
        assert_eq!(Circle::index_count(), Some(6));

        let vertices = Circle::generate_vertices();
        assert_eq!(vertices.len(), 4);

        // Verify vertex positions
        assert_eq!(vertices[0].position, [-1.0, -1.0]);
        assert_eq!(vertices[1].position, [1.0, -1.0]);
        assert_eq!(vertices[2].position, [1.0, 1.0]);
        assert_eq!(vertices[3].position, [-1.0, 1.0]);

        let indices = Circle::generate_indices().unwrap();
        assert_eq!(indices.len(), 6);
        assert_eq!(indices, vec![0, 1, 2, 0, 2, 3]);
    }

    #[test]
    fn test_circle_shaders() {
        // Verify that custom shaders are provided
        assert!(Circle::VERTEX_SHADER.is_some());
        assert!(Circle::FRAGMENT_SHADER.is_some());

        // Shader content will be loaded from include_str! in actual implementation
        // For now, we verify the constants exist
    }

    #[test]
    fn test_circle_attributes() {
        let attrs = CircleAttributes {
            center: vec2![10.0, 20.0],
            radius: 15.0,
            fill_color: vec4![1.0, 0.0, 0.0, 1.0],
            stroke_width: 2.0,
            stroke_color: vec4![0.0, 0.0, 0.0, 1.0],
        };

        assert_eq!(attrs.center.x, 10.0);
        assert_eq!(attrs.center.y, 20.0);
        assert_eq!(attrs.radius, 15.0);
        assert_eq!(attrs.fill_color.x, 1.0); // Red
        assert_eq!(attrs.stroke_width, 2.0);
        assert_eq!(attrs.stroke_color.w, 1.0); // Opaque
    }

    #[test]
    fn test_circle_attributes_default() {
        let default_attrs = CircleAttributes::default();
        assert_eq!(default_attrs.center.x, 0.0);
        assert_eq!(default_attrs.center.y, 0.0);
        assert_eq!(default_attrs.radius, 5.0);
        assert_eq!(default_attrs.fill_color.x, 1.0); // White
        assert_eq!(default_attrs.stroke_width, 1.0);
    }

    #[test]
    fn test_vertex_buffer_compatibility() {
        let vertices = Circle::generate_vertices();

        // Verify vertex data is valid for GPU upload
        for vertex in &vertices {
            assert!(vertex.position[0].is_finite());
            assert!(vertex.position[1].is_finite());
            assert!(vertex.position[0] >= -1.0 && vertex.position[0] <= 1.0);
            assert!(vertex.position[1] >= -1.0 && vertex.position[1] <= 1.0);
        }

        // Verify bytemuck conversion works
        let bytes: &[u8] = bytemuck::cast_slice(&vertices);
        assert_eq!(
            bytes.len(),
            vertices.len() * std::mem::size_of::<CircleVertex>()
        );
    }

    #[test]
    fn test_circle_vertex_properties() {
        // Verify vertex type implements required traits
        let vertex = CircleVertex {
            position: [0.5, -0.5],
        };

        // Should be able to clone and debug
        let _cloned = vertex;
        println!("{vertex:?}");

        // Bytemuck conversion should work
        let bytes = bytemuck::bytes_of(&vertex);
        assert_eq!(bytes.len(), std::mem::size_of::<CircleVertex>());
    }

    #[test]
    fn test_circle_attributes_properties() {
        // Test that attributes can be constructed with vec macros
        let attrs = CircleAttributes {
            center: vec2![1.0, 2.0],
            radius: 10.0,
            fill_color: vec4![0.5, 0.5, 0.5, 0.8],
            stroke_width: 1.5,
            stroke_color: vec4![0.0, 0.0, 0.0, 1.0],
        };

        // Verify values are set correctly
        assert_eq!(attrs.center.x, 1.0);
        assert_eq!(attrs.center.y, 2.0);
        assert_eq!(attrs.radius, 10.0);
        assert_eq!(attrs.fill_color.w, 0.8); // Alpha
        assert_eq!(attrs.stroke_width, 1.5);
    }

    #[test]
    fn test_circle_mark_instance_builder_default() {
        let instance = Circle::default_instance();
        assert_eq!(instance.center, [0.0, 0.0]);
        assert_eq!(instance.radius, 5.0);
        assert_eq!(instance.fill_color, [1.0, 1.0, 1.0, 1.0]);
        assert_eq!(instance.stroke_width, 1.0);
        assert_eq!(instance.stroke_color, [0.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn test_circle_mark_instance_builder_with_attrs() {
        use crate::selection::AttrValue;

        let instance = Circle::build_instance(&[
            ("center", AttrValue::Vec2([0.5, -0.3])),
            ("radius", AttrValue::Float(0.1)),
            ("fill_color", AttrValue::Vec4([1.0, 0.0, 0.0, 1.0])),
        ]);

        assert_eq!(instance.center, [0.5, -0.3]);
        assert_eq!(instance.radius, 0.1);
        assert_eq!(instance.fill_color, [1.0, 0.0, 0.0, 1.0]);
        // Unset attributes should use defaults
        assert_eq!(instance.stroke_width, 1.0);
        assert_eq!(instance.stroke_color, [0.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn test_circle_mark_instance_builder_aliases() {
        use crate::selection::AttrValue;

        // "position" should work as alias for "center"
        let instance = Circle::build_instance(&[
            ("position", AttrValue::Vec2([0.2, 0.4])),
            ("color", AttrValue::Vec4([0.0, 1.0, 0.0, 0.5])),
            ("size", AttrValue::Float(0.3)),
        ]);

        assert_eq!(instance.center, [0.2, 0.4]);
        assert_eq!(instance.fill_color, [0.0, 1.0, 0.0, 0.5]);
        assert_eq!(instance.radius, 0.3);
    }

    #[test]
    fn test_circle_mark_instance_builder_unknown_attrs_ignored() {
        use crate::selection::AttrValue;

        // Unknown attributes should not cause errors
        let instance = Circle::build_instance(&[
            ("center", AttrValue::Vec2([0.1, 0.2])),
            ("unknown_field", AttrValue::Float(99.0)),
        ]);

        assert_eq!(instance.center, [0.1, 0.2]);
    }

    #[test]
    fn test_circle_accessible_mark_describe_point() {
        use crate::selection::{AccessibleMark, AttrValue};

        let desc = Circle::describe_point(
            0,
            3,
            &[
                ("center", AttrValue::Vec2([10.0, 20.0])),
                ("radius", AttrValue::Float(5.0)),
                ("fill_color", AttrValue::Vec4([1.0, 0.0, 0.0, 1.0])),
            ],
        );
        assert!(desc.contains("Point 1 of 3"));
        assert!(desc.contains("at (10.0, 20.0)"));
        assert!(desc.contains("radius 5.0"));
        assert!(desc.contains("color red"));
    }

    #[test]
    fn test_circle_accessible_mark_type() {
        use crate::selection::AccessibleMark;
        assert_eq!(Circle::describe_mark_type(), "circle");
    }
}

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

    /// Generate vertex shader with shader function integration.
    ///
    /// When using generated shaders, this method creates WGSL that integrates
    /// with the shader function pipeline for dynamic attribute mapping.
    fn generate_vertex_shader(pipeline: &ComposableShaderPipeline) -> String {
        let base_shader = r#"
// Line instance data structure
struct LineInstance {
    start: vec2<f32>,
    end: vec2<f32>,
    color: vec4<f32>,
    width: f32,
    style: u32,
}

@group(1) @binding(0) var<storage, read> instances: array<LineInstance>;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) normal: vec2<f32>,
    @builtin(instance_index) instance_index: u32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec2<f32>,
    @location(1) local_position: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) width: f32,
    @location(4) style: u32,
    @location(5) line_length: f32,
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    let instance = instances[input.instance_index];
    
    // Apply shader functions to instance data
    let transformed_start = position_transform(instance.start, position_uniforms);
    let transformed_end = position_transform(instance.end, position_uniforms);
    let final_color = color_transform(instance.color, color_uniforms);
    let final_width = width_transform(instance.width, width_uniforms);
    
    // Calculate line direction and length
    let line_vec = transformed_end - transformed_start;
    let line_length = length(line_vec);
    let line_dir = normalize(line_vec);
    let line_normal = vec2<f32>(-line_dir.y, line_dir.x);
    
    // Calculate world position along the line
    let along_line = transformed_start + line_dir * (input.position.x * line_length);
    let across_line = line_normal * (input.normal.y * final_width * 0.5);
    let world_pos = along_line + across_line;
    
    var output: VertexOutput;
    output.clip_position = vec4<f32>(world_pos, 0.0, 1.0);
    output.world_position = world_pos;
    output.local_position = input.position;
    output.color = final_color;
    output.width = final_width;
    output.style = instance.style;
    output.line_length = line_length;
    
    return output;
}
"#;

        // Integrate with pipeline functions
        let mut shader = pipeline.generate_vertex_shader();
        shader.push_str("\n\n");
        shader.push_str(base_shader);
        shader
    }

    /// Generate fragment shader with anti-aliasing and line style support.
    ///
    /// Creates a fragment shader that renders smooth lines using distance
    /// field calculations and integrates with the shader function system.
    fn generate_fragment_shader(pipeline: &ComposableShaderPipeline) -> String {
        let base_shader = r#"
@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // Calculate distance from line center
    let distance_from_center = abs(input.local_position.y);
    
    // Calculate base alpha for line width
    let half_width = 0.5;
    let edge_width = 0.02;
    let base_alpha = 1.0 - smoothstep(half_width - edge_width, half_width + edge_width, distance_from_center);
    
    // Apply line style
    var style_alpha = 1.0;
    if (input.style == 1u) { // Dashed
        let dash_pattern = sin(input.local_position.x * input.line_length * 0.1) > 0.0;
        style_alpha = select(0.0, 1.0, dash_pattern);
    } else if (input.style == 2u) { // Dotted
        let dot_pattern = fract(input.local_position.x * input.line_length * 0.05) < 0.3;
        style_alpha = select(0.0, 1.0, dot_pattern);
    }
    
    // Combine alpha values
    let final_alpha = base_alpha * style_alpha;
    
    return vec4<f32>(input.color.rgb, input.color.a * final_alpha);
}
"#;

        let mut shader = pipeline.generate_fragment_shader();
        shader.push_str("\n\n");
        shader.push_str(base_shader);
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
}

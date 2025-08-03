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
#[derive(Debug, Clone)]
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

    /// Generate vertex shader with shader function integration.
    ///
    /// When using generated shaders, this method creates WGSL that integrates
    /// with the shader function pipeline for dynamic attribute mapping.
    fn generate_vertex_shader(pipeline: &ComposableShaderPipeline) -> String {
        let base_shader = r#"
// Rectangle instance data structure
struct RectangleInstance {
    center: vec2<f32>,
    size: vec2<f32>,
    fill_color: vec4<f32>,
    stroke_width: f32,
    stroke_color: vec4<f32>,
    corner_radius: f32,
}

@group(1) @binding(0) var<storage, read> instances: array<RectangleInstance>;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @builtin(instance_index) instance_index: u32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec2<f32>,
    @location(1) local_position: vec2<f32>,
    @location(2) fill_color: vec4<f32>,
    @location(3) stroke_color: vec4<f32>,
    @location(4) size: vec2<f32>,
    @location(5) stroke_width: f32,
    @location(6) corner_radius: f32,
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    let instance = instances[input.instance_index];
    
    // Apply shader functions to instance data
    let transformed_center = position_transform(instance.center, position_uniforms);
    let final_size = size_transform(instance.size, size_uniforms);
    let final_fill_color = color_transform(instance.fill_color, color_uniforms);
    
    // Calculate world position
    let world_pos = input.position * final_size + transformed_center;
    
    var output: VertexOutput;
    output.clip_position = vec4<f32>(world_pos, 0.0, 1.0);
    output.world_position = world_pos;
    output.local_position = input.position;
    output.fill_color = final_fill_color;
    output.stroke_color = instance.stroke_color;
    output.size = final_size;
    output.stroke_width = instance.stroke_width;
    output.corner_radius = instance.corner_radius;
    
    return output;
}
"#;

        // Integrate with pipeline functions
        let mut shader = pipeline.generate_vertex_shader();
        shader.push_str("\n\n");
        shader.push_str(base_shader);
        shader
    }

    /// Generate fragment shader with anti-aliasing and rounded corners support.
    ///
    /// Creates a fragment shader that renders smooth rectangles using distance
    /// field calculations and integrates with the shader function system.
    fn generate_fragment_shader(pipeline: &ComposableShaderPipeline) -> String {
        let base_shader = r#"
@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // Convert to rectangle coordinate system
    let half_size = input.size * 0.5;
    let pos = abs(input.local_position * half_size);
    
    // Calculate distance to rounded rectangle
    let corner_offset = max(pos - (half_size - input.corner_radius), vec2<f32>(0.0));
    let distance_to_corner = length(corner_offset);
    let distance_to_edge = max(
        max(pos.x - half_size.x, pos.y - half_size.y),
        distance_to_corner - input.corner_radius
    );
    
    // Calculate stroke boundaries
    let outer_distance = distance_to_edge;
    let inner_distance = distance_to_edge + input.stroke_width;
    
    // Anti-aliasing edge width
    let edge_width = 0.5;
    
    // Calculate alpha values with smooth transitions
    let outer_alpha = 1.0 - smoothstep(-edge_width, edge_width, outer_distance);
    
    // Handle stroke rendering
    if (input.stroke_width > 0.0) {
        let inner_alpha = smoothstep(-edge_width, edge_width, inner_distance);
        let stroke_alpha = outer_alpha * (1.0 - inner_alpha);
        let fill_alpha = outer_alpha * inner_alpha;
        
        // Blend stroke and fill colors
        let final_color = input.stroke_color * stroke_alpha + input.fill_color * fill_alpha;
        return vec4<f32>(final_color.rgb, max(stroke_alpha, fill_alpha));
    } else {
        // No stroke - just fill
        return vec4<f32>(input.fill_color.rgb, input.fill_color.a * outer_alpha);
    }
}
"#;

        let mut shader = pipeline.generate_fragment_shader();
        shader.push_str("\n\n");
        shader.push_str(base_shader);
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
}

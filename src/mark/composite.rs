// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Composite mark for combining multiple marks into a single visual element.
//!
//! CompositeMark allows combining different mark types (circles, rectangles, lines,
//! paths) into complex visual symbols. This is useful for creating custom glyphs,
//! legends, or complex chart elements.

use super::Mark;
use crate::error::{GupError, GupResult};
use crate::shader_pipeline::ComposableShaderPipeline;
use std::collections::HashMap;

/// A composite mark that combines multiple sub-marks.
///
/// This allows creating complex visual elements by composing simpler marks.
/// Each sub-mark can be independently positioned and styled.
///
/// # Examples
///
/// ```rust,ignore
/// use gup::mark::{CompositeMark, CompositeMarkAttributes, SubMark};
/// use gup::mark::{Circle, CircleAttributes};
/// use gup::Vec2;
///
/// // Create a target symbol (circle with center dot)
/// let outer_circle = SubMark::Circle(CircleAttributes {
///     center: Vec2 { x: 0.0, y: 0.0 },
///     radius: 10.0,
///     fill_color: [1.0, 1.0, 1.0, 1.0],
///     stroke_color: [0.0, 0.0, 0.0, 1.0],
///     stroke_width: 2.0,
/// });
///
/// let inner_circle = SubMark::Circle(CircleAttributes {
///     center: Vec2 { x: 0.0, y: 0.0 },
///     radius: 3.0,
///     fill_color: [1.0, 0.0, 0.0, 1.0],
///     stroke_color: [1.0, 0.0, 0.0, 1.0],
///     stroke_width: 0.0,
/// });
///
/// let target = CompositeMarkAttributes {
///     sub_marks: vec![outer_circle, inner_circle],
///     transform: Transform::identity(),
/// };
/// ```
#[derive(Debug, Clone)]
pub struct CompositeMark;

/// Transform for mark positioning and scaling.
#[derive(Debug, Clone, Copy)]
pub struct Transform {
    /// Translation (x, y)
    pub translation: [f32; 2],
    /// Scale (x, y)
    pub scale: [f32; 2],
    /// Rotation in radians
    pub rotation: f32,
}

impl Transform {
    /// Create an identity transform (no transformation).
    pub fn identity() -> Self {
        Self {
            translation: [0.0, 0.0],
            scale: [1.0, 1.0],
            rotation: 0.0,
        }
    }

    /// Create a translation transform.
    pub fn translate(x: f32, y: f32) -> Self {
        Self {
            translation: [x, y],
            scale: [1.0, 1.0],
            rotation: 0.0,
        }
    }

    /// Create a scale transform.
    pub fn scale(x: f32, y: f32) -> Self {
        Self {
            translation: [0.0, 0.0],
            scale: [x, y],
            rotation: 0.0,
        }
    }

    /// Create a rotation transform.
    pub fn rotate(radians: f32) -> Self {
        Self {
            translation: [0.0, 0.0],
            scale: [1.0, 1.0],
            rotation: radians,
        }
    }

    /// Convert to 4x4 transformation matrix.
    pub fn to_matrix(&self) -> [[f32; 4]; 4] {
        let cos = self.rotation.cos();
        let sin = self.rotation.sin();
        let sx = self.scale[0];
        let sy = self.scale[1];
        let tx = self.translation[0];
        let ty = self.translation[1];

        [
            [sx * cos, sx * sin, 0.0, 0.0],
            [-sy * sin, sy * cos, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [tx, ty, 0.0, 1.0],
        ]
    }
}

/// A sub-mark within a composite mark.
///
/// This enum allows different mark types to be combined in a composite.
#[derive(Debug, Clone)]
pub enum SubMark {
    /// A circle sub-mark.
    Circle {
        /// Circle mark attributes.
        attributes: super::CircleAttributes,
        /// Local transform for this sub-mark.
        transform: Transform,
    },
    /// A rectangle sub-mark.
    Rectangle {
        /// Rectangle mark attributes.
        attributes: super::RectangleAttributes,
        /// Local transform for this sub-mark.
        transform: Transform,
    },
    /// A line sub-mark.
    Line {
        /// Line mark attributes.
        attributes: super::LineAttributes,
        /// Local transform for this sub-mark.
        transform: Transform,
    },
    /// A path sub-mark.
    Path {
        /// Path mark attributes.
        attributes: super::PathAttributes,
        /// Local transform for this sub-mark.
        transform: Transform,
    },
}

/// Attributes for a composite mark.
#[derive(Debug, Clone)]
pub struct CompositeMarkAttributes {
    /// Sub-marks that make up this composite
    pub sub_marks: Vec<SubMark>,
    /// Transform applied to the entire composite
    pub transform: Transform,
}

/// Vertex data for composite mark rendering.
///
/// Composite marks use a simple quad for each sub-mark, with instance data
/// determining which mark type and attributes to use.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CompositeMarkVertex {
    /// Vertex position within unit quad
    pub position: [f32; 2],
}

impl Mark for CompositeMark {
    type Vertex = CompositeMarkVertex;
    type AttributeValue = CompositeMarkAttributes;

    /// Composite marks use a base quad.
    fn vertex_count() -> usize {
        4
    }

    /// Indexed rendering for quads.
    fn index_count() -> Option<usize> {
        Some(6)
    }

    /// Generate base quad vertices.
    fn generate_vertices() -> Vec<Self::Vertex> {
        vec![
            CompositeMarkVertex {
                position: [-1.0, -1.0],
            },
            CompositeMarkVertex {
                position: [1.0, -1.0],
            },
            CompositeMarkVertex {
                position: [1.0, 1.0],
            },
            CompositeMarkVertex {
                position: [-1.0, 1.0],
            },
        ]
    }

    /// Generate indices for quad rendering.
    fn generate_indices() -> Option<Vec<u32>> {
        Some(vec![0, 1, 2, 0, 2, 3])
    }

    /// Generate vertex shader for composite marks.
    fn generate_vertex_shader_with_functions(
        pipeline: &ComposableShaderPipeline,
        _attribute_functions: &HashMap<String, String>,
    ) -> String {
        let base_shader = pipeline.generate_vertex_shader();

        format!(
            r#"
// Composite mark vertex shader

struct CompositeInstance {{
    transform: mat4x4<f32>,
    mark_type: u32,
    padding: vec3<u32>,
}}

@group(0) @binding(0)
var<storage, read> instances: array<CompositeInstance>;

struct VertexOutput {{
    @builtin(position) position: vec4<f32>,
    @location(0) mark_type: u32,
}}

@vertex
fn vs_main(
    @location(0) position: vec2<f32>,
    @builtin(instance_index) instance_index: u32,
) -> VertexOutput {{
    let instance = instances[instance_index];
    var output: VertexOutput;
    
    // Apply composite transform
    let world_pos = instance.transform * vec4<f32>(position, 0.0, 1.0);
    output.position = world_pos;
    output.mark_type = instance.mark_type;
    
    return output;
}}

{base_shader}
"#
        )
    }

    /// Generate fragment shader for composite marks.
    fn generate_fragment_shader_with_functions(
        pipeline: &ComposableShaderPipeline,
        _attribute_functions: &HashMap<String, String>,
    ) -> String {
        let base_shader = pipeline.generate_fragment_shader();

        format!(
            r#"
// Composite mark fragment shader

struct FragmentInput {{
    @location(0) mark_type: u32,
}}

@fragment
fn fs_main(input: FragmentInput) -> @location(0) vec4<f32> {{
    // Dispatch to appropriate mark renderer based on mark_type
    // 0 = Circle, 1 = Rectangle, 2 = Line, 3 = Path
    
    // For now, return a default color
    // TODO: Implement proper sub-mark rendering
    return vec4<f32>(1.0, 0.0, 1.0, 1.0);
}}

{base_shader}
"#
        )
    }

    fn get_attribute_type(attribute_name: &str) -> GupResult<&'static str> {
        match attribute_name {
            "position" => Ok("vec2<f32>"),
            "transform" => Ok("mat4x4<f32>"),
            "sub_marks" => Ok("array<SubMark>"),
            _ => Err(GupError::validation_error(format!(
                "Unknown composite mark attribute: {attribute_name}"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transform_identity() {
        let transform = Transform::identity();
        assert_eq!(transform.translation, [0.0, 0.0]);
        assert_eq!(transform.scale, [1.0, 1.0]);
        assert_eq!(transform.rotation, 0.0);
    }

    #[test]
    fn test_transform_translate() {
        let transform = Transform::translate(10.0, 20.0);
        assert_eq!(transform.translation, [10.0, 20.0]);
    }

    #[test]
    fn test_transform_scale() {
        let transform = Transform::scale(2.0, 3.0);
        assert_eq!(transform.scale, [2.0, 3.0]);
    }

    #[test]
    fn test_transform_rotate() {
        let transform = Transform::rotate(std::f32::consts::PI / 2.0);
        assert!((transform.rotation - std::f32::consts::PI / 2.0).abs() < 1e-6);
    }

    #[test]
    fn test_transform_to_matrix() {
        let transform = Transform::identity();
        let matrix = transform.to_matrix();

        // Check identity matrix
        assert_eq!(matrix[0][0], 1.0);
        assert_eq!(matrix[1][1], 1.0);
        assert_eq!(matrix[2][2], 1.0);
        assert_eq!(matrix[3][3], 1.0);
    }

    #[test]
    fn test_composite_mark_vertex_layout() {
        let vertex = CompositeMarkVertex {
            position: [1.0, 2.0],
        };

        // Verify bytemuck traits work
        let bytes = bytemuck::bytes_of(&vertex);
        assert_eq!(bytes.len(), std::mem::size_of::<CompositeMarkVertex>());
    }

    #[test]
    fn test_composite_mark_vertex_generation() {
        let vertices = CompositeMark::generate_vertices();
        assert_eq!(vertices.len(), CompositeMark::vertex_count());
        assert_eq!(vertices.len(), 4);
    }

    #[test]
    fn test_composite_mark_index_generation() {
        let indices = CompositeMark::generate_indices();
        assert!(indices.is_some());
        let indices = indices.unwrap();
        assert_eq!(indices.len(), CompositeMark::index_count().unwrap());
        assert_eq!(indices.len(), 6);
    }

    #[test]
    fn test_composite_mark_attributes_creation() {
        let attrs = CompositeMarkAttributes {
            sub_marks: vec![],
            transform: Transform::identity(),
        };

        assert_eq!(attrs.sub_marks.len(), 0);
    }

    #[test]
    fn test_composite_mark_attribute_types() {
        assert_eq!(
            CompositeMark::get_attribute_type("position").unwrap(),
            "vec2<f32>"
        );
        assert_eq!(
            CompositeMark::get_attribute_type("transform").unwrap(),
            "mat4x4<f32>"
        );
        assert!(CompositeMark::get_attribute_type("invalid").is_err());
    }
}

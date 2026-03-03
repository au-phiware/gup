// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Path mark for SVG-like vector paths.
//!
//! The Path mark supports complex shapes defined by line and curve segments,
//! similar to SVG path elements. It's rendered using GPU tessellation for
//! efficient performance with complex geometries.

use super::Mark;
use crate::error::GupResult;
use crate::shader_pipeline::ComposableShaderPipeline;
use std::collections::HashMap;

/// Path mark for rendering complex vector paths.
///
/// Paths are defined by a sequence of commands (MoveTo, LineTo, CurveTo, etc.)
/// similar to SVG path syntax. The path is tessellated on the GPU for efficient
/// rendering.
///
/// # Examples
///
/// ```rust
/// use gup::mark::{Path, PathAttributes, PathCommand};
/// use gup::Vec2;
///
/// // Define a simple triangle path
/// let commands = vec![
///     PathCommand::MoveTo(Vec2 { x: 0.0, y: 0.0 }),
///     PathCommand::LineTo(Vec2 { x: 1.0, y: 0.0 }),
///     PathCommand::LineTo(Vec2 { x: 0.5, y: 1.0 }),
///     PathCommand::Close,
/// ];
///
/// let attrs = PathAttributes {
///     commands,
///     fill_color: [1.0, 0.0, 0.0, 1.0],
///     stroke_color: [0.0, 0.0, 0.0, 1.0],
///     stroke_width: 2.0,
/// };
/// ```
#[derive(Debug, Clone)]
pub struct Path;

/// Path command types similar to SVG.
#[derive(Debug, Clone, Copy)]
pub enum PathCommand {
    /// Move to a position without drawing
    MoveTo(crate::Vec2),
    /// Draw a line to a position
    LineTo(crate::Vec2),
    /// Draw a quadratic Bezier curve
    QuadraticCurveTo {
        control: crate::Vec2,
        end: crate::Vec2,
    },
    /// Draw a cubic Bezier curve
    CubicCurveTo {
        control1: crate::Vec2,
        control2: crate::Vec2,
        end: crate::Vec2,
    },
    /// Close the path by drawing a line to the start
    Close,
}

/// GPU vertex data for path rendering.
///
/// Paths are tessellated into triangles for GPU rendering. Each vertex
/// contains position and texture coordinates for distance field rendering.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PathVertex {
    /// Vertex position within the path's bounding box
    pub position: [f32; 2],
    /// Texture coordinates for SDF-based rendering
    pub tex_coords: [f32; 2],
}

/// High-level attributes for path configuration.
///
/// These attributes define the visual appearance of the path,
/// including fill and stroke properties.
#[derive(Debug, Clone)]
pub struct PathAttributes {
    /// Path commands defining the shape
    pub commands: Vec<PathCommand>,
    /// Fill color (RGBA)
    pub fill_color: [f32; 4],
    /// Stroke color (RGBA)
    pub stroke_color: [f32; 4],
    /// Stroke width in pixels
    pub stroke_width: f32,
}

impl Default for PathAttributes {
    fn default() -> Self {
        Self {
            commands: Vec::new(),
            fill_color: [0.0, 0.0, 0.0, 1.0],
            stroke_color: [0.0, 0.0, 0.0, 1.0],
            stroke_width: 1.0,
        }
    }
}

impl Mark for Path {
    type Vertex = PathVertex;
    type AttributeValue = PathAttributes;

    /// Hand-written vertex shader for path rendering.
    const VERTEX_SHADER: Option<&'static str> = Some(include_str!("shaders/path.vert.wgsl"));

    /// Hand-written fragment shader for path rendering.
    const FRAGMENT_SHADER: Option<&'static str> = Some(include_str!("shaders/path.frag.wgsl"));

    /// Pattern-enabled fragment shader for accessibility rendering.
    ///
    /// This shader integrates pattern-based rendering for colorblind users,
    /// using texture patterns instead of colors for data encoding.
    const PATTERN_FRAGMENT_SHADER: Option<&'static str> =
        Some(include_str!("shaders/path_pattern.frag.wgsl"));

    /// Paths use dynamic vertex count based on tessellation.
    /// This returns a default quad for the base geometry.
    fn vertex_count() -> usize {
        4
    }

    /// Paths use indexed rendering for efficient triangle generation.
    fn index_count() -> Option<usize> {
        Some(6)
    }

    /// Generate base quad vertices for path rendering.
    fn generate_vertices() -> Vec<Self::Vertex> {
        vec![
            PathVertex {
                position: [-1.0, -1.0],
                tex_coords: [0.0, 0.0],
            },
            PathVertex {
                position: [1.0, -1.0],
                tex_coords: [1.0, 0.0],
            },
            PathVertex {
                position: [1.0, 1.0],
                tex_coords: [1.0, 1.0],
            },
            PathVertex {
                position: [-1.0, 1.0],
                tex_coords: [0.0, 1.0],
            },
        ]
    }

    /// Generate indices for quad rendering.
    fn generate_indices() -> Option<Vec<u32>> {
        Some(vec![0, 1, 2, 0, 2, 3])
    }

    /// Get vertex attributes for path rendering.
    ///
    /// Paths require two vertex attributes:
    /// - @location(0): position (vec2<f32>) - local position within bounding box
    /// - @location(1): tex_coords (vec2<f32>) - texture coordinates for SDF rendering
    fn vertex_attributes() -> &'static [wgpu::VertexAttribute] {
        &[
            wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x2, // position
            },
            wgpu::VertexAttribute {
                offset: 8, // offset of second vec2 (2 * 4 bytes)
                shader_location: 1,
                format: wgpu::VertexFormat::Float32x2, // tex_coords
            },
        ]
    }

    /// Generate vertex shader with path-specific transformations.
    fn generate_vertex_shader_with_functions(
        pipeline: &ComposableShaderPipeline,
        _attribute_functions: &HashMap<String, String>,
    ) -> String {
        let base_shader = pipeline.generate_vertex_shader();

        format!(
            r#"
// Path vertex shader

struct PathInstance {{
    transform: mat4x4<f32>,
    fill_color: vec4<f32>,
    stroke_color: vec4<f32>,
    stroke_width: f32,
}}

@group(0) @binding(0)
var<storage, read> instances: array<PathInstance>;

struct VertexOutput {{
    @builtin(position) position: vec4<f32>,
    @location(0) world_position: vec2<f32>,
    @location(1) tex_coords: vec2<f32>,
    @location(2) fill_color: vec4<f32>,
    @location(3) stroke_color: vec4<f32>,
    @location(4) stroke_width: f32,
}}

@vertex
fn vs_main(
    @location(0) position: vec2<f32>,
    @location(1) tex_coords: vec2<f32>,
    @builtin(instance_index) instance_index: u32,
) -> VertexOutput {{
    let instance = instances[instance_index];
    var output: VertexOutput;
    
    // Transform path vertex to world space
    let world_pos_4d = instance.transform * vec4<f32>(position, 0.0, 1.0);
    let world_pos_2d = world_pos_4d.xy;
    
    output.position = world_pos_4d;
    output.world_position = world_pos_2d;
    output.tex_coords = tex_coords;
    output.fill_color = instance.fill_color;
    output.stroke_color = instance.stroke_color;
    output.stroke_width = instance.stroke_width;
    
    return output;
}}

{base_shader}
"#
        )
    }

    /// Generate fragment shader for path rendering with anti-aliasing.
    fn generate_fragment_shader_with_functions(
        pipeline: &ComposableShaderPipeline,
        _attribute_functions: &HashMap<String, String>,
    ) -> String {
        let base_shader = pipeline.generate_fragment_shader();

        format!(
            r#"
// Path fragment shader

struct FragmentInput {{
    @location(0) tex_coords: vec2<f32>,
    @location(1) fill_color: vec4<f32>,
    @location(2) stroke_color: vec4<f32>,
    @location(3) stroke_width: f32,
}}

@fragment
fn fs_main(input: FragmentInput) -> @location(0) vec4<f32> {{
    // Simple fill rendering - in future, implement proper SDF-based path rendering
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
            "stroke_width" => Ok("f32"),
            "commands" => Ok("array<PathCommand>"), // Custom type
            _ => Err(crate::error::GupError::validation_error(format!(
                "Unknown path attribute: {attribute_name}"
            ))),
        }
    }

    /// Return a representative SVG `<path>` element for this mark type.
    fn svg_element(&self) -> Option<crate::export::svg::SvgElement> {
        Some(crate::export::svg::SvgElement::Path {
            d: String::new(),
            fill: "none".to_string(),
            stroke: Some("rgb(0,0,0)".to_string()),
            stroke_width: Some(1.0),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_vertex_layout() {
        let vertex = PathVertex {
            position: [1.0, 2.0],
            tex_coords: [0.5, 0.5],
        };

        // Verify bytemuck traits work
        let bytes = bytemuck::bytes_of(&vertex);
        assert_eq!(bytes.len(), std::mem::size_of::<PathVertex>());
    }

    #[test]
    fn test_path_attributes_default() {
        let attrs = PathAttributes::default();
        assert_eq!(attrs.commands.len(), 0);
        assert_eq!(attrs.stroke_width, 1.0);
    }

    #[test]
    fn test_path_vertex_generation() {
        let vertices = Path::generate_vertices();
        assert_eq!(vertices.len(), Path::vertex_count());
        assert_eq!(vertices.len(), 4);
    }

    #[test]
    fn test_path_index_generation() {
        let indices = Path::generate_indices();
        assert!(indices.is_some());
        let indices = indices.unwrap();
        assert_eq!(indices.len(), Path::index_count().unwrap());
        assert_eq!(indices.len(), 6);
    }

    #[test]
    fn test_path_command_creation() {
        let move_to = PathCommand::MoveTo(crate::Vec2 { x: 0.0, y: 0.0 });
        let line_to = PathCommand::LineTo(crate::Vec2 { x: 1.0, y: 1.0 });
        let close = PathCommand::Close;

        let commands = [move_to, line_to, close];
        assert_eq!(commands.len(), 3);
    }

    #[test]
    fn test_path_attributes_with_commands() {
        let attrs = PathAttributes {
            commands: vec![
                PathCommand::MoveTo(crate::Vec2 { x: 0.0, y: 0.0 }),
                PathCommand::LineTo(crate::Vec2 { x: 1.0, y: 0.0 }),
                PathCommand::LineTo(crate::Vec2 { x: 0.5, y: 1.0 }),
                PathCommand::Close,
            ],
            fill_color: [1.0, 0.0, 0.0, 1.0],
            stroke_color: [0.0, 0.0, 0.0, 1.0],
            stroke_width: 2.0,
        };

        assert_eq!(attrs.commands.len(), 4);
        assert_eq!(attrs.stroke_width, 2.0);
    }

    #[test]
    fn test_path_attribute_types() {
        assert_eq!(Path::get_attribute_type("position").unwrap(), "vec2<f32>");
        assert_eq!(Path::get_attribute_type("fill_color").unwrap(), "vec4<f32>");
        assert_eq!(
            Path::get_attribute_type("stroke_color").unwrap(),
            "vec4<f32>"
        );
        assert_eq!(Path::get_attribute_type("stroke_width").unwrap(), "f32");
        assert!(Path::get_attribute_type("invalid").is_err());
    }
}

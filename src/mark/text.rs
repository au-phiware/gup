// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Text mark for rendering text labels using SDF fonts.
//!
//! The Text mark integrates the existing GPU-accelerated text rendering system
//! as a Mark, allowing text to be used in visualizations alongside other marks.

use super::Mark;
use crate::error::GupResult;
use crate::shader_pipeline::ComposableShaderPipeline;
use crate::text::{TextAnchor, TextStyle};
use std::collections::HashMap;

/// Text mark for rendering text labels.
///
/// This mark uses the SDF font rendering system to display text with high
/// quality at any scale.
///
/// # Examples
///
/// ```rust
/// use gup::mark::{Text, TextMarkAttributes};
/// use gup::text::{TextStyle, TextAnchor};
/// use gup::Vec2;
///
/// let attrs = TextMarkAttributes {
///     text: "Hello".to_string(),
///     position: Vec2 { x: 100.0, y: 100.0 },
///     style: TextStyle::default(),
///     anchor: TextAnchor::Center,
/// };
/// ```
#[derive(Debug, Clone)]
pub struct Text;

/// GPU vertex data for text rendering.
///
/// Text is rendered using SDF glyphs on quads.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TextVertex {
    /// Vertex position in screen space
    pub position: [f32; 2],
    /// Texture coordinates for the glyph
    pub tex_coords: [f32; 2],
}

/// Attributes for text mark configuration.
#[derive(Debug, Clone)]
pub struct TextMarkAttributes {
    /// The text content to render
    pub text: String,
    /// Position of the text anchor point
    pub position: crate::Vec2,
    /// Text style (font, size, color, etc.)
    pub style: TextStyle,
    /// Text anchor point for positioning
    pub anchor: TextAnchor,
}

impl Default for TextMarkAttributes {
    fn default() -> Self {
        Self {
            text: String::new(),
            position: crate::Vec2 { x: 0.0, y: 0.0 },
            style: TextStyle::default(),
            anchor: TextAnchor::TopLeft,
        }
    }
}

impl Mark for Text {
    type Vertex = TextVertex;
    type AttributeValue = TextMarkAttributes;

    /// Text uses a quad per glyph, so base count is 4.
    fn vertex_count() -> usize {
        4
    }

    /// Indexed rendering for efficiency.
    fn index_count() -> Option<usize> {
        Some(6)
    }

    /// Generate base quad vertices for a glyph.
    fn generate_vertices() -> Vec<Self::Vertex> {
        vec![
            TextVertex {
                position: [0.0, 0.0],
                tex_coords: [0.0, 0.0],
            },
            TextVertex {
                position: [1.0, 0.0],
                tex_coords: [1.0, 0.0],
            },
            TextVertex {
                position: [1.0, 1.0],
                tex_coords: [1.0, 1.0],
            },
            TextVertex {
                position: [0.0, 1.0],
                tex_coords: [0.0, 1.0],
            },
        ]
    }

    /// Generate indices for quad rendering.
    fn generate_indices() -> Option<Vec<u32>> {
        Some(vec![0, 1, 2, 0, 2, 3])
    }

    /// Generate vertex shader for text rendering with SDF support.
    fn generate_vertex_shader_with_functions(
        pipeline: &ComposableShaderPipeline,
        _attribute_functions: &HashMap<String, String>,
    ) -> String {
        let base_shader = pipeline.generate_vertex_shader();

        format!(
            r#"
// Text mark vertex shader

struct TextInstance {{
    position: vec2<f32>,
    glyph_offset: vec2<f32>,
    glyph_size: vec2<f32>,
    tex_coords: vec4<f32>,  // (u0, v0, u1, v1)
    color: vec4<f32>,
}}

@group(0) @binding(0)
var<storage, read> instances: array<TextInstance>;

@group(0) @binding(1)
var<uniform> projection: mat4x4<f32>;

struct VertexOutput {{
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @location(1) color: vec4<f32>,
}}

@vertex
fn vs_main(
    @location(0) vertex_pos: vec2<f32>,
    @location(1) vertex_tex: vec2<f32>,
    @builtin(instance_index) instance_index: u32,
) -> VertexOutput {{
    let instance = instances[instance_index];
    var output: VertexOutput;
    
    // Calculate glyph position
    let glyph_pos = instance.position + instance.glyph_offset + vertex_pos * instance.glyph_size;
    output.position = projection * vec4<f32>(glyph_pos, 0.0, 1.0);
    
    // Interpolate texture coordinates
    let u = mix(instance.tex_coords.x, instance.tex_coords.z, vertex_tex.x);
    let v = mix(instance.tex_coords.y, instance.tex_coords.w, vertex_tex.y);
    output.tex_coords = vec2<f32>(u, v);
    output.color = instance.color;
    
    return output;
}}

{base_shader}
"#
        )
    }

    /// Generate fragment shader for SDF text rendering.
    fn generate_fragment_shader_with_functions(
        pipeline: &ComposableShaderPipeline,
        _attribute_functions: &HashMap<String, String>,
    ) -> String {
        let base_shader = pipeline.generate_fragment_shader();

        format!(
            r#"
// Text mark fragment shader with SDF rendering

@group(0) @binding(2)
var font_texture: texture_2d<f32>;

@group(0) @binding(3)
var font_sampler: sampler;

struct FragmentInput {{
    @location(0) tex_coords: vec2<f32>,
    @location(1) color: vec4<f32>,
}}

@fragment
fn fs_main(input: FragmentInput) -> @location(0) vec4<f32> {{
    // Sample SDF value from font atlas
    let sdf_value = textureSample(font_texture, font_sampler, input.tex_coords).r;
    
    // SDF rendering parameters
    let smoothing = 0.05;  // Adjust for edge smoothness
    let threshold = 0.5;   // Distance field threshold
    
    // Calculate alpha from SDF value
    let alpha = smoothstep(threshold - smoothing, threshold + smoothing, sdf_value);
    
    // Return colored text with SDF anti-aliasing
    return vec4<f32>(input.color.rgb, input.color.a * alpha);
}}

{base_shader}
"#
        )
    }

    fn get_attribute_type(attribute_name: &str) -> GupResult<&'static str> {
        match attribute_name {
            "position" => Ok("vec2<f32>"),
            "text" => Ok("string"), // Special type
            "color" => Ok("vec4<f32>"),
            "font_size" => Ok("f32"),
            _ => Err(crate::error::GupError::validation_error(format!(
                "Unknown text mark attribute: {attribute_name}"
            ))),
        }
    }

    /// Return a representative SVG `<text>` element for this mark type.
    fn svg_element(&self) -> Option<crate::export::svg::SvgElement> {
        Some(crate::export::svg::SvgElement::Text {
            x: 0.0,
            y: 0.0,
            content: String::new(),
            font_family: "sans-serif".to_string(),
            font_size: 14.0,
            text_anchor: "start".to_string(),
            dominant_baseline: "alphabetic".to_string(),
            fill: "rgb(0,0,0)".to_string(),
            font_weight: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_vertex_layout() {
        let vertex = TextVertex {
            position: [1.0, 2.0],
            tex_coords: [0.5, 0.5],
        };

        // Verify bytemuck traits work
        let bytes = bytemuck::bytes_of(&vertex);
        assert_eq!(bytes.len(), std::mem::size_of::<TextVertex>());
    }

    #[test]
    fn test_text_attributes_default() {
        let attrs = TextMarkAttributes::default();
        assert_eq!(attrs.text, "");
        assert_eq!(attrs.position.x, 0.0);
        assert_eq!(attrs.position.y, 0.0);
    }

    #[test]
    fn test_text_vertex_generation() {
        let vertices = Text::generate_vertices();
        assert_eq!(vertices.len(), Text::vertex_count());
        assert_eq!(vertices.len(), 4);
    }

    #[test]
    fn test_text_index_generation() {
        let indices = Text::generate_indices();
        assert!(indices.is_some());
        let indices = indices.unwrap();
        assert_eq!(indices.len(), Text::index_count().unwrap());
        assert_eq!(indices.len(), 6);
    }

    #[test]
    fn test_text_attributes_creation() {
        let attrs = TextMarkAttributes {
            text: "Hello, World!".to_string(),
            position: crate::Vec2 { x: 100.0, y: 200.0 },
            style: TextStyle::default(),
            anchor: TextAnchor::Center,
        };

        assert_eq!(attrs.text, "Hello, World!");
        assert_eq!(attrs.position.x, 100.0);
        assert_eq!(attrs.position.y, 200.0);
        assert_eq!(attrs.anchor, TextAnchor::Center);
    }

    #[test]
    fn test_text_attribute_types() {
        assert_eq!(Text::get_attribute_type("position").unwrap(), "vec2<f32>");
        assert_eq!(Text::get_attribute_type("text").unwrap(), "string");
        assert_eq!(Text::get_attribute_type("color").unwrap(), "vec4<f32>");
        assert_eq!(Text::get_attribute_type("font_size").unwrap(), "f32");
        assert!(Text::get_attribute_type("invalid").is_err());
    }
}

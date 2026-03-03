// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! 3D line segment mark.
//!
//! Each line connects two `[f32; 3]` endpoints. The line is rendered as a
//! camera-facing quad (two triangles) whose width is a configurable number
//! of pixels. Unlike `Sphere3D` and `Box3D`, lines are **unlit** — they
//! use a constant colour.

use crate::mark::Mark;
use crate::shader_pipeline::ComposableShaderPipeline;
use std::collections::HashMap;

/// A 3D line segment between two endpoints.
#[derive(Debug, Clone, gup_macros::MarkTypeId)]
#[mark_type_id = 22]
pub struct Line3D;

/// GPU vertex for the line quad. Each line instance is a quad with 4 verts.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Line3DVertex {
    /// Index into the two endpoints (0 = start side, 1 = end side) and
    /// the offset direction (±1).
    ///
    /// x: endpoint selector (0.0 or 1.0)
    /// y: side offset (-1.0 or 1.0)
    pub selector: [f32; 2],
}

/// Per-instance GPU data.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Line3DInstance {
    /// Start point (world space).
    pub start: [f32; 3],
    /// Line width in clip-space units.
    pub width: f32,
    /// End point (world space).
    pub end: [f32; 3],
    pub _pad: f32,
    /// Colour (RGBA).
    pub color: [f32; 4],
}

/// High-level attributes for a 3D line.
#[derive(Debug, Clone)]
pub struct Line3DAttributes {
    pub start: [f32; 3],
    pub end: [f32; 3],
    pub width: f32,
    pub color: [f32; 4],
}

impl Default for Line3DAttributes {
    fn default() -> Self {
        Self {
            start: [0.0; 3],
            end: [1.0, 0.0, 0.0],
            width: 0.005,
            color: [1.0; 4],
        }
    }
}

impl Mark for Line3D {
    type Vertex = Line3DVertex;
    type AttributeValue = Line3DAttributes;

    const VERTEX_SHADER: Option<&'static str> = Some(include_str!("shaders/line3d.vert.wgsl"));
    const FRAGMENT_SHADER: Option<&'static str> = Some(include_str!("shaders/line3d.frag.wgsl"));

    fn generate_vertex_shader(pipeline: &ComposableShaderPipeline) -> String {
        Self::generate_vertex_shader_with_functions(pipeline, &HashMap::new())
    }

    fn generate_fragment_shader(pipeline: &ComposableShaderPipeline) -> String {
        Self::generate_fragment_shader_with_functions(pipeline, &HashMap::new())
    }

    fn vertex_count() -> usize {
        4
    }

    fn index_count() -> Option<usize> {
        Some(6)
    }

    fn generate_vertices() -> Vec<Self::Vertex> {
        vec![
            Line3DVertex {
                selector: [0.0, -1.0],
            }, // start, left
            Line3DVertex {
                selector: [0.0, 1.0],
            }, // start, right
            Line3DVertex {
                selector: [1.0, 1.0],
            }, // end, right
            Line3DVertex {
                selector: [1.0, -1.0],
            }, // end, left
        ]
    }

    fn generate_indices() -> Option<Vec<u32>> {
        Some(vec![0, 1, 2, 0, 2, 3])
    }

    fn get_attribute_type(attribute_name: &str) -> crate::error::GupResult<&'static str> {
        match attribute_name {
            "start" | "end" | "position" => Ok("vec3<f32>"),
            "width" => Ok("f32"),
            "color" | "fill_color" => Ok("vec4<f32>"),
            _ => Err(crate::error::GupError::validation_error(format!(
                "Unknown Line3D attribute: {attribute_name}"
            ))),
        }
    }

    fn is_attribute_compatible(attribute_name: &str, output_type: &str) -> bool {
        Self::get_attribute_type(attribute_name)
            .map(|t| t == output_type)
            .unwrap_or(false)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line3d_geometry() {
        let verts = Line3D::generate_vertices();
        assert_eq!(verts.len(), 4);
        let indices = Line3D::generate_indices().unwrap();
        assert_eq!(indices.len(), 6);
    }

    #[test]
    fn instance_bytemuck() {
        let inst = Line3DInstance {
            start: [0.0; 3],
            width: 0.01,
            end: [1.0, 0.0, 0.0],
            _pad: 0.0,
            color: [1.0; 4],
        };
        let bytes: &[u8] = bytemuck::bytes_of(&inst);
        assert_eq!(bytes.len(), std::mem::size_of::<Line3DInstance>());
    }
}

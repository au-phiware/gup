// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! 3D sphere mark rendered as a billboard quad with SDF-based shading.
//!
//! Each instance is a camera-facing quad that evaluates a sphere signed-distance
//! function in the fragment shader. Phong lighting is applied per-fragment and
//! the depth value is reconstructed so that spheres occlude correctly.

use crate::mark::Mark;
use crate::shader_pipeline::ComposableShaderPipeline;
use std::collections::HashMap;

/// A billboard sphere mark for 3D scatter plots.
#[derive(Debug, Clone, gup_macros::MarkTypeId)]
#[mark_type_id = 20]
pub struct Sphere3D;

/// GPU vertex for the unit quad shared by all sphere instances.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Sphere3DVertex {
    /// Local quad position in \[-1, 1\].
    pub position: [f32; 2],
}

/// Per-instance GPU data uploaded to a storage buffer.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Sphere3DInstance {
    /// World-space centre.
    pub position: [f32; 3],
    /// Sphere radius in world units.
    pub radius: f32,
    /// Base colour (RGBA, linear).
    pub color: [f32; 4],
    /// Material: albedo.xyz, ambient
    pub material_albedo_ambient: [f32; 4],
    /// Material: diffuse, specular, shininess, _pad
    pub material_dss: [f32; 4],
}

/// High-level attributes for configuring a sphere.
#[derive(Debug, Clone)]
pub struct Sphere3DAttributes {
    pub position: [f32; 3],
    pub radius: f32,
    pub color: [f32; 4],
}

impl Default for Sphere3DAttributes {
    fn default() -> Self {
        Self {
            position: [0.0; 3],
            radius: 0.05,
            color: [0.4, 0.6, 1.0, 1.0],
        }
    }
}

impl Mark for Sphere3D {
    type Vertex = Sphere3DVertex;
    type AttributeValue = Sphere3DAttributes;

    const VERTEX_SHADER: Option<&'static str> = Some(include_str!("shaders/sphere3d.vert.wgsl"));
    const FRAGMENT_SHADER: Option<&'static str> = Some(include_str!("shaders/sphere3d.frag.wgsl"));

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
            Sphere3DVertex {
                position: [-1.0, -1.0],
            },
            Sphere3DVertex {
                position: [1.0, -1.0],
            },
            Sphere3DVertex {
                position: [1.0, 1.0],
            },
            Sphere3DVertex {
                position: [-1.0, 1.0],
            },
        ]
    }

    fn generate_indices() -> Option<Vec<u32>> {
        Some(vec![0, 1, 2, 0, 2, 3])
    }

    fn get_attribute_type(attribute_name: &str) -> crate::error::GupResult<&'static str> {
        match attribute_name {
            "position" => Ok("vec3<f32>"),
            "radius" | "size" => Ok("f32"),
            "color" | "fill_color" => Ok("vec4<f32>"),
            _ => Err(crate::error::GupError::validation_error(format!(
                "Unknown Sphere3D attribute: {attribute_name}"
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
    fn sphere3d_vertices() {
        let verts = Sphere3D::generate_vertices();
        assert_eq!(verts.len(), 4);
        assert_eq!(Sphere3D::index_count(), Some(6));
    }

    #[test]
    fn instance_bytemuck() {
        let inst = Sphere3DInstance {
            position: [1.0, 2.0, 3.0],
            radius: 0.5,
            color: [1.0, 0.0, 0.0, 1.0],
            material_albedo_ambient: [0.8, 0.8, 0.8, 0.15],
            material_dss: [0.7, 0.5, 32.0, 0.0],
        };
        let bytes: &[u8] = bytemuck::bytes_of(&inst);
        assert_eq!(bytes.len(), std::mem::size_of::<Sphere3DInstance>());
    }

    #[test]
    fn attribute_types() {
        assert_eq!(
            Sphere3D::get_attribute_type("position").unwrap(),
            "vec3<f32>"
        );
        assert_eq!(Sphere3D::get_attribute_type("radius").unwrap(), "f32");
        assert_eq!(Sphere3D::get_attribute_type("color").unwrap(), "vec4<f32>");
    }
}

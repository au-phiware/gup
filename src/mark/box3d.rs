// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Axis-aligned 3D box mark.
//!
//! Each box is defined by a centre and half-extents. The vertex shader
//! generates the six faces (36 vertices via index buffer) from a unit
//! cube that is then scaled and translated per instance.

use crate::mark::Mark;
use crate::shader_pipeline::ComposableShaderPipeline;
use std::collections::HashMap;

/// An axis-aligned 3D box mark.
#[derive(Debug, Clone, gup_macros::MarkTypeId)]
#[mark_type_id = 21]
pub struct Box3D;

/// GPU vertex for the unit cube.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Box3DVertex {
    /// Position of the cube corner in \[-1, 1\]³.
    pub position: [f32; 3],
    /// Face normal.
    pub normal: [f32; 3],
    /// Padding for alignment (vec3 → 12 bytes; pad to 16 would help but
    /// wgpu vertex attributes handle 12-byte strides fine).
    pub _pad: [f32; 2],
}

/// Per-instance data for a 3D box.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Box3DInstance {
    /// World-space centre.
    pub center: [f32; 3],
    pub _pad0: f32,
    /// Half-extents (size/2 along each axis).
    pub half_extents: [f32; 3],
    pub _pad1: f32,
    /// Base colour (RGBA).
    pub color: [f32; 4],
    /// Material: albedo.xyz, ambient.
    pub material_albedo_ambient: [f32; 4],
    /// Material: diffuse, specular, shininess, _pad.
    pub material_dss: [f32; 4],
}

/// High-level attributes for configuring a box.
#[derive(Debug, Clone)]
pub struct Box3DAttributes {
    pub center: [f32; 3],
    pub half_extents: [f32; 3],
    pub color: [f32; 4],
}

impl Default for Box3DAttributes {
    fn default() -> Self {
        Self {
            center: [0.0; 3],
            half_extents: [0.05; 3],
            color: [0.8, 0.4, 0.2, 1.0],
        }
    }
}

// ---------------------------------------------------------------------------
// Unit-cube geometry generation
// ---------------------------------------------------------------------------

/// Generate the 24 unique vertices (4 per face × 6 faces) of a unit cube
/// with outward-facing normals.
fn generate_cube_vertices() -> Vec<Box3DVertex> {
    #[rustfmt::skip]
    let data: &[([f32; 3], [f32; 3])] = &[
        // +Z face
        ([-1.0, -1.0,  1.0], [ 0.0,  0.0,  1.0]),
        ([ 1.0, -1.0,  1.0], [ 0.0,  0.0,  1.0]),
        ([ 1.0,  1.0,  1.0], [ 0.0,  0.0,  1.0]),
        ([-1.0,  1.0,  1.0], [ 0.0,  0.0,  1.0]),
        // -Z face
        ([ 1.0, -1.0, -1.0], [ 0.0,  0.0, -1.0]),
        ([-1.0, -1.0, -1.0], [ 0.0,  0.0, -1.0]),
        ([-1.0,  1.0, -1.0], [ 0.0,  0.0, -1.0]),
        ([ 1.0,  1.0, -1.0], [ 0.0,  0.0, -1.0]),
        // +X face
        ([ 1.0, -1.0,  1.0], [ 1.0,  0.0,  0.0]),
        ([ 1.0, -1.0, -1.0], [ 1.0,  0.0,  0.0]),
        ([ 1.0,  1.0, -1.0], [ 1.0,  0.0,  0.0]),
        ([ 1.0,  1.0,  1.0], [ 1.0,  0.0,  0.0]),
        // -X face
        ([-1.0, -1.0, -1.0], [-1.0,  0.0,  0.0]),
        ([-1.0, -1.0,  1.0], [-1.0,  0.0,  0.0]),
        ([-1.0,  1.0,  1.0], [-1.0,  0.0,  0.0]),
        ([-1.0,  1.0, -1.0], [-1.0,  0.0,  0.0]),
        // +Y face
        ([-1.0,  1.0,  1.0], [ 0.0,  1.0,  0.0]),
        ([ 1.0,  1.0,  1.0], [ 0.0,  1.0,  0.0]),
        ([ 1.0,  1.0, -1.0], [ 0.0,  1.0,  0.0]),
        ([-1.0,  1.0, -1.0], [ 0.0,  1.0,  0.0]),
        // -Y face
        ([-1.0, -1.0, -1.0], [ 0.0, -1.0,  0.0]),
        ([ 1.0, -1.0, -1.0], [ 0.0, -1.0,  0.0]),
        ([ 1.0, -1.0,  1.0], [ 0.0, -1.0,  0.0]),
        ([-1.0, -1.0,  1.0], [ 0.0, -1.0,  0.0]),
    ];

    data.iter()
        .map(|(pos, norm)| Box3DVertex {
            position: *pos,
            normal: *norm,
            _pad: [0.0; 2],
        })
        .collect()
}

/// Generate indices for the unit cube (6 faces × 2 triangles × 3 indices).
fn generate_cube_indices() -> Vec<u32> {
    let mut indices = Vec::with_capacity(36);
    for face in 0..6u32 {
        let base = face * 4;
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    indices
}

// ---------------------------------------------------------------------------
// Mark implementation
// ---------------------------------------------------------------------------

impl Mark for Box3D {
    type Vertex = Box3DVertex;
    type AttributeValue = Box3DAttributes;

    const VERTEX_SHADER: Option<&'static str> = Some(include_str!("shaders/box3d.vert.wgsl"));
    const FRAGMENT_SHADER: Option<&'static str> = Some(include_str!("shaders/box3d.frag.wgsl"));

    fn generate_vertex_shader(pipeline: &ComposableShaderPipeline) -> String {
        Self::generate_vertex_shader_with_functions(pipeline, &HashMap::new())
    }

    fn generate_fragment_shader(pipeline: &ComposableShaderPipeline) -> String {
        Self::generate_fragment_shader_with_functions(pipeline, &HashMap::new())
    }

    fn vertex_count() -> usize {
        24
    }

    fn index_count() -> Option<usize> {
        Some(36)
    }

    fn generate_vertices() -> Vec<Self::Vertex> {
        generate_cube_vertices()
    }

    fn generate_indices() -> Option<Vec<u32>> {
        Some(generate_cube_indices())
    }

    fn vertex_attributes() -> &'static [wgpu::VertexAttribute] {
        &[
            wgpu::VertexAttribute {
                offset: 0,
                shader_location: 0,
                format: wgpu::VertexFormat::Float32x3, // position
            },
            wgpu::VertexAttribute {
                offset: 12,
                shader_location: 1,
                format: wgpu::VertexFormat::Float32x3, // normal
            },
        ]
    }

    fn get_attribute_type(attribute_name: &str) -> crate::error::GupResult<&'static str> {
        match attribute_name {
            "center" | "position" => Ok("vec3<f32>"),
            "half_extents" | "size" => Ok("vec3<f32>"),
            "color" | "fill_color" => Ok("vec4<f32>"),
            _ => Err(crate::error::GupError::validation_error(format!(
                "Unknown Box3D attribute: {attribute_name}"
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
    fn cube_geometry() {
        let verts = Box3D::generate_vertices();
        assert_eq!(verts.len(), 24); // 6 faces × 4 corners

        let indices = Box3D::generate_indices().unwrap();
        assert_eq!(indices.len(), 36); // 6 faces × 6 indices

        // All indices should be in range.
        assert!(indices.iter().all(|&i| (i as usize) < verts.len()));
    }

    #[test]
    fn instance_bytemuck() {
        let inst = Box3DInstance {
            center: [0.0; 3],
            _pad0: 0.0,
            half_extents: [1.0; 3],
            _pad1: 0.0,
            color: [1.0, 0.0, 0.0, 1.0],
            material_albedo_ambient: [0.8, 0.8, 0.8, 0.15],
            material_dss: [0.7, 0.5, 32.0, 0.0],
        };
        let bytes: &[u8] = bytemuck::bytes_of(&inst);
        assert_eq!(bytes.len(), std::mem::size_of::<Box3DInstance>());
    }

    #[test]
    fn normals_are_axis_aligned() {
        let verts = generate_cube_vertices();
        for v in &verts {
            let n = v.normal;
            // Exactly one component should be ±1.
            let sum = n[0].abs() + n[1].abs() + n[2].abs();
            assert!((sum - 1.0).abs() < 1e-6, "non-unit normal: {n:?}");
        }
    }
}

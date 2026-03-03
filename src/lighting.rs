// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Lighting and material types for 3D mark rendering.
//!
//! Provides [`Material`] and [`LightUniform`] as GPU-ready structs for
//! Phong / Blinn-Phong shading, plus a reusable WGSL snippet that 3D
//! fragment shaders can include.

/// Surface material properties for Phong-style lighting.
///
/// All fields use `bytemuck::Pod` so the struct can be uploaded directly to
/// a `wgpu::Buffer`.
///
/// # WGSL Layout
///
/// ```wgsl
/// struct Material {
///     albedo: vec3<f32>,   // 12 bytes
///     ambient: f32,        // 4  bytes
///     diffuse: f32,        // 4  bytes
///     specular: f32,       // 4  bytes
///     shininess: f32,      // 4  bytes
///     _pad: f32,           // 4  bytes  → total 32, 16-byte aligned
/// }
/// ```
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Material {
    /// Base colour of the surface (linear RGB, 0..1).
    pub albedo: [f32; 3],
    /// Ambient reflectance coefficient (0..1).
    pub ambient: f32,
    /// Diffuse reflectance coefficient (0..1).
    pub diffuse: f32,
    /// Specular reflectance coefficient (0..1).
    pub specular: f32,
    /// Shininess exponent for specular highlights.
    pub shininess: f32,
    /// Padding to reach 32-byte alignment.
    pub _pad: f32,
}

impl Default for Material {
    fn default() -> Self {
        Self {
            albedo: [0.8, 0.8, 0.8],
            ambient: 0.15,
            diffuse: 0.7,
            specular: 0.5,
            shininess: 32.0,
            _pad: 0.0,
        }
    }
}

/// Directional light uniform for GPU upload.
///
/// # WGSL Layout
///
/// ```wgsl
/// struct LightUniform {
///     direction: vec3<f32>,
///     _pad: f32,
///     color: vec3<f32>,
///     intensity: f32,
/// }
/// ```
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LightUniform {
    /// Direction *toward* the light (normalised).
    pub direction: [f32; 3],
    /// Padding for `vec4` alignment.
    pub _pad: f32,
    /// Light colour (linear RGB).
    pub color: [f32; 3],
    /// Light intensity multiplier.
    pub intensity: f32,
}

impl Default for LightUniform {
    fn default() -> Self {
        // Gentle top-right-front key light
        let dir = [0.3_f32, 0.8, 0.5];
        let len = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt();
        Self {
            direction: [dir[0] / len, dir[1] / len, dir[2] / len],
            _pad: 0.0,
            color: [1.0, 1.0, 1.0],
            intensity: 1.0,
        }
    }
}

/// WGSL source for the reusable `phong_lighting` and `blinn_phong_lighting`
/// functions.  Include this in any 3D fragment shader module.
pub const PHONG_WGSL: &str = include_str!("shaders/phong.wgsl");

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn material_bytemuck() {
        let m = Material::default();
        let bytes: &[u8] = bytemuck::bytes_of(&m);
        assert_eq!(bytes.len(), std::mem::size_of::<Material>());
        assert_eq!(bytes.len(), 32); // 8 × f32
    }

    #[test]
    fn light_uniform_bytemuck() {
        let l = LightUniform::default();
        let bytes: &[u8] = bytemuck::bytes_of(&l);
        assert_eq!(bytes.len(), std::mem::size_of::<LightUniform>());
        assert_eq!(bytes.len(), 32); // 8 × f32
    }

    #[test]
    fn light_direction_normalized() {
        let l = LightUniform::default();
        let len = (l.direction[0] * l.direction[0]
            + l.direction[1] * l.direction[1]
            + l.direction[2] * l.direction[2])
            .sqrt();
        assert!((len - 1.0).abs() < 1e-5, "direction length = {len}");
    }

    #[test]
    fn material_default_values() {
        let m = Material::default();
        assert_eq!(m.albedo, [0.8, 0.8, 0.8]);
        assert_eq!(m.ambient, 0.15);
        assert_eq!(m.diffuse, 0.7);
        assert_eq!(m.specular, 0.5);
        assert_eq!(m.shininess, 32.0);
    }
}

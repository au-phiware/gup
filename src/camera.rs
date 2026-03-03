// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! 3D camera abstraction for view and projection matrix management.
//!
//! Provides perspective and orthographic projections, a `look_at` view
//! matrix helper, and a GPU-ready [`CameraUniform`] that can be uploaded
//! directly to a wgpu uniform buffer.

/// 3D camera with projection and view matrices.
///
/// Create via [`Camera::perspective`] or [`Camera::orthographic`], then
/// position with [`Camera::look_at`].
///
/// # Example
///
/// ```rust
/// use gup::camera::Camera;
/// use gup::shader_function::Vec3;
///
/// let mut cam = Camera::perspective(
///     std::f32::consts::FRAC_PI_4,
///     16.0 / 9.0,
///     0.1,
///     100.0,
/// );
/// cam.look_at(
///     Vec3::new(0.0, 0.0, 5.0),
///     Vec3::new(0.0, 0.0, 0.0),
///     Vec3::new(0.0, 1.0, 0.0),
/// );
/// let uniform = cam.to_uniform();
/// ```
#[derive(Debug, Clone)]
pub struct Camera {
    /// Column-major projection matrix.
    projection: [[f32; 4]; 4],
    /// Column-major view matrix.
    view: [[f32; 4]; 4],
    /// Column-major model matrix.
    model: [[f32; 4]; 4],
}

/// GPU-ready camera uniform (view + projection + model matrices).
///
/// The struct is `bytemuck::Pod + Zeroable` so it can be written directly to a
/// `wgpu::Buffer` with `queue.write_buffer`.
///
/// Layout: three column-major `mat4x4<f32>` values (192 bytes total).
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniform {
    /// Column-major view matrix.
    pub view: [[f32; 4]; 4],
    /// Column-major projection matrix.
    pub projection: [[f32; 4]; 4],
    /// Column-major model matrix.
    pub model: [[f32; 4]; 4],
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// 4×4 identity matrix (column-major).
const fn identity() -> [[f32; 4]; 4] {
    [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

fn vec3_sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn vec3_cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn vec3_dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn vec3_normalize(v: [f32; 3]) -> [f32; 3] {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if len < 1e-10 {
        return [0.0; 3];
    }
    [v[0] / len, v[1] / len, v[2] / len]
}

// ---------------------------------------------------------------------------
// Camera implementation
// ---------------------------------------------------------------------------

impl Camera {
    /// Create a camera with a **perspective** projection.
    ///
    /// Uses a right-handed coordinate system with depth mapped to `[0, 1]`
    /// (wgpu / Vulkan convention).
    ///
    /// * `fov_y_radians` – vertical field of view.
    /// * `aspect` – width / height.
    /// * `near`, `far` – clipping planes (both positive).
    pub fn perspective(fov_y_radians: f32, aspect: f32, near: f32, far: f32) -> Self {
        let f = 1.0 / (fov_y_radians / 2.0).tan();
        let range_inv = 1.0 / (near - far);

        // Column-major, right-handed, depth [0..1] (wgpu convention)
        #[rustfmt::skip]
        let projection = [
            [f / aspect,  0.0,  0.0,                       0.0],
            [0.0,         f,    0.0,                       0.0],
            [0.0,         0.0,  far * range_inv,          -1.0],
            [0.0,         0.0,  near * far * range_inv,    0.0],
        ];

        Self {
            projection,
            view: identity(),
            model: identity(),
        }
    }

    /// Create a camera with an **orthographic** projection.
    ///
    /// Depth is mapped to `[0, 1]` (wgpu convention).
    pub fn orthographic(left: f32, right: f32, bottom: f32, top: f32, near: f32, far: f32) -> Self {
        let rml = right - left;
        let tmb = top - bottom;
        let fmn = far - near;

        #[rustfmt::skip]
        let projection = [
            [2.0 / rml,  0.0,        0.0,          0.0],
            [0.0,        2.0 / tmb,  0.0,          0.0],
            [0.0,        0.0,       -1.0 / fmn,    0.0],
            [-(right + left) / rml, -(top + bottom) / tmb, -near / fmn, 1.0],
        ];

        Self {
            projection,
            view: identity(),
            model: identity(),
        }
    }

    /// Set the **view** matrix by specifying eye, center, and up.
    ///
    /// Uses the standard right-handed look-at formulation.
    pub fn look_at(
        &mut self,
        eye: crate::shader_function::Vec3,
        center: crate::shader_function::Vec3,
        up: crate::shader_function::Vec3,
    ) {
        let eye = [eye.x, eye.y, eye.z];
        let center = [center.x, center.y, center.z];
        let up = [up.x, up.y, up.z];

        let f = vec3_normalize(vec3_sub(center, eye));
        let s = vec3_normalize(vec3_cross(f, up));
        let u = vec3_cross(s, f);

        #[rustfmt::skip]
        let view = [
            [ s[0],              u[0],             -f[0],             0.0],
            [ s[1],              u[1],             -f[1],             0.0],
            [ s[2],              u[2],             -f[2],             0.0],
            [-vec3_dot(s, eye), -vec3_dot(u, eye),  vec3_dot(f, eye), 1.0],
        ];

        self.view = view;
    }

    /// Replace the model matrix.
    pub fn set_model(&mut self, model: [[f32; 4]; 4]) {
        self.model = model;
    }

    /// Read-only access to the projection matrix.
    pub fn projection(&self) -> &[[f32; 4]; 4] {
        &self.projection
    }

    /// Read-only access to the view matrix.
    pub fn view_matrix(&self) -> &[[f32; 4]; 4] {
        &self.view
    }

    /// Read-only access to the model matrix.
    pub fn model(&self) -> &[[f32; 4]; 4] {
        &self.model
    }

    /// Build the GPU-ready [`CameraUniform`].
    pub fn to_uniform(&self) -> CameraUniform {
        CameraUniform {
            view: self.view,
            projection: self.projection,
            model: self.model,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shader_function::Vec3;
    use std::f32::consts::{FRAC_PI_2, FRAC_PI_4};

    #[test]
    fn perspective_maps_near_and_far() {
        let cam = Camera::perspective(FRAC_PI_2, 1.0, 0.1, 100.0);
        let p = cam.projection;

        // A point on the near plane (0,0,-near) in view space should map to
        // NDC z = 0 after perspective divide.
        let near = 0.1_f32;
        let z_clip = p[2][2] * (-near) + p[3][2];
        let w_clip = p[2][3] * (-near) + p[3][3];
        let ndc_z = z_clip / w_clip;
        assert!((ndc_z - 0.0).abs() < 1e-5, "near plane NDC z = {ndc_z}");

        // A point on the far plane (0,0,-far) should map to NDC z = 1.
        let far = 100.0_f32;
        let z_clip = p[2][2] * (-far) + p[3][2];
        let w_clip = p[2][3] * (-far) + p[3][3];
        let ndc_z = z_clip / w_clip;
        assert!((ndc_z - 1.0).abs() < 1e-5, "far plane NDC z = {ndc_z}");
    }

    #[test]
    fn perspective_aspect_ratio() {
        let cam = Camera::perspective(FRAC_PI_4, 2.0, 0.1, 100.0);
        let p = cam.projection;
        // p[0][0] should be half of p[1][1] when aspect = 2
        assert!(
            (p[0][0] - p[1][1] / 2.0).abs() < 1e-6,
            "aspect: p00={} p11={}",
            p[0][0],
            p[1][1]
        );
    }

    #[test]
    fn orthographic_identity_like() {
        let cam = Camera::orthographic(-1.0, 1.0, -1.0, 1.0, 0.0, 1.0);
        let p = cam.projection;
        // Should map (-1..1) → (-1..1) in x and y, and (0..1) → (0..1) in z.
        assert!((p[0][0] - 1.0).abs() < 1e-6);
        assert!((p[1][1] - 1.0).abs() < 1e-6);
        // z: -1/fmn = -1/1 = -1
        assert!((p[2][2] - (-1.0)).abs() < 1e-6);
    }

    #[test]
    fn look_at_orthonormal_basis() {
        let mut cam = Camera::perspective(FRAC_PI_4, 1.0, 0.1, 100.0);
        cam.look_at(
            Vec3::new(0.0, 0.0, 5.0),
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
        );

        let v = cam.view;
        // The upper-left 3×3 should be orthonormal (each column has unit length
        // and columns are perpendicular).
        for col in 0..3 {
            let len_sq = v[col][0] * v[col][0] + v[col][1] * v[col][1] + v[col][2] * v[col][2];
            assert!(
                (len_sq - 1.0).abs() < 1e-5,
                "column {col} length² = {len_sq}"
            );
        }

        // Dot product of columns 0 and 1 should be ~0.
        let dot01 = v[0][0] * v[1][0] + v[0][1] * v[1][1] + v[0][2] * v[1][2];
        assert!(dot01.abs() < 1e-5, "dot(col0,col1) = {dot01}");
    }

    #[test]
    fn look_at_places_eye() {
        let mut cam = Camera::perspective(FRAC_PI_4, 1.0, 0.1, 100.0);
        let eye = Vec3::new(3.0, 4.0, 5.0);
        cam.look_at(eye, Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0));

        // Multiplying the eye position by the view matrix should give (0,0,0,1)
        // in view space (the camera is at the origin of its own space).
        let v = cam.view;
        let vx = v[0][0] * eye.x + v[1][0] * eye.y + v[2][0] * eye.z + v[3][0];
        let vy = v[0][1] * eye.x + v[1][1] * eye.y + v[2][1] * eye.z + v[3][1];
        let vz = v[0][2] * eye.x + v[1][2] * eye.y + v[2][2] * eye.z + v[3][2];
        assert!(vx.abs() < 1e-4, "eye in view space x = {vx}");
        assert!(vy.abs() < 1e-4, "eye in view space y = {vy}");
        assert!(vz.abs() < 1e-4, "eye in view space z = {vz}");
    }

    #[test]
    fn camera_uniform_bytemuck() {
        let cam = Camera::perspective(FRAC_PI_4, 1.0, 0.1, 100.0);
        let uniform = cam.to_uniform();
        let bytes: &[u8] = bytemuck::bytes_of(&uniform);
        assert_eq!(bytes.len(), std::mem::size_of::<CameraUniform>());
        assert_eq!(bytes.len(), 192); // 3 * 64 bytes per mat4
    }

    #[test]
    fn model_defaults_to_identity() {
        let cam = Camera::perspective(FRAC_PI_4, 1.0, 0.1, 100.0);
        assert_eq!(*cam.model(), identity());
    }
}

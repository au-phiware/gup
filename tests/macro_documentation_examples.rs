// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for macro documentation examples
//!
//! Ensures that all code examples in the Type Construction Guide and
//! macro documentation compile and work correctly.

use gup::*;

#[test]
fn test_vec2_macro_basic() {
    let position = vec2![10.0, 20.0];
    assert_eq!(position.x, 10.0);
    assert_eq!(position.y, 20.0);
}

#[test]
fn test_vec3_macro_basic() {
    let position = vec3![1.0, 2.0, 3.0];
    assert_eq!(position.x, 1.0);
    assert_eq!(position.y, 2.0);
    assert_eq!(position.z, 3.0);
    // Padding is automatically set
    assert_eq!(position._padding, 0.0);
}

#[test]
fn test_vec4_macro_basic() {
    // RGBA color: orange with full opacity
    let color = vec4![1.0, 0.5, 0.0, 1.0];
    assert_eq!(color.x, 1.0);
    assert_eq!(color.y, 0.5);
    assert_eq!(color.z, 0.0);
    assert_eq!(color.w, 1.0);
}

#[test]
fn test_vec4_homogeneous_coordinates() {
    // Homogeneous coordinates
    let position = vec4![10.0, 20.0, 0.0, 1.0];
    assert_eq!(position.x, 10.0);
    assert_eq!(position.y, 20.0);
    assert_eq!(position.z, 0.0);
    assert_eq!(position.w, 1.0);
}

#[test]
fn test_mat2_identity() {
    let identity = mat2![1.0, 0.0, 0.0, 1.0];
    assert_eq!(identity.m00, 1.0);
    assert_eq!(identity.m01, 0.0);
    assert_eq!(identity.m10, 0.0);
    assert_eq!(identity.m11, 1.0);
}

#[test]
fn test_mat2_rotation() {
    // 90-degree rotation
    let rotation = mat2![0.0, -1.0, 1.0, 0.0];
    assert_eq!(rotation.m00, 0.0);
    assert_eq!(rotation.m01, -1.0);
    assert_eq!(rotation.m10, 1.0);
    assert_eq!(rotation.m11, 0.0);
}

#[test]
fn test_mat3_identity() {
    let identity = mat3![1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];
    assert_eq!(identity.m00, 1.0);
    assert_eq!(identity.m11, 1.0);
    assert_eq!(identity.m22, 1.0);
    // Off-diagonal should be zero
    assert_eq!(identity.m01, 0.0);
    assert_eq!(identity.m10, 0.0);
}

#[test]
fn test_mat3_affine_transform() {
    // 2D affine transformation (scale + translate)
    let transform = mat3![
        2.0, 0.0, 10.0, // Scale X=2, Translate X=10
        0.0, 2.0, 20.0, // Scale Y=2, Translate Y=20
        0.0, 0.0, 1.0 // Homogeneous coordinate
    ];
    assert_eq!(transform.m00, 2.0);
    assert_eq!(transform.m11, 2.0);
    assert_eq!(transform.m02, 10.0);
    assert_eq!(transform.m12, 20.0);
    assert_eq!(transform.m22, 1.0);
}

#[test]
fn test_mat4_identity() {
    let identity = mat4![
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0
    ];
    assert_eq!(identity.m00, 1.0);
    assert_eq!(identity.m11, 1.0);
    assert_eq!(identity.m22, 1.0);
    assert_eq!(identity.m33, 1.0);
}

#[test]
fn test_mat4_translation() {
    // Translation matrix
    let translate = mat4![
        1.0, 0.0, 0.0, 10.0, 0.0, 1.0, 0.0, 20.0, 0.0, 0.0, 1.0, 30.0, 0.0, 0.0, 0.0, 1.0
    ];
    assert_eq!(translate.m03, 10.0);
    assert_eq!(translate.m13, 20.0);
    assert_eq!(translate.m23, 30.0);
    assert_eq!(translate.m33, 1.0);
}

#[test]
fn test_vector_array_creation() {
    // Data transformation pipelines example
    let data_points = [vec2![10.0, 20.0], vec2![30.0, 40.0], vec2![50.0, 60.0]];
    assert_eq!(data_points.len(), 3);
    assert_eq!(data_points[0].x, 10.0);
    assert_eq!(data_points[1].x, 30.0);
    assert_eq!(data_points[2].x, 50.0);
}

#[test]
fn test_color_gradient_stops() {
    let gradient_stops = vec![
        vec4![1.0, 0.0, 0.0, 1.0], // Red
        vec4![1.0, 1.0, 0.0, 1.0], // Yellow
        vec4![0.0, 1.0, 0.0, 1.0], // Green
    ];
    assert_eq!(gradient_stops.len(), 3);
    // Red
    assert_eq!(gradient_stops[0].x, 1.0);
    assert_eq!(gradient_stops[0].y, 0.0);
    // Yellow
    assert_eq!(gradient_stops[1].x, 1.0);
    assert_eq!(gradient_stops[1].y, 1.0);
    // Green
    assert_eq!(gradient_stops[2].y, 1.0);
}

#[test]
fn test_gpu_vertex_data() {
    // Prepare vertex data for GPU
    let vertices = vec![
        vec3![0.0, 0.5, 0.0],   // Top
        vec3![-0.5, -0.5, 0.0], // Bottom left
        vec3![0.5, -0.5, 0.0],  // Bottom right
    ];
    assert_eq!(vertices.len(), 3);
    assert_eq!(vertices[0].y, 0.5);
    assert_eq!(vertices[1].x, -0.5);
    assert_eq!(vertices[2].x, 0.5);
}

#[test]
fn test_const_vector_usage() {
    const ORIGIN: Vec2 = vec2![0.0, 0.0];
    assert_eq!(ORIGIN.x, 0.0);
    assert_eq!(ORIGIN.y, 0.0);
}

#[test]
fn test_const_matrix_usage() {
    const IDENTITY_2X2: Mat2 = mat2![1.0, 0.0, 0.0, 1.0];
    assert_eq!(IDENTITY_2X2.m00, 1.0);
    assert_eq!(IDENTITY_2X2.m11, 1.0);
}

#[test]
fn test_quick_start_example() {
    // From Quick Start section
    let position = vec3![1.0, 2.0, 3.0];
    let color = vec4![1.0, 0.5, 0.0, 1.0];

    assert_eq!(position.x, 1.0);
    assert_eq!(color.w, 1.0);
}

#[test]
fn test_transform_matrix_example() {
    // From Quick Start section
    let transform = mat4![
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0
    ];

    // Verify diagonal is all 1.0
    assert_eq!(transform.m00, 1.0);
    assert_eq!(transform.m11, 1.0);
    assert_eq!(transform.m22, 1.0);
    assert_eq!(transform.m33, 1.0);
}

#[test]
fn test_macro_memory_layout() {
    // Verify Vec3 has correct size with padding
    assert_eq!(std::mem::size_of::<Vec3>(), 16);

    // Verify Vec2 has correct size
    assert_eq!(std::mem::size_of::<Vec2>(), 8);

    // Verify Vec4 has correct size
    assert_eq!(std::mem::size_of::<Vec4>(), 16);
}

#[test]
fn test_matrix_memory_layout() {
    // Mat2 has padding for GPU alignment
    assert!(std::mem::size_of::<Mat2>() >= 16);

    // Mat3 has padding between rows
    assert!(std::mem::size_of::<Mat3>() >= 36);

    // Mat4 is naturally aligned
    assert_eq!(std::mem::size_of::<Mat4>(), 64);
}

// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Custom Mark Development Kit Example
//!
//! Demonstrates how to create custom marks using the `#[derive(Mark)]` macro
//! and validate them with the `MarkValidator` framework.

use gup::mark::Mark;
use gup::mark::validation::{MarkProfiler, MarkValidator, assert_mark_valid};
use gup::shader_function::{Vec2, Vec4};

// ============================================================
// Example 1: Simple diamond mark using derive macro (quad)
// ============================================================

/// A diamond-shaped mark for rendering rotated squares.
///
/// Uses `#[derive(Mark)]` with quad primitive — the base geometry
/// is a unit quad that gets rotated 45° by the shader.
#[derive(Debug, Clone, gup_macros::Mark)]
#[mark(primitive = "quad")]
pub struct Diamond {
    /// Center position in clip space.
    pub center: Vec2,
    /// Scale factor for the diamond.
    pub size: f32,
    /// Fill color (RGBA).
    pub color: Vec4,
    /// Rotation angle in radians.
    pub angle: f32,
}

// ============================================================
// Example 2: Arrow mark using derive macro (triangle)
// ============================================================

/// A simple directional arrow mark.
///
/// Uses `#[derive(Mark)]` with triangle primitive — three vertices
/// forming an arrow pointing upward.
#[derive(Debug, Clone, gup_macros::Mark)]
#[mark(primitive = "triangle")]
pub struct Arrow {
    /// Tip position of the arrow.
    pub position: Vec2,
    /// Size of the arrow.
    pub size: f32,
    /// Color of the arrow (RGBA).
    pub color: Vec4,
}

// ============================================================
// Example 3: Fully manual mark implementation
// ============================================================

/// A hexagon mark with hand-written implementation.
///
/// This shows the "advanced" path: implementing `Mark` directly
/// for full control over geometry and shaders.
#[derive(Debug, Clone)]
pub struct Hexagon;

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct HexagonVertex {
    pub position: [f32; 2],
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct HexagonAttributes {
    pub center: Vec2,
    pub radius: f32,
    pub color: Vec4,
}

impl Mark for Hexagon {
    type Vertex = HexagonVertex;
    type AttributeValue = HexagonAttributes;

    fn vertex_count() -> usize {
        7 // Center + 6 outer vertices
    }

    fn index_count() -> Option<usize> {
        Some(18) // 6 triangles × 3 indices
    }

    fn generate_vertices() -> Vec<Self::Vertex> {
        let mut vertices = vec![HexagonVertex {
            position: [0.0, 0.0],
        }]; // center

        for i in 0..6 {
            let angle = std::f32::consts::FRAC_PI_3 * i as f32;
            vertices.push(HexagonVertex {
                position: [angle.cos(), angle.sin()],
            });
        }

        vertices
    }

    fn generate_indices() -> Option<Vec<u32>> {
        // 6 triangles from center to each edge
        Some(vec![
            0, 1, 2, // triangle 1
            0, 2, 3, // triangle 2
            0, 3, 4, // triangle 3
            0, 4, 5, // triangle 4
            0, 5, 6, // triangle 5
            0, 6, 1, // triangle 6
        ])
    }
}

fn main() {
    println!("=== Custom Mark Development Kit Demo ===\n");

    // Validate the derive-generated Diamond mark
    println!("--- Diamond Mark (derive macro, quad) ---");
    let report = MarkValidator::<Diamond>::validate();
    println!("{}", report.summary());

    // Validate the derive-generated Arrow mark
    println!("--- Arrow Mark (derive macro, triangle) ---");
    let report = MarkValidator::<Arrow>::validate();
    println!("{}", report.summary());

    // Validate the hand-written Hexagon mark
    println!("--- Hexagon Mark (manual implementation) ---");
    let report = MarkValidator::<Hexagon>::validate();
    println!("{}", report.summary());

    // Demonstrate assert_mark_valid convenience function
    println!("--- Quick validation checks ---");
    assert_mark_valid::<Diamond>().expect("Diamond should be valid");
    println!("  ✅ Diamond: valid");
    assert_mark_valid::<Arrow>().expect("Arrow should be valid");
    println!("  ✅ Arrow: valid");
    assert_mark_valid::<Hexagon>().expect("Hexagon should be valid");
    println!("  ✅ Hexagon: valid");

    // Profile each mark
    println!("\n--- Performance profiles ---");
    let profile = MarkProfiler::<Diamond>::profile();
    println!("{}", profile.summary());

    let profile = MarkProfiler::<Arrow>::profile();
    println!("{}", profile.summary());

    let profile = MarkProfiler::<Hexagon>::profile();
    println!("{}", profile.summary());

    // Show attribute type information
    println!("--- Attribute types ---");
    println!(
        "  Diamond.center: {}",
        Diamond::get_attribute_type("center").unwrap()
    );
    println!(
        "  Diamond.size:   {}",
        Diamond::get_attribute_type("size").unwrap()
    );
    println!(
        "  Diamond.color:  {}",
        Diamond::get_attribute_type("color").unwrap()
    );
    println!(
        "  Diamond.angle:  {}",
        Diamond::get_attribute_type("angle").unwrap()
    );

    println!("\n=== All custom marks validated and profiled successfully! ===");
}

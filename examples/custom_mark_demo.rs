// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Custom Mark Development Kit Example
//!
//! Demonstrates how to create custom marks using the `#[derive(Mark)]` macro
//! and validate them with the `MarkValidator` framework, including automatic
//! GPU instance buffer generation via field annotations.

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
///
/// Field annotations (`#[mark(position)]`, etc.) automatically generate
/// a `DiamondInstance` struct with GPU-compatible layout and alignment.
#[derive(Debug, Clone, gup_macros::Mark)]
#[mark(primitive = "quad")]
pub struct Diamond {
    /// Center position in clip space.
    #[mark(position)]
    pub center: Vec2,
    /// Scale factor for the diamond.
    #[mark(size)]
    pub size: f32,
    /// Fill color (RGBA).
    #[mark(color)]
    pub color: Vec4,
    /// Rotation angle in radians.
    #[mark(rotation)]
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
    #[mark(position)]
    pub position: Vec2,
    /// Size of the arrow.
    #[mark(size)]
    pub size: f32,
    /// Color of the arrow (RGBA).
    #[mark(color)]
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

    // ============================================================
    // Example 4: Auto-generated instance buffers
    // ============================================================
    println!("\n--- Instance Buffer Generation ---");

    // Create a Diamond with data
    let diamond = Diamond {
        center: Vec2 { x: 0.5, y: 0.5 },
        size: 0.1,
        color: Vec4 {
            x: 1.0,
            y: 0.2,
            z: 0.3,
            w: 1.0,
        },
        angle: std::f32::consts::FRAC_PI_4,
    };

    // Convert to GPU instance — auto-generated by #[mark(..)] annotations
    let instance = DiamondInstance::from(&diamond);
    println!("  DiamondInstance: {instance:?}");
    println!(
        "    size: {} bytes, alignment-padded for WGSL",
        std::mem::size_of::<DiamondInstance>()
    );

    // Verify bytemuck compatibility (storage buffer upload)
    let bytes: &[u8] = bytemuck::bytes_of(&instance);
    println!("    bytemuck bytes: {} bytes", bytes.len());

    // Batch conversion for storage buffer
    let diamonds = [
        Diamond {
            center: Vec2 { x: -0.5, y: 0.5 },
            size: 0.08,
            color: Vec4 {
                x: 0.0,
                y: 0.8,
                z: 0.2,
                w: 1.0,
            },
            angle: 0.0,
        },
        Diamond {
            center: Vec2 { x: 0.5, y: -0.5 },
            size: 0.12,
            color: Vec4 {
                x: 0.2,
                y: 0.3,
                z: 1.0,
                w: 0.9,
            },
            angle: 1.0,
        },
    ];
    let instances: Vec<DiamondInstance> = diamonds.iter().map(DiamondInstance::from).collect();
    let buffer_bytes: &[u8] = bytemuck::cast_slice(&instances);
    println!(
        "  Batch: {} instances → {} bytes for GPU upload",
        instances.len(),
        buffer_bytes.len()
    );

    // Arrow instance
    let arrow = Arrow {
        position: Vec2 { x: 0.0, y: 0.0 },
        size: 0.15,
        color: Vec4 {
            x: 1.0,
            y: 1.0,
            z: 0.0,
            w: 1.0,
        },
    };
    let arrow_instance = ArrowInstance::from(&arrow);
    println!("  ArrowInstance: {arrow_instance:?}");
    println!("    size: {} bytes", std::mem::size_of::<ArrowInstance>());

    println!("\n=== All custom marks validated and profiled successfully! ===");
}

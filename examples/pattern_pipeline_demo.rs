// Gup - GPU-Accelerated Data Visualization Library
// Copyright (C) 2025 Corin Lawson <corin@phiware.com.au>
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

//! # Pattern Pipeline Integration Demo
//!
//! Demonstrates the integration of pattern rendering into the mark pipeline for
//! accessibility. Shows how to:
//! - Create pattern-enabled render pipelines
//! - Configure pattern uniforms
//! - Switch between standard and pattern rendering modes
//! - Use patterns for colorblind-accessible data encoding
//!
//! Run with: `cargo run --example pattern_pipeline_demo`

use gup::accessibility::{AccessibilitySystem, Color, ContrastMode, Pattern, PatternRenderer, PatternUniforms};
use gup::context::GupContext;
use gup::error::GupResult;
use gup::mark::{Circle, Mark, MarkInfo, MarkInfoImpl, MarkRenderer};
use std::sync::Arc;

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct CircleInstanceData {
    center: [f32; 2],
    radius: f32,
    fill_color: [f32; 4],
    stroke_width: f32,
    stroke_color: [f32; 4],
    _padding: [f32; 2],
}

#[tokio::main]
async fn main() -> GupResult<()> {
    println!("Pattern Pipeline Integration Demo");
    println!("==================================\n");

    // Create GPU context
    let context = Arc::new(GupContext::headless().await?);
    let device = &context.device;
    let queue = &context.queue;

    // Demonstrate pattern pipeline creation
    demonstrate_pipeline_creation(device)?;

    // Demonstrate pattern renderer setup
    demonstrate_pattern_renderer(device, queue)?;

    // Demonstrate complete rendering setup
    demonstrate_rendering_setup(device, queue)?;

    // Demonstrate accessibility integration
    demonstrate_accessibility_integration()?;

    println!("\n✓ Pattern pipeline integration complete!");
    println!("  Patterns are now fully integrated into the mark rendering pipeline");
    println!("  Use ContrastMode::Pattern to enable pattern-based rendering");
    println!("  Patterns provide color-independent data encoding for accessibility");

    Ok(())
}

/// Demonstrate creating pattern-enabled pipelines.
fn demonstrate_pipeline_creation(device: &wgpu::Device) -> GupResult<()> {
    println!("1. Pattern Pipeline Creation");
    println!("-----------------------------");

    let mark_info = MarkInfoImpl::<Circle>::new();

    // Check if mark supports patterns
    if mark_info.has_pattern_shader() {
        println!("✓ Circle mark has pattern shader support");

        // Create standard pipeline
        let start = std::time::Instant::now();
        let _standard_pipeline = mark_info.create_render_pipeline(device)?;
        let standard_time = start.elapsed();
        println!("  Standard pipeline created in {:?}", standard_time);

        // Create pattern pipeline
        let start = std::time::Instant::now();
        let _pattern_pipeline = mark_info.create_render_pipeline_with_patterns(device)?;
        let pattern_time = start.elapsed();
        println!("  Pattern pipeline created in {:?}", pattern_time);

        println!("  Both pipelines created successfully\n");
    }

    Ok(())
}

/// Demonstrate pattern renderer configuration.
fn demonstrate_pattern_renderer(device: &wgpu::Device, queue: &wgpu::Queue) -> GupResult<()> {
    println!("2. Pattern Renderer Configuration");
    println!("----------------------------------");

    // Create different pattern types
    let patterns = vec![
        ("Solid (no pattern)", Pattern::Solid),
        ("Dots (spacing: 8.0)", Pattern::Dots { spacing: 8.0 }),
        (
            "Lines (horizontal)",
            Pattern::Lines {
                spacing: 6.0,
                angle: 0.0,
            },
        ),
        (
            "Lines (diagonal 45°)",
            Pattern::Lines {
                spacing: 6.0,
                angle: std::f32::consts::PI / 4.0,
            },
        ),
        (
            "Crosshatch (spacing: 8.0)",
            Pattern::Crosshatch { spacing: 8.0 },
        ),
    ];

    // Create renderer and demonstrate pattern switching
    let initial_uniforms = PatternUniforms::from_pattern(&Pattern::Solid, Color::BLACK, Color::WHITE);
    let mut renderer = PatternRenderer::new(device, initial_uniforms);

    for (name, pattern) in patterns {
        let uniforms = PatternUniforms::from_pattern(&pattern, Color::BLACK, Color::WHITE);
        renderer.update(queue, uniforms);
        println!("  ✓ {}: type_id={}", name, uniforms.pattern_type);
    }

    println!();
    Ok(())
}

/// Demonstrate complete rendering setup with patterns.
fn demonstrate_rendering_setup(device: &wgpu::Device, queue: &wgpu::Queue) -> GupResult<()> {
    println!("3. Complete Rendering Setup");
    println!("----------------------------");

    // Create mark renderer
    let mut mark_renderer = MarkRenderer::new(device);

    // Upload vertices
    let vertices = Circle::generate_vertices();
    mark_renderer.upload_vertices(device, queue, &vertices)?;
    println!("  ✓ Uploaded {} vertices", vertices.len());

    // Create sample instance data
    let instances = vec![
        CircleInstanceData {
            center: [100.0, 100.0],
            radius: 20.0,
            fill_color: [1.0, 0.0, 0.0, 1.0], // Red
            stroke_width: 2.0,
            stroke_color: [0.0, 0.0, 0.0, 1.0],
            _padding: [0.0; 2],
        },
        CircleInstanceData {
            center: [200.0, 100.0],
            radius: 20.0,
            fill_color: [0.0, 1.0, 0.0, 1.0], // Green
            stroke_width: 2.0,
            stroke_color: [0.0, 0.0, 0.0, 1.0],
            _padding: [0.0; 2],
        },
        CircleInstanceData {
            center: [300.0, 100.0],
            radius: 20.0,
            fill_color: [0.0, 0.0, 1.0, 1.0], // Blue
            stroke_width: 2.0,
            stroke_color: [0.0, 0.0, 0.0, 1.0],
            _padding: [0.0; 2],
        },
    ];

    mark_renderer.upload_instances(device, queue, &instances)?;
    println!("  ✓ Uploaded {} instances", instances.len());

    // Upload indices
    if let Some(indices) = Circle::generate_indices() {
        mark_renderer.upload_indices(device, queue, &indices)?;
        println!("  ✓ Uploaded {} indices", indices.len());
    }

    // Create pattern renderer for accessibility
    let pattern = Pattern::Dots { spacing: 8.0 };
    let pattern_uniforms = PatternUniforms::from_pattern(&pattern, Color::BLACK, Color::WHITE);
    let _pattern_renderer = PatternRenderer::new(device, pattern_uniforms);
    println!("  ✓ Pattern renderer ready with {:?}", pattern);

    println!("  ✓ All rendering resources uploaded\n");

    Ok(())
}

/// Demonstrate accessibility system integration.
fn demonstrate_accessibility_integration() -> GupResult<()> {
    println!("4. Accessibility Integration");
    println!("----------------------------");

    let mut accessibility = AccessibilitySystem::new();

    println!("  Current contrast mode: {:?}", accessibility.contrast_mode());

    // Switch to pattern mode
    accessibility.set_contrast_mode(ContrastMode::Pattern);
    println!("  ✓ Switched to Pattern mode");

    // Get patterns for different categories
    println!("\n  Pattern assignments for categories:");
    for category in 0..6 {
        let pattern = accessibility
            .high_contrast_renderer
            .get_pattern_for_category(category);
        println!("    Category {}: {:?}", category, pattern);
    }

    println!("\n  Patterns provide color-independent data encoding");
    println!("  Essential for colorblind and low-vision users");

    Ok(())
}

/// Demonstrate usage in rendering loop (conceptual).
#[allow(dead_code)]
fn rendering_loop_example() {
    println!("\n5. Rendering Loop Integration (Conceptual)");
    println!("-------------------------------------------");
    println!("
In a real rendering loop:

```rust
// Create render pass
let mut render_pass = encoder.begin_render_pass(&desc);

// Choose pipeline based on accessibility mode
if accessibility.contrast_mode() == ContrastMode::Pattern {{
    // Use pattern pipeline
    let pattern_pipeline = registry.get_pattern_pipeline::<Circle>(device)?;
    let pattern_bind_group = pattern_renderer.bind_group();
    
    mark_renderer.render_marks_with_patterns::<Circle>(
        &mut render_pass,
        pattern_pipeline,
        instance_bind_group,
        pattern_bind_group,
        instance_count,
    )?;
}} else {{
    // Use standard pipeline
    let pipeline = registry.get_pipeline::<Circle>(device)?;
    
    mark_renderer.render_marks::<Circle>(
        &mut render_pass,
        pipeline,
        instance_bind_group,
        instance_count,
    )?;
}}
```
");
}

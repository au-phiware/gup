// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Example demonstrating GPU resource dependency graph visualization.
//!
//! This example shows how to use the ResourceGraph to track and visualize
//! relationships between GPU resources like buffers, pipelines, and textures.

use gup::debug::{DebugResourceType, ResourceGraph};
use gup::error::GupResult;

fn main() -> GupResult<()> {
    println!("=== GPU Resource Dependency Graph Example ===\n");

    // Create a resource graph
    let mut graph = ResourceGraph::new();

    // Simulate a typical rendering setup

    // 1. Create vertex buffers
    let vertex_buffer = graph.add_resource(
        DebugResourceType::Buffer,
        Some("Vertex Buffer".to_string()),
        1024 * 1024 * 2, // 2 MB
        Some("VERTEX | COPY_DST".to_string()),
        vec![],
    );

    let index_buffer = graph.add_resource(
        DebugResourceType::Buffer,
        Some("Index Buffer".to_string()),
        512 * 1024, // 512 KB
        Some("INDEX | COPY_DST".to_string()),
        vec![],
    );

    // 2. Create uniform buffers
    let camera_uniform = graph.add_resource(
        DebugResourceType::Buffer,
        Some("Camera Uniform".to_string()),
        256, // 256 bytes
        Some("UNIFORM | COPY_DST".to_string()),
        vec![],
    );

    let model_uniform = graph.add_resource(
        DebugResourceType::Buffer,
        Some("Model Matrix Uniform".to_string()),
        256,
        Some("UNIFORM | COPY_DST".to_string()),
        vec![],
    );

    // 3. Create textures
    let diffuse_texture = graph.add_resource(
        DebugResourceType::Texture,
        Some("Diffuse Texture".to_string()),
        4 * 1024 * 1024, // 4 MB
        Some("SAMPLED | COPY_DST".to_string()),
        vec![],
    );

    let normal_map = graph.add_resource(
        DebugResourceType::Texture,
        Some("Normal Map".to_string()),
        4 * 1024 * 1024,
        Some("SAMPLED | COPY_DST".to_string()),
        vec![],
    );

    // 4. Create sampler
    let sampler = graph.add_resource(
        DebugResourceType::Sampler,
        Some("Texture Sampler".to_string()),
        64, // Small metadata
        None,
        vec![],
    );

    // 5. Create bind group (depends on uniforms, textures, sampler)
    let bind_group = graph.add_resource(
        DebugResourceType::BindGroup,
        Some("Material Bind Group".to_string()),
        128,
        None,
        vec![
            camera_uniform,
            model_uniform,
            diffuse_texture,
            normal_map,
            sampler,
        ],
    );

    // 6. Create render pipeline (depends on bind group)
    let render_pipeline = graph.add_resource(
        DebugResourceType::Pipeline,
        Some("Forward Render Pipeline".to_string()),
        1024, // Pipeline metadata
        None,
        vec![bind_group, vertex_buffer, index_buffer],
    );

    // 7. Create an unused shadow map (for demonstration)
    let shadow_map = graph.add_resource(
        DebugResourceType::Texture,
        Some("Shadow Map".to_string()),
        2 * 1024 * 1024,
        Some("SAMPLED | RENDER_ATTACHMENT".to_string()),
        vec![],
    );

    // Mark shadow map as inactive (not used in current render pass)
    graph.mark_inactive(shadow_map)?;

    // Display basic information
    println!("Resource Graph Statistics:");
    println!("  Total resources: {}", graph.resources().count());
    println!();

    // Generate and display analysis report
    let report = graph.generate_report();
    println!("{}", report.to_text());

    // Show dependency footprint for the render pipeline
    let footprint = graph.calculate_dependency_footprint(render_pipeline);
    println!(
        "\n📊 Total memory footprint for render pipeline: {:.2} MB",
        footprint as f64 / (1024.0 * 1024.0)
    );

    // Display resource tree
    println!("\n📋 Resource Dependency Tree:");
    println!("{}", graph.to_tree_text(None));

    // Check for circular dependencies
    let cycles = graph.detect_circular_dependencies();
    if !cycles.is_empty() {
        println!("⚠️  Circular dependencies detected:");
        for (i, cycle) in cycles.iter().enumerate() {
            println!("  Cycle {}: {:?}", i + 1, cycle);
        }
    } else {
        println!("✓ No circular dependencies detected");
    }

    // Find sharing opportunities
    println!("\n💡 Resource Sharing Opportunities:");
    let opportunities = graph.find_sharing_opportunities();
    if opportunities.is_empty() {
        println!("  No sharing opportunities found");
    } else {
        for (id, count) in opportunities.iter().take(5) {
            if let Some(node) = graph.get_resource(*id) {
                let default_label = format!("{}", node.resource_type);
                let label = node.label.as_deref().unwrap_or(&default_label);
                println!("  {} ({}) shared by {} resources", label, id, count);
            }
        }
    }

    // Find unused resources
    let unused = graph.find_unused_resources();
    if !unused.is_empty() {
        println!("\n♻️  Unused Resources (can be freed):");
        for id in unused {
            if let Some(node) = graph.get_resource(id) {
                let default_label = format!("{}", node.resource_type);
                let label = node.label.as_deref().unwrap_or(&default_label);
                let size_mb = node.size as f64 / (1024.0 * 1024.0);
                println!("  {} ({}) - {:.2} MB", label, id, size_mb);
            }
        }
    }

    // Export to DOT format
    println!("\n📄 Exporting graph to DOT format...");
    let dot = graph.to_dot();
    std::fs::write("resource_graph.dot", &dot)?;
    println!("  ✓ Written to resource_graph.dot");
    println!("  Run 'dot -Tsvg resource_graph.dot -o resource_graph.svg' to visualize");

    // Export to JSON format
    println!("\n📄 Exporting graph to JSON format...");
    let json = graph.to_json()?;
    std::fs::write("resource_graph.json", &json)?;
    println!("  ✓ Written to resource_graph.json");

    println!("\n=== Example Complete ===");

    Ok(())
}

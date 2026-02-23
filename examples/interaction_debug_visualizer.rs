// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Example demonstrating the GPU Interaction Debug Visualizer.
//!
//! This example shows how to use the InteractionDebugVisualizer to debug
//! GPU hit testing by visualizing elements, queries, and results.

use gup::RenderContext;
use gup::debug::InteractionDebugVisualizer;
use gup::interaction::{ElementData, GpuInteractionQuery, InteractionResult};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("🔍 GPU Interaction Debug Visualizer Demo");
    println!("==========================================\n");

    // Create render context
    let context = Arc::new(RenderContext::new().await?);
    let mut visualizer = InteractionDebugVisualizer::new(context.clone());

    // Scenario 1: Simple point query with hits and misses
    println!("📍 Scenario 1: Point Query Hit Testing");
    println!("--------------------------------------");

    let elements = vec![
        ElementData {
            position: [100.0, 100.0],
            size: [15.0, 15.0],
            mark_type: 0, // Circle
            element_id: 0,
            selection_id: 0,
            _padding: 0,
        },
        ElementData {
            position: [200.0, 150.0],
            size: [20.0, 30.0],
            mark_type: 1, // Rectangle
            element_id: 0,
            selection_id: 0,
            _padding: 0,
        },
        ElementData {
            position: [300.0, 200.0],
            size: [10.0, 10.0],
            mark_type: 0, // Circle
            element_id: 0,
            selection_id: 1,
            _padding: 0,
        },
        ElementData {
            position: [150.0, 250.0],
            size: [25.0, 5.0],
            mark_type: 2, // Line
            element_id: 0,
            selection_id: 1,
            _padding: 0,
        },
    ];

    let queries = vec![
        GpuInteractionQuery {
            position: [105.0, 105.0], // Near first circle
            region_size: [10.0, 0.0],
            query_type: 0,
            max_results: 10,
            _padding: [0, 0],
        },
        GpuInteractionQuery {
            position: [200.0, 150.0], // On rectangle
            region_size: [10.0, 0.0],
            query_type: 0,
            max_results: 10,
            _padding: [0, 0],
        },
        GpuInteractionQuery {
            position: [500.0, 500.0], // Far from all elements
            region_size: [10.0, 0.0],
            query_type: 0,
            max_results: 10,
            _padding: [0, 0],
        },
    ];

    let results = vec![
        InteractionResult {
            element_id: 0,
            selection_id: 0,
            distance: 7.07,
            is_hit: 1,
            intersection_point: [105.0, 105.0],
            _padding: [0, 0],
        },
        InteractionResult {
            element_id: 1,
            selection_id: 0,
            distance: 0.0,
            is_hit: 1,
            intersection_point: [200.0, 150.0],
            _padding: [0, 0],
        },
        InteractionResult {
            element_id: 2,
            selection_id: 1,
            distance: 300.0,
            is_hit: 0,
            intersection_point: [0.0, 0.0],
            _padding: [0, 0],
        },
        InteractionResult {
            element_id: 3,
            selection_id: 1,
            distance: 400.0,
            is_hit: 0,
            intersection_point: [0.0, 0.0],
            _padding: [0, 0],
        },
    ];

    visualizer.update(&elements, &queries, &results);

    // Display ASCII visualization
    let ascii = visualizer.render_ascii(60, 30)?;
    println!("{}", ascii);

    // Export detailed JSON
    let json_path = "/tmp/interaction_debug_scenario1.json";
    visualizer.export_json(json_path)?;
    println!("\n✅ Exported detailed data to: {}", json_path);

    // Show buffer inspection data
    let inspection = visualizer.inspect_buffers()?;
    println!("\n📊 Buffer Inspection:");
    println!(
        "  Element buffer: {} bytes ({} elements)",
        inspection.element_buffer_size_bytes,
        inspection.element_buffer.len()
    );
    println!(
        "  Query buffer:   {} bytes ({} queries)",
        inspection.query_buffer_size_bytes,
        inspection.query_buffer.len()
    );
    println!(
        "  Result buffer:  {} bytes ({} results)",
        inspection.result_buffer_size_bytes,
        inspection.result_buffer.len()
    );

    // Scenario 2: Dense element field
    println!("\n\n📍 Scenario 2: Dense Element Field (100 elements)");
    println!("--------------------------------------------------");

    let mut dense_elements = Vec::new();
    let mut dense_results = Vec::new();

    // Create 10x10 grid of circles
    for y in 0..10 {
        for x in 0..10 {
            let element_id = (y * 10 + x) as u32;
            dense_elements.push(ElementData {
                position: [50.0 + x as f32 * 40.0, 50.0 + y as f32 * 40.0],
                size: [8.0, 8.0],
                mark_type: 0, // Circle
                element_id: 0,
                selection_id: 0,
                _padding: 0,
            });

            // Simulate hit test results - hits in a diagonal line
            let is_hit = if x == y { 1 } else { 0 };
            let distance = if x == y { 0.0 } else { 50.0 };

            dense_results.push(InteractionResult {
                element_id,
                selection_id: 0,
                distance,
                is_hit,
                intersection_point: if x == y {
                    [50.0 + x as f32 * 40.0, 50.0 + y as f32 * 40.0]
                } else {
                    [0.0, 0.0]
                },
                _padding: [0, 0],
            });
        }
    }

    // Single query in the center
    let dense_queries = vec![GpuInteractionQuery {
        position: [200.0, 200.0],
        region_size: [50.0, 0.0],
        query_type: 0,
        max_results: 100,
        _padding: [0, 0],
    }];

    visualizer.clear();
    visualizer.update(&dense_elements, &dense_queries, &dense_results);

    let dense_ascii = visualizer.render_ascii(60, 30)?;
    println!("{}", dense_ascii);

    let json_path2 = "/tmp/interaction_debug_scenario2.json";
    visualizer.export_json(json_path2)?;
    println!("\n✅ Exported dense field data to: {}", json_path2);

    // Scenario 3: Multiple selection IDs
    println!("\n\n📍 Scenario 3: Multiple Selections");
    println!("-----------------------------------");

    let multi_elements = vec![
        ElementData {
            position: [100.0, 100.0],
            size: [20.0, 20.0],
            mark_type: 0,
            element_id: 0,
            selection_id: 0, // Selection A
            _padding: 0,
        },
        ElementData {
            position: [150.0, 100.0],
            size: [20.0, 20.0],
            mark_type: 0,
            element_id: 0,
            selection_id: 0, // Selection A
            _padding: 0,
        },
        ElementData {
            position: [100.0, 200.0],
            size: [30.0, 30.0],
            mark_type: 1,
            element_id: 0,
            selection_id: 1, // Selection B
            _padding: 0,
        },
        ElementData {
            position: [200.0, 200.0],
            size: [30.0, 30.0],
            mark_type: 1,
            element_id: 0,
            selection_id: 1, // Selection B
            _padding: 0,
        },
    ];

    let multi_queries = vec![GpuInteractionQuery {
        position: [125.0, 100.0],
        region_size: [30.0, 0.0],
        query_type: 0,
        max_results: 10,
        _padding: [0, 0],
    }];

    let multi_results = vec![
        InteractionResult {
            element_id: 0,
            selection_id: 0,
            distance: 25.0,
            is_hit: 1,
            intersection_point: [120.0, 100.0],
            _padding: [0, 0],
        },
        InteractionResult {
            element_id: 1,
            selection_id: 0,
            distance: 25.0,
            is_hit: 1,
            intersection_point: [130.0, 100.0],
            _padding: [0, 0],
        },
        InteractionResult {
            element_id: 2,
            selection_id: 1,
            distance: 100.0,
            is_hit: 0,
            intersection_point: [0.0, 0.0],
            _padding: [0, 0],
        },
        InteractionResult {
            element_id: 3,
            selection_id: 1,
            distance: 120.0,
            is_hit: 0,
            intersection_point: [0.0, 0.0],
            _padding: [0, 0],
        },
    ];

    visualizer.clear();
    visualizer.update(&multi_elements, &multi_queries, &multi_results);

    let multi_ascii = visualizer.render_ascii(60, 30)?;
    println!("{}", multi_ascii);

    let json_path3 = "/tmp/interaction_debug_scenario3.json";
    visualizer.export_json(json_path3)?;
    println!("\n✅ Exported multi-selection data to: {}", json_path3);

    println!("\n\n🎉 Interaction Debug Visualizer Demo Complete!");
    println!("\nJSON exports contain full details for further analysis:");
    println!("  - {}", json_path);
    println!("  - {}", json_path2);
    println!("  - {}", json_path3);

    Ok(())
}

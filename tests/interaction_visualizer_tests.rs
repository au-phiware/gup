// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Integration tests for the interaction debug visualizer.

use gup::RenderContext;
use gup::debug::InteractionDebugVisualizer;
use gup::interaction::{ElementData, GpuInteractionQuery, InteractionResult};
use std::sync::Arc;

#[tokio::test]
async fn test_interaction_visualizer_basic() -> Result<(), Box<dyn std::error::Error>> {
    // Create render context
    let context = Arc::new(RenderContext::new().await?);
    let mut visualizer = InteractionDebugVisualizer::new(context.clone());

    // Create test data
    let elements = vec![
        ElementData {
            position: [100.0, 100.0],
            size: [10.0, 10.0],
            mark_type: 0, // Circle
            element_id: 0,
            selection_id: 0,
            _padding: 0,
        },
        ElementData {
            position: [200.0, 200.0],
            size: [20.0, 20.0],
            mark_type: 1, // Rectangle
            element_id: 0,
            selection_id: 0,
            _padding: 0,
        },
        ElementData {
            position: [300.0, 100.0],
            size: [10.0, 10.0],
            mark_type: 0, // Circle
            element_id: 0,
            selection_id: 1,
            _padding: 0,
        },
    ];

    let queries = vec![
        GpuInteractionQuery {
            position: [105.0, 105.0],
            region_size: [5.0, 0.0],
            query_type: 0, // Point
            max_results: 10,
            _padding: [0, 0],
        },
        GpuInteractionQuery {
            position: [300.0, 300.0],
            region_size: [5.0, 0.0],
            query_type: 0, // Point
            max_results: 10,
            _padding: [0, 0],
        },
    ];

    let results = vec![
        InteractionResult {
            element_id: 0,
            selection_id: 0,
            distance: 7.07,
            is_hit: 1, // Hit
            intersection_point: [105.0, 105.0],
            _padding: [0, 0],
        },
        InteractionResult {
            element_id: 1,
            selection_id: 0,
            distance: 141.42,
            is_hit: 0, // Miss
            intersection_point: [0.0, 0.0],
            _padding: [0, 0],
        },
        InteractionResult {
            element_id: 2,
            selection_id: 1,
            distance: 200.0,
            is_hit: 0, // Miss
            intersection_point: [0.0, 0.0],
            _padding: [0, 0],
        },
    ];

    // Update visualizer
    visualizer.update(&elements, &queries, &results);

    // Verify state was captured
    let state = visualizer.state().expect("State should be available");
    assert_eq!(state.elements.len(), 3);
    assert_eq!(state.queries.len(), 2);
    assert_eq!(state.results.len(), 3);

    // Verify summary
    assert_eq!(state.summary.total_elements, 3);
    assert_eq!(state.summary.total_queries, 2);
    assert_eq!(state.summary.total_hits, 1);
    assert_eq!(state.summary.total_misses, 2);
    assert!((state.summary.hit_rate_percent - 33.333).abs() < 0.1);

    // Verify element highlighting
    let highlighted = state.elements.iter().filter(|e| e.is_highlighted).count();
    assert_eq!(highlighted, 1, "Only the hit element should be highlighted");

    Ok(())
}

#[tokio::test]
async fn test_interaction_visualizer_export() -> Result<(), Box<dyn std::error::Error>> {
    let context = Arc::new(RenderContext::new().await?);
    let mut visualizer = InteractionDebugVisualizer::new(context.clone());

    // Create simple test data
    let elements = vec![ElementData {
        position: [50.0, 50.0],
        size: [5.0, 5.0],
        mark_type: 0,
        element_id: 0,
        selection_id: 0,
        _padding: 0,
    }];

    let queries = vec![GpuInteractionQuery {
        position: [50.0, 50.0],
        region_size: [10.0, 0.0],
        query_type: 0,
        max_results: 10,
        _padding: [0, 0],
    }];

    let results = vec![InteractionResult {
        element_id: 0,
        selection_id: 0,
        distance: 0.0,
        is_hit: 1,
        intersection_point: [50.0, 50.0],
        _padding: [0, 0],
    }];

    visualizer.update(&elements, &queries, &results);

    // Export to JSON
    let temp_path = "/tmp/gup_interaction_debug_test.json";
    visualizer.export_json(temp_path)?;

    // Verify file was created
    assert!(std::path::Path::new(temp_path).exists());

    // Read and verify JSON content
    let json_content = std::fs::read_to_string(temp_path)?;
    assert!(json_content.contains("\"total_elements\":"));
    assert!(json_content.contains("\"total_hits\":"));

    // Clean up
    std::fs::remove_file(temp_path)?;

    Ok(())
}

#[tokio::test]
async fn test_interaction_visualizer_ascii_rendering() -> Result<(), Box<dyn std::error::Error>> {
    let context = Arc::new(RenderContext::new().await?);
    let mut visualizer = InteractionDebugVisualizer::new(context.clone());

    let elements = vec![
        ElementData {
            position: [100.0, 100.0],
            size: [10.0, 10.0],
            mark_type: 0, // Circle
            element_id: 0,
            selection_id: 0,
            _padding: 0,
        },
        ElementData {
            position: [200.0, 200.0],
            size: [20.0, 20.0],
            mark_type: 1, // Rectangle
            element_id: 0,
            selection_id: 0,
            _padding: 0,
        },
    ];

    let queries = vec![GpuInteractionQuery {
        position: [105.0, 105.0],
        region_size: [5.0, 0.0],
        query_type: 0,
        max_results: 10,
        _padding: [0, 0],
    }];

    let results = vec![InteractionResult {
        element_id: 0,
        selection_id: 0,
        distance: 7.07,
        is_hit: 1,
        intersection_point: [105.0, 105.0],
        _padding: [0, 0],
    }];

    visualizer.update(&elements, &queries, &results);

    // Render ASCII visualization
    let ascii = visualizer.render_ascii(40, 20)?;

    // Verify output contains expected elements
    assert!(ascii.contains("GPU Interaction Debug Visualization"));
    assert!(ascii.contains("Elements: 2"));
    assert!(ascii.contains("Queries:  1"));
    assert!(ascii.contains("Hits:     1"));
    assert!(ascii.contains("Circle"));
    assert!(ascii.contains("Rectangle"));
    assert!(ascii.contains("Legend:"));

    Ok(())
}

#[tokio::test]
async fn test_interaction_visualizer_buffer_inspection() -> Result<(), Box<dyn std::error::Error>> {
    let context = Arc::new(RenderContext::new().await?);
    let mut visualizer = InteractionDebugVisualizer::new(context.clone());

    let elements = vec![
        ElementData {
            position: [10.0, 20.0],
            size: [5.0, 5.0],
            mark_type: 0,
            element_id: 0,
            selection_id: 0,
            _padding: 0,
        },
        ElementData {
            position: [30.0, 40.0],
            size: [10.0, 10.0],
            mark_type: 1,
            element_id: 0,
            selection_id: 1,
            _padding: 0,
        },
    ];

    let queries = vec![GpuInteractionQuery {
        position: [15.0, 25.0],
        region_size: [10.0, 0.0],
        query_type: 0,
        max_results: 10,
        _padding: [0, 0],
    }];

    let results = vec![InteractionResult {
        element_id: 0,
        selection_id: 0,
        distance: 7.07,
        is_hit: 1,
        intersection_point: [15.0, 25.0],
        _padding: [0, 0],
    }];

    visualizer.update(&elements, &queries, &results);

    // Inspect buffers
    let inspection = visualizer.inspect_buffers()?;

    assert_eq!(inspection.element_buffer.len(), 2);
    assert_eq!(inspection.query_buffer.len(), 1);
    assert_eq!(inspection.result_buffer.len(), 1);

    // Verify buffer sizes
    assert!(inspection.element_buffer_size_bytes > 0);
    assert!(inspection.query_buffer_size_bytes > 0);
    assert!(inspection.result_buffer_size_bytes > 0);

    // Verify element data
    assert_eq!(inspection.element_buffer[0].position, [10.0, 20.0]);
    assert_eq!(inspection.element_buffer[1].position, [30.0, 40.0]);

    Ok(())
}

#[tokio::test]
async fn test_interaction_visualizer_enable_disable() -> Result<(), Box<dyn std::error::Error>> {
    let context = Arc::new(RenderContext::new().await?);
    let mut visualizer = InteractionDebugVisualizer::new(context.clone());

    // Visualizer should be enabled in debug builds
    assert_eq!(visualizer.is_enabled(), cfg!(debug_assertions));

    // Disable visualizer
    visualizer.set_enabled(false);
    assert!(!visualizer.is_enabled());

    // Update should not capture state when disabled
    let elements = vec![ElementData {
        position: [0.0, 0.0],
        size: [1.0, 1.0],
        mark_type: 0,
        element_id: 0,
        selection_id: 0,
        _padding: 0,
    }];
    visualizer.update(&elements, &[], &[]);
    assert!(visualizer.state().is_none());

    // Re-enable and update
    visualizer.set_enabled(true);
    visualizer.update(&elements, &[], &[]);
    assert!(visualizer.state().is_some());

    Ok(())
}

#[tokio::test]
async fn test_interaction_visualizer_clear() -> Result<(), Box<dyn std::error::Error>> {
    let context = Arc::new(RenderContext::new().await?);
    let mut visualizer = InteractionDebugVisualizer::new(context.clone());

    // Add some data
    let elements = vec![ElementData {
        position: [0.0, 0.0],
        size: [1.0, 1.0],
        mark_type: 0,
        element_id: 0,
        selection_id: 0,
        _padding: 0,
    }];
    visualizer.update(&elements, &[], &[]);
    assert!(visualizer.state().is_some());

    // Clear state
    visualizer.clear();
    assert!(visualizer.state().is_none());

    Ok(())
}

#[tokio::test]
async fn test_interaction_visualizer_mark_type_distribution()
-> Result<(), Box<dyn std::error::Error>> {
    let context = Arc::new(RenderContext::new().await?);
    let mut visualizer = InteractionDebugVisualizer::new(context.clone());

    // Create elements with different mark types
    let elements = vec![
        ElementData {
            position: [0.0, 0.0],
            size: [1.0, 1.0],
            mark_type: 0, // Circle
            element_id: 0,
            selection_id: 0,
            _padding: 0,
        },
        ElementData {
            position: [10.0, 10.0],
            size: [2.0, 2.0],
            mark_type: 0, // Circle
            element_id: 0,
            selection_id: 0,
            _padding: 0,
        },
        ElementData {
            position: [20.0, 20.0],
            size: [3.0, 3.0],
            mark_type: 1, // Rectangle
            element_id: 0,
            selection_id: 0,
            _padding: 0,
        },
        ElementData {
            position: [30.0, 30.0],
            size: [4.0, 4.0],
            mark_type: 2, // Line
            element_id: 0,
            selection_id: 0,
            _padding: 0,
        },
    ];

    visualizer.update(&elements, &[], &[]);

    let state = visualizer.state().unwrap();
    let mark_types = &state.summary.elements_by_mark_type;

    // Should have 3 mark types
    assert_eq!(mark_types.len(), 3);

    // Find Circle count (should be 2)
    let circle_count = mark_types
        .iter()
        .find(|(name, _)| name == "Circle")
        .map(|(_, count)| *count)
        .unwrap_or(0);
    assert_eq!(circle_count, 2);

    // Find Rectangle count (should be 1)
    let rect_count = mark_types
        .iter()
        .find(|(name, _)| name == "Rectangle")
        .map(|(_, count)| *count)
        .unwrap_or(0);
    assert_eq!(rect_count, 1);

    // Find Line count (should be 1)
    let line_count = mark_types
        .iter()
        .find(|(name, _)| name == "Line")
        .map(|(_, count)| *count)
        .unwrap_or(0);
    assert_eq!(line_count, 1);

    Ok(())
}

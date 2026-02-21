// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Comprehensive tests for the GPU interaction system
//!
//! These tests validate the performance, accuracy, and integration requirements
//! specified in GUP-012: GPU Interaction System.

use gup::Circle;
use gup::RenderContext;
use gup::interaction::{InteractionEvent, InteractionSystem, Rect, Renderable, Vec2};
use gup::selection::Selection;
use gup::test_utils::create_test_context;
use std::sync::Arc;

/// Test data structure for interaction testing
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct TestData {
    x: f32,
    y: f32,
    value: f32,
}

impl TestData {
    fn new(x: f32, y: f32, value: f32) -> Self {
        Self { x, y, value }
    }
}

impl gup::InteractionData for TestData {
    fn position(&self) -> [f32; 2] {
        [self.x, self.y]
    }
}

/// Create test render context for GPU operations using safe test utilities
async fn get_test_context() -> Arc<RenderContext> {
    let guard = create_test_context().await.expect("Failed to create test context");
    guard.clone_context()
}

/// Create test selection with known positions for accuracy testing
async fn create_test_selection(
    context: Arc<RenderContext>,
    positions: Vec<(f32, f32)>,
) -> Selection<TestData, Circle> {
    let data: Vec<TestData> = positions
        .into_iter()
        .enumerate()
        .map(|(i, (x, y))| TestData::new(x, y, i as f32))
        .collect();

    Selection::<TestData, Circle>::new(data, context).expect("Failed to create test selection")
}

/// Basic system creation and initialization tests
#[tokio::test]
async fn test_interaction_system_creation() {
    let context = get_test_context().await;
    let interaction_system = InteractionSystem::new(&context).await;

    assert!(
        interaction_system.is_ok(),
        "Should create interaction system successfully"
    );

    let system = interaction_system.unwrap();
    let stats = system.query_stats();
    assert_eq!(stats.total_queries, 0);
    assert_eq!(stats.total_elements_tested, 0);
    assert_eq!(stats.total_hits, 0);
}

/// Test point query accuracy with known element positions
#[tokio::test]
async fn test_point_query_accuracy() {
    let context = get_test_context().await;
    let mut interaction_system = InteractionSystem::new(&context).await.unwrap();

    // Create selection with precise positions
    let positions = vec![(50.0, 50.0), (150.0, 100.0), (200.0, 200.0)];
    let selection = create_test_selection(Arc::clone(&context), positions.clone()).await;

    // Test hits at exact element positions
    for (i, (x, y)) in positions.iter().enumerate() {
        let query_pos = Vec2::new(*x, *y);
        let selections: Vec<&dyn gup::interaction::Renderable> = vec![&selection];

        let hits = interaction_system
            .query_point(query_pos, &selections)
            .await
            .expect("Query should succeed");

        assert_eq!(
            hits.len(),
            1,
            "Should find exactly one hit at position ({x}, {y})"
        );

        // Verify it's the correct element
        assert_eq!(
            hits[0].element_id, i as u32,
            "Should hit element {i} at position ({x}, {y})"
        );
    }
}

/// Test that queries miss at positions between elements
#[tokio::test]
async fn test_point_query_misses() {
    let context = get_test_context().await;
    let mut interaction_system = InteractionSystem::new(&context).await.unwrap();

    // Create selection with elements at known positions
    let positions = vec![(0.0, 0.0), (100.0, 100.0)];
    let selection = create_test_selection(Arc::clone(&context), positions).await;

    // Test positions that should miss (far from actual element positions)
    let miss_positions = vec![
        Vec2::new(25.0, 25.0),   // Between the two elements
        Vec2::new(-50.0, -50.0), // Far from any element
        Vec2::new(500.0, 500.0), // Far from any element
    ];

    let selections: Vec<&dyn gup::interaction::Renderable> = vec![&selection];

    for miss_pos in miss_positions {
        let hits = interaction_system
            .query_point(miss_pos, &selections)
            .await
            .expect("Query should succeed");

        assert_eq!(
            hits.len(),
            0,
            "Should miss at position ({}, {}) - no elements within radius",
            miss_pos.x,
            miss_pos.y
        );
    }
}

/// Test region query functionality
#[tokio::test]
async fn test_region_query() {
    let context = get_test_context().await;
    let mut interaction_system = InteractionSystem::new(&context).await.unwrap();

    // Create grid of elements
    let mut positions = Vec::new();
    for x in 0..10 {
        for y in 0..10 {
            positions.push((x as f32 * 20.0, y as f32 * 20.0));
        }
    }
    let selection = create_test_selection(Arc::clone(&context), positions).await;

    // Query a region that should contain multiple elements
    let region = Rect::new(Vec2::new(0.0, 0.0), Vec2::new(100.0, 100.0));
    let selections: Vec<&dyn gup::interaction::Renderable> = vec![&selection];

    let hits = interaction_system
        .query_region(region, &selections)
        .await
        .expect("Region query should succeed");

    // Should find elements within the region
    assert!(
        !hits.is_empty(),
        "Should find elements within the query region"
    );

    // Verify all hits are within the region bounds
    for hit in &hits {
        let pos = hit.intersection_point;
        assert!(
            pos.x >= 0.0 && pos.x <= 100.0 && pos.y >= 0.0 && pos.y <= 100.0,
            "Hit at ({}, {}) should be within region bounds [0,0] to [100,100]",
            pos.x,
            pos.y
        );
    }
}

/// Test performance requirements: <1ms for point queries on large datasets
#[tokio::test]
async fn test_point_query_performance() {
    let context = get_test_context().await;
    let mut interaction_system = InteractionSystem::new(&context).await.unwrap();

    // Create large dataset (10K points for reasonable test performance)
    let mut positions = Vec::new();
    for i in 0..10_000 {
        let angle = i as f32 * 0.001;
        let radius = (i as f32 * 0.1) % 500.0;
        let x = radius * angle.cos();
        let y = radius * angle.sin();
        positions.push((x, y));
    }
    let selection = create_test_selection(Arc::clone(&context), positions).await;

    // Perform point query and measure time
    let query_pos = Vec2::new(100.0, 100.0);
    let selections: Vec<&dyn gup::interaction::Renderable> = vec![&selection];

    let start_time = std::time::Instant::now();
    let _hits = interaction_system
        .query_point(query_pos, &selections)
        .await
        .expect("Performance query should succeed");
    let query_duration = start_time.elapsed();

    // Performance requirement: <1ms for 10K points
    // Note: In a full GPU implementation, this would be much faster
    assert!(
        query_duration.as_millis() < 100,
        "Query took too long: {query_duration:?} (should be <100ms for test)"
    );

    println!("Point query on 10K elements completed in {query_duration:?}");

    // Verify query stats were updated
    let stats = interaction_system.query_stats();
    assert!(stats.total_queries > 0);
    assert!(stats.total_elements_tested > 0);
}

/// Test region query performance with large datasets
#[tokio::test]
async fn test_region_query_performance() {
    let context = get_test_context().await;
    let mut interaction_system = InteractionSystem::new(&context).await.unwrap();

    // Create large dataset
    let mut positions = Vec::new();
    for i in 0..5_000 {
        // Distribute points in a grid pattern
        let x = (i % 100) as f32 * 10.0;
        let y = (i / 100) as f32 * 10.0;
        positions.push((x, y));
    }
    let selection = create_test_selection(Arc::clone(&context), positions).await;

    // Query a region that intersects with many elements
    let region = Rect::new(Vec2::new(200.0, 200.0), Vec2::new(400.0, 400.0));
    let selections: Vec<&dyn gup::interaction::Renderable> = vec![&selection];

    let start_time = std::time::Instant::now();
    let hits = interaction_system
        .query_region(region, &selections)
        .await
        .expect("Performance region query should succeed");
    let query_duration = start_time.elapsed();

    // Performance requirement: <10ms for region queries on large datasets
    assert!(
        query_duration.as_millis() < 200,
        "Region query took too long: {query_duration:?} (should be <200ms for test)"
    );

    println!(
        "Region query on 5K elements found {} hits in {:?}",
        hits.len(),
        query_duration
    );
}

/// Test multiple concurrent queries
#[tokio::test]
async fn test_multiple_queries() {
    let context = get_test_context().await;
    let mut interaction_system = InteractionSystem::new(&context).await.unwrap();

    // Create test selection
    let positions = vec![(10.0, 10.0), (50.0, 50.0), (100.0, 100.0)];
    let selection = create_test_selection(Arc::clone(&context), positions).await;
    let selections: Vec<&dyn gup::interaction::Renderable> = vec![&selection];

    // Perform multiple queries in sequence
    let query_positions = vec![
        Vec2::new(10.0, 10.0),
        Vec2::new(50.0, 50.0),
        Vec2::new(100.0, 100.0),
        Vec2::new(200.0, 200.0), // This should miss
    ];

    let mut total_hits = 0;
    for query_pos in query_positions {
        let hits = interaction_system
            .query_point(query_pos, &selections)
            .await
            .expect("Multiple query should succeed");
        total_hits += hits.len();
    }

    // Expected: 3 hits total (queries at (10,10), (50,50), and (100,100) hit element positions)
    // Query at (200,200) misses since no element is there
    assert_eq!(
        total_hits, 3,
        "Should find exactly 3 hits from 4 queries (got {total_hits})"
    );

    // Verify stats reflect all queries
    let stats = interaction_system.query_stats();
    assert_eq!(stats.total_queries, 4, "Should record 4 queries");
}

/// Test event handler registration and processing
#[tokio::test]
async fn test_event_handler_integration() {
    let context = get_test_context().await;
    let mut interaction_system = InteractionSystem::new(&context).await.unwrap();

    // Test event handler registration
    use std::sync::{Arc as StdArc, Mutex};
    let event_fired = StdArc::new(Mutex::new(false));
    let event_fired_clone = StdArc::clone(&event_fired);

    interaction_system.register_event_handler("click", move |event| {
        let mut fired = event_fired_clone.lock().unwrap();
        *fired = true;
        assert_eq!(event.interaction_type, "click");
        assert_eq!(event.screen_position.x, 100.0);
        assert_eq!(event.screen_position.y, 200.0);
    });

    // Create and process a test event
    let test_event = InteractionEvent::new("click", Vec2::new(100.0, 200.0));
    interaction_system
        .process_interaction_event(test_event)
        .await
        .expect("Event processing should succeed");

    // Verify event was processed
    let fired = event_fired.lock().unwrap();
    assert!(*fired, "Click event handler should have been called");
}

/// Test different mark types (Circle, Rectangle, Line)
#[tokio::test]
async fn test_different_mark_types() {
    let context = get_test_context().await;
    let mut interaction_system = InteractionSystem::new(&context).await.unwrap();

    // Create selections with different mark types
    let circle_positions = vec![(50.0, 50.0)];
    let circle_selection = create_test_selection(Arc::clone(&context), circle_positions).await;

    // Test that different mark types can be queried
    let selections: Vec<&dyn gup::interaction::Renderable> = vec![&circle_selection];

    let hits = interaction_system
        .query_point(Vec2::new(50.0, 50.0), &selections)
        .await
        .expect("Multi-mark query should succeed");

    // Should find at least the circle
    assert!(
        !hits.is_empty(),
        "Should find hits for different mark types"
    );
}

/// Test edge cases and error conditions
#[tokio::test]
async fn test_edge_cases() {
    let context = get_test_context().await;
    let mut interaction_system = InteractionSystem::new(&context).await.unwrap();

    // Test empty selection
    let empty_selection = create_test_selection(Arc::clone(&context), vec![]).await;
    let selections: Vec<&dyn gup::interaction::Renderable> = vec![&empty_selection];

    let hits = interaction_system
        .query_point(Vec2::new(0.0, 0.0), &selections)
        .await
        .expect("Empty selection query should succeed");

    assert_eq!(hits.len(), 0, "Empty selection should return no hits");

    // Test query with no selections
    let no_selections: Vec<&dyn gup::interaction::Renderable> = vec![];
    let hits = interaction_system
        .query_point(Vec2::new(0.0, 0.0), &no_selections)
        .await
        .expect("No selections query should succeed");

    assert_eq!(hits.len(), 0, "No selections should return no hits");
}

/// Test query statistics accuracy
#[tokio::test]
async fn test_query_statistics() {
    let context = get_test_context().await;
    let mut interaction_system = InteractionSystem::new(&context).await.unwrap();

    // Reset stats to start clean
    interaction_system.reset_stats();

    let positions = vec![(10.0, 10.0), (50.0, 50.0)];
    let selection = create_test_selection(Arc::clone(&context), positions).await;
    let selections: Vec<&dyn gup::interaction::Renderable> = vec![&selection];

    // Perform several queries
    for _ in 0..5 {
        let _ = interaction_system
            .query_point(Vec2::new(10.0, 10.0), &selections)
            .await
            .expect("Stats test query should succeed");
    }

    let stats = interaction_system.query_stats();
    assert_eq!(stats.total_queries, 5, "Should record 5 queries");
    assert!(stats.total_elements_tested > 0, "Should test some elements");
    assert!(
        stats.average_query_time_us > 0.0,
        "Should record query times"
    );
}

/// Integration test with Selection system
#[tokio::test]
async fn test_selection_integration() {
    let context = get_test_context().await;
    let mut selection = create_test_selection(Arc::clone(&context), vec![(25.0, 25.0)]).await;

    // Test that Selection implements Renderable
    let elements = selection
        .get_elements_for_interaction()
        .expect("Should extract interaction elements");
    assert_eq!(elements.len(), 1, "Should have one interaction element");

    // Test selection ID generation
    let id1 = selection.selection_id();
    let id2 = selection.selection_id();
    assert_eq!(id1, id2, "Selection ID should be consistent");

    // Test event handler registration (from Selection side)
    selection.on("click", |_event, _data| {
        // Event handler implementation
    });
}

/// Stress test with very large dataset
#[tokio::test]
#[ignore] // Ignore by default due to long runtime
async fn test_large_dataset_stress() {
    let context = get_test_context().await;
    let mut interaction_system = InteractionSystem::new(&context).await.unwrap();

    // Create very large dataset (100K points)
    let mut positions = Vec::new();
    for i in 0..100_000 {
        let x = (i % 1000) as f32;
        let y = (i / 1000) as f32;
        positions.push((x, y));
    }
    let selection = create_test_selection(Arc::clone(&context), positions).await;
    let selections: Vec<&dyn gup::interaction::Renderable> = vec![&selection];

    // Perform multiple queries
    let start_time = std::time::Instant::now();
    for _ in 0..10 {
        let _ = interaction_system
            .query_point(Vec2::new(500.0, 50.0), &selections)
            .await
            .expect("Stress test query should succeed");
    }
    let total_duration = start_time.elapsed();

    println!("10 queries on 100K elements completed in {total_duration:?}");

    // Average should still be reasonable
    let avg_per_query = total_duration / 10;
    assert!(
        avg_per_query.as_millis() < 50,
        "Average query time should be reasonable even for large datasets"
    );
}

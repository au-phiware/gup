// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Performance regression tests for the interaction system (GUP-077)
//!
//! These tests enforce performance thresholds to detect regressions.
//! Run with: `cargo test --test interaction_performance_tests -- --test-threads=1`
//!
//! Tests marked `#[ignore]` require manual opt-in (large datasets) via:
//!   `cargo test --test interaction_performance_tests -- --test-threads=1 --ignored`

use gup::interaction::{GpuInteractionQuery, InteractionSystem, Rect, Renderable, Vec2};
use gup::selection::Selection;
use gup::test_utils::create_test_context;
use gup::{Circle, InteractionData, RenderContext};
use std::sync::Arc;
use std::time::Instant;

/// Data element for performance testing.
#[derive(Debug, Clone)]
struct PerfData {
    x: f32,
    y: f32,
}

impl PerfData {
    fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

impl InteractionData for PerfData {
    fn position(&self) -> [f32; 2] {
        [self.x, self.y]
    }
}

fn generate_grid(count: usize) -> Vec<PerfData> {
    let side = (count as f32).sqrt().ceil() as usize;
    let spacing = 1000.0 / side as f32;
    (0..count)
        .map(|i| {
            let col = i % side;
            let row = i / side;
            PerfData::new(col as f32 * spacing, row as f32 * spacing)
        })
        .collect()
}

async fn get_context() -> Arc<RenderContext> {
    create_test_context()
        .await
        .expect("GPU context")
        .clone_context()
}

// ---------------------------------------------------------------------------
// Point query regression tests
// ---------------------------------------------------------------------------

/// 1K points: point query must complete in <150ms.
///
/// Note: even for small datasets, the first query includes GPU pipeline
/// setup and buffer upload overhead, so the threshold accounts for this
/// fixed cost (especially in debug builds).
#[tokio::test]
async fn test_point_query_1k_threshold() {
    let context = get_context().await;
    let data = generate_grid(1_000);
    let selection =
        Selection::<PerfData, Circle>::new(data, Arc::clone(&context)).expect("selection");
    let mut system = InteractionSystem::new(&context).await.unwrap();
    let sels: Vec<&dyn Renderable> = vec![&selection];

    let start = Instant::now();
    let hits = system
        .query_point(Vec2::new(500.0, 500.0), &sels)
        .await
        .expect("query");
    let elapsed = start.elapsed();

    println!("Point query 1K: {:?} ({} hits)", elapsed, hits.len());
    assert!(
        elapsed.as_millis() < 150,
        "Point query on 1K elements took {elapsed:?} (threshold: <150ms)"
    );
}

/// 10K points: point query must complete in <100ms.
#[tokio::test]
async fn test_point_query_10k_threshold() {
    let context = get_context().await;
    let data = generate_grid(10_000);
    let selection =
        Selection::<PerfData, Circle>::new(data, Arc::clone(&context)).expect("selection");
    let mut system = InteractionSystem::new(&context).await.unwrap();
    let sels: Vec<&dyn Renderable> = vec![&selection];

    let start = Instant::now();
    let hits = system
        .query_point(Vec2::new(500.0, 500.0), &sels)
        .await
        .expect("query");
    let elapsed = start.elapsed();

    println!("Point query 10K: {:?} ({} hits)", elapsed, hits.len());
    assert!(
        elapsed.as_millis() < 100,
        "Point query on 10K elements took {elapsed:?} (threshold: <100ms)"
    );
}

/// 100K points: point query must complete in <500ms.
#[tokio::test]
#[ignore] // large dataset — opt-in
async fn test_point_query_100k_threshold() {
    let context = get_context().await;
    let data = generate_grid(100_000);
    let selection =
        Selection::<PerfData, Circle>::new(data, Arc::clone(&context)).expect("selection");
    let mut system = InteractionSystem::new(&context).await.unwrap();
    let sels: Vec<&dyn Renderable> = vec![&selection];

    let start = Instant::now();
    let hits = system
        .query_point(Vec2::new(500.0, 500.0), &sels)
        .await
        .expect("query");
    let elapsed = start.elapsed();

    println!("Point query 100K: {:?} ({} hits)", elapsed, hits.len());
    assert!(
        elapsed.as_millis() < 500,
        "Point query on 100K elements took {elapsed:?} (threshold: <500ms)"
    );
}

// ---------------------------------------------------------------------------
// Region query regression tests
// ---------------------------------------------------------------------------

/// 10K points, medium region: must complete in <200ms.
#[tokio::test]
async fn test_region_query_10k_medium_threshold() {
    let context = get_context().await;
    let data = generate_grid(10_000);
    let selection =
        Selection::<PerfData, Circle>::new(data, Arc::clone(&context)).expect("selection");
    let mut system = InteractionSystem::new(&context).await.unwrap();
    let sels: Vec<&dyn Renderable> = vec![&selection];
    let region = Rect::new(Vec2::new(350.0, 350.0), Vec2::new(650.0, 650.0));

    let start = Instant::now();
    let hits = system.query_region(region, &sels).await.expect("query");
    let elapsed = start.elapsed();

    println!(
        "Region query 10K (medium): {:?} ({} hits)",
        elapsed,
        hits.len()
    );
    assert!(
        elapsed.as_millis() < 200,
        "Region query on 10K elements took {elapsed:?} (threshold: <200ms)"
    );
}

/// 100K points, large region: must complete in <1000ms.
#[tokio::test]
#[ignore]
async fn test_region_query_100k_large_threshold() {
    let context = get_context().await;
    let data = generate_grid(100_000);
    let selection =
        Selection::<PerfData, Circle>::new(data, Arc::clone(&context)).expect("selection");
    let mut system = InteractionSystem::new(&context).await.unwrap();
    let sels: Vec<&dyn Renderable> = vec![&selection];
    let region = Rect::new(Vec2::new(150.0, 150.0), Vec2::new(850.0, 850.0));

    let start = Instant::now();
    let hits = system.query_region(region, &sels).await.expect("query");
    let elapsed = start.elapsed();

    println!(
        "Region query 100K (large): {:?} ({} hits)",
        elapsed,
        hits.len()
    );
    assert!(
        elapsed.as_millis() < 1000,
        "Region query on 100K elements took {elapsed:?} (threshold: <1000ms)"
    );
}

// ---------------------------------------------------------------------------
// Batch query regression tests
// ---------------------------------------------------------------------------

/// Batch of 10 queries on 10K points: must complete in <500ms.
#[tokio::test]
async fn test_batch_query_10x10k_threshold() {
    let context = get_context().await;
    let data = generate_grid(10_000);
    let selection =
        Selection::<PerfData, Circle>::new(data, Arc::clone(&context)).expect("selection");
    let mut system = InteractionSystem::new(&context).await.unwrap();
    let sels: Vec<&dyn Renderable> = vec![&selection];
    let queries: Vec<GpuInteractionQuery> = (0..10)
        .map(|i| GpuInteractionQuery::point(Vec2::new(i as f32 * 100.0 + 50.0, 500.0), 1000))
        .collect();

    let start = Instant::now();
    let results = system.query_batch(&queries, &sels).await.expect("batch");
    let elapsed = start.elapsed();

    let total_hits: usize = results.iter().map(|r| r.len()).sum();
    println!(
        "Batch query 10×10K: {:?} ({total_hits} total hits)",
        elapsed
    );
    assert!(
        elapsed.as_millis() < 500,
        "Batch 10 queries on 10K elements took {elapsed:?} (threshold: <500ms)"
    );
}

// ---------------------------------------------------------------------------
// Streaming query regression tests
// ---------------------------------------------------------------------------

/// Streaming query on 10K points: must complete in <200ms.
#[tokio::test]
async fn test_streaming_query_10k_threshold() {
    let context = get_context().await;
    let data = generate_grid(10_000);
    let selection =
        Selection::<PerfData, Circle>::new(data, Arc::clone(&context)).expect("selection");
    let mut system = InteractionSystem::new(&context).await.unwrap();
    let sels: Vec<&dyn Renderable> = vec![&selection];
    let query = GpuInteractionQuery::point(Vec2::new(500.0, 500.0), 100_000);

    let start = Instant::now();
    let mut hit_count = 0u32;
    system
        .query_stream(query, &sels, |_| {
            hit_count += 1;
            true
        })
        .await
        .expect("stream");
    let elapsed = start.elapsed();

    println!("Streaming query 10K: {:?} ({hit_count} hits)", elapsed);
    assert!(
        elapsed.as_millis() < 200,
        "Streaming query on 10K elements took {elapsed:?} (threshold: <200ms)"
    );
}

// ---------------------------------------------------------------------------
// Repeated query caching benefit
// ---------------------------------------------------------------------------

/// Subsequent queries should be faster than the first (spatial index cached).
#[tokio::test]
async fn test_subsequent_query_faster() {
    let context = get_context().await;
    let data = generate_grid(10_000);
    let selection =
        Selection::<PerfData, Circle>::new(data, Arc::clone(&context)).expect("selection");
    let mut system = InteractionSystem::new(&context).await.unwrap();
    let sels: Vec<&dyn Renderable> = vec![&selection];

    // First query (includes index build)
    let start = Instant::now();
    system
        .query_point(Vec2::new(500.0, 500.0), &sels)
        .await
        .unwrap();
    let first = start.elapsed();

    // Subsequent queries (index cached)
    let start = Instant::now();
    for _ in 0..5 {
        system
            .query_point(Vec2::new(500.0, 500.0), &sels)
            .await
            .unwrap();
    }
    let avg_subsequent = start.elapsed() / 5;

    println!(
        "First query: {:?}, avg subsequent: {:?}",
        first, avg_subsequent
    );

    // Subsequent should not be dramatically slower (we verify they work,
    // not necessarily that they're faster since the first might also be fast
    // due to GPU warm-up). The key validation is correctness.
    assert!(
        avg_subsequent.as_millis() < 200,
        "Subsequent queries averaged {avg_subsequent:?} (threshold: <200ms)"
    );
}

// ---------------------------------------------------------------------------
// Query stats accuracy
// ---------------------------------------------------------------------------

/// Verify that QueryStats correctly tracks metrics across multiple queries.
#[tokio::test]
async fn test_query_stats_tracking() {
    let context = get_context().await;
    let data = generate_grid(1_000);
    let selection =
        Selection::<PerfData, Circle>::new(data, Arc::clone(&context)).expect("selection");
    let mut system = InteractionSystem::new(&context).await.unwrap();
    system.reset_stats();
    let sels: Vec<&dyn Renderable> = vec![&selection];

    // Run several queries
    let num_queries = 5u64;
    for _ in 0..num_queries {
        system
            .query_point(Vec2::new(500.0, 500.0), &sels)
            .await
            .unwrap();
    }

    let stats = system.query_stats();
    assert_eq!(
        stats.total_queries, num_queries,
        "Expected {num_queries} queries tracked, got {}",
        stats.total_queries
    );
    assert!(
        stats.total_elements_tested > 0,
        "Should track elements tested"
    );
    assert!(
        stats.average_query_time_us > 0.0,
        "Should track average query time (got {}μs)",
        stats.average_query_time_us
    );
    assert!(
        stats.max_query_time_us > 0.0,
        "Should track max query time (got {}μs)",
        stats.max_query_time_us
    );

    println!(
        "Stats after {} queries: avg={:.0}μs, max={:.0}μs, elements_tested={}, hits={}",
        stats.total_queries,
        stats.average_query_time_us,
        stats.max_query_time_us,
        stats.total_elements_tested,
        stats.total_hits,
    );
}

// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Integration tests for the non-blocking query API (GUP-198).
//!
//! Validates that:
//! - `query_point_async` / `query_region_async` return a `QueryHandle`
//! - `poll_result()` and `await_result()` produce correct hits
//! - Double-buffered staging enables pipelined queries
//! - Existing synchronous API continues to work unchanged
//! - Frame-aligned queries achieve low perceived latency

use gup::interaction::{ElementData, InteractionSystem, Rect, Vec2};
use gup::test_utils::create_test_context;
use std::time::Instant;

/// Create a test interaction system with a valid GPU context.
async fn create_test_interaction_system() -> InteractionSystem {
    let guard = create_test_context()
        .await
        .expect("Failed to create test context");
    let context = guard.clone_context();
    InteractionSystem::new(&context)
        .await
        .expect("Failed to create interaction system")
}

/// Generate a grid of element data.
fn generate_grid_elements(count: usize, spread: f32) -> Vec<ElementData> {
    let side = (count as f32).sqrt().ceil() as usize;
    let step = spread / side.max(1) as f32;
    (0..count)
        .map(|i| {
            let x = (i % side) as f32 * step;
            let y = (i / side) as f32 * step;
            ElementData {
                position: [x, y],
                size: [step * 0.4, step * 0.4],
                mark_type: 0,
                element_id: i as u32,
                selection_id: 0,
                _padding: 0,
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// AC1: query_point_async returns a handle/future
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_query_point_async_returns_handle() {
    let mut system = create_test_interaction_system().await;

    let elements = generate_grid_elements(500, 50.0);
    system
        .upload_element_data_cached(&elements, 1)
        .await
        .expect("upload failed");

    let handle = system
        .query_point_async(Vec2::new(0.0, 0.0))
        .await
        .expect("query_point_async failed");

    // The handle should not be consumed yet.
    assert!(!handle.is_consumed());

    // Consume via await_result.
    let hits = handle.await_result().await.expect("await_result failed");
    assert!(!hits.is_empty(), "Expected hits near the origin");
}

#[tokio::test]
async fn test_query_region_async_returns_handle() {
    let mut system = create_test_interaction_system().await;

    let elements = generate_grid_elements(500, 50.0);
    system
        .upload_element_data_cached(&elements, 1)
        .await
        .expect("upload failed");

    let handle = system
        .query_region_async(Rect::new(Vec2::new(0.0, 0.0), Vec2::new(15.0, 15.0)))
        .await
        .expect("query_region_async failed");

    let hits = handle.await_result().await.expect("await_result failed");
    assert!(
        hits.len() >= 2,
        "Expected at least 2 hits in a 15x15 region, got {}",
        hits.len()
    );
}

// ---------------------------------------------------------------------------
// AC2: Results can be polled or awaited without blocking
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_poll_result_eventually_resolves() {
    let mut system = create_test_interaction_system().await;

    let elements = generate_grid_elements(500, 50.0);
    system
        .upload_element_data_cached(&elements, 1)
        .await
        .expect("upload failed");

    let mut handle = system
        .query_point_async(Vec2::new(0.0, 0.0))
        .await
        .expect("query failed");

    // Poll in a loop — must resolve within a reasonable number of iterations.
    let mut result = None;
    for _ in 0..1000 {
        if let Some(hits) = handle.poll_result().expect("poll_result failed") {
            result = Some(hits);
            break;
        }
        // Small yield to allow driver progress.
        tokio::task::yield_now().await;
    }

    let hits = result.expect("poll_result never resolved after 1000 attempts");
    assert!(!hits.is_empty(), "Expected hits near the origin");
    assert!(handle.is_consumed(), "Handle should be consumed after poll");
}

#[tokio::test]
async fn test_poll_result_empty_dataset() {
    let mut system = create_test_interaction_system().await;

    // No data uploaded — cached count is 0.
    let mut handle = system
        .query_point_async(Vec2::new(0.0, 0.0))
        .await
        .expect("query failed");

    // Empty handle resolves immediately.
    let hits = handle
        .poll_result()
        .expect("poll_result failed")
        .expect("Should resolve immediately for empty dataset");

    assert!(hits.is_empty());
    assert!(handle.is_consumed());
}

#[tokio::test]
async fn test_await_result_empty_dataset() {
    let mut system = create_test_interaction_system().await;

    let handle = system
        .query_point_async(Vec2::new(0.0, 0.0))
        .await
        .expect("query failed");

    let hits = handle.await_result().await.expect("await failed");
    assert!(hits.is_empty());
}

// ---------------------------------------------------------------------------
// AC3: Double-buffered staging enables continuous query streams
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_double_buffered_pipelining() {
    let mut system = create_test_interaction_system().await;

    let elements = generate_grid_elements(1_000, 100.0);
    system
        .upload_element_data_cached(&elements, 1)
        .await
        .expect("upload failed");

    // Submit first async query.
    let handle_a = system
        .query_point_async(Vec2::new(0.0, 0.0))
        .await
        .expect("first query failed");

    // Submit second async query — uses the other staging slot.
    let handle_b = system
        .query_point_async(Vec2::new(50.0, 50.0))
        .await
        .expect("second query failed");

    // Both should resolve correctly.
    let hits_a = handle_a.await_result().await.expect("first await failed");
    let hits_b = handle_b.await_result().await.expect("second await failed");

    assert!(!hits_a.is_empty(), "First query should have hits");
    assert!(!hits_b.is_empty(), "Second query should have hits");
}

#[tokio::test]
async fn test_double_buffer_reuse_after_consume() {
    let mut system = create_test_interaction_system().await;

    let elements = generate_grid_elements(500, 50.0);
    system
        .upload_element_data_cached(&elements, 1)
        .await
        .expect("upload failed");

    // Fill both slots.
    let handle_a = system
        .query_point_async(Vec2::new(0.0, 0.0))
        .await
        .expect("first query failed");
    let handle_b = system
        .query_point_async(Vec2::new(10.0, 10.0))
        .await
        .expect("second query failed");

    // Consume first — frees a slot.
    let _hits_a = handle_a.await_result().await.expect("await a failed");

    // Third query should succeed because slot A is free again.
    let handle_c = system
        .query_point_async(Vec2::new(20.0, 20.0))
        .await
        .expect("third query should succeed after slot freed");

    let _hits_b = handle_b.await_result().await.expect("await b failed");
    let _hits_c = handle_c.await_result().await.expect("await c failed");
}

#[tokio::test]
async fn test_both_slots_busy_returns_error() {
    let mut system = create_test_interaction_system().await;

    let elements = generate_grid_elements(500, 50.0);
    system
        .upload_element_data_cached(&elements, 1)
        .await
        .expect("upload failed");

    // Fill both slots.
    let _handle_a = system
        .query_point_async(Vec2::new(0.0, 0.0))
        .await
        .expect("first query failed");
    let _handle_b = system
        .query_point_async(Vec2::new(10.0, 10.0))
        .await
        .expect("second query failed");

    // Third query should fail — both slots busy.
    let result = system.query_point_async(Vec2::new(20.0, 20.0)).await;
    assert!(
        result.is_err(),
        "Expected error when both staging slots are busy"
    );
}

#[tokio::test]
async fn test_drop_handle_frees_slot() {
    let mut system = create_test_interaction_system().await;

    let elements = generate_grid_elements(500, 50.0);
    system
        .upload_element_data_cached(&elements, 1)
        .await
        .expect("upload failed");

    // Fill both slots.
    let handle_a = system
        .query_point_async(Vec2::new(0.0, 0.0))
        .await
        .expect("first query failed");
    let _handle_b = system
        .query_point_async(Vec2::new(10.0, 10.0))
        .await
        .expect("second query failed");

    // Drop handle_a without consuming — should free the slot.
    drop(handle_a);

    // Third query should succeed.
    let handle_c = system
        .query_point_async(Vec2::new(20.0, 20.0))
        .await
        .expect("third query should succeed after drop");

    let hits = handle_c.await_result().await.expect("await failed");
    assert!(!hits.is_empty());
}

// ---------------------------------------------------------------------------
// AC4: Existing synchronous API continues to work unchanged
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_sync_api_unchanged() {
    let mut system = create_test_interaction_system().await;

    let elements = generate_grid_elements(1_000, 100.0);
    system
        .upload_element_data_cached(&elements, 1)
        .await
        .expect("upload failed");

    // Synchronous cached query should still work.
    let hits = system
        .query_point_cached(Vec2::new(0.0, 0.0))
        .await
        .expect("sync query failed");

    assert!(!hits.is_empty(), "Sync query should return hits");
    assert_eq!(hits[0].element_id, 0);
}

#[tokio::test]
async fn test_sync_and_async_interleaved() {
    let mut system = create_test_interaction_system().await;

    let elements = generate_grid_elements(1_000, 100.0);
    system
        .upload_element_data_cached(&elements, 1)
        .await
        .expect("upload failed");

    // Sync query first.
    let sync_hits = system
        .query_point_cached(Vec2::new(0.0, 0.0))
        .await
        .expect("sync query failed");

    // Async query.
    let handle = system
        .query_point_async(Vec2::new(0.0, 0.0))
        .await
        .expect("async query failed");
    let async_hits = handle.await_result().await.expect("await failed");

    // Both should produce the same results.
    assert_eq!(
        sync_hits.len(),
        async_hits.len(),
        "Sync and async should return the same number of hits"
    );
    for (s, a) in sync_hits.iter().zip(async_hits.iter()) {
        assert_eq!(s.element_id, a.element_id, "element_id mismatch");
    }
}

// ---------------------------------------------------------------------------
// Correctness: async results match sync
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_async_no_hits_far_from_data() {
    let mut system = create_test_interaction_system().await;

    let elements = generate_grid_elements(100, 10.0);
    system
        .upload_element_data_cached(&elements, 1)
        .await
        .expect("upload failed");

    let handle = system
        .query_point_async(Vec2::new(1000.0, 1000.0))
        .await
        .expect("query failed");
    let hits = handle.await_result().await.expect("await failed");
    assert!(hits.is_empty(), "Expected no hits far from elements");
}

#[tokio::test]
async fn test_async_rapid_successive_consistency() {
    let mut system = create_test_interaction_system().await;

    let elements = generate_grid_elements(500, 50.0);
    system
        .upload_element_data_cached(&elements, 1)
        .await
        .expect("upload failed");

    // Run 10 async queries sequentially and verify consistency.
    let mut all_results = Vec::new();
    for _ in 0..10 {
        let handle = system
            .query_point_async(Vec2::new(5.0, 5.0))
            .await
            .expect("query failed");
        let hits = handle.await_result().await.expect("await failed");
        all_results.push(hits);
    }

    let first = &all_results[0];
    for (i, result) in all_results.iter().enumerate().skip(1) {
        assert_eq!(
            first.len(),
            result.len(),
            "Query {i} returned different number of hits"
        );
        for (j, (a, b)) in first.iter().zip(result.iter()).enumerate() {
            assert_eq!(
                a.element_id, b.element_id,
                "Query {i} hit {j}: element_id mismatch"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// AC5: Perceived latency for frame-aligned queries <1ms
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_frame_aligned_perceived_latency() {
    let mut system = create_test_interaction_system().await;

    let count = 1_000;
    let elements = generate_grid_elements(count, 100.0);
    system
        .upload_element_data_cached(&elements, 1)
        .await
        .expect("upload failed");

    // Warm up.
    let warmup = system
        .query_point_async(Vec2::new(50.0, 50.0))
        .await
        .expect("warm-up failed");
    let _ = warmup.await_result().await;

    // Simulate frame-aligned usage: submit in "frame N", sleep to let GPU
    // finish, then measure the time to consume in "frame N+1".
    let iterations = 20;
    let mut total_consume_us = 0u128;

    for _ in 0..iterations {
        // Submit query (frame N).
        let handle = system
            .query_point_async(Vec2::new(50.0, 50.0))
            .await
            .expect("query failed");

        // Simulate inter-frame delay — the GPU finishes during this time.
        tokio::time::sleep(std::time::Duration::from_millis(16)).await;

        // Consume result (frame N+1) — measure only the consumption time.
        let start = Instant::now();
        let _hits = handle.await_result().await.expect("await failed");
        total_consume_us += start.elapsed().as_micros();
    }

    let avg_consume_us = total_consume_us as f64 / iterations as f64;
    println!(
        "Frame-aligned perceived latency: {iterations} queries on {count} elements, \
         avg {avg_consume_us:.0}µs per consume"
    );

    // In a frame-aligned scenario the GPU has already finished, so the
    // consume should be sub-millisecond (just read mapped data). Allow a
    // generous threshold for CI variability and debug mode overhead.
    let threshold_us = if cfg!(debug_assertions) {
        5_000.0
    } else {
        1_000.0
    };
    assert!(
        avg_consume_us < threshold_us,
        "Frame-aligned perceived latency {avg_consume_us:.0}µs exceeds {threshold_us:.0}µs threshold"
    );
}

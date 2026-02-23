// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Streaming statistical aggregation tests (GUP-146)
//!
//! This test file verifies that streaming statistics correctly handle
//! large datasets using Welford's online algorithm and chunked processing.

use gup::StreamingStatistics;

/// Test basic streaming statistics with small dataset
#[test]
fn test_streaming_basic() {
    let mut stats = StreamingStatistics::new();
    let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];

    stats.push_chunk(&data);

    assert_eq!(stats.count(), 5);
    assert_eq!(stats.mean(), 3.0);

    let result = stats.finalize();
    assert_eq!(result.count, 5);
    assert_eq!(result.mean, 3.0);
    assert_eq!(result.min, 1.0);
    assert_eq!(result.max, 5.0);
}

/// Test that streaming variance matches batch computation
#[test]
fn test_streaming_variance_correctness() {
    let data = vec![2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0];

    // Streaming computation
    let mut streaming = StreamingStatistics::new();
    streaming.push_chunk(&data);

    // Batch computation (manual)
    let mean = data.iter().sum::<f32>() / data.len() as f32;
    let variance = data
        .iter()
        .map(|&x| {
            let diff = x - mean;
            diff * diff
        })
        .sum::<f32>()
        / data.len() as f32;
    let std_dev = variance.sqrt();

    let result = streaming.finalize();
    assert!((result.mean - mean).abs() < 0.001);
    assert!((result.variance - variance).abs() < 0.001);
    assert!((result.std_dev - std_dev).abs() < 0.001);
}

/// Test processing data in chunks
#[test]
fn test_chunked_processing() {
    let data: Vec<f32> = (1..=100).map(|x| x as f32).collect();

    // Process in chunks of 10
    let mut stats = StreamingStatistics::with_chunk_size(10);
    stats.process_slice(&data, None);

    assert_eq!(stats.count(), 100);
    assert_eq!(stats.chunks_processed(), 10);

    let result = stats.finalize();
    assert_eq!(result.mean, 50.5);
    assert_eq!(result.min, 1.0);
    assert_eq!(result.max, 100.0);
}

/// Test that push() and push_chunk() produce identical results
#[test]
fn test_push_equivalence() {
    let data = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];

    // Method 1: push individual values
    let mut stats1 = StreamingStatistics::new();
    for &value in &data {
        stats1.push(value);
    }

    // Method 2: push as chunk
    let mut stats2 = StreamingStatistics::new();
    stats2.push_chunk(&data);

    let result1 = stats1.finalize();
    let result2 = stats2.finalize();

    assert_eq!(result1.count, result2.count);
    assert!((result1.mean - result2.mean).abs() < 0.001);
    assert!((result1.variance - result2.variance).abs() < 0.001);
    assert_eq!(result1.min, result2.min);
    assert_eq!(result1.max, result2.max);
}

/// Test merging statistics from multiple streams (for parallel processing)
#[test]
fn test_merge_statistics() {
    let data1: Vec<f32> = (1..=50).map(|x| x as f32).collect();
    let data2: Vec<f32> = (51..=100).map(|x| x as f32).collect();

    // Process in separate streams
    let mut stats1 = StreamingStatistics::new();
    stats1.push_chunk(&data1);

    let mut stats2 = StreamingStatistics::new();
    stats2.push_chunk(&data2);

    // Merge
    stats1.merge(&stats2);

    // Should match single-stream processing
    let mut stats_combined = StreamingStatistics::new();
    let all_data: Vec<f32> = (1..=100).map(|x| x as f32).collect();
    stats_combined.push_chunk(&all_data);

    let result_merged = stats1.finalize();
    let result_combined = stats_combined.finalize();

    assert_eq!(result_merged.count, result_combined.count);
    assert!((result_merged.mean - result_combined.mean).abs() < 0.001);
    assert!((result_merged.variance - result_combined.variance).abs() < 0.001);
    assert_eq!(result_merged.min, result_combined.min);
    assert_eq!(result_merged.max, result_combined.max);
}

/// Test processing large dataset (1M elements)
#[test]
fn test_large_dataset() {
    // Generate 1M elements
    let data: Vec<f32> = (0..1_000_000).map(|x| (x % 1000) as f32).collect();

    let mut stats = StreamingStatistics::with_chunk_size(100_000);
    stats.process_slice(&data, None);

    assert_eq!(stats.count(), 1_000_000);
    assert_eq!(stats.chunks_processed(), 10);

    let result = stats.finalize();
    assert_eq!(result.min, 0.0);
    assert_eq!(result.max, 999.0);

    // Mean should be around 499.5 (average of 0..999)
    assert!((result.mean - 499.5).abs() < 1.0);
}

/// Test progress callback functionality
#[test]
fn test_progress_callback() {
    use std::sync::{Arc, Mutex};

    let data: Vec<f32> = (1..=1000).map(|x| x as f32).collect();

    let progress_calls = Arc::new(Mutex::new(Vec::new()));
    let progress_calls_clone = progress_calls.clone();

    let mut stats = StreamingStatistics::with_chunk_size(100);

    stats.process_slice(
        &data,
        Some(Box::new(move |processed, total| {
            progress_calls_clone
                .lock()
                .unwrap()
                .push((processed, total));
        })),
    );

    let calls = progress_calls.lock().unwrap();
    // Should have 10 progress callbacks (1000 elements / 100 chunk size)
    assert_eq!(calls.len(), 10);
    assert_eq!(calls.last().unwrap().0, 1000);
    assert_eq!(calls.last().unwrap().1, 1000);
}

/// Test processing from iterator
#[test]
fn test_process_iterator() {
    let data: Vec<f32> = (1..=100).map(|x| x as f32).collect();

    let mut stats = StreamingStatistics::with_chunk_size(10);
    stats.process_iter(data.iter().copied(), None);

    assert_eq!(stats.count(), 100);
    assert_eq!(stats.chunks_processed(), 10);
}

/// Test reset functionality
#[test]
fn test_reset() {
    let mut stats = StreamingStatistics::new();
    stats.push_chunk(&[1.0, 2.0, 3.0, 4.0, 5.0]);

    assert_eq!(stats.count(), 5);

    stats.reset();

    assert_eq!(stats.count(), 0);
    assert_eq!(stats.mean(), 0.0);
    assert_eq!(stats.std_dev(), 0.0);
    assert_eq!(stats.chunks_processed(), 0);
}

/// Test empty dataset handling
#[test]
fn test_empty_dataset() {
    let stats = StreamingStatistics::new();
    let result = stats.finalize();

    assert_eq!(result.count, 0);
    assert_eq!(result.mean, 0.0);
    assert_eq!(result.variance, 0.0);
    assert_eq!(result.std_dev, 0.0);
}

/// Test single value dataset
#[test]
fn test_single_value() {
    let mut stats = StreamingStatistics::new();
    stats.push(42.0);

    let result = stats.finalize();
    assert_eq!(result.count, 1);
    assert_eq!(result.mean, 42.0);
    assert_eq!(result.variance, 0.0);
    assert_eq!(result.std_dev, 0.0);
    assert_eq!(result.min, 42.0);
    assert_eq!(result.max, 42.0);
}

/// Test uniform dataset (zero variance)
#[test]
fn test_uniform_dataset() {
    let mut stats = StreamingStatistics::new();
    let data = vec![5.0; 100];
    stats.push_chunk(&data);

    let result = stats.finalize();
    assert_eq!(result.mean, 5.0);
    assert_eq!(result.variance, 0.0);
    assert_eq!(result.std_dev, 0.0);
}

/// Test merge with empty aggregator
#[test]
fn test_merge_with_empty() {
    let mut stats1 = StreamingStatistics::new();
    stats1.push_chunk(&[1.0, 2.0, 3.0]);

    let stats2 = StreamingStatistics::new(); // empty

    stats1.merge(&stats2);

    assert_eq!(stats1.count(), 3);
    assert_eq!(stats1.mean(), 2.0);
}

/// Test merge into empty aggregator
#[test]
fn test_merge_into_empty() {
    let mut stats1 = StreamingStatistics::new(); // empty

    let mut stats2 = StreamingStatistics::new();
    stats2.push_chunk(&[1.0, 2.0, 3.0]);

    stats1.merge(&stats2);

    assert_eq!(stats1.count(), 3);
    assert_eq!(stats1.mean(), 2.0);
}

/// Test numerical stability with large differences
#[test]
fn test_numerical_stability() {
    let mut stats = StreamingStatistics::new();

    // Add very large and very small numbers
    let data = vec![1e10, 1e-10, 1e10, 1e-10];
    stats.push_chunk(&data);

    let result = stats.finalize();

    // Should not overflow or lose precision catastrophically
    assert!(result.mean.is_finite());
    assert!(result.variance.is_finite());
    assert!(result.std_dev.is_finite());
}

/// Benchmark simulation: measure that memory usage is constant
#[test]
fn test_constant_memory_usage() {
    // This test simulates processing a huge dataset
    // by processing multiple chunks without holding full dataset in memory

    let chunk_size = 10_000;
    let num_chunks = 100; // simulates 1M elements

    let mut stats = StreamingStatistics::with_chunk_size(chunk_size);

    for chunk_id in 0..num_chunks {
        // Generate chunk on-the-fly (not holding all data in memory)
        let chunk: Vec<f32> = (0..chunk_size)
            .map(|i| ((chunk_id * chunk_size + i) % 1000) as f32)
            .collect();
        stats.push_chunk(&chunk);
        // chunk is dropped here, so memory is constant
    }

    assert_eq!(stats.count(), 1_000_000);
    assert_eq!(stats.chunks_processed(), 100);

    let result = stats.finalize();
    assert!(result.mean.is_finite());
}

/// Test that streaming results match exact statistical computation
#[test]
fn test_streaming_matches_exact() {
    let data = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];

    // Exact computation
    let n = data.len() as f32;
    let mean = data.iter().sum::<f32>() / n;
    let variance = data.iter().map(|&x| (x - mean).powi(2)).sum::<f32>() / n;
    let std_dev = variance.sqrt();
    let min = data.iter().fold(f32::INFINITY, |a, &b| a.min(b));
    let max = data.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));

    // Streaming computation
    let mut stats = StreamingStatistics::new();
    stats.push_chunk(&data);
    let result = stats.finalize();

    assert!((result.mean - mean).abs() < 0.0001);
    assert!((result.variance - variance).abs() < 0.0001);
    assert!((result.std_dev - std_dev).abs() < 0.0001);
    assert_eq!(result.min, min);
    assert_eq!(result.max, max);
}

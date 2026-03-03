// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Integration tests for `DataStream<T>` builder, streaming, and
//! `Selection::stream()` integration.

use gup::mark::circle::Circle;
use gup::render::RenderContext;
use gup::selection::Selection;
use gup::streaming::{BackpressureStrategy, DataStream, StreamMode};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// Helper to obtain a wgpu device and queue for GPU tests.
async fn test_device() -> (wgpu::Device, wgpu::Queue) {
    let ctx = RenderContext::new().await.unwrap();
    let device = ctx.device().clone();
    let queue = ctx.queue().clone();
    (device, queue)
}

// ---------------------------------------------------------------------------
// Builder integration
// ---------------------------------------------------------------------------

#[tokio::test]
async fn builder_constructs_valid_stream() {
    let (device, _queue) = test_device().await;

    let stream = DataStream::<[f32; 4]>::builder()
        .capacity(5_000)
        .mode(StreamMode::SlidingWindow)
        .backpressure(BackpressureStrategy::EvictOldest)
        .build(&device)
        .expect("valid configuration");

    assert_eq!(stream.capacity(), 5_000);
    assert_eq!(stream.mode(), StreamMode::SlidingWindow);
    assert_eq!(stream.backpressure(), BackpressureStrategy::EvictOldest);
    assert_eq!(stream.len(), 0);
    assert!(stream.is_empty());
}

#[tokio::test]
async fn builder_rejects_zero_capacity() {
    let (device, _queue) = test_device().await;
    let result = DataStream::<f32>::builder().capacity(0).build(&device);
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("capacity"), "Error message: {msg}");
}

#[tokio::test]
async fn builder_rejects_missing_capacity() {
    let (device, _queue) = test_device().await;
    let result = DataStream::<f32>::builder().build(&device);
    assert!(result.is_err());
}

#[tokio::test]
async fn builder_rejects_block_with_ring_buffer() {
    let (device, _queue) = test_device().await;
    let result = DataStream::<f32>::builder()
        .capacity(10)
        .mode(StreamMode::RingBuffer)
        .backpressure(BackpressureStrategy::Block)
        .build(&device);
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("Block"), "Error message: {msg}");
}

// ---------------------------------------------------------------------------
// Push and flush
// ---------------------------------------------------------------------------

#[tokio::test]
async fn push_and_flush_writes_to_gpu() {
    let (device, queue) = test_device().await;

    let mut stream = DataStream::<f32>::builder()
        .capacity(100)
        .build(&device)
        .unwrap();

    stream.push(1.0);
    stream.push(2.0);
    stream.push(3.0);
    assert_eq!(stream.len(), 3);
    assert!(stream.is_dirty());

    let bytes = stream.flush(&device, &queue);
    assert!(bytes > 0);
    assert!(!stream.is_dirty());
}

#[tokio::test]
async fn push_batch_and_verify_length() {
    let (device, _queue) = test_device().await;

    let mut stream = DataStream::<[f32; 2]>::builder()
        .capacity(1_000)
        .build(&device)
        .unwrap();

    let batch: Vec<[f32; 2]> = (0..100).map(|i| [i as f32, (i * 2) as f32]).collect();
    let inserted = stream.push_batch(batch);
    assert_eq!(inserted, 100);
    assert_eq!(stream.len(), 100);
}

// ---------------------------------------------------------------------------
// Backpressure behaviour
// ---------------------------------------------------------------------------

#[tokio::test]
async fn sliding_window_evicts_on_overflow() {
    let (device, _queue) = test_device().await;

    let mut stream = DataStream::<f32>::builder()
        .capacity(5)
        .mode(StreamMode::SlidingWindow)
        .build(&device)
        .unwrap();

    stream.push_batch(vec![1.0, 2.0, 3.0, 4.0, 5.0]);
    assert_eq!(stream.len(), 5);

    // One more should evict the oldest
    stream.push(6.0);
    assert_eq!(stream.len(), 5);
}

#[tokio::test]
async fn append_only_drop_newest_drops_when_full() {
    let (device, _queue) = test_device().await;

    let mut stream = DataStream::<f32>::builder()
        .capacity(3)
        .mode(StreamMode::AppendOnly)
        .backpressure(BackpressureStrategy::DropNewest)
        .build(&device)
        .unwrap();

    stream.push_batch(vec![1.0, 2.0, 3.0]);
    assert!(!stream.push(4.0)); // dropped
    assert_eq!(stream.len(), 3);
}

// ---------------------------------------------------------------------------
// Subscriber pattern
// ---------------------------------------------------------------------------

#[tokio::test]
async fn subscriber_receives_every_push() {
    let (device, _queue) = test_device().await;

    let mut stream = DataStream::<f32>::builder()
        .capacity(100)
        .build(&device)
        .unwrap();

    let count = Arc::new(AtomicUsize::new(0));
    let c = count.clone();
    stream.subscribe(move |_| {
        c.fetch_add(1, Ordering::Relaxed);
    });

    stream.push_batch(vec![1.0, 2.0, 3.0, 4.0, 5.0]);
    assert_eq!(count.load(Ordering::Relaxed), 5);
}

#[tokio::test]
async fn multiple_subscribers_all_notified() {
    let (device, _queue) = test_device().await;

    let mut stream = DataStream::<f32>::builder()
        .capacity(100)
        .build(&device)
        .unwrap();

    let a = Arc::new(AtomicUsize::new(0));
    let b = Arc::new(AtomicUsize::new(0));
    let a_clone = a.clone();
    let b_clone = b.clone();

    stream.subscribe(move |_| {
        a_clone.fetch_add(1, Ordering::Relaxed);
    });
    stream.subscribe(move |_| {
        b_clone.fetch_add(1, Ordering::Relaxed);
    });

    stream.push(42.0);
    assert_eq!(a.load(Ordering::Relaxed), 1);
    assert_eq!(b.load(Ordering::Relaxed), 1);
}

#[tokio::test]
async fn unsubscribe_stops_notifications() {
    let (device, _queue) = test_device().await;

    let mut stream = DataStream::<f32>::builder()
        .capacity(100)
        .build(&device)
        .unwrap();

    let count = Arc::new(AtomicUsize::new(0));
    let c = count.clone();
    let handle = stream.subscribe(move |_| {
        c.fetch_add(1, Ordering::Relaxed);
    });

    stream.push(1.0);
    assert_eq!(count.load(Ordering::Relaxed), 1);

    stream.unsubscribe(handle);
    stream.push(2.0);
    assert_eq!(count.load(Ordering::Relaxed), 1); // unchanged
}

// ---------------------------------------------------------------------------
// Selection::stream() integration
// ---------------------------------------------------------------------------

#[tokio::test]
async fn selection_stream_replaces_static_data() {
    let (device, _queue) = test_device().await;

    let mut sel = Selection::<f32, Circle>::from_data(vec![1.0, 2.0, 3.0]);
    assert_eq!(sel.data().len(), 3);

    let stream = DataStream::<f32>::builder()
        .capacity(100)
        .build(&device)
        .unwrap();

    sel.stream(stream);

    // Static data should be cleared
    assert_eq!(sel.data().len(), 0);
    assert!(sel.has_stream());
}

#[tokio::test]
async fn selection_stream_push_and_verify_buffer_len() {
    let (device, queue) = test_device().await;

    let mut sel = Selection::<f32, Circle>::from_data(vec![]);
    let stream = DataStream::<f32>::builder()
        .capacity(100)
        .build(&device)
        .unwrap();
    sel.stream(stream);

    // Push data via the stream
    let ds = sel.stream_mut::<f32>().expect("stream should be attached");
    ds.push(1.0);
    ds.push(2.0);
    ds.push(3.0);
    assert_eq!(ds.len(), 3);

    // Flush to GPU
    let bytes = ds.flush(&device, &queue);
    assert!(bytes > 0);
    assert!(!ds.is_dirty());
}

#[tokio::test]
async fn selection_stream_interleaved_push_and_flush() {
    let (device, queue) = test_device().await;

    let mut sel = Selection::<[f32; 2], Circle>::from_data(vec![]);
    let stream = DataStream::<[f32; 2]>::builder()
        .capacity(1_000)
        .mode(StreamMode::SlidingWindow)
        .build(&device)
        .unwrap();
    sel.stream(stream);

    // Frame 1: push + flush
    {
        let ds = sel.stream_mut::<[f32; 2]>().unwrap();
        ds.push_batch(vec![[0.0, 0.0], [1.0, 1.0]]);
        ds.flush(&device, &queue);
        assert_eq!(ds.len(), 2);
    }

    // Frame 2: push more + flush
    {
        let ds = sel.stream_mut::<[f32; 2]>().unwrap();
        ds.push_batch(vec![[2.0, 2.0], [3.0, 3.0], [4.0, 4.0]]);
        ds.flush(&device, &queue);
        assert_eq!(ds.len(), 5);
    }
}

#[tokio::test]
async fn selection_detach_stream() {
    let (device, _queue) = test_device().await;

    let mut sel = Selection::<f32, Circle>::from_data(vec![]);
    let stream = DataStream::<f32>::builder()
        .capacity(100)
        .build(&device)
        .unwrap();
    sel.stream(stream);
    assert!(sel.has_stream());

    let detached = sel.detach_stream::<f32>();
    assert!(detached.is_some());
    assert!(!sel.has_stream());
}

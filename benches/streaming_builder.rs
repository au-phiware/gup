// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Performance benchmarks for the streaming data builder API.
//!
//! Measures:
//! 1. Builder construction overhead (capacity + mode + backpressure + build).
//! 2. Per-push subscriber dispatch cost (zero subscribers vs one subscriber).

use criterion::{Criterion, criterion_group, criterion_main};
use gup::render::RenderContext;
use gup::streaming::{BackpressureStrategy, DataStream, StreamMode};
use std::hint::black_box;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::runtime::Runtime;

/// Benchmark: full builder chain → DataStream construction.
fn bench_builder_construction(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let ctx = rt.block_on(async { RenderContext::new().await.unwrap() });
    let device = ctx.device().clone();

    c.bench_function("DataStreamBuilder::build (f32, cap=10_000)", |b| {
        b.iter(|| {
            let stream = DataStream::<f32>::builder()
                .capacity(black_box(10_000))
                .mode(black_box(StreamMode::SlidingWindow))
                .backpressure(black_box(BackpressureStrategy::EvictOldest))
                .build(&device)
                .unwrap();
            black_box(stream);
        });
    });
}

/// Benchmark: per-push cost with zero subscribers.
fn bench_push_zero_subscribers(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let ctx = rt.block_on(async { RenderContext::new().await.unwrap() });
    let device = ctx.device().clone();

    let mut stream = DataStream::<f32>::builder()
        .capacity(100_000)
        .build(&device)
        .unwrap();

    c.bench_function("DataStream::push (0 subscribers)", |b| {
        b.iter(|| {
            stream.push(black_box(42.0));
        });
    });
}

/// Benchmark: per-push cost with one subscriber (no-op callback).
fn bench_push_one_subscriber(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let ctx = rt.block_on(async { RenderContext::new().await.unwrap() });
    let device = ctx.device().clone();

    let mut stream = DataStream::<f32>::builder()
        .capacity(100_000)
        .build(&device)
        .unwrap();

    let count = Arc::new(AtomicUsize::new(0));
    let c_clone = count.clone();
    stream.subscribe(move |_| {
        c_clone.fetch_add(1, Ordering::Relaxed);
    });

    c.bench_function("DataStream::push (1 subscriber)", |b| {
        b.iter(|| {
            stream.push(black_box(42.0));
        });
    });
}

/// Benchmark: push_batch throughput.
fn bench_push_batch(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let ctx = rt.block_on(async { RenderContext::new().await.unwrap() });
    let device = ctx.device().clone();

    let mut stream = DataStream::<f32>::builder()
        .capacity(100_000)
        .mode(StreamMode::RingBuffer)
        .build(&device)
        .unwrap();

    let batch: Vec<f32> = (0..1_000).map(|i| i as f32).collect();

    c.bench_function("DataStream::push_batch (1000 items, RingBuffer)", |b| {
        b.iter(|| {
            let inserted = stream.push_batch(black_box(batch.clone()));
            black_box(inserted);
        });
    });
}

criterion_group!(
    benches,
    bench_builder_construction,
    bench_push_zero_subscribers,
    bench_push_one_subscriber,
    bench_push_batch
);
criterion_main!(benches);

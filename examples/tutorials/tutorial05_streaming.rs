// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! # Tutorial 5 — Streaming Data
//!
//! Demonstrates the `DataStream` builder API from
//! [Tutorial 5: Streaming Data](../../docs/tutorials/05_streaming_data.md).
//!
//! A `SineWave` producer generates batches of sine-wave points which are
//! pushed into a sliding-window `DataStream` capped at 1 000 points. A
//! subscriber counts committed updates, and each batch is flushed to the GPU.
//!
//! Run with: `cargo run --example tutorial05_streaming`
//!
//! This example runs headlessly (no window) since the tutorial focuses on the
//! data plumbing rather than rendering.

use gup::GupContext;
use gup::error::GupResult;
use gup::streaming::{BackpressureStrategy, DataStream, StreamMode};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Produces an endless sequence of `[x, y]` points along a sine wave.
struct SineWave {
    step: usize,
    batch_size: usize,
}

impl SineWave {
    fn new(batch_size: usize) -> Self {
        Self {
            step: 0,
            batch_size,
        }
    }

    /// Return the next batch of points.
    fn next_batch(&mut self) -> Vec<[f32; 2]> {
        let batch = (0..self.batch_size)
            .map(|i| {
                let t = (self.step + i) as f32 * 0.02;
                [t % 2.0 - 1.0, (t * std::f32::consts::PI).sin()]
            })
            .collect();
        self.step += self.batch_size;
        batch
    }
}

const CAPACITY: usize = 1000;
const BATCH_SIZE: usize = 300;
const BATCHES: usize = 5;

#[tokio::main]
async fn main() -> GupResult<()> {
    println!("Tutorial 5 — Streaming Data");
    println!("===========================\n");

    let context = GupContext::headless().await?;
    let device = &context.device;
    let queue = &context.queue;

    let mut stream = DataStream::<[f32; 2]>::builder()
        .capacity(CAPACITY)
        .mode(StreamMode::SlidingWindow)
        .backpressure(BackpressureStrategy::EvictOldest)
        .build(device)
        .expect("valid stream configuration");

    let updates = Arc::new(AtomicUsize::new(0));
    let counter = updates.clone();
    stream.subscribe(move |_update| {
        counter.fetch_add(1, Ordering::Relaxed);
    });

    let mut source = SineWave::new(BATCH_SIZE);
    for batch_num in 1..=BATCHES {
        let inserted = stream.push_batch(source.next_batch());
        let bytes = stream.flush(device, queue);
        println!(
            "  Batch {batch_num}: inserted {inserted}, {} buffered, {bytes} bytes uploaded",
            stream.len()
        );
    }

    println!(
        "\nWindow holds {} of {} points pushed ({} subscriber updates).",
        stream.len(),
        BATCH_SIZE * BATCHES,
        updates.load(Ordering::Relaxed)
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sine_wave_produces_requested_batch_size() {
        let mut source = SineWave::new(50);
        assert_eq!(source.next_batch().len(), 50);
    }

    #[test]
    fn sine_wave_continues_where_previous_batch_ended() {
        let mut whole = SineWave::new(20);
        let mut halves = SineWave::new(10);
        let expected = whole.next_batch();
        let mut actual = halves.next_batch();
        actual.extend(halves.next_batch());
        assert_eq!(actual, expected);
    }

    #[tokio::test]
    async fn sliding_window_caps_at_capacity() {
        let context = GupContext::headless().await.expect("headless context");
        let mut stream = DataStream::<[f32; 2]>::builder()
            .capacity(CAPACITY)
            .mode(StreamMode::SlidingWindow)
            .build(&context.device)
            .expect("valid stream configuration");

        let mut source = SineWave::new(BATCH_SIZE);
        for _ in 0..BATCHES {
            stream.push_batch(source.next_batch());
        }
        assert_eq!(stream.len(), CAPACITY);
    }
}

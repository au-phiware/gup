// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Streaming Live Chart Example
//!
//! Demonstrates end-to-end usage of the `DataStream<T>` builder API:
//!
//! 1. Constructs a `DataStream` via the fluent builder.
//! 2. Connects it to a `Selection` via `.stream()`.
//! 3. Simulates incoming data in a render loop with interleaved push/flush
//!    cycles.
//! 4. Prints stream statistics after each frame.

use gup::mark::circle::Circle;
use gup::render::RenderContext;
use gup::selection::Selection;
use gup::streaming::{BackpressureStrategy, DataStream, StreamMode};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

#[tokio::main]
async fn main() {
    println!("🚀 Gup Streaming Live Chart Example");
    println!("====================================\n");

    // Initialise the GPU render context.
    let ctx = RenderContext::new().await.expect("GPU context");
    let device = ctx.device().clone();
    let queue = ctx.queue().clone();

    // -- Step 1: Build a DataStream ------------------------------------------
    println!("📋 Building DataStream (capacity=500, SlidingWindow, EvictOldest)...");
    let stream = DataStream::<[f32; 2]>::builder()
        .capacity(500)
        .mode(StreamMode::SlidingWindow)
        .backpressure(BackpressureStrategy::EvictOldest)
        .build(&device)
        .expect("valid stream configuration");

    println!("   mode:         {:?}", stream.mode());
    println!("   backpressure: {:?}", stream.backpressure());
    println!("   capacity:     {}", stream.capacity());

    // -- Step 2: Connect stream to a Selection --------------------------------
    let mut selection = Selection::<[f32; 2], Circle>::from_data(vec![]);
    selection.stream(stream);
    println!("\n✅ Stream connected to Selection (static data dropped)");
    assert!(selection.has_stream());

    // -- Step 3: Register a subscriber ----------------------------------------
    let push_count = Arc::new(AtomicUsize::new(0));
    {
        let ds = selection.stream_mut::<[f32; 2]>().unwrap();
        let pc = push_count.clone();
        ds.subscribe(move |_update| {
            pc.fetch_add(1, Ordering::Relaxed);
        });
    }
    println!("🔔 Subscriber registered\n");

    // -- Step 4: Simulated render loop ----------------------------------------
    let num_frames = 10;
    let points_per_frame = 50;

    for frame in 0..num_frames {
        let ds = selection.stream_mut::<[f32; 2]>().unwrap();

        // Generate simulated data points (e.g. a sinusoidal signal)
        let batch: Vec<[f32; 2]> = (0..points_per_frame)
            .map(|i| {
                let t = (frame * points_per_frame + i) as f32 * 0.02;
                let x = t;
                let y = (t * 2.0 * std::f32::consts::PI / 10.0).sin();
                [x, y]
            })
            .collect();

        let inserted = ds.push_batch(batch);
        let bytes = ds.flush(&device, &queue);

        println!(
            "  Frame {frame:>2}: pushed {inserted:>3} pts, len={:<4} flushed {bytes:>5} bytes",
            ds.len()
        );
    }

    let total_pushes = push_count.load(Ordering::Relaxed);
    println!("\n📊 Summary:");
    println!("   Total subscriber notifications: {total_pushes}");
    println!(
        "   Final stream length:           {}",
        selection.stream_ref::<[f32; 2]>().unwrap().len()
    );
    println!(
        "   Capacity:                      {}",
        selection.stream_ref::<[f32; 2]>().unwrap().capacity()
    );

    // -- Step 5: Detach stream ------------------------------------------------
    let detached = selection.detach_stream::<[f32; 2]>();
    assert!(detached.is_some());
    assert!(!selection.has_stream());
    println!("   Stream detached:               ✅");

    println!("\n✅ Streaming live chart example complete!");
}

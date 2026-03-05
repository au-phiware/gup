// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! # Tutorial 5 — Streaming Data
//!
//! Demonstrates the `StreamingDataSource` and `StreamingScatterPlot` from
//! [Tutorial 5: Streaming Data](../../docs/tutorials/05_streaming_data.md).
//!
//! Implements a `SineWaveSource` that produces an infinite stream of sine-wave
//! points and wires it to a `StreamingScatterPlot` capped at 1 000 visible
//! points.  The example then consumes a few batches and prints statistics.
//!
//! Run with: `cargo run --example tutorial05_streaming`
//!
//! This example runs headlessly (no window) since the tutorial focuses on the
//! data-source plumbing rather than rendering.

use async_trait::async_trait;
use gup::async_mixable::streaming::{
    Point2D, StreamStats, StreamingDataSource, StreamingScatterPlot,
};
use gup::error::GupResult;

// ---------------------------------------------------------------------------
// SineWaveSource — from the Tutorial 5 "Full Example"
// ---------------------------------------------------------------------------

struct SineWaveSource {
    step: usize,
    batch_size: usize,
}

impl SineWaveSource {
    fn new() -> Self {
        Self {
            step: 0,
            batch_size: 50,
        }
    }
}

#[async_trait]
impl StreamingDataSource<Point2D> for SineWaveSource {
    async fn next_batch(&mut self) -> Option<GupResult<Vec<Point2D>>> {
        let batch: Vec<Point2D> = (0..self.batch_size)
            .map(|i| {
                let t = (self.step + i) as f32 * 0.02;
                Point2D {
                    x: t % 2.0 - 1.0,
                    y: (t * 3.14).sin(),
                    color: [0.9, 0.4, 0.1, 0.8],
                }
            })
            .collect();
        self.step += self.batch_size;
        Some(Ok(batch))
    }

    fn has_more(&self) -> bool {
        true // infinite stream
    }

    fn stream_stats(&self) -> StreamStats {
        StreamStats::default()
    }

    fn set_batch_size(&mut self, size: usize) {
        self.batch_size = size;
    }

    fn batch_size(&self) -> usize {
        self.batch_size
    }
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() -> GupResult<()> {
    println!("Tutorial 5 — Streaming Data");
    println!("===========================\n");

    let source = SineWaveSource::new();
    let chart = StreamingScatterPlot::new(source, 1000);

    println!("Streaming scatter plot ready");
    println!("Max visible points: {}", chart.max_points());

    // Consume a few batches to verify the source works
    let mut source2 = SineWaveSource::new();
    let mut total = 0usize;
    for batch_num in 0..5 {
        if let Some(Ok(batch)) = source2.next_batch().await {
            total += batch.len();
            println!(
                "  Batch {}: {} points (total: {})",
                batch_num + 1,
                batch.len(),
                total
            );
        }
    }

    println!("\nStream has_more: {}", source2.has_more());
    println!("All done — source produced {total} points across 5 batches.");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn sine_wave_source_produces_correct_batch_size() {
        let mut source = SineWaveSource::new();
        let batch = source.next_batch().await.unwrap().unwrap();
        assert_eq!(batch.len(), 50);
    }

    #[tokio::test]
    async fn sine_wave_source_always_has_more() {
        let source = SineWaveSource::new();
        assert!(source.has_more());
    }

    #[tokio::test]
    async fn sine_wave_source_respects_set_batch_size() {
        let mut source = SineWaveSource::new();
        source.set_batch_size(10);
        assert_eq!(source.batch_size(), 10);
        let batch = source.next_batch().await.unwrap().unwrap();
        assert_eq!(batch.len(), 10);
    }

    #[tokio::test]
    async fn streaming_scatter_plot_has_correct_max_points() {
        let source = SineWaveSource::new();
        let chart = StreamingScatterPlot::new(source, 1000);
        assert_eq!(chart.max_points(), 1000);
    }
}

// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Lightweight latency and throughput tracker for streaming operations.
//!
//! Keeps a bounded circular window of recent latency samples and provides
//! aggregate statistics (mean, P50, P99, max) as well as a throughput estimate
//! (operations per second).

use std::time::{Duration, Instant};

/// Configuration for the latency tracker.
#[derive(Debug, Clone)]
pub struct LatencyTrackerConfig {
    /// Maximum number of samples to keep in the rolling window.
    pub window_size: usize,
}

impl Default for LatencyTrackerConfig {
    fn default() -> Self {
        Self { window_size: 1024 }
    }
}

/// Rolling-window latency and throughput tracker.
#[derive(Debug, Clone)]
pub struct LatencyTracker {
    /// Circular buffer of recent latency samples.
    samples: Vec<Duration>,
    /// Write position in the circular buffer.
    write_pos: usize,
    /// Number of samples stored (may be less than `samples.len()` initially).
    count: usize,
    /// Total number of operations recorded (never resets).
    total_ops: u64,
    /// Timestamp of the first recorded operation (for overall throughput).
    first_op: Option<Instant>,
    /// Timestamp of the most recent recorded operation.
    last_op: Option<Instant>,
    /// Maximum capacity of the circular buffer.
    window_size: usize,
}

impl LatencyTracker {
    /// Create a new tracker with the given configuration.
    pub fn new(config: LatencyTrackerConfig) -> Self {
        let window_size = config.window_size.max(1);
        Self {
            samples: vec![Duration::ZERO; window_size],
            write_pos: 0,
            count: 0,
            total_ops: 0,
            first_op: None,
            last_op: None,
            window_size,
        }
    }

    /// Record a single latency sample.
    pub fn record(&mut self, latency: Duration) {
        self.samples[self.write_pos] = latency;
        self.write_pos = (self.write_pos + 1) % self.window_size;
        if self.count < self.window_size {
            self.count += 1;
        }
        self.total_ops += 1;
        let now = Instant::now();
        if self.first_op.is_none() {
            self.first_op = Some(now);
        }
        self.last_op = Some(now);
    }

    /// Record a batch of operations that all share the same total latency.
    pub fn record_batch(&mut self, latency: Duration, batch_size: usize) {
        if batch_size == 0 {
            return;
        }
        // Record the per-item latency as the sample.
        let per_item = latency / batch_size as u32;
        self.samples[self.write_pos] = per_item;
        self.write_pos = (self.write_pos + 1) % self.window_size;
        if self.count < self.window_size {
            self.count += 1;
        }
        self.total_ops += batch_size as u64;
        let now = Instant::now();
        if self.first_op.is_none() {
            self.first_op = Some(now);
        }
        self.last_op = Some(now);
    }

    /// Number of samples currently in the rolling window.
    pub fn sample_count(&self) -> usize {
        self.count
    }

    /// Total number of operations recorded since creation.
    pub fn total_ops(&self) -> u64 {
        self.total_ops
    }

    /// Mean latency over the rolling window.
    pub fn mean(&self) -> Option<Duration> {
        if self.count == 0 {
            return None;
        }
        let sum: Duration = self.active_samples().iter().copied().sum();
        Some(sum / self.count as u32)
    }

    /// Latency at the given percentile (0.0–1.0) over the rolling window.
    pub fn percentile(&self, p: f64) -> Option<Duration> {
        if self.count == 0 {
            return None;
        }
        let mut sorted: Vec<Duration> = self.active_samples().to_vec();
        sorted.sort();
        let idx = ((p * (sorted.len() - 1) as f64).round() as usize).min(sorted.len() - 1);
        Some(sorted[idx])
    }

    /// P50 (median) latency.
    pub fn p50(&self) -> Option<Duration> {
        self.percentile(0.50)
    }

    /// P99 latency.
    pub fn p99(&self) -> Option<Duration> {
        self.percentile(0.99)
    }

    /// Maximum latency in the rolling window.
    pub fn max(&self) -> Option<Duration> {
        self.active_samples().iter().copied().max()
    }

    /// Estimated throughput in operations per second, based on total lifetime.
    pub fn throughput_ops_per_sec(&self) -> f64 {
        match (self.first_op, self.last_op) {
            (Some(first), Some(last)) => {
                let elapsed = last.duration_since(first).as_secs_f64();
                if elapsed > 0.0 {
                    self.total_ops as f64 / elapsed
                } else {
                    self.total_ops as f64 // all in the same instant
                }
            }
            _ => 0.0,
        }
    }

    /// Reset all samples and counters.
    pub fn reset(&mut self) {
        self.write_pos = 0;
        self.count = 0;
        self.total_ops = 0;
        self.first_op = None;
        self.last_op = None;
    }

    /// Snapshot of the current statistics.
    pub fn snapshot(&self) -> LatencySnapshot {
        LatencySnapshot {
            sample_count: self.count,
            total_ops: self.total_ops,
            mean: self.mean(),
            p50: self.p50(),
            p99: self.p99(),
            max: self.max(),
            throughput_ops_per_sec: self.throughput_ops_per_sec(),
        }
    }

    // --- internals ---

    /// Return a slice of only the populated samples.
    fn active_samples(&self) -> &[Duration] {
        if self.count < self.window_size {
            &self.samples[..self.count]
        } else {
            &self.samples
        }
    }
}

impl Default for LatencyTracker {
    fn default() -> Self {
        Self::new(LatencyTrackerConfig::default())
    }
}

/// Immutable snapshot of latency statistics.
#[derive(Debug, Clone)]
pub struct LatencySnapshot {
    /// Number of samples in the rolling window at snapshot time.
    pub sample_count: usize,
    /// Lifetime total of recorded operations.
    pub total_ops: u64,
    /// Mean latency over the rolling window.
    pub mean: Option<Duration>,
    /// Median (P50) latency.
    pub p50: Option<Duration>,
    /// 99th-percentile latency.
    pub p99: Option<Duration>,
    /// Maximum latency in the rolling window.
    pub max: Option<Duration>,
    /// Estimated throughput in operations per second.
    pub throughput_ops_per_sec: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_tracker_returns_none() {
        let tracker = LatencyTracker::default();
        assert_eq!(tracker.sample_count(), 0);
        assert_eq!(tracker.total_ops(), 0);
        assert!(tracker.mean().is_none());
        assert!(tracker.p50().is_none());
        assert!(tracker.p99().is_none());
        assert!(tracker.max().is_none());
    }

    #[test]
    fn single_sample() {
        let mut tracker = LatencyTracker::default();
        tracker.record(Duration::from_micros(500));

        assert_eq!(tracker.sample_count(), 1);
        assert_eq!(tracker.total_ops(), 1);
        assert_eq!(tracker.mean(), Some(Duration::from_micros(500)));
        assert_eq!(tracker.p99(), Some(Duration::from_micros(500)));
    }

    #[test]
    fn multiple_samples_statistics() {
        let mut tracker = LatencyTracker::new(LatencyTrackerConfig { window_size: 100 });

        for i in 1..=100 {
            tracker.record(Duration::from_micros(i * 10));
        }

        assert_eq!(tracker.sample_count(), 100);
        let mean = tracker.mean().unwrap();
        // Mean of 10..=1000 step 10 = 505
        assert!(mean.as_micros() >= 500 && mean.as_micros() <= 510);
        assert_eq!(tracker.max(), Some(Duration::from_micros(1000)));
    }

    #[test]
    fn rolling_window_evicts_old_samples() {
        let mut tracker = LatencyTracker::new(LatencyTrackerConfig { window_size: 4 });

        // Fill with high values
        for _ in 0..4 {
            tracker.record(Duration::from_millis(100));
        }
        assert_eq!(tracker.max(), Some(Duration::from_millis(100)));

        // Now overwrite with low values
        for _ in 0..4 {
            tracker.record(Duration::from_micros(10));
        }
        assert_eq!(tracker.sample_count(), 4);
        assert_eq!(tracker.max(), Some(Duration::from_micros(10)));
        assert_eq!(tracker.total_ops(), 8);
    }

    #[test]
    fn batch_recording() {
        let mut tracker = LatencyTracker::default();
        tracker.record_batch(Duration::from_millis(10), 100);

        assert_eq!(tracker.total_ops(), 100);
        assert_eq!(tracker.sample_count(), 1);
        // Per-item latency = 10ms / 100 = 100µs
        assert_eq!(tracker.mean(), Some(Duration::from_micros(100)));
    }

    #[test]
    fn zero_batch_is_noop() {
        let mut tracker = LatencyTracker::default();
        tracker.record_batch(Duration::from_millis(10), 0);
        assert_eq!(tracker.total_ops(), 0);
        assert_eq!(tracker.sample_count(), 0);
    }

    #[test]
    fn reset_clears_everything() {
        let mut tracker = LatencyTracker::default();
        tracker.record(Duration::from_millis(1));
        tracker.record(Duration::from_millis(2));
        tracker.reset();

        assert_eq!(tracker.sample_count(), 0);
        assert_eq!(tracker.total_ops(), 0);
        assert!(tracker.mean().is_none());
    }

    #[test]
    fn snapshot_captures_state() {
        let mut tracker = LatencyTracker::default();
        tracker.record(Duration::from_micros(100));
        tracker.record(Duration::from_micros(200));

        let snap = tracker.snapshot();
        assert_eq!(snap.sample_count, 2);
        assert_eq!(snap.total_ops, 2);
        assert!(snap.mean.is_some());
    }
}

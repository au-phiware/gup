// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Real-time data streaming infrastructure for GPU-accelerated visualizations.
//!
//! This module provides [`StreamingBuffer<T>`], a GPU buffer wrapper that supports
//! keyed insert/update/remove operations with dirty-region tracking and
//! double-buffered GPU flushes. Only mutated byte ranges are transferred to the
//! GPU, achieving sub-millisecond update latency for real-time data streams.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────┐     ┌──────────────┐     ┌──────────────┐
//! │  CPU Data    │────>│ Dirty Region │────>│  GPU Flush   │
//! │  (keyed map) │     │   Tracker    │     │ (partial)    │
//! └─────────────┘     └──────────────┘     └──────────────┘
//!                                                │
//!                                          ┌─────┴─────┐
//!                                          │  Double    │
//!                                          │  Buffer    │
//!                                          │  Swap      │
//!                                          └───────────┘
//! ```
//!
//! # Example
//!
//! ```no_run
//! use gup::streaming::{StreamingBuffer, StreamingBufferConfig};
//! # async fn example(device: &wgpu::Device, queue: &wgpu::Queue) {
//! let config = StreamingBufferConfig {
//!     capacity: 10_000,
//!     ..Default::default()
//! };
//! let mut buf = StreamingBuffer::<[f32; 4]>::new(device, config);
//!
//! // Keyed insert
//! buf.insert(42, [1.0, 2.0, 3.0, 1.0]);
//!
//! // Flush only the dirty regions to the GPU
//! buf.flush(device, queue);
//! # }
//! ```

pub mod backpressure;
pub mod builder;
pub mod dirty_region;
pub mod latency;
pub mod mode;
pub mod ring_buffer;
pub mod stream;
pub mod streaming_buffer;

pub use backpressure::BackpressureStrategy;
pub use builder::{DataStreamBuilder, DataStreamError};
pub use dirty_region::{BufferRegion, DirtyRegionTracker};
pub use latency::LatencyTracker;
pub use mode::StreamMode;
pub use ring_buffer::RingBuffer;
pub use stream::{DataStream, SubscriberHandle};
pub use streaming_buffer::{StreamUpdate, StreamingBuffer, StreamingBufferConfig};

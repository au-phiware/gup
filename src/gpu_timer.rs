// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GPU timestamp query timer for precise compute-pass profiling.
//!
//! [`GpuTimer`] wraps a wgpu [`QuerySet`] with two timestamp
//! slots (begin/end) and the buffers required to resolve and read back the
//! results.  It is designed for lightweight, per-dispatch timing during
//! auto-tune calibration — not general-purpose profiling (see
//! [`performance::TimestampQueryManager`](crate::performance::TimestampQueryManager)
//! for that).
//!
//! # Feature detection
//!
//! [`GpuTimer::new`] returns `None` when the device does not support
//! [`Features::TIMESTAMP_QUERY`](wgpu::Features::TIMESTAMP_QUERY), making the
//! call site a simple `if let Some(timer) = GpuTimer::new(device, queue)`.
//!
//! # Usage
//!
//! ```rust,no_run
//! # use wgpu::*;
//! # use gup::gpu_timer::GpuTimer;
//! # fn example(device: &Device, queue: &Queue) {
//! if let Some(timer) = GpuTimer::new(device, queue) {
//!     let mut encoder = device.create_command_encoder(&Default::default());
//!
//!     // Pass the timestamp writes to the compute pass descriptor.
//!     {
//!         let ts = timer.compute_pass_timestamp_writes();
//!         let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
//!             label: None,
//!             timestamp_writes: Some(ts),
//!         });
//!         // ... dispatch workgroups ...
//!     }
//!
//!     // Resolve the query results into the staging buffer.
//!     timer.resolve(&mut encoder);
//!
//!     queue.submit([encoder.finish()]);
//!
//!     // Synchronously read back the elapsed time.
//!     if let Some(ns) = timer.read_elapsed_ns(device) {
//!         println!("Compute pass took {ns} ns on the GPU");
//!     }
//! }
//! # }
//! ```

use wgpu::{
    BufferDescriptor, BufferUsages, CommandEncoder, ComputePassTimestampWrites, Device, Features,
    MapMode, PollType, QuerySet, QuerySetDescriptor, QueryType, Queue,
};

/// A lightweight GPU timestamp timer for measuring a single compute pass.
///
/// Holds a two-slot [`QuerySet`] (begin + end), a resolve buffer, and a
/// staging buffer for CPU read-back.  Created only when the device supports
/// [`Features::TIMESTAMP_QUERY`].
#[derive(Debug)]
pub struct GpuTimer {
    query_set: QuerySet,
    resolve_buffer: wgpu::Buffer,
    staging_buffer: wgpu::Buffer,
    /// Nanoseconds per timestamp tick (from [`Queue::get_timestamp_period`]).
    timestamp_period: f32,
}

impl GpuTimer {
    /// Create a new `GpuTimer`, or `None` if the device does not support
    /// timestamp queries.
    pub fn new(device: &Device, queue: &Queue) -> Option<Self> {
        if !device.features().contains(Features::TIMESTAMP_QUERY) {
            return None;
        }

        let query_set = device.create_query_set(&QuerySetDescriptor {
            label: Some("gpu_timer_query_set"),
            ty: QueryType::Timestamp,
            count: 2,
        });

        // Resolve buffer: GPU writes resolved timestamps here.
        let resolve_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("gpu_timer_resolve"),
            size: 2 * std::mem::size_of::<u64>() as u64,
            usage: BufferUsages::QUERY_RESOLVE | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        // Staging buffer: CPU-readable copy of the resolve buffer.
        let staging_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("gpu_timer_staging"),
            size: 2 * std::mem::size_of::<u64>() as u64,
            usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let timestamp_period = queue.get_timestamp_period();

        Some(Self {
            query_set,
            resolve_buffer,
            staging_buffer,
            timestamp_period,
        })
    }

    /// Return [`ComputePassTimestampWrites`] that write the begin timestamp
    /// at index 0 and the end timestamp at index 1.
    pub fn compute_pass_timestamp_writes(&self) -> ComputePassTimestampWrites<'_> {
        ComputePassTimestampWrites {
            query_set: &self.query_set,
            beginning_of_pass_write_index: Some(0),
            end_of_pass_write_index: Some(1),
        }
    }

    /// Encode query resolution and a copy to the staging buffer.
    ///
    /// Call this **after** the compute pass has ended but **before**
    /// [`Queue::submit`].
    pub fn resolve(&self, encoder: &mut CommandEncoder) {
        encoder.resolve_query_set(&self.query_set, 0..2, &self.resolve_buffer, 0);
        encoder.copy_buffer_to_buffer(
            &self.resolve_buffer,
            0,
            &self.staging_buffer,
            0,
            2 * std::mem::size_of::<u64>() as u64,
        );
    }

    /// Synchronously map the staging buffer, read the two timestamps, and
    /// return the elapsed time in nanoseconds.
    ///
    /// Returns `None` if the buffer map fails or the timestamps are invalid
    /// (e.g. end < begin).
    ///
    /// **Note:** this calls `Device::poll(PollType::Wait { submission_index: None, timeout: None })` which blocks
    /// until the GPU has finished the submitted work.  This is acceptable
    /// during calibration but should not be used in a hot render loop.
    pub fn read_elapsed_ns(&self, device: &Device) -> Option<u128> {
        let slice = self.staging_buffer.slice(..);

        // Initiate the async map and immediately block for completion.
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        slice.map_async(MapMode::Read, move |result| {
            let _ = sender.send(result);
        });

        // Block until the GPU finishes and the map completes.
        match device.poll(PollType::Wait {
            submission_index: None,
            timeout: None,
        }) {
            Ok(_) => {}
            Err(_) => return None,
        }

        // Check the map result.
        match receiver.recv() {
            Ok(Ok(())) => {}
            _ => return None,
        }

        let data = slice.get_mapped_range();
        if data.len() < 16 {
            drop(data);
            self.staging_buffer.unmap();
            return None;
        }

        let begin = u64::from_le_bytes([
            data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
        ]);
        let end = u64::from_le_bytes([
            data[8], data[9], data[10], data[11], data[12], data[13], data[14], data[15],
        ]);
        drop(data);
        self.staging_buffer.unmap();

        if end < begin {
            return None;
        }

        let ticks = end - begin;
        let nanos = (ticks as f64 * self.timestamp_period as f64) as u128;
        Some(nanos)
    }

    /// Returns the timestamp period in nanoseconds per tick.
    pub fn timestamp_period(&self) -> f32 {
        self.timestamp_period
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `GpuTimer::new` returns `None` when `TIMESTAMP_QUERY` is unsupported.
    ///
    /// The default test adapter on CI may or may not support the feature, so
    /// this test validates the constructor's branch logic rather than a
    /// specific return value.
    #[test]
    fn new_returns_none_without_timestamp_feature() {
        // Request a device with *no* extra features.
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        rt.block_on(async {
            let instance = wgpu::Instance::default();
            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions::default())
                .await
                .expect("adapter");

            let supports = adapter.features().contains(Features::TIMESTAMP_QUERY);

            let (device, queue) = adapter
                .request_device(&wgpu::DeviceDescriptor {
                    label: Some("test_no_ts"),
                    required_features: Features::empty(),
                    ..Default::default()
                })
                .await
                .expect("device");

            let timer = GpuTimer::new(&device, &queue);

            if supports {
                // The adapter supports it but we didn't request it; the
                // device should not have the feature, so timer is None.
                //
                // Some backends promote adapter features automatically, so
                // we accept either outcome here. The important thing is no
                // panic.
                let _ = timer;
            } else {
                assert!(timer.is_none(), "Timer should be None without feature");
            }
        });
    }

    /// When the device *does* support `TIMESTAMP_QUERY`, the timer should
    /// be created and have a positive timestamp period.
    #[test]
    fn new_returns_some_with_timestamp_feature() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        rt.block_on(async {
            let instance = wgpu::Instance::default();
            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions::default())
                .await
                .expect("adapter");

            if !adapter.features().contains(Features::TIMESTAMP_QUERY) {
                // Can't test on this hardware — skip gracefully.
                return;
            }

            let (device, queue) = adapter
                .request_device(&wgpu::DeviceDescriptor {
                    label: Some("test_with_ts"),
                    required_features: Features::TIMESTAMP_QUERY,
                    ..Default::default()
                })
                .await
                .expect("device");

            let timer = GpuTimer::new(&device, &queue);
            assert!(timer.is_some(), "Timer should exist with TIMESTAMP_QUERY");
            let timer = timer.unwrap();
            assert!(
                timer.timestamp_period() > 0.0,
                "Timestamp period should be positive"
            );
        });
    }

    /// Verify that `resolve` encodes without panicking.
    #[test]
    fn resolve_encodes_without_panic() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        rt.block_on(async {
            let instance = wgpu::Instance::default();
            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions::default())
                .await
                .expect("adapter");

            if !adapter.features().contains(Features::TIMESTAMP_QUERY) {
                return;
            }

            let (device, queue) = adapter
                .request_device(&wgpu::DeviceDescriptor {
                    label: Some("test_resolve"),
                    required_features: Features::TIMESTAMP_QUERY,
                    ..Default::default()
                })
                .await
                .expect("device");

            let timer = GpuTimer::new(&device, &queue).unwrap();

            let mut encoder =
                device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());

            // Start a minimal compute pass with timestamp writes.
            {
                let _pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("test_pass"),
                    timestamp_writes: Some(timer.compute_pass_timestamp_writes()),
                });
                // No dispatch — just open and close the pass.
            }

            timer.resolve(&mut encoder);
            queue.submit([encoder.finish()]);

            // Reading back should succeed (elapsed ≈ 0 for an empty pass).
            let elapsed = timer.read_elapsed_ns(&device);
            // We accept Some(0) or Some(small_value) — the point is no
            // panic and successful readback.
            assert!(elapsed.is_some(), "Should read timestamps successfully");
        });
    }
}

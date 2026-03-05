// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Advanced performance profiling and GPU timing.
//!
//! This module provides detailed performance profiling capabilities including GPU
//! timestamp queries, rendering phase breakdown, and performance regression detection.

use crate::debug::memory_bandwidth::{
    BandwidthConfig, FrameBandwidthStats, MemoryBandwidthProfiler,
};
use crate::error::{GupError, GupResult};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::time::{Duration, Instant};
use wgpu::*;

/// Configuration for performance profiling.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfilingConfig {
    /// Enable GPU timestamp queries (if supported)
    pub enable_gpu_timing: bool,
    /// Track per-component rendering costs
    pub track_components: bool,
    /// Maximum number of historical frames to keep
    pub history_size: usize,
    /// Enable regression detection
    pub enable_regression_detection: bool,
    /// Threshold for performance regression alerts (as percentage)
    pub regression_threshold_percent: f32,
}

impl Default for ProfilingConfig {
    fn default() -> Self {
        Self {
            enable_gpu_timing: true,
            track_components: true,
            history_size: 120, // 2 seconds at 60 FPS
            enable_regression_detection: false,
            regression_threshold_percent: 20.0, // 20% slowdown triggers alert
        }
    }
}

/// Detailed frame performance statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetailedFrameStats {
    /// Total CPU time for frame
    #[serde(with = "duration_serde")]
    pub cpu_time: Duration,
    /// Total GPU time for frame (if timestamp queries available)
    #[serde(with = "option_duration_serde")]
    pub gpu_time: Option<Duration>,
    /// Individual render pass timings
    pub render_pass_times: Vec<RenderPassTiming>,
    /// Buffer upload/download timing
    #[serde(with = "duration_serde")]
    pub buffer_upload_time: Duration,
    /// Pipeline switch count
    pub pipeline_switches: u32,
    /// Draw call count
    pub draw_calls: u32,
    /// Compute dispatch count
    pub compute_dispatches: u32,
    /// Timestamp when this frame was recorded
    #[serde(skip, default = "Instant::now")]
    pub timestamp: Instant,
    /// Memory bandwidth statistics for this frame (if bandwidth profiling enabled)
    pub bandwidth_stats: Option<FrameBandwidthStats>,
}

impl Default for DetailedFrameStats {
    fn default() -> Self {
        Self {
            cpu_time: Duration::ZERO,
            gpu_time: None,
            render_pass_times: Vec::new(),
            buffer_upload_time: Duration::ZERO,
            pipeline_switches: 0,
            draw_calls: 0,
            compute_dispatches: 0,
            timestamp: Instant::now(),
            bandwidth_stats: None,
        }
    }
}

/// Timing information for a single render pass.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderPassTiming {
    /// Optional label for the render pass
    pub label: Option<String>,
    /// CPU time to record the pass
    #[serde(with = "duration_serde")]
    pub cpu_time: Duration,
    /// GPU execution time (if available)
    #[serde(with = "option_duration_serde")]
    pub gpu_time: Option<Duration>,
    /// Number of draw calls in this pass
    pub draw_calls: u32,
}

/// Performance statistics aggregated over multiple frames.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregateStats {
    /// Number of frames in this aggregate
    pub frame_count: u64,
    /// Average CPU time
    #[serde(with = "duration_serde")]
    pub avg_cpu_time: Duration,
    /// Average GPU time (if available)
    #[serde(with = "option_duration_serde")]
    pub avg_gpu_time: Option<Duration>,
    /// Minimum frame time
    #[serde(with = "duration_serde")]
    pub min_frame_time: Duration,
    /// Maximum frame time
    #[serde(with = "duration_serde")]
    pub max_frame_time: Duration,
    /// 95th percentile frame time
    #[serde(with = "duration_serde")]
    pub p95_frame_time: Duration,
    /// 99th percentile frame time
    #[serde(with = "duration_serde")]
    pub p99_frame_time: Duration,
    /// Frame time standard deviation
    #[serde(with = "duration_serde")]
    pub std_dev: Duration,
    /// Average draw calls per frame
    pub avg_draw_calls: f32,
    /// Average pipeline switches per frame
    pub avg_pipeline_switches: f32,
}

impl Default for AggregateStats {
    fn default() -> Self {
        Self {
            frame_count: 0,
            avg_cpu_time: Duration::ZERO,
            avg_gpu_time: None,
            min_frame_time: Duration::MAX,
            max_frame_time: Duration::ZERO,
            p95_frame_time: Duration::ZERO,
            p99_frame_time: Duration::ZERO,
            std_dev: Duration::ZERO,
            avg_draw_calls: 0.0,
            avg_pipeline_switches: 0.0,
        }
    }
}

/// Performance baseline for regression detection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceBaseline {
    /// Label for this baseline
    pub label: String,
    /// Baseline aggregate statistics
    pub stats: AggregateStats,
    /// When this baseline was recorded
    #[serde(skip, default = "Instant::now")]
    pub recorded_at: Instant,
}

/// Performance alert types.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PerformanceAlert {
    /// Frame time regression detected
    FrameTimeRegression {
        /// Current frame time.
        #[serde(with = "duration_serde")]
        current: Duration,
        /// Baseline frame time.
        #[serde(with = "duration_serde")]
        baseline: Duration,
        /// Percentage increase over baseline.
        percent_increase: f32,
    },
    /// Draw call count spike
    DrawCallSpike {
        /// Current draw call count.
        current: u32,
        /// Baseline average draw call count.
        baseline: f32,
    },
    /// Pipeline switch overhead
    ExcessivePipelineSwitches {
        /// Number of pipeline switches.
        count: u32,
    },
    /// Memory bandwidth concern
    HighMemoryBandwidth {
        /// Estimated bandwidth in GB/s.
        estimated_gbps: f32,
    },
}

/// GPU timestamp query manager.
#[derive(Debug)]
pub struct TimestampQueryManager {
    /// Device reference
    device: Device,
    /// Query set for timestamps
    query_set: Option<QuerySet>,
    /// Query resolve buffer
    resolve_buffer: Option<Buffer>,
    /// Query readback buffer
    readback_buffer: Option<Buffer>,
    /// Timestamp period (nanoseconds per tick)
    timestamp_period: f32,
}

impl TimestampQueryManager {
    /// Create a new timestamp query manager.
    pub fn new(device: &Device, max_queries: u32) -> GupResult<Self> {
        // Check if timestamp queries are supported
        let features = device.features();
        if !features.contains(Features::TIMESTAMP_QUERY) {
            return Ok(Self {
                device: device.clone(),
                query_set: None,
                resolve_buffer: None,
                readback_buffer: None,
                timestamp_period: 1.0,
            });
        }

        // Create query set
        let query_set = device.create_query_set(&QuerySetDescriptor {
            label: Some("gup_timestamp_queries"),
            ty: QueryType::Timestamp,
            count: max_queries,
        });

        // Create resolve buffer (GPU-side)
        let resolve_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("gup_query_resolve"),
            size: (max_queries * 8) as u64, // Each timestamp is 8 bytes
            usage: BufferUsages::QUERY_RESOLVE | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        // Create readback buffer (CPU-accessible)
        let readback_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("gup_query_readback"),
            size: (max_queries * 8) as u64,
            usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Get timestamp period from limits
        // Note: wgpu 26 doesn't expose timestamp_period directly on Limits
        // We'll use a default of 1 nanosecond per tick for simplicity
        // In practice, this should be queried from the device adapter
        let timestamp_period = 1.0;

        Ok(Self {
            device: device.clone(),
            query_set: Some(query_set),
            resolve_buffer: Some(resolve_buffer),
            readback_buffer: Some(readback_buffer),
            timestamp_period,
        })
    }

    /// Check if GPU timestamps are available.
    pub fn is_available(&self) -> bool {
        self.query_set.is_some()
    }

    /// Get the query set for writing timestamps.
    pub fn query_set(&self) -> Option<&QuerySet> {
        self.query_set.as_ref()
    }

    /// Resolve timestamp queries to buffer.
    pub fn resolve_queries(&self, encoder: &mut CommandEncoder, query_range: std::ops::Range<u32>) {
        if let (Some(query_set), Some(resolve_buffer)) = (&self.query_set, &self.resolve_buffer) {
            encoder.resolve_query_set(
                query_set,
                query_range.clone(),
                resolve_buffer,
                (query_range.start * 8) as u64,
            );
        }
    }

    /// Copy resolved queries to readback buffer.
    pub fn copy_to_readback(&self, encoder: &mut CommandEncoder, query_count: u32) {
        if let (Some(resolve_buffer), Some(readback_buffer)) =
            (&self.resolve_buffer, &self.readback_buffer)
        {
            encoder.copy_buffer_to_buffer(
                resolve_buffer,
                0,
                readback_buffer,
                0,
                (query_count * 8) as u64,
            );
        }
    }

    /// Read timestamp results from GPU.
    pub async fn read_timestamps(&self, query_count: u32) -> GupResult<Vec<u64>> {
        let readback_buffer = self
            .readback_buffer
            .as_ref()
            .ok_or_else(|| GupError::invalid_operation("Timestamps not available".to_string()))?;

        let buffer_slice = readback_buffer.slice(..);

        // Map the buffer for reading
        let (sender, receiver) = futures::channel::oneshot::channel();
        buffer_slice.map_async(MapMode::Read, move |result| {
            sender.send(result).ok();
        });

        let _ = self.device.poll(PollType::Wait {
            submission_index: None,
            timeout: None,
        });

        receiver
            .await
            .map_err(|_| {
                GupError::resource_error("Failed to receive buffer map result".to_string())
            })?
            .map_err(|e| GupError::resource_error(format!("Failed to map buffer: {:?}", e)))?;

        // Read timestamps
        let data = buffer_slice.get_mapped_range();
        let timestamps: Vec<u64> = (0..query_count as usize)
            .map(|i| {
                let offset = i * 8;
                u64::from_le_bytes([
                    data[offset],
                    data[offset + 1],
                    data[offset + 2],
                    data[offset + 3],
                    data[offset + 4],
                    data[offset + 5],
                    data[offset + 6],
                    data[offset + 7],
                ])
            })
            .collect();

        drop(data);
        readback_buffer.unmap();

        Ok(timestamps)
    }

    /// Convert timestamp ticks to duration.
    pub fn ticks_to_duration(&self, ticks: u64) -> Duration {
        let nanos = (ticks as f64 * self.timestamp_period as f64) as u64;
        Duration::from_nanos(nanos)
    }
}

/// Performance profiler that tracks detailed frame statistics.
#[derive(Debug)]
pub struct PerformanceProfiler {
    /// Configuration
    config: ProfilingConfig,
    /// Current frame stats being recorded
    current_frame: DetailedFrameStats,
    /// Historical frame data
    history: VecDeque<DetailedFrameStats>,
    /// Performance baselines
    baselines: Vec<PerformanceBaseline>,
    /// GPU timestamp manager
    timestamp_manager: Option<TimestampQueryManager>,
    /// Active alerts
    alerts: Vec<PerformanceAlert>,
    /// Memory bandwidth profiler
    bandwidth_profiler: MemoryBandwidthProfiler,
}

impl PerformanceProfiler {
    /// Create a new performance profiler.
    pub fn new(device: &Device, config: ProfilingConfig) -> GupResult<Self> {
        let timestamp_manager = if config.enable_gpu_timing {
            Some(TimestampQueryManager::new(device, 64)?)
        } else {
            None
        };

        let history_size = config.history_size;

        Ok(Self {
            config,
            current_frame: DetailedFrameStats::default(),
            history: VecDeque::with_capacity(history_size),
            baselines: Vec::new(),
            timestamp_manager,
            alerts: Vec::new(),
            bandwidth_profiler: MemoryBandwidthProfiler::new(BandwidthConfig {
                history_size,
                ..Default::default()
            }),
        })
    }

    /// Start recording a new frame.
    pub fn begin_frame(&mut self) {
        self.current_frame = DetailedFrameStats::default();
        self.current_frame.timestamp = Instant::now();
        self.bandwidth_profiler.begin_frame();
    }

    /// Record a render pass timing.
    pub fn record_render_pass(&mut self, timing: RenderPassTiming) {
        self.current_frame.draw_calls += timing.draw_calls;
        self.current_frame.render_pass_times.push(timing);
    }

    /// Record buffer upload timing.
    pub fn record_buffer_upload(&mut self, duration: Duration) {
        self.current_frame.buffer_upload_time += duration;
    }

    /// Record a buffer upload with bandwidth tracking.
    ///
    /// Tracks both the timing (for the existing profiler) and the byte count
    /// (for bandwidth analysis).
    pub fn record_buffer_upload_bandwidth(&mut self, label: &str, bytes: u64, duration: Duration) {
        self.current_frame.buffer_upload_time += duration;
        self.bandwidth_profiler.record_buffer_upload(label, bytes);
    }

    /// Record a buffer download (GPU → CPU readback) for bandwidth tracking.
    pub fn record_buffer_download_bandwidth(&mut self, label: &str, bytes: u64) {
        self.bandwidth_profiler.record_buffer_download(label, bytes);
    }

    /// Record a texture binding for bandwidth analysis.
    pub fn record_texture_binding(
        &mut self,
        label: &str,
        width: u32,
        height: u32,
        bytes_per_pixel: u32,
        slot: u32,
    ) {
        self.bandwidth_profiler
            .record_texture_binding(label, width, height, bytes_per_pixel, slot);
    }

    /// Record a pipeline switch.
    pub fn record_pipeline_switch(&mut self) {
        self.current_frame.pipeline_switches += 1;
    }

    /// Record a compute dispatch.
    pub fn record_compute_dispatch(&mut self) {
        self.current_frame.compute_dispatches += 1;
    }

    /// Complete the current frame and add to history.
    pub fn end_frame(&mut self, cpu_time: Duration) {
        self.current_frame.cpu_time = cpu_time;

        // Finalize bandwidth stats for this frame
        self.current_frame.bandwidth_stats = self.bandwidth_profiler.end_frame();

        // Add to history
        if self.history.len() >= self.config.history_size {
            self.history.pop_front();
        }
        self.history.push_back(self.current_frame.clone());

        // Check for bandwidth alerts
        let pressure = self.bandwidth_profiler.get_memory_pressure();
        if pressure.bandwidth_utilization > 0.6 {
            self.alerts.push(PerformanceAlert::HighMemoryBandwidth {
                estimated_gbps: pressure.avg_upload_gbps + pressure.avg_download_gbps,
            });
        }

        // Check for regressions if enabled
        if self.config.enable_regression_detection {
            self.detect_regressions();
        }
    }

    /// Get timestamp query manager.
    pub fn timestamp_manager(&self) -> Option<&TimestampQueryManager> {
        self.timestamp_manager.as_ref()
    }

    /// Get the current frame stats.
    pub fn current_frame(&self) -> &DetailedFrameStats {
        &self.current_frame
    }

    /// Get aggregate statistics over recent history.
    pub fn aggregate_stats(&self) -> AggregateStats {
        if self.history.is_empty() {
            return AggregateStats::default();
        }

        let mut stats = AggregateStats {
            frame_count: self.history.len() as u64,
            ..Default::default()
        };

        // Calculate basic stats
        let mut total_cpu = Duration::ZERO;
        let mut total_draw_calls = 0u64;
        let mut total_pipeline_switches = 0u64;
        let mut frame_times: Vec<Duration> = Vec::with_capacity(self.history.len());

        for frame in &self.history {
            total_cpu += frame.cpu_time;
            total_draw_calls += frame.draw_calls as u64;
            total_pipeline_switches += frame.pipeline_switches as u64;
            frame_times.push(frame.cpu_time);

            stats.min_frame_time = stats.min_frame_time.min(frame.cpu_time);
            stats.max_frame_time = stats.max_frame_time.max(frame.cpu_time);
        }

        stats.avg_cpu_time = total_cpu / stats.frame_count as u32;
        stats.avg_draw_calls = total_draw_calls as f32 / stats.frame_count as f32;
        stats.avg_pipeline_switches = total_pipeline_switches as f32 / stats.frame_count as f32;

        // Calculate percentiles
        frame_times.sort();
        let p95_idx = (frame_times.len() as f32 * 0.95) as usize;
        let p99_idx = (frame_times.len() as f32 * 0.99) as usize;
        stats.p95_frame_time = frame_times[p95_idx.min(frame_times.len() - 1)];
        stats.p99_frame_time = frame_times[p99_idx.min(frame_times.len() - 1)];

        // Calculate standard deviation
        let avg_nanos = stats.avg_cpu_time.as_nanos() as f64;
        let variance: f64 = frame_times
            .iter()
            .map(|t| {
                let diff = t.as_nanos() as f64 - avg_nanos;
                diff * diff
            })
            .sum::<f64>()
            / frame_times.len() as f64;
        stats.std_dev = Duration::from_nanos(variance.sqrt() as u64);

        stats
    }

    /// Record a performance baseline.
    pub fn record_baseline(&mut self, label: impl Into<String>) {
        let baseline = PerformanceBaseline {
            label: label.into(),
            stats: self.aggregate_stats(),
            recorded_at: Instant::now(),
        };
        self.baselines.push(baseline);
    }

    /// Get all baselines.
    pub fn baselines(&self) -> &[PerformanceBaseline] {
        &self.baselines
    }

    /// Get active performance alerts.
    pub fn alerts(&self) -> &[PerformanceAlert] {
        &self.alerts
    }

    /// Clear all alerts.
    pub fn clear_alerts(&mut self) {
        self.alerts.clear();
    }

    /// Detect performance regressions compared to baselines.
    fn detect_regressions(&mut self) {
        if self.baselines.is_empty() || self.history.len() < 30 {
            return; // Need enough history and a baseline
        }

        let current_stats = self.aggregate_stats();

        for baseline in &self.baselines {
            // Check frame time regression
            let baseline_time = baseline.stats.avg_cpu_time.as_secs_f32();
            let current_time = current_stats.avg_cpu_time.as_secs_f32();

            if current_time > baseline_time {
                let percent_increase = ((current_time - baseline_time) / baseline_time) * 100.0;

                if percent_increase > self.config.regression_threshold_percent {
                    self.alerts.push(PerformanceAlert::FrameTimeRegression {
                        current: current_stats.avg_cpu_time,
                        baseline: baseline.stats.avg_cpu_time,
                        percent_increase,
                    });
                }
            }

            // Check draw call spikes
            if current_stats.avg_draw_calls > baseline.stats.avg_draw_calls * 1.5 {
                self.alerts.push(PerformanceAlert::DrawCallSpike {
                    current: current_stats.avg_draw_calls as u32,
                    baseline: baseline.stats.avg_draw_calls,
                });
            }

            // Check pipeline switch overhead
            if current_stats.avg_pipeline_switches > 50.0 {
                self.alerts
                    .push(PerformanceAlert::ExcessivePipelineSwitches {
                        count: current_stats.avg_pipeline_switches as u32,
                    });
            }
        }
    }

    /// Get frame history.
    pub fn history(&self) -> &VecDeque<DetailedFrameStats> {
        &self.history
    }

    /// Get the memory bandwidth profiler.
    pub fn bandwidth_profiler(&self) -> &MemoryBandwidthProfiler {
        &self.bandwidth_profiler
    }

    /// Get a mutable reference to the memory bandwidth profiler.
    pub fn bandwidth_profiler_mut(&mut self) -> &mut MemoryBandwidthProfiler {
        &mut self.bandwidth_profiler
    }

    /// Clear frame history.
    pub fn clear_history(&mut self) {
        self.history.clear();
    }
}

/// Helper module for serializing [`Duration`] as seconds (f64).
pub(crate) mod duration_serde {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        duration.as_secs_f64().serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let secs = f64::deserialize(deserializer)?;
        Ok(Duration::from_secs_f64(secs))
    }
}

/// Helper module for serializing [`Option<Duration>`] as optional seconds (f64).
pub(crate) mod option_duration_serde {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Option<Duration>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match duration {
            Some(d) => d.as_secs_f64().serialize(serializer),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Duration>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let secs: Option<f64> = Option::deserialize(deserializer)?;
        Ok(secs.map(Duration::from_secs_f64))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profiling_config_defaults() {
        let config = ProfilingConfig::default();
        assert!(config.enable_gpu_timing);
        assert!(config.track_components);
        assert_eq!(config.history_size, 120);
        assert!(!config.enable_regression_detection);
        assert_eq!(config.regression_threshold_percent, 20.0);
    }

    #[test]
    fn test_detailed_frame_stats_default() {
        let stats = DetailedFrameStats::default();
        assert_eq!(stats.cpu_time, Duration::ZERO);
        assert!(stats.gpu_time.is_none());
        assert_eq!(stats.render_pass_times.len(), 0);
        assert_eq!(stats.pipeline_switches, 0);
        assert_eq!(stats.draw_calls, 0);
    }
}

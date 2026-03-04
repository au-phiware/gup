// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GPU memory bandwidth profiling and analysis.
//!
//! This module provides detailed tracking of GPU memory transfers including:
//! - Buffer upload/download bandwidth measurement
//! - Texture binding frequency and access pattern tracking
//! - Memory pressure detection and saturation warnings
//! - Transfer pattern optimization suggestions
//!
//! # Overview
//!
//! The [`MemoryBandwidthProfiler`] tracks all CPU↔GPU data transfers, aggregates
//! per-frame statistics via [`FrameBandwidthStats`], and exposes real-time
//! [`MemoryPressureStatus`] with actionable [`OptimizationSuggestion`]s.
//!
//! # Examples
//!
//! ```rust,ignore
//! use gup::debug::memory_bandwidth::{MemoryBandwidthProfiler, BandwidthConfig};
//!
//! let mut profiler = MemoryBandwidthProfiler::new(BandwidthConfig::default());
//!
//! // In your render loop:
//! profiler.begin_frame();
//! profiler.record_buffer_upload("vertex_data", 4096);
//! profiler.record_texture_binding("albedo_map", 2048, 2048);
//! let frame_stats = profiler.end_frame();
//!
//! // Check memory pressure:
//! let pressure = profiler.get_memory_pressure();
//! for suggestion in profiler.get_optimization_suggestions() {
//!     eprintln!("Optimization: {}", suggestion.description);
//! }
//! ```

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

/// Configuration for memory bandwidth profiling.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BandwidthConfig {
    /// Maximum number of historical frames to keep.
    pub history_size: usize,
    /// Assumed theoretical GPU memory bandwidth in GB/s for pressure calculation.
    /// Typical values: integrated GPU ~30, discrete GPU ~300-900.
    pub theoretical_bandwidth_gbps: f32,
    /// Threshold (0.0–1.0) above which memory pressure is considered high.
    pub high_pressure_threshold: f32,
    /// Threshold (0.0–1.0) above which memory pressure is considered critical.
    pub critical_pressure_threshold: f32,
    /// Number of consecutive texture bindings to the same slot before flagging thrashing.
    pub texture_thrash_threshold: u32,
    /// Minimum number of frames before producing optimization suggestions.
    pub min_frames_for_suggestions: usize,
}

impl Default for BandwidthConfig {
    fn default() -> Self {
        Self {
            history_size: 120,
            theoretical_bandwidth_gbps: 100.0,
            high_pressure_threshold: 0.6,
            critical_pressure_threshold: 0.85,
            texture_thrash_threshold: 4,
            min_frames_for_suggestions: 30,
        }
    }
}

/// Aggregate memory bandwidth statistics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MemoryBandwidthStats {
    /// Total bytes uploaded (CPU → GPU) since tracking began.
    pub upload_bytes: u64,
    /// Total bytes downloaded (GPU → CPU) since tracking began.
    pub download_bytes: u64,
    /// Estimated upload bandwidth in GB/s (based on recent history).
    pub upload_bandwidth_gbps: f32,
    /// Estimated download bandwidth in GB/s (based on recent history).
    pub download_bandwidth_gbps: f32,
    /// Total texture bindings since tracking began.
    pub texture_bindings: u32,
    /// Memory pressure score (0.0 = idle, 1.0 = saturated).
    pub memory_pressure_score: f32,
}

/// A single recorded buffer transfer event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BufferTransferEvent {
    /// Direction of the transfer.
    pub direction: TransferDirection,
    /// Optional label for the buffer.
    pub label: String,
    /// Number of bytes transferred.
    pub bytes: u64,
    /// When this transfer was recorded (relative to frame start).
    #[serde(with = "duration_serde")]
    pub offset_from_frame_start: Duration,
}

/// Direction of a memory transfer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransferDirection {
    /// CPU to GPU upload.
    Upload,
    /// GPU to CPU download (readback).
    Download,
}

/// A single recorded texture binding event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextureBindingEvent {
    /// Texture label or identifier.
    pub label: String,
    /// Texture width in pixels.
    pub width: u32,
    /// Texture height in u32.
    pub height: u32,
    /// Estimated bytes per pixel (e.g. 4 for RGBA8).
    pub bytes_per_pixel: u32,
    /// Binding slot index.
    pub slot: u32,
}

/// Per-frame bandwidth statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameBandwidthStats {
    /// Frame index (monotonically increasing).
    pub frame_index: u64,
    /// Total bytes uploaded this frame.
    pub upload_bytes: u64,
    /// Total bytes downloaded this frame.
    pub download_bytes: u64,
    /// Number of buffer transfers this frame.
    pub transfer_count: u32,
    /// Number of texture bindings this frame.
    pub texture_binding_count: u32,
    /// Estimated total bandwidth usage for this frame in bytes/second.
    pub estimated_bandwidth_bytes_per_sec: f64,
    /// Frame wall-clock duration.
    #[serde(with = "duration_serde")]
    pub frame_duration: Duration,
    /// Individual transfer events (kept for detailed analysis).
    pub transfers: Vec<BufferTransferEvent>,
    /// Individual texture binding events.
    pub texture_bindings: Vec<TextureBindingEvent>,
    /// Top bandwidth consumers (label → bytes).
    pub top_consumers: Vec<(String, u64)>,
}

/// Memory pressure level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PressureLevel {
    /// Under normal load.
    Low,
    /// Moderate bandwidth usage; performance may be affected.
    Medium,
    /// High bandwidth usage; likely a bottleneck.
    High,
    /// Critical bandwidth saturation; definite performance impact.
    Critical,
}

/// Real-time memory pressure status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryPressureStatus {
    /// Current pressure level.
    pub level: PressureLevel,
    /// Pressure score (0.0 = idle, 1.0 = saturated).
    pub score: f32,
    /// Recent average upload bandwidth in GB/s.
    pub avg_upload_gbps: f32,
    /// Recent average download bandwidth in GB/s.
    pub avg_download_gbps: f32,
    /// Combined bandwidth as a fraction of theoretical max.
    pub bandwidth_utilization: f32,
    /// Whether texture thrashing is detected.
    pub texture_thrashing_detected: bool,
    /// Number of thrashing textures detected.
    pub thrashing_texture_count: u32,
}

/// An optimization suggestion based on profiling data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationSuggestion {
    /// Priority of the suggestion (higher = more impactful).
    pub priority: u32,
    /// Short title.
    pub title: String,
    /// Detailed description.
    pub description: String,
    /// Category of optimization.
    pub category: SuggestionCategory,
}

/// Category of optimization suggestion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SuggestionCategory {
    /// Reduce upload volume.
    ReduceUploads,
    /// Reduce download (readback) volume.
    ReduceDownloads,
    /// Fix texture thrashing.
    TextureThrashing,
    /// General bandwidth optimization.
    BandwidthOptimization,
    /// Buffer management improvement.
    BufferManagement,
}

/// Memory access efficiency metrics.
///
/// Provides aggregate insight into how efficiently memory bandwidth is being
/// used over the profiled frame history.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MemoryEfficiencyMetrics {
    /// Ratio of upload bytes to download bytes.
    /// High values indicate mostly write-only workloads (typical for rendering).
    /// Values near 1.0 indicate balanced read/write (e.g. readback-heavy).
    pub upload_download_ratio: f64,
    /// Average size of each individual transfer in bytes.
    /// Small values (<4 KiB) suggest inefficient batching.
    pub avg_transfer_size_bytes: u64,
    /// Average number of transfers per frame.
    pub avg_transfers_per_frame: f64,
    /// Average total bytes transferred per frame.
    pub avg_bytes_per_frame: f64,
    /// Average texture bindings per frame.
    pub avg_texture_bindings_per_frame: f64,
    /// Estimated total texture memory footprint (unique textures × their size).
    pub estimated_texture_memory_bytes: u64,
    /// Number of unique textures seen across all profiled frames.
    pub unique_texture_count: u32,
    /// Ratio of uploads to the same label in consecutive frames (potential
    /// redundancy). 0.0 = no redundancy, 1.0 = every upload is redundant.
    pub redundant_upload_ratio: f64,
    /// Number of frames included in the analysis.
    pub total_frames_analysed: u64,
}

/// Texture access pattern tracker for thrashing detection.
#[derive(Debug, Clone, Default)]
struct TextureSlotTracker {
    /// Per-slot: list of recent texture labels bound to that slot.
    slot_history: HashMap<u32, VecDeque<String>>,
}

impl TextureSlotTracker {
    fn record_binding(&mut self, slot: u32, label: &str, max_history: usize) {
        let history = self.slot_history.entry(slot).or_default();
        history.push_back(label.to_string());
        if history.len() > max_history {
            history.pop_front();
        }
    }

    /// Count slots where different textures are being swapped in rapidly.
    fn detect_thrashing(&self, threshold: u32) -> Vec<u32> {
        let mut thrashing_slots = Vec::new();
        for (&slot, history) in &self.slot_history {
            if history.len() < threshold as usize {
                continue;
            }
            // Check the last `threshold` entries for alternation
            let recent: Vec<&str> = history
                .iter()
                .rev()
                .take(threshold as usize)
                .map(|s| s.as_str())
                .collect();
            let unique_count = {
                let mut seen = std::collections::HashSet::new();
                for label in &recent {
                    seen.insert(*label);
                }
                seen.len()
            };
            // If we see more than 1 unique texture in the recent window, it's thrashing
            if unique_count > 1 {
                thrashing_slots.push(slot);
            }
        }
        thrashing_slots
    }

    fn clear(&mut self) {
        self.slot_history.clear();
    }
}

/// In-progress frame being recorded.
#[derive(Debug)]
struct FrameInProgress {
    start: Instant,
    uploads: Vec<BufferTransferEvent>,
    downloads: Vec<BufferTransferEvent>,
    texture_bindings: Vec<TextureBindingEvent>,
    upload_bytes: u64,
    download_bytes: u64,
}

/// GPU memory bandwidth profiler.
///
/// Tracks buffer upload/download operations, texture binding frequency,
/// and provides real-time memory pressure analysis.
#[derive(Debug)]
pub struct MemoryBandwidthProfiler {
    config: BandwidthConfig,
    /// Completed frame history.
    history: VecDeque<FrameBandwidthStats>,
    /// Running frame counter.
    frame_counter: u64,
    /// Cumulative totals.
    total_upload_bytes: u64,
    total_download_bytes: u64,
    total_texture_bindings: u32,
    /// Current frame being recorded (None if between frames).
    current_frame: Option<FrameInProgress>,
    /// Texture slot tracker for thrashing detection.
    texture_tracker: TextureSlotTracker,
    /// Per-label cumulative bytes for identifying top consumers.
    label_bytes: HashMap<String, u64>,
}

impl MemoryBandwidthProfiler {
    /// Create a new profiler with the given configuration.
    pub fn new(config: BandwidthConfig) -> Self {
        let history_size = config.history_size;
        Self {
            config,
            history: VecDeque::with_capacity(history_size),
            frame_counter: 0,
            total_upload_bytes: 0,
            total_download_bytes: 0,
            total_texture_bindings: 0,
            current_frame: None,
            texture_tracker: TextureSlotTracker::default(),
            label_bytes: HashMap::new(),
        }
    }

    /// Begin recording a new frame.
    ///
    /// Must be called before any `record_*` methods for this frame.
    pub fn begin_frame(&mut self) {
        self.current_frame = Some(FrameInProgress {
            start: Instant::now(),
            uploads: Vec::new(),
            downloads: Vec::new(),
            texture_bindings: Vec::new(),
            upload_bytes: 0,
            download_bytes: 0,
        });
    }

    /// Record a buffer upload (CPU → GPU).
    pub fn record_buffer_upload(&mut self, label: &str, bytes: u64) {
        if let Some(ref mut frame) = self.current_frame {
            let event = BufferTransferEvent {
                direction: TransferDirection::Upload,
                label: label.to_string(),
                bytes,
                offset_from_frame_start: frame.start.elapsed(),
            };
            frame.upload_bytes += bytes;
            frame.uploads.push(event);
        }
    }

    /// Record a buffer download (GPU → CPU readback).
    pub fn record_buffer_download(&mut self, label: &str, bytes: u64) {
        if let Some(ref mut frame) = self.current_frame {
            let event = BufferTransferEvent {
                direction: TransferDirection::Download,
                label: label.to_string(),
                bytes,
                offset_from_frame_start: frame.start.elapsed(),
            };
            frame.download_bytes += bytes;
            frame.downloads.push(event);
        }
    }

    /// Record a texture binding event.
    ///
    /// `bytes_per_pixel` defaults to 4 (RGBA8) if set to 0.
    pub fn record_texture_binding(
        &mut self,
        label: &str,
        width: u32,
        height: u32,
        bytes_per_pixel: u32,
        slot: u32,
    ) {
        let bpp = if bytes_per_pixel == 0 {
            4
        } else {
            bytes_per_pixel
        };
        if let Some(ref mut frame) = self.current_frame {
            frame.texture_bindings.push(TextureBindingEvent {
                label: label.to_string(),
                width,
                height,
                bytes_per_pixel: bpp,
                slot,
            });
        }
        self.texture_tracker
            .record_binding(slot, label, self.config.history_size);
    }

    /// End the current frame and return the frame's bandwidth statistics.
    ///
    /// Returns `None` if `begin_frame` was not called.
    pub fn end_frame(&mut self) -> Option<FrameBandwidthStats> {
        let frame = self.current_frame.take()?;
        let frame_duration = frame.start.elapsed();
        let frame_index = self.frame_counter;
        self.frame_counter += 1;

        // Update cumulative totals
        self.total_upload_bytes += frame.upload_bytes;
        self.total_download_bytes += frame.download_bytes;
        self.total_texture_bindings += frame.texture_bindings.len() as u32;

        // Merge transfers
        let mut all_transfers: Vec<BufferTransferEvent> = Vec::new();
        all_transfers.extend(frame.uploads);
        all_transfers.extend(frame.downloads);

        // Track per-label bytes
        for t in &all_transfers {
            *self.label_bytes.entry(t.label.clone()).or_insert(0) += t.bytes;
        }

        // Compute top consumers for this frame
        let mut frame_label_bytes: HashMap<String, u64> = HashMap::new();
        for t in &all_transfers {
            *frame_label_bytes.entry(t.label.clone()).or_insert(0) += t.bytes;
        }
        let mut top_consumers: Vec<(String, u64)> = frame_label_bytes.into_iter().collect();
        top_consumers.sort_by(|a, b| b.1.cmp(&a.1));
        top_consumers.truncate(10);

        // Estimate bandwidth
        let total_bytes = frame.upload_bytes + frame.download_bytes;
        let estimated_bps = if frame_duration.as_secs_f64() > 0.0 {
            total_bytes as f64 / frame_duration.as_secs_f64()
        } else {
            0.0
        };

        let stats = FrameBandwidthStats {
            frame_index,
            upload_bytes: frame.upload_bytes,
            download_bytes: frame.download_bytes,
            transfer_count: all_transfers.len() as u32,
            texture_binding_count: frame.texture_bindings.len() as u32,
            estimated_bandwidth_bytes_per_sec: estimated_bps,
            frame_duration,
            transfers: all_transfers,
            texture_bindings: frame.texture_bindings,
            top_consumers,
        };

        // Add to history
        if self.history.len() >= self.config.history_size {
            self.history.pop_front();
        }
        self.history.push_back(stats.clone());

        Some(stats)
    }

    /// Get aggregate bandwidth statistics over the profiler's lifetime.
    pub fn get_stats(&self) -> MemoryBandwidthStats {
        let (avg_upload_gbps, avg_download_gbps) = self.compute_recent_bandwidth();

        let total_bw = avg_upload_gbps + avg_download_gbps;
        let pressure = if self.config.theoretical_bandwidth_gbps > 0.0 {
            (total_bw / self.config.theoretical_bandwidth_gbps).min(1.0)
        } else {
            0.0
        };

        MemoryBandwidthStats {
            upload_bytes: self.total_upload_bytes,
            download_bytes: self.total_download_bytes,
            upload_bandwidth_gbps: avg_upload_gbps,
            download_bandwidth_gbps: avg_download_gbps,
            texture_bindings: self.total_texture_bindings,
            memory_pressure_score: pressure,
        }
    }

    /// Get real-time memory pressure status.
    pub fn get_memory_pressure(&self) -> MemoryPressureStatus {
        let (avg_upload_gbps, avg_download_gbps) = self.compute_recent_bandwidth();
        let combined = avg_upload_gbps + avg_download_gbps;

        let utilization = if self.config.theoretical_bandwidth_gbps > 0.0 {
            combined / self.config.theoretical_bandwidth_gbps
        } else {
            0.0
        };

        let thrashing_slots = self
            .texture_tracker
            .detect_thrashing(self.config.texture_thrash_threshold);

        let level = if utilization >= self.config.critical_pressure_threshold {
            PressureLevel::Critical
        } else if utilization >= self.config.high_pressure_threshold {
            PressureLevel::High
        } else if utilization >= self.config.high_pressure_threshold * 0.5 {
            PressureLevel::Medium
        } else {
            PressureLevel::Low
        };

        MemoryPressureStatus {
            level,
            score: utilization.min(1.0),
            avg_upload_gbps,
            avg_download_gbps,
            bandwidth_utilization: utilization,
            texture_thrashing_detected: !thrashing_slots.is_empty(),
            thrashing_texture_count: thrashing_slots.len() as u32,
        }
    }

    /// Get optimization suggestions based on recent profiling data.
    pub fn get_optimization_suggestions(&self) -> Vec<OptimizationSuggestion> {
        let mut suggestions = Vec::new();

        if self.history.len() < self.config.min_frames_for_suggestions {
            return suggestions;
        }

        let pressure = self.get_memory_pressure();

        // Suggestion: high upload bandwidth
        if pressure.avg_upload_gbps > self.config.theoretical_bandwidth_gbps * 0.4 {
            suggestions.push(OptimizationSuggestion {
                priority: 3,
                title: "High upload bandwidth".to_string(),
                description: format!(
                    "Upload bandwidth is {:.2} GB/s ({:.0}% of theoretical max). \
                     Consider using persistent mapped buffers, reducing per-frame \
                     upload volume, or batching updates.",
                    pressure.avg_upload_gbps,
                    pressure.avg_upload_gbps / self.config.theoretical_bandwidth_gbps * 100.0
                ),
                category: SuggestionCategory::ReduceUploads,
            });
        }

        // Suggestion: high download bandwidth
        if pressure.avg_download_gbps > self.config.theoretical_bandwidth_gbps * 0.2 {
            suggestions.push(OptimizationSuggestion {
                priority: 3,
                title: "High readback bandwidth".to_string(),
                description: format!(
                    "Download bandwidth is {:.2} GB/s. GPU→CPU readbacks are expensive. \
                     Consider reducing readback frequency, using compute shaders to \
                     summarise data on the GPU, or async readback with double-buffering.",
                    pressure.avg_download_gbps,
                ),
                category: SuggestionCategory::ReduceDownloads,
            });
        }

        // Suggestion: texture thrashing
        if pressure.texture_thrashing_detected {
            suggestions.push(OptimizationSuggestion {
                priority: 2,
                title: "Texture thrashing detected".to_string(),
                description: format!(
                    "Detected {} texture slot(s) with rapid binding changes. \
                     Consider using texture atlases or array textures to \
                     reduce binding switches.",
                    pressure.thrashing_texture_count,
                ),
                category: SuggestionCategory::TextureThrashing,
            });
        }

        // Suggestion: many small transfers
        let avg_transfer_count = self
            .history
            .iter()
            .map(|f| f.transfer_count as f64)
            .sum::<f64>()
            / self.history.len() as f64;
        let avg_transfer_size = {
            let total_bytes: u64 = self
                .history
                .iter()
                .map(|f| f.upload_bytes + f.download_bytes)
                .sum();
            let total_transfers: u64 = self.history.iter().map(|f| f.transfer_count as u64).sum();
            if total_transfers > 0 {
                total_bytes / total_transfers
            } else {
                0
            }
        };
        if avg_transfer_count > 20.0 && avg_transfer_size < 4096 {
            suggestions.push(OptimizationSuggestion {
                priority: 2,
                title: "Many small buffer transfers".to_string(),
                description: format!(
                    "Averaging {:.0} transfers/frame with {avg_transfer_size} bytes/transfer. \
                     Consider batching small transfers into fewer, larger writes.",
                    avg_transfer_count,
                ),
                category: SuggestionCategory::BufferManagement,
            });
        }

        // Suggestion: overall bandwidth pressure
        if pressure.level == PressureLevel::Critical {
            suggestions.push(OptimizationSuggestion {
                priority: 4,
                title: "Critical memory bandwidth saturation".to_string(),
                description: format!(
                    "Bandwidth utilization is at {:.0}% of theoretical max ({:.2} GB/s). \
                     This is a major bottleneck. Review all transfers and consider: \
                     GPU-side data generation, LOD-based transfer reduction, \
                     or frame-to-frame delta transfers.",
                    pressure.bandwidth_utilization * 100.0,
                    self.config.theoretical_bandwidth_gbps,
                ),
                category: SuggestionCategory::BandwidthOptimization,
            });
        }

        // Sort by priority descending
        suggestions.sort_by(|a, b| b.priority.cmp(&a.priority));
        suggestions
    }

    /// Get the frame history.
    pub fn history(&self) -> &VecDeque<FrameBandwidthStats> {
        &self.history
    }

    /// Get memory access efficiency metrics.
    ///
    /// Provides aggregate insight into how efficiently memory bandwidth is being
    /// used, including upload/download ratio, average transfer sizes, texture
    /// memory footprint, and bandwidth-per-draw-call estimates.
    pub fn get_efficiency_metrics(&self) -> MemoryEfficiencyMetrics {
        if self.history.is_empty() {
            return MemoryEfficiencyMetrics::default();
        }

        let total_upload: u64 = self.history.iter().map(|f| f.upload_bytes).sum();
        let total_download: u64 = self.history.iter().map(|f| f.download_bytes).sum();
        let total_transfers: u64 = self.history.iter().map(|f| f.transfer_count as u64).sum();
        let total_tex_bindings: u64 = self
            .history
            .iter()
            .map(|f| f.texture_binding_count as u64)
            .sum();
        let frame_count = self.history.len() as f64;

        let upload_download_ratio = if total_download > 0 {
            total_upload as f64 / total_download as f64
        } else if total_upload > 0 {
            f64::INFINITY
        } else {
            0.0
        };

        let avg_transfer_size = if total_transfers > 0 {
            (total_upload + total_download) / total_transfers
        } else {
            0
        };

        let avg_transfers_per_frame = total_transfers as f64 / frame_count;
        let avg_bytes_per_frame = (total_upload + total_download) as f64 / frame_count;
        let avg_texture_bindings_per_frame = total_tex_bindings as f64 / frame_count;

        // Estimate total texture memory footprint from binding events
        let mut unique_textures: HashMap<String, u64> = HashMap::new();
        for frame in &self.history {
            for tex in &frame.texture_bindings {
                let tex_bytes = tex.width as u64 * tex.height as u64 * tex.bytes_per_pixel as u64;
                unique_textures
                    .entry(tex.label.clone())
                    .and_modify(|b| *b = (*b).max(tex_bytes))
                    .or_insert(tex_bytes);
            }
        }
        let estimated_texture_memory: u64 = unique_textures.values().sum();

        // Redundant upload detection: same label appearing in consecutive frames
        let mut redundant_uploads = 0u64;
        let mut prev_labels: std::collections::HashSet<String> = std::collections::HashSet::new();
        for frame in &self.history {
            let mut current_labels: std::collections::HashSet<String> =
                std::collections::HashSet::new();
            for t in &frame.transfers {
                if t.direction == TransferDirection::Upload {
                    current_labels.insert(t.label.clone());
                    if prev_labels.contains(&t.label) {
                        redundant_uploads += 1;
                    }
                }
            }
            prev_labels = current_labels;
        }
        let total_uploads: u64 = self
            .history
            .iter()
            .flat_map(|f| f.transfers.iter())
            .filter(|t| t.direction == TransferDirection::Upload)
            .count() as u64;
        let redundant_upload_ratio = if total_uploads > 0 {
            redundant_uploads as f64 / total_uploads as f64
        } else {
            0.0
        };

        MemoryEfficiencyMetrics {
            upload_download_ratio,
            avg_transfer_size_bytes: avg_transfer_size,
            avg_transfers_per_frame,
            avg_bytes_per_frame,
            avg_texture_bindings_per_frame,
            estimated_texture_memory_bytes: estimated_texture_memory,
            unique_texture_count: unique_textures.len() as u32,
            redundant_upload_ratio,
            total_frames_analysed: self.history.len() as u64,
        }
    }

    /// Get per-label cumulative bytes transferred.
    pub fn label_bytes(&self) -> &HashMap<String, u64> {
        &self.label_bytes
    }

    /// Get the top N bandwidth consumers across all frames.
    pub fn top_consumers(&self, n: usize) -> Vec<(String, u64)> {
        let mut consumers: Vec<(String, u64)> = self.label_bytes.clone().into_iter().collect();
        consumers.sort_by(|a, b| b.1.cmp(&a.1));
        consumers.truncate(n);
        consumers
    }

    /// Get the configuration.
    pub fn config(&self) -> &BandwidthConfig {
        &self.config
    }

    /// Clear all profiling data.
    pub fn clear(&mut self) {
        self.history.clear();
        self.frame_counter = 0;
        self.total_upload_bytes = 0;
        self.total_download_bytes = 0;
        self.total_texture_bindings = 0;
        self.current_frame = None;
        self.texture_tracker.clear();
        self.label_bytes.clear();
    }

    /// Compute recent upload/download bandwidth in GB/s from frame history.
    fn compute_recent_bandwidth(&self) -> (f32, f32) {
        if self.history.is_empty() {
            return (0.0, 0.0);
        }

        let window = self.history.len().min(30);
        let recent: Vec<&FrameBandwidthStats> = self.history.iter().rev().take(window).collect();

        let total_upload: u64 = recent.iter().map(|f| f.upload_bytes).sum();
        let total_download: u64 = recent.iter().map(|f| f.download_bytes).sum();
        let total_duration: Duration = recent.iter().map(|f| f.frame_duration).sum();

        let secs = total_duration.as_secs_f64();
        if secs <= 0.0 {
            return (0.0, 0.0);
        }

        let upload_gbps = (total_upload as f64 / secs / 1e9) as f32;
        let download_gbps = (total_download as f64 / secs / 1e9) as f32;

        (upload_gbps, download_gbps)
    }
}

/// Helper module for serializing Duration.
mod duration_serde {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bandwidth_config_defaults() {
        let config = BandwidthConfig::default();
        assert_eq!(config.history_size, 120);
        assert_eq!(config.theoretical_bandwidth_gbps, 100.0);
        assert_eq!(config.high_pressure_threshold, 0.6);
        assert_eq!(config.critical_pressure_threshold, 0.85);
        assert_eq!(config.texture_thrash_threshold, 4);
        assert_eq!(config.min_frames_for_suggestions, 30);
    }

    #[test]
    fn test_new_profiler_is_empty() {
        let profiler = MemoryBandwidthProfiler::new(BandwidthConfig::default());
        let stats = profiler.get_stats();
        assert_eq!(stats.upload_bytes, 0);
        assert_eq!(stats.download_bytes, 0);
        assert_eq!(stats.texture_bindings, 0);
        assert_eq!(stats.memory_pressure_score, 0.0);
    }

    #[test]
    fn test_buffer_upload_tracking() {
        let mut profiler = MemoryBandwidthProfiler::new(BandwidthConfig::default());

        profiler.begin_frame();
        profiler.record_buffer_upload("vertex_data", 1024);
        profiler.record_buffer_upload("index_data", 512);
        let frame = profiler.end_frame().unwrap();

        assert_eq!(frame.upload_bytes, 1536);
        assert_eq!(frame.download_bytes, 0);
        assert_eq!(frame.transfer_count, 2);

        let stats = profiler.get_stats();
        assert_eq!(stats.upload_bytes, 1536);
    }

    #[test]
    fn test_buffer_download_tracking() {
        let mut profiler = MemoryBandwidthProfiler::new(BandwidthConfig::default());

        profiler.begin_frame();
        profiler.record_buffer_download("readback", 2048);
        let frame = profiler.end_frame().unwrap();

        assert_eq!(frame.download_bytes, 2048);
        assert_eq!(frame.upload_bytes, 0);

        let stats = profiler.get_stats();
        assert_eq!(stats.download_bytes, 2048);
    }

    #[test]
    fn test_per_frame_transfer_volume() {
        let mut profiler = MemoryBandwidthProfiler::new(BandwidthConfig::default());

        // Frame 1
        profiler.begin_frame();
        profiler.record_buffer_upload("buf_a", 4096);
        profiler.record_buffer_download("buf_b", 1024);
        let f1 = profiler.end_frame().unwrap();
        assert_eq!(f1.upload_bytes + f1.download_bytes, 5120);

        // Frame 2
        profiler.begin_frame();
        profiler.record_buffer_upload("buf_c", 8192);
        let f2 = profiler.end_frame().unwrap();
        assert_eq!(f2.upload_bytes, 8192);

        // Cumulative
        let stats = profiler.get_stats();
        assert_eq!(stats.upload_bytes, 4096 + 8192);
        assert_eq!(stats.download_bytes, 1024);
    }

    #[test]
    fn test_identify_high_bandwidth_operations() {
        let mut profiler = MemoryBandwidthProfiler::new(BandwidthConfig::default());

        profiler.begin_frame();
        profiler.record_buffer_upload("small_buf", 64);
        profiler.record_buffer_upload("huge_buf", 10_000_000);
        profiler.record_buffer_upload("medium_buf", 1000);
        let frame = profiler.end_frame().unwrap();

        // Top consumers should be sorted by bytes descending
        assert!(!frame.top_consumers.is_empty());
        assert_eq!(frame.top_consumers[0].0, "huge_buf");
        assert_eq!(frame.top_consumers[0].1, 10_000_000);

        // Global top consumers
        let top = profiler.top_consumers(2);
        assert_eq!(top[0].0, "huge_buf");
    }

    #[test]
    fn test_texture_binding_tracking() {
        let mut profiler = MemoryBandwidthProfiler::new(BandwidthConfig::default());

        profiler.begin_frame();
        profiler.record_texture_binding("albedo", 1024, 1024, 4, 0);
        profiler.record_texture_binding("normal", 512, 512, 4, 1);
        let frame = profiler.end_frame().unwrap();

        assert_eq!(frame.texture_binding_count, 2);
        assert_eq!(frame.texture_bindings.len(), 2);
        assert_eq!(frame.texture_bindings[0].label, "albedo");
        assert_eq!(frame.texture_bindings[0].bytes_per_pixel, 4);

        let stats = profiler.get_stats();
        assert_eq!(stats.texture_bindings, 2);
    }

    #[test]
    fn test_texture_thrashing_detection() {
        let config = BandwidthConfig {
            texture_thrash_threshold: 3,
            ..Default::default()
        };
        let mut profiler = MemoryBandwidthProfiler::new(config);

        // Simulate alternating texture bindings on slot 0
        for i in 0..6 {
            profiler.begin_frame();
            let label = if i % 2 == 0 { "tex_a" } else { "tex_b" };
            profiler.record_texture_binding(label, 256, 256, 4, 0);
            profiler.end_frame();
        }

        let pressure = profiler.get_memory_pressure();
        assert!(pressure.texture_thrashing_detected);
        assert!(pressure.thrashing_texture_count >= 1);
    }

    #[test]
    fn test_no_thrashing_with_stable_bindings() {
        let config = BandwidthConfig {
            texture_thrash_threshold: 3,
            ..Default::default()
        };
        let mut profiler = MemoryBandwidthProfiler::new(config);

        // Bind the same texture repeatedly — no thrashing
        for _ in 0..6 {
            profiler.begin_frame();
            profiler.record_texture_binding("stable_tex", 256, 256, 4, 0);
            profiler.end_frame();
        }

        let pressure = profiler.get_memory_pressure();
        assert!(!pressure.texture_thrashing_detected);
    }

    #[test]
    fn test_texture_bandwidth_estimation() {
        let mut profiler = MemoryBandwidthProfiler::new(BandwidthConfig::default());

        profiler.begin_frame();
        profiler.record_texture_binding("large_tex", 4096, 4096, 4, 0);
        let frame = profiler.end_frame().unwrap();

        // Verify texture info is captured correctly for bandwidth estimation
        let tex = &frame.texture_bindings[0];
        let estimated_bytes = tex.width as u64 * tex.height as u64 * tex.bytes_per_pixel as u64;
        assert_eq!(estimated_bytes, 4096 * 4096 * 4); // 64 MiB
    }

    #[test]
    fn test_memory_pressure_low() {
        let profiler = MemoryBandwidthProfiler::new(BandwidthConfig::default());
        let pressure = profiler.get_memory_pressure();
        assert_eq!(pressure.level, PressureLevel::Low);
        assert_eq!(pressure.score, 0.0);
    }

    #[test]
    fn test_memory_pressure_score_calculation() {
        let config = BandwidthConfig {
            theoretical_bandwidth_gbps: 100.0,
            ..Default::default()
        };
        let profiler = MemoryBandwidthProfiler::new(config);

        // With no data, pressure should be zero
        let pressure = profiler.get_memory_pressure();
        assert_eq!(pressure.bandwidth_utilization, 0.0);
    }

    #[test]
    fn test_optimization_suggestions_need_min_frames() {
        let config = BandwidthConfig {
            min_frames_for_suggestions: 5,
            ..Default::default()
        };
        let mut profiler = MemoryBandwidthProfiler::new(config);

        // Only 2 frames — should not produce suggestions
        for _ in 0..2 {
            profiler.begin_frame();
            profiler.record_buffer_upload("data", 1_000_000_000);
            profiler.end_frame();
        }
        assert!(profiler.get_optimization_suggestions().is_empty());
    }

    #[test]
    fn test_optimization_suggestions_for_many_small_transfers() {
        let config = BandwidthConfig {
            min_frames_for_suggestions: 2,
            theoretical_bandwidth_gbps: 100.0,
            ..Default::default()
        };
        let mut profiler = MemoryBandwidthProfiler::new(config);

        // Generate frames with many small transfers
        for _ in 0..5 {
            profiler.begin_frame();
            for i in 0..30 {
                profiler.record_buffer_upload(&format!("tiny_{i}"), 100);
            }
            profiler.end_frame();
        }

        let suggestions = profiler.get_optimization_suggestions();
        let has_batch_suggestion = suggestions
            .iter()
            .any(|s| s.category == SuggestionCategory::BufferManagement);
        assert!(has_batch_suggestion);
    }

    #[test]
    fn test_clear_resets_profiler() {
        let mut profiler = MemoryBandwidthProfiler::new(BandwidthConfig::default());

        profiler.begin_frame();
        profiler.record_buffer_upload("data", 4096);
        profiler.end_frame();

        profiler.clear();

        let stats = profiler.get_stats();
        assert_eq!(stats.upload_bytes, 0);
        assert_eq!(stats.download_bytes, 0);
        assert!(profiler.history().is_empty());
    }

    #[test]
    fn test_frame_history_bounded() {
        let config = BandwidthConfig {
            history_size: 3,
            ..Default::default()
        };
        let mut profiler = MemoryBandwidthProfiler::new(config);

        for i in 0..5 {
            profiler.begin_frame();
            profiler.record_buffer_upload("data", (i + 1) * 100);
            profiler.end_frame();
        }

        assert_eq!(profiler.history().len(), 3);
        // Should have the latest 3 frames
        assert_eq!(profiler.history()[0].frame_index, 2);
        assert_eq!(profiler.history()[2].frame_index, 4);
    }

    #[test]
    fn test_end_frame_without_begin_returns_none() {
        let mut profiler = MemoryBandwidthProfiler::new(BandwidthConfig::default());
        assert!(profiler.end_frame().is_none());
    }

    #[test]
    fn test_records_ignored_without_begin_frame() {
        let mut profiler = MemoryBandwidthProfiler::new(BandwidthConfig::default());

        // Recording without begin_frame should not panic
        profiler.record_buffer_upload("orphan", 1024);
        profiler.record_buffer_download("orphan", 512);

        let stats = profiler.get_stats();
        assert_eq!(stats.upload_bytes, 0);
    }

    #[test]
    fn test_bandwidth_stats_serialization() {
        let stats = MemoryBandwidthStats {
            upload_bytes: 1024,
            download_bytes: 512,
            upload_bandwidth_gbps: 10.5,
            download_bandwidth_gbps: 2.3,
            texture_bindings: 5,
            memory_pressure_score: 0.42,
        };

        let json = serde_json::to_string(&stats).unwrap();
        let deserialized: MemoryBandwidthStats = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.upload_bytes, 1024);
        assert_eq!(deserialized.download_bytes, 512);
        assert_eq!(deserialized.texture_bindings, 5);
    }

    #[test]
    fn test_frame_stats_serialization() {
        let stats = FrameBandwidthStats {
            frame_index: 42,
            upload_bytes: 2048,
            download_bytes: 1024,
            transfer_count: 3,
            texture_binding_count: 1,
            estimated_bandwidth_bytes_per_sec: 1e9,
            frame_duration: Duration::from_millis(16),
            transfers: vec![],
            texture_bindings: vec![],
            top_consumers: vec![("test".to_string(), 2048)],
        };

        let json = serde_json::to_string(&stats).unwrap();
        let deserialized: FrameBandwidthStats = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.frame_index, 42);
        assert_eq!(deserialized.upload_bytes, 2048);
    }

    #[test]
    fn test_memory_pressure_status_serialization() {
        let status = MemoryPressureStatus {
            level: PressureLevel::High,
            score: 0.72,
            avg_upload_gbps: 50.0,
            avg_download_gbps: 22.0,
            bandwidth_utilization: 0.72,
            texture_thrashing_detected: true,
            thrashing_texture_count: 2,
        };

        let json = serde_json::to_string(&status).unwrap();
        let deserialized: MemoryPressureStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.level, PressureLevel::High);
        assert!(deserialized.texture_thrashing_detected);
    }

    #[test]
    fn test_optimization_suggestion_serialization() {
        let suggestion = OptimizationSuggestion {
            priority: 3,
            title: "Test".to_string(),
            description: "A test suggestion".to_string(),
            category: SuggestionCategory::ReduceUploads,
        };

        let json = serde_json::to_string(&suggestion).unwrap();
        let deserialized: OptimizationSuggestion = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.priority, 3);
        assert_eq!(deserialized.category, SuggestionCategory::ReduceUploads);
    }

    #[test]
    fn test_texture_bytes_per_pixel_defaults_to_4() {
        let mut profiler = MemoryBandwidthProfiler::new(BandwidthConfig::default());

        profiler.begin_frame();
        profiler.record_texture_binding("tex", 256, 256, 0, 0); // 0 means default
        let frame = profiler.end_frame().unwrap();

        assert_eq!(frame.texture_bindings[0].bytes_per_pixel, 4);
    }

    #[test]
    fn test_efficiency_metrics_empty() {
        let profiler = MemoryBandwidthProfiler::new(BandwidthConfig::default());
        let metrics = profiler.get_efficiency_metrics();
        assert_eq!(metrics.total_frames_analysed, 0);
        assert_eq!(metrics.avg_transfer_size_bytes, 0);
        assert_eq!(metrics.upload_download_ratio, 0.0);
    }

    #[test]
    fn test_efficiency_metrics_upload_only() {
        let mut profiler = MemoryBandwidthProfiler::new(BandwidthConfig::default());

        for _ in 0..5 {
            profiler.begin_frame();
            profiler.record_buffer_upload("verts", 4096);
            profiler.record_buffer_upload("indices", 1024);
            profiler.end_frame();
        }

        let metrics = profiler.get_efficiency_metrics();
        assert_eq!(metrics.total_frames_analysed, 5);
        assert!(metrics.upload_download_ratio.is_infinite()); // no downloads
        assert_eq!(metrics.avg_transfer_size_bytes, (4096 + 1024) / 2); // 2 transfers per frame
        assert!((metrics.avg_transfers_per_frame - 2.0).abs() < 0.01);
        assert!((metrics.avg_bytes_per_frame - 5120.0).abs() < 0.01);
    }

    #[test]
    fn test_efficiency_metrics_upload_download_ratio() {
        let mut profiler = MemoryBandwidthProfiler::new(BandwidthConfig::default());

        profiler.begin_frame();
        profiler.record_buffer_upload("data", 2000);
        profiler.record_buffer_download("readback", 1000);
        profiler.end_frame();

        let metrics = profiler.get_efficiency_metrics();
        assert!((metrics.upload_download_ratio - 2.0).abs() < 0.01);
    }

    #[test]
    fn test_efficiency_metrics_texture_memory() {
        let mut profiler = MemoryBandwidthProfiler::new(BandwidthConfig::default());

        // Two unique textures
        profiler.begin_frame();
        profiler.record_texture_binding("albedo", 1024, 1024, 4, 0);
        profiler.record_texture_binding("normal", 512, 512, 4, 1);
        profiler.end_frame();

        let metrics = profiler.get_efficiency_metrics();
        assert_eq!(metrics.unique_texture_count, 2);
        let expected = 1024 * 1024 * 4 + 512 * 512 * 4;
        assert_eq!(metrics.estimated_texture_memory_bytes, expected);
    }

    #[test]
    fn test_efficiency_metrics_redundant_uploads() {
        let mut profiler = MemoryBandwidthProfiler::new(BandwidthConfig::default());

        // Same label uploaded every frame = redundant
        for _ in 0..5 {
            profiler.begin_frame();
            profiler.record_buffer_upload("static_buf", 4096);
            profiler.end_frame();
        }

        let metrics = profiler.get_efficiency_metrics();
        // Frames 2–5 all upload "static_buf" which was also in the previous frame
        // 4 out of 5 uploads are redundant
        assert!((metrics.redundant_upload_ratio - 0.8).abs() < 0.01);
    }

    #[test]
    fn test_efficiency_metrics_serialization() {
        let metrics = MemoryEfficiencyMetrics {
            upload_download_ratio: 3.5,
            avg_transfer_size_bytes: 8192,
            avg_transfers_per_frame: 4.2,
            avg_bytes_per_frame: 32000.0,
            avg_texture_bindings_per_frame: 2.0,
            estimated_texture_memory_bytes: 16 * 1024 * 1024,
            unique_texture_count: 4,
            redundant_upload_ratio: 0.25,
            total_frames_analysed: 120,
        };

        let json = serde_json::to_string(&metrics).unwrap();
        let deserialized: MemoryEfficiencyMetrics = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.unique_texture_count, 4);
        assert!((deserialized.upload_download_ratio - 3.5).abs() < f64::EPSILON);
    }
}

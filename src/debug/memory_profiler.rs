// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GPU memory profiling and leak detection tools.
//!
//! This module provides real-time GPU memory usage tracking, allocation/deallocation logging,
//! memory leak detection, and resource lifetime visualization.

use crate::error::{GupError, GupResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use wgpu::{Buffer, BufferUsages, Device, Queue};

/// GPU memory profiler for tracking allocations and detecting leaks
#[derive(Debug)]
pub struct GpuMemoryProfiler {
    #[allow(dead_code)]
    device: Device,
    #[allow(dead_code)]
    queue: Queue,
    /// Tracked buffer allocations
    allocations: Arc<Mutex<HashMap<u64, BufferAllocation>>>,
    /// Next allocation ID
    next_allocation_id: Arc<Mutex<u64>>,
    /// Memory usage history for trend analysis
    memory_history: Arc<Mutex<Vec<MemorySnapshot>>>,
    /// Configuration
    config: MemoryProfilerConfig,
    /// Session start time
    session_start: Instant,
}

impl GpuMemoryProfiler {
    /// Create a new GPU memory profiler
    pub fn new(device: &Device, queue: &Queue) -> Self {
        Self::with_config(device, queue, MemoryProfilerConfig::default())
    }

    /// Create a new GPU memory profiler with custom configuration
    pub fn with_config(device: &Device, queue: &Queue, config: MemoryProfilerConfig) -> Self {
        Self {
            device: device.clone(),
            queue: queue.clone(),
            allocations: Arc::new(Mutex::new(HashMap::new())),
            next_allocation_id: Arc::new(Mutex::new(0)),
            memory_history: Arc::new(Mutex::new(Vec::new())),
            config,
            session_start: Instant::now(),
        }
    }

    /// Register a buffer allocation for tracking
    pub fn register_allocation(
        &self,
        _buffer: &Buffer,
        label: Option<&str>,
        size: u64,
        usage: BufferUsages,
    ) -> u64 {
        let mut allocations = self.allocations.lock().unwrap();
        let mut next_id = self.next_allocation_id.lock().unwrap();

        let allocation_id = *next_id;
        *next_id += 1;

        let allocation = BufferAllocation {
            id: allocation_id,
            label: label.map(|s| s.to_string()),
            size,
            usage,
            allocated_at: Instant::now(),
            deallocated_at: None,
            stack_trace: if self.config.capture_stack_traces {
                Some(format!("{:?}", std::backtrace::Backtrace::capture()))
            } else {
                None
            },
        };

        allocations.insert(allocation_id, allocation);

        // Record memory snapshot if enabled
        if self.config.enable_memory_history {
            self.record_memory_snapshot();
        }

        allocation_id
    }

    /// Unregister a buffer allocation (mark as deallocated)
    pub fn unregister_allocation(&self, allocation_id: u64) -> GupResult<()> {
        let mut allocations = self.allocations.lock().unwrap();

        if let Some(allocation) = allocations.get_mut(&allocation_id) {
            if allocation.deallocated_at.is_some() {
                return Err(GupError::invalid_operation(format!(
                    "Allocation {} already deallocated",
                    allocation_id
                )));
            }

            allocation.deallocated_at = Some(Instant::now());

            // Record memory snapshot if enabled
            if self.config.enable_memory_history {
                drop(allocations); // Release lock before recording snapshot
                self.record_memory_snapshot();
            }

            Ok(())
        } else {
            Err(GupError::resource_error(format!(
                "Allocation {} not found",
                allocation_id
            )))
        }
    }

    /// Get current total GPU memory usage in bytes
    pub fn total_memory_usage(&self) -> u64 {
        let allocations = self.allocations.lock().unwrap();
        allocations
            .values()
            .filter(|a| a.deallocated_at.is_none())
            .map(|a| a.size)
            .sum()
    }

    /// Get current number of active allocations
    pub fn active_allocation_count(&self) -> usize {
        let allocations = self.allocations.lock().unwrap();
        allocations
            .values()
            .filter(|a| a.deallocated_at.is_none())
            .count()
    }

    /// Detect memory leaks (allocations that haven't been deallocated for a long time)
    pub fn detect_memory_leaks(&self) -> Vec<MemoryLeak> {
        let allocations = self.allocations.lock().unwrap();
        let now = Instant::now();
        let threshold = self.config.leak_detection_threshold;

        allocations
            .values()
            .filter(|a| a.deallocated_at.is_none())
            .filter(|a| now.duration_since(a.allocated_at) > threshold)
            .map(|a| MemoryLeak {
                allocation_id: a.id,
                label: a.label.clone(),
                size: a.size,
                age: now.duration_since(a.allocated_at),
                stack_trace: a.stack_trace.clone(),
            })
            .collect()
    }

    /// Get detailed memory usage report
    pub fn get_memory_report(&self) -> MemoryReport {
        let allocations = self.allocations.lock().unwrap();
        let now = Instant::now();

        let active_allocations: Vec<_> = allocations
            .values()
            .filter(|a| a.deallocated_at.is_none())
            .collect();

        let total_allocated = allocations.values().map(|a| a.size).sum();
        let total_active = active_allocations.iter().map(|a| a.size).sum();
        let total_deallocated = allocations
            .values()
            .filter(|a| a.deallocated_at.is_some())
            .map(|a| a.size)
            .sum();

        // Group by buffer usage
        let mut usage_breakdown: HashMap<String, u64> = HashMap::new();
        for allocation in &active_allocations {
            let usage_str = format!("{:?}", allocation.usage);
            *usage_breakdown.entry(usage_str).or_insert(0) += allocation.size;
        }

        // Calculate allocation rate (allocations per second)
        let session_duration = now.duration_since(self.session_start);
        let allocation_rate = if session_duration.as_secs() > 0 {
            allocations.len() as f32 / session_duration.as_secs_f32()
        } else {
            0.0
        };

        // Find largest allocations
        let mut largest_allocations = active_allocations
            .iter()
            .map(|a| AllocationInfo {
                id: a.id,
                label: a.label.clone(),
                size: a.size,
                age: now.duration_since(a.allocated_at),
            })
            .collect::<Vec<_>>();
        largest_allocations.sort_by_key(|a| std::cmp::Reverse(a.size));
        largest_allocations.truncate(10); // Top 10

        MemoryReport {
            timestamp: chrono::Utc::now(),
            session_duration,
            total_allocations: allocations.len(),
            active_allocations: active_allocations.len(),
            total_memory_allocated: total_allocated,
            total_memory_active: total_active,
            total_memory_deallocated: total_deallocated,
            allocation_rate,
            usage_breakdown,
            largest_allocations,
            detected_leaks: self.detect_memory_leaks(),
        }
    }

    /// Record a memory snapshot for trend analysis
    fn record_memory_snapshot(&self) {
        let mut history = self.memory_history.lock().unwrap();

        let snapshot = MemorySnapshot {
            timestamp: Instant::now(),
            total_memory: self.total_memory_usage(),
            active_allocations: self.active_allocation_count(),
        };

        history.push(snapshot);

        // Keep only recent history to prevent unbounded growth
        if history.len() > self.config.max_history_size {
            history.remove(0);
        }
    }

    /// Get memory usage history
    pub fn get_memory_history(&self) -> Vec<MemorySnapshot> {
        let history = self.memory_history.lock().unwrap();
        history.clone()
    }

    /// Get memory usage trend (increasing, stable, decreasing)
    pub fn get_memory_trend(&self) -> MemoryTrend {
        let history = self.memory_history.lock().unwrap();

        if history.len() < 2 {
            return MemoryTrend::Stable;
        }

        // Compare recent average to older average
        let recent_window = history.len().saturating_sub(10).max(history.len() / 2);
        let recent_avg: u64 = history[recent_window..]
            .iter()
            .map(|s| s.total_memory)
            .sum::<u64>()
            / (history.len() - recent_window) as u64;

        let older_avg: u64 = history[..recent_window]
            .iter()
            .map(|s| s.total_memory)
            .sum::<u64>()
            / recent_window as u64;

        let change_percent = if older_avg > 0 {
            ((recent_avg as f64 - older_avg as f64) / older_avg as f64) * 100.0
        } else {
            0.0
        };

        if change_percent > 20.0 {
            MemoryTrend::Increasing
        } else if change_percent < -20.0 {
            MemoryTrend::Decreasing
        } else {
            MemoryTrend::Stable
        }
    }

    /// Clear all tracking data (useful for starting fresh profiling session)
    pub fn clear(&self) {
        let mut allocations = self.allocations.lock().unwrap();
        allocations.clear();

        let mut next_id = self.next_allocation_id.lock().unwrap();
        *next_id = 0;

        let mut history = self.memory_history.lock().unwrap();
        history.clear();
    }

    /// Export memory report to JSON file
    pub fn export_report(&self, output_path: &str) -> GupResult<()> {
        let report = self.get_memory_report();
        let json = serde_json::to_string_pretty(&report).map_err(|e| {
            GupError::validation_error(format!("Failed to serialize memory report: {e}"))
        })?;

        std::fs::write(output_path, json)
            .map_err(|e| GupError::resource_error(format!("Failed to write memory report: {e}")))?;

        Ok(())
    }
}

/// Configuration for memory profiler
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryProfilerConfig {
    /// Capture stack traces for allocations (expensive but useful for leak detection)
    pub capture_stack_traces: bool,
    /// Enable memory usage history tracking
    pub enable_memory_history: bool,
    /// Maximum number of history entries to keep
    pub max_history_size: usize,
    /// Duration after which an allocation is considered a potential leak
    pub leak_detection_threshold: Duration,
}

impl Default for MemoryProfilerConfig {
    fn default() -> Self {
        Self {
            capture_stack_traces: cfg!(debug_assertions),
            enable_memory_history: true,
            max_history_size: 1000,
            leak_detection_threshold: Duration::from_secs(300), // 5 minutes
        }
    }
}

/// Information about a tracked buffer allocation
#[derive(Debug, Clone)]
pub struct BufferAllocation {
    pub id: u64,
    pub label: Option<String>,
    pub size: u64,
    pub usage: BufferUsages,
    pub allocated_at: Instant,
    pub deallocated_at: Option<Instant>,
    pub stack_trace: Option<String>,
}

/// Memory snapshot at a point in time
#[derive(Debug, Clone)]
pub struct MemorySnapshot {
    pub timestamp: Instant,
    pub total_memory: u64,
    pub active_allocations: usize,
}

/// Serializable version of MemorySnapshot for export
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySnapshotSerialized {
    pub timestamp_secs: f64,
    pub total_memory: u64,
    pub active_allocations: usize,
}

impl From<&MemorySnapshot> for MemorySnapshotSerialized {
    fn from(snapshot: &MemorySnapshot) -> Self {
        Self {
            timestamp_secs: snapshot.timestamp.elapsed().as_secs_f64(),
            total_memory: snapshot.total_memory,
            active_allocations: snapshot.active_allocations,
        }
    }
}

/// Memory trend direction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MemoryTrend {
    Increasing,
    Stable,
    Decreasing,
}

/// Detected memory leak
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryLeak {
    pub allocation_id: u64,
    pub label: Option<String>,
    pub size: u64,
    #[serde(with = "duration_serde")]
    pub age: Duration,
    pub stack_trace: Option<String>,
}

/// Detailed memory usage report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryReport {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    #[serde(with = "duration_serde")]
    pub session_duration: Duration,
    pub total_allocations: usize,
    pub active_allocations: usize,
    pub total_memory_allocated: u64,
    pub total_memory_active: u64,
    pub total_memory_deallocated: u64,
    pub allocation_rate: f32,
    pub usage_breakdown: HashMap<String, u64>,
    pub largest_allocations: Vec<AllocationInfo>,
    pub detected_leaks: Vec<MemoryLeak>,
}

/// Information about a single allocation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllocationInfo {
    pub id: u64,
    pub label: Option<String>,
    pub size: u64,
    #[serde(with = "duration_serde")]
    pub age: Duration,
}

/// Helper module for serializing Duration
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
    fn test_memory_profiler_config() {
        let config = MemoryProfilerConfig::default();
        assert_eq!(config.capture_stack_traces, cfg!(debug_assertions));
        assert!(config.enable_memory_history);
        assert_eq!(config.max_history_size, 1000);
        assert_eq!(config.leak_detection_threshold, Duration::from_secs(300));
    }

    #[test]
    fn test_memory_trend() {
        assert_eq!(MemoryTrend::Increasing, MemoryTrend::Increasing);
        assert_ne!(MemoryTrend::Increasing, MemoryTrend::Stable);
    }

    #[test]
    fn test_memory_snapshot() {
        let snapshot = MemorySnapshot {
            timestamp: Instant::now(),
            total_memory: 1024 * 1024,
            active_allocations: 10,
        };

        assert_eq!(snapshot.total_memory, 1024 * 1024);
        assert_eq!(snapshot.active_allocations, 10);
    }

    #[test]
    fn test_allocation_info() {
        let info = AllocationInfo {
            id: 42,
            label: Some("test_buffer".to_string()),
            size: 4096,
            age: Duration::from_secs(10),
        };

        assert_eq!(info.id, 42);
        assert_eq!(info.label, Some("test_buffer".to_string()));
        assert_eq!(info.size, 4096);
        assert_eq!(info.age, Duration::from_secs(10));
    }

    #[test]
    fn test_memory_leak() {
        let leak = MemoryLeak {
            allocation_id: 123,
            label: Some("leaked_buffer".to_string()),
            size: 1024 * 1024,
            age: Duration::from_secs(600),
            stack_trace: None,
        };

        assert_eq!(leak.allocation_id, 123);
        assert_eq!(leak.size, 1024 * 1024);
        assert!(leak.age > Duration::from_secs(300)); // Over threshold
    }
}

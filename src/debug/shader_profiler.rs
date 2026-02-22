// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Shader execution profiling and performance analysis tools.
//!
//! This module provides utilities for profiling GPU shader execution times, monitoring
//! GPU utilization, and detecting performance regressions in compute and render pipelines.

use crate::error::{GupError, GupResult};
use crate::performance::TimestampQueryManager;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use wgpu::*;

/// GPU shader profiler for performance analysis
#[derive(Debug)]
pub struct ShaderProfiler {
    device: Device,
    queue: Queue,
    /// Query sets for GPU timing measurements
    #[allow(dead_code)] // Kept for backward compatibility
    timestamp_query_sets: HashMap<String, QuerySet>,
    /// GPU timestamp query manager (if supported)
    timestamp_manager: Option<TimestampQueryManager>,
    /// Profiling session history
    profiling_sessions: Vec<ProfilingSession>,
    /// Performance baselines for regression detection
    performance_baselines: HashMap<String, PerformanceBaseline>,
    /// Current profiling session (if active)
    current_session: Option<ProfilingSession>,
    /// Whether timestamp queries are supported
    supports_timestamps: bool,
}

impl ShaderProfiler {
    /// Create a new shader profiler
    pub fn new(device: &Device, queue: &Queue) -> Self {
        let supports_timestamps = device.features().contains(Features::TIMESTAMP_QUERY);

        let timestamp_manager = if supports_timestamps {
            TimestampQueryManager::new(device, 64).ok()
        } else {
            None
        };

        Self {
            device: device.clone(),
            queue: queue.clone(),
            timestamp_query_sets: HashMap::new(),
            timestamp_manager,
            profiling_sessions: Vec::new(),
            performance_baselines: HashMap::new(),
            current_session: None,
            supports_timestamps,
        }
    }

    /// Check if GPU timestamp queries are supported
    pub fn supports_timestamps(&self) -> bool {
        self.supports_timestamps
    }

    /// Profile a compute shader execution
    pub async fn profile_compute(
        &mut self,
        pipeline: &ComputePipeline,
        bind_group: &BindGroup,
        dispatch_size: (u32, u32, u32),
    ) -> GupResult<ShaderExecutionStats> {
        let start_time = Instant::now();
        let mut used_hardware_timestamps = false;

        // Try to use GPU timestamps if available
        let gpu_duration = if let Some(ref timestamp_manager) = self.timestamp_manager {
            match self
                .profile_compute_with_timestamps(
                    pipeline,
                    bind_group,
                    dispatch_size,
                    timestamp_manager,
                )
                .await
            {
                Ok(duration) => {
                    used_hardware_timestamps = true;
                    Some(duration)
                }
                Err(_) => None,
            }
        } else {
            None
        };

        // Fallback to CPU timing if GPU timestamps unavailable or failed
        let total_duration = if let Some(gpu_dur) = gpu_duration {
            gpu_dur
        } else {
            // Create command encoder with timing
            let mut encoder = self
                .device
                .create_command_encoder(&CommandEncoderDescriptor {
                    label: Some("shader_profiler_compute"),
                });

            // Execute compute pass
            {
                let mut compute_pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                    label: Some("profiled_compute_pass"),
                    timestamp_writes: None,
                });

                compute_pass.set_pipeline(pipeline);
                compute_pass.set_bind_group(0, bind_group, &[]);
                compute_pass.dispatch_workgroups(dispatch_size.0, dispatch_size.1, dispatch_size.2);
            }

            // Submit and wait for completion
            let submission_index = self.queue.submit([encoder.finish()]);
            let _ = self
                .device
                .poll(PollType::WaitForSubmissionIndex(submission_index));

            start_time.elapsed()
        };

        // Calculate approximate GPU utilization (simplified)
        let workgroup_count = dispatch_size.0 * dispatch_size.1 * dispatch_size.2;
        let approximate_gpu_utilization =
            self.estimate_gpu_utilization(workgroup_count, total_duration);

        Ok(ShaderExecutionStats {
            duration: total_duration,
            gpu_utilization_percent: approximate_gpu_utilization,
            dispatch_size,
            workgroup_count,
            memory_bandwidth_gbps: 0.0, // TODO: Implement memory bandwidth calculation
            instructions_per_second: 0.0, // TODO: Implement instruction counting
            timestamp: chrono::Utc::now(),
            metadata: HashMap::new(),
            used_hardware_timestamps,
        })
    }

    /// Profile a compute shader execution with hardware timestamp queries
    async fn profile_compute_with_timestamps(
        &self,
        pipeline: &ComputePipeline,
        bind_group: &BindGroup,
        dispatch_size: (u32, u32, u32),
        timestamp_manager: &TimestampQueryManager,
    ) -> GupResult<Duration> {
        let query_set = timestamp_manager
            .query_set()
            .ok_or_else(|| GupError::invalid_operation("Query set not available".to_string()))?;

        // Create command encoder
        let mut encoder = self
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("shader_profiler_timestamp"),
            });

        // Execute compute pass with timestamp queries
        {
            let mut compute_pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some("profiled_compute_timestamp"),
                timestamp_writes: Some(wgpu::ComputePassTimestampWrites {
                    query_set,
                    beginning_of_pass_write_index: Some(0),
                    end_of_pass_write_index: Some(1),
                }),
            });

            compute_pass.set_pipeline(pipeline);
            compute_pass.set_bind_group(0, bind_group, &[]);
            compute_pass.dispatch_workgroups(dispatch_size.0, dispatch_size.1, dispatch_size.2);
        }

        // Resolve and copy timestamp queries
        timestamp_manager.resolve_queries(&mut encoder, 0..2);
        timestamp_manager.copy_to_readback(&mut encoder, 2);

        // Submit commands
        let submission_index = self.queue.submit([encoder.finish()]);
        let _ = self
            .device
            .poll(PollType::WaitForSubmissionIndex(submission_index));

        // Read timestamp results
        let timestamps = timestamp_manager.read_timestamps(2).await?;

        if timestamps.len() < 2 {
            return Err(GupError::validation_error(
                "Insufficient timestamp results".to_string(),
            ));
        }

        // Calculate duration from timestamp difference
        let start = timestamps[0];
        let end = timestamps[1];
        let duration_ticks = end.saturating_sub(start);
        let duration = timestamp_manager.ticks_to_duration(duration_ticks);

        Ok(duration)
    }

    /// Profile multiple compute shader executions and return batch statistics
    pub async fn profile_compute_batch(
        &mut self,
        executions: Vec<ComputeExecution>,
    ) -> GupResult<BatchExecutionStats> {
        let mut individual_stats = Vec::new();
        let batch_start = Instant::now();

        for execution in executions {
            let stats = self
                .profile_compute(
                    &execution.pipeline,
                    &execution.bind_group,
                    execution.dispatch_size,
                )
                .await?;
            individual_stats.push(stats);
        }

        let batch_duration = batch_start.elapsed();

        let average_duration = Duration::from_nanos(
            (individual_stats
                .iter()
                .map(|s| s.duration.as_nanos())
                .sum::<u128>()
                / individual_stats.len() as u128)
                .try_into()
                .unwrap_or(0),
        );
        let max_duration = individual_stats
            .iter()
            .map(|s| s.duration)
            .max()
            .unwrap_or(Duration::ZERO);
        let min_duration = individual_stats
            .iter()
            .map(|s| s.duration)
            .min()
            .unwrap_or(Duration::ZERO);

        Ok(BatchExecutionStats {
            total_duration: batch_duration,
            execution_count: individual_stats.len(),
            individual_stats,
            average_duration,
            max_duration,
            min_duration,
        })
    }

    /// Start a profiling session for continuous monitoring
    pub fn start_profiling_session(&mut self, session_name: &str) -> GupResult<()> {
        if self.current_session.is_some() {
            return Err(GupError::invalid_operation(
                "Profiling session already active".to_string(),
            ));
        }

        self.current_session = Some(ProfilingSession {
            name: session_name.to_string(),
            start_time: Instant::now(),
            executions: Vec::new(),
            total_gpu_time: Duration::ZERO,
            peak_memory_usage: 0,
        });

        Ok(())
    }

    /// End the current profiling session and return results
    pub fn end_profiling_session(&mut self) -> GupResult<ProfilingSessionResults> {
        let session = self.current_session.take().ok_or_else(|| {
            GupError::invalid_operation("No active profiling session".to_string())
        })?;

        let total_duration = session.start_time.elapsed();

        let results = ProfilingSessionResults {
            session_name: session.name.clone(),
            total_duration,
            execution_count: session.executions.len(),
            total_gpu_time: session.total_gpu_time,
            peak_memory_usage: session.peak_memory_usage,
            average_gpu_utilization: session
                .executions
                .iter()
                .map(|e| e.gpu_utilization_percent)
                .sum::<f32>()
                / session.executions.len() as f32,
            executions: session.executions.clone(),
        };

        self.profiling_sessions.push(session);
        Ok(results)
    }

    /// Record execution stats in current profiling session
    pub fn record_execution(&mut self, stats: ShaderExecutionStats) -> GupResult<()> {
        let session = self.current_session.as_mut().ok_or_else(|| {
            GupError::invalid_operation("No active profiling session".to_string())
        })?;

        session.total_gpu_time += stats.duration;
        session.executions.push(stats);

        Ok(())
    }

    /// Set performance baseline for regression detection
    pub fn set_performance_baseline(&mut self, name: &str, baseline: PerformanceBaseline) {
        self.performance_baselines
            .insert(name.to_string(), baseline);
    }

    /// Check for performance regression against baseline
    pub fn check_performance_regression(
        &self,
        baseline_name: &str,
        current_stats: &ShaderExecutionStats,
    ) -> Option<PerformanceRegression> {
        let baseline = self.performance_baselines.get(baseline_name)?;

        let duration_increase =
            current_stats.duration.as_secs_f32() / baseline.expected_duration.as_secs_f32();
        let utilization_decrease =
            baseline.expected_gpu_utilization - current_stats.gpu_utilization_percent;

        let mut regressions = Vec::new();

        if duration_increase > baseline.regression_threshold {
            regressions.push(format!(
                "Execution time increased by {:.1}% ({:.2}ms -> {:.2}ms)",
                (duration_increase - 1.0) * 100.0,
                baseline.expected_duration.as_secs_f32() * 1000.0,
                current_stats.duration.as_secs_f32() * 1000.0
            ));
        }

        if utilization_decrease > 10.0 {
            regressions.push(format!(
                "GPU utilization decreased by {:.1}% ({:.1}% -> {:.1}%)",
                utilization_decrease,
                baseline.expected_gpu_utilization,
                current_stats.gpu_utilization_percent
            ));
        }

        if regressions.is_empty() {
            None
        } else {
            Some(PerformanceRegression {
                baseline_name: baseline_name.to_string(),
                severity: if duration_increase > baseline.regression_threshold * 1.5 {
                    RegressionSeverity::Severe
                } else {
                    RegressionSeverity::Moderate
                },
                issues: regressions,
                current_stats: current_stats.clone(),
                baseline_stats: baseline.clone().into(),
            })
        }
    }

    /// Get profiling statistics summary
    pub fn get_profiling_summary(&self) -> ProfilingSummary {
        let total_executions: usize = self
            .profiling_sessions
            .iter()
            .map(|s| s.executions.len())
            .sum();

        let total_gpu_time: Duration = self
            .profiling_sessions
            .iter()
            .map(|s| s.total_gpu_time)
            .sum();

        let all_executions: Vec<&ShaderExecutionStats> = self
            .profiling_sessions
            .iter()
            .flat_map(|s| s.executions.iter())
            .collect();

        let average_duration = if !all_executions.is_empty() {
            Duration::from_nanos(
                (all_executions
                    .iter()
                    .map(|e| e.duration.as_nanos())
                    .sum::<u128>()
                    / all_executions.len() as u128)
                    .try_into()
                    .unwrap_or(0),
            )
        } else {
            Duration::ZERO
        };

        let average_gpu_utilization = if !all_executions.is_empty() {
            all_executions
                .iter()
                .map(|e| e.gpu_utilization_percent)
                .sum::<f32>()
                / all_executions.len() as f32
        } else {
            0.0
        };

        ProfilingSummary {
            total_sessions: self.profiling_sessions.len(),
            total_executions,
            total_gpu_time,
            average_duration,
            average_gpu_utilization,
            baseline_count: self.performance_baselines.len(),
        }
    }

    /// Clear profiling history and baselines
    pub fn clear_profiling_history(&mut self) {
        self.profiling_sessions.clear();
        self.performance_baselines.clear();
        self.current_session = None;
    }

    /// Estimate GPU utilization based on workgroup count and execution time
    fn estimate_gpu_utilization(&self, workgroup_count: u32, duration: Duration) -> f32 {
        // This is a simplified estimation
        // In practice, GPU utilization depends on many factors including:
        // - GPU architecture and compute unit count
        // - Memory bandwidth utilization
        // - Shader complexity and ALU utilization
        // - Memory access patterns

        // For now, provide a rough estimate based on workgroup density
        let workgroups_per_ms = workgroup_count as f32 / duration.as_secs_f32() / 1000.0;

        // Assume ideal GPU can handle ~10000 workgroups per ms (very rough estimate)
        let ideal_workgroups_per_ms = 10000.0;

        (workgroups_per_ms / ideal_workgroups_per_ms * 100.0).min(100.0)
    }
}

/// Statistics for a single shader execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShaderExecutionStats {
    pub duration: Duration,
    pub gpu_utilization_percent: f32,
    pub dispatch_size: (u32, u32, u32),
    pub workgroup_count: u32,
    pub memory_bandwidth_gbps: f32,
    pub instructions_per_second: f32,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub metadata: HashMap<String, String>,
    /// Whether this measurement used hardware timestamp queries
    pub used_hardware_timestamps: bool,
}

impl ShaderExecutionStats {
    pub fn with_metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }

    pub fn with_hardware_timestamps(mut self, used: bool) -> Self {
        self.used_hardware_timestamps = used;
        self
    }
}

/// Compute execution configuration for batch profiling
#[derive(Debug)]
pub struct ComputeExecution {
    pub pipeline: ComputePipeline,
    pub bind_group: BindGroup,
    pub dispatch_size: (u32, u32, u32),
}

/// Statistics for batch execution profiling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchExecutionStats {
    pub total_duration: Duration,
    pub execution_count: usize,
    pub individual_stats: Vec<ShaderExecutionStats>,
    pub average_duration: Duration,
    pub max_duration: Duration,
    pub min_duration: Duration,
}

/// Active profiling session for continuous monitoring
#[derive(Debug, Clone)]
pub struct ProfilingSession {
    pub name: String,
    pub start_time: Instant,
    pub executions: Vec<ShaderExecutionStats>,
    pub total_gpu_time: Duration,
    pub peak_memory_usage: u64,
}

/// Results from a completed profiling session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfilingSessionResults {
    pub session_name: String,
    pub total_duration: Duration,
    pub execution_count: usize,
    pub total_gpu_time: Duration,
    pub peak_memory_usage: u64,
    pub average_gpu_utilization: f32,
    pub executions: Vec<ShaderExecutionStats>,
}

/// Performance baseline for regression detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceBaseline {
    pub name: String,
    pub expected_duration: Duration,
    pub expected_gpu_utilization: f32,
    pub expected_memory_usage: u64,
    pub regression_threshold: f32, // Factor above baseline that triggers regression (e.g., 1.2 = 20% increase)
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl PerformanceBaseline {
    pub fn new(name: &str, expected_duration: Duration, expected_gpu_utilization: f32) -> Self {
        Self {
            name: name.to_string(),
            expected_duration,
            expected_gpu_utilization,
            expected_memory_usage: 0,
            regression_threshold: 1.2, // 20% increase by default
            created_at: chrono::Utc::now(),
        }
    }

    pub fn with_memory_usage(mut self, memory_usage: u64) -> Self {
        self.expected_memory_usage = memory_usage;
        self
    }

    pub fn with_threshold(mut self, threshold: f32) -> Self {
        self.regression_threshold = threshold;
        self
    }
}

impl From<PerformanceBaseline> for ShaderExecutionStats {
    fn from(baseline: PerformanceBaseline) -> Self {
        Self {
            duration: baseline.expected_duration,
            gpu_utilization_percent: baseline.expected_gpu_utilization,
            dispatch_size: (0, 0, 0), // Unknown for baseline
            workgroup_count: 0,
            memory_bandwidth_gbps: 0.0,
            instructions_per_second: 0.0,
            timestamp: baseline.created_at,
            metadata: HashMap::new(),
            used_hardware_timestamps: false,
        }
    }
}

/// Performance regression detection result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceRegression {
    pub baseline_name: String,
    pub severity: RegressionSeverity,
    pub issues: Vec<String>,
    pub current_stats: ShaderExecutionStats,
    pub baseline_stats: ShaderExecutionStats,
}

/// Severity level for performance regressions
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum RegressionSeverity {
    Minor,
    Moderate,
    Severe,
}

/// Summary of all profiling activities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfilingSummary {
    pub total_sessions: usize,
    pub total_executions: usize,
    pub total_gpu_time: Duration,
    pub average_duration: Duration,
    pub average_gpu_utilization: f32,
    pub baseline_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_performance_baseline_creation() {
        let baseline = PerformanceBaseline::new("test_baseline", Duration::from_millis(10), 85.5)
            .with_memory_usage(1024 * 1024)
            .with_threshold(1.3);

        assert_eq!(baseline.name, "test_baseline");
        assert_eq!(baseline.expected_duration, Duration::from_millis(10));
        assert_eq!(baseline.expected_gpu_utilization, 85.5);
        assert_eq!(baseline.expected_memory_usage, 1024 * 1024);
        assert_eq!(baseline.regression_threshold, 1.3);
    }

    #[test]
    fn test_shader_execution_stats() {
        let stats = ShaderExecutionStats {
            duration: Duration::from_millis(5),
            gpu_utilization_percent: 75.0,
            dispatch_size: (64, 64, 1),
            workgroup_count: 4096,
            memory_bandwidth_gbps: 500.0,
            instructions_per_second: 1_000_000.0,
            timestamp: chrono::Utc::now(),
            metadata: HashMap::new(),
            used_hardware_timestamps: false,
        }
        .with_metadata("test_key", "test_value");

        assert_eq!(stats.duration, Duration::from_millis(5));
        assert_eq!(stats.gpu_utilization_percent, 75.0);
        assert_eq!(stats.dispatch_size, (64, 64, 1));
        assert_eq!(stats.workgroup_count, 4096);
        assert!(!stats.used_hardware_timestamps);
        assert_eq!(
            stats.metadata.get("test_key"),
            Some(&"test_value".to_string())
        );
    }

    #[test]
    fn test_batch_execution_stats() {
        let individual_stats = vec![
            ShaderExecutionStats {
                duration: Duration::from_millis(5),
                gpu_utilization_percent: 75.0,
                dispatch_size: (64, 64, 1),
                workgroup_count: 4096,
                memory_bandwidth_gbps: 0.0,
                instructions_per_second: 0.0,
                timestamp: chrono::Utc::now(),
                metadata: HashMap::new(),
                used_hardware_timestamps: false,
            },
            ShaderExecutionStats {
                duration: Duration::from_millis(7),
                gpu_utilization_percent: 80.0,
                dispatch_size: (64, 64, 1),
                workgroup_count: 4096,
                memory_bandwidth_gbps: 0.0,
                instructions_per_second: 0.0,
                timestamp: chrono::Utc::now(),
                metadata: HashMap::new(),
                used_hardware_timestamps: false,
            },
        ];

        let batch_stats = BatchExecutionStats {
            total_duration: Duration::from_millis(15),
            execution_count: 2,
            individual_stats: individual_stats.clone(),
            average_duration: Duration::from_millis(6),
            max_duration: Duration::from_millis(7),
            min_duration: Duration::from_millis(5),
        };

        assert_eq!(batch_stats.execution_count, 2);
        assert_eq!(batch_stats.max_duration, Duration::from_millis(7));
        assert_eq!(batch_stats.min_duration, Duration::from_millis(5));
        assert_eq!(batch_stats.individual_stats.len(), 2);
    }

    #[test]
    fn test_performance_regression() {
        let current_stats = ShaderExecutionStats {
            duration: Duration::from_millis(12), // 20% increase from baseline
            gpu_utilization_percent: 70.0,       // 15% decrease from baseline
            dispatch_size: (64, 64, 1),
            workgroup_count: 4096,
            memory_bandwidth_gbps: 0.0,
            instructions_per_second: 0.0,
            timestamp: chrono::Utc::now(),
            metadata: HashMap::new(),
            used_hardware_timestamps: false,
        };

        let baseline_stats = ShaderExecutionStats {
            duration: Duration::from_millis(10),
            gpu_utilization_percent: 85.0,
            dispatch_size: (64, 64, 1),
            workgroup_count: 4096,
            memory_bandwidth_gbps: 0.0,
            instructions_per_second: 0.0,
            timestamp: chrono::Utc::now(),
            metadata: HashMap::new(),
            used_hardware_timestamps: false,
        };

        let regression = PerformanceRegression {
            baseline_name: "test_baseline".to_string(),
            severity: RegressionSeverity::Moderate,
            issues: vec![
                "Execution time increased by 20%".to_string(),
                "GPU utilization decreased by 15%".to_string(),
            ],
            current_stats,
            baseline_stats,
        };

        assert_eq!(regression.baseline_name, "test_baseline");
        assert_eq!(regression.issues.len(), 2);
        match regression.severity {
            RegressionSeverity::Moderate => {}
            _ => panic!("Expected Moderate severity"),
        }
    }

    #[test]
    fn test_profiling_summary() {
        let summary = ProfilingSummary {
            total_sessions: 5,
            total_executions: 100,
            total_gpu_time: Duration::from_secs(30),
            average_duration: Duration::from_millis(300),
            average_gpu_utilization: 82.5,
            baseline_count: 3,
        };

        assert_eq!(summary.total_sessions, 5);
        assert_eq!(summary.total_executions, 100);
        assert_eq!(summary.total_gpu_time, Duration::from_secs(30));
        assert_eq!(summary.average_gpu_utilization, 82.5);
        assert_eq!(summary.baseline_count, 3);
    }
}

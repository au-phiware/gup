// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GPU debugging and profiling tools for high-performance development workflows.
//!
//! This module provides comprehensive GPU debugging capabilities including buffer inspection,
//! shader profiling, memory layout validation, and performance regression detection.
//!
//! # Features
//!
//! - **Buffer Inspector**: Easy buffer content dumping and inspection with staging buffers
//! - **Memory Layout Validation**: Compare Rust vs WGSL struct layouts for compatibility
//! - **Shader Profiling**: Profile compute shader execution times and GPU utilization
//! - **Performance Monitoring**: Track GPU resource usage and detect performance regressions
//! - **Cross-Platform Analysis**: Compare behavior between native and WebAssembly targets
//!
//! # Examples
//!
//! ```rust,ignore
//! use gup::debug::{GpuBufferInspector, ShaderProfiler};
//! use gup::GupContext;
//! use std::sync::Arc;
//!
//! async fn debug_gpu_buffers() -> Result<(), Box<dyn std::error::Error>> {
//!     let context = Arc::new(GupContext::new().await?);
//!
//!     // Inspect buffer contents
//!     let inspector = GpuBufferInspector::new(&context.device, &context.queue);
//!     inspector.dump_buffer::<ElementData>(&element_buffer, "elements.json").await?;
//!
//!     // Validate memory layout
//!     inspector.validate_layout::<ElementData>("ElementData")?;
//!
//!     // Profile shader execution
//!     let profiler = ShaderProfiler::new(&context.device, &context.queue);
//!     let stats = profiler.profile_compute(&pipeline, &bind_group, (1024, 1, 1)).await?;
//!     println!("Execution time: {:?}", stats.duration);
//!
//!     Ok(())
//! }
//! ```

use crate::error::{GupError, GupResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use wgpu::{Device, Queue};

pub mod buffer_inspector;
pub mod ci_performance;
pub mod layout_validator;
pub mod memory_profiler;
pub mod resource_graph;
pub mod shader_profiler;
pub mod visualization;
pub mod web_dashboard;

pub use buffer_inspector::*;
// Export CI performance types explicitly to avoid conflicts
pub use ci_performance::{
    BaselineComparison, BaselineStorage, CiConfig, CiPerformanceRunner, PerformanceReport,
    PerformanceTest, PerformanceTestSuite, RegressionSeverity as CiRegressionSeverity, TestResult,
};
pub use layout_validator::*;
pub use memory_profiler::*;
// Export resource graph types with explicit names to avoid conflicts with error::ResourceId
pub use resource_graph::{
    ResourceGraph, ResourceGraphReport, ResourceId as DebugResourceId,
    ResourceNode, ResourceState as DebugResourceState, ResourceType as DebugResourceType,
};
pub use shader_profiler::*;
pub use visualization::*;
pub use web_dashboard::*;

/// Debug configuration for GPU debugging tools
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugConfig {
    /// Enable buffer content inspection
    pub enable_buffer_inspection: bool,
    /// Enable memory layout validation
    pub enable_layout_validation: bool,
    /// Enable shader profiling
    pub enable_shader_profiling: bool,
    /// Enable performance regression detection
    pub enable_performance_monitoring: bool,
    /// Output directory for debug files
    pub debug_output_dir: String,
    /// Maximum buffer size to inspect (in bytes)
    pub max_buffer_inspect_size: u64,
    /// Performance baseline thresholds
    pub performance_thresholds: PerformanceThresholds,
}

impl Default for DebugConfig {
    fn default() -> Self {
        Self {
            enable_buffer_inspection: cfg!(debug_assertions),
            enable_layout_validation: cfg!(debug_assertions),
            enable_shader_profiling: cfg!(debug_assertions),
            enable_performance_monitoring: cfg!(debug_assertions),
            debug_output_dir: "debug_output".to_string(),
            max_buffer_inspect_size: 1024 * 1024 * 10, // 10MB limit
            performance_thresholds: PerformanceThresholds::default(),
        }
    }
}

/// Performance threshold configuration for regression detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceThresholds {
    /// Maximum acceptable frame time in milliseconds
    pub max_frame_time_ms: f32,
    /// Maximum acceptable query time in microseconds
    pub max_query_time_us: f32,
    /// Maximum acceptable memory usage in bytes
    pub max_memory_usage_bytes: u64,
    /// Percentage increase that triggers regression warning
    pub regression_threshold_percent: f32,
}

impl Default for PerformanceThresholds {
    fn default() -> Self {
        Self {
            max_frame_time_ms: 16.67,                   // 60 FPS target
            max_query_time_us: 1000.0,                  // 1ms target for interaction queries
            max_memory_usage_bytes: 1024 * 1024 * 1024, // 1GB limit
            regression_threshold_percent: 20.0,         // 20% increase triggers warning
        }
    }
}

/// Unified GPU debugging context that manages all debugging tools
#[derive(Debug)]
pub struct GpuDebugContext {
    /// Buffer inspector for content analysis
    pub buffer_inspector: GpuBufferInspector,
    /// Memory layout validator
    pub layout_validator: MemoryLayoutValidator,
    /// Memory profiler for allocation tracking and leak detection
    pub memory_profiler: GpuMemoryProfiler,
    /// Shader profiler for performance analysis
    pub shader_profiler: ShaderProfiler,
    /// Resource dependency graph for relationship analysis
    pub resource_graph: ResourceGraph,
    /// Debug configuration
    pub config: DebugConfig,
    /// Performance history for regression detection
    performance_history: Vec<PerformanceSnapshot>,
}

impl GpuDebugContext {
    /// Create a new GPU debug context with default configuration
    pub fn new(device: &Device, queue: &Queue) -> Self {
        Self::with_config(device, queue, DebugConfig::default())
    }

    /// Create a new GPU debug context with custom configuration
    pub fn with_config(device: &Device, queue: &Queue, config: DebugConfig) -> Self {
        Self {
            buffer_inspector: GpuBufferInspector::new(device, queue),
            layout_validator: MemoryLayoutValidator::new(),
            memory_profiler: GpuMemoryProfiler::new(device, queue),
            shader_profiler: ShaderProfiler::new(device, queue),
            resource_graph: ResourceGraph::new(),
            config,
            performance_history: Vec::new(),
        }
    }

    /// Record a performance snapshot for regression detection
    pub fn record_performance(&mut self, snapshot: PerformanceSnapshot) {
        self.performance_history.push(snapshot.clone());

        // Keep only recent history to prevent unbounded growth
        const MAX_HISTORY_SIZE: usize = 1000;
        if self.performance_history.len() > MAX_HISTORY_SIZE {
            self.performance_history.remove(0);
        }

        // Check for performance regressions
        if let Some(regression) = self.detect_regression(&snapshot) {
            eprintln!("⚠️  Performance regression detected: {regression}");
        }
    }

    /// Detect performance regressions compared to recent history
    fn detect_regression(&self, current: &PerformanceSnapshot) -> Option<String> {
        if self.performance_history.len() < 10 {
            return None; // Need sufficient history for comparison
        }

        // Calculate moving average of recent performance
        let recent_samples = &self.performance_history[self.performance_history.len() - 10..];
        let avg_frame_time: f32 =
            recent_samples.iter().map(|s| s.frame_time_ms).sum::<f32>() / 10.0;
        let avg_memory: u64 = recent_samples
            .iter()
            .map(|s| s.memory_usage_bytes)
            .sum::<u64>()
            / 10;

        let mut regressions = Vec::new();

        // Check frame time regression
        let frame_time_increase = (current.frame_time_ms - avg_frame_time) / avg_frame_time * 100.0;
        if frame_time_increase
            > self
                .config
                .performance_thresholds
                .regression_threshold_percent
        {
            regressions.push(format!(
                "Frame time increased by {:.1}% ({:.2}ms -> {:.2}ms)",
                frame_time_increase, avg_frame_time, current.frame_time_ms
            ));
        }

        // Check memory usage regression
        let memory_increase = if avg_memory > 0 {
            (current.memory_usage_bytes as f64 - avg_memory as f64) / avg_memory as f64 * 100.0
        } else {
            0.0
        };
        if memory_increase
            > self
                .config
                .performance_thresholds
                .regression_threshold_percent as f64
        {
            regressions.push(format!(
                "Memory usage increased by {:.1}% ({} -> {} bytes)",
                memory_increase, avg_memory, current.memory_usage_bytes
            ));
        }

        if regressions.is_empty() {
            None
        } else {
            Some(regressions.join("; "))
        }
    }

    /// Get performance statistics summary
    pub fn get_performance_summary(&self) -> PerformanceSummary {
        if self.performance_history.is_empty() {
            return PerformanceSummary::default();
        }

        let frame_times: Vec<f32> = self
            .performance_history
            .iter()
            .map(|s| s.frame_time_ms)
            .collect();
        let memory_usage: Vec<u64> = self
            .performance_history
            .iter()
            .map(|s| s.memory_usage_bytes)
            .collect();

        let avg_frame_time = frame_times.iter().sum::<f32>() / frame_times.len() as f32;
        let min_frame_time = frame_times.iter().fold(f32::INFINITY, |a, &b| a.min(b));
        let max_frame_time = frame_times.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));

        let avg_memory = memory_usage.iter().sum::<u64>() / memory_usage.len() as u64;
        let min_memory = *memory_usage.iter().min().unwrap_or(&0);
        let max_memory = *memory_usage.iter().max().unwrap_or(&0);

        PerformanceSummary {
            sample_count: self.performance_history.len(),
            avg_frame_time_ms: avg_frame_time,
            min_frame_time_ms: min_frame_time,
            max_frame_time_ms: max_frame_time,
            avg_memory_usage_bytes: avg_memory,
            min_memory_usage_bytes: min_memory,
            max_memory_usage_bytes: max_memory,
            fps: if avg_frame_time > 0.0 {
                1000.0 / avg_frame_time
            } else {
                0.0
            },
        }
    }

    /// Clear performance history
    pub fn clear_performance_history(&mut self) {
        self.performance_history.clear();
    }

    /// Create an interactive visualizer for debug data
    ///
    /// This creates a `GpuDebugVisualizer` that can generate GPU-accelerated
    /// interactive visualizations of performance data, memory usage, and buffer contents.
    ///
    /// # Arguments
    ///
    /// * `context` - The render context to use for visualizations
    ///
    /// # Returns
    ///
    /// Returns a `GpuDebugVisualizer` configured for this debug context
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let render_context = Arc::new(RenderContext::new().await?);
    /// let mut debug_context = GpuDebugContext::new(&device, &queue);
    ///
    /// // Create visualizer
    /// let visualizer = debug_context.create_visualizer(render_context);
    ///
    /// // Visualize performance trends
    /// let chart = visualizer.visualize_performance_trends(&debug_context.performance_history).await?;
    /// ```
    pub fn create_visualizer(
        &self,
        context: std::sync::Arc<crate::RenderContext>,
    ) -> GpuDebugVisualizer {
        GpuDebugVisualizer::new(context)
    }

    /// Get access to performance history for visualization
    pub fn performance_history(&self) -> &[PerformanceSnapshot] {
        &self.performance_history
    }

    /// Export debug report with all collected data
    pub async fn export_debug_report(&self, output_path: &str) -> GupResult<()> {
        let summary = self.get_performance_summary();
        let memory_report = self.memory_profiler.get_memory_report();
        let resource_report = self.resource_graph.generate_report();
        let report = DebugReport {
            timestamp: chrono::Utc::now(),
            config: self.config.clone(),
            performance_summary: summary,
            performance_history: self.performance_history.clone(),
            layout_validation_results: self.layout_validator.get_validation_history(),
            memory_report: Some(memory_report),
            resource_graph_report: Some(resource_report),
        };

        let json = serde_json::to_string_pretty(&report).map_err(|e| {
            GupError::validation_error(format!("Failed to serialize debug report: {e}"))
        })?;

        std::fs::write(output_path, json)
            .map_err(|e| GupError::resource_error(format!("Failed to write debug report: {e}")))?;

        Ok(())
    }
}

/// Performance snapshot for a single measurement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceSnapshot {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub frame_time_ms: f32,
    pub memory_usage_bytes: u64,
    pub gpu_utilization_percent: f32,
    pub query_time_us: f32,
    pub metadata: HashMap<String, String>,
}

impl PerformanceSnapshot {
    pub fn new(frame_time_ms: f32, memory_usage_bytes: u64) -> Self {
        Self {
            timestamp: chrono::Utc::now(),
            frame_time_ms,
            memory_usage_bytes,
            gpu_utilization_percent: 0.0,
            query_time_us: 0.0,
            metadata: HashMap::new(),
        }
    }

    pub fn with_gpu_utilization(mut self, utilization_percent: f32) -> Self {
        self.gpu_utilization_percent = utilization_percent;
        self
    }

    pub fn with_query_time(mut self, query_time_us: f32) -> Self {
        self.query_time_us = query_time_us;
        self
    }

    pub fn with_metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }
}

/// Summary of performance statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PerformanceSummary {
    pub sample_count: usize,
    pub avg_frame_time_ms: f32,
    pub min_frame_time_ms: f32,
    pub max_frame_time_ms: f32,
    pub avg_memory_usage_bytes: u64,
    pub min_memory_usage_bytes: u64,
    pub max_memory_usage_bytes: u64,
    pub fps: f32,
}

/// Complete debug report with all collected data
#[derive(Debug, Serialize, Deserialize)]
pub struct DebugReport {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub config: DebugConfig,
    pub performance_summary: PerformanceSummary,
    pub performance_history: Vec<PerformanceSnapshot>,
    pub layout_validation_results: Vec<LayoutValidationResult>,
    pub memory_report: Option<MemoryReport>,
    pub resource_graph_report: Option<ResourceGraphReport>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_debug_config_default() {
        let config = DebugConfig::default();
        assert_eq!(config.enable_buffer_inspection, cfg!(debug_assertions));
        assert_eq!(config.debug_output_dir, "debug_output");
        assert_eq!(config.max_buffer_inspect_size, 1024 * 1024 * 10);
    }

    #[test]
    fn test_performance_thresholds() {
        let thresholds = PerformanceThresholds::default();
        assert_eq!(thresholds.max_frame_time_ms, 16.67);
        assert_eq!(thresholds.max_query_time_us, 1000.0);
        assert_eq!(thresholds.regression_threshold_percent, 20.0);
    }

    #[test]
    fn test_performance_snapshot() {
        let snapshot = PerformanceSnapshot::new(16.67, 1024 * 1024)
            .with_gpu_utilization(85.5)
            .with_query_time(500.0)
            .with_metadata("test", "value");

        assert_eq!(snapshot.frame_time_ms, 16.67);
        assert_eq!(snapshot.memory_usage_bytes, 1024 * 1024);
        assert_eq!(snapshot.gpu_utilization_percent, 85.5);
        assert_eq!(snapshot.query_time_us, 500.0);
        assert_eq!(snapshot.metadata.get("test"), Some(&"value".to_string()));
    }

    #[test]
    fn test_performance_summary_calculation() {
        // This test would be implemented with a mock GpuDebugContext
        // For now, just test the default
        let summary = PerformanceSummary::default();
        assert_eq!(summary.sample_count, 0);
        assert_eq!(summary.fps, 0.0);
    }

    #[tokio::test]
    async fn test_debug_report_serialization() {
        let config = DebugConfig::default();
        let summary = PerformanceSummary::default();
        let report = DebugReport {
            timestamp: chrono::Utc::now(),
            config,
            performance_summary: summary,
            performance_history: Vec::new(),
            layout_validation_results: Vec::new(),
            memory_report: None,
            resource_graph_report: None,
        };

        let json = serde_json::to_string_pretty(&report);
        assert!(json.is_ok());
    }
}

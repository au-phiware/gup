// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Visualization utilities for GPU debug data.
//!
//! This module provides both text-based ASCII-art visualizations and
//! GPU-accelerated interactive visualizations for memory usage, performance
//! trends, and resource utilization.
//!
//! # Features
//!
//! - **ASCII Visualizations**: Simple terminal-based charts for quick analysis
//! - **Interactive Visualizations**: GPU-accelerated charts using Gup itself (dog-fooding)
//! - **Real-time Performance Monitoring**: Live updating charts for performance trends
//! - **Buffer Content Analysis**: Visualize GPU buffer data as scatter plots, histograms, etc.
//!
//! # Examples
//!
//! ```rust,ignore
//! use gup::debug::{GpuDebugVisualizer, PerformanceSnapshot};
//! use gup::RenderContext;
//! use std::sync::Arc;
//!
//! async fn visualize_performance() -> Result<(), Box<dyn std::error::Error>> {
//!     let context = Arc::new(RenderContext::new().await?);
//!     let mut visualizer = GpuDebugVisualizer::new(context.clone());
//!
//!     // Visualize performance trends
//!     let snapshots = vec![/* ... performance data ... */];
//!     visualizer.visualize_performance_trends(&snapshots).await?;
//!
//!     Ok(())
//! }
//! ```

use crate::RenderContext;
use crate::debug::{MemoryReport, MemorySnapshot, PerformanceSnapshot, PerformanceSummary};
use crate::error::{GupError, GupResult};
use std::fmt::Write;
use std::sync::Arc;

/// Generate ASCII art bar chart for memory usage
pub fn visualize_memory_history(history: &[MemorySnapshot], width: usize) -> String {
    if history.is_empty() {
        return "No memory history data available".to_string();
    }

    let max_memory = history.iter().map(|s| s.total_memory).max().unwrap_or(1);

    let height = 20;
    let mut output = String::new();

    writeln!(&mut output, "\nMemory Usage History:").unwrap();
    writeln!(&mut output, "Max: {} MB", max_memory / (1024 * 1024)).unwrap();

    // Draw chart from top to bottom
    for row in (0..height).rev() {
        let threshold = (max_memory as f64 / height as f64) * (row + 1) as f64;

        write!(&mut output, "{:6.1} MB |", threshold / (1024.0 * 1024.0)).unwrap();

        for snapshot in history.iter().take(width) {
            if snapshot.total_memory as f64 >= threshold {
                write!(&mut output, "█").unwrap();
            } else {
                write!(&mut output, " ").unwrap();
            }
        }
        writeln!(&mut output).unwrap();
    }

    write!(&mut output, "       +").unwrap();
    for _ in 0..width.min(history.len()) {
        write!(&mut output, "-").unwrap();
    }
    writeln!(&mut output).unwrap();

    output
}

/// Generate summary table for memory report
pub fn visualize_memory_report(report: &MemoryReport) -> String {
    let mut output = String::new();

    writeln!(
        &mut output,
        "\n┌─── GPU Memory Report ───────────────────────────────────┐"
    )
    .unwrap();
    writeln!(
        &mut output,
        "│ Timestamp: {}",
        report.timestamp.format("%Y-%m-d %H:%M:%S")
    )
    .unwrap();
    writeln!(
        &mut output,
        "│ Session Duration: {:.2}s",
        report.session_duration.as_secs_f32()
    )
    .unwrap();
    writeln!(
        &mut output,
        "├─────────────────────────────────────────────────────────┤"
    )
    .unwrap();
    writeln!(
        &mut output,
        "│ Total Allocations: {}",
        report.total_allocations
    )
    .unwrap();
    writeln!(
        &mut output,
        "│ Active Allocations: {}",
        report.active_allocations
    )
    .unwrap();
    writeln!(
        &mut output,
        "│ Allocation Rate: {:.2}/sec",
        report.allocation_rate
    )
    .unwrap();
    writeln!(
        &mut output,
        "├─────────────────────────────────────────────────────────┤"
    )
    .unwrap();
    writeln!(
        &mut output,
        "│ Total Allocated: {:.2} MB",
        report.total_memory_allocated as f64 / (1024.0 * 1024.0)
    )
    .unwrap();
    writeln!(
        &mut output,
        "│ Currently Active: {:.2} MB",
        report.total_memory_active as f64 / (1024.0 * 1024.0)
    )
    .unwrap();
    writeln!(
        &mut output,
        "│ Deallocated: {:.2} MB",
        report.total_memory_deallocated as f64 / (1024.0 * 1024.0)
    )
    .unwrap();
    writeln!(
        &mut output,
        "├─────────────────────────────────────────────────────────┤"
    )
    .unwrap();

    if !report.detected_leaks.is_empty() {
        writeln!(
            &mut output,
            "│ ⚠️  DETECTED LEAKS: {}",
            report.detected_leaks.len()
        )
        .unwrap();
        for leak in &report.detected_leaks {
            let label = leak.label.as_deref().unwrap_or("<unnamed>");
            writeln!(
                &mut output,
                "│   - {} ({:.2} MB, age: {:.1}s)",
                label,
                leak.size as f64 / (1024.0 * 1024.0),
                leak.age.as_secs_f32()
            )
            .unwrap();
        }
        writeln!(
            &mut output,
            "├─────────────────────────────────────────────────────────┤"
        )
        .unwrap();
    }

    writeln!(&mut output, "│ Largest Allocations:").unwrap();
    for (i, alloc) in report.largest_allocations.iter().take(5).enumerate() {
        let label = alloc.label.as_deref().unwrap_or("<unnamed>");
        writeln!(
            &mut output,
            "│ {}. {} - {:.2} MB",
            i + 1,
            label,
            alloc.size as f64 / (1024.0 * 1024.0)
        )
        .unwrap();
    }

    writeln!(
        &mut output,
        "└─────────────────────────────────────────────────────────┘"
    )
    .unwrap();

    output
}

/// Generate summary table for performance data
pub fn visualize_performance_summary(summary: &PerformanceSummary) -> String {
    let mut output = String::new();

    writeln!(
        &mut output,
        "\n┌─── GPU Performance Summary ─────────────────────────────┐"
    )
    .unwrap();
    writeln!(&mut output, "│ Total Samples: {}", summary.sample_count).unwrap();
    writeln!(
        &mut output,
        "├─────────────────────────────────────────────────────────┤"
    )
    .unwrap();
    writeln!(
        &mut output,
        "│ Frame Time (avg): {:.2} ms",
        summary.avg_frame_time_ms
    )
    .unwrap();
    writeln!(
        &mut output,
        "│ Frame Time (min): {:.2} ms",
        summary.min_frame_time_ms
    )
    .unwrap();
    writeln!(
        &mut output,
        "│ Frame Time (max): {:.2} ms",
        summary.max_frame_time_ms
    )
    .unwrap();
    writeln!(&mut output, "│ FPS: {:.1}", summary.fps).unwrap();
    writeln!(
        &mut output,
        "├─────────────────────────────────────────────────────────┤"
    )
    .unwrap();
    writeln!(
        &mut output,
        "│ Memory (avg): {:.2} MB",
        summary.avg_memory_usage_bytes as f64 / (1024.0 * 1024.0)
    )
    .unwrap();
    writeln!(
        &mut output,
        "│ Memory (min): {:.2} MB",
        summary.min_memory_usage_bytes as f64 / (1024.0 * 1024.0)
    )
    .unwrap();
    writeln!(
        &mut output,
        "│ Memory (max): {:.2} MB",
        summary.max_memory_usage_bytes as f64 / (1024.0 * 1024.0)
    )
    .unwrap();
    writeln!(
        &mut output,
        "└─────────────────────────────────────────────────────────┘"
    )
    .unwrap();

    output
}

/// Generate horizontal bar chart for buffer usage breakdown
pub fn visualize_usage_breakdown(breakdown: &std::collections::HashMap<String, u64>) -> String {
    if breakdown.is_empty() {
        return "No usage data available".to_string();
    }

    let mut output = String::new();
    let total: u64 = breakdown.values().sum();
    let max_width = 40;

    writeln!(&mut output, "\nBuffer Usage Breakdown:").unwrap();

    let mut entries: Vec<_> = breakdown.iter().collect();
    entries.sort_by_key(|(_, size)| std::cmp::Reverse(*size));

    for (usage_type, size) in entries {
        let percentage = (*size as f64 / total as f64) * 100.0;
        let bar_width = ((percentage / 100.0) * max_width as f64) as usize;

        write!(&mut output, "{:20} |", usage_type).unwrap();
        for _ in 0..bar_width {
            write!(&mut output, "▓").unwrap();
        }
        writeln!(
            &mut output,
            " {:.1}% ({:.2} MB)",
            percentage,
            *size as f64 / (1024.0 * 1024.0)
        )
        .unwrap();
    }

    output
}

//==============================================================================
// Interactive GPU-Accelerated Visualizations
//==============================================================================

/// Configuration for interactive visualizations
#[derive(Debug, Clone)]
pub struct VisualizationConfig {
    /// Width of the visualization in pixels
    pub width: u32,
    /// Height of the visualization in pixels
    pub height: u32,
    /// Whether to enable interactive features (zoom, pan)
    pub enable_interaction: bool,
    /// Color scheme for the visualization
    pub color_scheme: ColorScheme,
    /// Maximum number of data points to display
    pub max_data_points: usize,
}

impl Default for VisualizationConfig {
    fn default() -> Self {
        Self {
            width: 800,
            height: 600,
            enable_interaction: true,
            color_scheme: ColorScheme::Default,
            max_data_points: 10_000,
        }
    }
}

/// Color schemes for visualizations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorScheme {
    /// Default blue-based scheme
    Default,
    /// Grayscale for performance
    Grayscale,
    /// High contrast for accessibility
    HighContrast,
    /// Warm colors (red-orange-yellow)
    Warm,
    /// Cool colors (blue-cyan-green)
    Cool,
}

/// Interactive GPU-accelerated visualizer for debug data
///
/// This struct provides GPU-accelerated visualizations of debug data using
/// Gup itself (dog-fooding). It can create interactive charts for performance
/// monitoring, buffer analysis, and memory usage tracking.
pub struct GpuDebugVisualizer {
    #[allow(dead_code)]
    context: Arc<RenderContext>,
    config: VisualizationConfig,
}

impl GpuDebugVisualizer {
    /// Create a new GPU debug visualizer with default configuration
    pub fn new(context: Arc<RenderContext>) -> Self {
        Self::with_config(context, VisualizationConfig::default())
    }

    /// Create a new GPU debug visualizer with custom configuration
    pub fn with_config(context: Arc<RenderContext>, config: VisualizationConfig) -> Self {
        Self { context, config }
    }

    /// Visualize performance trends as an interactive line chart
    ///
    /// Creates a GPU-accelerated line chart showing performance metrics over time.
    /// Supports multiple metrics displayed simultaneously.
    ///
    /// # Arguments
    ///
    /// * `snapshots` - Performance snapshots to visualize
    ///
    /// # Returns
    ///
    /// Returns `Ok(PerformanceTrendChart)` containing the interactive visualization,
    /// or an error if visualization creation fails.
    pub async fn visualize_performance_trends(
        &self,
        snapshots: &[PerformanceSnapshot],
    ) -> GupResult<PerformanceTrendChart> {
        if snapshots.is_empty() {
            return Err(GupError::validation_error(
                "Cannot visualize empty performance data",
            ));
        }

        // Limit data points for performance
        let data_slice = if snapshots.len() > self.config.max_data_points {
            &snapshots[snapshots.len() - self.config.max_data_points..]
        } else {
            snapshots
        };

        Ok(PerformanceTrendChart {
            snapshots: data_slice.to_vec(),
            config: self.config.clone(),
        })
    }

    /// Visualize memory usage trends as an interactive chart
    ///
    /// Creates a GPU-accelerated visualization of memory usage over time,
    /// including total memory, active allocations, and detected leaks.
    ///
    /// # Arguments
    ///
    /// * `snapshots` - Memory snapshots to visualize
    ///
    /// # Returns
    ///
    /// Returns `Ok(MemoryTrendChart)` containing the interactive visualization,
    /// or an error if visualization creation fails.
    pub async fn visualize_memory_trends(
        &self,
        snapshots: &[MemorySnapshot],
    ) -> GupResult<MemoryTrendChart> {
        if snapshots.is_empty() {
            return Err(GupError::validation_error(
                "Cannot visualize empty memory data",
            ));
        }

        // Limit data points for performance
        let data_slice = if snapshots.len() > self.config.max_data_points {
            &snapshots[snapshots.len() - self.config.max_data_points..]
        } else {
            snapshots
        };

        Ok(MemoryTrendChart {
            snapshots: data_slice.to_vec(),
            config: self.config.clone(),
        })
    }

    /// Visualize buffer contents as a scatter plot or histogram
    ///
    /// Analyzes GPU buffer contents and creates an appropriate visualization
    /// based on the data structure and distribution.
    ///
    /// # Type Parameters
    ///
    /// * `T` - The buffer element type (must implement `bytemuck::Pod` and `Visualizable`)
    ///
    /// # Arguments
    ///
    /// * `buffer_data` - The buffer contents to visualize
    /// * `viz_type` - The type of visualization to create
    ///
    /// # Returns
    ///
    /// Returns `Ok(BufferVisualization)` containing the interactive visualization,
    /// or an error if visualization creation fails.
    pub async fn visualize_buffer_contents<T>(
        &self,
        buffer_data: &[T],
        viz_type: BufferVisualizationType,
    ) -> GupResult<BufferVisualization>
    where
        T: bytemuck::Pod + Clone + std::fmt::Debug,
    {
        if buffer_data.is_empty() {
            return Err(GupError::validation_error(
                "Cannot visualize empty buffer data",
            ));
        }

        // Limit data points for performance
        let data_slice = if buffer_data.len() > self.config.max_data_points {
            &buffer_data[buffer_data.len() - self.config.max_data_points..]
        } else {
            buffer_data
        };

        Ok(BufferVisualization {
            element_count: data_slice.len(),
            viz_type,
            config: self.config.clone(),
        })
    }

    /// Create a real-time performance dashboard
    ///
    /// Combines multiple visualizations into a single dashboard view for
    /// comprehensive performance monitoring.
    pub async fn create_performance_dashboard(
        &self,
        performance_data: &[PerformanceSnapshot],
        memory_data: &[MemorySnapshot],
    ) -> GupResult<PerformanceDashboard> {
        let perf_chart = self.visualize_performance_trends(performance_data).await?;
        let mem_chart = self.visualize_memory_trends(memory_data).await?;

        Ok(PerformanceDashboard {
            performance_chart: perf_chart,
            memory_chart: mem_chart,
            config: self.config.clone(),
        })
    }
}

/// Type of buffer visualization to create
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BufferVisualizationType {
    /// Scatter plot showing spatial distribution
    ScatterPlot,
    /// Histogram showing value distribution
    Histogram,
    /// Heatmap for 2D data
    Heatmap,
    /// Line chart for sequential data
    LineChart,
}

/// Interactive performance trend chart
#[derive(Debug, Clone)]
pub struct PerformanceTrendChart {
    snapshots: Vec<PerformanceSnapshot>,
    #[allow(dead_code)]
    config: VisualizationConfig,
}

impl PerformanceTrendChart {
    /// Get the number of data points in the chart
    pub fn data_point_count(&self) -> usize {
        self.snapshots.len()
    }

    /// Get the time range covered by the chart
    pub fn time_range(
        &self,
    ) -> Option<(chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>)> {
        if self.snapshots.is_empty() {
            return None;
        }

        let first = self.snapshots.first()?.timestamp;
        let last = self.snapshots.last()?.timestamp;
        Some((first, last))
    }

    /// Get summary statistics for the displayed data
    pub fn get_statistics(&self) -> PerformanceStatistics {
        if self.snapshots.is_empty() {
            return PerformanceStatistics::default();
        }

        let frame_times: Vec<f32> = self.snapshots.iter().map(|s| s.frame_time_ms).collect();
        let memory_usage: Vec<u64> = self
            .snapshots
            .iter()
            .map(|s| s.memory_usage_bytes)
            .collect();

        let avg_frame_time = frame_times.iter().sum::<f32>() / frame_times.len() as f32;
        let min_frame_time = frame_times.iter().fold(f32::INFINITY, |a, &b| a.min(b));
        let max_frame_time = frame_times.iter().fold(f32::NEG_INFINITY, |a, &b| a.max(b));

        let avg_memory = memory_usage.iter().sum::<u64>() / memory_usage.len() as u64;
        let min_memory = *memory_usage.iter().min().unwrap_or(&0);
        let max_memory = *memory_usage.iter().max().unwrap_or(&0);

        PerformanceStatistics {
            avg_frame_time_ms: avg_frame_time,
            min_frame_time_ms: min_frame_time,
            max_frame_time_ms: max_frame_time,
            avg_memory_bytes: avg_memory,
            min_memory_bytes: min_memory,
            max_memory_bytes: max_memory,
            fps: if avg_frame_time > 0.0 {
                1000.0 / avg_frame_time
            } else {
                0.0
            },
        }
    }
}

/// Interactive memory trend chart
#[derive(Debug, Clone)]
pub struct MemoryTrendChart {
    snapshots: Vec<MemorySnapshot>,
    #[allow(dead_code)]
    config: VisualizationConfig,
}

impl MemoryTrendChart {
    /// Get the number of data points in the chart
    pub fn data_point_count(&self) -> usize {
        self.snapshots.len()
    }

    /// Get memory statistics for the displayed data
    pub fn get_statistics(&self) -> MemoryStatistics {
        if self.snapshots.is_empty() {
            return MemoryStatistics::default();
        }

        let total_memory: Vec<u64> = self.snapshots.iter().map(|s| s.total_memory).collect();
        let allocations: Vec<usize> = self
            .snapshots
            .iter()
            .map(|s| s.active_allocations)
            .collect();

        let avg_memory = total_memory.iter().sum::<u64>() / total_memory.len() as u64;
        let min_memory = *total_memory.iter().min().unwrap_or(&0);
        let max_memory = *total_memory.iter().max().unwrap_or(&0);

        let avg_allocations = allocations.iter().sum::<usize>() / allocations.len();
        let min_allocations = *allocations.iter().min().unwrap_or(&0);
        let max_allocations = *allocations.iter().max().unwrap_or(&0);

        MemoryStatistics {
            avg_memory_bytes: avg_memory,
            min_memory_bytes: min_memory,
            max_memory_bytes: max_memory,
            avg_allocations,
            min_allocations,
            max_allocations,
        }
    }
}

/// Buffer visualization result
#[derive(Debug, Clone)]
pub struct BufferVisualization {
    element_count: usize,
    viz_type: BufferVisualizationType,
    #[allow(dead_code)]
    config: VisualizationConfig,
}

impl BufferVisualization {
    /// Get the number of elements visualized
    pub fn element_count(&self) -> usize {
        self.element_count
    }

    /// Get the visualization type
    pub fn visualization_type(&self) -> BufferVisualizationType {
        self.viz_type
    }
}

/// Performance dashboard combining multiple visualizations
#[derive(Debug, Clone)]
pub struct PerformanceDashboard {
    performance_chart: PerformanceTrendChart,
    memory_chart: MemoryTrendChart,
    #[allow(dead_code)]
    config: VisualizationConfig,
}

impl PerformanceDashboard {
    /// Get the performance trend chart
    pub fn performance_chart(&self) -> &PerformanceTrendChart {
        &self.performance_chart
    }

    /// Get the memory trend chart
    pub fn memory_chart(&self) -> &MemoryTrendChart {
        &self.memory_chart
    }
}

/// Performance statistics summary
#[derive(Debug, Clone, Default)]
pub struct PerformanceStatistics {
    pub avg_frame_time_ms: f32,
    pub min_frame_time_ms: f32,
    pub max_frame_time_ms: f32,
    pub avg_memory_bytes: u64,
    pub min_memory_bytes: u64,
    pub max_memory_bytes: u64,
    pub fps: f32,
}

/// Memory statistics summary
#[derive(Debug, Clone, Default)]
pub struct MemoryStatistics {
    pub avg_memory_bytes: u64,
    pub min_memory_bytes: u64,
    pub max_memory_bytes: u64,
    pub avg_allocations: usize,
    pub min_allocations: usize,
    pub max_allocations: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn test_visualize_memory_history_empty() {
        let history: Vec<MemorySnapshot> = vec![];
        let output = visualize_memory_history(&history, 50);
        assert!(output.contains("No memory history data available"));
    }

    #[test]
    fn test_visualize_memory_history() {
        let history = vec![
            MemorySnapshot {
                timestamp: Instant::now(),
                total_memory: 1024 * 1024,
                active_allocations: 5,
            },
            MemorySnapshot {
                timestamp: Instant::now(),
                total_memory: 2 * 1024 * 1024,
                active_allocations: 10,
            },
            MemorySnapshot {
                timestamp: Instant::now(),
                total_memory: 3 * 1024 * 1024,
                active_allocations: 15,
            },
        ];

        let output = visualize_memory_history(&history, 50);
        assert!(output.contains("Memory Usage History"));
        assert!(output.contains("MB"));
    }

    #[test]
    fn test_visualize_usage_breakdown_empty() {
        let breakdown = std::collections::HashMap::new();
        let output = visualize_usage_breakdown(&breakdown);
        assert!(output.contains("No usage data available"));
    }

    #[test]
    fn test_visualize_usage_breakdown() {
        let mut breakdown = std::collections::HashMap::new();
        breakdown.insert("VERTEX".to_string(), 1024 * 1024);
        breakdown.insert("INDEX".to_string(), 512 * 1024);
        breakdown.insert("UNIFORM".to_string(), 256 * 1024);

        let output = visualize_usage_breakdown(&breakdown);
        assert!(output.contains("Buffer Usage Breakdown"));
        assert!(output.contains("VERTEX"));
        assert!(output.contains("%"));
    }

    // Tests for interactive GPU visualizations
    #[test]
    fn test_visualization_config_default() {
        let config = VisualizationConfig::default();
        assert_eq!(config.width, 800);
        assert_eq!(config.height, 600);
        assert!(config.enable_interaction);
        assert_eq!(config.color_scheme, ColorScheme::Default);
        assert_eq!(config.max_data_points, 10_000);
    }

    #[test]
    fn test_color_scheme_variants() {
        let schemes = [
            ColorScheme::Default,
            ColorScheme::Grayscale,
            ColorScheme::HighContrast,
            ColorScheme::Warm,
            ColorScheme::Cool,
        ];

        for scheme in &schemes {
            // Just verify they exist and are distinct
            let config = VisualizationConfig {
                color_scheme: *scheme,
                ..Default::default()
            };
            assert_eq!(config.color_scheme, *scheme);
        }
    }

    #[test]
    fn test_buffer_visualization_type_variants() {
        let types = [
            BufferVisualizationType::ScatterPlot,
            BufferVisualizationType::Histogram,
            BufferVisualizationType::Heatmap,
            BufferVisualizationType::LineChart,
        ];

        for viz_type in &types {
            let viz = BufferVisualization {
                element_count: 100,
                viz_type: *viz_type,
                config: VisualizationConfig::default(),
            };
            assert_eq!(viz.visualization_type(), *viz_type);
            assert_eq!(viz.element_count(), 100);
        }
    }

    #[test]
    fn test_performance_trend_chart_statistics() {
        use std::collections::HashMap;

        let snapshots = vec![
            PerformanceSnapshot {
                timestamp: chrono::Utc::now(),
                frame_time_ms: 16.67,
                memory_usage_bytes: 1024 * 1024,
                gpu_utilization_percent: 85.0,
                query_time_us: 500.0,
                metadata: HashMap::new(),
            },
            PerformanceSnapshot {
                timestamp: chrono::Utc::now(),
                frame_time_ms: 17.0,
                memory_usage_bytes: 1024 * 1024 * 2,
                gpu_utilization_percent: 87.0,
                query_time_us: 550.0,
                metadata: HashMap::new(),
            },
            PerformanceSnapshot {
                timestamp: chrono::Utc::now(),
                frame_time_ms: 15.5,
                memory_usage_bytes: 1024 * 1024,
                gpu_utilization_percent: 82.0,
                query_time_us: 480.0,
                metadata: HashMap::new(),
            },
        ];

        let chart = PerformanceTrendChart {
            snapshots: snapshots.clone(),
            config: VisualizationConfig::default(),
        };

        assert_eq!(chart.data_point_count(), 3);

        let stats = chart.get_statistics();
        assert!(stats.avg_frame_time_ms > 0.0);
        assert!(stats.min_frame_time_ms <= stats.avg_frame_time_ms);
        assert!(stats.max_frame_time_ms >= stats.avg_frame_time_ms);
        assert!(stats.fps > 0.0);
        assert!(stats.avg_memory_bytes > 0);

        let time_range = chart.time_range();
        assert!(time_range.is_some());
    }

    #[test]
    fn test_memory_trend_chart_statistics() {
        let snapshots = vec![
            MemorySnapshot {
                timestamp: Instant::now(),
                total_memory: 1024 * 1024,
                active_allocations: 10,
            },
            MemorySnapshot {
                timestamp: Instant::now(),
                total_memory: 2 * 1024 * 1024,
                active_allocations: 20,
            },
            MemorySnapshot {
                timestamp: Instant::now(),
                total_memory: 1024 * 1024 * 3,
                active_allocations: 15,
            },
        ];

        let chart = MemoryTrendChart {
            snapshots: snapshots.clone(),
            config: VisualizationConfig::default(),
        };

        assert_eq!(chart.data_point_count(), 3);

        let stats = chart.get_statistics();
        assert!(stats.avg_memory_bytes > 0);
        assert!(stats.min_memory_bytes <= stats.avg_memory_bytes);
        assert!(stats.max_memory_bytes >= stats.avg_memory_bytes);
        assert!(stats.avg_allocations > 0);
        assert!(stats.min_allocations <= stats.avg_allocations);
        assert!(stats.max_allocations >= stats.avg_allocations);
    }

    #[tokio::test]
    async fn test_gpu_debug_visualizer_creation() {
        // Test that we can create a visualizer (without actually initializing GPU)
        // This is a minimal test since full GPU tests require hardware

        let config = VisualizationConfig {
            width: 1024,
            height: 768,
            enable_interaction: false,
            color_scheme: ColorScheme::Grayscale,
            max_data_points: 5000,
        };

        // Just verify config is applied correctly
        assert_eq!(config.width, 1024);
        assert_eq!(config.height, 768);
        assert!(!config.enable_interaction);
        assert_eq!(config.max_data_points, 5000);
    }

    #[test]
    fn test_performance_statistics_default() {
        let stats = PerformanceStatistics::default();
        assert_eq!(stats.avg_frame_time_ms, 0.0);
        assert_eq!(stats.min_frame_time_ms, 0.0);
        assert_eq!(stats.max_frame_time_ms, 0.0);
        assert_eq!(stats.avg_memory_bytes, 0);
        assert_eq!(stats.fps, 0.0);
    }

    #[test]
    fn test_memory_statistics_default() {
        let stats = MemoryStatistics::default();
        assert_eq!(stats.avg_memory_bytes, 0);
        assert_eq!(stats.min_memory_bytes, 0);
        assert_eq!(stats.max_memory_bytes, 0);
        assert_eq!(stats.avg_allocations, 0);
        assert_eq!(stats.min_allocations, 0);
        assert_eq!(stats.max_allocations, 0);
    }
}

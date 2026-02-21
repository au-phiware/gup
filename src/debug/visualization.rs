// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Text-based visualization utilities for GPU debug data.
//!
//! This module provides simple ASCII-art visualizations for memory usage,
//! performance trends, and resource utilization that can be displayed in
//! terminals or log files.

use crate::debug::{MemoryReport, MemorySnapshot, PerformanceSummary};
use std::fmt::Write;

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
}

// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Performance targets and validation for Phase 1 components.
//!
//! This module defines the performance targets that Phase 1 must meet,
//! provides validation logic for benchmarking results, and includes
//! bottleneck analysis utilities.

use std::collections::HashMap;
use std::fmt;
use std::time::Duration;

/// Phase 1 performance targets for Gup.
///
/// These represent the minimum acceptable performance for a production-ready
/// visualization library with GPU acceleration.
#[derive(Debug, Clone)]
pub struct PerformanceTargets {
    /// Maximum average frame time for sustained rendering (16.67ms = 60 FPS).
    pub max_frame_time: Duration,
    /// Maximum interaction (hit-testing) response time.
    pub max_interaction_time: Duration,
    /// Maximum acceptable overhead of composed shaders vs hand-optimized (0.05 = 5%).
    pub max_shader_overhead: f32,
    /// Maximum acceptable memory overhead ratio (0.10 = 10%).
    pub max_memory_overhead: f32,
    /// Minimum data points rendered at target frame rate.
    pub min_throughput: usize,
}

impl PerformanceTargets {
    /// Default Phase 1 performance targets.
    pub fn phase1() -> Self {
        Self {
            max_frame_time: Duration::from_micros(16_670), // 60 FPS
            max_interaction_time: Duration::from_millis(1),
            max_shader_overhead: 0.05, // 5%
            max_memory_overhead: 0.10, // 10%
            min_throughput: 100_000,   // 100K points at 60 FPS
        }
    }

    /// Relaxed targets for debug builds.
    ///
    /// Debug builds are significantly slower due to bounds checking and
    /// lack of optimizations. These targets allow CI to pass in debug mode.
    pub fn debug() -> Self {
        Self {
            max_frame_time: Duration::from_millis(100),
            max_interaction_time: Duration::from_millis(50),
            max_shader_overhead: 0.50, // 50%
            max_memory_overhead: 0.30, // 30%
            min_throughput: 10_000,    // 10K points
        }
    }

    /// Targets appropriate for the current build profile.
    pub fn current_profile() -> Self {
        if cfg!(debug_assertions) {
            Self::debug()
        } else {
            Self::phase1()
        }
    }
}

/// Result of rendering performance measurement.
#[derive(Debug, Clone)]
pub struct RenderingResult {
    /// Number of data points rendered.
    pub point_count: usize,
    /// Average frame time across measured frames.
    pub avg_frame_time: Duration,
    /// 95th percentile frame time.
    pub p95_frame_time: Duration,
    /// Maximum frame time observed.
    pub max_frame_time: Duration,
    /// Total frames measured.
    pub frame_count: usize,
}

/// Result of interaction performance measurement.
#[derive(Debug, Clone)]
pub struct InteractionResult {
    /// Number of data points in the dataset.
    pub point_count: usize,
    /// Average query time across measured queries.
    pub avg_query_time: Duration,
    /// 95th percentile query time.
    pub p95_query_time: Duration,
    /// Maximum query time observed.
    pub max_query_time: Duration,
    /// Total queries measured.
    pub query_count: usize,
}

/// Result of shader composition overhead measurement.
#[derive(Debug, Clone)]
pub struct ShaderOverheadResult {
    /// Average time for composed shader execution.
    pub composed_time: Duration,
    /// Average time for hand-optimized shader execution.
    pub optimized_time: Duration,
    /// Overhead as a fraction (e.g. 0.03 = 3%).
    pub overhead: f32,
}

/// Result of memory scaling measurement.
#[derive(Debug, Clone)]
pub struct MemoryScalingResult {
    /// (data_size, memory_used_bytes) pairs.
    pub measurements: Vec<(usize, usize)>,
    /// R² coefficient of linear fit (1.0 = perfectly linear).
    pub linearity_r_squared: f32,
    /// Memory overhead ratio (total - theoretical_minimum) / theoretical_minimum.
    pub overhead_ratio: f32,
}

/// A single performance issue found during validation.
#[derive(Debug, Clone)]
pub enum PerformanceIssue {
    /// Frame time exceeds the target.
    FrameTimeExceeded {
        target: Duration,
        actual: Duration,
        point_count: usize,
    },
    /// Interaction response time exceeds the target.
    InteractionTimeExceeded {
        target: Duration,
        actual: Duration,
        point_count: usize,
    },
    /// Shader composition overhead exceeds the target.
    ShaderOverheadExceeded {
        target_percent: f32,
        actual_percent: f32,
    },
    /// Memory overhead exceeds the target.
    MemoryOverheadExceeded {
        target_percent: f32,
        actual_percent: f32,
    },
    /// Memory scaling is not linear.
    NonLinearMemoryScaling { r_squared: f32 },
    /// Throughput below minimum target.
    ThroughputBelowTarget { target: usize, actual: usize },
}

impl fmt::Display for PerformanceIssue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FrameTimeExceeded {
                target,
                actual,
                point_count,
            } => write!(
                f,
                "Frame time {actual:?} exceeds target {target:?} for {point_count} points"
            ),
            Self::InteractionTimeExceeded {
                target,
                actual,
                point_count,
            } => write!(
                f,
                "Interaction time {actual:?} exceeds target {target:?} for {point_count} points"
            ),
            Self::ShaderOverheadExceeded {
                target_percent,
                actual_percent,
            } => write!(
                f,
                "Shader overhead {actual_percent:.1}% exceeds target {target_percent:.1}%"
            ),
            Self::MemoryOverheadExceeded {
                target_percent,
                actual_percent,
            } => write!(
                f,
                "Memory overhead {actual_percent:.1}% exceeds target {target_percent:.1}%"
            ),
            Self::NonLinearMemoryScaling { r_squared } => {
                write!(f, "Memory scaling not linear (R²={r_squared:.3})")
            }
            Self::ThroughputBelowTarget { target, actual } => {
                write!(f, "Throughput {actual} below target {target}")
            }
        }
    }
}

/// Outcome of validating performance results against targets.
#[derive(Debug, Clone)]
pub struct ValidationResult {
    /// Issues found during validation.
    pub issues: Vec<PerformanceIssue>,
}

impl ValidationResult {
    /// Returns true if all targets were met.
    pub fn passed(&self) -> bool {
        self.issues.is_empty()
    }

    /// Human-readable summary of the validation.
    pub fn summary(&self) -> String {
        if self.passed() {
            "All performance targets met.".to_string()
        } else {
            let mut s = format!("{} performance issue(s):\n", self.issues.len());
            for issue in &self.issues {
                s.push_str(&format!("  - {issue}\n"));
            }
            s
        }
    }
}

impl PerformanceTargets {
    /// Validate a rendering result against these targets.
    pub fn validate_rendering(&self, result: &RenderingResult) -> ValidationResult {
        let mut issues = Vec::new();

        if result.avg_frame_time > self.max_frame_time {
            issues.push(PerformanceIssue::FrameTimeExceeded {
                target: self.max_frame_time,
                actual: result.avg_frame_time,
                point_count: result.point_count,
            });
        }

        if result.point_count < self.min_throughput && result.avg_frame_time <= self.max_frame_time
        {
            // Only flag throughput if frame time is ok but count is below minimum
            issues.push(PerformanceIssue::ThroughputBelowTarget {
                target: self.min_throughput,
                actual: result.point_count,
            });
        }

        ValidationResult { issues }
    }

    /// Validate an interaction result against these targets.
    pub fn validate_interaction(&self, result: &InteractionResult) -> ValidationResult {
        let mut issues = Vec::new();

        if result.avg_query_time > self.max_interaction_time {
            issues.push(PerformanceIssue::InteractionTimeExceeded {
                target: self.max_interaction_time,
                actual: result.avg_query_time,
                point_count: result.point_count,
            });
        }

        ValidationResult { issues }
    }

    /// Validate a shader overhead result against these targets.
    pub fn validate_shader_overhead(&self, result: &ShaderOverheadResult) -> ValidationResult {
        let mut issues = Vec::new();

        if result.overhead > self.max_shader_overhead {
            issues.push(PerformanceIssue::ShaderOverheadExceeded {
                target_percent: self.max_shader_overhead * 100.0,
                actual_percent: result.overhead * 100.0,
            });
        }

        ValidationResult { issues }
    }

    /// Validate a memory scaling result against these targets.
    pub fn validate_memory(&self, result: &MemoryScalingResult) -> ValidationResult {
        let mut issues = Vec::new();

        if result.overhead_ratio > self.max_memory_overhead {
            issues.push(PerformanceIssue::MemoryOverheadExceeded {
                target_percent: self.max_memory_overhead * 100.0,
                actual_percent: result.overhead_ratio * 100.0,
            });
        }

        // Check linearity: R² should be above 0.9 for roughly linear scaling.
        if result.linearity_r_squared < 0.9 {
            issues.push(PerformanceIssue::NonLinearMemoryScaling {
                r_squared: result.linearity_r_squared,
            });
        }

        ValidationResult { issues }
    }
}

// ---------------------------------------------------------------------------
// Bottleneck analysis
// ---------------------------------------------------------------------------

/// Severity of a detected bottleneck.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// Informational — not blocking but worth noting.
    Low,
    /// Worth investigating.
    Medium,
    /// Likely impacting user experience.
    High,
    /// Must be resolved before release.
    Critical,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "Low"),
            Self::Medium => write!(f, "Medium"),
            Self::High => write!(f, "High"),
            Self::Critical => write!(f, "Critical"),
        }
    }
}

/// Where a bottleneck occurs.
#[derive(Debug, Clone)]
pub enum BottleneckLocation {
    /// A GPU pipeline stage (e.g. "vertex_shader", "fragment_shader").
    Gpu(String),
    /// A CPU function or subsystem (e.g. "data_upload", "pipeline_creation").
    Cpu(String),
}

impl fmt::Display for BottleneckLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Gpu(stage) => write!(f, "GPU:{stage}"),
            Self::Cpu(func) => write!(f, "CPU:{func}"),
        }
    }
}

/// A detected performance bottleneck.
#[derive(Debug, Clone)]
pub struct Bottleneck {
    /// Where the bottleneck occurs.
    pub location: BottleneckLocation,
    /// Time spent in this bottleneck.
    pub time_spent: Duration,
    /// Fraction of total frame time (0.0–1.0).
    pub percentage: f32,
    /// How severe the bottleneck is.
    pub severity: Severity,
}

impl fmt::Display for Bottleneck {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}] {} — {:.1}% of frame ({:?})",
            self.severity,
            self.location,
            self.percentage * 100.0,
            self.time_spent,
        )
    }
}

/// Profile data collected from a rendering session.
#[derive(Debug, Clone, Default)]
pub struct ProfileData {
    /// GPU stage timings.
    pub gpu_timings: HashMap<String, Duration>,
    /// CPU stage timings.
    pub cpu_timings: HashMap<String, Duration>,
    /// Total frame time (wall-clock).
    pub total_frame_time: Duration,
}

/// Analyzes profile data to identify bottlenecks.
#[derive(Debug)]
pub struct BottleneckAnalyzer {
    /// GPU stage consuming more than this fraction triggers a bottleneck report.
    pub gpu_threshold: f32,
    /// CPU stage consuming more than this fraction triggers a bottleneck report.
    pub cpu_threshold: f32,
}

impl Default for BottleneckAnalyzer {
    fn default() -> Self {
        Self {
            gpu_threshold: 0.30, // 30% of frame time on a single GPU stage
            cpu_threshold: 0.10, // 10% of frame time on a single CPU stage
        }
    }
}

impl BottleneckAnalyzer {
    /// Create a new analyzer with custom thresholds.
    pub fn new(gpu_threshold: f32, cpu_threshold: f32) -> Self {
        Self {
            gpu_threshold,
            cpu_threshold,
        }
    }

    /// Identify bottlenecks from profile data.
    pub fn identify(&self, profile: &ProfileData) -> Vec<Bottleneck> {
        if profile.total_frame_time.is_zero() {
            return Vec::new();
        }

        let total_nanos = profile.total_frame_time.as_nanos() as f32;
        let mut bottlenecks = Vec::new();

        // Analyze GPU stages.
        for (stage, timing) in &profile.gpu_timings {
            let percentage = timing.as_nanos() as f32 / total_nanos;
            if percentage > self.gpu_threshold {
                let severity = if percentage > 0.5 {
                    Severity::Critical
                } else {
                    Severity::High
                };
                bottlenecks.push(Bottleneck {
                    location: BottleneckLocation::Gpu(stage.clone()),
                    time_spent: *timing,
                    percentage,
                    severity,
                });
            }
        }

        // Analyze CPU stages.
        for (func, timing) in &profile.cpu_timings {
            let percentage = timing.as_nanos() as f32 / total_nanos;
            if percentage > self.cpu_threshold {
                let severity = if percentage > 0.3 {
                    Severity::High
                } else {
                    Severity::Medium
                };
                bottlenecks.push(Bottleneck {
                    location: BottleneckLocation::Cpu(func.clone()),
                    time_spent: *timing,
                    percentage,
                    severity,
                });
            }
        }

        bottlenecks.sort_by(|a, b| {
            b.percentage
                .partial_cmp(&a.percentage)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        bottlenecks
    }
}

// ---------------------------------------------------------------------------
// Utility functions
// ---------------------------------------------------------------------------

/// Compute the p-th percentile of a sorted slice of durations.
///
/// `p` must be in `0.0..=1.0`. The slice must be sorted in ascending order.
pub fn percentile(sorted: &[Duration], p: f32) -> Duration {
    assert!(
        (0.0..=1.0).contains(&p),
        "percentile must be between 0.0 and 1.0"
    );
    if sorted.is_empty() {
        return Duration::ZERO;
    }
    let idx = ((sorted.len() as f32 * p) as usize).min(sorted.len() - 1);
    sorted[idx]
}

/// Compute simple linear regression R² for (x, y) pairs.
///
/// Returns 0.0 if there are fewer than 2 points or all x values are the same.
pub fn linear_r_squared(points: &[(f64, f64)]) -> f32 {
    if points.len() < 2 {
        return 0.0;
    }

    let n = points.len() as f64;
    let sum_x: f64 = points.iter().map(|(x, _)| x).sum();
    let sum_y: f64 = points.iter().map(|(_, y)| y).sum();
    let sum_xy: f64 = points.iter().map(|(x, y)| x * y).sum();
    let sum_x2: f64 = points.iter().map(|(x, _)| x * x).sum();
    let sum_y2: f64 = points.iter().map(|(_, y)| y * y).sum();

    let numerator = n * sum_xy - sum_x * sum_y;
    let denom_x = n * sum_x2 - sum_x * sum_x;
    let denom_y = n * sum_y2 - sum_y * sum_y;
    let denominator = denom_x * denom_y;

    if denominator <= 0.0 {
        return 0.0;
    }

    let r = numerator / denominator.sqrt();
    (r * r) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_phase1_targets() {
        let targets = PerformanceTargets::phase1();
        assert_eq!(targets.max_frame_time, Duration::from_micros(16_670));
        assert_eq!(targets.max_interaction_time, Duration::from_millis(1));
        assert_eq!(targets.max_shader_overhead, 0.05);
        assert_eq!(targets.max_memory_overhead, 0.10);
        assert_eq!(targets.min_throughput, 100_000);
    }

    #[test]
    fn test_debug_targets_are_relaxed() {
        let phase1 = PerformanceTargets::phase1();
        let debug = PerformanceTargets::debug();
        assert!(debug.max_frame_time > phase1.max_frame_time);
        assert!(debug.max_interaction_time > phase1.max_interaction_time);
        assert!(debug.max_shader_overhead > phase1.max_shader_overhead);
    }

    #[test]
    fn test_validate_rendering_pass() {
        let targets = PerformanceTargets::phase1();
        let result = RenderingResult {
            point_count: 100_000,
            avg_frame_time: Duration::from_millis(10),
            p95_frame_time: Duration::from_millis(14),
            max_frame_time: Duration::from_millis(18),
            frame_count: 100,
        };
        let validation = targets.validate_rendering(&result);
        assert!(validation.passed(), "{}", validation.summary());
    }

    #[test]
    fn test_validate_rendering_fail() {
        let targets = PerformanceTargets::phase1();
        let result = RenderingResult {
            point_count: 100_000,
            avg_frame_time: Duration::from_millis(25), // > 16.67ms
            p95_frame_time: Duration::from_millis(30),
            max_frame_time: Duration::from_millis(50),
            frame_count: 100,
        };
        let validation = targets.validate_rendering(&result);
        assert!(!validation.passed());
        assert!(matches!(
            validation.issues[0],
            PerformanceIssue::FrameTimeExceeded { .. }
        ));
    }

    #[test]
    fn test_validate_interaction_pass() {
        let targets = PerformanceTargets::phase1();
        let result = InteractionResult {
            point_count: 1_000_000,
            avg_query_time: Duration::from_micros(800),
            p95_query_time: Duration::from_micros(950),
            max_query_time: Duration::from_millis(2),
            query_count: 100,
        };
        let validation = targets.validate_interaction(&result);
        assert!(validation.passed(), "{}", validation.summary());
    }

    #[test]
    fn test_validate_interaction_fail() {
        let targets = PerformanceTargets::phase1();
        let result = InteractionResult {
            point_count: 1_000_000,
            avg_query_time: Duration::from_millis(5), // > 1ms
            p95_query_time: Duration::from_millis(8),
            max_query_time: Duration::from_millis(15),
            query_count: 100,
        };
        let validation = targets.validate_interaction(&result);
        assert!(!validation.passed());
    }

    #[test]
    fn test_validate_shader_overhead_pass() {
        let targets = PerformanceTargets::phase1();
        let result = ShaderOverheadResult {
            composed_time: Duration::from_micros(1020),
            optimized_time: Duration::from_micros(1000),
            overhead: 0.02,
        };
        let validation = targets.validate_shader_overhead(&result);
        assert!(validation.passed());
    }

    #[test]
    fn test_validate_memory_pass() {
        let targets = PerformanceTargets::phase1();
        let result = MemoryScalingResult {
            measurements: vec![(1000, 40000), (10_000, 400_000), (100_000, 4_000_000)],
            linearity_r_squared: 0.99,
            overhead_ratio: 0.05,
        };
        let validation = targets.validate_memory(&result);
        assert!(validation.passed(), "{}", validation.summary());
    }

    #[test]
    fn test_validate_memory_non_linear() {
        let targets = PerformanceTargets::phase1();
        let result = MemoryScalingResult {
            measurements: vec![(1000, 40000), (10_000, 4_000_000)],
            linearity_r_squared: 0.5,
            overhead_ratio: 0.05,
        };
        let validation = targets.validate_memory(&result);
        assert!(!validation.passed());
        assert!(matches!(
            validation.issues[0],
            PerformanceIssue::NonLinearMemoryScaling { .. }
        ));
    }

    #[test]
    fn test_percentile() {
        let sorted = vec![
            Duration::from_millis(1),
            Duration::from_millis(2),
            Duration::from_millis(3),
            Duration::from_millis(4),
            Duration::from_millis(5),
        ];
        assert_eq!(percentile(&sorted, 0.0), Duration::from_millis(1));
        assert_eq!(percentile(&sorted, 0.5), Duration::from_millis(3));
        assert_eq!(percentile(&sorted, 1.0), Duration::from_millis(5));
    }

    #[test]
    fn test_linear_r_squared_perfect() {
        let points: Vec<(f64, f64)> = (0..10).map(|i| (i as f64, i as f64 * 2.0)).collect();
        let r2 = linear_r_squared(&points);
        assert!((r2 - 1.0).abs() < 0.001, "Expected ~1.0, got {r2}");
    }

    #[test]
    fn test_linear_r_squared_poor() {
        let points = vec![(1.0, 1.0), (2.0, 100.0), (3.0, 1.0), (4.0, 100.0)];
        let r2 = linear_r_squared(&points);
        assert!(r2 < 0.5, "Expected poor linearity, got R²={r2}");
    }

    #[test]
    fn test_bottleneck_analyzer_identifies_gpu_bottleneck() {
        let analyzer = BottleneckAnalyzer::default();
        let mut profile = ProfileData {
            total_frame_time: Duration::from_millis(16),
            ..Default::default()
        };
        profile
            .gpu_timings
            .insert("fragment_shader".to_string(), Duration::from_millis(10));
        profile
            .gpu_timings
            .insert("vertex_shader".to_string(), Duration::from_millis(2));

        let bottlenecks = analyzer.identify(&profile);
        assert_eq!(bottlenecks.len(), 1);
        assert!(matches!(
            bottlenecks[0].location,
            BottleneckLocation::Gpu(ref s) if s == "fragment_shader"
        ));
        assert!(bottlenecks[0].severity >= Severity::High);
    }

    #[test]
    fn test_bottleneck_analyzer_cpu() {
        let analyzer = BottleneckAnalyzer::default();
        let mut profile = ProfileData {
            total_frame_time: Duration::from_millis(16),
            ..Default::default()
        };
        profile
            .cpu_timings
            .insert("data_upload".to_string(), Duration::from_millis(3));

        let bottlenecks = analyzer.identify(&profile);
        assert_eq!(bottlenecks.len(), 1);
        assert!(matches!(
            bottlenecks[0].location,
            BottleneckLocation::Cpu(ref s) if s == "data_upload"
        ));
    }

    #[test]
    fn test_bottleneck_analyzer_empty_profile() {
        let analyzer = BottleneckAnalyzer::default();
        let profile = ProfileData::default();
        let bottlenecks = analyzer.identify(&profile);
        assert!(bottlenecks.is_empty());
    }

    #[test]
    fn test_validation_result_summary() {
        let result = ValidationResult {
            issues: vec![PerformanceIssue::FrameTimeExceeded {
                target: Duration::from_millis(16),
                actual: Duration::from_millis(25),
                point_count: 100_000,
            }],
        };
        let summary = result.summary();
        assert!(summary.contains("1 performance issue"));
        assert!(summary.contains("Frame time"));
    }
}

// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Axis performance optimization infrastructure.
//!
//! This module provides performance optimizations for the axis rendering system,
//! including:
//!
//! * **Level of Detail (LOD)** — automatic quality adjustment based on axis size
//!   and performance budget
//! * **Geometry caching** — avoid regenerating axis and grid vertices every frame
//!   when configuration hasn't changed
//! * **Resource pooling** — shared, pre-allocated buffers across multiple axes
//! * **Adaptive performance monitoring** — rolling metrics with budget-based
//!   optimization strategy selection
//! * **Label culling** — skip labels outside the viewport before rendering

use crate::axis::{AxisBounds, AxisConfiguration, AxisPosition};
use crate::render::Vertex;
use std::collections::HashMap;
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// Level of Detail (LOD)
// ---------------------------------------------------------------------------

/// Quality level for axis rendering.
///
/// Higher levels include more visual detail (minor ticks, full label set,
/// smooth anti-aliasing). Lower levels progressively remove features to
/// maintain frame-rate targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LODLevel {
    /// Emergency fallback — basic axis lines only, no ticks or labels.
    Minimal,
    /// Minimal quality — sparse labels, simple geometry.
    Low,
    /// Reduced quality — major labels only, no minor ticks.
    Medium,
    /// Full quality — all labels, minor ticks, smooth anti-aliasing.
    High,
}

impl LODLevel {
    /// Whether minor ticks should be shown at this LOD.
    pub fn show_minor_ticks(self) -> bool {
        self == LODLevel::High
    }

    /// Whether major ticks should be shown at this LOD.
    pub fn show_major_ticks(self) -> bool {
        matches!(self, LODLevel::High | LODLevel::Medium)
    }

    /// Whether labels should be shown at this LOD.
    pub fn show_labels(self) -> bool {
        matches!(self, LODLevel::High | LODLevel::Medium | LODLevel::Low)
    }

    /// Maximum number of labels to display at this LOD.
    ///
    /// Returns `None` for unlimited (High), or a cap for lower levels.
    pub fn max_labels(self) -> Option<usize> {
        match self {
            LODLevel::High => None,
            LODLevel::Medium => Some(10),
            LODLevel::Low => Some(5),
            LODLevel::Minimal => Some(0),
        }
    }

    /// Whether the axis line should be shown at this LOD.
    pub fn show_line(self) -> bool {
        true // Always show the axis line
    }

    /// Apply this LOD level to an axis configuration, returning a
    /// modified copy with features disabled as appropriate.
    pub fn apply_to_config(self, config: &AxisConfiguration) -> AxisConfiguration {
        let mut adjusted = config.clone();
        if !self.show_minor_ticks() {
            adjusted.show_minor_ticks = false;
        }
        if !self.show_major_ticks() {
            adjusted.show_major_ticks = false;
        }
        if !self.show_line() {
            adjusted.show_line = false;
        }
        adjusted
    }
}

/// Thresholds that control LOD transitions.
#[derive(Debug, Clone)]
pub struct LODConfiguration {
    /// Axis shorter than this (in pixels) drops from High → Medium.
    pub high_to_medium_threshold: f32,
    /// Axis shorter than this (in pixels) drops from Medium → Low.
    pub medium_to_low_threshold: f32,
    /// Axis shorter than this (in pixels) drops from Low → Minimal.
    pub low_to_minimal_threshold: f32,
    /// If the most recent render took longer than this, force a downgrade.
    pub performance_downgrade_threshold: Duration,
}

impl Default for LODConfiguration {
    fn default() -> Self {
        Self {
            high_to_medium_threshold: 200.0,
            medium_to_low_threshold: 100.0,
            low_to_minimal_threshold: 50.0,
            performance_downgrade_threshold: Duration::from_millis(5),
        }
    }
}

/// Manages LOD selection for individual axes.
#[derive(Debug, Clone)]
pub struct AxisLODManager {
    config: LODConfiguration,
}

impl AxisLODManager {
    /// Create a new LOD manager with the given thresholds.
    pub fn new(config: LODConfiguration) -> Self {
        Self { config }
    }

    /// Calculate the optimal LOD for an axis given its pixel length and the
    /// most recent render time.
    pub fn calculate_lod(
        &self,
        axis_pixel_length: f32,
        last_render_time: Option<Duration>,
    ) -> LODLevel {
        // Performance constraint takes priority
        if let Some(render_time) = last_render_time
            && render_time > self.config.performance_downgrade_threshold
        {
            return LODLevel::Low;
        }

        // Size-based LOD selection
        if axis_pixel_length < self.config.low_to_minimal_threshold {
            LODLevel::Minimal
        } else if axis_pixel_length < self.config.medium_to_low_threshold {
            LODLevel::Low
        } else if axis_pixel_length < self.config.high_to_medium_threshold {
            LODLevel::Medium
        } else {
            LODLevel::High
        }
    }

    /// Access the current configuration.
    pub fn config(&self) -> &LODConfiguration {
        &self.config
    }

    /// Replace the configuration.
    pub fn set_config(&mut self, config: LODConfiguration) {
        self.config = config;
    }
}

impl Default for AxisLODManager {
    fn default() -> Self {
        Self::new(LODConfiguration::default())
    }
}

// ---------------------------------------------------------------------------
// Geometry caching
// ---------------------------------------------------------------------------

/// A fingerprint of the inputs that determine axis geometry.
///
/// When the fingerprint hasn't changed between frames the cached vertex
/// data can be reused without regeneration.
#[derive(Debug, Clone, PartialEq)]
struct GeometryCacheKey {
    bounds_start: [f32; 2],
    bounds_end: [f32; 2],
    lod: LODLevel,
    position: AxisPosition,
    viewport_size: (f32, f32),
    show_line: bool,
    show_major_ticks: bool,
    show_minor_ticks: bool,
    major_tick_length: u32, // stored as bits via to_bits()
    minor_tick_length: u32,
}

impl GeometryCacheKey {
    fn from_inputs(
        bounds: &AxisBounds,
        config: &AxisConfiguration,
        position: AxisPosition,
        viewport_size: (f32, f32),
        lod: LODLevel,
    ) -> Self {
        Self {
            bounds_start: [bounds.start.x, bounds.start.y],
            bounds_end: [bounds.end.x, bounds.end.y],
            lod,
            position,
            viewport_size,
            show_line: config.show_line,
            show_major_ticks: config.show_major_ticks,
            show_minor_ticks: config.show_minor_ticks,
            major_tick_length: config.major_tick_length.to_bits(),
            minor_tick_length: config.minor_tick_length.to_bits(),
        }
    }
}

/// Caches generated axis vertex data to avoid regeneration on every frame.
#[derive(Debug)]
pub struct AxisGeometryCache {
    cached_vertices: Option<Vec<Vertex>>,
    cache_key: Option<GeometryCacheKey>,
    hits: u64,
    misses: u64,
}

impl AxisGeometryCache {
    /// Create a new, empty cache.
    pub fn new() -> Self {
        Self {
            cached_vertices: None,
            cache_key: None,
            hits: 0,
            misses: 0,
        }
    }

    /// Try to retrieve cached vertices.
    ///
    /// Returns `Some(&[Vertex])` if the cache is valid for the given inputs,
    /// `None` if regeneration is needed.
    pub fn get(
        &mut self,
        bounds: &AxisBounds,
        config: &AxisConfiguration,
        position: AxisPosition,
        viewport_size: (f32, f32),
        lod: LODLevel,
    ) -> Option<&[Vertex]> {
        let key = GeometryCacheKey::from_inputs(bounds, config, position, viewport_size, lod);
        if self.cache_key.as_ref() == Some(&key) {
            self.hits += 1;
            self.cached_vertices.as_deref()
        } else {
            self.misses += 1;
            None
        }
    }

    /// Store newly generated vertices in the cache.
    pub fn store(
        &mut self,
        bounds: &AxisBounds,
        config: &AxisConfiguration,
        position: AxisPosition,
        viewport_size: (f32, f32),
        lod: LODLevel,
        vertices: Vec<Vertex>,
    ) {
        let key = GeometryCacheKey::from_inputs(bounds, config, position, viewport_size, lod);
        self.cache_key = Some(key);
        self.cached_vertices = Some(vertices);
    }

    /// Invalidate the cache, forcing regeneration on the next call.
    pub fn invalidate(&mut self) {
        self.cached_vertices = None;
        self.cache_key = None;
    }

    /// Cache hit rate (0.0–1.0). Returns 0.0 if no lookups have occurred.
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }

    /// Total number of cache lookups.
    pub fn total_lookups(&self) -> u64 {
        self.hits + self.misses
    }
}

impl Default for AxisGeometryCache {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Adaptive performance monitoring
// ---------------------------------------------------------------------------

/// Strategy selected by the performance monitor based on current metrics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptimizationStrategy {
    /// Both memory and quality should be reduced.
    ReduceMemoryAndQuality,
    /// Quality can be reduced but memory is fine.
    ReduceQuality,
    /// Current settings are acceptable.
    Maintain,
    /// Performance headroom allows higher quality.
    IncreaseQuality,
}

/// Performance budget for the axis system.
#[derive(Debug, Clone)]
pub struct PerformanceBudget {
    /// Target total axis rendering time per frame.
    pub target_render_time: Duration,
    /// Quality vs performance trade-off preference (0.0 = performance, 1.0 = quality).
    pub quality_preference: f32,
}

impl Default for PerformanceBudget {
    fn default() -> Self {
        Self {
            target_render_time: Duration::from_millis(1),
            quality_preference: 0.7,
        }
    }
}

/// Enhanced performance monitor with rolling averages and adaptive strategy.
#[derive(Debug)]
pub struct AxisPerformanceMonitor {
    /// Rolling window of recent render times.
    recent_render_times: Vec<Duration>,
    /// Maximum number of samples to keep.
    max_samples: usize,
    /// Performance budget.
    budget: PerformanceBudget,
    /// When the last render was recorded.
    last_record_time: Option<Instant>,
    /// Frame counter for periodic reporting.
    frame_count: u64,
}

impl AxisPerformanceMonitor {
    /// Create a new performance monitor with the given budget.
    pub fn new(budget: PerformanceBudget) -> Self {
        Self {
            recent_render_times: Vec::with_capacity(120),
            max_samples: 120,
            budget,
            last_record_time: None,
            frame_count: 0,
        }
    }

    /// Record a single frame's axis rendering time.
    pub fn record_render_time(&mut self, elapsed: Duration) {
        if self.recent_render_times.len() >= self.max_samples {
            self.recent_render_times.remove(0);
        }
        self.recent_render_times.push(elapsed);
        self.last_record_time = Some(Instant::now());
        self.frame_count += 1;
    }

    /// Compute the rolling average render time.
    pub fn average_render_time(&self) -> Duration {
        if self.recent_render_times.is_empty() {
            return Duration::ZERO;
        }
        let total: Duration = self.recent_render_times.iter().sum();
        total / self.recent_render_times.len() as u32
    }

    /// The worst (maximum) render time in the window.
    pub fn worst_render_time(&self) -> Duration {
        self.recent_render_times
            .iter()
            .copied()
            .max()
            .unwrap_or(Duration::ZERO)
    }

    /// Most recent render time, if any.
    pub fn last_render_time(&self) -> Option<Duration> {
        self.recent_render_times.last().copied()
    }

    /// Whether the system is currently within budget on average.
    pub fn is_within_budget(&self) -> bool {
        self.average_render_time() <= self.budget.target_render_time
    }

    /// Determine the recommended optimization strategy based on
    /// current performance metrics.
    pub fn recommended_strategy(&self) -> OptimizationStrategy {
        let avg = self.average_render_time();
        let target = self.budget.target_render_time;

        if avg > target {
            // Over budget
            OptimizationStrategy::ReduceQuality
        } else if avg > target.mul_f32(0.8) {
            // Close to budget
            OptimizationStrategy::Maintain
        } else if avg < target.mul_f32(0.5) {
            // Well under budget — room to improve quality
            OptimizationStrategy::IncreaseQuality
        } else {
            OptimizationStrategy::Maintain
        }
    }

    /// Access the budget.
    pub fn budget(&self) -> &PerformanceBudget {
        &self.budget
    }

    /// Replace the budget.
    pub fn set_budget(&mut self, budget: PerformanceBudget) {
        self.budget = budget;
    }

    /// Number of frames recorded.
    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }

    /// Number of samples currently stored.
    pub fn sample_count(&self) -> usize {
        self.recent_render_times.len()
    }
}

impl Default for AxisPerformanceMonitor {
    fn default() -> Self {
        Self::new(PerformanceBudget::default())
    }
}

// ---------------------------------------------------------------------------
// Resource pool
// ---------------------------------------------------------------------------

/// A pre-allocated pool of vertex buffers that can be reused across
/// multiple axes within the same frame to avoid per-axis allocations.
#[derive(Debug)]
pub struct AxisResourcePool {
    /// Reusable vertex buffers (returned after each frame).
    free_buffers: Vec<Vec<Vertex>>,
    /// Number of buffers currently checked out.
    in_use_count: usize,
    /// Total number of allocations since creation (for diagnostics).
    total_allocations: u64,
    /// Total number of reuses since creation.
    total_reuses: u64,
}

impl AxisResourcePool {
    /// Create a new resource pool, optionally pre-allocating `initial_count`
    /// buffers each with capacity `initial_capacity`.
    pub fn new(initial_count: usize, initial_capacity: usize) -> Self {
        let free_buffers = (0..initial_count)
            .map(|_| Vec::with_capacity(initial_capacity))
            .collect();
        Self {
            free_buffers,
            in_use_count: 0,
            total_allocations: initial_count as u64,
            total_reuses: 0,
        }
    }

    /// Acquire a vertex buffer from the pool. If no free buffer is available,
    /// a new one is allocated with `default_capacity`.
    pub fn acquire(&mut self, default_capacity: usize) -> Vec<Vertex> {
        self.in_use_count += 1;
        if let Some(mut buf) = self.free_buffers.pop() {
            buf.clear();
            self.total_reuses += 1;
            buf
        } else {
            self.total_allocations += 1;
            Vec::with_capacity(default_capacity)
        }
    }

    /// Return a buffer to the pool for future reuse.
    pub fn release(&mut self, buf: Vec<Vertex>) {
        self.in_use_count = self.in_use_count.saturating_sub(1);
        self.free_buffers.push(buf);
    }

    /// Number of buffers currently available for reuse.
    pub fn available_count(&self) -> usize {
        self.free_buffers.len()
    }

    /// Number of buffers currently checked out.
    pub fn in_use_count(&self) -> usize {
        self.in_use_count
    }

    /// Reuse rate (0.0–1.0).
    pub fn reuse_rate(&self) -> f64 {
        let total = self.total_allocations + self.total_reuses;
        if total == 0 {
            0.0
        } else {
            self.total_reuses as f64 / total as f64
        }
    }

    /// Shrink the pool by releasing excess free buffers, keeping at most
    /// `max_free` buffers on hand.
    pub fn shrink_to(&mut self, max_free: usize) {
        while self.free_buffers.len() > max_free {
            self.free_buffers.pop();
        }
    }
}

impl Default for AxisResourcePool {
    fn default() -> Self {
        // Pre-allocate 4 buffers (typical: 2 axes × 2 components each)
        Self::new(4, 64)
    }
}

// ---------------------------------------------------------------------------
// Label culling
// ---------------------------------------------------------------------------

/// Viewport bounds for label visibility culling.
#[derive(Debug, Clone, Copy)]
pub struct ViewportBounds {
    /// Left edge in screen pixels.
    pub left: f32,
    /// Top edge in screen pixels.
    pub top: f32,
    /// Right edge in screen pixels.
    pub right: f32,
    /// Bottom edge in screen pixels.
    pub bottom: f32,
}

impl ViewportBounds {
    /// Create viewport bounds from width and height (origin at top-left).
    pub fn from_size(width: f32, height: f32) -> Self {
        Self {
            left: 0.0,
            top: 0.0,
            right: width,
            bottom: height,
        }
    }

    /// Create viewport bounds with explicit edges.
    pub fn new(left: f32, top: f32, right: f32, bottom: f32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    /// Check whether a screen-space point is within the viewport
    /// (with an optional margin for partially-visible labels).
    pub fn contains(&self, x: f32, y: f32, margin: f32) -> bool {
        x >= (self.left - margin)
            && x <= (self.right + margin)
            && y >= (self.top - margin)
            && y <= (self.bottom + margin)
    }
}

/// Cull axis labels that fall outside the viewport.
///
/// `labels` is any iterable of items that have a screen position.
/// This function returns indices of labels that are within the viewport
/// bounds (plus `margin` pixels of slack).
pub fn cull_label_indices(
    screen_positions: &[[f32; 2]],
    viewport: &ViewportBounds,
    margin: f32,
) -> Vec<usize> {
    screen_positions
        .iter()
        .enumerate()
        .filter(|(_, pos)| viewport.contains(pos[0], pos[1], margin))
        .map(|(i, _)| i)
        .collect()
}

// ---------------------------------------------------------------------------
// Per-axis render statistics (lightweight)
// ---------------------------------------------------------------------------

/// Lightweight statistics collected per axis during a single frame.
#[derive(Debug, Clone, Default)]
pub struct AxisRenderStats {
    /// Number of vertices generated.
    pub vertex_count: usize,
    /// Number of labels rendered.
    pub label_count: usize,
    /// Number of labels culled (skipped).
    pub labels_culled: usize,
    /// Whether the geometry cache was hit.
    pub cache_hit: bool,
    /// LOD level used.
    pub lod: Option<LODLevel>,
    /// Render time for this axis.
    pub render_time: Duration,
}

/// Aggregated render statistics for the complete axis system in one frame.
#[derive(Debug, Clone, Default)]
pub struct AxisSystemRenderStats {
    /// Per-axis stats keyed by a string identifier (e.g. "x", "y").
    pub per_axis: HashMap<String, AxisRenderStats>,
    /// Total axis system render time.
    pub total_render_time: Duration,
    /// Total vertex count across all axes.
    pub total_vertices: usize,
    /// Total label count across all axes.
    pub total_labels: usize,
    /// Total labels culled across all axes.
    pub total_labels_culled: usize,
}

impl AxisSystemRenderStats {
    /// Create empty stats.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add stats for a named axis.
    pub fn add_axis(&mut self, name: impl Into<String>, stats: AxisRenderStats) {
        let stats_copy = stats.clone();
        self.total_vertices += stats_copy.vertex_count;
        self.total_labels += stats_copy.label_count;
        self.total_labels_culled += stats_copy.labels_culled;
        self.per_axis.insert(name.into(), stats);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::axis::{AxisBounds, AxisConfiguration};
    use crate::shader_function::Vec2;

    // -- LOD tests --

    #[test]
    fn test_lod_level_ordering() {
        assert!(LODLevel::High > LODLevel::Medium);
        assert!(LODLevel::Medium > LODLevel::Low);
        assert!(LODLevel::Low > LODLevel::Minimal);
    }

    #[test]
    fn test_lod_features() {
        assert!(LODLevel::High.show_minor_ticks());
        assert!(!LODLevel::Medium.show_minor_ticks());

        assert!(LODLevel::High.show_major_ticks());
        assert!(LODLevel::Medium.show_major_ticks());
        assert!(!LODLevel::Low.show_major_ticks());

        assert!(LODLevel::High.show_labels());
        assert!(LODLevel::Low.show_labels());
        assert!(!LODLevel::Minimal.show_labels());
    }

    #[test]
    fn test_lod_max_labels() {
        assert_eq!(LODLevel::High.max_labels(), None);
        assert_eq!(LODLevel::Medium.max_labels(), Some(10));
        assert_eq!(LODLevel::Low.max_labels(), Some(5));
        assert_eq!(LODLevel::Minimal.max_labels(), Some(0));
    }

    #[test]
    fn test_lod_apply_to_config() {
        let config = AxisConfiguration::default();

        let high = LODLevel::High.apply_to_config(&config);
        assert!(high.show_line);
        assert!(high.show_major_ticks);
        // Minor ticks are off by default in AxisConfiguration, so stays off

        let minimal = LODLevel::Minimal.apply_to_config(&config);
        assert!(minimal.show_line);
        assert!(!minimal.show_major_ticks);
        assert!(!minimal.show_minor_ticks);
    }

    #[test]
    fn test_lod_manager_size_based() {
        let manager = AxisLODManager::default();

        assert_eq!(manager.calculate_lod(500.0, None), LODLevel::High);
        assert_eq!(manager.calculate_lod(150.0, None), LODLevel::Medium);
        assert_eq!(manager.calculate_lod(80.0, None), LODLevel::Low);
        assert_eq!(manager.calculate_lod(30.0, None), LODLevel::Minimal);
    }

    #[test]
    fn test_lod_manager_performance_override() {
        let manager = AxisLODManager::default();

        // Even though axis is large, if render took too long, force Low
        let slow = Some(Duration::from_millis(10));
        assert_eq!(manager.calculate_lod(500.0, slow), LODLevel::Low);
    }

    // -- Geometry cache tests --

    #[test]
    fn test_geometry_cache_miss_on_empty() {
        let mut cache = AxisGeometryCache::new();
        let bounds = AxisBounds::new(Vec2 { x: -0.8, y: -0.8 }, Vec2 { x: 0.8, y: -0.8 }, 50.0);
        let config = AxisConfiguration::default();

        assert!(
            cache
                .get(
                    &bounds,
                    &config,
                    AxisPosition::Bottom,
                    (800.0, 600.0),
                    LODLevel::High
                )
                .is_none()
        );
        assert_eq!(cache.total_lookups(), 1);
    }

    #[test]
    fn test_geometry_cache_hit_after_store() {
        let mut cache = AxisGeometryCache::new();
        let bounds = AxisBounds::new(Vec2 { x: -0.8, y: -0.8 }, Vec2 { x: 0.8, y: -0.8 }, 50.0);
        let config = AxisConfiguration::default();
        let verts = vec![
            Vertex {
                position: [-0.8, -0.8],
                color: [0.0; 4],
            },
            Vertex {
                position: [0.8, -0.8],
                color: [0.0; 4],
            },
        ];

        cache.store(
            &bounds,
            &config,
            AxisPosition::Bottom,
            (800.0, 600.0),
            LODLevel::High,
            verts.clone(),
        );

        let cached = cache.get(
            &bounds,
            &config,
            AxisPosition::Bottom,
            (800.0, 600.0),
            LODLevel::High,
        );
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().len(), 2);
        assert!(cache.hit_rate() > 0.0);
    }

    #[test]
    fn test_geometry_cache_miss_on_changed_lod() {
        let mut cache = AxisGeometryCache::new();
        let bounds = AxisBounds::new(Vec2 { x: -0.8, y: -0.8 }, Vec2 { x: 0.8, y: -0.8 }, 50.0);
        let config = AxisConfiguration::default();

        cache.store(
            &bounds,
            &config,
            AxisPosition::Bottom,
            (800.0, 600.0),
            LODLevel::High,
            vec![],
        );

        // Different LOD => miss
        assert!(
            cache
                .get(
                    &bounds,
                    &config,
                    AxisPosition::Bottom,
                    (800.0, 600.0),
                    LODLevel::Low
                )
                .is_none()
        );
    }

    #[test]
    fn test_geometry_cache_invalidation() {
        let mut cache = AxisGeometryCache::new();
        let bounds = AxisBounds::new(Vec2 { x: 0.0, y: 0.0 }, Vec2 { x: 1.0, y: 0.0 }, 50.0);
        let config = AxisConfiguration::default();

        cache.store(
            &bounds,
            &config,
            AxisPosition::Bottom,
            (800.0, 600.0),
            LODLevel::High,
            vec![],
        );
        cache.invalidate();

        assert!(
            cache
                .get(
                    &bounds,
                    &config,
                    AxisPosition::Bottom,
                    (800.0, 600.0),
                    LODLevel::High
                )
                .is_none()
        );
    }

    // -- Performance monitor tests --

    #[test]
    fn test_perf_monitor_empty() {
        let monitor = AxisPerformanceMonitor::default();
        assert_eq!(monitor.average_render_time(), Duration::ZERO);
        assert_eq!(monitor.frame_count(), 0);
        assert!(monitor.is_within_budget());
    }

    #[test]
    fn test_perf_monitor_record_and_average() {
        let mut monitor = AxisPerformanceMonitor::default();
        monitor.record_render_time(Duration::from_micros(500));
        monitor.record_render_time(Duration::from_micros(700));

        let avg = monitor.average_render_time();
        assert_eq!(avg, Duration::from_micros(600));
        assert_eq!(monitor.frame_count(), 2);
    }

    #[test]
    fn test_perf_monitor_within_budget() {
        let mut monitor = AxisPerformanceMonitor::new(PerformanceBudget {
            target_render_time: Duration::from_millis(1),
            quality_preference: 0.5,
        });

        monitor.record_render_time(Duration::from_micros(200));
        assert!(monitor.is_within_budget());
    }

    #[test]
    fn test_perf_monitor_over_budget() {
        let mut monitor = AxisPerformanceMonitor::new(PerformanceBudget {
            target_render_time: Duration::from_millis(1),
            quality_preference: 0.5,
        });

        monitor.record_render_time(Duration::from_millis(5));
        assert!(!monitor.is_within_budget());
    }

    #[test]
    fn test_perf_monitor_strategy() {
        let mut monitor = AxisPerformanceMonitor::new(PerformanceBudget {
            target_render_time: Duration::from_millis(1),
            quality_preference: 0.5,
        });

        // Well under budget
        monitor.record_render_time(Duration::from_micros(100));
        assert_eq!(
            monitor.recommended_strategy(),
            OptimizationStrategy::IncreaseQuality
        );

        // Over budget
        let mut over = AxisPerformanceMonitor::new(PerformanceBudget {
            target_render_time: Duration::from_millis(1),
            quality_preference: 0.5,
        });
        over.record_render_time(Duration::from_millis(5));
        assert_eq!(
            over.recommended_strategy(),
            OptimizationStrategy::ReduceQuality
        );
    }

    #[test]
    fn test_perf_monitor_rolling_window() {
        let budget = PerformanceBudget {
            target_render_time: Duration::from_millis(1),
            quality_preference: 0.5,
        };
        let mut monitor = AxisPerformanceMonitor::new(budget);
        // Override max_samples for testing
        monitor.max_samples = 3;

        monitor.record_render_time(Duration::from_micros(100));
        monitor.record_render_time(Duration::from_micros(200));
        monitor.record_render_time(Duration::from_micros(300));
        monitor.record_render_time(Duration::from_micros(600));

        // Only 3 most recent should remain
        assert_eq!(monitor.sample_count(), 3);
        assert_eq!(monitor.frame_count(), 4);
    }

    // -- Resource pool tests --

    #[test]
    fn test_resource_pool_pre_allocation() {
        let pool = AxisResourcePool::new(4, 32);
        assert_eq!(pool.available_count(), 4);
        assert_eq!(pool.in_use_count(), 0);
    }

    #[test]
    fn test_resource_pool_acquire_release() {
        let mut pool = AxisResourcePool::new(2, 32);

        let buf1 = pool.acquire(32);
        assert_eq!(pool.available_count(), 1);
        assert_eq!(pool.in_use_count(), 1);

        pool.release(buf1);
        assert_eq!(pool.available_count(), 2);
        assert_eq!(pool.in_use_count(), 0);
    }

    #[test]
    fn test_resource_pool_grow_on_exhaustion() {
        let mut pool = AxisResourcePool::new(0, 0);
        assert_eq!(pool.available_count(), 0);

        let buf = pool.acquire(16);
        assert_eq!(pool.in_use_count(), 1);
        assert_eq!(buf.capacity(), 16);
    }

    #[test]
    fn test_resource_pool_reuse_rate() {
        let mut pool = AxisResourcePool::new(2, 32);

        let buf = pool.acquire(32); // reuse
        pool.release(buf);
        let _buf2 = pool.acquire(32); // reuse

        assert!(pool.reuse_rate() > 0.0);
    }

    #[test]
    fn test_resource_pool_shrink() {
        let mut pool = AxisResourcePool::new(10, 32);
        pool.shrink_to(2);
        assert_eq!(pool.available_count(), 2);
    }

    // -- Label culling tests --

    #[test]
    fn test_cull_label_indices_all_visible() {
        let viewport = ViewportBounds::from_size(800.0, 600.0);
        let positions = [[100.0, 100.0], [400.0, 300.0], [700.0, 500.0]];

        let visible = cull_label_indices(&positions, &viewport, 0.0);
        assert_eq!(visible, vec![0, 1, 2]);
    }

    #[test]
    fn test_cull_label_indices_some_outside() {
        let viewport = ViewportBounds::from_size(800.0, 600.0);
        let positions = [[-50.0, 100.0], [400.0, 300.0], [900.0, 700.0]];

        let visible = cull_label_indices(&positions, &viewport, 0.0);
        assert_eq!(visible, vec![1]);
    }

    #[test]
    fn test_cull_label_indices_with_margin() {
        let viewport = ViewportBounds::from_size(800.0, 600.0);
        let positions = [[-10.0, 100.0], [810.0, 300.0]];

        // Without margin, both culled
        let visible_no_margin = cull_label_indices(&positions, &viewport, 0.0);
        assert!(visible_no_margin.is_empty());

        // With 20px margin, both visible
        let visible_with_margin = cull_label_indices(&positions, &viewport, 20.0);
        assert_eq!(visible_with_margin, vec![0, 1]);
    }

    // -- Render stats tests --

    #[test]
    fn test_render_stats_aggregation() {
        let mut stats = AxisSystemRenderStats::new();
        stats.add_axis(
            "x",
            AxisRenderStats {
                vertex_count: 14,
                label_count: 6,
                labels_culled: 1,
                cache_hit: true,
                lod: Some(LODLevel::High),
                render_time: Duration::from_micros(100),
            },
        );
        stats.add_axis(
            "y",
            AxisRenderStats {
                vertex_count: 12,
                label_count: 5,
                labels_culled: 0,
                cache_hit: false,
                lod: Some(LODLevel::Medium),
                render_time: Duration::from_micros(150),
            },
        );

        assert_eq!(stats.total_vertices, 26);
        assert_eq!(stats.total_labels, 11);
        assert_eq!(stats.total_labels_culled, 1);
        assert_eq!(stats.per_axis.len(), 2);
    }
}

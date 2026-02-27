// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Utilities and builders for async compositions.

use super::{AsyncMixable, AsyncRenderStrategy, RenderProgress, TimeoutComposition};
use crate::{GupError, GupResult, RenderContext};
use async_trait::async_trait;
use futures::future;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

/// Multi-component async composition that can handle any number of components.
pub struct MultiAsyncComposition<T> {
    components: Vec<T>,
    strategy: AsyncRenderStrategy,
    progress_tracker: ProgressTracker,
    cancellation_token: CancellationToken,
}

impl<T> MultiAsyncComposition<T>
where
    T: AsyncMixable + 'static,
{
    /// Create a new multi-component composition.
    pub fn new(components: Vec<T>, strategy: AsyncRenderStrategy) -> Self {
        Self {
            components,
            strategy,
            progress_tracker: ProgressTracker::new(),
            cancellation_token: CancellationToken::new(),
        }
    }

    /// Create with default adaptive strategy.
    pub fn with_components(components: Vec<T>) -> Self {
        Self::new(components, AsyncRenderStrategy::Adaptive)
    }

    /// Set the render strategy.
    pub fn with_strategy(mut self, strategy: AsyncRenderStrategy) -> Self {
        self.strategy = strategy;
        self
    }

    /// Add a component to the composition.
    pub fn add_component(mut self, component: T) -> Self {
        self.components.push(component);
        self
    }

    /// Get the number of components.
    pub fn component_count(&self) -> usize {
        self.components.len()
    }

    /// Check if the composition is empty.
    pub fn is_empty(&self) -> bool {
        self.components.is_empty()
    }

    /// Get the render strategy.
    pub fn strategy(&self) -> AsyncRenderStrategy {
        self.strategy
    }
}

impl<T> std::fmt::Debug for MultiAsyncComposition<T>
where
    T: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MultiAsyncComposition")
            .field("component_count", &self.components.len())
            .field("strategy", &self.strategy)
            .field("cancelled", &self.cancellation_token.is_cancelled())
            .finish()
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<T> AsyncMixable for MultiAsyncComposition<T>
where
    T: AsyncMixable + 'static,
{
    type Output = ();

    async fn render_async(
        &self,
        context: &mut RenderContext,
        _cancellation: CancellationToken,
    ) -> GupResult<()> {
        if self.components.is_empty() {
            return Ok(());
        }

        // Link cancellation tokens
        let combined_token = CancellationToken::new();

        match self.strategy {
            AsyncRenderStrategy::Sequential => {
                for (i, component) in self.components.iter().enumerate() {
                    if combined_token.is_cancelled() {
                        return Err(GupError::render_error(format!(
                            "Rendering cancelled at component {i}"
                        )));
                    }
                    component
                        .render_async(context, combined_token.clone())
                        .await?;
                }
            }
            AsyncRenderStrategy::Parallel => {
                // For parallel rendering, fall back to sequential due to context mutability constraints
                for (i, component) in self.components.iter().enumerate() {
                    if combined_token.is_cancelled() {
                        return Err(GupError::render_error(format!(
                            "Rendering cancelled at component {i}"
                        )));
                    }
                    component
                        .render_async(context, combined_token.clone())
                        .await
                        .map_err(|e| {
                            GupError::render_error(format!("Component {i} failed: {e}"))
                        })?;
                }
            }
            AsyncRenderStrategy::Adaptive => {
                // Check readiness of all components
                let readiness_futures: Vec<_> =
                    self.components.iter().map(|c| c.is_ready()).collect();

                let readiness_results = future::join_all(readiness_futures).await;
                let _all_ready = readiness_results.iter().all(|&ready| ready);

                // For adaptive strategy, use sequential rendering for now
                for (i, component) in self.components.iter().enumerate() {
                    if combined_token.is_cancelled() {
                        return Err(GupError::render_error(format!(
                            "Rendering cancelled at component {i}"
                        )));
                    }
                    component
                        .render_async(context, combined_token.clone())
                        .await
                        .map_err(|e| {
                            GupError::render_error(format!("Component {i} failed: {e}"))
                        })?;
                }
            }
        }

        Ok(())
    }

    async fn is_ready(&self) -> bool {
        if self.components.is_empty() {
            return true;
        }

        let readiness_futures: Vec<_> = self.components.iter().map(|c| c.is_ready()).collect();
        let results = future::join_all(readiness_futures).await;
        results.iter().all(|&ready| ready)
    }

    fn progress(&self) -> Option<RenderProgress> {
        let progresses: Vec<_> = self
            .components
            .iter()
            .filter_map(|c| c.progress())
            .collect();

        self.progress_tracker.aggregate_progress(&progresses)
    }

    fn cancel(&self) {
        self.cancellation_token.cancel();
        for component in &self.components {
            component.cancel();
        }
    }

    fn description(&self) -> String {
        format!(
            "MultiAsyncComposition({} components, {:?})",
            self.components.len(),
            self.strategy
        )
    }

    fn is_valid(&self) -> bool {
        !self.cancellation_token.is_cancelled() && self.components.iter().all(|c| c.is_valid())
    }
}

/// Progress tracking for multi-component compositions.
pub struct ProgressTracker {
    weight_by_component_count: bool,
}

impl ProgressTracker {
    /// Create a new progress tracker.
    pub fn new() -> Self {
        Self {
            weight_by_component_count: true,
        }
    }

    /// Set whether to weight progress by component count.
    pub fn with_component_weighting(mut self, enabled: bool) -> Self {
        self.weight_by_component_count = enabled;
        self
    }

    /// Aggregate progress from multiple components.
    pub fn aggregate_progress(&self, progresses: &[RenderProgress]) -> Option<RenderProgress> {
        if progresses.is_empty() {
            return None;
        }

        let total_current: u64 = progresses.iter().map(|p| p.current).sum();

        let total_total: Option<u64> = {
            let totals: Vec<u64> = progresses.iter().filter_map(|p| p.total).collect();
            if totals.len() == progresses.len() {
                Some(totals.iter().sum())
            } else {
                None
            }
        };

        let combined_stage = if progresses.len() == 1 {
            progresses[0].stage.clone()
        } else {
            let stages: Vec<&str> = progresses.iter().map(|p| p.stage.as_str()).collect();
            if stages.len() <= 3 {
                stages.join(", ")
            } else {
                format!(
                    "{}, {} and {} others",
                    stages[0],
                    stages[1],
                    stages.len() - 2
                )
            }
        };

        // Estimate time remaining based on progress rates
        let estimated_time_remaining = self.estimate_remaining_time(progresses);

        Some(RenderProgress {
            current: total_current,
            total: total_total,
            stage: combined_stage,
            estimated_time_remaining,
        })
    }

    /// Estimate remaining time based on progress patterns.
    fn estimate_remaining_time(&self, progresses: &[RenderProgress]) -> Option<Duration> {
        // This is a simplified estimation - a more sophisticated implementation
        // could track progress rates over time
        let complete_count = progresses
            .iter()
            .filter(|p| p.percentage().is_some_and(|pct| pct >= 1.0))
            .count();

        let incomplete_count = progresses.len() - complete_count;
        if incomplete_count == 0 {
            return Some(Duration::from_secs(0));
        }

        // Very rough estimation: assume remaining components take similar time
        // In practice, this would use historical timing data
        let estimated_seconds_per_component = 1.0; // 1 second per component estimate
        Some(Duration::from_secs_f64(
            incomplete_count as f64 * estimated_seconds_per_component,
        ))
    }
}

impl Default for ProgressTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for creating async compositions with a fluent API.
pub struct AsyncCompositionBuilder<T> {
    components: Vec<T>,
    strategy: AsyncRenderStrategy,
    timeout: Option<Duration>,
}

impl<T> AsyncCompositionBuilder<T>
where
    T: AsyncMixable + 'static,
{
    /// Create a new builder.
    pub fn new() -> Self {
        Self {
            components: Vec::new(),
            strategy: AsyncRenderStrategy::Adaptive,
            timeout: None,
        }
    }

    /// Add a component to the composition.
    pub fn add_component(mut self, component: T) -> Self {
        self.components.push(component);
        self
    }

    /// Add multiple components.
    pub fn add_components(mut self, components: Vec<T>) -> Self {
        self.components.extend(components);
        self
    }

    /// Set the render strategy.
    pub fn with_strategy(mut self, strategy: AsyncRenderStrategy) -> Self {
        self.strategy = strategy;
        self
    }

    /// Set a timeout for the entire composition.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Build the composition.
    pub fn build(self) -> GupResult<Box<dyn AsyncMixable<Output = ()>>> {
        if self.components.is_empty() {
            return Err(GupError::render_error(
                "Cannot build composition with no components".to_string(),
            ));
        }

        let composition = MultiAsyncComposition::new(self.components, self.strategy);

        if let Some(timeout) = self.timeout {
            Ok(Box::new(TimeoutComposition::new(composition, timeout)))
        } else {
            Ok(Box::new(composition))
        }
    }

    /// Build into a specific composition type.
    pub fn build_typed(self) -> GupResult<MultiAsyncComposition<T>> {
        if self.components.is_empty() {
            return Err(GupError::render_error(
                "Cannot build composition with no components".to_string(),
            ));
        }

        Ok(MultiAsyncComposition::new(self.components, self.strategy))
    }

    /// Get the current component count.
    pub fn component_count(&self) -> usize {
        self.components.len()
    }

    /// Check if the builder is empty.
    pub fn is_empty(&self) -> bool {
        self.components.is_empty()
    }
}

impl<T> Default for AsyncCompositionBuilder<T>
where
    T: AsyncMixable + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

/// Convenience functions for creating async compositions.
pub mod compose {
    use super::*;

    /// Compose multiple async components with default adaptive strategy.
    pub fn all<T>(components: Vec<T>) -> GupResult<MultiAsyncComposition<T>>
    where
        T: AsyncMixable + 'static,
    {
        AsyncCompositionBuilder::new()
            .add_components(components)
            .build_typed()
    }

    /// Compose with sequential strategy.
    pub fn sequential<T>(components: Vec<T>) -> GupResult<MultiAsyncComposition<T>>
    where
        T: AsyncMixable + 'static,
    {
        AsyncCompositionBuilder::new()
            .add_components(components)
            .with_strategy(AsyncRenderStrategy::Sequential)
            .build_typed()
    }

    /// Compose with parallel strategy.
    pub fn parallel<T>(components: Vec<T>) -> GupResult<MultiAsyncComposition<T>>
    where
        T: AsyncMixable + 'static,
    {
        AsyncCompositionBuilder::new()
            .add_components(components)
            .with_strategy(AsyncRenderStrategy::Parallel)
            .build_typed()
    }

    /// Compose with adaptive strategy.
    pub fn adaptive<T>(components: Vec<T>) -> GupResult<MultiAsyncComposition<T>>
    where
        T: AsyncMixable + 'static,
    {
        AsyncCompositionBuilder::new()
            .add_components(components)
            .with_strategy(AsyncRenderStrategy::Adaptive)
            .build_typed()
    }

    /// Compose with timeout.
    pub fn with_timeout<T>(
        components: Vec<T>,
        timeout: Duration,
    ) -> GupResult<TimeoutComposition<MultiAsyncComposition<T>>>
    where
        T: AsyncMixable + 'static,
    {
        let composition = AsyncCompositionBuilder::new()
            .add_components(components)
            .build_typed()?;

        Ok(TimeoutComposition::new(composition, timeout))
    }
}

/// Performance monitoring for async compositions.
pub struct AsyncPerformanceMonitor {
    render_times: Vec<Duration>,
    component_times: Vec<(usize, Duration)>,
    max_samples: usize,
}

impl AsyncPerformanceMonitor {
    /// Create a new performance monitor.
    pub fn new() -> Self {
        Self {
            render_times: Vec::new(),
            component_times: Vec::new(),
            max_samples: 100,
        }
    }

    /// Set the maximum number of samples to keep.
    pub fn with_max_samples(mut self, max_samples: usize) -> Self {
        self.max_samples = max_samples;
        self
    }

    /// Record a render time.
    pub fn record_render_time(&mut self, duration: Duration) {
        self.render_times.push(duration);
        if self.render_times.len() > self.max_samples {
            self.render_times.remove(0);
        }
    }

    /// Record component-specific timing.
    pub fn record_component_time(&mut self, component_index: usize, duration: Duration) {
        self.component_times.push((component_index, duration));
        if self.component_times.len() > self.max_samples * 10 {
            // Keep more component samples for detailed analysis
            self.component_times.remove(0);
        }
    }

    /// Get average render time.
    pub fn average_render_time(&self) -> Option<Duration> {
        if self.render_times.is_empty() {
            return None;
        }

        let total: Duration = self.render_times.iter().sum();
        Some(total / self.render_times.len() as u32)
    }

    /// Get render time percentiles.
    pub fn render_time_percentile(&self, percentile: f32) -> Option<Duration> {
        if self.render_times.is_empty() {
            return None;
        }

        let mut sorted = self.render_times.clone();
        sorted.sort();

        let index = ((percentile / 100.0) * (sorted.len() - 1) as f32) as usize;
        Some(sorted[index])
    }

    /// Get component performance statistics.
    pub fn component_stats(&self, component_index: usize) -> ComponentStats {
        let component_times: Vec<Duration> = self
            .component_times
            .iter()
            .filter_map(|(idx, duration)| {
                if *idx == component_index {
                    Some(*duration)
                } else {
                    None
                }
            })
            .collect();

        ComponentStats::new(component_index, component_times)
    }

    /// Check if performance is degrading.
    pub fn is_performance_degrading(&self, threshold_factor: f32) -> bool {
        if self.render_times.len() < 10 {
            return false; // Not enough data
        }

        let recent_avg = {
            let recent_count = self.render_times.len().min(5);
            let recent_times = &self.render_times[self.render_times.len() - recent_count..];
            let total: Duration = recent_times.iter().sum();
            total / recent_count as u32
        };

        if let Some(overall_avg) = self.average_render_time() {
            recent_avg.as_secs_f32() > overall_avg.as_secs_f32() * threshold_factor
        } else {
            false
        }
    }

    /// Reset all statistics.
    pub fn reset(&mut self) {
        self.render_times.clear();
        self.component_times.clear();
    }
}

impl Default for AsyncPerformanceMonitor {
    fn default() -> Self {
        Self::new()
    }
}

/// Performance statistics for a specific component.
#[derive(Debug, Clone)]
pub struct ComponentStats {
    pub component_index: usize,
    pub sample_count: usize,
    pub average_time: Option<Duration>,
    pub min_time: Option<Duration>,
    pub max_time: Option<Duration>,
}

impl ComponentStats {
    /// Create component statistics from timing data.
    pub fn new(component_index: usize, times: Vec<Duration>) -> Self {
        if times.is_empty() {
            return Self {
                component_index,
                sample_count: 0,
                average_time: None,
                min_time: None,
                max_time: None,
            };
        }

        let total: Duration = times.iter().sum();
        let average_time = Some(total / times.len() as u32);
        let min_time = times.iter().min().copied();
        let max_time = times.iter().max().copied();

        Self {
            component_index,
            sample_count: times.len(),
            average_time,
            min_time,
            max_time,
        }
    }

    /// Check if this component is performing well.
    pub fn is_performing_well(&self, target_time: Duration) -> bool {
        self.average_time.is_none_or(|avg| avg <= target_time)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::async_mixable::tests::TestAsyncComponent;
    use std::time::Duration;

    #[tokio::test]
    async fn test_multi_async_composition_creation() {
        let components = vec![
            TestAsyncComponent::new("comp1"),
            TestAsyncComponent::new("comp2"),
            TestAsyncComponent::new("comp3"),
        ];

        let composition = MultiAsyncComposition::with_components(components);
        assert_eq!(composition.component_count(), 3);
        assert!(!composition.is_empty());
        assert_eq!(composition.strategy(), AsyncRenderStrategy::Adaptive);
    }

    #[tokio::test]
    async fn test_multi_composition_readiness() {
        let ready_comp = TestAsyncComponent::new("ready").with_ready_state(true);
        let not_ready_comp = TestAsyncComponent::new("not_ready").with_ready_state(false);

        let composition = MultiAsyncComposition::with_components(vec![ready_comp, not_ready_comp]);
        assert!(!composition.is_ready().await);

        let all_ready = MultiAsyncComposition::with_components(vec![
            TestAsyncComponent::new("ready1").with_ready_state(true),
            TestAsyncComponent::new("ready2").with_ready_state(true),
        ]);
        assert!(all_ready.is_ready().await);
    }

    #[tokio::test]
    async fn test_async_composition_builder() {
        let builder = AsyncCompositionBuilder::new()
            .add_component(TestAsyncComponent::new("comp1"))
            .add_component(TestAsyncComponent::new("comp2"))
            .with_strategy(AsyncRenderStrategy::Parallel)
            .with_timeout(Duration::from_secs(5));

        assert_eq!(builder.component_count(), 2);
        assert!(!builder.is_empty());

        let composition = builder.build();
        assert!(composition.is_ok());
    }

    #[tokio::test]
    async fn test_empty_composition_error() {
        let builder = AsyncCompositionBuilder::<TestAsyncComponent>::new();
        let result = builder.build();

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("no components"));
    }

    #[tokio::test]
    async fn test_compose_utilities() {
        let components = vec![
            TestAsyncComponent::new("comp1"),
            TestAsyncComponent::new("comp2"),
        ];

        let sequential = compose::sequential(components.clone()).unwrap();
        assert_eq!(sequential.strategy(), AsyncRenderStrategy::Sequential);

        let parallel = compose::parallel(components.clone()).unwrap();
        assert_eq!(parallel.strategy(), AsyncRenderStrategy::Parallel);

        let adaptive = compose::adaptive(components).unwrap();
        assert_eq!(adaptive.strategy(), AsyncRenderStrategy::Adaptive);
    }

    #[tokio::test]
    async fn test_compose_with_timeout() {
        let components = vec![
            TestAsyncComponent::new("comp1"),
            TestAsyncComponent::new("comp2"),
        ];

        let timeout_composition = compose::with_timeout(components, Duration::from_millis(100));
        assert!(timeout_composition.is_ok());

        let composition = timeout_composition.unwrap();
        assert_eq!(composition.timeout(), Duration::from_millis(100));
    }

    #[test]
    fn test_progress_tracker() {
        let tracker = ProgressTracker::new();

        let progresses = vec![
            RenderProgress::new(25, Some(100), "Loading data"),
            RenderProgress::new(50, Some(200), "Processing"),
        ];

        let combined = tracker.aggregate_progress(&progresses);
        assert!(combined.is_some());

        let progress = combined.unwrap();
        assert_eq!(progress.current, 75);
        assert_eq!(progress.total, Some(300));
        assert!(progress.stage.contains("Loading data"));
        assert!(progress.stage.contains("Processing"));
    }

    #[test]
    fn test_progress_tracker_empty() {
        let tracker = ProgressTracker::new();
        let empty_progresses = vec![];

        let result = tracker.aggregate_progress(&empty_progresses);
        assert!(result.is_none());
    }

    #[test]
    fn test_performance_monitor() {
        let mut monitor = AsyncPerformanceMonitor::new();

        monitor.record_render_time(Duration::from_millis(10));
        monitor.record_render_time(Duration::from_millis(20));
        monitor.record_render_time(Duration::from_millis(30));

        let avg = monitor.average_render_time().unwrap();
        assert_eq!(avg.as_millis(), 20);

        let p50 = monitor.render_time_percentile(50.0).unwrap();
        assert_eq!(p50.as_millis(), 20);

        assert!(!monitor.is_performance_degrading(2.0)); // Not enough recent samples
    }

    #[test]
    fn test_performance_monitor_component_stats() {
        let mut monitor = AsyncPerformanceMonitor::new();

        monitor.record_component_time(0, Duration::from_millis(15));
        monitor.record_component_time(0, Duration::from_millis(25));
        monitor.record_component_time(1, Duration::from_millis(5));

        let comp0_stats = monitor.component_stats(0);
        assert_eq!(comp0_stats.component_index, 0);
        assert_eq!(comp0_stats.sample_count, 2);
        assert_eq!(comp0_stats.average_time.unwrap().as_millis(), 20);

        let comp1_stats = monitor.component_stats(1);
        assert_eq!(comp1_stats.sample_count, 1);
    }

    #[test]
    fn test_component_stats_performance_check() {
        let times = vec![
            Duration::from_millis(10),
            Duration::from_millis(20),
            Duration::from_millis(15),
        ];

        let stats = ComponentStats::new(0, times);
        assert!(stats.is_performing_well(Duration::from_millis(20))); // Average is ~15ms
        assert!(!stats.is_performing_well(Duration::from_millis(10))); // Average exceeds target
    }

    #[test]
    fn test_performance_degradation_detection() {
        let mut monitor = AsyncPerformanceMonitor::new();

        // Add some baseline times
        for _ in 0..10 {
            monitor.record_render_time(Duration::from_millis(10));
        }

        // Add some recent slower times
        for _ in 0..5 {
            monitor.record_render_time(Duration::from_millis(25));
        }

        assert!(monitor.is_performance_degrading(1.5)); // Recent times are 2.5x baseline
        assert!(!monitor.is_performance_degrading(3.0)); // Within 3x threshold
    }
}

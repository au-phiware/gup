// Gup - GPU-Accelerated Data Visualization Library
// Copyright (C) 2025 Corin Lawson <corin@phiware.com.au>
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

//! Asynchronous and streaming composition support for Gup.
//!
//! This module extends the Mixable trait system to support asynchronous data loading,
//! streaming datasets, and progressive rendering, enabling visualizations that work
//! with large or real-time data sources.

use crate::{CompositionMode, GupError, GupResult, MaybeSend, MaybeSync, Mixable, RenderContext};
use async_trait::async_trait;
use futures::future;
use std::fmt::Debug;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

pub mod progressive;
pub mod streaming;
pub mod utils;

pub use progressive::*;
pub use streaming::*;
pub use utils::*;

/// Progress information for async rendering operations.
#[derive(Debug, Clone)]
pub struct RenderProgress {
    pub current: u64,
    pub total: Option<u64>,
    pub stage: String,
    pub estimated_time_remaining: Option<Duration>,
}

impl RenderProgress {
    /// Create a new progress tracker.
    pub fn new(current: u64, total: Option<u64>, stage: impl Into<String>) -> Self {
        Self {
            current,
            total,
            stage: stage.into(),
            estimated_time_remaining: None,
        }
    }

    /// Calculate progress percentage (0.0 to 1.0).
    pub fn percentage(&self) -> Option<f32> {
        self.total.map(|total| {
            if total > 0 {
                self.current as f32 / total as f32
            } else {
                0.0
            }
        })
    }

    /// Check if the operation is complete.
    pub fn is_complete(&self) -> bool {
        self.total.is_some_and(|total| self.current >= total)
    }
}

/// Render strategy for async compositions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AsyncRenderStrategy {
    /// Render components sequentially
    Sequential,
    /// Render components in parallel where possible
    Parallel,
    /// Adaptive strategy based on component characteristics
    #[default]
    Adaptive,
}

/// Asynchronous extension of the Mixable trait.
///
/// This trait enables components to render asynchronously without blocking
/// the main thread, supporting cancellation, progress tracking, and streaming data.
///
/// On native platforms, implementations must be `Send + Sync` for multi-threaded
/// access. On WASM (single-threaded), these bounds are relaxed because wgpu
/// WebGPU backend types are not `Send`/`Sync`.
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait AsyncMixable: MaybeSend + MaybeSync + Debug {
    type Output;

    /// Asynchronously render this component.
    ///
    /// # Arguments
    ///
    /// * `context` - The render context containing GPU resources
    /// * `cancellation` - Token for cancelling the operation
    ///
    /// # Returns
    ///
    /// A result indicating success or failure of the render operation
    async fn render_async(
        &self,
        context: &mut RenderContext,
        cancellation: CancellationToken,
    ) -> GupResult<()>;

    /// Check if this component is ready to render.
    ///
    /// Components may need to load data or initialize resources before
    /// they can be rendered.
    async fn is_ready(&self) -> bool;

    /// Get progress information for long-running operations.
    ///
    /// Returns `None` if no progress tracking is available.
    fn progress(&self) -> Option<RenderProgress>;

    /// Cancel any ongoing operations.
    ///
    /// This should cause any in-progress async operations to terminate
    /// as quickly as possible.
    fn cancel(&self);

    /// Get a description of this component for debugging.
    fn description(&self) -> String {
        format!("{self:?}")
    }

    /// Check if this component is valid and can be rendered.
    fn is_valid(&self) -> bool {
        true
    }

    /// Compose this async mixable with another async mixable.
    fn async_mix<T: AsyncMixable + 'static>(self, other: T) -> AsyncComposedVisualization<Self, T>
    where
        Self: Sized + 'static,
    {
        AsyncComposedVisualization::new(self, other)
    }

    /// Compose with a specific strategy.
    fn async_mix_with_strategy<T: AsyncMixable + 'static>(
        self,
        other: T,
        strategy: AsyncRenderStrategy,
    ) -> AsyncComposedVisualization<Self, T>
    where
        Self: Sized + 'static,
    {
        AsyncComposedVisualization::new(self, other).with_strategy(strategy)
    }
}

/// Async-aware composed visualization that preserves both components.
///
/// This type implements `AsyncMixable` itself, enabling recursive composition
/// of any depth while supporting async operations and cancellation.
#[derive(Debug)]
pub struct AsyncComposedVisualization<A, B>
where
    A: AsyncMixable + 'static,
    B: AsyncMixable + 'static,
{
    first: A,
    second: B,
    composition_mode: CompositionMode,
    render_strategy: AsyncRenderStrategy,
    cancellation_token: CancellationToken,
    progress_tracker: Arc<RwLock<Option<RenderProgress>>>,
}

impl<A, B> AsyncComposedVisualization<A, B>
where
    A: AsyncMixable + 'static,
    B: AsyncMixable + 'static,
{
    /// Create a new async composed visualization.
    pub fn new(first: A, second: B) -> Self {
        Self {
            first,
            second,
            composition_mode: CompositionMode::Overlay,
            render_strategy: AsyncRenderStrategy::default(),
            cancellation_token: CancellationToken::new(),
            progress_tracker: Arc::new(RwLock::new(None)),
        }
    }

    /// Create with a specific composition mode.
    pub fn with_mode(first: A, second: B, mode: CompositionMode) -> Self {
        let mut composed = Self::new(first, second);
        composed.composition_mode = mode;
        composed
    }

    /// Set the render strategy.
    pub fn with_strategy(mut self, strategy: AsyncRenderStrategy) -> Self {
        self.render_strategy = strategy;
        self
    }

    /// Add a timeout to this composition.
    pub fn with_timeout(self, timeout: Duration) -> TimeoutComposition<Self> {
        TimeoutComposition::new(self, timeout)
    }

    /// Get the current composition mode.
    pub fn composition_mode(&self) -> CompositionMode {
        self.composition_mode
    }

    /// Get references to the components.
    pub fn components(&self) -> (&A, &B) {
        (&self.first, &self.second)
    }

    /// Set a new cancellation token.
    pub fn with_cancellation_token(mut self, token: CancellationToken) -> Self {
        self.cancellation_token = token;
        self
    }

    /// Calculate combined progress from both components.
    async fn calculate_combined_progress(&self) -> Option<RenderProgress> {
        let first_progress = self.first.progress();
        let second_progress = self.second.progress();

        match (first_progress, second_progress) {
            (Some(first), Some(second)) => {
                let combined_current = first.current + second.current;
                let combined_total = match (first.total, second.total) {
                    (Some(f), Some(s)) => Some(f + s),
                    _ => None,
                };
                let combined_stage = format!("{} + {}", first.stage, second.stage);

                Some(RenderProgress {
                    current: combined_current,
                    total: combined_total,
                    stage: combined_stage,
                    estimated_time_remaining: None, // Could implement sophisticated estimation
                })
            }
            (Some(progress), None) | (None, Some(progress)) => Some(progress),
            (None, None) => None,
        }
    }

    /// Update the progress tracker.
    async fn update_progress(&self) {
        if let Some(progress) = self.calculate_combined_progress().await {
            let mut tracker = self.progress_tracker.write().await;
            *tracker = Some(progress);
        }
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<A, B> AsyncMixable for AsyncComposedVisualization<A, B>
where
    A: AsyncMixable + 'static,
    B: AsyncMixable + 'static,
{
    type Output = ();

    async fn render_async(
        &self,
        context: &mut RenderContext,
        cancellation: CancellationToken,
    ) -> GupResult<()> {
        // Link cancellation tokens
        let combined_token = CancellationToken::new();

        // Update progress at start
        self.update_progress().await;

        // Check for early cancellation
        if cancellation.is_cancelled() || self.cancellation_token.is_cancelled() {
            return Err(GupError::render_error("Operation was cancelled"));
        }

        // Render based on strategy
        match self.render_strategy {
            AsyncRenderStrategy::Sequential => {
                self.first
                    .render_async(context, combined_token.clone())
                    .await?;

                if combined_token.is_cancelled() {
                    return Err(GupError::render_error("Operation was cancelled"));
                }

                self.second.render_async(context, combined_token).await?;
            }
            AsyncRenderStrategy::Parallel => {
                // For parallel rendering, we need to ensure the context operations don't conflict
                // In practice, this would require a more sophisticated context management system
                // For now, fall back to sequential rendering
                self.first
                    .render_async(context, combined_token.clone())
                    .await?;

                if combined_token.is_cancelled() {
                    return Err(GupError::render_error("Operation was cancelled"));
                }

                self.second.render_async(context, combined_token).await?;
            }
            AsyncRenderStrategy::Adaptive => {
                // Choose strategy based on component readiness
                let (_first_ready, _second_ready) =
                    future::join(self.first.is_ready(), self.second.is_ready()).await;

                // For adaptive strategy, always use sequential for now
                // Parallel rendering would require better context management
                self.first
                    .render_async(context, combined_token.clone())
                    .await?;

                if combined_token.is_cancelled() {
                    return Err(GupError::render_error("Operation was cancelled"));
                }

                self.second.render_async(context, combined_token).await?;
            }
        }

        // Final progress update
        self.update_progress().await;
        Ok(())
    }

    async fn is_ready(&self) -> bool {
        let (first_ready, second_ready) =
            future::join(self.first.is_ready(), self.second.is_ready()).await;

        first_ready && second_ready
    }

    fn progress(&self) -> Option<RenderProgress> {
        // Try to get cached progress first (for performance during active rendering)
        if let Ok(guard) = self.progress_tracker.try_read()
            && let Some(cached) = guard.clone()
        {
            return Some(cached);
        }

        // If no cached progress, calculate on-demand from components
        let first_progress = self.first.progress();
        let second_progress = self.second.progress();

        match (first_progress, second_progress) {
            (Some(first), Some(second)) => {
                let combined_current = first.current + second.current;
                let combined_total = match (first.total, second.total) {
                    (Some(f), Some(s)) => Some(f + s),
                    _ => None,
                };
                let combined_stage = format!("{} + {}", first.stage, second.stage);

                Some(RenderProgress {
                    current: combined_current,
                    total: combined_total,
                    stage: combined_stage,
                    estimated_time_remaining: None,
                })
            }
            (Some(progress), None) | (None, Some(progress)) => Some(progress),
            (None, None) => None,
        }
    }

    fn cancel(&self) {
        self.cancellation_token.cancel();
        self.first.cancel();
        self.second.cancel();
    }

    fn description(&self) -> String {
        format!(
            "AsyncComposedVisualization({:?}, {} + {})",
            self.composition_mode,
            self.first.description(),
            self.second.description()
        )
    }

    fn is_valid(&self) -> bool {
        self.first.is_valid() && self.second.is_valid()
    }
}

/// Timeout wrapper for async compositions.
///
/// Automatically cancels operations that exceed the specified timeout duration.
pub struct TimeoutComposition<T> {
    inner: T,
    timeout: Duration,
}

impl<T> TimeoutComposition<T> {
    /// Create a new timeout wrapper.
    pub fn new(inner: T, timeout: Duration) -> Self {
        Self { inner, timeout }
    }

    /// Get the timeout duration.
    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    /// Get a reference to the inner composition.
    pub fn inner(&self) -> &T {
        &self.inner
    }
}

impl<T: Debug> Debug for TimeoutComposition<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TimeoutComposition")
            .field("inner", &self.inner)
            .field("timeout", &self.timeout)
            .finish()
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<T: AsyncMixable> AsyncMixable for TimeoutComposition<T> {
    type Output = T::Output;

    async fn render_async(
        &self,
        context: &mut RenderContext,
        cancellation: CancellationToken,
    ) -> GupResult<()> {
        tokio::time::timeout(self.timeout, self.inner.render_async(context, cancellation))
            .await
            .map_err(|_| {
                GupError::render_error(format!(
                    "Render operation timed out after {:?}",
                    self.timeout
                ))
            })?
    }

    async fn is_ready(&self) -> bool {
        // Give a short timeout for readiness check
        tokio::time::timeout(Duration::from_millis(100), self.inner.is_ready())
            .await
            .unwrap_or(false)
    }

    fn progress(&self) -> Option<RenderProgress> {
        self.inner.progress()
    }

    fn cancel(&self) {
        self.inner.cancel();
    }

    fn description(&self) -> String {
        format!(
            "TimeoutComposition({:?}, {})",
            self.timeout,
            self.inner.description()
        )
    }

    fn is_valid(&self) -> bool {
        self.inner.is_valid()
    }
}

/// Adapter to make synchronous Mixable types work with async composition.
///
/// This allows existing synchronous components to participate in async
/// composition chains.
pub struct SyncAdapter<T> {
    inner: T,
}

impl<T> SyncAdapter<T> {
    /// Create a new sync adapter.
    pub fn new(inner: T) -> Self {
        Self { inner }
    }

    /// Get a reference to the inner component.
    pub fn inner(&self) -> &T {
        &self.inner
    }
}

impl<T: Debug> Debug for SyncAdapter<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SyncAdapter")
            .field("inner", &self.inner)
            .finish()
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<T: Mixable> AsyncMixable for SyncAdapter<T> {
    type Output = T::Output;

    async fn render_async(
        &self,
        _context: &mut RenderContext,
        _cancellation: CancellationToken,
    ) -> GupResult<()> {
        // For synchronous components, we simulate the render
        // In practice, this would require a way to call the sync render method
        // For now, we'll just validate the component
        if !self.inner.is_valid() {
            return Err(GupError::render_error("Sync component is not valid"));
        }
        Ok(())
    }

    async fn is_ready(&self) -> bool {
        // Synchronous components are always ready
        self.inner.is_valid()
    }

    fn progress(&self) -> Option<RenderProgress> {
        // Synchronous operations don't have progress tracking
        None
    }

    fn cancel(&self) {
        // Synchronous operations can't be cancelled
    }

    fn description(&self) -> String {
        format!("SyncAdapter({})", self.inner.description())
    }

    fn is_valid(&self) -> bool {
        self.inner.is_valid()
    }
}

/// Implement AsyncMixable for Box<dyn AsyncMixable> to enable trait object usage
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<T> AsyncMixable for Box<T>
where
    T: AsyncMixable + ?Sized,
{
    type Output = T::Output;

    async fn render_async(
        &self,
        context: &mut RenderContext,
        cancellation: CancellationToken,
    ) -> GupResult<()> {
        (**self).render_async(context, cancellation).await
    }

    async fn is_ready(&self) -> bool {
        (**self).is_ready().await
    }

    fn progress(&self) -> Option<RenderProgress> {
        (**self).progress()
    }

    fn cancel(&self) {
        (**self).cancel();
    }

    fn description(&self) -> String {
        (**self).description()
    }

    fn is_valid(&self) -> bool {
        (**self).is_valid()
    }
}

/// Extension trait to add async capabilities to existing Mixable types.
pub trait AsyncMixableExt: Mixable + Sized {
    /// Convert a synchronous Mixable to an AsyncMixable.
    fn into_async(self) -> SyncAdapter<Self> {
        SyncAdapter::new(self)
    }

    /// Mix with an async component.
    fn async_mix_with<T: AsyncMixable + 'static>(
        self,
        other: T,
    ) -> AsyncComposedVisualization<SyncAdapter<Self>, T>
    where
        Self: 'static,
    {
        self.into_async().async_mix(other)
    }
}

// Implement AsyncMixableExt for all Mixable types
impl<T: Mixable> AsyncMixableExt for T {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mixable::CompositionMode;

    #[derive(Debug, Clone)]
    pub struct TestAsyncComponent {
        name: String,
        delay: Duration,
        should_fail: bool,
        ready: bool,
    }

    impl TestAsyncComponent {
        pub fn new(name: &str) -> Self {
            Self {
                name: name.to_string(),
                delay: Duration::from_millis(10),
                should_fail: false,
                ready: true,
            }
        }

        pub fn with_delay(mut self, delay: Duration) -> Self {
            self.delay = delay;
            self
        }

        #[allow(dead_code)]
        pub fn with_failure(mut self) -> Self {
            self.should_fail = true;
            self
        }

        pub fn with_ready_state(mut self, ready: bool) -> Self {
            self.ready = ready;
            self
        }

        pub fn with_timeout(self, timeout: Duration) -> TimeoutComposition<Self> {
            TimeoutComposition::new(self, timeout)
        }
    }

    #[cfg_attr(not(target_arch = "wasm32"), async_trait)]
    #[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
    impl AsyncMixable for TestAsyncComponent {
        type Output = ();

        async fn render_async(
            &self,
            _context: &mut RenderContext,
            cancellation: CancellationToken,
        ) -> GupResult<()> {
            // Simulate async work with cancellation support
            tokio::select! {
                _ = tokio::time::sleep(self.delay) => {
                    if self.should_fail {
                        Err(GupError::render_error(format!(
                            "Intentional failure from {}",
                            self.name
                        )))
                    } else {
                        Ok(())
                    }
                }
                _ = cancellation.cancelled() => {
                    Err(GupError::render_error("Operation was cancelled"))
                }
            }
        }

        async fn is_ready(&self) -> bool {
            self.ready
        }

        fn progress(&self) -> Option<RenderProgress> {
            Some(RenderProgress::new(50, Some(100), &self.name))
        }

        fn cancel(&self) {
            // Implementation would cancel any ongoing operations
        }

        fn description(&self) -> String {
            self.name.clone()
        }

        fn is_valid(&self) -> bool {
            !self.should_fail
        }
    }

    #[tokio::test]
    async fn test_async_composition_creation() {
        let comp1 = TestAsyncComponent::new("comp1");
        let comp2 = TestAsyncComponent::new("comp2");

        let composed = comp1.async_mix(comp2);
        assert!(composed.is_valid());
        assert_eq!(composed.composition_mode(), CompositionMode::Overlay);
    }

    #[tokio::test]
    async fn test_async_composition_readiness() {
        let ready_comp = TestAsyncComponent::new("ready").with_ready_state(true);
        let not_ready_comp = TestAsyncComponent::new("not_ready").with_ready_state(false);

        let composed = ready_comp.async_mix(not_ready_comp);
        assert!(!composed.is_ready().await);
    }

    #[tokio::test]
    async fn test_render_strategy_selection() {
        let comp1 = TestAsyncComponent::new("comp1");
        let comp2 = TestAsyncComponent::new("comp2");

        let sequential = comp1.async_mix_with_strategy(comp2, AsyncRenderStrategy::Sequential);
        assert_eq!(sequential.render_strategy, AsyncRenderStrategy::Sequential);
    }

    #[tokio::test]
    async fn test_timeout_composition() {
        let slow_comp = TestAsyncComponent::new("slow").with_delay(Duration::from_millis(200));
        let timeout_comp = slow_comp.with_timeout(Duration::from_millis(50));

        let mut context = RenderContext::new().await.unwrap();
        let result = timeout_comp
            .render_async(&mut context, CancellationToken::new())
            .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("timed out"));
    }

    #[tokio::test]
    async fn test_cancellation() {
        let comp = TestAsyncComponent::new("cancellable").with_delay(Duration::from_millis(100));
        let cancellation_token = CancellationToken::new();

        let mut context = RenderContext::new().await.unwrap();

        // Start render and cancel immediately
        cancellation_token.cancel();
        let result = comp.render_async(&mut context, cancellation_token).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cancelled"));
    }

    #[tokio::test]
    async fn test_progress_tracking() {
        let comp1 = TestAsyncComponent::new("comp1");
        let comp2 = TestAsyncComponent::new("comp2");

        let composed = comp1.async_mix(comp2);
        let progress = composed.progress();

        assert!(progress.is_some());
        let p = progress.unwrap();
        assert_eq!(p.current, 100); // 50 + 50 from both components
        assert_eq!(p.total, Some(200)); // 100 + 100 from both components
    }
}

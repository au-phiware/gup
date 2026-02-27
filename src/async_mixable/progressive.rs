// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Progressive loading support for large datasets.

use super::{AsyncMixable, RenderProgress};
use crate::{GupError, GupResult, MaybeSend, MaybeSync, RenderContext};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock};
use tokio_util::sync::CancellationToken;

/// Quality levels for progressive rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, PartialOrd, Ord)]
pub enum QualityLevel {
    /// Preview quality - 10% of data, fast loading
    #[default]
    Preview,
    /// Medium quality - 50% of data, balanced
    Medium,
    /// High quality - 90% of data, detailed
    High,
    /// Full quality - 100% of data, complete
    Full,
}

impl QualityLevel {
    /// Get the data percentage for this quality level.
    pub fn data_percentage(self) -> f32 {
        match self {
            QualityLevel::Preview => 0.1,
            QualityLevel::Medium => 0.5,
            QualityLevel::High => 0.9,
            QualityLevel::Full => 1.0,
        }
    }

    /// Get the next quality level.
    pub fn next(self) -> Option<Self> {
        match self {
            QualityLevel::Preview => Some(QualityLevel::Medium),
            QualityLevel::Medium => Some(QualityLevel::High),
            QualityLevel::High => Some(QualityLevel::Full),
            QualityLevel::Full => None,
        }
    }

    /// Get all quality levels in order.
    pub fn all() -> &'static [QualityLevel] {
        &[
            QualityLevel::Preview,
            QualityLevel::Medium,
            QualityLevel::High,
            QualityLevel::Full,
        ]
    }
}

/// Progress information for data loading operations.
#[derive(Debug, Clone)]
pub struct LoadProgress {
    pub chunks_loaded: usize,
    pub total_chunks: Option<usize>,
    pub current_quality: QualityLevel,
    pub target_quality: QualityLevel,
    pub bytes_loaded: u64,
    pub total_bytes: Option<u64>,
    pub loading_stage: String,
}

impl LoadProgress {
    /// Create new load progress.
    pub fn new(target_quality: QualityLevel) -> Self {
        Self {
            chunks_loaded: 0,
            total_chunks: None,
            current_quality: QualityLevel::Preview,
            target_quality,
            bytes_loaded: 0,
            total_bytes: None,
            loading_stage: "Initializing".to_string(),
        }
    }

    /// Calculate loading percentage.
    pub fn percentage(&self) -> f32 {
        if let Some(total) = self.total_chunks {
            let chunk_progress = self.chunks_loaded as f32 / total as f32;
            let byte_progress = if let Some(total_bytes) = self.total_bytes {
                if total_bytes > 0 {
                    self.bytes_loaded as f32 / total_bytes as f32
                } else {
                    0.0
                }
            } else {
                0.0
            };
            // Use the more accurate of the two progress indicators
            chunk_progress.max(byte_progress).min(1.0)
        } else {
            0.0
        }
    }

    /// Check if loading is complete for the target quality.
    pub fn is_complete(&self) -> bool {
        self.current_quality == self.target_quality
    }
}

/// Trait for progressive data loaders.
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
pub trait ProgressiveDataLoader<T>: MaybeSend + MaybeSync {
    /// Load a chunk of data at the specified offset and size.
    async fn load_chunk(&mut self, offset: usize, size: usize) -> GupResult<Vec<T>>;

    /// Get the total number of items available.
    fn total_size(&self) -> Option<usize>;

    /// Get the priority for loading a chunk (0.0 = lowest, 1.0 = highest).
    fn chunk_priority(&self, offset: usize) -> f32;

    /// Get the estimated size of a chunk in bytes.
    fn chunk_size_estimate(&self, size: usize) -> u64 {
        // Default estimate: 64 bytes per item
        size as u64 * 64
    }

    /// Check if the loader supports random access.
    fn supports_random_access(&self) -> bool {
        true
    }

    /// Get metadata about the dataset.
    fn metadata(&self) -> HashMap<String, String> {
        HashMap::new()
    }
}

/// Configuration for progressive loading.
#[derive(Debug, Clone)]
pub struct ProgressiveConfig {
    /// Default chunk size for loading operations
    pub chunk_size: usize,
    /// Target quality level to achieve
    pub target_quality: QualityLevel,
    /// Maximum time to spend loading per frame (to maintain responsiveness)
    pub max_loading_time_per_frame: Duration,
    /// Whether to enable background loading
    pub background_loading: bool,
    /// Priority-based loading strategy
    pub priority_loading: bool,
}

impl Default for ProgressiveConfig {
    fn default() -> Self {
        Self {
            chunk_size: 1000,
            target_quality: QualityLevel::High,
            max_loading_time_per_frame: Duration::from_millis(8), // ~120fps budget
            background_loading: true,
            priority_loading: true,
        }
    }
}

/// Progressive visualization that loads and renders data incrementally.
pub struct ProgressiveVisualization<T, L> {
    data_loader: Arc<Mutex<L>>,
    loaded_data: Arc<RwLock<Vec<T>>>,
    load_progress: Arc<RwLock<LoadProgress>>,
    config: ProgressiveConfig,
    cancellation_token: CancellationToken,
    background_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    chunk_cache: Arc<RwLock<HashMap<usize, Vec<T>>>>,
}

impl<T, L> ProgressiveVisualization<T, L>
where
    T: Clone + MaybeSend + MaybeSync + 'static,
    L: ProgressiveDataLoader<T> + 'static,
{
    /// Create a new progressive visualization.
    pub fn new(data_loader: L, config: ProgressiveConfig) -> Self {
        let data_loader = Arc::new(Mutex::new(data_loader));
        let loaded_data = Arc::new(RwLock::new(Vec::new()));
        let load_progress = Arc::new(RwLock::new(LoadProgress::new(config.target_quality)));
        let cancellation_token = CancellationToken::new();
        let chunk_cache = Arc::new(RwLock::new(HashMap::new()));

        let mut visualization = Self {
            data_loader: data_loader.clone(),
            loaded_data: loaded_data.clone(),
            load_progress: load_progress.clone(),
            config: config.clone(),
            cancellation_token: cancellation_token.clone(),
            background_task: Arc::new(Mutex::new(None)),
            chunk_cache: chunk_cache.clone(),
        };

        // Start background loading if enabled
        if config.background_loading {
            visualization.start_background_loading();
        }

        visualization
    }

    /// Create with default configuration.
    pub fn with_defaults(data_loader: L) -> Self {
        Self::new(data_loader, ProgressiveConfig::default())
    }

    /// Start background loading task.
    fn start_background_loading(&mut self) {
        let data_loader = self.data_loader.clone();
        let loaded_data = self.loaded_data.clone();
        let load_progress = self.load_progress.clone();
        let config = self.config.clone();
        let cancellation_token = self.cancellation_token.clone();
        let chunk_cache = self.chunk_cache.clone();

        #[cfg(not(target_arch = "wasm32"))]
        let task = tokio::spawn(async move {
            Self::background_loading_task(
                data_loader,
                loaded_data,
                load_progress,
                config,
                cancellation_token,
                chunk_cache,
            )
            .await;
        });
        #[cfg(target_arch = "wasm32")]
        let task = {
            // On WASM, spawn_local doesn't return a JoinHandle so we wrap in a
            // stub. Background loading still runs but cannot be joined.
            wasm_bindgen_futures::spawn_local(async move {
                Self::background_loading_task(
                    data_loader,
                    loaded_data,
                    load_progress,
                    config,
                    cancellation_token,
                    chunk_cache,
                )
                .await;
            });
            // Return a dummy handle that resolves immediately
            tokio::spawn(async {})
        };

        if let Ok(mut background_task) = self.background_task.try_lock() {
            *background_task = Some(task);
        }
    }

    /// Background task for progressive data loading.
    async fn background_loading_task(
        data_loader: Arc<Mutex<L>>,
        loaded_data: Arc<RwLock<Vec<T>>>,
        load_progress: Arc<RwLock<LoadProgress>>,
        config: ProgressiveConfig,
        cancellation_token: CancellationToken,
        chunk_cache: Arc<RwLock<HashMap<usize, Vec<T>>>>,
    ) {
        let mut current_quality = QualityLevel::Preview;
        let mut chunks_loaded = 0;

        while current_quality != config.target_quality {
            tokio::select! {
                _ = cancellation_token.cancelled() => break,
                _ = tokio::time::sleep(Duration::from_millis(16)) => {
                    // Load at ~60fps rhythm
                    let frame_start = Instant::now();

                    // Load chunks within time budget
                    while frame_start.elapsed() < config.max_loading_time_per_frame {
                        let chunk_loaded = Self::load_next_chunk(
                            &data_loader,
                            &loaded_data,
                            &load_progress,
                            &config,
                            &chunk_cache,
                            current_quality,
                            chunks_loaded,
                        ).await;

                        match chunk_loaded {
                            Ok(true) => {
                                chunks_loaded += 1;
                            }
                            Ok(false) => {
                                // No more chunks for this quality level
                                if let Some(next_quality) = current_quality.next() {
                                    current_quality = next_quality;
                                    chunks_loaded = 0;

                                    // Update progress
                                    if let Ok(mut progress) = load_progress.try_write() {
                                        progress.current_quality = current_quality;
                                    }
                                } else {
                                    break;
                                }
                            }
                            Err(_) => {
                                // Loading error, wait and retry
                                tokio::time::sleep(Duration::from_millis(100)).await;
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    /// Load the next chunk based on current quality level and priority.
    async fn load_next_chunk(
        data_loader: &Arc<Mutex<L>>,
        loaded_data: &Arc<RwLock<Vec<T>>>,
        load_progress: &Arc<RwLock<LoadProgress>>,
        config: &ProgressiveConfig,
        chunk_cache: &Arc<RwLock<HashMap<usize, Vec<T>>>>,
        current_quality: QualityLevel,
        chunks_loaded: usize,
    ) -> GupResult<bool> {
        let (total_size, chunk_offset) = {
            let loader = data_loader.lock().await;
            let total_size = loader.total_size();

            if let Some(total) = total_size {
                let quality_percentage = current_quality.data_percentage();
                let quality_size = (total as f32 * quality_percentage) as usize;
                let chunk_offset = chunks_loaded * config.chunk_size;

                if chunk_offset >= quality_size {
                    return Ok(false); // No more chunks for this quality level
                }

                (Some(total), chunk_offset)
            } else {
                return Ok(false); // Can't determine size
            }
        };

        // Check if chunk is already cached
        {
            let cache = chunk_cache.read().await;
            if cache.contains_key(&chunk_offset) {
                return Ok(true); // Already loaded
            }
        }

        // Load the chunk
        let chunk_data = {
            let mut loader = data_loader.lock().await;
            loader.load_chunk(chunk_offset, config.chunk_size).await?
        };

        if !chunk_data.is_empty() {
            // Cache the chunk
            {
                let mut cache = chunk_cache.write().await;
                cache.insert(chunk_offset, chunk_data.clone());
            }

            // Add to loaded data
            {
                let mut data = loaded_data.write().await;
                data.extend(chunk_data);
            }

            // Update progress
            {
                if let Ok(mut progress) = load_progress.try_write() {
                    progress.chunks_loaded = chunks_loaded + 1;
                    if let Some(total) = total_size {
                        let quality_percentage = current_quality.data_percentage();
                        let quality_chunks = ((total as f32 * quality_percentage)
                            / config.chunk_size as f32)
                            .ceil() as usize;
                        progress.total_chunks = Some(quality_chunks);
                    }
                    progress.loading_stage = format!(
                        "Loading {} quality",
                        match current_quality {
                            QualityLevel::Preview => "preview",
                            QualityLevel::Medium => "medium",
                            QualityLevel::High => "high",
                            QualityLevel::Full => "full",
                        }
                    );
                }
            }

            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Get the current number of loaded items.
    pub async fn loaded_count(&self) -> usize {
        self.loaded_data.read().await.len()
    }

    /// Get the current quality level.
    pub async fn current_quality(&self) -> QualityLevel {
        self.load_progress.read().await.current_quality
    }

    /// Get the target quality level.
    pub fn target_quality(&self) -> QualityLevel {
        self.config.target_quality
    }

    /// Set a new target quality level.
    pub async fn set_target_quality(&mut self, quality: QualityLevel) {
        self.config.target_quality = quality;

        // Update progress tracker
        if let Ok(mut progress) = self.load_progress.try_write() {
            progress.target_quality = quality;
        }

        // Restart background loading if needed
        if self.config.background_loading && !self.cancellation_token.is_cancelled() {
            self.start_background_loading();
        }
    }

    /// Force immediate loading up to a specific quality level.
    pub async fn load_to_quality(&self, quality: QualityLevel) -> GupResult<()> {
        let start_time = Instant::now();
        let timeout = Duration::from_secs(30); // 30-second timeout for loading

        let mut current_quality = self.current_quality().await;
        let mut chunks_loaded = 0;

        // Continue loading until we complete the target quality level
        while current_quality <= quality {
            if start_time.elapsed() > timeout {
                return Err(GupError::render_error("Loading timeout exceeded"));
            }

            let chunk_result = Self::load_next_chunk(
                &self.data_loader,
                &self.loaded_data,
                &self.load_progress,
                &self.config,
                &self.chunk_cache,
                current_quality,
                chunks_loaded,
            )
            .await?;

            if chunk_result {
                chunks_loaded += 1;
            } else {
                // No more chunks for this quality level
                if current_quality == quality {
                    // We've finished loading the target quality
                    break;
                }

                // Move to next quality level
                if let Some(next_quality) = current_quality.next() {
                    if let Ok(mut progress) = self.load_progress.try_write() {
                        progress.current_quality = next_quality;
                        progress.chunks_loaded = 0;
                    }
                    current_quality = next_quality;
                    chunks_loaded = 0;
                } else {
                    break; // Reached maximum quality
                }
            }

            // Yield control to prevent blocking
            tokio::task::yield_now().await;
        }

        Ok(())
    }

    /// Clear all cached data and restart loading.
    pub async fn reset(&mut self) -> GupResult<()> {
        // Cancel current operations
        self.cancellation_token.cancel();

        // Wait for background task to complete
        if let Ok(mut task_guard) = self.background_task.try_lock()
            && let Some(task) = task_guard.take()
        {
            let _ = task.await; // Ignore join errors
        }

        // Clear data and caches
        self.loaded_data.write().await.clear();
        self.chunk_cache.write().await.clear();

        // Reset progress
        {
            let mut progress = self.load_progress.write().await;
            *progress = LoadProgress::new(self.config.target_quality);
        }

        // Create new cancellation token and restart
        self.cancellation_token = CancellationToken::new();
        if self.config.background_loading {
            self.start_background_loading();
        }

        Ok(())
    }
}

impl<T, L> Drop for ProgressiveVisualization<T, L> {
    fn drop(&mut self) {
        self.cancellation_token.cancel();
    }
}

impl<T, L> std::fmt::Debug for ProgressiveVisualization<T, L> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProgressiveVisualization")
            .field("config", &self.config)
            .field("cancelled", &self.cancellation_token.is_cancelled())
            .finish()
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<T, L> AsyncMixable for ProgressiveVisualization<T, L>
where
    T: Clone + MaybeSend + MaybeSync + 'static,
    L: ProgressiveDataLoader<T> + 'static,
{
    type Output = ();

    async fn render_async(
        &self,
        _context: &mut RenderContext,
        _cancellation: CancellationToken,
    ) -> GupResult<()> {
        // For this base implementation, we just ensure some data is loaded
        // Specific visualization types would override this to do actual rendering

        if self.loaded_count().await == 0 {
            // Load at least preview quality if nothing is loaded
            let _ = self.load_to_quality(QualityLevel::Preview).await;
        }

        Ok(())
    }

    async fn is_ready(&self) -> bool {
        // Consider ready if we have at least some data loaded
        self.loaded_count().await > 0
    }

    fn progress(&self) -> Option<RenderProgress> {
        if let Ok(load_progress) = self.load_progress.try_read() {
            let progress = RenderProgress {
                current: load_progress.chunks_loaded as u64,
                total: load_progress.total_chunks.map(|t| t as u64),
                stage: load_progress.loading_stage.clone(),
                estimated_time_remaining: None, // Could implement based on loading rate
            };
            Some(progress)
        } else {
            None
        }
    }

    fn cancel(&self) {
        self.cancellation_token.cancel();
    }

    fn description(&self) -> String {
        format!(
            "ProgressiveVisualization(target: {:?}, chunk_size: {})",
            self.config.target_quality, self.config.chunk_size
        )
    }

    fn is_valid(&self) -> bool {
        !self.cancellation_token.is_cancelled()
    }
}

/// Mock data loader for testing purposes.
pub struct MockProgressiveDataLoader<T> {
    data: Vec<T>,
    chunk_access_count: HashMap<usize, usize>,
    simulate_slow_loading: bool,
}

impl<T> MockProgressiveDataLoader<T>
where
    T: Clone,
{
    /// Create a new mock data loader.
    pub fn new(data: Vec<T>) -> Self {
        Self {
            data,
            chunk_access_count: HashMap::new(),
            simulate_slow_loading: false,
        }
    }

    /// Enable slow loading simulation for testing.
    pub fn with_slow_loading(mut self) -> Self {
        self.simulate_slow_loading = true;
        self
    }

    /// Get access statistics for chunks.
    pub fn chunk_access_count(&self, offset: usize) -> usize {
        self.chunk_access_count.get(&offset).copied().unwrap_or(0)
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<T> ProgressiveDataLoader<T> for MockProgressiveDataLoader<T>
where
    T: Clone + MaybeSend + MaybeSync + 'static,
{
    async fn load_chunk(&mut self, offset: usize, size: usize) -> GupResult<Vec<T>> {
        // Track access for testing
        *self.chunk_access_count.entry(offset).or_insert(0) += 1;

        // Simulate slow loading if enabled
        if self.simulate_slow_loading {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        if offset >= self.data.len() {
            return Ok(Vec::new());
        }

        let end = (offset + size).min(self.data.len());
        Ok(self.data[offset..end].to_vec())
    }

    fn total_size(&self) -> Option<usize> {
        Some(self.data.len())
    }

    fn chunk_priority(&self, offset: usize) -> f32 {
        // Higher priority for earlier chunks
        let max_offset = self.data.len();
        if max_offset == 0 {
            return 1.0;
        }
        1.0 - (offset as f32 / max_offset as f32)
    }

    fn supports_random_access(&self) -> bool {
        true
    }

    fn metadata(&self) -> HashMap<String, String> {
        let mut meta = HashMap::new();
        meta.insert("type".to_string(), "mock".to_string());
        meta.insert("size".to_string(), self.data.len().to_string());
        meta
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quality_levels() {
        assert_eq!(QualityLevel::Preview.data_percentage(), 0.1);
        assert_eq!(QualityLevel::Medium.data_percentage(), 0.5);
        assert_eq!(QualityLevel::High.data_percentage(), 0.9);
        assert_eq!(QualityLevel::Full.data_percentage(), 1.0);

        assert_eq!(QualityLevel::Preview.next(), Some(QualityLevel::Medium));
        assert_eq!(QualityLevel::Full.next(), None);

        let all_levels = QualityLevel::all();
        assert_eq!(all_levels.len(), 4);
        assert_eq!(all_levels[0], QualityLevel::Preview);
    }

    #[test]
    fn test_load_progress() {
        let mut progress = LoadProgress::new(QualityLevel::High);
        assert_eq!(progress.target_quality, QualityLevel::High);
        assert_eq!(progress.current_quality, QualityLevel::Preview);
        assert!(!progress.is_complete());

        progress.current_quality = QualityLevel::High;
        assert!(progress.is_complete());
    }

    #[tokio::test]
    async fn test_mock_progressive_data_loader() {
        let test_data: Vec<i32> = (0..100).collect();
        let mut loader = MockProgressiveDataLoader::new(test_data);

        assert_eq!(loader.total_size(), Some(100));
        assert!(loader.supports_random_access());

        let chunk = loader.load_chunk(10, 5).await.unwrap();
        assert_eq!(chunk, vec![10, 11, 12, 13, 14]);
        assert_eq!(loader.chunk_access_count(10), 1);

        // Test loading beyond data size
        let empty_chunk = loader.load_chunk(200, 10).await.unwrap();
        assert!(empty_chunk.is_empty());
    }

    #[tokio::test]
    async fn test_progressive_visualization_creation() {
        let test_data: Vec<i32> = (0..1000).collect();
        let loader = MockProgressiveDataLoader::new(test_data);
        let config = ProgressiveConfig::default();

        let viz = ProgressiveVisualization::new(loader, config);
        assert!(viz.is_valid());
        assert_eq!(viz.target_quality(), QualityLevel::High);
    }

    #[tokio::test]
    async fn test_progressive_loading() {
        let test_data: Vec<i32> = (0..500).collect();
        let loader = MockProgressiveDataLoader::new(test_data);
        let config = ProgressiveConfig {
            chunk_size: 25,            // Small chunks for testing
            background_loading: false, // Disable for controlled testing
            ..Default::default()
        };

        let viz = ProgressiveVisualization::new(loader, config);

        // Should start with no data
        assert_eq!(viz.loaded_count().await, 0);
        assert_eq!(viz.current_quality().await, QualityLevel::Preview);

        // Load to preview quality
        viz.load_to_quality(QualityLevel::Preview).await.unwrap();

        let loaded_count = viz.loaded_count().await;
        assert!(loaded_count > 0);
        assert!(
            loaded_count <= 50,
            "Loaded {loaded_count} items, expected <= 50 for Preview quality"
        ); // Preview is ~10% of 500 items
    }

    #[tokio::test]
    async fn test_progressive_quality_upgrade() {
        let test_data: Vec<i32> = (0..200).collect();
        let loader = MockProgressiveDataLoader::new(test_data);
        let config = ProgressiveConfig {
            chunk_size: 10, // Small chunks for testing
            background_loading: false,
            ..Default::default()
        };

        let mut viz = ProgressiveVisualization::new(loader, config);

        // Load to medium quality first
        viz.load_to_quality(QualityLevel::Medium).await.unwrap();
        let medium_count = viz.loaded_count().await;

        // Upgrade to high quality
        viz.set_target_quality(QualityLevel::High).await;
        viz.load_to_quality(QualityLevel::High).await.unwrap();
        let high_count = viz.loaded_count().await;

        assert!(high_count > medium_count);
        assert_eq!(viz.current_quality().await, QualityLevel::High);
    }

    #[tokio::test]
    async fn test_progressive_reset() {
        let test_data: Vec<i32> = (0..100).collect();
        let loader = MockProgressiveDataLoader::new(test_data);
        let config = ProgressiveConfig {
            chunk_size: 5, // Small chunks for testing
            background_loading: false,
            ..Default::default()
        };

        let mut viz = ProgressiveVisualization::new(loader, config);

        // Load some data
        viz.load_to_quality(QualityLevel::Preview).await.unwrap();
        assert!(viz.loaded_count().await > 0);

        // Reset
        viz.reset().await.unwrap();
        assert_eq!(viz.loaded_count().await, 0);
        assert_eq!(viz.current_quality().await, QualityLevel::Preview);
    }

    #[tokio::test]
    async fn test_progressive_config() {
        let config = ProgressiveConfig::default();
        assert_eq!(config.chunk_size, 1000);
        assert_eq!(config.target_quality, QualityLevel::High);
        assert!(config.background_loading);
        assert!(config.priority_loading);

        let custom_config = ProgressiveConfig {
            chunk_size: 500,
            target_quality: QualityLevel::Full,
            max_loading_time_per_frame: Duration::from_millis(16),
            background_loading: false,
            priority_loading: false,
        };
        assert_eq!(custom_config.chunk_size, 500);
        assert_eq!(custom_config.target_quality, QualityLevel::Full);
    }

    #[tokio::test]
    async fn test_slow_loading_simulation() {
        let test_data: Vec<i32> = (0..10).collect();
        let mut loader = MockProgressiveDataLoader::new(test_data).with_slow_loading();

        let start = Instant::now();
        let _chunk = loader.load_chunk(0, 5).await.unwrap();
        let elapsed = start.elapsed();

        // Should take at least 10ms due to simulation
        assert!(elapsed >= Duration::from_millis(5)); // Allow some tolerance
    }
}

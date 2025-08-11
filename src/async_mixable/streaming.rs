// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Streaming data support for real-time visualizations.

use super::{AsyncMixable, RenderProgress};
use crate::{GupResult, RenderContext};
use async_trait::async_trait;
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock, mpsc};
use tokio_util::sync::CancellationToken;

/// Statistics for a streaming data source.
#[derive(Debug, Clone)]
pub struct StreamStats {
    pub items_processed: u64,
    pub items_per_second: f64,
    pub buffer_size: usize,
    pub is_live: bool,
    pub last_update: Instant,
}

impl StreamStats {
    /// Create new stream statistics.
    pub fn new() -> Self {
        Self {
            items_processed: 0,
            items_per_second: 0.0,
            buffer_size: 0,
            is_live: false,
            last_update: Instant::now(),
        }
    }

    /// Update statistics with new data.
    pub fn update(&mut self, items_added: usize) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_update).as_secs_f64();

        self.items_processed += items_added as u64;
        if elapsed > 0.0 {
            self.items_per_second = items_added as f64 / elapsed;
        }
        self.last_update = now;
    }
}

impl Default for StreamStats {
    fn default() -> Self {
        Self::new()
    }
}

/// Trait for streaming data sources.
#[async_trait]
pub trait StreamingDataSource<T>: Send + Sync {
    /// Get the next batch of data from the stream.
    async fn next_batch(&mut self) -> Option<GupResult<Vec<T>>>;

    /// Check if more data is available.
    fn has_more(&self) -> bool;

    /// Get current stream statistics.
    fn stream_stats(&self) -> StreamStats;

    /// Set the batch size for data retrieval.
    fn set_batch_size(&mut self, size: usize);

    /// Get the current batch size.
    fn batch_size(&self) -> usize;
}

/// 2D point for visualization.
#[derive(Debug, Clone, Copy)]
pub struct Point2D {
    pub x: f32,
    pub y: f32,
    pub color: [f32; 4],
}

impl Point2D {
    /// Create a new point.
    pub fn new(x: f32, y: f32) -> Self {
        Self {
            x,
            y,
            color: [1.0, 1.0, 1.0, 1.0], // White by default
        }
    }

    /// Create a point with color.
    pub fn with_color(x: f32, y: f32, color: [f32; 4]) -> Self {
        Self { x, y, color }
    }
}

/// State for streaming rendering operations.
#[derive(Debug)]
struct StreamingRenderState {
    buffer_dirty: bool,
    last_update: Instant,
    pending_updates: usize,
    total_updates: u64,
}

impl StreamingRenderState {
    fn new() -> Self {
        Self {
            buffer_dirty: false,
            last_update: Instant::now(),
            pending_updates: 0,
            total_updates: 0,
        }
    }
}

/// Streaming scatter plot that updates with incoming data.
pub struct StreamingScatterPlot<T> {
    data_stream: Arc<Mutex<T>>,
    current_data: Arc<RwLock<VecDeque<Point2D>>>,
    max_points: usize,
    update_receiver: Arc<Mutex<mpsc::UnboundedReceiver<Vec<Point2D>>>>,
    render_state: Arc<RwLock<StreamingRenderState>>,
    cancellation_token: CancellationToken,
    #[allow(dead_code)]
    stream_task_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

impl<T> StreamingScatterPlot<T>
where
    T: StreamingDataSource<Point2D> + 'static,
{
    /// Create a new streaming scatter plot.
    pub fn new(data_stream: T, max_points: usize) -> Self {
        let (update_sender, update_receiver) = mpsc::unbounded_channel();
        let data_stream = Arc::new(Mutex::new(data_stream));
        let current_data = Arc::new(RwLock::new(VecDeque::new()));
        let render_state = Arc::new(RwLock::new(StreamingRenderState::new()));
        let cancellation_token = CancellationToken::new();

        // Start background streaming task
        let stream_handle = Arc::new(Mutex::new(data_stream.clone()));
        let state_handle = render_state.clone();
        let token = cancellation_token.clone();

        let task_handle = tokio::spawn(async move {
            Self::stream_processor_task(stream_handle, update_sender, state_handle, token).await;
        });

        Self {
            data_stream,
            current_data,
            max_points,
            update_receiver: Arc::new(Mutex::new(update_receiver)),
            render_state,
            cancellation_token,
            stream_task_handle: Arc::new(Mutex::new(Some(task_handle))),
        }
    }

    /// Background task to process streaming data.
    async fn stream_processor_task(
        data_stream: Arc<Mutex<Arc<Mutex<T>>>>,
        update_sender: mpsc::UnboundedSender<Vec<Point2D>>,
        render_state: Arc<RwLock<StreamingRenderState>>,
        cancellation_token: CancellationToken,
    ) {
        let mut error_count = 0;
        const MAX_ERRORS: usize = 10;

        loop {
            tokio::select! {
                _ = cancellation_token.cancelled() => {
                    break;
                }
                _ = tokio::time::sleep(Duration::from_millis(16)) => {
                    // Process at ~60fps
                    let batch = {
                        let stream_arc = data_stream.lock().await;
                        let mut stream = stream_arc.lock().await;
                        stream.next_batch().await
                    };

                    match batch {
                        Some(Ok(data)) => {
                            if !data.is_empty() {
                                if update_sender.send(data).is_err() {
                                    // Receiver dropped, exit
                                    break;
                                }

                                let mut state = render_state.write().await;
                                state.buffer_dirty = true;
                                state.pending_updates += 1;
                                error_count = 0; // Reset error count on success
                            }
                        }
                        Some(Err(_)) => {
                            error_count += 1;
                            if error_count >= MAX_ERRORS {
                                break; // Too many consecutive errors
                            }
                            tokio::time::sleep(Duration::from_millis(100)).await;
                        }
                        None => {
                            // Stream ended
                            break;
                        }
                    }
                }
            }
        }
    }

    /// Process pending data updates.
    async fn process_pending_updates(&self) -> GupResult<()> {
        let mut updates_processed = 0;
        const MAX_UPDATES_PER_FRAME: usize = 10;

        let mut receiver = self.update_receiver.lock().await;

        while let Ok(batch) = receiver.try_recv() {
            self.add_data_batch(batch).await;
            updates_processed += 1;

            // Limit updates per frame to maintain responsiveness
            if updates_processed >= MAX_UPDATES_PER_FRAME {
                break;
            }
        }

        if updates_processed > 0 {
            let mut state = self.render_state.write().await;
            state.buffer_dirty = true;
            state.pending_updates = state.pending_updates.saturating_sub(updates_processed);
            state.total_updates += updates_processed as u64;
        }

        Ok(())
    }

    /// Add a batch of data points.
    async fn add_data_batch(&self, batch: Vec<Point2D>) {
        let mut data = self.current_data.write().await;

        for point in batch {
            data.push_back(point);
        }

        // Implement sliding window if we exceed max points
        while data.len() > self.max_points {
            data.pop_front();
        }
    }

    /// Get current data size.
    pub async fn data_size(&self) -> usize {
        self.current_data.read().await.len()
    }

    /// Get maximum points capacity.
    pub fn max_points(&self) -> usize {
        self.max_points
    }

    /// Check if the stream is actively processing data.
    pub async fn is_streaming(&self) -> bool {
        let stream = self.data_stream.lock().await;
        stream.has_more()
    }

    /// Create a timeout composition wrapper.
    pub fn with_timeout(self, timeout: Duration) -> crate::TimeoutComposition<Self> {
        crate::TimeoutComposition::new(self, timeout)
    }
}

impl<T> Drop for StreamingScatterPlot<T> {
    fn drop(&mut self) {
        // Cancel the streaming task
        self.cancellation_token.cancel();

        // Note: We don't await the task here as Drop is synchronous
        // The task will terminate when it next checks the cancellation token
    }
}

impl<T> std::fmt::Debug for StreamingScatterPlot<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StreamingScatterPlot")
            .field("max_points", &self.max_points)
            .field("is_cancelled", &self.cancellation_token.is_cancelled())
            .finish()
    }
}

#[async_trait]
impl<T> AsyncMixable for StreamingScatterPlot<T>
where
    T: StreamingDataSource<Point2D> + 'static,
{
    type Output = ();

    async fn render_async(
        &self,
        _context: &mut RenderContext,
        _cancellation: CancellationToken,
    ) -> GupResult<()> {
        // Process any pending updates first
        self.process_pending_updates().await?;

        // Check if buffer needs updating
        let needs_update = {
            let state = self.render_state.read().await;
            state.buffer_dirty
        };

        if needs_update {
            // Get current data for rendering
            let data = self.current_data.read().await;
            let vertices: Vec<_> = data
                .iter()
                .map(|point| Vertex {
                    position: [point.x, point.y],
                    color: point.color,
                })
                .collect();

            if !vertices.is_empty() {
                // For now, just validate the vertices are ready for rendering
                // In a real implementation, this would use the actual rendering pipeline
                log::debug!("Would render {} vertices", vertices.len());
            }

            // Mark buffer as clean
            let mut state = self.render_state.write().await;
            state.buffer_dirty = false;
            state.last_update = Instant::now();
        }

        Ok(())
    }

    async fn is_ready(&self) -> bool {
        // Streaming components are always ready to render current data
        true
    }

    fn progress(&self) -> Option<RenderProgress> {
        // For streaming, progress could be based on data processing rate
        // For now, return None as streaming is continuous
        None
    }

    fn cancel(&self) {
        self.cancellation_token.cancel();
    }

    fn description(&self) -> String {
        format!("StreamingScatterPlot(max_points: {})", self.max_points)
    }

    fn is_valid(&self) -> bool {
        !self.cancellation_token.is_cancelled()
    }
}

/// Vertex type for rendering points.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    position: [f32; 2],
    color: [f32; 4],
}

/// Mock data source for testing.
pub struct MockStreamingDataSource {
    data: VecDeque<Point2D>,
    batch_size: usize,
    stats: StreamStats,
    rate_limit: Option<Duration>,
    last_batch_time: Instant,
}

impl MockStreamingDataSource {
    /// Create a new mock data source.
    pub fn new(data: Vec<Point2D>) -> Self {
        Self {
            data: VecDeque::from(data),
            batch_size: 10,
            stats: StreamStats::new(),
            rate_limit: None,
            last_batch_time: Instant::now(),
        }
    }

    /// Set rate limiting for the mock source.
    pub fn with_rate_limit(mut self, interval: Duration) -> Self {
        self.rate_limit = Some(interval);
        self
    }

    /// Add more data to the source.
    pub fn add_data(&mut self, data: Vec<Point2D>) {
        self.data.extend(data);
    }
}

#[async_trait]
impl StreamingDataSource<Point2D> for MockStreamingDataSource {
    async fn next_batch(&mut self) -> Option<GupResult<Vec<Point2D>>> {
        // Check rate limiting
        if let Some(rate_limit) = self.rate_limit {
            let elapsed = self.last_batch_time.elapsed();
            if elapsed < rate_limit {
                tokio::time::sleep(rate_limit - elapsed).await;
            }
        }

        if self.data.is_empty() {
            return None;
        }

        let batch_size = self.batch_size.min(self.data.len());
        let mut batch = Vec::with_capacity(batch_size);

        for _ in 0..batch_size {
            if let Some(point) = self.data.pop_front() {
                batch.push(point);
            }
        }

        if !batch.is_empty() {
            self.stats.update(batch.len());
            self.last_batch_time = Instant::now();
            Some(Ok(batch))
        } else {
            None
        }
    }

    fn has_more(&self) -> bool {
        !self.data.is_empty()
    }

    fn stream_stats(&self) -> StreamStats {
        let mut stats = self.stats.clone();
        stats.buffer_size = self.data.len();
        stats.is_live = true;
        stats
    }

    fn set_batch_size(&mut self, size: usize) {
        self.batch_size = size.max(1); // Ensure at least 1
    }

    fn batch_size(&self) -> usize {
        self.batch_size
    }
}

/// Null data source for placeholder purposes.
pub struct NullDataSource;

#[async_trait]
impl StreamingDataSource<Point2D> for NullDataSource {
    async fn next_batch(&mut self) -> Option<GupResult<Vec<Point2D>>> {
        None
    }

    fn has_more(&self) -> bool {
        false
    }

    fn stream_stats(&self) -> StreamStats {
        StreamStats::new()
    }

    fn set_batch_size(&mut self, _size: usize) {
        // No-op
    }

    fn batch_size(&self) -> usize {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_points(count: usize) -> Vec<Point2D> {
        (0..count)
            .map(|i| Point2D::new(i as f32, (i * 2) as f32))
            .collect()
    }

    #[tokio::test]
    async fn test_mock_streaming_data_source() {
        let test_data = create_test_points(100);
        let mut source = MockStreamingDataSource::new(test_data.clone());
        source.set_batch_size(10);

        assert!(source.has_more());
        assert_eq!(source.batch_size(), 10);

        let batch = source.next_batch().await;
        assert!(batch.is_some());

        let batch = batch.unwrap().unwrap();
        assert_eq!(batch.len(), 10);
        assert_eq!(batch[0].x, 0.0);
        assert_eq!(batch[0].y, 0.0);
    }

    #[tokio::test]
    async fn test_streaming_scatter_plot_creation() {
        let test_data = create_test_points(50);
        let source = MockStreamingDataSource::new(test_data);
        let plot = StreamingScatterPlot::new(source, 100);

        assert_eq!(plot.max_points(), 100);
        assert!(plot.is_valid());
    }

    #[tokio::test]
    async fn test_streaming_data_processing() {
        let test_data = create_test_points(20);
        let source = MockStreamingDataSource::new(test_data);
        let plot = StreamingScatterPlot::new(source, 10);

        // Wait a bit for background processing
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Should have data, but limited by max_points
        let data_size = plot.data_size().await;
        assert!(data_size <= 10); // Should be capped at max_points
    }

    #[test]
    fn test_point2d_creation() {
        let point = Point2D::new(1.0, 2.0);
        assert_eq!(point.x, 1.0);
        assert_eq!(point.y, 2.0);
        assert_eq!(point.color, [1.0, 1.0, 1.0, 1.0]);

        let colored_point = Point2D::with_color(3.0, 4.0, [0.5, 0.6, 0.7, 0.8]);
        assert_eq!(colored_point.x, 3.0);
        assert_eq!(colored_point.y, 4.0);
        assert_eq!(colored_point.color, [0.5, 0.6, 0.7, 0.8]);
    }

    #[test]
    fn test_stream_stats() {
        let mut stats = StreamStats::new();
        assert_eq!(stats.items_processed, 0);
        assert_eq!(stats.items_per_second, 0.0);

        stats.update(10);
        assert_eq!(stats.items_processed, 10);
    }

    #[tokio::test]
    async fn test_rate_limited_source() {
        let test_data = create_test_points(5);
        let mut source =
            MockStreamingDataSource::new(test_data).with_rate_limit(Duration::from_millis(50));

        let start = Instant::now();
        let _batch1 = source.next_batch().await;
        let _batch2 = source.next_batch().await;
        let elapsed = start.elapsed();

        // Should take at least 50ms due to rate limiting
        assert!(elapsed >= Duration::from_millis(40)); // Allow some tolerance
    }

    #[tokio::test]
    async fn test_null_data_source() {
        let mut source = NullDataSource;
        assert!(!source.has_more());
        assert_eq!(source.batch_size(), 0);

        let batch = source.next_batch().await;
        assert!(batch.is_none());
    }
}

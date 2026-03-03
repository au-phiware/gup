// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! High-level [`DataStream<T>`] that wraps a
//! [`StreamingBuffer<T>`](super::StreamingBuffer) with stream-mode semantics,
//! backpressure handling, and an observable subscriber pattern.

use std::sync::atomic::{AtomicU64, Ordering};

use crate::error::GupResult;

use super::backpressure::BackpressureStrategy;
use super::builder::{DataStreamBuilder, DataStreamError};
use super::mode::StreamMode;
use super::streaming_buffer::{StreamUpdate, StreamingBuffer, StreamingBufferConfig};

// ---------------------------------------------------------------------------
// Subscriber infrastructure
// ---------------------------------------------------------------------------

static NEXT_SUBSCRIBER_ID: AtomicU64 = AtomicU64::new(1);
static NEXT_KEY: AtomicU64 = AtomicU64::new(1);

/// Opaque handle returned by [`DataStream::subscribe`].
///
/// Pass this to [`DataStream::unsubscribe`] to deregister the callback.
///
/// # Examples
///
/// ```no_run
/// use gup::streaming::{DataStream, SubscriberHandle};
///
/// # async fn example(device: &wgpu::Device) {
/// let mut stream = DataStream::<f32>::builder()
///     .capacity(100)
///     .build(device)
///     .unwrap();
///
/// let handle: SubscriberHandle = stream.subscribe(|update| {
///     println!("Got update: {update:?}");
/// });
///
/// // Later, unsubscribe:
/// stream.unsubscribe(handle);
/// # }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SubscriberHandle(u64);

/// Internal subscriber entry pairing a handle with its callback.
struct Subscriber<T: bytemuck::Pod + bytemuck::Zeroable> {
    handle: SubscriberHandle,
    callback: Box<dyn Fn(&StreamUpdate<T>) + Send + Sync + 'static>,
}

// ---------------------------------------------------------------------------
// DataStream
// ---------------------------------------------------------------------------

/// A high-level data stream for feeding live data to GPU visualizations.
///
/// `DataStream<T>` wraps a [`StreamingBuffer<T>`](super::StreamingBuffer) and
/// adds:
///
/// * **Stream mode**: [`AppendOnly`](StreamMode::AppendOnly),
///   [`SlidingWindow`](StreamMode::SlidingWindow), or
///   [`RingBuffer`](StreamMode::RingBuffer) semantics.
/// * **Backpressure**: configurable via
///   [`BackpressureStrategy`](super::BackpressureStrategy).
/// * **Observable subscribers**: register callbacks that fire on every
///   committed update.
///
/// Construct one through the fluent [`DataStreamBuilder`] returned by
/// [`DataStream::builder()`].
///
/// # Examples
///
/// ```no_run
/// use gup::streaming::{DataStream, StreamMode, BackpressureStrategy};
///
/// # async fn example(device: &wgpu::Device, queue: &wgpu::Queue) {
/// let mut stream = DataStream::<[f32; 2]>::builder()
///     .capacity(1_000)
///     .mode(StreamMode::SlidingWindow)
///     .backpressure(BackpressureStrategy::EvictOldest)
///     .build(device)
///     .unwrap();
///
/// // Push individual data points
/// stream.push([1.0, 2.0]);
///
/// // Push a batch
/// stream.push_batch(vec![[3.0, 4.0], [5.0, 6.0]]);
///
/// // Flush dirty regions to the GPU
/// stream.flush(device, queue);
/// # }
/// ```
pub struct DataStream<T: bytemuck::Pod + bytemuck::Zeroable> {
    buffer: StreamingBuffer<T>,
    mode: StreamMode,
    backpressure: BackpressureStrategy,
    subscribers: Vec<Subscriber<T>>,
}

impl<T: bytemuck::Pod + bytemuck::Zeroable> DataStream<T> {
    /// Return a new [`DataStreamBuilder`] for configuring a stream.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use gup::streaming::DataStream;
    ///
    /// # async fn example(device: &wgpu::Device) {
    /// let stream = DataStream::<f32>::builder()
    ///     .capacity(500)
    ///     .build(device)
    ///     .unwrap();
    /// # }
    /// ```
    pub fn builder() -> DataStreamBuilder<T> {
        DataStreamBuilder::new()
    }

    /// Internal constructor called by [`DataStreamBuilder::build`].
    pub(crate) fn from_builder(
        device: &wgpu::Device,
        capacity: usize,
        mode: StreamMode,
        backpressure: BackpressureStrategy,
    ) -> Self {
        let config = StreamingBufferConfig {
            capacity,
            ..Default::default()
        };
        Self {
            buffer: StreamingBuffer::new(device, config),
            mode,
            backpressure,
            subscribers: Vec::new(),
        }
    }

    // -----------------------------------------------------------------------
    // Push API
    // -----------------------------------------------------------------------

    /// Push a single data point into the stream.
    ///
    /// The behaviour when the stream is full depends on the configured
    /// [`StreamMode`] and [`BackpressureStrategy`]:
    ///
    /// * **SlidingWindow / RingBuffer**: the oldest item is evicted.
    /// * **AppendOnly + EvictOldest**: same as above.
    /// * **AppendOnly + DropNewest**: the push is silently dropped.
    /// * **AppendOnly + Block**: the push is silently dropped (blocking is
    ///   not yet implemented for synchronous push; use an async channel
    ///   upstream).
    ///
    /// Returns `true` if the data was actually inserted, `false` if it was
    /// dropped due to backpressure.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use gup::streaming::DataStream;
    ///
    /// # async fn example(device: &wgpu::Device) {
    /// let mut stream = DataStream::<f32>::builder()
    ///     .capacity(100)
    ///     .build(device)
    ///     .unwrap();
    ///
    /// assert!(stream.push(42.0));
    /// assert_eq!(stream.len(), 1);
    /// # }
    /// ```
    pub fn push(&mut self, data: T) -> bool {
        if self.is_at_capacity() && self.should_drop() {
            return false;
        }

        let key = NEXT_KEY.fetch_add(1, Ordering::Relaxed);
        let update = StreamUpdate::Insert { key, data };

        self.buffer.insert(key, data);
        self.notify_subscribers(&update);
        true
    }

    /// Push a batch of data points into the stream.
    ///
    /// Each item in `batch` is pushed individually, respecting backpressure.
    /// Returns the number of items actually inserted.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use gup::streaming::DataStream;
    ///
    /// # async fn example(device: &wgpu::Device) {
    /// let mut stream = DataStream::<f32>::builder()
    ///     .capacity(100)
    ///     .build(device)
    ///     .unwrap();
    ///
    /// let inserted = stream.push_batch(vec![1.0, 2.0, 3.0]);
    /// assert_eq!(inserted, 3);
    /// # }
    /// ```
    pub fn push_batch(&mut self, batch: Vec<T>) -> usize {
        let mut inserted = 0;
        for item in batch {
            if self.push(item) {
                inserted += 1;
            }
        }
        inserted
    }

    // -----------------------------------------------------------------------
    // GPU flush
    // -----------------------------------------------------------------------

    /// Flush dirty regions to the GPU, returning the number of bytes written.
    ///
    /// Only byte ranges modified since the last flush are transferred,
    /// keeping GPU bus traffic to a minimum.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use gup::streaming::DataStream;
    ///
    /// # async fn example(device: &wgpu::Device, queue: &wgpu::Queue) {
    /// let mut stream = DataStream::<f32>::builder()
    ///     .capacity(100)
    ///     .build(device)
    ///     .unwrap();
    ///
    /// stream.push(1.0);
    /// let bytes_written = stream.flush(device, queue);
    /// assert!(bytes_written > 0);
    /// # }
    /// ```
    pub fn flush(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) -> usize {
        self.buffer.flush(device, queue)
    }

    // -----------------------------------------------------------------------
    // Subscriber API
    // -----------------------------------------------------------------------

    /// Register a callback that is invoked for every committed update.
    ///
    /// Multiple subscribers can be registered; they are called in
    /// registration order. The callback receives a reference to the
    /// [`StreamUpdate<T>`] *after* it has been applied to the CPU-side
    /// buffer (but before the next GPU flush).
    ///
    /// Returns a [`SubscriberHandle`] that can be passed to
    /// [`unsubscribe`](Self::unsubscribe) to deregister the callback.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use gup::streaming::DataStream;
    /// use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
    ///
    /// # async fn example(device: &wgpu::Device) {
    /// let mut stream = DataStream::<f32>::builder()
    ///     .capacity(100)
    ///     .build(device)
    ///     .unwrap();
    ///
    /// let count = Arc::new(AtomicUsize::new(0));
    /// let count_clone = count.clone();
    /// stream.subscribe(move |_update| {
    ///     count_clone.fetch_add(1, Ordering::Relaxed);
    /// });
    ///
    /// stream.push(1.0);
    /// assert_eq!(count.load(Ordering::Relaxed), 1);
    /// # }
    /// ```
    pub fn subscribe(
        &mut self,
        callback: impl Fn(&StreamUpdate<T>) + Send + Sync + 'static,
    ) -> SubscriberHandle {
        let handle = SubscriberHandle(NEXT_SUBSCRIBER_ID.fetch_add(1, Ordering::Relaxed));
        self.subscribers.push(Subscriber {
            handle,
            callback: Box::new(callback),
        });
        handle
    }

    /// Deregister a subscriber by its handle.
    ///
    /// Returns `true` if the subscriber was found and removed, `false` if
    /// the handle was not recognised (e.g. already unsubscribed).
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use gup::streaming::DataStream;
    ///
    /// # async fn example(device: &wgpu::Device) {
    /// let mut stream = DataStream::<f32>::builder()
    ///     .capacity(100)
    ///     .build(device)
    ///     .unwrap();
    ///
    /// let handle = stream.subscribe(|_| {});
    /// assert!(stream.unsubscribe(handle));
    /// assert!(!stream.unsubscribe(handle)); // already removed
    /// # }
    /// ```
    pub fn unsubscribe(&mut self, handle: SubscriberHandle) -> bool {
        let before = self.subscribers.len();
        self.subscribers.retain(|s| s.handle != handle);
        self.subscribers.len() < before
    }

    // -----------------------------------------------------------------------
    // Accessors
    // -----------------------------------------------------------------------

    /// Number of live data items in the stream.
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    /// Whether the stream is empty.
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    /// Maximum capacity of the stream (in items).
    pub fn capacity(&self) -> usize {
        self.buffer.capacity()
    }

    /// The configured [`StreamMode`].
    pub fn mode(&self) -> StreamMode {
        self.mode
    }

    /// The configured [`BackpressureStrategy`].
    pub fn backpressure(&self) -> BackpressureStrategy {
        self.backpressure
    }

    /// Whether there are unflushed changes.
    pub fn is_dirty(&self) -> bool {
        self.buffer.is_dirty()
    }

    /// Reference to the underlying GPU buffer currently used for rendering.
    pub fn active_buffer(&self) -> &wgpu::Buffer {
        self.buffer.active_buffer()
    }

    /// Reference to the underlying [`StreamingBuffer<T>`].
    pub fn inner(&self) -> &StreamingBuffer<T> {
        &self.buffer
    }

    /// Mutable reference to the underlying [`StreamingBuffer<T>`].
    pub fn inner_mut(&mut self) -> &mut StreamingBuffer<T> {
        &mut self.buffer
    }

    /// Apply a [`StreamUpdate<T>`] directly (for advanced use-cases that
    /// need keyed updates/removes).
    ///
    /// Returns `Ok(())` on success. Subscriber callbacks are fired for the
    /// top-level update.
    pub fn apply_update(&mut self, update: StreamUpdate<T>) -> GupResult<()> {
        match &update {
            StreamUpdate::Insert { key, data } => {
                self.buffer.insert(*key, *data);
            }
            StreamUpdate::Update { key, data } => {
                self.buffer.update(*key, *data)?;
            }
            StreamUpdate::Remove { key } => {
                self.buffer.remove(*key)?;
            }
            StreamUpdate::Batch { updates } => {
                self.buffer.apply_batch(updates.clone())?;
            }
        }
        self.notify_subscribers(&update);
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    /// Check whether the stream is at capacity.
    fn is_at_capacity(&self) -> bool {
        self.buffer.len() >= self.buffer.capacity()
    }

    /// Determine whether to drop the incoming data point based on mode and
    /// backpressure strategy.
    fn should_drop(&self) -> bool {
        match self.mode {
            // SlidingWindow and RingBuffer always evict — never drop.
            StreamMode::SlidingWindow | StreamMode::RingBuffer => false,
            StreamMode::AppendOnly => match self.backpressure {
                BackpressureStrategy::EvictOldest => false,
                BackpressureStrategy::DropNewest | BackpressureStrategy::Block => true,
            },
        }
    }

    /// Notify all subscribers of an update.
    fn notify_subscribers(&self, update: &StreamUpdate<T>) {
        for subscriber in &self.subscribers {
            (subscriber.callback)(update);
        }
    }
}

impl<T: bytemuck::Pod + bytemuck::Zeroable> std::fmt::Debug for DataStream<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataStream")
            .field("len", &self.buffer.len())
            .field("capacity", &self.buffer.capacity())
            .field("mode", &self.mode)
            .field("backpressure", &self.backpressure)
            .field("subscribers", &self.subscribers.len())
            .field("dirty", &self.buffer.is_dirty())
            .finish()
    }
}

impl From<DataStreamError> for crate::error::GupError {
    fn from(err: DataStreamError) -> Self {
        match err {
            DataStreamError::InvalidCapacity { message } => {
                crate::error::GupError::ConfigurationError {
                    parameter: "capacity".into(),
                    message,
                }
            }
            DataStreamError::UnsupportedCombination { message } => {
                crate::error::GupError::ConfigurationError {
                    parameter: "mode/backpressure".into(),
                    message,
                }
            }
            DataStreamError::MissingConfiguration { message } => {
                crate::error::GupError::ConfigurationError {
                    parameter: "stream".into(),
                    message,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::RenderContext;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};

    async fn test_device() -> wgpu::Device {
        let ctx = RenderContext::new().await.unwrap();
        ctx.device().clone()
    }

    // -- Builder tests -------------------------------------------------------

    #[tokio::test]
    async fn builder_valid_defaults() {
        let device = test_device().await;
        let stream = DataStream::<f32>::builder()
            .capacity(100)
            .build(&device)
            .unwrap();
        assert_eq!(stream.capacity(), 100);
        assert_eq!(stream.mode(), StreamMode::RingBuffer);
        assert_eq!(stream.backpressure(), BackpressureStrategy::EvictOldest);
    }

    #[tokio::test]
    async fn builder_all_modes() {
        let device = test_device().await;
        for mode in [
            StreamMode::AppendOnly,
            StreamMode::SlidingWindow,
            StreamMode::RingBuffer,
        ] {
            let stream = DataStream::<f32>::builder()
                .capacity(10)
                .mode(mode)
                .build(&device)
                .unwrap();
            assert_eq!(stream.mode(), mode);
        }
    }

    #[tokio::test]
    async fn builder_all_backpressure_strategies() {
        let device = test_device().await;
        for bp in [
            BackpressureStrategy::Block,
            BackpressureStrategy::DropNewest,
            BackpressureStrategy::EvictOldest,
        ] {
            let stream = DataStream::<f32>::builder()
                .capacity(10)
                .mode(StreamMode::AppendOnly)
                .backpressure(bp)
                .build(&device)
                .unwrap();
            assert_eq!(stream.backpressure(), bp);
        }
    }

    #[tokio::test]
    async fn builder_zero_capacity_errors() {
        let device = test_device().await;
        let result = DataStream::<f32>::builder().capacity(0).build(&device);
        assert!(matches!(
            result,
            Err(DataStreamError::InvalidCapacity { .. })
        ));
    }

    #[tokio::test]
    async fn builder_missing_capacity_errors() {
        let device = test_device().await;
        let result = DataStream::<f32>::builder().build(&device);
        assert!(matches!(
            result,
            Err(DataStreamError::InvalidCapacity { .. })
        ));
    }

    #[tokio::test]
    async fn builder_block_with_ring_buffer_errors() {
        let device = test_device().await;
        let result = DataStream::<f32>::builder()
            .capacity(10)
            .mode(StreamMode::RingBuffer)
            .backpressure(BackpressureStrategy::Block)
            .build(&device);
        assert!(matches!(
            result,
            Err(DataStreamError::UnsupportedCombination { .. })
        ));
    }

    #[tokio::test]
    async fn builder_block_with_sliding_window_errors() {
        let device = test_device().await;
        let result = DataStream::<f32>::builder()
            .capacity(10)
            .mode(StreamMode::SlidingWindow)
            .backpressure(BackpressureStrategy::Block)
            .build(&device);
        assert!(matches!(
            result,
            Err(DataStreamError::UnsupportedCombination { .. })
        ));
    }

    #[tokio::test]
    async fn builder_block_with_append_only_ok() {
        let device = test_device().await;
        let result = DataStream::<f32>::builder()
            .capacity(10)
            .mode(StreamMode::AppendOnly)
            .backpressure(BackpressureStrategy::Block)
            .build(&device);
        assert!(result.is_ok());
    }

    // -- Push tests ----------------------------------------------------------

    #[tokio::test]
    async fn push_single_item() {
        let device = test_device().await;
        let mut stream = DataStream::<f32>::builder()
            .capacity(100)
            .build(&device)
            .unwrap();

        assert!(stream.push(1.0));
        assert_eq!(stream.len(), 1);
        assert!(!stream.is_empty());
    }

    #[tokio::test]
    async fn push_batch_items() {
        let device = test_device().await;
        let mut stream = DataStream::<f32>::builder()
            .capacity(100)
            .build(&device)
            .unwrap();

        let inserted = stream.push_batch(vec![1.0, 2.0, 3.0]);
        assert_eq!(inserted, 3);
        assert_eq!(stream.len(), 3);
    }

    #[tokio::test]
    async fn sliding_window_evicts_oldest() {
        let device = test_device().await;
        let mut stream = DataStream::<f32>::builder()
            .capacity(3)
            .mode(StreamMode::SlidingWindow)
            .build(&device)
            .unwrap();

        stream.push_batch(vec![1.0, 2.0, 3.0]);
        assert_eq!(stream.len(), 3);

        // 4th push should evict oldest
        stream.push(4.0);
        assert_eq!(stream.len(), 3);
    }

    #[tokio::test]
    async fn ring_buffer_evicts_oldest() {
        let device = test_device().await;
        let mut stream = DataStream::<f32>::builder()
            .capacity(3)
            .mode(StreamMode::RingBuffer)
            .build(&device)
            .unwrap();

        stream.push_batch(vec![1.0, 2.0, 3.0, 4.0]);
        assert_eq!(stream.len(), 3);
    }

    #[tokio::test]
    async fn append_only_drop_newest() {
        let device = test_device().await;
        let mut stream = DataStream::<f32>::builder()
            .capacity(3)
            .mode(StreamMode::AppendOnly)
            .backpressure(BackpressureStrategy::DropNewest)
            .build(&device)
            .unwrap();

        stream.push_batch(vec![1.0, 2.0, 3.0]);
        assert!(!stream.push(4.0)); // should be dropped
        assert_eq!(stream.len(), 3);
    }

    #[tokio::test]
    async fn append_only_evict_oldest() {
        let device = test_device().await;
        let mut stream = DataStream::<f32>::builder()
            .capacity(3)
            .mode(StreamMode::AppendOnly)
            .backpressure(BackpressureStrategy::EvictOldest)
            .build(&device)
            .unwrap();

        stream.push_batch(vec![1.0, 2.0, 3.0]);
        assert!(stream.push(4.0)); // should evict oldest
        assert_eq!(stream.len(), 3);
    }

    #[tokio::test]
    async fn append_only_block_drops_synchronously() {
        let device = test_device().await;
        let mut stream = DataStream::<f32>::builder()
            .capacity(3)
            .mode(StreamMode::AppendOnly)
            .backpressure(BackpressureStrategy::Block)
            .build(&device)
            .unwrap();

        stream.push_batch(vec![1.0, 2.0, 3.0]);
        assert!(!stream.push(4.0)); // sync push drops
        assert_eq!(stream.len(), 3);
    }

    // -- Subscriber tests ----------------------------------------------------

    #[tokio::test]
    async fn subscribe_receives_updates() {
        let device = test_device().await;
        let mut stream = DataStream::<f32>::builder()
            .capacity(100)
            .build(&device)
            .unwrap();

        let count = Arc::new(AtomicUsize::new(0));
        let count_clone = count.clone();
        stream.subscribe(move |_| {
            count_clone.fetch_add(1, AtomicOrdering::Relaxed);
        });

        stream.push(1.0);
        stream.push(2.0);
        assert_eq!(count.load(AtomicOrdering::Relaxed), 2);
    }

    #[tokio::test]
    async fn multiple_subscribers_called_in_order() {
        let device = test_device().await;
        let mut stream = DataStream::<f32>::builder()
            .capacity(100)
            .build(&device)
            .unwrap();

        let order = Arc::new(std::sync::Mutex::new(Vec::new()));
        let order1 = order.clone();
        let order2 = order.clone();

        stream.subscribe(move |_| {
            order1.lock().unwrap().push(1);
        });
        stream.subscribe(move |_| {
            order2.lock().unwrap().push(2);
        });

        stream.push(42.0);

        let recorded = order.lock().unwrap().clone();
        assert_eq!(recorded, vec![1, 2]);
    }

    #[tokio::test]
    async fn unsubscribe_removes_callback() {
        let device = test_device().await;
        let mut stream = DataStream::<f32>::builder()
            .capacity(100)
            .build(&device)
            .unwrap();

        let count = Arc::new(AtomicUsize::new(0));
        let count_clone = count.clone();
        let handle = stream.subscribe(move |_| {
            count_clone.fetch_add(1, AtomicOrdering::Relaxed);
        });

        stream.push(1.0);
        assert_eq!(count.load(AtomicOrdering::Relaxed), 1);

        assert!(stream.unsubscribe(handle));
        stream.push(2.0);
        // Count should not have increased
        assert_eq!(count.load(AtomicOrdering::Relaxed), 1);
    }

    #[tokio::test]
    async fn unsubscribe_unknown_handle_returns_false() {
        let device = test_device().await;
        let mut stream = DataStream::<f32>::builder()
            .capacity(100)
            .build(&device)
            .unwrap();

        let fake_handle = SubscriberHandle(999_999);
        assert!(!stream.unsubscribe(fake_handle));
    }

    #[tokio::test]
    async fn zero_subscribers_no_overhead() {
        let device = test_device().await;
        let mut stream = DataStream::<f32>::builder()
            .capacity(1_000)
            .build(&device)
            .unwrap();

        // Just ensure no panic with zero subscribers
        for i in 0..100 {
            stream.push(i as f32);
        }
        assert_eq!(stream.len(), 100);
    }

    // -- Mode × BackpressureStrategy combination tests -----------------------

    #[tokio::test]
    async fn all_valid_combinations() {
        let device = test_device().await;
        let modes = [
            StreamMode::AppendOnly,
            StreamMode::SlidingWindow,
            StreamMode::RingBuffer,
        ];
        let strategies = [
            BackpressureStrategy::Block,
            BackpressureStrategy::DropNewest,
            BackpressureStrategy::EvictOldest,
        ];

        for mode in &modes {
            for bp in &strategies {
                let result = DataStream::<f32>::builder()
                    .capacity(10)
                    .mode(*mode)
                    .backpressure(*bp)
                    .build(&device);

                let should_fail = *bp == BackpressureStrategy::Block
                    && matches!(mode, StreamMode::SlidingWindow | StreamMode::RingBuffer);

                if should_fail {
                    assert!(result.is_err(), "Expected error for {mode:?} + {bp:?}");
                } else {
                    assert!(result.is_ok(), "Expected ok for {mode:?} + {bp:?}");
                }
            }
        }
    }

    // -- Flush test ----------------------------------------------------------

    #[tokio::test]
    async fn flush_writes_to_gpu() {
        let ctx = RenderContext::new().await.unwrap();
        let device = ctx.device();
        let queue = ctx.queue();

        let mut stream = DataStream::<f32>::builder()
            .capacity(100)
            .build(device)
            .unwrap();

        stream.push(1.0);
        stream.push(2.0);
        assert!(stream.is_dirty());

        let bytes = stream.flush(device, queue);
        assert!(bytes > 0);
        assert!(!stream.is_dirty());
    }

    // -- Error conversion test -----------------------------------------------

    #[test]
    fn data_stream_error_into_gup_error() {
        let err = DataStreamError::InvalidCapacity {
            message: "test".into(),
        };
        let gup_err: crate::error::GupError = err.into();
        assert!(matches!(
            gup_err,
            crate::error::GupError::ConfigurationError { .. }
        ));
    }

    #[test]
    fn data_stream_error_display() {
        let err = DataStreamError::InvalidCapacity {
            message: "zero not allowed".into(),
        };
        let msg = err.to_string();
        assert!(msg.contains("capacity"));
        assert!(msg.contains("zero not allowed"));
    }
}

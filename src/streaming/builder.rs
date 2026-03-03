// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Fluent builder for constructing a [`DataStream<T>`](super::DataStream).
//!
//! # Examples
//!
//! ```no_run
//! use gup::streaming::{DataStream, StreamMode, BackpressureStrategy};
//!
//! # async fn example(device: &wgpu::Device) {
//! let stream = DataStream::<[f32; 4]>::builder()
//!     .capacity(10_000)
//!     .mode(StreamMode::SlidingWindow)
//!     .backpressure(BackpressureStrategy::EvictOldest)
//!     .build(device)
//!     .expect("valid stream configuration");
//! # }
//! ```

use super::backpressure::BackpressureStrategy;
use super::mode::StreamMode;
use super::stream::DataStream;

/// Error returned when [`DataStreamBuilder::build`] encounters invalid
/// configuration.
///
/// Implements [`std::error::Error`] for `?` propagation.
///
/// # Examples
///
/// ```
/// use gup::streaming::DataStreamError;
///
/// let err = DataStreamError::InvalidCapacity {
///     message: "capacity must be > 0".into(),
/// };
/// assert!(err.to_string().contains("capacity"));
/// ```
#[derive(Debug, Clone, thiserror::Error)]
pub enum DataStreamError {
    /// The requested capacity is invalid (e.g. zero).
    #[error("Invalid stream capacity: {message}")]
    InvalidCapacity { message: String },

    /// The chosen [`StreamMode`] and [`BackpressureStrategy`] combination is
    /// not supported.
    #[error("Unsupported mode/backpressure combination: {message}")]
    UnsupportedCombination { message: String },

    /// A required configuration field was not set.
    #[error("Missing required configuration: {message}")]
    MissingConfiguration { message: String },
}

/// Fluent builder for constructing a [`DataStream<T>`].
///
/// Obtain a builder via [`DataStream::builder()`], configure it with
/// chainable methods, then call [`.build(device)`](Self::build) to create
/// the stream.
///
/// # Type Parameters
///
/// * `T` — The data element type. Must be [`bytemuck::Pod`] and
///   [`bytemuck::Zeroable`] so elements can be memcpy'd to GPU buffers.
///
/// # Examples
///
/// ```no_run
/// use gup::streaming::{DataStream, StreamMode, BackpressureStrategy};
///
/// # async fn example(device: &wgpu::Device) {
/// let stream = DataStream::<[f32; 4]>::builder()
///     .capacity(5_000)
///     .mode(StreamMode::AppendOnly)
///     .backpressure(BackpressureStrategy::DropNewest)
///     .build(device)
///     .unwrap();
/// # }
/// ```
pub struct DataStreamBuilder<T: bytemuck::Pod + bytemuck::Zeroable> {
    capacity: Option<usize>,
    mode: StreamMode,
    backpressure: BackpressureStrategy,
    _marker: std::marker::PhantomData<T>,
}

impl<T: bytemuck::Pod + bytemuck::Zeroable> DataStreamBuilder<T> {
    /// Create a new builder with default settings.
    ///
    /// Defaults:
    /// * **mode**: [`StreamMode::RingBuffer`]
    /// * **backpressure**: [`BackpressureStrategy::EvictOldest`]
    /// * **capacity**: must be set explicitly before calling
    ///   [`.build()`](Self::build).
    pub(crate) fn new() -> Self {
        Self {
            capacity: None,
            mode: StreamMode::default(),
            backpressure: BackpressureStrategy::default(),
            _marker: std::marker::PhantomData,
        }
    }

    /// Set the maximum number of data points the stream can hold.
    ///
    /// When the buffer is full, the [`BackpressureStrategy`] and
    /// [`StreamMode`] determine what happens to incoming data.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use gup::streaming::DataStream;
    ///
    /// # async fn example(device: &wgpu::Device) {
    /// let stream = DataStream::<f32>::builder()
    ///     .capacity(1_000)
    ///     .build(device)
    ///     .unwrap();
    /// # }
    /// ```
    pub fn capacity(mut self, n: usize) -> Self {
        self.capacity = Some(n);
        self
    }

    /// Set the stream mode that controls eviction and overwrite semantics.
    ///
    /// See [`StreamMode`] for details on each variant.
    pub fn mode(mut self, mode: StreamMode) -> Self {
        self.mode = mode;
        self
    }

    /// Set the backpressure strategy for when the stream reaches capacity.
    ///
    /// See [`BackpressureStrategy`] for details on each variant.
    pub fn backpressure(mut self, strategy: BackpressureStrategy) -> Self {
        self.backpressure = strategy;
        self
    }

    /// Consume the builder and create a [`DataStream<T>`].
    ///
    /// # Errors
    ///
    /// Returns [`DataStreamError::InvalidCapacity`] if capacity is zero or
    /// was not set.
    ///
    /// Returns [`DataStreamError::UnsupportedCombination`] if the chosen mode
    /// and backpressure strategy are incompatible. Currently, using
    /// [`BackpressureStrategy::Block`] with
    /// [`StreamMode::SlidingWindow`] or [`StreamMode::RingBuffer`] is not
    /// supported because those modes inherently evict data.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use gup::streaming::{DataStream, StreamMode, BackpressureStrategy};
    ///
    /// # async fn example(device: &wgpu::Device) {
    /// // Valid configuration
    /// let stream = DataStream::<f32>::builder()
    ///     .capacity(100)
    ///     .build(device);
    /// assert!(stream.is_ok());
    ///
    /// // Zero capacity is invalid
    /// let err = DataStream::<f32>::builder()
    ///     .capacity(0)
    ///     .build(device);
    /// assert!(err.is_err());
    /// # }
    /// ```
    pub fn build(self, device: &wgpu::Device) -> Result<DataStream<T>, DataStreamError> {
        // Validate capacity
        let capacity = self
            .capacity
            .ok_or_else(|| DataStreamError::InvalidCapacity {
                message: "capacity must be set before calling build()".into(),
            })?;

        if capacity == 0 {
            return Err(DataStreamError::InvalidCapacity {
                message: "capacity must be greater than zero".into(),
            });
        }

        // Validate mode/backpressure combination
        if self.backpressure == BackpressureStrategy::Block
            && matches!(
                self.mode,
                StreamMode::SlidingWindow | StreamMode::RingBuffer
            )
        {
            return Err(DataStreamError::UnsupportedCombination {
                message: format!(
                    "Block backpressure is not compatible with {} mode \
                     (those modes inherently evict data)",
                    self.mode
                ),
            });
        }

        Ok(DataStream::from_builder(
            device,
            capacity,
            self.mode,
            self.backpressure,
        ))
    }
}

impl<T: bytemuck::Pod + bytemuck::Zeroable> std::fmt::Debug for DataStreamBuilder<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataStreamBuilder")
            .field("capacity", &self.capacity)
            .field("mode", &self.mode)
            .field("backpressure", &self.backpressure)
            .finish()
    }
}

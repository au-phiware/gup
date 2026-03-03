// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Stream mode variants that control eviction and overwrite semantics.
//!
//! Each [`StreamMode`] maps to a different behaviour when the underlying
//! [`StreamingBuffer`](super::StreamingBuffer) reaches capacity.

/// Controls how a [`DataStream`](super::DataStream) handles data when the
/// buffer reaches its configured capacity.
///
/// # Examples
///
/// ```
/// use gup::streaming::StreamMode;
///
/// let mode = StreamMode::SlidingWindow;
/// assert_eq!(format!("{mode:?}"), "SlidingWindow");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StreamMode {
    /// New data is appended. When the buffer is full, pushes are governed by
    /// the [`BackpressureStrategy`](super::BackpressureStrategy).
    AppendOnly,

    /// The buffer acts as a sliding window: once full, the oldest item is
    /// evicted on every new push so the most recent `capacity` items are
    /// always retained.
    SlidingWindow,

    /// The buffer wraps around, overwriting the oldest slot. This is the
    /// lowest-overhead mode and mirrors the underlying
    /// [`RingBuffer`](super::RingBuffer) directly.
    RingBuffer,
}

impl Default for StreamMode {
    fn default() -> Self {
        Self::RingBuffer
    }
}

impl std::fmt::Display for StreamMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AppendOnly => write!(f, "AppendOnly"),
            Self::SlidingWindow => write!(f, "SlidingWindow"),
            Self::RingBuffer => write!(f, "RingBuffer"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_ring_buffer() {
        assert_eq!(StreamMode::default(), StreamMode::RingBuffer);
    }

    #[test]
    fn display_variants() {
        assert_eq!(StreamMode::AppendOnly.to_string(), "AppendOnly");
        assert_eq!(StreamMode::SlidingWindow.to_string(), "SlidingWindow");
        assert_eq!(StreamMode::RingBuffer.to_string(), "RingBuffer");
    }
}

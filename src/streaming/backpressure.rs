// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Backpressure strategies for [`DataStream`](super::DataStream).
//!
//! When a stream operating in [`AppendOnly`](super::StreamMode::AppendOnly)
//! mode reaches capacity, the [`BackpressureStrategy`] determines what happens
//! to incoming data.

/// Determines behaviour when a push is attempted on a full
/// [`DataStream`](super::DataStream) operating in
/// [`AppendOnly`](super::StreamMode::AppendOnly) mode.
///
/// For [`SlidingWindow`](super::StreamMode::SlidingWindow) and
/// [`RingBuffer`](super::StreamMode::RingBuffer) modes this setting is
/// informational only — those modes always evict the oldest entry.
///
/// # Examples
///
/// ```
/// use gup::streaming::BackpressureStrategy;
///
/// let strategy = BackpressureStrategy::DropNewest;
/// assert_eq!(format!("{strategy:?}"), "DropNewest");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum BackpressureStrategy {
    /// Block the caller until space becomes available. Useful when data
    /// integrity is paramount and the consumer can keep up on average.
    Block,

    /// Silently drop the newest incoming data point. The stream remains at
    /// capacity and no existing data is disturbed.
    DropNewest,

    /// Evict the oldest data point to make room for the incoming one. This
    /// is semantically equivalent to a ring-buffer overwrite.
    #[default]
    EvictOldest,
}

impl std::fmt::Display for BackpressureStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Block => write!(f, "Block"),
            Self::DropNewest => write!(f, "DropNewest"),
            Self::EvictOldest => write!(f, "EvictOldest"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_evict_oldest() {
        assert_eq!(
            BackpressureStrategy::default(),
            BackpressureStrategy::EvictOldest
        );
    }

    #[test]
    fn display_variants() {
        assert_eq!(BackpressureStrategy::Block.to_string(), "Block");
        assert_eq!(BackpressureStrategy::DropNewest.to_string(), "DropNewest");
        assert_eq!(BackpressureStrategy::EvictOldest.to_string(), "EvictOldest");
    }
}

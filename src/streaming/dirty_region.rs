// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Dirty-region tracking for incremental GPU buffer updates.
//!
//! Tracks which byte ranges of a buffer have been modified so that only those
//! ranges need to be flushed to the GPU. Adjacent and overlapping regions are
//! automatically merged to minimise the number of `queue.write_buffer` calls.

use std::fmt;

/// A contiguous byte range within a buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BufferRegion {
    /// Byte offset from the start of the buffer.
    pub offset: usize,
    /// Length in bytes.
    pub len: usize,
}

impl BufferRegion {
    /// Create a new buffer region.
    pub fn new(offset: usize, len: usize) -> Self {
        Self { offset, len }
    }

    /// The exclusive end byte of this region.
    pub fn end(&self) -> usize {
        self.offset + self.len
    }

    /// Whether this region overlaps or is adjacent to `other`.
    pub fn touches(&self, other: &Self) -> bool {
        self.offset <= other.end() && other.offset <= self.end()
    }

    /// Merge `other` into this region, returning the union.
    pub fn merge(&self, other: &Self) -> Self {
        let start = self.offset.min(other.offset);
        let end = self.end().max(other.end());
        Self {
            offset: start,
            len: end - start,
        }
    }
}

impl fmt::Display for BufferRegion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}..{})", self.offset, self.end())
    }
}

/// Tracks dirty byte regions and merges adjacent/overlapping ranges.
#[derive(Debug, Clone)]
pub struct DirtyRegionTracker {
    /// Sorted, non-overlapping dirty regions.
    regions: Vec<BufferRegion>,
}

impl DirtyRegionTracker {
    /// Create a new, empty tracker.
    pub fn new() -> Self {
        Self {
            regions: Vec::new(),
        }
    }

    /// Mark a byte range as dirty.
    ///
    /// Adjacent and overlapping regions are merged automatically.
    pub fn mark_dirty(&mut self, region: BufferRegion) {
        if region.len == 0 {
            return;
        }

        // Find all existing regions that touch the new one and merge them.
        let mut merged = region;
        self.regions.retain(|existing| {
            if merged.touches(existing) {
                merged = merged.merge(existing);
                false // remove; it's absorbed into `merged`
            } else {
                true
            }
        });

        // Insert merged region in sorted order.
        let pos = self
            .regions
            .binary_search_by_key(&merged.offset, |r| r.offset)
            .unwrap_or_else(|p| p);
        self.regions.insert(pos, merged);
    }

    /// Take all dirty regions, leaving the tracker empty.
    pub fn drain(&mut self) -> Vec<BufferRegion> {
        std::mem::take(&mut self.regions)
    }

    /// Number of disjoint dirty regions currently tracked.
    pub fn region_count(&self) -> usize {
        self.regions.len()
    }

    /// Whether there are any dirty regions.
    pub fn is_dirty(&self) -> bool {
        !self.regions.is_empty()
    }

    /// Total number of dirty bytes across all regions.
    pub fn dirty_bytes(&self) -> usize {
        self.regions.iter().map(|r| r.len).sum()
    }

    /// Read-only view of the current dirty regions.
    pub fn regions(&self) -> &[BufferRegion] {
        &self.regions
    }

    /// Clear all tracked dirty regions without returning them.
    pub fn clear(&mut self) {
        self.regions.clear();
    }
}

impl Default for DirtyRegionTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_tracker() {
        let tracker = DirtyRegionTracker::new();
        assert!(!tracker.is_dirty());
        assert_eq!(tracker.region_count(), 0);
        assert_eq!(tracker.dirty_bytes(), 0);
    }

    #[test]
    fn single_region() {
        let mut tracker = DirtyRegionTracker::new();
        tracker.mark_dirty(BufferRegion::new(10, 20));

        assert!(tracker.is_dirty());
        assert_eq!(tracker.region_count(), 1);
        assert_eq!(tracker.dirty_bytes(), 20);
        assert_eq!(tracker.regions()[0], BufferRegion::new(10, 20));
    }

    #[test]
    fn disjoint_regions_stay_separate() {
        let mut tracker = DirtyRegionTracker::new();
        tracker.mark_dirty(BufferRegion::new(0, 10));
        tracker.mark_dirty(BufferRegion::new(100, 10));

        assert_eq!(tracker.region_count(), 2);
        assert_eq!(tracker.dirty_bytes(), 20);
    }

    #[test]
    fn overlapping_regions_merge() {
        let mut tracker = DirtyRegionTracker::new();
        tracker.mark_dirty(BufferRegion::new(0, 20));
        tracker.mark_dirty(BufferRegion::new(10, 20));

        assert_eq!(tracker.region_count(), 1);
        assert_eq!(tracker.regions()[0], BufferRegion::new(0, 30));
    }

    #[test]
    fn adjacent_regions_merge() {
        let mut tracker = DirtyRegionTracker::new();
        tracker.mark_dirty(BufferRegion::new(0, 10));
        tracker.mark_dirty(BufferRegion::new(10, 10));

        assert_eq!(tracker.region_count(), 1);
        assert_eq!(tracker.regions()[0], BufferRegion::new(0, 20));
    }

    #[test]
    fn new_region_spans_multiple_existing() {
        let mut tracker = DirtyRegionTracker::new();
        tracker.mark_dirty(BufferRegion::new(0, 5));
        tracker.mark_dirty(BufferRegion::new(10, 5));
        tracker.mark_dirty(BufferRegion::new(20, 5));
        assert_eq!(tracker.region_count(), 3);

        // Bridge them all
        tracker.mark_dirty(BufferRegion::new(3, 20));
        assert_eq!(tracker.region_count(), 1);
        assert_eq!(tracker.regions()[0], BufferRegion::new(0, 25));
    }

    #[test]
    fn drain_clears_tracker() {
        let mut tracker = DirtyRegionTracker::new();
        tracker.mark_dirty(BufferRegion::new(0, 10));
        tracker.mark_dirty(BufferRegion::new(50, 10));

        let regions = tracker.drain();
        assert_eq!(regions.len(), 2);
        assert!(!tracker.is_dirty());
    }

    #[test]
    fn zero_length_region_is_ignored() {
        let mut tracker = DirtyRegionTracker::new();
        tracker.mark_dirty(BufferRegion::new(10, 0));
        assert!(!tracker.is_dirty());
    }

    #[test]
    fn region_display() {
        let r = BufferRegion::new(10, 20);
        assert_eq!(format!("{r}"), "[10..30)");
    }

    #[test]
    fn region_touches() {
        let a = BufferRegion::new(0, 10);
        let b = BufferRegion::new(10, 10);
        let c = BufferRegion::new(11, 10);
        let d = BufferRegion::new(5, 3);

        assert!(a.touches(&b)); // adjacent
        assert!(a.touches(&d)); // overlapping
        assert!(!a.touches(&c)); // gap
    }

    #[test]
    fn region_merge() {
        let a = BufferRegion::new(5, 10);
        let b = BufferRegion::new(10, 20);
        let merged = a.merge(&b);
        assert_eq!(merged, BufferRegion::new(5, 25));
    }
}

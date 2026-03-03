// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Fixed-capacity ring buffer for bounded-memory data ingestion.
//!
//! When the buffer reaches capacity, the oldest entries are overwritten by new
//! inserts (FIFO eviction). This ensures that a streaming data source never
//! causes unbounded memory growth.

/// A fixed-capacity ring buffer backed by a contiguous `Vec`.
///
/// Items are stored in insertion order. When `capacity` is reached, the oldest
/// item is silently evicted and its slot is reused for the next insert.
#[derive(Debug, Clone)]
pub struct RingBuffer<T> {
    /// Storage – always has exactly `capacity` slots after the first
    /// `capacity` inserts. Before that it grows via `push`.
    data: Vec<Option<T>>,
    /// Index where the next item will be written.
    write_pos: usize,
    /// Number of live items (≤ capacity).
    len: usize,
    /// Maximum number of items.
    capacity: usize,
    /// Total number of items ever written (monotonically increasing).
    total_written: u64,
}

impl<T> RingBuffer<T> {
    /// Create an empty ring buffer with the given maximum capacity.
    ///
    /// # Panics
    ///
    /// Panics if `capacity` is zero.
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "RingBuffer capacity must be > 0");
        Self {
            data: Vec::with_capacity(capacity),
            write_pos: 0,
            len: 0,
            capacity,
            total_written: 0,
        }
    }

    /// Push an item into the ring buffer, returning the evicted item (if any).
    pub fn push(&mut self, item: T) -> Option<T> {
        self.total_written += 1;

        if self.data.len() < self.capacity {
            // Still growing – no eviction.
            self.data.push(Some(item));
            self.write_pos = self.data.len() % self.capacity;
            self.len = self.data.len();
            None
        } else {
            // Buffer is full – overwrite the oldest slot.
            let old = self.data[self.write_pos].take();
            self.data[self.write_pos] = Some(item);
            self.write_pos = (self.write_pos + 1) % self.capacity;
            old
        }
    }

    /// Get a reference to the item at the given *physical* index.
    pub fn get(&self, index: usize) -> Option<&T> {
        self.data.get(index).and_then(|slot| slot.as_ref())
    }

    /// Get a mutable reference to the item at the given *physical* index.
    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        self.data.get_mut(index).and_then(|slot| slot.as_mut())
    }

    /// Remove the item at the given physical index, leaving the slot empty.
    ///
    /// Returns the removed item, or `None` if the slot was already empty.
    pub fn remove(&mut self, index: usize) -> Option<T> {
        if index >= self.data.len() {
            return None;
        }
        let removed = self.data[index].take();
        if removed.is_some() {
            self.len -= 1;
        }
        removed
    }

    /// Replace the item at a physical index, returning the old value.
    pub fn replace(&mut self, index: usize, item: T) -> Option<T> {
        if index >= self.data.len() {
            return None;
        }
        let was_empty = self.data[index].is_none();
        let old = self.data[index].replace(item);
        if was_empty {
            self.len += 1;
        }
        old
    }

    /// The physical index where the next `push` will write.
    pub fn next_write_index(&self) -> usize {
        if self.data.len() < self.capacity {
            self.data.len()
        } else {
            self.write_pos
        }
    }

    /// Number of live (non-`None`) items.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Whether the buffer has reached its capacity (oldest items will be
    /// evicted on the next push).
    pub fn is_full(&self) -> bool {
        self.data.len() == self.capacity
    }

    /// Maximum capacity.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Total number of items ever written.
    pub fn total_written(&self) -> u64 {
        self.total_written
    }

    /// Iterate over all live items in insertion order (oldest first).
    pub fn iter(&self) -> impl Iterator<Item = (usize, &T)> {
        let cap = self.data.len();
        let start = if cap < self.capacity {
            0
        } else {
            self.write_pos
        };
        let data = &self.data;
        (0..cap).filter_map(move |i| {
            let physical = (start + i) % cap;
            data[physical].as_ref().map(|item| (physical, item))
        })
    }

    /// Clear all items.
    pub fn clear(&mut self) {
        for slot in &mut self.data {
            *slot = None;
        }
        self.len = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_push_and_get() {
        let mut rb = RingBuffer::new(4);
        rb.push(10);
        rb.push(20);
        rb.push(30);

        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(&10));
        assert_eq!(rb.get(1), Some(&20));
        assert_eq!(rb.get(2), Some(&30));
    }

    #[test]
    fn eviction_on_overflow() {
        let mut rb = RingBuffer::new(3);
        assert_eq!(rb.push(1), None);
        assert_eq!(rb.push(2), None);
        assert_eq!(rb.push(3), None);
        assert!(rb.is_full());

        // Pushing a 4th item evicts item 1
        let evicted = rb.push(4);
        assert_eq!(evicted, Some(1));
        assert_eq!(rb.len(), 3);
    }

    #[test]
    fn remove_and_reinsert() {
        let mut rb = RingBuffer::new(4);
        rb.push(10);
        rb.push(20);
        rb.push(30);

        let removed = rb.remove(1);
        assert_eq!(removed, Some(20));
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(1), None);

        // Replace into the empty slot
        let old = rb.replace(1, 25);
        assert_eq!(old, None);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(1), Some(&25));
    }

    #[test]
    fn replace_existing() {
        let mut rb = RingBuffer::new(4);
        rb.push(10);
        let old = rb.replace(0, 99);
        assert_eq!(old, Some(10));
        assert_eq!(rb.len(), 1); // count unchanged
        assert_eq!(rb.get(0), Some(&99));
    }

    #[test]
    fn iter_in_order() {
        let mut rb = RingBuffer::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4); // evicts 1

        let items: Vec<&i32> = rb.iter().map(|(_, v)| v).collect();
        assert_eq!(items, vec![&2, &3, &4]);
    }

    #[test]
    fn clear() {
        let mut rb = RingBuffer::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn total_written_tracks_lifetime() {
        let mut rb = RingBuffer::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        assert_eq!(rb.total_written(), 3);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    #[should_panic(expected = "capacity must be > 0")]
    fn zero_capacity_panics() {
        let _rb: RingBuffer<i32> = RingBuffer::new(0);
    }

    #[test]
    fn next_write_index_growing() {
        let mut rb = RingBuffer::new(4);
        assert_eq!(rb.next_write_index(), 0);
        rb.push(1);
        assert_eq!(rb.next_write_index(), 1);
        rb.push(2);
        assert_eq!(rb.next_write_index(), 2);
    }

    #[test]
    fn next_write_index_wrapping() {
        let mut rb = RingBuffer::new(2);
        rb.push(1);
        rb.push(2);
        assert_eq!(rb.next_write_index(), 0); // wraps
        rb.push(3);
        assert_eq!(rb.next_write_index(), 1);
    }
}

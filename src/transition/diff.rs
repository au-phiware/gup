// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Key-based data diffing for enter/update/exit partitioning.
//!
//! Given an old dataset and a new dataset with a key function that extracts a
//! stable identity from each item, [`diff_by_key`] partitions the data into
//! three groups:
//!
//! - **Enter**: items whose key is present in the new data but not in the old.
//! - **Update**: items whose key appears in both old and new, paired together.
//! - **Exit**: items whose key is present in the old data but not in the new.

use std::collections::HashMap;
use std::hash::Hash;

/// Result of diffing two datasets by key.
///
/// Contains the three groups produced by comparing old and new data: items that
/// are entering (new), updating (changed), or exiting (removed).
#[derive(Debug, Clone)]
pub struct DiffResult<T> {
    /// Items present in the new data but not in the old data.
    pub enter: Vec<T>,
    /// Pairs of `(old_item, new_item)` for keys present in both datasets.
    /// The ordering follows the new data's order.
    pub update: Vec<(T, T)>,
    /// Items present in the old data but not in the new data.
    pub exit: Vec<T>,
}

impl<T> DiffResult<T> {
    /// Returns the total number of items across all three groups.
    pub fn total_len(&self) -> usize {
        self.enter.len() + self.update.len() + self.exit.len()
    }

    /// Returns `true` if all three groups are empty.
    pub fn is_empty(&self) -> bool {
        self.enter.is_empty() && self.update.is_empty() && self.exit.is_empty()
    }
}

/// Diff two datasets by a key function, producing enter/update/exit groups.
///
/// The key function `key_fn` is evaluated on every item in both `old` and `new`
/// to determine element identity. Items are then partitioned:
///
/// - **Enter**: key exists only in `new`.
/// - **Update**: key exists in both `old` and `new` — paired as `(old, new)`.
/// - **Exit**: key exists only in `old`.
///
/// The update and enter groups preserve the ordering of `new`; the exit group
/// preserves the ordering of `old`.
///
/// # Panics
///
/// This function does not panic. Duplicate keys in either dataset are handled
/// by last-wins semantics (later items with the same key overwrite earlier ones).
///
/// # Examples
///
/// ```
/// use gup::transition::diff::diff_by_key;
///
/// let old = vec![("A", 1), ("B", 2), ("C", 3)];
/// let new = vec![("B", 20), ("C", 30), ("D", 40)];
///
/// let result = diff_by_key(&old, &new, |item| item.0);
///
/// assert_eq!(result.enter.len(), 1); // D
/// assert_eq!(result.update.len(), 2); // B, C
/// assert_eq!(result.exit.len(), 1); // A
/// ```
pub fn diff_by_key<T, K, F>(old: &[T], new: &[T], key_fn: F) -> DiffResult<T>
where
    T: Clone,
    K: Eq + Hash,
    F: Fn(&T) -> K,
{
    // Build a map from key → index into old data.
    let mut old_by_key: HashMap<K, usize> = HashMap::with_capacity(old.len());
    for (i, item) in old.iter().enumerate() {
        old_by_key.insert(key_fn(item), i);
    }

    // Track which old items are matched.
    let mut matched = vec![false; old.len()];

    let mut enter = Vec::new();
    let mut update = Vec::new();

    for new_item in new {
        let key = key_fn(new_item);
        if let Some(&old_idx) = old_by_key.get(&key) {
            update.push((old[old_idx].clone(), new_item.clone()));
            matched[old_idx] = true;
        } else {
            enter.push(new_item.clone());
        }
    }

    // Everything in old that wasn't matched is an exit.
    let exit: Vec<T> = old
        .iter()
        .enumerate()
        .filter(|(i, _)| !matched[*i])
        .map(|(_, item)| item.clone())
        .collect();

    DiffResult {
        enter,
        update,
        exit,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_diff() {
        let old = vec!["A", "B", "C"];
        let new = vec!["B", "C", "D"];
        let result = diff_by_key(&old, &new, |item| *item);

        assert_eq!(result.enter, vec!["D"]);
        assert_eq!(result.update.len(), 2);
        assert_eq!(result.update[0], ("B", "B"));
        assert_eq!(result.update[1], ("C", "C"));
        assert_eq!(result.exit, vec!["A"]);
    }

    #[test]
    fn test_all_enter() {
        let old: Vec<&str> = vec![];
        let new = vec!["A", "B", "C"];
        let result = diff_by_key(&old, &new, |item| *item);

        assert_eq!(result.enter, vec!["A", "B", "C"]);
        assert!(result.update.is_empty());
        assert!(result.exit.is_empty());
    }

    #[test]
    fn test_all_exit() {
        let old = vec!["A", "B", "C"];
        let new: Vec<&str> = vec![];
        let result = diff_by_key(&old, &new, |item| *item);

        assert!(result.enter.is_empty());
        assert!(result.update.is_empty());
        assert_eq!(result.exit, vec!["A", "B", "C"]);
    }

    #[test]
    fn test_all_update() {
        let old = vec!["A", "B", "C"];
        let new = vec!["A", "B", "C"];
        let result = diff_by_key(&old, &new, |item| *item);

        assert!(result.enter.is_empty());
        assert_eq!(result.update.len(), 3);
        assert!(result.exit.is_empty());
    }

    #[test]
    fn test_empty_both() {
        let old: Vec<&str> = vec![];
        let new: Vec<&str> = vec![];
        let result = diff_by_key(&old, &new, |item| *item);

        assert!(result.is_empty());
        assert_eq!(result.total_len(), 0);
    }

    #[test]
    fn test_no_overlap() {
        let old = vec!["A", "B"];
        let new = vec!["C", "D"];
        let result = diff_by_key(&old, &new, |item| *item);

        assert_eq!(result.enter, vec!["C", "D"]);
        assert!(result.update.is_empty());
        assert_eq!(result.exit, vec!["A", "B"]);
    }

    #[test]
    fn test_struct_with_key() {
        #[derive(Debug, Clone, PartialEq)]
        struct Point {
            id: u32,
            x: f32,
            y: f32,
        }

        let old = vec![
            Point {
                id: 1,
                x: 0.0,
                y: 0.0,
            },
            Point {
                id: 2,
                x: 1.0,
                y: 1.0,
            },
            Point {
                id: 3,
                x: 2.0,
                y: 2.0,
            },
        ];

        let new = vec![
            Point {
                id: 2,
                x: 10.0,
                y: 10.0,
            },
            Point {
                id: 3,
                x: 20.0,
                y: 20.0,
            },
            Point {
                id: 4,
                x: 30.0,
                y: 30.0,
            },
        ];

        let result = diff_by_key(&old, &new, |p| p.id);

        // Enter: id=4
        assert_eq!(result.enter.len(), 1);
        assert_eq!(result.enter[0].id, 4);

        // Update: id=2 and id=3
        assert_eq!(result.update.len(), 2);
        assert_eq!(result.update[0].0.id, 2);
        assert_eq!(result.update[0].0.x, 1.0); // old value
        assert_eq!(result.update[0].1.x, 10.0); // new value
        assert_eq!(result.update[1].0.id, 3);

        // Exit: id=1
        assert_eq!(result.exit.len(), 1);
        assert_eq!(result.exit[0].id, 1);
    }

    #[test]
    fn test_duplicate_keys_last_wins() {
        // When old has duplicates, the later one's index is used.
        let old = vec![("A", 1), ("A", 2)];
        let new = vec![("A", 10)];
        let result = diff_by_key(&old, &new, |item| item.0);

        assert!(result.enter.is_empty());
        // The last "A" in old (value=2) should be the one matched.
        assert_eq!(result.update.len(), 1);
        assert_eq!(result.update[0].0.1, 2);
        assert_eq!(result.update[0].1.1, 10);
        // The first "A" in old (value=1) is not matched but its key
        // was overwritten in the map, so it ends up as exit.
        assert_eq!(result.exit.len(), 1);
        assert_eq!(result.exit[0].1, 1);
    }

    #[test]
    fn test_preserves_new_order_for_update() {
        let old = vec!["C", "B", "A"];
        let new = vec!["A", "B", "C"];
        let result = diff_by_key(&old, &new, |item| *item);

        // Update order should follow `new` ordering.
        assert_eq!(result.update[0], ("A", "A"));
        assert_eq!(result.update[1], ("B", "B"));
        assert_eq!(result.update[2], ("C", "C"));
    }

    #[test]
    fn test_preserves_old_order_for_exit() {
        let old = vec!["C", "B", "A"];
        let new: Vec<&str> = vec![];
        let result = diff_by_key(&old, &new, |item| *item);

        // Exit order should follow `old` ordering.
        assert_eq!(result.exit, vec!["C", "B", "A"]);
    }

    #[test]
    fn test_total_len() {
        let old = vec!["A", "B", "C"];
        let new = vec!["B", "C", "D"];
        let result = diff_by_key(&old, &new, |item| *item);

        // 1 enter + 2 update + 1 exit = 4
        assert_eq!(result.total_len(), 4);
    }
}

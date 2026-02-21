// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Data merging capabilities for Mixable compositions.
//!
//! This module provides traits and strategies for combining data sources
//! from multiple visualizations into unified datasets.

use crate::{GupError, GupResult};
use std::any::TypeId;
use std::fmt::Debug;
use std::marker::PhantomData;

/// Trait for visualizations that can expose their data for merging.
///
/// Implement this trait to enable your visualization types to participate
/// in merge composition operations, allowing their underlying data to be
/// combined with other compatible visualizations.
///
/// # Type Parameters
///
/// * `T` - The data type stored in this visualization (must be 'static for type checking)
///
/// # Examples
///
/// ```rust,no_run
/// use gup::prelude::*;
/// use gup::mixable::merge::Mergeable;
///
/// struct MyChart {
///     data: Vec<(f32, f32)>,
/// }
///
/// impl Mergeable<(f32, f32)> for MyChart {
///     fn extract_data(&self) -> &[(f32, f32)] {
///         &self.data
///     }
///
///     fn from_merged_data(data: Vec<(f32, f32)>) -> Self {
///         Self { data }
///     }
/// }
/// ```
pub trait Mergeable<T: 'static>: Debug + Send + Sync {
    /// Get a reference to the underlying data for merging.
    ///
    /// This method should return a slice of all data items in the visualization.
    fn extract_data(&self) -> &[T];

    /// Create a new visualization from merged data.
    ///
    /// This method constructs a new instance of the visualization type
    /// with the combined data from multiple sources.
    ///
    /// # Arguments
    ///
    /// * `data` - The merged dataset to visualize
    fn from_merged_data(data: Vec<T>) -> Self
    where
        Self: Sized;

    /// Check if this visualization can merge with another data type.
    ///
    /// The default implementation uses TypeId comparison to check for
    /// identical data types. Override this for custom compatibility logic.
    ///
    /// # Type Parameters
    ///
    /// * `U` - The data type to check compatibility with
    fn can_merge_with<U: 'static>(&self, _other_type: PhantomData<U>) -> bool {
        TypeId::of::<T>() == TypeId::of::<U>()
    }
}

/// Strategies for merging datasets from multiple visualizations.
///
/// Different merge strategies provide different semantics for how
/// data sources are combined.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum MergeStrategy {
    /// Append all data points from both sources.
    ///
    /// This is the simplest merge strategy: just concatenate the datasets.
    /// All data points from both visualizations are included in the result.
    #[default]
    Append,

    /// Remove duplicate data points based on equality.
    ///
    /// This strategy appends data but removes exact duplicates using
    /// PartialEq comparison. Useful when combining overlapping datasets.
    Deduplicate,

    /// Interpolate between datasets (placeholder for future implementation).
    ///
    /// This would generate intermediate data points between the two datasets.
    /// Currently not implemented.
    Interpolate {
        /// Number of interpolation steps
        steps: u32,
    },

    /// Custom merge function (placeholder for future implementation).
    ///
    /// This would allow user-defined merge logic. Due to Rust's trait
    /// object limitations, this is represented as an enum variant.
    Custom {
        /// Description of the custom merge strategy
        description: String,
    },
}

impl MergeStrategy {
    /// Apply this merge strategy to two datasets.
    ///
    /// # Type Parameters
    ///
    /// * `T` - The data type (must implement Clone for deduplication)
    ///
    /// # Arguments
    ///
    /// * `data1` - First dataset
    /// * `data2` - Second dataset
    ///
    /// # Returns
    ///
    /// The merged dataset according to the strategy
    ///
    /// # Errors
    ///
    /// Returns an error if the merge strategy is not yet implemented
    /// (Interpolate, Custom) or if the merge operation fails.
    pub fn apply<T: Clone + PartialEq>(&self, data1: &[T], data2: &[T]) -> GupResult<Vec<T>> {
        match self {
            MergeStrategy::Append => {
                // Simple concatenation
                let mut result = Vec::with_capacity(data1.len() + data2.len());
                result.extend_from_slice(data1);
                result.extend_from_slice(data2);
                Ok(result)
            }
            MergeStrategy::Deduplicate => {
                // Append and remove duplicates
                let mut result = Vec::with_capacity(data1.len() + data2.len());
                result.extend_from_slice(data1);

                // Only add items from data2 that aren't already present
                for item in data2 {
                    if !result.contains(item) {
                        result.push(item.clone());
                    }
                }

                Ok(result)
            }
            MergeStrategy::Interpolate { steps } => Err(GupError::composition_error(format!(
                "Interpolate merge strategy not yet implemented (steps: {})",
                steps
            ))),
            MergeStrategy::Custom { description } => Err(GupError::composition_error(format!(
                "Custom merge strategy not yet implemented: {}",
                description
            ))),
        }
    }

    /// Get a human-readable description of this merge strategy.
    pub fn description(&self) -> String {
        match self {
            MergeStrategy::Append => "Append (concatenate all data)".to_string(),
            MergeStrategy::Deduplicate => "Deduplicate (remove exact duplicates)".to_string(),
            MergeStrategy::Interpolate { steps } => {
                format!("Interpolate ({} steps)", steps)
            }
            MergeStrategy::Custom { description } => {
                format!("Custom ({})", description)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_append_strategy() {
        let data1 = vec![1, 2, 3];
        let data2 = vec![4, 5, 6];

        let merged = MergeStrategy::Append.apply(&data1, &data2).unwrap();

        assert_eq!(merged, vec![1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn test_deduplicate_strategy() {
        let data1 = vec![1, 2, 3];
        let data2 = vec![2, 3, 4, 5];

        let merged = MergeStrategy::Deduplicate.apply(&data1, &data2).unwrap();

        assert_eq!(merged, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_deduplicate_no_duplicates() {
        let data1 = vec![1, 2, 3];
        let data2 = vec![4, 5, 6];

        let merged = MergeStrategy::Deduplicate.apply(&data1, &data2).unwrap();

        assert_eq!(merged, vec![1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn test_deduplicate_all_duplicates() {
        let data1 = vec![1, 2, 3];
        let data2 = vec![1, 2, 3];

        let merged = MergeStrategy::Deduplicate.apply(&data1, &data2).unwrap();

        assert_eq!(merged, vec![1, 2, 3]);
    }

    #[test]
    fn test_interpolate_not_implemented() {
        let data1 = vec![1.0, 2.0];
        let data2 = vec![3.0, 4.0];

        let result = MergeStrategy::Interpolate { steps: 5 }.apply(&data1, &data2);

        assert!(result.is_err());
    }

    #[test]
    fn test_custom_not_implemented() {
        let data1 = vec![1, 2];
        let data2 = vec![3, 4];

        let result = MergeStrategy::Custom {
            description: "test strategy".to_string(),
        }
        .apply(&data1, &data2);

        assert!(result.is_err());
    }

    #[test]
    fn test_merge_strategy_description() {
        assert_eq!(
            MergeStrategy::Append.description(),
            "Append (concatenate all data)"
        );
        assert_eq!(
            MergeStrategy::Deduplicate.description(),
            "Deduplicate (remove exact duplicates)"
        );
        assert_eq!(
            MergeStrategy::Interpolate { steps: 5 }.description(),
            "Interpolate (5 steps)"
        );
        assert_eq!(
            MergeStrategy::Custom {
                description: "my strategy".to_string()
            }
            .description(),
            "Custom (my strategy)"
        );
    }

    #[derive(Debug, Clone, PartialEq)]
    struct TestData {
        value: i32,
    }

    #[derive(Debug)]
    struct TestVisualization {
        data: Vec<TestData>,
    }

    impl Mergeable<TestData> for TestVisualization {
        fn extract_data(&self) -> &[TestData] {
            &self.data
        }

        fn from_merged_data(data: Vec<TestData>) -> Self {
            Self { data }
        }
    }

    #[test]
    fn test_mergeable_trait() {
        let viz = TestVisualization {
            data: vec![TestData { value: 1 }, TestData { value: 2 }],
        };

        let extracted = viz.extract_data();
        assert_eq!(extracted.len(), 2);
        assert_eq!(extracted[0].value, 1);
        assert_eq!(extracted[1].value, 2);
    }

    #[test]
    fn test_mergeable_from_merged_data() {
        let merged_data = vec![
            TestData { value: 1 },
            TestData { value: 2 },
            TestData { value: 3 },
        ];

        let viz = TestVisualization::from_merged_data(merged_data);

        assert_eq!(viz.data.len(), 3);
        assert_eq!(viz.data[0].value, 1);
    }

    #[test]
    fn test_can_merge_with_same_type() {
        let viz = TestVisualization {
            data: vec![TestData { value: 1 }],
        };

        assert!(viz.can_merge_with(PhantomData::<TestData>));
    }

    #[test]
    fn test_can_merge_with_different_type() {
        let viz = TestVisualization {
            data: vec![TestData { value: 1 }],
        };

        assert!(!viz.can_merge_with(PhantomData::<i32>));
    }
}

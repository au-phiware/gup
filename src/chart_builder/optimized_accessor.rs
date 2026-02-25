// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Optimized compile-time accessor system for zero-cost abstractions.
//!
//! This module provides generic accessor types that resolve at compile time,
//! eliminating the dynamic dispatch and type-erased value overhead of the
//! standard accessor system.

use std::marker::PhantomData;

/// Zero-cost generic field accessor.
///
/// Unlike the standard `AccessorFunction` which uses `Box<dyn Fn>`, this accessor
/// uses generics to eliminate dynamic dispatch entirely. The compiler can inline
/// these accessors and optimize them to direct field access.
///
/// # Type Parameters
///
/// - `T`: The data type
/// - `Output`: The output type (f32, [f32; 4], etc.)
/// - `F`: The accessor function type (inferred from closure)
pub struct GenericAccessor<T, Output, F>
where
    F: Fn(&T) -> Output,
{
    accessor: F,
    _phantom: PhantomData<(T, Output)>,
}

impl<T, Output, F> GenericAccessor<T, Output, F>
where
    F: Fn(&T) -> Output,
{
    /// Create a new generic accessor from a closure.
    ///
    /// The closure is stored without boxing, allowing the compiler to
    /// inline it completely.
    #[inline(always)]
    pub fn new(accessor: F) -> Self {
        Self {
            accessor,
            _phantom: PhantomData,
        }
    }

    /// Extract a value from the data.
    ///
    /// This method is marked `inline(always)` to ensure the compiler
    /// optimizes it to direct field access when possible.
    #[inline(always)]
    pub fn extract(&self, data: &T) -> Output {
        (self.accessor)(data)
    }
}

/// Convenience macro for creating zero-cost field accessors.
///
/// This macro generates a closure that directly accesses a field,
/// which the compiler can optimize to a simple memory read.
///
/// # Examples
///
/// ```rust,no_run
/// # use gup::chart_builder::optimized_accessor::*;
/// # use gup::field_accessor;
/// #[derive(Clone)]
/// struct Point {
///     x: f32,
///     y: f32,
/// }
///
/// let x_accessor = field_accessor!(Point, x);
/// let point = Point { x: 10.0, y: 20.0 };
/// assert_eq!(x_accessor.extract(&point), 10.0);
/// ```
#[macro_export]
macro_rules! field_accessor {
    ($type:ty, $field:ident) => {
        $crate::chart_builder::optimized_accessor::GenericAccessor::new(|data: &$type| data.$field)
    };
}

/// Optimized accessor function that preserves type information.
///
/// This is the optimized equivalent of `AccessorFunction` that uses
/// generics instead of trait objects for zero-cost abstraction.
///
/// # Type Parameters
///
/// - `T`: The data type
/// - `Output`: The output type
/// - `F`: The accessor function type
pub struct OptimizedAccessorFunction<T, Output, F>
where
    F: Fn(&T) -> Output + Send + Sync,
{
    function: F,
    _phantom: PhantomData<(T, Output)>,
}

impl<T, Output, F> OptimizedAccessorFunction<T, Output, F>
where
    F: Fn(&T) -> Output + Send + Sync,
{
    /// Create a new optimized accessor function.
    #[inline(always)]
    pub fn new(function: F) -> Self {
        Self {
            function,
            _phantom: PhantomData,
        }
    }

    /// Apply the accessor function.
    #[inline(always)]
    pub fn apply(&self, data: &T) -> Output {
        (self.function)(data)
    }
}

/// Trait for types that can be used as accessor functions.
///
/// This enables automatic conversion from closures to accessor functions
/// while preserving type information for zero-cost abstraction.
pub trait IntoOptimizedAccessor<T, Output> {
    /// The accessor function type that this converts to.
    type Accessor: Fn(&T) -> Output + Send + Sync;

    /// Convert this type into an optimized accessor.
    fn into_optimized_accessor(self) -> OptimizedAccessorFunction<T, Output, Self::Accessor>
    where
        Self: Sized;
}

impl<T, Output, F> IntoOptimizedAccessor<T, Output> for F
where
    F: Fn(&T) -> Output + Send + Sync,
{
    type Accessor = F;

    #[inline(always)]
    fn into_optimized_accessor(self) -> OptimizedAccessorFunction<T, Output, F> {
        OptimizedAccessorFunction::new(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone)]
    struct TestData {
        x: f32,
        y: f32,
        name: String,
    }

    #[test]
    fn test_generic_accessor_f32() {
        let data = TestData {
            x: 10.0,
            y: 20.0,
            name: "test".to_string(),
        };

        let x_accessor = GenericAccessor::new(|d: &TestData| d.x);
        assert_eq!(x_accessor.extract(&data), 10.0);

        let y_accessor = GenericAccessor::new(|d: &TestData| d.y);
        assert_eq!(y_accessor.extract(&data), 20.0);
    }

    #[test]
    fn test_generic_accessor_string() {
        let data = TestData {
            x: 10.0,
            y: 20.0,
            name: "hello".to_string(),
        };

        let name_accessor = GenericAccessor::new(|d: &TestData| d.name.clone());
        assert_eq!(name_accessor.extract(&data), "hello");
    }

    #[test]
    fn test_field_accessor_macro() {
        let data = TestData {
            x: 42.0,
            y: 24.0,
            name: "macro".to_string(),
        };

        let x_accessor = field_accessor!(TestData, x);
        assert_eq!(x_accessor.extract(&data), 42.0);

        let y_accessor = field_accessor!(TestData, y);
        assert_eq!(y_accessor.extract(&data), 24.0);
    }

    #[test]
    fn test_optimized_accessor_function() {
        let data = TestData {
            x: 15.0,
            y: 25.0,
            name: "optimized".to_string(),
        };

        let x_accessor = OptimizedAccessorFunction::new(|d: &TestData| d.x);
        assert_eq!(x_accessor.apply(&data), 15.0);

        let computed_accessor = OptimizedAccessorFunction::new(|d: &TestData| d.x + d.y);
        assert_eq!(computed_accessor.apply(&data), 40.0);
    }

    #[test]
    fn test_into_optimized_accessor() {
        let data = TestData {
            x: 5.0,
            y: 10.0,
            name: "trait".to_string(),
        };

        let x_accessor = (|d: &TestData| d.x).into_optimized_accessor();
        assert_eq!(x_accessor.apply(&data), 5.0);
    }

    #[test]
    fn test_complex_accessor() {
        let data = TestData {
            x: 3.0,
            y: 4.0,
            name: "complex".to_string(),
        };

        // Test complex computation with generic accessor
        let magnitude_accessor =
            GenericAccessor::new(|d: &TestData| (d.x * d.x + d.y * d.y).sqrt());

        assert_eq!(magnitude_accessor.extract(&data), 5.0);
    }

    #[test]
    fn test_color_accessor() {
        let data = TestData {
            x: 0.5,
            y: 0.75,
            name: "color".to_string(),
        };

        let color_accessor = GenericAccessor::new(|d: &TestData| [d.x, d.y, 0.0, 1.0]);

        let color = color_accessor.extract(&data);
        assert_eq!(color, [0.5, 0.75, 0.0, 1.0]);
    }
}

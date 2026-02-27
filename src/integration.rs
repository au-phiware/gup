// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Integration utilities for making external visualization types compatible with Mixable.
//!
//! This module provides tools to integrate external visualization libraries and types
//! with Gup's composition system, including wrapper types, converter utilities, and
//! plugin frameworks.
//!
//! # Examples
//!
//! ## Basic External Wrapper
//!
//! ```rust
//! use gup::integration::{wrap_point_data, ExternalVisualizationWrapper};
//! use gup::{Mixable, RenderContext, GupResult};
//!
//! // External data structure
//! #[derive(Debug)]
//! struct ExternalChart {
//!     data: Vec<(f32, f32)>,
//! }
//!
//! // Make it mixable
//! let external = ExternalChart {
//!     data: vec![(0.0, 0.0), (1.0, 1.0), (2.0, 2.0)],
//! };
//!
//! let mixable = wrap_point_data(external, |chart| {
//!     chart.data.iter().map(|&(x, y)| [x, y]).collect()
//! });
//!
//! // Now it can be composed with other Mixable types
//! // let composed = mixable.mix(other_visualization);
//! ```
//!
//! ## Custom Renderer
//!
//! ```rust
//! use gup::integration::wrap_with_custom_render;
//! use gup::{RenderContext, GupResult};
//!
//! #[derive(Debug)]
//! struct CustomData {
//!     values: Vec<f32>,
//! }
//!
//! let data = CustomData { values: vec![1.0, 2.0, 3.0] };
//!
//! let mixable = wrap_with_custom_render(data, |data, context| {
//!     // Custom rendering logic
//!     // Use context to submit GPU commands
//!     Ok(())
//! });
//! ```

use crate::{GupResult, MaybeSend, MaybeSync, Mixable, RenderContext};
use std::fmt::Debug;
use std::marker::PhantomData;

/// Wrapper for external visualization types that makes them compatible with Mixable.
///
/// This type allows you to integrate existing visualization libraries or custom
/// data structures into Gup's composition system without modifying the original types.
///
/// # Type Parameters
///
/// * `T` - The external type being wrapped
///
/// # Examples
///
/// ```rust
/// use gup::integration::{ExternalVisualizationWrapper, ExternalRenderer};
/// use gup::{RenderContext, GupResult};
///
/// #[derive(Debug)]
/// struct MyData {
///     points: Vec<[f32; 2]>,
/// }
///
/// #[derive(Debug)]
/// struct MyRenderer;
///
/// impl ExternalRenderer<MyData> for MyRenderer {
///     fn render(&self, data: &MyData, context: &mut RenderContext) -> GupResult<()> {
///         // Render the points using basic pipeline
///         Ok(())
///     }
///
///     fn is_valid(&self, data: &MyData) -> bool {
///         !data.points.is_empty()
///     }
///
///     fn description(&self, _data: &MyData) -> String {
///         "MyData Visualization".to_string()
///     }
/// }
///
/// let data = MyData { points: vec![[0.0, 0.0], [1.0, 1.0]] };
/// let wrapped = ExternalVisualizationWrapper::new(data, Box::new(MyRenderer));
/// ```
#[derive(Debug)]
pub struct ExternalVisualizationWrapper<T> {
    inner: T,
    renderer: Box<dyn ExternalRenderer<T>>,
}

// Note: Clone is not automatically derivable due to the boxed trait object
// Users needing Clone should use the constructor with a cloned inner value
// and a new instance of the renderer

/// Trait for rendering external visualization types within the Gup framework.
///
/// Implement this trait to define how your external visualization type should be
/// rendered using Gup's GPU-accelerated rendering pipeline.
pub trait ExternalRenderer<T>: MaybeSend + MaybeSync + Debug {
    /// Render the external visualization using the provided render context.
    ///
    /// # Arguments
    ///
    /// * `visualization` - The external visualization data to render
    /// * `context` - The render context containing GPU resources and state
    ///
    /// # Returns
    ///
    /// `Ok(())` on successful rendering, or a `GupError` on failure
    fn render(&self, visualization: &T, context: &mut RenderContext) -> GupResult<()>;

    /// Check if the external visualization is in a valid state for rendering.
    ///
    /// # Arguments
    ///
    /// * `visualization` - The external visualization data to validate
    ///
    /// # Returns
    ///
    /// `true` if the visualization can be rendered, `false` otherwise
    fn is_valid(&self, visualization: &T) -> bool;

    /// Get a human-readable description of the external visualization.
    ///
    /// # Arguments
    ///
    /// * `visualization` - The external visualization data to describe
    ///
    /// # Returns
    ///
    /// A string description of the visualization
    fn description(&self, visualization: &T) -> String;
}

impl<T: MaybeSend + MaybeSync + Debug> ExternalVisualizationWrapper<T> {
    /// Create a new wrapper with the specified external data and renderer.
    ///
    /// # Arguments
    ///
    /// * `inner` - The external data to wrap
    /// * `renderer` - The renderer implementation for this data type
    pub fn new(inner: T, renderer: Box<dyn ExternalRenderer<T>>) -> Self {
        Self { inner, renderer }
    }

    /// Get a reference to the wrapped external data.
    pub fn inner(&self) -> &T {
        &self.inner
    }

    /// Get a mutable reference to the wrapped external data.
    pub fn inner_mut(&mut self) -> &mut T {
        &mut self.inner
    }

    /// Decompose the wrapper into its constituent parts.
    ///
    /// Returns the inner data and the renderer.
    pub fn into_parts(self) -> (T, Box<dyn ExternalRenderer<T>>) {
        (self.inner, self.renderer)
    }
}

impl<T: MaybeSend + MaybeSync + Debug> Mixable for ExternalVisualizationWrapper<T> {
    type Output = ();

    fn render(&mut self, context: &mut RenderContext) -> GupResult<()> {
        self.renderer.render(&self.inner, context)
    }

    fn is_valid(&self) -> bool {
        self.renderer.is_valid(&self.inner)
    }

    fn description(&self) -> String {
        self.renderer.description(&self.inner)
    }
}

/// Builder for creating external visualization wrappers with fluent API.
///
/// This builder provides a convenient way to create `ExternalVisualizationWrapper`
/// instances with different types of renderers.
///
/// # Type Parameters
///
/// * `T` - The external type being wrapped
pub struct ExternalVisualizationBuilder<T> {
    _phantom: PhantomData<T>,
}

impl<T: MaybeSend + MaybeSync + Debug> ExternalVisualizationBuilder<T> {
    /// Create a new builder instance.
    pub fn new() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }

    /// Configure the builder to use a point-based renderer.
    ///
    /// This is suitable for external types that represent point-based visualizations.
    pub fn with_point_renderer(self) -> ExternalPointRendererBuilder<T> {
        ExternalPointRendererBuilder::new()
    }

    /// Configure the builder to use a custom renderer.
    ///
    /// # Arguments
    ///
    /// * `renderer` - The custom renderer implementation
    ///
    /// # Returns
    ///
    /// A closure that takes the external data and returns a wrapped visualization
    pub fn with_custom_renderer<R: ExternalRenderer<T> + 'static>(
        self,
        renderer: R,
    ) -> impl FnOnce(T) -> ExternalVisualizationWrapper<T> {
        move |inner| ExternalVisualizationWrapper::new(inner, Box::new(renderer))
    }
}

impl<T: MaybeSend + MaybeSync + Debug> Default for ExternalVisualizationBuilder<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for point-based external renderers.
///
/// This builder helps create renderers for external types that can be represented
/// as collections of 2D points.
pub struct ExternalPointRendererBuilder<T> {
    _phantom: PhantomData<T>,
}

impl<T: MaybeSend + MaybeSync + Debug> ExternalPointRendererBuilder<T> {
    fn new() -> Self {
        Self {
            _phantom: PhantomData,
        }
    }

    /// Create a wrapper using a point extractor function.
    ///
    /// The extractor function should convert the external data into a vector of 2D points
    /// that can be rendered using Gup's basic point rendering pipeline.
    ///
    /// # Arguments
    ///
    /// * `extractor` - Function that extracts 2D points from the external data
    ///
    /// # Returns
    ///
    /// A closure that creates the wrapped visualization
    pub fn with_point_extractor<F>(
        self,
        extractor: F,
    ) -> impl FnOnce(T) -> ExternalVisualizationWrapper<T>
    where
        F: Fn(&T) -> Vec<[f32; 2]> + MaybeSend + MaybeSync + 'static,
        T: Debug,
    {
        move |inner| {
            let renderer = PointExtractorRenderer::new(extractor);
            ExternalVisualizationWrapper::new(inner, Box::new(renderer))
        }
    }
}

/// Renderer implementation that uses a point extraction function.
struct PointExtractorRenderer<F> {
    extractor: F,
}

impl<F> PointExtractorRenderer<F> {
    fn new(extractor: F) -> Self {
        Self { extractor }
    }
}

impl<F> std::fmt::Debug for PointExtractorRenderer<F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PointExtractorRenderer")
            .field("extractor", &"<function>")
            .finish()
    }
}

impl<T, F> ExternalRenderer<T> for PointExtractorRenderer<F>
where
    F: Fn(&T) -> Vec<[f32; 2]> + MaybeSend + MaybeSync,
    T: Debug,
{
    fn render(&self, visualization: &T, _context: &mut RenderContext) -> GupResult<()> {
        let points = (self.extractor)(visualization);

        if points.is_empty() {
            return Ok(());
        }

        // TODO: Use the basic rendering pipeline to render points
        // This is a simplified implementation - for now we just return Ok
        // In practice, you would use context to create vertex buffers and render
        let _ = points; // Suppress unused variable warning
        Ok(())
    }

    fn is_valid(&self, visualization: &T) -> bool {
        !(self.extractor)(visualization).is_empty()
    }

    fn description(&self, _visualization: &T) -> String {
        "ExternalPointVisualization".to_string()
    }
}

/// Convenience functions for common integration patterns.
///
/// These functions provide quick ways to wrap external data types without
/// using the builder pattern.
///
/// Wrap external data that can be represented as 2D points.
///
/// # Arguments
///
/// * `data` - The external data to wrap
/// * `point_extractor` - Function that extracts 2D points from the data
///
/// # Returns
///
/// A wrapped visualization that implements `Mixable`
///
/// # Examples
///
/// ```rust
/// use gup::integration::wrap_point_data;
///
/// #[derive(Debug)]
/// struct MyChart {
///     coordinates: Vec<(f32, f32)>,
/// }
///
/// let chart = MyChart {
///     coordinates: vec![(0.0, 0.0), (1.0, 1.0), (2.0, 4.0)],
/// };
///
/// let mixable = wrap_point_data(chart, |chart| {
///     chart.coordinates.iter().map(|&(x, y)| [x, y]).collect()
/// });
/// ```
pub fn wrap_point_data<T>(
    data: T,
    point_extractor: impl Fn(&T) -> Vec<[f32; 2]> + MaybeSend + MaybeSync + 'static,
) -> ExternalVisualizationWrapper<T>
where
    T: MaybeSend + MaybeSync + Debug,
{
    ExternalVisualizationBuilder::new()
        .with_point_renderer()
        .with_point_extractor(point_extractor)(data)
}

/// Wrap external data with a custom rendering function.
///
/// # Arguments
///
/// * `data` - The external data to wrap
/// * `render_fn` - Function that defines how to render the data
///
/// # Returns
///
/// A wrapped visualization that implements `Mixable`
///
/// # Examples
///
/// ```rust
/// use gup::integration::wrap_with_custom_render;
/// use gup::{RenderContext, GupResult};
///
/// #[derive(Debug)]
/// struct MyData {
///     values: Vec<f32>,
/// }
///
/// let data = MyData { values: vec![1.0, 2.0, 3.0] };
///
/// let mixable = wrap_with_custom_render(data, |data, context| {
///     // Custom rendering implementation
///     // Use context to submit GPU rendering commands
///     Ok(())
/// });
/// ```
pub fn wrap_with_custom_render<T>(
    data: T,
    render_fn: impl Fn(&T, &mut RenderContext) -> GupResult<()> + MaybeSend + MaybeSync + 'static,
) -> ExternalVisualizationWrapper<T>
where
    T: MaybeSend + MaybeSync + Debug,
{
    struct CustomRenderer<F> {
        render_fn: F,
    }

    impl<F> std::fmt::Debug for CustomRenderer<F> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("CustomRenderer")
                .field("render_fn", &"<function>")
                .finish()
        }
    }

    impl<T, F> ExternalRenderer<T> for CustomRenderer<F>
    where
        F: Fn(&T, &mut RenderContext) -> GupResult<()> + MaybeSend + MaybeSync,
        T: Debug,
    {
        fn render(&self, visualization: &T, context: &mut RenderContext) -> GupResult<()> {
            (self.render_fn)(visualization, context)
        }

        fn is_valid(&self, _visualization: &T) -> bool {
            true
        }

        fn description(&self, _visualization: &T) -> String {
            "CustomExternalVisualization".to_string()
        }
    }

    let renderer = CustomRenderer { render_fn };
    ExternalVisualizationWrapper::new(data, Box::new(renderer))
}

/// Type conversion utilities for common data formats.
pub mod conversion {

    /// Convert a vector of tuples to a vector of 2D arrays.
    ///
    /// This is useful when working with external libraries that use tuple representations
    /// for 2D points but you need them as arrays for Gup's rendering pipeline.
    pub fn tuples_to_points(tuples: &[(f32, f32)]) -> Vec<[f32; 2]> {
        tuples.iter().map(|&(x, y)| [x, y]).collect()
    }

    /// Convert a flat vector of coordinates to a vector of 2D points.
    ///
    /// Assumes the input vector has an even number of elements where consecutive
    /// pairs represent (x, y) coordinates.
    ///
    /// # Arguments
    ///
    /// * `coords` - Flat vector of coordinates [x1, y1, x2, y2, ...]
    ///
    /// # Returns
    ///
    /// Vector of 2D points [[x1, y1], [x2, y2], ...]
    ///
    /// # Panics
    ///
    /// Panics if the input vector has an odd number of elements.
    pub fn flat_coords_to_points(coords: &[f32]) -> Vec<[f32; 2]> {
        assert!(
            coords.len().is_multiple_of(2),
            "Coordinate vector must have an even number of elements"
        );

        coords
            .chunks_exact(2)
            .map(|chunk| [chunk[0], chunk[1]])
            .collect()
    }

    /// Convert separate X and Y vectors to a vector of 2D points.
    ///
    /// # Arguments
    ///
    /// * `x_coords` - Vector of X coordinates
    /// * `y_coords` - Vector of Y coordinates
    ///
    /// # Returns
    ///
    /// Vector of 2D points
    ///
    /// # Panics
    ///
    /// Panics if the X and Y vectors have different lengths.
    pub fn separate_coords_to_points(x_coords: &[f32], y_coords: &[f32]) -> Vec<[f32; 2]> {
        assert_eq!(
            x_coords.len(),
            y_coords.len(),
            "X and Y coordinate vectors must have the same length"
        );

        x_coords
            .iter()
            .zip(y_coords.iter())
            .map(|(&x, &y)| [x, y])
            .collect()
    }
}

/// Adapter types for common external library patterns.
pub mod adapters {

    use super::*;

    /// Adapter for chart-like external types with data and configuration.
    ///
    /// This adapter is useful for external types that follow the common pattern
    /// of having separate data and configuration/styling fields.
    pub struct ChartAdapter<T, D> {
        /// The external chart/visualization type
        pub chart: T,
        /// Configuration for how to extract data from the chart
        pub data_extractor: D,
    }

    impl<T, D> std::fmt::Debug for ChartAdapter<T, D>
    where
        T: std::fmt::Debug,
    {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("ChartAdapter")
                .field("chart", &self.chart)
                .field("data_extractor", &"<function>")
                .finish()
        }
    }

    impl<T, D> ChartAdapter<T, D>
    where
        T: MaybeSend + MaybeSync + Debug,
        D: Fn(&T) -> Vec<[f32; 2]> + MaybeSend + MaybeSync,
    {
        /// Create a new chart adapter.
        ///
        /// # Arguments
        ///
        /// * `chart` - The external chart/visualization
        /// * `data_extractor` - Function to extract point data from the chart
        pub fn new(chart: T, data_extractor: D) -> Self {
            Self {
                chart,
                data_extractor,
            }
        }

        /// Convert this adapter into a Mixable wrapper.
        pub fn into_mixable(self) -> ExternalVisualizationWrapper<Self> {
            wrap_point_data(self, |adapter| (adapter.data_extractor)(&adapter.chart))
        }
    }

    /// Adapter for external types that implement common trait patterns.
    ///
    /// This adapter helps integrate external types that implement traits like
    /// `Iterator`, `IntoIterator`, or custom data access traits.
    #[derive(Debug)]
    pub struct TraitAdapter<T> {
        /// The external type
        pub data: T,
    }

    impl<T> TraitAdapter<T>
    where
        T: MaybeSend + MaybeSync + Debug,
    {
        /// Create a new trait adapter.
        pub fn new(data: T) -> Self {
            Self { data }
        }
    }

    impl<T> TraitAdapter<T>
    where
        T: MaybeSend + MaybeSync + Debug,
        for<'a> &'a T: IntoIterator<Item = [f32; 2]>,
    {
        /// Convert an iterable external type into a Mixable wrapper.
        ///
        /// This method is available when the external type can be iterated over
        /// to produce 2D points.
        pub fn into_mixable_from_iter(self) -> ExternalVisualizationWrapper<Self> {
            wrap_point_data(self, |adapter| adapter.data.into_iter().collect())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct TestExternalData {
        points: Vec<(f32, f32)>,
        name: String,
    }

    impl TestExternalData {
        fn new(name: &str, points: Vec<(f32, f32)>) -> Self {
            Self {
                points,
                name: name.to_string(),
            }
        }
    }

    #[tokio::test]
    async fn test_external_wrapper_basic() {
        let external_data = TestExternalData::new("test", vec![(0.0, 0.0), (1.0, 1.0)]);

        let wrapped = wrap_point_data(external_data, |data| {
            data.points.iter().map(|&(x, y)| [x, y]).collect()
        });

        assert!(wrapped.is_valid());
        assert_eq!(wrapped.description(), "ExternalPointVisualization");
        assert_eq!(wrapped.inner().name, "test");
    }

    #[tokio::test]
    async fn test_external_wrapper_render() {
        let mut context = RenderContext::new().await.unwrap();

        let external_data =
            TestExternalData::new("render_test", vec![(0.0, 0.0), (1.0, 1.0), (2.0, 2.0)]);

        let mut wrapped = wrap_point_data(external_data, |data| {
            data.points.iter().map(|&(x, y)| [x, y]).collect()
        });

        // This should not panic and should return Ok
        let result = wrapped.render(&mut context);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_custom_render_wrapper() {
        let mut context = RenderContext::new().await.unwrap();

        #[derive(Debug)]
        struct CustomData {
            #[allow(dead_code)]
            values: Vec<f32>,
        }

        let data = CustomData {
            values: vec![1.0, 2.0, 3.0],
        };

        let mut wrapped = wrap_with_custom_render(data, |_data, _context| {
            // Custom render implementation - just return Ok for test
            Ok(())
        });

        assert!(wrapped.is_valid());
        let result = wrapped.render(&mut context);
        assert!(result.is_ok());
    }

    #[test]
    fn test_builder_pattern() {
        let external_data = TestExternalData::new("builder_test", vec![(1.0, 2.0)]);

        let wrapped = ExternalVisualizationBuilder::new()
            .with_point_renderer()
            .with_point_extractor(|data: &TestExternalData| {
                data.points.iter().map(|&(x, y)| [x, y]).collect()
            })(external_data);

        assert!(wrapped.is_valid());
        assert_eq!(wrapped.inner().name, "builder_test");
    }

    #[test]
    fn test_empty_data_validation() {
        let empty_data = TestExternalData::new("empty", vec![]);

        let wrapped = wrap_point_data(empty_data, |data| {
            data.points.iter().map(|&(x, y)| [x, y]).collect()
        });

        // Empty data should be considered invalid
        assert!(!wrapped.is_valid());
    }

    #[test]
    fn test_conversion_utilities() {
        use conversion::*;

        // Test tuple conversion
        let tuples = vec![(0.0, 1.0), (2.0, 3.0), (4.0, 5.0)];
        let points = tuples_to_points(&tuples);
        assert_eq!(points, vec![[0.0, 1.0], [2.0, 3.0], [4.0, 5.0]]);

        // Test flat coords conversion
        let flat = vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0];
        let points = flat_coords_to_points(&flat);
        assert_eq!(points, vec![[0.0, 1.0], [2.0, 3.0], [4.0, 5.0]]);

        // Test separate coords conversion
        let x_coords = vec![0.0, 2.0, 4.0];
        let y_coords = vec![1.0, 3.0, 5.0];
        let points = separate_coords_to_points(&x_coords, &y_coords);
        assert_eq!(points, vec![[0.0, 1.0], [2.0, 3.0], [4.0, 5.0]]);
    }

    #[test]
    #[should_panic(expected = "Coordinate vector must have an even number of elements")]
    fn test_flat_coords_odd_length_panic() {
        use conversion::*;
        let odd_coords = vec![0.0, 1.0, 2.0]; // Odd number of elements
        flat_coords_to_points(&odd_coords);
    }

    #[test]
    #[should_panic(expected = "X and Y coordinate vectors must have the same length")]
    fn test_separate_coords_mismatched_length_panic() {
        use conversion::*;
        let x_coords = vec![0.0, 1.0];
        let y_coords = vec![2.0]; // Different length
        separate_coords_to_points(&x_coords, &y_coords);
    }

    #[test]
    fn test_chart_adapter() {
        use adapters::*;

        let external_data = TestExternalData::new("adapter_test", vec![(1.0, 2.0), (3.0, 4.0)]);

        let adapter = ChartAdapter::new(external_data, |data| {
            data.points.iter().map(|&(x, y)| [x, y]).collect()
        });

        let wrapped = adapter.into_mixable();
        assert!(wrapped.is_valid());
        assert_eq!(wrapped.inner().chart.name, "adapter_test");
    }
}

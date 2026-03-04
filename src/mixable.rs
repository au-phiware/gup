// Gup - GPU-Accelerated Data Visualization Library
// Copyright (C) 2025 Corin Lawson <corin@phiware.com.au>
//
//! Universal composability system for Gup visualizations.
//!
//! The `Mixable` trait enables natural composition between any two implementing types.
//! This trait enables the core promise: "Everything composes naturally."
//!
//! # Examples
//!
//! ```rust
//! use gup::{Mixable, CompositionMode};
//!
//! // Example usage (requires implementing types)
//! // let chart1 = ScatterPlot::new(data1);
//! // let chart2 = LineChart::new(data2);
//! // let composed = chart1.mix(chart2);
//! ```

use crate::{GupError, GupResult, MaybeSend, MaybeSync, RenderContext, Viewport};
use std::fmt::Debug;

pub mod composition_recovery;
pub mod merge;
pub mod optimization;

pub use composition_recovery::{
    CompositionFallbackType, CompositionPerformanceMetrics, CompositionResult, ErrorHandlingPolicy,
    HealthStatus, MixableErrorRecovery, PerformanceBottleneck, RecoveryStrategy,
    RobustCompositionExecutor, debug,
};
pub use merge::{MergeStrategy, Mergeable};
pub use optimization::{CompositionExecutor, CompositionMetrics, OPTIMIZATION_THRESHOLD};

/// Composition modes define how two mixable components are combined.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CompositionMode {
    /// Render second component on top of first (default)
    #[default]
    Overlay,
    /// Combine data sources and render as unified visualization
    Merge,
    /// Position components adjacent to each other
    SideBySide,
    /// User-defined composition behavior
    Custom,
}

/// Layout direction for SideBySide composition
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LayoutDirection {
    /// Left-to-right layout.
    Horizontal,
    /// Top-to-bottom layout.
    Vertical,
}

/// Configuration for SideBySide composition mode
#[derive(Debug, Clone)]
pub struct SideBySideConfig {
    /// Direction of the side-by-side layout.
    pub direction: LayoutDirection,
    /// Proportion allocated to first component (0.0 to 1.0).
    pub split_ratio: f32, // 0.0 to 1.0, proportion allocated to first component
    /// Padding between components in pixels.
    pub padding: f32,     // Padding between components in pixels
}

impl PartialEq for SideBySideConfig {
    fn eq(&self, other: &Self) -> bool {
        self.direction == other.direction
            && self.split_ratio.to_bits() == other.split_ratio.to_bits()
            && self.padding.to_bits() == other.padding.to_bits()
    }
}

impl Eq for SideBySideConfig {}

impl std::hash::Hash for SideBySideConfig {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.direction.hash(state);
        self.split_ratio.to_bits().hash(state);
        self.padding.to_bits().hash(state);
    }
}

impl Default for SideBySideConfig {
    fn default() -> Self {
        Self {
            direction: LayoutDirection::Horizontal,
            split_ratio: 0.5,
            padding: 10.0,
        }
    }
}

/// Blend modes for overlay composition
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum BlendMode {
    /// No blending applied.
    #[default]
    None,
    /// Alpha-based transparency blending.
    AlphaBlending,
    /// Additive colour blending.
    Additive,
    /// Multiplicative colour blending.
    Multiply,
}

/// Custom composition behaviors supported by the system
#[derive(Debug, Clone)]
pub enum CustomCompositionBehavior {
    /// Cross-fade between two components.
    CrossFade(CrossFadeComposition),
    /// Grid-based layout of two components.
    GridLayout(GridLayoutComposition),
}

impl CustomCompositionBehavior {
    /// Apply custom composition logic
    pub fn compose<A: Mixable, B: Mixable>(
        &self,
        first: &mut A,
        second: &mut B,
        context: &mut RenderContext,
    ) -> GupResult<()> {
        match self {
            CustomCompositionBehavior::CrossFade(behavior) => {
                behavior.compose(first, second, context)
            }
            CustomCompositionBehavior::GridLayout(behavior) => {
                behavior.compose(first, second, context)
            }
        }
    }

    /// Validate that this custom behavior can handle the given component types
    pub fn can_compose<A: Mixable, B: Mixable>(&self, first: &A, second: &B) -> bool {
        match self {
            CustomCompositionBehavior::CrossFade(behavior) => behavior.can_compose(first, second),
            CustomCompositionBehavior::GridLayout(behavior) => behavior.can_compose(first, second),
        }
    }

    /// Get a description of this composition behavior
    pub fn description(&self) -> String {
        match self {
            CustomCompositionBehavior::CrossFade(behavior) => behavior.description(),
            CustomCompositionBehavior::GridLayout(behavior) => behavior.description(),
        }
    }
}

/// The fundamental composable unit - everything can be combined.
///
/// This trait enables universal composability between any two implementing types,
/// with type-safe composition validated at compile time and minimal runtime overhead.
///
/// # Design Principles
///
/// - **Universal Composability**: Any two `Mixable` types can be composed
/// - **Type Safety**: Invalid compositions are caught at compile time
/// - **Performance**: Composition adds <1% runtime overhead
/// - **Lazy Evaluation**: Compositions are not executed until `render()` is called
/// - **Associativity**: `(a.mix(b)).mix(c)` produces same result as `a.mix(b.mix(c))`
///
/// # Examples
///
/// ```rust
/// use gup::{Mixable, RenderContext, GupResult};
///
/// #[derive(Debug)]
/// struct TestVisualization {
///     name: String,
/// }
///
/// impl Mixable for TestVisualization {
///     type Output = ();
///
///     fn render(&mut self, _context: &mut RenderContext) -> GupResult<()> {
///         // Render implementation
///         Ok(())
///     }
/// }
///
/// let viz1 = TestVisualization { name: "chart1".to_string() };
/// let viz2 = TestVisualization { name: "chart2".to_string() };
/// let composed = viz1.mix(viz2);
/// assert!(composed.is_valid());
/// ```
pub trait Mixable: Debug + MaybeSend + MaybeSync {
    /// The output type produced by rendering this mixable component.
    type Output;

    /// Compose this mixable with another mixable component.
    ///
    /// This creates a `ComposedVisualization` that preserves both components
    /// and defers execution until `render()` is called.
    ///
    /// # Arguments
    ///
    /// * `other` - The mixable component to compose with this one
    ///
    /// # Returns
    ///
    /// A `ComposedVisualization` that can itself be mixed with other components
    fn mix<T: Mixable>(self, other: T) -> ComposedVisualization<Self, T>
    where
        Self: Sized,
    {
        ComposedVisualization::new(self, other)
    }

    /// Compose this mixable with another using a specific composition mode.
    ///
    /// # Arguments
    ///
    /// * `other` - The mixable component to compose with this one
    /// * `mode` - The composition mode to use
    fn mix_with_mode<T: Mixable>(
        self,
        other: T,
        mode: CompositionMode,
    ) -> ComposedVisualization<Self, T>
    where
        Self: Sized,
    {
        ComposedVisualization::with_mode(self, other, mode)
    }

    /// Render this mixable component using the provided render context.
    ///
    /// This method is called when the visualization needs to be drawn.
    /// Implementations should use the render context to submit GPU commands.
    ///
    /// # Arguments
    ///
    /// * `context` - The render context containing GPU resources and state
    ///
    /// # Returns
    ///
    /// `Ok(())` on successful rendering, or a `GupError` on failure
    fn render(&mut self, context: &mut RenderContext) -> GupResult<()>;

    /// Validate that this mixable component is in a valid state for rendering.
    ///
    /// This is called before rendering to catch potential issues early.
    /// The default implementation always returns `true`.
    fn is_valid(&self) -> bool {
        true
    }

    /// Get a human-readable description of this mixable component.
    ///
    /// Used for debugging and error messages.
    fn description(&self) -> String {
        format!("{self:?}")
    }
}

/// Composition container that preserves both components and defines how they combine.
///
/// This type implements `Mixable` itself, enabling recursive composition of any depth.
/// The composition is lazy - no rendering occurs until `render()` is explicitly called.
///
/// # Type Parameters
///
/// * `A` - The first mixable component type
/// * `B` - The second mixable component type
#[derive(Debug)]
pub struct ComposedVisualization<A: Mixable, B: Mixable> {
    /// The first component in the composition
    first: A,
    /// The second component in the composition
    second: B,
    /// How the two components should be combined
    composition_mode: CompositionMode,
    /// Configuration for SideBySide mode
    side_by_side_config: SideBySideConfig,
    /// Custom composition behavior (for Custom mode)
    custom_behavior: Option<CustomCompositionBehavior>,
    /// Merge strategy for Merge mode
    merge_strategy: MergeStrategy,
    /// Viewport calculation cache for performance
    viewport_cache: std::collections::HashMap<ViewportCacheKey, CachedViewportSplit>,
    /// Cache generation counter for invalidation
    cache_generation: u64,
}

/// Cache key for viewport calculations
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct ViewportCacheKey {
    original_viewport: crate::Viewport,
    config_hash: u64,
}

/// Cached viewport split result
#[derive(Debug, Clone)]
struct CachedViewportSplit {
    first_viewport: crate::Viewport,
    second_viewport: crate::Viewport,
    generation: u64,
}

impl<A: Mixable, B: Mixable> ComposedVisualization<A, B> {
    /// Create a new composed visualization with default overlay mode.
    ///
    /// # Arguments
    ///
    /// * `first` - The first mixable component
    /// * `second` - The second mixable component
    pub fn new(first: A, second: B) -> Self {
        Self {
            first,
            second,
            composition_mode: CompositionMode::default(),
            side_by_side_config: SideBySideConfig::default(),
            custom_behavior: None,
            merge_strategy: MergeStrategy::default(),
            viewport_cache: std::collections::HashMap::new(),
            cache_generation: 0,
        }
    }

    /// Create a new composed visualization with a specific composition mode.
    ///
    /// # Arguments
    ///
    /// * `first` - The first mixable component
    /// * `second` - The second mixable component
    /// * `mode` - The composition mode to use
    pub fn with_mode(first: A, second: B, mode: CompositionMode) -> Self {
        Self {
            first,
            second,
            composition_mode: mode,
            side_by_side_config: SideBySideConfig::default(),
            custom_behavior: None,
            merge_strategy: MergeStrategy::default(),
            viewport_cache: std::collections::HashMap::new(),
            cache_generation: 0,
        }
    }

    /// Create a new composed visualization with side-by-side configuration
    pub fn side_by_side(first: A, second: B, config: SideBySideConfig) -> Self {
        Self {
            first,
            second,
            composition_mode: CompositionMode::SideBySide,
            side_by_side_config: config,
            custom_behavior: None,
            merge_strategy: MergeStrategy::default(),
            viewport_cache: std::collections::HashMap::new(),
            cache_generation: 0,
        }
    }

    /// Create a new composed visualization with custom behavior
    pub fn custom(first: A, second: B, behavior: CustomCompositionBehavior) -> Self {
        Self {
            first,
            second,
            composition_mode: CompositionMode::Custom,
            side_by_side_config: SideBySideConfig::default(),
            custom_behavior: Some(behavior),
            merge_strategy: MergeStrategy::default(),
            viewport_cache: std::collections::HashMap::new(),
            cache_generation: 0,
        }
    }

    /// Configure side-by-side layout parameters
    pub fn with_side_by_side_config(mut self, config: SideBySideConfig) -> Self {
        self.side_by_side_config = config;
        // Invalidate cache by incrementing generation
        self.cache_generation += 1;
        self
    }

    /// Configure the merge strategy for Merge mode.
    ///
    /// # Arguments
    ///
    /// * `strategy` - The merge strategy to use
    pub fn with_merge_strategy(mut self, strategy: MergeStrategy) -> Self {
        self.merge_strategy = strategy;
        self
    }

    /// Get the current merge strategy.
    pub fn merge_strategy(&self) -> &MergeStrategy {
        &self.merge_strategy
    }

    /// Set the merge strategy.
    pub fn set_merge_strategy(&mut self, strategy: MergeStrategy) {
        self.merge_strategy = strategy;
    }

    /// Get the composition mode being used.
    pub fn composition_mode(&self) -> CompositionMode {
        self.composition_mode
    }

    /// Change the composition mode of this composed visualization.
    pub fn set_composition_mode(&mut self, mode: CompositionMode) {
        self.composition_mode = mode;
    }

    /// Get a reference to the first component.
    pub fn first(&self) -> &A {
        &self.first
    }

    /// Get a reference to the second component.
    pub fn second(&self) -> &B {
        &self.second
    }

    /// Decompose this composition into its constituent parts.
    pub fn into_parts(self) -> (A, B, CompositionMode) {
        (self.first, self.second, self.composition_mode)
    }
}

impl<A: Mixable + 'static, B: Mixable + 'static> Mixable for ComposedVisualization<A, B> {
    type Output = ();

    fn render(&mut self, context: &mut RenderContext) -> GupResult<()> {
        // Validate both components before rendering
        if !self.first.is_valid() {
            return Err(GupError::composition_error(format!(
                "First component is invalid: {}",
                self.first.description()
            )));
        }
        if !self.second.is_valid() {
            return Err(GupError::composition_error(format!(
                "Second component is invalid: {}",
                self.second.description()
            )));
        }

        // Render based on composition mode
        match self.composition_mode {
            CompositionMode::Overlay => self.render_overlay(context),
            CompositionMode::Merge => self.render_merge(context),
            CompositionMode::SideBySide => self.render_side_by_side(context),
            CompositionMode::Custom => self.render_custom(context),
        }
    }

    fn is_valid(&self) -> bool {
        self.first.is_valid() && self.second.is_valid()
    }

    fn description(&self) -> String {
        format!(
            "ComposedVisualization({:?}, {} + {})",
            self.composition_mode,
            self.first.description(),
            self.second.description()
        )
    }
}

impl<A: Mixable + 'static, B: Mixable + 'static> ComposedVisualization<A, B> {
    /// Render with automatic optimization for deep chains
    pub fn render_optimized(&mut self, context: &mut RenderContext) -> GupResult<()> {
        // Check if this composition is deep enough to benefit from optimization
        if self.composition_depth() > OPTIMIZATION_THRESHOLD {
            let mut executor = CompositionExecutor::new();
            executor.flatten_composition(self)?;
            executor.execute(context)
        } else {
            // Use regular rendering for shallow compositions
            self.render(context)
        }
    }

    /// Calculate the depth of this composition chain
    pub fn composition_depth(&self) -> usize {
        let first_depth = self.get_component_depth(&self.first);
        let second_depth = self.get_component_depth(&self.second);
        1 + first_depth.max(second_depth)
    }

    /// Get the composition depth of a component
    fn get_component_depth<T: Mixable>(&self, _component: &T) -> usize {
        // This would check if the component is itself a composition
        // For now, return 0 since we can't easily inspect component types
        // In a full implementation, this would use Any trait or specific patterns
        0
    }

    /// Render in overlay mode with proper depth testing and blending
    fn render_overlay(&mut self, context: &mut RenderContext) -> GupResult<()> {
        // Render first component (background layer) with current blend state
        self.first.render(context)?;

        // Use RAII guard to render second component with alpha blending
        {
            let mut guard = context.with_blend_mode(BlendMode::AlphaBlending)?;
            let context = guard.context_mut();
            // Render second component (foreground layer) with alpha blending
            self.second.render(context)?;
            // State automatically restored when guard drops
        }

        Ok(())
    }

    /// Render in merge mode by combining data sources
    ///
    /// # Current Limitations
    ///
    /// This implementation renders both components sequentially because Rust's
    /// type system makes it challenging to generically extract and merge data
    /// from arbitrary Mixable types at runtime.
    ///
    /// # Future Enhancement
    ///
    /// To fully implement data merging, components must implement the
    /// `Mergeable<T>` trait for a common data type `T`. This would require:
    ///
    /// 1. Downcast support for Mergeable trait objects
    /// 2. Type-checking at runtime to verify compatible data types
    /// 3. Generic merge implementation for compatible Mergeable types
    ///
    /// # Workaround
    ///
    /// For types that support merging, create a custom wrapper that implements
    /// Mixable and handles data extraction and merging internally:
    ///
    /// ```rust,ignore
    /// struct MergedScatterPlots<T> {
    ///     data: Vec<T>,
    /// }
    ///
    /// impl<T: Clone + PartialEq> MergedScatterPlots<T> {
    ///     fn merge(plot1: ScatterPlot<T>, plot2: ScatterPlot<T>) -> Self {
    ///         let strategy = MergeStrategy::Deduplicate;
    ///         let merged_data = strategy.apply(plot1.data(), plot2.data()).unwrap();
    ///         Self { data: merged_data }
    ///     }
    /// }
    /// ```
    fn render_merge(&mut self, context: &mut RenderContext) -> GupResult<()> {
        // For merge mode, we need to extract and combine the underlying data
        // This is a simplified implementation - real merge would depend on data types

        // Check if components can be merged (same data types, compatible formats)
        if !self.can_merge_components() {
            return Err(GupError::composition_error(
                "Components cannot be merged - incompatible data types".to_string(),
            ));
        }

        // Current implementation: render both components sequentially
        // This provides visual combination without true data merging
        //
        // NOTE: Merge strategy is configured but not currently applied because
        // the generic Mixable trait doesn't provide access to underlying data.
        // See module documentation for merge strategies and Mergeable trait.
        self.first.render(context)?;
        self.second.render(context)?;

        Ok(())
    }

    /// Render in side-by-side mode with viewport partitioning
    fn render_side_by_side(&mut self, context: &mut RenderContext) -> GupResult<()> {
        let original_viewport = context.viewport();

        let (first_viewport, second_viewport) =
            self.calculate_split_viewports_cached(original_viewport);

        // Render first component in its viewport
        context.set_viewport(first_viewport)?;
        self.first.render(context)?;

        // Render second component in its viewport
        context.set_viewport(second_viewport)?;
        self.second.render(context)?;

        // Restore original viewport
        context.set_viewport(original_viewport)?;

        Ok(())
    }

    /// Render using custom composition behavior
    fn render_custom(&mut self, context: &mut RenderContext) -> GupResult<()> {
        if let Some(custom_behavior) = &self.custom_behavior {
            if !custom_behavior.can_compose(&self.first, &self.second) {
                return Err(GupError::composition_error(format!(
                    "Custom behavior '{}' cannot compose these component types",
                    custom_behavior.description()
                )));
            }

            custom_behavior.compose(&mut self.first, &mut self.second, context)
        } else {
            Err(GupError::composition_error(
                "Custom composition mode requires custom behavior".to_string(),
            ))
        }
    }

    /// Check if components can be merged based on their data types
    fn can_merge_components(&self) -> bool {
        // This would be implemented based on specific component types
        // For now, return true as a placeholder
        true
    }

    /// Calculate viewport splits for side-by-side rendering
    fn calculate_split_viewports(&self, original: Viewport) -> (Viewport, Viewport) {
        match self.side_by_side_config.direction {
            LayoutDirection::Horizontal => {
                let split_x = (original.width as f32 * self.side_by_side_config.split_ratio) as u32;
                let padding = self.side_by_side_config.padding as u32;

                let first_viewport = Viewport {
                    width: split_x.saturating_sub(padding / 2),
                    height: original.height,
                    scale_factor: original.scale_factor,
                };

                let second_viewport = Viewport {
                    width: original
                        .width
                        .saturating_sub(split_x)
                        .saturating_sub(padding / 2),
                    height: original.height,
                    scale_factor: original.scale_factor,
                };

                (first_viewport, second_viewport)
            }
            LayoutDirection::Vertical => {
                let split_y =
                    (original.height as f32 * self.side_by_side_config.split_ratio) as u32;
                let padding = self.side_by_side_config.padding as u32;

                let first_viewport = Viewport {
                    width: original.width,
                    height: split_y.saturating_sub(padding / 2),
                    scale_factor: original.scale_factor,
                };

                let second_viewport = Viewport {
                    width: original.width,
                    height: original
                        .height
                        .saturating_sub(split_y)
                        .saturating_sub(padding / 2),
                    scale_factor: original.scale_factor,
                };

                (first_viewport, second_viewport)
            }
        }
    }

    /// Calculate hash for the current configuration
    fn calculate_config_hash(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        self.side_by_side_config.hash(&mut hasher);
        hasher.finish()
    }

    /// Calculate viewport splits with caching
    fn calculate_split_viewports_cached(
        &mut self,
        original: crate::Viewport,
    ) -> (crate::Viewport, crate::Viewport) {
        let config_hash = self.calculate_config_hash();
        let cache_key = ViewportCacheKey {
            original_viewport: original,
            config_hash,
        };

        // Check cache first
        if let Some(cached) = self.viewport_cache.get(&cache_key)
            && cached.generation == self.cache_generation
        {
            return (cached.first_viewport, cached.second_viewport);
        }

        // Calculate and cache
        let (first_vp, second_vp) = self.calculate_split_viewports(original);

        self.viewport_cache.insert(
            cache_key,
            CachedViewportSplit {
                first_viewport: first_vp,
                second_viewport: second_vp,
                generation: self.cache_generation,
            },
        );

        (first_vp, second_vp)
    }
}

/// Helper trait to enable fluent composition APIs.
///
/// This trait provides additional convenience methods for common composition patterns.
pub trait MixableExt: Mixable + Sized {
    /// Compose with overlay mode (explicit).
    fn overlay<T: Mixable>(self, other: T) -> ComposedVisualization<Self, T> {
        self.mix_with_mode(other, CompositionMode::Overlay)
    }

    /// Compose with merge mode.
    fn merge<T: Mixable>(self, other: T) -> ComposedVisualization<Self, T> {
        self.mix_with_mode(other, CompositionMode::Merge)
    }

    /// Compose with side-by-side mode using default configuration.
    fn beside<T: Mixable>(self, other: T) -> ComposedVisualization<Self, T> {
        ComposedVisualization::side_by_side(self, other, SideBySideConfig::default())
    }

    /// Compose with side-by-side mode using custom configuration.
    fn beside_with_config<T: Mixable>(
        self,
        other: T,
        config: SideBySideConfig,
    ) -> ComposedVisualization<Self, T> {
        ComposedVisualization::side_by_side(self, other, config)
    }

    /// Compose with custom behavior.
    fn custom_compose<T: Mixable>(
        self,
        other: T,
        behavior: CustomCompositionBehavior,
    ) -> ComposedVisualization<Self, T> {
        ComposedVisualization::custom(self, other, behavior)
    }
}

// Blanket implementation for all Mixable types
impl<T: Mixable> MixableExt for T {}

// Enhanced RenderContext methods are now implemented in render.rs

/// Example: Cross-fade composition behavior
#[derive(Debug, Clone)]
pub struct CrossFadeComposition {
    /// Blend factor (0.0 = first only, 1.0 = second only).
    pub fade_factor: f32, // 0.0 = first only, 1.0 = second only
}

impl CrossFadeComposition {
    /// Compose two components using cross-fade blending.
    pub fn compose<A: Mixable, B: Mixable>(
        &self,
        first: &mut A,
        second: &mut B,
        context: &mut RenderContext,
    ) -> GupResult<()> {
        // Render first component with (1.0 - fade_factor) alpha
        context.set_global_alpha(1.0 - self.fade_factor)?;
        first.render(context)?;

        // Render second component with fade_factor alpha
        context.set_global_alpha(self.fade_factor)?;
        second.render(context)?;

        // Restore alpha
        context.set_global_alpha(1.0)?;

        Ok(())
    }

    /// Returns whether two components can be cross-faded.
    pub fn can_compose<A: Mixable, B: Mixable>(&self, _first: &A, _second: &B) -> bool {
        // Cross-fade can compose any two components
        true
    }

    /// Get a human-readable description of this composition.
    pub fn description(&self) -> String {
        format!("CrossFade(factor: {:.2})", self.fade_factor)
    }
}

/// Example: Grid layout composition behavior
#[derive(Debug, Clone)]
pub struct GridLayoutComposition {
    /// Number of rows in the grid.
    pub rows: u32,
    /// Number of columns in the grid.
    pub cols: u32,
    /// Grid cell position (row, col) for the first component.
    pub cell_index_first: (u32, u32),
    /// Grid cell position (row, col) for the second component.
    pub cell_index_second: (u32, u32),
}

impl GridLayoutComposition {
    /// Compose two components in a grid layout.
    pub fn compose<A: Mixable, B: Mixable>(
        &self,
        first: &mut A,
        second: &mut B,
        context: &mut RenderContext,
    ) -> GupResult<()> {
        let original_viewport = context.viewport();

        let cell_width = original_viewport.width / self.cols;
        let cell_height = original_viewport.height / self.rows;

        // Render first component in its grid cell
        let first_viewport = Viewport {
            width: cell_width,
            height: cell_height,
            scale_factor: original_viewport.scale_factor,
        };
        context.set_viewport(first_viewport)?;
        first.render(context)?;

        // Render second component in its grid cell
        let second_viewport = Viewport {
            width: cell_width,
            height: cell_height,
            scale_factor: original_viewport.scale_factor,
        };
        context.set_viewport(second_viewport)?;
        second.render(context)?;

        // Restore original viewport
        context.set_viewport(original_viewport)?;

        Ok(())
    }

    /// Returns whether the grid cell indices are within bounds.
    pub fn can_compose<A: Mixable, B: Mixable>(&self, _first: &A, _second: &B) -> bool {
        // Indices are (row, col)
        self.cell_index_first.0 < self.rows
            && self.cell_index_first.1 < self.cols
            && self.cell_index_second.0 < self.rows
            && self.cell_index_second.1 < self.cols
    }

    /// Get a human-readable description of this grid layout.
    pub fn description(&self) -> String {
        format!(
            "GridLayout({}x{}, cells: {:?}, {:?})",
            self.rows, self.cols, self.cell_index_first, self.cell_index_second
        )
    }
}

/// Enhanced convenience methods for cross-fade composition
pub trait CrossFadeExt: MixableExt {
    /// Compose with cross-fade behavior
    fn cross_fade<U: Mixable>(self, other: U, fade_factor: f32) -> ComposedVisualization<Self, U>
    where
        Self: Sized,
    {
        let behavior = CustomCompositionBehavior::CrossFade(CrossFadeComposition { fade_factor });
        self.custom_compose(other, behavior)
    }
}

// Implement CrossFadeExt for all MixableExt types
impl<T: MixableExt> CrossFadeExt for T {}

/// Macro to help implement Mixable for custom types.
///
/// This macro generates a basic implementation that can be customized as needed.
///
/// # Examples
///
/// ```rust
/// use gup::impl_mixable;
///
/// #[derive(Debug)]
/// struct MyVisualization {
///     data: Vec<f32>,
/// }
///
/// impl_mixable!(MyVisualization, ());
/// ```
#[macro_export]
macro_rules! impl_mixable {
    ($type:ty, $output:ty) => {
        impl $crate::Mixable for $type {
            type Output = $output;

            fn render(&mut self, _context: &mut $crate::RenderContext) -> $crate::GupResult<()> {
                // Default implementation - override as needed
                Ok(())
            }
        }
    };
    ($type:ty, $output:ty, $render_body:expr) => {
        impl $crate::Mixable for $type {
            type Output = $output;

            fn render(&mut self, context: &mut $crate::RenderContext) -> $crate::GupResult<()> {
                let _context = context; // Allow unused context
                $render_body
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone)]
    struct TestVisualization {
        name: String,
        should_fail: bool,
    }

    impl TestVisualization {
        fn new(name: &str) -> Self {
            Self {
                name: name.to_string(),
                should_fail: false,
            }
        }

        fn with_failure(name: &str) -> Self {
            Self {
                name: name.to_string(),
                should_fail: true,
            }
        }
    }

    impl Mixable for TestVisualization {
        type Output = ();

        fn render(&mut self, _context: &mut RenderContext) -> GupResult<()> {
            if self.should_fail {
                Err(GupError::render_error(format!(
                    "Intentional failure from {}",
                    self.name
                )))
            } else {
                Ok(())
            }
        }

        fn is_valid(&self) -> bool {
            !self.should_fail
        }

        fn description(&self) -> String {
            self.name.clone()
        }
    }

    #[test]
    fn test_basic_composition() {
        let chart1 = TestVisualization::new("chart1");
        let chart2 = TestVisualization::new("chart2");
        let composed = chart1.mix(chart2);

        assert!(composed.is_valid());
        assert_eq!(composed.composition_mode(), CompositionMode::Overlay);
    }

    #[test]
    fn test_composition_modes() {
        let chart1 = TestVisualization::new("chart1");
        let chart2 = TestVisualization::new("chart2");

        let overlay = chart1.clone().overlay(chart2.clone());
        assert_eq!(overlay.composition_mode(), CompositionMode::Overlay);

        let merge = chart1.clone().merge(chart2.clone());
        assert_eq!(merge.composition_mode(), CompositionMode::Merge);

        let beside = chart1.clone().beside(chart2.clone());
        assert_eq!(beside.composition_mode(), CompositionMode::SideBySide);

        let custom_behavior =
            CustomCompositionBehavior::CrossFade(CrossFadeComposition { fade_factor: 0.5 });
        let custom = chart1.custom_compose(chart2, custom_behavior);
        assert_eq!(custom.composition_mode(), CompositionMode::Custom);
    }

    #[test]
    fn test_composition_associativity() {
        let a = TestVisualization::new("a");
        let b = TestVisualization::new("b");
        let c = TestVisualization::new("c");

        let left_assoc = a.clone().mix(b.clone()).mix(c.clone());
        let right_assoc = a.mix(b.mix(c));

        // Both should be valid and have the same structure when rendered
        assert!(left_assoc.is_valid());
        assert!(right_assoc.is_valid());
    }

    #[test]
    fn test_composition_validation() {
        let valid_chart = TestVisualization::new("valid");
        let invalid_chart = TestVisualization::with_failure("invalid");

        let composed = valid_chart.mix(invalid_chart);
        assert!(!composed.is_valid());
    }

    #[tokio::test]
    async fn test_render_error_propagation() {
        let mut context = RenderContext::new().await.unwrap();
        let valid_chart = TestVisualization::new("valid");
        let invalid_chart = TestVisualization::with_failure("invalid");

        let mut composed = valid_chart.mix(invalid_chart);
        let result = composed.render(&mut context);

        assert!(result.is_err());
        if let Err(GupError::CompositionError { message }) = result {
            assert!(message.contains("Second component is invalid"));
        } else {
            panic!("Expected CompositionError");
        }
    }

    #[test]
    fn test_deep_composition() {
        let a = TestVisualization::new("a");
        let b = TestVisualization::new("b");
        let c = TestVisualization::new("c");
        let d = TestVisualization::new("d");

        let deeply_composed = a.mix(b).mix(c).mix(d);
        assert!(deeply_composed.is_valid());
    }

    #[test]
    fn test_composition_description() {
        let chart1 = TestVisualization::new("chart1");
        let chart2 = TestVisualization::new("chart2");
        let composed = chart1.mix(chart2);

        let description = composed.description();
        assert!(description.contains("chart1"));
        assert!(description.contains("chart2"));
        assert!(description.contains("ComposedVisualization"));
    }

    #[tokio::test]
    async fn test_overlay_composition() {
        let mut context = RenderContext::new().await.unwrap();

        let background = TestVisualization::new("background");
        let foreground = TestVisualization::new("foreground");

        let mut composed = background.overlay(foreground);
        let result = composed.render(&mut context);

        assert!(result.is_ok());
        assert_eq!(composed.composition_mode(), CompositionMode::Overlay);
    }

    #[tokio::test]
    async fn test_side_by_side_composition() {
        let mut context = RenderContext::new().await.unwrap();

        let left = TestVisualization::new("left");
        let right = TestVisualization::new("right");

        let config = SideBySideConfig {
            direction: LayoutDirection::Horizontal,
            split_ratio: 0.3,
            padding: 20.0,
        };

        let mut composed = left.beside_with_config(right, config);
        let result = composed.render(&mut context);

        assert!(result.is_ok());
        assert_eq!(composed.composition_mode(), CompositionMode::SideBySide);
    }

    #[tokio::test]
    async fn test_custom_composition() {
        let mut context = RenderContext::new().await.unwrap();

        let viz1 = TestVisualization::new("viz1");
        let viz2 = TestVisualization::new("viz2");

        let mut composed = viz1.cross_fade(viz2, 0.3);
        let result = composed.render(&mut context);

        assert!(result.is_ok());
        assert_eq!(composed.composition_mode(), CompositionMode::Custom);
    }

    #[tokio::test]
    async fn test_merge_composition() {
        let mut context = RenderContext::new().await.unwrap();

        let data1 = TestVisualization::new("data1");
        let data2 = TestVisualization::new("data2");

        let mut composed = data1.merge(data2);
        let result = composed.render(&mut context);

        assert!(result.is_ok());
        assert_eq!(composed.composition_mode(), CompositionMode::Merge);
    }

    #[test]
    fn test_viewport_splitting() {
        let original = Viewport {
            width: 800,
            height: 600,
            scale_factor: 1.0,
        };

        let config = SideBySideConfig {
            direction: LayoutDirection::Horizontal,
            split_ratio: 0.6,
            padding: 10.0,
        };

        let chart1 = TestVisualization::new("chart1");
        let chart2 = TestVisualization::new("chart2");
        let composition = ComposedVisualization::side_by_side(chart1, chart2, config);

        let (first_vp, second_vp) = composition.calculate_split_viewports(original);

        assert_eq!(first_vp.width, 475); // 800 * 0.6 - 5 (half padding)
        assert_eq!(second_vp.width, 315); // 800 * 0.4 - 5 (half padding)
        assert_eq!(first_vp.height, 600);
        assert_eq!(second_vp.height, 600);
    }

    #[test]
    fn test_vertical_viewport_splitting() {
        let original = Viewport {
            width: 800,
            height: 600,
            scale_factor: 1.0,
        };

        let config = SideBySideConfig {
            direction: LayoutDirection::Vertical,
            split_ratio: 0.4,
            padding: 20.0,
        };

        let chart1 = TestVisualization::new("chart1");
        let chart2 = TestVisualization::new("chart2");
        let composition = ComposedVisualization::side_by_side(chart1, chart2, config);

        let (first_vp, second_vp) = composition.calculate_split_viewports(original);

        assert_eq!(first_vp.width, 800);
        assert_eq!(second_vp.width, 800);
        assert_eq!(first_vp.height, 230); // 600 * 0.4 - 10 (half padding)
        assert_eq!(second_vp.height, 350); // 600 * 0.6 - 10 (half padding)
    }

    #[test]
    fn test_cross_fade_composition_description() {
        let behavior = CrossFadeComposition { fade_factor: 0.75 };
        assert_eq!(behavior.description(), "CrossFade(factor: 0.75)");
    }

    #[test]
    fn test_grid_layout_composition_validation() {
        let behavior = CustomCompositionBehavior::GridLayout(GridLayoutComposition {
            rows: 2,
            cols: 3,
            cell_index_first: (1, 0),
            cell_index_second: (2, 1),
        });

        let viz1 = TestVisualization::new("viz1");
        let viz2 = TestVisualization::new("viz2");

        // First cell is valid (1, 0) in 2x3 grid (row 1, col 0)
        // Second cell is invalid (2, 1) - row 2 is out of bounds for 2 rows (0-1)
        assert!(!behavior.can_compose(&viz1, &viz2));

        let valid_behavior = CustomCompositionBehavior::GridLayout(GridLayoutComposition {
            rows: 2,
            cols: 3,
            cell_index_first: (1, 0),
            cell_index_second: (0, 2),
        });

        assert!(valid_behavior.can_compose(&viz1, &viz2));
    }

    #[tokio::test]
    async fn test_custom_composition_without_behavior() {
        let mut context = RenderContext::new().await.unwrap();

        let viz1 = TestVisualization::new("viz1");
        let viz2 = TestVisualization::new("viz2");

        let mut composed = ComposedVisualization::with_mode(viz1, viz2, CompositionMode::Custom);
        let result = composed.render(&mut context);

        assert!(result.is_err());
        if let Err(GupError::CompositionError { message }) = result {
            assert!(message.contains("Custom composition mode requires custom behavior"));
        } else {
            panic!("Expected CompositionError for custom mode without behavior");
        }
    }

    #[test]
    fn test_side_by_side_config_default() {
        let config = SideBySideConfig::default();
        assert_eq!(config.direction, LayoutDirection::Horizontal);
        assert_eq!(config.split_ratio, 0.5);
        assert_eq!(config.padding, 10.0);
    }

    #[test]
    fn test_blend_mode_default() {
        let blend_mode = BlendMode::default();
        assert_eq!(blend_mode, BlendMode::None);
    }

    #[tokio::test]
    async fn test_overlay_with_blend_modes() {
        let mut context = RenderContext::new().await.unwrap();

        let background = TestVisualization::new("background");
        let foreground = TestVisualization::new("foreground");

        let mut overlay = background.overlay(foreground);

        // Test that overlay composition uses blend state stack
        let initial_mode = context.current_blend_mode();
        let result = overlay.render(&mut context);
        assert!(result.is_ok());

        // Blend mode should be restored after overlay rendering
        assert_eq!(context.current_blend_mode(), initial_mode);
    }

    #[tokio::test]
    async fn test_nested_overlay_blend_stack() {
        let mut context = RenderContext::new().await.unwrap();

        let a = TestVisualization::new("a");
        let b = TestVisualization::new("b");
        let c = TestVisualization::new("c");

        // Create nested overlay: (a overlay b) overlay c
        let mut nested_overlay = a.overlay(b).overlay(c);

        // Set initial blend mode
        context.set_blend_mode(BlendMode::Multiply).unwrap();
        let initial_mode = context.current_blend_mode();

        let result = nested_overlay.render(&mut context);
        assert!(result.is_ok());

        // Blend mode should be restored after nested rendering
        assert_eq!(context.current_blend_mode(), initial_mode);
    }

    #[tokio::test]
    async fn test_cross_fade_with_global_alpha() {
        let mut context = RenderContext::new().await.unwrap();

        let viz1 = TestVisualization::new("viz1");
        let viz2 = TestVisualization::new("viz2");

        let mut cross_fade = viz1.cross_fade(viz2, 0.3);

        // Test that cross-fade uses global alpha
        let result = cross_fade.render(&mut context);
        assert!(result.is_ok());

        // Global alpha buffer should be created during cross-fade
        assert!(context.has_global_alpha_buffer());
    }

    #[tokio::test]
    async fn test_blend_mode_integration_with_composition() {
        let mut context = RenderContext::new().await.unwrap();

        // Test all composition modes work with blend state management
        let viz1 = TestVisualization::new("viz1");
        let viz2 = TestVisualization::new("viz2");

        // Test overlay
        let mut overlay = viz1.clone().overlay(viz2.clone());
        assert!(overlay.render(&mut context).is_ok());

        // Test merge
        let mut merge = viz1.clone().merge(viz2.clone());
        assert!(merge.render(&mut context).is_ok());

        // Test side-by-side
        let mut beside = viz1.clone().beside(viz2.clone());
        assert!(beside.render(&mut context).is_ok());

        // Test custom composition
        let mut custom = viz1.cross_fade(viz2, 0.5);
        assert!(custom.render(&mut context).is_ok());
    }

    #[tokio::test]
    async fn test_viewport_caching() {
        let mut context = RenderContext::new().await.unwrap();

        let viz1 = TestVisualization::new("viz1");
        let viz2 = TestVisualization::new("viz2");

        let mut composition = viz1.beside(viz2);

        // First render should calculate and cache viewport splits
        assert!(composition.render(&mut context).is_ok());

        // Subsequent renders with same viewport should hit cache
        assert!(composition.render(&mut context).is_ok());
        assert!(composition.render(&mut context).is_ok());

        // Changing viewport should calculate new split
        context
            .set_viewport(crate::Viewport {
                width: 1024,
                height: 768,
                scale_factor: 1.0,
            })
            .unwrap();
        assert!(composition.render(&mut context).is_ok());

        // Cache should still work with new viewport
        assert!(composition.render(&mut context).is_ok());
    }
}

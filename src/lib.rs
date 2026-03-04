// Gup - GPU-Accelerated Data Visualization Library
// Copyright (C) 2025 Corin Lawson <corin@phiware.com.au>
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.
//
//! # Gup - GPU-Accelerated Data Visualization Library
//!
//! Gup is a high-performance data visualization library that leverages GPU acceleration through
//! WebGPU to create interactive, scalable visualizations that work both natively and in web
//! browsers.
//!
//! ## Features
//!
//! - **GPU-Accelerated Rendering**: Utilizes WebGPU for high-performance graphics
//! - **Cross-Platform**: Works on desktop (Windows, macOS, Linux) and web (WebAssembly)
//! - **Interactive Visualizations**: Built-in support for user interactions and animations
//! - **Extensible Architecture**: Modular design allowing custom marks and interactions
//!
//! ## Architecture Overview
//!
//! Gup is organised around several core abstractions that compose into a full
//! visualisation pipeline:
//!
//! - **[`Selection`]** – The primary data-binding type. A selection associates
//!   data elements with GPU-resident mark instances and drives the
//!   enter/update/exit lifecycle.
//! - **`ShaderFunction`** – Composable GPU
//!   shader functions that map data attributes to visual channels (position,
//!   colour, size, …) and can be combined through the
//!   [`ComposableShaderFunction`] trait.
//! - **[`Mark`]** – Visual primitives (circle, rectangle, line, …)
//!   rendered on the GPU. Marks declare their vertex layout and shader code and
//!   are registered in a [`MarkRegistry`].
//! - **[`GupContext`]** – The central GPU context that owns the wgpu device,
//!   queue, surface, and auxiliary caches (buffer pool, pipeline cache, texture
//!   pool).
//! - **[`ChartBuilder`]** – A high-level, fluent
//!   API for constructing common chart types (scatter, line, bar, area, …)
//!   without manual shader or mark wiring.
//!
//! Data flows through the library as follows:
//!
//! ```text
//! Data  ──▶  Selection  ──▶  ShaderFunction (scale/encode)  ──▶  Mark (GPU draw)
//!                                     │
//!                                     ▼
//!                            Axis / Grid / Label (annotation layer)
//! ```
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use gup::prelude::*;
//!
//! // Create GPU-compatible types with ergonomic macros
//! let position = vec3![1.0, 2.0, 3.0];
//! let color = vec4![1.0, 0.5, 0.0, 1.0];
//! ```
//!
//! ### Building a chart
//!
//! ```rust
//! use gup::ScatterPlotBuilder;
//!
//! let builder = ScatterPlotBuilder::<(f32, f32)>::new();
//! ```
//!
//! ## Type Construction
//!
//! Gup provides ergonomic macros for GPU-compatible vector and matrix types:
//!
//! - `vec2![x, y]` - 2D vectors
//! - `vec3![x, y, z]` - 3D vectors with automatic GPU padding
//! - `vec4![x, y, z, w]` - 4D vectors (colours, homogeneous coordinates)
//! - `mat2![...]`, `mat3![...]`, `mat4![...]` - Matrix construction
//!
//! These macros ensure proper GPU memory alignment and provide zero-cost abstractions.
//!
//! ## Module Organisation
//!
//! | Module | Purpose |
//! |--------|---------|
//! | [`accessibility`] | Screen reader support, ARIA tree, focus management |
//! | [`axis`] / [`axis_system`] | Axis rendering, tick generation, axis layout |
//! | [`buffer`] | GPU buffer pool, upload/download helpers |
//! | [`chart_builder`] | High-level chart construction API |
//! | [`context`] | [`GupContext`] — device, queue, surface management |
//! | [`error`] | [`GupError`] variants and the [`GupResult`] type alias |
//! | [`grid`] | GPU-accelerated grid line rendering |
//! | [`interaction`] | GPU hit-testing, event handling, gesture recognition |
//! | [`mark`] | Mark trait, registry, built-in marks (circle, rect, line, …) |
//! | [`mixable`] | Composable visualisation trait and helpers |
//! | [`scale`] | Scale types (linear, log, ordinal, …) |
//! | [`selection`] | [`Selection<T, M>`](Selection) — data-binding core |
//! | [`shader_function`] | Composable GPU shader functions |
//! | [`text`] | SDF-based GPU text rendering pipeline |
//! | [`prelude`] | Convenience re-exports for common imports |

#![cfg_attr(docsrs, feature(doc_cfg))]
#![deny(missing_docs)]

// Allow `gup::` paths in proc macro-generated code from within this crate.
extern crate self as gup;

pub mod accessibility;
pub mod app;
pub mod async_mixable;
pub mod axis;
pub mod axis_performance;
pub mod axis_system;
pub mod brush;
pub mod buffer;
pub mod camera;
pub mod chart_builder;
pub mod color_descriptor;
pub mod context;
pub mod debug;
pub mod depth;
pub mod error;
pub mod event;
#[doc(hidden)]
pub mod examples;
pub mod export;
pub mod gpu_timer;
pub mod grid;
#[doc(hidden)]
pub mod integration;
pub mod interaction;
pub mod label;
pub mod layout;
pub mod lighting;
pub mod linked_selection;
pub mod lod;
pub mod mark;
pub mod mark_selection;
pub mod math;
pub mod mixable;
pub mod performance;
pub mod performance_export;
pub mod performance_targets;
pub mod pipeline_cache;
pub mod platform;
pub mod plugins;
pub mod prelude;
pub mod render;
pub mod renderer;
pub mod scale;
pub mod selection;
pub mod selection_mask;
pub mod shader_ast;
pub mod shader_function;
pub mod shader_pipeline;
pub mod spatial_index;
pub mod streaming;
#[doc(hidden)]
pub mod test_utils;
pub mod text;
pub mod tick_generator;
pub mod transition;
#[doc(hidden)]
pub mod visual_test_utils;
pub mod wasm_api;
#[doc(hidden)]
pub mod wasm_bench;
#[doc(hidden)]
pub mod wasm_bench_axis;
#[doc(hidden)]
pub mod wasm_bench_interaction;
pub mod zoom;

/// Hidden module for procedural macro support.
///
/// This module re-exports types that the `#[derive(Mark)]` macro needs
/// to reference in generated code. Do not use directly.
#[doc(hidden)]
pub mod __private {
    pub use crate::error::GupError;
    pub use crate::mark::Mark;
    pub use crate::shader_pipeline::ComposableShaderPipeline;
}

// ---------------------------------------------------------------------------
// Conditional Send/Sync marker traits for cross-platform (native + WASM) support.
//
// On native targets wgpu types are Send + Sync, so async futures and trait
// objects must be Send/Sync too.  On WASM the WebGPU backend wraps JS objects
// (Rc, *mut u8, RefCell…) that are inherently !Send/!Sync, but WASM is
// single-threaded so this is fine.  These helper traits let the rest of the
// crate express "Send if native" without duplicating every trait definition.
// ---------------------------------------------------------------------------

/// Marker trait that equals `Send` on native and is auto-implemented on WASM.
#[cfg(not(target_arch = "wasm32"))]
pub trait MaybeSend: Send {}
#[cfg(not(target_arch = "wasm32"))]
impl<T: Send> MaybeSend for T {}

#[cfg(target_arch = "wasm32")]
pub trait MaybeSend {}
#[cfg(target_arch = "wasm32")]
impl<T> MaybeSend for T {}

/// Marker trait that equals `Sync` on native and is auto-implemented on WASM.
#[cfg(not(target_arch = "wasm32"))]
pub trait MaybeSync: Sync {}
#[cfg(not(target_arch = "wasm32"))]
impl<T: Sync> MaybeSync for T {}

#[cfg(target_arch = "wasm32")]
pub trait MaybeSync {}
#[cfg(target_arch = "wasm32")]
impl<T> MaybeSync for T {}

// Export application shell
pub use app::{AppRenderer, GupApp};

pub use color_descriptor::{
    ColorNamer, Hsl, describe_color, describe_color_detailed, describe_color_with, rgba_to_hsl,
};

// Export accessibility system components
pub use accessibility::high_contrast::{
    AccessibilityOverrides, Color as AccessibilityColor, calculate_contrast_ratio,
};
pub use accessibility::{
    AccessibilityAction, AccessibilitySettings, AccessibilitySystem, AriaLive, AriaNode,
    AriaProperties, AriaRelevant, AriaRole, AriaTree, AriaUpdate, AudioEvent, AudioParameter,
    AudioTrack, ContrastMode, ContrastTheme, DataPatterns, Direction, ElementId, ElementType,
    FocusManager, FocusableElement, HighContrastRenderer, KeyEvent, MappingFunction,
    NavigationMode, NodeId, Pattern, PatternLibrary, SonificationEngine, SonificationMapping,
    Trend,
};
// Export async components selectively to avoid conflicts
pub use async_mixable::{
    AsyncComposedVisualization, AsyncMixable, AsyncMixableExt, AsyncRenderStrategy, RenderProgress,
    SyncAdapter, TimeoutComposition,
    progressive::{
        ProgressiveConfig, ProgressiveDataLoader, ProgressiveVisualization, QualityLevel,
    },
    streaming::{Point2D, StreamStats, StreamingDataSource, StreamingScatterPlot},
    utils::{
        AsyncCompositionBuilder, AsyncPerformanceMonitor, ComponentStats, MultiAsyncComposition,
        compose,
    },
};
pub use axis::*;
pub use axis_system::{
    AxisLayout, AxisMappings, AxisMargins, AxisSystem, ChartArea, ScaleConfiguration,
};
pub use buffer::*;
pub use context::*;
pub use debug::*;
pub use error::*;
pub use examples::*;
pub use grid::*;
pub use integration::*;
pub use label::*;
pub use scale::*;
// Export interaction system components (excluding ambiguous types)
pub use interaction::{
    CustomInteractionQuery, ElementData, ElementHit, EventHandler, GestureRecognizer, GestureType,
    GpuInteractionQuery, InteractionElement, InteractionEvent, InteractionResult,
    InteractionSystem, InteractionType, PropagationPhase, QueryHandle, QueryStats, Rect,
    Renderable, TouchPoint,
};
// Export event handling system
pub use event::{
    CoalescingConfig, EventManager, EventResult, EventType, ModifierFlags, RawInputEvent,
    ViewportTransform,
};
// Export zoom/pan behaviour
pub use mixable::*;
pub use plugins::*;
pub use render::*;
pub use shader_function::*;
pub use shader_pipeline::*;
pub use text::*;
pub use zoom::{GpuViewportTransform, ZoomBehavior};
// Export brush selection system
pub use brush::{
    BrushBehavior, BrushEvent, BrushExtent, BrushMark, BrushOverlayRenderer, BrushStyle,
    GpuBrushConfig,
};
// Export tick generator with explicit types to avoid conflicts
pub use tick_generator::{
    // Renamed to avoid conflict with shader_function::LinearScale (the GPU
    // shader function). tick_generator::LinearScale is a CPU-side type used for
    // axis tick generation; shader_function::LinearScale is the GPU-side type
    // that generates WGSL code for data scaling on the GPU.
    LinearScale as TickLinearScale,
    LinearTickGenerator,
    LogarithmicScale,
    LogarithmicTickGenerator,
    Scale,
    TickGenerator,
    TimeInterval,
    TimeScale,
    TimeTickGenerator,
    TimeUnit,
};

// Export mark system with explicit re-exports to avoid conflicts
pub use mark::advanced_rendering::{
    DynamicAttributeBufferManager, DynamicAttributeMap, DynamicAttributeValue, MarkBlendConfig,
    MarkViewport, MultiPassConfig, MultiPassRenderer, RenderPassConfig, RenderStateManager,
    RenderStateSnapshot, ScissorRect, UploadStats,
};
pub use mark::batch_renderer::{
    BatchFrameStats, BatchRendererConfig, CullingManager, GeometryCache, InstanceAttributes,
    InstancedBatchRenderer, LodLevel, RenderBatch, Viewport2D,
};
pub use mark::boxplot::{
    BoxPlot, BoxPlotAttributes, BoxPlotInstance, BoxPlotOrientation, BoxPlotVertex,
};
pub use mark::circle::{Circle, CircleAttributes, CircleInstance, CircleVertex};
pub use mark::compute_instance_filter::{
    ComputeInstanceFilter, FilterConfig, FilterResult, PooledComputeInstanceFilter,
};
pub use mark::line::{Line, LineAttributes, LineInstance, LineStyle, LineVertex};
pub use mark::occlusion_culler::{
    OcclusionCuller, OcclusionGpuConfig, OcclusionParams, OcclusionResult, PooledOcclusionCuller,
};
pub use mark::radix_sort::{RadixSorter, SortBuffers, SortConfig};
pub use mark::rectangle::{Rectangle, RectangleAttributes, RectangleInstance, RectangleVertex};
pub use mark::unified_culling_pipeline::UnifiedCullingPipeline;
pub use mark::{Mark, MarkInfo, MarkInfoImpl, MarkRegistry};

// Export selection system
pub use pipeline_cache::PipelineCache;
pub use selection::{
    AccessibleMark, AriaUpdateConfig, AttrValue, InteractionData, IntoAttrValue, IntoAttrValues,
    MarkInstanceBuilder, Selection, ViewportUniforms,
};

// Export transition system
pub use transition::{
    TransitionBuilder, TransitionConfig, TransitionState,
    builder::{CommittedTransition, EasingFn, ElementTransition, TransitionGroup},
    diff::{DiffResult, diff_by_key},
};

// Export interactive mark selection system
pub use mark_selection::{
    BitSet, HapticFeedback, KeyModifiers, MarkSelectionSystem, SelectionEvent, SelectionMode,
    SelectionOperation, SelectionState, SelectionStatistics, SelectionStyle, SelectionTool,
    SelectionToolKind, ToolResult, ToolState, TouchEvent, TouchPhase, TouchSelectionAdapter,
    TouchSelectionConfig, point_in_polygon,
};

// Export linked-view coordination system
pub use linked_selection::{
    DimInstance, KeyedSelectionState, SharedSelectionState, build_dimmed_instances,
    has_changed_since,
};

// Export layout engine
pub use layout::{
    ForceDirected, GraphChartBuilder, GraphLayout, LayoutEdge, LayoutEngine, LayoutNode,
    LayoutRect, LayoutResult, NodePosition, TreeNode, TreemapAlgorithm, TreemapCell,
    TreemapOptions, TreemapResult,
};

// Export chart builder system (Observable Plot-style API)
// Re-export selectively to avoid conflicts with label::Margins
pub use chart_builder::builders::area::{AreaSegment, StackMode};
pub use chart_builder::builders::bar::{BarOrientation, Category, Orientation};
pub use chart_builder::builders::density::{
    ContourBand, ContourLevel, DensityConfig, DensityLayer, DensityPlotBuilder, DensityRenderMode,
};
pub use chart_builder::builders::heatmap::{AggregateFunc, BinGrid, BinSpec, HeatmapCell};
pub use chart_builder::builders::violin::{HalfSide, ViolinOrientation, ViolinPlotBuilder};
pub use chart_builder::{
    AreaChartBuilder, AxisLabelConfig, BarChartBuilder, ChartBuilder, ChartConfig, ChoroplethChart,
    ChoroplethChartBuilder, ComposedChart, HeatmapBuilder, LabelCapableBuilder, LabeledChart,
    LineChartBuilder, ScatterPlotBuilder, TitleAlignment, TitleConfig, plot,
};

/// Create a new [`DensityPlotBuilder`] (convenience shorthand).
///
/// # Examples
///
/// ```rust,no_run
/// use gup::prelude::*;
/// use gup::chart_builder::builders::AccessorFunction;
/// use gup::chart_builder::accessor::AccessorValue;
///
/// #[derive(Debug, Clone)]
/// struct Point { x: f32, y: f32 }
///
/// let builder = gup::density_plot()
///     .x(AccessorFunction::new(|d: &Point| AccessorValue::Float(d.x)))
///     .y(AccessorFunction::new(|d: &Point| AccessorValue::Float(d.y)));
/// ```
pub fn density_plot<T>() -> DensityPlotBuilder<T> {
    DensityPlotBuilder::new()
}

/// Create a new [`ChoroplethChartBuilder`] (convenience shorthand).
pub fn choropleth() -> ChoroplethChartBuilder {
    ChoroplethChartBuilder::new()
}

// Note: Procedural macros from gup_macros must be imported directly due to Rust limitations
// Available macros:
// - `use gup_macros::wgsl_function;` - WGSL shader function generation (write WGSL syntax)
// - `use gup_macros::shader_fn;`     - Rust-to-WGSL transpiled shader function generation
// - `use gup_macros::Mixable;` - Automatic Mixable trait derivation

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(all(target_arch = "wasm32", feature = "wasm-start"))]
#[wasm_bindgen(start)]
pub fn main() {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log::init_with_level(log::Level::Warn).expect("Couldn't initialize logger");
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_library_loads() {
        // Basic smoke test to ensure the library loads correctly
    }
}

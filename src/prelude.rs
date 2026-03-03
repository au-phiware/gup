// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Gup prelude module for convenient imports.
//!
//! This module re-exports the most commonly used types and functions
//! for convenience when using the Gup library.

// Core types and traits
pub use crate::RenderContext;
pub use crate::error::{GupError, GupResult};

// Chart builder API (Observable Plot style)
pub use crate::chart_builder::{
    AxisScale, ChartBuilder, ChartConfig,
    accessor::{AccessorValue, ConstantAccessor, FieldAccessor, color, size, x, y},
    builders::{
        AccessorFunction, AreaChartBuilder, BarChartBuilder, ConfigurableBuilder,
        GridCapableBuilder, HeatmapBuilder, LineChartBuilder, LineInterpolation, LineSegment,
        ScatterPlotBuilder, area,
        area::{AreaSegment, StackMode},
        bar,
        bar::{Category, Orientation},
        heatmap,
        heatmap::{AggregateFunc, HeatmapCell},
        line, scatter,
    },
    plot_api::{BoundPlotBuilder, PlotBuilder, plot},
};

// Selection API (low-level)
pub use crate::pipeline_cache::PipelineCache;
pub use crate::selection::{Mark, Selection};

// Mark types
pub use crate::{BoxPlot, Circle, Line, Rectangle};

// Mark validation and profiling
pub use crate::mark::validation::{MarkProfiler, MarkValidator, assert_mark_valid};

// Advanced mark rendering
pub use crate::mark::advanced_rendering::{
    DynamicAttributeBufferManager, DynamicAttributeMap, DynamicAttributeValue, MarkBlendConfig,
    MarkViewport, MultiPassConfig, MultiPassRenderer, RenderPassConfig, RenderStateManager,
    UploadStats,
};

// Mixable trait for composition
pub use crate::mixable::Mixable;

// Shader functions
pub use crate::shader_function::{
    // Alpha blending (GUP-053)
    AlphaBlending,
    // Animation event system (GUP-142)
    AnimationEventCallback,
    AnimationEventType,
    // Advanced temporal animation (GUP-138)
    AnimationPlaybackState,
    AnimationTimeline,
    AnimationTimelineWithEvents,
    BandwidthMethod,
    // Binning (GUP-053)
    BinningFunction,
    BinningStrategy,
    // Filtering and clamping (GUP-033)
    Clamp,
    ColorGradient,
    ColorGradientBuilder,

    // Storage buffer-based gradient (GUP-134)
    ColorGradientStorage,
    ColorMap,
    // ColorScale (GUP-255)
    ColorScale,
    ColorScaleKind,
    ColorScaleUniforms,
    // Color space conversion (GUP-053)
    ColorSpaceConverter,
    ColorSpaceDirection,
    ComposableFunction,
    // Core traits
    ComposableShaderFunction as ShaderFunction,
    // Advanced composition patterns (GUP-033 AC3)
    ConditionalFunction,
    CubicBezierTiming,
    // Distance function (GUP-053)
    DistanceFunction,
    Easing,
    EasingFunction,
    // Exponential scale (GUP-053)
    ExponentialScale,

    // HSV color mapping (GUP-053)
    HSVColorMap,
    // Histogram functions (GUP-143)
    Histogram,
    HistogramCompute,
    HistogramConfig,
    HistogramResult,
    // Spline interpolation (GUP-141)
    InterpolationMode,
    // Kernel Density Estimation (GUP-144)
    KDEResult,
    KDEResult2D,
    KernelDensity1D,
    KernelDensity2D,
    KernelFunction,
    Keyframe,
    KeyframeAnimation,
    // Storage buffer-based keyframe animation (GUP-140)
    KeyframeAnimationStorage,
    KeyframeAnimationStorageBuilder,
    // Basic transformations
    LinearScale,
    // Advanced scales (GUP-033)
    LogScale,
    LogScaleUniforms,
    MAX_KEYFRAMES,

    Mat2,
    Mat3,
    Mat4,
    // Matrix transform (GUP-053)
    MatrixTransform,
    // Statistical functions (GUP-139)
    Mean,
    MinMax,
    // Normalize function (GUP-053)
    NormalizeFunction,
    // Parallel composition (GUP-136)
    ParallelComposable,
    ParallelComposition,
    ParallelOutput,
    // Statistical functions (GUP-139)
    Percentile,
    // Polar transform (GUP-053)
    PolarTransform,

    PositionTransform,

    PowerScale,
    // Projection transform (GUP-053)
    ProjectionTransform,
    // Quantile function (GUP-053)
    QuantileFunction,

    ShaderCompatible,

    ShaderType,
    // Interpolation (GUP-033)
    SmoothStep,
    // Statistical functions (GUP-139)
    StandardDeviation,
    // Standardize function (GUP-053)
    StandardizeFunction,
    StatisticsCompute,
    StatisticsResult,
    // Streaming statistics (GUP-146)
    StreamingStatistics,
    TemporalInterpolation,
    Threshold,

    // Shader types
    Vec2,
    Vec3,
    Vec4,
    // Parallel output extraction (GUP-140)
    parallel_output_extraction,
};

// Attribute binding types
pub use crate::selection::{AttrValue, IntoAttrValue, IntoAttrValues, MarkInstanceBuilder};

// Transition system (enter/update/exit data animations)
pub use crate::transition::builder::{
    CommittedTransition, EasingFn, ElementTransition, TransitionGroup,
};
pub use crate::transition::diff::{DiffResult, diff_by_key};
pub use crate::transition::{TransitionBuilder, TransitionConfig, TransitionState};

// Text rendering system
pub use crate::text::hover_reveal::{
    ArrowDirection, ClippedTextRegistry, HoverRevealState, TooltipConfig, TooltipLayout,
};
pub use crate::text::tooltip_bg::TooltipBackgroundRenderer;
pub use crate::text::ui_quad::{UiQuadArrow, UiQuadConfig, UiQuadInstance, UiQuadRenderer};
pub use crate::text::{
    FontAtlas, GlyphBatch, PositionedGlyph, TextAnchor, TextBounds, TextLayoutEngine, TextRenderer,
    TextStyle,
};

// Label positioning and collision detection
pub use crate::label::{
    AxisInfo, LabelConstraints, LabelLayout, LabelPosition, LabelPositioner,
    LabelPositioningStrategy,
};

// Interaction system
pub use crate::interaction::{InteractionEvent, InteractionSystem, InteractionType, Renderable};

// Linked view coordination
pub use crate::linked_selection::{
    DimInstance, LinkedSelection, SharedSelectionState, build_dimmed_instances, has_changed_since,
};

// Zoom and pan behaviour
pub use crate::zoom::{GpuViewportTransform, ZoomBehavior};

// Integration and plugin system
pub use crate::integration::{
    ExternalRenderer, ExternalVisualizationBuilder, ExternalVisualizationWrapper, wrap_point_data,
    wrap_with_custom_render,
};
pub use crate::plugins::{
    MixablePlugin, MixablePluginRegistry, PluginMetadata, global_registry, try_make_mixable,
};

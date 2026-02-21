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
    ChartBuilder, ChartConfig,
    accessor::{AccessorValue, ConstantAccessor, FieldAccessor, color, size, x, y},
    builders::{
        AccessorFunction, AreaChartBuilder, BarChartBuilder, ConfigurableBuilder,
        GridCapableBuilder, HeatmapBuilder, LineChartBuilder, ScatterPlotBuilder, area, bar,
        heatmap, line, scatter,
    },
    plot_api::{BoundPlotBuilder, PlotBuilder, plot},
};

// Selection API (low-level)
pub use crate::selection::{Mark, Selection};

// Mark types
pub use crate::{Circle, Line, Rectangle};

// Mixable trait for composition
pub use crate::mixable::Mixable;

// Shader functions
pub use crate::shader_function::{
    // Filtering and clamping (GUP-033)
    Clamp,
    ColorGradient,

    ColorMap,
    ComposableFunction,
    // Core traits
    ComposableShaderFunction as ShaderFunction,
    // Basic transformations
    LinearScale,
    // Advanced scales (GUP-033)
    LogScale,
    Mat2,
    Mat3,
    Mat4,
    PositionTransform,

    PowerScale,

    ShaderCompatible,

    ShaderType,
    // Interpolation (GUP-033)
    SmoothStep,
    Threshold,

    // Shader types
    Vec2,
    Vec3,
    Vec4,
};

// Legacy aliases for compatibility
pub use crate::shader_function::ComposableShaderFunction as ColorShaderFunction;
pub use crate::shader_function::ComposableShaderFunction as PositionShaderFunction;

// Text rendering system
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

// Integration and plugin system
pub use crate::integration::{
    ExternalRenderer, ExternalVisualizationBuilder, ExternalVisualizationWrapper, wrap_point_data,
    wrap_with_custom_render,
};
pub use crate::plugins::{
    MixablePlugin, MixablePluginRegistry, PluginMetadata, global_registry, try_make_mixable,
};

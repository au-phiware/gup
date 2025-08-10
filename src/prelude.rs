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
        AreaChartBuilder, BarChartBuilder, HeatmapBuilder, LineChartBuilder, ScatterPlotBuilder,
        area, bar, heatmap, line, scatter,
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
pub use crate::{ColorShaderFunction, PositionShaderFunction, ShaderFunction};

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

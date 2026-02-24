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
//! ## Quick Start
//!
//! ```rust,ignore
//! use gup::prelude::*;
//!
//! // Create GPU-compatible types with ergonomic macros
//! let position = vec3![1.0, 2.0, 3.0];
//! let color = vec4![1.0, 0.5, 0.0, 1.0];
//!
//! // Create visualizations
//! gup::plot()
//!     .data(sales_data)
//!     .scatter(x("revenue"), y("profit"))
//!     .render()?;
//! ```
//!
//! ## Type Construction
//!
//! Gup provides ergonomic macros for GPU-compatible vector and matrix types:
//!
//! - `vec2![x, y]` - 2D vectors
//! - `vec3![x, y, z]` - 3D vectors with automatic GPU padding
//! - `vec4![x, y, z, w]` - 4D vectors (colors, homogeneous coordinates)
//! - `mat2![...]`, `mat3![...]`, `mat4![...]` - Matrix construction
//!
//! These macros ensure proper GPU memory alignment and provide zero-cost abstractions.
//! See the [Type Construction Guide](../docs/TYPE_CONSTRUCTION_GUIDE.md) for details.
//!
//! ## Module Organization
//!
//! - [`shader_function`] - Composable GPU shader functions
//! - [`mark`] - Visualization mark types (Circle, Rectangle, Line)
//! - [`scale`] - Data scaling and transformation
//! - [`axis`] - Axis rendering and tick generation
//! - [`accessibility`] - Screen reader and keyboard navigation support
//! - [`prelude`] - Commonly used imports

pub mod accessibility;
pub mod async_mixable;
pub mod axis;
pub mod axis_system;
pub mod buffer;
pub mod chart_builder;
pub mod context;
pub mod debug;
pub mod error;
pub mod examples;
pub mod grid;
pub mod integration;
pub mod interaction;
pub mod label;
pub mod mark;
pub mod mixable;
pub mod performance;
pub mod plugins;
pub mod prelude;
pub mod render;
pub mod scale;
pub mod selection;
pub mod shader_function;
pub mod shader_pipeline;
pub mod spatial_index;
pub mod test_utils;
pub mod text;
pub mod tick_generator;
pub mod visual_test_utils;
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
    InteractionSystem, InteractionType, PropagationPhase, QueryStats, Rect, Renderable, TouchPoint,
};
pub use mixable::*;
pub use plugins::*;
pub use render::*;
pub use shader_function::*;
pub use shader_pipeline::*;
pub use text::*;
// Export tick generator with explicit types to avoid conflicts
pub use tick_generator::{
    LinearScale as TickLinearScale, // Renamed to avoid conflict with shader_function::LinearScale
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
pub use mark::boxplot::{
    BoxPlot, BoxPlotAttributes, BoxPlotInstance, BoxPlotOrientation, BoxPlotVertex,
};
pub use mark::circle::{Circle, CircleAttributes, CircleInstance, CircleVertex};
pub use mark::line::{Line, LineAttributes, LineStyle, LineVertex};
pub use mark::rectangle::{Rectangle, RectangleAttributes, RectangleInstance, RectangleVertex};
pub use mark::{Mark, MarkInfo, MarkInfoImpl, MarkRegistry};

// Export selection system
pub use selection::{ColorShaderFunction, InteractionData, PositionShaderFunction, Selection};

// Export chart builder system (Observable Plot-style API)
// Re-export selectively to avoid conflicts with label::Margins
pub use chart_builder::{
    AreaChartBuilder, AxisLabelConfig, BarChartBuilder, ChartBuilder, ChartConfig, ComposedChart,
    HeatmapBuilder, LabelCapableBuilder, LabeledChart, LineChartBuilder, ScatterPlotBuilder, plot,
};

// Note: Procedural macros from gup_macros must be imported directly due to Rust limitations
// Available macros:
// - `use gup_macros::wgsl_function;` - WGSL shader function generation
// - `use gup_macros::Mixable;` - Automatic Mixable trait derivation

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
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

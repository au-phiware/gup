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

pub mod async_mixable;
pub mod axis;
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
pub mod plugins;
pub mod prelude;
pub mod render;
pub mod selection;
pub mod shader_function;
pub mod shader_pipeline;
pub mod text;
pub mod tick_generator;

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
pub use buffer::*;
pub use context::*;
pub use debug::*;
pub use error::*;
pub use examples::*;
pub use grid::*;
pub use integration::*;
pub use label::*;
// Export interaction system components (excluding ambiguous types)
pub use interaction::{
    CustomInteractionQuery, ElementData, ElementHit, EventHandler, GpuInteractionQuery,
    InteractionElement, InteractionEvent, InteractionResult, InteractionSystem, InteractionType,
    QueryStats, Rect, Renderable,
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
pub use mark::circle::{Circle, CircleAttributes, CircleVertex};
pub use mark::line::{Line, LineAttributes, LineStyle, LineVertex};
pub use mark::rectangle::{Rectangle, RectangleAttributes, RectangleVertex};
pub use mark::{Mark, MarkInfo, MarkInfoImpl, MarkRegistry};

// Export selection system
pub use selection::*;

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

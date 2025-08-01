// Gup - GPU-Accelerated Data Visualization Library
// Copyright (C) 2025 Corin Lawson <corin@phiware.com.au>
//

use crate::render::{BasicPipeline, Vertex};
use crate::{
    CrossFadeComposition, CustomCompositionBehavior, GupResult, LayoutDirection, Mixable,
    MixableExt, RenderContext, SideBySideConfig,
};
use std::sync::Arc;

/// Example of a real GPU-accelerated Mixable implementation
#[derive(Debug, Clone)]
pub struct GpuScatterPlot {
    points: Vec<Vertex>,
    pipeline: Option<Arc<BasicPipeline>>,
}

impl GpuScatterPlot {
    pub fn new(data: Vec<(f32, f32)>, color: [f32; 4]) -> Self {
        let points = data
            .into_iter()
            .map(|(x, y)| Vertex {
                position: [x, y],
                color,
            })
            .collect();

        Self {
            points,
            pipeline: None,
        }
    }

    fn ensure_pipeline(&mut self, context: &RenderContext) -> GupResult<Arc<BasicPipeline>> {
        if let Some(pipeline) = &self.pipeline {
            return Ok(pipeline.clone());
        }

        let surface_format = context.surface_format();
        let pipeline = Arc::new(BasicPipeline::new(context.device(), surface_format));
        self.pipeline = Some(pipeline.clone());
        Ok(pipeline)
    }
}

impl Mixable for GpuScatterPlot {
    type Output = ();

    fn render(&mut self, context: &mut RenderContext) -> GupResult<()> {
        // Get or create rendering pipeline first
        let pipeline = self.ensure_pipeline(context)?;

        // Create vertex buffer first
        let vertex_buffer = pipeline.render_points_with_context(&self.points, context)?;

        let mut render_pass = context.begin_render_pass()?;
        let mut rpass = render_pass.render_pass(None);

        // Render points using GPU pipeline
        pipeline.render_points(&mut rpass, &vertex_buffer, self.points.len() as u32)?;

        drop(rpass);
        render_pass.submit()?;

        Ok(())
    }

    fn is_valid(&self) -> bool {
        !self.points.is_empty()
    }

    fn description(&self) -> String {
        format!("GpuScatterPlot({} points)", self.points.len())
    }
}

/// Example: Creating compositions with different modes
pub mod composition_examples {
    use super::*;

    /// Demonstrates overlay composition - second plot renders on top of first
    pub fn overlay_example() -> impl Mixable<Output = ()> {
        let background_data = vec![(0.1, 0.2), (0.3, 0.4), (0.5, 0.6)];
        let foreground_data = vec![(0.2, 0.3), (0.4, 0.5), (0.6, 0.7)];

        let background = GpuScatterPlot::new(background_data, [0.5, 0.5, 1.0, 0.7]); // Blue, semi-transparent
        let foreground = GpuScatterPlot::new(foreground_data, [1.0, 0.5, 0.5, 0.8]); // Red, semi-transparent

        background.overlay(foreground)
    }

    /// Demonstrates side-by-side composition with custom layout
    pub fn side_by_side_example() -> impl Mixable<Output = ()> {
        let left_data = vec![(0.1, 0.2), (0.2, 0.4), (0.3, 0.6)];
        let right_data = vec![(0.7, 0.2), (0.8, 0.4), (0.9, 0.6)];

        let left_plot = GpuScatterPlot::new(left_data, [1.0, 0.0, 0.0, 1.0]); // Red
        let right_plot = GpuScatterPlot::new(right_data, [0.0, 1.0, 0.0, 1.0]); // Green

        let config = SideBySideConfig {
            direction: LayoutDirection::Horizontal,
            split_ratio: 0.6, // Left plot gets 60% of width
            padding: 20.0,
        };

        left_plot.beside_with_config(right_plot, config)
    }

    /// Demonstrates vertical side-by-side composition
    pub fn vertical_split_example() -> impl Mixable<Output = ()> {
        let top_data = vec![(0.2, 0.8), (0.4, 0.9), (0.6, 0.7)];
        let bottom_data = vec![(0.2, 0.3), (0.4, 0.2), (0.6, 0.4)];

        let top_plot = GpuScatterPlot::new(top_data, [0.0, 0.0, 1.0, 1.0]); // Blue
        let bottom_plot = GpuScatterPlot::new(bottom_data, [1.0, 1.0, 0.0, 1.0]); // Yellow

        let config = SideBySideConfig {
            direction: LayoutDirection::Vertical,
            split_ratio: 0.3, // Top plot gets 30% of height
            padding: 15.0,
        };

        top_plot.beside_with_config(bottom_plot, config)
    }

    /// Demonstrates cross-fade composition with custom behavior
    pub fn cross_fade_example() -> impl Mixable<Output = ()> {
        let data1 = vec![(0.1, 0.1), (0.3, 0.3), (0.5, 0.5)];
        let data2 = vec![(0.2, 0.2), (0.4, 0.4), (0.6, 0.6)];

        let plot1 = GpuScatterPlot::new(data1, [1.0, 0.0, 0.0, 1.0]); // Red
        let plot2 = GpuScatterPlot::new(data2, [0.0, 1.0, 0.0, 1.0]); // Green

        // Custom cross-fade composition with 70% of plot2 visible
        let behavior =
            CustomCompositionBehavior::CrossFade(CrossFadeComposition { fade_factor: 0.7 });

        plot1.custom_compose(plot2, behavior)
    }

    /// Demonstrates merge composition (currently placeholder behavior)
    pub fn merge_example() -> impl Mixable<Output = ()> {
        let dataset1 = vec![(0.1, 0.2), (0.3, 0.4)];
        let dataset2 = vec![(0.5, 0.6), (0.7, 0.8)];

        let plot1 = GpuScatterPlot::new(dataset1, [1.0, 0.0, 0.0, 1.0]); // Red
        let plot2 = GpuScatterPlot::new(dataset2, [0.0, 1.0, 0.0, 1.0]); // Green

        // In future implementations, this would combine the datasets
        plot1.merge(plot2)
    }

    /// Demonstrates complex nested composition
    pub fn complex_composition_example() -> impl Mixable<Output = ()> {
        let data1 = vec![(0.1, 0.1), (0.2, 0.2)];
        let data2 = vec![(0.3, 0.3), (0.4, 0.4)];
        let data3 = vec![(0.5, 0.5), (0.6, 0.6)];
        let data4 = vec![(0.7, 0.7), (0.8, 0.8)];

        let red_plot = GpuScatterPlot::new(data1, [1.0, 0.0, 0.0, 1.0]);
        let green_plot = GpuScatterPlot::new(data2, [0.0, 1.0, 0.0, 1.0]);
        let blue_plot = GpuScatterPlot::new(data3, [0.0, 0.0, 1.0, 1.0]);
        let yellow_plot = GpuScatterPlot::new(data4, [1.0, 1.0, 0.0, 1.0]);

        // Create a 2x2 layout using nested side-by-side compositions
        let top_row = red_plot.beside(green_plot);
        let bottom_row = blue_plot.beside(yellow_plot);

        let vertical_config = SideBySideConfig {
            direction: LayoutDirection::Vertical,
            split_ratio: 0.5,
            padding: 5.0,
        };

        top_row.beside_with_config(bottom_row, vertical_config)
    }
}

/// Blend modes demonstration (console output)
pub mod blend_modes;

/// Re-export the main showcase function for easy access
pub use blend_modes::run_blend_modes_showcase;

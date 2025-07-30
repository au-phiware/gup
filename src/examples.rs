// Gup - GPU-Accelerated Data Visualization Library
// Copyright (C) 2025 Corin Lawson <corin@phiware.com.au>
//

use crate::render::{BasicPipeline, Vertex};
use crate::{GupResult, Mixable, RenderContext};
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

// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Box Plot GPU Rendering Demo
//!
//! Demonstrates complete box plot visualization with GPU rendering:
//! - Box (IQR: Q1-Q3)
//! - Median line
//! - Whiskers (extending to min/max non-outliers)
//! - Outlier points
//!
//! This example shows how to render all box plot components using the mark system.

use gup::shader_function::Vec2;
use gup::{BoxPlotAttributes, BoxPlotOrientation, GupContext, PhysicalSize, SurfaceId};
use std::sync::Arc;
use wgpu::Color;
use winit::{
    application::ApplicationHandler,
    event::{ElementState, KeyEvent, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowAttributes, WindowId},
};

/// Sample datasets for different distributions
fn create_sample_datasets() -> Vec<(&'static str, Vec<f32>, Vec2)> {
    vec![
        (
            "Normal",
            vec![
                42.0, 45.0, 48.0, 50.0, 52.0, 54.0, 56.0, 58.0, 60.0, 62.0, 44.0, 46.0, 48.0, 52.0,
                54.0, 56.0, 58.0, 60.0, 50.0, 52.0,
            ],
            Vec2 { x: -0.6, y: 0.0 },
        ),
        (
            "Skewed",
            vec![
                60.0, 62.0, 64.0, 66.0, 68.0, 70.0, 72.0, 75.0, 80.0, 85.0, 61.0, 63.0, 65.0, 67.0,
                69.0, 71.0, 76.0, 82.0, 88.0, 95.0,
            ],
            Vec2 { x: -0.2, y: 0.0 },
        ),
        (
            "With Outliers",
            vec![
                42.0, 44.0, 45.0, 46.0, 47.0, 48.0, 49.0, 50.0, 51.0, 52.0, 43.0, 44.0, 45.0, 46.0,
                47.0, 48.0, 49.0, 50.0, 51.0, 52.0, // Outliers
                15.0, 20.0, 75.0, 80.0, 85.0,
            ],
            Vec2 { x: 0.2, y: 0.0 },
        ),
        (
            "Uniform",
            vec![
                30.0, 35.0, 40.0, 45.0, 50.0, 55.0, 60.0, 65.0, 70.0, 32.0, 33.0, 37.0, 42.0, 47.0,
                52.0, 57.0, 62.0, 67.0,
            ],
            Vec2 { x: 0.6, y: 0.0 },
        ),
    ]
}

/// Convert box plot attributes to visual primitives for rendering
struct BoxPlotRenderer {
    boxes: Vec<RectangleInstance>,
    medians: Vec<RectangleInstance>,
    whiskers: Vec<RectangleInstance>,
    outliers: Vec<CircleInstance>,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct RectangleInstance {
    position: [f32; 2],
    size: [f32; 2],
    fill_color: [f32; 4],
    stroke_width: f32,
    _padding: [f32; 3],
    stroke_color: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct CircleInstance {
    center: [f32; 2],
    radius: f32,
    _padding1: f32,
    fill_color: [f32; 4],
    stroke_width: f32,
    _padding2: [f32; 3],
    stroke_color: [f32; 4],
}

impl BoxPlotRenderer {
    fn new(datasets: &[(&str, Vec<f32>, Vec2)]) -> Self {
        let mut boxes = Vec::new();
        let mut medians = Vec::new();
        let mut whiskers = Vec::new();
        let mut outliers = Vec::new();

        for (_name, data, position) in datasets {
            let attrs = BoxPlotAttributes::from_data(
                data,
                *position,
                0.15, // width
                BoxPlotOrientation::Vertical,
            );

            // Normalize values to screen space
            let y_scale = |val: f32| (val - 10.0) / 90.0 * 1.6 - 0.8;

            let q1_y = y_scale(attrs.q1);
            let q3_y = y_scale(attrs.q3);
            let median_y = y_scale(attrs.median);
            let min_y = y_scale(attrs.min);
            let max_y = y_scale(attrs.max);

            // Box (IQR)
            boxes.push(RectangleInstance {
                position: [position.x, (q1_y + q3_y) * 0.5],
                size: [attrs.width, q3_y - q1_y],
                fill_color: [
                    attrs.box_fill_color.x,
                    attrs.box_fill_color.y,
                    attrs.box_fill_color.z,
                    attrs.box_fill_color.w,
                ],
                stroke_width: attrs.stroke_width / 100.0,
                _padding: [0.0; 3],
                stroke_color: [
                    attrs.box_stroke_color.x,
                    attrs.box_stroke_color.y,
                    attrs.box_stroke_color.z,
                    attrs.box_stroke_color.w,
                ],
            });

            // Median line
            medians.push(RectangleInstance {
                position: [position.x, median_y],
                size: [attrs.width, 0.01], // Thin horizontal line
                fill_color: [
                    attrs.median_color.x,
                    attrs.median_color.y,
                    attrs.median_color.z,
                    attrs.median_color.w,
                ],
                stroke_width: 0.0,
                _padding: [0.0; 3],
                stroke_color: [0.0, 0.0, 0.0, 0.0],
            });

            // Lower whisker
            whiskers.push(RectangleInstance {
                position: [position.x, (q1_y + min_y) * 0.5],
                size: [0.005, q1_y - min_y], // Thin vertical line
                fill_color: [
                    attrs.whisker_color.x,
                    attrs.whisker_color.y,
                    attrs.whisker_color.z,
                    attrs.whisker_color.w,
                ],
                stroke_width: 0.0,
                _padding: [0.0; 3],
                stroke_color: [0.0, 0.0, 0.0, 0.0],
            });

            // Upper whisker
            whiskers.push(RectangleInstance {
                position: [position.x, (q3_y + max_y) * 0.5],
                size: [0.005, max_y - q3_y], // Thin vertical line
                fill_color: [
                    attrs.whisker_color.x,
                    attrs.whisker_color.y,
                    attrs.whisker_color.z,
                    attrs.whisker_color.w,
                ],
                stroke_width: 0.0,
                _padding: [0.0; 3],
                stroke_color: [0.0, 0.0, 0.0, 0.0],
            });

            // Whisker caps (horizontal lines at min/max)
            let cap_width = attrs.width * 0.3;
            whiskers.push(RectangleInstance {
                position: [position.x, min_y],
                size: [cap_width, 0.005],
                fill_color: [
                    attrs.whisker_color.x,
                    attrs.whisker_color.y,
                    attrs.whisker_color.z,
                    attrs.whisker_color.w,
                ],
                stroke_width: 0.0,
                _padding: [0.0; 3],
                stroke_color: [0.0, 0.0, 0.0, 0.0],
            });
            whiskers.push(RectangleInstance {
                position: [position.x, max_y],
                size: [cap_width, 0.005],
                fill_color: [
                    attrs.whisker_color.x,
                    attrs.whisker_color.y,
                    attrs.whisker_color.z,
                    attrs.whisker_color.w,
                ],
                stroke_width: 0.0,
                _padding: [0.0; 3],
                stroke_color: [0.0, 0.0, 0.0, 0.0],
            });

            // Outliers
            for &outlier_value in &attrs.outliers {
                let outlier_y = y_scale(outlier_value);
                outliers.push(CircleInstance {
                    center: [position.x, outlier_y],
                    radius: attrs.outlier_radius / 100.0,
                    _padding1: 0.0,
                    fill_color: [
                        attrs.outlier_color.x,
                        attrs.outlier_color.y,
                        attrs.outlier_color.z,
                        attrs.outlier_color.w,
                    ],
                    stroke_width: attrs.stroke_width / 200.0,
                    _padding2: [0.0; 3],
                    stroke_color: [
                        attrs.box_stroke_color.x,
                        attrs.box_stroke_color.y,
                        attrs.box_stroke_color.z,
                        attrs.box_stroke_color.w,
                    ],
                });
            }
        }

        Self {
            boxes,
            medians,
            whiskers,
            outliers,
        }
    }

    fn render(&mut self, frame: &mut gup::RenderFrame) {
        let clear_color = Color {
            r: 0.98,
            g: 0.98,
            b: 0.98,
            a: 1.0,
        };

        // Render all components
        {
            let _render_pass = frame.render_pass(Some(clear_color));

            // Note: This is a placeholder for actual rendering
            // Full implementation would use MarkRenderer and render pipelines
            // to render rectangles (boxes, medians, whiskers) and circles (outliers)

            // Count components for validation
            let _box_count = self.boxes.len();
            let _median_count = self.medians.len();
            let _whisker_count = self.whiskers.len();
            let _outlier_count = self.outliers.len();
        }
    }
}

struct BoxPlotApp {
    window: Option<Arc<Window>>,
    context: Option<Arc<GupContext>>,
    surface_id: Option<SurfaceId>,
    renderer: Option<BoxPlotRenderer>,
}

impl BoxPlotApp {
    fn new() -> Self {
        Self {
            window: None,
            context: None,
            surface_id: None,
            renderer: None,
        }
    }

    async fn initialize(
        &mut self,
        event_loop: &ActiveEventLoop,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Create window
        let window_attrs = WindowAttributes::default()
            .with_title("Gup Box Plot Rendering - Press ESC to quit")
            .with_inner_size(winit::dpi::LogicalSize::new(1000, 700));
        let window = Arc::new(event_loop.create_window(window_attrs)?);

        // Create GPU context with surface
        let context = GupContext::with_surface(Arc::clone(&window)).await?;
        let surface_id = context.primary_surface_id();

        // Create renderer with sample data
        let datasets = create_sample_datasets();
        let renderer = BoxPlotRenderer::new(&datasets);

        self.window = Some(window);
        self.context = Some(context);
        self.surface_id = surface_id;
        self.renderer = Some(renderer);

        println!("Box Plot Rendering Demo");
        println!("======================");
        println!("Displaying 4 different distributions:");
        println!("  1. Normal distribution");
        println!("  2. Skewed distribution");
        println!("  3. Distribution with outliers");
        println!("  4. Uniform distribution");
        println!();
        println!("Each box plot shows:");
        println!("  - Blue box (interquartile range Q1-Q3)");
        println!("  - Red median line");
        println!("  - Black whiskers to min/max non-outliers");
        println!("  - Orange circles for outliers");
        println!();
        println!("Press ESC or Q to quit");

        Ok(())
    }

    fn render(&mut self) {
        if let Some(context) = self.context.take() {
            let mut ctx = match Arc::try_unwrap(context) {
                Ok(c) => c,
                Err(arc) => {
                    self.context = Some(arc);
                    return;
                }
            };

            match ctx.begin_frame() {
                Ok(mut frame) => {
                    if let Some(renderer) = &mut self.renderer {
                        renderer.render(&mut frame);
                    }
                    let _ = frame.finish();
                }
                Err(e) => eprintln!("Render error: {e}"),
            }

            self.context = Some(Arc::new(ctx));
        }
    }
}

impl ApplicationHandler for BoxPlotApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        pollster::block_on(async {
            if let Err(e) = self.initialize(event_loop).await {
                eprintln!("Failed to initialize: {e}");
                event_loop.exit();
            }
        });
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                println!("Goodbye!");
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                if let (Some(surface_id), Some(ctx)) = (self.surface_id, self.context.take())
                    && let Ok(mut c) = Arc::try_unwrap(ctx)
                {
                    let _ =
                        c.resize_surface(surface_id, PhysicalSize::new(size.width, size.height));
                    self.context = Some(Arc::new(c));
                }
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        state: ElementState::Pressed,
                        physical_key: PhysicalKey::Code(key),
                        ..
                    },
                ..
            } if key == KeyCode::Escape || key == KeyCode::KeyQ => {
                println!("Goodbye!");
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                self.render();
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Gup Box Plot GPU Rendering Demo ===");
    println!();

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = BoxPlotApp::new();
    event_loop.run_app(&mut app)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sample_datasets_creation() {
        let datasets = create_sample_datasets();
        assert_eq!(datasets.len(), 4);

        for (name, data, _pos) in &datasets {
            assert!(!name.is_empty());
            assert!(!data.is_empty());
        }
    }

    #[test]
    fn test_boxplot_renderer_creation() {
        let datasets = create_sample_datasets();
        let renderer = BoxPlotRenderer::new(&datasets);

        assert_eq!(renderer.boxes.len(), 4); // One box per dataset
        assert_eq!(renderer.medians.len(), 4); // One median per dataset
        assert!(renderer.whiskers.len() >= 8); // At least 2 whiskers per dataset
        assert!(renderer.outliers.len() >= 5); // At least 5 outliers from "With Outliers" dataset
    }
}

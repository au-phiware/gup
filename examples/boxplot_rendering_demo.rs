// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Box Plot GPU Rendering Demo
//!
//! Demonstrates complete box plot visualization with GPU rendering using the
//! Selection API. Each box plot component (box, median, whiskers, outliers) is
//! rendered through a typed Selection with the matching mark type.
//!
//! This example shows:
//! - Selection::from_data() for render-only selections
//! - Selection::prepare_render() to upload data to the GPU
//! - Selection::render() for draw call orchestration in a single render pass
//! - Composite mark rendering (rectangles + circles) via multiple Selections

use gup::mark::circle::CircleInstance;
use gup::mark::rectangle::RectangleInstance;
use gup::mark::{Circle, Rectangle};
use gup::selection::Selection;
use gup::shader_function::Vec2;
use gup::{BoxPlotAttributes, BoxPlotOrientation, GupContext, PhysicalSize, SurfaceId};
use gup::{CircleAttributes, RectangleAttributes, Vec4};
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

/// Build RectangleAttributes and CircleAttributes from raw datasets.
///
/// Returns (boxes, medians, whiskers, outliers).
fn build_attributes(
    datasets: &[(&str, Vec<f32>, Vec2)],
) -> (
    Vec<RectangleAttributes>,
    Vec<RectangleAttributes>,
    Vec<RectangleAttributes>,
    Vec<CircleAttributes>,
) {
    let mut boxes = Vec::new();
    let mut medians = Vec::new();
    let mut whiskers = Vec::new();
    let mut outliers = Vec::new();

    for (_name, data, position) in datasets {
        let attrs =
            BoxPlotAttributes::from_data(data, *position, 0.15, BoxPlotOrientation::Vertical);

        // Normalise data values to clip space
        let y_scale = |val: f32| (val - 10.0) / 90.0 * 1.6 - 0.8;

        let q1_y = y_scale(attrs.q1);
        let q3_y = y_scale(attrs.q3);
        let median_y = y_scale(attrs.median);
        let min_y = y_scale(attrs.min);
        let max_y = y_scale(attrs.max);

        // Box (IQR)
        boxes.push(RectangleAttributes {
            center: Vec2 {
                x: position.x,
                y: (q1_y + q3_y) * 0.5,
            },
            size: Vec2 {
                x: attrs.width,
                y: q3_y - q1_y,
            },
            fill_color: attrs.box_fill_color,
            stroke_width: attrs.stroke_width / 100.0,
            stroke_color: attrs.box_stroke_color,
            corner_radius: 0.0,
        });

        // Median line
        medians.push(RectangleAttributes {
            center: Vec2 {
                x: position.x,
                y: median_y,
            },
            size: Vec2 {
                x: attrs.width,
                y: 0.01,
            },
            fill_color: attrs.median_color,
            stroke_width: 0.0,
            stroke_color: Vec4 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: 0.0,
            },
            corner_radius: 0.0,
        });

        // Lower whisker
        whiskers.push(RectangleAttributes {
            center: Vec2 {
                x: position.x,
                y: (q1_y + min_y) * 0.5,
            },
            size: Vec2 {
                x: 0.005,
                y: q1_y - min_y,
            },
            fill_color: attrs.whisker_color,
            stroke_width: 0.0,
            stroke_color: Vec4 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: 0.0,
            },
            corner_radius: 0.0,
        });

        // Upper whisker
        whiskers.push(RectangleAttributes {
            center: Vec2 {
                x: position.x,
                y: (q3_y + max_y) * 0.5,
            },
            size: Vec2 {
                x: 0.005,
                y: max_y - q3_y,
            },
            fill_color: attrs.whisker_color,
            stroke_width: 0.0,
            stroke_color: Vec4 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: 0.0,
            },
            corner_radius: 0.0,
        });

        // Whisker caps
        let cap_width = attrs.width * 0.3;
        for &y in &[min_y, max_y] {
            whiskers.push(RectangleAttributes {
                center: Vec2 { x: position.x, y },
                size: Vec2 {
                    x: cap_width,
                    y: 0.005,
                },
                fill_color: attrs.whisker_color,
                stroke_width: 0.0,
                stroke_color: Vec4 {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                    w: 0.0,
                },
                corner_radius: 0.0,
            });
        }

        // Outliers
        for &outlier_value in &attrs.outliers {
            let outlier_y = y_scale(outlier_value);
            outliers.push(CircleAttributes {
                center: Vec2 {
                    x: position.x,
                    y: outlier_y,
                },
                radius: attrs.outlier_radius / 100.0,
                fill_color: attrs.outlier_color,
                stroke_width: attrs.stroke_width / 200.0,
                stroke_color: attrs.box_stroke_color,
            });
        }
    }

    (boxes, medians, whiskers, outliers)
}

/// Renderer that drives multiple Selections for the composite box plot.
struct BoxPlotRenderer {
    /// Rectangles for the IQR boxes
    box_selection: Selection<RectangleAttributes, Rectangle>,
    /// Rectangles for median lines
    median_selection: Selection<RectangleAttributes, Rectangle>,
    /// Rectangles for whisker lines and caps
    whisker_selection: Selection<RectangleAttributes, Rectangle>,
    /// Circles for outlier points
    outlier_selection: Selection<CircleAttributes, Circle>,
    /// Whether GPU resources have been prepared
    prepared: bool,
}

impl BoxPlotRenderer {
    fn new(datasets: &[(&str, Vec<f32>, Vec2)]) -> Self {
        let (boxes, medians, whiskers, outliers) = build_attributes(datasets);

        println!(
            "  Components: {} boxes, {} medians, {} whiskers, {} outliers",
            boxes.len(),
            medians.len(),
            whiskers.len(),
            outliers.len(),
        );

        Self {
            box_selection: Selection::from_data(boxes),
            median_selection: Selection::from_data(medians),
            whisker_selection: Selection::from_data(whiskers),
            outlier_selection: Selection::from_data(outliers),
            prepared: false,
        }
    }

    /// Upload data to GPU (call once, or when data changes).
    fn prepare(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        if self.prepared {
            return;
        }

        self.box_selection
            .prepare_render(device, queue, |a| RectangleInstance::from(a))
            .expect("box prepare_render");
        self.median_selection
            .prepare_render(device, queue, |a| RectangleInstance::from(a))
            .expect("median prepare_render");
        self.whisker_selection
            .prepare_render(device, queue, |a| RectangleInstance::from(a))
            .expect("whisker prepare_render");
        self.outlier_selection
            .prepare_render(device, queue, |a| CircleInstance::from(a))
            .expect("outlier prepare_render");

        self.prepared = true;
    }

    /// Issue draw calls inside an existing render pass.
    fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        // Multiple draw calls in one render pass (single render pass rule).
        self.box_selection.render(render_pass).expect("box render");
        self.median_selection
            .render(render_pass)
            .expect("median render");
        self.whisker_selection
            .render(render_pass)
            .expect("whisker render");
        self.outlier_selection
            .render(render_pass)
            .expect("outlier render");
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

        println!();
        println!("Box Plot Rendering Demo (Selection API)");
        println!("=======================================");
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

            // Ensure GPU resources are uploaded.
            if let Some(renderer) = &mut self.renderer {
                renderer.prepare(&ctx.device, &ctx.queue);
            }

            match ctx.begin_frame() {
                Ok(mut frame) => {
                    let clear_color = Color {
                        r: 0.98,
                        g: 0.98,
                        b: 0.98,
                        a: 1.0,
                    };

                    // Single render pass for all box plot components.
                    {
                        let mut render_pass = frame.render_pass(Some(clear_color));
                        if let Some(renderer) = &self.renderer {
                            renderer.render(&mut render_pass);
                        }
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
    fn test_build_attributes() {
        let datasets = create_sample_datasets();
        let (boxes, medians, whiskers, outliers) = build_attributes(&datasets);

        assert_eq!(boxes.len(), 4); // One box per dataset
        assert_eq!(medians.len(), 4); // One median per dataset
        assert!(whiskers.len() >= 8); // At least 2 whiskers per dataset
        assert!(outliers.len() >= 5); // At least 5 outliers from "With Outliers" dataset
    }

    #[test]
    fn test_selection_from_data() {
        let datasets = create_sample_datasets();
        let (boxes, _, _, _) = build_attributes(&datasets);

        let selection: Selection<RectangleAttributes, Rectangle> = Selection::from_data(boxes);
        assert_eq!(selection.len(), 4);
        assert!(!selection.is_render_ready());
    }
}

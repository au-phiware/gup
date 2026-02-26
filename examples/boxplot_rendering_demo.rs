// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Box Plot GPU Rendering Demo — Unified BoxPlot Mark
//!
//! Demonstrates rendering complete box plots through a single
//! `Selection<BoxPlotAttributes, BoxPlot>`.  Each box plot (box, median,
//! whiskers, caps, outliers) is rendered by the GPU in one instanced draw
//! call via the unified SDF shader.
//!
//! This example shows:
//! - `BoxPlotAttributes::from_data()` to compute statistics from raw values
//! - `BoxPlotInstance::from()` for GPU-ready data conversion
//! - `Selection::from_data()` / `prepare_render()` / `render()` pipeline
//! - All four distributions rendered in a single render pass

use gup::mark::BoxPlot;
use gup::mark::boxplot::BoxPlotInstance;
use gup::selection::Selection;
use gup::shader_function::Vec2;
use gup::{BoxPlotAttributes, BoxPlotOrientation, GupContext, PhysicalSize, SurfaceId, Vec4};
use std::sync::Arc;
use wgpu::Color;
use winit::{
    application::ApplicationHandler,
    event::{ElementState, KeyEvent, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowAttributes, WindowId},
};

/// Sample datasets for different distributions.
fn create_sample_datasets() -> Vec<(&'static str, Vec<f32>, f32)> {
    vec![
        (
            "Normal",
            vec![
                42.0, 45.0, 48.0, 50.0, 52.0, 54.0, 56.0, 58.0, 60.0, 62.0, 44.0, 46.0, 48.0, 52.0,
                54.0, 56.0, 58.0, 60.0, 50.0, 52.0,
            ],
            -0.6,
        ),
        (
            "Skewed",
            vec![
                60.0, 62.0, 64.0, 66.0, 68.0, 70.0, 72.0, 75.0, 80.0, 85.0, 61.0, 63.0, 65.0, 67.0,
                69.0, 71.0, 76.0, 82.0, 88.0, 95.0,
            ],
            -0.2,
        ),
        (
            "With Outliers",
            vec![
                42.0, 44.0, 45.0, 46.0, 47.0, 48.0, 49.0, 50.0, 51.0, 52.0, 43.0, 44.0, 45.0, 46.0,
                47.0, 48.0, 49.0, 50.0, 51.0, 52.0, // Outliers
                15.0, 20.0, 75.0, 80.0, 85.0,
            ],
            0.2,
        ),
        (
            "Uniform",
            vec![
                30.0, 35.0, 40.0, 45.0, 50.0, 55.0, 60.0, 65.0, 70.0, 32.0, 33.0, 37.0, 42.0, 47.0,
                52.0, 57.0, 62.0, 67.0,
            ],
            0.6,
        ),
    ]
}

/// Build clip-space `BoxPlotAttributes` from raw datasets.
fn build_boxplot_attributes(datasets: &[(&str, Vec<f32>, f32)]) -> Vec<BoxPlotAttributes> {
    // Map a data-space value (roughly 10–100) to clip-space Y.
    let y_scale = |val: f32| (val - 10.0) / 90.0 * 1.6 - 0.8;

    let palette: &[Vec4] = &[
        Vec4 {
            x: 0.55,
            y: 0.63,
            z: 0.90,
            w: 0.85,
        }, // blue
        Vec4 {
            x: 0.60,
            y: 0.85,
            z: 0.60,
            w: 0.85,
        }, // green
        Vec4 {
            x: 0.90,
            y: 0.65,
            z: 0.55,
            w: 0.85,
        }, // orange-red
        Vec4 {
            x: 0.80,
            y: 0.70,
            z: 0.90,
            w: 0.85,
        }, // purple
    ];

    datasets
        .iter()
        .enumerate()
        .map(|(i, (_name, data, x_pos))| {
            // Compute statistics from raw data.
            let raw = BoxPlotAttributes::from_data(
                data,
                Vec2 { x: 0.0, y: 0.0 },
                0.15,
                BoxPlotOrientation::Vertical,
            );

            // Rescale to clip space.
            BoxPlotAttributes {
                position: Vec2 { x: *x_pos, y: 0.0 },
                min: y_scale(raw.min),
                q1: y_scale(raw.q1),
                median: y_scale(raw.median),
                q3: y_scale(raw.q3),
                max: y_scale(raw.max),
                outliers: raw.outliers.iter().map(|&v| y_scale(v)).collect(),
                width: 0.15,
                orientation: BoxPlotOrientation::Vertical,
                box_fill_color: palette[i % palette.len()],
                box_stroke_color: Vec4 {
                    x: 0.15,
                    y: 0.15,
                    z: 0.15,
                    w: 1.0,
                },
                median_color: Vec4 {
                    x: 0.95,
                    y: 0.2,
                    z: 0.2,
                    w: 1.0,
                },
                whisker_color: Vec4 {
                    x: 0.15,
                    y: 0.15,
                    z: 0.15,
                    w: 1.0,
                },
                outlier_color: Vec4 {
                    x: 1.0,
                    y: 0.55,
                    z: 0.1,
                    w: 1.0,
                },
                stroke_width: 0.004,
                outlier_radius: 0.015,
                notched: i % 2 == 0, // Alternate: Normal and With Outliers are notched
                notch_width: 0.5,
            }
        })
        .collect()
}

/// Renderer that drives a single `Selection<BoxPlotAttributes, BoxPlot>`.
struct BoxPlotRenderer {
    selection: Selection<BoxPlotAttributes, BoxPlot>,
    prepared: bool,
}

impl BoxPlotRenderer {
    fn new(datasets: &[(&str, Vec<f32>, f32)]) -> Self {
        let attrs = build_boxplot_attributes(datasets);
        println!(
            "  {} box plots ({} with outliers)",
            attrs.len(),
            attrs.iter().filter(|a| !a.outliers.is_empty()).count(),
        );
        Self {
            selection: Selection::from_data(attrs),
            prepared: false,
        }
    }

    /// Upload data to GPU (call once, or when data changes).
    fn prepare(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        if self.prepared {
            return;
        }
        self.selection
            .prepare_render(device, queue, |a| BoxPlotInstance::from(a), None)
            .expect("boxplot prepare_render");
        self.prepared = true;
    }

    /// Issue a single instanced draw call inside an existing render pass.
    fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        self.selection.render(render_pass).expect("boxplot render");
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
        let window_attrs = WindowAttributes::default()
            .with_title("Gup Box Plot — Unified Mark Renderer (Press ESC to quit)")
            .with_inner_size(winit::dpi::LogicalSize::new(1000, 700));
        let window = Arc::new(event_loop.create_window(window_attrs)?);

        let context = GupContext::with_surface(Arc::clone(&window)).await?;
        let surface_id = context.primary_surface_id();

        let datasets = create_sample_datasets();
        let renderer = BoxPlotRenderer::new(&datasets);

        self.window = Some(window);
        self.context = Some(context);
        self.surface_id = surface_id;
        self.renderer = Some(renderer);

        println!();
        println!("Box Plot Rendering Demo (Unified BoxPlot Mark)");
        println!("===============================================");
        println!("Displaying 4 distributions via a single Selection:");
        println!("  1. Normal (notched)   2. Skewed   3. With Outliers (notched)   4. Uniform");
        println!();
        println!("Each box plot is rendered in ONE draw call:");
        println!("  - Coloured box (IQR Q1–Q3), notched at median for odd-numbered plots");
        println!("  - Red median line");
        println!("  - Dark whiskers + caps");
        println!("  - Orange outlier circles");
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

            if let Some(renderer) = &mut self.renderer {
                renderer.prepare(&ctx.device, &ctx.queue);
            }

            match ctx.begin_frame() {
                Ok(mut frame) => {
                    let clear_color = Color {
                        r: 0.97,
                        g: 0.97,
                        b: 0.97,
                        a: 1.0,
                    };

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
    println!("=== Gup Box Plot GPU Rendering Demo (Unified Mark) ===");
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
    fn test_build_boxplot_attributes() {
        let datasets = create_sample_datasets();
        let attrs = build_boxplot_attributes(&datasets);
        assert_eq!(attrs.len(), 4);

        for a in &attrs {
            // Values should be in clip space (roughly -1..1).
            assert!(a.q1 > -1.0 && a.q1 < 1.0);
            assert!(a.q3 > -1.0 && a.q3 < 1.0);
            assert!(a.q1 < a.median);
            assert!(a.median < a.q3);
        }
    }

    #[test]
    fn test_build_boxplot_outliers_present() {
        let datasets = create_sample_datasets();
        let attrs = build_boxplot_attributes(&datasets);

        // "With Outliers" dataset should have outliers.
        let with_outliers = &attrs[2];
        assert!(
            !with_outliers.outliers.is_empty(),
            "Third dataset should contain outliers"
        );
    }

    #[test]
    fn test_selection_from_data() {
        let datasets = create_sample_datasets();
        let attrs = build_boxplot_attributes(&datasets);

        let selection: Selection<BoxPlotAttributes, BoxPlot> = Selection::from_data(attrs);
        assert_eq!(selection.len(), 4);
        assert!(!selection.is_render_ready());
    }
}

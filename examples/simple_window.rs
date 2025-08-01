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

//! Simple windowed example demonstrating basic surface rendering.
//!
//! This example creates a single window that cycles through different colors
//! to demonstrate the basic surface management and rendering capabilities.

use gup::GupContext;
use std::sync::Arc;
use wgpu::Color;
use winit::{
    application::ApplicationHandler,
    event::{ElementState, KeyEvent, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowAttributes},
};

struct SimpleWindowApp {
    window: Option<Arc<Window>>,
    context: Option<Arc<GupContext>>,
    color_index: usize,
    frame_count: u64,
}

impl SimpleWindowApp {
    fn new() -> Self {
        Self {
            window: None,
            context: None,
            color_index: 0,
            frame_count: 0,
        }
    }

    fn colors() -> [Color; 6] {
        [
            Color {
                r: 0.8,
                g: 0.2,
                b: 0.2,
                a: 1.0,
            }, // Red
            Color {
                r: 0.2,
                g: 0.8,
                b: 0.2,
                a: 1.0,
            }, // Green
            Color {
                r: 0.2,
                g: 0.2,
                b: 0.8,
                a: 1.0,
            }, // Blue
            Color {
                r: 0.8,
                g: 0.8,
                b: 0.2,
                a: 1.0,
            }, // Yellow
            Color {
                r: 0.8,
                g: 0.2,
                b: 0.8,
                a: 1.0,
            }, // Magenta
            Color {
                r: 0.2,
                g: 0.8,
                b: 0.8,
                a: 1.0,
            }, // Cyan
        ]
    }

    fn get_current_color(&self) -> Color {
        Self::colors()[self.color_index]
    }

    fn next_color(&mut self) {
        self.color_index = (self.color_index + 1) % Self::colors().len();
        println!(
            "Switched to color index: {} ({:?})",
            self.color_index,
            self.get_current_color()
        );
    }

    async fn create_context_and_window(
        &mut self,
        event_loop: &ActiveEventLoop,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Create window
        let window_attributes = WindowAttributes::default()
            .with_title("Gup Simple Window Demo - Press SPACE to change color, Q to quit")
            .with_inner_size(winit::dpi::LogicalSize::new(800, 600));

        let window = Arc::new(event_loop.create_window(window_attributes)?);

        // Create context with the window
        let context = GupContext::with_surface(Arc::clone(&window)).await?;

        self.window = Some(window);
        self.context = Some(context);

        println!("✓ Window and GPU context created successfully");
        println!("✓ Surface configured and ready for rendering");

        Ok(())
    }

    fn render_frame(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(context) = self.context.take() {
            let mut ctx = Arc::try_unwrap(context).map_err(|_| "Failed to get mutable context")?;

            match ctx.begin_frame() {
                Ok(mut frame) => {
                    // Create render pass with current color
                    let render_pass = frame.render_pass(Some(self.get_current_color()));
                    drop(render_pass); // End render pass

                    frame.finish()?;
                    self.frame_count += 1;

                    // Print stats every 60 frames
                    if self.frame_count % 60 == 0 {
                        let stats = ctx.frame_stats();
                        println!(
                            "Frame {}: {:.1} FPS, {:.2}ms avg",
                            self.frame_count,
                            stats.fps(),
                            stats.avg_frame_time
                        );
                    }
                }
                Err(e) => {
                    eprintln!("Failed to render frame: {e}");
                }
            }

            self.context = Some(Arc::new(ctx));
        }
        Ok(())
    }

    fn print_final_stats(&mut self) {
        if let Some(context) = self.context.take() {
            if let Ok(ctx) = Arc::try_unwrap(context) {
                let stats = ctx.frame_stats();
                println!("\n=== Final Statistics ===");
                println!("Total frames rendered: {}", stats.frames_rendered);
                println!("Average frame time: {:.2}ms", stats.avg_frame_time);
                println!("Final FPS: {:.1}", stats.fps());
                println!("Min frame time: {:.2}ms", stats.min_frame_time);
                println!("Max frame time: {:.2}ms", stats.max_frame_time);
                println!("GPU memory usage: {} bytes", stats.gpu_memory_usage);
                println!("App frame count: {}", self.frame_count);
            }
        }
    }
}

impl ApplicationHandler for SimpleWindowApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        println!("=== Gup Simple Window Demo ===");
        println!("Initializing GPU context and window...");

        pollster::block_on(async {
            if let Err(e) = self.create_context_and_window(event_loop).await {
                eprintln!("Failed to create context and window: {e}");
                event_loop.exit();
                return;
            }

            println!("Ready! Press SPACE to cycle colors, Q to quit");
        });
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                println!("Window close requested");
                self.print_final_stats();
                event_loop.exit();
            }

            WindowEvent::Resized(size) => {
                println!("Window resized to: {}x{}", size.width, size.height);
                if let Some(context) = self.context.take() {
                    if let Ok(mut ctx) = Arc::try_unwrap(context) {
                        let start = std::time::Instant::now();
                        if let Err(e) = ctx.resize_surface(
                            ctx.primary_surface_id().unwrap(),
                            gup::PhysicalSize::new(size.width, size.height),
                        ) {
                            eprintln!("Failed to resize surface: {e}");
                        } else {
                            let duration = start.elapsed();
                            println!(
                                "Surface resize completed in {:.2}ms",
                                duration.as_secs_f64() * 1000.0
                            );
                        }
                        self.context = Some(Arc::new(ctx));
                    }
                }
            }

            WindowEvent::RedrawRequested => {
                if let Err(e) = self.render_frame() {
                    eprintln!("Error rendering frame: {e}");
                }
            }

            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(KeyCode::KeyQ),
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } => {
                println!("Quit key pressed");
                self.print_final_stats();
                event_loop.exit();
            }

            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(KeyCode::Space),
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } => {
                self.next_color();
            }

            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        // Request redraw
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting Gup Simple Window Demo...");

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = SimpleWindowApp::new();
    event_loop.run_app(&mut app)?;

    Ok(())
}

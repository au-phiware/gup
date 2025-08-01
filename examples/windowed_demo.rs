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

//! Real windowed example demonstrating multi-surface management.
//!
//! This example creates actual windows and demonstrates:
//! - Creating multiple real windows with surfaces
//! - Rendering different colors to each window
//! - Handling window resize events
//! - Managing surface lifecycle
//! - Performance monitoring

use gup::{GupContext, PhysicalSize, SurfaceId};
use std::collections::HashMap;
use std::sync::Arc;
use wgpu::Color;
use winit::{
    application::ApplicationHandler,
    event::{ElementState, KeyEvent, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowAttributes, WindowId},
};

struct WindowInfo {
    window: Arc<Window>,
    surface_id: SurfaceId,
    clear_color: Color,
}

struct MultiWindowApp {
    context: Option<Arc<GupContext>>,
    windows: HashMap<WindowId, WindowInfo>,
    frame_count: u64,
}

impl MultiWindowApp {
    fn new() -> Self {
        Self {
            context: None,
            windows: HashMap::new(),
            frame_count: 0,
        }
    }

    async fn create_context(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.context.is_none() {
            println!("Creating GPU context...");
            let context = GupContext::headless().await?;
            self.context = Some(context);
            println!("✓ GPU context created");
        }
        Ok(())
    }

    fn add_window(
        &mut self,
        event_loop: &ActiveEventLoop,
        title: &str,
        color: Color,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let window_attributes = WindowAttributes::default()
            .with_title(title)
            .with_inner_size(winit::dpi::LogicalSize::new(800, 600));

        let window = Arc::new(event_loop.create_window(window_attributes)?);
        let window_id = window.id();
        let surface_id = SurfaceId::new();

        println!("Creating window: {title}");

        // Add surface to context
        if let Some(context) = self.context.take() {
            let mut ctx = Arc::try_unwrap(context).map_err(|_| "Failed to get mutable context")?;

            ctx.add_surface(surface_id, Arc::clone(&window))?;
            self.context = Some(Arc::new(ctx));

            println!("✓ Surface {surface_id} added for window: {title}");
        }

        let info = WindowInfo {
            window,
            surface_id,
            clear_color: color,
        };

        self.windows.insert(window_id, info);
        Ok(())
    }

    fn remove_window(&mut self, window_id: WindowId) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(info) = self.windows.remove(&window_id) {
            println!("Removing window with surface: {}", info.surface_id);

            if let Some(context) = self.context.take() {
                let mut ctx =
                    Arc::try_unwrap(context).map_err(|_| "Failed to get mutable context")?;

                ctx.remove_surface(info.surface_id)?;
                self.context = Some(Arc::new(ctx));
            }
        }
        Ok(())
    }

    fn render_frame(&mut self, window_id: WindowId) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(window_info) = self.windows.get(&window_id) {
            let surface_id = window_info.surface_id;
            let clear_color = window_info.clear_color;

            if let Some(context) = self.context.take() {
                let mut ctx =
                    Arc::try_unwrap(context).map_err(|_| "Failed to get mutable context")?;

                match ctx.begin_frame_for_surface(surface_id) {
                    Ok(mut frame) => {
                        // Create render pass with window-specific color
                        let render_pass = frame.render_pass(Some(clear_color));
                        drop(render_pass); // End render pass

                        frame.finish()?;
                        self.frame_count += 1;
                    }
                    Err(e) => {
                        eprintln!("Failed to render frame: {e}");
                    }
                }

                self.context = Some(Arc::new(ctx));
            }
        }
        Ok(())
    }

    fn handle_resize(
        &mut self,
        window_id: WindowId,
        size: winit::dpi::PhysicalSize<u32>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(window_info) = self.windows.get(&window_id) {
            let surface_id = window_info.surface_id;
            println!("Resizing window to {}x{}", size.width, size.height);

            if let Some(context) = self.context.take() {
                let mut ctx =
                    Arc::try_unwrap(context).map_err(|_| "Failed to get mutable context")?;

                let start = std::time::Instant::now();
                ctx.resize_surface(surface_id, PhysicalSize::new(size.width, size.height))?;
                let duration = start.elapsed();

                println!(
                    "Resize completed in {:.2}ms",
                    duration.as_secs_f64() * 1000.0
                );

                if duration.as_millis() > 16 {
                    println!("Warning: Resize took longer than 16ms target");
                }

                self.context = Some(Arc::new(ctx));
            }
        }
        Ok(())
    }

    fn print_stats(&mut self) {
        if let Some(context) = self.context.take() {
            if let Ok(ctx) = Arc::try_unwrap(context) {
                let stats = ctx.frame_stats();
                println!("\n=== Performance Statistics ===");
                println!("Frames rendered: {}", stats.frames_rendered);
                println!("Average frame time: {:.2}ms", stats.avg_frame_time);
                println!("Current FPS: {:.1}", stats.fps());
                println!("GPU memory usage: {} bytes", stats.gpu_memory_usage);
                println!("Active windows: {}", self.windows.len());
                println!("Total frames rendered by app: {}", self.frame_count);

                // Restore context
                self.context = Some(Arc::new(ctx));
            }
        }
    }
}

impl ApplicationHandler for MultiWindowApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        // Create context and windows when app starts
        pollster::block_on(async {
            if let Err(e) = self.create_context().await {
                eprintln!("Failed to create context: {e}");
                event_loop.exit();
                return;
            }

            // Create multiple windows with different colors
            let windows_to_create = [
                (
                    "Main Window - Blue",
                    Color {
                        r: 0.2,
                        g: 0.3,
                        b: 0.8,
                        a: 1.0,
                    },
                ),
                (
                    "Secondary Window - Red",
                    Color {
                        r: 0.8,
                        g: 0.2,
                        b: 0.3,
                        a: 1.0,
                    },
                ),
                (
                    "Tool Window - Green",
                    Color {
                        r: 0.3,
                        g: 0.8,
                        b: 0.2,
                        a: 1.0,
                    },
                ),
            ];

            for (title, color) in &windows_to_create {
                if let Err(e) = self.add_window(event_loop, title, *color) {
                    eprintln!("Failed to create window '{title}': {e}");
                }
            }

            println!("\n=== Multi-Window Demo Started ===");
            println!("Created {} windows", self.windows.len());
            println!("Press 'q' to quit, 's' to show stats");
        });
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                println!("Close requested for window");
                if let Err(e) = self.remove_window(window_id) {
                    eprintln!("Error removing window: {e}");
                }

                // Exit if no windows left
                if self.windows.is_empty() {
                    println!("All windows closed, exiting...");
                    self.print_stats();
                    event_loop.exit();
                }
            }

            WindowEvent::Resized(size) => {
                if let Err(e) = self.handle_resize(window_id, size) {
                    eprintln!("Error handling resize: {e}");
                }
            }

            WindowEvent::RedrawRequested => {
                if let Err(e) = self.render_frame(window_id) {
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
                println!("Quit key pressed, exiting...");
                self.print_stats();
                event_loop.exit();
            }

            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(KeyCode::KeyS),
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } => {
                self.print_stats();
            }

            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        // Request redraws for all windows
        for window_info in self.windows.values() {
            window_info.window.request_redraw();
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Gup Multi-Window Windowed Demo ===");
    println!("This will create actual windows on your screen!");
    println!("Controls:");
    println!("  - Close any window to remove it");
    println!("  - Press 'q' to quit");
    println!("  - Press 's' to show performance stats");
    println!("  - Resize windows to test performance");
    println!();

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = MultiWindowApp::new();
    event_loop.run_app(&mut app)?;

    Ok(())
}

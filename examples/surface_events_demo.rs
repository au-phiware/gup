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

//! Surface event handling demonstration.
//!
//! This example shows how to handle surface events such as DPI changes,
//! window focus, and visibility changes in real-time.

use gup::{GupContext, GupResult, PhysicalSize, SurfaceEventHandler, SurfaceId};
use std::sync::Arc;
use wgpu::Color;
use winit::{
    application::ApplicationHandler,
    event::{ElementState, KeyEvent, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowAttributes},
};

/// Event handler that logs all surface events.
struct LoggingEventHandler;

impl SurfaceEventHandler for LoggingEventHandler {
    fn on_dpi_changed(&mut self, surface_id: SurfaceId, scale_factor: f64) -> GupResult<()> {
        println!(
            "📐 DPI Changed - Surface: {}, Scale Factor: {:.2}",
            surface_id, scale_factor
        );
        Ok(())
    }

    fn on_focus_changed(&mut self, surface_id: SurfaceId, focused: bool) -> GupResult<()> {
        let status = if focused { "FOCUSED" } else { "UNFOCUSED" };
        println!(
            "👁️  Focus Changed - Surface: {}, Status: {}",
            surface_id, status
        );
        Ok(())
    }

    fn on_visibility_changed(&mut self, surface_id: SurfaceId, visible: bool) -> GupResult<()> {
        let status = if visible { "VISIBLE" } else { "HIDDEN" };
        println!(
            "👁️  Visibility Changed - Surface: {}, Status: {}",
            surface_id, status
        );
        Ok(())
    }

    fn on_resized(&mut self, surface_id: SurfaceId, width: u32, height: u32) -> GupResult<()> {
        println!(
            "📏 Resized - Surface: {}, Size: {}x{}",
            surface_id, width, height
        );
        Ok(())
    }
}

struct SurfaceEventsApp {
    window: Option<Arc<Window>>,
    context: Option<Arc<GupContext>>,
    surface_id: Option<SurfaceId>,
    current_scale_factor: f64,
    is_focused: bool,
    is_visible: bool,
    frame_count: u64,
}

impl SurfaceEventsApp {
    fn new() -> Self {
        Self {
            window: None,
            context: None,
            surface_id: None,
            current_scale_factor: 1.0,
            is_focused: false,
            is_visible: true,
            frame_count: 0,
        }
    }

    async fn create_context_and_window(
        &mut self,
        event_loop: &ActiveEventLoop,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Create window
        let window_attributes = WindowAttributes::default()
            .with_title("Gup Surface Events Demo - Try resizing, minimizing, or changing DPI")
            .with_inner_size(winit::dpi::LogicalSize::new(800, 600));

        let window = Arc::new(event_loop.create_window(window_attributes)?);
        self.current_scale_factor = window.scale_factor();

        // Create context with the window
        let mut ctx = GupContext::with_surface(Arc::clone(&window)).await?;

        // Register event handler
        let ctx_mut = Arc::get_mut(&mut ctx).expect("Failed to get mutable context");
        ctx_mut.register_event_handler(Box::new(LoggingEventHandler));

        // Enable background throttling
        ctx_mut.set_background_throttling(true);

        // Get the surface ID
        self.surface_id = ctx_mut.primary_surface_id();

        self.window = Some(window);
        self.context = Some(ctx);

        println!("✓ Window and GPU context created successfully");
        println!("✓ Event handler registered");
        println!("✓ Background throttling enabled");
        println!("✓ Initial scale factor: {:.2}", self.current_scale_factor);
        println!("\n📋 Try the following actions:");
        println!("  - Resize the window");
        println!("  - Move the window to a different monitor (DPI change)");
        println!("  - Minimize/restore the window");
        println!("  - Click in/out of the window (focus change)");
        println!("  - Press Q to quit\n");

        Ok(())
    }

    fn get_render_color(&self) -> Color {
        // Change color based on state
        if !self.is_visible {
            // Gray when hidden
            Color {
                r: 0.2,
                g: 0.2,
                b: 0.2,
                a: 1.0,
            }
        } else if !self.is_focused {
            // Blue when unfocused
            Color {
                r: 0.2,
                g: 0.4,
                b: 0.8,
                a: 1.0,
            }
        } else {
            // Green when focused
            Color {
                r: 0.2,
                g: 0.8,
                b: 0.2,
                a: 1.0,
            }
        }
    }

    fn render_frame(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(context) = self.context.take() {
            let mut ctx = Arc::try_unwrap(context).map_err(|_| "Failed to get mutable context")?;

            // Check if background throttling should prevent rendering
            if ctx.is_background_throttling_enabled()
                && !self.is_visible
                && !self.frame_count.is_multiple_of(60)
            {
                // Skip most frames when hidden
                self.context = Some(Arc::new(ctx));
                return Ok(());
            }

            match ctx.begin_frame() {
                Ok(mut frame) => {
                    // Create render pass with state-based color
                    let render_pass = frame.render_pass(Some(self.get_render_color()));
                    drop(render_pass); // End render pass

                    frame.finish()?;
                    self.frame_count += 1;

                    // Print stats periodically
                    if self.frame_count.is_multiple_of(300) {
                        let stats = ctx.frame_stats();
                        println!(
                            "📊 Frame {}: {:.1} FPS (focused: {}, visible: {}, scale: {:.2})",
                            self.frame_count,
                            stats.fps(),
                            self.is_focused,
                            self.is_visible,
                            self.current_scale_factor
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
}

impl ApplicationHandler for SurfaceEventsApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        println!("=== Gup Surface Events Demo ===");
        println!("Initializing GPU context and event system...");

        pollster::block_on(async {
            if let Err(e) = self.create_context_and_window(event_loop).await {
                eprintln!("Failed to create context and window: {e}");
                event_loop.exit();
            }
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
                println!("\n👋 Closing application");
                event_loop.exit();
            }

            WindowEvent::Resized(size) => {
                if let Some(context) = self.context.take()
                    && let Ok(mut ctx) = Arc::try_unwrap(context)
                    && let Some(surface_id) = self.surface_id
                {
                    if let Err(e) =
                        ctx.resize_surface(surface_id, PhysicalSize::new(size.width, size.height))
                    {
                        eprintln!("Failed to resize surface: {e}");
                    }
                    self.context = Some(Arc::new(ctx));
                }
            }

            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                self.current_scale_factor = scale_factor;

                if let Some(context) = self.context.take()
                    && let Ok(mut ctx) = Arc::try_unwrap(context)
                    && let Some(surface_id) = self.surface_id
                {
                    if let Err(e) = ctx.update_surface_scale_factor(surface_id, scale_factor) {
                        eprintln!("Failed to update scale factor: {e}");
                    }
                    self.context = Some(Arc::new(ctx));
                }
            }

            WindowEvent::Focused(focused) => {
                self.is_focused = focused;

                if let Some(context) = self.context.take()
                    && let Ok(mut ctx) = Arc::try_unwrap(context)
                    && let Some(surface_id) = self.surface_id
                {
                    if let Err(e) = ctx.set_surface_focus(surface_id, focused) {
                        eprintln!("Failed to set surface focus: {e}");
                    }
                    self.context = Some(Arc::new(ctx));
                }
            }

            WindowEvent::Occluded(occluded) => {
                self.is_visible = !occluded;

                if let Some(context) = self.context.take()
                    && let Ok(mut ctx) = Arc::try_unwrap(context)
                    && let Some(surface_id) = self.surface_id
                {
                    if let Err(e) = ctx.set_surface_visibility(surface_id, !occluded) {
                        eprintln!("Failed to set surface visibility: {e}");
                    }
                    self.context = Some(Arc::new(ctx));
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
                println!("\n👋 Quit key pressed");
                event_loop.exit();
            }

            _ => {}
        }

        // Request redraw
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        event_loop.set_control_flow(ControlFlow::Poll);
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = SurfaceEventsApp::new();
    event_loop.run_app(&mut app)?;

    Ok(())
}

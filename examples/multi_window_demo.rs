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

//! Demonstration of multi-window surface management capabilities.
//!
//! This example shows how to:
//! - Create and manage multiple surfaces in a single GupContext
//! - Handle window resize events and surface updates
//! - Switch between fullscreen and windowed modes
//! - Manage DPI scaling changes
//! - Render to different surfaces independently

use gup::{GupContext, PhysicalSize, SurfaceId};
use std::collections::HashMap;
use std::sync::Arc;
use wgpu::Color;

// Mock application window for demonstration
#[derive(Debug)]
#[allow(dead_code)]
struct AppWindow {
    id: String,
    width: u32,
    height: u32,
    scale_factor: f64,
    is_focused: bool,
}

impl AppWindow {
    fn new(id: String, width: u32, height: u32) -> Arc<Self> {
        Arc::new(Self {
            id,
            width,
            height,
            scale_factor: 1.0,
            is_focused: true,
        })
    }
}

// Mock window handle implementation for testing
impl raw_window_handle::HasWindowHandle for AppWindow {
    fn window_handle(
        &self,
    ) -> Result<raw_window_handle::WindowHandle<'_>, raw_window_handle::HandleError> {
        use raw_window_handle::{RawWindowHandle, WebWindowHandle, WindowHandle};
        let handle = RawWindowHandle::Web(WebWindowHandle::new(0));
        Ok(unsafe { WindowHandle::borrow_raw(handle) })
    }
}

impl raw_window_handle::HasDisplayHandle for AppWindow {
    fn display_handle(
        &self,
    ) -> Result<raw_window_handle::DisplayHandle<'_>, raw_window_handle::HandleError> {
        use raw_window_handle::{DisplayHandle, RawDisplayHandle, WebDisplayHandle};
        let handle = RawDisplayHandle::Web(WebDisplayHandle::new());
        Ok(unsafe { DisplayHandle::borrow_raw(handle) })
    }
}

struct MultiWindowApp {
    context: Arc<GupContext>,
    windows: HashMap<SurfaceId, Arc<AppWindow>>,
    surface_colors: HashMap<SurfaceId, Color>,
}

impl MultiWindowApp {
    async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let context = GupContext::headless().await?;

        Ok(Self {
            context,
            windows: HashMap::new(),
            surface_colors: HashMap::new(),
        })
    }

    fn add_window(
        &mut self,
        window_id: String,
        width: u32,
        height: u32,
        color: Color,
    ) -> Result<SurfaceId, Box<dyn std::error::Error>> {
        let window = AppWindow::new(window_id.clone(), width, height);
        let surface_id = SurfaceId::new();

        // In a real application, this would create an actual window
        println!("Creating window '{window_id}' with size {width}x{height}");

        // Store window reference
        self.windows.insert(surface_id, window.clone());
        self.surface_colors.insert(surface_id, color);

        // Note: In headless mode, add_surface will fail
        // In a real windowed application, you would:
        if let Ok(mut ctx) = Arc::try_unwrap(Arc::clone(&self.context)) {
            match ctx.add_surface(surface_id, window) {
                Ok(_) => {
                    println!("Successfully added surface {surface_id} for window '{window_id}'");
                    self.context = Arc::new(ctx);
                }
                Err(e) => {
                    println!("Failed to add surface (expected in headless mode): {e}");
                    // Remove from our tracking
                    self.windows.remove(&surface_id);
                    self.surface_colors.remove(&surface_id);
                    return Err(e.into());
                }
            }
        }

        Ok(surface_id)
    }

    fn remove_window(&mut self, surface_id: SurfaceId) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(window) = self.windows.remove(&surface_id) {
            println!("Removing window '{}'", window.id);

            if let Ok(mut ctx) = Arc::try_unwrap(Arc::clone(&self.context)) {
                ctx.remove_surface(surface_id)?;
                self.context = Arc::new(ctx);
            }

            self.surface_colors.remove(&surface_id);
        }

        Ok(())
    }

    fn resize_window(
        &mut self,
        surface_id: SurfaceId,
        width: u32,
        height: u32,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(window) = self.windows.get(&surface_id) {
            println!("Resizing window '{}' to {}x{}", window.id, width, height);

            if let Ok(mut ctx) = Arc::try_unwrap(Arc::clone(&self.context)) {
                let start = std::time::Instant::now();
                ctx.resize_surface(surface_id, PhysicalSize::new(width, height))?;
                let duration = start.elapsed();

                println!(
                    "Resize completed in {:.2}ms",
                    duration.as_secs_f64() * 1000.0
                );

                // Verify performance requirement
                if duration.as_millis() > 16 {
                    println!("Warning: Resize took longer than 16ms target");
                }

                self.context = Arc::new(ctx);
            }
        }

        Ok(())
    }

    fn set_fullscreen(
        &mut self,
        surface_id: SurfaceId,
        fullscreen: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(window) = self.windows.get(&surface_id) {
            let mode = if fullscreen { "fullscreen" } else { "windowed" };
            println!("Setting window '{}' to {} mode", window.id, mode);

            if let Ok(mut ctx) = Arc::try_unwrap(Arc::clone(&self.context)) {
                ctx.set_fullscreen(surface_id, fullscreen)?;
                self.context = Arc::new(ctx);
            }
        }

        Ok(())
    }

    fn update_scale_factor(
        &mut self,
        surface_id: SurfaceId,
        scale_factor: f64,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(window) = self.windows.get(&surface_id) {
            println!(
                "Updating scale factor for window '{}' to {:.1}x",
                window.id, scale_factor
            );

            if let Ok(mut ctx) = Arc::try_unwrap(Arc::clone(&self.context)) {
                ctx.update_surface_scale_factor(surface_id, scale_factor)?;
                self.context = Arc::new(ctx);
            }
        }

        Ok(())
    }

    fn render_frame(&mut self, surface_id: SurfaceId) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(window) = self.windows.get(&surface_id) {
            let color = self
                .surface_colors
                .get(&surface_id)
                .copied()
                .unwrap_or(Color::BLACK);

            if let Ok(mut ctx) = Arc::try_unwrap(Arc::clone(&self.context)) {
                match ctx.begin_frame_for_surface(surface_id) {
                    Ok(mut frame) => {
                        // Create render pass with window-specific color
                        let render_pass = frame.render_pass(Some(color));
                        drop(render_pass); // End render pass

                        frame.finish()?;
                        println!("Rendered frame for window '{}'", window.id);
                    }
                    Err(e) => {
                        println!("Failed to render frame for window '{}': {}", window.id, e);
                    }
                }
                self.context = Arc::new(ctx);
            }
        }

        Ok(())
    }

    fn render_all_frames(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let surface_ids: Vec<SurfaceId> = self.windows.keys().copied().collect();

        for surface_id in surface_ids {
            self.render_frame(surface_id)?;
        }

        Ok(())
    }

    fn print_context_info(&mut self) {
        if let Ok(ctx) = Arc::try_unwrap(Arc::clone(&self.context)) {
            println!("\n=== Context Information ===");
            println!("Active surfaces: {}", ctx.surface_ids().len());
            println!("Primary surface: {:?}", ctx.primary_surface_id());

            for surface_id in ctx.surface_ids() {
                if let Some(window) = self.windows.get(&surface_id) {
                    println!(
                        "Surface {}: window '{}' ({}x{})",
                        surface_id, window.id, window.width, window.height
                    );

                    if let Some(format) = ctx.surface_format_for(surface_id) {
                        println!("  Format: {format:?}");
                    }
                    if let Some(size) = ctx.surface_size(surface_id) {
                        println!("  Size: {}x{}", size.width, size.height);
                    }
                    println!("  Fullscreen: {}", ctx.is_fullscreen(surface_id));
                    if let Some(scale) = ctx.surface_scale_factor(surface_id) {
                        println!("  Scale factor: {scale:.1}x");
                    }
                }
            }

            let stats = ctx.frame_stats();
            println!("Frames rendered: {}", stats.frames_rendered);
            println!("Average frame time: {:.2}ms", stats.avg_frame_time);
            println!("Current FPS: {:.1}", stats.fps());

            // Restore context
            self.context = Arc::new(ctx);
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Gup Multi-Window Surface Management Demo ===\n");

    let mut app = MultiWindowApp::new().await?;

    // Demonstrate multi-window capabilities
    println!("1. Creating multiple windows...");

    // Note: These will fail in headless mode, but demonstrate the API
    let results = vec![
        app.add_window(
            "Main Window".to_string(),
            800,
            600,
            Color {
                r: 0.2,
                g: 0.3,
                b: 0.8,
                a: 1.0,
            },
        ),
        app.add_window(
            "Secondary Window".to_string(),
            640,
            480,
            Color {
                r: 0.8,
                g: 0.3,
                b: 0.2,
                a: 1.0,
            },
        ),
        app.add_window(
            "Tool Window".to_string(),
            300,
            200,
            Color {
                r: 0.3,
                g: 0.8,
                b: 0.2,
                a: 1.0,
            },
        ),
    ];

    let mut surface_ids = Vec::new();
    for (i, result) in results.into_iter().enumerate() {
        match result {
            Ok(id) => {
                surface_ids.push(id);
                println!("  ✓ Window {} created with surface {}", i + 1, id);
            }
            Err(e) => {
                println!(
                    "  ✗ Window {} creation failed (expected in headless): {}",
                    i + 1,
                    e
                );
            }
        }
    }

    if surface_ids.is_empty() {
        println!("\nNote: Running in headless mode - demonstrating API without actual windows");

        // Show API usage patterns instead
        println!("\n2. Demonstrating API usage patterns...");

        // Show SurfaceId creation and properties
        let demo_id = SurfaceId::new();
        println!("Sample Surface ID: {demo_id}");
        println!("Raw ID value: {}", demo_id.raw());

        // Show PhysicalSize usage
        let size = PhysicalSize::new(1920u32, 1080u32);
        println!("Sample size: {}x{}", size.width, size.height);

        // Show context info
        app.print_context_info();

        println!("\n=== Demo completed (headless mode) ===");
        return Ok(());
    }

    app.print_context_info();

    // Demonstrate window operations
    println!("\n2. Testing window operations...");

    if let Some(&first_id) = surface_ids.first() {
        // Test resize
        println!("  Resizing first window...");
        app.resize_window(first_id, 1024, 768)?;

        // Test fullscreen toggle
        println!("  Setting fullscreen mode...");
        app.set_fullscreen(first_id, true)?;
        std::thread::sleep(std::time::Duration::from_millis(100));

        println!("  Returning to windowed mode...");
        app.set_fullscreen(first_id, false)?;

        // Test scale factor update
        println!("  Updating scale factor...");
        app.update_scale_factor(first_id, 2.0)?;
    }

    // Demonstrate rendering to multiple surfaces
    println!("\n3. Rendering frames to all windows...");
    for i in 0..5 {
        println!("  Rendering frame {}/5...", i + 1);
        app.render_all_frames()?;
        std::thread::sleep(std::time::Duration::from_millis(16)); // ~60 FPS
    }

    app.print_context_info();

    // Test performance - multiple rapid resizes
    println!("\n4. Performance testing...");
    if let Some(&first_id) = surface_ids.first() {
        let sizes = [
            (800, 600),
            (1024, 768),
            (1280, 720),
            (1920, 1080),
            (800, 600), // Back to original
        ];

        println!(
            "  Testing resize performance with {} operations...",
            sizes.len()
        );
        let start = std::time::Instant::now();

        for (width, height) in &sizes {
            app.resize_window(first_id, *width, *height)?;
        }

        let total_duration = start.elapsed();
        let avg_duration = total_duration / sizes.len() as u32;

        println!(
            "  Total time: {:.2}ms",
            total_duration.as_secs_f64() * 1000.0
        );
        println!(
            "  Average per resize: {:.2}ms",
            avg_duration.as_secs_f64() * 1000.0
        );

        if avg_duration.as_millis() < 16 {
            println!("  ✓ Performance target met (<16ms per resize)");
        } else {
            println!("  ⚠ Performance target missed (≥16ms per resize)");
        }
    }

    // Demonstrate surface management
    println!("\n5. Testing surface management...");

    // Remove middle window
    if surface_ids.len() > 1 {
        let middle_id = surface_ids[1];
        println!("  Removing middle window...");
        app.remove_window(middle_id)?;
        surface_ids.remove(1);
    }

    app.print_context_info();

    // Clean up remaining windows
    println!("\n6. Cleaning up...");
    for surface_id in surface_ids {
        app.remove_window(surface_id)?;
    }

    app.print_context_info();

    println!("\n=== Multi-Window Demo completed successfully! ===");

    // Show final context statistics
    if let Ok(ctx) = Arc::try_unwrap(app.context) {
        let stats = ctx.frame_stats();
        println!("\nFinal Statistics:");
        println!("  Total frames rendered: {}", stats.frames_rendered);
        println!("  Average frame time: {:.2}ms", stats.avg_frame_time);
        println!("  Final FPS: {:.1}", stats.fps());
        println!("  Min frame time: {:.2}ms", stats.min_frame_time);
        println!("  Max frame time: {:.2}ms", stats.max_frame_time);
        println!("  GPU memory usage: {} bytes", stats.gpu_memory_usage);
    }

    Ok(())
}

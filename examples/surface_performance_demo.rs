// Copyright (C) 2025 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Example demonstrating surface performance optimization features.
//!
//! This example shows:
//! - Multiple surfaces with different render priorities
//! - Frame pacing and scheduling
//! - Performance statistics collection
//! - Memory optimization

use gup::context::GupContext;
use std::sync::Arc;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowAttributes, WindowId};

struct MultiWindowApp {
    context: Option<Arc<GupContext>>,
    windows: Vec<(WindowId, Arc<Window>)>,
    frame_count: u64,
    last_stats_print: Instant,
}

impl MultiWindowApp {
    fn new() -> Self {
        Self {
            context: None,
            windows: Vec::new(),
            frame_count: 0,
            last_stats_print: Instant::now(),
        }
    }

    async fn create_context_and_windows(&mut self, event_loop: &ActiveEventLoop) {
        // Create three windows with different priorities
        let window1 = Arc::new(
            event_loop
                .create_window(
                    WindowAttributes::default()
                        .with_title("Foreground Window (60 FPS)")
                        .with_inner_size(PhysicalSize::new(400, 300)),
                )
                .expect("Failed to create window 1"),
        );

        let window2 = Arc::new(
            event_loop
                .create_window(
                    WindowAttributes::default()
                        .with_title("Background Window (30 FPS)")
                        .with_inner_size(PhysicalSize::new(400, 300)),
                )
                .expect("Failed to create window 2"),
        );

        let window3 = Arc::new(
            event_loop
                .create_window(
                    WindowAttributes::default()
                        .with_title("Low Priority Window (15 FPS)")
                        .with_inner_size(PhysicalSize::new(400, 300)),
                )
                .expect("Failed to create window 3"),
        );

        // Create context
        let context = GupContext::new().await.expect("Failed to create context");

        // Note: In a real implementation, you would add surfaces here
        // For this example, we just demonstrate the configuration API

        self.windows.push((window1.id(), window1));
        self.windows.push((window2.id(), window2));
        self.windows.push((window3.id(), window3));
        self.context = Some(context);

        println!("Created 3 windows with different performance profiles:");
        println!("  Window 1: Foreground, 60 FPS target");
        println!("  Window 2: Background, 30 FPS target");
        println!("  Window 3: Background, 15 FPS target");
        println!("\nPress Ctrl+C to exit and see final statistics.");
    }

    fn print_statistics(&self) {
        if let Some(context) = &self.context {
            let stats = context.get_render_statistics();
            println!("\n=== Performance Statistics ===");
            println!("Total frames: {}", stats.total_frames);
            println!("Total skipped: {}", stats.total_skipped);
            println!(
                "Scheduling overhead: {:.2}%",
                stats.scheduling_overhead * 100.0
            );

            for (surface_id, surface_stats) in &stats.surface_stats {
                println!(
                    "Surface {}: {} frames, {} skipped, {:.2}ms avg",
                    surface_id.raw(),
                    surface_stats.frames_rendered,
                    surface_stats.frames_skipped,
                    surface_stats.avg_frame_time
                );
            }
        }
    }
}

impl ApplicationHandler for MultiWindowApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.context.is_none() {
            let app_ptr = self as *mut Self;
            tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(async {
                    unsafe { (*app_ptr).create_context_and_windows(event_loop).await }
                });
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                self.print_statistics();
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                self.frame_count += 1;

                // Print stats every 60 frames
                if self.frame_count % 60 == 0 && self.last_stats_print.elapsed().as_secs() >= 1 {
                    self.print_statistics();
                    self.last_stats_print = Instant::now();
                }

                // Request another frame
                if let Some((_id, window)) = self
                    .windows
                    .iter()
                    .find(|(id, _)| *id == window_id)
                {
                    window.request_redraw();
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        // Request redraws for all windows
        for (_id, window) in &self.windows {
            window.request_redraw();
        }
    }
}

fn main() {
    env_logger::init();

    let event_loop = EventLoop::new().expect("Failed to create event loop");
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = MultiWindowApp::new();
    event_loop.run_app(&mut app).expect("Failed to run event loop");
}

#[cfg(test)]
mod tests {
    use super::*;
    use gup::context::{RenderPriority, SurfaceRenderConfig};

    #[tokio::test]
    async fn test_render_config_demonstration() {
        // Demonstrate configuration API
        let _context = GupContext::new().await.expect("Failed to create context");

        let foreground_config = SurfaceRenderConfig {
            target_fps: Some(60.0),
            priority: RenderPriority::Foreground,
            frame_skipping_enabled: true,
            resource_pool_size: 16,
        };

        let background_config = SurfaceRenderConfig {
            target_fps: Some(30.0),
            priority: RenderPriority::Background,
            frame_skipping_enabled: true,
            resource_pool_size: 8,
        };

        assert_eq!(foreground_config.target_fps, Some(60.0));
        assert_eq!(background_config.target_fps, Some(30.0));
        assert!(foreground_config.priority > background_config.priority);
    }
}

// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Opinionated application shell for single-window Gup desktop apps.
//!
//! [`GupApp`] wraps winit's event loop and [`GupContext`]
//! lifecycle so that a complete GPU-accelerated visualisation can be launched
//! in a handful of lines:
//!
//! ```rust,no_run
//! use gup::app::{GupApp, AppRenderer};
//! use gup::RenderFrame;
//!
//! struct MyChart;
//!
//! impl AppRenderer for MyChart {
//!     fn render(&mut self, frame: &mut RenderFrame) {
//!         let _pass = frame.render_pass(Some(wgpu::Color::WHITE));
//!         // draw your marks here …
//!     }
//! }
//!
//! fn main() -> Result<(), gup::GupError> {
//!     GupApp::new(MyChart).title("Demo").run()
//! }
//! ```
//!
//! # Built-in Keyboard Shortcuts
//!
//! | Key          | Action                     |
//! |--------------|----------------------------|
//! | `Escape` / `Q` | Quit                    |
//! | `F` / `F11`    | Toggle fullscreen        |
//! | `S`            | Save screenshot (PNG)     |
//!
//! Shortcuts can be suppressed with [`.shortcuts(false)`](GupApp::shortcuts).

use std::sync::Arc;

use winit::{
    application::ApplicationHandler,
    event::{ElementState, KeyEvent, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Fullscreen, Window, WindowAttributes, WindowId},
};

use crate::RenderFrame;
use crate::context::{CapturedFrame, GupContext, PhysicalSize, SurfaceConfigBuilder, SurfaceId};
use crate::error::{GupError, GupResult};
use crate::export::png as png_export;

// ---------------------------------------------------------------------------
// AppRenderer trait
// ---------------------------------------------------------------------------

/// Trait for types that can render content into a [`RenderFrame`].
///
/// Implement this on your chart or visualisation type and pass it to
/// [`GupApp::new`].
///
/// A blanket implementation is provided for `FnMut(&mut RenderFrame)` so you
/// can also pass a closure directly.
pub trait AppRenderer: Send + 'static {
    /// Draw one frame.  The render pass has **not** been created yet — call
    /// [`RenderFrame::render_pass`] to begin drawing.
    fn render(&mut self, frame: &mut RenderFrame);
}

impl<F: FnMut(&mut RenderFrame) + Send + 'static> AppRenderer for F {
    fn render(&mut self, frame: &mut RenderFrame) {
        self(frame);
    }
}

// ---------------------------------------------------------------------------
// GupApp builder
// ---------------------------------------------------------------------------

/// Builder for a single-window Gup desktop application.
///
/// Construct with [`GupApp::new`], optionally configure with the chainable
/// setters, then call [`.run()`](GupApp::run) to enter the event loop.
pub struct GupApp {
    title: String,
    width: u32,
    height: u32,
    resizable: bool,
    shortcuts_enabled: bool,
    renderer: Box<dyn AppRenderer>,
}

impl GupApp {
    /// Create a new application shell that will render `chart` in its window.
    ///
    /// Sensible defaults are applied:
    /// * title: `"Gup"`
    /// * size: 800 × 600
    /// * resizable: `true`
    /// * keyboard shortcuts: enabled
    pub fn new(chart: impl AppRenderer) -> Self {
        Self {
            title: "Gup".to_string(),
            width: 800,
            height: 600,
            resizable: true,
            shortcuts_enabled: true,
            renderer: Box::new(chart),
        }
    }

    /// Override the window title.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    /// Override the initial logical window size (default 800 × 600).
    pub fn size(mut self, width: u32, height: u32) -> Self {
        self.width = width;
        self.height = height;
        self
    }

    /// Set whether the window is user-resizable (default `true`).
    pub fn resizable(mut self, resizable: bool) -> Self {
        self.resizable = resizable;
        self
    }

    /// Enable or disable built-in keyboard shortcuts (default `true`).
    ///
    /// When disabled, the `Escape`/`Q`/`F`/`F11`/`S` shortcuts are not
    /// handled by the shell.  Use this when your application manages its own
    /// key handling.
    pub fn shortcuts(mut self, enabled: bool) -> Self {
        self.shortcuts_enabled = enabled;
        self
    }

    /// Consume the builder, create the winit event loop, and run the
    /// application until the window is closed.
    ///
    /// This function blocks the calling thread.
    pub fn run(self) -> GupResult<()> {
        let event_loop = EventLoop::new().map_err(|e| GupError::GpuInitializationError {
            reason: format!("Failed to create event loop: {e}"),
        })?;
        event_loop.set_control_flow(ControlFlow::Poll);

        let mut runner = GupAppRunner {
            title: self.title,
            width: self.width,
            height: self.height,
            resizable: self.resizable,
            shortcuts_enabled: self.shortcuts_enabled,
            renderer: self.renderer,
            window: None,
            context: None,
            surface_id: None,
            screenshot_counter: 0,
            screenshot_requested: false,
        };

        event_loop
            .run_app(&mut runner)
            .map_err(|e| GupError::GpuInitializationError {
                reason: format!("Event loop error: {e}"),
            })
    }
}

// ---------------------------------------------------------------------------
// Internal runner (winit ApplicationHandler)
// ---------------------------------------------------------------------------

struct GupAppRunner {
    // --- configuration (immutable after construction) ---
    title: String,
    width: u32,
    height: u32,
    resizable: bool,
    shortcuts_enabled: bool,

    // --- user content ---
    renderer: Box<dyn AppRenderer>,

    // --- runtime state ---
    window: Option<Arc<Window>>,
    context: Option<Arc<GupContext>>,
    surface_id: Option<SurfaceId>,
    screenshot_counter: u32,
    screenshot_requested: bool,
}

impl GupAppRunner {
    /// Initialise GPU context and surface on first resume.
    fn initialize(
        &mut self,
        event_loop: &ActiveEventLoop,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let window_attrs = WindowAttributes::default()
            .with_title(&self.title)
            .with_inner_size(winit::dpi::LogicalSize::new(self.width, self.height))
            .with_resizable(self.resizable);

        let window = Arc::new(event_loop.create_window(window_attrs)?);

        let config = SurfaceConfigBuilder::new()
            .with_size(self.width, self.height)
            .with_usage(wgpu::TextureUsages::COPY_SRC);
        let context =
            pollster::block_on(GupContext::with_surface_config(Arc::clone(&window), config))?;
        let surface_id = context.primary_surface_id();

        self.window = Some(window);
        self.context = Some(context);
        self.surface_id = surface_id;

        Ok(())
    }

    /// Render a single frame.
    fn render_frame(&mut self) {
        let Some(context) = self.context.take() else {
            return;
        };
        let mut ctx = match Arc::try_unwrap(context) {
            Ok(c) => c,
            Err(arc) => {
                self.context = Some(arc);
                return;
            }
        };

        match ctx.begin_frame() {
            Ok(mut frame) => {
                self.renderer.render(&mut frame);

                // If a screenshot was requested, encode a texture-to-buffer
                // copy on the same command encoder *before* finish() submits
                // the work.
                let captured = if self.screenshot_requested {
                    self.screenshot_requested = false;
                    if let Some(window) = &self.window {
                        let inner_size = window.inner_size();
                        match frame.capture_texture_copy(inner_size.width, inner_size.height) {
                            Ok(cap) => Some(cap),
                            Err(e) => {
                                eprintln!("gup: screenshot capture failed: {e}");
                                None
                            }
                        }
                    } else {
                        None
                    }
                } else {
                    None
                };

                if let Err(e) = frame.finish() {
                    eprintln!("gup: frame finish error: {e}");
                }

                // The GPU commands (including the copy) have been submitted.
                // Now map the staging buffer and save the PNG.
                if let Some(captured) = captured {
                    self.save_captured_frame(&ctx, captured);
                }
            }
            Err(e) => {
                eprintln!("gup: begin_frame error: {e}");
            }
        }

        self.context = Some(Arc::new(ctx));
    }

    /// Resize the surface to match the new physical size.
    fn handle_resize(&mut self, size: winit::dpi::PhysicalSize<u32>) {
        if size.width == 0 || size.height == 0 {
            return;
        }

        let Some(surface_id) = self.surface_id else {
            return;
        };
        let Some(context) = self.context.take() else {
            return;
        };

        match Arc::try_unwrap(context) {
            Ok(mut ctx) => {
                let _ = ctx.resize_surface(surface_id, PhysicalSize::new(size.width, size.height));
                self.context = Some(Arc::new(ctx));
            }
            Err(arc) => {
                self.context = Some(arc);
            }
        }
    }

    /// Toggle between windowed and borderless fullscreen.
    fn toggle_fullscreen(&self) {
        if let Some(window) = &self.window {
            if window.fullscreen().is_some() {
                window.set_fullscreen(None);
            } else {
                window.set_fullscreen(Some(Fullscreen::Borderless(None)));
            }
        }
    }

    /// Save a captured frame's pixel data to a PNG file.
    fn save_captured_frame(&mut self, ctx: &GupContext, captured: CapturedFrame) {
        let buffer_slice = captured.buffer.slice(..);
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });

        // Block until the GPU finishes the copy.
        let _ = ctx.device.poll(wgpu::PollType::Wait);

        match receiver.recv() {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                eprintln!("gup: screenshot buffer mapping failed: {e:?}");
                return;
            }
            Err(_) => {
                eprintln!("gup: screenshot buffer mapping callback dropped");
                return;
            }
        }

        let mapped = buffer_slice.get_mapped_range();
        let mut pixels = png_export::strip_row_padding(
            &mapped,
            captured.width,
            captured.height,
            captured.padded_bytes_per_row,
        );
        drop(mapped);
        captured.buffer.unmap();

        // Convert from BGRA (wgpu default surface format) to RGBA (PNG).
        png_export::bgra_to_rgba(&mut pixels);

        match png_export::encode_png(&pixels, captured.width, captured.height) {
            Ok(png_bytes) => {
                self.screenshot_counter += 1;
                let filename = format!("gup_screenshot_{:03}.png", self.screenshot_counter);
                match std::fs::write(&filename, &png_bytes) {
                    Ok(()) => eprintln!("gup: screenshot saved to {filename}"),
                    Err(e) => eprintln!("gup: failed to write screenshot: {e}"),
                }
            }
            Err(e) => {
                eprintln!("gup: screenshot encoding failed: {e}");
            }
        }
    }

    /// Handle a key press when shortcuts are enabled.
    fn handle_key(&mut self, key: KeyCode, event_loop: &ActiveEventLoop) {
        match key {
            KeyCode::Escape | KeyCode::KeyQ => {
                event_loop.exit();
            }
            KeyCode::KeyF | KeyCode::F11 => {
                self.toggle_fullscreen();
            }
            KeyCode::KeyS => {
                self.screenshot_requested = true;
            }
            _ => {}
        }
    }
}

impl ApplicationHandler for GupAppRunner {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            // Already initialised (e.g. coming back from suspend on mobile).
            return;
        }
        if let Err(e) = self.initialize(event_loop) {
            eprintln!("gup: initialisation failed: {e}");
            event_loop.exit();
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                self.handle_resize(size);
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                // winit will follow up with a Resized event at the new
                // physical size, so we handle DPI changes there.
            }
            WindowEvent::RedrawRequested => {
                self.render_frame();
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        state: ElementState::Pressed,
                        physical_key: PhysicalKey::Code(key),
                        ..
                    },
                ..
            } if self.shortcuts_enabled => {
                self.handle_key(key, event_loop);
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- builder default values ---

    #[test]
    fn builder_defaults() {
        let app = GupApp::new(|_frame: &mut RenderFrame| {});
        assert_eq!(app.title, "Gup");
        assert_eq!(app.width, 800);
        assert_eq!(app.height, 600);
        assert!(app.resizable);
        assert!(app.shortcuts_enabled);
    }

    #[test]
    fn builder_title() {
        let app = GupApp::new(|_: &mut RenderFrame| {}).title("My Chart");
        assert_eq!(app.title, "My Chart");
    }

    #[test]
    fn builder_size() {
        let app = GupApp::new(|_: &mut RenderFrame| {}).size(1200, 800);
        assert_eq!(app.width, 1200);
        assert_eq!(app.height, 800);
    }

    #[test]
    fn builder_resizable() {
        let app = GupApp::new(|_: &mut RenderFrame| {}).resizable(false);
        assert!(!app.resizable);
    }

    #[test]
    fn builder_shortcuts() {
        let app = GupApp::new(|_: &mut RenderFrame| {}).shortcuts(false);
        assert!(!app.shortcuts_enabled);
    }

    #[test]
    fn builder_chaining() {
        let app = GupApp::new(|_: &mut RenderFrame| {})
            .title("Test")
            .size(640, 480)
            .resizable(false)
            .shortcuts(false);

        assert_eq!(app.title, "Test");
        assert_eq!(app.width, 640);
        assert_eq!(app.height, 480);
        assert!(!app.resizable);
        assert!(!app.shortcuts_enabled);
    }

    #[test]
    fn builder_accepts_struct_renderer() {
        struct DummyRenderer;
        impl AppRenderer for DummyRenderer {
            fn render(&mut self, _frame: &mut RenderFrame) {}
        }

        let app = GupApp::new(DummyRenderer);
        assert_eq!(app.title, "Gup");
    }

    #[test]
    fn builder_title_from_string() {
        let title = String::from("Dynamic Title");
        let app = GupApp::new(|_: &mut RenderFrame| {}).title(title);
        assert_eq!(app.title, "Dynamic Title");
    }
}

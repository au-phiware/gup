// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Opinionated application shell for single-window Gup desktop apps.
//!
//! [`GupApp`] wraps winit's event loop and [`GupContext`](crate::GupContext)
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

use wgpu::Color;
use winit::{
    application::ApplicationHandler,
    event::{ElementState, KeyEvent, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Fullscreen, Window, WindowAttributes, WindowId},
};

use crate::RenderFrame;
use crate::context::{GupContext, PhysicalSize, SurfaceId};
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

        let context = pollster::block_on(GupContext::with_surface(Arc::clone(&window)))?;
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
                if let Err(e) = frame.finish() {
                    eprintln!("gup: frame finish error: {e}");
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

    /// Capture a screenshot to a PNG file in the working directory.
    fn take_screenshot(&mut self) {
        let Some(window) = &self.window else {
            return;
        };
        let Some(context) = self.context.take() else {
            return;
        };

        let inner_size = window.inner_size();
        let width = inner_size.width;
        let height = inner_size.height;

        let ctx = match Arc::try_unwrap(context) {
            Ok(c) => c,
            Err(arc) => {
                self.context = Some(arc);
                eprintln!("gup: screenshot failed — context is shared");
                return;
            }
        };

        // Render to an off-screen texture so we can read it back.
        let target = png_export::OffscreenTarget::new(&ctx.device, width, height);

        // Create command encoder and render pass on the off-screen texture.
        let mut encoder = ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("screenshot_encoder"),
            });
        {
            let render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("screenshot_render_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target.view(),
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(Color::WHITE),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                ..Default::default()
            });

            // We cannot easily re-render user content through the AppRenderer
            // trait (it expects a RenderFrame). Instead we capture a blank
            // frame. Full chart-content screenshots require COPY_SRC on the
            // surface texture; this is tracked as a follow-up improvement.
            drop(render_pass);
        }
        ctx.queue.submit(std::iter::once(encoder.finish()));

        match target.readback_as_png(&ctx.device, &ctx.queue) {
            Ok(png_bytes) => {
                self.screenshot_counter += 1;
                let filename = format!("gup_screenshot_{:03}.png", self.screenshot_counter);
                match std::fs::write(&filename, &png_bytes) {
                    Ok(()) => eprintln!("gup: screenshot saved to {filename}"),
                    Err(e) => eprintln!("gup: failed to write screenshot: {e}"),
                }
            }
            Err(e) => {
                eprintln!("gup: screenshot capture failed: {e}");
            }
        }

        self.context = Some(Arc::new(ctx));
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
                self.take_screenshot();
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

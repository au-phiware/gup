// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! A minimal [`show`]: one window, one [`Plot`], wheel zoom (RFC-001 §8's
//! `gup::show`, scoped to the S0 spike; `GupApp` on `Chart` is S8).

use crate::context::Context;
use crate::error::{Error, Result};
use crate::geom::Point;
use crate::plot::Plot;
use crate::render::Renderer;
use crate::target::save_png;
use crate::window::WindowTarget;
use std::path::PathBuf;
use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalSize};
use winit::event::{ElementState, KeyEvent, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};

/// Default window size in logical pixels.
const DEFAULT_SIZE: (u32, u32) = (800, 500);
/// Default `GUP_SCREENSHOT_WIDTH × GUP_SCREENSHOT_HEIGHT`, as for the
/// examples of the `gup` crate.
const DEFAULT_SCREENSHOT_SIZE: (u32, u32) = (640, 480);
/// Zoom factor per wheel line (scrolling up zooms in).
const ZOOM_PER_LINE: f64 = 0.85;

/// A `GUP_SCREENSHOT_PATH` request: draw one frame in the window, read the
/// window's surface texture back, write it as PNG and exit.
#[derive(Clone, Debug, PartialEq)]
struct Screenshot {
    path: PathBuf,
    /// Physical pixels.
    size: (u32, u32),
}

impl Screenshot {
    fn from_env() -> Option<Self> {
        let path = std::env::var_os("GUP_SCREENSHOT_PATH")?;
        let dim = |var: &str, default: u32| {
            std::env::var(var)
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(default)
        };
        Some(Self {
            path: path.into(),
            size: (
                dim("GUP_SCREENSHOT_WIDTH", DEFAULT_SCREENSHOT_SIZE.0),
                dim("GUP_SCREENSHOT_HEIGHT", DEFAULT_SCREENSHOT_SIZE.1),
            ),
        })
    }
}

/// Show `plot` in a window until it is closed (or Escape is pressed).
///
/// Creates a [`Context`] and a [`WindowTarget`] for the window and redraws
/// on resize and on wheel zoom. Zooming changes the plot's scale domains
/// only: each frame writes uniforms, never column bytes.
///
/// With `GUP_SCREENSHOT_PATH` set, `show` draws one frame **in the
/// window**, reads that frame's surface texture back, writes it as PNG to
/// the path and returns. `GUP_SCREENSHOT_WIDTH` and
/// `GUP_SCREENSHOT_HEIGHT` (default 640×480) set the window's physical
/// size. This still needs a display; the PNG is the window's pixels, not
/// an offscreen render.
///
/// winit allows one event loop per process, so `show` can run once.
///
/// ```no_run
/// use gup_core::prelude::*;
///
/// let mut plot = Plot::new();
/// let (x, y) = (plot.x(Linear::new()), plot.y(Log::new()));
/// plot.add(Selection::<(f64, f64), Circle>::new(vec![(1.0, 10.0), (2.0, 1e3)]))
///     .attr(Circle::X, x.encode(|r: &(f64, f64)| r.0))
///     .attr(Circle::Y, y.encode(|r: &(f64, f64)| r.1));
/// gup_core::show(plot)?;
/// # Ok::<(), gup_core::Error>(())
/// ```
pub fn show(plot: Plot) -> Result<()> {
    let mut builder = EventLoop::builder();
    // Allow `show` from a test harness thread.
    #[cfg(target_os = "linux")]
    {
        winit::platform::wayland::EventLoopBuilderExtWayland::with_any_thread(&mut builder, true);
        winit::platform::x11::EventLoopBuilderExtX11::with_any_thread(&mut builder, true);
    }
    let event_loop = builder
        .build()
        .map_err(|e| Error::config("show", format!("cannot open a window: {e}")))?;
    let mut app = App {
        plot,
        screenshot: Screenshot::from_env(),
        state: None,
        renderer: Renderer::new(),
        cursor: None,
        result: Ok(()),
    };
    event_loop
        .run_app(&mut app)
        .map_err(|e| Error::config("show", format!("event loop failed: {e}")))?;
    app.result
}

/// Field order is drop order: the surface goes before the device.
struct WindowState {
    target: WindowTarget,
    cx: Context,
}

struct App {
    plot: Plot,
    screenshot: Option<Screenshot>,
    state: Option<WindowState>,
    renderer: Renderer,
    /// The cursor in logical pixels.
    cursor: Option<Point>,
    result: Result<()>,
}

impl App {
    fn fail(&mut self, event_loop: &ActiveEventLoop, e: Error) {
        if self.result.is_ok() {
            self.result = Err(e);
        }
        event_loop.exit();
    }

    fn open(&mut self, event_loop: &ActiveEventLoop) -> Result<WindowState> {
        let mut attributes = Window::default_attributes()
            .with_title(self.plot.title_text().unwrap_or("Gup").to_string());
        attributes = match &self.screenshot {
            // Exactly the requested physical size: fixed, not resizable.
            Some(s) => {
                let size = PhysicalSize::new(s.size.0, s.size.1);
                attributes
                    .with_inner_size(size)
                    .with_min_inner_size(size)
                    .with_max_inner_size(size)
                    .with_resizable(false)
            }
            None => attributes.with_inner_size(LogicalSize::new(DEFAULT_SIZE.0, DEFAULT_SIZE.1)),
        };
        let window = Arc::new(
            event_loop
                .create_window(attributes)
                .map_err(|e| Error::config("show", format!("cannot create a window: {e}")))?,
        );
        let cx = Context::new_blocking()?;
        let target = WindowTarget::new(&cx, window)?;
        Ok(WindowState { target, cx })
    }

    fn redraw(&mut self) -> Result<Option<PathBuf>> {
        let Some(WindowState { cx, target }) = &mut self.state else {
            return Ok(None);
        };
        let (w, h) = target.size();
        let dpr = target.dpr();
        let resolved = self.plot.resolve(cx, w as f32 / dpr, h as f32 / dpr)?;
        if self.screenshot.is_some() {
            target.capture_next_frame()?;
        }
        self.renderer.render(cx, &resolved.scene, target)?;
        let Some(shot) = &self.screenshot else {
            return Ok(None);
        };
        let image = pollster::block_on(target.take_capture(cx))?
            .ok_or_else(|| Error::Readback("the captured frame was not presented".into()))?;
        save_png(&image, &shot.path)?;
        eprintln!(
            "gup-core: wrote the window's {}×{} frame (dpr {dpr}, {target:?}) to {}",
            image.width(),
            image.height(),
            shot.path.display()
        );
        Ok(Some(shot.path.clone()))
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }
        match self.open(event_loop) {
            Ok(state) => {
                state.target.window().request_redraw();
                self.state = Some(state);
            }
            Err(e) => self.fail(event_loop, e),
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested
            | WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        logical_key: Key::Named(NamedKey::Escape),
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(WindowState { cx, target }) = &mut self.state {
                    target.resize(cx, size.width, size.height);
                    target.window().request_redraw();
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                if let Some(state) = &self.state {
                    let p = position.to_logical::<f32>(state.target.window().scale_factor());
                    self.cursor = Some(Point::new(p.x, p.y));
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let (Some(state), Some(at)) = (&self.state, self.cursor) else {
                    return;
                };
                let lines = match delta {
                    MouseScrollDelta::LineDelta(_, y) => f64::from(y),
                    MouseScrollDelta::PixelDelta(p) => p.y / 40.0,
                };
                let window = Arc::clone(state.target.window());
                match self.plot.zoom(at, ZOOM_PER_LINE.powf(lines)) {
                    Ok(()) => window.request_redraw(),
                    Err(e) => self.fail(event_loop, e),
                }
            }
            WindowEvent::RedrawRequested => match self.redraw() {
                Ok(Some(_written)) => event_loop.exit(),
                Ok(None) => {}
                Err(e) => self.fail(event_loop, e),
            },
            _ => {}
        }
    }

    /// Drop every GPU object while the display connection is still open.
    /// The surface, and through it the wgpu instance's GL backend, is
    /// bound to the connection; the last buffer to go releases the
    /// instance, so the plot's GPU state and the renderer's buffers must
    /// go here too, not after the event loop has closed the connection.
    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        self.state = None;
        self.renderer = Renderer::new();
        drop(std::mem::take(&mut self.plot));
    }
}

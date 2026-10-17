// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! [`GupApp`] and [`show`]: one window showing one [`Chart`] (RFC-001 §10(b)).

use crate::chart::Chart;
use crate::context::Context;
use crate::error::{Error, Result};
use crate::geom::Point;
use crate::input::{Button, Input};
use crate::render::Renderer;
use crate::target::save_png;
use crate::window::WindowTarget;
use std::path::PathBuf;
use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalSize};
use winit::error::EventLoopError;
use winit::event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowId};

/// Default window size in logical pixels.
const DEFAULT_SIZE: (u32, u32) = (800, 500);
/// Default `GUP_SCREENSHOT_WIDTH × GUP_SCREENSHOT_HEIGHT`, as for the
/// examples of the `gup` crate.
const DEFAULT_SCREENSHOT_SIZE: (u32, u32) = (640, 480);

/// A `GUP_SCREENSHOT_PATH` request: draw one frame in the window, read the
/// window's surface texture back, write it as PNG and exit.
#[derive(Clone, Debug, PartialEq)]
struct Screenshot {
    path: PathBuf,
    /// Physical pixels.
    size: (u32, u32),
    /// `GUP_SCREENSHOT_POINTER=x,y`, in logical pixels.
    pointer: Option<Point>,
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
        let pointer = std::env::var("GUP_SCREENSHOT_POINTER").ok().and_then(|v| {
            let (x, y) = v.split_once(',')?;
            Some(Point::new(x.trim().parse().ok()?, y.trim().parse().ok()?))
        });
        Some(Self {
            path: path.into(),
            pointer,
            size: (
                dim("GUP_SCREENSHOT_WIDTH", DEFAULT_SCREENSHOT_SIZE.0),
                dim("GUP_SCREENSHOT_HEIGHT", DEFAULT_SCREENSHOT_SIZE.1),
            ),
        })
    }
}

/// A window showing one chart until it is closed or Escape is pressed.
///
/// Creates a [`Context`] and a [`WindowTarget`] for the window, forwards
/// pointer input (move, leave, buttons, wheel) to [`Chart::handle`] and
/// redraws when the chart asks.
///
/// With `GUP_SCREENSHOT_PATH` set, it draws one frame **in the window**,
/// reads that frame's surface texture back, writes it as PNG to the path
/// and returns. `GUP_SCREENSHOT_WIDTH` and `GUP_SCREENSHOT_HEIGHT`
/// (default 640×480) set the window's physical size, and
/// `GUP_SCREENSHOT_POINTER=x,y` (logical pixels) moves the pointer there
/// before the captured frame. This still needs a display.
///
/// winit allows one event loop per process, so [`GupApp::run`] can run once.
pub struct GupApp {
    chart: Option<Box<dyn Chart>>,
    title: Option<String>,
    size: (u32, u32),
}

impl GupApp {
    /// An app showing `chart`.
    pub fn new(chart: impl Chart + 'static) -> Self {
        Self {
            chart: Some(Box::new(chart)),
            title: None,
            size: DEFAULT_SIZE,
        }
    }

    /// Window title (default: the chart's title, else "Gup").
    #[must_use]
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Inner size in logical pixels (default 800 × 500).
    #[must_use]
    pub fn size(mut self, width: u32, height: u32) -> Self {
        self.size = (width, height);
        self
    }

    /// Run the event loop. Once per process: a second call returns `Err`.
    pub fn run(self) -> Result<()> {
        let mut builder = EventLoop::builder();
        // Allow running from a test harness thread.
        #[cfg(target_os = "linux")]
        {
            winit::platform::wayland::EventLoopBuilderExtWayland::with_any_thread(
                &mut builder,
                true,
            );
            winit::platform::x11::EventLoopBuilderExtX11::with_any_thread(&mut builder, true);
        }
        let event_loop = builder.build().map_err(|e| match e {
            EventLoopError::RecreationAttempt => Error::config(
                "show",
                "winit allows one event loop per process; show or GupApp::run has already run",
            ),
            e => Error::config("show", format!("cannot open a window: {e}")),
        })?;
        let mut app = App {
            chart: self.chart,
            title: self.title,
            size: self.size,
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
}

/// `GupApp::new(chart).run()`.
pub fn show(chart: impl Chart + 'static) -> Result<()> {
    GupApp::new(chart).run()
}

/// Map a window event to a chart input. `cursor` is the last logical
/// pointer position; `scale` is the window's scale factor.
fn input_from(event: &WindowEvent, cursor: Option<Point>, scale: f64) -> Option<Input> {
    match event {
        WindowEvent::CursorMoved { position, .. } => {
            let p = position.to_logical::<f32>(scale);
            Some(Input::PointerMoved {
                at: Point::new(p.x, p.y),
            })
        }
        WindowEvent::CursorLeft { .. } => Some(Input::PointerLeft),
        WindowEvent::MouseInput { state, button, .. } => {
            let button = match button {
                MouseButton::Left => Button::Primary,
                MouseButton::Right => Button::Secondary,
                MouseButton::Middle => Button::Middle,
                _ => return None,
            };
            let at = cursor?;
            Some(match state {
                ElementState::Pressed => Input::PointerDown { at, button },
                ElementState::Released => Input::PointerUp { at, button },
            })
        }
        WindowEvent::MouseWheel { delta, .. } => {
            let lines = match delta {
                MouseScrollDelta::LineDelta(_, y) => *y,
                MouseScrollDelta::PixelDelta(p) => (p.y / 40.0) as f32,
            };
            Some(Input::Wheel { at: cursor?, lines })
        }
        _ => None,
    }
}

/// Field order is drop order: the surface goes before the device.
struct WindowState {
    target: WindowTarget,
    cx: Context,
}

struct App {
    chart: Option<Box<dyn Chart>>,
    title: Option<String>,
    size: (u32, u32),
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
        let title = self
            .title
            .clone()
            .or_else(|| {
                let chart = self.chart.as_ref()?;
                chart.title_text().map(str::to_string)
            })
            .unwrap_or_else(|| "Gup".to_string());
        let mut attributes = Window::default_attributes().with_title(title);
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
            None => attributes.with_inner_size(LogicalSize::new(self.size.0, self.size.1)),
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
        let (lw, lh) = (w as f32 / dpr, h as f32 / dpr);
        let chart = self
            .chart
            .as_mut()
            .ok_or_else(|| Error::config("show", "the chart has been dropped"))?;
        if let Some(at) = self.screenshot.as_ref().and_then(|s| s.pointer) {
            chart.resolve(cx, lw, lh)?;
            let _ = chart.handle(Input::PointerMoved { at });
        }
        let resolved = chart.resolve(cx, lw, lh)?;
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
                    match target.resize(cx, size.width, size.height) {
                        Ok(()) => target.window().request_redraw(),
                        Err(e) => self.fail(event_loop, e),
                    }
                }
            }
            ref e @ (WindowEvent::CursorMoved { .. }
            | WindowEvent::CursorLeft { .. }
            | WindowEvent::MouseInput { .. }
            | WindowEvent::MouseWheel { .. }) => {
                let (Some(state), Some(chart)) = (&self.state, self.chart.as_mut()) else {
                    return;
                };
                let window = Arc::clone(state.target.window());
                let Some(input) = input_from(e, self.cursor, window.scale_factor()) else {
                    return;
                };
                match input {
                    Input::PointerMoved { at } => self.cursor = Some(at),
                    Input::PointerLeft => self.cursor = None,
                    _ => {}
                }
                if chart.handle(input).redraw {
                    window.request_redraw();
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
    /// instance, so the chart's GPU state and the renderer's buffers must
    /// go here too, not after the event loop has closed the connection.
    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        self.state = None;
        self.renderer = Renderer::new();
        drop(self.chart.take());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use winit::dpi::PhysicalPosition;
    use winit::event::{DeviceId, TouchPhase};

    fn dev() -> DeviceId {
        DeviceId::dummy()
    }

    fn click(state: ElementState, button: MouseButton) -> WindowEvent {
        WindowEvent::MouseInput {
            device_id: dev(),
            state,
            button,
        }
    }

    fn wheel(delta: MouseScrollDelta) -> WindowEvent {
        WindowEvent::MouseWheel {
            device_id: dev(),
            delta,
            phase: TouchPhase::Moved,
        }
    }

    #[test]
    fn input_from_maps_pointer_events() {
        let at = Point::new(3.0, 4.0);
        let moved = WindowEvent::CursorMoved {
            device_id: dev(),
            position: PhysicalPosition::new(20.0, 10.0),
        };
        assert_eq!(
            input_from(&moved, None, 2.0),
            Some(Input::PointerMoved {
                at: Point::new(10.0, 5.0)
            })
        );
        let left = WindowEvent::CursorLeft { device_id: dev() };
        assert_eq!(input_from(&left, Some(at), 1.0), Some(Input::PointerLeft));
        for (b, button) in [
            (MouseButton::Left, Button::Primary),
            (MouseButton::Right, Button::Secondary),
            (MouseButton::Middle, Button::Middle),
        ] {
            assert_eq!(
                input_from(&click(ElementState::Pressed, b), Some(at), 1.0),
                Some(Input::PointerDown { at, button })
            );
            assert_eq!(
                input_from(&click(ElementState::Released, b), Some(at), 1.0),
                Some(Input::PointerUp { at, button })
            );
        }
        assert_eq!(
            input_from(
                &click(ElementState::Pressed, MouseButton::Back),
                Some(at),
                1.0
            ),
            None
        );
        assert_eq!(
            input_from(&click(ElementState::Pressed, MouseButton::Left), None, 1.0),
            None
        );
    }

    #[test]
    fn input_from_maps_the_wheel() {
        let at = Point::new(3.0, 4.0);
        assert_eq!(
            input_from(&wheel(MouseScrollDelta::LineDelta(0.0, 2.0)), Some(at), 1.0),
            Some(Input::Wheel { at, lines: 2.0 })
        );
        assert_eq!(
            input_from(
                &wheel(MouseScrollDelta::PixelDelta(PhysicalPosition::new(
                    0.0, 80.0
                ))),
                Some(at),
                1.0
            ),
            Some(Input::Wheel { at, lines: 2.0 })
        );
        assert_eq!(
            input_from(&wheel(MouseScrollDelta::LineDelta(0.0, 1.0)), None, 1.0),
            None
        );
    }
}

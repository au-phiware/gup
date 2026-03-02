// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Zoom and pan demo — GPU viewport transform (GUP-277).
//!
//! Renders 500 000 coloured circles and attaches a [`ZoomBehavior`] so
//! the user can navigate the dataset smoothly at 60 FPS. The geometry
//! buffers are never rebuilt during navigation — only a 16-byte uniform
//! (`GpuViewportTransform`) is uploaded each frame.
//!
//! # Controls
//!
//! - **Mouse wheel**: Zoom in/out (anchored at cursor)
//! - **Left-click drag**: Pan
//! - **R**: Reset zoom/pan to identity
//! - **Q** / **Escape**: Quit

use gup::mark::circle::{Circle, CircleInstance};
use gup::selection::Selection;
use gup::zoom::ZoomBehavior;
use gup::{GupContext, PipelineCache};
use std::sync::Arc;
use std::time::Instant;
use wgpu::Color;
use winit::{
    application::ApplicationHandler,
    event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowAttributes, WindowId},
};

// ---------------------------------------------------------------------------
// Data generation
// ---------------------------------------------------------------------------

const NUM_POINTS: usize = 500_000;

/// A single data point in the scatter plot.
#[derive(Clone)]
struct Point {
    x: f32,
    y: f32,
    radius: f32,
    color: [f32; 4],
}

/// Generate `NUM_POINTS` pseudo-random points in the range [-0.9, 0.9].
fn generate_data() -> Vec<Point> {
    let mut data = Vec::with_capacity(NUM_POINTS);
    // Simple deterministic PRNG (xorshift32) for reproducibility.
    let mut rng: u32 = 0xDEAD_BEEF;
    let mut next = || -> f32 {
        rng ^= rng << 13;
        rng ^= rng >> 17;
        rng ^= rng << 5;
        (rng as f32) / (u32::MAX as f32)
    };

    let palette: [[f32; 4]; 8] = [
        [0.92, 0.26, 0.21, 0.6],
        [0.13, 0.59, 0.95, 0.6],
        [0.30, 0.69, 0.31, 0.6],
        [1.00, 0.76, 0.03, 0.6],
        [0.61, 0.15, 0.69, 0.6],
        [0.00, 0.74, 0.83, 0.6],
        [1.00, 0.34, 0.13, 0.6],
        [0.47, 0.33, 0.28, 0.6],
    ];

    for i in 0..NUM_POINTS {
        let x = next() * 1.8 - 0.9;
        let y = next() * 1.8 - 0.9;
        let radius = 0.001 + next() * 0.002;
        let color = palette[i % palette.len()];
        data.push(Point {
            x,
            y,
            radius,
            color,
        });
    }
    data
}

// ---------------------------------------------------------------------------
// Application state
// ---------------------------------------------------------------------------

struct App {
    window: Option<Arc<Window>>,
    context: Option<Arc<GupContext>>,
    data: Vec<Point>,
    selection: Option<Selection<Point, Circle>>,
    cache: PipelineCache,
    zoom: ZoomBehavior,

    win_size: [f32; 2],
    mouse_clip: [f64; 2],
    needs_redraw: bool,
    frame_count: u64,
    fps_timer: Instant,
}

impl App {
    fn new() -> Self {
        let data = generate_data();
        Self {
            window: None,
            context: None,
            data,
            selection: None,
            cache: PipelineCache::new(),
            zoom: ZoomBehavior::new()
                .scale_extent(0.1, 100.0)
                .inertia_decay(0.85),
            win_size: [800.0, 600.0],
            mouse_clip: [0.0; 2],
            needs_redraw: true,
            frame_count: 0,
            fps_timer: Instant::now(),
        }
    }

    /// Convert screen pixel coordinates to clip space [-1, 1].
    fn screen_to_clip(&self, px: f64, py: f64) -> [f64; 2] {
        [
            (px / self.win_size[0] as f64) * 2.0 - 1.0,
            -((py / self.win_size[1] as f64) * 2.0 - 1.0),
        ]
    }

    fn render(&mut self) {
        let Some(ctx) = self.context.take() else {
            return;
        };
        let mut ctx = match Arc::try_unwrap(ctx) {
            Ok(c) => c,
            Err(arc) => {
                self.context = Some(arc);
                return;
            }
        };

        if let Some(sel) = &mut self.selection {
            // Only prepare instances once (they never change).
            if !sel.is_render_ready() {
                let _ = sel.prepare_render(
                    &ctx.device,
                    &ctx.queue,
                    |p: &Point| CircleInstance {
                        center: [p.x, p.y],
                        radius: p.radius,
                        _pad0: 0.0,
                        fill_color: p.color,
                        stroke_width: 0.0,
                        _pad1: [0.0; 3],
                        stroke_color: [0.0; 4],
                    },
                    Some(&mut self.cache),
                    None,
                );
            }

            // Upload the current viewport transform every frame.
            let transform = self.zoom.gpu_transform();
            sel.set_viewport_transform(&ctx.queue, &transform);
        }

        match ctx.begin_frame() {
            Ok(mut frame) => {
                let bg = Color {
                    r: 0.02,
                    g: 0.02,
                    b: 0.04,
                    a: 1.0,
                };
                {
                    let mut rp = frame.render_pass(Some(bg));
                    if let Some(s) = &self.selection {
                        let _ = s.render(&mut rp);
                    }
                }
                let _ = frame.finish();
            }
            Err(e) => eprintln!("frame: {e}"),
        }
        self.context = Some(Arc::new(ctx));

        // FPS counter — update window title every second.
        self.frame_count += 1;
        let elapsed = self.fps_timer.elapsed().as_secs_f64();
        if elapsed >= 1.0 {
            let fps = self.frame_count as f64 / elapsed;
            let scale = self.zoom.scale();
            if let Some(w) = &self.window {
                w.set_title(&format!(
                    "GUP-277 Zoom/Pan — {NUM_POINTS} pts | {fps:.0} FPS | {scale:.1}×"
                ));
            }
            self.frame_count = 0;
            self.fps_timer = Instant::now();
        }
    }
}

// ---------------------------------------------------------------------------
// winit ApplicationHandler
// ---------------------------------------------------------------------------

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        pollster::block_on(async {
            let attrs = WindowAttributes::default()
                .with_title(format!("GUP-277 Zoom/Pan — {NUM_POINTS} points"))
                .with_inner_size(winit::dpi::LogicalSize::new(800, 600));
            let window = match event_loop.create_window(attrs) {
                Ok(w) => Arc::new(w),
                Err(e) => {
                    eprintln!("window: {e}");
                    event_loop.exit();
                    return;
                }
            };
            match GupContext::with_surface(Arc::clone(&window)).await {
                Ok(ctx) => {
                    self.context = Some(ctx);
                    self.window = Some(window);
                    self.selection = Some(Selection::<Point, Circle>::from_data(self.data.clone()));
                    println!("✓ Ready — scroll to zoom, drag to pan, R to reset, Q to quit");
                }
                Err(e) => {
                    eprintln!("GPU init: {e}");
                    event_loop.exit();
                }
            }
        });
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _wid: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),

            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(code),
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } => match code {
                KeyCode::KeyQ | KeyCode::Escape => event_loop.exit(),
                KeyCode::KeyR => {
                    self.zoom.reset();
                    self.needs_redraw = true;
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                }
                _ => {}
            },

            WindowEvent::Resized(size) => {
                self.win_size = [size.width as f32, size.height as f32];
                if let Some(context) = self.context.take()
                    && let Ok(mut ctx) = Arc::try_unwrap(context)
                {
                    if let Some(id) = ctx.primary_surface_id() {
                        let _ =
                            ctx.resize_surface(id, gup::PhysicalSize::new(size.width, size.height));
                    }
                    self.context = Some(Arc::new(ctx));
                }
            }

            WindowEvent::CursorMoved { position, .. } => {
                self.mouse_clip = self.screen_to_clip(position.x, position.y);
            }

            WindowEvent::MouseWheel { delta, .. } => {
                let dy = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y as f64 * 30.0,
                    MouseScrollDelta::PixelDelta(pos) => pos.y,
                };
                self.zoom
                    .on_wheel(dy, self.mouse_clip[0], self.mouse_clip[1]);
                self.needs_redraw = true;
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }

            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                self.zoom
                    .on_drag_start(self.mouse_clip[0], self.mouse_clip[1]);
            }

            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            } => {
                self.zoom.on_drag_end();
                if self.zoom.is_animating()
                    && let Some(w) = &self.window {
                        w.request_redraw();
                    }
            }

            WindowEvent::RedrawRequested => {
                // Update drag position if dragging.
                if self.zoom.is_dragging() {
                    self.zoom
                        .on_drag_move(self.mouse_clip[0], self.mouse_clip[1]);
                }
                // Advance inertia.
                let inertia_moved = self.zoom.tick();

                self.render();
                self.needs_redraw = false;

                // Request another frame if animating.
                if (inertia_moved || self.zoom.is_dragging())
                    && let Some(w) = &self.window {
                        w.request_redraw();
                    }
            }

            _ => {}
        }

        // While dragging, request continuous redraws.
        if self.zoom.is_dragging()
            && let Some(w) = &self.window {
                w.request_redraw();
            }
    }
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

fn main() {
    println!("GUP-277 Zoom and Pan Demo");
    println!("  Scroll wheel to zoom (anchored at cursor)");
    println!("  Left-click drag to pan");
    println!("  R to reset view");
    println!("  Q or Escape to quit");
    println!("  Rendering {NUM_POINTS} points");

    let event_loop = EventLoop::new().expect("event loop");
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = App::new();
    let _ = event_loop.run_app(&mut app);
}

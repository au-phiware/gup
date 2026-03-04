// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Brush selection demo — rectangular drag-to-select (GUP-278).
//!
//! Renders 1 000 coloured circles and attaches a [`BrushBehavior`] so
//! the user can drag a rectangle to select data points. Selected IDs
//! are printed to stdout on release.
//!
//! # Controls
//!
//! - **Left-click drag**: Draw a brush rectangle
//! - **Q** / **Escape**: Quit

use gup::brush::{BrushBehavior, BrushEvent, BrushOverlayRenderer, BrushStyle};
use gup::event::ViewportTransform;
use gup::interaction::Vec2;
use gup::mark::circle::{Circle, CircleInstance};
use gup::mark_selection::MarkSelectionSystem;
use gup::selection::Selection;
use gup::{GupContext, PipelineCache};
use std::sync::Arc;
use std::time::Instant;
use wgpu::Color;
use winit::{
    application::ApplicationHandler,
    event::{ElementState, KeyEvent, MouseButton, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowAttributes, WindowId},
};

// ---------------------------------------------------------------------------
// Data generation
// ---------------------------------------------------------------------------

const NUM_POINTS: usize = 1_000;

/// A single data point in the scatter plot.
#[derive(Clone)]
struct Point {
    x: f32,
    y: f32,
    radius: f32,
    color: [f32; 4],
}

/// Generate `NUM_POINTS` pseudo-random points in the range [-0.8, 0.8].
fn generate_data() -> Vec<Point> {
    let mut data = Vec::with_capacity(NUM_POINTS);
    let mut rng: u32 = 0xCAFE_BABE;
    let mut next = || -> f32 {
        rng ^= rng << 13;
        rng ^= rng >> 17;
        rng ^= rng << 5;
        (rng as f32) / (u32::MAX as f32)
    };

    let palette: [[f32; 4]; 5] = [
        [0.92, 0.26, 0.21, 0.8],
        [0.13, 0.59, 0.95, 0.8],
        [0.30, 0.69, 0.31, 0.8],
        [1.00, 0.76, 0.03, 0.8],
        [0.61, 0.15, 0.69, 0.8],
    ];

    for i in 0..NUM_POINTS {
        let x = next() * 1.6 - 0.8;
        let y = next() * 1.6 - 0.8;
        let radius = 0.005 + next() * 0.008;
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
    brush: BrushBehavior,
    overlay_renderer: Option<BrushOverlayRenderer>,
    mark_system: MarkSelectionSystem,
    viewport: ViewportTransform,

    win_size: [f32; 2],
    mouse_pos: [f32; 2],
    needs_redraw: bool,
    frame_count: u64,
    fps_timer: Instant,
}

impl App {
    fn new() -> Self {
        let data = generate_data();

        // Set up mark positions for hit testing
        let positions: Vec<[f32; 2]> = data.iter().map(|p| [p.x, p.y]).collect();
        let mut mark_system = MarkSelectionSystem::new(data.len());
        mark_system.set_positions(positions);

        let brush = BrushBehavior::new()
            .style(BrushStyle::default())
            .on("brush", |e: &BrushEvent| {
                if !e.selection.is_empty() {
                    print!("\r\x1b[K  Brushing: {} marks in region", e.selection.len());
                }
            })
            .on("brushend", |e: &BrushEvent| {
                println!(
                    "\r\x1b[K  Selected {} marks: {:?}",
                    e.selection.len(),
                    if e.selection.len() <= 20 {
                        format!("{:?}", e.selection)
                    } else {
                        format!(
                            "[{}, ... +{} more]",
                            e.selection
                                .iter()
                                .take(10)
                                .map(|id| id.to_string())
                                .collect::<Vec<_>>()
                                .join(", "),
                            e.selection.len() - 10,
                        )
                    }
                );
            });

        Self {
            window: None,
            context: None,
            data,
            selection: None,
            cache: PipelineCache::new(),
            brush,
            overlay_renderer: None,
            mark_system,
            viewport: ViewportTransform::default(),
            win_size: [800.0, 600.0],
            mouse_pos: [0.0; 2],
            needs_redraw: true,
            frame_count: 0,
            fps_timer: Instant::now(),
        }
    }

    /// Convert screen pixel coordinates to data space.
    ///
    /// Data points are in [-0.8, 0.8] clip space, which maps to the window.
    fn screen_to_data(&self, px: f32, py: f32) -> [f32; 2] {
        [
            (px / self.win_size[0]) * 2.0 - 1.0,
            -((py / self.win_size[1]) * 2.0 - 1.0),
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

        if let Some(sel) = &mut self.selection
            && !sel.is_render_ready()
        {
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

        // Lazily create the overlay renderer on first frame.
        if self.overlay_renderer.is_none() {
            match BrushOverlayRenderer::new(&ctx.device, &ctx.queue, &mut self.cache) {
                Ok(r) => self.overlay_renderer = Some(r),
                Err(e) => eprintln!("overlay init: {e}"),
            }
        }

        // Upload overlay geometry from BrushMark state.
        if let Some(ref mut overlay) = self.overlay_renderer {
            overlay.update(self.brush.overlay(), &ctx.queue);
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
                    // Draw brush overlay after data marks (highest z-order).
                    if let Some(ref overlay) = self.overlay_renderer {
                        overlay.render(&mut rp);
                    }
                }
                let _ = frame.finish();
            }
            Err(e) => eprintln!("frame: {e}"),
        }
        self.context = Some(Arc::new(ctx));

        // FPS counter
        self.frame_count += 1;
        let elapsed = self.fps_timer.elapsed().as_secs_f64();
        if elapsed >= 1.0 {
            let fps = self.frame_count as f64 / elapsed;
            if let Some(w) = &self.window {
                let brush_status = if self.brush.is_dragging() {
                    " [BRUSHING]"
                } else {
                    ""
                };
                w.set_title(&format!(
                    "GUP-285 Brush Selection — {NUM_POINTS} pts | {fps:.0} FPS{brush_status}"
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
                .with_title(format!("GUP-285 Brush Selection — {NUM_POINTS} points"))
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
                    println!("✓ Ready — drag to brush-select, Q/Escape to quit");
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
            } => {
                if matches!(code, KeyCode::KeyQ | KeyCode::Escape) {
                    event_loop.exit();
                }
            }

            WindowEvent::Resized(size) => {
                self.win_size = [size.width as f32, size.height as f32];

                // Update viewport transform to map screen pixels → data space
                self.viewport = ViewportTransform {
                    offset: Vec2::new(self.win_size[0] / 2.0, self.win_size[1] / 2.0),
                    scale: Vec2::new(self.win_size[0] / 2.0, -self.win_size[1] / 2.0),
                };

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
                self.mouse_pos = [position.x as f32, position.y as f32];

                // Update brush during drag
                if self.brush.is_dragging() {
                    let data_pos = self.screen_to_data(position.x as f32, position.y as f32);
                    self.brush.on_pointer_move(
                        Vec2::new(data_pos[0], data_pos[1]),
                        &self.viewport,
                        Some(&self.mark_system),
                    );
                    self.needs_redraw = true;
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                }
            }

            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                let data_pos = self.screen_to_data(self.mouse_pos[0], self.mouse_pos[1]);
                self.brush
                    .on_pointer_down(Vec2::new(data_pos[0], data_pos[1]));
            }

            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            } => {
                let data_pos = self.screen_to_data(self.mouse_pos[0], self.mouse_pos[1]);
                self.brush.on_pointer_up(
                    Vec2::new(data_pos[0], data_pos[1]),
                    &self.viewport,
                    Some(&self.mark_system),
                );
                self.needs_redraw = true;
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }

            WindowEvent::RedrawRequested => {
                self.render();
                self.needs_redraw = false;
            }

            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

fn main() {
    println!("GUP-285 Brush Selection Demo");
    println!("  Left-click drag to select data points");
    println!("  Q or Escape to quit");
    println!("  Rendering {NUM_POINTS} points");

    let event_loop = EventLoop::new().expect("event loop");
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = App::new();
    let _ = event_loop.run_app(&mut app);
}

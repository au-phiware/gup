// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Interactive circles demo — event handling system (GUP-013).
//!
//! Demonstrates the `.on()` event handler API on `Selection<T, Circle>`:
//!
//! - **Hover**: highlights circles on mouse-enter (yellow stroke).
//! - **Click**: logs data to stdout and briefly changes fill colour.
//! - **Global handler**: counts every mouse move.
//!
//! Uses `Selection::trigger_event` + `EventManager` to route events.
//!
//! # Controls
//!
//! - Move mouse over circles to see hover highlighting
//! - Click on circles to log their data
//! - Press **Q** or **Escape** to quit

use gup::event::{EventManager, EventResult, EventType, ModifierFlags, RawInputEvent};
use gup::interaction::{ElementHit, Vec2 as IVec2};
use gup::mark::circle::{Circle, CircleInstance};
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
    window::{Window, WindowAttributes},
};

// ---------------------------------------------------------------------------
// Data
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct CircleData {
    x: f32,
    y: f32,
    radius: f32,
    color: [f32; 4],
    label: String,
}

fn generate_data() -> Vec<CircleData> {
    let colors = [
        [0.92, 0.26, 0.21, 0.9], // red
        [0.13, 0.59, 0.95, 0.9], // blue
        [0.30, 0.69, 0.31, 0.9], // green
        [1.00, 0.76, 0.03, 0.9], // amber
        [0.61, 0.15, 0.69, 0.9], // purple
        [0.00, 0.74, 0.83, 0.9], // cyan
        [1.00, 0.34, 0.13, 0.9], // deep orange
        [0.47, 0.33, 0.28, 0.9], // brown
    ];
    let n = 30;
    (0..n)
        .map(|i| {
            let t = i as f32 / n as f32;
            let angle = t * std::f32::consts::TAU;
            let r = 0.35 + 0.25 * (angle * 2.0).sin();
            CircleData {
                x: r * angle.cos(),
                y: r * angle.sin(),
                radius: 0.025 + 0.015 * (i as f32 * 0.7).sin().abs(),
                color: colors[i % colors.len()],
                label: format!("circle-{i}"),
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Application state
// ---------------------------------------------------------------------------

struct App {
    window: Option<Arc<Window>>,
    context: Option<Arc<GupContext>>,
    data: Vec<CircleData>,
    selection: Option<Selection<CircleData, Circle>>,
    cache: PipelineCache,
    event_manager: EventManager,

    win_size: [f32; 2],
    mouse: [f32; 2],
    hovered_idx: Option<usize>,
    clicked_idx: Option<(usize, Instant)>,
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
            event_manager: EventManager::new(),
            win_size: [800.0, 600.0],
            mouse: [0.0; 2],
            hovered_idx: None,
            clicked_idx: None,
        }
    }

    fn screen_to_clip(&self, x: f32, y: f32) -> [f32; 2] {
        [
            (x / self.win_size[0]) * 2.0 - 1.0,
            -((y / self.win_size[1]) * 2.0 - 1.0),
        ]
    }

    fn hit_test(&self, clip: [f32; 2]) -> Option<usize> {
        self.data.iter().enumerate().find_map(|(i, d)| {
            let dx = d.x - clip[0];
            let dy = d.y - clip[1];
            if (dx * dx + dy * dy).sqrt() <= d.radius * 1.5 {
                Some(i)
            } else {
                None
            }
        })
    }

    fn rebuild_and_render(&mut self) {
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

        let hovered = self.hovered_idx;
        let clicked = self.clicked_idx;

        if let Some(sel) = &mut self.selection {
            let data_ref = &self.data;
            let _ = sel.prepare_render(
                &ctx.device,
                &ctx.queue,
                |d: &CircleData| {
                    let idx = data_ref
                        .iter()
                        .position(|dd| dd.label == d.label)
                        .unwrap_or(usize::MAX);
                    let is_hovered = hovered == Some(idx);
                    let is_clicked = clicked
                        .map(|(ci, t)| ci == idx && t.elapsed().as_millis() < 300)
                        .unwrap_or(false);

                    let fill = if is_clicked {
                        [1.0, 1.0, 1.0, 0.95]
                    } else {
                        d.color
                    };
                    let (stroke_color, stroke_width) = if is_hovered {
                        ([1.0, 0.92, 0.23, 1.0], 0.004)
                    } else {
                        ([0.0; 4], 0.0)
                    };

                    CircleInstance {
                        center: [d.x, d.y],
                        radius: d.radius,
                        _pad0: 0.0,
                        fill_color: fill,
                        stroke_width,
                        _pad1: [0.0; 3],
                        stroke_color,
                    }
                },
                Some(&mut self.cache),
                None,
            );
        }

        match ctx.begin_frame() {
            Ok(mut frame) => {
                let bg = Color {
                    r: 0.05,
                    g: 0.05,
                    b: 0.08,
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
    }

    fn setup_event_handlers(selection: &mut Selection<CircleData, Circle>, mgr: &mut EventManager) {
        let sel_id = selection.selection_id();

        selection.on_hover(|_event, data| {
            println!("[hover] entered: {}", data.label);
        });

        selection.on_click(|_event, data| {
            println!("[click] {} at ({:.2}, {:.2})", data.label, data.x, data.y);
        });

        mgr.register_global("mousemove", move |_event| {
            // Global handler fires for every mouse move (selection-independent).
            let _ = sel_id;
            EventResult::Continue
        });
    }
}

// ---------------------------------------------------------------------------
// winit ApplicationHandler
// ---------------------------------------------------------------------------

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        pollster::block_on(async {
            let attrs = WindowAttributes::default()
                .with_title("GUP-013 Interactive Circles")
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
                    let mut sel = Selection::<CircleData, Circle>::from_data(self.data.clone());
                    App::setup_event_handlers(&mut sel, &mut self.event_manager);
                    self.selection = Some(sel);
                    println!("✓ Ready — hover/click circles, Q to quit");
                }
                Err(e) => {
                    eprintln!("GPU init: {e}");
                    event_loop.exit();
                }
            }
        });
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _wid: winit::window::WindowId,
        event: WindowEvent,
    ) {
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
            } if code == KeyCode::KeyQ || code == KeyCode::Escape => {
                event_loop.exit();
            }

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
                self.mouse = [position.x as f32, position.y as f32];
                let clip = self.screen_to_clip(self.mouse[0], self.mouse[1]);
                let new_hover = self.hit_test(clip);

                if new_hover != self.hovered_idx {
                    if let Some(sel) = &self.selection {
                        if let Some(prev) = self.hovered_idx {
                            let raw = RawInputEvent::new(
                                EventType::MouseLeave,
                                IVec2::new(self.mouse[0], self.mouse[1]),
                            );
                            let mut ie = raw.into_interaction_event(None);
                            sel.trigger_event("mouseleave", &mut ie, prev as u32);
                        }
                        if let Some(cur) = new_hover {
                            let raw = RawInputEvent::new(
                                EventType::MouseEnter,
                                IVec2::new(self.mouse[0], self.mouse[1]),
                            );
                            let mut ie = raw.into_interaction_event(None);
                            sel.trigger_event("mouseenter", &mut ie, cur as u32);
                        }
                    }
                    self.hovered_idx = new_hover;
                    if let Some(win) = &self.window {
                        win.request_redraw();
                    }
                }

                // Dispatch through EventManager for global handlers.
                let raw = RawInputEvent::new(
                    EventType::MouseMove,
                    IVec2::new(self.mouse[0], self.mouse[1]),
                );
                let mut ie = raw.into_interaction_event(None);
                let hits: Vec<ElementHit> = new_hover
                    .map(|idx| {
                        let sel_id = self
                            .selection
                            .as_ref()
                            .map(|s| s.selection_id())
                            .unwrap_or(0);
                        vec![ElementHit::new(
                            idx as u32,
                            sel_id,
                            0.0,
                            IVec2::new(clip[0], clip[1]),
                        )]
                    })
                    .unwrap_or_default();
                self.event_manager.dispatch(&mut ie, &hits);
            }

            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                let clip = self.screen_to_clip(self.mouse[0], self.mouse[1]);
                if let Some(idx) = self.hit_test(clip) {
                    if let Some(sel) = &self.selection {
                        let raw = RawInputEvent::new(
                            EventType::MouseUp,
                            IVec2::new(self.mouse[0], self.mouse[1]),
                        )
                        .with_modifiers(ModifierFlags::NONE);
                        let mut ie = raw.into_interaction_event(None);
                        sel.trigger_event("click", &mut ie, idx as u32);
                    }
                    self.clicked_idx = Some((idx, Instant::now()));
                    if let Some(win) = &self.window {
                        win.request_redraw();
                    }
                }
            }

            WindowEvent::RedrawRequested => {
                self.rebuild_and_render();
            }

            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

fn main() {
    println!("GUP-013 Interactive Circles Demo");
    println!("  Hover over circles to highlight");
    println!("  Click on circles to log data");
    println!("  Press Q or Escape to quit");

    let event_loop = EventLoop::new().expect("event loop");
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = App::new();
    let _ = event_loop.run_app(&mut app);
}

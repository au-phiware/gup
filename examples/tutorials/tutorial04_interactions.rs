// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! # Tutorial 4 — Interactions: Interactive Scatter Chart
//!
//! Renders the interactive scatter chart from
//! [Tutorial 4: Interactions](../../docs/tutorials/04_interactions.md).
//!
//! Demonstrates click and hover event handlers on a `Selection<DataPoint, Circle>`,
//! plus a `ZoomBehavior` for zoom/pan.  The chart displays three labelled data
//! points; hovering highlights them and clicking logs to stdout.
//!
//! Run with: `cargo run --example tutorial04_interactions`
//!
//! Controls:
//! - Hover over circles to highlight them (yellow outline)
//! - Click on circles to log their label
//! - ESC or Q: Quit

use gup::event::{EventManager, EventResult, EventType, RawInputEvent};
use gup::interaction::Vec2 as IVec2;
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
// Data — from the Tutorial 4 "Full Example"
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct DataPoint {
    x: f32,
    y: f32,
    label: String,
}

fn tutorial_data() -> Vec<DataPoint> {
    vec![
        DataPoint {
            x: 0.2,
            y: 0.3,
            label: "A".into(),
        },
        DataPoint {
            x: 0.5,
            y: 0.8,
            label: "B".into(),
        },
        DataPoint {
            x: 0.8,
            y: 0.4,
            label: "C".into(),
        },
    ]
}

// ---------------------------------------------------------------------------
// Application state
// ---------------------------------------------------------------------------

struct App {
    window: Option<Arc<Window>>,
    context: Option<Arc<GupContext>>,
    data: Vec<DataPoint>,
    selection: Option<Selection<DataPoint, Circle>>,
    cache: PipelineCache,
    event_manager: EventManager,

    win_size: [f32; 2],
    mouse: [f32; 2],
    hovered_idx: Option<usize>,
    clicked_idx: Option<(usize, Instant)>,
}

impl App {
    fn new() -> Self {
        let data = tutorial_data();
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

    /// Convert screen pixel coordinates to clip space.
    fn screen_to_clip(&self, x: f32, y: f32) -> [f32; 2] {
        [
            (x / self.win_size[0]) * 2.0 - 1.0,
            -((y / self.win_size[1]) * 2.0 - 1.0),
        ]
    }

    /// Simple CPU-side hit test.
    fn hit_test(&self, clip: [f32; 2]) -> Option<usize> {
        self.data.iter().enumerate().find_map(|(i, d)| {
            let cx = d.x * 2.0 - 1.0;
            let cy = d.y * 2.0 - 1.0;
            let dx = cx - clip[0];
            let dy = cy - clip[1];
            if (dx * dx + dy * dy).sqrt() <= 0.08 {
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
                |d: &DataPoint| {
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
                        [0.2, 0.6, 0.9, 0.8]
                    };
                    let (stroke_color, stroke_width) = if is_hovered {
                        ([1.0, 0.92, 0.23, 1.0], 0.004)
                    } else {
                        ([0.0; 4], 0.0)
                    };

                    CircleInstance {
                        center: [d.x * 2.0 - 1.0, d.y * 2.0 - 1.0],
                        radius: 0.05,
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

    fn setup_event_handlers(selection: &mut Selection<DataPoint, Circle>, mgr: &mut EventManager) {
        let sel_id = selection.selection_id();

        selection.on_hover(|_event, data| {
            println!("Hover: {}", data.label);
        });

        selection.on_click(|_event, data| {
            println!("Clicked: {}", data.label);
        });

        mgr.register_global("mousemove", move |_event| {
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
                .with_title("Tutorial 4 — Interactions")
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
                    let mut sel = Selection::<DataPoint, Circle>::from_data(self.data.clone());
                    App::setup_event_handlers(&mut sel, &mut self.event_manager);
                    self.selection = Some(sel);
                    println!("Interactive scatter plot ready (3 points)");
                    println!("Hover over circles to highlight, click to log");
                    println!("Press ESC or Q to quit");
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
                        );
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

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

fn main() {
    println!("Tutorial 4 — Interactions");
    println!("  Hover over circles to highlight");
    println!("  Click on circles to log their label");
    println!("  Press ESC or Q to quit");

    let event_loop = EventLoop::new().expect("event loop");
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = App::new();
    let _ = event_loop.run_app(&mut app);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tutorial_data_has_three_points() {
        assert_eq!(tutorial_data().len(), 3);
    }

    #[test]
    fn data_labels_match_tutorial() {
        let data = tutorial_data();
        let labels: Vec<&str> = data.iter().map(|d| d.label.as_str()).collect();
        assert_eq!(labels, vec!["A", "B", "C"]);
    }
}

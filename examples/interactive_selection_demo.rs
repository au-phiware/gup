// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Interactive mark selection demo.
//!
//! Demonstrates the mark selection system with:
//! - Click to select individual marks (single mode)
//! - Shift+click for additive selection
//! - Ctrl+click to toggle individual marks
//! - Rectangle drag selection (press R to toggle tool)
//! - Undo/Redo with Z/Y keys
//! - Visual feedback: selected marks highlighted, others dimmed
//!
//! # Controls
//!
//! - **Left Click**: Select mark (or clear if clicking empty space)
//! - **Shift+Click**: Add to selection
//! - **Ctrl+Click**: Toggle selection
//! - **R**: Toggle between point and rectangle selection tool
//! - **A**: Select all
//! - **Escape**: Clear selection / cancel tool
//! - **Z**: Undo
//! - **Y**: Redo
//! - **Q**: Quit

use gup::mark::circle::{Circle, CircleInstance};
use gup::mark_selection::{KeyModifiers, MarkSelectionSystem, SelectionStyle, SelectionToolKind};
use gup::selection::Selection;
use gup::{GupContext, PipelineCache};
use std::sync::Arc;
use wgpu::Color;
use winit::{
    application::ApplicationHandler,
    event::{ElementState, KeyEvent, MouseButton, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, ModifiersState, PhysicalKey},
    window::{Window, WindowAttributes},
};

// ---------------------------------------------------------------------------
// Data
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct DataPoint {
    x: f32,
    y: f32,
    radius: f32,
    color: [f32; 4],
}

fn generate_data(count: usize) -> Vec<DataPoint> {
    (0..count)
        .map(|i| {
            let t = i as f32 / count as f32;
            let angle = t * std::f32::consts::TAU * 3.0;
            let r = 0.3 + t * 0.5;
            DataPoint {
                x: r * angle.cos(),
                y: r * angle.sin(),
                radius: 0.012 + (1.0 - t) * 0.015,
                color: [
                    0.2 + 0.6 * (t * 4.0).sin().abs(),
                    0.3 + 0.5 * ((t + 0.33) * 4.0).sin().abs(),
                    0.4 + 0.5 * ((t + 0.67) * 4.0).sin().abs(),
                    0.9,
                ],
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Application
// ---------------------------------------------------------------------------

struct App {
    window: Option<Arc<Window>>,
    context: Option<Arc<GupContext>>,
    data: Vec<DataPoint>,
    selection: Option<Selection<DataPoint, Circle>>,
    cache: PipelineCache,
    sel_sys: MarkSelectionSystem,
    win_size: [f32; 2],
    mouse: [f32; 2],
    modifiers: ModifiersState,
}

impl App {
    fn new() -> Self {
        let data = generate_data(200);
        let sel_sys = MarkSelectionSystem::with_style(data.len(), SelectionStyle::default());
        Self {
            window: None,
            context: None,
            data,
            selection: None,
            cache: PipelineCache::new(),
            sel_sys,
            win_size: [800.0, 600.0],
            mouse: [0.0, 0.0],
            modifiers: ModifiersState::empty(),
        }
    }

    fn screen_to_clip(&self, x: f32, y: f32) -> [f32; 2] {
        [
            (x / self.win_size[0]) * 2.0 - 1.0,
            -((y / self.win_size[1]) * 2.0 - 1.0),
        ]
    }

    fn hit_test(&self, clip: [f32; 2]) -> Option<u32> {
        self.data.iter().enumerate().find_map(|(i, d)| {
            let dx = d.x - clip[0];
            let dy = d.y - clip[1];
            if (dx * dx + dy * dy).sqrt() <= d.radius * 1.5 {
                Some(i as u32)
            } else {
                None
            }
        })
    }

    fn rect_hit_test(&self, min: [f32; 2], max: [f32; 2]) -> Vec<u32> {
        self.data
            .iter()
            .enumerate()
            .filter(|(_, d)| d.x >= min[0] && d.x <= max[0] && d.y >= min[1] && d.y <= max[1])
            .map(|(i, _)| i as u32)
            .collect()
    }

    /// Rebuild instances reflecting current selection visual state.
    fn rebuild_instances(&mut self) {
        let instances: Vec<CircleInstance> = self
            .data
            .iter()
            .enumerate()
            .map(|(i, d)| {
                let id = i as u32;
                let opacity = self.sel_sys.mark_opacity(id);
                let scale = self.sel_sys.mark_scale(id);
                let outline = self.sel_sys.mark_outline(id);
                let mut color = d.color;
                color[3] *= opacity;
                let (sc, sw) = outline
                    .map(|(c, w)| (c, w * 0.005))
                    .unwrap_or(([0.0; 4], 0.0));
                CircleInstance {
                    center: [d.x, d.y],
                    radius: d.radius * scale,
                    _pad0: 0.0,
                    fill_color: color,
                    stroke_width: sw,
                    _pad1: [0.0; 3],
                    stroke_color: sc,
                }
            })
            .collect();

        if let (Some(ctx), Some(sel)) = (&self.context, &mut self.selection) {
            let inst = instances.clone();
            let _ = sel.prepare_render(
                &ctx.device,
                &ctx.queue,
                |d: &DataPoint| {
                    // Linear search — fine for 200 points.
                    inst.iter()
                        .find(|ci| {
                            (ci.center[0] - d.x).abs() < f32::EPSILON
                                && (ci.center[1] - d.y).abs() < f32::EPSILON
                        })
                        .copied()
                        .unwrap_or(CircleInstance {
                            center: [d.x, d.y],
                            radius: d.radius,
                            _pad0: 0.0,
                            fill_color: d.color,
                            stroke_width: 0.0,
                            _pad1: [0.0; 3],
                            stroke_color: [0.0; 4],
                        })
                },
                Some(&mut self.cache),
            );
        }
    }

    fn render_frame(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(context) = self.context.take() {
            let mut ctx = Arc::try_unwrap(context).map_err(|_| "context in use")?;
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
                    frame.finish()?;
                }
                Err(e) => eprintln!("frame: {e}"),
            }
            self.context = Some(Arc::new(ctx));
        }
        Ok(())
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        pollster::block_on(async {
            let attrs = WindowAttributes::default()
                .with_title("Gup Interactive Selection — Click/Drag, R=rect, Z=undo, Q=quit")
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
                    self.selection =
                        Some(Selection::<DataPoint, Circle>::from_data(self.data.clone()));
                    self.rebuild_instances();
                    println!("✓ Ready! Click marks, R=rect, Shift/Ctrl+Click, Z=undo");
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

            WindowEvent::ModifiersChanged(m) => {
                self.modifiers = m.state();
                self.sel_sys.set_modifiers(KeyModifiers {
                    ctrl: self.modifiers.control_key(),
                    shift: self.modifiers.shift_key(),
                    alt: self.modifiers.alt_key(),
                });
            }

            WindowEvent::CursorMoved { position, .. } => {
                self.mouse = [position.x as f32, position.y as f32];
                let clip = self.screen_to_clip(position.x as f32, position.y as f32);
                let hit = self.hit_test(clip);
                let prev = self.sel_sys.state().hover();
                self.sel_sys.on_hover(hit);
                if prev != hit {
                    self.rebuild_instances();
                }
                if self.sel_sys.is_tool_active() {
                    self.sel_sys.on_mouse_move(clip);
                }
            }

            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                let clip = self.screen_to_clip(self.mouse[0], self.mouse[1]);
                self.sel_sys.on_mouse_down(clip);
            }

            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            } => {
                let clip = self.screen_to_clip(self.mouse[0], self.mouse[1]);
                let ids = match self.sel_sys.tool_kind() {
                    SelectionToolKind::Point => self.hit_test(clip).into_iter().collect::<Vec<_>>(),
                    SelectionToolKind::Rectangle => {
                        if let Some(r) = self.sel_sys.current_drag_rect() {
                            self.rect_hit_test([r.min.x, r.min.y], [r.max.x, r.max.y])
                        } else {
                            vec![]
                        }
                    }
                    SelectionToolKind::Lasso => {
                        if let Some(pts) = self.sel_sys.current_lasso_points() {
                            let pos: Vec<[f32; 2]> = self.data.iter().map(|d| [d.x, d.y]).collect();
                            MarkSelectionSystem::filter_by_lasso(pts, &pos)
                        } else {
                            vec![]
                        }
                    }
                };
                self.sel_sys.on_mouse_up(clip, &ids);
                self.rebuild_instances();
                let s = self.sel_sys.statistics();
                println!("{} / {} selected", s.selected_count, s.total_marks);
            }

            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(key),
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } => match key {
                KeyCode::KeyQ => event_loop.exit(),
                KeyCode::KeyR => {
                    let t = if *self.sel_sys.tool_kind() == SelectionToolKind::Rectangle {
                        SelectionToolKind::Point
                    } else {
                        SelectionToolKind::Rectangle
                    };
                    self.sel_sys.set_tool(t.clone());
                    println!("Tool: {t:?}");
                }
                KeyCode::KeyA => {
                    self.sel_sys.state_mut().select_all();
                    self.rebuild_instances();
                    println!("All selected");
                }
                KeyCode::KeyZ => {
                    if self.sel_sys.undo() {
                        self.rebuild_instances();
                        println!("Undo → {}", self.sel_sys.state().count());
                    }
                }
                KeyCode::KeyY => {
                    if self.sel_sys.redo() {
                        self.rebuild_instances();
                        println!("Redo → {}", self.sel_sys.state().count());
                    }
                }
                KeyCode::Escape => {
                    if self.sel_sys.is_tool_active() {
                        self.sel_sys.cancel();
                    } else {
                        self.sel_sys.state_mut().clear();
                        self.rebuild_instances();
                        println!("Cleared");
                    }
                }
                _ => {}
            },

            WindowEvent::RedrawRequested => {
                let _ = self.render_frame();
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting Interactive Selection Demo...");
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);
    event_loop.run_app(&mut App::new())?;
    Ok(())
}

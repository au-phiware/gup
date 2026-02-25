// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Large-dataset GPU-accelerated selection demo (GUP-181).
//!
//! Demonstrates GPU-accelerated hit testing with 50K+ marks:
//! - Point, rectangle, and lasso selection tools
//! - GPU compute shader hit testing via InteractionSystem
//! - CPU fallback when GPU is unavailable
//! - Sub-millisecond selection on large datasets
//!
//! # Controls
//!
//! - **Left Click**: Select mark
//! - **Shift+Click**: Add to selection
//! - **Ctrl+Click**: Toggle selection
//! - **R**: Cycle tool: Point → Rectangle → Lasso → Point
//! - **A**: Select all
//! - **Escape**: Clear selection / cancel tool
//! - **Z**: Undo
//! - **Y**: Redo
//! - **Q**: Quit

use gup::interaction::{InteractionSystem, Rect, Vec2};
use gup::mark::circle::{Circle, CircleInstance};
use gup::mark_selection::{KeyModifiers, MarkSelectionSystem, SelectionStyle, SelectionToolKind};
use gup::selection::Selection;
use gup::{GupContext, PipelineCache, RenderContext};
use std::sync::Arc;
use std::time::Instant;
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

const MARK_COUNT: usize = 50_000;

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
            // Generate a galaxy-like distribution
            let arm = (i % 5) as f32;
            let arm_angle = arm * std::f32::consts::TAU / 5.0;
            let r = 0.05 + t.sqrt() * 0.85;
            let spread = 0.15 * (1.0 - t * 0.5);
            let angle = arm_angle + t * std::f32::consts::TAU * 2.0;
            // Add some noise
            let noise_x = ((i as f32 * 7.3).sin() * 0.5 + 0.5) * spread;
            let noise_y = ((i as f32 * 13.7).cos() * 0.5 + 0.5) * spread;
            DataPoint {
                x: r * angle.cos() + noise_x - spread * 0.5,
                y: r * angle.sin() + noise_y - spread * 0.5,
                radius: 0.002 + (1.0 - t) * 0.003,
                color: [
                    0.2 + 0.6 * (arm / 5.0),
                    0.3 + 0.5 * t,
                    0.4 + 0.5 * (1.0 - t),
                    0.85,
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
    interaction_system: Option<InteractionSystem>,
    win_size: [f32; 2],
    mouse: [f32; 2],
    modifiers: ModifiersState,
    last_hit_test_ms: f64,
}

impl App {
    fn new() -> Self {
        let data = generate_data(MARK_COUNT);
        let mut sel_sys = MarkSelectionSystem::with_style(data.len(), SelectionStyle::default());

        // Register positions for hit testing (enables both CPU and GPU paths)
        let positions: Vec<[f32; 2]> = data.iter().map(|d| [d.x, d.y]).collect();
        let sizes: Vec<[f32; 2]> = data.iter().map(|d| [d.radius, d.radius]).collect();
        sel_sys.set_positions_with_sizes(positions, sizes);

        Self {
            window: None,
            context: None,
            data,
            selection: None,
            cache: PipelineCache::new(),
            sel_sys,
            interaction_system: None,
            win_size: [1024.0, 768.0],
            mouse: [0.0, 0.0],
            modifiers: ModifiersState::empty(),
            last_hit_test_ms: 0.0,
        }
    }

    fn screen_to_clip(&self, x: f32, y: f32) -> [f32; 2] {
        [
            (x / self.win_size[0]) * 2.0 - 1.0,
            -((y / self.win_size[1]) * 2.0 - 1.0),
        ]
    }

    fn hit_test_with_timing(&mut self, clip: [f32; 2]) -> Vec<u32> {
        let start = Instant::now();
        let result = if let Some(ref mut is) = self.interaction_system {
            // GPU-accelerated path
            pollster::block_on(self.sel_sys.hit_test_gpu(clip, is, 0.015)).unwrap_or_default()
        } else {
            // CPU fallback
            self.sel_sys.hit_test(clip, 0.015)
        };
        self.last_hit_test_ms = start.elapsed().as_secs_f64() * 1000.0;
        result
    }

    fn rect_hit_test_with_timing(&mut self, rect: &Rect) -> Vec<u32> {
        let start = Instant::now();
        let result = if let Some(ref mut is) = self.interaction_system {
            pollster::block_on(self.sel_sys.rect_hit_test_gpu(rect, is)).unwrap_or_default()
        } else {
            self.sel_sys.rect_hit_test(rect)
        };
        self.last_hit_test_ms = start.elapsed().as_secs_f64() * 1000.0;
        result
    }

    fn lasso_hit_test_with_timing(&mut self, path: &[Vec2]) -> Vec<u32> {
        let start = Instant::now();
        let result = if let Some(ref mut is) = self.interaction_system {
            pollster::block_on(self.sel_sys.lasso_hit_test_gpu(path, is)).unwrap_or_default()
        } else {
            self.sel_sys.lasso_hit_test(path)
        };
        self.last_hit_test_ms = start.elapsed().as_secs_f64() * 1000.0;
        result
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
                    .map(|(c, w)| (c, w * 0.002))
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
                        r: 0.02,
                        g: 0.02,
                        b: 0.05,
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
                .with_title(format!(
                    "Gup Large-Dataset Selection — {MARK_COUNT} marks, R=cycle tool, Q=quit"
                ))
                .with_inner_size(winit::dpi::LogicalSize::new(1024, 768));
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
                    // Initialise InteractionSystem for GPU hit testing
                    match RenderContext::new().await {
                        Ok(render_ctx) => match InteractionSystem::new(&render_ctx).await {
                            Ok(is) => {
                                self.interaction_system = Some(is);
                                println!("✓ GPU InteractionSystem initialised");
                            }
                            Err(e) => {
                                eprintln!(
                                    "InteractionSystem init failed (using CPU fallback): {e}"
                                );
                            }
                        },
                        Err(e) => {
                            eprintln!("RenderContext init failed (using CPU fallback): {e}");
                        }
                    }

                    self.context = Some(ctx);
                    self.window = Some(window);
                    self.selection =
                        Some(Selection::<DataPoint, Circle>::from_data(self.data.clone()));
                    self.rebuild_instances();
                    println!(
                        "✓ Ready with {MARK_COUNT} marks! Click, R=tool cycle, Shift/Ctrl+Click, Z=undo"
                    );
                    println!(
                        "  Using: {}",
                        if self.interaction_system.is_some() {
                            "GPU-accelerated hit testing"
                        } else {
                            "CPU fallback hit testing"
                        }
                    );
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
                // Hover hit test (throttled — only closest mark)
                let hits = self.hit_test_with_timing(clip);
                let hit = hits.first().copied();
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
                    SelectionToolKind::Point => self.hit_test_with_timing(clip),
                    SelectionToolKind::Rectangle => {
                        if let Some(r) = self.sel_sys.current_drag_rect() {
                            self.rect_hit_test_with_timing(&r)
                        } else {
                            vec![]
                        }
                    }
                    SelectionToolKind::Lasso => {
                        if let Some(pts) = self.sel_sys.current_lasso_points() {
                            let pts_owned: Vec<Vec2> = pts.to_vec();
                            self.lasso_hit_test_with_timing(&pts_owned)
                        } else {
                            vec![]
                        }
                    }
                };
                self.sel_sys.on_mouse_up(clip, &ids);
                self.rebuild_instances();
                let s = self.sel_sys.statistics();
                println!(
                    "{} / {} selected | hit test: {:.2}ms | tool: {:?}",
                    s.selected_count,
                    s.total_marks,
                    self.last_hit_test_ms,
                    self.sel_sys.tool_kind()
                );
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
                    let t = match self.sel_sys.tool_kind() {
                        SelectionToolKind::Point => SelectionToolKind::Rectangle,
                        SelectionToolKind::Rectangle => SelectionToolKind::Lasso,
                        SelectionToolKind::Lasso => SelectionToolKind::Point,
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
    println!("Starting Large-Dataset GPU Selection Demo with {MARK_COUNT} marks...");
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);
    event_loop.run_app(&mut App::new())?;
    Ok(())
}

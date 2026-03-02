// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Linked views demo — cross-chart coordinated selection (GUP-279).
//!
//! Renders two scatter plots side by side. Both share the same data set
//! and the same [`SharedSelectionState`]. Dragging a brush rectangle on
//! the **left** plot selects data points; the corresponding points in the
//! **right** plot are highlighted at full opacity while unselected points
//! are dimmed.
//!
//! The left plot shows **x vs y** and the right plot shows **x vs value**,
//! demonstrating that linked selection works across different projections
//! of the same data.
//!
//! # Controls
//!
//! - **Left-click drag** on the left plot: Brush-select points
//! - **Q** / **Escape**: Quit

use gup::brush::{BrushBehavior, BrushStyle};
use gup::event::ViewportTransform;
use gup::interaction::Vec2;
use gup::linked_selection::{SharedSelectionState, build_dimmed_instances, has_changed_since};
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

const NUM_POINTS: usize = 500;
/// Default dim opacity for unselected items.
const DIM_OPACITY: f32 = 0.15;

/// A single data point with three dimensions.
#[derive(Clone, Debug)]
struct DataPoint {
    /// X position (normalised 0..1)
    x: f32,
    /// Y position (normalised 0..1)
    y: f32,
    /// A third value, used as the Y-axis in the right plot (normalised 0..1)
    value: f32,
    /// Category index (for colouring)
    category: usize,
}

/// Colour palette — one per category.
const PALETTE: [[f32; 4]; 5] = [
    [0.92, 0.26, 0.21, 0.85],
    [0.13, 0.59, 0.95, 0.85],
    [0.30, 0.69, 0.31, 0.85],
    [1.00, 0.76, 0.03, 0.85],
    [0.61, 0.15, 0.69, 0.85],
];

/// Simple PRNG (xorshift32).
fn xorshift(state: &mut u32) -> f32 {
    *state ^= *state << 13;
    *state ^= *state >> 17;
    *state ^= *state << 5;
    (*state as f32) / (u32::MAX as f32)
}

fn generate_data() -> Vec<DataPoint> {
    let mut rng: u32 = 0xDEAD_BEEF;
    (0..NUM_POINTS)
        .map(|i| {
            let x = xorshift(&mut rng);
            let y = xorshift(&mut rng);
            // value is a noisy function of x + y to create visual correlation
            let value = ((x + y) * 0.5 + xorshift(&mut rng) * 0.3).clamp(0.0, 1.0);
            DataPoint {
                x,
                y,
                value,
                category: i % PALETTE.len(),
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Mapping functions — data → CircleInstance
// ---------------------------------------------------------------------------

/// Map a point into the **left** plot (x vs y).
///
/// Clip-space x: [-0.95, -0.05], y: [-0.9, 0.9]
fn to_left_instance(p: &DataPoint) -> CircleInstance {
    let cx = -0.95 + p.x * 0.9; // left half
    let cy = -0.9 + p.y * 1.8;
    CircleInstance {
        center: [cx, cy],
        radius: 0.012,
        _pad0: 0.0,
        fill_color: PALETTE[p.category],
        stroke_width: 0.0,
        _pad1: [0.0; 3],
        stroke_color: [0.0; 4],
    }
}

/// Map a point into the **right** plot (x vs value).
///
/// Clip-space x: [0.05, 0.95], y: [-0.9, 0.9]
fn to_right_instance(p: &DataPoint) -> CircleInstance {
    let cx = 0.05 + p.x * 0.9; // right half
    let cy = -0.9 + p.value * 1.8;
    CircleInstance {
        center: [cx, cy],
        radius: 0.012,
        _pad0: 0.0,
        fill_color: PALETTE[p.category],
        stroke_width: 0.0,
        _pad1: [0.0; 3],
        stroke_color: [0.0; 4],
    }
}

// ---------------------------------------------------------------------------
// Application state
// ---------------------------------------------------------------------------

struct App {
    window: Option<Arc<Window>>,
    context: Option<Arc<GupContext>>,
    data: Vec<DataPoint>,

    // Left plot (x vs y)
    left_selection: Option<Selection<DataPoint, Circle>>,
    // Right plot (x vs value)
    right_selection: Option<Selection<DataPoint, Circle>>,

    cache: PipelineCache,
    brush: BrushBehavior,
    mark_system: MarkSelectionSystem,
    viewport: ViewportTransform,

    // Linked-view state
    shared_state: SharedSelectionState<usize>,
    last_seen_gen: u64,

    win_size: [f32; 2],
    mouse_pos: [f32; 2],
    needs_redraw: bool,
    frame_count: u64,
    fps_timer: Instant,
}

impl App {
    fn new() -> Self {
        let data = generate_data();

        // Mark positions for brush hit-testing (left plot data-space coords).
        // The left plot maps x,y ∈ [0,1] to clip-space [-0.95,-0.05] × [-0.9,0.9].
        let positions: Vec<[f32; 2]> = data
            .iter()
            .map(|p| {
                let cx = -0.95 + p.x * 0.9;
                let cy = -0.9 + p.y * 1.8;
                [cx, cy]
            })
            .collect();
        let mut mark_system = MarkSelectionSystem::new(data.len());
        mark_system.set_positions(positions);

        // Shared selection state — keyed by data index (usize)
        let shared_state = SharedSelectionState::<usize>::new();

        // Brush wired to update shared selection
        let brush = BrushBehavior::new()
            .style(BrushStyle {
                fill: [0.4, 0.7, 1.0, 0.18],
                stroke: [0.4, 0.7, 1.0, 0.7],
                stroke_width: 1.5,
            })
            .with_shared_selection(shared_state.clone(), |idx| idx as usize);

        Self {
            window: None,
            context: None,
            data,
            left_selection: None,
            right_selection: None,
            cache: PipelineCache::new(),
            brush,
            mark_system,
            viewport: ViewportTransform::default(),
            shared_state,
            last_seen_gen: 0,
            win_size: [1000.0, 600.0],
            mouse_pos: [0.0; 2],
            needs_redraw: true,
            frame_count: 0,
            fps_timer: Instant::now(),
        }
    }

    /// Convert screen pixel coordinates to data space (clip space).
    ///
    /// Only the left half of the window is interactive; we map the left
    /// half to the left plot's clip-space range.
    fn screen_to_data(&self, px: f32, py: f32) -> [f32; 2] {
        [
            (px / self.win_size[0]) * 2.0 - 1.0,
            -((py / self.win_size[1]) * 2.0 - 1.0),
        ]
    }

    /// Returns true if the mouse is in the left half of the window.
    fn mouse_in_left_half(&self) -> bool {
        self.mouse_pos[0] < self.win_size[0] / 2.0
    }

    /// Rebuild instance buffers for both selections, applying dimming.
    fn rebuild_instances(&mut self) {
        let Some(ctx) = &self.context else { return };

        // Left plot
        if let Some(sel) = &mut self.left_selection {
            let instances = build_dimmed_instances(
                &self.data,
                to_left_instance,
                |_p, idx| idx,
                &self.shared_state,
                DIM_OPACITY,
            );
            let inst_clone = instances.clone();
            let _ = sel.prepare_render(
                &ctx.device,
                &ctx.queue,
                |d: &DataPoint| {
                    let idx = self
                        .data
                        .iter()
                        .position(|p| {
                            (p.x - d.x).abs() < f32::EPSILON && (p.y - d.y).abs() < f32::EPSILON
                        })
                        .unwrap_or(0);
                    inst_clone[idx]
                },
                Some(&mut self.cache),
                None,
            );
        }

        // Right plot
        if let Some(sel) = &mut self.right_selection {
            let instances = build_dimmed_instances(
                &self.data,
                to_right_instance,
                |_p, idx| idx,
                &self.shared_state,
                DIM_OPACITY,
            );
            let inst_clone = instances.clone();
            let _ = sel.prepare_render(
                &ctx.device,
                &ctx.queue,
                |d: &DataPoint| {
                    let idx = self
                        .data
                        .iter()
                        .position(|p| {
                            (p.x - d.x).abs() < f32::EPSILON && (p.y - d.y).abs() < f32::EPSILON
                        })
                        .unwrap_or(0);
                    inst_clone[idx]
                },
                Some(&mut self.cache),
                None,
            );
        }
    }

    fn render(&mut self) {
        // Check for selection state changes and rebuild if needed
        if let Some(new_gen) = has_changed_since(&self.shared_state, self.last_seen_gen) {
            self.last_seen_gen = new_gen;
            self.rebuild_instances();
        }

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
                    // Render left scatter plot
                    if let Some(s) = &self.left_selection {
                        let _ = s.render(&mut rp);
                    }
                    // Render right scatter plot
                    if let Some(s) = &self.right_selection {
                        let _ = s.render(&mut rp);
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
            let selected = self.shared_state.selected_count();
            let title = format!(
                "GUP-279 Linked Views — {NUM_POINTS} pts × 2 | {selected} selected | {fps:.0} FPS"
            );
            if let Some(w) = &self.window {
                w.set_title(&title);
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
                .with_title(format!("GUP-279 Linked Views — {NUM_POINTS} points × 2"))
                .with_inner_size(winit::dpi::LogicalSize::new(1000, 600));
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
                    self.left_selection =
                        Some(Selection::<DataPoint, Circle>::from_data(self.data.clone()));
                    self.right_selection =
                        Some(Selection::<DataPoint, Circle>::from_data(self.data.clone()));

                    // Initial render with no selection
                    self.rebuild_instances();
                    println!("✓ Ready — drag on the LEFT scatter plot to brush-select");
                    println!("  Left: X vs Y   |   Right: X vs Value");
                    println!("  Selected points are highlighted in both plots");
                    println!("  Q/Escape to quit");
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
                // Only start brushing in the left half
                if self.mouse_in_left_half() {
                    let data_pos = self.screen_to_data(self.mouse_pos[0], self.mouse_pos[1]);
                    self.brush
                        .on_pointer_down(Vec2::new(data_pos[0], data_pos[1]));
                }
            }

            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            } => {
                if self.brush.is_dragging() {
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
    println!("GUP-279 Linked Views Demo");
    println!("  Two scatter plots sharing a SharedSelectionState");
    println!("  Drag on the LEFT plot to brush-select");

    let event_loop = EventLoop::new().expect("event loop");
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = App::new();
    let _ = event_loop.run_app(&mut app);
}

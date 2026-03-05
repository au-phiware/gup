// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Windowed treemap rendering example (GUP-314).
//!
//! Renders a synthetic hierarchy as a treemap in a real GPU window using
//! Rectangle marks.  Cells are coloured by depth or by value, switchable
//! at runtime.  The treemap re-layouts on window resize.
//!
//! # Controls
//!
//! - **C**: Cycle colour mode (depth / value)
//! - **A**: Cycle algorithm (Squarified / Binary / Strip / SliceDice)
//! - **Q** / **Escape**: Quit
//!
//! # Usage
//!
//! ```sh
//! cargo run --example treemap_window
//! cargo run --example treemap_window -- --nodes 5000
//! cargo run --example treemap_window -- --color value
//! cargo run --example treemap_window -- --algo binary
//! ```

use gup::layout::{LayoutEngine, LayoutRect, TreeNode, TreemapAlgorithm, TreemapOptions};
use gup::mark::rectangle::{Rectangle, RectangleInstance};
use gup::render::RenderContext;
use gup::selection::Selection;
use gup::{GupContext, PipelineCache};
use std::sync::Arc;
use std::time::Instant;
use wgpu::Color;
use winit::{
    application::ApplicationHandler,
    event::{ElementState, KeyEvent, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowAttributes, WindowId},
};

// ---------------------------------------------------------------------------
// Deterministic pseudo-random number generator
// ---------------------------------------------------------------------------

struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed)
    }
    fn next_f32(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1);
        ((self.0 >> 33) as f32) / (u32::MAX as f32)
    }
    fn next_u32(&mut self, max: u32) -> u32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1);
        ((self.0 >> 33) % max as u64) as u32
    }
}

// ---------------------------------------------------------------------------
// Tree generation (reused from treemap.rs CLI example)
// ---------------------------------------------------------------------------

fn generate_tree(n: u32, rng: &mut Rng) -> (Vec<TreeNode>, Vec<f32>) {
    if n == 0 {
        return (vec![], vec![]);
    }

    let mut nodes = Vec::with_capacity(n as usize);
    let mut values = Vec::with_capacity(n as usize);

    nodes.push(TreeNode {
        parent: None,
        child_start: 0,
        child_count: 0,
    });
    values.push(0.0);

    if n == 1 {
        values[0] = 1.0;
        return (nodes, values);
    }

    let mut next_idx = 1u32;
    let mut parent_queue = std::collections::VecDeque::new();
    parent_queue.push_back(0u32);

    while next_idx < n && !parent_queue.is_empty() {
        let parent = parent_queue.pop_front().unwrap();
        let remaining = n - next_idx;
        let max_children = remaining.min(2 + rng.next_u32(5));
        if max_children == 0 {
            continue;
        }

        nodes[parent as usize].child_start = next_idx;
        nodes[parent as usize].child_count = max_children;

        for _ in 0..max_children {
            let idx = next_idx;
            nodes.push(TreeNode {
                parent: Some(parent),
                child_start: 0,
                child_count: 0,
            });
            values.push(1.0 + rng.next_f32() * 99.0);
            parent_queue.push_back(idx);
            next_idx += 1;
            if next_idx >= n {
                break;
            }
        }
    }

    (nodes, values)
}

// ---------------------------------------------------------------------------
// Colour helpers
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum ColorMode {
    Depth,
    Value,
}

impl ColorMode {
    fn next(self) -> Self {
        match self {
            Self::Depth => Self::Value,
            Self::Value => Self::Depth,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Depth => "depth",
            Self::Value => "value",
        }
    }
}

/// Map depth to an RGBA colour (blue → green → yellow → red).
fn depth_color(depth: u32, max_depth: u32) -> [f32; 4] {
    let t = if max_depth > 0 {
        (depth as f32) / (max_depth as f32)
    } else {
        0.0
    };
    let (r, g, b) = if t < 0.33 {
        let s = t / 0.33;
        (0.0, s, 1.0 - s)
    } else if t < 0.66 {
        let s = (t - 0.33) / 0.33;
        (s, 1.0, 0.0)
    } else {
        let s = (t - 0.66) / 0.34;
        (1.0, 1.0 - s, 0.0)
    };
    [r, g, b, 0.85]
}

/// Map normalised value (0–1) to an RGBA colour (light → dark blue).
fn value_color(normalised: f32) -> [f32; 4] {
    let t = normalised.clamp(0.0, 1.0);
    let r = 0.1 + 0.2 * (1.0 - t);
    let g = 0.2 + 0.3 * (1.0 - t);
    let b = 0.5 + 0.5 * t;
    [r, g, b, 0.9]
}

// ---------------------------------------------------------------------------
// Algorithm cycling
// ---------------------------------------------------------------------------

const ALL_ALGORITHMS: [TreemapAlgorithm; 4] = [
    TreemapAlgorithm::Squarified,
    TreemapAlgorithm::Binary,
    TreemapAlgorithm::Strip,
    TreemapAlgorithm::SliceDice,
];

fn algo_label(algo: TreemapAlgorithm) -> &'static str {
    match algo {
        TreemapAlgorithm::Squarified => "squarified",
        TreemapAlgorithm::Binary => "binary",
        TreemapAlgorithm::Strip => "strip",
        TreemapAlgorithm::SliceDice => "slice-dice",
    }
}

fn next_algorithm(algo: TreemapAlgorithm) -> TreemapAlgorithm {
    let idx = ALL_ALGORITHMS.iter().position(|&a| a == algo).unwrap_or(0);
    ALL_ALGORITHMS[(idx + 1) % ALL_ALGORITHMS.len()]
}

// ---------------------------------------------------------------------------
// Cell data row for Selection
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct CellData {
    /// Centre X in clip space [-1, 1].
    cx: f32,
    /// Centre Y in clip space [-1, 1].
    cy: f32,
    /// Width in clip space units.
    w: f32,
    /// Height in clip space units.
    h: f32,
    /// Fill colour (RGBA).
    color: [f32; 4],
}

// ---------------------------------------------------------------------------
// Application state
// ---------------------------------------------------------------------------

struct App {
    // Window / GPU
    window: Option<Arc<Window>>,
    gup_ctx: Option<Arc<GupContext>>,

    // Layout engine (uses a separate headless context)
    render_ctx: Option<RenderContext>,
    engine: Option<LayoutEngine>,

    // Tree data
    nodes: Vec<TreeNode>,
    values: Vec<f32>,

    // Current settings
    node_count: u32,
    color_mode: ColorMode,
    algorithm: TreemapAlgorithm,
    win_size: [f32; 2],

    // Rendering
    rect_sel: Option<Selection<CellData, Rectangle>>,
    cache: PipelineCache,
    needs_layout: bool,

    // Stats
    frame_count: u64,
    fps_timer: Instant,
}

impl App {
    fn new(node_count: u32, color_mode: ColorMode, algorithm: TreemapAlgorithm) -> Self {
        let mut rng = Rng::new(42);
        let (nodes, values) = generate_tree(node_count, &mut rng);

        Self {
            window: None,
            gup_ctx: None,
            render_ctx: None,
            engine: None,
            nodes,
            values,
            node_count,
            color_mode,
            algorithm,
            win_size: [800.0, 600.0],
            rect_sel: None,
            cache: PipelineCache::new(),
            needs_layout: true,
            frame_count: 0,
            fps_timer: Instant::now(),
        }
    }

    /// Run the treemap layout and convert cells to clip-space CellData.
    fn run_layout(&mut self) {
        let Some(engine) = &self.engine else {
            return;
        };

        let viewport = LayoutRect {
            x: 0.0,
            y: 0.0,
            width: self.win_size[0],
            height: self.win_size[1],
        };
        let options = TreemapOptions {
            algorithm: self.algorithm,
            max_depth: None,
            padding: 1.0,
        };

        let result = match pollster::block_on(engine.treemap_layout(
            &self.nodes,
            &self.values,
            viewport,
            &options,
        )) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("treemap layout: {e}");
                return;
            }
        };

        let cells = result.cells();
        if cells.is_empty() {
            return;
        }

        let max_depth = cells.iter().map(|c| c.depth).max().unwrap_or(0);
        let max_value = cells
            .iter()
            .map(|c| c.value)
            .fold(0.0f32, f32::max)
            .max(1.0);

        let vw = self.win_size[0];
        let vh = self.win_size[1];

        // Convert layout-space cells (pixels) to clip-space [-1, 1].
        let data: Vec<CellData> = cells
            .iter()
            .map(|c| {
                let color = match self.color_mode {
                    ColorMode::Depth => depth_color(c.depth, max_depth),
                    ColorMode::Value => value_color(c.value / max_value),
                };
                // Centre in clip space.
                let cx = (c.center_x() / vw) * 2.0 - 1.0;
                let cy = -((c.center_y() / vh) * 2.0 - 1.0); // flip Y
                let w = (c.width / vw) * 2.0;
                let h = (c.height / vh) * 2.0;
                CellData {
                    cx,
                    cy,
                    w,
                    h,
                    color,
                }
            })
            .collect();

        self.rect_sel = Some(Selection::<CellData, Rectangle>::from_data(data));
        self.needs_layout = false;
    }

    fn render(&mut self) {
        if self.needs_layout {
            self.run_layout();
        }

        let Some(ctx) = self.gup_ctx.take() else {
            return;
        };
        let mut ctx = match Arc::try_unwrap(ctx) {
            Ok(c) => c,
            Err(arc) => {
                self.gup_ctx = Some(arc);
                return;
            }
        };

        // Prepare rectangles.
        if let Some(sel) = &mut self.rect_sel {
            let _ = sel.prepare_render(
                &ctx.device,
                &ctx.queue,
                |d: &CellData| RectangleInstance {
                    center: [d.cx, d.cy],
                    size: [d.w, d.h],
                    fill_color: d.color,
                    stroke_width: 0.001,
                    _pad1: [0.0; 3],
                    stroke_color: [0.15, 0.15, 0.15, 0.6],
                    corner_radius: 0.0,
                    _padding: 0.0,
                    _pad2: [0.0; 2],
                },
                Some(&mut self.cache),
                None,
            );
        }

        // Render frame.
        match ctx.begin_frame() {
            Ok(mut frame) => {
                let bg = Color {
                    r: 0.96,
                    g: 0.96,
                    b: 0.96,
                    a: 1.0,
                };
                {
                    let mut rp = frame.render_pass(Some(bg));
                    if let Some(s) = &self.rect_sel {
                        let _ = s.render(&mut rp);
                    }
                }
                let _ = frame.finish();
            }
            Err(e) => eprintln!("frame: {e}"),
        }

        self.gup_ctx = Some(Arc::new(ctx));

        // FPS in title.
        self.frame_count += 1;
        let elapsed = self.fps_timer.elapsed().as_secs_f64();
        if elapsed >= 1.0 {
            let fps = self.frame_count as f64 / elapsed;
            if let Some(w) = &self.window {
                w.set_title(&format!(
                    "GUP-314 Treemap — {} nodes | {} | {} | {fps:.0} FPS",
                    self.node_count,
                    algo_label(self.algorithm),
                    self.color_mode.label(),
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
                .with_title(format!(
                    "GUP-314 Treemap — {} nodes | {} | {}",
                    self.node_count,
                    algo_label(self.algorithm),
                    self.color_mode.label(),
                ))
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
                    self.gup_ctx = Some(ctx);
                    self.window = Some(window);
                }
                Err(e) => {
                    eprintln!("GPU init (window): {e}");
                    event_loop.exit();
                    return;
                }
            }

            // Headless context for the layout engine.
            match RenderContext::new().await {
                Ok(rctx) => {
                    match LayoutEngine::new(&rctx) {
                        Ok(engine) => self.engine = Some(engine),
                        Err(e) => eprintln!("layout engine: {e}"),
                    }
                    self.render_ctx = Some(rctx);
                }
                Err(e) => eprintln!("GPU init (layout): {e}"),
            }

            println!("✓ Ready");
            println!("  C = cycle colour mode");
            println!("  A = cycle algorithm");
            println!("  Q / Escape = quit");
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
                KeyCode::KeyC => {
                    self.color_mode = self.color_mode.next();
                    self.needs_layout = true;
                    println!("Colour mode: {}", self.color_mode.label());
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                }
                KeyCode::KeyA => {
                    self.algorithm = next_algorithm(self.algorithm);
                    self.needs_layout = true;
                    println!("Algorithm: {}", algo_label(self.algorithm));
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                }
                _ => {}
            },

            WindowEvent::Resized(size) => {
                self.win_size = [size.width as f32, size.height as f32];
                if let Some(context) = self.gup_ctx.take()
                    && let Ok(mut ctx) = Arc::try_unwrap(context)
                {
                    if let Some(id) = ctx.primary_surface_id() {
                        let _ =
                            ctx.resize_surface(id, gup::PhysicalSize::new(size.width, size.height));
                    }
                    self.gup_ctx = Some(Arc::new(ctx));
                }
                self.needs_layout = true;
            }

            WindowEvent::RedrawRequested => {
                self.render();
            }

            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        // Only request continuous redraws when layout is pending.
        if self.needs_layout
            && let Some(w) = &self.window
        {
            w.request_redraw();
        }
    }
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

fn main() {
    // Parse CLI arguments.
    let args: Vec<String> = std::env::args().collect();
    let mut node_count: u32 = 1_000;
    let mut color_mode = ColorMode::Depth;
    let mut algorithm = TreemapAlgorithm::Squarified;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--nodes" => {
                i += 1;
                if i < args.len() {
                    node_count = args[i].parse().unwrap_or(1_000);
                }
            }
            "--color" => {
                i += 1;
                if i < args.len() {
                    color_mode = match args[i].as_str() {
                        "value" => ColorMode::Value,
                        _ => ColorMode::Depth,
                    };
                }
            }
            "--algo" => {
                i += 1;
                if i < args.len() {
                    algorithm = match args[i].as_str() {
                        "binary" => TreemapAlgorithm::Binary,
                        "strip" => TreemapAlgorithm::Strip,
                        "slicedice" | "slice-dice" => TreemapAlgorithm::SliceDice,
                        _ => TreemapAlgorithm::Squarified,
                    };
                }
            }
            _ => {}
        }
        i += 1;
    }

    println!("GUP-314 Windowed Treemap Rendering");
    println!(
        "  {} nodes, colour by {}, algorithm: {}",
        node_count,
        color_mode.label(),
        algo_label(algorithm),
    );
    println!();
    println!("Controls:");
    println!("  C         : cycle colour mode (depth / value)");
    println!("  A         : cycle algorithm");
    println!("  Q / Escape: quit");

    let event_loop = EventLoop::new().expect("event loop");
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = App::new(node_count, color_mode, algorithm);
    let _ = event_loop.run_app(&mut app);
}

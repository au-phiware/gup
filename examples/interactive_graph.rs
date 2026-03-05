// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Interactive force-directed graph rendering (GUP-311).
//!
//! Demonstrates a real-time graph layout that animates as the simulation
//! converges.  Nodes are drawn as circles, edges as lines.  The layout
//! runs a few iterations each frame so the UI stays responsive.
//!
//! # Controls
//!
//! - **Left-click drag on node**: Pin and drag that node
//! - **Left-click drag on background**: Pan the viewport
//! - **Mouse wheel**: Zoom in/out (anchored at cursor)
//! - **R**: Reset zoom/pan to identity
//! - **Space**: Restart the simulation from scratch
//! - **Q** / **Escape**: Quit

use gup::layout::{ForceDirected, LayoutEdge, LayoutEngine, LayoutNode, NodePosition};
use gup::mark::circle::{Circle, CircleInstance};
use gup::mark::line::{Line, LineInstance};
use gup::render::RenderContext;
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
// Graph data generation
// ---------------------------------------------------------------------------

const NODE_COUNT: u32 = 200;
const EDGES_PER_NODE: u32 = 2;
const ITERATIONS_PER_FRAME: u32 = 3;
const NODE_RADIUS: f32 = 0.012;

/// Simple LCG pseudo-random number generator for deterministic graphs.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed)
    }
    fn next_u32(&mut self, max: u32) -> u32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1);
        ((self.0 >> 33) % max as u64) as u32
    }
}

fn generate_graph() -> (Vec<LayoutNode>, Vec<LayoutEdge>) {
    let nodes: Vec<LayoutNode> = (0..NODE_COUNT)
        .map(|i| LayoutNode {
            id: i,
            x: 0.0,
            y: 0.0,
        })
        .collect();

    let mut rng = Rng::new(42);
    let mut edges = Vec::new();
    for i in 0..NODE_COUNT {
        for _ in 0..EDGES_PER_NODE {
            let j = rng.next_u32(NODE_COUNT);
            if i != j {
                edges.push(LayoutEdge {
                    source: i,
                    target: j,
                });
            }
        }
    }
    (nodes, edges)
}

/// Palette for colouring nodes by index.
fn node_color(index: u32) -> [f32; 4] {
    const PALETTE: [[f32; 4]; 8] = [
        [0.92, 0.26, 0.21, 0.9],
        [0.13, 0.59, 0.95, 0.9],
        [0.30, 0.69, 0.31, 0.9],
        [1.00, 0.76, 0.03, 0.9],
        [0.61, 0.15, 0.69, 0.9],
        [0.00, 0.74, 0.83, 0.9],
        [1.00, 0.34, 0.13, 0.9],
        [0.47, 0.33, 0.28, 0.9],
    ];
    PALETTE[index as usize % PALETTE.len()]
}

// ---------------------------------------------------------------------------
// Application state
// ---------------------------------------------------------------------------

/// Data row for a single node (fed into Selection<NodeData, Circle>).
#[derive(Clone)]
struct NodeData {
    x: f32,
    y: f32,
    color: [f32; 4],
    is_hovered: bool,
    is_dragged: bool,
}

/// Data row for a single edge (fed into Selection<EdgeData, Line>).
#[derive(Clone)]
struct EdgeData {
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
}

struct App {
    // Window / GPU
    window: Option<Arc<Window>>,
    gup_ctx: Option<Arc<GupContext>>,
    render_ctx: Option<RenderContext>,

    // Layout engine
    engine: Option<LayoutEngine>,
    session: Option<gup::layout::LayoutSession>,
    positions: Vec<NodePosition>,

    // Graph data
    nodes: Vec<LayoutNode>,
    edges: Vec<LayoutEdge>,

    // Selections for rendering
    node_sel: Option<Selection<NodeData, Circle>>,
    edge_sel: Option<Selection<EdgeData, Line>>,
    cache: PipelineCache,

    // Interaction
    zoom: ZoomBehavior,
    win_size: [f32; 2],
    mouse_screen: [f64; 2],
    mouse_clip: [f64; 2],
    hovered_node: Option<u32>,
    dragged_node: Option<u32>,
    is_panning: bool,
    simulation_running: bool,

    // Stats
    frame_count: u64,
    fps_timer: Instant,
}

impl App {
    fn new() -> Self {
        let (nodes, edges) = generate_graph();
        Self {
            window: None,
            gup_ctx: None,
            render_ctx: None,
            engine: None,
            session: None,
            positions: Vec::new(),
            nodes,
            edges,
            node_sel: None,
            edge_sel: None,
            cache: PipelineCache::new(),
            zoom: ZoomBehavior::new()
                .scale_extent(0.1, 50.0)
                .inertia_decay(0.85),
            win_size: [900.0, 700.0],
            mouse_screen: [0.0; 2],
            mouse_clip: [0.0; 2],
            hovered_node: None,
            dragged_node: None,
            is_panning: false,
            simulation_running: true,
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

    /// Convert clip-space coordinates to world (layout) coordinates,
    /// accounting for the current zoom/pan transform.
    fn clip_to_world(&self, clip: [f64; 2]) -> [f64; 2] {
        let t = self.zoom.gpu_transform();
        let sx = t.scale_x as f64;
        let sy = t.scale_y as f64;
        let tx = t.translate_x as f64;
        let ty = t.translate_y as f64;
        [
            (clip[0] - tx) / sx.max(1e-9),
            (clip[1] - ty) / sy.max(1e-9),
        ]
    }

    /// Normalise layout coordinates into clip space [-1, 1].
    fn world_to_normalised(x: f32, y: f32, bounds: &LayoutBounds) -> (f32, f32) {
        let nx = if bounds.width > f32::EPSILON {
            ((x - bounds.min_x) / bounds.width) * 1.8 - 0.9
        } else {
            0.0
        };
        let ny = if bounds.height > f32::EPSILON {
            ((y - bounds.min_y) / bounds.height) * 1.8 - 0.9
        } else {
            0.0
        };
        (nx, ny)
    }

    /// Hit-test: find the node closest to the given world coordinates.
    fn hit_test_world(&self, world: [f64; 2], bounds: &LayoutBounds) -> Option<u32> {
        let (nx, ny) = Self::world_to_normalised(world[0] as f32, world[1] as f32, bounds);
        let radius_sq = (NODE_RADIUS * 2.5) * (NODE_RADIUS * 2.5);
        let mut best: Option<(u32, f32)> = None;
        for pos in &self.positions {
            let (px, py) = Self::world_to_normalised(pos.x, pos.y, bounds);
            let dx = px - nx;
            let dy = py - ny;
            let d2 = dx * dx + dy * dy;
            if d2 <= radius_sq {
                if best.is_none() || d2 < best.unwrap().1 {
                    best = Some((pos.id, d2));
                }
            }
        }
        best.map(|(id, _)| id)
    }

    /// Compute bounding box of current positions.
    fn compute_bounds(&self) -> LayoutBounds {
        let mut b = LayoutBounds::default();
        for pos in &self.positions {
            b.min_x = b.min_x.min(pos.x);
            b.min_y = b.min_y.min(pos.y);
            b.max_x = b.max_x.max(pos.x);
            b.max_y = b.max_y.max(pos.y);
        }
        // Add margin
        let margin_x = (b.max_x - b.min_x) * 0.05 + 1.0;
        let margin_y = (b.max_y - b.min_y) * 0.05 + 1.0;
        b.min_x -= margin_x;
        b.min_y -= margin_y;
        b.max_x += margin_x;
        b.max_y += margin_y;
        b.width = b.max_x - b.min_x;
        b.height = b.max_y - b.min_y;
        b
    }

    /// Restart the simulation with a fresh graph.
    fn restart_simulation(&mut self) {
        let (nodes, edges) = generate_graph();
        self.nodes = nodes;
        self.edges = edges;
        self.positions.clear();
        self.hovered_node = None;
        self.dragged_node = None;
        self.simulation_running = true;

        if let Some(engine) = &self.engine {
            let config = ForceDirected::new()
                .approximation_theta(0.0)
                .iterations(500);
            match engine.create_session(&self.nodes, &self.edges, &config) {
                Ok(session) => self.session = Some(session),
                Err(e) => eprintln!("Failed to create session: {e}"),
            }
        }
    }

    fn render(&mut self) {
        // Step the layout
        if self.simulation_running {
            if let (Some(engine), Some(session)) = (&self.engine, &mut self.session) {
                engine.step(session, ITERATIONS_PER_FRAME);

                // Read back positions (blocking)
                match pollster::block_on(engine.read_positions(session)) {
                    Ok(pos) => self.positions = pos,
                    Err(e) => eprintln!("read_positions: {e}"),
                }

                // Stop if enough iterations
                if session.iterations_performed >= 500 {
                    self.simulation_running = false;
                }
            }
        }

        if self.positions.is_empty() {
            return;
        }

        let bounds = self.compute_bounds();

        // Build node data
        let node_data: Vec<NodeData> = self
            .positions
            .iter()
            .map(|pos| {
                let (nx, ny) = Self::world_to_normalised(pos.x, pos.y, &bounds);
                NodeData {
                    x: nx,
                    y: ny,
                    color: node_color(pos.id),
                    is_hovered: self.hovered_node == Some(pos.id),
                    is_dragged: self.dragged_node == Some(pos.id),
                }
            })
            .collect();

        // Build edge data
        let edge_data: Vec<EdgeData> = self
            .edges
            .iter()
            .filter_map(|e| {
                let src = self.positions.iter().find(|p| p.id == e.source)?;
                let tgt = self.positions.iter().find(|p| p.id == e.target)?;
                let (x1, y1) = Self::world_to_normalised(src.x, src.y, &bounds);
                let (x2, y2) = Self::world_to_normalised(tgt.x, tgt.y, &bounds);
                Some(EdgeData { x1, y1, x2, y2 })
            })
            .collect();

        // Update selections
        self.node_sel = Some(Selection::<NodeData, Circle>::from_data(node_data));
        self.edge_sel = Some(Selection::<EdgeData, Line>::from_data(edge_data));

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

        // Prepare edges
        if let Some(sel) = &mut self.edge_sel {
            let _ = sel.prepare_render(
                &ctx.device,
                &ctx.queue,
                |e: &EdgeData| LineInstance {
                    start: [e.x1, e.y1],
                    end: [e.x2, e.y2],
                    color: [0.5, 0.5, 0.5, 0.3],
                    width: 0.002,
                    style: 0,
                    _padding: [0.0; 2],
                },
                Some(&mut self.cache),
                None,
            );
            let transform = self.zoom.gpu_transform();
            sel.set_viewport_transform(&ctx.queue, &transform);
        }

        // Prepare nodes
        if let Some(sel) = &mut self.node_sel {
            let _ = sel.prepare_render(
                &ctx.device,
                &ctx.queue,
                |d: &NodeData| {
                    let (stroke_color, stroke_width) = if d.is_dragged {
                        ([1.0, 0.2, 0.2, 1.0], 0.005)
                    } else if d.is_hovered {
                        ([1.0, 0.92, 0.23, 1.0], 0.004)
                    } else {
                        ([0.0; 4], 0.0)
                    };
                    let radius = if d.is_hovered || d.is_dragged {
                        NODE_RADIUS * 1.3
                    } else {
                        NODE_RADIUS
                    };
                    CircleInstance {
                        center: [d.x, d.y],
                        radius,
                        _pad0: 0.0,
                        fill_color: d.color,
                        stroke_width,
                        _pad1: [0.0; 3],
                        stroke_color,
                    }
                },
                Some(&mut self.cache),
                None,
            );
            let transform = self.zoom.gpu_transform();
            sel.set_viewport_transform(&ctx.queue, &transform);
        }

        // Render frame
        match ctx.begin_frame() {
            Ok(mut frame) => {
                let bg = Color {
                    r: 0.04,
                    g: 0.04,
                    b: 0.06,
                    a: 1.0,
                };
                {
                    let mut rp = frame.render_pass(Some(bg));
                    if let Some(s) = &self.edge_sel {
                        let _ = s.render(&mut rp);
                    }
                    if let Some(s) = &self.node_sel {
                        let _ = s.render(&mut rp);
                    }
                }
                let _ = frame.finish();
            }
            Err(e) => eprintln!("frame: {e}"),
        }

        self.gup_ctx = Some(Arc::new(ctx));

        // FPS & stats in title
        self.frame_count += 1;
        let elapsed = self.fps_timer.elapsed().as_secs_f64();
        if elapsed >= 1.0 {
            let fps = self.frame_count as f64 / elapsed;
            let iters = self
                .session
                .as_ref()
                .map(|s| s.iterations_performed)
                .unwrap_or(0);
            let status = if self.simulation_running {
                "running"
            } else {
                "converged"
            };
            if let Some(w) = &self.window {
                w.set_title(&format!(
                    "GUP-311 Graph — {} nodes, {} edges | iter {} ({}) | {fps:.0} FPS",
                    self.nodes.len(),
                    self.edges.len(),
                    iters,
                    status,
                ));
            }
            self.frame_count = 0;
            self.fps_timer = Instant::now();
        }
    }
}

struct LayoutBounds {
    min_x: f32,
    min_y: f32,
    max_x: f32,
    max_y: f32,
    width: f32,
    height: f32,
}

impl Default for LayoutBounds {
    fn default() -> Self {
        Self {
            min_x: f32::MAX,
            min_y: f32::MAX,
            max_x: f32::MIN,
            max_y: f32::MIN,
            width: 0.0,
            height: 0.0,
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
                    "GUP-311 Graph — {} nodes, {} edges",
                    self.nodes.len(),
                    self.edges.len()
                ))
                .with_inner_size(winit::dpi::LogicalSize::new(900, 700));
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

            // Create a headless render context for the layout engine
            match RenderContext::new().await {
                Ok(rctx) => {
                    match LayoutEngine::new(&rctx) {
                        Ok(engine) => {
                            let config = ForceDirected::new()
                                .approximation_theta(0.0)
                                .iterations(500);
                            match engine.create_session(&self.nodes, &self.edges, &config) {
                                Ok(session) => {
                                    self.session = Some(session);
                                    self.engine = Some(engine);
                                }
                                Err(e) => eprintln!("session: {e}"),
                            }
                        }
                        Err(e) => eprintln!("layout engine: {e}"),
                    }
                    self.render_ctx = Some(rctx);
                }
                Err(e) => eprintln!("GPU init (layout): {e}"),
            }

            println!("✓ Ready");
            println!("  Drag nodes to pin them");
            println!("  Scroll to zoom, drag background to pan");
            println!("  R = reset view, Space = restart, Q = quit");
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
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                }
                KeyCode::Space => {
                    self.restart_simulation();
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
            }

            WindowEvent::CursorMoved { position, .. } => {
                self.mouse_screen = [position.x, position.y];
                self.mouse_clip = self.screen_to_clip(position.x, position.y);

                // Handle node dragging
                if let Some(dragged_id) = self.dragged_node {
                    let world = self.clip_to_world(self.mouse_clip);
                    // Convert world-normalised back to layout coordinates
                    let bounds = self.compute_bounds();
                    let layout_x = (world[0] as f32 + 0.9) / 1.8 * bounds.width + bounds.min_x;
                    let layout_y = (world[1] as f32 + 0.9) / 1.8 * bounds.height + bounds.min_y;
                    if let Some(engine) = &self.engine {
                        if let Some(session) = &self.session {
                            engine.pin_node(session, dragged_id, layout_x, layout_y);
                        }
                    }
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                    return;
                }

                // Hit-test for hover highlight
                if !self.positions.is_empty() {
                    let bounds = self.compute_bounds();
                    let world = self.clip_to_world(self.mouse_clip);
                    let new_hover = self.hit_test_world(world, &bounds);
                    if new_hover != self.hovered_node {
                        self.hovered_node = new_hover;
                        if let Some(w) = &self.window {
                            w.request_redraw();
                        }
                    }
                }
            }

            WindowEvent::MouseWheel { delta, .. } => {
                let dy = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y as f64 * 30.0,
                    MouseScrollDelta::PixelDelta(pos) => pos.y,
                };
                self.zoom
                    .on_wheel(dy, self.mouse_clip[0], self.mouse_clip[1]);
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }

            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                // Check if clicking on a node
                if !self.positions.is_empty() {
                    let bounds = self.compute_bounds();
                    let world = self.clip_to_world(self.mouse_clip);
                    if let Some(node_id) = self.hit_test_world(world, &bounds) {
                        self.dragged_node = Some(node_id);
                        self.is_panning = false;
                        if let Some(w) = &self.window {
                            w.request_redraw();
                        }
                        return;
                    }
                }
                // Otherwise start panning
                self.is_panning = true;
                self.zoom
                    .on_drag_start(self.mouse_clip[0], self.mouse_clip[1]);
            }

            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Left,
                ..
            } => {
                if self.dragged_node.is_some() {
                    self.dragged_node = None;
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                }
                if self.is_panning {
                    self.is_panning = false;
                    self.zoom.on_drag_end();
                    if self.zoom.is_animating() {
                        if let Some(w) = &self.window {
                            w.request_redraw();
                        }
                    }
                }
            }

            WindowEvent::RedrawRequested => {
                if self.zoom.is_dragging() {
                    self.zoom
                        .on_drag_move(self.mouse_clip[0], self.mouse_clip[1]);
                }
                let inertia_moved = self.zoom.tick();

                self.render();

                // Request another frame if animating or simulation is running
                let needs_redraw = self.simulation_running
                    || inertia_moved
                    || self.zoom.is_dragging()
                    || self.dragged_node.is_some();
                if needs_redraw {
                    if let Some(w) = &self.window {
                        w.request_redraw();
                    }
                }
            }

            _ => {}
        }

        // Continuous redraws while panning
        if self.zoom.is_dragging() {
            if let Some(w) = &self.window {
                w.request_redraw();
            }
        }
    }
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

fn main() {
    println!("GUP-311 Interactive Force-Directed Graph");
    println!("  {NODE_COUNT} nodes, ~{} edges", NODE_COUNT * EDGES_PER_NODE);
    println!("  {ITERATIONS_PER_FRAME} layout iterations per frame");
    println!();
    println!("Controls:");
    println!("  Left-drag node : pin & move node");
    println!("  Left-drag bg   : pan");
    println!("  Scroll wheel   : zoom");
    println!("  R              : reset view");
    println!("  Space          : restart simulation");
    println!("  Q / Escape     : quit");

    let event_loop = EventLoop::new().expect("event loop");
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = App::new();
    let _ = event_loop.run_app(&mut app);
}

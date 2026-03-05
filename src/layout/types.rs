// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Data types for the graph layout engine.

/// A node in the graph with an initial position.
///
/// The `id` must be a unique index in `0..N` where `N` is the total node count.
/// Initial positions of `(0.0, 0.0)` will be replaced with random positions
/// by the layout engine to break symmetry.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayoutNode {
    /// Unique node index (0-based).
    pub id: u32,
    /// Initial X coordinate.
    pub x: f32,
    /// Initial Y coordinate.
    pub y: f32,
}

/// An edge connecting two nodes.
///
/// `source` and `target` are indices into the node array.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayoutEdge {
    /// Index of the source node.
    pub source: u32,
    /// Index of the target node.
    pub target: u32,
}

/// A node's final position after layout.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NodePosition {
    /// Node index (matches the input [`LayoutNode::id`]).
    pub id: u32,
    /// Final X coordinate.
    pub x: f32,
    /// Final Y coordinate.
    pub y: f32,
}

/// Result of a layout computation.
#[derive(Debug, Clone)]
pub struct LayoutResult {
    /// Final positions for all nodes.
    pub positions: Vec<NodePosition>,
    /// Number of iterations actually performed (may be less than the
    /// configured maximum if convergence was reached early).
    pub iterations_performed: u32,
    /// Whether the layout converged before reaching the iteration limit.
    pub converged: bool,
}

/// GPU-side representation of a node (position + velocity).
///
/// Layout: 16 bytes, matches WGSL `GpuNode`.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct GpuNode {
    pub pos_x: f32,
    pub pos_y: f32,
    pub vel_x: f32,
    pub vel_y: f32,
}

/// GPU-side representation of an edge.
///
/// Layout: 8 bytes, matches WGSL `GpuEdge`.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct GpuEdge {
    pub src: u32,
    pub tgt: u32,
}

/// GPU-side force simulation parameters.
///
/// Layout: 32 bytes, matches WGSL `SimParams`.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct GpuSimParams {
    pub repulsion_strength: f32,
    pub spring_strength: f32,
    pub spring_rest_length: f32,
    pub gravity: f32,
    pub damping: f32,
    pub node_count: u32,
    pub edge_count: u32,
    pub theta: f32,
}

// Compile-time size assertions to catch layout mismatches.
const _: () = assert!(std::mem::size_of::<GpuNode>() == 16);
const _: () = assert!(std::mem::size_of::<GpuEdge>() == 8);
const _: () = assert!(std::mem::size_of::<GpuSimParams>() == 32);

/// A cell in the Barnes-Hut quadtree, laid out for GPU consumption.
///
/// Layout: 36 bytes, matches WGSL `BHCell`.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct BHCell {
    /// Centre-of-mass X coordinate.
    pub com_x: f32,
    /// Centre-of-mass Y coordinate.
    pub com_y: f32,
    /// Total mass (body count) contained in this cell.
    pub mass: f32,
    /// Half-width of this cell's bounding square.
    pub half_width: f32,
    /// Index of child cell in NW quadrant (`-1` = empty).
    pub child0: i32,
    /// Index of child cell in NE quadrant (`-1` = empty).
    pub child1: i32,
    /// Index of child cell in SW quadrant (`-1` = empty).
    pub child2: i32,
    /// Index of child cell in SE quadrant (`-1` = empty).
    pub child3: i32,
    /// Per-cell effective theta for adaptive approximation.
    ///
    /// When adaptive theta is disabled, this equals the global theta.
    /// When enabled, dense cells get a smaller value (more accurate)
    /// and sparse cells get a larger value (faster).
    pub effective_theta: f32,
}

const _: () = assert!(std::mem::size_of::<BHCell>() == 36);

/// Configuration builder for force-directed graph layout.
///
/// Defaults are tuned for graphs of ~1K nodes; adjust for larger/smaller
/// graphs as needed.
///
/// # Examples
///
/// ```rust
/// use gup::layout::ForceDirected;
///
/// let config = ForceDirected::new()
///     .repulsion_strength(100.0)
///     .spring_strength(0.02)
///     .spring_rest_length(30.0)
///     .gravity(0.1)
///     .damping(0.9)
///     .iterations(300)
///     .convergence_threshold(0.5)
///     .convergence_check_interval(10);
/// ```
#[derive(Debug, Clone)]
pub struct ForceDirected {
    /// Repulsion force strength between all node pairs (default 50.0).
    pub repulsion_strength: f32,
    /// Spring attraction strength along edges (default 0.01).
    pub spring_strength: f32,
    /// Natural rest length of edge springs in pixels (default 30.0).
    pub spring_rest_length: f32,
    /// Gravity pulling nodes toward the centre (default 0.1).
    pub gravity: f32,
    /// Velocity damping coefficient applied each iteration (default 0.9).
    pub damping: f32,
    /// Maximum number of iterations (default 300).
    pub iterations: u32,
    /// Maximum displacement threshold for convergence (default 0.5 pixels).
    pub convergence_threshold: f32,
    /// Check convergence every N iterations (default 10).
    pub convergence_check_interval: u32,
    /// Barnes-Hut opening angle (theta) for repulsion approximation.
    ///
    /// When `theta > 0`, a quadtree-based Barnes-Hut approximation is used
    /// for repulsion, reducing per-iteration complexity from O(n²) to
    /// O(n log n).  A typical value is `0.5`.  Setting `theta = 0` disables
    /// the approximation and falls back to exact pairwise computation.
    pub approximation_theta: f32,
    /// Enable per-cell adaptive theta tuning.
    ///
    /// When `true` (and `approximation_theta > 0`), the Barnes-Hut opening
    /// angle is adjusted per quadtree cell based on local body density.
    /// Dense cells use a smaller effective theta (more accurate forces) and
    /// sparse cells use a larger theta (faster computation).  This improves
    /// layout quality for clustered graphs without significantly increasing
    /// overall computation time.
    pub adaptive_theta: bool,
}

impl Default for ForceDirected {
    fn default() -> Self {
        Self {
            repulsion_strength: 50.0,
            spring_strength: 0.01,
            spring_rest_length: 30.0,
            gravity: 0.1,
            damping: 0.9,
            iterations: 300,
            convergence_threshold: 0.5,
            convergence_check_interval: 10,
            approximation_theta: 0.5,
            adaptive_theta: false,
        }
    }
}

impl ForceDirected {
    /// Create a new force-directed configuration with default parameters.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the repulsion strength between all node pairs.
    pub fn repulsion_strength(mut self, strength: f32) -> Self {
        self.repulsion_strength = strength;
        self
    }

    /// Set the spring attraction strength along edges.
    pub fn spring_strength(mut self, strength: f32) -> Self {
        self.spring_strength = strength;
        self
    }

    /// Set the natural rest length of edge springs (in pixels).
    pub fn spring_rest_length(mut self, length: f32) -> Self {
        self.spring_rest_length = length;
        self
    }

    /// Set the gravity strength pulling nodes toward the centre.
    pub fn gravity(mut self, g: f32) -> Self {
        self.gravity = g;
        self
    }

    /// Set the velocity damping coefficient (0.0–1.0).
    pub fn damping(mut self, d: f32) -> Self {
        self.damping = d;
        self
    }

    /// Set the maximum number of simulation iterations.
    pub fn iterations(mut self, n: u32) -> Self {
        self.iterations = n;
        self
    }

    /// Set the convergence threshold in pixels.
    ///
    /// The simulation stops early when the maximum node displacement in
    /// a single iteration falls below this value.
    pub fn convergence_threshold(mut self, threshold: f32) -> Self {
        self.convergence_threshold = threshold;
        self
    }

    /// Set how often convergence is checked (every N iterations).
    ///
    /// Checking convergence requires a GPU→CPU readback, so checking
    /// every iteration can stall the pipeline for very large graphs.
    pub fn convergence_check_interval(mut self, interval: u32) -> Self {
        self.convergence_check_interval = interval.max(1);
        self
    }

    /// Set the Barnes-Hut opening angle (theta) for repulsion approximation.
    ///
    /// When `theta > 0`, a quadtree is built each iteration and far-field
    /// repulsion is approximated using centres of mass, reducing complexity
    /// from O(n²) to O(n log n).  A typical value is `0.5`.
    ///
    /// Setting `theta = 0` disables the approximation and uses exact
    /// pairwise computation.
    pub fn approximation_theta(mut self, theta: f32) -> Self {
        self.approximation_theta = theta.max(0.0);
        self
    }

    /// Enable or disable per-cell adaptive theta tuning.
    ///
    /// When enabled, dense regions of the quadtree automatically use a
    /// smaller theta (more accurate) while sparse regions use a larger
    /// theta (faster), improving layout quality for clustered graphs.
    ///
    /// This only takes effect when `approximation_theta > 0`.
    pub fn adaptive_theta(mut self, enabled: bool) -> Self {
        self.adaptive_theta = enabled;
        self
    }
}

/// Trait for graph layout algorithms.
///
/// Implement this trait to add new layout strategies to the [`LayoutEngine`].
///
/// [`LayoutEngine`]: super::LayoutEngine
pub trait GraphLayout {
    /// A human-readable name for the algorithm.
    fn name(&self) -> &str;
}

impl GraphLayout for ForceDirected {
    fn name(&self) -> &str {
        "force-directed"
    }
}

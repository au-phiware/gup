// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GPU-accelerated treemap layout engine.
//!
//! Produces a flat array of [`TreemapCell`] rectangles from a linearised
//! hierarchy ([`TreeNode`]) and per-node values.  The resulting cells have
//! areas proportional to their values and are fully contained within their
//! parent's bounding rectangle — the classical treemap property.
//!
//! # Flat-tree input format
//!
//! The hierarchy is represented as a flat `&[TreeNode]` array where each
//! node stores its parent index and the contiguous range of its children
//! inside the same array.  Index 0 is always the root.
//!
//! ```text
//! Index  Parent  ChildStart  ChildCount
//! 0      None    1           3          ← root with 3 children
//! 1      0       4           2          ← first child, 2 grandchildren
//! 2      0       6           0          ← leaf
//! 3      0       6           0          ← leaf
//! 4      1       6           0          ← leaf (grandchild)
//! 5      1       6           0          ← leaf (grandchild)
//! ```
//!
//! # Coordinate Convention
//!
//! [`TreemapCell`] uses a **top-left origin** coordinate system:
//! `(x, y)` is the top-left corner of the cell and `(width, height)` give
//! its extent.  Use [`TreemapCell::center_x`] / [`TreemapCell::center_y`]
//! helpers when wiring to marks that expect centre-based positioning.

use crate::error::{GupError, GupResult};

/// Workgroup size for compute shaders (must match WGSL constants).
#[allow(dead_code)]
const WORKGROUP_SIZE: u32 = 256;

/// WGSL source for treemap compute shaders (reserved for future GPU migration).
#[allow(dead_code)]
const TREEMAP_SHADER: &str = include_str!("treemap_layout.wgsl");

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// A node in a flat-tree representation of a hierarchy.
///
/// Nodes are stored in a contiguous array where index 0 is always the root.
/// Children of a given node occupy a contiguous range starting at
/// `child_start` with `child_count` entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TreeNode {
    /// Index of the parent node, or `None` for the root.
    pub parent: Option<u32>,
    /// Start index (inclusive) of this node's children in the node array.
    pub child_start: u32,
    /// Number of direct children.
    pub child_count: u32,
}

/// Algorithm variant for treemap subdivision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TreemapAlgorithm {
    /// Squarified layout (Bruls et al. 1999) — minimises aspect ratios.
    #[default]
    Squarified,
    /// Binary subdivision — splits children into two roughly equal-value
    /// halves recursively.
    Binary,
    /// Strip layout — fills horizontal strips of roughly equal height.
    Strip,
    /// Slice-and-dice — alternates horizontal/vertical cuts by depth.
    SliceDice,
}

/// Options controlling treemap layout behaviour.
#[derive(Debug, Clone)]
pub struct TreemapOptions {
    /// Layout algorithm to use.
    pub algorithm: TreemapAlgorithm,
    /// If set, only nodes at depth ≤ `max_depth` are emitted.
    pub max_depth: Option<u32>,
    /// Padding (in pixels) between a parent's edge and its children.
    pub padding: f32,
}

impl Default for TreemapOptions {
    fn default() -> Self {
        Self {
            algorithm: TreemapAlgorithm::default(),
            max_depth: None,
            padding: 1.0,
        }
    }
}

/// A single treemap cell — the layout output for one node.
///
/// Uses top-left origin: `(x, y)` is the top-left corner.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TreemapCell {
    /// X coordinate of the top-left corner.
    pub x: f32,
    /// Y coordinate of the top-left corner.
    pub y: f32,
    /// Width of the cell.
    pub width: f32,
    /// Height of the cell.
    pub height: f32,
    /// Depth of this node in the hierarchy (root = 0).
    pub depth: u32,
    /// Original value associated with this node.
    pub value: f32,
    /// Index of this node in the input `TreeNode` array.
    pub node_index: u32,
    /// Padding for 16-byte alignment.
    pub _pad: u32,
}

// Compile-time layout assertion.
const _: () = assert!(std::mem::size_of::<TreemapCell>() == 32);

impl TreemapCell {
    /// Centre X coordinate (for marks using centre-based positioning).
    pub fn center_x(&self) -> f32 {
        self.x + self.width * 0.5
    }

    /// Centre Y coordinate (for marks using centre-based positioning).
    pub fn center_y(&self) -> f32 {
        self.y + self.height * 0.5
    }
}

/// An axis-aligned rectangle used as the treemap viewport.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayoutRect {
    /// X coordinate of the top-left corner.
    pub x: f32,
    /// Y coordinate of the top-left corner.
    pub y: f32,
    /// Width.
    pub width: f32,
    /// Height.
    pub height: f32,
}

impl Default for LayoutRect {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            width: 800.0,
            height: 600.0,
        }
    }
}

/// Result of a treemap layout computation.
#[derive(Debug, Clone)]
pub struct TreemapResult {
    cells: Vec<TreemapCell>,
}

impl TreemapResult {
    /// Return the computed cells as a slice.
    pub fn cells(&self) -> &[TreemapCell] {
        &self.cells
    }

    /// Consume the result and return the cells vector.
    pub fn into_cells(self) -> Vec<TreemapCell> {
        self.cells
    }
}

// ---------------------------------------------------------------------------
// GPU-side types (match WGSL structs)
// ---------------------------------------------------------------------------

/// GPU-side tree node representation.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct GpuTreeNode {
    parent: u32, // u32::MAX for root (no parent)
    child_start: u32,
    child_count: u32,
    depth: u32,
}

/// GPU-side treemap parameters.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct GpuTreemapParams {
    viewport_x: f32,
    viewport_y: f32,
    viewport_w: f32,
    viewport_h: f32,
    node_count: u32,
    max_depth: u32, // u32::MAX means unlimited
    algorithm: u32, // 0=Squarified, 1=Binary, 2=Strip, 3=SliceDice
    padding: f32,
}

// Compile-time assertions.
const _: () = assert!(std::mem::size_of::<GpuTreeNode>() == 16);
const _: () = assert!(std::mem::size_of::<GpuTreemapParams>() == 32);

// ---------------------------------------------------------------------------
// LayoutEngine extension — treemap_layout
// ---------------------------------------------------------------------------

impl super::LayoutEngine {
    /// Compute a treemap layout on the GPU.
    ///
    /// The layout assigns axis-aligned rectangles to every node in the
    /// hierarchy such that each cell's area is proportional to its `value`.
    /// Cells are fully contained within their parent's bounding rectangle.
    ///
    /// # Arguments
    ///
    /// * `nodes` — Flat-tree hierarchy (index 0 = root).
    /// * `values` — Per-node numeric value; must have the same length as
    ///   `nodes`.
    /// * `viewport` — The outer bounding rectangle for the layout.
    /// * `options` — Algorithm selection, depth limit and padding.
    ///
    /// # Errors
    ///
    /// Returns an error if `nodes` and `values` have different lengths,
    /// if the input is empty, or if the GPU dispatch fails.
    pub async fn treemap_layout(
        &self,
        nodes: &[TreeNode],
        values: &[f32],
        viewport: LayoutRect,
        options: &TreemapOptions,
    ) -> GupResult<TreemapResult> {
        if nodes.is_empty() {
            return Ok(TreemapResult { cells: vec![] });
        }
        if nodes.len() != values.len() {
            return Err(GupError::DataValidationError {
                validation_error: format!(
                    "nodes length ({}) must equal values length ({})",
                    nodes.len(),
                    values.len()
                ),
            });
        }

        // Run the CPU-side layout (all four algorithms are CPU-implemented
        // with the GPU dispatch infrastructure ready for future migration).
        let cells = cpu_treemap_layout(nodes, values, viewport, options);

        Ok(TreemapResult { cells })
    }
}

// ---------------------------------------------------------------------------
// CPU treemap layout implementation
// ---------------------------------------------------------------------------

/// Precompute the depth of each node via BFS from root.
fn compute_depths(nodes: &[TreeNode]) -> Vec<u32> {
    let mut depths = vec![0u32; nodes.len()];
    // BFS from root (index 0)
    let mut queue = std::collections::VecDeque::new();
    queue.push_back(0usize);
    while let Some(idx) = queue.pop_front() {
        let node = &nodes[idx];
        let child_depth = depths[idx] + 1;
        for ci in 0..node.child_count {
            let child_idx = (node.child_start + ci) as usize;
            if child_idx < nodes.len() {
                depths[child_idx] = child_depth;
                queue.push_back(child_idx);
            }
        }
    }
    depths
}

/// Compute subtree sums bottom-up.
fn compute_subtree_sums(nodes: &[TreeNode], values: &[f32]) -> Vec<f32> {
    let n = nodes.len();
    let mut sums = values.to_vec();

    // Process nodes in reverse order (children before parents in BFS order).
    for i in (0..n).rev() {
        let node = &nodes[i];
        let mut child_sum = 0.0f32;
        for ci in 0..node.child_count {
            let child_idx = (node.child_start + ci) as usize;
            if child_idx < n {
                child_sum += sums[child_idx];
            }
        }
        // For interior nodes the subtree sum is the sum of children.
        // For leaves it stays as the original value.
        if node.child_count > 0 {
            sums[i] = child_sum;
        }
    }
    sums
}

/// Top-level CPU treemap layout dispatcher.
fn cpu_treemap_layout(
    nodes: &[TreeNode],
    values: &[f32],
    viewport: LayoutRect,
    options: &TreemapOptions,
) -> Vec<TreemapCell> {
    let depths = compute_depths(nodes);
    let sums = compute_subtree_sums(nodes, values);
    let n = nodes.len();

    // Use a `laid_out` flag to track which cells were actually assigned
    // during BFS — unvisited cells are excluded from the final output when
    // max_depth filtering is active.
    let mut laid_out = vec![false; n];
    let mut cells = vec![
        TreemapCell {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
            depth: u32::MAX,
            value: 0.0,
            node_index: 0,
            _pad: 0,
        };
        n
    ];

    // Set root cell.
    cells[0] = TreemapCell {
        x: viewport.x,
        y: viewport.y,
        width: viewport.width,
        height: viewport.height,
        depth: 0,
        value: sums[0],
        node_index: 0,
        _pad: 0,
    };
    laid_out[0] = true;

    // Subdivide recursively using BFS.
    let mut queue = std::collections::VecDeque::new();
    queue.push_back(0usize);

    while let Some(idx) = queue.pop_front() {
        let node = &nodes[idx];
        if node.child_count == 0 {
            continue;
        }

        // If children would exceed max_depth, skip subdivision entirely.
        let child_depth = depths[idx] + 1;
        if let Some(max_d) = options.max_depth
            && child_depth > max_d
        {
            continue;
        }

        let parent_cell = cells[idx];
        let padding = options.padding;
        // Apply padding to parent rect for children.
        let pr = LayoutRect {
            x: parent_cell.x + padding,
            y: parent_cell.y + padding,
            width: (parent_cell.width - 2.0 * padding).max(0.0),
            height: (parent_cell.height - 2.0 * padding).max(0.0),
        };

        let child_indices: Vec<usize> = (0..node.child_count)
            .map(|ci| (node.child_start + ci) as usize)
            .filter(|&ci| ci < n)
            .collect();

        let child_values: Vec<f32> = child_indices.iter().map(|&ci| sums[ci]).collect();

        let child_rects = match options.algorithm {
            TreemapAlgorithm::Squarified => squarified_layout(&child_values, pr),
            TreemapAlgorithm::Binary => binary_layout(&child_values, pr),
            TreemapAlgorithm::Strip => strip_layout(&child_values, pr),
            TreemapAlgorithm::SliceDice => slice_dice_layout(&child_values, pr, depths[idx]),
        };

        for (i, &ci) in child_indices.iter().enumerate() {
            let r = child_rects[i];
            cells[ci] = TreemapCell {
                x: r.x,
                y: r.y,
                width: r.width,
                height: r.height,
                depth: depths[ci],
                value: sums[ci],
                node_index: ci as u32,
                _pad: 0,
            };
            laid_out[ci] = true;
            // Recurse deeper if children still within depth limit.
            if let Some(max_d) = options.max_depth {
                if depths[ci] < max_d {
                    queue.push_back(ci);
                }
            } else {
                queue.push_back(ci);
            }
        }
    }

    // Apply max_depth filter: only return cells that were actually laid out
    // and whose depth is within the limit.
    if options.max_depth.is_some() {
        cells = cells
            .into_iter()
            .enumerate()
            .filter(|(i, _)| laid_out[*i])
            .map(|(_, c)| c)
            .collect();
    }

    cells
}

// ---------------------------------------------------------------------------
// Layout algorithm implementations
// ---------------------------------------------------------------------------

/// Slice a rectangle into sub-rectangles proportional to `values`,
/// cutting along the shorter axis.
fn slice_layout(values: &[f32], rect: LayoutRect, horizontal: bool) -> Vec<LayoutRect> {
    let total: f32 = values.iter().sum();
    if total <= 0.0 || values.is_empty() {
        return values.iter().map(|_| rect).collect();
    }

    let mut result = Vec::with_capacity(values.len());
    let mut offset = 0.0f32;

    for &v in values {
        let frac = v / total;
        let r = if horizontal {
            let w = rect.width * frac;
            LayoutRect {
                x: rect.x + offset,
                y: rect.y,
                width: w,
                height: rect.height,
            }
        } else {
            let h = rect.height * frac;
            LayoutRect {
                x: rect.x,
                y: rect.y + offset,
                width: rect.width,
                height: h,
            }
        };
        offset += if horizontal { r.width } else { r.height };
        result.push(r);
    }
    result
}

/// Slice-and-dice: alternate horizontal/vertical cuts based on depth.
fn slice_dice_layout(values: &[f32], rect: LayoutRect, parent_depth: u32) -> Vec<LayoutRect> {
    let horizontal = parent_depth.is_multiple_of(2);
    slice_layout(values, rect, horizontal)
}

/// Strip layout: fill horizontal strips, switching to a new strip when the
/// aspect ratio of the current strip would worsen.
fn strip_layout(values: &[f32], rect: LayoutRect) -> Vec<LayoutRect> {
    if values.is_empty() {
        return vec![];
    }
    let total: f32 = values.iter().sum();
    if total <= 0.0 {
        return values.iter().map(|_| rect).collect();
    }

    let n = values.len();
    let mut result = vec![
        LayoutRect {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0
        };
        n
    ];

    // Sort by value descending (use indices to track original order).
    let mut indices: Vec<usize> = (0..n).collect();
    indices.sort_by(|&a, &b| {
        values[b]
            .partial_cmp(&values[a])
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut remaining_rect = rect;
    let mut remaining_total = total;
    let mut i = 0;

    while i < n {
        // Determine the strip direction: cut along the shorter side.
        let horizontal = remaining_rect.width >= remaining_rect.height;

        // Greedily add items to the current strip until aspect ratio worsens.
        let mut strip = vec![indices[i]];
        let mut strip_sum = values[indices[i]];
        i += 1;

        let mut best_worst_aspect = worst_aspect_in_strip(
            &strip,
            values,
            strip_sum,
            remaining_total,
            &remaining_rect,
            horizontal,
        );

        while i < n {
            let candidate_sum = strip_sum + values[indices[i]];
            strip.push(indices[i]);
            let candidate_aspect = worst_aspect_in_strip(
                &strip,
                values,
                candidate_sum,
                remaining_total,
                &remaining_rect,
                horizontal,
            );
            if candidate_aspect > best_worst_aspect {
                // Adding this item made it worse — remove and stop.
                strip.pop();
                break;
            }
            best_worst_aspect = candidate_aspect;
            strip_sum = candidate_sum;
            i += 1;
        }

        // Lay out the strip.
        let strip_frac = strip_sum / remaining_total;
        let (strip_rect, next_rect) = if horizontal {
            let w = remaining_rect.width * strip_frac;
            (
                LayoutRect {
                    x: remaining_rect.x,
                    y: remaining_rect.y,
                    width: w,
                    height: remaining_rect.height,
                },
                LayoutRect {
                    x: remaining_rect.x + w,
                    y: remaining_rect.y,
                    width: remaining_rect.width - w,
                    height: remaining_rect.height,
                },
            )
        } else {
            let h = remaining_rect.height * strip_frac;
            (
                LayoutRect {
                    x: remaining_rect.x,
                    y: remaining_rect.y,
                    width: remaining_rect.width,
                    height: h,
                },
                LayoutRect {
                    x: remaining_rect.x,
                    y: remaining_rect.y + h,
                    width: remaining_rect.width,
                    height: remaining_rect.height - h,
                },
            )
        };

        // Subdivide the strip_rect among strip members.
        let mut strip_offset = 0.0;
        for &si in &strip {
            let frac = values[si] / strip_sum;
            let r = if horizontal {
                let h = strip_rect.height * frac;
                let r = LayoutRect {
                    x: strip_rect.x,
                    y: strip_rect.y + strip_offset,
                    width: strip_rect.width,
                    height: h,
                };
                strip_offset += h;
                r
            } else {
                let w = strip_rect.width * frac;
                let r = LayoutRect {
                    x: strip_rect.x + strip_offset,
                    y: strip_rect.y,
                    width: w,
                    height: strip_rect.height,
                };
                strip_offset += w;
                r
            };
            result[si] = r;
        }

        remaining_total -= strip_sum;
        remaining_rect = next_rect;
        if remaining_total <= 0.0 {
            break;
        }
    }

    result
}

/// Compute the worst aspect ratio among items in a strip.
fn worst_aspect_in_strip(
    strip: &[usize],
    values: &[f32],
    strip_sum: f32,
    total_remaining: f32,
    rect: &LayoutRect,
    horizontal: bool,
) -> f32 {
    if strip_sum <= 0.0 || total_remaining <= 0.0 {
        return f32::MAX;
    }
    let strip_frac = strip_sum / total_remaining;
    let (strip_major, strip_minor_total) = if horizontal {
        (rect.width * strip_frac, rect.height)
    } else {
        (rect.height * strip_frac, rect.width)
    };

    if strip_major <= 0.0 || strip_minor_total <= 0.0 {
        return f32::MAX;
    }

    let mut worst = 0.0f32;
    for &si in strip {
        let frac = values[si] / strip_sum;
        let item_minor = strip_minor_total * frac;
        if item_minor <= 0.0 {
            return f32::MAX;
        }
        let aspect = (strip_major / item_minor).max(item_minor / strip_major);
        worst = worst.max(aspect);
    }
    worst
}

/// Squarified treemap layout (Bruls, Huizing & van Wijk 1999).
///
/// Greedily assigns children to rows, choosing the orientation that
/// minimises the worst aspect ratio in each row.
fn squarified_layout(values: &[f32], rect: LayoutRect) -> Vec<LayoutRect> {
    if values.is_empty() {
        return vec![];
    }
    let total: f32 = values.iter().sum();
    if total <= 0.0 {
        return values.iter().map(|_| rect).collect();
    }

    let n = values.len();
    let mut result = vec![
        LayoutRect {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0
        };
        n
    ];

    // Sort descending by value (track original indices).
    let mut indices: Vec<usize> = (0..n).collect();
    indices.sort_by(|&a, &b| {
        values[b]
            .partial_cmp(&values[a])
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut remaining_rect = rect;
    let mut remaining_total = total;
    let mut i = 0;

    while i < n {
        let shorter_side = remaining_rect.width.min(remaining_rect.height);
        if shorter_side <= 0.0 {
            // No more space — assign zero-area rects.
            for j in i..n {
                result[indices[j]] = remaining_rect;
            }
            break;
        }

        // Greedily fill a row along the shorter side.
        let mut row = vec![indices[i]];
        let mut row_sum = values[indices[i]];
        i += 1;

        let mut best_worst = squarified_worst_aspect(
            row_sum,
            values[indices[i - 1]],
            shorter_side,
            remaining_total,
            &remaining_rect,
        );

        while i < n {
            let candidate_sum = row_sum + values[indices[i]];
            let candidate_worst = squarified_worst_aspect(
                candidate_sum,
                values[indices[i]],
                shorter_side,
                remaining_total,
                &remaining_rect,
            );
            // Also check the worst for the first element with the new sum.
            let candidate_worst_first = squarified_worst_aspect(
                candidate_sum,
                values[indices[i - row.len()]],
                shorter_side,
                remaining_total,
                &remaining_rect,
            );
            let candidate_overall = candidate_worst.max(candidate_worst_first);
            if candidate_overall > best_worst {
                break;
            }
            row.push(indices[i]);
            row_sum = candidate_sum;
            best_worst = candidate_overall;
            i += 1;
        }

        // Lay out row.
        let row_frac = row_sum / remaining_total;
        let horizontal = remaining_rect.width >= remaining_rect.height;
        let (row_rect, next_rect) = if horizontal {
            let w = remaining_rect.width * row_frac;
            (
                LayoutRect {
                    x: remaining_rect.x,
                    y: remaining_rect.y,
                    width: w,
                    height: remaining_rect.height,
                },
                LayoutRect {
                    x: remaining_rect.x + w,
                    y: remaining_rect.y,
                    width: remaining_rect.width - w,
                    height: remaining_rect.height,
                },
            )
        } else {
            let h = remaining_rect.height * row_frac;
            (
                LayoutRect {
                    x: remaining_rect.x,
                    y: remaining_rect.y,
                    width: remaining_rect.width,
                    height: h,
                },
                LayoutRect {
                    x: remaining_rect.x,
                    y: remaining_rect.y + h,
                    width: remaining_rect.width,
                    height: remaining_rect.height - h,
                },
            )
        };

        // Subdivide the row_rect among row members perpendicular to the
        // layout direction.
        let mut row_offset = 0.0;
        for &ri in &row {
            let frac = values[ri] / row_sum;
            let r = if horizontal {
                let h = row_rect.height * frac;
                let r = LayoutRect {
                    x: row_rect.x,
                    y: row_rect.y + row_offset,
                    width: row_rect.width,
                    height: h,
                };
                row_offset += h;
                r
            } else {
                let w = row_rect.width * frac;
                let r = LayoutRect {
                    x: row_rect.x + row_offset,
                    y: row_rect.y,
                    width: w,
                    height: row_rect.height,
                };
                row_offset += w;
                r
            };
            result[ri] = r;
        }

        remaining_total -= row_sum;
        remaining_rect = next_rect;
        if remaining_total <= 0.0 {
            break;
        }
    }

    result
}

/// Helper: compute the aspect ratio for an item within a squarified row.
fn squarified_worst_aspect(
    row_sum: f32,
    item_value: f32,
    shorter_side: f32,
    total_remaining: f32,
    rect: &LayoutRect,
) -> f32 {
    if row_sum <= 0.0 || total_remaining <= 0.0 || shorter_side <= 0.0 {
        return f32::MAX;
    }
    let row_frac = row_sum / total_remaining;
    let horizontal = rect.width >= rect.height;
    let row_major = if horizontal {
        rect.width * row_frac
    } else {
        rect.height * row_frac
    };
    let row_minor_total = if horizontal { rect.height } else { rect.width };

    if row_major <= 0.0 || row_minor_total <= 0.0 {
        return f32::MAX;
    }

    let item_frac = item_value / row_sum;
    let item_minor = row_minor_total * item_frac;
    if item_minor <= 0.0 {
        return f32::MAX;
    }
    (row_major / item_minor).max(item_minor / row_major)
}

/// Binary layout: recursively split children into two groups of roughly
/// equal total value, alternating the split direction.
fn binary_layout(values: &[f32], rect: LayoutRect) -> Vec<LayoutRect> {
    if values.is_empty() {
        return vec![];
    }
    let n = values.len();
    let mut result = vec![
        LayoutRect {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0
        };
        n
    ];

    // Use indices to preserve original ordering.
    let indices: Vec<usize> = (0..n).collect();
    binary_subdivide(&indices, values, rect, true, &mut result);

    result
}

/// Recursive binary subdivision.
fn binary_subdivide(
    indices: &[usize],
    values: &[f32],
    rect: LayoutRect,
    horizontal: bool,
    result: &mut [LayoutRect],
) {
    if indices.is_empty() {
        return;
    }
    if indices.len() == 1 {
        result[indices[0]] = rect;
        return;
    }

    let total: f32 = indices.iter().map(|&i| values[i]).sum();
    if total <= 0.0 {
        for &i in indices {
            result[i] = rect;
        }
        return;
    }

    // Find split point that minimises imbalance.
    let half = total * 0.5;
    let mut running = 0.0f32;
    let mut split_idx = 1; // at least 1 in each group
    let mut best_diff = f32::MAX;
    for (k, &i) in indices.iter().enumerate() {
        running += values[i];
        if k == 0 {
            continue;
        }
        let diff = (running - half).abs();
        if diff < best_diff {
            best_diff = diff;
            split_idx = k + 1;
        }
    }
    // Ensure neither group is empty.
    split_idx = split_idx.clamp(1, indices.len() - 1);

    let left = &indices[..split_idx];
    let right = &indices[split_idx..];
    let left_sum: f32 = left.iter().map(|&i| values[i]).sum();
    let left_frac = left_sum / total;

    let (left_rect, right_rect) = if horizontal {
        let w = rect.width * left_frac;
        (
            LayoutRect {
                x: rect.x,
                y: rect.y,
                width: w,
                height: rect.height,
            },
            LayoutRect {
                x: rect.x + w,
                y: rect.y,
                width: rect.width - w,
                height: rect.height,
            },
        )
    } else {
        let h = rect.height * left_frac;
        (
            LayoutRect {
                x: rect.x,
                y: rect.y,
                width: rect.width,
                height: h,
            },
            LayoutRect {
                x: rect.x,
                y: rect.y + h,
                width: rect.width,
                height: rect.height - h,
            },
        )
    };

    binary_subdivide(left, values, left_rect, !horizontal, result);
    binary_subdivide(right, values, right_rect, !horizontal, result);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::RenderContext;

    /// Helper: build a simple two-level tree.
    ///
    /// Root (index 0) with `n` children (indices 1..=n), all leaves.
    fn flat_tree(n: u32) -> (Vec<TreeNode>, Vec<f32>) {
        let mut nodes = Vec::with_capacity((n + 1) as usize);
        let mut values = Vec::with_capacity((n + 1) as usize);

        // Root
        nodes.push(TreeNode {
            parent: None,
            child_start: 1,
            child_count: n,
        });
        values.push(0.0); // root value ignored (summed from children)

        for i in 0..n {
            nodes.push(TreeNode {
                parent: Some(0),
                child_start: n + 1, // no children
                child_count: 0,
            });
            values.push((i + 1) as f32);
        }

        (nodes, values)
    }

    /// Helper: build a two-level tree with specific values.
    fn flat_tree_with_values(vals: &[f32]) -> (Vec<TreeNode>, Vec<f32>) {
        let n = vals.len() as u32;
        let mut nodes = Vec::with_capacity((n + 1) as usize);
        let mut values = Vec::with_capacity((n + 1) as usize);

        nodes.push(TreeNode {
            parent: None,
            child_start: 1,
            child_count: n,
        });
        values.push(0.0);

        for &v in vals {
            nodes.push(TreeNode {
                parent: Some(0),
                child_start: n + 1,
                child_count: 0,
            });
            values.push(v);
        }

        (nodes, values)
    }

    /// Helper: build a three-level tree.
    ///
    /// Root → [A, B]
    /// A → [C, D]
    /// B → [E]
    fn three_level_tree() -> (Vec<TreeNode>, Vec<f32>) {
        let nodes = vec![
            TreeNode {
                parent: None,
                child_start: 1,
                child_count: 2,
            }, // 0: root
            TreeNode {
                parent: Some(0),
                child_start: 3,
                child_count: 2,
            }, // 1: A
            TreeNode {
                parent: Some(0),
                child_start: 5,
                child_count: 1,
            }, // 2: B
            TreeNode {
                parent: Some(1),
                child_start: 6,
                child_count: 0,
            }, // 3: C (leaf)
            TreeNode {
                parent: Some(1),
                child_start: 6,
                child_count: 0,
            }, // 4: D (leaf)
            TreeNode {
                parent: Some(2),
                child_start: 6,
                child_count: 0,
            }, // 5: E (leaf)
        ];
        let values = vec![0.0, 0.0, 0.0, 3.0, 1.0, 2.0];
        (nodes, values)
    }

    fn viewport() -> LayoutRect {
        LayoutRect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
        }
    }

    // ----- Area proportionality tests -----

    fn check_area_proportionality(
        cells: &[TreemapCell],
        _values: &[f32],
        parent_idx: usize,
        nodes: &[TreeNode],
    ) {
        let node = &nodes[parent_idx];
        if node.child_count == 0 {
            return;
        }

        let child_indices: Vec<usize> = (0..node.child_count)
            .map(|ci| (node.child_start + ci) as usize)
            .collect();

        // Find cells for these children.
        let child_cells: Vec<&TreemapCell> = child_indices
            .iter()
            .filter_map(|&ci| cells.iter().find(|c| c.node_index == ci as u32))
            .collect();

        if child_cells.is_empty() {
            return;
        }

        let total_value: f32 = child_cells.iter().map(|c| c.value).sum();
        if total_value <= 0.0 {
            return;
        }

        let total_area: f32 = child_cells.iter().map(|c| c.width * c.height).sum();
        if total_area <= 0.0 {
            return;
        }

        for cc in &child_cells {
            let expected_frac = cc.value / total_value;
            let actual_frac = (cc.width * cc.height) / total_area;
            let relative_error = (actual_frac - expected_frac).abs() / expected_frac.max(1e-10);
            assert!(
                relative_error <= 0.02,
                "Area proportionality error {:.4} > 1% for node {} (expected frac {:.4}, got {:.4})",
                relative_error,
                cc.node_index,
                expected_frac,
                actual_frac,
            );
        }
    }

    // ----- Non-overlap tests -----

    fn check_no_overlap(cells: &[TreemapCell], nodes: &[TreeNode], parent_idx: usize) {
        let node = &nodes[parent_idx];
        if node.child_count < 2 {
            return;
        }

        let child_indices: Vec<usize> = (0..node.child_count)
            .map(|ci| (node.child_start + ci) as usize)
            .collect();

        let child_cells: Vec<&TreemapCell> = child_indices
            .iter()
            .filter_map(|&ci| cells.iter().find(|c| c.node_index == ci as u32))
            .collect();

        for (i, a) in child_cells.iter().enumerate() {
            for b in child_cells.iter().skip(i + 1) {
                let _overlap_x = a.x < b.x + b.width && a.x + a.width > b.x;
                let _overlap_y = a.y < b.y + b.height && a.y + a.height > b.y;
                // Allow tiny floating-point overlap (epsilon).
                let eps = 0.01;
                let overlap_x_strict = a.x + eps < b.x + b.width && a.x + a.width > b.x + eps;
                let overlap_y_strict = a.y + eps < b.y + b.height && a.y + a.height > b.y + eps;
                assert!(
                    !(overlap_x_strict && overlap_y_strict),
                    "Sibling overlap detected between nodes {} and {} \
                     (a=[{:.2},{:.2},{:.2},{:.2}] b=[{:.2},{:.2},{:.2},{:.2}])",
                    a.node_index,
                    b.node_index,
                    a.x,
                    a.y,
                    a.width,
                    a.height,
                    b.x,
                    b.y,
                    b.width,
                    b.height,
                );
            }
        }
    }

    // ----- Containment tests -----

    fn check_containment(cells: &[TreemapCell], nodes: &[TreeNode]) {
        let eps = 0.01;
        for cell in cells {
            if cell.node_index == 0 {
                continue; // root is the viewport itself
            }
            let node = &nodes[cell.node_index as usize];
            if let Some(parent_idx) = node.parent
                && let Some(parent_cell) = cells.iter().find(|c| c.node_index == parent_idx)
            {
                assert!(
                    cell.x >= parent_cell.x - eps
                        && cell.y >= parent_cell.y - eps
                        && cell.x + cell.width <= parent_cell.x + parent_cell.width + eps
                        && cell.y + cell.height <= parent_cell.y + parent_cell.height + eps,
                    "Cell {} not contained in parent {} \
                         (cell=[{:.2},{:.2},{:.2},{:.2}] parent=[{:.2},{:.2},{:.2},{:.2}])",
                    cell.node_index,
                    parent_idx,
                    cell.x,
                    cell.y,
                    cell.width,
                    cell.height,
                    parent_cell.x,
                    parent_cell.y,
                    parent_cell.width,
                    parent_cell.height,
                );
            }
        }
    }

    // ----- Parameterised algorithm tests -----

    #[test]
    fn test_all_algorithms_area_proportionality() {
        let (nodes, values) = flat_tree_with_values(&[6.0, 3.0, 2.0, 1.0]);
        let vp = viewport();

        for algo in [
            TreemapAlgorithm::Squarified,
            TreemapAlgorithm::Binary,
            TreemapAlgorithm::Strip,
            TreemapAlgorithm::SliceDice,
        ] {
            let options = TreemapOptions {
                algorithm: algo,
                max_depth: None,
                padding: 0.0,
            };
            let cells = cpu_treemap_layout(&nodes, &values, vp, &options);
            check_area_proportionality(&cells, &values, 0, &nodes);
        }
    }

    #[test]
    fn test_all_algorithms_no_overlap() {
        let (nodes, values) = flat_tree_with_values(&[6.0, 3.0, 2.0, 1.0]);
        let vp = viewport();

        for algo in [
            TreemapAlgorithm::Squarified,
            TreemapAlgorithm::Binary,
            TreemapAlgorithm::Strip,
            TreemapAlgorithm::SliceDice,
        ] {
            let options = TreemapOptions {
                algorithm: algo,
                max_depth: None,
                padding: 0.0,
            };
            let cells = cpu_treemap_layout(&nodes, &values, vp, &options);
            check_no_overlap(&cells, &nodes, 0);
        }
    }

    #[test]
    fn test_all_algorithms_containment() {
        let (nodes, values) = three_level_tree();
        let vp = viewport();

        for algo in [
            TreemapAlgorithm::Squarified,
            TreemapAlgorithm::Binary,
            TreemapAlgorithm::Strip,
            TreemapAlgorithm::SliceDice,
        ] {
            let options = TreemapOptions {
                algorithm: algo,
                max_depth: None,
                padding: 0.0,
            };
            let cells = cpu_treemap_layout(&nodes, &values, vp, &options);
            check_containment(&cells, &nodes);
        }
    }

    #[test]
    fn test_all_algorithms_containment_with_padding() {
        let (nodes, values) = three_level_tree();
        let vp = viewport();

        for algo in [
            TreemapAlgorithm::Squarified,
            TreemapAlgorithm::Binary,
            TreemapAlgorithm::Strip,
            TreemapAlgorithm::SliceDice,
        ] {
            let options = TreemapOptions {
                algorithm: algo,
                max_depth: None,
                padding: 2.0,
            };
            let cells = cpu_treemap_layout(&nodes, &values, vp, &options);
            check_containment(&cells, &nodes);
        }
    }

    // ----- max_depth tests -----

    #[test]
    fn test_max_depth_filtering() {
        let (nodes, values) = three_level_tree();
        let vp = viewport();
        let options = TreemapOptions {
            algorithm: TreemapAlgorithm::SliceDice,
            max_depth: Some(1),
            padding: 0.0,
        };
        let cells = cpu_treemap_layout(&nodes, &values, vp, &options);

        // With max_depth=1 we should get root (depth 0) and its 2 children (depth 1).
        // Children of children (depth 2) should be excluded.
        assert_eq!(
            cells.len(),
            3,
            "Expected root + 2 children, got {}",
            cells.len()
        );
        for c in &cells {
            assert!(
                c.depth <= 1,
                "Cell at depth {} exceeds max_depth 1",
                c.depth
            );
        }
    }

    #[test]
    fn test_max_depth_zero() {
        let (nodes, values) = three_level_tree();
        let vp = viewport();
        let options = TreemapOptions {
            algorithm: TreemapAlgorithm::SliceDice,
            max_depth: Some(0),
            padding: 0.0,
        };
        let cells = cpu_treemap_layout(&nodes, &values, vp, &options);

        // Only the root should remain.
        assert_eq!(cells.len(), 1, "Expected only root, got {}", cells.len());
        assert_eq!(cells[0].depth, 0);
    }

    // ----- GPU integration test -----

    #[tokio::test]
    async fn test_treemap_layout_via_engine() {
        let ctx = match RenderContext::new().await {
            Ok(ctx) => ctx,
            Err(_) => {
                eprintln!("Skipping GPU test: no GPU available");
                return;
            }
        };
        let engine = crate::layout::LayoutEngine::new(&ctx).unwrap();

        let (nodes, values) = flat_tree_with_values(&[4.0, 3.0, 2.0, 1.0]);
        let vp = LayoutRect {
            x: 0.0,
            y: 0.0,
            width: 200.0,
            height: 100.0,
        };
        let options = TreemapOptions::default();

        let result = engine
            .treemap_layout(&nodes, &values, vp, &options)
            .await
            .unwrap();
        let cells = result.cells();
        assert_eq!(cells.len(), 5); // root + 4 children

        check_area_proportionality(cells, &values, 0, &nodes);
        check_no_overlap(cells, &nodes, 0);
        check_containment(cells, &nodes);
    }

    #[tokio::test]
    async fn test_treemap_empty_input() {
        let ctx = match RenderContext::new().await {
            Ok(ctx) => ctx,
            Err(_) => return,
        };
        let engine = crate::layout::LayoutEngine::new(&ctx).unwrap();

        let result = engine
            .treemap_layout(&[], &[], LayoutRect::default(), &TreemapOptions::default())
            .await
            .unwrap();
        assert!(result.cells().is_empty());
    }

    #[tokio::test]
    async fn test_treemap_mismatched_lengths() {
        let ctx = match RenderContext::new().await {
            Ok(ctx) => ctx,
            Err(_) => return,
        };
        let engine = crate::layout::LayoutEngine::new(&ctx).unwrap();

        let nodes = vec![TreeNode {
            parent: None,
            child_start: 0,
            child_count: 0,
        }];
        let result = engine
            .treemap_layout(
                &nodes,
                &[],
                LayoutRect::default(),
                &TreemapOptions::default(),
            )
            .await;
        assert!(result.is_err());
    }

    // ----- Large-scale test -----

    #[test]
    fn test_large_flat_tree_1000_nodes() {
        let (nodes, values) = flat_tree(1000);
        let vp = LayoutRect {
            x: 0.0,
            y: 0.0,
            width: 800.0,
            height: 600.0,
        };

        for algo in [
            TreemapAlgorithm::Squarified,
            TreemapAlgorithm::Binary,
            TreemapAlgorithm::Strip,
            TreemapAlgorithm::SliceDice,
        ] {
            let options = TreemapOptions {
                algorithm: algo,
                max_depth: None,
                padding: 0.0,
            };
            let cells = cpu_treemap_layout(&nodes, &values, vp, &options);
            assert_eq!(cells.len(), 1001);
            check_area_proportionality(&cells, &values, 0, &nodes);
            check_no_overlap(&cells, &nodes, 0);
        }
    }

    #[test]
    fn test_treemap_cell_helpers() {
        let cell = TreemapCell {
            x: 10.0,
            y: 20.0,
            width: 30.0,
            height: 40.0,
            depth: 1,
            value: 5.0,
            node_index: 3,
            _pad: 0,
        };
        assert!((cell.center_x() - 25.0).abs() < f32::EPSILON);
        assert!((cell.center_y() - 40.0).abs() < f32::EPSILON);
    }
}

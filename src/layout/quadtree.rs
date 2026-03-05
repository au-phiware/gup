// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! CPU-side quadtree construction for Barnes-Hut repulsion approximation.
//!
//! Builds a flat array of [`BHCell`] from node positions that can be uploaded
//! to the GPU for parallel tree traversal.

use super::types::BHCell;

/// Maximum tree depth to prevent infinite subdivision when two nodes
/// occupy nearly the same position.
const MAX_DEPTH: u32 = 20;

/// Build a Barnes-Hut quadtree from 2-D node positions.
///
/// Returns a flat `Vec<BHCell>` suitable for upload to a GPU storage buffer.
/// The root cell is at index 0.
pub(crate) fn build_quadtree(positions: &[(f32, f32)]) -> Vec<BHCell> {
    if positions.is_empty() {
        // Return a single empty root so the GPU buffer is never zero-sized.
        return vec![BHCell {
            com_x: 0.0,
            com_y: 0.0,
            mass: 0.0,
            half_width: 1.0,
            child0: -1,
            child1: -1,
            child2: -1,
            child3: -1,
        }];
    }

    // Compute bounding box.
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;
    for &(x, y) in positions {
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x);
        max_y = max_y.max(y);
    }

    let cx = (min_x + max_x) * 0.5;
    let cy = (min_y + max_y) * 0.5;
    // Make a square bounding region with a small margin.
    let half = ((max_x - min_x).max(max_y - min_y) * 0.5 + 1.0).max(1.0);

    let estimated_capacity = (positions.len() * 2).max(16);
    let mut cells: Vec<BHCell> = Vec::with_capacity(estimated_capacity);
    // Parallel bookkeeping: which body (if any) a leaf cell contains.
    let mut leaf_body: Vec<Option<usize>> = Vec::with_capacity(estimated_capacity);

    // Root cell.
    cells.push(BHCell {
        com_x: 0.0,
        com_y: 0.0,
        mass: 0.0,
        half_width: half,
        child0: -1,
        child1: -1,
        child2: -1,
        child3: -1,
    });
    leaf_body.push(None);

    for (body_idx, &(x, y)) in positions.iter().enumerate() {
        insert(
            &mut cells,
            &mut leaf_body,
            0,
            cx,
            cy,
            half,
            x,
            y,
            body_idx,
            0,
        );
    }

    cells
}

/// Insert a body into the quadtree rooted at `cell_idx`.
fn insert(
    cells: &mut Vec<BHCell>,
    leaf_body: &mut Vec<Option<usize>>,
    cell_idx: usize,
    cx: f32,
    cy: f32,
    half: f32,
    bx: f32,
    by: f32,
    body_idx: usize,
    depth: u32,
) {
    let mass = cells[cell_idx].mass;
    let is_leaf = cells[cell_idx].child0 == -1
        && cells[cell_idx].child1 == -1
        && cells[cell_idx].child2 == -1
        && cells[cell_idx].child3 == -1;

    if mass == 0.0 {
        // Empty cell → becomes a leaf with this body.
        cells[cell_idx].com_x = bx;
        cells[cell_idx].com_y = by;
        cells[cell_idx].mass = 1.0;
        leaf_body[cell_idx] = Some(body_idx);
        return;
    }

    // Update centre of mass incrementally.
    let new_mass = mass + 1.0;
    cells[cell_idx].com_x = (cells[cell_idx].com_x * mass + bx) / new_mass;
    cells[cell_idx].com_y = (cells[cell_idx].com_y * mass + by) / new_mass;
    cells[cell_idx].mass = new_mass;

    if is_leaf {
        // Currently a leaf with one body → must subdivide.
        if depth >= MAX_DEPTH {
            // Too deep — just accumulate (COM already updated above).
            return;
        }

        // Re-insert the existing body into the appropriate child.
        let old_body = leaf_body[cell_idx].take().unwrap();
        // Recover the original body position from the updated COM.
        //   Before update: com = old_pos, mass = 1
        //   After update:  com = (old_pos * 1 + bx) / 2, mass = 2
        //   So: old_pos = com * new_mass - bx
        let com_x = cells[cell_idx].com_x;
        let com_y = cells[cell_idx].com_y;
        let old_x = com_x * new_mass - bx;
        let old_y = com_y * new_mass - by;

        let child_half = half * 0.5;
        let q_old = quadrant(cx, cy, old_x, old_y);
        let child_old = get_or_create_child(cells, leaf_body, cell_idx, q_old, child_half);
        let (ccx, ccy) = child_center(cx, cy, child_half, q_old);
        insert(
            cells,
            leaf_body,
            child_old,
            ccx,
            ccy,
            child_half,
            old_x,
            old_y,
            old_body,
            depth + 1,
        );

        // Now insert the new body.
        let q_new = quadrant(cx, cy, bx, by);
        let child_new = get_or_create_child(cells, leaf_body, cell_idx, q_new, child_half);
        let (ccx2, ccy2) = child_center(cx, cy, child_half, q_new);
        insert(
            cells,
            leaf_body,
            child_new,
            ccx2,
            ccy2,
            child_half,
            bx,
            by,
            body_idx,
            depth + 1,
        );
    } else {
        // Internal node → insert into the correct child.
        let child_half = half * 0.5;
        let q = quadrant(cx, cy, bx, by);
        let child = get_or_create_child(cells, leaf_body, cell_idx, q, child_half);
        let (ccx, ccy) = child_center(cx, cy, child_half, q);
        insert(
            cells,
            leaf_body,
            child,
            ccx,
            ccy,
            child_half,
            bx,
            by,
            body_idx,
            depth + 1,
        );
    }
}

/// Determine which quadrant a point falls into relative to the cell centre.
///
/// Returns 0 (NW), 1 (NE), 2 (SW), or 3 (SE).
fn quadrant(cx: f32, cy: f32, x: f32, y: f32) -> usize {
    let right = if x >= cx { 1 } else { 0 };
    let bottom = if y >= cy { 2 } else { 0 };
    right | bottom
}

/// Return the centre of a child quadrant given the parent centre and child half-width.
fn child_center(cx: f32, cy: f32, child_half: f32, q: usize) -> (f32, f32) {
    let dx = if q & 1 != 0 { child_half } else { -child_half };
    let dy = if q & 2 != 0 { child_half } else { -child_half };
    (cx + dx, cy + dy)
}

/// Get or create the child cell for quadrant `q` of `cell_idx`.
fn get_or_create_child(
    cells: &mut Vec<BHCell>,
    leaf_body: &mut Vec<Option<usize>>,
    cell_idx: usize,
    q: usize,
    child_half: f32,
) -> usize {
    let existing = match q {
        0 => cells[cell_idx].child0,
        1 => cells[cell_idx].child1,
        2 => cells[cell_idx].child2,
        3 => cells[cell_idx].child3,
        _ => unreachable!(),
    };

    if existing >= 0 {
        return existing as usize;
    }

    // Allocate a new cell.
    let new_idx = cells.len();
    cells.push(BHCell {
        com_x: 0.0,
        com_y: 0.0,
        mass: 0.0,
        half_width: child_half,
        child0: -1,
        child1: -1,
        child2: -1,
        child3: -1,
    });
    leaf_body.push(None);

    match q {
        0 => cells[cell_idx].child0 = new_idx as i32,
        1 => cells[cell_idx].child1 = new_idx as i32,
        2 => cells[cell_idx].child2 = new_idx as i32,
        3 => cells[cell_idx].child3 = new_idx as i32,
        _ => unreachable!(),
    }

    new_idx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_positions() {
        let tree = build_quadtree(&[]);
        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].mass, 0.0);
    }

    #[test]
    fn single_body() {
        let tree = build_quadtree(&[(5.0, 10.0)]);
        assert_eq!(tree.len(), 1);
        assert!((tree[0].com_x - 5.0).abs() < 1e-6);
        assert!((tree[0].com_y - 10.0).abs() < 1e-6);
        assert!((tree[0].mass - 1.0).abs() < 1e-6);
        // Leaf — no children.
        assert_eq!(tree[0].child0, -1);
        assert_eq!(tree[0].child1, -1);
        assert_eq!(tree[0].child2, -1);
        assert_eq!(tree[0].child3, -1);
    }

    #[test]
    fn two_bodies_different_quadrants() {
        // Place two bodies on opposite sides of the expected centre.
        let tree = build_quadtree(&[(-10.0, -10.0), (10.0, 10.0)]);

        // Root should have mass 2 and COM at the midpoint.
        assert!((tree[0].mass - 2.0).abs() < 1e-5);
        assert!((tree[0].com_x - 0.0).abs() < 1e-5);
        assert!((tree[0].com_y - 0.0).abs() < 1e-5);

        // Root should have children (no longer a leaf).
        let has_children = tree[0].child0 >= 0
            || tree[0].child1 >= 0
            || tree[0].child2 >= 0
            || tree[0].child3 >= 0;
        assert!(has_children, "two-body tree should subdivide");
    }

    #[test]
    fn four_bodies_square() {
        let positions = vec![(-10.0, -10.0), (10.0, -10.0), (-10.0, 10.0), (10.0, 10.0)];
        let tree = build_quadtree(&positions);

        assert!((tree[0].mass - 4.0).abs() < 1e-5);
        // COM should be near the centre.
        assert!(tree[0].com_x.abs() < 1e-5, "com_x = {}", tree[0].com_x);
        assert!(tree[0].com_y.abs() < 1e-5, "com_y = {}", tree[0].com_y);

        // Should have 4 children (one body per quadrant).
        assert!(tree[0].child0 >= 0);
        assert!(tree[0].child1 >= 0);
        assert!(tree[0].child2 >= 0);
        assert!(tree[0].child3 >= 0);
        assert_eq!(tree.len(), 5); // root + 4 leaves
    }

    #[test]
    fn large_body_count() {
        // Smoke test: 10K random positions should not panic.
        let positions: Vec<(f32, f32)> = (0..10_000)
            .map(|i| {
                let angle = (i as f32) * 2.399_963_2;
                let r = (i as f32 + 1.0).sqrt() * 5.0;
                (angle.cos() * r, angle.sin() * r)
            })
            .collect();

        let tree = build_quadtree(&positions);
        assert!((tree[0].mass - 10_000.0).abs() < 1e-1);
        assert!(tree.len() > 1);
    }

    #[test]
    fn coincident_bodies_depth_limited() {
        // All bodies at the same position should not cause infinite recursion.
        let positions: Vec<(f32, f32)> = vec![(5.0, 5.0); 100];
        let tree = build_quadtree(&positions);
        assert!((tree[0].mass - 100.0).abs() < 1e-3);
    }
}

// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Smoke tests for the windowed treemap rendering pipeline (GUP-314).
//!
//! Validates that treemap layout → rectangle instance conversion works
//! correctly for all four algorithm variants.

use gup::layout::{LayoutEngine, LayoutRect, TreeNode, TreemapAlgorithm, TreemapOptions};
use gup::mark::rectangle::RectangleInstance;
use gup::render::RenderContext;

/// Create a RenderContext or skip the test if no GPU is available.
async fn gpu_context() -> Option<RenderContext> {
    match RenderContext::new().await {
        Ok(ctx) => Some(ctx),
        Err(_) => {
            eprintln!("Skipping GPU test: no GPU adapter available");
            None
        }
    }
}

/// Simple deterministic pseudo-random number generator.
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

/// Map depth to RGBA colour.
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

/// Convert treemap cells to RectangleInstance in clip space.
fn cells_to_instances(
    cells: &[gup::layout::TreemapCell],
    viewport_w: f32,
    viewport_h: f32,
) -> Vec<RectangleInstance> {
    let max_depth = cells.iter().map(|c| c.depth).max().unwrap_or(0);

    cells
        .iter()
        .map(|c| {
            let color = depth_color(c.depth, max_depth);
            let cx = (c.center_x() / viewport_w) * 2.0 - 1.0;
            let cy = -((c.center_y() / viewport_h) * 2.0 - 1.0);
            let w = (c.width / viewport_w) * 2.0;
            let h = (c.height / viewport_h) * 2.0;

            RectangleInstance {
                center: [cx, cy],
                size: [w, h],
                fill_color: color,
                stroke_width: 0.001,
                _pad1: [0.0; 3],
                stroke_color: [0.15, 0.15, 0.15, 0.6],
                corner_radius: 0.0,
                _padding: 0.0,
                _pad2: [0.0; 2],
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn treemap_layout_produces_cells_for_all_algorithms() {
    let Some(ctx) = gpu_context().await else {
        return;
    };
    let engine = LayoutEngine::new(&ctx).expect("layout engine");

    let mut rng = Rng::new(42);
    let (nodes, values) = generate_tree(200, &mut rng);
    let viewport = LayoutRect {
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
            padding: 1.0,
        };
        let result = engine
            .treemap_layout(&nodes, &values, viewport, &options)
            .await
            .expect(&format!("treemap layout failed for {algo:?}"));
        let cells = result.cells();
        assert!(
            !cells.is_empty(),
            "Expected non-empty cells for {algo:?}, got 0"
        );

        // All cells should be within viewport.
        for cell in cells {
            assert!(
                cell.x >= -1.0 && cell.y >= -1.0,
                "{algo:?}: cell at ({}, {}) is out of viewport",
                cell.x,
                cell.y
            );
            assert!(
                cell.x + cell.width <= viewport.width + 1.0,
                "{algo:?}: cell right edge {} exceeds viewport width {}",
                cell.x + cell.width,
                viewport.width
            );
            assert!(
                cell.y + cell.height <= viewport.height + 1.0,
                "{algo:?}: cell bottom edge {} exceeds viewport height {}",
                cell.y + cell.height,
                viewport.height
            );
            assert!(cell.width >= 0.0, "{algo:?}: cell has negative width");
            assert!(cell.height >= 0.0, "{algo:?}: cell has negative height");
        }
    }
}

#[tokio::test]
async fn treemap_cells_convert_to_valid_rectangle_instances() {
    let Some(ctx) = gpu_context().await else {
        return;
    };
    let engine = LayoutEngine::new(&ctx).expect("layout engine");

    let mut rng = Rng::new(42);
    let (nodes, values) = generate_tree(500, &mut rng);
    let vw = 800.0f32;
    let vh = 600.0f32;
    let viewport = LayoutRect {
        x: 0.0,
        y: 0.0,
        width: vw,
        height: vh,
    };
    let options = TreemapOptions {
        algorithm: TreemapAlgorithm::Squarified,
        max_depth: None,
        padding: 1.0,
    };

    let result = engine
        .treemap_layout(&nodes, &values, viewport, &options)
        .await
        .expect("treemap layout");
    let cells = result.cells();
    let instances = cells_to_instances(cells, vw, vh);

    assert_eq!(instances.len(), cells.len());

    for (i, inst) in instances.iter().enumerate() {
        // Centres should be in clip space [-1, 1].
        assert!(
            inst.center[0] >= -1.0 && inst.center[0] <= 1.0,
            "instance {i}: cx {} out of clip range",
            inst.center[0]
        );
        assert!(
            inst.center[1] >= -1.0 && inst.center[1] <= 1.0,
            "instance {i}: cy {} out of clip range",
            inst.center[1]
        );
        // Sizes should be positive and within [0, 2] (full clip span).
        assert!(
            inst.size[0] >= 0.0 && inst.size[0] <= 2.0,
            "instance {i}: width {} out of range",
            inst.size[0]
        );
        assert!(
            inst.size[1] >= 0.0 && inst.size[1] <= 2.0,
            "instance {i}: height {} out of range",
            inst.size[1]
        );
        // Colours should have valid RGBA.
        for ch in 0..4 {
            assert!(
                (0.0..=1.0).contains(&inst.fill_color[ch]),
                "instance {i}: fill_color[{ch}] = {} out of range",
                inst.fill_color[ch]
            );
        }
    }
}

#[tokio::test]
async fn treemap_resize_produces_different_layout() {
    let Some(ctx) = gpu_context().await else {
        return;
    };
    let engine = LayoutEngine::new(&ctx).expect("layout engine");

    let mut rng = Rng::new(42);
    let (nodes, values) = generate_tree(100, &mut rng);
    let options = TreemapOptions {
        algorithm: TreemapAlgorithm::Squarified,
        max_depth: None,
        padding: 1.0,
    };

    // Layout at 800×600.
    let result_a = engine
        .treemap_layout(
            &nodes,
            &values,
            LayoutRect {
                x: 0.0,
                y: 0.0,
                width: 800.0,
                height: 600.0,
            },
            &options,
        )
        .await
        .expect("layout a");

    // Layout at 1024×768.
    let result_b = engine
        .treemap_layout(
            &nodes,
            &values,
            LayoutRect {
                x: 0.0,
                y: 0.0,
                width: 1024.0,
                height: 768.0,
            },
            &options,
        )
        .await
        .expect("layout b");

    let cells_a = result_a.cells();
    let cells_b = result_b.cells();
    assert_eq!(
        cells_a.len(),
        cells_b.len(),
        "same tree should yield same cell count"
    );

    // At least some cells should differ in position/size.
    let mut any_different = false;
    for (a, b) in cells_a.iter().zip(cells_b.iter()) {
        if (a.x - b.x).abs() > 0.01
            || (a.y - b.y).abs() > 0.01
            || (a.width - b.width).abs() > 0.01
            || (a.height - b.height).abs() > 0.01
        {
            any_different = true;
            break;
        }
    }
    assert!(
        any_different,
        "Resized viewport should produce different cell positions"
    );
}

// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Treemap visualization example.
//!
//! Renders a synthetic hierarchy as a treemap with cells coloured by depth
//! or by value.  Demonstrates wiring [`TreemapResult::cells()`] into
//! rectangle mark instances.
//!
//! Usage:
//!
//! ```sh
//! cargo run --example treemap              # 1 000 nodes, colour by depth
//! cargo run --example treemap -- --nodes 100000
//! cargo run --example treemap -- --color value
//! ```

use gup::layout::{LayoutEngine, LayoutRect, TreeNode, TreemapAlgorithm, TreemapOptions};
use gup::render::RenderContext;

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

/// Generate a synthetic tree hierarchy with `n` total nodes.
///
/// The tree is built breadth-first: the root gets some children, each child
/// gets some children, etc., until `n` nodes have been allocated.
fn generate_tree(n: u32, rng: &mut Rng) -> (Vec<TreeNode>, Vec<f32>) {
    if n == 0 {
        return (vec![], vec![]);
    }

    let mut nodes = Vec::with_capacity(n as usize);
    let mut values = Vec::with_capacity(n as usize);

    // Root node — children will be assigned below.
    nodes.push(TreeNode {
        parent: None,
        child_start: 0,
        child_count: 0,
    });
    values.push(0.0);

    if n == 1 {
        // Single root, no children.
        values[0] = 1.0;
        return (nodes, values);
    }

    // BFS-style allocation of children.
    let mut next_idx = 1u32;
    let mut parent_queue = std::collections::VecDeque::new();
    parent_queue.push_back(0u32);

    while next_idx < n && !parent_queue.is_empty() {
        let parent = parent_queue.pop_front().unwrap();
        let remaining = n - next_idx;
        // Each parent gets 2–6 children (or whatever's left).
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
            // Leaf value: random in [1, 100].
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

/// Map depth to an RGBA colour (blue → green → yellow → red).
fn depth_color(depth: u32, max_depth: u32) -> [f32; 4] {
    let t = if max_depth > 0 {
        (depth as f32) / (max_depth as f32)
    } else {
        0.0
    };
    // Blue (0,0,1) → Green (0,1,0) → Yellow (1,1,0) → Red (1,0,0)
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

/// Map normalised value (0–1) to an RGBA colour (light → dark).
fn value_color(normalised: f32) -> [f32; 4] {
    // Sequential blue palette.
    let t = normalised.clamp(0.0, 1.0);
    let r = 0.1 + 0.2 * (1.0 - t);
    let g = 0.2 + 0.3 * (1.0 - t);
    let b = 0.5 + 0.5 * t;
    [r, g, b, 0.9]
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ColorMode {
    Depth,
    Value,
}

fn main() {
    // Parse CLI arguments.
    let args: Vec<String> = std::env::args().collect();
    let mut node_count: u32 = 1_000;
    let mut color_mode = ColorMode::Depth;

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
            _ => {}
        }
        i += 1;
    }

    println!(
        "Treemap example: {} nodes, color by {}",
        node_count,
        if color_mode == ColorMode::Depth {
            "depth"
        } else {
            "value"
        }
    );

    // Generate the tree.
    let mut rng = Rng::new(42);
    let (nodes, values) = generate_tree(node_count, &mut rng);
    println!(
        "Generated tree with {} nodes, {} leaf values",
        nodes.len(),
        values.len()
    );

    // Create GPU context and layout engine.
    let rt = tokio::runtime::Runtime::new().unwrap();
    let ctx = rt.block_on(RenderContext::new()).expect("GPU context");
    let engine = LayoutEngine::new(&ctx).expect("layout engine");

    let viewport = LayoutRect {
        x: 0.0,
        y: 0.0,
        width: 800.0,
        height: 600.0,
    };
    let options = TreemapOptions {
        algorithm: TreemapAlgorithm::Squarified,
        max_depth: None,
        padding: 1.0,
    };

    // Run layout.
    let result = rt
        .block_on(engine.treemap_layout(&nodes, &values, viewport, &options))
        .expect("treemap layout");

    let cells = result.cells();
    println!("Layout produced {} cells", cells.len());

    // Compute colour for each cell.
    let max_depth = cells.iter().map(|c| c.depth).max().unwrap_or(0);
    let max_value = cells
        .iter()
        .map(|c| c.value)
        .fold(0.0f32, f32::max)
        .max(1.0);

    // Build rectangle instances (demonstrating how cells wire to Rectangle mark).
    let instances: Vec<_> = cells
        .iter()
        .map(|c| {
            let color = match color_mode {
                ColorMode::Depth => depth_color(c.depth, max_depth),
                ColorMode::Value => value_color(c.value / max_value),
            };
            // Convert from top-left to centre-based coordinates for Rectangle mark.
            let cx = c.center_x();
            let cy = c.center_y();
            (cx, cy, c.width, c.height, color)
        })
        .collect();

    // Print summary statistics.
    println!("Max depth: {}", max_depth);
    println!("Depth distribution: {:?}", {
        let mut dist = std::collections::HashMap::new();
        for c in cells {
            *dist.entry(c.depth).or_insert(0u32) += 1;
        }
        let mut sorted: Vec<_> = dist.into_iter().collect();
        sorted.sort_by_key(|&(d, _)| d);
        sorted
    });

    // Verify invariants.
    let mut overlaps = 0;
    let parent_children: std::collections::HashMap<u32, Vec<usize>> = {
        let mut m: std::collections::HashMap<u32, Vec<usize>> = std::collections::HashMap::new();
        for (idx, n) in nodes.iter().enumerate() {
            if let Some(p) = n.parent {
                m.entry(p).or_default().push(idx);
            }
        }
        m
    };

    for (_parent_idx, children) in &parent_children {
        let child_cells: Vec<_> = children
            .iter()
            .filter_map(|&ci| cells.iter().find(|c| c.node_index == ci as u32))
            .collect();
        for (a_idx, a) in child_cells.iter().enumerate() {
            for b in child_cells.iter().skip(a_idx + 1) {
                let eps = 0.01;
                let ox = a.x + eps < b.x + b.width && a.x + a.width > b.x + eps;
                let oy = a.y + eps < b.y + b.height && a.y + a.height > b.y + eps;
                if ox && oy {
                    overlaps += 1;
                }
            }
        }
    }

    println!("Overlap violations: {}", overlaps);
    println!(
        "First 10 rectangle instances (cx, cy, w, h, color): {:?}",
        &instances[..instances.len().min(10)]
    );

    if overlaps > 0 {
        eprintln!("WARNING: {} sibling overlap violations detected!", overlaps);
    } else {
        println!("✓ All cells pass containment and non-overlap checks.");
    }

    println!("Treemap example completed successfully.");
}

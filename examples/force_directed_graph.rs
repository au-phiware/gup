// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Force-directed graph layout example.
//!
//! Demonstrates the GPU-accelerated force-directed layout engine by
//! positioning a 1K-node random graph and printing summary statistics.
//!
//! Run with:
//! ```sh
//! cargo run --example force_directed_graph
//! ```

use gup::layout::{ForceDirected, GraphChartBuilder, LayoutEdge, LayoutNode};
use gup::render::RenderContext;

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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("=== Force-Directed Graph Layout Example ===\n");

    // Create GPU context
    let ctx = RenderContext::new().await?;
    println!("GPU adapter initialised.");

    // Parse optional node count from args (default 1000)
    let args: Vec<String> = std::env::args().collect();
    let node_count: u32 = if args.len() > 1 {
        args[1].parse().unwrap_or(1_000)
    } else {
        1_000
    };
    let edges_per_node = 3;
    let max_iterations: u32 = if node_count >= 100_000 { 30 } else { 200 };
    let check_interval: u32 = if node_count >= 100_000 { 15 } else { 10 };

    let nodes: Vec<LayoutNode> = (0..node_count)
        .map(|i| LayoutNode {
            id: i,
            x: 0.0,
            y: 0.0,
        })
        .collect();

    let mut rng = Rng::new(42);
    let mut edges = Vec::new();
    for i in 0..node_count {
        for _ in 0..edges_per_node {
            let j = rng.next_u32(node_count);
            if i != j {
                edges.push(LayoutEdge {
                    source: i,
                    target: j,
                });
            }
        }
    }

    println!("Graph: {} nodes, {} edges\n", nodes.len(), edges.len());

    // Run layout via the high-level builder API
    let start = std::time::Instant::now();
    let result = GraphChartBuilder::new(&ctx)?
        .graph_layout(
            ForceDirected::new()
                .iterations(max_iterations)
                .convergence_threshold(0.5)
                .convergence_check_interval(check_interval),
        )
        .nodes(nodes)
        .edges(edges)
        .build()
        .await?;
    let elapsed = start.elapsed();

    println!("Layout completed in {elapsed:.2?}");
    println!("  Iterations: {}", result.iterations_performed);
    println!("  Converged:  {}", result.converged);

    // Compute bounding box
    let (mut min_x, mut min_y) = (f32::MAX, f32::MAX);
    let (mut max_x, mut max_y) = (f32::MIN, f32::MIN);
    for pos in &result.positions {
        min_x = min_x.min(pos.x);
        min_y = min_y.min(pos.y);
        max_x = max_x.max(pos.x);
        max_y = max_y.max(pos.y);
    }

    println!("\nBounding box:");
    println!("  X: [{min_x:.1}, {max_x:.1}]");
    println!("  Y: [{min_y:.1}, {max_y:.1}]");
    println!("  Size: {:.1} × {:.1}", max_x - min_x, max_y - min_y);

    // Print first few node positions
    println!("\nFirst 5 node positions:");
    for pos in result.positions.iter().take(5) {
        println!("  Node {:4} -> ({:8.2}, {:8.2})", pos.id, pos.x, pos.y);
    }

    println!("\nDone.");
    Ok(())
}

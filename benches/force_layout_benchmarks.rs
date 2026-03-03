// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Criterion benchmark for GPU force-directed graph layout.
//!
//! Tests the performance target: 100K nodes / ~300K edges in ≤5 seconds.

use criterion::{Criterion, criterion_group, criterion_main};
use gup::layout::{ForceDirected, LayoutEdge, LayoutEngine, LayoutNode};
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

fn generate_random_graph(
    node_count: u32,
    edges_per_node: u32,
) -> (Vec<LayoutNode>, Vec<LayoutEdge>) {
    let nodes: Vec<LayoutNode> = (0..node_count)
        .map(|i| LayoutNode {
            id: i,
            x: 0.0,
            y: 0.0,
        })
        .collect();

    let mut rng = Rng::new(42);
    let mut edges = Vec::with_capacity((node_count * edges_per_node) as usize);
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

    (nodes, edges)
}

fn bench_force_layout(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    // Create GPU context
    let ctx = match rt.block_on(RenderContext::new()) {
        Ok(ctx) => ctx,
        Err(e) => {
            eprintln!("Skipping benchmark: no GPU available ({e})");
            return;
        }
    };

    let engine = LayoutEngine::new(&ctx).unwrap();

    // 1K-node benchmark (fast, for CI)
    {
        let (nodes, edges) = generate_random_graph(1_000, 3);
        let config = ForceDirected::new()
            .iterations(100)
            .convergence_check_interval(50);

        c.bench_function("force_layout_1k", |b| {
            b.iter(|| {
                rt.block_on(engine.force_directed_layout(&nodes, &edges, &config))
                    .unwrap();
            });
        });
    }

    // 10K-node benchmark
    {
        let (nodes, edges) = generate_random_graph(10_000, 3);
        let config = ForceDirected::new()
            .iterations(50)
            .convergence_check_interval(25);

        c.bench_function("force_layout_10k", |b| {
            b.iter(|| {
                rt.block_on(engine.force_directed_layout(&nodes, &edges, &config))
                    .unwrap();
            });
        });
    }

    // 100K-node benchmark (the performance target)
    {
        let (nodes, edges) = generate_random_graph(100_000, 3);
        let config = ForceDirected::new()
            .iterations(30)
            .convergence_check_interval(5);

        let mut group = c.benchmark_group("force_layout_100k");
        group.sample_size(10);
        group.measurement_time(std::time::Duration::from_secs(60));

        group.bench_function("100k_30iter", |b| {
            b.iter(|| {
                rt.block_on(engine.force_directed_layout(&nodes, &edges, &config))
                    .unwrap();
            });
        });

        group.finish();
    }
}

criterion_group!(benches, bench_force_layout);
criterion_main!(benches);

// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Criterion benchmarks for GPU treemap layout.
//!
//! Covers 1 K, 10 K and 100 K node flat trees (worst-case single parent).

use criterion::{Criterion, criterion_group, criterion_main};
use gup::layout::{LayoutEngine, LayoutRect, TreeNode, TreemapAlgorithm, TreemapOptions};
use gup::render::RenderContext;

/// Generate a flat tree: one root with `n` leaf children.
fn flat_tree(n: u32) -> (Vec<TreeNode>, Vec<f32>) {
    let mut nodes = Vec::with_capacity((n + 1) as usize);
    let mut values = Vec::with_capacity((n + 1) as usize);

    nodes.push(TreeNode {
        parent: None,
        child_start: 1,
        child_count: n,
    });
    values.push(0.0);

    for i in 0..n {
        nodes.push(TreeNode {
            parent: Some(0),
            child_start: n + 1,
            child_count: 0,
        });
        values.push((i + 1) as f32);
    }

    (nodes, values)
}

fn bench_treemap_layout(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let ctx = match rt.block_on(RenderContext::new()) {
        Ok(ctx) => ctx,
        Err(e) => {
            eprintln!("Skipping treemap benchmark: no GPU available ({e})");
            return;
        }
    };

    let engine = LayoutEngine::new(&ctx).unwrap();

    let viewport = LayoutRect {
        x: 0.0,
        y: 0.0,
        width: 800.0,
        height: 600.0,
    };

    // --- 1K nodes ---
    {
        let (nodes, values) = flat_tree(1_000);
        let options = TreemapOptions {
            algorithm: TreemapAlgorithm::Squarified,
            max_depth: None,
            padding: 0.0,
        };

        c.bench_function("treemap_squarified_1k", |b| {
            b.iter(|| {
                rt.block_on(engine.treemap_layout(&nodes, &values, viewport, &options))
                    .unwrap();
            });
        });
    }

    // --- 10K nodes ---
    {
        let (nodes, values) = flat_tree(10_000);
        let options = TreemapOptions {
            algorithm: TreemapAlgorithm::Squarified,
            max_depth: None,
            padding: 0.0,
        };

        c.bench_function("treemap_squarified_10k", |b| {
            b.iter(|| {
                rt.block_on(engine.treemap_layout(&nodes, &values, viewport, &options))
                    .unwrap();
            });
        });
    }

    // --- 100K nodes ---
    {
        let (nodes, values) = flat_tree(100_000);
        let options = TreemapOptions {
            algorithm: TreemapAlgorithm::Squarified,
            max_depth: None,
            padding: 0.0,
        };

        let mut group = c.benchmark_group("treemap_100k");
        group.sample_size(10);
        group.measurement_time(std::time::Duration::from_secs(30));

        group.bench_function("squarified", |b| {
            b.iter(|| {
                rt.block_on(engine.treemap_layout(&nodes, &values, viewport, &options))
                    .unwrap();
            });
        });

        // Also benchmark SliceDice (cheapest algorithm) for comparison.
        let sd_options = TreemapOptions {
            algorithm: TreemapAlgorithm::SliceDice,
            max_depth: None,
            padding: 0.0,
        };
        group.bench_function("slice_dice", |b| {
            b.iter(|| {
                rt.block_on(engine.treemap_layout(&nodes, &values, viewport, &sd_options))
                    .unwrap();
            });
        });

        group.finish();
    }
}

criterion_group!(benches, bench_treemap_layout);
criterion_main!(benches);

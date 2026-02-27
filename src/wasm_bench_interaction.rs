// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! WASM-compatible interaction benchmarks.
//!
//! This module provides benchmark functions that exercise the interaction system
//! (point queries, region queries, batch queries) using the same dataset
//! patterns as the native criterion benchmarks in `benches/interaction_benchmarks.rs`.
//!
//! On `wasm32` targets, `run_wasm_benchmarks` is exported via `wasm_bindgen`
//! and can be called from JavaScript to produce a JSON report.

use std::sync::Arc;

use crate::interaction::{GpuInteractionQuery, InteractionSystem, Rect, Renderable, Vec2};
use crate::selection::Selection;
use crate::wasm_bench::{BenchConfig, BenchResult, BenchSuite, Timer, from_timings};
use crate::{Circle, InteractionData, RenderContext};

// ---------------------------------------------------------------------------
// Data generators (mirrors benches/interaction_benchmarks.rs)
// ---------------------------------------------------------------------------

/// Test data element with position.
#[derive(Debug, Clone)]
struct BenchData {
    x: f32,
    y: f32,
}

impl BenchData {
    fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

impl InteractionData for BenchData {
    fn position(&self) -> [f32; 2] {
        [self.x, self.y]
    }
}

/// Generate a grid of data points spread evenly in a 1000x1000 space.
fn generate_grid_data(count: usize) -> Vec<BenchData> {
    let side = (count as f32).sqrt().ceil() as usize;
    let spacing = 1000.0 / side as f32;
    (0..count)
        .map(|i| {
            let col = i % side;
            let row = i / side;
            BenchData::new(col as f32 * spacing, row as f32 * spacing)
        })
        .collect()
}

/// Generate clustered data points (multiple dense groups).
fn generate_clustered_data(count: usize) -> Vec<BenchData> {
    let clusters = 10;
    let per_cluster = count / clusters;
    let mut data = Vec::with_capacity(count);
    for c in 0..clusters {
        let cx = (c % 4) as f32 * 250.0 + 125.0;
        let cy = (c / 4) as f32 * 333.0 + 166.0;
        for i in 0..per_cluster {
            let angle = i as f32 * std::f32::consts::TAU / per_cluster as f32;
            let r = (i as f32 * 0.5) % 50.0;
            data.push(BenchData::new(cx + r * angle.cos(), cy + r * angle.sin()));
        }
    }
    while data.len() < count {
        data.push(BenchData::new(500.0, 500.0));
    }
    data
}

// ---------------------------------------------------------------------------
// Point query benchmarks
// ---------------------------------------------------------------------------

/// Run point query benchmarks across different dataset sizes and patterns.
pub async fn bench_point_queries(
    context: &Arc<RenderContext>,
    config: &BenchConfig,
) -> Vec<BenchResult> {
    let timer = Timer::new();
    let mut results = Vec::new();

    for &size in &[1_000usize, 10_000] {
        let data = generate_grid_data(size);
        let selection = Selection::<BenchData, Circle>::new(data, Arc::clone(context))
            .expect("Failed to create selection");
        let mut system = InteractionSystem::new(context.as_ref())
            .await
            .expect("Failed to create system");
        let sels: Vec<&dyn Renderable> = vec![&selection];

        // Warmup
        for _ in 0..config.warmup_iterations {
            let hits = system
                .query_point(Vec2::new(500.0, 500.0), &sels)
                .await
                .unwrap();
            std::hint::black_box(hits);
        }
        // Measure
        let mut timings = Vec::with_capacity(config.measured_iterations as usize);
        for _ in 0..config.measured_iterations {
            let start = timer.now_ms();
            let hits = system
                .query_point(Vec2::new(500.0, 500.0), &sels)
                .await
                .unwrap();
            std::hint::black_box(hits);
            timings.push(timer.now_ms() - start);
        }
        results.push(from_timings(
            &format!("point_queries/grid/{size}"),
            &timings,
        ));
    }

    // Clustered data pattern
    for &size in &[1_000usize, 10_000] {
        let data = generate_clustered_data(size);
        let selection = Selection::<BenchData, Circle>::new(data, Arc::clone(context))
            .expect("Failed to create selection");
        let mut system = InteractionSystem::new(context.as_ref())
            .await
            .expect("Failed to create system");
        let sels: Vec<&dyn Renderable> = vec![&selection];

        for _ in 0..config.warmup_iterations {
            let hits = system
                .query_point(Vec2::new(125.0, 166.0), &sels)
                .await
                .unwrap();
            std::hint::black_box(hits);
        }
        let mut timings = Vec::with_capacity(config.measured_iterations as usize);
        for _ in 0..config.measured_iterations {
            let start = timer.now_ms();
            let hits = system
                .query_point(Vec2::new(125.0, 166.0), &sels)
                .await
                .unwrap();
            std::hint::black_box(hits);
            timings.push(timer.now_ms() - start);
        }
        results.push(from_timings(
            &format!("point_queries/clustered/{size}"),
            &timings,
        ));
    }

    results
}

// ---------------------------------------------------------------------------
// Region query benchmarks
// ---------------------------------------------------------------------------

/// Run region query benchmarks with varying coverage areas.
pub async fn bench_region_queries(
    context: &Arc<RenderContext>,
    config: &BenchConfig,
) -> Vec<BenchResult> {
    let timer = Timer::new();
    let mut results = Vec::new();
    let size = 10_000usize;
    let data = generate_grid_data(size);
    let selection = Selection::<BenchData, Circle>::new(data, Arc::clone(context))
        .expect("Failed to create selection");

    // Small region (~1% coverage)
    {
        let mut system = InteractionSystem::new(context.as_ref())
            .await
            .expect("Failed to create system");
        let sels: Vec<&dyn Renderable> = vec![&selection];
        let region = Rect::new(Vec2::new(450.0, 450.0), Vec2::new(550.0, 550.0));

        for _ in 0..config.warmup_iterations {
            let hits = system.query_region(region, &sels).await.unwrap();
            std::hint::black_box(hits);
        }
        let mut timings = Vec::with_capacity(config.measured_iterations as usize);
        for _ in 0..config.measured_iterations {
            let start = timer.now_ms();
            let hits = system.query_region(region, &sels).await.unwrap();
            std::hint::black_box(hits);
            timings.push(timer.now_ms() - start);
        }
        results.push(from_timings("region_queries/small_region_10k", &timings));
    }

    // Medium region (~10% coverage)
    {
        let mut system = InteractionSystem::new(context.as_ref())
            .await
            .expect("Failed to create system");
        let sels: Vec<&dyn Renderable> = vec![&selection];
        let region = Rect::new(Vec2::new(350.0, 350.0), Vec2::new(650.0, 650.0));

        for _ in 0..config.warmup_iterations {
            let hits = system.query_region(region, &sels).await.unwrap();
            std::hint::black_box(hits);
        }
        let mut timings = Vec::with_capacity(config.measured_iterations as usize);
        for _ in 0..config.measured_iterations {
            let start = timer.now_ms();
            let hits = system.query_region(region, &sels).await.unwrap();
            std::hint::black_box(hits);
            timings.push(timer.now_ms() - start);
        }
        results.push(from_timings("region_queries/medium_region_10k", &timings));
    }

    // Large region (~50% coverage)
    {
        let mut system = InteractionSystem::new(context.as_ref())
            .await
            .expect("Failed to create system");
        let sels: Vec<&dyn Renderable> = vec![&selection];
        let region = Rect::new(Vec2::new(150.0, 150.0), Vec2::new(850.0, 850.0));

        for _ in 0..config.warmup_iterations {
            let hits = system.query_region(region, &sels).await.unwrap();
            std::hint::black_box(hits);
        }
        let mut timings = Vec::with_capacity(config.measured_iterations as usize);
        for _ in 0..config.measured_iterations {
            let start = timer.now_ms();
            let hits = system.query_region(region, &sels).await.unwrap();
            std::hint::black_box(hits);
            timings.push(timer.now_ms() - start);
        }
        results.push(from_timings("region_queries/large_region_10k", &timings));
    }

    results
}

// ---------------------------------------------------------------------------
// Batch query benchmarks
// ---------------------------------------------------------------------------

/// Run batch query benchmarks comparing individual vs batched queries.
pub async fn bench_batch_queries(
    context: &Arc<RenderContext>,
    config: &BenchConfig,
) -> Vec<BenchResult> {
    let timer = Timer::new();
    let mut results = Vec::new();
    let size = 10_000usize;
    let data = generate_grid_data(size);
    let selection = Selection::<BenchData, Circle>::new(data, Arc::clone(context))
        .expect("Failed to create selection");

    // Single query baseline
    {
        let mut system = InteractionSystem::new(context.as_ref())
            .await
            .expect("Failed to create system");
        let sels: Vec<&dyn Renderable> = vec![&selection];

        for _ in 0..config.warmup_iterations {
            let hits = system
                .query_point(Vec2::new(500.0, 500.0), &sels)
                .await
                .unwrap();
            std::hint::black_box(hits);
        }
        let mut timings = Vec::with_capacity(config.measured_iterations as usize);
        for _ in 0..config.measured_iterations {
            let start = timer.now_ms();
            let hits = system
                .query_point(Vec2::new(500.0, 500.0), &sels)
                .await
                .unwrap();
            std::hint::black_box(hits);
            timings.push(timer.now_ms() - start);
        }
        results.push(from_timings("batch_queries/single_query_10k", &timings));
    }

    // Batch of 5 queries
    {
        let mut system = InteractionSystem::new(context.as_ref())
            .await
            .expect("Failed to create system");
        let sels: Vec<&dyn Renderable> = vec![&selection];
        let queries: Vec<GpuInteractionQuery> = (0..5)
            .map(|i| GpuInteractionQuery::point(Vec2::new(i as f32 * 200.0 + 100.0, 500.0), 1000))
            .collect();

        for _ in 0..config.warmup_iterations {
            let hits = system.query_batch(&queries, &sels).await.unwrap();
            std::hint::black_box(hits);
        }
        let mut timings = Vec::with_capacity(config.measured_iterations as usize);
        for _ in 0..config.measured_iterations {
            let start = timer.now_ms();
            let hits = system.query_batch(&queries, &sels).await.unwrap();
            std::hint::black_box(hits);
            timings.push(timer.now_ms() - start);
        }
        results.push(from_timings("batch_queries/batch_5_queries_10k", &timings));
    }

    // Batch of 10 queries
    {
        let mut system = InteractionSystem::new(context.as_ref())
            .await
            .expect("Failed to create system");
        let sels: Vec<&dyn Renderable> = vec![&selection];
        let queries: Vec<GpuInteractionQuery> = (0..10)
            .map(|i| GpuInteractionQuery::point(Vec2::new(i as f32 * 100.0 + 50.0, 500.0), 1000))
            .collect();

        for _ in 0..config.warmup_iterations {
            let hits = system.query_batch(&queries, &sels).await.unwrap();
            std::hint::black_box(hits);
        }
        let mut timings = Vec::with_capacity(config.measured_iterations as usize);
        for _ in 0..config.measured_iterations {
            let start = timer.now_ms();
            let hits = system.query_batch(&queries, &sels).await.unwrap();
            std::hint::black_box(hits);
            timings.push(timer.now_ms() - start);
        }
        results.push(from_timings("batch_queries/batch_10_queries_10k", &timings));
    }

    results
}

// ---------------------------------------------------------------------------
// Full suite runner
// ---------------------------------------------------------------------------

/// Run all interaction benchmarks and return a complete suite of results.
///
/// This is the main entry point for both native and WASM benchmarks. It
/// initialises a [`RenderContext`], runs point/region/batch benchmarks, and
/// packages the results into a [`BenchSuite`].
pub async fn run_interaction_benchmarks(config: &BenchConfig) -> BenchSuite {
    let context = Arc::new(
        RenderContext::new()
            .await
            .expect("Failed to create context"),
    );

    let mut results = Vec::new();
    results.extend(bench_point_queries(&context, config).await);
    results.extend(bench_region_queries(&context, config).await);
    results.extend(bench_batch_queries(&context, config).await);

    let platform = if cfg!(target_arch = "wasm32") {
        "wasm".to_string()
    } else {
        "native".to_string()
    };

    let timestamp = chrono::Utc::now().to_rfc3339();

    BenchSuite {
        platform,
        timestamp,
        results,
        user_agent: None,
    }
}

/// Run all benchmarks on native target and return JSON.
///
/// This is used by the comparison script to generate a native baseline
/// in the same format as the WASM benchmark output.
#[cfg(not(target_arch = "wasm32"))]
pub fn run_native_benchmarks() -> String {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let config = BenchConfig::default();
    let suite = rt.block_on(run_interaction_benchmarks(&config));
    serde_json::to_string_pretty(&suite).expect("Failed to serialize results")
}

// ---------------------------------------------------------------------------
// WASM entry point
// ---------------------------------------------------------------------------

/// Run all benchmarks from JavaScript and return JSON results.
///
/// Call from JS: `const json = await gup.run_wasm_benchmarks();`
#[cfg(all(target_arch = "wasm32", not(test)))]
#[wasm_bindgen::prelude::wasm_bindgen]
pub async fn run_wasm_benchmarks() -> String {
    let config = BenchConfig::default();
    let mut suite = run_interaction_benchmarks(&config).await;

    // Capture browser user agent
    suite.user_agent = web_sys::window().and_then(|w| w.navigator().user_agent().ok());
    suite.platform = "wasm".to_string();

    serde_json::to_string_pretty(&suite).expect("Failed to serialize results")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_grid_data() {
        let data = generate_grid_data(100);
        assert_eq!(data.len(), 100);

        // All points should be in [0, 1000] range
        for d in &data {
            assert!(d.x >= 0.0 && d.x <= 1000.0);
            assert!(d.y >= 0.0 && d.y <= 1000.0);
        }
    }

    #[test]
    fn test_generate_clustered_data() {
        let data = generate_clustered_data(1000);
        assert_eq!(data.len(), 1000);
    }

    #[test]
    fn test_bench_data_interaction() {
        let d = BenchData::new(42.0, 99.0);
        assert_eq!(d.position(), [42.0, 99.0]);
    }
}

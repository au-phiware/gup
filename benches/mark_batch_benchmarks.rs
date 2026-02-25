// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Performance benchmarks for the instanced batch rendering system.
//!
//! Measures CPU-side overhead for batch preparation, culling, and LOD
//! classification at various data scales (1K, 10K, 100K, 1M instances).
//!
//! GPU upload benchmarks are excluded to avoid driver race conditions
//! in tight criterion loops (same issue as buffer_benchmarks.rs).

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use gup::mark::batch_renderer::{BatchRendererConfig, CullingManager, LodLevel, Viewport2D};
use std::hint::black_box;

// ---------------------------------------------------------------------------
// Helper: generate random-ish circle data
// ---------------------------------------------------------------------------

fn generate_circle_data(n: usize) -> (Vec<[f32; 2]>, Vec<f32>) {
    let mut centers = Vec::with_capacity(n);
    let mut radii = Vec::with_capacity(n);
    for i in 0..n {
        // Distribute points across clip space [-1, 1] using a simple
        // deterministic pattern that ensures some are off-screen.
        let t = i as f32 / n as f32;
        let x = (t * 37.0).sin() * 1.5; // some exceed [-1,1]
        let y = (t * 53.0).cos() * 1.5;
        let r = 0.001 + (t * 19.0).sin().abs() * 0.05;
        centers.push([x, y]);
        radii.push(r);
    }
    (centers, radii)
}

// ---------------------------------------------------------------------------
// Culling benchmarks
// ---------------------------------------------------------------------------

fn bench_culling(c: &mut Criterion) {
    let mut group = c.benchmark_group("mark_culling");

    for &size in &[1_000usize, 10_000, 100_000, 1_000_000] {
        let (centers, radii) = generate_circle_data(size);

        group.bench_with_input(BenchmarkId::new("is_visible", size), &size, |b, _| {
            let cfg = BatchRendererConfig::default();
            let cm = CullingManager::new(&cfg);
            b.iter(|| {
                let mut visible = 0u32;
                for (center, &radius) in centers.iter().zip(radii.iter()) {
                    if cm.is_visible(center[0], center[1], radius) {
                        visible += 1;
                    }
                }
                black_box(visible)
            });
        });

        group.bench_with_input(BenchmarkId::new("classify_circles", size), &size, |b, _| {
            let cfg = BatchRendererConfig::default();
            let cm = CullingManager::new(&cfg);
            b.iter(|| black_box(cm.classify_circles(&centers, &radii)));
        });
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// LOD computation benchmarks
// ---------------------------------------------------------------------------

fn bench_lod(c: &mut Criterion) {
    let mut group = c.benchmark_group("mark_lod");

    for &size in &[1_000usize, 10_000, 100_000, 1_000_000] {
        let (_, radii) = generate_circle_data(size);

        group.bench_with_input(BenchmarkId::new("compute_lod", size), &size, |b, _| {
            let cfg = BatchRendererConfig::default();
            let cm = CullingManager::new(&cfg);
            b.iter(|| {
                let mut full = 0u32;
                let mut simplified = 0u32;
                let mut point = 0u32;
                let mut culled = 0u32;
                for &r in &radii {
                    match cm.compute_lod(r) {
                        LodLevel::Full => full += 1,
                        LodLevel::Simplified => simplified += 1,
                        LodLevel::Point => point += 1,
                        LodLevel::Culled => culled += 1,
                    }
                }
                black_box((full, simplified, point, culled))
            });
        });
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Viewport configuration benchmarks
// ---------------------------------------------------------------------------

fn bench_viewport_configs(c: &mut Criterion) {
    let mut group = c.benchmark_group("viewport_configs");

    let (centers, radii) = generate_circle_data(100_000);

    // Full viewport: all clip space visible
    group.bench_function("full_viewport", |b| {
        let cfg = BatchRendererConfig::default();
        let cm = CullingManager::new(&cfg);
        b.iter(|| black_box(cm.classify_circles(&centers, &radii)));
    });

    // Small viewport: only center 25% visible
    group.bench_function("quarter_viewport", |b| {
        let cfg = BatchRendererConfig::default();
        let mut cm = CullingManager::new(&cfg);
        cm.set_viewport(Viewport2D {
            min_x: -0.5,
            max_x: 0.5,
            min_y: -0.5,
            max_y: 0.5,
            pixel_width: 400.0,
            pixel_height: 300.0,
        });
        b.iter(|| black_box(cm.classify_circles(&centers, &radii)));
    });

    // High-res viewport: more instances at Full LOD
    group.bench_function("hires_viewport", |b| {
        let cfg = BatchRendererConfig::default();
        let mut cm = CullingManager::new(&cfg);
        cm.set_viewport(Viewport2D {
            min_x: -1.0,
            max_x: 1.0,
            min_y: -1.0,
            max_y: 1.0,
            pixel_width: 3840.0,
            pixel_height: 2160.0,
        });
        b.iter(|| black_box(cm.classify_circles(&centers, &radii)));
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Instance data preparation benchmarks (CPU only)
// ---------------------------------------------------------------------------

fn bench_instance_preparation(c: &mut Criterion) {
    let mut group = c.benchmark_group("instance_preparation");

    for &size in &[1_000usize, 10_000, 100_000] {
        let (centers, radii) = generate_circle_data(size);

        // Benchmark: filtering instances using culling results
        group.bench_with_input(
            BenchmarkId::new("filter_by_culling", size),
            &size,
            |b, _| {
                let cfg = BatchRendererConfig::default();
                let cm = CullingManager::new(&cfg);
                let classified = cm.classify_circles(&centers, &radii);
                b.iter(|| {
                    let mut visible_count = 0u32;
                    for indices in classified.values() {
                        visible_count += indices.len() as u32;
                    }
                    black_box(visible_count)
                });
            },
        );

        // Benchmark: creating bytemuck-compatible instance data
        group.bench_with_input(
            BenchmarkId::new("create_circle_instances", size),
            &size,
            |b, _| {
                b.iter(|| {
                    let instances: Vec<[f32; 16]> = (0..size)
                        .map(|i| {
                            let mut data = [0.0f32; 16];
                            data[0] = centers[i][0];
                            data[1] = centers[i][1];
                            data[2] = radii[i];
                            data[4] = 1.0; // fill_color.r
                            data[5] = 0.0;
                            data[6] = 0.0;
                            data[7] = 1.0; // fill_color.a
                            data
                        })
                        .collect();
                    black_box(instances)
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_culling,
    bench_lod,
    bench_viewport_configs,
    bench_instance_preparation,
);
criterion_main!(benches);

// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Performance benchmarks for GPU occlusion culling.
//!
//! Measures occlusion culling dispatch time for dense datasets at various
//! scales and compares with/without occlusion culling to quantify the
//! reduction in visible instances.
//!
//! GPU benchmarks require a functional GPU. If no GPU is available the
//! benchmarks are skipped silently.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use gup::context::GupContext;
use gup::mark::batch_renderer::{InstanceAttributes, Viewport2D};
use gup::mark::occlusion_culler::{OcclusionCuller, OcclusionParams, PooledOcclusionCuller};
use wgpu::{BufferDescriptor, BufferUsages};

// ---------------------------------------------------------------------------
// Helper: generate dense clustered instances (heavily overlapping)
// ---------------------------------------------------------------------------

fn generate_dense_cluster(n: usize) -> Vec<InstanceAttributes> {
    (0..n)
        .map(|i| {
            let t = i as f32 / n as f32;
            // Tightly clustered near the center with small radii.
            let x = (t * 37.0).sin() * 0.3;
            let y = (t * 53.0).cos() * 0.3;
            InstanceAttributes::from_circle([x, y], 0.05, [1.0, 0.0, 0.0, 1.0])
        })
        .collect()
}

fn generate_stacked_instances(n: usize) -> Vec<InstanceAttributes> {
    (0..n)
        .map(|_| InstanceAttributes::from_circle([0.0, 0.0], 0.1, [1.0, 0.0, 0.0, 1.0]))
        .collect()
}

/// Mixed mark sizes: small scatter points + large background rectangles.
/// This exercises the coarse Hi-Z early-reject path (GUP-223).
fn generate_mixed_size_dataset(n_small: usize, n_large: usize) -> Vec<InstanceAttributes> {
    let mut instances = Vec::with_capacity(n_small + n_large);

    // Large background marks drawn first (lower z / earlier draw order).
    for i in 0..n_large {
        let t = i as f32 / n_large as f32;
        let x = (t * 7.0).sin() * 0.4;
        let y = (t * 11.0).cos() * 0.4;
        instances.push(InstanceAttributes::from_circle(
            [x, y],
            0.3,
            [0.2, 0.2, 0.8, 1.0],
        ));
    }

    // Small scatter points drawn on top (higher z).
    for i in 0..n_small {
        let t = i as f32 / n_small as f32;
        let x = (t * 37.0).sin() * 0.5;
        let y = (t * 53.0).cos() * 0.5;
        instances.push(InstanceAttributes::from_circle(
            [x, y],
            0.02,
            [1.0, 0.0, 0.0, 1.0],
        ));
    }

    instances
}

// ---------------------------------------------------------------------------
// Occlusion culling dispatch benchmark
// ---------------------------------------------------------------------------

fn bench_occlusion_dispatch(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let ctx = match rt.block_on(GupContext::headless()) {
        Ok(c) => c,
        Err(_) => {
            eprintln!("No GPU available — skipping occlusion benchmarks");
            return;
        }
    };

    let mut group = c.benchmark_group("occlusion_culling_dispatch");

    for &size in &[1_000usize, 10_000, 100_000] {
        let instances = generate_dense_cluster(size);
        let data: &[u8] = bytemuck::cast_slice(&instances);

        let input_buffer = ctx.device.create_buffer(&BufferDescriptor {
            label: Some("bench_occlusion_input"),
            size: data.len() as u64,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        ctx.queue.write_buffer(&input_buffer, 0, data);

        let culler = OcclusionCuller::new(&ctx.device).unwrap();
        let viewport = Viewport2D::default();
        let params = OcclusionParams::default();

        group.bench_with_input(BenchmarkId::new("fresh_buffers", size), &size, |b, _| {
            b.iter(|| {
                let result = rt.block_on(culler.dispatch(
                    &ctx.device,
                    &ctx.queue,
                    &input_buffer,
                    size as u32,
                    &viewport,
                    &params,
                ));
                criterion::black_box(result.unwrap());
            });
        });
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Pooled occlusion culling dispatch benchmark
// ---------------------------------------------------------------------------

fn bench_pooled_occlusion_dispatch(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let ctx = match rt.block_on(GupContext::headless()) {
        Ok(c) => c,
        Err(_) => {
            eprintln!("No GPU available — skipping pooled occlusion benchmarks");
            return;
        }
    };

    let mut group = c.benchmark_group("occlusion_culling_pooled");

    for &size in &[1_000usize, 10_000, 100_000] {
        let instances = generate_dense_cluster(size);
        let data: &[u8] = bytemuck::cast_slice(&instances);

        let input_buffer = ctx.device.create_buffer(&BufferDescriptor {
            label: Some("bench_occlusion_input"),
            size: data.len() as u64,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        ctx.queue.write_buffer(&input_buffer, 0, data);

        let inner = OcclusionCuller::new(&ctx.device).unwrap();
        let viewport = Viewport2D::default();
        let params = OcclusionParams::default();
        let mut pooled =
            PooledOcclusionCuller::new(&ctx.device, inner, size as u32, &viewport, &params);

        group.bench_with_input(BenchmarkId::new("pooled_buffers", size), &size, |b, _| {
            b.iter(|| {
                let result = rt.block_on(pooled.dispatch(
                    &ctx.device,
                    &ctx.queue,
                    &input_buffer,
                    size as u32,
                    &viewport,
                    &params,
                ));
                criterion::black_box(result.unwrap());
            });
        });
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Culling effectiveness benchmark (measures cull rate)
// ---------------------------------------------------------------------------

fn bench_culling_effectiveness(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let ctx = match rt.block_on(GupContext::headless()) {
        Ok(c) => c,
        Err(_) => {
            eprintln!("No GPU available — skipping effectiveness benchmarks");
            return;
        }
    };

    let mut group = c.benchmark_group("occlusion_culling_effectiveness");

    // Stacked instances: maximum overlap → maximum culling.
    for &size in &[100usize, 1_000, 10_000] {
        let instances = generate_stacked_instances(size);
        let data: &[u8] = bytemuck::cast_slice(&instances);

        let input_buffer = ctx.device.create_buffer(&BufferDescriptor {
            label: Some("bench_stacked_input"),
            size: data.len() as u64,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        ctx.queue.write_buffer(&input_buffer, 0, data);

        let culler = OcclusionCuller::new(&ctx.device).unwrap();
        let viewport = Viewport2D::default();
        let params = OcclusionParams {
            tile_size: 4,
            conservative_margin: 0.0,
        };

        group.bench_with_input(
            BenchmarkId::new("stacked_instances", size),
            &size,
            |b, _| {
                b.iter(|| {
                    let result = rt.block_on(culler.dispatch(
                        &ctx.device,
                        &ctx.queue,
                        &input_buffer,
                        size as u32,
                        &viewport,
                        &params,
                    ));
                    criterion::black_box(result.unwrap());
                });
            },
        );
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Mixed-size dataset benchmark (GUP-223: coarse Hi-Z early reject)
// ---------------------------------------------------------------------------

fn bench_mixed_size_dispatch(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let ctx = match rt.block_on(GupContext::headless()) {
        Ok(c) => c,
        Err(_) => {
            eprintln!("No GPU available — skipping mixed-size benchmarks");
            return;
        }
    };

    let mut group = c.benchmark_group("occlusion_culling_mixed_size");

    // With adaptive build_coverage (GUP-234), tile_size=4 works correctly
    // for large marks — no need for the tile_size=16 workaround.
    let viewport = Viewport2D::default();
    let params = OcclusionParams {
        tile_size: 4,
        conservative_margin: 0.01,
    };

    for &(n_small, n_large) in &[(1_000usize, 50usize), (5_000, 100), (10_000, 200)] {
        let instances = generate_mixed_size_dataset(n_small, n_large);
        let total = instances.len();
        let data: &[u8] = bytemuck::cast_slice(&instances);

        let input_buffer = ctx.device.create_buffer(&BufferDescriptor {
            label: Some("bench_mixed_input"),
            size: data.len() as u64,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        ctx.queue.write_buffer(&input_buffer, 0, data);

        let inner = OcclusionCuller::new(&ctx.device).unwrap();
        let mut pooled =
            PooledOcclusionCuller::new(&ctx.device, inner, total as u32, &viewport, &params);

        group.bench_with_input(
            BenchmarkId::new("mixed_small_large", format!("{n_small}s_{n_large}l")),
            &total,
            |b, _| {
                b.iter(|| {
                    let result = rt.block_on(pooled.dispatch(
                        &ctx.device,
                        &ctx.queue,
                        &input_buffer,
                        total as u32,
                        &viewport,
                        &params,
                    ));
                    criterion::black_box(result.unwrap());
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_occlusion_dispatch,
    bench_pooled_occlusion_dispatch,
    bench_culling_effectiveness,
    bench_mixed_size_dispatch,
);
criterion_main!(benches);

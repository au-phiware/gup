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

criterion_group!(
    benches,
    bench_occlusion_dispatch,
    bench_pooled_occlusion_dispatch,
    bench_culling_effectiveness,
);
criterion_main!(benches);

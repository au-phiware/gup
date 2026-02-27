// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Performance benchmarks comparing unified vs separate frustum + occlusion pipelines.
//!
//! Measures dispatch latency at 1K and 10K scales to verify that the unified
//! pipeline is equal to or better than running both pipelines separately.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use gup::context::GupContext;
use gup::mark::batch_renderer::{InstanceAttributes, Viewport2D};
use gup::mark::compute_instance_filter::{ComputeInstanceFilter, PooledComputeInstanceFilter};
use gup::mark::occlusion_culler::{OcclusionCuller, OcclusionParams, PooledOcclusionCuller};
use gup::mark::unified_culling_pipeline::UnifiedCullingPipeline;
use wgpu::{BufferDescriptor, BufferUsages};

// ---------------------------------------------------------------------------
// Helper: generate dense overlapping instances (many stacked at center)
// ---------------------------------------------------------------------------

fn generate_dense_instances(n: usize) -> Vec<InstanceAttributes> {
    (0..n)
        .map(|i| {
            let t = i as f32 / n as f32;
            // Dense cluster at center with slight offsets.
            let x = (t * 37.0).sin() * 0.3;
            let y = (t * 53.0).cos() * 0.3;
            InstanceAttributes::from_circle([x, y], 0.15, [1.0, 0.0, 0.0, 1.0])
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Benchmarks
// ---------------------------------------------------------------------------

fn bench_separate_pipelines(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let ctx = match rt.block_on(GupContext::headless()) {
        Ok(c) => c,
        Err(_) => {
            eprintln!("No GPU available — skipping separate pipeline benchmarks");
            return;
        }
    };

    let mut group = c.benchmark_group("separate_frustum_then_occlusion");

    for &n in &[1_000u32, 10_000] {
        let instances = generate_dense_instances(n as usize);
        let data: &[u8] = bytemuck::cast_slice(&instances);

        let input_buffer = ctx.device.create_buffer(&BufferDescriptor {
            label: Some("bench_input"),
            size: data.len() as u64,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        ctx.queue.write_buffer(&input_buffer, 0, data);

        let filter = ComputeInstanceFilter::new(&ctx.device).unwrap();
        let mut pooled_filter = PooledComputeInstanceFilter::new(&ctx.device, filter, n);

        let occlusion = OcclusionCuller::new(&ctx.device).unwrap();
        let viewport = Viewport2D::default();
        let params = OcclusionParams {
            tile_size: 4,
            conservative_margin: 0.0,
        };
        let mut pooled_occlusion =
            PooledOcclusionCuller::new(&ctx.device, occlusion, n, &viewport, &params);

        let thresholds = [4.0, 1.0, 0.25];

        group.bench_with_input(BenchmarkId::new("dispatch", n), &n, |b, &n| {
            b.iter(|| {
                rt.block_on(async {
                    // Two separate dispatches.
                    let _filter_result = pooled_filter
                        .dispatch(
                            &ctx.device,
                            &ctx.queue,
                            &input_buffer,
                            n,
                            6,
                            &viewport,
                            &thresholds,
                        )
                        .await
                        .unwrap();

                    let _occlusion_result = pooled_occlusion
                        .dispatch(
                            &ctx.device,
                            &ctx.queue,
                            &input_buffer,
                            n,
                            &viewport,
                            &params,
                        )
                        .await
                        .unwrap();
                });
            });
        });
    }

    group.finish();
}

fn bench_unified_pipeline(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let ctx = match rt.block_on(GupContext::headless()) {
        Ok(c) => c,
        Err(_) => {
            eprintln!("No GPU available — skipping unified pipeline benchmarks");
            return;
        }
    };

    let mut group = c.benchmark_group("unified_frustum_occlusion");

    for &n in &[1_000u32, 10_000] {
        let instances = generate_dense_instances(n as usize);
        let data: &[u8] = bytemuck::cast_slice(&instances);

        let input_buffer = ctx.device.create_buffer(&BufferDescriptor {
            label: Some("bench_input"),
            size: data.len() as u64,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        ctx.queue.write_buffer(&input_buffer, 0, data);

        let viewport = Viewport2D::default();
        let params = OcclusionParams {
            tile_size: 4,
            conservative_margin: 0.0,
        };
        let mut pipeline = UnifiedCullingPipeline::new(&ctx.device, n, &viewport, &params).unwrap();

        let thresholds = [4.0, 1.0, 0.25];

        group.bench_with_input(BenchmarkId::new("dispatch", n), &n, |b, &n| {
            b.iter(|| {
                rt.block_on(async {
                    let _result = pipeline
                        .dispatch(
                            &ctx.device,
                            &ctx.queue,
                            &input_buffer,
                            n,
                            6,
                            &viewport,
                            &thresholds,
                            Some(&params),
                        )
                        .await
                        .unwrap();
                });
            });
        });
    }

    group.finish();
}

criterion_group!(benches, bench_separate_pipelines, bench_unified_pipeline,);
criterion_main!(benches);

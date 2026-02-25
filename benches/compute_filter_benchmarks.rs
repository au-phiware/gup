// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Performance benchmarks comparing CPU vs GPU instance filtering.
//!
//! Measures end-to-end time for frustum culling + LOD classification
//! + stream compaction at 100K, 1M, and 10M instance scales using both
//! the CPU [`CullingManager`] and the GPU [`ComputeInstanceFilter`].
//!
//! GPU benchmarks include dispatch + readback so that the numbers are
//! directly comparable to the CPU path.
//!
//! Note: GPU benchmarks require a functional GPU. If no GPU is available
//! the GPU group is skipped silently.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use gup::context::GupContext;
use gup::mark::batch_renderer::{
    BatchRendererConfig, CullingManager, InstanceAttributes, Viewport2D,
};
use gup::mark::compute_instance_filter::ComputeInstanceFilter;
use std::hint::black_box;
use wgpu::{BufferDescriptor, BufferUsages};

// ---------------------------------------------------------------------------
// Helper: generate deterministic instance data
// ---------------------------------------------------------------------------

fn generate_instances(n: usize) -> Vec<InstanceAttributes> {
    (0..n)
        .map(|i| {
            let t = i as f32 / n as f32;
            // Distribute across [-1.5, 1.5] so some are off-screen.
            let x = (t * 37.0).sin() * 1.5;
            let y = (t * 53.0).cos() * 1.5;
            let r = 0.001 + (t * 19.0).sin().abs() * 0.05;
            InstanceAttributes::from_circle([x, y], r, [1.0, 0.0, 0.0, 1.0])
        })
        .collect()
}

// ---------------------------------------------------------------------------
// CPU culling benchmark
// ---------------------------------------------------------------------------

fn bench_cpu_culling(c: &mut Criterion) {
    let mut group = c.benchmark_group("instance_filter_cpu");

    for &size in &[100_000usize, 1_000_000, 10_000_000] {
        let instances = generate_instances(size);
        let centers: Vec<[f32; 2]> = instances.iter().map(|i| i.position()).collect();
        let radii: Vec<f32> = instances.iter().map(|i| i.scale()[0]).collect();

        group.bench_with_input(BenchmarkId::new("classify_circles", size), &size, |b, _| {
            let cfg = BatchRendererConfig::default();
            let cm = CullingManager::new(&cfg);
            b.iter(|| {
                let classified = cm.classify_circles(black_box(&centers), black_box(&radii));
                black_box(classified);
            });
        });
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// GPU culling benchmark
// ---------------------------------------------------------------------------

fn bench_gpu_culling(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let ctx = match rt.block_on(GupContext::headless()) {
        Ok(c) => c,
        Err(_) => {
            eprintln!("No GPU available — skipping GPU benchmarks");
            return;
        }
    };
    let device = &ctx.device;
    let queue = &ctx.queue;

    let filter = match ComputeInstanceFilter::new(device) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Failed to create ComputeInstanceFilter: {e:?} — skipping GPU benchmarks");
            return;
        }
    };

    let viewport = Viewport2D::default();
    let thresholds = [4.0f32, 1.0, 0.25];

    let mut group = c.benchmark_group("instance_filter_gpu");

    // GPU benchmarks: capped at 1M because 10M × 96 bytes exceeds the
    // default wgpu max_buffer_size (256 MB). In practice, datasets
    // beyond 1M would use streaming / chunked processing.
    for &size in &[100_000usize, 1_000_000] {
        let instances = generate_instances(size);
        let attr_bytes: &[u8] = bytemuck::cast_slice(&instances);

        let input_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("bench_input"),
            size: attr_bytes.len() as u64,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&input_buffer, 0, attr_bytes);

        group.bench_with_input(BenchmarkId::new("dispatch_filter", size), &size, |b, _| {
            b.iter(|| {
                let result = rt.block_on(filter.dispatch(
                    device,
                    queue,
                    black_box(&input_buffer),
                    size as u32,
                    6, // vertex_count for circles
                    &viewport,
                    &thresholds,
                ));
                black_box(result.unwrap());
            });
        });
    }

    group.finish();
}

criterion_group!(benches, bench_cpu_culling, bench_gpu_culling);
criterion_main!(benches);

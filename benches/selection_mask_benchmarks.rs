// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Performance benchmark for the GPU selection mask buffer.
//!
//! Measures the time to apply a 10K-item selection to a 100K-point chart
//! using the GPU compute shader path. The acceptance target is < 2 ms.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use gup::context::GupContext;
use gup::linked_selection::SharedSelectionState;
use gup::mark::circle::CircleInstance;
use gup::selection_mask::{AlphaOffsets, SelectionMaskBuffer};
use wgpu::{BufferDescriptor, BufferUsages};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_circle_instances(count: usize) -> Vec<CircleInstance> {
    (0..count)
        .map(|i| {
            let v = i as f32 / count as f32;
            CircleInstance {
                center: [v * 2.0 - 1.0, (v * 7.0).sin()],
                radius: 0.005,
                _pad0: 0.0,
                fill_color: [1.0, v, 0.0, 1.0],
                stroke_width: 0.5,
                _pad1: [0.0; 3],
                stroke_color: [0.0, 0.0, 0.0, 0.8],
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Benchmark: mask update + compute dispatch
// ---------------------------------------------------------------------------

fn bench_selection_mask_dimming(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let ctx = match rt.block_on(GupContext::headless()) {
        Ok(c) => c,
        Err(_) => {
            eprintln!("No GPU available — skipping selection mask benchmarks");
            return;
        }
    };
    let device = &ctx.device;
    let queue = &ctx.queue;

    let mut group = c.benchmark_group("selection_mask_dimming");

    for &(instance_count, selection_count) in
        &[(100_000, 10_000), (100_000, 50_000), (10_000, 1_000)]
    {
        let instances = make_circle_instances(instance_count);
        let offsets = AlphaOffsets::for_circle();

        // Create source buffer.
        let source_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("bench_source"),
            size: (instance_count * std::mem::size_of::<CircleInstance>()) as u64,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&source_buffer, 0, bytemuck::cast_slice(&instances));

        let data: Vec<usize> = (0..instance_count).collect();
        let selected_keys: Vec<usize> = (0..selection_count).collect();

        group.bench_with_input(
            BenchmarkId::new(
                "update_and_dispatch",
                format!("{instance_count}pts_{selection_count}sel"),
            ),
            &(instance_count, selection_count),
            |b, _| {
                // Pre-create the pipeline (one-time cost).
                let mut mask_buf =
                    SelectionMaskBuffer::new(device, instance_count as u32, &offsets).unwrap();
                let state = SharedSelectionState::<usize>::new();
                // Two different selections to alternate between.
                let keys_a: Vec<usize> = (0..selection_count).collect();
                let keys_b: Vec<usize> = (1..selection_count + 1).collect();
                let mut use_a = true;

                b.iter(|| {
                    // Toggle selection to force a new generation each iteration.
                    if use_a {
                        state.set(keys_a.iter().copied());
                    } else {
                        state.set(keys_b.iter().copied());
                    }
                    use_a = !use_a;

                    mask_buf.update_and_dispatch(
                        device,
                        queue,
                        &data,
                        |_item, idx| idx,
                        &state,
                        &source_buffer,
                        instance_count as u32,
                        0.2,
                    );

                    // Ensure GPU work completes.
                    let _ = device.poll(wgpu::PollType::Wait);
                });
            },
        );

        // Also benchmark the pure GPU path (upload + dispatch only).
        group.bench_with_input(
            BenchmarkId::new(
                "gpu_only_dispatch",
                format!("{instance_count}pts_{selection_count}sel"),
            ),
            &(instance_count, selection_count),
            |b, _| {
                let mut mask_buf =
                    SelectionMaskBuffer::new(device, instance_count as u32, &offsets).unwrap();

                // Pre-build mask on CPU.
                let state = SharedSelectionState::<usize>::new();
                state.select(selected_keys.clone());
                mask_buf.update_mask(queue, &data, |_item, idx| idx, &state);

                b.iter(|| {
                    // Only measure the GPU dispatch + poll.
                    mask_buf.dispatch_dimming(
                        device,
                        queue,
                        &source_buffer,
                        instance_count as u32,
                        0.2,
                    );
                    let _ = device.poll(wgpu::PollType::Wait);
                });
            },
        );

        // Measure async submission (no poll) — the actual frame impact.
        group.bench_with_input(
            BenchmarkId::new(
                "encode_and_submit",
                format!("{instance_count}pts_{selection_count}sel"),
            ),
            &(instance_count, selection_count),
            |b, _| {
                let mut mask_buf =
                    SelectionMaskBuffer::new(device, instance_count as u32, &offsets).unwrap();
                let state = SharedSelectionState::<usize>::new();
                state.select(selected_keys.clone());
                mask_buf.update_mask(queue, &data, |_item, idx| idx, &state);

                b.iter(|| {
                    // Measure encode + submit only (no poll).
                    mask_buf.dispatch_dimming(
                        device,
                        queue,
                        &source_buffer,
                        instance_count as u32,
                        0.2,
                    );
                });
                // Flush after benchmark.
                let _ = device.poll(wgpu::PollType::Wait);
            },
        );

        // Also benchmark the warm path (incremental mask update only).
        group.bench_with_input(
            BenchmarkId::new(
                "incremental_update",
                format!("{instance_count}pts_{selection_count}sel"),
            ),
            &(instance_count, selection_count),
            |b, _| {
                let state = SharedSelectionState::<usize>::new();
                state.select(selected_keys.clone());

                let mut mask_buf =
                    SelectionMaskBuffer::new(device, instance_count as u32, &offsets).unwrap();

                // Warm up: do initial update.
                mask_buf.update_and_dispatch(
                    device,
                    queue,
                    &data,
                    |_item, idx| idx,
                    &state,
                    &source_buffer,
                    instance_count as u32,
                    0.2,
                );
                let _ = device.poll(wgpu::PollType::Wait);

                b.iter(|| {
                    // Modify selection slightly (add one, remove one).
                    state.set(selected_keys.iter().copied().skip(1).chain(std::iter::once(
                        instance_count - 1,
                    )));

                    mask_buf.update_and_dispatch(
                        device,
                        queue,
                        &data,
                        |_item, idx| idx,
                        &state,
                        &source_buffer,
                        instance_count as u32,
                        0.2,
                    );

                    let _ = device.poll(wgpu::PollType::Wait);
                });
            },
        );
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Compare GPU mask path vs CPU build_dimmed_instances
// ---------------------------------------------------------------------------

fn bench_cpu_vs_gpu_dimming(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let ctx = match rt.block_on(GupContext::headless()) {
        Ok(c) => c,
        Err(_) => {
            eprintln!("No GPU available — skipping CPU vs GPU dimming benchmarks");
            return;
        }
    };
    let device = &ctx.device;
    let queue = &ctx.queue;

    let mut group = c.benchmark_group("cpu_vs_gpu_dimming");
    let instance_count = 100_000;
    let selection_count = 10_000;

    let instances = make_circle_instances(instance_count);
    let data: Vec<usize> = (0..instance_count).collect();
    let selected_keys: Vec<usize> = (0..selection_count).collect();

    // CPU path: build_dimmed_instances
    group.bench_function(
        BenchmarkId::new("cpu_build_dimmed", format!("{instance_count}pts")),
        |b| {
            let state = SharedSelectionState::<usize>::new();
            state.select(selected_keys.clone());
            b.iter(|| {
                let _dimmed = gup::linked_selection::build_dimmed_instances(
                    &data,
                    |_item| instances[0], // use same instance for all
                    |_item, idx| idx,
                    &state,
                    0.2,
                );
            });
        },
    );

    // GPU path: mask update + dispatch
    group.bench_function(
        BenchmarkId::new("gpu_mask_dispatch", format!("{instance_count}pts")),
        |b| {
            let offsets = AlphaOffsets::for_circle();
            let source_buffer = device.create_buffer(&BufferDescriptor {
                label: Some("bench_source"),
                size: (instance_count * std::mem::size_of::<CircleInstance>()) as u64,
                usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            queue.write_buffer(&source_buffer, 0, bytemuck::cast_slice(&instances));

            let state = SharedSelectionState::<usize>::new();
            let mut mask_buf =
                SelectionMaskBuffer::new(device, instance_count as u32, &offsets).unwrap();
            let keys_a: Vec<usize> = (0..selection_count).collect();
            let keys_b: Vec<usize> = (1..selection_count + 1).collect();
            let mut use_a = true;

            b.iter(|| {
                if use_a {
                    state.set(keys_a.iter().copied());
                } else {
                    state.set(keys_b.iter().copied());
                }
                use_a = !use_a;

                mask_buf.update_and_dispatch(
                    device,
                    queue,
                    &data,
                    |_item, idx| idx,
                    &state,
                    &source_buffer,
                    instance_count as u32,
                    0.2,
                );
                let _ = device.poll(wgpu::PollType::Wait);
            });
        },
    );

    group.finish();
}

criterion_group!(
    benches,
    bench_selection_mask_dimming,
    bench_cpu_vs_gpu_dimming
);
criterion_main!(benches);

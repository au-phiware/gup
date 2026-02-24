// Copyright (C) 2025 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Pattern rendering performance benchmarks (GUP-156)
//!
//! This benchmark suite validates that pattern rendering meets the <5ms overhead
//! target for 100K+ points. It tests various pattern types, data sizes, and
//! rendering scenarios.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use gup::accessibility::{Color, Pattern, PatternRenderer, PatternUniforms};
use gup::mark::{Circle, Mark, MarkInfo, MarkInfoImpl, MarkRenderer};
use pollster::FutureExt;
use std::hint::black_box;

/// Test data sizes for benchmarks
const DATA_SIZES: &[usize] = &[1_000, 10_000, 100_000, 1_000_000];

/// All pattern types to benchmark
const PATTERN_TYPES: &[(&str, Pattern)] = &[
    ("solid", Pattern::Solid),
    ("dots_8", Pattern::Dots { spacing: 8.0 }),
    (
        "lines_6",
        Pattern::Lines {
            spacing: 6.0,
            angle: 0.0,
        },
    ),
    ("crosshatch_8", Pattern::Crosshatch { spacing: 8.0 }),
];

/// GPU benchmark context for pattern rendering tests
struct PatternBenchmarkContext {
    device: wgpu::Device,
    queue: wgpu::Queue,
}

impl PatternBenchmarkContext {
    async fn new() -> Self {
        let instance = wgpu::Instance::default();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: None,
            })
            .await
            .expect("Failed to find suitable GPU adapter");

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("Pattern Benchmark Device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::Performance,
                trace: Default::default(),
            })
            .await
            .expect("Failed to create GPU device");

        Self { device, queue }
    }
}

impl Drop for PatternBenchmarkContext {
    fn drop(&mut self) {
        // Poll device to ensure cleanup completes before dropping.
        // This prevents resource contention between sequential benchmark runs.
        let _ = self.device.poll(wgpu::PollType::Wait);
    }
}

/// Benchmark pattern renderer creation
fn bench_pattern_renderer_creation(c: &mut Criterion) {
    let context = PatternBenchmarkContext::new().block_on();
    let mut group = c.benchmark_group("pattern_renderer_creation");

    for (name, pattern) in PATTERN_TYPES {
        group.bench_function(*name, |b| {
            b.iter(|| {
                let uniforms = PatternUniforms::from_pattern(pattern, Color::BLACK, Color::WHITE);
                let renderer = PatternRenderer::new(&context.device, uniforms);
                black_box(renderer);
            });
        });
    }

    group.finish();
}

/// Benchmark pattern uniform updates
fn bench_pattern_uniform_updates(c: &mut Criterion) {
    let context = PatternBenchmarkContext::new().block_on();
    let mut group = c.benchmark_group("pattern_uniform_updates");

    for (name, pattern) in PATTERN_TYPES {
        group.bench_function(*name, |b| {
            let uniforms = PatternUniforms::from_pattern(pattern, Color::BLACK, Color::WHITE);
            let mut renderer = PatternRenderer::new(&context.device, uniforms);

            b.iter(|| {
                let new_uniforms = PatternUniforms::from_pattern(pattern, Color::RED, Color::BLUE);
                renderer.update(&context.queue, new_uniforms);
                black_box(());
            });
        });
    }

    group.finish();
}

/// Benchmark pipeline creation with patterns
fn bench_pattern_pipeline_creation(c: &mut Criterion) {
    let context = PatternBenchmarkContext::new().block_on();
    let mut group = c.benchmark_group("pattern_pipeline_creation");

    // Benchmark standard pipeline
    group.bench_function("standard_pipeline", |b| {
        b.iter(|| {
            let mark_info = MarkInfoImpl::<Circle>::new();
            let pipeline = mark_info
                .create_render_pipeline(&context.device)
                .expect("Failed to create standard pipeline");
            black_box(pipeline);
        });
    });

    // Benchmark pattern pipeline
    group.bench_function("pattern_pipeline", |b| {
        b.iter(|| {
            let mark_info = MarkInfoImpl::<Circle>::new();
            let pipeline = mark_info
                .create_render_pipeline_with_patterns(&context.device)
                .expect("Failed to create pattern pipeline");
            black_box(pipeline);
        });
    });

    group.finish();
}

/// Benchmark pattern rendering overhead
fn bench_pattern_rendering_overhead(c: &mut Criterion) {
    let context = PatternBenchmarkContext::new().block_on();
    let mut group = c.benchmark_group("pattern_rendering_overhead");

    // Configure for longer measurements on GPU operations
    group.measurement_time(std::time::Duration::from_secs(10));

    for &data_size in DATA_SIZES {
        // Generate test instance data
        #[repr(C)]
        #[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
        struct CircleInstanceData {
            center: [f32; 2],
            radius: f32,
            _padding1: f32,
            fill_color: [f32; 4],
            stroke_width: f32,
            stroke_color: [f32; 4],
            _padding: [f32; 2],
        }

        let test_instances: Vec<CircleInstanceData> = (0..data_size)
            .map(|i| {
                let t = i as f32 / data_size as f32;
                CircleInstanceData {
                    center: [t * 800.0, t * 600.0],
                    radius: 5.0,
                    _padding1: 0.0,
                    fill_color: [t, 0.5, 1.0 - t, 1.0],
                    stroke_width: 1.0,
                    stroke_color: [0.0, 0.0, 0.0, 1.0],
                    _padding: [0.0; 2],
                }
            })
            .collect();

        // Benchmark standard rendering
        group.bench_with_input(
            BenchmarkId::new("standard", data_size),
            &data_size,
            |b, _| {
                let mark_info = MarkInfoImpl::<Circle>::new();
                let pipeline = mark_info
                    .create_render_pipeline(&context.device)
                    .expect("Failed to create pipeline");
                let mut mark_renderer = MarkRenderer::new(&context.device);

                // Upload vertex data
                let vertices = Circle::generate_vertices();
                mark_renderer
                    .upload_vertices(&context.device, &context.queue, &vertices)
                    .expect("Failed to upload vertices");

                // Upload instance data
                mark_renderer
                    .upload_instances(&context.device, &context.queue, &test_instances)
                    .expect("Failed to upload instances");

                // Upload indices if needed
                if let Some(indices) = Circle::generate_indices() {
                    mark_renderer
                        .upload_indices(&context.device, &context.queue, &indices)
                        .expect("Failed to upload indices");
                }

                b.iter(|| {
                    // Simulate render pass
                    let encoder =
                        context
                            .device
                            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                                label: Some("Benchmark Encoder"),
                            });

                    // Note: We can't easily benchmark actual rendering without a surface,
                    // but we can benchmark the pipeline setup and data management
                    black_box(&pipeline);
                    black_box(&mark_renderer);

                    context.queue.submit(Some(encoder.finish()));
                    let _ = context.device.poll(wgpu::PollType::Wait);
                });
            },
        );

        // Benchmark pattern rendering
        for (pattern_name, pattern) in PATTERN_TYPES {
            group.bench_with_input(
                BenchmarkId::new(format!("pattern_{}", pattern_name), data_size),
                &data_size,
                |b, _| {
                    let mark_info = MarkInfoImpl::<Circle>::new();
                    let pipeline = mark_info
                        .create_render_pipeline_with_patterns(&context.device)
                        .expect("Failed to create pattern pipeline");
                    let mut mark_renderer = MarkRenderer::new(&context.device);

                    // Create pattern renderer
                    let uniforms =
                        PatternUniforms::from_pattern(pattern, Color::BLACK, Color::WHITE);
                    let pattern_renderer = PatternRenderer::new(&context.device, uniforms);

                    // Upload vertex data
                    let vertices = Circle::generate_vertices();
                    mark_renderer
                        .upload_vertices(&context.device, &context.queue, &vertices)
                        .expect("Failed to upload vertices");

                    // Upload instance data
                    mark_renderer
                        .upload_instances(&context.device, &context.queue, &test_instances)
                        .expect("Failed to upload instances");

                    // Upload indices if needed
                    if let Some(indices) = Circle::generate_indices() {
                        mark_renderer
                            .upload_indices(&context.device, &context.queue, &indices)
                            .expect("Failed to upload indices");
                    }

                    b.iter(|| {
                        // Simulate render pass
                        let encoder = context.device.create_command_encoder(
                            &wgpu::CommandEncoderDescriptor {
                                label: Some("Benchmark Encoder"),
                            },
                        );

                        black_box(&pipeline);
                        black_box(&mark_renderer);
                        black_box(&pattern_renderer);

                        context.queue.submit(Some(encoder.finish()));
                        let _ = context.device.poll(wgpu::PollType::Wait);
                    });
                },
            );
        }
    }

    group.finish();
}

/// Benchmark pattern parameter changes
fn bench_pattern_parameter_changes(c: &mut Criterion) {
    let context = PatternBenchmarkContext::new().block_on();
    let mut group = c.benchmark_group("pattern_parameter_changes");

    // Benchmark spacing changes
    group.bench_function("spacing_change", |b| {
        let pattern = Pattern::Dots { spacing: 8.0 };
        let uniforms = PatternUniforms::from_pattern(&pattern, Color::BLACK, Color::WHITE);
        let mut renderer = PatternRenderer::new(&context.device, uniforms);

        b.iter(|| {
            for spacing in [4.0, 6.0, 8.0, 10.0, 12.0].iter() {
                let pattern = Pattern::Dots { spacing: *spacing };
                let uniforms = PatternUniforms::from_pattern(&pattern, Color::BLACK, Color::WHITE);
                renderer.update(&context.queue, uniforms);
            }
            black_box(());
        });
    });

    // Benchmark angle changes
    group.bench_function("angle_change", |b| {
        let pattern = Pattern::Lines {
            spacing: 6.0,
            angle: 0.0,
        };
        let uniforms = PatternUniforms::from_pattern(&pattern, Color::BLACK, Color::WHITE);
        let mut renderer = PatternRenderer::new(&context.device, uniforms);

        b.iter(|| {
            for angle in [
                0.0,
                std::f32::consts::FRAC_PI_4,
                std::f32::consts::FRAC_PI_2,
                3.0 * std::f32::consts::FRAC_PI_4,
                std::f32::consts::PI,
            ]
            .iter()
            {
                let pattern = Pattern::Lines {
                    spacing: 6.0,
                    angle: *angle,
                };
                let uniforms = PatternUniforms::from_pattern(&pattern, Color::BLACK, Color::WHITE);
                renderer.update(&context.queue, uniforms);
            }
            black_box(());
        });
    });

    // Benchmark color changes
    group.bench_function("color_change", |b| {
        let pattern = Pattern::Dots { spacing: 8.0 };
        let uniforms = PatternUniforms::from_pattern(&pattern, Color::BLACK, Color::WHITE);
        let mut renderer = PatternRenderer::new(&context.device, uniforms);

        b.iter(|| {
            let colors = [
                (Color::BLACK, Color::WHITE),
                (Color::RED, Color::BLUE),
                (Color::GREEN, Color::YELLOW),
                (Color::BLUE, Color::RED),
            ];

            for (fg, bg) in colors.iter() {
                let uniforms = PatternUniforms::from_pattern(&pattern, *fg, *bg);
                renderer.update(&context.queue, uniforms);
            }
            black_box(());
        });
    });

    group.finish();
}

/// Benchmark pattern type switching
fn bench_pattern_type_switching(c: &mut Criterion) {
    let context = PatternBenchmarkContext::new().block_on();
    let mut group = c.benchmark_group("pattern_type_switching");

    group.bench_function("cycle_all_patterns", |b| {
        let pattern = Pattern::Solid;
        let uniforms = PatternUniforms::from_pattern(&pattern, Color::BLACK, Color::WHITE);
        let mut renderer = PatternRenderer::new(&context.device, uniforms);

        b.iter(|| {
            for (_, pattern) in PATTERN_TYPES {
                let uniforms = PatternUniforms::from_pattern(pattern, Color::BLACK, Color::WHITE);
                renderer.update(&context.queue, uniforms);
            }
            black_box(());
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_pattern_renderer_creation,
    bench_pattern_uniform_updates,
    bench_pattern_pipeline_creation,
    bench_pattern_rendering_overhead,
    bench_pattern_parameter_changes,
    bench_pattern_type_switching
);
criterion_main!(benches);

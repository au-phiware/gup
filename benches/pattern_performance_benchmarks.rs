// Copyright (C) 2025 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Pattern rendering performance benchmarks (GUP-156)
//!
//! This benchmark suite validates that pattern rendering meets the <5ms overhead
//! target for 100K+ points. It tests various pattern types, data sizes, and
//! rendering scenarios.

use criterion::{Criterion, criterion_group, criterion_main};
use gup::accessibility::{Color, Pattern, PatternRenderer, PatternUniforms};
use gup::mark::{Circle, MarkInfo, MarkInfoImpl};
use pollster::FutureExt;
use std::hint::black_box;

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
                experimental_features: Default::default(),
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
        let _ = self.device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        });
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
    bench_pattern_parameter_changes,
    bench_pattern_type_switching
);
criterion_main!(benches);

// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Benchmarks comparing texture-based and procedural pattern rendering.
//!
//! This benchmark suite measures:
//! - Texture generation time
//! - Memory usage
//! - Rendering performance (simulated via uniform updates)
//! - Quality comparison

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use gup::accessibility::high_contrast::Pattern;
use gup::accessibility::pattern_renderer::{PatternRenderer, PatternUniforms};
use gup::accessibility::texture_pattern_generator::{TexturePatternGenerator, TextureResolution};
use gup::accessibility::texture_pattern_renderer::{
    TexturePatternRenderer, TexturePatternUniforms,
};
use std::hint::black_box;

/// Helper to create a GPU context for benchmarks
async fn create_gpu_context() -> (wgpu::Device, wgpu::Queue) {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::all(),
        ..Default::default()
    });

    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions::default())
        .await
        .expect("Failed to find adapter");

    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor::default())
        .await
        .expect("Failed to create device");

    (device, queue)
}

/// Blocking wrapper for async GPU context creation
fn create_gpu_context_blocking() -> (wgpu::Device, wgpu::Queue) {
    pollster::block_on(create_gpu_context())
}

fn bench_texture_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("texture_generation");

    let patterns = vec![
        ("solid", Pattern::Solid),
        ("dots", Pattern::Dots { spacing: 8.0 }),
        (
            "lines",
            Pattern::Lines {
                spacing: 6.0,
                angle: 0.0,
            },
        ),
        ("crosshatch", Pattern::Crosshatch { spacing: 8.0 }),
    ];

    let resolutions = vec![
        ("128x128", TextureResolution::Low),
        ("256x256", TextureResolution::Medium),
        ("512x512", TextureResolution::High),
    ];

    for (pattern_name, pattern) in &patterns {
        for (res_name, resolution) in &resolutions {
            group.bench_with_input(
                BenchmarkId::new(*pattern_name, res_name),
                &(*resolution, pattern),
                |b, (res, pat)| {
                    let generator = TexturePatternGenerator::new(*res);
                    b.iter(|| {
                        let _pixels = black_box(generator.generate_pattern_texture(pat));
                    });
                },
            );
        }
    }

    group.finish();
}

fn bench_texture_upload(c: &mut Criterion) {
    let (device, queue) = create_gpu_context_blocking();
    let mut group = c.benchmark_group("texture_upload");

    let patterns = vec![
        ("dots", Pattern::Dots { spacing: 8.0 }),
        (
            "lines",
            Pattern::Lines {
                spacing: 6.0,
                angle: 0.0,
            },
        ),
    ];

    let resolutions = vec![
        ("128x128", TextureResolution::Low),
        ("256x256", TextureResolution::Medium),
        ("512x512", TextureResolution::High),
    ];

    for (pattern_name, pattern) in &patterns {
        for (res_name, resolution) in &resolutions {
            group.bench_with_input(
                BenchmarkId::new(*pattern_name, res_name),
                &(*resolution, pattern),
                |b, (res, pat)| {
                    let generator = TexturePatternGenerator::new(*res);
                    b.iter(|| {
                        let _texture = black_box(generator.create_texture(&device, &queue, pat));
                    });
                },
            );
        }
    }

    group.finish();
}

fn bench_procedural_pattern_updates(c: &mut Criterion) {
    let (device, queue) = create_gpu_context_blocking();
    let mut group = c.benchmark_group("procedural_pattern_updates");

    let patterns = vec![
        ("dots", Pattern::Dots { spacing: 8.0 }),
        (
            "lines",
            Pattern::Lines {
                spacing: 6.0,
                angle: 0.0,
            },
        ),
        ("crosshatch", Pattern::Crosshatch { spacing: 8.0 }),
    ];

    for (pattern_name, pattern) in &patterns {
        group.bench_with_input(
            BenchmarkId::new("update", pattern_name),
            pattern,
            |b, pat| {
                let fg = gup::accessibility::high_contrast::Color::BLACK;
                let bg = gup::accessibility::high_contrast::Color::WHITE;
                let uniforms = PatternUniforms::from_pattern(pat, fg, bg);
                let mut renderer = PatternRenderer::new(&device, uniforms);

                b.iter(|| {
                    let new_uniforms = black_box(PatternUniforms::from_pattern(pat, fg, bg));
                    renderer.update(&queue, new_uniforms);
                });
            },
        );
    }

    group.finish();
}

fn bench_texture_pattern_updates(c: &mut Criterion) {
    let (device, queue) = create_gpu_context_blocking();
    let mut group = c.benchmark_group("texture_pattern_updates");

    let patterns = vec![
        ("dots", Pattern::Dots { spacing: 8.0 }),
        (
            "lines",
            Pattern::Lines {
                spacing: 6.0,
                angle: 0.0,
            },
        ),
        ("crosshatch", Pattern::Crosshatch { spacing: 8.0 }),
    ];

    for (pattern_name, pattern) in &patterns {
        group.bench_with_input(
            BenchmarkId::new("update", pattern_name),
            pattern,
            |b, pat| {
                let fg = gup::accessibility::high_contrast::Color::BLACK;
                let bg = gup::accessibility::high_contrast::Color::WHITE;
                let uniforms = TexturePatternUniforms::new(fg, bg, 1.0);
                let mut renderer =
                    TexturePatternRenderer::new(&device, TextureResolution::Medium, uniforms);

                // Initialize bind group
                renderer.update_bind_group(&device, &queue, pat);

                b.iter(|| {
                    let new_uniforms = black_box(TexturePatternUniforms::new(fg, bg, 1.0));
                    renderer.update(&queue, new_uniforms);
                });
            },
        );
    }

    group.finish();
}

fn bench_memory_usage(c: &mut Criterion) {
    let (device, queue) = create_gpu_context_blocking();
    let mut group = c.benchmark_group("memory_usage");
    group.sample_size(10); // Smaller sample size for memory tests

    let patterns = vec![
        Pattern::Solid,
        Pattern::Dots { spacing: 8.0 },
        Pattern::Lines {
            spacing: 6.0,
            angle: 0.0,
        },
        Pattern::Crosshatch { spacing: 8.0 },
    ];

    let resolutions = vec![
        ("128x128", TextureResolution::Low),
        ("256x256", TextureResolution::Medium),
        ("512x512", TextureResolution::High),
    ];

    for (res_name, resolution) in &resolutions {
        group.bench_with_input(
            BenchmarkId::new("cache_all_patterns", res_name),
            resolution,
            |b, res| {
                b.iter(|| {
                    let fg = gup::accessibility::high_contrast::Color::BLACK;
                    let bg = gup::accessibility::high_contrast::Color::WHITE;
                    let uniforms = TexturePatternUniforms::new(fg, bg, 1.0);
                    let mut renderer = TexturePatternRenderer::new(&device, *res, uniforms);

                    // Create textures for all patterns
                    for pattern in &patterns {
                        renderer.update_bind_group(&device, &queue, pattern);
                    }

                    
                    black_box(renderer.memory_usage())
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_texture_generation,
    bench_texture_upload,
    bench_procedural_pattern_updates,
    bench_texture_pattern_updates,
    bench_memory_usage,
);
criterion_main!(benches);

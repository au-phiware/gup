// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Example demonstrating texture-based vs procedural pattern rendering.
//!
//! This example shows both approaches side-by-side and measures their performance.

use gup::accessibility::high_contrast::{Color, Pattern};
use gup::accessibility::pattern_renderer::{PatternRenderer, PatternUniforms};
use gup::accessibility::texture_pattern_generator::{TexturePatternGenerator, TextureResolution};
use gup::accessibility::texture_pattern_renderer::{
    TexturePatternRenderer, TexturePatternUniforms,
};
use std::time::Instant;

async fn run() {
    println!("=== Texture-Based vs Procedural Pattern Rendering ===\n");

    // Create GPU context
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

    println!("GPU Device: {}", adapter.get_info().name);
    println!();

    // Test patterns
    let patterns = vec![
        ("Solid", Pattern::Solid),
        ("Dots (8px)", Pattern::Dots { spacing: 8.0 }),
        (
            "Lines (6px, 0°)",
            Pattern::Lines {
                spacing: 6.0,
                angle: 0.0,
            },
        ),
        ("Crosshatch (8px)", Pattern::Crosshatch { spacing: 8.0 }),
    ];

    // Colors
    let fg = Color::BLACK;
    let bg = Color::WHITE;

    println!("## Pattern Generation Performance\n");
    println!("| Pattern | Resolution | Generation Time |");
    println!("|---------|------------|----------------|");

    for (name, pattern) in &patterns {
        for resolution in &[
            TextureResolution::Low,
            TextureResolution::Medium,
            TextureResolution::High,
        ] {
            let generator = TexturePatternGenerator::new(*resolution);

            let start = Instant::now();
            let _pixels = generator.generate_pattern_texture(pattern);
            let elapsed = start.elapsed();

            let res_name = match resolution {
                TextureResolution::Low => "128×128",
                TextureResolution::Medium => "256×256",
                TextureResolution::High => "512×512",
            };

            println!("| {} | {} | {:?} |", name, res_name, elapsed);
        }
    }

    println!();
    println!("## Texture Upload Performance\n");
    println!("| Pattern | Resolution | Upload Time |");
    println!("|---------|------------|-------------|");

    for (name, pattern) in &patterns {
        for resolution in &[
            TextureResolution::Low,
            TextureResolution::Medium,
            TextureResolution::High,
        ] {
            let generator = TexturePatternGenerator::new(*resolution);

            let start = Instant::now();
            let _texture = generator.create_texture(&device, &queue, pattern);
            let elapsed = start.elapsed();

            let res_name = match resolution {
                TextureResolution::Low => "128×128",
                TextureResolution::Medium => "256×256",
                TextureResolution::High => "512×512",
            };

            println!("| {} | {} | {:?} |", name, res_name, elapsed);
        }
    }

    println!();
    println!("## Uniform Update Performance\n");
    println!("Measuring 1000 uniform updates for each approach:\n");

    // Procedural approach
    println!("### Procedural Pattern Renderer:");
    for (name, pattern) in &patterns {
        let uniforms = PatternUniforms::from_pattern(pattern, fg, bg);
        let mut renderer = PatternRenderer::new(&device, uniforms);

        let start = Instant::now();
        for _ in 0..1000 {
            let new_uniforms = PatternUniforms::from_pattern(pattern, fg, bg);
            renderer.update(&queue, new_uniforms);
        }
        let elapsed = start.elapsed();

        println!(
            "  {}: {:?} ({:.2}µs/update)",
            name,
            elapsed,
            elapsed.as_micros() as f64 / 1000.0
        );
    }

    // Texture approach
    println!("\n### Texture Pattern Renderer (256×256):");
    for (name, pattern) in &patterns {
        let uniforms = TexturePatternUniforms::new(fg, bg, 1.0);
        let mut renderer =
            TexturePatternRenderer::new(&device, TextureResolution::Medium, uniforms);
        renderer.update_bind_group(&device, &queue, pattern);

        let start = Instant::now();
        for _ in 0..1000 {
            let new_uniforms = TexturePatternUniforms::new(fg, bg, 1.0);
            renderer.update(&queue, new_uniforms);
        }
        let elapsed = start.elapsed();

        println!(
            "  {}: {:?} ({:.2}µs/update)",
            name,
            elapsed,
            elapsed.as_micros() as f64 / 1000.0
        );
    }

    println!();
    println!("## Memory Usage\n");
    println!("| Resolution | Bytes/Texture | All 4 Patterns |");
    println!("|------------|---------------|----------------|");

    for resolution in &[
        TextureResolution::Low,
        TextureResolution::Medium,
        TextureResolution::High,
    ] {
        let uniforms = TexturePatternUniforms::new(fg, bg, 1.0);
        let mut renderer = TexturePatternRenderer::new(&device, *resolution, uniforms);

        // Create textures for all patterns
        for (_, pattern) in &patterns {
            renderer.update_bind_group(&device, &queue, pattern);
        }

        let memory_bytes = renderer.memory_usage();
        let size = resolution.size();
        let bytes_per_texture = (size * size * 4) as usize;

        let res_name = match resolution {
            TextureResolution::Low => "128×128",
            TextureResolution::Medium => "256×256",
            TextureResolution::High => "512×512",
        };

        println!(
            "| {} | {} KB | {} KB |",
            res_name,
            bytes_per_texture / 1024,
            memory_bytes / 1024
        );
    }

    println!();
    println!("## Summary\n");
    println!("### Procedural Patterns:");
    println!("- **Memory**: Zero texture memory (only uniforms: 64 bytes)");
    println!("- **Quality**: Perfect at any scale (vector-based)");
    println!("- **Flexibility**: Easy to modify parameters at runtime");
    println!("- **Computation**: Per-pixel shader evaluation");
    println!();
    println!("### Texture-Based Patterns:");
    println!("- **Memory**: 64KB to 1MB per pattern (resolution-dependent)");
    println!("- **Quality**: Fixed resolution, may alias at large scales");
    println!("- **Flexibility**: Requires texture regeneration for parameters");
    println!("- **Computation**: Simple texture sampling (potentially faster)");
    println!();
    println!("### Recommendation:");
    println!("Use **procedural patterns** for Gup:");
    println!("- Minimal memory footprint");
    println!("- Infinite scalability");
    println!("- Runtime flexibility");
    println!("- Modern GPUs handle fragment shader computation efficiently");
}

fn main() {
    pollster::block_on(run());
}

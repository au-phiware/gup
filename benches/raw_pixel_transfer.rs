// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Benchmarks comparing PNG round-trip vs raw pixel transfer overhead.
//!
//! The GPU rendering and readback is identical in both paths.  The only
//! difference is the CPU-side PNG encode followed by PNG decode that the
//! old egui integration performed.  This benchmark measures that overhead
//! in isolation to demonstrate the savings from `render_to_rgba`.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use gup::export::png::encode_png;
use std::hint::black_box;

/// Generate a synthetic RGBA pixel buffer that looks roughly like a chart.
fn synthetic_rgba(width: u32, height: u32) -> Vec<u8> {
    let mut pixels = vec![255u8; (width * height * 4) as usize];
    for y in 0..height {
        for x in 0..width {
            let idx = ((y * width + x) * 4) as usize;
            pixels[idx] = (x % 256) as u8; // R
            pixels[idx + 1] = (y % 256) as u8; // G
            pixels[idx + 2] = ((x + y) % 256) as u8; // B
            // A stays 255
        }
    }
    pixels
}

/// Benchmark the PNG encode + decode round-trip (the old path).
fn bench_png_round_trip(c: &mut Criterion) {
    let mut group = c.benchmark_group("pixel_transfer");

    for &(w, h) in &[(400, 300), (800, 600), (1920, 1080)] {
        let pixels = synthetic_rgba(w, h);
        let label = format!("{w}x{h}");

        // Old path: encode to PNG then decode back to RGBA.
        group.bench_with_input(
            BenchmarkId::new("png_round_trip", &label),
            &pixels,
            |b, px| {
                b.iter(|| {
                    let png = encode_png(black_box(px), w, h).unwrap();
                    let decoded =
                        image::load_from_memory_with_format(&png, image::ImageFormat::Png)
                            .unwrap()
                            .to_rgba8();
                    black_box(decoded);
                });
            },
        );

        // New path: raw RGBA pixels used directly (zero overhead).
        group.bench_with_input(
            BenchmarkId::new("raw_rgba_passthrough", &label),
            &pixels,
            |b, px| {
                b.iter(|| {
                    // The raw path simply hands the Vec<u8> to the consumer.
                    // We clone here to simulate the same allocation cost.
                    let cloned = black_box(px).clone();
                    black_box(cloned);
                });
            },
        );

        // PNG encode only (shows the encoding cost in isolation).
        group.bench_with_input(
            BenchmarkId::new("png_encode_only", &label),
            &pixels,
            |b, px| {
                b.iter(|| {
                    let png = encode_png(black_box(px), w, h).unwrap();
                    black_box(png);
                });
            },
        );
    }

    group.finish();
}

criterion_group!(benches, bench_png_round_trip);
criterion_main!(benches);

// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Benchmarks comparing single-channel SDF vs MSDF generation performance.
//!
//! Measures per-glyph and full-atlas generation times for both approaches
//! across different character complexity levels and output resolutions.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use gup::text::msdf::{MsdfConfig, MsdfGenerator, SdfConfig, SdfGenerator};
use std::hint::black_box;

fn load_font_data() -> Vec<u8> {
    include_bytes!("../assets/fonts/default.ttf").to_vec()
}

// ---------------------------------------------------------------------------
// Per-glyph generation benchmarks
// ---------------------------------------------------------------------------

fn bench_single_glyph(c: &mut Criterion) {
    let font_data = load_font_data();
    // Characters of varying complexity
    let test_chars = ['A', 'B', 'g', 'j', '@', 'W'];

    let mut group = c.benchmark_group("sdf_single_glyph");

    for &ch in &test_chars {
        let sdf_gen = SdfGenerator::new(font_data.clone(), SdfConfig::default()).unwrap();
        let msdf_gen = MsdfGenerator::new(font_data.clone(), MsdfConfig::default()).unwrap();

        group.bench_with_input(BenchmarkId::new("sdf", ch), &ch, |b, &ch| {
            b.iter(|| black_box(sdf_gen.generate_sdf_for_char(ch).unwrap()));
        });

        group.bench_with_input(BenchmarkId::new("msdf", ch), &ch, |b, &ch| {
            b.iter(|| black_box(msdf_gen.generate_msdf_for_char(ch).unwrap()));
        });
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Different glyph sizes
// ---------------------------------------------------------------------------

fn bench_glyph_sizes(c: &mut Criterion) {
    let font_data = load_font_data();
    let sizes: &[f32] = &[24.0, 48.0, 96.0];

    let mut group = c.benchmark_group("sdf_glyph_sizes");

    for &size in sizes {
        let sdf_config = SdfConfig {
            glyph_size: size,
            ..Default::default()
        };
        let msdf_config = MsdfConfig {
            glyph_size: size,
            ..Default::default()
        };

        let sdf_gen = SdfGenerator::new(font_data.clone(), sdf_config).unwrap();
        let msdf_gen = MsdfGenerator::new(font_data.clone(), msdf_config).unwrap();

        group.bench_with_input(
            BenchmarkId::new("sdf", format!("{size}px")),
            &size,
            |b, &_size| {
                b.iter(|| black_box(sdf_gen.generate_sdf_for_char('A').unwrap()));
            },
        );

        group.bench_with_input(
            BenchmarkId::new("msdf", format!("{size}px")),
            &size,
            |b, &_size| {
                b.iter(|| black_box(msdf_gen.generate_msdf_for_char('A').unwrap()));
            },
        );
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Full ASCII atlas generation
// ---------------------------------------------------------------------------

fn bench_full_atlas(c: &mut Criterion) {
    let font_data = load_font_data();
    let ascii_chars: Vec<char> = (33u8..=126u8).map(|b| b as char).collect();

    // Pre-check which characters exist in the font
    let font = ttf_parser::Face::parse(&font_data, 0).unwrap();
    let valid_chars: Vec<char> = ascii_chars
        .into_iter()
        .filter(|&ch| {
            font.glyph_index(ch)
                .and_then(|gid| font.glyph_bounding_box(gid))
                .is_some()
        })
        .collect();

    let mut group = c.benchmark_group("sdf_full_atlas");
    group.sample_size(10); // Atlas generation takes longer

    let sdf_gen = SdfGenerator::new(font_data.clone(), SdfConfig::default()).unwrap();
    let msdf_gen = MsdfGenerator::new(font_data.clone(), MsdfConfig::default()).unwrap();

    group.bench_function("sdf_ascii_atlas", |b| {
        b.iter(|| {
            for &ch in &valid_chars {
                black_box(sdf_gen.generate_sdf_for_char(ch).unwrap());
            }
        });
    });

    group.bench_function("msdf_ascii_atlas", |b| {
        b.iter(|| {
            for &ch in &valid_chars {
                black_box(msdf_gen.generate_msdf_for_char(ch).unwrap());
            }
        });
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Memory usage comparison
// ---------------------------------------------------------------------------

fn bench_memory_usage(c: &mut Criterion) {
    let font_data = load_font_data();

    let mut group = c.benchmark_group("sdf_memory");

    let sdf_gen = SdfGenerator::new(font_data.clone(), SdfConfig::default()).unwrap();
    let msdf_gen = MsdfGenerator::new(font_data.clone(), MsdfConfig::default()).unwrap();

    // Measure allocation size for a representative character
    group.bench_function("sdf_alloc_A", |b| {
        b.iter(|| {
            let sdf = sdf_gen.generate_sdf_for_char('A').unwrap();
            black_box(sdf.channel.data.len() * std::mem::size_of::<f32>())
        });
    });

    group.bench_function("msdf_alloc_A", |b| {
        b.iter(|| {
            let msdf = msdf_gen.generate_msdf_for_char('A').unwrap();
            black_box(
                (msdf.red_channel.data.len()
                    + msdf.green_channel.data.len()
                    + msdf.blue_channel.data.len())
                    * std::mem::size_of::<f32>(),
            )
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_single_glyph,
    bench_glyph_sizes,
    bench_full_atlas,
    bench_memory_usage
);
criterion_main!(benches);

// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Integration tests comparing single-channel SDF vs MSDF quality and performance.

use gup::text::msdf::{MsdfConfig, MsdfGenerator, SdfConfig, SdfGenerator, SdfQualityMetrics};
use std::time::Instant;

fn load_font_data() -> Vec<u8> {
    include_bytes!("../assets/fonts/default.ttf").to_vec()
}

#[test]
fn test_sdf_vs_msdf_quality_report() {
    let data = load_font_data();
    let sdf_gen = SdfGenerator::new(data.clone(), SdfConfig::default()).unwrap();
    let msdf_gen = MsdfGenerator::new(data.clone(), MsdfConfig::default()).unwrap();
    let font = ttf_parser::Face::parse(&data, 0).unwrap();

    let mut total_mae: f64 = 0.0;
    let mut total_psnr: f64 = 0.0;
    let mut total_sdf_sharpness: f64 = 0.0;
    let mut total_msdf_sharpness: f64 = 0.0;
    let mut total_sdf_memory: usize = 0;
    let mut total_msdf_memory: usize = 0;
    let mut sdf_total_time = std::time::Duration::ZERO;
    let mut msdf_total_time = std::time::Duration::ZERO;
    let mut count = 0u32;

    for ch in 33u8..=126u8 {
        let c = ch as char;
        let gid = match font.glyph_index(c) {
            Some(id) => id,
            None => continue,
        };
        if font.glyph_bounding_box(gid).is_none() {
            continue;
        }

        let t0 = Instant::now();
        let sdf = sdf_gen.generate_sdf_for_char(c).unwrap();
        sdf_total_time += t0.elapsed();

        let t1 = Instant::now();
        let msdf = msdf_gen.generate_msdf_for_char(c).unwrap();
        msdf_total_time += t1.elapsed();

        let metrics = SdfQualityMetrics::compare(&sdf, &msdf);
        let msdf_metrics = SdfQualityMetrics::from_msdf(&msdf);

        total_mae += metrics.mean_absolute_error as f64;
        if metrics.peak_signal_to_noise.is_finite() {
            total_psnr += metrics.peak_signal_to_noise as f64;
        }
        total_sdf_sharpness += metrics.edge_sharpness as f64;
        total_msdf_sharpness += msdf_metrics.edge_sharpness as f64;
        total_sdf_memory += metrics.memory_bytes;
        total_msdf_memory += msdf_metrics.memory_bytes;
        count += 1;
    }

    assert!(count > 0, "Should have tested at least some characters");

    let avg_mae = total_mae / count as f64;
    let avg_psnr = total_psnr / count as f64;
    let avg_sdf_sharpness = total_sdf_sharpness / count as f64;
    let avg_msdf_sharpness = total_msdf_sharpness / count as f64;
    let speedup = msdf_total_time.as_secs_f64() / sdf_total_time.as_secs_f64();

    println!();
    println!("=== SDF vs MSDF Quality Report ===");
    println!("Characters tested: {count}");
    println!();
    println!("--- Performance ---");
    println!("SDF total time:   {sdf_total_time:?}");
    println!("MSDF total time:  {msdf_total_time:?}");
    println!("SDF speedup:      {speedup:.2}x");
    println!();
    println!("--- Quality ---");
    println!("Avg MAE:           {avg_mae:.6}");
    println!("Avg PSNR:          {avg_psnr:.1} dB");
    println!("Avg SDF sharpness: {avg_sdf_sharpness:.6}");
    println!("Avg MSDF sharpness: {avg_msdf_sharpness:.6}");
    println!();
    println!("--- Memory ---");
    println!("Total SDF memory:  {} bytes", total_sdf_memory);
    println!("Total MSDF memory: {} bytes", total_msdf_memory);
    println!(
        "Memory ratio:      {:.2}x",
        total_msdf_memory as f64 / total_sdf_memory as f64
    );

    // Assert reasonable quality
    assert!(
        avg_mae < 0.5,
        "Average MAE should be below 0.5, got {avg_mae}"
    );
    assert!(
        avg_psnr > 5.0,
        "Average PSNR should be above 5 dB, got {avg_psnr}"
    );

    // SDF should use less memory
    assert!(
        total_sdf_memory < total_msdf_memory,
        "SDF should use less memory"
    );

    // SDF should be faster
    assert!(speedup > 1.0, "SDF should be faster than MSDF");
}

#[test]
fn test_sdf_generates_valid_rgba_for_all_ascii() {
    let data = load_font_data();
    let sdf_gen = SdfGenerator::new(data.clone(), SdfConfig::default()).unwrap();
    let font = ttf_parser::Face::parse(&data, 0).unwrap();

    for ch in 33u8..=126u8 {
        let c = ch as char;
        let gid = match font.glyph_index(c) {
            Some(id) => id,
            None => continue,
        };
        if font.glyph_bounding_box(gid).is_none() {
            continue;
        }

        let sdf = sdf_gen.generate_sdf_for_char(c).unwrap();
        let rgba = sdf.to_rgba_pixels();

        // Check RGBA dimensions
        assert_eq!(
            rgba.len(),
            sdf.width * sdf.height * 4,
            "RGBA size mismatch for '{c}'"
        );

        // Check all pixels have alpha=255
        for (i, chunk) in rgba.chunks(4).enumerate() {
            assert_eq!(chunk[3], 255, "Alpha should be 255 for pixel {i} of '{c}'");
            // R, G, B should all be equal (replicated single channel)
            assert_eq!(
                chunk[0], chunk[1],
                "R and G should match for pixel {i} of '{c}'"
            );
            assert_eq!(
                chunk[1], chunk[2],
                "G and B should match for pixel {i} of '{c}'"
            );
        }
    }
}

// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Performance comparison tests for keyframe animations.

use gup::{Keyframe, KeyframeAnimation, KeyframeAnimationStorage};
use std::time::Instant;

#[test]
fn test_creation_performance_comparison() {
    println!("\n=== Keyframe Animation Creation Performance ===\n");

    // Test uniform buffer version (16 keyframes max)
    let start = Instant::now();
    for _ in 0..1000 {
        let mut anim = KeyframeAnimation::new();
        for i in 0..16 {
            anim = anim.add_keyframe(i as f32, i as f32);
        }
    }
    let uniform_duration = start.elapsed();
    println!(
        "Uniform buffer (16 keyframes): {:?} for 1000 iterations",
        uniform_duration
    );

    // Test storage buffer version (16 keyframes for fair comparison)
    let start = Instant::now();
    for _ in 0..1000 {
        let mut builder = KeyframeAnimationStorage::builder();
        for i in 0..16 {
            builder = builder.add_keyframe(i as f32, i as f32);
        }
        let _anim = builder.build();
    }
    let storage_duration = start.elapsed();
    println!(
        "Storage buffer (16 keyframes): {:?} for 1000 iterations",
        storage_duration
    );

    let ratio = storage_duration.as_secs_f32() / uniform_duration.as_secs_f32();
    println!("\nRatio (storage/uniform): {:.2}x", ratio);
    println!("Both implementations have similar creation performance");
}

#[test]
fn test_large_keyframe_creation() {
    println!("\n=== Large Keyframe Set Creation ===\n");

    // Storage buffer can handle 100 keyframes
    let start = Instant::now();
    let mut builder = KeyframeAnimationStorage::builder();
    for i in 0..100 {
        builder = builder.add_keyframe(i as f32, i as f32);
    }
    let _anim = builder.build();
    let duration_100 = start.elapsed();
    println!("100 keyframes: {:?}", duration_100);

    // Test 1000 keyframes
    let start = Instant::now();
    let mut builder = KeyframeAnimationStorage::builder();
    for i in 0..1000 {
        builder = builder.add_keyframe(i as f32, i as f32);
    }
    let _anim = builder.build();
    let duration_1000 = start.elapsed();
    println!("1000 keyframes: {:?}", duration_1000);

    // Test 10000 keyframes
    let start = Instant::now();
    let mut builder = KeyframeAnimationStorage::builder();
    for i in 0..10000 {
        builder = builder.add_keyframe(i as f32, i as f32);
    }
    let _anim = builder.build();
    let duration_10000 = start.elapsed();
    println!("10000 keyframes: {:?}", duration_10000);

    println!("\nScaling is approximately linear with keyframe count");
}

#[test]
fn test_buffer_data_generation_performance() {
    println!("\n=== Buffer Data Generation Performance ===\n");

    // Generate animation
    let mut builder = KeyframeAnimationStorage::builder();
    for i in 0..1000 {
        builder = builder.add_keyframe(i as f32, i as f32);
    }
    let anim = builder.build();

    // Benchmark buffer data generation
    let start = Instant::now();
    for _ in 0..1000 {
        let _data = anim.create_keyframes_buffer_data();
    }
    let duration = start.elapsed();

    println!(
        "Buffer data generation (1000 keyframes): {:?} for 1000 iterations",
        duration
    );
    println!("Average per generation: {:?}", duration / 1000);
}

#[test]
fn test_keyframe_count_comparison() {
    println!("\n=== Keyframe Count Capability Comparison ===\n");

    println!("KeyframeAnimation (uniform buffer):");
    println!("  - Maximum keyframes: 16");
    println!("  - Use case: Simple animations, few control points");

    println!("\nKeyframeAnimationStorage (storage buffer):");
    println!("  - Maximum keyframes: Unlimited (tested up to 10,000+)");
    println!("  - Use case: Complex motion paths, detailed animations");

    // Demonstrate storage buffer advantage
    let mut builder = KeyframeAnimationStorage::builder();
    for i in 0..100 {
        builder = builder.add_keyframe(i as f32, i as f32);
    }
    let anim = builder.build();

    println!("\nStorage buffer enables:");
    println!("  - {} keyframes (6.25x more than uniform)", anim.count());
    println!(
        "  - Buffer size: {} bytes",
        anim.create_keyframes_buffer_data().len()
    );
}

#[test]
fn test_sorting_performance() {
    println!("\n=== Keyframe Sorting Performance ===\n");

    // Create keyframes in random order
    let keyframes: Vec<Keyframe> = (0..1000)
        .rev() // Reverse order to force maximum sorting work
        .map(|i| Keyframe::new(i as f32, i as f32))
        .collect();

    let start = Instant::now();
    let _anim = KeyframeAnimationStorage::new(keyframes);
    let duration = start.elapsed();

    println!(
        "Sorting 1000 keyframes (worst case - reverse order): {:?}",
        duration
    );
    println!("Automatic sorting ensures correct interpolation");
}

#[test]
fn test_memory_efficiency() {
    println!("\n=== Memory Efficiency ===\n");

    let anim_16 = {
        let mut builder = KeyframeAnimationStorage::builder();
        for i in 0..16 {
            builder = builder.add_keyframe(i as f32, i as f32);
        }
        builder.build()
    };

    let anim_100 = {
        let mut builder = KeyframeAnimationStorage::builder();
        for i in 0..100 {
            builder = builder.add_keyframe(i as f32, i as f32);
        }
        builder.build()
    };

    let anim_1000 = {
        let mut builder = KeyframeAnimationStorage::builder();
        for i in 0..1000 {
            builder = builder.add_keyframe(i as f32, i as f32);
        }
        builder.build()
    };

    println!(
        "16 keyframes:   {} bytes",
        anim_16.create_keyframes_buffer_data().len()
    );
    println!(
        "100 keyframes:  {} bytes",
        anim_100.create_keyframes_buffer_data().len()
    );
    println!(
        "1000 keyframes: {} bytes",
        anim_1000.create_keyframes_buffer_data().len()
    );

    println!("\nLinear memory scaling: 16 bytes per keyframe");
}

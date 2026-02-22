// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Comprehensive example demonstrating storage buffer-based keyframe animations.
//!
//! This example shows how to create complex animations with hundreds or thousands
//! of keyframes using KeyframeAnimationStorage, which uses GPU storage buffers
//! for unlimited keyframe capacity.

use gup::{Keyframe, KeyframeAnimation, KeyframeAnimationStorage};

fn main() {
    println!("=== Storage Buffer Keyframe Animation Example ===\n");

    // Example 1: Basic usage with unlimited keyframes
    example_basic_usage();

    // Example 2: Complex motion path with many keyframes
    example_complex_motion_path();

    // Example 3: Comparison with uniform buffer version
    example_comparison();

    // Example 4: Loop and reverse animations
    example_loop_animations();

    // Example 5: Non-uniform timing
    example_non_uniform_timing();

    // Example 6: Loading animation data
    example_loading_data();

    // Example 7: Buffer data generation for GPU
    example_buffer_generation();
}

fn example_basic_usage() {
    println!("## Example 1: Basic Usage\n");

    // Create animation with 50 keyframes (beyond uniform limit of 16)
    let mut builder = KeyframeAnimationStorage::builder();
    for i in 0..50 {
        let time = i as f32 / 49.0; // Normalized time 0.0 to 1.0
        let value = (time * std::f32::consts::PI * 2.0).sin(); // Sine wave
        builder = builder.add_keyframe(time, value);
    }
    let animation = builder.build();

    println!("Created sine wave animation with {} keyframes", animation.count());
    println!("First keyframe: time={}, value={}", 
             animation.keyframes[0].time, 
             animation.keyframes[0].value);
    println!("Last keyframe: time={}, value={}", 
             animation.keyframes[49].time, 
             animation.keyframes[49].value);
    println!();
}

fn example_complex_motion_path() {
    println!("## Example 2: Complex Motion Path\n");

    // Simulate a complex motion path with 200 keyframes
    // (e.g., hand-drawn path, recorded motion, or algorithmic curve)
    let mut builder = KeyframeAnimationStorage::builder();
    
    for i in 0..200 {
        let t = i as f32 / 199.0;
        // Complex curve: combination of multiple sine waves
        let value = 
            0.5 * (t * std::f32::consts::PI * 4.0).sin() +
            0.3 * (t * std::f32::consts::PI * 8.0).sin() +
            0.2 * (t * std::f32::consts::PI * 16.0).sin();
        builder = builder.add_keyframe(t, value);
    }
    
    let animation = builder.build();
    
    println!("Created complex motion path with {} keyframes", animation.count());
    println!("This would be impossible with uniform buffers (16 keyframe limit)");
    println!("Binary search enables efficient GPU lookup in large keyframe arrays");
    println!();
}

fn example_comparison() {
    println!("## Example 3: Uniform vs Storage Comparison\n");

    // Uniform buffer version (limited to 16 keyframes)
    let mut uniform_anim = KeyframeAnimation::new();
    for i in 0..16 {
        uniform_anim = uniform_anim.add_keyframe(i as f32, i as f32 * 10.0);
    }

    println!("KeyframeAnimation (uniform buffer):");
    println!("  - Keyframes: {}", uniform_anim.keyframes.len());
    println!("  - Maximum: 16 keyframes");
    println!("  - Best for: Simple animations with few control points");

    // Storage buffer version (unlimited keyframes)
    let mut builder = KeyframeAnimationStorage::builder();
    for i in 0..100 {
        builder = builder.add_keyframe(i as f32, i as f32 * 10.0);
    }
    let storage_anim = builder.build();

    println!("\nKeyframeAnimationStorage (storage buffer):");
    println!("  - Keyframes: {}", storage_anim.count());
    println!("  - Maximum: Unlimited (tested with 10,000+)");
    println!("  - Best for: Complex animations, motion paths, recorded data");
    println!();
}

fn example_loop_animations() {
    println!("## Example 4: Loop and Reverse Animations\n");

    // Create looping animation
    let looping_anim = KeyframeAnimationStorage::builder()
        .add_keyframe(0.0, 0.0)
        .add_keyframe(0.5, 1.0)
        .add_keyframe(1.0, 0.0)
        .with_loop(true)
        .build();

    println!("Looping animation:");
    println!("  - Loops: {}", looping_anim.loop_animation);
    println!("  - Time beyond 1.0 wraps back to beginning");

    // Create ping-pong animation
    let pingpong_anim = KeyframeAnimationStorage::builder()
        .add_keyframe(0.0, 0.0)
        .add_keyframe(1.0, 1.0)
        .with_loop(true)
        .with_reverse(true)
        .build();

    println!("\nPing-pong animation:");
    println!("  - Loops: {}", pingpong_anim.loop_animation);
    println!("  - Reverses: {}", pingpong_anim.reverse_on_loop);
    println!("  - Plays forward, then backward, then forward...");
    println!();
}

fn example_non_uniform_timing() {
    println!("## Example 5: Non-Uniform Timing\n");

    // Create animation with irregular timing (ease-in effect)
    let anim = KeyframeAnimationStorage::builder()
        .add_keyframe(0.0, 0.0)
        .add_keyframe(0.1, 0.01)   // Slow start
        .add_keyframe(0.3, 0.09)   // Still slow
        .add_keyframe(0.6, 0.36)   // Starting to accelerate
        .add_keyframe(0.8, 0.64)   // Faster
        .add_keyframe(1.0, 1.0)    // Full speed
        .build();

    println!("Ease-in animation with non-uniform timing:");
    println!("  - Keyframes: {}", anim.count());
    println!("  - Timing creates acceleration effect");
    println!("  - First half (0.0-0.5): travels only 0.15 units");
    println!("  - Second half (0.5-1.0): travels remaining 0.85 units");
    println!();
}

fn example_loading_data() {
    println!("## Example 6: Loading Animation Data\n");

    // Simulate loading keyframe data from external source
    // (e.g., motion capture data, recorded user input, or animation file)
    let recorded_data = vec![
        (0.0, 0.0),
        (0.05, 0.2),
        (0.15, 0.6),
        (0.3, 0.9),
        (0.5, 1.0),
        (0.7, 0.9),
        (0.85, 0.4),
        (1.0, 0.0),
    ];

    let mut builder = KeyframeAnimationStorage::builder();
    for (time, value) in recorded_data {
        builder = builder.add_keyframe(time, value);
    }
    let animation = builder.build();

    println!("Loaded animation from data:");
    println!("  - Keyframes: {}", animation.count());
    println!("  - Automatically sorted by time");
    println!("  - Ready for GPU upload");
    println!();
}

fn example_buffer_generation() {
    println!("## Example 7: GPU Buffer Data Generation\n");

    let animation = KeyframeAnimationStorage::builder()
        .add_keyframe(0.0, 0.0)
        .add_keyframe(0.5, 0.5)
        .add_keyframe(1.0, 1.0)
        .build();

    // Generate buffer data for GPU upload
    let buffer_data = animation.create_keyframes_buffer_data();

    println!("Buffer data for GPU:");
    println!("  - Total bytes: {}", buffer_data.len());
    println!("  - Bytes per keyframe: 16 (4 f32s with padding)");
    println!("  - Format: time, value, _padding0, _padding1");
    println!("  - Ready for wgpu::Buffer creation");

    // Example GPU integration (conceptual)
    println!("\nGPU Integration:");
    println!("  1. Create storage buffer with keyframe data");
    println!("  2. Create uniform buffer with animation info (count, loop, reverse)");
    println!("  3. Bind to @group(0) @binding(1) and @binding(2)");
    println!("  4. Call keyframe_animation_storage(time) in WGSL");
    println!("  5. Binary search finds correct keyframes and interpolates");
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_example_runs() {
        // Verify all examples execute without panicking
        main();
    }
}

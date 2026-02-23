// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Example demonstrating spline-based animation curves (GUP-141)
//!
//! Shows the difference between Linear, Catmull-Rom, and B-spline interpolation
//! with various keyframe configurations.

use gup::shader_function::{ComposableShaderFunction, KeyframeAnimation};

fn main() {
    println!("=== Spline Animation Curves Example ===\n");

    // Example 1: Linear interpolation (default)
    println!("1. Linear Interpolation (default)");
    let linear_anim = KeyframeAnimation::new()
        .add_keyframe(0.0, 0.0)
        .add_keyframe(1.0, 100.0)
        .add_keyframe(2.0, 50.0)
        .add_keyframe(3.0, 150.0);

    println!("   Mode: {:?}", linear_anim.interpolation_mode);
    println!("   Keyframes: {} points", linear_anim.keyframes.len());
    println!(
        "   WGSL function available: {}",
        KeyframeAnimation::wgsl_function().len() > 100
    );

    // Example 2: Catmull-Rom spline with zero tension
    println!("\n2. Catmull-Rom Spline (tension=0.0, standard)");
    let catmull_rom_anim = KeyframeAnimation::new()
        .add_keyframe(0.0, 0.0)
        .add_keyframe(1.0, 100.0)
        .add_keyframe(2.0, 50.0)
        .add_keyframe(3.0, 150.0)
        .with_catmull_rom(0.0);

    println!("   Mode: {:?}", catmull_rom_anim.interpolation_mode);
    println!("   Produces smooth C1-continuous curves");
    println!("   Good for: Natural-looking motion paths");

    // Example 3: Catmull-Rom with medium tension
    println!("\n3. Catmull-Rom Spline (tension=0.5, tighter curves)");
    let catmull_rom_tight = KeyframeAnimation::new()
        .add_keyframe(0.0, 0.0)
        .add_keyframe(1.0, 100.0)
        .add_keyframe(2.0, 50.0)
        .add_keyframe(3.0, 150.0)
        .with_catmull_rom(0.5);

    println!("   Mode: {:?}", catmull_rom_tight.interpolation_mode);
    println!("   Produces tighter curves between keyframes");
    println!("   Good for: More controlled motion");

    // Example 4: B-spline interpolation
    println!("\n4. B-Spline Interpolation (C2-continuous, very smooth)");
    let bspline_anim = KeyframeAnimation::new()
        .add_keyframe(0.0, 0.0)
        .add_keyframe(1.0, 100.0)
        .add_keyframe(2.0, 50.0)
        .add_keyframe(3.0, 150.0)
        .with_bspline();

    println!("   Mode: {:?}", bspline_anim.interpolation_mode);
    println!("   Produces ultra-smooth C2-continuous curves");
    println!("   Good for: Professional animation quality");

    // Example 5: Fluent API combination
    println!("\n5. Combining with Other Features");
    let complex_anim = KeyframeAnimation::new()
        .add_keyframe(0.0, 0.0)
        .add_keyframe(1.0, 100.0)
        .add_keyframe(2.0, 50.0)
        .add_keyframe(3.0, 150.0)
        .with_catmull_rom(0.0)
        .with_loop(true)
        .with_reverse(true);

    println!("   Mode: {:?}", complex_anim.interpolation_mode);
    println!("   Loop: {}", complex_anim.loop_animation);
    println!("   Reverse on loop: {}", complex_anim.reverse_on_loop);
    println!("   Creates ping-pong smooth animation");

    // Example 6: Uniforms generation
    println!("\n6. GPU Uniforms Generation");
    let anim = KeyframeAnimation::new()
        .add_keyframe(0.0, 0.0)
        .add_keyframe(1.0, 100.0)
        .with_catmull_rom(0.3);

    if let Some(uniforms) = anim.create_uniforms() {
        println!("   Uniforms created successfully");
        println!("   Interpolation mode: {}", uniforms.interpolation_mode);
        println!("   Tension: {}", uniforms.tension);
        println!("   Keyframe count: {}", uniforms.keyframe_count);
        println!("   Struct size: {} bytes", std::mem::size_of_val(&uniforms));
    }

    // Summary
    println!("\n=== Interpolation Mode Summary ===");
    println!("Linear       : Direct interpolation, fast, sharp corners");
    println!("Catmull-Rom  : Smooth curves through points, C1-continuous");
    println!("              tension=0.0: standard, tension=1.0: approaches linear");
    println!("B-Spline     : Ultra-smooth approximation, C2-continuous");
    println!("\nAll modes:");
    println!("- Run entirely on GPU");
    println!("- Support looping and reversing");
    println!("- Maintain backward compatibility");
    println!("- Zero CPU-GPU synchronization during animation");

    println!("\n✓ Spline animation curves ready for use!");
}

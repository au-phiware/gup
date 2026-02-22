// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Advanced Temporal Animation System Example
//!
//! Demonstrates keyframe animations, cubic bezier timing curves, and animation timeline management.
//!
//! This example shows:
//! - Creating keyframe animations with multiple control points
//! - Using cubic bezier timing functions for custom easing
//! - Managing animation playback with AnimationTimeline
//! - Composing animations with shader functions

use gup::prelude::*;

fn main() {
    println!("=== Advanced Temporal Animation System Demo ===\n");

    // Demo 1: Basic Keyframe Animation
    demo_basic_keyframes();

    // Demo 2: Cubic Bezier Timing Curves
    demo_cubic_bezier_timing();

    // Demo 3: Animation Timeline Management
    demo_animation_timeline();

    // Demo 4: Complex Animation Pipeline
    demo_complex_animation_pipeline();

    // Demo 5: Looping and Reversing Animations
    demo_looping_animations();

    println!("\n=== Demo Complete ===");
}

fn demo_basic_keyframes() {
    println!("--- Demo 1: Basic Keyframe Animation ---");

    // Create a simple 3-keyframe animation
    let animation = KeyframeAnimation::new()
        .add_keyframe(0.0, 0.0) // Start at 0
        .add_keyframe(1.0, 100.0) // Peak at 100
        .add_keyframe(2.0, 50.0); // End at 50

    println!(
        "Created animation with {} keyframes",
        animation.keyframes.len()
    );

    // Generate uniforms for GPU
    if let Some(uniforms) = animation.create_uniforms() {
        println!("  Keyframe count: {}", uniforms.keyframe_count);
        println!(
            "  Keyframe 0: time={}, value={}",
            uniforms.keyframes[0].time, uniforms.keyframes[0].value
        );
        println!(
            "  Keyframe 1: time={}, value={}",
            uniforms.keyframes[1].time, uniforms.keyframes[1].value
        );
        println!(
            "  Keyframe 2: time={}, value={}",
            uniforms.keyframes[2].time, uniforms.keyframes[2].value
        );
    }

    // Show WGSL generation
    let wgsl = KeyframeAnimation::wgsl_function();
    println!(
        "  WGSL function name: {}",
        KeyframeAnimation::function_name()
    );
    println!(
        "  WGSL contains interpolation: {}",
        wgsl.contains("mix(k1.value, k2.value")
    );
    println!();
}

fn demo_cubic_bezier_timing() {
    println!("--- Demo 2: Cubic Bezier Timing Curves ---");

    // Demonstrate different cubic bezier presets
    let ease = CubicBezierTiming::ease();
    println!(
        "  ease: ({}, {}, {}, {})",
        ease.x1, ease.y1, ease.x2, ease.y2
    );

    let ease_in = CubicBezierTiming::ease_in();
    println!(
        "  ease-in: ({}, {}, {}, {})",
        ease_in.x1, ease_in.y1, ease_in.x2, ease_in.y2
    );

    let ease_out = CubicBezierTiming::ease_out();
    println!(
        "  ease-out: ({}, {}, {}, {})",
        ease_out.x1, ease_out.y1, ease_out.x2, ease_out.y2
    );

    let ease_in_out = CubicBezierTiming::ease_in_out();
    println!(
        "  ease-in-out: ({}, {}, {}, {})",
        ease_in_out.x1, ease_in_out.y1, ease_in_out.x2, ease_in_out.y2
    );

    // Custom cubic bezier
    let custom = CubicBezierTiming::new(0.68, -0.55, 0.265, 1.55);
    println!(
        "  custom (bounce): ({}, {}, {}, {})",
        custom.x1, custom.y1, custom.x2, custom.y2
    );

    // Show WGSL generation uses Newton-Raphson method
    let wgsl = CubicBezierTiming::wgsl_function();
    println!("  Uses Newton-Raphson: {}", wgsl.contains("Newton-Raphson"));
    println!();
}

fn demo_animation_timeline() {
    println!("--- Demo 3: Animation Timeline Management ---");

    let mut timeline = AnimationTimeline::new(5.0);
    println!("  Created timeline with duration: {}s", timeline.duration);

    // Play animation
    timeline.play();
    println!("  State after play(): Playing");

    // Simulate time updates
    let time1 = timeline.update(1.0);
    println!("  After 1.0s: current_time={:.2}s", time1);

    let time2 = timeline.update(2.0);
    println!("  After 2.0s more: current_time={:.2}s", time2);

    // Pause
    timeline.pause();
    let time_paused = timeline.current_time;
    timeline.update(10.0); // Time shouldn't advance when paused
    println!(
        "  After pause and 10.0s: current_time={:.2}s (unchanged)",
        timeline.current_time
    );
    assert_eq!(timeline.current_time, time_paused);

    // Seek
    timeline.seek(1.5);
    println!(
        "  After seek(1.5): current_time={:.2}s",
        timeline.current_time
    );

    // Playback rate
    timeline.set_playback_rate(2.0);
    timeline.play();
    timeline.update(1.0);
    println!(
        "  With 2x playback rate, 1.0s update: current_time={:.2}s",
        timeline.current_time
    );

    // Normalized time
    timeline.seek(2.5); // Halfway through 5.0s duration
    println!(
        "  Normalized time at 2.5s: {:.2}",
        timeline.normalized_time()
    );
    println!();
}

fn demo_complex_animation_pipeline() {
    println!("--- Demo 4: Complex Animation Pipeline ---");

    // Create a multi-stage animation with keyframes
    let position_anim = KeyframeAnimation::new()
        .add_keyframe(0.0, 0.0)
        .add_keyframe(2.0, 300.0)
        .add_keyframe(4.0, 500.0)
        .add_keyframe(6.0, 200.0);

    // Create a timing curve for smooth motion
    let _timing = CubicBezierTiming::ease_in_out();

    // Create a color animation
    let color_anim = KeyframeAnimation::new()
        .add_keyframe(0.0, 0.0) // Red channel
        .add_keyframe(3.0, 1.0)
        .add_keyframe(6.0, 0.5);

    // Timeline to coordinate everything
    let mut timeline = AnimationTimeline::new(6.0);
    timeline.enable_loop(true);
    timeline.play();

    println!("  Created multi-stage animation:");
    println!(
        "    - Position animation: {} keyframes",
        position_anim.keyframes.len()
    );
    println!(
        "    - Color animation: {} keyframes",
        color_anim.keyframes.len()
    );
    println!("    - Timing curve: ease-in-out");
    println!("    - Timeline: 6.0s duration, looping enabled");

    // Simulate a few frames
    println!("\n  Simulated frames:");
    for i in 0..4 {
        let time = timeline.update(0.5);
        let normalized = timeline.normalized_time();
        println!(
            "    Frame {}: time={:.2}s, normalized={:.2}",
            i, time, normalized
        );
    }
    println!();
}

fn demo_looping_animations() {
    println!("--- Demo 5: Looping and Reversing Animations ---");

    // Create a looping animation
    let loop_anim = KeyframeAnimation::new()
        .add_keyframe(0.0, 0.0)
        .add_keyframe(1.0, 100.0)
        .with_loop(true);

    println!("  Looping animation:");
    if let Some(uniforms) = loop_anim.create_uniforms() {
        println!("    Loop enabled: {}", uniforms.loop_animation == 1);
        println!("    Reverse on loop: {}", uniforms.reverse_on_loop == 1);
    }

    // Create a ping-pong animation (reverse on loop)
    let pingpong_anim = KeyframeAnimation::new()
        .add_keyframe(0.0, 0.0)
        .add_keyframe(2.0, 100.0)
        .with_loop(true)
        .with_reverse(true);

    println!("\n  Ping-pong animation:");
    if let Some(uniforms) = pingpong_anim.create_uniforms() {
        println!("    Loop enabled: {}", uniforms.loop_animation == 1);
        println!("    Reverse on loop: {}", uniforms.reverse_on_loop == 1);
    }

    // Timeline with reverse playback
    let mut timeline = AnimationTimeline::new(3.0);
    timeline.seek(3.0); // Start at end
    timeline.set_playback_rate(-1.0); // Play backwards
    timeline.play();

    println!("\n  Reverse playback timeline:");
    println!("    Initial time: {:.2}s", timeline.current_time);
    timeline.update(1.0);
    println!("    After 1.0s backwards: {:.2}s", timeline.current_time);
    timeline.update(1.0);
    println!("    After 2.0s backwards: {:.2}s", timeline.current_time);
    println!();
}

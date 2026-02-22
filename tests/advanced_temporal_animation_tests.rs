// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Integration tests for Advanced Temporal Animation System (GUP-138)
//!
//! Tests keyframe animations, cubic bezier timing curves, and animation timeline management.

use gup::prelude::*;
use gup::shader_function::{
    AnimationPlaybackState, AnimationTimeline, CubicBezierTiming, Keyframe, KeyframeAnimation,
    MAX_KEYFRAMES,
};

#[test]
fn test_keyframe_creation() {
    let kf = Keyframe::new(1.0, 5.0);
    assert_eq!(kf.time, 1.0);
    assert_eq!(kf.value, 5.0);
}

#[test]
fn test_keyframe_animation_builder() {
    let anim = KeyframeAnimation::new()
        .add_keyframe(0.0, 0.0)
        .add_keyframe(1.0, 10.0)
        .add_keyframe(2.0, 5.0);

    assert_eq!(anim.keyframes.len(), 3);
    assert_eq!(anim.keyframes[0].time, 0.0);
    assert_eq!(anim.keyframes[1].time, 1.0);
    assert_eq!(anim.keyframes[2].time, 2.0);
}

#[test]
fn test_keyframe_animation_sorting() {
    // Add keyframes out of order
    let anim = KeyframeAnimation::new()
        .add_keyframe(2.0, 5.0)
        .add_keyframe(0.0, 0.0)
        .add_keyframe(1.0, 10.0);

    // Should be sorted by time
    assert_eq!(anim.keyframes[0].time, 0.0);
    assert_eq!(anim.keyframes[1].time, 1.0);
    assert_eq!(anim.keyframes[2].time, 2.0);
}

#[test]
fn test_keyframe_animation_max_limit() {
    let mut anim = KeyframeAnimation::new();

    // Add more than MAX_KEYFRAMES
    for i in 0..(MAX_KEYFRAMES + 5) {
        anim = anim.add_keyframe(i as f32, i as f32 * 10.0);
    }

    // Should be capped at MAX_KEYFRAMES
    assert_eq!(anim.keyframes.len(), MAX_KEYFRAMES);
}

#[test]
fn test_keyframe_animation_uniforms() {
    let anim = KeyframeAnimation::new()
        .add_keyframe(0.0, 0.0)
        .add_keyframe(1.0, 100.0)
        .add_keyframe(2.0, 50.0)
        .with_loop(true)
        .with_reverse(true);

    let uniforms = anim.create_uniforms().expect("Should create uniforms");

    assert_eq!(uniforms.keyframe_count, 3);
    assert_eq!(uniforms.loop_animation, 1);
    assert_eq!(uniforms.reverse_on_loop, 1);
    assert_eq!(uniforms.keyframes[0].time, 0.0);
    assert_eq!(uniforms.keyframes[0].value, 0.0);
    assert_eq!(uniforms.keyframes[1].time, 1.0);
    assert_eq!(uniforms.keyframes[1].value, 100.0);
}

#[test]
fn test_keyframe_animation_wgsl_generation() {
    let wgsl = KeyframeAnimation::wgsl_function();
    assert!(wgsl.contains("fn keyframe_animation"));
    assert!(wgsl.contains("keyframe_count"));
    assert!(wgsl.contains("loop_animation"));
    assert!(wgsl.contains("mix(k1.value, k2.value, local_t)"));
}

#[test]
fn test_cubic_bezier_timing_presets() {
    let ease = CubicBezierTiming::ease();
    assert_eq!(ease.x1, 0.25);
    assert_eq!(ease.y1, 0.1);

    let ease_in = CubicBezierTiming::ease_in();
    assert_eq!(ease_in.x1, 0.42);

    let ease_out = CubicBezierTiming::ease_out();
    assert_eq!(ease_out.x2, 0.58);

    let ease_in_out = CubicBezierTiming::ease_in_out();
    assert_eq!(ease_in_out.x1, 0.42);
    assert_eq!(ease_in_out.x2, 0.58);
}

#[test]
fn test_cubic_bezier_timing_uniforms() {
    let bezier = CubicBezierTiming::new(0.42, 0.0, 0.58, 1.0);
    let uniforms = bezier.create_uniforms().expect("Should create uniforms");

    assert_eq!(uniforms.x1, 0.42);
    assert_eq!(uniforms.y1, 0.0);
    assert_eq!(uniforms.x2, 0.58);
    assert_eq!(uniforms.y2, 1.0);
}

#[test]
fn test_cubic_bezier_timing_wgsl_generation() {
    let wgsl = CubicBezierTiming::wgsl_function();
    assert!(wgsl.contains("fn cubic_bezier_timing"));
    assert!(wgsl.contains("Newton-Raphson"));
    assert!(wgsl.contains("bezier_x"));
    assert!(wgsl.contains("bezier_y"));
}

#[test]
fn test_animation_timeline_creation() {
    let timeline = AnimationTimeline::new(10.0);
    assert_eq!(timeline.current_time, 0.0);
    assert_eq!(timeline.playback_rate, 1.0);
    assert_eq!(timeline.duration, 10.0);
    assert!(!timeline.loop_timeline);
}

#[test]
fn test_animation_timeline_playback_control() {
    let mut timeline = AnimationTimeline::new(5.0);

    timeline.play();
    match timeline.state {
        AnimationPlaybackState::Playing => {}
        _ => panic!("Expected Playing state"),
    }

    timeline.pause();
    match timeline.state {
        AnimationPlaybackState::Paused => {}
        _ => panic!("Expected Paused state"),
    }

    timeline.stop();
    match timeline.state {
        AnimationPlaybackState::Stopped => {}
        _ => panic!("Expected Stopped state"),
    }
    assert_eq!(timeline.current_time, 0.0);
}

#[test]
fn test_animation_timeline_seek() {
    let mut timeline = AnimationTimeline::new(10.0);

    timeline.seek(5.0);
    assert_eq!(timeline.current_time, 5.0);

    // Should clamp to duration
    timeline.seek(15.0);
    assert_eq!(timeline.current_time, 10.0);

    // Should clamp to 0
    timeline.seek(-5.0);
    assert_eq!(timeline.current_time, 0.0);
}

#[test]
fn test_animation_timeline_update_playing() {
    let mut timeline = AnimationTimeline::new(10.0);
    timeline.play();

    timeline.update(1.0);
    assert_eq!(timeline.current_time, 1.0);

    timeline.update(2.0);
    assert_eq!(timeline.current_time, 3.0);
}

#[test]
fn test_animation_timeline_update_paused() {
    let mut timeline = AnimationTimeline::new(10.0);
    timeline.play();
    timeline.update(2.0);
    timeline.pause();

    let time_before = timeline.current_time;
    timeline.update(5.0);
    assert_eq!(timeline.current_time, time_before);
}

#[test]
fn test_animation_timeline_playback_rate() {
    let mut timeline = AnimationTimeline::new(10.0);
    timeline.set_playback_rate(2.0);
    timeline.play();

    timeline.update(1.0);
    assert_eq!(timeline.current_time, 2.0); // 1.0 * 2.0
}

#[test]
fn test_animation_timeline_loop() {
    let mut timeline = AnimationTimeline::new(5.0);
    timeline.enable_loop(true);
    timeline.play();

    timeline.update(6.0);
    assert_eq!(timeline.current_time, 1.0); // Wrapped around
}

#[test]
fn test_animation_timeline_stop_at_end() {
    let mut timeline = AnimationTimeline::new(5.0);
    timeline.play();

    timeline.update(6.0);
    assert_eq!(timeline.current_time, 5.0);
    match timeline.state {
        AnimationPlaybackState::Stopped => {}
        _ => panic!("Expected Stopped state at end"),
    }
}

#[test]
fn test_animation_timeline_normalized_time() {
    let mut timeline = AnimationTimeline::new(10.0);

    timeline.seek(0.0);
    assert_eq!(timeline.normalized_time(), 0.0);

    timeline.seek(5.0);
    assert_eq!(timeline.normalized_time(), 0.5);

    timeline.seek(10.0);
    assert_eq!(timeline.normalized_time(), 1.0);
}

#[test]
fn test_keyframe_animation_composition_with_easing() {
    // Test that KeyframeAnimation can be composed with Easing
    let keyframes = KeyframeAnimation::new()
        .add_keyframe(0.0, 0.0)
        .add_keyframe(1.0, 100.0);

    let easing = Easing::ease_in_out();

    // Both should generate valid WGSL
    let keyframe_wgsl = KeyframeAnimation::wgsl_function();
    let easing_wgsl = Easing::wgsl_function();

    assert!(keyframe_wgsl.contains("fn keyframe_animation"));
    assert!(easing_wgsl.contains("fn easing"));

    // Both should create uniforms
    assert!(keyframes.create_uniforms().is_some());
    assert!(easing.create_uniforms().is_some());
}

#[test]
fn test_keyframe_animation_composition_with_cubic_bezier() {
    // Test composition of KeyframeAnimation with CubicBezierTiming
    let _keyframes = KeyframeAnimation::new()
        .add_keyframe(0.0, 0.0)
        .add_keyframe(2.0, 50.0)
        .add_keyframe(4.0, 100.0);

    let _bezier = CubicBezierTiming::ease_in_out();

    // Both should generate valid WGSL
    let keyframe_wgsl = KeyframeAnimation::wgsl_function();
    let bezier_wgsl = CubicBezierTiming::wgsl_function();

    assert!(keyframe_wgsl.contains("fn keyframe_animation"));
    assert!(bezier_wgsl.contains("fn cubic_bezier_timing"));
}

#[test]
fn test_complex_animation_pipeline() {
    // Create a complex animation with multiple stages
    let timeline = AnimationTimeline::new(10.0);
    let keyframes = KeyframeAnimation::new()
        .add_keyframe(0.0, 0.0)
        .add_keyframe(5.0, 100.0)
        .add_keyframe(10.0, 0.0)
        .with_loop(true);

    let bezier = CubicBezierTiming::ease();
    let easing = Easing::new(EasingFunction::EaseInOutCubic);

    // All components should be valid
    assert!(keyframes.create_uniforms().is_some());
    assert!(bezier.create_uniforms().is_some());
    assert!(easing.create_uniforms().is_some());
    assert_eq!(timeline.duration, 10.0);
}

#[test]
fn test_animation_timeline_reverse_playback() {
    let mut timeline = AnimationTimeline::new(10.0);
    timeline.set_playback_rate(-1.0);
    timeline.seek(5.0);
    timeline.play();

    timeline.update(1.0);
    assert_eq!(timeline.current_time, 4.0); // 5.0 + 1.0 * (-1.0)
}

#[test]
fn test_keyframe_animation_empty() {
    let anim = KeyframeAnimation::new();
    assert_eq!(anim.keyframes.len(), 0);

    let uniforms = anim.create_uniforms().expect("Should create uniforms");
    assert_eq!(uniforms.keyframe_count, 0);
}

#[test]
fn test_keyframe_animation_single_keyframe() {
    let anim = KeyframeAnimation::new().add_keyframe(5.0, 100.0);

    assert_eq!(anim.keyframes.len(), 1);
    let uniforms = anim.create_uniforms().expect("Should create uniforms");
    assert_eq!(uniforms.keyframe_count, 1);
    assert_eq!(uniforms.keyframes[0].value, 100.0);
}

#[test]
fn test_keyframe_animation_defaults() {
    let anim = KeyframeAnimation::default();
    assert_eq!(anim.keyframes.len(), 0);
    assert!(!anim.loop_animation);
    assert!(!anim.reverse_on_loop);
}

#[test]
fn test_shader_uniform_trait_implementations() {
    use gup::shader_function::{
        CubicBezierTimingUniforms, KeyframeAnimationUniforms, ShaderUniform,
    };

    // Test KeyframeAnimationUniforms
    let keyframe_def = KeyframeAnimationUniforms::wgsl_struct_definition();
    assert!(keyframe_def.contains("struct Keyframe"));
    assert!(keyframe_def.contains("struct KeyframeAnimationUniforms"));
    assert_eq!(
        KeyframeAnimationUniforms::wgsl_type_name(),
        "KeyframeAnimationUniforms"
    );

    // Test CubicBezierTimingUniforms
    let bezier_def = CubicBezierTimingUniforms::wgsl_struct_definition();
    assert!(bezier_def.contains("struct CubicBezierTimingUniforms"));
    assert_eq!(
        CubicBezierTimingUniforms::wgsl_type_name(),
        "CubicBezierTimingUniforms"
    );
}

#[test]
fn test_animation_timeline_zero_duration() {
    let timeline = AnimationTimeline::new(0.0);
    assert_eq!(timeline.normalized_time(), 0.0);

    // Seek should still clamp properly
    let mut timeline = AnimationTimeline::new(0.0);
    timeline.seek(5.0);
    assert_eq!(timeline.current_time, 0.0);
}

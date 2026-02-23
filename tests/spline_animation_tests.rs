// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for Spline-Based Animation Curves (GUP-141)
//!
//! Tests Catmull-Rom and B-spline interpolation modes for keyframe animations.

use gup::prelude::*;
use gup::shader_function::{InterpolationMode, KeyframeAnimation};

#[test]
fn test_interpolation_mode_default() {
    let mode = InterpolationMode::default();
    assert_eq!(mode, InterpolationMode::Linear);
}

#[test]
fn test_keyframe_animation_with_interpolation() {
    let anim = KeyframeAnimation::new()
        .add_keyframe(0.0, 0.0)
        .add_keyframe(1.0, 10.0)
        .with_interpolation(InterpolationMode::CatmullRom { tension: 0.0 });

    assert_eq!(
        anim.interpolation_mode,
        InterpolationMode::CatmullRom { tension: 0.0 }
    );
}

#[test]
fn test_keyframe_animation_with_catmull_rom() {
    let anim = KeyframeAnimation::new()
        .add_keyframe(0.0, 0.0)
        .add_keyframe(1.0, 10.0)
        .with_catmull_rom(0.5);

    assert_eq!(
        anim.interpolation_mode,
        InterpolationMode::CatmullRom { tension: 0.5 }
    );
}

#[test]
fn test_keyframe_animation_with_bspline() {
    let anim = KeyframeAnimation::new()
        .add_keyframe(0.0, 0.0)
        .add_keyframe(1.0, 10.0)
        .with_bspline();

    assert_eq!(anim.interpolation_mode, InterpolationMode::BSpline);
}

#[test]
fn test_catmull_rom_tension_clamping() {
    // Test that tension is clamped to [0, 1]
    let anim = KeyframeAnimation::new()
        .add_keyframe(0.0, 0.0)
        .add_keyframe(1.0, 10.0)
        .with_catmull_rom(-0.5);

    assert_eq!(
        anim.interpolation_mode,
        InterpolationMode::CatmullRom { tension: 0.0 }
    );

    let anim2 = KeyframeAnimation::new()
        .add_keyframe(0.0, 0.0)
        .add_keyframe(1.0, 10.0)
        .with_catmull_rom(1.5);

    assert_eq!(
        anim2.interpolation_mode,
        InterpolationMode::CatmullRom { tension: 1.0 }
    );
}

#[test]
fn test_keyframe_animation_uniforms_linear() {
    let anim = KeyframeAnimation::new()
        .add_keyframe(0.0, 0.0)
        .add_keyframe(1.0, 100.0);

    let uniforms = anim.create_uniforms().expect("Should create uniforms");

    assert_eq!(uniforms.interpolation_mode, 0); // Linear
    assert_eq!(uniforms.tension, 0.0);
}

#[test]
fn test_keyframe_animation_uniforms_catmull_rom() {
    let anim = KeyframeAnimation::new()
        .add_keyframe(0.0, 0.0)
        .add_keyframe(1.0, 100.0)
        .with_catmull_rom(0.3);

    let uniforms = anim.create_uniforms().expect("Should create uniforms");

    assert_eq!(uniforms.interpolation_mode, 1); // CatmullRom
    assert_eq!(uniforms.tension, 0.3);
}

#[test]
fn test_keyframe_animation_uniforms_bspline() {
    let anim = KeyframeAnimation::new()
        .add_keyframe(0.0, 0.0)
        .add_keyframe(1.0, 100.0)
        .with_bspline();

    let uniforms = anim.create_uniforms().expect("Should create uniforms");

    assert_eq!(uniforms.interpolation_mode, 2); // BSpline
    assert_eq!(uniforms.tension, 0.0);
}

#[test]
fn test_wgsl_contains_spline_functions() {
    let wgsl = KeyframeAnimation::wgsl_function();

    // Check for helper functions
    assert!(wgsl.contains("fn catmull_rom_interpolate"));
    assert!(wgsl.contains("fn bspline_interpolate"));

    // Check for interpolation mode branching
    assert!(wgsl.contains("params.interpolation_mode == 0u")); // Linear
    assert!(wgsl.contains("params.interpolation_mode == 1u")); // CatmullRom
    assert!(wgsl.contains("params.interpolation_mode == 2u")); // BSpline

    // Check for tension parameter usage
    assert!(wgsl.contains("params.tension"));
}

#[test]
fn test_backward_compatibility_default_linear() {
    // Ensure existing code without interpolation mode specified still works with linear
    let anim = KeyframeAnimation::new()
        .add_keyframe(0.0, 0.0)
        .add_keyframe(1.0, 10.0)
        .add_keyframe(2.0, 5.0);

    assert_eq!(anim.interpolation_mode, InterpolationMode::Linear);

    let uniforms = anim.create_uniforms().unwrap();
    assert_eq!(uniforms.interpolation_mode, 0);
}

#[test]
fn test_multiple_keyframes_with_spline() {
    // Test that spline modes work with multiple keyframes
    let anim = KeyframeAnimation::new()
        .add_keyframe(0.0, 0.0)
        .add_keyframe(1.0, 10.0)
        .add_keyframe(2.0, 5.0)
        .add_keyframe(3.0, 15.0)
        .with_catmull_rom(0.0);

    assert_eq!(anim.keyframes.len(), 4);
    assert_eq!(
        anim.interpolation_mode,
        InterpolationMode::CatmullRom { tension: 0.0 }
    );
}

#[test]
fn test_spline_with_loop_and_reverse() {
    // Test that spline modes can be combined with loop and reverse
    let anim = KeyframeAnimation::new()
        .add_keyframe(0.0, 0.0)
        .add_keyframe(1.0, 10.0)
        .add_keyframe(2.0, 0.0)
        .with_catmull_rom(0.0)
        .with_loop(true)
        .with_reverse(true);

    assert_eq!(
        anim.interpolation_mode,
        InterpolationMode::CatmullRom { tension: 0.0 }
    );
    assert!(anim.loop_animation);
    assert!(anim.reverse_on_loop);
}

#[test]
fn test_wgsl_struct_definition_includes_interpolation_fields() {
    use gup::shader_function::{KeyframeAnimationUniforms, ShaderUniform};

    let struct_def = KeyframeAnimationUniforms::wgsl_struct_definition();

    assert!(struct_def.contains("interpolation_mode: u32"));
    assert!(struct_def.contains("tension: f32"));
    assert!(struct_def.contains("_padding: vec3<f32>"));
}

#[test]
fn test_interpolation_mode_methods_are_fluent() {
    // Test that methods can be chained
    let anim = KeyframeAnimation::new()
        .add_keyframe(0.0, 0.0)
        .add_keyframe(1.0, 10.0)
        .with_catmull_rom(0.5)
        .with_loop(true);

    assert_eq!(
        anim.interpolation_mode,
        InterpolationMode::CatmullRom { tension: 0.5 }
    );
    assert!(anim.loop_animation);
}

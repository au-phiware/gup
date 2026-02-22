// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Comprehensive tests for storage buffer-based keyframe animations.

use gup::{Keyframe, KeyframeAnimationStorage};

#[test]
fn test_basic_storage_creation() {
    let keyframes = vec![Keyframe::new(0.0, 0.0), Keyframe::new(1.0, 1.0)];
    let anim = KeyframeAnimationStorage::new(keyframes);

    assert_eq!(anim.keyframes.len(), 2);
    assert_eq!(anim.count(), 2);
    assert!(!anim.loop_animation);
    assert!(!anim.reverse_on_loop);
}

#[test]
fn test_builder_pattern() {
    let anim = KeyframeAnimationStorage::builder()
        .add_keyframe(0.0, 0.0)
        .add_keyframe(0.5, 0.5)
        .add_keyframe(1.0, 1.0)
        .with_loop(true)
        .with_reverse(true)
        .build();

    assert_eq!(anim.count(), 3);
    assert!(anim.loop_animation);
    assert!(anim.reverse_on_loop);
}

#[test]
fn test_keyframe_sorting() {
    // Add keyframes out of order
    let anim = KeyframeAnimationStorage::builder()
        .add_keyframe(1.0, 1.0)
        .add_keyframe(0.0, 0.0)
        .add_keyframe(0.5, 0.5)
        .build();

    // Verify they're sorted
    assert_eq!(anim.keyframes[0].time, 0.0);
    assert_eq!(anim.keyframes[1].time, 0.5);
    assert_eq!(anim.keyframes[2].time, 1.0);
}

#[test]
fn test_many_keyframes() {
    // Test with 100 keyframes (well beyond uniform buffer limit of 16)
    let mut builder = KeyframeAnimationStorage::builder();
    for i in 0..100 {
        builder = builder.add_keyframe(i as f32, (i as f32) * 2.0);
    }
    let anim = builder.build();

    assert_eq!(anim.count(), 100);
    assert_eq!(anim.keyframes[0].time, 0.0);
    assert_eq!(anim.keyframes[99].time, 99.0);
}

#[test]
fn test_large_keyframe_set() {
    // Test with 1000 keyframes to verify storage buffer scalability
    let mut builder = KeyframeAnimationStorage::builder();
    for i in 0..1000 {
        builder = builder.add_keyframe(i as f32, (i as f32) / 1000.0);
    }
    let anim = builder.build();

    assert_eq!(anim.count(), 1000);
    // Verify first and last are correct
    assert_eq!(anim.keyframes.first().unwrap().time, 0.0);
    assert_eq!(anim.keyframes.last().unwrap().time, 999.0);
}

#[test]
fn test_buffer_data_generation() {
    let anim = KeyframeAnimationStorage::builder()
        .add_keyframe(0.0, 0.0)
        .add_keyframe(1.0, 1.0)
        .build();

    let buffer_data = anim.create_keyframes_buffer_data();

    // Each keyframe is 16 bytes (4 floats)
    assert_eq!(buffer_data.len(), 2 * 16);

    // Verify first keyframe data
    let first_time = f32::from_le_bytes([
        buffer_data[0],
        buffer_data[1],
        buffer_data[2],
        buffer_data[3],
    ]);
    let first_value = f32::from_le_bytes([
        buffer_data[4],
        buffer_data[5],
        buffer_data[6],
        buffer_data[7],
    ]);

    assert_eq!(first_time, 0.0);
    assert_eq!(first_value, 0.0);
}

#[test]
fn test_wgsl_struct_definition() {
    let wgsl = KeyframeAnimationStorage::wgsl_struct_definition();

    // Verify essential struct definitions are present
    assert!(wgsl.contains("struct Keyframe"));
    assert!(wgsl.contains("struct KeyframeAnimationStorageInfo"));
    assert!(wgsl.contains("@group(0) @binding(1)"));
    assert!(wgsl.contains("var<storage, read> keyframe_data"));
    assert!(wgsl.contains("@group(0) @binding(2)"));
    assert!(wgsl.contains("var<uniform> animation_info"));
}

#[test]
fn test_wgsl_function_code() {
    let wgsl = KeyframeAnimationStorage::wgsl_function();

    // Verify binary search implementation
    assert!(wgsl.contains("fn keyframe_animation_storage"));
    assert!(wgsl.contains("var low = 0u"));
    assert!(wgsl.contains("var high = count - 1u"));
    assert!(wgsl.contains("while (low + 1u < high)"));
    assert!(wgsl.contains("let mid = (low + high) / 2u"));

    // Verify interpolation
    assert!(wgsl.contains("mix(k1.value, k2.value, local_t)"));

    // Verify loop support
    assert!(wgsl.contains("animation_info.loop_animation"));
    assert!(wgsl.contains("animation_info.reverse_on_loop"));
}

#[test]
fn test_loop_configuration() {
    let anim =
        KeyframeAnimationStorage::new(vec![Keyframe::new(0.0, 0.0), Keyframe::new(1.0, 1.0)])
            .with_loop(true);

    assert!(anim.loop_animation);
    assert!(!anim.reverse_on_loop);
}

#[test]
fn test_reverse_configuration() {
    let anim =
        KeyframeAnimationStorage::new(vec![Keyframe::new(0.0, 0.0), Keyframe::new(1.0, 1.0)])
            .with_reverse(true);

    assert!(!anim.loop_animation);
    assert!(anim.reverse_on_loop);
}

#[test]
fn test_combined_loop_and_reverse() {
    let anim =
        KeyframeAnimationStorage::new(vec![Keyframe::new(0.0, 0.0), Keyframe::new(1.0, 1.0)])
            .with_loop(true)
            .with_reverse(true);

    assert!(anim.loop_animation);
    assert!(anim.reverse_on_loop);
}

#[test]
#[should_panic(expected = "Must have at least one keyframe")]
fn test_empty_keyframes_panics() {
    KeyframeAnimationStorage::new(vec![]);
}

#[test]
#[should_panic(expected = "Animation must have at least one keyframe")]
fn test_builder_empty_panics() {
    KeyframeAnimationStorage::builder().build();
}

#[test]
fn test_single_keyframe() {
    let anim = KeyframeAnimationStorage::new(vec![Keyframe::new(0.0, 5.0)]);

    assert_eq!(anim.count(), 1);
    assert_eq!(anim.keyframes[0].value, 5.0);
}

#[test]
fn test_non_uniform_timing() {
    // Test keyframes with irregular time spacing
    let anim = KeyframeAnimationStorage::builder()
        .add_keyframe(0.0, 0.0)
        .add_keyframe(0.1, 0.5)
        .add_keyframe(0.9, 0.6)
        .add_keyframe(1.0, 1.0)
        .build();

    assert_eq!(anim.count(), 4);
    // Verify sparse timing is preserved
    assert_eq!(anim.keyframes[1].time, 0.1);
    assert_eq!(anim.keyframes[2].time, 0.9);
}

#[test]
fn test_very_large_keyframe_set() {
    // Test with 10,000 keyframes to verify scalability
    let mut builder = KeyframeAnimationStorage::builder();
    for i in 0..10000 {
        builder = builder.add_keyframe(i as f32, (i % 100) as f32);
    }
    let anim = builder.build();

    assert_eq!(anim.count(), 10000);
    let buffer_data = anim.create_keyframes_buffer_data();
    assert_eq!(buffer_data.len(), 10000 * 16); // 16 bytes per keyframe
}

#[test]
fn test_negative_times() {
    // Test with negative time values
    let anim = KeyframeAnimationStorage::builder()
        .add_keyframe(-1.0, -1.0)
        .add_keyframe(0.0, 0.0)
        .add_keyframe(1.0, 1.0)
        .build();

    assert_eq!(anim.count(), 3);
    assert_eq!(anim.keyframes[0].time, -1.0);
}

#[test]
fn test_duplicate_times() {
    // Test behavior with duplicate time values (should keep both)
    let anim = KeyframeAnimationStorage::builder()
        .add_keyframe(0.0, 0.0)
        .add_keyframe(0.5, 0.5)
        .add_keyframe(0.5, 0.6) // duplicate time
        .add_keyframe(1.0, 1.0)
        .build();

    assert_eq!(anim.count(), 4);
    // Both keyframes with time=0.5 should be present
    assert_eq!(anim.keyframes[1].time, 0.5);
    assert_eq!(anim.keyframes[2].time, 0.5);
}

#[test]
fn test_buffer_data_alignment() {
    let anim = KeyframeAnimationStorage::builder()
        .add_keyframe(0.0, 0.0)
        .build();

    let buffer_data = anim.create_keyframes_buffer_data();

    // Each keyframe should be exactly 16 bytes (4 f32s with padding)
    assert_eq!(buffer_data.len() % 16, 0);
}

// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Integration tests for parallel shader function composition (GUP-136).

use gup::prelude::*;
use gup::vec4;

#[test]
fn test_two_way_parallel_composition() {
    let position_scale = LinearScale::new(0.0, 100.0, 0.0, 800.0);
    let color_map = ColorMap::new(vec4![0.0, 0.0, 1.0, 1.0], vec4![1.0, 0.0, 0.0, 1.0]);

    let parallel = position_scale.parallel(color_map);
    let uniforms = parallel.create_uniforms();
    assert!(uniforms.is_some(), "Should create uniforms");

    let wgsl = parallel.generate_wgsl();
    assert!(
        wgsl.contains("ParallelOutput"),
        "Should have ParallelOutput"
    );
    assert!(wgsl.contains("parallel_composed"), "Should have function");
}

#[test]
fn test_parallel_wgsl_generation() {
    let scale = LinearScale::new(0.0, 100.0, 0.0, 1.0);
    let color = ColorMap::new(vec4![0.0, 0.0, 0.0, 1.0], vec4![1.0, 1.0, 1.0, 1.0]);

    let parallel = scale.parallel(color);
    let wgsl = parallel.generate_wgsl();

    assert!(wgsl.contains(LinearScale::function_name()));
    assert!(wgsl.contains(ColorMap::function_name()));
}

#[test]
fn test_parallel_with_chained_composition() {
    let input_scale = LinearScale::new(0.0, 1000.0, 0.0, 100.0);
    let position = LinearScale::new(0.0, 100.0, 0.0, 800.0);
    let color = ColorMap::new(vec4![0.0, 0.0, 1.0, 1.0], vec4![1.0, 0.0, 0.0, 1.0]);

    let parallel = position.parallel(color);
    let pipeline = input_scale.compose(parallel);

    assert!(pipeline.create_uniforms().is_some());
}

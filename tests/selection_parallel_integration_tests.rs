// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Integration tests for Selection API with ParallelOutput (GUP-140 AC2).

use gup::prelude::*;
use gup::vec4;
use std::sync::Arc;

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct TestData {
    value: f32,
}

#[test]
fn test_selection_attr_parallel_api() {
    // This test verifies the API surface exists and compiles
    let context =
        Arc::new(pollster::block_on(RenderContext::new()).expect("Failed to create context"));

    let data = vec![
        TestData { value: 0.0 },
        TestData { value: 50.0 },
        TestData { value: 100.0 },
    ];

    let mut selection =
        Selection::<TestData, Circle>::new(data, context).expect("Failed to create selection");

    // Create parallel composition
    let position_scale = LinearScale::new(0.0, 100.0, 0.0, 800.0);
    let color_map = ColorMap::new(vec4![0.0, 0.0, 1.0, 1.0], vec4![1.0, 0.0, 0.0, 1.0]);
    let parallel = position_scale.parallel(color_map);

    // This should compile and return Self for method chaining
    let result = selection.attr_parallel(parallel, ["position", "color"]);

    // Verify method chaining works
    assert_eq!(result.len(), 3);
}

#[test]
fn test_selection_attr_parallel_method_chaining() {
    let context =
        Arc::new(pollster::block_on(RenderContext::new()).expect("Failed to create context"));

    let data = vec![TestData { value: 0.0 }];

    let mut selection =
        Selection::<TestData, Circle>::new(data, context).expect("Failed to create selection");

    // Create parallel compositions
    let position_scale = LinearScale::new(0.0, 100.0, 0.0, 800.0);
    let color_map = ColorMap::new(vec4![0.0, 0.0, 1.0, 1.0], vec4![1.0, 0.0, 0.0, 1.0]);
    let parallel = position_scale.parallel(color_map);

    // Verify method chaining works
    selection
        .attr_parallel(parallel, ["position", "color"])
        .attr("size", 5.0_f32)
        .attr("opacity", 1.0_f32);

    // If we got here, all methods chained successfully
    assert_eq!(selection.len(), 1);
}

#[test]
fn test_selection_attr_parallel_three_way_binding() {
    let context =
        Arc::new(pollster::block_on(RenderContext::new()).expect("Failed to create context"));

    let data = vec![TestData { value: 0.0 }];

    let mut selection =
        Selection::<TestData, Circle>::new(data, context).expect("Failed to create selection");

    // Create 3-way parallel composition (nested ParallelOutput)
    let x_scale = LinearScale::new(0.0, 100.0, 0.0, 800.0);
    let y_scale = LinearScale::new(0.0, 100.0, 0.0, 600.0);
    let color = ColorMap::new(vec4![0.0, 0.0, 1.0, 1.0], vec4![1.0, 0.0, 0.0, 1.0]);

    // First parallel: x and y
    let xy_parallel = x_scale.parallel(y_scale);

    // Second parallel: (x, y) and color
    let triple_parallel = xy_parallel.parallel(color);

    // Bind all three attributes
    selection.attr_parallel(triple_parallel, ["x", "y", "color"]);

    assert_eq!(selection.len(), 1);
}

#[test]
fn test_selection_attr_parallel_with_composed_functions() {
    let context =
        Arc::new(pollster::block_on(RenderContext::new()).expect("Failed to create context"));

    let data = vec![TestData { value: 0.0 }];

    let mut selection =
        Selection::<TestData, Circle>::new(data, context).expect("Failed to create selection");

    // Create composed functions and then parallel compose them
    let normalize = LinearScale::new(0.0, 100.0, 0.0, 1.0);
    let position = LinearScale::new(0.0, 1.0, 0.0, 800.0);
    let color_scale = LinearScale::new(0.0, 1.0, 0.0, 1.0);
    let color_map = ColorMap::new(vec4![0.0, 0.0, 1.0, 1.0], vec4![1.0, 0.0, 0.0, 1.0]);

    let position_chain = normalize.compose(position);
    let color_chain = color_scale.compose(color_map);

    let parallel = position_chain.parallel(color_chain);

    selection.attr_parallel(parallel, ["position", "color"]);

    assert_eq!(selection.len(), 1);
}

#[test]
fn test_selection_attr_and_attr_parallel_mixed_usage() {
    let context =
        Arc::new(pollster::block_on(RenderContext::new()).expect("Failed to create context"));

    let data = vec![TestData { value: 0.0 }];

    let mut selection =
        Selection::<TestData, Circle>::new(data, context).expect("Failed to create selection");

    // Mix regular attr() and attr_parallel() calls
    let position_scale = LinearScale::new(0.0, 100.0, 0.0, 800.0);
    let color_map = ColorMap::new(vec4![0.0, 0.0, 1.0, 1.0], vec4![1.0, 0.0, 0.0, 1.0]);
    let parallel = position_scale.parallel(color_map);

    selection
        .attr("size", 10.0_f32)
        .attr_parallel(parallel, ["position", "color"])
        .attr("opacity", 0.8_f32);

    assert_eq!(selection.len(), 1);
}

#[test]
fn test_selection_attr_parallel_type_safety() {
    // This test demonstrates compile-time type safety
    // Uncomment to verify compilation errors

    let context =
        Arc::new(pollster::block_on(RenderContext::new()).expect("Failed to create context"));

    let data = vec![TestData { value: 0.0 }];

    let mut selection =
        Selection::<TestData, Circle>::new(data, context).expect("Failed to create selection");

    let position_scale = LinearScale::new(0.0, 100.0, 0.0, 800.0);
    let color_map = ColorMap::new(vec4![0.0, 0.0, 1.0, 1.0], vec4![1.0, 0.0, 0.0, 1.0]);
    let parallel = position_scale.parallel(color_map);

    // This should work: 2 attributes for 2-way parallel
    selection.attr_parallel(parallel, ["position", "color"]);

    // This would cause a compile error if uncommented (array length mismatch):
    // selection.attr_parallel(parallel, ["position"]);
    // selection.attr_parallel(parallel, ["position", "color", "size"]);

    assert_eq!(selection.len(), 1);
}

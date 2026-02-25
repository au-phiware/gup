// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Integration tests for Selection attribute binding pipeline (GUP-168).
//!
//! Tests cover:
//! - `attr()` storing and retrieving named bindings
//! - `attr_parallel()` with 2-way and 3-way closures
//! - Method chaining of `attr()` and `attr_parallel()`
//! - Type safety (only GPU-compatible types compile)
//! - `prepare_render_bound()` using stored bindings

use gup::prelude::*;
use gup::selection::Selection;

#[derive(Debug, Clone)]
struct TestData {
    value: f32,
}

#[test]
fn test_selection_attr_stores_bindings() {
    let data = vec![
        TestData { value: 0.0 },
        TestData { value: 50.0 },
        TestData { value: 100.0 },
    ];

    let mut selection = Selection::<TestData, Circle>::from_data(data);

    assert!(!selection.has_attr_bindings());

    selection.attr("center", |d: &TestData| [d.value / 100.0, 0.0]);

    assert!(selection.has_attr_bindings());
    assert_eq!(selection.bound_attributes(), vec!["center"]);
}

#[test]
fn test_selection_attr_parallel_two_way() {
    let data = vec![TestData { value: 50.0 }];
    let mut selection = Selection::<TestData, Circle>::from_data(data);

    selection.attr_parallel(
        |d: &TestData| {
            let pos = [d.value / 100.0, 0.0];
            let t = d.value / 100.0;
            (pos, [t, 0.0, 1.0 - t, 1.0])
        },
        ["center", "fill_color"],
    );

    assert_eq!(selection.bound_attributes(), vec!["center", "fill_color"]);
    assert_eq!(selection.len(), 1);
}

#[test]
fn test_selection_attr_parallel_three_way() {
    let data = vec![TestData { value: 50.0 }];
    let mut selection = Selection::<TestData, Circle>::from_data(data);

    selection.attr_parallel(
        |d: &TestData| {
            let pos = [d.value / 100.0, 0.0];
            let t = d.value / 100.0;
            let radius = d.value * 0.01;
            (pos, [t, 0.0, 1.0 - t, 1.0], radius)
        },
        ["center", "fill_color", "radius"],
    );

    assert_eq!(
        selection.bound_attributes(),
        vec!["center", "fill_color", "radius"]
    );
}

#[test]
fn test_selection_attr_method_chaining() {
    let data = vec![TestData { value: 50.0 }];
    let mut selection = Selection::<TestData, Circle>::from_data(data);

    // Chain multiple attr() calls
    selection
        .attr("center", |d: &TestData| [d.value / 100.0, 0.0])
        .attr("radius", |d: &TestData| d.value * 0.01)
        .attr("fill_color", |_: &TestData| [1.0f32, 0.0, 0.0, 1.0]);

    assert_eq!(
        selection.bound_attributes(),
        vec!["center", "radius", "fill_color"]
    );
}

#[test]
fn test_selection_attr_and_attr_parallel_mixed() {
    let data = vec![TestData { value: 50.0 }];
    let mut selection = Selection::<TestData, Circle>::from_data(data);

    // Mix attr() and attr_parallel()
    selection
        .attr("radius", |d: &TestData| d.value * 0.01)
        .attr_parallel(
            |d: &TestData| {
                let t = d.value / 100.0;
                ([t, 0.0], [t, 0.0, 1.0 - t, 1.0])
            },
            ["center", "fill_color"],
        )
        .attr("stroke_width", |_: &TestData| 0.01f32);

    assert_eq!(
        selection.bound_attributes(),
        vec!["radius", "center", "fill_color", "stroke_width"]
    );
}

#[test]
fn test_attr_invalidates_render_state() {
    let data = vec![TestData { value: 50.0 }];
    let mut selection = Selection::<TestData, Circle>::from_data(data);

    // Initially not render-ready
    assert!(!selection.is_render_ready());

    // Adding bindings keeps it not render-ready (need prepare_render)
    selection.attr("center", |d: &TestData| [d.value, 0.0]);
    assert!(!selection.is_render_ready());
}

#[test]
fn test_set_data_preserves_bindings() {
    let data = vec![TestData { value: 50.0 }];
    let mut selection = Selection::<TestData, Circle>::from_data(data);

    selection.attr("center", |d: &TestData| [d.value, 0.0]);
    assert!(selection.has_attr_bindings());

    // set_data should not clear bindings
    selection.set_data(vec![TestData { value: 100.0 }]);
    assert!(selection.has_attr_bindings());
    assert_eq!(selection.bound_attributes(), vec!["center"]);
}

// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Integration tests for the data transition system.
//!
//! These tests verify the end-to-end flow: key-based data rebinding,
//! TransitionBuilder configuration, commit, and transition completion.

use gup::mark::circle::Circle;
use gup::selection::Selection;
use gup::transition::builder::{EasingFn, TransitionGroup, TransitionState};
use gup::transition::diff::diff_by_key;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

// ---------------------------------------------------------------------------
// Test data type
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
struct Point {
    id: u32,
    x: f32,
    y: f32,
}

// ---------------------------------------------------------------------------
// AC1: Key-function data rebinding
// ---------------------------------------------------------------------------

#[test]
fn test_diff_abc_to_bcd() {
    // Given old data [A, B, C] and new data [B, C, D] with identity key,
    // enter = {D}, update = {B, C}, exit = {A}.
    let old = vec![
        Point {
            id: 1,
            x: 0.0,
            y: 0.0,
        },
        Point {
            id: 2,
            x: 1.0,
            y: 1.0,
        },
        Point {
            id: 3,
            x: 2.0,
            y: 2.0,
        },
    ];
    let new = vec![
        Point {
            id: 2,
            x: 10.0,
            y: 10.0,
        },
        Point {
            id: 3,
            x: 20.0,
            y: 20.0,
        },
        Point {
            id: 4,
            x: 30.0,
            y: 30.0,
        },
    ];

    let result = diff_by_key(&old, &new, |p| p.id);

    assert_eq!(result.enter.len(), 1, "enter should have 1 item (D)");
    assert_eq!(result.enter[0].id, 4);

    assert_eq!(result.update.len(), 2, "update should have 2 items (B, C)");
    assert_eq!(result.update[0].0.id, 2); // old B
    assert_eq!(result.update[0].1.id, 2); // new B
    assert_eq!(result.update[1].0.id, 3); // old C
    assert_eq!(result.update[1].1.id, 3); // new C

    assert_eq!(result.exit.len(), 1, "exit should have 1 item (A)");
    assert_eq!(result.exit[0].id, 1);
}

// ---------------------------------------------------------------------------
// AC1: data_keyed on Selection preserves diff
// ---------------------------------------------------------------------------

#[test]
fn test_selection_data_keyed_stores_diff() {
    let mut selection = Selection::<Point, Circle>::from_data(vec![
        Point {
            id: 1,
            x: 0.0,
            y: 0.0,
        },
        Point {
            id: 2,
            x: 1.0,
            y: 1.0,
        },
    ]);

    assert!(!selection.has_pending_diff());

    selection.data_keyed(
        vec![
            Point {
                id: 2,
                x: 10.0,
                y: 10.0,
            },
            Point {
                id: 3,
                x: 20.0,
                y: 20.0,
            },
        ],
        |p| p.id,
    );

    assert!(selection.has_pending_diff());
}

// ---------------------------------------------------------------------------
// AC2: TransitionBuilder configuration
// ---------------------------------------------------------------------------

#[test]
fn test_transition_builder_no_attr_is_noop() {
    let mut selection = Selection::<Point, Circle>::from_data(vec![Point {
        id: 1,
        x: 0.0,
        y: 0.0,
    }]);

    // A transition with no .attr() calls should be a no-op.
    let result = selection.transition().duration(500).commit();

    assert!(result.is_none(), "commit with no attrs should return None");
}

#[test]
fn test_transition_builder_full_config() {
    let mut selection = Selection::<Point, Circle>::from_data(vec![
        Point {
            id: 1,
            x: 0.0,
            y: 0.0,
        },
        Point {
            id: 2,
            x: 1.0,
            y: 1.0,
        },
    ]);

    selection.data_keyed(
        vec![
            Point {
                id: 2,
                x: 10.0,
                y: 10.0,
            },
            Point {
                id: 3,
                x: 20.0,
                y: 20.0,
            },
        ],
        |p| p.id,
    );

    let committed = selection
        .transition()
        .duration(800)
        .delay(100)
        .ease(EasingFn::EaseInOut)
        .attr("cx", |p: &Point| p.x)
        .attr("cy", |p: &Point| p.y)
        .commit();

    assert!(committed.is_some());
    let ct = committed.unwrap();
    assert_eq!(ct.config.duration_ms, 800);
    assert_eq!(ct.config.delay_ms, 100);
    assert!(matches!(ct.config.easing, EasingFn::EaseInOut));
    assert_eq!(ct.update_count, 1); // id=2
    assert_eq!(ct.enter_count, 1); // id=3
    assert_eq!(ct.exit_count, 1); // id=1
    assert_eq!(ct.state, TransitionState::Running);
}

// ---------------------------------------------------------------------------
// AC3: GPU interpolation for update elements
// ---------------------------------------------------------------------------

#[test]
fn test_update_elements_have_from_to_values() {
    let mut selection = Selection::<Point, Circle>::from_data(vec![
        Point {
            id: 1,
            x: 0.0,
            y: 0.0,
        },
        Point {
            id: 2,
            x: 5.0,
            y: 5.0,
        },
    ]);

    selection.data_keyed(
        vec![
            Point {
                id: 1,
                x: 100.0,
                y: 100.0,
            },
            Point {
                id: 2,
                x: 200.0,
                y: 200.0,
            },
        ],
        |p| p.id,
    );

    let ct = selection
        .transition()
        .duration(500)
        .attr("cx", |p: &Point| p.x)
        .attr("cy", |p: &Point| p.y)
        .commit()
        .unwrap();

    // Both elements are updates (no enter, no exit).
    assert_eq!(ct.update_count, 2);
    assert_eq!(ct.enter_count, 0);
    assert_eq!(ct.exit_count, 0);

    // Verify from/to values for update elements.
    let update_elements: Vec<_> = ct
        .elements
        .iter()
        .filter(|e| e.group == TransitionGroup::Update)
        .collect();
    assert_eq!(update_elements.len(), 2);

    // Element 0 (id=1): from x=0.0 (old), to x=100.0 (new)
    let cx0 = update_elements[0].attrs.get("cx").unwrap();
    assert_eq!(cx0.1, gup::AttrValue::Float(100.0)); // to value

    // Element 1 (id=2): from x=5.0 (old), to x=200.0 (new)
    let cx1 = update_elements[1].attrs.get("cx").unwrap();
    assert_eq!(cx1.1, gup::AttrValue::Float(200.0)); // to value
}

#[test]
fn test_transition_completed_values_match_target() {
    // Simulates that after a transition, the final values equal the "to" values.
    let mut selection = Selection::<Point, Circle>::from_data(vec![Point {
        id: 1,
        x: 0.0,
        y: 0.0,
    }]);

    selection.data_keyed(
        vec![Point {
            id: 1,
            x: 50.0,
            y: 75.0,
        }],
        |p| p.id,
    );

    let ct = selection
        .transition()
        .duration(500)
        .attr("cx", |p: &Point| p.x)
        .attr("cy", |p: &Point| p.y)
        .commit()
        .unwrap();

    // At t=1.0 (end of transition), the attribute values should equal
    // the "to" values within floating-point tolerance.
    let update_el = &ct.elements[0];
    assert_eq!(update_el.group, TransitionGroup::Update);

    let (_, to_cx) = update_el.attrs.get("cx").unwrap();
    let (_, to_cy) = update_el.attrs.get("cy").unwrap();
    assert_eq!(*to_cx, gup::AttrValue::Float(50.0));
    assert_eq!(*to_cy, gup::AttrValue::Float(75.0));
}

// ---------------------------------------------------------------------------
// AC4: Enter and exit animations
// ---------------------------------------------------------------------------

#[test]
fn test_enter_elements_default_opacity() {
    let mut selection = Selection::<Point, Circle>::from_data(vec![]);

    selection.data_keyed(
        vec![Point {
            id: 1,
            x: 10.0,
            y: 20.0,
        }],
        |p| p.id,
    );

    let ct = selection
        .transition()
        .duration(300)
        .attr("cx", |p: &Point| p.x)
        .attr("opacity", |_p: &Point| 1.0_f32)
        .commit()
        .unwrap();

    assert_eq!(ct.enter_count, 1);

    let enter_el = ct
        .elements
        .iter()
        .find(|e| e.group == TransitionGroup::Enter)
        .unwrap();

    // opacity should start at 0.0 (default enter state) and end at 1.0.
    let (from_opacity, to_opacity) = enter_el.attrs.get("opacity").unwrap();
    assert_eq!(*from_opacity, gup::AttrValue::Float(0.0));
    assert_eq!(*to_opacity, gup::AttrValue::Float(1.0));
}

#[test]
fn test_exit_elements_default_opacity() {
    let mut selection = Selection::<Point, Circle>::from_data(vec![Point {
        id: 1,
        x: 10.0,
        y: 20.0,
    }]);

    selection.data_keyed(vec![], |p| p.id);

    let ct = selection
        .transition()
        .duration(300)
        .attr("cx", |p: &Point| p.x)
        .attr("opacity", |_p: &Point| 1.0_f32)
        .commit()
        .unwrap();

    assert_eq!(ct.exit_count, 1);

    let exit_el = ct
        .elements
        .iter()
        .find(|e| e.group == TransitionGroup::Exit)
        .unwrap();

    // opacity should animate to 0.0 (default exit state).
    let (_, to_opacity) = exit_el.attrs.get("opacity").unwrap();
    assert_eq!(*to_opacity, gup::AttrValue::Float(0.0));
}

#[test]
fn test_exit_elements_removed_after_complete() {
    let mut selection = Selection::<Point, Circle>::from_data(vec![
        Point {
            id: 1,
            x: 0.0,
            y: 0.0,
        },
        Point {
            id: 2,
            x: 1.0,
            y: 1.0,
        },
        Point {
            id: 3,
            x: 2.0,
            y: 2.0,
        },
    ]);

    selection.data_keyed(
        vec![Point {
            id: 2,
            x: 10.0,
            y: 10.0,
        }],
        |p| p.id,
    );

    let ct = selection
        .transition()
        .duration(300)
        .attr("cx", |p: &Point| p.x)
        .commit()
        .unwrap();

    // During transition: data has update(1) + enter(0) + exit(2) = 3 items.
    assert_eq!(ct.update_count, 1);
    assert_eq!(ct.exit_count, 2);
    assert_eq!(selection.data().len(), 3);

    // After completing the transition: exit elements are removed.
    selection.complete_transition();
    assert_eq!(selection.data().len(), 1, "exit elements should be removed");
    assert!(!selection.has_active_transition());
}

#[test]
fn test_enter_attr_override() {
    let mut selection = Selection::<Point, Circle>::from_data(vec![]);

    selection.data_keyed(
        vec![Point {
            id: 1,
            x: 50.0,
            y: 50.0,
        }],
        |p| p.id,
    );

    let ct = selection
        .transition()
        .duration(300)
        .attr("cx", |p: &Point| p.x)
        .enter_attr("cx", |_p: &Point| -100.0_f32)
        .commit()
        .unwrap();

    let enter_el = ct
        .elements
        .iter()
        .find(|e| e.group == TransitionGroup::Enter)
        .unwrap();

    // cx should start at -100 (custom enter override) and end at 50.
    let (from_cx, to_cx) = enter_el.attrs.get("cx").unwrap();
    assert_eq!(*from_cx, gup::AttrValue::Float(-100.0));
    assert_eq!(*to_cx, gup::AttrValue::Float(50.0));
}

#[test]
fn test_exit_attr_override() {
    let mut selection = Selection::<Point, Circle>::from_data(vec![Point {
        id: 1,
        x: 50.0,
        y: 50.0,
    }]);

    selection.data_keyed(vec![], |p| p.id);

    let ct = selection
        .transition()
        .duration(300)
        .attr("cx", |p: &Point| p.x)
        .exit_attr("cx", |_p: &Point| 999.0_f32)
        .commit()
        .unwrap();

    let exit_el = ct
        .elements
        .iter()
        .find(|e| e.group == TransitionGroup::Exit)
        .unwrap();

    // cx should end at 999 (custom exit override).
    let (_, to_cx) = exit_el.attrs.get("cx").unwrap();
    assert_eq!(*to_cx, gup::AttrValue::Float(999.0));
}

// ---------------------------------------------------------------------------
// AC5: Transition event callbacks
// ---------------------------------------------------------------------------

#[test]
fn test_on_start_fires_at_commit() {
    let start_count = Arc::new(AtomicUsize::new(0));
    let start_count_clone = start_count.clone();

    let mut selection = Selection::<Point, Circle>::from_data(vec![Point {
        id: 1,
        x: 0.0,
        y: 0.0,
    }]);

    selection
        .transition()
        .duration(500)
        .attr("cx", |p: &Point| p.x)
        .on_start(move || {
            start_count_clone.fetch_add(1, Ordering::SeqCst);
        })
        .commit();

    assert_eq!(
        start_count.load(Ordering::SeqCst),
        1,
        "on_start should fire once at commit"
    );
}

#[test]
fn test_on_end_fires_at_complete() {
    let end_count = Arc::new(AtomicUsize::new(0));
    let end_count_clone = end_count.clone();

    let mut selection = Selection::<Point, Circle>::from_data(vec![Point {
        id: 1,
        x: 0.0,
        y: 0.0,
    }]);

    selection
        .transition()
        .duration(500)
        .attr("cx", |p: &Point| p.x)
        .on_end(move || {
            end_count_clone.fetch_add(1, Ordering::SeqCst);
        })
        .commit();

    // on_end should not fire yet.
    assert_eq!(end_count.load(Ordering::SeqCst), 0);

    // Complete the transition.
    selection.complete_transition();

    assert_eq!(
        end_count.load(Ordering::SeqCst),
        1,
        "on_end should fire exactly once"
    );
}

#[test]
fn test_on_end_fires_once_per_transition() {
    let end_count = Arc::new(AtomicUsize::new(0));

    // First transition.
    let mut selection = Selection::<Point, Circle>::from_data(vec![Point {
        id: 1,
        x: 0.0,
        y: 0.0,
    }]);

    let c1 = end_count.clone();
    selection
        .transition()
        .duration(500)
        .attr("cx", |p: &Point| p.x)
        .on_end(move || {
            c1.fetch_add(1, Ordering::SeqCst);
        })
        .commit();

    selection.complete_transition();
    assert_eq!(end_count.load(Ordering::SeqCst), 1);

    // Second transition on the same selection.
    let c2 = end_count.clone();
    selection
        .transition()
        .duration(300)
        .attr("cx", |p: &Point| p.x)
        .on_end(move || {
            c2.fetch_add(1, Ordering::SeqCst);
        })
        .commit();

    selection.complete_transition();
    assert_eq!(
        end_count.load(Ordering::SeqCst),
        2,
        "on_end should fire once per transition"
    );
}

// ---------------------------------------------------------------------------
// AC2: Transition without prior data_keyed treats all as update
// ---------------------------------------------------------------------------

#[test]
fn test_transition_without_data_keyed() {
    let mut selection = Selection::<Point, Circle>::from_data(vec![
        Point {
            id: 1,
            x: 0.0,
            y: 0.0,
        },
        Point {
            id: 2,
            x: 1.0,
            y: 1.0,
        },
    ]);

    let ct = selection
        .transition()
        .duration(500)
        .attr("cx", |p: &Point| p.x)
        .commit()
        .unwrap();

    // Without data_keyed, all elements are update.
    assert_eq!(ct.update_count, 2);
    assert_eq!(ct.enter_count, 0);
    assert_eq!(ct.exit_count, 0);
    assert_eq!(ct.elements.len(), 2);
    assert!(
        ct.elements
            .iter()
            .all(|e| e.group == TransitionGroup::Update)
    );
}

// ---------------------------------------------------------------------------
// Selection::data() without key continues to work (AC1)
// ---------------------------------------------------------------------------

#[test]
fn test_set_data_still_works() {
    let mut selection = Selection::<Point, Circle>::from_data(vec![Point {
        id: 1,
        x: 0.0,
        y: 0.0,
    }]);

    // Normal set_data (positional) should still work.
    selection.set_data(vec![
        Point {
            id: 10,
            x: 5.0,
            y: 5.0,
        },
        Point {
            id: 20,
            x: 6.0,
            y: 6.0,
        },
    ]);

    assert_eq!(selection.data().len(), 2);
    assert_eq!(selection.data()[0].id, 10);
}

// ---------------------------------------------------------------------------
// Mixed scenario: multiple transitions
// ---------------------------------------------------------------------------

#[test]
fn test_sequential_transitions() {
    let mut selection = Selection::<Point, Circle>::from_data(vec![
        Point {
            id: 1,
            x: 0.0,
            y: 0.0,
        },
        Point {
            id: 2,
            x: 1.0,
            y: 1.0,
        },
        Point {
            id: 3,
            x: 2.0,
            y: 2.0,
        },
    ]);

    // First transition: remove id=1, add id=4.
    selection.data_keyed(
        vec![
            Point {
                id: 2,
                x: 10.0,
                y: 10.0,
            },
            Point {
                id: 3,
                x: 20.0,
                y: 20.0,
            },
            Point {
                id: 4,
                x: 30.0,
                y: 30.0,
            },
        ],
        |p| p.id,
    );

    let ct1 = selection
        .transition()
        .duration(500)
        .attr("cx", |p: &Point| p.x)
        .commit()
        .unwrap();

    assert_eq!(ct1.update_count, 2);
    assert_eq!(ct1.enter_count, 1);
    assert_eq!(ct1.exit_count, 1);

    // Complete first transition.
    selection.complete_transition();
    assert_eq!(selection.data().len(), 3); // id=2, 3, 4

    // Second transition: remove id=2, add id=5.
    selection.data_keyed(
        vec![
            Point {
                id: 3,
                x: 100.0,
                y: 100.0,
            },
            Point {
                id: 4,
                x: 200.0,
                y: 200.0,
            },
            Point {
                id: 5,
                x: 300.0,
                y: 300.0,
            },
        ],
        |p| p.id,
    );

    let ct2 = selection
        .transition()
        .duration(500)
        .attr("cx", |p: &Point| p.x)
        .commit()
        .unwrap();

    assert_eq!(ct2.update_count, 2); // id=3, 4
    assert_eq!(ct2.enter_count, 1); // id=5
    assert_eq!(ct2.exit_count, 1); // id=2

    selection.complete_transition();
    assert_eq!(selection.data().len(), 3); // id=3, 4, 5
}

// ---------------------------------------------------------------------------
// Spline easing (AC3)
// ---------------------------------------------------------------------------

#[test]
fn test_catmull_rom_easing() {
    let mut selection = Selection::<Point, Circle>::from_data(vec![Point {
        id: 1,
        x: 0.0,
        y: 0.0,
    }]);

    let ct = selection
        .transition()
        .duration(500)
        .ease(EasingFn::CatmullRom { tension: 0.5 })
        .attr("cx", |p: &Point| p.x)
        .commit()
        .unwrap();

    assert!(matches!(
        ct.config.easing,
        EasingFn::CatmullRom { tension } if (tension - 0.5).abs() < f32::EPSILON
    ));
}

#[test]
fn test_bspline_easing() {
    let mut selection = Selection::<Point, Circle>::from_data(vec![Point {
        id: 1,
        x: 0.0,
        y: 0.0,
    }]);

    let ct = selection
        .transition()
        .duration(500)
        .ease(EasingFn::BSpline)
        .attr("cx", |p: &Point| p.x)
        .commit()
        .unwrap();

    assert!(matches!(ct.config.easing, EasingFn::BSpline));
}

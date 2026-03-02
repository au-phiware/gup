// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Animated scatter plot demonstrating enter/update/exit data transitions.
//!
//! This example shows how to use `Selection::data_keyed()` and
//! `Selection::transition()` to smoothly animate between datasets.
//!
//! # Overview
//!
//! 1. An initial dataset of 20 points is created as `Circle` marks.
//! 2. A new dataset of 20 points is generated: 10 shared keys (update group),
//!    5 new keys (enter group), and 5 removed keys (exit group).
//! 3. The transition is configured with duration, easing, and per-attribute
//!    target values.
//! 4. After committing, the example prints the transition details and
//!    completes the animation.

use gup::mark::circle::Circle;
use gup::selection::Selection;
use gup::transition::builder::{EasingFn, TransitionGroup};

/// A data point with a stable identity and position.
#[derive(Debug, Clone)]
struct ScatterPoint {
    /// Unique identifier for tracking across data updates.
    id: u32,
    /// X position in normalised coordinates [-1, 1].
    x: f32,
    /// Y position in normalised coordinates [-1, 1].
    y: f32,
    /// Radius of the circle mark.
    radius: f32,
}

/// Generate the initial dataset of 20 points (ids 0..20).
fn initial_dataset() -> Vec<ScatterPoint> {
    (0..20)
        .map(|i| {
            let angle = (i as f32) * std::f32::consts::TAU / 20.0;
            ScatterPoint {
                id: i,
                x: angle.cos() * 0.6,
                y: angle.sin() * 0.6,
                radius: 0.03,
            }
        })
        .collect()
}

/// Generate the updated dataset of 20 points.
///
/// - 10 shared keys (ids 5..15): these will animate to new positions (update).
/// - 5 new keys (ids 20..25): these will fade in (enter).
/// - 5 removed keys (ids 0..5): these will fade out (exit).
fn updated_dataset() -> Vec<ScatterPoint> {
    let mut points = Vec::with_capacity(20);

    // Update group: ids 5..15, moved to new positions.
    for i in 5..15 {
        let angle = (i as f32) * std::f32::consts::TAU / 10.0;
        points.push(ScatterPoint {
            id: i,
            x: angle.cos() * 0.3,
            y: angle.sin() * 0.3,
            radius: 0.04,
        });
    }

    // Enter group: ids 20..25, new points.
    for i in 20..25 {
        let angle = ((i - 20) as f32) * std::f32::consts::TAU / 5.0;
        points.push(ScatterPoint {
            id: i,
            x: angle.cos() * 0.8,
            y: angle.sin() * 0.8,
            radius: 0.02,
        });
    }

    // Ids 15..20 also remain but move to new positions.
    for i in 15..20 {
        let angle = ((i - 15) as f32) * std::f32::consts::TAU / 5.0 + 0.5;
        points.push(ScatterPoint {
            id: i,
            x: angle.cos() * 0.5,
            y: angle.sin() * 0.5,
            radius: 0.035,
        });
    }

    points
}

fn main() {
    println!("=== Data Transition Scatter Plot Example ===\n");

    // -----------------------------------------------------------------------
    // Step 1: Create the initial selection with 20 circle marks.
    // -----------------------------------------------------------------------
    let initial_data = initial_dataset();
    println!(
        "Initial dataset: {} points (ids: {:?})",
        initial_data.len(),
        initial_data.iter().map(|p| p.id).collect::<Vec<_>>()
    );

    let mut selection = Selection::<ScatterPoint, Circle>::from_data(initial_data);

    // Bind attributes (position, radius, colour).
    selection
        .attr("center", |p: &ScatterPoint| [p.x, p.y])
        .attr("radius", |p: &ScatterPoint| p.radius)
        .attr("fill_color", |_p: &ScatterPoint| [0.2_f32, 0.6, 0.9, 1.0])
        .attr("opacity", |_p: &ScatterPoint| 1.0_f32);

    println!("Selection created with {} elements.\n", selection.len());

    // -----------------------------------------------------------------------
    // Step 2: Rebind to the new dataset using key-based diffing.
    // -----------------------------------------------------------------------
    let new_data = updated_dataset();
    println!(
        "New dataset: {} points (ids: {:?})",
        new_data.len(),
        new_data.iter().map(|p| p.id).collect::<Vec<_>>()
    );

    // data_keyed() computes the diff by the `id` field:
    // - Enter: ids 20..25 (5 new points)
    // - Update: ids 5..20 (15 shared points that move to new positions)
    // - Exit: ids 0..5 (5 old points that are removed)
    selection.data_keyed(new_data, |p| p.id);

    println!("Diff computed. Starting transition...\n");

    // -----------------------------------------------------------------------
    // Step 3: Configure and commit the transition.
    // -----------------------------------------------------------------------
    let committed = selection
        .transition()
        .duration(800) // 800ms animation
        .delay(0)
        .ease(EasingFn::EaseInOut)
        // Declare target attribute values for the new state.
        .attr("center", |p: &ScatterPoint| [p.x, p.y])
        .attr("radius", |p: &ScatterPoint| p.radius)
        .attr("opacity", |_p: &ScatterPoint| 1.0_f32)
        // Enter elements start off-screen and animate in.
        .enter_attr("center", |_p: &ScatterPoint| [0.0_f32, 0.0])
        // Exit elements fade out by animating opacity to 0.
        // (opacity is automatically set to 0 for exit, but we can also
        //  move them off-screen.)
        .exit_attr("center", |_p: &ScatterPoint| [0.0_f32, 0.0])
        .on_start(|| println!("  [callback] Transition started!"))
        .on_end(|| println!("  [callback] Transition ended!"))
        .commit();

    match committed {
        Some(ct) => {
            println!("Transition committed:");
            println!("  Duration: {} ms", ct.config.duration_ms);
            println!("  Delay: {} ms", ct.config.delay_ms);
            println!("  Easing: {:?}", ct.config.easing);
            println!("  Enter elements: {}", ct.enter_count);
            println!("  Update elements: {}", ct.update_count);
            println!("  Exit elements: {}", ct.exit_count);
            println!("  Total elements during transition: {}", ct.elements.len());
            println!();

            // Print per-element details.
            for (i, el) in ct.elements.iter().enumerate() {
                let group_label = match el.group {
                    // Enter: new elements that appear (fade/grow in).
                    TransitionGroup::Enter => "ENTER",
                    // Update: existing elements animating to new values.
                    TransitionGroup::Update => "UPDATE",
                    // Exit: removed elements that disappear (fade/shrink out).
                    TransitionGroup::Exit => "EXIT",
                };
                if i < 5 || i >= ct.elements.len() - 2 {
                    println!("  Element {}: [{}]", i, group_label);
                    for (name, (from, to)) in &el.attrs {
                        println!("    {}: {:?} → {:?}", name, from, to);
                    }
                } else if i == 5 {
                    println!("  ... ({} more elements) ...", ct.elements.len() - 7);
                }
            }
            println!();

            // ---------------------------------------------------------------
            // Step 4: Complete the transition (simulating animation end).
            // ---------------------------------------------------------------
            println!("Completing transition (simulating animation end)...");
            selection.complete_transition();

            println!(
                "After completion: {} elements (exit elements removed)",
                selection.len()
            );
            assert!(!selection.has_active_transition());
        }
        None => {
            println!("Transition was a no-op (no attr bindings).");
        }
    }

    println!("\n=== Example Complete ===");
}

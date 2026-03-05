// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! # Tutorial 6 — Custom Marks: Arrow Mark
//!
//! Demonstrates the `#[derive(Mark)]` macro and `MarkValidator` from
//! [Tutorial 6: Custom Marks](../../docs/tutorials/06_custom_marks.md).
//!
//! Defines an `Arrow` mark using the derive macro with a triangle primitive,
//! validates it, and creates a selection of three wind-reading data points
//! with arrow marks.
//!
//! Run with: `cargo run --example tutorial06_custom_marks`
//!
//! This example runs headlessly (no window) since the tutorial focuses on
//! mark definition and validation rather than rendering.

use gup::error::GupResult;
use gup::mark::validation::assert_mark_valid;
use gup::render::RenderContext;
use gup::selection::Selection;
use gup::shader_function::{Vec2, Vec4};
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Arrow mark — from the Tutorial 6 "Full Derive Example"
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, gup_macros::Mark)]
#[mark(primitive = "triangle")]
pub struct Arrow {
    #[mark(position)]
    pub position: Vec2,
    #[mark(size)]
    pub size: f32,
    #[mark(color)]
    pub color: Vec4,
}

// ---------------------------------------------------------------------------
// Data — from the Tutorial 6 "Full Derive Example"
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct WindReading {
    x: f32,
    y: f32,
    speed: f32,
}

fn tutorial_data() -> Vec<WindReading> {
    vec![
        WindReading {
            x: 0.2,
            y: 0.3,
            speed: 0.5,
        },
        WindReading {
            x: 0.7,
            y: 0.8,
            speed: 0.9,
        },
        WindReading {
            x: 0.4,
            y: 0.1,
            speed: 0.3,
        },
    ]
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() -> GupResult<()> {
    println!("Tutorial 6 — Custom Marks: Arrow");
    println!("=================================\n");

    // Validate the mark at startup
    assert_mark_valid::<Arrow>()?;
    println!("✓ Arrow mark validated successfully");

    let data = tutorial_data();

    let _context = Arc::new(RenderContext::new().await?);
    println!("✓ GPU context initialised");

    let mut selection = Selection::<WindReading, Arrow>::from_data(data);
    selection
        .attr("position", |d: &WindReading| {
            [d.x * 2.0 - 1.0, d.y * 2.0 - 1.0]
        })
        .attr("size", |d: &WindReading| 0.02 + d.speed * 0.05)
        .attr("color", |d: &WindReading| {
            [d.speed, 0.3, 1.0 - d.speed, 0.8]
        });

    println!(
        "✓ Arrow mark selection ready ({} elements)",
        selection.len()
    );

    // Print details for each element
    for (i, reading) in tutorial_data().iter().enumerate() {
        let sz = 0.02 + reading.speed * 0.05;
        println!(
            "  Arrow {}: pos=({:.1}, {:.1}) size={:.3} speed={:.1}",
            i, reading.x, reading.y, sz, reading.speed
        );
    }

    println!("\nAll done!");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use gup::mark::Mark;

    #[test]
    fn arrow_mark_is_valid() {
        assert_mark_valid::<Arrow>().unwrap();
    }

    #[test]
    fn arrow_has_triangle_geometry() {
        // A triangle primitive should have 3 vertices
        assert_eq!(Arrow::vertex_count(), 3);
    }

    #[test]
    fn tutorial_data_has_three_readings() {
        assert_eq!(tutorial_data().len(), 3);
    }

    #[test]
    fn wind_speeds_are_in_valid_range() {
        for reading in tutorial_data() {
            assert!(reading.speed >= 0.0 && reading.speed <= 1.0);
        }
    }
}

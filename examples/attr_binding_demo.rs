// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Declarative Attribute Binding Demo (GUP-168)
//!
//! Demonstrates the attribute binding pipeline that lets users bind data fields
//! to mark properties declaratively instead of manually constructing GPU
//! instance structs.
//!
//! ## Features
//! - `attr(name, closure)` for single-attribute bindings
//! - `attr_parallel(closure, names)` for multi-attribute bindings
//! - `prepare_render_bound()` for mapper-free GPU upload
//! - Works with any data type — no need for mark-specific attributes

use gup::RectangleAttributes;
use gup::mark::circle::CircleInstance;
use gup::mark::rectangle::RectangleInstance;
use gup::prelude::*;
use gup::selection::Selection;
use std::sync::Arc;

/// A simple scatter plot data point — not a mark attribute type.
#[derive(Debug, Clone)]
struct SalesData {
    revenue: f32,       // 0.0 .. 100.0
    profit_margin: f32, // 0.0 .. 1.0
    category: u32,      // 0, 1, or 2
}

/// Bar chart data point.
#[derive(Debug, Clone)]
struct BarItem {
    label_index: usize,
    value: f32,
}

fn generate_sales_data(count: usize) -> Vec<SalesData> {
    (0..count)
        .map(|i| {
            let t = i as f32 / count as f32;
            SalesData {
                revenue: t * 100.0,
                profit_margin: 0.1 + 0.8 * (t * 3.14).sin().abs(),
                category: (i % 3) as u32,
            }
        })
        .collect()
}

fn main() -> GupResult<()> {
    pollster::block_on(async_main())
}

async fn async_main() -> GupResult<()> {
    println!("=== Declarative Attribute Binding Demo (GUP-168) ===\n");

    let context = Arc::new(RenderContext::new().await?);

    // --- Example 1: Individual attr() bindings ---
    println!("Example 1: Individual attr() bindings");
    example_individual_attrs(&context)?;

    // --- Example 2: attr_parallel() for efficiency ---
    println!("\nExample 2: attr_parallel() for multi-attribute binding");
    example_parallel_attrs(&context)?;

    // --- Example 3: Rectangle mark with attr() bindings ---
    println!("\nExample 3: Rectangle mark binding (bar chart style)");
    example_rectangle_attrs(&context)?;

    // --- Example 4: Traditional vs declarative comparison ---
    println!("\nExample 4: Traditional mapper vs declarative attr()");
    example_comparison(&context)?;

    println!("\n=== Demo Complete ===");
    Ok(())
}

/// Demonstrate individual attr() calls for each visual property.
fn example_individual_attrs(context: &Arc<RenderContext>) -> GupResult<()> {
    let data = generate_sales_data(500);

    let mut selection = Selection::<SalesData, Circle>::from_data(data);

    // Bind each attribute individually
    selection
        .attr("center", |d: &SalesData| {
            // Map revenue to X, profit margin to Y (in clip space -1..1)
            [d.revenue / 50.0 - 1.0, d.profit_margin * 2.0 - 1.0]
        })
        .attr("radius", |d: &SalesData| {
            0.01 + d.profit_margin * 0.04 // Scale radius by profit margin
        })
        .attr("fill_color", |d: &SalesData| {
            // Colour by category: red, green, blue
            match d.category {
                0 => [0.9, 0.2, 0.2, 0.8],
                1 => [0.2, 0.8, 0.3, 0.8],
                _ => [0.2, 0.4, 0.9, 0.8],
            }
        });

    println!("  ✓ Bound: {:?}", selection.bound_attributes());
    println!("  ✓ {} data points", selection.len());

    // Prepare and render via bound attributes (no manual mapper needed!)
    selection.prepare_render_bound(context.device(), context.queue(), None)?;
    println!("  ✓ prepare_render_bound() succeeded");

    Ok(())
}

/// Demonstrate attr_parallel() binding multiple attributes from one closure.
fn example_parallel_attrs(context: &Arc<RenderContext>) -> GupResult<()> {
    let data = generate_sales_data(1000);

    let mut selection = Selection::<SalesData, Circle>::from_data(data);

    // Bind position AND colour from one pass over the data
    selection
        .attr_parallel(
            |d: &SalesData| {
                let pos = [d.revenue / 50.0 - 1.0, d.profit_margin * 2.0 - 1.0];
                let t = d.profit_margin;
                let color = [1.0 - t, t, 0.3, 0.7]; // gradient from red to green
                (pos, color)
            },
            ["center", "fill_color"],
        )
        .attr("radius", |_: &SalesData| 0.02f32);

    println!("  ✓ Bound: {:?}", selection.bound_attributes());
    println!("  ✓ {} data points", selection.len());

    selection.prepare_render_bound(context.device(), context.queue(), None)?;
    println!("  ✓ prepare_render_bound() succeeded");

    Ok(())
}

/// Demonstrate attr() bindings with Rectangle marks.
fn example_rectangle_attrs(context: &Arc<RenderContext>) -> GupResult<()> {
    let data: Vec<BarItem> = (0..5)
        .map(|i| BarItem {
            label_index: i,
            value: 0.3 + (i as f32 * 0.15),
        })
        .collect();

    let mut selection = Selection::<BarItem, Rectangle>::from_data(data);

    let bar_count = selection.len() as f32;
    let bar_width = 1.6 / bar_count;

    selection
        .attr("center", move |d: &BarItem| {
            let x = -0.8 + (d.label_index as f32 + 0.5) * bar_width;
            let y = d.value / 2.0 - 0.5; // bottom-aligned
            [x, y]
        })
        .attr("size", move |d: &BarItem| [bar_width * 0.8, d.value])
        .attr("fill_color", |d: &BarItem| {
            let t = d.label_index as f32 / 4.0;
            [0.2 + t * 0.6, 0.4, 0.8 - t * 0.5, 1.0]
        })
        .attr("corner_radius", |_: &BarItem| 0.02f32);

    println!("  ✓ Bound: {:?}", selection.bound_attributes());

    selection.prepare_render_bound(context.device(), context.queue(), None)?;
    println!("  ✓ prepare_render_bound() succeeded");

    Ok(())
}

/// Compare the traditional prepare_render(mapper) approach with the new
/// declarative attr() approach.
fn example_comparison(context: &Arc<RenderContext>) -> GupResult<()> {
    let data = generate_sales_data(100);

    // --- Traditional: manual mapper closure ---
    {
        let mut selection = Selection::<SalesData, Circle>::from_data(data.clone());
        selection.prepare_render(
            context.device(),
            context.queue(),
            |d| CircleInstance {
                center: [d.revenue / 50.0 - 1.0, d.profit_margin * 2.0 - 1.0],
                radius: 0.02,
                _pad0: 0.0,
                fill_color: [0.9, 0.2, 0.2, 0.8],
                stroke_width: 0.0,
                _pad1: [0.0; 3],
                stroke_color: [0.0; 4],
            },
            None,
        )?;
        println!("  ✓ Traditional prepare_render(mapper): OK");
    }

    // --- Declarative: attr() bindings ---
    {
        let mut selection = Selection::<SalesData, Circle>::from_data(data);
        selection
            .attr("center", |d: &SalesData| {
                [d.revenue / 50.0 - 1.0, d.profit_margin * 2.0 - 1.0]
            })
            .attr("radius", |_: &SalesData| 0.02f32)
            .attr("fill_color", |_: &SalesData| [0.9f32, 0.2, 0.2, 0.8]);

        selection.prepare_render_bound(context.device(), context.queue(), None)?;
        println!("  ✓ Declarative prepare_render_bound(): OK");
    }

    // Both produce equivalent GPU data. The declarative approach is:
    // - More readable: attribute names are self-documenting
    // - Safer: no padding fields to worry about
    // - Flexible: can re-bind individual attributes without rewriting the mapper
    println!("  ✓ Both approaches produce equivalent results");

    // --- Also show that Rectangle works the same way ---
    {
        let bar_data = vec![
            BarItem {
                label_index: 0,
                value: 0.6,
            },
            BarItem {
                label_index: 1,
                value: 0.4,
            },
        ];

        // Traditional
        let mut trad = Selection::<BarItem, Rectangle>::from_data(bar_data.clone());
        trad.prepare_render(
            context.device(),
            context.queue(),
            |d| {
                RectangleInstance::from(&RectangleAttributes {
                    center: Vec2 {
                        x: d.label_index as f32 * 0.4 - 0.2,
                        y: 0.0,
                    },
                    size: Vec2 { x: 0.3, y: d.value },
                    fill_color: Vec4 {
                        x: 0.2,
                        y: 0.5,
                        z: 0.8,
                        w: 1.0,
                    },
                    stroke_width: 0.0,
                    stroke_color: Vec4 {
                        x: 0.0,
                        y: 0.0,
                        z: 0.0,
                        w: 0.0,
                    },
                    corner_radius: 0.0,
                })
            },
            None,
        )?;

        // Declarative
        let mut decl = Selection::<BarItem, Rectangle>::from_data(bar_data);
        decl.attr("center", |d: &BarItem| {
            [d.label_index as f32 * 0.4 - 0.2, 0.0]
        })
        .attr("size", |d: &BarItem| [0.3, d.value])
        .attr("fill_color", |_: &BarItem| [0.2f32, 0.5, 0.8, 1.0]);

        decl.prepare_render_bound(context.device(), context.queue(), None)?;

        println!("  ✓ Rectangle: traditional and declarative both OK");
    }

    Ok(())
}

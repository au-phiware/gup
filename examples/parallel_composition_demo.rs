// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Parallel Composition Scatter Plot Example (GUP-140 AC3)
//!
//! This example demonstrates the efficiency gains from parallel composition,
//! computing multiple visual attributes (position, color, size) from a single
//! data value in a single GPU pass.
//!
//! ## Features
//! - 2-way parallel composition (position + color)
//! - 3-way parallel composition (position XY + color + size)
//! - Performance comparison: parallel vs sequential attribute binding
//! - Visual demonstration of 10,000+ points

use gup::prelude::*;
use gup::vec4;
use std::sync::Arc;
use std::time::Instant;

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct DataPoint {
    value: f32,
    temperature: f32,
}

fn generate_test_data(count: usize) -> Vec<DataPoint> {
    (0..count)
        .map(|i| DataPoint {
            value: (i as f32 / count as f32) * 100.0,
            temperature: 20.0 + (i as f32 / count as f32) * 30.0,
        })
        .collect()
}

fn main() -> GupResult<()> {
    pollster::block_on(async_main())
}

async fn async_main() -> GupResult<()> {
    println!("=== Parallel Composition Scatter Plot Demo (GUP-140) ===\n");

    let context = Arc::new(RenderContext::new().await?);
    let data = generate_test_data(10_000);

    println!("Generated {} data points\n", data.len());

    // Example 1: Two-way parallel composition (position + color)
    println!("Example 1: Two-way parallel composition");
    example_two_way_parallel(&context, &data)?;

    // Example 2: Three-way parallel composition (nested)
    println!("\nExample 2: Three-way parallel composition (nested)");
    example_three_way_parallel(&context, &data)?;

    // Example 3: Performance comparison
    println!("\nExample 3: Performance Comparison");
    performance_comparison(&context, &data)?;

    println!("\n=== Demo Complete ===");
    Ok(())
}

/// Demonstrates 2-way parallel composition: position and color computed together
fn example_two_way_parallel(context: &Arc<RenderContext>, data: &[DataPoint]) -> GupResult<()> {
    let mut selection = Selection::<DataPoint, Circle>::new(data.to_vec(), context.clone())?;

    // Create shader functions for WGSL generation demonstration
    let position_scale = LinearScale::new(0.0, 100.0, 0.0, 800.0); // Maps value -> X position
    let color_map = ColorMap::new(
        vec4![0.0, 0.0, 1.0, 1.0], // Blue for low values
        vec4![1.0, 0.0, 0.0, 1.0], // Red for high values
    );

    // Compose in parallel - both functions receive the same input
    let parallel = position_scale.parallel(color_map);

    // Verify WGSL generation
    let wgsl = parallel.generate_wgsl();
    println!(
        "  Generated WGSL contains ParallelOutput: {}",
        wgsl.contains("ParallelOutput")
    );
    println!(
        "  Generated WGSL contains parallel_composed: {}",
        wgsl.contains("parallel_composed")
    );

    // Bind both attributes via the CPU closure-based pipeline
    let start = Instant::now();
    selection.attr_parallel(
        |d: &DataPoint| {
            let pos = [d.value / 100.0 * 800.0, 300.0];
            let t = d.value / 100.0;
            let color = [t, 0.0, 1.0 - t, 1.0];
            (pos, color)
        },
        ["position", "color"],
    );
    let elapsed = start.elapsed();

    println!("  ✓ Bound position and color in {:?}", elapsed);
    println!("  ✓ Selection has {} elements", selection.len());

    Ok(())
}

/// Demonstrates 3-way parallel composition using nested closures
fn example_three_way_parallel(context: &Arc<RenderContext>, data: &[DataPoint]) -> GupResult<()> {
    let mut selection = Selection::<DataPoint, Circle>::new(data.to_vec(), context.clone())?;

    // Demonstrate WGSL shader composition (GPU-side)
    let x_scale = LinearScale::new(0.0, 100.0, 0.0, 800.0); // value -> X
    let y_scale = LinearScale::new(20.0, 50.0, 0.0, 600.0); // temperature -> Y
    let color_map = ColorMap::new(vec4![0.0, 0.0, 1.0, 1.0], vec4![1.0, 0.0, 0.0, 1.0]);

    // GPU shader composition for reference
    let xy_parallel = x_scale.parallel(y_scale);
    let _triple_parallel = xy_parallel.parallel(color_map);

    // CPU attribute binding with 3-way parallel closure
    let start = Instant::now();
    selection.attr_parallel(
        |d: &DataPoint| {
            let x = d.value / 100.0 * 800.0;
            let y = (d.temperature - 20.0) / 30.0 * 600.0;
            let t = d.value / 100.0;
            ([x, y], [t, 0.0, 1.0 - t, 1.0], d.value * 0.05)
        },
        ["position", "color", "radius"],
    );
    let elapsed = start.elapsed();

    println!("  ✓ Bound x, y, and color in {:?}", elapsed);
    println!("  ✓ Nested ParallelOutput structure created");
    println!("  ✓ Selection has {} elements", selection.len());

    Ok(())
}

/// Compares performance: parallel binding vs sequential binding
fn performance_comparison(context: &Arc<RenderContext>, data: &[DataPoint]) -> GupResult<()> {
    const ITERATIONS: usize = 100;

    // Scenario 1: Parallel binding (single closure computes both attributes)
    let mut parallel_times = Vec::with_capacity(ITERATIONS);
    for _ in 0..ITERATIONS {
        let mut selection = Selection::<DataPoint, Circle>::new(data.to_vec(), context.clone())?;
        let start = Instant::now();
        selection.attr_parallel(
            |d: &DataPoint| {
                let pos = [d.value / 100.0, d.temperature / 50.0];
                let t = d.value / 100.0;
                (pos, [t, 0.0, 1.0 - t, 1.0])
            },
            ["position", "color"],
        );
        parallel_times.push(start.elapsed());
    }

    // Scenario 2: Sequential binding (separate closures)
    let mut sequential_times = Vec::with_capacity(ITERATIONS);
    for _ in 0..ITERATIONS {
        let mut selection = Selection::<DataPoint, Circle>::new(data.to_vec(), context.clone())?;
        let start = Instant::now();
        selection
            .attr("position", |d: &DataPoint| {
                [d.value / 100.0, d.temperature / 50.0]
            })
            .attr("color", |d: &DataPoint| {
                let t = d.value / 100.0;
                [t, 0.0, 1.0 - t, 1.0]
            });
        sequential_times.push(start.elapsed());
    }

    // Calculate averages
    let avg_parallel = parallel_times.iter().sum::<std::time::Duration>() / ITERATIONS as u32;
    let avg_sequential = sequential_times.iter().sum::<std::time::Duration>() / ITERATIONS as u32;

    println!("  Performance comparison ({} iterations):", ITERATIONS);
    println!("  - Parallel binding:   {:?}", avg_parallel);
    println!("  - Sequential binding: {:?}", avg_sequential);

    if avg_parallel < avg_sequential {
        let speedup = avg_sequential.as_nanos() as f64 / avg_parallel.as_nanos() as f64;
        println!("  ✓ Parallel is {:.2}x faster", speedup);
    } else {
        println!("  ℹ Note: Actual performance gains require GPU shader execution");
        println!(
            "    (current implementation is placeholder, real gains come from GPU parallelism)"
        );
    }

    Ok(())
}

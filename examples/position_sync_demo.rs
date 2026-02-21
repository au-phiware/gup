// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Position Synchronization Demo
//!
//! This example demonstrates synchronized positioning between GPU-rendered marks
//! and DOM overlay elements for accessibility.
//!
//! To run this example:
//! ```bash
//! cargo run --example position_sync_demo
//! ```

use gup::accessibility::{GpuPosition, ViewportTransform};
// Commented out: These are not yet exported or broken:
// AccessibilitySystem, AriaNode, AriaRole, NodeId,
// extract_positions_from_selection, PositionExtractor, ScreenPosition
// use gup::mark::Mark;
// use gup::mark::circle::{Circle, CircleAttributes};

fn main() {
    println!("Position Synchronization Demo");
    println!("==============================\n");

    // Demo 1: Coordinate Transformation
    demo_coordinate_transformation();

    // Demo 2 and 3 are currently disabled - they depend on APIs
    // that don't exist yet (create_vertex, extract_positions_from_selection)
    // demo_position_extraction();
    // demo_viewport_updates();

    println!("\nNote: Demo 2 and 3 are disabled pending API implementation.");
}

fn demo_coordinate_transformation() {
    println!("Demo 1: Coordinate Transformation");
    println!("----------------------------------");

    let transform = ViewportTransform::new(800.0, 600.0);

    // Test center point
    let gpu_center = GpuPosition { x: 0.0, y: 0.0 };
    let screen_center = transform.gpu_to_screen(gpu_center);
    println!(
        "GPU center (0, 0) -> Screen ({:.1}, {:.1})",
        screen_center.x, screen_center.y
    );
    assert_eq!(screen_center.x, 400.0);
    assert_eq!(screen_center.y, 300.0);

    // Test top-left corner
    let gpu_top_left = GpuPosition { x: -1.0, y: 1.0 };
    let screen_top_left = transform.gpu_to_screen(gpu_top_left);
    println!(
        "GPU top-left (-1, 1) -> Screen ({:.1}, {:.1})",
        screen_top_left.x, screen_top_left.y
    );

    // Test bottom-right corner
    let gpu_bottom_right = GpuPosition { x: 1.0, y: -1.0 };
    let screen_bottom_right = transform.gpu_to_screen(gpu_bottom_right);
    println!(
        "GPU bottom-right (1, -1) -> Screen ({:.1}, {:.1})",
        screen_bottom_right.x, screen_bottom_right.y
    );

    // Test with zoom
    let transform_zoomed = ViewportTransform::new(800.0, 600.0).with_zoom(2.0);
    let screen_center_zoomed = transform_zoomed.gpu_to_screen(gpu_center);
    println!(
        "GPU center (0, 0) with 2x zoom -> Screen ({:.1}, {:.1})",
        screen_center_zoomed.x, screen_center_zoomed.y
    );

    // Test with pan
    let transform_panned = ViewportTransform::new(800.0, 600.0).with_pan(50.0, 30.0);
    let screen_center_panned = transform_panned.gpu_to_screen(gpu_center);
    println!(
        "GPU center (0, 0) with pan (50, 30) -> Screen ({:.1}, {:.1})",
        screen_center_panned.x, screen_center_panned.y
    );

    // Test roundtrip conversion
    let original = GpuPosition { x: 0.3, y: -0.7 };
    let screen = transform.gpu_to_screen(original);
    let roundtrip = transform.screen_to_gpu(screen);
    println!(
        "\nRoundtrip: GPU ({:.3}, {:.3}) -> Screen ({:.1}, {:.1}) -> GPU ({:.3}, {:.3})",
        original.x, original.y, screen.x, screen.y, roundtrip.x, roundtrip.y
    );
    assert!((roundtrip.x - original.x).abs() < 0.001);
    assert!((roundtrip.y - original.y).abs() < 0.001);

    println!("\n");
}

/*
// These demos are commented out until the required APIs are implemented
// (create_vertex, extract_positions_from_selection)

fn demo_position_extraction() {
    println!("Demo 2: Position Extraction from Marks");
    println!("---------------------------------------");

    // Create some circle attributes
    let attributes = vec![
        CircleAttributes {
            position: [-0.5, 0.5],
            color: [1.0, 0.0, 0.0, 1.0],
            radius: 5.0,
        },
        CircleAttributes {
            position: [0.0, 0.0],
            color: [0.0, 1.0, 0.0, 1.0],
            radius: 7.0,
        },
        CircleAttributes {
            position: [0.5, -0.5],
            color: [0.0, 0.0, 1.0, 1.0],
            radius: 6.0,
        },
    ];

    println!("Created {} circle attributes:", attributes.len());

    // Extract positions
    let positions: Vec<GpuPosition> = attributes
        .iter()
        .map(|attr| {
            let vertex = Circle::create_vertex(attr);
            vertex.extract_position()
        })
        .collect();

    // Transform to screen coordinates
    let transform = ViewportTransform::new(800.0, 600.0);
    println!("\nGPU -> Screen coordinate mapping:");
    for (i, (gpu_pos, attr)) in positions.iter().zip(attributes.iter()).enumerate() {
        let screen_pos = transform.gpu_to_screen(*gpu_pos);
        println!(
            "  Circle {}: GPU ({:>5.2}, {:>5.2}) -> Screen ({:>6.1}, {:>6.1}) | radius: {:.1}px",
            i + 1,
            gpu_pos.x,
            gpu_pos.y,
            screen_pos.x,
            screen_pos.y,
            attr.radius
        );
    }

    println!("\n");
}

fn demo_viewport_updates() {
    println!("Demo 3: Viewport Updates (Pan/Zoom/Resize)");
    println!("-------------------------------------------");

    let data_point = GpuPosition { x: 0.0, y: 0.0 };

    // Original viewport
    let original = ViewportTransform::new(800.0, 600.0);
    let pos_original = original.gpu_to_screen(data_point);
    println!(
        "Original viewport (800x600): Screen position = ({:.1}, {:.1})",
        pos_original.x, pos_original.y
    );

    // After resize
    let after_resize = ViewportTransform::new(1600.0, 1200.0);
    let pos_resize = after_resize.gpu_to_screen(data_point);
    println!(
        "After resize (1600x1200): Screen position = ({:.1}, {:.1})",
        pos_resize.x, pos_resize.y
    );

    // After pan
    let after_pan = ViewportTransform::new(800.0, 600.0).with_pan(100.0, 50.0);
    let pos_pan = after_pan.gpu_to_screen(data_point);
    println!(
        "After pan (100, 50): Screen position = ({:.1}, {:.1})",
        pos_pan.x, pos_pan.y
    );

    // After zoom
    let after_zoom = ViewportTransform::new(800.0, 600.0).with_zoom(2.0);
    let pos_zoom = after_zoom.gpu_to_screen(data_point);
    println!(
        "After zoom (2x): Screen position = ({:.1}, {:.1})",
        pos_zoom.x, pos_zoom.y
    );

    // Combined transformations
    let combined = ViewportTransform::new(800.0, 600.0)
        .with_pan(50.0, 30.0)
        .with_zoom(1.5);
    let pos_combined = combined.gpu_to_screen(data_point);
    println!(
        "Combined (pan+zoom): Screen position = ({:.1}, {:.1})",
        pos_combined.x, pos_combined.y
    );

    println!("\n");
}
*/

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_demo_runs_without_panic() {
        demo_coordinate_transformation();
        // Commented out until APIs are implemented:
        // demo_position_extraction();
        // demo_viewport_updates();
    }
}

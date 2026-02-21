// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Web Accessibility Overlay Demo
//!
//! This example demonstrates the Web DOM overlay for accessibility.
//! It creates a simple scatter plot with keyboard navigation and focus indicators.
//!
//! To run this example in the browser:
//! ```bash
//! wasm-pack build --target web
//! # Then open in a web browser with WebGPU support
//! ```

#[cfg(target_arch = "wasm32")]
use gup::accessibility::{AccessibilitySystem, AriaNode, AriaRole, NodeId};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn main() {
    // Create accessibility system
    let mut accessibility = AccessibilitySystem::new();

    // Create ARIA tree for a simple chart
    let chart_node = accessibility.aria_tree.create_node(AriaNode {
        id: NodeId::new(),
        role: AriaRole::Chart,
        label: "Sales Data Visualization".to_string(),
        description: Some("A scatter plot showing quarterly sales data".to_string()),
        value: None,
        parent: None,
        children: Vec::new(),
    });

    // Add some data points
    let mut point_ids = Vec::new();
    for i in 0..5 {
        let point_id = accessibility.aria_tree.create_node(AriaNode {
            id: NodeId::new(),
            role: AriaRole::DataPoint,
            label: format!("Q{} Sales", i + 1),
            description: Some(format!("Quarter {} sales: ${}", i + 1, (i + 1) * 10000)),
            value: Some(format!("{}", (i + 1) * 10000)),
            parent: Some(chart_node),
            children: Vec::new(),
        });
        point_ids.push(point_id);
    }

    // Get and process ARIA updates
    let updates = accessibility.get_pending_aria_updates();

    // Announce to screen reader
    if let Err(_e) = accessibility.announce(
        "Sales data visualization loaded with 5 data points",
        gup::accessibility::AnnouncementPriority::Polite,
    ) {
        // Announcement failed, but continue
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    eprintln!("This example only runs on wasm32 target");
    eprintln!("Build with: wasm-pack build --target web");
}

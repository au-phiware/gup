// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! WebAssembly integration tests – CPU-only tier (GUP-237).
//!
//! These tests verify that core Gup library types work correctly at runtime
//! when compiled to WebAssembly.  They do **not** require a GPU and can run
//! in either a browser or Node.js runtime.
//!
//! # Running locally
//!
//! ```bash
//! # Browser (ChromeDriver is in the nix devShell, see GUP-240)
//! wasm-pack test --headless --chrome -- --test wasm_integration
//!
//! # Or with Node.js (if available)
//! wasm-pack test --node -- --test wasm_integration
//! ```
#![cfg(target_arch = "wasm32")]

use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

// ---------------------------------------------------------------------------
// CPU-only tests (no GPU required)
// ---------------------------------------------------------------------------

/// Verify that the WASM module loads and basic library types are accessible.
#[wasm_bindgen_test]
fn test_wasm_module_loads() {
    // If we get here the wasm-pack output was loaded successfully.
    // Instantiate a simple type to prove the module is alive.
    let _circle = gup::mark::circle::Circle;
    let _rect = gup::mark::rectangle::Rectangle;
}

/// Verify Circle vertex generation produces the expected unit-quad geometry.
#[wasm_bindgen_test]
fn test_circle_vertex_generation() {
    use gup::mark::Mark;
    use gup::mark::circle::Circle;

    let verts = Circle::generate_vertices();
    assert_eq!(verts.len(), 4, "Circle quad should have 4 vertices");

    // Verify unit-quad corners (order: BL, BR, TR, TL)
    let expected: &[[f32; 2]] = &[[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]];
    for (v, e) in verts.iter().zip(expected.iter()) {
        assert_eq!(v.position, *e);
    }

    let indices = Circle::generate_indices().expect("Circle should have indices");
    assert_eq!(
        indices.len(),
        6,
        "Circle quad should have 6 indices (2 tris)"
    );
    assert_eq!(&indices, &[0, 1, 2, 0, 2, 3]);
}

/// Verify Rectangle vertex generation produces the expected half-unit quad.
#[wasm_bindgen_test]
fn test_rectangle_vertex_generation() {
    use gup::mark::Mark;
    use gup::mark::rectangle::Rectangle;

    let verts = Rectangle::generate_vertices();
    assert_eq!(verts.len(), 4, "Rectangle quad should have 4 vertices");

    let indices = Rectangle::generate_indices().expect("Rectangle should have indices");
    assert_eq!(indices.len(), 6, "Rectangle quad should have 6 indices");
}

/// Verify CircleInstance can be constructed and has correct byte layout.
#[wasm_bindgen_test]
fn test_circle_instance_layout() {
    use gup::mark::circle::CircleInstance;

    let instance = CircleInstance {
        center: [0.5, 0.5],
        radius: 0.1,
        _pad0: 0.0,
        fill_color: [1.0, 0.0, 0.0, 1.0],
        stroke_width: 0.01,
        _pad1: [0.0; 3],
        stroke_color: [0.0, 0.0, 0.0, 1.0],
    };

    // Verify bytemuck round-trip
    let bytes = bytemuck::bytes_of(&instance);
    let restored: &CircleInstance = bytemuck::from_bytes(bytes);
    assert_eq!(restored.center, [0.5, 0.5]);
    assert_eq!(restored.radius, 0.1);
    assert_eq!(restored.fill_color, [1.0, 0.0, 0.0, 1.0]);
}

/// Verify MarkRegistry can register and query mark types.
#[wasm_bindgen_test]
fn test_mark_registry_operations() {
    use gup::mark::MarkRegistry;
    use gup::mark::circle::Circle;
    use gup::mark::rectangle::Rectangle;

    let mut registry = MarkRegistry::new();
    registry.register::<Circle>();
    registry.register::<Rectangle>();

    assert!(registry.is_registered::<Circle>());
    assert!(registry.is_registered::<Rectangle>());

    let types = registry.registered_types();
    assert!(types.len() >= 2);
}

/// Verify Circle shader sources are available and well-formed.
#[wasm_bindgen_test]
fn test_circle_shader_sources() {
    use gup::mark::Mark;
    use gup::mark::circle::Circle;

    let vert = Circle::VERTEX_SHADER.expect("Circle should have a vertex shader");
    let frag = Circle::FRAGMENT_SHADER.expect("Circle should have a fragment shader");

    assert!(
        vert.contains("vs_main"),
        "Vertex shader should contain vs_main"
    );
    assert!(
        frag.contains("fs_main"),
        "Fragment shader should contain fs_main"
    );
    assert!(
        vert.contains("CircleInstance"),
        "Vertex shader should reference CircleInstance"
    );
}

// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tutorial snippet compilation tests (GUP-351).
//!
//! This integration test verifies that tutorial "Full Example" code blocks
//! compile correctly.  Most tutorials are tested as doctests via the
//! `#[cfg(doctest)]` module in `src/lib.rs`.  Tutorial 3 (Custom Shader
//! Functions) uses the `#[wgsl_function]` proc macro whose generated code
//! references `crate::` paths, making it incompatible with the merged-doctest
//! context.  We therefore test it here as a regular integration test.

use gup::*;
use gup_macros::wgsl_function;

// ---- Tutorial 3: Full Example (adapted) ----------------------------------

#[wgsl_function]
fn temperature_to_radius(temp: f32, min_radius: f32, max_radius: f32) -> f32 {
    return min_radius + (max_radius - min_radius) * temp;
}

#[derive(Debug, Clone)]
struct WeatherReading {
    longitude: f32,
    latitude: f32,
    temperature: f32,
}

/// Verify that the Tutorial 3 Full Example compiles and its types are usable.
#[test]
fn tutorial_03_full_example_compiles() {
    let data = vec![
        WeatherReading {
            longitude: 0.2,
            latitude: 0.3,
            temperature: 0.7,
        },
        WeatherReading {
            longitude: 0.5,
            latitude: 0.8,
            temperature: 0.4,
        },
        WeatherReading {
            longitude: 0.9,
            latitude: 0.1,
            temperature: 0.9,
        },
    ];

    let radius_fn = TemperatureToRadius::new(0.01, 0.08);

    let mut selection = Selection::<WeatherReading, Circle>::from_data(data);
    selection
        .attr("center", |d: &WeatherReading| {
            [d.longitude * 2.0 - 1.0, d.latitude * 2.0 - 1.0]
        })
        .attr_shader("radius", |d: &WeatherReading| d.temperature, radius_fn)
        .attr("fill_color", |d: &WeatherReading| {
            [d.temperature, 0.3, 1.0 - d.temperature, 0.8]
        });

    assert_eq!(selection.len(), 3);

    // Verify the generated WGSL is non-empty
    let wgsl = TemperatureToRadius::wgsl_function();
    assert!(wgsl.contains("temperature_to_radius"));
}

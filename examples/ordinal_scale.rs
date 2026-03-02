// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Demonstrates constructing an `OrdinalScale` from string categories and
//! composing `BandScale` / `PointScale` shader functions.
//!
//! This example builds CPU-side category mappings, computes GPU uniforms, and
//! prints the resulting positions to show that the ordinal scale types work
//! end-to-end without requiring a GPU window.

use gup::shader_function::{
    BandScale, ComposableShaderFunction, OrdinalScale, OrdinalScaleUniforms, PointScale,
    ShaderUniform,
};

fn main() {
    println!("=== OrdinalScale GPU Shader Function Demo ===\n");

    // ---------------------------------------------------------------
    // 1. Build an OrdinalScale from category labels
    // ---------------------------------------------------------------
    let categories = ["Mon", "Tue", "Wed", "Thu", "Fri"];
    let scale = OrdinalScale::from_categories(&categories);

    println!("Categories: {:?}", scale.labels());
    println!("Count:      {}", scale.category_count());
    for label in &categories {
        println!(
            "  category_index({label:?}) = {:?}",
            scale.category_index(label)
        );
    }
    println!(
        "  category_index(\"Sat\") = {:?}  (missing → None)",
        scale.category_index("Sat")
    );

    // ---------------------------------------------------------------
    // 2. BandScale — positions + bandwidth
    // ---------------------------------------------------------------
    let range = (0.0_f32, 500.0_f32);
    let padding = 0.2_f32;
    let band = scale.band_scale(range, padding);

    println!("\n--- BandScale (range={range:?}, padding={padding}) ---");
    println!("  step_size = {:.4}", band.step_size());
    println!("  bandwidth = {:.4}", band.bandwidth());
    for (i, label) in categories.iter().enumerate() {
        let pos = band.apply(i as u32);
        println!("  {label}: centre = {pos:.4}");
    }

    // Show that the WGSL function is well-formed
    let wgsl = BandScale::wgsl_function();
    println!("\n  WGSL function ({} bytes):", wgsl.len());
    for line in wgsl.trim().lines() {
        println!("    {line}");
    }

    // Show the GPU uniforms
    let uniforms: OrdinalScaleUniforms = band.uniforms();
    println!("\n  Uniforms: {uniforms:?}");
    println!(
        "  WGSL struct:\n    {}",
        OrdinalScaleUniforms::wgsl_struct_definition()
    );

    // ---------------------------------------------------------------
    // 3. PointScale — evenly distributed points
    // ---------------------------------------------------------------
    let point = scale.point_scale(range, 0.5);

    println!("\n--- PointScale (range={range:?}, padding=0.5) ---");
    println!("  step_size       = {:.4}", point.step_size());
    for (i, label) in categories.iter().enumerate() {
        let pos = point.apply(i as u32);
        println!("  {label}: position = {pos:.4}");
    }

    let wgsl = PointScale::wgsl_function();
    println!("\n  WGSL function ({} bytes):", wgsl.len());
    for line in wgsl.trim().lines() {
        println!("    {line}");
    }

    // ---------------------------------------------------------------
    // 4. Composition readiness — demonstrate that the types satisfy
    //    ComposableShaderFunction and can produce uniforms
    // ---------------------------------------------------------------
    println!("\n--- Composition compatibility ---");
    println!(
        "  BandScale  function_name = {:?}",
        BandScale::function_name()
    );
    println!(
        "  PointScale function_name = {:?}",
        PointScale::function_name()
    );

    let band_u = band.create_uniforms();
    let point_u = point.create_uniforms();
    println!("  BandScale  create_uniforms = {:?}", band_u.is_some());
    println!("  PointScale create_uniforms = {:?}", point_u.is_some());

    // Round-trip the uniforms through bytemuck to prove GPU upload safety
    let bytes: &[u8] = bytemuck::bytes_of(&uniforms);
    let _round_trip: &OrdinalScaleUniforms = bytemuck::from_bytes(bytes);
    println!(
        "  bytemuck round-trip OK ({} bytes)",
        std::mem::size_of::<OrdinalScaleUniforms>()
    );

    println!("\nDone.");
}

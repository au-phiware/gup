// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Geographic Projection Shader Demo (GUP-273)
//!
//! Demonstrates composable geographic projection shader functions by plotting
//! world-city coordinates as `Circle` marks using the Mercator projection
//! composed with a screen-space position transform.
//!
//! ## Features
//! - Mercator projection of (longitude, latitude) to screen coordinates
//! - Composition with `PositionTransform` via `compose()`
//! - Equirectangular, Stereographic, and Orthographic variants
//! - Boundary clipping with `CLIP_SENTINEL`

use gup::prelude::*;
use gup::shader_function::ComposableShaderFunction;
use gup::shader_function::geo::*;
use std::sync::Arc;

/// A world city with a name and geographic coordinates.
#[derive(Debug, Clone)]
struct City {
    name: &'static str,
    lon: f32,
    lat: f32,
}

/// A small collection of world cities for demonstration.
fn world_cities() -> Vec<City> {
    vec![
        City {
            name: "London",
            lon: -0.1278,
            lat: 51.5074,
        },
        City {
            name: "Paris",
            lon: 2.3522,
            lat: 48.8566,
        },
        City {
            name: "New York",
            lon: -74.0060,
            lat: 40.7128,
        },
        City {
            name: "Tokyo",
            lon: 139.6917,
            lat: 35.6895,
        },
        City {
            name: "Sydney",
            lon: 151.2093,
            lat: -33.8688,
        },
        City {
            name: "Cairo",
            lon: 31.2357,
            lat: 30.0444,
        },
        City {
            name: "Rio de Janeiro",
            lon: -43.1729,
            lat: -22.9068,
        },
        City {
            name: "Mumbai",
            lon: 72.8777,
            lat: 19.0760,
        },
        City {
            name: "Beijing",
            lon: 116.4074,
            lat: 39.9042,
        },
        City {
            name: "Los Angeles",
            lon: -118.2437,
            lat: 34.0522,
        },
        City {
            name: "Moscow",
            lon: 37.6173,
            lat: 55.7558,
        },
        City {
            name: "São Paulo",
            lon: -46.6333,
            lat: -23.5505,
        },
        City {
            name: "Nairobi",
            lon: 36.8219,
            lat: -1.2921,
        },
        City {
            name: "Buenos Aires",
            lon: -58.3816,
            lat: -34.6037,
        },
        City {
            name: "Singapore",
            lon: 103.8198,
            lat: 1.3521,
        },
    ]
}

fn main() -> GupResult<()> {
    pollster::block_on(async_main())
}

async fn async_main() -> GupResult<()> {
    println!("=== Geographic Projection Shader Demo (GUP-273) ===\n");

    let context = Arc::new(RenderContext::new().await?);
    let cities = world_cities();

    println!("Plotting {} world cities\n", cities.len());

    // ── Mercator Projection ──
    demo_mercator(&context, &cities)?;

    // ── Equirectangular Projection ──
    demo_equirectangular(&context, &cities)?;

    // ── Orthographic Projection ──
    demo_orthographic(&context, &cities)?;

    // ── Stereographic Projection ──
    demo_stereographic(&context, &cities)?;

    // ── Composition Pipeline ──
    demo_composition(&cities)?;

    println!("\n=== Demo Complete ===");
    Ok(())
}

/// Demonstrates Mercator projection with screen-space composition.
fn demo_mercator(context: &Arc<RenderContext>, cities: &[City]) -> GupResult<()> {
    println!("── Mercator Projection ──");

    let _selection = Selection::<City, Circle>::new(cities.to_vec(), context.clone())?;

    // Set up the projection pipeline
    let proj = MercatorProjection::new()
        .scale(100.0)
        .translate(400.0, 300.0);

    // Generate WGSL and show it
    let wgsl = <MercatorProjection as ComposableShaderFunction>::wgsl_function();
    println!("  WGSL function length: {} chars", wgsl.len());
    assert!(wgsl.contains("mercator_projection"));

    // Verify uniforms
    let uniforms = proj.create_uniforms().unwrap();
    println!(
        "  Uniforms: center_lon={}, scale={}, clip_lat={}°",
        uniforms.center_lon, uniforms.scale, uniforms.clip_lat
    );

    // Compose with screen transform
    let screen = PositionTransform::new(Vec2::new(1.0, -1.0), Vec2::new(0.0, 600.0));
    let composed = proj.compose(screen);
    let composed_wgsl = composed.generate_wgsl();
    println!("  Composed WGSL functions: {} chars", composed_wgsl.len());
    assert!(composed_wgsl.contains("mercator_projection"));
    assert!(composed_wgsl.contains("position_transform"));
    assert!(composed_wgsl.contains("composed_chain"));

    // Bind data to circle marks
    println!("  Selection created with {} marks", cities.len());

    // Project a few cities through the CPU reference
    for city in cities.iter().take(5) {
        let (x, y) = mercator_cpu(city.lon, city.lat, &uniforms);
        if x == CLIP_SENTINEL {
            println!(
                "  {} ({:.1}°, {:.1}°) → CLIPPED",
                city.name, city.lon, city.lat
            );
        } else {
            // Apply screen transform
            let sx = x * 1.0 + 0.0;
            let sy = y * -1.0 + 600.0;
            println!(
                "  {} ({:.1}°, {:.1}°) → ({:.1}, {:.1})",
                city.name, city.lon, city.lat, sx, sy
            );
        }
    }

    println!();
    Ok(())
}

/// Demonstrates equirectangular projection.
fn demo_equirectangular(context: &Arc<RenderContext>, cities: &[City]) -> GupResult<()> {
    println!("── Equirectangular Projection ──");

    let _selection = Selection::<City, Circle>::new(cities.to_vec(), context.clone())?;

    let proj = EquirectangularProjection::new()
        .scale(200.0)
        .translate(400.0, 300.0);
    let uniforms = proj.create_uniforms().unwrap();

    let wgsl = <EquirectangularProjection as ComposableShaderFunction>::wgsl_function();
    println!("  WGSL function length: {} chars", wgsl.len());

    for city in cities.iter().take(3) {
        let (x, y) = equirectangular_cpu(city.lon, city.lat, &uniforms);
        println!(
            "  {} ({:.1}°, {:.1}°) → ({:.1}, {:.1})",
            city.name, city.lon, city.lat, x, y
        );
    }

    println!();
    Ok(())
}

/// Demonstrates orthographic projection with hemisphere clipping.
fn demo_orthographic(context: &Arc<RenderContext>, cities: &[City]) -> GupResult<()> {
    println!("── Orthographic Projection ──");

    let _selection = Selection::<City, Circle>::new(cities.to_vec(), context.clone())?;

    // Centre on London
    let proj = OrthographicProjection::new()
        .center(-0.1278, 51.5074)
        .scale(300.0)
        .translate(400.0, 300.0);
    let uniforms = proj.create_uniforms().unwrap();

    let wgsl = <OrthographicProjection as ComposableShaderFunction>::wgsl_function();
    println!("  WGSL function length: {} chars", wgsl.len());
    println!(
        "  Centre: London ({:.1}°, {:.1}°)",
        uniforms.center_lon, uniforms.center_lat
    );

    let mut visible = 0;
    let mut clipped = 0;
    for city in cities {
        let (x, y) = orthographic_cpu(city.lon, city.lat, &uniforms);
        if x == CLIP_SENTINEL {
            clipped += 1;
            println!("  {} → CLIPPED (far hemisphere)", city.name);
        } else {
            visible += 1;
            println!("  {} → ({:.1}, {:.1})", city.name, x, y);
        }
    }
    println!("  Visible: {visible}, Clipped: {clipped}");

    println!();
    Ok(())
}

/// Demonstrates stereographic projection.
fn demo_stereographic(context: &Arc<RenderContext>, cities: &[City]) -> GupResult<()> {
    println!("── Stereographic Projection ──");

    let _selection = Selection::<City, Circle>::new(cities.to_vec(), context.clone())?;

    let proj = StereographicProjection::new()
        .scale(200.0)
        .translate(400.0, 300.0);
    let uniforms = proj.create_uniforms().unwrap();

    let wgsl = <StereographicProjection as ComposableShaderFunction>::wgsl_function();
    println!("  WGSL function length: {} chars", wgsl.len());

    for city in cities.iter().take(5) {
        let (x, y) = stereographic_cpu(city.lon, city.lat, &uniforms);
        println!(
            "  {} ({:.1}°, {:.1}°) → ({:.1}, {:.1})",
            city.name, city.lon, city.lat, x, y
        );
    }

    println!();
    Ok(())
}

/// Demonstrates full composition pipeline: projection → screen transform.
fn demo_composition(cities: &[City]) -> GupResult<()> {
    println!("── Full Composition Pipeline ──");

    let proj = MercatorProjection::new().scale(1.0);
    let screen = PositionTransform::new(Vec2::new(100.0, -100.0), Vec2::new(400.0, 300.0));
    let composed = proj.compose(screen);

    let wgsl = composed.generate_wgsl();
    println!("  Generated composed WGSL ({} chars):", wgsl.len());
    println!("  ─────────────────────────────────────────");
    for line in wgsl.lines().take(20) {
        println!("  │ {line}");
    }
    if wgsl.lines().count() > 20 {
        println!("  │ ... ({} more lines)", wgsl.lines().count() - 20);
    }
    println!("  ─────────────────────────────────────────");

    // Verify the uniform chain
    let uniforms = composed.create_uniforms().unwrap();
    println!(
        "  Chain uniforms: first.scale={}, second.scale=({}, {})",
        uniforms.first.scale, uniforms.second.scale[0], uniforms.second.scale[1]
    );

    // Manually trace a point through the pipeline
    let city = &cities[0]; // London
    let merc_u = MercatorProjection::new()
        .scale(1.0)
        .create_uniforms()
        .unwrap();
    let (mx, my) = mercator_cpu(city.lon, city.lat, &merc_u);
    let px = mx * 100.0 + 400.0;
    let py = my * (-100.0) + 300.0;
    println!(
        "\n  Pipeline trace for {} ({:.4}°, {:.4}°):",
        city.name, city.lon, city.lat
    );
    println!("    → Mercator: ({:.4}, {:.4})", mx, my);
    println!("    → Screen:   ({:.1}, {:.1})", px, py);

    println!();
    Ok(())
}

// ─── CPU Reference Implementations ───────────────────────────────────────────
// These mirror the WGSL implementations for validation.

const DEG_TO_RAD: f32 = std::f32::consts::PI / 180.0;

fn mercator_cpu(lon: f32, lat: f32, u: &MercatorUniforms) -> (f32, f32) {
    if lat.abs() > u.clip_lat {
        return (CLIP_SENTINEL, CLIP_SENTINEL);
    }
    let max_safe: f32 = 85.051129_f32 * DEG_TO_RAD;
    let lon_rad = (lon - u.center_lon) * DEG_TO_RAD;
    let lat_rad = (lat * DEG_TO_RAD).clamp(-max_safe, max_safe);
    let x = lon_rad;
    let y = (std::f32::consts::FRAC_PI_4 + lat_rad * 0.5).tan().ln();
    (x * u.scale + u.translate_x, y * u.scale + u.translate_y)
}

fn equirectangular_cpu(lon: f32, lat: f32, u: &EquirectangularUniforms) -> (f32, f32) {
    let lon_rad = (lon - u.center_lon) * DEG_TO_RAD;
    let lat_rad = (lat - u.center_lat) * DEG_TO_RAD;
    let center_lat_rad = u.center_lat * DEG_TO_RAD;
    let x = lon_rad * center_lat_rad.cos();
    let y = lat_rad;
    (x * u.scale + u.translate_x, y * u.scale + u.translate_y)
}

fn orthographic_cpu(lon: f32, lat: f32, u: &OrthographicUniforms) -> (f32, f32) {
    let lon_r = lon * DEG_TO_RAD;
    let lat_r = lat * DEG_TO_RAD;
    let lon0 = u.center_lon * DEG_TO_RAD;
    let lat0 = u.center_lat * DEG_TO_RAD;
    let d_lon = lon_r - lon0;
    let cos_c = lat0.sin() * lat_r.sin() + lat0.cos() * lat_r.cos() * d_lon.cos();
    if cos_c < 0.0 {
        return (CLIP_SENTINEL, CLIP_SENTINEL);
    }
    let x = lat_r.cos() * d_lon.sin();
    let y = lat0.cos() * lat_r.sin() - lat0.sin() * lat_r.cos() * d_lon.cos();
    (x * u.scale + u.translate_x, y * u.scale + u.translate_y)
}

fn stereographic_cpu(lon: f32, lat: f32, u: &StereographicUniforms) -> (f32, f32) {
    let lon_r = lon * DEG_TO_RAD;
    let lat_r = lat * DEG_TO_RAD;
    let lon0 = u.center_lon * DEG_TO_RAD;
    let lat0 = u.center_lat * DEG_TO_RAD;
    let d_lon = lon_r - lon0;
    let k_denom = 1.0 + lat0.sin() * lat_r.sin() + lat0.cos() * lat_r.cos() * d_lon.cos();
    let k = 2.0 / k_denom;
    let x = k * lat_r.cos() * d_lon.sin();
    let y = k * (lat0.cos() * lat_r.sin() - lat0.sin() * lat_r.cos() * d_lon.cos());
    (x * u.scale + u.translate_x, y * u.scale + u.translate_y)
}

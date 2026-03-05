// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later
//
// Vertex shader for GPU-side choropleth recolouring.
//
// Each vertex carries a (longitude, latitude) position and a region_index
// (u32). The shader applies a geographic projection to produce clip-space
// coordinates and passes the region_index through to the fragment stage,
// where the actual colour is looked up from a storage buffer.

const DEG_TO_RAD: f32 = 0.017453292519943295;

// Projection type constants.
const PROJ_MERCATOR: u32 = 0u;
const PROJ_EQUIRECTANGULAR: u32 = 1u;

struct ChoroplethUniforms {
    // Projection parameters
    center_lon: f32,
    center_lat: f32,
    scale: f32,
    projection_type: u32,
    // Viewport offset (translation after projection)
    translate_x: f32,
    translate_y: f32,
    // Stroke colour (used when edge_flag > 0.5)
    stroke_color: vec4<f32>,
    stroke_width: f32,
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
}

@group(0) @binding(0)
var<uniform> u: ChoroplethUniforms;

// Per-region RGBA colour buffer.
@group(0) @binding(1)
var<storage, read> region_colors: array<vec4<f32>>;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) fill_color: vec4<f32>,
    @location(1) stroke_color: vec4<f32>,
    @location(2) edge_flag: f32,
    @location(3) stroke_width: f32,
}

// Mercator projection: (lon, lat) → (x, y) in normalised units.
fn mercator(lon: f32, lat: f32, center_lon: f32) -> vec2<f32> {
    let max_lat: f32 = 85.051129 * DEG_TO_RAD;
    let lon_rad = (lon - center_lon) * DEG_TO_RAD;
    let lat_rad = clamp(lat * DEG_TO_RAD, -max_lat, max_lat);
    let x = lon_rad;
    let quarter_pi: f32 = 0.7853981633974483;
    let y = log(tan(quarter_pi + lat_rad * 0.5));
    return vec2<f32>(x, y);
}

// Equirectangular (Plate Carrée) projection.
fn equirectangular(lon: f32, lat: f32, center_lon: f32, center_lat: f32) -> vec2<f32> {
    let lon_rad = (lon - center_lon) * DEG_TO_RAD;
    let lat_rad = (lat - center_lat) * DEG_TO_RAD;
    let center_lat_rad = center_lat * DEG_TO_RAD;
    let x = lon_rad * cos(center_lat_rad);
    let y = lat_rad;
    return vec2<f32>(x, y);
}

@vertex
fn vs_main(
    @location(0) lonlat: vec2<f32>,
    @location(1) region_index: u32,
) -> VertexOutput {
    var projected: vec2<f32>;
    if (u.projection_type == PROJ_MERCATOR) {
        projected = mercator(lonlat.x, lonlat.y, u.center_lon);
    } else {
        projected = equirectangular(lonlat.x, lonlat.y, u.center_lon, u.center_lat);
    }

    // Apply scale and translation to map to clip space.
    let x = projected.x * u.scale + u.translate_x;
    let y = projected.y * u.scale + u.translate_y;

    // Look up the region colour from the storage buffer.
    let fill = region_colors[region_index];

    var output: VertexOutput;
    output.position = vec4<f32>(x, y, 0.0, 1.0);
    output.fill_color = fill;
    output.stroke_color = u.stroke_color;
    output.edge_flag = 0.0; // Fill vertices always have edge_flag = 0.
    output.stroke_width = u.stroke_width;
    return output;
}

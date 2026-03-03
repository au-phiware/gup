// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later
//
// Marching-squares contour extraction compute shader.
//
// Reads a 2D density grid (texture) and emits line-segment endpoints
// for a single iso-level.  Each workgroup thread processes one grid
// cell.

struct MarchingSquaresParams {
    // Grid dimensions.
    grid_cols: u32,
    grid_rows: u32,
    // The iso-level threshold.
    threshold: f32,
    // Grid spatial extent (used to map indices to world coords).
    x_min: f32,
    x_max: f32,
    y_min: f32,
    y_max: f32,
    // Atomic counter for output vertex pairs.
    _pad0: f32,
};

struct Vertex2D {
    x: f32,
    y: f32,
};

@group(0) @binding(0) var<uniform> params: MarchingSquaresParams;
@group(0) @binding(1) var density_tex: texture_2d<f32>;
@group(0) @binding(2) var<storage, read_write> vertices: array<Vertex2D>;
@group(0) @binding(3) var<storage, read_write> vertex_count: atomic<u32>;

// Read density at grid position (col, row).
fn read_density(col: u32, row: u32) -> f32 {
    return textureLoad(density_tex, vec2<i32>(i32(col), i32(row)), 0).r;
}

// Map grid position to world coordinates.
fn grid_to_world(col: f32, row: f32) -> vec2<f32> {
    let x = params.x_min + col * (params.x_max - params.x_min) / f32(params.grid_cols);
    let y = params.y_min + row * (params.y_max - params.y_min) / f32(params.grid_rows);
    return vec2<f32>(x, y);
}

// Linear interpolation factor.
fn lerp_t(threshold: f32, a: f32, b: f32) -> f32 {
    let denom = b - a;
    if abs(denom) < 1e-12 {
        return 0.5;
    }
    return clamp((threshold - a) / denom, 0.0, 1.0);
}

// Interpolated position along an edge of a cell at (col, row).
// Edge numbering: 0=top, 1=right, 2=bottom, 3=left.
fn edge_point(col: u32, row: u32, edge: u32, v00: f32, v10: f32, v01: f32, v11: f32) -> vec2<f32> {
    let c = f32(col);
    let r = f32(row);
    switch edge {
        case 0u: { // top (v00 → v10)
            let t = lerp_t(params.threshold, v00, v10);
            return grid_to_world(c + t, r);
        }
        case 1u: { // right (v10 → v11)
            let t = lerp_t(params.threshold, v10, v11);
            return grid_to_world(c + 1.0, r + t);
        }
        case 2u: { // bottom (v01 → v11)
            let t = lerp_t(params.threshold, v01, v11);
            return grid_to_world(c + t, r + 1.0);
        }
        case 3u: { // left (v00 → v01)
            let t = lerp_t(params.threshold, v00, v01);
            return grid_to_world(c, r + t);
        }
        default: {
            return vec2<f32>(0.0, 0.0);
        }
    }
}

// Emit a line segment (two vertices).
fn emit_segment(p0: vec2<f32>, p1: vec2<f32>) {
    let idx = atomicAdd(&vertex_count, 2u);
    vertices[idx] = Vertex2D(p0.x, p0.y);
    vertices[idx + 1u] = Vertex2D(p1.x, p1.y);
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let col = gid.x;
    let row = gid.y;

    // Each thread processes one cell; a cell needs the value at
    // (col, row), (col+1, row), (col, row+1), (col+1, row+1).
    if col >= params.grid_cols - 1u || row >= params.grid_rows - 1u {
        return;
    }

    let v00 = read_density(col, row);
    let v10 = read_density(col + 1u, row);
    let v01 = read_density(col, row + 1u);
    let v11 = read_density(col + 1u, row + 1u);

    var idx: u32 = 0u;
    if v00 >= params.threshold { idx |= 1u; }
    if v10 >= params.threshold { idx |= 2u; }
    if v11 >= params.threshold { idx |= 4u; }
    if v01 >= params.threshold { idx |= 8u; }

    // Look-up table encoded as pairs of (edge_from, edge_to).
    // Each case uses up to 4 values (2 segments max).
    // -1 (0xFF) marks unused slots.
    // Case 5 and 10 are saddle points — disambiguated by centre value.
    switch idx {
        case 0u, 15u: {
            // No segments.
            return;
        }
        case 1u: {
            emit_segment(
                edge_point(col, row, 3u, v00, v10, v01, v11),
                edge_point(col, row, 0u, v00, v10, v01, v11));
        }
        case 2u: {
            emit_segment(
                edge_point(col, row, 0u, v00, v10, v01, v11),
                edge_point(col, row, 1u, v00, v10, v01, v11));
        }
        case 3u: {
            emit_segment(
                edge_point(col, row, 3u, v00, v10, v01, v11),
                edge_point(col, row, 1u, v00, v10, v01, v11));
        }
        case 4u: {
            emit_segment(
                edge_point(col, row, 1u, v00, v10, v01, v11),
                edge_point(col, row, 2u, v00, v10, v01, v11));
        }
        case 5u: {
            // Saddle: disambiguate using centre value.
            let centre = (v00 + v10 + v01 + v11) * 0.25;
            if centre >= params.threshold {
                emit_segment(
                    edge_point(col, row, 3u, v00, v10, v01, v11),
                    edge_point(col, row, 2u, v00, v10, v01, v11));
                emit_segment(
                    edge_point(col, row, 0u, v00, v10, v01, v11),
                    edge_point(col, row, 1u, v00, v10, v01, v11));
            } else {
                emit_segment(
                    edge_point(col, row, 3u, v00, v10, v01, v11),
                    edge_point(col, row, 0u, v00, v10, v01, v11));
                emit_segment(
                    edge_point(col, row, 1u, v00, v10, v01, v11),
                    edge_point(col, row, 2u, v00, v10, v01, v11));
            }
        }
        case 6u: {
            emit_segment(
                edge_point(col, row, 0u, v00, v10, v01, v11),
                edge_point(col, row, 2u, v00, v10, v01, v11));
        }
        case 7u: {
            emit_segment(
                edge_point(col, row, 3u, v00, v10, v01, v11),
                edge_point(col, row, 2u, v00, v10, v01, v11));
        }
        case 8u: {
            emit_segment(
                edge_point(col, row, 2u, v00, v10, v01, v11),
                edge_point(col, row, 3u, v00, v10, v01, v11));
        }
        case 9u: {
            emit_segment(
                edge_point(col, row, 2u, v00, v10, v01, v11),
                edge_point(col, row, 0u, v00, v10, v01, v11));
        }
        case 10u: {
            // Saddle: disambiguate using centre value.
            let centre = (v00 + v10 + v01 + v11) * 0.25;
            if centre >= params.threshold {
                emit_segment(
                    edge_point(col, row, 0u, v00, v10, v01, v11),
                    edge_point(col, row, 3u, v00, v10, v01, v11));
                emit_segment(
                    edge_point(col, row, 2u, v00, v10, v01, v11),
                    edge_point(col, row, 1u, v00, v10, v01, v11));
            } else {
                emit_segment(
                    edge_point(col, row, 0u, v00, v10, v01, v11),
                    edge_point(col, row, 1u, v00, v10, v01, v11));
                emit_segment(
                    edge_point(col, row, 2u, v00, v10, v01, v11),
                    edge_point(col, row, 3u, v00, v10, v01, v11));
            }
        }
        case 11u: {
            emit_segment(
                edge_point(col, row, 2u, v00, v10, v01, v11),
                edge_point(col, row, 1u, v00, v10, v01, v11));
        }
        case 12u: {
            emit_segment(
                edge_point(col, row, 1u, v00, v10, v01, v11),
                edge_point(col, row, 3u, v00, v10, v01, v11));
        }
        case 13u: {
            emit_segment(
                edge_point(col, row, 1u, v00, v10, v01, v11),
                edge_point(col, row, 0u, v00, v10, v01, v11));
        }
        case 14u: {
            emit_segment(
                edge_point(col, row, 0u, v00, v10, v01, v11),
                edge_point(col, row, 3u, v00, v10, v01, v11));
        }
        default: {
            return;
        }
    }
}

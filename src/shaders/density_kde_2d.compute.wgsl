// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later
//
// 2D Kernel Density Estimation compute shader.
//
// Each invocation writes a single cell of the output density grid.
// The kernel is a product Gaussian: K(u,v) = K(u) * K(v).

struct KDEParams {
    // Grid dimensions (cols, rows).
    grid_cols: u32,
    grid_rows: u32,
    // Number of input data points.
    n_points: u32,
    // Bandwidth per axis.
    bandwidth_x: f32,
    bandwidth_y: f32,
    // Grid spatial extent.
    x_min: f32,
    x_max: f32,
    y_min: f32,
    y_max: f32,
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
};

@group(0) @binding(0) var<uniform> params: KDEParams;
@group(0) @binding(1) var<storage, read> points: array<vec2<f32>>;
@group(0) @binding(2) var output_grid: texture_storage_2d<r32float, write>;

// Gaussian kernel: (1 / sqrt(2 pi)) * exp(-0.5 * u * u)
fn gaussian(u: f32) -> f32 {
    return 0.3989422804014327 * exp(-0.5 * u * u);
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let col = gid.x;
    let row = gid.y;

    if col >= params.grid_cols || row >= params.grid_rows {
        return;
    }

    // Map grid cell to world coordinates.
    let x = params.x_min + (f32(col) + 0.5) * (params.x_max - params.x_min) / f32(params.grid_cols);
    let y = params.y_min + (f32(row) + 0.5) * (params.y_max - params.y_min) / f32(params.grid_rows);

    var density: f32 = 0.0;
    let n = params.n_points;
    let bw_x = params.bandwidth_x;
    let bw_y = params.bandwidth_y;

    for (var i: u32 = 0u; i < n; i = i + 1u) {
        let pt = points[i];
        let ux = (x - pt.x) / bw_x;
        let uy = (y - pt.y) / bw_y;
        density += gaussian(ux) * gaussian(uy);
    }

    // Normalise by sample count and bandwidth product.
    density = density / (f32(n) * bw_x * bw_y);

    textureStore(output_grid, vec2<i32>(i32(col), i32(row)), vec4<f32>(density, 0.0, 0.0, 0.0));
}

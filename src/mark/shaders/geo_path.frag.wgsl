// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later
//
// Fragment shader for GeoPathMark rendering.
//
// Outputs either the fill colour or the stroke colour depending on the
// edge_flag interpolated from the vertex stage.

struct FragmentInput {
    @location(0) fill_color: vec4<f32>,
    @location(1) stroke_color: vec4<f32>,
    @location(2) edge_flag: f32,
    @location(3) stroke_width: f32,
}

@fragment
fn fs_main(input: FragmentInput) -> @location(0) vec4<f32> {
    // edge_flag == 1.0 for stroke line-list vertices, 0.0 for fill triangles.
    if (input.edge_flag > 0.5) {
        return input.stroke_color;
    }
    return input.fill_color;
}

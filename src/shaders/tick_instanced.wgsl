// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later
//
// Instanced tick mark shader.
//
// A single base line segment (two vertices at t = 0.0 and t = 1.0) is
// instanced for every tick mark.  Per-instance data carries the NDC
// position on the axis, a direction-and-length vector, and the tick
// colour.  The vertex shader interpolates between the on-axis point and
// the tick end so that one draw call renders all ticks of a given type.

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(
    // Base geometry: single float 0.0 (on-axis) or 1.0 (tick end)
    @location(0) t: f32,
    // Per-instance data
    @location(1) inst_position: vec2<f32>,
    @location(2) inst_tick_vector: vec2<f32>,
    @location(3) inst_color: vec4<f32>,
) -> VertexOutput {
    var out: VertexOutput;
    let pos = inst_position + inst_tick_vector * t;
    out.clip_position = vec4<f32>(pos, 0.0, 1.0);
    out.color = inst_color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}

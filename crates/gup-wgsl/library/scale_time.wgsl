// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// Time position scale: linear over seconds, read from a hi/lo column
// (`F32x2Relative`, GUP-418).
//
// The column stores v = x - origin as a pair of f32s whose sum is v to about
// 48 bits; the CPU computes base = origin - d0 in f64 and splits it the same
// way. For a point on screen, x - d0 is small next to v and -base, so
// v.x + base.x is exact (the two nearly cancel) and the low words add the
// rest: px = r0 + (x - d0) * k without ever forming x or v in one f32.
#define_import_path gup::scale::time

struct Params {
    // Pixels per second: (r1 - r0) / (d1 - d0).
    k: f32,
    // Range start r0 (px).
    range_start: f32,
    // Uniform `Params` structs span a multiple of 16 bytes (GUP-401).
    pad_a: u32,
    pad_b: u32,
}

fn map_rel(v: vec2<f32>, base: vec2<f32>, p: Params) -> f32 {
    let hi = v.x + base.x;
    let lo = v.y + base.y;
    return p.range_start + (hi + lo) * p.k;
}

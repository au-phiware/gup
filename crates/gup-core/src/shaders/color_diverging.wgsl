// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// Diverging colour scale (RFC-001 S5b): two ramps meeting at a midpoint. A
// value below the midpoint maps linearly into [0, 0.5], one above it into
// [0.5, 1], each side with its own slope, then samples a palette LUT (an
// Rgba8Unorm texture of sRGB-encoded colours, linearly filtered between
// texel centres) whose middle texel is the midpoint's colour.
#define_import_path gup::color::diverging

struct Params {
    // The midpoint's domain value.
    mid: f32,
    // 0.5 / (mid - d0): the slope below the midpoint.
    inv_lo: f32,
    // 0.5 / (d1 - mid): the slope above it.
    inv_hi: f32,
    // Non-zero to run the palette backwards.
    reverse: u32,
}

fn map(v: f32, p: Params, lut: texture_2d<f32>, smp: sampler) -> vec4<f32> {
    let d = v - p.mid;
    var t = clamp(0.5 + d * select(p.inv_hi, p.inv_lo, d < 0.0), 0.0, 1.0);
    if (p.reverse != 0u) {
        t = 1.0 - t;
    }
    let n = f32(textureDimensions(lut).x);
    let u = (t * (n - 1.0) + 0.5) / n;
    return textureSampleLevel(lut, smp, vec2<f32>(u, 0.5), 0.0);
}

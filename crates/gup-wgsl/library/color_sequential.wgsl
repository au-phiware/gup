// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// Sequential colour scale: normalise into [0, 1], then sample a palette LUT
// (an Rgba8Unorm texture of sRGB-encoded colours, linearly filtered between
// texel centres).
#define_import_path gup::color::sequential

struct Params {
    // Domain start d0.
    lo: f32,
    // 1 / (d1 - d0).
    inv_span: f32,
    // Non-zero to run the palette backwards.
    reverse: u32,
    // Uniform `Params` structs span a multiple of 16 bytes (GUP-401).
    padding: u32,
}

fn map(v: f32, p: Params, lut: texture_2d<f32>, smp: sampler) -> vec4<f32> {
    var t = clamp((v - p.lo) * p.inv_span, 0.0, 1.0);
    if (p.reverse != 0u) {
        t = 1.0 - t;
    }
    let n = f32(textureDimensions(lut).x);
    let u = (t * (n - 1.0) + 0.5) / n;
    return textureSampleLevel(lut, smp, vec2<f32>(u, 0.5), 0.0);
}

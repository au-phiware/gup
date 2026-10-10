// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// Categorical colour (RFC-001 S5b): a dictionary code picks a texel of a
// palette LUT (an Rgba8Unorm texture of sRGB-encoded colours, read by texel,
// never filtered), repeating past the LUT's end. Codes outside the domain
// (the keys the scale was fitted to), including the reserved null code,
// pick the null colour.
#define_import_path gup::color::categorical

struct Params {
    // The colour of a missing key, sRGB-encoded with straight alpha.
    null_color: vec4<f32>,
    // Keys in the domain: codes 0..count.
    count: u32,
    // Uniform `Params` structs span a multiple of 16 bytes (GUP-401).
    pad_a: u32,
    pad_b: u32,
    pad_c: u32,
}

// The sampler is part of the LUT resource's calling convention; a palette
// is read by texel, so it is unused.
fn map(code: u32, p: Params, lut: texture_2d<f32>, smp: sampler) -> vec4<f32> {
    let n = textureDimensions(lut).x;
    let color = textureLoad(lut, vec2<u32>(code % n, 0u), 0);
    // NULL_CODE (0xffffffff) is never below `count`.
    return select(color, p.null_color, code >= p.count);
}

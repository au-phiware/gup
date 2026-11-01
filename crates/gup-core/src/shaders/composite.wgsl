// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// A translucent line layer's offscreen target, composited into the scene
// (GUP-439). The layer was drawn into `coverage`, a texture the target's
// size, with `Max` blending: each texel holds the premultiplied colour of
// the most covering segment or piece, so joins, overlaps and
// self-crossings count once. One triangle covers the target (the draw's
// scissor limits it to the layer's clip), and each pixel loads its texel
// for premultiplied-over blending. A texel of 0 leaves the pixel as it is.
//
// The texel is found from the triangle's interpolated position in target
// pixels, not from the fragment's framebuffer position, so a host's
// viewport offset does not move it.

@group(0) @binding(0) var coverage: texture_2d<f32>;

struct CompositeOut {
    @builtin(position) clip: vec4<f32>,
    // Physical pixels from the target's top-left corner.
    @location(0) at: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> CompositeOut {
    // (0, 0), (2, 0), (0, 2) in units of the target's size.
    let uv = vec2<f32>(f32((vertex_index << 1u) & 2u), f32(vertex_index & 2u));
    var out: CompositeOut;
    out.clip = vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, 0.0, 1.0);
    out.at = uv * vec2<f32>(textureDimensions(coverage));
    return out;
}

@fragment
fn fs_main(v: CompositeOut) -> @location(0) vec4<f32> {
    let size = vec2<i32>(textureDimensions(coverage));
    let texel = clamp(vec2<i32>(floor(v.at)), vec2<i32>(0), size - vec2<i32>(1));
    let c = textureLoad(coverage, texel, 0);
    if (c.a <= 0.0) {
        discard;
    }
    return c;
}

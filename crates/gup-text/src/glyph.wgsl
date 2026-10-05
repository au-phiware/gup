// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// Bitmap glyph quads from the glyph atlas. Quads sit on whole physical
// pixels and atlas rects are the same size, so the atlas is sampled 1:1.

@group(0) @binding(0) var atlas: texture_2d<f32>;
@group(0) @binding(1) var atlas_smp: sampler;

struct GlyphIn {
    // x0, y0, x1, y1 in clip space.
    @location(0) rect: vec4<f32>,
    // u0, v0, u1, v1 in atlas texels.
    @location(1) uv: vec4<f32>,
    // sRGB-encoded, straight alpha.
    @location(2) color: vec4<f32>,
}

struct GlyphOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32, g: GlyphIn) -> GlyphOut {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(1.0, 1.0),
    );
    let c = corners[vertex_index];
    var out: GlyphOut;
    out.clip = vec4<f32>(mix(g.rect.xy, g.rect.zw, c), 0.0, 1.0);
    // Texels to UV here, so glyph positions survive the atlas growing.
    out.uv = mix(g.uv.xy, g.uv.zw, c) / vec2<f32>(textureDimensions(atlas));
    out.color = g.color;
    return out;
}

@fragment
fn fs_main(v: GlyphOut) -> @location(0) vec4<f32> {
    let coverage = textureSampleLevel(atlas, atlas_smp, v.uv, 0.0).r;
    let alpha = v.color.a * coverage;
    // Premultiplied output, blended in sRGB space.
    return vec4<f32>(v.color.rgb * alpha, alpha);
}

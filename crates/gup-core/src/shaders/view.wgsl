// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// The view transform: the only place where clip space (NDC) exists. Marks
// and guides work in logical pixels (Px, origin top-left, y down) and call
// `px_to_clip` last.
#define_import_path gup::view

struct View {
    // Target size in logical pixels.
    size: vec2<f32>,
    // Device pixels per logical pixel.
    dpr: f32,
    padding: f32,
}

fn px_to_clip(p: vec2<f32>, view: View) -> vec4<f32> {
    let ndc = vec2<f32>(p.x / view.size.x * 2.0 - 1.0, 1.0 - p.y / view.size.y * 2.0);
    return vec4<f32>(ndc, 0.0, 1.0);
}

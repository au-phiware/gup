// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// `gpu_pick` needs `hit_distance` (and `hit_row`) in the mark's module.
use gup_core::prelude::*;

#[derive(Debug, Mark)]
#[mark(
    wgsl = r#"
    #import gup::view::{View, px_to_clip}

    struct DotIn {
        x: f32,
        y: f32,
    }

    struct Varyings {
        @builtin(position) clip: vec4<f32>,
        @location(0) @interpolate(flat) row: u32,
    }

    fn vertex(m: DotIn, vertex_index: u32, row: u32, view: View) -> Varyings {
        var out: Varyings;
        out.clip = px_to_clip(vec2<f32>(m.x, m.y), view);
        out.row = row;
        return out;
    }

    fn shade(v: Varyings) -> vec4<f32> {
        return vec4<f32>(1.0);
    }

    fn hit_row(v: Varyings) -> u32 {
        return v.row;
    }
"#,
    geometry = Quad,
    gpu_pick
)]
struct Dot {
    #[channel(role = X)]
    x: Px,
    #[channel(role = Y)]
    y: Px,
}

fn main() {}

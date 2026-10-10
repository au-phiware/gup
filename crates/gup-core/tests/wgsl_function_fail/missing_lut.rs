// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// The WGSL takes a palette LUT, but the impl does not supply one.
use gup_core::prelude::*;

#[derive(Clone)]
struct Heat;

#[wgsl_function(wgsl = r#"
    struct Params { lo: f32, span: f32 }

    fn heat(v: f32, p: Params, lut: texture_2d<f32>, smp: sampler) -> vec4<f32> {
        let t = clamp((v - p.lo) / p.span, 0.0, 1.0);
        return textureSampleLevel(lut, smp, vec2<f32>(t, 0.5), 0.0);
    }
"#)]
impl ShaderFn for Heat {
    fn params(&self) -> HeatParams {
        HeatParams { lo: 0.0, span: 1.0 }
    }
}

fn main() {}

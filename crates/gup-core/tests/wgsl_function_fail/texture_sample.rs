// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// `textureSample` needs derivatives, which the vertex stage lacks.
use gup_core::prelude::*;
use gup_core::Resource;

#[derive(Clone)]
struct Heat;

#[wgsl_function(wgsl = r#"
    struct Params { lo: f32, span: f32 }

    fn heat(v: f32, p: Params, lut: texture_2d<f32>, smp: sampler) -> vec4<f32> {
        let t = clamp((v - p.lo) / p.span, 0.0, 1.0);
        return textureSample(lut, smp, vec2<f32>(t, 0.5));
    }
"#)]
impl ShaderFn for Heat {
    fn params(&self) -> HeatParams {
        HeatParams { lo: 0.0, span: 1.0 }
    }

    fn resources(&self) -> Vec<Resource> {
        vec![Resource::Lut(vec![[0, 0, 0, 255], [255, 64, 0, 255]])]
    }
}

fn main() {}

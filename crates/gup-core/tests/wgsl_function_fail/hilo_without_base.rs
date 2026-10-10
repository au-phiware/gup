// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// A hi/lo input without its per-chunk base.
use gup_core::prelude::*;

#[derive(Clone)]
struct Seconds;

#[wgsl_function(wgsl = r#"
    struct Params { k: f32 }

    fn seconds(v: vec2<f32>, p: Params) -> f32 {
        return (v.x + v.y) * p.k;
    }
"#)]
impl ShaderFn for Seconds {
    fn params(&self) -> SecondsParams {
        SecondsParams { k: 1.0 }
    }
}

fn main() {}

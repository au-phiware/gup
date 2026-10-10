// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// `Params` before the value: not a shape the glue calls.
use gup_core::prelude::*;

#[derive(Clone)]
struct Gamma;

#[wgsl_function(wgsl = r#"
    struct Params { gamma: f32 }

    fn gamma(p: Params, t: f32) -> f32 {
        return pow(t, p.gamma);
    }
"#)]
impl ShaderFn for Gamma {
    fn params(&self) -> GammaParams {
        GammaParams { gamma: 2.2 }
    }
}

fn main() {}

// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// A `Params` member ending in a digit: naga renames it, so naga_oil
// rejects it.
use gup_core::prelude::*;

#[derive(Clone)]
struct Gamma;

#[wgsl_function(wgsl = r#"
    struct Params { gamma2: f32 }

    fn gamma(t: f32, p: Params) -> f32 {
        return pow(t, p.gamma2);
    }
"#)]
impl ShaderFn for Gamma {
    fn params(&self) -> GammaParams {
        GammaParams { gamma2: 2.2 }
    }
}

fn main() {}

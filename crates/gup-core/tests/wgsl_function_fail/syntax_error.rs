// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// A WGSL syntax error (a missing semicolon).
use gup_core::prelude::*;

#[derive(Clone)]
struct Gamma;

#[wgsl_function(wgsl = r#"
    struct Params { gamma: f32 }

    fn gamma(t: f32, p: Params) -> f32 {
        let g = p.gamma
        return pow(t, g);
    }
"#)]
impl ShaderFn for Gamma {
    fn params(&self) -> GammaParams {
        GammaParams { gamma: 2.2 }
    }
}

fn main() {}

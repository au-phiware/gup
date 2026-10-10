// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// `monotonic` derives `image` from the CPU mirror, which is missing.
use gup_core::prelude::*;

#[derive(Clone)]
struct Gamma;

#[wgsl_function(monotonic, wgsl = r#"
    struct Params { gamma: f32 }

    fn gamma(t: f32, p: Params) -> f32 {
        return pow(t, p.gamma);
    }
"#)]
impl ShaderFn for Gamma {
    fn params(&self) -> GammaParams {
        GammaParams { gamma: 2.2 }
    }
}

fn main() {}

// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// The attribute goes on the `impl ShaderFn` block, not the type.
use gup_core::prelude::*;

#[wgsl_function(wgsl = r#"
    struct Params { gamma: f32 }

    fn gamma(t: f32, p: Params) -> f32 {
        return pow(t, p.gamma);
    }
"#)]
#[derive(Clone)]
struct Gamma;

fn main() {}

// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// An import that is not in Gup's library.
use gup_core::prelude::*;

#[derive(Clone)]
struct Twice;

#[wgsl_function(wgsl = r#"
    #import gup::scale::liner as linear

    struct Params { k: f32 }

    fn twice(v: f32, p: Params) -> f32 {
        return linear::map(v) * p.k;
    }
"#)]
impl ShaderFn for Twice {
    fn params(&self) -> TwiceParams {
        TwiceParams { k: 2.0 }
    }
}

fn main() {}

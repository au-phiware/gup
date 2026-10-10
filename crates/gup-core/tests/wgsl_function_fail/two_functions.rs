// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// With a helper, the macro needs to be told which function is the entry.
use gup_core::prelude::*;

#[derive(Clone)]
struct Soften;

#[wgsl_function(wgsl = r#"
    struct Params { edge: f32 }

    fn ease(t: f32) -> f32 {
        return t * t * (3.0 - 2.0 * t);
    }

    fn soften(t: f32, p: Params) -> f32 {
        return ease(t / p.edge);
    }
"#)]
impl ShaderFn for Soften {
    fn params(&self) -> SoftenParams {
        SoftenParams { edge: 1.0 }
    }
}

fn main() {}

// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

// A function over dictionary codes is fitted by `fit_keys`.
use gup_core::prelude::*;

#[derive(Clone)]
struct Stripe {
    count: u32,
}

#[wgsl_function(wgsl = r#"
    struct Params { count: u32 }

    fn stripe(code: u32, p: Params) -> vec4<f32> {
        let on = f32(code % 2u);
        return vec4<f32>(on, on, on, 1.0);
    }
"#)]
impl ShaderFn for Stripe {
    fn params(&self) -> StripeParams {
        StripeParams { count: self.count }
    }

    fn fit_domain(&mut self, extent: (f64, f64)) -> gup_core::Result<()> {
        self.count = extent.1 as u32 + 1;
        Ok(())
    }
}

fn main() {}

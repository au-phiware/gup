// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! `#[wgsl_function]` expanding in a crate that only names the facade.
use charts::prelude::*;

#[derive(Clone, Debug)]
pub struct Energy;

#[wgsl_function(
    monotonic,
    wgsl = r#"
    struct Params { slope: f32, offset: f32 }

    fn energy(magnitude: f32, p: Params) -> f32 {
        return pow(10.0, p.slope * magnitude + p.offset);
    }
"#
)]
impl ShaderFn for Energy {
    fn params(&self) -> EnergyParams {
        EnergyParams {
            slope: 1.5,
            offset: 4.8,
        }
    }
}

impl CpuMirror for Energy {
    fn eval(&self, magnitude: f64) -> f64 {
        10f64.powf(1.5 * magnitude + 4.8)
    }
}

// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! RFC-001 S5c (GUP-420): `#[wgsl_function]` from outside gup-core.

use gup_core::prelude::*;

/// Seismic energy released by an earthquake of magnitude `m`:
/// `log10 E = 1.5 m + 4.8` (joules).
#[derive(Clone, Debug)]
struct Energy;

#[wgsl_function(
    monotonic,
    wgsl = r#"
    struct Params {
        slope: f32,
        offset: f32,
        pad_a: u32,
        pad_b: u32,
    }

    fn energy(magnitude: f32, p: Params) -> f32 {
        return exp2((p.slope * magnitude + p.offset) * log2(10.0));
    }
"#
)]
impl ShaderFn for Energy {
    fn params(&self) -> EnergyParams {
        EnergyParams {
            slope: 1.5,
            offset: 4.8,
            ..Default::default()
        }
    }
}

impl CpuMirror for Energy {
    fn eval(&self, m: f64) -> f64 {
        10f64.powf(1.5 * m + 4.8)
    }
}

#[test]
fn the_macro_fills_in_the_static_half() {
    assert_eq!(<Energy as ShaderFn>::ENTRY, "energy");
    assert_eq!(
        <Energy as ShaderFn>::MODULE.import_path,
        "wgsl_function::energy"
    );
    assert!(
        <Energy as ShaderFn>::MODULE
            .wgsl
            .contains("fn wgsl_function_energy_energy("),
        "{}",
        <Energy as ShaderFn>::MODULE.wgsl
    );
    let image = ShaderFn::image(&Energy, (4.0, 6.0)).unwrap();
    assert_eq!(image, (Energy.eval(4.0), Energy.eval(6.0)));
}

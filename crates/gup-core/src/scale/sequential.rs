// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::channel::Color;
use crate::column::ColumnFormat;
use crate::encoding::{CpuMirror, Resource, ShaderFn};
use crate::error::{Error, Result};
use crate::shader::{COLOR_SEQUENTIAL, WgslModule};
use std::sync::Arc;

/// Entries in a palette lookup table.
pub const LUT_SIZE: usize = 256;

/// Viridis control points (sRGB-encoded), evenly spaced.
const VIRIDIS: [[f32; 3]; 11] = [
    [0.267004, 0.004874, 0.329415],
    [0.282623, 0.140926, 0.457517],
    [0.253935, 0.265254, 0.529983],
    [0.206756, 0.371758, 0.553117],
    [0.163625, 0.471133, 0.558148],
    [0.127568, 0.566949, 0.550556],
    [0.134692, 0.658636, 0.517649],
    [0.266941, 0.748751, 0.440573],
    [0.477504, 0.821444, 0.318195],
    [0.741388, 0.873449, 0.149561],
    [0.993248, 0.906157, 0.143936],
];

/// A sequential colour scale: a numeric domain onto a palette LUT.
#[derive(Clone, Debug, PartialEq)]
pub struct Sequential {
    domain: Option<(f64, f64)>,
    explicit: bool,
    reverse: bool,
    lut: Arc<[[u8; 4]]>,
}

impl Sequential {
    /// The viridis palette, with its domain fitted to the data.
    pub fn viridis() -> Self {
        Self::from_stops(&VIRIDIS)
    }

    /// A palette through evenly spaced sRGB `stops`, interpolated in OKLab
    /// into a [`LUT_SIZE`]-entry table.
    pub fn from_stops(stops: &[[f32; 3]]) -> Self {
        Self {
            domain: None,
            explicit: false,
            reverse: false,
            lut: build_lut(stops).into(),
        }
    }

    /// Use a fixed domain instead of fitting the data.
    pub fn domain(mut self, d0: f64, d1: f64) -> Self {
        self.domain = Some((d0, d1));
        self.explicit = true;
        self
    }

    /// Run the palette backwards.
    pub fn reversed(mut self) -> Self {
        self.reverse = !self.reverse;
        self
    }

    /// The domain, if set or fitted.
    pub fn current_domain(&self) -> Option<(f64, f64)> {
        self.domain
    }

    fn norm(&self) -> (f64, f64) {
        let (d0, d1) = self.domain.unwrap_or((0.0, 1.0));
        let inv = if d1 == d0 { 0.0 } else { 1.0 / (d1 - d0) };
        (d0, inv)
    }
}

/// WGSL `gup::color::sequential::Params`.
#[derive(Copy, Clone, Debug, PartialEq, encase::ShaderType)]
pub struct SequentialParams {
    lo: f32,
    inv_span: f32,
    reverse: u32,
}

impl ShaderFn for Sequential {
    type In = f32;
    type Out = Color;
    type Params = SequentialParams;
    const MODULE: &'static WgslModule = &COLOR_SEQUENTIAL;
    const ENTRY: &'static str = "map";

    fn params(&self) -> SequentialParams {
        let (d0, inv) = self.norm();
        SequentialParams {
            lo: d0 as f32,
            inv_span: inv as f32,
            reverse: u32::from(self.reverse),
        }
    }

    fn input_format(&self) -> ColumnFormat {
        ColumnFormat::F32
    }

    fn resources(&self) -> Vec<Resource> {
        vec![Resource::Lut(self.lut.to_vec())]
    }

    fn fit_domain(&mut self, extent: (f64, f64)) -> Result<()> {
        if !extent.0.is_finite() || !extent.1.is_finite() {
            return Err(Error::config(
                "sequential scale domain",
                format!("data extent {extent:?} is not finite"),
            ));
        }
        if !self.explicit {
            self.domain = Some(extent);
        }
        Ok(())
    }
}

impl CpuMirror for Sequential {
    /// Mirrors the shader: normalise, clamp, then linearly interpolate the
    /// 8-bit LUT between texel centres, as hardware filtering does.
    fn eval(&self, x: f64) -> Color {
        let (d0, inv) = self.norm();
        let t = ((x - d0) * inv).clamp(0.0, 1.0);
        sample_lut(&self.lut, if self.reverse { 1.0 - t } else { t })
    }
}

impl Sequential {
    /// The palette LUT (shared, not copied) and whether it runs backwards:
    /// what a colour legend for this scale draws.
    pub(crate) fn palette(&self) -> (Arc<[[u8; 4]]>, bool) {
        (Arc::clone(&self.lut), self.reverse)
    }
}

/// `lut` at `t` in `0..=1`, linearly interpolated between texel centres
/// as the GPU's filtering sampler does (the CPU mirror of
/// `gup::color::sequential::map` after normalising).
pub(crate) fn sample_lut(lut: &[[u8; 4]], t: f64) -> Color {
    let pos = t.clamp(0.0, 1.0) * (lut.len() - 1) as f64;
    let i0 = (pos.floor() as usize).min(lut.len() - 2);
    let f = pos - i0 as f64;
    let c = |k: usize| {
        let a = f64::from(lut[i0][k]) / 255.0;
        let b = f64::from(lut[i0 + 1][k]) / 255.0;
        (a + (b - a) * f) as f32
    };
    Color {
        r: c(0),
        g: c(1),
        b: c(2),
        a: c(3),
    }
}

fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(c: f32) -> f32 {
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

fn srgb_to_oklab([r, g, b]: [f32; 3]) -> [f32; 3] {
    let (r, g, b) = (srgb_to_linear(r), srgb_to_linear(g), srgb_to_linear(b));
    let l = (0.412_221_46 * r + 0.536_332_55 * g + 0.051_445_995 * b).cbrt();
    let m = (0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b).cbrt();
    let s = (0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b).cbrt();
    [
        0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s,
        1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s,
        0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s,
    ]
}

fn oklab_to_srgb([l, a, b]: [f32; 3]) -> [f32; 3] {
    let l_ = (l + 0.396_337_78 * a + 0.215_803_76 * b).powi(3);
    let m_ = (l - 0.105_561_346 * a - 0.063_854_17 * b).powi(3);
    let s_ = (l - 0.089_484_18 * a - 1.291_485_5 * b).powi(3);
    [
        4.076_741_7 * l_ - 3.307_711_6 * m_ + 0.230_969_94 * s_,
        -1.268_438 * l_ + 2.609_757_4 * m_ - 0.341_319_38 * s_,
        -0.004_196_086_3 * l_ - 0.703_418_6 * m_ + 1.707_614_7 * s_,
    ]
    .map(|c| linear_to_srgb(c.clamp(0.0, 1.0)))
}

/// Interpolate evenly spaced sRGB stops in OKLab into an RGBA8 LUT.
fn build_lut(stops: &[[f32; 3]]) -> Vec<[u8; 4]> {
    assert!(stops.len() >= 2, "a palette needs at least two stops");
    let lab: Vec<[f32; 3]> = stops.iter().map(|&s| srgb_to_oklab(s)).collect();
    (0..LUT_SIZE)
        .map(|i| {
            let pos = i as f32 / (LUT_SIZE - 1) as f32 * (lab.len() - 1) as f32;
            let j = (pos.floor() as usize).min(lab.len() - 2);
            let f = pos - j as f32;
            let mix = |k: usize| lab[j][k] + (lab[j + 1][k] - lab[j][k]) * f;
            let [r, g, b] = oklab_to_srgb([mix(0), mix(1), mix(2)]);
            let q = |c: f32| (c.clamp(0.0, 1.0) * 255.0).round() as u8;
            [q(r), q(g), q(b), 255]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lut_endpoints_are_the_palette_endpoints() {
        let s = Sequential::viridis().domain(0.0, 1.0);
        assert_eq!(s.eval(0.0).to_rgba8(), [68, 1, 84, 255]); // #440154
        assert_eq!(s.eval(1.0).to_rgba8(), [253, 231, 37, 255]); // #fde725
        // Out-of-domain values clamp.
        assert_eq!(s.eval(-5.0), s.eval(0.0));
        assert_eq!(s.eval(5.0), s.eval(1.0));
        // Midpoint is viridis teal (#21918c within a couple of units).
        let mid = s.eval(0.5).to_rgba8();
        for (got, want) in mid.iter().zip([33u8, 145, 140]) {
            assert!(got.abs_diff(want) <= 3, "{mid:?}");
        }
    }

    #[test]
    fn reversed_runs_backwards() {
        let s = Sequential::viridis().domain(0.0, 10.0).reversed();
        assert_eq!(s.eval(0.0).to_rgba8(), [253, 231, 37, 255]);
    }

    #[test]
    fn oklab_round_trips() {
        for c in [[0.2, 0.5, 0.9], [1.0, 1.0, 1.0], [0.0, 0.0, 0.0]] {
            let back = oklab_to_srgb(srgb_to_oklab(c));
            for k in 0..3 {
                assert!((back[k] - c[k]).abs() < 1e-3, "{c:?} → {back:?}");
            }
        }
    }
}

// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

use super::legend::{ColorScale, Legend, Ramp};
use super::sequential::{build_lut, sample_lut};
use crate::channel::Color;
use crate::column::ColumnFormat;
use crate::encoding::{CpuMirror, Resource, ShaderFn};
use crate::error::{Error, Result};
use crate::shader::{COLOR_DIVERGING, WgslModule};
use std::sync::Arc;

/// Entries in a diverging LUT: odd, so the midpoint is a texel centre.
const DIVERGING_LUT_SIZE: usize = 257;

/// Blue through near-white to red (sRGB), cold to hot: the 11-class
/// red–blue ColorBrewer scheme, low values blue.
const BLUE_RED: [[f32; 3]; 11] = [
    rgb(0x053061),
    rgb(0x2166ac),
    rgb(0x4393c3),
    rgb(0x92c5de),
    rgb(0xd1e5f0),
    rgb(0xf7f7f7),
    rgb(0xfddbc7),
    rgb(0xf4a582),
    rgb(0xd6604d),
    rgb(0xb2182b),
    rgb(0x67001f),
];

/// sRGB components of a `0xRRGGBB` literal.
const fn rgb(hex: u32) -> [f32; 3] {
    let c = Color::hex(hex);
    [c.r, c.g, c.b]
}

/// A diverging colour scale: two colour ramps meeting at a midpoint, for
/// a signed quantity (an anomaly, a change, a correlation). Values below
/// the midpoint take the low ramp, values above it the high ramp, and
/// the midpoint itself the palette's middle colour.
///
/// The automatic domain is **symmetric** about the midpoint: it reaches
/// as far below the midpoint as the data's farthest value reaches either
/// side, so equal distances from the midpoint get equally strong
/// colours. [`domain`](Self::domain) sets the low end, midpoint and high
/// end explicitly; each side then has its own slope.
///
/// Its palette is a 257-entry LUT (the midpoint's colour is a texel), as
/// [`Sequential`](super::Sequential)'s is, so `Linear.then(diverging)`
/// and a [`GradientBar`](crate::scene::GradientBar) legend work the same
/// way.
///
/// ```
/// use gup_core::prelude::*;
///
/// let anomaly = Diverging::blue_red().domain(-2.0, 0.0, 4.0);
/// // The midpoint is the palette's middle (near-white); the ends are its
/// // ends, and each side is linear in its own half.
/// assert_eq!(anomaly.eval(0.0).to_rgba8(), [247, 247, 247, 255]);
/// assert_eq!(anomaly.eval(-2.0).to_rgba8(), [5, 48, 97, 255]);
/// assert_eq!(anomaly.eval(4.0).to_rgba8(), [103, 0, 31, 255]);
/// assert_eq!(anomaly.eval(-1.0), anomaly.ramp().color_at(0.25));
/// assert_eq!(anomaly.eval(2.0), anomaly.ramp().color_at(0.75));
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct Diverging {
    /// Low end, midpoint, high end, once set or fitted.
    domain: Option<[f64; 3]>,
    mid: f64,
    explicit: bool,
    reverse: bool,
    lut: Arc<[[u8; 4]]>,
}

impl Diverging {
    /// Blue (low) through near-white (the midpoint, 0 by default) to red
    /// (high), with its domain fitted to the data.
    pub fn blue_red() -> Self {
        Self::from_stops(&BLUE_RED)
    }

    /// A palette through evenly spaced sRGB `stops`, interpolated in OKLab;
    /// the middle stop is the midpoint's colour.
    ///
    /// # Panics
    ///
    /// If there are fewer than three stops, or an even number.
    pub fn from_stops(stops: &[[f32; 3]]) -> Self {
        assert!(
            stops.len() >= 3 && stops.len() % 2 == 1,
            "a diverging palette needs an odd number (at least 3) of stops, not {}",
            stops.len()
        );
        Self {
            domain: None,
            mid: 0.0,
            explicit: false,
            reverse: false,
            lut: build_lut(stops, DIVERGING_LUT_SIZE).into(),
        }
    }

    /// Fit the domain symmetrically about `mid` instead of 0.
    pub fn midpoint(mut self, mid: f64) -> Self {
        self.mid = mid;
        self
    }

    /// Use a fixed domain, `d0` (low end), `mid` and `d1` (high end),
    /// instead of fitting the data.
    pub fn domain(mut self, d0: f64, mid: f64, d1: f64) -> Self {
        self.domain = Some([d0, mid, d1]);
        self.mid = mid;
        self.explicit = true;
        self
    }

    /// Run the palette backwards.
    pub fn reversed(mut self) -> Self {
        self.reverse = !self.reverse;
        self
    }

    /// The low end, midpoint and high end, if set or fitted.
    pub fn current_domain(&self) -> Option<[f64; 3]> {
        self.domain
    }

    /// The legend's ramp: the palette from the low end to the high end,
    /// with the midpoint at its centre and ticks at round values on both
    /// sides. Draw it with [`GradientBar::new`](crate::scene::GradientBar::new).
    pub fn ramp(&self) -> Ramp {
        let stops = self.stops();
        Ramp::piecewise(Arc::clone(&self.lut), self.reverse, stops, 6)
    }

    fn stops(&self) -> [f64; 3] {
        self.domain
            .unwrap_or([self.mid - 1.0, self.mid, self.mid + 1.0])
    }

    /// The midpoint and each side's slope in normalised units (0.5 per
    /// half), as the shader takes them.
    fn slopes(&self) -> (f64, f64, f64) {
        let [d0, mid, d1] = self.stops();
        let slope = |a: f64, b: f64| if b == a { 0.0 } else { 0.5 / (b - a) };
        (mid, slope(d0, mid), slope(mid, d1))
    }
}

/// WGSL `gup::color::diverging::Params`.
#[derive(Copy, Clone, Debug, PartialEq, encase::ShaderType)]
pub struct DivergingParams {
    mid: f32,
    inv_lo: f32,
    inv_hi: f32,
    reverse: u32,
}

impl ShaderFn for Diverging {
    type In = f32;
    type Out = Color;
    type Params = DivergingParams;
    const MODULE: &'static WgslModule = &COLOR_DIVERGING;
    const ENTRY: &'static str = "map";

    fn params(&self) -> DivergingParams {
        let (mid, inv_lo, inv_hi) = self.slopes();
        DivergingParams {
            mid: mid as f32,
            inv_lo: inv_lo as f32,
            inv_hi: inv_hi as f32,
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
                "diverging scale domain",
                format!("data extent {extent:?} is not finite"),
            ));
        }
        if !self.explicit {
            let reach = (extent.0 - self.mid).abs().max((extent.1 - self.mid).abs());
            self.domain = Some([self.mid - reach, self.mid, self.mid + reach]);
        }
        Ok(())
    }
}

impl CpuMirror for Diverging {
    /// Mirrors the shader: each side's slope from the midpoint, clamp,
    /// then the LUT linearly interpolated between texel centres.
    fn eval(&self, x: f64) -> Color {
        let (mid, inv_lo, inv_hi) = self.slopes();
        let d = x - mid;
        let t = (0.5 + d * if d < 0.0 { inv_lo } else { inv_hi }).clamp(0.0, 1.0);
        sample_lut(&self.lut, if self.reverse { 1.0 - t } else { t })
    }
}

impl ColorScale for Diverging {
    fn legend(&self) -> Legend {
        Legend::Ramp(self.ramp())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_auto_domain_is_symmetric_about_the_midpoint() {
        let mut d = Diverging::blue_red().midpoint(10.0);
        d.fit_domain((4.0, 13.0)).unwrap();
        assert_eq!(d.current_domain(), Some([4.0, 10.0, 16.0]));
        // Equal distances, equal strength: opposite ends of the palette.
        assert_eq!(d.eval(10.0).to_rgba8(), [247, 247, 247, 255]);
        assert_eq!(d.eval(4.0).to_rgba8(), [5, 48, 97, 255]);
        assert_eq!(d.eval(16.0).to_rgba8(), [103, 0, 31, 255]);
        // Clamped outside the domain.
        assert_eq!(d.eval(-100.0), d.eval(4.0));
        // An explicit domain is kept.
        let mut e = Diverging::blue_red().domain(-1.0, 0.0, 3.0);
        e.fit_domain((-50.0, 50.0)).unwrap();
        assert_eq!(e.current_domain(), Some([-1.0, 0.0, 3.0]));
    }

    /// Below the midpoint is blue and above it red, getting darker (lower
    /// OKLab lightness) towards both ends: the ramp reads correctly.
    #[test]
    fn colours_darken_away_from_the_midpoint() {
        let d = Diverging::blue_red().domain(-1.0, 0.0, 1.0);
        let lightness = |v: f64| {
            let c = d.eval(v);
            super::super::sequential::srgb_to_oklab([c.r, c.g, c.b])[0]
        };
        for k in 0..20 {
            let v = f64::from(k) / 20.0;
            assert!(lightness(v + 0.05) < lightness(v), "above at {v}");
            assert!(lightness(-v - 0.05) < lightness(-v), "below at {v}");
        }
        let (cold, hot) = (d.eval(-0.5), d.eval(0.5));
        assert!(cold.b > cold.r && hot.r > hot.b, "{cold:?} {hot:?}");
    }

    #[test]
    fn the_legend_ramp_labels_both_sides_and_the_midpoint() {
        let d = Diverging::blue_red().domain(-2.0, 0.0, 4.0);
        let ramp = d.ramp();
        let ticks: Vec<(f64, &str)> = ramp
            .ticks()
            .iter()
            .map(|t| (t.value, t.label.as_str()))
            .collect();
        assert!(ticks.contains(&(0.0, "0")), "{ticks:?}");
        let zero = ramp.ticks().iter().find(|t| t.value == 0.0).unwrap();
        assert_eq!(zero.t, 0.5);
        assert_eq!(ramp.domain(), (-2.0, 4.0));
        for t in ramp.ticks() {
            assert_eq!(ramp.color_at(t.t), d.eval(t.value), "{}", t.label);
        }
    }
}

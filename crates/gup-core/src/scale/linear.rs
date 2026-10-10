// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

use super::{PositionScale, Ticks, format_with_step, step_value, tick_step};
use crate::channel::Px;
use crate::column::ColumnFormat;
use crate::encoding::{CpuMirror, ShaderFn};
use crate::error::{Error, Result};
use crate::shader::{SCALE_LINEAR, WgslModule};

/// A linear position scale: `px = r0 + (x - d0) · (r1 - r0) / (d1 - d0)`.
///
/// Reads [`ColumnFormat::F32Relative`] columns, so large values such as
/// Unix timestamps keep their precision on the GPU.
#[derive(Clone, Debug, PartialEq)]
pub struct Linear {
    domain: Option<(f64, f64)>,
    explicit: bool,
    range: (f64, f64),
}

impl Default for Linear {
    fn default() -> Self {
        Self::new()
    }
}

impl Linear {
    /// A scale whose domain is fitted to the data (and made nice).
    pub fn new() -> Self {
        Self {
            domain: None,
            explicit: false,
            range: (0.0, 1.0),
        }
    }

    /// Use a fixed domain instead of fitting the data.
    pub fn domain(mut self, d0: f64, d1: f64) -> Self {
        self.domain = Some((d0, d1));
        self.explicit = true;
        self
    }

    /// Set the pixel range.
    pub fn range(mut self, r0: Px, r1: Px) -> Self {
        self.set_range(r0, r1);
        self
    }

    fn resolved_domain(&self) -> (f64, f64) {
        self.domain.unwrap_or((0.0, 1.0))
    }

    /// Replace the domain, keeping whether it is automatic: for scales
    /// that wrap a linear one and make it nice their own way.
    pub(super) fn replace_domain(&mut self, d: (f64, f64)) {
        self.domain = Some(d);
    }

    /// Pixels per domain unit (0 for an empty domain).
    fn k(&self) -> f64 {
        let (d0, d1) = self.resolved_domain();
        let span = d1 - d0;
        if span == 0.0 {
            0.0
        } else {
            (self.range.1 - self.range.0) / span
        }
    }
}

/// WGSL `gup::scale::linear::Params`.
#[derive(Copy, Clone, Debug, PartialEq, encase::ShaderType)]
pub struct LinearParams {
    k: f32,
    range_start: f32,
    // To 16 bytes, as every uniform `Params` (see the WGSL module).
    pad_a: u32,
    pad_b: u32,
}

impl ShaderFn for Linear {
    type In = f32;
    type Out = Px;
    type Params = LinearParams;
    const MODULE: &'static WgslModule = &SCALE_LINEAR;
    const ENTRY: &'static str = "map_rel";

    fn params(&self) -> LinearParams {
        LinearParams {
            k: self.k() as f32,
            range_start: self.range.0 as f32,
            pad_a: 0,
            pad_b: 0,
        }
    }

    fn input_format(&self) -> ColumnFormat {
        ColumnFormat::F32Relative
    }

    /// `origin - d0`, computed in f64.
    fn chunk_base(&self, origin: f64) -> f64 {
        origin - self.resolved_domain().0
    }

    fn fit_domain(&mut self, extent: (f64, f64)) -> Result<()> {
        if !extent.0.is_finite() || !extent.1.is_finite() {
            return Err(Error::config(
                "linear scale domain",
                format!("data extent {extent:?} is not finite"),
            ));
        }
        if !self.explicit {
            self.domain = Some(extent);
        }
        Ok(())
    }
}

impl CpuMirror for Linear {
    fn eval(&self, x: f64) -> f64 {
        self.range.0 + (x - self.resolved_domain().0) * self.k()
    }
}

impl PositionScale for Linear {
    fn current_domain(&self) -> Option<(f64, f64)> {
        self.domain
    }

    fn is_auto(&self) -> bool {
        !self.explicit
    }

    fn set_range(&mut self, r0: Px, r1: Px) {
        self.range = (f64::from(r0.0), f64::from(r1.0));
    }

    fn set_domain(&mut self, d0: f64, d1: f64) -> Result<()> {
        if !(d0.is_finite() && d1.is_finite()) || d0 == d1 {
            return Err(Error::config(
                "linear scale domain",
                format!("({d0}, {d1}) must be finite and non-empty"),
            ));
        }
        self.domain = Some((d0, d1));
        self.explicit = true;
        Ok(())
    }

    fn invert(&self, px: f64) -> f64 {
        let k = self.k();
        let d0 = self.resolved_domain().0;
        if k == 0.0 {
            d0
        } else {
            d0 + (px - self.range.0) / k
        }
    }

    fn nice(&mut self) {
        let (mut lo, mut hi) = self.resolved_domain();
        if lo == hi {
            let pad = if lo == 0.0 { 1.0 } else { lo.abs() * 0.1 };
            (lo, hi) = (lo - pad, hi + pad);
        }
        // Two passes: the step can change once the domain grows.
        for _ in 0..2 {
            let step = tick_step(lo, hi, 10);
            lo = (lo / step).floor() * step;
            hi = (hi / step).ceil() * step;
        }
        self.domain = Some((lo, hi));
    }

    fn ticks(&self, count: usize) -> Ticks {
        let (d0, d1) = self.resolved_domain();
        let (lo, hi) = (d0.min(d1), d0.max(d1));
        let step = tick_step(lo, hi, count);
        let (start, end) = ((lo / step - 1e-9).ceil(), (hi / step + 1e-9).floor());
        let values: Vec<f64> = (0..=((end - start).max(-1.0) as i64))
            .map(|i| step_value(start + i as f64, step))
            .collect();
        let labels = values.iter().map(|&v| format_with_step(v, step)).collect();
        Ticks { values, labels }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scale::{DynPositionScale, ScaleRef};

    #[test]
    fn invert_undoes_eval() {
        let s = Linear::new().domain(10.0, 20.0).range(Px(100.0), Px(300.0));
        assert_eq!(s.invert(200.0), 15.0);
        assert_eq!(s.invert(s.eval(12.5)), 12.5);
    }

    #[test]
    fn set_domain_makes_the_domain_explicit() {
        let mut s = Linear::new();
        s.set_domain(-5.0, 5.0).unwrap();
        s.fit_domain((0.0, 100.0)).unwrap();
        assert_eq!(s.current_domain(), Some((-5.0, 5.0)));
        assert!(!s.is_auto());
        assert!(s.set_domain(1.0, 1.0).is_err());
        assert!(s.set_domain(f64::NAN, 1.0).is_err());
    }

    /// Zooming keeps the value under the anchor where it is, and works the
    /// same through the object-safe handle the plot holds.
    #[test]
    fn zoom_keeps_the_anchor_fixed() {
        let s = ScaleRef::new(Linear::new().domain(0.0, 100.0).range(Px(0.0), Px(500.0)));
        let anchor = 100.0; // value 20
        s.zoom(anchor, 0.5).unwrap();
        assert_eq!(s.current_domain(), Some((10.0, 60.0)));
        assert_eq!(DynPositionScale::eval(&s, 20.0), anchor);
        let log = ScaleRef::new(crate::Log::new().domain(1.0, 1e4).range(Px(400.0), Px(0.0)));
        log.zoom(200.0, 0.5).unwrap(); // value 100, two decades each way
        let (d0, d1) = log.current_domain().unwrap();
        assert!(
            (d0 - 10.0).abs() < 1e-9 && (d1 - 1e3).abs() < 1e-9,
            "{d0} {d1}"
        );
    }

    #[test]
    fn eval_maps_domain_onto_range() {
        let s = Linear::new().domain(10.0, 20.0).range(Px(100.0), Px(300.0));
        assert_eq!(s.eval(10.0), 100.0);
        assert_eq!(s.eval(15.0), 200.0);
        assert_eq!(s.eval(20.0), 300.0);
        // Reversed ranges (y axes) work too.
        let y = Linear::new().domain(0.0, 1.0).range(Px(400.0), Px(0.0));
        assert_eq!(y.eval(0.25), 300.0);
    }

    #[test]
    fn fitted_domains_nice_to_round_values() {
        let mut s = Linear::new();
        s.fit_domain((0.37, 9.6)).unwrap();
        s.nice();
        assert_eq!(s.current_domain(), Some((0.0, 10.0)));
        let t = s.ticks(5);
        assert_eq!(t.values, vec![0.0, 2.0, 4.0, 6.0, 8.0, 10.0]);
        assert_eq!(t.labels, vec!["0", "2", "4", "6", "8", "10"]);
    }

    #[test]
    fn explicit_domains_ignore_the_data() {
        let mut s = Linear::new().domain(-1.0, 1.0);
        s.fit_domain((0.0, 100.0)).unwrap();
        assert_eq!(s.current_domain(), Some((-1.0, 1.0)));
        assert!(!s.is_auto());
    }

    #[test]
    fn chunk_base_folds_the_domain_start_in_f64() {
        let s = Linear::new()
            .domain(1.7e9, 1.7e9 + 86_400.0)
            .range(Px(0.0), Px(864.0));
        let origin = 1.7e9 + 3_600.0;
        assert_eq!(ShaderFn::chunk_base(&s, origin), 3_600.0);
        // GPU formula with a relative value v = x - origin.
        let v = 0.5f32;
        let p = s.params();
        let px = p.range_start + (v + ShaderFn::chunk_base(&s, origin) as f32) * p.k;
        assert!((f64::from(px) - s.eval(origin + 0.5)).abs() < 1e-3);
    }

    #[test]
    fn fractional_ticks_label_with_enough_decimals() {
        let s = Linear::new().domain(0.0, 1.0);
        let t = s.ticks(5);
        assert_eq!(t.labels, vec!["0.0", "0.2", "0.4", "0.6", "0.8", "1.0"]);
    }
}

// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

use super::{PositionScale, Ticks};
use crate::channel::Px;
use crate::column::ColumnFormat;
use crate::encoding::{CpuMirror, ShaderFn};
use crate::error::{Error, Result};
use crate::shader::{SCALE_LOG, WgslModule};

/// A logarithmic position scale over a strictly positive domain.
///
/// Reads absolute [`ColumnFormat::F32`] columns: log is scale invariant, so
/// absolute f32 keeps relative precision. The base affects ticks only.
#[derive(Clone, Debug, PartialEq)]
pub struct Log {
    domain: Option<(f64, f64)>,
    explicit: bool,
    range: (f64, f64),
    base: f64,
}

impl Default for Log {
    fn default() -> Self {
        Self::new()
    }
}

impl Log {
    /// A base-10 scale whose domain is fitted to the data (and made nice).
    pub fn new() -> Self {
        Self {
            domain: None,
            explicit: false,
            range: (0.0, 1.0),
            base: 10.0,
        }
    }

    /// Use a fixed (strictly positive) domain instead of fitting the data.
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
        self.domain.unwrap_or((1.0, self.base))
    }

    fn log_parts(&self) -> (f64, f64) {
        let (d0, d1) = self.resolved_domain();
        let (l0, l1) = (d0.log2(), d1.log2());
        let inv = if l1 == l0 { 0.0 } else { 1.0 / (l1 - l0) };
        (l0, inv)
    }
}

/// WGSL `gup::scale::log::Params`.
#[derive(Copy, Clone, Debug, PartialEq, encase::ShaderType)]
pub struct LogParams {
    log_lo: f32,
    inv_log_span: f32,
    range_start: f32,
    r_span: f32,
}

impl ShaderFn for Log {
    type In = f32;
    type Out = Px;
    type Params = LogParams;
    const MODULE: &'static WgslModule = &SCALE_LOG;
    const ENTRY: &'static str = "map";

    fn image(&self, extent: (f64, f64)) -> Option<(f64, f64)> {
        super::monotonic_image(|x| self.eval(x), extent)
    }

    fn params(&self) -> LogParams {
        let (log_d0, inv_log_span) = self.log_parts();
        LogParams {
            log_lo: log_d0 as f32,
            inv_log_span: inv_log_span as f32,
            range_start: self.range.0 as f32,
            r_span: (self.range.1 - self.range.0) as f32,
        }
    }

    fn input_format(&self) -> ColumnFormat {
        ColumnFormat::F32
    }

    fn fit_domain(&mut self, extent: (f64, f64)) -> Result<()> {
        if !(extent.0 > 0.0 && extent.1.is_finite()) {
            return Err(Error::config(
                "log scale domain",
                format!(
                    "data extent {extent:?} includes values ≤ 0, which a log scale cannot show; \
                     filter them out or use a linear scale"
                ),
            ));
        }
        if !self.explicit {
            self.domain = Some(extent);
        }
        Ok(())
    }
}

impl CpuMirror for Log {
    fn eval(&self, x: f64) -> f64 {
        let (log_d0, inv) = self.log_parts();
        self.range.0 + (x.log2() - log_d0) * inv * (self.range.1 - self.range.0)
    }
}

impl PositionScale for Log {
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
        if !(d0 > 0.0 && d1 > 0.0 && d0.is_finite() && d1.is_finite()) || d0 == d1 {
            return Err(Error::config(
                "log scale domain",
                format!("({d0}, {d1}) must be finite, positive and non-empty"),
            ));
        }
        self.domain = Some((d0, d1));
        self.explicit = true;
        Ok(())
    }

    fn invert(&self, px: f64) -> f64 {
        let (d0, d1) = self.resolved_domain();
        let span = self.range.1 - self.range.0;
        if span == 0.0 {
            return d0;
        }
        let t = (px - self.range.0) / span;
        (d0.log2() + t * (d1.log2() - d0.log2())).exp2()
    }

    /// Extend the domain to whole powers of the base.
    fn nice(&mut self) {
        let (d0, d1) = self.resolved_domain();
        let b = self.base.ln();
        let lo = self.base.powf((d0.ln() / b - 1e-9).floor());
        let mut hi = self.base.powf((d1.ln() / b + 1e-9).ceil());
        if hi <= lo {
            hi = lo * self.base;
        }
        self.domain = Some((lo, hi));
    }

    /// Powers of the base, plus 2× and 5× multiples when there are fewer
    /// than `count` decades.
    fn ticks(&self, count: usize) -> Ticks {
        let (d0, d1) = self.resolved_domain();
        let (lo, hi) = (d0.min(d1), d0.max(d1));
        let b = self.base;
        let (k0, k1) = (
            (lo.ln() / b.ln() - 1e-9).floor() as i32,
            (hi.ln() / b.ln() + 1e-9).ceil() as i32,
        );
        let in_domain = |v: f64| v >= lo * (1.0 - 1e-9) && v <= hi * (1.0 + 1e-9);
        let decades = (k0..=k1).map(|k| b.powi(k)).filter(|&v| in_domain(v));
        let mut values: Vec<f64> = decades.clone().collect();
        if values.len() < count && b == 10.0 {
            values = (k0..=k1)
                .flat_map(|k| [1.0, 2.0, 5.0].map(|m| m * b.powi(k)))
                .filter(|&v| in_domain(v))
                .collect();
        }
        let labels = values.iter().map(|&v| format_log_tick(v)).collect();
        Ticks { values, labels }
    }
}

/// Plain decimals from 0.001 to 999, SI suffixes (k, M, G, T) from 1000 up
/// and exponent form below 0.001, so one axis never mixes notations.
pub(super) fn format_log_tick(v: f64) -> String {
    // Round away binary noise from powi (e.g. 0.30000000000000004).
    let clean = |x: f64| {
        let scale = 10f64.powi(6 - x.abs().log10().floor() as i32);
        format!("{}", (x * scale).round() / scale)
    };
    if v < 1e-3 {
        return format!("{v:e}");
    }
    for (div, suffix) in [(1e12, "T"), (1e9, "G"), (1e6, "M"), (1e3, "k")] {
        if v >= div {
            return format!("{}{suffix}", clean(v / div));
        }
    }
    clean(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invert_undoes_eval_in_log_space() {
        let s = Log::new().domain(1.0, 1e6).range(Px(400.0), Px(0.0));
        for x in [1.0, 3.0, 1e3, 4.2e5, 1e6] {
            assert!((s.invert(s.eval(x)) / x - 1.0).abs() < 1e-12, "{x}");
        }
        assert!((s.invert(200.0) - 1e3).abs() < 1e-9);
    }

    #[test]
    fn set_domain_rejects_non_positive_or_empty_domains() {
        let mut s = Log::new();
        s.set_domain(10.0, 1e4).unwrap();
        assert_eq!(s.current_domain(), Some((10.0, 1e4)));
        assert!(!s.is_auto());
        for (d0, d1) in [(0.0, 10.0), (-1.0, 10.0), (5.0, 5.0), (1.0, f64::INFINITY)] {
            let err = s.set_domain(d0, d1).unwrap_err();
            assert!(err.to_string().contains("positive"), "{err}");
        }
        assert_eq!(s.current_domain(), Some((10.0, 1e4)));
    }

    #[test]
    fn eval_maps_decades_evenly() {
        let s = Log::new().domain(1.0, 1000.0).range(Px(300.0), Px(0.0));
        assert!((s.eval(1.0) - 300.0).abs() < 1e-9);
        assert!((s.eval(10.0) - 200.0).abs() < 1e-9);
        assert!((s.eval(100.0) - 100.0).abs() < 1e-9);
        assert!((s.eval(1000.0) - 0.0).abs() < 1e-9);
    }

    #[test]
    fn nice_extends_to_powers_of_ten() {
        let mut s = Log::new();
        s.fit_domain((3.2, 870.0)).unwrap();
        s.nice();
        assert_eq!(s.current_domain(), Some((1.0, 1000.0)));
    }

    #[test]
    fn few_decades_get_two_and_five_ticks() {
        let s = Log::new().domain(1.0, 1000.0);
        let t = s.ticks(5);
        assert_eq!(
            t.labels,
            vec!["1", "2", "5", "10", "20", "50", "100", "200", "500", "1k"]
        );
        let wide = Log::new().domain(1e-2, 1e8);
        assert_eq!(wide.ticks(5).values.len(), 11);
        assert_eq!(wide.ticks(5).labels[0], "0.01");
        assert_eq!(wide.ticks(5).labels[10], "100M");
        let pop = Log::new().domain(1e5, 1e9).ticks(8).labels;
        assert_eq!(pop[..4], ["100k", "200k", "500k", "1M"]);
        assert_eq!(pop.last().unwrap(), "1G");
        assert_eq!(format_log_tick(2e-5), "2e-5");
    }

    #[test]
    fn non_positive_data_is_a_configuration_error() {
        let mut s = Log::new();
        let err = s.fit_domain((0.0, 10.0)).unwrap_err();
        assert!(err.to_string().contains("log scale"), "{err}");
    }
}

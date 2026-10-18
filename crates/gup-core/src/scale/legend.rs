// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Colour scales and what their legends show (RFC-001 §5): labelled
//! swatches for a categorical scale, a palette ramp with ticks for a
//! continuous one. Placing a legend in a chart is RFC-001 S7's guides; a
//! [`Ramp`] already draws as a [`GradientBar`](crate::scene::GradientBar).

use super::sequential::sample_lut;
use crate::channel::Color;
use crate::encoding::{CpuMirror, EncodeFn, ShaderFn};
use std::sync::Arc;

/// A colour scale: a [`ShaderFn`] producing a [`Color`], with an f64
/// mirror. Its [`legend`](ShaderFn::legend) shows what the scale maps,
/// from its current domain, in the mirror's colours, so they match the
/// GPU's.
pub trait ColorScale: ShaderFn<Out = Color> + EncodeFn<Output = Color> + CpuMirror {}

/// A colour scale's legend.
#[derive(Clone, Debug, PartialEq)]
pub enum Legend {
    /// One swatch per key, in domain (first-seen) order.
    Swatches(Vec<Swatch>),
    /// A continuous palette with labelled ticks.
    Ramp(Ramp),
}

impl Legend {
    /// The swatches (none for a ramp).
    pub fn swatches(&self) -> &[Swatch] {
        match self {
            Legend::Swatches(s) => s,
            Legend::Ramp(_) => &[],
        }
    }

    /// The ramp, if this is one.
    pub fn ramp(&self) -> Option<&Ramp> {
        match self {
            Legend::Ramp(r) => Some(r),
            Legend::Swatches(_) => None,
        }
    }
}

/// A key and the colour it is drawn in.
#[derive(Clone, Debug, PartialEq)]
pub struct Swatch {
    /// The key's label.
    pub label: Arc<str>,
    /// Its colour, exactly as the GPU draws it.
    pub color: Color,
}

/// A palette from a scale's low end (`t = 0`) to its high end (`t = 1`),
/// with ticks at round domain values. For a [`Diverging`](super::Diverging)
/// scale with an off-centre midpoint, `t` is piecewise linear in the
/// value: the midpoint is always at `t = 0.5`.
#[derive(Clone, Debug, PartialEq)]
pub struct Ramp {
    pub(crate) lut: Arc<[[u8; 4]]>,
    pub(crate) reverse: bool,
    /// Domain values at `t = 0`, `0.5` and `1`.
    pub(crate) stops: [f64; 3],
    pub(crate) ticks: Vec<RampTick>,
}

/// A labelled value on a [`Ramp`].
#[derive(Clone, Debug, PartialEq)]
pub struct RampTick {
    /// Where along the ramp, from 0 (low end) to 1 (high end).
    pub t: f64,
    /// The domain value.
    pub value: f64,
    /// Its label.
    pub label: String,
}

impl Ramp {
    /// The domain's low and high ends.
    pub fn domain(&self) -> (f64, f64) {
        (self.stops[0], self.stops[2])
    }

    /// The ticks, low to high.
    pub fn ticks(&self) -> &[RampTick] {
        &self.ticks
    }

    /// Where `value` falls along the ramp (0 to 1, clamped).
    pub fn t(&self, value: f64) -> f64 {
        let [lo, mid, hi] = self.stops;
        let half = |a: f64, b: f64, v: f64| if b == a { 0.0 } else { (v - a) / (b - a) };
        let t = if value < mid {
            0.5 * half(lo, mid, value)
        } else {
            0.5 + 0.5 * half(mid, hi, value)
        };
        t.clamp(0.0, 1.0)
    }

    /// The colour at `t` (0 to 1), as the GPU samples the palette.
    pub fn color_at(&self, t: f64) -> Color {
        let t = t.clamp(0.0, 1.0);
        sample_lut(&self.lut, if self.reverse { 1.0 - t } else { t })
    }

    /// A ramp of `f`'s colours over `domain` (256 entries, `t` linear in
    /// the value), with about `ticks` ticks at round values: the legend of
    /// a colour function that has a CPU mirror but no palette of its own.
    pub fn sampled(domain: (f64, f64), f: impl Fn(f64) -> Color, ticks: usize) -> Self {
        const SIZE: usize = 256;
        let byte = |c: f32| (c.clamp(0.0, 1.0) * 255.0).round() as u8;
        let lut: Vec<[u8; 4]> = (0..SIZE)
            .map(|i| {
                let v = domain.0 + (domain.1 - domain.0) * i as f64 / (SIZE - 1) as f64;
                let c = f(v);
                [byte(c.r), byte(c.g), byte(c.b), byte(c.a)]
            })
            .collect();
        Self::linear(lut.into(), false, domain, ticks)
    }

    /// This ramp with its ticks replaced by about `count` round values of
    /// the data `extent` that feeds it through `image` (a chain's earlier
    /// links): each tick's `t` is where its image falls along the bar. A
    /// tick whose image is not finite is dropped. Ticks stay in data
    /// order, so a non-monotonic `image` puts them out of order on the bar.
    pub(crate) fn with_data_ticks(
        mut self,
        extent: (f64, f64),
        image: impl Fn(f64) -> f64,
        count: usize,
        format: Option<&super::TickFormat>,
    ) -> Self {
        let (lo, hi) = (extent.0.min(extent.1), extent.0.max(extent.1));
        let step = super::tick_step(lo, hi, count);
        let first = (lo / step).ceil() as i64;
        let last = (hi / step).floor() as i64;
        let mut values: Vec<f64> = (first..=last)
            .map(|i| super::step_value(i as f64, step))
            .collect();
        if values.is_empty() {
            values = vec![lo, hi];
        }
        self.ticks = values
            .into_iter()
            .filter_map(|value| {
                let img = image(value);
                img.is_finite().then(|| RampTick {
                    t: self.t(img),
                    value,
                    label: format
                        .map_or_else(|| super::format_with_step(value, step), |f| f.apply(value)),
                })
            })
            .collect();
        self
    }

    /// A ramp whose `t` is linear in the value over `(lo, hi)`, with
    /// about `count` ticks.
    pub(crate) fn linear(
        lut: Arc<[[u8; 4]]>,
        reverse: bool,
        (lo, hi): (f64, f64),
        count: usize,
    ) -> Self {
        Self::build(lut, reverse, [lo, (lo + hi) / 2.0, hi], count, false)
    }

    /// A ramp through `stops` (low, middle at `t = 0.5`, high), with about
    /// `count` ticks at round values: the middle, and multiples of one
    /// step on either side of it, so a diverging scale's zero is labelled.
    pub(crate) fn piecewise(
        lut: Arc<[[u8; 4]]>,
        reverse: bool,
        stops: [f64; 3],
        count: usize,
    ) -> Self {
        Self::build(lut, reverse, stops, count, true)
    }

    /// `mark_mid`: label the middle stop even when it is not a round value
    /// (a diverging scale's midpoint; a linear ramp's centre is not special).
    fn build(
        lut: Arc<[[u8; 4]]>,
        reverse: bool,
        stops: [f64; 3],
        count: usize,
        mark_mid: bool,
    ) -> Self {
        let mut ramp = Self {
            lut,
            reverse,
            stops,
            ticks: Vec::new(),
        };
        let [lo, mid, hi] = stops;
        let (lo, hi) = (lo.min(hi), lo.max(hi));
        let step = super::tick_step(lo, hi, count);
        let first = (lo / step).ceil() as i64;
        let last = (hi / step).floor() as i64;
        let mut values: Vec<f64> = (first..=last)
            .map(|i| super::step_value(i as f64, step))
            .collect();
        if values.is_empty() {
            values = vec![lo, hi];
        }
        if mark_mid
            && !values.iter().any(|v| (v - mid).abs() < step * 1e-9)
            && (lo..=hi).contains(&mid)
        {
            values.push(mid);
            values.sort_by(f64::total_cmp);
        }
        ramp.ticks = values
            .into_iter()
            .map(|value| RampTick {
                t: ramp.t(value),
                value,
                label: super::format_with_step(value, step),
            })
            .collect();
        ramp
    }
}

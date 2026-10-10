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
/// mirror and a legend.
pub trait ColorScale: ShaderFn<Out = Color> + EncodeFn<Output = Color> + CpuMirror {
    /// What a legend for this scale shows, from its current domain. Each
    /// colour is the mirror's, so it matches the GPU's.
    fn legend(&self) -> Legend;
}

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

    /// A ramp whose `t` is linear in the value over `(lo, hi)`, with
    /// about `count` ticks.
    pub(crate) fn linear(
        lut: Arc<[[u8; 4]]>,
        reverse: bool,
        (lo, hi): (f64, f64),
        count: usize,
    ) -> Self {
        Self::piecewise(lut, reverse, [lo, (lo + hi) / 2.0, hi], count)
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
        if !values.iter().any(|v| (v - mid).abs() < step * 1e-9) && (lo..=hi).contains(&mid) {
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

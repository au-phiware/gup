// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Ordinal position scales over dictionary codes (RFC-001 S5b): [`Band`]
//! gives each key a band of the range, [`Point`] a point. Both are one
//! WGSL function, `offset + step · code`, and take their domain, the
//! channel's keys in first-seen order, from [`ShaderFn::fit_keys`].

use super::{PositionScale, Ticks};
use crate::channel::Px;
use crate::column::{ColumnFormat, NULL_CODE};
use crate::encoding::{CpuMirror, ShaderFn};
use crate::error::{Error, Result};
use crate::shader::{SCALE_BAND, WgslModule};
use std::sync::Arc;

/// The keys, the range and the padding: what both ordinal scales share.
#[derive(Clone, Debug, PartialEq)]
struct Slots {
    keys: Vec<Arc<str>>,
    range: (f64, f64),
    /// Gap between bands, as a fraction of the step (1 for points).
    inner: f64,
    /// Space before the first and after the last band, in steps.
    outer: f64,
}

impl Slots {
    fn new(inner: f64, outer: f64) -> Self {
        Self {
            keys: Vec::new(),
            range: (0.0, 1.0),
            inner,
            outer,
        }
    }

    /// Distance between the starts of adjacent bands (negative for a
    /// reversed range, such as a y axis).
    fn step(&self) -> f64 {
        let n = self.keys.len() as f64;
        (self.range.1 - self.range.0) / (n - self.inner + 2.0 * self.outer).max(1.0)
    }

    /// Width of each band (0 for points).
    fn band_width(&self) -> f64 {
        self.step() * (1.0 - self.inner)
    }

    /// The centre of code 0's band; the rest are `step` apart. The bands
    /// sit in the middle of the range.
    fn offset(&self) -> f64 {
        let n = self.keys.len() as f64;
        let step = self.step();
        let used = step * (n - self.inner).max(0.0);
        self.range.0 + (self.range.1 - self.range.0 - used) / 2.0 + self.band_width() / 2.0
    }

    /// The centre of `code`'s band; NaN for a null or non-finite code.
    fn eval(&self, code: f64) -> f64 {
        if !code.is_finite() || code >= f64::from(NULL_CODE) {
            return f64::NAN;
        }
        self.offset() + self.step() * code
    }

    /// The code whose band centre is nearest `px`.
    fn invert(&self, px: f64) -> f64 {
        let step = self.step();
        let last = self.keys.len().saturating_sub(1) as f64;
        if step == 0.0 {
            return 0.0;
        }
        ((px - self.offset()) / step).round().clamp(0.0, last)
    }

    /// Every key's code and label, thinned to every `k`-th key when there
    /// are more than twice `count` (labels on a categorical axis are
    /// usually short; collision-aware placement is RFC-001 S7's).
    fn ticks(&self, count: usize) -> Ticks {
        let n = self.keys.len();
        let stride = n.div_ceil(2 * count.max(1)).max(1);
        let codes: Vec<usize> = (0..n).step_by(stride).collect();
        Ticks {
            values: codes.iter().map(|&c| c as f64).collect(),
            labels: codes.iter().map(|&c| self.keys[c].to_string()).collect(),
        }
    }

    fn params(&self) -> BandParams {
        BandParams {
            offset: self.offset() as f32,
            step: self.step() as f32,
            pad_a: 0,
            pad_b: 0,
        }
    }
}

fn check_padding(what: &str, p: f64, max: f64) {
    assert!(
        (0.0..=max).contains(&p),
        "{what} must be in 0..={max}, not {p}"
    );
}

/// A band position scale: each key of a dictionary-encoded channel gets an
/// equal band of the range, in first-seen order, with padding between and
/// around the bands. It maps a key to the **centre** of its band, so a
/// circle sits in the middle and a bar ([`band_width`](Self::band_width)
/// wide) is centred on it.
///
/// `padding_inner` is the gap between bands as a fraction of the step
/// (band plus gap; default 0.1), and `padding_outer` the space before the
/// first band and after the last, in steps (default 0.1). The axis has a
/// tick per key, labelled by the key.
///
/// Its domain is the channel's keys ([`ShaderFn::fit_keys`]); it cannot
/// be set to a numeric range, so zooming a plot with a band axis is an
/// error. A missing key ([`encode_nullable_key`](crate::EncodeFn::encode_nullable_key))
/// is not drawn: the glue compares the code with the reserved null code.
///
/// ```
/// use gup_core::prelude::*;
///
/// let mut band = Band::new().padding(0.25).range(Px(0.0), Px(325.0));
/// band.fit_keys(&["Mon".into(), "Tue".into(), "Wed".into()])?;
/// // Three bands, a step of 325 / (3 - 0.25 + 2 · 0.25) = 100 px apart, each
/// // 75 px wide, after 25 px of outer padding.
/// assert_eq!(band.step(), 100.0);
/// assert_eq!(band.band_width(), 75.0);
/// assert_eq!(band.eval(0.0), 25.0 + 37.5);
/// assert_eq!(band.ticks(10).labels, ["Mon", "Tue", "Wed"]);
/// # Ok::<(), gup_core::Error>(())
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct Band(Slots);

impl Default for Band {
    fn default() -> Self {
        Self::new()
    }
}

impl Band {
    /// A band scale with 0.1 inner and outer padding.
    pub fn new() -> Self {
        Self(Slots::new(0.1, 0.1))
    }

    /// Set both paddings (`padding_inner` must be in `0..=1`).
    pub fn padding(self, p: f64) -> Self {
        self.padding_inner(p).padding_outer(p)
    }

    /// The gap between bands, as a fraction of the step.
    ///
    /// # Panics
    ///
    /// Unless `0 <= p <= 1`.
    pub fn padding_inner(mut self, p: f64) -> Self {
        check_padding("Band::padding_inner", p, 1.0);
        self.0.inner = p;
        self
    }

    /// The space before the first band and after the last, in steps.
    ///
    /// # Panics
    ///
    /// If `p` is negative or not finite.
    pub fn padding_outer(mut self, p: f64) -> Self {
        check_padding("Band::padding_outer", p, f64::MAX);
        self.0.outer = p;
        self
    }

    /// Set the pixel range.
    pub fn range(mut self, r0: Px, r1: Px) -> Self {
        self.set_range(r0, r1);
        self
    }

    /// The domain: each key's label, in code order.
    pub fn keys(&self) -> &[Arc<str>] {
        &self.0.keys
    }

    /// The width of each band, in pixels (negative on a reversed range,
    /// such as a y axis).
    pub fn band_width(&self) -> f64 {
        self.0.band_width()
    }

    /// The distance between adjacent bands' centres, in pixels.
    pub fn step(&self) -> f64 {
        self.0.step()
    }
}

/// A point position scale: each key of a dictionary-encoded channel gets
/// a point, evenly spaced across the range in first-seen order, with
/// `padding` steps (default 0.5) before the first and after the last. A
/// [`Band`] with no width: for scatter-style categorical axes.
///
/// Its domain, ticks, zoom and null keys work as [`Band`]'s.
///
/// ```
/// use gup_core::prelude::*;
///
/// let mut point = Point::new().range(Px(0.0), Px(300.0));
/// point.fit_keys(&["low".into(), "mid".into(), "high".into()])?;
/// // Steps of 300 / (2 + 2 · 0.5) = 100 px, starting half a step in.
/// assert_eq!(point.step(), 100.0);
/// assert_eq!([point.eval(0.0), point.eval(1.0), point.eval(2.0)], [50.0, 150.0, 250.0]);
/// # Ok::<(), gup_core::Error>(())
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct Point(Slots);

impl Default for Point {
    fn default() -> Self {
        Self::new()
    }
}

impl Point {
    /// A point scale with 0.5 steps of padding at each end.
    pub fn new() -> Self {
        Self(Slots::new(1.0, 0.5))
    }

    /// The space before the first point and after the last, in steps.
    ///
    /// # Panics
    ///
    /// If `p` is negative or not finite.
    pub fn padding(mut self, p: f64) -> Self {
        check_padding("Point::padding", p, f64::MAX);
        self.0.outer = p;
        self
    }

    /// Set the pixel range.
    pub fn range(mut self, r0: Px, r1: Px) -> Self {
        self.set_range(r0, r1);
        self
    }

    /// The domain: each key's label, in code order.
    pub fn keys(&self) -> &[Arc<str>] {
        &self.0.keys
    }

    /// The distance between adjacent points, in pixels.
    pub fn step(&self) -> f64 {
        self.0.step()
    }
}

/// WGSL `gup::scale::band::Params`.
#[derive(Copy, Clone, Debug, PartialEq, encase::ShaderType)]
pub struct BandParams {
    offset: f32,
    step: f32,
    // To 16 bytes, as every uniform `Params` (see the WGSL module).
    pad_a: u32,
    pad_b: u32,
}

macro_rules! ordinal_scale {
    ($scale:ident) => {
        impl ShaderFn for $scale {
            type In = u32;
            type Out = Px;
            type Params = BandParams;
            const MODULE: &'static WgslModule = &SCALE_BAND;
            const ENTRY: &'static str = "map";

            fn params(&self) -> BandParams {
                self.0.params()
            }

            fn input_format(&self) -> ColumnFormat {
                ColumnFormat::U32
            }

            fn fit_keys(&mut self, keys: &[Arc<str>]) -> Result<()> {
                if self.0.keys != keys {
                    self.0.keys = keys.to_vec();
                }
                Ok(())
            }

            fn image(&self, extent: (f64, f64)) -> Option<(f64, f64)> {
                super::monotonic_image(|code| self.eval(code), extent)
            }
        }

        impl CpuMirror for $scale {
            /// The centre of `code`'s slot; NaN for a null code (whose row
            /// the GPU does not draw).
            fn eval(&self, code: f64) -> f64 {
                self.0.eval(code)
            }
        }

        impl PositionScale for $scale {
            /// Codes `0..len`, once the keys are known.
            fn current_domain(&self) -> Option<(f64, f64)> {
                let n = self.0.keys.len();
                (n > 0).then(|| (0.0, (n - 1) as f64))
            }

            /// Always: the domain is the channel's keys.
            fn is_auto(&self) -> bool {
                true
            }

            fn set_domain(&mut self, d0: f64, d1: f64) -> Result<()> {
                Err(Error::config(
                    concat!(stringify!($scale), " scale domain"),
                    format!(
                        "({d0}, {d1}): the domain of a {} scale is its channel's keys, not a \
                         numeric range, so it cannot be set or zoomed",
                        stringify!($scale)
                    ),
                ))
            }

            fn set_range(&mut self, r0: Px, r1: Px) {
                self.0.range = (f64::from(r0.0), f64::from(r1.0));
            }

            /// The code whose slot centre is nearest `px`.
            fn invert(&self, px: f64) -> f64 {
                self.0.invert(px)
            }

            /// Nothing to round: the keys are the domain.
            fn nice(&mut self) {}

            fn ticks(&self, count: usize) -> Ticks {
                self.0.ticks(count)
            }
        }
    };
}

ordinal_scale!(Band);
ordinal_scale!(Point);

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(n: usize) -> Vec<Arc<str>> {
        (0..n).map(|i| format!("k{i}").into()).collect()
    }

    /// Bands tile the range: outer padding, then `n` bands separated by
    /// inner gaps, all centred; a reversed range (a y axis) runs the
    /// other way.
    #[test]
    fn bands_tile_the_range_with_padding() {
        let mut b = Band::new()
            .padding_inner(0.2)
            .padding_outer(0.5)
            .range(Px(100.0), Px(500.0));
        b.fit_keys(&keys(4)).unwrap();
        // step = 400 / (4 - 0.2 + 1) = 83.33…; first band starts 0.5 step in.
        let step = 400.0 / 4.8;
        assert!((b.step() - step).abs() < 1e-12);
        assert!((b.band_width() - 0.8 * step).abs() < 1e-12);
        let first_start = 100.0 + 0.5 * step;
        assert!((b.eval(0.0) - (first_start + 0.4 * step)).abs() < 1e-9);
        let last_end = b.eval(3.0) + b.band_width() / 2.0;
        assert!((last_end - (500.0 - 0.5 * step)).abs() < 1e-9);
        assert!(b.eval(f64::from(NULL_CODE)).is_nan());
        assert_eq!(b.invert(b.eval(2.0) + 10.0), 2.0);
        assert_eq!(b.invert(-1e6), 0.0);

        let mut y = b.clone().range(Px(500.0), Px(100.0));
        y.fit_keys(&keys(4)).unwrap();
        assert!((y.eval(0.0) - (600.0 - b.eval(0.0))).abs() < 1e-9);
        assert!(y.band_width() < 0.0);
    }

    #[test]
    fn points_have_no_width_and_one_key_sits_in_the_middle() {
        let mut p = Point::new().range(Px(0.0), Px(200.0));
        p.fit_keys(&keys(1)).unwrap();
        assert_eq!(p.eval(0.0), 100.0);
        p.fit_keys(&keys(5)).unwrap();
        assert_eq!(p.step(), 40.0);
        assert_eq!(p.eval(4.0), 180.0);
        let edge = Point::new().padding(0.0).range(Px(0.0), Px(200.0));
        let mut edge = edge;
        edge.fit_keys(&keys(5)).unwrap();
        assert_eq!((edge.eval(0.0), edge.eval(4.0)), (0.0, 200.0));
    }

    /// A tick per key, labelled by the key; more than twice `count` keys
    /// are thinned evenly. The domain cannot be set (so not zoomed).
    #[test]
    fn ticks_are_keys_and_the_domain_is_not_numeric() {
        let mut b = Band::new();
        b.fit_keys(&keys(5)).unwrap();
        let t = b.ticks(3);
        assert_eq!(t.values, [0.0, 1.0, 2.0, 3.0, 4.0]);
        assert_eq!(t.labels, ["k0", "k1", "k2", "k3", "k4"]);
        b.fit_keys(&keys(30)).unwrap();
        let t = b.ticks(5);
        assert_eq!(t.values.len(), 10, "{:?}", t.labels);
        assert_eq!(t.labels[1], "k3");
        assert_eq!(b.current_domain(), Some((0.0, 29.0)));
        let err = b.set_domain(0.0, 3.0).unwrap_err().to_string();
        assert!(
            err.contains("domain of a Band scale is its channel's keys"),
            "{err}"
        );
        assert_eq!(Band::new().current_domain(), None);
    }
}

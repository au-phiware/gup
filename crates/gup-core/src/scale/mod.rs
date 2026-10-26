// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! One scale family (RFC-001 §5): every scale is a [`ShaderFn`] with an
//! exact f64 [`CpuMirror`], so axes, ticks and the GPU always agree.
//!
//! - **Position** ([`PositionScale`]): [`Linear`], [`Pow`] (and
//!   [`Pow::sqrt`]), [`Log`], [`Symlog`] and [`Time`] over numbers;
//!   [`Band`] and [`Point`] over dictionary codes (categorical axes).
//! - **Colour** ([`ColorScale`], each with a [`Legend`]): [`Sequential`]
//!   and [`Diverging`] over numbers (palette LUTs); [`Categorical`] over
//!   dictionary codes.
//!
//! Any scale with a numeric output can feed another through
//! [`then`](crate::EncodeFn::then). Scales over dictionary codes take
//! their domain from the channel's keys ([`ShaderFn::fit_keys`]).

mod band;
mod categorical;
mod diverging;
mod legend;
mod linear;
mod log;
mod offset;
mod pow;
pub(crate) mod sequential;
mod symlog;
mod time;

#[cfg(test)]
pub(crate) mod conformance;

pub use band::{Band, BandParams, Point};
pub use categorical::OKABE_ITO_DARK;
pub use categorical::{
    CATEGORICAL_COLORS, Categorical, CategoricalParams, MAX_PALETTE, NULL_COLOR, OKABE_ITO,
};
pub use diverging::{Diverging, DivergingParams};
pub use legend::{ColorScale, Legend, Ramp, RampTick, Swatch};
pub use linear::{Linear, LinearParams};
pub use log::{Log, LogParams};
pub use offset::{Offset, OffsetParams};
pub use pow::{Pow, PowParams};
pub use sequential::{Sequential, SequentialParams};
pub use symlog::{Symlog, SymlogParams};
pub use time::Time;

/// A continuous colour scale for a builder's `color_value`: a
/// [`Sequential`] ramp, or a [`Diverging`] one around a midpoint.
#[derive(Clone, Debug)]
pub enum ColorValueScale {
    /// A ramp from low to high.
    Sequential(Sequential),
    /// A ramp that diverges from a midpoint.
    Diverging(Diverging),
}

impl From<Sequential> for ColorValueScale {
    fn from(scale: Sequential) -> Self {
        Self::Sequential(scale)
    }
}

impl From<Diverging> for ColorValueScale {
    fn from(scale: Diverging) -> Self {
        Self::Diverging(scale)
    }
}

use crate::channel::Px;
use crate::column::ColumnFormat;
use crate::encoding::{CpuMirror, DictionarySlot, EncodeFn, Resource, ShaderFn};
use crate::error::Result;
use crate::shader::WgslModule;
use crate::theme::Theme;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard};

/// Tick positions (domain values) and their labels.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Ticks {
    /// Tick values in domain units, ascending.
    pub values: Vec<f64>,
    /// One label per value.
    pub labels: Vec<String>,
    /// One flag per value: a major tick is drawn heavier (a `Time` tick that
    /// begins a year while the tick interval is shorter than one). Every
    /// other scale marks none.
    pub major: Vec<bool>,
}

impl Ticks {
    /// Ticks with no major ones.
    pub fn new(values: Vec<f64>, labels: Vec<String>) -> Self {
        let major = vec![false; values.len()];
        Self {
            values,
            labels,
            major,
        }
    }

    /// The weight tick `i`'s label is drawn in.
    pub(crate) fn weight(&self, i: usize) -> gup_text::Weight {
        if self.major.get(i).copied().unwrap_or(false) {
            gup_text::Weight::SemiBold
        } else {
            gup_text::Weight::Regular
        }
    }
}

/// A scale from a domain to a pixel range: a numeric domain (`In = f32`:
/// [`Linear`], [`Log`], …), or dictionary codes (`In = u32`: [`Band`],
/// [`Point`]), whose domain values are the codes `0..len` and whose ticks
/// are labelled by the keys.
pub trait PositionScale: ShaderFn<Out = Px> + EncodeFn<Output = Px> + CpuMirror {
    /// The domain, if set or fitted.
    fn current_domain(&self) -> Option<(f64, f64)>;
    /// Whether the domain is data-driven (not set explicitly).
    fn is_auto(&self) -> bool;
    /// Set an explicit domain, e.g. to zoom or pan. Only uniforms derived
    /// from the domain change; no column bytes are re-uploaded.
    fn set_domain(&mut self, d0: f64, d1: f64) -> Result<()>;
    /// Make the domain data-driven again ([`is_auto`](Self::is_auto) is
    /// true); the next resolve refits it. No-op for [`Band`] and [`Point`],
    /// which are always automatic.
    fn clear_domain(&mut self);
    /// Set the pixel range the domain maps onto.
    fn set_range(&mut self, r0: Px, r1: Px);
    /// The domain value drawn at pixel `px`: the inverse of
    /// [`CpuMirror::eval`].
    fn invert(&self, px: f64) -> f64;
    /// Extend the domain to round values.
    fn nice(&mut self);
    /// Extend the domain to multiples of the step `ticks(count)` uses, so
    /// both ends are ticks. Defaults to [`nice`](Self::nice).
    fn nice_to(&mut self, _count: usize) {
        self.nice();
    }
    /// About `count` ticks inside the domain.
    fn ticks(&self, count: usize) -> Ticks;
    /// The code of the key labelled `key` in an ordinal scale's domain
    /// ([`Band`], [`Point`]); `None` for an unknown key and for every
    /// numeric scale (the default). How a text layer places a label by key.
    #[doc(hidden)]
    fn key_code(&self, key: &str) -> Option<f64> {
        let _ = key;
        None
    }
}

/// A shared handle to a scale. The plot keeps one clone to draw axes and
/// set ranges; the layer's encoding keeps another to write uniforms, so
/// both always see the same domain and range.
#[derive(Debug, Default)]
pub struct ScaleRef<S>(Arc<Shared<S>>);

/// What clones of a [`ScaleRef`] share.
#[derive(Debug, Default)]
struct Shared<S> {
    scale: RwLock<S>,
    /// The union of the extents fitted since [`ShaderFn::begin_fit`].
    fit: Mutex<Option<(f64, f64)>>,
    /// The key dictionary of the layers that encode keys through the scale.
    keys: DictionarySlot,
    /// Bumped by every [`ScaleRef::write`], so a cache can tell that the
    /// scale may have changed.
    revision: AtomicU64,
    /// The user's tick label format ([`ScaleRef::tick_format`]).
    format: RwLock<Option<TickFormat>>,
}

/// A user's tick label format: shared by the clones of a [`ScaleRef`],
/// applied to every tick value the scale labels.
#[derive(Clone)]
pub struct TickFormat(Arc<dyn Fn(f64) -> String + Send + Sync>);

impl TickFormat {
    pub(crate) fn new(f: impl Fn(f64) -> String + Send + Sync + 'static) -> Self {
        Self(Arc::new(f))
    }

    /// The label of `value`.
    pub fn apply(&self, value: f64) -> String {
        (self.0)(value)
    }
}

impl std::fmt::Debug for TickFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("TickFormat(..)")
    }
}

/// The position scales whose tick labels a user may format
/// ([`ScaleRef::tick_format`]): those labelled from numbers. [`Band`] and
/// [`Point`] are labelled from keys.
pub trait NumericTicks: PositionScale {}
impl NumericTicks for Linear {}
impl NumericTicks for Pow {}
impl NumericTicks for Log {}
impl NumericTicks for Symlog {}
impl NumericTicks for Time {}

impl<S> Clone for ScaleRef<S> {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

impl<S> ScaleRef<S> {
    /// Share `scale`.
    pub fn new(scale: S) -> Self {
        Self(Arc::new(Shared {
            scale: RwLock::new(scale),
            fit: Mutex::new(None),
            keys: DictionarySlot::default(),
            revision: AtomicU64::new(0),
            format: RwLock::new(None),
        }))
    }

    /// Read the scale.
    pub fn read(&self) -> RwLockReadGuard<'_, S> {
        self.0.scale.read().unwrap_or_else(|p| p.into_inner())
    }

    /// Modify the scale.
    pub fn write(&self) -> RwLockWriteGuard<'_, S> {
        self.0.revision.fetch_add(1, Ordering::Relaxed);
        self.0.scale.write().unwrap_or_else(|p| p.into_inner())
    }
}

impl<S: NumericTicks> ScaleRef<S> {
    /// Label this scale's ticks with `f` instead of the default labels:
    /// axis labels and, when the scale feeds a colour legend, the legend's
    /// ticks. `f` receives the tick value in the scale's domain units
    /// (for [`Time`], Unix seconds as `f64`) and its string is used as is,
    /// also to measure and thin labels. Clones of the `ScaleRef` share the
    /// format; setting it counts as a write, so caches rebuild.
    ///
    /// ```
    /// use gup_core::prelude::*;
    /// let x = ScaleRef::new(Linear::new().domain(0.0, 3.0e6));
    /// x.tick_format(|v| format!("${:.1}M", v / 1e6));
    /// assert_eq!(x.tick_labels(3)[1], "$1.0M");
    /// ```
    ///
    /// Band and point scales are labelled from their keys:
    ///
    /// ```compile_fail
    /// use gup_core::prelude::*;
    /// let x = ScaleRef::new(Band::new());
    /// x.tick_format(|v| v.to_string());
    /// ```
    pub fn tick_format(&self, f: impl Fn(f64) -> String + Send + Sync + 'static) -> &Self {
        *self.0.format.write().unwrap_or_else(|p| p.into_inner()) = Some(TickFormat::new(f));
        self.0.revision.fetch_add(1, Ordering::Relaxed);
        self
    }
}

impl<S: PositionScale> ScaleRef<S> {
    /// The labels of about `count` ticks, formatted as set by
    /// [`tick_format`](Self::tick_format).
    pub fn tick_labels(&self, count: usize) -> Vec<String> {
        DynPositionScale::ticks(self, count).labels
    }
}

impl<S: ShaderFn> ShaderFn for ScaleRef<S> {
    type In = S::In;
    type Out = S::Out;
    type Params = S::Params;
    const MODULE: &'static WgslModule = S::MODULE;
    const ENTRY: &'static str = S::ENTRY;

    fn params(&self) -> Self::Params {
        self.read().params()
    }

    fn input_format(&self) -> ColumnFormat {
        self.read().input_format()
    }

    fn chunk_base(&self, origin: f64) -> f64 {
        self.read().chunk_base(origin)
    }

    fn resources(&self) -> Vec<Resource> {
        self.read().resources()
    }

    fn dictionary_slot(&self) -> Option<&DictionarySlot> {
        Some(&self.0.keys)
    }

    fn legend(&self) -> Option<Legend> {
        ShaderFn::legend(&*self.read())
    }

    fn revision(&self) -> u64 {
        self.0.revision.load(Ordering::Relaxed)
    }

    fn tick_format(&self) -> Option<TickFormat> {
        self.0
            .format
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    fn begin_fit(&self) {
        *self.0.fit.lock().unwrap_or_else(|p| p.into_inner()) = None;
    }

    /// Unions `extent` into the pass's extent (see
    /// [`begin_fit`](ShaderFn::begin_fit)) before fitting the scale.
    fn fit_domain(&mut self, extent: (f64, f64)) -> Result<()> {
        if !(extent.0.is_finite() && extent.1.is_finite()) {
            // Let the scale report it; a union would hide it.
            return self.write().fit_domain(extent);
        }
        let union = {
            let mut fit = self.0.fit.lock().unwrap_or_else(|p| p.into_inner());
            let union = fit.map_or(extent, |(lo, hi)| (lo.min(extent.0), hi.max(extent.1)));
            *fit = Some(union);
            union
        };
        self.write().fit_domain(union)
    }

    fn fit_keys(&mut self, keys: &[Arc<str>]) -> Result<()> {
        self.write().fit_keys(keys)
    }

    fn apply_theme(&mut self, theme: &Theme) {
        self.write().apply_theme(theme);
    }

    fn image(&self, extent: (f64, f64)) -> Option<(f64, f64)> {
        self.read().image(extent)
    }

    fn cpu_color(&self, x: f64) -> Option<crate::Color> {
        self.read().cpu_color(x)
    }
}

impl<S> CpuMirror for ScaleRef<S>
where
    S: ShaderFn + CpuMirror<Output = <S as ShaderFn>::Out>,
{
    fn eval(&self, x: f64) -> <S::Out as crate::channel::GpuType>::Cpu {
        self.read().eval(x)
    }
}

impl<S> ColorScale for ScaleRef<S> where S: ColorScale + CpuMirror<Output = <S as ShaderFn>::Out> {}

/// Object-safe position-scale operations the plot needs for its x and y
/// slots.
pub(crate) trait DynPositionScale: Send + Sync {
    /// Counts writes to the scale (see [`ScaleRef::write`]).
    fn revision(&self) -> u64;
    fn is_auto(&self) -> bool;
    fn nice(&self);
    fn nice_to(&self, count: usize);
    fn set_range(&self, r0: Px, r1: Px);
    fn ticks(&self, count: usize) -> Ticks;
    fn eval(&self, x: f64) -> f64;
    fn current_domain(&self) -> Option<(f64, f64)>;
    fn set_domain(&self, d0: f64, d1: f64) -> Result<()>;
    fn clear_domain(&self);
    fn invert(&self, px: f64) -> f64;
    /// Union the data value drawn at pixel `px` into the automatic domain
    /// being fitted (a constant or derived position, e.g. a bar baseline).
    fn fit_position(&self, px: f64);
    /// Identifies the scale among its clones (they share state).
    fn identity(&self) -> usize;
    /// Grow an automatic, already rounded domain, in data units, so that
    /// its near end maps `near` pixels and its far end `far` pixels inside
    /// the pixel interval `a..b`. Returns whether the domain changed; an explicit domain and an
    /// ordinal scale never change. Ticks stay on the rounded values.
    fn widen(&self, a: f32, b: f32, near: f32, far: f32) -> bool;
    /// A clone of the scale as an encoding function; it shares state with
    /// the plot's scale (GUP-447).
    fn dyn_encode(&self) -> Box<dyn crate::encoding::DynEncodeFn>;
    /// Whether the scale reads dictionary keys (`Band`, `Point`).
    fn reads_keys(&self) -> bool;
    /// The pixel centre of `key`'s band (or point), for a scale that
    /// [`reads_keys`](Self::reads_keys); `None` for a numeric scale or a
    /// key not in the domain.
    fn key_position(&self, key: &str) -> Option<f64>;
    /// Label the scale's ticks with `format`: the shared slot of
    /// [`ScaleRef::tick_format`], and a write for the caches.
    fn set_tick_format(&self, format: TickFormat);

    /// Zoom by `factor` about pixel `anchor`: the domain becomes the values
    /// now drawn at the range ends pulled towards (`factor < 1`) or pushed
    /// away from (`factor > 1`) the anchor. Working in pixels and inverting
    /// makes this right for every scale (a log scale zooms in log space).
    fn zoom(&self, anchor: f64, factor: f64) -> Result<()> {
        let Some((d0, d1)) = self.current_domain() else {
            return Ok(());
        };
        let (r0, r1) = (self.eval(d0), self.eval(d1));
        let at = |r: f64| self.invert(anchor + (r - anchor) * factor);
        self.set_domain(at(r0), at(r1))
    }

    /// Move the domain so the content moves `dpx` pixels along the range:
    /// the domain becomes the values now drawn `dpx` pixels before the range
    /// ends. In pixels, like [`zoom`](Self::zoom), so log, pow and time
    /// scales pan in their own space.
    fn pan(&self, dpx: f64) -> Result<()> {
        let Some((d0, d1)) = self.current_domain() else {
            return Ok(());
        };
        let (r0, r1) = (self.eval(d0), self.eval(d1));
        self.set_domain(self.invert(r0 - dpx), self.invert(r1 - dpx))
    }
}

impl<S: PositionScale> DynPositionScale for ScaleRef<S> {
    fn revision(&self) -> u64 {
        self.0.revision.load(Ordering::Relaxed)
    }

    fn dyn_encode(&self) -> Box<dyn crate::encoding::DynEncodeFn> {
        Box::new(self.clone())
    }

    fn set_tick_format(&self, format: TickFormat) {
        *self.0.format.write().unwrap_or_else(|p| p.into_inner()) = Some(format);
        self.0.revision.fetch_add(1, Ordering::Relaxed);
    }

    fn reads_keys(&self) -> bool {
        EncodeFn::links(self)
            .first()
            .is_some_and(|l| l.input_format() == crate::column::ColumnFormat::U32)
    }

    fn key_position(&self, key: &str) -> Option<f64> {
        let scale = self.read();
        scale.key_code(key).map(|code| scale.eval(code))
    }

    fn is_auto(&self) -> bool {
        self.read().is_auto()
    }

    fn nice(&self) {
        self.write().nice();
    }

    fn nice_to(&self, count: usize) {
        self.write().nice_to(count);
    }

    fn set_range(&self, r0: Px, r1: Px) {
        self.write().set_range(r0, r1);
    }

    fn ticks(&self, count: usize) -> Ticks {
        let mut ticks = self.read().ticks(count);
        if let Some(format) = ShaderFn::tick_format(self) {
            ticks.labels = ticks.values.iter().map(|&v| format.apply(v)).collect();
        }
        ticks
    }

    fn eval(&self, x: f64) -> f64 {
        self.read().eval(x)
    }

    fn current_domain(&self) -> Option<(f64, f64)> {
        self.read().current_domain()
    }

    fn set_domain(&self, d0: f64, d1: f64) -> Result<()> {
        self.write().set_domain(d0, d1)
    }

    fn clear_domain(&self) {
        self.write().clear_domain();
    }

    fn invert(&self, px: f64) -> f64 {
        self.read().invert(px)
    }

    fn identity(&self) -> usize {
        std::ptr::from_ref(&self.0.keys) as usize
    }

    fn fit_position(&self, px: f64) {
        if !self.read().is_auto() || self.reads_keys() {
            return;
        }
        let (value, span) = {
            let scale = self.read();
            let span = scale.current_domain().map(|(a, b)| (b - a).abs());
            (scale.invert(px), span)
        };
        if !value.is_finite() {
            return;
        }
        // A pixel is an `f32`; snap the value back so that a baseline of
        // zero is zero, not a rounding error that `nice` would step past.
        let value = match span.filter(|s| s.is_finite() && *s > 0.0) {
            Some(span) => (value / (span * 1e-5)).round() * (span * 1e-5),
            None => value,
        };
        let mut this = self.clone();
        let _ = ShaderFn::fit_domain(&mut this, (value, value));
    }

    fn widen(&self, a: f32, b: f32, near: f32, far: f32) -> bool {
        if (near <= 0.0 && far <= 0.0) || !self.read().is_auto() {
            return false;
        }
        let sign = if b >= a { 1.0 } else { -1.0 };
        let mut scale = self.write();
        let Some(extent) = scale.current_domain() else {
            return false;
        };
        // Map the rounded domain `near` and `far` pixels inside the pixel interval,
        // then read off what its ends show; a scale that cannot take the
        // domain keeps it.
        scale.set_range(Px(a + sign * near), Px(b - sign * far));
        let (lo, hi) = (scale.invert(f64::from(a)), scale.invert(f64::from(b)));
        if !(lo.is_finite() && hi.is_finite()) || (lo == extent.0 && hi == extent.1) {
            return false;
        }
        scale.fit_domain((lo, hi)).is_ok()
    }
}

/// The image of `extent` under the monotonic `f`, ordered; `None` if
/// either end maps to a non-finite value.
pub(crate) fn monotonic_image(f: impl Fn(f64) -> f64, extent: (f64, f64)) -> Option<(f64, f64)> {
    let (a, b) = (f(extent.0), f(extent.1));
    (a.is_finite() && b.is_finite()).then(|| (a.min(b), a.max(b)))
}

/// A 1–2–5 step close to `(hi - lo) / count`.
pub(crate) fn tick_step(lo: f64, hi: f64, count: usize) -> f64 {
    let raw = (hi - lo).abs() / count.max(1) as f64;
    if !raw.is_finite() || raw <= 0.0 {
        return 1.0;
    }
    let power = 10f64.powf(raw.log10().floor());
    let err = raw / power;
    let mult = if err >= 50f64.sqrt() {
        10.0
    } else if err >= 10f64.sqrt() {
        5.0
    } else if err >= 2f64.sqrt() {
        2.0
    } else {
        1.0
    };
    mult * power
}

/// Multiply `i` by `step` without accumulating error for fractional steps.
pub(crate) fn step_value(i: f64, step: f64) -> f64 {
    if step < 1.0 {
        let inv = (1.0 / step).round();
        if (inv * step - 1.0).abs() < 1e-9 {
            return i / inv;
        }
    }
    i * step
}

/// The minus sign tick labels use: U+2212 MINUS SIGN, as wide as a digit
/// in the bundled font (tabular figures), not the shorter hyphen-minus.
pub(crate) const MINUS: char = '\u{2212}';

/// `label` with every hyphen-minus (a sign or an exponent's sign) set as
/// [`MINUS`].
pub(crate) fn typographic(label: String) -> String {
    if label.contains('-') {
        label.replace('-', "\u{2212}")
    } else {
        label
    }
}

/// Whether a tick set is labelled with thousands separators: its largest
/// absolute value is at least 10,000 (so years stay `2019`).
pub(crate) fn groups_thousands(values: &[f64]) -> bool {
    values.iter().any(|v| v.abs() >= 10_000.0)
}

/// `digits` (ASCII, no sign) with `,` between groups of three integer
/// digits; any fraction is left alone.
fn group_digits(digits: &str) -> String {
    let (int, frac) = digits
        .split_once('.')
        .map_or((digits, None), |(i, f)| (i, Some(f)));
    let mut out = String::with_capacity(digits.len() + int.len() / 3);
    for (n, c) in int.chars().enumerate() {
        if n > 0 && (int.len() - n) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    if let Some(f) = frac {
        out.push('.');
        out.push_str(f);
    }
    out
}

/// Format `v` with as many decimals as `step` needs (no `−0`), with a
/// typographic minus sign, and `,` between groups of three integer digits
/// when `grouped` (see [`groups_thousands`]).
pub(crate) fn format_with_step(v: f64, step: f64, grouped: bool) -> String {
    let decimals = if step >= 1.0 {
        0
    } else {
        (-step.log10().floor()) as usize
    };
    let s = format!("{v:.decimals$}");
    let unsigned = s.trim_start_matches('-');
    if unsigned.chars().all(|c| c == '0' || c == '.') {
        return unsigned.to_string();
    }
    let body = if grouped {
        let g = group_digits(unsigned);
        if s.starts_with('-') {
            format!("-{g}")
        } else {
            g
        }
    } else {
        s
    };
    typographic(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// GUP-453: `clear_domain` makes every position scale automatic again
    /// (`Band` and `Point` always are).
    #[test]
    fn clear_domain_makes_the_scale_automatic() {
        fn check<S: PositionScale>(mut s: S, d: (f64, f64)) {
            if s.set_domain(d.0, d.1).is_ok() {
                assert!(!s.is_auto());
            }
            s.clear_domain();
            assert!(s.is_auto());
        }
        check(Linear::new(), (1.0, 2.0));
        check(Log::new(), (1.0, 100.0));
        check(Pow::sqrt(), (1.0, 4.0));
        check(Symlog::new(), (-3.0, 5.0));
        check(Time::new(), (0.0, 1e9));
        check(Band::new(), (0.0, 3.0));
        check(Point::new(), (0.0, 3.0));
    }

    /// GUP-542: `key_position` is the centre of a key's band or point, by
    /// label, and `None` for an unknown key or a numeric scale.
    #[test]
    fn key_position_is_the_centre_of_a_keys_slot() {
        let keys: Vec<Arc<str>> = ["Mon", "Tue", "Wed"].map(Arc::from).to_vec();
        let mut band = Band::new().padding(0.25).range(Px(0.0), Px(325.0));
        ShaderFn::fit_keys(&mut band, &keys).unwrap();
        let band = ScaleRef::new(band);
        // A step of 100 px after 25 px of outer padding; bands 75 px wide.
        assert_eq!(band.key_position("Mon"), Some(62.5));
        assert_eq!(band.key_position("Wed"), Some(262.5));
        assert_eq!(band.key_position("Thu"), None);

        let mut point = Point::new().range(Px(0.0), Px(300.0));
        ShaderFn::fit_keys(&mut point, &keys).unwrap();
        let point = ScaleRef::new(point);
        assert_eq!(point.key_position("Mon"), Some(50.0));
        assert_eq!(point.key_position("Tue"), Some(150.0));
        assert_eq!(point.key_position(""), None);

        let lin = ScaleRef::new(Linear::new().domain(0.0, 1.0));
        assert_eq!(lin.key_position("Mon"), None);
    }

    /// Panning moves the domain by whole pixels in the scale's own space.
    #[test]
    fn pan_moves_the_domain_by_pixels() {
        let lin = ScaleRef::new(Linear::new().domain(0.0, 100.0).range(Px(0.0), Px(200.0)));
        lin.pan(20.0).unwrap();
        let (a, b) = lin.current_domain().unwrap();
        assert!(
            (a + 10.0).abs() < 1e-12 && (b - 90.0).abs() < 1e-12,
            "{a} {b}"
        );
        let log = ScaleRef::new(Log::new().domain(1.0, 100.0).range(Px(0.0), Px(200.0)));
        log.pan(-100.0).unwrap();
        let (a, b) = log.current_domain().unwrap();
        assert!(
            (a - 10.0).abs() < 1e-9 && (b - 1000.0).abs() < 1e-7,
            "{a} {b}"
        );
    }

    /// GUP-424: fits in one pass union; a new pass starts from scratch; an
    /// explicit domain ignores fits.
    #[test]
    fn fit_passes_union_domains() {
        let s = ScaleRef::new(Linear::new());
        let mut a = s.clone();
        let mut b = s.clone();
        s.begin_fit();
        a.fit_domain((0.0, 10.0)).unwrap();
        b.fit_domain((5.0, 30.0)).unwrap();
        assert_eq!(s.read().current_domain(), Some((0.0, 30.0)));
        s.begin_fit();
        a.fit_domain((2.0, 3.0)).unwrap();
        assert_eq!(s.read().current_domain(), Some((2.0, 3.0)));

        let mut fixed = ScaleRef::new(Linear::new().domain(1.0, 2.0));
        fixed.begin_fit();
        fixed.fit_domain((0.0, 10.0)).unwrap();
        assert_eq!(fixed.read().current_domain(), Some((1.0, 2.0)));
    }

    #[test]
    fn steps_are_one_two_five() {
        assert_eq!(tick_step(0.0, 10.0, 5), 2.0);
        assert_eq!(tick_step(0.0, 100.0, 10), 10.0);
        assert_eq!(tick_step(0.0, 1.0, 4), 0.2);
        assert_eq!(tick_step(0.0, 47.0, 5), 10.0);
    }

    #[test]
    fn fractional_steps_do_not_accumulate_error() {
        assert_eq!(step_value(3.0, 0.1), 0.3);
        assert_eq!(format_with_step(0.30000000000000004, 0.1, false), "0.3");
        assert_eq!(format_with_step(-0.0, 0.5, false), "0.0");
        assert_eq!(format_with_step(2500.0, 500.0, false), "2500");
    }

    #[test]
    fn thousands_group_from_ten_thousand() {
        assert_eq!(format_with_step(12345.0, 5.0, true), "12,345");
        assert_eq!(format_with_step(-1234567.0, 1.0, true), "\u{2212}1,234,567");
        assert_eq!(format_with_step(2500.0, 500.0, true), "2,500");
        assert_eq!(format_with_step(10000.5, 0.5, true), "10,000.5");
        assert_eq!(format_with_step(0.0, 5000.0, true), "0");
        assert_eq!(format_with_step(2019.0, 1.0, false), "2019");
        assert!(groups_thousands(&[0.0, 10_000.0]));
        assert!(groups_thousands(&[-20_000.0, 0.0]));
        assert!(!groups_thousands(&[0.0, 9_999.0]));
    }

    /// GUP-451: the bundled SemiBold subset is ASCII plus a few spaces, and
    /// the major `Time` tick labels (years) are the only SemiBold text the
    /// library draws, so every one must map to a real glyph in it.
    #[test]
    fn major_tick_labels_fit_the_semibold_subset() {
        let font = gup_text::Font::inter_semibold();
        let year = |y: f64| (y - 1970.0) * 365.2425 * 86_400.0;
        let mut majors = 0;
        for start in [1850.0, 1970.0] {
            for span_years in [1.0 / 12.0, 1.0, 10.0, 200.0] {
                for count in [2, 5, 10, 20] {
                    let t0 = year(start);
                    let ticks = Time::new()
                        .domain(t0, t0 + span_years * 365.2425 * 86_400.0)
                        .ticks(count);
                    for (label, _) in ticks
                        .labels
                        .iter()
                        .zip(&ticks.major)
                        .filter(|(_, major)| **major)
                    {
                        majors += 1;
                        let missing: Vec<char> =
                            label.chars().filter(|&c| !font.has_own_glyph(c)).collect();
                        assert!(
                            missing.is_empty(),
                            "{label:?} (start {start}, span {span_years} y) needs {missing:?}"
                        );
                    }
                }
            }
        }
        assert!(majors > 0, "no major ticks were generated");
    }

    /// GUP-407: the bundled font is a subset, so every character a tick
    /// formatter can emit must be in it. Sweeps linear and log domains
    /// from 1e-12 to 1e15, positive, negative and straddling zero.
    #[test]
    fn tick_labels_are_covered_by_the_bundled_font() {
        let mut emitted = std::collections::BTreeSet::new();
        let mut collect = |ticks: Ticks| {
            assert!(!ticks.labels.is_empty());
            emitted.extend(ticks.labels.iter().flat_map(|l| l.chars()));
        };
        for exp in -12..=15 {
            let m = 10f64.powi(exp);
            for (d0, d1) in [(0.0, m), (-m, m), (-3.7 * m, -1.2 * m), (1.5 * m, 1.75 * m)] {
                for count in [2, 5, 10] {
                    collect(Linear::new().domain(d0, d1).ticks(count));
                }
            }
            if exp < 15 {
                for count in [3, 10, 30] {
                    collect(Log::new().domain(m, m * 1000.0).ticks(count));
                    collect(Log::new().domain(m, m * 3.0).ticks(count));
                }
            }
            for count in [2, 5, 10] {
                collect(
                    Symlog::new()
                        .constant(m)
                        .domain(-m * 1e3, m * 1e4)
                        .ticks(count),
                );
                collect(Pow::sqrt().domain(0.0, m).ticks(count));
            }
        }
        // Time: spans from a tenth of a millisecond to a century, at the
        // epoch, around 1.7e9 and before 1970.
        for exp in -4..=10 {
            let span = 10f64.powi(exp);
            for t0 in [0.0, 1.7e9 - span / 3.0, -2.2e9] {
                for count in [2, 5, 10] {
                    collect(Time::new().domain(t0, t0 + span).ticks(count));
                }
            }
        }
        // Digits, sign, point, exponent, every SI suffix and the calendar
        // labels' colon were reached (month names are checked below).
        emitted.extend("JanFebMarAprMayJunJulAugSepOctNovDec".chars());
        for c in "0123456789−.ekMBT:".chars() {
            assert!(
                emitted.contains(&c),
                "the sweep never emitted {c:?}: {emitted:?}"
            );
        }
        let font = gup_text::Font::inter();
        let missing: Vec<char> = emitted
            .iter()
            .copied()
            .filter(|&c| !font.has_glyph(c))
            .collect();
        assert!(missing.is_empty(), "the bundled font lacks {missing:?}");
    }
}

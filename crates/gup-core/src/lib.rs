// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! # gup-core
//!
//! Gup's new core, built to [RFC-001](https://github.com/au-phiware/gup/blob/main/docs/planning/rfcs/RFC-001_Core_Architecture.md):
//!
//! - one cloneable [`Context`] that owns or wraps a wgpu device;
//! - a GPU [column store](mod@column) — accessors run once, columns live on
//!   the GPU as instance-rate vertex buffers, relative to a per-chunk f64
//!   origin; keys (strings, integers, enums) are dictionary-encoded
//!   ([`EncodeFn::encode_key`], [`EncodeFn::encode_owned_key`]), nulls are
//!   validity bits or a reserved code (a null position is not drawn, a
//!   null colour is [`NULL_COLOR`]), and [`Retain`] says how much CPU data
//!   to keep after upload;
//! - typed [channels](channel): `Circle::RADIUS: Channel<Circle, Px>`, so
//!   wrong value types, unknown channels and wrong marks don't compile;
//! - one [scale] family, each a [`ShaderFn`] with an f64
//!   [`CpuMirror`] — zooming changes uniforms only;
//! - shader composition at build time: naga_oil flattens the WGSL library
//!   once, in `build.rs`, and a typed glue emitter's output is linked to it
//!   at run time, so neither naga nor naga_oil is in the binary on any
//!   target — no find-and-replace on authored WGSL;
//! - a resolved [`Scene`] drawn by a [`Renderer`] into any
//!   [`RenderTarget`] ([`ImageTarget`], [`TextureTarget`], a window, or a
//!   host's own pass), or written as SVG by [`SvgTarget`] (guides only).
//!
//! The scale family is complete (RFC-001 S5a, S5b): [`Linear`], [`Pow`],
//! [`Log`], [`Symlog`] and [`Time`] for numbers; [`Band`] and [`Point`]
//! for keys; [`Sequential`], [`Diverging`] and [`Categorical`] for colour,
//! each with a [`Legend`]. There is still one mark ([`Circle`]) and one
//! layer per plot, and guides are axes only (legends are placed by hand
//! until RFC-001 S7). It is not re-exported by the `gup` crate until the
//! RFC-001 S14 flip, and it never depends on `gup`.
//!
//! ## Contracts
//!
//! - **One device per process by default.** [`Context::shared`] is the
//!   process-wide context; the old `gup` crate's `RenderContext` and
//!   `GupContext` take their device from it too (RFC-001 S1).
//! - **Every GPU write is counted.** All buffer and texture writes go
//!   through [`Context`], counted by [`Upload`] kind in
//!   [`Context::upload_stats`]. The `every_gpu_write_is_counted` test fails
//!   if any source file writes to GPU memory another way, so claims such as
//!   "zooming writes 0 column bytes" stay provable as the crate grows.
//! - **Lock order.** A context's pipeline cache and its text system are
//!   never held together. Debug builds check this on every acquisition.
//! - **GPU errors are `Err`, never a panic or a blank image.** Pipeline
//!   creation, [`Plot::resolve`], [`Renderer::prepare`], every target's
//!   render (encoding and submission) and target and surface setup run
//!   inside WebGPU error scopes. A validation, out-of-memory or (natively)
//!   internal error becomes [`Error::Gpu`], naming the pipeline (label and
//!   glue signature), layer or target, with wgpu's or the browser's
//!   message (for example a WGSL diagnostic). A failed pipeline is never
//!   cached: the next render creates it again and reports it again.
//!
//!   Natively the error comes back from the call that caused it. In a
//!   browser WebGPU reports errors asynchronously:
//!   [`ImageTarget::render`] and [`ImageTarget::read`] wait for them, so
//!   an awaited render returns its own error; synchronous calls
//!   ([`Renderer::render`] into a texture or window, [`Renderer::prepare`],
//!   [`Plot::resolve`]) return errors that arrived since the previous call,
//!   so a failing frame is reported by the next one. Errors are per
//!   [`Context`]: any call on it may report them. On a device Gup created
//!   in a browser, errors outside every scope are recorded too (wgpu sets
//!   no handler there); a host's device ([`Context::from_wgpu`]) keeps the
//!   host's handler.

/// The exact wgpu version Gup is built on. Hosts passing devices to
/// [`Context::from_wgpu`] must use the same major version.
pub use wgpu;

pub mod channel;
mod chart;
pub mod column;
mod context;
mod encoding;
mod error;
pub mod geom;
mod guide;
mod input;
pub mod marks;
mod order;
mod pick;
pub mod plot;
mod render;
pub mod scale;
pub mod scene;
mod scope;
mod selection;
mod shader;
#[cfg(all(feature = "window", not(target_arch = "wasm32")))]
mod show;
#[cfg(any(feature = "window", all(feature = "web", target_arch = "wasm32")))]
mod surface;
mod svg;
pub mod target;
mod theme;
mod tooltip;
#[cfg(all(feature = "web", target_arch = "wasm32"))]
pub mod web;
#[cfg(feature = "window")]
mod window;

pub use channel::{
    Antialias, Channel, ChannelDesc, Color, ConstValue, Geometry, GpuType, HitShape, Mark, Px,
    Role, Unit, Visual,
};
pub use chart::{Chart, ChartExt};
pub use context::{Caps, Context, ContextId, ContextOptions, Upload, UploadStats, WriteCount};
pub use encoding::{
    ColumnMarker, ColumnValue, ConstMarker, CpuMirror, Derived, DerivedMarker, DynColumnEncoding,
    DynShaderFn, EncodeFn, Encoded, Encoding, Feeds, IntoEncoding, Key, KeyAccessor, KeyEncoded,
    KeyMarker, KeySource, NullableKey, NullableOwnedKey, OwnedKey, Resource, ShaderFn, Then,
};
pub use error::{Error, Result};
pub use input::{Button, Event, EventInfo, Input, Response};
pub use marks::{Circle, CircleAttrs, Line, LineAttrs, Rect, RectAttrs, Segment, SegmentAttrs};
pub use pick::{PICK_TOLERANCE, RowId};
pub use plot::{Layout, PlacedText, Plot, Resolved};
pub use render::{PipelineStats, Prepared, Renderer, TargetDesc};
pub use scale::{
    Band, CATEGORICAL_COLORS, Categorical, ColorScale, Diverging, Legend, Linear, Log, MAX_PALETTE,
    NULL_COLOR, OKABE_ITO, Point, PositionScale, Pow, Ramp, RampTick, ScaleRef, Sequential, Swatch,
    Symlog, Ticks, Time,
};
pub use scene::Scene;
pub use selection::{Retain, Selection};
pub use shader::WgslModule;
#[cfg(all(feature = "window", not(target_arch = "wasm32")))]
pub use show::show;
pub use svg::{SvgOptions, SvgTarget, VectorTarget};
pub use target::{DEFAULT_SAMPLES, Frame, ImageTarget, RenderTarget, TargetOptions, TextureTarget};
pub use theme::Theme;
#[cfg(feature = "window")]
pub use window::WindowTarget;

// `#[wgsl_function]` names `::gup_core`, in this crate's own tests too.
extern crate self as gup_core;

/// Write a shader function (a scale, transfer function or palette) as WGSL
/// in your own crate: `#[wgsl_function]` completes its [`ShaderFn`] impl,
/// and checks the WGSL when your crate compiles.
///
/// Put it on an `impl ShaderFn for YourType` block holding the WGSL:
///
/// ```
/// use gup_core::prelude::*;
///
/// /// The energy an earthquake releases, in joules, from its magnitude:
/// /// log10 E = 1.5 M + 4.8.
/// #[derive(Clone)]
/// struct Energy;
///
/// #[wgsl_function(monotonic, wgsl = r#"
///     struct Params { slope: f32, offset: f32 }
///
///     fn energy(magnitude: f32, p: Params) -> f32 {
///         return pow(10.0, p.slope * magnitude + p.offset);
///     }
/// "#)]
/// impl ShaderFn for Energy {
///     fn params(&self) -> EnergyParams {
///         EnergyParams { slope: 1.5, offset: 4.8 }
///     }
/// }
///
/// // Its f64 twin, for axes and legends, and to start a chain.
/// impl CpuMirror for Energy {
///     fn eval(&self, magnitude: f64) -> f64 {
///         10f64.powf(1.5 * magnitude + 4.8)
///     }
/// }
///
/// // Size each quake by the energy it released: its area grows with the
/// // energy, through a square-root radius.
/// struct Quake { lon: f64, lat: f64, magnitude: f64 }
/// let quakes = vec![
///     Quake { lon: 142.4, lat: 38.3, magnitude: 7.8 },
///     Quake { lon: 142.9, lat: 38.0, magnitude: 6.1 },
///     Quake { lon: 141.8, lat: 37.6, magnitude: 5.2 },
///     Quake { lon: 142.1, lat: 38.9, magnitude: 4.4 },
/// ];
/// let area = Energy.then(Pow::sqrt().domain(0.0, Energy.eval(7.8)).range(Px(0.0), Px(30.0)));
///
/// let cx = Context::new_blocking()?;
/// let mut plot = Plot::new();
/// let (x, y) = (plot.x(Linear::new()), plot.y(Linear::new()));
/// plot.add(Selection::<Quake, Circle>::new(quakes))
///     .attr(Circle::X, x.encode(|q: &Quake| q.lon))
///     .attr(Circle::Y, y.encode(|q: &Quake| q.lat))
///     .attr(Circle::RADIUS, area.encode(|q: &Quake| q.magnitude));
/// let (image, _) = plot.render_resolved(&cx, 480, 320)?;
///
/// // The main shock's disc is 30 px in radius; one 1.7 magnitudes
/// // smaller released 1/355 of its energy, so its radius is 30/√355.
/// assert_eq!(area.eval(7.8), 30.0);
/// assert!((area.eval(6.1) - 30.0 / 10f64.powf(1.5 * 1.7).sqrt()).abs() < 1e-9);
/// let (px, py) = (x.read().eval(142.4), y.read().eval(38.3));
/// let inside = image.get_pixel(px as u32, (py - 27.0) as u32).0;
/// let outside = image.get_pixel(px as u32, (py - 33.0) as u32).0;
/// assert_ne!(inside, outside, "the main shock's disc ends at 30 px");
/// # Ok::<(), gup_core::Error>(())
/// ```
///
/// # What you write
///
/// - **The WGSL** (`wgsl = r#"…"#`, or `file = "src/energy.wgsl"`
///   relative to your crate's manifest): a `struct Params` of uniform
///   values and the function the glue calls, in one of these shapes:
///
///   | Signature | Input |
///   | --- | --- |
///   | `fn f(v: f32, p: Params) -> T` | a number |
///   | `fn f(v: f32, base: f32, p: Params) -> T` | a number stored relative to its chunk; `base` comes from [`ShaderFn::chunk_base`] |
///   | `fn f(v: vec2<f32>, base: vec2<f32>, p: Params) -> T` | a hi/lo pair relative to its chunk, for deep zoom |
///   | `fn f(code: u32, p: Params) -> T` | a dictionary code ([`EncodeFn::encode_key`]) |
///
///   `T` is `f32` (a number) or `vec4<f32>` (a [`Color`]). A palette adds
///   `lut: texture_2d<f32>, smp: sampler` after `p`, read with
///   `textureSampleLevel` or `textureLoad` (the glue calls the function
///   from the vertex stage). It may `#import` Gup's library
///   (`#import gup::scale::linear as linear`) and declare helper
///   functions; with more than one function, `entry = "f"` names the one
///   the glue calls. `Params` members are `f32`, `u32`, `i32` or
///   `vec4<f32>`. Uniform `Params` structs span a multiple of 16 bytes, and
///   the macro pads yours in the flattened WGSL, so you need not.
/// - **The impl's dynamic half**: `fn params(&self) -> <Type>Params`,
///   returning the generated twin of `Params` (named after your type, with
///   one public field per member),
///   and any of [`fit_domain`](ShaderFn::fit_domain),
///   [`fit_keys`](ShaderFn::fit_keys) (for functions over codes),
///   [`chunk_base`](ShaderFn::chunk_base),
///   [`resources`](ShaderFn::resources) (the LUT) and
///   [`image`](ShaderFn::image). Write `type Out = Px;` for a function
///   whose `f32` result is pixels (a position or size); otherwise an `f32`
///   result is a plain number, which can feed another function.
/// - **A [`CpuMirror`]**, to start a chain ([`EncodeFn::then`]) or place
///   axes and legends. With `monotonic`, the macro derives
///   [`ShaderFn::image`] from it, so a plot sizing marks through your
///   function keeps them inside the plot.
///
/// The macro derives the rest: `In`, `Out`, `Params`, `MODULE`, `ENTRY`
/// and `input_format`, from the WGSL.
///
/// # Checked when your crate compiles
///
/// The WGSL is composed with naga_oil against Gup's library at expansion,
/// so these are compile errors at your code, with naga_oil's report
/// pointing at the WGSL line: syntax and type errors, a wrong number of
/// arguments, an unknown import, identifiers that end in a digit (naga
/// renames them), a `Params` member with no Rust twin, a signature the
/// glue cannot call, operations the vertex stage lacks (`textureSample`,
/// derivatives), resource bindings, and impl items that contradict the
/// WGSL. Anything only the GPU sees fails at pipeline
/// creation as [`Error::Gpu`], whose message names your module.
///
/// # Import path
///
/// The module's WGSL import path is `<crate>::<type in snake_case>`
/// (`my_crate::energy`), or `path = "my_crate::seismic::energy"`. Two
/// functions with one path in a plot fail with [`Error::Compose`] naming
/// both types; the `gup::` namespace is Gup's own.
#[doc(inline)]
pub use gup_core_macros::wgsl_function;

/// The derive for [`Mark`]; see its documentation.
#[doc(inline)]
pub use gup_core_macros::Mark;

/// The names most programs need.
pub mod prelude {
    pub use crate::{
        Band, Categorical, Chart, ChartExt, Circle, CircleAttrs, Color, ColorScale, Context,
        CpuMirror, Derived, Diverging, EncodeFn, Event, Line, LineAttrs, Linear, Log, Mark, Plot,
        Point, PositionScale, Pow, Px, Rect, RectAttrs, Retain, ScaleRef, Segment, SegmentAttrs,
        Selection, Sequential, ShaderFn, Symlog, Theme, Time, Unit, wgsl_function,
    };
}

/// What `#[wgsl_function]`'s expansion names. Not public API.
#[doc(hidden)]
pub mod __private {
    pub use crate::encoding::DictionarySlot;
    pub use encase;

    /// The library's flattened modules (`VIEW`, `SCALE_LINEAR`, …), which a
    /// user module imports.
    pub mod wgsl {
        pub use crate::shader::*;
    }

    /// Autoref specialisation: `(&MirrorProbe(f)).sampled(extent)` is a
    /// sampled ramp when the function's type has a colour CPU mirror, and
    /// `None` when it has not (or is generic).
    pub struct MirrorProbe<'a, T>(pub &'a T);

    /// See [`MirrorProbe`].
    pub trait MirrorSampled {
        /// The sampled ramp over `extent`.
        fn sampled(&self, extent: (f64, f64)) -> Option<crate::Legend>;
    }

    impl<T: crate::CpuMirror + crate::EncodeFn<Output = crate::Color>> MirrorSampled
        for MirrorProbe<'_, T>
    {
        fn sampled(&self, extent: (f64, f64)) -> Option<crate::Legend> {
            Some(crate::Legend::Ramp(crate::Ramp::sampled(
                extent,
                |x| self.0.eval(x),
                5,
            )))
        }
    }

    /// See [`MirrorProbe`].
    pub trait MirrorFallback {
        /// No legend.
        fn sampled(&self, extent: (f64, f64)) -> Option<crate::Legend>;
    }

    impl<T> MirrorFallback for &MirrorProbe<'_, T> {
        fn sampled(&self, _extent: (f64, f64)) -> Option<crate::Legend> {
            None
        }
    }

    /// See `scale::monotonic_image`.
    pub fn monotonic_image(f: impl Fn(f64) -> f64, extent: (f64, f64)) -> Option<(f64, f64)> {
        crate::scale::monotonic_image(f, extent)
    }

    /// `a == b`, in a constant.
    pub const fn str_eq(a: &str, b: &str) -> bool {
        let (a, b) = (a.as_bytes(), b.as_bytes());
        if a.len() != b.len() {
            return false;
        }
        let mut i = 0;
        while i < a.len() {
            if a[i] != b[i] {
                return false;
            }
            i += 1;
        }
        true
    }
}

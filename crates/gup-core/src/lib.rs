// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! # gup-core
//!
//! Gup's new core, built to [RFC-001](https://github.com/au-phiware/gup/blob/main/docs/planning/rfcs/RFC-001_Core_Architecture.md):
//!
//! - one cloneable [`Context`] that owns or wraps a wgpu device;
//! - a GPU [column store](mod@column) — accessors run once, columns live on
//!   the GPU as instance-rate vertex buffers, relative to a per-chunk f64
//!   origin; string keys are dictionary-encoded ([`ShaderFn::encode_key`]),
//!   nulls are validity bits or a reserved code (a null position is not
//!   drawn, a null colour is [`NULL_COLOR`]), and [`Retain`] says how much
//!   CPU data to keep after upload;
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
//! The data side is still the S0 vertical slice: one mark ([`Circle`]), four
//! scales ([`Linear`], [`Log`], [`Sequential`] and a minimal [`Categorical`])
//! and a headless PNG path. It is not re-exported by the `gup` crate until
//! the RFC-001 S14 flip, and it never depends on `gup`.
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
pub mod column;
mod context;
mod encoding;
mod error;
pub mod geom;
pub mod marks;
pub mod plot;
mod render;
pub mod scale;
pub mod scene;
mod scope;
mod selection;
mod shader;
#[cfg(all(feature = "window", not(target_arch = "wasm32")))]
mod show;
mod svg;
pub mod target;
#[cfg(feature = "window")]
mod window;

pub use channel::{Channel, ChannelDesc, Color, ConstValue, GpuType, Mark, Px, Role, Visual};
pub use context::{Caps, Context, ContextId, ContextOptions, Upload, UploadStats, WriteCount};
pub use encoding::{
    ColumnMarker, ColumnValue, ConstMarker, CpuMirror, DynColumnEncoding, DynShaderFn, EncodeFn,
    Encoded, Encoding, Feeds, IntoEncoding, Key, KeyAccessor, KeyEncoded, KeyMarker, KeySource,
    NullableKey, NullableOwnedKey, OwnedKey, Resource, ShaderFn, Then,
};
pub use error::{Error, Result};
pub use marks::Circle;
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
#[cfg(feature = "window")]
pub use window::WindowTarget;

/// The names most programs need.
pub mod prelude {
    pub use crate::{
        Band, Categorical, Circle, Color, ColorScale, Context, CpuMirror, Diverging, EncodeFn,
        Linear, Log, Plot, Point, PositionScale, Pow, Px, Retain, ScaleRef, Selection, Sequential,
        ShaderFn, Symlog, Time,
    };
}

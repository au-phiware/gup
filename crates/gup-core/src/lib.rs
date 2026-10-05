// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! # gup-core
//!
//! Gup's new core, built to [RFC-001](https://github.com/au-phiware/gup/blob/main/docs/planning/rfcs/RFC-001_Core_Architecture.md):
//!
//! - one cloneable [`Context`] that owns or wraps a wgpu device;
//! - a GPU [column store](column) — accessors run once, columns live on
//!   the GPU as instance-rate vertex buffers, relative to a per-chunk f64
//!   origin;
//! - typed [channels](channel): `Circle::RADIUS: Channel<Circle, Px>`, so
//!   wrong value types, unknown channels and wrong marks don't compile;
//! - one [scale](scale) family, each a [`ShaderFn`] with an f64
//!   [`CpuMirror`] — zooming changes uniforms only;
//! - shader composition with naga_oil library modules plus a typed glue
//!   emitter — no find-and-replace on authored WGSL;
//! - a resolved [`Scene`] drawn by a target ([`ImageTarget`]).
//!
//! This is the S0a vertical slice: one mark ([`Circle`]), three scales
//! ([`Linear`], [`Log`], [`Sequential`]) and a headless PNG path. It is not
//! re-exported by the `gup` crate until the RFC-001 S14 flip, and it never
//! depends on `gup`.
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
//! - **Lock order.** A context's pipeline cache may be held while its
//!   shader library is taken, never the reverse, and its text system is
//!   never held with either. Debug builds check this on every acquisition.

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
mod selection;
mod shader;
#[cfg(all(feature = "window", not(target_arch = "wasm32")))]
mod show;
pub mod target;
#[cfg(feature = "window")]
mod window;

pub use channel::{Channel, ChannelDesc, Color, ConstValue, GpuType, Mark, Px, Role, Visual};
pub use context::{Caps, Context, ContextId, ContextOptions, Upload, UploadStats, WriteCount};
pub use encoding::{
    ColumnMarker, ColumnValue, ConstMarker, CpuMirror, DynColumnEncoding, DynShaderFn, Encoded,
    Encoding, IntoEncoding, Resource, ShaderFn,
};
pub use error::{Error, Result};
pub use marks::Circle;
pub use plot::{Layout, PlacedText, Plot, Resolved};
pub use render::{PipelineStats, Prepared, Renderer, TargetDesc};
pub use scale::{Linear, Log, PositionScale, ScaleRef, Sequential, Ticks};
pub use scene::Scene;
pub use selection::Selection;
pub use shader::WgslModule;
#[cfg(all(feature = "window", not(target_arch = "wasm32")))]
pub use show::show;
pub use target::{DEFAULT_SAMPLES, Frame, ImageTarget, RenderTarget, TargetOptions, TextureTarget};
#[cfg(feature = "window")]
pub use window::WindowTarget;

/// The names most programs need.
pub mod prelude {
    pub use crate::{
        Circle, Color, Context, CpuMirror, Linear, Log, Plot, PositionScale, Px, Selection,
        Sequential, ShaderFn,
    };
}

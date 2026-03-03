// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Adaptive viewport renderer for LOD-driven rendering.
//!
//! This module provides [`AdaptiveRenderer`], which selects the appropriate LOD
//! tier from a [`LodPyramid`] each frame based on the current viewport and
//! issues a frustum-culled indirect draw via [`ComputeInstanceFilter`].
//!
//! # Architecture
//!
//! The render path is:
//!
//! 1. **Tier selection** — a pure function picks the coarsest LOD tier whose
//!    on-screen density yields ≥ 1 pixel per visible point.
//! 2. **Blend state update** — if the selected tier changed, a cross-fade
//!    transition is started over a configurable number of frames.
//! 3. **Frustum culling** — a compute pass discards off-screen points.
//! 4. **Indirect draw** — the compacted buffer drives an indirect draw call.
//! 5. **Debug overlay** (optional) — shows active tier and visible count.

mod adaptive;
mod blend;
mod debug_overlay;
mod viewport;
mod viewport_cull;

pub use adaptive::{AdaptiveRenderer, AdaptiveRendererConfig};
pub use blend::LodBlendState;
pub use debug_overlay::{DebugOverlay, DebugOverlayInfo};
pub use viewport::AdaptiveViewport;
pub use viewport_cull::{ViewportCullResult, ViewportCuller};

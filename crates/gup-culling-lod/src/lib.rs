// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! Experimental GPU culling and level-of-detail (LOD) code for Gup.
//!
//! **Not wired into the `gup` render path.** This crate holds GPU culling and
//! LOD code that was quarantined out of the `gup` crate (GUP-390, strategic
//! review 2026-10, Decision 1) because no chart builder uses it, but it is
//! tested GPU engineering that the RFC-001 core (GPU column chunk culling, the
//! billion-point goal) is expected to draw from. See `README.md`.
//!
//! Modules keep the paths they had inside `gup`:
//!
//! - [`mark`]: instanced batch renderer and CPU culling manager, GPU compute
//!   instance filter, Hi-Z occlusion culler, GPU radix sort and the unified
//!   frustum + occlusion pipeline.
//! - [`lod`]: GPU-aggregated LOD pyramid, tier selection and streaming LOD.
//! - [`renderer`]: adaptive viewport renderer that picks a LOD tier per frame
//!   and draws it through a frustum-culled indirect draw.

#![deny(missing_docs)]

pub mod lod;
pub mod mark;
pub mod renderer;

pub use mark::batch_renderer::{
    BatchFrameStats, BatchRendererConfig, CullingManager, GeometryCache, InstanceAttributes,
    InstancedBatchRenderer, LodLevel, RenderBatch, Viewport2D,
};
pub use mark::compute_instance_filter::{
    ComputeInstanceFilter, FilterConfig, FilterResult, PooledComputeInstanceFilter,
};
pub use mark::occlusion_culler::{
    OcclusionCuller, OcclusionGpuConfig, OcclusionParams, OcclusionResult, PooledOcclusionCuller,
};
pub use mark::radix_sort::{RadixSorter, SortBuffers, SortConfig};
pub use mark::unified_culling_pipeline::UnifiedCullingPipeline;

// Copyright (C) 2024 Corin Lawson
// SPDX-License-Identifier: GPL-3.0-or-later

//! GPU culling for mark instances: batch rendering, compute instance
//! filtering, occlusion culling and depth sorting.
//!
//! These modules lived under `gup::mark` before GUP-390 and keep their names.
//! They operate on [`InstanceAttributes`] and [`Viewport2D`] rather than on the
//! `gup` chart builders, which never called them.

pub mod batch_renderer;
pub mod compute_instance_filter;
pub mod occlusion_culler;
pub mod radix_sort;
pub mod unified_culling_pipeline;

pub use batch_renderer::{
    BatchFrameStats, BatchRendererConfig, CullingManager, GeometryCache, InstanceAttributes,
    InstancedBatchRenderer, LodLevel, RenderBatch, Viewport2D,
};
pub use compute_instance_filter::{ComputeInstanceFilter, FilterConfig, FilterResult};
pub use occlusion_culler::{
    OcclusionCuller, OcclusionGpuConfig, OcclusionParams, OcclusionResult, PooledOcclusionCuller,
};
pub use unified_culling_pipeline::UnifiedCullingPipeline;
